//! The idiomatic Rust API. [`Regex`] compiles and searches a pattern,
//! [`RegexBuilder`] sets its options and syntax, and [`Match`], [`Captures`] and
//! [`FindIter`] report what a search found. It wraps the C-port functions
//! [`onig_new`](crate::regcomp::onig_new) and [`onig_search`].

use std::cell::RefCell;
use std::fmt;
use std::ops::{Index, Range};
use std::panic::AssertUnwindSafe;
use std::str::FromStr;
use std::sync::Arc;
#[cfg(feature = "dfa-prefilter")]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

#[cfg(feature = "dfa-prefilter")]
use crate::dfa_prefilter::SingleSeek;
use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::error::RegexError;
use crate::oniguruma::*;
use crate::regcomp::{AcceptSeek, ScannerSeek, onig_new_for_single_seek};
use crate::regexec::{
    MatchArg, SearchLimits, cache_msa, onig_name_to_backref_number, onig_search,
    onig_search_bounds, onig_search_captures, pure_literal_first, release_oversized_stack,
    search_in_range, search_in_range_inner, take_cached_msa,
};
use crate::regint::RegexType;
use crate::regparse_types::NameTable;
use crate::regsyntax::{
    OnigSyntaxASIS, OnigSyntaxEmacs, OnigSyntaxGnuRegex, OnigSyntaxGrep, OnigSyntaxJava,
    OnigSyntaxOniguruma, OnigSyntaxPerl, OnigSyntaxPerl_NG, OnigSyntaxPosixBasic,
    OnigSyntaxPosixExtended, OnigSyntaxPython, OnigSyntaxRuby,
};

thread_local! {
    /// One reusable capture region per thread. Taking the value out keeps
    /// nested and re-entrant searches independent; returning the larger region
    /// retains the most useful allocation when result values overlap.
    static CACHED_REGION: RefCell<Option<OnigRegion>> = const { RefCell::new(None) };
}

fn take_cached_region() -> OnigRegion {
    CACHED_REGION
        .try_with(|cached| cached.borrow_mut().take())
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn cache_region(region: OnigRegion) {
    // Drop can run after this thread-local's destructor during thread
    // teardown. In that case, simply let the uncached region be freed.
    let _ = CACHED_REGION.try_with(|cached| {
        let mut cached = cached.borrow_mut();
        let region_capacity = region.beg.capacity() + region.end.capacity();
        let should_replace = match cached.as_ref() {
            Some(current) => region_capacity > current.beg.capacity() + current.end.capacity(),
            None => true,
        };
        if should_replace {
            *cached = Some(region);
        }
    });
}

/// Panics unless `start` is a char boundary of `text` at or before its end:
/// the start offsets that `find_at` and its relatives accept for `&str`.
#[track_caller]
fn check_str_start(text: &str, start: usize) {
    check_bytes_start(text.as_bytes(), start);
    assert!(
        text.is_char_boundary(start),
        "start {start} is not a char boundary of the text"
    );
}

/// Panics unless `start` is at or before the end of `text`.
#[track_caller]
fn check_bytes_start(text: &[u8], start: usize) {
    assert!(
        start <= text.len(),
        "start {start} is past the end of the text ({} bytes)",
        text.len()
    );
}

fn timeout_to_millis(timeout: Duration) -> u64 {
    if timeout.is_zero() {
        return 0;
    }

    let millis = timeout.as_millis();
    let millis = millis + u128::from(!timeout.subsec_nanos().is_multiple_of(1_000_000));
    millis.clamp(1, u64::MAX as u128) as u64
}

/// Limits for a single search.
///
/// Every limit left unset keeps its process-wide setting (for example from
/// [`onig_set_time_limit`](crate::regexec::onig_set_time_limit)). A limit set
/// to zero turns that limit off for this search, as in Oniguruma.
///
/// Pass the options to the `*_with` search methods of [`Regex`], such as
/// [`Regex::find_with`]. Unlike the plain methods, they report a search that
/// stopped at a limit as an error instead of as "no match".
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use ferroni::prelude::*;
///
/// let re = Regex::new(r"(a+)+b").unwrap();
/// let options = SearchOptions::new().timeout(Duration::from_millis(50));
///
/// assert_eq!(re.find_with("aab", options).unwrap().unwrap().as_str(), "aab");
///
/// let hostile = "a".repeat(40);
/// assert!(re.find_with(&hostile, options).is_err());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    timeout: Option<Duration>,
    retry_limit_in_match: Option<u64>,
    retry_limit_in_search: Option<u64>,
    match_stack_limit: Option<u32>,
}

impl SearchOptions {
    /// Options that keep every process-wide limit.
    pub const fn new() -> Self {
        SearchOptions {
            timeout: None,
            retry_limit_in_match: None,
            retry_limit_in_search: None,
            match_stack_limit: None,
        }
    }

    /// Stop the search with [`RegexError::TimeLimitOver`] once `timeout` has
    /// passed. `Duration::ZERO` removes the time limit for this search.
    ///
    /// The engine reads the clock every 512 backtracks, so the search can run
    /// slightly past the deadline. Durations are rounded up to whole
    /// milliseconds.
    ///
    /// `wasm32-unknown-unknown` has no clock: there a search with a time
    /// limit, from this option or from
    /// [`onig_set_time_limit`](crate::regexec::onig_set_time_limit), panics
    /// when it starts. Bound the work with
    /// [`retry_limit_in_search`](Self::retry_limit_in_search) on that target
    /// instead. A search without a time limit never reads the clock.
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Stop the search with [`RegexError::RetryLimitInMatchOver`] after `limit`
    /// backtracks at one start position. Zero removes the limit.
    pub const fn retry_limit_in_match(mut self, limit: u64) -> Self {
        self.retry_limit_in_match = Some(limit);
        self
    }

    /// Stop the search with [`RegexError::RetryLimitInSearchOver`] after `limit`
    /// backtracks in total. Zero removes the limit.
    pub const fn retry_limit_in_search(mut self, limit: u64) -> Self {
        self.retry_limit_in_search = Some(limit);
        self
    }

    /// Stop the search with [`RegexError::MatchStackLimitOver`] once the
    /// backtracking stack needs more than `limit` entries. As in C Oniguruma,
    /// a limit below 320 lets the stack hold 321 entries. Zero removes the
    /// limit.
    pub const fn match_stack_limit(mut self, limit: u32) -> Self {
        self.match_stack_limit = Some(limit);
        self
    }

    /// The limits a search with these options applies, or `None` when every
    /// limit is process-wide. `None` runs the same path as the standard methods.
    #[inline]
    fn limits(self) -> Option<SearchLimits> {
        if self == Self::new() {
            return None;
        }
        let process_wide = SearchLimits::process_wide();
        Some(SearchLimits {
            match_stack_limit: self
                .match_stack_limit
                .unwrap_or(process_wide.match_stack_limit),
            retry_limit_in_match: self
                .retry_limit_in_match
                .unwrap_or(process_wide.retry_limit_in_match),
            retry_limit_in_search: self
                .retry_limit_in_search
                .unwrap_or(process_wide.retry_limit_in_search),
            time_limit: self
                .timeout
                .map_or(process_wide.time_limit, timeout_to_millis),
            progress_callout: process_wide.progress_callout,
            retraction_callout: process_wide.retraction_callout,
            callout_user_data: process_wide.callout_user_data,
        })
    }
}

/// The immutable state behind a [`Regex`]: the compiled program, the pattern
/// it was compiled from, and the single-pattern seek automaton. Every clone of
/// a `Regex`, and every [`Captures`] it returns, shares one value through an
/// `Arc`.
struct Compiled {
    /// Wrapped in `AssertUnwindSafe`; the notes on [`Regex`] explain why the
    /// assertion holds.
    program: AssertUnwindSafe<RegexType>,
    pattern: Box<[u8]>,
    /// Rust-only (ADR-008): the seek automaton that standard searches consult,
    /// when the builder's `dfa` option is on, built on the first search that
    /// consults it. Wrapped like `program`: the lazy DFA's cache pool is not
    /// `RefUnwindSafe`.
    #[cfg(feature = "dfa-prefilter")]
    seek: AssertUnwindSafe<LazySeek>,
    /// Rust-only (ADR-008): decides whether the capture and find methods consult the
    /// seek, from the rejections they measure. Shared by the clones with the
    /// seek, as is the seek's cache pool.
    #[cfg(feature = "dfa-prefilter")]
    seek_gate: SeekGate,
}

/// Rust-only (ADR-008): a regex's seek automaton, built from its encoded seek
/// and accept seek by the first search that consults it. Building decodes the
/// seeks and compiles their NFAs and lazy DFAs, which costs a large part of
/// a compilation, while the iterators and the `*_with` methods never consult
/// the seek and many regexes are never searched at all. Every clone shares
/// the one automaton.
#[cfg(feature = "dfa-prefilter")]
struct LazySeek {
    /// The seek and the accept seek, until the automaton is built.
    codes: std::sync::Mutex<Option<(crate::dfa_prefilter::Seek, AcceptSeek)>>,
    /// The automaton, or `None` where it does not build (`SingleSeek::build`).
    automaton: std::sync::OnceLock<Option<Box<SingleSeek>>>,
}

#[cfg(feature = "dfa-prefilter")]
impl LazySeek {
    /// A seek to build from `codes`, or none.
    fn new(codes: Option<(crate::dfa_prefilter::Seek, AcceptSeek)>) -> LazySeek {
        let automaton = std::sync::OnceLock::new();
        if codes.is_none() {
            let _ = automaton.set(None);
        }
        LazySeek {
            codes: std::sync::Mutex::new(codes),
            automaton,
        }
    }

    /// The automaton, built on the first call; a concurrent first call waits
    /// for it.
    #[inline]
    fn get(&self) -> Option<&SingleSeek> {
        self.automaton
            .get_or_init(|| {
                let codes = self
                    .codes
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                codes
                    .and_then(|(seek, accept)| SingleSeek::build(&seek, accept.as_ref()))
                    .map(Box::new)
            })
            .as_deref()
    }

    /// Whether the regex may have an automaton: none was built yet, or one
    /// was. Does not build it.
    #[inline]
    fn may_exist(&self) -> bool {
        self.automaton.get().is_none_or(Option::is_some)
    }

    /// Whether the automaton was built. For the tests.
    #[cfg(test)]
    fn built(&self) -> bool {
        self.automaton.get().is_some_and(Option::is_some)
    }
}

/// While the [`SeekGate`] is closed, one capture or find search in this many consults
/// the seek anyway, so that the rejection rate is still measured.
#[cfg(feature = "dfa-prefilter")]
const SEEK_GATE_PROBE_INTERVAL: u32 = 32;

#[cfg(feature = "dfa-prefilter")]
thread_local! {
    /// Per-thread state of [`probe_closed_gate`]: a xorshift32 generator with
    /// a fixed nonzero seed, so a thread's probe sequence is reproducible.
    static GATE_PROBE: std::cell::Cell<u32> = const { std::cell::Cell::new(0x9E37_79B9) };
}

/// Whether a search through a closed [`SeekGate`] consults the seek: one in
/// [`SEEK_GATE_PROBE_INTERVAL`] on average.
///
/// The gate is shared by every thread that searches with the regex. Counting
/// the passed searches in one shared atomic made every search write it, which
/// serialized those threads. A thread-local counter would avoid that but would
/// alias with regexes a thread alternates between (two regexes taking turns
/// would always land the probe on the same one); independent draws from a
/// per-thread generator do neither.
#[cfg(feature = "dfa-prefilter")]
fn probe_closed_gate() -> bool {
    GATE_PROBE.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        state.set(x);
        // The high bits decide: xorshift's low bits are its weakest.
        (u64::from(x) * u64::from(SEEK_GATE_PROBE_INTERVAL)) >> 32 == 0
    })
}

/// Estimated instructions of one consultation: a seek search of the text from
/// the start, about 250 to 410 on the production patterns that consult it.
#[cfg(feature = "dfa-prefilter")]
const SEEK_CONSULT_COST_IR: u32 = 300;

/// Estimated instructions one rejection saves: the matcher's attempts on a
/// text the seek rejects. Estimated from the production cases that reject:
/// about 800 for `jinja_float_token` and about 5,000 for
/// `fluent_bit_nginx`, so this is the low end.
#[cfg(feature = "dfa-prefilter")]
const SEEK_REJECT_SAVING_IR: u32 = 800;

/// The rejection rate, in permille, at which a closed [`SeekGate`] opens: the
/// break-even of a consultation, `SEEK_CONSULT_COST_IR / SEEK_REJECT_SAVING_IR`.
#[cfg(feature = "dfa-prefilter")]
const SEEK_GATE_OPEN_PERMILLE: u32 = SEEK_CONSULT_COST_IR * 1000 / SEEK_REJECT_SAVING_IR;

/// The rate, in permille, below which an open [`SeekGate`] closes: two thirds
/// of the break-even. The gap keeps a run of accepted searches from closing
/// the gate on a pattern that rejects often.
#[cfg(feature = "dfa-prefilter")]
const SEEK_GATE_CLOSE_PERMILLE: u32 = SEEK_GATE_OPEN_PERMILLE * 2 / 3;

/// The adaptive gate of the seek's consultations by the standard capture
/// methods (ADR-008). A consultation costs about [`SEEK_CONSULT_COST_IR`]
/// instructions and saves [`SEEK_REJECT_SAVING_IR`] for each rejection, so it
/// pays at a rejection rate of about [`SEEK_GATE_OPEN_PERMILLE`] per mille.
///
/// The gate keeps an exponentially weighted rate of the consultations' answers
/// (each new answer weighs 1/8, a memory of about eight searches). It starts
/// open, so the first searches consult the seek and the rate settles within
/// a few of them. An open gate closes below [`SEEK_GATE_CLOSE_PERMILLE`]; a
/// closed one consults one search in [`SEEK_GATE_PROBE_INTERVAL`], so the rate
/// keeps being measured, and opens again at [`SEEK_GATE_OPEN_PERMILLE`].
///
/// The gate changes only which search runs the seek: a rejection is the
/// matcher's answer under every limit, so no result depends on it. Its state
/// is relaxed atomics, shared by the clones of a `Regex` as the seek's cache
/// pool is. Two threads may lose an update to the rate, which costs time at
/// most.
#[cfg(feature = "dfa-prefilter")]
struct SeekGate {
    /// The rejection rate of the consultations, in permille.
    rate: AtomicU32,
    /// Whether every search consults the seek.
    open: AtomicBool,
    /// Test-only override: 0 is the adaptive gate, 1 consults on every search,
    /// 2 on none.
    #[cfg(test)]
    forced: std::sync::atomic::AtomicU8,
}

#[cfg(feature = "dfa-prefilter")]
impl SeekGate {
    const fn new() -> Self {
        SeekGate {
            rate: AtomicU32::new(400),
            open: AtomicBool::new(true),
            #[cfg(test)]
            forced: std::sync::atomic::AtomicU8::new(0),
        }
    }

    /// Whether this search consults the seek.
    #[inline]
    fn consults(&self) -> bool {
        #[cfg(test)]
        match self.forced.load(Ordering::Relaxed) {
            1 => return true,
            2 => return false,
            _ => {}
        }
        self.open.load(Ordering::Relaxed) || probe_closed_gate()
    }

    /// Records the answer of one consultation and sets the state for the next
    /// search.
    fn record(&self, rejected: bool) {
        let sample = if rejected { 1000 } else { 0 };
        let previous = self.rate.load(Ordering::Relaxed);
        let rate = (previous * 7 + sample) / 8;
        // Stores only on a change: a steady rejection rate reaches a fixed
        // point (0 or 993), and threads that keep rewriting the same value
        // still take the cache line from each other.
        if rate != previous {
            self.rate.store(rate, Ordering::Relaxed);
        }
        let open = self.open.load(Ordering::Relaxed);
        let threshold = if open {
            SEEK_GATE_CLOSE_PERMILLE
        } else {
            SEEK_GATE_OPEN_PERMILLE
        };
        if (rate >= threshold) != open {
            self.open.store(!open, Ordering::Relaxed);
        }
    }

    /// Whether the gate is open and the rejection rate, in permille. For the
    /// tests.
    #[cfg(test)]
    fn state(&self) -> (bool, u32) {
        (
            self.open.load(Ordering::Relaxed),
            self.rate.load(Ordering::Relaxed),
        )
    }
}

/// Whether a builder and `Regex::new` build the seek automaton by default:
/// with the `dfa-prefilter` feature, yes (ADR-008).
const DFA_DEFAULT: bool = cfg!(feature = "dfa-prefilter");

/// Whether `start` is a character boundary of valid UTF-8 `text` (or the
/// end of it). The seek reads characters from there.
#[cfg(feature = "dfa-prefilter")]
fn is_char_boundary(text: &[u8], start: usize) -> bool {
    text.get(start).is_none_or(|&byte| (byte as i8) >= -0x40)
}

