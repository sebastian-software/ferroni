//! Replacement and splitting for the idiomatic API. The [`Replacer`] trait
//! drives [`Regex::replace_all`] and its relatives, and [`Split`] and its
//! relatives back [`Regex::split`]. They follow the shape of the `regex` crate
//! and walk the same matches as [`Regex::find_iter`] and
//! [`Regex::captures_iter`].

// Each method includes the empty match that Ferroni reports right after a
// non-empty one, so every method here agrees with those iterators.

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

use crate::api::{Captures, FindIter, Match, Regex};

// === Replacer ===

/// Produces the text that replaces one match in [`Regex::replace`] and its
/// relatives.
///
/// Implemented for `&str`, `String`, `&String` and `Cow<str>`, which expand
/// `$` references with [`Captures::expand`]; for [`NoExpand`], which inserts
/// its text unchanged; and for closures that map a [`Captures`] to a string.
///
/// # Examples
///
/// ```
/// use ferroni::prelude::*;
///
/// let re = Regex::new(r"(?<word>\w+)@").unwrap();
/// assert_eq!(re.replace_all("ada@ grace@", "${word}"), "ada grace");
/// assert_eq!(re.replace_all("ada@", |caps: &Captures| caps["word"].to_uppercase()), "ADA");
/// assert_eq!(re.replace_all("ada@", NoExpand("$word")), "$word");
/// ```
pub trait Replacer {
    /// Append the replacement for the match `caps` to `dst`.
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String);

    /// The replacement as a fixed string, when it needs no expansion.
    ///
    /// Returning `Some` lets the replace methods skip capture groups and use
    /// the cheaper `find_iter`. Return `None` (the default) when the
    /// replacement depends on the match.
    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        None
    }

    /// A mutable reference to this replacer, for passing it by reference.
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"a").unwrap();
    /// let mut rep = |_: &Captures| String::from("b");
    /// assert_eq!(re.replace_all("aa", rep.by_ref()), "bb");
    /// assert_eq!(re.replace_all("aa", rep), "bb");
    /// ```
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}

/// The replacement for a template with no `$`, or `None` if it may expand.
fn literal_template(template: &str) -> Option<Cow<'_, str>> {
    (!template.contains('$')).then_some(Cow::Borrowed(template))
}

impl Replacer for &str {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        caps.expand(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        literal_template(self)
    }
}

impl Replacer for String {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        caps.expand(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        literal_template(self)
    }
}

impl Replacer for &String {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        caps.expand(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        literal_template(self)
    }
}

impl Replacer for Cow<'_, str> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        caps.expand(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        literal_template(self)
    }
}

impl<F, T> Replacer for F
where
    F: FnMut(&Captures<'_>) -> T,
    T: AsRef<str>,
{
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        dst.push_str(self(caps).as_ref());
    }
}

/// A replacement that is inserted exactly as given, with no `$` expansion.
///
/// # Examples
///
/// ```
/// use ferroni::prelude::*;
///
/// let re = Regex::new(r"\d+").unwrap();
/// assert_eq!(re.replace_all("a1 b22", NoExpand("$0")), "a$0 b$0");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoExpand<'t>(pub &'t str);

impl Replacer for NoExpand<'_> {
    fn replace_append(&mut self, _caps: &Captures<'_>, dst: &mut String) {
        dst.push_str(self.0);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(self.0))
    }
}

// === ReplacerBytes ===

