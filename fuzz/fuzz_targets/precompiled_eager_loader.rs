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
fn seed_index() -> &'static Vec<u8> {
    static INDEX: OnceLock<Vec<u8>> = OnceLock::new();
    INDEX.get_or_init(|| seed().export_record_index().unwrap())
}
fn populate(source: &PrecompiledEagerRecords, prepared: &PrecompiledEager) {
    let programs = PrecompiledBytes::new(prepared.program_bytes().to_vec());
    let groups = PrecompiledBytes::new(prepared.group_bytes().to_vec());
    for record in source.programs().chain(source.groups()) {
        let image = match record.kind() {
            PrecompiledEagerRecordKind::Program => &programs,
            PrecompiledEagerRecordKind::Group => &groups,
            _ => continue,
        };
        if let Ok(bytes) = image.slice(record.range()) {
            source.insert(&record, bytes).unwrap();
            let _ = source.validate_record(&record);
        }
    }
}
fn check_index(index: &[u8], prepared: &PrecompiledEager) {
    if let Ok(source) = PrecompiledEagerRecords::from_index(index) {
        // Validation is safe for arbitrary metadata/instructions; execution is
        // reserved for the unmutated producer below.
        populate(&source, prepared);
    }
}
thread_local! {
    static SCANNERS: RefCell<(Scanner, Scanner, Scanner)> = RefCell::new({
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled_eager(seed().clone());
        let prepared = Scanner::with_pattern_cache(PATTERNS, &config(), &mut cache).unwrap();
        assert!(seed().stats().loaded_groups > 0);
        let records = PrecompiledEagerRecords::from_index(seed_index()).unwrap();
        populate(&records, seed());
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled_eager_records(records.clone());
        let selective = Scanner::with_pattern_cache(PATTERNS, &config(), &mut cache).unwrap();
        assert_eq!(records.stats().loaded_groups, 1);
        (prepared, selective, Scanner::with_config(PATTERNS, &config()).unwrap())
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
            check_index(&prepared.export_record_index().unwrap(), &prepared);
        }
    };
    check_index(data, seed());
    let mut index = seed_index().clone();
    for chunk in data.as_chunks::<3>().0 {
        let at = usize::from(u16::from_le_bytes([chunk[0], chunk[1]])) % index.len();
        index[at] ^= chunk[2];
    }
    let checksum = mutations::checksum(&index[12..]);
    index[8..12].copy_from_slice(&checksum.to_le_bytes());
    check_index(&index, seed());
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
        let (prepared, selective, reference) = &mut *scanners.borrow_mut();
        for start in text
            .char_indices()
            .map(|(at, _)| at)
            .take(8)
            .chain([text.len()])
        {
            for options in [ScannerFindOptions::NONE, ScannerFindOptions::from_bits(7)] {
                let expected = reference.find_next_match(&text, start, options);
                assert_eq!(prepared.find_next_match(&text, start, options), expected);
                assert_eq!(selective.find_next_match(&text, start, options), expected);
            }
        }
        let utf16 = OnigString::new(text.as_ref());
        for start in [0, utf16.utf16_len() / 2, utf16.utf16_len()] {
            let expected = reference.find_next_match_utf16(&utf16, start, ScannerFindOptions::NONE);
            assert_eq!(
                prepared.find_next_match_utf16(&utf16, start, ScannerFindOptions::NONE),
                expected
            );
            assert_eq!(
                selective.find_next_match_utf16(&utf16, start, ScannerFindOptions::NONE),
                expected
            );
        }
    });
});
