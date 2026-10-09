//! What the scanner's DFA pre-filter (ADR-008) covers and costs over the
//! captured scanner traces, and a fixed-iteration replay for instruction
//! counts and peak memory.
//!
//! ```sh
//! cargo build --release --example prefilter_census
//! # Coverage, automata sizes and construction cost per trace.
//! target/release/examples/prefilter_census census [TRACE.json ...]
//! # Per scanner group with calls: NFA states against the replay time with
//! # and without the pre-filter (min of REPS).
//! target/release/examples/prefilter_census groups TRACE.json [REPS]
//! # One replay for `/usr/bin/time -l` (instructions retired, max RSS), with
//! # the time of each iteration (the first ones warm the lazy DFAs up),
//! # optionally of one scanner group's calls.
//! /usr/bin/time -l target/release/examples/prefilter_census replay TRACE.json ITERATIONS on|off [GROUP]
//! ```

#[path = "../benches/cpp_scanner/mod.rs"]
mod scanner_replay;

use ferroni::scanner::{Scanner, ScannerConfig};
use scanner_replay::{Corpus, replay};
use std::time::Instant;

const TRACES: [&str; 5] = [
    "benches/cpp_scanner/trace.json",
    "benches/java_scanner/trace.json",
    "benches/scss_scanner/trace.json",
    "benches/c_scanner/trace.json",
    "benches/php_scanner/trace.json",
];

const USAGE: &str = "usage: prefilter_census census [TRACE.json ...] | groups TRACE.json [REPS] | replay TRACE.json ITERATIONS on|off [GROUP]";

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1 << 20) as f64
}

#[cfg(unix)]
fn max_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is a live, zeroed `rusage`; `getrusage` only writes it.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    assert_eq!(rc, 0);
    // SAFETY: the call returned 0, so the kernel initialized `usage`.
    let usage = unsafe { usage.assume_init() };
    if cfg!(target_os = "macos") {
        usage.ru_maxrss as u64
    } else {
        (usage.ru_maxrss as u64) * 1024
    }
}

#[cfg(not(unix))]
fn max_rss_bytes() -> u64 {
    0
}

fn load(path: &str) -> Corpus {
    Corpus::from_json(&std::fs::read_to_string(path).expect("trace exists"))
}

/// Coverage, automata sizes and construction cost of every scanner of each
/// trace, with the pre-filter and without.
fn census(traces: &[String]) {
    let with = ScannerConfig::default();
    let without = ScannerConfig::default().prefilter(false);
    for path in traces {
        let corpus = load(path);
        let rss_before = max_rss_bytes();
        let started = Instant::now();
        let plain = corpus.scanners_with(&without);
        let plain_construction = started.elapsed();
        drop(plain);
        let started = Instant::now();
        let scanners = corpus.scanners_with(&with);
        let construction = started.elapsed();
        let rss_after = max_rss_bytes();

        let stats: Vec<_> = scanners.iter().map(Scanner::prefilter_stats).collect();
        let built = stats.iter().filter(|s| s.built).count();
        let covered: usize = stats.iter().map(|s| s.covered).sum();
        let own: usize = stats.iter().filter(|s| s.built).map(|s| s.own).sum();
        let patterns: usize = corpus.patterns.iter().map(Vec::len).sum();
        let memory: usize = stats.iter().map(|s| s.memory_usage).sum();
        let max_states = stats.iter().map(|s| s.nfa_states).max().unwrap_or(0);
        let skipped: Vec<String> = stats
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.built)
            .map(|(i, _)| format!("{i} ({} patterns)", corpus.patterns[i].len()))
            .collect();
        println!("== {path}");
        println!(
            "scanners {} (automata built for {built}), patterns {patterns}, covered {covered} ({:.1}%), own {own} in sets with automata",
            scanners.len(),
            100.0 * covered as f64 / patterns.max(1) as f64,
        );
        println!(
            "construction {:.1} ms with the pre-filter, {:.1} ms without ({:.2}x); automata memory {:.2} MiB, largest set {max_states} NFA states; RSS {:.1} -> {:.1} MiB",
            construction.as_secs_f64() * 1e3,
            plain_construction.as_secs_f64() * 1e3,
            construction.as_secs_f64() / plain_construction.as_secs_f64().max(1e-9),
            mib(memory),
            mib(rss_before as usize),
            mib(rss_after as usize),
        );
        if !skipped.is_empty() {
            println!("sets without automata: {}", skipped.join(", "));
        }
    }
}

