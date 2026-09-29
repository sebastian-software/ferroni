//! Shape recognition, safety guards, and capture-preserving AST rewrites.

use super::{BacktrackingRewrite, BacktrackingRewriteRefusal};
use crate::oniguruma::*;
use crate::regint::*;
use crate::regparse_types::*;

/// Each visit examines a bounded-size rule after stripping capture wrappers.
/// The whole pass is linear in AST size and adds a bounded number of nodes
/// per rewrite and one diagnostic per candidate. It never follows AST raw pointers.
pub(crate) fn apply(
    root: &mut Node,
    options: OnigOptionType,
    capture_history: MemStatusType,
) -> Vec<BacktrackingRewrite> {
    let refusal = if capture_history != 0
        || options.intersects(
            ONIG_OPTION_FIND_LONGEST | ONIG_OPTION_FIND_NOT_EMPTY | ONIG_OPTION_CALLBACK_EACH_MATCH,
        ) {
        Some(BacktrackingRewriteRefusal::UnsupportedMode)
    } else if unsupported(root) {
        Some(BacktrackingRewriteRefusal::UnsupportedConstruct)
    } else {
        None
    };
    let mut reports = Vec::new();
    visit(root, None, refusal, &mut reports);
    reports
}

fn without_captures(mut node: &Node) -> &Node {
    while let NodeInner::Bag(bag) = &node.inner {
        if bag.bag_type != BagType::Memory {
            break;
        }
        let Some(body) = bag.body.as_deref() else {
            break;
        };
        node = body;
    }
    node
}

fn decimal_loop(node: &Node) -> bool {
    decimal_loop_with_lower(node, 1)
}

fn decimal_loop_with_lower(node: &Node, lower: i32) -> bool {
    let NodeInner::Quant(outer) = &node.inner else {
        return false;
    };
    if !outer.greedy || outer.lower != lower || !is_infinite_repeat(outer.upper) {
        return false;
    }
    let Some(body) = outer.body.as_deref() else {
        return false;
    };
    let NodeInner::List(first) = &without_captures(body).inner else {
        return false;
    };
    let Some(tail) = first.cdr.as_deref() else {
        return false;
    };
    let NodeInner::List(second) = &tail.inner else {
        return false;
    };
    if second.cdr.is_some() {
        return false;
    }
    let NodeInner::Quant(digits) = &without_captures(&first.car).inner else {
        return false;
    };
    let NodeInner::Quant(separator) = &without_captures(&second.car).inner else {
        return false;
    };
    digits.greedy
        && digits.lower == 1
        && is_infinite_repeat(digits.upper)
        && digits.body.as_deref().is_some_and(decimal_class)
        && separator.greedy
        && separator.lower == 0
        && separator.upper == 1
        && separator.body.as_deref().is_some_and(|body| {
            matches!(&without_captures(body).inner, NodeInner::String(s) if s.s == b"_" && !s.is_crude())
        })
}

fn decimal_tail(node: &Node) -> bool {
    let NodeInner::List(first) = &node.inner else {
        return false;
    };
    let Some(tail) = first.cdr.as_deref() else {
        return false;
    };
    let NodeInner::List(second) = &tail.inner else {
        return false;
    };
    decimal_loop_with_lower(without_captures(&first.car), 0)
        && matches!(&without_captures(&second.car).inner, NodeInner::Quant(q)
        if q.greedy && q.lower == 1 && is_infinite_repeat(q.upper)
            && q.body.as_deref().is_some_and(decimal_class))
}

fn decimal_class(node: &Node) -> bool {
    let NodeInner::CClass(class) = &without_captures(node).inner else {
        return false;
    };
    !class.is_not()
        && class.mbuf.is_none()
        && (0..SINGLE_BYTE_SIZE).all(|byte| {
            bitset_at(&class.bs, byte) == (b'0' as usize..=b'9' as usize).contains(&byte)
        })
}

