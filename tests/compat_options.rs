// compat_options.rs - Integration tests ported from oniguruma test/test_options.c
//
// Tests compile/search options passed to both onig_new() and onig_search().
// Uses ONIG_SYNTAX_ONIGURUMA (the default syntax) and ONIG_ENCODING_UTF8.
//
// Port note: In the C original, options are passed as the first parameter
// to x2/x3/n macros and applied to both onig_new() and onig_search().

use ferroni::oniguruma::*;
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::regsyntax::OnigSyntaxOniguruma;

fn x2(options: OnigOptionType, pattern: &[u8], input: &[u8], from: i32, to: i32) {
    let reg = onig_new(
        pattern,
        options,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
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
        options,
    );

    assert!(
        result >= 0,
        "x2: expected match for {:?} against {:?} with options {:#x}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        options,
        result
    );

    let region = region.unwrap();
    assert_eq!(
        region.beg[0],
        from,
        "x2: wrong start for {:?} against {:?}: expected {}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        from,
        region.beg[0]
    );
    assert_eq!(
        region.end[0],
        to,
        "x2: wrong end for {:?} against {:?}: expected {}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        to,
        region.end[0]
    );
}

fn x3(options: OnigOptionType, pattern: &[u8], input: &[u8], from: i32, to: i32, mem: usize) {
    let reg = onig_new(
        pattern,
        options,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
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
        options,
    );

    assert!(
        result >= 0,
        "x3: expected match for {:?} against {:?} with options {:#x}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        options,
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
        region.beg[mem],
        from,
        "x3: wrong start for group {} of {:?}: expected {}, got {}",
        mem,
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        from,
        region.beg[mem]
    );
    assert_eq!(
        region.end[mem],
        to,
        "x3: wrong end for group {} of {:?}: expected {}, got {}",
        mem,
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        to,
        region.end[mem]
    );
}

fn n(options: OnigOptionType, pattern: &[u8], input: &[u8]) {
    let reg = onig_new(
        pattern,
        options,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
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
        options,
    );

    assert_eq!(
        result,
        ONIG_MISMATCH,
        "n: expected no match for {:?} against {:?} with options {:#x}, got {}",
        std::str::from_utf8(pattern).unwrap_or("<invalid>"),
        std::str::from_utf8(input).unwrap_or("<invalid>"),
        options,
        result
    );
}

// ============================================================================
// IGNORECASE
// ============================================================================

const OIA: OnigOptionType = ONIG_OPTION_IGNORECASE.union(ONIG_OPTION_IGNORECASE_IS_ASCII);

#[test]
fn option_ignorecase_basic() {
    x2(ONIG_OPTION_IGNORECASE, b"a", b"A", 0, 1);
}

#[test]
fn option_ignorecase_is_ascii_no_fold() {
    n(ONIG_OPTION_IGNORECASE_IS_ASCII, b"a", b"A");
}

#[test]
fn option_ignorecase_kelvin_sign_to_k() {
    // KELVIN SIGN U+212A
    x2(ONIG_OPTION_IGNORECASE, b"\xe2\x84\xaa", b"k", 0, 1);
}

#[test]
fn option_ignorecase_k_to_kelvin_sign() {
    x2(ONIG_OPTION_IGNORECASE, b"k", b"\xe2\x84\xaa", 0, 3);
}

#[test]
fn option_oia_no_kelvin_to_k() {
    n(OIA, b"\xe2\x84\xaa", b"k");
}

#[test]
fn option_oia_no_k_to_kelvin() {
    n(OIA, b"k", b"\xe2\x84\xaa");
}

#[test]
fn option_oia_a_to_a() {
    x2(OIA, b"a", b"a", 0, 1);
}

#[test]
fn option_oia_upper_a() {
    x2(OIA, b"A", b"A", 0, 1);
}

#[test]
fn option_oia_lower_a_to_upper_a() {
    x2(OIA, b"a", b"A", 0, 1);
}

#[test]
fn option_oia_upper_a_to_lower_a() {
    x2(OIA, b"A", b"a", 0, 1);
}

#[test]
fn option_oia_full_alphabet_upper_to_lower() {
    x2(
        OIA,
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        b"abcdefghijklmnopqrstuvwxyz",
        0,
        26,
    );
}

#[test]
fn option_oia_full_alphabet_lower_to_upper() {
    x2(
        OIA,
        b"abcdefghijklmnopqrstuvwxyz",
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        0,
        26,
    );
}

#[test]
fn option_oia_upper_alpha_partial_match() {
    x2(
        OIA,
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        b"ABCabcdefghijklmnopqrstuvwxyz",
        3,
        29,
    );
}

#[test]
fn option_oia_lower_alpha_partial_match() {
    x2(
        OIA,
        b"abcdefghijklmnopqrstuvwxyz",
        b"abcABCDEFGHIJKLMNOPQRSTUVWXYZ",
        3,
        29,
    );
}

#[test]
fn option_oia_capture_group() {
    x3(OIA, b"#%(a!;)(b&)", b"#%A!;B&", 5, 7, 2);
}

// ============================================================================
// IGNORECASE with sharp-s (U+00DF)
// ============================================================================

#[test]
fn option_ignorecase_ss_to_sharp_s() {
    x2(ONIG_OPTION_IGNORECASE, b"ss", b"\xc3\x9f", 0, 2);
}

#[test]
fn option_ignorecase_sharp_s_to_ss() {
    x2(ONIG_OPTION_IGNORECASE, b"\xc3\x9f", b"SS", 0, 2);
}

