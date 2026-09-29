//! Compile-time backtracking check: shapes it flags, shapes it leaves alone,
//! and the opt-in rejection.

use ferroni::api::Regex;
use ferroni::backtrack_lint::BacktrackRisk;
use ferroni::scanner::Scanner;

fn risks(pattern: &str) -> Vec<BacktrackRisk> {
    Regex::new(pattern)
        .unwrap()
        .backtracking_warnings()
        .iter()
        .map(|w| w.risk)
        .collect()
}

#[test]
fn flags_nested_quantifiers() {
    for pattern in [
        r"(a+)+$",
        r"(a*)*b",
        r"(\w+\s*)+$",
        r"(\s*\w+)*",
        r"([0-9]+(_?))+(\.)([0-9]+)",
        r"(?:a+b?)+",
        r"((a|b)+)*",
        r"(a+a)+",
        r"0x(?:\h+_?)+",
    ] {
        assert!(
            risks(pattern).contains(&BacktrackRisk::NestedQuantifier),
            "{pattern}"
        );
    }
}

#[test]
fn flags_overlapping_alternation() {
    for pattern in [r"(a|aa)*", r"(?:foo|foobar)+", r"(\w|\d)*", r"(?:a+|b)*c"] {
        assert!(!risks(pattern).is_empty(), "{pattern}");
    }
    assert_eq!(risks(r"(a|aa)*"), [BacktrackRisk::OverlappingAlternation]);
}

#[test]
fn leaves_unambiguous_patterns_alone() {
    for pattern in [
        r"(a+b)+",
        r"(?:,\s*\w+)*",
        r#"(?:[^"\\]|\\.)*"#,
        r"(?:ab|ac)*",
        r"(?>a+)+",
        r"(?:a++)+",
        r"a+a+",
        r"\w+(\s+\w+)*",
        r"(?:[a-z]+\.)*[a-z]+",
        r"(?:a|b)*",
        r"(a+)(b+)",
        r"(a{2}){3}",
        // Delimited: the next iteration cannot start with what the inner repeat eats.
        r"(?:\[[^\[]*?])*",
        // A look-around decides the way.
        r"(?:\*(?!/)|[^*])*\*/",
        r#"(["'])(`\1|.(?<!\1))*\1"#,
        // Case folding lists `s` and `ss` side by side.
        r"(?i)[_a-z\x7F-\x{10FFFF}]*",
    ] {
        assert!(risks(pattern).is_empty(), "{pattern}: {:?}", risks(pattern));
    }
}

#[test]
fn flagging_does_not_change_matching() {
    let re = Regex::new(r"(a+)+b").unwrap();
    assert_eq!(re.backtracking_warnings().len(), 1);
    assert_eq!(re.find("xaaab").unwrap().as_str(), "aaab");
    assert!(re.backtracking_warnings()[0].to_string().contains("nested"));
}

#[test]
fn rejection_is_opt_in() {
    assert!(Regex::builder(r"(a+)+$").build().is_ok());
    assert!(
        Regex::builder(r"(a+)+$")
            .reject_backtracking_risks(true)
            .build()
            .is_err()
    );
    assert!(
        Regex::builder(r"(a+b)+$")
            .reject_backtracking_risks(true)
            .build()
            .is_ok()
    );
}

#[test]
fn scanner_reports_per_pattern() {
    let scanner = Scanner::new(&["ok+", "(a+)+$", r"(?:a|aa)*"]).unwrap();
    let counts: Vec<usize> = scanner.warnings().iter().map(Vec::len).collect();
    assert_eq!(counts, [0, 1, 1]);
}
