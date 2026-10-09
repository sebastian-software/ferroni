//! What the scanner's DFA pre-filter (ADR-008) covers and costs over the
//! captured scanner traces, and a fixed-iteration replay for instruction
//! counts and peak memory.
//!
//! ```sh
//! cargo build --release --example prefilter_census
//! # Construction time and live heap per trace, uncached and through one
//! # pattern cache, with and without the pre-filter; the first replay,
//! # which builds the automata; what the automata and the seeks hold.
//! target/release/examples/prefilter_census census [REPS] [TRACE.json ...]
//! # Per scanner group with calls: NFA states against the replay time with
//! # and without the pre-filter (min of REPS).
//! target/release/examples/prefilter_census groups TRACE.json [REPS]
//! # One replay for `/usr/bin/time -l` (instructions retired, max RSS), with
//! # the time of each iteration (the first one builds the automata and
//! # warms the lazy DFAs up), optionally of one scanner group's calls, and
//! # optionally through one pattern cache.
//! /usr/bin/time -l target/release/examples/prefilter_census replay TRACE.json ITERATIONS on|off [--group N] [--cached]
//! ```

#[path = "../benches/cpp_scanner/mod.rs"]
mod scanner_replay;

use ferroni::scanner::{PrefilterStats, Scanner, ScannerConfig, ScannerPatternCache};
use scanner_replay::{Corpus, replay};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const TRACES: [&str; 5] = [
    "benches/cpp_scanner/trace.json",
    "benches/java_scanner/trace.json",
    "benches/scss_scanner/trace.json",
    "benches/c_scanner/trace.json",
    "benches/php_scanner/trace.json",
];

const USAGE: &str = "usage: prefilter_census census [REPS] [TRACE.json ...] | groups TRACE.json [REPS] | replay TRACE.json ITERATIONS on|off [--group N] [--cached] | replay-id TRACE.json ITERATIONS on|off";

/// The system allocator, counting the bytes that are live.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every method forwards to `System` with the layout it was given
// and only keeps a count of the bytes handed out.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract is forwarded unchanged.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract is forwarded unchanged.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract is forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller's contract is forwarded unchanged.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            LIVE.fetch_add(new_size, Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Live heap bytes.
fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1 << 20) as f64
}

