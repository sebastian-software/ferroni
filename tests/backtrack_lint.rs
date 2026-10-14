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
        r"(\s*\w+)*$",
        r"([0-9]+(_?))+(\.)([0-9]+)",
        r"(?:a+b?)+$",
        r"((a|b)+)*$",
        r"(a+a)+$",
        r"0x(?:\h+_?)+$",
        r"(?:[0-9]+_?)+\b",
    ] {
        assert!(
            risks(pattern).contains(&BacktrackRisk::NestedQuantifier),
            "{pattern}"
        );
    }
}

#[test]
fn flags_overlapping_alternation() {
    for pattern in [
        r"(a|aa)*$",
        r"(?:foo|foobar)+$",
        r"(\w|\d)*$",
        r"(?:a+|b)*c",
    ] {
        assert!(!risks(pattern).is_empty(), "{pattern}");
    }
    assert_eq!(risks(r"(a|aa)*$"), [BacktrackRisk::OverlappingAlternation]);
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
        // Ends the pattern: nothing after the loop can fail.
        r"0x(?:\h+_?)+",
        r"(?:[0-9]+_?)+",
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
    let scanner = Scanner::new(&["ok+", "(a+)+$", r"(?:a|aa)*$"]).unwrap();
    let counts: Vec<usize> = scanner.warnings().iter().map(Vec::len).collect();
    assert_eq!(counts, [0, 1, 1]);
}

#[test]
fn trailing_loops_count_when_the_search_is_exhaustive() {
    use ferroni::oniguruma::ONIG_OPTION_FIND_LONGEST;
    assert!(risks(r"(?:[0-9]+_?)+").is_empty());
    let re = Regex::builder(r"(?:[0-9]+_?)+")
        .option(ONIG_OPTION_FIND_LONGEST)
        .build()
        .unwrap();
    assert_eq!(re.backtracking_warnings().len(), 1);
}

/// Class and type first bytes, with and without negation and case folding.
/// The expected findings were taken from the check before its first-byte
/// sets were computed word-wise, so any drift in those sets shows here.
#[test]
fn class_and_type_first_bytes_keep_their_findings() {
    use BacktrackRisk::{NestedQuantifier as N, OverlappingAlternation as O};
    let cases: &[(&str, &[BacktrackRisk])] = &[
        (r#"((?:a*|(?>[[:alpha:]])+)+*){1,}"#, &[N]),
        (r#"(?:[^0-9]|(?i)[^a-c]|(?i)\w)+$"#, &[O]),
        (r#"(?:[[:^digit:]]|\s|\S)+b"#, &[O]),
        (r#"((?:.|\Sa)+){0}"#, &[O]),
        (r#"(?:(?:.|[a-c])*|[^0-9]){2,}"#, &[O]),
        (r#"(?:(?i)[a-c]|\h)*?\z"#, &[O]),
        (r#"(?:[a-c]|\w)*b"#, &[O]),
        (r#"(?:[\w_]|\\b|\h)*{0}"#, &[O]),
        (r#"((?:(?>[^\w])+|x?)*){2}"#, &[N]),
        (r#"(((?:[\d.]|[0-9]|(?i)[a-c])+)*){2}*+\z"#, &[N, O]),
        (r#"(?:[[:word:]]|\p{L}){2,}\z"#, &[O]),
        (r#"(?:[[:space:]]|[ß]++){0,3}"#, &[]),
        (r#"(?:[[:space:]]|\S|[ab]){2}"#, &[]),
        (r#"(?i)a[a-c]"#, &[]),
        (r#"([^0-9]){2}"#, &[]),
        (r#"\W[A-Z]"#, &[]),
        (r#"([^\w]){1,}?"#, &[]),
        (r#"([0-9])+*([^\w]){1,}"#, &[]),
        (r#"(?>[^\w][\x00-\x7F]){2,}"#, &[]),
        (r#"[A-Z](?>[^0-9])*{2}ss"#, &[]),
        (r#"\d[\x{100}-\x{200}]"#, &[]),
        (r#"(?:(?i)a|[^0-9]|\S)*"#, &[]),
        (r#"(?:[ab]|\S|\x{e9})*+b"#, &[]),
        (r#"[0-9]([A-Z])*\s"#, &[]),
        (r#"(?:(?i)a|[a-z\x7F-\x{10FFFF}])*?"#, &[]),
        (r#"(?:\d|\D)+x"#, &[]),
        (r#"(?:[^\d]|\d)*x"#, &[]),
        (r#"(?:\D|[^\D])*x"#, &[]),
        (r#"(?:[[:^alpha:]]|[[:alpha:]])+x"#, &[]),
        (r#"(?:[^a]|b)+x"#, &[]),
        (r#"(?:\W|\w)+x"#, &[]),
        (r#"(?:[^\w]|\s)+$"#, &[O]),
        (r#"(?:\S|[^\w])*x"#, &[O]),
        (r#"(?i)(?:[^a-c]|\w)*x"#, &[O]),
    ];
    for (pattern, expected) in cases {
        assert_eq!(risks(pattern), *expected, "{pattern}");
    }
}
