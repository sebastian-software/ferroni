//! Counted character repeats (ADR-008, `CClassRepeat` and `AnyCharRepeat`).
//!
//! A greedy `C{min,max}` over one character instruction runs as a counted
//! loop. These tests compare it with the upstream counter loop
//! (`COUNTED_CHAR_REPEAT_DISABLED`) over bodies, bounds, followers, subjects
//! with multibyte and malformed UTF-8, and every start offset. With the `ffi`
//! feature they compare it with C Oniguruma as well, including under the
//! retry and match stack limits (ADR-017: where C completes, the result is
//! C's; where C stops at a limit, the error is C's or the unlimited result).

use crate::oniguruma::*;
use crate::regcomp::{COUNTED_CHAR_REPEAT_DISABLED, onig_new};
use crate::regenc::OnigEncoding;
use crate::regexec::{onig_search, shared_limits};
use crate::regint::{OpCode, RegexType};
use crate::regsyntax::OnigSyntaxOniguruma;

const UTF8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;

/// Compiles `pattern` with the counted loops on or off.
fn compile(pattern: &str, counted: bool) -> Result<RegexType, crate::error::RegexError> {
    COUNTED_CHAR_REPEAT_DISABLED.with(|disabled| disabled.set(!counted));
    let reg = onig_new(
        pattern.as_bytes(),
        ONIG_OPTION_NONE,
        UTF8,
        &OnigSyntaxOniguruma,
    );
    COUNTED_CHAR_REPEAT_DISABLED.with(|disabled| disabled.set(false));
    reg
}

/// The result of one search: the return value and the capture spans.
type Outcome = (i32, Vec<(i32, i32)>);

fn search(reg: &RegexType, subject: &[u8], start: usize) -> Outcome {
    let (result, region) = onig_search(
        reg,
        subject,
        subject.len(),
        start,
        subject.len(),
        Some(OnigRegion::new()),
        ONIG_OPTION_NONE,
    );
    let spans = match region {
        Some(region) if result >= 0 => region
            .beg
            .iter()
            .zip(&region.end)
            .take(region.num_regs as usize)
            .map(|(&b, &e)| (b, e))
            .collect(),
        _ => Vec::new(),
    };
    (result, spans)
}

#[cfg(feature = "ffi")]
fn c_search(c: &crate::ffi::CRegex, subject: &[u8], start: usize) -> Outcome {
    let mut region = crate::ffi::CRegion::new();
    let result = c.search(subject, start, subject.len(), Some(&mut region), 0);
    let spans = if result >= 0 {
        region.capture_ranges()
    } else {
        Vec::new()
    };
    (result, spans)
}

/// Bodies that compile to one character instruction, and some that do not.
const BODIES: [&str; 18] = [
    "[0-9]",
    "[^0-9]",
    "[a-zA-Z-]",
    "[^a-z\\n]",
    "[ab]",
    "[0-9é]",
    "[^é]",
    "[aé]",
    "a",
    "1",
    ".",
    "(?m).",
    "(?-m:.)",
    "é",
    "(?i)a",
    "(?i)[a-z]",
    "\\d",
    "\\w",
];

/// Bounds of `C{n,m}` and `C{n,}`, including the ones that C expands inline.
const BOUNDS: [&str; 10] = [
    "{0,6}", "{1,6}", "{2,7}", "{0,256}", "{6,6}", "{3,}", "{11,}", "{0,2}", "{2,2}", "{0,7}",
];

/// Followers: disjoint, overlapping, nothing, a literal, a capture, a look-around.
const FOLLOWERS: [&str; 14] = [
    "",
    "x",
    "[a-z]",
    "[0-9]",
    "\\.",
    "(?:a|b)",
    "$",
    "\\b",
    "(?=b)",
    "(?!1)",
    "(?<=a)",
    "\\z",
    "\\.[0-9]+",
    "x\\b",
];

