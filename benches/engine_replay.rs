//! The captured scanner calls through the engines of `engines.rs`, and the
//! benchmark group every scanner replay bench registers with
//! [`bench_replays`].
//!
//! Engines without a RegSet search every pattern of the call's scanner from
//! the start position and keep the earliest match, stopping at one that starts
//! right there: the loop vscode-oniguruma runs for lines of 1000 bytes or
//! more. Each pattern keeps its match in the engine's own result storage, and
//! only the winner's captures are read, as vscode-oniguruma keeps one region
//! per pattern.
//!
//! `Engine::FancySet` instead builds one fancy-regex `RegexSet` per scanner,
//! once, and asks it for the earliest match of each call, as a highlighter
//! with RegexSet support would.
//!
//! Every comparator call has the boundary of Ferroni's
//! `find_next_match_utf16`: it converts the UTF-16 start through the
//! subject's offset table, searches, and returns the winner as a
//! [`Utf16Match`]. Subjects and their tables are built outside timing.

use crate::engines::{self, Compiled, Engine, Found};
#[cfg(feature = "ffi")]
use crate::scanner_replay::CReplay;
use crate::scanner_replay::{
    Call, Corpus, Subject, Utf16Match, bench_cold_document, expected_form, replay,
};
use criterion::{BatchSize, Criterion, Throughput};
use smallvec::SmallVec;
use std::hint::black_box;

/// Registers the replays of one captured trace as the group
/// `<language>_scanner`: the whole document (`document`) and each hot scanner
/// group (`group_<id>`) through Ferroni, the vscode-oniguruma C scanner
/// (`_c`, with `ffi`) and every engine of `engines.rs` (`_<engine>`). With
/// `cold`, the one-shot `cold_document` lanes follow for the same engines.
///
/// Every replay is checked against the captured results before timing. One
/// that reproduces them prints a `VALIDATED` line; an engine that rejects a
/// pattern or returns a different result prints `UNSUPPORTED`, and one whose
/// only differences are empty groups reported as unset (or the reverse)
/// prints `EQUIVALENT`.
///
/// `SCANNER_REPLAY_ID`, a full benchmark ID such as `cpp_scanner/document_c`,
/// limits setup to that benchmark, for a runner that times one engine per
/// process (`scripts/compare-scanner-replays.py`).
pub fn bench_replays(c: &mut Criterion, language: &str, corpus: &Corpus, cold: bool) {
    let group_name = format!("{language}_scanner");
    let only = std::env::var("SCANNER_REPLAY_ID").ok();
    let wanted = |name: &str| {
        only.as_deref()
            .is_none_or(|id| id == format!("{group_name}/{name}"))
    };
    // Ferroni's full check, with the independent C oracle, unless the selected
    // benchmark is another engine's: that one's own check follows.
    let ferroni = std::iter::once("document".to_owned())
        .chain(corpus.hot_groups.iter().map(|id| format!("group_{id}")))
        .chain(cold.then(|| "cold_document".to_owned()));
    if ferroni.into_iter().any(|name| wanted(&name)) {
        corpus.validate();
    }
    let mut group = c.benchmark_group(&group_name);
    for selected in std::iter::once(None).chain(corpus.hot_groups.iter().copied().map(Some)) {
        let calls = corpus.selected(selected);
        let name = selected.map_or_else(|| "document".to_owned(), |id| format!("group_{id}"));
        group.throughput(Throughput::Elements(calls.len() as u64));
        if wanted(&name) {
            validated(&format!("{group_name}/{name}"), "rust", calls.len());
            let mut scanners = corpus.scanners();
            group.bench_function(&name, |b| {
                b.iter_batched_ref(
                    || corpus.strings(),
                    |strings| replay(&mut scanners, strings, &calls),
                    BatchSize::LargeInput,
                );
            });
        }
        #[cfg(feature = "ffi")]
        if wanted(&format!("{name}_c")) {
            let c_replay = CReplay::new(corpus, &calls);
            let scanners = CReplay::compile(corpus);
            c_replay.validate(&scanners, &calls);
            validated(&format!("{group_name}/{name}_c"), "c", calls.len());
            group.bench_function(format!("{name}_c"), |b| {
                b.iter_batched(
                    || c_replay.fresh_ids(),
                    |ids| c_replay.replay(&scanners, ids),
                    BatchSize::SmallInput,
                );
            });
        }
        for &engine in Engine::ALL {
            let name = format!("{name}_{}", engine.id());
            if !wanted(&name) {
                continue;
            }
            let id = format!("{group_name}/{name}");
            let replay = EngineReplay::new(engine, corpus, &calls);
            match replay.checked(corpus, &calls, false) {
                Ok((searchers, differences)) => {
                    report(&id, engine, calls.len(), differences);
                    group.bench_function(&name, |b| b.iter(|| replay.replay(&searchers)));
                }
                Err(reason) => engines::unsupported(&id, engine, &reason),
            }
        }
    }
    if cold {
        bench_cold(&mut group, &group_name, corpus, &wanted);
    }
    group.finish();
}