/// A compiled regular expression.
///
/// Cloning a `Regex` is cheap: clones share one compiled program through an
/// `Arc`, so a clone costs one atomic increment and no recompilation.
///
/// `Regex` is [`UnwindSafe`](std::panic::UnwindSafe) and
/// [`RefUnwindSafe`](std::panic::RefUnwindSafe), so it can be used inside
/// [`std::panic::catch_unwind`] without a wrapper. This holds because the
/// compiled program is never mutated after compilation: searches only read
/// it, apart from search helpers built on first use behind a `OnceLock`, which
/// is unwind-safe itself. Their scratch state lives in thread-local caches and
/// in each search's own region. The one field the auto traits reject is the
/// `&'static dyn Encoding` inside `RegexType`, since a trait object does not
/// promise `RefUnwindSafe`. Encodings are stateless statics, so the assertion
/// is sound, and it needs no `unsafe`.
///
/// # Examples
///
/// ```
/// use ferroni::api::Regex;
///
/// let re = Regex::new(r"\d+").unwrap();
/// assert!(re.is_match("hello 42"));
///
/// let m = re.find("hello 42").unwrap();
/// assert_eq!(m.as_str(), "42");
/// assert_eq!(m.start(), 6);
/// assert_eq!(m.end(), 8);
/// ```
///
/// A `Regex` parses from a string and displays as its pattern:
///
/// ```
/// use ferroni::api::Regex;
///
/// let re: Regex = r"\d+".parse().unwrap();
/// assert_eq!(re.to_string(), r"\d+");
/// assert_eq!(re.clone().as_str(), Some(r"\d+"));
/// ```
#[derive(Clone)]
pub struct Regex {
    inner: Arc<Compiled>,
}

impl Regex {
    /// Compile a pattern using default options (Oniguruma syntax, UTF-8, no flags).
    pub fn new(pattern: &str) -> Result<Regex, RegexError> {
        Self::new_bytes(pattern.as_bytes())
    }

    /// Compile a pattern from raw bytes using default options.
    ///
    /// The pattern is kept as given, so [`Regex::as_bytes`] returns exactly
    /// these bytes. Oniguruma accepts some byte sequences that are not valid
    /// UTF-8 (encoded surrogates, for example), and [`Regex::as_str`] returns
    /// `None` for those.
    pub fn new_bytes(pattern: &[u8]) -> Result<Regex, RegexError> {
        let (program, seek, accept) = onig_new_for_single_seek(
            pattern,
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
            false,
            DFA_DEFAULT,
        )?;
        Ok(Self::from_parts(program, pattern.into(), seek, accept))
    }

    /// Wraps a compiled program, its seek and its accept seek. The seek's
    /// automaton is built by the first search that consults it (ADR-008,
    /// `LazySeek`), once for every clone.
    #[cfg_attr(not(feature = "dfa-prefilter"), allow(unused_variables))]
    fn from_parts(
        program: RegexType,
        pattern: Box<[u8]>,
        seek: ScannerSeek,
        accept: AcceptSeek,
    ) -> Regex {
        // A pure literal is found by a literal search, so it has no automaton.
        // The automaton is boxed: inline, it grew every compiled regex, and
        // the compile and search costs of the others with it.
        #[cfg(feature = "dfa-prefilter")]
        let seek = AssertUnwindSafe(LazySeek::new(if program.pure_literal {
            None
        } else {
            seek.map(|seek| (seek, accept))
        }));
        Regex {
            inner: Arc::new(Compiled {
                program: AssertUnwindSafe(program),
                pattern,
                #[cfg(feature = "dfa-prefilter")]
                seek,
                #[cfg(feature = "dfa-prefilter")]
                seek_gate: SeekGate::new(),
            }),
        }
    }

    /// The compiled program, shared by every clone of this regex.
    fn program(&self) -> &RegexType {
        &self.inner.program
    }

    /// The answer of the single-pattern seek (ADR-008) to a standard search
    /// from `start`, when it decides one, and `None` for the matcher.
    ///
    /// `Some(false)` is "no match", given when no seek match starts in
    /// `text[start..]`: the matcher can only find a match the seek describes,
    /// so the answer is the matcher's under every limit, which can only turn
    /// a match into "no match".
    ///
    /// `Some(true)` is "match", given with `accept` only when the seek is
    /// exact (`SeekCode::exact`) or the accept seek, an under-approximation
    /// of the pattern, matches. A match of either is a match the matcher
    /// finds when no limit stops it. Under ADR-017 that is the answer
    /// wherever the matcher would stop at a process-wide limit: C then stops
    /// with an error, and Ferroni answers the result C gives without the
    /// limit (rule 2), and it never reports an error where C completes
    /// (rule 3). With `accept`, the accept seek also answers "no match" where
    /// it is exact on the text (`SingleSeek::accept_answer`), which is the
    /// matcher's answer under every limit. Every other case goes to the
    /// matcher. `find` does not accept, since it needs the match's bounds.
    ///
    /// `utf8` says whether `text` is known to be valid UTF-8; otherwise it is
    /// checked. A seek reads characters, so the answer is only the matcher's
    /// for valid UTF-8 and a `start` on a character boundary. Searches under
    /// the caller's own limits (`*_with`) do not come here, so their limit
    /// errors are the matcher's.
    #[cfg(feature = "dfa-prefilter")]
    #[inline]
    fn seek_answer(&self, text: &[u8], start: usize, mut utf8: bool, accept: bool) -> Option<bool> {
        let seek = self.inner.seek.get()?;
        if accept && seek.has_accept() {
            if !(utf8 || std::str::from_utf8(text).is_ok()) || !is_char_boundary(text, start) {
                return None;
            }
            utf8 = true;
            if let Some(answer) = seek.accept_answer(text, start) {
                #[cfg(feature = "prefilter-self-check")]
                self.check_accept_answer(text, start, answer);
                return Some(answer);
            }
        }
        let found = seek.search(text, start)?;
        if found && !(accept && seek.exact()) {
            return None;
        }
        if !(utf8 || std::str::from_utf8(text).is_ok()) || !is_char_boundary(text, start) {
            return None;
        }
        Some(found)
    }

    /// Validation (feature `prefilter-self-check`): an answer of the accept
    /// seek is the matcher's wherever the matcher completes under the
    /// process-wide limits.
    #[cfg(feature = "prefilter-self-check")]
    fn check_accept_answer(&self, text: &[u8], start: usize, answer: bool) {
        let (result, _) = onig_search(
            self.program(),
            text,
            text.len(),
            start,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        if result >= 0 || result == ONIG_MISMATCH {
            assert_eq!(
                result >= 0,
                answer,
                "the accept seek of {:?} answers {answer} on {:?} from {start}",
                String::from_utf8_lossy(self.as_bytes()),
                String::from_utf8_lossy(text),
            );
        }
    }

    #[cfg(not(feature = "dfa-prefilter"))]
    #[inline]
    fn seek_answer(&self, text: &[u8], start: usize, utf8: bool, accept: bool) -> Option<bool> {
        let _ = (text, start, utf8, accept);
        None
    }

    /// Whether the seek rejects a capture or find search from `start`: the answer of
    /// [`Regex::seek_answer`] is "no match", and the adaptive gate
    /// ([`SeekGate`]) lets this search consult the seek. The rejection is the
    /// matcher's answer under every limit, so the capture and find methods return `None`
    /// for it without running the matcher.
    #[cfg(feature = "dfa-prefilter")]
    #[inline]
    fn seek_rejects_search(&self, text: &[u8], start: usize, utf8: bool) -> bool {
        if !self.inner.seek.may_exist() || !self.inner.seek_gate.consults() {
            return false;
        }
        let rejected = self.seek_answer(text, start, utf8, false) == Some(false);
        self.inner.seek_gate.record(rejected);
        rejected
    }

    #[cfg(not(feature = "dfa-prefilter"))]
    #[inline(always)]
    fn seek_rejects_search(&self, text: &[u8], start: usize, utf8: bool) -> bool {
        let _ = (text, start, utf8);
        false
    }

    /// Whether the regex has a seek automaton. For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn seek_present(&self) -> bool {
        self.inner.seek.get().is_some()
    }