/// Subjects: empty, multibyte, malformed UTF-8, newlines, long runs.
const SUBJECTS: [&[u8]; 22] = [
    b"",
    b"0",
    b"7",
    b"12",
    b"123456789",
    b"12.3",
    b"a1-b",
    b"ab-cd.ef",
    "é12".as_bytes(),
    "12é3".as_bytes(),
    "aé-é".as_bytes(),
    "é".as_bytes(),
    b"1\n2",
    b"\n",
    b"x12345678x",
    b"1234567890123456789.",
    b"12\x80",
    b"\xe9x",
    b"aaaaaaaaaaaaaaaaaaaa",
    b"ab\nab",
    b"AbZ-b",
    "ééééééééé1.".as_bytes(),
];

/// Every pattern of the grid: the body and bound, then a follower, in three
/// shapes: plain, as a capture, and inside a loop with a mandatory literal.
fn grid() -> Vec<String> {
    let mut patterns = Vec::new();
    for body in BODIES {
        for bound in BOUNDS {
            for follower in FOLLOWERS {
                patterns.push(format!("{body}{bound}{follower}"));
                patterns.push(format!("({body}{bound}){follower}"));
                patterns.push(format!("(?:{body}{bound}{follower}x?)+"));
            }
        }
    }
    patterns
}

#[test]
fn counted_loops_match_the_counter_loop_over_the_grid() {
    let _limits = shared_limits();
    let mut compared = 0usize;
    let mut counted_patterns = 0usize;
    for pattern in grid() {
        let Ok(counted) = compile(&pattern, true) else {
            assert!(
                compile(&pattern, false).is_err(),
                "{pattern} compiles only without counted loops"
            );
            continue;
        };
        let upstream = compile(&pattern, false).expect("the counter loop compiles");
        if counted
            .ops
            .iter()
            .any(|op| matches!(op.opcode, OpCode::CClassRepeat | OpCode::AnyCharRepeat))
        {
            counted_patterns += 1;
        }
        for subject in SUBJECTS {
            for start in 0..=subject.len() {
                let got = search(&counted, subject, start);
                let want = search(&upstream, subject, start);
                assert_eq!(
                    got, want,
                    "{pattern} on {subject:?} from {start}, counted loops against the counter loop"
                );
                compared += 1;
            }
        }
    }
    // The grid must exercise the new loops, not only the old path.
    assert!(
        counted_patterns > 1000,
        "{counted_patterns} patterns used a counted loop"
    );
    assert!(compared > 100_000, "only {compared} searches compared");
}

#[cfg(feature = "ffi")]
#[test]
fn counted_loops_match_c_over_the_grid() {
    let _limits = shared_limits();
    let mut mismatches_before = 0usize;
    for pattern in grid() {
        let Ok(counted) = compile(&pattern, true) else {
            continue;
        };
        let upstream = compile(&pattern, false).expect("the counter loop compiles");
        let c = crate::ffi::CRegex::new(pattern.as_bytes(), crate::ffi::ONIG_OPTION_NONE)
            .unwrap_or_else(|code| panic!("C rejects {pattern}: {code}"));
        for subject in SUBJECTS {
            for start in 0..=subject.len() {
                let want = c_search(&c, subject, start);
                let got = search(&counted, subject, start);
                // Some malformed-UTF-8 searches already differ from C on the
                // counter loop (its start positions); the counted loop must
                // not add a difference to the others.
                if search(&upstream, subject, start) == want {
                    assert_eq!(
                        got, want,
                        "{pattern} on {subject:?} from {start}, against C"
                    );
                } else {
                    mismatches_before += 1;
                }
            }
        }
    }
    // Keep the exception list visible: it is the counter loop's own.
    assert!(
        mismatches_before < 2_000,
        "{mismatches_before} searches differ from C"
    );
}

/// Patterns whose searches run long enough for the limits to matter.
#[cfg(feature = "ffi")]
const LIMITED: [(&str, &str); 8] = [
    (
        "[0-9]{0,256}\\.",
        "1234567890123456789012345678901234567890",
    ),
    (
        "([0-9]{0,256}[a-z]){0,3}x",
        "1111111111a1111111111b1111111111c1111",
    ),
    ("[a-z]{2,}x", "abcdefghijklmnopqrstuvwxyzabcdefghijklmnop"),
    (
        ".{0,300}y",
        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
    ),
    (
        "(?:[0-9]{0,40}a?){2}z",
        "1234567890123456789012345678901234567890",
    ),
    ("[0-9]{6,}9", "1234567890123456789012345678901234567890"),
    ("é{0,20}z", "éééééééééééééééééééééééééééé"),
    (
        "(?:\\d{0,64}a){0,4}b",
        "1111111111111111111111111111111111111111111111111111111111111112",
    ),
];

