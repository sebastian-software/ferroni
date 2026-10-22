//! Rust-only (ADR-008): a multi-pattern DFA pre-filter for the scanner's
//! RegSet search, after fancy-regex's `RegexSet`.
//!
//! A TextMate scanner asks its set for the earliest match of any pattern,
//! and most attempts the position-lead search makes fail. The pre-filter
//! decides, before any attempt, which entries can match where.
//!
//! [`derive`] reads a pattern's tuned parse tree and writes an
//! over-approximation of it as regex-automata's HIR, its *seek*: wherever
//! the pattern can match, the seek matches too (it may match in more
//! places, never fewer). Literals, classes, concatenation, alternation and
//! repetition are exact; groups are transparent; look-behind, negative
//! look-ahead, `\G`, `\K` and text-segment boundaries become empty, except
//! that a negative look at a class holding every ASCII word character
//! becomes the ASCII half boundary it is contained in; back references and
//! calls become `(?s:.)*`; a positive look-ahead keeps its body where
//! nothing consuming follows it and is dropped otherwise. Classes are
//! byte-based: the ASCII members are exact and any non-ASCII member stands
//! for one UTF-8 sequence, which keeps the automata small; any character is
//! an ASCII byte or such a sequence. A seek that matches at every position
//! narrows nothing; its entry is searched on its own (`Seek::Everywhere`).
//! The regex does not carry its seek: the scanner hands it to its set, and
//! a pattern cache keeps one per distinct pattern, in a compact encoding
//! ([`SeekCode`]) that is decoded when the automata are built.
//!
//! [`Automata`] compiles the seeks of one pattern list into a forward and a
//! reverse lazy DFA, which find the earliest position where any seek matches
//! (the strategy of regex-automata's meta regex), and a lazy DFA over all
//! seeks, which says which of them match at that position. They are built on
//! a set's first pre-filtered
//! search, not when the set is constructed, and every set built from one
//! pattern cache over the same pattern list shares them ([`AutomataCell`]);
//! [`SetPrefilter`] is a set's own part: the lazy DFA caches and the
//! search's scratch state. The RegSet (`regset_search_body_prefilter`)
//! attempts only the entries the automata name, anchored at that position,
//! in index order; the first event decides, and without one the search
//! goes on one character later. A pattern list whose automata would exceed
//! [`MAX_NFA_STATES`] builds none and its sets keep the position-lead
//! search as it was.
//!
//! The `dfa-prefilter` feature compiles this module, and
//! `ScannerConfig::prefilter` switches it per scanner. A search under a
//! limit of the caller's own keeps C's attempts (see `prefilter_decides` in
//! `regset.rs`).

mod accept;

pub(crate) use accept::{AcceptCode, derive_accept};

use crate::oniguruma::{
    ONIG_OPTION_FIND_NOT_EMPTY, ONIG_OPTION_NOTBOL, ONIG_OPTION_NOTEOL, ONIGENC_CTYPE_WORD,
};
use crate::regcomp::literal_alt_trie_index;
use crate::regenc::OnigEncoding;
use crate::regint::*;
use crate::regparse_types::*;
use regex_automata::hybrid::LazyStateID;
use regex_automata::hybrid::dfa as hybrid;
use regex_automata::hybrid::regex as hybrid_regex;
use regex_automata::nfa::thompson;
use regex_automata::util::pool::Pool;

use regex_automata::{Anchored, Input, MatchKind, PatternID};
use regex_syntax::hir::{
    Class, ClassBytes, ClassBytesRange, ClassUnicode, ClassUnicodeRange, Hir, HirKind, Look,
    Repetition,
};
use regex_syntax::utf8::Utf8Sequences;
use std::sync::{Arc, OnceLock};

const MIB: usize = 1 << 20;
/// Lazy DFA cache of the search DFAs (fancy-regex: 64 MiB).
const SEARCH_CACHE_CAPACITY: usize = 2 * MIB;
/// The search DFAs' efficiency check: regex-automata's meta regex gives its
/// own lazy DFAs the same clear count (`MIN_CACHE_CLEARS`) and this.
const SEARCH_MIN_BYTES_PER_STATE: usize = 10;
/// Lazy DFA cache of the overlapping DFA (fancy-regex: 64 MiB).
const OVERLAPPING_CACHE_CAPACITY: usize = MIB;
/// Hard bound on the NFA size per set, for both automata.
const NFA_SIZE_LIMIT: usize = 16 * MIB;
/// The overlapping DFA gives up once its cache has been cleared this many
/// times and it searched fewer than `MIN_BYTES_PER_STATE` bytes per state
/// it holds: it is creating states faster than it can reuse them (every
/// state of a set over a hundred case-insensitive keyword lists holds
/// thousands of NFA states, so a 1 MiB cache holds a few hundred of them,
/// and a walk at every position of a long identifier rebuilt them all
/// every few searches: 100 µs a search against 0.4 without the pre-filter
/// on the flat CSS grammar set). The set then drops its pre-filter for
/// good (`retire_prefilter` in `regset.rs`). regex-automata's meta regex
/// gives its own lazy DFAs the same clear count.
const MIN_CACHE_CLEARS: usize = 3;
/// See `MIN_CACHE_CLEARS`.
const MIN_BYTES_PER_STATE: usize = 20;
/// Counted repetitions above this bound become unbounded ones.
const MAX_COUNTED_REPEAT: u32 = 64;
/// Bytes the anchored candidate scan reads past a position before it
/// declares every covered entry a candidate there. A seek such as `\w+`
/// stays alive to the end of a word run, and the scan would otherwise read
/// it again from every position of the run.
pub(crate) const CANDIDATE_SCAN_BYTES: usize = 256;

/// A seek whose matches are at most this long settles within that many
/// bytes of a candidate walk: it has matched or its states are dead. The
/// longer and the unbounded seeks (`\w+:`) are the ones that can keep a
/// walk alive past that, and a walk asks which of their entries the
/// search admits at the position: an entry the search would not attempt
/// never decides there, so a walk that has recorded every admissible one
/// stops. Asking costs about as much as a long walk, so a walk asks right
/// after the bounded seeks settled only while the set's walks keep
/// running to the bound or ending by such an ask (`ask_early`: every
/// position of a word run a seek like `\w+:` sits in while its entry is
/// ruled out), and otherwise only after `LONG_WALK_BYTES` (see ADR-008).
pub(crate) const LONG_SEEK_BYTES: usize = 16;

/// Bytes after which a walk asks about the long seeks in any case.
pub(crate) const LONG_WALK_BYTES: usize = 64;

/// Sets whose NFA has more states than this get no pre-filter: a bound on
/// what one set costs to build and keep, not a break-even. The largest set
/// of the captured replays (PHP, 37,437 states, 4.2 MiB of automata, built
/// in about 60 ms) takes 2% of its time with the pre-filter, and size does
/// not tell which sets lose: C++ sets of 25,000 states take a quarter of
/// their time, the hot SCSS set at 17,000 states 13% more (see ADR-008).
pub(crate) const MAX_NFA_STATES: usize = 65_536;

/// Sets whose automata would cover at most this many entries with at most
/// `TINY_SET_NFA_STATES` NFA states get no pre-filter unless the scanner
/// asks for it from the first search (`ScannerConfig::prefilter_warmup`
/// of 0): such patterns are literals and small classes, whose attempts
/// the first-byte table dispatches and the first-instruction tests reject
/// as cheaply as a walk of the automata would, so the pre-filter has
/// nothing to save there and costs its walks and search DFA searches on
/// every call (the six sets of the JSON grammar, 3 to 11 patterns over 24
/// to 104 states, took 1.5 to 2.5 times their time with it, and no set of
/// the captured replays within these bounds gains; see ADR-008).
pub(crate) const TINY_SET_ENTRIES: usize = 12;
/// See `TINY_SET_ENTRIES`.
pub(crate) const TINY_SET_NFA_STATES: usize = 128;

#[cfg(test)]
thread_local! {
    /// Searches of a set's search DFAs on this thread, for tests that bound
    /// the pre-filter's work (`search_in`).
    pub(crate) static EARLIEST_CALLS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Bytes the candidate scans read on this thread.
    pub(crate) static SCAN_STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Bytes the search DFAs read on this thread.
    pub(crate) static META_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// The overlapping DFA's cache capacity for the automata built on this
    /// thread, when a test sets one (`overlapping_config`).
    pub(crate) static OVERLAPPING_CACHE_BYTES: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

/// The configuration of the overlapping DFA. A test sets a cache capacity
/// for its thread (`OVERLAPPING_CACHE_BYTES`): the lazy DFA then clears its
/// cache as often as a walk needs, without the give-up that would retire the
/// pre-filter before the walk reached the case under test.
fn overlapping_config() -> hybrid::Config {
    let config = hybrid::Config::new()
        .match_kind(MatchKind::All)
        .skip_cache_capacity_check(true);
    #[cfg(test)]
    if let Some(bytes) = OVERLAPPING_CACHE_BYTES.with(|bytes| bytes.get()) {
        return config
            .cache_capacity(bytes)
            .minimum_cache_clear_count(None)
            .minimum_bytes_per_state(None);
    }
    config
        .cache_capacity(OVERLAPPING_CACHE_CAPACITY)
        .minimum_cache_clear_count(Some(MIN_CACHE_CLEARS))
        .minimum_bytes_per_state(Some(MIN_BYTES_PER_STATE))
}

/// The seek approximation of one compiled pattern. A scanner passes it to
/// its set, which builds the automata from it, and its pattern cache keeps
/// it per distinct pattern (ADR-006); the regex itself does not carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Seek {
    /// The approximation matches at every position, so it cannot narrow
    /// anything: the entry is searched on its own.
    Everywhere,
    /// The approximation, for the automata.
    Pattern(SeekCode),
}

/// The seek approximation of `reg`, whose tuned parse tree is `root`.
/// `None` for an entry the pre-filter must not decide for: callouts observe
/// every attempt, and the seek reads UTF-8.
pub(crate) fn derive(root: &Node, reg: &RegexType, precise: bool) -> Option<Seek> {
    if reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0) {
        return None;
    }
    // Trait-object pointers may differ in their vtable between codegen
    // units, so the encoding is identified by name.
    let utf8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
    if reg.enc.name() != utf8.name() || reg.enc.max_enc_len() != utf8.max_enc_len() {
        return None;
    }
    walk_seek(root, reg, precise)
}

/// The walk of `derive`. A precise seek whose automaton would be too large
/// falls back to the scanner's seek, which is smaller.
fn walk_seek(root: &Node, reg: &RegexType, precise: bool) -> Option<Seek> {
    // The tail rewrite of `\Z` is exact, and the scanner's seeks keep their
    // approximation of it (`$`), so only a single regex's seek takes it.
    let mut tails = Vec::new();
    if precise {
        collect_tails(root, &mut tails);
    }
    let mut walk = Walk {
        reg,
        lookaheads_kept: 0,
        inexact: false,
        tails,
        precise,
    };
    let hir = walk.item(root, true);
    if matches_everywhere(&hir) {
        return Some(Seek::Everywhere);
    }
    if precise && nfa_states_estimate(&hir) > MAX_NFA_STATES {
        return walk_seek(root, reg, false);
    }
    // Exact when the seek matches exactly where the pattern does, so that a
    // match of the seek is a match of the pattern (`SeekCode::exact`). The
    // line anchors and the options that change what a line start or end
    // is, or what counts as a match, make the seek inexact.
    let exact = !walk.inexact
        && line_starts_exact(&hir)
        && !reg
            .options
            .intersects(ONIG_OPTION_NOTBOL | ONIG_OPTION_NOTEOL | ONIG_OPTION_FIND_NOT_EMPTY);
    // A node the encoding has no tag for leaves the entry on its own, which
    // is always right; the walk writes none.
    let code = SeekCode::encode(&hir, exact)?;
    debug_assert_eq!(code.decode(), hir, "the seek code round-trips");
    Some(Seek::Pattern(code))
}

/// A seek's HIR in a compact encoding, kept per distinct pattern until the
/// automata are built: a flag byte (whether the seek is exact, see
/// [`SeekCode::exact`]), then the tree in pre-order, each node a tag byte and
/// its data. An HIR node carries a boxed property block and its own
/// allocations, some 20–30 KiB per seek over the captured grammars; the
/// code is a few hundred bytes. Decoding rebuilds the tree through the
/// smart constructors that built it (`Hir::concat`, `Hir::alternation`,
/// `Hir::class`, ...), whose simplifications are stable on their own
/// output, so the decoded HIR equals the original (`derive` asserts it in
/// debug builds, and a test checks it over every seek of the captured
/// grammars).
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SeekCode(Box<[u8]>);

/// The node tags of a `SeekCode`.
mod tag {
    pub(super) const EMPTY: u8 = 0;
    /// Length, then the bytes.
    pub(super) const LITERAL: u8 = 1;
    /// Range count, then each range as two bytes.
    pub(super) const BYTES: u8 = 2;
    /// Range count, then each range as two code points.
    pub(super) const UNICODE: u8 = 3;
    /// `Look::as_repr`.
    pub(super) const LOOK: u8 = 4;
    /// Minimum, maximum plus one (0 for unbounded), greediness, the body.
    pub(super) const REPETITION: u8 = 5;
    /// Count, then the parts.
    pub(super) const CONCAT: u8 = 6;
    /// Count, then the branches.
    pub(super) const ALTERNATION: u8 = 7;
}

