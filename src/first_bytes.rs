//! Rust-only (ADR-008): the bytes a path through compiled bytecode can
//! consume first.
//!
//! RegSet uses the map from the start of a pattern as a start filter for its
//! fallback searches. The compiler uses the map from inside a pattern to
//! guard backtrack pushes (`PushOrJumpByteSet`).

use crate::regint::*;

/// The bytes a path may consume first, one bit per byte.
type ByteMap = BitSet;

fn add_all(map: &mut ByteMap) {
    map.fill(!0);
}

fn add_exact(map: &mut ByteMap, byte: u8) {
    bitset_set_bit(map, byte as usize);
}

fn add_bitset(map: &mut ByteMap, bitset: &BitSet, inverted: bool) {
    for (word, &bits) in map.iter_mut().zip(bitset) {
        *word |= if inverted { !bits } else { bits };
    }
}

/// Every byte from 0x80 on: the lead and continuation bytes of multibyte
/// characters.
fn add_high_bytes(map: &mut ByteMap) {
    map[0x80 / BITS_IN_ROOM..].fill(!0);
}

/// `[0-9A-Za-z_]`, or its complement.
fn add_ascii_word(map: &mut ByteMap, inverted: bool) {
    let mut word = [0; BITSET_REAL_SIZE];
    for byte in (0..=0x7f_u8).filter(|&b| b.is_ascii_alphanumeric() || b == b'_') {
        bitset_set_bit(&mut word, byte as usize);
    }
    add_bitset(map, &word, inverted);
}

fn target(pc: usize, addr: RelAddrType, len: usize) -> Option<usize> {
    let target = (pc as i64).checked_add(addr as i64)?;
    (target >= 0 && (target as usize) < len).then_some(target as usize)
}

/// What an instruction contributes to a first-byte map.
enum FirstBytes {
    /// Consumes nothing; control flow decides what follows.
    None,
    /// Consumes one of the recorded bytes.
    Consumes,
    /// A loop that consumes recorded bytes or nothing, then falls through.
    ConsumesOrFallsThrough,
}

