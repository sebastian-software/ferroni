// compat_regset.rs — tests ported from oniguruma/test/test_regset.c
//
// Tests the RegSet API: multi-regex simultaneous search with both
// position-lead and regex-lead modes.

use ferroni::oniguruma::*;
use ferroni::regcomp::onig_new;
use ferroni::regint::RegexType;
use ferroni::regset::{
    onig_regset_get_region, onig_regset_new, onig_regset_search, OnigRegSet, OnigRegSetLead,
};
use ferroni::regsyntax::OnigSyntaxOniguruma;
use ferroni::scanner::{Scanner, ScannerFindOptions};

fn compile(pattern: &[u8]) -> Box<RegexType> {
    let reg = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
    );
    match reg {
        Ok(r) => Box::new(r),
        Err(e) => panic!(
            "compile failed for {:?}: error {}",
            std::str::from_utf8(pattern).unwrap_or("<?>"),
            e
        ),
    }
}

fn make_regset(patterns: &[&[u8]]) -> Box<OnigRegSet> {
    let regs: Vec<Box<RegexType>> = patterns.iter().map(|p| compile(p)).collect();
    let (set, r) = onig_regset_new(regs);
    assert_eq!(r, ONIG_NORMAL, "onig_regset_new failed: {}", r);
    set.unwrap()
}

/// x2: expect match at [from, to] for group 0
fn x2(set: &mut Box<OnigRegSet>, input: &[u8], lead: OnigRegSetLead, from: i32, to: i32) {
    let (idx, _pos) = onig_regset_search(
        set,
        input,
        input.len(),
        0,
        input.len(),
        lead,
        ONIG_OPTION_NONE,
    );
    assert!(
        idx >= 0,
        "x2: expected match, got index {} for input {:?}",
        idx,
        std::str::from_utf8(input).unwrap_or("<?>")
    );
    let region = onig_regset_get_region(set, idx as usize).unwrap();
    assert_eq!(
        region.beg[0],
        from,
        "x2: beg mismatch for input {:?}: expected {}, got {}",
        std::str::from_utf8(input).unwrap_or("<?>"),
        from,
        region.beg[0]
    );
    assert_eq!(
        region.end[0],
        to,
        "x2: end mismatch for input {:?}: expected {}, got {}",
        std::str::from_utf8(input).unwrap_or("<?>"),
        to,
        region.end[0]
    );
}

/// x3: expect match at [from, to] for capture group `mem`
fn x3(
    set: &mut Box<OnigRegSet>,
    input: &[u8],
    lead: OnigRegSetLead,
    from: i32,
    to: i32,
    mem: usize,
) {
    let (idx, _pos) = onig_regset_search(
        set,
        input,
        input.len(),
        0,
        input.len(),
        lead,
        ONIG_OPTION_NONE,
    );
    assert!(
        idx >= 0,
        "x3: expected match, got index {} for input {:?}",
        idx,
        std::str::from_utf8(input).unwrap_or("<?>")
    );
    let region = onig_regset_get_region(set, idx as usize).unwrap();
    assert_eq!(
        region.beg[mem],
        from,
        "x3: beg[{}] mismatch for input {:?}: expected {}, got {}",
        mem,
        std::str::from_utf8(input).unwrap_or("<?>"),
        from,
        region.beg[mem]
    );
    assert_eq!(
        region.end[mem],
        to,
        "x3: end[{}] mismatch for input {:?}: expected {}, got {}",
        mem,
        std::str::from_utf8(input).unwrap_or("<?>"),
        to,
        region.end[mem]
    );
}

/// n: expect no match
fn n_search(set: &mut Box<OnigRegSet>, input: &[u8], lead: OnigRegSetLead) {
    let (idx, _pos) = onig_regset_search(
        set,
        input,
        input.len(),
        0,
        input.len(),
        lead,
        ONIG_OPTION_NONE,
    );
    assert_eq!(
        idx,
        ONIG_MISMATCH,
        "n: expected no match, got index {} for input {:?}",
        idx,
        std::str::from_utf8(input).unwrap_or("<?>")
    );
}

/// nzero: search with empty set, expect no match
fn nzero(input: &[u8], lead: OnigRegSetLead) {
    let mut set = make_regset(&[]);
    n_search(&mut set, input, lead);
}

// ============================================================================
// Position-lead tests (C: XX_LEAD = ONIG_REGSET_POSITION_LEAD)
// ============================================================================

