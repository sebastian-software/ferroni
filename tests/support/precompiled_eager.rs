//! Fixed native-produced input shared by native, WASI, and browser-target checks.
use ferroni::prelude::*;

pub const SETS: &[&[&str]] = &[
    &[r"\b[a-z]+\b", r"(\d+)", r"(?<=prefix:)suffix", r"^end$"],
    &[r"(\d+)", r"\b[a-z]+\b", r"(\d+)"],
    &[r"(?<=x)y", r"(a)\1"],
    &[r"\A\z"],
];
pub const PROGRAMS: &[u8] = include_bytes!("../fixtures/precompiled-eager/patterns.programs");
pub const INDEX: &[u8] = include_bytes!("../fixtures/precompiled-eager/patterns.records");
pub const GROUPS: &[u8] = include_bytes!("../fixtures/precompiled-eager/patterns.portable");

pub fn config() -> ScannerConfig {
    ScannerConfig::default().acceleration(AccelerationMode::Eager)
}

pub fn fixture() -> PrecompiledEager {
    PrecompiledEager::from_bytes(PROGRAMS, GROUPS).unwrap()
}

pub fn compare(patterns: &[&str], config: &ScannerConfig, cache: &mut ScannerPatternCache) {
    let mut actual = Scanner::with_pattern_cache(patterns, config, cache).unwrap();
    let mut reference = Scanner::with_config(patterns, config).unwrap();
    for text in [
        "",
        "aa xy 12 end",
        "é💻 007 word",
        "end\n",
        "prefix:suffix",
        "xx\ny",
    ] {
        for start in text.char_indices().map(|(at, _)| at).chain([text.len()]) {
            for options in [ScannerFindOptions::NONE, ScannerFindOptions::from_bits(7)] {
                assert_eq!(
                    actual.find_next_match(text, start, options),
                    reference.find_next_match(text, start, options)
                );
            }
        }
        let string = OnigString::new(text);
        for start in 0..=string.utf16_len() {
            assert_eq!(
                actual.find_next_match_utf16(&string, start, ScannerFindOptions::NONE),
                reference.find_next_match_utf16(&string, start, ScannerFindOptions::NONE)
            );
        }
    }
}

pub fn check_record_fixture() {
    assert_eq!(PRECOMPILED_EAGER_INDEX_FORMAT_VERSION, 1);
    assert_eq!(fixture().export_record_index().unwrap(), INDEX);
    let source = PrecompiledEagerRecords::from_index(INDEX).unwrap();
    let programs = PrecompiledBytes::new(PROGRAMS.to_vec());
    let groups = PrecompiledBytes::new(GROUPS.to_vec());
    for record in source.programs().chain(source.groups()) {
        assert_eq!(source.validate_record(&record), Ok(false));
        let image = match record.kind() {
            PrecompiledEagerRecordKind::Program => &programs,
            PrecompiledEagerRecordKind::Group => &groups,
            _ => unreachable!(),
        };
        source
            .insert(&record, image.slice(record.range()).unwrap())
            .unwrap();
        assert_eq!(source.validate_record(&record), Ok(true));
    }
    assert_eq!(source.stats(), PrecompiledEagerRecordsStats::default());
    let mut cache = ScannerPatternCache::new();
    cache.add_precompiled_eager_records(source.clone());
    for patterns in SETS {
        compare(patterns, &config(), &mut cache);
    }
    assert_eq!(source.stats().loaded_programs, source.programs().len());
    assert_eq!(source.stats().loaded_groups, source.groups().len());
    assert_eq!(
        (
            source.stats().invalid_programs,
            source.stats().invalid_groups
        ),
        (0, 0)
    );
    assert_eq!(
        cache
            .export_precompiled_eager()
            .unwrap()
            .export_record_index()
            .unwrap(),
        INDEX
    );
}

pub fn check_fixture() {
    check_record_fixture();
    let mut bytes = PROGRAMS.to_vec();
    bytes.extend_from_slice(GROUPS);
    let image = PrecompiledBytes::new(bytes);
    let shared = PrecompiledEager::from_shared_bytes(
        image.slice(0..PROGRAMS.len()).unwrap(),
        image.slice(PROGRAMS.len()..image.len()).unwrap(),
    )
    .unwrap();
    for prepared in [fixture(), shared] {
        assert_eq!(prepared.len(), SETS.len());
        assert!(!prepared.is_empty());
        prepared.validate_groups().unwrap();
        assert_eq!(prepared.stats(), PrecompiledEagerStats::default());
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled_eager(prepared.clone());
        for patterns in SETS {
            compare(patterns, &config(), &mut cache);
        }
        assert_eq!(prepared.stats().loaded_groups, SETS.len());
        assert_eq!(prepared.stats().invalid_groups, 0);
        let exported = cache.export_precompiled_eager().unwrap();
        assert_eq!(exported.program_bytes(), PROGRAMS);
        assert_eq!(exported.group_bytes(), GROUPS);
    }
}
