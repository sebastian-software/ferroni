//! Structural checks before decoded data reaches the existing VM and planners.
use super::{PrecompiledError, Result};
use crate::literal_trie::CaseFolds;
use crate::regint::*;

fn ensure(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(PrecompiledError::InvalidData)
    }
}
fn target(reg: &RegexType, pc: usize, addr: i32) -> Result<usize> {
    let target = pc as i64 + i64::from(addr);
    ensure(target >= 0 && target < reg.ops.len() as i64)?;
    Ok(target as usize)
}
fn capture(reg: &RegexType, n: i32) -> Result<()> {
    ensure(n >= 0 && n <= reg.num_mem)
}
fn ranges(mb: &[u32]) -> Result<()> {
    let Some(&count) = mb.first() else {
        return ensure(false);
    };
    ensure(count as usize == (mb.len() - 1) / 2 && mb.len() % 2 == 1)?;
    let mut prev = None;
    for pair in mb[1..].as_chunks::<2>().0 {
        ensure(pair[0] <= pair[1] && prev.is_none_or(|p| p < pair[0]))?;
        prev = Some(pair[1]);
    }
    Ok(())
}
fn fast(bsp: &BitSet, kind: CClassAsciiFastKind) -> Result<()> {
    ensure(kind == crate::regcomp::detect_cclass_ascii_fast(bsp))
}

pub(super) fn trie(
    literals: &[Vec<u8>],
    insensitive: bool,
    folds: Option<&CaseFolds>,
) -> Result<()> {
    ensure(!literals.is_empty() && literals.iter().all(|s| !s.is_empty()))?;
    if let Some(folds) = folds {
        ensure(
            insensitive
                && literals.iter().all(|s| s.is_ascii())
                && folds.segments.len() == literals.len(),
        )?;
        ensure(
            folds.class_members.windows(2).all(|p| p[0].0 < p[1].0)
                && folds
                    .class_members
                    .iter()
                    .all(|(code, c)| *code <= 0x10FFFF && c.is_ascii_lowercase()),
        )?;
        ensure(
            folds
                .ligatures
                .iter()
                .all(|(a, b)| !a.is_empty() && !b.is_empty() && b.is_ascii()),
        )?;
        ensure(
            folds
                .accepted
                .iter()
                .all(|a| !a.is_empty() && a.iter().all(|s| !s.is_empty())),
        )?;
        for (literal, segments) in literals.iter().zip(&folds.segments) {
            let mut end = 0;
            for &(start, len, index) in segments {
                ensure(start >= end && len > 0 && index < folds.accepted.len())?;
                end = start
                    .checked_add(len)
                    .ok_or(PrecompiledError::InvalidData)?;
                ensure(end <= literal.len())?;
            }
        }
    }
    Ok(())
}

