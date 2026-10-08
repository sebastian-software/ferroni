//! Spike, not for merge (refs #252): a DFA candidate pre-filter for the
//! RegSet position-lead search, after fancy-regex's `RegexSet`.
//!
//! [`derive`] reads a pattern's tuned parse tree and writes an
//! over-approximation of it as regex-syntax HIR: wherever the pattern can
//! match, the HIR matches too (it may match in more places, never fewer).
//! Literals, classes, concatenation, alternation and repetition are exact;
//! groups are transparent; look-behind, negative look-ahead, `\G`, `\K` and
//! text-segment boundaries become empty; back references and calls become
//! `(?s:.)*`; a positive look-ahead keeps its body where nothing consuming
//! follows it and is dropped otherwise.
//!
//! [`SetPrefilter`] compiles the HIRs of one set into a regex-automata meta
//! regex, which finds the earliest position where any pattern of the set can
//! match, and a lazy DFA, which says which patterns can match there. The
//! RegSet (`regset_search_body_prefilter`) runs the VM only for those, in
//! index order, anchored at that position. Patterns whose HIR matches at
//! every position (`Seek::always`) are searched on their own with the
//! ordinary per-entry search instead.
//!
//! Switches: the `dfa-prefilter` feature compiles this module; the
//! environment variable `FERRONI_DFA_PREFILTER=0` turns it off at run time
//! (read once), and `FERRONI_DFA_PREFILTER_WB=0` drops word boundaries from
//! the HIR instead of keeping them.

use crate::oniguruma::{ONIGENC_CTYPE_WORD, OnigCodePoint};
use crate::regcomp::literal_alt_trie_index;
use crate::regenc::OnigEncoding;
use crate::regint::*;
use crate::regparse_types::*;
use regex_automata::hybrid::dfa as hybrid;
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;
use regex_automata::{Anchored, Input, MatchErrorKind, MatchKind, PatternID, PatternSet};
use regex_syntax::hir::{
    Class, ClassBytes, ClassBytesRange, ClassUnicode, ClassUnicodeRange, Dot, Hir, HirKind, Look,
    Repetition,
};
use std::sync::OnceLock;

const MIB: usize = 1 << 20;
/// Lazy DFA cache of the meta regex (fancy-regex: 64 MiB).
const META_CACHE_CAPACITY: usize = 2 * MIB;
/// Lazy DFA cache of the overlapping DFA (fancy-regex: 64 MiB).
const OVERLAPPING_CACHE_CAPACITY: usize = MIB;
/// NFA size limit per set, for both automata.
const NFA_SIZE_LIMIT: usize = 16 * MIB;
/// Counted repetitions above this bound become unbounded ones.
const MAX_COUNTED_REPEAT: u32 = 64;

/// Whether the pre-filter is used at all (`FERRONI_DFA_PREFILTER`).
pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        !matches!(
            std::env::var("FERRONI_DFA_PREFILTER").as_deref(),
            Ok("0") | Ok("off") | Ok("false") | Ok("no")
        )
    })
}

/// Whether word boundaries are kept in the HIR (`FERRONI_DFA_PREFILTER_WB`).
fn keep_word_boundaries() -> bool {
    static KEEP: OnceLock<bool> = OnceLock::new();
    *KEEP.get_or_init(|| {
        !matches!(
            std::env::var("FERRONI_DFA_PREFILTER_WB").as_deref(),
            Ok("0") | Ok("off") | Ok("false") | Ok("no")
        )
    })
}

/// Whether classes stay exact Unicode code point sets
/// (`FERRONI_DFA_PREFILTER_UNICODE=1`). By default the HIR is byte-based:
/// the ASCII part of a class is exact and any non-ASCII character is
/// approximated by one UTF-8 sequence, which keeps the automata small.
fn unicode_classes() -> bool {
    static UNICODE: OnceLock<bool> = OnceLock::new();
    *UNICODE.get_or_init(|| {
        matches!(
            std::env::var("FERRONI_DFA_PREFILTER_UNICODE").as_deref(),
            Ok("1") | Ok("on") | Ok("true") | Ok("yes")
        )
    })
}

