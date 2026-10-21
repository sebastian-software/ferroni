//! Rust-only (ADR-008): the accept seek of a single `Regex`, an
//! under-approximation of its pattern.
//!
//! The seek of [`derive`](super::derive) over-approximates: where it finds
//! no match, the pattern has none, and only an exact seek's match is a match
//! of the pattern. Under `(?i)`, a class such as `[a-z¡-￿0-9]` holds
//! characters with multi-character case folds (`ß`, `ﬀ`), and the parser
//! adds the strings they fold to as alternatives beside the class, as C
//! does (`ss` as `[sSſ][sSſ]`, `ff`, ...): some seventy per occurrence.
//! Kept, they push a precise seek past `MAX_NFA_STATES`; the scanner's seek,
//! which stands in for it, approximates the class bytewise and is inexact.
//!
//! [`derive_accept`] reads the same tuned tree into a seek whose every match
//! is a match of the pattern at the same start: its language is a subset of
//! the pattern's, so "the accept seek matches" proves "the pattern matches"
//! and is what a standard `is_match` may answer. Every construct is written
//! exactly, with one exception, and a construct the walk cannot write
//! exactly leaves the pattern without an accept seek:
//!
//! - The strings a class's case folds add beside it are left out (a branch
//!   of an alternation led by a class that is a string, an alternation of
//!   strings, or a short sequence of strings and small classes, see
//!   `fold_strings`). Leaving out branches of an alternation outside any
//!   negative look-around or atomic group only removes ways to match.
//! - Classes are sets of code points, written as UTF-8 exactly
//!   (`ClassUnicode`), not as the seek's byte approximation.
//! - A negative look-ahead or look-behind of one character (`(?!-)`,
//!   `(?<!-)`) is folded into the item after or before it in its sequence,
//!   where that item reads at least one character on every path: the look
//!   then only constrains that character (`\.(?!-)C{1,63}(?<!-)` is
//!   `\.D(?:C{0,61}D)?` with `D` the class `C` without `-`). Any other
//!   look-around has no accept seek.
//! - An atomic group that the tuner made of a greedy repetition before an
//!   item that cannot start where the repetition's body does (C's
//!   `tune_next`, which marks it `ND_ST_STRICT_REAL_REPEAT`) matches what the
//!   plain repetition matches, so it is written as that repetition. Other
//!   atomic groups and possessive quantifiers have no accept seek.
//! - Anchors are written as the exact seek writes them (`\A`, `\z`, `$`, `^`
//!   before an item that reads a byte, `\Z` at the end of the pattern).
//!
//! Since only the fold strings are left out, the accept seek is exact on a
//! text that contains none of them: no path of the pattern through a
//! left-out string can match there ([`AcceptRejects`]). A pattern with no
//! fold strings gets no accept seek: it would be exact, but the seek
//! approximates such patterns for a reason (counted repetitions above
//! `MAX_COUNTED_REPEAT` are widened there and kept here), and their automata
//! can cost more than the rest of the compilation.

use super::{
    MAX_NFA_STATES, SeekCode, alternation, class_members, collect_tails, concat, cons_items,
    is_fail, line_starts_exact, states_estimate,
};
use crate::oniguruma::{
    ONIG_OPTION_FIND_NOT_EMPTY, ONIG_OPTION_NOTBOL, ONIG_OPTION_NOTEOL, ONIGENC_CTYPE_WORD,
};
use crate::regcomp::literal_alt_trie_index;
use crate::regint::*;
use crate::regparse_types::*;
use regex_syntax::hir::{Class, ClassUnicode, ClassUnicodeRange, Hir, Look, Repetition};

/// The most NFA states an accept seek may compile to, by `states_estimate`:
/// a quarter of the seek's bound. The django validators' accept seeks
/// estimate at about 5,500 and 6,000 and compile to some 2,300 states.
const MAX_ACCEPT_NFA_STATES: usize = MAX_NFA_STATES / 4;

/// The accept seek of a pattern, kept from the compilation until the
/// automaton is built.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct AcceptCode {
    /// The seek (never exact: something was left out).
    pub(super) code: SeekCode,
    pub(super) rejects: AcceptRejects,
}

impl std::fmt::Debug for AcceptCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcceptCode")
            .field("code", &self.code)
            .field("rejects", &self.rejects)
            .finish()
    }
}

/// Where a text the accept seek does not match has no match of the pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AcceptRejects {
    /// On a text that contains none of the left-out strings, which no path
    /// through them can then match.
    Unless(Box<FoldTriggers>),
    /// Nowhere: an empty string was left out, which every text contains.
    Never,
}

/// A superset of the strings the accept seek left out, to test a text for
/// cheaply: any non-ASCII byte (every left-out string with a non-ASCII
/// character has one), the single bytes of the one-byte strings, and the
/// first two bytes of each longer ASCII string (`ss`, `ff`, `st`, ...).
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FoldTriggers {
    /// For each first byte, the second bytes of the pairs it starts.
    pairs: [u128; 128],
    /// The one-byte strings.
    bytes: u128,
}

impl std::fmt::Debug for FoldTriggers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pairs: Vec<String> = (0..128u8)
            .flat_map(|a| {
                (0..128u8)
                    .filter(move |&b| self.pairs[a as usize] >> b & 1 != 0)
                    .map(move |b| String::from_utf8_lossy(&[a, b]).into_owned())
            })
            .collect();
        f.debug_struct("FoldTriggers")
            .field("pairs", &pairs)
            .field("bytes", &format_args!("{:#x}", self.bytes))
            .finish()
    }
}

impl FoldTriggers {
    /// No triggers.
    fn new() -> FoldTriggers {
        FoldTriggers {
            pairs: [0; 128],
            bytes: 0,
        }
    }

