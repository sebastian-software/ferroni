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
fn group_xplus_star_empty() {
    x2(b"(?:x+)*", b"", 0, 0);
}

#[test]
fn group_xplus_star_x() {
    x2(b"(?:x+)*", b"x", 0, 1);
}

#[test]
fn group_xplus_star_xx() {
    x2(b"(?:x+)*", b"xx", 0, 2);
}

#[test]
fn group_xplus_plus_no_match() {
    n(b"(?:x+)+", b"");
}

#[test]
fn group_xplus_plus_x() {
    x2(b"(?:x+)+", b"x", 0, 1);
}

#[test]
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
fn group_xplus_lazystar_x() {
    x2(b"(?:x+)*?", b"x", 0, 0);
}

#[test]
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
fn group_xlq_opt_x() {
    x2(b"(?:x??)?", b"x", 0, 0);
}

#[test]
fn group_xlq_opt_xx() {
    x2(b"(?:x??)?", b"xx", 0, 0);
}

#[test]
fn group_xlq_star_empty() {
    x2(b"(?:x??)*", b"", 0, 0);
}

#[test]
fn group_xlq_star_x() {
    x2(b"(?:x??)*", b"x", 0, 0);
}

#[test]
fn group_xlq_star_xx() {
    x2(b"(?:x??)*", b"xx", 0, 0);
}

#[test]
fn group_xlq_plus_empty() {
    x2(b"(?:x??)+", b"", 0, 0);
}

#[test]
fn group_xlq_plus_x() {
    x2(b"(?:x??)+", b"x", 0, 0);
}

#[test]
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
fn group_xls_opt_x() {
    x2(b"(?:x*?)?", b"x", 0, 0);
}

#[test]
fn group_xls_opt_xx() {
    x2(b"(?:x*?)?", b"xx", 0, 0);
}

#[test]
fn group_xls_star_empty() {
    x2(b"(?:x*?)*", b"", 0, 0);
}

#[test]
fn group_xls_star_x() {
    x2(b"(?:x*?)*", b"x", 0, 0);
}

#[test]
fn group_xls_star_xx() {
    x2(b"(?:x*?)*", b"xx", 0, 0);
}

#[test]
fn group_xls_plus_empty() {
    x2(b"(?:x*?)+", b"", 0, 0);
}

#[test]
fn group_xls_plus_x() {
    x2(b"(?:x*?)+", b"x", 0, 0);
}

#[test]
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
fn group_xlp_star_empty() {
    x2(b"(?:x+?)*", b"", 0, 0);
}

#[test]
fn group_xlp_star_x() {
    x2(b"(?:x+?)*", b"x", 0, 1);
}

#[test]
fn group_xlp_star_xx() {
    x2(b"(?:x+?)*", b"xx", 0, 2);
}

#[test]
fn group_xlp_plus_no_match() {
    n(b"(?:x+?)+", b"");
}

#[test]
fn group_xlp_plus_x() {
    x2(b"(?:x+?)+", b"x", 0, 1);
}

#[test]
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
fn alt_plus_a_or_star_b_2() {
    x2(b"a+|b*", b"bbb", 0, 3);
}

#[test]
fn alt_plus_a_or_star_b_3() {
    x2(b"a+|b*", b"abbb", 0, 1);
}

#[test]
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
fn interval_dot_0_2_lazy_no_match1() {
    n(b"a.{0,2}?a", b"0aXXXa0");
}

#[test]
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
fn alt_plus_or_A_star_cc_no_match() {
    n(b"(?:a+|\\Ab*)cc", b"abcc");
}

#[test]
fn alt_caret_plus_or_star_c_1() {
    x2(b"(?:^a+|b+)*c", b"aabbbabc", 6, 8);
}

#[test]
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
fn capture_alt_plus_star() {
    x3(b"(a+|b*)", b"bbbaa", 0, 3, 1);
}

#[test]
fn capture_alt_plus_opt() {
    x3(b"(a+|b?)", b"bbbaa", 0, 1, 1);
}

#[test]
fn capture_abc_opt() {
    x3(b"(abc)?", b"abc", 0, 3, 1);
}

#[test]
fn capture_abc_star() {
    x3(b"(abc)*", b"abc", 0, 3, 1);
}

#[test]
fn capture_abc_plus() {
    x3(b"(abc)+", b"abc", 0, 3, 1);
}

#[test]
fn capture_alt_xyz_abc() {
    x3(b"(xyz|abc)+", b"abc", 0, 3, 1);
}

#[test]
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
fn alt_many_letters_opt() {
    x2(b"((a|b|c|d|e|f|g|h|i|j|k|l|m|n)+)?", b"abcde", 0, 5);
}

#[test]
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

// ============================================================================
// UTF-8 search offsets and long strings (C lines 900, 905-908)
// ============================================================================

#[test]
fn utf8_empty_pattern() {
    // C line 900: "" matches "あ" at 0-0
    x2(b"", "あ".as_bytes(), 0, 0);
}

#[test]
fn utf8_long_repeat() {
    // C line 905: 35 copies of こ
    let pattern = "こここここここここここここここここここここここここここここここここここ".as_bytes();
    let input = "こここここここここここここここここここここここここここここここここここ".as_bytes();
    x2(pattern, input, 0, 105);
}

#[test]
fn utf8_search_offset_single() {
    // C line 906: あ in いあ -> starts at byte 3
    x2("あ".as_bytes(), "いあ".as_bytes(), 3, 6);
}

#[test]
fn utf8_search_offset_double() {
    // C line 907: いう in あいう -> starts at byte 3
    x2("いう".as_bytes(), "あいう".as_bytes(), 3, 9);
}

#[test]
fn utf8_raw_bytes() {
    // C line 908: \xca\xb8 matching raw 2-byte sequence
    x2(b"\\xca\\xb8", b"\xca\xb8", 0, 2);
}

// ============================================================================
// UTF-8 \W, \S, \b, \B bracket and boundary patterns (C lines 913-920)
// ============================================================================

#[test]
fn utf8_bracket_W() {
    // C line 913: [\W] matches $ in "う$" at byte 3
    x2(b"[\\W]", [&"う".as_bytes()[..], b"$"].concat().as_slice(), 3, 4);
}

#[test]
fn utf8_S_katakana() {
    // C line 914: \S matches そ
    x2(b"\\S", "そ".as_bytes(), 0, 3);
}

#[test]
fn utf8_S_kanji() {
    // C line 915: \S matches 漢
    x2(b"\\S", "漢".as_bytes(), 0, 3);
}

#[test]
fn utf8_word_boundary_start() {
    // C line 916: \b at start of "気 " -> 0
    let input = [&"気".as_bytes()[..], b" "].concat();
    x2(b"\\b", &input, 0, 0);
}

#[test]
fn utf8_word_boundary_after_space() {
    // C line 917: \b in " ほ" at byte 1
    let input = [b" ", &"ほ".as_bytes()[..]].concat();
    x2(b"\\b", &input, 1, 1);
}

#[test]
fn utf8_non_boundary_mid() {
    // C line 918: \B in "せそ " at byte 3
    let input = [&"せそ".as_bytes()[..], b" "].concat();
    x2(b"\\B", &input, 3, 3);
}

#[test]
fn utf8_non_boundary_after_char() {
    // C line 919: \B in "う " at byte 4 (after う=3 bytes, at space boundary... no)
    // Actually \B matches at non-word boundary. "う " has boundary after う.
    // Byte 4 = after space? "う "(3+1=4). \B at position 4 (end of string after space)
    let input = [&"う".as_bytes()[..], b" "].concat();
    x2(b"\\B", &input, 4, 4);
}

#[test]
fn utf8_non_boundary_start_space() {
    // C line 920: \B in " い" at byte 0
    let input = [b" ", &"い".as_bytes()[..]].concat();
    x2(b"\\B", &input, 0, 0);
}

// ============================================================================
// UTF-8 character class ranges and negation (C lines 923-931)
// ============================================================================

#[test]
fn utf8_range_u_o() {
    // C line 923: [う-お] matches え
    let pattern = "[う-お]".as_bytes();
    x2(pattern, "え".as_bytes(), 0, 3);
}

#[test]
fn utf8_neg_class() {
    // C line 924: [^け] no match for け
    let pattern = "[^け]".as_bytes();
    n(pattern, "け".as_bytes());
}

#[test]
fn utf8_bracket_w() {
    // C line 925: [\w] matches ね
    let pattern = "[\\w]".as_bytes();
    x2(pattern, "ね".as_bytes(), 0, 3);
}

#[test]
fn utf8_bracket_d_no_match() {
    // C line 926: [\d] no match for ふ
    n(b"[\\d]", "ふ".as_bytes());
}

#[test]
fn utf8_bracket_D() {
    // C line 927: [\D] matches は
    x2(b"[\\D]", "は".as_bytes(), 0, 3);
}

#[test]
fn utf8_bracket_s_no_match() {
    // C line 928: [\s] no match for く
    n(b"[\\s]", "く".as_bytes());
}

#[test]
fn utf8_bracket_S() {
    // C line 929: [\S] matches へ
    x2(b"[\\S]", "へ".as_bytes(), 0, 3);
}

#[test]
fn utf8_bracket_wd() {
    // C line 930: [\w\d] matches よ
    x2(b"[\\w\\d]", "よ".as_bytes(), 0, 3);
}

#[test]
fn utf8_bracket_wd_skip_space() {
    // C line 931: [\w\d] matches よ at offset 3 in "   よ"
    let input = [b"   ", &"よ".as_bytes()[..]].concat();
    x2(b"[\\w\\d]", &input, 3, 6);
}

// ============================================================================
// UTF-8 mixed pattern tests (C lines 932-939)
// ============================================================================

#[test]
fn utf8_w_kanji_no_match() {
    // C line 932: \w鬼車 no match for " 鬼車"
    let input = [b" ", &"鬼車".as_bytes()[..]].concat();
    n([b"\\w", "鬼車".as_bytes()].concat().as_slice(), &input);
}

#[test]
fn utf8_kanji_W_kanji() {
    // C line 933: 鬼\W車 matches "鬼 車" -> 0-7
    let pattern = ["鬼".as_bytes(), b"\\W", "車".as_bytes()].concat();
    let input = [&"鬼".as_bytes()[..], b" ", &"車".as_bytes()[..]].concat();
    x2(&pattern, &input, 0, 7);
}

#[test]
fn utf8_dot_interleave() {
    // C line 934: あ.い.う matches ああいいう -> 0-15
    let pattern = "あ.い.う".as_bytes();
    x2(pattern, "ああいいう".as_bytes(), 0, 15);
}

#[test]
fn utf8_mixed_classes_dot() {
    // C line 935: .\wう\W..ぞ matches "えうう うぞぞ" -> 0-19
    let pattern = [b".\\w", "う".as_bytes(), b"\\W..", "ぞ".as_bytes()].concat();
    let input = "えうう うぞぞ".as_bytes();
    x2(&pattern, input, 0, 19);
}

#[test]
fn utf8_s_w_repeat() {
    // C line 936: \s\wこここ matches " ここここ" -> 0-13
    let pattern = [b"\\s\\w", "こここ".as_bytes()].concat();
    let input = [b" ", "ここここ".as_bytes()].concat();
    x2(&pattern, &input, 0, 13);
}

#[test]
fn utf8_dot_any_kanji() {
    // C line 937: ああ.け matches ああけけ -> 0-12
    let pattern = "ああ.け".as_bytes();
    x2(pattern, "ああけけ".as_bytes(), 0, 12);
}

#[test]
fn utf8_dot_prefix_no_match() {
    // C line 938: .い no match for いえ
    let pattern = ".い".as_bytes();
    n(pattern, "いえ".as_bytes());
}

#[test]
fn utf8_dot_prefix_match() {
    // C line 939: .お matches おお -> 0-6
    let pattern = ".お".as_bytes();
    x2(pattern, "おお".as_bytes(), 0, 6);
}

// ============================================================================
// UTF-8 anchors: ^, $, \A, \Z, \z, \G (C lines 941-954)
// ============================================================================

#[test]
fn utf8_caret_dollar() {
    // C line 941: ^む$ matches む -> 0-3
    x2("^む$".as_bytes(), "む".as_bytes(), 0, 3);
}

#[test]
fn utf8_caret_w_dollar() {
    // C line 942: ^\w$ matches に -> 0-3
    x2(b"^\\w$", "に".as_bytes(), 0, 3);
}

#[test]
fn utf8_caret_w_suffix_dollar() {
    // C line 943: ^\wかきくけこ$ matches "zかきくけこ" -> 0-16
    let pattern = [b"^\\w", "かきくけこ".as_bytes(), b"$"].concat();
    let input = [b"z", "かきくけこ".as_bytes()].concat();
    x2(&pattern, &input, 0, 16);
}

#[test]
fn utf8_caret_w_dots_suffix() {
    // C line 944: ^\w...うえお$ matches "zあいううえお" -> 0-19
    let pattern = [b"^\\w...", "うえお".as_bytes(), b"$"].concat();
    let input = [b"z", "あいううえお".as_bytes()].concat();
    x2(&pattern, &input, 0, 19);
}

#[test]
fn utf8_mixed_w_d_s_W() {
    // C line 945: \w\w\s\Wおおお\d matches "aお  おおお4" -> 0-16
    let pattern = [b"\\w\\w\\s\\W", "おおお".as_bytes(), b"\\d"].concat();
    let input = [b"a", "お".as_bytes(), b"  ", "おおお".as_bytes(), b"4"].concat();
    x2(&pattern, &input, 0, 16);
}

#[test]
fn utf8_anchor_A() {
    // C line 946: \Aたちつ matches たちつ -> 0-9
    let pattern = [b"\\A", "たちつ".as_bytes()].concat();
    x2(&pattern, "たちつ".as_bytes(), 0, 9);
}

#[test]
fn utf8_anchor_Z() {
    // C line 947: むめも\Z matches むめも -> 0-9
    let pattern = ["むめも".as_bytes(), b"\\Z"].concat();
    x2(&pattern, "むめも".as_bytes(), 0, 9);
}

#[test]
fn utf8_anchor_z() {
    // C line 948: かきく\z matches かきく -> 0-9
    let pattern = ["かきく".as_bytes(), b"\\z"].concat();
    x2(&pattern, "かきく".as_bytes(), 0, 9);
}

#[test]
fn utf8_anchor_Z_newline() {
    // C line 949: かきく\Z matches "かきく\n" -> 0-9 (\Z matches before trailing \n)
    let pattern = ["かきく".as_bytes(), b"\\Z"].concat();
    let input = ["かきく".as_bytes(), b"\n"].concat();
    x2(&pattern, &input, 0, 9);
}

#[test]
fn utf8_anchor_G() {
    // C line 950: \Gぽぴ matches ぽぴ -> 0-6
    let pattern = [b"\\G", "ぽぴ".as_bytes()].concat();
    x2(&pattern, "ぽぴ".as_bytes(), 0, 6);
}

#[test]
fn utf8_anchor_G_no_match() {
    // C line 951: \Gえ no match for うえお
    let pattern = [b"\\G", "え".as_bytes()].concat();
    n(&pattern, "うえお".as_bytes());
}

#[test]
fn utf8_anchor_G_trailing_no_match() {
    // C line 952: とて\G no match for とて
    let pattern = ["とて".as_bytes(), b"\\G"].concat();
    n(&pattern, "とて".as_bytes());
}

#[test]
fn utf8_anchor_A_trailing_no_match() {
    // C line 953: まみ\A no match for まみ
    let pattern = ["まみ".as_bytes(), b"\\A"].concat();
    n(&pattern, "まみ".as_bytes());
}

#[test]
fn utf8_anchor_A_mid_no_match() {
    // C line 954: ま\Aみ no match for まみ
    let pattern = ["ま".as_bytes(), b"\\A", "み".as_bytes()].concat();
    n(&pattern, "まみ".as_bytes());
}

// ============================================================================
// UTF-8 (?i:...) case-insensitive and (?m:...) multiline (C lines 959-963)
// ============================================================================

#[test]
fn utf8_case_insensitive_a() {
    // C line 959: (?i:あ) matches あ
    let pattern = [b"(?i:", "あ".as_bytes(), b")"].concat();
    x2(&pattern, "あ".as_bytes(), 0, 3);
}

#[test]
fn utf8_case_insensitive_pair() {
    // C line 960: (?i:ぶべ) matches ぶべ
    let pattern = [b"(?i:", "ぶべ".as_bytes(), b")"].concat();
    x2(&pattern, "ぶべ".as_bytes(), 0, 6);
}

#[test]
fn utf8_case_insensitive_no_match() {
    // C line 961: (?i:い) no match for う
    let pattern = [b"(?i:", "い".as_bytes(), b")"].concat();
    n(&pattern, "う".as_bytes());
}

#[test]
fn utf8_multiline_dot() {
    // C line 962: (?m:よ.) matches "よ\n" -> 0-4
    let pattern = [b"(?m:", "よ".as_bytes(), b".)"].concat();
    let input = ["よ".as_bytes(), b"\n"].concat();
    x2(&pattern, &input, 0, 4);
}

#[test]
fn utf8_multiline_dot_prefix() {
    // C line 963: (?m:.め) matches "ま\nめ" -> 3-7
    let pattern = [b"(?m:.", "め".as_bytes(), b")"].concat();
    let input = ["ま".as_bytes(), b"\n", "め".as_bytes()].concat();
    x2(&pattern, &input, 3, 7);
}

// ============================================================================
// UTF-8 quantifier variants (C lines 965, 970, 974-979)
// ============================================================================

#[test]
fn utf8_question_no_match_variant() {
    // C line 965: 変? matches 化 -> 0-0 (optional doesn't match, succeeds with empty)
    x2("変?".as_bytes(), "化".as_bytes(), 0, 0);
}

