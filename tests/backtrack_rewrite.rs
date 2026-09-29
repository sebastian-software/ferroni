//! Decimal rewrites retain bounds, captures, and priority.

#[path = "fixtures/decimal_patterns.rs"]
mod patterns;

use ferroni::api::{Regex, SearchOptions};
use ferroni::backtrack_rewrite::{BacktrackingRewrite, BacktrackingRewriteRefusal};
use ferroni::oniguruma::*;
use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions};

type Trace = Option<Vec<Option<(usize, usize)>>>;

fn trace(re: &Regex, input: &str) -> Trace {
    re.captures_with(
        input,
        SearchOptions::new()
            .retry_limit_in_match(0)
            .retry_limit_in_search(0),
    )
    .unwrap()
    .map(|captures| {
        (0..captures.len())
            .map(|i| captures.get(i).map(|m| (m.start(), m.end())))
            .collect()
    })
}

fn optimized(pattern: &str) -> Regex {
    Regex::builder(pattern)
        .optimize_backtracking(true)
        .build()
        .unwrap()
}

#[test]
fn decimal_rewrites_are_explicit_and_keep_original_warnings() {
    for pattern in [patterns::V_FLOAT, patterns::V_EXPONENT] {
        let plain = Regex::new(pattern).unwrap();
        let fast = optimized(pattern);
        assert!(plain.backtracking_rewrites().is_empty());
        assert_eq!(
            fast.backtracking_rewrites(),
            [BacktrackingRewrite::PossessiveDecimalDigits]
        );
        assert_eq!(fast.backtracking_warnings(), plain.backtracking_warnings());
        assert_eq!(fast.captures_len(), plain.captures_len());
        assert!(
            Regex::builder(pattern)
                .optimize_backtracking(true)
                .optimize_backtracking(false)
                .build()
                .unwrap()
                .backtracking_rewrites()
                .is_empty()
        );
        assert!(
            Regex::builder(pattern)
                .optimize_backtracking(true)
                .reject_backtracking_risks(true)
                .build()
                .is_err()
        );
    }
    // A capture inside the repeated primitive requires the original
    // outer-atomic fallback; its capture must still be the last digit.
    let fallback = optimized(r"((([0-9])+)((_)?))+(\.)([0-9]*)");
    assert_eq!(
        fallback.backtracking_rewrites(),
        [BacktrackingRewrite::AtomicDecimalLoop]
    );
    let input = "12_34.56";
    assert_eq!(
        trace(&fallback, input),
        trace(
            &Regex::new(r"((([0-9])+)((_)?))+(\.)([0-9]*)").unwrap(),
            input
        )
    );
    let fast = optimized(patterns::V_FLOAT);
    assert_eq!(
        trace(&fast, "12_34.56"),
        Some(vec![
            Some((0, 8)),
            Some((3, 5)),
            Some((5, 5)),
            Some((5, 6)),
            Some((6, 8)),
        ])
    );
    let plain = Regex::new(patterns::PURESCRIPT_INTEGER).unwrap();
    let fast = optimized(patterns::PURESCRIPT_INTEGER);
    assert_eq!(
        fast.backtracking_rewrites(),
        [BacktrackingRewrite::DeterministicDecimalTail]
    );
    assert_eq!(fast.captures_len(), plain.captures_len());
    assert_eq!(fast.backtracking_warnings(), plain.backtracking_warnings());
    assert!(plain.backtracking_rewrites().is_empty());
    // The original last prefix capture excludes the digit returned to the tail.
    assert_eq!(
        trace(&fast, "123"),
        Some(vec![Some((0, 3)), Some((0, 3)), Some((0, 2)), None])
    );
    for input in [
        "1",
        "12_3",
        "12_34",
        "0x1f",
        "0X7F",
        "0o71",
        "0O7",
        "01_",
        "01x",
        "é01",
        "01é",
        "01\u{301}",
    ] {
        assert_eq!(trace(&fast, input), trace(&plain, input), "{input:?}");
    }
    let body = format!("{}[0-9]+_?{}", "(".repeat(34), ")".repeat(34));
    let pattern = format!(r"\b{body}*[0-9]+\b");
    let plain = Regex::new(&pattern).unwrap();
    let fast = optimized(&pattern);
    assert_eq!(
        fast.backtracking_rewrites(),
        [BacktrackingRewrite::DeterministicDecimalTail]
    );
    for input in ["1", "12", "12_3", "12_34", "123x"] {
        assert_eq!(
            trace(&fast, input),
            trace(&plain, input),
            "high capture numbers: {input}"
        );
        #[cfg(feature = "ffi")]
        {
            let c = ferroni::ffi::CRegex::new(pattern.as_bytes(), 0).unwrap();
            let mut region = ferroni::ffi::CRegion::new();
            let result = c.search(input.as_bytes(), 0, input.len(), Some(&mut region), 0);
            assert!(result >= -1);
            let c_trace = (result >= 0).then(|| {
                region
                    .capture_ranges()
                    .into_iter()
                    .map(|(a, b)| (a >= 0).then_some((a as usize, b as usize)))
                    .collect()
            });
            assert_eq!(
                trace(&fast, input),
                c_trace,
                "C high capture numbers: {input}"
            );
        }
    }
    for pattern in [r"\b(([0-9]+_?)*)[0-9]+\b", r"\b([0-9]+(_?))*[0-9]+\b"] {
        assert_eq!(
            optimized(pattern).backtracking_rewrites(),
            [BacktrackingRewrite::AtomicDecimalTail]
        );
    }
    // Possessifying the inner run prevents returning the required last digit.
    assert_ne!(
        trace(&Regex::new(r"\A([0-9]++_?)*[0-9]+\z").unwrap(), "123"),
        trace(&Regex::new(r"\A([0-9]+_?)*[0-9]+\z").unwrap(), "123")
    );
}

