# ADR-003: Stack Overflow in Debug Builds During Full Test Suite

## Status

Accepted

## Context

When running the full test suite (`cargo test --test compat_utf8 -- --test-threads=1`), a SIGSEGV occurs around test 1029. The affected test (`named_group_underscore_backref`) passes fine in isolation.

The root cause is a stack overflow: The recursive tree traversal functions `tune_tree()`, `compile_tree()`, `tune_call()`, `tune_call2()`, `tune_called_state()`, and `is_exclusive()` have — like in the C original — no depth limit. In Rust debug builds (without optimization), stack frames are significantly larger than in the C equivalent (~1-2 KB per recursion level). The default thread stack of 2 MB is insufficient after ~1000 sequential compilations.

## Investigation

- **All `unsafe` blocks** were audited: No use-after-free, no dangling pointers, no state leaks between tests.
- **No global mutable state** between tests (each test creates fresh `RegexType` instances).
- **`RUST_MIN_STACK=268435456`** (256 MB) resolves the issue completely — all 1468 tests pass with single-threaded execution.
- Using `--test-threads=2` or higher with 64 MB stack also works reliably.
- After adding further recursive passes (tune_call, tune_call2, tune_called_state, tune_next, is_exclusive etc.), 16 MB is no longer sufficient.
- **Release builds** with optimization and inlining are not affected.
- **The C original also has no depth limit** in `tune_tree`/`compile_tree` (only the parser has `PARSE_DEPTH_LIMIT`). The issue doesn't manifest in C because C stack frames are smaller.

## Decision

We do not introduce an artificial depth limit because:

1. The C original has none — feature parity takes precedence.
2. Release builds are not affected.
3. `RUST_MIN_STACK` provides a simple solution for debug testing.

## Consequences

- Tests should be run with increased stack size:
  ```bash
  RUST_MIN_STACK=268435456 cargo test --test compat_utf8
  ```
- Alternatively: `--test-threads=2` or higher distributes the load across multiple threads (usually works but is not deterministic).
- If a depth limit is desired in the future, it can be added analogous to `PARSE_DEPTH_LIMIT` in `tune_tree()` and `compile_tree()`.
