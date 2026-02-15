// api_test.rs - Integration tests for the idiomatic Rust API.

use ferroni::api::{Regex, RegexBuilder};
use ferroni::error::RegexError;
use ferroni::prelude::*;

// === Regex::new ===

#[test]
fn simple_pattern() {
    let re = Regex::new(r"\d+").unwrap();
    let m = re.find("abc 123 def").unwrap();
    assert_eq!(m.as_str(), "123");
}

#[test]
fn unicode_pattern() {
    let re = Regex::new(r"\p{Hiragana}+").unwrap();
    let m = re.find("hello せかい world").unwrap();
    assert_eq!(m.as_str(), "せかい");
}

#[test]
fn no_match_returns_none() {
    let re = Regex::new(r"xyz").unwrap();
    assert!(re.find("abc").is_none());
}

#[test]
fn empty_pattern() {
    let re = Regex::new(r"").unwrap();
    let m = re.find("hello").unwrap();
    assert_eq!(m.start(), 0);
    assert_eq!(m.end(), 0);
    assert!(m.is_empty());
}

#[test]
fn invalid_pattern_syntax_error() {
    let err = Regex::new(r"(unclosed").unwrap_err();
    match err {
        RegexError::Syntax { code, .. } => assert!(code < 0),
        other => panic!("expected Syntax error, got {:?}", other),
    }
}

#[test]
fn invalid_pattern_empty_char_class() {
    let err = Regex::new(r"[]").unwrap_err();
    assert!(matches!(err, RegexError::Syntax { .. }));
}

// === Regex::is_match ===

#[test]
fn is_match_true() {
    let re = Regex::new(r"world").unwrap();
    assert!(re.is_match("hello world"));
}

#[test]
fn is_match_false() {
    let re = Regex::new(r"world").unwrap();
    assert!(!re.is_match("hello earth"));
}

// === Regex::find ===

#[test]
fn find_start_end_range() {
    let re = Regex::new(r"bar").unwrap();
    let m = re.find("foobarbaz").unwrap();
    assert_eq!(m.start(), 3);
    assert_eq!(m.end(), 6);
    assert_eq!(m.range(), 3..6);
    assert_eq!(m.len(), 3);
    assert!(!m.is_empty());
}

#[test]
fn find_as_bytes() {
    let re = Regex::new(r"\w+").unwrap();
    let m = re.find("hello world").unwrap();
    assert_eq!(m.as_bytes(), b"hello");
}

// === Regex::captures ===

#[test]
fn captures_groups() {
    let re = Regex::new(r"(\w+)\s+(\w+)").unwrap();
    let caps = re.captures("hello world").unwrap();
    assert_eq!(caps.get(0).unwrap().as_str(), "hello world");
    assert_eq!(caps.get(1).unwrap().as_str(), "hello");
    assert_eq!(caps.get(2).unwrap().as_str(), "world");
    assert_eq!(caps.len(), 3); // group 0 + 2 captures
}

#[test]
fn captures_optional_group() {
    let re = Regex::new(r"(a)(b)?c").unwrap();
    let caps = re.captures("ac").unwrap();
    assert_eq!(caps.get(0).unwrap().as_str(), "ac");
    assert_eq!(caps.get(1).unwrap().as_str(), "a");
    assert!(caps.get(2).is_none()); // group 2 didn't participate
}

#[test]
fn captures_named() {
    let re = Regex::new(r"(?<first>\w+)\s+(?<last>\w+)").unwrap();
    let caps = re.captures("John Doe").unwrap();
    assert_eq!(caps.name("first").unwrap().as_str(), "John");
    assert_eq!(caps.name("last").unwrap().as_str(), "Doe");
    assert!(caps.name("middle").is_none());
}

#[test]
fn captures_no_match() {
    let re = Regex::new(r"(\d+)").unwrap();
    assert!(re.captures("no digits").is_none());
}

#[test]
fn captures_len() {
    let re = Regex::new(r"(a)(b)(c)(d)").unwrap();
    assert_eq!(re.captures_len(), 4);
}

#[test]
fn captures_iter() {
    let re = Regex::new(r"(a)(b)").unwrap();
    let caps = re.captures("ab").unwrap();
    let items: Vec<_> = caps.iter().collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].unwrap().as_str(), "ab");
    assert_eq!(items[1].unwrap().as_str(), "a");
    assert_eq!(items[2].unwrap().as_str(), "b");
}

// === Regex::find_iter ===

#[test]
fn find_iter_multiple() {
    let re = Regex::new(r"\d+").unwrap();
    let results: Vec<&str> = re.find_iter("1 and 22 and 333").map(|m| m.as_str()).collect();
    assert_eq!(results, vec!["1", "22", "333"]);
}

#[test]
fn find_iter_no_matches() {
    let re = Regex::new(r"\d+").unwrap();
    let results: Vec<_> = re.find_iter("no digits").collect();
    assert!(results.is_empty());
}

#[test]
fn find_iter_empty_pattern() {
    let re = Regex::new(r"").unwrap();
    let results: Vec<_> = re.find_iter("ab").collect();
    // Should find empty match at positions 0, 1, 2
    assert_eq!(results.len(), 3);
    for (i, m) in results.iter().enumerate() {
        assert_eq!(m.start(), i);
        assert!(m.is_empty());
    }
}

