//! The idiomatic Rust API. [`Regex`] compiles and searches a pattern,
//! [`RegexBuilder`] sets its options and syntax, and [`Match`], [`Captures`] and
//! [`FindIter`] report what a search found. It wraps the C-port functions
//! [`onig_new`] and [`onig_search`].

use std::cell::RefCell;
use std::fmt;
use std::ops::{Index, Range};
use std::panic::AssertUnwindSafe;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::error::RegexError;
use crate::oniguruma::*;
use crate::regcomp::{onig_new, onig_new_with_backtracking_optimization};
use crate::regexec::{
    MatchArg, OnigMatchParam, cache_msa, onig_name_to_backref_number, onig_new_match_param,
    onig_search, onig_search_bounds, onig_search_with_param, take_cached_msa,
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
    /// backtracking stack holds `limit` entries. Zero removes the limit.
    pub const fn match_stack_limit(mut self, limit: u32) -> Self {
        self.match_stack_limit = Some(limit);
        self
    }

    fn match_param(self) -> OnigMatchParam {
        let mut match_param = onig_new_match_param();
        if let Some(timeout) = self.timeout {
            match_param.time_limit = timeout_to_millis(timeout);
        }
        if let Some(limit) = self.retry_limit_in_match {
            match_param.retry_limit_in_match = limit;
        }
        if let Some(limit) = self.retry_limit_in_search {
            match_param.retry_limit_in_search = limit;
        }
        if let Some(limit) = self.match_stack_limit {
            match_param.match_stack_limit = limit;
        }
        match_param
    }
}

