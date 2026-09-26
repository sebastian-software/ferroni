//! Run with `cargo bench --features match-cache --bench match_cache_bench`.
mod grammar_loader;
mod scanner_css_workload;
mod scanner_documents;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ferroni::api::{Regex, SearchOptions};
use ferroni::match_cache::MatchCacheConfig;
use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions};
use std::hint::black_box;

fn scan(
    scanner: &mut Scanner,
    lines: &[(OnigString, usize)],
    mut visit: impl FnMut(&ferroni::scanner::ScannerMatch),
) -> usize {
    let mut count = 0;
    for (line, len) in lines {
        let mut pos = 0;
        while pos < *len {
            let Some(found) = scanner.find_next_match_utf16(line, pos, ScannerFindOptions::NONE)
            else {
                break;
            };
            let end = found.capture_indices[0].end;
            visit(&found);
            count += 1;
            pos = if end > pos { end } else { pos + 1 };
        }
    }
    count
}

fn trace(
    scanner: &mut Scanner,
    lines: &[(OnigString, usize)],
) -> Vec<(usize, Vec<(usize, usize)>)> {
    let mut result = Vec::new();
    scan(scanner, lines, |found| {
        result.push((
            found.index,
            found
                .capture_indices
                .iter()
                .map(|c| (c.start, c.end))
                .collect(),
        ))
    });
    result
}

fn documents(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_cache_documents");
    for (name, grammar, doc) in [
        (
            "typescript",
            grammar_loader::typescript_patterns(),
            scanner_documents::TYPESCRIPT_DOCUMENT,
        ),
        (
            "css",
            grammar_loader::css_patterns(),
            scanner_css_workload::CSS_INPUT,
        ),
        (
            "rust",
            grammar_loader::rust_patterns(),
            scanner_documents::RUST_DOCUMENT,
        ),
    ] {
        let patterns: Vec<_> = grammar.iter().map(String::as_str).collect();
        let lines: Vec<_> = doc
            .lines()
            .map(|line| {
                let line = format!("{line}\n");
                let len = line.encode_utf16().count();
                (OnigString::new(&line), len)
            })
            .collect();
        let mut plain = Scanner::new(&patterns).unwrap();
        let mut cached = Scanner::with_match_cache(
            &patterns,
            &ScannerConfig::default(),
            MatchCacheConfig::new(),
        )
        .unwrap();
        assert_eq!(
            trace(&mut plain, &lines),
            trace(&mut cached, &lines),
            "{name} capture trace"
        );
        for (mode, scanner) in [("off", &mut plain), ("on", &mut cached)] {
            group.bench_function(BenchmarkId::new(name, mode), |b| {
                b.iter(|| black_box(scan(scanner, black_box(&lines), |_| {})));
            });
        }
    }
    group.finish();
}

fn hostile(c: &mut Criterion) {
    let pattern = r"(?:\w+\s*,\s*)*\w+\s*$";
    let plain = Regex::new(pattern).unwrap();
    let cached = Regex::builder(pattern)
        .match_cache(MatchCacheConfig::new())
        .build()
        .unwrap();
    let limits = SearchOptions::new()
        .retry_limit_in_match(0)
        .retry_limit_in_search(0);
    let mut group = c.benchmark_group("match_cache_hostile");
    group.sample_size(10);
    for len in [256, 1024, 2048] {
        let text = format!("{}!", "a".repeat(len));
        for (mode, re) in [("off", &plain), ("on", &cached)] {
            assert!(re.find_with(&text, limits).unwrap().is_none());
            group.bench_function(BenchmarkId::new(mode, len), |b| {
                b.iter(|| black_box(re.find_with(black_box(&text), limits).unwrap()));
            });
        }
    }
    group.finish();
}

fn hostile_scanner(c: &mut Criterion) {
    let mut grammar = grammar_loader::typescript_patterns();
    grammar.insert(0, r"(a+)+$".into());
    let patterns: Vec<_> = grammar.iter().map(String::as_str).collect();
    let mut plain = Scanner::new(&patterns).unwrap();
    let mut cached = Scanner::with_match_cache(
        &patterns,
        &ScannerConfig::default(),
        MatchCacheConfig::new(),
    )
    .unwrap();
    let mut group = c.benchmark_group("match_cache_hostile_scanner");
    group.sample_size(10);
    for len in [12, 16, 20] {
        // An exponential failure at the first position, before the ordinary
        // identifier rule can win. The plain scanner remains below its retry
        // limit at these sizes, allowing an exact token/capture comparison.
        let text = format!("{}!", "a".repeat(len));
        let utf16_len = text.encode_utf16().count();
        let lines = [(OnigString::new(&text), utf16_len)];
        assert_eq!(trace(&mut plain, &lines), trace(&mut cached, &lines));
        assert!(cached.match_cache_bytes() > 0, "cache did not activate");
        eprintln!(
            "hostile scanner {len}: {} cache bytes retained after tokenization",
            cached.match_cache_bytes()
        );
        for (mode, scanner) in [("off", &mut plain), ("on", &mut cached)] {
            group.bench_function(BenchmarkId::new(mode, len), |b| {
                b.iter(|| {
                    // Fresh immutable identity per iteration: include the first
                    // expensive search rather than time an already settled line.
                    let lines = [(OnigString::new(black_box(&text)), utf16_len)];
                    black_box(scan(scanner, &lines, |_| {}))
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, documents, hostile, hostile_scanner);
criterion_main!(benches);
