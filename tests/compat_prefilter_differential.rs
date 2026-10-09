//! The scanner with its DFA pre-filter (ADR-008) against the scanner
//! without it, over every pattern/text pair of the compat files that the
//! scanner accepts, at every character boundary of the text.
//!
//! The pairs are read from the sources of tests/compat_utf8.rs,
//! compat_back.rs, compat_syntax.rs and compat_options.rs: the arguments of
//! their `x2`, `x3`, `n` and `e` helpers (and the `_syn` variants, compiled
//! in the default syntax here). The options of compat_options.rs are the
//! compile options of both scanners; pairs with search-time options the
//! scanner has no equivalent for are left out.

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

/// Every pattern/text pair of the compat files the scanner accepts answers
/// the same with the pre-filter as without, from every character boundary.
#[test]
fn compat_pairs_answer_alike_with_and_without_the_prefilter() {
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
            let config = ScannerConfig::default().options(case.options);
            let plain = Scanner::with_config(&[pattern], &config.clone().prefilter(false));
            let filtered = Scanner::with_config(&[pattern], &config);
            let (Ok(mut plain), Ok(mut filtered)) = (plain, filtered) else {
                continue;
            };
            accepted += 1;
            covered += usize::from(filtered.prefilter_stats().covered == 1);
            let starts = (0..=text.len()).filter(|&at| text.is_char_boundary(at));
            for start in starts {
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
    println!(
        "{pairs} pairs, {accepted} accepted by the scanner, {covered} covered by the pre-filter, {calls} calls compared"
    );
    assert!(
        pairs >= 2_000,
        "the compat files hold more pairs than {pairs}"
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