/// What the HIR approximates, as flags, for the census.
pub mod approx {
    /// A positive look-ahead whose body was kept in place.
    pub const LOOKAHEAD_KEPT: u32 = 1 << 0;
    /// A positive look-ahead dropped (something consuming follows it).
    pub const LOOKAHEAD_DROPPED: u32 = 1 << 1;
    /// A negative look-ahead (dropped).
    pub const NEGATIVE_LOOKAHEAD: u32 = 1 << 2;
    /// A look-behind, positive or negative (dropped).
    pub const LOOKBEHIND: u32 = 1 << 3;
    /// A back reference (`(?s:.)*`).
    pub const BACKREF: u32 = 1 << 4;
    /// A subexpression call (`(?s:.)*`).
    pub const CALL: u32 = 1 << 5;
    /// A conditional (both branches as an alternation).
    pub const CONDITIONAL: u32 = 1 << 6;
    /// `\G` (dropped).
    pub const BEGIN_POSITION: u32 = 1 << 7;
    /// `\y` / `\Y` (dropped).
    pub const TEXT_SEGMENT: u32 = 1 << 8;
    /// `\Z` (as `(?m:$)`).
    pub const SEMI_END_BUF: u32 = 1 << 9;
    /// A ctype other than `\w` or `.` (one code point).
    pub const OTHER_CTYPE: u32 = 1 << 10;
    /// A class with bits at or above 0x80 (one code point).
    pub const CLASS_HIGH_BITS: u32 = 1 << 11;
    /// A case-insensitive string the tuner left unraveled (`(?s:.)*`).
    pub const IGNORECASE_STRING: u32 = 1 << 12;
    /// A folded literal trie (rebuilt from its case-fold data).
    pub const FOLDED_TRIE: u32 = 1 << 13;
    /// A counted repetition above `MAX_COUNTED_REPEAT` (unbounded).
    pub const REPEAT_CAPPED: u32 = 1 << 14;
    /// Word boundaries dropped (`FERRONI_DFA_PREFILTER_WB=0`).
    pub const WORD_BOUNDARY_DROPPED: u32 = 1 << 15;
    /// A gimmick other than `(*FAIL)`: `\K`, absent-operator bookkeeping,
    /// callouts (dropped).
    pub const GIMMICK: u32 = 1 << 16;

    /// Names of the flags, in bit order.
    pub const NAMES: [&str; 17] = [
        "lookahead kept",
        "lookahead dropped",
        "negative lookahead",
        "lookbehind",
        "backref",
        "call",
        "conditional",
        "\\G",
        "text segment",
        "\\Z",
        "other ctype",
        "class high bits",
        "ignorecase string",
        "folded trie",
        "repeat capped",
        "word boundary dropped",
        "gimmick",
    ];
}

/// The seek approximation of one compiled pattern.
pub struct Seek {
    /// The over-approximation.
    pub hir: Hir,
    /// The HIR matches at every position, so it cannot narrow anything: the
    /// entry is searched on its own.
    pub always: bool,
    /// `approx` flags of what the HIR approximates.
    pub approximated: u32,
}

impl std::fmt::Debug for Seek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seek")
            .field("hir", &self.hir.to_string())
            .field("always", &self.always)
            .field("approximated", &self.approximated)
            .finish()
    }
}

