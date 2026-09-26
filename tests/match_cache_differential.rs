#![cfg(all(feature = "match-cache", feature = "ffi"))]

use ferroni::api::Regex;
use ferroni::ffi::{CRegex, CRegion};
use ferroni::match_cache::MatchCacheConfig;
use ferroni::oniguruma::*;
use ferroni::regexec::onig_search;

fn snapshot(
    re: &Regex,
    text: &[u8],
    start: usize,
    range: usize,
    option: OnigOptionType,
) -> (i32, Vec<(i32, i32)>) {
    let (result, region) = onig_search(
        re.as_raw(),
        text,
        text.len(),
        start,
        range,
        Some(OnigRegion::new()),
        option,
    );
    let captures = if result >= 0 {
        let r = region.unwrap();
        r.beg
            .into_iter()
            .zip(r.end)
            .take(r.num_regs as usize)
            .collect()
    } else {
        Vec::new()
    };
    (result, captures)
}

#[test]
fn seeded_cache_plain_and_c_comparison() {
    let atoms = ["a", "[ab]", r"\w", "é", "(?:a|ab)", "[^,]"];
    let mut patterns = Vec::new();
    for atom in atoms {
        for quant in ["*", "+", "?", "*?", "+?", "{1,2}"] {
            for template in [
                format!("({atom}{quant})b"),
                format!("({atom}{quant})+$"),
                format!("({atom}{quant})a|b"),
                format!("(?:{atom}{quant},)*x"),
            ] {
                patterns.push(template);
            }
        }
    }
    patterns.extend(
        [
            r"(a(b)?)+c|a+",
            r"((ab|a)+)(b?)",
            r"(a|aa)*$",
            r"(.*)(a|b)$",
            r"(?i)(a|ä|ss)+$",
            r"(?:\w+\s*,\s*)*\w+\s*$",
            r"(?=a)a",
            r"(a+)\1",
            r"(?>a*)a",
            r"(?>(?:ab)*)(a?)",
            r"(?>a*b*)c",
            r"(?>a*)b|a*",
            r"(?<=a)b",
            r"[ab]?[ab]{3}",
            r"([ab]{2})+[ab]{2}x",
            r"([ab]{3}|a)[ab]{2}",
            r"(?<=[ab])[ab]{3}",
            r"(\p{L}[\p{L}\p{M}]*)+!",
            r"([^\p{L}]+)(\p{L}*)",
            r"(?<=\p{L})([a\p{M}]*)",
            r"[ab]+c",
            r"[ab]+,(a?)",
            r"[ab]+,\K(a?)",
            r"[ab]+c(?<!ac)",
        ]
        .map(str::to_owned),
    );
    let mut seed = 0x163_feff_u64;
    let alphabet = ["a", "b", "c", "!", ",", " ", "\n", "é", "😀"];
    let mut texts = vec![
        String::new(),
        "aaaaa!".into(),
        "aaabaaab".into(),
        "é😀é".into(),
    ];
    for _ in 0..192 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let len = (seed >> 32) as usize % 13;
        let mut text = String::new();
        for _ in 0..len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(alphabet[(seed >> 32) as usize % alphabet.len()]);
        }
        texts.push(text);
    }
    let mut comparisons = 0;
    let mut eligible = 0;
    let mut c_comparisons = 0;
    for pattern in patterns {
        let plain = Regex::new(&pattern).unwrap();
        let cached = Regex::builder(&pattern)
            .match_cache(MatchCacheConfig::new().activation_threshold(0))
            .build()
            .unwrap();
        eligible += usize::from(cached.is_linear_time());
        let c = CRegex::new(pattern.as_bytes(), 0).unwrap();
        let mut c_region = CRegion::new();
        for text in &texts {
            // The raw Rust API accepts byte offsets, including offsets inside
            // a UTF-8 character. Cache/plain equality covers those too.
            for start in 0..=text.len() {
                for range in [text.len(), start, 0] {
                    for option in [ONIG_OPTION_NONE, ONIG_OPTION_FIND_NOT_EMPTY] {
                        let plain_result = snapshot(&plain, text.as_bytes(), start, range, option);
                        let cached_result =
                            snapshot(&cached, text.as_bytes(), start, range, option);
                        assert_eq!(
                            cached_result, plain_result,
                            "cache: {pattern:?} {text:?} start={start} range={range} {option:?}"
                        );
                        c_region.clear();
                        let result = c.search(
                            text.as_bytes(),
                            start,
                            range,
                            Some(&mut c_region),
                            option.bits(),
                        );
                        let c_result = (
                            result,
                            if result >= 0 {
                                c_region.capture_ranges()
                            } else {
                                Vec::new()
                            },
                        );
                        // The baseline and C already differ on some equal-
                        // endpoint/backward searches (e.g. ([ab]{1,2})b on
                        // "bbb", start=range=1). Those still require exact
                        // cache/plain equality above. C is the independent
                        // oracle for full-range forward searches here.
                        if start < range && range == text.len() && text.is_char_boundary(start) {
                            c_comparisons += 1;
                            assert_eq!(
                                cached_result, c_result,
                                "C: {pattern:?} {text:?} start={start} range={range} {option:?}"
                            );
                        }
                        comparisons += 1;
                    }
                }
            }
        }
    }
    assert!(
        eligible > 50,
        "only {eligible} patterns exercised memoization"
    );
    eprintln!(
        "{comparisons} cache/plain comparisons, {c_comparisons} forward comparisons with C; {eligible} eligible patterns"
    );
}