    /// Whether the seek's automaton was built. For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn seek_built(&self) -> bool {
        self.inner.seek.built()
    }

    /// Whether the seek automaton is exact. For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn seek_exact(&self) -> Option<bool> {
        self.inner.seek.get().map(SingleSeek::exact)
    }

    /// The seek's answer to `is_match` on `text` from 0, accepting where it
    /// may, as `is_match` asks. For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn seek_decides_is_match(&self, text: &str) -> Option<bool> {
        self.seek_answer(text.as_bytes(), 0, true, true)
    }

    /// Whether the regex has an accept seek. For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn accept_seek_present(&self) -> bool {
        self.inner.seek.get().is_some_and(SingleSeek::has_accept)
    }

    /// The accept seek's own answer on `text` from `start`
    /// (`SingleSeek::accept_answer`). For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn accept_answer(&self, text: &str, start: usize) -> Option<bool> {
        self.inner.seek.get()?.accept_answer(text.as_bytes(), start)
    }

    /// Heap memory of the seek's automata and this thread's caches. For the
    /// tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn seek_memory_usage(&self) -> usize {
        self.inner.seek.get().map_or(0, SingleSeek::memory_usage)
    }

    /// The adaptive gate of the capture methods: whether it is open and its
    /// rejection rate in permille. For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn seek_gate_state(&self) -> (bool, u32) {
        self.inner.seek_gate.state()
    }

    /// Forces the gate on (`Some(true)`), off (`Some(false)`) or back to the
    /// adaptive rule (`None`). For the tests.
    #[cfg(all(test, feature = "dfa-prefilter"))]
    pub(crate) fn seek_gate_force(&self, mode: Option<bool>) {
        let value = match mode {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        };
        self.inner
            .seek_gate
            .forced
            .store(value, std::sync::atomic::Ordering::Relaxed);
    }

    /// The pattern this regex was compiled from, or `None` if the pattern is
    /// not valid UTF-8 (only possible through [`Regex::new_bytes`]).
    ///
    /// Use [`Regex::as_bytes`] to read the pattern losslessly.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// assert_eq!(Regex::new(r"a+b").unwrap().as_str(), Some("a+b"));
    /// assert_eq!(Regex::new_bytes(b"a+b").unwrap().as_str(), Some("a+b"));
    /// ```
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.inner.pattern).ok()
    }

    /// The pattern's bytes, exactly as passed to [`Regex::new`] or
    /// [`Regex::new_bytes`].
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// assert_eq!(Regex::new("é").unwrap().as_bytes(), "é".as_bytes());
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner.pattern
    }

    /// Iterate over the names of the capture groups, in group order.
    ///
    /// The iterator yields one item per group, starting with group 0, so item
    /// `i` names group `i`, the index that [`Captures::get`] takes. Group 0
    /// and unnamed groups yield `None`. A name shared by several groups
    /// appears at each of them.
    ///
    /// Oniguruma does not capture plain `(...)` groups in a pattern that has
    /// named groups, so such groups are absent from the iterator as well.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
    /// let names: Vec<_> = re.capture_names().collect();
    /// assert_eq!(names, [None, Some("year"), Some("month")]);
    /// ```
    pub fn capture_names(&self) -> CaptureNames<'_> {
        CaptureNames {
            table: self.program().name_table.as_ref(),
            next_group: 0,
            end_group: self.captures_len() as i32 + 1,
        }
    }

    /// Create a [`RegexBuilder`] for fine-grained control over compilation.
    pub fn builder(pattern: &str) -> RegexBuilder {
        RegexBuilder::new(pattern)
    }

    /// Return the first match in `text`, or `None` if no match.
    ///
    /// A search that stops at a process-wide limit (time, retry or stack) is
    /// reported as no match. Use [`Regex::find_with`] with [`SearchOptions`] to tell
    /// the two apart.
    pub fn find<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.find_from(Subject::Str(text), 0)
    }

    /// Return the first match in `text` (as bytes), or `None` if no match.
    pub fn find_bytes<'t>(&self, text: &'t [u8]) -> Option<Match<'t>> {
        self.find_from(Subject::Bytes(text), 0)
    }

    /// Return the first match in `text` at or after byte offset `start`, or
    /// `None` if no match.
    ///
    /// The search sees all of `text`, not only the part from `start`. A
    /// look-behind, `\b` or `^` at `start` looks at the bytes before it, and
    /// `\G` matches at `start`. Slicing the text instead (`&text[start..]`)
    /// hides those bytes, which can change the result. The returned offsets
    /// are relative to `text`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()` or is not on a char
    /// boundary of `text`.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// let re = Regex::new(r"(?<=a)b").unwrap();
    /// // The look-behind sees the `a` before offset 1.
    /// assert_eq!(re.find_at("ab", 1).unwrap().range(), 1..2);
    /// // A slice is a new text, and its `a` is out of view.
    /// assert!(re.find(&"ab"[1..]).is_none());
    /// ```
    ///
    /// `\G` matches at `start` itself:
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// let re = Regex::new(r"\Gb").unwrap();
    /// assert_eq!(re.find_at("ab", 1).unwrap().range(), 1..2);
    /// assert!(re.find_at("ab", 0).is_none());
    /// ```
    #[inline]
    pub fn find_at<'t>(&self, text: &'t str, start: usize) -> Option<Match<'t>> {
        check_str_start(text, start);
        self.find_from(Subject::Str(text), start)
    }

    /// Return the first match in `text` (as bytes) at or after byte offset
    /// `start`, or `None` if no match.
    ///
    /// Behaves as [`Regex::find_at`] does, except that `text` is not required
    /// to be valid UTF-8 and `start` may fall inside a character.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    pub fn find_bytes_at<'t>(&self, text: &'t [u8], start: usize) -> Option<Match<'t>> {
        check_bytes_start(text, start);
        self.find_from(Subject::Bytes(text), start)
    }

    /// The search behind the `find` family: the first match from `start`,
    /// with all of `text` visible.
    #[inline]
    fn find_from<'t>(&self, subject: Subject<'t>, start: usize) -> Option<Match<'t>> {
        // The adaptive gate decides, as for the capture methods: a seek that
        // keeps finding matches only repeats the matcher's scan up to the
        // match, so a search consults it only while it rejects often enough.
        if self.seek_rejects_search(subject.bytes(), start, subject.is_str()) {
            return None;
        }
        // A standard search reports a limit it reached as no match.
        self.find_from_limited(subject, start, None).ok().flatten()
    }

    /// The search behind the `find` family under `limits` (`None`: the
    /// process-wide limits). Returns the code of a limit the search reached.
    #[inline]
    fn find_from_limited<'t>(
        &self,
        subject: Subject<'t>,
        start: usize,
        limits: Option<SearchLimits>,
    ) -> Result<Option<Match<'t>>, i32> {
        let text = subject.bytes();
        // Only the bounds are returned: where they follow from the attempt
        // position and the match length, no region is needed (see
        // `find_iter_bytes`).
        if !self.program().keep_moves_match_start
            && !self.program().options.contains(ONIG_OPTION_FIND_LONGEST)
        {
            // Rust-only (ADR-008): a pure literal pattern skips the search state.
            // It runs no attempt that could fail, so no limit can be reached.
            if let Some(found) = pure_literal_first(
                self.program(),
                text,
                text.len(),
                start,
                text.len(),
                text.len(),
                self.program().options,
            ) {
                return Ok(found.map(|(match_start, match_end)| Match {
                    subject,
                    start: match_start,
                    end: match_end,
                }));
            }
            let mut msa = take_cached_msa(self.program(), ONIG_OPTION_NONE, None, start, limits);
            let found = onig_search_bounds(self.program(), text, start, &mut msa);
            cache_msa(msa);
            let Some((match_start, match_end)) = found? else {
                return Ok(None);
            };
            return Ok((match_end <= text.len()).then_some(Match {
                subject,
                start: match_start,
                end: match_end,
            }));
        }
        let len = text.len();
        let (result, region) = search_in_range(
            self.program(),
            text,
            len,
            start,
            len,
            len,
            Some(take_cached_region()),
            ONIG_OPTION_NONE,
            limits,
        );
        let Some(region) = region else {
            return Ok(None);
        };
        if result < 0 {
            cache_region(region);
            return if result == ONIG_MISMATCH {
                Ok(None)
            } else {
                Err(result)
            };
        }
        if region.num_regs < 1 {
            cache_region(region);
            return Ok(None);
        }
        let m = Match::from_region(subject, region.beg[0], region.end[0]);
        cache_region(region);
        Ok(m)
    }

    /// Return the first match in `text` under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached, such as
    /// [`RegexError::TimeLimitOver`], instead of treating it as "no match".
    #[inline]
    pub fn find_with<'t>(
        &self,
        text: &'t str,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        self.find_from_with(Subject::Str(text), 0, options)
    }

    /// Return the first match in `text` (as bytes) under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    #[inline]
    pub fn find_bytes_with<'t>(
        &self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        self.find_from_with(Subject::Bytes(text), 0, options)
    }

    /// Return the first match in `text` at or after byte offset `start` under
    /// per-search `options`.
    ///
    /// Searches as [`Regex::find_at`] does.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()` or is not on a char
    /// boundary of `text`.
    #[inline]
    pub fn find_at_with<'t>(
        &self,
        text: &'t str,
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        check_str_start(text, start);
        self.find_from_with(Subject::Str(text), start, options)
    }

    /// Return the first match in `text` (as bytes) at or after byte offset
    /// `start` under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    #[inline]
    pub fn find_bytes_at_with<'t>(
        &self,
        text: &'t [u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        check_bytes_start(text, start);
        self.find_from_with(Subject::Bytes(text), start, options)
    }

    fn find_from_with<'t>(
        &self,
        subject: Subject<'t>,
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        self.find_from_limited(subject, start, options.limits())
            .map_err(RegexError::from)
    }

    /// Check whether `text` matches the pattern anywhere.
    ///
    /// A search that stops at a process-wide limit (time, retry or stack) is
    /// reported as no match. Use [`Regex::is_match_with`] with [`SearchOptions`] to tell
    /// the two apart. The single-pattern seek ([`RegexBuilder::dfa`]) can
    /// report a match that the matcher reaches only without the limit.
    pub fn is_match(&self, text: &str) -> bool {
        self.is_match_from(text.as_bytes(), 0, true)
    }

    /// Check whether `text` (as bytes) matches the pattern anywhere.
    pub fn is_match_bytes(&self, text: &[u8]) -> bool {
        self.is_match_from(text, 0, false)
    }

    /// Check whether `text` has a match at or after byte offset `start`.
    ///
    /// Searches as [`Regex::find_at`] does: look-behind, `\b` and `^` see the
    /// bytes before `start`, and `\G` matches at `start`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()` or is not on a char
    /// boundary of `text`.
    pub fn is_match_at(&self, text: &str, start: usize) -> bool {
        check_str_start(text, start);
        self.is_match_from(text.as_bytes(), start, true)
    }

    /// Check whether `text` (as bytes) has a match at or after byte offset
    /// `start`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    pub fn is_match_bytes_at(&self, text: &[u8], start: usize) -> bool {
        check_bytes_start(text, start);
        self.is_match_from(text, start, false)
    }

    #[inline]
    fn is_match_from(&self, text: &[u8], start: usize, utf8: bool) -> bool {
        if let Some(answer) = self.seek_answer(text, start, utf8, true) {
            return answer;
        }
        let (result, _) = onig_search(
            self.program(),
            text,
            text.len(),
            start,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        result >= 0
    }

    /// Check whether `text` matches under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    #[inline]
    pub fn is_match_with(&self, text: &str, options: SearchOptions) -> Result<bool, RegexError> {
        self.is_match_bytes_with(text.as_bytes(), options)
    }

    /// Check whether `text` (as bytes) matches under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    #[inline]
    pub fn is_match_bytes_with(
        &self,
        text: &[u8],
        options: SearchOptions,
    ) -> Result<bool, RegexError> {
        self.is_match_from_with(text, 0, options)
    }

    /// Check whether `text` has a match at or after byte offset `start` under
    /// per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()` or is not on a char
    /// boundary of `text`.
    #[inline]
    pub fn is_match_at_with(
        &self,
        text: &str,
        start: usize,
        options: SearchOptions,
    ) -> Result<bool, RegexError> {
        check_str_start(text, start);
        self.is_match_from_with(text.as_bytes(), start, options)
    }

    /// Check whether `text` (as bytes) has a match at or after byte offset
    /// `start` under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    #[inline]
    pub fn is_match_bytes_at_with(
        &self,
        text: &[u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<bool, RegexError> {
        check_bytes_start(text, start);
        self.is_match_from_with(text, start, options)
    }

    fn is_match_from_with(
        &self,
        text: &[u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<bool, RegexError> {
        let len = text.len();
        let (result, _) = search_in_range(
            self.program(),
            text,
            len,
            start,
            len,
            len,
            None,
            ONIG_OPTION_NONE,
            options.limits(),
        );
        if result < 0 && result != ONIG_MISMATCH {
            return Err(RegexError::from(result));
        }
        Ok(result >= 0)
    }

    /// Return the first match with all capture groups, or `None`.
    ///
    /// A search that stops at a process-wide limit (time, retry or stack) is
    /// reported as no match. Use [`Regex::captures_with`] with [`SearchOptions`] to tell
    /// the two apart.
    pub fn captures<'t>(&self, text: &'t str) -> Option<Captures<'t>> {
        self.captures_from(Subject::Str(text), 0)
    }

    /// Return the first match with all capture groups (bytes), or `None`.
    pub fn captures_bytes<'t>(&self, text: &'t [u8]) -> Option<Captures<'t>> {
        self.captures_from(Subject::Bytes(text), 0)
    }

    /// Return the first match at or after byte offset `start`, with all
    /// capture groups, or `None`.
    ///
    /// Searches as [`Regex::find_at`] does. Group 0 is the match, and the
    /// offsets of every group are relative to `text`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()` or is not on a char
    /// boundary of `text`.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// let re = Regex::new(r"(?<=a)(b)(c)").unwrap();
    /// let caps = re.captures_at("abc", 1).unwrap();
    /// assert_eq!(&caps[0], "bc");
    /// assert_eq!(&caps[1], "b");
    /// assert_eq!(&caps[2], "c");
    /// ```
    pub fn captures_at<'t>(&self, text: &'t str, start: usize) -> Option<Captures<'t>> {
        check_str_start(text, start);
        self.captures_from(Subject::Str(text), start)
    }

    /// Return the first match at or after byte offset `start` (as bytes), with
    /// all capture groups, or `None`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    pub fn captures_bytes_at<'t>(&self, text: &'t [u8], start: usize) -> Option<Captures<'t>> {
        check_bytes_start(text, start);
        self.captures_from(Subject::Bytes(text), start)
    }

    /// The search behind the capture methods that report no limit: the seek
    /// answers a rejection first (see [`Regex::seek_rejects_search`]).
    #[inline]
    fn captures_from<'t>(&self, subject: Subject<'t>, start: usize) -> Option<Captures<'t>> {
        let text = subject.bytes();
        if self.seek_rejects_search(text, start, subject.is_str()) {
            return None;
        }
        let (result, region) =
            onig_search_captures(self.program(), text, start, take_cached_region());
        let region = region?;
        if result < 0 {
            cache_region(region);
            return None;
        }
        Some(Captures {
            text: subject,
            region,
            compiled: name_lookup(&self.inner),
        })
    }

    /// Return the first match with all capture groups under per-search
    /// `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    #[inline]
    pub fn captures_with<'t>(
        &self,
        text: &'t str,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        self.captures_from_with(Subject::Str(text), 0, options)
    }

    /// Return the first match with all capture groups (bytes) under
    /// per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    #[inline]
    pub fn captures_bytes_with<'t>(
        &self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        self.captures_from_with(Subject::Bytes(text), 0, options)
    }

    /// Return the first match at or after byte offset `start`, with all
    /// capture groups, under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()` or is not on a char
    /// boundary of `text`.
    #[inline]
    pub fn captures_at_with<'t>(
        &self,
        text: &'t str,
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        check_str_start(text, start);
        self.captures_from_with(Subject::Str(text), start, options)
    }

    /// Return the first match at or after byte offset `start` (as bytes), with
    /// all capture groups, under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    #[inline]
    pub fn captures_bytes_at_with<'t>(
        &self,
        text: &'t [u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        check_bytes_start(text, start);
        self.captures_from_with(Subject::Bytes(text), start, options)
    }

    fn captures_from_with<'t>(
        &self,
        subject: Subject<'t>,
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        let text = subject.bytes();
        let len = text.len();
        let (result, region) = search_in_range(
            self.program(),
            text,
            len,
            start,
            len,
            len,
            Some(take_cached_region()),
            ONIG_OPTION_NONE,
            options.limits(),
        );
        let Some(region) = region else {
            return Ok(None);
        };
        if result < 0 {
            cache_region(region);
            return if result == ONIG_MISMATCH {
                Ok(None)
            } else {
                Err(RegexError::from(result))
            };
        }
        Ok(Some(Captures {
            text: subject,
            region,
            compiled: name_lookup(&self.inner),
        }))
    }

    /// Iterate over all non-overlapping matches in `text`.
    ///
    /// Unlike the `regex` crate, this also reports an empty match right after
    /// a non-empty one; see [the guide](https://ferroni.dev/guide/coming-from-regex).
    ///
    /// Iteration ends early when a search stops at a process-wide limit. Use
    /// [`Regex::find_iter_with`] with [`SearchOptions`] to see that error.
    pub fn find_iter<'r, 't>(&'r self, text: &'t str) -> FindIter<'r, 't> {
        self.find_iter_limited(Subject::Str(text), None)
    }

    /// Iterate over all non-overlapping matches in `text` (as bytes).
    pub fn find_iter_bytes<'r, 't>(&'r self, text: &'t [u8]) -> FindIter<'r, 't> {
        self.find_iter_limited(Subject::Bytes(text), None)
    }

    /// The iterator behind the `find_iter` family under `limits` (`None`: the
    /// process-wide limits).
    fn find_iter_limited<'r, 't>(
        &'r self,
        subject: Subject<'t>,
        limits: Option<SearchLimits>,
    ) -> FindIter<'r, 't> {
        // An iterator yields match bounds only. Where they follow from the
        // attempt position and the match length, it searches without a
        // region, reusing one MatchArg for all its searches.
        let bounds_only = !self.program().keep_moves_match_start
            && !self.program().options.contains(ONIG_OPTION_FIND_LONGEST);
        FindIter {
            regex: self,
            program: self.program(),
            text: subject.bytes(),
            subject,
            cursor: MatchCursor::default(),
            region: if bounds_only {
                OnigRegion::new()
            } else {
                take_cached_region()
            },
            msa: bounds_only
                .then(|| take_cached_msa(self.program(), ONIG_OPTION_NONE, None, 0, limits)),
            limits,
        }
    }

    /// Iterate over all non-overlapping matches in `text` under per-search
    /// `options`.
    ///
    /// The options apply to each search the iterator runs. A search that
    /// reaches a limit yields its error once and ends the iteration.
    #[inline]
    pub fn find_iter_with<'r, 't>(
        &'r self,
        text: &'t str,
        options: SearchOptions,
    ) -> TryFindIter<'r, 't> {
        TryFindIter {
            iter: self.find_iter_limited(Subject::Str(text), options.limits()),
            finished: false,
        }
    }

    /// Iterate over all non-overlapping matches in `text` (as bytes) under
    /// per-search `options`.
    #[inline]
    pub fn find_iter_bytes_with<'r, 't>(
        &'r self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> TryFindIter<'r, 't> {
        TryFindIter {
            iter: self.find_iter_limited(Subject::Bytes(text), options.limits()),
            finished: false,
        }
    }

    /// Iterate over the capture groups of all non-overlapping matches in
    /// `text`.
    ///
    /// The iterator yields the matches of [`Regex::find_iter`], in the same
    /// order and with the same empty-match handling, each as a [`Captures`].
    /// Iteration ends early when a search stops at a process-wide limit. Use
    /// [`Regex::captures_iter_with`] with [`SearchOptions`] to see that error.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
    /// let years: Vec<&str> = re
    ///     .captures_iter("2025-01 and 2026-10")
    ///     .map(|caps| caps.name("year").unwrap().as_str())
    ///     .collect();
    /// assert_eq!(years, ["2025", "2026"]);
    /// ```
    pub fn captures_iter<'r, 't>(&'r self, text: &'t str) -> CaptureMatches<'r, 't> {
        CaptureMatches::new(self, Subject::Str(text), None)
    }

    /// Iterate over the capture groups of all non-overlapping matches in
    /// `text` (as bytes).
    pub fn captures_iter_bytes<'r, 't>(&'r self, text: &'t [u8]) -> CaptureMatches<'r, 't> {
        CaptureMatches::new(self, Subject::Bytes(text), None)
    }

    /// Iterate over the capture groups of all non-overlapping matches in
    /// `text` under per-search `options`.
    ///
    /// The options apply to each search the iterator runs. A search that
    /// reaches a limit yields its error once and ends the iteration.
    #[inline]
    pub fn captures_iter_with<'r, 't>(
        &'r self,
        text: &'t str,
        options: SearchOptions,
    ) -> TryCaptureMatches<'r, 't> {
        TryCaptureMatches {
            iter: CaptureMatches::new(self, Subject::Str(text), options.limits()),
            finished: false,
        }
    }

    /// Iterate over the capture groups of all non-overlapping matches in
    /// `text` (as bytes) under per-search `options`.
    #[inline]
    pub fn captures_iter_bytes_with<'r, 't>(
        &'r self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> TryCaptureMatches<'r, 't> {
        TryCaptureMatches {
            iter: CaptureMatches::new(self, Subject::Bytes(text), options.limits()),
            finished: false,
        }
    }

    /// Return the number of capture groups in the pattern (excluding group 0).
    ///
    /// Unlike the `regex` crate, group 0 is not counted; see
    /// [the guide](https://ferroni.dev/guide/coming-from-regex).
    pub fn captures_len(&self) -> usize {
        self.program().num_mem as usize
    }

    /// Findings of the compile-time check for patterns that can backtrack
    /// catastrophically, such as `(a+)+` or `(a|aa)*`.
    ///
    /// The list is empty for patterns the check does not flag. It is a
    /// heuristic: it can miss risky patterns and flag harmless ones, and it
    /// never changes how the pattern matches. Use
    /// [`RegexBuilder::reject_backtracking_risks`] to turn findings into a
    /// compile error.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// assert!(Regex::new(r"(a+)+$").unwrap().backtracking_warnings().len() == 1);
    /// assert!(Regex::new(r"(a+b)+$").unwrap().backtracking_warnings().is_empty());
    /// ```
    pub fn backtracking_warnings(&self) -> &[crate::backtrack_lint::BacktrackWarning] {
        &self.program().backtrack_warnings
    }

    /// Applied and refused decimal-loop candidates from the opt-in rewrite
    /// pass, in AST traversal order. Empty when optimization is disabled or
    /// no supported shape is recognized. This is not a safety certification.
    pub fn backtracking_rewrites(&self) -> &[crate::backtrack_rewrite::BacktrackingRewrite] {
        &self.program().backtrack_rewrites
    }

    /// Access the underlying `RegexType` for advanced / C-style usage.
    pub fn as_raw(&self) -> &RegexType {
        self.program()
    }
}

/// Shows the pattern. Bytes that are not valid UTF-8 appear as U+FFFD.
impl fmt::Debug for Regex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Regex")
            .field("pattern", &String::from_utf8_lossy(&self.inner.pattern))
            .finish_non_exhaustive()
    }
}

/// Writes the pattern. Bytes that are not valid UTF-8 appear as U+FFFD; use
/// [`Regex::as_bytes`] for the exact bytes.
impl fmt::Display for Regex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.inner.pattern))
    }
}

/// Equivalent to [`Regex::new`].
///
/// ```
/// use ferroni::api::Regex;
///
/// let re: Regex = "a+".parse().unwrap();
/// assert!(re.is_match("aaa"));
/// assert!("(".parse::<Regex>().is_err());
/// ```
impl FromStr for Regex {
    type Err = RegexError;

    fn from_str(pattern: &str) -> Result<Self, Self::Err> {
        Regex::new(pattern)
    }
}

// === RegexBuilder ===

/// Builder for compiling a [`Regex`] with custom options.
///
/// # Examples
///
/// ```
/// use ferroni::api::Regex;
///
/// let re = Regex::builder(r"hello world")
///     .case_insensitive(true)
///     .build()
///     .unwrap();
/// assert!(re.is_match("Hello World"));
/// ```
pub struct RegexBuilder {
    pattern: Vec<u8>,
    options: OnigOptionType,
    syntax: &'static OnigSyntaxType,
    reject_backtracking_risks: bool,
    optimize_backtracking: bool,
    dfa: bool,
}

impl RegexBuilder {
    /// Create a new builder for the given pattern.
    pub fn new(pattern: &str) -> Self {
        Self::new_bytes(pattern.as_bytes())
    }

    /// Create a new builder for a pattern given as raw bytes.
    ///
    /// The bytes are compiled exactly as [`Regex::new_bytes`] compiles them.
    ///
    /// ```
    /// use ferroni::api::RegexBuilder;
    ///
    /// let re = RegexBuilder::new_bytes(b"hello")
    ///     .case_insensitive(true)
    ///     .build()
    ///     .unwrap();
    /// assert!(re.is_match("HELLO"));
    /// ```
    pub fn new_bytes(pattern: &[u8]) -> Self {
        RegexBuilder {
            pattern: pattern.to_vec(),
            options: ONIG_OPTION_NONE,
            syntax: &OnigSyntaxOniguruma,
            reject_backtracking_risks: false,
            optimize_backtracking: false,
            dfa: DFA_DEFAULT,
        }
    }

    /// Enable or disable case-insensitive matching.
    pub fn case_insensitive(mut self, yes: bool) -> Self {
        if yes {
            self.options |= ONIG_OPTION_IGNORECASE;
        } else {
            self.options &= !ONIG_OPTION_IGNORECASE;
        }
        self
    }

    /// Enable or disable multiline mode (`.` matches `\n`).
    pub fn dot_matches_newline(mut self, yes: bool) -> Self {
        if yes {
            self.options |= ONIG_OPTION_MULTILINE;
        } else {
            self.options &= !ONIG_OPTION_MULTILINE;
        }
        self
    }

    /// Enable or disable `^`/`$` matching at every line boundary.
    ///
    /// With the default Oniguruma syntax, this behavior is enabled by default.
    /// When disabled, anchors match only at input boundaries. Calling this
    /// method overrides the selected syntax's default anchor behavior.
    pub fn multi_line_anchors(mut self, yes: bool) -> Self {
        if yes {
            self.options |= ONIG_OPTION_NEGATE_SINGLELINE;
            self.options &= !ONIG_OPTION_SINGLELINE;
        } else {
            self.options &= !ONIG_OPTION_NEGATE_SINGLELINE;
            self.options |= ONIG_OPTION_SINGLELINE;
        }
        self
    }

    /// Enable or disable extended mode (whitespace and `#` comments ignored).
    pub fn extended(mut self, yes: bool) -> Self {
        if yes {
            self.options |= ONIG_OPTION_EXTEND;
        } else {
            self.options &= !ONIG_OPTION_EXTEND;
        }
        self
    }

    /// Set a raw option flag. See `ONIG_OPTION_*` constants.
    ///
    /// Use [`clear_option`](Self::clear_option) to unset a flag again.
    pub fn option(mut self, flag: OnigOptionType) -> Self {
        self.options |= flag;
        self
    }

    /// Clear a raw option flag set earlier on this builder.
    ///
    /// The counterpart of [`option`](Self::option). Clearing a flag the
    /// builder does not hold has no effect.
    ///
    /// ```
    /// use ferroni::api::Regex;
    /// use ferroni::oniguruma::ONIG_OPTION_IGNORECASE;
    ///
    /// let re = Regex::builder("hello")
    ///     .option(ONIG_OPTION_IGNORECASE)
    ///     .clear_option(ONIG_OPTION_IGNORECASE)
    ///     .build()
    ///     .unwrap();
    /// assert!(!re.is_match("HELLO"));
    /// ```
    pub fn clear_option(mut self, flag: OnigOptionType) -> Self {
        self.options &= !flag;
        self
    }