/// The immutable state behind a [`Regex`]: the compiled program and the
/// pattern it was compiled from. Every clone of a `Regex`, and every
/// [`Captures`] it returns, shares one value through an `Arc`.
struct Compiled {
    /// Wrapped in `AssertUnwindSafe`; the notes on [`Regex`] explain why the
    /// assertion holds.
    program: AssertUnwindSafe<RegexType>,
    pattern: Box<[u8]>,
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
        let program = onig_new(
            pattern,
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
        )?;
        Ok(Self::from_parts(program, pattern.into()))
    }

    fn from_parts(program: RegexType, pattern: Box<[u8]>) -> Regex {
        Regex {
            inner: Arc::new(Compiled {
                program: AssertUnwindSafe(program),
                pattern,
            }),
        }
    }

    /// The compiled program, shared by every clone of this regex.
    fn program(&self) -> &RegexType {
        &self.inner.program
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
        self.find_bytes(text.as_bytes())
    }

    /// Return the first match in `text` (as bytes), or `None` if no match.
    pub fn find_bytes<'t>(&self, text: &'t [u8]) -> Option<Match<'t>> {
        self.find_from(text, 0)
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
    pub fn find_at<'t>(&self, text: &'t str, start: usize) -> Option<Match<'t>> {
        check_str_start(text, start);
        self.find_from(text.as_bytes(), start)
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
        self.find_from(text, start)
    }

    /// The search behind the `find` family: the first match from `start`,
    /// with all of `text` visible.
    #[inline]
    fn find_from<'t>(&self, text: &'t [u8], start: usize) -> Option<Match<'t>> {
        // Only the bounds are returned: where they follow from the attempt
        // position and the match length, no region is needed (see
        // `find_iter_bytes`).
        if !self.program().keep_moves_match_start
            && !self.program().options.contains(ONIG_OPTION_FIND_LONGEST)
        {
            let mut msa = take_cached_msa(self.program(), ONIG_OPTION_NONE, None, start);
            let found = onig_search_bounds(self.program(), text, start, &mut msa);
            cache_msa(msa);
            let (match_start, match_end) = found.ok()??;
            return (match_end <= text.len()).then_some(Match {
                text,
                start: match_start,
                end: match_end,
            });
        }
        let (result, region) = onig_search(
            self.program(),
            text,
            text.len(),
            start,
            text.len(),
            Some(take_cached_region()),
            ONIG_OPTION_NONE,
        );
        let region = region?;
        if result < 0 {
            cache_region(region);
            return None;
        }
        if region.num_regs < 1 {
            cache_region(region);
            return None;
        }
        let m = Match::from_region(text, region.beg[0], region.end[0]);
        cache_region(region);
        m
    }

    /// Return the first match in `text` under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached, such as
    /// [`RegexError::TimeLimitOver`], instead of treating it as "no match".
    pub fn find_with<'t>(
        &self,
        text: &'t str,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        self.find_bytes_with(text.as_bytes(), options)
    }

    /// Return the first match in `text` (as bytes) under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    pub fn find_bytes_with<'t>(
        &self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        self.find_from_with(text, 0, options)
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
    pub fn find_at_with<'t>(
        &self,
        text: &'t str,
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        check_str_start(text, start);
        self.find_from_with(text.as_bytes(), start, options)
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
    pub fn find_bytes_at_with<'t>(
        &self,
        text: &'t [u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        check_bytes_start(text, start);
        self.find_from_with(text, start, options)
    }

    fn find_from_with<'t>(
        &self,
        text: &'t [u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Match<'t>>, RegexError> {
        let (result, region) =
            self.search_with(text, start, text.len(), Some(take_cached_region()), options)?;
        let Some(region) = region else {
            return Ok(None);
        };
        if result < 0 || region.num_regs < 1 {
            cache_region(region);
            return Ok(None);
        }
        let m = Match::from_region(text, region.beg[0], region.end[0]);
        cache_region(region);
        Ok(m)
    }

    /// Check whether `text` matches the pattern anywhere.
    ///
    /// A search that stops at a process-wide limit (time, retry or stack) is
    /// reported as no match. Use [`Regex::is_match_with`] with [`SearchOptions`] to tell
    /// the two apart.
    pub fn is_match(&self, text: &str) -> bool {
        self.is_match_bytes(text.as_bytes())
    }

    /// Check whether `text` (as bytes) matches the pattern anywhere.
    pub fn is_match_bytes(&self, text: &[u8]) -> bool {
        self.is_match_from(text, 0)
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
        self.is_match_from(text.as_bytes(), start)
    }

    /// Check whether `text` (as bytes) has a match at or after byte offset
    /// `start`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    pub fn is_match_bytes_at(&self, text: &[u8], start: usize) -> bool {
        check_bytes_start(text, start);
        self.is_match_from(text, start)
    }

    #[inline]
    fn is_match_from(&self, text: &[u8], start: usize) -> bool {
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
    pub fn is_match_with(&self, text: &str, options: SearchOptions) -> Result<bool, RegexError> {
        self.is_match_bytes_with(text.as_bytes(), options)
    }

    /// Check whether `text` (as bytes) matches under per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
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
        let (result, _) = self.search_with(text, start, text.len(), None, options)?;
        Ok(result >= 0)
    }

    /// Return the first match with all capture groups, or `None`.
    ///
    /// A search that stops at a process-wide limit (time, retry or stack) is
    /// reported as no match. Use [`Regex::captures_with`] with [`SearchOptions`] to tell
    /// the two apart.
    pub fn captures<'t>(&self, text: &'t str) -> Option<Captures<'t>> {
        self.captures_bytes(text.as_bytes())
    }

    /// Return the first match with all capture groups (bytes), or `None`.
    pub fn captures_bytes<'t>(&self, text: &'t [u8]) -> Option<Captures<'t>> {
        self.captures_from(text, 0)
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
        self.captures_from(text.as_bytes(), start)
    }

    /// Return the first match at or after byte offset `start` (as bytes), with
    /// all capture groups, or `None`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `text.len()`.
    pub fn captures_bytes_at<'t>(&self, text: &'t [u8], start: usize) -> Option<Captures<'t>> {
        check_bytes_start(text, start);
        self.captures_from(text, start)
    }

    #[inline]
    fn captures_from<'t>(&self, text: &'t [u8], start: usize) -> Option<Captures<'t>> {
        let (result, region) = onig_search(
            self.program(),
            text,
            text.len(),
            start,
            text.len(),
            Some(take_cached_region()),
            ONIG_OPTION_NONE,
        );
        let region = region?;
        if result < 0 {
            cache_region(region);
            return None;
        }
        Some(Captures {
            text,
            region,
            compiled: Arc::clone(&self.inner),
        })
    }

    /// Return the first match with all capture groups under per-search
    /// `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    pub fn captures_with<'t>(
        &self,
        text: &'t str,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        self.captures_bytes_with(text.as_bytes(), options)
    }

    /// Return the first match with all capture groups (bytes) under
    /// per-search `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`RegexError`] of a limit the search reached.
    pub fn captures_bytes_with<'t>(
        &self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        self.captures_from_with(text, 0, options)
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
    pub fn captures_at_with<'t>(
        &self,
        text: &'t str,
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        check_str_start(text, start);
        self.captures_from_with(text.as_bytes(), start, options)
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
    pub fn captures_bytes_at_with<'t>(
        &self,
        text: &'t [u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        check_bytes_start(text, start);
        self.captures_from_with(text, start, options)
    }

    fn captures_from_with<'t>(
        &self,
        text: &'t [u8],
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Captures<'t>>, RegexError> {
        let (result, region) =
            self.search_with(text, start, text.len(), Some(take_cached_region()), options)?;
        let Some(region) = region else {
            return Ok(None);
        };
        if result < 0 {
            cache_region(region);
            return Ok(None);
        }
        Ok(Some(Captures {
            text,
            region,
            compiled: Arc::clone(&self.inner),
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
        self.find_iter_bytes(text.as_bytes())
    }

    /// Iterate over all non-overlapping matches in `text` (as bytes).
    pub fn find_iter_bytes<'r, 't>(&'r self, text: &'t [u8]) -> FindIter<'r, 't> {
        // An iterator yields match bounds only. Where they follow from the
        // attempt position and the match length, it searches without a
        // region, reusing one MatchArg for all its searches.
        let bounds_only = !self.program().keep_moves_match_start
            && !self.program().options.contains(ONIG_OPTION_FIND_LONGEST);
        FindIter {
            regex: self,
            text,
            cursor: MatchCursor::default(),
            region: if bounds_only {
                OnigRegion::new()
            } else {
                take_cached_region()
            },
            msa: bounds_only.then(|| take_cached_msa(self.program(), ONIG_OPTION_NONE, None, 0)),
        }
    }

    /// Iterate over all non-overlapping matches in `text` under per-search
    /// `options`.
    ///
    /// The options apply to each search the iterator runs. A search that
    /// reaches a limit yields its error once and ends the iteration.
    pub fn find_iter_with<'r, 't>(
        &'r self,
        text: &'t str,
        options: SearchOptions,
    ) -> TryFindIter<'r, 't> {
        self.find_iter_bytes_with(text.as_bytes(), options)
    }

    /// Iterate over all non-overlapping matches in `text` (as bytes) under
    /// per-search `options`.
    pub fn find_iter_bytes_with<'r, 't>(
        &'r self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> TryFindIter<'r, 't> {
        TryFindIter {
            regex: self,
            text,
            cursor: MatchCursor::default(),
            region: take_cached_region(),
            options,
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
        self.captures_iter_bytes(text.as_bytes())
    }

    /// Iterate over the capture groups of all non-overlapping matches in
    /// `text` (as bytes).
    pub fn captures_iter_bytes<'r, 't>(&'r self, text: &'t [u8]) -> CaptureMatches<'r, 't> {
        CaptureMatches {
            regex: self,
            text,
            cursor: MatchCursor::default(),
            region: take_cached_region(),
        }
    }

    /// Iterate over the capture groups of all non-overlapping matches in
    /// `text` under per-search `options`.
    ///
    /// The options apply to each search the iterator runs. A search that
    /// reaches a limit yields its error once and ends the iteration.
    pub fn captures_iter_with<'r, 't>(
        &'r self,
        text: &'t str,
        options: SearchOptions,
    ) -> TryCaptureMatches<'r, 't> {
        self.captures_iter_bytes_with(text.as_bytes(), options)
    }

    /// Iterate over the capture groups of all non-overlapping matches in
    /// `text` (as bytes) under per-search `options`.
    pub fn captures_iter_bytes_with<'r, 't>(
        &'r self,
        text: &'t [u8],
        options: SearchOptions,
    ) -> TryCaptureMatches<'r, 't> {
        TryCaptureMatches {
            regex: self,
            text,
            cursor: MatchCursor::default(),
            region: take_cached_region(),
            options,
            finished: false,
        }
    }

    fn search_with(
        &self,
        text: &[u8],
        start: usize,
        range: usize,
        region: Option<OnigRegion>,
        options: SearchOptions,
    ) -> Result<(i32, Option<OnigRegion>), RegexError> {
        let (result, region) = onig_search_with_param(
            self.program(),
            text,
            text.len(),
            start,
            range,
            region,
            ONIG_OPTION_NONE,
            &options.match_param(),
        );
        if result < 0 && result != ONIG_MISMATCH {
            if let Some(region) = region {
                cache_region(region);
            }
            return Err(RegexError::from(result));
        }
        Ok((result, region))
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

    /// Compile the pattern into a [`Regex`].
    pub fn build(self) -> Result<Regex, RegexError> {
        let inner = onig_new_with_backtracking_optimization(
            &self.pattern,
            self.options,
            &ONIG_ENCODING_UTF8,
            self.syntax,
            self.optimize_backtracking,
        )?;
        if self.reject_backtracking_risks && !inner.backtrack_warnings.is_empty() {
            return Err(ONIGERR_VERY_INEFFICIENT_PATTERN.into());
        }
        Ok(Regex::from_parts(inner, self.pattern.into_boxed_slice()))
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

// === Match ===

/// A single match result referencing the original text.
#[derive(Debug, Clone, Copy)]
pub struct Match<'t> {
    text: &'t [u8],
    start: usize,
    end: usize,
}

impl<'t> Match<'t> {
    /// Build a match from one region entry, or `None` if the entry is not a
    /// range of `text`: unset (negative), past the end, or with start > end.
    /// Oniguruma can report start > end for a capture whose group started
    /// again but failed before closing; every `Match` upholds
    /// `start <= end <= text.len()`, so its accessors never panic on slicing.
    fn from_region(text: &'t [u8], beg: i32, end: i32) -> Option<Self> {
        let start = usize::try_from(beg).ok()?;
        let end = usize::try_from(end).ok()?;
        (start <= end && end <= text.len()).then_some(Match { text, start, end })
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
        &self.text[self.start..self.end]
    }

    /// The matched text as a `&str`.
    ///
    /// Use [`Match::as_bytes`] when the match may be invalid UTF-8; see
    /// [the guide](https://ferroni.dev/guide/coming-from-regex).
    ///
    /// # Panics
    ///
    /// Panics if the matched bytes are not valid UTF-8.
    pub fn as_str(&self) -> &'t str {
        std::str::from_utf8(self.as_bytes()).expect("match is not valid UTF-8")
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
    text: &'t [u8],
    region: OnigRegion,
    compiled: Arc<Compiled>,
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
        let num = onig_name_to_backref_number(
            &self.compiled.program,
            name.as_bytes(),
            Some(&self.region),
        )
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
/// same empty-match rule.
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
        Verdict::Yield
    }
}

// === FindIter ===

/// Iterator over all non-overlapping matches in a text.
pub struct FindIter<'r, 't> {
    regex: &'r Regex,
    text: &'t [u8],
    cursor: MatchCursor,
    region: OnigRegion,
    /// Set when the searches need no region (`onig_search_bounds`).
    msa: Option<Box<MatchArg>>,
}

impl<'r, 't> Iterator for FindIter<'r, 't> {
    type Item = Match<'t>;

    fn next(&mut self) -> Option<Match<'t>> {
        if self.cursor.last_end > self.text.len() {
            return None;
        }

        let m = if let Some(msa) = self.msa.as_deref_mut() {
            let (start, end) =
                onig_search_bounds(self.regex.program(), self.text, self.cursor.last_end, msa)
                    .ok()??;
            (end <= self.text.len()).then_some(Match {
                text: self.text,
                start,
                end,
            })?
        } else {
            let (result, region) = onig_search(
                self.regex.program(),
                self.text,
                self.text.len(),
                self.cursor.last_end,
                self.text.len(),
                Some(std::mem::take(&mut self.region)),
                ONIG_OPTION_NONE,
            );
            self.region = region?;

            if result < 0 {
                return None;
            }
            if self.region.num_regs < 1 {
                return None;
            }

            Match::from_region(self.text, self.region.beg[0], self.region.end[0])?
        };

        match self
            .cursor
            .accept(self.regex.program(), self.text, m.start, m.end)
        {
            Verdict::Yield => Some(m),
            Verdict::Retry => self.next(),
            Verdict::Finish => None,
        }
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
    regex: &'r Regex,
    text: &'t [u8],
    cursor: MatchCursor,
    region: OnigRegion,
    options: SearchOptions,
    finished: bool,
}

impl<'r, 't> Iterator for TryFindIter<'r, 't> {
    type Item = Result<Match<'t>, RegexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished || self.cursor.last_end > self.text.len() {
            self.finished = true;
            return None;
        }

        let search = self.regex.search_with(
            self.text,
            self.cursor.last_end,
            self.text.len(),
            Some(std::mem::take(&mut self.region)),
            self.options,
        );
        let (result, region) = match search {
            Ok(result) => result,
            Err(error) => {
                self.finished = true;
                return Some(Err(error));
            }
        };
        let Some(region) = region else {
            self.finished = true;
            return None;
        };
        self.region = region;

        if result < 0 {
            self.finished = true;
            return None;
        }
        if self.region.num_regs < 1 {
            self.finished = true;
            return None;
        }

        let Some(m) = Match::from_region(self.text, self.region.beg[0], self.region.end[0]) else {
            self.finished = true;
            return None;
        };

        match self
            .cursor
            .accept(self.regex.program(), self.text, m.start, m.end)
        {
            Verdict::Yield => Some(Ok(m)),
            Verdict::Retry => self.next(),
            Verdict::Finish => {
                self.finished = true;
                None
            }
        }
    }
}

impl std::iter::FusedIterator for TryFindIter<'_, '_> {}

impl Drop for TryFindIter<'_, '_> {
    fn drop(&mut self) {
        cache_region(std::mem::take(&mut self.region));
    }
}

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
    text: &'t [u8],
    cursor: MatchCursor,
    /// The region for the next search. Empty once a yielded [`Captures`] has
    /// taken it.
    region: OnigRegion,
}

