//! The literal seek of lazy dot loops (`PushLazyAnyChar`, ADR-008) gives C
//! Oniguruma's results and captures: literal groups read by back references,
//! negative one-character look-behinds between the loop and the literal,
//! multibyte and malformed subjects, and patterns that must not seek.

#![cfg(feature = "ffi")]

use ferroni::api::Regex;
use ferroni::ffi::{CRegex, CRegion};
use ferroni::oniguruma::ONIG_OPTION_NONE;
use ferroni::regexec::onig_match;

type Trace = Option<Vec<Option<(usize, usize)>>>;

const PATTERNS: &[&str] = &[
    r"(\_)\1(?!\1)(.+?)(?<!\w)\1(?!\1)(.+?)\1{3}(?!\w)",
    r"(?<!\w)(_)(?!_)(.+?)(?<!_)\1(?!\w)",
    r"(?<!\w)(_)(.+?)(?<!\w)\1",
    r"(a)(.*?)(?<!\w)a",
    r"(a)(.*?)(?<!b)\1",
    r"(a)(.*?)x(?<!\w)\1",
    r"(a)(.*?)\1",
    r"(?m)(.*?)(?<!\w)\n",
    r"(\n)(.*?)(?<!\w)\1",
    r"(é)(.*?)(?<!\w)\1",
    r"(é)(.*?)(?<!\w)é(?<!\W)",
    r"()(.*?)(?<!\w)\1",
    r"(?i)(a)(.*?)(?<!\w)\1",
    r"(a)(.*?)(?<!\w)(?i)\1",
    r"(|a)(.*?)(?<!\w)\1",
    r"(?:(a)|b)(.*?)(?<!\w)\1",
    r"(a)?(.*?)(?<!\w)\1",
    r"(.*?)(?<!\w)\1",
    r"(a)(.*?)(?<=\w)\1",
    r"(a)(.*?)(?<!\w)(b)\3",
    r"(a)(.*?)(?<!\w)(?:a|b)",
    r"(_)(.*?)_(?<!\w)\1",
];

const EDGES: &[&[u8]] = &[
    b"",
    b"_",
    b"__",
    b"_a_",
    b"a_b_c_",
    b"a\n_",
    "é_é_".as_bytes(),
    "_é_".as_bytes(),
    b"x_\n_x",
    b"\xff_",
    b"_\xc3",
    b"____",
    b"\n",
    b"_\n",
    b"a\n\na_a_",
    "aé_".as_bytes(),
    b"_\xe2\x82",
    b"a_\xff_a",
    b"aaaa",
    b"aXa",
    b"abab\n",
    b"a\nb\na",
    b"aa b a\xc3",
    b"a(b)a_ab",
];

const PIECES: &[&[u8]] = &[
    b"_",
    b"a",
    b"b",
    b"x",
    b"\n",
    b" ",
    "é".as_bytes(),
    b"\xff",
    b"\xc3",
    b"\xe2\x82",
    b"1",
];

/// Ferroni's search result and captures, as `CRegex::search` reports them.
fn ferroni_trace(re: &Regex, text: &[u8]) -> Trace {
    re.captures_bytes(text).map(|captures| {
        (0..captures.len())
            .map(|i| captures.get(i).map(|m| (m.start(), m.end())))
            .collect()
    })
}

fn c_trace(re: &CRegex, text: &[u8]) -> Trace {
    let mut region = CRegion::new();
    let result = re.search(text, 0, text.len(), Some(&mut region), 0);
    assert!(result >= -1, "C error {result} on {text:?}");
    (result >= 0).then(|| {
        region
            .capture_ranges()
            .into_iter()
            .map(|(begin, end)| (begin >= 0).then_some((begin as usize, end as usize)))
            .collect()
    })
}

/// Whether C's `\w` agrees with Ferroni's at every byte of `text`.
/// Ferroni's Unicode data is newer than C's (ADR-015): a malformed sequence
/// such as `e2 82 5f` decodes to U+209F, a word character only since
/// Unicode 17.
fn word_tables_agree(text: &[u8]) -> bool {
    let ours = Regex::new(r"\w").unwrap();
    let c = CRegex::new(br"\w", 0).unwrap();
    (0..text.len()).all(|at| {
        let (result, _) = onig_match(ours.as_raw(), text, text.len(), at, None, ONIG_OPTION_NONE);
        (result >= 0) == (c.match_at(text, at, None, 0) >= 0)
    })
}

#[test]
fn lazy_literal_seek_agrees_with_c() {
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let mut subjects: Vec<Vec<u8>> = EDGES.iter().map(|edge| edge.to_vec()).collect();
    for _ in 0..200 {
        subjects.push(
            (0..next(20))
                .flat_map(|_| PIECES[next(PIECES.len())].to_vec())
                .collect(),
        );
    }
    let subjects: Vec<Vec<u8>> = subjects
        .into_iter()
        .filter(|subject| word_tables_agree(subject))
        .collect();
    let mut comparisons = 0;
    for pattern in PATTERNS {
        let ours = Regex::new(pattern).unwrap();
        let c = CRegex::new(pattern.as_bytes(), 0).unwrap();
        for subject in &subjects {
            assert_eq!(
                ferroni_trace(&ours, subject),
                c_trace(&c, subject),
                "{pattern} on {subject:?}"
            );
            comparisons += 1;
        }
    }
    assert!(comparisons > 1_000, "{comparisons}");
}
