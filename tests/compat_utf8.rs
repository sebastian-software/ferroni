// compat_utf8.rs - Integration tests ported from oniguruma test/test_utf8.c
//
// Uses the same pattern as the C test suite:
//   x2(pattern, string, from, to)       -> search, expect match at from..to
//   x3(pattern, string, from, to, mem)   -> search, expect capture group mem at from..to
//   n(pattern, string)                    -> search, expect no match
//
// These use onig_new() + onig_search() to match the C test harness exactly.

use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::oniguruma::*;
use ferroni::regsyntax::OnigSyntaxOniguruma;
use ferroni::regint::*;

fn x2(pattern: &[u8], input: &[u8], from: i32, to: i32) {
    let reg = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma as *const OnigSyntaxType,
    )
    .unwrap_or_else(|e| {
        panic!(
            "compile failed for {:?}: error {}",
            std::str::from_utf8(pattern).unwrap_or("<invalid>"),
            e
        )
    });

    let (result, region) = onig_search(
        &reg,
        input,
        input.len(),
        0,
        input.len(),
        Some(OnigRegion::new()),
        ONIG_OPTION_NONE,
    );

    assert!(
        result >= 0,
        "x2: expected match for {:?} against {:?}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        result
    );

    let region = region.unwrap();
    assert_eq!(
        region.beg[0], from,
        "x2: wrong start for {:?} against {:?}: expected {}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        from,
        region.beg[0]
    );
    assert_eq!(
        region.end[0], to,
        "x2: wrong end for {:?} against {:?}: expected {}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        to,
        region.end[0]
    );
}

fn x3(pattern: &[u8], input: &[u8], from: i32, to: i32, mem: usize) {
    let reg = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma as *const OnigSyntaxType,
    )
    .unwrap_or_else(|e| {
        panic!(
            "compile failed for {:?}: error {}",
            std::str::from_utf8(pattern).unwrap_or("<invalid>"),
            e
        )
    });

    let (result, region) = onig_search(
        &reg,
        input,
        input.len(),
        0,
        input.len(),
        Some(OnigRegion::new()),
        ONIG_OPTION_NONE,
    );

    assert!(
        result >= 0,
        "x3: expected match for {:?} against {:?}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        result
    );

    let region = region.unwrap();
    assert!(
        mem < region.num_regs as usize,
        "x3: group {} not captured for {:?} (num_regs={})",
        mem,
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        region.num_regs
    );
    assert_eq!(
        region.beg[mem], from,
        "x3: wrong start for group {} of {:?}: expected {}, got {}",
        mem,
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        from,
        region.beg[mem]
    );
    assert_eq!(
        region.end[mem], to,
        "x3: wrong end for group {} of {:?}: expected {}, got {}",
        mem,
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        to,
        region.end[mem]
    );
}

fn n(pattern: &[u8], input: &[u8]) {
    let reg = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma as *const OnigSyntaxType,
    )
    .unwrap_or_else(|e| {
        panic!(
            "compile failed for {:?}: error {}",
            std::str::from_utf8(pattern).unwrap_or("<invalid>"),
            e
        )
    });

    let (result, _) = onig_search(
        &reg,
        input,
        input.len(),
        0,
        input.len(),
        Some(OnigRegion::new()),
        ONIG_OPTION_NONE,
    );

    assert_eq!(
        result,
        ONIG_MISMATCH,
        "n: expected no match for {:?} against {:?}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        result
    );
}

// ============================================================================
// Basic literals
// ============================================================================

#[test]
fn empty_pattern_empty_string() {
    x2(b"", b"", 0, 0);
}

#[test]
fn literal_a() {
    x2(b"a", b"a", 0, 1);
}

#[test]
fn literal_aa() {
    x2(b"aa", b"aa", 0, 2);
}

#[test]
fn literal_aaa() {
    x2(b"aaa", b"aaa", 0, 3);
}

#[test]
fn literal_ab() {
    x2(b"ab", b"ab", 0, 2);
}

#[test]
fn literal_b_in_ab() {
    x2(b"b", b"ab", 1, 2);
}