/// Under a retry limit, C's result when it completes, and otherwise C's error
/// or its unlimited result (ADR-017).
#[cfg(feature = "ffi")]
#[test]
fn counted_loops_stay_within_c_limits() {
    let _limits = crate::regexec::exclusive_limits();
    let (saved_retry, saved_stack) = (
        crate::ffi::retry_limit_in_match(),
        crate::ffi::match_stack_limit_size(),
    );
    for (pattern, subject) in LIMITED {
        let reg = compile(pattern, true).unwrap();
        let c = crate::ffi::CRegex::new(pattern.as_bytes(), crate::ffi::ONIG_OPTION_NONE).unwrap();
        let subject = subject.as_bytes();
        crate::ffi::set_retry_limit_in_match(0);
        let unlimited = c_search(&c, subject, 0);
        assert!(unlimited.0 >= -1, "{pattern}: C fails without a limit");
        for limit in [1, 2, 3, 4, 6, 10, 25, 100, 1000, 10_000] {
            crate::ffi::set_retry_limit_in_match(limit);
            let c_limited = c_search(&c, subject, 0);
            crate::regexec::onig_set_retry_limit_in_match(limit);
            let got = search(&reg, subject, 0);
            crate::regexec::onig_set_retry_limit_in_match(0);
            let context = format!("{pattern} on {subject:?} with retry limit {limit}");
            if c_limited.0 >= -1 {
                assert_eq!(got, c_limited, "{context}: C completes");
            } else {
                assert!(
                    got == c_limited || got == unlimited,
                    "{context}: C stops with {c_limited:?}, Ferroni returned {got:?}"
                );
            }
        }
        // C grows its match stack from 160 entries and stops only when the
        // stack would grow past the limit, so below 160 it does not stop
        // where Ferroni's entry count does. There the counter loop is the
        // reference: the counted loop must not stop where it completes.
        let upstream = compile(pattern, false).unwrap();
        for size in [1, 2, 3, 5, 8, 16, 40, 100, 159, 160, 1000] {
            crate::ffi::set_match_stack_limit_size(size);
            let c_limited = c_search(&c, subject, 0);
            crate::regexec::onig_set_match_stack_limit_size(size);
            let got = search(&reg, subject, 0);
            let old = search(&upstream, subject, 0);
            crate::regexec::onig_set_match_stack_limit_size(0);
            let context = format!("{pattern} on {subject:?} with stack limit {size}");
            if old.0 >= -1 {
                assert_eq!(got, old, "{context}: the counter loop completes");
            }
            if c_limited.0 >= -1 && size >= 160 {
                assert_eq!(got, c_limited, "{context}: C completes");
            } else if c_limited.0 < -1 {
                assert!(
                    got == c_limited || got == unlimited,
                    "{context}: C stops with {c_limited:?}, Ferroni returned {got:?}"
                );
            }
        }
    }
    crate::ffi::set_retry_limit_in_match(saved_retry);
    crate::ffi::set_match_stack_limit_size(saved_stack);
}