#[test]
fn pos_lead_empty_set() {
    nzero(b" abab bccab ca", OnigRegSetLead::PositionLead);
}

#[test]
fn pos_lead_p1_x2() {
    let mut set = make_regset(&[b"abc", b"(bca)", b"(cab)"]);
    x2(
        &mut set,
        b" abab bccab ca",
        OnigRegSetLead::PositionLead,
        8,
        11,
    );
}

#[test]
fn pos_lead_p1_x3() {
    let mut set = make_regset(&[b"abc", b"(bca)", b"(cab)"]);
    x3(
        &mut set,
        b" abab bccab ca",
        OnigRegSetLead::PositionLead,
        8,
        11,
        1,
    );
}

#[test]
fn pos_lead_p2_no_match() {
    let mut set = make_regset(&["小説".as_bytes(), b"9", "夏目漱石".as_bytes()]);
    n_search(
        &mut set,
        b" XXXX AAA 1223 012345678bbb",
        OnigRegSetLead::PositionLead,
    );
}

#[test]
fn pos_lead_p2_digit() {
    let mut set = make_regset(&["小説".as_bytes(), b"9", "夏目漱石".as_bytes()]);
    x2(&mut set, b"0123456789", OnigRegSetLead::PositionLead, 9, 10);
}

#[test]
fn pos_lead_p7_digits() {
    let mut set = make_regset(&[
        b"0+", b"1+", b"2+", b"3+", b"4+", b"5+", b"6+", b"7+", b"8+", b"9+",
    ]);
    x2(
        &mut set,
        b"abcde 555 qwert",
        OnigRegSetLead::PositionLead,
        6,
        9,
    );
}

#[test]
fn pos_lead_p8_empty_string() {
    let mut set = make_regset(&[b"a", b".*"]);
    x2(&mut set, b"", OnigRegSetLead::PositionLead, 0, 0);
}

// ============================================================================
// Regex-lead tests (C: XX_LEAD = ONIG_REGSET_REGEX_LEAD)
// ============================================================================

#[test]
fn reg_lead_empty_set() {
    nzero(b" abab bccab ca", OnigRegSetLead::RegexLead);
}

#[test]
fn reg_lead_p1_x2() {
    let mut set = make_regset(&[b"abc", b"(bca)", b"(cab)"]);
    x2(
        &mut set,
        b" abab bccab ca",
        OnigRegSetLead::RegexLead,
        8,
        11,
    );
}

#[test]
fn reg_lead_p1_x3() {
    let mut set = make_regset(&[b"abc", b"(bca)", b"(cab)"]);
    x3(
        &mut set,
        b" abab bccab ca",
        OnigRegSetLead::RegexLead,
        8,
        11,
        1,
    );
}

#[test]
fn reg_lead_p2_no_match() {
    let mut set = make_regset(&["小説".as_bytes(), b"9", "夏目漱石".as_bytes()]);
    n_search(
        &mut set,
        b" XXXX AAA 1223 012345678bbb",
        OnigRegSetLead::RegexLead,
    );
}

#[test]
fn reg_lead_p2_digit() {
    let mut set = make_regset(&["小説".as_bytes(), b"9", "夏目漱石".as_bytes()]);
    x2(&mut set, b"0123456789", OnigRegSetLead::RegexLead, 9, 10);
}

#[test]
fn reg_lead_p7_digits() {
    let mut set = make_regset(&[
        b"0+", b"1+", b"2+", b"3+", b"4+", b"5+", b"6+", b"7+", b"8+", b"9+",
    ]);
    x2(
        &mut set,
        b"abcde 555 qwert",
        OnigRegSetLead::RegexLead,
        6,
        9,
    );
}

#[test]
fn scanner_cache_id_small_input_progression() {
    let mut scanner = Scanner::new(&[";", "}"]).expect("scanner");
    let input = "a;b}";

    let m1 = scanner
        .find_next_match_with_id(input, 1, 0, ScannerFindOptions::NONE)
        .expect("first match");
    assert_eq!(m1.index, 0);
    assert_eq!(m1.capture_indices[0].start, 1);
    assert_eq!(m1.capture_indices[0].end, 2);

    let m2 = scanner
        .find_next_match_with_id(input, 1, 2, ScannerFindOptions::NONE)
        .expect("second match");
    assert_eq!(m2.index, 1);
    assert_eq!(m2.capture_indices[0].start, 3);
    assert_eq!(m2.capture_indices[0].end, 4);
}

