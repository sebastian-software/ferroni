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

#[derive(Clone, Copy)]
struct VariableOptimizerCandidate {
    slot: u16,
    index: u16,
    dist_min: usize,
    dist_max: usize,
}

/// Pre-computed memchr needle for SIMD-accelerated position skipping.
///
/// When the dispatch table has only 1–3 non-empty byte slots (and zero
/// always-candidate patterns), we can use `memchr` to jump directly to the
/// next position where at least one table-dispatched pattern could match.
#[derive(Clone, Copy)]
enum SkipNeedle {
    /// No skipping possible (always-candidate patterns exist, or >3 bytes).
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
    /// For each byte value 0..255, the list of entry indices whose first-byte
    /// pre-filter does not exclude that byte. Built at construction time.
    first_byte_candidates: Box<[Vec<u16>; 256]>,
    /// Entries whose finite-distance optimizer byte may occur after the match
    /// start. They must use optimizer-event dispatch; treating that byte as
    /// the first byte loses earlier matches.
    variable_distance_candidates: Vec<u16>,
    /// Reverse lookup from an optimizer byte to variable-distance entries.
    variable_optimizer_candidates: Box<[Vec<VariableOptimizerCandidate>; 256]>,
    max_variable_distance: usize,
    /// Reused per-search state for de-duplicating candidate start positions.
    variable_last_start_scratch: Vec<usize>,
    variable_matched_scratch: Vec<bool>,
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

#[inline]
fn has_variable_optimizer(reg: &RegexType) -> bool {
    reg.dist_max > 0
        && match reg.optimize {
            OptimizeType::Map => true,
            OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward => {
                !reg.exact.is_empty()
            }
            _ => false,
        }
}

#[inline]
fn has_finite_variable_optimizer(reg: &RegexType) -> bool {
    reg.dist_max != INFINITE_LEN && has_variable_optimizer(reg)
}

/// Derive a conservative byte map for the first consuming instruction from
/// compiled bytecode. Returning `None` is deliberate: any control-flow shape
/// we cannot prove safe stays on the optimizer-event fallback.
fn derive_start_byte_map(reg: &RegexType) -> Option<[u8; CHAR_MAP_SIZE]> {
    fn target(pc: usize, addr: RelAddrType, len: usize) -> Option<usize> {
        let target = (pc as i64).checked_add(addr as i64)?;
        (target >= 0 && (target as usize) < len).then_some(target as usize)
    }

    fn add_all(map: &mut [u8; CHAR_MAP_SIZE]) {
        map.fill(1);
    }

    fn add_exact(map: &mut [u8; CHAR_MAP_SIZE], byte: u8) {
        map[byte as usize] = 1;
    }

    fn add_bitset(map: &mut [u8; CHAR_MAP_SIZE], bitset: &BitSet, inverted: bool) {
        for (byte, value) in map.iter_mut().enumerate() {
            if bitset_at(bitset, byte) != inverted {
                *value = 1;
            }
        }
    }

    fn restored_assertion_end(reg: &RegexType, pc: usize, id: MemNumType) -> Option<usize> {
        reg.ops
            .iter()
            .enumerate()
            .skip(pc + 1)
            .find_map(|(at, op)| match op.payload {
                OperationPayload::CutToMark {
                    id: cut_id,
                    restore_pos: true,
                } if cut_id == id => Some(at + 1),
                _ => None,
            })
    }

    fn is_negative_assertion_push(reg: &RegexType, pc: usize, alt: usize) -> bool {
        if alt <= pc + 1
            || reg.ops.get(alt.wrapping_sub(1)).map(|op| op.opcode) != Some(OpCode::Fail)
        {
            return false;
        }

        let lookahead_id = reg.ops.get(pc + 1).and_then(|op| match op.payload {
            OperationPayload::Mark {
                id,
                save_pos: false,
            } => Some(id),
            _ => None,
        });
        let lookbehind_id = pc.checked_sub(1).and_then(|at| match reg.ops[at].payload {
            OperationPayload::Mark {
                id,
                save_pos: false,
            } => Some(id),
            _ => None,
        });

        let has_matching_pop = |id| {
            reg.ops[pc + 1..alt].iter().any(|op| {
                matches!(op.payload, OperationPayload::PopToMark { id: pop_id } if pop_id == id)
            })
        };
        if lookahead_id.is_some_and(has_matching_pop) {
            return true;
        }

        lookbehind_id.is_some_and(|id| {
            has_matching_pop(id)
                && reg.ops[pc + 1..alt]
                    .iter()
                    .any(|op| op.opcode == OpCode::StepBackStart)
        })
    }

    let mut map = [0; CHAR_MAP_SIZE];
    let mut pending = vec![0usize];
    let mut visited = vec![false; reg.ops.len()];
    let mut saw_consumer = false;

    while let Some(pc) = pending.pop() {
        if pc >= reg.ops.len() || visited[pc] {
            continue;
        }
        visited[pc] = true;
        let op = &reg.ops[pc];

        match op.opcode {
            OpCode::Str1 | OpCode::Str2 | OpCode::Str3 | OpCode::Str4 | OpCode::Str5 => {
                let OperationPayload::Exact { s } = &op.payload else {
                    return None;
                };
                add_exact(&mut map, s[0]);
                saw_consumer = true;
            }
            OpCode::StrN => {
                let OperationPayload::ExactN { s, .. } = &op.payload else {
                    return None;
                };
                add_exact(&mut map, *s.first()?);
                saw_consumer = true;
            }
            OpCode::StrMb2n1
            | OpCode::StrMb2n2
            | OpCode::StrMb2n3
            | OpCode::StrMb2n
            | OpCode::StrMb3n
            | OpCode::StrMbn => {
                let OperationPayload::ExactLenN { s, .. } = &op.payload else {
                    return None;
                };
                add_exact(&mut map, *s.first()?);
                saw_consumer = true;
            }
            OpCode::CClass | OpCode::CClassNot => {
                let OperationPayload::CClass { bsp, .. } = &op.payload else {
                    return None;
                };
                add_bitset(&mut map, bsp, op.opcode == OpCode::CClassNot);
                saw_consumer = true;
            }
            OpCode::CClassMb => {
                for value in &mut map[0x80..] {
                    *value = 1;
                }
                saw_consumer = true;
            }
            OpCode::CClassMbNot => {
                add_all(&mut map);
                saw_consumer = true;
            }
            OpCode::CClassMix | OpCode::CClassMixNot => {
                let OperationPayload::CClassMix { bsp, .. } = &op.payload else {
                    return None;
                };
                add_bitset(&mut map, bsp, op.opcode == OpCode::CClassMixNot);
                for value in &mut map[0x80..] {
                    *value = 1;
                }
                saw_consumer = true;
            }
            OpCode::Word | OpCode::NoWord | OpCode::AnyChar | OpCode::AnyCharMl => {
                add_all(&mut map);
                saw_consumer = true;
            }
            OpCode::WordAscii | OpCode::NoWordAscii => {
                let inverted = op.opcode == OpCode::NoWordAscii;
                for (byte, value) in map.iter_mut().enumerate() {
                    let is_word = (byte as u8).is_ascii_alphanumeric() || byte == b'_' as usize;
                    if is_word != inverted {
                        *value = 1;
                    }
                }
                saw_consumer = true;
            }
            OpCode::CClassStar => {
                let OperationPayload::CClass { bsp, .. } = &op.payload else {
                    return None;
                };
                add_bitset(&mut map, bsp, false);
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::CClassMixStar => {
                let OperationPayload::CClassMix { bsp, .. } = &op.payload else {
                    return None;
                };
                add_bitset(&mut map, bsp, false);
                for value in &mut map[0x80..] {
                    *value = 1;
                }
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::CClassMbStar => {
                for value in &mut map[0x80..] {
                    *value = 1;
                }
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::WordStar => {
                add_all(&mut map);
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::WordAsciiStar => {
                for (byte, value) in map.iter_mut().enumerate() {
                    if (byte as u8).is_ascii_alphanumeric() || byte == b'_' as usize {
                        *value = 1;
                    }
                }
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::CClassStarPeekNext => {
                let OperationPayload::CClassStarPeekNext { bsp, .. } = &op.payload else {
                    return None;
                };
                add_bitset(&mut map, bsp, false);
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::WordAsciiStarPeekNext => {
                for (byte, value) in map.iter_mut().enumerate() {
                    if (byte as u8).is_ascii_alphanumeric() || byte == b'_' as usize {
                        *value = 1;
                    }
                }
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::AltLiterals => {
                let OperationPayload::AltLiterals { trie_idx } = op.payload else {
                    return None;
                };
                let trie = reg.literal_tries.get(trie_idx as usize)?;
                for literal in trie.literals() {
                    let byte = *literal.first()?;
                    add_exact(&mut map, byte);
                    if trie.is_case_insensitive() && byte.is_ascii_alphabetic() {
                        add_exact(&mut map, byte.to_ascii_lowercase());
                        add_exact(&mut map, byte.to_ascii_uppercase());
                    }
                }
                saw_consumer = true;
            }
            OpCode::Jump => {
                let OperationPayload::Jump { addr } = op.payload else {
                    return None;
                };
                pending.push(target(pc, addr, reg.ops.len())?);
            }
            OpCode::Push | OpCode::PushSuper => {
                let OperationPayload::Push { addr } = op.payload else {
                    return None;
                };
                let alt = target(pc, addr, reg.ops.len())?;
                pending.push(alt);
                if !is_negative_assertion_push(reg, pc, alt) {
                    pending.push(pc + 1);
                }
            }
            OpCode::PushOrJumpExact1 => {
                let OperationPayload::PushOrJumpExact1 { addr, .. } = op.payload else {
                    return None;
                };
                pending.push(pc + 1);
                pending.push(target(pc, addr, reg.ops.len())?);
            }
            OpCode::PushIfPeekNext => {
                let OperationPayload::PushIfPeekNext { addr, .. } = op.payload else {
                    return None;
                };
                pending.push(pc + 1);
                pending.push(target(pc, addr, reg.ops.len())?);
            }
            OpCode::Repeat | OpCode::RepeatNg => {
                let OperationPayload::Repeat { id, addr } = op.payload else {
                    return None;
                };
                let repeat = reg.repeat_range.get(id as usize)?;
                pending.push(pc + 1);
                if repeat.lower == 0 {
                    pending.push(target(pc, addr, reg.ops.len())?);
                }
            }
            OpCode::Mark => {
                let OperationPayload::Mark { id, save_pos } = op.payload else {
                    return None;
                };
                if save_pos {
                    // Positive lookahead/lookbehind restores the original input
                    // position at its matching CutToMark. Its body therefore
                    // cannot determine the byte at the match start.
                    pending.push(restored_assertion_end(reg, pc, id)?);
                } else {
                    pending.push(pc + 1);
                }
            }
            OpCode::StepBackStart | OpCode::StepBackNext => {
                // A negative lookbehind's successful continuation is already
                // represented by its surrounding Push target. Stop the body
                // path here so bytes before the match start never enter the map.
            }
            OpCode::MemStart
            | OpCode::MemStartPush
            | OpCode::MemEnd
            | OpCode::MemEndPush
            | OpCode::MemEndRec
            | OpCode::MemEndPushRec
            | OpCode::WordBoundary
            | OpCode::NoWordBoundary
            | OpCode::WordBegin
            | OpCode::WordEnd
            | OpCode::TextSegmentBoundary
            | OpCode::BeginBuf
            | OpCode::EndBuf
            | OpCode::BeginLine
            | OpCode::EndLine
            | OpCode::SemiEndBuf
            | OpCode::CheckPosition
            | OpCode::BackRefCheck
            | OpCode::BackRefCheckWithLevel
            | OpCode::EmptyCheckStart
            | OpCode::EmptyCheckEnd
            | OpCode::EmptyCheckEndMemst
            | OpCode::EmptyCheckEndMemstPush
            | OpCode::Pop
            | OpCode::PopToMark
            | OpCode::CutToMark
            | OpCode::SaveVal
            | OpCode::UpdateVar
            | OpCode::CalloutContents
            | OpCode::CalloutName => pending.push(pc + 1),
            OpCode::Fail => {}
            OpCode::AnyCharStar
            | OpCode::AnyCharMlStar
            | OpCode::AnyCharStarPeekNext
            | OpCode::AnyCharMlStarPeekNext => {
                add_all(&mut map);
                saw_consumer = true;
            }
            OpCode::Finish | OpCode::End => return None,
            _ => return None,
        }
    }

    saw_consumer.then_some(map)
}

fn add_entry_by_start_map(
    table: &mut [Vec<u16>; CHAR_MAP_SIZE],
    start_map: &[u8; CHAR_MAP_SIZE],
    idx: u16,
) {
    for (byte, entries) in table.iter_mut().enumerate() {
        if start_map[byte] != 0 {
            entries.push(idx);
        }
    }
}

/// Add a single non-variable entry to the appropriate first-byte slots.
fn add_entry_to_first_byte_table(table: &mut [Vec<u16>; 256], reg: &RegexType, idx: u16) {
    if reg.optimize == OptimizeType::Map && reg.dist_min == 0 {
        // Map-filterable: only bytes where map[b] != 0
        for (b, slot) in table.iter_mut().enumerate() {
            if reg.map[b] != 0 {
                slot.push(idx);
            }
        }
    } else if reg.dist_min == 0 && !reg.exact.is_empty() {
        // Exact-filterable: only the first byte of the exact string
        table[reg.exact[0] as usize].push(idx);
    } else if reg.has_first_byte_map {
        // Fallback: use the first-byte prefilter map.
        for (b, slot) in table.iter_mut().enumerate() {
            if reg.first_byte_map[b] != 0 {
                slot.push(idx);
            }
        }
    } else {
        // Always-candidate: appears in all 256 slots.
        for slot in table.iter_mut() {
            slot.push(idx);
        }
    }
}

fn add_variable_optimizer_candidate(
    table: &mut [Vec<VariableOptimizerCandidate>; 256],
    reg: &RegexType,
    index: u16,
    slot: u16,
) {
    let candidate = VariableOptimizerCandidate {
        slot,
        index,
        dist_min: reg.dist_min as usize,
        dist_max: reg.dist_max as usize,
    };

    if reg.optimize == OptimizeType::Map {
        for (byte, entries) in table.iter_mut().enumerate() {
            if reg.map[byte] != 0 {
                entries.push(candidate);
            }
        }
    } else {
        table[reg.exact[0] as usize].push(candidate);
    }
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
    let mut variable_distance_candidates = Vec::new();
    let mut variable_optimizer_candidates: Box<[Vec<VariableOptimizerCandidate>; 256]> =
        Box::new(std::array::from_fn(|_| Vec::new()));
    let mut max_variable_distance = 0;
    for (i, entry) in set.entries.iter().enumerate() {
        if has_variable_optimizer(&entry.reg) {
            if let Some(start_map) = derive_start_byte_map(&entry.reg) {
                add_entry_by_start_map(&mut table, &start_map, i as u16);
                continue;
            }
            {
                let slot = variable_distance_candidates.len() as u16;
                variable_distance_candidates.push(i as u16);
                add_variable_optimizer_candidate(
                    &mut variable_optimizer_candidates,
                    &entry.reg,
                    i as u16,
                    slot,
                );
                if entry.reg.dist_max == INFINITE_LEN {
                    max_variable_distance = usize::MAX;
                } else {
                    max_variable_distance = max_variable_distance.max(entry.reg.dist_max as usize);
                }
            }
        } else {
            add_entry_to_first_byte_table(&mut table, &entry.reg, i as u16);
        }
    }
    set.skip_needle = compute_skip_needle(&table);
    set.first_byte_candidates = table;
    set.variable_distance_candidates = variable_distance_candidates;
    set.variable_optimizer_candidates = variable_optimizer_candidates;
    set.max_variable_distance = max_variable_distance;
    set.variable_last_start_scratch
        .resize(set.variable_distance_candidates.len(), 0);
    set.variable_matched_scratch
        .resize(set.variable_distance_candidates.len(), false);
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
        variable_distance_candidates: Vec::new(),
        variable_optimizer_candidates: Box::new(std::array::from_fn(|_| Vec::new())),
        max_variable_distance: 0,
        variable_last_start_scratch: Vec::new(),
        variable_matched_scratch: Vec::new(),
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
    if has_variable_optimizer(&set.entries[new_idx as usize].reg) {
        if let Some(start_map) = derive_start_byte_map(&set.entries[new_idx as usize].reg) {
            add_entry_by_start_map(&mut set.first_byte_candidates, &start_map, new_idx);
            set.skip_needle = compute_skip_needle(&set.first_byte_candidates);
        } else {
            let slot = set.variable_distance_candidates.len() as u16;
            set.variable_distance_candidates.push(new_idx);
            add_variable_optimizer_candidate(
                &mut set.variable_optimizer_candidates,
                &set.entries[new_idx as usize].reg,
                new_idx,
                slot,
            );
            if set.entries[new_idx as usize].reg.dist_max == INFINITE_LEN {
                set.max_variable_distance = usize::MAX;
            } else {
                set.max_variable_distance = set
                    .max_variable_distance
                    .max(set.entries[new_idx as usize].reg.dist_max as usize);
            }
            set.variable_last_start_scratch.push(0);
            set.variable_matched_scratch.push(false);
        }
    } else {
        add_entry_to_first_byte_table(
            &mut set.first_byte_candidates,
            &set.entries[new_idx as usize].reg,
            new_idx,
        );
        set.skip_needle = compute_skip_needle(&set.first_byte_candidates);
    }

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

#[derive(Clone, Copy)]
struct RegSetWinner {
    index: i32,
    position: i32,
    match_len: i32,
}

#[inline]
fn winner_is_better(candidate: RegSetWinner, current: Option<RegSetWinner>) -> bool {
    match current {
        None => true,
        Some(current) => {
            candidate.position < current.position
                || (candidate.position == current.position && candidate.index < current.index)
        }
    }
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
        let result = onig_match_with_msa_start(
            &set.entries[index].reg,
            str_data,
            end,
            position,
            search_start,
            option,
            msa,
        );
        set.entries[index].region = msa.region.take();
        result
    }
}

fn clear_regset_entry_region(set: &mut OnigRegSet, index: i32) {
    if let Some(region) = set.entries[index as usize].region.as_mut() {
        region.clear();
    }
}

#[inline]
fn optimizer_target_exists(reg: &RegexType, str_data: &[u8], start: usize, end: usize) -> bool {
    let haystack = &str_data[start..end];
    match reg.optimize {
        OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward => {
            memchr::memmem::find(haystack, &reg.exact).is_some()
        }
        OptimizeType::Map => match reg.map_byte_count {
            1 => memchr::memchr(reg.map_bytes[0], haystack).is_some(),
            2 => memchr::memchr2(reg.map_bytes[0], reg.map_bytes[1], haystack).is_some(),
            3 => memchr::memchr3(
                reg.map_bytes[0],
                reg.map_bytes[1],
                reg.map_bytes[2],
                haystack,
            )
            .is_some(),
            _ => haystack.iter().any(|&byte| reg.map[byte as usize] != 0),
        },
        OptimizeType::None => true,
    }
}

/// Position-lead search: iterate positions, try each regex at each position.
fn regset_search_body_position_lead_table(
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

    'search: loop {
        if s > range {
            break;
        }

        // SIMD-accelerated position skip: jump to next byte that could match.
        // The range position itself must still be attempted (matching
        // Oniguruma's do-while loop), so a failed skip lands on `range`.
        if s < range {
            s = match set.skip_needle {
                SkipNeedle::None => s,
                SkipNeedle::One(b) => {
                    memchr::memchr(b, &str_data[s..range]).map_or(range, |off| s + off)
                }
                SkipNeedle::Two(b1, b2) => {
                    memchr::memchr2(b1, b2, &str_data[s..range]).map_or(range, |off| s + off)
                }
                SkipNeedle::Three(b1, b2, b3) => {
                    memchr::memchr3(b1, b2, b3, &str_data[s..range]).map_or(range, |off| s + off)
                }
            };
        }

        let prev_is_newline = if prev_is_newline_check && s > 0 {
            // Check if previous character is newline
            s > 0 && str_data[s - 1] == b'\n'
        } else {
            true // default: allow matching
        };

        let remaining = end - s;

        // At the logical end there is no first byte to dispatch on. Try all
        // entries there; the threshold check cheaply rejects non-empty ones.
        let at_end = s == end;
        let candidate_count = if at_end {
            set.entries.len()
        } else {
            set.first_byte_candidates[str_data[s] as usize].len()
        };

        for candidate_at in 0..candidate_count {
            let i = if at_end {
                candidate_at
            } else {
                set.first_byte_candidates[str_data[s] as usize][candidate_at] as usize
            };

            // ANCR_ANYCHAR_INF optimization: skip if previous char is not newline
            if (set.entries[i].reg.anchor & ANCR_ANYCHAR_INF) != 0 && !prev_is_newline {
                continue;
            }

            // Pre-filter: remaining text too short for this pattern
            if set.entries[i].reg.threshold_len > 0
                && remaining < set.entries[i].reg.threshold_len as usize
            {
                continue;
            }
            if has_variable_optimizer(&set.entries[i].reg)
                && !optimizer_target_exists(&set.entries[i].reg, str_data, s, end)
            {
                continue;
            }

            let r = if skip_region_for_nomem && set.entries[i].reg.num_mem == 0 {
                // No capture groups: scanner only needs full-match length, so avoid
                // region take/clear/restore on this hot path.
                msa.region = None;
                onig_match_with_msa_start(
                    &set.entries[i].reg,
                    str_data,
                    end,
                    s,
                    start,
                    option,
                    &mut msa,
                )
            } else {
                // Swap region into msa for this match, then swap back.
                msa.region = set.entries[i].region.take();
                let r = onig_match_with_msa_start(
                    &set.entries[i].reg,
                    str_data,
                    end,
                    s,
                    start,
                    option,
                    &mut msa,
                );
                set.entries[i].region = msa.region.take();
                r
            };

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

        if s >= range {
            break;
        }
        s += enclen(enc, str_data, s);
    }

    set.scratch_msa = Some(msa);

    result
}

/// Run the established table scan first, then check only variable-distance
/// optimizer events that can still beat its winner. This keeps the common
/// position-lead path byte-for-byte hot while preserving correct earlier
/// starts for delayed optimizer bytes.
fn regset_search_body_position_lead(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
) -> (i32, i32) {
    if set.variable_distance_candidates.is_empty() {
        return regset_search_body_position_lead_table(
            set,
            str_data,
            end,
            start,
            range,
            option,
            skip_region_for_nomem,
        );
    }

    let fixed_result = regset_search_body_position_lead_table(
        set,
        str_data,
        end,
        start,
        range,
        option,
        skip_region_for_nomem,
    );
    if fixed_result.0 < 0 && fixed_result.0 != ONIG_MISMATCH {
        return fixed_result;
    }

    let fixed_match_len = set.last_match_len;
    let mut winner = if fixed_result.0 >= 0 {
        Some(RegSetWinner {
            index: fixed_result.0,
            position: fixed_result.1,
            match_len: fixed_match_len,
        })
    } else {
        None
    };

    set.variable_last_start_scratch.fill(0);
    set.variable_matched_scratch.fill(false);
    let mut msa = set
        .scratch_msa
        .take()
        .unwrap_or_else(|| MatchArg::new(&set.entries[0].reg, option, None, start));
    let last_relevant_start = winner.map_or(range, |current| current.position as usize);
    let event_limit = if end == 0 {
        0
    } else {
        last_relevant_start
            .saturating_add(set.max_variable_distance)
            .min(end - 1)
    };

    let mut cursor = start;
    while cursor <= event_limit {
        let event_count = set.variable_optimizer_candidates[str_data[cursor] as usize].len();
        for event_at in 0..event_count {
            let event = set.variable_optimizer_candidates[str_data[cursor] as usize][event_at];
            let slot = event.slot as usize;
            if set.variable_matched_scratch[slot] || cursor < event.dist_min {
                continue;
            }
            if let Some(current) = winner {
                if current.position == start as i32 && event.index as i32 >= current.index {
                    continue;
                }
            }

            let lower = start.max(cursor.saturating_sub(event.dist_max));
            let upper = range.min(cursor - event.dist_min);
            if lower > upper {
                continue;
            }

            let mut candidate_start = lower.max(set.variable_last_start_scratch[slot]);
            while candidate_start <= upper {
                set.variable_last_start_scratch[slot] = candidate_start.saturating_add(1);
                if let Some(current) = winner {
                    if candidate_start > current.position as usize
                        || (candidate_start == current.position as usize
                            && event.index as i32 >= current.index)
                    {
                        candidate_start += 1;
                        continue;
                    }
                }
                if set
                    .enc
                    .left_adjust_char_head(start, candidate_start, str_data)
                    != candidate_start
                {
                    candidate_start += 1;
                    continue;
                }

                let index = event.index as usize;
                let prev_is_newline = if set.anychar_inf && candidate_start > 0 {
                    str_data[candidate_start - 1] == b'\n'
                } else {
                    true
                };
                if (set.entries[index].reg.anchor & ANCR_ANYCHAR_INF) != 0 && !prev_is_newline {
                    candidate_start += 1;
                    continue;
                }
                if set.entries[index].reg.threshold_len > 0
                    && end - candidate_start < set.entries[index].reg.threshold_len as usize
                {
                    candidate_start += 1;
                    continue;
                }

                let result = match_regset_entry(
                    set,
                    index,
                    str_data,
                    end,
                    candidate_start,
                    start,
                    option,
                    skip_region_for_nomem,
                    &mut msa,
                );
                if result >= 0 {
                    set.variable_matched_scratch[slot] = true;
                    let candidate = RegSetWinner {
                        index: index as i32,
                        position: candidate_start as i32,
                        match_len: result,
                    };
                    if winner_is_better(candidate, winner) {
                        if let Some(current) = winner {
                            clear_regset_entry_region(set, current.index);
                        }
                        winner = Some(candidate);
                    } else {
                        clear_regset_entry_region(set, candidate.index);
                    }
                    break;
                }
                if result != ONIG_MISMATCH {
                    if let Some(current) = winner {
                        clear_regset_entry_region(set, current.index);
                    }
                    set.scratch_msa = Some(msa);
                    return (result, 0);
                }
                candidate_start += 1;
            }
        }

        if let Some(current) = winner {
            if cursor >= (current.position as usize).saturating_add(set.max_variable_distance) {
                break;
            }
        }
        if cursor == event_limit {
            break;
        }
        cursor += 1;
    }

    set.scratch_msa = Some(msa);
    if let Some(winner) = winner {
        set.last_match_len = winner.match_len;
        (winner.index, winner.position)
    } else {
        set.last_match_len = ONIG_MISMATCH;
        (ONIG_MISMATCH, 0)
    }
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

    // Empty logical string handling. A non-empty string searched from its end
    // must continue through the lead-specific path, matching upstream.
    if end == 0 {
        for i in 0..n {
            if set.entries[i].reg.threshold_len == 0 {
                let region = set.entries[i].region.take();
                let (r, returned_region) =
                    onig_match(&set.entries[i].reg, str_data, end, start, region, option);
                set.entries[i].region = returned_region;
                if r >= 0 {
                    set.last_match_len = r;
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

    // Resize and clear all regions
    for entry in &mut set.entries {
        if let Some(ref mut region) = entry.region {
            region.resize(entry.reg.num_mem + 1);
            region.clear();
        }
    }

    // Empty logical string handling. A non-empty string searched from its end
    // must continue through the lead-specific path, matching upstream.
    if end == 0 {
        for i in 0..n {
            if set.entries[i].reg.threshold_len == 0 {
                let region = set.entries[i].region.take();
                let (r, returned_region) =
                    onig_match(&set.entries[i].reg, str_data, end, start, region, option);
                set.entries[i].region = returned_region;
                if r >= 0 {
                    set.last_match_len = r;
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
    fn start_byte_map_includes_optional_prefix_and_required_byte() {
        let reg = compile(b"a?[bcd]");
        assert!(has_finite_variable_optimizer(&reg));

        let map = derive_start_byte_map(&reg).expect("simple optional prefix is analyzable");
        for byte in [b'a', b'b', b'c', b'd'] {
            assert_ne!(map[byte as usize], 0, "missing byte {byte:?}");
        }
        assert_eq!(map[b'x' as usize], 0);
    }

    #[test]
    fn start_byte_map_handles_compiled_consumer_and_control_flow_shapes() {
        fn str1(byte: u8) -> Operation {
            let mut exact = [0; 16];
            exact[0] = byte;
            Operation {
                opcode: OpCode::Str1,
                payload: OperationPayload::Exact { s: exact },
            }
        }

        fn map_for(ops: Vec<Operation>, repeat_range: Vec<RepeatRange>) -> [u8; CHAR_MAP_SIZE] {
            let mut reg = compile(b"a");
            reg.ops = ops;
            reg.repeat_range = repeat_range;
            derive_start_byte_map(&reg).expect("analyzable bytecode")
        }

        let consumer_cases = [
            Operation {
                opcode: OpCode::StrN,
                payload: OperationPayload::ExactN {
                    s: b"long literal".to_vec(),
                    n: 12,
                },
            },
            Operation {
                opcode: OpCode::StrMb2n1,
                payload: OperationPayload::ExactLenN {
                    s: "é".as_bytes().to_vec(),
                    n: 2,
                    len: 1,
                },
            },
            Operation {
                opcode: OpCode::CClassMb,
                payload: OperationPayload::CClassMb { mb: Vec::new() },
            },
            Operation {
                opcode: OpCode::CClassMix,
                payload: OperationPayload::CClassMix {
                    mb: Vec::new(),
                    bsp: Box::new([0; BITSET_REAL_SIZE]),
                },
            },
            Operation {
                opcode: OpCode::Word,
                payload: OperationPayload::None,
            },
            Operation {
                opcode: OpCode::WordAscii,
                payload: OperationPayload::None,
            },
            Operation {
                opcode: OpCode::AnyCharStar,
                payload: OperationPayload::None,
            },
        ];
        for operation in consumer_cases {
            assert!(map_for(vec![operation], Vec::new())
                .iter()
                .any(|&value| value != 0));
        }

        for opcode in [
            OpCode::Push,
            OpCode::PushOrJumpExact1,
            OpCode::PushIfPeekNext,
        ] {
            let payload = match opcode {
                OpCode::Push => OperationPayload::Push { addr: 2 },
                OpCode::PushOrJumpExact1 => OperationPayload::PushOrJumpExact1 { addr: 2, c: b'a' },
                _ => OperationPayload::PushIfPeekNext { addr: 2, c: b'a' },
            };
            let map = map_for(
                vec![Operation { opcode, payload }, str1(b'a'), str1(b'b')],
                Vec::new(),
            );
            assert_ne!(map[b'a' as usize], 0);
            assert_ne!(map[b'b' as usize], 0);
        }

        let jump_map = map_for(
            vec![
                Operation {
                    opcode: OpCode::Jump,
                    payload: OperationPayload::Jump { addr: 1 },
                },
                str1(b'j'),
            ],
            Vec::new(),
        );
        assert_ne!(jump_map[b'j' as usize], 0);

        let repeat_map = map_for(
            vec![
                Operation {
                    opcode: OpCode::Repeat,
                    payload: OperationPayload::Repeat { id: 0, addr: 2 },
                },
                str1(b'r'),
                str1(b's'),
            ],
            vec![RepeatRange {
                lower: 0,
                upper: 1,
                u_offset: 0,
            }],
        );
        assert_ne!(repeat_map[b'r' as usize], 0);
        assert_ne!(repeat_map[b's' as usize], 0);
    }

    #[test]
    fn lookbehind_start_map_ignores_bytes_before_the_match_start() {
        let reg = compile(b"(?<=x)a?bc");
        assert!(has_finite_variable_optimizer(&reg));
        let map = derive_start_byte_map(&reg).expect("lookbehind is analyzable");
        assert_ne!(map[b'a' as usize], 0);
        assert_ne!(map[b'b' as usize], 0);
        assert_eq!(map[b'x' as usize], 0);

        let (set, result) = onig_regset_new(vec![reg]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(set.variable_distance_candidates.is_empty());

        let input = b"xabc";
        let (index, position) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!((index, position), (0, 1));
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
    fn regset_fast_empty_string_records_zero_match_length() {
        let (set, r) = onig_regset_new(vec![compile(b"$")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        assert_eq!(
            onig_regset_search_fast(
                &mut set,
                b"",
                0,
                0,
                0,
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 0)
        );
        assert_eq!(onig_regset_last_match_len(&set), 0);
    }

    #[test]
    fn regset_position_lead_attempts_the_range_position() {
        let (set, r) = onig_regset_new(vec![compile(b"(?=b)")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        assert_eq!(
            onig_regset_search(
                &mut set,
                b"abc",
                3,
                0,
                1,
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 1)
        );
    }

    #[test]
    fn regset_position_lead_finds_zero_width_matches_at_end() {
        for pattern in [b"$".as_slice(), b"\\z".as_slice(), b"a*".as_slice()] {
            let (set, r) = onig_regset_new(vec![compile(pattern)]);
            assert_eq!(r, ONIG_NORMAL);
            let mut set = set.unwrap();

            assert_eq!(
                onig_regset_search(
                    &mut set,
                    b"abc",
                    3,
                    3,
                    3,
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ),
                (0, 3),
                "pattern {:?}",
                std::str::from_utf8(pattern).unwrap()
            );
        }
    }

    #[test]
    fn regset_nonempty_eos_keeps_regex_lead_semantics() {
        for lead in [
            OnigRegSetLead::RegexLead,
            OnigRegSetLead::PriorityToRegexOrder,
        ] {
            let (set, r) = onig_regset_new(vec![compile(b"a*")]);
            assert_eq!(r, ONIG_NORMAL);
            let mut set = set.unwrap();

            assert_eq!(
                onig_regset_search(&mut set, b"\na", 2, 2, 2, lead, ONIG_OPTION_NONE),
                (ONIG_MISMATCH, 0),
                "lead {lead:?}"
            );
        }
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