#[test]
fn first_byte_disjointness_is_not_used_as_a_safety_proof() {
    let pattern = r"\A(?:a|ab)+c\z";
    let fast = optimized(pattern);
    assert!(fast.backtracking_rewrites().is_empty());
    assert_eq!(fast.find("abc").unwrap().as_str(), "abc");
    assert!(
        Regex::new(r"\A(?>(?:a|ab)+)c\z")
            .unwrap()
            .find("abc")
            .is_none()
    );
}

#[test]
fn unsupported_shapes_and_giving_digits_back_stay_unchanged() {
    for pattern in [
        r"(?:[0-9]+_?)*\.[0-9]+", // outer star is outside the decimal rule
        r"(?:[0-9]+_?)+?\.[0-9]+",
        r"(?:[0-9]+?_?)+\.[0-9]+",
        r"(?:[0-9]+_??)+\.[0-9]+",
        r"(?:[0-9]{1,3}_?)+\.[0-9]+",
        r"(?:[0-9]+_?){2,}\.[0-9]+",
        r"(?:[0-9_]+_?)+\.[0-9]+",
        r"(?:[0-9]+__?)+\.[0-9]+",
        r"(?:\d+_?)+\.[0-9]+",
        r"(?:[0-9]+_?|[0-9])+\.[0-9]+",
    ] {
        assert!(
            optimized(pattern).backtracking_rewrites().is_empty(),
            "{pattern}"
        );
    }
    for pattern in [
        r"(?:[0-9]+_?)+[0-9]",
        r"(?:[0-9]+_?)+\.?[0-9]+",
        r"(?:[0-9]+_?)+[.][0-9]+",
        r"(?:[0-9]+_?)+$",
        r"(?:[0-9]+_?)+",
        r"(?:[0-9]+_?)+\b\.[0-9]+",
    ] {
        assert_eq!(
            optimized(pattern).backtracking_rewrites(),
            [BacktrackingRewrite::Refused(
                BacktrackingRewriteRefusal::NoMandatoryDot
            )],
            "{pattern}"
        );
    }
    for pattern in [
        r"([0-9]+_?)+\.[0-9]+\1",
        r"(?<n>[0-9]+_?)+\.[0-9]+\g<n>",
        r"(?=x)([0-9]+_?)+\.[0-9]+",
        r"([0-9]+_?)+\.[0-9]+(?<!x)",
        r"\G([0-9]+_?)+\.[0-9]+",
        r"([0-9]+_?)+\.[0-9]+\K",
        r"(?i:([0-9]+_?)+\.[0-9]+)",
        r"([0-9]+_?)+\.[0-9]+(?(1)x|y)",
        r"([0-9]+_?)+\.[0-9]+(?{x})",
        r"([0-9]+_?)+\.[0-9]+(?~x)",
        r"(?>([0-9]+_?)+\.[0-9]+)",
    ] {
        let re = optimized(pattern);
        assert!(
            re.backtracking_rewrites()
                .contains(&BacktrackingRewrite::Refused(
                    BacktrackingRewriteRefusal::UnsupportedConstruct
                )),
            "{pattern}: {:?}",
            re.backtracking_rewrites()
        );
    }
    for pattern in [
        r"([0-9]+_?)*[0-9]+",
        r"([0-9]+_?)*[0-9]+x",
        r"([0-9]+_?)*[0-9]+\B",
        r"([0-9]+_?)*[0-9]+[0-9]",
    ] {
        assert_eq!(
            optimized(pattern).backtracking_rewrites(),
            [BacktrackingRewrite::Refused(
                BacktrackingRewriteRefusal::NoWordBoundary
            )],
            "{pattern}"
        );
    }
    for pattern in [
        r"([0-9]+_?)*[0-9]+\b\1",
        r"(?i:([0-9]+_?)*[0-9]+\b)",
        r"(?=x)([0-9]+_?)*[0-9]+\b",
    ] {
        assert_eq!(
            optimized(pattern).backtracking_rewrites(),
            [BacktrackingRewrite::Refused(
                BacktrackingRewriteRefusal::UnsupportedConstruct
            )],
            "{pattern}"
        );
    }
    for option in [
        ONIG_OPTION_FIND_LONGEST,
        ONIG_OPTION_FIND_NOT_EMPTY,
        ONIG_OPTION_CALLBACK_EACH_MATCH,
    ] {
        let re = Regex::builder(patterns::V_FLOAT)
            .option(option)
            .optimize_backtracking(true)
            .build()
            .unwrap();
        assert_eq!(
            re.backtracking_rewrites(),
            [BacktrackingRewrite::Refused(
                BacktrackingRewriteRefusal::UnsupportedMode
            )]
        );
    }
}

