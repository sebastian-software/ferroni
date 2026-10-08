//! The default case-fold flag in compilation.
//
// C: onig_new passes ONIGENC_CASE_FOLD_DEFAULT, the OnigDefaultCaseFoldFlag
// that onig_set_default_case_fold_flag stores, to onig_reg_init (regcomp.c).
// It starts as ONIGENC_CASE_FOLD_MIN. The flag is
// process-wide, so this file holds a single test that sets and restores it in
// order, as tests/subexp_call_limits.rs does.
//
// Expectations checked against C Oniguruma (ONIG_SYNTAX_ONIGURUMA, UTF-8);
// with the `ffi` feature the test repeats every case against C.

use ferroni::oniguruma::*;
use ferroni::prelude::Regex;
use ferroni::regcomp::{
    onig_get_default_case_fold_flag, onig_new, onig_set_default_case_fold_flag,
};
use ferroni::regexec::onig_search;
use ferroni::regsyntax::OnigSyntaxOniguruma;

type Span = Option<(i32, i32)>;

/// The flags the cases are run under, in the order of their expectations.
const FLAGS: [OnigCaseFoldType; 3] = [ONIGENC_CASE_FOLD_MIN, 0, ONIGENC_CASE_FOLD_ASCII_ONLY];

/// Pattern, subject, and the match span under each of `FLAGS`.
const CASES: &[(&str, &str, [Span; 3])] = &[
    // Multi-character folds need INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR.
    ("(?i)ß", "ss", [Some((0, 2)), None, None]),
    ("(?i)ss", "ß", [Some((0, 2)), None, None]),
    ("(?i)[ß]", "SS", [Some((0, 2)), None, None]),
    ("(?i)strasse", "straße", [Some((0, 7)), None, None]),
    ("(?i)ff", "ﬀ", [Some((0, 3)), None, None]),
    // An alternation of ASCII literals compiles to a folded trie (ADR-008).
    ("(?i)(?:foo|bar|ss)", "xß", [Some((1, 3)), None, None]),
    // Single-character folds outside ASCII stay without it, and stop under
    // ONIGENC_CASE_FOLD_ASCII_ONLY.
    ("(?i)k", "\u{212a}", [Some((0, 3)), Some((0, 3)), None]),
    ("(?i)[a-z]", "\u{212a}", [Some((0, 3)), Some((0, 3)), None]),
    ("(?i)é", "É", [Some((0, 2)), Some((0, 2)), None]),
    ("(?i)s", "ſ", [Some((0, 2)), Some((0, 2)), None]),
    // ASCII folds hold under every flag.
    ("(?i)a", "A", [Some((0, 1)), Some((0, 1)), Some((0, 1))]),
];

/// Search the whole subject; returns the bounds of region 0.
fn find(pattern: &str, subject: &str) -> Span {
    let reg = onig_new(
        pattern.as_bytes(),
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
    )
    .unwrap();
    let text = subject.as_bytes();
    let (r, region) = onig_search(
        &reg,
        text,
        text.len(),
        0,
        text.len(),
        Some(OnigRegion::new()),
        ONIG_OPTION_NONE,
    );
    (r >= 0).then(|| {
        let region = region.unwrap();
        (region.beg[0], region.end[0])
    })
}

/// The same search in C Oniguruma.
#[cfg(feature = "ffi")]
fn c_find(pattern: &str, subject: &str) -> Span {
    let reg =
        ferroni::ffi::CRegex::new(pattern.as_bytes(), ferroni::ffi::ONIG_OPTION_NONE).unwrap();
    let mut region = ferroni::ffi::CRegion::new();
    let r = reg.search(subject.as_bytes(), 0, subject.len(), Some(&mut region), 0);
    assert!(r >= ONIG_MISMATCH, "C error {r}: {pattern} on {subject:?}");
    (r >= 0).then(|| region.capture_ranges()[0])
}

/// Sets the default flag in Ferroni and, with the `ffi` feature, in C.
fn set_default_case_fold_flag(flag: OnigCaseFoldType) {
    assert_eq!(onig_set_default_case_fold_flag(flag), 0);
    assert_eq!(onig_get_default_case_fold_flag(), flag);
    #[cfg(feature = "ffi")]
    {
        // SAFETY: the setter stores into a C global and takes no pointers.
        // This file's single test is the only code in the process that calls
        // into C, so the store does not race a C compilation.
        let r = unsafe { ferroni::ffi::onig_set_default_case_fold_flag(flag) };
        assert_eq!(r, 0);
    }
}

#[test]
fn the_default_case_fold_flag_applies_to_patterns_compiled_after_it_is_set() {
    assert_eq!(onig_get_default_case_fold_flag(), ONIGENC_CASE_FOLD_MIN);

    for (i, &flag) in FLAGS.iter().enumerate() {
        set_default_case_fold_flag(flag);
        for &(pattern, subject, expected) in CASES {
            assert_eq!(
                find(pattern, subject),
                expected[i],
                "flag {flag:#x}: {pattern} on {subject:?}"
            );
            #[cfg(feature = "ffi")]
            assert_eq!(
                c_find(pattern, subject),
                expected[i],
                "C, flag {flag:#x}: {pattern} on {subject:?}"
            );
        }
    }

    // The Rust API compiles through the same entry point.
    set_default_case_fold_flag(0);
    assert!(!Regex::new("(?i)ß").unwrap().is_match("ss"));
    assert!(Regex::new("(?i)k").unwrap().is_match("\u{212a}"));

    // A regex compiled earlier keeps the flag it was compiled with.
    set_default_case_fold_flag(ONIGENC_CASE_FOLD_MIN);
    let sharp_s = Regex::new("(?i)ß").unwrap();
    set_default_case_fold_flag(0);
    assert!(sharp_s.is_match("ss"));

    set_default_case_fold_flag(ONIGENC_CASE_FOLD_MIN);
    assert!(Regex::new("(?i)ß").unwrap().is_match("ss"));
}
