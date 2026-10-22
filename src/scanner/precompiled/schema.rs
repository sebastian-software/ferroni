//! Format v1 instruction and metadata schema. Tags are explicit: changing a
//! layout, opcode meaning, Unicode data or optimizer semantics requires a
//! format-version change, independently of the crate version.
use super::wire::{Reader, Wire, read_map, write_map};
use super::{PrecompiledError, Result};
use crate::backtrack_lint::{BacktrackRisk, BacktrackWarning};
use crate::backtrack_rewrite::{BacktrackingRewrite, BacktrackingRewriteRefusal};
use crate::literal_trie::{CaseFolds, LiteralTrie};
use crate::regint::*;

macro_rules! fields {
    ($ty:ty { $($field:ident),* $(,)? }) => {
        impl Wire for $ty {
            fn write(&self, out: &mut Vec<u8>) -> Result<()> { $(self.$field.write(out)?;)* Ok(()) }
            fn read(input: &mut Reader<'_>) -> Result<Self> { Ok(Self { $($field: Wire::read(input)?),* }) }
        }
    };
}
macro_rules! variants {
    ($ty:ty { $($tag:literal => $variant:ident),* $(,)? }) => {
        impl Wire for $ty {
            fn write(&self, out: &mut Vec<u8>) -> Result<()> {
                let tag: u8 = match self { $(Self::$variant => $tag),* }; tag.write(out)
            }
            fn read(input: &mut Reader<'_>) -> Result<Self> {
                match u8::read(input)? { $($tag => Ok(Self::$variant)),*, _ => Err(PrecompiledError::InvalidData) }
            }
        }
    };
}
variants!(OpCode { 0 => Finish, 1 => End, 2 => Str1, 3 => Str2, 4 => Str3, 5 => Str4, 6 => Str5, 7 => StrN, 8 => StrMb2n1, 9 => StrMb2n2, 10 => StrMb2n3, 11 => StrMb2n, 12 => StrMb3n, 13 => StrMbn, 14 => CClass, 15 => CClassMb, 16 => CClassMix, 17 => CClassNot, 18 => CClassMbNot, 19 => CClassMixNot, 20 => AnyChar, 21 => AnyCharMl, 22 => AnyCharStar, 23 => AnyCharMlStar, 24 => AnyCharStarPeekNext, 25 => AnyCharMlStarPeekNext, 26 => Word, 27 => WordAscii, 28 => NoWord, 29 => NoWordAscii, 30 => WordBoundary, 31 => NoWordBoundary, 32 => WordBegin, 33 => WordEnd, 34 => TextSegmentBoundary, 35 => BeginBuf, 36 => EndBuf, 37 => BeginLine, 38 => EndLine, 39 => SemiEndBuf, 40 => CheckPosition, 41 => BackRef1, 42 => BackRef2, 43 => BackRefN, 44 => BackRefNIc, 45 => BackRefMulti, 46 => BackRefMultiIc, 47 => BackRefWithLevel, 48 => BackRefWithLevelIc, 49 => BackRefCheck, 50 => BackRefCheckWithLevel, 51 => MemStart, 52 => MemStartPush, 53 => MemEndPush, 54 => MemEndPushRec, 55 => MemEnd, 56 => MemEndRec, 57 => Fail, 58 => Jump, 59 => Push, 60 => PushSuper, 61 => Pop, 62 => PopToMark, 63 => PushOrJumpExact1, 64 => PushIfPeekNext, 65 => Repeat, 66 => RepeatNg, 67 => RepeatInc, 68 => RepeatIncNg, 69 => EmptyCheckStart, 70 => EmptyCheckEnd, 71 => EmptyCheckEndMemst, 72 => EmptyCheckEndMemstPush, 73 => Move, 74 => StepBackStart, 75 => StepBackNext, 76 => CutToMark, 77 => Mark, 78 => SaveVal, 79 => UpdateVar, 80 => Call, 81 => Return, 82 => CalloutContents, 83 => CalloutName, 84 => CClassStar, 85 => CClassMixStar, 86 => CClassMbStar, 87 => WordStar, 88 => WordAsciiStar, 89 => CClassStarPeekNext, 90 => WordAsciiStarPeekNext, 91 => AltLiterals, 92 => CClassNotStar, 93 => CClassMbNotStar, 94 => CClassMixNotStar, 95 => LookBehindOp, 96 => PushOrJumpByteSet, 97 => CClassRun, 98 => CClassPossessiveStar, 99 => DecimalTailPrefix, 100 => PushLazyAnyChar, 101 => PopStarChoices, 102 => GraphemeCluster, 103 => CClassRepeat, 104 => AnyCharRepeat });
variants!(SaveType { 0 => Keep, 1 => S, 2 => RightRange });
variants!(UpdateVarType { 0 => KeepFromStackLast, 1 => SFromStack, 2 => RightRangeFromStack, 3 => RightRangeFromSStack, 4 => RightRangeToS, 5 => RightRangeInit });
variants!(CheckPositionType { 0 => SearchStart, 1 => CurrentRightRange });
variants!(TextSegmentBoundaryType { 0 => ExtendedGraphemeCluster, 1 => Word });
variants!(StackPopLevel { 0 => Free, 1 => MemStart, 2 => All });
variants!(OptimizeType { 0 => None, 1 => Str, 2 => StrFast, 3 => StrFastStepForward, 4 => Map });
variants!(BacktrackRisk { 0 => NestedQuantifier, 1 => OverlappingAlternation });
variants!(BacktrackingRewriteRefusal { 0 => NoMandatoryDot, 1 => NoWordBoundary, 2 => UnsupportedConstruct, 3 => UnsupportedMode });

impl Wire for BacktrackingRewrite {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Self::AtomicDecimalLoop => 0u8.write(out),
            Self::PossessiveDecimalDigits => 1u8.write(out),
            Self::AtomicDecimalTail => 2u8.write(out),
            Self::DeterministicDecimalTail => 3u8.write(out),
            Self::Refused(reason) => {
                4u8.write(out)?;
                reason.write(out)
            }
        }
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(match u8::read(input)? {
            0 => Self::AtomicDecimalLoop,
            1 => Self::PossessiveDecimalDigits,
            2 => Self::AtomicDecimalTail,
            3 => Self::DeterministicDecimalTail,
            4 => Self::Refused(Wire::read(input)?),
            _ => return Err(PrecompiledError::InvalidData),
        })
    }
}
fields!(BacktrackWarning { risk, message });
fields!(RepeatRange {
    lower,
    upper,
    u_offset
});
fields!(GuardSkipGate {
    bsp,
    skipped_retries,
    addr,
    links
});
fields!(CaseFolds {
    class_members,
    ligatures,
    segments,
    accepted
});
impl Wire for CClassAsciiFastKind {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Self::None => 0u8.write(out),
            Self::Eq(b) => {
                1u8.write(out)?;
                b.write(out)
            }
            Self::EqFoldLower(b) => {
                2u8.write(out)?;
                b.write(out)
            }
        }
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(match u8::read(input)? {
            0 => Self::None,
            1 => Self::Eq(Wire::read(input)?),
            2 => Self::EqFoldLower(Wire::read(input)?),
            _ => return Err(PrecompiledError::InvalidData),
        })
    }
}
impl Wire for OperationPayload {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Self::None => {
                0u8.write(out)?;
            }
            Self::DecimalTailPrefix { captures } => {
                1u8.write(out)?;
                captures.write(out)?;
            }
            Self::Exact { s } => {
                2u8.write(out)?;
                s.write(out)?;
            }
            Self::ExactN { s, n } => {
                3u8.write(out)?;
                s.write(out)?;
                n.write(out)?;
            }
            Self::ExactLenN { s, n, len } => {
                4u8.write(out)?;
                s.write(out)?;
                n.write(out)?;
                len.write(out)?;
            }
            Self::CClass { bsp, ascii_fast } => {
                5u8.write(out)?;
                bsp.write(out)?;
                ascii_fast.write(out)?;
            }
            Self::CClassRun { bsp, len } => {
                6u8.write(out)?;
                bsp.write(out)?;
                len.write(out)?;
            }
            Self::CClassMb { mb } => {
                7u8.write(out)?;
                mb.write(out)?;
            }
            Self::CClassMix { mb, bsp } => {
                8u8.write(out)?;
                mb.write(out)?;
                bsp.write(out)?;
            }
            Self::AnyCharStarPeekNext { c } => {
                9u8.write(out)?;
                c.write(out)?;
            }
            Self::CClassStarPeekNext { bsp, c } => {
                10u8.write(out)?;
                bsp.write(out)?;
                c.write(out)?;
            }
            Self::CClassRepeat {
                bsp,
                ascii_fast,
                not,
                min,
                max,
                peek,
                ascii_only,
            } => {
                11u8.write(out)?;
                bsp.write(out)?;
                ascii_fast.write(out)?;
                not.write(out)?;
                min.write(out)?;
                max.write(out)?;
                peek.write(out)?;
                ascii_only.write(out)?;
            }
            Self::AnyCharRepeat {
                multiline,
                min,
                max,
            } => {
                12u8.write(out)?;
                multiline.write(out)?;
                min.write(out)?;
                max.write(out)?;
            }
            Self::WordAsciiStarPeekNext { c } => {
                13u8.write(out)?;
                c.write(out)?;
            }
            Self::WordBoundary { mode } => {
                14u8.write(out)?;
                mode.write(out)?;
            }
            Self::TextSegmentBoundary { boundary_type, not } => {
                15u8.write(out)?;
                boundary_type.write(out)?;
                not.write(out)?;
            }
            Self::CheckPosition { check_type } => {
                16u8.write(out)?;
                check_type.write(out)?;
            }
            Self::BackRefN { n1 } => {
                17u8.write(out)?;
                n1.write(out)?;
            }
            Self::BackRefGeneral {
                num,
                ns,
                nest_level,
            } => {
                18u8.write(out)?;
                num.write(out)?;
                ns.write(out)?;
                nest_level.write(out)?;
            }
            Self::MemoryStart { num } => {
                19u8.write(out)?;
                num.write(out)?;
            }
            Self::MemoryEnd { num } => {
                20u8.write(out)?;
                num.write(out)?;
            }
            Self::Jump { addr } => {
                21u8.write(out)?;
                addr.write(out)?;
            }
            Self::Push { addr } => {
                22u8.write(out)?;
                addr.write(out)?;
            }
            Self::PushOrJumpExact1 {
                addr,
                c,
                skipped_retries,
                skip_gate,
            } => {
                23u8.write(out)?;
                addr.write(out)?;
                c.write(out)?;
                skipped_retries.write(out)?;
                skip_gate.write(out)?;
            }
            Self::PushIfPeekNext { addr, c } => {
                24u8.write(out)?;
                addr.write(out)?;
                c.write(out)?;
            }
            Self::PopToMark { id } => {
                25u8.write(out)?;
                id.write(out)?;
            }
            Self::Repeat { id, addr } => {
                26u8.write(out)?;
                id.write(out)?;
                addr.write(out)?;
            }
            Self::RepeatInc { id } => {
                27u8.write(out)?;
                id.write(out)?;
            }
            Self::EmptyCheckStart { mem } => {
                28u8.write(out)?;
                mem.write(out)?;
            }
            Self::EmptyCheckEnd {
                mem,
                empty_status_mem,
            } => {
                29u8.write(out)?;
                mem.write(out)?;
                empty_status_mem.write(out)?;
            }
            Self::Move { n } => {
                30u8.write(out)?;
                n.write(out)?;
            }
            Self::StepBackStart {
                initial,
                remaining,
                addr,
            } => {
                31u8.write(out)?;
                initial.write(out)?;
                remaining.write(out)?;
                addr.write(out)?;
            }
            Self::CutToMark { id, restore_pos } => {
                32u8.write(out)?;
                id.write(out)?;
                restore_pos.write(out)?;
            }
            Self::Mark { id, save_pos } => {
                33u8.write(out)?;
                id.write(out)?;
                save_pos.write(out)?;
            }
            Self::SaveVal { save_type, id } => {
                34u8.write(out)?;
                save_type.write(out)?;
                id.write(out)?;
            }
            Self::UpdateVar {
                var_type,
                id,
                clear,
            } => {
                35u8.write(out)?;
                var_type.write(out)?;
                id.write(out)?;
                clear.write(out)?;
            }
            Self::Call { addr } => {
                36u8.write(out)?;
                addr.write(out)?;
            }
            Self::Return => {
                37u8.write(out)?;
            }
            Self::CalloutContents { num } => {
                38u8.write(out)?;
                num.write(out)?;
            }
            Self::CalloutName { num, id } => {
                39u8.write(out)?;
                num.write(out)?;
                id.write(out)?;
            }
            Self::AltLiterals { trie_idx } => {
                40u8.write(out)?;
                trie_idx.write(out)?;
            }
            Self::LookBehindOp {
                char_len,
                not,
                ascii_compatible,
            } => {
                41u8.write(out)?;
                char_len.write(out)?;
                not.write(out)?;
                ascii_compatible.write(out)?;
            }
            Self::PushLazyAnyChar { addr, c } => {
                42u8.write(out)?;
                addr.write(out)?;
                c.write(out)?;
            }
            Self::PushOrJumpByteSet {
                addr,
                bsp,
                skipped_retries,
                skip_gate,
            } => {
                43u8.write(out)?;
                addr.write(out)?;
                bsp.write(out)?;
                skipped_retries.write(out)?;
                skip_gate.write(out)?;
            }
        }
        Ok(())
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(match u8::read(input)? {
            0 => Self::None,
            1 => Self::DecimalTailPrefix {
                captures: Wire::read(input)?,
            },
            2 => Self::Exact {
                s: Wire::read(input)?,
            },
            3 => Self::ExactN {
                s: Wire::read(input)?,
                n: Wire::read(input)?,
            },
            4 => Self::ExactLenN {
                s: Wire::read(input)?,
                n: Wire::read(input)?,
                len: Wire::read(input)?,
            },
            5 => Self::CClass {
                bsp: Wire::read(input)?,
                ascii_fast: Wire::read(input)?,
            },
            6 => Self::CClassRun {
                bsp: Wire::read(input)?,
                len: Wire::read(input)?,
            },
            7 => Self::CClassMb {
                mb: Wire::read(input)?,
            },
            8 => Self::CClassMix {
                mb: Wire::read(input)?,
                bsp: Wire::read(input)?,
            },
            9 => Self::AnyCharStarPeekNext {
                c: Wire::read(input)?,
            },
            10 => Self::CClassStarPeekNext {
                bsp: Wire::read(input)?,
                c: Wire::read(input)?,
            },
            11 => Self::CClassRepeat {
                bsp: Wire::read(input)?,
                ascii_fast: Wire::read(input)?,
                not: Wire::read(input)?,
                min: Wire::read(input)?,
                max: Wire::read(input)?,
                peek: Wire::read(input)?,
                ascii_only: Wire::read(input)?,
            },
            12 => Self::AnyCharRepeat {
                multiline: Wire::read(input)?,
                min: Wire::read(input)?,
                max: Wire::read(input)?,
            },
            13 => Self::WordAsciiStarPeekNext {
                c: Wire::read(input)?,
            },
            14 => Self::WordBoundary {
                mode: Wire::read(input)?,
            },
            15 => Self::TextSegmentBoundary {
                boundary_type: Wire::read(input)?,
                not: Wire::read(input)?,
            },
            16 => Self::CheckPosition {
                check_type: Wire::read(input)?,
            },
            17 => Self::BackRefN {
                n1: Wire::read(input)?,
            },
            18 => Self::BackRefGeneral {
                num: Wire::read(input)?,
                ns: Wire::read(input)?,
                nest_level: Wire::read(input)?,
            },
            19 => Self::MemoryStart {
                num: Wire::read(input)?,
            },
            20 => Self::MemoryEnd {
                num: Wire::read(input)?,
            },
            21 => Self::Jump {
                addr: Wire::read(input)?,
            },
            22 => Self::Push {
                addr: Wire::read(input)?,
            },
            23 => Self::PushOrJumpExact1 {
                addr: Wire::read(input)?,
                c: Wire::read(input)?,
                skipped_retries: Wire::read(input)?,
                skip_gate: Wire::read(input)?,
            },
            24 => Self::PushIfPeekNext {
                addr: Wire::read(input)?,
                c: Wire::read(input)?,
            },
            25 => Self::PopToMark {
                id: Wire::read(input)?,
            },
            26 => Self::Repeat {
                id: Wire::read(input)?,
                addr: Wire::read(input)?,
            },
            27 => Self::RepeatInc {
                id: Wire::read(input)?,
            },
            28 => Self::EmptyCheckStart {
                mem: Wire::read(input)?,
            },
            29 => Self::EmptyCheckEnd {
                mem: Wire::read(input)?,
                empty_status_mem: Wire::read(input)?,
            },
            30 => Self::Move {
                n: Wire::read(input)?,
            },
            31 => Self::StepBackStart {
                initial: Wire::read(input)?,
                remaining: Wire::read(input)?,
                addr: Wire::read(input)?,
            },
            32 => Self::CutToMark {
                id: Wire::read(input)?,
                restore_pos: Wire::read(input)?,
            },
            33 => Self::Mark {
                id: Wire::read(input)?,
                save_pos: Wire::read(input)?,
            },
            34 => Self::SaveVal {
                save_type: Wire::read(input)?,
                id: Wire::read(input)?,
            },
            35 => Self::UpdateVar {
                var_type: Wire::read(input)?,
                id: Wire::read(input)?,
                clear: Wire::read(input)?,
            },
            36 => Self::Call {
                addr: Wire::read(input)?,
            },
            37 => Self::Return,
            38 => Self::CalloutContents {
                num: Wire::read(input)?,
            },
            39 => Self::CalloutName {
                num: Wire::read(input)?,
                id: Wire::read(input)?,
            },
            40 => Self::AltLiterals {
                trie_idx: Wire::read(input)?,
            },
            41 => Self::LookBehindOp {
                char_len: Wire::read(input)?,
                not: Wire::read(input)?,
                ascii_compatible: Wire::read(input)?,
            },
            42 => Self::PushLazyAnyChar {
                addr: Wire::read(input)?,
                c: Wire::read(input)?,
            },
            43 => Self::PushOrJumpByteSet {
                addr: Wire::read(input)?,
                bsp: Wire::read(input)?,
                skipped_retries: Wire::read(input)?,
                skip_gate: Wire::read(input)?,
            },
            _ => return Err(PrecompiledError::InvalidData),
        })
    }
}
fields!(Operation { opcode, payload });