/// The specialized prefix retains only captures around the complete repeated
/// body. Captures within digit/separator primitives, or around the outer star,
/// keep the general atomic lowering. The final run is compiled normally.
pub(super) fn deterministic_tail_parts(node: &Node) -> Option<(&Node, &Node)> {
    if !decimal_tail(node) {
        return None;
    }
    let NodeInner::List(first) = &node.inner else {
        return None;
    };
    let NodeInner::Quant(prefix) = &first.car.inner else {
        return None;
    };
    let body = prefix.body.as_deref()?;
    let NodeInner::List(digits) = &without_captures(body).inner else {
        return None;
    };
    let NodeInner::Quant(q) = &digits.car.inner else {
        return None;
    };
    if !matches!(&q.body.as_deref()?.inner, NodeInner::CClass(_)) {
        return None;
    }
    let NodeInner::List(separator) = &digits.cdr.as_deref()?.inner else {
        return None;
    };
    let NodeInner::Quant(q) = &separator.car.inner else {
        return None;
    };
    if !matches!(&q.body.as_deref()?.inner, NodeInner::String(_)) {
        return None;
    }
    let NodeInner::List(tail) = &first.cdr.as_deref()?.inner else {
        return None;
    };
    Some((body, &tail.car))
}

fn tail_followed_by_boundary(node: &Node, next: Option<&Node>) -> bool {
    let NodeInner::List(first) = &node.inner else {
        return false;
    };
    let Some(tail) = first.cdr.as_deref() else {
        return false;
    };
    let NodeInner::List(second) = &tail.inner else {
        return false;
    };
    let following = second.cdr.as_deref().and_then(|rest| match &rest.inner {
        NodeInner::List(list) => Some(list.car.as_ref()),
        _ => None,
    });
    matches!(following.or(next).map(without_captures),
        Some(Node { inner: NodeInner::Anchor(anchor), .. })
            if anchor.anchor_type == ANCR_WORD_BOUNDARY && anchor.body.is_none())
}

fn make_tail_atomic(node: &mut Node) {
    let deterministic = deterministic_tail_parts(node).is_some();
    let mut pair = std::mem::replace(node, *node_new_bag(BagType::StopBacktrack));
    let NodeInner::List(first) = &mut pair.inner else {
        unreachable!("decimal tail must be a list");
    };
    let NodeInner::List(second) = &mut first.cdr.as_deref_mut().unwrap().inner else {
        unreachable!("decimal tail must have a second element");
    };
    let rest = second.cdr.take();
    node.set_body(Some(Box::new(pair)));
    if deterministic {
        node.status |= ND_ST_DECIMAL_TAIL_PREFIX;
    }
    if rest.is_some() {
        let atomic = std::mem::replace(node, *node_new_bag(BagType::StopBacktrack));
        *node = *node_new_list(Box::new(atomic), rest);
    }
}

fn without_captures_mut(mut node: &mut Node) -> &mut Node {
    while matches!(&node.inner, NodeInner::Bag(bag) if bag.bag_type == BagType::Memory && bag.body.is_some())
    {
        let NodeInner::Bag(bag) = &mut node.inner else {
            unreachable!("capture bag was checked above");
        };
        node = bag
            .body
            .as_deref_mut()
            .expect("capture body was checked above");
    }
    node
}

/// Prefer a primitive digit run: it has no captures or choices in its body.
/// Captures around the run remain outside its new wrapper. Captures inside
/// the repeated character require the general outer-atomic fallback.
fn possessify_digits(node: &mut Node) -> bool {
    let NodeInner::Quant(outer) = &mut node.inner else {
        return false;
    };
    let Some(body) = outer.body.as_deref_mut() else {
        return false;
    };
    let NodeInner::List(first) = &mut without_captures_mut(body).inner else {
        return false;
    };
    let digits = without_captures_mut(&mut first.car);
    if !matches!(&digits.inner, NodeInner::Quant(q) if q.body.as_deref().is_some_and(|body| matches!(body.inner, NodeInner::CClass(_))))
    {
        return false;
    }
    let original = std::mem::replace(digits, *node_new_bag(BagType::StopBacktrack));
    digits.set_body(Some(Box::new(original)));
    digits.status |= ND_ST_POSSESSIVE_CLASS_REPEAT;
    true
}

fn mandatory_dot(node: &Node) -> bool {
    matches!(&without_captures(node).inner, NodeInner::String(s) if s.s.first() == Some(&b'.') && !s.is_crude())
}

fn unsupported(node: &Node) -> bool {
    match &node.inner {
        NodeInner::BackRef(_) | NodeInner::Call(_) | NodeInner::Gimmick(_) => true,
        NodeInner::Anchor(anchor) => {
            anchor.body.is_some() || anchor.anchor_type == ANCR_BEGIN_POSITION
        }
        NodeInner::Bag(bag) => {
            bag.bag_type != BagType::Memory || bag.body.as_deref().is_some_and(unsupported)
        }
        NodeInner::Quant(q) => q.body.as_deref().is_some_and(unsupported),
        NodeInner::List(list) | NodeInner::Alt(list) => {
            unsupported(&list.car) || list.cdr.as_deref().is_some_and(unsupported)
        }
        NodeInner::String(_) | NodeInner::CClass(_) | NodeInner::CType(_) => false,
    }
}

