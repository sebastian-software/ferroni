// regexec.rs - Port of regexec.c
// VM executor: bytecode interpreter, match_at, onig_match, onig_search.
//
// This is a 1:1 port of oniguruma's regexec.c (~7,000 LOC).
// Structure mirrors the C original: stack types → stack operations →
// match_at (opcode dispatch) → onig_match → onig_search.

#![allow(non_upper_case_globals)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_mut)]

use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::time::Instant;

use crate::oniguruma::*;
use crate::regenc::*;
use crate::regint::*;

// ============================================================================
// Global Limits (port of C's onig_retry_limit_in_match etc.)
// ============================================================================

static RETRY_LIMIT_IN_MATCH: AtomicU64 = AtomicU64::new(DEFAULT_RETRY_LIMIT_IN_MATCH);
static RETRY_LIMIT_IN_SEARCH: AtomicU64 = AtomicU64::new(DEFAULT_RETRY_LIMIT_IN_SEARCH);
static MATCH_STACK_LIMIT: AtomicU32 = AtomicU32::new(DEFAULT_MATCH_STACK_LIMIT_SIZE);
static TIME_LIMIT: AtomicU64 = AtomicU64::new(DEFAULT_TIME_LIMIT_MSEC);

pub fn onig_set_retry_limit_in_match(n: u64) { RETRY_LIMIT_IN_MATCH.store(n, Ordering::Relaxed); }
pub fn onig_get_retry_limit_in_match() -> u64 { RETRY_LIMIT_IN_MATCH.load(Ordering::Relaxed) }
pub fn onig_set_retry_limit_in_search(n: u64) { RETRY_LIMIT_IN_SEARCH.store(n, Ordering::Relaxed); }
pub fn onig_get_retry_limit_in_search() -> u64 { RETRY_LIMIT_IN_SEARCH.load(Ordering::Relaxed) }
pub fn onig_set_match_stack_limit(n: u32) { MATCH_STACK_LIMIT.store(n, Ordering::Relaxed); }
pub fn onig_get_match_stack_limit() -> u32 { MATCH_STACK_LIMIT.load(Ordering::Relaxed) }
pub fn onig_set_time_limit(n: u64) { TIME_LIMIT.store(n, Ordering::Relaxed); }
pub fn onig_get_time_limit() -> u64 { TIME_LIMIT.load(Ordering::Relaxed) }

// ============================================================================
// Stack Types (port of StackType / STK_* constants)
// ============================================================================

/// Memory pointer - tracks capture group start/end positions.
/// Corresponds to C's StkPtrType union { StackIndex i; UChar* s; }
#[derive(Clone, Copy, Debug)]
enum MemPtr {
    /// Not yet matched / invalid
    Invalid,
    /// Index into the stack (for push_mem variants that use backtracking)
    StackIdx(usize),
    /// Direct string position (for non-push variants)
    Pos(usize),
}

/// Stack entry - corresponds to C's StackType struct.
/// Uses enum to distinguish entry types instead of C's type field + union.
#[derive(Clone)]
enum StackEntry {
    /// Choice point (STK_ALT / STK_SUPER_ALT) - alternate path for backtracking.
    Alt {
        pcode: usize,    // bytecode index to jump to on backtrack
        pstr: usize,     // string position to restore
        zid: i32,        // remaining count for StepBackNext (-1 = unused)
        is_super: bool,  // true for SUPER_ALT (survives CutToMark)
    },
    /// Capture group start (STK_MEM_START)
    MemStart {
        zid: usize,
        pstr: usize,
        prev_start: MemPtr,
        prev_end: MemPtr,
    },
    /// Capture group end (STK_MEM_END)
    MemEnd {
        zid: usize,
        pstr: usize,
        prev_start: MemPtr,
        prev_end: MemPtr,
    },
    /// Capture group end marker (STK_MEM_END_MARK) - for recursive groups
    MemEndMark {
        zid: usize,
    },
    /// Repeat counter (STK_REPEAT_INC)
    RepeatInc {
        zid: usize,
        count: i32,
    },
    /// Empty check start marker (STK_EMPTY_CHECK_START)
    EmptyCheckStart {
        zid: usize,
        pstr: usize,
    },
    /// Empty check end marker (STK_EMPTY_CHECK_END)
    EmptyCheckEnd {
        zid: usize,
    },
    /// Named checkpoint (STK_MARK) - for lookaheads/lookbehinds
    Mark {
        zid: usize,
        pos: Option<usize>, // saved string position (if save_pos)
    },
    /// Saved value (STK_SAVE_VAL)
    SaveVal {
        zid: usize,
        save_type: SaveType,
        v: usize,
    },
    /// Call frame return address (STK_CALL_FRAME)
    CallFrame {
        ret_addr: usize,
    },
    /// Return marker (STK_RETURN)
    Return,
    /// Callout entry (STK_CALLOUT) - for retraction callbacks
    Callout {
        num: i32,       // callout list index (1-based)
        id: i32,        // builtin id (for name callouts) or ONIG_NON_NAME_ID
    },
    /// Voided entry (STK_VOID) - dead space, skipped during pops
    Void,
}

impl StackEntry {
    /// Returns true if this entry is an ALT (choice point) that stops STACK_POP.
    #[inline]
    fn is_alt(&self) -> bool {
        matches!(self, StackEntry::Alt { .. })
    }

    /// Returns true if this entry needs handling during pop at ALL level.
    #[inline]
    fn is_pop_handled(&self) -> bool {
        matches!(
            self,
            StackEntry::MemStart { .. }
                | StackEntry::MemEnd { .. }
                | StackEntry::RepeatInc { .. }
                | StackEntry::EmptyCheckStart { .. }
                | StackEntry::CallFrame { .. }
                | StackEntry::Return
        )
    }
}

// Sentinel value for the bottom ALT entry's pcode
const FINISH_PCODE: usize = usize::MAX;

// ============================================================================
// MatchArg - runtime match state (port of C's MatchArg)
// ============================================================================

pub struct MatchArg {
    pub options: OnigOptionType,
    pub region: Option<OnigRegion>,
    pub start: usize, // search start position (for \G anchor)
    pub best_len: i32,
    pub best_s: usize,
    // Safety limits
    pub retry_limit_in_match: u64,
    pub retry_limit_in_search: u64,
    pub retry_limit_in_search_counter: u64,
    pub match_stack_limit: u32,
    pub time_limit: u64,  // milliseconds, 0 = unlimited
    /// Lazily-initialized search start time for time-limit checking.
    /// None until the first time check fires, then set to Instant::now().
    time_start: Option<Box<Instant>>,
}

const CHECK_TIME_INTERVAL: u64 = 512;

impl MatchArg {
    fn new(
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
    ) -> Self {
        MatchArg {
            options: option | reg.options,
            region,
            start,
            best_len: ONIG_MISMATCH,
            best_s: 0,
            retry_limit_in_match: RETRY_LIMIT_IN_MATCH.load(Ordering::Relaxed),
            retry_limit_in_search: RETRY_LIMIT_IN_SEARCH.load(Ordering::Relaxed),
            retry_limit_in_search_counter: 0,
            match_stack_limit: MATCH_STACK_LIMIT.load(Ordering::Relaxed),
            time_limit: TIME_LIMIT.load(Ordering::Relaxed),
            time_start: None,
        }
    }

    /// Check if the time limit has been exceeded. Returns true if over limit.
    /// On first call, initializes the start time.
    #[inline]
    fn check_time_limit(&mut self) -> bool {
        if self.time_limit == 0 { return false; }
        let start = self.time_start.get_or_insert_with(|| Box::new(Instant::now()));
        start.elapsed() >= std::time::Duration::from_millis(self.time_limit)
    }
}

// ============================================================================
// Stack limit check
// ============================================================================

/// Check stack size against match_stack_limit. Returns Err with error code if exceeded.
#[inline]
fn check_stack_limit(stack_len: usize, limit: u32) -> Result<(), i32> {
    if limit != 0 && stack_len >= limit as usize {
        return Err(ONIGERR_MATCH_STACK_LIMIT_OVER);
    }
    Ok(())
}

// ============================================================================
// Stack operations (port of STACK_PUSH_* / STACK_POP macros)
// ============================================================================

/// Pop stack entries until an ALT (choice point) is found.
/// Restores mem_start_stk/mem_end_stk as needed based on pop_level.
/// Handles callout retraction when reg/callout_data are provided.
/// Returns Some((pcode, pstr, zid)) from the ALT entry, or None if stack is empty.
fn stack_pop(
    stack: &mut Vec<StackEntry>,
    pop_level: StackPopLevel,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
    reg: &RegexType,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
) -> Option<(usize, usize, i32)> {
    loop {
        let entry = stack.pop()?;
        match entry {
            StackEntry::Alt { pcode, pstr, zid, .. } => {
                return Some((pcode, pstr, zid));
            }
            _ => match pop_level {
                StackPopLevel::Free => {
                    // Skip non-ALT entries without restoration
                }
                StackPopLevel::MemStart => {
                    // Restore MEM_START entries only
                    if let StackEntry::MemStart {
                        zid,
                        prev_start,
                        prev_end,
                        ..
                    } = &entry
                    {
                        mem_start_stk[*zid] = *prev_start;
                        mem_end_stk[*zid] = *prev_end;
                    }
                }
                StackPopLevel::All => {
                    // Restore all handled entries
                    match &entry {
                        StackEntry::MemStart {
                            zid,
                            prev_start,
                            prev_end,
                            ..
                        } => {
                            mem_start_stk[*zid] = *prev_start;
                            mem_end_stk[*zid] = *prev_end;
                        }
                        StackEntry::MemEnd {
                            zid,
                            prev_start,
                            prev_end,
                            ..
                        } => {
                            mem_start_stk[*zid] = *prev_start;
                            mem_end_stk[*zid] = *prev_end;
                        }
                        StackEntry::Callout { num, id } => {
                            // Retraction callback
                            run_builtin_callout_retraction(reg, *num, *id, callout_data);
                        }
                        // RepeatInc, EmptyCheckStart, CallFrame, Return:
                        // handled implicitly (popping removes them)
                        _ => {}
                    }
                }
            },
        }
    }
}

/// Pop stack entries until a Mark with matching zid is found (STACK_POP_TO_MARK).
/// Removes ALL entries. Restores mem_start_stk/mem_end_stk along the way.
/// Returns the saved position from the Mark entry (if any).
fn stack_pop_to_mark(
    stack: &mut Vec<StackEntry>,
    mark_id: usize,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
) -> Option<usize> {
    loop {
        let entry = stack.pop()?;
        match &entry {
            StackEntry::Mark { zid, pos } if *zid == mark_id => {
                return *pos;
            }
            StackEntry::MemStart {
                zid,
                prev_start,
                prev_end,
                ..
            } => {
                mem_start_stk[*zid] = *prev_start;
                mem_end_stk[*zid] = *prev_end;
            }
            StackEntry::MemEnd {
                zid,
                prev_start,
                prev_end,
                ..
            } => {
                mem_start_stk[*zid] = *prev_start;
                mem_end_stk[*zid] = *prev_end;
            }
            _ => {}
        }
    }
}

/// Void stack entries until a Mark with matching zid is found (STACK_TO_VOID_TO_MARK).
/// Only voids "void targets" (regular Alt, EmptyCheckStart, Mark) by setting them to Void.
/// Preserves non-void targets (SuperAlt, SaveVal, MemStart, MemEnd, RepeatInc, etc.) in place.
/// Void stack entries from top to the Mark with matching id (C: STACK_TO_VOID_TO_MARK).
/// Returns the mark's saved position. Voids regular Alt and EmptyCheckStart entries,
/// but preserves Super Alt entries and Marks with different IDs.
fn stack_void_to_mark(
    stack: &mut Vec<StackEntry>,
    mark_id: usize,
) -> Option<usize> {
    let mut i = stack.len();
    while i > 0 {
        i -= 1;
        // Check if this is the target Mark
        if let StackEntry::Mark { zid, pos } = &stack[i] {
            if *zid == mark_id {
                let saved_pos = *pos;
                stack[i] = StackEntry::Void;
                return saved_pos;
            }
            // Different id mark: don't void, just skip
            continue;
        }
        // Void targets: regular Alt and EmptyCheckStart
        // Super Alt (is_super=true) is NOT voided — it survives cuts
        let is_void_target = matches!(
            &stack[i],
            StackEntry::Alt { is_super: false, .. }
            | StackEntry::EmptyCheckStart { .. }
        );
        if is_void_target {
            stack[i] = StackEntry::Void;
        }
    }
    None
}

/// Search backwards through the stack for the most recent RepeatInc with matching zid.
/// Returns the count from that entry.
fn stack_get_repeat_count(stack: &[StackEntry], zid: usize) -> i32 {
    for entry in stack.iter().rev() {
        if let StackEntry::RepeatInc {
            zid: id,
            count,
            ..
        } = entry
        {
            if *id == zid {
                return *count;
            }
        }
    }
    0
}

