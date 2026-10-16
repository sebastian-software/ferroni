//! Criterion benchmark that replays the Java grammar's scanner calls through Ferroni
//! and the other engines in `engines.rs`, including the one-shot `cold_document`
//! lanes. See `engine_replay::bench_replays`.

#[path = "engine_replay.rs"]
mod engine_replay;
#[path = "engines.rs"]
mod engines;
#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;
use criterion::{Criterion, criterion_group, criterion_main};
use engine_replay::bench_replays;
use scanner_replay::Corpus;

fn bench_java(c: &mut Criterion) {
    bench_replays(
        c,
        "java",
        &Corpus::from_json(include_str!("java_scanner/trace.json")),
        true,
    );
}
criterion_group!(benches, bench_java);
criterion_main!(benches);
