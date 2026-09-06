// regset.rs - Port of USE_REGSET section from regexec.c
// Multi-regex search for syntax highlighters and text editors.

use crate::oniguruma::*;
use crate::regenc::{
    OnigEncoding, onigenc_get_prev_char_head, onigenc_is_ascii_compatible_encoding,
};
use crate::regexec::{
    MatchArg, OnigMatchParam, onig_get_global_limit_revision, onig_get_match_stack_limit,
    onig_get_retry_limit_in_match, onig_get_retry_limit_in_search, onig_get_time_limit, onig_match,
    onig_match_with_msa_start, onig_search, onig_search_with_msa_and_right_range,
    onig_search_with_param,
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
    /// Caching a fallback search must not suppress observable callouts or
    /// position-sensitive bytecode such as partial `\G` anchors.
    fallback_memo_safe: bool,
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
    /// Number of entries routed through `first_byte_candidates`. A pure
    /// fallback set has no table work at any position.
    table_entry_count: usize,
    /// Entries whose start byte cannot be derived safely from bytecode. They
    /// are searched independently with their own optimizer after the table
    /// pass, rather than routing on an optimizer byte that can occur later
    /// than the true match start.
    fallback_search_candidates: Vec<u16>,
    /// Scanner-only memoization for optimizer-backed fallback searches. The
    /// caller supplies a stable immutable string identity, so a no-match or
    /// a later match can be reused as tokenization advances.
    fallback_memo_key: Option<FallbackMemoKey>,
    fallback_memos: Vec<Vec<FallbackMemo>>,
    /// Global limits captured by `scratch_msa`. Identity changes invalidate
    /// cached results, but do not require discarding its reusable buffers.
    scratch_limits: Option<FallbackMemoLimits>,
    /// Revision of the process-global limits captured in `scratch_limits`.
    /// The revision check is intentionally cheaper than reloading all limits
    /// for table-only scanner calls.
    scratch_limits_revision: Option<u64>,
    /// Cumulative search retry budgets for table-routed entries. Oniguruma
    /// accumulates this budget across positions per regex, not across
    /// different regexes at the same position.
    scratch_table_retry_counters: Vec<u64>,
    /// SIMD-accelerated skip needle derived from the dispatch table.
    skip_needle: SkipNeedle,
    /// Reused MatchArg scratch space for position-lead searches.
    scratch_msa: Option<MatchArg>,
    /// Match length from the last successful position-lead search.
    last_match_len: i32,
}

