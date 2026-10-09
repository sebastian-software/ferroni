//! Spike (refs #252): a fixed-iteration replay of one captured scanner
//! trace, for instruction counts (`/usr/bin/time -l`) and peak RSS with and
//! without the DFA pre-filter (`FERRONI_DFA_PREFILTER=0`).
//!
//! ```sh
//! cargo build --release --features dfa-prefilter --example dfa_prefilter_replay
//! /usr/bin/time -l target/release/examples/dfa_prefilter_replay benches/cpp_scanner/trace.json 20
//! ```
//!
//! Prints the scanner construction time, the replay time per iteration, and
//! the maximum RSS after construction and after the replay.

#[path = "../benches/cpp_scanner/mod.rs"]
#[allow(dead_code)]
mod scanner_replay;

use scanner_replay::{Corpus, replay};
use std::time::Instant;

fn max_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is a live, zeroed `rusage`; `getrusage` only writes it.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    assert_eq!(rc, 0);
    // SAFETY: the call returned 0, so the kernel initialized `usage`.
    let usage = unsafe { usage.assume_init() };
    #[cfg(target_os = "macos")]
    {
        usage.ru_maxrss as u64
    }
    #[cfg(not(target_os = "macos"))]
    {
        (usage.ru_maxrss as u64) * 1024
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .unwrap_or_else(|| "benches/cpp_scanner/trace.json".to_owned());
    let iterations: usize = args.next().map_or(10, |n| n.parse().expect("iterations"));
    let group: Option<usize> = args.next().map(|g| g.parse().expect("group"));
    let json = std::fs::read_to_string(&path).expect("trace exists");
    let corpus = Corpus::from_json(&json);
    let calls = corpus.selected(group);
    let rss_loaded = max_rss_bytes();

    let started = Instant::now();
    // `FERRONI_REPLAY_CACHED=1` builds the scanners through one pattern
    // cache, as a grammar loader does (ADR-006).
    let mut scanners = if std::env::var_os("FERRONI_REPLAY_CACHED").is_some() {
        corpus.cached_scanners()
    } else {
        corpus.scanners()
    };
    let construction = started.elapsed();
    let rss_built = max_rss_bytes();

    let started = Instant::now();
    for _ in 0..iterations {
        let strings = corpus.strings();
        replay(&mut scanners, &strings, &calls);
    }
    let elapsed = started.elapsed();
    let rss_replayed = max_rss_bytes();
    #[cfg(feature = "dfa-prefilter")]
    {
        let mut rows: Vec<(usize, usize, usize, usize, u64, usize)> = scanners
            .iter()
            .enumerate()
            .map(|(i, scanner)| {
                let r = scanner.dfa_prefilter_report();
                (
                    r.memory_usage,
                    i,
                    r.nfa_states,
                    r.dfa_cache_clears,
                    r.dfa_quits,
                    corpus.patterns[i].len(),
                )
            })
            .collect();
        let memory: usize = rows.iter().map(|r| r.0).sum();
        let states: usize = rows.iter().map(|r| r.2).sum();
        let clears: usize = rows.iter().map(|r| r.3).sum();
        let quits: u64 = rows.iter().map(|r| r.4).sum();
        rows.sort_unstable_by(|a, b| b.cmp(a));
        println!(
            "after replay: automata memory {:.1} MiB, NFA states {states} (max {}), overlapping cache clears {clears} in {} sets, DFA quits {quits}",
            memory as f64 / (1 << 20) as f64,
            rows.iter().map(|r| r.2).max().unwrap_or(0),
            rows.iter().filter(|r| r.3 > 0).count(),
        );
        for (memory, i, states, clears, quits, patterns) in rows.iter().take(6) {
            println!(
                "  set {i}: {:.2} MiB, {states} NFA states, {clears} clears, {quits} quits, {patterns} patterns",
                *memory as f64 / (1 << 20) as f64
            );
        }
    }
    println!(
        "{path}: {} scanners built in {:.1} ms; {} calls x {iterations} iterations in {:.1} ms ({:.2} ms per iteration); max RSS loaded {:.1} MiB, built {:.1} MiB, replayed {:.1} MiB",
        scanners.len(),
        construction.as_secs_f64() * 1e3,
        calls.len(),
        elapsed.as_secs_f64() * 1e3,
        elapsed.as_secs_f64() * 1e3 / iterations as f64,
        rss_loaded as f64 / (1 << 20) as f64,
        rss_built as f64 / (1 << 20) as f64,
        rss_replayed as f64 / (1 << 20) as f64,
    );
}
