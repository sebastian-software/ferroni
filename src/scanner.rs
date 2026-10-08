// scanner.rs - High-level multi-pattern scanner API.
//
// Compatible with vscode-oniguruma's OnigScanner interface, used by Shiki
// and other syntax highlighters built on vscode-textmate.
//
// Scanner API design and test cases derived from vscode-oniguruma
// (MIT License, Copyright (c) Microsoft Corporation).

use smallvec::SmallVec;

use crate::api::Syntax;
use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::error::RegexError;
use crate::oniguruma::*;
use crate::regcomp::onig_new_with_backtracking_optimization;
use crate::regexec::{onig_get_global_limit_revision, onig_get_retry_limit_in_search};
use crate::regint::{ANCR_ANYCHAR_INF, RegexType};
use crate::regset::{
    FallbackMemoIdentity, OnigRegSet, OnigRegSetLead, RegSetEntryEvent, onig_regset_entry_search,
    onig_regset_get_regex, onig_regset_last_match_len, onig_regset_new_shared,
    onig_regset_number_of_regex, onig_regset_search_utf8, onig_regset_swap_region,
};
use std::collections::HashMap;
use std::fmt;
use std::ops::{BitOr, BitOrAssign};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ONIG_STRING_ID: AtomicU64 = AtomicU64::new(1);

/// Result of a capture group match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureIndex {
    /// Byte offset of the start of the capture.
    pub start: usize,
    /// Byte offset of the end of the capture.
    pub end: usize,
    /// Length of the capture in bytes (`end - start`).
    pub length: usize,
}

/// Result of a scanner match.
///
/// Read the capture groups with [`captures`](Self::captures).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScannerMatch {
    /// Index of the pattern that matched (0-based).
    pub index: usize,
    /// Capture group information. Index 0 is the full match. Read it with
    /// [`captures`](Self::captures).
    capture_indices: SmallVec<[CaptureIndex; 8]>,
}

impl ScannerMatch {
    /// The capture groups of the match as a slice. Index 0 is the full match.
    ///
    /// ```
    /// use ferroni::scanner::{Scanner, ScannerFindOptions};
    ///
    /// let mut scanner = Scanner::new(&[r"(\d)(\d)"]).unwrap();
    /// let m = scanner.find_next_match("x42", 0, ScannerFindOptions::NONE).unwrap();
    /// assert_eq!(m.captures().len(), 3);
    /// assert_eq!((m.captures()[0].start, m.captures()[0].end), (1, 3));
    /// ```
    pub fn captures(&self) -> &[CaptureIndex] {
        &self.capture_indices
    }
}

/// Options for `Scanner::find_next_match`, matching vscode-oniguruma's `FindOption`.
///
/// Options combine with `|` and `|=`:
///
/// ```
/// use ferroni::scanner::{Scanner, ScannerFindOptions};
///
/// let mut scanner = Scanner::new(&[r"\Aab\z"]).unwrap();
/// let mut options = ScannerFindOptions::NOT_BEGIN_STRING;
/// options |= ScannerFindOptions::NOT_END_STRING;
/// assert!(scanner.find_next_match("ab", 0, options).is_none());
/// assert!(scanner.find_next_match("ab", 0, ScannerFindOptions::NONE).is_some());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScannerFindOptions(u32);

impl ScannerFindOptions {
    /// No options: the search treats the text as a whole string.
    pub const NONE: Self = Self(0);
    /// The start of the text is not the start of a string, so `\A` does not
    /// match there.
    pub const NOT_BEGIN_STRING: Self = Self(1);
    /// The end of the text is not the end of a string, so `\z` and `\Z` do
    /// not match there.
    pub const NOT_END_STRING: Self = Self(2);
    /// The search start position is not the start of the search, so `\G`
    /// does not match there.
    pub const NOT_BEGIN_POSITION: Self = Self(4);

    /// Create from a raw bitmask.
    pub fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    fn to_onig_options(self) -> OnigOptionType {
        let mut opts = ONIG_OPTION_NONE;
        if self.0 & 1 != 0 {
            opts |= ONIG_OPTION_NOT_BEGIN_STRING;
        }
        if self.0 & 2 != 0 {
            opts |= ONIG_OPTION_NOT_END_STRING;
        }
        if self.0 & 4 != 0 {
            opts |= ONIG_OPTION_NOT_BEGIN_POSITION;
        }
        opts
    }
}

impl BitOr for ScannerFindOptions {
    type Output = Self;

    /// Combine two sets of options. Unknown bits from [`from_bits`](Self::from_bits)
    /// are kept as they are.
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ScannerFindOptions {
    /// Add the options of `rhs` to `self`.
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Regex syntax variant, matching vscode-oniguruma's `Syntax` enum.
///
/// This is the same type as [`crate::api::Syntax`], so one value can select a
/// syntax for both [`crate::api::RegexBuilder::syntax_mode`] and a scanner.
///
/// ```
/// use ferroni::api::Syntax;
/// use ferroni::scanner::ScannerSyntax;
///
/// let syntax: ScannerSyntax = Syntax::Ruby;
/// assert_eq!(syntax, ScannerSyntax::Ruby);
/// ```
pub type ScannerSyntax = Syntax;

/// Configuration for creating a `Scanner`, matching vscode-oniguruma's `IOnigScannerConfig`.
///
/// The default options enable unnamed captures when a pattern also contains
/// named groups, matching vscode-oniguruma's default `CaptureGroup` option.
///
/// Build a configuration from [`ScannerConfig::default`] and the chainable
/// [`options`](Self::options) and [`syntax`](Self::syntax) setters. The struct
/// is `#[non_exhaustive]`, so a struct literal cannot be written outside this
/// crate, and new settings can be added without a breaking change.
///
/// ```
/// use ferroni::api::Syntax;
/// use ferroni::scanner::ScannerConfig;
///
/// let config = ScannerConfig::default().syntax(Syntax::Ruby);
/// assert_eq!(config.syntax, Syntax::Ruby);
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ScannerConfig {
    /// Compile-time options applied to all patterns. Defaults to
    /// [`ONIG_OPTION_CAPTURE_GROUP`].
    pub options: OnigOptionType,
    /// Regex syntax variant to use.
    pub syntax: ScannerSyntax,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        ScannerConfig {
            options: ONIG_OPTION_CAPTURE_GROUP,
            syntax: ScannerSyntax::default(),
        }
    }
}

impl ScannerConfig {
    /// Replace the compile-time options applied to all patterns. The default
    /// is [`ONIG_OPTION_CAPTURE_GROUP`].
    ///
    /// ```
    /// use ferroni::oniguruma::ONIG_OPTION_IGNORECASE;
    /// use ferroni::scanner::{Scanner, ScannerConfig, ScannerFindOptions};
    ///
    /// let config = ScannerConfig::default().options(ONIG_OPTION_IGNORECASE);
    /// let mut scanner = Scanner::with_config(&["hello"], &config).unwrap();
    /// assert!(scanner.find_next_match("HELLO", 0, ScannerFindOptions::NONE).is_some());
    /// ```
    pub const fn options(mut self, options: OnigOptionType) -> Self {
        self.options = options;
        self
    }

    /// Replace the regex syntax used for all patterns. The default is
    /// [`ScannerSyntax::Oniguruma`].
    ///
    /// ```
    /// use ferroni::scanner::{Scanner, ScannerConfig, ScannerFindOptions, ScannerSyntax};
    ///
    /// let config = ScannerConfig::default().syntax(ScannerSyntax::Ruby);
    /// let mut scanner = Scanner::with_config(&[r"\w+"], &config).unwrap();
    /// assert!(scanner.find_next_match("ok", 0, ScannerFindOptions::NONE).is_some());
    /// ```
    pub const fn syntax(mut self, syntax: ScannerSyntax) -> Self {
        self.syntax = syntax;
        self
    }
}

/// Compiled patterns that scanners share, owned by the caller.
///
/// A TextMate grammar creates one scanner per rule context, and the same
/// pattern strings recur across them: the C++ grammar's scanners in a
/// captured highlighting session hold 4,026 patterns, 250 of them distinct.
/// [`Scanner::new`] and [`Scanner::with_config`] compile every pattern they
/// are given. [`Scanner::with_pattern_cache`] compiles a pattern only if the
/// cache does not hold it yet, adds it, and shares the compiled program with
/// every scanner built from the same cache.
///
/// A compiled pattern is reused only for the same pattern text, compile
/// options, syntax and backtracking optimization choice. Each scanner keeps
/// its own search state, so a scanner built from a cache returns exactly
/// what a scanner built without one returns, including its
/// [`warnings`](Scanner::warnings) and
/// [`backtracking_rewrites`](Scanner::backtracking_rewrites).
///
/// # Scope
///
/// The cache knows nothing about grammars: every scanner built from it
/// shares its patterns, so its owner decides how far sharing reaches. One
/// cache for all the grammars a highlighter loads also shares patterns
/// between grammars. Related grammars repeat each other's patterns (in the
/// Shiki grammar collection, TypeScript and TSX have 351 of their 362 and
/// 375 regex source strings in common), and a grammar that embeds another,
/// such as Vue or Markdown, builds scanners over the embedded grammar's
/// patterns again. A cache per grammar shares only within that grammar and
/// is freed with it.
///
/// # Lifetime
///
/// The cache and the scanners built from it hold strong references to the
/// compiled patterns: a pattern is freed when the last of them is dropped.
/// [`clear`](Self::clear) or dropping the cache releases only the cache's
/// own references and never affects scanners already built. The cache does
/// not evict anything by itself; there is no process-wide cache.
///
/// Patterns are compiled under the process-wide compile settings in effect
/// when the cache first compiles them (`onig_set_capture_num_limit`,
/// `onig_set_parse_depth_limit`, user-defined Unicode properties, callout
/// names), and the warning callbacks (`onig_set_warn_func`) run only then.
/// Clear the cache after changing those settings. Search limits such as the
/// retry limit are read by every search, so they apply to all scanners alike.
///
/// # Threads
///
/// The cache is `Send` and `Sync`, and a scanner built from it is `Send`
/// and `Sync` like any other. Searches only read a shared compiled pattern.
/// Building a scanner takes `&mut` access to the cache; to build scanners
/// from one cache on several threads, put the cache behind a `Mutex`.
///
/// # Example
///
/// ```
/// use ferroni::scanner::{Scanner, ScannerConfig, ScannerFindOptions, ScannerPatternCache};
///
/// let config = ScannerConfig::default();
/// let mut cache = ScannerPatternCache::new();
/// let mut top_level =
///     Scanner::with_pattern_cache(&[r"\bfn\b", r#""[^"]*""#], &config, &mut cache).unwrap();
/// let mut in_call =
///     Scanner::with_pattern_cache(&[r"\)", r#""[^"]*""#], &config, &mut cache).unwrap();
/// // The string pattern was compiled once, for both scanners.
/// assert_eq!(cache.len(), 3);
///
/// let m = in_call.find_next_match(r#"("a")"#, 0, ScannerFindOptions::NONE).unwrap();
/// assert_eq!((m.index, m.captures()[0].start), (1, 1));
/// let m = top_level.find_next_match("fn f", 0, ScannerFindOptions::NONE).unwrap();
/// assert_eq!(m.index, 0);
/// ```
#[derive(Default)]
pub struct ScannerPatternCache {
    /// The compiled patterns of each compile setting, in first-use order.
    /// A grammar usually compiles all its patterns with one setting.
    compiled: Vec<(PatternSettings, CompiledPatterns)>,
}

/// Compiled programs by pattern text, for one `PatternSettings`.
type CompiledPatterns = HashMap<Box<str>, Arc<RegexType>>;

impl ScannerPatternCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of compiled patterns the cache holds. A pattern compiled
    /// under two configurations counts twice.
    pub fn len(&self) -> usize {
        self.compiled
            .iter()
            .map(|(_, patterns)| patterns.len())
            .sum()
    }

    /// Whether the cache holds no compiled pattern.
    pub fn is_empty(&self) -> bool {
        self.compiled
            .iter()
            .all(|(_, patterns)| patterns.is_empty())
    }

    /// Drop the cache's references to every compiled pattern. Scanners
    /// built from it keep theirs.
    pub fn clear(&mut self) {
        self.compiled.clear();
    }

    /// Build a scanner from the cached patterns, compiling and adding the
    /// others. If construction fails, the cache is left as it was.
    fn scanner(
        &mut self,
        patterns: &[&str],
        settings: PatternSettings,
    ) -> Result<Scanner, RegexError> {
        let at = match self.compiled.iter().position(|(s, _)| *s == settings) {
            Some(at) => at,
            None => {
                self.compiled.push((settings, CompiledPatterns::new()));
                self.compiled.len() - 1
            }
        };
        let compiled = &mut self.compiled[at].1;
        let mut added = Vec::new();
        let scanner = Scanner::compile(patterns, settings, |pattern| {
            if let Some(reg) = compiled.get(pattern) {
                return Ok(Arc::clone(reg));
            }
            let reg = settings.compile(pattern)?;
            compiled.insert(pattern.into(), Arc::clone(&reg));
            added.push(pattern);
            Ok(reg)
        });
        if scanner.is_err() {
            for pattern in added {
                compiled.remove(pattern);
            }
        }
        if compiled.is_empty() {
            self.compiled.remove(at);
        }
        scanner
    }
}

/// Shows the number of cached patterns, as [`ScannerPatternCache::len`] does.
impl fmt::Debug for ScannerPatternCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScannerPatternCache")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

/// What a compiled scanner pattern depends on besides its text.
#[derive(Clone, Copy, PartialEq, Eq)]
struct PatternSettings {
    options: OnigOptionType,
    syntax: ScannerSyntax,
    optimize_backtracking: bool,
}

