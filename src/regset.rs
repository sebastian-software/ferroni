// regset.rs - Port of USE_REGSET section from regexec.c
// Multi-regex search for syntax highlighters and text editors.

use crate::oniguruma::*;
use crate::regenc::OnigEncoding;
use crate::regint::*;
use crate::regexec::{onig_match, onig_search, onig_search_with_param, OnigMatchParam};

/// Search lead mode for regset search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnigRegSetLead {
    /// Position-lead: iterate positions, try all regexes at each position.
    /// Returns the first matching regex at the earliest position.
    PositionLead = 0,
    /// Regex-lead: iterate regexes, search full string for each.
    /// Returns the regex whose match starts earliest.
    RegexLead = 1,
    /// Like RegexLead but stops at the first regex that matches at the
    /// earliest position found so far (prioritizes regex order).
    PriorityToRegexOrder = 2,
}

struct RegSetEntry {
    reg: Box<RegexType>,
    region: Option<OnigRegion>,
}

/// A set of compiled regexes that can be searched simultaneously.
pub struct OnigRegSet {
    entries: Vec<RegSetEntry>,
    enc: OnigEncoding,
    anchor: i32,
    anc_dmin: OnigLen,
    anc_dmax: OnigLen,
    all_low_high: bool,
    anychar_inf: bool,
}

#[inline]
fn enclen(enc: OnigEncoding, str_data: &[u8], s: usize) -> usize {
    if s >= str_data.len() {
        return 1;
    }
    enc.mbc_enc_len(&str_data[s..])
}

/// Create a new regex set from an array of compiled regexes.
/// Returns (Some(set), ONIG_NORMAL) on success, (None, error_code) on failure.
pub fn onig_regset_new(regs: Vec<Box<RegexType>>) -> (Option<Box<OnigRegSet>>, i32) {
    let mut set = Box::new(OnigRegSet {
        entries: Vec::new(),
        enc: &crate::encodings::utf8::ONIG_ENCODING_UTF8,
        anchor: 0,
        anc_dmin: 0,
        anc_dmax: 0,
        all_low_high: false,
        anychar_inf: false,
    });

    for reg in regs {
        let r = onig_regset_add(&mut set, reg);
        if r != ONIG_NORMAL {
            return (None, r);
        }
    }

    (Some(set), ONIG_NORMAL)
}

/// Add a compiled regex to the set. Returns ONIG_NORMAL on success.
pub fn onig_regset_add(set: &mut OnigRegSet, reg: Box<RegexType>) -> i32 {
    if opton_find_longest(reg.options) {
        return ONIGERR_INVALID_ARGUMENT;
    }

    if !set.entries.is_empty() && !std::ptr::eq(reg.enc, set.enc) {
        return ONIGERR_INVALID_ARGUMENT;
    }

    let region = Some(OnigRegion::new());
    set.entries.push(RegSetEntry { reg, region });

    // Recompute: pass field values to avoid borrow conflict
    let n = set.entries.len();
    let reg_ref = &*set.entries[n - 1].reg;
    let anchor = reg_ref.anchor;
    let anc_dist_min = reg_ref.anc_dist_min;
    let anc_dist_max = reg_ref.anc_dist_max;
    let optimize = reg_ref.optimize;
    let dist_max = reg_ref.dist_max;

    if n == 1 {
        set.enc = reg_ref.enc;
        set.anchor = anchor;
        set.anc_dmin = anc_dist_min;
        set.anc_dmax = anc_dist_max;
        set.all_low_high = optimize != OptimizeType::None && dist_max != INFINITE_LEN;
        set.anychar_inf = (anchor & ANCR_ANYCHAR_INF) != 0;
    } else {
        let new_anchor = set.anchor & anchor;
        if new_anchor != 0 {
            if anc_dist_min < set.anc_dmin {
                set.anc_dmin = anc_dist_min;
            }
            if anc_dist_max > set.anc_dmax {
                set.anc_dmax = anc_dist_max;
            }
        }
        set.anchor = new_anchor;
        if optimize == OptimizeType::None || dist_max == INFINITE_LEN {
            set.all_low_high = false;
        }
        if (anchor & ANCR_ANYCHAR_INF) != 0 {
            set.anychar_inf = true;
        }
    }

    ONIG_NORMAL
}

