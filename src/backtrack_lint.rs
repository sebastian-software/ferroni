//! Rust-only (ADR-008): compile-time warnings for patterns that can
//! backtrack catastrophically.
//!
//! The check walks the parsed pattern and looks for two shapes inside an
//! unbounded repeat:
//!
//! - an inner unbounded repeat whose characters can also start whatever
//!   follows it in the same iteration, or the next iteration: `(a+)+`,
//!   `([0-9]+(_?))+`, `(\s*\w+)*`
//! - alternatives that can start with the same byte: `(a|aa)*`, `(\w|\d)*`
//!
//! It is a heuristic, tuned against the 260 shiki grammars (34,676 patterns,
//! about 240 distinct patterns flagged). It compares first bytes only, and only ASCII bytes for
//! classes and character types, so it misses patterns whose overlap sits in
//! non-ASCII characters and flags some patterns that are harmless in
//! practice. Atomic groups and possessive repeats are not entered, because
//! nothing backtracks into them, and a repeat that ends the pattern is left
//! alone, since nothing after it can fail. The result never changes how a pattern
//! matches; it complements the retry, time and stack limits.

use std::fmt;

use crate::regenc::OnigEncoding;
use crate::regint::*;
use crate::regparse_types::*;

/// The shape that makes a pattern risky.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BacktrackRisk {
    /// An unbounded repeat inside an unbounded repeat that can split the same
    /// input in many ways, as in `(a+)+`.
    NestedQuantifier,
    /// Alternatives inside an unbounded repeat that can match the same
    /// input, as in `(a|aa)*`.
    OverlappingAlternation,
}

/// A finding of the compile-time backtracking check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BacktrackWarning {
    /// Which shape was found.
    pub risk: BacktrackRisk,
    /// A human-readable explanation, ready to show to a grammar author.
    pub message: String,
}

impl fmt::Display for BacktrackWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// The bytes a node can consume first, one bit per byte.
#[derive(Clone, Copy, Default)]
struct ByteSet([u128; 2]);

impl ByteSet {
    fn add(&mut self, byte: u8) {
        self.0[(byte >> 7) as usize] |= 1u128 << (byte & 0x7f);
    }

    fn union(&mut self, other: &ByteSet) {
        self.0[0] |= other.0[0];
        self.0[1] |= other.0[1];
    }

    fn overlaps(&self, other: &ByteSet) -> bool {
        (self.0[0] & other.0[0]) | (self.0[1] & other.0[1]) != 0
    }
}

/// What a node can start with, and whether it can match without consuming.
#[derive(Clone, Copy, Default)]
struct First {
    set: ByteSet,
    nullable: bool,
}

struct Lint {
    enc: OnigEncoding,
    warnings: Vec<BacktrackWarning>,
}

/// Checks a parsed pattern and returns its findings.
///
/// `exhaustive` is set for searches that keep looking after the first match
/// (`FIND_LONGEST`) or reject some matches (`FIND_NOT_EMPTY`): there, a loop at
/// the end of the pattern can be forced to backtrack as well.
pub(crate) fn check(root: &Node, enc: OnigEncoding, exhaustive: bool) -> Vec<BacktrackWarning> {
    let mut lint = Lint {
        enc,
        warnings: Vec::new(),
    };
    lint.visit(root, !exhaustive);
    lint.warnings
}

