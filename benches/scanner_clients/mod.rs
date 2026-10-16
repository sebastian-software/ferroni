//! Recorded public Scanner calls; no tokenizer, rendering or instrumentation.
use ferroni::scanner::{OnigString, Scanner, ScannerFindOptions, ScannerPatternCache};
use serde_json::{Value, json};
use std::hint::black_box;

pub struct ScannerSpec {
    pub patterns: Vec<String>,
    pub cache: Option<usize>,
}

pub enum Event {
    Create(usize),
    Find(usize, usize, usize, u32, super::scanner_replay::Match),
    Drop(usize),
    Clear(usize),
    DropCache(usize),
}

pub struct Trace {
    pub scanners: Vec<ScannerSpec>,
    pub subjects: Vec<String>,
    pub events: Vec<Event>,
    pub caches: usize,
    pub legacy: super::scanner_replay::Corpus,
}

fn index(value: &Value) -> usize {
    usize::try_from(value.as_u64().expect("nonnegative integer")).expect("usize index")
}

impl Trace {
    pub fn parse(input: &str) -> Self {
        let root: Value = serde_json::from_str(input).expect("valid client trace");
        assert_eq!(root["format_version"], 1);
        let caches = index(&root["caches"]);
        let subjects: Vec<String> = serde_json::from_value(root["subjects"].clone()).unwrap();
        let scanners: Vec<ScannerSpec> = root["scanners"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let cache = (!s["cache"].is_null()).then(|| index(&s["cache"]));
                assert!(cache.is_none_or(|id| id < caches));
                ScannerSpec {
                    patterns: serde_json::from_value(s["patterns"].clone()).unwrap(),
                    cache,
                }
            })
            .collect();
        let mut live = vec![false; scanners.len()];
        let mut created = vec![false; scanners.len()];
        let mut cache_state = vec![0u8; caches]; // 0 unseen, 1 live, 2 destroyed.
        let mut calls = Vec::new();
        let events = root["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let id = index(&row[1]);
                match row[0].as_str().unwrap() {
                    "create" => {
                        assert!(!created[id], "duplicate creation");
                        if let Some(cache) = scanners[id].cache {
                            assert_ne!(cache_state[cache], 2, "reuse of destroyed cache");
                            cache_state[cache] = 1;
                        }
                        created[id] = true;
                        live[id] = true;
                        Event::Create(id)
                    }
                    "drop" => {
                        assert!(live[id], "drop of dead scanner");
                        live[id] = false;
                        Event::Drop(id)
                    }
                    "find" => {
                        assert!(live[id], "search of dead scanner");
                        let subject = index(&row[2]);
                        assert!(subject < subjects.len());
                        let start = index(&row[3]);
                        assert!(start <= subjects[subject].encode_utf16().count());
                        let options = u32::try_from(index(&row[4])).unwrap();
                        assert_eq!(options & !7, 0);
                        let expected =
                            if row[5].is_null() {
                                assert!(row[6].as_array().unwrap().is_empty());
                                None
                            } else {
                                let pattern = index(&row[5]);
                                assert!(pattern < scanners[id].patterns.len());
                                let captures: Vec<(usize, usize)> =
                                    serde_json::from_value(row[6].clone()).unwrap();
                                assert!(!captures.is_empty());
                                assert!(captures.iter().all(|&(a, b)| a <= b
                                    && b <= subjects[subject].encode_utf16().count()));
                                Some((pattern, captures))
                            };
                        calls.push(json!([id, subject, start, options, row[5], row[6]]));
                        Event::Find(id, subject, start, options, expected)
                    }
                    "clear" => {
                        assert!(id < caches);
                        assert_eq!(cache_state[id], 1, "clear of dead cache");
                        Event::Clear(id)
                    }
                    "drop_cache" => {
                        assert!(id < caches);
                        assert_eq!(cache_state[id], 1, "drop of dead cache");
                        cache_state[id] = 2;
                        Event::DropCache(id)
                    }
                    _ => panic!("unknown event"),
                }
            })
            .collect();
        assert!(created.iter().all(|&v| v));
        assert!(
            live.iter().all(|&v| !v),
            "capture omitted final scanner drops"
        );
        assert!(!calls.is_empty());
        assert!(
            cache_state.iter().all(|&state| state == 2),
            "capture omitted cache drops"
        );
        let legacy = super::scanner_replay::Corpus::from_json(
            &json!({
                "format_version":1,
                "scanners":scanners.iter().map(|s| &s.patterns).collect::<Vec<_>>(),
                "subjects":subjects, "calls":calls, "hot_groups":[]
            })
            .to_string(),
        );
        Self {
            scanners,
            subjects,
            events,
            caches,
            legacy,
        }
    }

    pub fn strings(&self) -> Vec<OnigString> {
        self.subjects.iter().map(|s| OnigString::new(s)).collect()
    }

    fn create(
        &self,
        id: usize,
        caches: &mut [Option<ScannerPatternCache>],
        shared: bool,
    ) -> Scanner {
        let config = super::scanner_replay::bench_config();
        let spec = &self.scanners[id];
        let patterns: Vec<&str> = spec.patterns.iter().map(String::as_str).collect();
        if let Some(cache) = spec.cache.filter(|_| shared) {
            Scanner::with_pattern_cache(
                &patterns,
                &config,
                caches[cache].get_or_insert_with(ScannerPatternCache::new),
            )
            .unwrap()
        } else {
            Scanner::with_config(&patterns, &config).unwrap()
        }
    }

    /// All captured create/clear/drop events, and optionally every search.
    /// Scheduler, result allocation and recorded destruction are inside timing.
    pub fn lifecycle(&self, strings: &[OnigString], shared: bool, searches: bool, validate: bool) {
        let mut scanners: Vec<Option<Scanner>> = (0..self.scanners.len()).map(|_| None).collect();
        let mut caches: Vec<Option<ScannerPatternCache>> = (0..self.caches).map(|_| None).collect();
        for event in &self.events {
            match event {
                Event::Create(id) => scanners[*id] = Some(self.create(*id, &mut caches, shared)),
                Event::Drop(id) => drop(black_box(scanners[*id].take().expect("live scanner"))),
                Event::Clear(id) => {
                    if let Some(cache) = &mut caches[*id] {
                        cache.clear();
                    }
                }
                Event::DropCache(id) => drop(black_box(caches[*id].take())),
                Event::Find(id, subject, start, options, expected) if searches => {
                    let found = scanners[*id].as_mut().unwrap().find_next_match_utf16(
                        &strings[*subject],
                        *start,
                        ScannerFindOptions::from_bits(*options),
                    );
                    if validate {
                        assert_eq!(super::scanner_replay::normalized(found), *expected);
                    } else {
                        drop(black_box(found));
                    }
                }
                Event::Find(..) => {}
            }
        }
        assert!(scanners.iter().all(Option::is_none));
    }

    /// Persistent scanners for the fixed-call, warmed search lane only.
    pub fn prepared(&self, shared: bool) -> (Vec<Scanner>, Vec<Option<ScannerPatternCache>>) {
        let mut caches = (0..self.caches).map(|_| None).collect::<Vec<_>>();
        let scanners = (0..self.scanners.len())
            .map(|id| self.create(id, &mut caches, shared))
            .collect();
        (scanners, caches)
    }

    pub fn validate(&self, shared: bool) {
        self.legacy.validate(); // With `ffi`, independently checks every match/capture against C.
        self.lifecycle(&self.strings(), shared, true, true);
    }

    pub fn stats(&self) -> Value {
        let mut uses = vec![0usize; self.scanners.len()];
        for event in &self.events {
            if let Event::Find(id, ..) = event {
                uses[*id] += 1;
            }
        }
        json!({"scanners":self.scanners.len(), "subjects":self.subjects.len(),
            "calls":self.legacy.calls.len(), "events":self.events.len(), "pattern_caches":self.caches,
            "short_lived_scanners":uses.iter().filter(|&&n| n <= 2).count(),
            "cached_scanners":self.scanners.iter().filter(|s| s.cache.is_some()).count(),
            "pattern_references":self.scanners.iter().map(|s|s.patterns.len()).sum::<usize>()})
    }
}
