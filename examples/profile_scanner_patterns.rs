//! Diagnostic per-pattern searches on the captured subjects, outside RegSet.
//! These independent searches cannot be summed into actual scanner costs.
#[path = "../benches/cpp_scanner/mod.rs"]
mod scanner_replay;
use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::oniguruma::{ONIG_OPTION_CAPTURE_GROUP, OnigOptionType, OnigRegion};
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::regsyntax::OnigSyntaxOniguruma;
use scanner_replay::Corpus;
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        (2..=3).contains(&args.len()),
        "Usage: profile_scanner_patterns TRACE.json GROUP_ID [ROUNDS]"
    );
    let corpus = Corpus::from_json(&std::fs::read_to_string(&args[0]).unwrap());
    let group: usize = args[1].parse().expect("integer group ID");
    let rounds: usize = args
        .get(2)
        .map_or(3, |s| s.parse().expect("integer rounds"));
    assert!((1..=20).contains(&rounds));
    let calls = corpus.selected(Some(group));
    let subjects: Vec<_> = calls
        .iter()
        .map(|call| {
            let text = corpus.subjects[call.subject].as_str();
            let mut utf16 = 0;
            let start = text
                .char_indices()
                .find_map(|(byte, ch)| {
                    let at = utf16;
                    utf16 += ch.len_utf16();
                    (call.start_utf16 >= at && call.start_utf16 < utf16).then_some(byte)
                })
                .unwrap_or(text.len());
            (text.as_bytes(), start, call.option_bits << 22)
        })
        .collect();
    let mut results = Vec::new();
    for (index, pattern) in corpus.patterns[group].iter().enumerate() {
        let reg = onig_new(
            pattern.as_bytes(),
            ONIG_OPTION_CAPTURE_GROUP,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
        )
        .expect("pattern compiles");
        let mut region = OnigRegion::new();
        // Validate individual searches, including their full byte captures.
        #[cfg(feature = "ffi")]
        {
            use ferroni::ffi::{CRegex, CRegion};
            let c = CRegex::new(pattern.as_bytes(), ONIG_OPTION_CAPTURE_GROUP.bits()).unwrap();
            let mut c_region = CRegion::new();
            for &(text, start, options) in &subjects {
                let (actual, returned) = onig_search(
                    &reg,
                    text,
                    text.len(),
                    start,
                    text.len(),
                    Some(region),
                    OnigOptionType::from_bits_retain(options),
                );
                region = returned.unwrap();
                c_region.clear();
                let expected = c.search(text, start, text.len(), Some(&mut c_region), options);
                assert_eq!(actual, expected, "pattern {index} match differs");
                if actual >= 0 {
                    assert_eq!(
                        region.beg[..region.num_regs as usize]
                            .iter()
                            .copied()
                            .zip(region.end.iter().copied())
                            .collect::<Vec<_>>(),
                        c_region.capture_ranges(),
                        "pattern {index} captures differ"
                    );
                }
            }
        }
        let mut samples_ns = Vec::new();
        let mut matches = 0;
        for round in 0..=rounds {
            let started = Instant::now();
            let mut count = 0;
            for &(text, start, options) in &subjects {
                let (result, returned) = onig_search(
                    &reg,
                    black_box(text),
                    text.len(),
                    black_box(start),
                    text.len(),
                    Some(region),
                    OnigOptionType::from_bits_retain(options),
                );
                region = returned.unwrap();
                assert!(result >= -1, "pattern {index} search error {result}");
                count += usize::from(result >= 0);
                black_box(&region);
            }
            if round > 0 {
                samples_ns.push(started.elapsed().as_nanos());
            }
            matches = count;
        }
        results.push(serde_json::json!({"index":index,"pattern":pattern,"calls":subjects.len(),"matches":matches,"samples_ns":samples_ns}));
    }
    println!(
        "{}",
        serde_json::json!({"group":group,"boundary":"independent full-range per-pattern search, no scanner memo","c_reference_validated":cfg!(feature="ffi"),"patterns":results})
    );
}