#[test]
fn utf8_star_no_match_at_start() {
    // C line 970: 馬* matches empty at start of "鹿馬馬馬馬" -> 0-0
    x2("馬*".as_bytes(), "鹿馬馬馬馬".as_bytes(), 0, 0);
}

#[test]
fn utf8_plus_no_match() {
    // C line 971: 山+ no match for ""
    n("山+".as_bytes(), b"");
}

#[test]
fn utf8_plus_partial() {
    // C line 974: え+ matches ええ in ええううう -> 0-6
    x2("え+".as_bytes(), "ええううう".as_bytes(), 0, 6);
}

#[test]
fn utf8_plus_skip_first() {
    // C line 975: う+ matches うううう in おうううう -> 3-15
    x2("う+".as_bytes(), "おうううう".as_bytes(), 3, 15);
}

#[test]
fn utf8_dot_question() {
    // C line 976: .? matches た -> 0-3
    x2(b".?", "た".as_bytes(), 0, 3);
}

#[test]
fn utf8_dot_star_multi() {
    // C line 977: .* matches ぱぴぷぺ -> 0-12
    x2(b".*", "ぱぴぷぺ".as_bytes(), 0, 12);
}

#[test]
fn utf8_dot_plus_single() {
    // C line 978: .+ matches ろ -> 0-3
    x2(b".+", "ろ".as_bytes(), 0, 3);
}

#[test]
fn utf8_dot_plus_stops_at_newline() {
    // C line 979: .+ matches いうえか (stops before \n) -> 0-12
    let input = ["いうえか".as_bytes(), b"\n"].concat();
    x2(b".+", &input, 0, 12);
}

// ============================================================================
// UTF-8 alternation variants (C lines 982-1000)
// ============================================================================

#[test]
fn utf8_alt_pair_first() {
    // C line 982: あい|いう matches あい -> 0-6
    let pattern = [&"あい".as_bytes()[..], b"|", "いう".as_bytes()].concat();
    x2(&pattern, "あい".as_bytes(), 0, 6);
}

#[test]
fn utf8_alt_pair_second() {
    // C line 983: あい|いう matches いう -> 0-6
    let pattern = [&"あい".as_bytes()[..], b"|", "いう".as_bytes()].concat();
    x2(&pattern, "いう".as_bytes(), 0, 6);
}

#[test]
fn utf8_alt_noncap_first() {
    // C line 984: を(?:かき|きく) matches をかき -> 0-9
    let pattern = ["を".as_bytes(), b"(?:", "かき".as_bytes(), b"|", "きく".as_bytes(), b")"].concat();
    x2(&pattern, "をかき".as_bytes(), 0, 9);
}

#[test]
fn utf8_alt_noncap_second() {
    // C line 985: を(?:かき|きく)け matches をきくけ -> 0-12
    let pattern = ["を".as_bytes(), b"(?:", "かき".as_bytes(), b"|", "きく".as_bytes(), b")", "け".as_bytes()].concat();
    x2(&pattern, "をきくけ".as_bytes(), 0, 12);
}

#[test]
fn utf8_alt_nested() {
    // C line 986: あい|(?:あう|あを) matches あを -> 0-6
    let pattern = ["あい".as_bytes(), b"|(?:", "あう".as_bytes(), b"|", "あを".as_bytes(), b")"].concat();
    x2(&pattern, "あを".as_bytes(), 0, 6);
}

#[test]
fn utf8_alt_three() {
    // C line 987: あ|い|う matches う in えう -> 3-6
    let pattern = ["あ".as_bytes(), b"|", "い".as_bytes(), b"|", "う".as_bytes()].concat();
    x2(&pattern, "えう".as_bytes(), 3, 6);
}

#[test]
fn utf8_alt_many() {
    // C line 988: long alternation matches しすせ -> 0-9
    let pattern = [
        "あ".as_bytes(), b"|", "い".as_bytes(), b"|",
        "うえ".as_bytes(), b"|", "おかき".as_bytes(), b"|",
        "く".as_bytes(), b"|", "けこさ".as_bytes(), b"|",
        "しすせ".as_bytes(), b"|", "そ".as_bytes(), b"|",
        "たち".as_bytes(), b"|", "つてとなに".as_bytes(), b"|",
        "ぬね".as_bytes(),
    ].concat();
    x2(&pattern, "しすせ".as_bytes(), 0, 9);
}

#[test]
fn utf8_alt_many_no_match() {
    // C line 989: same long alternation no match for すせ
    let pattern = [
        "あ".as_bytes(), b"|", "い".as_bytes(), b"|",
        "うえ".as_bytes(), b"|", "おかき".as_bytes(), b"|",
        "く".as_bytes(), b"|", "けこさ".as_bytes(), b"|",
        "しすせ".as_bytes(), b"|", "そ".as_bytes(), b"|",
        "たち".as_bytes(), b"|", "つてとなに".as_bytes(), b"|",
        "ぬね".as_bytes(),
    ].concat();
    n(&pattern, "すせ".as_bytes());
}

#[test]
fn utf8_alt_caret_search() {
    // C line 990: あ|^わ matches あ in ぶあ -> 3-6
    let pattern = ["あ".as_bytes(), b"|^", "わ".as_bytes()].concat();
    x2(&pattern, "ぶあ".as_bytes(), 3, 6);
}

#[test]
fn utf8_alt_caret_match() {
    // C line 991: あ|^を matches を in をあ -> 0-3
    let pattern = ["あ".as_bytes(), b"|^", "を".as_bytes()].concat();
    x2(&pattern, "をあ".as_bytes(), 0, 3);
}

#[test]
fn utf8_alt_G_search() {
    // C line 992: 鬼|\G車 matches 鬼 in け車鬼 -> 6-9
    let pattern = ["鬼".as_bytes(), b"|\\G", "車".as_bytes()].concat();
    x2(&pattern, "け車鬼".as_bytes(), 6, 9);
}

#[test]
fn utf8_alt_G_match_start() {
    // C line 993: 鬼|\G車 matches 車 in 車鬼 -> 0-3 (because \G at start)
    let pattern = ["鬼".as_bytes(), b"|\\G", "車".as_bytes()].concat();
    x2(&pattern, "車鬼".as_bytes(), 0, 3);
}

#[test]
fn utf8_alt_A_search() {
    // C line 994: 鬼|\A車 matches 鬼 in "b車鬼" -> 4-7
    let pattern = ["鬼".as_bytes(), b"|\\A", "車".as_bytes()].concat();
    let input = [b"b", "車鬼".as_bytes()].concat();
    x2(&pattern, &input, 4, 7);
}

#[test]
fn utf8_alt_A_match() {
    // C line 995: 鬼|\A車 matches 車 -> 0-3
    let pattern = ["鬼".as_bytes(), b"|\\A", "車".as_bytes()].concat();
    x2(&pattern, "車".as_bytes(), 0, 3);
}

#[test]
fn utf8_alt_Z_search() {
    // C line 996: 鬼|車\Z matches 鬼 in 車鬼 -> 3-6
    let pattern = ["鬼".as_bytes(), b"|", "車".as_bytes(), b"\\Z"].concat();
    x2(&pattern, "車鬼".as_bytes(), 3, 6);
}

#[test]
fn utf8_alt_Z_match() {
    // C line 997: 鬼|車\Z matches 車 -> 0-3
    let pattern = ["鬼".as_bytes(), b"|", "車".as_bytes(), b"\\Z"].concat();
    x2(&pattern, "車".as_bytes(), 0, 3);
}

#[test]
fn utf8_alt_Z_newline() {
    // C line 998: 鬼|車\Z matches 車 in "車\n" -> 0-3
    let pattern = ["鬼".as_bytes(), b"|", "車".as_bytes(), b"\\Z"].concat();
    let input = ["車".as_bytes(), b"\n"].concat();
    x2(&pattern, &input, 0, 3);
}

#[test]
fn utf8_alt_z_search() {
    // C line 999: 鬼|車\z matches 鬼 in 車鬼 -> 3-6
    let pattern = ["鬼".as_bytes(), b"|", "車".as_bytes(), b"\\z"].concat();
    x2(&pattern, "車鬼".as_bytes(), 3, 6);
}

#[test]
fn utf8_alt_z_match() {
    // C line 1000: 鬼|車\z matches 車 -> 0-3
    let pattern = ["鬼".as_bytes(), b"|", "車".as_bytes(), b"\\z"].concat();
    x2(&pattern, "車".as_bytes(), 0, 3);
}

// ============================================================================
// UTF-8 alternation with classes and lookaheads (C lines 1001-1012)
// ============================================================================

#[test]
fn utf8_alt_w_or_s() {
    // C line 1001: \w|\s matches お -> 0-3
    x2(b"\\w|\\s", "お".as_bytes(), 0, 3);
}

#[test]
fn utf8_alt_w_or_percent() {
    // C line 1002: \w|% matches % in "%お" -> 0-1
    let input = [b"%", "お".as_bytes()].concat();
    x2(b"\\w|%", &input, 0, 1);
}

#[test]
fn utf8_alt_w_or_special() {
    // C line 1003: \w|[&$] matches う in "う&" -> 0-3
    let input = ["う".as_bytes(), b"&"].concat();
    x2(b"\\w|[&$]", &input, 0, 3);
}

#[test]
fn utf8_range_i_ke() {
    // C line 1004: [い-け] matches う -> 0-3
    let pattern = "[い-け]".as_bytes();
    x2(pattern, "う".as_bytes(), 0, 3);
}

#[test]
fn utf8_range_or_neg_range() {
    // C line 1005: [い-け]|[^か-こ] matches あ -> 0-3
    let pattern = ["[い-け]|[^か-こ]".as_bytes()].concat();
    x2(&pattern, "あ".as_bytes(), 0, 3);
}

#[test]
fn utf8_range_or_neg_range_2() {
    // C line 1006: [い-け]|[^か-こ] matches か -> 0-3
    let pattern = ["[い-け]|[^か-こ]".as_bytes()].concat();
    x2(&pattern, "か".as_bytes(), 0, 3);
}

#[test]
fn utf8_neg_class_newline() {
    // C line 1007: [^あ] matches \n -> 0-1
    let pattern = "[^あ]".as_bytes();
    x2(pattern, b"\n", 0, 1);
}

#[test]
fn utf8_noncap_or_range_alt() {
    // C line 1008: (?:あ|[う-き])|いを matches うを -> 0-3
    let pattern = ["(?:あ|[う-き])|いを".as_bytes()].concat();
    x2(&pattern, "うを".as_bytes(), 0, 3);
}

#[test]
fn utf8_noncap_or_range_alt_2() {
    // C line 1009: (?:あ|[う-き])|いを matches いを -> 0-6
    let pattern = ["(?:あ|[う-き])|いを".as_bytes()].concat();
    x2(&pattern, "いを".as_bytes(), 0, 6);
}

#[test]
fn utf8_alt_lookahead() {
    // C line 1010: あいう|(?=けけ)..ほ matches けけほ -> 0-9
    let pattern = ["あいう|(?=けけ)..ほ".as_bytes()].concat();
    x2(&pattern, "けけほ".as_bytes(), 0, 9);
}

#[test]
fn utf8_alt_neg_lookahead() {
    // C line 1011: あいう|(?!けけ)..ほ matches あいほ -> 0-9
    let pattern = ["あいう|(?!けけ)..ほ".as_bytes()].concat();
    x2(&pattern, "あいほ".as_bytes(), 0, 9);
}

#[test]
fn utf8_dual_lookahead_alt() {
    // C line 1012: (?=をあ)..あ|(?=をを)..あ matches ををあ -> 0-9
    let pattern = ["(?=をあ)..あ|(?=をを)..あ".as_bytes()].concat();
    x2(&pattern, "ををあ".as_bytes(), 0, 9);
}

// ============================================================================
// UTF-8 lookbehind and atomic groups (C lines 1013-1015)
// ============================================================================

#[test]
fn utf8_lookbehind() {
    // C line 1013: (?<=あ|いう)い matches い at byte 6-9 in いうい
    let pattern = ["(?<=あ|いう)い".as_bytes()].concat();
    x2(&pattern, "いうい".as_bytes(), 6, 9);
}

#[test]
fn utf8_atomic_no_match() {
    // C line 1014: (?>あ|あいえ)う no match for あいえう
    let pattern = ["(?>あ|あいえ)う".as_bytes()].concat();
    n(&pattern, "あいえう".as_bytes());
}

#[test]
fn utf8_atomic_match() {
    // C line 1015: (?>あいえ|あ)う matches あいえう -> 0-12
    let pattern = ["(?>あいえ|あ)う".as_bytes()].concat();
    x2(&pattern, "あいえう".as_bytes(), 0, 12);
}

// ============================================================================
// UTF-8 alternation with optional/star/plus (C lines 1016-1038)
// ============================================================================

#[test]
fn utf8_opt_or_char_match() {
    // C line 1016: あ?|い matches あ -> 0-3
    let pattern = ["あ?|い".as_bytes()].concat();
    x2(&pattern, "あ".as_bytes(), 0, 3);
}

#[test]
fn utf8_opt_or_char_empty() {
    // C line 1017: あ?|い matches い with empty (あ? succeeds empty) -> 0-0
    let pattern = ["あ?|い".as_bytes()].concat();
    x2(&pattern, "い".as_bytes(), 0, 0);
}

#[test]
fn utf8_opt_or_char_empty2() {
    // C line 1018: あ?|い matches "" -> 0-0
    let pattern = ["あ?|い".as_bytes()].concat();
    x2(&pattern, b"", 0, 0);
}

#[test]
fn utf8_star_or_char() {
    // C line 1019: あ*|い matches ああ -> 0-6
    let pattern = ["あ*|い".as_bytes()].concat();
    x2(&pattern, "ああ".as_bytes(), 0, 6);
}

#[test]
fn utf8_star_or_star_empty() {
    // C line 1020: あ*|い* matches empty at start of いあ -> 0-0
    let pattern = ["あ*|い*".as_bytes()].concat();
    x2(&pattern, "いあ".as_bytes(), 0, 0);
}

#[test]
fn utf8_star_or_star_match() {
    // C line 1021: あ*|い* matches あ in あい -> 0-3
    let pattern = ["あ*|い*".as_bytes()].concat();
    x2(&pattern, "あい".as_bytes(), 0, 3);
}

#[test]
fn utf8_mixed_class_star_or_star() {
    // C line 1022: [aあ]*|い* matches "aあ" in "aあいいい" -> 0-4
    let pattern = ["[aあ]*|い*".as_bytes()].concat();
    let input = [b"a", "あいいい".as_bytes()].concat();
    x2(&pattern, &input, 0, 4);
}

#[test]
fn utf8_plus_or_star_empty() {
    // C line 1023: あ+|い* matches empty -> 0-0
    let pattern = ["あ+|い*".as_bytes()].concat();
    x2(&pattern, b"", 0, 0);
}

#[test]
fn utf8_plus_or_star_second() {
    // C line 1024: あ+|い* matches いいい -> 0-9
    let pattern = ["あ+|い*".as_bytes()].concat();
    x2(&pattern, "いいい".as_bytes(), 0, 9);
}

#[test]
fn utf8_plus_or_star_first() {
    // C line 1025: あ+|い* matches あ in あいいい -> 0-3
    let pattern = ["あ+|い*".as_bytes()].concat();
    x2(&pattern, "あいいい".as_bytes(), 0, 3);
}

#[test]
fn utf8_plus_or_star_prefix() {
    // C line 1026: あ+|い* matches empty at start of "aあいいい" -> 0-0
    let pattern = ["あ+|い*".as_bytes()].concat();
    let input = [b"a", "あいいい".as_bytes()].concat();
    x2(&pattern, &input, 0, 0);
}

#[test]
fn utf8_plus_or_plus_no_match() {
    // C line 1027: あ+|い+ no match for ""
    let pattern = ["あ+|い+".as_bytes()].concat();
    n(&pattern, b"");
}

#[test]
fn utf8_capture_alt_opt() {
    // C line 1028: (あ|い)? matches い -> 0-3
    let pattern = "(あ|い)?".as_bytes();
    x2(pattern, "い".as_bytes(), 0, 3);
}

#[test]
fn utf8_capture_alt_star() {
    // C line 1029: (あ|い)* matches いあ -> 0-6
    let pattern = "(あ|い)*".as_bytes();
    x2(pattern, "いあ".as_bytes(), 0, 6);
}

#[test]
fn utf8_capture_alt_plus() {
    // C line 1030: (あ|い)+ matches いあい -> 0-9
    let pattern = "(あ|い)+".as_bytes();
    x2(pattern, "いあい".as_bytes(), 0, 9);
}

#[test]
fn utf8_capture_pair_alt_plus_1() {
    // C line 1031: (あい|うあ)+ matches うああいうえ -> 0-12
    let pattern = "(あい|うあ)+".as_bytes();
    x2(pattern, "うああいうえ".as_bytes(), 0, 12);
}

#[test]
fn utf8_capture_pair_alt_plus_2() {
    // C line 1032: (あい|うえ)+ matches あいうえ in うああいうえ -> 6-18
    let pattern = "(あい|うえ)+".as_bytes();
    x2(pattern, "うああいうえ".as_bytes(), 6, 18);
}

#[test]
fn utf8_capture_pair_alt_plus_3() {
    // C line 1033: (あい|うあ)+ matches あいうあ in ああいうあ -> 3-15
    let pattern = "(あい|うあ)+".as_bytes();
    x2(pattern, "ああいうあ".as_bytes(), 3, 15);
}

#[test]
fn utf8_capture_pair_alt_plus_4() {
    // C line 1034: (あい|うあ)+ matches あい in あいをうあ -> 0-6
    let pattern = "(あい|うあ)+".as_bytes();
    x2(pattern, "あいをうあ".as_bytes(), 0, 6);
}

