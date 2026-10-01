//! The captured Shiki scanner calls through engines without a RegSet. Each
//! call searches every pattern of its scanner from the start position and
//! keeps the earliest match, stopping at one that starts right there: the
//! loop vscode-oniguruma runs for lines of 1000 bytes or more.

use crate::engines::{Compiled, Engine};
use crate::scanner_replay::{Call, Corpus};
use std::hint::black_box;

pub struct EngineReplay {
    /// Calls whose only difference from Shiki is an empty capture group
    /// reported as unset or the other way round. vscode-textmate skips
    /// zero-length captures, so both highlight alike.
    pub empty_capture_differences: usize,
    scanners: Vec<Vec<Compiled>>,
    subjects: Vec<String>,
    /// (scanner, subject, byte start)
    calls: Vec<(usize, usize, usize)>,
}

impl EngineReplay {
    /// Compiles the scanners the calls use and checks every call against the
    /// captured Shiki result; the error names the first pattern or call that
    /// differs.
    pub fn new(engine: Engine, corpus: &Corpus, calls: &[&Call]) -> Result<Self, String> {
        let mut scanners: Vec<Vec<Compiled>> = Vec::new();
        for (id, patterns) in corpus.patterns.iter().enumerate() {
            let used = calls.iter().any(|call| call.scanner == id);
            let compiled = if used {
                patterns
                    .iter()
                    .map(|pattern| {
                        engine
                            .compile(pattern, false, true)
                            .map_err(|error| format!("scanner {id}, pattern {pattern:?}: {error}"))
                    })
                    .collect::<Result<_, _>>()?
            } else {
                Vec::new()
            };
            scanners.push(compiled);
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
            let actual = replay
                .find(scanner, text.as_bytes(), start)?
                .map(|(index, captures)| {
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
        text: &[u8],
        start: usize,
    ) -> Result<Option<(usize, crate::engines::Captures)>, String> {
        let mut best: Option<(usize, crate::engines::Captures)> = None;
        for (index, regex) in self.scanners[scanner].iter().enumerate() {
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

    pub fn replay(&self) {
        for &(scanner, subject, start) in &self.calls {
            let text = self.subjects[subject].as_bytes();
            black_box(
                self.find(scanner, black_box(text), black_box(start))
                    .unwrap(),
            );
        }
    }
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
