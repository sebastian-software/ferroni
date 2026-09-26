#![no_main]

use ferroni::api::Regex;
use ferroni::match_cache::MatchCacheConfig;
use ferroni::oniguruma::{ONIG_MISMATCH, ONIG_OPTION_NONE, OnigRegion};
use ferroni::regexec::{onig_new_match_param, onig_search_with_param};
use libfuzzer_sys::fuzz_target;

// Two-byte pattern length, UTF-8 pattern, arbitrary subject bytes. Keep both
// searches bounded without a match-stack limit (which disables memoization).
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > 514 {
        return;
    }
    let len = u16::from_le_bytes([data[0], data[1]]) as usize;
    if len > 256 || len > data.len() - 2 {
        return;
    }
    let (pattern, text) = data[2..].split_at(len);
    let Ok(pattern) = std::str::from_utf8(pattern) else {
        return;
    };
    let Ok(plain) = Regex::new(pattern) else {
        return;
    };
    let cached = Regex::builder(pattern)
        .match_cache(
            MatchCacheConfig::new()
                .memory_budget(1024 * 1024)
                .activation_threshold(0),
        )
        .build()
        .unwrap();
    let mut limits = onig_new_match_param();
    limits.retry_limit_in_match = 10_000;
    limits.retry_limit_in_search = 10_000;
    let snapshot = |re: &Regex| {
        let (result, region) = onig_search_with_param(
            re.as_raw(),
            text,
            text.len(),
            0,
            text.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
            &limits,
        );
        let bounds = if result >= 0 {
            let region = region.unwrap();
            region
                .beg
                .into_iter()
                .zip(region.end)
                .take(region.num_regs as usize)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        (result, bounds)
    };
    // Memoization intentionally changes how much work reaches a retry limit.
    // Compare answers whenever both searches finish instead of timing out.
    let plain = snapshot(&plain);
    let cached = snapshot(&cached);
    if plain.0 >= ONIG_MISMATCH && cached.0 >= ONIG_MISMATCH {
        assert_eq!(plain, cached, "{pattern:?} {text:?}");
    }
});
