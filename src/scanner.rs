// scanner.rs - High-level multi-pattern scanner API.
//
// Compatible with vscode-oniguruma's OnigScanner interface, used by Shiki
// and other syntax highlighters built on vscode-textmate.

use smallvec::SmallVec;

use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::error::RegexError;
use crate::oniguruma::*;
use crate::regcomp::onig_new;
use crate::regexec::{onig_search_with_msa, MatchArg};
use crate::regint::RegexType;
use crate::regset::{onig_regset_new, onig_regset_search, OnigRegSet, OnigRegSetLead};
use crate::regsyntax::*;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScannerSyntax {
    /// Oniguruma syntax (default).
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
    fn to_onig_syntax(&self) -> &'static OnigSyntaxType {
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

impl Default for ScannerSyntax {
    fn default() -> Self {
        Self::Oniguruma
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
    content: String,
    /// Maps UTF-16 code unit index → UTF-8 byte offset. Length = utf16_len + 1.
    utf16_to_utf8: Vec<usize>,
    /// Maps UTF-8 byte offset → UTF-16 code unit index. Length = utf8_len + 1.
    utf8_to_utf16: Vec<usize>,
}

impl OnigString {
    /// Create a new `OnigString` from a Rust string, building offset tables.
    pub fn new(content: &str) -> Self {
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
            content: content.to_string(),
            utf16_to_utf8,
            utf8_to_utf16,
        }
    }

    /// The underlying UTF-8 string content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Length of the string in UTF-16 code units.
    pub fn utf16_len(&self) -> usize {
        self.utf16_to_utf8.len() - 1
    }

    /// Convert a UTF-16 code unit offset to a UTF-8 byte offset.
    fn utf16_offset_to_utf8(&self, utf16_offset: usize) -> usize {
        if utf16_offset >= self.utf16_to_utf8.len() {
            self.content.len()
        } else {
            self.utf16_to_utf8[utf16_offset]
        }
    }

    /// Convert a UTF-8 byte offset to a UTF-16 code unit offset.
    fn utf8_offset_to_utf16(&self, utf8_offset: usize) -> usize {
        if utf8_offset >= self.utf8_to_utf16.len() {
            self.utf16_len()
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

/// Threshold for switching between RegSet and per-regex search.
/// Matches vscode-oniguruma's `MAX_REGSET_MATCH_INPUT_LEN`.
const MAX_REGSET_MATCH_INPUT_LEN: usize = 1000;

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
    regexes: Vec<Box<RegexType>>,
    caches: Vec<CacheEntry>,
    regset: Box<OnigRegSet>,
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
        let syntax = config.syntax.to_onig_syntax();
        let options = config.options;

        let mut regexes = Vec::with_capacity(patterns.len());
        let mut caches = Vec::with_capacity(patterns.len());
        let mut regset_regs = Vec::with_capacity(patterns.len());

        for pattern in patterns {
            // Compile once for the per-regex search path.
            let reg = onig_new(
                pattern.as_bytes(),
                options,
                &ONIG_ENCODING_UTF8,
                syntax,
            )?;
            regexes.push(Box::new(reg));

            // Compile again for the RegSet (it takes ownership).
            let reg2 = onig_new(
                pattern.as_bytes(),
                options,
                &ONIG_ENCODING_UTF8,
                syntax,
            )?;
            regset_regs.push(Box::new(reg2));

            caches.push(CacheEntry::new(pattern));
        }

        let (regset, r) = onig_regset_new(regset_regs);
        if r != ONIG_NORMAL {
            return Err(r.into());
        }

        Ok(Scanner {
            regexes,
            caches,
            regset: regset.unwrap(),
        })
    }

    /// Find the next match starting at `start_position` (byte offset).
    ///
    /// For short strings (<1000 bytes), uses the RegSet fast path.
    /// For longer strings, uses per-regex search (no caching without a string ID).
    pub fn find_next_match(
        &mut self,
        text: &str,
        start_position: usize,
        options: ScannerFindOptions,
    ) -> Option<ScannerMatch> {
        self.find_next_match_inner(text, 0, start_position, options, false)
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
        self.find_next_match_inner(text, str_id, start_position, options, true)
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
        let utf8_start = string.utf16_offset_to_utf8(start_position);
        let m = self.find_next_match_inner(string.content(), 0, utf8_start, options, false)?;
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
        let utf8_start = string.utf16_offset_to_utf8(start_position);
        let m = self.find_next_match_inner(string.content(), str_id, utf8_start, options, true)?;
        Some(convert_match_to_utf16(string, m))
    }

    fn find_next_match_inner(
        &mut self,
        text: &str,
        str_id: u64,
        start_position: usize,
        options: ScannerFindOptions,
        use_cache: bool,
    ) -> Option<ScannerMatch> {
        let str_data = text.as_bytes();
        let end = str_data.len();

        if start_position > end {
            return None;
        }

        let onig_opts = options.to_onig_options();

        if end < MAX_REGSET_MATCH_INPUT_LEN {
            self.search_regset(str_data, end, start_position, onig_opts)
        } else {
            self.search_per_regex(str_data, end, start_position, str_id, options.0, onig_opts, use_cache)
        }
    }

    /// RegSet fast path for short strings.
    fn search_regset(
        &mut self,
        str_data: &[u8],
        end: usize,
        start: usize,
        option: OnigOptionType,
    ) -> Option<ScannerMatch> {
        let (idx, _pos) = onig_regset_search(
            &mut self.regset,
            str_data,
            end,
            start,
            end,
            OnigRegSetLead::PositionLead,
            option,
        );

        if idx < 0 {
            return None;
        }

        let regex_idx = idx as usize;
        let region = crate::regset::onig_regset_get_region(&self.regset, regex_idx)?;
        Some(build_scanner_match(regex_idx, region))
    }

    /// Per-regex search with caching for long strings.
    ///
    /// Regions are reused from cache entries to avoid per-call allocation.
    /// A single MatchArg is reused across all regex iterations to avoid
    /// repeated heap allocations for the VM stack.
    /// The best match is read directly from the cache at the end (no cloning).
    fn search_per_regex(
        &mut self,
        str_data: &[u8],
        end: usize,
        start: usize,
        str_id: u64,
        options_raw: u32,
        onig_opts: OnigOptionType,
        use_cache: bool,
    ) -> Option<ScannerMatch> {
        let mut best_index: Option<usize> = None;
        let mut best_pos: usize = usize::MAX;

        // Lazy MatchArg — only allocated on first cache miss (warm path: zero alloc)
        let mut msa: Option<MatchArg> = None;

        for i in 0..self.regexes.len() {
            let cache = &self.caches[i];

            // Check cache
            if use_cache
                && !cache.has_g_anchor
                && cache.last_str_id == str_id
                && cache.last_options == options_raw
                && cache.last_position <= start
            {
                if !cache.last_matched {
                    continue;
                }
                if cache.last_result >= 0 && (cache.last_result as usize) >= start {
                    let match_pos = cache.last_result as usize;
                    if match_pos < best_pos {
                        best_pos = match_pos;
                        best_index = Some(i);
                        if best_pos == start {
                            break;
                        }
                    }
                    continue;
                }
            }

            // Reuse the cached region (avoids allocation after first call)
            let region = self.caches[i].last_region.take()
                .unwrap_or_else(OnigRegion::new);

            // Create MatchArg on first miss, reuse on subsequent misses
            let msa = msa.get_or_insert_with(|| {
                MatchArg::new(&self.regexes[i], onig_opts, None, start)
            });
            msa.reset_for_search(&self.regexes[i], onig_opts, Some(region), start);

            let (r, returned_region) = onig_search_with_msa(
                &self.regexes[i],
                str_data,
                end,
                start,
                end,
                msa,
            );

            // Put region back in cache (no clone needed)
            let cache = &mut self.caches[i];
            cache.last_str_id = str_id;
            cache.last_position = start;
            cache.last_options = options_raw;
            cache.last_region = returned_region;

            if r >= 0 {
                cache.last_matched = true;
                cache.last_result = r;

                let match_pos = r as usize;
                if match_pos < best_pos {
                    best_pos = match_pos;
                    best_index = Some(i);
                    if best_pos == start {
                        break;
                    }
                }
            } else {
                cache.last_matched = false;
                cache.last_result = r;
            }
        }

        let idx = best_index?;
        let region = self.caches[idx].last_region.as_ref()?;
        Some(build_scanner_match(idx, region))
    }
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
                capture_indices: smallvec![CaptureIndex { start: 1, end: 4, length: 3 }],
            })
        );
        assert_eq!(
            scanner.find_next_match(s, 2, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex { start: 6, end: 8, length: 2 }],
            })
        );
    }

    /// Port of vscode-oniguruma `simple2`.
    #[test]
    fn vscode_simple2() {
        let mut scanner = Scanner::new(&["a", "b", "c"]).unwrap();
        assert_eq!(scanner.find_next_match("x", 0, ScannerFindOptions::NONE), None);
        assert_eq!(
            scanner.find_next_match("xxaxxbxxc", 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 2, end: 3, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match("xxaxxbxxc", 4, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex { start: 5, end: 6, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match("xxaxxbxxc", 7, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 2,
                capture_indices: smallvec![CaptureIndex { start: 8, end: 9, length: 1 }],
            })
        );
        assert_eq!(scanner.find_next_match("xxaxxbxxc", 9, ScannerFindOptions::NONE), None);
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
                capture_indices: smallvec![CaptureIndex { start: 8, end: 9, length: 1 }],
            })
        );

        let mut scanner2 = Scanner::new(&["\""]).unwrap();
        // '{"…": 1}': {=0 "=1 …=2..4 "=5 :=6 ' '=7 1=8 }=9
        // Start at byte 1, find '"' at byte 1
        assert_eq!(
            scanner2.find_next_match("{\"\\u{2026}\": 1}", 1, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 1, end: 2, length: 1 }],
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
                capture_indices: smallvec![CaptureIndex { start: 6, end: 7, length: 1 }],
            })
        );
        // From byte 5 (='b'): Y at byte 6
        assert_eq!(
            scanner.find_next_match(s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 6, end: 7, length: 1 }],
            })
        );
        // From byte 6 (='Y'): Y at byte 6
        assert_eq!(
            scanner.find_next_match(s, 6, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 6, end: 7, length: 1 }],
            })
        );
        // From byte 7 (='X'): X at byte 7
        assert_eq!(
            scanner.find_next_match(s, 7, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex { start: 7, end: 8, length: 1 }],
            })
        );
    }

    /// Port of vscode-oniguruma `unicode3`.
    /// 'Возврат' = 7 Cyrillic chars × 2 bytes each = 14 bytes
    #[test]
    fn vscode_unicode3() {
        let mut scanner = Scanner::new(&["\u{0412}\u{043E}\u{0437}\u{0432}\u{0440}\u{0430}\u{0442}"]).unwrap();
        let s = "\u{0412}\u{043E}\u{0437}\u{0432}\u{0440}\u{0430}\u{0442} long_var_name;";
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 0, end: 14, length: 14 }],
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
                capture_indices: smallvec![CaptureIndex { start: 0, end: 1, length: 1 }],
            })
        );
        // Start beyond end: no match
        assert_eq!(scanner.find_next_match(s, 1000, ScannerFindOptions::NONE), None);
    }

    /// Port of vscode-oniguruma `regex with \G`.
    #[test]
    fn vscode_g_anchor() {
        let mut scanner = Scanner::new(&["\\G-and"]).unwrap();
        let s = "first-and-second";
        assert_eq!(scanner.find_next_match(s, 0, ScannerFindOptions::NONE), None);
        assert_eq!(
            scanner.find_next_match(s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 5, end: 9, length: 4 }],
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
        assert_eq!(scanner.find_next_match(s, 0, ScannerFindOptions::NONE), None);
    }

    /// Port of vscode-oniguruma `FindOption.NotBeginString`.
    #[test]
    fn vscode_find_option_not_begin_string() {
        let mut scanner = Scanner::new(&["\\Afirst"]).unwrap();
        let s = "first-and-first";
        assert_eq!(scanner.find_next_match(s, 10, ScannerFindOptions::NONE), None);
        assert_eq!(
            scanner.find_next_match(s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 0, end: 5, length: 5 }],
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
                capture_indices: smallvec![CaptureIndex { start: 10, end: 15, length: 5 }],
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
                capture_indices: smallvec![CaptureIndex { start: 5, end: 9, length: 4 }],
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
                capture_indices: smallvec![CaptureIndex { start: 0, end: 4, length: 4 }],
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
                    CaptureIndex { start: 0, end: 15, length: 15 },
                    CaptureIndex { start: 0, end: 15, length: 15 },
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
                capture_indices: smallvec![CaptureIndex { start: 1, end: 4, length: 3 }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 2, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex { start: 6, end: 8, length: 2 }],
            })
        );
    }

    /// Port of vscode-oniguruma `simple2` — UTF-16 API.
    #[test]
    fn vscode_utf16_simple2() {
        let mut scanner = Scanner::new(&["a", "b", "c"]).unwrap();
        let x = OnigString::new("x");
        assert_eq!(scanner.find_next_match_utf16(&x, 0, ScannerFindOptions::NONE), None);
        let abc = OnigString::new("xxaxxbxxc");
        assert_eq!(
            scanner.find_next_match_utf16(&abc, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 2, end: 3, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&abc, 4, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex { start: 5, end: 6, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&abc, 7, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 2,
                capture_indices: smallvec![CaptureIndex { start: 8, end: 9, length: 1 }],
            })
        );
        assert_eq!(scanner.find_next_match_utf16(&abc, 9, ScannerFindOptions::NONE), None);
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
                capture_indices: smallvec![CaptureIndex { start: 6, end: 7, length: 1 }],
            })
        );

        let mut scanner2 = Scanner::new(&["\""]).unwrap();
        let s2 = OnigString::new("{\"\\u{2026}\": 1}");
        assert_eq!(
            scanner2.find_next_match_utf16(&s2, 1, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 1, end: 2, length: 1 }],
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
                capture_indices: smallvec![CaptureIndex { start: 4, end: 5, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 1, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 4, end: 5, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 3, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 4, end: 5, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 4, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 4, end: 5, length: 1 }],
            })
        );
        assert_eq!(
            scanner.find_next_match_utf16(&s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 1,
                capture_indices: smallvec![CaptureIndex { start: 5, end: 6, length: 1 }],
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
                capture_indices: smallvec![CaptureIndex { start: 0, end: 7, length: 7 }],
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
                capture_indices: smallvec![CaptureIndex { start: 0, end: 1, length: 1 }],
            })
        );
        assert_eq!(scanner.find_next_match_utf16(&s, 1000, ScannerFindOptions::NONE), None);
    }

    /// Port of vscode-oniguruma `regex with \G` — UTF-16 API.
    #[test]
    fn vscode_utf16_g_anchor() {
        let mut scanner = Scanner::new(&["\\G-and"]).unwrap();
        let s = OnigString::new("first-and-second");
        assert_eq!(scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE), None);
        assert_eq!(
            scanner.find_next_match_utf16(&s, 5, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 5, end: 9, length: 4 }],
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
        assert_eq!(scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE), None);
    }

    /// Port of vscode-oniguruma `FindOption.NotBeginString` — UTF-16 API.
    #[test]
    fn vscode_utf16_find_option_not_begin_string() {
        let mut scanner = Scanner::new(&["\\Afirst"]).unwrap();
        let s = OnigString::new("first-and-first");
        assert_eq!(scanner.find_next_match_utf16(&s, 10, ScannerFindOptions::NONE), None);
        assert_eq!(
            scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE),
            Some(ScannerMatch {
                index: 0,
                capture_indices: smallvec![CaptureIndex { start: 0, end: 5, length: 5 }],
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
                capture_indices: smallvec![CaptureIndex { start: 10, end: 15, length: 5 }],
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
                capture_indices: smallvec![CaptureIndex { start: 5, end: 9, length: 4 }],
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
}
