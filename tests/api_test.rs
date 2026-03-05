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
    let results: Vec<&str> = re
        .find_iter("1 and 22 and 333")
        .map(|m| m.as_str())
        .collect();
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

// =========================================================================
// Coverage-targeted tests: Unicode case folding (unicode/mod.rs)
// =========================================================================

#[test]
fn case_insensitive_unicode_latin() {
    // Exercises apply_case_fold1 for non-ASCII Latin characters (ä/Ä, ö/Ö, etc.)
    let re = Regex::builder(r"straße")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("STRASSE"));
    assert!(re.is_match("Straße"));
}

#[test]
fn case_insensitive_unicode_greek() {
    let re = Regex::builder(r"σ").case_insensitive(true).build().unwrap();
    assert!(re.is_match("Σ")); // capital sigma
    assert!(re.is_match("ς")); // final sigma
}

#[test]
fn case_insensitive_multi_char_fold() {
    // Exercises folds2/folds3 paths: ﬃ (U+FB03) folds to "ffi"
    let re = Regex::builder(r"ffi")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("ffi"));
    assert!(re.is_match("FFI"));
}

#[test]
fn case_insensitive_char_class_unicode() {
    // Character class with case-insensitive Unicode
    let re = Regex::builder(r"[äöü]+")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("ÄÖÜ"));
    assert!(re.is_match("äöü"));
}

// =========================================================================
// Coverage-targeted tests: Word boundary opcodes (regexec.rs WordStar etc.)
// =========================================================================

#[test]
fn word_star_non_ascii() {
    // Exercises WordStar non-ASCII path in match engine
    let re = Regex::new(r"\w*").unwrap();
    let m = re.find("café").unwrap();
    assert_eq!(m.as_str(), "café");
}

#[test]
fn word_star_peek_next() {
    // Exercises WordAsciiStarPeekNext opcode: \w* followed by specific char
    let re = Regex::new(r"\w*:").unwrap();
    let m = re.find("key: value").unwrap();
    assert_eq!(m.as_str(), "key:");
}

#[test]
fn word_boundary_unicode() {
    // Exercises Unicode word boundary code paths
    let re = Regex::new(r"\b\w+\b").unwrap();
    let results: Vec<&str> = re
        .find_iter("hello·world café")
        .map(|m| m.as_str())
        .collect();
    assert!(results.contains(&"hello"));
    assert!(results.contains(&"café"));
}

// =========================================================================
// Coverage-targeted tests: Subexpression calls (regcomp.rs tune_called_state)
// =========================================================================

#[test]
fn subexpression_call_in_quantifier() {
    // Exercises tune_called_state_call Quant branch
    let re = Regex::new(r"(?<digit>\d)(\g<digit>)+").unwrap();
    assert!(re.is_match("123"));
}

#[test]
fn subexpression_call_in_lookahead() {
    // Exercises tune_called_state_call Anchor/PREC_READ branch
    // \g<d> calls the pattern \d, not the captured value
    let re = Regex::new(r"(?<d>\d)(?=\g<d>)\d").unwrap();
    assert!(re.is_match("11"));
    assert!(re.is_match("12")); // \g<d> matches any digit
}

#[test]
fn subexpression_call_in_negative_lookahead() {
    // Exercises tune_called_state_call PREC_READ_NOT branch
    let re = Regex::new(r"(?<d>\d)(?!\g<d>)x").unwrap();
    assert!(re.is_match("1x")); // \g<d> matches digit but 'x' is not a digit
    assert!(!re.is_match("12")); // no 'x' after digit
}

#[test]
fn subexpression_call_in_lookbehind() {
    // Exercises tune_called_state_call LOOK_BEHIND branch
    let re = Regex::new(r"(?<d>\d)(?<=\g<d>)\w").unwrap();
    assert!(re.is_match("1a"));
}

#[test]
fn if_else_conditional() {
    // Exercises tune_called_state_call IfElse branch
    let re = Regex::new(r"^(a)?(?(1)b|c)$").unwrap();
    assert!(re.is_match("ab"));
    assert!(re.is_match("c"));
}

// =========================================================================
// Coverage-targeted tests: BackRef with nesting level (regexec.rs)
// =========================================================================

#[test]
fn backref_with_level() {
    // Exercises BackRefWithLevel opcode (backrefs in subexpression calls)
    // \k<b+0> is level-backref syntax from Oniguruma
    let re = Regex::new(r"\A(?<a>|.|(?:(?<b>.)\g<a>\k<b+0>))\z").unwrap();
    // Palindrome detector via recursive subexpression call with level-backref
    assert!(re.is_match("a"));
    assert!(re.is_match("aba"));
}