impl<'r, 't> Iterator for CaptureMatches<'r, 't> {
    type Item = Captures<'t>;

    fn next(&mut self) -> Option<Captures<'t>> {
        if self.cursor.last_end > self.text.len() {
            return None;
        }

        let (result, region) = onig_search(
            self.regex.program(),
            self.text,
            self.text.len(),
            self.cursor.last_end,
            self.text.len(),
            Some(std::mem::take(&mut self.region)),
            ONIG_OPTION_NONE,
        );
        let region = region?;
        if result < 0 || region.num_regs < 1 {
            self.region = region;
            return None;
        }
        let Some(m) = Match::from_region(self.text, region.beg[0], region.end[0]) else {
            self.region = region;
            return None;
        };

        match self
            .cursor
            .accept(self.regex.program(), self.text, m.start, m.end)
        {
            Verdict::Yield => Some(Captures {
                text: self.text,
                region,
                compiled: Arc::clone(&self.regex.inner),
            }),
            Verdict::Retry => {
                self.region = region;
                self.next()
            }
            Verdict::Finish => {
                self.region = region;
                None
            }
        }
    }
}

impl Drop for CaptureMatches<'_, '_> {
    fn drop(&mut self) {
        cache_region(std::mem::take(&mut self.region));
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
    regex: &'r Regex,
    text: &'t [u8],
    cursor: MatchCursor,
    /// The region for the next search. Empty once a yielded [`Captures`] has
    /// taken it.
    region: OnigRegion,
    options: SearchOptions,
    finished: bool,
}

impl<'r, 't> Iterator for TryCaptureMatches<'r, 't> {
    type Item = Result<Captures<'t>, RegexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished || self.cursor.last_end > self.text.len() {
            self.finished = true;
            return None;
        }

