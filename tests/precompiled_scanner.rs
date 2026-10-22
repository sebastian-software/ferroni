//! Archive loading against captured UTF-16 scanner calls from Ferriki.
use ferroni::{
    OnigString, PrecompiledPatterns, Scanner, ScannerConfig, ScannerFindOptions,
    ScannerPatternCache,
};

#[test]
fn captured_scanner_sets_preserve_tokens_and_captures() {
    for (name, json) in [
        ("cpp", include_str!("../benches/cpp_scanner/trace.json")),
        ("scss", include_str!("../benches/scss_scanner/trace.json")),
        ("java", include_str!("../benches/java_scanner/trace.json")),
    ] {
        let fixture: serde_json::Value = serde_json::from_str(json).unwrap();
        let patterns: Vec<Vec<String>> =
            serde_json::from_value(fixture["scanners"].clone()).unwrap();
        let subjects: Vec<String> = serde_json::from_value(fixture["subjects"].clone()).unwrap();
        let subjects: Vec<_> = subjects.iter().map(|s| OnigString::new(s)).collect();
        let config = ScannerConfig::default().prefilter_warmup(0);
        let mut build = ScannerPatternCache::new();
        for patterns in &patterns {
            let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
            Scanner::with_pattern_cache(&refs, &config, &mut build).unwrap();
        }
        let blob = build
            .export_precompiled()
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let source = PrecompiledPatterns::from_bytes(blob).unwrap();
        for patterns in &patterns {
            for p in patterns {
                assert_eq!(
                    source.validate_pattern(p, &config, false),
                    Ok(true),
                    "{name}: {p}"
                );
            }
        }
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled(source);
        let mut scanners: Vec<_> = patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
                Scanner::with_pattern_cache(&refs, &config, &mut cache).unwrap()
            })
            .collect();
        let calls = fixture["calls"].as_array().unwrap();
        for (index, row) in calls.iter().enumerate() {
            let number = |at: usize| row[at].as_u64().unwrap() as usize;
            let found = scanners[number(0)].find_next_match_utf16(
                &subjects[number(1)],
                number(2),
                ScannerFindOptions::from_bits(number(3) as u32),
            );
            let found = found.map(|m| {
                (
                    m.index,
                    m.captures()
                        .iter()
                        .map(|c| (c.start, c.end))
                        .collect::<Vec<_>>(),
                )
            });
            let expected = row[4].as_u64().map(|pattern| {
                (
                    pattern as usize,
                    row[5]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| {
                            (
                                c[0].as_u64().unwrap() as usize,
                                c[1].as_u64().unwrap() as usize,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            });
            assert_eq!(found, expected, "{name} call {index}");
        }
        eprintln!(
            "{name}: {} distinct programs, {} captured calls",
            cache.len(),
            calls.len()
        );
    }
}

/// Issue #448: the optimizer merges a look-behind's one-byte class with the
/// same class right after it into `CClassRun`, a body the VM accepts. Such
/// programs must export, load and match like compiled ones.
#[test]
fn look_behind_class_runs_round_trip() {
    let patterns = [
        r"(?<=[?!])[?!]+",
        r"(?<=[ab])[ab]+",
        r"(?<=[ab])[ab]",
        r"(?<=[ab])[ab]+x",
        r"x(?<=[ab])[ab]+",
        r"(?<![ab])[ab]+",
        r"(?<=[ab][ab])[ab]",
    ];
    let subjects = [
        "",
        "a",
        "ab",
        "aab",
        "xab",
        "xabbx",
        "b?!!",
        "!?",
        "?!?!",
        "ba x abbbx",
    ];
    let config = ScannerConfig::default();
    for pattern in patterns {
        let mut build = ScannerPatternCache::new();
        let mut compiled = Scanner::with_pattern_cache(&[pattern], &config, &mut build).unwrap();
        let blob = build
            .export_precompiled()
            .unwrap_or_else(|e| panic!("{pattern}: {e}"));
        let source = PrecompiledPatterns::from_bytes(blob).unwrap();
        assert_eq!(
            source.validate_pattern(pattern, &config, false),
            Ok(true),
            "{pattern}"
        );
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled(source);
        let mut loaded = Scanner::with_pattern_cache(&[pattern], &config, &mut cache).unwrap();
        for subject in subjects {
            for start in 0..=subject.len() {
                let found = |scanner: &mut Scanner| {
                    scanner
                        .find_next_match(subject, start, ScannerFindOptions::NONE)
                        .map(|m| {
                            (
                                m.index,
                                m.captures()
                                    .iter()
                                    .map(|c| (c.start, c.end))
                                    .collect::<Vec<_>>(),
                            )
                        })
                };
                assert_eq!(
                    found(&mut loaded),
                    found(&mut compiled),
                    "{pattern} on {subject:?} from {start}"
                );
            }
        }
    }
}
