//! The single-pattern seek path (ADR-008) against the matcher: a `Regex` built
//! with the seek (`RegexBuilder::dfa`, on by default) answers the standard
//! `is_match*` and `find*` methods from its seek where the seek decides, and
//! every answer must equal the one the matcher gives under
//! `SearchOptions::new()`, which runs the same search without the path. The
//! same patterns built with `dfa(false)` must equal the matcher too, and the
//! two builds must agree on every standard answer, including matcher errors.
//!
//! The patterns are the production and tokenizer catalogs, fifty
//! hand-written ones (anchors, multi-line anchors, case folding, non-ASCII
//! classes, look-arounds, back references, atomic groups, absent operators,
//! `\K`, `\G`, and host-name shapes that get an accept seek), and random
//! subjects of valid UTF-8 and random bytes. Every subject is searched from
//! each character boundary and, through the bytes API, from each byte
//! offset. With the `ffi` feature, the standard `is_match` of the catalog
//! patterns is also compared with C Oniguruma.

use ferroni::api::{Regex, RegexBuilder, SearchOptions};
use serde_json::Value;

/// Deterministic pseudo-random numbers (a linear congruential generator).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Characters random subjects are built from: ASCII that the patterns use,
/// and non-ASCII characters whose folds and classes matter.
const ALPHABET: &[&str] = &[
    "a", "b", "c", "A", "B", "x", "0", "1", "9", "-", ".", ":", "@", "/", "_", " ", "\n", "(", ")",
    "é", "É", "ß", "ss", "€", "😀", "\u{212A}", "İ", "ä", "k", "K", "\u{3042}", "\u{80}",
];

fn random_subject(rng: &mut Lcg) -> Vec<u8> {
    let len = rng.below(14);
    let mut text = String::new();
    for _ in 0..len {
        text.push_str(ALPHABET[rng.below(ALPHABET.len())]);
    }
    text.into_bytes()
}

fn random_bytes(rng: &mut Lcg) -> Vec<u8> {
    let len = rng.below(10);
    (0..len).map(|_| rng.next() as u8).collect()
}

/// A pattern with its flags: ignore case, dot matches newline, extended,
/// multi-line anchors (`None` keeps the syntax's default).
#[derive(Clone)]
struct Case {
    pattern: Vec<u8>,
    ignore_case: bool,
    dotall: bool,
    extended: bool,
    multiline: Option<bool>,
}

impl Case {
    fn new(pattern: &str) -> Case {
        Case {
            pattern: pattern.as_bytes().to_vec(),
            ignore_case: false,
            dotall: false,
            extended: false,
            multiline: None,
        }
    }

    fn flags(mut self, ignore_case: bool, dotall: bool, multiline: Option<bool>) -> Case {
        self.ignore_case = ignore_case;
        self.dotall = dotall;
        self.multiline = multiline;
        self
    }

    fn compile(&self, dfa: bool) -> Option<Regex> {
        let mut builder = RegexBuilder::new_bytes(&self.pattern)
            .case_insensitive(self.ignore_case)
            .dot_matches_newline(self.dotall)
            .extended(self.extended)
            .dfa(dfa);
        if let Some(multiline) = self.multiline {
            builder = builder.multi_line_anchors(multiline);
        }
        builder.build().ok()
    }
}

