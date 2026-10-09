// Shared TextMate grammar loader for benchmark suites.
//
// Embeds full, unmodified Shiki grammars (TypeScript, CSS, Rust) and extracts
// every `match` and `begin` pattern from the grammar's pattern tree and
// repository. Filters to patterns that Ferroni can compile. External snapshot
// pins and file hashes live in benches/battle_inputs.toml.

use ferroni::scanner::Scanner;
use std::collections::{HashMap, HashSet};

const TYPESCRIPT_JSON: &str = include_str!("grammars/typescript.json");
#[allow(dead_code)]
const CSS_JSON: &str = include_str!("grammars/css.json");
#[allow(dead_code)]
const RUST_JSON: &str = include_str!("grammars/rust.json");

/// All compilable patterns from the full Shiki TypeScript grammar.
pub fn typescript_patterns() -> Vec<String> {
    extract_patterns(TYPESCRIPT_JSON)
}

#[allow(dead_code)]
/// All compilable patterns from the full Shiki CSS grammar.
pub fn css_patterns() -> Vec<String> {
    extract_patterns(CSS_JSON)
}

#[allow(dead_code)]
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

/// Every rule's pattern list of a TextMate grammar JSON, as vscode-textmate
/// compiles the rule's scanner: a rule is any object with a `patterns`
/// array (the grammar itself, a repository entry, a `begin`/`end` rule, a
/// bare group, a capture with patterns); its list holds the rule's `end`
/// (or `while`) pattern first, then the `match` or `begin` of each entry of
/// its `patterns`, with `include`s of the grammar's own repository
/// (`#name`), of the grammar itself (`$self`, `$base`) and of bare groups
/// resolved, each once. An include of another grammar is left out, as are
/// the patterns the Scanner rejects (an `end` with a back reference to its
/// `begin`, which vscode-textmate substitutes); lists left empty are
/// dropped.
#[allow(dead_code)]
pub fn rule_pattern_lists(json: &str) -> Vec<Vec<String>> {
    let root: serde_json::Value = serde_json::from_str(json).expect("invalid grammar JSON");
    let mut repository = HashMap::new();
    collect_repositories(&root, &mut repository);
    let mut rules = Vec::new();
    collect_rules(&root, &mut rules);
    let mut lists = Vec::new();
    for rule in rules {
        let mut list = Vec::new();
        let mut seen = HashSet::new();
        if let Some(end) = rule
            .get("end")
            .or_else(|| rule.get("while"))
            .and_then(|v| v.as_str())
        {
            seen.insert(end.to_string());
            list.push(end.to_string());
        }
        let mut visited = HashSet::new();
        if let Some(entries) = rule.get("patterns").and_then(|v| v.as_array()) {
            collect_rule_patterns(
                entries,
                &root,
                &repository,
                &mut visited,
                &mut seen,
                &mut list,
            );
        }
        list.retain(|p| Scanner::new(&[p.as_str()]).is_ok());
        if !list.is_empty() {
            lists.push(list);
        }
    }
    lists
}

/// Every `repository` object of the grammar merged into one map, the outer
/// ones taking precedence.
fn collect_repositories<'a>(
    value: &'a serde_json::Value,
    out: &mut HashMap<String, &'a serde_json::Value>,
) {
    if let Some(repo) = value.get("repository").and_then(|v| v.as_object()) {
        for (key, entry) in repo {
            out.entry(key.clone()).or_insert(entry);
        }
    }
    match value {
        serde_json::Value::Object(map) => {
            for (_, child) in map {
                collect_repositories(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_repositories(item, out);
            }
        }
        _ => {}
    }
}

/// Every object of the grammar with a `patterns` array.
fn collect_rules<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("patterns").is_some_and(|v| v.is_array()) {
                out.push(value);
            }
            for (_, child) in map {
                collect_rules(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_rules(item, out);
            }
        }
        _ => {}
    }
}

/// The scanner patterns of a `patterns` array, includes resolved.
fn collect_rule_patterns<'a>(
    entries: &'a [serde_json::Value],
    root: &'a serde_json::Value,
    repository: &HashMap<String, &'a serde_json::Value>,
    visited: &mut HashSet<String>,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    for entry in entries {
        let own = entry
            .get("match")
            .or_else(|| entry.get("begin"))
            .and_then(|v| v.as_str());
        if let Some(pattern) = own {
            if seen.insert(pattern.to_string()) {
                out.push(pattern.to_string());
            }
            continue;
        }
        let included = match entry.get("include").and_then(|v| v.as_str()) {
            Some(name) if name == "$self" || name == "$base" => {
                visited.insert("$self".to_string()).then_some(root)
            }
            Some(name) => match name.strip_prefix('#') {
                Some(key) => visited
                    .insert(key.to_string())
                    .then(|| repository.get(key).copied())
                    .flatten(),
                // Another grammar.
                None => None,
            },
            None => Some(entry),
        };
        let Some(rule) = included else {
            continue;
        };
        if let Some(pattern) = rule
            .get("match")
            .or_else(|| rule.get("begin"))
            .and_then(|v| v.as_str())
        {
            if seen.insert(pattern.to_string()) {
                out.push(pattern.to_string());
            }
        } else if let Some(nested) = rule.get("patterns").and_then(|v| v.as_array()) {
            collect_rule_patterns(nested, root, repository, visited, seen, out);
        }
    }
}

/// Recursively collect `match` and `begin` fields from a pattern entry.
fn collect_patterns(value: &serde_json::Value, seen: &mut HashSet<String>, out: &mut Vec<String>) {
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