/// The byte-slice counterpart of [`Replacer`], used by the `_bytes` replace
/// methods such as [`Regex::replace_all_bytes`].
///
/// Implemented for `&[u8]`, `Vec<u8>`, `&Vec<u8>` and `Cow<[u8]>`, which expand
/// `$` references with [`Captures::expand_bytes`]; for [`NoExpandBytes`]; and
/// for closures that map a [`Captures`] to bytes. Unlike [`Replacer`], it
/// does not accept `&str`, as in the `regex` crate's `bytes` module.
///
/// It has no `by_ref`: a closure implements both traits, so a second method of
/// that name would make `closure.by_ref()` ambiguous under the prelude. Pass
/// `&mut closure` instead, which implements this trait as well.
///
/// # Examples
///
/// ```
/// use ferroni::prelude::*;
///
/// let re = Regex::new(r"(?<word>\w+)@").unwrap();
/// assert_eq!(re.replace_all_bytes(b"ada@", &b"${word}"[..]), &b"ada"[..]);
/// ```
pub trait ReplacerBytes {
    /// Append the replacement for the match `caps` to `dst`.
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut Vec<u8>);

    /// The replacement as fixed bytes, when it needs no expansion. See
    /// [`Replacer::no_expansion`].
    fn no_expansion(&mut self) -> Option<Cow<'_, [u8]>> {
        None
    }
}

/// The replacement for a template with no `$`, or `None` if it may expand.
fn literal_bytes_template(template: &[u8]) -> Option<Cow<'_, [u8]>> {
    (!template.contains(&b'$')).then_some(Cow::Borrowed(template))
}

impl ReplacerBytes for &[u8] {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut Vec<u8>) {
        caps.expand_bytes(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, [u8]>> {
        literal_bytes_template(self)
    }
}

impl ReplacerBytes for Vec<u8> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut Vec<u8>) {
        caps.expand_bytes(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, [u8]>> {
        literal_bytes_template(self)
    }
}

impl ReplacerBytes for &Vec<u8> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut Vec<u8>) {
        caps.expand_bytes(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, [u8]>> {
        literal_bytes_template(self)
    }
}

impl ReplacerBytes for Cow<'_, [u8]> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut Vec<u8>) {
        caps.expand_bytes(self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, [u8]>> {
        literal_bytes_template(self)
    }
}

impl<F, T> ReplacerBytes for F
where
    F: FnMut(&Captures<'_>) -> T,
    T: AsRef<[u8]>,
{
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut Vec<u8>) {
        dst.extend_from_slice(self(caps).as_ref());
    }
}

/// A byte replacement that is inserted exactly as given, with no `$`
/// expansion. The byte counterpart of [`NoExpand`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoExpandBytes<'t>(pub &'t [u8]);

impl ReplacerBytes for NoExpandBytes<'_> {
    fn replace_append(&mut self, _caps: &Captures<'_>, dst: &mut Vec<u8>) {
        dst.extend_from_slice(self.0);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, [u8]>> {
        Some(Cow::Borrowed(self.0))
    }
}

// === Template expansion ===

/// One piece of a replacement template.
#[derive(Debug, PartialEq, Eq)]
enum Piece<'a> {
    /// Text that is copied as it is. A lone `$` that starts no reference is
    /// one of these, as is the `$` of an escaped `$$`.
    Literal(&'a [u8]),
    /// A reference to a group by number (`$1`) or by name (`$name`), as
    /// written between the `$` and the end of the reference.
    Group(&'a str),
}

/// Splits a template into [`Piece`]s by the rules of [`Captures::expand`].
struct Pieces<'a> {
    rest: &'a [u8],
}

fn pieces(template: &[u8]) -> Pieces<'_> {
    Pieces { rest: template }
}

impl<'a> Iterator for Pieces<'a> {
    type Item = Piece<'a>;

    fn next(&mut self) -> Option<Piece<'a>> {
        if self.rest.is_empty() {
            return None;
        }
        let Some(dollar) = self.rest.iter().position(|&byte| byte == b'$') else {
            let text = self.rest;
            self.rest = &[];
            return Some(Piece::Literal(text));
        };
        if dollar > 0 {
            let (text, rest) = self.rest.split_at(dollar);
            self.rest = rest;
            return Some(Piece::Literal(text));
        }
        if self.rest.get(1) == Some(&b'$') {
            self.rest = &self.rest[2..];
            return Some(Piece::Literal(b"$"));
        }
        match reference(self.rest) {
            Some((name, len)) => {
                self.rest = &self.rest[len..];
                Some(Piece::Group(name))
            }
            None => {
                self.rest = &self.rest[1..];
                Some(Piece::Literal(b"$"))
            }
        }
    }
}

