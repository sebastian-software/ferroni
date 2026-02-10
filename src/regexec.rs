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

use crate::oniguruma::*;
use crate::regenc::*;
use crate::regint::*;

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
        pcode: usize, // bytecode index to jump to on backtrack
        pstr: usize,  // string position to restore
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
}

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
        }
    }
}

// ============================================================================
// Stack operations (port of STACK_PUSH_* / STACK_POP macros)
// ============================================================================

/// Pop stack entries until an ALT (choice point) is found.
/// Restores mem_start_stk/mem_end_stk as needed based on pop_level.
/// Returns Some((pcode, pstr)) from the ALT entry, or None if stack is empty.
fn stack_pop(
    stack: &mut Vec<StackEntry>,
    pop_level: StackPopLevel,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
) -> Option<(usize, usize)> {
    loop {
        let entry = stack.pop()?;
        match entry {
            StackEntry::Alt { pcode, pstr } => {
                return Some((pcode, pstr));
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
                        // RepeatInc, EmptyCheckStart, CallFrame, Return:
                        // handled implicitly (popping removes them)
                        _ => {}
                    }
                }
            },
        }
    }
}

/// Pop stack entries until a Mark with matching zid is found.
/// Restores mem_start_stk/mem_end_stk along the way.
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

/// Get the saved value for a given save_type and zid from the stack.
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

// ============================================================================
// Helper functions
// ============================================================================