/// The counted loops replace exactly the single-instruction bounded repeats
/// that the counter loop ran; everything else keeps its code.
#[test]
fn counted_loops_are_compiled_for_single_instruction_bodies_only() {
    let _limits = shared_limits();
    let counted = |pattern: &str| {
        let reg = compile(pattern, true).unwrap();
        reg.ops
            .iter()
            .filter(|op| matches!(op.opcode, OpCode::CClassRepeat | OpCode::AnyCharRepeat))
            .count()
    };
    let counter = |pattern: &str| {
        let reg = compile(pattern, true).unwrap();
        reg.ops
            .iter()
            .filter(|op| matches!(op.opcode, OpCode::Repeat | OpCode::RepeatNg))
            .count()
    };

    // Single class, byte or dot bodies over the counter-loop bounds.
    assert_eq!(counted("[0-9]{0,256}"), 1);
    assert_eq!(counted("[^0-9]{0,256}"), 1);
    assert_eq!(counted("a{6,7}"), 1);
    assert_eq!(counted("a{6,}"), 1);
    assert_eq!(counted("[0-9]{11,}"), 1);
    assert_eq!(counted(".{0,300}"), 1);
    assert_eq!(counted("(?m).{0,300}"), 1);
    assert_eq!(counter("[0-9]{0,256}"), 0);
    assert_eq!(counter("a{6,}"), 0);

    // Expanded inline (C's guard) and lazy ranges keep their code.
    assert_eq!(counter("a{2,}"), 0);
    assert_eq!(counted("a{2,5}"), 0);
    assert_eq!(counted("[0-9]{0,5}"), 0);
    assert_eq!(counted("a{2,}"), 1, "{{2,}} is a counted loop too");
    assert_eq!(counted("[0-9]{1,}"), 0, "+ has its star opcode");
    assert_eq!(counted("[0-9]{0,256}?"), 0);
    assert_eq!(counter("[0-9]{0,256}?"), 1);
    assert_eq!(counted("[0-9]{6,6}"), 0, "exact counts are not ranges");
    assert_eq!(counted("[0-9]{0,1}"), 0);

    // Bodies that are not one instruction keep the counter loop.
    assert_eq!(counted("é{0,256}"), 0, "a multibyte literal");
    assert_eq!(counted("[é]{0,256}"), 0, "a class with a multibyte member");
    assert_eq!(counted("(?:ab){0,256}"), 0, "two characters");
    assert_eq!(counted("(a){0,256}"), 0, "a capture");
    assert_eq!(counter("(a){0,256}"), 1);
    assert_eq!(
        counted("\\p{N}{0,256}"),
        0,
        "a property class has multibyte members"
    );
    assert_eq!(counter("\\p{N}{0,256}"), 1);
    // Case folding keeps a one-byte class one instruction; a class whose
    // folding adds multibyte members (U+017F, U+212A) keeps the counter loop.
    assert_eq!(
        counted("(?i)a{0,256}"),
        1,
        "a case-folded byte is a one-byte class"
    );
    assert_eq!(
        counted("(?i)[ab]{0,256}"),
        1,
        "a case-folded class of ASCII bytes"
    );
    assert_eq!(
        counted("(?i)[a-z]{0,256}"),
        0,
        "folding adds multibyte members"
    );
    assert_eq!(counter("(?i)[a-z]{0,256}"), 1);
    assert_eq!(counted("(?m).{0,256}"), 1);
    assert_eq!(counted("\\d{0,256}"), 0, "a character type");
    assert_eq!(counter("\\d{0,256}"), 1);
}

/// The counted loop gives back one character at a time, down to the
/// minimum, and takes the same alternatives as the counter loop.
#[test]
fn counted_loop_results_on_hand_written_cases() {
    let _limits = shared_limits();
    type Case = (&'static str, &'static [u8], Outcome);
    let cases: [Case; 9] = [
        // Greedy, with give-back down to the minimum.
        ("^[0-9]{2,4}5", b"12345", (0, vec![(0, 5)])),
        ("^[0-9]{2,4}5", b"1234", (-1, vec![])),
        ("^[0-9]{0,4}(\\d)", b"12", (0, vec![(0, 2), (1, 2)])),
        // Multibyte characters in the run: a give-back lands on a character head.
        ("^.{1,4}é", "aaaaé".as_bytes(), (0, vec![(0, 6)])),
        ("^.{0,2}é", "aaé".as_bytes(), (0, vec![(0, 4)])),
        ("^[^a]{1,3}a", "bbéa".as_bytes(), (0, vec![(0, 5)])),
        ("^[^a]{2,}a", "xéyéa".as_bytes(), (0, vec![(0, 7)])),
        // A malformed byte is one character of `.`.
        ("^.{1,3}z", b"\x80\x80z", (0, vec![(0, 3)])),
        // The minimum is not given back.
        ("^a{3,5}", b"aa", (-1, vec![])),
    ];
    for (pattern, subject, want) in cases {
        let reg = compile(pattern, true).unwrap();
        assert_eq!(search(&reg, subject, 0), want, "{pattern} on {subject:?}");
        let upstream = compile(pattern, false).unwrap();
        assert_eq!(
            search(&upstream, subject, 0),
            want,
            "{pattern} on {subject:?}, upstream"
        );
    }
}