/// Per scanner group with calls: the automata's size against the replay
/// time with and without the pre-filter, the minimum of `reps` runs each,
/// interleaved.
fn groups(path: &str, reps: usize) {
    let corpus = load(path);
    let with = ScannerConfig::default();
    let without = ScannerConfig::default().prefilter(false);
    let mut filtered = corpus.scanners_with(&with);
    let mut plain = corpus.scanners_with(&without);
    let mut rows = Vec::new();
    for group in 0..corpus.patterns.len() {
        let calls = corpus.selected(Some(group));
        if calls.is_empty() {
            continue;
        }
        let mut best_with = f64::MAX;
        let mut best_without = f64::MAX;
        for _ in 0..reps {
            let strings = corpus.strings();
            let started = Instant::now();
            replay(&mut filtered, &strings, &calls);
            best_with = best_with.min(started.elapsed().as_secs_f64());
            let strings = corpus.strings();
            let started = Instant::now();
            replay(&mut plain, &strings, &calls);
            best_without = best_without.min(started.elapsed().as_secs_f64());
        }
        let stats = filtered[group].prefilter_stats();
        rows.push((
            stats.nfa_states,
            group,
            calls.len(),
            stats,
            best_with,
            best_without,
        ));
    }
    rows.sort_unstable_by_key(|row| row.0);
    println!(
        "{path}: {} groups with calls, min of {reps} replays each",
        rows.len()
    );
    println!(
        "{:>5} {:>6} {:>9} {:>6} {:>8} {:>7} {:>10} {:>10} {:>7}",
        "group", "calls", "nfa", "built", "mem MiB", "clears", "with us", "without us", "ratio"
    );
    for (states, group, calls, stats, best_with, best_without) in rows {
        println!(
            "{group:>5} {calls:>6} {states:>9} {:>6} {:>8.2} {:>7} {:>10.1} {:>10.1} {:>7.2}",
            stats.built,
            mib(stats.memory_usage),
            stats.cache_clears,
            best_with * 1e6,
            best_without * 1e6,
            best_with / best_without.max(1e-12),
        );
    }
}

/// A fixed number of replays of every call of the trace, for
/// `/usr/bin/time -l`: the construction and replay times and the maximum RSS
/// after each.
/// `replay_trace` through `find_next_match_utf16_with_id` with the subject's
/// index as the string id, the cache route a grammar loader takes for the
/// lines of a document, where the scanner may switch to its per-regex route.
fn replay_trace_with_ids(path: &str, iterations: usize, prefilter: bool) {
    let corpus = load(path);
    let calls = corpus.selected(None);
    let config = ScannerConfig::default().prefilter(prefilter);
    let mut scanners = corpus.scanners_with(&config);
    let started = Instant::now();
    let mut per_iteration = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let strings = corpus.strings();
        let iteration = Instant::now();
        for call in &calls {
            std::hint::black_box(scanners[call.scanner].find_next_match_utf16_with_id(
                &strings[call.subject],
                call.subject as u64 + 1,
                call.start_utf16,
                call.options,
            ));
        }
        per_iteration.push(iteration.elapsed().as_secs_f64() * 1e3);
    }
    let elapsed = started.elapsed();
    let shown: Vec<String> = per_iteration.iter().map(|ms| format!("{ms:.2}")).collect();
    println!("per iteration ms: {}", shown.join(" "));
    let stats = scanners.iter().fold((0u64, 0u64), |sum, scanner| {
        let stats = scanner.stats();
        (
            sum.0 + stats.route_cache_regset_calls,
            sum.1 + stats.route_cache_per_regex_calls,
        )
    });
    println!(
        "{path}: pre-filter {}; {} calls x {iterations} iterations with string ids in {:.1} ms ({:.2} ms per iteration); RegSet route {} calls, per-regex route {} calls",
        if prefilter { "on" } else { "off" },
        calls.len(),
        elapsed.as_secs_f64() * 1e3,
        elapsed.as_secs_f64() * 1e3 / iterations.max(1) as f64,
        stats.0,
        stats.1,
    );
}