impl PatternSettings {
    fn of(config: &ScannerConfig, optimize_backtracking: bool) -> Self {
        PatternSettings {
            options: config.options,
            syntax: config.syntax,
            optimize_backtracking,
        }
    }

    fn compile(self, pattern: &str) -> Result<Arc<RegexType>, RegexError> {
        onig_new_with_backtracking_optimization(
            pattern.as_bytes(),
            self.options,
            &ONIG_ENCODING_UTF8,
            self.syntax.as_onig_syntax(),
            self.optimize_backtracking,
        )
        .map(Arc::new)
    }
}

/// A string wrapper that maintains UTF-16 ↔ UTF-8 offset mappings.
///
/// JavaScript strings are UTF-16 encoded, while Ferroni operates on UTF-8.
/// `OnigString` bridges this gap by precomputing offset tables, enabling
/// the scanner to accept UTF-16 positions (as used by vscode-textmate/Shiki)
/// and return results in UTF-16 positions.
///
/// # Example
///
/// ```
/// use ferroni::scanner::OnigString;
///
/// let s = OnigString::new("a💻b");
/// assert_eq!(s.utf16_len(), 4); // a(1) + 💻(2) + b(1) = 4 UTF-16 code units
/// assert_eq!(s.content().len(), 6); // a(1) + 💻(4) + b(1) = 6 UTF-8 bytes
/// ```
pub struct OnigString {
    cache_id: u64,
    content: String,
    is_ascii: bool,
    utf16_len: usize,
    /// Maps UTF-16 code unit index → UTF-8 byte offset. Length = utf16_len + 1.
    utf16_to_utf8: Vec<usize>,
    /// Maps UTF-8 byte offset → UTF-16 code unit index. Length = utf8_len + 1.
    utf8_to_utf16: Vec<usize>,
}

impl OnigString {
    /// Create a new `OnigString` from a Rust string, building offset tables.
    pub fn new(content: &str) -> Self {
        let cache_id = NEXT_ONIG_STRING_ID.fetch_add(1, Ordering::Relaxed);
        if content.is_ascii() {
            let len = content.len();
            return OnigString {
                cache_id,
                content: content.to_string(),
                is_ascii: true,
                utf16_len: len,
                utf16_to_utf8: Vec::new(),
                utf8_to_utf16: Vec::new(),
            };
        }

        let utf8_len = content.len();
        let utf16_len: usize = content.chars().map(|c| c.len_utf16()).sum();

        let mut utf16_to_utf8 = Vec::with_capacity(utf16_len + 1);
        let mut utf8_to_utf16 = vec![0usize; utf8_len + 1];

        let mut utf8_pos = 0;
        for ch in content.chars() {
            let u8_len = ch.len_utf8();
            let u16_len = ch.len_utf16();

            // First UTF-16 code unit maps to the start of the UTF-8 sequence
            utf16_to_utf8.push(utf8_pos);

            let utf16_pos = utf16_to_utf8.len() - 1;
            // All UTF-8 bytes of this char map to the same UTF-16 position
            for b in 0..u8_len {
                utf8_to_utf16[utf8_pos + b] = utf16_pos;
            }

            if u16_len == 2 {
                // Surrogate pair: low surrogate maps to byte AFTER this char
                utf16_to_utf8.push(utf8_pos + u8_len);
            }

            utf8_pos += u8_len;
        }

        // Sentinels for end-of-string positions
        utf16_to_utf8.push(utf8_pos);
        utf8_to_utf16[utf8_pos] = utf16_len;

        OnigString {
            cache_id,
            content: content.to_string(),
            is_ascii: false,
            utf16_len,
            utf16_to_utf8,
            utf8_to_utf16,
        }
    }

    /// The underlying UTF-8 string content.
    pub fn content(&self) -> &str {
        &self.content
    }

    #[inline]
    fn is_ascii(&self) -> bool {
        self.is_ascii
    }

    /// Length of the string in UTF-16 code units.
    pub fn utf16_len(&self) -> usize {
        self.utf16_len
    }

    /// Convert a UTF-16 code unit offset to a UTF-8 byte offset.
    fn utf16_offset_to_utf8(&self, utf16_offset: usize) -> usize {
        if self.is_ascii {
            utf16_offset.min(self.content.len())
        } else if utf16_offset >= self.utf16_to_utf8.len() {
            self.content.len()
        } else {
            self.utf16_to_utf8[utf16_offset]
        }
    }

    /// Convert a UTF-8 byte offset to a UTF-16 code unit offset.
    fn utf8_offset_to_utf16(&self, utf8_offset: usize) -> usize {
        if self.is_ascii {
            utf8_offset.min(self.content.len())
        } else if utf8_offset >= self.utf8_to_utf16.len() {
            self.utf16_len
        } else {
            self.utf8_to_utf16[utf8_offset]
        }
    }
}

/// Shows the content. The offset tables are left out.
impl fmt::Debug for OnigString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnigString")
            .field("content", &self.content)
            .finish_non_exhaustive()
    }
}

/// Per-regex cache entry, mirroring vscode-oniguruma's caching strategy.
struct CacheEntry {
    has_g_anchor: bool,
    /// The regex starts with an any-char star (`ANCR_ANYCHAR_INF`).
    after_newline_only: bool,
    last_str_id: u64,
    last_position: usize,
    last_options: u32,
    /// `onig_get_global_limit_revision()` when the result was found.
    last_limit_revision: u64,
    last_matched: bool,
    last_result: i32,
    last_region: Option<OnigRegion>,
}

impl CacheEntry {
    fn new(pattern: &str, anchor: i32) -> Self {
        CacheEntry {
            has_g_anchor: pattern.contains("\\G"),
            after_newline_only: (anchor & ANCR_ANYCHAR_INF) != 0,
            last_str_id: 0,
            last_position: 0,
            last_options: u32::MAX, // invalid sentinel
            last_limit_revision: 0,
            last_matched: false,
            last_result: ONIG_MISMATCH,
            last_region: None,
        }
    }

    /// Record the first match (or, with `None`, the absence of any match or
    /// error) of a search over the whole rest of the subject from `start`.
    fn remember(
        &mut self,
        str_id: u64,
        start: usize,
        options: u32,
        limit_revision: u64,
        found: Option<usize>,
    ) {
        self.last_str_id = str_id;
        self.last_position = start;
        self.last_options = options;
        self.last_limit_revision = limit_revision;
        self.last_matched = found.is_some();
        self.last_result = found.map_or(ONIG_MISMATCH, |position| position as i32);
    }

    fn forget(&mut self) {
        self.last_str_id = 0;
        self.last_position = 0;
        self.last_options = u32::MAX;
        self.last_matched = false;
        self.last_result = ONIG_MISMATCH;
    }
}

/// Lightweight scanner counters for profiling and routing diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScannerStats {
    /// Number of calls routed through RegSet.
    pub route_regset_calls: u64,
    /// Number of calls routed through per-regex search.
    pub route_per_regex_calls: u64,
    /// Number of cache-mode calls routed through RegSet.
    pub route_cache_regset_calls: u64,
    /// Number of cache-mode calls routed through per-regex search.
    pub route_cache_per_regex_calls: u64,
    /// Number of per-regex probes while routing was set to RegSet.
    pub route_cache_probe_calls: u64,
    /// Number of route switches from per-regex to RegSet.
    pub route_switch_to_regset: u64,
    /// Number of route switches from RegSet to per-regex.
    pub route_switch_to_per_regex: u64,
    /// Number of cache eligibility checks in per-regex search.
    pub cache_checks: u64,
    /// Number of per-regex cache hits.
    pub cache_hits: u64,
    /// Number of per-regex cache misses.
    pub cache_misses: u64,
    /// Number of VM searches executed in per-regex path.
    pub vm_search_calls: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CacheRoute {
    #[default]
    RegSet,
    PerRegex,
}

#[derive(Debug, Clone, Copy, Default)]
struct CacheRouteState {
    str_id: u64,
    options: u32,
    route: CacheRoute,
    calls_since_probe: u32,
    last_start: usize,
    same_start_streak: u16,
    poor_per_regex_streak: u8,
    good_per_regex_streak: u8,
}

#[derive(Debug, Clone, Copy, Default)]
struct PerRegexCallStats {
    cache_checks: u32,
    cache_hits: u32,
    vm_calls: u32,
}

impl PerRegexCallStats {
    #[inline]
    fn cache_misses(self) -> u32 {
        self.cache_checks.saturating_sub(self.cache_hits)
    }
}

const ROUTE_PROBE_EVERY: u32 = 8;
const ROUTE_MIN_SAME_START_FOR_PROBE: u16 = 8;
const ROUTE_POOR_STREAK_TO_REGSET: u8 = 3;
const ROUTE_GOOD_STREAK_TO_PER_REGEX: u8 = 2;
const SCANNER_STATS_ENABLED: bool = cfg!(any(test, debug_assertions));

/// Multi-pattern scanner compatible with vscode-oniguruma's `OnigScanner`.
///
/// # Example
///
/// ```
/// use ferroni::scanner::{Scanner, ScannerFindOptions};
///
/// let mut scanner = Scanner::new(&["\\d+", "[a-z]+"]).unwrap();
/// let m = scanner.find_next_match("hello42", 0, ScannerFindOptions::NONE).unwrap();
/// assert_eq!(m.index, 1); // "[a-z]+" matched first
/// assert_eq!(m.captures()[0].start, 0);
/// assert_eq!(m.captures()[0].end, 5);
/// ```
pub struct Scanner {
    caches: Vec<CacheEntry>,
    regset: Box<OnigRegSet>,
    stats: ScannerStats,
    cache_route: CacheRouteState,
    /// `onig_get_global_limit_revision()` when `search_budget` was read.
    limit_revision: Option<u64>,
    /// A search retry budget is set (`onig_set_retry_limit_in_search`).
    search_budget: bool,
    warnings: Vec<Vec<crate::backtrack_lint::BacktrackWarning>>,
    rewrites: Vec<Vec<crate::backtrack_rewrite::BacktrackingRewrite>>,
    /// The compile settings every pattern of this scanner was built with.
    settings: PatternSettings,
}

// Scanners hold their compiled patterns as `Arc<RegexType>`, shared or not.
// That is `Send` and `Sync` because `RegexType` is, from its fields alone, so
// a scanner can move to or be shared with another thread whether or not it
// came from a `ScannerPatternCache`. Neither is `UnwindSafe`: `RegexType`
// holds an `OnigEncoding` trait object, under a `Box` as under an `Arc`.
const _: () = {
    const fn send_sync<T: Send + Sync>() {}
    send_sync::<RegexType>();
    send_sync::<OnigRegSet>();
    send_sync::<Scanner>();
    send_sync::<ScannerPatternCache>();
};

/// Shows the pattern count and the settings the patterns were compiled with.
impl fmt::Debug for Scanner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scanner")
            .field("patterns", &self.caches.len())
            .field("options", &self.settings.options)
            .field("syntax", &self.settings.syntax)
            .field(
                "optimize_backtracking",
                &self.settings.optimize_backtracking,
            )
            .finish_non_exhaustive()
    }
}

impl Scanner {
    /// Create a scanner from a list of pattern strings using the
    /// vscode-oniguruma defaults (Oniguruma syntax and capture groups enabled).
    ///
    /// Enabling capture groups preserves unnamed captures and numbered
    /// backreferences when a pattern also contains named groups. Pass an
    /// explicit [`ScannerConfig`] to select different compile-time options.
    pub fn new(patterns: &[&str]) -> Result<Scanner, RegexError> {
        Self::with_config(patterns, &ScannerConfig::default())
    }

    /// Create a scanner with custom configuration (syntax and compile-time options).
    ///
    /// # Example
    ///
    /// ```
    /// use ferroni::scanner::{Scanner, ScannerConfig, ScannerSyntax, ScannerFindOptions};
    /// use ferroni::oniguruma::OnigOptionType;
    ///
    /// let config = ScannerConfig::default()
    ///     .options(OnigOptionType::IGNORECASE)
    ///     .syntax(ScannerSyntax::Oniguruma);
    /// let mut scanner = Scanner::with_config(&["hello"], &config).unwrap();
    /// let m = scanner.find_next_match("HELLO", 0, ScannerFindOptions::NONE);
    /// assert!(m.is_some());
    /// ```
    pub fn with_config(patterns: &[&str], config: &ScannerConfig) -> Result<Scanner, RegexError> {
        let settings = PatternSettings::of(config, false);
        Self::compile(patterns, settings, |pattern| settings.compile(pattern))
    }

    /// Create a scanner with the conservative, experimental AST rewrites
    /// described by [`crate::api::RegexBuilder::optimize_backtracking`].
    /// Successful captures and pattern priority are preserved; retry, stack,
    /// and timeout outcomes may differ. See [`Scanner::backtracking_rewrites`].
    pub fn with_backtracking_optimization(
        patterns: &[&str],
        config: &ScannerConfig,
    ) -> Result<Scanner, RegexError> {
        let settings = PatternSettings::of(config, true);
        Self::compile(patterns, settings, |pattern| settings.compile(pattern))
    }

