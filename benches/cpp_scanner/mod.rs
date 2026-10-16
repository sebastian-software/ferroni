//! Replay of real Ferriki scanner calls, with setup outside measurement.
use criterion::{BatchSize, BenchmarkGroup, Throughput, measurement::WallTime};
use ferroni::scanner::{
    OnigString, Scanner, ScannerConfig, ScannerFindOptions, ScannerMatch, ScannerPatternCache,
};
use serde_json::Value;
use smallvec::SmallVec;
use std::hint::black_box;

pub type Match = Option<(usize, Vec<(usize, usize)>)>;

pub struct Call {
    pub scanner: usize,
    pub subject: usize,
    pub start_utf16: usize,
    pub options: ScannerFindOptions,
    pub option_bits: u32,
    pub expected: Match,
}

pub struct Corpus {
    pub patterns: Vec<Vec<String>>,
    pub subjects: Vec<String>,
    pub calls: Vec<Call>,
    pub hot_groups: Vec<usize>,
}

fn index(v: &Value) -> usize {
    usize::try_from(v.as_u64().expect("fixture integer")).expect("index fits usize")
}

/// The scanner configuration the benches build with: the default, or with
/// the DFA pre-filter off when `FERRONI_BENCH_PREFILTER=0` is set, so that
/// one binary measures both (ABBA runs).
pub fn bench_config() -> ScannerConfig {
    let off = std::env::var_os("FERRONI_BENCH_PREFILTER").is_some_and(|value| value == "0");
    ScannerConfig::default().prefilter(!off)
}

pub fn normalized(matched: Option<ScannerMatch>) -> Match {
    matched.map(|m| {
        (
            m.index,
            m.captures().iter().map(|c| (c.start, c.end)).collect(),
        )
    })
}

impl Corpus {
    pub fn load() -> Self {
        Self::from_json(include_str!("trace.json"))
    }