pub(super) fn program(reg: &RegexType) -> Result<()> {
    if reg.ops.len() > 65_536
        || reg.num_mem > 32_767
        || reg.num_repeat > 65_536
        || reg.num_empty_check > 65_536
        || reg.num_call > 65_536
    {
        return Err(PrecompiledError::ResourceLimit);
    }
    ensure(!reg.ops.is_empty() && reg.ops.last().is_some_and(|o| o.opcode == OpCode::End))?;
    ensure(
        reg.num_mem >= 0 && reg.num_repeat >= 0 && reg.num_empty_check >= 0 && reg.num_call >= 0,
    )?;
    ensure(reg.enc.name() == "UTF-8")?;
    if reg.extp.as_ref().is_some_and(|e| e.callout_num != 0) {
        return Err(PrecompiledError::NonportablePattern);
    }
    ensure(
        reg.repeat_range.len() == reg.num_repeat as usize
            && reg.unset_call_addrs.iter().all(|(pc, g)| {
                *pc < reg.ops.len() && *g >= 0 && (*g as usize) < reg.called_addrs.len()
            }),
    )?;
    ensure(
        reg.threshold_len >= 0
            && reg.map_byte_count <= 3
            && reg.map_offset >= 0
            && reg.map_offset <= OPT_EXACT_MAXLEN as i32
            && reg.dist_min <= reg.dist_max
            && reg.anc_dist_min <= reg.anc_dist_max
            && reg.exact.len() <= OPT_EXACT_MAXLEN,
    )?;
    if matches!(
        reg.optimize,
        OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward
    ) {
        ensure(!reg.exact.is_empty())?;
    }
    ensure(reg.look_behind_reach <= LOOK_BEHIND_MAX_CHAR_LEN as u32)?;
    for &addr in &reg.called_addrs {
        ensure(addr == -1 || (addr >= 0 && (addr as usize) < reg.ops.len()))?;
    }
    if let Some(table) = &reg.name_table {
        for entry in table.entries.values() {
            for &n in &entry.back_refs {
                capture(reg, n)?;
            }
        }
    }
    for range in &reg.repeat_range {
        ensure(
            range.lower >= 0
                && (range.upper == INFINITE_REPEAT || range.upper >= range.lower)
                && range.u_offset >= 0
                && (range.u_offset as usize) < reg.ops.len(),
        )?;
    }
    for (pc, op) in reg.ops.iter().enumerate() {
        use OperationPayload as P;
        // Opcode and payload are independently tagged on the wire. No mismatched
        // pair may fall through one of the VM's payload tests or planners.
        let paired = matches!(
            (&op.opcode, &op.payload),
            (
                OpCode::Str1 | OpCode::Str2 | OpCode::Str3 | OpCode::Str4 | OpCode::Str5,
                P::Exact { .. },
            ) | (OpCode::StrN, P::ExactN { .. })
                | (
                    OpCode::StrMb2n1
                        | OpCode::StrMb2n2
                        | OpCode::StrMb2n3
                        | OpCode::StrMb2n
                        | OpCode::StrMb3n
                        | OpCode::StrMbn,
                    P::ExactLenN { .. },
                )
                | (
                    OpCode::CClass
                        | OpCode::CClassNot
                        | OpCode::CClassStar
                        | OpCode::CClassNotStar
                        | OpCode::CClassPossessiveStar,
                    P::CClass { .. },
                )
                | (OpCode::CClassRun, P::CClassRun { .. })
                | (
                    OpCode::CClassMb
                        | OpCode::CClassMbNot
                        | OpCode::CClassMbStar
                        | OpCode::CClassMbNotStar,
                    P::CClassMb { .. },
                )
                | (
                    OpCode::CClassMix
                        | OpCode::CClassMixNot
                        | OpCode::CClassMixStar
                        | OpCode::CClassMixNotStar,
                    P::CClassMix { .. },
                )
                | (
                    OpCode::AnyCharStarPeekNext | OpCode::AnyCharMlStarPeekNext,
                    P::AnyCharStarPeekNext { .. },
                )
                | (OpCode::CClassStarPeekNext, P::CClassStarPeekNext { .. })
                | (
                    OpCode::WordAsciiStarPeekNext,
                    P::WordAsciiStarPeekNext { .. }
                )
                | (OpCode::CClassRepeat, P::CClassRepeat { .. })
                | (OpCode::AnyCharRepeat, P::AnyCharRepeat { .. })
                | (
                    OpCode::WordBoundary
                        | OpCode::NoWordBoundary
                        | OpCode::WordBegin
                        | OpCode::WordEnd,
                    P::WordBoundary { .. },
                )
                | (OpCode::TextSegmentBoundary, P::TextSegmentBoundary { .. })
                | (OpCode::CheckPosition, P::CheckPosition { .. })
                | (OpCode::BackRefN | OpCode::BackRefNIc, P::BackRefN { .. })
                | (
                    OpCode::BackRefMulti
                        | OpCode::BackRefMultiIc
                        | OpCode::BackRefWithLevel
                        | OpCode::BackRefWithLevelIc
                        | OpCode::BackRefCheck
                        | OpCode::BackRefCheckWithLevel,
                    P::BackRefGeneral { .. },
                )
                | (
                    OpCode::MemStart | OpCode::MemStartPush,
                    P::MemoryStart { .. }
                )
                | (
                    OpCode::MemEnd | OpCode::MemEndPush | OpCode::MemEndRec | OpCode::MemEndPushRec,
                    P::MemoryEnd { .. },
                )
                | (OpCode::Jump, P::Jump { .. })
                | (OpCode::Push | OpCode::PushSuper, P::Push { .. })
                | (OpCode::PushOrJumpExact1, P::PushOrJumpExact1 { .. })
                | (OpCode::PushOrJumpByteSet, P::PushOrJumpByteSet { .. })
                | (OpCode::PushIfPeekNext, P::PushIfPeekNext { .. })
                | (OpCode::PushLazyAnyChar, P::PushLazyAnyChar { .. })
                | (OpCode::PopToMark, P::PopToMark { .. })
                | (OpCode::Repeat | OpCode::RepeatNg, P::Repeat { .. })
                | (OpCode::RepeatInc | OpCode::RepeatIncNg, P::RepeatInc { .. })
                | (OpCode::EmptyCheckStart, P::EmptyCheckStart { .. })
                | (
                    OpCode::EmptyCheckEnd
                        | OpCode::EmptyCheckEndMemst
                        | OpCode::EmptyCheckEndMemstPush,
                    P::EmptyCheckEnd { .. },
                )
                | (OpCode::Move, P::Move { .. })
                | (OpCode::StepBackStart, P::StepBackStart { .. })
                | (OpCode::Mark, P::Mark { .. })
                | (OpCode::CutToMark, P::CutToMark { .. })
                | (OpCode::SaveVal, P::SaveVal { .. })
                | (OpCode::UpdateVar, P::UpdateVar { .. })
                | (OpCode::Call, P::Call { .. })
                | (OpCode::Return, P::Return)
                | (OpCode::AltLiterals, P::AltLiterals { .. })
                | (OpCode::DecimalTailPrefix, P::DecimalTailPrefix { .. })
                | (OpCode::LookBehindOp, P::LookBehindOp { .. })
                | (
                    OpCode::Finish
                        | OpCode::End
                        | OpCode::AnyChar
                        | OpCode::AnyCharMl
                        | OpCode::AnyCharStar
                        | OpCode::AnyCharMlStar
                        | OpCode::Word
                        | OpCode::WordAscii
                        | OpCode::NoWord
                        | OpCode::NoWordAscii
                        | OpCode::BeginBuf
                        | OpCode::EndBuf
                        | OpCode::BeginLine
                        | OpCode::EndLine
                        | OpCode::SemiEndBuf
                        | OpCode::BackRef1
                        | OpCode::BackRef2
                        | OpCode::Fail
                        | OpCode::Pop
                        | OpCode::StepBackNext
                        | OpCode::WordStar
                        | OpCode::WordAsciiStar
                        | OpCode::PopStarChoices
                        | OpCode::GraphemeCluster,
                    P::None,
                )
        );
        ensure(paired)?;
        match &op.payload {
            P::ExactN { s, n } => ensure(*n > 0 && *n as usize == s.len())?,
            P::ExactLenN { s, n, len } => {
                ensure(*n > 0 && *n as usize == s.len() && (1..=6).contains(len))?
            }
            P::CClass { bsp, ascii_fast } => fast(bsp, *ascii_fast)?,
            P::CClassMb { mb } | P::CClassMix { mb, .. } => ranges(mb)?,
            P::CClassRun { bsp, len } => {
                ensure(
                    *len >= 2
                        && bsp[4..].iter().all(|v| *v == 0)
                        && pc + (*len as usize) < reg.ops.len(),
                )?;
                for next in &reg.ops[pc + 1..pc + *len as usize] {
                    ensure(match &next.payload {
                        P::CClass { bsp: b, .. } | P::CClassRun { bsp: b, .. } => b == bsp,
                        _ => false,
                    })?;
                }
            }
            P::CClassRepeat {
                bsp,
                ascii_fast,
                min,
                max,
                ascii_only,
                not,
                ..
            } => {
                fast(bsp, *ascii_fast)?;
                ensure(min <= max)?;
                ensure(!ascii_only || (!not && bsp[4..].iter().all(|v| *v == 0)))?;
            }
            P::AnyCharRepeat { min, max, .. } => ensure(min <= max)?,
            P::WordBoundary { mode } => ensure(*mode == 0 || *mode == 1)?,
            P::MemoryStart { num } | P::MemoryEnd { num } => capture(reg, *num)?,
            P::BackRefN { n1 } => capture(reg, *n1)?,
            P::BackRefGeneral {
                num,
                ns,
                nest_level,
            } => {
                ensure(*num > 0 && *num as usize == ns.len() && *nest_level >= 0)?;
                for &n in ns {
                    capture(reg, n)?;
                }
            }
            P::DecimalTailPrefix { captures } => {
                for &n in captures {
                    capture(reg, n)?;
                }
            }
            P::Jump { addr }
            | P::Push { addr }
            | P::PushIfPeekNext { addr, .. }
            | P::PushLazyAnyChar { addr, .. } => {
                target(reg, pc, *addr)?;
            }
            P::PushOrJumpExact1 {
                addr, skip_gate, ..
            }
            | P::PushOrJumpByteSet {
                addr, skip_gate, ..
            } => {
                target(reg, pc, *addr)?;
                if *addr == -1 {
                    ensure(pc > 0)?;
                }
                if let Some(index) = skip_gate {
                    let gate = reg
                        .skip_gates
                        .get(*index as usize)
                        .ok_or(PrecompiledError::InvalidData)?;
                    ensure(
                        gate.addr > 0 && gate.links >= 2 && gate.links as usize <= reg.ops.len(),
                    )?;
                    target(reg, pc, gate.addr)?;
                }
            }
            P::Repeat { id, addr } => {
                ensure(*id >= 0 && *id < reg.num_repeat)?;
                target(reg, pc, *addr)?;
            }
            P::RepeatInc { id } => ensure(*id >= 0 && *id < reg.num_repeat)?,
            P::EmptyCheckStart { mem } => ensure(*mem >= 0 && *mem < reg.num_empty_check)?,
            P::EmptyCheckEnd { mem, .. } => {
                ensure(*mem >= 0 && *mem < reg.num_empty_check && pc + 2 < reg.ops.len())?
            }
            P::Move { n } => ensure(*n != i32::MIN)?,
            P::StepBackStart {
                initial,
                remaining,
                addr,
            } => {
                ensure(
                    *initial >= 0
                        && *initial <= LOOK_BEHIND_MAX_CHAR_LEN
                        && *remaining >= -1
                        && *remaining <= LOOK_BEHIND_MAX_CHAR_LEN,
                )?;
                if *remaining != 0 {
                    target(reg, pc, *addr)?;
                }
            }
            P::PopToMark { id }
            | P::Mark { id, .. }
            | P::CutToMark { id, .. }
            | P::SaveVal { id, .. }
            | P::UpdateVar { id, .. } => ensure(*id >= 0 && *id <= reg.num_call)?,
            P::Call { addr } => {
                target(reg, 0, *addr)?;
            }
            P::AltLiterals { trie_idx } => ensure((*trie_idx as usize) < reg.literal_tries.len())?,
            P::LookBehindOp {
                char_len,
                ascii_compatible,
                ..
            } => {
                ensure(
                    *char_len <= LOOK_BEHIND_MAX_CHAR_LEN as u32
                        && *ascii_compatible
                        && pc + 2 < reg.ops.len(),
                )?;
                ensure(matches!(
                    reg.ops[pc + 1].opcode,
                    OpCode::Str1
                        | OpCode::Str2
                        | OpCode::Str3
                        | OpCode::Str4
                        | OpCode::Str5
                        | OpCode::StrN
                        | OpCode::StrMb2n1
                        | OpCode::StrMb2n2
                        | OpCode::StrMb2n3
                        | OpCode::StrMb2n
                        | OpCode::StrMb3n
                        | OpCode::StrMbn
                        | OpCode::CClass
                        | OpCode::CClassNot
                        // The optimizer merges a one-byte class body with the
                        // same class after the look-behind (`(?<=[ab])[ab]`).
                        | OpCode::CClassRun
                        | OpCode::CClassMb
                        | OpCode::CClassMbNot
                        | OpCode::CClassMix
                        | OpCode::CClassMixNot
                        | OpCode::Word
                        | OpCode::NoWord
                        | OpCode::WordAscii
                        | OpCode::NoWordAscii
                        | OpCode::AnyChar
                        | OpCode::AnyCharMl
                ))?;
            }
            P::CalloutName { .. } | P::CalloutContents { .. } => {
                return Err(PrecompiledError::NonportablePattern);
            }
            _ => {}
        }
        if op.opcode == OpCode::BackRef1 {
            capture(reg, 1)?;
        }
        if op.opcode == OpCode::BackRef2 {
            capture(reg, 2)?;
        }
    }
    progress_cycles(reg)
}