fn hand_written() -> Vec<Case> {
    let plain = [
        "^abc$",
        r"\Aabc\z",
        r"abc\Z",
        r"^\d+$",
        "^x",
        r"a$",
        "(?i)straße",
        "(?i)k",
        "(?i)ss",
        "(?i)é",
        "[é]x",
        "[^a]b",
        r"[^\x{0}-\x{7f}]+",
        r"\w+@\w+",
        r"\W",
        r"\bfoo\b",
        r"\Bo\B",
        "(?<=a)b",
        "(?<!a)b",
        "a(?=b)",
        "a(?!b)",
        r"(\w)\1",
        "(?<x>a)\\k<x>",
        "(?>a+)b",
        "a*?b",
        "a{2,5}",
        "(a|ab)*c",
        "(a|aa)+$",
        "(x+x+)+y|.*",
        "(?>abc|ab)c",
        "a++b",
        "x|",
        r"\d{3}-\d{4}",
        "[[:alpha:]]+",
        r"\s+\S",
        ".+",
        ".",
        r"\G\w",
        r"\Kfoo",
        r"a\Kb",
        "(x)?(?(1)a|b)",
        "(?~abc)",
        "(*FAIL)|a",
        r"\p{L}+",
        "[a-z&&[^aeiou]]+",
        "[\u{3040}-\u{309F}]",
        "ß",
        r"\h+",
        "(?i)[a-z]+",
        r"\x{1F600}",
        "(?:foo|bar|baz)+qux",
        r"[^\n]*\n",
        r"\A\s*\z",
        "(?m:^)\\s*$",
        r"\R\R",
        r"\y\w",
        "(?s).",
        "ab|cd|é",
        "[aeiou]{3}x",
        "^",
        "$",
        "^$",
        "\\n$",
        "\\n^a",
        "x\\n$",
        "(?:^|x)a",
        "(?:a$|b)",
        "^a|^b\\n",
        "(?m)^a$",
        "(?m)a$|^$",
    ];
    let mut cases: Vec<Case> = plain.iter().map(|p| Case::new(p)).collect();
    // The same patterns under the flags that change their meaning.
    cases.push(Case::new("^abc$").flags(false, false, Some(false)));
    cases.push(Case::new("a$|b$").flags(false, false, Some(false)));
    cases.push(Case::new("\\w$").flags(false, false, Some(false)));
    cases.push(Case::new("^\\d+$").flags(false, false, Some(false)));
    cases.push(Case::new("a$b").flags(false, false, Some(false)));
    cases.push(Case::new("(?:x|\\n)$").flags(false, false, Some(false)));
    cases.push(Case::new("a$").flags(false, false, Some(true)));
    cases.push(Case::new("^x").flags(false, false, Some(false)));
    cases.push(Case::new("(?i)straße").flags(true, false, None));
    cases.push(Case::new("[a-c]+x").flags(true, false, None));
    cases.push(Case::new(".+").flags(false, true, None));
    cases.push(Case::new("a.b").flags(false, true, None));
    cases.push(Case::new("^\\w+$").flags(true, false, Some(false)));
    cases.push(Case::new("(?i)é+").flags(true, false, None));
    cases.push(Case::new("[^é]").flags(false, false, None));
    cases.push(Case::new("\\b").flags(false, false, None));
    cases.push(Case::new("\\B\\w").flags(false, false, Some(false)));
    // Inexact seeks with an accept seek (ADR-008): case folds of a class
    // left out, one-character looks folded into the label classes, and a
    // repetition `tune_next` made possessive.
    cases.push(Case::new(r"\.(?!-)[a-z¡-￿0-9-]{1,63}(?<!-)").flags(true, false, Some(false)));
    cases.push(
        Case::new(r"\.(?!-)(?:[a-z¡-￿-]{2,4}|xn--[a-z0-9]{1,3})(?<!-)\.?\z").flags(
            true,
            false,
            Some(false),
        ),
    );
    cases.push(Case::new(r"^[a-zß¡-ÿ]+$").flags(true, false, None));
    cases.push(Case::new(r"^(?!-)[a-z-]+(?<!-)$"));
    cases.push(Case::new(r"[a-z]+@[a-z]+\.com"));
    cases
}

/// Subjects for the accept seeks: host names with the characters whose case
/// folds are strings, the strings they fold to, and hyphens at label ends.
const HOST_SUBJECTS: &[&str] = &[
    "straße.de",
    "STRASSE.DE",
    "ſſ.de",
    "x.ß",
    "x.ss",
    "ﬀ.com",
    "a.ﬆ",
    "a.st",
    "xn--bcher.ch",
    "-a.b",
    "a-.b",
    "a.-b",
    "a.b-",
    "john@doe.com",
];