/// Check if a byte is a word character (ASCII).
#[inline]
fn is_word_ascii(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Check word boundary at position s.
/// Returns true if there's a word boundary between s-1 and s.
fn is_word_boundary(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    let at_start = s == 0;
    let at_end = s >= end;

    if at_start && at_end {
        return false;
    }
    if at_start {
        return is_word_ascii(str_data[s]);
    }
    if at_end {
        return is_word_ascii(str_data[s - 1]);
    }

    let prev_word = is_word_ascii(str_data[s - 1]);
    let curr_word = is_word_ascii(str_data[s]);
    prev_word != curr_word
}

/// Check if position s is at the start of a word.
fn is_word_begin(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    if s >= end {
        return false;
    }
    let curr_word = is_word_ascii(str_data[s]);
    if !curr_word {
        return false;
    }
    if s == 0 {
        return true;
    }
    !is_word_ascii(str_data[s - 1])
}

/// Check if position s is at the end of a word.
fn is_word_end(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    if s == 0 {
        return false;
    }
    let prev_word = is_word_ascii(str_data[s - 1]);
    if !prev_word {
        return false;
    }
    if s >= end {
        return true;
    }
    !is_word_ascii(str_data[s])
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

    // Push bottom sentinel (like C's STACK_PUSH_BOTTOM with FinishCode)
    stack.push(StackEntry::Alt {
        pcode: FINISH_PCODE,
        pstr: 0,
    });

    // ---- Main dispatch loop ----
    loop {
        if p >= reg.ops.len() {
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
                    } else {
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
                // For now, treat as ExactN
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
                } else if let OperationPayload::ExactLenN { s: ref exact, n, len } =
                    reg.ops[p].payload
                {
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
            OpCode::CClassMb | OpCode::CClassMbNot => {
                // TODO: implement multibyte character class matching
                // For now, fail
                goto_fail = true;
            }

            // Mixed character class (single-byte bitset + multibyte ranges)
            OpCode::CClassMix | OpCode::CClassMixNot => {
                let not = opcode == OpCode::CClassMixNot;
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if let OperationPayload::CClassMix { ref bsp, .. } = reg.ops[p].payload
                {
                    let c = str_data[s];
                    let in_class = if c < 128 {
                        bitset_at(bsp, c as usize)
                    } else {
                        // TODO: check multibyte ranges
                        false
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
                    stack.push(StackEntry::Alt { pcode: p + 1, pstr: s });
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
                    stack.push(StackEntry::Alt { pcode: p + 1, pstr: s });
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
                            stack.push(StackEntry::Alt { pcode: p + 1, pstr: s });
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
                            stack.push(StackEntry::Alt { pcode: p + 1, pstr: s });
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
            OpCode::Word | OpCode::WordAscii => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if !is_word_ascii(str_data[s]) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            OpCode::NoWord | OpCode::NoWordAscii => {
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
                // TODO: text segment boundary
                p += 1;
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

            // Case-insensitive and multi backrefs (TODO: full implementation)
            OpCode::BackRefNIc
            | OpCode::BackRefMulti
            | OpCode::BackRefMultiIc
            | OpCode::BackRefWithLevel
            | OpCode::BackRefWithLevelIc
            | OpCode::BackRefCheck
            | OpCode::BackRefCheckWithLevel => {
                // TODO: implement these variants
                goto_fail = true;
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
                // TODO: recursive capture end
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

            OpCode::MemEndRec => {
                // TODO: recursive capture end (non-push)
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let num = num as usize;
                    mem_end_stk[num] = MemPtr::Pos(s);
                    // Also push end mark for stack scanning
                    stack.push(StackEntry::MemEndMark { zid: num });
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
                    stack.push(StackEntry::Alt {
                        pcode: alt_target,
                        pstr: s,
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
                            });
                        } else {
                            // Non-greedy: push body as alternative, try skip first
                            stack.push(StackEntry::Alt {
                                pcode: p + 1,
                                pstr: s,
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
            OpCode::RepeatInc | OpCode::RepeatIncNg => {
                if let OperationPayload::RepeatInc { id } = reg.ops[p].payload {
                    let id = id as usize;
                    let count = stack_get_repeat_count(&stack, id) + 1;
                    let lower = reg.repeat_range[id].lower;
                    let upper = reg.repeat_range[id].upper;
                    let body_start = reg.repeat_range[id].u_offset as usize;

                    if upper != INFINITE_REPEAT && count >= upper {
                        // Reached maximum, exit loop
                        p += 1;
                    } else if count >= lower {
                        // In valid range, can exit or continue
                        if opcode == OpCode::RepeatInc {
                            // Greedy: push exit as alternative, continue body
                            p += 1;
                            stack.push(StackEntry::Alt { pcode: p, pstr: s });
                            p = body_start;
                        } else {
                            // Non-greedy: push body as alternative, exit
                            stack.push(StackEntry::Alt {
                                pcode: body_start,
                                pstr: s,
                            });
                            p += 1;
                        }
                    } else {
                        // Below minimum, must continue body
                        p = body_start;
                    }

                    // Push updated count
                    stack.push(StackEntry::RepeatInc { zid: id, count });
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

            OpCode::EmptyCheckEnd | OpCode::EmptyCheckEndMemst | OpCode::EmptyCheckEndMemstPush => {
                if let OperationPayload::EmptyCheckEnd { mem, .. } = reg.ops[p].payload {
                    let mem = mem as usize;
                    let is_empty = stack_empty_check(&stack, mem, s);
                    p += 1;
                    if is_empty {
                        // Empty match detected - skip the next instruction
                        // to break the infinite loop
                        goto_fail = true;
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
                    s = (s as i32 + n) as usize;
                    p += 1;
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
                    // Step back 'initial' characters
                    if s < initial {
                        goto_fail = true;
                    } else {
                        // For ASCII/single-byte, just subtract
                        s -= initial;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::StepBackNext => {
                p += 1;
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
            // OP_CUT_TO_MARK - pop to mark, optionally restore position
            // ================================================================
            OpCode::CutToMark => {
                if let OperationPayload::CutToMark { id, restore_pos } = reg.ops[p].payload {
                    let id = id as usize;
                    let saved_pos =
                        stack_pop_to_mark(&mut stack, id, &mut mem_start_stk, &mut mem_end_stk);
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
                                stack_get_save_val_last(&stack, SaveType::Keep, id)
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
                // Search backwards for CallFrame
                let mut ret_addr = None;
                for i in (0..stack.len()).rev() {
                    if let StackEntry::CallFrame { ret_addr: ra } = &stack[i] {
                        ret_addr = Some(*ra);
                        stack.truncate(i); // Remove call frame and everything above
                        break;
                    }
                }
                if let Some(ra) = ret_addr {
                    p = ra;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Callout opcodes (TODO: not implemented)
            // ================================================================
            OpCode::CalloutContents | OpCode::CalloutName => {
                p += 1; // Skip for now
            }
        }

        // Handle failure (backtracking)
        if goto_fail {
            match stack_pop(
                &mut stack,
                pop_level,
                &mut mem_start_stk,
                &mut mem_end_stk,
            ) {
                Some((pcode, pstr)) => {
                    if pcode == FINISH_PCODE {
                        // Hit bottom sentinel - no more alternatives
                        break;
                    }
                    p = pcode;
                    s = pstr;
                }
                None => {
                    // Stack empty - match failed
                    break;
                }
            }
        }
    }

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
        // TODO: implement backward search
        return (ONIG_MISMATCH, msa.region);
    }

    // Forward search
    let mut s = start;
    while s <= range {
        if let Some(ref mut r) = msa.region {
            r.resize(reg.num_mem + 1);
            r.clear();
        }

        let r = match_at(reg, str_data, end, end, s, &mut msa);
        if r != ONIG_MISMATCH {
            // Found a match - return the match start position
            // Region is already populated by match_at
            return (s as i32, msa.region);
        }

        if s >= end {
            break;
        }

        // Advance to next character position
        s += enclen(enc, str_data, s);
    }

    (ONIG_MISMATCH, msa.region)
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
        let r = regcomp::onig_compile(&root, &mut reg, &env);
        assert_eq!(r, 0, "compile failed for {:?}", std::str::from_utf8(pattern));
        onig_match(&reg, input, input.len(), 0, Some(OnigRegion::new()), ONIG_OPTION_NONE)
    }

    fn compile_and_search(pattern: &[u8], input: &[u8]) -> (i32, Option<OnigRegion>) {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::onig_compile(&root, &mut reg, &env);
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
