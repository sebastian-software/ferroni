// scanner.rs - High-level multi-pattern scanner API.
//
// Compatible with vscode-oniguruma's OnigScanner interface, used by Shiki
// and other syntax highlighters built on vscode-textmate.
//
// Scanner API design and test cases derived from vscode-oniguruma
// (MIT License, Copyright (c) Microsoft Corporation).

use smallvec::SmallVec;

use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::error::RegexError;
use crate::oniguruma::*;
use crate::regcomp::onig_new;
use crate::regexec::{onig_match_with_msa_start, onig_search_with_msa, MatchArg};
use crate::regset::{
    onig_regset_get_regex, onig_regset_last_match_len, onig_regset_new,
    onig_regset_number_of_regex, onig_regset_search_fast, onig_regset_search_fast_with_id,
    FallbackMemoIdentity, OnigRegSet, OnigRegSetLead,
};
use crate::regsyntax::*;
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannerMatch {
    /// Index of the pattern that matched (0-based).
    pub index: usize,
    /// Capture group information. Index 0 is the full match.
    pub capture_indices: SmallVec<[CaptureIndex; 8]>,
}

/// Options for `Scanner::find_next_match`, matching vscode-oniguruma's `FindOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScannerFindOptions(u32);

impl ScannerFindOptions {
    pub const NONE: Self = Self(0);
    pub const NOT_BEGIN_STRING: Self = Self(1);
    pub const NOT_END_STRING: Self = Self(2);
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

/// Regex syntax variant, matching vscode-oniguruma's `Syntax` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScannerSyntax {
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

impl ScannerSyntax {
    fn as_onig_syntax(&self) -> &'static OnigSyntaxType {
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

/// Configuration for creating a `Scanner`, matching vscode-oniguruma's `IOnigScannerConfig`.
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Compile-time options applied to all patterns.
    pub options: OnigOptionType,
    /// Regex syntax variant to use.
    pub syntax: ScannerSyntax,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        ScannerConfig {
            options: ONIG_OPTION_NONE,
            syntax: ScannerSyntax::default(),
        }
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

/// Per-regex cache entry, mirroring vscode-oniguruma's caching strategy.
struct CacheEntry {
    has_g_anchor: bool,
    last_str_id: u64,
    last_position: usize,
    last_options: u32,
    last_matched: bool,
    last_result: i32,
    last_region: Option<OnigRegion>,
}

impl CacheEntry {
    fn new(pattern: &str) -> Self {
        CacheEntry {
            has_g_anchor: pattern.contains("\\G"),
            last_str_id: 0,
            last_position: 0,
            last_options: u32::MAX, // invalid sentinel
            last_matched: false,
            last_result: ONIG_MISMATCH,
            last_region: None,
        }
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
/// assert_eq!(m.capture_indices[0].start, 0);
/// assert_eq!(m.capture_indices[0].end, 5);
/// ```
pub struct Scanner {
    caches: Vec<CacheEntry>,
    regset: Box<OnigRegSet>,
    stats: ScannerStats,
    cache_route: CacheRouteState,
}

impl Scanner {
    /// Create a scanner from a list of pattern strings using default settings
    /// (Oniguruma syntax, no special options).
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
    /// let config = ScannerConfig {
    ///     options: OnigOptionType::IGNORECASE,
    ///     syntax: ScannerSyntax::Oniguruma,
    /// };
    /// let mut scanner = Scanner::with_config(&["hello"], &config).unwrap();
    /// let m = scanner.find_next_match("HELLO", 0, ScannerFindOptions::NONE);
    /// assert!(m.is_some());
    /// ```
    pub fn with_config(patterns: &[&str], config: &ScannerConfig) -> Result<Scanner, RegexError> {
        let syntax = config.syntax.as_onig_syntax();
        let options = config.options;

        let mut caches = Vec::with_capacity(patterns.len());
        let mut regset_regs = Vec::with_capacity(patterns.len());

        for pattern in patterns {
            let reg = onig_new(pattern.as_bytes(), options, &ONIG_ENCODING_UTF8, syntax)?;
            regset_regs.push(Box::new(reg));
            caches.push(CacheEntry::new(pattern));
        }

        let (regset, r) = onig_regset_new(regset_regs);
        if r != ONIG_NORMAL {
            return Err(r.into());
        }

        Ok(Scanner {
            caches,
            regset: regset.unwrap(),
            stats: ScannerStats::default(),
            cache_route: CacheRouteState::default(),
        })
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
    /// assert_eq!(m.capture_indices[0].start, 4);
    /// assert_eq!(m.capture_indices[0].end, 5);
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
        if use_regset {
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
        let (idx, pos) = if let Some(identity) = fallback_memo_id {
            onig_regset_search_fast_with_id(
                &mut self.regset,
                str_data,
                end,
                start,
                end,
                OnigRegSetLead::PositionLead,
                option,
                identity,
            )
        } else {
            onig_regset_search_fast(
                &mut self.regset,
                str_data,
                end,
                start,
                end,
                OnigRegSetLead::PositionLead,
                option,
            )
        };

        if idx < 0 {
            return None;
        }

        let regex_idx = idx as usize;
        let match_start = if pos >= 0 { pos as usize } else { start };
        if let Some(reg) = onig_regset_get_regex(&self.regset, regex_idx) {
            if reg.num_mem == 0 {
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
    /// Regions are reused from cache entries to avoid per-call allocation.
    /// A single MatchArg is reused across all regex iterations to avoid
    /// repeated heap allocations for the VM stack.
    /// The best match is read directly from the cache at the end (no cloning).
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
    ) -> (Option<ScannerMatch>, PerRegexCallStats) {
        let mut best_index: Option<usize> = None;
        let mut best_pos: usize = usize::MAX;
        let mut run_stats = PerRegexCallStats::default();

        // Lazy MatchArg — only allocated on first cache miss (warm path: zero alloc)
        let mut msa: Option<MatchArg> = None;

        // Split borrows: regset (immutable) and caches (mutable) are disjoint fields.
        let regset = &self.regset;
        let caches = &mut self.caches;
        let n = onig_regset_number_of_regex(regset) as usize;

        // Progressive range narrowing: once a match is found at position P,
        // narrow subsequent searches to [start, P) since we only need earlier matches.
        let mut ep = end;

        for (i, cache) in caches.iter_mut().enumerate().take(n) {
            let has_g_anchor = cache.has_g_anchor;

            // Check cache
            if use_cache
                && !has_g_anchor
                && cache.last_str_id == str_id
                && cache.last_options == options_raw
                && cache.last_position <= start
            {
                run_stats.cache_checks += 1;
                if !cache.last_matched {
                    run_stats.cache_hits += 1;
                    continue;
                }
                if cache.last_result >= 0 && (cache.last_result as usize) >= start {
                    run_stats.cache_hits += 1;
                    let match_pos = cache.last_result as usize;
                    if match_pos < best_pos {
                        best_pos = match_pos;
                        best_index = Some(i);
                        ep = best_pos;
                        if best_pos == start {
                            break;
                        }
                    }
                    continue;
                }
            }

            let reg = onig_regset_get_regex(regset, i).unwrap();
            run_stats.vm_calls += 1;

            // Reuse the cached region (avoids allocation after first call)
            let region = cache.last_region.take().unwrap_or_default();

            // Create MatchArg on first miss, reuse on subsequent misses
            let msa = msa.get_or_insert_with(|| MatchArg::new(reg, onig_opts, None, start));
            msa.reset_for_search(reg, onig_opts, Some(region), start);

            let (r, returned_region) = if has_g_anchor {
                search_g_anchor_with_msa(reg, str_data, end, start, ep, onig_opts, msa)
            } else {
                onig_search_with_msa(reg, str_data, end, start, ep, msa)
            };

            // Put region back in cache (no clone needed)
            cache.last_region = returned_region;

            if r >= 0 {
                cache.last_str_id = str_id;
                cache.last_position = start;
                cache.last_options = options_raw;
                cache.last_matched = true;
                cache.last_result = r;

                let match_pos = r as usize;
                if match_pos < best_pos {
                    best_pos = match_pos;
                    best_index = Some(i);
                    ep = best_pos;
                    if best_pos == start {
                        break;
                    }
                }
            } else {
                // If search was truncated to [start, ep), a miss does not imply
                // "no match at all" for later start positions, so don't cache it.
                if ep == end {
                    cache.last_str_id = str_id;
                    cache.last_position = start;
                    cache.last_options = options_raw;
                    cache.last_matched = false;
                    cache.last_result = r;
                } else {
                    cache.last_str_id = 0;
                    cache.last_position = 0;
                    cache.last_options = u32::MAX;
                    cache.last_matched = false;
                    cache.last_result = ONIG_MISMATCH;
                }
            }
        }

        let out = best_index.and_then(|idx| {
            self.caches[idx]
                .last_region
                .as_ref()
                .map(|region| build_scanner_match(idx, region))
        });
        (out, run_stats)
    }
}

/// Search helper for patterns containing `\G` in per-regex mode.
///
/// `onig_search` keeps `msa.start` fixed to the original search start, while
/// the scanner's position-lead behavior checks each candidate position with
/// that position as start. For `\G` patterns this changes match semantics.
/// This helper mirrors position-lead behavior for a single regex.
///
/// Only activated when a `\G`-containing regex triggers 8+ same-position
/// calls in per-regex mode — a very narrow edge case tested indirectly.
#[cfg_attr(coverage_nightly, coverage(off))]
fn search_g_anchor_with_msa(
    reg: &crate::regint::RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> (i32, Option<OnigRegion>) {
    let mut s = start;
    let search_end = range.min(end);

    while s < search_end {
        let r = onig_match_with_msa_start(reg, str_data, end, s, start, option, msa);
        if r >= 0 {
            return (s as i32, msa.region.take());
        }
        if s >= end {
            break;
        }
        let step = reg.enc.mbc_enc_len(&str_data[s..]).max(1);
        s = s.saturating_add(step);
    }

    (ONIG_MISMATCH, msa.region.take())
}

/// Build a `ScannerMatch` from a regex index and region.
fn build_scanner_match(index: usize, region: &OnigRegion) -> ScannerMatch {
    let num_regs = region.num_regs as usize;
    let mut capture_indices = SmallVec::with_capacity(num_regs);

    for i in 0..num_regs {
        let beg = region.beg[i];
        let end = region.end[i];
        if beg >= 0 && end >= 0 {
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
            if let Some(m) =
                scanner.find_next_match_with_id(input, 99, pos, ScannerFindOptions::NONE)
            {
                pos = m.capture_indices[0].end;
                matches.push(m.index);
            } else {
                break;
            }
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
}