impl SeekCode {
    #[cfg(feature = "precompiled")]
    pub(crate) fn precompiled_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Check the complete seek before the infallible internal decoder sees it.
    /// Depth and node budgets also bound the HIR constructors and destruction.
    #[cfg(feature = "precompiled")]
    pub(crate) fn from_precompiled(bytes: Vec<u8>) -> Option<Self> {
        struct Check<'a> {
            bytes: &'a [u8],
            nodes: usize,
        }
        impl Check<'_> {
            fn byte(&mut self) -> Option<u8> {
                let (&first, rest) = self.bytes.split_first()?;
                self.bytes = rest;
                Some(first)
            }
            fn number(&mut self) -> Option<usize> {
                let mut value = 0u64;
                for shift in (0..35).step_by(7) {
                    let b = self.byte()?;
                    value |= u64::from(b & 0x7f) << shift;
                    if b & 0x80 == 0 {
                        return usize::try_from(value)
                            .ok()
                            .filter(|v| *v <= u32::MAX as usize);
                    }
                }
                None
            }
            fn node(&mut self, depth: usize) -> Option<()> {
                self.nodes = self.nodes.checked_sub(1)?;
                if depth > 256 {
                    return None;
                }
                match self.byte()? {
                    tag::EMPTY => {}
                    tag::LITERAL => {
                        let len = self.number()?;
                        self.bytes = self.bytes.get(len..)?;
                    }
                    kind @ (tag::BYTES | tag::UNICODE) => {
                        let count = self.number()?;
                        if count > self.bytes.len() / 2 {
                            return None;
                        }
                        let mut previous = None;
                        for _ in 0..count {
                            let (lo, hi) = if kind == tag::BYTES {
                                (u32::from(self.byte()?), u32::from(self.byte()?))
                            } else {
                                let lo = self.number()? as u32;
                                let hi = self.number()? as u32;
                                char::from_u32(lo)?;
                                char::from_u32(hi)?;
                                (lo, hi)
                            };
                            if lo > hi || previous.is_some_and(|p| p >= lo) {
                                return None;
                            }
                            previous = Some(hi);
                        }
                    }
                    tag::LOOK => {
                        Look::from_repr(self.number()? as u32)?;
                    }
                    tag::REPETITION => {
                        let min = self.number()?;
                        let max = self.number()?;
                        if (max != 0 && max - 1 < min) || self.byte()? > 1 {
                            return None;
                        }
                        self.node(depth + 1)?;
                    }
                    tag::CONCAT | tag::ALTERNATION => {
                        let count = self.number()?;
                        if count > self.nodes {
                            return None;
                        }
                        for _ in 0..count {
                            self.node(depth + 1)?;
                        }
                    }
                    _ => return None,
                }
                Some(())
            }
        }
        let mut check = Check {
            bytes: &bytes,
            nodes: 65_536,
        };
        if check.byte()? > 1 {
            return None;
        }
        check.node(0)?;
        if !check.bytes.is_empty() {
            return None;
        }
        Some(Self(bytes.into_boxed_slice()))
    }

    /// `hir` encoded, or `None` for a node kind the walk never writes
    /// (a capture).
    fn encode(hir: &Hir, exact: bool) -> Option<SeekCode> {
        let mut out = vec![u8::from(exact)];
        Self::write(hir, &mut out)?;
        Some(SeekCode(out.into_boxed_slice()))
    }

    /// Whether the seek matches exactly where the pattern does, for the
    /// purposes of existence: no construct was approximated, no line start
    /// the matcher would not see is admitted, and no option changes a match
    /// the seek describes. A match of an exact seek is a match of the
    /// pattern, which is what a standard `is_match` may answer from it. The
    /// matcher finds that match with no limit; under a limit it may stop
    /// instead, and the answer is then the one without the limit (ADR-017).
    pub(crate) fn exact(&self) -> bool {
        self.0[0] != 0
    }

    fn write(hir: &Hir, out: &mut Vec<u8>) -> Option<()> {
        match hir.kind() {
            HirKind::Empty => out.push(tag::EMPTY),
            HirKind::Literal(literal) => {
                out.push(tag::LITERAL);
                Self::varint(literal.0.len(), out);
                out.extend_from_slice(&literal.0);
            }
            HirKind::Class(Class::Bytes(class)) => {
                out.push(tag::BYTES);
                Self::varint(class.ranges().len(), out);
                for range in class.ranges() {
                    out.push(range.start());
                    out.push(range.end());
                }
            }
            HirKind::Class(Class::Unicode(class)) => {
                out.push(tag::UNICODE);
                Self::varint(class.ranges().len(), out);
                for range in class.ranges() {
                    Self::varint(range.start() as usize, out);
                    Self::varint(range.end() as usize, out);
                }
            }
            HirKind::Look(look) => {
                out.push(tag::LOOK);
                Self::varint(look.as_repr() as usize, out);
            }
            HirKind::Repetition(rep) => {
                out.push(tag::REPETITION);
                Self::varint(rep.min as usize, out);
                Self::varint(rep.max.map_or(0, |max| max as usize + 1), out);
                out.push(u8::from(rep.greedy));
                Self::write(&rep.sub, out)?;
            }
            HirKind::Concat(parts) => {
                out.push(tag::CONCAT);
                Self::varint(parts.len(), out);
                for part in parts {
                    Self::write(part, out)?;
                }
            }
            HirKind::Alternation(branches) => {
                out.push(tag::ALTERNATION);
                Self::varint(branches.len(), out);
                for branch in branches {
                    Self::write(branch, out)?;
                }
            }
            HirKind::Capture(_) => return None,
        }
        Some(())
    }

    /// LEB128.
    fn varint(mut value: usize, out: &mut Vec<u8>) {
        while value >= 0x80 {
            out.push((value as u8) | 0x80);
            value >>= 7;
        }
        out.push(value as u8);
    }

    /// The HIR the code was made from.
    pub(crate) fn decode(&self) -> Hir {
        let mut reader = SeekReader {
            code: &self.0,
            at: 1,
        };
        let hir = reader.hir();
        debug_assert_eq!(reader.at, self.0.len(), "the seek code is read whole");
        hir
    }

    /// Bytes of the code.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// A cursor over a `SeekCode`, decoding it.
struct SeekReader<'a> {
    code: &'a [u8],
    at: usize,
}

impl SeekReader<'_> {
    fn byte(&mut self) -> u8 {
        let b = self.code[self.at];
        self.at += 1;
        b
    }

    fn varint(&mut self) -> usize {
        let mut value = 0usize;
        let mut shift = 0;
        loop {
            let b = self.byte();
            value |= usize::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    }

    fn hir(&mut self) -> Hir {
        match self.byte() {
            tag::EMPTY => Hir::empty(),
            tag::LITERAL => {
                let len = self.varint();
                let bytes = &self.code[self.at..self.at + len];
                self.at += len;
                Hir::literal(bytes)
            }
            tag::BYTES => {
                let count = self.varint();
                let ranges: Vec<ClassBytesRange> = (0..count)
                    .map(|_| {
                        let lo = self.byte();
                        let hi = self.byte();
                        ClassBytesRange::new(lo, hi)
                    })
                    .collect();
                Hir::class(Class::Bytes(ClassBytes::new(ranges)))
            }
            tag::UNICODE => {
                let count = self.varint();
                let ranges: Vec<ClassUnicodeRange> = (0..count)
                    .map(|_| {
                        let lo = char::from_u32(self.varint() as u32).expect("a code point");
                        let hi = char::from_u32(self.varint() as u32).expect("a code point");
                        ClassUnicodeRange::new(lo, hi)
                    })
                    .collect();
                Hir::class(Class::Unicode(ClassUnicode::new(ranges)))
            }
            tag::LOOK => Hir::look(Look::from_repr(self.varint() as u32).expect("a look")),
            tag::REPETITION => {
                let min = self.varint() as u32;
                let max = match self.varint() {
                    0 => None,
                    max => Some(max as u32 - 1),
                };
                let greedy = self.byte() != 0;
                let sub = Box::new(self.hir());
                Hir::repetition(Repetition {
                    min,
                    max,
                    greedy,
                    sub,
                })
            }
            tag::CONCAT => {
                let count = self.varint();
                Hir::concat((0..count).map(|_| self.hir()).collect())
            }
            tag::ALTERNATION => {
                let count = self.varint();
                Hir::alternation((0..count).map(|_| self.hir()).collect())
            }
            _ => unreachable!("a seek code tag"),
        }
    }
}

/// The decoded seek in regex syntax.
impl std::fmt::Debug for SeekCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SeekCode")
            .field(&self.decode().to_string())
            .finish()
    }
}

/// Any character, newline included: an ASCII byte or one UTF-8 sequence.
/// One byte would not do: `.x` matches `éx` at the `é`, two bytes before
/// the `x`.
fn any_char() -> Hir {
    byte_class(u128::MAX, true)
}

/// Any character but a newline.
fn any_char_except_newline() -> Hir {
    byte_class(!(1u128 << b'\n'), true)
}

/// `(?s:.)*`, for a node the seek cannot express.
fn placeholder() -> Hir {
    Hir::repetition(Repetition {
        min: 0,
        max: None,
        greedy: true,
        sub: Box::new(any_char()),
    })
}

fn is_placeholder(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Repetition(rep) => rep.min == 0 && rep.max.is_none() && *rep.sub == any_char(),
        _ => false,
    }
}

/// Any non-ASCII character as its UTF-8 bytes: a lead byte and one to
/// three continuation bytes, a superset of every valid sequence.
fn non_ascii_char() -> Hir {
    Hir::concat(vec![
        Hir::class(Class::Bytes(ClassBytes::new([ClassBytesRange::new(
            0xC2, 0xF4,
        )]))),
        Hir::repetition(Repetition {
            min: 1,
            max: Some(3),
            greedy: true,
            sub: Box::new(Hir::class(Class::Bytes(ClassBytes::new([
                ClassBytesRange::new(0x80, 0xBF),
            ])))),
        }),
    ])
}

/// Whether the HIR matches at every position: it is nullable without any
/// look assertion, or it starts with `(?s:.)*`.
fn matches_everywhere(hir: &Hir) -> bool {
    let properties = hir.properties();
    if properties.minimum_len() == Some(0) && properties.look_set().is_empty() {
        return true;
    }
    let first = match hir.kind() {
        HirKind::Concat(subs) => subs.first(),
        _ => Some(hir),
    };
    first.is_some_and(is_placeholder)
}

/// Whether a class over these code point ranges is exact in the seek: its
/// non-ASCII members are none, or every non-ASCII character, since `class_hir`
/// writes them as one UTF-8 sequence, which is exact only then. A negation
/// takes the complement of those, so the same condition covers it.
fn class_exact(ranges: &[(u32, u32)]) -> bool {
    ranges
        .iter()
        .all(|&(lo, hi)| lo > hi || hi < 0x80 || (lo.max(0x80) == 0x80 && hi >= 0x10FFFF))
}

/// The most non-ASCII members a class may list one by one (`explicit_class`).
const EXPLICIT_MEMBERS: usize = 16;

/// A positive class whose non-ASCII members are few, written exactly: its
/// ASCII members as bytes and each non-ASCII member as its UTF-8 sequence. A
/// case-insensitive class holds such members (the Kelvin sign for `k`, the
/// long s for `s`), and listing them keeps the class exact where
/// `class_hir` would approximate them. `None` when there are more than
/// `EXPLICIT_MEMBERS`, which `class_hir` writes instead.
fn explicit_class(ranges: &[(u32, u32)]) -> Option<Hir> {
    let mut ascii: u128 = 0;
    let mut members = Vec::new();
    for &(lo, hi) in ranges {
        if lo > hi {
            continue;
        }
        if lo <= 0x7F {
            ascii |= ascii_mask(lo as u8, hi.min(0x7F) as u8);
        }
        let lo = lo.max(0x80);
        if hi < lo {
            continue;
        }
        let count = (hi - lo) as usize + 1;
        if members.len() + count > EXPLICIT_MEMBERS {
            return None;
        }
        for code in lo..=hi {
            // A surrogate is no character, and the matcher never reads one.
            if let Some(c) = char::from_u32(code) {
                members.push(Hir::literal(c.to_string().into_bytes().into_boxed_slice()));
            }
        }
    }
    members.insert(0, byte_class(ascii, false));
    Some(alternation(members))
}

/// Whether every `^` (`Look::StartLF`) in the seek is one the matcher reads
/// alike. The matcher holds `^` at the start of the subject and after a
/// newline that is not the last byte, the seek after any newline; the two
/// differ only where nothing follows the `^`. So a `^` must be followed by
/// something that consumes a byte, in its own sequence or branch.
fn line_starts_exact(hir: &Hir) -> bool {
    if !hir.properties().look_set().contains(Look::StartLF) {
        return true;
    }
    match hir.kind() {
        HirKind::Concat(parts) => parts.iter().enumerate().all(|(k, part)| {
            if matches!(part.kind(), HirKind::Look(Look::StartLF)) {
                parts[k + 1..]
                    .iter()
                    .map(|rest| rest.properties().minimum_len().unwrap_or(0))
                    .sum::<usize>()
                    >= 1
            } else {
                line_starts_exact(part)
            }
        }),
        HirKind::Alternation(branches) => branches.iter().all(line_starts_exact),
        // A `^` under a repetition, a capture or alone is not judged here.
        _ => false,
    }
}

/// A class over code point ranges (inclusive), negated or not: the ASCII
/// members exactly, and one UTF-8 sequence for any non-ASCII member.
///
/// One pass over the ranges fills an ASCII bitset and notes whether the
/// class reaches past ASCII. A negated class reaches past ASCII unless one
/// range held every code point from 0x80 up; a negated class that covers
/// more of them than that keeps the sequence too, which admits more.
fn class_hir(ranges: impl IntoIterator<Item = (u32, u32)>, negate: bool) -> Hir {
    let mut ascii: u128 = 0;
    let mut non_ascii = false;
    let mut all_non_ascii = false;
    for (lo, hi) in ranges {
        if lo > hi {
            continue;
        }
        if lo <= 0x7F {
            ascii |= ascii_mask(lo as u8, hi.min(0x7F) as u8);
        }
        if hi >= 0x80 {
            non_ascii = true;
            all_non_ascii |= lo <= 0x80 && hi >= 0x10FFFF;
        }
    }
    if negate {
        ascii = !ascii;
        non_ascii = !all_non_ascii;
    }
    byte_class(ascii, non_ascii)
}

/// The bits `lo..=hi` of an ASCII bitset.
fn ascii_mask(lo: u8, hi: u8) -> u128 {
    debug_assert!(lo <= hi && hi <= 0x7F);
    let width = u32::from(hi - lo) + 1;
    let run = if width == 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    };
    run << lo
}

/// The class of the ASCII bytes in `ascii`, and of any non-ASCII character
/// when `non_ascii`.
fn byte_class(ascii: u128, non_ascii: bool) -> Hir {
    let mut ranges = Vec::new();
    let mut byte = 0u32;
    while byte <= 0x7F {
        if ascii & (1u128 << byte) == 0 {
            byte += 1;
            continue;
        }
        let lo = byte;
        while byte < 0x7F && ascii & (1u128 << (byte + 1)) != 0 {
            byte += 1;
        }
        ranges.push(ClassBytesRange::new(lo as u8, byte as u8));
        byte += 1;
    }
    match (ranges.is_empty(), non_ascii) {
        (false, false) => Hir::class(Class::Bytes(ClassBytes::new(ranges))),
        (false, true) => alternation(vec![
            Hir::class(Class::Bytes(ClassBytes::new(ranges))),
            non_ascii_char(),
        ]),
        (true, true) => non_ascii_char(),
        (true, false) => Hir::fail(),
    }
}

/// Whether the HIR never matches (an empty class).
fn is_fail(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Class(Class::Bytes(class)) => class.ranges().is_empty(),
        HirKind::Class(Class::Unicode(class)) => class.ranges().is_empty(),
        _ => false,
    }
}

/// `Hir::concat` without the parts a never-matching part makes
/// unreachable, which `Hir::concat` keeps.
fn concat(parts: Vec<Hir>) -> Hir {
    if parts.iter().any(is_fail) {
        Hir::fail()
    } else {
        Hir::concat(parts)
    }
}

/// `sub` repeated `min` to `max` times (`None`: unbounded), always greedy,
/// since the automata only ask where a seek matches.
///
/// A repetition of a repetition is flattened: `(X{a,b}){c,d}` reads between
/// `a·c` and `b·d` copies of `X`, so `X{a·c,b·d}` matches wherever it does
/// and compiles to fewer states. Counted bounds above `MAX_COUNTED_REPEAT`
/// become unbounded.
fn repetition(mut min: u32, mut max: Option<u32>, sub: Hir, inexact: &mut bool) -> Hir {
    let nested = match sub.kind() {
        HirKind::Repetition(inner) => Some((inner.min, inner.max, (*inner.sub).clone())),
        _ => None,
    };
    let sub = match nested {
        Some((inner_min, inner_max, inner)) => {
            min = min.saturating_mul(inner_min);
            max = match (max, inner_max) {
                (Some(outer), Some(inner)) => Some(outer.saturating_mul(inner)),
                _ => None,
            };
            inner
        }
        None => sub,
    };
    if min > MAX_COUNTED_REPEAT || max.is_some_and(|max| max > MAX_COUNTED_REPEAT) {
        // A bound above the limit widens the repetition.
        *inexact = true;
        min = min.min(MAX_COUNTED_REPEAT);
        max = None;
    }
    Hir::repetition(Repetition {
        min,
        max,
        greedy: true,
        sub: Box::new(sub),
    })
}

