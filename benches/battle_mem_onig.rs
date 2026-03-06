// Process-isolated memory harness for the Oniguruma scanner comparison path.
// Run via scripts/run-battle-memory.sh or:
// cargo bench --features ffi --bench battle_mem_onig -- --nocapture

mod battle_mem_common;
mod grammar_loader;

use battle_mem_common::{load_typescript_workload, max_rss_bytes, print_result, TokenizeStats};
use ferroni::ffi;

fn main() {
    let workload = load_typescript_workload();
    let pattern_bytes: Vec<Vec<u8>> = workload
        .patterns
        .iter()
        .map(|pattern| pattern.as_bytes().to_vec())
        .collect();
    let pattern_refs: Vec<&[u8]> = pattern_bytes.iter().map(|pattern| pattern.as_slice()).collect();

    let scanner = ffi::CScanner::new(&pattern_refs).expect("Oniguruma scanner compile failed");
    let compile_peak = max_rss_bytes();
    print_result("oniguruma", "compile", &workload, compile_peak, None);

    let tokenize_stats = scan_c_lines(&scanner, &workload.source);
    let total_peak = max_rss_bytes();
    print_result(
        "oniguruma",
        "scan",
        &workload,
        total_peak,
        Some(&tokenize_stats),
    );
}

fn scan_c_lines(scanner: &ffi::CScanner, source: &str) -> TokenizeStats {
    let mut line_count = 0usize;
    let mut match_count = 0u64;

    for (line_idx, line) in source.split_inclusive('\n').enumerate() {
        line_count += 1;
        let bytes = line.as_bytes();
        if scanner.find_next_match(bytes, line_idx as i32, 0).is_some() {
            match_count += 1;
        }
    }

    TokenizeStats {
        line_count,
        match_count,
    }
}
