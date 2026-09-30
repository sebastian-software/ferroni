#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
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
        group.bench_function(name, |b| {
            b.iter_batched_ref(
                || corpus.strings(),
                |strings| replay(&mut scanners, strings, &calls),
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}
criterion_group!(benches, bench_scss);
criterion_main!(benches);
