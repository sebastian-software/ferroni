//! The scanner with its DFA pre-filter (ADR-008) against the scanner
//! without it, over every pattern/text pair of the compat files that the
//! scanner accepts, at every character boundary of the text.
//!
//! The pairs are read from the sources of tests/compat_utf8.rs,
//! compat_back.rs, compat_syntax.rs and compat_options.rs: the arguments of
//! their `x2`, `x3`, `n` and `e` helpers (and the `_syn` variants, compiled
//! in the default syntax here). The options of compat_options.rs are the
//! compile options of both scanners; pairs with search-time options the
//! scanner has no equivalent for are left out. A second test composes each
//! accepted pattern with a literal before and after it, which puts the
//! pattern's optimizer at a distance from the match start.

use ferroni::oniguruma::*;
use ferroni::scanner::{Scanner, ScannerConfig, ScannerFindOptions};

const SOURCES: [(&str, &str); 4] = [
    ("compat_utf8.rs", include_str!("compat_utf8.rs")),
    ("compat_back.rs", include_str!("compat_back.rs")),
    ("compat_syntax.rs", include_str!("compat_syntax.rs")),
    ("compat_options.rs", include_str!("compat_options.rs")),
];

const HELPERS: [&str; 8] = ["x2_syn", "x3_syn", "n_syn", "e_syn", "x2", "x3", "n", "e"];

struct Case {
    file: &'static str,
    line: usize,
    options: OnigOptionType,
    pattern: Vec<u8>,
    text: Vec<u8>,
}

/// A compile option the scanner takes, by the name the compat files use.
fn option(name: &str) -> Option<OnigOptionType> {
    Some(match name {
        "ONIG_OPTION_NONE" => ONIG_OPTION_NONE,
        "ONIG_OPTION_IGNORECASE" => ONIG_OPTION_IGNORECASE,
        "ONIG_OPTION_IGNORECASE_IS_ASCII" => ONIG_OPTION_IGNORECASE_IS_ASCII,
        "ONIG_OPTION_WORD_IS_ASCII" => ONIG_OPTION_WORD_IS_ASCII,
        "ONIG_OPTION_DIGIT_IS_ASCII" => ONIG_OPTION_DIGIT_IS_ASCII,
        "ONIG_OPTION_SPACE_IS_ASCII" => ONIG_OPTION_SPACE_IS_ASCII,
        "ONIG_OPTION_POSIX_IS_ASCII" => ONIG_OPTION_POSIX_IS_ASCII,
        "ONIG_OPTION_EXTEND" => ONIG_OPTION_EXTEND,
        "ONIG_OPTION_FIND_LONGEST" => ONIG_OPTION_FIND_LONGEST,
        "ONIG_OPTION_FIND_NOT_EMPTY" => ONIG_OPTION_FIND_NOT_EMPTY,
        "ONIG_OPTION_MATCH_WHOLE_STRING" => ONIG_OPTION_MATCH_WHOLE_STRING,
        _ => return None,
    })
}

/// The compile options of an argument expression such as
/// `ONIG_OPTION_IGNORECASE | ONIG_OPTION_WORD_IS_ASCII`, or `None` for one
/// the scanner cannot take.
fn options(expression: &str) -> Option<OnigOptionType> {
    let mut options = ONIG_OPTION_NONE;
    for name in expression.split('|') {
        options |= option(name.trim())?;
    }
    Some(options)
}