    pub fn from_json(json: &str) -> Self {
        let fixture: Value = serde_json::from_str(json).expect("valid trace JSON");
        assert_eq!(fixture["format_version"], 1);
        let patterns: Vec<Vec<String>> = fixture["scanners"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                p.as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_owned())
                    .collect()
            })
            .collect();
        let subjects: Vec<String> = fixture["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();
        let calls = fixture["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let option_bits = u32::try_from(index(&row[3])).unwrap();
                assert_eq!(option_bits & !7, 0);
                let expected = if row[4].is_null() {
                    None
                } else {
                    Some((
                        index(&row[4]),
                        row[5]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|c| (index(&c[0]), index(&c[1])))
                            .collect(),
                    ))
                };
                let call = Call {
                    scanner: index(&row[0]),
                    subject: index(&row[1]),
                    start_utf16: index(&row[2]),
                    options: ScannerFindOptions::from_bits(option_bits),
                    option_bits,
                    expected,
                };
                assert!(call.scanner < patterns.len() && call.subject < subjects.len());
                if let Some((matched, captures)) = &call.expected {
                    assert!(*matched < patterns[call.scanner].len());
                    assert!(!captures.is_empty());
                    assert!(captures.iter().all(
                        |&(a, b)| a <= b && b <= subjects[call.subject].encode_utf16().count()
                    ));
                }
                call
            })
            .collect();
        let hot_groups = fixture["hot_groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(index)
            .collect();
        Self {
            patterns,
            subjects,
            calls,
            hot_groups,
        }
    }

    /// The scanners of the trace, built with [`bench_config`].
    pub fn scanners(&self) -> Vec<Scanner> {
        self.scanners_with(&bench_config())
    }

    /// The scanners of the trace, built with `config`.
    pub fn scanners_with(&self, config: &ScannerConfig) -> Vec<Scanner> {
        self.patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
                Scanner::with_config(&refs, config).expect("captured patterns compile")
            })
            .collect()
    }

    /// The scanners built from one pattern cache, as a grammar loader that
    /// shares compiled patterns builds them.
    pub fn cached_scanners(&self) -> Vec<Scanner> {
        self.cached_scanners_with(&bench_config(), &mut ScannerPatternCache::new())
    }

    /// The scanners of the trace, built with `config` through `cache`.
    #[allow(dead_code)]
    pub fn cached_scanners_with(
        &self,
        config: &ScannerConfig,
        cache: &mut ScannerPatternCache,
    ) -> Vec<Scanner> {
        self.patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
                Scanner::with_pattern_cache(&refs, config, cache)
                    .expect("captured patterns compile")
            })
            .collect()
    }

    /// Keep distinct original subjects distinct, even when their contents match.
    /// New wrappers on every replay prevent fallback memo reuse across documents.
    pub fn strings(&self) -> Vec<OnigString> {
        self.subjects.iter().map(|s| OnigString::new(s)).collect()
    }

    pub fn selected(&self, group: Option<usize>) -> Vec<&Call> {
        if let Some(id) = group {
            assert!(id < self.patterns.len(), "invalid scanner group");
        }
        self.calls
            .iter()
            .filter(|call| group.is_none_or(|id| id == call.scanner))
            .collect()
    }

    pub fn validate(&self) {
        for (kind, mut scanners) in [
            ("uncached", self.scanners()),
            ("pattern cache", self.cached_scanners()),
        ] {
            let strings = self.strings();
            for (i, call) in self.calls.iter().enumerate() {
                let actual = normalized(scanners[call.scanner].find_next_match_utf16(
                    &strings[call.subject],
                    call.start_utf16,
                    call.options,
                ));
                assert_eq!(
                    actual, call.expected,
                    "{kind} capture trace differs at call {i}"
                );
            }
        }
        let strings = self.strings();
        // A group replay preserves that scanner's call order independently.
        for &id in &self.hot_groups {
            let mut scanners = self.scanners();
            for call in self.selected(Some(id)) {
                assert_eq!(
                    normalized(scanners[id].find_next_match_utf16(
                        &strings[call.subject],
                        call.start_utf16,
                        call.options
                    )),
                    call.expected
                );
            }
        }
        #[cfg(feature = "ffi")]
        self.validate_c();
    }

    #[cfg(feature = "ffi")]
    fn validate_c(&self) {
        use ferroni::ffi::{CRegex, CRegion, ONIG_OPTION_CAPTURE_GROUP};
        let regexes: Vec<Vec<_>> = self
            .patterns
            .iter()
            .map(|ps| {
                ps.iter()
                    .map(|p| {
                        CRegex::new(p.as_bytes(), ONIG_OPTION_CAPTURE_GROUP)
                            .expect("C pattern compiles")
                    })
                    .collect()
            })
            .collect();
        let strings = self.strings();
        let mut region = CRegion::new();
        for (i, call) in self.calls.iter().enumerate() {
            let subject = &strings[call.subject];
            let mut utf16 = 0;
            let start = subject
                .content()
                .char_indices()
                .find_map(|(byte, ch)| {
                    let at = utf16;
                    utf16 += ch.len_utf16();
                    (call.start_utf16 < utf16 && call.start_utf16 >= at).then_some(byte)
                })
                .unwrap_or(subject.content().len());
            let options = call.option_bits << 22;
            let mut best: Match = None;
            for (pattern, regex) in regexes[call.scanner].iter().enumerate() {
                region.clear();
                let position = regex.search(
                    subject.content().as_bytes(),
                    start,
                    subject.content().len(),
                    Some(&mut region),
                    options,
                );
                assert!(position >= -1, "C search error {position} at call {i}");
                if position >= 0 {
                    let captures: Vec<_> = region
                        .capture_ranges()
                        .into_iter()
                        .map(|(a, b)| {
                            if a >= 0 && b >= a {
                                (
                                    subject.content()[..a as usize].encode_utf16().count(),
                                    subject.content()[..b as usize].encode_utf16().count(),
                                )
                            } else {
                                (0, 0)
                            }
                        })
                        .collect();
                    if best
                        .as_ref()
                        .is_none_or(|(_, prior)| captures[0].0 < prior[0].0)
                    {
                        best = Some((pattern, captures));
                    }
                }
            }
            assert_eq!(best, call.expected, "C capture trace differs at call {i}");
        }
    }
}

