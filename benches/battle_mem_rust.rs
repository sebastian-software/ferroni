// Process-isolated memory harness for Ferroni's Rust scanner path.
// Run via scripts/run-battle-memory.sh or:
// cargo bench --bench battle_mem_rust -- --nocapture

mod battle_mem_common;
mod grammar_loader;

use battle_mem_common::{load_typescript_workload, max_rss_bytes, print_result, scan_rust_lines};
use ferroni::scanner::Scanner;

fn main() {
    let workload = load_typescript_workload();
    let pattern_refs: Vec<&str> = workload.patterns.iter().map(|pattern| pattern.as_str()).collect();

    let mut scanner = Scanner::new(&pattern_refs).expect("Rust scanner compile failed");
    let compile_peak = max_rss_bytes();
    print_result("rust", "compile", &workload, compile_peak, None);

    let tokenize_stats = scan_rust_lines(&mut scanner, &workload.source);
    let total_peak = max_rss_bytes();
    print_result(
        "rust",
        "scan",
        &workload,
        total_peak,
        Some(&tokenize_stats),
    );
}