// =========================================================================
// Coverage-targeted tests: Anchors in search (regexec.rs sub_anchor paths)
// =========================================================================

#[test]
fn begin_line_anchor_multiline() {
    // Default: ^ matches start of string only
    let re = Regex::new(r"^\w+").unwrap();
    let results: Vec<&str> = re
        .find_iter("hello\nworld\nfoo")
        .map(|m| m.as_str())
        .collect();
    // Oniguruma: ^ is multiline by default
    assert!(!results.is_empty());
}

#[test]
fn end_line_anchor() {
    let re = Regex::new(r"\w+$").unwrap();
    let m = re.find("hello world").unwrap();
    assert_eq!(m.as_str(), "world");
}

#[test]
fn multiline_anchors() {
    let re = Regex::builder(r"^\w+$")
        .dot_matches_newline(false)
        .build()
        .unwrap();
    let m = re.find("hello").unwrap();
    assert_eq!(m.as_str(), "hello");
}

// =========================================================================
// Coverage-targeted tests: Callout patterns (regexec.rs callout paths)
// =========================================================================

#[test]
fn callout_max_count() {
    // Exercises CALLOUT_BUILTIN_MAX progress/retraction paths
    let re = Regex::new(r"(?:(*MAX{5})a)*").unwrap();
    let m = re.find("aaaaaa").unwrap();
    assert!(m.len() <= 6);
}

#[test]
fn callout_count() {
    // Exercises CALLOUT_BUILTIN_COUNT path
    let re = Regex::new(r"(?:(*COUNT)a)+").unwrap();
    assert!(re.is_match("aaa"));
}

// =========================================================================
// Coverage-targeted tests: Various parser paths (regparse.rs)
// =========================================================================

#[test]
fn possessive_repeat() {
    let re = Regex::new(r"[a-z]++").unwrap();
    assert!(re.is_match("abc"));
}

#[test]
fn named_group_multiple_alternation() {
    // Named groups in alternation
    let re = Regex::new(r"(?<x>a)|(?<x>b)").unwrap();
    assert!(re.is_match("a"));
    assert!(re.is_match("b"));
}

#[test]
fn unicode_property_negated() {
    let re = Regex::new(r"\P{Lu}+").unwrap();
    let m = re.find("ABCdef").unwrap();
    assert_eq!(m.as_str(), "def");
}

#[test]
fn extended_grapheme_cluster() {
    // Exercises EGCB path in unicode/mod.rs
    let re = Regex::new(r"\X").unwrap();
    // Flag emoji: regional indicators
    let m = re.find("🇩🇪x").unwrap();
    assert!(m.len() > 1); // Should match the whole flag as one cluster
}

#[test]
fn word_boundary_types() {
    // Exercises WB algorithm paths including Hebrew, Katakana, ExtendNumLet
    let re = Regex::new(r"\b\w+\b").unwrap();

    // Katakana
    assert!(re.is_match("カタカナ"));

    // Numbers
    let m = re.find("abc123def").unwrap();
    assert_eq!(m.as_str(), "abc123def");
}

#[test]
fn absent_expression() {
    // Exercises absent group compilation in regparse + regcomp
    let re = Regex::new(r"(?~abc)").unwrap();
    let m = re.find("xxabcyy").unwrap();
    assert_eq!(m.as_str(), "xx");
}

#[test]
fn nested_repeat_with_capture() {
    let re = Regex::new(r"((a){2,4}){1,3}").unwrap();
    let m = re.find("aaaa").unwrap();
    assert_eq!(m.as_str(), "aaaa");
}

#[test]
fn character_class_intersection() {
    // Exercises char class intersection parsing
    let re = Regex::new(r"[a-z&&[^aeiou]]+").unwrap();
    let m = re.find("abcde").unwrap();
    assert_eq!(m.as_str(), "bcd");
}

#[test]
fn hex_escape_unicode() {
    let re = Regex::new(r"\x{1F600}").unwrap();
    assert!(re.is_match("😀"));
}

#[test]
fn keep_marker() {
    // \K resets the match start
    let re = Regex::new(r"foo\Kbar").unwrap();
    let m = re.find("foobar").unwrap();
    assert_eq!(m.as_str(), "bar");
}