/// `Hir::alternation` without the branches that never match, which
/// `Hir::alternation` keeps.
fn alternation(mut branches: Vec<Hir>) -> Hir {
    branches.retain(|branch| !is_fail(branch));
    if branches.is_empty() {
        Hir::fail()
    } else {
        Hir::alternation(branches)
    }
}

/// The `(from, to)` pairs of a code range buffer (`data[0]` is the count).
fn code_ranges(bbuf: &BBuf) -> impl Iterator<Item = (u32, u32)> + '_ {
    let words = bbuf.data.as_chunks::<4>().0;
    let n = words.first().map_or(0, |chunk| u32::from_ne_bytes(*chunk)) as usize;
    words[1..]
        .as_chunks::<2>()
        .0
        .iter()
        .take(n)
        .map(|pair| (u32::from_ne_bytes(pair[0]), u32::from_ne_bytes(pair[1])))
}

/// The members of a class node, as the matcher reads them over UTF-8: the
/// bits below 0x80 decide a single-byte character and the code ranges from
/// 0x80 up a multibyte one, and nothing else is read. Nested negations
/// (`[^[^[^İ]\S]]`) leave the other side with entries the matcher never
/// consults: bits at 0x80 and up, and code ranges below 0x80.
fn class_members(cc: &CClassNode) -> impl Iterator<Item = (u32, u32)> + '_ {
    let bits = bitset_members(&cc.bs)
        .filter(|&member| member < 0x80)
        .map(|member| (member as u32, member as u32));
    let ranges = cc
        .mbuf
        .iter()
        .flat_map(code_ranges)
        .filter_map(|(lo, hi)| (hi >= 0x80).then_some((lo.max(0x80), hi)));
    bits.chain(ranges)
}

/// The ASCII half boundary a negative look stands for, where its body is
/// one class that holds every ASCII word character: `(?<!\w)` and
/// `(?<![-\w])` hold only where the previous character is not an ASCII word
/// character (`\b{start-half}`), `(?!\w)` and `(?![.:\w])` only where the
/// next one is not (`\b{end-half}`). Next to a non-ASCII character the
/// half boundary holds and the look may not, which admits more. Without
/// this, a keyword list behind `(?<![-\w])` would be a candidate inside
/// every identifier that contains a keyword.
fn word_class_look(an: &AnchorNode) -> Option<Look> {
    let look = match an.anchor_type {
        ANCR_LOOK_BEHIND_NOT => Look::WordStartHalfAscii,
        ANCR_PREC_READ_NOT => Look::WordEndHalfAscii,
        _ => return None,
    };
    let body = an.body.as_deref()?;
    let word_bytes = (b'0'..=b'9')
        .chain(b'A'..=b'Z')
        .chain(b'a'..=b'z')
        .chain(*b"_");
    let holds_every_word_byte = match &body.inner {
        NodeInner::CType(ct) => ct.ctype == ONIGENC_CTYPE_WORD as i32 && !ct.not,
        NodeInner::CClass(cc) => {
            let mut members = word_bytes.map(|byte| bitset_at(&cc.bs, byte as usize));
            if cc.is_not() {
                members.all(|member| !member)
            } else {
                members.all(|member| member)
            }
        }
        _ => false,
    };
    holds_every_word_byte.then_some(look)
}

/// A group-number condition (`(?(1)…)`, `(?(<name>)…)`): a back reference
/// that only checks whether the group captured.
fn is_capture_check(node: &Node) -> bool {
    matches!(node.inner, NodeInner::BackRef(_)) && node.has_status(ND_ST_CHECKER)
}

/// Whether a stop-backtrack bag is one `tune_next` made for automatic
/// possessification: a greedy repeat followed by an item that cannot start
/// where its body does is `(?>a*)` followed by that item (C: `tune_next`).
/// Only that pass sets `ND_ST_STRICT_REAL_REPEAT`, and it sets it on that
/// bag alone, so a user-written atomic group or possessive quantifier never
/// has it. The bag keeps the language of its repetition (the item cannot
/// match at any position the repeat could give back to), so the seek writes
/// the plain repetition and stays exact.
fn automatic_possessive(node: &Node) -> bool {
    node.has_status(ND_ST_STRICT_REAL_REPEAT)
}

/// A node that produces nothing in the seek and consumes nothing, so a
/// positive look-ahead before it may keep its body in place.
fn droppable(node: &Node) -> bool {
    match &node.inner {
        NodeInner::BackRef(_) => is_capture_check(node),
        NodeInner::Anchor(an) => match an.anchor_type {
            ANCR_PREC_READ_NOT | ANCR_LOOK_BEHIND_NOT => word_class_look(an).is_none(),
            ANCR_PREC_READ
            | ANCR_LOOK_BEHIND
            | ANCR_BEGIN_POSITION
            | ANCR_TEXT_SEGMENT_BOUNDARY
            | ANCR_NO_TEXT_SEGMENT_BOUNDARY => true,
            _ => false,
        },
        NodeInner::Gimmick(_) => true,
        NodeInner::String(sn) => sn.s.is_empty(),
        NodeInner::Quant(qn) => qn.upper == 0 || qn.body.as_deref().is_none_or(droppable),
        NodeInner::Bag(bn) => match &bn.bag_data {
            BagData::IfElse {
                then_node,
                else_node,
            } => {
                bn.body.as_deref().is_none_or(droppable)
                    && then_node.as_deref().is_none_or(droppable)
                    && else_node.as_deref().is_none_or(droppable)
            }
            _ => bn.body.as_deref().is_none_or(droppable),
        },
        NodeInner::List(_) | NodeInner::Alt(_) => cons_items(node).into_iter().all(droppable),
        _ => false,
    }
}

/// The code points of a node that is a class, a string, or a list,
/// alternation or group of those, added to `ranges`, with the fewest and
/// most characters it reads. `None` for any other node.
fn class_union(node: &Node, ranges: &mut Vec<(u32, u32)>) -> Option<(usize, usize)> {
    match &node.inner {
        NodeInner::CClass(cc) => {
            if cc.is_not() {
                return None;
            }
            ranges.extend(class_members(cc));
            Some((1, 1))
        }
        NodeInner::String(sn) => {
            if node.has_status(ND_ST_IGNORECASE) && !sn.is_crude() {
                return None;
            }
            let text = std::str::from_utf8(&sn.s).ok()?;
            let mut count = 0;
            for c in text.chars() {
                ranges.push((c as u32, c as u32));
                count += 1;
            }
            Some((count, count))
        }
        NodeInner::List(_) => {
            let mut lens = (0, 0);
            for item in cons_items(node) {
                let (lo, hi) = class_union(item, ranges)?;
                lens = (lens.0 + lo, lens.1 + hi);
            }
            Some(lens)
        }
        NodeInner::Alt(_) => {
            let mut lens = (usize::MAX, 0);
            for item in cons_items(node) {
                let (lo, hi) = class_union(item, ranges)?;
                lens = (lens.0.min(lo), lens.1.max(hi));
            }
            Some(lens)
        }
        NodeInner::Bag(bn) if !matches!(bn.bag_data, BagData::IfElse { .. }) => {
            class_union(bn.body.as_deref()?, ranges)
        }
        _ => None,
    }
}

/// The elements of a List or Alt chain.
fn cons_items(node: &Node) -> Vec<&Node> {
    let mut items = Vec::new();
    let mut cur = node;
    loop {
        match &cur.inner {
            NodeInner::List(cons) | NodeInner::Alt(cons) => {
                items.push(&*cons.car);
                match &cons.cdr {
                    Some(next) => cur = next,
                    None => break,
                }
            }
            _ => {
                items.push(cur);
                break;
            }
        }
    }
    items
}

/// Writes the `\Z` nodes that end the pattern: the last element of a list,
/// a branch of an alternation in that position, or the body of a group there.
/// Nothing follows such a node, so `\Z` holds exactly where the seek writes
/// `(?:\n)?\z`: before the end or before a final newline. Elsewhere `\Z` is
/// approximated by `$` (`Walk::anchor`). Addresses only, compared and never
/// read.
fn collect_tails(node: &Node, out: &mut Vec<*const Node>) {
    match &node.inner {
        NodeInner::List(_) => {
            if let Some(&last) = cons_items(node).last() {
                collect_tails(last, out);
            }
        }
        NodeInner::Alt(_) => {
            for branch in cons_items(node) {
                collect_tails(branch, out);
            }
        }
        NodeInner::Bag(bn) if !matches!(bn.bag_data, BagData::IfElse { .. }) => {
            if let Some(body) = bn.body.as_deref() {
                collect_tails(body, out);
            }
        }
        NodeInner::Anchor(an) if an.anchor_type == ANCR_SEMI_END_BUF => {
            out.push(node as *const Node);
        }
        _ => {}
    }
}

/// One walk over a tuned parse tree, writing the seek HIR.
struct Walk<'a> {
    reg: &'a RegexType,
    /// Positive look-aheads whose body was kept in place so far.
    lookaheads_kept: u32,
    /// A construct the seek approximates was written (`SeekCode::exact`).
    inexact: bool,
    /// The `\Z` nodes that end the pattern (`collect_tails`), by address.
    tails: Vec<*const Node>,
    /// The seek of a single `Regex` (`derive`'s `precise`): alternations
    /// are written branch by branch, where the scanner's seek shortcuts a
    /// class-led alternation as a repetition of its class.
    precise: bool,
}