#[test]
fn find_iter_overlapping_region() {
    let re = Regex::new(r"\w+").unwrap();
    let results: Vec<&str> = re.find_iter("a bb ccc").map(|m| m.as_str()).collect();
    assert_eq!(results, vec!["a", "bb", "ccc"]);
}

// === RegexBuilder ===

#[test]
fn builder_case_insensitive() {
    let re = RegexBuilder::new(r"hello")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("HELLO"));
    assert!(re.is_match("HeLlO"));
}

#[test]
fn builder_dot_matches_newline() {
    let re = Regex::builder(r"a.b")
        .dot_matches_newline(true)
        .build()
        .unwrap();
    assert!(re.is_match("a\nb"));

    let re2 = Regex::builder(r"a.b").build().unwrap();
    assert!(!re2.is_match("a\nb"));
}

#[test]
fn builder_extended_mode() {
    let re = Regex::builder(
        r"
        \d+   # digits
        \s+   # space
        \w+   # word
    ",
    )
    .extended(true)
    .build()
    .unwrap();
    assert!(re.is_match("42 hello"));
}

#[test]
fn builder_syntax() {
    use ferroni::regsyntax::OnigSyntaxPerl;
    let re = Regex::builder(r"\d+")
        .syntax(&OnigSyntaxPerl)
        .build()
        .unwrap();
    assert!(re.is_match("42"));
}

#[test]
fn builder_chaining() {
    let re = Regex::builder(r"hello world")
        .case_insensitive(true)
        .dot_matches_newline(true)
        .extended(false)
        .build()
        .unwrap();
    assert!(re.is_match("HELLO WORLD"));
}

// === Byte API ===

#[test]
fn find_bytes() {
    let re = Regex::new_bytes(b"\\d+").unwrap();
    let m = re.find_bytes(b"abc 42 def").unwrap();
    assert_eq!(m.as_bytes(), b"42");
    assert_eq!(m.start(), 4);
}

#[test]
fn is_match_bytes() {
    let re = Regex::new_bytes(b"hello").unwrap();
    assert!(re.is_match_bytes(b"say hello"));
    assert!(!re.is_match_bytes(b"goodbye"));
}

// === RegexError ===

#[test]
fn error_display() {
    let err = Regex::new(r"(").unwrap_err();
    let msg = format!("{}", err);
    assert!(!msg.is_empty());
}

#[test]
fn error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(Regex::new(r"[").unwrap_err());
    assert!(!err.to_string().is_empty());
}

#[test]
fn error_code() {
    let err = Regex::new(r"(").unwrap_err();
    assert!(err.code() < 0);
}

// === Prelude ===

#[test]
fn prelude_imports_work() {
    // This test verifies that the prelude re-exports are accessible.
    let re = Regex::new(r"(\w+)").unwrap();
    let caps: Captures = re.captures("hello").unwrap();
    let m: Match = caps.get(0).unwrap();
    assert_eq!(m.as_str(), "hello");
    let _: &RegexError = &Regex::new(r"(").unwrap_err();
}

// === as_raw escape hatch ===

#[test]
fn as_raw_access() {
    use ferroni::regexec::onig_number_of_captures;
    let re = Regex::new(r"(a)(b)(c)").unwrap();
    let raw = re.as_raw();
    assert_eq!(onig_number_of_captures(raw), 3);
}

// === Complex patterns ===

#[test]
fn alternation() {
    let re = Regex::new(r"cat|dog|bird").unwrap();
    assert_eq!(re.find("I have a dog").unwrap().as_str(), "dog");
}

#[test]
fn backreference() {
    let re = Regex::new(r"(\w+)\s+\1").unwrap();
    let m = re.find("hello hello world").unwrap();
    assert_eq!(m.as_str(), "hello hello");
}

#[test]
fn lookahead() {
    let re = Regex::new(r"\d+(?= dollars)").unwrap();
    let m = re.find("I have 42 dollars").unwrap();
    assert_eq!(m.as_str(), "42");
}

#[test]
fn lookbehind() {
    let re = Regex::new(r"(?<=\$)\d+").unwrap();
    let m = re.find("price: $99").unwrap();
    assert_eq!(m.as_str(), "99");
}

#[test]
fn possessive_quantifier() {
    let re = Regex::new(r"a++b").unwrap();
    // "aab" should match since possessive consumes all 'a's, then finds 'b'
    assert!(re.is_match("aab"));
    // "aa" should not match since possessive consumes 'a's but no 'b'
    assert!(!re.is_match("aa"));
}

#[test]
fn date_extraction() {
    let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})").unwrap();
    let caps = re.captures("Today is 2026-02-14.").unwrap();
    assert_eq!(caps.name("year").unwrap().as_str(), "2026");
    assert_eq!(caps.name("month").unwrap().as_str(), "02");
    assert_eq!(caps.name("day").unwrap().as_str(), "14");
    assert_eq!(caps.get(0).unwrap().as_str(), "2026-02-14");
}

#[test]
fn debug_impl() {
    let re = Regex::new(r"\d+").unwrap();
    let dbg = format!("{:?}", re);
    assert!(dbg.contains("Regex"));
}

#[test]
fn captures_debug_impl() {
    let re = Regex::new(r"(\d+)").unwrap();
    let caps = re.captures("42").unwrap();
    let dbg = format!("{:?}", caps);
    assert!(!dbg.is_empty());
}