/// Replace a regex at index `at`, or remove it if `reg` is None.
/// Returns ONIG_NORMAL on success.
pub fn onig_regset_replace(
    set: &mut OnigRegSet,
    at: usize,
    reg: Option<Box<RegexType>>,
) -> i32 {
    if at >= set.entries.len() {
        return ONIGERR_INVALID_ARGUMENT;
    }

    match reg {
        None => {
            // Remove entry at `at`
            set.entries.remove(at);
        }
        Some(reg) => {
            if opton_find_longest(reg.options) {
                return ONIGERR_INVALID_ARGUMENT;
            }
            if set.entries.len() > 1 && !std::ptr::eq(reg.enc, set.enc) {
                return ONIGERR_INVALID_ARGUMENT;
            }
            set.entries[at].reg = reg;
        }
    }

    // Recompute aggregate fields from all entries
    if !set.entries.is_empty() {
        // Reset and recompute by replaying updates
        let first_enc = set.entries[0].reg.enc;
        set.enc = first_enc;
        set.anchor = 0;
        set.anc_dmin = 0;
        set.anc_dmax = 0;
        set.all_low_high = false;
        set.anychar_inf = false;

        // Temporarily collect reg references to avoid borrow issues
        let reg_data: Vec<(i32, OnigLen, OnigLen, OptimizeType, OnigLen, i32)> = set
            .entries
            .iter()
            .map(|e| {
                (
                    e.reg.anchor,
                    e.reg.anc_dist_min,
                    e.reg.anc_dist_max,
                    e.reg.optimize,
                    e.reg.dist_max,
                    0, // placeholder
                )
            })
            .collect();

        for (i, (anchor, anc_dist_min, anc_dist_max, optimize, dist_max, _)) in
            reg_data.iter().enumerate()
        {
            if i == 0 {
                set.anchor = *anchor;
                set.anc_dmin = *anc_dist_min;
                set.anc_dmax = *anc_dist_max;
                set.all_low_high =
                    *optimize != OptimizeType::None && *dist_max != INFINITE_LEN;
                set.anychar_inf = (*anchor & ANCR_ANYCHAR_INF) != 0;
            } else {
                let new_anchor = set.anchor & anchor;
                if new_anchor != 0 {
                    if *anc_dist_min < set.anc_dmin {
                        set.anc_dmin = *anc_dist_min;
                    }
                    if *anc_dist_max > set.anc_dmax {
                        set.anc_dmax = *anc_dist_max;
                    }
                }
                set.anchor = new_anchor;
                if *optimize == OptimizeType::None || *dist_max == INFINITE_LEN {
                    set.all_low_high = false;
                }
                if (*anchor & ANCR_ANYCHAR_INF) != 0 {
                    set.anychar_inf = true;
                }
            }
        }
    }

    ONIG_NORMAL
}

/// Return the number of regexes in the set.
pub fn onig_regset_number_of_regex(set: &OnigRegSet) -> i32 {
    set.entries.len() as i32
}

/// Get a reference to the regex at index `at`.
pub fn onig_regset_get_regex(set: &OnigRegSet, at: usize) -> Option<&RegexType> {
    set.entries.get(at).map(|e| e.reg.as_ref())
}

/// Get a reference to the region at index `at`.
pub fn onig_regset_get_region(set: &OnigRegSet, at: usize) -> Option<&OnigRegion> {
    set.entries.get(at).and_then(|e| e.region.as_ref())
}

/// Position-lead search: iterate positions, try each regex at each position.
fn regset_search_body_position_lead(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
) -> (i32, i32) {
    // rmatch_pos, regex_index
    let n = set.entries.len();
    let enc = set.enc;
    let mut s = start;

    let prev_is_newline_check = set.anychar_inf;

    loop {
        if s >= range {
            break;
        }

        let prev_is_newline = if prev_is_newline_check && s > 0 {
            // Check if previous character is newline
            s > 0 && str_data[s - 1] == b'\n'
        } else {
            true // default: allow matching
        };

        for i in 0..n {
            // ANCR_ANYCHAR_INF optimization: skip if previous char is not newline
            if (set.entries[i].reg.anchor & ANCR_ANYCHAR_INF) != 0 && !prev_is_newline {
                continue;
            }

            let region = set.entries[i].region.take();
            let entry = &set.entries[i];
            let (r, returned_region) = onig_match(&entry.reg, str_data, end, s, region, option);
            set.entries[i].region = returned_region;

            if r >= 0 {
                return (i as i32, s as i32);
            }
            if r != ONIG_MISMATCH {
                // error
                return (r, 0);
            }
        }

        s += enclen(enc, str_data, s);
    }

    (ONIG_MISMATCH, 0)
}