/// Whether `byte` may appear in a group name in a replacement: `[_0-9A-Za-z]`.
fn is_reference_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

/// The group reference at the start of `rest`, which begins with `$`: the
/// name and the length of the reference, or `None` if the `$` is literal.
///
/// A plain reference takes the longest run of `[_0-9A-Za-z]` after the `$`.
/// A braced one, `${...}`, runs to the first `}` and takes everything between
/// the braces, as the `regex` crate does, so `${a-b}` names a group `a-b`. It
/// stays literal when no `}` follows, or when its name is not UTF-8.
fn reference(rest: &[u8]) -> Option<(&str, usize)> {
    if rest.get(1) == Some(&b'{') {
        let close = rest[2..].iter().position(|&byte| byte == b'}')?;
        let name = std::str::from_utf8(&rest[2..2 + close]).ok()?;
        Some((name, close + 3))
    } else {
        let name_len = rest[1..]
            .iter()
            .take_while(|&&byte| is_reference_byte(byte))
            .count();
        if name_len == 0 {
            return None;
        }
        // The bytes are ASCII, so they are valid UTF-8.
        let name = std::str::from_utf8(&rest[1..=name_len]).ok()?;
        Some((name, name_len + 1))
    }
}

/// The text of the group that a `$` reference names, or `None` if it names no
/// participating group.
///
/// A name that parses as a number refers to that group, so `$01` is group 1.
/// Any other name is resolved as [`Captures::name`] resolves it, which finds
/// nothing for a name that no group has.
fn referenced_group<'t>(caps: &Captures<'t>, name: &str) -> Option<Match<'t>> {
    match name.parse::<usize>() {
        Ok(index) => caps.get(index),
        Err(_) => caps.name(name),
    }
}

impl Captures<'_> {
    /// Append `replacement` to `dst`, with each `$` reference replaced by the
    /// text of the group it names.
    ///
    /// The references are:
    ///
    /// - `$1` or `${1}`: the group with that number. Group 0 is the whole match.
    /// - `$name` or `${name}`: the group with that name, resolved as
    ///   [`Captures::name`] does, so a name shared by several groups yields the
    ///   last one that participated.
    /// - `$$`: a literal `$`.
    ///
    /// An unbraced name is the longest run of `[_0-9A-Za-z]` after the `$`, so
    /// `$1x` refers to a group named `1x`, not to group 1 followed by `x`. Write
    /// `${1}x` to follow a group with letters. A braced reference runs to the
    /// first `}`, and what lies between the braces is looked up as a number or
    /// a name, so `${a-b}` refers to a group named `a-b`.
    ///
    /// A reference to a group that does not exist, or that did not participate
    /// in the match, expands to nothing. A `$` that starts no reference, such as
    /// a `$` at the end, a `$` before a character that cannot start a name, or
    /// a `${` with no `}` after it, is copied literally.
    ///
    /// # Named groups hide plain groups
    ///
    /// As in Oniguruma, a pattern with named groups does not capture its plain
    /// `(...)` groups, unless the pattern was built with
    /// [`RegexBuilder::capture_group(true)`](crate::api::RegexBuilder::capture_group).
    /// So `$2` in `(?<y>\d+)-(\d+)` refers to no group and expands to nothing,
    /// while the named `$y` works:
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"(?<y>\d+)-(\d+)").unwrap();
    /// let caps = re.captures("2026-10").unwrap();
    /// let mut out = String::new();
    /// caps.expand("$y/$2", &mut out);
    /// assert_eq!(out, "2026/");
    ///
    /// let re = Regex::builder(r"(?<y>\d+)-(\d+)").capture_group(true).build().unwrap();
    /// let caps = re.captures("2026-10").unwrap();
    /// let mut out = String::new();
    /// caps.expand("$y/$2", &mut out);
    /// assert_eq!(out, "2026/10");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a group that the template inserts is not valid UTF-8. That
    /// only happens for captures from the `_bytes` methods; use
    /// [`Captures::expand_bytes`] for those.
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"(?<word>\w+)-(?<num>\d+)").unwrap();
    /// let caps = re.captures("item-42").unwrap();
    ///
    /// let mut out = String::new();
    /// caps.expand("$word:$2 $0 $$ $3 ${num}x $num-", &mut out);
    /// assert_eq!(out, "item:42 item-42 $  42x 42-");
    /// ```
    pub fn expand(&self, replacement: &str, dst: &mut String) {
        for piece in pieces(replacement.as_bytes()) {
            match piece {
                // The pieces are cut only at ASCII bytes, so each literal piece
                // is valid UTF-8 when the template is.
                Piece::Literal(text) => dst.push_str(
                    std::str::from_utf8(text).expect("a str template splits at ASCII bytes"),
                ),
                Piece::Group(name) => {
                    if let Some(group) = referenced_group(self, name) {
                        dst.push_str(group.as_str());
                    }
                }
            }
        }
    }

    /// Append `replacement` to `dst` as bytes, with the same rules as
    /// [`Captures::expand`]. Use it for captures from the `_bytes` methods,
    /// whose groups need not be valid UTF-8.
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"(?<word>\w+)").unwrap();
    /// let caps = re.captures_bytes(b"hello").unwrap();
    /// let mut out = Vec::new();
    /// caps.expand_bytes(b"[$word]", &mut out);
    /// assert_eq!(out, b"[hello]");
    /// ```
    pub fn expand_bytes(&self, replacement: &[u8], dst: &mut Vec<u8>) {
        for piece in pieces(replacement) {
            match piece {
                Piece::Literal(text) => dst.extend_from_slice(text),
                Piece::Group(name) => {
                    if let Some(group) = referenced_group(self, name) {
                        dst.extend_from_slice(group.as_bytes());
                    }
                }
            }
        }
    }
}