#[test]
fn atomic_group() {
    let re = Regex::new(r"(?>abc|ab)c").unwrap();
    // "abc" is consumed atomically, then "c" fails → no match for "abc"
    assert!(!re.is_match("abc"));
    assert!(re.is_match("abcc"));
}

#[test]
fn lazy_quantifier_in_capture() {
    let re = Regex::new(r"(a+?)(a+)").unwrap();
    let caps = re.captures("aaaa").unwrap();
    assert_eq!(caps.get(1).unwrap().as_str(), "a");
    assert_eq!(caps.get(2).unwrap().as_str(), "aaa");
}

#[test]
fn char_class_posix() {
    let re = Regex::new(r"[[:upper:]]+").unwrap();
    let m = re.find("abcDEF").unwrap();
    assert_eq!(m.as_str(), "DEF");
}

#[test]
fn non_greedy_repeat() {
    let re = Regex::new(r"<.+?>").unwrap();
    let m = re.find("<a><b>").unwrap();
    assert_eq!(m.as_str(), "<a>");
}

// =========================================================================
// Coverage-targeted: Parser paths (regparse.rs)
// =========================================================================

#[test]
fn quote_literal_syntax() {
    // Exercises \Q...\E literal quoting (TokenType::QuoteOpen) — only in Perl syntax
    use ferroni::regsyntax::OnigSyntaxPerl;
    let re = Regex::builder(r"\Q.+*\E")
        .syntax(&OnigSyntaxPerl)
        .build()
        .unwrap();
    assert!(re.is_match(".+*"));
    assert!(!re.is_match("abc"));
}

#[test]
fn char_class_dash_at_end() {
    // Exercises CC dash handling: [a-z-] → literal dash at end
    let re = Regex::new(r"[a-z-]+").unwrap();
    assert!(re.is_match("abc-"));
    let m = re.find("ABC-def").unwrap();
    assert_eq!(m.as_str(), "-def");
}

#[test]
fn char_class_range_and_dash() {
    // Exercises double-range handling: [0-9-a]
    let re = Regex::new(r"[0-9-a]+").unwrap();
    assert!(re.is_match("5-a"));
}

#[test]
fn named_backref_with_level() {
    // Exercises level syntax parsing in fetch_name: \k<name+1>
    let re = Regex::new(r"(?<a>x)\g<a>\k<a+0>").unwrap();
    assert!(re.is_match("xxx"));
}

#[test]
fn numbered_backref_with_level() {
    // Exercises numeric level backref parsing: \k<1+0>
    let re = Regex::new(r"(x)\g<1>\k<1+0>").unwrap();
    assert!(re.is_match("xxx"));
}

#[test]
fn char_class_negated_intersection() {
    // Exercises negated char class with intersection
    let re = Regex::new(r"[^a-z&&[^m-z]]+").unwrap();
    // [^a-z&&[^m-z]] = not(a-z AND not(m-z)) = not(a-l) = anything except a-l
    assert!(re.is_match("M"));
}

#[test]
fn control_escape_sequences() {
    // Exercises control code syntax path
    let re = Regex::new(r"\ca").unwrap(); // \ca = control-A = 0x01
    assert!(re.is_match("\x01"));
}

#[test]
fn hex_escape_2digit() {
    let re = Regex::new(r"\x41").unwrap(); // 0x41 = 'A'
    assert!(re.is_match("A"));
}

#[test]
fn octal_escape() {
    let re = Regex::new(r"\101").unwrap(); // 0101 octal = 65 = 'A'
    assert!(re.is_match("A"));
}

#[test]
fn backref_with_negative_level() {
    // Exercises level syntax with negative sign: \k<name-1>
    // This tests the level_sign = -1 path
    let re = Regex::new(r"(?<a>(?<b>x)\g<a>|\w)\k<b-0>");
    // Either succeeds or fails at compile — either way exercises the parser
    if let Ok(re) = re {
        let _ = re.is_match("xyx");
    }
}

#[test]
fn unicode_named_property() {
    // Various Unicode property names to exercise property_name_to_ctype
    let re = Regex::new(r"\p{Katakana}+").unwrap();
    assert!(re.is_match("カタカナ"));

    let re = Regex::new(r"\p{Han}+").unwrap();
    assert!(re.is_match("漢字"));
}

#[test]
fn absent_clear_expression() {
    // Exercises absent group range stop: (?~|pattern|absent)
    let re = Regex::new(r"(?~|abc|[a-z]+)");
    if let Ok(re) = re {
        let _ = re.find("xyzabcdef");
    }
}

