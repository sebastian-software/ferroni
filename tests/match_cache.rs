#![cfg(feature = "match-cache")]

use ferroni::api::{Regex, SearchOptions};
use ferroni::match_cache::MatchCacheConfig;

fn cached(pattern: &str) -> Regex {
    Regex::builder(pattern)
        .match_cache(MatchCacheConfig::new().activation_threshold(0))
        .build()
        .unwrap()
}

#[test]
fn eligibility_rejects_stateful_bytecode() {
    for pattern in [
        r"(a+)\1",
        r"(?=a)a",
        r"(?<=a)b",
        r"(?>a|ab)c",
        r"\Ga",
        r"a\Kb",
        r"(a?)*b",
    ] {
        assert!(!cached(pattern).is_linear_time(), "{pattern}");
    }
    for pattern in [
        r"(a+)+$",
        r"(a|aa)*$",
        r"(?:\w*,)*x",
        r"(?:\w+\s*,\s*)*\w+\s*$",
    ] {
        assert!(cached(pattern).is_linear_time(), "{pattern}");
    }
}

#[test]
fn cache_respects_retry_budgets_for_nested_repeats_and_guarded_stars() {
    let text = format!("{}!", "a".repeat(8192));
    let limits = SearchOptions::new()
        .retry_limit_in_match(200_000)
        .retry_limit_in_search(1_000_000);
    for pattern in [r"(a+)+$", r"(a|aa)*$", r"(?:\w+\s*,\s*)*\w+\s*$"] {
        let plain = Regex::new(pattern).unwrap();
        assert!(
            plain.find_with(&text, limits).is_err(),
            "baseline must exhaust its budget: {pattern}"
        );
        let cached = cached(pattern);
        let found = cached
            .find_with(&text, limits)
            .unwrap_or_else(|e| panic!("{pattern}: {e}"));
        // The first two patterns can match the empty suffix at the end.
        let expected = if pattern == r"(a|aa)*$" {
            Some(text.len()..text.len())
        } else {
            None
        };
        assert_eq!(found.map(|m| m.range()), expected, "{pattern}");
    }

    // A star's peek guard proves which suffix attempts cannot match. Keeping
    // that guard matters even after activation: trying the impossible exits
    // could exhaust a small per-attempt retry limit that the plain VM meets.
    let text = format!("{}c", "a".repeat(512));
    let limits = SearchOptions::new()
        .retry_limit_in_match(8)
        .retry_limit_in_search(10_000);
    for pattern in [r".*b|c", r"[ab]*b|c"] {
        let plain = Regex::new(pattern).unwrap();
        assert_eq!(
            plain.find_with(&text, limits).unwrap().map(|m| m.range()),
            cached(pattern)
                .find_with(&text, limits)
                .unwrap()
                .map(|m| m.range()),
            "{pattern}"
        );
    }
}

#[test]
fn cache_and_plain_matcher_agree_on_captures_and_reused_allocations() {
    let patterns = [
        r"(a|aa)*$",
        r"(a+)+$",
        r"(a(b)?)+c|a+",
        r"((ab|a)+)(b?)",
        r"(?i)(a|ä|ss)+$",
        r"([^x]*)x",
        r"(\w+)(\s*)(\w*)",
        r"(.*)(a|b)$",
        r"(a|b)*?b",
        r"([é😀]+)(é?)",
        r"(?:\w*,)*x",
    ];
    let limits = SearchOptions::new().retry_limit_in_match(1_000_000);
    for pattern in patterns {
        let plain = Regex::new(pattern).unwrap();
        let memoized = cached(pattern);
        for text in [
            "",
            "a",
            "ab",
            "aaaa!",
            "aababc",
            "abb",
            "aa\nb",
            "äSS",
            "é😀é",
            "word word",
            "ax",
        ] {
            let snapshot = |re: &Regex| {
                re.captures_with(text, limits).map(|result| {
                    result.map(|caps| {
                        caps.iter()
                            .map(|m| m.map(|m| m.range()))
                            .collect::<Vec<_>>()
                    })
                })
            };
            assert_eq!(
                snapshot(&plain),
                snapshot(&memoized),
                "{pattern:?} {text:?}"
            );
        }
        // Same address, length and capacity, different contents: no address-keyed reuse.
        let mut text = String::from("aaa");
        assert_eq!(
            plain.find(&text).map(|m| m.range()),
            memoized.find(&text).map(|m| m.range())
        );
        text.replace_range(.., "bbb");
        assert_eq!(
            plain.find(&text).map(|m| m.range()),
            memoized.find(&text).map(|m| m.range())
        );
    }
}

