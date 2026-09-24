// subexp_call_limits.rs - Subexpression call limits in search
//
// C: regexec.c OP_CALL fails once subexp_call_nest_counter reaches
// SubexpCallMaxNestLevel (onig_set_subexp_call_max_nest_level, default 20)
// and returns ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER once a search made more
// than SubexpCallLimitInSearch calls (onig_set_subexp_call_limit_in_search,
// default 0 = unlimited). Both limits are process-wide, so this file holds a
// single test that sets and restores them in order.
//
// Expectations checked against C Oniguruma (ONIG_SYNTAX_ONIGURUMA, UTF-8).

use ferroni::oniguruma::*;
use ferroni::prelude::{Regex, RegexError, SearchOptions};
use ferroni::regcomp::onig_new;
use ferroni::regexec::{
    onig_get_subexp_call_limit_in_search, onig_get_subexp_call_max_nest_level, onig_search,
    onig_set_subexp_call_limit_in_search, onig_set_subexp_call_max_nest_level,
};
use ferroni::regsyntax::OnigSyntaxOniguruma;

/// Search the whole subject; returns the result code and region 0/1 bounds.
fn search(pattern: &str, subject: &str) -> (i32, Option<(i32, i32)>) {
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
    let bounds = (r >= 0).then(|| {
        let region = region.unwrap();
        (region.beg[0], region.end[0])
    });
    (r, bounds)
}

fn nest_level_limits_recursion_depth() {
    assert_eq!(onig_get_subexp_call_max_nest_level(), 20);

    // The top-level occurrence of a called group is itself a call, so 20
    // nested calls consume 20 characters.
    let a20 = "a".repeat(20);
    let a21 = "a".repeat(21);
    assert_eq!(search(r"\A(?<a>a\g<a>?)\z", &a20), (0, Some((0, 20))));
    assert_eq!(search(r"\A(?<a>a\g<a>?)\z", &a21).0, ONIG_MISMATCH);
    assert_eq!(search(r"\A(?<a>a\g<a>|a)\z", &a21).0, ONIG_MISMATCH);
    assert_eq!(
        search(r"(?<a>a\g<a>?)", &"a".repeat(22)),
        (0, Some((0, 20)))
    );

    // Indirect recursion counts every call on the way down.
    assert_eq!(
        search(r"\A(?<a>a\g<b>?)(?<b>b\g<a>?)?\z", &"ab".repeat(10)),
        (0, Some((0, 20)))
    );
    assert_eq!(
        search(r"\A(?<a>a\g<b>?)(?<b>b\g<a>?)?\z", &"ab".repeat(20)).0,
        ONIG_MISMATCH
    );

    // Whole-pattern recursion: the match moves to where it fits.
    let ab21 = format!("{}{}", "a".repeat(21), "b".repeat(21));
    assert_eq!(search(r"a\g<0>?b", &ab21), (1, Some((1, 41))));

    // The nest limit is a plain failure, not an error.
    let re = Regex::new(r"\A(?<a>a\g<a>?)\z").unwrap();
    assert_eq!(re.is_match_with(&a21, SearchOptions::new()), Ok(false));

    onig_set_subexp_call_max_nest_level(5);
    assert_eq!(search(r"\A(?<a>a\g<a>?)\z", "aaaaa"), (0, Some((0, 5))));
    assert_eq!(search(r"\A(?<a>a\g<a>?)\z", "aaaaaa").0, ONIG_MISMATCH);

    // Level 0 forbids every call; a group that is never called still matches.
    onig_set_subexp_call_max_nest_level(0);
    assert_eq!(search(r"\A(?<a>a\g<a>?)\z", "a").0, ONIG_MISMATCH);
    assert_eq!(search(r"\A(?<a>a)\z", "a"), (0, Some((0, 1))));

    // A negative level compares as a huge unsigned value: unlimited.
    onig_set_subexp_call_max_nest_level(-1);
    assert_eq!(search(r"\A(?<a>a\g<a>?)\z", &a21), (0, Some((0, 21))));

    onig_set_subexp_call_max_nest_level(20);
}

fn limit_in_search_reports_an_error() {
    assert_eq!(onig_get_subexp_call_limit_in_search(), 0);

    onig_set_subexp_call_limit_in_search(10);
    assert_eq!(search(r"\A(?<a>a\g<a>?)\z", "aaaaaaaaa"), (0, Some((0, 9))));
    assert_eq!(
        search(r"\A(?<a>a\g<a>?)\z", "aaaaaaaaaa").0,
        ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER
    );

    // The counter covers the whole search, not a single start position.
    onig_set_subexp_call_limit_in_search(2);
    // "bac" alone needs two calls; after the failed attempt at 0 in "bxbac"
    // the third call of the search trips the limit.
    assert_eq!(search(r"(?<a>a|b\g<a>c)", "bac"), (0, Some((0, 3))));
    assert_eq!(
        search(r"(?<a>a|b\g<a>c)", "bxbac").0,
        ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER
    );

    let re = Regex::new(r"(?<a>a|b\g<a>c)").unwrap();
    assert_eq!(
        re.is_match_with("bbac", SearchOptions::new()),
        Err(RegexError::SubexpCallLimitOver)
    );

    onig_set_subexp_call_limit_in_search(0);
}

#[test]
fn subexp_call_limits_in_search() {
    nest_level_limits_recursion_depth();
    limit_in_search_reports_an_error();
}
