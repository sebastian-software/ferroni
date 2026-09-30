mod cpp_scanner;
use cpp_scanner::{Corpus, replay};
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

fn bench_cpp(c: &mut Criterion) {
    let corpus = Corpus::load();
    corpus.validate();
    let mut group = c.benchmark_group("cpp_scanner");
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
criterion_group!(benches, bench_cpp);
criterion_main!(benches);