impl Walk<'_> {
    /// `node` as an element whose following siblings are all droppable
    /// when `rest_droppable`: a positive look-ahead then keeps its body.
    fn item(&mut self, node: &Node, rest_droppable: bool) -> Hir {
        if let NodeInner::Anchor(an) = &node.inner {
            if an.anchor_type == ANCR_PREC_READ {
                return self.lookahead(an, rest_droppable);
            }
        }
        self.node(node, rest_droppable)
    }

    fn lookahead(&mut self, an: &AnchorNode, keep: bool) -> Hir {
        self.inexact = true;
        if keep {
            self.lookaheads_kept += 1;
            an.body
                .as_deref()
                .map_or_else(Hir::empty, |body| self.item(body, true))
        } else {
            Hir::empty()
        }
    }

    fn node(&mut self, node: &Node, rest_droppable: bool) -> Hir {
        if node.has_status(ND_ST_LITERAL_ALT) {
            return self.trie(node);
        }
        match &node.inner {
            NodeInner::String(sn) => {
                // The tuner unravels a case-insensitive string into exact
                // alternatives; one it left stands for anything.
                if node.has_status(ND_ST_IGNORECASE) {
                    self.inexact = true;
                    if !sn.is_crude() {
                        return placeholder();
                    }
                }
                if sn.s.is_empty() {
                    Hir::empty()
                } else {
                    Hir::literal(sn.s.clone().into_boxed_slice())
                }
            }
            NodeInner::CClass(cc) => self.cclass(cc),
            NodeInner::CType(ct) => self.ctype(node, ct),
            NodeInner::BackRef(_) | NodeInner::Call(_) => {
                self.inexact = true;
                placeholder()
            }
            NodeInner::Quant(qn) => {
                let Some(body) = qn.body.as_deref() else {
                    return Hir::empty();
                };
                // A look-ahead kept in a body that runs again would be read
                // before the next iteration's text, so only a body that runs
                // at most once keeps one.
                let body_rest = rest_droppable && (0..=1).contains(&qn.upper);
                let sub = self.item(body, body_rest);
                let min = u32::try_from(qn.lower.max(0)).unwrap_or(u32::MAX);
                let max = (qn.upper >= 0).then_some(qn.upper as u32);
                repetition(min, max, sub, &mut self.inexact)
            }
            NodeInner::Bag(bn) => match &bn.bag_data {
                // The condition runs first and, where it holds, the then
                // branch goes on after what the condition consumed; where
                // it fails, the else branch runs from the start. A
                // group-number condition (`(?(1)…)`) is a zero-width check.
                BagData::IfElse {
                    then_node,
                    else_node,
                } => {
                    self.inexact = true;
                    let condition = bn
                        .body
                        .as_deref()
                        .map_or_else(Hir::empty, |cond| self.condition(cond));
                    let then = then_node
                        .as_deref()
                        .map_or_else(Hir::empty, |n| self.item(n, rest_droppable));
                    let otherwise = else_node
                        .as_deref()
                        .map_or_else(Hir::empty, |n| self.item(n, rest_droppable));
                    alternation(vec![concat(vec![condition, then]), otherwise])
                }
                // An atomic group keeps only the first way its body matches,
                // which the seek, keeping every way, does not: `(?>abc|ab)c`
                // has no match in `abc`, its seek does. The bag `tune_next`
                // makes for automatic possessification is the repetition
                // itself, see `automatic_possessive`.
                BagData::StopBacktrack | BagData::TextSegment => {
                    if !(matches!(bn.bag_data, BagData::StopBacktrack)
                        && automatic_possessive(node))
                    {
                        self.inexact = true;
                    }
                    bn.body
                        .as_deref()
                        .map_or_else(Hir::empty, |body| self.item(body, rest_droppable))
                }
                _ => bn
                    .body
                    .as_deref()
                    .map_or_else(Hir::empty, |body| self.item(body, rest_droppable)),
            },
            NodeInner::Anchor(an) => {
                if self.tails.contains(&(node as *const Node)) {
                    let newline = repetition(0, Some(1), Hir::literal(*b"\n"), &mut self.inexact);
                    return concat(vec![newline, Hir::look(Look::End)]);
                }
                self.anchor(an)
            }
            NodeInner::List(_) => self.list(node, rest_droppable),
            NodeInner::Alt(_) => {
                let branches = cons_items(node);
                // `(?i)` turns a class with multi-character folds into
                // `Alt(class, "ss", "fi", …)`, and such classes repeat in a
                // pattern: the union of their characters, repeated as often
                // as the longest branch reads, stands for all of them.
                if !self.precise
                    && branches.len() >= 2
                    && matches!(branches[0].inner, NodeInner::CClass(_))
                {
                    let mut ranges = Vec::new();
                    let mut lens: Option<(usize, usize)> = Some((usize::MAX, 0));
                    for branch in &branches {
                        match class_union(branch, &mut ranges) {
                            Some((lo, hi)) => lens = lens.map(|(a, b)| (a.min(lo), b.max(hi))),
                            None => {
                                lens = None;
                                break;
                            }
                        }
                    }
                    if let Some((min, max)) = lens {
                        self.inexact = true;
                        return Hir::repetition(Repetition {
                            min: u32::try_from(min).unwrap_or(u32::MAX),
                            max: Some(u32::try_from(max).unwrap_or(u32::MAX)),
                            greedy: true,
                            sub: Box::new(class_hir(ranges, false)),
                        });
                    }
                }
                let branches = branches
                    .into_iter()
                    .map(|branch| self.item(branch, rest_droppable))
                    .collect();
                alternation(branches)
            }
            // `\K` and the absent operator's bookkeeping consume nothing;
            // the absent operator's own `Fail` never matches (`(*FAIL)` is
            // a callout, which leaves the pattern without a seek).
            NodeInner::Gimmick(gn) => {
                // `\K` is exact for existence. The range cutter of the absent
                // operator (a save and updates of the range, and a `Fail`)
                // is not, and the other gimmicks are left out.
                let keep = match gn.gimmick_type {
                    GimmickType::Save => gn.detail_type == SaveType::Keep as i32,
                    GimmickType::UpdateVar => {
                        gn.detail_type == UpdateVarType::KeepFromStackLast as i32
                    }
                    _ => false,
                };
                if !keep {
                    self.inexact = true;
                }
                match gn.gimmick_type {
                    GimmickType::Fail => Hir::fail(),
                    _ => Hir::empty(),
                }
            }
        }
    }

    /// The condition of a conditional: a group-number condition is a
    /// zero-width check of the group's capture; any other reads text as the
    /// expression it is.
    fn condition(&mut self, cond: &Node) -> Hir {
        if is_capture_check(cond) {
            Hir::empty()
        } else {
            self.item(cond, false)
        }
    }

    fn list(&mut self, node: &Node, rest_droppable: bool) -> Hir {
        let items = cons_items(node);
        let mut parts = Vec::with_capacity(items.len());
        let mut kept_before = false;
        for (k, item) in items.iter().enumerate() {
            // At most one look-ahead per list keeps its body: two of them
            // hold at the same position, and in the seek the second would
            // follow the first's text.
            let rest =
                rest_droppable && !kept_before && items[k + 1..].iter().all(|n| droppable(n));
            let kept = self.lookaheads_kept;
            parts.push(self.item(item, rest));
            if self.lookaheads_kept != kept {
                kept_before = true;
            }
        }
        concat(parts)
    }

    fn anchor(&mut self, an: &AnchorNode) -> Hir {
        // Only `\A`, `\z`, `$` and `^` read the text the way the seek does.
        if !matches!(
            an.anchor_type,
            ANCR_BEGIN_BUF | ANCR_END_BUF | ANCR_END_LINE | ANCR_BEGIN_LINE
        ) {
            self.inexact = true;
        }
        match an.anchor_type {
            // A look-ahead reaches here only where something consuming
            // follows it. Look-behinds read what is before the position.
            ANCR_PREC_READ | ANCR_LOOK_BEHIND => Hir::empty(),
            ANCR_PREC_READ_NOT | ANCR_LOOK_BEHIND_NOT => {
                word_class_look(an).map_or_else(Hir::empty, Hir::look)
            }
            ANCR_BEGIN_BUF => Hir::look(Look::Start),
            ANCR_END_BUF => Hir::look(Look::End),
            // `\Z` holds before a final newline; `(?m:$)` before any.
            ANCR_SEMI_END_BUF | ANCR_END_LINE => Hir::look(Look::EndLF),
            ANCR_BEGIN_LINE => Hir::look(Look::StartLF),
            // `\G` depends on the search start.
            ANCR_BEGIN_POSITION => Hir::empty(),
            ANCR_WORD_BOUNDARY | ANCR_NO_WORD_BOUNDARY | ANCR_WORD_BEGIN | ANCR_WORD_END
                if an.ascii_mode =>
            {
                // ASCII word boundaries need no quit bytes.
                Hir::look(match an.anchor_type {
                    ANCR_WORD_BOUNDARY => Look::WordAscii,
                    ANCR_NO_WORD_BOUNDARY => Look::WordAsciiNegate,
                    ANCR_WORD_BEGIN => Look::WordStartAscii,
                    _ => Look::WordEndAscii,
                })
            }
            ANCR_WORD_BOUNDARY | ANCR_NO_WORD_BOUNDARY | ANCR_WORD_BEGIN | ANCR_WORD_END => {
                // A Unicode word boundary makes a lazy DFA quit at non-ASCII
                // bytes, after which a search gives up on the DFA (see
                // `candidate_in`). Between two ASCII characters it is the ASCII
                // boundary; next to a non-ASCII character it may hold, and
                // the ASCII half boundaries (the previous or the next
                // character is not an ASCII word character) admit that.
                Hir::alternation(vec![
                    Hir::look(match an.anchor_type {
                        ANCR_WORD_BOUNDARY => Look::WordAscii,
                        ANCR_NO_WORD_BOUNDARY => Look::WordAsciiNegate,
                        ANCR_WORD_BEGIN => Look::WordStartAscii,
                        _ => Look::WordEndAscii,
                    }),
                    Hir::look(Look::WordStartHalfAscii),
                    Hir::look(Look::WordEndHalfAscii),
                ])
            }
            // Text segment boundaries (`\y`, `\Y`).
            _ => Hir::empty(),
        }
    }

    fn cclass(&mut self, cc: &CClassNode) -> Hir {
        let ranges: Vec<(u32, u32)> = class_members(cc).collect();
        if self.precise && !cc.is_not() {
            if let Some(hir) = explicit_class(&ranges) {
                return hir;
            }
        }
        if !class_exact(&ranges) {
            self.inexact = true;
        }
        class_hir(ranges, cc.is_not())
    }

    fn ctype(&mut self, node: &Node, ct: &CtypeNode) -> Hir {
        if ct.ctype == CTYPE_ANYCHAR {
            return if node.has_status(ND_ST_MULTILINE) {
                any_char()
            } else {
                any_char_except_newline()
            };
        }
        // The non-ASCII part of a Unicode `\w` or `\W` is every non-ASCII
        // character, which is exact only in ASCII mode (see below).
        if !(ct.ctype == ONIGENC_CTYPE_WORD as i32 && ct.ascii_mode) {
            self.inexact = true;
        }
        if ct.ctype == ONIGENC_CTYPE_WORD as i32 {
            // The ASCII word characters are the same in both modes; the
            // Unicode mode adds non-ASCII ones, which stand for any
            // non-ASCII character. `\W` is the complement of the exact ASCII
            // set plus any non-ASCII character: every one of them in ASCII
            // mode, all but the Unicode word characters otherwise.
            // Complementing the widened set would leave them all out.
            let word = ascii_mask(b'0', b'9')
                | ascii_mask(b'A', b'Z')
                | ascii_mask(b'_', b'_')
                | ascii_mask(b'a', b'z');
            return if ct.not {
                byte_class(!word, true)
            } else {
                byte_class(word, !ct.ascii_mode)
            };
        }
        // The parser turns the other ctypes into classes; a remaining one
        // stands for any character.
        any_char()
    }

    /// A literal trie: its literals, or for a folded trie what the engine
    /// accepts for them (ASCII case pairs, the non-ASCII members of each
    /// letter's class, and the strings of each multi-character segment).
    fn trie(&mut self, node: &Node) -> Hir {
        let reg = self.reg;
        let Some(trie) = literal_alt_trie_index(node).and_then(|idx| reg.literal_tries.get(idx))
        else {
            self.inexact = true;
            return placeholder();
        };
        let literals = trie.literals();
        if !trie.is_case_insensitive() {
            return alternation(
                literals
                    .iter()
                    .map(|literal| Hir::literal(literal.clone().into_boxed_slice()))
                    .collect(),
            );
        }
        self.inexact = true;
        let Some(folds) = trie.folds() else {
            return placeholder();
        };
        // Every literal as ASCII case classes: one byte class per letter.
        let mut branches: Vec<Hir> = literals
            .iter()
            .map(|literal| {
                concat(
                    literal
                        .iter()
                        .map(|&c| {
                            if c.is_ascii_alphabetic() {
                                Hir::class(Class::Bytes(ClassBytes::new([
                                    ClassBytesRange::new(
                                        c.to_ascii_lowercase(),
                                        c.to_ascii_lowercase(),
                                    ),
                                    ClassBytesRange::new(
                                        c.to_ascii_uppercase(),
                                        c.to_ascii_uppercase(),
                                    ),
                                ])))
                            } else {
                                Hir::literal([c])
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        // A folded match that reads a non-ASCII character (a class member
        // such as the Kelvin sign for `k`, or a ligature such as `ß` for
        // `ss`) reads at most `longest - 1` ASCII characters before it and,
        // since every remaining literal character reads at least one text
        // character, at most `longest - 1` characters of any kind after it:
        // one escape alternative per trie admits those matches, and ends
        // where the literal can end, so what follows the trie is read from
        // there.
        let mut non_ascii: Vec<Hir> = folds
            .class_members
            .iter()
            .filter_map(|&(code, _)| char::from_u32(code))
            .map(|c| Hir::literal(c.to_string().into_bytes().into_boxed_slice()))
            .chain(
                folds
                    .ligatures
                    .iter()
                    .map(|(bytes, _)| Hir::literal(bytes.clone().into_boxed_slice())),
            )
            .collect();
        if !non_ascii.is_empty() {
            non_ascii.sort_by_key(|a| a.to_string());
            non_ascii.dedup_by(|a, b| a.to_string() == b.to_string());
            let longest = literals.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let rest = u32::try_from(longest - 1).unwrap_or(u32::MAX);
            let ascii_prefix = repetition(
                0,
                Some(rest),
                Hir::class(Class::Bytes(ClassBytes::new([ClassBytesRange::new(
                    0x00, 0x7F,
                )]))),
                &mut self.inexact,
            );
            let suffix = repetition(0, Some(rest), any_char(), &mut self.inexact);
            branches.push(concat(vec![ascii_prefix, alternation(non_ascii), suffix]));
        }
        alternation(branches)
    }
}

/// The automata of one pattern list: forward and reverse lazy DFAs over the
/// covered seeks for the earliest candidate position, and an overlapping
/// lazy DFA for the entries whose seek matches there. Both are read-only
/// once built (their lazy DFA caches are the set's, in `SetPrefilter`), so
/// every set over the same pattern list can hold the same `Automata`.
pub(crate) struct Automata {
    search: Arc<hybrid_regex::Regex>,
    dfa: hybrid::DFA,
    /// The entry index each pattern ID stands for, ascending.
    entries: Vec<u16>,
    /// Entries searched on their own, in index order.
    own: Vec<u16>,
    /// The pattern IDs of the seeks that can keep a walk alive past
    /// `settle`: unbounded or longer than `LONG_SEEK_BYTES`.
    long: Vec<u16>,
    /// Bytes after which every other seek has settled.
    settle: usize,
}

/// A set's handle on its automata: built by the first set that searches
/// with the pre-filter, and shared with every set built from one pattern
/// cache over the same pattern list (ADR-006). Holds `None` once a build
/// found nothing to cover, an entry with callouts, or too much
/// (`MAX_NFA_STATES`): those sets keep the position-lead search.
#[derive(Clone, Default)]
pub(crate) struct AutomataCell(Arc<OnceLock<Option<Arc<Automata>>>>);

impl AutomataCell {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The automata, built by `build` unless a set has built them already.
    pub(crate) fn get_or_build(
        &self,
        build: impl FnOnce() -> Option<Automata>,
    ) -> Option<Arc<Automata>> {
        self.0.get_or_init(|| build().map(Arc::new)).clone()
    }
}

/// A set's part of the pre-filter: its lazy DFA caches over the shared
/// [`Automata`], and the scratch state of its searches.
pub(crate) struct SetPrefilter {
    /// The automata of the set's pattern list: clones of the shared
    /// [`Automata`]'s, which keep their NFAs and transition tables behind
    /// `Arc`s, so a search reads them as the set's own, without an
    /// indirection, and the heavy parts are held once.
    search: Arc<hybrid_regex::Regex>,
    search_cache: hybrid_regex::Cache,
    dfa: hybrid::DFA,
    dfa_cache: hybrid::Cache,
    patset: PatternBits,
    /// The entry index each pattern ID stands for, ascending.
    entries: Vec<u16>,
    /// Entries searched on their own, in index order.
    own: Vec<u16>,
    /// The pattern IDs of the seeks that can keep a walk alive past
    /// `settle`: unbounded or longer than `LONG_SEEK_BYTES`.
    long: Vec<u16>,
    /// Bytes after which every other seek has settled.
    settle: usize,
    /// The last walk ran to the bound or ended by asking: the next one
    /// asks as soon as the bounded seeks have settled.
    ask_early: bool,
    /// The overlapping DFA or a search DFA gave up on its cache
    /// (`MIN_CACHE_CLEARS`): the set retires the pre-filter after the search.
    gave_up: bool,
    /// Bytes the walks of the current search read, which the DFA cache's
    /// efficiency check counts: one progress span per search, updated
    /// after each walk (a span per walk cost the captured C replay 8% of
    /// its instructions).
    walked: usize,
    /// Per pattern ID, the bytes the set's first-byte table dispatches its
    /// entry on (every byte for a fallback entry).
    dispatch: Vec<[u64; 4]>,
    /// The bytes any long seek's entry is dispatched on, and any entry's.
    long_dispatch: [u64; 4],
    any_dispatch: [u64; 4],
    /// Per pattern ID, the `stamp` of the last walk that found its entry
    /// admissible at the position.
    admissible: Vec<u32>,
    stamp: u32,
    /// Scratch for `candidates_at`, and the position it holds the
    /// candidates of in the current search.
    candidates: Vec<u16>,
    walked_at: Option<usize>,
    /// The last search result for a subject (`earliest_memo`).
    search_memo: Option<SearchMemo>,
    /// The walks of a subject searched again.
    candidate_memo: CandidateMemo,
}

/// What the walks of a subject found (`candidates_at`), per position, for
/// the searches of that subject after its first: a walk reads the subject
/// from its position and asks admission there, both of which depend on the
/// subject only (see `SearchMemo`), so a later search reuses it where it
/// walks a position again, the way the position-lead search reuses its
/// fallback memo. A line tokenized again (an editor re-tokenizing it, a
/// benchmark) walks every position again; a tokenizing pass walks a
/// position the previous search's window walked. The memo holds at most
/// `CANDIDATE_MEMO_POSITIONS` positions, kept sorted (the loop walks
/// forwards), and is cleared when the subject changes. Recording starts
/// once a search of the subject starts at or before the previous one's
/// start, which a tokenizing pass never does (every call starts at the
/// last match's end), so a line tokenized once pays nothing: recording
/// every walk cost the captured C replay 10% of its instructions.
#[derive(Default)]
struct CandidateMemo {
    subject: Option<(crate::regset::FallbackMemoIdentity, usize)>,
    /// The subject was searched again from no later than before: record
    /// and reuse.
    active: bool,
    /// Where the last search of the subject started.
    last_start: usize,
    /// The positions walked, ascending, each with its candidates' span in
    /// `store` and whether a seek matched there (`Some` from
    /// `candidates_at`).
    at: Vec<u32>,
    spans: Vec<(u32, u16, bool)>,
    store: Vec<u16>,
}

/// Records the patterns of the match state `sid`. Out of line on purpose:
/// inlined into the byte walk, the bitset's fields kept the walk's state ID
/// on the stack in Ferriki's arm64 addon, and a walk reaches a match state
/// far less often than it reads a byte. Rust-only (ADR-008).
#[inline(never)]
fn record_match(
    dfa: &hybrid::DFA,
    cache: &hybrid::Cache,
    patset: &mut PatternBits,
    sid: LazyStateID,
) {
    for k in 0..dfa.match_len(cache, sid) {
        patset.insert(dfa.match_pattern(cache, sid, k));
    }
}

/// `record_match`, counting the newly recorded patterns stamped `stamp`.
#[inline(never)]
fn record_needed(
    dfa: &hybrid::DFA,
    cache: &hybrid::Cache,
    patset: &mut PatternBits,
    sid: LazyStateID,
    stamps: &[u32],
    stamp: u32,
) -> usize {
    let mut found = 0;
    for k in 0..dfa.match_len(cache, sid) {
        let pid = dfa.match_pattern(cache, sid, k);
        if patset.insert(pid) && stamps[pid.as_usize()] == stamp {
            found += 1;
        }
    }
    found
}

/// The covered entries a walk recorded, as a bitset over their pattern IDs.
/// `regex_automata::PatternSet` keeps one bool per pattern, which a walk
/// pays for in every `clear` and every ascending scan of the set;
/// membership and insertion here are one word access, and `iter` yields the
/// IDs ascending by `trailing_zeros`, so the candidates keep their order.
/// Rust-only (ADR-008): the same set of IDs, in the same order.
struct PatternBits {
    words: Box<[u64]>,
    /// The bits set.
    len: usize,
    capacity: usize,
}

impl PatternBits {
    fn new(capacity: usize) -> Self {
        PatternBits {
            words: vec![0; capacity.div_ceil(64)].into_boxed_slice(),
            len: 0,
            capacity,
        }
    }

    /// Empties the set; a set that holds nothing is left as it is.
    #[inline]
    fn clear(&mut self) {
        if self.len > 0 {
            self.words.fill(0);
            self.len = 0;
        }
    }

    #[inline]
    fn contains(&self, pid: PatternID) -> bool {
        let i = pid.as_usize();
        i < self.capacity && (self.words[i / 64] >> (i % 64)) & 1 == 1
    }

    /// Adds `pid`, and reports whether it was not in the set.
    #[inline]
    fn insert(&mut self, pid: PatternID) -> bool {
        let i = pid.as_usize();
        assert!(i < self.capacity, "pattern ID {i} outside the set");
        let word = &mut self.words[i / 64];
        let bit = 1u64 << (i % 64);
        if *word & bit != 0 {
            return false;
        }
        *word |= bit;
        self.len += 1;
        true
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// The pattern IDs in the set, ascending.
    #[inline]
    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(w, &word)| {
            let mut bits = word;
            std::iter::from_fn(move || {
                (bits != 0).then(|| {
                    let b = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    w * 64 + b
                })
            })
        })
    }
}

/// Positions a `CandidateMemo` holds at most.
const CANDIDATE_MEMO_POSITIONS: usize = 4096;

impl CandidateMemo {
    /// The memo's state for a search of `subject` from `start`: the
    /// previous subject's walks are dropped, and recording starts once a
    /// search of the subject starts no later than the previous one.
    #[inline]
    fn begin(
        &mut self,
        subject: Option<(crate::regset::FallbackMemoIdentity, usize)>,
        start: usize,
    ) {
        if subject.is_some() && subject == self.subject {
            if start <= self.last_start {
                self.active = true;
            }
            self.last_start = start;
            return;
        }
        self.subject = subject;
        self.active = false;
        self.last_start = start;
        self.at.clear();
        self.spans.clear();
        self.store.clear();
    }

    /// The candidates a walk at `at` found, and whether a seek matched.
    fn get(&self, at: usize) -> Option<(&[u16], bool)> {
        let at = u32::try_from(at).ok()?;
        let i = self.at.binary_search(&at).ok()?;
        let (start, len, some) = self.spans[i];
        Some((
            &self.store[start as usize..start as usize + len as usize],
            some,
        ))
    }

    /// Records a walk at `at`.
    fn insert(&mut self, at: usize, candidates: &[u16], some: bool) {
        if self.at.len() >= CANDIDATE_MEMO_POSITIONS {
            return;
        }
        let (Ok(at), Ok(len)) = (u32::try_from(at), u16::try_from(candidates.len())) else {
            return;
        };
        let start = self.store.len() as u32;
        self.store.extend_from_slice(candidates);
        let i = match self.at.binary_search(&at) {
            Ok(_) => return,
            Err(i) => i,
        };
        self.at.insert(i, at);
        self.spans.insert(i, (start, len, some));
    }
}

/// What the covered searches of a subject found, reusable by every later
/// search of it from a position at or after `from`: no covered entry can
/// decide in `[from, at)` and one can at `at`, or, with `at` `None`, none
/// in `[from, until)` (to the end of the subject once the windows reached
/// it). The facts are start-independent: the loop head looks at a search's
/// first position itself, and admission elsewhere depends on the subject
/// and the position only (the gate cache serves later starts alike).
#[derive(Clone, Copy)]
struct SearchMemo {
    subject: (crate::regset::FallbackMemoIdentity, usize),
    from: usize,
    until: usize,
    at: Option<usize>,
}

/// How a candidate walk ended.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanEnd {
    /// The DFA died or the haystack ended: every seek match is recorded.
    Finished,
    /// Stopped early: every entry that can decide at the position is
    /// recorded.
    Settled,
    /// The bound, or the DFA quit: the DFA is still alive.
    Bound,
    /// The DFA gave up on its cache: as `Bound`, and the set retires the
    /// pre-filter after the search.
    GaveUp,
}

impl std::fmt::Debug for SetPrefilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetPrefilter")
            .field("entries", &self.entries.len())
            .field("own", &self.own.len())
            .field("nfa_states", &self.nfa_states())
            .finish_non_exhaustive()
    }
}

impl Automata {
    /// The automata over the seeks of the entries (in entry order, each with
    /// its regex), or `None` when no entry can be pre-filtered, an entry has
    /// callouts, the automata would be too large (`MAX_NFA_STATES`) or do
    /// not build, or, with `skip_tiny`, too small to pay (`TINY_SET_ENTRIES`).
    pub(crate) fn build<'a>(
        entries: impl Iterator<Item = (Option<&'a Seek>, &'a RegexType)>,
        skip_tiny: bool,
    ) -> Option<Self> {
        let mut hirs: Vec<Hir> = Vec::new();
        let mut covered = Vec::new();
        let mut own = Vec::new();
        for (index, (seek, reg)) in entries.enumerate() {
            let index = u16::try_from(index).ok()?;
            match seek {
                Some(Seek::Pattern(code)) => {
                    hirs.push(code.decode());
                    covered.push(index);
                }
                Some(Seek::Everywhere) => own.push(index),
                // Callouts observe every attempt; nothing in the set may
                // skip one. Patterns compiled without a seek are searched on
                // their own.
                None if reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0) => return None,
                None => own.push(index),
            }
        }
        if covered.is_empty() {
            return None;
        }
        let entries = covered;
        let nfa = thompson::Compiler::new()
            .configure(
                thompson::Config::new()
                    .utf8(false)
                    .which_captures(thompson::WhichCaptures::None)
                    .nfa_size_limit(Some(NFA_SIZE_LIMIT)),
            )
            .build_many_from_hir(&hirs)
            .ok()?;
        if nfa.states().len() > MAX_NFA_STATES {
            return None;
        }
        if skip_tiny
            && entries.len() <= TINY_SET_ENTRIES
            && nfa.states().len() <= TINY_SET_NFA_STATES
        {
            return None;
        }
        // The earliest match start is found as regex-automata's meta regex
        // finds it (its lazy DFA strategy): a leftmost-first forward search
        // for the end, then a reverse search from there for the start. The
        // searches report no captures, so both DFAs run over NFAs without
        // capture states. The forward NFA is the one the overlapping DFA
        // runs on, shared rather than compiled again (Rust-only, ADR-008).
        let nfarev = thompson::Compiler::new()
            .configure(
                thompson::Config::new()
                    .utf8(false)
                    .reverse(true)
                    .which_captures(thompson::WhichCaptures::None)
                    .nfa_size_limit(Some(NFA_SIZE_LIMIT)),
            )
            .build_many_from_hir(&hirs)
            .ok()?;
        let forward = search_dfa(nfa.clone(), MatchKind::LeftmostFirst)?;
        let reverse = search_dfa(nfarev, MatchKind::All)?;
        let search = Arc::new(hybrid_regex::Builder::new().build_from_dfas(forward, reverse));
        // The seeks use ASCII look-arounds only, so the DFA never quits.
        let dfa = hybrid::Builder::new()
            .configure(overlapping_config())
            .build_from_nfa(nfa)
            .ok()?;
        let mut long = Vec::new();
        let mut settle = 0;
        for (pid, hir) in hirs.iter().enumerate() {
            match hir.properties().maximum_len() {
                // A match state is entered by the byte after the match.
                Some(len) if len < LONG_SEEK_BYTES => settle = settle.max(len + 1),
                _ => long.push(pid as u16),
            }
        }
        Some(Automata {
            search,
            dfa,
            entries,
            own,
            long,
            settle,
        })
    }

    /// NFA states of the overlapping DFA (the search DFAs hold about twice
    /// as many: a forward and a reverse NFA).
    pub(crate) fn nfa_states(&self) -> usize {
        self.dfa.get_nfa().states().len()
    }

    /// Heap memory of the automata, in bytes, without any cache.
    pub(crate) fn memory_usage(&self) -> usize {
        automata_memory_usage(&self.dfa, &self.search)
    }
}

