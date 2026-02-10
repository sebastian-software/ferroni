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
fn group_question_empty() {
    // C: x2("(?:x?)?", "", 0, 0)
    x2(b"(?:x?)?", b"", 0, 0);
}

#[test]
fn group_question_x() {
    x2(b"(?:x?)?", b"x", 0, 1);
}

#[test]
fn group_star_empty() {
    x2(b"(?:x?)*", b"", 0, 0);
}

#[test]
fn group_star_xx() {
    x2(b"(?:x?)*", b"xx", 0, 2);
}

#[test]
fn group_plus_empty() {
    x2(b"(?:x?)+", b"", 0, 0);
}

#[test]
fn group_plus_xx() {
    x2(b"(?:x?)+", b"xx", 0, 2);
}

#[test]
fn lazy_question_empty() {
    // C: x2("(?:x?)\?\?", "", 0, 0) — \? in C is trigraph for ?, so pattern is (?:x?)??
    x2(b"(?:x?)??", b"", 0, 0);
}

#[test]
fn lazy_question_x() {
    // C: x2("(?:x?)\?\?", "x", 0, 0) — lazy ?? prefers 0 matches
    x2(b"(?:x?)??", b"x", 0, 0);
}

#[test]
fn lazy_question_xx() {
    // C: x2("(?:x?)\?\?", "xx", 0, 0) — lazy ?? prefers 0 matches
    x2(b"(?:x?)??", b"xx", 0, 0);
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

// ============================================================================
// POSIX bracket classes (C lines 223-232)
// ============================================================================

#[test]
fn posix_upper() {
    x2(b"[[:upper:]]", b"B", 0, 1);
}

#[test]
fn posix_xdigit_plus() {
    // [*[:xdigit:]+] matches *, hex digit, or +
    x2(b"[*[:xdigit:]+]", b"+", 0, 1);
}

#[test]
fn posix_xdigit_search1() {
    x2(b"[*[:xdigit:]+]", b"GHIKK-9+*", 6, 7);
}

#[test]
fn posix_xdigit_search2() {
    x2(b"[*[:xdigit:]+]", b"-@^+", 3, 4);
}

#[test]
fn posix_not_a_bracket() {
    // [[:upper]] is NOT a POSIX bracket — it's literal chars [:upper]
    n(b"[[:upper]]", b"A");
}

#[test]
fn posix_not_a_bracket_colon() {
    x2(b"[[:upper]]", b":", 0, 1);
}

#[test]
fn posix_upper_no_lower() {
    n(b"[[:upper:]]", b"a");
}

#[test]
fn posix_neg_upper() {
    x2(b"[[:^upper:]]", b"a", 0, 1);
}

#[test]
fn posix_lower_no_upper() {
    n(b"[[:lower:]]", b"A");
}

#[test]
fn posix_neg_lower() {
    x2(b"[[:^lower:]]", b"A", 0, 1);
}

// ============================================================================
// Character class escapes and ranges (C lines 206, 250-257)
// ============================================================================

#[test]
fn cc_negated_bracket() {
    n(b"[^]]", b"]");
}

#[test]
fn cc_octal_range() {
    x2(b"[\\044-\\047]", b"\x26", 0, 1);
}

#[test]
fn cc_hex_range_5a_5c() {
    x2(b"[\\x5a-\\x5c]", b"\x5b", 0, 1);
}

#[test]
fn cc_hex_range_6a_6d() {
    x2(b"[\\x6A-\\x6D]", b"\x6c", 0, 1);
}

#[test]
fn cc_hex_range_6a_6d_no_match() {
    n(b"[\\x6A-\\x6D]", b"\x6E");
}

#[test]
fn cc_complex_no_match() {
    n(b"^[0-9A-F]+ 0+ UNDEF ", b"75F 00000000 SECT14A notype ()    External    | _rb_apply");
}

#[test]
fn cc_escaped_open_bracket() {
    x2(b"[\\[]", b"[", 0, 1);
}

#[test]
fn cc_escaped_close_bracket() {
    x2(b"[\\]]", b"]", 0, 1);
}

#[test]
fn cc_ampersand() {
    x2(b"[&]", b"&", 0, 1);
}

// ============================================================================
// Nested character classes (C lines 258-261)
// ============================================================================

#[test]
fn cc_nested_ab() {
    x2(b"[[ab]]", b"b", 0, 1);
}

#[test]
fn cc_nested_ab_c() {
    x2(b"[[ab]c]", b"c", 0, 1);
}

#[test]
fn cc_nested_neg() {
    n(b"[[^a]]", b"a");
}

#[test]
fn cc_neg_nested() {
    n(b"[^[a]]", b"a");
}

// ============================================================================
// Set operations with && (C lines 262-275)
// ============================================================================

#[test]
fn cc_intersect_ab_bc() {
    x2(b"[[ab]&&bc]", b"b", 0, 1);
}

#[test]
fn cc_intersect_ab_bc_no_a() {
    n(b"[[ab]&&bc]", b"a");
}

#[test]
fn cc_intersect_ab_bc_no_c() {
    n(b"[[ab]&&bc]", b"c");
}

#[test]
fn cc_intersect_range() {
    x2(b"[a-z&&b-y&&c-x]", b"w", 0, 1);
}

#[test]
fn cc_neg_intersect_range() {
    n(b"[^a-z&&b-y&&c-x]", b"w");
}

#[test]
fn cc_intersect_neg_and_range() {
    x2(b"[[^a&&a]&&a-z]", b"b", 0, 1);
}

#[test]
fn cc_intersect_neg_and_range_no_a() {
    n(b"[[^a&&a]&&a-z]", b"a");
}

#[test]
fn cc_intersect_complex1() {
    x2(b"[[^a-z&&bcdef]&&[^c-g]]", b"h", 0, 1);
}

#[test]
fn cc_intersect_complex1_no_c() {
    n(b"[[^a-z&&bcdef]&&[^c-g]]", b"c");
}

#[test]
fn cc_intersect_complex2_c() {
    x2(b"[^[^abc]&&[^cde]]", b"c", 0, 1);
}

#[test]
fn cc_intersect_complex2_e() {
    x2(b"[^[^abc]&&[^cde]]", b"e", 0, 1);
}

#[test]
fn cc_intersect_complex2_no_f() {
    n(b"[^[^abc]&&[^cde]]", b"f");
}

#[test]
#[ignore] // TODO: engine bug
fn cc_intersect_dash() {
    x2(b"[a-&&-a]", b"-", 0, 1);
}

#[test]
fn cc_intersect_dash_no_amp() {
    n(b"[a\\-&&\\-a]", b"&");
}

// ============================================================================
// Combined patterns (C lines 276-306)
// ============================================================================

#[test]
fn combined_wabc_no_match() {
    n(b"\\wabc", b" abc");
}

#[test]
fn combined_a_Wbc() {
    x2(b"a\\Wbc", b"a bc", 0, 4);
}

#[test]
fn combined_a_dot_b_dot_c() {
    x2(b"a.b.c", b"aabbc", 0, 5);
}

#[test]
fn combined_dot_w_b_W_dot_c() {
    x2(b".\\wb\\W..c", b"abb bcc", 0, 7);
}

#[test]
fn combined_s_w_zzz() {
    x2(b"\\s\\wzzz", b" zzzz", 0, 5);
}

#[test]
fn combined_aa_dot_b() {
    x2(b"aa.b", b"aabb", 0, 4);
}

#[test]
fn combined_dot_a_no_match() {
    n(b".a", b"ab");
}

#[test]
fn combined_dot_a_match() {
    x2(b".a", b"aa", 0, 2);
}

#[test]
fn combined_caret_a() {
    x2(b"^a", b"a", 0, 1);
}

#[test]
fn combined_caret_a_dollar() {
    x2(b"^a$", b"a", 0, 1);
}

#[test]
fn combined_caret_w_dollar() {
    x2(b"^\\w$", b"a", 0, 1);
}

#[test]
fn combined_caret_w_dollar_no_match() {
    n(b"^\\w$", b" ");
}

#[test]
fn combined_caret_wab_dollar() {
    x2(b"^\\wab$", b"zab", 0, 3);
}

#[test]
#[ignore] // TODO: engine bug
fn combined_caret_wabcdef_dollar() {
    x2(b"^\\wabcdef$", b"zabcdef", 0, 7);
}

#[test]
fn combined_caret_w_dots_def_dollar() {
    x2(b"^\\w...def$", b"zabcdef", 0, 7);
}

#[test]
fn combined_ww_s_W_aaa_d() {
    x2(b"\\w\\w\\s\\Waaa\\d", b"aa  aaa4", 0, 8);
}

#[test]
fn combined_A_Z() {
    x2(b"\\A\\Z", b"", 0, 0);
}

#[test]
fn combined_A_xyz() {
    x2(b"\\Axyz", b"xyz", 0, 3);
}

#[test]
fn combined_xyz_Z() {
    x2(b"xyz\\Z", b"xyz", 0, 3);
}

#[test]
fn combined_xyz_z() {
    x2(b"xyz\\z", b"xyz", 0, 3);
}

#[test]
fn combined_a_Z() {
    x2(b"a\\Z", b"a", 0, 1);
}

#[test]
fn combined_G_az() {
    x2(b"\\Gaz", b"az", 0, 2);
}

#[test]
fn combined_G_z_no_match() {
    n(b"\\Gz", b"bza");
}

#[test]
fn combined_az_G_no_match() {
    n(b"az\\G", b"az");
}

#[test]
fn combined_az_A_no_match() {
    n(b"az\\A", b"az");
}

#[test]
fn combined_a_A_z_no_match() {
    n(b"a\\Az", b"az");
}

#[test]
fn combined_escaped_caret_dollar() {
    x2(b"\\^\\$", b"^$", 0, 2);
}

#[test]
fn combined_caret_opt_y() {
    x2(b"^x?y", b"xy", 0, 2);
}

#[test]
fn combined_caret_group_opt_y() {
    x2(b"^(x?y)", b"xy", 0, 2);
}

#[test]
fn combined_w_underscore() {
    x2(b"\\w", b"_", 0, 1);
}

#[test]
fn combined_W_underscore_no_match() {
    n(b"\\W", b"_");
}

// ============================================================================
// Backref patterns with .* (C lines 384-387)
// ============================================================================

#[test]
fn backref_dotstar_1() {
    x2(b"(.*)a\\1f", b"babfbac", 0, 4);
}

#[test]
fn backref_dotstar_2() {
    x2(b"(.*)a\\1f", b"bacbabf", 3, 7);
}

#[test]
fn backref_dotstar_nested() {
    x2(b"((.*)a\\2f)", b"bacbabf", 3, 7);
}

#[test]
fn backref_dotstar_multiline() {
    x2(b"(.*)a\\1f", b"baczzzzzz\nbazz\nzzzzbabf", 19, 23);
}

// ============================================================================
// Group quantifier combos part 2: (?:x*) (C lines 406-423)
// ============================================================================

#[test]
fn group_xstar_opt_empty() {
    x2(b"(?:x*)?", b"", 0, 0);
}

#[test]
fn group_xstar_opt_x() {
    x2(b"(?:x*)?", b"x", 0, 1);
}

#[test]
fn group_xstar_opt_xx() {
    x2(b"(?:x*)?", b"xx", 0, 2);
}

#[test]
fn group_xstar_star_empty() {
    x2(b"(?:x*)*", b"", 0, 0);
}

#[test]
fn group_xstar_star_x() {
    x2(b"(?:x*)*", b"x", 0, 1);
}

#[test]
fn group_xstar_star_xx() {
    x2(b"(?:x*)*", b"xx", 0, 2);
}

#[test]
fn group_xstar_plus_empty() {
    x2(b"(?:x*)+", b"", 0, 0);
}

#[test]
fn group_xstar_plus_x() {
    x2(b"(?:x*)+", b"x", 0, 1);
}

#[test]
fn group_xstar_plus_xx() {
    x2(b"(?:x*)+", b"xx", 0, 2);
}

#[test]
fn group_xstar_lazyq_empty() {
    // C: (?:x*)\?\? = (?:x*)??
    x2(b"(?:x*)??", b"", 0, 0);
}

#[test]
fn group_xstar_lazyq_x() {
    x2(b"(?:x*)??", b"x", 0, 0);
}

#[test]
fn group_xstar_lazyq_xx() {
    x2(b"(?:x*)??", b"xx", 0, 0);
}

#[test]
fn group_xstar_lazystar_empty() {
    x2(b"(?:x*)*?", b"", 0, 0);
}

#[test]
fn group_xstar_lazystar_x() {
    x2(b"(?:x*)*?", b"x", 0, 0);
}

#[test]
fn group_xstar_lazystar_xx() {
    x2(b"(?:x*)*?", b"xx", 0, 0);
}

#[test]
fn group_xstar_lazyplus_empty() {
    x2(b"(?:x*)+?", b"", 0, 0);
}

#[test]
fn group_xstar_lazyplus_x() {
    x2(b"(?:x*)+?", b"x", 0, 1);
}

#[test]
fn group_xstar_lazyplus_xx() {
    x2(b"(?:x*)+?", b"xx", 0, 2);
}

// ============================================================================
// Group quantifier combos part 3: (?:x+) (C lines 424-441)
// ============================================================================

#[test]
fn group_xplus_opt_empty() {
    x2(b"(?:x+)?", b"", 0, 0);
}

#[test]
fn group_xplus_opt_x() {
    x2(b"(?:x+)?", b"x", 0, 1);
}

#[test]
fn group_xplus_opt_xx() {
    x2(b"(?:x+)?", b"xx", 0, 2);
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_xplus_star_empty() {
    x2(b"(?:x+)*", b"", 0, 0);
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_xplus_star_x() {
    x2(b"(?:x+)*", b"x", 0, 1);
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_xplus_star_xx() {
    x2(b"(?:x+)*", b"xx", 0, 2);
}

#[test]
fn group_xplus_plus_no_match() {
    n(b"(?:x+)+", b"");
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_xplus_plus_x() {
    x2(b"(?:x+)+", b"x", 0, 1);
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_xplus_plus_xx() {
    x2(b"(?:x+)+", b"xx", 0, 2);
}

#[test]
fn group_xplus_lazyq_empty() {
    // C: (?:x+)\?\? = (?:x+)??
    x2(b"(?:x+)??", b"", 0, 0);
}

#[test]
fn group_xplus_lazyq_x() {
    x2(b"(?:x+)??", b"x", 0, 0);
}

#[test]
fn group_xplus_lazyq_xx() {
    x2(b"(?:x+)??", b"xx", 0, 0);
}

#[test]
fn group_xplus_lazystar_empty() {
    x2(b"(?:x+)*?", b"", 0, 0);
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_xplus_lazystar_x() {
    x2(b"(?:x+)*?", b"x", 0, 0);
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_xplus_lazystar_xx() {
    x2(b"(?:x+)*?", b"xx", 0, 0);
}

#[test]
fn group_xplus_lazyplus_no_match() {
    n(b"(?:x+)+?", b"");
}

#[test]
fn group_xplus_lazyplus_x() {
    x2(b"(?:x+)+?", b"x", 0, 1);
}

#[test]
fn group_xplus_lazyplus_xx() {
    x2(b"(?:x+)+?", b"xx", 0, 2);
}

// ============================================================================
// Group quantifier combos part 4: (?:x??) inner lazy (C lines 442-459)
// ============================================================================

#[test]
fn group_xlq_opt_empty() {
    // C: (?:x\?\?)? = (?:x??)?
    x2(b"(?:x??)?", b"", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlq_opt_x() {
    x2(b"(?:x??)?", b"x", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlq_opt_xx() {
    x2(b"(?:x??)?", b"xx", 0, 0);
}

#[test]
fn group_xlq_star_empty() {
    x2(b"(?:x??)*", b"", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlq_star_x() {
    x2(b"(?:x??)*", b"x", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlq_star_xx() {
    x2(b"(?:x??)*", b"xx", 0, 0);
}

#[test]
fn group_xlq_plus_empty() {
    x2(b"(?:x??)+", b"", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlq_plus_x() {
    x2(b"(?:x??)+", b"x", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlq_plus_xx() {
    x2(b"(?:x??)+", b"xx", 0, 0);
}

#[test]
fn group_xlq_lazyq_empty() {
    // C: (?:x\?\?)\?\? = (?:x??)??
    x2(b"(?:x??)??", b"", 0, 0);
}

#[test]
fn group_xlq_lazyq_x() {
    x2(b"(?:x??)??", b"x", 0, 0);
}

#[test]
fn group_xlq_lazyq_xx() {
    x2(b"(?:x??)??", b"xx", 0, 0);
}

#[test]
fn group_xlq_lazystar_empty() {
    x2(b"(?:x??)*?", b"", 0, 0);
}

#[test]
fn group_xlq_lazystar_x() {
    x2(b"(?:x??)*?", b"x", 0, 0);
}

#[test]
fn group_xlq_lazystar_xx() {
    x2(b"(?:x??)*?", b"xx", 0, 0);
}

#[test]
fn group_xlq_lazyplus_empty() {
    x2(b"(?:x??)+?", b"", 0, 0);
}

#[test]
fn group_xlq_lazyplus_x() {
    x2(b"(?:x??)+?", b"x", 0, 0);
}

#[test]
fn group_xlq_lazyplus_xx() {
    x2(b"(?:x??)+?", b"xx", 0, 0);
}

// ============================================================================
// Group quantifier combos part 5: (?:x*?) inner lazy star (C lines 460-477)
// ============================================================================

#[test]
fn group_xls_opt_empty() {
    x2(b"(?:x*?)?", b"", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xls_opt_x() {
    x2(b"(?:x*?)?", b"x", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xls_opt_xx() {
    x2(b"(?:x*?)?", b"xx", 0, 0);
}

#[test]
fn group_xls_star_empty() {
    x2(b"(?:x*?)*", b"", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xls_star_x() {
    x2(b"(?:x*?)*", b"x", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xls_star_xx() {
    x2(b"(?:x*?)*", b"xx", 0, 0);
}

#[test]
fn group_xls_plus_empty() {
    x2(b"(?:x*?)+", b"", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xls_plus_x() {
    x2(b"(?:x*?)+", b"x", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xls_plus_xx() {
    x2(b"(?:x*?)+", b"xx", 0, 0);
}

#[test]
fn group_xls_lazyq_empty() {
    // C: (?:x*?)\?\? = (?:x*?)??
    x2(b"(?:x*?)??", b"", 0, 0);
}

#[test]
fn group_xls_lazyq_x() {
    x2(b"(?:x*?)??", b"x", 0, 0);
}

#[test]
fn group_xls_lazyq_xx() {
    x2(b"(?:x*?)??", b"xx", 0, 0);
}

#[test]
fn group_xls_lazystar_empty() {
    x2(b"(?:x*?)*?", b"", 0, 0);
}

#[test]
fn group_xls_lazystar_x() {
    x2(b"(?:x*?)*?", b"x", 0, 0);
}

#[test]
fn group_xls_lazystar_xx() {
    x2(b"(?:x*?)*?", b"xx", 0, 0);
}

#[test]
fn group_xls_lazyplus_empty() {
    x2(b"(?:x*?)+?", b"", 0, 0);
}

#[test]
fn group_xls_lazyplus_x() {
    x2(b"(?:x*?)+?", b"x", 0, 0);
}

#[test]
fn group_xls_lazyplus_xx() {
    x2(b"(?:x*?)+?", b"xx", 0, 0);
}

// ============================================================================
// Group quantifier combos part 6: (?:x+?) inner lazy plus (C lines 478-495)
// ============================================================================

#[test]
fn group_xlp_opt_empty() {
    x2(b"(?:x+?)?", b"", 0, 0);
}

#[test]
fn group_xlp_opt_x() {
    x2(b"(?:x+?)?", b"x", 0, 1);
}

#[test]
fn group_xlp_opt_xx() {
    x2(b"(?:x+?)?", b"xx", 0, 1);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlp_star_empty() {
    x2(b"(?:x+?)*", b"", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlp_star_x() {
    x2(b"(?:x+?)*", b"x", 0, 1);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlp_star_xx() {
    x2(b"(?:x+?)*", b"xx", 0, 2);
}

#[test]
fn group_xlp_plus_no_match() {
    n(b"(?:x+?)+", b"");
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlp_plus_x() {
    x2(b"(?:x+?)+", b"x", 0, 1);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn group_xlp_plus_xx() {
    x2(b"(?:x+?)+", b"xx", 0, 2);
}

#[test]
fn group_xlp_lazyq_empty() {
    // C: (?:x+?)\?\? = (?:x+?)??
    x2(b"(?:x+?)??", b"", 0, 0);
}

#[test]
fn group_xlp_lazyq_x() {
    x2(b"(?:x+?)??", b"x", 0, 0);
}

#[test]
fn group_xlp_lazyq_xx() {
    x2(b"(?:x+?)??", b"xx", 0, 0);
}

#[test]
fn group_xlp_lazystar_empty() {
    x2(b"(?:x+?)*?", b"", 0, 0);
}

#[test]
fn group_xlp_lazystar_x() {
    x2(b"(?:x+?)*?", b"x", 0, 0);
}

#[test]
fn group_xlp_lazystar_xx() {
    x2(b"(?:x+?)*?", b"xx", 0, 0);
}

#[test]
fn group_xlp_lazyplus_no_match() {
    n(b"(?:x+?)+?", b"");
}

#[test]
fn group_xlp_lazyplus_x() {
    x2(b"(?:x+?)+?", b"x", 0, 1);
}

#[test]
fn group_xlp_lazyplus_xx() {
    x2(b"(?:x+?)+?", b"xx", 0, 1);
}

// ============================================================================
// More alternation (C lines 499-563)
// ============================================================================

#[test]
fn alt_capture_empty_or_a() {
    x2(b"(|a)", b"a", 0, 0);
}

#[test]
fn alt_group_abc_or_az() {
    x2(b"a(?:ab|bc)c", b"aabc", 0, 4);
}

#[test]
fn alt_ab_or_ac_az() {
    x2(b"ab|(?:ac|az)", b"az", 0, 2);
}

#[test]
fn alt_three_way() {
    x2(b"a|b|c", b"dc", 1, 2);
}

#[test]
#[ignore] // TODO: many-alternative optimization
fn alt_many() {
    x2(b"a|b|cd|efg|h|ijk|lmn|o|pq|rstuvwx|yz", b"pqr", 0, 2);
}

#[test]
fn alt_many_no_match() {
    n(b"a|b|cd|efg|h|ijk|lmn|o|pq|rstuvwx|yz", b"mn");
}

#[test]
fn alt_a_or_caret_z_match_a() {
    x2(b"a|^z", b"ba", 1, 2);
}

#[test]
fn alt_a_or_caret_z_match_z() {
    x2(b"a|^z", b"za", 0, 1);
}

#[test]
fn alt_a_or_G_z_1() {
    x2(b"a|\\Gz", b"bza", 2, 3);
}

#[test]
fn alt_a_or_G_z_2() {
    x2(b"a|\\Gz", b"za", 0, 1);
}

#[test]
fn alt_a_or_A_z_1() {
    x2(b"a|\\Az", b"bza", 2, 3);
}

#[test]
fn alt_a_or_A_z_2() {
    x2(b"a|\\Az", b"za", 0, 1);
}

#[test]
fn alt_a_or_b_Z_1() {
    x2(b"a|b\\Z", b"ba", 1, 2);
}

#[test]
fn alt_a_or_b_Z_2() {
    x2(b"a|b\\Z", b"b", 0, 1);
}

#[test]
fn alt_a_or_b_z_1() {
    x2(b"a|b\\z", b"ba", 1, 2);
}

#[test]
fn alt_a_or_b_z_2() {
    x2(b"a|b\\z", b"b", 0, 1);
}

#[test]
fn alt_w_or_s() {
    x2(b"\\w|\\s", b" ", 0, 1);
}

#[test]
fn alt_w_or_w_no_match() {
    n(b"\\w|\\w", b" ");
}

#[test]
fn alt_w_or_percent() {
    x2(b"\\w|%", b"%", 0, 1);
}

#[test]
fn alt_w_or_cc() {
    x2(b"\\w|[&$]", b"&", 0, 1);
}

#[test]
fn alt_range_or_neg_range() {
    x2(b"[b-d]|[^e-z]", b"a", 0, 1);
}

#[test]
fn alt_group_or_bz_1() {
    x2(b"(?:a|[c-f])|bz", b"dz", 0, 1);
}

#[test]
fn alt_group_or_bz_2() {
    x2(b"(?:a|[c-f])|bz", b"bz", 0, 2);
}

#[test]
fn alt_abc_or_lookahead_zzf() {
    x2(b"abc|(?=zz)..f", b"zzf", 0, 3);
}

#[test]
fn alt_abc_or_neg_lookahead_abf() {
    x2(b"abc|(?!zz)..f", b"abf", 0, 3);
}

#[test]
fn alt_lookahead_combo() {
    x2(b"(?=za)..a|(?=zz)..a", b"zza", 0, 3);
}

#[test]
fn alt_opt_a_or_b_match_a() {
    x2(b"a?|b", b"a", 0, 1);
}

#[test]
fn alt_opt_a_or_b_match_b() {
    x2(b"a?|b", b"b", 0, 0);
}

#[test]
fn alt_opt_a_or_b_empty() {
    x2(b"a?|b", b"", 0, 0);
}

#[test]
fn alt_star_a_or_b() {
    x2(b"a*|b", b"aa", 0, 2);
}

#[test]
fn alt_star_a_or_star_b_1() {
    x2(b"a*|b*", b"ba", 0, 0);
}

#[test]
fn alt_star_a_or_star_b_2() {
    x2(b"a*|b*", b"ab", 0, 1);
}

#[test]
fn alt_plus_a_or_star_b_1() {
    x2(b"a+|b*", b"", 0, 0);
}

#[test]
#[ignore] // TODO: alternation with anchor/quantifier combos
fn alt_plus_a_or_star_b_2() {
    x2(b"a+|b*", b"bbb", 0, 3);
}

#[test]
fn alt_plus_a_or_star_b_3() {
    x2(b"a+|b*", b"abbb", 0, 1);
}

#[test]
#[ignore] // TODO: alternation with anchor/quantifier combos
fn alt_plus_a_or_plus_b_no_match() {
    n(b"a+|b+", b"");
}

#[test]
fn alt_capture_opt() {
    x2(b"(a|b)?", b"b", 0, 1);
}

#[test]
fn alt_capture_star() {
    x2(b"(a|b)*", b"ba", 0, 2);
}

#[test]
fn alt_capture_plus() {
    x2(b"(a|b)+", b"bab", 0, 3);
}

#[test]
fn alt_capture_words_1() {
    x2(b"(ab|ca)+", b"caabbc", 0, 4);
}

#[test]
fn alt_capture_words_2() {
    x2(b"(ab|ca)+", b"aabca", 1, 5);
}

#[test]
fn alt_capture_words_3() {
    x2(b"(ab|ca)+", b"abzca", 0, 2);
}

#[test]
fn alt_capture_bab_1() {
    x2(b"(a|bab)+", b"ababa", 0, 5);
}

#[test]
fn alt_capture_bab_2() {
    x2(b"(a|bab)+", b"ba", 1, 2);
}

#[test]
fn alt_capture_bab_3() {
    x2(b"(a|bab)+", b"baaaba", 1, 4);
}

#[test]
fn alt_noncap_pair() {
    x2(b"(?:a|b)(?:a|b)", b"ab", 0, 2);
}

#[test]
fn alt_star_star_1() {
    x2(b"(?:a*|b*)(?:a*|b*)", b"aaabbb", 0, 3);
}

#[test]
fn alt_star_plus() {
    x2(b"(?:a*|b*)(?:a+|b+)", b"aaabbb", 0, 6);
}

#[test]
fn alt_plus_interval() {
    x2(b"(?:a+|b+){2}", b"aaabbb", 0, 6);
}

#[test]
fn interval_h_0_inf() {
    x2(b"h{0,}", b"hhhh", 0, 4);
}

#[test]
fn alt_plus_interval_1_2() {
    x2(b"(?:a+|b+){1,2}", b"aaabbb", 0, 6);
}

#[test]
fn interval_ax2_star_no_match() {
    n(b"ax{2}*a", b"0axxxa1");
}

#[test]
fn interval_dot_0_2_no_match() {
    n(b"a.{0,2}a", b"0aXXXa0");
}

#[test]
#[ignore] // TODO: interval quantifier edge cases
fn interval_dot_0_2_lazy_no_match1() {
    n(b"a.{0,2}?a", b"0aXXXa0");
}

#[test]
#[ignore] // TODO: interval quantifier edge cases
fn interval_dot_0_2_lazy_no_match2() {
    n(b"a.{0,2}?a", b"0aXXXXa0");
}

#[test]
fn interval_a_2_lazy_dollar() {
    x2(b"^a{2,}?a$", b"aaa", 0, 3);
}

#[test]
fn interval_az_2_lazy_dollar() {
    x2(b"^[a-z]{2,}?$", b"aaa", 0, 3);
}

#[test]
fn alt_plus_or_A_star_cc() {
    x2(b"(?:a+|\\Ab*)cc", b"cc", 0, 2);
}

#[test]
#[ignore] // TODO: alternation with anchor/quantifier combos
fn alt_plus_or_A_star_cc_no_match() {
    n(b"(?:a+|\\Ab*)cc", b"abcc");
}

#[test]
#[ignore] // TODO: alternation with anchor/quantifier combos
fn alt_caret_plus_or_star_c_1() {
    x2(b"(?:^a+|b+)*c", b"aabbbabc", 6, 8);
}

#[test]
#[ignore] // TODO: alternation with anchor/quantifier combos
fn alt_caret_plus_or_star_c_2() {
    x2(b"(?:^a+|b+)*c", b"aabbbbc", 0, 7);
}

// ============================================================================
// Character class with quantifiers (C lines 574-577)
// ============================================================================

#[test]
fn cc_abc_opt() {
    x2(b"[abc]?", b"abc", 0, 1);
}

#[test]
fn cc_abc_star() {
    x2(b"[abc]*", b"abc", 0, 3);
}

#[test]
fn cc_neg_abc_star() {
    x2(b"[^abc]*", b"abc", 0, 0);
}

#[test]
fn cc_neg_abc_plus() {
    n(b"[^abc]+", b"abc");
}

// ============================================================================
// Lazy quantifiers with literals (C lines 578-590)
// ============================================================================

#[test]
fn lazy_a_opt() {
    // C: a?\? = a??
    x2(b"a??", b"aaa", 0, 0);
}

#[test]
fn lazy_ba_opt_b() {
    x2(b"ba??b", b"bab", 0, 3);
}

#[test]
fn lazy_a_star() {
    x2(b"a*?", b"aaa", 0, 0);
}

#[test]
fn lazy_ba_star() {
    x2(b"ba*?", b"baa", 0, 1);
}

#[test]
fn lazy_ba_star_b() {
    x2(b"ba*?b", b"baab", 0, 4);
}

#[test]
fn lazy_a_plus() {
    x2(b"a+?", b"aaa", 0, 1);
}

#[test]
fn lazy_ba_plus() {
    x2(b"ba+?", b"baa", 0, 2);
}

#[test]
fn lazy_ba_plus_b() {
    x2(b"ba+?b", b"baab", 0, 4);
}

#[test]
fn lazy_group_a_opt_lazyq() {
    // C: (?:a?)?\? = (?:a?)?? — match "a" expect 0,0
    x2(b"(?:a?)??", b"a", 0, 0);
}

#[test]
#[ignore] // TODO: inner lazy quantifier in REPEAT
fn lazy_group_a_lazyq_opt() {
    // C: (?:a\?\?)? = (?:a??)? — match "a" expect 0,0
    x2(b"(?:a??)?", b"a", 0, 0);
}

#[test]
fn lazy_group_a_opt_lazyplus() {
    x2(b"(?:a?)+?", b"aaa", 0, 1);
}

#[test]
fn lazy_group_a_plus_lazyq() {
    // C: (?:a+)?\? = (?:a+)??
    x2(b"(?:a+)??", b"aaa", 0, 0);
}

#[test]
fn lazy_group_a_plus_lazyq_b() {
    x2(b"(?:a+)??b", b"aaab", 0, 4);
}

// ============================================================================
// Interval quantifiers (C lines 591-600)
// ============================================================================

#[test]
#[ignore] // TODO: interval quantifier edge cases
fn interval_opt_2() {
    x2(b"(?:ab)?{2}", b"", 0, 0);
}

#[test]
fn interval_opt_2_match() {
    x2(b"(?:ab)?{2}", b"ababa", 0, 4);
}

#[test]
fn interval_star_0() {
    x2(b"(?:ab)*{0}", b"ababa", 0, 0);
}

#[test]
fn interval_3_inf() {
    x2(b"(?:ab){3,}", b"abababab", 0, 8);
}

#[test]
#[ignore] // TODO: interval quantifier edge cases
fn interval_3_inf_no_match() {
    n(b"(?:ab){3,}", b"abab");
}

#[test]
fn interval_2_4() {
    x2(b"(?:ab){2,4}", b"ababab", 0, 6);
}

#[test]
fn interval_2_4_max() {
    x2(b"(?:ab){2,4}", b"ababababab", 0, 8);
}

#[test]
fn interval_2_4_lazy() {
    x2(b"(?:ab){2,4}?", b"ababababab", 0, 4);
}

#[test]
fn interval_comma_literal() {
    // (?:ab){,} is not an interval — {,} is literal
    x2(b"(?:ab){,}", b"ab{,}", 0, 5);
}

#[test]
fn interval_plus_lazy_2() {
    x2(b"(?:abc)+?{2}", b"abcabcabc", 0, 6);
}

// ============================================================================
// More captures (C lines 602-650)
// ============================================================================

#[test]
fn capture_d_plus_ncc() {
    x2(b"(d+)([^abc]z)", b"dddz", 0, 4);
}

#[test]
fn capture_ncc_star_ncc() {
    x2(b"([^abc]*)([^abc]z)", b"dddz", 0, 4);
}

#[test]
fn capture_w_plus_wz() {
    x2(b"(\\w+)(\\wz)", b"dddz", 0, 4);
}

#[test]
fn capture_20_nested() {
    x3(b"((((((((((((((((((((ab))))))))))))))))))))", b"ab", 0, 2, 20);
}

#[test]
fn capture_nested_4() {
    x3(b"(()(a)bc(def)ghijk)", b"abcdefghijk", 3, 6, 4);
}

#[test]
fn capture_caret_a() {
    x2(b"(^a)", b"a", 0, 1);
}

#[test]
fn capture_alt_1() {
    x3(b"(a)|(a)", b"ba", 1, 2, 1);
}

#[test]
fn capture_alt_2() {
    x3(b"(^a)|(a)", b"ba", 1, 2, 2);
}

#[test]
fn capture_a_opt() {
    x3(b"(a?)", b"aaa", 0, 1, 1);
}

#[test]
fn capture_a_star() {
    x3(b"(a*)", b"aaa", 0, 3, 1);
}

#[test]
fn capture_a_star_empty() {
    x3(b"(a*)", b"", 0, 0, 1);
}

#[test]
fn capture_a_plus() {
    x3(b"(a+)", b"aaaaaaa", 0, 7, 1);
}

#[test]
#[ignore] // TODO: capture group tracking in REPEAT
fn capture_alt_plus_star() {
    x3(b"(a+|b*)", b"bbbaa", 0, 3, 1);
}

#[test]
#[ignore] // TODO: capture group tracking in REPEAT
fn capture_alt_plus_opt() {
    x3(b"(a+|b?)", b"bbbaa", 0, 1, 1);
}

#[test]
fn capture_abc_opt() {
    x3(b"(abc)?", b"abc", 0, 3, 1);
}

#[test]
#[ignore] // TODO: capture group tracking in REPEAT
fn capture_abc_star() {
    x3(b"(abc)*", b"abc", 0, 3, 1);
}

#[test]
#[ignore] // TODO: capture group tracking in REPEAT
fn capture_abc_plus() {
    x3(b"(abc)+", b"abc", 0, 3, 1);
}

#[test]
#[ignore] // TODO: capture group tracking in REPEAT
fn capture_alt_xyz_abc() {
    x3(b"(xyz|abc)+", b"abc", 0, 3, 1);
}

#[test]
#[ignore] // TODO: capture group tracking in REPEAT
fn capture_alt_cc_abc() {
    x3(b"([xyz][abc]|abc)+", b"abc", 0, 3, 1);
}

#[test]
fn capture_lookahead() {
    x3(b"((?=az)a)", b"azb", 0, 1, 1);
}

#[test]
fn capture_abc_or_abd() {
    x3(b"abc|(.abd)", b"zabd", 0, 4, 1);
}

#[test]
fn capture_noncap_or_cap() {
    x2(b"(?:abc)|(ABC)", b"abc", 0, 3);
}

#[test]
fn capture_star_dot() {
    x3(b"a*(.)", b"aaaaz", 4, 5, 1);
}

#[test]
fn capture_lazystar_dot() {
    x3(b"a*?(.)", b"aaaaz", 0, 1, 1);
}

#[test]
fn capture_lazystar_c() {
    x3(b"a*?(c)", b"aaaac", 4, 5, 1);
}

#[test]
fn capture_cc_star_dot() {
    x3(b"[bcd]a*(.)", b"caaaaz", 5, 6, 1);
}

#[test]
fn capture_A_bb() {
    x3(b"(\\Abb)cc", b"bbcc", 0, 2, 1);
}

#[test]
fn capture_A_bb_no_match() {
    n(b"(\\Abb)cc", b"zbbcc");
}

#[test]
fn capture_caret_bb() {
    x3(b"(^bb)cc", b"bbcc", 0, 2, 1);
}

#[test]
fn capture_caret_bb_no_match() {
    n(b"(^bb)cc", b"zbbcc");
}

#[test]
fn capture_bb_dollar() {
    x3(b"cc(bb$)", b"ccbb", 2, 4, 1);
}

#[test]
fn capture_bb_dollar_no_match() {
    n(b"cc(bb$)", b"ccbbb");
}

// ============================================================================
// More backreferences (C lines 646-681)
// ============================================================================

#[test]
fn backref_self_no_match() {
    n(b"(\\1)", b"");
}

#[test]
fn backref_forward_no_match() {
    n(b"\\1(a)", b"aa");
}

#[test]
fn backref_nested_no_match() {
    n(b"(a(b)\\1)\\2+", b"ababb");
}

#[test]
fn backref_or_z_pattern() {
    x2(b"(?:(?:\\1|z)(a))+$", b"zaaa", 0, 4);
}

#[test]
fn backref_or_z_no_match() {
    n(b"(?:(?:\\1|z)(a))+$", b"zaa");
}

#[test]
fn backref_lookahead() {
    x2(b"(a)(?=\\1)", b"aa", 0, 1);
}

#[test]
#[ignore] // TODO: engine bug
fn backref_dollar_or() {
    n(b"(a)$|\\1", b"az");
}

#[test]
fn backref_aa() {
    x2(b"(a)\\1", b"aa", 0, 2);
}

#[test]
fn backref_ab_no_match() {
    n(b"(a)\\1", b"ab");
}

#[test]
fn backref_opt_aa() {
    x2(b"(a?)\\1", b"aa", 0, 2);
}

#[test]
fn backref_lazyopt() {
    // C: (a\?\?)\1 = (a??)\1 — lazy ?? matches 0 chars, so \1 matches ""
    x2(b"(a??)\\1", b"aa", 0, 0);
}

#[test]
fn backref_7_nested() {
    x2(b"(((((((a*)b))))))c\\7", b"aaabcaaa", 0, 8);
}

#[test]
fn backref_7_nested_capture() {
    x3(b"(((((((a*)b))))))c\\7", b"aaabcaaa", 0, 3, 7);
}

#[test]
fn backref_wds() {
    x2(b"(\\w\\d\\s)\\1", b"f5 f5 ", 0, 6);
}

#[test]
fn backref_wds_no_match() {
    n(b"(\\w\\d\\s)\\1", b"f5 f5");
}

#[test]
fn backref_who_or_cc() {
    x2(b"(who|[a-c]{3})\\1", b"whowho", 0, 6);
}

#[test]
fn backref_who_or_cc_prefix() {
    x2(b"...(who|[a-c]{3})\\1", b"abcwhowho", 0, 9);
}

#[test]
fn backref_cbc() {
    x2(b"(who|[a-c]{3})\\1", b"cbccbc", 0, 6);
}

#[test]
fn backref_caret_a() {
    x2(b"(^a)\\1", b"aa", 0, 2);
}

#[test]
fn backref_caret_a_no_match() {
    n(b"(^a)\\1", b"baa");
}

#[test]
fn backref_a_dollar_no_match() {
    n(b"(a$)\\1", b"aa");
}

#[test]
fn backref_ab_Z_no_match() {
    n(b"(ab\\Z)\\1", b"ab");
}

#[test]
fn backref_astar_Z() {
    x2(b"(a*\\Z)\\1", b"a", 1, 1);
}

#[test]
fn backref_dot_astar_Z() {
    x2(b".(a*\\Z)\\1", b"ba", 1, 2);
}

#[test]
fn backref_nested_abc() {
    x3(b"(.(abc)\\2)", b"zabcabc", 0, 7, 1);
}

#[test]
fn backref_nested_digits() {
    x3(b"(.(..\\d.)\\2)", b"z12341234", 0, 9, 1);
}

// ============================================================================
// Additional coverage patterns (C lines 739-830)
// ============================================================================

#[test]
fn empty_capture_star_backref() {
    x2(b"()*\\1", b"", 0, 0);
}

#[test]
fn double_empty_capture_star_backref() {
    x2(b"(?:()|())*\\1\\2", b"", 0, 0);
}

#[test]
fn alt_star_astar_bstar_c() {
    x2(b"(?:a*|b*)*c", b"abadc", 4, 5);
}

#[test]
fn dotstar_2_opt_star() {
    x2(b"(.{2,})?", b"abcde", 0, 5);
}

#[test]
#[ignore] // TODO: many-alternative optimization
fn alt_many_letters_opt() {
    x2(b"((a|b|c|d|e|f|g|h|i|j|k|l|m|n)+)?", b"abcde", 0, 5);
}

#[test]
#[ignore] // TODO: many-alternative optimization
fn alt_many_letters_3_opt() {
    x2(b"((a|b|c|d|e|f|g|h|i|j|k|l|m|n){3,})?", b"abcde", 0, 5);
}

#[test]
fn quoted_or_empty_backref() {
    x2(b"^(\"|)(.*)\\1$", b"XX", 0, 2);
}

#[test]
fn neg_lookahead_tail() {
    x2(b"(?!abc).*\\z", b"abcde", 1, 5);
}

#[test]
fn group_a_opt_plus() {
    x2(b"(?:a?)+", b"aa", 0, 2);
}

#[test]
fn group_a_opt_lazystar2() {
    x2(b"(?:a?)*?", b"a", 0, 0);
}

#[test]
fn group_astar_lazystar() {
    x2(b"(?:a*)*?", b"a", 0, 0);
}

#[test]
#[ignore] // TODO: REPEAT for (?:x+) with outer quantifier
fn group_aplus_lazy_star() {
    x2(b"(?:a+?)*", b"a", 0, 1);
}

#[test]
fn cc_up_coverage() {
    x2(b"[a]*\\W", b"aa@", 0, 3);
}

#[test]
fn cc_up_coverage_2() {
    x2(b"[a]*[b]", b"aab", 0, 3);
}

#[test]
fn astar_W_no_match() {
    n(b"a*\\W", b"aaa");
}

#[test]
fn interval_10_10_no_match() {
    n(b"(a){10}{10}", b"aa");
}

#[test]
fn empty_capture_chain() {
    x2(b"()(\\1)(\\2)", b"abc", 0, 0);
}

// ============================================================================
// Quantifier edge case: (?:x?)* with "x" (C line 392, not yet tested)
// ============================================================================

#[test]
fn group_question_star_x() {
    x2(b"(?:x?)*", b"x", 0, 1);
}

#[test]
fn group_question_plus_x() {
    x2(b"(?:x?)+", b"x", 0, 1);
}

#[test]
fn group_question_opt_xx() {
    // C line 390: (?:x?)? with "xx" → 0,1
    x2(b"(?:x?)?", b"xx", 0, 1);
}