#[derive(Clone, Copy)]
enum FallbackMemo {
    /// A direct position-lead attempt failed at this exact start. It is safe
    /// to skip only an identical retry; a different start upgrades to an
    /// optimizer search so mixed table/fallback scans remain linear.
    ExactStartMiss(usize),
    NoMatchFrom(usize),
    MatchAt {
        searched_from: usize,
        position: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FallbackMemoKey {
    identity: FallbackMemoIdentity,
    end: usize,
    option: OnigOptionType,
    retry_limit_in_match: u64,
    retry_limit_in_search: u64,
    match_stack_limit: u32,
    time_limit: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FallbackMemoLimits {
    retry_limit_in_match: u64,
    retry_limit_in_search: u64,
    match_stack_limit: u32,
    time_limit: u64,
}

impl FallbackMemoLimits {
    fn current() -> Self {
        Self {
            retry_limit_in_match: onig_get_retry_limit_in_match(),
            retry_limit_in_search: onig_get_retry_limit_in_search(),
            match_stack_limit: onig_get_match_stack_limit(),
            time_limit: onig_get_time_limit(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FallbackMemoIdentity {
    Caller(u64),
    OnigString(u64),
}

const FALLBACK_MEMO_CAPACITY: usize = 8;

#[inline]
fn fallback_memo_is_safe(reg: &RegexType) -> bool {
    reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0)
        && !reg.ops.iter().any(|op| op.opcode == OpCode::CheckPosition)
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

    fn mark_continuation(reg: &RegexType, pc: usize, id: MemNumType) -> Option<usize> {
        reg.ops
            .iter()
            .enumerate()
            .skip(pc + 1)
            .find_map(|(at, op)| match op.payload {
                OperationPayload::CutToMark {
                    id: cut_id,
                    restore_pos,
                } if cut_id == id => Some(if restore_pos { at + 1 } else { pc + 1 }),
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
                    // position at its matching CutToMark. Non-restoring marks
                    // are VM bookkeeping (for example greedy star loops) and
                    // continue normally into their consuming instruction.
                    pending.push(mark_continuation(reg, pc, id)?);
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
    let mut fallback_search_candidates = Vec::new();
    let mut table_entry_count = 0;
    for (i, entry) in set.entries.iter().enumerate() {
        if has_variable_optimizer(&entry.reg) {
            // A start-byte map proves semantic routing, but an unbounded
            // prefix could still re-run its VM at every matching byte. Keep
            // those entries on their optimizer-backed fallback; only a
            // finite prefix has a bounded table-dispatch cost.
            if entry.reg.dist_max != INFINITE_LEN {
                if let Some(start_map) = derive_start_byte_map(&entry.reg) {
                    add_entry_by_start_map(&mut table, &start_map, i as u16);
                    table_entry_count += 1;
                    continue;
                }
            }
            fallback_search_candidates.push(i as u16);
        } else {
            add_entry_to_first_byte_table(&mut table, &entry.reg, i as u16);
            table_entry_count += 1;
        }
    }
    set.skip_needle = compute_skip_needle(&table);
    set.first_byte_candidates = table;
    set.table_entry_count = table_entry_count;
    set.fallback_search_candidates = fallback_search_candidates;
    set.fallback_memo_key = None;
    set.fallback_memos = vec![Vec::new(); set.entries.len()];
    set.scratch_limits = None;
    set.scratch_limits_revision = None;
    set.scratch_table_retry_counters = vec![0; set.entries.len()];
    set.scratch_msa = None;
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
        table_entry_count: 0,
        fallback_search_candidates: Vec::new(),
        fallback_memo_key: None,
        fallback_memos: Vec::new(),
        scratch_limits: None,
        scratch_limits_revision: None,
        scratch_table_retry_counters: Vec::new(),
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
    let fallback_memo_safe = fallback_memo_is_safe(&reg);
    set.entries.push(RegSetEntry {
        reg,
        region,
        fallback_memo_safe,
    });
    set.fallback_memo_key = None;
    set.fallback_memos.resize_with(set.entries.len(), Vec::new);
    set.scratch_limits = None;
    set.scratch_limits_revision = None;
    set.scratch_table_retry_counters
        .resize(set.entries.len(), 0);
    set.scratch_table_retry_counters.fill(0);
    set.scratch_msa = None;

    // Add the new entry to the first-byte dispatch table
    let new_idx = (set.entries.len() - 1) as u16;
    if has_variable_optimizer(&set.entries[new_idx as usize].reg) {
        let start_map = (set.entries[new_idx as usize].reg.dist_max != INFINITE_LEN)
            .then(|| derive_start_byte_map(&set.entries[new_idx as usize].reg))
            .flatten();
        if let Some(start_map) = start_map {
            add_entry_by_start_map(&mut set.first_byte_candidates, &start_map, new_idx);
            set.table_entry_count += 1;
            set.skip_needle = compute_skip_needle(&set.first_byte_candidates);
        } else {
            set.fallback_search_candidates.push(new_idx);
        }
    } else {
        add_entry_to_first_byte_table(
            &mut set.first_byte_candidates,
            &set.entries[new_idx as usize].reg,
            new_idx,
        );
        set.table_entry_count += 1;
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
            set.entries[at].fallback_memo_safe = fallback_memo_is_safe(&reg);
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

#[derive(Clone, Copy)]
struct RegSetError {
    code: i32,
    index: i32,
    position: i32,
}

#[derive(Clone, Copy)]
enum RegSetDecision {
    Match(RegSetWinner),
    Error(RegSetError),
}

#[inline]
fn decision_position_and_index(decision: RegSetDecision) -> (i32, i32) {
    match decision {
        RegSetDecision::Match(candidate) => (candidate.position, candidate.index),
        RegSetDecision::Error(error) => (error.position, error.index),
    }
}

#[inline]
fn decision_is_better(candidate: RegSetDecision, current: Option<RegSetDecision>) -> bool {
    current.is_none_or(|current| {
        decision_position_and_index(candidate) < decision_position_and_index(current)
    })
}

fn clear_regset_entry_region(set: &mut OnigRegSet, index: i32) {
    if let Some(region) = set.entries[index as usize].region.as_mut() {
        region.clear();
    }
}

fn record_regset_decision(
    set: &mut OnigRegSet,
    current: &mut Option<RegSetDecision>,
    candidate: RegSetDecision,
) {
    if decision_is_better(candidate, *current) {
        if let Some(RegSetDecision::Match(previous)) = current {
            clear_regset_entry_region(set, previous.index);
        }
        *current = Some(candidate);
    } else if let RegSetDecision::Match(candidate) = candidate {
        clear_regset_entry_region(set, candidate.index);
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

#[allow(clippy::too_many_arguments)]
fn locate_regset_entry_decision(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
    msa: &mut MatchArg,
) -> Option<RegSetDecision> {
    // Replay the search's cumulative retry budget from its first candidate
    // position. `onig_match_with_msa_start` deliberately does not reset this
    // counter because a position-lead caller reuses one MatchArg.
    msa.retry_limit_in_search_counter = 0;
    let mut position = start;
    loop {
        let result = match_regset_entry(
            set,
            index,
            str_data,
            end,
            position,
            start,
            option,
            skip_region_for_nomem,
            msa,
        );
        if result >= 0 {
            return Some(RegSetDecision::Match(RegSetWinner {
                index: index as i32,
                position: position as i32,
                match_len: result,
            }));
        }
        if result != ONIG_MISMATCH {
            return Some(RegSetDecision::Error(RegSetError {
                code: result,
                index: index as i32,
                position: position as i32,
            }));
        }
        if msa.retry_limit_in_search != 0
            && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
        {
            return Some(RegSetDecision::Error(RegSetError {
                code: ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER,
                index: index as i32,
                position: position as i32,
            }));
        }
        if position >= range || position >= end {
            return None;
        }
        position += enclen(set.enc, str_data, position);
    }
}

#[inline]
fn regset_decision_result(set: &mut OnigRegSet, decision: Option<RegSetDecision>) -> (i32, i32) {
    match decision {
        Some(RegSetDecision::Match(winner)) => {
            set.last_match_len = winner.match_len;
            (winner.index, winner.position)
        }
        Some(RegSetDecision::Error(error)) => {
            set.last_match_len = ONIG_MISMATCH;
            (error.code, 0)
        }
        None => {
            set.last_match_len = ONIG_MISMATCH;
            (ONIG_MISMATCH, 0)
        }
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
) -> Option<RegSetDecision> {
    // All entries are optimizer-backed fallbacks. They are searched below in
    // one pass each; walking every byte here would add an otherwise empty
    // O(n) table pass to every cached no-match lookup.
    if set.table_entry_count == 0 {
        return None;
    }

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
    // Search retry budgets start fresh for this public API call and then
    // accumulate independently per regex as position-lead advances. The
    // default global limit is zero, so avoid touching the per-entry scratch
    // vector on the table-only fast path where it cannot be observed.
    let track_search_retry_limit = msa.retry_limit_in_search != 0;
    if track_search_retry_limit {
        if set.scratch_table_retry_counters.len() != set.entries.len() {
            set.scratch_table_retry_counters
                .resize(set.entries.len(), 0);
        }
        set.scratch_table_retry_counters.fill(0);
    }

    let prev_is_newline_check = set.anychar_inf;
    let mut result = None;

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
            if track_search_retry_limit {
                msa.retry_limit_in_search_counter = set.scratch_table_retry_counters[i];
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
            if track_search_retry_limit {
                set.scratch_table_retry_counters[i] = msa.retry_limit_in_search_counter;
            }

            if r >= 0 {
                result = Some(RegSetDecision::Match(RegSetWinner {
                    index: i as i32,
                    position: s as i32,
                    match_len: r,
                }));
                break 'search;
            }
            if r != ONIG_MISMATCH {
                result = Some(RegSetDecision::Error(RegSetError {
                    code: r,
                    index: i as i32,
                    position: s as i32,
                }));
                break 'search;
            }
            if track_search_retry_limit
                && set.scratch_table_retry_counters[i] > msa.retry_limit_in_search
            {
                result = Some(RegSetDecision::Error(RegSetError {
                    code: ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER,
                    index: i as i32,
                    position: s as i32,
                }));
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

/// Run the established table scan first, then independently search only the
/// entries whose start byte is not provable from their bytecode.
///
/// A delayed optimizer byte is not a safe position-lead dispatch key: it may
/// occur after a match's real start. Calling `onig_search` for such an entry
/// preserves that entry's optimizer and obtains its true earliest start. The
/// current table winner bounds each fallback search: a later-index entry only
/// needs positions strictly before the winner, while an earlier-index entry
/// also needs the winner's position to resolve a tie.
#[allow(clippy::too_many_arguments)]
fn regset_search_body_position_lead(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
    fallback_memo_id: Option<FallbackMemoIdentity>,
) -> (i32, i32) {
    // MatchArg captures process-global limits when created. The Acquire
    // revision read synchronizes with a setter's Release revision bump before
    // this changed path reloads the tuple, while unchanged table-only scanner
    // calls avoid all four limit atomics. A non-zero time limit additionally
    // requires a fresh search clock for each public PositionLead invocation.
    let limit_revision = onig_get_global_limit_revision();
    if set.scratch_limits_revision != Some(limit_revision) {
        set.scratch_limits = Some(FallbackMemoLimits::current());
        set.scratch_limits_revision = Some(limit_revision);
        set.scratch_msa = None;
    }
    let limits = set
        .scratch_limits
        .expect("global limit revision always initializes cached limits");
    if limits.time_limit != 0 {
        set.scratch_msa = None;
    }

    if set.fallback_search_candidates.is_empty() {
        let decision = regset_search_body_position_lead_table(
            set,
            str_data,
            end,
            start,
            range,
            option,
            skip_region_for_nomem,
        );
        return regset_decision_result(set, decision);
    }

    let memo_enabled = fallback_memo_id.is_some() && range == end;
    if let Some(identity) = fallback_memo_id.filter(|_| memo_enabled) {
        let key = FallbackMemoKey {
            identity,
            end,
            option,
            retry_limit_in_match: limits.retry_limit_in_match,
            retry_limit_in_search: limits.retry_limit_in_search,
            match_stack_limit: limits.match_stack_limit,
            time_limit: limits.time_limit,
        };
        if set.fallback_memo_key != Some(key) {
            set.fallback_memo_key = Some(key);
            for memos in &mut set.fallback_memos {
                memos.clear();
            }
        }
    }

    let mut decision = regset_search_body_position_lead_table(
        set,
        str_data,
        end,
        start,
        range,
        option,
        skip_region_for_nomem,
    );
    let mut fallback_msa = None;

    for candidate_at in 0..set.fallback_search_candidates.len() {
        let index = set.fallback_search_candidates[candidate_at] as usize;
        // Callouts and position-sensitive bytecode can observe each attempt,
        // so replay those entries rather than reusing a cached result.
        let memo_enabled = memo_enabled && set.entries[index].fallback_memo_safe;
        if (set.entries[index].reg.anchor & ANCR_BEGIN_POSITION) != 0 {
            if decision.is_some_and(|current| {
                (start as i32, index as i32) >= decision_position_and_index(current)
            }) {
                continue;
            }

            let msa = fallback_msa.get_or_insert_with(|| {
                set.scratch_msa
                    .take()
                    .unwrap_or_else(|| MatchArg::new(&set.entries[0].reg, option, None, start))
            });
            if let Some(candidate) = locate_regset_entry_decision(
                set,
                index,
                str_data,
                end,
                start,
                start,
                option,
                skip_region_for_nomem,
                msa,
            ) {
                record_regset_decision(set, &mut decision, candidate);
            }
            continue;
        }

        let search_range = match decision {
            Some(current) if index as i32 >= decision_position_and_index(current).1 => {
                let position = decision_position_and_index(current).0 as usize;
                match onigenc_get_prev_char_head(set.enc, start, position, str_data) {
                    Some(position) => position,
                    None => continue,
                }
            }
            Some(current) => decision_position_and_index(current).0 as usize,
            None => range,
        };
        if search_range < start {
            continue;
        }

        if memo_enabled {
            if set.fallback_memos[index].iter().any(|memo| {
                matches!(memo, FallbackMemo::NoMatchFrom(searched_from) if start >= *searched_from)
            }) {
                continue;
            }
            if let Some((searched_from, position)) = set.fallback_memos[index]
                .iter()
                .filter_map(|memo| match *memo {
                    FallbackMemo::MatchAt {
                        searched_from,
                        position,
                    } if start >= searched_from && position >= start => {
                        Some((searched_from, position))
                    }
                    _ => None,
                })
                .min_by_key(|(_, position)| *position)
            {
                if decision.is_some_and(|current| {
                    (position as i32, index as i32) >= decision_position_and_index(current)
                }) {
                    continue;
                }
                let msa = fallback_msa.get_or_insert_with(|| {
                    set.scratch_msa
                        .take()
                        .unwrap_or_else(|| MatchArg::new(&set.entries[0].reg, option, None, start))
                });
                if let Some(candidate) = locate_regset_entry_decision(
                    set,
                    index,
                    str_data,
                    end,
                    position,
                    position,
                    option,
                    skip_region_for_nomem,
                    msa,
                ) {
                    record_regset_decision(set, &mut decision, candidate);
                } else {
                    set.fallback_memos[index]
                            .retain(|memo| !matches!(memo, FallbackMemo::MatchAt { searched_from: cached_from, position: cached_position } if *cached_from == searched_from && *cached_position == position));
                }
                continue;
            }

            if let Some(exact_start) = set.fallback_memos[index].iter().find_map(|memo| match *memo
            {
                FallbackMemo::ExactStartMiss(exact_start) => Some(exact_start),
                _ => None,
            }) {
                if exact_start == start {
                    continue;
                }
                // A different position cannot use an exact miss. Fall
                // through to one optimizer search over the remaining text;
                // its MatchAt/NoMatchFrom result is the advancing cursor.
            }
        }

        // `onig_search` treats an equal start/range as exactly one direct
        // match attempt. Avoid rebuilding its optimizer/search state for that
        // case; the position-lead helper has identical `\G`, retry-limit and
        // FIND_LONGEST-at-this-position semantics.
        let has_different_exact_start_miss = memo_enabled
            && set.fallback_memos[index].iter().any(|memo| {
                matches!(memo, FallbackMemo::ExactStartMiss(exact_start) if *exact_start != start)
            });
        if search_range == start && !has_different_exact_start_miss {
            let msa = fallback_msa.get_or_insert_with(|| {
                set.scratch_msa
                    .take()
                    .unwrap_or_else(|| MatchArg::new(&set.entries[0].reg, option, None, start))
            });
            if let Some(candidate) = locate_regset_entry_decision(
                set,
                index,
                str_data,
                end,
                start,
                start,
                option,
                skip_region_for_nomem,
                msa,
            ) {
                record_regset_decision(set, &mut decision, candidate);
            } else if memo_enabled {
                let memos = &mut set.fallback_memos[index];
                if memos.len() == FALLBACK_MEMO_CAPACITY {
                    memos.remove(0);
                }
                memos.push(FallbackMemo::ExactStartMiss(start));
            }
            continue;
        }

        let region = set.entries[index].region.take();
        let msa = fallback_msa.get_or_insert_with(|| {
            set.scratch_msa
                .take()
                .unwrap_or_else(|| MatchArg::new(&set.entries[0].reg, option, None, start))
        });
        msa.reset_for_search(&set.entries[index].reg, option, region, start);
        let (position, returned_region) = onig_search_with_msa_and_right_range(
            &set.entries[index].reg,
            str_data,
            end,
            start,
            if memo_enabled { end } else { search_range },
            end,
            msa,
        );
        set.entries[index].region = returned_region;

        if position >= 0 {
            if memo_enabled {
                let memos = &mut set.fallback_memos[index];
                memos.retain(|memo| !matches!(memo, FallbackMemo::ExactStartMiss(_)));
                memos.retain(|memo| {
                    !matches!(memo, FallbackMemo::MatchAt { searched_from, .. } if *searched_from == start)
                });
                if memos.len() == FALLBACK_MEMO_CAPACITY {
                    memos.remove(0);
                }
                memos.push(FallbackMemo::MatchAt {
                    searched_from: start,
                    position: position as usize,
                });
            }
            let region = set.entries[index]
                .region
                .as_ref()
                .expect("regset entries retain their match region");
            record_regset_decision(
                set,
                &mut decision,
                RegSetDecision::Match(RegSetWinner {
                    index: index as i32,
                    position,
                    match_len: region.end[0].saturating_sub(region.beg[0]),
                }),
            );
        } else if position == ONIG_MISMATCH {
            if memo_enabled {
                let memos = &mut set.fallback_memos[index];
                memos.clear();
                memos.push(FallbackMemo::NoMatchFrom(start));
            }
        } else {
            // `onig_search` reports the error code but not the start position
            // that caused it. Only this exceptional path replays exact match
            // attempts up to the current decision boundary, so an earlier
            // fallback match (or error) wins with position-lead semantics.
            if let Some(candidate) = locate_regset_entry_decision(
                set,
                index,
                str_data,
                end,
                start,
                search_range,
                option,
                skip_region_for_nomem,
                msa,
            ) {
                record_regset_decision(set, &mut decision, candidate);
            }
        }
    }

    if fallback_msa.is_some() {
        set.scratch_msa = fallback_msa;
    }
    regset_decision_result(set, decision)
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
    fallback_memo_id: Option<FallbackMemoIdentity>,
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

    // A begin-position anchor sets `cur_range = start + 1` even when `start`
    // already sits at the logical end, so the range can point one past the
    // haystack. C tolerates that because its buffer is NUL-terminated; the
    // position loops below index the haystack up to `cur_range`, so keep it
    // inside the string. The only attempt that survives is the one at `end`.
    let cur_range = cur_range.min(end);

    let (result, match_pos) = if lead == OnigRegSetLead::PositionLead {
        regset_search_body_position_lead(
            set,
            str_data,
            end,
            cur_start,
            cur_range,
            option,
            !eager_region_reset,
            fallback_memo_id,
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
    onig_regset_search_impl(set, str_data, end, start, range, lead, option, true, None)
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
    onig_regset_search_impl(set, str_data, end, start, range, lead, option, false, None)
}

/// Fast RegSet search with a stable immutable string identity for caching
/// optimizer-backed fallback results across advancing scanner positions.
#[allow(clippy::too_many_arguments)]
pub(crate) fn onig_regset_search_fast_with_id(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    identity: FallbackMemoIdentity,
) -> (i32, i32) {
    onig_regset_search_impl(
        set,
        str_data,
        end,
        start,
        range,
        lead,
        option,
        false,
        Some(identity),
    )
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
    regset_search_body_position_lead(set, str_data, end, start, range, option, false, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encodings::utf8::ONIG_ENCODING_UTF8;
    use crate::regcomp::onig_new;
    use crate::regexec::{
        LIMIT_TEST_LOCK, onig_get_global_limit_revision, onig_get_match_stack_limit,
        onig_get_retry_limit_in_match, onig_get_retry_limit_in_search, onig_get_time_limit,
        onig_set_match_stack_limit, onig_set_retry_limit_in_match, onig_set_retry_limit_in_search,
        onig_set_time_limit,
    };
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
        for byte in *b"abcd" {
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
            assert!(
                map_for(vec![operation], Vec::new())
                    .iter()
                    .any(|&value| value != 0)
            );
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
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let reg = compile(b"(?<=x)a?bc");
        assert!(has_finite_variable_optimizer(&reg));
        let map = derive_start_byte_map(&reg).expect("lookbehind is analyzable");
        assert_ne!(map[b'a' as usize], 0);
        assert_ne!(map[b'b' as usize], 0);
        assert_eq!(map[b'x' as usize], 0);

        let (set, result) = onig_regset_new(vec![reg]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(set.fallback_search_candidates.is_empty());

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
    fn unbounded_optimizer_stays_on_the_search_fallback() {
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let set = set.expect("regset");

        assert_eq!(set.fallback_search_candidates, vec![0]);
        assert_eq!(set.table_entry_count, 0);
    }

    #[test]
    fn table_entry_count_tracks_mixed_add_remove_and_replace_sets() {
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.table_entry_count, 0);

        assert_eq!(onig_regset_add(&mut set, compile(b"x")), ONIG_NORMAL);
        assert_eq!(set.table_entry_count, 1);
        assert_eq!(
            onig_regset_replace(&mut set, 1, None),
            ONIG_NORMAL,
            "removing the sole table entry restores a pure fallback set"
        );
        assert_eq!(set.table_entry_count, 0);

        assert_eq!(
            onig_regset_replace(&mut set, 0, Some(compile(b"x"))),
            ONIG_NORMAL
        );
        assert_eq!(set.table_entry_count, 1);
        assert!(set.fallback_search_candidates.is_empty());

        assert_eq!(onig_regset_add(&mut set, compile(b"a*bc")), ONIG_NORMAL);
        assert_eq!(set.table_entry_count, 1);
        assert_eq!(set.fallback_search_candidates, vec![1]);
    }

    #[test]
    fn all_fallback_memo_hit_skips_the_empty_table_pass() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = vec![b'a'; 80_000];
        let identity = FallbackMemoIdentity::OnigString(11);

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::NoMatchFrom(0)]
        ));

        // A cached fallback miss needs neither a table MatchArg nor a
        // byte-by-byte table walk. Leaving this empty distinguishes the O(1)
        // bypass from the former empty-table scan.
        set.scratch_msa = None;
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert!(set.scratch_msa.is_none());
    }

    #[test]
    fn mixed_set_keeps_its_table_search() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.table_entry_count, 1);

        let input = b"x";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::OnigString(12),
            ),
            (1, 0)
        );
    }

    #[test]
    fn fallback_memo_preserves_progressing_match_regions_and_lengths() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"aabcxxbc";

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(41),
            ),
            (0, 0)
        );
        assert_eq!(onig_regset_last_match_len(&set), 4);
        let region = onig_regset_get_region(&set, 0).expect("first region");
        assert_eq!((region.beg[0], region.end[0]), (0, 4));

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                4,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(41),
            ),
            (0, 6)
        );
        assert_eq!(onig_regset_last_match_len(&set), 2);
        let region = onig_regset_get_region(&set, 0).expect("second region");
        assert_eq!((region.beg[0], region.end[0]), (6, 8));
        assert_eq!(set.fallback_memos[0].len(), 2);
    }

    #[test]
    fn fallback_memo_invalidates_ids_limits_and_regset_changes() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"aaaa";
        let search = |set: &mut OnigRegSet, id| {
            onig_regset_search_fast_with_id(
                set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                id,
            )
        };

        assert_eq!(
            search(&mut set, FallbackMemoIdentity::Caller(42)),
            (ONIG_MISMATCH, 0)
        );
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::NoMatchFrom(0)]
        ));
        let first_key = set.fallback_memo_key.expect("memo key");
        let first_revision = set
            .scratch_limits_revision
            .expect("first search captures the global-limit revision");
        assert_eq!(
            search(&mut set, FallbackMemoIdentity::Caller(43)),
            (ONIG_MISMATCH, 0)
        );
        assert_ne!(
            set.fallback_memo_key.expect("new ID key").identity,
            first_key.identity
        );
        assert!(set.scratch_msa.is_some());
        assert_eq!(
            set.scratch_limits,
            Some(FallbackMemoLimits::current()),
            "identity changes clear result memos, not reusable MatchArg buffers"
        );

        let old_retry_match = onig_get_retry_limit_in_match();
        let old_retry_search = onig_get_retry_limit_in_search();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        onig_set_retry_limit_in_match(old_retry_match.saturating_add(1));
        onig_set_retry_limit_in_search(old_retry_search.saturating_add(1));
        onig_set_match_stack_limit(old_stack.saturating_add(1));
        onig_set_time_limit(old_time.saturating_add(1));
        assert_ne!(
            onig_get_global_limit_revision(),
            first_revision,
            "every global limit setter invalidates cached MatchArg limits"
        );
        assert_eq!(
            search(&mut set, FallbackMemoIdentity::Caller(43)),
            (ONIG_MISMATCH, 0)
        );
        let changed_key = set.fallback_memo_key.expect("limit key");
        assert_ne!(
            changed_key.retry_limit_in_match,
            first_key.retry_limit_in_match
        );
        assert_ne!(
            changed_key.retry_limit_in_search,
            first_key.retry_limit_in_search
        );
        assert_ne!(changed_key.match_stack_limit, first_key.match_stack_limit);
        assert_ne!(changed_key.time_limit, first_key.time_limit);
        onig_set_retry_limit_in_match(old_retry_match);
        onig_set_retry_limit_in_search(old_retry_search);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);

        assert_eq!(onig_regset_add(&mut set, compile(b"x")), ONIG_NORMAL);
        assert!(set.fallback_memo_key.is_none());
        assert_eq!(set.fallback_memos.len(), 2);
        assert_eq!(onig_regset_replace(&mut set, 1, None), ONIG_NORMAL);
        assert!(set.fallback_memo_key.is_none());
        assert_eq!(set.fallback_memos.len(), 1);
    }

    #[test]
    fn fallback_memo_separates_caller_and_onig_string_id_domains() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");

        let no_match = b"aaaa";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                no_match,
                no_match.len(),
                0,
                no_match.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(1),
            ),
            (ONIG_MISMATCH, 0)
        );

        // The numeric values intentionally collide. Their sources do not:
        // caller-managed IDs must never poison an internally allocated
        // `OnigString` identity.
        let matching = b"aabc";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                matching,
                matching.len(),
                0,
                matching.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::OnigString(1),
            ),
            (0, 0)
        );
    }

    #[test]
    fn fallback_memo_rebuilds_match_arg_after_limit_change() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = onig_get_retry_limit_in_match();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"aaaa";
        let identity = FallbackMemoIdentity::OnigString(7);

        onig_set_retry_limit_in_match(old_limit.saturating_add(1));
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert_eq!(
            set.scratch_msa
                .as_ref()
                .expect("first search leaves scratch state")
                .retry_limit_in_match,
            old_limit.saturating_add(1)
        );

        let lowered = old_limit.saturating_sub(1);
        onig_set_retry_limit_in_match(lowered);
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert_eq!(
            set.scratch_msa
                .as_ref()
                .expect("limit change rebuilds scratch state")
                .retry_limit_in_match,
            lowered
        );
        onig_set_retry_limit_in_match(old_limit);
    }

    #[test]
    fn fallback_memo_limit_change_matches_a_fresh_regset_error() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = onig_get_retry_limit_in_match();
        let input = format!("x{}b", "a".repeat(1_001));
        let identity = FallbackMemoIdentity::OnigString(10);
        let new_set = || {
            let (set, result) = onig_regset_new(vec![compile(br".*x(a+)+b")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = new_set();

        onig_set_retry_limit_in_match(old_limit.max(1_000_000));
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut reused,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (0, 0)
        );

        onig_set_retry_limit_in_match(1);
        let reused_result = onig_regset_search_fast_with_id(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
            identity,
        );
        let mut fresh = new_set();
        let fresh_result = onig_regset_search_fast_with_id(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
            identity,
        );
        onig_set_retry_limit_in_match(old_limit);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn position_lead_refreshes_retry_limits_for_all_fallback_no_id_searches() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        let input = format!("{}bx", "a".repeat(1_001));
        let make = || {
            let (set, result) = onig_regset_new(vec![compile(br"(a*)\1b")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = make();

        onig_set_retry_limit_in_match(old_match.max(1_000_000));
        onig_set_retry_limit_in_search(0);
        let _ = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(100);
        let reused_result = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        let mut fresh = make();
        let fresh_result = onig_regset_search(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn table_position_lead_refreshes_retry_limits_and_reports_search_over() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        let input = format!("{}b", "a".repeat(32));
        let make = || {
            let (set, result) = onig_regset_new(vec![compile(br"(?:a|aa)*\z")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = make();
        assert_eq!(reused.table_entry_count, 1);

        onig_set_retry_limit_in_match(0);
        onig_set_retry_limit_in_search(1_000_000);
        let _ = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_search(100);
        let reused_result = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        let mut fresh = make();
        let fresh_result = onig_regset_search(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, 0));
    }

    #[test]
    fn table_position_lead_skips_retry_scratch_when_search_limit_is_disabled() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_search = onig_get_retry_limit_in_search();
        onig_set_retry_limit_in_search(0);

        let (set, result) = onig_regset_new(vec![compile(b"a")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.table_entry_count, 1);
        // A nonzero sentinel proves the default path neither initializes nor
        // clears the vector whose contents are irrelevant when the limit is 0.
        set.scratch_table_retry_counters[0] = 123;
        assert_eq!(
            onig_regset_search(
                &mut set,
                b"a",
                1,
                0,
                1,
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 0)
        );
        assert_eq!(set.scratch_table_retry_counters, vec![123]);

        onig_set_retry_limit_in_search(old_search);
    }

    #[test]
    fn table_retry_search_budget_is_isolated_per_regex() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        onig_set_retry_limit_in_match(0);
        onig_set_retry_limit_in_search(136);

        let input = format!("{}c", "a".repeat(16));
        let upstream = compile(br"(a+)\1bc");
        let (upstream_result, _) = onig_search(
            &upstream,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(upstream_result, ONIG_MISMATCH);
        for regs in [
            vec![compile(br"(a+)\1bc")],
            vec![compile(br"(a+)\1bc"), compile(br"(a+)\1bc")],
        ] {
            let (set, result) = onig_regset_new(regs);
            assert_eq!(result, ONIG_NORMAL);
            let mut set = set.expect("regset");
            assert!(set.table_entry_count >= 1);
            assert_eq!(
                onig_regset_search(
                    &mut set,
                    input.as_bytes(),
                    input.len(),
                    0,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ),
                (ONIG_MISMATCH, 0),
                "each regex receives its own upstream-compatible search budget"
            );
        }

        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);
    }

    #[test]
    fn nonzero_time_limit_starts_a_fresh_position_lead_search_clock() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_time = onig_get_time_limit();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        let input = "a".repeat(600);
        let make = || {
            let (set, result) = onig_regset_new(vec![compile(br"(?:a|aa)*\z")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = make();

        onig_set_retry_limit_in_match(0);
        onig_set_retry_limit_in_search(0);
        onig_set_time_limit(50);
        assert_eq!(
            onig_regset_search(
                &mut reused,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 0)
        );
        std::thread::sleep(std::time::Duration::from_millis(75));

        let reused_result = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        let mut fresh = make();
        let fresh_result = onig_regset_search(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        onig_set_time_limit(old_time);
        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (0, 0));
    }

    #[test]
    fn fallback_memo_skips_position_dependent_patterns() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(br"a*(?:\Gx|y)")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.fallback_search_candidates, vec![0]);
        assert!(
            set.entries[0]
                .reg
                .ops
                .iter()
                .any(|op| op.opcode == OpCode::CheckPosition)
        );

        let input = b"axc";
        let identity = FallbackMemoIdentity::OnigString(8);
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (0, 1)
        );
        assert!(set.fallback_memos[0].is_empty());
    }

    #[test]
    fn fallback_memo_safety_is_precomputed_when_entries_change() {
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(set.entries[0].fallback_memo_safe);

        assert_eq!(
            onig_regset_replace(&mut set, 0, Some(compile(br".*(?{x})a"))),
            ONIG_NORMAL
        );
        assert!(!set.entries[0].fallback_memo_safe);

        assert_eq!(
            onig_regset_replace(&mut set, 0, Some(compile(br"a*(?:\Gx|y)"))),
            ONIG_NORMAL
        );
        assert!(!set.entries[0].fallback_memo_safe);
    }

    #[test]
    fn exact_start_fallback_checks_do_not_prefetch_the_suffix() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"xaaaaaaaaaaaaaaaa";

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::OnigString(9),
            ),
            (1, 0)
        );
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::ExactStartMiss(0)]
        ));
    }

    #[test]
    fn identical_direct_miss_is_cached_without_a_second_vm_attempt() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"x";
        let identity = FallbackMemoIdentity::OnigString(13);

        for _ in 0..2 {
            assert_eq!(
                onig_regset_search_fast_with_id(
                    &mut set,
                    input,
                    input.len(),
                    0,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                    identity,
                ),
                (1, 0)
            );
        }
        assert!(
            set.fallback_memos[0]
                .iter()
                .any(|memo| matches!(memo, FallbackMemo::ExactStartMiss(0)))
        );
    }

    #[test]
    fn second_direct_miss_upgrades_to_an_advancing_optimizer_cursor() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = vec![b'x'; 80_000];
        let identity = FallbackMemoIdentity::OnigString(14);

        // The first table winner causes one cheap exact fallback attempt.
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::ExactStartMiss(0)]
        ));

        // The next start replaces the exact-only result with a full-search
        // cursor. Every later table winner can reuse its no-match result.
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 1)
        );
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::NoMatchFrom(1)]
        ));
        for start in [2, input.len() - 1] {
            assert_eq!(
                onig_regset_search_fast_with_id(
                    &mut set,
                    &input,
                    input.len(),
                    start,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                    identity,
                ),
                (1, start as i32)
            );
        }
    }

    #[test]
    fn direct_miss_upgrade_preserves_fallback_match_region_and_restart_passes() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x"), compile(b"a")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"xaabc";
        let identity = FallbackMemoIdentity::OnigString(15);

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (0, 1)
        );
        assert_eq!(onig_regset_last_match_len(&set), 4);
        let region = onig_regset_get_region(&set, 0).expect("fallback region");
        assert_eq!((region.beg[0], region.end[0]), (1, 5));

        // A pass restarted at zero cannot reuse a cursor that began at one.
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        assert!(
            set.fallback_memos[0]
                .iter()
                .any(|memo| matches!(memo, FallbackMemo::ExactStartMiss(0)))
        );
    }

    #[test]
    fn direct_miss_upgrade_preserves_a_same_position_retry_error() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let (set, result) =
            onig_regset_new(vec![compile(br"a*x(a+)+b"), compile(b"y"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.fallback_search_candidates, vec![0]);
        let input = format!("yx{}c", "a".repeat(1_001));
        let identity = FallbackMemoIdentity::OnigString(16);

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        let error = onig_regset_search_fast_with_id(
            &mut set,
            input.as_bytes(),
            input.len(),
            1,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
            identity,
        );
        onig_set_retry_limit_in_match(old_limit);
        assert_eq!(error, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn fallback_memo_skips_callout_patterns() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(br".*(?{x})a")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(
            set.entries[0]
                .reg
                .extp
                .as_ref()
                .is_some_and(|ext| ext.callout_num != 0)
        );
        assert_eq!(set.fallback_search_candidates, vec![0]);

        let input = b"x";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(5),
            ),
            (ONIG_MISMATCH, 0)
        );
        assert!(set.fallback_memos[0].is_empty());
    }

    #[test]
    fn unbounded_no_match_uses_one_optimizer_scan_then_memoizes() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.fallback_search_candidates, vec![0]);
        assert!(set.first_byte_candidates[b'a' as usize].is_empty());

        let input = vec![b'a'; 80_000];
        for start in [0, 1] {
            assert_eq!(
                onig_regset_search_fast_with_id(
                    &mut set,
                    &input,
                    input.len(),
                    start,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                    FallbackMemoIdentity::Caller(99),
                ),
                (ONIG_MISMATCH, 0)
            );
        }
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::NoMatchFrom(0)]
        ));
    }

    #[test]
    fn fallback_does_not_probe_a_start_after_an_unbeatable_table_winner() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let (set, result) = onig_regset_new(vec![compile(br"(a*)\1b"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = format!("x{}b", "a".repeat(1_001));
        let (index, position) = onig_regset_search(
            &mut set,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!((index, position), (1, 0));
    }

    #[test]
    fn fallback_match_precedes_a_later_table_retry_error() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let input = format!("bx{}c", "a".repeat(1_001));
        for (patterns, expected_index) in [
            ([br"x(a+)+b".as_slice(), br"(a*)\1b".as_slice()], 1),
            ([br"(a*)\1b".as_slice(), br"x(a+)+b".as_slice()], 0),
        ] {
            let (set, result) = onig_regset_new(patterns.into_iter().map(compile).collect());
            assert_eq!(result, ONIG_NORMAL);
            let mut set = set.expect("regset");

            let (index, position) = onig_regset_search(
                &mut set,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            );
            assert_eq!((index, position), (expected_index, 0));
            assert_eq!(onig_regset_last_match_len(&set), 1);
        }

        onig_set_retry_limit_in_match(old_limit);
    }

    #[test]
    fn table_retry_error_wins_a_same_start_later_fallback_match() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let input = format!("x{}c", "a".repeat(1_001));
        let (set, result) = onig_regset_new(vec![compile(br"x(a+)+b"), compile(br"(x*)\1?")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");

        let (index, position) = onig_regset_search(
            &mut set,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!((index, position), (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn fallback_error_clears_a_superseded_winner_and_match_length() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let input = format!("{}bx", "a".repeat(1_001));
        let (set, result) = onig_regset_new(vec![compile(br"(a*)\1b"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let search_result = onig_regset_search(
            &mut set,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!(search_result, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
        assert_eq!(onig_regset_last_match_len(&set), ONIG_MISMATCH);
        for index in 0..2 {
            let region = onig_regset_get_region(&set, index).expect("entry region");
            assert_eq!(
                (region.beg[0], region.end[0]),
                (ONIG_REGION_NOTPOS, ONIG_REGION_NOTPOS)
            );
        }
    }

    #[test]
    fn fallback_search_preserves_g_anchor_and_beats_a_later_table_match() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();
        let (set, result) = onig_regset_new(vec![compile(br"\["), compile(br"\G\s*\[")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.fallback_search_candidates, vec![1]);

        let input = b"xx [";
        let (index, position) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            2,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        assert_eq!((index, position), (1, 2));
        let region = onig_regset_get_region(&set, index as usize).expect("winning region");
        assert_eq!((region.beg[0], region.end[0]), (2, 4));
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