/// Heap memory of the automata's NFAs and DFAs, in bytes, without any cache.
/// The forward NFA is shared by the overlapping DFA and the forward search
/// DFA, and counted once.
fn automata_memory_usage(dfa: &hybrid::DFA, search: &hybrid_regex::Regex) -> usize {
    dfa.get_nfa().memory_usage()
        + search.reverse().get_nfa().memory_usage()
        + search.forward().memory_usage()
        + search.reverse().memory_usage()
        + dfa.memory_usage()
}

/// A lazy DFA over the seeks' NFA for one of the search DFAs, with the
/// cache capacity and the give-up checks regex-automata's meta regex gives
/// its own lazy DFAs. `None` where it does not build.
fn search_dfa(nfa: thompson::NFA, match_kind: MatchKind) -> Option<hybrid::DFA> {
    hybrid::Builder::new()
        .configure(
            hybrid::Config::new()
                .match_kind(match_kind)
                .cache_capacity(SEARCH_CACHE_CAPACITY)
                .minimum_cache_clear_count(Some(MIN_CACHE_CLEARS))
                .minimum_bytes_per_state(Some(SEARCH_MIN_BYTES_PER_STATE)),
        )
        .build_from_nfa(nfa)
        .ok()
}

impl SetPrefilter {
    /// A set's pre-filter over `automata`, with fresh caches.
    pub(crate) fn new(automata: &Automata) -> Self {
        let search_cache = automata.search.create_cache();
        let dfa_cache = automata.dfa.create_cache();
        let covered = automata.entries.len();
        SetPrefilter {
            search: automata.search.clone(),
            search_cache,
            dfa: automata.dfa.clone(),
            dfa_cache,
            patset: PatternBits::new(covered),
            entries: automata.entries.clone(),
            own: automata.own.clone(),
            long: automata.long.clone(),
            settle: automata.settle,
            ask_early: false,
            gave_up: false,
            walked: 0,
            dispatch: vec![[u64::MAX; 4]; covered],
            long_dispatch: [0; 4],
            any_dispatch: [0; 4],
            admissible: vec![0; covered],
            stamp: 0,
            candidates: Vec::new(),
            walked_at: None,
            search_memo: None,
            candidate_memo: CandidateMemo::default(),
        }
    }

    /// Records, per covered entry, the bytes the set's first-byte table
    /// dispatches it on, for the admission a walk asks about.
    pub(crate) fn set_dispatch(&mut self, mut bytes: impl FnMut(u16) -> [u64; 4]) {
        for (pid, &index) in self.entries.iter().enumerate() {
            self.dispatch[pid] = bytes(index);
        }
        let union = |pids: &mut dyn Iterator<Item = usize>| {
            pids.fold([0u64; 4], |mut bits, pid| {
                for (word, dispatch) in bits.iter_mut().zip(self.dispatch[pid]) {
                    *word |= dispatch;
                }
                bits
            })
        };
        let long_dispatch = union(&mut self.long.iter().map(|&pid| pid as usize));
        let any_dispatch = union(&mut (0..self.entries.len()));
        self.long_dispatch = long_dispatch;
        self.any_dispatch = any_dispatch;
    }

    /// The bytes any covered entry is dispatched on: a position whose byte
    /// is not among them has no candidate.
    #[inline]
    pub(crate) fn any_dispatch(&self) -> [u64; 4] {
        self.any_dispatch
    }

    /// Entries the automata do not cover, in index order.
    pub(crate) fn own(&self) -> &[u16] {
        &self.own
    }

    /// Entries the automata cover, in index order.
    pub(crate) fn covered_entries(&self) -> &[u16] {
        &self.entries
    }

    /// Number of entries the automata cover.
    pub(crate) fn covered(&self) -> usize {
        self.entries.len()
    }

    /// The earliest position in `[from, until)` where some covered entry
    /// can decide, reading the subject only up to `until` plus the bounded
    /// seeks' length (`search_in`), and for a stable `subject` through the
    /// memo of what earlier searches found: a span already known to hold
    /// nothing is not read again, and a window past it is searched from
    /// where the known span ends.
    pub(crate) fn candidate_in(
        &mut self,
        haystack: &[u8],
        from: usize,
        until: usize,
        subject: Option<(crate::regset::FallbackMemoIdentity, usize)>,
        admits: &mut dyn FnMut(u16, usize) -> bool,
    ) -> Option<usize> {
        let mut search_from = from;
        let mut memo_from = from;
        if let (Some(subject), Some(memo)) = (subject, self.search_memo) {
            if memo.subject == subject && from >= memo.from {
                match memo.at {
                    Some(at) if from <= at => return (at < until).then_some(at),
                    None if until <= memo.until => return None,
                    None if from <= memo.until => {
                        search_from = memo.until;
                        memo_from = memo.from;
                    }
                    _ => {}
                }
            }
        }
        let at = self.search_in(haystack, search_from, until, admits);
        if let Some(subject) = subject {
            self.search_memo = Some(SearchMemo {
                subject,
                from: memo_from,
                until,
                at,
            });
        }
        at
    }

    /// Records what the loop head found at `at` for `subject`'s memo, where
    /// the known span ends there: a candidate, or nothing up to `next`.
    #[inline]
    pub(crate) fn note_position(
        &mut self,
        subject: Option<(crate::regset::FallbackMemoIdentity, usize)>,
        at: usize,
        candidate: bool,
        next: usize,
    ) {
        if let (Some(subject), Some(memo)) = (subject, self.search_memo.as_mut()) {
            if memo.subject == subject && memo.at.is_none() && memo.until == at {
                if candidate {
                    memo.at = Some(at);
                } else {
                    memo.until = next;
                }
            }
        }
    }