impl Lint {
    /// `tail`: nothing after this node can fail, because it ends the pattern.
    /// A repeat there is never asked to give characters back, so its own
    /// splits cost nothing; repeats inside its body are still checked.
    fn visit(&mut self, node: &Node, tail: bool) {
        match &node.inner {
            NodeInner::Quant(q) => {
                if let Some(body) = q.body.as_deref() {
                    if is_infinite_repeat(q.upper) && !tail {
                        self.check_loop(body);
                    }
                    self.visit(body, false);
                }
            }
            NodeInner::Bag(b) => {
                if let Some(body) = b.body.as_deref() {
                    match &body.inner {
                        // A possessive repeat or an atomic group around a repeat
                        // never gives back what the loop took: the loop itself
                        // is safe, only its body is checked.
                        NodeInner::Quant(q) if b.bag_type == BagType::StopBacktrack => {
                            if let Some(inner) = q.body.as_deref() {
                                self.visit(inner, false);
                            }
                        }
                        _ => self.visit(body, tail),
                    }
                }
                if let BagData::IfElse {
                    then_node,
                    else_node,
                } = &b.bag_data
                {
                    for n in [then_node, else_node].into_iter().flatten() {
                        self.visit(n, false);
                    }
                }
            }
            NodeInner::Anchor(a) => {
                if let Some(body) = a.body.as_deref() {
                    self.visit(body, false);
                }
            }
            NodeInner::List(_) => {
                let mut items = Vec::new();
                collect_chain(node, &mut items);
                let last = items.len() - 1;
                for (i, item) in items.into_iter().enumerate() {
                    self.visit(item, tail && i == last);
                }
            }
            NodeInner::Alt(_) => {
                let mut items = Vec::new();
                collect_chain(node, &mut items);
                for item in items {
                    self.visit(item, tail);
                }
            }
            _ => {}
        }
    }

    /// One unbounded repeat with the given body: is an iteration ambiguous?
    fn check_loop(&mut self, body: &Node) {
        let loop_back = self.first(body).set;
        if let Some(risk) = self.walk(body, &loop_back, &loop_back) {
            let message = match risk {
                BacktrackRisk::NestedQuantifier => {
                    "nested unbounded repeat: an inner repeat can match characters that \
                     the rest of the enclosing repeat also accepts, so a failing match \
                     can retry exponentially many splits of the same input"
                }
                BacktrackRisk::OverlappingAlternation => {
                    "overlapping alternatives inside an unbounded repeat: more than one \
                     branch can match the same input, so a failing match can retry \
                     exponentially many combinations"
                }
            };
            self.warnings.push(BacktrackWarning {
                risk,
                message: message.to_string(),
            });
        }
    }

