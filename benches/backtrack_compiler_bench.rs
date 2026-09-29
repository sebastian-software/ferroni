//! Compiler versus capture-preserving source rewrites. All modes validate captures.
//! `cargo bench --locked --bench backtrack_compiler_bench`
mod grammar_loader;
mod scanner_documents;

#[path = "../tests/fixtures/decimal_patterns.rs"]
mod patterns;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ferroni::api::{Regex, SearchOptions};
use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions};
use std::hint::black_box;
use std::time::Duration;

fn reverse_order() -> bool {
    std::env::var_os("FERRONI_REWRITE_REVERSE").is_some()
}

fn source_outer(pattern: &str) -> String {
    pattern.replacen("([0-9]+(_?))+", "(?>([0-9]+(_?))+)", 1)
}

fn source_inner(pattern: &str) -> String {
    pattern.replacen("[0-9]+(_?)", "[0-9]++(_?)", 1)
}

fn limits() -> SearchOptions {
    SearchOptions::new()
        .retry_limit_in_match(0)
        .retry_limit_in_search(0)
}

fn captures(regex: &Regex, text: &str) -> Option<Vec<Option<(usize, usize)>>> {
    regex.captures_with(text, limits()).unwrap().map(|c| {
        (0..c.len())
            .map(|i| c.get(i).map(|m| (m.start(), m.end())))
            .collect()
    })
}

fn regexes(c: &mut Criterion) {
    let mut group = c.benchmark_group("decimal_compiler_match");
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1));
    for (name, pattern) in [
        ("v_float", patterns::V_FLOAT),
        ("v_exponent", patterns::V_EXPONENT),
    ] {
        let plain = Regex::new(pattern).unwrap();
        let fast = Regex::builder(pattern)
            .optimize_backtracking(true)
            .build()
            .unwrap();
        assert_eq!(fast.backtracking_rewrites().len(), 1);
        let outer = Regex::new(&source_outer(pattern)).unwrap();
        let inner = Regex::new(&source_inner(pattern)).unwrap();
        let mut cases = vec![
            ("success_short".to_string(), "12_34.56E+78".to_string()),
            (
                "success_long".to_string(),
                format!("{}.56E+78", "123_456_".repeat(512)),
            ),
        ];
        for len in [12, 16, 20] {
            cases.push((
                format!("failed_tail_{len}"),
                format!("{}.x", "1".repeat(len)),
            ));
        }
        for (case, text) in cases {
            let expected = captures(&plain, &text);
            for re in [&fast, &outer, &inner] {
                assert_eq!(captures(re, &text), expected, "{name}/{case}");
            }
            #[cfg(feature = "ffi")]
            {
                let re = ferroni::ffi::CRegex::new(pattern.as_bytes(), 0).unwrap();
                let mut region = ferroni::ffi::CRegion::new();
                let result = re.search(text.as_bytes(), 0, text.len(), Some(&mut region), 0);
                assert!(result >= -1, "C limit error: {name}/{case}");
                let c_trace = (result >= 0).then(|| {
                    region
                        .capture_ranges()
                        .into_iter()
                        .map(|(start, end)| (start >= 0).then_some((start as usize, end as usize)))
                        .collect()
                });
                assert_eq!(expected, c_trace, "C: {name}/{case}");
            }
            let mut modes = [
                ("plain", &plain),
                ("source_outer", &outer),
                ("source_inner", &inner),
                ("compiler", &fast),
            ];
            if reverse_order() {
                modes.reverse();
            }
            for (mode, re) in modes {
                group.bench_with_input(
                    BenchmarkId::new(format!("{name}/{case}"), mode),
                    &text,
                    |b, text| {
                        b.iter(|| black_box(re.find_with(black_box(text), limits()).unwrap()));
                    },
                );
            }
        }
    }
    group.finish();
}

fn scanner_trace(
    scanner: &mut Scanner,
    lines: &[OnigString],
) -> Vec<ferroni::scanner::ScannerMatch> {
    let mut trace = Vec::new();
    scan(scanner, lines, |found| trace.push(found));
    trace
}