fn visit(
    node: &mut Node,
    next: Option<&Node>,
    refusal: Option<BacktrackingRewriteRefusal>,
    reports: &mut Vec<BacktrackingRewrite>,
) {
    if decimal_tail(node) {
        if let Some(reason) = refusal {
            reports.push(BacktrackingRewrite::Refused(reason));
        } else if !tail_followed_by_boundary(node, next) {
            reports.push(BacktrackingRewrite::Refused(
                BacktrackingRewriteRefusal::NoWordBoundary,
            ));
        } else {
            // The entire pair's first successful path ends at the last digit
            // of its maximal accepted prefix, giving digits back internally
            // when needed. Every shorter exit is followed by a digit or an
            // underscore: both sides are word characters, so its boundary
            // fails. At the maximal exit the original first capture assignment
            // is retained. The whole-tree guard makes discarded assignments
            // unobservable to any suffix. Keep the internal give-back choices;
            // making the inner digit run possessive would change results.
            // A primitive prefix can calculate that first assignment directly,
            // reserving the last digit rather than producing give-back choices.
            let deterministic = deterministic_tail_parts(node).is_some();
            make_tail_atomic(node);
            reports.push(if deterministic {
                BacktrackingRewrite::DeterministicDecimalTail
            } else {
                BacktrackingRewrite::AtomicDecimalTail
            });
            if let NodeInner::List(list) = &mut node.inner {
                if let Some(rest) = list.cdr.as_deref_mut() {
                    visit(rest, next, refusal, reports);
                }
            }
            return;
        }
    }
    if decimal_loop(node) {
        if let Some(reason) = refusal {
            reports.push(BacktrackingRewrite::Refused(reason));
        } else if !next.is_some_and(mandatory_dot) {
            reports.push(BacktrackingRewrite::Refused(
                BacktrackingRewriteRefusal::NoMandatoryDot,
            ));
        } else {
            // Proof: on its first successful path the greedy loop consumes
            // the maximal prefix of digits with at most one underscore per
            // digit run. Its only internal choices repartition digit runs.
            // They cannot cross a dot or produce a longer accepted prefix.
            // Every shorter exit leaves a digit or underscore where the
            // mandatory dot must match, and therefore fails. At the same
            // exit, any suffix success has already been tried with the first
            // (unchanged) capture assignment. No suffix may read captures or
            // observe backtracking, as established by the whole-tree guard.
            // Removing those retries preserves successful bounds, priority,
            // and all captures, including in enclosing loops/alternations.
            // Retry, stack, and timeout outcomes may change by design.
            // Making only the primitive digit run possessive removes every
            // digit partition while retaining the first greedy assignment.
            // Underscores cannot be skipped to reach another iteration, so
            // the remaining outer exits cannot create another successful
            // prefix. The same mandatory-dot/whole-tree proof applies.
            if possessify_digits(node) {
                reports.push(BacktrackingRewrite::PossessiveDecimalDigits);
            } else {
                let original = std::mem::replace(node, *node_new_bag(BagType::StopBacktrack));
                node.set_body(Some(Box::new(original)));
                reports.push(BacktrackingRewrite::AtomicDecimalLoop);
            }
            return;
        }
    }
    match &mut node.inner {
        NodeInner::List(list) => {
            let following = list.cdr.as_deref().and_then(|tail| match &tail.inner {
                NodeInner::List(rest) => Some(rest.car.as_ref()),
                _ => None,
            });
            visit(&mut list.car, following.or(next), refusal, reports);
            if let Some(rest) = list.cdr.as_deref_mut() {
                visit(rest, next, refusal, reports);
            }
        }
        NodeInner::Alt(list) => {
            visit(&mut list.car, next, refusal, reports);
            if let Some(rest) = list.cdr.as_deref_mut() {
                visit(rest, next, refusal, reports);
            }
        }
        NodeInner::Bag(bag) => {
            if let Some(body) = bag.body.as_deref_mut() {
                visit(body, next, refusal, reports);
            }
        }
        NodeInner::Quant(q) => {
            if let Some(body) = q.body.as_deref_mut() {
                visit(body, None, refusal, reports);
            }
        }
        NodeInner::Anchor(anchor) => {
            if let Some(body) = anchor.body.as_deref_mut() {
                visit(body, None, refusal, reports);
            }
        }
        _ => {}
    }
}
