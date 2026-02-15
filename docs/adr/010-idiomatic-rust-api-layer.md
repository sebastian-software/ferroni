# ADR-010: Idiomatic Rust API Layer

## Status

Accepted

## Context

Ferroni is a 1:1 C-to-Rust port of Oniguruma (ADR-001). The internal code intentionally mirrors C structure, naming, and patterns for upstream traceability. However, the public API was also C-shaped, requiring users to:

- Pass raw pointer casts (`&OnigSyntaxOniguruma as *const OnigSyntaxType`)
- Interpret `i32` return codes that mix match positions with error codes
- Construct `OnigRegion` manually and index into `Vec<i32>` with `-1` sentinels
- Pass 7 parameters to `onig_search()` including redundant length arguments

This made the library difficult to use for Rust developers accustomed to APIs like `regex::Regex`.

## Decision

Add an idiomatic Rust API layer on top of the C-ported internals:

### Tier 1: Public API Types (additive, zero regression risk)

1. **`RegexError` enum** (`src/error.rs`): Groups ~100 `const i32` error codes into 12 semantic variants (`Memory`, `Syntax`, `TimeLimitOver`, etc.) with `std::error::Error` implementation and `From<i32>` conversion.

2. **`Regex` wrapper** (`src/api.rs`): Wraps `RegexType` with methods like `new()`, `find()`, `is_match()`, `captures()`, `find_iter()`. Delegates to `onig_new()` and `onig_search()` internally.

3. **`RegexBuilder`** (`src/api.rs`): Fluent builder for compile options (`case_insensitive()`, `dot_matches_newline()`, `multi_line_anchors()`, `extended()`, `syntax()`).

4. **`Match` and `Captures`** (`src/api.rs`): Type-safe result types that hide `OnigRegion` internals. Users get `start()`, `end()`, `as_str()`, `as_bytes()` instead of raw `beg`/`end` vectors.

5. **Prelude** (`src/prelude.rs`): Re-exports `Regex`, `RegexBuilder`, `Match`, `Captures`, `RegexError` for convenient `use ferroni::prelude::*`.

### Tier 2: Targeted Internal Improvements (low risk)

1. **`bitflags!` for option flags**: Replace `type OnigOptionType = u32` with a `bitflags` struct for type-safe option manipulation. Old constant names preserved as aliases.

2. **`onig_new()` returns `Result<RegexType, RegexError>`**: The primary compilation entry point now returns typed errors instead of `i32` codes.

3. **References instead of raw pointers**: `onig_new()` takes `&OnigSyntaxType` instead of `*const OnigSyntaxType`. `onig_get_syntax()` returns `&OnigSyntaxType`.

4. **Encapsulated `RegexType` fields**: All 37 `pub` fields changed to `pub(crate)`. External access via accessor functions or the Tier 1 wrapper.

## Rationale

- **Zero regression risk for Tier 1**: All new code, no existing code modified. The C-ported internals remain intact for upstream traceability (ADR-001).
- **Low risk for Tier 2**: Each change is mechanical and verified by the full 1,695-test suite.
- **User-facing impact**: Reduces a 7-line C-style regex creation to a 1-line idiomatic call.
- **Layered architecture**: Users who need C-level control can still use `onig_new()` / `onig_search()` directly.

## Consequences

- The C-ported internal code remains structurally faithful to the C original (ADR-001 unchanged).
- New users should prefer `ferroni::prelude::Regex` over `ferroni::regcomp::onig_new()`.
- The `bitflags` crate is added as a dependency.
- `RegexType` fields are no longer directly accessible from outside the crate; use accessor functions or the `Regex` wrapper.
- Future API additions should follow the idiomatic layer pattern: wrap C-ported internals, don't modify them.