/// The seek approximation of `reg`, whose tuned parse tree is `root`.
/// `None` for an entry the pre-filter must not decide for: callouts observe
/// every attempt, and the HIR models UTF-8 code points.
pub(crate) fn derive(root: &Node, reg: &RegexType) -> Option<Seek> {
    if reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0) {
        return None;
    }
    // Trait-object pointers may differ in their vtable between codegen
    // units, so the encoding is identified by name.
    let utf8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
    if reg.enc.name() != utf8.name() || reg.enc.max_enc_len() != utf8.max_enc_len() {
        return None;
    }
    let mut walk = Walk {
        reg,
        approximated: 0,
        lookaheads_kept: 0,
    };
    let hir = walk.item(root, true);
    let always = matches_everywhere(&hir);
    Some(Seek {
        hir,
        always,
        approximated: walk.approximated,
    })
}

/// One character, newline included.
fn any_char() -> Hir {
    Hir::dot(if unicode_classes() {
        Dot::AnyChar
    } else {
        Dot::AnyByte
    })
}

/// `(?s:.)*`, for a node the HIR cannot express.
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
        HirKind::Repetition(rep) if rep.min == 0 && rep.max.is_none() => match rep.sub.kind() {
            HirKind::Class(Class::Unicode(class)) => {
                let ranges = class.ranges();
                ranges.len() == 1 && ranges[0].start() == '\0' && ranges[0].end() == '\u{10FFFF}'
            }
            HirKind::Class(Class::Bytes(class)) => {
                let ranges = class.ranges();
                ranges.len() == 1 && ranges[0].start() == 0 && ranges[0].end() == 0xFF
            }
            _ => false,
        },
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

/// A class over code point ranges (inclusive), negated or not.
fn class_hir(ranges: impl IntoIterator<Item = (u32, u32)>, negate: bool) -> Hir {
    let mut out = Vec::new();
    for (lo, hi) in ranges {
        let hi = hi.min(0x10FFFF);
        if lo > hi {
            continue;
        }
        // Surrogates are not scalar values; valid UTF-8 never holds them.
        for (a, b) in [(lo, hi.min(0xD7FF)), (lo.max(0xE000), hi)] {
            if a <= b {
                if let (Some(a), Some(b)) = (char::from_u32(a), char::from_u32(b)) {
                    out.push(ClassUnicodeRange::new(a, b));
                }
            }
        }
    }
    let mut class = ClassUnicode::new(out);
    if negate {
        class.negate();
    }
    if unicode_classes() {
        return Hir::class(Class::Unicode(class));
    }
    // Byte mode: the ASCII members exactly, and one UTF-8 sequence for any
    // non-ASCII member.
    let ascii = ClassBytes::new(class.ranges().iter().filter_map(|range| {
        let lo = range.start() as u32;
        let hi = (range.end() as u32).min(0x7F);
        (lo <= hi).then(|| ClassBytesRange::new(lo as u8, hi as u8))
    }));
    let non_ascii = class
        .ranges()
        .iter()
        .any(|range| range.end() as u32 >= 0x80);
    let ascii_hir = Hir::class(Class::Bytes(ascii));
    if non_ascii {
        Hir::alternation(vec![ascii_hir, non_ascii_char()])
    } else {
        ascii_hir
    }
}

/// The `(from, to)` pairs of a code range buffer (`data[0]` is the count).
fn code_ranges(bbuf: &BBuf) -> Vec<(u32, u32)> {
    let words: Vec<u32> = bbuf
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_ne_bytes(*chunk))
        .collect();
    let n = words.first().copied().unwrap_or(0) as usize;
    (0..n)
        .filter_map(|i| Some((*words.get(1 + 2 * i)?, *words.get(2 + 2 * i)?)))
        .collect()
}

