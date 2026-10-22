#![no_main]
//! Raw archives and mutations of structurally valid archives. Recompute the
//! corruption checks so mutations also reach the instruction/seek validators.
use ferroni::{PrecompiledPatterns, Scanner, ScannerConfig, ScannerPatternCache};
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;

const PATTERNS: &[&str] = &[
    r"(?<word>\w+)\s+\k<word>",
    r"(?i)(alpha|beta|gamma|delta)",
    r"(?<=foo)[^\r\n]*+(?=bar)",
    r"(a?){10,20}",
    r"\p{L}{2,4}",
    r"(a|b)\g<1>?",
    r"(?~END)",
    r"\X",
    r"[a-z][a-z][a-z]",
];
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize)
}
fn recheck(bytes: &mut [u8]) {
    let Some(index_len) = word(bytes, 12) else {
        return;
    };
    let Some(data_start) = 16usize.checked_add(index_len) else {
        return;
    };
    let Some(count) = word(bytes, 16).filter(|n| *n <= 100_000) else {
        return;
    };
    let mut at = 20usize;
    for _ in 0..count {
        let Some(key_len) = word(bytes, at) else {
            return;
        };
        let Some(next) = at.checked_add(4).and_then(|v| v.checked_add(key_len)) else {
            return;
        };
        at = next;
        let (Some(offset), Some(len)) = (word(bytes, at), word(bytes, at + 4)) else {
            return;
        };
        let Some(start) = data_start.checked_add(offset) else {
            return;
        };
        let Some(end) = start.checked_add(len) else {
            return;
        };
        let Some(payload) = bytes.get(start..end) else {
            return;
        };
        let hash = checksum(payload);
        let Some(slot) = bytes.get_mut(at + 8..at + 12) else {
            return;
        };
        slot.copy_from_slice(&hash.to_le_bytes());
        at += 12;
    }
}
// Mirrors the archive checksum of format version 2.
fn checksum(bytes: &[u8]) -> u32 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let (words, tail) = bytes.as_chunks::<8>();
    for word in words {
        hash = (hash ^ u64::from_le_bytes(*word)).wrapping_mul(PRIME);
        hash ^= hash >> 29;
    }
    for &byte in tail {
        hash = (hash ^ u64::from(byte)).wrapping_mul(PRIME);
    }
    (hash ^ (hash >> 32)) as u32
}
fuzz_target!(|data: &[u8]| {
    if data.len() > 16_384 {
        return;
    }
    let check = |bytes: Vec<u8>| {
        if let Ok(source) = PrecompiledPatterns::from_bytes(bytes) {
            for pattern in PATTERNS {
                let _ = source.validate_pattern(pattern, &ScannerConfig::default(), false);
            }
        }
    };
    check(data.to_vec());
    static SEED: OnceLock<Vec<u8>> = OnceLock::new();
    let seed = SEED.get_or_init(|| {
        let mut cache = ScannerPatternCache::new();
        Scanner::with_pattern_cache(PATTERNS, &ScannerConfig::default(), &mut cache).unwrap();
        cache.export_precompiled().unwrap()
    });
    let mut blob = seed.clone();
    for mutation in data.as_chunks::<3>().0 {
        let at = usize::from(u16::from_le_bytes([mutation[0], mutation[1]])) % blob.len();
        blob[at] ^= mutation[2];
    }
    recheck(&mut blob);
    check(blob);
});
