//! Spike (refs #252): census of the DFA pre-filter over captured scanner
//! traces. Builds every scanner of each trace and prints how many patterns
//! the automata cover, which ones are searched on their own, what the seek
//! HIRs approximate, and what the automata cost to build and keep.
//!
//! ```sh
//! cargo run --release --features dfa-prefilter --example dfa_prefilter_census \
//!     [benches/<name>_scanner/trace.json ...]
//! ```

#[cfg(not(feature = "dfa-prefilter"))]
fn main() {
    eprintln!("build with --features dfa-prefilter");
}

#[cfg(feature = "dfa-prefilter")]
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

#[cfg(feature = "dfa-prefilter")]
fn main() {
    use ferroni::dfa_prefilter::approx;
    use ferroni::scanner::Scanner;
    use std::collections::BTreeMap;

    let mut traces: Vec<String> = std::env::args().skip(1).collect();
    if traces.is_empty() {
        traces = ["cpp", "java", "scss", "c", "php"]
            .iter()
            .map(|name| format!("benches/{name}_scanner/trace.json"))
            .collect();
    }
    for path in traces {
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("trace exists")).expect("JSON");
        let patterns: Vec<Vec<String>> =
            serde_json::from_value(json["scanners"].clone()).expect("scanner patterns");
        let references: Vec<Vec<&str>> = patterns
            .iter()
            .map(|scanner| scanner.iter().map(String::as_str).collect())
            .collect();

        let rss_before = max_rss_bytes();
        let started = std::time::Instant::now();
        let scanners: Vec<Scanner> = references
            .iter()
            .map(|refs| Scanner::new(refs).expect("patterns compile"))
            .collect();
        let construction = started.elapsed();
        let rss_after = max_rss_bytes();

        let mut sets_built = 0usize;
        let mut covered = 0usize;
        let mut own = 0usize;
        let mut total = 0usize;
        let mut memory = 0usize;
        let mut build_nanos = 0u64;
        let mut max_memory = 0usize;
        let mut breakdown = [0usize; 4];
        let mut flag_counts = [0usize; approx::NAMES.len()];
        let mut flagged_distinct: BTreeMap<&str, u32> = BTreeMap::new();
        let mut own_distinct: BTreeMap<&str, &'static str> = BTreeMap::new();
        for (scanner, patterns) in scanners.iter().zip(&patterns) {
            let report = scanner.dfa_prefilter_report();
            total += patterns.len();
            if report.built {
                sets_built += 1;
                covered += report.covered;
                memory += report.memory_usage;
                max_memory = max_memory.max(report.memory_usage);
                for (sum, part) in breakdown.iter_mut().zip(report.memory_breakdown) {
                    *sum += part;
                }
                build_nanos += report.build_nanos;
            }
            own += report.own.len();
            for &i in &report.own {
                let reason = if report.seeks[i].is_none() {
                    "no seek"
                } else if report.built {
                    "matches everywhere"
                } else {
                    "set not built"
                };
                own_distinct.entry(&patterns[i]).or_insert(reason);
            }
            for (i, &flags) in report.approximated.iter().enumerate() {
                flagged_distinct
                    .entry(&patterns[i])
                    .and_modify(|f| *f |= flags)
                    .or_insert(flags);
            }
        }
        for &flags in flagged_distinct.values() {
            for (bit, count) in flag_counts.iter_mut().enumerate() {
                if flags & (1 << bit) != 0 {
                    *count += 1;
                }
            }
        }
        let distinct: std::collections::BTreeSet<&str> =
            patterns.iter().flatten().map(String::as_str).collect();
        println!("== {path}");
        println!(
            "scanners {} (prefilter built for {}), patterns {} ({} distinct), covered {} ({:.1}%), own {} ({} distinct)",
            scanners.len(),
            sets_built,
            total,
            distinct.len(),
            covered,
            100.0 * covered as f64 / total.max(1) as f64,
            own,
            own_distinct.len()
        );
        println!(
            "construction {:.1} ms for all scanners, automata build {:.1} ms of it ({:.2} ms per set), automata memory {:.2} MiB total, {:.2} MiB max per set, RSS {:.1} -> {:.1} MiB",
            construction.as_secs_f64() * 1e3,
            build_nanos as f64 / 1e6,
            build_nanos as f64 / 1e6 / sets_built.max(1) as f64,
            memory as f64 / (1 << 20) as f64,
            max_memory as f64 / (1 << 20) as f64,
            rss_before as f64 / (1 << 20) as f64,
            rss_after as f64 / (1 << 20) as f64,
        );
        println!(
            "automata memory: meta {:.2} MiB, meta cache {:.2} MiB, overlapping DFA {:.2} MiB, its cache {:.2} MiB",
            breakdown[0] as f64 / (1 << 20) as f64,
            breakdown[1] as f64 / (1 << 20) as f64,
            breakdown[2] as f64 / (1 << 20) as f64,
            breakdown[3] as f64 / (1 << 20) as f64,
        );
        println!("approximations (distinct patterns with the flag):");
        for (name, count) in approx::NAMES.iter().zip(flag_counts) {
            if count > 0 {
                println!("  {count:5}  {name}");
            }
        }
        println!("own patterns (distinct, first 20):");
        for (pattern, reason) in own_distinct.iter().take(20) {
            let shown: String = pattern.chars().take(100).collect();
            println!("  [{reason}] {shown}");
        }
        if std::env::var_os("FERRONI_DFA_PREFILTER_TOP").is_some() {
            // NFA states of each distinct pattern's seek HIR on its own.
            let syntax = regex_automata::util::syntax::Config::new().utf8(false);
            let mut sizes: Vec<(usize, &str, usize)> = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for (scanner, patterns) in scanners.iter().zip(&patterns) {
                let report = scanner.dfa_prefilter_report();
                for (i, pattern) in patterns.iter().enumerate() {
                    if !seen.insert(pattern.as_str()) {
                        continue;
                    }
                    let Some(seek) = report.seeks[i].as_deref() else {
                        continue;
                    };
                    let states = regex_automata::nfa::thompson::Compiler::new()
                        .syntax(syntax)
                        .configure(
                            regex_automata::nfa::thompson::Config::new()
                                .utf8(false)
                                .which_captures(regex_automata::nfa::thompson::WhichCaptures::None),
                        )
                        .build(seek)
                        .map_or(usize::MAX, |nfa| nfa.states().len());
                    sizes.push((states, pattern, seek.len()));
                }
            }
            sizes.sort_unstable_by(|a, b| b.cmp(a));
            println!("largest seek NFAs (distinct patterns):");
            for (states, pattern, seek_len) in sizes.iter().take(12) {
                let shown: String = pattern.chars().take(110).collect();
                println!("  {states:7} states, seek {seek_len:6} chars: {shown}");
            }
            let total: usize = sizes
                .iter()
                .filter(|r| r.0 != usize::MAX)
                .map(|r| r.0)
                .sum();
            println!("  all distinct patterns: {total} states");
        }
        if std::env::var_os("FERRONI_DFA_PREFILTER_SEEKS").is_some() {
            println!("seek HIRs (distinct, first 40):");
            let mut seen = std::collections::BTreeSet::new();
            'outer: for (scanner, patterns) in scanners.iter().zip(&patterns) {
                let report = scanner.dfa_prefilter_report();
                for (i, pattern) in patterns.iter().enumerate() {
                    if seen.insert(pattern.as_str()) {
                        let seek = report.seeks[i].as_deref().unwrap_or("-");
                        let shown: String = seek.chars().take(160).collect();
                        println!(
                            "  {} => {}",
                            pattern.chars().take(80).collect::<String>(),
                            shown
                        );
                        if seen.len() >= 40 {
                            break 'outer;
                        }
                    }
                }
            }
        }
    }
}