    /// The earliest position in `[from, until)` where some covered entry
    /// can decide, reading the subject only up to `until` plus the bounded
    /// seeks' length. The position `from` by a walk first: a greedy seek
    /// that matches there would make the search read on to its match end
    /// to report that start. Then the search DFAs over the span find every
    /// bounded seek's match that starts in the window (its match ends
    /// inside the span), and the long seeks, whose matches may end past it,
    /// are walked at the positions up to the search's find where their
    /// bytes dispatch (`candidates_at`, with its bound and admission stop).
    /// Where a search DFA gives up, `from` is returned: a position no later
    /// than the earliest one, which only costs walks, and the set retires
    /// the pre-filter after the search (`gave_up`).
    fn search_in(
        &mut self,
        haystack: &[u8],
        from: usize,
        until: usize,
        admits: &mut dyn FnMut(u16, usize) -> bool,
    ) -> Option<usize> {
        if from >= until || from > haystack.len() {
            return None;
        }
        // Only a long seek can make the search read far for a start here;
        // a set without one is spared the walk, and the search looks at
        // `from` itself.
        let mut next = from;
        if !self.long.is_empty() {
            if self
                .candidates_at(haystack, from, admits)
                .is_some_and(|candidates| !candidates.is_empty())
            {
                return Some(from);
            }
            next = from + 1;
            while next < haystack.len() && (haystack[next] & 0xC0) == 0x80 {
                next += 1;
            }
        }
        let span_end = haystack.len().min(until.saturating_add(self.settle));
        // An empty span at the end still holds the empty matches there.
        let found = if next <= span_end {
            #[cfg(test)]
            EARLIEST_CALLS.with(|calls| calls.set(calls.get() + 1));
            let input = Input::new(haystack).span(next..span_end);
            let Ok(found) = self.search.try_search(&mut self.search_cache, &input) else {
                // The lazy DFAs give up only on their cache's efficiency
                // (`MIN_CACHE_CLEARS`) and quit only on non-ASCII look-arounds,
                // which the seeks do not have: `from` is the earliest
                // position the search can name, and the set drops its
                // pre-filter after this search.
                self.gave_up = true;
                return Some(from);
            };
            #[cfg(test)]
            META_BYTES.with(|bytes| {
                // The forward scan to the match end, and the reverse scan
                // back to its start; the whole span without a match.
                let read = found.map_or(span_end - next, |m| {
                    (m.end() - next) + (m.end() - m.start())
                });
                bytes.set(bytes.get() + read as u64);
            });
            found.map(|m| m.start()).filter(|&at| at < until)
        } else {
            None
        };
        if self.long.is_empty() {
            return found;
        }
        let dispatch = self.long_dispatch;
        let limit = found.unwrap_or(until);
        let mut p = next;
        while p < limit {
            let admitted = haystack
                .get(p)
                .is_none_or(|&byte| (dispatch[byte as usize / 64] >> (byte % 64)) & 1 == 1);
            if admitted
                && self
                    .candidates_at(haystack, p, admits)
                    .is_some_and(|candidates| !candidates.is_empty())
            {
                return Some(p);
            }
            p += 1;
            while p < limit && p < haystack.len() && (haystack[p] & 0xC0) == 0x80 {
                p += 1;
            }
        }
        found
    }

    /// The covered entries whose seek matches at `at`, ascending, or `None`
    /// where none does: the patterns of every match state the overlapping
    /// DFA reaches from `at`, within `CANDIDATE_SCAN_BYTES`. The walk stops
    /// as soon as every covered entry has been recorded. Once the bounded
    /// seeks have settled (`settle`, right away while `ask_early`, after
    /// `LONG_WALK_BYTES` otherwise), it asks which entries of the long
    /// seeks the search attempts at `at` (`admits`, for those not recorded
    /// and dispatched on the byte at `at`), and stops as soon as every one
    /// of those has been recorded: the others never decide there, so
    /// nothing more can be learned. At the bound, or where the DFA gives
    /// up or quits, the recorded entries and the admissible long ones are
    /// the candidates, every covered one where nothing was asked.
    #[inline]
    pub(crate) fn candidates_at(
        &mut self,
        haystack: &[u8],
        at: usize,
        admits: &mut dyn FnMut(u16, usize) -> bool,
    ) -> Option<&[u16]> {
        self.candidates.clear();
        self.walked_at = Some(at);
        if self.candidate_memo.active {
            if let Some((memo, some)) = self.candidate_memo.get(at) {
                self.candidates.extend_from_slice(memo);
                return some.then_some(&self.candidates);
            }
        }
        self.patset.clear();
        let (end, asked) = self.scan_candidates(haystack, at, admits);
        self.ask_early = end == ScanEnd::Bound || (asked && end == ScanEnd::Settled);
        if end == ScanEnd::Finished && self.patset.is_empty() {
            if self.candidate_memo.active {
                self.candidate_memo.insert(at, &[], false);
            }
            return None;
        }
        let entries = &self.entries;
        if end == ScanEnd::Bound {
            let stamps = &self.admissible;
            let stamp = self.stamp;
            let patset = &self.patset;
            self.candidates.extend(
                entries
                    .iter()
                    .enumerate()
                    .filter(|&(pid, _)| {
                        !asked
                            || stamps[pid] == stamp
                            || patset.contains(PatternID::new_unchecked(pid))
                    })
                    .map(|(_, &index)| index),
            );
        } else {
            self.candidates
                .extend(self.patset.iter().map(|pid| entries[pid]));
        }
        if self.candidate_memo.active {
            self.candidate_memo.insert(at, &self.candidates, true);
        }
        Some(&self.candidates)
    }

    /// The candidates the last `candidates_at` found.
    #[inline]
    pub(crate) fn candidates(&self) -> &[u16] {
        &self.candidates
    }

    /// The position `candidates` holds the candidates of in the current
    /// search, so a walk the stretch search made at the position the loop
    /// goes on at is not made again.
    #[inline]
    pub(crate) fn walked_at(&self) -> Option<usize> {
        self.walked_at
    }

    /// Forgets the candidates of the previous search, and readies the
    /// walk memo for a search of `subject` from `start`.
    #[inline]
    pub(crate) fn begin_search(
        &mut self,
        subject: Option<(crate::regset::FallbackMemoIdentity, usize)>,
        start: usize,
    ) {
        self.walked_at = None;
        self.candidate_memo.begin(subject, start);
        self.walked = 0;
        self.dfa_cache.search_start(0);
    }

    /// The anchored walk of `candidates_at`, filling `patset`: how it ended,
    /// and whether it asked `admits` (then `admissible[pid] == stamp` marks
    /// the admissible long seeks). The bytes walked are recorded for the
    /// DFA's cache efficiency check (`MIN_CACHE_CLEARS`); a walk the DFA
    /// gave up on ends as at the bound and marks the set for retirement.
    fn scan_candidates(
        &mut self,
        haystack: &[u8],
        at: usize,
        admits: &mut dyn FnMut(u16, usize) -> bool,
    ) -> (ScanEnd, bool) {
        let Self {
            dfa,
            dfa_cache: cache,
            patset,
            entries,
            long,
            settle,
            ask_early,
            gave_up,
            walked,
            dispatch,
            admissible: stamps,
            stamp,
            ..
        } = self;
        let mut i = at;
        let (end, asked) = Self::walk(
            dfa, cache, patset, entries, long, *settle, *ask_early, dispatch, stamps, stamp,
            haystack, at, &mut i, admits,
        );
        *walked += i - at;
        cache.search_update(*walked);
        if end == ScanEnd::GaveUp {
            *gave_up = true;
            return (ScanEnd::Bound, asked);
        }
        (end, asked)
    }

    /// `scan_candidates` from `at`, leaving the position it read up to in
    /// `i`.
    #[allow(clippy::too_many_arguments)]
    fn walk(
        dfa: &hybrid::DFA,
        cache: &mut hybrid::Cache,
        patset: &mut PatternBits,
        entries: &[u16],
        long: &[u16],
        settle: usize,
        ask_early: bool,
        dispatch: &[[u64; 4]],
        stamps: &mut [u32],
        stamp: &mut u32,
        haystack: &[u8],
        at: usize,
        i: &mut usize,
        admits: &mut dyn FnMut(u16, usize) -> bool,
    ) -> (ScanEnd, bool) {
        let end = haystack.len();
        let input = Input::new(haystack).span(at..end).anchored(Anchored::Yes);
        let Ok(mut sid) = dfa.start_state_forward(cache, &input) else {
            return (ScanEnd::GaveUp, false);
        };
        let cut = end.min(at.saturating_add(CANDIDATE_SCAN_BYTES));
        // The position at which the walk asks about the long seeks.
        let ask_at = if ask_early {
            settle
        } else {
            settle.max(LONG_WALK_BYTES)
        };
        let ask_i = at.saturating_add(ask_at);
        // A match state stands for its patterns wherever the walk reaches
        // it; a run of word characters keeps the walk in one match state
        // for every identifier-like seek, which is recorded once.
        // The match state last recorded, with the cache's clear count: a
        // clear reassigns state IDs, so the same ID may name another state.
        let mut recorded: Option<(LazyStateID, usize)> = None;
        macro_rules! at_end {
            ($asked:expr) => {
                if let Ok(eoi) = dfa.next_eoi_state(cache, sid) {
                    if eoi.is_match() {
                        record_match(dfa, cache, patset, eoi);
                    }
                    return (ScanEnd::Finished, $asked);
                }
                return (ScanEnd::GaveUp, $asked);
            };
        }
        // Up to `ask_i`, the plain walk. All three byte limits are fixed
        // during the walk; compare their minimum once per byte, then retain
        // end-of-input's precedence over the scan bound at that boundary.
        let plain_end = cut.min(ask_i);
        loop {
            if sid.is_dead() {
                return (ScanEnd::Finished, false);
            }
            if sid.is_quit() {
                return (ScanEnd::Bound, false);
            }
            // A match state stands for the matches that ended before the
            // byte it was reached by (or at the end of the haystack).
            if sid.is_match() && recorded != Some((sid, cache.clear_count())) {
                recorded = Some((sid, cache.clear_count()));
                record_match(dfa, cache, patset, sid);
                if patset.is_full() {
                    return (ScanEnd::Settled, false);
                }
            }
            if *i >= plain_end {
                if *i == end {
                    at_end!(false);
                }
                if *i == cut {
                    return (ScanEnd::Bound, false);
                }
                break;
            }
            #[cfg(test)]
            SCAN_STEPS.with(|steps| steps.set(steps.get() + 1));
            sid = match dfa.next_state(cache, sid, haystack[*i]) {
                Ok(sid) => sid,
                Err(_) => return (ScanEnd::GaveUp, false),
            };
            *i += 1;
        }
        // The bounded seeks have settled: only an admissible long seek not
        // yet recorded can add a candidate.
        *stamp = stamp.wrapping_add(1);
        if *stamp == 0 {
            stamps.fill(0);
            *stamp = 1;
        }
        let byte = haystack[at];
        let mut needed = 0usize;
        for &pid in long.iter() {
            let bits = dispatch[pid as usize];
            if patset.contains(PatternID::new_unchecked(pid as usize))
                || (bits[byte as usize / 64] >> (byte % 64)) & 1 == 0
            {
                continue;
            }
            if admits(entries[pid as usize], at) {
                stamps[pid as usize] = *stamp;
                needed += 1;
            }
        }
        if needed == 0 {
            return (ScanEnd::Settled, true);
        }
        // On, until every admissible long seek has been recorded.
        loop {
            #[cfg(test)]
            SCAN_STEPS.with(|steps| steps.set(steps.get() + 1));
            sid = match dfa.next_state(cache, sid, haystack[*i]) {
                Ok(sid) => sid,
                Err(_) => return (ScanEnd::GaveUp, true),
            };
            *i += 1;
            if sid.is_dead() {
                return (ScanEnd::Finished, true);
            }
            if sid.is_quit() {
                return (ScanEnd::Bound, true);
            }
            if sid.is_match() && recorded != Some((sid, cache.clear_count())) {
                recorded = Some((sid, cache.clear_count()));
                needed -= record_needed(dfa, cache, patset, sid, stamps, *stamp);
                if needed == 0 || patset.is_full() {
                    return (ScanEnd::Settled, true);
                }
            }
            if *i == cut {
                if *i == end {
                    at_end!(true);
                }
                return (ScanEnd::Bound, true);
            }
        }
    }

    /// The overlapping DFA gave up on its cache during the last search
    /// (`MIN_CACHE_CLEARS`): the set should retire the pre-filter.
    pub(crate) fn gave_up(&self) -> bool {
        self.gave_up
    }

    /// NFA states of the overlapping DFA (the search DFAs hold about twice
    /// as many: a forward and a reverse NFA).
    pub(crate) fn nfa_states(&self) -> usize {
        self.dfa.get_nfa().states().len()
    }

    /// Heap memory of the automata (shared with every set over the same
    /// pattern list) and of this set's caches, in bytes.
    pub(crate) fn memory_usage(&self) -> usize {
        automata_memory_usage(&self.dfa, &self.search) + self.cache_memory_usage()
    }

    /// Heap memory of this set's lazy DFA caches, in bytes.
    pub(crate) fn cache_memory_usage(&self) -> usize {
        self.search_cache.memory_usage() + self.dfa_cache.memory_usage()
    }

    /// Times the overlapping DFA's cache was cleared because it filled up.
    pub(crate) fn cache_clears(&self) -> usize {
        self.dfa_cache.clear_count()
    }
}

/// Rust-only (ADR-008): the seek of one `Regex` as a lazy DFA, which answers
/// whether any seek match starts in a subject from a position on. A `Regex`
/// builds it with the pattern and searches with it from the first search
/// (`RegexBuilder::dfa`).
///
/// Without an exact seek, "no seek match" proves "no match" and nothing else:
/// a seek match leaves the answer to the matcher. An exact seek's match is a
/// match of the pattern (`SeekCode::exact`). An inexact seek may come with
/// an accept seek ([`accept`]), whose match is a match of the pattern, and
/// whose "no match" is the pattern's where its `AcceptRejects` says so. A
/// search that a lazy DFA gives up on answers nothing, and the matcher runs.
pub(crate) struct SingleSeek {
    dfa: hybrid::DFA,
    /// The accept seek's lazy DFA, and where its "no match" holds.
    accept: Option<(hybrid::DFA, accept::AcceptRejects)>,
    /// The lazy DFAs' caches, one pair per thread that searches (the pool's
    /// owner fast path), so a search takes no lock.
    caches: Pool<SingleCaches, CacheMaker>,
    exact: bool,
}

/// The caches of a `SingleSeek`'s lazy DFAs.
struct SingleCaches {
    seek: hybrid::Cache,
    accept: Option<hybrid::Cache>,
}

/// The cache constructor of a `SingleSeek`'s pool.
type CacheMaker = Box<dyn Fn() -> SingleCaches + Send + Sync>;

/// Lazy DFA cache of a single `Regex`'s seek, and of its accept seek.
const SINGLE_CACHE_CAPACITY: usize = MIB;

/// An estimate of the NFA states `hir` compiles to, from above: a repetition
/// counts its body once per copy its bound makes, and the parts add up.
/// Saturating, so a nested bound cannot overflow it.
fn nfa_states_estimate(hir: &Hir) -> usize {
    states_estimate(hir, false)
}

/// `nfa_states_estimate`, with `utf8_classes` counting a Unicode class as
/// the UTF-8 sequences it compiles to, a state per byte of each (the accept
/// seek's classes; the seek's own are bytes or a few members).
fn states_estimate(hir: &Hir, utf8_classes: bool) -> usize {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => 1,
        HirKind::Literal(literal) => literal.0.len() + 1,
        HirKind::Class(Class::Unicode(class)) if utf8_classes => {
            class.ranges().iter().fold(2usize, |sum, range| {
                Utf8Sequences::new(range.start(), range.end())
                    .fold(sum, |sum, sequence| sum.saturating_add(sequence.len()))
            })
        }
        HirKind::Class(_) => 2,
        HirKind::Capture(capture) => states_estimate(&capture.sub, utf8_classes) + 2,
        HirKind::Repetition(rep) => {
            let copies = rep.max.unwrap_or(rep.min).max(rep.min).max(1) as usize;
            states_estimate(&rep.sub, utf8_classes)
                .saturating_mul(copies)
                .saturating_add(2)
        }
        HirKind::Concat(parts) => parts.iter().fold(0usize, |sum, part| {
            sum.saturating_add(states_estimate(part, utf8_classes))
        }),
        HirKind::Alternation(branches) => branches.iter().fold(2usize, |sum, branch| {
            sum.saturating_add(states_estimate(branch, utf8_classes))
        }),
    }
}

