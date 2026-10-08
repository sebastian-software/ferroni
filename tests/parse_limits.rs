//! The nesting limit and the AST budget are process-wide, so this file holds a
//! single test that sets and restores them in order, as
//! tests/subexp_call_limits.rs does.
//
// Raising either limit raises the stack a compile needs, so the test compiles
// on an 8 MiB thread. The defaults are checked against 2 MiB in api_test.rs.

use ferroni::prelude::{Regex, RegexError};
use ferroni::regparse::{
    onig_get_ast_node_limit, onig_get_parse_depth_limit, onig_set_ast_node_limit,
    onig_set_parse_depth_limit,
};

// The documented defaults of the parser limits (ADR-013; the setters in
// `ferroni::regparse` restore them on zero). `regint` is crate-private.
const DEFAULT_PARSE_DEPTH_LIMIT: u32 = 256;
const DEFAULT_AST_NODE_LIMIT: u32 = 4096;

/// Compiles `pattern` on a spawned thread with an explicit 8 MiB stack.
fn compile_on_eight_mib_stack(pattern: String) -> Result<(), RegexError> {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || Regex::new(&pattern).map(|_| ()))
        .expect("spawn compile thread")
        .join()
        .expect("compile thread panicked")
}

fn alternatives(count: usize) -> String {
    (0..count)
        .map(|i| format!("a{i}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn nested_groups(groups: usize) -> String {
    "(".repeat(groups) + "a" + &")".repeat(groups)
}

#[test]
fn raising_the_limits_admits_larger_patterns_and_zero_restores_them() {
    assert_eq!(onig_get_parse_depth_limit(), DEFAULT_PARSE_DEPTH_LIMIT);
    assert_eq!(onig_get_ast_node_limit(), DEFAULT_AST_NODE_LIMIT);

    // The AST budget is separate from nesting: raising it admits a wider
    // alternation, and zero restores the default that refuses it.
    let wide = alternatives(DEFAULT_AST_NODE_LIMIT as usize + 904);
    assert_eq!(
        compile_on_eight_mib_stack(wide.clone()),
        Err(RegexError::ParseDepthLimitOver)
    );
    assert_eq!(onig_set_ast_node_limit(8192), 0);
    assert_eq!(onig_get_ast_node_limit(), 8192);
    assert_eq!(compile_on_eight_mib_stack(wide.clone()), Ok(()));
    assert_eq!(onig_set_ast_node_limit(0), 0);
    assert_eq!(onig_get_ast_node_limit(), DEFAULT_AST_NODE_LIMIT);
    assert_eq!(
        compile_on_eight_mib_stack(wide),
        Err(RegexError::ParseDepthLimitOver)
    );

    // Nesting is separate from the AST budget: 200 groups reach depth 402,
    // which the default refuses and a limit of 512 admits.
    let deep = nested_groups(200);
    assert_eq!(
        compile_on_eight_mib_stack(deep.clone()),
        Err(RegexError::ParseDepthLimitOver)
    );
    assert_eq!(onig_set_parse_depth_limit(512), 0);
    assert_eq!(onig_get_parse_depth_limit(), 512);
    assert_eq!(compile_on_eight_mib_stack(deep.clone()), Ok(()));
    assert_eq!(onig_set_parse_depth_limit(0), 0);
    assert_eq!(onig_get_parse_depth_limit(), DEFAULT_PARSE_DEPTH_LIMIT);
    assert_eq!(
        compile_on_eight_mib_stack(deep),
        Err(RegexError::ParseDepthLimitOver)
    );
}