#[test]
fn exhausted_cache_budget_preserves_limit_errors() {
    let re = Regex::builder(r"(a+)+$")
        .match_cache(
            MatchCacheConfig::new()
                .memory_budget(1)
                .activation_threshold(0),
        )
        .build()
        .unwrap();
    let limits = SearchOptions::new().retry_limit_in_match(1000);
    let text = format!("{}!", "a".repeat(64));
    assert_eq!(
        Regex::new(r"(a+)+$")
            .unwrap()
            .find_with(&text, limits)
            .unwrap_err(),
        re.find_with(&text, limits).unwrap_err()
    );
}

#[test]
fn scanner_reuses_only_immutable_subjects_and_shares_one_budget() {
    use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions};
    let patterns = [r"(a+)+$", r"(?:\w+\s*,\s*)*\w+\s*$", r"!", r"(é|😀)+"];
    let config = MatchCacheConfig::new()
        .memory_budget(2 * 1024 * 1024)
        .activation_threshold(0);
    let mut memoized =
        Scanner::with_match_cache(&patterns, &ScannerConfig::default(), config).unwrap();
    let mut plain = Scanner::new(&patterns).unwrap();
    for text in ["aaaa!", "abc, def !", "a😀é!", "aaa", "bbb!"] {
        let subject = OnigString::new(text);
        // Include repeated, advancing, and backward-moving starts on one subject.
        for start in [0, 1, 2, 0, 3, 1, 0] {
            let expected = plain.find_next_match_utf16(&subject, start, ScannerFindOptions::NONE);
            let actual = memoized.find_next_match_utf16(&subject, start, ScannerFindOptions::NONE);
            assert_eq!(
                format!("{actual:?}"),
                format!("{expected:?}"),
                "{text:?} {start}"
            );
            assert!(memoized.match_cache_bytes() <= 2 * 1024 * 1024);
        }
    }
    // A new subject drops the previous allocations even if an optimizer can
    // answer without entering the VM.
    let empty = OnigString::new("");
    memoized.find_next_match_utf16(&empty, 0, ScannerFindOptions::NONE);
    assert_eq!(memoized.match_cache_bytes(), 0);
}

#[test]
fn unsupported_modes_keep_the_original_limits_and_results() {
    use ferroni::oniguruma::*;
    use ferroni::regexec::onig_search;
    for flag in [ONIG_OPTION_FIND_LONGEST, ONIG_OPTION_FIND_NOT_EMPTY] {
        assert!(
            !Regex::builder("a*")
                .option(flag)
                .build()
                .unwrap()
                .is_linear_time()
        );
    }
    let plain = Regex::new(r"(a+)+$").unwrap();
    let memoized = cached(r"(a+)+$");
    let text = b"aaaa!";
    for start in 0..=text.len() {
        for range in 0..=text.len() {
            for option in [
                ONIG_OPTION_NONE,
                ONIG_OPTION_FIND_LONGEST,
                ONIG_OPTION_FIND_NOT_EMPTY,
            ] {
                let search = |re: &Regex| {
                    let (result, region) = onig_search(
                        re.as_raw(),
                        text,
                        text.len(),
                        start,
                        range,
                        Some(OnigRegion::new()),
                        option,
                    );
                    (result, region.map(|r| (r.beg, r.end)))
                };
                assert_eq!(
                    search(&plain),
                    search(&memoized),
                    "{start} {range} {option:?}"
                );
            }
        }
    }
    let limited = SearchOptions::new().match_stack_limit(10);
    let text = format!("{}!", "a".repeat(64));
    assert_eq!(
        plain.find_with(&text, limited).unwrap_err(),
        memoized.find_with(&text, limited).unwrap_err()
    );
}
