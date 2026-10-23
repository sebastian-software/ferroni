//! Ordered scanner-group archive round trips and bounded decoder failures.

#![cfg(feature = "scanner-group-nfa-prototype")]

use ferroni::scanner::group_prototype::GroupKind;
use ferroni::{AccelerationMode, Scanner, ScannerConfig, ScannerFindOptions, ScannerPatternCache};

#[test]
fn group_archives_preserve_order_duplicates_captures_and_fallback() {
    let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
    let sets = [
        vec![r"\b[a-z]+\b", r"([0-9]+)", r"^end$", r"(?<=x)y", r"(a)\1"],
        vec![r"([0-9]+)", r"\b[a-z]+\b", r"([0-9]+)"],
        vec![r"\A\z"],
        vec![r"(?<=x)y"],
    ];
    let mut build = ScannerPatternCache::new();
    for set in &sets {
        Scanner::with_pattern_cache(set, &config, &mut build).unwrap();
    }
    let programs = build.export_precompiled().unwrap();
    for kind in kinds() {
        let groups = build.export_group_prototype(kind).unwrap();
        let mut loaded = ScannerPatternCache::new();
        loaded.add_precompiled(ferroni::PrecompiledPatterns::from_bytes(programs.clone()).unwrap());
        let stats = loaded.add_group_prototype(groups).unwrap();
        assert!(stats.nfa + stats.dfa + stats.portable > 0);
        if kind == GroupKind::Dfa {
            assert!(stats.dfa > 0);
        }
        for mode in [AccelerationMode::Auto, AccelerationMode::Off] {
            let mut cache = ScannerPatternCache::new();
            cache.add_precompiled(
                ferroni::PrecompiledPatterns::from_bytes(programs.clone()).unwrap(),
            );
            cache
                .add_group_prototype(build.export_group_prototype(kind).unwrap())
                .unwrap();
            Scanner::with_pattern_cache(&sets[0], &config.clone().acceleration(mode), &mut cache)
                .unwrap();
            assert_eq!(cache.group_prototype_load_stats(), (0, 0));
        }
        for set in sets
            .iter()
            .chain(std::iter::once(&vec!["missing", "[a-z]+extra"]))
        {
            let mut reference = Scanner::with_config(set, &config).unwrap();
            let mut actual = Scanner::with_pattern_cache(set, &config, &mut loaded).unwrap();
            for subject in ["", "a", "aa xy 12 end", "é 007 word", "end\n", "xx\ny"] {
                for (start, _) in subject
                    .char_indices()
                    .chain(std::iter::once((subject.len(), '\0')))
                {
                    for options in [ScannerFindOptions::NONE, ScannerFindOptions::from_bits(7)] {
                        assert_eq!(
                            actual.find_next_match(subject, start, options),
                            reference.find_next_match(subject, start, options),
                            "{kind:?} {set:?} {subject:?} {start}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn damaged_group_archive_rejects_or_falls_back() {
    let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
    let patterns = ["foo", "[0-9]+"];
    let mut build = ScannerPatternCache::new();
    Scanner::with_pattern_cache(&patterns, &config, &mut build).unwrap();
    let programs = build.export_precompiled().unwrap();
    for kind in kinds() {
        let groups = build.export_group_prototype(kind).unwrap();
        let mut damaged = groups.clone();
        *damaged.last_mut().unwrap() ^= 0xFF;
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled(ferroni::PrecompiledPatterns::from_bytes(programs.clone()).unwrap());
        cache.add_group_prototype(damaged).unwrap();
        let mut scanner = Scanner::with_pattern_cache(&patterns, &config, &mut cache).unwrap();
        assert_eq!(
            scanner
                .find_next_match("foo", 0, ScannerFindOptions::NONE)
                .unwrap()
                .index,
            0
        );
        assert_eq!(cache.group_prototype_load_stats(), (0, 1));
        for end in [0, 8, groups.len() / 2, groups.len() - 1] {
            let mut cache = ScannerPatternCache::new();
            cache.add_precompiled(
                ferroni::PrecompiledPatterns::from_bytes(programs.clone()).unwrap(),
            );
            assert!(cache.add_group_prototype(groups[..end].to_vec()).is_err());
            let mut scanner = Scanner::with_pattern_cache(&patterns, &config, &mut cache).unwrap();
            assert_eq!(
                scanner
                    .find_next_match("foo", 0, ScannerFindOptions::NONE)
                    .unwrap()
                    .index,
                0
            );
        }
    }
}

fn kinds() -> Vec<GroupKind> {
    #[allow(unused_mut)]
    let mut kinds = vec![GroupKind::Nfa];
    #[cfg(feature = "scanner-group-prototype")]
    kinds.push(GroupKind::Dfa);
    #[cfg(feature = "portable-eager-prototype")]
    kinds.push(GroupKind::Portable);
    kinds
}

#[cfg(feature = "portable-eager-prototype")]
#[test]
fn portable_images_can_be_shared_and_exported_again_after_execution() {
    let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
    let patterns = [r"\b[a-z]+\b", r"(\d+)", r"(?<=prefix:)suffix", r"^end$"];
    let mut producer = ScannerPatternCache::new();
    Scanner::with_pattern_cache(&patterns, &config, &mut producer).unwrap();
    let programs = producer.export_precompiled().unwrap();
    let image = producer
        .export_group_prototype(GroupKind::Portable)
        .unwrap();
    let mut consumer = ScannerPatternCache::new();
    consumer.add_precompiled(ferroni::PrecompiledPatterns::from_bytes(programs).unwrap());
    let stats = consumer.add_group_prototype(image.clone()).unwrap();
    assert_eq!(stats.portable, 1);
    let mut loaded = Scanner::with_pattern_cache(&patterns, &config, &mut consumer).unwrap();
    let mut ordinary = Scanner::with_config(&patterns, &config).unwrap();
    for subject in ["word 123", "prefix:suffix", "é end\n", "", "unseen input"] {
        for start in subject
            .char_indices()
            .map(|(at, _)| at)
            .chain([subject.len()])
        {
            assert_eq!(
                loaded.find_next_match(subject, start, ScannerFindOptions::NONE),
                ordinary.find_next_match(subject, start, ScannerFindOptions::NONE)
            );
        }
    }
    assert_eq!(consumer.group_prototype_load_stats(), (1, 0));
    assert_eq!(
        consumer
            .export_group_prototype(GroupKind::Portable)
            .unwrap(),
        image
    );
    // A different experimental representation must not accidentally serialize
    // the unused compatibility handles instead of the actual loaded image.
    assert!(consumer.export_group_prototype(GroupKind::Nfa).is_err());
}
