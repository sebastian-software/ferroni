//! Adversarial differential fuzzing of the opt-in match cache (issue #181).
//!
//! A grammar-based generator produces patterns biased toward the shapes the
//! cache treats specially (nested and alternated loops, possessive character
//! runs, anchors, captures, look-arounds, back-references, subexpression calls).
//! Each pattern is compiled uncached and with several cache configurations
//! (immediate activation, tiny shared budgets that force a mid-search
//! fallback, mid-search activation, and the adaptive default), then searched on
//! generated subjects. Uncached Ferroni is the reference for exact equality;
//! upstream C Oniguruma is the independent oracle wherever the existing
//! baseline is known to agree with it (forward, full-range, character-aligned
//! searches).
//!
//! Scale and seed are configurable so longer campaigns can run outside CI:
//!
//! ```sh
//! MATCH_CACHE_ADVERSARIAL_CASES=200000 MATCH_CACHE_ADVERSARIAL_SEED=7 \
//!   cargo test --release --features "match-cache ffi" --test match_cache_adversarial -- --nocapture
//! ```
#![cfg(all(feature = "match-cache", feature = "ffi"))]

use ferroni::api::Regex;
use ferroni::ffi::{CRegex, CRegion, CScanner};
use ferroni::match_cache::MatchCacheConfig;
use ferroni::oniguruma::*;
use ferroni::regexec::{OnigMatchParam, onig_new_match_param, onig_search_with_param};
use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

const ATOMS: &[&str] = &[
    "a", "b", "c", "ab", ",", " ", "é", "x", ".", r"\w", r"\W", r"\s", r"\d", "[ab]", "[^a]",
    "[a-c]", r"\p{L}", r"\h", "[^,]", r"\b", r"\B", "^", "$", r"\A", r"\z", r"\Z", r"\G", r"\K",
    "(?:)",
];
const QUANTS: &[&str] = &[
    "*", "+", "?", "*?", "+?", "??", "{2}", "{1,2}", "{0,3}", "{2,}", "{1,2}?", "*+", "++", "?+",
];

fn atom(rng: &mut Rng) -> String {
    rng.pick(ATOMS).to_string()
}

fn generate(rng: &mut Rng, depth: usize, groups: &mut usize) -> String {
    if depth == 0 {
        return atom(rng);
    }
    match rng.below(100) {
        0..=24 => atom(rng),
        25..=44 => {
            let n = 2 + rng.below(2);
            (0..n).map(|_| generate(rng, depth - 1, groups)).collect()
        }
        45..=56 => {
            let n = 2 + rng.below(2);
            (0..n)
                .map(|_| generate(rng, depth - 1, groups))
                .collect::<Vec<_>>()
                .join("|")
        }
        57..=79 => {
            // Loops around a group are what memoization targets.
            let inner = generate(rng, depth - 1, groups);
            let quant = *rng.pick(QUANTS);
            match rng.below(3) {
                0 => {
                    *groups += 1;
                    format!("({inner}){quant}")
                }
                _ => format!("(?:{inner}){quant}"),
            }
        }
        80..=90 => {
            let inner = generate(rng, depth - 1, groups);
            match rng.below(8) {
                0 => {
                    *groups += 1;
                    format!("({inner})")
                }
                1 => format!("(?>{inner})"),
                2 => format!("(?={inner})"),
                3 => format!("(?!{inner})"),
                4 => format!("(?<={})", rng.pick(&["a", "b|c", "é", "ab"])),
                5 => format!("(?<!{})", rng.pick(&["a", "b|c", "é", "ab"])),
                6 => format!("(?i:{inner})"),
                _ => format!("(?~{inner})"),
            }
        }
        91..=95 if *groups > 0 => format!(r"\{}", 1 + rng.below(*groups)),
        96..=97 if *groups > 0 => format!(r"(?(1){}|{})", atom(rng), atom(rng)),
        _ => atom(rng),
    }
}

fn pattern(rng: &mut Rng) -> String {
    let mut groups = 0;
    let depth = 1 + rng.below(4);
    let mut p = generate(rng, depth, &mut groups);
    match rng.below(6) {
        0 => p.push('$'),
        1 => p.push_str(",?x"),
        2 => p = format!("(?:{p})+$"),
        3 => p = format!("(?i){p}"),
        _ => {}
    }
    p
}

