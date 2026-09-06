use ferroni::scanner::{Scanner, ScannerFindOptions};
use std::fmt::Write;
use std::fs;
use std::mem::MaybeUninit;
use std::path::PathBuf;

pub const SCENARIO_NAME: &str = "typescript_scanner_large";

pub struct MemoryWorkload {
    pub patterns: Vec<String>,
    pub source: String,
    pub source_bytes: usize,
    pub source_lines: usize,
    pub source_utf16_units: usize,
    pub repeat_blocks: usize,
}

pub struct TokenizeStats {
    pub line_count: usize,
    pub match_count: u64,
}

pub fn load_typescript_workload() -> MemoryWorkload {
    let patterns = crate::grammar_loader::typescript_patterns();
    let repeat_blocks = read_repeat_blocks();
    let source = build_large_typescript_source(repeat_blocks);
    let source_bytes = source.len();
    let source_lines = source.lines().count();
    let source_utf16_units = source.encode_utf16().count();

    MemoryWorkload {
        patterns,
        source,
        source_bytes,
        source_lines,
        source_utf16_units,
        repeat_blocks,
    }
}

#[allow(dead_code)]
pub fn scan_rust_lines(scanner: &mut Scanner, source: &str) -> TokenizeStats {
    let mut line_count = 0usize;
    let mut match_count = 0u64;

    for (line_idx, line) in source.split_inclusive('\n').enumerate() {
        line_count += 1;
        if scanner
            .find_next_match_with_id(line, line_idx as u64, 0, ScannerFindOptions::NONE)
            .is_some()
        {
            match_count += 1;
        }
    }

    TokenizeStats {
        line_count,
        match_count,
    }
}

pub fn max_rss_bytes() -> u64 {
    let mut usage = MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is a live, zeroed `rusage` allocation, and `getrusage`
    // only writes through the pointer it is given.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    assert_eq!(rc, 0, "getrusage failed");
    // SAFETY: the call above returned 0, so the kernel initialized `usage`.
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

pub fn print_result(
    engine: &str,
    phase: &str,
    workload: &MemoryWorkload,
    peak_rss_bytes: u64,
    token_stats: Option<&TokenizeStats>,
) {
    print!(
        "RESULT engine={engine} scenario={} phase={phase} patterns={} repeat_blocks={} source_bytes={} source_lines={} source_utf16_units={} peak_rss_bytes={peak_rss_bytes}",
        SCENARIO_NAME,
        workload.patterns.len(),
        workload.repeat_blocks,
        workload.source_bytes,
        workload.source_lines,
        workload.source_utf16_units,
    );

    if let Some(stats) = token_stats {
        print!(
            " scanned_lines={} matched_lines={}",
            stats.line_count, stats.match_count
        );
    }

    println!();
}

fn build_large_typescript_source(repeat_blocks: usize) -> String {
    let mut source = String::with_capacity(repeat_blocks * 90);
    source.push_str("import { fetchUsers } from \"./api\";\n");
    source.push_str("type UserRecord = { id: number; email: string; active: boolean };\n");

    for i in 0..repeat_blocks {
        let _ = writeln!(
            source,
            "const result = await fetchUsers({{ limit: 100, offset: {i} }}); // API call"
        );
    }

    source
}

fn read_repeat_blocks() -> usize {
    let metadata_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/battle_inputs.toml");
    let contents = fs::read_to_string(&metadata_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", metadata_path.display()));
    let mut in_memory_workload = false;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') {
            in_memory_workload = line == "[memory_workload]";
            continue;
        }

        if !in_memory_workload {
            continue;
        }

        if let Some(value) = line.strip_prefix("repeat_blocks =") {
            return value
                .trim()
                .parse::<usize>()
                .expect("repeat_blocks must be a positive integer");
        }
    }

    panic!(
        "missing [memory_workload].repeat_blocks in {}",
        metadata_path.display()
    );
}