    /// [`Scanner::with_config`], sharing compiled patterns through `cache`.
    ///
    /// A pattern the cache already holds for this `config` is reused
    /// without compiling it again; the others are compiled and added. The
    /// scanner behaves exactly like one from [`Scanner::with_config`]. If
    /// construction fails (a pattern does not compile, or the set is
    /// refused), the error is the one [`Scanner::with_config`] returns and
    /// the cache is left as it was. See [`ScannerPatternCache`] for lifetime
    /// and threads.
    ///
    /// # Example
    ///
    /// ```
    /// use ferroni::scanner::{Scanner, ScannerConfig, ScannerPatternCache};
    ///
    /// let config = ScannerConfig::default();
    /// let mut cache = ScannerPatternCache::new();
    /// let rules: [&[&str]; 3] = [&[r"\d+", r"\w+"], &[r"\w+", r"\s+"], &[r"\d+", r"\s+"]];
    /// let scanners: Vec<Scanner> = rules
    ///     .iter()
    ///     .map(|patterns| Scanner::with_pattern_cache(patterns, &config, &mut cache))
    ///     .collect::<Result<_, _>>()
    ///     .unwrap();
    /// // Six patterns, three of them distinct, each compiled once.
    /// assert_eq!((scanners.len(), cache.len()), (3, 3));
    ///
    /// // A pattern that does not compile leaves the cache unchanged.
    /// assert!(Scanner::with_pattern_cache(&["[a-z]+", "("], &config, &mut cache).is_err());
    /// assert_eq!(cache.len(), 3);
    /// ```
    pub fn with_pattern_cache(
        patterns: &[&str],
        config: &ScannerConfig,
        cache: &mut ScannerPatternCache,
    ) -> Result<Scanner, RegexError> {
        cache.scanner(patterns, PatternSettings::of(config, false))
    }

    /// [`Scanner::with_backtracking_optimization`], sharing compiled
    /// patterns through `cache` as [`Scanner::with_pattern_cache`] does.
    /// Patterns compiled with and without the optimization are cached
    /// separately.
    pub fn with_backtracking_optimization_and_pattern_cache(
        patterns: &[&str],
        config: &ScannerConfig,
        cache: &mut ScannerPatternCache,
    ) -> Result<Scanner, RegexError> {
        cache.scanner(patterns, PatternSettings::of(config, true))
    }

