//! Look-behinds and the character map search over malformed UTF-8 give C
//! Oniguruma's results (ADR-001). C's map_search steps by each lead byte's
//! length, so it never tries a map byte that a malformed lead byte covers;
//! a look-behind steps back with `onigenc_step_back` to the lead byte and
//! decodes the character from there.

#![cfg(feature = "ffi")]

use ferroni::api::Regex;
use ferroni::ffi::{CRegex, CRegion};

type Trace = Option<Vec<Option<(usize, usize)>>>;

/// Ferroni's search result and captures, as `CRegex::search` reports them.
fn ferroni_trace(re: &Regex, text: &[u8]) -> Trace {
    re.captures_bytes(text).map(|captures| {
        (0..captures.len())
            .map(|i| captures.get(i).map(|m| (m.start(), m.end())))
            .collect()
    })
}

fn c_trace(re: &CRegex, text: &[u8]) -> Trace {
    let mut region = CRegion::new();
    let result = re.search(text, 0, text.len(), Some(&mut region), 0);
    assert!(result >= -1, "C error {result} on {text:?}");
    (result >= 0).then(|| {
        region
            .capture_ranges()
            .into_iter()
            .map(|(begin, end)| (begin >= 0).then_some((begin as usize, end as usize)))
            .collect()
    })
}

/// Asserts that Ferroni's search and `is_match` agree with C, and returns
/// C's whole match.
fn assert_agrees_with_c(pattern: &str, subject: &[u8]) -> Option<(usize, usize)> {
    let ours = Regex::new(pattern).unwrap();
    let c = CRegex::new(pattern.as_bytes(), 0).unwrap();
    let expected = c_trace(&c, subject);
    assert_eq!(
        ferroni_trace(&ours, subject),
        expected,
        "{pattern} on {subject:?}"
    );
    assert_eq!(
        ours.is_match_bytes(subject),
        expected.is_some(),
        "is_match {pattern} on {subject:?}"
    );
    expected.and_then(|captures| captures[0])
}

#[test]
fn map_search_skips_a_byte_inside_a_truncated_sequence() {
    // `e2` starts a three-byte character, so C steps from 0 to 3 and never
    // tries the `a` at 2.
    for pattern in [r"[ab]", r"(?i)a", r"(?:a|b)"] {
        assert_eq!(assert_agrees_with_c(pattern, b"\xe2\x82a"), None);
        assert_eq!(assert_agrees_with_c(pattern, b"\xe2\x82a a"), Some((4, 5)));
    }
    assert_eq!(
        assert_agrees_with_c(r"(?i)(a)(.*?)(?<!\w)\1", b"\xe2\x82a a"),
        None
    );
    // An exact string search is not a map search; C finds the `a`.
    assert_eq!(assert_agrees_with_c(r"a", b"\xe2\x82a"), Some((2, 3)));
}

#[test]
fn map_search_lines_up_with_c_before_a_case_insensitive_back_reference() {
    let subjects: [&[u8]; 2] = [
        b"\xe2\x82a1\xc31\xc3\xe2\x821\xc3\xc3\xc3\xc3\xa9 ax\xe2\x82\xc3",
        b"\xe2\x821\xffb\xc3\xa9 \n \xff\n\xc3a a1\xc3a1\xc3\xa9",
    ];
    for pattern in [r"(?i)(a)(.*?)(?<!\w)\1", r"(?:(a)|b)(.*?)(?<!\w)\1"] {
        for subject in subjects {
            assert_eq!(assert_agrees_with_c(pattern, subject), None);
        }
    }
}

#[test]
fn word_look_behind_differs_from_c_only_by_unicode_data() {
    // `(?<!\w)(_)(?!_)(.+?)(?<!_)\1(?!\w)` matches 5..10 in C but not in
    // Ferroni on `_b\n\xe2\x82_\xc3\xa9b_`. The look-behind at 5 decodes
    // U+209F in both; Unicode 17 assigned it (LATIN SUBSCRIPT SMALL LETTER
    // Z, a word character), which C's older tables do not know (ADR-015).
    let ours = Regex::new(r"\w").unwrap();
    let c = CRegex::new(br"\w", 0).unwrap();
    let well_formed = "\u{209f}".as_bytes();
    assert!(ours.is_match_bytes(well_formed));
    assert!(c_trace(&c, well_formed).is_none());
    // With a truncated sequence both tables classify alike, `e2 84 5f` as
    // U+211F RESPONSE, a non-word character, the results agree.
    assert_eq!(
        assert_agrees_with_c(
            r"(?<!\w)(_)(?!_)(.+?)(?<!_)\1(?!\w)",
            b"_b\n\xe2\x84_\xc3\xa9b_"
        ),
        Some((5, 10))
    );
}
