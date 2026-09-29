//! Bounded decimal-prefix scanning and native capture restoration entries.

use crate::regexec::{MemPtr, StackEntry};
use crate::regint::*;

// Rust-only, opt-in decimal lowering (ADR-008). The helpers stay outside the
// general VM: experiment 5 measured 2–3% extra TypeScript scanner time and about
// 10% extra ordinary decimal time when this body was inside match_at_impl.
// Keeping both helpers non-inlined is part of this measured dispatch contract.
#[inline(never)]
pub(crate) fn decimal_tail_prefix_bounds(
    str_data: &[u8],
    right_range: usize,
    start: usize,
) -> (usize, Option<(usize, usize)>) {
    let mut cursor = start;
    let mut previous = None;
    while cursor < right_range && str_data[cursor].is_ascii_digit() {
        let begin = cursor;
        while cursor < right_range && str_data[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor + 1 < right_range
            && str_data[cursor] == b'_'
            && str_data[cursor + 1].is_ascii_digit()
        {
            previous = Some((begin, cursor + 1));
            cursor += 1;
            continue;
        }
        // Preserve the first greedy success: reserve one final digit. A
        // one-digit last segment leaves the previous iteration's capture.
        let prefix_end = cursor - 1;
        let last = if prefix_end > begin {
            Some((begin, prefix_end))
        } else {
            previous
        };
        return (prefix_end, last);
    }
    // Zero prefix iterations must leave old captures untouched. The original
    // final run still performs its own success/failure and capture operations.
    (start, None)
}

#[inline(never)]
pub(crate) fn record_decimal_prefix_captures(
    reg: &RegexType,
    captures: &[MemNumType],
    (begin, finish): (usize, usize),
    stack: &mut Vec<StackEntry>,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
) {
    for &num in captures {
        let num = num as usize;
        if mem_status_at(reg.push_mem_start, num) {
            let si = stack.len();
            stack.push(StackEntry::MemStart {
                zid: num,
                pstr: begin,
                prev_start: mem_start_stk[num],
                prev_end: mem_end_stk[num],
            });
            mem_start_stk[num] = MemPtr::stack_idx(si);
            mem_end_stk[num] = MemPtr::invalid();
        } else {
            mem_start_stk[num] = MemPtr::pos(begin);
        }
    }
    for &num in captures.iter().rev() {
        let num = num as usize;
        if mem_status_at(reg.push_mem_end, num) {
            let si = stack.len();
            stack.push(StackEntry::MemEnd {
                zid: num,
                pstr: finish,
                prev_start: mem_start_stk[num],
                prev_end: mem_end_stk[num],
            });
            mem_end_stk[num] = MemPtr::stack_idx(si);
        } else {
            mem_end_stk[num] = MemPtr::pos(finish);
        }
    }
}