// === Replace ===

/// The number of matches to replace for a `limit`, where 0 means all of them.
fn replacement_limit(limit: usize) -> usize {
    if limit == 0 { usize::MAX } else { limit }
}

impl Regex {
    /// Replace the first match in `text` with `rep`.
    ///
    /// See [`Regex::replacen`]. When `rep` is a `&str`, `String` or `Cow<str>`,
    /// `$` references expand as described in [`Captures::expand`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"\d+").unwrap();
    /// assert_eq!(re.replace("a1 b2", "#"), "a# b2");
    /// ```
    pub fn replace<'h, R: Replacer>(&self, text: &'h str, rep: R) -> Cow<'h, str> {
        self.replacen(text, 1, rep)
    }

    /// Replace every match in `text` with `rep`.
    ///
    /// The matches are those of [`Regex::find_iter`], so an empty match right
    /// after a non-empty one is replaced too. Ferroni reports that match where
    /// the `regex` crate does not. Here `a*` matches `a` at 0, then an empty
    /// match at 1, then one at 2:
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"a*").unwrap();
    /// assert_eq!(re.replace_all("ab", "-"), "--b-");
    /// ```
    ///
    /// # Named groups and `$n`
    ///
    /// A `$2` refers to a plain `(...)` group only when that group is captured.
    /// In a pattern with named groups, plain groups are not captured unless the
    /// builder sets `capture_group(true)`. See [`Captures::expand`] for the
    /// details and an example.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
    /// assert_eq!(re.replace_all("2025-01 and 2026-10", "$month/$year"), "01/2025 and 10/2026");
    ///
    /// // A closure gets the captures of each match.
    /// let shout = Regex::new(r"[a-z]+").unwrap();
    /// let loud = shout.replace_all("make it loud", |caps: &Captures| {
    ///     caps[0].to_uppercase()
    /// });
    /// assert_eq!(loud, "MAKE IT LOUD");
    /// ```
    ///
    /// A text with no match is returned borrowed, without allocating:
    ///
    /// ```
    /// use ferroni::prelude::*;
    /// use std::borrow::Cow;
    ///
    /// let re = Regex::new(r"\d+").unwrap();
    /// assert!(matches!(re.replace_all("none", "#"), Cow::Borrowed(_)));
    /// ```
    pub fn replace_all<'h, R: Replacer>(&self, text: &'h str, rep: R) -> Cow<'h, str> {
        self.replacen(text, 0, rep)
    }

    /// Replace the first `limit` matches in `text` with `rep`, or all of them
    /// when `limit` is 0.
    ///
    /// The matches are those of [`Regex::find_iter`] (see
    /// [`Regex::replace_all`]). The result borrows `text` when nothing is
    /// replaced, and is a new string otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"\d").unwrap();
    /// assert_eq!(re.replacen("1 2 3", 2, "x"), "x x 3");
    /// assert_eq!(re.replacen("1 2 3", 0, "x"), "x x x");
    /// ```
    pub fn replacen<'h, R: Replacer>(
        &self,
        text: &'h str,
        limit: usize,
        mut rep: R,
    ) -> Cow<'h, str> {
        if let Some(literal) = rep.no_expansion() {
            // Inserting a fixed string needs no capture groups.
            let mut out: Option<String> = None;
            let mut last = 0;
            for m in self.find_iter(text).take(replacement_limit(limit)) {
                let out = out.get_or_insert_with(|| String::with_capacity(text.len()));
                out.push_str(&text[last..m.start()]);
                out.push_str(&literal);
                last = m.end();
            }
            return finish(text, out, last);
        }

        let mut out: Option<String> = None;
        let mut last = 0;
        for caps in self.captures_iter(text).take(replacement_limit(limit)) {
            // Captures only come from successful searches, so group 0 is set.
            let whole = caps.get(0).expect("group 0 is the match");
            let out = out.get_or_insert_with(|| String::with_capacity(text.len()));
            out.push_str(&text[last..whole.start()]);
            rep.replace_append(&caps, out);
            last = whole.end();
        }
        finish(text, out, last)
    }

    /// Replace the first match in `text` (as bytes) with `rep`.
    ///
    /// See [`Regex::replacen_bytes`].
    pub fn replace_bytes<'h, R: ReplacerBytes>(&self, text: &'h [u8], rep: R) -> Cow<'h, [u8]> {
        self.replacen_bytes(text, 1, rep)
    }

    /// Replace every match in `text` (as bytes) with `rep`.
    ///
    /// See [`Regex::replace_all`] for how the matches are chosen.
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"\s+").unwrap();
    /// assert_eq!(re.replace_all_bytes(b"a  b\tc", &b" "[..]), &b"a b c"[..]);
    /// ```
    pub fn replace_all_bytes<'h, R: ReplacerBytes>(&self, text: &'h [u8], rep: R) -> Cow<'h, [u8]> {
        self.replacen_bytes(text, 0, rep)
    }

    /// Replace the first `limit` matches in `text` (as bytes) with `rep`, or
    /// all of them when `limit` is 0.
    ///
    /// Behaves as [`Regex::replacen`] does, for text that need not be valid
    /// UTF-8. The result borrows `text` when nothing is replaced.
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"(?<d>\d)").unwrap();
    /// assert_eq!(re.replacen_bytes(b"1 2", 1, &b"<$d>"[..]), &b"<1> 2"[..]);
    /// ```
    pub fn replacen_bytes<'h, R: ReplacerBytes>(
        &self,
        text: &'h [u8],
        limit: usize,
        mut rep: R,
    ) -> Cow<'h, [u8]> {
        if let Some(literal) = rep.no_expansion() {
            let mut out: Option<Vec<u8>> = None;
            let mut last = 0;
            for m in self.find_iter_bytes(text).take(replacement_limit(limit)) {
                let out = out.get_or_insert_with(|| Vec::with_capacity(text.len()));
                out.extend_from_slice(&text[last..m.start()]);
                out.extend_from_slice(&literal);
                last = m.end();
            }
            return finish_bytes(text, out, last);
        }

        let mut out: Option<Vec<u8>> = None;
        let mut last = 0;
        for caps in self
            .captures_iter_bytes(text)
            .take(replacement_limit(limit))
        {
            let whole = caps.get(0).expect("group 0 is the match");
            let out = out.get_or_insert_with(|| Vec::with_capacity(text.len()));
            out.extend_from_slice(&text[last..whole.start()]);
            rep.replace_append(&caps, out);
            last = whole.end();
        }
        finish_bytes(text, out, last)
    }

    /// Iterate over the pieces of `text` between the matches of the pattern.
    ///
    /// The pieces are the text before the first match, the text between each
    /// pair of consecutive matches, and the text after the last match. So
    /// there is always one more piece than there are matches. The matches are
    /// those of [`Regex::find_iter`].
    ///
    /// An empty match splits the text at its position, so a piece can be
    /// empty. Ferroni also reports an empty match right after a non-empty one,
    /// which the `regex` crate does not. For `a*` on `ab` the matches are `a`
    /// at 0..1, then empty matches at 1 and at 2, so
    /// `Regex::new("a*").split("ab")` yields `"", "", "b", ""`, where the
    /// `regex` crate yields `"", "b", ""`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r"\s*,\s*").unwrap();
    /// let parts: Vec<&str> = re.split("a , b,c").collect();
    /// assert_eq!(parts, ["a", "b", "c"]);
    ///
    /// // A match at the start or end leaves an empty piece there.
    /// let parts: Vec<&str> = Regex::new(",").unwrap().split(",x,").collect();
    /// assert_eq!(parts, ["", "x", ""]);
    /// ```
    pub fn split<'r, 'h>(&'r self, text: &'h str) -> Split<'r, 'h> {
        Split {
            ranges: SplitRanges::new(self.find_iter(text), text.len()),
            text,
        }
    }

    /// Iterate over at most `limit` pieces of `text`, the last one holding the
    /// rest of the text unsplit.
    ///
    /// The first `limit - 1` pieces are those of [`Regex::split`], and the
    /// last piece is everything after them. A `limit` of 0 yields nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferroni::prelude::*;
    ///
    /// let re = Regex::new(r",").unwrap();
    /// let parts: Vec<&str> = re.splitn("a,b,c", 2).collect();
    /// assert_eq!(parts, ["a", "b,c"]);
    /// assert_eq!(re.splitn("a,b,c", 1).next(), Some("a,b,c"));
    /// assert_eq!(re.splitn("a,b,c", 0).next(), None);
    /// ```
    pub fn splitn<'r, 'h>(&'r self, text: &'h str, limit: usize) -> SplitN<'r, 'h> {
        SplitN {
            split: self.split(text),
            remaining: limit,
        }
    }

    /// Iterate over the pieces of `text` (as bytes) between the matches of the
    /// pattern. See [`Regex::split`].
    pub fn split_bytes<'r, 'h>(&'r self, text: &'h [u8]) -> SplitBytes<'r, 'h> {
        SplitBytes {
            ranges: SplitRanges::new(self.find_iter_bytes(text), text.len()),
            text,
        }
    }

    /// Iterate over at most `limit` pieces of `text` (as bytes). See
    /// [`Regex::splitn`].
    pub fn splitn_bytes<'r, 'h>(&'r self, text: &'h [u8], limit: usize) -> SplitNBytes<'r, 'h> {
        SplitNBytes {
            split: self.split_bytes(text),
            remaining: limit,
        }
    }
}

