//! Small, separately reported application workloads over the public Scanner API.
#[path = "scanner_clients/mod.rs"]
mod clients;
#[path = "cpp_scanner/mod.rs"]
mod scanner_replay;

use criterion::{BatchSize, Criterion};
use std::hint::black_box;

fn main() {
    let validate = std::env::args().any(|arg| arg == "--validate");
    let selected = std::env::var("SCANNER_CLIENT_CASE").ok();
    let mode = std::env::var("SCANNER_CLIENT_MODE").unwrap_or_else(|_| "recorded".into());
    assert!(matches!(mode.as_str(), "recorded" | "uncached"));
    let mut criterion = (!validate).then(|| Criterion::default().configure_from_args());
    for (id, input, lanes) in [
        (
            "tsx",
            include_str!("scanner_clients/tsx.json"),
            &["compile", "first_use", "steady"][..],
        ),
        (
            "astro",
            include_str!("scanner_clients/astro.json"),
            &["compile", "first_use"][..],
        ),
        (
            "related",
            include_str!("scanner_clients/related.json"),
            &["compile", "first_use"][..],
        ),
    ] {
        if selected.as_ref().is_some_and(|case| case != id) {
            continue;
        }
        assert!(
            mode == "recorded" || id == "related",
            "uncached control is only for the related-grammar case"
        );
        let trace = clients::Trace::parse(input);
        let shared = mode == "recorded";
        trace.validate(shared);
        if validate {
            println!(
                "{}",
                serde_json::json!({"case":id,"mode":mode,"status":"eligible","stats":trace.stats(),"c_oracle":cfg!(feature="ffi")})
            );
            continue;
        }
        let mut group = criterion
            .as_mut()
            .unwrap()
            .benchmark_group(format!("scanner_clients/{id}"));
        for lane in lanes {
            group.bench_function(format!("{lane}/{mode}"), |b| {
                if *lane == "steady" {
                    let (mut scanners, _caches) = trace.prepared(shared);
                    let calls = trace.legacy.selected(None);
                    b.iter_batched_ref(
                        || trace.strings(),
                        |strings| {
                            scanner_replay::replay(&mut scanners, strings, &calls);
                        },
                        BatchSize::LargeInput,
                    );
                } else {
                    b.iter_batched_ref(
                        || trace.strings(),
                        |strings| {
                            trace.lifecycle(
                                black_box(strings),
                                shared,
                                *lane == "first_use",
                                false,
                            );
                        },
                        BatchSize::LargeInput,
                    );
                }
            });
        }
        group.finish();
    }
    if let Some(criterion) = &mut criterion {
        criterion.final_summary();
    }
}
