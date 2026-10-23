//! Shared setup for the bounded scanner-group archive experiment.
#![allow(dead_code)]
#[path = "../cpp_scanner/mod.rs"]
mod scanner_replay;
use ferroni::{AccelerationMode, PrecompiledPatterns, Scanner, ScannerConfig, ScannerPatternCache};
pub use scanner_replay::{Corpus, normalized, replay};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

pub fn corpus(name: &str) -> Corpus {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("benches/{name}_scanner/trace.json"));
    Corpus::from_json(&std::fs::read_to_string(path).unwrap())
}

pub struct Bundle {
    pub programs: Arc<[u8]>,
    pub groups: Option<Arc<[u8]>>,
}
impl Bundle {
    pub fn read(dir: &Path, name: &str, variant: &str) -> Self {
        Self {
            programs: std::fs::read(dir.join(format!("{name}.programs")))
                .unwrap()
                .into(),
            groups: match variant {
                "eager" | "control" => None,
                "nfa" | "dfa" | "dense" | "portable" => Some(
                    std::fs::read(dir.join(format!("{name}.{variant}")))
                        .unwrap()
                        .into(),
                ),
                _ => panic!("unknown archive variant: {variant}"),
            },
        }
    }
    pub fn scanners(&self, corpus: &Corpus) -> (ScannerPatternCache, Vec<Scanner>) {
        let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled(PrecompiledPatterns::from_bytes(self.programs.clone()).unwrap());
        #[cfg(feature = "scanner-group-nfa-prototype")]
        let expected = self
            .groups
            .as_ref()
            .map(|groups| cache.add_group_prototype(groups.clone()).unwrap());
        #[cfg(not(feature = "scanner-group-nfa-prototype"))]
        assert!(self.groups.is_none());
        let scanners = corpus
            .patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
                Scanner::with_pattern_cache(&refs, &config, &mut cache).unwrap()
            })
            .collect();
        #[cfg(feature = "scanner-group-nfa-prototype")]
        if let Some(expected) = expected {
            assert_eq!(
                cache.group_prototype_load_stats(),
                (
                    expected.nfa + expected.dfa + expected.disabled + expected.portable,
                    0
                ),
                "every archived group must be consumed without a decode fallback"
            );
        }
        (cache, scanners)
    }
    pub fn validate(&self, corpus: &Corpus) {
        let (_cache, mut scanners) = self.scanners(corpus);
        let strings = corpus.strings();
        for (i, call) in corpus.calls.iter().enumerate() {
            assert_eq!(
                normalized(scanners[call.scanner].find_next_match_utf16(
                    &strings[call.subject],
                    call.start_utf16,
                    call.options
                )),
                call.expected,
                "capture trace differs at call {i}"
            );
        }
    }
}

pub fn stats(corpus: &Corpus, scanners: &[Scanner]) -> Value {
    let mut seen = HashSet::new();
    let mut immutable = 0;
    let mut scratch = 0;
    let mut built = 0;
    let mut nfa_states = 0;
    let mut covered = 0;
    let mut own = 0;
    let mut retired = 0;
    let mut clears = 0;
    for (patterns, scanner) in corpus.patterns.iter().zip(scanners) {
        let s = scanner.prefilter_stats();
        scratch += s.cache_memory_usage;
        if !seen.insert(patterns) {
            continue;
        }
        immutable += s.memory_usage - s.cache_memory_usage;
        built += usize::from(s.built);
        nfa_states += s.nfa_states;
        covered += s.covered;
        own += s.own;
        retired += usize::from(s.retired);
        clears += s.cache_clears;
    }
    json!({"unique_groups":seen.len(),"built_groups":built,"immutable_automata_bytes":immutable,
        "scanner_cache_bytes":scratch,"forward_nfa_states":nfa_states,"covered":covered,"own":own,
        "retired_groups":retired,"cache_clears":clears})
}