    /// Capture unnamed groups even when the pattern also has named groups.
    ///
    /// `true` sets `ONIG_OPTION_CAPTURE_GROUP`, so named and unnamed groups
    /// are all captured. It also clears `ONIG_OPTION_DONT_CAPTURE_GROUP` set
    /// through [`option`](Self::option), since the two flags conflict.
    ///
    /// `false` clears `ONIG_OPTION_CAPTURE_GROUP` and leaves the choice to the
    /// syntax, which is the default. The Oniguruma, Ruby and Perl-NG syntaxes
    /// capture only the named groups of a pattern that has any; the other
    /// syntaxes capture unnamed groups as well.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// let pattern = r"(?<word>\w+) (\d+)";
    /// let named_only = Regex::builder(pattern).build().unwrap();
    /// assert_eq!(named_only.captures_len(), 1);
    ///
    /// let all = Regex::builder(pattern).capture_group(true).build().unwrap();
    /// assert_eq!(all.captures_len(), 2);
    /// ```
    pub fn capture_group(mut self, yes: bool) -> Self {
        if yes {
            self.options |= ONIG_OPTION_CAPTURE_GROUP;
            self.options &= !ONIG_OPTION_DONT_CAPTURE_GROUP;
        } else {
            self.options &= !ONIG_OPTION_CAPTURE_GROUP;
        }
        self
    }

    /// Select the syntax definition to use (default: Oniguruma).
    ///
    /// Pass one of the `OnigSyntax*` statics from [`crate::regsyntax`]. The
    /// typed alternative [`syntax_mode`](Self::syntax_mode) selects the same
    /// definitions by name.
    pub fn syntax(mut self, syntax: &'static OnigSyntaxType) -> Self {
        self.syntax = syntax;
        self
    }

    /// Select one of the built-in [`Syntax`] definitions (default: Oniguruma).
    ///
    /// This is the typed counterpart of [`syntax`](Self::syntax), which takes
    /// the `OnigSyntax*` statics. Both set the same definition, and the last
    /// call wins.
    ///
    /// ```
    /// use ferroni::api::{Regex, Syntax};
    ///
    /// // Plain text: metacharacters match only themselves.
    /// let re = Regex::builder("a.b").syntax_mode(Syntax::Asis).build().unwrap();
    /// assert!(re.is_match("a.b"));
    /// assert!(!re.is_match("axb"));
    /// ```
    pub fn syntax_mode(mut self, syntax: Syntax) -> Self {
        self.syntax = syntax.as_onig_syntax();
        self
    }

    /// Fail to compile a pattern that the backtracking check flags.
    ///
    /// By default such a pattern compiles and the findings are available
    /// from [`Regex::backtracking_warnings`]. With this set, `build` returns
    /// `ONIGERR_VERY_INEFFICIENT_PATTERN` instead. The check is a heuristic
    /// and may flag harmless patterns; leave this off to keep them.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// assert!(Regex::builder(r"(a+)+$").reject_backtracking_risks(true).build().is_err());
    /// assert!(Regex::builder(r"(a+)+$").build().is_ok());
    /// ```
    pub fn reject_backtracking_risks(mut self, yes: bool) -> Self {
        self.reject_backtracking_risks = yes;
        self
    }

    /// Enable conservative AST rewrites for backtracking-prone patterns.
    ///
    /// Default: off. The decimal rule recognizes greedy `(?:[0-9]+_?)+`
    /// immediately before a mandatory literal dot. Primitive digit runs use
    /// possessive bytecode; captured primitives use an outer atomic group.
    /// A greedy `(?:[0-9]+_?)*[0-9]+` before a word boundary uses a
    /// deterministic prefix when captures wrap only the repeated body. Other
    /// capture placements keep the complete pair atomic and its give-back
    /// choices. Both rules retain all groups and successful results. Unsupported
    /// constructs stay unchanged. Retry, stack, and time-limit outcomes may
    /// differ because the optimized matcher performs less work. This does
    /// not guarantee linear-time unanchored searches.
    ///
    /// See [`Regex::backtracking_rewrites`] for applied and refused candidates.
    /// Original lint warnings remain available, and risk rejection still
    /// rejects the original pattern even when a rewrite would apply.
    pub fn optimize_backtracking(mut self, yes: bool) -> Self {
        self.optimize_backtracking = yes;
        self
    }

    /// Enable or disable the single-pattern seek automaton for standard
    /// searches (ADR-008).
    ///
    /// The default is on when the `dfa-prefilter` feature is enabled, which
    /// it is by default, and off otherwise. With `true`, [`RegexBuilder::build`]
    /// derives the seek from the compiled tree and builds its lazy DFA in the
    /// same compilation, so the cost is paid once, and the clones and searches
    /// of the returned [`Regex`] share the automaton. A standard `is_match`
    /// asks the automaton first. The automaton answers "no match" whenever
    /// it can, and answers "match" for an exact seek's match, which is the
    /// matcher's match with no limit. A pattern whose classes have
    /// multi-character case folds (`(?i)[a-zß]`) and whose seek is not exact
    /// may get a second automaton, an accept seek, whose matches are matches
    /// of the pattern: `is_match` answers "match" from it, and "no match"
    /// on a text that holds none of the strings the folds add (`ss`, `ff`,
    /// or any non-ASCII character). Where a process-wide limit would stop
    /// the matcher, `is_match` can therefore report the match that the
    /// matcher finds without the limit (ADR-017). In every other case the
    /// matcher answers, so the results, including the limit errors of
    /// [`Regex::find_with`] and the other `*_with` methods, are the same with
    /// the option on or off.
    ///
    /// The cost is compile time and memory. The seek is derived in the same
    /// compilation as the program, so `build` does not compile the pattern
    /// twice, but it does build the automaton: the compile time of a pattern
    /// with the option on is higher than with it off by the time of that
    /// build. The automaton's NFA is at most 16 MiB, and the seek is absent
    /// for a pattern too large for it. Each thread that searches with the
    /// regex keeps a lazy DFA cache of at most 1 MiB for the seek, and one
    /// more for an accept seek.
    ///
    /// Use `dfa(false)` for patterns that are compiled once and searched
    /// rarely, or in a memory-constrained process; the results do not change
    /// except as the paragraph above describes.
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// // The automaton is built with the regex (the default with the feature).
    /// let with = Regex::builder(r"\d{4}-\d{2}").dfa(true).build().unwrap();
    /// // Without it, the matcher answers every search.
    /// let without = Regex::builder(r"\d{4}-\d{2}").dfa(false).build().unwrap();
    /// assert_eq!(with.find("on 2026-10").unwrap().as_str(), "2026-10");
    /// assert_eq!(
    ///     with.find("on 2026-10").map(|m| m.range()),
    ///     without.find("on 2026-10").map(|m| m.range()),
    /// );
    /// ```
    pub fn dfa(mut self, yes: bool) -> Self {
        self.dfa = yes;
        self
    }

    /// Compile the pattern into a [`Regex`].
    pub fn build(self) -> Result<Regex, RegexError> {
        let (inner, seek, accept) = onig_new_for_single_seek(
            &self.pattern,
            self.options,
            &ONIG_ENCODING_UTF8,
            self.syntax,
            self.optimize_backtracking,
            self.dfa,
        )?;
        if self.reject_backtracking_risks && !inner.backtrack_warnings.is_empty() {
            return Err(ONIGERR_VERY_INEFFICIENT_PATTERN.into());
        }
        Ok(Regex::from_parts(
            inner,
            self.pattern.into_boxed_slice(),
            seek,
            accept,
        ))
    }
}

/// Shows the pattern, the options and the settings. `syntax` is `None` when
/// the syntax came from [`RegexBuilder::syntax`] and is not a built-in one.
impl fmt::Debug for RegexBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Kept in step with the variants of `Syntax`.
        const BUILT_IN: [Syntax; 12] = [
            Syntax::Oniguruma,
            Syntax::Asis,
            Syntax::PosixBasic,
            Syntax::PosixExtended,
            Syntax::Emacs,
            Syntax::Grep,
            Syntax::GnuRegex,
            Syntax::Java,
            Syntax::Perl,
            Syntax::PerlNg,
            Syntax::Ruby,
            Syntax::Python,
        ];
        let syntax = BUILT_IN
            .into_iter()
            .find(|syntax| std::ptr::eq(syntax.as_onig_syntax(), self.syntax));
        f.debug_struct("RegexBuilder")
            .field("pattern", &String::from_utf8_lossy(&self.pattern))
            .field("options", &self.options)
            .field("syntax", &syntax)
            .field("reject_backtracking_risks", &self.reject_backtracking_risks)
            .field("optimize_backtracking", &self.optimize_backtracking)
            .field("dfa", &self.dfa)
            .finish()
    }
}

impl Regex {
    /// Create a [`RegexBuilder`] for a pattern given as raw bytes.
    ///
    /// The byte form of [`Regex::builder`]; see [`RegexBuilder::new_bytes`].
    ///
    /// ```
    /// use ferroni::api::Regex;
    ///
    /// let re = Regex::builder_bytes(br"\d+").build().unwrap();
    /// assert_eq!(re.find("abc 42").unwrap().as_str(), "42");
    /// ```
    pub fn builder_bytes(pattern: &[u8]) -> RegexBuilder {
        RegexBuilder::new_bytes(pattern)
    }
}

/// A built-in regex syntax definition.
///
/// A typed alternative to the `OnigSyntax*` statics in [`crate::regsyntax`],
/// accepted by [`RegexBuilder::syntax_mode`] and used by the scanner as
/// [`ScannerSyntax`](crate::scanner::ScannerSyntax). The variants match the
/// scanner's syntax choices, which follow vscode-oniguruma's `Syntax` enum.
///
/// The default is [`Syntax::Oniguruma`]. The enum is `#[non_exhaustive]`, so a
/// new syntax can be added without a breaking change; a `match` outside this
/// crate needs a wildcard arm.
///
/// ```
/// use ferroni::api::{Regex, Syntax};
///
/// assert_eq!(Syntax::default(), Syntax::Oniguruma);
/// let re = Regex::builder(r"\w+").syntax_mode(Syntax::Ruby).build().unwrap();
/// assert!(re.is_match("ok"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Syntax {
    /// Oniguruma syntax (default).
    #[default]
    Oniguruma,
    /// Plain text, no metacharacters.
    Asis,
    /// POSIX Basic Regular Expressions.
    PosixBasic,
    /// POSIX Extended Regular Expressions.
    PosixExtended,
    /// Emacs regex syntax.
    Emacs,
    /// grep syntax.
    Grep,
    /// GNU regex syntax.
    GnuRegex,
    /// Java regex syntax.
    Java,
    /// Perl regex syntax.
    Perl,
    /// Perl-NG regex syntax.
    PerlNg,
    /// Ruby regex syntax.
    Ruby,
    /// Python regex syntax.
    Python,
}

impl Syntax {
    pub(crate) fn as_onig_syntax(&self) -> &'static OnigSyntaxType {
        match self {
            Self::Oniguruma => &OnigSyntaxOniguruma,
            Self::Asis => &OnigSyntaxASIS,
            Self::PosixBasic => &OnigSyntaxPosixBasic,
            Self::PosixExtended => &OnigSyntaxPosixExtended,
            Self::Emacs => &OnigSyntaxEmacs,
            Self::Grep => &OnigSyntaxGrep,
            Self::GnuRegex => &OnigSyntaxGnuRegex,
            Self::Java => &OnigSyntaxJava,
            Self::Perl => &OnigSyntaxPerl,
            Self::PerlNg => &OnigSyntaxPerl_NG,
            Self::Ruby => &OnigSyntaxRuby,
            Self::Python => &OnigSyntaxPython,
        }
    }
}

// === Subject ===

/// The text of a search as the caller passed it. A `&str` keeps its type, so
/// that [`Match::as_str`] slices it at the match's bounds, two character
/// boundary checks, instead of validating the matched bytes as UTF-8 again on
/// every call.
#[derive(Clone, Copy)]
enum Subject<'t> {
    Str(&'t str),
    Bytes(&'t [u8]),
}

impl<'t> Subject<'t> {
    #[inline(always)]
    fn bytes(self) -> &'t [u8] {
        match self {
            Subject::Str(text) => text.as_bytes(),
            Subject::Bytes(text) => text,
        }
    }

    /// Whether the text is known to be valid UTF-8.
    #[inline(always)]
    fn is_str(self) -> bool {
        matches!(self, Subject::Str(_))
    }
}

// === Match ===

/// A single match result referencing the original text.
#[derive(Clone, Copy)]
pub struct Match<'t> {
    subject: Subject<'t>,
    start: usize,
    end: usize,
}

impl fmt::Debug for Match<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The layout a derived `Debug` printed before `Subject`.
        f.debug_struct("Match")
            .field("text", &self.subject.bytes())
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

impl<'t> Match<'t> {
    /// Build a match from one region entry, or `None` if the entry is not a
    /// range of `text`: unset (negative), past the end, or with start > end.
    /// Oniguruma can report start > end for a capture whose group started
    /// again but failed before closing; every `Match` upholds
    /// `start <= end <= text.len()`, so its accessors never panic on slicing.
    fn from_region(subject: Subject<'t>, beg: i32, end: i32) -> Option<Self> {
        let start = usize::try_from(beg).ok()?;
        let end = usize::try_from(end).ok()?;
        (start <= end && end <= subject.bytes().len()).then_some(Match {
            subject,
            start,
            end,
        })
    }

    /// Byte offset of the start of the match.
    pub fn start(&self) -> usize {
        self.start
    }

    /// Byte offset of the end of the match (exclusive).
    pub fn end(&self) -> usize {
        self.end
    }

    /// Byte range of the match.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// The matched text as a byte slice.
    pub fn as_bytes(&self) -> &'t [u8] {
        &self.subject.bytes()[self.start..self.end]
    }

    /// The matched text as a `&str`.
    ///
    /// Use [`Match::as_bytes`] when the match may be invalid UTF-8; see
    /// [the guide](https://ferroni.dev/guide/coming-from-regex).
    ///
    /// A match of a `&str` search costs two character-boundary checks; a match
    /// of a `_bytes` search validates its bytes.
    ///
    /// # Panics
    ///
    /// Panics if the matched bytes are not valid UTF-8.
    pub fn as_str(&self) -> &'t str {
        match self.subject {
            // Valid UTF-8 already: the slice is valid exactly when both bounds
            // are character boundaries, which `get` checks.
            Subject::Str(text) => text.get(self.start..self.end),
            Subject::Bytes(text) => std::str::from_utf8(&text[self.start..self.end]).ok(),
        }
        .expect("match is not valid UTF-8")
    }

    /// Returns the length of the match in bytes.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns `true` if the match is empty (zero-length).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

// === Captures ===

/// All capture groups from a single match.
///
/// Group 0 is the entire match. Groups 1..N correspond to `(...)` in the pattern.
///
/// A `Captures` borrows only the haystack, not the [`Regex`] that produced
/// it, so it can be returned from a function that received the regex by
/// reference. It keeps a shared handle to the compiled program for name
/// lookups, so creating one costs an atomic increment and no allocation.
///
/// # Examples
///
/// ```
/// use ferroni::api::Regex;
///
/// fn year<'h>(re: &Regex, text: &'h str) -> Option<&'h str> {
///     re.captures(text)?.name("year").map(|m| m.as_str())
/// }
///
/// let re = Regex::new(r"(?<year>\d{4})-\d{2}").unwrap();
/// assert_eq!(year(&re, "published 2026-10"), Some("2026"));
/// ```
pub struct Captures<'t> {
    text: Subject<'t>,
    region: OnigRegion,
    /// The compiled regex, kept only for [`Captures::name`] and so only when
    /// the pattern has named groups. Cloning the `Arc` for every match writes
    /// its shared reference count, which serializes threads that search with
    /// the same regex; see [`name_lookup`].
    compiled: Option<Arc<Compiled>>,
}

/// The compiled regex a [`Captures`] keeps for name lookups: a clone of
/// `inner` when the pattern has named groups, `None` otherwise.
///
/// Every `Arc` clone and drop is an atomic write to one reference count. When
/// threads search with the same regex, each match moved that cache line
/// between their cores, and `captures_iter` stopped scaling at about four
/// threads. Patterns without named groups now leave the count alone; with
/// named groups the clone remains, because [`Captures::name`] needs the
/// compiled name table and `Captures` does not borrow the regex.
fn name_lookup(inner: &Arc<Compiled>) -> Option<Arc<Compiled>> {
    (crate::regexec::onig_number_of_names(&inner.program) > 0).then(|| Arc::clone(inner))
}

impl<'t> Captures<'t> {
    /// Get capture group `i`, or `None` if the group did not participate.
    ///
    /// Group 0 is the entire match.
    ///
    /// Like Oniguruma, the engine can leave a capture with its start after
    /// its end: a group that matched once, then started again and failed
    /// before closing, keeps the new start and the old end (for example
    /// group 1 of `((?=(a|ab))a?){2}` against `"a"` is `1..0`). Such a
    /// capture is not a range of the text and is reported as not
    /// participating (`None`), here and in [`Captures::iter`] and
    /// [`Captures::name`]. The raw values stay available through the
    /// low-level [`OnigRegion`].
    pub fn get(&self, i: usize) -> Option<Match<'t>> {
        if i >= self.region.num_regs as usize {
            return None;
        }
        Match::from_region(self.text, self.region.beg[i], self.region.end[i])
    }

    /// Get the last capture group with the given name that participated, or `None`.
    pub fn name(&self, name: &str) -> Option<Match<'t>> {
        // Without named groups there is no compiled regex to ask, and no name
        // it could resolve.
        let compiled = self.compiled.as_deref()?;
        let num =
            onig_name_to_backref_number(&compiled.program, name.as_bytes(), Some(&self.region))
                .ok()?;
        self.get(num as usize)
    }

    /// Number of capture groups (including group 0).
    pub fn len(&self) -> usize {
        self.region.num_regs as usize
    }

    /// Returns `true` if there are no capture groups (should never happen for a valid match).
    pub fn is_empty(&self) -> bool {
        self.region.num_regs == 0
    }

    /// Iterate over all capture groups.
    pub fn iter(&self) -> CapturesIter<'_, 't> {
        CapturesIter {
            captures: self,
            index: 0,
        }
    }
}

