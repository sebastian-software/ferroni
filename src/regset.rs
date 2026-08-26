// regset.rs - Port of USE_REGSET section from regexec.c
// Multi-regex search for syntax highlighters and text editors.

use crate::oniguruma::*;
use crate::regenc::{onigenc_is_ascii_compatible_encoding, OnigEncoding};
use crate::regexec::{
    onig_match, onig_match_with_msa_start, onig_search, onig_search_with_param, MatchArg,
    OnigMatchParam,
};
use crate::regint::*;

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

/// A variable-distance optimizer target that can produce candidate match
/// starts. This is deliberately separate from first-byte dispatch: `min` and
/// `max` describe where the optimizer target may occur after the match start.
#[derive(Clone, Copy)]
struct VariableDistanceCandidate {
    index: u16,
    min: u8,
    max: u8,
}

/// Pre-computed memchr needle for SIMD-accelerated position skipping.
///
/// When the dispatch table has only 1–3 non-empty byte slots, we can use
/// `memchr` to jump directly to the next position where a fixed-first-byte
/// pattern could match.
#[derive(Clone, Copy)]
enum SkipNeedle {
    /// No skipping possible (>3 bytes, or no dispatch candidates).
    None,
    One(u8),
    Two(u8, u8),
    Three(u8, u8, u8),
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
    /// For each byte value 0..255, the list of entry indices whose optimizer
    /// proves a fixed first byte and does not exclude that byte.
    first_byte_candidates: Box<[Vec<u16>; 256]>,
    /// Entries whose optimizer information is not fixed at the match start.
    /// They have no bounded optimizer target and are checked directly.
    fallback_candidates: Vec<u16>,
    /// Variable-distance optimizer targets, indexed by their observed input
    /// byte. Hits yield candidate match starts instead of dispatching by the
    /// match's first byte.
    variable_distance_candidates: Box<[Vec<VariableDistanceCandidate>; 256]>,
    variable_distance_max: usize,
    variable_distance_seen: Vec<usize>,
    has_dispatch_candidates: bool,
    /// SIMD-accelerated skip needle derived from the dispatch table.
    skip_needle: SkipNeedle,
    /// Reused MatchArg scratch space for position-lead searches.
    scratch_msa: Option<MatchArg>,
    /// Match length from the last successful position-lead search.
    last_match_len: i32,
}

#[inline]
fn enclen(enc: OnigEncoding, str_data: &[u8], s: usize) -> usize {
    if s >= str_data.len() {
        return 1;
    }
    if str_data[s] < 0x80 && onigenc_is_ascii_compatible_encoding(enc) {
        return 1;
    }
    enc.mbc_enc_len(&str_data[s..])
}

/// Add an entry to the first-byte table if its optimizer data is fixed at the match start.
fn add_entry_to_first_byte_table(table: &mut [Vec<u16>; 256], reg: &RegexType, idx: u16) -> bool {
    if reg.optimize == OptimizeType::Map && reg.dist_max == 0 {
        // Map-filterable: only bytes where map[b] != 0
        for (b, slot) in table.iter_mut().enumerate() {
            if reg.map[b] != 0 {
                slot.push(idx);
            }
        }
        true
    } else if reg.dist_max == 0 && !reg.exact.is_empty() {
        // Exact-filterable: only the first byte of the exact string
        table[reg.exact[0] as usize].push(idx);
        true
    } else if reg.has_first_byte_map {
        // The map is recorded only when it is fixed at the match start.
        for (b, slot) in table.iter_mut().enumerate() {
            if reg.first_byte_map[b] != 0 {
                slot.push(idx);
            }
        }
        true
    } else {
        false
    }
}

/// Index an optimizer target with a small, finite distance window. The caller
/// still validates the regex VM at each derived match start.
fn add_variable_distance_candidate(
    table: &mut [Vec<VariableDistanceCandidate>; 256],
    reg: &RegexType,
    idx: u16,
) -> Option<usize> {
    const MAX_VARIABLE_DISTANCE_SPAN: usize = 8;

    if reg.dist_min == INFINITE_LEN || reg.dist_max == INFINITE_LEN {
        return None;
    }
    let min = reg.dist_min as usize;
    let max = reg.dist_max as usize;
    if max < min || max - min > MAX_VARIABLE_DISTANCE_SPAN || max > u8::MAX as usize {
        return None;
    }

    let candidate = VariableDistanceCandidate {
        index: idx,
        min: min as u8,
        max: max as u8,
    };
    match reg.optimize {
        OptimizeType::Map => {
            for (byte, slot) in table.iter_mut().enumerate() {
                if reg.map[byte] != 0 {
                    slot.push(candidate);
                }
            }
        }
        OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward
            if !reg.exact.is_empty() =>
        {
            table[reg.exact[0] as usize].push(candidate);
        }
        _ => return None,
    }

    Some(max)
}