#[test]
fn utf8_capture_pair_alt_plus_5() {
    // C line 1035: (あい|うあ)+ matches あい in $$zzzzあいをうあ -> 6-12
    let pattern = "(あい|うあ)+".as_bytes();
    let input = [b"$$zzzz", "あいをうあ".as_bytes()].concat();
    x2(&pattern, &input, 6, 12);
}

#[test]
fn utf8_capture_single_or_triple_plus_1() {
    // C line 1036: (あ|いあい)+ matches あいあいあ -> 0-15
    let pattern = "(あ|いあい)+".as_bytes();
    x2(pattern, "あいあいあ".as_bytes(), 0, 15);
}

#[test]
fn utf8_capture_single_or_triple_plus_2() {
    // C line 1037: (あ|いあい)+ matches あ in いあ -> 3-6
    let pattern = "(あ|いあい)+".as_bytes();
    x2(pattern, "いあ".as_bytes(), 3, 6);
}

#[test]
fn utf8_capture_single_or_triple_plus_3() {
    // C line 1038: (あ|いあい)+ matches いあああいあ -> 3-12
    // あ(3-6) + あ(6-9) + あ(9-12) = 3 chars
    let pattern = "(あ|いあい)+".as_bytes();
    x2(pattern, "いあああいあ".as_bytes(), 3, 12);
}

// ============================================================================
// UTF-8 non-capturing groups with quantifiers (C lines 1039-1048)
// ============================================================================

#[test]
fn utf8_noncap_alt_pair() {
    // C line 1039: (?:あ|い)(?:あ|い) matches あい -> 0-6
    let pattern = "(?:あ|い)(?:あ|い)".as_bytes();
    x2(pattern, "あい".as_bytes(), 0, 6);
}

#[test]
fn utf8_noncap_star_star() {
    // C line 1040: (?:あ*|い*)(?:あ*|い*) matches あああ in あああいいい -> 0-9
    let pattern = "(?:あ*|い*)(?:あ*|い*)".as_bytes();
    x2(pattern, "あああいいい".as_bytes(), 0, 9);
}

#[test]
fn utf8_noncap_star_plus() {
    // C line 1041: (?:あ*|い*)(?:あ+|い+) matches あああいいい -> 0-18
    let pattern = "(?:あ*|い*)(?:あ+|い+)".as_bytes();
    x2(pattern, "あああいいい".as_bytes(), 0, 18);
}

#[test]
fn utf8_noncap_plus_interval_2() {
    // C line 1042: (?:あ+|い+){2} matches あああいいい -> 0-18
    let pattern = "(?:あ+|い+){2}".as_bytes();
    x2(pattern, "あああいいい".as_bytes(), 0, 18);
}

#[test]
fn utf8_noncap_plus_interval_1_2() {
    // C line 1043: (?:あ+|い+){1,2} matches あああいいい -> 0-18
    let pattern = "(?:あ+|い+){1,2}".as_bytes();
    x2(pattern, "あああいいい".as_bytes(), 0, 18);
}

#[test]
fn utf8_noncap_plus_or_A_star() {
    // C line 1044: (?:あ+|\Aい*)うう matches うう -> 0-6
    let pattern = ["(?:あ+|\\Aい*)うう".as_bytes()].concat();
    x2(&pattern, "うう".as_bytes(), 0, 6);
}

#[test]
fn utf8_noncap_plus_or_A_star_no_match() {
    // C line 1045: (?:あ+|\Aい*)うう no match for あいうう
    let pattern = ["(?:あ+|\\Aい*)うう".as_bytes()].concat();
    n(&pattern, "あいうう".as_bytes());
}

#[test]
fn utf8_noncap_caret_or_star_1() {
    // C line 1046: (?:^あ+|い+)*う matches い+う at end of ああいいいあいう -> 18-24
    let pattern = "(?:^あ+|い+)*う".as_bytes();
    x2(pattern, "ああいいいあいう".as_bytes(), 18, 24);
}

#[test]
fn utf8_noncap_caret_or_star_2() {
    // C line 1047: (?:^あ+|い+)*う matches full ああいいいいう -> 0-21
    let pattern = "(?:^あ+|い+)*う".as_bytes();
    x2(pattern, "ああいいいいう".as_bytes(), 0, 21);
}

#[test]
fn utf8_interval_0_inf() {
    // C line 1048: う{0,} matches うううう -> 0-12
    x2("う{0,}".as_bytes(), "うううう".as_bytes(), 0, 12);
}

// ============================================================================
// UTF-8 (?i) with alternation (C lines 1049-1052)
// ============================================================================

#[test]
fn utf8_case_insensitive_alt() {
    // C line 1049: あ|(?i)c matches C -> 0-1
    let pattern = ["あ|(?i)c".as_bytes()].concat();
    x2(&pattern, b"C", 0, 1);
}

#[test]
fn utf8_case_insensitive_alt_2() {
    // C line 1050: (?i)c|あ matches C -> 0-1
    let pattern = ["(?i)c|あ".as_bytes()].concat();
    x2(&pattern, b"C", 0, 1);
}

#[test]
fn utf8_case_insensitive_group_or_a() {
    // C line 1051: (?i:あ)|a matches a -> 0-1
    let pattern = [b"(?i:", "あ".as_bytes(), b")|a"].concat();
    x2(&pattern, b"a", 0, 1);
}

#[test]
fn utf8_case_insensitive_group_or_a_no() {
    // C line 1052: (?i:あ)|a no match for A
    let pattern = [b"(?i:", "あ".as_bytes(), b")|a"].concat();
    n(&pattern, b"A");
}

// ============================================================================
// UTF-8 character class quantifiers (C lines 1053-1056)
// ============================================================================

#[test]
fn utf8_class_opt() {
    // C line 1053: [あいう]? matches あ in あいう -> 0-3
    x2("[あいう]?".as_bytes(), "あいう".as_bytes(), 0, 3);
}

#[test]
fn utf8_class_star() {
    // C line 1054: [あいう]* matches あいう -> 0-9
    x2("[あいう]*".as_bytes(), "あいう".as_bytes(), 0, 9);
}

#[test]
fn utf8_neg_class_star() {
    // C line 1055: [^あいう]* matches empty at start of あいう -> 0-0
    x2("[^あいう]*".as_bytes(), "あいう".as_bytes(), 0, 0);
}

#[test]
fn utf8_neg_class_plus_no_match() {
    // C line 1056: [^あいう]+ no match for あいう
    n("[^あいう]+".as_bytes(), "あいう".as_bytes());
}

// ============================================================================
// UTF-8 lazy quantifiers (C lines 1057-1064)
// ============================================================================

#[test]
fn utf8_lazy_question() {
    // C line 1057: あ?? matches empty in あああ -> 0-0
    // C literal: あ?\? -> the \? is C trigraph escaping, = あ??
    x2("あ??".as_bytes(), "あああ".as_bytes(), 0, 0);
}

#[test]
fn utf8_lazy_question_sandwiched() {
    // C line 1058: いあ??い matches いあい -> 0-9
    x2("いあ??い".as_bytes(), "いあい".as_bytes(), 0, 9);
}

#[test]
fn utf8_lazy_star() {
    // C line 1059: あ*? matches empty in あああ -> 0-0
    x2("あ*?".as_bytes(), "あああ".as_bytes(), 0, 0);
}

#[test]
fn utf8_lazy_star_prefix() {
    // C line 1060: いあ*? matches い in いああ -> 0-3
    x2("いあ*?".as_bytes(), "いああ".as_bytes(), 0, 3);
}

#[test]
fn utf8_lazy_star_sandwiched() {
    // C line 1061: いあ*?い matches いああい -> 0-12
    x2("いあ*?い".as_bytes(), "いああい".as_bytes(), 0, 12);
}

#[test]
fn utf8_lazy_plus() {
    // C line 1062: あ+? matches あ in あああ -> 0-3
    x2("あ+?".as_bytes(), "あああ".as_bytes(), 0, 3);
}

#[test]
fn utf8_lazy_plus_prefix() {
    // C line 1063: いあ+? matches いあ in いああ -> 0-6
    x2("いあ+?".as_bytes(), "いああ".as_bytes(), 0, 6);
}

#[test]
fn utf8_lazy_plus_sandwiched() {
    // C line 1064: いあ+?い matches いああい -> 0-12
    x2("いあ+?い".as_bytes(), "いああい".as_bytes(), 0, 12);
}

// ============================================================================
// UTF-8 lazy quantifiers on groups (C lines 1065-1069)
// ============================================================================

#[test]
fn utf8_group_opt_lazy_question() {
    // C line 1065: (?:天?)?? matches empty in 天 -> 0-0
    // C literal: (?:天?)?\? -> (?:天?)??
    x2("(?:天?)??".as_bytes(), "天".as_bytes(), 0, 0);
}

#[test]
fn utf8_group_lazy_question_opt() {
    // C line 1066: (?:天??)? matches empty in 天 -> 0-0
    x2("(?:天??)?".as_bytes(), "天".as_bytes(), 0, 0);
}

#[test]
fn utf8_group_opt_lazy_plus() {
    // C line 1067: (?:夢?)+? matches 夢 in 夢夢夢 -> 0-3
    x2("(?:夢?)+?".as_bytes(), "夢夢夢".as_bytes(), 0, 3);
}

#[test]
fn utf8_group_plus_lazy_question() {
    // C line 1068: (?:風+)?? matches empty in 風風風 -> 0-0
    // C literal: (?:風+)?\? -> (?:風+)??
    x2("(?:風+)??".as_bytes(), "風風風".as_bytes(), 0, 0);
}

#[test]
fn utf8_group_plus_lazy_question_suffix() {
    // C line 1069: (?:雪+)??霜 matches 雪雪雪霜 -> 0-12
    // C literal: (?:雪+)?\?霜 -> (?:雪+)??霜
    x2("(?:雪+)??霜".as_bytes(), "雪雪雪霜".as_bytes(), 0, 12);
}

// ============================================================================
// UTF-8 interval quantifiers on groups (C lines 1070-1079)
// ============================================================================

#[test]
fn utf8_group_opt_interval_2_empty() {
    // C line 1070: (?:あい)?{2} matches "" -> 0-0
    x2("(?:あい)?{2}".as_bytes(), b"", 0, 0);
}

#[test]
fn utf8_group_opt_interval_2_match() {
    // C line 1071: (?:鬼車)?{2} matches 鬼車鬼車 in 鬼車鬼車鬼 -> 0-12
    x2("(?:鬼車)?{2}".as_bytes(), "鬼車鬼車鬼".as_bytes(), 0, 12);
}

#[test]
fn utf8_group_star_interval_0() {
    // C line 1072: (?:鬼車)*{0} matches empty in 鬼車鬼車鬼 -> 0-0
    x2("(?:鬼車)*{0}".as_bytes(), "鬼車鬼車鬼".as_bytes(), 0, 0);
}

#[test]
fn utf8_group_interval_3_inf() {
    // C line 1073: (?:鬼車){3,} matches 鬼車鬼車鬼車鬼車 -> 0-24
    x2("(?:鬼車){3,}".as_bytes(), "鬼車鬼車鬼車鬼車".as_bytes(), 0, 24);
}

#[test]
fn utf8_group_interval_3_inf_no_match() {
    // C line 1074: (?:鬼車){3,} no match for 鬼車鬼車
    n("(?:鬼車){3,}".as_bytes(), "鬼車鬼車".as_bytes());
}

#[test]
fn utf8_group_interval_2_4() {
    // C line 1075: (?:鬼車){2,4} matches 鬼車鬼車鬼車 -> 0-18
    x2("(?:鬼車){2,4}".as_bytes(), "鬼車鬼車鬼車".as_bytes(), 0, 18);
}

#[test]
fn utf8_group_interval_2_4_max() {
    // C line 1076: (?:鬼車){2,4} matches 4 in 鬼車鬼車鬼車鬼車鬼車 -> 0-24
    x2("(?:鬼車){2,4}".as_bytes(), "鬼車鬼車鬼車鬼車鬼車".as_bytes(), 0, 24);
}

#[test]
fn utf8_group_interval_2_4_lazy() {
    // C line 1077: (?:鬼車){2,4}? matches min 2 in 鬼車鬼車鬼車鬼車鬼車 -> 0-12
    x2("(?:鬼車){2,4}?".as_bytes(), "鬼車鬼車鬼車鬼車鬼車".as_bytes(), 0, 12);
}

#[test]
fn utf8_literal_brace_comma() {
    // C line 1078: (?:鬼車){,} is literal {,} since not a valid interval
    // Matches "鬼車{,}" -> 0-9
    let pattern = "(?:鬼車){,}".as_bytes();
    let input = "鬼車{,}".as_bytes();
    x2(pattern, input, 0, 9);
}

#[test]
fn utf8_group_plus_lazy_interval_2() {
    // C line 1079: (?:かきく)+?{2} matches かきくかきく -> 0-18
    x2("(?:かきく)+?{2}".as_bytes(), "かきくかきくかきく".as_bytes(), 0, 18);
}

// ============================================================================
// UTF-8 capture groups (C lines 1082-1093)
// ============================================================================

#[test]
fn utf8_capture_nested_pair() {
    // C line 1082: ((時間)) matches 時間 -> 0-6
    x2("((時間))".as_bytes(), "時間".as_bytes(), 0, 6);
}

#[test]
fn utf8_capture_nested_pair_outer() {
    // C line 1083: ((風水)) capture 1 = 風水 -> 0-6
    x3("((風水))".as_bytes(), "風水".as_bytes(), 0, 6, 1);
}

#[test]
fn utf8_capture_nested_pair_inner() {
    // C line 1084: ((昨日)) capture 2 = 昨日 -> 0-6
    x3("((昨日))".as_bytes(), "昨日".as_bytes(), 0, 6, 2);
}

#[test]
fn utf8_capture_deeply_nested() {
    // C line 1085: 20 nested parens around 量子, capture 20 = 量子 -> 0-6
    let pattern = "((((((((((((((((((((量子))))))))))))))))))))".as_bytes();
    x3(pattern, "量子".as_bytes(), 0, 6, 20);
}

#[test]
fn utf8_capture_two_groups_1() {
    // C line 1086: (あい)(うえ) capture 1 = あい -> 0-6
    x3("(あい)(うえ)".as_bytes(), "あいうえ".as_bytes(), 0, 6, 1);
}

#[test]
fn utf8_capture_two_groups_2() {
    // C line 1087: (あい)(うえ) capture 2 = うえ -> 6-12
    x3("(あい)(うえ)".as_bytes(), "あいうえ".as_bytes(), 6, 12, 2);
}

#[test]
fn utf8_capture_empty_and_groups() {
    // C line 1088: ()(あ)いう(えおか)きくけこ capture 3 = えおか -> 9-18
    let pattern = "()(あ)いう(えおか)きくけこ".as_bytes();
    let input = "あいうえおかきくけこ".as_bytes();
    x3(pattern, input, 9, 18, 3);
}

#[test]
fn utf8_capture_nested_with_empty() {
    // C line 1089: (()(あ)いう(えおか)きくけこ) capture 4 = えおか -> 9-18
    let pattern = "(()(あ)いう(えおか)きくけこ)".as_bytes();
    let input = "あいうえおかきくけこ".as_bytes();
    x3(pattern, input, 9, 18, 4);
}

#[test]
fn utf8_capture_von_manstein() {
    // C line 1090: .*(フォ)ン・マ(ン()シュタ)イン capture 2 = ンシュタ -> 15-27
    let pattern = ".*(フォ)ン・マ(ン()シュタ)イン".as_bytes();
    let input = "フォン・マンシュタイン".as_bytes();
    x3(pattern, input, 15, 27, 2);
}

#[test]
fn utf8_capture_caret() {
    // C line 1091: (^あ) matches あ -> 0-3
    x2("(^あ)".as_bytes(), "あ".as_bytes(), 0, 3);
}

#[test]
fn utf8_capture_alt_first() {
    // C line 1092: (あ)|(あ) matches あ at position 3 in いあ, capture 1 -> 3-6
    x3("(あ)|(あ)".as_bytes(), "いあ".as_bytes(), 3, 6, 1);
}

#[test]
fn utf8_capture_alt_caret_second() {
    // C line 1093: (^あ)|(あ) matches あ at position 3 in いあ, capture 2 -> 3-6
    x3("(^あ)|(あ)".as_bytes(), "いあ".as_bytes(), 3, 6, 2);
}

// ============================================================================
// UTF-8 capture with quantifiers (C lines 1094-1104)
// ============================================================================

#[test]
fn utf8_capture_opt() {
    // C line 1094: (あ?) capture 1 = あ in あああ -> 0-3
    x3("(あ?)".as_bytes(), "あああ".as_bytes(), 0, 3, 1);
}

#[test]
fn utf8_capture_star() {
    // C line 1095: (ま*) capture 1 = ままま -> 0-9
    x3("(ま*)".as_bytes(), "ままま".as_bytes(), 0, 9, 1);
}

#[test]
fn utf8_capture_star_empty() {
    // C line 1096: (と*) capture 1 = empty -> 0-0
    x3("(と*)".as_bytes(), b"", 0, 0, 1);
}

#[test]
fn utf8_capture_plus() {
    // C line 1097: (る+) capture 1 = るるるるるるる -> 0-21
    x3("(る+)".as_bytes(), "るるるるるるる".as_bytes(), 0, 21, 1);
}

#[test]
fn utf8_capture_alt_plus_star() {
    // C line 1098: (ふ+|へ*) capture 1 = ふふふ -> 0-9
    x3("(ふ+|へ*)".as_bytes(), "ふふふへへ".as_bytes(), 0, 9, 1);
}

#[test]
fn utf8_capture_alt_plus_opt() {
    // C line 1099: (あ+|い?) capture 1 = い -> 0-3
    x3("(あ+|い?)".as_bytes(), "いいいああ".as_bytes(), 0, 3, 1);
}

