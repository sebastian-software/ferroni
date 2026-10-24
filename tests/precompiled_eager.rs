//! Public prepared Eager API, compatibility, fallback, and ownership contracts.
#![cfg(feature = "precompiled-eager")]

#[path = "support/precompiled_eager.rs"]
mod support;
use ferroni::prelude::*;
use support::*;

#[test]
fn frozen_native_pair_preserves_results_and_wire_format() {
    check_fixture();
    assert_eq!(
        &GROUPS[..8],
        format!("FPRT{PRECOMPILED_EAGER_FORMAT_VERSION:03}\0").as_bytes()
    );
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PrecompiledEager>();
    let mut producer = ScannerPatternCache::new();
    for patterns in SETS.iter().rev() {
        Scanner::with_pattern_cache(patterns, &config(), &mut producer).unwrap();
    }
    let exported = producer.export_precompiled_eager().unwrap();
    assert_eq!(exported.program_bytes(), PROGRAMS);
    assert_eq!(exported.group_bytes(), GROUPS);
}

#[test]
fn missing_groups_and_different_settings_compile_normally() {
    let source = fixture();
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager(source.clone());
    compare(&["missing", "[0-9]+extra"], &config(), &mut cache);
    let changed = config().options(ferroni::oniguruma::ONIG_OPTION_IGNORECASE);
    compare(SETS[0], &changed, &mut cache);
    for mode in [AccelerationMode::Auto, AccelerationMode::Off] {
        compare(SETS[0], &config().acceleration(mode), &mut cache);
    }
    assert_eq!(source.stats(), PrecompiledEagerStats::default());
}

#[test]
fn clones_share_observations_and_clearing_sources_preserves_live_scanners() {
    let source = fixture();
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager(source.clone());
    let mut scanner = Scanner::with_pattern_cache(SETS[0], &config(), &mut cache).unwrap();
    assert_eq!(source.stats().loaded_groups, 1);
    cache.clear();
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(source.stats().loaded_groups, 2);
    cache.clear_precompiled();
    cache.clear();
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(source.stats().loaded_groups, 2);
    assert_eq!(
        scanner
            .find_next_match("42", 0, ScannerFindOptions::NONE)
            .unwrap()
            .index,
        1
    );
    let mut other = ScannerPatternCache::new();
    other.add_precompiled_eager(source.clone());
    compare(SETS[1], &config(), &mut other);
    assert_eq!(source.stats().loaded_groups, 3);
}

#[test]
fn mismatched_pairs_versions_and_bounded_indexes_are_rejected() {
    let mut other = ScannerPatternCache::new();
    Scanner::with_pattern_cache(&["different"], &config(), &mut other).unwrap();
    assert!(PrecompiledEager::from_bytes(other.export_precompiled().unwrap(), GROUPS).is_err());
    for end in [0, 8, GROUPS.len() / 2, GROUPS.len() - 1] {
        assert!(PrecompiledEager::from_bytes(PROGRAMS, &GROUPS[..end]).is_err());
    }
    let mut unknown = GROUPS.to_vec();
    unknown[..8].copy_from_slice(b"FPRT999\0");
    assert!(matches!(
        PrecompiledEager::from_bytes(PROGRAMS, unknown),
        Err(PrecompiledError::UnsupportedFormat)
    ));
    let mut oversized = GROUPS.to_vec();
    oversized[13..17].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        PrecompiledEager::from_bytes(PROGRAMS, oversized),
        Err(PrecompiledError::ResourceLimit)
    ));
}

#[test]
fn damaged_records_fail_preflight_and_fall_back_during_loading() {
    let mut damaged = GROUPS.to_vec();
    *damaged.last_mut().unwrap() ^= 0xff;
    let source = PrecompiledEager::from_bytes(PROGRAMS, damaged).unwrap();
    assert!(source.validate_groups().is_err());
    assert_eq!(source.stats(), PrecompiledEagerStats::default());
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager(source.clone());
    for patterns in SETS {
        compare(patterns, &config(), &mut cache);
    }
    assert_eq!(source.stats().invalid_groups, 1);
    assert_eq!(source.stats().loaded_groups, SETS.len() - 1);
}

#[test]
fn empty_archives_are_valid_optional_sources() {
    let source = ScannerPatternCache::new()
        .export_precompiled_eager()
        .unwrap();
    assert!(source.is_empty());
    source.validate_groups().unwrap();
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager(source.clone());
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(source.stats(), PrecompiledEagerStats::default());
}

#[path = "support/eager_archive_mutations.rs"]
mod mutations;

#[test]
fn valid_checksums_do_not_bypass_instruction_validation() {
    use mutations::{repair_checksums, word};
    let mut damaged = GROUPS.to_vec();
    // Find a compact group and corrupt its first program's anchored entry.
    let mut at = 17;
    let mut changed = false;
    for _ in 0..word(&damaged, 13).unwrap() {
        let checksum_at = at + 4 + 4 * word(&damaged, at).unwrap();
        let start = checksum_at + 8;
        let end = start + word(&damaged, checksum_at + 4).unwrap();
        if damaged[start] == 6 {
            let mut program = start + 1;
            for _ in 0..3 {
                program += 4 + 4 * word(&damaged, program).unwrap();
            }
            program += 4; // Settle limit, then the program representation tag.
            assert_eq!(damaged[program], 0);
            damaged[program + 1..program + 5].copy_from_slice(&u32::MAX.to_le_bytes());
            changed = true;
            break;
        }
        at = end;
    }
    assert!(changed);
    repair_checksums(&mut damaged);
    let source = PrecompiledEager::from_bytes(PROGRAMS, damaged).unwrap();
    assert!(source.validate_groups().is_err());
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager(source.clone());
    for patterns in SETS {
        compare(patterns, &config(), &mut cache);
    }
    assert_eq!(source.stats().invalid_groups, 1);
}