/// The production and tokenizer catalogs: the pattern and flags of each
/// case, and its inputs' texts (with the starts they are searched from).
fn catalog_cases() -> (Vec<Case>, Vec<(Vec<u8>, usize)>) {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut cases = Vec::new();
    let mut subjects = Vec::new();
    for file in [
        "benches/production_regex/catalog.json",
        "benches/tokenizer_regex/catalog.json",
    ] {
        let text = std::fs::read_to_string(format!("{root}/{file}")).expect("catalog");
        let catalog: Value = serde_json::from_str(&text).expect("catalog JSON");
        for case in catalog["cases"].as_array().expect("cases") {
            let pattern = case["pattern"].as_str().expect("pattern");
            let flags = &case["flags"];
            let flag = |name: &str| flags[name].as_bool() == Some(true);
            let mut entry = Case::new(pattern).flags(
                flag("ignore_case"),
                flag("dotall"),
                Some(flag("multiline")),
            );
            entry.extended = flag("extended");
            cases.push(entry);
            for input in case["inputs"].as_array().into_iter().flatten() {
                let text = input["text"].as_str().unwrap_or("");
                let start = input["start"].as_u64().unwrap_or(0) as usize;
                subjects.push((text.as_bytes().to_vec(), start));
            }
        }
    }
    (cases, subjects)
}

/// Every answer of the standard methods on `text` from `start` equals the
/// matcher's under `SearchOptions::new()`. Returns the number of
/// comparisons made (a matcher error is not compared).
fn check_search(re: &Regex, text: &[u8], start: usize) -> usize {
    let options = SearchOptions::new();
    let mut compared = 0;
    if let Ok(expected) = re.is_match_bytes_at_with(text, start, options) {
        assert_eq!(
            re.is_match_bytes_at(text, start),
            expected,
            "is_match of {:?} from {start} on {:?}",
            re.as_bytes(),
            text
        );
        compared += 1;
    }
    if let Ok(expected) = re.find_bytes_at_with(text, start, options) {
        let expected = expected.map(|m| m.range());
        assert_eq!(
            re.find_bytes_at(text, start).map(|m| m.range()),
            expected,
            "find of {:?} from {start} on {:?}",
            re.as_bytes(),
            text
        );
        compared += 1;
    }
    if let Ok(s) = std::str::from_utf8(text) {
        if start <= s.len() && s.is_char_boundary(start) {
            if let Ok(expected) = re.is_match_at_with(s, start, options) {
                assert_eq!(re.is_match_at(s, start), expected, "str is_match");
                compared += 1;
            }
            if let Ok(expected) = re.find_at_with(s, start, options) {
                assert_eq!(
                    re.find_at(s, start).map(|m| m.range()),
                    expected.map(|m| m.range()),
                    "str find"
                );
                compared += 1;
            }
        }
    }
    compared
}

/// Checks every subject from every start.
///
/// Long subjects (over `LONG_SUBJECT` bytes) are searched from every
/// `LONG_STRIDE`th start, and from the first and the last one, to keep the
/// debug build of the catalogs within a test run.
fn check_regex(re: &Regex, subjects: &[(Vec<u8>, usize)], bytes_offsets: bool) -> usize {
    let mut compared = 0;
    for (text, _) in subjects {
        let stride = if text.len() > LONG_SUBJECT {
            LONG_STRIDE
        } else {
            1
        };
        for start in (0..=text.len())
            .filter(|start| start % stride == 0 || *start == text.len() || stride == 1)
        {
            let boundary = text.get(start).is_none_or(|&b| (b as i8) >= -0x40);
            if bytes_offsets || boundary {
                compared += check_search(re, text, start);
            }
        }
    }
    compared
}

const LONG_SUBJECT: usize = 32;
const LONG_STRIDE: usize = 13;

#[test]
fn hand_written_patterns_agree_with_the_matcher() {
    let mut rng = Lcg(0x5eed_0001);
    let mut subjects: Vec<(Vec<u8>, usize)> =
        (0..200).map(|_| (random_subject(&mut rng), 0)).collect();
    subjects.extend((0..60).map(|_| (random_bytes(&mut rng), 0)));
    subjects.extend(
        [
            "",
            "abc",
            "ABC",
            "a\nb",
            "Straße STRASSE",
            "é€😀",
            "foo bar",
            "x\nxa",
            "\n",
            "a\n",
            "\n\n",
            "\na",
            "ab\n",
        ]
        .iter()
        .chain(HOST_SUBJECTS)
        .map(|s| (s.as_bytes().to_vec(), 0)),
    );
    let mut compiled = 0;
    let mut compared = 0;
    for case in hand_written() {
        for dfa in [true, false] {
            let Some(re) = case.compile(dfa) else {
                continue;
            };
            if dfa {
                compiled += 1;
            }
            compared += check_regex(&re, &subjects, true);
        }
    }
    assert!(
        compiled >= 50,
        "{compiled} of the hand-written patterns compile"
    );
    assert!(compared > 100_000, "{compared} comparisons");
}

