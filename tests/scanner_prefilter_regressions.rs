//! The review findings of the scanner's DFA pre-filter (PR #321, ADR-008),
//! as the reviewer reproduced them: with the pre-filter a scanner answers
//! as one without it, on every route, and under an explicit subexpression
//! call budget.
//!
//! The call budget is process-wide, so this file holds a single test that
//! runs the six reproductions in order and sets and restores the budget
//! last, as tests/subexp_call_limits.rs does.
use ferroni::scanner::{Scanner, ScannerConfig, ScannerFindOptions};

fn bounds(found: Option<ferroni::scanner::ScannerMatch>) -> Option<(usize, usize, usize)> {
    found.map(|m| (m.index, m.captures()[0].start, m.captures()[0].end))
}

fn compare(patterns: &[&str], text: &str, expected: (usize, usize, usize)) {
    let mut plain =
        Scanner::with_config(patterns, &ScannerConfig::default().prefilter(false)).unwrap();
    let mut filtered = Scanner::new(patterns).unwrap();
    assert_eq!(
        filtered.prefilter_stats().built,
        cfg!(feature = "dfa-prefilter")
    );
    let want = bounds(plain.find_next_match(text, 0, ScannerFindOptions::NONE));
    let got = bounds(filtered.find_next_match(text, 0, ScannerFindOptions::NONE));
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

fn identical_default_limit_searches_have_identical_results() {
    let mut scanner = Scanner::new(&[r"(a+)+b", "c"]).unwrap();
    let text = format!("{}c b", "a".repeat(27));
    let found: Vec<_> = (0..20)
        .map(|_| bounds(scanner.find_next_match_with_id(&text, 7, 0, ScannerFindOptions::NONE)))
        .collect();
    assert!(
        found.iter().all(|got| *got == found[0]),
        "identical searches returned different results: {found:?}"
    );
}

fn explicit_subexpression_call_limit_disables_skipping() {
    let saved = ferroni::regexec::onig_get_subexp_call_limit_in_search();
    ferroni::regexec::onig_set_subexp_call_limit_in_search(1);
    let patterns = [r"x(?<n>a){0}(?=\g<n>\g<n>)[ab]+[cd]", "c"];
    let text = "xaaa! c";
    let mut plain =
        Scanner::with_config(&patterns, &ScannerConfig::default().prefilter(false)).unwrap();
    let mut filtered = Scanner::new(&patterns).unwrap();
    let want = bounds(plain.find_next_match(text, 0, ScannerFindOptions::NONE));
    let got = bounds(filtered.find_next_match(text, 0, ScannerFindOptions::NONE));
    ferroni::regexec::onig_set_subexp_call_limit_in_search(saved);
    assert_eq!(want, None);
    assert_eq!(
        got, want,
        "the prefilter hid a subexpression call limit error"
    );
}

#[test]
fn review_findings_of_the_prefilter() {
    unicode_non_word_is_not_ascii_only();
    folded_trie_keeps_the_suffix_after_a_fold();
    folded_trie_reaches_the_following_anchor();
    conditional_preserves_the_consumed_condition();
    identical_default_limit_searches_have_identical_results();
    explicit_subexpression_call_limit_disables_skipping();
}
