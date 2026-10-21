//! Regression tests for native scanner cache behavior.

#![cfg(feature = "ffi")]

use ferroni::ffi::CScanner;
use ferroni::scanner::{Scanner, ScannerFindOptions};

#[test]
fn native_scanner_first_long_search_does_not_reuse_an_empty_cache() {
    let text = format!("{}needle", "x".repeat(994));
    assert_eq!(text.len(), 1_000);

    let mut rust_scanner = Scanner::new(&["needle"]).expect("Rust scanner compiles");
    let rust_match = rust_scanner
        .find_next_match_with_id(&text, 0, 0, ScannerFindOptions::NONE)
        .expect("Rust scanner finds the match");
    let rust_result = (
        rust_match.index,
        rust_match
            .captures()
            .iter()
            .map(|capture| (capture.start as i32, capture.end as i32))
            .collect::<Vec<_>>(),
    );

    let c_scanner = CScanner::new(&[b"needle"]).expect("C scanner compiles");
    let c_result = c_scanner
        .find_next_match(text.as_bytes(), 0, 0)
        .expect("native scanner finds the match on its first cache-id-zero search");

    assert_eq!(rust_result, (0, vec![(994, 1_000)]));
    assert_eq!(c_result, rust_result);
}