#[test]
fn catalog_patterns_agree_with_the_matcher() {
    let (cases, inputs) = catalog_cases();
    let mut rng = Lcg(0x5eed_0002);
    let random: Vec<(Vec<u8>, usize)> = (0..40).map(|_| (random_subject(&mut rng), 0)).collect();
    let mut subjects = inputs;
    subjects.extend(random);
    subjects.extend(HOST_SUBJECTS.iter().map(|s| (s.as_bytes().to_vec(), 0)));
    let mut compared = 0;
    let mut compiled = 0;
    for case in &cases {
        for dfa in [true, false] {
            let Some(re) = case.compile(dfa) else {
                continue;
            };
            if dfa {
                compiled += 1;
            }
            compared += check_regex(&re, &subjects, false);
        }
    }
    assert!(compiled >= 27, "{compiled} catalog patterns compile");
    assert!(compared > 10_000, "{compared} comparisons");
}

/// Patterns whose repeats the compiler makes possessive (`tune_next`: a
/// greedy repeat before an item that no body iteration can start), next to
/// user-written atomic groups and possessive quantifiers, which keep their
/// seeks inexact. The first group's seeks are exact (asserted in the unit
/// tests of `api.rs`), so their answers come from the seek.
const AUTO_POSSESSIVE: &[&str] = &[
    "^[a-z]+@[a-z]+\\.com$",
    "[a-z]*@",
    "^[0-9]*\\.[0-9]+$",
    "\\d+\\.\\d+",
    "\\w+:\\s",
    "([a-z]+)@([a-z]+)\\.(com|org)",
    "(?:[a-z]+@)+x",
    "[^\"]*\"",
    "(?:ab)*c",
    "[^a]+a",
    "é+x",
    "(?i)[a-z]+1",
    "(?i)k+s",
    "x+y*z",
    "^[a-z]+@(?>[a-z]+)\\.com$",
    "(?>[a-z]+)@",
    "[a-z]++@",
    "a*+b",
    "(?>a+)a",
    "[a-z]*+[a-z]",
    "^[a-z]+@[a-z]+\\.com(?=\\s|$)",
];

/// Subjects that match some of the patterns above and miss others, at the
/// ends and in the middle of the subject.
const AUTO_POSSESSIVE_SUBJECTS: &[&str] = &[
    "john@doe.com",
    "john@doe.org",
    "john@doe.co",
    "JOHN@DOE.COM",
    "@doe.com",
    "john@doe.com.",
    "john@doe.com x",
    "john@doe.com\n",
    "a@b.com",
    "12.5",
    "12.x",
    ".5",
    "1.2.3",
    "1.25",
    "ab1",
    "a1b",
    "aab",
    "abab c",
    "ccab",
    "\"x\"",
    "\"x\"\"",
    "\"\"",
    "é+x",
    "ééx",
    "éx",
    "ax",
    "a1",
    "1a",
    "k1",
    "K1",
    "\u{212A}1",
    "ks",
    "KSs",
    "x+y*z",
    "xyz",
    "a:",
    "a: ",
    "é€:\n",
    "a bc",
    "a+b",
    "ab",
    "aa",
    "bbb",
    "ba",
];

/// The patterns of `AUTO_POSSESSIVE` agree with the matcher on the listed
/// subjects and on random ones, from every start, built with the seek and
/// without it.
#[test]
fn auto_possessive_repeats_agree_with_the_matcher() {
    let mut rng = Lcg(0x5eed_0003);
    let mut subjects: Vec<(Vec<u8>, usize)> = AUTO_POSSESSIVE_SUBJECTS
        .iter()
        .map(|s| (s.as_bytes().to_vec(), 0))
        .collect();
    subjects.extend((0..150).map(|_| (random_subject(&mut rng), 0)));
    let mut compared = 0;
    for pattern in AUTO_POSSESSIVE {
        for ignore_case in [false, true] {
            // The `(?i)` patterns are checked with and without the flag too.
            let case = Case::new(pattern).flags(ignore_case, false, None);
            for dfa in [true, false] {
                let re = case
                    .compile(dfa)
                    .unwrap_or_else(|| panic!("{pattern} compiles"));
                compared += check_regex(&re, &subjects, true);
            }
        }
    }
    assert!(compared > 50_000, "{compared} comparisons");
}

