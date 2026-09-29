//! Specialized bytecode length and emission for marked decimal groups.
//! Unmarked groups and capture layouts requiring atomic fallback stay with
//! the original bag compiler in `regcomp`.

use crate::regcomp::{
    SIZE_INC, add_op, compile_cclass_node, compile_length_cclass_node, compile_length_tree,
    compile_tree, detect_cclass_ascii_fast,
};
use crate::regint::*;
use crate::regparse_types::*;

/// An opt-in rewrite marker, with a defensive check of the lowering shape.
/// Ordinary and source-written atomic groups keep the original compiler path.
fn possessive_ascii_class_repeat(bag: &BagNode, status: u32) -> Option<&CClassNode> {
    if status & ND_ST_POSSESSIVE_CLASS_REPEAT == 0 || bag.bag_type != BagType::StopBacktrack {
        return None;
    }
    let NodeInner::Quant(q) = &bag.body.as_deref()?.inner else {
        return None;
    };
    if !q.greedy || q.lower != 1 || !is_infinite_repeat(q.upper) {
        return None;
    }
    let NodeInner::CClass(class) = &q.body.as_deref()?.inner else {
        return None;
    };
    (!class.is_not()
        && class.mbuf.is_none()
        && (128..SINGLE_BYTE_SIZE).all(|byte| !bitset_at(&class.bs, byte)))
    .then_some(class)
}

/// Shares the opt-in shape check between length calculation and emission.
fn decimal_tail_prefix(bag: &BagNode, status: u32) -> Option<(&Node, &Node)> {
    if status & ND_ST_DECIMAL_TAIL_PREFIX == 0 || bag.bag_type != BagType::StopBacktrack {
        return None;
    }
    super::ast::deterministic_tail_parts(bag.body.as_deref()?)
}

/// Return a specialized length, or defer to the original compiler.
pub(crate) fn compile_length(
    bag: &BagNode,
    node_status: u32,
    reg: &RegexType,
    env: &ParseEnv,
) -> Option<i32> {
    if let Some((_, tail)) = decimal_tail_prefix(bag, node_status) {
        let tail_len = compile_length_tree(tail, reg, env);
        return Some(if tail_len < 0 {
            tail_len
        } else {
            SIZE_INC + tail_len
        });
    }
    if let Some(class) = possessive_ascii_class_repeat(bag, node_status) {
        // One mandatory character followed by a choice-free star.
        return Some(compile_length_cclass_node(class, reg) + SIZE_INC);
    }
    None
}

/// Emit specialized bytecode, or defer to the original compiler.
pub(crate) fn compile(
    bag: &BagNode,
    node_status: u32,
    reg: &mut RegexType,
    env: &ParseEnv,
) -> Option<i32> {
    if let Some((mut body, tail)) = decimal_tail_prefix(bag, node_status) {
        let mut captures = Vec::new();
        while let NodeInner::Bag(memory) = &body.inner {
            let BagData::Memory { regnum, .. } = memory.bag_data else {
                unreachable!("decimal prefix admits only memory wrappers");
            };
            captures.push(regnum);
            body = memory.body.as_deref().expect("checked decimal prefix body");
        }
        add_op(
            reg,
            OpCode::DecimalTailPrefix,
            OperationPayload::DecimalTailPrefix { captures },
        );
        return Some(compile_tree(tail, reg, env));
    }
    if let Some(class) = possessive_ascii_class_repeat(bag, node_status) {
        let result = compile_cclass_node(class, reg);
        if result != 0 {
            return Some(result);
        }
        add_op(
            reg,
            OpCode::CClassPossessiveStar,
            OperationPayload::CClass {
                bsp: Box::new(class.bs),
                ascii_fast: detect_cclass_ascii_fast(&class.bs),
            },
        );
        return Some(0);
    }
    None
}