/// The one-shot lanes: each iteration builds every scanner of the trace and
/// replays the whole document once. Compilation (with PCRE2's JIT and
/// Ferroni's pre-filter warm-up and build) is timed; subjects, string
/// identities and the destruction of the scanners are not.
fn bench_cold(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    group_name: &str,
    corpus: &Corpus,
    wanted: &dyn Fn(&str) -> bool,
) {
    let calls = corpus.selected(None);
    group.throughput(Throughput::Elements(calls.len() as u64));
    if wanted("cold_document") {
        // `Corpus::validate` checked freshly built scanners on every call.
        validated(&format!("{group_name}/cold_document"), "rust", calls.len());
        bench_cold_document(group, corpus);
    }
    #[cfg(feature = "ffi")]
    if wanted("cold_document_c") {
        let c_replay = CReplay::new(corpus, &calls);
        c_replay.validate(&CReplay::compile(corpus), &calls);
        validated(&format!("{group_name}/cold_document_c"), "c", calls.len());
        group.bench_function("cold_document_c", |b| {
            b.iter_batched(
                || c_replay.fresh_ids(),
                |ids| {
                    let scanners = CReplay::compile(corpus);
                    c_replay.replay(&scanners, ids);
                    // Returned, so that Criterion drops them after timing.
                    scanners
                },
                BatchSize::PerIteration,
            );
        });
    }
    for &engine in Engine::ALL {
        let name = format!("cold_document_{}", engine.id());
        if !wanted(&name) {
            continue;
        }
        let id = format!("{group_name}/{name}");
        let replay = EngineReplay::new(engine, corpus, &calls);
        match replay.checked(corpus, &calls, true) {
            Ok((_, differences)) => {
                report(&id, engine, calls.len(), differences);
                group.bench_function(&name, |b| {
                    b.iter_batched(
                        || (),
                        |()| {
                            let searchers = replay.compile(corpus, true).unwrap();
                            replay.replay(&searchers);
                            searchers
                        },
                        BatchSize::PerIteration,
                    );
                });
            }
            Err(reason) => engines::unsupported(&id, engine, &reason),
        }
    }
}

fn validated(id: &str, engine: &str, calls: usize) {
    println!(
        "VALIDATED {}",
        serde_json::json!({"id": id, "engine": engine, "calls": calls})
    );
}

/// `VALIDATED`, or `EQUIVALENT` with the number of calls that differ only in
/// empty groups reported as unset (or the reverse).
fn report(id: &str, engine: Engine, calls: usize, empty_capture_differences: usize) {
    if empty_capture_differences == 0 {
        validated(id, engine.id(), calls);
    } else {
        println!(
            "EQUIVALENT {}",
            serde_json::json!({"id": id, "engine": engine.id(), "calls": calls,
                               "empty_capture_differences": empty_capture_differences})
        );
    }
}

/// One engine's replay of a call selection: the prepared subjects and calls.
/// The compiled scanners come from [`EngineReplay::compile`], so that the
/// cold lane can time their construction.
pub struct EngineReplay {
    engine: Engine,
    subjects: Vec<Subject>,
    /// (scanner, subject, UTF-16 start)
    calls: Vec<(usize, usize, usize)>,
}