fn differential_patterns() -> Vec<String> {
    vec![
        patterns::V_FLOAT.into(),
        patterns::V_EXPONENT.into(),
        patterns::PURESCRIPT_INTEGER.into(),
        r"\b(([0-9]+_?))*([0-9]+)\b".into(),
        r"\b([0-9]+_?)*([0-9])+\b".into(),
        r"\b(([0-9]+_?)*)[0-9]+\b".into(),
        r"(?:(?:([0-9]+_?)*[0-9]+)\b[ .]?|x)*".into(),
        r"\A(?:(?:([0-9]+_?)*[0-9]+)\b.|x)*x\z".into(),
        r"\A([0-9]+_?)*[0-9]+\b".into(),
        r"\b(([0-9]+)(_?))*([0-9]+)\b".into(),
        r"\b((([0-9])+)((_)?))*(([0-9])+)\b".into(),
        r"(?<prefix>[0-9]+_?)*(?<tail>[0-9]+)\b".into(),
        r"((([0-9]+_?)*[0-9]+)\b|[0-9]+)".into(),
        r"((([0-9]+_?)*[0-9]+)\b)*x?".into(),
        r"([0-9]+_?)*[0-9]+\b.([0-9]+_?)*[0-9]+\b".into(),
        r"\A([0-9]+(_?))+(\.)([0-9]+)\z".into(),
        r"(([0-9]+)(_?))+(\.)([0-9]*)".into(),
        r"((([0-9])+)((_)?))+(\.)([0-9]*)".into(),
        r"(?<digits>[0-9]+_?)+(?<dot>\.)(?<tail>[0-9]*)".into(),
        r"x?(([0-9]+_?)+)\.([0-9]*|x)".into(),
        r"(([0-9]+_?)+\.([0-9]+)|[0-9]+)".into(),
        r"(([0-9]+_?)+\.([0-9]*))*x?".into(),
        r"(([0-9]+_?)+\.([0-9]*)){1,2}".into(),
        r"((?:([0-9]+_?)+|x)\.([0-9]*))".into(),
        r"(?:[0-9]+_?)+\.[0-9]*(?:[0-9]+_?)+\.[0-9]*".into(),
    ]
}

fn enumerate_strings(max_len: usize, mut visit: impl FnMut(&str)) -> usize {
    fn walk(text: &mut String, remaining: usize, visit: &mut impl FnMut(&str), count: &mut usize) {
        *count += 1;
        visit(text);
        if remaining > 0 {
            for ch in ['0', '1', '_', '.', 'x', 'é'] {
                let len = text.len();
                text.push(ch);
                walk(text, remaining - 1, visit, count);
                text.truncate(len);
            }
        }
    }
    let mut count = 0;
    walk(&mut String::new(), max_len, &mut visit, &mut count);
    count
}

fn differential(max_len: usize) {
    let patterns = differential_patterns();
    let mut comparisons = 0;
    for pattern in &patterns {
        let plain = Regex::new(pattern).unwrap();
        let fast = optimized(pattern);
        assert!(
            fast.backtracking_rewrites().iter().any(|report| matches!(
                report,
                BacktrackingRewrite::PossessiveDecimalDigits
                    | BacktrackingRewrite::AtomicDecimalLoop
                    | BacktrackingRewrite::AtomicDecimalTail
                    | BacktrackingRewrite::DeterministicDecimalTail
            )),
            "{pattern}"
        );
        #[cfg(feature = "ffi")]
        let c =
            ferroni::ffi::CRegex::new(pattern.as_bytes(), ferroni::ffi::ONIG_OPTION_NONE).unwrap();
        #[cfg(feature = "ffi")]
        let mut c_region = ferroni::ffi::CRegion::new();
        comparisons += enumerate_strings(max_len, |text| {
            let expected = trace(&plain, text);
            assert_eq!(trace(&fast, text), expected, "{pattern} on {text:?}");
            #[cfg(feature = "ffi")]
            {
                let result = c.search(text.as_bytes(), 0, text.len(), Some(&mut c_region), 0);
                assert!(result >= -1, "C error {result}: {pattern} on {text:?}");
                let c_trace = (result >= 0).then(|| {
                    c_region
                        .capture_ranges()
                        .into_iter()
                        .map(|(start, end)| (start >= 0).then_some((start as usize, end as usize)))
                        .collect()
                });
                assert_eq!(expected, c_trace, "C: {pattern} on {text:?}");
            }
        });
    }
    eprintln!("{comparisons} pattern/input comparisons, maximum length {max_len}");
}