    /// Adds the triggers of a left-out non-empty `string`.
    fn add(&mut self, string: &[u8]) {
        match *string {
            [a] if a < 0x80 => self.bytes |= 1 << a,
            [a, b, ..] if a < 0x80 && b < 0x80 => self.pairs[a as usize] |= 1 << b,
            // A non-ASCII byte in the string: the text has one too.
            _ => {}
        }
    }

    /// Whether `text` may contain a left-out string.
    pub(super) fn occur_in(&self, text: &[u8]) -> bool {
        let mut previous: Option<u8> = None;
        for &byte in text {
            if byte >= 0x80 || self.bytes >> byte & 1 != 0 {
                return true;
            }
            if previous.is_some_and(|a| self.pairs[a as usize] >> byte & 1 != 0) {
                return true;
            }
            previous = Some(byte);
        }
        false
    }
}

/// The accept seek of `reg`, whose tuned parse tree is `root`, or `None`
/// where a construct has no exact writing (see the module notes), where the
/// seek would never match, or where its automaton would be too large. The
/// caller has checked the encoding and the callouts (`derive`).
pub(crate) fn derive_accept(root: &Node, reg: &RegexType) -> Option<AcceptCode> {
    // The options that change where a line starts or ends, or what counts
    // as a match.
    if reg
        .options
        .intersects(ONIG_OPTION_NOTBOL | ONIG_OPTION_NOTEOL | ONIG_OPTION_FIND_NOT_EMPTY)
    {
        return None;
    }
    // Only a pattern with fold strings to leave out gets an accept seek (see
    // `LeftOut::Nothing` below); most patterns have none, and this scan
    // costs them less than the walk.
    if !has_fold_strings(root) {
        return None;
    }
    let mut tails = Vec::new();
    collect_tails(root, &mut tails);
    let mut walk = AcceptWalk {
        reg,
        tails,
        left_out: LeftOut::Nothing,
    };
    let part = walk.node(root)?;
    let rejects = match walk.left_out {
        // Nothing left out: the seek is inexact for another reason (a
        // repetition `tune_next` made possessive, a Unicode class, a look),
        // and the accept seek would be exact. Such seeks are not built: their
        // counted repetitions are exact here where the seek widens them, and
        // their automata cost several times the compilation they belong to
        // (`semver_version_parse`), mostly for patterns that are never asked
        // `is_match`.
        LeftOut::Nothing => return None,
        LeftOut::Strings(triggers) => AcceptRejects::Unless(triggers),
        LeftOut::Empty => AcceptRejects::Never,
    };
    let hir = part.into_hir()?;
    if is_fail(&hir)
        || !line_starts_exact(&hir)
        || states_estimate(&hir, true) > MAX_ACCEPT_NFA_STATES
    {
        return None;
    }
    let code = SeekCode::encode(&hir, false)?;
    debug_assert_eq!(code.decode(), hir, "the accept code round-trips");
    Some(AcceptCode { code, rejects })
}

/// A part of the accept seek, before its look-arounds are folded away.
#[derive(Clone, Debug)]
enum Part {
    Empty,
    /// One character of the set.
    Char(ClassUnicode),
    /// A non-empty string.
    Text(Box<str>),
    /// A zero-width assertion the seek reads as the matcher does.
    Look(Look),
    Concat(Vec<Part>),
    Alt(Vec<Part>),
    Repeat {
        min: u32,
        max: Option<u32>,
        sub: Box<Part>,
    },
    /// `(?!S)` for a set `S` of single characters.
    NotNext(ClassUnicode),
    /// `(?<!S)` for a set `S` of single characters.
    NotPrev(ClassUnicode),
}

impl Part {
    /// Whether some path matches without reading a character.
    fn nullable(&self) -> bool {
        match self {
            Part::Empty | Part::Look(_) | Part::NotNext(_) | Part::NotPrev(_) => true,
            Part::Char(_) | Part::Text(_) => false,
            Part::Concat(parts) => parts.iter().all(Part::nullable),
            Part::Alt(branches) => branches.iter().any(Part::nullable),
            Part::Repeat { min, sub, .. } => *min == 0 || sub.nullable(),
        }
    }

    /// The HIR, or `None` while a look-around is left.
    fn into_hir(self) -> Option<Hir> {
        Some(match self {
            Part::Empty => Hir::empty(),
            Part::Char(set) => {
                if set.ranges().is_empty() {
                    Hir::fail()
                } else {
                    Hir::class(Class::Unicode(set))
                }
            }
            Part::Text(text) => Hir::literal(text.as_bytes()),
            Part::Look(look) => Hir::look(look),
            Part::Concat(parts) => concat(
                parts
                    .into_iter()
                    .map(Part::into_hir)
                    .collect::<Option<_>>()?,
            ),
            Part::Alt(branches) => alternation(
                branches
                    .into_iter()
                    .map(Part::into_hir)
                    .collect::<Option<_>>()?,
            ),
            Part::Repeat { min, max, sub } => Hir::repetition(Repetition {
                min,
                max,
                greedy: true,
                sub: Box::new(sub.into_hir()?),
            }),
            Part::NotNext(_) | Part::NotPrev(_) => return None,
        })
    }
}

/// `part` split into its matches that read a character and whether it also
/// matches the empty string with no condition, or `None` where its empty
/// matches carry one (a look-around).
fn split_empty(part: Part) -> Option<(Option<Part>, bool)> {
    if !part.nullable() {
        return Some((Some(part), false));
    }
    match part {
        Part::Empty => Some((None, true)),
        Part::Repeat { min: 0, max, sub } if !sub.nullable() => {
            let reading = (max != Some(0)).then_some(Part::Repeat { min: 1, max, sub });
            Some((reading, true))
        }
        Part::Alt(branches) => {
            let mut reading = Vec::new();
            let mut empty = false;
            for branch in branches {
                let (branch_reading, branch_empty) = split_empty(branch)?;
                reading.extend(branch_reading);
                empty |= branch_empty;
            }
            Some(((!reading.is_empty()).then_some(Part::Alt(reading)), empty))
        }
        _ => None,
    }
}

