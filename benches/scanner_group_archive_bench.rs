//! Compare in-memory archive loading, warm execution, and one-shot replay.
mod group_archive;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use std::{hint::black_box, path::PathBuf, time::Duration};

fn bench(c: &mut Criterion) {
    let name = std::env::var("GROUP_ARCHIVE_CORPUS").expect("GROUP_ARCHIVE_CORPUS");
    let variant = std::env::var("GROUP_ARCHIVE_VARIANT").expect("GROUP_ARCHIVE_VARIANT");
    let dir = PathBuf::from(std::env::var_os("GROUP_ARCHIVE_DIR").expect("GROUP_ARCHIVE_DIR"));
    let corpus = group_archive::corpus(&name);
    let bundle = group_archive::Bundle::read(&dir, &name, &variant);
    bundle.validate(&corpus);
    let mut group = c.benchmark_group(format!("{name}/{variant}"));
    group
        .sample_size(40)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(1))
        .nresamples(10_000);
    group.throughput(Throughput::Elements(corpus.calls.len() as u64));
    group.bench_function("load", |b| {
        b.iter_batched(
            || (),
            |()| {
                // Return ownership so teardown remains outside the load boundary.
                black_box(bundle.scanners(&corpus))
            },
            BatchSize::PerIteration,
        )
    });
    let calls = corpus.selected(None);
    let (_cache, mut scanners) = bundle.scanners(&corpus);
    for _ in 0..3 {
        group_archive::replay(&mut scanners, &corpus.strings(), &calls);
    }
    group.bench_function("search", |b| {
        b.iter_batched_ref(
            || corpus.strings(),
            |strings| {
                group_archive::replay(&mut scanners, strings, &calls);
            },
            BatchSize::LargeInput,
        )
    });
    group.bench_function("cold_document", |b| {
        b.iter_batched(
            || corpus.strings(),
            |strings| {
                let (cache, mut scanners) = bundle.scanners(&corpus);
                group_archive::replay(&mut scanners, &strings, &calls);
                (cache, scanners, strings)
            },
            BatchSize::PerIteration,
        )
    });
    group.finish();
}
fn config() -> Criterion {
    Criterion::default().output_directory(&PathBuf::from(
        std::env::var_os("GROUP_ARCHIVE_OUTPUT").expect("GROUP_ARCHIVE_OUTPUT"),
    ))
}
criterion_group! { name = benches; config = config(); targets = bench }
criterion_main!(benches);