impl Wire for LiteralTrie {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.literals().to_vec().write(out)?;
        self.is_case_insensitive().write(out)?;
        self.folds().is_some().write(out)?;
        if let Some(folds) = self.folds() {
            folds.write(out)?;
        }
        Ok(())
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        let literals = Vec::<Vec<u8>>::read(input)?;
        let insensitive = bool::read(input)?;
        let folds = Option::<CaseFolds>::read(input)?;
        super::validate::trie(&literals, insensitive, folds.as_ref())?;
        let total = literals
            .iter()
            .try_fold(1usize, |sum, s| sum.checked_add(s.len()))
            .ok_or(PrecompiledError::ResourceLimit)?;
        input.claim(
            total
                .checked_mul(64)
                .ok_or(PrecompiledError::ResourceLimit)?,
        )?;
        let refs: Vec<_> = literals.iter().map(Vec::as_slice).collect();
        Ok(match folds {
            Some(folds) => LiteralTrie::build_folded(&refs, folds),
            None => LiteralTrie::build(&refs, insensitive),
        })
    }
}
fn write_names(names: &Option<crate::regparse_types::NameTable>, out: &mut Vec<u8>) -> Result<()> {
    names.is_some().write(out)?;
    if let Some(table) = names {
        let mut names: Vec<_> = table.entries.iter().collect();
        names.sort_by(|a, b| a.0.cmp(b.0));
        names.len().write(out)?;
        for (key, value) in names {
            key.write(out)?;
            value.name.write(out)?;
            value.back_num.write(out)?;
            value.back_refs.write(out)?;
        }
    }
    Ok(())
}
fn read_names(input: &mut Reader<'_>) -> Result<Option<crate::regparse_types::NameTable>> {
    if !bool::read(input)? {
        return Ok(None);
    }
    let count = usize::read(input)?;
    input.claim(
        count
            .checked_mul(128)
            .ok_or(PrecompiledError::ResourceLimit)?,
    )?;
    let mut table = crate::regparse_types::NameTable::new();
    for _ in 0..count {
        let key = Vec::<u8>::read(input)?;
        let name = Vec::<u8>::read(input)?;
        let back_num = i32::read(input)?;
        let back_refs = Vec::<i32>::read(input)?;
        if key != name
            || back_num <= 0
            || back_num as usize != back_refs.len()
            || table
                .entries
                .insert(
                    key,
                    crate::regparse_types::NameEntry {
                        name,
                        back_num,
                        back_refs,
                    },
                )
                .is_some()
        {
            return Err(PrecompiledError::InvalidData);
        }
    }
    Ok(Some(table))
}