/// Which end of a match a one-character look constrains.
#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    /// The first character (`(?!S)` before the item).
    First,
    /// The last character (`(?<!S)` after the item).
    Last,
}

/// The matches of `part` whose character at `end` is not in `set`, or
/// `None` where `part` may match without reading a character (or that
/// cannot be told). Together with "`part` reads a character on every path"
/// this is exactly `(?!S)part` (`End::First`) or `part(?<!S)` (`End::Last`):
/// the look reads the character the item reads first or last.
fn exclude(part: Part, set: &ClassUnicode, end: End) -> Option<Part> {
    match part {
        Part::Char(mut chars) => {
            chars.difference(set);
            Some(Part::Char(chars))
        }
        Part::Text(text) => {
            let c = match end {
                End::First => text.chars().next()?,
                End::Last => text.chars().next_back()?,
            };
            let excluded = set
                .ranges()
                .iter()
                .any(|range| range.start() <= c && c <= range.end());
            Some(if excluded {
                Part::Char(ClassUnicode::empty())
            } else {
                Part::Text(text)
            })
        }
        Part::Alt(branches) => branches
            .into_iter()
            .map(|branch| exclude(branch, set, end))
            .collect::<Option<_>>()
            .map(Part::Alt),
        // `X{m,n}` with `m ≥ 1` is `X X{m-1,n-1}` (or `X{m-1,n-1} X`), and the
        // constrained copy reads the end character.
        Part::Repeat { min, max, sub } if min >= 1 && !sub.nullable() => {
            let rest = Part::Repeat {
                min: min - 1,
                max: max.map(|max| max - 1),
                sub: sub.clone(),
            };
            let one = exclude(*sub, set, end)?;
            Some(Part::Concat(match end {
                End::First => vec![one, rest],
                End::Last => vec![rest, one],
            }))
        }
        Part::Concat(mut parts) => {
            // Assertions at the constrained end hold at the same position as
            // the look, so the look passes them.
            if end == End::Last {
                parts.reverse();
            }
            let k = parts.iter().position(|p| !matches!(p, Part::Look(_)))?;
            let mut rest = parts.split_off(k);
            let head = rest.remove(0);
            let mut out = parts;
            if !head.nullable() {
                out.push(exclude(head, set, end)?);
                out.extend(rest);
            } else {
                // The item at the end reads a character, and it is the one
                // constrained, or it is empty and the next one's is.
                let (reading, empty) = split_empty(head)?;
                let mut branches = Vec::new();
                if let Some(reading) = reading {
                    let mut seq = vec![exclude(reading, set, end)?];
                    seq.extend(rest.iter().cloned());
                    if end == End::Last {
                        seq.reverse();
                    }
                    branches.push(Part::Concat(seq));
                }
                if empty {
                    if end == End::Last {
                        rest.reverse();
                    }
                    branches.push(exclude(Part::Concat(rest), set, end)?);
                }
                out.push(Part::Alt(branches));
            }
            if end == End::Last {
                out.reverse();
            }
            Some(Part::Concat(out))
        }
        _ => None,
    }
}

/// A sequence with its one-character looks folded into their neighbors where
/// that is exact: `(?<!S)` into the item before it, then `(?!S)` into the
/// item after it, each only where that item reads a character on every
/// path. A look that does not fold stays, and the sequence then has no HIR.
fn sequence(mut parts: Vec<Part>) -> Option<Part> {
    let mut k = 0;
    while k < parts.len() {
        if let Part::NotPrev(set) = &parts[k] {
            if k > 0 && !parts[k - 1].nullable() {
                let set = set.clone();
                let before = std::mem::replace(&mut parts[k - 1], Part::Empty);
                parts[k - 1] = exclude(before, &set, End::Last)?;
                parts.remove(k);
                continue;
            }
        }
        k += 1;
    }
    let mut k = parts.len();
    while k > 0 {
        k -= 1;
        if let Part::NotNext(set) = &parts[k] {
            if k + 1 < parts.len() && !parts[k + 1].nullable() {
                let set = set.clone();
                let after = std::mem::replace(&mut parts[k + 1], Part::Empty);
                parts[k + 1] = exclude(after, &set, End::First)?;
                parts.remove(k);
            }
        }
    }
    Some(if parts.len() == 1 {
        parts.pop()?
    } else {
        Part::Concat(parts)
    })
}

/// The code points of a class node as the matcher reads them over valid
/// UTF-8 (`class_members`), surrogates left out.
fn class_set(cc: &CClassNode) -> ClassUnicode {
    let mut ranges = Vec::new();
    for (lo, hi) in class_members(cc) {
        let hi = hi.min(0x10FFFF);
        for (lo, hi) in [(lo, hi.min(0xD7FF)), (lo.max(0xE000), hi)] {
            if lo <= hi {
                if let (Some(lo), Some(hi)) = (char::from_u32(lo), char::from_u32(hi)) {
                    ranges.push(ClassUnicodeRange::new(lo, hi));
                }
            }
        }
    }
    let mut set = ClassUnicode::new(ranges);
    if cc.is_not() {
        set.negate();
    }
    set
}