#[test]
fn option_oia_no_ss_to_sharp_s() {
    n(OIA, b"ss", b"\xc3\x9f");
}

#[test]
fn option_oia_no_sharp_s_to_ss() {
    n(OIA, b"\xc3\x9f", b"ss");
}

#[test]
fn option_oia_ss_to_upper_ss() {
    x2(OIA, b"ss", b"SS", 0, 2);
}

#[test]
fn option_oia_mixed_case_ss() {
    x2(OIA, b"Ss", b"sS", 0, 2);
}

// ============================================================================
// NOTBOL / NOTEOL
// ============================================================================

#[test]
fn option_notbol_caret() {
    n(ONIG_OPTION_NOTBOL, b"^ab", b"ab");
}

#[test]
fn option_notbol_begin_buf() {
    n(ONIG_OPTION_NOTBOL, b"\\Aab", b"ab");
}

#[test]
fn option_noteol_dollar() {
    n(ONIG_OPTION_NOTEOL, b"ab$", b"ab");
}

#[test]
fn option_noteol_end_buf_z() {
    n(ONIG_OPTION_NOTEOL, b"ab\\z", b"ab");
}

#[test]
fn option_noteol_end_buf_big_z() {
    n(ONIG_OPTION_NOTEOL, b"ab\\Z", b"ab");
}

#[test]
fn option_noteol_end_buf_big_z_newline() {
    n(ONIG_OPTION_NOTEOL, b"ab\\Z", b"ab\n");
}

// ============================================================================
// NOT_BEGIN_STRING / NOT_END_STRING
// ============================================================================

#[test]
fn option_not_begin_string() {
    n(ONIG_OPTION_NOT_BEGIN_STRING, b"\\Aab", b"ab");
}

#[test]
fn option_not_end_string_z() {
    n(ONIG_OPTION_NOT_END_STRING, b"ab\\z", b"ab");
}

#[test]
fn option_not_end_string_big_z() {
    n(ONIG_OPTION_NOT_END_STRING, b"ab\\Z", b"ab");
}

#[test]
fn option_not_end_string_big_z_newline() {
    n(ONIG_OPTION_NOT_END_STRING, b"ab\\Z", b"ab\n");
}

// ============================================================================
// MATCH_WHOLE_STRING
// ============================================================================

#[test]
fn option_none_alternation_partial() {
    x2(ONIG_OPTION_NONE, b"a|abc", b"abc", 0, 1);
}

#[test]
fn option_none_alternation_with_end_anchor() {
    x2(ONIG_OPTION_NONE, b"(a|abc)\\Z", b"abc", 0, 3);
}

#[test]
fn option_match_whole_string_abc() {
    x2(ONIG_OPTION_MATCH_WHOLE_STRING, b"a|abc", b"abc", 0, 3);
}

#[test]
fn option_match_whole_string_a() {
    x2(ONIG_OPTION_MATCH_WHOLE_STRING, b"a|abc", b"a", 0, 1);
}

// ============================================================================
// *_IS_ASCII options
// ============================================================================

#[test]
fn option_word_is_ascii_match() {
    x2(ONIG_OPTION_WORD_IS_ASCII, b"\\w", b"@g", 1, 2);
}

#[test]
fn option_word_is_ascii_no_hiragana() {
    n(ONIG_OPTION_WORD_IS_ASCII, b"\\w", "あ".as_bytes());
}

#[test]
fn option_none_digit_fullwidth() {
    x2(ONIG_OPTION_NONE, b"\\d", "１".as_bytes(), 0, 3);
}

#[test]
fn option_digit_is_ascii_no_fullwidth() {
    n(ONIG_OPTION_DIGIT_IS_ASCII, b"\\d", "１".as_bytes());
}

#[test]
fn option_space_is_ascii_space() {
    x2(ONIG_OPTION_SPACE_IS_ASCII, b"\\s", b" ", 0, 1);
}

#[test]
fn option_none_space_fullwidth() {
    // U+3000 IDEOGRAPHIC SPACE
    x2(ONIG_OPTION_NONE, b"\\s", "　".as_bytes(), 0, 3);
}

#[test]
fn option_space_is_ascii_no_fullwidth() {
    n(ONIG_OPTION_SPACE_IS_ASCII, b"\\s", "　".as_bytes());
}

// ============================================================================
// POSIX_IS_ASCII
// ============================================================================

#[test]
fn option_posix_is_ascii_match() {
    x2(ONIG_OPTION_POSIX_IS_ASCII, b"\\w\\d\\s", b"c3 ", 0, 3);
}

#[test]
fn option_posix_is_ascii_no_fullwidth() {
    n(
        ONIG_OPTION_POSIX_IS_ASCII,
        b"\\w|\\d|\\s",
        "あ４　".as_bytes(),
    );
}

// ============================================================================
// EXTEND / FIND_LONGEST / FIND_NOT_EMPTY
// ============================================================================

#[test]
fn option_extend_whitespace_ignored() {
    x2(ONIG_OPTION_EXTEND, b" abc  \n def", b"abcdef", 0, 6);
}

#[test]
fn option_find_longest() {
    x2(ONIG_OPTION_FIND_LONGEST, b"\\w+", b"abc defg hij", 4, 8);
}

#[test]
fn option_find_not_empty() {
    x2(
        ONIG_OPTION_FIND_NOT_EMPTY,
        b"\\w*",
        b"@@@ abc defg hij",
        4,
        7,
    );
}