/// Unconditional jump chains must terminate. Other VM control flow also
/// depends on the input and backtracking stack; structural validation does not
/// try to prove its semantics from an over-approximate control-flow graph.
fn progress_cycles(reg: &RegexType) -> Result<()> {
    let mut done = vec![false; reg.ops.len()];
    let mut visiting = vec![false; reg.ops.len()];
    for root in 0..reg.ops.len() {
        let mut pc = root;
        let mut path = Vec::new();
        while !done[pc] {
            let OperationPayload::Jump { addr } = reg.ops[pc].payload else {
                break;
            };
            ensure(!visiting[pc])?;
            visiting[pc] = true;
            path.push(pc);
            pc = target(reg, pc, addr)?;
        }
        for pc in path {
            done[pc] = true;
            visiting[pc] = false;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn compiled(pattern: &str) -> RegexType {
        let (reg, _) =
            super::super::PatternSettings::of(&super::super::ScannerConfig::default(), false)
                .compile(pattern)
                .unwrap();
        std::sync::Arc::try_unwrap(reg).ok().unwrap()
    }
    #[test]
    fn rejects_invalid_references_lengths_and_nonprogress_cycles() {
        let mut reg = compiled("(a)");
        reg.ops[0].payload = OperationPayload::MemoryStart { num: 10_000 };
        assert_eq!(program(&reg), Err(PrecompiledError::InvalidData));
        let mut reg = compiled("abc");
        reg.ops[0] = Operation {
            opcode: OpCode::Jump,
            payload: OperationPayload::Jump { addr: 0 },
        };
        assert_eq!(program(&reg), Err(PrecompiledError::InvalidData));
        reg.ops[0].payload = OperationPayload::Jump { addr: i32::MAX };
        assert_eq!(program(&reg), Err(PrecompiledError::InvalidData));
        let mut reg = compiled("abcdefgh");
        reg.ops[0].payload = OperationPayload::ExactN {
            s: vec![b'a'],
            n: 8,
        };
        assert_eq!(program(&reg), Err(PrecompiledError::InvalidData));
        let mut reg = compiled("alpha|beta|gamma|delta");
        reg.ops[0].payload = OperationPayload::AltLiterals { trie_idx: u32::MAX };
        assert_eq!(program(&reg), Err(PrecompiledError::InvalidData));
        let mut reg = compiled("a");
        reg.ops[0] = Operation {
            opcode: OpCode::RepeatInc,
            payload: OperationPayload::RepeatInc { id: 0 },
        };
        assert_eq!(program(&reg), Err(PrecompiledError::InvalidData));
        assert_eq!(ranges(&[2, 100, 200]), Err(PrecompiledError::InvalidData));
        assert_eq!(ranges(&[1, 200, 100]), Err(PrecompiledError::InvalidData));
    }
}
