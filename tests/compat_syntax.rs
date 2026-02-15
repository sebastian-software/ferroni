// compat_syntax.rs - Integration tests ported from oniguruma test/test_syntax.c
//
// Tests different regex syntax definitions: Perl, Java, Python, POSIX Basic,
// Grep, Emacs, Perl_NG. Uses forward search with ONIG_ENCODING_UTF8.
//
// Port note: The C original uses a global `Syntax` variable that changes
// between test groups. Here, each test function receives the syntax as a
// parameter through syntax-specific helper functions.

use ferroni::oniguruma::*;
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::regsyntax::*;

fn x2_syn(syntax: &OnigSyntaxType, pattern: &[u8], input: &[u8], from: i32, to: i32) {
    let reg = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        syntax,
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

fn x3_syn(syntax: &OnigSyntaxType, pattern: &[u8], input: &[u8], from: i32, to: i32, mem: usize) {
    let reg = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        syntax,
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

fn n_syn(syntax: &OnigSyntaxType, pattern: &[u8], input: &[u8]) {
    let reg = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        syntax,
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

fn e_syn(syntax: &OnigSyntaxType, pattern: &[u8], input: &[u8], expected_error: i32) {
    let result = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        syntax,
    );
    match result {
        Err(code) => {
            assert_eq!(
                code.code(),
                expected_error,
                "e: expected error {} for {:?}, got error {}",
                expected_error,
                std::str::from_utf8(pattern).unwrap_or("<invalid>"),
                code.code()
            );
        }
        Ok(reg) => {
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
                expected_error,
                "e: expected error {} for {:?}, but got result {}",
                expected_error,
                std::str::from_utf8(pattern).unwrap_or("<invalid>"),
                result
            );
        }
    }
}

// ============================================================================
// Shared test functions (called with multiple syntaxes, matching C structure)
// ============================================================================

fn test_reluctant_interval(syn: &OnigSyntaxType) {
    x2_syn(syn, b"a{1,3}?", b"aaa", 0, 1);
    x2_syn(syn, b"a{3}", b"aaa", 0, 3);
    x2_syn(syn, b"a{3}?", b"aaa", 0, 3);
    n_syn(syn, b"a{3}?", b"aa");
    x2_syn(syn, b"a{3,3}?", b"aaa", 0, 3);
    n_syn(syn, b"a{3,3}?", b"aa");
}

fn test_possessive_interval(syn: &OnigSyntaxType) {
    x2_syn(syn, b"a{1,3}+", b"aaaaaa", 0, 3);
    x2_syn(syn, b"a{3}+", b"aaaaaa", 0, 3);
    x2_syn(syn, b"a{3,3}+", b"aaaaaa", 0, 3);
}

fn test_isolated_option(syn: &OnigSyntaxType) {
    x2_syn(syn, b"", b"", 0, 0);
    x2_syn(syn, b"^", b"", 0, 0);
    n_syn(syn, b"^a", b"\na");
    n_syn(syn, b".", b"\n");
    x2_syn(syn, b"(?s:.)", b"\n", 0, 1);
    x2_syn(syn, b"(?s).", b"\n", 0, 1);
    x2_syn(syn, b"(?s)a|.", b"\n", 0, 1);
    n_syn(syn, b"(?s:a)|.", b"\n");
    x2_syn(syn, b"b(?s)a|.", b"\n", 0, 1);
    n_syn(syn, b"((?s)a)|.", b"\n");
    n_syn(syn, b"b(?:(?s)a)|z|.", b"\n");
    n_syn(syn, b".|b(?s)a", b"\n");
    n_syn(syn, b".(?s)", b"\n");
    n_syn(syn, b"(?s)(?-s)a|.", b"\n");
    x2_syn(syn, b"(?s)a|.(?-s)", b"\n", 0, 1);
    x2_syn(syn, b"(?s)a|((?-s)).", b"\n", 0, 1);
    x2_syn(syn, b"(?s)a|(?:(?-s)).", b"\n", 0, 1); // !!! Perl 5.26.1 returns empty match
    x2_syn(syn, b"(?s)a|(?:).", b"\n", 0, 1); // !!! Perl 5.26.1 returns empty match
    x2_syn(syn, b"(?s)a|(?:.)", b"\n", 0, 1);
    x2_syn(syn, b"(?s)a|(?:a*).", b"\n", 0, 1);
    n_syn(syn, b"a|(?:).", b"\n"); // !!! Perl 5.26.1 returns empty match
    n_syn(syn, b"a|(?:)(.)", b"\n");
    x2_syn(syn, b"(?s)a|(?:)(.)", b"\n", 0, 1);
    x2_syn(syn, b"b(?s)a|(?:)(.)", b"\n", 0, 1);
    n_syn(syn, b"b((?s)a)|(?:)(.)", b"\n");
}

fn test_prec_read(syn: &OnigSyntaxType) {
    x2_syn(syn, b"(?=a).b", b"ab", 0, 2);
    x2_syn(syn, b"(?=ab|(.))\\1", b"ab", 1, 2);
    n_syn(syn, b"(?!(.)z)a\\1", b"aa"); // ! Perl 5.26.1 match with "aa"
}

fn test_look_behind(syn: &OnigSyntaxType) {
    x2_syn(syn, b"(?<=a)b", b"ab", 1, 2);
    x2_syn(syn, b"(?<=a|b)c", b"abc", 2, 3);
    x2_syn(syn, b"(?<=a|(.))\\1", b"abcc", 3, 4);

    // #295
    n_syn(syn, b"(?<!RMA)X", b"123RMAX");
    x2_syn(syn, b"(?<=RMA)X", b"123RMAX", 6, 7);
    n_syn(syn, b"(?<!RMA)$", b"123RMA");
    x2_syn(syn, b"(?<=RMA)$", b"123RMA", 6, 6);
    n_syn(syn, b"(?<!RMA)\\Z", b"123RMA");
    x2_syn(syn, b"(?<=RMA)\\Z", b"123RMA", 6, 6);
    n_syn(syn, b"(?<!RMA)\\z", b"123RMA");
    x2_syn(syn, b"(?<=RMA)\\z", b"123RMA", 6, 6);

    // following is not match in Perl and Java
    //x2_syn(syn, b"(?<=a|(.))\\1", b"aa", 1, 2);

    n_syn(syn, b"(?<!c|c)a", b"ca");
}

fn test_char_class(syn: &OnigSyntaxType) {
    x2_syn(syn, b"[\\w\\-%]", b"a", 0, 1);
    x2_syn(syn, b"[\\w\\-%]", b"%", 0, 1);
    x2_syn(syn, b"[\\w\\-%]", b"-", 0, 1);

    //e_syn(syn, b"[\\w-%]", b"-", ONIGERR_UNMATCHED_RANGE_SPECIFIER_IN_CHAR_CLASS);
    x2_syn(syn, b"[\\w-%]", b"a", 0, 1);
    x2_syn(syn, b"[\\w-%]", b"%", 0, 1);
    x2_syn(syn, b"[\\w-%]", b"-", 0, 1);
}

fn test_python_option_ascii(syn: &OnigSyntaxType) {
    x2_syn(syn, b"(?a)\\w", b"a", 0, 1);
    x2_syn(syn, b"\\w", "あ".as_bytes(), 0, 3);
    n_syn(syn, b"(?a)\\w", "あ".as_bytes());
    x2_syn(syn, b"\\s", "　".as_bytes(), 0, 3);
    n_syn(syn, b"(?a)\\s", "　".as_bytes());
    x2_syn(syn, b"\\d", "５".as_bytes(), 0, 3);
    n_syn(syn, b"(?a)\\d", "５".as_bytes());
    x2_syn(syn, "あ\\b ".as_bytes(), "あ ".as_bytes(), 0, 4);
    n_syn(syn, "(?a)あ\\b ".as_bytes(), "あ ".as_bytes());
    n_syn(syn, "あ\\B ".as_bytes(), "あ ".as_bytes());
    x2_syn(syn, "(?a)あ\\B ".as_bytes(), "あ ".as_bytes(), 0, 4);
    x2_syn(syn, b"(?a)\\W", "あ".as_bytes(), 0, 3);
    n_syn(syn, b"\\W", "あ".as_bytes());
    x2_syn(syn, b"(?a)\\S", "　".as_bytes(), 0, 3);
    n_syn(syn, b"\\S", "　".as_bytes());
    x2_syn(syn, b"(?a)\\D", "５".as_bytes(), 0, 3);
    n_syn(syn, b"\\D", "５".as_bytes());
}

fn test_python_z(syn: &OnigSyntaxType) {
    x2_syn(syn, b"a\\Z", b"a", 0, 1);
    n_syn(syn, b"a\\Z", b"a\n");
    e_syn(syn, b"\\z", b"a", ONIGERR_UNDEFINED_OPERATOR);
}

fn test_python_single_multi(syn: &OnigSyntaxType) {
    n_syn(syn, b".", b"\n");
    x2_syn(syn, b"(?s).", b"\n", 0, 1);

    n_syn(syn, b"^abc", b"\nabc");
    x2_syn(syn, b"(?m)^abc", b"\nabc", 1, 4);
    n_syn(syn, b"abc$", b"abc\ndef");
    x2_syn(syn, b"abc$", b"abc\n", 0, 3);
    x2_syn(syn, b"(?m)abc$", b"abc\ndef", 0, 3);
}

fn test_bre_anchors(syn: &OnigSyntaxType) {
    x2_syn(syn, b"a\\^b", b"a^b", 0, 3);
    x2_syn(syn, b"a^b", b"a^b", 0, 3);
    x2_syn(syn, b"a\\$b", b"a$b", 0, 3);
    x2_syn(syn, b"a$b", b"a$b", 0, 3);

    x2_syn(syn, b"^ab", b"ab", 0, 2);
    x2_syn(syn, b"(^ab)", b"(^ab)", 0, 5);
    x2_syn(syn, b"\\(^ab\\)", b"ab", 0, 2);
    x2_syn(syn, b"\\\\(^ab\\\\)", b"\\(^ab\\)", 0, 7);
    n_syn(syn, b"\\\\\\(^ab\\\\\\)", b"\\ab\\");
    x2_syn(syn, b"^\\\\\\(ab\\\\\\)", b"\\ab\\", 0, 4);

    x2_syn(syn, b"ab$", b"ab", 0, 2);
    x2_syn(syn, b"(ab$)", b"(ab$)", 0, 5);
    x2_syn(syn, b"\\(ab$\\)", b"ab", 0, 2);
    x2_syn(syn, b"\\\\(ab$\\\\)", b"\\(ab$\\)", 0, 7);
    n_syn(syn, b"\\\\\\(ab$\\\\\\)", b"\\ab\\");
    x2_syn(syn, b"\\\\\\(ab\\\\\\)$", b"\\ab\\", 0, 4);
}

// ============================================================================
// Perl syntax
// ============================================================================

#[test]
fn perl_reluctant_interval() {
    test_reluctant_interval(&OnigSyntaxPerl);
}

#[test]
fn perl_possessive_interval() {
    test_possessive_interval(&OnigSyntaxPerl);
}

#[test]
fn perl_isolated_option() {
    test_isolated_option(&OnigSyntaxPerl);
}

#[test]
fn perl_prec_read() {
    test_prec_read(&OnigSyntaxPerl);
}

#[test]
fn perl_look_behind() {
    test_look_behind(&OnigSyntaxPerl);
}

#[test]
fn perl_char_class() {
    test_char_class(&OnigSyntaxPerl);
}

#[test]
fn perl_var_lookbehind_error() {
    // Variable length lookbehind not implemented in Perl 5.26.1
    e_syn(
        &OnigSyntaxPerl,
        b"(?<=ab|(.))\\1",
        b"abb",
        ONIGERR_INVALID_LOOK_BEHIND_PATTERN,
    );
}

#[test]
fn perl_empty_group() {
    x3_syn(&OnigSyntaxPerl, b"()", b"abc", 0, 0, 1);
}

#[test]
fn perl_unmatched_paren() {
    e_syn(
        &OnigSyntaxPerl,
        b"(",
        b"",
        ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS,
    );
}

// ============================================================================
// Java syntax
// ============================================================================

#[test]
fn java_reluctant_interval() {
    test_reluctant_interval(&OnigSyntaxJava);
}

#[test]
fn java_possessive_interval() {
    test_possessive_interval(&OnigSyntaxJava);
}

#[test]
fn java_isolated_option() {
    test_isolated_option(&OnigSyntaxJava);
}

#[test]
fn java_prec_read() {
    test_prec_read(&OnigSyntaxJava);
}

#[test]
fn java_look_behind() {
    test_look_behind(&OnigSyntaxJava);
}

#[test]
fn java_char_class() {
    test_char_class(&OnigSyntaxJava);
}

#[test]
fn java_posix_bracket_literal() {
    // In Java syntax, [:digit:] is treated as literal character class
    n_syn(&OnigSyntaxJava, b"[[:digit:]]", b"1");
    x2_syn(&OnigSyntaxJava, b"[[:digit:]]", b"g", 0, 1);
}

#[test]
fn java_var_lookbehind_match() {
    x2_syn(&OnigSyntaxJava, b"(?<=ab|(.))\\1", b"abb", 2, 3);
}

#[test]
fn java_neg_lookbehind_alternation() {
    n_syn(&OnigSyntaxJava, b"(?<!ab|b)c", b"bbc");
    n_syn(&OnigSyntaxJava, b"(?<!b|ab)c", b"bbc");
}

// ============================================================================
// Python syntax
// ============================================================================

#[test]
fn python_reluctant_interval() {
    test_reluctant_interval(&OnigSyntaxPython);
}

#[test]
fn python_option_ascii() {
    test_python_option_ascii(&OnigSyntaxPython);
}

#[test]
fn python_z_anchor() {
    test_python_z(&OnigSyntaxPython);
}

#[test]
fn python_single_multi() {
    test_python_single_multi(&OnigSyntaxPython);
}

#[test]
fn python_posix_bracket_literal() {
    // In Python syntax, [:digit:] is literal
    n_syn(&OnigSyntaxPython, b"[[:digit:]]", b"1");
    x2_syn(&OnigSyntaxPython, b"[[:digit:]]", b"g]", 0, 2);
}

#[test]
fn python_named_group() {
    x2_syn(&OnigSyntaxPython, b"(?P<name>abc)", b"abc", 0, 3);
}

#[test]
fn python_named_backref() {
    x2_syn(
        &OnigSyntaxPython,
        b"(?P<name>abc)(?P=name)",
        b"abcabc",
        0,
        6,
    );
}

#[test]
fn python_named_call() {
    x2_syn(
        &OnigSyntaxPython,
        b"(?P<name>abc){0}(?P>name)",
        b"abc",
        0,
        3,
    );
}

#[test]
fn python_recursive_named_call() {
    x2_syn(
        &OnigSyntaxPython,
        b"(?P<expr>[^()]+|\\((?P>expr)\\)){0}(?P>expr)",
        b"((((xyz))))",
        0,
        11,
    );
}

#[test]
fn python_unicode_escape_u() {
    x2_syn(&OnigSyntaxPython, b"\\u0041", b"A", 0, 1);
}

#[test]
fn python_unicode_escape_big_u() {
    x2_syn(&OnigSyntaxPython, b"\\U00000041", b"A", 0, 1);
}

#[test]
fn python_unicode_escape_invalid() {
    e_syn(
        &OnigSyntaxPython,
        b"\\U0041",
        b"A",
        ONIGERR_INVALID_CODE_POINT_VALUE,
    );
}

// ============================================================================
// POSIX Basic syntax
// ============================================================================

#[test]
fn posix_basic_bre_anchors() {
    test_bre_anchors(&OnigSyntaxPosixBasic);
}

// ============================================================================
// Grep syntax
// ============================================================================

#[test]
fn grep_bre_anchors() {
    test_bre_anchors(&OnigSyntaxGrep);
}

#[test]
fn grep_alternation() {
    x2_syn(&OnigSyntaxGrep, b"zz\\|^ab", b"ab", 0, 2);
    x2_syn(&OnigSyntaxGrep, b"ab$\\|zz", b"ab", 0, 2);
}

#[test]
fn grep_literal_star() {
    x2_syn(&OnigSyntaxGrep, b"*", b"*", 0, 1);
    x2_syn(&OnigSyntaxGrep, b"^*", b"*", 0, 1);
}

#[test]
fn grep_literal_question_mark() {
    x2_syn(&OnigSyntaxGrep, b"abc\\|?", b"?", 0, 1);
}

#[test]
fn grep_literal_braces() {
    x2_syn(&OnigSyntaxGrep, b"\\{1\\}", b"{1}", 0, 3);
    x2_syn(&OnigSyntaxGrep, b"^\\{1\\}", b"{1}", 0, 3);
    x2_syn(&OnigSyntaxGrep, b"\\(\\{1\\}\\)", b"{1}", 0, 3);
    x2_syn(&OnigSyntaxGrep, b"^\\(\\{1\\}\\)", b"{1}", 0, 3);
}

#[test]
fn grep_bare_braces_literal() {
    x2_syn(&OnigSyntaxGrep, b"{1}", b"{1}", 0, 3);
    x2_syn(&OnigSyntaxGrep, b"^{1}", b"{1}", 0, 3);
    x2_syn(&OnigSyntaxGrep, b"\\({1}\\)", b"{1}", 0, 3);
    x2_syn(&OnigSyntaxGrep, b"^\\({1}\\)", b"{1}", 0, 3);
}

#[test]
fn grep_brace_range_literal() {
    x2_syn(&OnigSyntaxGrep, b"{1,2}", b"{1,2}", 0, 5);
    x2_syn(&OnigSyntaxGrep, b"^{1,2}", b"{1,2}", 0, 5);
    x2_syn(&OnigSyntaxGrep, b"\\({1,2}\\)", b"{1,2}", 0, 5);
    x2_syn(&OnigSyntaxGrep, b"^\\({1,2}\\)", b"{1,2}", 0, 5);
}

// ============================================================================
// Emacs syntax
// ============================================================================

#[test]
fn emacs_group() {
    x2_syn(&OnigSyntaxEmacs, b"\\(abc\\)", b"abc", 0, 3);
}

#[test]
fn emacs_shy_group() {
    x2_syn(&OnigSyntaxEmacs, b"\\(?:abc\\)", b"abc", 0, 3);
}

#[test]
fn emacs_shy_group_capture() {
    x3_syn(
        &OnigSyntaxEmacs,
        b"\\(?:abc\\)\\(xyz\\)",
        b"abcxyz",
        3,
        6,
        1,
    );
}

// ============================================================================
// Perl_NG syntax
// ============================================================================

#[test]
fn perl_ng_case_insensitive() {
    x2_syn(&OnigSyntaxPerl_NG, b"(?i)test", b"test", 0, 4);
    x2_syn(&OnigSyntaxPerl_NG, b"(?-i)test", b"test", 0, 4);
    x2_syn(&OnigSyntaxPerl_NG, b"(?i)test", b"TEST", 0, 4);
    n_syn(&OnigSyntaxPerl_NG, b"(?-i)test", b"teSt");
    x2_syn(&OnigSyntaxPerl_NG, b"(?i)te(?-i)st", b"TEst", 0, 4);
    n_syn(&OnigSyntaxPerl_NG, b"(?i)te(?-i)st", b"TesT");
}

#[test]
fn perl_ng_relative_call() {
    x2_syn(&OnigSyntaxPerl_NG, b"(abc)(?-1)", b"abcabc", 0, 6);
    x2_syn(&OnigSyntaxPerl_NG, b"(?+1)(abc)", b"abcabc", 0, 6);
    x2_syn(&OnigSyntaxPerl_NG, b"(abc)(?1)", b"abcabc", 0, 6);
}
