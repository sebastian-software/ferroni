//! PROTOTYPE — issue #44 ECMAScript look-behind research spike.
//!
//! Question: can Ferroni's existing compiler and VM execute captures and
//! nested look-ahead inside negative look-behind with Node-compatible match
//! positions and capture state when validation is enabled only for an
//! experimental syntax?
//!
//! Run with:
//! cargo run --example research_ecmascript_lookbehind

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use ferroni::prelude::Regex;
use ferroni::regsyntax::prototype_ecmascript_lookbehind_syntax;
use serde_json::{json, Value};

struct Case {
    label: &'static str,
    pattern: &'static str,
    subject: &'static str,
    names: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        label: "capture is unset when negative look-behind succeeds",
        pattern: r"(?<!(a))b",
        subject: "b",
        names: &[],
    },
    Case {
        label: "capture makes negative look-behind fail",
        pattern: r"(?<!(a))b",
        subject: "ab",
        names: &[],
    },
    Case {
        label: "capture numbering after negative look-behind",
        pattern: r"(?<!(a))(b)",
        subject: "b",
        names: &[],
    },
    Case {
        label: "capture numbering after a failed candidate",
        pattern: r"(?<!(a))(b)",
        subject: "ab",
        names: &[],
    },
    Case {
        label: "numbered backreference to unset negative capture",
        pattern: r"(?<!(a))b\1",
        subject: "b",
        names: &[],
    },
    Case {
        label: "numbered backreference must not retain failed assertion state",
        pattern: r"(?<!(a))b\1",
        subject: "abb",
        names: &[],
    },
    Case {
        label: "named negative capture and later named capture",
        pattern: r"(?<!(?<inside>a))(?<outside>b)",
        subject: "b",
        names: &["inside", "outside"],
    },
    Case {
        label: "named backreference to unset negative capture",
        pattern: r"(?<!(?<inside>a))b\k<inside>",
        subject: "b",
        names: &["inside"],
    },
    Case {
        label: "capture under variable-length quantified look-behind",
        pattern: r"(?<!\\(\\\\)*)x",
        subject: "x",
        names: &[],
    },
    Case {
        label: "variable-length captured body rejects escaped target",
        pattern: r"(?<!\\(\\\\)*)x",
        subject: r"\x",
        names: &[],
    },
    Case {
        label: "positive look-ahead in negative look-behind succeeds",
        pattern: r"(?<!a(?=b))b",
        subject: "b",
        names: &[],
    },
    Case {
        label: "positive look-ahead in negative look-behind rejects",
        pattern: r"(?<!a(?=b))b",
        subject: "ab",
        names: &[],
    },
    Case {
        label: "negative look-ahead in negative look-behind rejects",
        pattern: r"(?<!a(?!c))b",
        subject: "ab",
        names: &[],
    },
    Case {
        label: "negative look-ahead in negative look-behind succeeds",
        pattern: r"(?<!a(?!c))b",
        subject: "acb",
        names: &[],
    },
    Case {
        label: "look-ahead stays scoped to its alternation branch A",
        pattern: r"(?<!a|b(?=c))c",
        subject: "ac",
        names: &[],
    },
    Case {
        label: "look-ahead stays scoped to its alternation branch B",
        pattern: r"(?<!a|b(?=c))c",
        subject: "bc",
        names: &[],
    },
    Case {
        label: "look-ahead alternation allows unrelated prefix",
        pattern: r"(?<!a|b(?=c))c",
        subject: "dc",
        names: &[],
    },
    Case {
        label: "reported apostrophe case rejects contraction",
        pattern: r"(?<!n(?='t))'",
        subject: "can't",
        names: &[],
    },
    Case {
        label: "reported apostrophe case accepts possessive",
        pattern: r"(?<!n(?='t))'",
        subject: "John's",
        names: &[],
    },
    Case {
        label: "capture inside nested look-ahead remains unset on success",
        pattern: r"(?<!a(?=(b)))(b)",
        subject: "b",
        names: &[],
    },
    Case {
        label: "capture inside nested look-ahead rejects matching prefix",
        pattern: r"(?<!a(?=(b)))(b)",
        subject: "ab",
        names: &[],
    },
];