/// The pairs of a Unicode ctype table (`CODE_RANGES`: pairs, no count).
fn ctype_ranges(table: &[OnigCodePoint]) -> Vec<(u32, u32)> {
    table
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

/// A node that produces nothing in the HIR and consumes nothing, so a
/// positive look-ahead before it may keep its body in place.
fn droppable(node: &Node) -> bool {
    match &node.inner {
        NodeInner::Anchor(an) => matches!(
            an.anchor_type,
            ANCR_PREC_READ
                | ANCR_PREC_READ_NOT
                | ANCR_LOOK_BEHIND
                | ANCR_LOOK_BEHIND_NOT
                | ANCR_BEGIN_POSITION
                | ANCR_TEXT_SEGMENT_BOUNDARY
                | ANCR_NO_TEXT_SEGMENT_BOUNDARY
        ),
        NodeInner::Gimmick(_) => true,
        NodeInner::String(sn) => sn.s.is_empty(),
        NodeInner::Quant(qn) => qn.upper == 0 || qn.body.as_deref().is_none_or(droppable),
        NodeInner::Bag(bn) => match &bn.bag_data {
            BagData::IfElse {
                then_node,
                else_node,
            } => {
                then_node.as_deref().is_none_or(droppable)
                    && else_node.as_deref().is_none_or(droppable)
            }
            _ => bn.body.as_deref().is_none_or(droppable),
        },
        NodeInner::List(_) | NodeInner::Alt(_) => cons_items(node).into_iter().all(droppable),
        _ => false,
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

struct Walk<'a> {
    reg: &'a RegexType,
    approximated: u32,
    lookaheads_kept: u32,
}

impl Walk<'_> {
    fn note(&mut self, flag: u32) {
        self.approximated |= flag;
    }

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
        if keep {
            self.note(approx::LOOKAHEAD_KEPT);
            self.lookaheads_kept += 1;
            an.body
                .as_deref()
                .map_or_else(Hir::empty, |body| self.item(body, true))
        } else {
            self.note(approx::LOOKAHEAD_DROPPED);
            Hir::empty()
        }
    }

    fn node(&mut self, node: &Node, rest_droppable: bool) -> Hir {
        if node.has_status(ND_ST_LITERAL_ALT) {
            return self.trie(node);
        }
        match &node.inner {
            NodeInner::String(sn) => {
                if node.has_status(ND_ST_IGNORECASE) && !sn.is_crude() {
                    self.note(approx::IGNORECASE_STRING);
                    return placeholder();
                }
                if sn.s.is_empty() {
                    Hir::empty()
                } else {
                    Hir::literal(sn.s.clone().into_boxed_slice())
                }
            }
            NodeInner::CClass(cc) => self.cclass(cc),
            NodeInner::CType(ct) => self.ctype(node, ct),
            NodeInner::BackRef(_) => {
                self.note(approx::BACKREF);
                placeholder()
            }
            NodeInner::Call(_) => {
                self.note(approx::CALL);
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
                let mut min = u32::try_from(qn.lower.max(0)).unwrap_or(u32::MAX);
                let mut max = (qn.upper >= 0).then_some(qn.upper as u32);
                if min > MAX_COUNTED_REPEAT || max.is_some_and(|max| max > MAX_COUNTED_REPEAT) {
                    self.note(approx::REPEAT_CAPPED);
                    min = min.min(MAX_COUNTED_REPEAT);
                    max = None;
                }
                Hir::repetition(Repetition {
                    min,
                    max,
                    greedy: qn.greedy,
                    sub: Box::new(sub),
                })
            }
            NodeInner::Bag(bn) => match &bn.bag_data {
                BagData::IfElse {
                    then_node,
                    else_node,
                } => {
                    self.note(approx::CONDITIONAL);
                    let then = then_node
                        .as_deref()
                        .map_or_else(Hir::empty, |n| self.item(n, rest_droppable));
                    let otherwise = else_node
                        .as_deref()
                        .map_or_else(Hir::empty, |n| self.item(n, rest_droppable));
                    Hir::alternation(vec![then, otherwise])
                }
                _ => bn
                    .body
                    .as_deref()
                    .map_or_else(Hir::empty, |body| self.item(body, rest_droppable)),
            },
            NodeInner::Anchor(an) => self.anchor(an),
            NodeInner::List(_) => self.list(node, rest_droppable),
            NodeInner::Alt(_) => {
                let branches = cons_items(node)
                    .into_iter()
                    .map(|branch| self.item(branch, rest_droppable))
                    .collect();
                Hir::alternation(branches)
            }
            NodeInner::Gimmick(gn) => match gn.gimmick_type {
                GimmickType::Fail => Hir::fail(),
                _ => {
                    self.note(approx::GIMMICK);
                    Hir::empty()
                }
            },
        }
    }

    fn list(&mut self, node: &Node, rest_droppable: bool) -> Hir {
        let items = cons_items(node);
        let mut parts = Vec::with_capacity(items.len());
        let mut kept_before = false;
        for (k, item) in items.iter().enumerate() {
            // At most one look-ahead per list keeps its body: two of them
            // hold at the same position, and in the HIR the second would
            // follow the first's text.
            let rest =
                rest_droppable && !kept_before && items[k + 1..].iter().all(|n| droppable(n));
            let kept = self.lookaheads_kept;
            parts.push(self.item(item, rest));
            if self.lookaheads_kept != kept {
                kept_before = true;
            }
        }
        Hir::concat(parts)
    }

    fn anchor(&mut self, an: &AnchorNode) -> Hir {
        let word = |unicode: Look, ascii: Look| if an.ascii_mode { ascii } else { unicode };
        match an.anchor_type {
            // A look-ahead reaches here only where something consuming follows it.
            ANCR_PREC_READ => {
                self.note(approx::LOOKAHEAD_DROPPED);
                Hir::empty()
            }
            ANCR_PREC_READ_NOT => {
                self.note(approx::NEGATIVE_LOOKAHEAD);
                Hir::empty()
            }
            ANCR_LOOK_BEHIND | ANCR_LOOK_BEHIND_NOT => {
                self.note(approx::LOOKBEHIND);
                Hir::empty()
            }
            ANCR_BEGIN_BUF => Hir::look(Look::Start),
            ANCR_END_BUF => Hir::look(Look::End),
            ANCR_SEMI_END_BUF => {
                self.note(approx::SEMI_END_BUF);
                Hir::look(Look::EndLF)
            }
            ANCR_BEGIN_LINE => Hir::look(Look::StartLF),
            ANCR_END_LINE => Hir::look(Look::EndLF),
            ANCR_BEGIN_POSITION => {
                self.note(approx::BEGIN_POSITION);
                Hir::empty()
            }
            ANCR_WORD_BOUNDARY | ANCR_NO_WORD_BOUNDARY | ANCR_WORD_BEGIN | ANCR_WORD_END
                if !keep_word_boundaries() =>
            {
                self.note(approx::WORD_BOUNDARY_DROPPED);
                Hir::empty()
            }
            ANCR_WORD_BOUNDARY => Hir::look(word(Look::WordUnicode, Look::WordAscii)),
            ANCR_NO_WORD_BOUNDARY => {
                Hir::look(word(Look::WordUnicodeNegate, Look::WordAsciiNegate))
            }
            ANCR_WORD_BEGIN => Hir::look(word(Look::WordStartUnicode, Look::WordStartAscii)),
            ANCR_WORD_END => Hir::look(word(Look::WordEndUnicode, Look::WordEndAscii)),
            _ => {
                self.note(approx::TEXT_SEGMENT);
                Hir::empty()
            }
        }
    }

    fn cclass(&mut self, cc: &CClassNode) -> Hir {
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        for member in bitset_members(&cc.bs) {
            if member >= 0x80 {
                self.note(approx::CLASS_HIGH_BITS);
                return any_char();
            }
            let member = member as u32;
            match ranges.last_mut() {
                Some((_, hi)) if *hi + 1 == member => *hi = member,
                _ => ranges.push((member, member)),
            }
        }
        if let Some(mbuf) = &cc.mbuf {
            ranges.extend(code_ranges(mbuf));
        }
        class_hir(ranges, cc.is_not())
    }

    fn ctype(&mut self, node: &Node, ct: &CtypeNode) -> Hir {
        if ct.ctype == CTYPE_ANYCHAR {
            if node.has_status(ND_ST_MULTILINE) {
                return any_char();
            }
            return Hir::dot(if unicode_classes() {
                Dot::AnyCharExceptLF
            } else {
                Dot::AnyByteExceptLF
            });
        }
        if ct.ctype == ONIGENC_CTYPE_WORD as i32 {
            if ct.ascii_mode {
                return class_hir(
                    [
                        (b'0' as u32, b'9' as u32),
                        (b'A' as u32, b'Z' as u32),
                        (b'_' as u32, b'_' as u32),
                        (b'a' as u32, b'z' as u32),
                    ],
                    ct.not,
                );
            }
            if let Some(table) =
                crate::unicode::onigenc_unicode_ctype_code_range(ONIGENC_CTYPE_WORD)
            {
                return class_hir(ctype_ranges(table), ct.not);
            }
        }
        self.note(approx::OTHER_CTYPE);
        any_char()
    }

    /// A literal trie: its literals, or for a folded trie what the engine
    /// accepts for them (ASCII case pairs, the non-ASCII members of each
    /// letter's class, and the strings of each multi-character segment).
    fn trie(&mut self, node: &Node) -> Hir {
        let Some(trie) =
            literal_alt_trie_index(node).and_then(|idx| self.reg.literal_tries.get(idx))
        else {
            return placeholder();
        };
        let literals = trie.literals();
        if !trie.is_case_insensitive() {
            return Hir::alternation(
                literals
                    .iter()
                    .map(|literal| Hir::literal(literal.clone().into_boxed_slice()))
                    .collect(),
            );
        }
        let Some(folds) = trie.folds() else {
            return placeholder();
        };
        self.note(approx::FOLDED_TRIE);
        let letter_class = |c: u8| -> Hir {
            if !c.is_ascii_alphabetic() {
                return Hir::literal([c]);
            }
            let lower = c.to_ascii_lowercase();
            let mut ranges = vec![
                (lower as u32, lower as u32),
                (c.to_ascii_uppercase() as u32, c.to_ascii_uppercase() as u32),
            ];
            ranges.extend(
                folds
                    .class_members
                    .iter()
                    .filter(|&&(_, letter)| letter == lower)
                    .map(|&(code, _)| (code, code)),
            );
            class_hir(ranges, false)
        };
        let branches = literals
            .iter()
            .enumerate()
            .map(|(li, literal)| {
                let segments = folds.segments.get(li).map_or(&[][..], Vec::as_slice);
                let mut parts = Vec::new();
                let mut pos = 0;
                while pos < literal.len() {
                    if let Some(&(_, len, accepted)) =
                        segments.iter().find(|&&(start, _, _)| start == pos)
                    {
                        let len = len.max(1).min(literal.len() - pos);
                        let mut alternatives: Vec<Hir> = folds
                            .accepted
                            .get(accepted)
                            .map_or(&[][..], Vec::as_slice)
                            .iter()
                            .map(|s| Hir::literal(s.clone().into_boxed_slice()))
                            .collect();
                        alternatives.push(Hir::concat(
                            literal[pos..pos + len]
                                .iter()
                                .map(|&c| letter_class(c))
                                .collect(),
                        ));
                        parts.push(Hir::alternation(alternatives));
                        pos += len;
                    } else {
                        parts.push(letter_class(literal[pos]));
                        pos += 1;
                    }
                }
                Hir::concat(parts)
            })
            .collect();
        Hir::alternation(branches)
    }
}