/// The bytes of the Rust string literal at `source[at..]` (`"…"`, `b"…"`,
/// `r"…"`, `br"…"`, `r#"…"#`, …) and the index after it, with a trailing
/// `.as_bytes()` consumed.
fn literal(source: &str, at: usize) -> Option<(Vec<u8>, usize)> {
    let bytes = source.as_bytes();
    let mut i = at;
    if bytes.get(i) == Some(&b'b') {
        i += 1;
    }
    let raw = bytes.get(i) == Some(&b'r');
    if raw {
        i += 1;
    }
    let mut hashes = 0;
    while bytes.get(i) == Some(&b'#') {
        hashes += 1;
        i += 1;
    }
    if bytes.get(i) != Some(&b'"') {
        return None;
    }
    i += 1;
    let mut out = Vec::new();
    loop {
        let byte = *bytes.get(i)?;
        if byte == b'"' {
            let closing = &bytes[i + 1..];
            if raw && closing.len() >= hashes && closing[..hashes].iter().all(|&b| b == b'#') {
                i += 1 + hashes;
                break;
            }
            if !raw {
                i += 1;
                break;
            }
        }
        if byte == b'\\' && !raw {
            i += 1;
            match *bytes.get(i)? {
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'0' => out.push(0),
                b'\\' => out.push(b'\\'),
                b'"' => out.push(b'"'),
                b'\'' => out.push(b'\''),
                b'x' => {
                    let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                    out.push(u8::from_str_radix(hex, 16).ok()?);
                    i += 2;
                }
                b'u' => {
                    let close = source[i..].find('}')? + i;
                    let code = u32::from_str_radix(&source[i + 2..close], 16).ok()?;
                    let mut buffer = [0u8; 4];
                    out.extend_from_slice(
                        char::from_u32(code)?.encode_utf8(&mut buffer).as_bytes(),
                    );
                    i = close;
                }
                b'\n' => {
                    // A line continuation skips the following whitespace.
                    while bytes.get(i + 1).is_some_and(|b| b.is_ascii_whitespace()) {
                        i += 1;
                    }
                }
                _ => return None,
            }
            i += 1;
            continue;
        }
        out.push(byte);
        i += 1;
    }
    let rest = &source[i..];
    if let Some(after) = rest.strip_prefix(".as_bytes()") {
        i = source.len() - after.len();
    }
    Some((out, i))
}

fn skip_whitespace(source: &str, mut at: usize) -> usize {
    let bytes = source.as_bytes();
    while bytes.get(at).is_some_and(|b| b.is_ascii_whitespace()) {
        at += 1;
    }
    at
}

/// The helper calls of one compat file as cases.
fn cases(file: &'static str, source: &str) -> Vec<Case> {
    let mut found = Vec::new();
    let bytes = source.as_bytes();
    let mut at = 0;
    while at < source.len() {
        let Some(offset) = source[at..].find('(') else {
            break;
        };
        let open = at + offset;
        at = open + 1;
        let name_start = source[..open]
            .char_indices()
            .rev()
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let name = &source[name_start..open];
        if !HELPERS.contains(&name)
            || !bytes
                .get(name_start.wrapping_sub(1))
                .is_none_or(|b| b.is_ascii_whitespace())
        {
            continue;
        }
        let line = source[..open].matches('\n').count() + 1;
        let mut i = skip_whitespace(source, open + 1);
        let mut compile_options = ONIG_OPTION_NONE;
        // A leading identifier argument: the options or the syntax.
        if bytes[i].is_ascii_alphabetic() && literal(source, i).is_none() {
            let end = source[i..].find(',').map_or(source.len(), |c| i + c);
            let expression = source[i..end].trim();
            if expression.starts_with("ONIG_OPTION_") {
                match options(expression) {
                    Some(options) => compile_options = options,
                    None => continue,
                }
            } else if expression != "syn" && expression != "syntax" {
                continue;
            }
            i = skip_whitespace(source, end + 1);
        }
        let Some((pattern, after_pattern)) = literal(source, i) else {
            continue;
        };
        let i = skip_whitespace(source, after_pattern);
        if bytes.get(i) != Some(&b',') {
            continue;
        }
        let i = skip_whitespace(source, i + 1);
        let Some((text, _)) = literal(source, i) else {
            continue;
        };
        found.push(Case {
            file,
            line,
            options: compile_options,
            pattern,
            text,
        });
    }
    found
}

fn bounds(found: Option<ferroni::scanner::ScannerMatch>) -> Option<(usize, Vec<(usize, usize)>)> {
    found.map(|m| {
        (
            m.index,
            m.captures().iter().map(|c| (c.start, c.end)).collect(),
        )
    })
}

