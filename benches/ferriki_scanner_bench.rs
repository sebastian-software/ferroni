//! Replays of the scanner calls Ferriki's tokenizer makes for the curated
//! JSON, Astro, TSX and Rust fixtures, recorded with
//! benches/json_scanner/capture.patch, with their one-shot `cold_document`
//! lanes. See benches/json_scanner, benches/astro_scanner, benches/tsx_scanner
//! and benches/rust_scanner.
#[path = "engine_replay.rs"]
mod engine_replay;
#[path = "engines.rs"]
mod engines;
#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;
use criterion::{Criterion, criterion_group, criterion_main};
use engine_replay::bench_replays;
use scanner_replay::Corpus;

fn bench_ferriki(c: &mut Criterion) {
    bench_replays(
        c,
        "json",
        &Corpus::from_json(include_str!("json_scanner/trace.json")),
        true,
    );
    bench_replays(
        c,
        "astro",
        &Corpus::from_json(include_str!("astro_scanner/trace.json")),
        true,
    );
    bench_replays(
        c,
        "tsx",
        &Corpus::from_json(include_str!("tsx_scanner/trace.json")),
        true,
    );
    bench_replays(
        c,
        "rust",
        &Corpus::from_json(include_str!("rust_scanner/trace.json")),
        true,
    );
}

criterion_group!(benches, bench_ferriki);
criterion_main!(benches);
