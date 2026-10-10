//! Replays of the scanner calls Ferriki's tokenizer makes for the curated
//! JSON and Astro fixtures, recorded with benches/json_scanner/capture.patch.
//! See benches/json_scanner and benches/astro_scanner.
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

fn bench_language(c: &mut Criterion, language: &str, trace: &str) {
    let corpus = Corpus::from_json(trace);
    corpus.validate();
    let mut group = c.benchmark_group(format!("{language}_scanner"));
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
                    let id = format!("{language}_scanner/{name}_{}", engine.id());
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
                    &format!("{language}_scanner/{name}_{}", engine.id()),
                    engine,
                    &reason,
                ),
            }
        }
    }
    group.finish();
}
fn bench_ferriki(c: &mut Criterion) {
    bench_language(c, "json", include_str!("json_scanner/trace.json"));
    bench_language(c, "astro", include_str!("astro_scanner/trace.json"));
}

criterion_group!(benches, bench_ferriki);
criterion_main!(benches);
