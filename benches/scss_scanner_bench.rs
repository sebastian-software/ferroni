//! Criterion benchmark that replays the SCSS grammar's scanner calls through Ferroni
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

fn bench_scss(c: &mut Criterion) {
    bench_replays(
        c,
        "scss",
        &Corpus::from_json(include_str!("scss_scanner/trace.json")),
        true,
    );
}
criterion_group!(benches, bench_scss);
criterion_main!(benches);