/// Returns the text of group `i`.
///
/// # Panics
///
/// Panics if the pattern has no group `i`, if group `i` did not participate
/// in the match, or if its bytes are not valid UTF-8 (captures from the
/// `_bytes` methods). [`Captures::get`] reports the first two cases as `None`.
///
/// ```
/// use ferroni::api::Regex;
///
/// let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
/// let caps = re.captures("2026-10").unwrap();
/// assert_eq!(&caps[1], "2026");
/// assert_eq!(&caps[2], "10");
/// ```
impl Index<usize> for Captures<'_> {
    type Output = str;

    fn index(&self, i: usize) -> &str {
        match self.get(i) {
            Some(m) => m.as_str(),
            None => panic!("no group at index '{i}'"),
        }
    }
}

/// Returns the text of the last participating group named `name`, as
/// [`Captures::name`] does.
///
/// # Panics
///
/// Panics if no group has that name, if no group with that name participated
/// in the match, or if its bytes are not valid UTF-8 (captures from the
/// `_bytes` methods). [`Captures::name`] reports these cases as `None`.
///
/// ```
/// use ferroni::api::Regex;
///
/// let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
/// let caps = re.captures("2026-10").unwrap();
/// assert_eq!(&caps["year"], "2026");
/// ```
impl<'n> Index<&'n str> for Captures<'_> {
    type Output = str;

    fn index(&self, name: &'n str) -> &str {
        match self.name(name) {
            Some(m) => m.as_str(),
            None => panic!("no group named '{name}'"),
        }
    }
}

impl fmt::Debug for Captures<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for i in 0..self.len() {
            list.entry(&self.get(i));
        }
        list.finish()
    }
}

impl Drop for Captures<'_> {
    fn drop(&mut self) {
        cache_region(std::mem::take(&mut self.region));
    }
}

// === CapturesIter ===

/// Iterator over capture groups in a [`Captures`].
pub struct CapturesIter<'c, 't> {
    captures: &'c Captures<'t>,
    index: usize,
}

impl<'c, 't> Iterator for CapturesIter<'c, 't> {
    type Item = Option<Match<'t>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.captures.len() {
            return None;
        }
        let m = self.captures.get(self.index);
        self.index += 1;
        Some(m)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.captures.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for CapturesIter<'_, '_> {}

impl fmt::Debug for CapturesIter<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapturesIter").finish_non_exhaustive()
    }
}

// === CaptureNames ===

/// Iterator over the names of the capture groups, from
/// [`Regex::capture_names`].
pub struct CaptureNames<'r> {
    table: Option<&'r NameTable>,
    next_group: i32,
    end_group: i32,
}

impl<'r> CaptureNames<'r> {
    /// The name of `group`, or `None` if it has none. A name that is not
    /// valid UTF-8 is reported as absent, because `&str` cannot hold it.
    fn name_of(&self, group: i32) -> Option<&'r str> {
        let table = self.table?;
        let entry = table
            .entries
            .values()
            .find(|entry| entry.back_refs.contains(&group))?;
        std::str::from_utf8(&entry.name).ok()
    }
}

impl<'r> Iterator for CaptureNames<'r> {
    type Item = Option<&'r str>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_group >= self.end_group {
            return None;
        }
        let group = self.next_group;
        self.next_group += 1;
        Some(self.name_of(group))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.end_group - self.next_group) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for CaptureNames<'_> {}

impl std::iter::FusedIterator for CaptureNames<'_> {}

impl fmt::Debug for CaptureNames<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureNames").finish_non_exhaustive()
    }
}

// === MatchCursor ===

/// Where a match iterator searches next, and whether its previous match was
/// empty. Shared by every iterator that yields matches, so they all apply the
/// same empty-match rule. A `last_end` past the end of the text ends the
/// iteration.
#[derive(Clone, Copy, Debug, Default)]
struct MatchCursor {
    last_end: usize,
    last_was_empty: bool,
}

/// What an iterator does with a match its search has found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// Yield the match.
    Yield,
    /// Search again from the cursor, which moved one character on.
    Retry,
    /// End the iteration.
    Finish,
}

impl MatchCursor {
    /// Apply the empty-match rule to the match `start..end` that a search from
    /// the cursor found. An empty match directly after another empty match is
    /// skipped: the cursor moves one character on, and the search runs again.
    /// That keeps the iteration moving without yielding the same empty match
    /// twice.
    ///
    /// Where the program allows it (`skip_repeated_empty_search`), the step
    /// past an empty match is taken with the match: the search from its end
    /// would only find that match again, so the iterator does not run it
    /// (Rust-only, ADR-008). The rule is the same either way: the next
    /// search starts one character on, after an empty match.
    #[inline]
    fn accept(&mut self, program: &RegexType, text: &[u8], start: usize, end: usize) -> Verdict {
        if start == end {
            if self.last_was_empty {
                if self.last_end >= text.len() {
                    return Verdict::Finish;
                }
                // Skip one character to avoid infinite loop on empty match
                self.last_end += program.enc.mbc_enc_len(&text[self.last_end..]);
                self.last_was_empty = false;
                return Verdict::Retry;
            }
            self.last_was_empty = true;
        } else {
            self.last_was_empty = false;
        }

        self.last_end = end;
        if start == end && program.skip_repeated_empty_search {
            if end >= text.len() {
                // Nothing follows: the search from the end would find this
                // empty match again and finish the iteration.
                self.last_end = text.len() + 1;
            } else {
                self.last_end += program.enc.mbc_enc_len(&text[end..]);
                self.last_was_empty = false;
            }
        }
        Verdict::Yield
    }
}

// === FindIter ===

/// Iterator over all non-overlapping matches in a text.
pub struct FindIter<'r, 't> {
    regex: &'r Regex,
    /// `regex`'s program, read once so that each search does not deref it.
    program: &'r RegexType,
    text: &'t [u8],
    /// `text` as the caller passed it, for the matches.
    subject: Subject<'t>,
    cursor: MatchCursor,
    region: OnigRegion,
    /// Set when the searches need no region (`onig_search_bounds`).
    msa: Option<Box<MatchArg>>,
    /// The limits of each search; `None` keeps the process-wide ones.
    limits: Option<SearchLimits>,
}

impl<'r, 't> FindIter<'r, 't> {
    /// The next match, or the code of a limit that a search reached.
    fn try_next(&mut self) -> Result<Option<Match<'t>>, i32> {
        loop {
            if self.cursor.last_end > self.text.len() {
                return Ok(None);
            }
            let program = self.program;
            let m = if let Some(msa) = self.msa.as_deref_mut() {
                debug_assert!(msa.region.is_none(), "bounds-only searches keep no region");
                msa.reset_for_iteration(program, self.cursor.last_end, self.limits);
                let Some((start, end)) =
                    onig_search_bounds(program, self.text, self.cursor.last_end, msa)?
                else {
                    return Ok(None);
                };
                if end > self.text.len() {
                    return Ok(None);
                }
                Match {
                    subject: self.subject,
                    start,
                    end,
                }
            } else {
                let len = self.text.len();
                let (result, region) = search_in_range(
                    program,
                    self.text,
                    len,
                    self.cursor.last_end,
                    len,
                    len,
                    Some(std::mem::take(&mut self.region)),
                    ONIG_OPTION_NONE,
                    self.limits,
                );
                let Some(region) = region else {
                    return Ok(None);
                };
                self.region = region;

                if result < 0 {
                    return if result == ONIG_MISMATCH {
                        Ok(None)
                    } else {
                        Err(result)
                    };
                }
                if self.region.num_regs < 1 {
                    return Ok(None);
                }

                let Some(m) =
                    Match::from_region(self.subject, self.region.beg[0], self.region.end[0])
                else {
                    return Ok(None);
                };
                m
            };

            match self.cursor.accept(program, self.text, m.start, m.end) {
                Verdict::Yield => return Ok(Some(m)),
                Verdict::Retry => continue,
                Verdict::Finish => return Ok(None),
            }
        }
    }
}

impl<'r, 't> Iterator for FindIter<'r, 't> {
    type Item = Match<'t>;

    fn next(&mut self) -> Option<Match<'t>> {
        // A standard iteration reports a limit as the end of the matches.
        self.try_next().ok().flatten()
    }
}

impl Drop for FindIter<'_, '_> {
    fn drop(&mut self) {
        match self.msa.take() {
            Some(msa) => cache_msa(msa),
            None => cache_region(std::mem::take(&mut self.region)),
        }
    }
}

impl fmt::Debug for FindIter<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FindIter").finish_non_exhaustive()
    }
}

/// Iterator over matches that applies [`SearchOptions`] to each search.
///
/// Created by [`Regex::find_iter_with`]. A search that reaches a limit is
/// yielded once as `Err` and ends the iterator.
pub struct TryFindIter<'r, 't> {
    iter: FindIter<'r, 't>,
    finished: bool,
}

impl<'r, 't> Iterator for TryFindIter<'r, 't> {
    type Item = Result<Match<'t>, RegexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.iter.try_next() {
            Ok(Some(m)) => Some(Ok(m)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(code) => {
                self.finished = true;
                Some(Err(RegexError::from(code)))
            }
        }
    }
}

impl std::iter::FusedIterator for TryFindIter<'_, '_> {}

impl fmt::Debug for TryFindIter<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryFindIter").finish_non_exhaustive()
    }
}

// === CaptureMatches ===

/// Iterator over the capture groups of all non-overlapping matches in a text.
///
/// Created by [`Regex::captures_iter`] and [`Regex::captures_iter_bytes`]. It
/// yields the matches of [`FindIter`], in the same order, each as a
/// [`Captures`].
pub struct CaptureMatches<'r, 't> {
    regex: &'r Regex,
    /// `regex`'s program, read once so that each search does not deref it.
    program: &'r RegexType,
    text: &'t [u8],
    /// `text` as the caller passed it, for the captures.
    subject: Subject<'t>,
    cursor: MatchCursor,
    /// Rust-only (ADR-008): the search state of every search of this
    /// iterator, kept from one search to the next. Between searches it holds
    /// the region of the next search, which a yielded [`Captures`] takes. It
    /// returns to the thread cache when the iterator is dropped.
    msa: Option<Box<MatchArg>>,
    /// The limits of each search; `None` keeps the process-wide ones.
    limits: Option<SearchLimits>,
}

impl<'r, 't> CaptureMatches<'r, 't> {
    fn new(regex: &'r Regex, subject: Subject<'t>, limits: Option<SearchLimits>) -> Self {
        let program = regex.program();
        CaptureMatches {
            regex,
            program,
            text: subject.bytes(),
            subject,
            cursor: MatchCursor::default(),
            msa: Some(take_cached_msa(program, ONIG_OPTION_NONE, None, 0, limits)),
            limits,
        }
    }

    /// The region for the next search. A yielded [`Captures`] takes this
    /// iterator's region, and returns it to the thread cache when it is
    /// dropped, so the search takes the cached region then, instead of
    /// allocating a new one (Rust-only, ADR-008).
    fn next_region(&mut self) -> OnigRegion {
        let region = self
            .msa
            .as_deref_mut()
            .and_then(|msa| msa.region.take())
            .unwrap_or_default();
        if region.beg.capacity() == 0 {
            take_cached_region()
        } else {
            region
        }
    }

    /// The next match with its captures, or the code of a limit that a search
    /// reached.
    fn try_next(&mut self) -> Result<Option<Captures<'t>>, i32> {
        loop {
            if self.cursor.last_end > self.text.len() {
                return Ok(None);
            }

            let program = self.program;
            let len = self.text.len();
            let start = self.cursor.last_end;
            let region = self.next_region();
            let Some(msa) = self.msa.as_deref_mut() else {
                return Ok(None);
            };
            // The same search as `search_in_range` runs with a cached
            // `MatchArg`, which this iterator keeps instead (Rust-only, ADR-008).
            msa.reset_for_iteration(program, start, self.limits);
            msa.region = Some(region);
            let result = search_in_range_inner(program, self.text, len, start, len, len, msa);
            let region = msa.region.take();
            release_oversized_stack(msa);
            let Some(region) = region else {
                return Ok(None);
            };
            if result < 0 {
                msa.region = Some(region);
                return if result == ONIG_MISMATCH {
                    Ok(None)
                } else {
                    Err(result)
                };
            }
            if region.num_regs < 1 {
                msa.region = Some(region);
                return Ok(None);
            }
            let Some(m) = Match::from_region(self.subject, region.beg[0], region.end[0]) else {
                msa.region = Some(region);
                return Ok(None);
            };

            match self.cursor.accept(program, self.text, m.start, m.end) {
                Verdict::Yield => {
                    return Ok(Some(Captures {
                        text: self.subject,
                        region,
                        compiled: name_lookup(&self.regex.inner),
                    }));
                }
                Verdict::Retry => msa.region = Some(region),
                Verdict::Finish => {
                    msa.region = Some(region);
                    return Ok(None);
                }
            }
        }
    }
}

impl<'r, 't> Iterator for CaptureMatches<'r, 't> {
    type Item = Captures<'t>;

    fn next(&mut self) -> Option<Captures<'t>> {
        // A standard iteration reports a limit as the end of the matches.
        self.try_next().ok().flatten()
    }
}

impl Drop for CaptureMatches<'_, '_> {
    fn drop(&mut self) {
        if let Some(mut msa) = self.msa.take() {
            cache_region(msa.region.take().unwrap_or_default());
            cache_msa(msa);
        }
    }
}

impl fmt::Debug for CaptureMatches<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureMatches").finish_non_exhaustive()
    }
}

/// Iterator over the capture groups of matches that applies [`SearchOptions`]
/// to each search.
///
/// Created by [`Regex::captures_iter_with`]. A search that reaches a limit is
/// yielded once as `Err` and ends the iterator.
pub struct TryCaptureMatches<'r, 't> {
    iter: CaptureMatches<'r, 't>,
    finished: bool,
}

impl<'r, 't> Iterator for TryCaptureMatches<'r, 't> {
    type Item = Result<Captures<'t>, RegexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.iter.try_next() {
            Ok(Some(captures)) => Some(Ok(captures)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(code) => {
                self.finished = true;
                Some(Err(RegexError::from(code)))
            }
        }
    }
}

impl std::iter::FusedIterator for TryCaptureMatches<'_, '_> {}