fn scan(
    scanner: &mut Scanner,
    lines: &[OnigString],
    mut visit: impl FnMut(ferroni::scanner::ScannerMatch),
) {
    for line in lines {
        let mut start = 0;
        while let Some(found) = scanner.find_next_match_utf16(line, start, ScannerFindOptions::NONE)
        {
            let end = found.capture_indices[0].end;
            visit(found);
            start = if end > start { end } else { start + 1 };
            if start > line.utf16_len() {
                break;
            }
        }
    }
}

fn scanners_and_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("decimal_compiler_scanner");
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1));
    let ts = grammar_loader::typescript_patterns();
    let ts_patterns: Vec<_> = ts.iter().map(String::as_str).collect();
    let v_patterns = [
        patterns::V_EXPONENT,
        patterns::V_FLOAT,
        r"[0-9]+",
        r"[a-zA-Z_]+",
        r"\s+",
        r".",
    ];
    let ts_lines: Vec<_> = scanner_documents::TYPESCRIPT_DOCUMENT
        .lines()
        .map(OnigString::new)
        .collect();
    let v_lines: Vec<_> = (0..128)
        .map(|i| {
            OnigString::new(&format!(
                "value_{i} := 12_345.67 + 89.01E+23 // normal input"
            ))
        })
        .collect();
    for (name, patterns, lines) in [
        ("v_normal", v_patterns.as_slice(), &v_lines),
        ("typescript_document", ts_patterns.as_slice(), &ts_lines),
    ] {
        let config = ScannerConfig::default();
        let mut plain = Scanner::with_config(patterns, &config).unwrap();
        let mut fast = Scanner::with_backtracking_optimization(patterns, &config).unwrap();
        let outer_patterns: Vec<_> = patterns.iter().map(|p| source_outer(p)).collect();
        let inner_patterns: Vec<_> = patterns.iter().map(|p| source_inner(p)).collect();
        let outer_refs: Vec<_> = outer_patterns.iter().map(String::as_str).collect();
        let inner_refs: Vec<_> = inner_patterns.iter().map(String::as_str).collect();
        let mut outer = Scanner::with_config(&outer_refs, &config).unwrap();
        let mut inner = Scanner::with_config(&inner_refs, &config).unwrap();
        let expected = scanner_trace(&mut plain, lines);
        for scanner in [&mut fast, &mut outer, &mut inner] {
            assert_eq!(expected, scanner_trace(scanner, lines), "{name}");
        }
        let mut modes = [
            ("plain", &mut plain),
            ("source_outer", &mut outer),
            ("source_inner", &mut inner),
            ("compiler", &mut fast),
        ];
        if reverse_order() {
            modes.reverse();
        }
        for (mode, scanner) in modes {
            group.bench_with_input(BenchmarkId::new(name, mode), lines, |b, lines| {
                b.iter(|| {
                    scan(scanner, black_box(lines), |found| {
                        black_box(found);
                    })
                });
            });
        }
    }
    group.finish();

    let mut group = c.benchmark_group("decimal_compiler_compile");
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1));
    for (name, pattern) in [
        ("v_float", patterns::V_FLOAT),
        ("purescript_integer", patterns::PURESCRIPT_INTEGER),
    ] {
        let mut modes = [
            ("plain", pattern.to_owned(), false),
            ("source_outer", source_outer(pattern), false),
            ("source_inner", source_inner(pattern), false),
            ("compiler", pattern.to_owned(), true),
        ];
        if reverse_order() {
            modes.reverse();
        }
        for (mode, source, enabled) in modes {
            group.bench_function(BenchmarkId::new(name, mode), |b| {
                b.iter(|| {
                    black_box(
                        Regex::builder(black_box(&source))
                            .optimize_backtracking(enabled)
                            .build()
                            .unwrap(),
                    )
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, regexes, scanners_and_compilation);
criterion_main!(benches);