/// The result of a replace: `text` when nothing was replaced, otherwise the
/// output with the rest of `text` after `last` appended.
fn finish<'h>(text: &'h str, out: Option<String>, last: usize) -> Cow<'h, str> {
    match out {
        None => Cow::Borrowed(text),
        Some(mut out) => {
            out.push_str(&text[last..]);
            Cow::Owned(out)
        }
    }
}

/// [`finish`] for bytes.
fn finish_bytes(text: &[u8], out: Option<Vec<u8>>, last: usize) -> Cow<'_, [u8]> {
    match out {
        None => Cow::Borrowed(text),
        Some(mut out) => {
            out.extend_from_slice(&text[last..]);
            Cow::Owned(out)
        }
    }
}

// === Split ===

/// The byte ranges of the pieces between the matches of a [`FindIter`], shared
/// by the split iterators.
struct SplitRanges<'r, 't> {
    matches: FindIter<'r, 't>,
    /// Where the next piece starts: the end of the last match, or 0.
    last: usize,
    /// The length of the text, where the final piece ends.
    len: usize,
    /// Set once the final piece has been taken.
    done: bool,
}

impl<'r, 't> SplitRanges<'r, 't> {
    fn new(matches: FindIter<'r, 't>, len: usize) -> Self {
        SplitRanges {
            matches,
            last: 0,
            len,
            done: false,
        }
    }

