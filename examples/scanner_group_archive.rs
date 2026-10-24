//! Prepare, validate, and inspect scanner-group prototype artifacts.
#[path = "../benches/group_archive/mod.rs"]
mod support;
use serde_json::json;
use std::{path::Path, time::Instant};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let command = args[1].as_str();
    let name = &args[2];
    let dir = Path::new(&args[3]);
    let corpus = support::corpus(name);
    #[cfg(feature = "precompiled-eager")]
    if command == "prepare-portable" {
        use ferroni::scanner::group_prototype::GroupKind;
        use ferroni::{AccelerationMode, Scanner, ScannerConfig, ScannerPatternCache};
        std::fs::create_dir_all(dir).unwrap();
        let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
        let mut cache = ScannerPatternCache::new();
        for patterns in &corpus.patterns {
            let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
            Scanner::with_pattern_cache(&refs, &config, &mut cache).unwrap();
        }
        let start = Instant::now();
        let programs = cache.export_precompiled().unwrap();
        let groups = cache.export_group_prototype(GroupKind::Portable).unwrap();
        std::fs::write(dir.join(format!("{name}.programs")), &programs).unwrap();
        std::fs::write(dir.join(format!("{name}.portable")), &groups).unwrap();
        println!(
            "{}",
            json!({"corpus":name,"variant":"portable","export_seconds":start.elapsed().as_secs_f64(),"program_bytes":programs.len(),"group_bytes":groups.len()})
        );
        return;
    }
    if command == "prepare" {
        #[cfg(feature = "scanner-group-prototype")]
        {
            prepare(name, dir, &corpus);
            return;
        }
        #[cfg(not(feature = "scanner-group-prototype"))]
        panic!("preparation requires scanner-group-prototype");
    }
    let variant = &args[4];
    let bundle = support::Bundle::read(dir, name, variant);
    #[cfg(feature = "scanner-load-profile")]
    if command == "load-phases" {
        bundle.validate(&corpus);
        let count = args.get(5).map_or(20, |n| n.parse::<usize>().unwrap());
        let mut rows = Vec::new();
        for _ in 0..count {
            let (scanners, samples) =
                ferroni::scanner::load_profile::measure(|| bundle.scanners(&corpus));
            rows.push(
                samples
                    .iter()
                    .map(|s| {
                        json!({"phase":s.phase,"calls":s.calls,
                "inclusive_ns":s.inclusive_ns,"exclusive_ns":s.exclusive_ns})
                    })
                    .collect::<Vec<_>>(),
            );
            std::hint::black_box(&scanners);
        }
        println!(
            "{}",
            json!({"corpus":name,"variant":variant,"samples":rows})
        );
        return;
    }
    if command == "load-probe" {
        bundle.validate(&corpus);
        for _ in 0..3 {
            std::hint::black_box(bundle.scanners(&corpus));
        }
        let count = args.get(5).map_or(40, |n| n.parse::<usize>().unwrap());
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            let started = Instant::now();
            let scanners = bundle.scanners(&corpus);
            samples.push(started.elapsed().as_nanos());
            std::hint::black_box(&scanners);
        }
        println!(
            "{}",
            json!({"corpus":name,"variant":variant,"samples_ns":samples})
        );
        return;
    }
    if command == "validate" {
        bundle.validate(&corpus);
        println!(
            "{}",
            json!({"corpus":name,"variant":variant,"validated_calls":corpus.calls.len()})
        );
    } else if command == "search-probe" {
        // Short ablation screen. The final comparison uses Criterion instead.
        bundle.validate(&corpus);
        let (_cache, mut scanners) = bundle.scanners(&corpus);
        let calls = corpus.selected(None);
        for _ in 0..3 {
            support::replay(&mut scanners, &corpus.strings(), &calls);
        }
        let count = args.get(5).map_or(40, |n| n.parse::<usize>().unwrap());
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            let strings = corpus.strings();
            let start = Instant::now();
            support::replay(&mut scanners, &strings, &calls);
            samples.push(start.elapsed().as_nanos());
        }
        println!(
            "{}",
            json!({"corpus":name,"variant":variant,"samples_ns":samples,
            "after_search":support::stats(&corpus, &scanners)})
        );
    } else if command == "memory" {
        let before = Instant::now();
        let (_cache, mut scanners) = bundle.scanners(&corpus);
        let load_ns = before.elapsed().as_nanos();
        let loaded = support::stats(&corpus, &scanners);
        let strings = corpus.strings();
        let calls = corpus.selected(None);
        let start = Instant::now();
        support::replay(&mut scanners, &strings, &calls);
        let first_search_ns = start.elapsed().as_nanos();
        for _ in 0..3 {
            let fresh = corpus.strings();
            support::replay(&mut scanners, &fresh, &calls);
        }
        let warm = support::stats(&corpus, &scanners);
        #[cfg(feature = "scanner-group-nfa-prototype")]
        let prepared_loads = json!({"groups":_cache.group_prototype_load_stats()});
        #[cfg(not(feature = "scanner-group-nfa-prototype"))]
        let prepared_loads = serde_json::Value::Null;
        println!(
            "{}",
            json!({"corpus":name,"variant":variant,"prepared_loads":prepared_loads,"load_ns":load_ns,
            "first_search_ns":first_search_ns,"after_load":loaded,"after_search":warm,
            "program_bytes":bundle.programs.len(),"group_bytes":bundle.groups.as_ref().map_or(0, |g| g.len())})
        );
        std::hint::black_box((&bundle, &scanners, &_cache));
    } else {
        panic!("unknown command");
    }
}

#[cfg(feature = "scanner-group-prototype")]
fn prepare(name: &str, dir: &Path, corpus: &support::Corpus) {
    use ferroni::scanner::group_prototype::GroupKind;
    use ferroni::{AccelerationMode, Scanner, ScannerConfig, ScannerPatternCache};
    std::fs::create_dir_all(dir).unwrap();
    let mut cache = ScannerPatternCache::new();
    let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
    for patterns in &corpus.patterns {
        let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
        Scanner::with_pattern_cache(&refs, &config, &mut cache).unwrap();
    }
    let programs = cache.export_precompiled().unwrap();
    std::fs::write(dir.join(format!("{name}.programs")), &programs).unwrap();
    for (variant, kind) in [
        ("nfa", GroupKind::Nfa),
        (
            if cfg!(feature = "scanner-group-dense-prototype") {
                "dense"
            } else {
                "dfa"
            },
            GroupKind::Dfa,
        ),
    ] {
        let start = Instant::now();
        let bytes = cache.export_group_prototype(kind).unwrap();
        let elapsed = start.elapsed().as_secs_f64();
        let mut loaded = ScannerPatternCache::new();
        loaded.add_precompiled(ferroni::PrecompiledPatterns::from_bytes(programs.clone()).unwrap());
        let stats = loaded.add_group_prototype(bytes.clone()).unwrap();
        std::fs::write(dir.join(format!("{name}.{variant}")), &bytes).unwrap();
        println!(
            "{}",
            json!({"corpus":name,"variant":variant,"export_seconds":elapsed,
            "group_bytes":bytes.len(),"program_bytes":programs.len(),
            "nfa_groups":stats.nfa,"dfa_groups":stats.dfa,"disabled_groups":stats.disabled})
        );
    }
}
