//! Opt-in, result-preserving AST rewrites (ADR-008).
//!
//! Recognizes only greedy `(?:[0-9]+_?)+` immediately before
//! a mandatory literal dot, and `(?:[0-9]+_?)*[0-9]+` before a word boundary.
//! Capture wrappers are retained verbatim. This is
//! deliberately independent of the heuristic in `backtrack_lint`.

use crate::oniguruma::*;
use crate::regint::*;
use crate::regparse_types::*;

/// Why a recognized decimal-loop candidate was left unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BacktrackingRewriteRefusal {
    /// The next node does not require a literal dot immediately.
    NoMandatoryDot,
    /// The decimal tail is not immediately followed by a word boundary.
    NoWordBoundary,
    /// The pattern contains capture reads, calls, look-arounds, scoped
    /// options, position checks, callouts, or other stateful constructs.
    UnsupportedConstruct,
    /// Capture history or an exhaustive matching option is enabled.
    UnsupportedMode,
}

/// One recognized candidate, in AST traversal order. Unsupported shapes
/// have no entry; absence of a report does not mean a pattern is safe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BacktrackingRewrite {
    /// Wrapped a decimal loop in an atomic group, retaining every capture.
    AtomicDecimalLoop,
    /// Made the primitive digit run possessive using choice-free bytecode.
    PossessiveDecimalDigits,
    /// Made the complete decimal tail atomic, retaining its internal captures.
    AtomicDecimalTail,
    /// Evaluate the decimal prefix directly, preserving its last iteration's captures.
    DeterministicDecimalTail,
    /// Recognized the decimal loop but could not establish its safety.
    Refused(BacktrackingRewriteRefusal),
}

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
pub(crate) fn deterministic_tail_parts(node: &Node) -> Option<(&Node, &Node)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encodings::utf8::ONIG_ENCODING_UTF8;
    use crate::regcomp::{onig_compile, onig_new_with_backtracking_optimization};
    use crate::regexec::onig_search;
    use crate::regsyntax::OnigSyntaxOniguruma;

    fn compile(pattern: &str, enabled: bool) -> RegexType {
        onig_new_with_backtracking_optimization(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
            enabled,
        )
        .unwrap()
    }

    fn raw_trace(
        reg: &RegexType,
        text: &[u8],
        end: usize,
        start: usize,
        range: usize,
        options: OnigOptionType,
    ) -> (i32, Vec<(i32, i32)>) {
        let (result, region) = onig_search(
            reg,
            text,
            end,
            start,
            range,
            Some(OnigRegion::new()),
            options,
        );
        let captures = if result >= 0 {
            let region = region.unwrap();
            region
                .beg
                .into_iter()
                .zip(region.end)
                .take(region.num_regs as usize)
                .collect()
        } else {
            Vec::new()
        };
        (result, captures)
    }

    #[test]
    fn rewrites_preserve_raw_forward_backward_bounded_and_invalid_byte_searches() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        for pattern in [
            r"([0-9]+(_?))+(\.)([0-9]*)",
            r"\b([0-9]+(_?))+(\.)([0-9]*)",
            r"(([0-9]+_?)+\.([0-9]*))*x?",
            r"([0-9]+_?)+\.([0-9]*)|([0-9]+)",
            r"\b(([0-9]+_?)*[0-9]+|0([Xx]\h+|[Oo][0-7]+))\b",
            r"\b((([0-9])+)((_)?))*(([0-9])+)\b",
        ] {
            let plain = compile(pattern, false);
            let fast = compile(pattern, true);
            if pattern == r"([0-9]+(_?))+(\.)([0-9]*)" || pattern == r"\b([0-9]+(_?))+(\.)([0-9]*)"
            {
                assert!(
                    fast.leading_run.is_some(),
                    "possessive opcode must retain the leading-run plan"
                );
            }
            if fast
                .backtrack_rewrites
                .contains(&BacktrackingRewrite::PossessiveDecimalDigits)
            {
                assert!(
                    fast.backtrack_rewrites
                        .contains(&BacktrackingRewrite::PossessiveDecimalDigits)
                );
                assert!(
                    fast.ops
                        .iter()
                        .any(|op| op.opcode == OpCode::CClassPossessiveStar)
                );
                assert!(
                    !fast
                        .ops
                        .iter()
                        .any(|op| matches!(op.opcode, OpCode::Mark | OpCode::CutToMark))
                );
                assert!(
                    !plain
                        .ops
                        .iter()
                        .any(|op| op.opcode == OpCode::CClassPossessiveStar)
                );
            } else {
                if fast
                    .backtrack_rewrites
                    .contains(&BacktrackingRewrite::DeterministicDecimalTail)
                {
                    assert!(
                        fast.ops
                            .iter()
                            .any(|op| op.opcode == OpCode::DecimalTailPrefix)
                    );
                    assert!(
                        !fast
                            .ops
                            .iter()
                            .any(|op| matches!(op.opcode, OpCode::Mark | OpCode::CutToMark))
                    );
                } else {
                    assert!(
                        fast.backtrack_rewrites
                            .contains(&BacktrackingRewrite::AtomicDecimalTail)
                    );
                    assert!(fast.ops.iter().any(|op| op.opcode == OpCode::CutToMark));
                }
            }
            assert!(
                !plain
                    .ops
                    .iter()
                    .any(|op| op.opcode == OpCode::DecimalTailPrefix)
            );
            for text in [
                b"x1_2.3 4.5".as_slice(),
                b"12__.3",
                b"12_.3",
                b"\xff12.3\x801.2",
                b"12.\xe2\x82",
                b"123_ 0x1f\xff01 0o7",
            ] {
                for end in 0..=text.len() {
                    for start in 0..=end {
                        for range in 0..=end {
                            for options in [
                                ONIG_OPTION_NONE,
                                ONIG_OPTION_NOT_BEGIN_STRING | ONIG_OPTION_NOT_END_STRING,
                                ONIG_OPTION_FIND_LONGEST,
                                ONIG_OPTION_FIND_NOT_EMPTY,
                                ONIG_OPTION_MATCH_WHOLE_STRING,
                            ] {
                                assert_eq!(
                                    raw_trace(&fast, text, end, start, range, options),
                                    raw_trace(&plain, text, end, start, range, options),
                                    "{pattern} on {text:?}, end {end}, start {start}, range {range}, {options:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn capture_history_is_refused_and_recompilation_clears_reports() {
        let mut syntax = OnigSyntaxOniguruma.clone();
        syntax.op2 |= ONIG_SYN_OP2_ATMARK_CAPTURE_HISTORY;
        let reg = onig_new_with_backtracking_optimization(
            br"(?@([0-9]+_?))+\.[0-9]+",
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &syntax,
            true,
        )
        .unwrap();
        assert_eq!(
            reg.backtrack_rewrites,
            [BacktrackingRewrite::Refused(
                BacktrackingRewriteRefusal::UnsupportedMode
            )]
        );

        let mut reg = compile(r"([0-9]+_?)+\.[0-9]+", true);
        assert_eq!(
            reg.backtrack_rewrites,
            [BacktrackingRewrite::PossessiveDecimalDigits]
        );
        assert_eq!(onig_compile(&mut reg, br"([0-9]+_?)+\.[0-9]+"), 0);
        assert!(reg.backtrack_rewrites.is_empty());
    }
}