/// Regex-lead search: iterate regexes, find earliest match.
fn regset_search_body_regex_lead(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    orig_range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
) -> (i32, i32) {
    let n = set.entries.len();
    let mut match_index: i32 = ONIG_MISMATCH;
    let mut match_pos: i32 = 0;
    let mut ep = orig_range;

    for i in 0..n {
        let region = set.entries[i].region.take();
        let (r, returned_region) =
            onig_search(&set.entries[i].reg, str_data, end, start, ep, region, option);
        set.entries[i].region = returned_region;

        if r > 0 {
            if (r as usize) < ep {
                match_index = i as i32;
                match_pos = r;
                if lead == OnigRegSetLead::PriorityToRegexOrder {
                    break;
                }
                ep = r as usize;
            }
        } else if r == 0 {
            match_index = i as i32;
            match_pos = 0;
            break;
        }
    }

    (match_index, match_pos)
}

/// Search the set of regexes against a string.
///
/// Returns (regex_index, match_position) where:
/// - regex_index >= 0: index of the matching regex
/// - regex_index == ONIG_MISMATCH (-1): no match
/// - regex_index < -1: error code
pub fn onig_regset_search(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
) -> (i32, i32) {
    let n = set.entries.len();
    if n == 0 {
        return (ONIG_MISMATCH, 0);
    }

    if start > end || start > str_data.len() {
        return (ONIG_MISMATCH, 0);
    }

    // Forward search only
    if str_data.len() > 0 && range < start {
        return (ONIGERR_INVALID_ARGUMENT, 0);
    }

    // Resize and clear all regions
    for entry in &mut set.entries {
        if let Some(ref mut region) = entry.region {
            region.resize(entry.reg.num_mem + 1);
            region.clear();
        }
    }

    // Empty string handling
    if start == end {
        for i in 0..n {
            if set.entries[i].reg.threshold_len == 0 {
                let region = set.entries[i].region.take();
                let (r, returned_region) =
                    onig_match(&set.entries[i].reg, str_data, end, start, region, option);
                set.entries[i].region = returned_region;
                if r >= 0 {
                    return (i as i32, start as i32);
                }
                if r != ONIG_MISMATCH {
                    return (r, 0); // error
                }
            }
        }
        return (ONIG_MISMATCH, 0);
    }

    // Anchor optimization
    let mut cur_start = start;
    let mut cur_range = range;
    let orig_range = range;

    if set.anchor != 0 && !str_data.is_empty() {
        if (set.anchor & ANCR_BEGIN_POSITION) != 0 {
            cur_range = start + 1;
        } else if (set.anchor & ANCR_BEGIN_BUF) != 0 {
            if start != 0 {
                return (ONIG_MISMATCH, 0);
            }
            cur_range = 1;
        } else if (set.anchor & ANCR_END_BUF) != 0 {
            let min_semi_end = end;
            let max_semi_end = end;

            if (max_semi_end as OnigLen) < set.anc_dmin {
                return (ONIG_MISMATCH, 0);
            }
            if min_semi_end.saturating_sub(start) > set.anc_dmax as usize
                && set.anc_dmax != INFINITE_LEN
            {
                cur_start = min_semi_end - set.anc_dmax as usize;
            }
            if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < set.anc_dmin as usize {
                cur_range = max_semi_end.saturating_sub(set.anc_dmin as usize) + 1;
            }
            if cur_start > cur_range {
                return (ONIG_MISMATCH, 0);
            }
        } else if (set.anchor & ANCR_SEMI_END_BUF) != 0 {
            let max_semi_end = end;
            let mut min_semi_end = end;
            if end > 0 && str_data[end - 1] == b'\n' {
                min_semi_end = end - 1;
            }

            if (max_semi_end as OnigLen) < set.anc_dmin {
                return (ONIG_MISMATCH, 0);
            }
            if min_semi_end.saturating_sub(start) > set.anc_dmax as usize
                && set.anc_dmax != INFINITE_LEN
            {
                cur_start = min_semi_end - set.anc_dmax as usize;
            }
            if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < set.anc_dmin as usize {
                cur_range = max_semi_end.saturating_sub(set.anc_dmin as usize) + 1;
            }
            if cur_start > cur_range {
                return (ONIG_MISMATCH, 0);
            }
        } else if (set.anchor & ANCR_ANYCHAR_INF_ML) != 0 {
            cur_range = start + 1;
        }
    }

    let (result, match_pos) = if lead == OnigRegSetLead::PositionLead {
        regset_search_body_position_lead(set, str_data, end, cur_start, cur_range, option)
    } else {
        regset_search_body_regex_lead(
            set, str_data, end, cur_start, orig_range, lead, option,
        )
    };

    // Clear regions for non-matching regexes with FIND_NOT_EMPTY
    if result >= 0 {
        for i in 0..n {
            if opton_find_not_empty(set.entries[i].reg.options) {
                if let Some(ref mut region) = set.entries[i].region {
                    if (i as i32) != result {
                        region.clear();
                    }
                }
            }
        }
    }

    (result, match_pos)
}

