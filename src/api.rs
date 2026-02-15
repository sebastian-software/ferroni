// api.rs - Idiomatic Rust API for Ferroni.
//
// Wraps the C-ported internals (onig_new, onig_search, etc.) with
// Rust-native types: Regex, RegexBuilder, Match, Captures, FindIter.

use std::ops::Range;

use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::error::RegexError;
use crate::oniguruma::*;
use crate::regcomp::onig_new;
use crate::regexec::{onig_name_to_group_numbers, onig_search};
use crate::regint::RegexType;
use crate::regsyntax::OnigSyntaxOniguruma;

/// A compiled regular expression.
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
pub struct Regex {
    inner: RegexType,
}

impl Regex {
    /// Compile a pattern using default options (Oniguruma syntax, UTF-8, no flags).
    pub fn new(pattern: &str) -> Result<Regex, RegexError> {
        Self::new_bytes(pattern.as_bytes())
    }

    /// Compile a pattern from raw bytes using default options.
    pub fn new_bytes(pattern: &[u8]) -> Result<Regex, RegexError> {
        let inner = onig_new(
            pattern,
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
        )?;
        Ok(Regex { inner })
    }

    /// Create a [`RegexBuilder`] for fine-grained control over compilation.
    pub fn builder(pattern: &str) -> RegexBuilder {
        RegexBuilder::new(pattern)
    }

    /// Return the first match in `text`, or `None` if no match.
    pub fn find<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.find_bytes(text.as_bytes())
    }

    /// Return the first match in `text` (as bytes), or `None` if no match.
    pub fn find_bytes<'t>(&self, text: &'t [u8]) -> Option<Match<'t>> {
        let (result, region) = onig_search(
            &self.inner,
            text,
            text.len(),
            0,
            text.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        if result < 0 {
            return None;
        }
        let region = region?;
        if region.num_regs < 1 {
            return None;
        }
        let start = region.beg[0] as usize;
        let end = region.end[0] as usize;
        Some(Match { text, start, end })
    }

    /// Check whether `text` matches the pattern anywhere.
    pub fn is_match(&self, text: &str) -> bool {
        self.is_match_bytes(text.as_bytes())
    }

    /// Check whether `text` (as bytes) matches the pattern anywhere.
    pub fn is_match_bytes(&self, text: &[u8]) -> bool {
        let (result, _) = onig_search(
            &self.inner,
            text,
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        result >= 0
    }

    /// Return the first match with all capture groups, or `None`.
    pub fn captures<'t>(&'t self, text: &'t str) -> Option<Captures<'t>> {
        self.captures_bytes(text.as_bytes())
    }

    /// Return the first match with all capture groups (bytes), or `None`.
    pub fn captures_bytes<'t>(&'t self, text: &'t [u8]) -> Option<Captures<'t>> {
        let (result, region) = onig_search(
            &self.inner,
            text,
            text.len(),
            0,
            text.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        if result < 0 {
            return None;
        }
        let region = region?;
        Some(Captures {
            text,
            region,
            regex: self,
        })
    }

    /// Iterate over all non-overlapping matches in `text`.
    pub fn find_iter<'r, 't>(&'r self, text: &'t str) -> FindIter<'r, 't> {
        FindIter {
            regex: self,
            text: text.as_bytes(),
            last_end: 0,
            last_was_empty: false,
        }
    }

    /// Iterate over all non-overlapping matches in `text` (as bytes).
    pub fn find_iter_bytes<'r, 't>(&'r self, text: &'t [u8]) -> FindIter<'r, 't> {
        FindIter {
            regex: self,
            text,
            last_end: 0,
            last_was_empty: false,
        }
    }

    /// Return the number of capture groups in the pattern (excluding group 0).
    pub fn captures_len(&self) -> usize {
        self.inner.num_mem as usize
    }

    /// Access the underlying `RegexType` for advanced / C-style usage.
    pub fn as_raw(&self) -> &RegexType {
        &self.inner
    }
}

impl std::fmt::Debug for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Regex").finish_non_exhaustive()
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
}