    /// The next piece: the text up to the next match, or the final piece
    /// once the matches run out.
    fn next_range(&mut self) -> Option<Range<usize>> {
        if self.done {
            return None;
        }
        match self.matches.next() {
            Some(m) => {
                // Matches do not overlap and never start before the previous
                // one ended, so `last <= m.start()`.
                let piece = self.last..m.start();
                self.last = m.end();
                Some(piece)
            }
            None => {
                self.done = true;
                Some(self.last..self.len)
            }
        }
    }

    /// The rest of the text, from where the last piece ended to the end,
    /// with no further match taken as a boundary.
    fn rest_range(&mut self) -> Option<Range<usize>> {
        if self.done {
            return None;
        }
        self.done = true;
        Some(self.last..self.len)
    }
}

/// Iterator over the pieces of a `&str` between matches, from [`Regex::split`].
pub struct Split<'r, 'h> {
    ranges: SplitRanges<'r, 'h>,
    text: &'h str,
}

impl<'h> Iterator for Split<'_, 'h> {
    type Item = &'h str;

    fn next(&mut self) -> Option<&'h str> {
        let text = self.text;
        self.ranges.next_range().map(|range| &text[range])
    }
}

impl std::iter::FusedIterator for Split<'_, '_> {}

impl fmt::Debug for Split<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Split").finish_non_exhaustive()
    }
}

