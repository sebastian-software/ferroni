//! Replay of real Ferriki scanner calls, with setup outside measurement.
use ferroni::scanner::{
    OnigString, Scanner, ScannerConfig, ScannerFindOptions, ScannerMatch, ScannerPatternCache,
};
use serde_json::Value;
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

pub fn normalized(matched: Option<ScannerMatch>) -> Match {
    matched.map(|m| {
        (
            m.index,
            m.capture_indices.iter().map(|c| (c.start, c.end)).collect(),
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

    pub fn scanners(&self) -> Vec<Scanner> {
        self.patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
                Scanner::new(&refs).expect("captured patterns compile")
            })
            .collect()
    }

    /// The scanners built from one pattern cache, as a grammar loader that
    /// shares compiled patterns builds them.
    pub fn cached_scanners(&self) -> Vec<Scanner> {
        let config = ScannerConfig::default();
        let mut cache = ScannerPatternCache::new();
        self.patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_str).collect();
                Scanner::with_pattern_cache(&refs, &config, &mut cache)
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

/// The same calls through the vscode-oniguruma C scanner
/// (`benches/vscode_scanner_native.c`), the native counterpart of Shiki's WASM
/// engine. UTF-16 starts become byte offsets outside timing; vscode-oniguruma
/// performs that conversion in JavaScript, Ferroni inside its timed call.
#[cfg(feature = "ffi")]
pub struct CReplay {
    scanners: Vec<ferroni::ffi::CScanner>,
    subjects: Vec<String>,
    /// (scanner, subject, byte start, Oniguruma search options)
    calls: Vec<(usize, usize, usize, std::os::raw::c_uint)>,
}

#[cfg(feature = "ffi")]
static NEXT_C_STR_CACHE_ID: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);

#[cfg(feature = "ffi")]
impl CReplay {
    /// Builds the C scanners and checks every selected call against the
    /// captured Shiki result before anything is timed.
    pub fn new(corpus: &Corpus, calls: &[&Call]) -> Self {
        let scanners = corpus
            .patterns
            .iter()
            .map(|patterns| {
                let refs: Vec<_> = patterns.iter().map(String::as_bytes).collect();
                ferroni::ffi::CScanner::new(&refs).expect("C scanner compiles")
            })
            .collect();
        let replay = Self {
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
                        call.option_bits << 22,
                    )
                })
                .collect(),
        };
        let ids = replay.fresh_ids();
        for (i, (&(scanner, subject, start, options), call)) in
            replay.calls.iter().zip(calls).enumerate()
        {
            let text = &replay.subjects[subject];
            let actual = replay.scanners[scanner]
                .find_next_match_with_options(text.as_bytes(), ids + subject as i32, start, options)
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
                        .collect();
                    (index, captures)
                });
            assert_eq!(actual, call.expected, "C scanner trace differs at call {i}");
        }
        replay
    }

    /// New string-cache identities for one replay, like `Corpus::strings`.
    pub fn fresh_ids(&self) -> i32 {
        NEXT_C_STR_CACHE_ID.fetch_add(
            i32::try_from(self.subjects.len()).unwrap(),
            std::sync::atomic::Ordering::Relaxed,
        )
    }

    pub fn replay(&self, ids: i32) {
        for &(scanner, subject, start, options) in &self.calls {
            black_box(self.scanners[scanner].find_next_match_with_options(
                black_box(self.subjects[subject].as_bytes()),
                ids + subject as i32,
                black_box(start),
                options,
            ));
        }
    }
}

#[cfg(feature = "ffi")]
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