impl RegexBuilder {
    /// Create a new builder for the given pattern.
    pub fn new(pattern: &str) -> Self {
        RegexBuilder {
            pattern: pattern.as_bytes().to_vec(),
            options: ONIG_OPTION_NONE,
            syntax: &OnigSyntaxOniguruma,
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
    pub fn multi_line_anchors(mut self, yes: bool) -> Self {
        if yes {
            self.options |= ONIG_OPTION_SINGLELINE;
        } else {
            self.options &= !ONIG_OPTION_SINGLELINE;
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
    pub fn option(mut self, flag: OnigOptionType) -> Self {
        self.options |= flag;
        self
    }

    /// Select the syntax definition to use (default: Oniguruma).
    ///
    /// Pass one of the `OnigSyntax*` statics from [`crate::regsyntax`].
    pub fn syntax(mut self, syntax: &'static OnigSyntaxType) -> Self {
        self.syntax = syntax;
        self
    }

    /// Compile the pattern into a [`Regex`].
    pub fn build(self) -> Result<Regex, RegexError> {
        let inner = onig_new(&self.pattern, self.options, &ONIG_ENCODING_UTF8, self.syntax)?;
        Ok(Regex { inner })
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
pub struct Captures<'t> {
    text: &'t [u8],
    region: OnigRegion,
    regex: &'t Regex,
}

impl<'t> Captures<'t> {
    /// Get capture group `i`, or `None` if the group did not participate.
    ///
    /// Group 0 is the entire match.
    pub fn get(&self, i: usize) -> Option<Match<'t>> {
        if i >= self.region.num_regs as usize {
            return None;
        }
        let beg = self.region.beg[i];
        let end = self.region.end[i];
        if beg == ONIG_REGION_NOTPOS {
            return None;
        }
        Some(Match {
            text: self.text,
            start: beg as usize,
            end: end as usize,
        })
    }

    /// Get the first capture group with the given name, or `None`.
    pub fn name(&self, name: &str) -> Option<Match<'t>> {
        let nums = onig_name_to_group_numbers(&self.regex.inner, name.as_bytes()).ok()?;
        for &num in nums {
            let m = self.get(num as usize);
            if m.is_some() {
                return m;
            }
        }
        None
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

impl std::fmt::Debug for Captures<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_list();
        for i in 0..self.len() {
            list.entry(&self.get(i));
        }
        list.finish()
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

// === FindIter ===

/// Iterator over all non-overlapping matches in a text.
pub struct FindIter<'r, 't> {
    regex: &'r Regex,
    text: &'t [u8],
    last_end: usize,
    last_was_empty: bool,
}

impl<'r, 't> Iterator for FindIter<'r, 't> {
    type Item = Match<'t>;

    fn next(&mut self) -> Option<Match<'t>> {
        if self.last_end > self.text.len() {
            return None;
        }

        let (result, region) = onig_search(
            &self.regex.inner,
            self.text,
            self.text.len(),
            self.last_end,
            self.text.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );

        if result < 0 {
            return None;
        }

        let region = region?;
        if region.num_regs < 1 {
            return None;
        }

        let start = region.beg[0] as usize;
        let end = region.end[0] as usize;

        // Handle empty matches: advance by one byte to avoid infinite loop.
        if start == end {
            if self.last_was_empty {
                if self.last_end >= self.text.len() {
                    return None;
                }
                // Skip one character to avoid infinite loop on empty match
                self.last_end +=
                    self.regex.inner.enc.mbc_enc_len(&self.text[self.last_end..]);
                self.last_was_empty = false;
                return self.next();
            }
            self.last_was_empty = true;
        } else {
            self.last_was_empty = false;
        }

        self.last_end = end;

        Some(Match {
            text: self.text,
            start,
            end,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