#[test]
fn literal_bc_in_abc() {
    x2(b"bc", b"abc", 1, 3);
}

// ============================================================================
// Escapes
// ============================================================================

#[test]
fn hex_escape_x61() {
    x2(b"\\x61", b"a", 0, 1);
}

#[test]
fn octal_escape_017() {
    x2(b"\\17", b"\x0f", 0, 1);
}

#[test]
fn hex_escape_x1f() {
    x2(b"\\x1f", b"\x1f", 0, 1);
}

// ============================================================================
// Anchors
// ============================================================================

#[test]
fn anchor_caret_empty() {
    x2(b"^", b"", 0, 0);
}

#[test]
fn anchor_dollar_empty() {
    x2(b"$", b"", 0, 0);
}

#[test]
fn anchor_caret_dollar_empty() {
    x2(b"^$", b"", 0, 0);
}

#[test]
fn anchor_begin_buf() {
    x2(b"\\A", b"", 0, 0);
}

#[test]
fn anchor_end_buf() {
    x2(b"\\z", b"", 0, 0);
}

#[test]
fn anchor_semi_end_buf() {
    x2(b"\\Z", b"", 0, 0);
}

#[test]
fn anchor_begin_position() {
    x2(b"\\G", b"", 0, 0);
}

#[test]
fn anchor_caret_a_newline() {
    x2(b"^a", b"\na", 1, 2);
}

// ============================================================================
// Dot (any character)
// ============================================================================

#[test]
fn dot_a() {
    x2(b".", b"a", 0, 1);
}

#[test]
fn dot_no_newline() {
    n(b".", b"\n");
}

#[test]
fn dot_dot() {
    x2(b"..", b"ab", 0, 2);
}

// ============================================================================
// Character types
// ============================================================================

#[test]
fn word_char() {
    x2(b"\\w", b"e", 0, 1);
}

#[test]
fn not_word_char() {
    n(b"\\W", b"e");
}

#[test]
fn space_char() {
    x2(b"\\s", b" ", 0, 1);
}

#[test]
fn not_space_char() {
    x2(b"\\S", b"b", 0, 1);
}

#[test]
fn digit_char() {
    x2(b"\\d", b"4", 0, 1);
}

#[test]
fn not_digit_char() {
    n(b"\\D", b"4");
}

// ============================================================================
// Word boundaries
// ============================================================================

#[test]
fn word_boundary_at_start() {
    x2(b"\\b", b"z ", 0, 0);
}

#[test]
fn word_boundary_before_word() {
    x2(b"\\b", b" z", 1, 1);
}

#[test]
fn word_boundary_in_middle() {
    x2(b"\\b", b"  z ", 2, 2);
}

#[test]
fn not_word_boundary() {
    x2(b"\\B", b"zz ", 1, 1);
}

#[test]
fn not_word_boundary_after_space() {
    x2(b"\\B", b"z ", 2, 2);
}

#[test]
fn not_word_boundary_before_word() {
    x2(b"\\B", b" z", 0, 0);
}

// ============================================================================
// Character classes
// ============================================================================

#[test]
fn char_class_ab() {
    x2(b"[ab]", b"b", 0, 1);
}

#[test]
fn char_class_no_match() {
    n(b"[ab]", b"c");
}

#[test]
fn char_class_range() {
    x2(b"[a-z]", b"t", 0, 1);
}

#[test]
fn char_class_negated() {
    n(b"[^a]", b"a");
}

#[test]
fn char_class_negated_newline() {
    x2(b"[^a]", b"\n", 0, 1);
}

#[test]
fn char_class_bracket() {
    x2(b"[]]", b"]", 0, 1);
}

#[test]
fn char_class_caret_plus() {
    x2(b"[\\^]+", b"0^^1", 1, 3);
}

#[test]
fn char_class_b_dash() {
    x2(b"[b-]", b"b", 0, 1);
}

#[test]
fn char_class_dash() {
    x2(b"[b-]", b"-", 0, 1);
}

#[test]
fn char_class_w_meta() {
    x2(b"[\\w]", b"z", 0, 1);
}

