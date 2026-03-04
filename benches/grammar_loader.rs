// Shared TextMate grammar loader for benchmark suites.
//
// Embeds full, unmodified Shiki grammars (TypeScript, CSS, Rust) and extracts
// every `match` and `begin` pattern from the grammar's pattern tree and
// repository. Filters to patterns that Ferroni can compile.

use ferroni::scanner::Scanner;
use std::collections::HashSet;

const TYPESCRIPT_JSON: &str = include_str!("grammars/typescript.json");
const CSS_JSON: &str = include_str!("grammars/css.json");
const RUST_JSON: &str = include_str!("grammars/rust.json");

/// All compilable patterns from the full Shiki TypeScript grammar.
pub fn typescript_patterns() -> Vec<String> {
    extract_patterns(TYPESCRIPT_JSON)
}

/// All compilable patterns from the full Shiki CSS grammar.
pub fn css_patterns() -> Vec<String> {
    extract_patterns(CSS_JSON)
}

/// All compilable patterns from the full Shiki Rust grammar.
pub fn rust_patterns() -> Vec<String> {
    extract_patterns(RUST_JSON)
}

/// Extract and deduplicate all `match` and `begin` patterns from a TextMate
/// grammar JSON, then filter to those that Ferroni's Scanner can compile.
fn extract_patterns(json: &str) -> Vec<String> {
    let root: serde_json::Value = serde_json::from_str(json).expect("invalid grammar JSON");
    let mut seen = HashSet::new();
    let mut patterns = Vec::new();

    // Walk top-level "patterns" array
    if let Some(arr) = root.get("patterns").and_then(|v| v.as_array()) {
        for entry in arr {
            collect_patterns(entry, &mut seen, &mut patterns);
        }
    }

    // Walk "repository" entries
    if let Some(repo) = root.get("repository").and_then(|v| v.as_object()) {
        for (_key, entry) in repo {
            collect_patterns(entry, &mut seen, &mut patterns);
            // Each repository entry may also have a "patterns" array
            if let Some(arr) = entry.get("patterns").and_then(|v| v.as_array()) {
                for item in arr {
                    collect_patterns(item, &mut seen, &mut patterns);
                }
            }
        }
    }

    // Filter to compilable patterns
    patterns
        .into_iter()
        .filter(|p| Scanner::new(&[p.as_str()]).is_ok())
        .collect()
}

/// Recursively collect `match` and `begin` fields from a pattern entry.
fn collect_patterns(
    value: &serde_json::Value,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    // Extract "match" field
    if let Some(s) = value.get("match").and_then(|v| v.as_str()) {
        if seen.insert(s.to_string()) {
            out.push(s.to_string());
        }
    }

    // Extract "begin" field
    if let Some(s) = value.get("begin").and_then(|v| v.as_str()) {
        if seen.insert(s.to_string()) {
            out.push(s.to_string());
        }
    }

    // Recurse into nested "patterns" arrays
    if let Some(arr) = value.get("patterns").and_then(|v| v.as_array()) {
        for item in arr {
            collect_patterns(item, seen, out);
        }
    }

    // Recurse into "captures", "beginCaptures", "endCaptures" which may have nested patterns
    for key in &["captures", "beginCaptures", "endCaptures"] {
        if let Some(obj) = value.get(*key).and_then(|v| v.as_object()) {
            for (_k, cap) in obj {
                if let Some(arr) = cap.get("patterns").and_then(|v| v.as_array()) {
                    for item in arr {
                        collect_patterns(item, seen, out);
                    }
                }
            }
        }
    }
}
