//! Rust-only (ADR-008): the bytes a path through compiled bytecode can
//! consume first.
//!
//! RegSet uses the map from the start of a pattern as a start filter for its
//! fallback searches.

use crate::regint::*;

/// Derive a conservative byte map for the first consuming instruction from
/// compiled bytecode. Returning `None` is deliberate: any control-flow shape
/// we cannot prove safe stays on the optimizer-event fallback.
pub(crate) fn derive_start_byte_map(reg: &RegexType) -> Option<[u8; CHAR_MAP_SIZE]> {
    first_byte_map_from(reg, 0, None, 0)
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
) -> Option<[u8; CHAR_MAP_SIZE]> {
    /// Nested look-aheads whose bodies are derived before giving up.
    const MAX_LOOKAHEAD_DEPTH: u32 = 4;

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
    let mut pending = vec![entry];
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
            OpCode::CClassNotStar => {
                let OperationPayload::CClass { bsp, .. } = &op.payload else {
                    return None;
                };
                add_bitset(&mut map, bsp, true);
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::CClassMbNotStar => {
                add_all(&mut map);
                pending.push(pc + 1);
                saw_consumer = true;
            }
            OpCode::CClassMixNotStar => {
                let OperationPayload::CClassMix { bsp, .. } = &op.payload else {
                    return None;
                };
                add_bitset(&mut map, bsp, true);
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
                if trie.is_case_insensitive() {
                    // Case folding also admits non-ASCII input (`K` for `k`,
                    // `ß` for `ss`), including malformed sequences a class
                    // decodes; any lead byte may start a match.
                    for byte in 0x80..=0xFF {
                        add_exact(&mut map, byte);
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