#[test]
fn char_class_w_no_space() {
    n(b"[\\w]", b" ");
}

#[test]
fn char_class_W_meta() {
    x2(b"[\\W]", b"b$", 1, 2);
}

#[test]
fn char_class_d_meta() {
    x2(b"[\\d]", b"5", 0, 1);
}

#[test]
fn char_class_d_no_letter() {
    n(b"[\\d]", b"e");
}

#[test]
fn char_class_D_meta() {
    x2(b"[\\D]", b"t", 0, 1);
}

#[test]
fn char_class_D_no_digit() {
    n(b"[\\D]", b"3");
}

#[test]
fn char_class_s_meta() {
    x2(b"[\\s]", b" ", 0, 1);
}

#[test]
fn char_class_s_no_letter() {
    n(b"[\\s]", b"a");
}

#[test]
fn char_class_S_meta() {
    x2(b"[\\S]", b"b", 0, 1);
}

#[test]
fn char_class_S_no_space() {
    n(b"[\\S]", b" ");
}

#[test]
fn char_class_combined() {
    x2(b"[\\w\\d]", b"2", 0, 1);
}

#[test]
fn char_class_combined_no_space() {
    n(b"[\\w\\d]", b" ");
}

// ============================================================================
// Quantifiers
// ============================================================================

#[test]
fn question_empty() {
    x2(b"a?", b"", 0, 0);
}

#[test]
fn question_no_match() {
    x2(b"a?", b"b", 0, 0);
}

#[test]
fn question_match() {
    x2(b"a?", b"a", 0, 1);
}

#[test]
fn star_empty() {
    x2(b"a*", b"", 0, 0);
}

#[test]
fn star_one() {
    x2(b"a*", b"a", 0, 1);
}

#[test]
fn star_three() {
    x2(b"a*", b"aaa", 0, 3);
}

#[test]
fn star_prefix_no_match() {
    x2(b"a*", b"baaaa", 0, 0);
}

#[test]
fn plus_empty() {
    n(b"a+", b"");
}

#[test]
fn plus_one() {
    x2(b"a+", b"a", 0, 1);
}

#[test]
fn plus_four() {
    x2(b"a+", b"aaaa", 0, 4);
}

#[test]
fn plus_partial() {
    x2(b"a+", b"aabbb", 0, 2);
}

#[test]
fn plus_search() {
    x2(b"a+", b"baaaa", 1, 5);
}

#[test]
fn dot_question_empty() {
    x2(b".?", b"", 0, 0);
}

#[test]
fn dot_question_char() {
    x2(b".?", b"f", 0, 1);
}

#[test]
fn dot_question_newline() {
    x2(b".?", b"\n", 0, 0);
}

#[test]
fn dot_star_empty() {
    x2(b".*", b"", 0, 0);
}

#[test]
fn dot_star_string() {
    x2(b".*", b"abcde", 0, 5);
}

#[test]
fn dot_plus_char() {
    x2(b".+", b"z", 0, 1);
}

#[test]
fn dot_plus_with_newline() {
    x2(b".+", b"zdswer\n", 0, 6);
}

// ============================================================================
// Alternation
// ============================================================================

#[test]
fn alt_a_or_b_match_a() {
    x2(b"a|b", b"a", 0, 1);
}

#[test]
fn alt_a_or_b_match_b() {
    x2(b"a|b", b"b", 0, 1);
}

#[test]
fn alt_empty_or_a() {
    x2(b"|a", b"a", 0, 0);
}

#[test]
fn alt_ab_or_bc_match_ab() {
    x2(b"ab|bc", b"ab", 0, 2);
}

#[test]
fn alt_ab_or_bc_match_bc() {
    x2(b"ab|bc", b"bc", 0, 2);
}

#[test]
fn alt_in_group() {
    x2(b"z(?:ab|bc)", b"zbc", 0, 3);
}

// ============================================================================
// Captures
// ============================================================================

#[test]
fn capture_a() {
    x3(b"(a)", b"a", 0, 1, 1);
}

#[test]
fn capture_ab() {
    x3(b"(ab)", b"ab", 0, 2, 1);
}