/// Iterator over at most a given number of pieces of a `&str`, from
/// [`Regex::splitn`].
pub struct SplitN<'r, 'h> {
    split: Split<'r, 'h>,
    remaining: usize,
}

impl<'h> Iterator for SplitN<'_, 'h> {
    type Item = &'h str;

    fn next(&mut self) -> Option<&'h str> {
        match self.remaining {
            0 => None,
            1 => {
                self.remaining = 0;
                let text = self.split.text;
                self.split.ranges.rest_range().map(|range| &text[range])
            }
            n => {
                self.remaining = n - 1;
                self.split.next()
            }
        }
    }
}

impl std::iter::FusedIterator for SplitN<'_, '_> {}

impl fmt::Debug for SplitN<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitN").finish_non_exhaustive()
    }
}

/// Iterator over the pieces of a byte slice between matches, from
/// [`Regex::split_bytes`].
pub struct SplitBytes<'r, 'h> {
    ranges: SplitRanges<'r, 'h>,
    text: &'h [u8],
}

impl<'h> Iterator for SplitBytes<'_, 'h> {
    type Item = &'h [u8];

    fn next(&mut self) -> Option<&'h [u8]> {
        let text = self.text;
        self.ranges.next_range().map(|range| &text[range])
    }
}

impl std::iter::FusedIterator for SplitBytes<'_, '_> {}

impl fmt::Debug for SplitBytes<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitBytes").finish_non_exhaustive()
    }
}

/// Iterator over at most a given number of pieces of a byte slice, from
/// [`Regex::splitn_bytes`].
pub struct SplitNBytes<'r, 'h> {
    split: SplitBytes<'r, 'h>,
    remaining: usize,
}

impl<'h> Iterator for SplitNBytes<'_, 'h> {
    type Item = &'h [u8];

    fn next(&mut self) -> Option<&'h [u8]> {
        match self.remaining {
            0 => None,
            1 => {
                self.remaining = 0;
                let text = self.split.text;
                self.split.ranges.rest_range().map(|range| &text[range])
            }
            n => {
                self.remaining = n - 1;
                self.split.next()
            }
        }
    }
}

impl std::iter::FusedIterator for SplitNBytes<'_, '_> {}

impl fmt::Debug for SplitNBytes<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitNBytes").finish_non_exhaustive()
    }
}