/// Check if the empty check for the given zid matches the same string position.
/// Returns true if the string position hasn't advanced (empty match).
fn stack_empty_check(stack: &[StackEntry], zid: usize, s: usize) -> bool {
    for entry in stack.iter().rev() {
        if let StackEntry::EmptyCheckStart { zid: id, pstr } = entry {
            if *id == zid {
                return *pstr == s;
            }
        }
    }
    false
}

/// Memory-aware empty check. Returns true only if position is same AND no capture
/// groups (indicated by empty_status_mem) have changed since the EmptyCheckStart.
/// Mirrors C's STACK_EMPTY_CHECK_MEM.
/// Check if a quantifier iteration was empty (position unchanged).
/// Returns: false = not empty (position changed or captures changed),
///          true = truly empty (pos same AND captures same)
fn stack_empty_check_mem(
    stack: &[StackEntry],
    zid: usize,
    s: usize,
    empty_status_mem: u32,
    _reg: &RegexType,
    _mem_start_stk: &[MemPtr],
    _mem_end_stk: &[MemPtr],
) -> bool {
    // Find the EmptyCheckStart entry
    let mut klow_idx = None;
    for (i, entry) in stack.iter().enumerate().rev() {
        if let StackEntry::EmptyCheckStart { zid: id, pstr } = entry {
            if *id == zid {
                if *pstr != s {
                    return false; // position changed → not empty
                }
                klow_idx = Some(i);
                break;
            }
        }
    }

    let klow_idx = match klow_idx {
        Some(i) => i,
        None => return false,
    };

    // Position is the same. Check if any capture groups changed.
    let mut ms = empty_status_mem as u32;
    for k_idx in (klow_idx + 1..stack.len()).rev() {
        if let StackEntry::MemEnd { zid: mem_zid, pstr: end_pstr, .. } = &stack[k_idx] {
            if ms & (1u32 << *mem_zid) != 0 {
                // Found a MemEnd for a tracked group. Check if its value differs
                // from the previous iteration's value.
                // Look for the corresponding MemStart between klow and this MemEnd.
                for kk_idx in klow_idx + 1..k_idx {
                    if let StackEntry::MemStart { zid: start_zid, prev_end, .. } = &stack[kk_idx] {
                        if *start_zid == *mem_zid {
                            // Check if prev_end was invalid (group wasn't captured before)
                            match prev_end {
                                MemPtr::Invalid => {
                                    // Previously not captured, now captured → not empty
                                    return false;
                                }
                                MemPtr::Pos(prev_pos) => {
                                    if *prev_pos != *end_pstr {
                                        return false; // end position changed
                                    }
                                }
                                MemPtr::StackIdx(si) => {
                                    if let StackEntry::MemEnd { pstr: prev_pstr, .. } = &stack[*si] {
                                        if *prev_pstr != *end_pstr {
                                            return false;
                                        }
                                    }
                                }
                            }
                            ms &= !(1u32 << *mem_zid);
                            break;
                        }
                    }
                }
                if ms == 0 { break; }
            }
        }
    }

    true // position same AND no captures changed → truly empty
}

/// Get the saved value for a given save_type and zid from the stack.
/// Search stack for last SaveVal by type only (C: STACK_GET_SAVE_VAL_TYPE_LAST)
fn stack_get_save_val_type_last(
    stack: &[StackEntry],
    save_type: SaveType,
) -> Option<usize> {
    for entry in stack.iter().rev() {
        if let StackEntry::SaveVal {
            save_type: st,
            v,
            ..
        } = entry
        {
            if *st == save_type {
                return Some(*v);
            }
        }
    }
    None
}

/// Search stack for last SaveVal by type AND id (C: STACK_GET_SAVE_VAL_TYPE_LAST_ID)
fn stack_get_save_val_last(
    stack: &[StackEntry],
    save_type: SaveType,
    zid: usize,
) -> Option<usize> {
    for entry in stack.iter().rev() {
        if let StackEntry::SaveVal {
            zid: id,
            save_type: st,
            v,
        } = entry
        {
            if *id == zid && *st == save_type {
                return Some(*v);
            }
        }
    }
    None
}

/// Get the string position where a capture group starts.
fn get_mem_start(
    reg: &RegexType,
    stack: &[StackEntry],
    mem_start_stk: &[MemPtr],
    idx: usize,
) -> Option<usize> {
    match mem_start_stk[idx] {
        MemPtr::Invalid => None,
        MemPtr::Pos(pos) => Some(pos),
        MemPtr::StackIdx(si) => {
            if let StackEntry::MemStart { pstr, .. } = &stack[si] {
                Some(*pstr)
            } else {
                None
            }
        }
    }
}

/// Get the string position where a capture group ends.
fn get_mem_end(
    reg: &RegexType,
    stack: &[StackEntry],
    mem_end_stk: &[MemPtr],
    idx: usize,
) -> Option<usize> {
    match mem_end_stk[idx] {
        MemPtr::Invalid => None,
        MemPtr::Pos(pos) => Some(pos),
        MemPtr::StackIdx(si) => {
            if let StackEntry::MemEnd { pstr, .. } = &stack[si] {
                Some(*pstr)
            } else {
                None
            }
        }
    }
}

/// Level-aware scan for the matching MEM_START of a recursive capture group.
/// Mirrors C's STACK_GET_MEM_START macro — counts MemEnd/MemEndMark entries
/// to track nesting level, finds MemStart at level 0.
fn stack_get_mem_start_for_rec(
    stack: &[StackEntry],
    mem: usize,
    push_mem_start: u32,
) -> (MemPtr, usize) {
    let mut level: i32 = 0;
    let mut k = stack.len();
    while k > 0 {
        k -= 1;
        match &stack[k] {
            StackEntry::MemEnd { zid, .. } | StackEntry::MemEndMark { zid } if *zid == mem => {
                level += 1;
            }
            StackEntry::MemStart { zid, pstr, .. } if *zid == mem => {
                if level == 0 {
                    // Found matching start at correct level
                    if mem_status_at(push_mem_start, mem) {
                        return (MemPtr::StackIdx(k), *pstr);
                    } else {
                        return (MemPtr::Pos(*pstr), *pstr);
                    }
                }
                level -= 1;
            }
            _ => {}
        }
    }
    (MemPtr::Invalid, 0)
}

/// Match a backref at a specific nesting level in the recursion stack.
/// Walks the stack backwards counting CallFrame/Return to find the right level,
/// then matches the captured text at that level against current position.
fn backref_match_at_nested_level(
    reg: &RegexType,
    stack: &[StackEntry],
    ignore_case: bool,
    case_fold_flag: OnigCaseFoldType,
    nest: i32,
    mem_num: i32,
    mems: &[i32],
    s: &mut usize,
    str_data: &[u8],
    end: usize,
) -> bool {
    let mut level: i32 = 0;
    let mut pend: Option<usize> = None;
    let mut k = stack.len();

    while k > 0 {
        k -= 1;
        match &stack[k] {
            StackEntry::CallFrame { .. } => {
                level -= 1;
            }
            StackEntry::Return => {
                level += 1;
            }
            StackEntry::MemStart { zid, pstr, .. } if level == nest => {
                if mem_is_in_mems(*zid, mem_num, mems) {
                    if let Some(pe) = pend {
                        let pstart = *pstr;
                        let cap_len = pe - pstart;
                        if cap_len > end - *s {
                            return false;
                        }
                        if ignore_case {
                            if !string_cmp_ic(reg.enc, case_fold_flag, str_data, pstart, s, cap_len) {
                                return false;
                            }
                        } else {
                            if str_data[pstart..pe] != str_data[*s..*s + cap_len] {
                                return false;
                            }
                            *s += cap_len;
                        }
                        return true;
                    }
                }
            }
            StackEntry::MemEnd { zid, pstr, .. } if level == nest => {
                if mem_is_in_mems(*zid, mem_num, mems) {
                    pend = Some(*pstr);
                }
            }
            _ => {}
        }
    }
    false
}