/// The characters a ctype node reads, where the seek can list them: any
/// character (but a newline without the multi-line option), and the ASCII
/// word characters in ASCII mode.
fn ctype_set(node: &Node, ct: &CtypeNode) -> Option<ClassUnicode> {
    if ct.ctype == CTYPE_ANYCHAR {
        let mut set = ClassUnicode::new([ClassUnicodeRange::new('\0', char::MAX)]);
        if !node.has_status(ND_ST_MULTILINE) {
            set.difference(&ClassUnicode::new([ClassUnicodeRange::new('\n', '\n')]));
        }
        return Some(set);
    }
    if ct.ctype == ONIGENC_CTYPE_WORD as i32 && ct.ascii_mode {
        let mut set = ClassUnicode::new(
            [('0', '9'), ('A', 'Z'), ('_', '_'), ('a', 'z')]
                .map(|(lo, hi)| ClassUnicodeRange::new(lo, hi)),
        );
        if ct.not {
            set.negate();
        }
        return Some(set);
    }
    None
}

/// The set of a look-around body that reads exactly one character: a class,
/// a one-character string, `.` or an ASCII `\w`, possibly in an option
/// group. Its folds would be alternatives, which no longer read one
/// character, so a body under `(?i)` that has any is not one.
fn one_char_set(node: &Node) -> Option<ClassUnicode> {
    match &node.inner {
        NodeInner::CClass(cc) => Some(class_set(cc)),
        NodeInner::String(sn)
            if !node.has_status(ND_ST_IGNORECASE) && !node.has_status(ND_ST_LITERAL_ALT) =>
        {
            let mut chars = std::str::from_utf8(&sn.s).ok()?.chars();
            let c = chars.next()?;
            chars
                .next()
                .is_none()
                .then(|| ClassUnicode::new([ClassUnicodeRange::new(c, c)]))
        }
        NodeInner::CType(ct) => ctype_set(node, ct),
        NodeInner::Bag(bn) if matches!(bn.bag_data, BagData::Option { .. }) => {
            one_char_set(bn.body.as_deref()?)
        }
        _ => None,
    }
}

/// The most strings a sequence of classes in a fold branch may stand for.
/// A case fold reads at most three characters, each with a few case
/// variants: `ffi` as `[fF][fF][iI]` stands for eight.
const MAX_FOLD_STRINGS: usize = 64;

/// The literal strings of a branch that is a string, an alternation of
/// strings, or a sequence of strings and small positive classes, or `None`
/// for another branch. The parser adds a class's multi-character case folds
/// beside it as such sequences, as C does (`ß` as `[sSſ][sSſ]`).
fn fold_strings(node: &Node) -> Option<Vec<Vec<u8>>> {
    fn literal(node: &Node) -> Option<&[u8]> {
        match &node.inner {
            NodeInner::String(sn)
                if !node.has_status(ND_ST_IGNORECASE) && !node.has_status(ND_ST_LITERAL_ALT) =>
            {
                Some(&sn.s[..])
            }
            _ => None,
        }
    }
    // The strings one item of a sequence reads.
    fn item_strings(node: &Node) -> Option<Vec<Vec<u8>>> {
        if let Some(text) = literal(node) {
            return Some(vec![text.to_vec()]);
        }
        let NodeInner::CClass(cc) = &node.inner else {
            return None;
        };
        if cc.is_not() {
            return None;
        }
        let mut strings = Vec::new();
        for (lo, hi) in class_members(cc) {
            for code in lo..=hi {
                if strings.len() == MAX_FOLD_STRINGS {
                    return None;
                }
                strings.push(char::from_u32(code)?.to_string().into_bytes());
            }
        }
        Some(strings)
    }
    match &node.inner {
        NodeInner::Alt(_) => cons_items(node)
            .into_iter()
            .map(|branch| literal(branch).map(<[u8]>::to_vec))
            .collect(),
        NodeInner::List(_) => {
            let mut strings = vec![Vec::new()];
            for item in cons_items(node) {
                let next = item_strings(item)?;
                if strings.len() * next.len() > MAX_FOLD_STRINGS {
                    return None;
                }
                strings = strings
                    .iter()
                    .flat_map(|head| next.iter().map(move |tail| [&head[..], tail].concat()))
                    .collect();
            }
            Some(strings)
        }
        _ => literal(node).map(|text| vec![text.to_vec()]),
    }
}

/// Whether the tree has an alternation that the walk would leave branches
/// of out: one led by a positive class, with a branch of fold strings.
fn has_fold_strings(node: &Node) -> bool {
    match &node.inner {
        NodeInner::Alt(_) => {
            let branches = cons_items(node);
            let class_led = matches!(&branches[0].inner, NodeInner::CClass(cc) if !cc.is_not());
            (class_led
                && branches[1..]
                    .iter()
                    .any(|branch| fold_strings(branch).is_some()))
                || branches.into_iter().any(has_fold_strings)
        }
        NodeInner::List(_) => cons_items(node).into_iter().any(has_fold_strings),
        NodeInner::Quant(QuantNode { body, .. })
        | NodeInner::Anchor(AnchorNode { body, .. })
        | NodeInner::Bag(BagNode { body, .. }) => body.as_deref().is_some_and(has_fold_strings),
        _ => false,
    }
}

/// What the walk left out so far.
enum LeftOut {
    Nothing,
    /// Non-empty strings, kept as their triggers.
    Strings(Box<FoldTriggers>),
    /// An empty string among them.
    Empty,
}

impl LeftOut {
    fn add(&mut self, string: &[u8]) {
        if string.is_empty() {
            *self = LeftOut::Empty;
            return;
        }
        match self {
            LeftOut::Nothing => {
                let mut triggers = Box::new(FoldTriggers::new());
                triggers.add(string);
                *self = LeftOut::Strings(triggers);
            }
            LeftOut::Strings(triggers) => triggers.add(string),
            LeftOut::Empty => {}
        }
    }
}

/// One walk over a tuned parse tree, writing the accept seek.
struct AcceptWalk<'a> {
    reg: &'a RegexType,
    /// The `\Z` nodes that end the pattern (`collect_tails`), by address.
    tails: Vec<*const Node>,
    /// The fold strings left out.
    left_out: LeftOut,
}