pub(super) fn write_reg(reg: &RegexType, out: &mut Vec<u8>) -> Result<()> {
    super::validate::program(reg)?;
    reg.backtrack_warnings.write(out)?;
    reg.backtrack_rewrites.write(out)?;
    reg.ops.write(out)?;
    reg.num_mem.write(out)?;
    reg.num_repeat.write(out)?;
    reg.num_empty_check.write(out)?;
    reg.num_call.write(out)?;
    reg.capture_history.write(out)?;
    reg.push_mem_start.write(out)?;
    reg.push_mem_end.write(out)?;
    reg.stack_pop_level.write(out)?;
    reg.repeat_range.write(out)?;
    reg.options.write(out)?;
    reg.case_fold_flag.write(out)?;
    write_names(&reg.name_table, out)?;
    reg.optimize.write(out)?;
    reg.threshold_len.write(out)?;
    reg.anchor.write(out)?;
    reg.anc_dist_min.write(out)?;
    reg.anc_dist_max.write(out)?;
    reg.sub_anchor.write(out)?;
    reg.exact.write(out)?;
    write_map(&reg.map, out)?;
    reg.map_offset.write(out)?;
    reg.map_bytes.write(out)?;
    reg.map_byte_count.write(out)?;
    reg.dist_min.write(out)?;
    reg.dist_max.write(out)?;
    reg.needs_capture_tracking.write(out)?;
    reg.keep_moves_match_start.write(out)?;
    write_map(&reg.first_byte_map, out)?;
    reg.has_first_byte_map.write(out)?;
    reg.start_dispatch.write(out)?;
    reg.called_addrs.write(out)?;
    reg.unset_call_addrs.write(out)?;
    reg.literal_tries.write(out)?;
    reg.check_dependent_guards.write(out)?;
    reg.skip_gates.write(out)?;
    write_literal_prefix(&reg.literal_prefix, out)?;
    reg.anychar_run.write(out)?;
    reg.look_behind_reach.write(out)?;
    reg.extra_map_optimizer.write(out)?;
    write_required_literals(&reg.required_literals, out)?;
    Ok(())
}
pub(super) fn read_reg(
    input: &mut Reader<'_>,
    settings: super::PatternSettings,
) -> Result<RegexType> {
    let mut reg = RegexType {
        enc: &crate::encodings::utf8::ONIG_ENCODING_UTF8,
        backtrack_warnings: Wire::read(input)?,
        backtrack_rewrites: Wire::read(input)?,
        ops: Wire::read(input)?,
        string_pool: Vec::new(),
        num_mem: Wire::read(input)?,
        num_repeat: Wire::read(input)?,
        num_empty_check: Wire::read(input)?,
        num_call: Wire::read(input)?,
        capture_history: Wire::read(input)?,
        push_mem_start: Wire::read(input)?,
        push_mem_end: Wire::read(input)?,
        stack_pop_level: Wire::read(input)?,
        repeat_range: Wire::read(input)?,
        options: Wire::read(input)?,
        syntax: settings.syntax.as_onig_syntax().clone(),
        case_fold_flag: Wire::read(input)?,
        name_table: read_names(input)?,
        optimize: Wire::read(input)?,
        threshold_len: Wire::read(input)?,
        anchor: Wire::read(input)?,
        anc_dist_min: Wire::read(input)?,
        anc_dist_max: Wire::read(input)?,
        sub_anchor: Wire::read(input)?,
        exact: Wire::read(input)?,
        exact_finder: None,
        pure_literal: false,
        skip_repeated_empty_search: false,
        map: read_map(input)?,
        map_offset: Wire::read(input)?,
        map_bytes: Wire::read(input)?,
        map_byte_count: Wire::read(input)?,
        map_ascii_ranges: MapAsciiRanges::default(),
        dist_min: Wire::read(input)?,
        dist_max: Wire::read(input)?,
        needs_capture_tracking: Wire::read(input)?,
        keep_moves_match_start: Wire::read(input)?,
        first_byte_map: read_map(input)?,
        has_first_byte_map: Wire::read(input)?,
        start_dispatch: Wire::read(input)?,
        called_addrs: Wire::read(input)?,
        unset_call_addrs: Wire::read(input)?,
        extp: None,
        literal_tries: Wire::read(input)?,
        check_dependent_guards: Wire::read(input)?,
        skip_gates: Wire::read(input)?,
        ac_alt: None,
        ac_alt_has_capture: false,
        leading_run: None,
        literal_prefix: read_literal_prefix(input)?,
        anychar_run: Wire::read(input)?,
        look_behind_reach: Wire::read(input)?,
        extra_map_optimizer: Wire::read(input)?,
        search_start_map: None,
        search_jump: None,
        required_literals: read_required_literals(input)?,
    };
    super::validate::program(&reg)?;
    crate::regcomp::rebuild_precompiled_search(&mut reg);
    Ok(reg)
}