/// Derive the skip needle from a completed dispatch table.
fn compute_skip_needle(table: &[Vec<u16>; 256]) -> SkipNeedle {
    let mut bytes: Vec<u8> = Vec::new();
    for (b, slot) in table.iter().enumerate() {
        if !slot.is_empty() {
            bytes.push(b as u8);
            if bytes.len() > 3 {
                return SkipNeedle::None;
            }
        }
    }
    match bytes.len() {
        0 => SkipNeedle::None,
        1 => SkipNeedle::One(bytes[0]),
        2 => SkipNeedle::Two(bytes[0], bytes[1]),
        3 => SkipNeedle::Three(bytes[0], bytes[1], bytes[2]),
        _ => SkipNeedle::None,
    }
}

/// Build the first-byte dispatch table from scratch for all entries.
fn build_first_byte_table(set: &mut OnigRegSet) {
    let mut table: Box<[Vec<u16>; 256]> = Box::new(std::array::from_fn(|_| Vec::new()));
    let mut variable_distance_candidates: Box<[Vec<VariableDistanceCandidate>; 256]> =
        Box::new(std::array::from_fn(|_| Vec::new()));
    let mut fallback_candidates = Vec::new();
    let mut variable_distance_max = 0;
    for (i, entry) in set.entries.iter().enumerate() {
        if !add_entry_to_first_byte_table(&mut table, &entry.reg, i as u16) {
            if let Some(max) = add_variable_distance_candidate(
                &mut variable_distance_candidates,
                &entry.reg,
                i as u16,
            ) {
                variable_distance_max = variable_distance_max.max(max);
            } else {
                fallback_candidates.push(i as u16);
            }
        }
    }
    set.skip_needle = compute_skip_needle(&table);
    set.first_byte_candidates = table;
    set.fallback_candidates = fallback_candidates;
    set.variable_distance_candidates = variable_distance_candidates;
    set.variable_distance_max = variable_distance_max;
    set.variable_distance_seen = vec![usize::MAX; set.entries.len()];
    set.has_dispatch_candidates = set
        .first_byte_candidates
        .iter()
        .any(|slot| !slot.is_empty());
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
        first_byte_candidates: Box::new(std::array::from_fn(|_| Vec::new())),
        fallback_candidates: Vec::new(),
        variable_distance_candidates: Box::new(std::array::from_fn(|_| Vec::new())),
        variable_distance_max: 0,
        variable_distance_seen: Vec::new(),
        has_dispatch_candidates: false,
        skip_needle: SkipNeedle::None,
        scratch_msa: None,
        last_match_len: ONIG_MISMATCH,
    });

    for reg in regs {
        let r = onig_regset_add(&mut set, reg);
        if r != ONIG_NORMAL {
            return (None, r);
        }
    }

    build_first_byte_table(&mut set);

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

    // Add the new entry to the first-byte dispatch table
    let new_idx = (set.entries.len() - 1) as u16;
    if !add_entry_to_first_byte_table(
        &mut set.first_byte_candidates,
        &set.entries[new_idx as usize].reg,
        new_idx,
    ) {
        if let Some(max) = add_variable_distance_candidate(
            &mut set.variable_distance_candidates,
            &set.entries[new_idx as usize].reg,
            new_idx,
        ) {
            set.variable_distance_max = set.variable_distance_max.max(max);
        } else {
            set.fallback_candidates.push(new_idx);
        }
    } else {
        set.has_dispatch_candidates = true;
        set.skip_needle = compute_skip_needle(&set.first_byte_candidates);
    }
    set.variable_distance_seen.push(usize::MAX);

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
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_replace(set: &mut OnigRegSet, at: usize, reg: Option<Box<RegexType>>) -> i32 {
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
                set.all_low_high = *optimize != OptimizeType::None && *dist_max != INFINITE_LEN;
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

    // Rebuild first-byte dispatch table from scratch
    build_first_byte_table(set);

    ONIG_NORMAL
}

/// Return the number of regexes in the set.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_number_of_regex(set: &OnigRegSet) -> i32 {
    set.entries.len() as i32
}