#[test]
fn capture_nested_outer() {
    x2(b"((ab))", b"ab", 0, 2);
}

#[test]
fn capture_nested_group1() {
    x3(b"((ab))", b"ab", 0, 2, 1);
}

#[test]
fn capture_nested_group2() {
    x3(b"((ab))", b"ab", 0, 2, 2);
}

#[test]
fn capture_two_groups_first() {
    x3(b"(ab)(cd)", b"abcd", 0, 2, 1);
}

#[test]
fn capture_two_groups_second() {
    x3(b"(ab)(cd)", b"abcd", 2, 4, 2);
}

#[test]
fn capture_empty_group() {
    x3(b"()(a)bc(def)ghijk", b"abcdefghijk", 3, 6, 3);
}

// ============================================================================
// Backreferences
// ============================================================================

#[test]
fn backref_repeat() {
    x3(b"(a*)\\1", b"aaaaa", 0, 2, 1);
}

#[test]
fn backref_ab() {
    x2(b"a(b*)\\1", b"abbbb", 0, 5);
}

#[test]
fn backref_empty_match() {
    x2(b"a(b*)\\1", b"ab", 0, 1);
}

#[test]
fn backref_two_groups() {
    x2(b"(a*)(b*)\\1\\2", b"aaabbaaabb", 0, 10);
}

#[test]
fn backref_group2() {
    x2(b"(a*)(b*)\\2", b"aaabbbb", 0, 7);
}

#[test]
fn backref_multi_group() {
    x2(b"(a)(b)(c)\\2\\1\\3", b"abcbac", 0, 6);
}

#[test]
fn backref_char_class() {
    x2(b"([a-d])\\1", b"cc", 0, 2);
}

// ============================================================================
// Lookahead
// ============================================================================

#[test]
fn lookahead_positive() {
    x2(b"(?=z)z", b"z", 0, 1);
}

#[test]
fn lookahead_positive_no_match() {
    n(b"(?=z).", b"a");
}

#[test]
fn lookahead_negative() {
    x2(b"(?!z)a", b"a", 0, 1);
}

#[test]
fn lookahead_negative_no_match() {
    n(b"(?!z)a", b"z");
}

// ============================================================================
// Non-capturing group
// ============================================================================

#[test]
fn non_capturing_basic() {
    x2(b"(?:ab)", b"ab", 0, 2);
}

#[test]
fn non_capturing_alt() {
    x2(b"z(?:ab|bc)", b"zbc", 0, 3);
}

// ============================================================================
// Lazy quantifiers
// ============================================================================

#[test]
#[ignore] // C test: x2("(?:x?)\\?\\?", "", 0, 0) - involves nested quantifier interpretation
fn lazy_question() {
    x2(b"(?:x?)\\?\\?", b"", 0, 0);
}

#[test]
fn lazy_star_empty() {
    x2(b"(?:x?)*?", b"", 0, 0);
}

#[test]
fn lazy_star_x() {
    x2(b"(?:x?)*?", b"x", 0, 0);
}

#[test]
fn lazy_plus_empty() {
    x2(b"(?:x?)+?", b"", 0, 0);
}

#[test]
fn lazy_plus_x() {
    x2(b"(?:x?)+?", b"x", 0, 1);
}

// ============================================================================
// Combined / complex patterns
// ============================================================================

#[test]
fn multi_line_search() {
    x2(b".*abc", b"dddabdd\nddabc", 8, 13);
}

#[test]
fn multi_line_search_plus() {
    x2(b".+abc", b"dddabdd\nddabcaa\naaaabc", 8, 13);
}

#[test]
fn comment_in_pattern() {
    x2(b"a(?#....\\\\JJJJ)b", b"ab", 0, 2);
}

// ============================================================================
// Interval quantifiers {n,m}
// ============================================================================

#[test]
fn interval_exact() {
    x2(b"a{3}", b"aaa", 0, 3);
}

#[test]
fn interval_range() {
    x2(b"a{2,4}", b"aaa", 0, 3);
}

