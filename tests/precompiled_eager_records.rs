//! Selective record imports, shared ownership, source identity and fallbacks.
#![cfg(feature = "precompiled-eager-records-prototype")]

#[path = "support/precompiled_eager.rs"]
mod support;
use ferroni::scanner::{PrecompiledBytes, PrecompiledEagerRecordKind, PrecompiledEagerRecords};
use ferroni::{AccelerationMode, Scanner, ScannerConfig, ScannerPatternCache};
use support::{GROUPS, PROGRAMS, SETS, compare, config, fixture};

fn source() -> PrecompiledEagerRecords {
    PrecompiledEagerRecords::from_index(&fixture().export_record_index().unwrap()).unwrap()
}
fn supply(source: &PrecompiledEagerRecords, patterns: &[&str]) {
    for record in patterns
        .iter()
        .filter_map(|pattern| source.program(pattern, &config(), false))
        .chain(source.group(patterns, &config(), false))
    {
        if !source.contains(&record) {
            let bytes = match record.kind() {
                PrecompiledEagerRecordKind::Program => PROGRAMS,
                PrecompiledEagerRecordKind::Group => GROUPS,
                _ => unreachable!(),
            };
            source
                .insert(
                    &record,
                    PrecompiledBytes::new(bytes[record.range()].to_vec()),
                )
                .unwrap();
        }
    }
}

#[test]
fn overlapping_lists_load_shared_programs_once_and_preserve_order_and_duplicates() {
    let source = source();
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager_records(source.clone());
    supply(&source, SETS[0]);
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(
        (source.stats().loaded_programs, source.stats().loaded_groups),
        (4, 1)
    );
    supply(&source, SETS[1]);
    compare(SETS[1], &config(), &mut cache);
    assert_eq!(cache.len(), 4);
    assert_eq!(
        (source.stats().loaded_programs, source.stats().loaded_groups),
        (4, 2)
    );
    let before = source.stats();
    compare(SETS[1], &config(), &mut cache);
    assert_eq!(source.stats(), before);
    for patterns in &SETS[2..] {
        supply(&source, patterns);
        compare(patterns, &config(), &mut cache);
    }
    assert_eq!(source.stats().loaded_programs, source.programs().len());
    assert_eq!(source.stats().loaded_groups, SETS.len());
    assert_eq!(
        (
            source.stats().invalid_programs,
            source.stats().invalid_groups
        ),
        (0, 0)
    );
    let exported = cache.export_precompiled_eager().unwrap();
    assert_eq!(exported.program_bytes(), PROGRAMS);
    assert_eq!(exported.group_bytes(), GROUPS);
}

#[test]
fn descriptors_are_source_scoped_and_payload_views_retain_their_owner() {
    use std::sync::Arc;
    let a = source();
    let b = source();
    let descriptor = a.programs().next().unwrap();
    let owner = Arc::new(PROGRAMS.to_vec());
    let weak = Arc::downgrade(&owner);
    let bytes = PrecompiledBytes::from_shared_owner(owner.clone())
        .slice(descriptor.range())
        .unwrap();
    assert!(b.insert(&descriptor, bytes.clone()).is_err());
    assert!(!b.contains(&descriptor));
    assert!(a.insert(&descriptor, bytes.slice(0..1).unwrap()).is_err());
    a.insert(&descriptor, bytes).unwrap();
    assert!(a.clone().contains(&descriptor));
    drop(owner);
    assert!(weak.upgrade().is_some());
    drop(a);
    assert!(weak.upgrade().is_none());
}

#[test]
fn missing_records_and_changed_settings_keep_normal_fallback() {
    let source = source();
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager_records(source.clone());
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(source.stats(), Default::default());
    supply(&source, SETS[0]);
    let changed = config().options(ferroni::oniguruma::ONIG_OPTION_IGNORECASE);
    assert!(source.program(SETS[0][0], &changed, false).is_none());
    assert!(source.group(SETS[0], &changed, false).is_none());
    compare(SETS[0], &changed, &mut cache);
    assert_eq!(source.stats(), Default::default());
    cache.clear();
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(source.stats().loaded_groups, 1);
    cache.clear_precompiled();
    cache.clear();
    let before = source.stats();
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(source.stats(), before);
}