        let search = self.regex.search_with(
            self.text,
            self.cursor.last_end,
            self.text.len(),
            Some(std::mem::take(&mut self.region)),
            self.options,
        );
        let (result, region) = match search {
            Ok(result) => result,
            Err(error) => {
                self.finished = true;
                return Some(Err(error));
            }
        };
        let Some(region) = region else {
            self.finished = true;
            return None;
        };
        if result < 0 || region.num_regs < 1 {
            self.finished = true;
            self.region = region;
            return None;
        }
        let Some(m) = Match::from_region(self.text, region.beg[0], region.end[0]) else {
            self.finished = true;
            self.region = region;
            return None;
        };

        match self
            .cursor
            .accept(self.regex.program(), self.text, m.start, m.end)
        {
            Verdict::Yield => Some(Ok(Captures {
                text: self.text,
                region,
                compiled: Arc::clone(&self.regex.inner),
            })),
            Verdict::Retry => {
                self.region = region;
                self.next()
            }
            Verdict::Finish => {
                self.finished = true;
                self.region = region;
                None
            }
        }
    }
}

impl std::iter::FusedIterator for TryCaptureMatches<'_, '_> {}

impl Drop for TryCaptureMatches<'_, '_> {
    fn drop(&mut self) {
        cache_region(std::mem::take(&mut self.region));
    }
}

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

    #[test]
    fn compiled_regex_types_are_send_and_sync() {
        assert_send_sync::<Regex>();
        assert_send_sync::<crate::regset::OnigRegSet>();
        assert_send_sync::<crate::scanner::Scanner>();
    }

    #[test]
    fn public_types_implement_debug() {
        use crate::scanner::{OnigString, Scanner, ScannerConfig, ScannerPatternCache};

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
                    text: text.as_bytes(),
                    cursor: MatchCursor::default(),
                    region: OnigRegion::new(),
                    msa: None,
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

    #[test]
    fn regex_new_and_find() {
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
        let re = Regex::new(r"\d+").unwrap();
        assert!(re.find("no digits here").is_none());
    }

    #[test]
    fn regex_is_match() {
        let re = Regex::new(r"hello").unwrap();
        assert!(re.is_match("say hello"));
        assert!(!re.is_match("say goodbye"));
    }

    #[test]
    fn search_options_methods_return_matches_and_captures() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
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
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
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
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
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

    #[test]
    fn search_options_surface_retry_limit_errors() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
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

    #[test]
    fn search_options_leave_unset_limits_process_wide() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
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
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
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
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
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
        let re = Regex::new(r"\d+").unwrap();
        let matches: Vec<&str> = re.find_iter("1 + 22 = 333").map(|m| m.as_str()).collect();
        assert_eq!(matches, vec!["1", "22", "333"]);
    }

    #[test]
    fn regex_builder_case_insensitive() {
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
        let re = Regex::new(r"world").unwrap();
        let m = re.find("hello world").unwrap();
        assert_eq!(m.as_bytes(), b"world");
    }

    #[test]
    fn captures_iter() {
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
        let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
        let caps = re.captures("2026-02").unwrap();
        assert_eq!(caps.name("year").unwrap().as_str(), "2026");
        assert_eq!(caps.name("month").unwrap().as_str(), "02");
        assert!(caps.name("day").is_none());
    }

    #[test]
    fn empty_match_find_iter() {
        let re = Regex::new(r"").unwrap();
        let matches: Vec<_> = re.find_iter("ab").collect();
        // Should yield empty matches at positions 0, 1, 2
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].start(), 0);
        assert_eq!(matches[1].start(), 1);
        assert_eq!(matches[2].start(), 2);
    }
}