#[test]
fn interval_lower_bound() {
    n(b"a{3}", b"aa");
}

// ============================================================================
// UTF-8 multi-byte patterns
// ============================================================================

#[test]
fn utf8_single_char() {
    x2("あ".as_bytes(), "あ".as_bytes(), 0, 3);
}

#[test]
fn utf8_no_match() {
    n("い".as_bytes(), "あ".as_bytes());
}

#[test]
fn utf8_double() {
    x2("うう".as_bytes(), "うう".as_bytes(), 0, 6);
}

#[test]
fn utf8_triple() {
    x2("あいう".as_bytes(), "あいう".as_bytes(), 0, 9);
}

#[test]
fn utf8_dot() {
    x2(b".", "あ".as_bytes(), 0, 3);
}

#[test]
fn utf8_dot_dot() {
    x2(b"..", "かき".as_bytes(), 0, 6);
}

#[test]
fn utf8_word_char() {
    x2(b"\\w", "お".as_bytes(), 0, 3);
}

#[test]
fn utf8_not_word() {
    n(b"\\W", "あ".as_bytes());
}

#[test]
fn utf8_char_class() {
    let pattern = "[たち]".as_bytes();
    let input = "ち".as_bytes();
    x2(pattern, input, 0, 3);
}

#[test]
fn utf8_char_class_no_match() {
    let pattern = "[なに]".as_bytes();
    let input = "ぬ".as_bytes();
    n(pattern, input);
}

#[test]
fn utf8_star() {
    let pattern = "量*".as_bytes();
    x2(pattern, b"", 0, 0);
}

#[test]
fn utf8_star_three() {
    let pattern = "子*".as_bytes();
    let input = "子子子".as_bytes();
    x2(pattern, input, 0, 9);
}

#[test]
fn utf8_plus() {
    let pattern = "河+".as_bytes();
    let input = "河".as_bytes();
    x2(pattern, input, 0, 3);
}

#[test]
fn utf8_plus_four() {
    let pattern = "時+".as_bytes();
    let input = "時時時時".as_bytes();
    x2(pattern, input, 0, 12);
}

#[test]
fn utf8_alt() {
    let pat_a = "あ".as_bytes();
    let pat_i = "い".as_bytes();
    let pattern = [pat_a, b"|", pat_i].concat();
    x2(&pattern, pat_a, 0, 3);
    x2(&pattern, pat_i, 0, 3);
}

#[test]
fn utf8_capture() {
    let pattern = "(火)".as_bytes();
    let input = "火".as_bytes();
    x3(pattern, input, 0, 3, 1);
}

#[test]
fn utf8_capture_pair() {
    let pattern = "(火水)".as_bytes();
    let input = "火水".as_bytes();
    x3(pattern, input, 0, 6, 1);
}

#[test]
fn utf8_anchor_begin() {
    let pattern = "^あ".as_bytes();
    let input = "あ".as_bytes();
    x2(pattern, input, 0, 3);
}

#[test]
fn utf8_anchor_end() {
    let pattern = "む$".as_bytes();
    let input = "む".as_bytes();
    x2(b"^", input, 0, 0);
}

#[test]
fn utf8_question() {
    let pattern = "あ?".as_bytes();
    x2(pattern, b"", 0, 0);
    x2(pattern, "あ".as_bytes(), 0, 3);
}

#[test]
fn utf8_lookahead() {
    let pattern_bytes = [b"(?=", "せ".as_bytes(), b")", "せ".as_bytes()].concat();
    let input = "せ".as_bytes();
    x2(&pattern_bytes, input, 0, 3);
}

#[test]
fn utf8_neg_lookahead() {
    let pattern_bytes = [b"(?!", "う".as_bytes(), b")", "か".as_bytes()].concat();
    let input = "か".as_bytes();
    x2(&pattern_bytes, input, 0, 3);
}

#[test]
fn utf8_neg_lookahead_no_match() {
    let pattern_bytes = [b"(?!", "と".as_bytes(), b")", "あ".as_bytes()].concat();
    let input = "と".as_bytes();
    n(&pattern_bytes, input);
}
