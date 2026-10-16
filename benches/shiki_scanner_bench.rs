//! Replays of the scanner calls Shiki makes for the TIOBE C and PHP documents,
//! recorded with benches/shiki_js/capture.mjs. See benches/c_scanner and
//! benches/php_scanner.
#[path = "engine_replay.rs"]
mod engine_replay;
#[path = "engines.rs"]
mod engines;
#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;
use criterion::{Criterion, criterion_group, criterion_main};
use engine_replay::bench_replays;
use scanner_replay::Corpus;

fn bench_shiki(c: &mut Criterion) {
    bench_replays(
        c,
        "c",
        &Corpus::from_json(include_str!("c_scanner/trace.json")),
        false,
    );
    bench_replays(
        c,
        "php",
        &Corpus::from_json(include_str!("php_scanner/trace.json")),
        false,
    );
}

criterion_group!(benches, bench_shiki);
criterion_main!(benches);