#[test]
fn scanner_cache_id_small_input_no_match() {
    let mut scanner = Scanner::new(&[";", "}"]).expect("scanner");
    let input = "abc";

    let m = scanner.find_next_match_with_id(input, 7, 0, ScannerFindOptions::NONE);
    assert!(m.is_none());
}

#[test]
fn scanner_optional_prefix_exact_matches_from_the_match_start() {
    let mut scanner = Scanner::new(&["a?bc"]).expect("scanner");

    let m = scanner
        .find_next_match("xabc", 0, ScannerFindOptions::NONE)
        .expect("match");

    assert_eq!(m.index, 0);
    assert_eq!(m.capture_indices[0].start, 1);
    assert_eq!(m.capture_indices[0].end, 4);
}

#[test]
fn scanner_optional_prefix_map_matches_from_the_match_start() {
    let mut scanner = Scanner::new(&["x?[abc]"]).expect("scanner");

    let m = scanner
        .find_next_match("qxa", 0, ScannerFindOptions::NONE)
        .expect("match");

    assert_eq!(m.index, 0);
    assert_eq!(m.capture_indices[0].start, 1);
    assert_eq!(m.capture_indices[0].end, 3);
}

#[test]
fn regset_optional_prefix_wins_at_the_earliest_position() {
    let mut set = make_regset(&[b"a?b", br"\s", b"[abc]", b"a|b"]);
    let input = b"abc";

    let (idx, pos) = onig_regset_search(
        &mut set,
        input,
        input.len(),
        0,
        input.len(),
        OnigRegSetLead::PositionLead,
        ONIG_OPTION_NONE,
    );

    assert_eq!(idx, 0);
    assert_eq!(pos, 0);
    let region = onig_regset_get_region(&set, idx as usize).expect("winning region");
    assert_eq!(region.beg[0], 0);
    assert_eq!(region.end[0], 2);
}

#[test]
fn regset_fixed_first_byte_pattern_wins_fallback_ties_by_order() {
    let mut set = make_regset(&[b"a", b"a?b"]);
    let input = b"ab";

    let (idx, pos) = onig_regset_search(
        &mut set,
        input,
        input.len(),
        0,
        input.len(),
        OnigRegSetLead::PositionLead,
        ONIG_OPTION_NONE,
    );

    assert_eq!(idx, 0);
    assert_eq!(pos, 0);
    let region = onig_regset_get_region(&set, idx as usize).expect("winning region");
    assert_eq!(region.beg[0], 0);
    assert_eq!(region.end[0], 1);
}

#[test]
fn scanner_repeated_optional_prefix_match_agrees_across_routes() {
    let mut scanner = Scanner::new(&["a?bc", "q"]).expect("scanner");
    let input = "qabc";

    for _ in 0..25 {
        let matched = scanner
            .find_next_match_with_id(input, 55, 1, ScannerFindOptions::NONE)
            .expect("match");
        assert_eq!(matched.index, 0);
        assert_eq!(matched.capture_indices[0].start, 1);
        assert_eq!(matched.capture_indices[0].end, 4);
    }
}

#[test]
fn large_regset_variable_map_finds_the_true_earliest_start() {
    let mut patterns: Vec<&[u8]> = vec![b"a?[bcd]", b"x"];
    patterns.resize(33, b"(?!)");
    let mut set = make_regset(&patterns);
    let input = b"qac";

    let (index, position) = onig_regset_search(
        &mut set,
        input,
        input.len(),
        0,
        input.len(),
        OnigRegSetLead::PositionLead,
        ONIG_OPTION_NONE,
    );

    assert_eq!((index, position), (0, 1));
    let region = onig_regset_get_region(&set, index as usize).expect("winning region");
    assert_eq!((region.beg[0], region.end[0]), (1, 3));
}

#[test]
fn large_regset_merges_variable_and_table_ties_by_regex_order() {
    let mut patterns: Vec<&[u8]> = vec![b"a", b"a?[bcd]"];
    patterns.resize(33, b"(?!)");
    let mut set = make_regset(&patterns);
    let input = b"ab";

    let (index, position) = onig_regset_search(
        &mut set,
        input,
        input.len(),
        0,
        input.len(),
        OnigRegSetLead::PositionLead,
        ONIG_OPTION_NONE,
    );

    assert_eq!((index, position), (0, 0));
    let region = onig_regset_get_region(&set, index as usize).expect("winning region");
    assert_eq!((region.beg[0], region.end[0]), (0, 1));
}
