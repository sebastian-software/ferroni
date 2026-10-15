//! Rules 2 and 3 of ADR-017 under small match stack limits: where C Oniguruma
//! completes, Ferroni completes with the same result, and where C stops,
//! Ferroni stops too or returns the result of the search without a limit.
//!
//! C checks `match_stack_limit` only when `stack_double` reallocates a heap
//! stack. Its first stack holds `INIT_MATCH_STACK_SIZE` (160) entries on the
//! C stack and moves to a heap stack of 320 entries without a check. When a
//! push finds a heap stack full and the limit is below its size, C reallocates
//! it to twice its size, grants that push and sets the end of the stack to
//! the limit, so the next push stops the match. Under any limit below 320,
//! C therefore completes every search that holds at most 321 entries, more
//! than under a limit of 320. The reallocation keeps the doubled size, so the
//! entry above the limit stays inside the buffer and calling C in this regime
//! is safe. Ferroni caps every limit below 320 at those 321 entries
//! (`match_stack_cap`).
#![cfg(feature = "ffi")]

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::ffi::{CRegex, CRegion};
use ferroni::oniguruma::*;
use ferroni::regcomp::onig_new;
use ferroni::regexec::{
    onig_new_match_param, onig_search_with_param, onig_set_match_stack_limit_size_of_match_param,
};
use ferroni::regsyntax::OnigSyntaxOniguruma;

/// A search result: the return value and the capture spans.
type Outcome = (i32, Vec<(i32, i32)>);

fn ferroni_search(reg: &RegexType, subject: &[u8], limit: u32) -> Outcome {
    let mut mp = onig_new_match_param();
    onig_set_match_stack_limit_size_of_match_param(&mut mp, limit);
    let (r, region) = onig_search_with_param(
        reg,
        subject,
        subject.len(),
        0,
        subject.len(),
        Some(OnigRegion::new()),
        ONIG_OPTION_NONE,
        &mp,
    );
    let spans = match region {
        Some(region) if r >= 0 => region
            .beg
            .iter()
            .zip(&region.end)
            .map(|(&beg, &end)| (beg, end))
            .collect(),
        _ => Vec::new(),
    };
    (r, spans)
}

fn c_search(c: &CRegex, subject: &[u8], limit: u32) -> Outcome {
    let mut region = CRegion::new();
    let r = c.search_with_match_stack_limit(
        subject,
        0,
        subject.len(),
        Some(&mut region),
        ferroni::ffi::ONIG_OPTION_NONE,
        limit,
    );
    let spans = if r >= 0 {
        region.capture_ranges()
    } else {
        Vec::new()
    };
    (r, spans)
}

fn compile(pattern: &str) -> (RegexType, CRegex) {
    let reg = onig_new(
        pattern.as_bytes(),
        ONIG_OPTION_NONE,
        &ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
    )
    .unwrap_or_else(|e| panic!("{pattern} compiles: {e:?}"));
    let c = CRegex::new(pattern.as_bytes(), ferroni::ffi::ONIG_OPTION_NONE).unwrap();
    (reg, c)
}

/// Patterns that hold about one or two stack entries per repetition of the
/// unit, so the repetition counts below put the deepest stack on both sides
/// of 160, 320 and 321 entries.
const CASES: &[(&str, &str)] = &[
    (r"(?:a|b)*z", "a"),
    (r"(a|b)*z", "a"),
    (r"(?:\w+\s)*z", "ab "),
    (r"(?:(a)|b)*c", "a"),
    (r"(?:a|ab)*?z", "ab"),
    (r"(?<=a|b)(?:x|y)*z", "x"),
    // More than ALLOCA_PTR_NUM_LIMIT pointers: C starts on the heap and
    // checks the limit at its first doubling, from 160 entries.
    (
        r"(a)(b)?(c)?(d)?(e)?(f)?(g)?(h)?(i)?(j)?(k)?(l)?(m)?(n)?(o)?(p)?(q)?(r)?(s)?(t)?(u)?(v)?(w)?(x)?(y)?(?:a|b)*z",
        "a",
    ),
];