impl SingleSeek {
    /// The lazy DFA of `seek`, or `None` for a seek that matches everywhere
    /// (it rejects nothing), one whose automaton would exceed
    /// `MAX_NFA_STATES`, or one that does not build. The lazy DFA of
    /// `accept`, the pattern's accept seek, is built beside it where it
    /// builds within the same bounds.
    pub(crate) fn build(seek: &Seek, accept: Option<&AcceptCode>) -> Option<Self> {
        let Seek::Pattern(code) = seek else {
            return None;
        };
        let dfa = single_dfa(&code.decode())?;
        let accept = accept.and_then(|accept| {
            let dfa = single_dfa(&accept.code.decode())?;
            Some((dfa, accept.rejects.clone()))
        });
        let make_seek = dfa.clone();
        let make_accept = accept.as_ref().map(|(dfa, _)| dfa.clone());
        let caches = Pool::new(Box::new(move || SingleCaches {
            seek: make_seek.create_cache(),
            accept: make_accept.as_ref().map(hybrid::DFA::create_cache),
        }) as CacheMaker);
        Some(SingleSeek {
            dfa,
            accept,
            caches,
            exact: code.exact(),
        })
    }

    /// Whether the seek matches somewhere in `text[start..]`, with the look
    /// around `start` read from `text`, or `None` when the lazy DFA gives up.
    /// The text must be valid UTF-8 and `start` a character boundary, as the
    /// matcher reads them.
    pub(crate) fn search(&self, text: &[u8], start: usize) -> Option<bool> {
        let input = Input::new(text).span(start..text.len()).earliest(true);
        let mut caches = self.caches.get();
        self.dfa
            .try_search_fwd(&mut caches.seek, &input)
            .ok()
            .map(|found| found.is_some())
    }

    /// Whether the seek is exact (`SeekCode::exact`).
    pub(crate) fn exact(&self) -> bool {
        self.exact
    }

    /// Whether the seek has an accept seek.
    pub(crate) fn has_accept(&self) -> bool {
        self.accept.is_some()
    }

    /// The accept seek's answer for a search of `text` from `start`, read as
    /// `search` reads it: `Some(true)` where an accept seek match starts in
    /// `text[start..]`, which is a match of the pattern; `Some(false)` where
    /// none does and the accept seek is exact on this text (its
    /// `AcceptRejects`); `None` otherwise, or when the lazy DFA gives up.
    pub(crate) fn accept_answer(&self, text: &[u8], start: usize) -> Option<bool> {
        let (dfa, rejects) = self.accept.as_ref()?;
        let input = Input::new(text).span(start..text.len()).earliest(true);
        let mut caches = self.caches.get();
        let found = dfa
            .try_search_fwd(caches.accept.as_mut()?, &input)
            .ok()?
            .is_some();
        if found {
            return Some(true);
        }
        match rejects {
            // A match starts at or after `start` and reads only from there.
            accept::AcceptRejects::Unless(triggers) => {
                (!triggers.occur_in(&text[start..])).then_some(false)
            }
            accept::AcceptRejects::Never => None,
        }
    }

    /// Heap memory of the lazy DFAs and of this thread's caches, in bytes.
    #[cfg(test)]
    pub(crate) fn memory_usage(&self) -> usize {
        let caches = self.caches.get();
        let accept = self
            .accept
            .as_ref()
            .map_or(0, |(dfa, _)| dfa.memory_usage());
        self.dfa.memory_usage()
            + accept
            + caches.seek.memory_usage()
            + caches
                .accept
                .as_ref()
                .map_or(0, hybrid::Cache::memory_usage)
    }
}