/// Compares the scanner with and without the pre-filter over the cases,
/// each pattern composed by `compose`, from every byte offset of the text.
/// Returns (pairs, accepted, covered, calls) and the mismatches.
fn differences(compose: impl Fn(&str) -> String) -> ((usize, usize, usize, u64), Vec<String>) {
    let mut pairs = 0;
    let mut accepted = 0;
    let mut covered = 0;
    let mut calls = 0u64;
    let mut mismatches = Vec::new();
    for (file, source) in SOURCES {
        for case in cases(file, source) {
            pairs += 1;
            let (Ok(pattern), Ok(text)) = (
                std::str::from_utf8(&case.pattern),
                std::str::from_utf8(&case.text),
            ) else {
                continue;
            };
            let pattern = compose(pattern);
            let config = ScannerConfig::default()
                .options(case.options)
                .prefilter_warmup(0);
            let plain = Scanner::with_config(&[&pattern], &config.clone().prefilter(false));
            let filtered = Scanner::with_config(&[&pattern], &config);
            let (Ok(mut plain), Ok(mut filtered)) = (plain, filtered) else {
                continue;
            };
            accepted += 1;
            covered += usize::from(filtered.prefilter_stats().covered == 1);
            // Every byte offset: a start inside a character takes the
            // position-lead path, and has to answer the same.
            for start in 0..=text.len() {
                calls += 1;
                let want = bounds(plain.find_next_match(text, start, ScannerFindOptions::NONE));
                let got = bounds(filtered.find_next_match(text, start, ScannerFindOptions::NONE));
                if got != want {
                    mismatches.push(format!(
                        "{}:{} {pattern:?} on {text:?} from {start}: with {got:?}, without {want:?}",
                        case.file, case.line
                    ));
                }
            }
        }
    }
    ((pairs, accepted, covered, calls), mismatches)
}

