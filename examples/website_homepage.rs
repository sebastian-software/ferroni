//! Generate the real output the home page shows: Ferroni's match for every
//! selected feature, a date read by named captures and rewritten, the scanner
//! steps on an ASCII line and on a line where bytes and UTF-16 units differ, and
//! the backtracking guardrails. `pnpm sample:write` in `docs/` writes the result
//! to `docs/app/data/home-specimens.json`; `pnpm sample:check` compares it.

use std::path::Path;
use std::time::Duration;

use ferroni::prelude::*;
use serde_json::{Value, json};

const DATE_PATTERN: &str = r"(?<=Date: )(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})";
const DATE_TEXT: &str = "Date: 2026-09-26";
const DATE_REPLACEMENT: &str = "${day}.${month}.${year}";
const SCANNER_PATTERNS: [&str; 3] = [r"\b(function|const|let|var)\b", r#""[^"]*""#, r"//.*$"];
const ASCII_LINE: &str = r#"const x = "hello" // greeting"#;
const WIDE_LINE: &str = r#"let größe = "Straße" // Maß"#;
const RISKY_PATTERN: &str = r"(a+)+$";
const HOSTILE_PATTERN: &str = r"(a+)+b";
const HOSTILE_REPEAT: usize = 40;
const TIME_LIMIT_MS: u64 = 50;

/// The features and engine support checked by `tests/regex_feature_matrix.rs`.
fn capabilities() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/app/data/regex-capabilities.json");
    let text = std::fs::read_to_string(path).expect("the capability matrix is readable");
    serde_json::from_str(&text).expect("the capability matrix is JSON")
}

fn features() -> Result<Vec<Value>, RegexError> {
    let capabilities = capabilities();
    let mut features = Vec::new();
    for feature in capabilities["features"].as_array().expect("a feature list") {
        let pattern = feature["pattern"].as_str().expect("a pattern");
        let input = feature["input"].as_str().expect("an input");
        let found = Regex::new(pattern)?
            .find(input)
            .expect("every selected feature matches its input");
        features.push(json!({
            "id": feature["id"], "label": feature["label"], "pattern": pattern,
            "input": input, "support": feature["support"],
            "match": {
                "before": &input[..found.start()], "matched": found.as_str(),
                "after": &input[found.end()..], "start": found.start(), "end": found.end(),
            },
        }));
    }
    Ok(features)
}

fn date() -> Result<Value, RegexError> {
    let regex = Regex::new(DATE_PATTERN)?;
    let captures = regex.captures(DATE_TEXT).expect("the sample date matches");
    let named: Vec<Value> = ["year", "month", "day"]
        .iter()
        .map(|name| {
            let value = captures.name(name).expect("the named group exists");
            json!({ "name": name, "value": value.as_str(), "start": value.start(), "end": value.end() })
        })
        .collect();
    Ok(json!({
        "pattern": DATE_PATTERN, "input": DATE_TEXT, "captures": named,
        "replacement": DATE_REPLACEMENT,
        "replaced": regex.replace_all(DATE_TEXT, DATE_REPLACEMENT),
    }))
}

fn scan(scanner: &mut Scanner, input: &str) -> Value {
    let mut steps = Vec::new();
    let mut from = 0;
    while let Some(found) = scanner.find_next_match(input, from, ScannerFindOptions::NONE) {
        let hit = &found.captures()[0];
        steps.push(json!({
            "from": from, "index": found.index, "start": hit.start, "end": hit.end,
            "utf16Start": input[..hit.start].encode_utf16().count(),
            "utf16End": input[..hit.end].encode_utf16().count(),
            "matched": &input[hit.start..hit.end],
        }));
        from = hit.end;
    }
    json!({ "input": input, "steps": steps })
}

fn guardrails() -> Result<Value, RegexError> {
    let warnings: Vec<String> = Regex::new(RISKY_PATTERN)?
        .backtracking_warnings()
        .iter()
        .map(ToString::to_string)
        .collect();
    let strict = Regex::builder(RISKY_PATTERN)
        .reject_backtracking_risks(true)
        .build()
        .expect_err("the strict build refuses the risky pattern");
    let hostile = "a".repeat(HOSTILE_REPEAT);
    let options = SearchOptions::new().timeout(Duration::from_millis(TIME_LIMIT_MS));
    let stopped = Regex::new(HOSTILE_PATTERN)?
        .find_with(&hostile, options)
        .expect_err("the time limit stops the hostile search");
    Ok(json!({
        "pattern": RISKY_PATTERN, "warnings": warnings, "strictError": strict.to_string(),
        "timeoutPattern": HOSTILE_PATTERN, "timeoutRepeat": HOSTILE_REPEAT,
        "timeoutMs": TIME_LIMIT_MS, "timeoutError": format!("{stopped:?}"),
    }))
}

fn main() -> Result<(), RegexError> {
    let mut scanner = Scanner::new(&SCANNER_PATTERNS)?;
    let ascii = scan(&mut scanner, ASCII_LINE);
    let wide = scan(&mut scanner, WIDE_LINE);
    let specimens = json!({
        "ferroni": env!("CARGO_PKG_VERSION"),
        "features": features()?,
        "date": date()?,
        "scanner": { "patterns": SCANNER_PATTERNS, "ascii": ascii, "wide": wide },
        "guardrails": guardrails()?,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&specimens).expect("the specimens serialize")
    );
    Ok(())
}
