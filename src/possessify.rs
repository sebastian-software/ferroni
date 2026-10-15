//! Rust-only (ADR-008): automatic possessification of greedy single-character
//! stars across optional groups, alternations and end anchors.
//!
//! C's `tune_next` makes a greedy `X*` or `X+` possessive when the node right
//! after it starts with a character `X` cannot match (`[a-z]+:` runs as
//! `(?>[a-z]+):`). It looks at that one node only, so the star in
//! `[^\s:@/]+(?::[^\s:@/]*)?@`, whose next node is optional, keeps every
//! choice: on a long run that fails, it gives back one character at a time
//! and tries the rest at each position. PCRE2's automatic possessification
//! looks further, and so does this pass.
//!
//! **Why giving back cannot help.** A greedy star over a body `X` that
//! matches one character gives back to a position `p` only after a further
//! iteration matched there, so the character at `p` is one `X` matches, and
//! `p` is before the end of the subject. What follows the star (its
//! *follower*) has to match from `p`. The follower fails at `p` when it
//! cannot begin there with a character `X` matches:
//!
//! - a character, class or string that `X` excludes (C's `is_exclusive`)
//!   fails at `p`;
//! - `\z` fails before the end; `$` and `\Z` hold before the end only at a
//!   newline, so they fail when `X` cannot match a newline;
//! - an element that can match the empty string (an optional group, a star,
//!   a zero-width check such as a look-ahead or a word boundary) is passed
//!   when it cannot begin with such a character either; the rest decides;
//! - an alternation fails when each of its branches does;
//! - at the end of a group, the walk goes on with what follows the group:
//!   for a repeated group, the body again or what follows the repeat.
//!
//! If the follower fails at every position the star could give back to,
//! the only run that can lead to a match is the longest one, which the
//! greedy star tries first; dropping the other choices changes no result.
//! A star with this proof compiles to its usual instruction followed by
//! `PopStarChoices`, which pops the choices the star pushed. Under
//! ADR-017 the backtracks this leaves out are not counted.
//!
//! **Not applied** (the pass does not run, or the walk gives up): lazy and
//! bounded repeats; bodies other than one class, the word type or one ASCII
//! byte (so no captures); a known next byte (C's peek-next form); back
//! references, subroutine calls and callouts anywhere in the pattern;
//! look-behinds, conditions, gimmicks other than `\K` and literal tries in
//! the follower; the end of the pattern, where the follower accepts; and
//! encodings that are not ASCII-compatible. Stars inside look-arounds,
//! atomic groups and conditions are left alone.

#[cfg(test)]
use std::cell::Cell;

use crate::oniguruma::{ONIGENC_CTYPE_WORD, OnigCodePoint};
use crate::regcomp::{compiles_to_plain_star, is_exclusive, onig_is_code_in_cc};
use crate::regenc::onigenc_is_ascii_compatible_encoding;
use crate::regint::*;
use crate::regparse_types::*;

/// The most follower nodes one star's walk visits before it gives up.
const MAX_FOLLOW_STEPS: u32 = 256;

#[cfg(test)]
thread_local! {
    /// Leaves every star as it is, as the reference for differential tests.
    pub(crate) static POSSESSIFY_DISABLED: Cell<bool> = const { Cell::new(false) };
}

/// What follows a node, up to the end of the pattern.
enum Follow<'a> {
    /// The end of the pattern, where every position is accepted.
    End,
    /// The remaining cells of a list (a `List` node), then the rest.
    Seq(&'a Node, &'a Follow<'a>),
    /// The end of a repeated body: the body again, or the rest.
    Again(&'a Node, &'a Follow<'a>),
}

/// How a follower node can match at a position whose character the star's
/// body matches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Start {
    /// It cannot match there.
    Fails,
    /// It can only match the empty string there.
    Empty,
}

/// Mark every greedy star in `root` whose follower can never match where
/// the star would give back (see the module documentation).
pub(crate) fn apply(root: &mut Node, reg: &RegexType, env: &ParseEnv) {
    #[cfg(test)]
    if POSSESSIFY_DISABLED.with(Cell::get) {
        return;
    }
    if !onigenc_is_ascii_compatible_encoding(reg.enc)
        || env.num_call > 0
        || env.backref_num > 0
        || reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0)
    {
        return;
    }
    let mut stars = Vec::new();
    collect(root, &Follow::End, reg, &mut Vec::new(), &mut stars);
    for path in &stars {
        mark(root, path);
    }
}

/// `\K`: a gimmick that saves the match start.
fn is_keep(gn: &GimmickNode) -> bool {
    gn.gimmick_type == GimmickType::Save && gn.detail_type == SaveType::Keep as i32
}

