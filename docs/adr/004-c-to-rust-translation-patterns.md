# ADR-004: C-to-Rust Translation Patterns

## Status

Accepted

## Context

The port follows a 1:1 structural mapping from C to Rust (ADR-001). Many C constructs have no direct Rust equivalent. This ADR documents the canonical translation patterns used throughout the codebase, so that contributors can follow the same conventions and cross-reference the C original.

## Decision

### Pointers and Slices

C Oniguruma uses `UChar* p` / `UChar* end` pointer pairs to represent string ranges. In Rust, these become:

- **Read-only string data:** `&[u8]` slice, or `(pos: usize, data: &[u8])` when the function needs to advance a cursor through the slice.
- **The `(pos, data)` pattern** is the most common in the parser (`regparse.rs`), where `pos` replaces the C pointer that gets incremented (`p++` becomes `pos += 1`).

### Error Handling

C Oniguruma returns negative `int` error codes (e.g. `ONIGERR_MEMORY = -5`). In Rust:

- Internal functions return `Result<T, i32>` where the `Err` variant carries the same negative integer.
- The error code constants are preserved identically (`ONIGERR_MEMORY`, `ONIGERR_PARSER_BUG`, etc.).
- `goto fail` cleanup patterns become `loop { ... break }` or early `return Err(...)`.
- The public entry point `onig_new()` returns `Result<RegexType, RegexError>`, where `RegexError` groups ~100 error codes into semantic variants ([ADR-010](010-idiomatic-rust-api-layer.md)).

### Unions to Enums

C uses `union` for variant types. In Rust:

- **AST nodes:** C's `union node_u` (with type tag) becomes `enum Node { String(..), CClass(..), Quant(..), Bag(..), Anchor(..), ... }`.
- **VM operation payloads:** C's `union OpArg` becomes `struct Operation { opcode: OpCode, payload: enum OperationPayload { ... } }`.

### Hash Tables

C's `st.c` (custom hash table for named groups) is replaced by `std::collections::HashMap`. No separate module needed.

### Memory Management

- C's `malloc`/`free`/`xfree` become Rust's owned types (`Box`, `Vec`, `String`) with automatic `Drop`.
- `onig_free`, `onig_region_free`, `onig_free_match_param`, etc. are not ported -- Rust's ownership model makes them unnecessary.
- `onig_new_without_alloc` and `onig_reg_init` are not ported -- Rust handles allocation via `Box` in `onig_new`.

### Global State

C uses unguarded mutable globals for configuration (match stack limit, retry limit, etc.). In Rust:

- All global state uses `AtomicU32`, `AtomicU64`, or `AtomicPtr` with appropriate ordering.
- Global function pointers (callout callbacks, warn functions) use `AtomicPtr` + `transmute`, matching the C pattern but with atomic access.

### Control Flow

| C Pattern | Rust Equivalent |
|-----------|-----------------|
| `goto fail` | `loop { ... break }` or `return Err(...)` |
| `switch (opcode)` | `match opcode { ... }` |
| `for (p = start; p < end; p++)` | `while pos < end { ... pos += 1; }` |
| `do { ... } while (0)` (macro) | inline block or function |
| Fallthrough in `switch` | Explicit shared code or combined match arms |

## Consequences

- Rust code may look non-idiomatic in places (large match arms, manual index tracking instead of iterators) -- this is intentional per ADR-001.
- The `(pos, data)` pattern is pervasive in the parser and compiler. New code in these modules should follow the same convention.
- Contributors should read the corresponding C function when modifying Rust code, as the translation is meant to be line-for-line comparable.
