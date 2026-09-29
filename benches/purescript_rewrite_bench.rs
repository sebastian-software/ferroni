//! PureScript decimal tails: full-capture compiler/atomic comparison, plus
//! the grammar-specific flattened rewrite (only match bounds are equivalent).
#[path = "../tests/fixtures/decimal_patterns.rs"]
mod patterns;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ferroni::api::{Regex, SearchOptions};
use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions};
use std::hint::black_box;
use std::time::Duration;

const ATOMIC: &str = r"\b((?>([0-9]+_?)*[0-9]+)|0([Xx]\h+|[Oo][0-7]+))\b";
const FLAT: &str = r"\b([0-9]+(?:_[0-9]+)*|0([Xx]\h+|[Oo][0-7]+))\b";

fn options() -> SearchOptions {
    SearchOptions::new()
        .retry_limit_in_match(0)
        .retry_limit_in_search(0)
}

fn trace(re: &Regex, text: &str) -> Option<Vec<Option<(usize, usize)>>> {
    re.captures_with(text, options()).unwrap().map(|c| {
        (0..c.len())
            .map(|i| c.get(i).map(|m| (m.start(), m.end())))
            .collect()
    })
}

fn modes() -> Vec<(&'static str, &'static str, bool)> {
    let mut modes = vec![
        ("plain", patterns::PURESCRIPT_INTEGER, false),
        ("source_atomic", ATOMIC, false),
        ("compiler", patterns::PURESCRIPT_INTEGER, true),
        ("grammar_flat", FLAT, false),
    ];
    if std::env::var_os("FERRONI_REWRITE_REVERSE").is_some() {
        modes.reverse();
    }
    modes
}

fn configure(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1));
}

fn regexes(c: &mut Criterion) {
    let mut group = c.benchmark_group("purescript_rewrite_match");
    configure(&mut group);
    let plain = Regex::new(patterns::PURESCRIPT_INTEGER).unwrap();
    let cases = vec![
        ("success_short", "12_34".to_owned()),
        ("success_long_digits", "1".repeat(4096)),
        (
            "success_long_segments",
            format!("{}123", "123_456_".repeat(512)),
        ),
        ("failed_letter_12", format!("{}x", "1".repeat(12))),
        ("failed_letter_20", format!("{}x", "1".repeat(20))),
        ("failed_underscore_20", format!("{}_", "1".repeat(20))),
        ("hex_control", "0x1f".to_owned()),
    ];
    for (case, text) in cases {
        let expected = trace(&plain, &text);
        #[cfg(feature = "ffi")]
        {
            let re = ferroni::ffi::CRegex::new(patterns::PURESCRIPT_INTEGER.as_bytes(), 0).unwrap();
            let mut region = ferroni::ffi::CRegion::new();
            let result = re.search(text.as_bytes(), 0, text.len(), Some(&mut region), 0);
            assert!(result >= -1, "C limit error: {case}");
            let c_trace = (result >= 0).then(|| {
                region
                    .capture_ranges()
                    .into_iter()
                    .map(|(a, b)| (a >= 0).then_some((a as usize, b as usize)))
                    .collect()
            });
            assert_eq!(expected, c_trace, "C: {case}");
        }
        for (mode, pattern, enabled) in modes() {
            let re = Regex::builder(pattern)
                .optimize_backtracking(enabled)
                .build()
                .unwrap();
            if mode == "grammar_flat" {
                assert_eq!(
                    trace(&re, &text).map(|c| c[0]),
                    expected.as_ref().map(|c| c[0])
                );
            } else {
                assert_eq!(trace(&re, &text), expected, "{mode}/{case}");
            }
            group.bench_with_input(BenchmarkId::new(case, mode), &text, |b, text| {
                b.iter(|| black_box(re.find_with(black_box(text), options()).unwrap()))
            });
        }
    }
    group.finish();
}

fn captures(c: &mut Criterion) {
    let mut group = c.benchmark_group("purescript_rewrite_captures");
    configure(&mut group);
    for (case, text) in [
        ("success_short", "12_34".to_owned()),
        ("success_long_digits", "1".repeat(4096)),
        (
            "success_long_segments",
            format!("{}123", "123_456_".repeat(512)),
        ),
    ] {
        for (mode, pattern, enabled) in modes() {
            let re = Regex::builder(pattern)
                .optimize_backtracking(enabled)
                .build()
                .unwrap();
            group.bench_with_input(BenchmarkId::new(case, mode), &text, |b, text| {
                b.iter(|| black_box(re.captures_with(black_box(text), options()).unwrap()))
            });
        }
    }
    group.finish();
}

fn scan(scanner: &mut Scanner, lines: &[OnigString]) -> Vec<ferroni::scanner::ScannerMatch> {
    let mut matches = Vec::new();
    for line in lines {
        let mut start = 0;
        while let Some(found) = scanner.find_next_match_utf16(line, start, ScannerFindOptions::NONE)
        {
            let end = found.capture_indices[0].end;
            matches.push(found);
            start = if end > start { end } else { start + 1 };
            if start > line.utf16_len() {
                break;
            }
        }
    }
    matches
}

fn scanner_and_compile(c: &mut Criterion) {
    // A small numeric scanner, not a complete TextMate tokenizer. Distinct
    // lines keep the scanner's last-string memo cold between iterations.
    let lines: Vec<_> = [
        "module Demo where\n",
        "small = 12_34\n",
        "hex = 0x1f\n",
        "octal = 0o71\n",
        "other = 123x\n",
        "😀 value = 98_76\n",
    ]
    .into_iter()
    .map(OnigString::new)
    .collect();
    let mut plain = Scanner::new(&[patterns::PURESCRIPT_INTEGER, r"[A-Za-z]+", r"\S"]).unwrap();
    let expected = scan(&mut plain, &lines);
    let mut group = c.benchmark_group("purescript_rewrite_scanner");
    configure(&mut group);
    for (mode, pattern, enabled) in modes() {
        let patterns = [pattern, r"[A-Za-z]+", r"\S"];
        let mut scanner = if enabled {
            Scanner::with_backtracking_optimization(&patterns, &ScannerConfig::default()).unwrap()
        } else {
            Scanner::new(&patterns).unwrap()
        };
        let actual = scan(&mut scanner, &lines);
        if mode == "grammar_flat" {
            assert_eq!(
                actual
                    .iter()
                    .map(|m| (
                        m.index,
                        m.capture_indices[0].start,
                        m.capture_indices[0].end
                    ))
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|m| (
                        m.index,
                        m.capture_indices[0].start,
                        m.capture_indices[0].end
                    ))
                    .collect::<Vec<_>>()
            );
        } else {
            assert_eq!(actual, expected);
        }
        group.bench_function(mode, |b| {
            b.iter(|| black_box(scan(&mut scanner, black_box(&lines))))
        });
    }
    group.finish();
    let mut group = c.benchmark_group("purescript_rewrite_compile");
    configure(&mut group);
    for (mode, pattern, enabled) in modes() {
        group.bench_function(mode, |b| {
            b.iter(|| {
                black_box(
                    Regex::builder(black_box(pattern))
                        .optimize_backtracking(enabled)
                        .build()
                        .unwrap(),
                )
            })
        });
    }
    group.finish();
}

criterion_group!(benches, regexes, captures, scanner_and_compile);
criterion_main!(benches);