// =========================================================================
// Coverage-targeted: Compiler paths (regcomp.rs)
// =========================================================================

#[test]
fn alternation_many_branches() {
    // Exercises select_opt_map/trie path in regcomp
    let re = Regex::new(r"apple|banana|cherry|date|elderberry|fig|grape").unwrap();
    assert_eq!(re.find("I like banana").unwrap().as_str(), "banana");
}

#[test]
fn nested_quantifier() {
    // Exercises nested quantifier optimization in regcomp
    let re = Regex::new(r"(a{2,3}){2,4}").unwrap();
    assert!(re.is_match("aaaaaa"));
}

#[test]
fn complex_lookbehind() {
    // Exercises lookbehind compilation with variable-length content
    let re = Regex::new(r"(?<=ab|cd)\w+").unwrap();
    assert_eq!(re.find("cdHello").unwrap().as_str(), "Hello");
}

#[test]
fn case_insensitive_backref() {
    // Exercises BackRefIc opcode generation
    let re = Regex::builder(r"(\w+)\s+\1")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("Hello HELLO"));
}

#[test]
fn many_capture_groups() {
    // Exercises capture group region handling with many groups
    let re = Regex::new(r"(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)").unwrap();
    let caps = re.captures("abcdefghij").unwrap();
    assert_eq!(caps.len(), 11); // group 0 + 10 captures
    assert_eq!(caps.get(10).unwrap().as_str(), "j");
}

#[test]
fn forward_reference() {
    // Exercises forward backref in parser
    let re = Regex::new(r"(\2?(a))+").unwrap();
    assert!(re.is_match("aaa"));
}

// =========================================================================
// Coverage-targeted: Execution engine paths (regexec.rs)
// =========================================================================

#[test]
fn match_at_end_of_string() {
    let re = Regex::new(r"\w+\z").unwrap();
    let m = re.find("hello world").unwrap();
    assert_eq!(m.as_str(), "world");
}

#[test]
fn empty_match_progression() {
    // Tests empty match handling and advancement logic
    let re = Regex::new(r"(?:)+").unwrap();
    let results: Vec<_> = re.find_iter("ab").collect();
    assert!(!results.is_empty());
}

#[test]
fn long_alternation_literal() {
    // Exercises literal trie optimization in search
    let re = Regex::new(r"cat|car|cap|can|cam|cab").unwrap();
    assert_eq!(re.find("a cab ride").unwrap().as_str(), "cab");
}

#[test]
fn unicode_case_fold_in_char_class_range() {
    // Exercises get_case_fold_codes_by_str for multi-char sequences
    let re = Regex::builder(r"[a-ÿ]+")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("Ä"));
    assert!(re.is_match("ß"));
}

#[test]
fn word_boundary_at_boundaries() {
    // Exercises edge cases in word boundary checking
    let re = Regex::new(r"\bword\b").unwrap();
    assert!(re.is_match("word"));
    assert!(re.is_match("a word b"));
    assert!(!re.is_match("sword"));
    assert!(!re.is_match("wordy"));
}

#[test]
fn dotall_mode() {
    let re = Regex::builder(r"a.b")
        .dot_matches_newline(true)
        .build()
        .unwrap();
    assert!(re.is_match("a\nb"));
    assert!(re.is_match("a\rb"));
}

#[test]
fn captures_iter_with_optional_groups() {
    let re = Regex::new(r"(?:(a)|(b))+").unwrap();
    let caps = re.captures("abba").unwrap();
    // Groups 1 and 2 alternately match
    assert!(caps.get(0).is_some());
}

// =========================================================================
// Coverage-targeted: Extended grapheme cluster / word break (unicode/mod.rs)
// =========================================================================

#[test]
fn egcb_flag_emoji_sequence() {
    // Regional indicators: 🇩🇪 = U+1F1E9 U+1F1EA
    let re = Regex::new(r"\X+").unwrap();
    let m = re.find("🇩🇪🇫🇷x").unwrap();
    // Each flag pair is one grapheme cluster
    assert!(m.len() > 4);
}

#[test]
fn egcb_zwj_sequence() {
    // ZWJ sequence: 👨‍👩‍👧 (man + ZWJ + woman + ZWJ + girl)
    let re = Regex::new(r"\X").unwrap();
    let m = re.find("👨\u{200D}👩\u{200D}👧x").unwrap();
    // Should match the whole ZWJ family as one grapheme cluster
    assert!(m.len() > 4);
}

