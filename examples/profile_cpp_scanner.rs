//! Bounded, warmed scanner replay for external CPU profilers.
#[path = "../benches/cpp_scanner/mod.rs"]
mod cpp_scanner;
use cpp_scanner::{Corpus, replay};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.len() <= 2,
        "Usage: profile_cpp_scanner [SECONDS] [GROUP_ID]"
    );
    let seconds: u64 = args
        .first()
        .map_or(20, |s| s.parse().expect("integer seconds"));
    assert!((1..=300).contains(&seconds));
    let selected = args.get(1).map(|s| s.parse().expect("integer group ID"));
    let corpus = Corpus::load();
    corpus.validate();
    let calls = corpus.selected(selected);
    let mut scanners = corpus.scanners();
    for _ in 0..5 {
        replay(&mut scanners, &corpus.strings(), &calls);
    }
    eprintln!("Warm workload ready; pid={}", std::process::id());
    let wall = Instant::now();
    let mut timings_ns = Vec::new();
    while wall.elapsed() < Duration::from_secs(seconds) {
        let strings = corpus.strings();
        let start = Instant::now();
        replay(&mut scanners, &strings, &calls);
        timings_ns.push(start.elapsed().as_nanos());
    }
    println!(
        "{}",
        serde_json::json!({"group": selected, "calls_per_replay": calls.len(), "timings_ns": timings_ns, "wall_seconds": wall.elapsed().as_secs_f64(), "ferroni_version": env!("CARGO_PKG_VERSION"), "c_reference_validated": cfg!(feature="ffi")})
    );
}