impl AcceptWalk<'_> {
    fn node(&mut self, node: &Node) -> Option<Part> {
        if node.has_status(ND_ST_LITERAL_ALT) {
            return self.trie(node);
        }
        match &node.inner {
            NodeInner::String(sn) => {
                // A case-insensitive string the tuner left whole.
                if node.has_status(ND_ST_IGNORECASE) {
                    return None;
                }
                if sn.s.is_empty() {
                    return Some(Part::Empty);
                }
                Some(Part::Text(std::str::from_utf8(&sn.s).ok()?.into()))
            }
            NodeInner::CClass(cc) => Some(Part::Char(class_set(cc))),
            NodeInner::CType(ct) => ctype_set(node, ct).map(Part::Char),
            NodeInner::BackRef(_) | NodeInner::Call(_) => None,
            NodeInner::Quant(qn) => {
                let Some(body) = qn.body.as_deref() else {
                    return Some(Part::Empty);
                };
                let sub = self.node(body)?;
                let min = u32::try_from(qn.lower).ok()?;
                let max = if qn.upper < 0 {
                    None
                } else {
                    Some(u32::try_from(qn.upper).ok()?)
                };
                Some(Part::Repeat {
                    min,
                    max,
                    sub: Box::new(sub),
                })
            }
            NodeInner::Bag(bn) => {
                let transparent = match bn.bag_data {
                    BagData::Memory { .. } | BagData::Option { .. } => true,
                    // `tune_next`'s possessive repetition matches what the
                    // plain one does: the body cannot start where the next
                    // item does, so giving an iteration back never helps.
                    BagData::StopBacktrack => {
                        node.has_status(ND_ST_STRICT_REAL_REPEAT)
                            && !node.has_status(
                                ND_ST_POSSESSIVE_CLASS_REPEAT | ND_ST_DECIMAL_TAIL_PREFIX,
                            )
                    }
                    // `\X` keeps only the first way its body matches, as an
                    // atomic group does.
                    BagData::IfElse { .. } | BagData::TextSegment => false,
                };
                if !transparent {
                    return None;
                }
                bn.body
                    .as_deref()
                    .map_or(Some(Part::Empty), |body| self.node(body))
            }
            NodeInner::Anchor(an) => self.anchor(node, an),
            NodeInner::List(_) => {
                let parts = cons_items(node)
                    .into_iter()
                    .map(|item| self.node(item))
                    .collect::<Option<Vec<_>>>()?;
                sequence(parts)
            }
            NodeInner::Alt(_) => {
                let branches = cons_items(node);
                let class_led = matches!(&branches[0].inner, NodeInner::CClass(cc) if !cc.is_not());
                let mut parts = Vec::with_capacity(branches.len());
                for (k, branch) in branches.into_iter().enumerate() {
                    if class_led && k > 0 {
                        if let Some(strings) = fold_strings(branch) {
                            for string in strings {
                                self.left_out.add(&string);
                            }
                            continue;
                        }
                    }
                    parts.push(self.node(branch)?);
                }
                Some(Part::Alt(parts))
            }
            // `\K` only moves the reported start.
            NodeInner::Gimmick(gn) => {
                let keep = match gn.gimmick_type {
                    GimmickType::Save => gn.detail_type == SaveType::Keep as i32,
                    GimmickType::UpdateVar => {
                        gn.detail_type == UpdateVarType::KeepFromStackLast as i32
                    }
                    _ => false,
                };
                keep.then_some(Part::Empty)
            }
        }
    }

    fn anchor(&mut self, node: &Node, an: &AnchorNode) -> Option<Part> {
        if self.tails.contains(&(node as *const Node)) {
            let newline = Part::Repeat {
                min: 0,
                max: Some(1),
                sub: Box::new(Part::Text("\n".into())),
            };
            return Some(Part::Concat(vec![newline, Part::Look(Look::End)]));
        }
        // A look-behind's `lead_node` is a copy of its body's last literal,
        // which the matcher checks first: it changes nothing the body reads.
        let look_body = || an.body.as_deref();
        Some(match an.anchor_type {
            ANCR_BEGIN_BUF => Part::Look(Look::Start),
            ANCR_END_BUF => Part::Look(Look::End),
            ANCR_END_LINE => Part::Look(Look::EndLF),
            ANCR_BEGIN_LINE => Part::Look(Look::StartLF),
            ANCR_PREC_READ_NOT => Part::NotNext(one_char_set(look_body()?)?),
            ANCR_LOOK_BEHIND_NOT => Part::NotPrev(one_char_set(look_body()?)?),
            _ => return None,
        })
    }

    /// A literal trie: its literals, where they are case-sensitive.
    fn trie(&mut self, node: &Node) -> Option<Part> {
        let trie = self.reg.literal_tries.get(literal_alt_trie_index(node)?)?;
        if trie.is_case_insensitive() {
            return None;
        }
        trie.literals()
            .iter()
            .map(|literal| {
                let text = std::str::from_utf8(literal).ok()?;
                Some(if text.is_empty() {
                    Part::Empty
                } else {
                    Part::Text(text.into())
                })
            })
            .collect::<Option<_>>()
            .map(Part::Alt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Regex, RegexBuilder, SearchOptions};
    use crate::encodings::utf8::ONIG_ENCODING_UTF8;
    use crate::oniguruma::{
        ONIG_OPTION_IGNORECASE, ONIG_OPTION_NONE, ONIG_OPTION_SINGLELINE, OnigOptionType,
    };
    use crate::regsyntax::OnigSyntaxOniguruma;

    /// The production catalog's `django_email_domain`.
    const DOMAIN: &str = r"^[a-z¡-￿0-9](?:[a-z¡-￿0-9-]{0,61}[a-z¡-￿0-9])?(?:\.(?!-)[a-z¡-￿0-9-]{1,63}(?<!-))*\.(?!-)(?:[a-z¡-￿-]{2,63}|xn--[a-z0-9]{1,59})(?<!-)\z";
    /// The production catalog's `django_url_regex`.
    const URL: &str = r"^(?:[a-z0-9.+-]*)://(?:[^\s:@/]+(?::[^\s:@/]*)?@)?(?:(?:0|25[0-5]|2[0-4][0-9]|1[0-9]?[0-9]?|[1-9][0-9]?)(?:\.(?:0|25[0-5]|2[0-4][0-9]|1[0-9]?[0-9]?|[1-9][0-9]?)){3}|\[[0-9a-f:.]+\]|([a-z¡-￿0-9](?:[a-z¡-￿0-9-]{0,61}[a-z¡-￿0-9])?(?:\.(?!-)[a-z¡-￿0-9-]{1,63}(?<!-))*\.(?!-)(?:[a-z¡-￿-]{2,63}|xn--[a-z0-9]{1,59})(?<!-)\.?|localhost))(?::[0-9]{1,5})?(?:[/?#][^\s]*)?\z";
    /// The catalog's flags for both: ignore case, `$` at the end only.
    const DJANGO: OnigOptionType = ONIG_OPTION_IGNORECASE.union(ONIG_OPTION_SINGLELINE);

    fn accept_with(pattern: &str, options: OnigOptionType) -> Option<AcceptCode> {
        crate::regcomp::onig_new_for_single_seek(
            pattern.as_bytes(),
            options,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
            false,
            true,
        )
        .unwrap()
        .2
    }

    /// The accept seek of `pattern` in regex syntax, or `None`.
    fn accept_pattern(pattern: &str, options: OnigOptionType) -> Option<String> {
        accept_with(pattern, options).map(|accept| accept.code.decode().to_string())
    }

    /// The django validators get an accept seek: the case folds of their
    /// classes are left out (the ASCII ones are the triggers), the
    /// look-arounds next to the label classes fold into them, and the seek
    /// fits the automaton's bounds.
    #[test]
    fn django_validators_have_accept_seeks() {
        let pairs = [
            "FF", "FI", "FL", "Ff", "Fi", "Fl", "SS", "ST", "Ss", "St", "fF", "fI", "fL", "ff",
            "fi", "fl", "sS", "sT", "ss", "st",
        ];
        for pattern in [DOMAIN, URL] {
            let accept = accept_with(pattern, DJANGO).expect(pattern);
            assert!(!accept.code.exact(), "{pattern}");
            let AcceptRejects::Unless(triggers) = &accept.rejects else {
                panic!("{:?}", accept.rejects);
            };
            assert_eq!(
                format!("{triggers:?}"),
                format!("FoldTriggers {{ pairs: {pairs:?}, bytes: 0x0 }}")
            );
            let re = RegexBuilder::new(pattern)
                .case_insensitive(true)
                .multi_line_anchors(false)
                .build()
                .unwrap();
            assert!(re.accept_seek_present(), "{pattern}");
        }
        // A label: `\.(?!-)C{1,63}(?<!-)` is `\.(?:D C{0,61} D|D)` with
        // `D` = `C` without `-`; the top-level domain's alternatives each
        // take the looks.
        let domain = accept_pattern(DOMAIN, DJANGO).unwrap();
        let c = "[0-9A-Za-z¡-\u{d7ff}\u{e000}-\u{ffff}\u{1df95}]";
        let d = "[\\-0-9A-Za-z¡-\u{d7ff}\u{e000}-\u{ffff}\u{1df95}]";
        assert!(
            domain.contains(&format!(r"(?:\.(?:(?:{c}{d}{{0,61}}{c})|{c}))*")),
            "{domain}"
        );
        let alnum = "[0-9A-Za-z\u{17f}\u{212a}]";
        assert!(
            domain.contains(&format!(r"(?:[Xx][Nn](?:\-\-){alnum}{{0,58}}{alnum})")),
            "{domain}"
        );
    }

    /// A class with multi-character folds, appended to a pattern so that its
    /// accept seek leaves something out (a pattern that leaves nothing out
    /// has none).
    const FOLD: &str = "(?i:[ß])?";

    /// The walk writes look-arounds, `tune_next`'s possessive repetitions,
    /// Unicode classes and case-sensitive tries exactly, and leaves a pattern
    /// without an accept seek where it cannot, or where it leaves nothing out.
    #[test]
    fn accept_seeks_are_absent_where_the_walk_says() {
        for pattern in [
            // `tune_next`'s `[0-9]+` alone is exact in the seek (ADR-008);
            // the Unicode class keeps this one inexact.
            "^[0-9]+:[a-zé-ü]+$",
            "[é-ü]+x",
            r"\.(?!-)[a-z-]{1,3}(?<!-)",
            "x(?!-)ab",
            "(?:alpha|beta|gamma|delta|epsilon|zeta|eta|theta)[é-ü]",
        ] {
            // Inexact seeks: nothing to leave out, so no accept seek.
            assert_eq!(accept_pattern(pattern, ONIG_OPTION_NONE), None, "{pattern}");
            let accept = accept_with(&format!("{pattern}{FOLD}"), ONIG_OPTION_NONE).expect(pattern);
            assert!(!accept.code.exact(), "{pattern}");
            assert!(
                matches!(accept.rejects, AcceptRejects::Unless(_)),
                "{pattern}"
            );
        }
        let folded = accept_pattern(
            &format!(r"\.(?!-)[a-z-]{{1,3}}(?<!-){FOLD}"),
            ONIG_OPTION_NONE,
        )
        .unwrap();
        assert!(
            folded.starts_with(r"(?:\.(?:(?:[a-z][\-a-z]?[a-z])|[a-z])"),
            "{folded}"
        );
        for (pattern, options) in [
            // User atomic groups and possessive quantifiers.
            ("(?>abc|ab)c", ONIG_OPTION_NONE),
            ("a++", ONIG_OPTION_NONE),
            // `\X`, compiled as one instruction outside look-behinds.
            (r"a\X", ONIG_OPTION_NONE),
            // Back references, calls and conditions.
            (r"(\w)\1", ONIG_OPTION_NONE),
            (r"(?<n>a|b\g<n>c)", ONIG_OPTION_NONE),
            ("(a)?(?(1)b|c)", ONIG_OPTION_NONE),
            // Look-arounds that do not fold: positive ones, a body of more
            // than a character, nothing that reads after or before it.
            ("a(?=b)", ONIG_OPTION_NONE),
            ("a(?!bc)c", ONIG_OPTION_NONE),
            ("(?<!ab)c", ONIG_OPTION_NONE),
            ("a(?!-)(?:b)?", ONIG_OPTION_NONE),
            ("(?<!-)a", ONIG_OPTION_NONE),
            ("(?!-)x?a", ONIG_OPTION_NONE),
            ("(?i)(?!ß)a", ONIG_OPTION_NONE),
            // Unicode word characters and boundaries, `\G`.
            (r"\w+", ONIG_OPTION_NONE),
            (r"\ba", ONIG_OPTION_NONE),
            (r"\Ga", ONIG_OPTION_NONE),
            // A case-insensitive trie, and an option that changes matches.
            (
                "(?:alpha|beta|gamma|delta|epsilon|zeta|eta|theta)",
                ONIG_OPTION_IGNORECASE,
            ),
            ("a", crate::oniguruma::ONIG_OPTION_NOTBOL),
        ] {
            let pattern = format!("{pattern}[é-ü]{FOLD}");
            assert_eq!(accept_pattern(&pattern, options), None, "{pattern}");
        }
        // An exact seek needs no accept seek.
        let (_, seek, accept) = crate::regcomp::onig_new_for_single_seek(
            b"^[a-z]{2,4}@x(?i:[\xc3\x9f])?$",
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
            false,
            true,
        )
        .unwrap();
        assert!(matches!(seek, Some(super::super::Seek::Pattern(code)) if code.exact()));
        assert_eq!(accept, None);
    }

    #[test]
    fn fold_triggers_find_the_left_out_strings() {
        let mut triggers = FoldTriggers::new();
        for string in ["ss", "ß", "x", "a\u{331}"] {
            triggers.add(string.as_bytes());
        }
        for text in ["ss", "class", "x", "é", "aß", "\u{80}"] {
            assert!(triggers.occur_in(text.as_bytes()), "{text}");
        }
        for text in ["", "s", "sa", "as.s", "abc-def.com"] {
            assert!(!triggers.occur_in(text.as_bytes()), "{text}");
        }
        let mut left_out = LeftOut::Nothing;
        left_out.add(b"ss");
        left_out.add(b"");
        left_out.add(b"ff");
        assert!(matches!(left_out, LeftOut::Empty));
    }

    /// Deterministic pseudo-random numbers (a linear congruential generator).
    struct Lcg(u64);

    impl Lcg {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) % n as u64) as usize
        }
    }

    /// Host names, URLs and other subjects for the accept seeks: labels of
    /// ASCII, of characters with multi-character folds and of the strings
    /// they fold to, hyphens at either end, labels around the 63-character
    /// bound, and random text.
    fn subjects() -> Vec<String> {
        const PIECES: &[&str] = &[
            "a", "b", "x", "n", "z", "0", "9", "-", "-", ".", ".", "s", "S", "f", "t", "ß", "ẞ",
            "ſ", "ﬀ", "ﬆ", "ss", "st", "ff", "é", "\u{212A}", "k", "xn--", "@", ":", "/", "\n",
        ];
        let mut rng = Lcg(0x5eed_0a11);
        let mut subjects: Vec<String> = [
            "",
            "a.com",
            "here.and.there.com",
            "straße.de",
            "strasse.de",
            "STRASSE.DE",
            "ﬀ.com",
            "ff.com",
            "a.ﬀ",
            "a.ff",
            "a.ﬆ",
            "a.st",
            "xn--bcher-kva.ch",
            "xn--.ch",
            "a.xn--",
            "a.xn--ß",
            "-a.com",
            "a-.com",
            "a.-b.com",
            "a.b-.com",
            "a.c-",
            "a.-c",
            "a..com",
            "a.c",
            "ß",
            "ss",
            "SS",
            "ſſ",
            "a.ßß",
            "a.ſt",
            "x.\u{212A}k",
            "http://www.djangoproject.com/",
            "HTTP://WWW.DJANGOPROJECT.COM/",
            "http://localhost/",
            "http://LOCALHOST:8000/x",
            "http://localhoſt/",
            "http://locaﬅ/",
            "http://127.0.0.1:80/",
            "http://[::1]/",
            "ftp://u:p@host.straße/",
            "http://-a.com",
            "http://a-.com",
            "http://ﬀ.de/",
            "http://a.b.c.d.e.example/?q#f",
            "http://a b.com",
            "http://a.com\n",
            "file:///etc",
            "://a.co",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for n in [60, 61, 62, 63, 64] {
            let label = "a".repeat(n);
            subjects.push(format!("{label}.com"));
            subjects.push(format!("x.{label}"));
            subjects.push(format!("x.{label}.com"));
            subjects.push(format!("x.ß{}", "a".repeat(n - 1)));
            subjects.push(format!("x.ss{}", "a".repeat(n - 2)));
            subjects.push(format!("http://x.{label}/"));
        }
        for _ in 0..600 {
            let mut text = String::new();
            if rng.below(3) == 0 {
                text.push_str("http://");
            }
            for _ in 0..rng.below(16) {
                text.push_str(PIECES[rng.below(PIECES.len())]);
            }
            subjects.push(text);
        }
        subjects
    }

    /// The accept seek against the matcher (`dfa(false)`, no retry limit):
    /// where it answers "match", the matcher finds one, and where it answers
    /// "no match" (on a text without its triggers), the matcher finds none.
    /// The standard `is_match` agrees with the matcher too, on the patterns
    /// with an accept seek and on those the walk leaves without one.
    #[test]
    fn accept_answers_are_the_matchers() {
        // `is_match_at` reads the process-wide retry limits.
        let _limits = crate::regexec::shared_limits();
        // Pattern, the django flags (ignore case, `$` at the end only) or
        // the defaults, and whether the pattern has an accept seek.
        let patterns: &[(&str, bool, bool)] = &[
            (DOMAIN, true, true),
            (URL, true, true),
            (DOMAIN, false, true),
            (URL, false, true),
            (r"(?i)^[a-zßà-ÿ]+$", false, true),
            (r"(?i)^[ßà-ÿ]{1,2}$", false, true),
            (r"(?i)\A[a-z¡-￿-]{2,5}(?<!-)\z", false, true),
            (r"(?i)(?!-)[^.]{1,3}(?<!-)\.", false, true),
            (r"^(?!-)[a-z-]+(?<!-)$", false, true),
            (r"\.(?!-)[a-z0-9-]{1,3}(?<!-)", false, true),
            (r"(?!a)(?:ab|cd|e?f)x", false, true),
            (r"x(?:ab|c*d)(?<!d)", false, true),
            (r"(?:-|a)(?<!-)b", false, true),
            (r"^[0-9]+:[a-zé-ü]+$", false, true),
            // Exact seek: answered without an accept seek.
            (r"^[0-9]+:[a-z]+$", false, false),
            (r"(?i)straße[é-ü]", false, true),
            (r"(?i)[ßà-ÿ]+x", false, true),
            (r"(?i)\Ak[a-zà-ÿ]*\z", false, true),
            (r"[α-ω]+\d", false, true),
            (r"[^\s:@/]+@[é-ü]+", false, true),
            (r".+x[é-ü]", false, true),
            (r"(?m).+\n[é-ü]", false, true),
            (r"^x[é-ü]$", false, true),
            (r"\Aab[é-ü]|cd\z", false, true),
            (r"ab[é-ü]\Z", false, true),
            (r"\Kfoo|bar[é-ü]", false, true),
            (
                "(?:alpha|beta|gamma|delta|epsilon|zeta|eta|theta)[é-ü]",
                false,
                true,
            ),
            (
                r"(?i)\.(?!-)(?:[a-zß-]{2,4}|xn--[a-z]{1,3})(?<!-)\.?\z",
                false,
                true,
            ),
            // Constructs the walk does not write: the matcher answers.
            ("(?>abc|ab)c[é-ü]", false, false),
            ("x[a-z]++y[é-ü]", false, false),
            ("a(?=b)[é-ü]", false, false),
            ("(?<!ab)c[é-ü]", false, false),
            (r"\w+-[é-ü]", false, false),
        ];
        let unlimited = SearchOptions::new().retry_limit_in_match(0);
        let mut subjects = subjects();
        subjects.extend(
            [
                "abcé", "abcc", "xaby", "xayé", "xaay", "abé", "acé", "abcé", "w-é", "dé-é",
            ]
            .map(String::from),
        );
        let (mut accepted, mut rejected) = (0, 0);
        for &(pattern, django, has_accept) in patterns {
            // Under the default flags `FOLD` leads the pattern, so that a
            // pattern without folds of its own gets an accept seek.
            let pattern = &if django {
                pattern.to_string()
            } else {
                format!("{FOLD}{pattern}")
            };
            let build = |dfa: bool| {
                RegexBuilder::new(pattern)
                    .case_insensitive(django)
                    .multi_line_anchors(!django)
                    .dfa(dfa)
                    .build()
                    .unwrap()
            };
            let (re, matcher): (Regex, Regex) = (build(true), build(false));
            assert_eq!(re.accept_seek_present(), has_accept, "{pattern}");
            for text in &subjects {
                for start in (0..=text.len()).filter(|&start| text.is_char_boundary(start)) {
                    let Ok(expected) = matcher.is_match_at_with(text, start, unlimited) else {
                        continue;
                    };
                    assert_eq!(
                        re.is_match_at(text, start),
                        expected,
                        "{pattern:?} on {text:?} from {start}"
                    );
                    let Some(answer) = re.accept_answer(text, start) else {
                        continue;
                    };
                    assert_eq!(answer, expected, "{pattern:?} on {text:?} from {start}");
                    if answer {
                        accepted += 1;
                    } else {
                        rejected += 1;
                    }
                }
            }
        }
        assert!(
            accepted > 2_000 && rejected > 20_000,
            "{accepted} {rejected}"
        );
    }

    /// The django validators' accept seeks stay well within their bound, and
    /// the automata with this thread's caches within a few MiB after long
    /// searches.
    #[test]
    fn django_accept_seeks_are_bounded() {
        for pattern in [DOMAIN, URL] {
            let accept = accept_with(pattern, DJANGO).unwrap();
            assert!(states_estimate(&accept.code.decode(), true) < MAX_ACCEPT_NFA_STATES * 3 / 4);
            let re = RegexBuilder::new(pattern)
                .case_insensitive(true)
                .multi_line_anchors(false)
                .build()
                .unwrap();
            let long = format!("http://{}example.com/", "a1-b.".repeat(300));
            for text in ["example.com", "http://example.com/", &long] {
                let _ = re.is_match(text);
            }
            assert!(
                re.seek_memory_usage() < 4 * super::super::MIB,
                "{}",
                re.seek_memory_usage()
            );
        }
    }
}