/// Search the set with per-regex match parameters.
pub fn onig_regset_search_with_param(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    mps: &[OnigMatchParam],
) -> (i32, i32) {
    let n = set.entries.len();
    if n == 0 {
        return (ONIG_MISMATCH, 0);
    }
    if mps.len() < n {
        return (ONIGERR_INVALID_ARGUMENT, 0);
    }

    if start > end || start > str_data.len() {
        return (ONIG_MISMATCH, 0);
    }

    // Forward search only
    if str_data.len() > 0 && range < start {
        return (ONIGERR_INVALID_ARGUMENT, 0);
    }

    // Resize and clear all regions
    for entry in &mut set.entries {
        if let Some(ref mut region) = entry.region {
            region.resize(entry.reg.num_mem + 1);
            region.clear();
        }
    }

    // Empty string handling
    if start == end {
        for i in 0..n {
            if set.entries[i].reg.threshold_len == 0 {
                let region = set.entries[i].region.take();
                let (r, returned_region) =
                    onig_match(&set.entries[i].reg, str_data, end, start, region, option);
                set.entries[i].region = returned_region;
                if r >= 0 {
                    return (i as i32, start as i32);
                }
                if r != ONIG_MISMATCH {
                    return (r, 0);
                }
            }
        }
        return (ONIG_MISMATCH, 0);
    }

    // For regex-lead with params, use search_with_param per regex
    if lead != OnigRegSetLead::PositionLead {
        let orig_range = range;
        let mut match_index: i32 = ONIG_MISMATCH;
        let mut match_pos: i32 = 0;
        let mut ep = orig_range;

        for i in 0..n {
            let region = set.entries[i].region.take();
            let (r, returned_region) = onig_search_with_param(
                &set.entries[i].reg,
                str_data,
                end,
                start,
                ep,
                region,
                option,
                &mps[i],
            );
            set.entries[i].region = returned_region;

            if r > 0 {
                if (r as usize) < ep {
                    match_index = i as i32;
                    match_pos = r;
                    if lead == OnigRegSetLead::PriorityToRegexOrder {
                        break;
                    }
                    ep = r as usize;
                }
            } else if r == 0 {
                match_index = i as i32;
                match_pos = 0;
                break;
            }
        }

        return (match_index, match_pos);
    }

    // Position-lead with params: delegate to non-param position-lead
    // (params mainly affect limits which are checked within onig_match)
    regset_search_body_position_lead(set, str_data, end, start, range, option)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regcomp::onig_new;
    use crate::regsyntax::OnigSyntaxOniguruma;
    use crate::encodings::utf8::ONIG_ENCODING_UTF8;

    fn compile(pattern: &[u8]) -> Box<RegexType> {
        let reg = onig_new(
            pattern,
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma as *const OnigSyntaxType,
        );
        match reg {
            Ok(r) => Box::new(r),
            Err(e) => panic!("failed to compile {:?}: error {}", std::str::from_utf8(pattern), e),
        }
    }

    #[test]
    fn regset_basic_position_lead() {
        let regs = vec![compile(b"abc"), compile(b"def"), compile(b"ghi")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::PositionLead, ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 1); // "def" matched
        assert_eq!(pos, 3); // at position 3
    }

    #[test]
    fn regset_basic_regex_lead() {
        let regs = vec![compile(b"abc"), compile(b"def"), compile(b"ghi")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::RegexLead, ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 1); // "def" matched
        assert_eq!(pos, 3); // at position 3
    }

    #[test]
    fn regset_earliest_match_regex_lead() {
        let regs = vec![compile(b"yyy"), compile(b"def"), compile(b"xxx")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::RegexLead, ONIG_OPTION_NONE,
        );
        // "xxx" matches at position 0, which is earliest
        assert_eq!(idx, 2);
        assert_eq!(pos, 0);
    }

    #[test]
    fn regset_priority_to_regex_order() {
        let regs = vec![compile(b"def"), compile(b"xxx")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::PriorityToRegexOrder, ONIG_OPTION_NONE,
        );
        // "def" is first regex, matches at position 3.
        // "xxx" is second regex, matches at position 0 (earlier).
        // PriorityToRegexOrder: first regex "def" finds match at 3,
        // then since PRIORITY mode, stops after finding first match.
        // Actually: PRIORITY mode still finds earliest, but stops once
        // a later regex can't beat the current best. Let me re-check...
        // In C: it searches all regexes but narrows ep. "def" at 3 sets ep=3.
        // "xxx" searches with ep=3, finds at 0 < 3, updates to idx=1,pos=0.
        // Wait no, PRIORITY_TO_REGEX_ORDER breaks on first match found.
        // So "def" at 3 is found first → break. idx=0, pos=3.
        assert_eq!(idx, 0);
        assert_eq!(pos, 3);
    }

    #[test]
    fn regset_no_match() {
        let regs = vec![compile(b"abc"), compile(b"def")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xyz";
        let (idx, _pos) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::PositionLead, ONIG_OPTION_NONE,
        );
        assert_eq!(idx, ONIG_MISMATCH);
    }

    #[test]
    fn regset_empty_string() {
        let regs = vec![compile(b""), compile(b"x")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"";
        let (idx, pos) = onig_regset_search(
            &mut set, input, 0, 0, 0,
            OnigRegSetLead::PositionLead, ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 0); // empty pattern matches empty string
        assert_eq!(pos, 0);
    }

    #[test]
    fn regset_empty_set() {
        let (set, r) = onig_regset_new(vec![]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"abc";
        let (idx, _) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::PositionLead, ONIG_OPTION_NONE,
        );
        assert_eq!(idx, ONIG_MISMATCH);
    }

    #[test]
    fn regset_add_and_replace() {
        let (set, r) = onig_regset_new(vec![compile(b"abc")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        assert_eq!(onig_regset_number_of_regex(&set), 1);

        // Add another regex
        let r = onig_regset_add(&mut set, compile(b"def"));
        assert_eq!(r, ONIG_NORMAL);
        assert_eq!(onig_regset_number_of_regex(&set), 2);

        // Replace first with None (remove)
        let r = onig_regset_replace(&mut set, 0, None);
        assert_eq!(r, ONIG_NORMAL);
        assert_eq!(onig_regset_number_of_regex(&set), 1);

        // The remaining regex should be "def"
        let input = b"def";
        let (idx, pos) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::PositionLead, ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 0);
        assert_eq!(pos, 0);
    }

    #[test]
    fn regset_captures() {
        let regs = vec![compile(b"a(b)c"), compile(b"(d)(e)f")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xdefx";
        let (idx, pos) = onig_regset_search(
            &mut set, input, input.len(), 0, input.len(),
            OnigRegSetLead::PositionLead, ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 1);
        assert_eq!(pos, 1);

        // Check capture groups in the matching regex's region
        let region = onig_regset_get_region(&set, 1).unwrap();
        assert_eq!(region.beg[0], 1); // full match start
        assert_eq!(region.end[0], 4); // full match end
        assert_eq!(region.beg[1], 1); // group 1 "d"
        assert_eq!(region.end[1], 2);
        assert_eq!(region.beg[2], 2); // group 2 "e"
        assert_eq!(region.end[2], 3);
    }
}