/// Get a reference to the regex at index `at`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_get_regex(set: &OnigRegSet, at: usize) -> Option<&RegexType> {
    set.entries.get(at).map(|e| e.reg.as_ref())
}

/// Get a reference to the region at index `at`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_get_region(set: &OnigRegSet, at: usize) -> Option<&OnigRegion> {
    set.entries.get(at).and_then(|e| e.region.as_ref())
}

/// Return the match length from the last successful position-lead search.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_last_match_len(set: &OnigRegSet) -> i32 {
    set.last_match_len
}

/// Whether an optimizer with a variable first-byte distance permits a match at
/// `start`. This is only a fallback match gate: unlike the dispatch table, it
/// checks every possible optimizer offset and therefore never treats the
/// optimizer byte as the match's first byte.
#[inline]
fn fallback_optimizer_allows_start(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
) -> bool {
    const MAX_FALLBACK_OPTIMIZER_SPAN: usize = 8;

    let (map, exact) = match reg.optimize {
        OptimizeType::Map => (Some(&reg.map), None),
        OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward
            if !reg.exact.is_empty() =>
        {
            (None, Some(reg.exact[0]))
        }
        _ => return true,
    };

    if reg.dist_min == INFINITE_LEN || reg.dist_max == INFINITE_LEN {
        return true;
    }
    let min = reg.dist_min as usize;
    let max = reg.dist_max as usize;
    if max < min || max - min > MAX_FALLBACK_OPTIMIZER_SPAN {
        return true;
    }

    let Some(first) = start.checked_add(min) else {
        return false;
    };
    let Some(last) = start.checked_add(max) else {
        return false;
    };
    if first >= end {
        return false;
    }

    let last = last.min(end - 1);
    (first..=last).any(|position| match map {
        Some(map) => map[str_data[position] as usize] != 0,
        None => str_data[position] == exact.expect("exact optimizer byte"),
    })
}

#[allow(clippy::too_many_arguments)]
fn find_fallback_match(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    limit: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
    msa: &mut MatchArg,
) -> (i32, i32) {
    let enc = set.enc;
    let prev_is_newline_check = set.anychar_inf;
    let mut s = start;

    while s < limit {
        let prev_is_newline = if prev_is_newline_check && s > 0 {
            str_data[s - 1] == b'\n'
        } else {
            true
        };
        let remaining = end - s;
        let mut match_index = ONIG_MISMATCH;

        // A variable-distance optimizer target at s + d can only produce a
        // match start at s when d is in the target's proven window. This
        // avoids testing every fallback regex at every input position.
        let max_distance = set
            .variable_distance_max
            .min(end.saturating_sub(s.saturating_add(1)));
        for distance in 0..=max_distance {
            let candidate_byte = str_data[s + distance] as usize;
            let candidate_count = set.variable_distance_candidates[candidate_byte].len();
            for candidate_pos in 0..candidate_count {
                let candidate = set.variable_distance_candidates[candidate_byte][candidate_pos];
                let index = candidate.index as usize;
                if distance < candidate.min as usize
                    || distance > candidate.max as usize
                    || set.variable_distance_seen[index] == s
                    || (match_index >= 0 && index >= match_index as usize)
                {
                    continue;
                }
                set.variable_distance_seen[index] = s;
                if (set.entries[index].reg.anchor & ANCR_ANYCHAR_INF) != 0 && !prev_is_newline {
                    continue;
                }
                if set.entries[index].reg.threshold_len > 0
                    && remaining < set.entries[index].reg.threshold_len as usize
                {
                    continue;
                }

                let r = match_regset_entry(
                    set,
                    index,
                    str_data,
                    end,
                    s,
                    start,
                    option,
                    skip_region_for_nomem,
                    msa,
                );
                if r >= 0 {
                    match_index = index as i32;
                } else if r != ONIG_MISMATCH {
                    return (r, 0);
                }
            }
        }

        for fallback_pos in 0..set.fallback_candidates.len() {
            let index = set.fallback_candidates[fallback_pos] as usize;
            if match_index >= 0 && index >= match_index as usize {
                continue;
            }
            if (set.entries[index].reg.anchor & ANCR_ANYCHAR_INF) != 0 && !prev_is_newline {
                continue;
            }
            if set.entries[index].reg.threshold_len > 0
                && remaining < set.entries[index].reg.threshold_len as usize
            {
                continue;
            }
            if !fallback_optimizer_allows_start(&set.entries[index].reg, str_data, end, s) {
                continue;
            }

            let r = match_regset_entry(
                set,
                index,
                str_data,
                end,
                s,
                start,
                option,
                skip_region_for_nomem,
                msa,
            );
            if r >= 0 {
                match_index = index as i32;
            } else if r != ONIG_MISMATCH {
                return (r, 0);
            }
        }

        if match_index >= 0 {
            return (match_index, s as i32);
        }

        s += enclen(enc, str_data, s);
    }

    (ONIG_MISMATCH, 0)
}