fn kib(bytes: usize) -> f64 {
    bytes as f64 / 1024.0
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

/// The minimum over `reps` constructions, in milliseconds, and the heap the
/// last one holds above `base`, with its scanners.
fn construction(
    reps: usize,
    base: usize,
    mut build: impl FnMut() -> Vec<Scanner>,
) -> (f64, usize, Vec<Scanner>) {
    let mut best = f64::MAX;
    let mut scanners = Vec::new();
    for _ in 0..reps {
        drop(std::mem::take(&mut scanners));
        let started = Instant::now();
        scanners = build();
        best = best.min(started.elapsed().as_secs_f64() * 1e3);
    }
    (best, live() - base, scanners)
}

/// What the automata of `scanners` hold: sets with automata, their memory
/// (each distinct pattern list once, since scanners built from one cache
/// over the same patterns share theirs), the scanners' own caches, and the
/// cache clears.
struct Automata {
    built: usize,
    unique: usize,
    caches: usize,
    /// The largest cache of one scanner, and how many scanners hold more
    /// than 256 KiB, 512 KiB and 1 MiB in theirs.
    largest_cache: usize,
    caches_over: [usize; 3],
    clears: usize,
    largest: usize,
}

fn automata(corpus: &Corpus, scanners: &[Scanner], shared: bool) -> Automata {
    let stats: Vec<PrefilterStats> = scanners.iter().map(Scanner::prefilter_stats).collect();
    let mut seen = HashSet::new();
    let mut unique = 0;
    for (patterns, stats) in corpus.patterns.iter().zip(&stats) {
        let own = stats.memory_usage - stats.cache_memory_usage;
        if !shared || seen.insert(patterns) {
            unique += own;
        }
    }
    let over = |bytes: usize| {
        stats
            .iter()
            .filter(|s| s.cache_memory_usage > bytes)
            .count()
    };
    Automata {
        built: stats.iter().filter(|s| s.built).count(),
        unique,
        caches: stats.iter().map(|s| s.cache_memory_usage).sum(),
        largest_cache: stats
            .iter()
            .map(|s| s.cache_memory_usage)
            .max()
            .unwrap_or(0),
        caches_over: [over(256 << 10), over(512 << 10), over(1 << 20)],
        clears: stats.iter().map(|s| s.cache_clears).sum(),
        largest: stats.iter().map(|s| s.nfa_states).max().unwrap_or(0),
    }
}

impl Automata {
    fn caches_shown(&self) -> String {
        format!(
            "caches {:.1} MiB (largest {:.0} KiB; {} over 256 KiB, {} over 512 KiB, {} over 1 MiB), {} cache clears",
            mib(self.caches),
            kib(self.largest_cache),
            self.caches_over[0],
            self.caches_over[1],
            self.caches_over[2],
            self.clears,
        )
    }
}

/// One replay of every call, in milliseconds.
fn one_replay(corpus: &Corpus, scanners: &mut [Scanner]) -> f64 {
    let calls = corpus.selected(None);
    let strings = corpus.strings();
    let started = Instant::now();
    replay(scanners, &strings, &calls);
    started.elapsed().as_secs_f64() * 1e3
}

/// Construction time and live heap of every scanner of each trace, uncached
/// and through one pattern cache, with the pre-filter and without; the
/// first replay, which builds the automata of the sets it touches, and
/// what they hold; what the seeks cost in the cache.
fn census(reps: usize, traces: &[String]) {
    let with = ScannerConfig::default();
    let without = ScannerConfig::default().prefilter(false);
    for path in traces {
        let corpus = load(path);
        let patterns: usize = corpus.patterns.iter().map(Vec::len).sum();
        let distinct_lists = corpus.patterns.iter().collect::<HashSet<_>>().len();
        let distinct: HashSet<&str> = corpus
            .patterns
            .iter()
            .flatten()
            .map(String::as_str)
            .collect();
        println!("== {path}");
        println!(
            "sets {} ({distinct_lists} distinct pattern lists), patterns {patterns} ({} distinct), calls {}; min of {reps} constructions",
            corpus.patterns.len(),
            distinct.len(),
            corpus.calls.len(),
        );
        let base = live();

        // Uncached, without the pre-filter.
        let (ms, heap, scanners) = construction(reps, base, || corpus.scanners_with(&without));
        println!(
            "uncached, pre-filter off: {ms:>7.1} ms, heap {:>6.1} MiB",
            mib(heap)
        );
        drop(scanners);

        // Uncached, with the pre-filter.
        let (ms, heap, mut scanners) = construction(reps, base, || corpus.scanners_with(&with));
        let first = one_replay(&corpus, &mut scanners);
        let second = one_replay(&corpus, &mut scanners);
        let heap_replayed = live() - base;
        let a = automata(&corpus, &scanners, false);
        println!(
            "uncached, pre-filter on:  {ms:>7.1} ms, heap {:>6.1} MiB; first replay {first:.1} ms (second {second:.1} ms), heap {:.1} MiB; automata built for {} sets: {:.1} MiB, {}, largest set {} NFA states",
            mib(heap),
            mib(heap_replayed),
            a.built,
            mib(a.unique),
            a.caches_shown(),
            a.largest,
        );
        drop(scanners);

        // Through one pattern cache, without the pre-filter.
        let mut cache = ScannerPatternCache::new();
        let (ms, heap, scanners) = construction(reps, base, || {
            cache.clear();
            corpus.cached_scanners_with(&without, &mut cache)
        });
        drop(scanners);
        let cache_off = live() - base;
        println!(
            "cached, pre-filter off:   {ms:>7.1} ms, heap {:>6.1} MiB (the cache alone {:.1} MiB)",
            mib(heap),
            mib(cache_off),
        );
        drop(cache);

        // Through one pattern cache, with the pre-filter.
        let mut cache = ScannerPatternCache::new();
        let (ms, heap, mut scanners) = construction(reps, base, || {
            cache.clear();
            corpus.cached_scanners_with(&with, &mut cache)
        });
        // The cache alone, with the scanners never searched: the patterns
        // and their seeks, no automata.
        let cache_on = {
            let probe = ScannerPatternCache::new();
            let mut rebuilt = ScannerPatternCache::new();
            drop(probe);
            let before = live();
            let scanners = corpus.cached_scanners_with(&with, &mut rebuilt);
            drop(scanners);
            let alone = live() - before;
            drop(rebuilt);
            alone
        };
        let first = one_replay(&corpus, &mut scanners);
        let second = one_replay(&corpus, &mut scanners);
        let heap_replayed = live() - base;
        let a = automata(&corpus, &scanners, true);
        drop(scanners);
        let cache_after = live() - base;
        println!(
            "cached, pre-filter on:    {ms:>7.1} ms, heap {:>6.1} MiB (the cache alone {:.1} MiB: seeks {:.1} MiB, {:.1} KiB per distinct pattern); first replay {first:.1} ms (second {second:.1} ms), heap {:.1} MiB; automata built for {} sets: {:.1} MiB over the distinct lists, {}; the cache alone after the replay {:.1} MiB",
            mib(heap),
            mib(cache_on),
            mib(cache_on.saturating_sub(cache_off)),
            kib(cache_on.saturating_sub(cache_off)) / distinct.len().max(1) as f64,
            mib(heap_replayed),
            a.built,
            mib(a.unique),
            a.caches_shown(),
            mib(cache_after),
        );
        drop(cache);
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

/// A fixed number of replays of every call of the trace, for
/// `/usr/bin/time -l`: the construction and replay times, the live heap and
/// the maximum RSS after each.
fn replay_trace(
    path: &str,
    iterations: usize,
    prefilter: bool,
    group: Option<usize>,
    cached: bool,
) {
    let corpus = load(path);
    let calls = corpus.selected(group);
    let config = ScannerConfig::default().prefilter(prefilter);
    let rss_loaded = max_rss_bytes();
    let heap_loaded = live();
    let mut cache = ScannerPatternCache::new();
    let started = Instant::now();
    let mut scanners = if cached {
        corpus.cached_scanners_with(&config, &mut cache)
    } else {
        corpus.scanners_with(&config)
    };
    let construction = started.elapsed();
    let rss_built = max_rss_bytes();
    let heap_built = live() - heap_loaded;
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
    let heap_replayed = live() - heap_loaded;
    let a = automata(&corpus, &scanners, cached);
    println!(
        "{path}: pre-filter {}, {}; {} scanners built in {:.1} ms; {} calls x {iterations} iterations in {:.1} ms ({:.2} ms per iteration); automata built for {} sets: {:.1} MiB, {}; heap built {:.1} MiB, replayed {:.1} MiB; max RSS loaded {:.1} MiB, built {:.1} MiB, replayed {:.1} MiB",
        if prefilter { "on" } else { "off" },
        if cached {
            "through one pattern cache"
        } else {
            "uncached"
        },
        scanners.len(),
        construction.as_secs_f64() * 1e3,
        calls.len(),
        elapsed.as_secs_f64() * 1e3,
        elapsed.as_secs_f64() * 1e3 / iterations.max(1) as f64,
        a.built,
        mib(a.unique),
        a.caches_shown(),
        mib(heap_built),
        mib(heap_replayed),
        mib(rss_loaded as usize),
        mib(rss_built as usize),
        mib(rss_replayed as usize),
    );
    drop(scanners);
    drop(cache);
}

fn on_or_off(arg: &str) -> bool {
    match arg {
        "on" => true,
        "off" => false,
        _ => panic!("{USAGE}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("census") => {
            let (reps, rest) = match args.get(1).and_then(|n| n.parse::<usize>().ok()) {
                Some(reps) => (reps, &args[2..]),
                None => (3, &args[1..]),
            };
            let traces: Vec<String> = if rest.is_empty() {
                TRACES.iter().map(|&t| t.to_owned()).collect()
            } else {
                rest.to_vec()
            };
            census(reps, &traces);
        }
        Some("groups") if args.len() >= 2 => {
            let reps = args.get(2).map_or(5, |n| n.parse().expect("integer REPS"));
            groups(&args[1], reps);
        }
        Some("replay") if args.len() >= 4 => {
            let iterations = args[2].parse().expect("integer ITERATIONS");
            let prefilter = on_or_off(&args[3]);
            let mut group = None;
            let mut cached = false;
            let mut rest = args[4..].iter();
            while let Some(flag) = rest.next() {
                match flag.as_str() {
                    "--group" => {
                        group = Some(rest.next().expect("GROUP").parse().expect("integer GROUP"));
                    }
                    "--cached" => cached = true,
                    _ => panic!("{USAGE}"),
                }
            }
            replay_trace(&args[1], iterations, prefilter, group, cached);
        }
        Some("replay-id") if args.len() == 4 => {
            let iterations = args[2].parse().expect("integer ITERATIONS");
            replay_trace_with_ids(&args[1], iterations, on_or_off(&args[3]));
        }
        _ => panic!("{USAGE}"),
    }
}
