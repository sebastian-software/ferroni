//! The captured Shiki scanner calls through engines without a RegSet. Each
//! call searches every pattern of its scanner from the start position and
//! keeps the earliest match, stopping at one that starts right there: the
//! loop vscode-oniguruma runs for lines of 1000 bytes or more.
//!
//! `Engine::FancySet` instead builds one fancy-regex `RegexSet` per scanner,
//! once, and asks it for the earliest match of each call, as a highlighter
//! with RegexSet support would.

use crate::engines::{Compiled, Engine};
use crate::scanner_replay::{Call, Corpus};
use std::hint::black_box;

pub struct EngineReplay {
    /// Calls whose only difference from Shiki is an empty capture group
    /// reported as unset or the other way round. vscode-textmate skips
    /// zero-length captures, so both highlight alike.
    pub empty_capture_differences: usize,
    scanners: Vec<Searcher>,
    subjects: Vec<String>,
    /// (scanner, subject, byte start)
    calls: Vec<(usize, usize, usize)>,
}

/// One scanner's patterns as the engine searches them.
enum Searcher {
    /// Searched one pattern at a time, keeping the earliest match.
    Patterns(Vec<Compiled>),
    /// One RegexSet over all patterns, shared by every call of the scanner.
    Set(fancy_regex::RegexSet),
}

impl EngineReplay {
    /// Compiles the scanners the calls use and checks every call against the
    /// captured Shiki result; the error names the first pattern or call that
    /// differs.
    pub fn new(engine: Engine, corpus: &Corpus, calls: &[&Call]) -> Result<Self, String> {
        let mut scanners: Vec<Searcher> = Vec::new();
        for (id, patterns) in corpus.patterns.iter().enumerate() {
            let used = calls.iter().any(|call| call.scanner == id);
            let searcher = if used {
                compile_scanner(engine, id, patterns)?
            } else {
                Searcher::Patterns(Vec::new())
            };
            scanners.push(searcher);
        }
        let mut replay = Self {
            empty_capture_differences: 0,
            scanners,
            subjects: corpus.subjects.clone(),
            calls: calls
                .iter()
                .map(|call| {
                    let text = &corpus.subjects[call.subject];
                    (
                        call.scanner,
                        call.subject,
                        byte_offset(text, call.start_utf16),
                    )
                })
                .collect(),
        };
        for (i, (&(scanner, subject, start), call)) in replay.calls.iter().zip(calls).enumerate() {
            let text = &replay.subjects[subject];
            let actual = replay.find(scanner, text, start)?.map(|(index, captures)| {
                let utf16 = |byte: i32| text[..byte as usize].encode_utf16().count();
                let captures = captures
                    .into_iter()
                    .map(|(beg, end)| {
                        if beg >= 0 && end >= beg {
                            (utf16(beg), utf16(end))
                        } else {
                            (0, 0)
                        }
                    })
                    .collect::<Vec<_>>();
                (index, captures)
            });
            if actual != call.expected && highlighted(&actual) == highlighted(&call.expected) {
                replay.empty_capture_differences += 1;
            } else if actual != call.expected {
                return Err(format!(
                    "call {i} (scanner {scanner}): {actual:?}, Shiki {:?}",
                    call.expected
                ));
            }
        }
        Ok(replay)
    }

    fn find(
        &self,
        scanner: usize,
        text: &str,
        start: usize,
    ) -> Result<Option<(usize, crate::engines::Captures)>, String> {
        match &self.scanners[scanner] {
            Searcher::Patterns(regexes) => {
                let mut best: Option<(usize, crate::engines::Captures)> = None;
                for (index, regex) in regexes.iter().enumerate() {
                    if let Some(captures) = regex.captures(text, start)? {
                        let at = captures[0].0;
                        if best.as_ref().is_none_or(|(_, prior)| at < prior[0].0) {
                            best = Some((index, captures));
                        }
                        if at == start as i32 {
                            break;
                        }
                    }
                }
                Ok(best)
            }
            Searcher::Set(set) => {
                let input = fancy_regex::RegexInput::new(text).from_pos(start);
                let Some(mut matches) = set.find_input(input).map_err(|error| error.to_string())?
                else {
                    return Ok(None);
                };
                // The matches at the earliest position come in ascending pattern
                // order, so the first one is the earliest match, with the lowest
                // pattern index among those that start there.
                let Some(first) = matches.next() else {
                    return Ok(None);
                };
                let first = first.map_err(|error| error.to_string())?;
                let captures = first.captures();
                let spans = (0..captures.len())
                    .map(|group| {
                        captures
                            .get(group)
                            .map_or((-1, -1), |m| (m.start() as i32, m.end() as i32))
                    })
                    .collect();
                Ok(Some((first.pattern(), spans)))
            }
        }
    }

    pub fn replay(&self) {
        for &(scanner, subject, start) in &self.calls {
            let text = self.subjects[subject].as_str();
            black_box(
                self.find(scanner, black_box(text), black_box(start))
                    .unwrap(),
            );
        }
    }
}

/// Compiles one scanner's patterns for `engine`. The RegexSet takes the
/// fancy-regex engine's options (Oniguruma mode, multi-line anchors) and no
/// seek pre-filter: a RegexSet verifies its members anchored at candidate
/// positions, where seek does nothing, and its earliest-match search always
/// uses the seek approximations of the patterns.
fn compile_scanner(engine: Engine, id: usize, patterns: &[String]) -> Result<Searcher, String> {
    if engine != Engine::FancySet {
        return patterns
            .iter()
            .map(|pattern| {
                engine
                    .compile(pattern, false, true)
                    .map_err(|error| format!("scanner {id}, pattern {pattern:?}: {error}"))
            })
            .collect::<Result<_, _>>()
            .map(Searcher::Patterns);
    }
    let mut options = fancy_regex::RegexOptionsBuilder::new();
    options
        .oniguruma_mode(true)
        .multi_line(true)
        .ignore_numbered_groups_when_named_groups_exist(false);
    fancy_regex::RegexSet::new_with_options(patterns, &options)
        .map(Searcher::Set)
        .map_err(|error| {
            // Name the pattern that the per-pattern engine rejects, if one does.
            patterns
                .iter()
                .find_map(|pattern| {
                    Engine::Fancy
                        .compile(pattern, false, true)
                        .err()
                        .map(|reason| format!("scanner {id}, pattern {pattern:?}: {reason}"))
                })
                .unwrap_or_else(|| format!("scanner {id}: {error}"))
        })
}

fn byte_offset(text: &str, start_utf16: usize) -> usize {
    let mut utf16 = 0;
    text.char_indices()
        .find_map(|(byte, ch)| {
            let at = utf16;
            utf16 += ch.len_utf16();
            (start_utf16 >= at && start_utf16 < utf16).then_some(byte)
        })
        .unwrap_or(text.len())
}

/// A scanner result as vscode-textmate uses it: empty groups count as unset.
/// The whole match (group 0) stays exact.
fn highlighted(result: &crate::scanner_replay::Match) -> crate::scanner_replay::Match {
    result.as_ref().map(|(index, captures)| {
        let captures = captures
            .iter()
            .enumerate()
            .map(|(group, &(beg, end))| {
                if group > 0 && beg == end {
                    (0, 0)
                } else {
                    (beg, end)
                }
            })
            .collect();
        (*index, captures)
    })
}
