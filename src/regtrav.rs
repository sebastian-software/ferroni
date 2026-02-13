// regtrav.rs - Port of regtrav.c
// Capture tree traversal for capture history.

use crate::oniguruma::*;

/// Recursive helper for capture tree traversal.
/// Callback receives (group, beg, end, level, at).
#[cfg_attr(coverage_nightly, coverage(off))]
fn capture_tree_traverse<F>(
    node: &OnigCaptureTreeNode,
    at: i32,
    callback: &mut F,
    level: i32,
) -> i32
where
    F: FnMut(i32, i32, i32, i32, i32) -> i32,
{
    if (at & ONIG_TRAVERSE_CALLBACK_AT_FIRST) != 0 {
        let r = callback(
            node.group,
            node.beg,
            node.end,
            level,
            ONIG_TRAVERSE_CALLBACK_AT_FIRST,
        );
        if r != 0 {
            return r;
        }
    }

    for child in &node.childs {
        let r = capture_tree_traverse(child, at, callback, level + 1);
        if r != 0 {
            return r;
        }
    }

    if (at & ONIG_TRAVERSE_CALLBACK_AT_LAST) != 0 {
        let r = callback(
            node.group,
            node.beg,
            node.end,
            level,
            ONIG_TRAVERSE_CALLBACK_AT_LAST,
        );
        if r != 0 {
            return r;
        }
    }

    0
}

/// Traverse the capture tree of a region.
/// Callback receives (group, beg, end, level, at) and should return 0 to continue.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_capture_tree_traverse<F>(region: &OnigRegion, at: i32, mut callback: F) -> i32
where
    F: FnMut(i32, i32, i32, i32, i32) -> i32,
{
    if let Some(ref root) = region.history_root {
        capture_tree_traverse(root, at, &mut callback, 0)
    } else {
        0
    }
}
