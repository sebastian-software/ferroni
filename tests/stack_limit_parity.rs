//! Rule 3 of ADR-017 under the match stack limit: where C Oniguruma completes,
//! Ferroni completes with the same result.
//!
//! Possessive repeats of one character (C's automatic possessification in
//! `tune_next`, `a*+`, `(?>a*)`) are the cases C compiles to a PUSH/POP loop,
//! which needs no stack per iteration.
//!
//! C checks the limit only when its stack grows past the first reallocation:
//! it starts with `INIT_MATCH_STACK_SIZE` (160) entries and doubles once to
//! 320 without a check, and with a smaller limit it shrinks the stack below
//! the entries in use. The comparison therefore starts at 320 entries, where
//! both engines hold at most `limit` entries.
#![cfg(feature = "ffi")]

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::ffi::{CRegex, CRegion};
use ferroni::oniguruma::*;
use ferroni::regcomp::onig_new;
use ferroni::regexec::{
    onig_new_match_param, onig_search_with_param, onig_set_match_stack_limit_size_of_match_param,
};
use ferroni::regsyntax::OnigSyntaxOniguruma;

/// The smallest limit that C observes (twice `INIT_MATCH_STACK_SIZE`).
const C_LIMIT_FLOOR: u32 = 320;

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

/// Repeated groups whose body ends in a possessive one-character repeat,
/// with the unit that repeats them.
const CASES: &[(&str, &str)] = &[
    // Possessive through C's `tune_next`: `;` cannot start `[a-z]`.
    (r"(?:[a-z]+;)*z", "a;"),
    (r"(?:[a-z]+;)*z", "abc;"),
    (r"(?:[a-z]*;)*z", "ab;"),
    (r"(?:a+b)*c", "ab"),
    (r"(?:x*y)+z", "xxy"),
    (r"(?:[^;]+;)*z", "é;"),
    // Possessive in the source.
    (r"(?:[a-z]++;)*z", "a;"),
    (r"(?:[a-z]*+;)*z", "ab;"),
    (r"(?:(?>[a-z]+);)*z", "abc;"),
    (r"(?:(?>\d*),)*z", "12,"),
    (r"(?:a++)*z", "a"),
    // Bodies without a star instruction, which run C's PUSH/POP loop.
    (r"(?:(?>(?:ab)+)c)*z", "ababc"),
    (r"(?:\S++ )*z", "a1 "),
    (r"(?:é*+;)*z", "éé;"),
    // With a capture around the repeat, which C needs a stack entry for in
    // every pass: the limit is reached in both engines.
    (r"(?:([a-z]+);)*z", "ab;"),
    // Not possessive in C (`\s` is not exclusive with `\w` there): one entry
    // per pass in both engines.
    (r"(?:\w+\s)*z", "ab "),
];

#[test]
fn possessive_repeats_never_need_more_stack_than_c() {
    for &(pattern, unit) in CASES {
        let reg = onig_new(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
        )
        .unwrap_or_else(|e| panic!("{pattern} compiles: {e:?}"));
        let c = CRegex::new(pattern.as_bytes(), ferroni::ffi::ONIG_OPTION_NONE).unwrap();
        for count in [1, 10, 1_000] {
            for tail in ["z", "", "!"] {
                let subject = format!("{}{tail}", unit.repeat(count));
                let subject = subject.as_bytes();
                let least = (C_LIMIT_FLOOR..=100_000)
                    .find(|&limit| c_search(&c, subject, limit).0 != ONIGERR_MATCH_STACK_LIMIT_OVER)
                    .expect("C completes under some limit");
                for limit in [least, least + 1, least * 2] {
                    assert_eq!(
                        ferroni_search(&reg, subject, limit),
                        c_search(&c, subject, limit),
                        "{pattern} on {count} x {unit:?} + {tail:?} at stack limit {limit} \
                         (C's least is {least})"
                    );
                }
            }
        }
    }
}
