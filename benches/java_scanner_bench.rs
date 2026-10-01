#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
#[cfg(feature = "ffi")]
use scanner_replay::CReplay;
use scanner_replay::{Corpus, replay};

fn bench_java(c: &mut Criterion) {
    let corpus = Corpus::from_json(include_str!("java_scanner/trace.json"));
    corpus.validate();
    let mut group = c.benchmark_group("java_scanner");
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
    }
    group.finish();
}
criterion_group!(benches, bench_java);
criterion_main!(benches);