/// Every pattern/text pair of the compat files the scanner accepts answers
/// the same with the pre-filter as without, from every byte offset.
#[test]
fn compat_pairs_answer_alike_with_and_without_the_prefilter() {
    let ((pairs, accepted, covered, calls), mismatches) = differences(str::to_owned);
    println!(
        "{pairs} pairs, {accepted} accepted by the scanner, {covered} covered by the pre-filter, {calls} calls compared"
    );
    assert!(
        pairs >= 2_000,
        "the compat files hold more pairs than {pairs}"
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// The compile options of a case as the inline option group the fuzz
/// target's default compile can take (`(?iW)`), or `None` for an option
/// without an inline form.
fn inline_options(options: OnigOptionType) -> Option<String> {
    let mut letters = String::new();
    let mut rest = options;
    for (option, letter) in [
        (ONIG_OPTION_IGNORECASE, 'i'),
        (ONIG_OPTION_EXTEND, 'x'),
        (ONIG_OPTION_WORD_IS_ASCII, 'W'),
        (ONIG_OPTION_DIGIT_IS_ASCII, 'D'),
        (ONIG_OPTION_SPACE_IS_ASCII, 'S'),
        (ONIG_OPTION_POSIX_IS_ASCII, 'P'),
    ] {
        if rest.contains(option) {
            letters.push(letter);
            rest.remove(option);
        }
    }
    if rest != ONIG_OPTION_NONE {
        return None;
    }
    Some(if letters.is_empty() {
        String::new()
    } else {
        format!("(?{letters})")
    })
}

/// The pattern sets and texts of the review findings of PR #321 (the
/// regressions of tests/scanner_prefilter_regressions.rs and the shapes of
/// the tokenizing-loop test), as seeds. The retry-limit regression goes in
/// over a run short enough to stay under the limit: a search that reaches
/// it takes seconds in the instrumented fuzz build, and the target ends
/// such a case after one (see `SLOW_SEARCH` there).
const REVIEW_SEEDS: &[(&[&str], &str)] = &[
    (&[r"\W", "a"], "😀a"),
    (&[r"\W", "x"], "éx"),
    (&[r"(?i)(?:kelvin|street|fiat|xyz)!", "!"], "Kelvin!"),
    (&[r"(?i)(?:kelvin|street|fiat|xyz)$", "!"], "Kelvin"),
    (&[r"(?i)(?:kelvin|street|fiat|xyz)\b", "!"], "Kelvin ſtreet"),
    (&[r"(?(a)b|c)", "b"], "ab"),
    (&[r"((?(a)b|c))(\1)"], "abab"),
    (&[r"(?(a)(?:b|c))!", "!"], "ac!"),
    (&[r"(a+)+b", "c"], "aaaaaaaaaaaac b"),
    (&[r"x(?<n>a){0}(?=\g<n>\g<n>)[ab]+[cd]", "c"], "xaaa! c"),
    (&["."], "é"),
    (&[r"[^[^[^İ]\S]]", "a"], " a"),
    (
        &[r"(?<![-\w])(?:kelvin|street)(?![.:\w])", "k"],
        "street kelvin",
    ),
    (&[r"(?<=\.)\w+", "z"], "aaaaaaaa"),
    (&["a", r"(?<=z)"], "aaaaaaaa"),
    (&["a", r".*(?<=z)"], "aaaaaaaa"),
    (&[r"\G ?", "a"], "aaaa"),
    (&[r"(?<=b)a?", r"a[ab]{2}a"], "babababa"),
    (&[r"(?<=z)a[ab]*a", "a"], "babababa"),
    (&[r"(?<=\.)\w+", r"\w+:", r"\d+", "a"], "aaaaaaaa"),
    (&[r"(?<=/)[^/]+", r"[^/]+/", "a"], "aaaaaaaa"),
];

/// Writes the compat pairs and the review regressions as seeds for the
/// `prefilter-differential` fuzz target (fuzz/README.md) into the directory
/// `FERRONI_FUZZ_SEED_DIR` names, in the target's raw layout: a non-zero
/// first byte, the patterns separated by NULs, a NUL, then the text. A case
/// with compile options goes in with their inline form in front of the
/// pattern; one whose options have none, or that the scanner rejects, is
/// left out.
#[test]
#[ignore = "writes the fuzz seed corpus to FERRONI_FUZZ_SEED_DIR"]
fn write_fuzz_seeds() {
    let Some(dir) = std::env::var_os("FERRONI_FUZZ_SEED_DIR") else {
        println!("FERRONI_FUZZ_SEED_DIR is not set; nothing written");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut written = 0;
    let mut write = |name: &str, patterns: &[&str], text: &[u8]| {
        if patterns
            .iter()
            .any(|pattern| Scanner::new(&[pattern]).is_err())
        {
            return;
        }
        let mut seed = vec![1u8];
        for pattern in patterns {
            seed.extend_from_slice(pattern.as_bytes());
            seed.push(0);
        }
        seed.extend_from_slice(text);
        std::fs::write(dir.join(name), seed).unwrap();
        written += 1;
    };
    for (file, source) in SOURCES {
        let stem = file.strip_suffix(".rs").unwrap_or(file);
        for case in cases(file, source) {
            let (Ok(pattern), Some(prefix)) = (
                std::str::from_utf8(&case.pattern),
                inline_options(case.options),
            ) else {
                continue;
            };
            if pattern.contains('\0') || case.text.contains(&0) {
                continue;
            }
            let pattern = format!("{prefix}{pattern}");
            write(
                &format!("compat-{stem}-{}", case.line),
                &[&pattern],
                &case.text,
            );
        }
    }
    for (n, (patterns, text)) in REVIEW_SEEDS.iter().enumerate() {
        write(&format!("review-{n}"), patterns, text.as_bytes());
    }
    println!("{written} seeds written to {}", dir.display());
    assert!(written > 2_000, "only {written} seeds");
}

/// The same with a literal appended to and prepended to each pattern,
/// which puts C's optimizer string at a distance from the match start
/// (`(?(a)(?:b|c))!` on `ac!` is attempted at 2 only).
#[test]
fn composed_compat_patterns_answer_alike_with_and_without_the_prefilter() {
    let appended = |pattern: &str| format!("{pattern}!");
    let prepended = |pattern: &str| format!("!{pattern}");
    for (name, (pairs, accepted, covered, calls), mismatches) in [
        ("appended", differences(appended)),
        ("prepended", differences(prepended)),
    ]
    .into_iter()
    .map(|(name, (counts, mismatches))| (name, counts, mismatches))
    {
        println!(
            "{name}: {pairs} pairs, {accepted} accepted by the scanner, {covered} covered by the pre-filter, {calls} calls compared"
        );
        assert!(mismatches.is_empty(), "{name}: {}", mismatches.join("\n"));
    }
}