/// Collect the paths to the stars under `node` (at `path`) that can be made
/// possessive, given what follows `node`. A path step is the index of an
/// element in a list or alternation, or 0 for the body of a repeat or group.
fn collect<'a>(
    node: &'a Node,
    follow: &Follow<'a>,
    reg: &RegexType,
    path: &mut Vec<u32>,
    stars: &mut Vec<Vec<u32>>,
) {
    match &node.inner {
        NodeInner::List(_) => {
            let mut cell = node;
            let mut index = 0;
            while let NodeInner::List(cons) = &cell.inner {
                path.push(index);
                match cons.cdr.as_deref() {
                    Some(rest) => {
                        collect(&cons.car, &Follow::Seq(rest, follow), reg, path, stars);
                        cell = rest;
                    }
                    None => collect(&cons.car, follow, reg, path, stars),
                }
                path.pop();
                if cons.cdr.is_none() {
                    break;
                }
                index += 1;
            }
        }
        NodeInner::Alt(_) => {
            let mut cell = node;
            let mut index = 0;
            while let NodeInner::Alt(cons) = &cell.inner {
                path.push(index);
                collect(&cons.car, follow, reg, path, stars);
                path.pop();
                match cons.cdr.as_deref() {
                    Some(rest) => cell = rest,
                    None => break,
                }
                index += 1;
            }
        }
        NodeInner::Quant(qn) => {
            if let Some(body) = star_body(qn) {
                let mut steps = MAX_FOLLOW_STEPS;
                if follow_fails(follow, body, reg, &mut steps) {
                    stars.push(path.clone());
                }
                return;
            }
            let Some(body) = qn.body.as_deref() else {
                return;
            };
            path.push(0);
            match qn.upper {
                0 => {}
                1 => collect(body, follow, reg, path, stars),
                _ => collect(body, &Follow::Again(body, follow), reg, path, stars),
            }
            path.pop();
        }
        NodeInner::Bag(bn) => {
            if matches!(bn.bag_type, BagType::Memory | BagType::Option) {
                if let Some(body) = bn.body.as_deref() {
                    path.push(0);
                    collect(body, follow, reg, path, stars);
                    path.pop();
                }
            }
        }
        _ => {}
    }
}

/// The body of a greedy star this pass can make possessive: one that
/// compiles to a plain star instruction (see `compiles_to_plain_star`) over
/// one class, the word type or one ASCII byte.
fn star_body(qn: &QuantNode) -> Option<&Node> {
    if !compiles_to_plain_star(qn) {
        return None;
    }
    let body = qn.body.as_deref()?;
    match &body.inner {
        NodeInner::CClass(_) | NodeInner::CType(_) => Some(body),
        NodeInner::String(sn)
            if sn.s.len() == 1
                && !body.has_status(ND_ST_LITERAL_ALT)
                && !is_real_ignorecase(body, sn) =>
        {
            Some(body)
        }
        _ => None,
    }
}

/// A string that still matches case-insensitively (C's
/// `ND_IS_REAL_IGNORECASE`); `tune_tree` unravels these elsewhere.
fn is_real_ignorecase(node: &Node, sn: &StrNode) -> bool {
    node.has_status(ND_ST_IGNORECASE) && !sn.is_crude()
}

/// Whether `follow` fails at every position before the end of the subject
/// whose character `body` matches.
fn follow_fails(follow: &Follow<'_>, body: &Node, reg: &RegexType, steps: &mut u32) -> bool {
    match *follow {
        Follow::End => false,
        Follow::Seq(cells, rest) => {
            let mut cell = cells;
            while let NodeInner::List(cons) = &cell.inner {
                match start(&cons.car, body, reg, steps) {
                    None => return false,
                    Some(Start::Fails) => return true,
                    Some(Start::Empty) => {}
                }
                match cons.cdr.as_deref() {
                    Some(next) => cell = next,
                    None => return follow_fails(rest, body, reg, steps),
                }
            }
            false
        }
        // Another pass through the body can only match the empty string
        // there, which leaves the position where it was.
        Follow::Again(again, rest) => {
            start(again, body, reg, steps).is_some() && follow_fails(rest, body, reg, steps)
        }
    }
}