const MATRIX_PATTERNS: &[(&str, &str, &[&str])] = &[
    (
        "optional capture with ECMAScript backreference",
        r"(a)?b\1",
        &[],
    ),
    (
        "alternative capture with ECMAScript backreference",
        r"(?:(a)|x)b\1",
        &[],
    ),
    ("single capture", r"(?<!(a))b", &[]),
    ("quantified capture", r"(?<!(a+))b", &[]),
    ("captured alternatives", r"(?<!(a)|(bc))d", &[]),
    ("positive look-ahead", r"(?<!a(?=b))b", &[]),
    ("negative look-ahead", r"(?<!a(?!c))b", &[]),
    ("look-ahead in alternative", r"(?<!a|b(?=c))c", &[]),
    ("look-ahead after unbounded prefix", r"(?<!a+(?=b))b", &[]),
    ("zero-width positive look-ahead", r"(?<!(?=a))a", &[]),
    ("zero-width negative look-ahead", r"(?<!(?!a))a", &[]),
    ("nested optional capture", r"(?<!((ab)?))c", &[]),
    ("internal numbered backreference", r"(?<!(a)\1)b", &[]),
    ("look-ahead internal backreference", r"(?<!(a)(?=\1))a", &[]),
    (
        "later reference to nested look-ahead capture",
        r"(?<!a(?=(b)))(b)\1",
        &[],
    ),
    (
        "later references to alternative captures",
        r"(?<!(a)|(b))c\1\2",
        &[],
    ),
    (
        "named capture and later reference",
        r"(?<!(?<inside>a))b\k<inside>",
        &["inside"],
    ),
    ("multibyte capture", r"(?<!(é))x", &[]),
    ("multibyte look-ahead", r"(?<!é(?=x))x", &[]),
    ("supplementary capture", r"(?<!(🙂))x", &[]),
];

const MATRIX_SUBJECTS: &[&str] = &[
    "", "a", "b", "c", "d", "x", "ab", "aab", "aaab", "bc", "abc", "acb", "dc", "abab", "abb",
    "aac", "abcd", "éx", "aéx", "🙂x", "a🙂x", r"\x", r"\\x",
];

fn ferroni_outcome(case: &Case) -> (Value, Option<String>) {
    let regex = match Regex::builder(case.pattern)
        .syntax(prototype_ecmascript_lookbehind_syntax())
        .build()
    {
        Ok(regex) => regex,
        Err(error) => {
            return (
                json!({ "compile": "error", "match": null }),
                Some(error.to_string()),
            );
        }
    };

    let Some(captures) = regex.captures(case.subject) else {
        return (json!({ "compile": "ok", "match": null }), None);
    };

    let capture_values: Vec<Value> = captures
        .iter()
        .map(|capture| match capture {
            Some(found) => json!(found.as_str()),
            None => Value::Null,
        })
        .collect();
    let whole = captures.get(0).expect("group zero must exist");
    let named: BTreeMap<&str, Value> = case
        .names
        .iter()
        .map(|name| {
            let value = captures
                .name(name)
                .map_or(Value::Null, |found| json!(found.as_str()));
            (*name, value)
        })
        .collect();

    (
        json!({
            "compile": "ok",
            "match": {
                "start": whole.start(),
                "end": whole.end(),
                "captures": capture_values,
                "named": named,
            }
        }),
        None,
    )
}

fn node_outcome(case: &Case) -> Value {
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/research_ecmascript_lookbehind_node.js");
    let output = Command::new("node")
        .arg(helper)
        .env("FERRONI_SPIKE_PATTERN", case.pattern)
        .env("FERRONI_SPIKE_SUBJECT", case.subject)
        .env(
            "FERRONI_SPIKE_NAMES",
            serde_json::to_string(case.names).expect("names serialize"),
        )
        .output()
        .expect("Node.js is required for this research spike");
    assert!(output.status.success(), "Node helper failed");
    let mut value: Value = serde_json::from_slice(&output.stdout).expect("Node returned JSON");
    if let Some(object) = value.as_object_mut() {
        object.remove("detail");
    }
    value
}

fn main() {
    let verbose = std::env::args().any(|argument| argument == "--verbose");
    println!("PROTOTYPE — ECMAScript negative look-behind differential");
    println!("Comparing Ferroni experimental syntax with Node.js\n");

    for pattern in [r"(?<!(a))b", r"(?<!a(?=b))b"] {
        assert!(
            Regex::new(pattern).is_err(),
            "the default Oniguruma syntax must continue to reject {pattern:?}"
        );
    }
    println!("Default Oniguruma validation remains unchanged.\n");

    let mut equal = 0;
    let mut different = 0;
    let mut total = 0;

    let mut compare = |case: &Case| {
        total += 1;
        let (ferroni, ferroni_detail) = ferroni_outcome(case);
        let node = node_outcome(case);
        let matches = ferroni == node;
        if matches {
            equal += 1;
        } else {
            different += 1;
        }

        if verbose || !matches {
            println!(
                "{:>3}. {} — {}",
                total,
                if matches { "MATCH" } else { "DIFF" },
                case.label
            );
            println!("     pattern: {:?}", case.pattern);
            println!("     subject: {:?}", case.subject);
            println!("     ferroni: {}", ferroni);
            if let Some(detail) = ferroni_detail {
                println!("     ferroni detail: {detail}");
            }
            println!("     node:    {}\n", node);
        }
    };

    for case in CASES {
        compare(case);
    }
    for (label, pattern, names) in MATRIX_PATTERNS {
        for subject in MATRIX_SUBJECTS {
            compare(&Case {
                label,
                pattern,
                subject,
                names,
            });
        }
    }

    println!("Summary: {equal} matched, {different} differed, {total} total");
    if !verbose {
        println!("Run again with --verbose to print every matching case.");
    }
}
