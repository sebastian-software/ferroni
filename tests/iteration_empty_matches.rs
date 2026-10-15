//! The match iterators against the C search loop, over empty matches.
//!
//! C has no iterator. Its `onig_search` is driven here with the empty-match
//! rule of `Regex::captures_iter`: each search starts at the end of the
//! previous match, and an empty match right after an empty match is skipped
//! by a search one character on. The iterators must yield the same matches
//! and captures.

#![cfg(feature = "ffi")]

use ferroni::api::Regex;
use ferroni::ffi::{CRegex, CRegion};

type Groups = Vec<Option<(usize, usize)>>;

fn c_captures_iteration(pattern: &str, text: &str) -> Vec<Groups> {
    let c = CRegex::new(pattern.as_bytes(), 0).unwrap();
    let mut region = CRegion::new();
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut last_end = 0;
    let mut last_was_empty = false;
    while last_end <= bytes.len() {
        let result = c.search(bytes, last_end, bytes.len(), Some(&mut region), 0);
        if result == -1 {
            break;
        }
        assert!(result >= 0, "C error {result}: {pattern:?} on {text:?}");
        let groups: Groups = region
            .capture_ranges()
            .into_iter()
            .map(|(start, end)| (start >= 0).then_some((start as usize, end as usize)))
            .collect();
        let (start, end) = groups[0].expect("whole match");
        if start == end {
            if last_was_empty {
                if last_end >= bytes.len() {
                    break;
                }
                last_end += text[last_end..].chars().next().unwrap().len_utf8();
                last_was_empty = false;
                continue;
            }
            last_was_empty = true;
        } else {
            last_was_empty = false;
        }
        last_end = end;
        found.push(groups);
    }
    found
}

fn ferroni_captures_iteration(regex: &Regex, text: &str) -> Vec<Groups> {
    regex
        .captures_iter(text)
        .map(|caps| {
            caps.iter()
                .map(|group| group.map(|m| (m.start(), m.end())))
                .collect()
        })
        .collect()
}

/// Every string of up to `max_len` characters over `alphabet`.
fn strings_up_to(alphabet: &[char], max_len: usize) -> Vec<String> {
    let mut all = vec![String::new()];
    let mut current = vec![String::new()];
    for _ in 0..max_len {
        let mut next = Vec::new();
        for prefix in &current {
            for &c in alphabet {
                let mut s = prefix.clone();
                s.push(c);
                next.push(s);
            }
        }
        all.extend(next.iter().cloned());
        current = next;
    }
    all
}

#[test]
fn captures_iter_matches_the_c_search_loop_on_empty_matches() {
    let patterns = [
        "a*",
        "x*",
        r"\b",
        r"\B\w*",
        "(?=x)",
        "|",
        "(a|)",
        "(a)?(b*)",
        r"(\d*)(\D?)",
        r"(?<=a)|b*",
        "(a)|(b*)",
        "#(\\w+)",
        r"(\w)(\d)?",
        "(?=(x*))",
        r"a*?$",
        // \G and \K keep their searches: the same iteration as C's.
        r"(?<=a)(?!\G)|b",
        r"\Ga*",
        r"x*\K",
        r"(a)\K|b*",
        // The first attempt fails and a later match is recorded by a second pass.
        "(a)b|(c)",
        r"(?<=z)(a)|(b)",
        r"\w(\d)|(x)",
        "(a)(b)?c|(d)e",
    ];
    let alphabet = ['a', 'b', 'c', 'x', '1', ' ', 'é'];
    let texts = strings_up_to(&alphabet, 4);
    let mut compared = 0;
    for pattern in patterns {
        let regex = Regex::new(pattern).unwrap();
        for text in &texts {
            let expected = c_captures_iteration(pattern, text);
            let found = ferroni_captures_iteration(&regex, text);
            assert_eq!(found, expected, "{pattern:?} on {text:?}");
            compared += 1;
        }
    }
    assert_eq!(compared, patterns.len() * texts.len());
}
