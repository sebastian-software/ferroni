#![no_main]
//! Decode arbitrary groups with repaired checksums, without executing mutated
//! instructions. Separately compare trusted prepared execution on random text.
use ferroni::prelude::*;
use libfuzzer_sys::fuzz_target;
use std::{
    cell::RefCell,
    sync::{Arc, OnceLock},
};

#[path = "../../tests/support/eager_archive_mutations.rs"]
mod mutations;

const PATTERNS: &[&str] = &[
    r"\b[a-z]+\b",
    r"(\d+)",
    r"(?<=x)y",
    r"(a)\1",
    r"^end$",
    r"[^\r\n]*END",
];

fn config() -> ScannerConfig {
    ScannerConfig::default().acceleration(AccelerationMode::Eager)
}
fn seed() -> &'static PrecompiledEager {
    static SEED: OnceLock<PrecompiledEager> = OnceLock::new();
    SEED.get_or_init(|| {
        let mut cache = ScannerPatternCache::new();
        Scanner::with_pattern_cache(PATTERNS, &config(), &mut cache).unwrap();
        cache.export_precompiled_eager().unwrap()
    })
}
thread_local! {
    static SCANNERS: RefCell<(Scanner, Scanner)> = RefCell::new({
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled_eager(seed().clone());
        let prepared = Scanner::with_pattern_cache(PATTERNS, &config(), &mut cache).unwrap();
        assert!(seed().stats().loaded_groups > 0);
        (prepared, Scanner::with_config(PATTERNS, &config()).unwrap())
    });
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 16_384 {
        return;
    }
    let check = |groups: Vec<u8>| {
        // Program bytecode stays trusted. Never execute mutated portable code:
        // structural validity alone does not prove the claimed regex semantics.
        if let Ok(prepared) =
            PrecompiledEager::from_bytes(Arc::<[u8]>::from(seed().program_bytes()), groups)
        {
            let _ = prepared.validate_groups();
        }
    };
    check(data.to_vec());
    let mut groups = seed().group_bytes().to_vec();
    for chunk in data.as_chunks::<3>().0 {
        let at = usize::from(u16::from_le_bytes([chunk[0], chunk[1]])) % groups.len();
        groups[at] ^= chunk[2];
    }
    mutations::repair_checksums(&mut groups);
    check(groups);
    let text = String::from_utf8_lossy(&data[..data.len().min(128)]);
    SCANNERS.with(|scanners| {
        let (prepared, reference) = &mut *scanners.borrow_mut();
        for start in text
            .char_indices()
            .map(|(at, _)| at)
            .take(8)
            .chain([text.len()])
        {
            for options in [ScannerFindOptions::NONE, ScannerFindOptions::from_bits(7)] {
                assert_eq!(
                    prepared.find_next_match(&text, start, options),
                    reference.find_next_match(&text, start, options)
                );
            }
        }
        let utf16 = OnigString::new(text.as_ref());
        for start in [0, utf16.utf16_len() / 2, utf16.utf16_len()] {
            assert_eq!(
                prepared.find_next_match_utf16(&utf16, start, ScannerFindOptions::NONE),
                reference.find_next_match_utf16(&utf16, start, ScannerFindOptions::NONE)
            );
        }
    });
});