fn replay_trace(path: &str, iterations: usize, prefilter: bool, group: Option<usize>) {
    let corpus = load(path);
    let calls = corpus.selected(group);
    let config = ScannerConfig::default().prefilter(prefilter);
    let rss_loaded = max_rss_bytes();
    let started = Instant::now();
    let mut scanners = corpus.scanners_with(&config);
    let construction = started.elapsed();
    let rss_built = max_rss_bytes();
    let started = Instant::now();
    let mut per_iteration = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let strings = corpus.strings();
        let iteration = Instant::now();
        replay(&mut scanners, &strings, &calls);
        per_iteration.push(iteration.elapsed().as_secs_f64() * 1e3);
    }
    let elapsed = started.elapsed();
    let shown: Vec<String> = per_iteration.iter().map(|ms| format!("{ms:.2}")).collect();
    println!("per iteration ms: {}", shown.join(" "));
    let rss_replayed = max_rss_bytes();
    let stats: Vec<_> = scanners.iter().map(Scanner::prefilter_stats).collect();
    let memory: usize = stats.iter().map(|s| s.memory_usage).sum();
    let clears: usize = stats.iter().map(|s| s.cache_clears).sum();
    println!(
        "{path}: pre-filter {}; {} scanners built in {:.1} ms; {} calls x {iterations} iterations in {:.1} ms ({:.2} ms per iteration); automata {:.1} MiB, {clears} cache clears; max RSS loaded {:.1} MiB, built {:.1} MiB, replayed {:.1} MiB",
        if prefilter { "on" } else { "off" },
        scanners.len(),
        construction.as_secs_f64() * 1e3,
        calls.len(),
        elapsed.as_secs_f64() * 1e3,
        elapsed.as_secs_f64() * 1e3 / iterations.max(1) as f64,
        mib(memory),
        mib(rss_loaded as usize),
        mib(rss_built as usize),
        mib(rss_replayed as usize),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("census") => {
            let traces: Vec<String> = if args.len() > 1 {
                args[1..].to_vec()
            } else {
                TRACES.iter().map(|&t| t.to_owned()).collect()
            };
            census(&traces);
        }
        Some("groups") if args.len() >= 2 => {
            let reps = args.get(2).map_or(5, |n| n.parse().expect("integer REPS"));
            groups(&args[1], reps);
        }
        Some("replay") if args.len() == 4 || args.len() == 5 => {
            let iterations = args[2].parse().expect("integer ITERATIONS");
            let prefilter = match args[3].as_str() {
                "on" => true,
                "off" => false,
                _ => panic!("{USAGE}"),
            };
            let group = args.get(4).map(|g| g.parse().expect("integer GROUP"));
            replay_trace(&args[1], iterations, prefilter, group);
        }
        Some("replay-id") if args.len() == 4 => {
            let iterations = args[2].parse().expect("integer ITERATIONS");
            let prefilter = match args[3].as_str() {
                "on" => true,
                "off" => false,
                _ => panic!("{USAGE}"),
            };
            replay_trace_with_ids(&args[1], iterations, prefilter);
        }
        _ => panic!("{USAGE}"),
    }
}
