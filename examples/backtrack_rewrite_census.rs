//! Evaluate opt-in rewrite experiments against TextMate grammar JSON files.
//! `cargo run --release --example backtrack_rewrite_census -- GRAMMAR_DIR REPORT_JSON`
use ferroni::api::{Regex, SearchOptions};
use ferroni::backtrack_rewrite::BacktrackingRewrite;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

fn patterns(value: &Value, out: &mut BTreeSet<String>, occurrences: &mut usize) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(key.as_str(), "match" | "begin" | "end") {
                    if let Some(pattern) = value.as_str() {
                        out.insert(pattern.to_owned());
                        *occurrences += 1;
                    }
                }
                patterns(value, out, occurrences);
            }
        }
        Value::Array(array) => {
            for value in array {
                patterns(value, out, occurrences);
            }
        }
        _ => {}
    }
}

type CaptureTrace = Option<Vec<Option<(usize, usize)>>>;

fn trace(re: &Regex, text: &str) -> Result<CaptureTrace, ferroni::error::RegexError> {
    let options = SearchOptions::new()
        .retry_limit_in_match(100_000)
        .retry_limit_in_search(100_000);
    Ok(re.captures_with(text, options)?.map(|captures| {
        (0..captures.len())
            .map(|i| captures.get(i).map(|m| (m.start(), m.end())))
            .collect()
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: backtrack_rewrite_census GRAMMAR_DIR REPORT_JSON".into());
    }
    let mut paths = std::fs::read_dir(&args[1])?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "json"));
    paths.sort();
    let mut extracted = 0;
    let mut occurrences = 0;
    let mut compiled = 0;
    let mut rewritten = Vec::new();
    let mut refused = Vec::new();
    let mut compile_errors = Vec::new();
    let mut comparisons = 0;
    // Bounded diagnostic corpus, including valid floats, malformed separators,
    // failed suffixes, alternative start positions, and non-ASCII prefixes.
    // This census complements the exhaustive differential suite, not replaces it.
    let mut corpus = vec!["".to_owned(), "x".to_owned(), "é😀".to_owned()];
    for run in ["0", "123", "12_34", "12_34_", "12__34", "123456"] {
        for tail in ["", ".0", ".12E+3", ".x", "..1", ".1_"] {
            corpus.push(format!("x {run}{tail} é {run}{tail}"));
        }
    }
    corpus.extend(
        [
            "0x1f", "0X7F", "0o71", "0O7", "12_3", "12_34", "123x", "123_", "123__", "é123",
            "123é", "😀 123",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    for path in &paths {
        let grammar: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let mut expressions = BTreeSet::new();
        patterns(&grammar, &mut expressions, &mut occurrences);
        extracted += expressions.len();
        let name = path.file_name().unwrap().to_string_lossy();
        for pattern in expressions {
            let fast = match Regex::builder(&pattern).optimize_backtracking(true).build() {
                Ok(re) => re,
                Err(error) => {
                    match Regex::new(&pattern) {
                        Err(expected) if expected == error => {}
                        _ => {
                            return Err(format!(
                                "new compilation failure in {name}: {pattern:?}: {error}"
                            )
                            .into());
                        }
                    }
                    compile_errors.push(
                        json!({"grammar": name, "pattern": pattern, "error": error.to_string()}),
                    );
                    continue;
                }
            };
            compiled += 1;
            let reports = fast.backtracking_rewrites();
            if reports.is_empty() {
                continue;
            }
            let record = json!({"grammar": name, "pattern": pattern,
                "reports": reports.iter().map(|report| format!("{report:?}")).collect::<Vec<_>>()});
            if reports.iter().any(|report| {
                matches!(
                    report,
                    BacktrackingRewrite::AtomicDecimalLoop
                        | BacktrackingRewrite::PossessiveDecimalDigits
                        | BacktrackingRewrite::AtomicDecimalTail
                        | BacktrackingRewrite::DeterministicDecimalTail
                )
            }) {
                let plain = Regex::new(&pattern)?;
                for text in &corpus {
                    let expected = trace(&plain, text)?;
                    let actual = trace(&fast, text)?;
                    if actual != expected {
                        return Err(format!(
                            "capture difference in {name}: {pattern:?} on {text:?}"
                        )
                        .into());
                    }
                    comparisons += 1;
                }
                rewritten.push(record);
            } else {
                refused.push(record);
            }
        }
    }
    let summary = json!({"grammar_files": paths.len(), "patterns_per_grammar_deduplicated": extracted, "pattern_occurrences": occurrences,
        "compiled": compiled, "compile_errors": compile_errors.len(),
        "rewritten_patterns": rewritten.len(), "refused_patterns": refused.len(),
        "capture_comparisons": comparisons, "corpus_inputs": corpus.len()});
    println!("{}", serde_json::to_string_pretty(&summary)?);
    let report = json!({"summary": summary, "rewritten": rewritten, "refused": refused,
        "compile_errors": compile_errors});
    std::fs::write(
        Path::new(&args[2]),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
