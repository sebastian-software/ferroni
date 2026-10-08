//! Criterion benchmark that replays the SCSS grammar's scanner calls through Ferroni
//! and the other engines in `engines.rs`.

#[path = "engine_replay.rs"]
mod engine_replay;
#[path = "engines.rs"]
mod engines;
#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
#[cfg(feature = "ffi")]
use scanner_replay::CReplay;
use scanner_replay::{Corpus, replay};

fn bench_scss(c: &mut Criterion) {
    let corpus = Corpus::from_json(include_str!("scss_scanner/trace.json"));
    corpus.validate();
    let mut group = c.benchmark_group("scss_scanner");
    for selected in std::iter::once(None).chain(corpus.hot_groups.iter().copied().map(Some)) {
        let calls = corpus.selected(selected);
        let mut scanners = corpus.scanners();
        let name = selected.map_or_else(|| "document".to_owned(), |id| format!("group_{id}"));
        group.throughput(Throughput::Elements(calls.len() as u64));
        group.bench_function(&name, |b| {
            b.iter_batched_ref(
                || corpus.strings(),
                |strings| replay(&mut scanners, strings, &calls),
                BatchSize::LargeInput,
            );
        });
        #[cfg(feature = "ffi")]
        {
            let c_replay = CReplay::new(&corpus, &calls);
            group.bench_function(format!("{name}_c"), |b| {
                b.iter_batched(
                    || c_replay.fresh_ids(),
                    |ids| c_replay.replay(ids),
                    BatchSize::SmallInput,
                );
            });
        }
        for &engine in engines::Engine::ALL {
            match engine_replay::EngineReplay::new(engine, &corpus, &calls) {
                Ok(replay) => {
                    let id = format!("scss_scanner/{name}_{}", engine.id());
                    if replay.empty_capture_differences > 0 {
                        println!(
                            "EQUIVALENT {}",
                            serde_json::json!({"id": id, "empty_capture_differences": replay.empty_capture_differences})
                        );
                    }
                    group.bench_function(format!("{name}_{}", engine.id()), |b| {
                        b.iter(|| replay.replay());
                    });
                }
                Err(reason) => engines::unsupported(
                    &format!("scss_scanner/{name}_{}", engine.id()),
                    engine,
                    &reason,
                ),
            }
        }
    }
    group.finish();
}
criterion_group!(benches, bench_scss);
criterion_main!(benches);
