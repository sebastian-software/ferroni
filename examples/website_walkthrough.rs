//! Generate the highlighted matches and scanner steps shown on the home page.
use ferroni::prelude::*;
use serde_json::json;

fn main() -> Result<(), RegexError> {
    let mut syntax = Vec::new();
    for (title, pattern, input, explanation) in [
        (
            "Look-behind",
            r"(?<=Date: )\d{4}-\d{2}-\d{2}",
            "Date: 2026-09-26",
            "Require the Date: prefix without including it in the match.",
        ),
        (
            "Backreferences",
            r"\b(\w+)\s+\1\b",
            "Find the the repeated word.",
            "Match a word followed by the same word, using the first capture again.",
        ),
        (
            "Unicode properties",
            r"\p{Greek}+",
            "Hello, κόσμος!",
            "Select Greek letters by their Unicode property instead of listing characters.",
        ),
    ] {
        let re = Regex::new(pattern)?;
        let found = re.find(input).expect("the syntax example matches");
        syntax.push(json!({
            "title": title, "pattern": pattern, "input": input,
            "before": &input[..found.start()], "matched": found.as_str(),
            "after": &input[found.end()..], "explanation": explanation,
        }));
    }

    // This ASCII line has identical byte and UTF-16 offsets.
    let input = r#"const x = "hello" // greeting"#;
    let patterns = [r"\b(function|const|let|var)\b", r#""[^"]*""#, r"//.*$"];
    let mut scanner = Scanner::new(&patterns)?;
    let mut start = 0;
    let mut steps = Vec::new();
    while let Some(found) = scanner.find_next_match(input, start, ScannerFindOptions::NONE) {
        let capture = &found.captures()[0];
        steps.push(json!({
            "from": start, "index": found.index, "start": capture.start,
            "end": capture.end, "matched": &input[capture.start..capture.end],
        }));
        start = capture.end;
    }
    println!(
        "{}",
        json!({ "syntax": syntax, "scanner": { "input": input, "patterns": patterns, "steps": steps } })
    );
    Ok(())
}
