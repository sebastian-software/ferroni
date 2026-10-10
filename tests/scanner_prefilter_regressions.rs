//! The review findings of the scanner's DFA pre-filter (PR #321, ADR-008),
//! as the reviewer reproduced them: with the pre-filter a scanner answers
//! as one without it, on every route, and under an explicit subexpression
//! call budget.
//!
//! The call budget is process-wide, so this file holds a single test that
//! runs the reproductions in order and sets and restores the budget last,
//! as tests/subexp_call_limits.rs does.
use ferroni::scanner::{Scanner, ScannerConfig, ScannerFindOptions};

fn bounds(found: Option<ferroni::scanner::ScannerMatch>) -> Option<(usize, usize, usize)> {
    found.map(|m| (m.index, m.captures()[0].start, m.captures()[0].end))
}

fn compare(patterns: &[&str], text: &str, expected: (usize, usize, usize)) {
    let mut plain =
        Scanner::with_config(patterns, &ScannerConfig::default().prefilter(false)).unwrap();
    let mut filtered =
        Scanner::with_config(patterns, &ScannerConfig::default().prefilter_warmup(0)).unwrap();
    let want = bounds(plain.find_next_match(text, 0, ScannerFindOptions::NONE));
    let got = bounds(filtered.find_next_match(text, 0, ScannerFindOptions::NONE));
    // The first search built the automata.
    assert_eq!(
        filtered.prefilter_stats().built,
        cfg!(feature = "dfa-prefilter")
    );
    assert_eq!(want, Some(expected));
    assert_eq!(got, want, "patterns={patterns:?}, text={text:?}");
}

fn unicode_non_word_is_not_ascii_only() {
    compare(&[r"\W", "a"], "😀a", (0, 0, 4));
}

fn folded_trie_keeps_the_suffix_after_a_fold() {
    compare(
        &[r"(?i)(?:kelvin|street|fiat|xyz)!", "!"],
        "Kelvin!",
        (0, 0, 9),
    );
}

fn folded_trie_reaches_the_following_anchor() {
    compare(
        &[r"(?i)(?:kelvin|street|fiat|xyz)$", "!"],
        "Kelvin",
        (0, 0, 8),
    );
}

fn conditional_preserves_the_consumed_condition() {
    compare(&[r"(?(a)b|c)", "b"], "ab", (0, 0, 2));
}

/// Identical calls through the cache route, which probes the per-regex
/// route after a run of same-start calls, answer alike under the default
/// retry limit, which `(a+)+b` exhausts on this text wherever it is
/// attempted. Two scanners: under the default configuration the two
/// patterns are a tiny set (ADR-008, _Warm-up and tiny sets_), so it never
/// builds the pre-filter and every call answers no match; with the
/// `dfa-prefilter` feature and `prefilter_warmup(0)` it builds the pre-filter
/// at once, every call stays
/// on the route the pre-filter decides (`onig_regset_prefilter_decides`)
/// and answers `c`. Neither crosses the end of a warm-up, where a set past
/// the tiny bound changes its answer to such calls once (ADR-008,
/// _Observability_).
fn identical_default_limit_searches_have_identical_results() {
    let text = format!("{}c b", "a".repeat(27));
    for config in [
        ScannerConfig::default(),
        ScannerConfig::default().prefilter_warmup(0),
    ] {
        let mut scanner = Scanner::with_config(&[r"(a+)+b", "c"], &config).unwrap();
        let found: Vec<_> = (0..20)
            .map(|_| bounds(scanner.find_next_match_with_id(&text, 7, 0, ScannerFindOptions::NONE)))
            .collect();
        assert!(
            found.iter().all(|got| *got == found[0]),
            "identical searches returned different results ({config:?}): {found:?}"
        );
    }
}

fn explicit_subexpression_call_limit_disables_skipping() {
    let saved = ferroni::regexec::onig_get_subexp_call_limit_in_search();
    ferroni::regexec::onig_set_subexp_call_limit_in_search(1);
    let patterns = [r"x(?<n>a){0}(?=\g<n>\g<n>)[ab]+[cd]", "c"];
    let text = "xaaa! c";
    let mut plain =
        Scanner::with_config(&patterns, &ScannerConfig::default().prefilter(false)).unwrap();
    let mut filtered =
        Scanner::with_config(&patterns, &ScannerConfig::default().prefilter_warmup(0)).unwrap();
    let want = bounds(plain.find_next_match(text, 0, ScannerFindOptions::NONE));
    let got = bounds(filtered.find_next_match(text, 0, ScannerFindOptions::NONE));
    ferroni::regexec::onig_set_subexp_call_limit_in_search(saved);
    assert_eq!(want, None);
    assert_eq!(
        got, want,
        "the prefilter hid a subexpression call limit error"
    );
}

/// The second review's `compare`: from a given start.
fn compare_from(patterns: &[&str], text: &str, start: usize, expected: (usize, usize, usize)) {
    let mut off =
        Scanner::with_config(patterns, &ScannerConfig::default().prefilter(false)).unwrap();
    let mut on =
        Scanner::with_config(patterns, &ScannerConfig::default().prefilter_warmup(0)).unwrap();
    let want = bounds(off.find_next_match(text, start, ScannerFindOptions::NONE));
    let got = bounds(on.find_next_match(text, start, ScannerFindOptions::NONE));
    assert_eq!(want, Some(expected));
    assert_eq!(got, want);
}

fn interior_byte_offset() {
    compare_from(&["."], "é", 1, (0, 1, 2));
}

fn conditional_optimizer_admission() {
    compare_from(&[r"(?(a)(?:b|c))!", "!"], "ac!", 0, (0, 2, 3));
}

/// The `prefilter-differential` fuzz target's first finding, through the
/// self-check: a winner without capture groups or `\K` is rebuilt from the
/// attempt position and the match length, and the position-lead search
/// leaves its region unwritten where the pre-filter's attempt fills it.
/// The scanner never reads that region, so both routes report the match
/// alike, and the self-check compares registers only where they are read.
fn winner_without_captures_on_a_long_subject() {
    let text = format!("dd\n{}\ndda>$\nbcaa\naaaabc", "y".repeat(250));
    let last_line = text.rfind('\n').unwrap() + 1;
    compare_from(&[r".+abc"], &text, 1, (0, last_line, last_line + 6));
}

#[test]
fn review_findings_of_the_prefilter() {
    unicode_non_word_is_not_ascii_only();
    folded_trie_keeps_the_suffix_after_a_fold();
    folded_trie_reaches_the_following_anchor();
    conditional_preserves_the_consumed_condition();
    identical_default_limit_searches_have_identical_results();
    interior_byte_offset();
    conditional_optimizer_admission();
    winner_without_captures_on_a_long_subject();
    explicit_subexpression_call_limit_disables_skipping();
}
