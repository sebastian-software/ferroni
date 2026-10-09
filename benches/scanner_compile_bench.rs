//! Construction and destruction of complete captured scanner sets, uncached
//! and through one `ScannerPatternCache` per set (`<name>_pattern_cache`).
//! JSON parsing and fixture loading happen outside the measurement.
//! `FERRONI_BENCH_PREFILTER=0` builds the scanners without the DFA
//! pre-filter (`scanner_replay::bench_config`).
#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferroni::scanner::{Scanner, ScannerPatternCache};
use scanner_replay::bench_config;
use std::hint::black_box;
use std::path::PathBuf;

fn bench_compile(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scss = std::env::var_os("FERRONI_SCSS_TRACE").map_or_else(
        || root.join("benches/scss_scanner/trace.json"),
        PathBuf::from,
    );
    let config = bench_config();
    let mut group = c.benchmark_group("scanner_compile_and_drop");
    for (name, path) in [
        ("cpp", root.join("benches/cpp_scanner/trace.json")),
        ("scss", scss),
        ("java", root.join("benches/java_scanner/trace.json")),
    ] {
        let fixture: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).expect("captured fixture exists"))
                .expect("valid captured JSON");
        let patterns: Vec<Vec<String>> =
            serde_json::from_value(fixture["scanners"].clone()).expect("ordered scanner patterns");
        let references: Vec<Vec<&str>> = patterns
            .iter()
            .map(|scanner| scanner.iter().map(String::as_str).collect())
            .collect();
        group.throughput(Throughput::Elements(
            patterns.iter().map(Vec::len).sum::<usize>() as u64,
        ));
        group.bench_function(name, |b| {
            b.iter(|| {
                let scanners: Vec<_> = references
                    .iter()
                    .map(|patterns| {
                        Scanner::with_config(black_box(patterns), &config)
                            .expect("captured patterns compile")
                    })
                    .collect();
                // Keep teardown inside the named measurement boundary.
                drop(black_box(scanners));
            });
        });
        // A fresh cache per iteration: each distinct pattern is compiled
        // once, as on the first highlight of a grammar in a process.
        group.bench_function(format!("{name}_pattern_cache"), |b| {
            b.iter(|| {
                let mut cache = ScannerPatternCache::new();
                let scanners: Vec<_> = references
                    .iter()
                    .map(|patterns| {
                        Scanner::with_pattern_cache(black_box(patterns), &config, &mut cache)
                            .expect("captured patterns compile")
                    })
                    .collect();
                drop(black_box(cache));
                drop(black_box(scanners));
            });
        });
    }
    group.finish();
}
criterion_group!(benches, bench_compile);
criterion_main!(benches);