/// Check if a backref capture exists at a specific nesting level.
fn backref_check_at_nested_level(
    stack: &[StackEntry],
    nest: i32,
    mem_num: i32,
    mems: &[i32],
) -> bool {
    let mut level: i32 = 0;
    let mut k = stack.len();

    while k > 0 {
        k -= 1;
        match &stack[k] {
            StackEntry::CallFrame { .. } => { level -= 1; }
            StackEntry::Return => { level += 1; }
            StackEntry::MemStart { zid, .. } if level == nest => {
                if mem_is_in_mems(*zid, mem_num, mems) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[inline]
fn mem_is_in_mems(mem: usize, num: i32, mems: &[i32]) -> bool {
    for i in 0..num as usize {
        if mem == mems[i] as usize {
            return true;
        }
    }
    false
}


// ============================================================================
// Helper functions
// ============================================================================

/// Check if a byte is a word character (ASCII-only).
#[inline]
fn is_word_ascii(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Check if the character at position s is a word character (encoding-aware).
/// Uses the encoding's Unicode-aware is_code_ctype for multi-byte encodings.
#[inline]
fn is_word_char_at(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    if s >= end {
        return false;
    }
    let code = enc.mbc_to_code(&str_data[s..], end);
    enc.is_code_ctype(code, ONIGENC_CTYPE_WORD)
}

/// Get the start of the previous character (left_adjust_char_head).
#[inline]
fn prev_char_head(enc: OnigEncoding, start: usize, s: usize, str_data: &[u8]) -> usize {
    if s <= start {
        return s;
    }
    enc.left_adjust_char_head(start, s - 1, str_data)
}

/// Check word boundary at position s (encoding-aware).
/// Returns true if there's a word boundary between s-1 and s.
fn is_word_boundary(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    let at_start = s == 0;
    let at_end = s >= end;

    if at_start && at_end {
        return false;
    }
    if at_start {
        return is_word_char_at(enc, str_data, s, end);
    }
    if at_end {
        let prev = prev_char_head(enc, 0, s, str_data);
        return is_word_char_at(enc, str_data, prev, end);
    }

    let prev = prev_char_head(enc, 0, s, str_data);
    let prev_word = is_word_char_at(enc, str_data, prev, end);
    let curr_word = is_word_char_at(enc, str_data, s, end);
    prev_word != curr_word
}

/// Check if position s is at the start of a word (encoding-aware).
fn is_word_begin(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    if s >= end {
        return false;
    }
    if !is_word_char_at(enc, str_data, s, end) {
        return false;
    }
    if s == 0 {
        return true;
    }
    let prev = prev_char_head(enc, 0, s, str_data);
    !is_word_char_at(enc, str_data, prev, end)
}

/// Check if position s is at the end of a word (encoding-aware).
fn is_word_end(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    if s == 0 {
        return false;
    }
    let prev = prev_char_head(enc, 0, s, str_data);
    if !is_word_char_at(enc, str_data, prev, end) {
        return false;
    }
    if s >= end {
        return true;
    }
    !is_word_char_at(enc, str_data, s, end)
}

/// Check if a code point is in a multi-byte range table.
/// The table format is: n:u32 (range count) followed by n pairs of (from:u32, to:u32).
/// All values in native-endian byte order. Binary search.
pub(crate) fn is_in_code_range(mb: &[u8], code: OnigCodePoint) -> bool {
    if mb.len() < 4 {
        return false;
    }
    let n = u32::from_ne_bytes([mb[0], mb[1], mb[2], mb[3]]) as usize;
    if mb.len() < 4 + n * 8 {
        return false;
    }

    let mut low: usize = 0;
    let mut high: usize = n;
    while low < high {
        let x = (low + high) >> 1;
        let off = 4 + x * 8;
        let range_high = u32::from_ne_bytes([mb[off + 4], mb[off + 5], mb[off + 6], mb[off + 7]]);
        if code > range_high {
            low = x + 1;
        } else {
            high = x;
        }
    }

    if low < n {
        let off = 4 + low * 8;
        let range_low = u32::from_ne_bytes([mb[off], mb[off + 1], mb[off + 2], mb[off + 3]]);
        code >= range_low
    } else {
        false
    }
}

/// Get the character length at position s for the given encoding.
#[inline]
fn enclen(enc: OnigEncoding, str_data: &[u8], s: usize) -> usize {
    if s >= str_data.len() {
        1
    } else {
        enc.mbc_enc_len(&str_data[s..])
    }
}

/// Case-insensitive string comparison using encoding-aware case folding.
/// Compares `mblen` bytes starting at `s1_pos` with bytes starting at `*s2_pos`.
/// Advances `*s2_pos` past consumed bytes on success. Returns true if equal.
fn string_cmp_ic(
    enc: OnigEncoding,
    case_fold_flag: OnigCaseFoldType,
    data: &[u8],
    s1_pos: usize,
    s2_pos: &mut usize,
    mblen: usize,
) -> bool {
    let mut buf1 = [0u8; ONIGENC_MBC_CASE_FOLD_MAXLEN];
    let mut buf2 = [0u8; ONIGENC_MBC_CASE_FOLD_MAXLEN];
    let end1 = s1_pos + mblen;
    let end2 = *s2_pos + mblen;
    let mut p1 = s1_pos;
    let mut p2 = *s2_pos;

    while p1 < end1 {
        let len1 = enc.mbc_case_fold(case_fold_flag, &mut p1, end1, data, &mut buf1);
        let len2 = enc.mbc_case_fold(case_fold_flag, &mut p2, end2, data, &mut buf2);
        if len1 != len2 {
            return false;
        }
        if buf1[..len1 as usize] != buf2[..len2 as usize] {
            return false;
        }
        if p2 >= end2 {
            if p1 < end1 {
                return false;
            }
            break;
        }
    }

    *s2_pos = p2;
    true
}

// ============================================================================
// ============================================================================
// Extended Grapheme Cluster boundary detection (simplified)
// ============================================================================

/// Check if position `s` in `data` is an Extended Grapheme Cluster boundary.
/// Returns true if there's a break at this position (GB1/GB2 rules).
///
/// Simplified implementation covering common cases:
/// - Start/end of string are always breaks (GB1, GB2)
/// - CR+LF is not a break (GB3)
/// - Control chars (CR/LF/Control) cause breaks (GB4, GB5)
/// - Combining marks (Extend/ZWJ/SpacingMark) after non-control chars are not breaks (GB9, GB9a)
/// - Everything else is a break (GB999)
fn egcb_is_break_position(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    // GB1: Break at start of text
    if s == 0 { return true; }
    // GB2: Break at end of text
    if s >= end { return true; }

    let prev_pos = prev_char_head(enc, 0, s, str_data);
    if prev_pos >= s { return true; } // no previous char

    let from = enc.mbc_to_code(&str_data[prev_pos..], end);
    let to = enc.mbc_to_code(&str_data[s..], end);

    // GB3: Do not break between CR and LF
    if from == 0x0D && to == 0x0A { return false; }

    // GB4: Break after Control, CR, LF
    if is_egcb_control_cr_lf(from) { return true; }
    // GB5: Break before Control, CR, LF
    if is_egcb_control_cr_lf(to) { return true; }

    // GB9: Do not break before Extend or ZWJ
    if is_egcb_extend(to) || to == 0x200D { return false; }

    // GB9a: Do not break before SpacingMark
    if is_egcb_spacing_mark(to) { return false; }

    // GB9b: Do not break after Prepend
    if is_egcb_prepend(from) { return false; }

    // GB999: Otherwise, break everywhere
    true
}

/// Check if a codepoint is a Control, CR, or LF character
fn is_egcb_control_cr_lf(c: u32) -> bool {
    c == 0x0D || c == 0x0A
        || (c >= 0x00 && c <= 0x1F && c != 0x0D && c != 0x0A) // C0 controls
        || c == 0x7F // DEL
        || c == 0xAD // SOFT HYPHEN
        || (c >= 0x0600 && c <= 0x0605) // Arabic number signs (actually Prepend in newer Unicode)
        || c == 0x200B // ZERO WIDTH SPACE
        || c == 0x2028 // LINE SEPARATOR
        || c == 0x2029 // PARAGRAPH SEPARATOR
        || (c >= 0xFFF0 && c <= 0xFFF8) // Specials
        || (c >= 0xE0000 && c <= 0xE007F && c != 0xE0020 && c != 0xE007F) // Tags (simplified)
}

/// Check if a codepoint is an Extend character (combining marks, etc.)
fn is_egcb_extend(c: u32) -> bool {
    // Combining Diacritical Marks
    (c >= 0x0300 && c <= 0x036F)
    // Combining Diacritical Marks Extended
    || (c >= 0x1AB0 && c <= 0x1AFF)
    // Combining Diacritical Marks Supplement
    || (c >= 0x1DC0 && c <= 0x1DFF)
    // Combining Diacritical Marks for Symbols
    || (c >= 0x20D0 && c <= 0x20FF)
    // Combining Half Marks
    || (c >= 0xFE20 && c <= 0xFE2F)
    // General Category M (Mark) approximation for common ranges
    || (c >= 0x0483 && c <= 0x0489)
    || (c >= 0x0591 && c <= 0x05BD)
    || (c >= 0x0610 && c <= 0x061A)
    || (c >= 0x064B && c <= 0x065F)
    || c == 0x0670
    || (c >= 0x06D6 && c <= 0x06DC)
    || (c >= 0x06DF && c <= 0x06E4)
    || (c >= 0x06E7 && c <= 0x06E8)
    || (c >= 0x06EA && c <= 0x06ED)
    || (c >= 0x0900 && c <= 0x0903)
    || (c >= 0x093A && c <= 0x094F)
    // Variation Selectors
    || (c >= 0xFE00 && c <= 0xFE0F)
    || (c >= 0xE0100 && c <= 0xE01EF)
    // Zero Width Joiner is handled separately
    // Format characters that act as Extend
    || c == 0x200C // ZWNJ
    || c == 0x200D // ZWJ (handled separately but include here for safety)
}

/// Check if a codepoint is a SpacingMark
fn is_egcb_spacing_mark(c: u32) -> bool {
    // Common SpacingMark ranges (Indic scripts vowel signs, etc.)
    (c >= 0x0903 && c <= 0x0903)
    || (c >= 0x093B && c <= 0x093B)
    || (c >= 0x093E && c <= 0x0940)
    || (c >= 0x0949 && c <= 0x094C)
    || c == 0x094E || c == 0x094F
    || (c >= 0x0982 && c <= 0x0983)
}

/// Check if a codepoint is a Prepend character
fn is_egcb_prepend(c: u32) -> bool {
    c == 0x0600 || c == 0x0601 || c == 0x0602 || c == 0x0603
    || c == 0x0604 || c == 0x0605
    || c == 0x06DD || c == 0x070F
    || c == 0x0890 || c == 0x0891
    || c == 0x08E2
    || c == 0x110BD || c == 0x110CD
}

// ============================================================================
// Builtin callout functions
// ============================================================================

/// Run a builtin callout in the progress direction.
/// Returns ONIG_CALLOUT_SUCCESS or ONIG_CALLOUT_FAIL.
fn run_builtin_callout(
    reg: &RegexType,
    num: i32,
    _id: i32,
    is_retraction: bool,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
) -> i32 {
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return ONIG_CALLOUT_SUCCESS,
    };
    if num < 1 || (num as usize) > ext.callout_list.len() {
        return ONIG_CALLOUT_SUCCESS;
    }
    let entry = &ext.callout_list[(num - 1) as usize];
    let idx = (num - 1) as usize;

    match entry.builtin_id {
        CALLOUT_BUILTIN_MAX => {
            let max_val = resolve_callout_arg(&entry.args[0], &ext.callout_list, callout_data);
            builtin_max(entry, is_retraction, &mut callout_data[idx], max_val)
        }
        CALLOUT_BUILTIN_COUNT => builtin_count(entry, is_retraction, &mut callout_data[idx]),
        CALLOUT_BUILTIN_CMP => {
            let lv = resolve_callout_arg(&entry.args[0], &ext.callout_list, callout_data);
            let rv = resolve_callout_arg(&entry.args[2], &ext.callout_list, callout_data);
            builtin_cmp(entry, &mut callout_data[idx], lv, rv)
        }
        _ => ONIG_CALLOUT_SUCCESS,
    }
}

/// Run a builtin callout in the retraction direction (called from stack_pop).
fn run_builtin_callout_retraction(
    reg: &RegexType,
    num: i32,
    _id: i32,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
) {
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return,
    };
    if num < 1 || (num as usize) > ext.callout_list.len() {
        return;
    }
    let entry = &ext.callout_list[(num - 1) as usize];
    let slots = &mut callout_data[(num - 1) as usize];

    match entry.builtin_id {
        CALLOUT_BUILTIN_MAX => {
            // Retraction for MAX: same logic as progress but with is_retraction=true
            // We can't pass the full callout_data for tag resolution here since we already
            // have a mutable borrow. But MAX retraction only reads slots[0] (its own counter).
            let count_type = if entry.args.len() > 1 {
                match &entry.args[1] {
                    CalloutArg::Char(c) => *c,
                    _ => b'X',
                }
            } else {
                b'X'
            };
            if count_type == b'<' {
                // retraction + '<': increment and check
                let max_val = resolve_callout_arg(&entry.args[0], &[], &[]);
                if slots[0] >= max_val {
                    // fail — but retraction result is ignored per C code
                } else {
                    slots[0] += 1;
                }
            } else if count_type == b'X' {
                // retraction + 'X': decrement
                slots[0] -= 1;
            }
            // retraction + '>': no-op
        }
        CALLOUT_BUILTIN_COUNT => {
            let count_type = if !entry.args.is_empty() {
                match &entry.args[0] {
                    CalloutArg::Char(c) => *c,
                    _ => b'>',
                }
            } else {
                b'>'
            };
            // Retraction: slot 2
            if count_type == b'<' {
                slots[0] += 1;
            } else if count_type == b'X' {
                slots[0] -= 1;
            }
            // slot 2 (retraction counter) increment
            slots[2] += 1;
        }
        _ => {}
    }
}

/// Resolve a callout argument: Long value directly or Tag → lookup slot[0] of tagged callout.
fn resolve_callout_arg(
    arg: &CalloutArg,
    ext_callout_list: &[CalloutListEntry],
    all_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
) -> i64 {
    match arg {
        CalloutArg::Long(n) => *n,
        CalloutArg::Tag(tag) => {
            for (i, e) in ext_callout_list.iter().enumerate() {
                if let Some(ref t) = e.tag {
                    if t == tag {
                        if i < all_data.len() {
                            return all_data[i][0];
                        }
                    }
                }
            }
            0
        }
        _ => 0,
    }
}

fn builtin_max(
    entry: &CalloutListEntry,
    is_retraction: bool,
    slots: &mut [i64; ONIG_CALLOUT_DATA_SLOT_NUM],
    max_val: i64,
) -> i32 {
    // slots[0] = current count
    let count_type = if entry.args.len() > 1 {
        match &entry.args[1] {
            CalloutArg::Char(c) => *c,
            _ => b'X',
        }
    } else {
        b'X'
    };

    if is_retraction {
        if count_type == b'<' {
            if slots[0] >= max_val {
                return ONIG_CALLOUT_FAIL;
            }
            slots[0] += 1;
        } else if count_type == b'X' {
            slots[0] -= 1;
        }
    } else {
        if count_type != b'<' {
            if slots[0] >= max_val {
                return ONIG_CALLOUT_FAIL;
            }
            slots[0] += 1;
        }
    }

    ONIG_CALLOUT_SUCCESS
}

fn builtin_count(
    entry: &CalloutListEntry,
    is_retraction: bool,
    slots: &mut [i64; ONIG_CALLOUT_DATA_SLOT_NUM],
) -> i32 {
    // slots[0] = main counter (progress - retraction adjusted)
    // slots[1] = progress counter
    // slots[2] = retraction counter
    let count_type = if !entry.args.is_empty() {
        match &entry.args[0] {
            CalloutArg::Char(c) => *c,
            _ => b'>',
        }
    } else {
        b'>'
    };

    if is_retraction {
        if count_type == b'<' {
            slots[0] += 1;
        } else if count_type == b'X' {
            slots[0] -= 1;
        }
        // slot 2 (retraction counter)
        slots[2] += 1;
    } else {
        if count_type != b'<' {
            slots[0] += 1;
        }
        // slot 1 (progress counter)
        slots[1] += 1;
    }

    ONIG_CALLOUT_SUCCESS
}

fn builtin_cmp(
    entry: &CalloutListEntry,
    slots: &mut [i64; ONIG_CALLOUT_DATA_SLOT_NUM],
    lv: i64,
    rv: i64,
) -> i32 {
    // CMP is progress-only
    if entry.args.len() < 3 {
        return ONIG_CALLOUT_FAIL;
    }

    // Parse op on first call, cache in slots[0]
    let op = if slots[3] == 0 {
        // First call: parse operator string
        let op_val = match &entry.args[1] {
            CalloutArg::Str(s) => parse_cmp_op(s),
            CalloutArg::Char(c) => parse_cmp_op(&[*c]),
            _ => return ONIG_CALLOUT_FAIL,
        };
        slots[3] = 1; // mark as initialized
        slots[4] = op_val as i64;
        op_val
    } else {
        slots[4] as i32
    };

    let result = match op {
        0 => lv == rv, // ==
        1 => lv != rv, // !=
        2 => lv < rv,  // <
        3 => lv > rv,  // >
        4 => lv <= rv, // <=
        5 => lv >= rv, // >=
        _ => false,
    };

    if result { ONIG_CALLOUT_SUCCESS } else { ONIG_CALLOUT_FAIL }
}

fn parse_cmp_op(s: &[u8]) -> i32 {
    match s {
        b"==" => 0,
        b"!=" => 1,
        b"<" => 2,
        b">" => 3,
        b"<=" => 4,
        b">=" => 5,
        _ => -1,
    }
}

// ============================================================================
// match_at - the core VM executor (port of C's match_at function)
// ============================================================================

/// Execute the bytecode VM starting at position `sstart` in the string.
/// Returns match length on success, ONIG_MISMATCH (-1) on failure.
///
/// Parameters match C's match_at(reg, str, end, in_right_range, sstart, msa):
/// - reg: compiled regex with bytecode in reg.ops
/// - str_data: the input string bytes
/// - end: end position in str_data
/// - in_right_range: right boundary for matching
/// - sstart: position to start matching at
/// - msa: mutable match state (options, region, etc.)
fn match_at(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    in_right_range: usize,
    sstart: usize,
    msa: &mut MatchArg,
) -> i32 {
    let mut p: usize = 0; // bytecode index into reg.ops
    let mut s: usize = sstart; // current string position
    let mut right_range: usize = in_right_range;
    let pop_level = reg.stack_pop_level;
    let num_mem = reg.num_mem as usize;
    let enc = reg.enc;
    let options = msa.options;

    // Stack for backtracking
    let mut stack: Vec<StackEntry> = Vec::with_capacity(INIT_MATCH_STACK_SIZE);

    // Capture group tracking arrays
    let mut mem_start_stk: Vec<MemPtr> = vec![MemPtr::Invalid; num_mem + 1];
    let mut mem_end_stk: Vec<MemPtr> = vec![MemPtr::Invalid; num_mem + 1];

    let mut keep: usize = sstart;
    let mut best_len: i32 = ONIG_MISMATCH;
    let mut last_alt_zid: i32 = -1;

    // Safety limits
    let retry_limit_in_match = msa.retry_limit_in_match;
    let mut retry_in_match_counter: u64 = 0;
    let match_stack_limit = msa.match_stack_limit;
    let time_limit_ms = msa.time_limit;

    // Callout data: per-callout mutable slots (indexed by callout num - 1)
    let callout_count = reg.extp.as_ref().map_or(0, |e| e.callout_num as usize);
    let mut callout_data: Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]> = vec![[0i64; ONIG_CALLOUT_DATA_SLOT_NUM]; callout_count];

    // Push bottom sentinel (like C's STACK_PUSH_BOTTOM with FinishCode)
    stack.push(StackEntry::Alt {
        pcode: FINISH_PCODE,
        pstr: 0,
        zid: -1,
        is_super: false,
    });

    // ---- Main dispatch loop ----
    loop {
        if p >= reg.ops.len() {
            break;
        }

        // Stack limit check (checked once per opcode, like C's STACK_PUSH macro)
        if match_stack_limit != 0 && stack.len() >= match_stack_limit as usize {
            best_len = ONIGERR_MATCH_STACK_LIMIT_OVER;
            break;
        }

        let opcode = reg.ops[p].opcode;
        let mut goto_fail = false;

        match opcode {
            // ================================================================
            // OP_FINISH - reached bottom sentinel, return result
            // ================================================================
            OpCode::Finish => {
                break;
            }

            // ================================================================
            // OP_END - successful match, populate region
            // ================================================================
            OpCode::End => {
                // Check MATCH_WHOLE_STRING option
                if opton_match_whole_string(options) && s < end {
                    goto_fail = true;
                } else {
                    let n = (s - sstart) as i32;
                    if n == 0 && opton_find_not_empty(options) {
                        goto_fail = true;
                    } else if n > best_len {
                        best_len = n;

                        // Populate region with capture groups
                        if let Some(ref mut region) = msa.region {
                            region.resize(num_mem as i32 + 1);
                            region.beg[0] = (keep - 0) as i32; // offset from str start
                            region.end[0] = s as i32;

                            for i in 1..=num_mem {
                                if let Some(mem_end) =
                                    get_mem_end(reg, &stack, &mem_end_stk, i)
                                {
                                    let mem_start =
                                        get_mem_start(reg, &stack, &mem_start_stk, i);
                                    region.beg[i] = mem_start
                                        .map(|v| v as i32)
                                        .unwrap_or(ONIG_REGION_NOTPOS);
                                    region.end[i] = mem_end as i32;
                                } else {
                                    region.beg[i] = ONIG_REGION_NOTPOS;
                                    region.end[i] = ONIG_REGION_NOTPOS;
                                }
                            }
                        }

                        // For non-FIND_LONGEST, return immediately
                        if !opton_find_longest(options) {
                            return best_len;
                        }

                        // FIND_LONGEST: save best and continue searching
                        msa.best_len = best_len;
                        msa.best_s = sstart;
                        goto_fail = true; // backtrack to try longer matches
                    } else {
                        // FIND_LONGEST but shorter/equal match: backtrack for more
                        goto_fail = true;
                    }
                }
            }

            // ================================================================
            // OP_STR1..STR5 - match 1-5 literal bytes
            // ================================================================
            OpCode::Str1 => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s] {
                        goto_fail = true;
                    } else {
                        s += 1;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str2 => {
                if right_range.saturating_sub(s) < 2 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s] || exact[1] != str_data[s + 1] {
                        goto_fail = true;
                    } else {
                        s += 2;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str3 => {
                if right_range.saturating_sub(s) < 3 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s]
                        || exact[1] != str_data[s + 1]
                        || exact[2] != str_data[s + 2]
                    {
                        goto_fail = true;
                    } else {
                        s += 3;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str4 => {
                if right_range.saturating_sub(s) < 4 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s]
                        || exact[1] != str_data[s + 1]
                        || exact[2] != str_data[s + 2]
                        || exact[3] != str_data[s + 3]
                    {
                        goto_fail = true;
                    } else {
                        s += 4;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str5 => {
                if right_range.saturating_sub(s) < 5 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s]
                        || exact[1] != str_data[s + 1]
                        || exact[2] != str_data[s + 2]
                        || exact[3] != str_data[s + 3]
                        || exact[4] != str_data[s + 4]
                    {
                        goto_fail = true;
                    } else {
                        s += 5;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::StrN => {
                if let OperationPayload::ExactN { s: ref exact, n } = reg.ops[p].payload {
                    let n = n as usize;
                    if right_range.saturating_sub(s) < n {
                        goto_fail = true;
                    } else if str_data[s..s + n] != exact[..n] {
                        goto_fail = true;
                    } else {
                        s += n;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // MB string opcodes (for multibyte encodings)
            OpCode::StrMb2n1 | OpCode::StrMb2n2 | OpCode::StrMb2n3 | OpCode::StrMb2n
            | OpCode::StrMb3n | OpCode::StrMbn => {
                // Multi-byte string comparison. ExactLenN.n = total byte count.
                if let OperationPayload::ExactLenN { s: ref exact, n, .. } =
                    reg.ops[p].payload
                {
                    let byte_len = n as usize;
                    if right_range.saturating_sub(s) < byte_len {
                        goto_fail = true;
                    } else if str_data[s..s + byte_len] != exact[..byte_len] {
                        goto_fail = true;
                    } else {
                        s += byte_len;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_CCLASS / OP_CCLASS_NOT - character class matching
            // ================================================================
            OpCode::CClass => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if let OperationPayload::CClass { ref bsp } = reg.ops[p].payload {
                    if !bitset_at(bsp, str_data[s] as usize) {
                        goto_fail = true;
                    } else {
                        s += enclen(enc, str_data, s);
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassNot => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if let OperationPayload::CClass { ref bsp } = reg.ops[p].payload {
                    if bitset_at(bsp, str_data[s] as usize) {
                        goto_fail = true;
                    } else {
                        s += enclen(enc, str_data, s);
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // MB character class (multibyte)
            OpCode::CClassMb => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMb { ref mb } = reg.ops[p].payload {
                    let mb_len = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < mb_len {
                        goto_fail = true;
                    } else {
                        let code = enc.mbc_to_code(&str_data[s..], end);
                        if !is_in_code_range(mb, code) {
                            goto_fail = true;
                        } else {
                            s += mb_len;
                            p += 1;
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassMbNot => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMb { ref mb } = reg.ops[p].payload {
                    let mb_len = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < mb_len {
                        goto_fail = true;
                    } else {
                        let code = enc.mbc_to_code(&str_data[s..], end);
                        if is_in_code_range(mb, code) {
                            goto_fail = true;
                        } else {
                            s += mb_len;
                            p += 1;
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // Mixed character class (single-byte bitset + multibyte ranges)
            OpCode::CClassMix | OpCode::CClassMixNot => {
                let not = opcode == OpCode::CClassMixNot;
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMix { ref bsp, ref mb } = reg.ops[p].payload
                {
                    let in_class = if enc.mbc_enc_len(&str_data[s..]) > 1 {
                        let code = enc.mbc_to_code(&str_data[s..], end);
                        is_in_code_range(mb, code)
                    } else {
                        let c = str_data[s];
                        if (c as usize) < SINGLE_BYTE_SIZE {
                            bitset_at(bsp, c as usize)
                        } else {
                            false
                        }
                    };
                    if in_class == not {
                        goto_fail = true;
                    } else {
                        s += enclen(enc, str_data, s);
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_ANYCHAR / OP_ANYCHAR_ML - match any character
            // ================================================================
            OpCode::AnyChar => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else {
                    let n = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < n {
                        goto_fail = true;
                    } else if enc.is_mbc_newline(&str_data[s..], end) {
                        goto_fail = true; // ANYCHAR doesn't match newline
                    } else {
                        s += n;
                        p += 1;
                    }
                }
            }

            OpCode::AnyCharMl => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else {
                    let n = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < n {
                        goto_fail = true;
                    } else {
                        s += n; // ANYCHAR_ML matches newlines too
                        p += 1;
                    }
                }
            }

            // ================================================================
            // OP_ANYCHAR_STAR / OP_ANYCHAR_ML_STAR - .* optimization
            // ================================================================
            OpCode::AnyCharStar => {
                // Push alternation for each possible length
                // Greedy: try matching as many chars as possible
                while s < right_range {
                    let n = enclen(enc, str_data, s);
                    if s + n > right_range {
                        break;
                    }
                    if enc.is_mbc_newline(&str_data[s..], end) {
                        break;
                    }
                    stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                    s += n;
                }
                p += 1;
            }

            OpCode::AnyCharMlStar => {
                while s < right_range {
                    let n = enclen(enc, str_data, s);
                    if s + n > right_range {
                        break;
                    }
                    stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                    s += n;
                }
                p += 1;
            }

            OpCode::AnyCharStarPeekNext => {
                if let OperationPayload::AnyCharStarPeekNext { c } = reg.ops[p].payload {
                    while s < right_range {
                        let n = enclen(enc, str_data, s);
                        if s + n > right_range {
                            break;
                        }
                        if enc.is_mbc_newline(&str_data[s..], end) {
                            break;
                        }
                        if s < end && str_data[s] == c {
                            stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                        }
                        s += n;
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::AnyCharMlStarPeekNext => {
                if let OperationPayload::AnyCharStarPeekNext { c } = reg.ops[p].payload {
                    while s < right_range {
                        let n = enclen(enc, str_data, s);
                        if s + n > right_range {
                            break;
                        }
                        if s < end && str_data[s] == c {
                            stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                        }
                        s += n;
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Word / NoWord - \w and \W character type matching
            // ================================================================
            OpCode::Word => {
                if s >= right_range {
                    goto_fail = true;
                } else if !is_word_char_at(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            OpCode::WordAscii => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if !is_word_ascii(str_data[s]) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            OpCode::NoWord => {
                if s >= right_range {
                    goto_fail = true;
                } else if is_word_char_at(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            OpCode::NoWordAscii => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if is_word_ascii(str_data[s]) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            // ================================================================
            // Word boundary opcodes
            // ================================================================
            OpCode::WordBoundary => {
                if !is_word_boundary(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::NoWordBoundary => {
                if is_word_boundary(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::WordBegin => {
                if !is_word_begin(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::WordEnd => {
                if !is_word_end(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::TextSegmentBoundary => {
                if let OperationPayload::TextSegmentBoundary { boundary_type, not } = reg.ops[p].payload {
                    let is_break = match boundary_type {
                        TextSegmentBoundaryType::ExtendedGraphemeCluster => {
                            egcb_is_break_position(enc, str_data, s, end)
                        }
                        TextSegmentBoundaryType::Word => {
                            // Word boundary type - treat as always break for now
                            true
                        }
                    };
                    let result = if not { !is_break } else { is_break };
                    if result {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Position anchors
            // ================================================================
            OpCode::BeginBuf => {
                if s != 0 {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::EndBuf => {
                if s != end {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::BeginLine => {
                if s == 0 {
                    if opton_notbol(options) {
                        goto_fail = true;
                    } else {
                        p += 1;
                    }
                } else if s > 0 && str_data[s - 1] == b'\n' {
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EndLine => {
                if s == end {
                    if opton_noteol(options) {
                        goto_fail = true;
                    } else {
                        p += 1;
                    }
                } else if enc.is_mbc_newline(&str_data[s..], end) {
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::SemiEndBuf => {
                // Match end of string or before final newline
                if s == end {
                    p += 1;
                } else if s + 1 == end && str_data[s] == b'\n' {
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CheckPosition => {
                if let OperationPayload::CheckPosition { check_type } = reg.ops[p].payload {
                    match check_type {
                        CheckPositionType::SearchStart => {
                            if s != msa.start {
                                goto_fail = true;
                            } else {
                                p += 1;
                            }
                        }
                        CheckPositionType::CurrentRightRange => {
                            if s != right_range {
                                goto_fail = true;
                            } else {
                                p += 1;
                            }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Back references
            // ================================================================
            OpCode::BackRef1 => {
                if num_mem >= 1 {
                    if let (Some(ms), Some(me)) = (
                        get_mem_start(reg, &stack, &mem_start_stk, 1),
                        get_mem_end(reg, &stack, &mem_end_stk, 1),
                    ) {
                        let ref_len = me - ms;
                        if right_range.saturating_sub(s) < ref_len {
                            goto_fail = true;
                        } else if str_data[s..s + ref_len] != str_data[ms..me] {
                            goto_fail = true;
                        } else {
                            s += ref_len;
                            p += 1;
                        }
                    } else {
                        goto_fail = true; // group not yet matched
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRef2 => {
                if num_mem >= 2 {
                    if let (Some(ms), Some(me)) = (
                        get_mem_start(reg, &stack, &mem_start_stk, 2),
                        get_mem_end(reg, &stack, &mem_end_stk, 2),
                    ) {
                        let ref_len = me - ms;
                        if right_range.saturating_sub(s) < ref_len {
                            goto_fail = true;
                        } else if str_data[s..s + ref_len] != str_data[ms..me] {
                            goto_fail = true;
                        } else {
                            s += ref_len;
                            p += 1;
                        }
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefN => {
                if let OperationPayload::BackRefN { n1 } = reg.ops[p].payload {
                    let n1 = n1 as usize;
                    if n1 <= num_mem {
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, n1),
                            get_mem_end(reg, &stack, &mem_end_stk, n1),
                        ) {
                            let ref_len = me - ms;
                            if right_range.saturating_sub(s) < ref_len {
                                goto_fail = true;
                            } else if str_data[s..s + ref_len] != str_data[ms..me] {
                                goto_fail = true;
                            } else {
                                s += ref_len;
                                p += 1;
                            }
                        } else {
                            goto_fail = true;
                        }
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefNIc => {
                if let OperationPayload::BackRefN { n1 } = reg.ops[p].payload {
                    let n1 = n1 as usize;
                    if n1 <= num_mem {
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, n1),
                            get_mem_end(reg, &stack, &mem_end_stk, n1),
                        ) {
                            let ref_len = me - ms;
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len {
                                    goto_fail = true;
                                } else if !string_cmp_ic(enc, reg.case_fold_flag, str_data, ms, &mut s, ref_len) {
                                    goto_fail = true;
                                } else {
                                    p += 1;
                                }
                            } else {
                                p += 1;
                            }
                        } else {
                            goto_fail = true;
                        }
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefMulti => {
                if let OperationPayload::BackRefGeneral { num, ref ns, .. } = &reg.ops[p].payload {
                    let tlen = *num as usize;
                    let mut matched = false;
                    for i in 0..tlen {
                        let mem = ns[i] as usize;
                        if mem > num_mem { continue; }
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, mem),
                            get_mem_end(reg, &stack, &mem_end_stk, mem),
                        ) {
                            let ref_len = me - ms;
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len { continue; }
                                if str_data[s..s + ref_len] != str_data[ms..me] { continue; }
                                s += ref_len;
                            }
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefMultiIc => {
                if let OperationPayload::BackRefGeneral { num, ref ns, .. } = &reg.ops[p].payload {
                    let tlen = *num as usize;
                    let mut matched = false;
                    for i in 0..tlen {
                        let mem = ns[i] as usize;
                        if mem > num_mem { continue; }
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, mem),
                            get_mem_end(reg, &stack, &mem_end_stk, mem),
                        ) {
                            let ref_len = me - ms;
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len { continue; }
                                let mut swork = s;
                                if !string_cmp_ic(enc, reg.case_fold_flag, str_data, ms, &mut swork, ref_len) { continue; }
                                s = swork;
                            }
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefCheck => {
                if let OperationPayload::BackRefGeneral { num, ref ns, .. } = &reg.ops[p].payload {
                    let tlen = *num as usize;
                    let mut found = false;
                    for i in 0..tlen {
                        let mem = ns[i] as usize;
                        if mem > num_mem { continue; }
                        if get_mem_start(reg, &stack, &mem_start_stk, mem).is_some()
                            && get_mem_end(reg, &stack, &mem_end_stk, mem).is_some()
                        {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefWithLevel | OpCode::BackRefWithLevelIc => {
                if let OperationPayload::BackRefGeneral { num, ref ns, nest_level } = &reg.ops[p].payload {
                    let ignore_case = reg.ops[p].opcode == OpCode::BackRefWithLevelIc;
                    if backref_match_at_nested_level(
                        reg, &stack, ignore_case, reg.case_fold_flag,
                        *nest_level, *num, ns, &mut s, str_data, end,
                    ) {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefCheckWithLevel => {
                if let OperationPayload::BackRefGeneral { num, ref ns, nest_level } = &reg.ops[p].payload {
                    let found = if backref_check_at_nested_level(&stack, *nest_level, *num, ns) {
                        true
                    } else if *nest_level == 0 {
                        // At level 0, also check mem arrays directly (group may use
                        // non-push MemStart with no stack entry)
                        let tlen = *num as usize;
                        let mut f = false;
                        for i in 0..tlen {
                            let mem = ns[i] as usize;
                            if mem > num_mem { continue; }
                            if get_mem_start(reg, &stack, &mem_start_stk, mem).is_some()
                                && get_mem_end(reg, &stack, &mem_end_stk, mem).is_some()
                            {
                                f = true;
                                break;
                            }
                        }
                        f
                    } else {
                        false
                    };
                    if found {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Memory (capture group) operations
            // ================================================================
            OpCode::MemStart => {
                if let OperationPayload::MemoryStart { num } = reg.ops[p].payload {
                    let num = num as usize;
                    mem_start_stk[num] = MemPtr::Pos(s);
                    mem_end_stk[num] = MemPtr::Invalid;
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemStartPush => {
                if let OperationPayload::MemoryStart { num } = reg.ops[p].payload {
                    let num = num as usize;
                    let prev_start = mem_start_stk[num];
                    let prev_end = mem_end_stk[num];
                    let si = stack.len();
                    stack.push(StackEntry::MemStart {
                        zid: num,
                        pstr: s,
                        prev_start,
                        prev_end,
                    });
                    mem_start_stk[num] = MemPtr::StackIdx(si);
                    mem_end_stk[num] = MemPtr::Invalid;
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEnd => {
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let num = num as usize;
                    mem_end_stk[num] = MemPtr::Pos(s);
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndPush => {
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let num = num as usize;
                    let prev_start = mem_start_stk[num];
                    let prev_end = mem_end_stk[num];
                    let si = stack.len();
                    stack.push(StackEntry::MemEnd {
                        zid: num,
                        pstr: s,
                        prev_start,
                        prev_end,
                    });
                    mem_end_stk[num] = MemPtr::StackIdx(si);
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndPushRec => {
                // Recursive capture end (push variant): find matching MEM_START,
                // push MEM_END, update start/end tracking
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let mem = num as usize;
                    let (start_ptr, _) = stack_get_mem_start_for_rec(&stack, mem, reg.push_mem_start);
                    let si = stack.len();
                    stack.push(StackEntry::MemEnd {
                        zid: mem,
                        pstr: s,
                        prev_start: mem_start_stk[mem],
                        prev_end: mem_end_stk[mem],
                    });
                    mem_start_stk[mem] = start_ptr;
                    mem_end_stk[mem] = MemPtr::StackIdx(si);
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndRec => {
                // Recursive capture end (non-push variant): find matching MEM_START,
                // update start/end, push MemEndMark for level tracking
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let mem = num as usize;
                    mem_end_stk[mem] = MemPtr::Pos(s);
                    let (start_ptr, _) = stack_get_mem_start_for_rec(&stack, mem, reg.push_mem_start);
                    mem_start_stk[mem] = start_ptr;
                    stack.push(StackEntry::MemEndMark { zid: mem });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_FAIL - backtrack
            // ================================================================
            OpCode::Fail => {
                goto_fail = true;
            }

            // ================================================================
            // OP_JUMP - unconditional jump
            // ================================================================
            OpCode::Jump => {
                if let OperationPayload::Jump { addr } = reg.ops[p].payload {
                    p = (p as i32 + addr) as usize;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH / OP_PUSH_SUPER - push choice point (alternation)
            // ================================================================
            OpCode::Push | OpCode::PushSuper => {
                if let OperationPayload::Push { addr } = reg.ops[p].payload {
                    let alt_target = (p as i32 + addr) as usize;
                    let is_super = reg.ops[p].opcode == OpCode::PushSuper;
                    stack.push(StackEntry::Alt {
                        pcode: alt_target,
                        pstr: s,
                        zid: -1,
                        is_super,
                    });
                    p += 1; // try main path first
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_POP - discard top stack entry
            // ================================================================
            OpCode::Pop => {
                stack.pop();
                p += 1;
            }

            // ================================================================
            // OP_POP_TO_MARK - pop until Mark with matching id
            // ================================================================
            OpCode::PopToMark => {
                if let OperationPayload::PopToMark { id } = reg.ops[p].payload {
                    let id = id as usize;
                    // Pop entries until we find the matching Mark, but don't
                    // restore positions (unlike CutToMark)
                    loop {
                        if let Some(entry) = stack.pop() {
                            if let StackEntry::Mark { zid, .. } = &entry {
                                if *zid == id {
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH_OR_JUMP_EXACT1 - optimized push for exact char
            // ================================================================
            OpCode::PushOrJumpExact1 => {
                if let OperationPayload::PushOrJumpExact1 { addr, c } = reg.ops[p].payload {
                    if s < right_range && str_data[s] == c {
                        // Character matches: push alternative and continue
                        let alt_target = (p as i32 + addr) as usize;
                        stack.push(StackEntry::Alt {
                            pcode: alt_target,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
                        p += 1;
                    } else {
                        // Character doesn't match: jump
                        p = (p as i32 + addr) as usize;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH_IF_PEEK_NEXT - push only if next char matches
            // ================================================================
            OpCode::PushIfPeekNext => {
                if let OperationPayload::PushIfPeekNext { addr, c } = reg.ops[p].payload {
                    if s < right_range && str_data[s] == c {
                        let alt_target = (p as i32 + addr) as usize;
                        stack.push(StackEntry::Alt {
                            pcode: alt_target,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_REPEAT / OP_REPEAT_NG - initialize repeat counter
            // ================================================================
            OpCode::Repeat | OpCode::RepeatNg => {
                if let OperationPayload::Repeat { id, addr } = reg.ops[p].payload {
                    let id = id as usize;
                    // Push initial repeat count = 0
                    stack.push(StackEntry::RepeatInc { zid: id, count: 0 });

                    if reg.repeat_range[id].lower == 0 {
                        // Can skip the loop body entirely
                        let alt_target = (p as i32 + addr) as usize;
                        if opcode == OpCode::Repeat {
                            // Greedy: push skip as alternative, try body first
                            stack.push(StackEntry::Alt {
                                pcode: alt_target,
                                pstr: s,
                                zid: -1,
                                is_super: false,
                            });
                        } else {
                            // Non-greedy: push body as alternative, try skip first
                            stack.push(StackEntry::Alt {
                                pcode: p + 1,
                                pstr: s,
                                zid: -1,
                                is_super: false,
                            });
                            p = alt_target;
                            continue;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_REPEAT_INC / OP_REPEAT_INC_NG - increment and check repeat
            // ================================================================
            OpCode::RepeatInc => {
                if let OperationPayload::RepeatInc { id } = reg.ops[p].payload {
                    let id = id as usize;
                    let count = stack_get_repeat_count(&stack, id) + 1;
                    let lower = reg.repeat_range[id].lower;
                    let upper = reg.repeat_range[id].upper;
                    let body_start = reg.repeat_range[id].u_offset as usize;

                    // C order for greedy: branch first, then push count
                    if upper != INFINITE_REPEAT && count >= upper {
                        p += 1;
                    } else if count >= lower {
                        p += 1;
                        stack.push(StackEntry::Alt { pcode: p, pstr: s, zid: -1, is_super: false });
                        p = body_start;
                    } else {
                        p = body_start;
                    }
                    // Count pushed AFTER Alt — gets popped on backtrack (correct for greedy)
                    stack.push(StackEntry::RepeatInc { zid: id, count });
                } else {
                    goto_fail = true;
                }
            }

            OpCode::RepeatIncNg => {
                if let OperationPayload::RepeatInc { id } = reg.ops[p].payload {
                    let id = id as usize;
                    let count = stack_get_repeat_count(&stack, id) + 1;
                    let lower = reg.repeat_range[id].lower;
                    let upper = reg.repeat_range[id].upper;
                    let body_start = reg.repeat_range[id].u_offset as usize;

                    // C order for non-greedy: push count FIRST, then branch
                    // Count pushed BEFORE Alt — survives backtrack (correct for lazy)
                    stack.push(StackEntry::RepeatInc { zid: id, count });

                    if upper != INFINITE_REPEAT && count as i32 == upper {
                        p += 1;
                    } else if count >= lower {
                        stack.push(StackEntry::Alt { pcode: body_start, pstr: s, zid: -1, is_super: false });
                        p += 1;
                    } else {
                        p = body_start;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_EMPTY_CHECK_START / END - detect empty match in loops
            // ================================================================
            OpCode::EmptyCheckStart => {
                if let OperationPayload::EmptyCheckStart { mem } = reg.ops[p].payload {
                    let mem = mem as usize;
                    stack.push(StackEntry::EmptyCheckStart {
                        zid: mem,
                        pstr: s,
                    });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EmptyCheckEnd => {
                if let OperationPayload::EmptyCheckEnd { mem, .. } = reg.ops[p].payload {
                    let mem = mem as usize;
                    let is_empty = stack_empty_check(&stack, mem, s);
                    p += 1;
                    if is_empty {
                        // Empty loop detected — skip the next instruction
                        // (JUMP, PUSH, REPEAT_INC, or REPEAT_INC_NG) to break the loop.
                        // Mirrors C: empty_check_found: INC_OP;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EmptyCheckEndMemst | OpCode::EmptyCheckEndMemstPush => {
                if let OperationPayload::EmptyCheckEnd { mem, empty_status_mem } = reg.ops[p].payload {
                    let mem = mem as usize;
                    let is_empty = stack_empty_check_mem(&stack, mem, s, empty_status_mem as u32, reg,
                                                         &mem_start_stk, &mem_end_stk);
                    p += 1;
                    if is_empty {
                        // Truly empty → skip next op (JUMP back)
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_MOVE - move string position
            // ================================================================
            OpCode::Move => {
                if let OperationPayload::Move { n } = reg.ops[p].payload {
                    if n < 0 {
                        // Step back n characters (encoding-aware)
                        match onigenc_step_back(enc, 0, s, str_data, (-n) as usize) {
                            Some(new_s) => { s = new_s; p += 1; }
                            None => { goto_fail = true; }
                        }
                    } else {
                        // Step forward n characters
                        match onigenc_step(enc, s, end, str_data, n as usize) {
                            Some(new_s) => { s = new_s; p += 1; }
                            None => { goto_fail = true; }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_STEP_BACK_START / NEXT - lookbehind support
            // ================================================================
            OpCode::StepBackStart => {
                if let OperationPayload::StepBackStart {
                    initial,
                    remaining,
                    addr,
                } = reg.ops[p].payload
                {
                    let initial = initial as usize;
                    // Step back 'initial' characters (encoding-aware)
                    if initial != 0 {
                        match onigenc_step_back(enc, 0, s, str_data, initial) {
                            Some(new_s) => { s = new_s; }
                            None => { goto_fail = true; }
                        }
                    }
                    if !goto_fail {
                        if remaining != 0 {
                            // Variable-length: push Alt with remaining count, jump to addr
                            stack.push(StackEntry::Alt {
                                pcode: p + 1,
                                pstr: s,
                                zid: remaining,
                                is_super: false,
                            });
                            p = (p as i32 + addr as i32) as usize;
                        } else {
                            p += 1;
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::StepBackNext => {
                // last_alt_zid was set by the backtrack that jumped here
                let mut remaining = last_alt_zid;
                if remaining != INFINITE_LEN as i32 {
                    remaining -= 1;
                }
                match onigenc_step_back(enc, 0, s, str_data, 1) {
                    Some(new_s) => { s = new_s; }
                    None => { goto_fail = true; }
                }
                if !goto_fail {
                    if remaining != 0 {
                        stack.push(StackEntry::Alt {
                            pcode: p,
                            pstr: s,
                            zid: remaining,
                            is_super: false,
                        });
                    }
                    p += 1;
                }
            }

            // ================================================================
            // OP_MARK - push a named checkpoint
            // ================================================================
            OpCode::Mark => {
                if let OperationPayload::Mark { id, save_pos } = reg.ops[p].payload {
                    let id = id as usize;
                    let pos = if save_pos { Some(s) } else { None };
                    stack.push(StackEntry::Mark { zid: id, pos });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_CUT_TO_MARK - void entries to mark, optionally restore position
            // C always uses STACK_TO_VOID_TO_MARK (not POP_TO_MARK)
            // ================================================================
            OpCode::CutToMark => {
                if let OperationPayload::CutToMark { id, restore_pos } = reg.ops[p].payload {
                    let id = id as usize;
                    let saved_pos = stack_void_to_mark(&mut stack, id);
                    if restore_pos {
                        if let Some(pos) = saved_pos {
                            s = pos;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_SAVE_VAL - save a value on the stack
            // ================================================================
            OpCode::SaveVal => {
                if let OperationPayload::SaveVal { save_type, id } = reg.ops[p].payload {
                    let id = id as usize;
                    let v = match save_type {
                        SaveType::Keep => s,
                        SaveType::S => s,
                        SaveType::RightRange => right_range,
                    };
                    stack.push(StackEntry::SaveVal {
                        zid: id,
                        save_type,
                        v,
                    });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_UPDATE_VAR - update a variable from the stack
            // ================================================================
            OpCode::UpdateVar => {
                if let OperationPayload::UpdateVar {
                    var_type, id, clear,
                } = reg.ops[p].payload
                {
                    let id = id as usize;
                    match var_type {
                        UpdateVarType::KeepFromStackLast => {
                            if let Some(v) =
                                stack_get_save_val_type_last(&stack, SaveType::Keep)
                            {
                                keep = v;
                            }
                        }
                        UpdateVarType::SFromStack => {
                            if let Some(v) =
                                stack_get_save_val_last(&stack, SaveType::S, id)
                            {
                                s = v;
                            }
                        }
                        UpdateVarType::RightRangeFromStack => {
                            if let Some(v) =
                                stack_get_save_val_last(&stack, SaveType::RightRange, id)
                            {
                                right_range = v;
                            }
                        }
                        UpdateVarType::RightRangeFromSStack => {
                            if let Some(v) =
                                stack_get_save_val_last(&stack, SaveType::S, id)
                            {
                                right_range = v;
                            }
                        }
                        UpdateVarType::RightRangeToS => {
                            right_range = s;
                        }
                        UpdateVarType::RightRangeInit => {
                            right_range = in_right_range;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_CALL / OP_RETURN - subroutine call/return
            // ================================================================
            OpCode::Call => {
                if let OperationPayload::Call { addr } = reg.ops[p].payload {
                    let addr = addr as usize;
                    stack.push(StackEntry::CallFrame { ret_addr: p + 1 });
                    p = addr;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Return => {
                // Search backwards for CallFrame, skipping nested Return markers.
                // Each STK_RETURN increments the level; each STK_CALL_FRAME decrements.
                let mut level = 0i32;
                let mut ret_addr = None;
                for i in (0..stack.len()).rev() {
                    match &stack[i] {
                        StackEntry::CallFrame { ret_addr: ra } => {
                            if level == 0 {
                                ret_addr = Some(*ra);
                                break;
                            }
                            level -= 1;
                        }
                        StackEntry::Return => {
                            level += 1;
                        }
                        _ => {}
                    }
                }
                if let Some(ra) = ret_addr {
                    stack.push(StackEntry::Return);
                    p = ra;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Callout opcodes
            // ================================================================
            OpCode::CalloutContents => {
                // Callout of contents: always succeeds (we don't execute user code)
                let num = match &reg.ops[p].payload {
                    OperationPayload::CalloutContents { num } => *num,
                    _ => 0,
                };
                if let Some(ref ext) = reg.extp {
                    if num >= 1 && (num as usize) <= ext.callout_list.len() {
                        let entry = &ext.callout_list[(num - 1) as usize];
                        if (entry.callout_in & CALLOUT_IN_RETRACTION) != 0 {
                            stack.push(StackEntry::Callout {
                                num,
                                id: ONIG_NON_NAME_ID,
                            });
                        }
                    }
                }
                p += 1;
            }
            OpCode::CalloutName => {
                let (num, id) = match &reg.ops[p].payload {
                    OperationPayload::CalloutName { num, id } => (*num, *id),
                    _ => (0, 0),
                };
                let call_result = run_builtin_callout(reg, num, id, false, &mut callout_data);
                if call_result == ONIG_CALLOUT_FAIL {
                    goto_fail = true;
                } else {
                    // Push retraction entry if needed
                    if let Some(ref ext) = reg.extp {
                        if num >= 1 && (num as usize) <= ext.callout_list.len() {
                            let entry = &ext.callout_list[(num - 1) as usize];
                            if (entry.callout_in & CALLOUT_IN_RETRACTION) != 0 {
                                stack.push(StackEntry::Callout { num, id });
                            }
                        }
                    }
                    p += 1;
                }
            }
        }

        // Handle failure (backtracking)
        if goto_fail {
            // Retry limit check
            retry_in_match_counter += 1;
            if retry_limit_in_match != 0 && retry_in_match_counter > retry_limit_in_match {
                best_len = ONIGERR_RETRY_LIMIT_IN_MATCH_OVER;
                break;
            }
            // Time limit check (every CHECK_TIME_INTERVAL retries)
            if time_limit_ms > 0 && (retry_in_match_counter % CHECK_TIME_INTERVAL) == 0 {
                if msa.check_time_limit() {
                    best_len = ONIGERR_TIME_LIMIT_OVER;
                    break;
                }
            }

            match stack_pop(
                &mut stack,
                pop_level,
                &mut mem_start_stk,
                &mut mem_end_stk,
                reg,
                &mut callout_data,
            ) {
                Some((pcode, pstr, alt_zid)) => {
                    if pcode == FINISH_PCODE {
                        // Hit bottom sentinel - no more alternatives
                        break;
                    }
                    p = pcode;
                    s = pstr;
                    last_alt_zid = alt_zid;
                }
                None => {
                    // Stack empty - match failed
                    break;
                }
            }
        }
    }

    // Accumulate retry counter into search counter
    msa.retry_limit_in_search_counter += retry_in_match_counter;
    best_len
}

// ============================================================================
// onig_match - match at a specific position (port of C's onig_match)
// ============================================================================

/// Try to match the regex at exactly position `at` in the string.
/// Returns the match length on success, ONIG_MISMATCH (-1) on failure.
///
/// Parameters:
/// - reg: compiled regex
/// - str_data: the input string bytes
/// - end: end position (typically str_data.len())
/// - at: position to try matching at
/// - region: optional region to fill with capture group positions
/// - option: match options
pub fn onig_match(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    at: usize,
    region: Option<OnigRegion>,
    option: OnigOptionType,
) -> (i32, Option<OnigRegion>) {
    let mut msa = MatchArg::new(reg, option, region, at);

    if let Some(ref mut r) = msa.region {
        r.resize(reg.num_mem + 1);
        r.clear();
    }

    let result = match_at(reg, str_data, end, end, at, &mut msa);

    // Handle FIND_LONGEST
    let result = if opton_find_longest(msa.options) && result == ONIG_MISMATCH {
        if msa.best_len >= 0 {
            msa.best_len
        } else {
            result
        }
    } else {
        result
    };

    (result, msa.region)
}

// ============================================================================
// onig_search - search for a match anywhere in the string
// ============================================================================

/// Search for the regex pattern in the string, trying each position from
/// `start` to `range`.
/// Returns the match position on success, ONIG_MISMATCH (-1) on failure.
///
/// Parameters:
/// - reg: compiled regex
// ============================================================================
// Search optimization functions — mirrors C's regexec.c lines 5168-5645
// ============================================================================

/// Naive string search. Mirrors C's slow_search.
fn slow_search(enc: OnigEncoding, target: &[u8], text: &[u8],
               text_start: usize, text_end: usize, text_range: usize) -> Option<usize> {
    if target.is_empty() { return Some(text_start); }
    let tlen = target.len();
    let mut limit = text_end.saturating_sub(tlen - 1);
    if limit > text_range { limit = text_range; }
    let mut s = text_start;
    while s < limit {
        if text[s] == target[0] {
            let mut ok = true;
            for i in 1..tlen {
                if s + i >= text_end || text[s + i] != target[i] {
                    ok = false;
                    break;
                }
            }
            if ok { return Some(s); }
        }
        s += enclen(enc, text, s);
    }
    None
}

/// Sunday quick search (BMH variant). Mirrors C's sunday_quick_search.
fn sunday_quick_search(reg: &RegexType, target: &[u8], text: &[u8],
                       text_start: usize, text_end: usize, text_range: usize) -> Option<usize> {
    let map_offset = reg.map_offset as usize;
    let tlen = target.len();
    if tlen == 0 { return Some(text_start); }
    let tail_idx = tlen - 1;

    let end = if tlen > text_end.saturating_sub(text_range) {
        if tlen > text_end.saturating_sub(text_start) {
            return None;
        }
        text_end
    } else {
        text_range + tlen
    };

    let mut s = text_start + tail_idx;
    while s < end {
        let mut p = s;
        let mut t = tail_idx;
        loop {
            if text[p] != target[t] { break; }
            if t == 0 { return Some(p); }
            p -= 1;
            t -= 1;
        }
        if s + map_offset >= text_end { break; }
        s += reg.map[text[s + map_offset] as usize] as usize;
    }
    None
}

/// Sunday quick search with step forward for multi-byte safe operation.
/// Mirrors C's sunday_quick_search_step_forward.
fn sunday_quick_search_step_forward(reg: &RegexType, target: &[u8], text: &[u8],
                                     text_start: usize, text_end: usize,
                                     text_range: usize) -> Option<usize> {
    let enc = reg.enc;
    let tlen = target.len();
    if tlen == 0 { return Some(text_start); }
    let tail_idx = tlen - 1;
    let mut end = text_range;
    if tail_idx as usize > text_end.saturating_sub(end) {
        end = text_end.saturating_sub(tail_idx);
    }

    let map_offset = reg.map_offset as usize;
    let mut s = text_start;
    while s < end {
        let se = s + tail_idx;
        let mut p = se;
        let mut t = tail_idx;
        loop {
            if text[p] != target[t] { break; }
            if t == 0 { return Some(s); }
            p -= 1;
            t -= 1;
        }
        if se + map_offset >= text_end { break; }
        let skip = reg.map[text[se + map_offset] as usize] as usize;
        let next = s + skip;
        if next < end {
            s = onigenc_get_right_adjust_char_head(enc, text, s, next);
        } else {
            break;
        }
    }
    None
}

/// Character map search. Mirrors C's map_search.
fn map_search(enc: OnigEncoding, map: &[u8; CHAR_MAP_SIZE], text: &[u8],
              text_start: usize, text_range: usize) -> Option<usize> {
    let mut s = text_start;
    while s < text_range {
        if map[text[s] as usize] != 0 { return Some(s); }
        s += enclen(enc, text, s);
    }
    None
}

/// Get right-adjusted char head for multi-byte encoding.
fn onigenc_get_right_adjust_char_head(enc: OnigEncoding, text: &[u8],
                                       start: usize, s: usize) -> usize {
    let mut pos = s;
    // For UTF-8: if pos lands on a continuation byte, advance to next char start
    while pos < text.len() {
        let len = enc.mbc_enc_len(&text[pos..]);
        if len > 0 { return pos; }
        pos += 1;
    }
    pos
}

/// Forward search using optimization strategy.
/// Returns Some((low, high)) if a candidate was found, None otherwise.
fn forward_search(reg: &RegexType, str_data: &[u8], end: usize,
                  start: usize, range: usize) -> Option<(usize, usize)> {
    let mut p = start;
    if reg.dist_min != 0 {
        if end.saturating_sub(p) <= reg.dist_min as usize {
            return None;
        }
        if enc_is_singlebyte(reg.enc) {
            p += reg.dist_min as usize;
        } else {
            let target = p + reg.dist_min as usize;
            while p < target && p < end {
                p += enclen(reg.enc, str_data, p);
            }
        }
    }

    let mut pprev: Option<usize> = None;
    loop {
        // Search for the optimization target
        let found = match reg.optimize {
            OptimizeType::Str => {
                slow_search(reg.enc, &reg.exact, str_data, p, end, range)
            }
            OptimizeType::StrFast => {
                sunday_quick_search(reg, &reg.exact, str_data, p, end, range)
            }
            OptimizeType::StrFastStepForward => {
                sunday_quick_search_step_forward(reg, &reg.exact, str_data, p, end, range)
            }
            OptimizeType::Map => {
                map_search(reg.enc, &reg.map, str_data, p, range)
            }
            OptimizeType::None => { return None; }
        };

        p = match found {
            Some(pos) if pos < range => pos,
            _ => return None,
        };

        if p.saturating_sub(start) < reg.dist_min as usize {
            pprev = Some(p);
            p += enclen(reg.enc, str_data, p);
            continue; // retry
        }

        // Validate sub_anchor
        if reg.sub_anchor != 0 {
            let mut retry = false;
            if (reg.sub_anchor & ANCR_BEGIN_LINE) != 0 {
                if p > 0 {
                    let prev = pprev.unwrap_or(0);
                    let prev_pos = onigenc_get_prev_char_head(reg.enc, str_data, prev, p);
                    if !is_mbc_newline(reg.enc, str_data, prev_pos, end) {
                        retry = true;
                    }
                }
            }
            if !retry && (reg.sub_anchor & ANCR_END_LINE) != 0 {
                if p >= end {
                    // at end - OK for some cases
                } else if !is_mbc_newline(reg.enc, str_data, p, end) {
                    retry = true;
                }
            }
            if retry {
                pprev = Some(p);
                p += enclen(reg.enc, str_data, p);
                continue; // retry
            }
        }

        // Calculate low/high range
        let (low, high);
        if reg.dist_max == 0 {
            low = p;
            high = p;
        } else {
            if reg.dist_max != INFINITE_LEN {
                if p.saturating_sub(0) < reg.dist_max as usize {
                    low = 0;
                } else {
                    let mut l = p - reg.dist_max as usize;
                    if l > start {
                        l = onigenc_get_right_adjust_char_head(reg.enc, str_data, start, l);
                    }
                    low = l;
                }
            } else {
                low = start; // infinite dist_max: any position from start
            }
            if p.saturating_sub(0) < reg.dist_min as usize {
                high = 0;
            } else {
                high = p - reg.dist_min as usize;
            }
        }
        return Some((low, high));
    }
}

/// Check if encoding is single-byte.
fn enc_is_singlebyte(enc: OnigEncoding) -> bool {
    enc.max_enc_len() == 1
}

/// Get previous character head position.
fn onigenc_get_prev_char_head(_enc: OnigEncoding, _text: &[u8], _start: usize, pos: usize) -> usize {
    if pos == 0 { return 0; }
    // For UTF-8: walk backwards past continuation bytes
    let mut p = pos - 1;
    while p > 0 && _text[p] & 0xC0 == 0x80 {
        p -= 1;
    }
    p
}

/// Check if position is a newline.
fn is_mbc_newline(_enc: OnigEncoding, text: &[u8], pos: usize, end: usize) -> bool {
    if pos >= end { return false; }
    text[pos] == b'\n'
}

/// - str_data: the input string bytes
/// - end: end position (typically str_data.len())
/// - start: starting search position
/// - range: search range end (exclusive for forward search)
/// - region: optional region to fill with capture group positions
/// - option: match options
pub fn onig_search(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    region: Option<OnigRegion>,
    option: OnigOptionType,
) -> (i32, Option<OnigRegion>) {
    let mut msa = MatchArg::new(reg, option, region, start);
    let enc = reg.enc;

    if start > range {
        // Backward search - not yet optimized
        return (ONIG_MISMATCH, msa.region);
    }

    let find_longest = opton_find_longest(msa.options);
    let mut best_start: i32 = ONIG_MISMATCH;
    let mut best_len: i32 = ONIG_MISMATCH;

    let mut cur_start = start;
    let mut cur_range = range;
    let data_range = if range > start { range } else { end };

    // === Anchor optimization: narrow search range ===
    if reg.anchor != 0 && start < end {
        if (reg.anchor & ANCR_BEGIN_POSITION) != 0 {
            // search start-position only
            if range > start {
                cur_range = start + 1;
            } else {
                cur_range = start;
            }
        } else if (reg.anchor & ANCR_BEGIN_BUF) != 0 {
            // search str-position only (must start at 0)
            if range > start {
                if start != 0 {
                    return (ONIG_MISMATCH, msa.region);
                }
                cur_range = 1;
            } else {
                return (ONIG_MISMATCH, msa.region);
            }
        } else if (reg.anchor & ANCR_END_BUF) != 0 {
            let min_semi_end = end;
            let max_semi_end = end;
            if (max_semi_end as OnigLen) < reg.anc_dist_min {
                return (ONIG_MISMATCH, msa.region);
            }
            if range > start {
                if reg.anc_dist_max != INFINITE_LEN
                    && min_semi_end.saturating_sub(start) > reg.anc_dist_max as usize
                {
                    cur_start = min_semi_end - reg.anc_dist_max as usize;
                }
                if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < reg.anc_dist_min as usize {
                    if max_semi_end + 1 < reg.anc_dist_min as usize {
                        return (ONIG_MISMATCH, msa.region);
                    } else {
                        cur_range = max_semi_end - reg.anc_dist_min as usize + 1;
                    }
                }
                if cur_start > cur_range {
                    return (ONIG_MISMATCH, msa.region);
                }
            }
        } else if (reg.anchor & ANCR_SEMI_END_BUF) != 0 {
            let mut min_semi_end = end;
            let max_semi_end = end;
            // Check if last char before end is newline
            if end > 0 && str_data[end - 1] == b'\n' {
                min_semi_end = end - 1;
            }
            if (max_semi_end as OnigLen) < reg.anc_dist_min {
                return (ONIG_MISMATCH, msa.region);
            }
            if range > start {
                if reg.anc_dist_max != INFINITE_LEN
                    && min_semi_end.saturating_sub(start) > reg.anc_dist_max as usize
                {
                    cur_start = min_semi_end - reg.anc_dist_max as usize;
                }
                if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < reg.anc_dist_min as usize {
                    if max_semi_end + 1 < reg.anc_dist_min as usize {
                        return (ONIG_MISMATCH, msa.region);
                    } else {
                        cur_range = max_semi_end - reg.anc_dist_min as usize + 1;
                    }
                }
                if cur_start > cur_range {
                    return (ONIG_MISMATCH, msa.region);
                }
            }
        } else if (reg.anchor & ANCR_ANYCHAR_INF_ML) != 0 && range > start {
            // start-position only
            if range > start {
                cur_range = start + 1;
            }
        }
    } else if start == end {
        // Empty string
        if reg.threshold_len == 0 {
            let mut s = start;
            if let Some(ref mut r) = msa.region {
                r.resize(reg.num_mem + 1);
                r.clear();
            }
            msa.best_len = ONIG_MISMATCH;
            msa.best_s = 0;
            let r = match_at(reg, str_data, end, end, s, &mut msa);
            if r < ONIG_MISMATCH { return (r, msa.region); } // error
            if r != ONIG_MISMATCH {
                return (s as i32, msa.region);
            }
        }
        return (ONIG_MISMATCH, msa.region);
    }

    // === Threshold length check ===
    if (end as i32 - cur_start as i32) < reg.threshold_len {
        return (ONIG_MISMATCH, msa.region);
    }

    // === Forward search ===
    let mut s = cur_start;

    // Use optimization if available
    if reg.optimize != OptimizeType::None {
        // Calculate search range for optimization
        let sch_range = if reg.dist_max != 0 {
            if reg.dist_max == INFINITE_LEN {
                end
            } else if end.saturating_sub(cur_range) < reg.dist_max as usize {
                end
            } else {
                cur_range + reg.dist_max as usize
            }
        } else {
            cur_range
        };

        if reg.dist_max != INFINITE_LEN {
            // Finite dist_max: iterate with forward_search
            loop {
                let (low, high) = match forward_search(reg, str_data, end, s, sch_range) {
                    Some(lh) => lh,
                    None => break, // mismatch
                };
                if s < low { s = low; }
                while s <= high {
                    if let Some(ref mut r) = msa.region {
                        r.resize(reg.num_mem + 1);
                        r.clear();
                    }
                    msa.best_len = ONIG_MISMATCH;
                    msa.best_s = 0;
                    let r = match_at(reg, str_data, end, data_range, s, &mut msa);
                    if r != ONIG_MISMATCH {
                        if r < 0 { return (r, msa.region); } // error
                        if find_longest {
                            let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                            if best_len == ONIG_MISMATCH || match_len > best_len {
                                best_start = s as i32;
                                best_len = match_len;
                            }
                        } else {
                            return (s as i32, msa.region);
                        }
                    }
                    if msa.retry_limit_in_search != 0
                        && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
                    {
                        return (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, msa.region);
                    }
                    s += enclen(enc, str_data, s);
                }
                if s >= cur_range { break; }
            }
        } else {
            // Infinite dist_max: just check once, then fall through to normal loop
            if forward_search(reg, str_data, end, s, sch_range).is_none() {
                return finish_search(find_longest, best_start, best_len, reg,
                                    str_data, end, &mut msa);
            }
            // ANCR_ANYCHAR_INF: skip past newlines
            if (reg.anchor & ANCR_ANYCHAR_INF) != 0
                && (reg.anchor & (ANCR_LOOK_BEHIND | ANCR_PREC_READ_NOT)) == 0
            {
                while s < cur_range {
                    if let Some(ref mut r) = msa.region {
                        r.resize(reg.num_mem + 1);
                        r.clear();
                    }
                    msa.best_len = ONIG_MISMATCH;
                    msa.best_s = 0;
                    let r = match_at(reg, str_data, end, data_range, s, &mut msa);
                    if r != ONIG_MISMATCH {
                        if r < 0 { return (r, msa.region); }
                        if find_longest {
                            let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                            if best_len == ONIG_MISMATCH || match_len > best_len {
                                best_start = s as i32;
                                best_len = match_len;
                            }
                        } else {
                            return (s as i32, msa.region);
                        }
                    }
                    if msa.retry_limit_in_search != 0
                        && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
                    {
                        return (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, msa.region);
                    }
                    let prev = s;
                    s += enclen(enc, str_data, s);
                    // Skip past non-newline chars
                    while s < cur_range && !is_mbc_newline(enc, str_data, prev, end) {
                        let prev2 = s;
                        s += enclen(enc, str_data, s);
                        if is_mbc_newline(enc, str_data, prev2, end) { break; }
                    }
                }
                return finish_search(find_longest, best_start, best_len, reg,
                                    str_data, end, &mut msa);
            }
            // Fall through to normal position loop below
        }
    }

    // Normal position-by-position search (no optimization or fallthrough)
    if best_start == ONIG_MISMATCH {
        loop {
            if let Some(ref mut r) = msa.region {
                r.resize(reg.num_mem + 1);
                r.clear();
            }
            msa.best_len = ONIG_MISMATCH;
            msa.best_s = 0;
            let r = match_at(reg, str_data, end, data_range, s, &mut msa);
            if r != ONIG_MISMATCH {
                if r < 0 { return (r, msa.region); }
                if find_longest {
                    let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                    if best_len == ONIG_MISMATCH || match_len > best_len {
                        best_start = s as i32;
                        best_len = match_len;
                    }
                } else {
                    return (s as i32, msa.region);
                }
            }
            if msa.retry_limit_in_search != 0
                && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
            {
                return (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, msa.region);
            }
            if s >= cur_range { break; }
            if s >= end { break; }
            s += enclen(enc, str_data, s);
        }
    }

    finish_search(find_longest, best_start, best_len, reg, str_data, end, &mut msa)
}

fn finish_search(
    find_longest: bool, best_start: i32, best_len: i32,
    reg: &RegexType, str_data: &[u8], end: usize, msa: &mut MatchArg,
) -> (i32, Option<OnigRegion>) {
    if find_longest && best_start != ONIG_MISMATCH {
        if let Some(ref mut r) = msa.region {
            r.resize(reg.num_mem + 1);
            r.clear();
        }
        msa.best_len = ONIG_MISMATCH;
        msa.best_s = 0;
        match_at(reg, str_data, end, end, best_start as usize, msa);
        return (best_start, msa.region.take());
    }
    (ONIG_MISMATCH, msa.region.take())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regcomp;
    use crate::regparse;
    use crate::regparse_types::ParseEnv;
    use crate::regsyntax;

    fn make_test_context() -> (RegexType, ParseEnv) {
        use crate::regsyntax::OnigSyntaxOniguruma;
        let enc: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
        let reg = RegexType {
            ops: Vec::new(),
            string_pool: Vec::new(),
            num_mem: 0,
            num_repeat: 0,
            num_empty_check: 0,
            num_call: 0,
            capture_history: 0,
            push_mem_start: 0,
            push_mem_end: 0,
            stack_pop_level: StackPopLevel::Free,
            repeat_range: Vec::new(),
            enc,
            options: ONIG_OPTION_NONE,
            syntax: &OnigSyntaxOniguruma as *const OnigSyntaxType,
            case_fold_flag: ONIGENC_CASE_FOLD_MIN,
            name_table: None,
            optimize: OptimizeType::None,
            threshold_len: 0,
            anchor: 0,
            anc_dist_min: 0,
            anc_dist_max: 0,
            sub_anchor: 0,
            exact: Vec::new(),
            map: [0u8; CHAR_MAP_SIZE],
            map_offset: 0,
            dist_min: 0,
            dist_max: 0,
            called_addrs: vec![],
            unset_call_addrs: vec![],
            extp: None,
        };
        let env = ParseEnv {
            options: 0,
            case_fold_flag: 0,
            enc,
            syntax: &OnigSyntaxOniguruma,
            cap_history: 0,
            backtrack_mem: 0,
            backrefed_mem: 0,
            pattern: std::ptr::null(),
            pattern_end: std::ptr::null(),
            error: std::ptr::null(),
            error_end: std::ptr::null(),
            reg: std::ptr::null_mut(),
            num_call: 0,
            num_mem: 0,
            num_named: 0,
            mem_alloc: 0,
            mem_env_static: Default::default(),
            mem_env_dynamic: None,
            backref_num: 0,
            keep_num: 0,
            id_num: 0,
            save_alloc_num: 0,
            saves: None,
            unset_addr_list: None,
            parse_depth: 0,
            flags: 0,
        };
        (reg, env)
    }

    fn compile_and_match(pattern: &[u8], input: &[u8]) -> (i32, Option<OnigRegion>) {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0, "compile failed for {:?}", std::str::from_utf8(pattern));
        onig_match(&reg, input, input.len(), 0, Some(OnigRegion::new()), ONIG_OPTION_NONE)
    }

    fn compile_and_search(pattern: &[u8], input: &[u8]) -> (i32, Option<OnigRegion>) {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0, "compile failed for {:?}", std::str::from_utf8(pattern));
        onig_search(
            &reg,
            input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        )
    }

    // ---- Basic literal matching ----

    #[test]
    fn match_literal_abc() {
        let (r, _) = compile_and_match(b"abc", b"abc");
        assert_eq!(r, 3); // matched 3 bytes
    }

    #[test]
    fn match_literal_fail() {
        let (r, _) = compile_and_match(b"abc", b"abd");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_literal_too_short() {
        let (r, _) = compile_and_match(b"abcd", b"abc");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_empty_pattern() {
        let (r, _) = compile_and_match(b"", b"abc");
        assert_eq!(r, 0); // empty pattern matches with length 0
    }

    #[test]
    fn match_single_char() {
        let (r, _) = compile_and_match(b"x", b"xyz");
        assert_eq!(r, 1);
    }


    // ---- Dot (anychar) ----

    #[test]
    fn match_dot() {
        let (r, _) = compile_and_match(b"a.c", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_dot_no_newline() {
        let (r, _) = compile_and_match(b"a.c", b"a\nc");
        assert_eq!(r, ONIG_MISMATCH); // dot doesn't match newline
    }

    // ---- Alternation ----

    #[test]
    fn match_alternation_first() {
        let (r, _) = compile_and_match(b"a|b", b"a");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_alternation_second() {
        let (r, _) = compile_and_match(b"a|b", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_alternation_fail() {
        let (r, _) = compile_and_match(b"a|b", b"c");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Quantifiers ----

    #[test]
    fn match_star() {
        let (r, _) = compile_and_match(b"a*", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_star_empty() {
        let (r, _) = compile_and_match(b"a*", b"bbb");
        assert_eq!(r, 0); // a* can match empty
    }

    #[test]
    fn match_plus() {
        let (r, _) = compile_and_match(b"a+", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_plus_fail() {
        let (r, _) = compile_and_match(b"a+", b"bbb");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_question() {
        let (r, _) = compile_and_match(b"a?b", b"ab");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_question_without() {
        let (r, _) = compile_and_match(b"a?b", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_lazy_star() {
        // a*? should match as few as possible from position 0
        let (r, _) = compile_and_match(b"a*?", b"aaa");
        assert_eq!(r, 0); // lazy: match empty
    }

    // ---- Character classes ----

    #[test]
    fn match_char_class() {
        let (r, _) = compile_and_match(b"[abc]", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_fail() {
        let (r, _) = compile_and_match(b"[abc]", b"d");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_char_class_range() {
        let (r, _) = compile_and_match(b"[a-z]", b"m");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_negated() {
        let (r, _) = compile_and_match(b"[^abc]", b"d");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_negated_fail() {
        let (r, _) = compile_and_match(b"[^abc]", b"a");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Anchors ----

    #[test]
    fn match_begin_anchor() {
        let (r, _) = compile_and_match(b"^abc", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_end_anchor() {
        let (r, _) = compile_and_match(b"abc$", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_begin_end_anchors() {
        let (r, _) = compile_and_match(b"^abc$", b"abc");
        assert_eq!(r, 3);
    }

    // ---- Capture groups ----

    #[test]
    fn match_capture_group() {
        let (r, region) = compile_and_match(b"(abc)", b"abc");
        assert_eq!(r, 3);
        let region = region.unwrap();
        assert!(region.num_regs >= 2);
        assert_eq!(region.beg[0], 0);
        assert_eq!(region.end[0], 3);
        assert_eq!(region.beg[1], 0);
        assert_eq!(region.end[1], 3);
    }

    #[test]
    fn match_multiple_captures() {
        let (r, region) = compile_and_match(b"(a)(b)(c)", b"abc");
        assert_eq!(r, 3);
        let region = region.unwrap();
        assert!(region.num_regs >= 4);
        // Group 0: full match
        assert_eq!(region.beg[0], 0);
        assert_eq!(region.end[0], 3);
        // Group 1: a
        assert_eq!(region.beg[1], 0);
        assert_eq!(region.end[1], 1);
        // Group 2: b
        assert_eq!(region.beg[2], 1);
        assert_eq!(region.end[2], 2);
        // Group 3: c
        assert_eq!(region.beg[3], 2);
        assert_eq!(region.end[3], 3);
    }

    #[test]
    fn match_non_capturing_group() {
        let (r, _) = compile_and_match(b"(?:abc)", b"abc");
        assert_eq!(r, 3);
    }

    // ---- Search (find anywhere in string) ----

    #[test]
    fn search_literal() {
        let (pos, _) = compile_and_search(b"bc", b"abcdef");
        assert_eq!(pos, 1); // found at position 1
    }

    #[test]
    fn search_literal_not_found() {
        let (pos, _) = compile_and_search(b"xyz", b"abcdef");
        assert_eq!(pos, ONIG_MISMATCH);
    }

    #[test]
    fn search_at_start() {
        let (pos, _) = compile_and_search(b"abc", b"abcdef");
        assert_eq!(pos, 0);
    }

    #[test]
    fn search_at_end() {
        let (pos, _) = compile_and_search(b"ef", b"abcdef");
        assert_eq!(pos, 4);
    }

    #[test]
    fn search_with_captures() {
        let (pos, region) = compile_and_search(b"(b)(c)", b"abcdef");
        assert_eq!(pos, 1);
        let region = region.unwrap();
        assert_eq!(region.beg[1], 1);
        assert_eq!(region.end[1], 2);
        assert_eq!(region.beg[2], 2);
        assert_eq!(region.end[2], 3);
    }

    #[test]
    fn search_with_quantifier() {
        let (pos, _) = compile_and_search(b"a+", b"bbaab");
        assert_eq!(pos, 2);
    }

    // ---- Complex patterns ----

    #[test]
    fn match_word_boundary() {
        let (pos, _) = compile_and_search(b"\\bfoo\\b", b"a foo b");
        assert_eq!(pos, 2);
    }

    #[test]
    fn match_complex_alternation() {
        let (r, _) = compile_and_match(b"abc|def|ghi", b"def");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_nested_groups() {
        let (r, region) = compile_and_match(b"((a)(b))", b"ab");
        assert_eq!(r, 2);
        let region = region.unwrap();
        // Group 1: ab
        assert_eq!(region.beg[1], 0);
        assert_eq!(region.end[1], 2);
        // Group 2: a
        assert_eq!(region.beg[2], 0);
        assert_eq!(region.end[2], 1);
        // Group 3: b
        assert_eq!(region.beg[3], 1);
        assert_eq!(region.end[3], 2);
    }

    #[test]
    fn match_dot_star() {
        let (r, _) = compile_and_match(b"a.*b", b"aXXXb");
        assert_eq!(r, 5);
    }

    #[test]
    fn match_backtracking() {
        // Pattern: a.*b matches "axb" - requires backtracking
        let (r, _) = compile_and_match(b"a.*b", b"axb");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_quantifier() {
        let (r, _) = compile_and_match(b"a{3}", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_quantifier_fail() {
        let (r, _) = compile_and_match(b"a{3}", b"aa");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_interval_range() {
        let (r, _) = compile_and_match(b"a{2,4}", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_range_min() {
        let (r, _) = compile_and_match(b"a{2,4}", b"aa");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_digit_class() {
        let (r, _) = compile_and_match(b"\\d+", b"123");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_word_class() {
        let (r, _) = compile_and_match(b"\\w+", b"abc123_");
        assert_eq!(r, 7);
    }

    #[test]
    fn search_email_like() {
        let (pos, _) = compile_and_search(b"\\w+@\\w+", b"send to user@host ok");
        assert_eq!(pos, 8); // "user@host" starts at position 8
    }

    #[test]
    fn match_escaped_special() {
        let (r, _) = compile_and_match(b"a\\.b", b"a.b");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_back_reference() {
        let (r, _) = compile_and_match(b"(a)\\1", b"aa");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_back_reference_fail() {
        let (r, _) = compile_and_match(b"(a)\\1", b"ab");
        assert_eq!(r, ONIG_MISMATCH);
    }
}