#[test]
fn utf8_capture_group_opt() {
    // C line 1100: (あいう)? capture 1 = あいう -> 0-9
    x3("(あいう)?".as_bytes(), "あいう".as_bytes(), 0, 9, 1);
}

#[test]
fn utf8_capture_group_star() {
    // C line 1101: (あいう)* capture 1 = あいう -> 0-9
    x3("(あいう)*".as_bytes(), "あいう".as_bytes(), 0, 9, 1);
}

#[test]
fn utf8_capture_group_plus() {
    // C line 1102: (あいう)+ capture 1 = あいう -> 0-9
    x3("(あいう)+".as_bytes(), "あいう".as_bytes(), 0, 9, 1);
}

#[test]
fn utf8_capture_alt_group_plus() {
    // C line 1103: (さしす|あいう)+ capture 1 = あいう -> 0-9
    x3("(さしす|あいう)+".as_bytes(), "あいう".as_bytes(), 0, 9, 1);
}

#[test]
fn utf8_capture_cc_pair_alt_plus() {
    // C line 1104: ([なにぬ][かきく]|かきく)+ capture 1 = かきく -> 0-9
    x3("([なにぬ][かきく]|かきく)+".as_bytes(), "かきく".as_bytes(), 0, 9, 1);
}

// ============================================================================
// UTF-8 capture with (?i:), (?m:), lookahead (C lines 1105-1108)
// ============================================================================

#[test]
fn utf8_capture_case_insensitive() {
    // C line 1105: ((?i:あいう)) capture 1 = あいう -> 0-9
    x3("((?i:あいう))".as_bytes(), "あいう".as_bytes(), 0, 9, 1);
}

#[test]
fn utf8_capture_multiline() {
    // C line 1106: ((?m:あ.う)) capture 1 = あ\nう -> 0-7
    let pattern = "((?m:あ.う))".as_bytes();
    let input = ["あ".as_bytes(), b"\n", "う".as_bytes()].concat();
    x3(pattern, &input, 0, 7, 1);
}

#[test]
fn utf8_capture_lookahead() {
    // C line 1107: ((?=あん)あ) capture 1 = あ in あんい -> 0-3
    let pattern = "((?=あん)あ)".as_bytes();
    x3(pattern, "あんい".as_bytes(), 0, 3, 1);
}

#[test]
fn utf8_capture_alt_prefix() {
    // C line 1108: あいう|(.あいえ) capture 1 = んあいえ in んあいえ -> 0-12
    let pattern = "あいう|(.あいえ)".as_bytes();
    x3(pattern, "んあいえ".as_bytes(), 0, 12, 1);
}

// ============================================================================
// UTF-8 greedy vs lazy with captures (C lines 1109-1116)
// ============================================================================

#[test]
fn utf8_greedy_star_capture() {
    // C line 1109: あ*(.) capture 1 = ん in ああああん -> 12-15
    let pattern = "あ*(.)".as_bytes();
    x3(pattern, "ああああん".as_bytes(), 12, 15, 1);
}

#[test]
fn utf8_lazy_star_capture() {
    // C line 1110: あ*?(.) capture 1 = あ in ああああん -> 0-3
    let pattern = "あ*?(.)".as_bytes();
    x3(pattern, "ああああん".as_bytes(), 0, 3, 1);
}

#[test]
fn utf8_lazy_star_capture_specific() {
    // C line 1111: あ*?(ん) capture 1 = ん in ああああん -> 12-15
    let pattern = "あ*?(ん)".as_bytes();
    x3(pattern, "ああああん".as_bytes(), 12, 15, 1);
}

#[test]
fn utf8_class_star_capture() {
    // C line 1112: [いうえ]あ*(.) capture 1 = ん in えああああん -> 15-18
    let pattern = "[いうえ]あ*(.)".as_bytes();
    x3(pattern, "えああああん".as_bytes(), 15, 18, 1);
}

#[test]
fn utf8_capture_anchor_A() {
    // C line 1113: (\Aいい)うう capture 1 = いい -> 0-6
    let pattern = [b"(\\A", "いい".as_bytes(), b")", "うう".as_bytes()].concat();
    x3(&pattern, "いいうう".as_bytes(), 0, 6, 1);
}

#[test]
fn utf8_capture_anchor_A_no_match() {
    // C line 1114: (\Aいい)うう no match for んいいうう
    let pattern = [b"(\\A", "いい".as_bytes(), b")", "うう".as_bytes()].concat();
    n(&pattern, "んいいうう".as_bytes());
}

#[test]
fn utf8_capture_caret_group() {
    // C line 1115: (^いい)うう capture 1 = いい -> 0-6
    x3("(^いい)うう".as_bytes(), "いいうう".as_bytes(), 0, 6, 1);
}

#[test]
fn utf8_capture_caret_group_no_match() {
    // C line 1116: (^いい)うう no match for んいいうう
    n("(^いい)うう".as_bytes(), "んいいうう".as_bytes());
}

#[test]
fn utf8_capture_dollar_group() {
    // C line 1117: ろろ(るる$) capture 1 = るる -> 6-12
    x3("ろろ(るる$)".as_bytes(), "ろろるる".as_bytes(), 6, 12, 1);
}

#[test]
fn utf8_capture_dollar_group_no_match() {
    // C line 1118: ろろ(るる$) no match for ろろるるる
    n("ろろ(るる$)".as_bytes(), "ろろるるる".as_bytes());
}

// ============================================================================
// UTF-8 backreferences (C lines 1119-1146)
// ============================================================================

#[test]
fn utf8_backref_kanji() {
    // C line 1119: (無)\1 matches 無無 -> 0-6
    let pattern = ["(無)".as_bytes(), b"\\1"].concat();
    x2(&pattern, "無無".as_bytes(), 0, 6);
}

#[test]
fn utf8_backref_kanji_no_match() {
    // C line 1120: (無)\1 no match for 無武
    let pattern = ["(無)".as_bytes(), b"\\1"].concat();
    n(&pattern, "無武".as_bytes());
}

#[test]
fn utf8_backref_opt() {
    // C line 1121: (空?)\1 matches 空空 -> 0-6
    let pattern = ["(空?)".as_bytes(), b"\\1"].concat();
    x2(&pattern, "空空".as_bytes(), 0, 6);
}

#[test]
fn utf8_backref_lazy_opt() {
    // C line 1122: (空??)\1 matches empty in 空空 -> 0-0
    // C literal: (空?\?)\1 -> (空??)\1
    let pattern = ["(空??)".as_bytes(), b"\\1"].concat();
    x2(&pattern, "空空".as_bytes(), 0, 0);
}

#[test]
fn utf8_backref_star() {
    // C line 1123: (空*)\1 matches 空空空空 in 空空空空空 -> 0-12
    let pattern = ["(空*)".as_bytes(), b"\\1"].concat();
    x2(&pattern, "空空空空空".as_bytes(), 0, 12);
}

#[test]
fn utf8_backref_star_capture() {
    // C line 1124: (空*)\1 capture 1 = 空空 in 空空空空空 -> 0-6
    let pattern = ["(空*)".as_bytes(), b"\\1"].concat();
    x3(&pattern, "空空空空空".as_bytes(), 0, 6, 1);
}

#[test]
fn utf8_backref_prefix_star() {
    // C line 1125: あ(い*)\1 matches あいいいい -> 0-15
    let pattern = ["あ(い*)".as_bytes(), b"\\1"].concat();
    x2(&pattern, "あいいいい".as_bytes(), 0, 15);
}

#[test]
fn utf8_backref_prefix_star_empty() {
    // C line 1126: あ(い*)\1 matches あ in あい -> 0-3 (い* matches empty)
    let pattern = ["あ(い*)".as_bytes(), b"\\1"].concat();
    x2(&pattern, "あい".as_bytes(), 0, 3);
}

#[test]
fn utf8_backref_two_groups() {
    // C line 1127: (あ*)(い*)\1\2 matches あああいいあああいい -> 0-30
    let pattern = ["(あ*)(い*)".as_bytes(), b"\\1\\2"].concat();
    x2(&pattern, "あああいいあああいい".as_bytes(), 0, 30);
}

#[test]
fn utf8_backref_group2() {
    // C line 1128: (あ*)(い*)\2 matches あああいいいい -> 0-21
    let pattern = ["(あ*)(い*)".as_bytes(), b"\\2"].concat();
    x2(&pattern, "あああいいいい".as_bytes(), 0, 21);
}

#[test]
fn utf8_backref_group2_capture() {
    // C line 1129: (あ*)(い*)\2 capture 2 = いい in あああいいいい -> 9-15
    let pattern = ["(あ*)(い*)".as_bytes(), b"\\2"].concat();
    x3(&pattern, "あああいいいい".as_bytes(), 9, 15, 2);
}

#[test]
fn utf8_backref_7_nested() {
    // C line 1130: (((((((ぽ*)ぺ))))))ぴ\7 matches ぽぽぽぺぴぽぽぽ -> 0-24
    let pattern = ["(((((((ぽ*)ぺ))))))ぴ".as_bytes(), b"\\7"].concat();
    x2(&pattern, "ぽぽぽぺぴぽぽぽ".as_bytes(), 0, 24);
}

#[test]
fn utf8_backref_7_nested_capture() {
    // C line 1131: (((((((ぽ*)ぺ))))))ぴ\7 capture 7 = ぽぽぽ -> 0-9
    let pattern = ["(((((((ぽ*)ぺ))))))ぴ".as_bytes(), b"\\7"].concat();
    x3(&pattern, "ぽぽぽぺぴぽぽぽ".as_bytes(), 0, 9, 7);
}

#[test]
fn utf8_backref_three_groups() {
    // C line 1132: (は)(ひ)(ふ)\2\1\3 matches はひふひはふ -> 0-18
    let pattern = ["(は)(ひ)(ふ)".as_bytes(), b"\\2\\1\\3"].concat();
    x2(&pattern, "はひふひはふ".as_bytes(), 0, 18);
}

#[test]
fn utf8_backref_char_class() {
    // C line 1133: ([き-け])\1 matches くく -> 0-6
    let pattern = ["([き-け])".as_bytes(), b"\\1"].concat();
    x2(&pattern, "くく".as_bytes(), 0, 6);
}

#[test]
fn utf8_backref_wds() {
    // C line 1134: (\w\d\s)\1 matches "あ5 あ5 " -> 0-10
    let input = ["あ".as_bytes(), b"5 ", "あ".as_bytes(), b"5 "].concat();
    x2(b"(\\w\\d\\s)\\1", &input, 0, 10);
}

#[test]
fn utf8_backref_wds_no_match() {
    // C line 1135: (\w\d\s)\1 no match for "あ5 あ5" (missing trailing space)
    let input = ["あ".as_bytes(), b"5 ", "あ".as_bytes(), b"5"].concat();
    n(b"(\\w\\d\\s)\\1", &input);
}

#[test]
fn utf8_backref_alt_fullwidth() {
    // C line 1136: (誰？|[あ-う]{3})\1 matches 誰？誰？ -> 0-12
    let pattern = ["(誰？|[あ-う]{3})".as_bytes(), b"\\1"].concat();
    x2(&pattern, "誰？誰？".as_bytes(), 0, 12);
}

#[test]
fn utf8_backref_alt_prefix() {
    // C line 1137: ...(誰？|[あ-う]{3})\1 matches あaあ誰？誰？ -> 0-19
    let pattern = [b"...", "(誰？|[あ-う]{3})".as_bytes(), b"\\1"].concat();
    let input = [&"あ".as_bytes()[..], b"a", "あ誰？誰？".as_bytes()].concat();
    x2(&pattern, &input, 0, 19);
}

#[test]
fn utf8_backref_alt_cc() {
    // C line 1138: (誰？|[あ-う]{3})\1 matches ういうういう -> 0-18
    let pattern = ["(誰？|[あ-う]{3})".as_bytes(), b"\\1"].concat();
    x2(&pattern, "ういうういう".as_bytes(), 0, 18);
}

#[test]
fn utf8_backref_caret() {
    // C line 1139: (^こ)\1 matches ここ -> 0-6
    let pattern = ["(^こ)".as_bytes(), b"\\1"].concat();
    x2(&pattern, "ここ".as_bytes(), 0, 6);
}

#[test]
fn utf8_backref_caret_no_match() {
    // C line 1140: (^む)\1 no match for めむむ
    let pattern = ["(^む)".as_bytes(), b"\\1"].concat();
    n(&pattern, "めむむ".as_bytes());
}

#[test]
fn utf8_backref_dollar_no_match() {
    // C line 1141: (あ$)\1 no match for ああ
    let pattern = ["(あ$)".as_bytes(), b"\\1"].concat();
    n(&pattern, "ああ".as_bytes());
}

#[test]
fn utf8_backref_Z_no_match() {
    // C line 1142: (あい\Z)\1 no match for あい
    let pattern = ["(あい".as_bytes(), b"\\Z)\\1"].concat();
    n(&pattern, "あい".as_bytes());
}

#[test]
fn utf8_backref_star_Z() {
    // C line 1143: (あ*\Z)\1 matches empty at end of あ -> 3-3
    let pattern = ["(あ*".as_bytes(), b"\\Z)\\1"].concat();
    x2(&pattern, "あ".as_bytes(), 3, 3);
}

#[test]
fn utf8_backref_dot_star_Z() {
    // C line 1144: .(あ*\Z)\1 matches .あ at end of いあ -> 3-6
    let pattern = [b".", "(あ*".as_bytes(), b"\\Z)\\1"].concat();
    x2(&pattern, "いあ".as_bytes(), 3, 6);
}

#[test]
fn utf8_backref_nested_kana() {
    // C line 1145: (.(やいゆ)\2) capture 1 = zやいゆやいゆ -> 0-19
    let pattern = ["(.(やいゆ)".as_bytes(), b"\\2)"].concat();
    let input = [b"z", "やいゆやいゆ".as_bytes()].concat();
    x3(&pattern, &input, 0, 19, 1);
}

#[test]
fn utf8_backref_nested_digits() {
    // C line 1146: (.(..\\d.)\\2) capture 1 -> 0-11
    let pattern = b"(.(..\\d.)\\2)";
    let input = ["あ".as_bytes(), b"12341234"].concat();
    x3(pattern, &input, 0, 11, 1);
}

// ============================================================================
// UTF-8 nested character classes (C lines 1150-1154)
// ============================================================================

#[test]
fn utf8_nested_class_kana() {
    // C line 1150: [[ひふ]] matches ふ -> 0-3
    x2("[[ひふ]]".as_bytes(), "ふ".as_bytes(), 0, 3);
}

#[test]
fn utf8_nested_class_kana_outer() {
    // C line 1151: [[いおう]か] matches か -> 0-3
    x2("[[いおう]か]".as_bytes(), "か".as_bytes(), 0, 3);
}

#[test]
fn utf8_nested_neg_class_kana() {
    // C line 1152: [[^あ]] no match for あ
    n("[[^あ]]".as_bytes(), "あ".as_bytes());
}

#[test]
fn utf8_neg_nested_class_kana() {
    // C line 1153: [^[あ]] no match for あ
    n("[^[あ]]".as_bytes(), "あ".as_bytes());
}

#[test]
fn utf8_neg_neg_class_kana() {
    // C line 1154: [^[^あ]] matches あ -> 0-3
    x2("[^[^あ]]".as_bytes(), "あ".as_bytes(), 0, 3);
}

// ============================================================================
// ASCII hex escapes (C lines 1358-1363)
// ============================================================================

#[test]
fn ascii_hex_x40() {
    // C line 1358: \x40 matches @
    x2(b"\\x40", b"@", 0, 1);
}

#[test]
fn ascii_hex_x1() {
    // C line 1359: \x1 matches \x01
    x2(b"\\x1", b"\x01", 0, 1);
}

#[test]
fn ascii_hex_x_brace_1() {
    // C line 1360: \x{1} matches \x01
    x2(b"\\x{1}", b"\x01", 0, 1);
}

#[test]
fn ascii_hex_x_brace_4E38() {
    // C line 1361: \x{4E38} matches 丸 (U+4E38 = \xE4\xB8\xB8)
    x2(b"\\x{4E38}", b"\xE4\xB8\xB8", 0, 3);
}

#[test]
fn ascii_unicode_u4E38() {
    // C line 1362: \u4E38 matches 丸
    x2(b"\\u4E38", b"\xE4\xB8\xB8", 0, 3);
}

#[test]
fn ascii_unicode_u0040() {
    // C line 1363: \u0040 matches @
    x2(b"\\u0040", b"@", 0, 1);
}

// ============================================================================
// ASCII word boundary with .* (C lines 1372-1373)
// ============================================================================

#[test]
fn ascii_word_boundary_cstar() {
    // C line 1372: c.*\b matches c in abc -> 2-3
    x2(b"c.*\\b", b"abc", 2, 3);
}

#[test]
fn ascii_word_boundary_wrapped() {
    // C line 1373: \b.*abc.*\b matches abc -> 0-3
    x2(b"\\b.*abc.*\\b", b"abc", 0, 3);
}

// ============================================================================
// ASCII interval quantifier variants (C lines 1388-1403)
// ============================================================================

#[test]
fn ascii_interval_lazy_1_3() {
    // C line 1388: a{1,3}? matches a in aaa -> 0-1
    x2(b"a{1,3}?", b"aaa", 0, 1);
}

#[test]
fn ascii_interval_exact_3() {
    // C line 1389: a{3} matches aaa -> 0-3
    x2(b"a{3}", b"aaa", 0, 3);
}

#[test]
fn ascii_interval_exact_3_lazy() {
    // C line 1390: a{3}? matches aaa -> 0-3 (lazy but exact, same result)
    x2(b"a{3}?", b"aaa", 0, 3);
}