/// Only the scanner calls and their returned captures belong to this boundary.
pub fn replay(scanners: &mut [Scanner], strings: &[OnigString], calls: &[&Call]) {
    for call in calls {
        black_box(scanners[call.scanner].find_next_match_utf16(
            black_box(&strings[call.subject]),
            black_box(call.start_utf16),
            black_box(call.options),
        ));
    }
}

/// The cold lane: each iteration compiles fresh scanners and replays the whole
/// document once. The timed section covers compilation, the pre-filter warm-up
/// and its build (after `PREFILTER_WARMUP` searches per scanner), and one replay.
/// JSON parsing, the string wrappers, and dropping the scanners stay outside
/// the timed section, as in the warm lane.
#[allow(dead_code)]
pub fn bench_cold_document(group: &mut BenchmarkGroup<'_, WallTime>, corpus: &Corpus) {
    let calls = corpus.selected(None);
    let config = bench_config();
    group.throughput(Throughput::Elements(calls.len() as u64));
    group.bench_function("cold_document", |b| {
        b.iter_batched(
            || corpus.strings(),
            |strings| {
                let mut scanners = corpus.scanners_with(&config);
                replay(&mut scanners, &strings, &calls);
                // Returned, so that Criterion drops them after the timed section.
                (scanners, strings)
            },
            BatchSize::PerIteration,
        );
    });
}

/// A scanner result as the comparator replays return it from each timed call:
/// the pattern index and every capture in UTF-16 code units, with the inline
/// capacity of Ferroni's `ScannerMatch`, so that neither side allocates for
/// up to eight groups.
pub type Utf16Match = Option<(usize, SmallVec<[(usize, usize); 8]>)>;

/// A subject with the offset tables `OnigString::new` builds, for the
/// comparator replays. They are built outside timing, as `Corpus::strings`
/// builds Ferroni's; the comparators then convert the UTF-16 start and the
/// returned captures with the same lookups inside their timed calls as
/// `find_next_match_utf16` does, and as vscode-oniguruma does in JavaScript.
pub struct Subject {
    pub text: String,
    ascii: bool,
    /// UTF-16 code unit -> UTF-8 byte offset, with an end sentinel.
    utf16_to_utf8: Vec<u32>,
    /// UTF-8 byte offset -> UTF-16 code unit, with an end sentinel.
    utf8_to_utf16: Vec<u32>,
}

impl Subject {
    pub fn new(text: &str) -> Self {
        let mut subject = Self {
            text: text.to_owned(),
            ascii: text.is_ascii(),
            utf16_to_utf8: Vec::new(),
            utf8_to_utf16: Vec::new(),
        };
        if subject.ascii {
            return subject;
        }
        subject.utf8_to_utf16 = vec![0; text.len() + 1];
        for (byte, ch) in text.char_indices() {
            let utf16 = subject.utf16_to_utf8.len() as u32;
            subject.utf8_to_utf16[byte..byte + ch.len_utf8()].fill(utf16);
            subject.utf16_to_utf8.push(byte as u32);
            // The low surrogate maps to the byte after the character.
            if ch.len_utf16() == 2 {
                subject.utf16_to_utf8.push((byte + ch.len_utf8()) as u32);
            }
        }
        subject.utf8_to_utf16[text.len()] = subject.utf16_to_utf8.len() as u32;
        subject.utf16_to_utf8.push(text.len() as u32);
        subject
    }

    /// The byte offset of a UTF-16 start position.
    #[inline]
    pub fn utf8_offset(&self, utf16: usize) -> usize {
        if self.ascii {
            utf16.min(self.text.len())
        } else {
            self.utf16_to_utf8
                .get(utf16)
                .map_or(self.text.len(), |&byte| byte as usize)
        }
    }

