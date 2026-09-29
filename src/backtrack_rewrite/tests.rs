use super::*;
use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::oniguruma::*;
use crate::regcomp::{onig_compile, onig_new_with_backtracking_optimization};
use crate::regexec::onig_search;
use crate::regint::*;
use crate::regsyntax::OnigSyntaxOniguruma;

fn compile(pattern: &str, enabled: bool) -> RegexType {
    onig_new_with_backtracking_optimization(
        pattern.as_bytes(),
        ONIG_OPTION_NONE,
        &ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
        enabled,
    )
    .unwrap()
}

fn raw_trace(
    reg: &RegexType,
    text: &[u8],
    end: usize,
    start: usize,
    range: usize,
    options: OnigOptionType,
) -> (i32, Vec<(i32, i32)>) {
    let (result, region) = onig_search(
        reg,
        text,
        end,
        start,
        range,
        Some(OnigRegion::new()),
        options,
    );
    let captures = if result >= 0 {
        let region = region.unwrap();
        region
            .beg
            .into_iter()
            .zip(region.end)
            .take(region.num_regs as usize)
            .collect()
    } else {
        Vec::new()
    };
    (result, captures)
}

#[test]
fn rewrites_preserve_raw_forward_backward_bounded_and_invalid_byte_searches() {
    let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
    for pattern in [
        r"([0-9]+(_?))+(\.)([0-9]*)",
        r"\b([0-9]+(_?))+(\.)([0-9]*)",
        r"(([0-9]+_?)+\.([0-9]*))*x?",
        r"([0-9]+_?)+\.([0-9]*)|([0-9]+)",
        r"\b(([0-9]+_?)*[0-9]+|0([Xx]\h+|[Oo][0-7]+))\b",
        r"\b((([0-9])+)((_)?))*(([0-9])+)\b",
    ] {
        let plain = compile(pattern, false);
        let fast = compile(pattern, true);
        if pattern == r"([0-9]+(_?))+(\.)([0-9]*)" || pattern == r"\b([0-9]+(_?))+(\.)([0-9]*)" {
            assert!(
                fast.leading_run.is_some(),
                "possessive opcode must retain the leading-run plan"
            );
        }
        if fast
            .backtrack_rewrites
            .contains(&BacktrackingRewrite::PossessiveDecimalDigits)
        {
            assert!(
                fast.backtrack_rewrites
                    .contains(&BacktrackingRewrite::PossessiveDecimalDigits)
            );
            assert!(
                fast.ops
                    .iter()
                    .any(|op| op.opcode == OpCode::CClassPossessiveStar)
            );
            assert!(
                !fast
                    .ops
                    .iter()
                    .any(|op| matches!(op.opcode, OpCode::Mark | OpCode::CutToMark))
            );
            assert!(
                !plain
                    .ops
                    .iter()
                    .any(|op| op.opcode == OpCode::CClassPossessiveStar)
            );
        } else {
            if fast
                .backtrack_rewrites
                .contains(&BacktrackingRewrite::DeterministicDecimalTail)
            {
                assert!(
                    fast.ops
                        .iter()
                        .any(|op| op.opcode == OpCode::DecimalTailPrefix)
                );
                assert!(
                    !fast
                        .ops
                        .iter()
                        .any(|op| matches!(op.opcode, OpCode::Mark | OpCode::CutToMark))
                );
            } else {
                assert!(
                    fast.backtrack_rewrites
                        .contains(&BacktrackingRewrite::AtomicDecimalTail)
                );
                assert!(fast.ops.iter().any(|op| op.opcode == OpCode::CutToMark));
            }
        }
        assert!(
            !plain
                .ops
                .iter()
                .any(|op| op.opcode == OpCode::DecimalTailPrefix)
        );
        for text in [
            b"x1_2.3 4.5".as_slice(),
            b"12__.3",
            b"12_.3",
            b"\xff12.3\x801.2",
            b"12.\xe2\x82",
            b"123_ 0x1f\xff01 0o7",
        ] {
            for end in 0..=text.len() {
                for start in 0..=end {
                    for range in 0..=end {
                        for options in [
                            ONIG_OPTION_NONE,
                            ONIG_OPTION_NOT_BEGIN_STRING | ONIG_OPTION_NOT_END_STRING,
                            ONIG_OPTION_FIND_LONGEST,
                            ONIG_OPTION_FIND_NOT_EMPTY,
                            ONIG_OPTION_MATCH_WHOLE_STRING,
                        ] {
                            assert_eq!(
                                raw_trace(&fast, text, end, start, range, options),
                                raw_trace(&plain, text, end, start, range, options),
                                "{pattern} on {text:?}, end {end}, start {start}, range {range}, {options:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn capture_history_is_refused_and_recompilation_clears_reports() {
    let mut syntax = OnigSyntaxOniguruma.clone();
    syntax.op2 |= ONIG_SYN_OP2_ATMARK_CAPTURE_HISTORY;
    let reg = onig_new_with_backtracking_optimization(
        br"(?@([0-9]+_?))+\.[0-9]+",
        ONIG_OPTION_NONE,
        &ONIG_ENCODING_UTF8,
        &syntax,
        true,
    )
    .unwrap();
    assert_eq!(
        reg.backtrack_rewrites,
        [BacktrackingRewrite::Refused(
            BacktrackingRewriteRefusal::UnsupportedMode
        )]
    );

    let mut reg = compile(r"([0-9]+_?)+\.[0-9]+", true);
    assert_eq!(
        reg.backtrack_rewrites,
        [BacktrackingRewrite::PossessiveDecimalDigits]
    );
    assert_eq!(onig_compile(&mut reg, br"([0-9]+_?)+\.[0-9]+"), 0);
    assert!(reg.backtrack_rewrites.is_empty());
}
