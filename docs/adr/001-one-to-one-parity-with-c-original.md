# ADR-001: 1:1 Parity Goal with C Original

## Status

Accepted

## Context

This project is a Rust port of the Oniguruma regex engine (C, by K. Kosako). The central question is how closely the Rust port should follow the C original — whether to redesign with idiomatic Rust patterns, or to maintain structural fidelity to the C codebase.

## Decision

The Rust port targets **1:1 structural parity** with the C original:

1. **Same module mapping.** Each C source file maps to exactly one Rust module (e.g. `regparse.c` -> `regparse.rs`, `regcomp.c` -> `regcomp.rs`).

2. **Same function names and signatures.** Public and internal functions keep their C names and parameter order. This makes it easy to cross-reference the C and Rust code side by side.

3. **Same internal control flow.** The compilation pipeline (`onig_parse_tree` -> `reduce_string_list` -> `tune_tree` -> `compile_tree`), VM executor loop, and optimization passes follow the same logic and ordering as the C original.

4. **Same test suite.** All test cases from the C test suite (`test_utf8.c`: 1,554 cases) are ported 1:1, using the same patterns, input strings, and expected match positions.

5. **Minimal Rust-idiomatic deviations** are allowed only where C constructs have no direct equivalent:
   - C `union` -> Rust `enum` (e.g. `Node`, `Operation` payload)
   - C pointer arithmetic -> Rust index-based access with `usize`
   - C error codes (negative `int`) -> Rust `Result<T, i32>`
   - C `goto fail` -> Rust `loop { ... break }` or early `return Err(...)`

6. **An idiomatic Rust API layer** wraps the C-ported internals without modifying them ([ADR-010](010-idiomatic-rust-api-layer.md)). This provides `Regex::new()`, typed errors, and safe result types while keeping the internal 1:1 structure intact.

## Rationale

- **Correctness by construction.** Oniguruma is a mature, battle-tested engine with subtle edge cases in backtracking, empty-loop detection, lookbehind validation, and Unicode folding. Keeping the same structure means every C code path has a verifiable Rust counterpart.
- **Easier debugging.** When a test fails, the C source can be consulted line-by-line to find the divergence. This has proven essential — 28 non-trivial bugs were found and fixed during the port, all diagnosed by comparing C and Rust control flow.
- **Upstream tracking.** Future Oniguruma releases can be diffed and applied to the Rust port with minimal translation effort.
- **No accidental feature loss.** A redesign risks dropping subtle behaviors (e.g. the 6x6 quantifier reduction table, auto-possessification exclusivity checks, or `FIXED_INTERVAL_IS_GREEDY_ONLY`).

## Consequences

- The Rust code may not look idiomatic in places (e.g. large match arms mirroring C switch statements, manual index tracking instead of iterators).
- Performance characteristics should be comparable to the C original, not fundamentally different.
- New features or optimizations should first be contributed upstream to C Oniguruma, then ported — not invented in the Rust port.
- Code review should compare against the C original, not against Rust best practices in isolation.