#[allow(clippy::too_many_arguments)]
fn match_regset_entry(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
    msa: &mut MatchArg,
) -> i32 {
    if skip_region_for_nomem && set.entries[index].reg.num_mem == 0 {
        msa.region = None;
        onig_match_with_msa_start(
            &set.entries[index].reg,
            str_data,
            end,
            position,
            search_start,
            option,
            msa,
        )
    } else {
        msa.region = set.entries[index].region.take();
        let r = onig_match_with_msa_start(
            &set.entries[index].reg,
            str_data,
            end,
            position,
            search_start,
            option,
            msa,
        );
        set.entries[index].region = msa.region.take();
        r
    }
}

/// Position-lead search: iterate positions, try each regex at each position.
fn regset_search_body_position_lead(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
) -> (i32, i32) {
    // rmatch_pos, regex_index
    let enc = set.enc;
    let mut s = start;

    // Reuse a single MatchArg across patterns and across regset calls.
    let mut msa = if let Some(msa) = set.scratch_msa.take() {
        msa
    } else {
        let first_reg = &*set.entries[0].reg;
        MatchArg::new(first_reg, option, None, start)
    };

    let prev_is_newline_check = set.anychar_inf;
    let mut result: (i32, i32) = (ONIG_MISMATCH, 0);
    set.last_match_len = ONIG_MISMATCH;

    if set.has_dispatch_candidates {
        'search: loop {
            if s >= range {
                break;
            }

            // SIMD-accelerated position skip: jump to the next dispatch byte.
            s = match set.skip_needle {
                SkipNeedle::None => s,
                SkipNeedle::One(b) => match memchr::memchr(b, &str_data[s..range]) {
                    Some(off) => s + off,
                    None => break,
                },
                SkipNeedle::Two(b1, b2) => match memchr::memchr2(b1, b2, &str_data[s..range]) {
                    Some(off) => s + off,
                    None => break,
                },
                SkipNeedle::Three(b1, b2, b3) => {
                    match memchr::memchr3(b1, b2, b3, &str_data[s..range]) {
                        Some(off) => s + off,
                        None => break,
                    }
                }
            };

            let prev_is_newline = if prev_is_newline_check && s > 0 {
                s > 0 && str_data[s - 1] == b'\n'
            } else {
                true
            };
            let remaining = end - s;

            let candidate_byte = str_data[s] as usize;
            let candidate_count = set.first_byte_candidates[candidate_byte].len();
            for candidate_pos in 0..candidate_count {
                let i = set.first_byte_candidates[candidate_byte][candidate_pos] as usize;
                if (set.entries[i].reg.anchor & ANCR_ANYCHAR_INF) != 0 && !prev_is_newline {
                    continue;
                }
                if set.entries[i].reg.threshold_len > 0
                    && remaining < set.entries[i].reg.threshold_len as usize
                {
                    continue;
                }

                let r = match_regset_entry(
                    set,
                    i,
                    str_data,
                    end,
                    s,
                    start,
                    option,
                    skip_region_for_nomem,
                    &mut msa,
                );
                if r >= 0 {
                    set.last_match_len = r;
                    result = (i as i32, s as i32);
                    break 'search;
                }
                if r != ONIG_MISMATCH {
                    result = (r, 0);
                    break 'search;
                }
            }

            s += enclen(enc, str_data, s);
        }
    }

    // Optimizer bytes with nonzero maximum distance are not first-byte data.
    // Check them only up to the dispatch result, preserving position and index
    // priority without turning them into candidates at every byte.
    let fallback_limit = if result.0 >= 0 {
        (result.1 as usize + 1).min(range)
    } else {
        range
    };
    let fallback_result = find_fallback_match(
        set,
        str_data,
        end,
        start,
        fallback_limit,
        option,
        skip_region_for_nomem,
        &mut msa,
    );
    if fallback_result.0 != ONIG_MISMATCH && fallback_result.0 < 0 {
        set.scratch_msa = Some(msa);
        return fallback_result;
    }

    if fallback_result.0 >= 0
        && (result.0 == ONIG_MISMATCH
            || fallback_result.1 < result.1
            || (fallback_result.1 == result.1 && fallback_result.0 < result.0))
    {
        let index = fallback_result.0 as usize;
        let r = match_regset_entry(
            set,
            index,
            str_data,
            end,
            fallback_result.1 as usize,
            start,
            option,
            skip_region_for_nomem,
            &mut msa,
        );
        if r >= 0 {
            set.last_match_len = r;
            result = fallback_result;
        } else if r != ONIG_MISMATCH {
            result = (r, 0);
        }
    }

    set.scratch_msa = Some(msa);
    result
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
        let (r, returned_region) = onig_search(
            &set.entries[i].reg,
            str_data,
            end,
            start,
            ep,
            region,
            option,
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

    (match_index, match_pos)
}