/// Records the bytes `op` can consume first. `None` when the payload does not
/// fit the opcode.
fn record_first_bytes(reg: &RegexType, op: &Operation, map: &mut ByteMap) -> Option<FirstBytes> {
    Some(match op.opcode {
        OpCode::Str1 | OpCode::Str2 | OpCode::Str3 | OpCode::Str4 | OpCode::Str5 => {
            let OperationPayload::Exact { s } = &op.payload else {
                return None;
            };
            add_exact(map, s[0]);
            FirstBytes::Consumes
        }
        OpCode::StrN => {
            let OperationPayload::ExactN { s, .. } = &op.payload else {
                return None;
            };
            add_exact(map, *s.first()?);
            FirstBytes::Consumes
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
            add_exact(map, *s.first()?);
            FirstBytes::Consumes
        }
        OpCode::CClass | OpCode::CClassNot => {
            let OperationPayload::CClass { bsp, .. } = &op.payload else {
                return None;
            };
            add_bitset(map, bsp, op.opcode == OpCode::CClassNot);
            FirstBytes::Consumes
        }
        OpCode::CClassMb => {
            add_high_bytes(map);
            FirstBytes::Consumes
        }
        OpCode::CClassMbNot => {
            add_all(map);
            FirstBytes::Consumes
        }
        OpCode::CClassMix | OpCode::CClassMixNot => {
            let OperationPayload::CClassMix { bsp, .. } = &op.payload else {
                return None;
            };
            add_bitset(map, bsp, op.opcode == OpCode::CClassMixNot);
            add_high_bytes(map);
            FirstBytes::Consumes
        }
        OpCode::Word | OpCode::NoWord | OpCode::AnyChar | OpCode::AnyCharMl => {
            add_all(map);
            FirstBytes::Consumes
        }
        OpCode::WordAscii | OpCode::NoWordAscii => {
            add_ascii_word(map, op.opcode == OpCode::NoWordAscii);
            FirstBytes::Consumes
        }
        OpCode::CClassStar => {
            let OperationPayload::CClass { bsp, .. } = &op.payload else {
                return None;
            };
            add_bitset(map, bsp, false);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::CClassMixStar => {
            let OperationPayload::CClassMix { bsp, .. } = &op.payload else {
                return None;
            };
            add_bitset(map, bsp, false);
            add_high_bytes(map);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::CClassMbStar => {
            add_high_bytes(map);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::CClassNotStar => {
            let OperationPayload::CClass { bsp, .. } = &op.payload else {
                return None;
            };
            add_bitset(map, bsp, true);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::CClassMbNotStar => {
            add_all(map);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::CClassMixNotStar => {
            let OperationPayload::CClassMix { bsp, .. } = &op.payload else {
                return None;
            };
            add_bitset(map, bsp, true);
            add_high_bytes(map);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::WordStar => {
            add_all(map);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::WordAsciiStar => {
            add_ascii_word(map, false);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::CClassStarPeekNext => {
            let OperationPayload::CClassStarPeekNext { bsp, .. } = &op.payload else {
                return None;
            };
            add_bitset(map, bsp, false);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::WordAsciiStarPeekNext => {
            add_ascii_word(map, false);
            FirstBytes::ConsumesOrFallsThrough
        }
        OpCode::AltLiterals => {
            let OperationPayload::AltLiterals { trie_idx } = op.payload else {
                return None;
            };
            let trie = reg.literal_tries.get(trie_idx as usize)?;
            for byte in trie.first_bytes()? {
                add_exact(map, byte);
                if trie.is_case_insensitive() && byte.is_ascii_alphabetic() {
                    add_exact(map, byte.to_ascii_lowercase());
                    add_exact(map, byte.to_ascii_uppercase());
                }
            }
            if trie.is_case_insensitive() {
                // Case folding also admits non-ASCII input (`K` for `k`,
                // `ß` for `ss`), including malformed sequences a class
                // decodes; any lead byte may start a match.
                add_high_bytes(map);
            }
            FirstBytes::Consumes
        }
        OpCode::AnyCharStar
        | OpCode::AnyCharMlStar
        | OpCode::AnyCharStarPeekNext
        | OpCode::AnyCharMlStarPeekNext => {
            add_all(map);
            FirstBytes::Consumes
        }
        _ => FirstBytes::None,
    })
}

/// Whether the `Push` at `pc` opens a negative look-around whose failure
/// continues at `alt`.
fn is_negative_assertion_push(reg: &RegexType, pc: usize, alt: usize) -> bool {
    if alt <= pc + 1 || reg.ops.get(alt.wrapping_sub(1)).map(|op| op.opcode) != Some(OpCode::Fail) {
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
        reg.ops[pc + 1..alt].iter().any(
            |op| matches!(op.payload, OperationPayload::PopToMark { id: pop_id } if pop_id == id),
        )
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

/// Instructions a guard walk visits before giving up; every push derives a
/// guard, so the walk stays local.
const MAX_STEPS: usize = 64;

/// Whether the push-like `op` at `pc` opens a negative look-around the guard
/// walk skips: it only decides whether the path goes on at `alt`, at the same
/// position, and nothing in it outlives the look-around.
fn skips_negative_look_around(reg: &RegexType, pc: usize, op: &Operation, alt: usize) -> bool {
    op.opcode != OpCode::PushIfPeekNext
        && (pc + 1..=pc + MAX_STEPS).contains(&alt)
        && is_negative_assertion_push(reg, pc, alt)
        && reg.ops[pc + 1..alt]
            .iter()
            .all(|op| ends_with_the_look_around(reg, op))
}

/// Scratch buffers for [`guard_byte_map`], reused across a pattern's pushes.
#[derive(Default)]
pub(crate) struct GuardWalk {
    pending: Vec<usize>,
    visited: Vec<usize>,
}

/// The bytes the main path of a push can consume first, when it starts at
/// `entry`. A push can be skipped when the current byte is not among them.
///
/// Unlike the start map, this walk begins inside the pattern, where
/// look-arounds, captures and counters may already be in progress. It only
/// passes instructions whose effects backtracking undoes. A byte outside the
/// map therefore means that every path from `entry` fails before it moves
/// the position, changes state that backtracking keeps, or reshapes the
/// stack.
pub(crate) fn guard_byte_map(
    reg: &RegexType,
    entry: usize,
    walk: &mut GuardWalk,
) -> Option<ByteMap> {
    let mut map: ByteMap = [0; BITSET_REAL_SIZE];
    let mut saw_consumer = false;
    walk.pending.clear();
    walk.visited.clear();
    walk.pending.push(entry);

    while let Some(pc) = walk.pending.pop() {
        if walk.visited.contains(&pc) {
            continue;
        }
        if walk.visited.len() == MAX_STEPS {
            return None;
        }
        walk.visited.push(pc);
        let op = reg.ops.get(pc)?;

        match record_first_bytes(reg, op, &mut map)? {
            FirstBytes::Consumes => {
                saw_consumer = true;
                continue;
            }
            FirstBytes::ConsumesOrFallsThrough => {
                saw_consumer = true;
                walk.pending.push(pc + 1);
                continue;
            }
            FirstBytes::None => {}
        }

        match (op.opcode, &op.payload) {
            (OpCode::Jump, OperationPayload::Jump { addr }) => {
                walk.pending.push(target(pc, *addr, reg.ops.len())?);
            }
            (
                OpCode::Push
                | OpCode::PushOrJumpByteSet
                | OpCode::PushOrJumpExact1
                | OpCode::PushIfPeekNext,
                OperationPayload::Push { addr }
                | OperationPayload::PushOrJumpByteSet { addr, .. }
                | OperationPayload::PushOrJumpExact1 { addr, .. }
                | OperationPayload::PushIfPeekNext { addr, .. },
            ) => {
                let alt = target(pc, *addr, reg.ops.len())?;
                walk.pending.push(alt);
                // A negative look-around only decides whether the path goes
                // on at `alt`, at the same position. Skipping it is exact if
                // nothing in it outlives the look-around; otherwise its body
                // is walked like any other branch, which only adds bytes.
                if !skips_negative_look_around(reg, pc, op, alt) {
                    walk.pending.push(pc + 1);
                }
            }
            // Pushes a mark entry that backtracking pops.
            (OpCode::Mark, _) => walk.pending.push(pc + 1),
            // Cutting to a mark this path pushed only drops what the path
            // pushed since, and restores a position it has not consumed
            // from. A mark from before `entry` would drop the push itself.
            (OpCode::CutToMark, &OperationPayload::CutToMark { id, .. }) => {
                let pushed_on_path = walk.visited.iter().any(|&at| {
                    (entry..pc).contains(&at)
                        && matches!(
                            reg.ops[at].payload,
                            OperationPayload::Mark { id: mark, .. } if mark == id
                        )
                });
                if !pushed_on_path {
                    return None;
                }
                walk.pending.push(pc + 1);
            }
            // A look-behind consumes nothing; its body instruction follows.
            (OpCode::LookBehindOp, _) => walk.pending.push(pc + 2),
            // Checks, and stack entries that backtracking pops again.
            (
                OpCode::MemStartPush
                | OpCode::MemEndPush
                | OpCode::EmptyCheckStart
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
                | OpCode::CheckPosition,
                _,
            ) => walk.pending.push(pc + 1),
            (OpCode::Fail, _) => {}
            _ => return None,
        }
    }

    saw_consumer.then_some(map)
}

/// The backtracks an unguarded push takes when the current byte is outside
/// `map`, the guard byte map of its main path starting at `entry`. Every path
/// from `entry` then fails without consuming, and each failure is one
/// backtrack: a guard that jumps instead counts them itself, so that the retry
/// limits trip where they do without the guard (and in C).
///
/// Runs the main path on a model of the backtracking stack: string and class
/// instructions fail, pushes and marks stack up, cuts drop what their mark
/// covers. Checks and fused look-behinds depend on the position, so every
/// combination of their outcomes is run, and the count must not depend on
/// them. `None` when it does, or for instructions the model does not cover;
/// such a push stays unguarded.
pub(crate) fn guard_skipped_retries(reg: &RegexType, entry: usize, map: &ByteMap) -> Option<u32> {
    /// Instructions one run may execute.
    const MAX_RUN_STEPS: usize = 4096;
    /// Checks whose outcomes are combined.
    const MAX_CHECKS: usize = 6;

    enum Entry {
        Alt(usize),
        Mark(MemNumType),
    }

    /// The outcome of the check at `at` in the run `passes` (bit i for
    /// `checks[i]`); `None` past `MAX_CHECKS`.
    fn check(at: usize, passes: u32, checks: &mut Vec<usize>) -> Option<bool> {
        let index = match checks.iter().position(|&c| c == at) {
            Some(index) => index,
            None => {
                checks.push(at);
                checks.len() - 1
            }
        };
        (index < MAX_CHECKS).then_some(passes & (1 << index) != 0)
    }

    // One run with the check outcomes in `passes`.
    let run = |passes: u32, checks: &mut Vec<usize>| -> Option<u32> {
        let mut stack: Vec<Entry> = Vec::new();
        let mut pc = entry;
        let mut failures = 0u32;
        let mut scratch: ByteMap = [0; BITSET_REAL_SIZE];
        for _ in 0..MAX_RUN_STEPS {
            let op = reg.ops.get(pc)?;
            let next = match record_first_bytes(reg, op, &mut scratch)? {
                // Cannot consume the current byte.
                FirstBytes::Consumes => None,
                // Consumes nothing and pushes nothing.
                FirstBytes::ConsumesOrFallsThrough => Some(pc + 1),
                FirstBytes::None => match (op.opcode, &op.payload) {
                    (OpCode::Jump, OperationPayload::Jump { addr }) => {
                        Some(target(pc, *addr, reg.ops.len())?)
                    }
                    // Upstream's instruction: the current byte is not `c`,
                    // so it jumps without pushing.
                    (
                        OpCode::PushOrJumpExact1,
                        &OperationPayload::PushOrJumpExact1 {
                            addr,
                            c,
                            skipped_retries: 0,
                        },
                    ) => {
                        if !bitset_at(map, c as usize) {
                            return None;
                        }
                        Some(target(pc, addr, reg.ops.len())?)
                    }
                    // Pushes only when the current byte is `c`.
                    (OpCode::PushIfPeekNext, &OperationPayload::PushIfPeekNext { c, .. }) => {
                        if !bitset_at(map, c as usize) {
                            return None;
                        }
                        Some(pc + 1)
                    }
                    // A plain push, or a guard that jumps and counts what
                    // the push takes.
                    (
                        OpCode::Push | OpCode::PushOrJumpByteSet | OpCode::PushOrJumpExact1,
                        OperationPayload::Push { addr }
                        | OperationPayload::PushOrJumpByteSet { addr, .. }
                        | OperationPayload::PushOrJumpExact1 { addr, .. },
                    ) => {
                        let alt = target(pc, *addr, reg.ops.len())?;
                        if skips_negative_look_around(reg, pc, op, alt) {
                            return None;
                        }
                        stack.push(Entry::Alt(alt));
                        Some(pc + 1)
                    }
                    (OpCode::Mark, &OperationPayload::Mark { id, .. }) => {
                        stack.push(Entry::Mark(id));
                        Some(pc + 1)
                    }
                    // Drops what the path pushed since its mark; a mark from
                    // before `entry` would drop the push itself.
                    (OpCode::CutToMark, &OperationPayload::CutToMark { id, .. }) => {
                        loop {
                            if let Entry::Mark(mark) = stack.pop()? {
                                if mark == id {
                                    break;
                                }
                            }
                        }
                        Some(pc + 1)
                    }
                    (OpCode::MemStartPush | OpCode::MemEndPush | OpCode::EmptyCheckStart, _) => {
                        Some(pc + 1)
                    }
                    (
                        OpCode::WordBoundary
                        | OpCode::NoWordBoundary
                        | OpCode::WordBegin
                        | OpCode::WordEnd
                        | OpCode::TextSegmentBoundary
                        | OpCode::BeginBuf
                        | OpCode::EndBuf
                        | OpCode::BeginLine
                        | OpCode::EndLine
                        | OpCode::SemiEndBuf
                        | OpCode::CheckPosition,
                        _,
                    ) => check(pc, passes, checks)?.then_some(pc + 1),
                    (OpCode::LookBehindOp, _) => check(pc, passes, checks)?.then_some(pc + 2),
                    (OpCode::Fail, _) => None,
                    _ => return None,
                },
            };
            pc = match next {
                Some(next) => next,
                None => {
                    failures = failures.checked_add(1)?;
                    loop {
                        match stack.pop() {
                            // The push's own alternative: the main path is done.
                            None => return Some(failures),
                            Some(Entry::Alt(alt)) => break alt,
                            Some(Entry::Mark(_)) => {}
                        }
                    }
                }
            };
        }
        None
    };

    let mut checks = Vec::new();
    let mut count = None;
    let mut passes = 0u32;
    loop {
        let failures = run(passes, &mut checks)?;
        if count.is_some_and(|count| count != failures) {
            return None;
        }
        count = Some(failures);
        passes += 1;
        if checks.len() > MAX_CHECKS {
            return None;
        }
        if passes >= 1 << checks.len() {
            return count;
        }
    }
}

/// Whether `op`, inside a negative look-around, leaves no trace once the
/// look-around has decided: it consumes, checks, moves the position, or
/// pushes and pops stack entries. Captures without push, counters, variables
/// and callouts would outlive it.
fn ends_with_the_look_around(reg: &RegexType, op: &Operation) -> bool {
    matches!(
        op.opcode,
        OpCode::Jump
            | OpCode::Push
            | OpCode::PushOrJumpByteSet
            | OpCode::PushOrJumpExact1
            | OpCode::PushIfPeekNext
            | OpCode::Pop
            | OpCode::Mark
            | OpCode::PopToMark
            | OpCode::CutToMark
            | OpCode::Fail
            | OpCode::LookBehindOp
            | OpCode::StepBackStart
            | OpCode::StepBackNext
            | OpCode::MemStartPush
            | OpCode::MemEndPush
            | OpCode::EmptyCheckStart
            | OpCode::EmptyCheckEnd
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
    ) || {
        let mut scratch: ByteMap = [0; BITSET_REAL_SIZE];
        matches!(
            record_first_bytes(reg, op, &mut scratch),
            Some(FirstBytes::Consumes | FirstBytes::ConsumesOrFallsThrough)
        )
    }
}

/// Derive a conservative byte map for the first consuming instruction from
/// compiled bytecode. Returning `None` is deliberate: any control-flow shape
/// we cannot prove safe stays on the optimizer-event fallback.
pub(crate) fn derive_start_byte_map(reg: &RegexType) -> Option<[u8; CHAR_MAP_SIZE]> {
    let bits = first_byte_map_from(reg, 0, None, 0)?;
    let mut map = [0; CHAR_MAP_SIZE];
    for byte in bitset_members(&bits) {
        map[byte] = 1;
    }
    Some(map)
}

/// The bytes a match can start with when the VM starts at `entry`.
///
/// With `lookahead` set, `entry` is the body of that positive look-ahead and
/// reaching its closing `CutToMark` means the body matched empty, so no byte
/// is fixed. `None` whenever some path can match without consuming input.
fn first_byte_map_from(
    reg: &RegexType,
    entry: usize,
    lookahead: Option<MemNumType>,
    depth: u32,
) -> Option<ByteMap> {
    /// Nested look-aheads whose bodies are derived before giving up.
    const MAX_LOOKAHEAD_DEPTH: u32 = 4;

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

    let mut map: ByteMap = [0; BITSET_REAL_SIZE];
    let mut pending = vec![entry];
    let mut visited = vec![false; reg.ops.len()];
    let mut saw_consumer = false;

    while let Some(pc) = pending.pop() {
        if pc >= reg.ops.len() || visited[pc] {
            continue;
        }
        visited[pc] = true;
        let op = &reg.ops[pc];

        match record_first_bytes(reg, op, &mut map)? {
            FirstBytes::Consumes => {
                saw_consumer = true;
                continue;
            }
            FirstBytes::ConsumesOrFallsThrough => {
                saw_consumer = true;
                pending.push(pc + 1);
                continue;
            }
            FirstBytes::None => {}
        }

        match op.opcode {
            OpCode::Jump => {
                let OperationPayload::Jump { addr } = op.payload else {
                    return None;
                };
                pending.push(target(pc, addr, reg.ops.len())?);
            }
            // A guarded push may open a negative look-around as well.
            OpCode::Push
            | OpCode::PushSuper
            | OpCode::PushOrJumpByteSet
            | OpCode::PushOrJumpExact1 => {
                let (OperationPayload::Push { addr }
                | OperationPayload::PushOrJumpByteSet { addr, .. }
                | OperationPayload::PushOrJumpExact1 { addr, .. }) = op.payload
                else {
                    return None;
                };
                let alt = target(pc, addr, reg.ops.len())?;
                pending.push(alt);
                if !is_negative_assertion_push(reg, pc, alt) {
                    pending.push(pc + 1);
                }
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
                    let continuation = mark_continuation(reg, pc, id)?;
                    let lookbehind =
                        reg.ops.get(pc + 1).map(|op| op.opcode) == Some(OpCode::StepBackStart);
                    // A positive look-ahead whose body cannot match empty must
                    // consume the byte at the start position, so its body's
                    // first bytes bound this path like a consuming instruction.
                    let body =
                        (continuation != pc + 1 && !lookbehind && depth < MAX_LOOKAHEAD_DEPTH)
                            .then(|| first_byte_map_from(reg, pc + 1, Some(id), depth + 1))
                            .flatten();
                    match body {
                        Some(body) => {
                            for (value, &in_body) in map.iter_mut().zip(&body) {
                                *value |= in_body;
                            }
                            saw_consumer = true;
                        }
                        None => pending.push(continuation),
                    }
                } else {
                    pending.push(pc + 1);
                }
            }
            // A look-behind consumes nothing; its body instruction follows.
            OpCode::LookBehindOp => pending.push(pc + 2),
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
            | OpCode::SaveVal
            | OpCode::UpdateVar
            | OpCode::CalloutContents
            | OpCode::CalloutName => pending.push(pc + 1),
            OpCode::CutToMark => {
                let OperationPayload::CutToMark { id, .. } = op.payload else {
                    return None;
                };
                if lookahead == Some(id) {
                    // The look-ahead body matched without consuming input.
                    return None;
                }
                pending.push(pc + 1);
            }
            OpCode::Fail => {}
            OpCode::Finish | OpCode::End => return None,
            _ => return None,
        }
    }

    saw_consumer.then_some(map)
}