/// How `node` can match at a position before the end of the subject whose
/// character `body` matches; `None` when it may consume that character, or
/// when the walk does not know.
fn start(node: &Node, body: &Node, reg: &RegexType, steps: &mut u32) -> Option<Start> {
    *steps = steps.checked_sub(1)?;
    match &node.inner {
        NodeInner::String(sn) => {
            if sn.s.is_empty() {
                Some(Start::Empty)
            } else if node.has_status(ND_ST_LITERAL_ALT) || is_real_ignorecase(node, sn) {
                None
            } else {
                is_exclusive(body, node, reg).then_some(Start::Fails)
            }
        }
        NodeInner::CClass(_) | NodeInner::CType(_) => {
            is_exclusive(body, node, reg).then_some(Start::Fails)
        }
        NodeInner::List(_) => {
            let mut cell = node;
            while let NodeInner::List(cons) = &cell.inner {
                if start(&cons.car, body, reg, steps)? == Start::Fails {
                    return Some(Start::Fails);
                }
                match cons.cdr.as_deref() {
                    Some(next) => cell = next,
                    None => break,
                }
            }
            Some(Start::Empty)
        }
        NodeInner::Alt(_) => {
            let mut all_fail = true;
            let mut cell = node;
            while let NodeInner::Alt(cons) = &cell.inner {
                all_fail &= start(&cons.car, body, reg, steps)? == Start::Fails;
                match cons.cdr.as_deref() {
                    Some(next) => cell = next,
                    None => break,
                }
            }
            Some(if all_fail { Start::Fails } else { Start::Empty })
        }
        NodeInner::Quant(qn) => {
            if qn.upper == 0 {
                return Some(Start::Empty);
            }
            let inner = start(qn.body.as_deref()?, body, reg, steps)?;
            Some(if qn.lower == 0 { Start::Empty } else { inner })
        }
        NodeInner::Bag(bn) => match bn.bag_type {
            // An atomic group matches a subset of what its body matches.
            BagType::Memory | BagType::Option | BagType::StopBacktrack => {
                start(bn.body.as_deref()?, body, reg, steps)
            }
            BagType::IfElse => None,
        },
        NodeInner::Anchor(an) => match an.anchor_type {
            ANCR_END_BUF => Some(Start::Fails),
            ANCR_SEMI_END_BUF | ANCR_END_LINE => {
                (!matches_newline(body, reg)).then_some(Start::Fails)
            }
            // Zero-width checks: they may hold, and leave the position.
            ANCR_PREC_READ
            | ANCR_PREC_READ_NOT
            | ANCR_BEGIN_BUF
            | ANCR_BEGIN_LINE
            | ANCR_BEGIN_POSITION
            | ANCR_WORD_BOUNDARY
            | ANCR_NO_WORD_BOUNDARY
            | ANCR_WORD_BEGIN
            | ANCR_WORD_END
            | ANCR_TEXT_SEGMENT_BOUNDARY
            | ANCR_NO_TEXT_SEGMENT_BOUNDARY => Some(Start::Empty),
            // Look-behinds read the characters the star would give back.
            _ => None,
        },
        NodeInner::Gimmick(gn) if is_keep(gn) => Some(Start::Empty),
        NodeInner::Gimmick(_) | NodeInner::BackRef(_) | NodeInner::Call(_) => None,
    }
}

/// Whether the star's body can match a newline, the only character before
/// which `$` and `\Z` hold short of the end. In an ASCII-compatible encoding
/// a newline is the byte 0x0A at a character head (`is_mbc_newline`).
fn matches_newline(body: &Node, reg: &RegexType) -> bool {
    const NEWLINE: OnigCodePoint = 0x0a;
    match &body.inner {
        NodeInner::CClass(cc) => onig_is_code_in_cc(reg.enc, NEWLINE, cc),
        NodeInner::CType(ct) if ct.ctype == ONIGENC_CTYPE_WORD as i32 => ct.not,
        NodeInner::String(sn) => sn.s.first() == Some(&(NEWLINE as u8)),
        _ => true,
    }
}

/// Set `possessive_star` on the star at `path` (see `collect`).
fn mark(root: &mut Node, path: &[u32]) {
    const ON_PATH: &str = "a collected path follows the tree";
    fn cell(node: &mut Node) -> &mut ConsAltNode {
        match &mut node.inner {
            NodeInner::List(cons) | NodeInner::Alt(cons) => cons,
            _ => unreachable!("{ON_PATH}"),
        }
    }
    let mut node = root;
    for &step in path {
        node = if matches!(node.inner, NodeInner::List(_) | NodeInner::Alt(_)) {
            for _ in 0..step {
                node = cell(node).cdr.as_deref_mut().expect(ON_PATH);
            }
            &mut cell(node).car
        } else {
            match &mut node.inner {
                NodeInner::Quant(QuantNode { body, .. }) | NodeInner::Bag(BagNode { body, .. }) => {
                    body.as_deref_mut().expect(ON_PATH)
                }
                _ => unreachable!("{ON_PATH}"),
            }
        };
    }
    if let NodeInner::Quant(qn) = &mut node.inner {
        qn.possessive_star = true;
    }
}

#[cfg(test)]
mod tests;