/// One scanner's patterns as the engine searches them.
pub enum Searcher {
    /// Searched one pattern at a time, keeping the earliest match.
    Patterns(Vec<Compiled>),
    /// One RegexSet over all patterns, shared by every call of the scanner.
    Set(fancy_regex::RegexSet),
}

impl EngineReplay {
    pub fn new(engine: Engine, corpus: &Corpus, calls: &[&Call]) -> Self {
        Self {
            engine,
            subjects: corpus.subjects.iter().map(|s| Subject::new(s)).collect(),
            calls: calls
                .iter()
                .map(|call| (call.scanner, call.subject, call.start_utf16))
                .collect(),
        }
    }

    /// Compiles the scanners the calls use or, with `all`, every scanner of
    /// the trace, as the cold lane builds Ferroni's; the error names the
    /// first pattern the engine rejects.
    pub fn compile(&self, corpus: &Corpus, all: bool) -> Result<Vec<Searcher>, String> {
        corpus
            .patterns
            .iter()
            .enumerate()
            .map(|(id, patterns)| {
                if all || self.calls.iter().any(|&(scanner, _, _)| scanner == id) {
                    compile_scanner(self.engine, id, patterns)
                } else {
                    Ok(Searcher::Patterns(Vec::new()))
                }
            })
            .collect()
    }

    /// Compiles and checks every call against the captured result, through
    /// the code the timed replay runs. Returns the scanners and the number of
    /// calls whose only difference from the capture is an empty group
    /// reported as unset or the other way round; vscode-textmate skips
    /// zero-length captures, so both highlight alike. The error names the
    /// first pattern or call that differs otherwise.
    pub fn checked(
        &self,
        corpus: &Corpus,
        calls: &[&Call],
        all: bool,
    ) -> Result<(Vec<Searcher>, usize), String> {
        let searchers = self.compile(corpus, all)?;
        let mut differences = 0;
        for (i, (&(scanner, subject, start), call)) in self.calls.iter().zip(calls).enumerate() {
            let actual = expected_form(self.find(&searchers, scanner, subject, start)?);
            if actual != call.expected && highlighted(&actual) == highlighted(&call.expected) {
                differences += 1;
            } else if actual != call.expected {
                return Err(format!(
                    "call {i} (scanner {scanner}): {actual:?}, captured {:?}",
                    call.expected
                ));
            }
        }
        Ok((searchers, differences))
    }

    #[inline]
    fn find(
        &self,
        searchers: &[Searcher],
        scanner: usize,
        subject: usize,
        start_utf16: usize,
    ) -> Result<Utf16Match, String> {
        let subject = &self.subjects[subject];
        let text = subject.text.as_str();
        let start = subject.utf8_offset(start_utf16);
        match &searchers[scanner] {
            Searcher::Patterns(regexes) => {
                let mut best: Option<(usize, usize, Found<'_>)> = None;
                for (index, regex) in regexes.iter().enumerate() {
                    if let Some((at, found)) = regex.search_found(text, start)? {
                        if best.as_ref().is_none_or(|&(_, prior, _)| at < prior) {
                            best = Some((index, at, found));
                        }
                        if at == start {
                            break;
                        }
                    }
                }
                Ok(best.map(|(index, _, found)| {
                    let mut captures = SmallVec::new();
                    regexes[index].read_found(&found, |beg, end| {
                        captures.push(subject.utf16_span(beg, end));
                    });
                    (index, captures)
                }))
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
                        captures.get(group).map_or((0, 0), |m| {
                            subject.utf16_span(m.start() as i32, m.end() as i32)
                        })
                    })
                    .collect();
                Ok(Some((first.pattern(), spans)))
            }
        }
    }

    pub fn replay(&self, searchers: &[Searcher]) {
        for &(scanner, subject, start) in &self.calls {
            black_box(
                self.find(searchers, scanner, black_box(subject), black_box(start))
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