#[test]
fn off_and_auto_ignore_prepared_groups() {
    for mode in [AccelerationMode::Off, AccelerationMode::Auto] {
        let source = source();
        supply(&source, SETS[0]);
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled_eager_records(source.clone());
        compare(
            SETS[0],
            &ScannerConfig::default().acceleration(mode),
            &mut cache,
        );
        assert_eq!(source.stats().loaded_groups, 0);
        assert_eq!(source.stats().invalid_groups, 0);
        assert_eq!(
            source.stats().loaded_programs,
            if mode == AccelerationMode::Auto { 4 } else { 0 }
        );
    }
}

#[test]
fn corrupt_program_and_group_payloads_fall_back_independently() {
    let source = source();
    supply(&source, SETS[0]);
    for record in [
        source.program(SETS[0][0], &config(), false).unwrap(),
        source.group(SETS[0], &config(), false).unwrap(),
    ] {
        let archive = if record.kind() == PrecompiledEagerRecordKind::Program {
            PROGRAMS
        } else {
            GROUPS
        };
        let mut bytes = archive[record.range()].to_vec();
        *bytes.last_mut().unwrap() ^= 0xff;
        source.insert(&record, bytes.into()).unwrap();
    }
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager_records(source.clone());
    compare(SETS[0], &config(), &mut cache);
    assert_eq!(
        (
            source.stats().loaded_programs,
            source.stats().invalid_programs
        ),
        (3, 1)
    );
    assert_eq!(
        (source.stats().loaded_groups, source.stats().invalid_groups),
        (0, 1)
    );
}

#[test]
fn malformed_manifests_are_rejected_before_any_source_is_attached() {
    let bytes = fixture().export_record_index().unwrap();
    for len in 0..bytes.len() {
        assert!(PrecompiledEagerRecords::from_index(&bytes[..len]).is_err());
    }
    for index in [0, 8, 12, 16, 20, bytes.len() - 1] {
        let mut changed = bytes.clone();
        changed[index] ^= 0xff;
        assert!(PrecompiledEagerRecords::from_index(&changed).is_err());
    }
    let source = PrecompiledEagerRecords::from_index(
        &ScannerPatternCache::new()
            .export_precompiled_eager()
            .unwrap()
            .export_record_index()
            .unwrap(),
    )
    .unwrap();
    assert_eq!((source.programs().len(), source.groups().len()), (0, 0));
}

#[test]
fn program_source_order_is_preserved_across_archive_and_record_imports() {
    for archive_first in [true, false] {
        let source = source();
        supply(&source, SETS[0]);
        let mut cache = ScannerPatternCache::new();
        if archive_first {
            cache.add_precompiled_eager(fixture());
        }
        cache.add_precompiled_eager_records(source.clone());
        if !archive_first {
            cache.add_precompiled_eager(fixture());
        }
        Scanner::with_pattern_cache(SETS[0], &config(), &mut cache).unwrap();
        assert_eq!(
            source.stats().loaded_programs,
            if archive_first { 0 } else { 4 }
        );
        assert_eq!(source.stats().loaded_groups, usize::from(archive_first));
    }
}

#[path = "support/eager_archive_mutations.rs"]
mod mutations;

#[test]
fn repaired_payload_checksums_still_require_valid_group_instructions() {
    use mutations::{repair_checksums, word};
    let mut damaged = GROUPS.to_vec();
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
            program += 4;
            assert_eq!(damaged[program], 0);
            damaged[program + 1..program + 5].copy_from_slice(&u32::MAX.to_le_bytes());
            changed = true;
            break;
        }
        at = end;
    }
    assert!(changed);
    repair_checksums(&mut damaged);
    let pair = ferroni::scanner::PrecompiledEager::from_bytes(PROGRAMS, damaged.clone()).unwrap();
    let source = PrecompiledEagerRecords::from_index(&pair.export_record_index().unwrap()).unwrap();
    for record in source.programs().chain(source.groups()) {
        let archive = if record.kind() == PrecompiledEagerRecordKind::Program {
            PROGRAMS
        } else {
            &damaged
        };
        source
            .insert(&record, archive[record.range()].to_vec().into())
            .unwrap();
    }
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager_records(source.clone());
    for patterns in SETS {
        compare(patterns, &config(), &mut cache);
    }
    assert_eq!(source.stats().invalid_groups, 1);
    assert_eq!(source.stats().loaded_groups, SETS.len() - 1);
    assert_eq!(source.stats().invalid_programs, 0);
}