const COUNTS: &[usize] = &[1, 100, 157, 158, 159, 160, 316, 317, 318, 319, 320, 400];

/// Limits below, at and just above C's unchecked first stack.
fn limits() -> impl Iterator<Item = u32> {
    (1..=8)
        .chain([100, 159, 160, 161, 162, 200])
        .chain(310..=330)
        .chain([400, 640, 641, 700])
}

#[test]
fn small_stack_limits_never_stop_ferroni_where_c_completes() {
    for &(pattern, unit) in CASES {
        let (reg, c) = compile(pattern);
        for &count in COUNTS {
            for tail in ["z", "!"] {
                let subject = format!("{}{tail}", unit.repeat(count));
                let subject = subject.as_bytes();
                let unlimited = c_search(&c, subject, 0);
                assert_eq!(
                    ferroni_search(&reg, subject, 0),
                    unlimited,
                    "{pattern} on {count} x {unit:?} + {tail:?} without a limit"
                );
                for limit in limits() {
                    let ferroni = ferroni_search(&reg, subject, limit);
                    let c = c_search(&c, subject, limit);
                    let context = format!(
                        "{pattern} on {count} x {unit:?} + {tail:?} at stack limit {limit}"
                    );
                    if c.0 == ONIGERR_MATCH_STACK_LIMIT_OVER {
                        assert!(
                            ferroni == c || ferroni == unlimited,
                            "{context}: C stops, Ferroni returns {ferroni:?}"
                        );
                    } else {
                        assert_eq!(ferroni, c, "{context}");
                    }
                }
            }
        }
    }
}

/// A search that C completes under any limit: before the floor, Ferroni
/// stopped it at a limit of 1.
#[test]
fn shallow_searches_complete_under_a_limit_of_one() {
    for (pattern, subject) in [(r"(?:[a-z]+;)*z", "a;z"), (r"(a|b)*z", "abz"), (r"a", "a")] {
        let (reg, c) = compile(pattern);
        let subject = subject.as_bytes();
        let unlimited = c_search(&c, subject, 0);
        assert!(unlimited.0 >= 0, "{pattern} matches {subject:?}");
        assert_eq!(c_search(&c, subject, 1), unlimited, "C, {pattern}");
        assert_eq!(
            ferroni_search(&reg, subject, 1),
            unlimited,
            "Ferroni, {pattern}"
        );
    }
}

/// C's floor itself: under a limit of 320 a search stops at its 321st entry,
/// under any smaller limit it completes with 321 entries and stops at 322.
#[test]
fn limits_below_320_hold_321_entries_like_c() {
    let (reg, c) = compile(r"(?:a|b)*z");
    // `(?:a|b)*z` holds the bottom entry, one alternative per `a` and two
    // more entries, so 318 repetitions hold 321 entries.
    for (count, completes_below_320) in [(318, true), (319, false)] {
        let subject = format!("{}z", "a".repeat(count));
        let subject = subject.as_bytes();
        for limit in [1, 2, 160, 319] {
            let expected = if completes_below_320 {
                0
            } else {
                ONIGERR_MATCH_STACK_LIMIT_OVER
            };
            assert_eq!(
                c_search(&c, subject, limit).0,
                expected,
                "C, {count} at {limit}"
            );
            assert_eq!(
                ferroni_search(&reg, subject, limit).0,
                expected,
                "Ferroni, {count} at {limit}"
            );
        }
        assert_eq!(
            c_search(&c, subject, 320).0,
            ONIGERR_MATCH_STACK_LIMIT_OVER,
            "C, {count} at 320"
        );
        assert_eq!(
            ferroni_search(&reg, subject, 320).0,
            ONIGERR_MATCH_STACK_LIMIT_OVER,
            "Ferroni, {count} at 320"
        );
    }
}