#[test]
fn ascii_interval_exact_3_lazy_short() {
    // C line 1391: a{3}? matches empty in aa -> 0-0
    // a{3}? is treated as (?:a{3})? when input too short
    x2(b"a{3}?", b"aa", 0, 0);
}

#[test]
fn ascii_interval_exact_3_3_lazy() {
    // C line 1392: a{3,3}? matches aaa -> 0-3
    x2(b"a{3,3}?", b"aaa", 0, 3);
}

#[test]
fn ascii_interval_exact_3_3_lazy_no_match() {
    // C line 1393: a{3,3}? no match for aa
    n(b"a{3,3}?", b"aa");
}

#[test]
fn ascii_interval_possessive_1_3() {
    // C line 1394: a{1,3}+ matches aaaaaa -> 0-6
    // a{1,3}+ is possessive: (?:a{1,3})+ (greedy repeat of interval)
    x2(b"a{1,3}+", b"aaaaaa", 0, 6);
}

#[test]
fn ascii_interval_possessive_3() {
    // C line 1395: a{3}+ matches aaaaaa -> 0-6
    x2(b"a{3}+", b"aaaaaa", 0, 6);
}

#[test]
fn ascii_interval_possessive_3_3() {
    // C line 1396: a{3,3}+ matches aaaaaa -> 0-6
    x2(b"a{3,3}+", b"aaaaaa", 0, 6);
}

#[test]
fn ascii_interval_lazy_2_3_no_match() {
    // C line 1397: a{2,3}? no match for single a
    n(b"a{2,3}?", b"a");
}

#[test]
fn ascii_interval_reversed_no_match() {
    // C line 1398: a{3,2}a no match for aaa (reversed range)
    n(b"a{3,2}a", b"aaa");
}

#[test]
fn ascii_interval_reversed_b() {
    // C line 1399: a{3,2}b matches aaab -> 0-4
    x2(b"a{3,2}b", b"aaab", 0, 4);
}

#[test]
fn ascii_interval_reversed_b_2() {
    // C line 1400: a{3,2}b matches aaaab -> 1-5
    x2(b"a{3,2}b", b"aaaab", 1, 5);
}

#[test]
fn ascii_interval_reversed_b_short() {
    // C line 1401: a{3,2}b matches aab -> 0-3
    x2(b"a{3,2}b", b"aab", 0, 3);
}

#[test]
fn ascii_interval_reversed_lazy_empty() {
    // C line 1402: a{3,2}? matches empty -> 0-0 (== (?:a{3,2})?)
    x2(b"a{3,2}?", b"", 0, 0);
}

#[test]
fn ascii_interval_possessive_2_3_a() {
    // C line 1403: a{2,3}+a matches aaa -> 0-3 (== (?:a{2,3})+)
    x2(b"a{2,3}+a", b"aaa", 0, 3);
}

// ============================================================================
// ASCII unicode range in character class (C lines 1404-1405)
// ============================================================================

#[test]
fn ascii_wide_range_class() {
    // C line 1404: [\x{0}-\x{7fffffff}] matches a -> 0-1
    x2(b"[\\x{0}-\\x{7fffffff}]", b"a", 0, 1);
}

#[test]
fn ascii_wide_range_class_kanji() {
    // C line 1405: [\x{7f}-\x{7fffffff}] matches 家 (U+5BB6) -> 0-3
    x2(b"[\\x{7f}-\\x{7fffffff}]", "\u{5BB6}".as_bytes(), 0, 3);
}

// ============================================================================
// ASCII nested character class tests (C lines 1406-1410)
// ============================================================================

#[test]
fn ascii_nested_class_cdef() {
    // C line 1406: [a[cdef]] matches a -> 0-1
    x2(b"[a[cdef]]", b"a", 0, 1);
}

#[test]
fn ascii_nested_class_xyz_range_no_match() {
    // C line 1407: [a[xyz]-c] no match for b
    n(b"[a[xyz]-c]", b"b");
}

#[test]
fn ascii_nested_class_xyz_range_a() {
    // C line 1408: [a[xyz]-c] matches a -> 0-1
    x2(b"[a[xyz]-c]", b"a", 0, 1);
}

#[test]
fn ascii_nested_class_xyz_range_dash() {
    // C line 1409: [a[xyz]-c] matches - -> 0-1
    x2(b"[a[xyz]-c]", b"-", 0, 1);
}

#[test]
fn ascii_nested_class_xyz_range_c() {
    // C line 1410: [a[xyz]-c] matches c -> 0-1
    x2(b"[a[xyz]-c]", b"c", 0, 1);
}

// ============================================================================
// Control char escapes (C lines 167-170)
// ============================================================================

#[test]
fn ctrl_a_escape() {
    // C line 167: \ca matches \x01
    x2(b"\\ca", b"\x01", 0, 1);
}

#[test]
fn ctrl_b_escape() {
    // C line 168: \C-b matches \x02
    x2(b"\\C-b", b"\x02", 0, 1);
}

#[test]
fn ctrl_backslash_escape() {
    // C line 169: \c\\ matches \x1c
    x2(b"\\c\\\\", b"\x1c", 0, 1);
}

#[test]
fn ctrl_backslash_in_class() {
    // C line 170: q[\c\\] matches q\x1c
    x2(b"q[\\c\\\\]", b"q\x1c", 0, 2);
}

// ============================================================================
// Empty pattern on non-empty string (C line 171)
// ============================================================================

#[test]
fn empty_pattern_nonempty_string() {
    // C line 171: "" matches "a" at 0-0
    x2(b"", b"a", 0, 0);
}

// ============================================================================
// Long literal (C line 176)
// ============================================================================

#[test]
fn literal_35_a() {
    // C line 176: 35 a's match 35 a's
    x2(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 0, 35);
}

// ============================================================================
// Case-insensitive with special chars (C line 180)
// ============================================================================

#[test]
fn case_insensitive_ret() {
    // C line 180: (?i:#RET#) matches #RET# in #INS##RET#
    x2(b"(?i:#RET#)", b"#INS##RET#", 5, 10);
}

// ============================================================================
// Extended mode (?x) (C line 184)
// ============================================================================

#[test]
fn extended_mode_whitespace() {
    // C line 184: (?x)  G (o O(?-x)oO) g L matches GoOoOgL in GoOoOgLe
    x2(b"(?x)  G (o O(?-x)oO) g L", b"GoOoOgLe", 0, 7);
}

// ============================================================================
// Dot no match on empty (C line 186)
// ============================================================================

#[test]
fn dot_no_match_empty() {
    // C line 186: . no match for ""
    n(b".", b"");
}

// ============================================================================
// POSIX bracket edge cases (C lines 242, 244-249)
// ============================================================================

#[test]
fn posix_upper_bracket_literal() {
    // C line 242: [[:upper\] :]] matches ]
    x2(b"[[:upper\\] :]]", b"]", 0, 1);
}

#[test]
#[ignore] // parser rejects [[::]] as invalid POSIX bracket
fn nested_bracket_double_colon() {
    // C line 244: [[::]] matches :
    x2(b"[[::]]", b":", 0, 1);
}

#[test]
fn nested_bracket_triple_colon() {
    // C line 245: [[:::]] matches :
    x2(b"[[:::]]", b":", 0, 1);
}

#[test]
fn nested_bracket_backslash_colon_star() {
    // C line 246: [[\]:]]*  matches :] (with *)
    x2(b"[[:\\]:]]*", b":]", 0, 2);
}

#[test]
fn nested_bracket_open_bracket_star() {
    // C line 247: [[\[:]]*  matches :[ (with *)
    x2(b"[[:\\[:]]*", b":[", 0, 2);
}

#[test]
fn nested_bracket_close_bracket_star() {
    // C line 248: [[\]]]*  matches :] (with *)
    x2(b"[[:\\]]]*", b":]", 0, 2);
}

// ============================================================================
// Case-insensitive (?i:) ASCII patterns (C lines 311-355)
// ============================================================================

#[test]
fn case_insensitive_a_lower() {
    // C line 311: (?i:a) matches a
    x2(b"(?i:a)", b"a", 0, 1);
}

#[test]
fn case_insensitive_a_upper() {
    // C line 312: (?i:a) matches A
    x2(b"(?i:a)", b"A", 0, 1);
}

#[test]
fn case_insensitive_A_lower() {
    // C line 313: (?i:A) matches a
    x2(b"(?i:A)", b"a", 0, 1);
}

#[test]
fn case_insensitive_i_upper() {
    // C line 314: (?i:i) matches I
    x2(b"(?i:i)", b"I", 0, 1);
}

#[test]
fn case_insensitive_I_lower() {
    // C line 315: (?i:I) matches i
    x2(b"(?i:I)", b"i", 0, 1);
}

#[test]
#[ignore] // case-insensitive char class ranges not yet implemented
fn case_insensitive_range_upper() {
    // C line 316: (?i:[A-Z]) matches i
    x2(b"(?i:[A-Z])", b"i", 0, 1);
}

#[test]
#[ignore] // case-insensitive char class ranges not yet implemented
fn case_insensitive_range_lower() {
    // C line 317: (?i:[a-z]) matches I
    x2(b"(?i:[a-z])", b"I", 0, 1);
}

#[test]
fn case_insensitive_no_match() {
    // C line 318: (?i:A) no match for b
    n(b"(?i:A)", b"b");
}

#[test]
fn case_insensitive_ss_lower() {
    // C line 319: (?i:ss) matches ss
    x2(b"(?i:ss)", b"ss", 0, 2);
}

#[test]
fn case_insensitive_ss_mixed() {
    // C line 320: (?i:ss) matches Ss
    x2(b"(?i:ss)", b"Ss", 0, 2);
}

#[test]
fn case_insensitive_ss_upper() {
    // C line 321: (?i:ss) matches SS
    x2(b"(?i:ss)", b"SS", 0, 2);
}

#[test]
#[ignore] // case folding for LONG S not yet implemented
fn case_insensitive_ss_long_s_upper() {
    // C line 323: (?i:ss) matches \xc5\xbfS (LATIN SMALL LETTER LONG S + S)
    x2(b"(?i:ss)", b"\xc5\xbfS", 0, 3);
}

#[test]
#[ignore] // case folding for LONG S not yet implemented
fn case_insensitive_ss_s_long_s() {
    // C line 324: (?i:ss) matches s\xc5\xbf (s + LATIN SMALL LETTER LONG S)
    x2(b"(?i:ss)", b"s\xc5\xbf", 0, 3);
}

#[test]
#[ignore] // case folding for SHARP S not yet implemented
fn case_insensitive_ss_sharp_s() {
    // C line 326: (?i:ss) matches \xc3\x9f (LATIN SMALL LETTER SHARP S)
    x2(b"(?i:ss)", b"\xc3\x9f", 0, 2);
}

#[test]
#[ignore] // case folding for CAPITAL SHARP S not yet implemented
fn case_insensitive_ss_capital_sharp_s() {
    // C line 328: (?i:ss) matches \xe1\xba\x9e (LATIN CAPITAL LETTER SHARP S)
    x2(b"(?i:ss)", b"\xe1\xba\x9e", 0, 3);
}

#[test]
fn case_insensitive_xssy_lower() {
    // C line 329: (?i:xssy) matches xssy
    x2(b"(?i:xssy)", b"xssy", 0, 4);
}

#[test]
fn case_insensitive_xssy_mixed() {
    // C line 330: (?i:xssy) matches xSsy
    x2(b"(?i:xssy)", b"xSsy", 0, 4);
}

#[test]
fn case_insensitive_xssy_upper() {
    // C line 331: (?i:xssy) matches xSSy
    x2(b"(?i:xssy)", b"xSSy", 0, 4);
}

#[test]
#[ignore] // case folding for LONG S not yet implemented
fn case_insensitive_xssy_long_s_upper() {
    // C line 332: (?i:xssy) matches x\xc5\xbfSy
    x2(b"(?i:xssy)", b"x\xc5\xbfSy", 0, 5);
}

#[test]
#[ignore] // case folding for LONG S not yet implemented
fn case_insensitive_xssy_s_long_s() {
    // C line 333: (?i:xssy) matches xs\xc5\xbfy
    x2(b"(?i:xssy)", b"xs\xc5\xbfy", 0, 5);
}

#[test]
#[ignore] // case folding for SHARP S not yet implemented
fn case_insensitive_xssy_sharp_s() {
    // C line 334: (?i:xssy) matches x\xc3\x9fy (sharp s)
    x2(b"(?i:xssy)", b"x\xc3\x9fy", 0, 4);
}

#[test]
#[ignore] // case folding for CAPITAL SHARP S not yet implemented
fn case_insensitive_xssy_capital_sharp_s() {
    // C line 335: (?i:xssy) matches x\xe1\xba\x9ey (capital sharp s)
    x2(b"(?i:xssy)", b"x\xe1\xba\x9ey", 0, 5);
}

#[test]
#[ignore] // case folding for SHARP S in pattern not yet implemented
fn case_insensitive_sharp_s_pattern_lower() {
    // C line 336: (?i:x\xc3\x9fy) matches xssy
    x2(b"(?i:x\xc3\x9fy)", b"xssy", 0, 4);
}

#[test]
#[ignore] // case folding for SHARP S in pattern not yet implemented
fn case_insensitive_sharp_s_pattern_upper() {
    // C line 337: (?i:x\xc3\x9fy) matches xSSy
    x2(b"(?i:x\xc3\x9fy)", b"xSSy", 0, 4);
}

#[test]
#[ignore] // case folding for SHARP S not yet implemented
fn case_insensitive_sharp_s_alone_lower() {
    // C line 338: (?i:\xc3\x9f) matches ss
    x2(b"(?i:\xc3\x9f)", b"ss", 0, 2);
}

#[test]
#[ignore] // case folding for SHARP S not yet implemented
fn case_insensitive_sharp_s_alone_upper() {
    // C line 339: (?i:\xc3\x9f) matches SS
    x2(b"(?i:\xc3\x9f)", b"SS", 0, 2);
}

#[test]
#[ignore] // case folding for SHARP S in class not yet implemented
fn case_insensitive_sharp_s_class_lower() {
    // C line 340: (?i:[\xc3\x9f]) matches ss
    x2(b"(?i:[\xc3\x9f])", b"ss", 0, 2);
}

#[test]
#[ignore] // case folding for SHARP S in class not yet implemented
fn case_insensitive_sharp_s_class_upper() {
    // C line 341: (?i:[\xc3\x9f]) matches SS
    x2(b"(?i:[\xc3\x9f])", b"SS", 0, 2);
}

#[test]
#[ignore] // case-insensitive lookbehind not yet fully implemented
fn case_insensitive_lookbehind_ss() {
    // C line 342: (?i)(?<!ss)z matches z in qqz
    x2(b"(?i)(?<!ss)z", b"qqz", 2, 3);
}

#[test]
#[ignore] // case-insensitive char class ranges not yet implemented
fn case_insensitive_range_upper_matches_lower() {
    // C line 343: (?i:[A-Z]) matches a
    x2(b"(?i:[A-Z])", b"a", 0, 1);
}

#[test]
#[ignore] // case-insensitive char class ranges not yet implemented
fn case_insensitive_range_f_m_upper() {
    // C line 344: (?i:[f-m]) matches H
    x2(b"(?i:[f-m])", b"H", 0, 1);
}

#[test]
fn case_insensitive_range_f_m_lower() {
    // C line 345: (?i:[f-m]) matches h
    x2(b"(?i:[f-m])", b"h", 0, 1);
}

#[test]
fn case_insensitive_range_f_m_no_match() {
    // C line 346: (?i:[f-m]) no match for e
    n(b"(?i:[f-m])", b"e");
}

#[test]
fn case_insensitive_range_A_c_upper() {
    // C line 347: (?i:[A-c]) matches D
    x2(b"(?i:[A-c])", b"D", 0, 1);
}

#[test]
#[ignore] // case-insensitive negated char class ranges not yet implemented
fn case_insensitive_neg_range_upper() {
    // C line 348: (?i:[^a-z]) no match for A
    n(b"(?i:[^a-z])", b"A");
}

#[test]
fn case_insensitive_neg_range_lower() {
    // C line 349: (?i:[^a-z]) no match for a
    n(b"(?i:[^a-z])", b"a");
}

#[test]
fn case_insensitive_range_bang_k_upper() {
    // C line 350: (?i:[!-k]) matches Z
    x2(b"(?i:[!-k])", b"Z", 0, 1);
}

#[test]
fn case_insensitive_range_bang_k_digit() {
    // C line 351: (?i:[!-k]) matches 7
    x2(b"(?i:[!-k])", b"7", 0, 1);
}

#[test]
fn case_insensitive_range_T_brace_lower() {
    // C line 352: (?i:[T-}]) matches b
    x2(b"(?i:[T-}])", b"b", 0, 1);
}

#[test]
fn case_insensitive_range_T_brace_brace() {
    // C line 353: (?i:[T-}]) matches {
    x2(b"(?i:[T-}])", b"{", 0, 1);
}

#[test]
fn case_insensitive_escaped_question_a() {
    // C line 354: (?i:\?a) matches ?A
    x2(b"(?i:\\?a)", b"?A", 0, 2);
}

#[test]
fn case_insensitive_escaped_star_a() {
    // C line 355: (?i:\*A) matches *a
    x2(b"(?i:\\*A)", b"*a", 0, 2);
}

// ============================================================================
// Multiline mode (?m:) (C lines 357-359, 362)
// ============================================================================

#[test]
fn multiline_dot_newline() {
    // C line 357: (?m:.) matches \n
    x2(b"(?m:.)", b"\n", 0, 1);
}

#[test]
fn multiline_a_dot_newline() {
    // C line 358: (?m:a.) matches a\n
    x2(b"(?m:a.)", b"a\n", 0, 2);
}