    /// Looks for an ambiguity inside one iteration. `follow` is what can
    /// consume input right after `node`, within the iteration or at the
    /// start of the next one.
    fn walk(&self, node: &Node, follow: &ByteSet, loop_back: &ByteSet) -> Option<BacktrackRisk> {
        match &node.inner {
            NodeInner::Quant(q) => {
                let body = q.body.as_deref()?;
                if is_infinite_repeat(q.upper) {
                    // A look-around in the body usually decides which way to
                    // go, as in `(?:\*(?!/)|[^*])*`. Do not second-guess it.
                    if has_look_around(body) {
                        return None;
                    }
                    let repeated = self.first(body).set;
                    // Exponential only when a split can also move the boundary
                    // between iterations: the repeat must be able to eat what
                    // follows it and what the next iteration starts with.
                    (repeated.overlaps(follow) && repeated.overlaps(loop_back))
                        .then_some(BacktrackRisk::NestedQuantifier)
                } else if q.upper == 0 {
                    None
                } else {
                    let mut after = *follow;
                    if q.upper > 1 {
                        after.union(&self.first(body).set);
                    }
                    self.walk(body, &after, loop_back)
                }
            }
            NodeInner::Bag(b) if b.bag_type != BagType::StopBacktrack => {
                self.walk(b.body.as_deref()?, follow, loop_back)
            }
            NodeInner::List(_) => {
                let mut items = Vec::new();
                collect_chain(node, &mut items);
                let mut after = *follow;
                for item in items.into_iter().rev() {
                    if let Some(risk) = self.walk(item, &after, loop_back) {
                        return Some(risk);
                    }
                    let first = self.first(item);
                    if first.nullable {
                        after.union(&first.set);
                    } else {
                        after = first.set;
                    }
                }
                None
            }
            NodeInner::Alt(_) => {
                let mut branches = Vec::new();
                collect_chain(node, &mut branches);
                for (i, a) in branches.iter().enumerate() {
                    if let Some(risk) = self.walk(a, follow, loop_back) {
                        return Some(risk);
                    }
                    for b in &branches[i + 1..] {
                        if self.branches_overlap(a, b) {
                            return Some(BacktrackRisk::OverlappingAlternation);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Two alternatives that can begin with the same byte. Plain strings
    /// only count when one is a prefix of the other: `ab|ac` is unambiguous.
    /// A string against a non-string is not compared.
    fn branches_overlap(&self, a: &Node, b: &Node) -> bool {
        if has_look_around(a) || has_look_around(b) {
            return false;
        }
        match (&a.inner, &b.inner) {
            (NodeInner::String(x), NodeInner::String(y)) => {
                // Case folding lists `s` and `ss` (for `ß`) side by side.
                if a.has_status(ND_ST_IGNORECASE) || b.has_status(ND_ST_IGNORECASE) {
                    return false;
                }
                return x.s.starts_with(&y.s) || y.s.starts_with(&x.s);
            }
            // Case folding turns a class into "class or multi-character
            // string" (`ß` -> `ss`); that is not a real ambiguity.
            (NodeInner::String(_), _) | (_, NodeInner::String(_)) => return false,
            _ => {}
        }
        self.first(a).set.overlaps(&self.first(b).set)
    }

    fn first(&self, node: &Node) -> First {
        let mut out = First::default();
        match &node.inner {
            NodeInner::String(s) => match s.s.first() {
                Some(&byte) => {
                    out.set.add(byte);
                    if byte.is_ascii_alphabetic() && node.has_status(ND_ST_IGNORECASE) {
                        out.set.add(byte ^ 0x20);
                    }
                }
                None => out.nullable = true,
            },
            NodeInner::CClass(c) => {
                for byte in 0..0x80u8 {
                    if bitset_at(&c.bs, byte as usize) != c.is_not() {
                        out.set.add(byte);
                    }
                }
            }
            NodeInner::CType(t) => {
                for byte in 0..0x80u8 {
                    let hit = t.ctype == CTYPE_ANYCHAR
                        || self.enc.is_code_ctype(byte as u32, t.ctype as u32) != t.not;
                    if hit {
                        out.set.add(byte);
                    }
                }
            }
            NodeInner::Quant(q) => {
                if let Some(body) = q.body.as_deref() {
                    let inner = self.first(body);
                    if q.upper != 0 {
                        out.set = inner.set;
                    }
                    out.nullable = q.lower == 0 || q.upper == 0 || inner.nullable;
                } else {
                    out.nullable = true;
                }
            }
            NodeInner::Bag(b) => match (&b.bag_data, b.body.as_deref()) {
                (BagData::IfElse { .. }, _) | (_, None) => out.nullable = true,
                (_, Some(body)) => out = self.first(body),
            },
            NodeInner::List(_) => {
                let mut items = Vec::new();
                collect_chain(node, &mut items);
                out.nullable = true;
                for item in items {
                    let first = self.first(item);
                    out.set.union(&first.set);
                    if !first.nullable {
                        out.nullable = false;
                        break;
                    }
                }
            }
            NodeInner::Alt(_) => {
                let mut branches = Vec::new();
                collect_chain(node, &mut branches);
                for branch in branches {
                    let first = self.first(branch);
                    out.set.union(&first.set);
                    out.nullable |= first.nullable;
                }
            }
            // A back-reference matches something, but the check cannot say what.
            NodeInner::BackRef(_) => {}
            // Anchors, look-arounds, calls and gimmicks
            // consume nothing that the check can name.
            _ => out.nullable = true,
        }
        out
    }
}

fn has_look_around(node: &Node) -> bool {
    match &node.inner {
        NodeInner::Anchor(a) => {
            a.anchor_type
                & (ANCR_PREC_READ | ANCR_PREC_READ_NOT | ANCR_LOOK_BEHIND | ANCR_LOOK_BEHIND_NOT)
                != 0
        }
        NodeInner::Quant(q) => q.body.as_deref().is_some_and(has_look_around),
        NodeInner::Bag(b) => b.body.as_deref().is_some_and(has_look_around),
        NodeInner::List(_) | NodeInner::Alt(_) => {
            let mut items = Vec::new();
            collect_chain(node, &mut items);
            items.into_iter().any(has_look_around)
        }
        _ => false,
    }
}

fn collect_chain<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    let mut cur = Some(node);
    while let Some(n) = cur {
        match &n.inner {
            NodeInner::List(c) | NodeInner::Alt(c) => {
                out.push(&c.car);
                cur = c.cdr.as_deref();
            }
            _ => {
                out.push(n);
                cur = None;
            }
        }
    }
}