/// `(x+x+)+y|.*` on thirty `x`s: the `.*` alternative matches everywhere, so
/// the pattern has no seek and both builds answer with the matcher, which
/// stops at the retry limit on this subject. Both builds agree with each other
/// on the standard methods, errors included.
#[test]
fn unbounded_retries_are_left_to_the_matcher() {
    let text = "x".repeat(30);
    let text = text.as_bytes();
    let on = Regex::new("(x+x+)+y|.*").unwrap();
    let off = RegexBuilder::new("(x+x+)+y|.*").dfa(false).build().unwrap();
    assert_eq!(on.is_match_bytes(text), off.is_match_bytes(text));
    assert_eq!(
        on.find_bytes(text).map(|m| m.range()),
        off.find_bytes(text).map(|m| m.range())
    );
    let _ = check_search(&on, text, 0);
}

/// An exact seek answers "match" where the retry limit stops the matcher: the
/// answer is the one the matcher gives without the limit (ADR-017, rule 2),
/// while the matcher alone, under the limit, reports no match.
#[test]
fn exact_seek_answers_match_where_the_retry_limit_stops_the_matcher() {
    let pattern = r"^[a-z@.]+@[a-z.]+\.com$";
    let on = Regex::new(pattern).unwrap();
    let off = RegexBuilder::new(pattern).dfa(false).build().unwrap();
    let text = b"john@doe.com";
    let tight = SearchOptions::new().retry_limit_in_match(1);
    let unlimited = SearchOptions::new().retry_limit_in_match(0);
    assert!(off.is_match_bytes_at_with(text, 0, tight).is_err());
    assert_eq!(off.is_match_bytes_at_with(text, 0, unlimited), Ok(true));
    assert!(on.is_match_bytes(text));
    assert!(on.is_match("john@doe.com"));
    assert!(!on.is_match_bytes(b"john@doe.org"));
    // The `*_with` methods run the matcher under the caller's limit.
    assert!(on.is_match_bytes_at_with(text, 0, tight).is_err());
}

/// The standard `is_match` of the catalog patterns against C Oniguruma, on
/// the catalog inputs and the host subjects: equal wherever C completes.
/// Where C stops at a limit, ADR-017 lets the seeks answer the result
/// without it, which the comparisons above check against Ferroni's matcher.
#[cfg(feature = "ffi")]
#[test]
fn catalog_is_match_agrees_with_c() {
    use ferroni::ffi::CRegex;
    use ferroni::oniguruma::*;

    let (cases, inputs) = catalog_cases();
    let mut subjects = inputs;
    subjects.extend(HOST_SUBJECTS.iter().map(|s| (s.as_bytes().to_vec(), 0)));
    let mut compared = 0;
    for case in &cases {
        let mut options = ONIG_OPTION_NONE;
        for (enabled, flag) in [
            (case.ignore_case, ONIG_OPTION_IGNORECASE),
            (case.multiline == Some(true), ONIG_OPTION_NEGATE_SINGLELINE),
            (case.multiline == Some(false), ONIG_OPTION_SINGLELINE),
            (case.dotall, ONIG_OPTION_MULTILINE),
            (case.extended, ONIG_OPTION_EXTEND),
        ] {
            if enabled {
                options |= flag;
            }
        }
        let (Ok(c), Some(re)) = (
            CRegex::new(&case.pattern, options.bits()),
            case.compile(true),
        ) else {
            continue;
        };
        for (text, start) in &subjects {
            if *start > text.len() {
                continue;
            }
            let result = c.search(text, *start, text.len(), None, ONIG_OPTION_NONE.bits());
            if result >= 0 || result == ONIG_MISMATCH {
                assert_eq!(
                    re.is_match_bytes_at(text, *start),
                    result >= 0,
                    "{:?} on {:?} from {start}",
                    String::from_utf8_lossy(&case.pattern),
                    String::from_utf8_lossy(text)
                );
                compared += 1;
            }
        }
    }
    assert!(compared > 10_000, "{compared} comparisons");
}
