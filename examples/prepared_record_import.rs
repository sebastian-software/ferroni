//! Diagnostic A/B of selective import versus per-list subset archive assembly.
//! Metadata and trace parsing are outside timing; payload file I/O is inside.
#[allow(dead_code)]
#[path = "../tests/support/eager_archive_mutations.rs"]
mod checksums;
#[path = "../benches/group_archive/mod.rs"]
mod support;
use ferroni::scanner::{
    PrecompiledBytes, PrecompiledEager, PrecompiledEagerRecord, PrecompiledEagerRecordKind,
    PrecompiledEagerRecords,
};
use ferroni::{AccelerationMode, Scanner, ScannerConfig, ScannerPatternCache};
use serde_json::json;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::Instant,
};

fn word(bytes: &[u8], at: &mut usize) -> usize {
    let n = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap()) as usize;
    *at += 4;
    n
}
fn put(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&u32::try_from(n).unwrap().to_le_bytes());
}
struct Key {
    bytes: Vec<u8>,
    checksum: u32,
}
fn keys(index: &[u8]) -> (Vec<Key>, Vec<u32>) {
    let mut at = 21;
    let count = word(index, &mut at);
    let programs = (0..count)
        .map(|_| {
            let len = word(index, &mut at);
            let bytes = index[at..at + len].to_vec();
            at += len + 8; // Original offset and record length.
            let checksum = word(index, &mut at) as u32;
            Key { bytes, checksum }
        })
        .collect();
    let groups = word(index, &mut at);
    let checksums = (0..groups)
        .map(|_| {
            let ids = word(index, &mut at);
            at += 4 * ids + 8;
            word(index, &mut at) as u32
        })
        .collect();
    (programs, checksums)
}
struct Input<'a> {
    source: Option<PrecompiledEagerRecords>,
    programs: File,
    groups: File,
    keys: &'a [Key],
    group_checksums: &'a [u32],
    raw: Vec<Option<PrecompiledBytes>>,
    pairs: HashMap<Vec<usize>, PrecompiledEager>,
    read_bytes: usize,
    copied_bytes: usize,
}
impl Input<'_> {
    fn source(&self) -> &PrecompiledEagerRecords {
        self.source.as_ref().unwrap()
    }
    fn read(&mut self, record: &PrecompiledEagerRecord) -> PrecompiledBytes {
        let mut bytes = Vec::new();
        self.read_into(record, &mut bytes);
        bytes.into()
    }
    fn read_into(&mut self, record: &PrecompiledEagerRecord, bytes: &mut Vec<u8>) {
        let file = match record.kind() {
            PrecompiledEagerRecordKind::Program => &mut self.programs,
            PrecompiledEagerRecordKind::Group => &mut self.groups,
            _ => unreachable!(),
        };
        let range = record.range();
        file.seek(SeekFrom::Start(range.start as u64)).unwrap();
        let start = bytes.len();
        bytes.resize(start + range.len(), 0);
        file.read_exact(&mut bytes[start..]).unwrap();
        self.read_bytes += range.len();
    }
    fn supply(&mut self, patterns: &[&str], config: &ScannerConfig) {
        for record in patterns
            .iter()
            .map(|p| self.source().program(p, config, false).unwrap())
            .chain(self.source().group(patterns, config, false))
            .collect::<Vec<_>>()
        {
            if !self.source().contains(&record) {
                let bytes = self.read(&record);
                self.source().insert(&record, bytes).unwrap();
            }
        }
    }
    // Reproduce the existing adapter's work: retain raw program records once,
    // then copy each requested dictionary and remap ordered IDs into a new pair.
    fn pair(&mut self, patterns: &[&str], config: &ScannerConfig) -> PrecompiledEager {
        let records: Vec<_> = patterns
            .iter()
            .map(|p| self.source().program(p, config, false).unwrap())
            .collect();
        let ordered: Vec<_> = records.iter().map(|r| r.id()).collect();
        if let Some(pair) = self.pairs.get(&ordered) {
            return pair.clone();
        }
        let mut selected = records;
        selected.sort_by_key(|r| r.id());
        selected.dedup_by_key(|r| r.id());
        let mut index = Vec::new();
        put(&mut index, selected.len());
        let mut offset = 0;
        for record in &selected {
            let key = &self.keys[record.id()];
            put(&mut index, key.bytes.len());
            index.extend_from_slice(&key.bytes);
            put(&mut index, offset);
            put(&mut index, record.range().len());
            index.extend_from_slice(&key.checksum.to_le_bytes());
            offset += record.range().len();
        }
        let mut programs = b"FERRONI\0".to_vec();
        put(&mut programs, 2);
        put(&mut programs, index.len());
        programs.extend(index);
        programs.reserve(offset);
        for record in &selected {
            if self.raw[record.id()].is_none() {
                let bytes = self.read(record);
                self.raw[record.id()] = Some(bytes);
            }
            programs.extend_from_slice(self.raw[record.id()].as_ref().unwrap());
        }
        self.copied_bytes += offset;
        let mut groups = b"FPRT003\0".to_vec();
        groups.extend_from_slice(&checksums::checksum(&programs).to_le_bytes());
        groups.push(1);
        if let Some(record) = self.source().group(patterns, config, false) {
            put(&mut groups, 1);
            put(&mut groups, ordered.len());
            for id in &ordered {
                put(
                    &mut groups,
                    selected.binary_search_by_key(id, |r| r.id()).unwrap(),
                );
            }
            groups.extend_from_slice(&self.group_checksums[record.id()].to_le_bytes());
            put(&mut groups, record.range().len());
            self.read_into(&record, &mut groups);
        } else {
            put(&mut groups, 0);
        }
        let pair = PrecompiledEager::from_shared_bytes(programs.into(), groups.into()).unwrap();
        self.pairs.insert(ordered, pair.clone());
        pair
    }
}
fn validate(corpus: &support::Corpus, scanners: &mut [Scanner]) {
    let strings = corpus.strings();
    for (i, call) in corpus.calls.iter().enumerate() {
        assert_eq!(
            support::normalized(scanners[call.scanner].find_next_match_utf16(
                &strings[call.subject],
                call.start_utf16,
                call.options
            )),
            call.expected,
            "call {i}"
        );
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let command = &args[1];
    let name = &args[2];
    let dir = Path::new(&args[3]);
    let mut corpus = support::corpus(name);
    // A prefix lane keeps the same source archive but requests only scanners
    // reached by the first captured calls. Preparation always exports all groups.
    let call_limit = args.get(6).map_or(0, |n| n.parse::<usize>().unwrap());
    if command == "measure" && call_limit != 0 {
        corpus.calls.truncate(call_limit);
        let mut used: Vec<_> = corpus.calls.iter().map(|call| call.scanner).collect();
        used.sort_unstable();
        used.dedup();
        corpus.patterns = used.iter().map(|&id| corpus.patterns[id].clone()).collect();
        for call in &mut corpus.calls {
            call.scanner = used.binary_search(&call.scanner).unwrap();
        }
        corpus.hot_groups.clear();
    }
    let refs: Vec<Vec<_>> = corpus
        .patterns
        .iter()
        .map(|p| p.iter().map(String::as_str).collect())
        .collect();
    let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
    if command == "prepare" {
        let mut cache = ScannerPatternCache::new();
        for patterns in &refs {
            Scanner::with_pattern_cache(patterns, &config, &mut cache).unwrap();
        }
        let pair = cache.export_precompiled_eager().unwrap();
        let index = pair.export_record_index().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        for (suffix, bytes) in [
            ("programs", pair.program_bytes()),
            ("portable", pair.group_bytes()),
            ("records", &index),
        ] {
            std::fs::write(dir.join(format!("{name}.{suffix}")), bytes).unwrap();
        }
        println!(
            "{}",
            json!({"corpus":name,"index_bytes":index.len(),"program_bytes":pair.program_bytes().len(),"group_bytes":pair.group_bytes().len()})
        );
        return;
    }
    assert_eq!(command, "measure");
    let variant = &args[4];
    let count = args.get(5).map_or(12, |n| n.parse::<usize>().unwrap());
    let index = std::fs::read(dir.join(format!("{name}.records"))).unwrap();
    let (keys, group_checksums) = keys(&index); // Host metadata already available in both lanes.
    let lookup = PrecompiledEagerRecords::from_index(&index).unwrap();
    let expected_groups = refs
        .iter()
        .filter_map(|patterns| lookup.group(patterns, &config, false))
        .map(|record| record.id())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let mut rows = Vec::new();
    for _ in 0..count {
        let started = Instant::now();
        let source = match variant.as_str() {
            "records" | "subset" => Some(PrecompiledEagerRecords::from_index(&index).unwrap()),
            "archive" => None,
            _ => panic!("unknown variant"),
        };
        let mut input = Input {
            source: source.clone(),
            programs: File::open(dir.join(format!("{name}.programs"))).unwrap(),
            groups: File::open(dir.join(format!("{name}.portable"))).unwrap(),
            keys: &keys,
            group_checksums: &group_checksums,
            raw: vec![None; keys.len()],
            pairs: HashMap::new(),
            read_bytes: 0,
            copied_bytes: 0,
        };
        let mut cache = ScannerPatternCache::new();
        match variant.as_str() {
            "records" => cache.add_precompiled_eager_records(source.clone().unwrap()),
            "archive" => {
                let mut programs = Vec::new();
                let mut groups = Vec::new();
                input.programs.read_to_end(&mut programs).unwrap();
                input.groups.read_to_end(&mut groups).unwrap();
                input.read_bytes += programs.len() + groups.len();
                cache.add_precompiled_eager(
                    PrecompiledEager::from_shared_bytes(programs.into(), groups.into()).unwrap(),
                );
            }
            "subset" => (),
            _ => panic!("unknown variant"),
        }
        let mut scanners: Vec<_> = refs
            .iter()
            .map(|patterns| {
                match variant.as_str() {
                    "records" => input.supply(patterns, &config),
                    "subset" => cache.add_precompiled_eager(input.pair(patterns, &config)),
                    _ => (),
                }
                Scanner::with_pattern_cache(patterns, &config, &mut cache).unwrap()
            })
            .collect();
        let load_ns = started.elapsed().as_nanos();
        // Include correctness in every fresh load, outside the timed boundary.
        validate(&corpus, &mut scanners);
        let stats = source.as_ref().map(|s| s.stats()).unwrap_or_default();
        assert_eq!((stats.invalid_programs, stats.invalid_groups), (0, 0));
        let (loaded_groups, invalid_groups) = if variant == "subset" {
            input
                .pairs
                .values()
                .fold((0, 0), |(loaded, invalid), pair| {
                    let stats = pair.stats();
                    (loaded + stats.loaded_groups, invalid + stats.invalid_groups)
                })
        } else {
            cache.group_prototype_load_stats()
        };
        assert_eq!((loaded_groups, invalid_groups), (expected_groups, 0));
        if variant == "records" {
            assert_eq!(stats.loaded_groups, expected_groups);
        }
        let calls = corpus.selected(None);
        let strings = corpus.strings();
        for _ in 0..3 {
            support::replay(&mut scanners, &strings, &calls);
        }
        let mut search = Vec::new();
        for _ in 0..8 {
            let strings = corpus.strings();
            let started = Instant::now();
            support::replay(&mut scanners, &strings, &calls);
            search.push(started.elapsed().as_nanos());
        }
        rows.push(json!({"load_ns":load_ns,"search_ns":search,"read_bytes":input.read_bytes,"copied_program_bytes":input.copied_bytes,"loaded_programs":stats.loaded_programs,"loaded_groups":loaded_groups,"invalid_groups":invalid_groups}));
        std::hint::black_box((&input, &cache, &scanners));
    }
    println!(
        "{}",
        json!({"corpus":name,"variant":variant,"call_limit":call_limit,"calls":corpus.calls.len(),"scanners":refs.len(),"samples":rows})
    );
}
