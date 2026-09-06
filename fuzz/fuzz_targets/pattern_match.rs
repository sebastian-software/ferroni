#![no_main]

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::oniguruma::{ONIG_MISMATCH, ONIG_OPTION_NONE, OnigRegion};
use ferroni::regcomp::onig_new;
use ferroni::regexec::{
    onig_new_match_param, onig_search_with_param, onig_set_match_stack_limit_size_of_match_param,
    onig_set_retry_limit_in_match_of_match_param, onig_set_retry_limit_in_search_of_match_param,
};
use ferroni::regsyntax::OnigSyntaxOniguruma;
use libfuzzer_sys::fuzz_target;

const MAX_PATTERN_BYTES: usize = 1024;
const MAX_HAYSTACK_BYTES: usize = 16 * 1024;

// A per-call step budget. Catastrophic backtracking is a property of the
// pattern, not a bug in the engine, so the budget keeps the fuzzer looking for
// crashes instead of timing out on a pattern like `(a+)+$`. These limits are
// per match parameter, so nothing is shared between fuzzer threads.
const RETRY_LIMIT_IN_MATCH: u64 = 100_000;
const RETRY_LIMIT_IN_SEARCH: u64 = 100_000;
const MATCH_STACK_LIMIT: u32 = 8192;

// Searching an arbitrary pattern over arbitrary bytes must either report a
// match inside the haystack or report a mismatch or error; it must never
// panic, and never return an out-of-range position. The input is split as
// a one-byte pattern length prefix plus that many pattern bytes, so the two
// halves mutate independently.
fuzz_target!(|data: &[u8]| {
    if data.len() < 3 || data.len() > MAX_PATTERN_BYTES + MAX_HAYSTACK_BYTES + 2 {
        return;
    }

    let pattern_len = u16::from_le_bytes([data[0], data[1]]) as usize;
    let body = &data[2..];
    if pattern_len > body.len() || pattern_len > MAX_PATTERN_BYTES {
        return;
    }
    let (pattern, haystack) = body.split_at(pattern_len);
    if haystack.len() > MAX_HAYSTACK_BYTES {
        return;
    }

    let Ok(reg) = onig_new(
        pattern,
        ONIG_OPTION_NONE,
        &ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
    ) else {
        return;
    };

    let mut match_param = onig_new_match_param();
    onig_set_retry_limit_in_match_of_match_param(&mut match_param, RETRY_LIMIT_IN_MATCH);
    onig_set_retry_limit_in_search_of_match_param(&mut match_param, RETRY_LIMIT_IN_SEARCH);
    onig_set_match_stack_limit_size_of_match_param(&mut match_param, MATCH_STACK_LIMIT);

    let (position, region) = onig_search_with_param(
        &reg,
        haystack,
        haystack.len(),
        0,
        haystack.len(),
        Some(OnigRegion::new()),
        ONIG_OPTION_NONE,
        &match_param,
    );

    if position < 0 {
        // ONIG_MISMATCH and the negative error codes below it are all fine;
        // hitting the step budget reports one of them.
        assert!(position <= ONIG_MISMATCH, "unexpected result {position}");
        return;
    }

    let position = position as usize;
    assert!(
        position <= haystack.len(),
        "match at {position} is past the end of a {} byte haystack",
        haystack.len()
    );

    let Some(region) = region else {
        return;
    };
    for group in 0..region.num_regs as usize {
        let (begin, end) = (region.beg[group], region.end[group]);
        // An unset group reports -1 for both ends.
        if begin < 0 || end < 0 {
            continue;
        }
        let (begin, end) = (begin as usize, end as usize);
        assert!(
            begin <= end && end <= haystack.len(),
            "group {group} spans {begin}..{end} of a {} byte haystack",
            haystack.len()
        );
    }
});