#[allow(clippy::too_many_arguments)]
fn onig_regset_search_impl(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    eager_region_reset: bool,
) -> (i32, i32) {
    let n = set.entries.len();
    if n == 0 {
        return (ONIG_MISMATCH, 0);
    }
    set.last_match_len = ONIG_MISMATCH;

    let end = end.min(str_data.len());
    let range = range.min(end);
    if start > end {
        return (ONIG_MISMATCH, 0);
    }

    // Forward search only
    if !str_data.is_empty() && range < start {
        return (ONIGERR_INVALID_ARGUMENT, 0);
    }

    if eager_region_reset {
        // Preserve classic regset behavior: all regions are reset on each call.
        for entry in &mut set.entries {
            if let Some(ref mut region) = entry.region {
                region.resize(entry.reg.num_mem + 1);
                region.clear();
            }
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
        regset_search_body_position_lead(
            set,
            str_data,
            end,
            cur_start,
            cur_range,
            option,
            !eager_region_reset,
        )
    } else {
        regset_search_body_regex_lead(set, str_data, end, cur_start, orig_range, lead, option)
    };

    if eager_region_reset {
        // Clear regions for non-matching regexes with FIND_NOT_EMPTY.
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
    }

    (result, match_pos)
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
    onig_regset_search_impl(set, str_data, end, start, range, lead, option, true)
}

/// Fast regset search for high-frequency callers (e.g. scanner tokenization).
///
/// Semantics of match index/position are identical to `onig_regset_search`.
/// Only the matched regex's region is guaranteed to be up-to-date; non-matching
/// regex regions may remain from previous calls.
pub fn onig_regset_search_fast(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
) -> (i32, i32) {
    onig_regset_search_impl(set, str_data, end, start, range, lead, option, false)
}

/// Search the set with per-regex match parameters.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_arguments)]
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

    let end = end.min(str_data.len());
    let range = range.min(end);
    if start > end {
        return (ONIG_MISMATCH, 0);
    }

    // Forward search only
    if !str_data.is_empty() && range < start {
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

        for (i, entry) in set.entries.iter_mut().take(n).enumerate() {
            let region = entry.region.take();
            let (r, returned_region) = onig_search_with_param(
                &entry.reg, str_data, end, start, ep, region, option, &mps[i],
            );
            entry.region = returned_region;

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
    regset_search_body_position_lead(set, str_data, end, start, range, option, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encodings::utf8::ONIG_ENCODING_UTF8;
    use crate::regcomp::onig_new;
    use crate::regsyntax::OnigSyntaxOniguruma;

    fn compile(pattern: &[u8]) -> Box<RegexType> {
        let reg = onig_new(
            pattern,
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
        );
        match reg {
            Ok(r) => Box::new(r),
            Err(e) => panic!(
                "failed to compile {:?}: error {}",
                std::str::from_utf8(pattern),
                e
            ),
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::RegexLead,
            ONIG_OPTION_NONE,
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::RegexLead,
            ONIG_OPTION_NONE,
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PriorityToRegexOrder,
            ONIG_OPTION_NONE,
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
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
            &mut set,
            input,
            0,
            0,
            0,
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, ONIG_MISMATCH);
    }

    #[test]
    fn regset_search_normalizes_out_of_range_endpoints() {
        let (set, r) = onig_regset_new(vec![compile(b"(?=b)")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();
        let input = b"abc";

        for (end, start, range, expected) in [
            (input.len(), 0, 100, (0, 1)),
            (100, 0, 100, (0, 1)),
            (input.len(), 100, 0, (ONIG_MISMATCH, 0)),
        ] {
            assert_eq!(
                onig_regset_search(
                    &mut set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ),
                expected,
                "end={end}, start={start}, range={range}"
            );
        }
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
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
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
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