/// The automata of one set.
pub struct SetPrefilter {
    meta: regex_automata::meta::Regex,
    meta_cache: regex_automata::meta::Cache,
    dfa: hybrid::DFA,
    dfa_cache: hybrid::Cache,
    patset: PatternSet,
    /// The entry index each pattern ID stands for, ascending.
    entries: Vec<u16>,
    /// Entries searched on their own, in index order.
    own: Vec<u16>,
    /// Scratch for `candidates_at`.
    candidates: Vec<u16>,
    /// Times the overlapping DFA gave up and every entry was a candidate.
    pub dfa_quits: u64,
    /// Nanoseconds the automata took to build.
    pub build_nanos: u64,
}

impl std::fmt::Debug for SetPrefilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetPrefilter")
            .field("entries", &self.entries.len())
            .field("own", &self.own.len())
            .finish_non_exhaustive()
    }
}

impl SetPrefilter {
    /// The automata over the seek HIRs of `regs` (in entry order), or `None`
    /// when no entry can be pre-filtered or the automata do not build.
    pub(crate) fn build<'a>(regs: impl Iterator<Item = &'a RegexType>) -> Option<Self> {
        let started = std::time::Instant::now();
        let mut hirs: Vec<&Hir> = Vec::new();
        let mut entries = Vec::new();
        let mut own = Vec::new();
        for (index, reg) in regs.enumerate() {
            let index = u16::try_from(index).ok()?;
            match &reg.seek {
                Some(seek) if !seek.always => {
                    hirs.push(&seek.hir);
                    entries.push(index);
                }
                // Callouts observe every attempt; nothing in the set may skip one.
                None if reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0) => return None,
                _ => own.push(index),
            }
        }
        if entries.is_empty() {
            return None;
        }
        let debug = std::env::var_os("FERRONI_DFA_PREFILTER_DEBUG").is_some();
        let syntax = syntax::Config::new().utf8(false);
        let meta = regex_automata::meta::Builder::new()
            .syntax(syntax)
            .configure(
                regex_automata::meta::Config::new()
                    .match_kind(MatchKind::LeftmostFirst)
                    .utf8_empty(false)
                    .nfa_size_limit(Some(NFA_SIZE_LIMIT))
                    .hybrid_cache_capacity(META_CACHE_CAPACITY),
            )
            .build_many_from_hir(&hirs)
            .inspect_err(|error| {
                if debug {
                    eprintln!("dfa-prefilter: meta regex not built: {error}");
                }
            })
            .ok()?;
        let nfa = thompson::Compiler::new()
            .configure(
                thompson::Config::new()
                    .utf8(false)
                    .which_captures(thompson::WhichCaptures::None)
                    .nfa_size_limit(Some(NFA_SIZE_LIMIT)),
            )
            .build_many_from_hir(&hirs)
            .inspect_err(|error| {
                if debug {
                    eprintln!("dfa-prefilter: NFA not built: {error}");
                }
            })
            .ok()?;
        let dfa = hybrid::Builder::new()
            .configure(
                hybrid::Config::new()
                    .match_kind(MatchKind::All)
                    .unicode_word_boundary(true)
                    .cache_capacity(OVERLAPPING_CACHE_CAPACITY)
                    .skip_cache_capacity_check(true),
            )
            .build_from_nfa(nfa)
            .inspect_err(|error| {
                if debug {
                    eprintln!("dfa-prefilter: overlapping DFA not built: {error}");
                }
            })
            .ok()?;
        let meta_cache = meta.create_cache();
        let dfa_cache = dfa.create_cache();
        let patset = PatternSet::new(entries.len());
        Some(SetPrefilter {
            meta,
            meta_cache,
            dfa,
            dfa_cache,
            patset,
            entries,
            own,
            candidates: Vec::new(),
            dfa_quits: 0,
            build_nanos: started.elapsed().as_nanos() as u64,
        })
    }

    /// Entries the automata do not cover, in index order.
    pub(crate) fn own(&self) -> &[u16] {
        &self.own
    }

    /// Number of entries the automata cover.
    pub fn covered(&self) -> usize {
        self.entries.len()
    }

    /// The earliest position at or after `from` where some covered entry's
    /// HIR matches in `haystack`.
    #[inline]
    pub(crate) fn earliest(&mut self, haystack: &[u8], from: usize) -> Option<usize> {
        let input = Input::new(haystack).span(from..haystack.len());
        self.meta
            .search_with(&mut self.meta_cache, &input)
            .map(|m| m.start())
    }

    /// The covered entries whose HIR matches at `at`, ascending; every
    /// covered entry when the DFA gives up.
    #[inline]
    pub(crate) fn candidates_at(&mut self, haystack: &[u8], at: usize) -> &[u16] {
        self.candidates.clear();
        self.patset.clear();
        let input = Input::new(haystack)
            .span(at..haystack.len())
            .anchored(Anchored::Yes);
        match self
            .dfa
            .try_which_overlapping_matches(&mut self.dfa_cache, &input, &mut self.patset)
        {
            Ok(()) => {
                let entries = &self.entries;
                self.candidates.extend(
                    self.patset
                        .iter()
                        .map(|id: PatternID| entries[id.as_usize()]),
                );
            }
            Err(error) => {
                debug_assert!(matches!(
                    error.kind(),
                    MatchErrorKind::Quit { .. } | MatchErrorKind::GaveUp { .. }
                ));
                self.dfa_quits += 1;
                self.candidates.extend_from_slice(&self.entries);
            }
        }
        &self.candidates
    }

    /// Heap memory of the automata and their caches, in bytes.
    pub fn memory_usage(&self) -> usize {
        self.memory_breakdown().iter().sum()
    }

    /// Heap memory in bytes of the meta regex, its cache, the overlapping
    /// DFA and its cache.
    pub fn memory_breakdown(&self) -> [usize; 4] {
        [
            self.meta.memory_usage(),
            self.meta_cache.memory_usage(),
            self.dfa.memory_usage(),
            self.dfa_cache.memory_usage(),
        ]
    }
}