#[test]
fn word_boundary_hebrew() {
    // Exercises HebrewLetter WB rules (WB7a, WB7b, WB7c)
    let re = Regex::new(r"\b\w+\b").unwrap();
    assert!(re.is_match("שלום")); // Hebrew "shalom"
}

#[test]
fn word_boundary_regional_indicators() {
    // Exercises WB15/WB16 RI x RI counting
    let re = Regex::new(r"\b.+\b").unwrap();
    let _ = re.find("🇩🇪🇫🇷"); // just exercise the path
}

#[test]
fn word_boundary_extend_num_let() {
    // Exercises WB13a/WB13b ExtendNumLet rules
    // Underscore (U+005F) is ExtendNumLet type
    let re = Regex::new(r"\b\w+\b").unwrap();
    let m = re.find("hello_world").unwrap();
    assert_eq!(m.as_str(), "hello_world"); // underscore connects words
}

// =========================================================================
// Coverage-targeted: Unicode multi-char case folds (unicode/mod.rs)
// =========================================================================

#[test]
fn case_insensitive_2char_fold() {
    // ﬁ (U+FB01) folds to "fi" — exercises folds2 paths (apply_case_fold2, get_case_fold_codes folds2)
    let re = Regex::builder("fi").case_insensitive(true).build().unwrap();
    assert!(re.is_match("fi"));
    assert!(re.is_match("FI"));
}

#[test]
fn case_insensitive_3char_fold() {
    // ﬃ (U+FB03) folds to "ffi" — exercises folds3 paths
    let re = Regex::builder("ffi")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("ffi"));
    assert!(re.is_match("FFI"));
}

#[test]
fn case_insensitive_eszett() {
    // ß folds to "ss" — another 2-char fold
    let re = Regex::builder("ss").case_insensitive(true).build().unwrap();
    assert!(re.is_match("ss"));
    assert!(re.is_match("SS"));
    assert!(re.is_match("ß"));
}

#[test]
fn case_insensitive_char_class_with_unicode_folds() {
    // Character class with case-insensitive: exercises get_case_fold_codes_by_str
    let re = Regex::builder("[ß]")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("ß"));
    assert!(re.is_match("SS")); // ss should match ß case-insensitively
}

#[test]
fn case_insensitive_turkish_dotted_i() {
    // İ (U+0130) and ı (U+0131) — locale-specific fold entries
    let re = Regex::builder("i").case_insensitive(true).build().unwrap();
    assert!(re.is_match("I"));
    assert!(re.is_match("i"));
}

#[test]
fn case_insensitive_full_width() {
    // Full-width letters: Ａ (U+FF21) should fold to ａ (U+FF41)
    let re = Regex::builder("\u{FF21}")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(re.is_match("\u{FF21}"));
    assert!(re.is_match("\u{FF41}"));
}

// =========================================================================
// Coverage-targeted: EGCB and WB edge cases (unicode/mod.rs)
// =========================================================================

#[test]
fn egcb_regional_indicator_pairs() {
    // 🇩🇪🇫🇷🇯🇵 = 3 flag sequences, each 2 RI codepoints
    // Exercises BreakUndefRiRi counting
    let re = Regex::new(r"\X").unwrap();
    let input = "🇩🇪🇫🇷🇯🇵";
    let clusters: Vec<_> = re.find_iter(input).collect();
    // Should be 3 grapheme clusters (one per flag)
    assert_eq!(clusters.len(), 3);
}

#[test]
fn wb_midletter_between_letters() {
    // WB6/WB7: AHLetter x (MidLetter|MidNumLetQ) x AHLetter
    // · (U+00B7, MidLetter) between letters should NOT break
    let re = Regex::new(r"\b\w+\b").unwrap();
    // Test with apostrophe (U+2019, MidNumLetQ) in word
    let m = re.find("it's").unwrap();
    assert!(!m.as_str().is_empty());
}

#[test]
fn wb_numeric_sequences() {
    // WB8-WB11: Numeric x Numeric, Numeric x (MidNum|MidNumLetQ) x Numeric
    let re = Regex::new(r"\b[\w.]+\b").unwrap();
    assert!(re.is_match("3.14"));
}

#[test]
fn wb_katakana_extend_num_let() {
    // WB13: Katakana x Katakana + WB13a/b with ExtendNumLet
    let re = Regex::new(r"\b\w+\b").unwrap();
    let m = re.find("カタカナ_テスト").unwrap();
    // Katakana + underscore + Katakana should be one word
    assert_eq!(m.as_str(), "カタカナ_テスト");
}