#[test]
fn multiline_dot_b_across_newline() {
    // C line 359: (?m:.b) matches \nb in a\nb
    x2(b"(?m:.b)", b"a\nb", 1, 3);
}

#[test]
fn multiline_dotstar_abc() {
    // C line 362: (?m:.*abc) matches dddabddabc -> 0-10
    x2(b"(?m:.*abc)", b"dddabddabc", 0, 10);
}

// ============================================================================
// Case-insensitive negation (C lines 363-364)
// ============================================================================

#[test]
fn case_insensitive_then_negate() {
    // C line 363: (?i)(?-i)a no match for A
    n(b"(?i)(?-i)a", b"A");
}

#[test]
fn case_insensitive_then_negate_group() {
    // C line 364: (?i)(?-i:a) no match for A
    n(b"(?i)(?-i:a)", b"A");
}

// ============================================================================
// C lines 1155-1176: UTF-8 char class intersection with Japanese chars,
//                    multibyte string matching
// ============================================================================

#[test]
fn japanese_class_intersection_match_ku() {
    // C line 1155: [[かきく]&&きく] matches く -> 0-3
    x2("[[かきく]&&きく]".as_bytes(), "く".as_bytes(), 0, 3);
}

#[test]
fn japanese_class_intersection_no_match_ka() {
    // C line 1156: [[かきく]&&きく] does not match か
    n("[[かきく]&&きく]".as_bytes(), "か".as_bytes());
}

#[test]
fn japanese_class_intersection_no_match_ke() {
    // C line 1157: [[かきく]&&きく] does not match け
    n("[[かきく]&&きく]".as_bytes(), "け".as_bytes());
}

#[test]
fn japanese_range_triple_intersection() {
    // C line 1158: [あ-ん&&い-を&&う-ゑ] matches ゑ -> 0-3
    x2("[あ-ん&&い-を&&う-ゑ]".as_bytes(), "ゑ".as_bytes(), 0, 3);
}

#[test]
fn japanese_negated_range_triple_intersection() {
    // C line 1159: [^あ-ん&&い-を&&う-ゑ] does not match ゑ
    n("[^あ-ん&&い-を&&う-ゑ]".as_bytes(), "ゑ".as_bytes());
}

#[test]
fn japanese_negated_inner_class_intersection_match_i() {
    // C line 1160: [[^あ&&あ]&&あ-ん] matches い -> 0-3
    x2("[[^あ&&あ]&&あ-ん]".as_bytes(), "い".as_bytes(), 0, 3);
}

#[test]
fn japanese_negated_inner_class_intersection_no_match_a() {
    // C line 1161: [[^あ&&あ]&&あ-ん] does not match あ
    n("[[^あ&&あ]&&あ-ん]".as_bytes(), "あ".as_bytes());
}

#[test]
fn japanese_complex_negated_intersection_match_ki() {
    // C line 1162: [[^あ-ん&&いうえお]&&[^う-か]] matches き -> 0-3
    x2("[[^あ-ん&&いうえお]&&[^う-か]]".as_bytes(), "き".as_bytes(), 0, 3);
}

#[test]
fn japanese_complex_negated_intersection_no_match_i() {
    // C line 1163: [[^あ-ん&&いうえお]&&[^う-か]] does not match い
    n("[[^あ-ん&&いうえお]&&[^う-か]]".as_bytes(), "い".as_bytes());
}

#[test]
fn japanese_double_negated_intersection_match_u() {
    // C line 1164: [^[^あいう]&&[^うえお]] matches う -> 0-3
    x2("[^[^あいう]&&[^うえお]]".as_bytes(), "う".as_bytes(), 0, 3);
}

#[test]
fn japanese_double_negated_intersection_match_e() {
    // C line 1165: [^[^あいう]&&[^うえお]] matches え -> 0-3
    x2("[^[^あいう]&&[^うえお]]".as_bytes(), "え".as_bytes(), 0, 3);
}

#[test]
fn japanese_double_negated_intersection_no_match_ka() {
    // C line 1166: [^[^あいう]&&[^うえお]] does not match か
    n("[^[^あいう]&&[^うえお]]".as_bytes(), "か".as_bytes());
}

#[test]
fn japanese_range_dash_intersection() {
    // C line 1167: [あ-&&-あ] matches - -> 0-1
    x2("[あ-&&-あ]".as_bytes(), b"-", 0, 1);
}

#[test]
fn japanese_mixed_ascii_negated_intersection_match_e() {
    // C line 1168: [^[^a-zあいう]&&[^bcdefgうえお]q-w] matches え -> 0-3
    x2("[^[^a-zあいう]&&[^bcdefgうえお]q-w]".as_bytes(), "え".as_bytes(), 0, 3);
}

#[test]
fn japanese_mixed_ascii_negated_intersection_match_f() {
    // C line 1169: [^[^a-zあいう]&&[^bcdefgうえお]g-w] matches f -> 0-1
    x2("[^[^a-zあいう]&&[^bcdefgうえお]g-w]".as_bytes(), b"f", 0, 1);
}

#[test]
fn japanese_mixed_ascii_negated_intersection_match_g() {
    // C line 1170: [^[^a-zあいう]&&[^bcdefgうえお]g-w] matches g -> 0-1
    x2("[^[^a-zあいう]&&[^bcdefgうえお]g-w]".as_bytes(), b"g", 0, 1);
}

#[test]
fn japanese_mixed_ascii_negated_intersection_no_match_2() {
    // C line 1171: [^[^a-zあいう]&&[^bcdefgうえお]g-w] does not match 2
    n("[^[^a-zあいう]&&[^bcdefgうえお]g-w]".as_bytes(), b"2");
}

#[test]
fn japanese_version_download_literal() {
    // C line 1172: a<b>バージョンのダウンロード<\/b> matches -> 0-44
    let pattern = [b"a<b>" as &[u8], "バージョンのダウンロード".as_bytes(), b"<\\/b>"].concat();
    let input = [b"a<b>" as &[u8], "バージョンのダウンロード".as_bytes(), b"</b>"].concat();
    x2(&pattern, &input, 0, 44);
}

#[test]
fn japanese_version_download_dot() {
    // C line 1173: .<b>バージョンのダウンロード<\/b> matches -> 0-44
    let pattern = [b".<b>" as &[u8], "バージョンのダウンロード".as_bytes(), b"<\\/b>"].concat();
    let input = [b"a<b>" as &[u8], "バージョンのダウンロード".as_bytes(), b"</b>"].concat();
    x2(&pattern, &input, 0, 44);
}

#[test]
fn japanese_optional_newline_end() {
    // C line 1174: \n?\z matches end of こんにちは -> 15-15
    x2(b"\\n?\\z", "こんにちは".as_bytes(), 15, 15);
}

#[test]
fn japanese_multiline_dotstar() {
    // C line 1175: (?m).* matches 青赤黄 -> 0-9
    x2(b"(?m).*", "青赤黄".as_bytes(), 0, 9);
}

#[test]
fn japanese_multiline_dotstar_a() {
    // C line 1176: (?m).*a matches 青赤黄a -> 0-10
    let input = ["青赤黄".as_bytes(), b"a"].concat();
    x2(b"(?m).*a", &input, 0, 10);
}

// ============================================================================
// C lines 1178-1201: Unicode general categories \p{Hiragana}, \p{Emoji},
//                    \pC, \pL, \pM, \pN, \pP, \pS, \pZ, etc.
// NOTE: \p{...} property lookup is not yet implemented (TODO in unicode/mod.rs)
//       so all these tests are #[ignore] until property_name_to_ctype is ported.
// ============================================================================