fn subject(rng: &mut Rng) -> Vec<u8> {
    let alphabet: &[&str] = &["a", "b", "c", "!", ",", " ", "\n", "é", "x", "1", "😀"];
    let mut s = match rng.below(5) {
        0 | 1 => (0..rng.below(14))
            .map(|_| *rng.pick(alphabet))
            .collect::<String>(),
        2 => format!(
            "{}{}",
            "a".repeat(rng.below(28)),
            rng.pick(&["!", "", "b", "$"])
        ),
        3 => {
            let words = ["ab", "a", ",", " ", "abc", "é"];
            (0..rng.below(9)).map(|_| *rng.pick(&words)).collect()
        }
        _ => format!(
            "{}{}",
            "ab".repeat(rng.below(9)),
            rng.pick(&["c", ",", "a", "!"])
        ),
    }
    .into_bytes();
    // A few subjects carry invalid UTF-8; the cache must disable itself.
    if rng.below(12) == 0 && !s.is_empty() {
        let at = rng.below(s.len());
        s[at] = *rng.pick(&[0xFF, 0x80, 0xC3]);
    }
    s
}

type Snapshot = (i32, Vec<(i32, i32)>);
type ScannerHit = (usize, Vec<(usize, usize)>);

fn snapshot_with(
    re: &Regex,
    text: &[u8],
    start: usize,
    range: usize,
    option: OnigOptionType,
    limits: &OnigMatchParam,
) -> Snapshot {
    let (result, region) = onig_search_with_param(
        re.as_raw(),
        text,
        text.len(),
        start,
        range,
        Some(OnigRegion::new()),
        option,
        limits,
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

fn is_limit_error(result: i32) -> bool {
    matches!(
        result,
        ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
            | ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER
            | ONIGERR_MATCH_STACK_LIMIT_OVER
            | ONIGERR_TIME_LIMIT_OVER
    )
}

fn cache_configs(rng: &mut Rng) -> Vec<(String, MatchCacheConfig)> {
    let tiny = *rng.pick(&[1usize, 8, 32, 100, 256, 1000, 4096]);
    let late = 1 + rng.below(40);
    vec![
        (
            "immediate".into(),
            MatchCacheConfig::new().activation_threshold(0),
        ),
        (
            format!("tiny-budget-{tiny}"),
            MatchCacheConfig::new()
                .memory_budget(tiny)
                .activation_threshold(0),
        ),
        (
            format!("late-{late}"),
            MatchCacheConfig::new().activation_threshold(late),
        ),
        ("adaptive".into(), MatchCacheConfig::new()),
        (
            "zero-budget".into(),
            MatchCacheConfig::new()
                .memory_budget(0)
                .activation_threshold(0),
        ),
    ]
}

#[derive(Default)]
struct Tally {
    patterns: usize,
    skipped_compile: usize,
    c_compile_mismatch: usize,
    eligible: usize,
    cache_plain: u64,
    c_compared: u64,
    c_limit_skipped: u64,
    limit_cases: u64,
    plain_c_divergences: Vec<String>,
}

#[test]
fn adversarial_cache_plain_c_comparison() {
    let cases = env_usize("MATCH_CACHE_ADVERSARIAL_CASES", 250);
    let seed = env_usize("MATCH_CACHE_ADVERSARIAL_SEED", 181) as u64;
    let mut rng = Rng(seed);
    let mut tally = Tally::default();

    for _ in 0..cases {
        let pat = pattern(&mut rng);
        let Ok(plain) = Regex::new(&pat) else {
            tally.skipped_compile += 1;
            continue;
        };
        let Ok(c) = CRegex::new(pat.as_bytes(), 0) else {
            tally.c_compile_mismatch += 1;
            continue;
        };
        tally.patterns += 1;
        let caches: Vec<_> = cache_configs(&mut rng)
            .into_iter()
            .map(|(name, config)| {
                let re = Regex::builder(&pat).match_cache(config).build().unwrap();
                (name, re)
            })
            .collect();
        tally.eligible += usize::from(caches[0].1.is_linear_time());
        let mut c_region = CRegion::new();

        for _ in 0..4 {
            let text = subject(&mut rng);
            let valid = std::str::from_utf8(&text).is_ok();
            let len = text.len();
            let mut probes = vec![(0, len), (len, 0), (len / 2, len)];
            for _ in 0..3 {
                let start = rng.below(len + 1);
                probes.push((start, *rng.pick(&[len, start, 0])));
            }
            for (start, range) in probes {
                for option in [
                    ONIG_OPTION_NONE,
                    ONIG_OPTION_FIND_NOT_EMPTY,
                    ONIG_OPTION_FIND_LONGEST,
                    ONIG_OPTION_NOTBOL,
                    ONIG_OPTION_NOTEOL,
                ] {
                    // Retry limits change how far a search gets; each variant
                    // still has to agree wherever no limit error is reported.
                    let limit_variants: [(u64, u64); 4] = [(0, 0), (4, 0), (16, 50), (64, 500)];
                    let (lm, ls) = *rng.pick(&limit_variants);
                    let mut limits = onig_new_match_param();
                    if lm != 0 {
                        limits.retry_limit_in_match = lm;
                        limits.retry_limit_in_search = ls;
                    }
                    let reference = snapshot_with(&plain, &text, start, range, option, &limits);

                    // Through this FFI path C reports a mismatch for every
                    // FIND_LONGEST search (cause not investigated; the cache
                    // rejects that option), so it cannot serve as an oracle.
                    let forward_oracle = valid
                        && option != ONIG_OPTION_FIND_LONGEST
                        && start < range
                        && range == len
                        && std::str::from_utf8(&text[..start]).is_ok()
                        && (start == len || (text[start] & 0xC0) != 0x80);
                    let c_result = forward_oracle.then(|| {
                        c_region.clear();
                        let r = c.search(&text, start, range, Some(&mut c_region), option.bits());
                        (
                            r,
                            if r >= 0 {
                                c_region.capture_ranges()
                            } else {
                                Vec::new()
                            },
                        )
                    });
                    let c_usable = c_result.as_ref().filter(|(r, _)| !is_limit_error(*r));
                    if forward_oracle && c_usable.is_none() {
                        tally.c_limit_skipped += 1;
                    }
                    if let Some(truth) = c_usable {
                        if !is_limit_error(reference.0) && &reference != truth {
                            tally.plain_c_divergences.push(format!(
                                "{pat:?} {text:?} start={start} range={range} {option:?} plain={reference:?} c={truth:?}"
                            ));
                        }
                    }

                    for (name, cached) in &caches {
                        let got = snapshot_with(cached, &text, start, range, option, &limits);
                        tally.cache_plain += 1;
                        if lm != 0 {
                            tally.limit_cases += 1;
                        }
                        // Memoization only ever removes work, so a search that
                        // finishes uncached must give the identical answer.
                        if !is_limit_error(reference.0) {
                            assert_eq!(
                                got, reference,
                                "cache[{name}] vs plain: {pat:?} {text:?} start={start} range={range} {option:?} limits=({lm},{ls})"
                            );
                        } else if !is_limit_error(got.0) {
                            // Cached finished where plain gave up. The answer
                            // must be the true one (uncapped uncached search).
                            let unlimited = snapshot_with(
                                &plain,
                                &text,
                                start,
                                range,
                                option,
                                &onig_new_match_param(),
                            );
                            if !is_limit_error(unlimited.0) {
                                assert_eq!(
                                    got, unlimited,
                                    "cache[{name}] finished under limits ({lm},{ls}) with a wrong answer: {pat:?} {text:?} start={start} range={range} {option:?}"
                                );
                            }
                        }
                        if let Some(truth) = c_usable {
                            if !is_limit_error(got.0) && &got != truth && reference == *truth {
                                panic!(
                                    "cache[{name}] vs C: {pat:?} {text:?} start={start} range={range} {option:?} got={got:?} c={truth:?}"
                                );
                            }
                            tally.c_compared += 1;
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "seed={seed} cases={cases}: {} patterns ({} eligible with immediate cache; {} rejected by Ferroni, {} by C only), \
         {} cache/plain comparisons ({} under retry limits), {} against C ({} skipped for C limit errors), \
         {} plain-vs-C baseline divergences",
        tally.patterns,
        tally.eligible,
        tally.skipped_compile,
        tally.c_compile_mismatch,
        tally.cache_plain,
        tally.limit_cases,
        tally.c_compared,
        tally.c_limit_skipped,
        tally.plain_c_divergences.len(),
    );
    let mut by_option = std::collections::BTreeMap::<String, usize>::new();
    for d in &tally.plain_c_divergences {
        let option = d
            .split(" OnigOptionType(")
            .nth(1)
            .and_then(|r| r.split(')').next());
        *by_option
            .entry(option.unwrap_or("?").to_owned())
            .or_default() += 1;
    }
    eprintln!("  baseline divergences by option: {by_option:?}");
    for d in tally
        .plain_c_divergences
        .iter()
        .filter(|d| !d.contains("FIND_LONGEST"))
        .take(20)
    {
        eprintln!("  baseline divergence: {d}");
    }
    assert!(
        tally.eligible > tally.patterns / 20,
        "too few eligible patterns"
    );
}

fn char_boundaries(text: &str) -> Vec<usize> {
    (0..=text.len())
        .filter(|&i| text.is_char_boundary(i))
        .collect()
}

fn scanner_result(
    m: Option<ferroni::scanner::ScannerMatch>,
) -> Option<(usize, Vec<(usize, usize)>)> {
    m.map(|m| {
        (
            m.index,
            m.capture_indices.iter().map(|c| (c.start, c.end)).collect(),
        )
    })
}

/// Scanner semantics (earliest start, lowest index on ties) recomputed pattern by
/// pattern with a very large retry budget. The scanner swallows limit errors as
/// "no match", so this is the ground truth when the cached scanner finishes a
/// search that the uncached scanner abandoned. `None` means a limit was still hit.
fn unlimited_scanner_truth(
    regs: &[Regex],
    text: &str,
    start: usize,
    options: ScannerFindOptions,
) -> Option<Option<ScannerHit>> {
    let bits = if options == ScannerFindOptions::NOT_BEGIN_STRING {
        1
    } else if options == ScannerFindOptions::NOT_BEGIN_POSITION {
        4
    } else {
        0
    };
    let mut limits = onig_new_match_param();
    limits.retry_limit_in_match = 400_000_000;
    let mut best: Option<(usize, ScannerHit)> = None;
    for (index, re) in regs.iter().enumerate() {
        let onig_options = {
            let mut o = ONIG_OPTION_NONE;
            if bits & 1 != 0 {
                o |= ONIG_OPTION_NOT_BEGIN_STRING;
            }
            if bits & 2 != 0 {
                o |= ONIG_OPTION_NOT_END_STRING;
            }
            if bits & 4 != 0 {
                o |= ONIG_OPTION_NOT_BEGIN_POSITION;
            }
            o
        };
        let (r, caps) = snapshot_with(
            re,
            text.as_bytes(),
            start,
            text.len(),
            onig_options,
            &limits,
        );
        if is_limit_error(r) {
            return None;
        }
        if r >= 0 && best.as_ref().is_none_or(|b| (r as usize) < b.0) {
            let caps = caps
                .into_iter()
                .map(|(b, e)| (b.max(0) as usize, e.max(0) as usize))
                .collect();
            best = Some((r as usize, (index, caps)));
        }
    }
    Some(best.map(|(_, hit)| hit))
}

/// Shared scanner budgets, fallback, and subject lifecycle: several patterns in
/// one scanner searched repeatedly along one line, with unrelated strings
/// interleaved, compared with the uncached scanner and the C scanner.
#[test]
fn adversarial_scanner_shared_budget_and_lifecycle() {
    let cases = env_usize("MATCH_CACHE_ADVERSARIAL_SCANNER_CASES", 150);
    let seed = env_usize("MATCH_CACHE_ADVERSARIAL_SEED", 181) as u64 ^ 0x5CA_22E2;
    let mut rng = Rng(seed);
    let (mut scanners, mut searches, mut c_compared, mut fallback_hits, mut beyond_limit) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let config = ScannerConfig::default();
    let mut c_divergences = Vec::new();

    for _ in 0..cases {
        let n = 1 + rng.below(4);
        let pats: Vec<String> = (0..n).map(|_| pattern(&mut rng)).collect();
        let refs: Vec<&str> = pats.iter().map(String::as_str).collect();
        let Ok(mut plain) = Scanner::with_config(&refs, &config) else {
            continue;
        };
        let truth_regs: Vec<Regex> = pats
            .iter()
            .map(|p| {
                Regex::builder(p)
                    .option(ONIG_OPTION_CAPTURE_GROUP)
                    .build()
                    .unwrap()
            })
            .collect();
        let raw: Vec<&[u8]> = pats.iter().map(|p| p.as_bytes()).collect();
        let c = CScanner::new(&raw).ok();
        scanners += 1;
        let budgets = [16usize * 1024 * 1024, 64, 512];
        let mut cached: Vec<(usize, Scanner)> = budgets
            .iter()
            .flat_map(|&budget| {
                [0usize, 8]
                    .into_iter()
                    .map(move |threshold| (budget, threshold))
            })
            .map(|(budget, threshold)| {
                let cfg = MatchCacheConfig::new()
                    .memory_budget(budget)
                    .activation_threshold(threshold);
                (
                    budget,
                    Scanner::with_match_cache(&refs, &config, cfg).unwrap(),
                )
            })
            .collect();

        for _ in 0..3 {
            let text =
                String::from_utf8(subject(&mut rng).into_iter().filter(u8::is_ascii).collect())
                    .unwrap();
            let unicode: String = if rng.below(3) == 0 {
                format!("é{text}😀{text}")
            } else {
                text.clone()
            };
            for text in [text, unicode] {
                let line = OnigString::new(&text);
                let other = OnigString::new("zzz,ab");
                let mut starts = char_boundaries(&text);
                // Mostly advancing like a tokenizer, sometimes jumping back.
                if rng.below(3) == 0 {
                    let i = rng.below(starts.len());
                    starts.swap(0, i);
                }
                for start in starts {
                    for options in [
                        ScannerFindOptions::NONE,
                        ScannerFindOptions::NOT_BEGIN_STRING,
                        ScannerFindOptions::NOT_BEGIN_POSITION,
                    ] {
                        let ascii = text.is_ascii();
                        let expect = if ascii {
                            scanner_result(plain.find_next_match_utf16(&line, start, options))
                        } else {
                            scanner_result(plain.find_next_match(&text, start, options))
                        };
                        if options == ScannerFindOptions::NONE && text.len() <= 400 {
                            if let Some(c) = &c {
                                let got = c.find_next_match(text.as_bytes(), 0, start);
                                let got = got.map(|(i, caps)| {
                                    (
                                        i,
                                        caps.into_iter()
                                            .map(|(b, e)| (b.max(0) as usize, e.max(0) as usize))
                                            .collect::<Vec<_>>(),
                                    )
                                });
                                c_compared += 1;
                                if got != expect {
                                    c_divergences.push(format!(
                                        "{pats:?} {text:?} start={start} plain={expect:?} c={got:?}"
                                    ));
                                }
                            }
                        }
                        for (budget, scanner) in &mut cached {
                            let got = if ascii {
                                scanner_result(scanner.find_next_match_utf16(&line, start, options))
                            } else {
                                scanner_result(scanner.find_next_match(&text, start, options))
                            };
                            searches += 1;
                            if got != expect {
                                // The cache may only differ by finishing a search
                                // the plain scanner abandoned at a retry limit.
                                let truth =
                                    unlimited_scanner_truth(&truth_regs, &text, start, options);
                                assert_eq!(
                                    Some(&got),
                                    truth.as_ref(),
                                    "scanner cache(budget {budget}) vs plain {expect:?}: {pats:?} {text:?} start={start} {options:?}"
                                );
                                beyond_limit += 1;
                            }
                            assert!(
                                scanner.match_cache_bytes() <= *budget,
                                "budget {budget} exceeded: {}",
                                scanner.match_cache_bytes()
                            );
                            fallback_hits += u64::from(scanner.match_cache_bytes() == 0);
                            if rng.below(4) == 0 {
                                // A different subject must not inherit failures.
                                let _ = scanner.find_next_match_utf16(
                                    &other,
                                    rng.below(3),
                                    ScannerFindOptions::NONE,
                                );
                            }
                        }
                        if rng.below(4) == 0 {
                            let _ = plain.find_next_match_utf16(
                                &other,
                                rng.below(3),
                                ScannerFindOptions::NONE,
                            );
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "seed={seed}: {scanners} scanners, {searches} cached-vs-plain scanner searches, \
         {c_compared} against the C scanner ({} baseline divergences), {fallback_hits} with no retained cache, \
         {beyond_limit} where the cache finished a search the plain scanner abandoned at a retry limit (verified against an unlimited search)",
        c_divergences.len()
    );
    for d in c_divergences.iter().take(20) {
        eprintln!("  baseline divergence: {d}");
    }
}