fn write_literal_prefix(
    prefix: &Option<Box<crate::leading_run::LiteralPrefix>>,
    out: &mut Vec<u8>,
) -> Result<()> {
    prefix.is_some().write(out)?;
    if let Some(prefix) = prefix {
        let (bytes, class, multibyte, retries) = prefix.precompiled_parts();
        bytes.write(out)?;
        class.write(out)?;
        multibyte.write(out)?;
        retries.write(out)?;
    }
    Ok(())
}
fn read_literal_prefix(
    input: &mut Reader<'_>,
) -> Result<Option<Box<crate::leading_run::LiteralPrefix>>> {
    if !bool::read(input)? {
        return Ok(None);
    }
    crate::leading_run::LiteralPrefix::from_precompiled(
        Wire::read(input)?,
        Wire::read(input)?,
        Wire::read(input)?,
        Wire::read(input)?,
    )
    .map(|p| Some(Box::new(p)))
    .ok_or(PrecompiledError::InvalidData)
}
fn write_required_literals(
    literals: &Option<Box<crate::required_literals::RequiredLiterals>>,
    out: &mut Vec<u8>,
) -> Result<()> {
    literals.is_some().write(out)?;
    if let Some(literals) = literals {
        literals.precompiled_parts().write(out)?;
    }
    Ok(())
}
fn read_required_literals(
    input: &mut Reader<'_>,
) -> Result<Option<Box<crate::required_literals::RequiredLiterals>>> {
    if !bool::read(input)? {
        return Ok(None);
    }
    let (bytes, ends, calls) = Wire::read(input)?;
    crate::required_literals::RequiredLiterals::from_precompiled(bytes, ends, calls)
        .map(|p| Some(Box::new(p)))
        .ok_or(PrecompiledError::InvalidData)
}
#[cfg(feature = "dfa-prefilter")]
pub(super) fn write_seek(seek: &super::SharedSeek, out: &mut Vec<u8>) -> Result<()> {
    use crate::dfa_prefilter::Seek;
    match seek.as_deref() {
        None => 0u8.write(out),
        Some(Seek::Everywhere) => 1u8.write(out),
        Some(Seek::Pattern(code)) => {
            2u8.write(out)?;
            code.precompiled_bytes().to_vec().write(out)
        }
    }
}
#[cfg(not(feature = "dfa-prefilter"))]
pub(super) fn write_seek(_: &super::SharedSeek, out: &mut Vec<u8>) -> Result<()> {
    0u8.write(out)
}
#[cfg(feature = "dfa-prefilter")]
pub(super) fn read_seek(input: &mut Reader<'_>) -> Result<super::SharedSeek> {
    use crate::dfa_prefilter::{Seek, SeekCode};
    Ok(match u8::read(input)? {
        0 => None,
        1 => Some(std::sync::Arc::new(Seek::Everywhere)),
        2 => {
            let bytes = Vec::<u8>::read(input)?;
            let code = SeekCode::from_precompiled(bytes).ok_or(PrecompiledError::InvalidData)?;
            Some(std::sync::Arc::new(Seek::Pattern(code)))
        }
        _ => return Err(PrecompiledError::InvalidData),
    })
}
#[cfg(not(feature = "dfa-prefilter"))]
pub(super) fn read_seek(input: &mut Reader<'_>) -> Result<super::SharedSeek> {
    if u8::read(input)? != 0 {
        return Err(PrecompiledError::InvalidData);
    }
    Ok(())
}

impl Wire for crate::oniguruma::OnigOptionType {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.bits().write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Self::from_bits(u32::read(input)?).ok_or(PrecompiledError::InvalidData)
    }
}