#[test]
fn exhaustive_short_strings_keep_every_capture() {
    differential(5);
}

#[test]
#[ignore = "extended experiment; run in release mode with --ignored --nocapture"]
fn exhaustive_extended_strings_keep_every_capture() {
    differential(7);
}

#[test]
fn long_successful_and_failed_inputs_keep_results_under_explicit_limits() {
    for pattern in [patterns::V_FLOAT, patterns::V_EXPONENT] {
        let plain = Regex::new(pattern).unwrap();
        let fast = optimized(pattern);
        for text in [
            format!("{}.1E+23", "123_456_".repeat(500)),
            format!("x__{}.1E+23", "123_456_".repeat(500)),
            format!("é{}.1E+23", "1".repeat(5_000)),
        ] {
            // These contain a valid suffix (possibly at a later position).
            assert_eq!(trace(&fast, &text), trace(&plain, &text));
        }
    }
    let plain = Regex::new(patterns::PURESCRIPT_INTEGER).unwrap();
    let fast = optimized(patterns::PURESCRIPT_INTEGER);
    for text in ["1".repeat(5000), format!("{}123", "123_456_".repeat(500))] {
        assert_eq!(trace(&fast, &text), trace(&plain, &text));
        let limited = fast
            .captures_with(
                &text,
                SearchOptions::new()
                    .match_stack_limit(64)
                    .retry_limit_in_match(0)
                    .retry_limit_in_search(0),
            )
            .unwrap();
        assert!(
            limited.is_some(),
            "the fused prefix needs no per-iteration stack entries"
        );
    }
    for ending in ["x", "_", "é"] {
        let text = format!("{}{}", "1".repeat(64), ending);
        let options = SearchOptions::new()
            .retry_limit_in_match(1000)
            .retry_limit_in_search(0);
        assert!(matches!(
            plain.find_with(&text, options),
            Err(ferroni::error::RegexError::RetryLimitInMatchOver)
        ));
        assert!(fast.find_with(&text, options).unwrap().is_none());
    }
    let pattern = format!(r"\A{}\z", patterns::V_FLOAT);
    let plain = Regex::new(&pattern).unwrap();
    let fast = optimized(&pattern);
    let text = format!("{}.x", "1".repeat(64));
    let options = SearchOptions::new()
        .retry_limit_in_match(1_000)
        .retry_limit_in_search(0);
    assert!(matches!(
        plain.find_with(&text, options),
        Err(ferroni::error::RegexError::RetryLimitInMatchOver)
    ));
    assert!(fast.find_with(&text, options).unwrap().is_none());
}

#[test]
fn scanner_keeps_capture_indices_pattern_priority_and_utf16_offsets() {
    let patterns = [
        patterns::V_EXPONENT,
        patterns::V_FLOAT,
        patterns::PURESCRIPT_INTEGER,
        r"[0-9]+",
        "x",
    ];
    let mut plain = Scanner::new(&patterns).unwrap();
    let mut fast =
        Scanner::with_backtracking_optimization(&patterns, &ScannerConfig::default()).unwrap();
    assert_eq!(plain.warnings(), fast.warnings());
    assert_eq!(
        fast.backtracking_rewrites()[0],
        [BacktrackingRewrite::PossessiveDecimalDigits]
    );
    assert_eq!(
        fast.backtracking_rewrites()[1],
        [BacktrackingRewrite::PossessiveDecimalDigits]
    );
    for text in [
        "😀 12_34.56E+78 9.0 x",
        "12__34.5",
        "0.0 1.1",
        "12_34_.56",
        "9.",
        "😀 12_34 0x1f 0o71 123x",
    ] {
        let string = OnigString::new(text);
        for start in 0..=string.utf16_len() {
            assert_eq!(
                fast.find_next_match_utf16(&string, start, ScannerFindOptions::NONE),
                plain.find_next_match_utf16(&string, start, ScannerFindOptions::NONE),
                "{text:?} at {start}"
            );
        }
    }
}