/// The lazy DFA of a single regex's seek or accept seek, or `None` for one
/// whose automaton would exceed `MAX_NFA_STATES` or that does not build.
fn single_dfa(hir: &Hir) -> Option<hybrid::DFA> {
    // Cut a seek that is clearly too large before compiling it: a compile
    // of a seek with nested counted repetitions can take far longer than
    // the automaton it would keep. (`derive_accept` estimated the accept
    // seek's Unicode classes already.)
    if nfa_states_estimate(hir) > MAX_NFA_STATES {
        return None;
    }
    // The seeks use ASCII look-arounds only, so the DFA never quits.
    let nfa = thompson::Compiler::new()
        .configure(
            thompson::Config::new()
                .utf8(false)
                .which_captures(thompson::WhichCaptures::None)
                .nfa_size_limit(Some(NFA_SIZE_LIMIT)),
        )
        .build_from_hir(hir)
        .ok()?;
    if nfa.states().len() > MAX_NFA_STATES {
        return None;
    }
    hybrid::Builder::new()
        .configure(
            hybrid::Config::new()
                .match_kind(MatchKind::LeftmostFirst)
                .cache_capacity(SINGLE_CACHE_CAPACITY),
        )
        .build_from_nfa(nfa)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encodings::utf8::ONIG_ENCODING_UTF8;
    use crate::oniguruma::{
        ONIG_OPTION_CAPTURE_GROUP, ONIG_OPTION_IGNORECASE, ONIG_OPTION_WORD_IS_ASCII,
        OnigOptionType,
    };
    use crate::regcomp::onig_new_for_scanner;
    use crate::regsyntax::OnigSyntaxOniguruma;
    use crate::scanner::{Scanner, ScannerConfig, ScannerFindOptions};

    fn seek_with(
        pattern: &str,
        options: OnigOptionType,
    ) -> Result<Option<Seek>, crate::error::RegexError> {
        onig_new_for_scanner(
            pattern.as_bytes(),
            options,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
            false,
            true,
        )
        .map(|(_, seek)| seek)
    }

    fn seek(pattern: &str) -> Option<Seek> {
        seek_with(pattern, ONIG_OPTION_CAPTURE_GROUP).unwrap()
    }

    /// The seek of `pattern` under `options` in regex syntax, or what it is
    /// instead of a pattern.
    fn seek_pattern_with(pattern: &str, options: OnigOptionType) -> String {
        match seek_with(pattern, options) {
            Ok(Some(Seek::Pattern(seek))) => seek.decode().to_string(),
            other => format!("{other:?}"),
        }
    }

    fn seek_pattern(pattern: &str) -> String {
        seek_pattern_with(pattern, ONIG_OPTION_CAPTURE_GROUP)
    }

    /// Whether the seek of `pattern` under `options` is exact, as a single
    /// regex's seek is derived (`onig_new_for_single_seek`).
    fn exact_with(pattern: &str, options: OnigOptionType) -> Option<bool> {
        let seek = crate::regcomp::onig_new_for_single_seek(
            pattern.as_bytes(),
            options,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
            false,
            true,
        )
        .unwrap()
        .1;
        match seek {
            Some(Seek::Pattern(code)) => Some(code.exact()),
            _ => None,
        }
    }

    /// The constructs whose seek is exact, and those whose seek approximates
    /// the matcher (for existence): each `SeekCode::exact` is pinned.
    #[test]
    fn seeks_are_exact_exactly_where_the_matcher_reads_alike() {
        let exact = [
            "abc",
            "[a-c]",
            "a|bc",
            "ab{2,5}c",
            "[^a]",
            ".",
            "\u{3042}+",
            "\\x{1F600}",
            "x|$",
            "a$",
            "\\Aab\\z",
            "a\\Kb",
            "^a",
            "^[0-9]{8}$",
            "(?:^a|^b)c",
            "(?i)[0-9a-f]+",
            "(?i)k",
            "(?i)s",
            "[\u{e9}]",
            "(a)(b)",
            // Repeats that `tune_next` makes possessive: their bags keep the
            // language of the repeat, so the seek is exact.
            "^[a-z]+@[a-z]+\\.com$",
            "[a-z]*@",
            "[^\"]*\"",
            "(?:ab)*c",
            "([a-z]+)@([a-z]+)",
        ];
        let inexact = [
            "(?>abc|ab)c",
            "(?>a+)b",
            "a++b",
            "a*+b",
            "[a-z]++@",
            // A user-written atomic group stays inexact beside an automatic one.
            "^[a-z]+@(?>[a-z]+)\\.com$",
            "(?>[a-z]+)@",
            "\\b",
            "\\B",
            "(?=a)b",
            "(?<=a)b",
            "a(?!b)",
            "a(?<!b)",
            "\\Zb",
            "^",
            "^$",
            "(a)\\1",
            "\\W",
            "\\w",
            "\\d+",
            "[^\u{e9}]",
            "\\G\\w",
            "(?:ab|cd){100}",
            "(x)?(?(1)a|b)",
        ];
        for pattern in exact {
            assert_eq!(
                exact_with(pattern, ONIG_OPTION_CAPTURE_GROUP),
                Some(true),
                "{pattern}"
            );
        }
        for pattern in inexact {
            assert_eq!(
                exact_with(pattern, ONIG_OPTION_CAPTURE_GROUP),
                Some(false),
                "{pattern}"
            );
        }
        // ASCII word characters: the repeat before `:` is possessive and exact.
        let ascii_word = ONIG_OPTION_CAPTURE_GROUP | crate::oniguruma::ONIG_OPTION_WORD_IS_ASCII;
        assert_eq!(exact_with("\\w+:", ascii_word), Some(true));
        assert_eq!(exact_with("\\w+:", ONIG_OPTION_CAPTURE_GROUP), Some(false));
        // Without multi-line anchors `$` is `\Z`, which is exact at the end of
        // the pattern (where it is `(?:\n)?\z`) and approximated elsewhere.
        let singleline = ONIG_OPTION_CAPTURE_GROUP | crate::oniguruma::ONIG_OPTION_SINGLELINE;
        assert_eq!(exact_with("^a$", singleline), Some(true));
        assert_eq!(exact_with("a$|b$", singleline), Some(true));
        assert_eq!(exact_with("(?:a|b)\\Z", singleline), Some(true));
        assert_eq!(exact_with("a$b", singleline), Some(false));
        assert_eq!(exact_with("a$", ONIG_OPTION_CAPTURE_GROUP), Some(true));
        // The absent operator's range cutter has no seek of its own here, so
        // the pattern is searched on its own (no exactness to speak of).
        assert_eq!(exact_with("(?~abc)x", ONIG_OPTION_CAPTURE_GROUP), None);
        // `(*FAIL)` is a callout, which leaves the pattern without a seek.
        assert_eq!(exact_with("(*FAIL)|a", ONIG_OPTION_CAPTURE_GROUP), None);
        // Options that change what a line start, a line end or a match is.
        assert_eq!(exact_with("^a", ONIG_OPTION_NOTBOL), Some(false));
        assert_eq!(exact_with("a$", ONIG_OPTION_NOTEOL), Some(false));
        assert_eq!(exact_with("a", ONIG_OPTION_FIND_NOT_EMPTY), Some(false));
    }

    /// Each construct as the seek writes it. A seek matches wherever the
    /// pattern can match; the approximations are the documented ones.
    #[test]
    fn seeks_approximate_each_construct() {
        let cases: &[(&str, &str)] = &[
            // Exact: literals, classes, concatenation, alternation,
            // repetition; groups are transparent.
            ("abc", "(?:abc)"),
            ("[a-c]", "(?-u:[a-c])"),
            ("a|bc", "(?:a|(?:bc))"),
            ("ab{2,5}c", "(?:ab{2,5}c)"),
            // Greediness decides which match is preferred, not where one
            // exists, so every repetition is written greedy.
            ("(?:ab)+?", "(?:ab)+"),
            ("(a)(?:b)(?>c)d?", "(?:(?:abc)d?)"),
            ("(?:a|bc){70}", "(?:a|(?:bc)){64,}"),
            // A repetition of a repetition is flattened to a superset:
            // regex-syntax would print `(?:ab)+?`, a lazy plus.
            ("(?:(?:ab)+)?c", "(?:(?:ab)*c)"),
            ("(?:(?:ab){2,3}){2}", "(?:ab){4,6}"),
            ("(?:a*)+b", "(?:a*b)"),
            (
                "[^x]{2,70}",
                "(?:(?-u:[\\x00-wy-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3})){2,}",
            ),
            // Classes are byte-based: ASCII exact, non-ASCII as one
            // UTF-8 sequence.
            ("[é]", "(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3})"),
            (
                "[^a]",
                "(?:(?-u:[\\x00-`b-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))",
            ),
            (
                "\\w",
                "(?:(?-u:[0-9A-Z_a-z])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))",
            ),
            // Any character is an ASCII byte or one UTF-8 sequence.
            (
                ".",
                "(?:(?-u:[\\x00-\\x09\\x0B-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))",
            ),
            (
                "(?m:.)",
                "(?:(?-u:[\\x00-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))",
            ),
            // Anchors.
            ("\\Aa", "(?:\\Aa)"),
            ("a\\z", "(?:a\\z)"),
            ("a\\Z", "(?:a(?m:$))"),
            ("^a$", "(?:(?m:^)a(?m:$))"),
            (
                "\\ba",
                "(?:(?:(?-u:\\b)|(?-u:\\b{start-half})|(?-u:\\b{end-half}))a)",
            ),
            // Empty: look-behinds, negative look-aheads, `\G`, `\K`, `\y`.
            ("(?<=x)a", "a"),
            ("(?<!x)a", "a"),
            // A negative look at a class with every ASCII word character
            // is an ASCII half boundary.
            ("(?<!\\w)a", "(?:(?-u:\\b{start-half})a)"),
            (
                "(?<![-\\w])a(?![-\\w])",
                "(?:(?-u:\\b{start-half})a(?-u:\\b{end-half}))",
            ),
            ("(?<!\\W)a", "a"),
            ("(?<![^\\w])a", "a"),
            ("(?=ab)(?<!\\w)", "(?-u:\\b{start-half})"),
            ("(?!x)a", "a"),
            ("\\Ga", "a"),
            ("a\\Kb", "(?:ab)"),
            ("\\ya", "a"),
            // A positive look-ahead keeps its body where nothing consuming
            // follows it, and is dropped otherwise.
            (
                "\\s+(?=use\\b)",
                "(?:(?:(?-u:[\\x09-\\x0D\\x20])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))+(?:use)(?:(?-u:\\b)|(?-u:\\b{start-half})|(?-u:\\b{end-half})))",
            ),
            ("(?=a)(?<=b)", "a"),
            ("(?=a)b", "b"),
            ("(?=a)(?=b)", "a"),
            ("y(?:x(?=a))?", "(?:y(?:xa)?)"),
            ("y(?:x(?=a))+", "(?:yx+)"),
            ("(?:x(?=a))?y", "(?:x?y)"),
            // Anything: back references and calls.
            (
                "(a)\\1",
                "(?:a(?:(?-u:[\\x00-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))*)",
            ),
            (
                "a\\g<1>|(b)",
                "(?:(?:a(?:(?-u:[\\x00-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))*)|b)",
            ),
            // Both branches of a conditional.
            ("(a)?(?(1)b|c)", "(?:a?[bc])"),
            ("(?(a)b|c)", "(?:(?:ab)|c)"),
            // The matcher reads a class's bits for single-byte characters
            // and its code ranges for multibyte ones; nested negations leave
            // entries on the other side (a code range for the space here).
            (
                "[^[^[^İ]\\S]]",
                "(?:(?-u:[\\x00-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))",
            ),
            ("[^[^İ]\\S]", "[a&&b]"),
            (
                "\\W",
                "(?:(?-u:[\\x00-/:-@\\[-\\^`\\{-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))",
            ),
            // Case-insensitive strings as the tuner unravels them.
            ("(?i)ab", "(?:(?-u:[Aa])(?-u:[Bb]))"),
            (
                "(?i)[k]",
                "(?:(?-u:[Kk])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))",
            ),
            // Literal tries: the literals, or their ASCII case variants
            // with an escape for the non-ASCII folds the trie accepts.
            (
                "(?:error|warn|fatal|panic)",
                "(?:(?:error)|(?:warn)|(?:fatal)|(?:panic))",
            ),
            (
                "(?i)(?:ab|cd|ef|gh)",
                "(?:(?:(?-u:[Aa])(?-u:[Bb]))|(?:(?-u:[Cc])(?-u:[Dd]))|(?:(?-u:[Ee])(?-u:[Ff]))|(?:(?-u:[Gg])(?-u:[Hh]))|(?:(?-u:[\\x00-\\x7F])?[KSks\u{17f}\u{212a}](?:(?-u:[\\x00-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))?))",
            ),
            (
                "(?i)(?:kb|ss|st|xy)",
                "(?:(?:(?-u:[Kk])(?-u:[Bb]))|(?:(?-u:[Ss])(?-u:[Ss]))|(?:(?-u:[Ss])(?-u:[Tt]))|(?:(?-u:[Xx])(?-u:[Yy]))|(?:(?-u:[\\x00-\\x7F])?[KSks\u{df}\u{17f}\u{1e9e}\u{212a}\u{fb05}\u{fb06}\u{1df95}](?:(?-u:[\\x00-\\x7F])|(?:(?-u:[\\xC2-\\xF4])(?-u:[\\x80-\\xBF]){1,3}))?))",
            ),
        ];
        let mismatches: Vec<String> = cases
            .iter()
            .filter(|(pattern, expected)| seek_pattern(pattern) != *expected)
            .map(|(pattern, _)| format!("{pattern:?} => {:?}", seek_pattern(pattern)))
            .collect();
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
        // ASCII word characters and boundaries.
        let ascii_word = ONIG_OPTION_CAPTURE_GROUP | ONIG_OPTION_WORD_IS_ASCII;
        assert_eq!(seek_pattern_with(r"\w", ascii_word), "(?-u:[0-9A-Z_a-z])");
        assert_eq!(seek_pattern_with(r"\ba", ascii_word), "(?:(?-u:\\b)a)");
    }

    /// A seek that matches at every position narrows nothing.
    #[test]
    fn seeks_that_match_everywhere_leave_the_entry_on_its_own() {
        for pattern in [
            "",
            r"(?<=\))",
            r"(?!\s*\[)",
            "a*",
            r"(a)?\1",
            r"\G ?",
            r"(?<=}|%>)\s*",
            r"(?i)x*",
            // The absent operator reads any characters first.
            r"(?~abc)d",
        ] {
            assert_eq!(seek(pattern), Some(Seek::Everywhere), "{pattern}");
        }
        // Nullable with a look assertion: the assertion narrows.
        assert_eq!(seek_pattern("^"), "(?m:^)");
        assert_eq!(
            seek_pattern(r"\b"),
            "(?:(?-u:\\b)|(?-u:\\b{start-half})|(?-u:\\b{end-half}))"
        );
    }

    /// The seek code of every distinct pattern of the captured grammars
    /// decodes to the HIR it was made from (`derive` asserts that in debug
    /// builds as well), encodes to the same code again, and holds a few
    /// hundred bytes where the HIR held tens of KiB.
    #[test]
    fn seek_codes_round_trip_over_the_captured_grammars() {
        let mut patterns = std::collections::BTreeSet::new();
        for name in ["cpp", "java", "scss", "c", "php"] {
            let path = format!(
                "{}/benches/{name}_scanner/trace.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let trace: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            for set in trace["scanners"].as_array().unwrap() {
                for pattern in set.as_array().unwrap() {
                    patterns.insert(pattern.as_str().unwrap().to_owned());
                }
            }
        }
        let mut codes = 0usize;
        let mut bytes = 0usize;
        for pattern in &patterns {
            let Some(Seek::Pattern(code)) = seek(pattern) else {
                continue;
            };
            let hir = code.decode();
            let again = SeekCode::encode(&hir, code.exact()).expect("the walk's nodes encode");
            assert_eq!(again, code, "{pattern}");
            assert_eq!(again.decode(), hir, "{pattern}");
            codes += 1;
            bytes += code.len();
        }
        assert!(
            codes > 600,
            "{codes} seeks over {} patterns",
            patterns.len()
        );
        assert!(bytes / codes < 1024, "{bytes} bytes over {codes} seeks");
    }

    /// The `match` and `begin` patterns of a TextMate grammar JSON, as
    /// `benches/grammar_loader.rs` extracts them for the flat scanners of
    /// `battle_bench`, those the scanner compiles.
    fn flat_grammar_patterns(json: &str) -> Vec<String> {
        fn collect(value: &serde_json::Value, out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for key in ["match", "begin"] {
                        if let Some(pattern) = map.get(key).and_then(|v| v.as_str()) {
                            if !out.iter().any(|p| p == pattern) {
                                out.push(pattern.to_owned());
                            }
                        }
                    }
                    for (_, value) in map {
                        collect(value, out);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        collect(item, out);
                    }
                }
                _ => {}
            }
        }
        let root: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut patterns = Vec::new();
        collect(&root, &mut patterns);
        patterns
            .into_iter()
            .filter(|pattern| Scanner::new(&[pattern.as_str()]).is_ok())
            .collect()
    }

    fn tokenize(
        scanner: &mut Scanner,
        line: &crate::scanner::OnigString,
    ) -> Vec<(usize, usize, usize)> {
        let len = line.utf16_len();
        let mut pos = 0;
        let mut tokens = Vec::new();
        while pos < len {
            let Some(m) = scanner.find_next_match_utf16(line, pos, ScannerFindOptions::NONE) else {
                break;
            };
            let end = m.captures()[0].end;
            tokens.push((m.index, m.captures()[0].start, end));
            pos = if end > pos { end } else { pos + 1 };
        }
        tokens
    }

    /// A set whose overlapping DFA cannot reuse its cache retires the
    /// pre-filter and answers as before: the flat scanner over every
    /// pattern of the CSS grammar, whose 116 case-insensitive property
    /// lists in one automaton make every DFA state hold thousands of NFA
    /// states, tokenizing a stylesheet where a `\G`-anchored zero-width
    /// entry wins at every position (`battle_bench`, see ADR-008).
    #[test]
    fn a_set_whose_cache_thrashes_retires_the_prefilter() {
        let _limits = crate::regexec::shared_limits();
        let json = std::fs::read_to_string(format!(
            "{}/benches/grammars/css.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let patterns = flat_grammar_patterns(&json);
        let refs: Vec<&str> = patterns.iter().map(String::as_str).collect();
        let mut filtered =
            Scanner::with_config(&refs, &ScannerConfig::default().prefilter_warmup(0)).unwrap();
        let mut plain =
            Scanner::with_config(&refs, &ScannerConfig::default().prefilter(false)).unwrap();
        let stylesheet = ".navbar-primary > .nav-item:first-child {\n\
            background-color: oklch(0.65 0.15 250);\n\
            font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;\n\
            transition: all 150ms cubic-bezier(0.4, 0, 0.2, 1);\n\
            --custom-property: var(--color-primary, #3b82f6);\n\
            @media (min-width: 768px) and (prefers-color-scheme: dark) {\n}\n";
        let line = crate::scanner::OnigString::new(stylesheet);
        let stats = filtered.prefilter_stats();
        assert!(stats.covered > 100 && !stats.retired, "{stats:?}");
        let want = tokenize(&mut plain, &line);
        let got = tokenize(&mut filtered, &line);
        assert_eq!(got, want);
        assert!(got.len() > 200);
        let stats = filtered.prefilter_stats();
        assert!(
            stats.retired && !stats.built && stats.own == refs.len(),
            "{stats:?}"
        );
        // Retired for good: the next pass answers alike without automata.
        assert_eq!(tokenize(&mut filtered, &line), want);
        assert!(filtered.prefilter_stats().retired);
    }

    /// A subject tokenized again reuses the walks (`CandidateMemo`): the
    /// first pass over a string records nothing and the second, which
    /// starts over, records, so the third walks a fraction of the steps; a
    /// string of the same content with another identity walks them all,
    /// and every pass answers alike.
    #[test]
    fn walks_of_a_subject_searched_again_are_kept() {
        let _limits = crate::regexec::shared_limits();
        let patterns = [r"[a-z]+:", r"\d+", r"[a-z]+\(", "b", "a"];
        let mut filtered =
            Scanner::with_config(&patterns, &ScannerConfig::default().prefilter_warmup(0)).unwrap();
        let mut plain =
            Scanner::with_config(&patterns, &ScannerConfig::default().prefilter(false)).unwrap();
        let text = "abc: 12 foo(bar) a b xyz: 7 quux(1) ba";
        let line = crate::scanner::OnigString::new(text);
        let steps = || SCAN_STEPS.with(|c| c.get());
        let want = tokenize(&mut plain, &line);
        let before = steps();
        assert_eq!(tokenize(&mut filtered, &line), want);
        let first = steps() - before;
        assert!(first > 0);
        // The second pass starts over: it records, and walks as the first.
        let before = steps();
        assert_eq!(tokenize(&mut filtered, &line), want);
        assert_eq!(steps() - before, first);
        let before = steps();
        assert_eq!(tokenize(&mut filtered, &line), want);
        let third = steps() - before;
        assert!(third < first / 4, "{third} steps after {first}");
        let before = steps();
        assert_eq!(tokenize(&mut filtered, &line), want);
        assert_eq!(steps() - before, third);
        // Another identity with the same content walks everything again.
        let other = crate::scanner::OnigString::new(text);
        let before = steps();
        assert_eq!(tokenize(&mut filtered, &other), want);
        assert_eq!(steps() - before, first);
    }

    /// Patterns compiled for a scanner without the pre-filter, with
    /// callouts, or under another encoding have no seek.
    #[test]
    fn no_seek_without_the_prefilter_or_with_callouts() {
        let plain = onig_new_for_scanner(
            b"abc",
            ONIG_OPTION_CAPTURE_GROUP,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
            false,
            false,
        )
        .unwrap();
        assert_eq!(plain.1, None);
        // `(*FAIL)` and `(*MAX{n})` are built-in callouts.
        assert_eq!(seek("(*FAIL)"), None);
        assert_eq!(seek("(*MAX{1})a"), None);
        assert_eq!(
            seek_pattern_with("abc", ONIG_OPTION_IGNORECASE),
            "(?:(?-u:[Aa])(?-u:[Bb])(?-u:[Cc]))"
        );
        let ascii = onig_new_for_scanner(
            b"abc",
            ONIG_OPTION_CAPTURE_GROUP,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII,
            &OnigSyntaxOniguruma,
            false,
            true,
        )
        .unwrap();
        assert_eq!(ascii.1, None);
    }

    /// A set whose automata would exceed `MAX_NFA_STATES` keeps the
    /// position-lead search, and answers alike.
    #[test]
    fn sets_over_the_state_cutoff_build_no_automata() {
        let _limits = crate::regexec::shared_limits();
        // 16 branches of 64 class states each, per pattern.
        let branches: Vec<String> = (0..16)
            .map(|i| format!("[{}{}]{{64}}{i:x}", (b'a' + i) as char, (b'A' + i) as char))
            .collect();
        let pattern = format!("(?:{})", branches.join("|"));
        let patterns: Vec<&str> = std::iter::repeat_n(pattern.as_str(), 70)
            .chain(["2"])
            .collect();
        let text = format!("1 {}0 2", "a".repeat(64));
        let config = ScannerConfig::default().prefilter_warmup(0);
        let mut big = Scanner::with_config(&patterns, &config).unwrap();
        let mut little = Scanner::with_config(&patterns[69..], &config).unwrap();
        // The automata are built by the first search.
        for scanner in [&big, &little] {
            let stats = scanner.prefilter_stats();
            assert!(!stats.built && stats.nfa_states == 0, "{stats:?}");
        }
        let found = |scanner: &mut Scanner, start: usize| {
            scanner
                .find_next_match(&text, start, ScannerFindOptions::NONE)
                .map(|m| (m.index, m.captures()[0].start, m.captures()[0].end))
        };
        assert_eq!(found(&mut big, 0), Some((0, 2, 67)));
        assert_eq!(found(&mut big, 3), Some((70, 68, 69)));
        assert_eq!(found(&mut little, 0), Some((0, 2, 67)));
        assert_eq!(found(&mut little, 3), Some((1, 68, 69)));
        let stats = big.prefilter_stats();
        assert!(
            !stats.built && stats.covered == 0 && stats.own == 71,
            "{stats:?}"
        );
        let stats = little.prefilter_stats();
        assert!(stats.built && stats.covered == 2 && stats.nfa_states < MAX_NFA_STATES);
    }

    /// Sets the overlapping DFA's cache capacity on this thread while it
    /// lives: the automata the thread builds then clear that cache as often
    /// as the capacity needs (`overlapping_config`).
    struct OverlappingCacheBytes;

    impl OverlappingCacheBytes {
        fn set(bytes: usize) -> Self {
            OVERLAPPING_CACHE_BYTES.with(|cell| cell.set(Some(bytes)));
            Self
        }
    }

    impl Drop for OverlappingCacheBytes {
        fn drop(&mut self) {
            OVERLAPPING_CACHE_BYTES.with(|cell| cell.set(None));
        }
    }

    /// A walk that clears the overlapping DFA's cache can reach a match
    /// state that takes the ID of the match state it recorded before the
    /// clear. Keyed by the ID alone, the walk took that state as already
    /// recorded and dropped its patterns: here the `zz` at 4 was lost to
    /// two `z` candidates, where the scanner without the pre-filter answers
    /// `zz`. The 2 KiB cache is cleared twice by the search of this subject,
    /// and the give-up, which would retire the pre-filter first, is off.
    #[test]
    fn a_walk_that_clears_the_cache_keeps_the_state_it_reaches() {
        let _limits = crate::regexec::shared_limits();
        let patterns = ["zz", "z", r"z\w*", "q:"];
        let _cache = OverlappingCacheBytes::set(2048);
        let mut filtered =
            Scanner::with_config(&patterns, &ScannerConfig::default().prefilter_warmup(0)).unwrap();
        let mut plain =
            Scanner::with_config(&patterns, &ScannerConfig::default().prefilter(false)).unwrap();
        let line = crate::scanner::OnigString::new("zqzzzz(");
        let want = tokenize(&mut plain, &line);
        let got = tokenize(&mut filtered, &line);
        let stats = filtered.prefilter_stats();
        assert!(stats.built && stats.cache_clears > 0, "{stats:?}");
        assert_eq!(want, [(1, 0, 1), (0, 2, 4), (0, 4, 6)]);
        assert_eq!(got, want);
    }
}