    /// Build a scanner from the compiled form of each pattern, which
    /// `regex` provides in pattern order. `settings` are the ones `regex`
    /// compiles with.
    fn compile<'p>(
        patterns: &[&'p str],
        settings: PatternSettings,
        mut regex: impl FnMut(&'p str) -> Result<Arc<RegexType>, RegexError>,
    ) -> Result<Scanner, RegexError> {
        let mut caches = Vec::with_capacity(patterns.len());
        let mut regset_regs = Vec::with_capacity(patterns.len());

        let mut warnings = Vec::with_capacity(patterns.len());
        let mut rewrites = Vec::with_capacity(patterns.len());

        for pattern in patterns {
            let reg = regex(pattern)?;
            rewrites.push(reg.backtrack_rewrites.clone());
            warnings.push(reg.backtrack_warnings.clone());
            caches.push(CacheEntry::new(pattern, reg.anchor));
            regset_regs.push(reg);
        }

        let (regset, r) = onig_regset_new_shared(regset_regs);
        if r != ONIG_NORMAL {
            return Err(r.into());
        }

        Ok(Scanner {
            caches,
            regset: regset.unwrap(),
            stats: ScannerStats::default(),
            cache_route: CacheRouteState::default(),
            limit_revision: None,
            search_budget: false,
            warnings,
            rewrites,
            settings,
        })
    }

    /// Findings of the compile-time backtracking check, one list per
    /// pattern in the order they were given. Grammar loaders can surface
    /// these to grammar authors. See [`crate::backtrack_lint`] for limits.
    ///
    /// ```
    /// use ferroni::scanner::Scanner;
    ///
    /// let scanner = Scanner::new(&["ok+", "(a+)+$"]).unwrap();
    /// assert!(scanner.warnings()[0].is_empty());
    /// assert_eq!(scanner.warnings()[1].len(), 1);
    /// ```
    pub fn warnings(&self) -> &[Vec<crate::backtrack_lint::BacktrackWarning>] {
        &self.warnings
    }

    /// Rewrite reports for each pattern, in the original pattern order.
    /// Unsupported shapes have no report; original warnings stay in
    /// [`Scanner::warnings`], including for successfully rewritten patterns.
    pub fn backtracking_rewrites(&self) -> &[Vec<crate::backtrack_rewrite::BacktrackingRewrite>] {
        &self.rewrites
    }

    /// Get current scanner counters.
    pub fn stats(&self) -> ScannerStats {
        self.stats
    }

    /// Reset scanner counters.
    pub fn reset_stats(&mut self) {
        self.stats = ScannerStats::default();
    }

    /// Find the next match starting at `start_position` (byte offset).
    ///
    /// One-off searches (without a stable string ID) use the RegSet path.
    /// Use `find_next_match_with_id` to enable per-regex cache reuse when
    /// repeatedly advancing through the same string.
    pub fn find_next_match(
        &mut self,
        text: &str,
        start_position: usize,
        options: ScannerFindOptions,
    ) -> Option<ScannerMatch> {
        self.find_next_match_inner(text, 0, start_position, options, false, None)
    }

    /// Find the next match with a string ID for caching.
    ///
    /// When searching the same string repeatedly (advancing `start_position`),
    /// pass the same `str_id` to enable cache hits that skip redundant searches.
    pub fn find_next_match_with_id(
        &mut self,
        text: &str,
        str_id: u64,
        start_position: usize,
        options: ScannerFindOptions,
    ) -> Option<ScannerMatch> {
        self.find_next_match_inner(
            text,
            str_id,
            start_position,
            options,
            true,
            Some(FallbackMemoIdentity::Caller(str_id)),
        )
    }

    /// Find the next match using UTF-16 positions (for vscode-textmate/Shiki compatibility).
    ///
    /// `start_position` is in UTF-16 code units. The returned `CaptureIndex` values
    /// (start, end, length) are also in UTF-16 code units.
    ///
    /// # Example
    ///
    /// ```
    /// use ferroni::scanner::{Scanner, ScannerFindOptions, OnigString};
    ///
    /// let mut scanner = Scanner::new(&["Y", "X"]).unwrap();
    /// let s = OnigString::new("a💻bYX");
    /// // 💻 is 2 UTF-16 code units, so Y is at UTF-16 position 4
    /// let m = scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE).unwrap();
    /// assert_eq!(m.captures()[0].start, 4);
    /// assert_eq!(m.captures()[0].end, 5);
    /// ```
    pub fn find_next_match_utf16(
        &mut self,
        string: &OnigString,
        start_position: usize,
        options: ScannerFindOptions,
    ) -> Option<ScannerMatch> {
        if string.is_ascii() {
            let start = start_position.min(string.content().len());
            return self.find_next_match_inner(
                string.content(),
                0,
                start,
                options,
                false,
                Some(FallbackMemoIdentity::OnigString(string.cache_id)),
            );
        }
        let utf8_start = string.utf16_offset_to_utf8(start_position);
        let m = self.find_next_match_inner(
            string.content(),
            0,
            utf8_start,
            options,
            false,
            Some(FallbackMemoIdentity::OnigString(string.cache_id)),
        )?;
        Some(convert_match_to_utf16(string, m))
    }

    /// Find the next match using UTF-16 positions with a string ID for caching.
    pub fn find_next_match_utf16_with_id(
        &mut self,
        string: &OnigString,
        str_id: u64,
        start_position: usize,
        options: ScannerFindOptions,
    ) -> Option<ScannerMatch> {
        if string.is_ascii() {
            let start = start_position.min(string.content().len());
            return self.find_next_match_inner(
                string.content(),
                str_id,
                start,
                options,
                true,
                Some(FallbackMemoIdentity::Caller(str_id)),
            );
        }
        let utf8_start = string.utf16_offset_to_utf8(start_position);
        let m = self.find_next_match_inner(
            string.content(),
            str_id,
            utf8_start,
            options,
            true,
            Some(FallbackMemoIdentity::Caller(str_id)),
        )?;
        Some(convert_match_to_utf16(string, m))
    }

    fn find_next_match_inner(
        &mut self,
        text: &str,
        str_id: u64,
        start_position: usize,
        options: ScannerFindOptions,
        use_cache: bool,
        fallback_memo_id: Option<FallbackMemoIdentity>,
    ) -> Option<ScannerMatch> {
        let str_data = text.as_bytes();
        let end = str_data.len();

        if start_position > end {
            return None;
        }

        let onig_opts = options.to_onig_options();

        // One-off calls always use RegSet.
        if !use_cache {
            if SCANNER_STATS_ENABLED {
                self.stats.route_regset_calls += 1;
            }
            return self.search_regset(str_data, end, start_position, onig_opts, fallback_memo_id);
        }

        let use_regset = self.should_use_regset_for_cache(str_id, options.0, start_position);
        // So does every call while a search retry budget makes a result
        // depend on where its search began (see `search_per_regex`).
        let limit_revision = if use_regset {
            0
        } else {
            let revision = onig_get_global_limit_revision();
            if self.limit_revision != Some(revision) {
                self.limit_revision = Some(revision);
                self.search_budget = onig_get_retry_limit_in_search() != 0;
            }
            revision
        };
        if use_regset || self.search_budget {
            if SCANNER_STATS_ENABLED {
                self.stats.route_regset_calls += 1;
                self.stats.route_cache_regset_calls += 1;
            }
            self.search_regset(str_data, end, start_position, onig_opts, fallback_memo_id)
        } else {
            if SCANNER_STATS_ENABLED {
                self.stats.route_per_regex_calls += 1;
                self.stats.route_cache_per_regex_calls += 1;
                if self.cache_route.route == CacheRoute::RegSet {
                    self.stats.route_cache_probe_calls += 1;
                }
            }
            let (m, run_stats) = self.search_per_regex(
                str_data,
                end,
                start_position,
                str_id,
                options.0,
                onig_opts,
                use_cache,
                limit_revision,
            );
            self.observe_per_regex_outcome(run_stats);
            m
        }
    }

    #[inline]
    fn should_use_regset_for_cache(
        &mut self,
        str_id: u64,
        options_raw: u32,
        start_position: usize,
    ) -> bool {
        if self.cache_route.str_id != str_id || self.cache_route.options != options_raw {
            self.cache_route.options = options_raw;
            self.cache_route.str_id = str_id;
            self.cache_route.route = CacheRoute::RegSet;
            self.cache_route.calls_since_probe = 0;
            self.cache_route.last_start = start_position;
            self.cache_route.same_start_streak = 1;
            self.cache_route.good_per_regex_streak = 0;
            self.cache_route.poor_per_regex_streak = 0;
            return true;
        }

        if start_position == self.cache_route.last_start {
            self.cache_route.same_start_streak =
                self.cache_route.same_start_streak.saturating_add(1);
        } else {
            self.cache_route.same_start_streak = 1;
            self.cache_route.last_start = start_position;
            if self.cache_route.route == CacheRoute::RegSet {
                return true;
            }
        }
        self.cache_route.last_start = start_position;

        match self.cache_route.route {
            CacheRoute::PerRegex => false,
            CacheRoute::RegSet => {
                if self.cache_route.same_start_streak < ROUTE_MIN_SAME_START_FOR_PROBE {
                    return true;
                }
                self.cache_route.calls_since_probe =
                    self.cache_route.calls_since_probe.saturating_add(1);
                if self.cache_route.calls_since_probe >= ROUTE_PROBE_EVERY {
                    self.cache_route.calls_since_probe = 0;
                    false
                } else {
                    true
                }
            }
        }
    }

    #[inline]
    fn observe_per_regex_outcome(&mut self, run_stats: PerRegexCallStats) {
        if SCANNER_STATS_ENABLED {
            self.stats.cache_checks += run_stats.cache_checks as u64;
            self.stats.cache_hits += run_stats.cache_hits as u64;
            self.stats.cache_misses += run_stats.cache_misses() as u64;
            self.stats.vm_search_calls += run_stats.vm_calls as u64;
        }

        // Route quality based on effective cache reuse vs VM work.
        // This catches line-by-line scans where cache eligibility is low.
        let denom = run_stats.cache_hits as u64 + run_stats.vm_calls as u64;
        let reuse_permille = (run_stats.cache_hits as u64 * 1000)
            .checked_div(denom)
            .unwrap_or(0);
        let poor = run_stats.vm_calls >= 4 && reuse_permille < 350;
        let good = run_stats.cache_hits >= 4 && run_stats.vm_calls <= 2 && reuse_permille >= 700;

        if poor {
            self.cache_route.poor_per_regex_streak =
                self.cache_route.poor_per_regex_streak.saturating_add(1);
            self.cache_route.good_per_regex_streak = 0;
        } else if good {
            self.cache_route.good_per_regex_streak =
                self.cache_route.good_per_regex_streak.saturating_add(1);
            self.cache_route.poor_per_regex_streak = 0;
        }

        if self.cache_route.route == CacheRoute::PerRegex
            && self.cache_route.poor_per_regex_streak >= ROUTE_POOR_STREAK_TO_REGSET
        {
            self.cache_route.route = CacheRoute::RegSet;
            self.cache_route.calls_since_probe = 0;
            self.cache_route.poor_per_regex_streak = 0;
            self.cache_route.good_per_regex_streak = 0;
            if SCANNER_STATS_ENABLED {
                self.stats.route_switch_to_regset += 1;
            }
        } else if self.cache_route.route == CacheRoute::RegSet
            && self.cache_route.good_per_regex_streak >= ROUTE_GOOD_STREAK_TO_PER_REGEX
        {
            self.cache_route.route = CacheRoute::PerRegex;
            self.cache_route.calls_since_probe = 0;
            self.cache_route.poor_per_regex_streak = 0;
            self.cache_route.good_per_regex_streak = 0;
            if SCANNER_STATS_ENABLED {
                self.stats.route_switch_to_per_regex += 1;
            }
        }
    }

    /// RegSet path for one-off searches (`use_cache = false`).
    fn search_regset(
        &mut self,
        str_data: &[u8],
        end: usize,
        start: usize,
        option: OnigOptionType,
        fallback_memo_id: Option<FallbackMemoIdentity>,
    ) -> Option<ScannerMatch> {
        let (idx, pos) = onig_regset_search_utf8(
            &mut self.regset,
            str_data,
            end,
            start,
            end,
            OnigRegSetLead::PositionLead,
            option,
            fallback_memo_id,
        );

        if idx < 0 {
            return None;
        }

        let regex_idx = idx as usize;
        let match_start = if pos >= 0 { pos as usize } else { start };
        if let Some(reg) = onig_regset_get_regex(&self.regset, regex_idx) {
            // `pos` is the position the winning attempt began at, as in C's
            // `onig_regset_search`. Rebuilding the match from it is only valid
            // while the match starts there; `\K` moves the start, and the
            // region below is then the only record of it.
            if reg.num_mem == 0 && !reg.keep_moves_match_start {
                let len = onig_regset_last_match_len(&self.regset);
                if len < 0 {
                    return None;
                }
                let match_end = match_start.saturating_add(len as usize).min(end);
                let mut capture_indices = SmallVec::with_capacity(1);
                capture_indices.push(CaptureIndex {
                    start: match_start,
                    end: match_end,
                    length: match_end.saturating_sub(match_start),
                });
                return Some(ScannerMatch {
                    index: regex_idx,
                    capture_indices,
                });
            }
        }

        let region = crate::regset::onig_regset_get_region(&self.regset, regex_idx)?;
        Some(build_scanner_match(regex_idx, region))
    }

    /// Per-regex search with optional cache reuse.
    ///
    /// Each regex is searched on its own exactly as the position-lead RegSet
    /// search attempts it (`onig_regset_entry_search`), so combining the
    /// first events -- earliest position, lower index on a tie, an error
    /// counting like a match -- gives the RegSet route's result, limit errors
    /// included. A regex's match or no-match from an earlier start stays
    /// valid for later starts up to that match: the later search attempts a
    /// subset of the same positions (the retry-in-search budget, which would
    /// break that, keeps these calls on the RegSet route). Errors are not
    /// cached, and regions move between the RegSet and the cache entries
    /// without copies.
    #[allow(clippy::too_many_arguments)]
    fn search_per_regex(
        &mut self,
        str_data: &[u8],
        end: usize,
        start: usize,
        str_id: u64,
        options_raw: u32,
        onig_opts: OnigOptionType,
        use_cache: bool,
        limit_revision: u64,
    ) -> (Option<ScannerMatch>, PerRegexCallStats) {
        // (position, index, is_match) of the earliest event so far.
        let mut best: Option<(usize, usize, bool)> = None;
        let mut run_stats = PerRegexCallStats::default();
        // An any-char-star regex attempts the first position of a search even
        // where it does not follow a newline, which a search from an earlier
        // start skipped.
        let mid_line = start > 0 && str_data[start - 1] != b'\n';

        let regset = &mut self.regset;
        let caches = &mut self.caches;
        let n = onig_regset_number_of_regex(regset) as usize;

        for (i, cache) in caches.iter_mut().enumerate().take(n) {
            // Regexes run in index order: a later one cannot beat an event at
            // the start.
            let stop = match best {
                None => end,
                Some((position, _, _)) if position <= start => break,
                Some((position, _, _)) => {
                    let mut before = position - 1;
                    while before > start && (str_data[before] & 0xC0) == 0x80 {
                        before -= 1;
                    }
                    before
                }
            };

            if use_cache
                && !cache.has_g_anchor
                && cache.last_str_id == str_id
                && cache.last_options == options_raw
                && cache.last_limit_revision == limit_revision
                && cache.last_position <= start
                && !(cache.after_newline_only && mid_line && cache.last_position != start)
            {
                run_stats.cache_checks += 1;
                if !cache.last_matched {
                    run_stats.cache_hits += 1;
                    continue;
                }
                if cache.last_result >= 0 && (cache.last_result as usize) >= start {
                    run_stats.cache_hits += 1;
                    let position = cache.last_result as usize;
                    if best.is_none_or(|(best_position, _, _)| position < best_position) {
                        best = Some((position, i, true));
                    }
                    continue;
                }
            }

            run_stats.vm_calls += 1;
            let event =
                onig_regset_entry_search(regset, i, str_data, end, start, stop, onig_opts, true);
            match event {
                RegSetEntryEvent::Match { position } => {
                    onig_regset_swap_region(regset, i, &mut cache.last_region);
                    cache.remember(str_id, start, options_raw, limit_revision, Some(position));
                    if best.is_none_or(|(best_position, _, _)| position < best_position) {
                        best = Some((position, i, true));
                    }
                }
                RegSetEntryEvent::Error { position, .. } => {
                    cache.forget();
                    if best.is_none_or(|(best_position, _, _)| position < best_position) {
                        best = Some((position, i, false));
                    }
                }
                // A search cut short at an earlier regex's event says
                // nothing about the rest of the subject.
                RegSetEntryEvent::None if stop == end => {
                    cache.remember(str_id, start, options_raw, limit_revision, None);
                }
                RegSetEntryEvent::None => cache.forget(),
            }
        }

        let out = match best {
            Some((_, index, true)) => self.caches[index]
                .last_region
                .as_ref()
                .map(|region| build_scanner_match(index, region)),
            _ => None,
        };
        (out, run_stats)
    }
}

/// Build a `ScannerMatch` from a regex index and region.
fn build_scanner_match(index: usize, region: &OnigRegion) -> ScannerMatch {
    let num_regs = region.num_regs as usize;
    let mut capture_indices = SmallVec::with_capacity(num_regs);

    for i in 0..num_regs {
        let beg = region.beg[i];
        let end = region.end[i];
        // A capture whose start lies after its end (a group that started
        // again and failed before closing) is not a range of the string;
        // report it like an unmatched group.
        if beg >= 0 && end >= beg {
            let start = beg as usize;
            let end = end as usize;
            capture_indices.push(CaptureIndex {
                start,
                end,
                length: end - start,
            });
        } else {
            // Unmatched optional capture group
            capture_indices.push(CaptureIndex {
                start: 0,
                end: 0,
                length: 0,
            });
        }
    }

    ScannerMatch {
        index,
        capture_indices,
    }
}

/// Convert a `ScannerMatch` with UTF-8 byte offsets to UTF-16 code unit offsets.
fn convert_match_to_utf16(string: &OnigString, m: ScannerMatch) -> ScannerMatch {
    ScannerMatch {
        index: m.index,
        capture_indices: m
            .capture_indices
            .into_iter()
            .map(|ci| {
                let start = string.utf8_offset_to_utf16(ci.start);
                let end = string.utf8_offset_to_utf16(ci.end);
                CaptureIndex {
                    start,
                    end,
                    length: end - start,
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    /// Group 1 of `((?=(a|ab))a?){2}` on "a" ends before it starts (1..0, as
    /// in C Oniguruma). The scanner reports it like an unmatched group
    /// instead of underflowing `end - start`.
    #[test]
    fn inverted_capture_reads_as_unmatched() {
        let mut scanner = Scanner::new(&[r"((?=(a|ab))a?){2}"]).unwrap();
        let m = scanner
            .find_next_match("a", 0, ScannerFindOptions::NONE)
            .unwrap();
        let spans: Vec<_> = m
            .capture_indices
            .iter()
            .map(|c| (c.start, c.end, c.length))
            .collect();
        assert_eq!(spans, vec![(0, 0, 0), (0, 0, 0), (1, 1, 0)]);
    }

    #[test]
    fn cache_miss_with_truncated_range_is_not_reused() {
        let mut scanner = Scanner::new(&[";", "}"]).unwrap();
        let s = "a;b}";

        assert_eq!(
            scanner.find_next_match_with_id(s, 1, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 1,
                    end: 2,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_with_id(s, 1, 2, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 3,
                    end: 4,
                    length: 1
                }],
            })
        );
    }

    #[test]
    fn stats_and_reset_work() {
        let mut scanner = Scanner::new(&["a"]).unwrap();
        assert_eq!(scanner.stats(), ScannerStats::default());

        let _ = scanner.find_next_match("ba", 0, ScannerFindOptions::NONE);
        let _ = scanner.find_next_match_with_id("ba", 1, 0, ScannerFindOptions::NONE);

        let stats = scanner.stats();
        assert!(stats.route_regset_calls >= 2);
        assert_eq!(stats.route_per_regex_calls, 0);

        scanner.reset_stats();
        assert_eq!(scanner.stats(), ScannerStats::default());
    }

    #[test]
    fn cache_mode_regset_probe_is_counted() {
        // A search retry budget disables this route; exclude global-limit tests.
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let mut scanner = Scanner::new(&["a"]).unwrap();
        for _ in 0..20 {
            let _ = scanner.find_next_match_with_id("ba", 1, 0, ScannerFindOptions::NONE);
        }
        let stats = scanner.stats();
        assert!(stats.route_cache_regset_calls > 0);
        assert!(stats.route_cache_probe_calls > 0);
        assert!(stats.route_per_regex_calls > 0);
    }

    #[test]
    fn optional_prefix_match_agrees_after_cache_route_switches() {
        // A search retry budget disables this route; exclude global-limit tests.
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let mut scanner = Scanner::new(&["a?bc", "q"]).unwrap();

        for _ in 0..25 {
            let matched = scanner
                .find_next_match_with_id("qabc", 55, 1, ScannerFindOptions::NONE)
                .expect("match");
            assert_eq!(matched.index, 0);
            assert_eq!(matched.capture_indices[0].start, 1);
            assert_eq!(matched.capture_indices[0].end, 4);
        }

        let stats = scanner.stats();
        assert!(stats.route_cache_regset_calls > 0, "{stats:?}");
        assert!(stats.route_cache_per_regex_calls > 0, "{stats:?}");
    }

    // =========================================================================
    // Tests ported from vscode-oniguruma (src/test/index.test.ts)
    // Positions adapted from UTF-16 code units to UTF-8 byte offsets.
    // =========================================================================

    /// Port of vscode-oniguruma `simple1`.
    #[test]
    fn vscode_simple1() {
        let mut scanner = Scanner::new(&["ell", "wo"]).unwrap();
        let s = "Hello world!";
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 1,
                    end: 4,
                    length: 3
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match(s, 2, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 6,
                    end: 8,
                    length: 2
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `simple2`.
    #[test]
    fn vscode_simple2() {
        let mut scanner = Scanner::new(&["a", "b", "c"]).unwrap();
        assert_eq!(
            scanner.find_next_match("x", 0, ScannerFindOptions::NONE),
            None
        );
        assert_eq!(
            scanner.find_next_match("xxaxxbxxc", 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 2,
                    end: 3,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match("xxaxxbxxc", 4, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 5,
                    end: 6,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match("xxaxxbxxc", 7, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 2,
                capture_indices: smallvec![CaptureIndex {
                    start: 8,
                    end: 9,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match("xxaxxbxxc", 9, ScannerFindOptions::NONE),
            None
        );
    }

    /// Port of vscode-oniguruma `unicode1`.
    /// Original uses UTF-16 positions; adapted to UTF-8 byte offsets.
    /// 'ab…cde21': a(1) b(1) …(3) c(1) d(1) e(1) 2(1) 1(1)
    /// UTF-8 byte offsets: a=0, b=1, …=2..4, c=5, d=6, e=7, 2=8, 1=9
    #[test]
    fn vscode_unicode1() {
        let mut scanner1 = Scanner::new(&["1", "2"]).unwrap();
        // Start at byte 7 (='e'), find '2' at byte 8
        assert_eq!(
            scanner1.find_next_match("ab\u{2026}cde21", 7, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 8,
                    end: 9,
                    length: 1
                }],
            })
        );

        let mut scanner2 = Scanner::new(&["\""]).unwrap();
        // '{"…": 1}': {=0 "=1 …=2..4 "=5 :=6 ' '=7 1=8 }=9
        // Start at byte 1, find '"' at byte 1
        assert_eq!(
            scanner2.find_next_match("{\"\\u{2026}\": 1}", 1, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 1,
                    end: 2,
                    length: 1
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `unicode2`.
    /// 'a💻bYX': a(1) 💻(4) b(1) Y(1) X(1) — total 8 bytes
    /// UTF-8 byte offsets: a=0, 💻=1..4, b=5, Y=6, X=7
    #[test]
    fn vscode_unicode2() {
        let mut scanner = Scanner::new(&["Y", "X"]).unwrap();
        let s = "a\u{1F4BB}bYX";
        assert_eq!(s.len(), 8);

        // From byte 0: Y at byte 6
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 6,
                    end: 7,
                    length: 1
                }],
            })
        );
        // From byte 5 (='b'): Y at byte 6
        assert_eq!(
            scanner.find_next_match(s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 6,
                    end: 7,
                    length: 1
                }],
            })
        );
        // From byte 6 (='Y'): Y at byte 6
        assert_eq!(
            scanner.find_next_match(s, 6, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 6,
                    end: 7,
                    length: 1
                }],
            })
        );
        // From byte 7 (='X'): X at byte 7
        assert_eq!(
            scanner.find_next_match(s, 7, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 7,
                    end: 8,
                    length: 1
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `unicode3`.
    /// 'Возврат' = 7 Cyrillic chars × 2 bytes each = 14 bytes
    #[test]
    fn vscode_unicode3() {
        let mut scanner =
            Scanner::new(&["\u{0412}\u{043E}\u{0437}\u{0432}\u{0440}\u{0430}\u{0442}"]).unwrap();
        let s = "\u{0412}\u{043E}\u{0437}\u{0432}\u{0440}\u{0430}\u{0442} long_var_name;";
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 0,
                    end: 14,
                    length: 14
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `out of bounds`.
    /// Note: Rust uses usize, so negative start is not possible.
    /// We test that start > len returns None.
    #[test]
    fn vscode_out_of_bounds() {
        let mut scanner = Scanner::new(&["X"]).unwrap();
        let s = "X\u{1F4BB}X"; // X(1) 💻(4) X(1) = 6 bytes
        // Start at 0: X at byte 0
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 0,
                    end: 1,
                    length: 1
                }],
            })
        );
        // Start beyond end: no match
        assert_eq!(
            scanner.find_next_match(s, 1000, ScannerFindOptions::NONE),
            None
        );
    }

    /// Port of vscode-oniguruma `regex with \G`.
    #[test]
    fn vscode_g_anchor() {
        let mut scanner = Scanner::new(&["\\G-and"]).unwrap();
        let s = "first-and-second";
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            None
        );
        assert_eq!(
            scanner.find_next_match(s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 5,
                    end: 9,
                    length: 4
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `kkos/oniguruma#192`.
    /// Complex regex that should NOT match the given input.
    #[test]
    fn vscode_oniguruma_issue_192() {
        let mut scanner = Scanner::new(&[
            "(?x)\n  (?<!\\+\\+|--)(?<=[({\\[,?=>:*]|&&|\\|\\||\\?|\\*\\/|^await|[^\\._$[:alnum:]]await|^return|[^\\._$[:alnum:]]return|^default|[^\\._$[:alnum:]]default|^yield|[^\\._$[:alnum:]]yield|^)\\s*\n  (?!<\\s*[_$[:alpha:]][_$[:alnum:]]*((\\s+extends\\s+[^=>])|,)) # look ahead is not type parameter of arrow\n  (?=(<)\\s*(?:([_$[:alpha:]][-_$[:alnum:].]*)(?<!\\.|-)(:))?((?:[a-z][a-z0-9]*|([_$[:alpha:]][-_$[:alnum:].]*))(?<!\\.|-))(?=((<\\s*)|(\\s+))(?!\\?)|\\/?>))",
        ]).unwrap();
        let s = "    while (i < len && f(array[i]))";
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            None
        );
    }

    /// Port of vscode-oniguruma `FindOption.NotBeginString`.
    #[test]
    fn vscode_find_option_not_begin_string() {
        let mut scanner = Scanner::new(&["\\Afirst"]).unwrap();
        let s = "first-and-first";
        assert_eq!(
            scanner.find_next_match(s, 10, ScannerFindOptions::NONE),
            None
        );
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 0,
                    end: 5,
                    length: 5
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NOT_BEGIN_STRING),
            None
        );
    }

    /// Port of vscode-oniguruma `FindOption.NotEndString`.
    #[test]
    fn vscode_find_option_not_end_string() {
        let mut scanner = Scanner::new(&["first\\z"]).unwrap();
        let s = "first-and-first";
        assert_eq!(
            scanner.find_next_match(s, 10, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 10,
                    end: 15,
                    length: 5
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match(s, 10, ScannerFindOptions::NOT_END_STRING),
            None
        );
    }

    /// Port of vscode-oniguruma `FindOption.NotBeginPosition`.
    #[test]
    fn vscode_find_option_not_begin_position() {
        let mut scanner = Scanner::new(&["\\G-and"]).unwrap();
        let s = "first-and-second";
        assert_eq!(
            scanner.find_next_match(s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 5,
                    end: 9,
                    length: 4
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match(s, 5, ScannerFindOptions::NOT_BEGIN_POSITION),
            None
        );
    }

    /// Port of vscode-oniguruma `Configure scanner`.
    #[test]
    fn vscode_configure_scanner() {
        let config = ScannerConfig {
            options: OnigOptionType::IGNORECASE,
            ..Default::default()
        };
        let mut scanner = Scanner::with_config(&["^[a-z]*$"], &config).unwrap();
        let s = "ABCD";
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 0,
                    end: 4,
                    length: 4
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `Configure syntax`.
    #[test]
    fn vscode_configure_syntax() {
        let config = ScannerConfig {
            syntax: ScannerSyntax::Python,
            ..Default::default()
        };
        let mut scanner = Scanner::with_config(&["^(?P<name>.*)$"], &config).unwrap();
        let s = "first-and-first";
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![
                    CaptureIndex {
                        start: 0,
                        end: 15,
                        length: 15
                    },
                    CaptureIndex {
                        start: 0,
                        end: 15,
                        length: 15
                    },
                ],
            })
        );
    }

    /// Port of vscode-oniguruma `Throw error`.
    /// `(?P<name>...)` is Python syntax, not valid in Oniguruma default syntax.
    #[test]
    fn vscode_invalid_pattern_error() {
        let result = Scanner::new(&["(?P<name>a*)"]);
        assert!(result.is_err());
    }

    // =========================================================================
    // Tests ported from vscode-oniguruma using UTF-16 API (OnigString).
    // These use the ORIGINAL positions from the TypeScript tests verbatim.
    // =========================================================================

    /// Port of vscode-oniguruma `simple1` — UTF-16 API.
    #[test]
    fn vscode_utf16_simple1() {
        let mut scanner = Scanner::new(&["ell", "wo"]).unwrap();
        let s = OnigString::new("Hello world!");
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 1,
                    end: 4,
                    length: 3
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 2, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 6,
                    end: 8,
                    length: 2
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `simple2` — UTF-16 API.
    #[test]
    fn vscode_utf16_simple2() {
        let mut scanner = Scanner::new(&["a", "b", "c"]).unwrap();
        let x = OnigString::new("x");
        assert_eq!(
            scanner.find_next_match_utf16(&x, 0, ScannerFindOptions::NONE),
            None
        );
        let abc = OnigString::new("xxaxxbxxc");
        assert_eq!(
            scanner.find_next_match_utf16(&abc, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 2,
                    end: 3,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&abc, 4, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 5,
                    end: 6,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&abc, 7, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 2,
                capture_indices: smallvec![CaptureIndex {
                    start: 8,
                    end: 9,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&abc, 9, ScannerFindOptions::NONE),
            None
        );
    }

    /// Port of vscode-oniguruma `unicode1` — UTF-16 API.
    /// Original positions used verbatim (UTF-16 code units).
    #[test]
    fn vscode_utf16_unicode1() {
        let mut scanner1 = Scanner::new(&["1", "2"]).unwrap();
        let s1 = OnigString::new("ab\u{2026}cde21"); // … is 1 UTF-16 code unit
        assert_eq!(
            scanner1.find_next_match_utf16(&s1, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 6,
                    end: 7,
                    length: 1
                }],
            })
        );

        let mut scanner2 = Scanner::new(&["\""]).unwrap();
        let s2 = OnigString::new("{\"\\u{2026}\": 1}");
        assert_eq!(
            scanner2.find_next_match_utf16(&s2, 1, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 1,
                    end: 2,
                    length: 1
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `unicode2` — UTF-16 API.
    /// 'a💻bYX' in UTF-16: a(0) 💻(1,2) b(3) Y(4) X(5) = 6 code units.
    /// These are the ORIGINAL test positions from vscode-oniguruma.
    #[test]
    fn vscode_utf16_unicode2() {
        let mut scanner = Scanner::new(&["Y", "X"]).unwrap();
        let s = OnigString::new("a\u{1F4BB}bYX");
        assert_eq!(s.utf16_len(), 6);

        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 4,
                    end: 5,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 1, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 4,
                    end: 5,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 3, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 4,
                    end: 5,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 4, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 4,
                    end: 5,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex {
                    start: 5,
                    end: 6,
                    length: 1
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `unicode3` — UTF-16 API.
    /// 'Возврат' = 7 Cyrillic chars, each 1 UTF-16 code unit.
    #[test]
    fn vscode_utf16_unicode3() {
        let mut scanner = Scanner::new(&["Возврат"]).unwrap();
        let s = OnigString::new("Возврат long_var_name;");
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 0,
                    end: 7,
                    length: 7
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `out of bounds` — UTF-16 API.
    #[test]
    fn vscode_utf16_out_of_bounds() {
        let mut scanner = Scanner::new(&["X"]).unwrap();
        let s = OnigString::new("X\u{1F4BB}X"); // X(0) 💻(1,2) X(3) = 4 UTF-16 code units
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 0,
                    end: 1,
                    length: 1
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 1000, ScannerFindOptions::NONE),
            None
        );
    }

    /// Port of vscode-oniguruma `regex with \G` — UTF-16 API.
    #[test]
    fn vscode_utf16_g_anchor() {
        let mut scanner = Scanner::new(&["\\G-and"]).unwrap();
        let s = OnigString::new("first-and-second");
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            None
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 5,
                    end: 9,
                    length: 4
                }],
            })
        );
    }

    /// Port of vscode-oniguruma `kkos/oniguruma#192` — UTF-16 API.
    #[test]
    fn vscode_utf16_oniguruma_issue_192() {
        let mut scanner = Scanner::new(&[
            "(?x)\n  (?<!\\+\\+|--)(?<=[({\\[,?=>:*]|&&|\\|\\||\\?|\\*\\/|^await|[^\\._$[:alnum:]]await|^return|[^\\._$[:alnum:]]return|^default|[^\\._$[:alnum:]]default|^yield|[^\\._$[:alnum:]]yield|^)\\s*\n  (?!<\\s*[_$[:alpha:]][_$[:alnum:]]*((\\s+extends\\s+[^=>])|,)) # look ahead is not type parameter of arrow\n  (?=(<)\\s*(?:([_$[:alpha:]][-_$[:alnum:].]*)(?<!\\.|-)(:))?((?:[a-z][a-z0-9]*|([_$[:alpha:]][-_$[:alnum:].]*))(?<!\\.|-))(?=((<\\s*)|(\\s+))(?!\\?)|\\/?>))",
        ]).unwrap();
        let s = OnigString::new("    while (i < len && f(array[i]))");
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            None
        );
    }

    /// Port of vscode-oniguruma `FindOption.NotBeginString` — UTF-16 API.
    #[test]
    fn vscode_utf16_find_option_not_begin_string() {
        let mut scanner = Scanner::new(&["\\Afirst"]).unwrap();
        let s = OnigString::new("first-and-first");
        assert_eq!(
            scanner.find_next_match_utf16(&s, 10, ScannerFindOptions::NONE),
            None
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 0,
                    end: 5,
                    length: 5
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NOT_BEGIN_STRING),
            None
        );
    }

    /// Port of vscode-oniguruma `FindOption.NotEndString` — UTF-16 API.
    #[test]
    fn vscode_utf16_find_option_not_end_string() {
        let mut scanner = Scanner::new(&["first\\z"]).unwrap();
        let s = OnigString::new("first-and-first");
        assert_eq!(
            scanner.find_next_match_utf16(&s, 10, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 10,
                    end: 15,
                    length: 5
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 10, ScannerFindOptions::NOT_END_STRING),
            None
        );
    }

    /// Port of vscode-oniguruma `FindOption.NotBeginPosition` — UTF-16 API.
    #[test]
    fn vscode_utf16_find_option_not_begin_position() {
        let mut scanner = Scanner::new(&["\\G-and"]).unwrap();
        let s = OnigString::new("first-and-second");
        assert_eq!(
            scanner.find_next_match_utf16(&s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex {
                    start: 5,
                    end: 9,
                    length: 4
                }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 5, ScannerFindOptions::NOT_BEGIN_POSITION),
            None
        );
    }

    // =========================================================================
    // OnigString unit tests
    // =========================================================================

    #[test]
    fn onig_string_ascii() {
        let s = OnigString::new("hello");
        assert_eq!(s.utf16_len(), 5);
        assert_eq!(s.utf16_offset_to_utf8(0), 0);
        assert_eq!(s.utf16_offset_to_utf8(3), 3);
        assert_eq!(s.utf16_offset_to_utf8(5), 5);
        assert_eq!(s.utf8_offset_to_utf16(0), 0);
        assert_eq!(s.utf8_offset_to_utf16(5), 5);
    }

    #[test]
    fn onig_string_instances_with_equal_contents_have_distinct_cache_ids() {
        let first = OnigString::new("unchanged");
        let second = OnigString::new("unchanged");

        assert_ne!(first.cache_id, second.cache_id);

        let mut scanner = Scanner::new(&["a*bc"]).expect("scanner");
        assert_eq!(
            scanner.find_next_match_utf16(&first, 0, ScannerFindOptions::NONE),
            None
        );
        assert_eq!(
            scanner.find_next_match_utf16(&second, 0, ScannerFindOptions::NONE),
            None
        );
    }

    #[test]
    fn onig_string_bmp() {
        // 'Возврат' = 7 Cyrillic chars, 2 bytes each in UTF-8, 1 code unit each in UTF-16
        let s = OnigString::new("Возврат");
        assert_eq!(s.utf16_len(), 7);
        assert_eq!(s.content().len(), 14);
        assert_eq!(s.utf16_offset_to_utf8(0), 0);
        assert_eq!(s.utf16_offset_to_utf8(1), 2);
        assert_eq!(s.utf16_offset_to_utf8(7), 14);
        assert_eq!(s.utf8_offset_to_utf16(0), 0);
        assert_eq!(s.utf8_offset_to_utf16(2), 1);
        assert_eq!(s.utf8_offset_to_utf16(14), 7);
    }

    #[test]
    fn onig_string_supplementary() {
        // 'a💻b': a=1 byte/1 unit, 💻=4 bytes/2 units, b=1 byte/1 unit
        let s = OnigString::new("a\u{1F4BB}b");
        assert_eq!(s.utf16_len(), 4); // a(1) + 💻(2) + b(1)
        assert_eq!(s.content().len(), 6); // a(1) + 💻(4) + b(1)

        // UTF-16 → UTF-8
        assert_eq!(s.utf16_offset_to_utf8(0), 0); // a
        assert_eq!(s.utf16_offset_to_utf8(1), 1); // 💻 high surrogate
        assert_eq!(s.utf16_offset_to_utf8(2), 5); // 💻 low surrogate → after 💻
        assert_eq!(s.utf16_offset_to_utf8(3), 5); // b
        assert_eq!(s.utf16_offset_to_utf8(4), 6); // end

        // UTF-8 → UTF-16
        assert_eq!(s.utf8_offset_to_utf16(0), 0); // a
        assert_eq!(s.utf8_offset_to_utf16(1), 1); // 💻 byte 1
        assert_eq!(s.utf8_offset_to_utf16(2), 1); // 💻 byte 2 (continuation)
        assert_eq!(s.utf8_offset_to_utf16(3), 1); // 💻 byte 3 (continuation)
        assert_eq!(s.utf8_offset_to_utf16(4), 1); // 💻 byte 4 (continuation)
        assert_eq!(s.utf8_offset_to_utf16(5), 3); // b
        assert_eq!(s.utf8_offset_to_utf16(6), 4); // end
    }

    // =========================================================================
    // Additional tests (not from vscode-oniguruma)
    // =========================================================================

    #[test]
    fn multi_pattern_correct_index() {
        let mut scanner = Scanner::new(&["\\d+", "[a-z]+"]).unwrap();
        let m = scanner
            .find_next_match("hello42", 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 1); // "[a-z]+" matches at 0, before "\\d+" at 5
        assert_eq!(m.capture_indices[0].start, 0);
        assert_eq!(m.capture_indices[0].end, 5);
    }

    #[test]
    fn capture_groups() {
        let mut scanner = Scanner::new(&["(\\d{4})-(\\d{2})-(\\d{2})"]).unwrap();
        let m = scanner
            .find_next_match("date: 2026-02-16", 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 0);
        assert_eq!(m.capture_indices.len(), 4); // full + 3 groups
        assert_eq!(m.capture_indices[0].start, 6);
        assert_eq!(m.capture_indices[0].end, 16);
        assert_eq!(m.capture_indices[1].start, 6);
        assert_eq!(m.capture_indices[1].end, 10);
        assert_eq!(m.capture_indices[2].start, 11);
        assert_eq!(m.capture_indices[2].end, 13);
        assert_eq!(m.capture_indices[3].start, 14);
        assert_eq!(m.capture_indices[3].end, 16);
    }

    #[test]
    fn long_string_path() {
        // String > 1000 bytes triggers per-regex search path
        let long = "a".repeat(1500);
        let mut scanner = Scanner::new(&["aaa"]).unwrap();
        let m = scanner
            .find_next_match(&long, 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 0);
        assert_eq!(m.capture_indices[0].start, 0);
        assert_eq!(m.capture_indices[0].end, 3);
    }

    #[test]
    fn caching_with_str_id() {
        let long = "x".repeat(500) + "hello" + &"y".repeat(1000);
        let mut scanner = Scanner::new(&["hello", "world"]).unwrap();

        let m = scanner
            .find_next_match_with_id(&long, 1, 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 0);
        assert_eq!(m.capture_indices[0].start, 500);

        // Advancing past the match; "world" was cached as no-match
        let m = scanner.find_next_match_with_id(&long, 1, 501, ScannerFindOptions::NONE);
        assert!(m.is_none());
    }

    #[test]
    fn g_anchor_bypasses_cache() {
        let long = "a".repeat(1500);
        let mut scanner = Scanner::new(&["\\Ga"]).unwrap();

        let m = scanner
            .find_next_match_with_id(&long, 1, 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.capture_indices[0].start, 0);

        // \G patterns must not use cache (anchor is position-dependent)
        let m = scanner
            .find_next_match_with_id(&long, 1, 1, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.capture_indices[0].start, 1);
    }

    #[test]
    fn g_anchor_uses_original_search_start_in_position_lead() {
        let mut scanner = Scanner::new(&["\\G(\\s+)", "\\s+"]).unwrap();
        let s = "x> y";

        // Search starts at index 1 ('>').
        // \G(\\s+) must not be allowed to re-anchor at index 2 (' ').
        // The plain \\s+ pattern should win.
        let m = scanner
            .find_next_match(s, 1, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 1);
        assert_eq!(m.capture_indices[0].start, 2);
        assert_eq!(m.capture_indices[0].end, 3);

        let m = scanner
            .find_next_match_with_id(s, 42, 1, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 1);
        assert_eq!(m.capture_indices[0].start, 2);
        assert_eq!(m.capture_indices[0].end, 3);

        // Warm cache route and re-run at a shifted start to exercise
        // per-regex probing with the same string id.
        let _ = scanner.find_next_match_with_id(s, 42, 0, ScannerFindOptions::NONE);
        let m = scanner
            .find_next_match_with_id(s, 42, 1, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 1);
        assert_eq!(m.capture_indices[0].start, 2);
        assert_eq!(m.capture_indices[0].end, 3);
    }

    #[test]
    fn find_options_conversion() {
        let opts = ScannerFindOptions::NOT_BEGIN_STRING;
        let onig = opts.to_onig_options();
        assert!(onig.contains(OnigOptionType::NOT_BEGIN_STRING));

        let opts = ScannerFindOptions::NOT_END_STRING;
        let onig = opts.to_onig_options();
        assert!(onig.contains(OnigOptionType::NOT_END_STRING));

        let opts = ScannerFindOptions::NOT_BEGIN_POSITION;
        let onig = opts.to_onig_options();
        assert!(onig.contains(OnigOptionType::NOT_BEGIN_POSITION));

        let opts = ScannerFindOptions::from_bits(3); // NOT_BEGIN_STRING | NOT_END_STRING
        let onig = opts.to_onig_options();
        assert!(onig.contains(OnigOptionType::NOT_BEGIN_STRING));
        assert!(onig.contains(OnigOptionType::NOT_END_STRING));
    }

    type Found = Option<(usize, usize, usize)>;
    type LimitCase = (&'static [&'static str], String, Vec<(usize, Found)>);

    /// Every route answers like one fresh position-lead RegSet search, limit
    /// errors included, whatever calls came before: in order, reversed, and
    /// with each start repeated 20 times (which moves `with_id` calls onto
    /// the per-regex cache route). The expected results are those of
    /// vscode-oniguruma's scanner over C Oniguruma with a retry limit of
    /// 10,000 (`None` where C stops at the limit, or finds nothing).
    #[test]
    fn limit_errors_match_c_on_every_route_and_call_history() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let old_limit = crate::regexec::onig_get_retry_limit_in_match();
        crate::regexec::onig_set_retry_limit_in_match(10_000);

        let cases: [LimitCase; 2] = [
            // C's optimizer finds no `x`, so C never attempts `(\w+)+x`.
            (
                &[r"(\w+)+x", "a"],
                "a".repeat(30),
                vec![
                    (0, Some((1, 0, 1))),
                    (1, Some((1, 1, 2))),
                    (5, Some((1, 5, 6))),
                    (29, Some((1, 29, 30))),
                    (30, None),
                ],
            ),
            // `(a+)+b` stops at the limit from 0 and 10, which ends C's search.
            (
                &[r"(a+)+b", "c"],
                format!("{}c b", "a".repeat(27)),
                vec![
                    (0, None),
                    (10, None),
                    (15, Some((1, 27, 28))),
                    (20, Some((1, 27, 28))),
                    (27, Some((1, 27, 28))),
                ],
            ),
        ];
        let mut failures = Vec::new();
        for (patterns, text, expected) in &cases {
            let onig = OnigString::new(text);
            let reversed: Vec<_> = expected.iter().rev().copied().collect();
            let repeated: Vec<_> = expected
                .iter()
                .flat_map(|&call| std::iter::repeat_n(call, 20))
                .collect();
            for order in [expected.clone(), reversed, repeated] {
                let mut scanners: [Scanner; 4] =
                    std::array::from_fn(|_| Scanner::new(patterns).unwrap());
                for &(start, want) in &order {
                    let none = ScannerFindOptions::NONE;
                    let found = [
                        scanners[0].find_next_match(text, start, none),
                        scanners[1].find_next_match_with_id(text, 7, start, none),
                        scanners[2].find_next_match_utf16(&onig, start, none),
                        scanners[3].find_next_match_utf16_with_id(&onig, 7, start, none),
                    ];
                    for (route, found) in found.iter().enumerate() {
                        let found = found.as_ref().map(|m| {
                            let whole = &m.capture_indices[0];
                            (m.index, whole.start, whole.end)
                        });
                        if found != want {
                            failures.push((patterns.to_vec(), route, start, found, want));
                        }
                    }
                }
            }
        }

        crate::regexec::onig_set_retry_limit_in_match(old_limit);
        assert!(failures.is_empty(), "{failures:?}");
    }

    /// C ranks a match by the position its attempt began at, not by the
    /// start `\K` moves it to: `.+\K,` attempted at 1 beats `[^\s]` at 1 on
    /// its lower index and reports 6..7. vscode-oniguruma's scanner over C
    /// Oniguruma reports pattern 0 at 6..7 from starts 0, 1 and 2; every
    /// route has to agree, whatever calls came before.
    #[test]
    fn keep_matches_rank_by_their_attempt_position_on_every_route() {
        let patterns = [r".+\K,", r"[^\s]"];
        let text = "\n!c1 1,é1";
        let onig = OnigString::new(text);
        let starts = [0, 1, 2, 2, 1, 0];
        let mut scanners: [Scanner; 4] = std::array::from_fn(|_| Scanner::new(&patterns).unwrap());
        for start in starts.into_iter().chain(std::iter::repeat_n(0, 20)) {
            let none = ScannerFindOptions::NONE;
            let found = [
                scanners[0].find_next_match(text, start, none),
                scanners[1].find_next_match_with_id(text, 3, start, none),
                scanners[2].find_next_match_utf16(&onig, start, none),
                scanners[3].find_next_match_utf16_with_id(&onig, 3, start, none),
            ];
            for (route, found) in found.iter().enumerate() {
                let found = found.as_ref().map(|m| {
                    let whole = &m.capture_indices[0];
                    (m.index, whole.start, whole.end)
                });
                assert_eq!(found, Some((0, 6, 7)), "route {route}, start {start}");
            }
        }
    }

    #[test]
    fn multi_pattern_earliest_wins() {
        let mut scanner = Scanner::new(&["world", "hello"]).unwrap();
        let m = scanner
            .find_next_match("hello world", 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 1); // "hello" matches earlier at position 0
        assert_eq!(m.capture_indices[0].start, 0);
    }

    #[test]
    fn empty_pattern_matches() {
        let mut scanner = Scanner::new(&["", "x"]).unwrap();
        let m = scanner
            .find_next_match("hello", 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.index, 0); // empty pattern matches at position 0
    }

    #[test]
    fn optional_capture_group() {
        let mut scanner = Scanner::new(&["(a)(b)?(c)"]).unwrap();
        let m = scanner
            .find_next_match("ac", 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.capture_indices.len(), 4);
        // Group 2 (b)? did not match
        assert_eq!(m.capture_indices[2].start, 0);
        assert_eq!(m.capture_indices[2].end, 0);
        assert_eq!(m.capture_indices[2].length, 0);
    }

    // =================================================================
    // Coverage-targeted: per-regex cache hit paths
    // =================================================================

    #[test]
    fn cache_hit_on_repeated_search_same_string() {
        // A search retry budget disables this route; exclude global-limit tests.
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        // Exercises per-regex cache reuse: need ≥8 same-start calls to trigger probe
        let mut scanner = Scanner::new(&["foo", "bar", "baz"]).unwrap();
        let input = "xxfooxxbarxxbaz";

        // Do 9 calls from same start to trigger per-regex probe (ROUTE_MIN_SAME_START_FOR_PROBE=8)
        for _ in 0..9 {
            let _ = scanner.find_next_match_with_id(input, 42, 0, ScannerFindOptions::NONE);
        }
        // After probing, do more calls — some should use per-regex with cache
        for _ in 0..20 {
            let _ = scanner.find_next_match_with_id(input, 42, 0, ScannerFindOptions::NONE);
        }

        let stats = scanner.stats();
        assert!(
            stats.route_per_regex_calls > 0 || stats.cache_hits > 0,
            "expected per-regex or cache activity, got {:?}",
            stats
        );
    }

    #[test]
    fn cache_no_match_reused() {
        // A search retry budget disables this route; exclude global-limit tests.
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        // Exercises cache path where a pattern previously found no match
        // Need repeated same-start calls to trigger per-regex mode
        let mut scanner = Scanner::new(&["zzz", "a"]).unwrap();
        let input = "aaa";

        // 30 calls from same start → triggers per-regex mode and cache reuse
        for _ in 0..30 {
            let m = scanner.find_next_match_with_id(input, 1, 0, ScannerFindOptions::NONE);
            assert!(m.is_some());
            assert_eq!(m.unwrap().index, 1); // always "a"
        }

        let stats = scanner.stats();
        assert!(
            stats.route_per_regex_calls > 0 || stats.cache_hits > 0,
            "expected per-regex or cache activity, got {:?}",
            stats
        );
    }

    #[test]
    fn cache_invalidated_on_new_string() {
        // Exercises cache reset when str_id changes
        let mut scanner = Scanner::new(&["x"]).unwrap();

        let m1 = scanner
            .find_next_match_with_id("axb", 1, 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m1.capture_indices[0].start, 1);

        // Different str_id → cache reset
        let m2 = scanner
            .find_next_match_with_id("xab", 2, 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m2.capture_indices[0].start, 0);
    }

    // =================================================================
    // Coverage-targeted: UTF-16 with ID
    // =================================================================

    #[test]
    fn utf16_with_id_ascii() {
        let mut scanner = Scanner::new(&["x"]).unwrap();
        let s = OnigString::new("axb");
        let m = scanner
            .find_next_match_utf16_with_id(&s, 10, 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m.capture_indices[0].start, 1);
    }

    #[test]
    fn utf16_with_id_unicode() {
        let mut scanner = Scanner::new(&["x"]).unwrap();
        let s = OnigString::new("💻x");
        let m = scanner
            .find_next_match_utf16_with_id(&s, 20, 0, ScannerFindOptions::NONE)
            .unwrap();
        // 💻 = 2 UTF-16 code units, so x is at UTF-16 offset 2
        assert_eq!(m.capture_indices[0].start, 2);
    }

    // =================================================================
    // Coverage-targeted: ScannerSyntax variants
    // =================================================================

    #[test]
    fn scanner_syntax_variants() {
        let syntaxes = [
            ScannerSyntax::Asis,
            ScannerSyntax::PosixBasic,
            ScannerSyntax::Emacs,
            ScannerSyntax::Grep,
            ScannerSyntax::GnuRegex,
            ScannerSyntax::Java,
            ScannerSyntax::Perl,
            ScannerSyntax::PerlNg,
            ScannerSyntax::Ruby,
            ScannerSyntax::Python,
        ];
        for syntax in syntaxes {
            let config = ScannerConfig {
                options: ONIG_OPTION_NONE,
                syntax,
            };
            // Simple literal pattern should work in all syntaxes
            let scanner = Scanner::with_config(&["hello"], &config);
            assert!(scanner.is_ok(), "failed for {:?}", syntax);
        }
    }

    // =================================================================
    // Coverage-targeted: \G anchor in per-regex mode
    // =================================================================

    #[test]
    fn g_anchor_pattern() {
        // Exercises search_g_anchor_with_msa path
        let mut scanner = Scanner::new(&[r"\Gx", "y"]).unwrap();
        let input = "xxy";

        // First match: \G matches at position 0
        let m1 = scanner
            .find_next_match_with_id(input, 1, 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m1.index, 0); // \Gx
        assert_eq!(m1.capture_indices[0].start, 0);

        // Search from position 2: \G should match at 2 if using per-regex path
        let m2 = scanner
            .find_next_match_with_id(input, 1, 2, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!(m2.index, 1); // "y" at position 2
    }

    #[test]
    fn zero_width_matches_at_end_are_reported() {
        for pattern in ["$", r"\z", "a*"] {
            let mut scanner = Scanner::new(&[pattern]).unwrap();
            let found = scanner
                .find_next_match("abc", 3, ScannerFindOptions::NONE)
                .unwrap_or_else(|| panic!("missing end match for {pattern:?}"));

            assert_eq!(found.index, 0);
            assert_eq!(found.capture_indices[0].start, 3);
            assert_eq!(found.capture_indices[0].end, 3);
            assert_eq!(found.capture_indices[0].length, 0);
        }
    }

    #[test]
    fn zero_width_match_on_empty_input_is_reported() {
        let mut scanner = Scanner::new(&["$"]).unwrap();
        let found = scanner
            .find_next_match("", 0, ScannerFindOptions::NONE)
            .expect("empty input should match the end anchor");

        assert_eq!(found.index, 0);
        assert_eq!(found.capture_indices[0].start, 0);
        assert_eq!(found.capture_indices[0].end, 0);
        assert_eq!(found.capture_indices[0].length, 0);
    }

    #[test]
    fn repeated_end_anchor_search_agrees_across_adaptive_routes() {
        let mut scanner = Scanner::new(&[r"\z", "q"]).unwrap();

        for call in 0..25 {
            let found = scanner
                .find_next_match_with_id("abcd", 7, 1, ScannerFindOptions::NONE)
                .unwrap_or_else(|| panic!("missing end match on call {call}"));
            assert_eq!(found.index, 0, "call {call}");
            assert_eq!(found.capture_indices[0].start, 4, "call {call}");
            assert_eq!(found.capture_indices[0].end, 4, "call {call}");
        }
    }

    // =================================================================
    // Coverage-targeted: route switching (RegSet ↔ PerRegex)
    // =================================================================

    #[test]
    fn many_searches_trigger_route_switching() {
        // Exercises observe_per_regex_outcome and route switching logic
        let mut scanner = Scanner::new(&["a+", "b+", "c+"]).unwrap();
        let input = "aabbcc";

        // Do many searches on the same string to trigger route switching
        let mut pos = 0;
        let mut matches = Vec::new();
        for _ in 0..20 {
            let Some(m) = scanner.find_next_match_with_id(input, 99, pos, ScannerFindOptions::NONE)
            else {
                break;
            };
            pos = m.capture_indices[0].end;
            matches.push(m.index);
        }
        assert_eq!(matches, vec![0, 1, 2]); // a+, b+, c+
        let stats = scanner.stats();
        assert!(stats.route_regset_calls > 0);
    }

    #[test]
    fn same_start_streak_triggers_per_regex() {
        // Exercises same_start_streak counting in should_use_regset_for_cache
        let mut scanner = Scanner::new(&["x", "y"]).unwrap();
        let input = "xy";

        // Search multiple times from same position to build streak
        for _ in 0..12 {
            let _ = scanner.find_next_match_with_id(input, 1, 0, ScannerFindOptions::NONE);
        }
        let stats = scanner.stats();
        assert!(stats.route_regset_calls > 0 || stats.route_per_regex_calls > 0);
    }

    // =================================================================
    // Pattern cache
    // =================================================================

    /// Pattern lists that repeat each other's patterns. Between them they
    /// reach table and fallback entries, captures and named groups, `\G`,
    /// `\A`, `\z`, look-behind, back references, `\K`, any-char stars,
    /// Unicode classes, required literals, a callout with per-search data,
    /// a backtracking warning and a backtracking rewrite.
    const CACHED_PATTERN_SETS: [&[&str]; 3] = [
        &[
            r"\G(\s+)",
            r"(?<word>\w+)",
            r#""(?:[^"\\]|\\.)*""#,
            r"\Afirst",
            r"(?<=\.)\w+",
            r"(a+)+b",
        ],
        &[
            r"(\w)\1",
            r".+\K,",
            r"(?<word>\w+)",
            r"\s*$",
            r"[a-z]*(?:foo|bar|baz)",
            r"(?:[ab]|(*MAX{2}).)*c",
        ],
        &[
            r"\b(?:fn|let|first)\b",
            r"\p{Greek}+",
            r"first\z",
            r".*;",
            r"(?<word>\w+)",
            r"(a+)+b",
            r"([0-9]+(_?))+(\.)([0-9]+)",
        ],
    ];

    const CACHED_PATTERN_SUBJECTS: [&str; 6] = [
        "first-and-first",
        "let x = \"a\\\"b\"; // done",
        "\n!c1 1,é1 zfoo",
        "αβγ fn foo.bar 12_34.56",
        "abcbaaccaaa abc",
        "aaaaaaaaaaaaaaaaaaaa\u{1F4BB}c b",
    ];

    /// The address of each pattern's compiled program in `scanner`.
    fn compiled_addresses(scanner: &Scanner) -> Vec<*const RegexType> {
        (0..onig_regset_number_of_regex(&scanner.regset) as usize)
            .map(|i| onig_regset_get_regex(&scanner.regset, i).unwrap() as *const RegexType)
            .collect()
    }

    /// One scanner per route of `every_route`, so each sees a uniform call
    /// history.
    fn route_scanners(mut build: impl FnMut() -> Scanner) -> [Scanner; 4] {
        std::array::from_fn(|_| build())
    }

    /// The same call on every route: plain, with a string id, UTF-16, and
    /// UTF-16 with a string id. `start` is a byte offset.
    fn every_route(
        scanners: &mut [Scanner; 4],
        onig: &OnigString,
        id: u64,
        start: usize,
        options: ScannerFindOptions,
    ) -> [Option<ScannerMatch>; 4] {
        let text = onig.content();
        let utf16_start = onig.utf8_offset_to_utf16(start);
        [
            scanners[0].find_next_match(text, start, options),
            scanners[1].find_next_match_with_id(text, id, start, options),
            scanners[2].find_next_match_utf16(onig, utf16_start, options),
            scanners[3].find_next_match_utf16_with_id(onig, id, utf16_start, options),
        ]
    }

    /// Run the same calls through cached and uncached scanners and collect
    /// every difference: each start once, then each start eleven times
    /// (which moves the calls with a string id onto the per-regex route).
    fn cached_and_uncached_differences(
        cached: &mut [[Scanner; 4]],
        uncached: &mut [[Scanner; 4]],
        subjects: &[&str],
        options: &[ScannerFindOptions],
    ) -> Vec<String> {
        let mut differences = Vec::new();
        for (set, (cached, uncached)) in cached.iter_mut().zip(uncached.iter_mut()).enumerate() {
            for &options in options {
                for (id, subject) in (1..).zip(subjects) {
                    // A fresh wrapper per pass keeps fallback memos apart,
                    // as for a new line.
                    let onig = OnigString::new(subject);
                    let starts: Vec<usize> = (0..=subject.len())
                        .filter(|&at| subject.is_char_boundary(at))
                        .collect();
                    let calls = starts
                        .iter()
                        .copied()
                        .chain(starts.iter().flat_map(|&at| std::iter::repeat_n(at, 11)));
                    for start in calls {
                        let want = every_route(uncached, &onig, id, start, options);
                        let got = every_route(cached, &onig, id, start, options);
                        if got != want {
                            differences.push(format!(
                                "set {set}, {options:?}, {subject:?} from {start}: \
                                 {got:?} != {want:?}"
                            ));
                        }
                    }
                }
            }
        }
        differences
    }

    /// Scanners built from one pattern cache answer every call exactly as
    /// scanners compiled on their own do, on every route, with every find
    /// option, with and without backtracking optimization, and under the
    /// retry, search and stack limits. Their warnings and rewrites agree too,
    /// and every occurrence of a pattern shares one compiled program.
    #[test]
    fn pattern_cache_scanners_match_uncached_scanners() {
        // The routes and limit outcomes read the process-wide limits.
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let config = ScannerConfig::default();
        let every_option: Vec<_> = (0..8).map(ScannerFindOptions::from_bits).collect();
        let distinct: std::collections::HashSet<&str> = CACHED_PATTERN_SETS
            .iter()
            .flat_map(|set| set.iter().copied())
            .collect();
        let mut cache = ScannerPatternCache::new();
        let mut failures = Vec::new();

        for optimize_backtracking in [false, true] {
            let uncached_scanner = |patterns: &[&str]| {
                if optimize_backtracking {
                    Scanner::with_backtracking_optimization(patterns, &config).unwrap()
                } else {
                    Scanner::with_config(patterns, &config).unwrap()
                }
            };
            let mut cached: Vec<[Scanner; 4]> = CACHED_PATTERN_SETS
                .iter()
                .map(|patterns| {
                    route_scanners(|| {
                        if optimize_backtracking {
                            Scanner::with_backtracking_optimization_and_pattern_cache(
                                patterns, &config, &mut cache,
                            )
                        } else {
                            Scanner::with_pattern_cache(patterns, &config, &mut cache)
                        }
                        .unwrap()
                    })
                })
                .collect();
            let mut uncached: Vec<[Scanner; 4]> = CACHED_PATTERN_SETS
                .iter()
                .map(|patterns| route_scanners(|| uncached_scanner(patterns)))
                .collect();
            // Each setting compiles each distinct pattern once.
            let settings = usize::from(optimize_backtracking) + 1;
            assert_eq!(cache.len(), settings * distinct.len());

            let mut addresses = HashMap::new();
            for (patterns, (cached, uncached)) in
                CACHED_PATTERN_SETS.iter().zip(cached.iter().zip(&uncached))
            {
                for scanner in cached {
                    assert_eq!(scanner.warnings(), uncached[0].warnings());
                    assert_eq!(
                        scanner.backtracking_rewrites(),
                        uncached[0].backtracking_rewrites()
                    );
                    for (pattern, address) in patterns.iter().zip(compiled_addresses(scanner)) {
                        assert_eq!(*addresses.entry(*pattern).or_insert(address), address);
                    }
                }
                for (pattern, address) in patterns.iter().zip(compiled_addresses(&uncached[0])) {
                    assert_ne!(addresses[pattern], address);
                }
            }
            let rewritten = uncached[2][0].backtracking_rewrites()[6].len();
            assert_eq!(rewritten, usize::from(optimize_backtracking));
            assert_eq!(uncached[0][0].warnings()[5].len(), 1);

            failures.extend(cached_and_uncached_differences(
                &mut cached,
                &mut uncached,
                &CACHED_PATTERN_SUBJECTS,
                &every_option,
            ));
            let per_regex_calls: u64 = (cached.iter())
                .flat_map(|routes| [&routes[1], &routes[3]])
                .map(|scanner| scanner.stats().route_per_regex_calls)
                .sum();
            assert!(per_regex_calls > 0);
        }

        // The limits are read by each search, so they apply to cached
        // patterns as to any other. `(a+)+b` from 0 stops at a retry limit
        // of 1,000, which ends the search with no match; a later start
        // finds the `c` (as in
        // `limit_errors_match_c_on_every_route_and_call_history`).
        let limit_case = format!("{}c b", "a".repeat(12));
        let limit_sets: Vec<&[&str]> = (CACHED_PATTERN_SETS.iter().copied())
            .chain([&[r"(a+)+b", "c"][..]])
            .collect();
        let limit_subjects: Vec<&str> = (CACHED_PATTERN_SUBJECTS.iter().copied())
            .chain([limit_case.as_str()])
            .collect();
        // Every setting keeps that retry limit in match, which bounds each
        // attempt; the others add a search retry budget or a stack limit.
        let limits = [
            ("retry limit in match", 0, 0),
            ("retry limit in search", 20_000, 0),
            ("match stack limit", 0, 64),
        ];
        let saved = (
            crate::regexec::onig_get_retry_limit_in_match(),
            crate::regexec::onig_get_retry_limit_in_search(),
            crate::regexec::onig_get_match_stack_limit(),
        );
        for (name, in_search, stack) in limits {
            crate::regexec::onig_set_retry_limit_in_match(1_000);
            crate::regexec::onig_set_retry_limit_in_search(in_search);
            crate::regexec::onig_set_match_stack_limit(stack);
            let mut cached: Vec<[Scanner; 4]> = (limit_sets.iter())
                .map(|patterns| {
                    route_scanners(|| {
                        Scanner::with_pattern_cache(patterns, &config, &mut cache).unwrap()
                    })
                })
                .collect();
            let mut uncached: Vec<[Scanner; 4]> = (limit_sets.iter())
                .map(|patterns| route_scanners(|| Scanner::with_config(patterns, &config).unwrap()))
                .collect();
            if in_search == 0 && stack == 0 {
                let limited = &mut cached[3][0];
                let none = ScannerFindOptions::NONE;
                assert_eq!(limited.find_next_match(&limit_case, 0, none), None);
                let after = limited.find_next_match(&limit_case, 5, none).unwrap();
                assert_eq!((after.index, after.capture_indices[0].start), (1, 12));
            }
            failures.extend(
                cached_and_uncached_differences(
                    &mut cached,
                    &mut uncached,
                    &limit_subjects,
                    &[ScannerFindOptions::NONE],
                )
                .into_iter()
                .map(|difference| format!("{name}: {difference}")),
            );
        }
        crate::regexec::onig_set_retry_limit_in_match(saved.0);
        crate::regexec::onig_set_retry_limit_in_search(saved.1);
        crate::regexec::onig_set_match_stack_limit(saved.2);
        // The limits did not grow the cache beyond the added `c`.
        assert_eq!(cache.len(), 2 * distinct.len() + 1);

        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// The same pattern text compiles to a separate entry for every other
    /// option set, syntax and backtracking optimization choice, whichever
    /// is built first.
    #[test]
    fn pattern_cache_keeps_settings_apart() {
        let plain = ScannerConfig::default();
        let ignore_case = ScannerConfig {
            options: plain.options | ONIG_OPTION_IGNORECASE,
            ..plain.clone()
        };
        let asis = ScannerConfig {
            syntax: ScannerSyntax::Asis,
            ..plain.clone()
        };
        let decimal = r"([0-9]+(_?))+(\.)([0-9]+)";
        let pattern = "a+b";
        let mut cache = ScannerPatternCache::new();
        // Optimized first: the plain scanner must not pick up its rewrite.
        let optimized = Scanner::with_backtracking_optimization_and_pattern_cache(
            &[pattern, decimal],
            &plain,
            &mut cache,
        )
        .unwrap();
        let mut scanners = [&plain, &ignore_case, &asis].map(|config| {
            Scanner::with_pattern_cache(&[pattern, decimal], config, &mut cache).unwrap()
        });
        assert_eq!(cache.len(), 8);
        assert_eq!(cache.compiled.len(), 4);

        let mut addresses: Vec<_> = scanners.iter().flat_map(compiled_addresses).collect();
        addresses.extend(compiled_addresses(&optimized));
        let all = addresses.len();
        addresses.sort();
        addresses.dedup();
        assert_eq!(addresses.len(), all);

        assert!(scanners[0].backtracking_rewrites()[1].is_empty());
        assert_eq!(optimized.backtracking_rewrites()[1].len(), 1);
        let text = "AAB a+b aab";
        let found = scanners.each_mut().map(|scanner| {
            let m = scanner.find_next_match(text, 0, ScannerFindOptions::NONE)?;
            Some((m.capture_indices[0].start, m.capture_indices[0].end))
        });
        assert_eq!(found, [Some((8, 11)), Some((0, 3)), Some((4, 7))]);

        // Each setting reuses its own entries.
        let again =
            Scanner::with_pattern_cache(&[decimal, pattern], &ignore_case, &mut cache).unwrap();
        assert_eq!(cache.len(), 8);
        let before = compiled_addresses(&scanners[1]);
        assert_eq!(compiled_addresses(&again), [before[1], before[0]]);
    }

    /// A pattern that fails to compile, or a set the RegSet refuses, returns
    /// the error an uncached scanner returns and leaves the cache as it was:
    /// patterns compiled earlier in the same call and a setting first seen
    /// in it are taken out again.
    #[test]
    fn failed_construction_leaves_the_pattern_cache_unchanged() {
        let config = ScannerConfig::default();
        let mut cache = ScannerPatternCache::new();
        let kept = Scanner::with_pattern_cache(&["a", "b"], &config, &mut cache).unwrap();
        let snapshot = |cache: &ScannerPatternCache| {
            let mut entries: Vec<_> = cache
                .compiled
                .iter()
                .flat_map(|(_, patterns)| patterns.iter())
                .map(|(pattern, reg)| (pattern.to_string(), Arc::as_ptr(reg)))
                .collect();
            entries.sort();
            (cache.compiled.len(), entries)
        };
        let before = snapshot(&cache);

        let failing = ["a", "c", "(", "d"];
        let error = Scanner::with_pattern_cache(&failing, &config, &mut cache).err();
        assert!(error.is_some());
        assert_eq!(error, Scanner::with_config(&failing, &config).err());
        assert_eq!(snapshot(&cache), before);

        // Every pattern compiles, but the RegSet refuses FIND_LONGEST.
        let longest = ScannerConfig {
            options: config.options | ONIG_OPTION_FIND_LONGEST,
            ..config.clone()
        };
        let error = Scanner::with_pattern_cache(&["e", "f"], &longest, &mut cache).err();
        assert!(error.is_some());
        assert_eq!(error, Scanner::with_config(&["e", "f"], &longest).err());
        assert_eq!(snapshot(&cache), before);

        // An empty list adds no setting either.
        let ruby = ScannerConfig {
            syntax: ScannerSyntax::Ruby,
            ..config.clone()
        };
        let mut empty = Scanner::with_pattern_cache(&[], &ruby, &mut cache).unwrap();
        assert_eq!(
            empty.find_next_match("a", 0, ScannerFindOptions::NONE),
            None
        );
        assert_eq!(snapshot(&cache), before);

        // The kept entries still serve; the compiled `c` was dropped.
        let next = Scanner::with_pattern_cache(&["b", "c", "b"], &config, &mut cache).unwrap();
        let kept_addresses = compiled_addresses(&kept);
        let next_addresses = compiled_addresses(&next);
        assert_eq!(next_addresses[0], kept_addresses[1]);
        assert_eq!(next_addresses[2], kept_addresses[1]);
        assert_eq!(cache.len(), 3);
    }

    /// The cache and its scanners hold strong references: dropping
    /// scanners releases only theirs, and clearing or dropping the cache
    /// leaves built scanners working.
    #[test]
    fn pattern_cache_and_scanners_drop_in_any_order() {
        let config = ScannerConfig::default();
        let mut cache = ScannerPatternCache::new();
        assert!(cache.is_empty());
        let mut first = Scanner::with_pattern_cache(&["x(y)", "z"], &config, &mut cache).unwrap();
        let second = Scanner::with_pattern_cache(&["z"], &config, &mut cache).unwrap();
        let references = |cache: &ScannerPatternCache, pattern: &str| {
            Arc::strong_count(&cache.compiled[0].1[pattern])
        };
        assert_eq!(
            (references(&cache, "x(y)"), references(&cache, "z")),
            (2, 3)
        );
        drop(second);
        assert_eq!(references(&cache, "z"), 2);

        // The scanner outlives the cleared cache's references.
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        let whole = |m: Option<ScannerMatch>| {
            m.map(|m| {
                let spans: Vec<_> = m.capture_indices.iter().map(|c| (c.start, c.end)).collect();
                (m.index, spans)
            })
        };
        let found = whole(first.find_next_match("axyz", 0, ScannerFindOptions::NONE));
        assert_eq!(found, Some((0, vec![(1, 3), (2, 3)])));

        // A cleared cache compiles anew, and the cache outlives this
        // scanner as the next one outlives the cache.
        let third = Scanner::with_pattern_cache(&["z"], &config, &mut cache).unwrap();
        assert_ne!(compiled_addresses(&third)[0], compiled_addresses(&first)[1]);
        drop(first);
        let mut fourth = Scanner::with_pattern_cache(&["z"], &config, &mut cache).unwrap();
        drop(third);
        assert_eq!(references(&cache, "z"), 2);
        drop(cache);
        let found = whole(fourth.find_next_match("axyz", 0, ScannerFindOptions::NONE));
        assert_eq!(found, Some((0, vec![(3, 4)])));
    }

    /// Scanners that share compiled patterns search on several threads at
    /// once, including the lazily built literal searchers and automata of
    /// their shared patterns.
    #[test]
    fn scanners_sharing_a_pattern_cache_search_on_several_threads() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let patterns: &[&str] = &[
            r"[a-z]*(?:foo|bar|baz)",
            r"\w*end\b",
            r"(?i)(?:error|warn|fatal|panic)",
            r"(?<word>\w+)@(?<host>[a-z]+)",
            r"\s+",
        ];
        let subjects = [
            "a weekend of error reports from foo@bar",
            "WARN: the legend ends here, zbaz",
            "\u{1F4BB} fatal\tpanic   friend",
        ];
        let config = ScannerConfig::default();
        let mut reference = Scanner::with_config(patterns, &config).unwrap();
        let expected: Vec<Vec<Option<ScannerMatch>>> = subjects
            .iter()
            .map(|subject| {
                (0..=subject.len())
                    .filter(|&at| subject.is_char_boundary(at))
                    .map(|at| reference.find_next_match(subject, at, ScannerFindOptions::NONE))
                    .collect()
            })
            .collect();
        let mut cache = ScannerPatternCache::new();
        let scanners: Vec<Scanner> = (0..4)
            .map(|_| Scanner::with_pattern_cache(patterns, &config, &mut cache).unwrap())
            .collect();
        std::thread::scope(|scope| {
            for mut scanner in scanners {
                let expected = &expected;
                scope.spawn(move || {
                    for _ in 0..20 {
                        for (subject, expected) in subjects.iter().zip(expected) {
                            let found: Vec<_> = (0..=subject.len())
                                .filter(|&at| subject.is_char_boundary(at))
                                .map(|at| {
                                    scanner.find_next_match(subject, at, ScannerFindOptions::NONE)
                                })
                                .collect();
                            assert_eq!(&found, expected);
                        }
                    }
                });
            }
        });
    }

    /// Replays every call of the captured Shiki sessions
    /// (`benches/*_scanner/trace.json`) through scanners built from one
    /// pattern cache per session. Each distinct pattern is compiled once and
    /// shared by all its occurrences, each scanner's warnings are those of
    /// its patterns compiled on their own, and every result equals the
    /// captured one, which the benchmarks check against C.
    #[test]
    fn pattern_cache_replays_the_captured_scanner_traces() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        type Expected = Option<(usize, Vec<(usize, usize)>)>;
        let index = |value: &serde_json::Value| value.as_u64().unwrap() as usize;
        for (name, distinct_patterns) in [("cpp", 250), ("java", 115), ("scss", 104)] {
            let path = format!(
                "{}/benches/{name}_scanner/trace.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let trace: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let pattern_sets: Vec<Vec<&str>> = trace["scanners"]
                .as_array()
                .unwrap()
                .iter()
                .map(|set| {
                    let set = set.as_array().unwrap();
                    set.iter().map(|p| p.as_str().unwrap()).collect()
                })
                .collect();
            let config = ScannerConfig::default();
            let mut cache = ScannerPatternCache::new();
            let mut scanners: Vec<Scanner> = pattern_sets
                .iter()
                .map(|patterns| Scanner::with_pattern_cache(patterns, &config, &mut cache).unwrap())
                .collect();
            assert_eq!(cache.len(), distinct_patterns, "{name}");

            let mut fresh_warnings = HashMap::new();
            let mut addresses = HashMap::new();
            for (patterns, scanner) in pattern_sets.iter().zip(&scanners) {
                let compiled = patterns.iter().zip(compiled_addresses(scanner));
                for (j, (pattern, address)) in compiled.enumerate() {
                    assert_eq!(*addresses.entry(*pattern).or_insert(address), address);
                    let fresh = fresh_warnings
                        .entry(*pattern)
                        .or_insert_with(|| Scanner::new(&[pattern]).unwrap().warnings()[0].clone());
                    assert_eq!(&scanner.warnings()[j], fresh, "{name}: {pattern}");
                }
            }
            assert_eq!(addresses.len(), distinct_patterns, "{name}");

            let strings: Vec<OnigString> = trace["subjects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| OnigString::new(s.as_str().unwrap()))
                .collect();
            for (i, row) in trace["calls"].as_array().unwrap().iter().enumerate() {
                let expected: Expected = (!row[4].is_null()).then(|| {
                    let captures = row[5].as_array().unwrap();
                    let captures = captures.iter().map(|c| (index(&c[0]), index(&c[1])));
                    (index(&row[4]), captures.collect())
                });
                let found = scanners[index(&row[0])].find_next_match_utf16(
                    &strings[index(&row[1])],
                    index(&row[2]),
                    ScannerFindOptions::from_bits(index(&row[3]) as u32),
                );
                let found: Expected = found.map(|m| {
                    let captures = m.capture_indices.iter().map(|c| (c.start, c.end));
                    (m.index, captures.collect())
                });
                assert_eq!(found, expected, "{name}: call {i}");
            }
        }
    }
}