/// Constructs the grid does not reach: back references, look-behinds, `\K`,
/// atomic and possessive groups, lazy ranges, nested loops and alternations.
#[test]
fn counted_loops_agree_with_the_counter_loop_in_other_constructs() {
    let _limits = shared_limits();
    let patterns = [
        "^([0-9]{1,4})x\\1",
        "^(a{0,3})b\\1$",
        "(?<g>[0-9]{0,6})-\\k<g>",
        "(?<=[0-9]{2})x",
        "(?<=a|[0-9]{0,6})x",
        "(?<!a[0-9]{0,6})x",
        "[0-9]{0,6}\\K\\.",
        "(?>[0-9]{0,6})\\.",
        "(?>[0-9]{2,6})[0-9]",
        "[0-9]{0,6}+\\.",
        "[0-9]{0,6}?\\.",
        "[0-9]{2,6}?\\.",
        "(?:[0-9]{0,6}\\.)+",
        "(?:[0-9]{0,6}|x){2}",
        "(?:[0-9]{0,6}){2}\\.",
        "([0-9]{0,6}){0,3}z",
        "a|[0-9]{0,6}b|[^0-9]{2,6}c",
        "(?:a[0-9]{1,6}|b.{0,6})+$",
        "(?m)^.{0,6}$",
        "(?m).{0,6}\\n",
        "(?i:[a-f]{2,6})g",
        "\\A[0-9]{0,8}\\z",
        "[0-9]{0,6}(?=\\.)",
        "[0-9]{0,6}(?!\\.)\\d",
        "(?:x[0-9]{3,}|y[0-9]{0,4})z",
        "[^\\s]{1,5}\\s{0,1}[^\\s]{1,5}$",
    ];
    let subjects: [&[u8]; 9] = [
        b"12x12",
        b"aabaa",
        b"12-12",
        b"1234567.8",
        b"a12x",
        b"12345.67z\n",
        b"aaaaabbbbbcccccc",
        b"ab\ncd\n",
        "éé1.2z é".as_bytes(),
    ];
    let mut compiled = 0;
    let mut counted_loops = 0;
    for pattern in patterns {
        let counted = compile(pattern, true);
        let upstream = compile(pattern, false);
        if let Ok(reg) = &counted {
            compiled += 1;
            counted_loops += usize::from(
                reg.ops
                    .iter()
                    .any(|op| matches!(op.opcode, OpCode::CClassRepeat | OpCode::AnyCharRepeat)),
            );
        }
        let (Ok(counted), Ok(upstream)) = (counted.as_ref(), upstream.as_ref()) else {
            assert_eq!(
                counted.map(|_| ()).err().map(|e| format!("{e:?}")),
                upstream.map(|_| ()).err().map(|e| format!("{e:?}")),
                "{pattern}: the compile errors differ"
            );
            continue;
        };
        #[cfg(feature = "ffi")]
        let c = crate::ffi::CRegex::new(pattern.as_bytes(), crate::ffi::ONIG_OPTION_NONE).ok();
        for subject in subjects {
            for start in 0..=subject.len() {
                let got = search(counted, subject, start);
                assert_eq!(
                    got,
                    search(upstream, subject, start),
                    "{pattern} on {subject:?} from {start}"
                );
                // As in the grid: where the counter loop agrees with C, so must this.
                #[cfg(feature = "ffi")]
                if let Some(c) = &c {
                    let want = c_search(c, subject, start);
                    if search(upstream, subject, start) == want {
                        assert_eq!(
                            got, want,
                            "{pattern} on {subject:?} from {start}, against C"
                        );
                    }
                }
            }
        }
    }
    assert!(compiled >= 20, "only {compiled} patterns compile");
    assert!(
        counted_loops >= 10,
        "only {counted_loops} patterns use a counted loop"
    );
}