    /// A capture in UTF-16 code units. An unset group (-1) reads as (0, 0),
    /// as in Ferroni's scanner and the traces.
    #[inline]
    pub fn utf16_span(&self, beg: i32, end: i32) -> (usize, usize) {
        if beg < 0 || end < beg {
            (0, 0)
        } else if self.ascii {
            (beg as usize, end as usize)
        } else {
            (
                self.utf8_to_utf16[beg as usize] as usize,
                self.utf8_to_utf16[end as usize] as usize,
            )
        }
    }
}

/// A [`Utf16Match`] in the trace's form, for comparison with `Call::expected`.
pub fn expected_form(found: Utf16Match) -> Match {
    found.map(|(index, captures)| (index, captures.into_vec()))
}

/// The same calls through the vscode-oniguruma C scanner
/// (`benches/vscode_scanner_native.c`), the native counterpart of Shiki's WASM
/// engine. Each timed call converts the UTF-16 start through the subject's
/// table, searches, and reads the winner's captures from the scanner's result
/// buffer into a [`Utf16Match`], the boundary Ferroni's
/// `find_next_match_utf16` has.
#[cfg(feature = "ffi")]
pub struct CReplay {
    subjects: Vec<Subject>,
    /// (scanner, subject, UTF-16 start, Oniguruma search options)
    calls: Vec<(usize, usize, usize, std::os::raw::c_uint)>,
}

#[cfg(feature = "ffi")]
static NEXT_C_STR_CACHE_ID: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);

#[cfg(feature = "ffi")]
impl CReplay {
    /// Prepares the subjects and calls; [`Self::compile`] builds the scanners.
    pub fn new(corpus: &Corpus, calls: &[&Call]) -> Self {
        Self {
            subjects: corpus.subjects.iter().map(|s| Subject::new(s)).collect(),
            calls: calls
                .iter()
                .map(|call| {
                    (
                        call.scanner,
                        call.subject,
                        call.start_utf16,
                        call.option_bits << 22,
                    )
                })
                .collect(),
        }
    }

    /// Every scanner of the trace, as `Corpus::scanners` builds Ferroni's.
    pub fn compile(corpus: &Corpus) -> Vec<ferroni::ffi::CScanner> {
        corpus
            .patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_bytes).collect();
                ferroni::ffi::CScanner::new(&refs).expect("C scanner compiles")
            })
            .collect()
    }

    /// Checks every call against the captured result, through the code the
    /// timed replay runs, before anything is timed.
    pub fn validate(&self, scanners: &[ferroni::ffi::CScanner], calls: &[&Call]) {
        let ids = self.fresh_ids();
        for (i, (&(scanner, subject, start, options), call)) in
            self.calls.iter().zip(calls).enumerate()
        {
            let actual = self.find(scanners, scanner, subject, ids, start, options);
            assert_eq!(
                expected_form(actual),
                call.expected,
                "C scanner trace differs at call {i}"
            );
        }
    }

    /// New string-cache identities for one replay, like `Corpus::strings`.
    pub fn fresh_ids(&self) -> i32 {
        NEXT_C_STR_CACHE_ID.fetch_add(
            i32::try_from(self.subjects.len()).unwrap(),
            std::sync::atomic::Ordering::Relaxed,
        )
    }

    #[inline]
    fn find(
        &self,
        scanners: &[ferroni::ffi::CScanner],
        scanner: usize,
        subject: usize,
        ids: i32,
        start_utf16: usize,
        options: std::os::raw::c_uint,
    ) -> Utf16Match {
        let subject_text = &self.subjects[subject];
        let start = subject_text.utf8_offset(start_utf16);
        scanners[scanner].find_next_match_encoded(
            subject_text.text.as_bytes(),
            ids + subject as i32,
            start,
            options,
            |index, bounds| {
                let captures = bounds
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[beg, end]| subject_text.utf16_span(beg, end))
                    .collect();
                (index, captures)
            },
        )
    }

    pub fn replay(&self, scanners: &[ferroni::ffi::CScanner], ids: i32) {
        for &(scanner, subject, start, options) in &self.calls {
            black_box(self.find(
                scanners,
                scanner,
                black_box(subject),
                ids,
                black_box(start),
                options,
            ));
        }
    }
}
