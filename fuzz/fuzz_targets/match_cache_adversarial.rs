#![no_main]

use ferroni::api::Regex;
#[cfg(feature = "c-oracle")]
use ferroni::ffi::{CRegex, CRegion};
use ferroni::match_cache::MatchCacheConfig;
use ferroni::oniguruma::{
    ONIG_MISMATCH, ONIG_OPTION_FIND_LONGEST, ONIG_OPTION_FIND_NOT_EMPTY, ONIG_OPTION_NONE,
    OnigOptionType, OnigRegion,
};
use ferroni::regexec::{onig_new_match_param, onig_search_with_param};
use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions};
use libfuzzer_sys::fuzz_target;

// Each input picks a known control-flow shape, search direction, runtime mode,
// cache policy, and subject. This reliably enters eligible cache paths while
// still mutating the bytes that drive backtracking and capture boundaries.
const PATTERNS: &[&str] = &[
    r"(a+)+$",
    r"(a|aa)*$",
    r"((ab|a)+)(b?)",
    r"(?:\w*,)*x",
    r"(?:\w+\s*,\s*)*\w+\s*$",
    r"([ab]{3}|a)[ab]{2}",
    r"(?i)(a|ä|ss)+$",
    r"(.*)(a|b)$",
    r"(?>a*)b|a*",
    r"(a+)\1",
    r"(?<=a)b",
    r"ab\K(c?)",
];

fn snapshot(
    regex: &Regex,
    text: &[u8],
    start: usize,
    range: usize,
    option: OnigOptionType,
) -> (i32, Vec<(i32, i32)>) {
    let mut limits = onig_new_match_param();
    limits.retry_limit_in_match = 20_000;
    limits.retry_limit_in_search = 20_000;
    let (result, region) = onig_search_with_param(
        regex.as_raw(),
        text,
        text.len(),
        start,
        range,
        Some(OnigRegion::new()),
        option,
        &limits,
    );
    let captures = if result >= 0 {
        let region = region.unwrap();
        region
            .beg
            .into_iter()
            .zip(region.end)
            .take(region.num_regs as usize)
            .collect()
    } else {
        Vec::new()
    };
    (result, captures)
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 || data.len() > 70 {
        return;
    }
    let pattern = PATTERNS[data[0] as usize % PATTERNS.len()];
    let mode = data[1];
    let mut generated = Vec::new();
    let text = if mode & 1 == 0 {
        // Keep most generated subjects valid UTF-8 and rich in overlapping
        // alternatives. Odd modes retain raw bytes to test cache fallback.
        for &byte in &data[5..] {
            generated.extend_from_slice(
                [
                    b"a",
                    b"b",
                    b"c",
                    b"x",
                    b"!",
                    b",",
                    b" ",
                    b"\n",
                    "ä".as_bytes(),
                ][byte as usize % 9],
            );
        }
        generated.as_slice()
    } else {
        &data[5..]
    };
    let start = data[2] as usize % (text.len() + 1);
    let range = if mode & 0x10 == 0 {
        text.len()
    } else {
        data[3] as usize % (text.len() + 1)
    };
    let option = match (mode >> 2) & 3 {
        1 => ONIG_OPTION_FIND_NOT_EMPTY,
        2 => ONIG_OPTION_FIND_LONGEST,
        _ => ONIG_OPTION_NONE,
    };
    let budget = [0, 64, 1024, 1024 * 1024][data[4] as usize % 4];
    let mut config = MatchCacheConfig::new().memory_budget(budget);
    if mode & 0x20 == 0 {
        config = config.activation_threshold(0);
    }

    let plain = Regex::new(pattern).unwrap();
    let cached = Regex::builder(pattern).match_cache(config).build().unwrap();
    let plain_result = snapshot(&plain, text, start, range, option);
    let cached_result = snapshot(&cached, text, start, range, option);
    // A cached search can finish where the plain matcher exhausts its retry
    // budget. Completed answers and capture bounds must always agree.
    if plain_result.0 >= ONIG_MISMATCH && cached_result.0 >= ONIG_MISMATCH {
        assert_eq!(
            cached_result, plain_result,
            "{pattern:?} {text:?} {start} {range} {option:?} {budget}"
        );
    }

    #[cfg(feature = "c-oracle")]
    if text.len() <= 16
        && option == ONIG_OPTION_NONE
        && start < range
        && range == text.len()
        && std::str::from_utf8(text).is_ok_and(|s| s.is_char_boundary(start))
        && plain_result.0 >= ONIG_MISMATCH
    {
        let c = CRegex::new(pattern.as_bytes(), 0).unwrap();
        let mut region = CRegion::new();
        let result = c.search(text, start, range, Some(&mut region), option.bits());
        let captures = if result >= 0 {
            region.capture_ranges()
        } else {
            Vec::new()
        };
        assert_eq!(
            plain_result,
            (result, captures),
            "C oracle: {pattern:?} {text:?} {start}"
        );
    }

    if mode & 0x80 != 0
        && text.len() <= 16
        && let Ok(text) = std::str::from_utf8(text)
    {
        let patterns = [pattern, "!", r"(\w+)"];
        let mut plain = Scanner::new(&patterns).unwrap();
        let mut cached =
            Scanner::with_match_cache(&patterns, &ScannerConfig::default(), config).unwrap();
        let subject = OnigString::new(text);
        for position in [0, start, 0, range.min(text.len())] {
            let options = ScannerFindOptions::from_bits((mode as u32 >> 4) & 7);
            let expected = plain.find_next_match_utf16(&subject, position, options);
            let actual = cached.find_next_match_utf16(&subject, position, options);
            assert_eq!(actual, expected, "scanner: {pattern:?} {text:?} {position}");
            assert!(cached.match_cache_bytes() <= budget);
        }
        let changed = OnigString::new("b!");
        assert_eq!(
            cached.find_next_match_utf16(&changed, 0, ScannerFindOptions::NONE),
            plain.find_next_match_utf16(&changed, 0, ScannerFindOptions::NONE),
        );
        assert!(cached.match_cache_bytes() <= budget);
    }
});