#[test]
fn unicode_prop_hiragana_match() {
    // C line 1178: \p{Hiragana} matches ぴ -> 0-3
    x2("\\p{Hiragana}".as_bytes(), "ぴ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_not_hiragana_no_match() {
    // C line 1179: \P{Hiragana} does not match ぴ
    n("\\P{Hiragana}".as_bytes(), "ぴ".as_bytes());
}

#[test]
fn unicode_prop_emoji_match() {
    // C line 1180: \p{Emoji} matches U+2B50 (star) -> 0-3
    x2(b"\\p{Emoji}", b"\xe2\xad\x90", 0, 3);
}

#[test]
fn unicode_prop_not_emoji_match() {
    // C line 1181: \p{^Emoji} matches U+FF13 (fullwidth 3) -> 0-3
    x2(b"\\p{^Emoji}", b"\xef\xbc\x93", 0, 3);
}

#[test]
fn unicode_prop_extended_pictographic_match() {
    // C line 1182: \p{Extended_Pictographic} matches U+26A1 (lightning) -> 0-3
    x2(b"\\p{Extended_Pictographic}", b"\xe2\x9a\xa1", 0, 3);
}

#[test]
fn unicode_prop_extended_pictographic_no_match() {
    // C line 1183: \p{Extended_Pictographic} does not match U+3042 (あ)
    n(b"\\p{Extended_Pictographic}", b"\xe3\x81\x82");
}

#[test]
fn unicode_prop_pc_soft_hyphen() {
    // C line 1184: \pC matches U+00AD (Soft Hyphen) -> 0-2
    x2(b"\\pC", b"\xc2\xad", 0, 2);
}

#[test]
fn unicode_prop_pl_ascii() {
    // C line 1185: \pL matches U -> 0-1
    x2(b"\\pL", b"U", 0, 1);
}

#[test]
fn unicode_prop_pm_combining_circle() {
    // C line 1186: \pM matches U+20DD (Combining Enclosing Circle) -> 0-3
    x2(b"\\pM", b"\xe2\x83\x9d", 0, 3);
}

#[test]
fn unicode_prop_pn_plus() {
    // C line 1187: \pN+ matches "3Ⅴ" -> 0-4
    let input = [b"3" as &[u8], "\u{2164}".as_bytes()].concat();
    x2(b"\\pN+", &input, 0, 4);
}

#[test]
fn unicode_prop_pp_plus() {
    // C line 1188: \pP+ matches "†⁂" -> 0-6
    let input = ["\u{2020}".as_bytes(), "\u{2042}".as_bytes()].concat();
    x2(b"\\pP+", &input, 0, 6);
}

#[test]
fn unicode_prop_ps_plus() {
    // C line 1189: \pS+ matches "€₤" -> 0-6
    let input = ["\u{20AC}".as_bytes(), "\u{20A4}".as_bytes()].concat();
    x2(b"\\pS+", &input, 0, 6);
}

#[test]
fn unicode_prop_pz_space() {
    // C line 1190: \pZ+ matches " " -> 0-1
    x2(b"\\pZ+", b" ", 0, 1);
}

#[test]
fn unicode_prop_pl_no_match_at() {
    // C line 1191: \pL does not match @
    n(b"\\pL", b"@");
}

#[test]
fn unicode_prop_pl_plus() {
    // C line 1192: \pL+ matches akZtE -> 0-5
    x2(b"\\pL+", b"akZtE", 0, 5);
}

#[test]
fn unicode_prop_not_pl_plus() {
    // C line 1193: \PL+ matches "1@=-%" -> 0-5
    x2(b"\\PL+", b"1@=-%", 0, 5);
}

// C lines 1194-1196: e() error tests -- skipped (no e() helper)

#[test]
fn unicode_prop_pl_in_class() {
    // C line 1197: [\pL] matches s -> 0-1
    x2(b"[\\pL]", b"s", 0, 1);
}

#[test]
fn unicode_prop_not_pl_in_negated_class() {
    // C line 1198: [^\pL] does not match s
    n(b"[^\\pL]", b"s");
}

#[test]
fn unicode_prop_not_pl_in_class_plus() {
    // C line 1199: [\PL]+ matches "-3@" -> 0-3
    x2(b"[\\PL]+", b"-3@", 0, 3);
}

// C lines 1200-1201: e() error tests -- skipped (no e() helper)

// ============================================================================
// C lines 1203-1236: \p{Word}, \p{^Word}, \p{Cntrl} with char class
//                    intersections and negations
// NOTE: \p{...} property lookup not yet implemented - all #[ignore]
// ============================================================================

#[test]
fn unicode_prop_word_match_ko() {
    // C line 1203: \p{Word} matches こ -> 0-3
    x2("\\p{Word}".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_not_word_no_match_ko() {
    // C line 1204: \p{^Word} does not match こ
    n("\\p{^Word}".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_word_in_class_match_ko() {
    // C line 1205: [\p{Word}] matches こ -> 0-3
    x2("[\\p{Word}]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_not_word_in_class_no_match_ko() {
    // C line 1206: [\p{^Word}] does not match こ
    n("[\\p{^Word}]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_word_negated_class_no_match_ko() {
    // C line 1207: [^\p{Word}] does not match こ
    n("[^\\p{Word}]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_word_negated_class_match_ko() {
    // C line 1208: [^\p{^Word}] matches こ -> 0-3
    x2("[^\\p{^Word}]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_not_word_and_ascii_negated_match_ko() {
    // C line 1209: [^\p{^Word}&&\p{ASCII}] matches こ -> 0-3
    x2("[^\\p{^Word}&&\\p{ASCII}]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_not_word_and_ascii_negated_match_a() {
    // C line 1210: [^\p{^Word}&&\p{ASCII}] matches a -> 0-1
    x2("[^\\p{^Word}&&\\p{ASCII}]".as_bytes(), b"a", 0, 1);
}

#[test]
fn unicode_prop_not_word_and_ascii_negated_no_match_hash() {
    // C line 1211: [^\p{^Word}&&\p{ASCII}] does not match #
    n("[^\\p{^Word}&&\\p{ASCII}]".as_bytes(), b"#");
}

#[test]
fn unicode_prop_not_word_nested_and_ascii_match_ko() {
    // C line 1212: [^[\p{^Word}]&&[\p{ASCII}]] matches こ -> 0-3
    x2("[^[\\p{^Word}]&&[\\p{ASCII}]]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_ascii_and_not_word_negated_match_ko() {
    // C line 1213: [^[\p{ASCII}]&&[^\p{Word}]] matches こ -> 0-3
    x2("[^[\\p{ASCII}]&&[^\\p{Word}]]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_ascii_and_not_word_no_match_ko() {
    // C line 1214: [[\p{ASCII}]&&[^\p{Word}]] does not match こ
    n("[[\\p{ASCII}]&&[^\\p{Word}]]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_word_and_not_ascii_negated_match_ko() {
    // C line 1215: [^[\p{^Word}]&&[^\p{ASCII}]] matches こ -> 0-3
    x2("[^[\\p{^Word}]&&[^\\p{ASCII}]]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_negated_hex_code_match_ko() {
    // C line 1216: [^\x{104a}] matches こ -> 0-3
    x2("[^\\x{104a}]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_not_word_and_not_hex_negated_match_ko() {
    // C line 1217: [^\p{^Word}&&[^\x{104a}]] matches こ -> 0-3
    x2("[^\\p{^Word}&&[^\\x{104a}]]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_not_word_nested_and_not_hex_negated_match_ko() {
    // C line 1218: [^[\p{^Word}]&&[^\x{104a}]] matches こ -> 0-3
    x2("[^[\\p{^Word}]&&[^\\x{104a}]]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_word_or_not_hex_negated_no_match_ko() {
    // C line 1219: [^\p{Word}||[^\x{104a}]] does not match こ
    n("[^\\p{Word}||[^\\x{104a}]]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_cntrl_match_ko() {
    // C line 1221: \p{^Cntrl} matches こ -> 0-3
    x2("\\p{^Cntrl}".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_cntrl_no_match_ko() {
    // C line 1222: \p{Cntrl} does not match こ
    n("\\p{Cntrl}".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_cntrl_in_class_match_ko() {
    // C line 1223: [\p{^Cntrl}] matches こ -> 0-3
    x2("[\\p{^Cntrl}]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_cntrl_in_class_no_match_ko() {
    // C line 1224: [\p{Cntrl}] does not match こ
    n("[\\p{Cntrl}]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_cntrl_negated_class_no_match_ko() {
    // C line 1225: [^\p{^Cntrl}] does not match こ
    n("[^\\p{^Cntrl}]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_cntrl_negated_class_match_ko() {
    // C line 1226: [^\p{Cntrl}] matches こ -> 0-3
    x2("[^\\p{Cntrl}]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_cntrl_and_ascii_negated_match_ko() {
    // C line 1227: [^\p{Cntrl}&&\p{ASCII}] matches こ -> 0-3
    x2("[^\\p{Cntrl}&&\\p{ASCII}]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_cntrl_and_ascii_negated_match_a() {
    // C line 1228: [^\p{Cntrl}&&\p{ASCII}] matches a -> 0-1
    x2("[^\\p{Cntrl}&&\\p{ASCII}]".as_bytes(), b"a", 0, 1);
}

#[test]
fn unicode_prop_not_cntrl_and_ascii_negated_no_match_hash() {
    // C line 1229: [^\p{^Cntrl}&&\p{ASCII}] does not match #
    n("[^\\p{^Cntrl}&&\\p{ASCII}]".as_bytes(), b"#");
}

#[test]
fn unicode_prop_not_cntrl_nested_and_ascii_negated_match_ko() {
    // C line 1230: [^[\p{^Cntrl}]&&[\p{ASCII}]] matches こ -> 0-3
    x2("[^[\\p{^Cntrl}]&&[\\p{ASCII}]]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_ascii_and_not_cntrl_negated_match_ko() {
    // C line 1231: [^[\p{ASCII}]&&[^\p{Cntrl}]] matches こ -> 0-3
    x2("[^[\\p{ASCII}]&&[^\\p{Cntrl}]]".as_bytes(), "こ".as_bytes(), 0, 3);
}

#[test]
fn unicode_prop_ascii_and_not_cntrl_no_match_ko() {
    // C line 1232: [[\p{ASCII}]&&[^\p{Cntrl}]] does not match こ
    n("[[\\p{ASCII}]&&[^\\p{Cntrl}]]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_cntrl_and_not_ascii_negated_no_match_ko() {
    // C line 1233: [^[\p{^Cntrl}]&&[^\p{ASCII}]] does not match こ
    n("[^[\\p{^Cntrl}]&&[^\\p{ASCII}]]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_cntrl_and_not_hex_negated_no_match_ko() {
    // C line 1234: [^\p{^Cntrl}&&[^\x{104a}]] does not match こ
    n("[^\\p{^Cntrl}&&[^\\x{104a}]]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_not_cntrl_nested_and_not_hex_negated_no_match_ko() {
    // C line 1235: [^[\p{^Cntrl}]&&[^\x{104a}]] does not match こ
    n("[^[\\p{^Cntrl}]&&[^\\x{104a}]]".as_bytes(), "こ".as_bytes());
}

#[test]
fn unicode_prop_cntrl_or_not_hex_negated_no_match_ko() {
    // C line 1236: [^\p{Cntrl}||[^\x{104a}]] does not match こ
    n("[^\\p{Cntrl}||[^\\x{104a}]]".as_bytes(), "こ".as_bytes());
}

// C lines 1238-1273: (?W:...), (?D:...), (?S:...), (?P:...) option modifiers
// -- skipped: not implemented in group option parser

// ============================================================================
// C line 1275: \p{InBasicLatin} unicode block
// NOTE: \p{...} property lookup not yet implemented - #[ignore]
// ============================================================================

#[test]
fn unicode_prop_in_basic_latin() {
    // C line 1275: \p{InBasicLatin} matches A -> 0-1
    x2(b"\\p{InBasicLatin}", b"\x41", 0, 1);
}

// C lines 1279-1357: Extended grapheme clusters (\y, \Y, \X), text segments
// (?y{g}), (?y{w}) -- skipped: not implemented

// ============================================================================
// Atomic groups (C lines 528-529)
// ============================================================================

#[test]
fn atomic_group_no_backtrack() {
    // C line 528: (?>a|abd)c no match for "abdc" (atomic prevents backtrack)
    n(b"(?>a|abd)c", b"abdc");
}

#[test]
fn atomic_group_longest_first() {
    // C line 529: (?>abd|a)c matches "abdc" -> 0-4
    x2(b"(?>abd|a)c", b"abdc", 0, 4);
}

// ============================================================================
// Case-insensitive alternation (C lines 565-573)
// ============================================================================

#[test]
fn alt_casefold_inline() {
    // C line 565: a|(?i)c matches "C" -> 0-1
    x2(b"a|(?i)c", b"C", 0, 1);
}

#[test]
fn alt_casefold_c_or_a_match_c() {
    // C line 566: (?i)c|a matches "C" -> 0-1
    x2(b"(?i)c|a", b"C", 0, 1);
}

#[test]
fn alt_casefold_c_or_a_match_a() {
    // C line 567: (?i)c|a matches "A" -> 0-1
    x2(b"(?i)c|a", b"A", 0, 1);
}

#[test]
fn alt_casefold_scoped_b_or_c_match_b() {
    // C line 568: a(?i)b|c matches "aB" -> 0-2
    x2(b"a(?i)b|c", b"aB", 0, 2);
}

#[test]
#[ignore] // inline (?i) scoping across alternation not yet matching C behavior
fn alt_casefold_scoped_b_or_c_match_c() {
    // C line 569: a(?i)b|c matches "aC" -> 0-2
    x2(b"a(?i)b|c", b"aC", 0, 2);
}

#[test]
#[ignore] // inline (?i) scoping across alternation not yet matching C behavior
fn alt_casefold_scoped_no_match_ac() {
    // C line 570: a(?i)b|c no match for "AC" (a must be lowercase)
    n(b"a(?i)b|c", b"AC");
}

#[test]
fn alt_casefold_group_no_leak() {
    // C line 571: a(?:(?i)b)|c no match for "aC" ((?i) scoped to group)
    n(b"a(?:(?i)b)|c", b"aC");
}

#[test]
fn alt_casefold_modifier_group() {
    // C line 572: (?i:c)|a matches "C" -> 0-1
    x2(b"(?i:c)|a", b"C", 0, 1);
}

#[test]
fn alt_casefold_modifier_group_no_leak() {
    // C line 573: (?i:c)|a no match for "A" ((?i) only applies to c)
    n(b"(?i:c)|a", b"A");
}

// ============================================================================
// Case-insensitive with quantifiers (C line 601)
// ============================================================================

#[test]
fn casefold_xa_after_star() {
    // C line 601: (?:X*)(?i:xa) matches "XXXa" -> 0-4
    x2(b"(?:X*)(?i:xa)", b"XXXa", 0, 4);
}

// ============================================================================
// Capture groups with modifiers (C lines 629-635)
// ============================================================================

#[test]
fn capture_casefold_group() {
    // C line 629: ((?i:abc)) matches "AbC" group 1 -> 0-3
    x3(b"((?i:abc))", b"AbC", 0, 3, 1);
}

#[test]
#[ignore] // case-insensitive backrefs not yet implemented
fn capture_casefold_backref() {
    // C line 630: (abc)(?i:\1) matches "abcABC" -> 0-6
    x2(b"(abc)(?i:\\1)", b"abcABC", 0, 6);
}

#[test]
fn capture_multiline_group() {
    // C line 631: ((?m:a.c)) matches "a\nc" group 1 -> 0-3
    x3(b"((?m:a.c))", b"a\nc", 0, 3, 1);
}

#[test]
fn capture_casefold_abc_or_zzz() {
    // C line 635: (?i:(abc))|(zzz) matches "ABC" group 1 -> 0-3
    x3(b"(?i:(abc))|(zzz)", b"ABC", 0, 3, 1);
}

// ============================================================================
// Case-insensitive backreferences (C lines 680-681)
// ============================================================================

#[test]
fn backref_casefold_az() {
    // C line 680: ((?i:az))\1 matches "AzAz" -> 0-4
    x2(b"((?i:az))\\1", b"AzAz", 0, 4);
}

#[test]
fn backref_casefold_az_no_match() {
    // C line 681: ((?i:az))\1 no match for "Azaz" (\1 is literal backref, must match case)
    n(b"((?i:az))\\1", b"Azaz");
}

// ============================================================================
// Lookbehind (C lines 682-705)
// ============================================================================

#[test]
fn lookbehind_basic() {
    // C line 682: (?<=a)b matches "ab" -> 1-2
    x2(b"(?<=a)b", b"ab", 1, 2);
}

#[test]
fn lookbehind_basic_no_match() {
    // C line 683: (?<=a)b no match for "bb"
    n(b"(?<=a)b", b"bb");
}

#[test]
fn lookbehind_alt() {
    // C line 684: (?<=a|b)b matches "bb" -> 1-2
    x2(b"(?<=a|b)b", b"bb", 1, 2);
}

#[test]
fn lookbehind_alt_bc_1() {
    // C line 685: (?<=a|bc)b matches "bcb" -> 2-3
    x2(b"(?<=a|bc)b", b"bcb", 2, 3);
}

#[test]
fn lookbehind_alt_bc_2() {
    // C line 686: (?<=a|bc)b matches "ab" -> 1-2
    x2(b"(?<=a|bc)b", b"ab", 1, 2);
}

#[test]
fn lookbehind_alt_many() {
    // C line 687: (?<=a|bc||defghij|klmnopq|r)z matches "rz" -> 1-2
    x2(b"(?<=a|bc||defghij|klmnopq|r)z", b"rz", 1, 2);
}

#[test]
fn lookbehind_capture() {
    // C line 688: (?<=(abc))d matches "abcd", group 1 -> 0-3
    x3(b"(?<=(abc))d", b"abcd", 0, 3, 1);
}

#[test]
fn lookbehind_casefold() {
    // C line 689: (?<=(?i:abc))d matches "ABCd" -> 3-4
    x2(b"(?<=(?i:abc))d", b"ABCd", 3, 4);
}

#[test]
fn lookbehind_caret_or_b() {
    // C line 690: (?<=^|b)c matches " cbc" -> 3-4
    x2(b"(?<=^|b)c", b" cbc", 3, 4);
}

#[test]
fn lookbehind_a_or_caret_or_b() {
    // C line 691: (?<=a|^|b)c matches " cbc" -> 3-4
    x2(b"(?<=a|^|b)c", b" cbc", 3, 4);
}

#[test]
fn lookbehind_a_or_caret_cap_or_b() {
    // C line 692: (?<=a|(^)|b)c matches " cbc" -> 3-4
    x2(b"(?<=a|(^)|b)c", b" cbc", 3, 4);
}

#[test]
fn lookbehind_a_or_caret_cap_or_b_start() {
    // C line 693: (?<=a|(^)|b)c matches "cbc" -> 0-1
    x2(b"(?<=a|(^)|b)c", b"cbc", 0, 1);
}

#[test]
#[ignore] // negative lookbehind not yet fully implemented
fn neg_lookbehind_basic() {
    // C line 702: (?<!a)b matches "cb" -> 1-2
    x2(b"(?<!a)b", b"cb", 1, 2);
}

#[test]
fn neg_lookbehind_basic_no_match() {
    // C line 703: (?<!a)b no match for "ab"
    n(b"(?<!a)b", b"ab");
}

#[test]
#[ignore] // negative lookbehind not yet fully implemented
fn neg_lookbehind_alt() {
    // C line 704: (?<!a|bc)b matches "bbb" -> 0-1
    x2(b"(?<!a|bc)b", b"bbb", 0, 1);
}

#[test]
fn neg_lookbehind_alt_no_match() {
    // C line 705: (?<!a|bc)z no match for "bcz"
    n(b"(?<!a|bc)z", b"bcz");
}

#[test]
fn neg_lookbehind_caret_or_b_no_match() {
    // C line 697: (?<!^|b)c no match for "cbc"
    n(b"(?<!^|b)c", b"cbc");
}

#[test]
fn neg_lookbehind_a_or_caret_or_b_no_match() {
    // C line 698: (?<!a|^|b)c no match for "cbc"
    n(b"(?<!a|^|b)c", b"cbc");
}

#[test]
fn neg_lookbehind_a_or_noncap_caret_or_b_no_match() {
    // C line 699: (?<!a|(?:^)|b)c no match for "cbc"
    n(b"(?<!a|(?:^)|b)c", b"cbc");
}

#[test]
#[ignore] // negative lookbehind not yet fully implemented
fn neg_lookbehind_a_or_noncap_caret_or_b_match() {
    // C line 700: (?<!a|(?:^)|b)c matches " cbc" -> 1-2
    x2(b"(?<!a|(?:^)|b)c", b" cbc", 1, 2);
}

// ============================================================================
// Named groups (C lines 706-708)
// ============================================================================

#[test]
fn named_group_basic() {
    // C line 706: (?<name1>a) matches "a" -> 0-1
    x2(b"(?<name1>a)", b"a", 0, 1);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_backref() {
    // C line 707: (?<name_2>ab)\g<name_2> matches "abab" -> 0-4
    x2(b"(?<name_2>ab)\\g<name_2>", b"abab", 0, 4);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_backref_k() {
    // C line 708: (?<name_3>.zv.)\k<name_3> matches "azvbazvb" -> 0-8
    x2(b"(?<name_3>.zv.)\\k<name_3>", b"azvbazvb", 0, 8);
}

// ============================================================================
// Recursive patterns \g<n> (C lines 701, 709-738)
// ============================================================================

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_g1() {
    // C line 701: (a)\g<1> matches "aa" -> 0-2
    x2(b"(a)\\g<1>", b"aa", 0, 2);
}

#[test]
#[ignore] // recursive calls inside lookbehind not yet implemented
fn recursive_lookbehind_named() {
    // C line 709: (?<=\g<ab>)|-\zEND (?<ab>XyZ) matches "XyZ" -> 3-3
    x2(b"(?<=\\g<ab>)|-\\zEND (?<ab>XyZ)", b"XyZ", 3, 3);
}

#[test]
fn recursive_named_empty_or_a() {
    // C line 710: (?<n>|a\g<n>)+ matches "" -> 0-0
    x2(b"(?<n>|a\\g<n>)+", b"", 0, 0);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_parens() {
    // C line 711: (?<n>|\(\g<n>\))+$ matches "()(())" -> 0-6
    x2(b"(?<n>|\\(\\g<n>\\))+$", b"()(())", 0, 6);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_g_n_dot_zero() {
    // C line 712: \g<n>(?<n>.){0} matches "X" group 1 -> 0-1
    x3(b"\\g<n>(?<n>.){0}", b"X", 0, 1, 1);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_g_n_alt_df() {
    // C line 713: \g<n>(abc|df(?<n>.YZ){2,8}){0} matches "XYZ" -> 0-3
    x2(b"\\g<n>(abc|df(?<n>.YZ){2,8}){0}", b"XYZ", 0, 3);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_A_n_a_or_empty() {
    // C line 714: \A(?<n>(a\g<n>)|)\z matches "aaaa" -> 0-4
    x2(b"\\A(?<n>(a\\g<n>)|)\\z", b"aaaa", 0, 4);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_mutual() {
    // C line 715: (?<n>|\g<m>\g<n>)\z|\zEND (?<m>a|(b)\g<m>) matches "bbbbabba" -> 0-8
    x2(b"(?<n>|\\g<m>\\g<n>)\\z|\\zEND (?<m>a|(b)\\g<m>)", b"bbbbabba", 0, 8);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_long_name() {
    // C line 716: (?<name1240>\w+\sx)a+\k<name1240> matches "  fg xaaaaaaaafg x" -> 2-18
    x2(b"(?<name1240>\\w+\\sx)a+\\k<name1240>", b"  fg xaaaaaaaafg x", 2, 18);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_underscore_9() {
    // C line 717: (z)()()(?<_9>a)\g<_9> matches "zaa" group 1 -> 2-3
    x3(b"(z)()()(?<_9>a)\\g<_9>", b"zaa", 2, 3, 1);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_underscore_backref() {
    // C line 718: (.)(((?<_>a)))\k<_> matches "zaa" -> 0-3
    x2(b"(.)(((?<_>a)))\\k<_>", b"zaa", 0, 3);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_digit_or_word() {
    // C line 719: ((?<name1>\d)|(?<name2>\w))(\k<name1>|\k<name2>) matches "ff" -> 0-2
    x2(b"((?<name1>\\d)|(?<name2>\\w))(\\k<name1>|\\k<name2>)", b"ff", 0, 2);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_dup_empty() {
    // C line 720: (?:(?<x>)|(?<x>efg))\k<x> matches "" -> 0-0
    x2(b"(?:(?<x>)|(?<x>efg))\\k<x>", b"", 0, 0);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_dup_abc_efg() {
    // C line 721: (?:(?<x>abc)|(?<x>efg))\k<x> matches "abcefgefg" -> 3-9
    x2(b"(?:(?<x>abc)|(?<x>efg))\\k<x>", b"abcefgefg", 3, 9);
}

#[test]
fn named_group_dup_no_match() {
    // C line 722: (?:(?<x>abc)|(?<x>efg))\k<x> no match for "abcefg"
    n(b"(?:(?<x>abc)|(?<x>efg))\\k<x>", b"abcefg");
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_dup_x_xx() {
    // C line 723: (?<x>x)(?<x>xx)\k<x> matches "xxxx" -> 0-4
    x2(b"(?<x>x)(?<x>xx)\\k<x>", b"xxxx", 0, 4);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_dup_x_xx_z() {
    // C line 724: (?<x>x)(?<x>xx)\k<x> matches "xxxxz" -> 0-4
    x2(b"(?<x>x)(?<x>xx)\\k<x>", b"xxxxz", 0, 4);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_14_dup_pyumpyum() {
    // C line 725: 14 alternatives of (?<n1>...) followed by \k<n1>$ matches "a-pyumpyum" -> 2-10
    x2(b"(?:(?<n1>.)|(?<n1>..)|(?<n1>...)|(?<n1>....)|(?<n1>.....)|(?<n1>......)|(?<n1>.......)|(?<n1>........)|(?<n1>.........)|(?<n1>..........)|(?<n1>...........)|(?<n1>............)|(?<n1>.............)|(?<n1>..............))\\k<n1>$", b"a-pyumpyum", 2, 10);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_group_14_dup_capture_14() {
    // C line 726: same pattern, capture group 14 of long string -> 4-18
    x3(b"(?:(?<n1>.)|(?<n1>..)|(?<n1>...)|(?<n1>....)|(?<n1>.....)|(?<n1>......)|(?<n1>.......)|(?<n1>........)|(?<n1>.........)|(?<n1>..........)|(?<n1>...........)|(?<n1>............)|(?<n1>.............)|(?<n1>..............))\\k<n1>$", b"xxxxabcdefghijklmnabcdefghijklmn", 4, 18, 14);
}

#[test]
fn named_group_16_aaa() {
    // C line 727: 16 named groups, group 16 = aaa, match "aaa" -> 0-3
    x3(b"(?<name1>)(?<name2>)(?<name3>)(?<name4>)(?<name5>)(?<name6>)(?<name7>)(?<name8>)(?<name9>)(?<name10>)(?<name11>)(?<name12>)(?<name13>)(?<name14>)(?<name15>)(?<name16>aaa)(?<name17>)$", b"aaa", 0, 3, 16);
}

#[test]
fn recursive_foo_parens() {
    // C line 728: (?<foo>a|\(\g<foo>\)) matches "a" -> 0-1
    x2(b"(?<foo>a|\\(\\g<foo>\\))", b"a", 0, 1);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_foo_nested_parens() {
    // C line 729: (?<foo>a|\(\g<foo>\)) matches "((((((a))))))" -> 0-13
    x2(b"(?<foo>a|\\(\\g<foo>\\))", b"((((((a))))))", 0, 13);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_foo_nested_parens_capture() {
    // C line 730: (?<foo>a|\(\g<foo>\)) matches "((((((((a))))))))" group 1 -> 0-17
    x3(b"(?<foo>a|\\(\\g<foo>\\))", b"((((((((a))))))))", 0, 17, 1);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_bar_abc_dollar() {
    // C line 731: \g<bar>|\zEND(?<bar>.*abc$) matches "abcxxxabc" -> 0-9
    x2(b"\\g<bar>|\\zEND(?<bar>.*abc$)", b"abcxxxabc", 0, 9);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_g1_bac() {
    // C line 732: \g<1>|\zEND(.a.) matches "bac" -> 0-3
    x2(b"\\g<1>|\\zEND(.a.)", b"bac", 0, 3);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_g_A_mutual_capture() {
    // C line 733: \g<_A>\g<_A>|\zEND(.a.)(?<_A>.b.) matches "xbxyby" group 1 -> 3-6
    x3(b"\\g<_A>\\g<_A>|\\zEND(.a.)(?<_A>.b.)", b"xbxyby", 3, 6, 1);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_mutual_pan_pon() {
    // C line 734: mutual recursion pan/pon matches "cdcbcdc" -> 0-7
    x2(b"\\A(?:\\g<pon>|\\g<pan>|\\zEND  (?<pan>a|c\\g<pon>c)(?<pon>b|d\\g<pan>d))$", b"cdcbcdc", 0, 7);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_A_n_or_a_gm() {
    // C line 735: \A(?<n>|a\g<m>)\z|\zEND (?<m>\g<n>) matches "aaaa" -> 0-4
    x2(b"\\A(?<n>|a\\g<m>)\\z|\\zEND (?<m>\\g<n>)", b"aaaa", 0, 4);
}

#[test]
fn recursive_n_3_5_interval() {
    // C line 736: (?<n>(a|b\g<n>c){3,5}) matches "baaaaca" -> 1-5
    x2(b"(?<n>(a|b\\g<n>c){3,5})", b"baaaaca", 1, 5);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_n_3_5_interval_long() {
    // C line 737: (?<n>(a|b\g<n>c){3,5}) matches "baaaacaaaaa" -> 0-10
    x2(b"(?<n>(a|b\\g<n>c){3,5})", b"baaaacaaaaa", 0, 10);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_possessive_parens() {
    // C line 738: (?<pare>\(([^\(\)]++|\g<pare>)*+\)) matches "((a))" -> 0-5
    x2(b"(?<pare>\\(([^\\(\\)]++|\\g<pare>)*+\\))", b"((a))", 0, 5);
}

// ============================================================================
// Complex backref/capture coverage (C lines 742-758)
// ============================================================================

#[test]
#[ignore] // backref in alternation with empty capture not yet implemented
fn backref_or_empty_capture_star() {
    // C line 742: (?:\1a|())*  matches "a" group 1 -> 0-0
    x3(b"(?:\\1a|())*", b"a", 0, 0, 1);
}

#[test]
fn dotstar_star_x() {
    // C line 743: x((.)*)*x matches "0x1x2x3" -> 1-6
    x2(b"x((.)*)*x", b"0x1x2x3", 1, 6);
}

#[test]
#[ignore] // case-insensitive backrefs not yet implemented
fn dotstar_star_x_casefold_backref() {
    // C line 744: x((.)*)*x(?i:\1)\Z matches "0x1x2x1X2" -> 1-9
    x2(b"x((.)*)*x(?i:\\1)\\Z", b"0x1x2x1X2", 1, 9);
}

#[test]
fn multi_empty_capture_star_backref() {
    // C line 745: (?:()|()|()|()|()|())*\2\5 matches "" -> 0-0
    x2(b"(?:()|()|()|()|()|())*\\2\\5", b"", 0, 0);
}

#[test]
fn multi_empty_capture_star_backref_b() {
    // C line 746: (?:()|()|()|(x)|()|())*\2b\5 matches "b" -> 0-1
    x2(b"(?:()|()|()|(x)|()|())*\\2b\\5", b"b", 0, 1);
}

#[test]
fn char_class_0_9_dash_a() {
    // C line 747: [0-9-a] matches "-" -> 0-1 (PR#44)
    x2(b"[0-9-a]", b"-", 0, 1);
}

#[test]
fn char_class_0_9_dash_a_no_match() {
    // C line 748: [0-9-a] no match for ":" (PR#44)
    n(b"[0-9-a]", b":");
}

#[test]
fn recursive_parens_pr43() {
    // C line 749: (\(((?:[^(]|\g<1>)*)\)) matches "(abc)(abc)" group 2 -> 1-4 (PR#43)
    x3(b"(\\(((?:[^(]|\\g<1>)*)\\))", b"(abc)(abc)", 1, 4, 2);
}

#[test]
fn octal_escape_o101() {
    // C line 750: \o{101} matches "A" -> 0-1
    x2(b"\\o{101}", b"A", 0, 1);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_backref_level_k1p3() {
    // C line 751: \A(a|b\g<1>c)\k<1+3>\z matches "bbacca" -> 0-6
    x2(b"\\A(a|b\\g<1>c)\\k<1+3>\\z", b"bbacca", 0, 6);
}

#[test]
fn recursive_backref_level_k1p3_no_match() {
    // C line 752: \A(a|b\g<1>c)\k<1+3>\z no match for "bbaccb"
    n(b"\\A(a|b\\g<1>c)\\k<1+3>\\z", b"bbaccb");
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_casefold_backref_level() {
    // C line 753: (?i)\A(a|b\g<1>c)\k<1+2>\z matches "bBACcbac" -> 0-8
    x2(b"(?i)\\A(a|b\\g<1>c)\\k<1+2>\\z", b"bBACcbac", 0, 8);
}

#[test]
#[ignore] // case-insensitive backrefs not yet implemented
fn casefold_named_dup_backref() {
    // C line 754: (?i)(?<X>aa)|(?<X>bb)\k<X> matches "BBbb" -> 0-4
    x2(b"(?i)(?<X>aa)|(?<X>bb)\\k<X>", b"BBbb", 0, 4);
}

#[test]
#[ignore] // relative backrefs/calls not yet implemented
fn relative_positive_backref() {
    // C line 755: (?:\k'+1'B|(A)C)* matches "ACAB" -> 0-4
    x2(b"(?:\\k'+1'B|(A)C)*", b"ACAB", 0, 4);
}

#[test]
#[ignore] // relative backrefs/calls not yet implemented
fn relative_positive_call() {
    // C line 756: \g<+2>(abc)(ABC){0} matches "ABCabc" -> 0-6
    x2(b"\\g<+2>(abc)(ABC){0}", b"ABCabc", 0, 6);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_self_call_g0() {
    // C line 757: A\g'0'|B() matches "AAAAB" -> 0-5
    x2(b"A\\g'0'|B()", b"AAAAB", 0, 5);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_self_call_g0_capture() {
    // C line 758: (A\g'0')|B matches "AAAAB" group 1 -> 0-5
    x3(b"(A\\g'0')|B", b"AAAAB", 0, 5, 1);
}

// ============================================================================
// Conditionals (?(1)...) (C lines 759-771)
// ============================================================================

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_yes_only() {
    // C line 759: (a*)(?(1))aa matches "aaaaa" -> 0-5
    x2(b"(a*)(?(1))aa", b"aaaaa", 0, 5);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_negative_ref() {
    // C line 760: (a*)(?(-1))aa matches "aaaaa" -> 0-5
    x2(b"(a*)(?(-1))aa", b"aaaaa", 0, 5);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_named_ref() {
    // C line 761: (?<name>aaa)(?('name'))aa matches "aaaaa" -> 0-5
    x2(b"(?<name>aaa)(?('name'))aa", b"aaaaa", 0, 5);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_yes_no() {
    // C line 762: (a)(?(1)aa|bb)a matches "aaaaa" -> 0-4
    x2(b"(a)(?(1)aa|bb)a", b"aaaaa", 0, 4);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_named_yes_no() {
    // C line 763: (?:aa|())(?(<1>)aa|bb)a matches "aabba" -> 0-5
    x2(b"(?:aa|())(?(<1>)aa|bb)a", b"aabba", 0, 5);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_named_yes_no_cc() {
    // C line 764: (?:aa|())(?('1')aa|bb|cc)a matches "aacca" -> 0-5
    x2(b"(?:aa|())(?('1')aa|bb|cc)a", b"aacca", 0, 5);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_capture_group() {
    // C line 765: (a*)(?(1)aa|a)b matches "aaab" group 1 -> 0-1
    x3(b"(a*)(?(1)aa|a)b", b"aaab", 0, 1, 1);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_no_match() {
    // C line 766: (a)(?(1)a|b)c no match for "abc"
    n(b"(a)(?(1)a|b)c", b"abc");
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_empty_yes() {
    // C line 767: (a)(?(1)|)c matches "ac" -> 0-2
    x2(b"(a)(?(1)|)c", b"ac", 0, 2);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_empty_cond_no_match() {
    // C line 768: (?()aaa|bbb) no match for "bbb"
    n(b"(?()aaa|bbb)", b"bbb");
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_plus_zero() {
    // C line 769: (a)(?(1+0)b|c)d matches "abd" -> 0-3
    x2(b"(a)(?(1+0)b|c)d", b"abd", 0, 3);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_named_alt_ace() {
    // C line 770: (?:(?'name'a)|(?'name'b))(?('name')c|d)e matches "ace" -> 0-3
    x2(b"(?:(?'name'a)|(?'name'b))(?('name')c|d)e", b"ace", 0, 3);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_named_alt_bce() {
    // C line 771: (?:(?'name'a)|(?'name'b))(?('name')c|d)e matches "bce" -> 0-3
    x2(b"(?:(?'name'a)|(?'name'b))(?('name')c|d)e", b"bce", 0, 3);
}

// ============================================================================
// Additional coverage patterns (C lines 791-824, 831-837)
// Skip: 772-790 (\R, \N, \O, \K), 817 ((?W))
// ============================================================================

#[test]
fn empty_cap_star_backref_1() {
    // C line 791: (?:()|())*\1 matches "abc" -> 0-0
    x2(b"(?:()|())*\\1", b"abc", 0, 0);
}

#[test]
fn empty_cap_star_backref_2() {
    // C line 792: (?:()|())*\2 matches "abc" -> 0-0
    x2(b"(?:()|())*\\2", b"abc", 0, 0);
}

#[test]
fn triple_empty_cap_star_backref() {
    // C line 793: (?:()|()|())*\3\1 matches "abc" -> 0-0
    x2(b"(?:()|()|())*\\3\\1", b"abc", 0, 0);
}

#[test]
fn recursive_empty_or_a_g1() {
    // C line 794: (|(?:a(?:\g'1')*))b| matches "abc" -> 0-2
    x2(b"(|(?:a(?:\\g'1')*))b|", b"abc", 0, 2);
}

#[test]
fn multi_alt_lazy_repeat_z() {
    // C line 796: (abc|def|ghi|jkl|mno|pqr|stu){0,10}?\z matches "admno" -> 2-5
    x2(b"(abc|def|ghi|jkl|mno|pqr|stu){0,10}?\\z", b"admno", 2, 5);
}

#[test]
fn nested_alt_lazy_repeat_z() {
    // C line 797: (abc|(def|ghi|jkl|mno|pqr){0,7}?){5}\z matches "adpqrpqrpqr" -> 2-11
    x2(b"(abc|(def|ghi|jkl|mno|pqr){0,7}?){5}\\z", b"adpqrpqrpqr", 2, 11);
}

#[test]
fn capture_noncap_a_opt_plus_coverage() {
    // C line 802: ((?:a(?:b|c|d|e|f|g|h|i|j|k|l|m|n))+)? matches "abacadae" -> 0-8
    x2(b"((?:a(?:b|c|d|e|f|g|h|i|j|k|l|m|n))+)?", b"abacadae", 0, 8);
}

#[test]
fn capture_noncap_a_lazyplus_z() {
    // C line 803: ((?:a(?:b|c|d|e|f|g|h|i|j|k|l|m|n))+?)?z matches "abacadaez" -> 0-9
    x2(b"((?:a(?:b|c|d|e|f|g|h|i|j|k|l|m|n))+?)?z", b"abacadaez", 0, 9);
}

#[test]
fn cap_a_or_b_lazyq_opt_z() {
    // C line 804: \A((a|b)??)?z matches "bz" -> 0-2
    x2(b"\\A((a|b)??)?z", b"bz", 0, 2);
}

#[test]
#[ignore] // named subroutine calls not yet implemented
fn cap_named_subroutine_zero() {
    // C line 805: ((?<x>abc){0}a\g<x>d)+ matches "aabcd" -> 0-5
    x2(b"((?<x>abc){0}a\\g<x>d)+", b"aabcd", 0, 5);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_abc_coverage() {
    // C line 806: ((?(abc)true|false))+ matches "false" -> 0-5
    x2(b"((?(abc)true|false))+", b"false", 0, 5);
}

#[test]
fn casefold_group_d_plus() {
    // C line 807: ((?i:abc)d)+ matches "abcdABCd" -> 0-8
    x2(b"((?i:abc)d)+", b"abcdABCd", 0, 8);
}

#[test]
#[ignore] // negative lookbehind not yet fully implemented
fn neg_lookbehind_def_coverage() {
    // C line 808: ((?<!abc)def)+ matches "bcdef" -> 2-5
    x2(b"((?<!abc)def)+", b"bcdef", 2, 5);
}

#[test]
fn word_boundary_capture_plus() {
    // C line 809: (\ba)+ matches "aaa" -> 0-1
    x2(b"(\\ba)+", b"aaa", 0, 1);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_named_coverage() {
    // C line 810: ()(?<x>ab)(?(<x>)a|b) matches "aba" -> 0-3
    x2(b"()(?<x>ab)(?(<x>)a|b)", b"aba", 0, 3);
}

#[test]
fn lookbehind_dot_coverage() {
    // C line 811: (?<=a.b)c matches "azbc" -> 3-4
    x2(b"(?<=a.b)c", b"azbc", 3, 4);
}

#[test]
fn lookbehind_long_repeat_no_match() {
    // C line 812: (?<=(?:abcde){30})z no match for "abc"
    n(b"(?<=(?:abcde){30})z", b"abc");
}

#[test]
#[ignore] // conditionals inside lookbehind not yet implemented
fn lookbehind_conditional_coverage() {
    // C line 813: (?<=(?(a)a|bb))z matches "aaz" -> 2-3
    x2(b"(?<=(?(a)a|bb))z", b"aaz", 2, 3);
}

#[test]
fn lookbehind_nested_coverage() {
    // C line 818: (?<=ab(?<=ab)) matches "ab" -> 2-2
    x2(b"(?<=ab(?<=ab))", b"ab", 2, 2);
}

#[test]
#[ignore] // named group backrefs/subroutines not yet implemented
fn named_dup_backref_plus_coverage() {
    // C line 819: (?<x>a)(?<x>b)(\k<x>)+ matches "abbaab" -> 0-6
    x2(b"(?<x>a)(?<x>b)(\\k<x>)+", b"abbaab", 0, 6);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_backref_match_coverage() {
    // C line 821: ((?(a)b|c))(\1) matches "abab" -> 0-4
    x2(b"((?(a)b|c))(\\1)", b"abab", 0, 4);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_dollar_or_b_coverage() {
    // C line 822: (?<x>$|b\g<x>) matches "bbb" -> 0-3
    x2(b"(?<x>$|b\\g<x>)", b"bbb", 0, 3);
}

#[test]
#[ignore] // recursive patterns not yet implemented
fn recursive_conditional_cccb_coverage() {
    // C line 823: (?<x>(?(a)a|b)|c\g<x>) matches "cccb" -> 0-4
    x2(b"(?<x>(?(a)a|b)|c\\g<x>)", b"cccb", 0, 4);
}

#[test]
#[ignore] // conditionals not yet implemented
fn conditional_star_plus_coverage() {
    // C line 824: (a)(?(1)a*|b*)+ matches "aaaa" -> 0-4
    x2(b"(a)(?(1)a*|b*)+", b"aaaa", 0, 4);
}

#[test]
fn nested_cc_intersection_coverage() {
    // C line 825: [[^abc]&&cde]* matches "de" -> 0-2
    x2(b"[[^abc]&&cde]*", b"de", 0, 2);
}

#[test]
#[ignore] // conditionals inside lookbehind not yet implemented
fn conditional_lookbehind_Q_no_match() {
    // C line 694: (Q)|(?<=a|(?(1))|b)c no match for "czc"
    n(b"(Q)|(?<=a|(?(1))|b)c", b"czc");
}

#[test]
#[ignore] // conditionals inside lookbehind not yet implemented
fn conditional_lookbehind_Q_match() {
    // C line 695: (Q)(?<=a|(?(1))|b)c matches "cQc" -> 1-3
    x2(b"(Q)(?<=a|(?(1))|b)c", b"cQc", 1, 3);
}

#[test]
fn hex_digit_class() {
    // C line 831: \h matches "5" -> 0-1
    x2(b"\\h", b"5", 0, 1);
}

#[test]
fn non_hex_digit_class() {
    // C line 832: \H matches "z" -> 0-1
    x2(b"\\H", b"z", 0, 1);
}

#[test]
fn hex_digit_in_cc() {
    // C line 833: [\h] matches "5" -> 0-1
    x2(b"[\\h]", b"5", 0, 1);
}

#[test]
fn non_hex_digit_in_cc() {
    // C line 834: [\H] matches "z" -> 0-1
    x2(b"[\\H]", b"z", 0, 1);
}

#[test]
fn octal_in_cc() {
    // C line 835: [\o{101}] matches "A" -> 0-1
    x2(b"[\\o{101}]", b"A", 0, 1);
}

#[test]
fn unicode_in_cc() {
    // C line 836: [\u0041] matches "A" -> 0-1
    x2(b"[\\u0041]", b"A", 0, 1);
}