/// What a scanner's pre-filter looks like, for the spike's census.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// The set has automata.
    pub built: bool,
    /// Patterns the automata cover.
    pub covered: usize,
    /// Patterns searched on their own: no seek (callouts, encoding) or a
    /// seek that matches everywhere.
    pub own: Vec<usize>,
    /// Per pattern, the seek HIR as regex syntax, or `None`.
    pub seeks: Vec<Option<String>>,
    /// Per pattern, the `approx` flags.
    pub approximated: Vec<u32>,
    /// Heap memory of the automata and caches, in bytes.
    pub memory_usage: usize,
    /// `memory_usage` split into the meta regex, its cache, the overlapping
    /// DFA and its cache.
    pub memory_breakdown: [usize; 4],
    /// Nanoseconds the automata took to build.
    pub build_nanos: u64,
    /// Times the overlapping DFA gave up so far.
    pub dfa_quits: u64,
}

#[cfg(test)]
mod tests {
    use crate::scanner::{Scanner, ScannerFindOptions};

    fn find(patterns: &[&str], text: &str, start: usize) -> Option<(usize, Vec<(usize, usize)>)> {
        let mut scanner = Scanner::new(patterns).unwrap();
        for (i, seek) in scanner.dfa_prefilter_report().seeks.iter().enumerate() {
            eprintln!("pattern {i}: {seek:?}");
        }
        scanner
            .find_next_match(text, start, ScannerFindOptions::NONE)
            .map(|m| {
                (
                    m.index,
                    m.captures().iter().map(|c| (c.start, c.end)).collect(),
                )
            })
    }

    #[test]
    fn java_trace_call_35() {
        let _limits = crate::regexec::shared_limits();
        let patterns = [
            r"\b((?:[A-Z_a-z]\w*\s*\.\s*)*[A-Z_]\w*)\b((?=\s*[\n$A-Z_a-z])|(?=\s*\.\.\.))",
            ",",
        ];
        let text = "    record Order(String customer, BigDecimal amount, boolean completed) {}\n";
        assert_eq!(
            find(&patterns, text, 17),
            Some((0, vec![(17, 23), (17, 23), (23, 23)]))
        );
    }
}