impl fmt::Debug for TryCaptureMatches<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryCaptureMatches").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    fn clear_cached_region() {
        CACHED_REGION.with(|cached| *cached.borrow_mut() = None);
    }

    fn cached_region_buffer() -> Option<(*const i32, usize)> {
        CACHED_REGION.with(|cached| {
            cached
                .borrow()
                .as_ref()
                .map(|region| (region.beg.as_ptr(), region.beg.capacity()))
        })
    }

    /// The seek automaton is built with the regex, once per compilation and
    /// shared by its clones, for a pattern with a seek that can reject; a
    /// pure literal and a seek that matches everywhere have none. The option
    /// turns it off.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn seek_is_built_with_the_regex() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"^[0-9A-F]{8}$").unwrap();
        assert!(re.seek_present());
        assert_eq!(re.seek_exact(), Some(true));
        assert!(re.clone().seek_present());
        assert!(!re.is_match("zzzz"));

        // Case folding that reaches non-ASCII characters (the Kelvin sign for
        // `k`) is listed exactly, so the seek stays exact.
        let folded = RegexBuilder::new("^k+$")
            .case_insensitive(true)
            .build()
            .unwrap();
        assert_eq!(folded.seek_exact(), Some(true));
        assert!(!folded.is_match("xxx"));
        assert!(folded.is_match("\u{212A}k"));

        // A pure literal is answered by its own search, and a seek that
        // matches everywhere rejects nothing.
        for pattern in ["GET", "a*"] {
            let re = Regex::new(pattern).unwrap();
            assert!(!re.seek_present(), "{pattern}");
            assert!(re.is_match("GET a"), "{pattern}");
        }

        let off = RegexBuilder::new(r"^[0-9A-F]{8}$")
            .dfa(false)
            .build()
            .unwrap();
        assert!(!off.seek_present());
        assert!(off.is_match("0123ABCD"));
    }

    /// An exact seek answers "match" for unbounded repetitions, which have no
    /// static retry bound: the seek decides, so the matcher is not run, and
    /// its answer is the matcher's without limits. A rejection holds as well.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn exact_seek_answers_match_for_an_unbounded_repetition() {
        let _limits = crate::regexec::shared_limits();
        let pattern = r"^[a-z@.]+@[a-z.]+\.com$";
        let re = Regex::new(pattern).unwrap();
        let matcher = RegexBuilder::new(pattern).dfa(false).build().unwrap();
        assert_eq!(re.seek_exact(), Some(true));
        assert_eq!(re.seek_decides_is_match("john@doe.com"), Some(true));
        assert_eq!(re.seek_decides_is_match("john@doe.org"), Some(false));
        assert!(re.is_match("john@doe.com"));
        assert!(matcher.is_match("john@doe.com"));
        assert!(!re.is_match("john@doe.org"));
    }

    /// The repeats `tune_next` makes possessive keep the language of the
    /// repeat, so the seek of `^[a-z]+@[a-z]+\.com$` is exact and answers
    /// "match" for it. A user-written atomic group next to them is not: its
    /// seek stays inexact and answers "match" only from the matcher.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn automatic_possessive_repeats_keep_the_seek_exact() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"^[a-z]+@[a-z]+\.com$").unwrap();
        assert_eq!(re.seek_exact(), Some(true));
        assert_eq!(re.seek_decides_is_match("john@doe.com"), Some(true));
        assert_eq!(re.seek_decides_is_match("john@doe.org"), Some(false));
        assert!(re.is_match("john@doe.com"));
        assert!(!re.is_match("john@doe.org"));

        let atomic = Regex::new(r"^[a-z]+@(?>[a-z]+)\.com$").unwrap();
        assert_eq!(atomic.seek_exact(), Some(false));
        assert_eq!(atomic.seek_decides_is_match("john@doe.com"), None);
        assert_eq!(atomic.seek_decides_is_match("john@doe.org"), Some(false));
        assert!(atomic.is_match("john@doe.com"));
    }

    /// Where a tiny process-wide retry limit stops the matcher, an exact seek
    /// still answers "match", and the answer is the matcher's without the
    /// limit (ADR-017, rule 2). The limit is restored when the guard drops.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn exact_seek_answers_where_a_tiny_retry_limit_stops_the_matcher() {
        let _limits = crate::regexec::exclusive_limits();
        let re = Regex::new(r"^[a-z]+ab\z").unwrap();
        let text = "xaaab";
        assert_eq!(re.seek_exact(), Some(true));
        crate::regexec::onig_set_retry_limit_in_match(1);
        // The premise: under the limit the matcher stops on this subject.
        assert!(
            re.is_match_bytes_at_with(text.as_bytes(), 0, SearchOptions::new())
                .is_err()
        );
        assert_eq!(re.seek_decides_is_match(text), Some(true));
        assert!(re.is_match(text));
        // The matcher without the limit finds the same match.
        crate::regexec::onig_set_retry_limit_in_match(0);
        assert!(
            re.is_match_bytes_at_with(text.as_bytes(), 0, SearchOptions::new())
                .unwrap()
        );
        // A subject without the match stays a rejection under the limit.
        crate::regexec::onig_set_retry_limit_in_match(1);
        assert!(!re.is_match("xaaac"));
    }

    /// The catalog's validators have exact seeks, so a standard `is_match`
    /// answers their matches from the seek. The django local part has
    /// unbounded repeats, and the IPv6 validator a zone suffix with no bound.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn catalog_validators_have_exact_seeks() {
        let _limits = crate::regexec::shared_limits();
        let cases: &[(&str, &str, bool, &str)] = &[
            (
                "uuid_shape",
                "^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$",
                true,
                "0123ABCD-0123-0123-0123-0123456789AB",
            ),
            (
                "uuid_v4",
                "^[0-9A-F]{8}-[0-9A-F]{4}-4[0-9A-F]{3}-[89AB][0-9A-F]{3}-[0-9A-F]{12}$",
                true,
                "0123ABCD-0123-4123-8123-0123456789AB",
            ),
            (
                "ipv4_validation",
                "^((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])$",
                false,
                "192.168.0.1",
            ),
            (
                "ipv6_validation",
                "^((?:(?:[0-9a-fA-F]{1,4}):){7}(?:(?:[0-9a-fA-F]{1,4})|:)|(?:(?:[0-9a-fA-F]{1,4}):){6}(?:((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])|:(?:[0-9a-fA-F]{1,4})|:)|(?:(?:[0-9a-fA-F]{1,4}):){5}(?::((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])|(:(?:[0-9a-fA-F]{1,4})){1,2}|:)|(?:(?:[0-9a-fA-F]{1,4}):){4}(?:(:(?:[0-9a-fA-F]{1,4})){0,1}:((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])|(:(?:[0-9a-fA-F]{1,4})){1,3}|:)|(?:(?:[0-9a-fA-F]{1,4}):){3}(?:(:(?:[0-9a-fA-F]{1,4})){0,2}:((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])|(:(?:[0-9a-fA-F]{1,4})){1,4}|:)|(?:(?:[0-9a-fA-F]{1,4}):){2}(?:(:(?:[0-9a-fA-F]{1,4})){0,3}:((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])|(:(?:[0-9a-fA-F]{1,4})){1,5}|:)|(?:(?:[0-9a-fA-F]{1,4}):){1}(?:(:(?:[0-9a-fA-F]{1,4})){0,4}:((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])|(:(?:[0-9a-fA-F]{1,4})){1,6}|:)|(?::((?::(?:[0-9a-fA-F]{1,4})){0,5}:((?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])[.]){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])|(?::(?:[0-9a-fA-F]{1,4})){1,7}|:)))(%[0-9a-zA-Z.]{1,})?$",
                false,
                "fe80::1",
            ),
            (
                "django_email_local_part",
                "(^[-!#$%&'*+/=?^_`{}|~0-9A-Z]+(\\.[-!#$%&'*+/=?^_`{}|~0-9A-Z]+)*\\z|^\"([\\001-\\010\\013\\014\\016-\\037!#-\\[\\]-\\177]|\\\\[\\001-\\011\\013\\014\\016-\\177])*\"\\z)",
                true,
                "john.doe",
            ),
        ];
        for &(name, pattern, ignore_case, matching) in cases {
            let re = RegexBuilder::new(pattern)
                .case_insensitive(ignore_case)
                .multi_line_anchors(false)
                .build()
                .unwrap();
            assert_eq!(re.seek_exact(), Some(true), "{name}");
            assert_eq!(re.seek_decides_is_match(matching), Some(true), "{name}");
        }
    }

    /// Text the seek reads as UTF-8 and the matcher may read otherwise is
    /// left to the matcher: an invalid byte or a start inside a character.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn seek_path_leaves_invalid_utf8_and_inner_starts_to_the_matcher() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"[^a]").unwrap();
        let text = "é".as_bytes();
        for start in 0..=text.len() {
            let expected = re
                .is_match_bytes_at_with(text, start, SearchOptions::new())
                .unwrap();
            assert_eq!(re.is_match_bytes_at(text, start), expected, "start {start}");
        }
        let invalid = [0xFF_u8];
        let expected = re
            .is_match_bytes_at_with(&invalid, 0, SearchOptions::new())
            .unwrap();
        assert_eq!(re.is_match_bytes(&invalid), expected);
        assert_eq!(re.find_bytes(&invalid).is_some(), expected);
    }

    /// The bounds of every group of a capture result, `None` for no match.
    fn capture_spans(found: Option<Captures<'_>>) -> Option<Vec<Option<Range<usize>>>> {
        found.map(|caps| {
            (0..caps.len())
                .map(|group| caps.get(group).map(|m| m.range()))
                .collect()
        })
    }

    /// The capture methods with the seek gate forced on, forced off and left
    /// adaptive give the matcher's answer (`captures_*_with`, which never
    /// reaches the seek), at every character boundary and for every method.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn capture_gate_never_changes_a_capture_result() {
        let _limits = crate::regexec::shared_limits();
        let patterns = [
            r#"\A(?:('[^'\\]*(?:\\.[^'\\]*)*')|("[^"\\]*(?:\\.[^"\\]*)*"))"#,
            r"(\d+)-(\d+)",
            r"x(?=y)(.)",
            r"\Gab(c)?",
            r"^(\w+)\s*=\s*(\S*)$",
            r"(?<=é)a(b)?",
            r"\A(a|b)\1",
            r"(?i)\bfoo\b(.)",
        ];
        let texts = [
            "",
            "x",
            "ab",
            "abc",
            "'it''s'",
            "\"q\\\"\"",
            "12-34 x",
            "xy",
            "é",
            "éab",
            "Foo foo",
            "a=b",
            "bb",
            "a\u{1F600}b",
        ];
        for pattern in patterns {
            let re = Regex::new(pattern).unwrap();
            for text in texts {
                for start in (0..=text.len()).filter(|&at| text.is_char_boundary(at)) {
                    let expected = re
                        .captures_at_with(text, start, SearchOptions::new())
                        .unwrap();
                    let expected = capture_spans(expected);
                    let expected_find = re
                        .find_at_with(text, start, SearchOptions::new())
                        .unwrap()
                        .map(|found| found.range());
                    for mode in [Some(true), Some(false), None] {
                        re.seek_gate_force(mode);
                        assert_eq!(
                            capture_spans(re.captures_at(text, start)),
                            expected,
                            "{pattern} on {text:?} from {start}, gate {mode:?}"
                        );
                        assert_eq!(
                            re.find_at(text, start).map(|found| found.range()),
                            expected_find,
                            "find: {pattern} on {text:?} from {start}, gate {mode:?}"
                        );
                    }
                    re.seek_gate_force(None);
                }
                // The other entry points of the capture family.
                let expected = capture_spans(re.captures_with(text, SearchOptions::new()).unwrap());
                let expected_bytes = capture_spans(re.captures_bytes(text.as_bytes()));
                assert_eq!(expected_bytes, expected, "{pattern} on {text:?}");
                assert_eq!(
                    capture_spans(re.captures(text)),
                    expected,
                    "{pattern} on {text:?}"
                );
            }
        }
    }

    /// The capture methods of a pattern that matches only at its start (`\A`,
    /// `\G`, `^`) give the matcher's answer of `captures_*_with`, at every
    /// character boundary, through the direct path of `onig_search_captures`.
    /// The patterns share the thread's cached region, one with groups and one
    /// without, so a region reused from another pattern cannot leak into a result.
    #[test]
    fn anchored_captures_match_the_general_path_through_the_api() {
        let _limits = crate::regexec::shared_limits();
        let patterns = [
            r"\A(\d+)-(\w+)?",
            r"\A",
            r"\Gab(c)?",
            r"^(\w+)\s*=\s*(\S*)$",
            r"\A(?:(a)|b)",
            r"(\d+)",
            r"\Ab",
        ];
        let texts = ["", "12-ab", "a=b", "ab", "abc", "bb", "é", "x\nab", "abab"];
        for text in texts {
            for pattern in patterns {
                let re = Regex::new(pattern).unwrap();
                for start in (0..=text.len()).filter(|&at| text.is_char_boundary(at)) {
                    let expected = capture_spans(
                        re.captures_at_with(text, start, SearchOptions::new())
                            .unwrap(),
                    );
                    assert_eq!(
                        capture_spans(re.captures_at(text, start)),
                        expected,
                        "{pattern} on {text:?} from {start}"
                    );
                    assert_eq!(
                        capture_spans(re.captures_bytes_at(text.as_bytes(), start)),
                        expected,
                        "{pattern} on {text:?} from {start} (bytes)"
                    );
                }
                let expected = capture_spans(re.captures_with(text, SearchOptions::new()).unwrap());
                assert_eq!(
                    capture_spans(re.captures(text)),
                    expected,
                    "{pattern} on {text:?}"
                );
            }
        }
    }

    /// The adaptive gate changes state with the rejections it measures: a
    /// pattern the seek rejects keeps the gate open, one whose seek finds a
    /// match closes it, and the state of a clone is the original's.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn capture_gate_follows_the_rejection_rate() {
        let _limits = crate::regexec::shared_limits();
        let rejecting = Regex::new(r"\Aabc(d)").unwrap();
        for _ in 0..32 {
            assert!(rejecting.captures("xyz").is_none());
        }
        let (open, rate) = rejecting.seek_gate_state();
        assert!(open, "rate {rate}");
        assert!(rate >= SEEK_GATE_OPEN_PERMILLE, "rate {rate}");

        // A pure literal has no seek, so the accepting pattern is not one.
        let accepting = Regex::new(r"x\d").unwrap();
        for _ in 0..32 {
            assert!(accepting.captures("x1").is_some());
        }
        let (open, rate) = accepting.seek_gate_state();
        assert!(!open, "rate {rate}");
        assert!(rate < SEEK_GATE_CLOSE_PERMILLE, "rate {rate}");
        // A closed gate still measures: its probes run the seek, and rejections
        // among them reopen it.
        for _ in 0..SEEK_GATE_PROBE_INTERVAL * 16 {
            assert!(accepting.captures("y").is_none());
        }
        let (open, rate) = accepting.seek_gate_state();
        assert!(open, "rate {rate}");

        let original = Regex::new(r"\Aabc").unwrap();
        let clone = original.clone();
        let before = original.seek_gate_state();
        assert!(clone.captures("xyz").is_none());
        assert_ne!(original.seek_gate_state(), before);
        assert_eq!(original.seek_gate_state(), clone.seek_gate_state());
    }

    /// Clones of a regex share its gate and seek: captures on several threads
    /// give the matcher's answers, and nothing panics or deadlocks.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn capture_gate_is_shared_and_thread_safe() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"\Aabc(d)?").unwrap();
        std::thread::scope(|scope| {
            for thread in 0..4 {
                let re = re.clone();
                scope.spawn(move || {
                    for round in 0..500 {
                        let (text, matches) = if (round + thread) % 3 == 0 {
                            ("abcd", true)
                        } else {
                            ("xyz", false)
                        };
                        let found = re.captures(text);
                        assert_eq!(found.is_some(), matches, "{text}");
                        let expected = re
                            .captures_with(text, SearchOptions::new())
                            .unwrap()
                            .is_some();
                        assert_eq!(found.is_some(), expected, "{text}");
                    }
                });
            }
        });
        assert_eq!(re.seek_gate_state(), re.clone().seek_gate_state());
    }

    /// The seek's automaton is built by the first search that consults it,
    /// not by the builder: iterating and the `*_with` methods never build
    /// it, the first standard search does, and a clone shares it.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn seek_automaton_is_built_by_the_first_search_that_consults_it() {
        let _limits = crate::regexec::shared_limits();
        let pattern = r"\b[a-z]+ing\b";
        let text = "nothing here but sing and ring";
        let re = Regex::new(pattern).unwrap();
        assert!(!re.seek_built());
        assert_eq!(re.find_iter(text).count(), 3);
        assert!(
            re.captures_with(text, SearchOptions::new())
                .unwrap()
                .is_some()
        );
        assert!(!re.seek_built());
        let clone = re.clone();
        assert!(re.is_match(text));
        assert!(re.seek_built() && clone.seek_built());

        // A pure literal has no automaton to build, and one that does not
        // build stays unbuilt.
        let literal = Regex::new("sing").unwrap();
        assert!(literal.is_match(text));
        assert!(!literal.seek_built() && !literal.seek_present());
        let unseekable = RegexBuilder::new(pattern).dfa(false).build().unwrap();
        assert!(unseekable.is_match(text));
        assert!(!unseekable.seek_built());
    }

    /// Threads that race to a regex's first search get the one automaton and
    /// the matcher's answers.
    #[cfg(feature = "dfa-prefilter")]
    #[test]
    fn seek_automaton_is_built_once_for_racing_threads() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"\b[a-z]+ing\b").unwrap();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let re = re.clone();
                scope.spawn(move || {
                    assert!(re.is_match("nothing here but sing and ring"));
                    assert!(!re.is_match("no match here"));
                });
            }
        });
        assert!(re.seek_built());
    }

    /// A match of a `&str` search slices the text in `as_str` instead of
    /// validating the bytes; every route must give what validation gives, and
    /// `Debug` keeps the layout it had before matches knew their subject.
    #[test]
    fn str_matches_slice_what_validation_returns() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(\w+)(?:é|ü)?").unwrap();
        let text = "Grüße café x 12 naïve";
        let validated = |m: Match<'_>| std::str::from_utf8(m.as_bytes()).unwrap().to_owned();
        for m in re.find_iter(text) {
            assert_eq!(m.as_str(), validated(m));
        }
        for caps in re.captures_iter(text) {
            for m in caps.iter().flatten() {
                assert_eq!(m.as_str(), validated(m));
            }
        }
        let m = re.find_at(text, 7).unwrap();
        assert_eq!(m.as_str(), validated(m));
        let caps = re
            .captures_at_with(text, 7, SearchOptions::new())
            .unwrap()
            .unwrap();
        assert_eq!(&caps[1], validated(caps.get(1).unwrap()));
        let bytes = re.find_bytes(text.as_bytes()).unwrap();
        assert_eq!(bytes.as_str(), "Grüße");
        assert_eq!(
            format!("{:?}", Regex::new("ab").unwrap().find("xab").unwrap()),
            "Match { text: [120, 97, 98], start: 1, end: 3 }"
        );
    }

    /// Captures keep the compiled regex only to resolve names. A pattern
    /// without named groups must not touch the shared reference count per
    /// match: that write serialized threads iterating with the same regex.
    #[test]
    fn captures_hold_the_regex_only_for_named_groups() {
        let _limits = crate::regexec::shared_limits();
        let unnamed = Regex::new(r"<(\w+)>").unwrap();
        let held: Vec<Captures<'_>> = unnamed.captures_iter("<a><b><c>").collect();
        assert_eq!(held.len(), 3);
        assert_eq!(Arc::strong_count(&unnamed.inner), 1);
        let single = unnamed.captures("<d>").unwrap();
        assert_eq!(Arc::strong_count(&unnamed.inner), 1);
        assert!(single.name("tag").is_none());
        assert_eq!(&single[1], "d");

        let named = Regex::new(r"<(?<tag>\w+)>").unwrap();
        let held: Vec<Captures<'_>> = named.captures_iter("<a><b>").collect();
        assert_eq!(Arc::strong_count(&named.inner), 3);
        let tags: Vec<&str> = held
            .iter()
            .map(|caps| caps.name("tag").unwrap().as_str())
            .collect();
        assert_eq!(tags, ["a", "b"]);
        drop(held);
        assert_eq!(Arc::strong_count(&named.inner), 1);
    }

    #[test]
    fn compiled_regex_types_are_send_and_sync() {
        assert_send_sync::<Regex>();
        assert_send_sync::<crate::regset::OnigRegSet>();
        assert_send_sync::<crate::scanner::Scanner>();
    }

    #[test]
    fn public_types_implement_debug() {
        use crate::scanner::{OnigString, Scanner, ScannerConfig, ScannerPatternCache};
        let _limits = crate::regexec::shared_limits();

        let re = Regex::new(r"(a)(?<n>b)").unwrap();
        let text = "ab ab";
        let captures = re.captures(text).unwrap();
        let config = ScannerConfig::default();
        let mut cache = ScannerPatternCache::new();
        let _top =
            Scanner::with_pattern_cache(&[r"\bfn\b", r#""[^"]*""#], &config, &mut cache).unwrap();
        let _call =
            Scanner::with_pattern_cache(&[r"\)", r#""[^"]*""#], &config, &mut cache).unwrap();
        let scanner = Scanner::new(&[r"\d+", "[a-z]+"]).unwrap();
        let custom_syntax: &'static OnigSyntaxType =
            Box::leak(Box::new(crate::regsyntax::OnigSyntaxRuby.clone()));

        let shown: [(String, &str); 15] = [
            (
                format!("{:?}", Regex::builder("a+").syntax_mode(Syntax::Ruby)),
                "pattern: \"a+\", options:",
            ),
            (
                format!("{:?}", Regex::builder("a+").syntax(custom_syntax)),
                "syntax: None",
            ),
            (format!("{:?}", captures.iter()), "CapturesIter { .. }"),
            (format!("{:?}", re.capture_names()), "CaptureNames { .. }"),
            (format!("{:?}", re.find_iter(text)), "FindIter { .. }"),
            (
                format!("{:?}", re.find_iter_with(text, SearchOptions::new())),
                "TryFindIter { .. }",
            ),
            (
                format!("{:?}", re.captures_iter(text)),
                "CaptureMatches { .. }",
            ),
            (
                format!("{:?}", re.captures_iter_with(text, SearchOptions::new())),
                "TryCaptureMatches { .. }",
            ),
            (format!("{:?}", re.split(text)), "Split { .. }"),
            (format!("{:?}", re.splitn(text, 2)), "SplitN { .. }"),
            (
                format!("{:?}", re.split_bytes(text.as_bytes())),
                "SplitBytes { .. }",
            ),
            (
                format!("{:?}", re.splitn_bytes(text.as_bytes(), 2)),
                "SplitNBytes { .. }",
            ),
            (format!("{scanner:?}"), "Scanner { patterns: 2,"),
            (format!("{cache:?}"), "ScannerPatternCache { len: 3, .. }"),
            (
                format!("{:?}", OnigString::new("a💻b")),
                "OnigString { content: \"a💻b\", .. }",
            ),
        ];
        for (shown, expected) in &shown {
            assert!(
                shown.contains(*expected),
                "{shown} does not contain {expected}"
            );
        }
    }

    #[test]
    fn find_reuses_the_thread_local_region_buffer() {
        let _limits = crate::regexec::shared_limits();
        clear_cached_region();
        // `\K` keeps `find` on the region path.
        let re = Regex::new(r"(a)(b)\K(c)").unwrap();

        assert_eq!(re.find("abc").unwrap().as_str(), "c");
        let first = cached_region_buffer().expect("find should return its region to the cache");
        assert!(first.1 >= 4);

        assert_eq!(re.find("abc").unwrap().as_str(), "c");
        let second = cached_region_buffer().expect("find should preserve the cached region");
        assert_eq!(second, first);
    }

    #[test]
    fn captures_returns_its_region_buffer_when_dropped() {
        let _limits = crate::regexec::shared_limits();
        clear_cached_region();
        let re = Regex::new(r"(a)(b)(c)").unwrap();
        let captures = re.captures("abc").unwrap();
        let buffer = captures.region.beg.as_ptr();

        assert!(cached_region_buffer().is_none());
        drop(captures);

        let cached = cached_region_buffer().expect("drop should return the captures region");
        assert_eq!(cached.0, buffer);
        assert!(cached.1 >= 4);
    }

    #[test]
    fn result_drop_during_thread_local_teardown_does_not_panic() {
        let _limits = crate::regexec::shared_limits();
        thread_local! {
            static LATE_RESULT: RefCell<Option<Captures<'static>>> = const { RefCell::new(None) };
        }

        std::thread::spawn(|| {
            // Initialize the result holder before CACHED_REGION so it is
            // destroyed afterwards and drops Captures with the cache gone.
            LATE_RESULT.with(|result| assert!(result.borrow().is_none()));
            let regex = Box::leak(Box::new(Regex::new(r"(a)").unwrap()));
            let captures = regex.captures("a").unwrap();
            LATE_RESULT.with(|result| *result.borrow_mut() = Some(captures));
        })
        .join()
        .expect("result drop during thread teardown must not panic");
    }

    #[test]
    fn find_iter_reuses_one_region_for_every_step() {
        let _limits = crate::regexec::shared_limits();
        clear_cached_region();
        // `\K` moves the match start away from the attempt, so only the
        // region carries the match.
        let re = Regex::new(r"(\w)\K\w+").unwrap();
        let mut matches = re.find_iter("one two");
        assert!(matches.msa.is_none());

        assert_eq!(matches.next().unwrap().as_str(), "ne");
        let first = matches.region.beg.as_ptr();
        assert_eq!(matches.next().unwrap().as_str(), "wo");
        assert_eq!(matches.region.beg.as_ptr(), first);
        drop(matches);

        let cached = cached_region_buffer().expect("iterator drop should return its region");
        assert_eq!(cached.0, first);
    }

    #[test]
    fn find_iter_reuses_one_match_arg_without_a_region() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(\w+)").unwrap();
        let mut matches = re.find_iter("one two");
        let msa: *const MatchArg = &**matches.msa.as_ref().expect("bounds-only iterator");
        assert_eq!(matches.next().unwrap().as_str(), "one");
        assert_eq!(matches.next().unwrap().as_str(), "two");
        assert!(matches.next().is_none());
        assert!(std::ptr::eq(&**matches.msa.as_ref().unwrap(), msa));
        assert!(matches.region.beg.is_empty(), "no region was requested");
    }

    /// The bounds-only iterator yields what the region-based one yields.
    #[test]
    fn find_iter_bounds_match_the_region_path() {
        let _limits = crate::regexec::shared_limits();
        let patterns = [
            r"\d+",
            r"(\w+)",
            r"(a)|b",
            r"\b(\w+)\s+\1\b",
            r"(?<=@)\w+",
            r"\w+(?=\()",
            r"x*",
            r"",
            r"^",
            r"$",
            r"(?m)^\s*$",
            r"alpha|beta|gamma|delta",
            r"(alpha|beta|gamma|delta)",
            r"(?i)(?:error|warn|fatal|panic)",
            r"\b(?:if|else|for|while)\b",
            r"(?>a+)b|a",
            r"(a|ab)(c|bcd)(d*)",
            r"[\w.+-]+@\w+\.\w+",
            r"\p{L}+",
            r"é|e",
            r".*",
            r"(?<n>\d)(?<m>\d)?",
        ];
        let texts = [
            "",
            "a",
            "one two two three",
            "x@y.z foo(1) bar( a@b.cd",
            "alpha beta gammadelta ERROR warn Panic",
            "if else\nfor  \n\nwhile",
            "aab abcd abcbcdd",
            "Grüße café éte 12 345",
            "xxaxxb",
        ];
        for pattern in patterns {
            let re = Regex::new(pattern).unwrap();
            for text in texts {
                let bounds = re.find_iter(text);
                assert!(bounds.msa.is_some(), "{pattern}");
                let region_path = FindIter {
                    regex: &re,
                    program: re.program(),
                    text: text.as_bytes(),
                    subject: Subject::Str(text),
                    cursor: MatchCursor::default(),
                    region: OnigRegion::new(),
                    msa: None,
                    limits: None,
                };
                let got: Vec<_> = bounds.map(|m| m.range()).collect();
                let expected: Vec<_> = region_path.map(|m| m.range()).collect();
                assert_eq!(got, expected, "{pattern:?} {text:?}");
                assert_eq!(
                    re.find(text).map(|m| m.range()),
                    expected.first().cloned(),
                    "{pattern:?} {text:?}"
                );
            }
        }
        // With `\K` the region carries the match start.
        let keep = Regex::new(r"(\w)\K\w+").unwrap();
        assert_eq!(keep.find("one two").map(|m| m.range()), Some(1..3));
    }

    /// Iteration over an unbounded map optimizer, where the next search starts
    /// on a candidate byte or before one. The expected ranges are the
    /// leftmost-first matches, whatever path finds them.
    #[test]
    fn find_iter_map_candidates_at_and_after_the_start() {
        let _limits = crate::regexec::shared_limits();
        // The start is a candidate (`a`) after each match: the search starts there.
        let re = Regex::new(r"a+|b").unwrap();
        let ranges: Vec<_> = re.find_iter("aabxba").map(|m| m.range()).collect();
        assert_eq!(ranges, vec![0..2, 2..3, 4..5, 5..6]);
        // The first byte is no candidate: the search scans on to the `a`.
        let ranges: Vec<_> = re.find_iter("xa").map(|m| m.range()).collect();
        assert_eq!(ranges, vec![1..2]);
        // Multibyte characters hold no candidate byte of `a+|b`: the match is
        // the `a` after them.
        let ranges: Vec<_> = re.find_iter("éaé").map(|m| m.range()).collect();
        assert_eq!(ranges, vec![2..3]);
        let re = Regex::new(r"é+|a").unwrap();
        let ranges: Vec<_> = re.find_iter("éaé").map(|m| m.range()).collect();
        assert_eq!(ranges, vec![0..2, 2..3, 3..5]);
        // A start on a character boundary, where the match starts at once.
        assert_eq!(re.find_at("éaé", 2).map(|m| m.range()), Some(2..3));
        // No candidate from the start: no match.
        assert!(re.find_at("éaé", 5).is_none());
        // Empty matches still step one character at a time.
        let re = Regex::new(r"x*|a").unwrap();
        let ranges: Vec<_> = re.find_iter("ab").map(|m| m.range()).collect();
        assert_eq!(ranges, vec![0..0, 1..1, 2..2]);
        // Look-behind at the boundary of the previous match.
        let re = Regex::new(r"(?<=a)b|c+").unwrap();
        let ranges: Vec<_> = re.find_iter("abcc b").map(|m| m.range()).collect();
        assert_eq!(ranges, vec![1..2, 2..4]);
        // \G anchors each search at its own start.
        let re = Regex::new(r"\Ga|b").unwrap();
        let ranges: Vec<_> = re.find_iter("aba").map(|m| m.range()).collect();
        assert_eq!(ranges, vec![0..1, 1..2, 2..3]);
    }

    #[test]
    fn regex_new_and_find() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"\d+").unwrap();
        let m = re.find("hello 42 world").unwrap();
        assert_eq!(m.as_str(), "42");
        assert_eq!(m.start(), 6);
        assert_eq!(m.end(), 8);
        assert_eq!(m.range(), 6..8);
        assert_eq!(m.len(), 2);
        assert!(!m.is_empty());
    }

    #[test]
    fn regex_no_match() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"\d+").unwrap();
        assert!(re.find("no digits here").is_none());
    }

    #[test]
    fn regex_is_match() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"hello").unwrap();
        assert!(re.is_match("say hello"));
        assert!(!re.is_match("say goodbye"));
    }

    #[test]
    fn search_options_methods_return_matches_and_captures() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(\w+)").unwrap();
        let options = SearchOptions::new().timeout(Duration::from_secs(1));

        assert_eq!(
            re.find_with("hello 42", options).unwrap().unwrap().as_str(),
            "hello"
        );
        assert_eq!(
            re.find_bytes_with(b"hello 42", options)
                .unwrap()
                .unwrap()
                .as_str(),
            "hello"
        );
        assert!(re.is_match_with("hello", options).unwrap());
        assert!(!re.is_match_bytes_with(b"!!!", options).unwrap());
        assert_eq!(
            re.captures_with("hello 42", options)
                .unwrap()
                .unwrap()
                .get(1)
                .unwrap()
                .as_str(),
            "hello"
        );
        assert!(re.captures_bytes_with(b"!!!", options).unwrap().is_none());
    }

    #[test]
    fn search_options_surface_time_limit_errors() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(a+)+b").unwrap();
        let text = "a".repeat(40);
        let options = SearchOptions::new()
            .timeout(Duration::from_millis(1))
            .retry_limit_in_match(0)
            .retry_limit_in_search(0);

        assert!(matches!(
            re.find_with(&text, options),
            Err(RegexError::TimeLimitOver)
        ));
        assert!(matches!(
            re.is_match_with(&text, options),
            Err(RegexError::TimeLimitOver)
        ));
        assert!(matches!(
            re.captures_with(&text, options),
            Err(RegexError::TimeLimitOver)
        ));
    }

    #[test]
    fn search_options_timeout_spans_every_start_position() {
        let _limits = crate::regexec::shared_limits();
        // Each start position backtracks only ~400 times; the limit must still
        // fire for the search as a whole.
        let re = Regex::new(r"a{1,400}?(?=b)").unwrap();
        let text = "a".repeat(500_000);
        let options = SearchOptions::new().timeout(Duration::from_millis(10));

        let started = std::time::Instant::now();
        assert!(matches!(
            re.find_with(&text, options),
            Err(RegexError::TimeLimitOver)
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    /// Rust-only (ADR-008): the skip gates of guard chains keep the retry
    /// limit's boundary where the one-by-one walk puts it. With the gates off,
    /// the smallest per-match limit that finds the match is found, and the
    /// gated program gives the same result at that limit and one below it.
    #[test]
    fn skip_gates_keep_the_retry_limit_boundary() {
        let _limits = crate::regexec::shared_limits();
        let compile = |pattern: &str, gated: bool| {
            crate::regcomp::SKIP_GATES_DISABLED.with(|disabled| disabled.set(!gated));
            let re = Regex::new(pattern);
            crate::regcomp::SKIP_GATES_DISABLED.with(|disabled| disabled.set(false));
            re.unwrap()
        };
        let text = format!("{}ss!", "ß".repeat(4));
        let plain = compile(r"(?i)^[[:word:]]{0,40}s!", false);
        let gated = compile(r"(?i)^[[:word:]]{0,40}s!", true);
        let at = |re: &Regex, limit: u64| {
            re.find_with(&text, SearchOptions::new().retry_limit_in_match(limit))
                .map(|m| m.map(|m| (m.start(), m.end())))
        };
        let boundary = (1..=100_000u64)
            .find(|&limit| matches!(at(&plain, limit), Ok(Some(_))))
            .expect("the match fits in the default limit");
        assert!(boundary > 1, "the search backtracks");
        for limit in [boundary - 1, boundary] {
            assert_eq!(at(&gated, limit), at(&plain, limit), "limit {limit}");
        }
        assert!(matches!(
            at(&plain, boundary - 1),
            Err(RegexError::RetryLimitInMatchOver)
        ));
    }

    #[test]
    fn search_options_surface_retry_limit_errors() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(a+)+b").unwrap();
        let text = "a".repeat(30);

        assert!(matches!(
            re.find_with(&text, SearchOptions::new().retry_limit_in_match(1_000)),
            Err(RegexError::RetryLimitInMatchOver)
        ));
        assert!(matches!(
            re.find_with(
                &text,
                SearchOptions::new()
                    .retry_limit_in_match(0)
                    .retry_limit_in_search(1_000)
            ),
            Err(RegexError::RetryLimitInSearchOver)
        ));
    }

    /// A search under per-search limits leaves none behind: the standard
    /// methods, which reuse the same thread-cached MatchArg, keep the
    /// process-wide limits.
    #[test]
    fn search_options_limits_do_not_leak_into_standard_searches() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"a*?b").unwrap();
        let text = format!("{}b", "a".repeat(50));
        let tight = SearchOptions::new().retry_limit_in_match(3);

        assert!(matches!(
            re.find_with(&text, tight),
            Err(RegexError::RetryLimitInMatchOver)
        ));
        assert!(matches!(
            re.is_match_with(&text, tight),
            Err(RegexError::RetryLimitInMatchOver)
        ));
        assert!(matches!(
            re.captures_with(&text, tight),
            Err(RegexError::RetryLimitInMatchOver)
        ));
        assert!(matches!(
            re.find_iter_with(&text, tight).next(),
            Some(Err(RegexError::RetryLimitInMatchOver))
        ));
        assert!(matches!(
            re.captures_iter_with(&text, tight).next(),
            Some(Err(RegexError::RetryLimitInMatchOver))
        ));

        assert_eq!(re.find(&text).map(|m| m.range()), Some(0..51));
        assert!(re.is_match(&text));
        assert!(re.captures(&text).is_some());
        assert_eq!(re.find_iter(&text).count(), 1);
        assert_eq!(
            re.find_with(&text, SearchOptions::new())
                .unwrap()
                .map(|m| m.range()),
            Some(0..51)
        );
    }

    /// The bounds-only path (`find`, `find_iter`) and the region path
    /// (`captures`) report the same limit errors for the same options, so
    /// `find_with` may use the bounds-only path.
    #[test]
    fn bounds_and_region_paths_report_the_same_limit_errors() {
        let _limits = crate::regexec::shared_limits();
        let patterns = [
            r"a*?b",
            r"(a+)+b",
            r"(?:a|aa)*c",
            r"\w+\s+\w+x",
            r"(\w+)@(\w+)\.com",
            r"x*y|a+z",
            r"(?>a+)b|a",
        ];
        // Short subjects: the catastrophic patterns run into their process-wide
        // limits, and 16 bytes keep that cheap.
        let texts = [
            format!("{}b", "a".repeat(16)),
            format!("{}c", "a".repeat(15)),
            "a".repeat(16),
            "one two three four five six".to_owned(),
            "mail user@example.com and more".to_owned(),
            format!("{}z", "a".repeat(20)),
            // Deep enough for a stack limit: any limit below 320 holds 321
            // entries.
            format!("{}b", "a".repeat(400)),
        ];
        let limits = [
            SearchOptions::new().retry_limit_in_match(1),
            SearchOptions::new().retry_limit_in_match(3),
            SearchOptions::new().retry_limit_in_match(20),
            SearchOptions::new().retry_limit_in_search(5),
            SearchOptions::new().retry_limit_in_search(50),
            SearchOptions::new().match_stack_limit(4),
            SearchOptions::new().match_stack_limit(40),
            SearchOptions::new().match_stack_limit(320),
        ];
        for pattern in patterns {
            let re = Regex::new(pattern).unwrap();
            for text in &texts {
                for options in limits {
                    let bounds = re.find_with(text, options).map(|m| m.map(|m| m.range()));
                    let region = re
                        .captures_with(text, options)
                        .map(|c| c.map(|c| c.get(0).unwrap().range()));
                    assert_eq!(bounds, region, "{pattern:?} {text:?} {options:?}");
                    let is_match = re.is_match_with(text, options);
                    assert_eq!(
                        is_match,
                        bounds.as_ref().map(Option::is_some).map_err(Clone::clone),
                        "{pattern:?} {text:?} {options:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn search_options_leave_unset_limits_process_wide() {
        let _limits = crate::regexec::exclusive_limits();
        let old_retry = crate::regexec::onig_get_retry_limit_in_match();
        crate::regexec::onig_set_retry_limit_in_match(1_000);
        let re = Regex::new(r"(a+)+b").unwrap();
        let text = "a".repeat(30);

        let unset = re.find_with(&text, SearchOptions::new());
        let disabled = re.find_with(
            &text,
            SearchOptions::new()
                .retry_limit_in_match(0)
                .retry_limit_in_search(0)
                .timeout(Duration::from_millis(50)),
        );
        crate::regexec::onig_set_retry_limit_in_match(old_retry);

        assert!(matches!(unset, Err(RegexError::RetryLimitInMatchOver)));
        assert!(matches!(disabled, Err(RegexError::TimeLimitOver)));
    }

    #[test]
    fn search_options_iterator_yields_matches_and_then_finishes() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"\w+").unwrap();
        let options = SearchOptions::new().timeout(Duration::from_secs(1));
        let mut matches = re.find_iter_with("one two", options);

        assert_eq!(matches.next().unwrap().unwrap().as_str(), "one");
        assert_eq!(matches.next().unwrap().unwrap().as_str(), "two");
        assert!(matches.next().is_none());
        assert!(matches.next().is_none());
        let bytes: Vec<_> = re
            .find_iter_bytes_with(b"a b", options)
            .map(|m| m.unwrap().as_bytes().to_vec())
            .collect();
        assert_eq!(bytes, vec![b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn search_options_iterator_surfaces_an_error_once() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(a+)+b").unwrap();
        let text = "a".repeat(40);
        let options = SearchOptions::new()
            .timeout(Duration::from_millis(1))
            .retry_limit_in_match(0)
            .retry_limit_in_search(0);
        let mut matches = re.find_iter_with(&text, options);

        assert!(matches!(
            matches.next().unwrap(),
            Err(RegexError::TimeLimitOver)
        ));
        assert!(matches.next().is_none());
    }

    #[test]
    fn search_options_default_keeps_every_limit_unset() {
        assert_eq!(SearchOptions::default(), SearchOptions::new());
        assert_send_sync::<SearchOptions>();
        assert_send_sync::<TryFindIter<'static, 'static>>();
    }

    #[test]
    fn timeout_is_rounded_up_to_milliseconds() {
        assert_eq!(timeout_to_millis(Duration::ZERO), 0);
        assert_eq!(timeout_to_millis(Duration::from_nanos(1)), 1);
        assert_eq!(timeout_to_millis(Duration::from_millis(1)), 1);
        assert_eq!(timeout_to_millis(Duration::from_nanos(1_000_001)), 2);
        assert_eq!(timeout_to_millis(Duration::MAX), u64::MAX);
    }

    #[test]
    fn regex_captures() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
        let caps = re.captures("date: 2026-02-14").unwrap();
        assert_eq!(caps.get(0).unwrap().as_str(), "2026-02-14");
        assert_eq!(caps.get(1).unwrap().as_str(), "2026");
        assert_eq!(caps.get(2).unwrap().as_str(), "02");
        assert_eq!(caps.get(3).unwrap().as_str(), "14");
        assert!(caps.get(4).is_none());
        assert_eq!(caps.len(), 4);
    }

    #[test]
    fn regex_captures_len() {
        let re = Regex::new(r"(a)(b)(c)").unwrap();
        assert_eq!(re.captures_len(), 3);
    }

    #[test]
    fn regex_find_iter() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"\d+").unwrap();
        let matches: Vec<&str> = re.find_iter("1 + 22 = 333").map(|m| m.as_str()).collect();
        assert_eq!(matches, vec!["1", "22", "333"]);
    }

    #[test]
    fn regex_builder_case_insensitive() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::builder(r"hello")
            .case_insensitive(true)
            .build()
            .unwrap();
        assert!(re.is_match("HELLO"));
        assert!(re.is_match("Hello"));
    }

    #[test]
    fn regex_invalid_pattern() {
        let err = Regex::new(r"(unclosed").unwrap_err();
        assert!(matches!(err, RegexError::Syntax { .. }));
    }

    #[test]
    fn match_as_bytes() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"world").unwrap();
        let m = re.find("hello world").unwrap();
        assert_eq!(m.as_bytes(), b"world");
    }

    #[test]
    fn captures_iter() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(a)(b)?").unwrap();
        let caps = re.captures("a").unwrap();
        let items: Vec<_> = caps.iter().collect();
        // group 0 = "a", group 1 = "a", group 2 = None (didn't participate)
        assert_eq!(items.len(), 3);
        assert!(items[0].is_some());
        assert!(items[1].is_some());
        assert!(items[2].is_none());
    }

    #[test]
    fn named_captures() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
        let caps = re.captures("2026-02").unwrap();
        assert_eq!(caps.name("year").unwrap().as_str(), "2026");
        assert_eq!(caps.name("month").unwrap().as_str(), "02");
        assert!(caps.name("day").is_none());
    }

    #[test]
    fn empty_match_find_iter() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"").unwrap();
        let matches: Vec<_> = re.find_iter("ab").collect();
        // Should yield empty matches at positions 0, 1, 2
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[1].start(), 1);
        assert_eq!(matches[2].start(), 2);
    }

    /// The iteration as the search loop ran it before an empty match stepped
    /// past its own search: every search starts at the end of the previous
    /// match, and an empty match right after an empty match is skipped by a
    /// search one character on. `search(at)` returns the match found from
    /// `at` with the item that the iterator yields for it.
    fn repeated_search_iteration<T>(
        re: &Regex,
        text: &[u8],
        search: impl Fn(usize) -> Option<(usize, usize, T)>,
    ) -> Vec<T> {
        let mut found = Vec::new();
        let mut last_end = 0;
        let mut last_was_empty = false;
        while last_end <= text.len() {
            let Some((start, end, item)) = search(last_end) else {
                break;
            };
            if start == end {
                if last_was_empty {
                    if last_end >= text.len() {
                        break;
                    }
                    last_end += re.program().enc.mbc_enc_len(&text[last_end..]);
                    last_was_empty = false;
                    continue;
                }
                last_was_empty = true;
            } else {
                last_was_empty = false;
            }
            last_end = end;
            found.push(item);
        }
        found
    }

    /// The spans of the groups of a match, `None` for a group that did not
    /// participate.
    type Groups = Vec<Option<(usize, usize)>>;

    fn group_spans(caps: &Captures<'_>) -> Groups {
        caps.iter()
            .map(|group| group.map(|m| (m.start(), m.end())))
            .collect()
    }

    /// Patterns with empty matches, and with `\G` and `\K`, which keep the
    /// searches of the iteration.
    const EMPTY_MATCH_PATTERNS: &[&str] = &[
        "",
        "a*",
        "a*?",
        "x*",
        r"\b",
        r"\B",
        "(?=x)",
        "(?!x)",
        "|",
        "a|",
        "|a",
        "(a|)",
        "(a)?(b*)",
        r"(\w*)",
        "(?<=a)",
        "^",
        "$",
        r"\b\w*",
        "(?:)",
        r"\d*",
        "é*",
        "(?i)k*",
        r"a*?$",
        r"(?=\w)|\s",
        r"\b|é",
        // \G reads the search start: the step is not taken. The search from
        // the end of the empty match at 1 matches "b" there, which the
        // iterator must yield.
        r"(?<=a)(?!\G)|b",
        r"\Ga*",
        r"\G",
        r"\Gx*",
        // \K moves the match start: the step is not taken.
        r"a\K",
        r"\K",
        r"x*\K",
        r"(a)\K|b*",
    ];

    const EMPTY_MATCH_TEXTS: &[&str] = &[
        "",
        "a",
        "ab",
        "aXa",
        "xéx日本",
        "a\nb\n",
        "ééé",
        "\u{1F600}x",
        "bbb",
        "x  x",
    ];

    #[test]
    fn empty_match_step_only_where_the_program_allows_it() {
        let _limits = crate::regexec::shared_limits();
        for pattern in ["", "a*", r"\b", "(?=x)", "|", r"(\w*)", "(a)?(b*)"] {
            let re = Regex::new(pattern).unwrap();
            assert!(
                re.program().skip_repeated_empty_search,
                "{pattern:?} has no \\G, \\K or callout"
            );
        }
        for pattern in [r"\Ga*", r"\G", r"a\K", r"\K", r"(a)\K|b*"] {
            let re = Regex::new(pattern).unwrap();
            assert!(
                !re.program().skip_repeated_empty_search,
                "{pattern:?} reads the search start or keeps the match start"
            );
        }
    }

    /// The iterators yield what the repeated search yields, for empty matches
    /// over ASCII, multibyte and invalid text, with and without `\G` and `\K`.
    #[test]
    fn empty_match_step_yields_what_the_repeated_search_yields() {
        let _limits = crate::regexec::shared_limits();
        for pattern in EMPTY_MATCH_PATTERNS {
            let re = Regex::new(pattern).unwrap();
            for text in EMPTY_MATCH_TEXTS {
                let bytes = text.as_bytes();
                let expected = repeated_search_iteration(&re, bytes, |at| {
                    re.find_bytes_at(bytes, at)
                        .map(|m| (m.start(), m.end(), (m.start(), m.end())))
                });
                let found: Vec<_> = re.find_iter(text).map(|m| (m.start(), m.end())).collect();
                assert_eq!(found, expected, "find_iter: {pattern:?} on {text:?}");
                let found: Vec<_> = re
                    .find_iter_bytes(bytes)
                    .map(|m| (m.start(), m.end()))
                    .collect();
                assert_eq!(found, expected, "find_iter_bytes: {pattern:?} on {text:?}");

                let expected = repeated_search_iteration(&re, bytes, |at| {
                    re.captures_bytes_at(bytes, at).map(|caps| {
                        let spans = group_spans(&caps);
                        let (start, end) = spans[0].expect("whole match");
                        (start, end, spans)
                    })
                });
                let found: Vec<_> = re.captures_iter(text).map(|c| group_spans(&c)).collect();
                assert_eq!(found, expected, "captures_iter: {pattern:?} on {text:?}");
            }
        }
    }

    /// Invalid UTF-8 is searched by the bytes API, one byte character at a time
    /// where the encoding cannot read a character.
    #[test]
    fn empty_match_step_on_invalid_utf8() {
        let _limits = crate::regexec::shared_limits();
        let text: &[u8] = b"a\xffb\xe6\x97";
        for pattern in ["", "a*", r"\b", "x*", r"(\w*)"] {
            let re = Regex::new(pattern).unwrap();
            let expected = repeated_search_iteration(&re, text, |at| {
                re.find_bytes_at(text, at)
                    .map(|m| (m.start(), m.end(), (m.start(), m.end())))
            });
            let found: Vec<_> = re
                .find_iter_bytes(text)
                .map(|m| (m.start(), m.end()))
                .collect();
            assert_eq!(found, expected, "{pattern:?} on invalid UTF-8");
        }
    }

    /// An empty match costs one search, not two: the search from its end is
    /// not run again.
    #[test]
    fn empty_match_step_takes_one_search_per_match() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new("x*").unwrap();
        let before = crate::regexec::VM_ATTEMPTS.with(std::cell::Cell::get);
        let matches = re.find_iter("abcd").count();
        let attempts = crate::regexec::VM_ATTEMPTS.with(std::cell::Cell::get) - before;
        assert_eq!(matches, 5);
        assert_eq!(attempts, 5);
    }

    /// The captures of every match, including the matches found by a search
    /// whose first attempt fails, and by a map search that starts at a later
    /// candidate.
    #[test]
    fn captures_iter_records_the_captures_of_every_match() {
        let _limits = crate::regexec::shared_limits();
        let re = Regex::new(r"#(\w+)").unwrap();
        let found: Vec<_> = re
            .captures_iter("x #ab #c")
            .map(|c| group_spans(&c))
            .collect();
        assert_eq!(
            found,
            vec![
                vec![Some((2, 5)), Some((3, 5))],
                vec![Some((6, 8)), Some((7, 8))],
            ]
        );

        let re = Regex::new(r"(a)|(b)").unwrap();
        let found: Vec<_> = re.captures_iter("cba").map(|c| group_spans(&c)).collect();
        assert_eq!(
            found,
            vec![
                vec![Some((1, 2)), None, Some((1, 2))],
                vec![Some((2, 3)), Some((2, 3)), None],
            ]
        );

        let re = Regex::new(r"(\d)(\d)?").unwrap();
        let found: Vec<_> = re
            .captures_iter("a1 22b")
            .map(|c| group_spans(&c))
            .collect();
        assert_eq!(
            found,
            vec![
                vec![Some((1, 2)), Some((1, 2)), None],
                vec![Some((3, 5)), Some((3, 4)), Some((4, 5))],
            ]
        );

        // The first attempt fails and a later one matches, untracked: the
        // captures are recorded by a second pass.
        let cases: [(&str, &str, Vec<Groups>); 4] = [
            (
                r"(a)b|(c)",
                "axc ac",
                vec![
                    vec![Some((2, 3)), None, Some((2, 3))],
                    vec![Some((5, 6)), None, Some((5, 6))],
                ],
            ),
            (
                r"(?<=z)(a)|(b)",
                "zaxb",
                vec![
                    vec![Some((1, 2)), Some((1, 2)), None],
                    vec![Some((3, 4)), None, Some((3, 4))],
                ],
            ),
            (
                r"\w(\d)|(x)",
                "1x2",
                vec![vec![Some((1, 3)), Some((2, 3)), None]],
            ),
            (
                r"(a)(b)?c|(d)e",
                "ab de",
                vec![vec![Some((3, 5)), None, None, Some((3, 4))]],
            ),
        ];
        for (pattern, text, expected) in cases {
            let re = Regex::new(pattern).unwrap();
            let found: Vec<_> = re.captures_iter(text).map(|c| group_spans(&c)).collect();
            assert_eq!(found, expected, "{pattern:?} on {text:?}");
        }
    }

    /// An iterator reads the process-wide limits again for every search, so
    /// a limit set between two matches applies to the next search, as it does
    /// for a plain search (Rust-only, ADR-008: one `MatchArg` serves the
    /// iterator's searches).
    #[test]
    fn find_iter_reads_the_process_wide_limits_for_each_search() {
        let _limits = crate::regexec::exclusive_limits();
        crate::regexec::onig_set_retry_limit_in_search(0);
        let re = Regex::new(r"x|a+c").unwrap();
        let mut matches = re.find_iter("x aabaac");
        assert_eq!(matches.next().unwrap().range(), 0..1);

        crate::regexec::onig_set_retry_limit_in_search(1);
        // The search from offset 1 backtracks in `a+c` before it reaches
        // `aac`, which it would find without the limit.
        assert!(matches.next().is_none());
    }

    /// The captures iterator reads the process-wide limits for each search,
    /// as `find_iter` does.
    #[test]
    fn captures_iter_reads_the_process_wide_limits_for_each_search() {
        let _limits = crate::regexec::exclusive_limits();
        crate::regexec::onig_set_retry_limit_in_search(0);
        let re = Regex::new(r"(x)|(a+c)").unwrap();
        let mut matches = re.captures_iter("x aabaac");
        assert_eq!(matches.next().unwrap().get(0).unwrap().range(), 0..1);

        crate::regexec::onig_set_retry_limit_in_search(1);
        assert!(matches.next().is_none());
    }

    /// Each yielded `Captures` returns its region to the thread cache when it
    /// is dropped, and the next search takes it from there.
    #[test]
    fn captures_iter_reuses_the_region_of_a_dropped_match() {
        let _limits = crate::regexec::shared_limits();
        clear_cached_region();
        let re = Regex::new(r"(\w)(\w?)").unwrap();
        let mut matches = re.captures_iter("ab cd ef");
        let first = matches.next().unwrap().region.beg.as_ptr();
        let second = matches.next().unwrap().region.beg.as_ptr();
        assert_eq!(second, first);
        let third = matches.next().unwrap().region.beg.as_ptr();
        assert_eq!(third, first);
    }
}
