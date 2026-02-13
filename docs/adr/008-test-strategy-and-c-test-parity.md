# ADR-008: Test Strategy and C Test Suite Parity

## Status

Accepted

## Context

The C Oniguruma test suite consists of 8 test files covering different encodings, features, and API surfaces. The question is which tests to port and how to structure them in Rust.

## Decision

### Ported Test Suites (5)

All C test files that exercise the engine with ASCII/UTF-8 are ported 1:1:

| C Test File | Rust File | Tests | Content |
|-------------|-----------|------:|---------|
| `test_utf8.c` | `tests/compat_utf8.rs` | 1,554 | Core regex features with UTF-8 |
| `test_back.c` | `tests/compat_back.rs` | 1,225 | Backward search tests (26 sections) |
| `test_syntax.c` | `tests/compat_syntax.rs` | 43 | Syntax mode behavior |
| `test_options.c` | `tests/compat_options.rs` | 47 | Option flag behavior |
| `test_regset.c` | `tests/compat_regset.rs` | 13 | RegSet multi-pattern search |

**Total: 1,695 `#[test]` functions.**

### Not Ported (3)

| C Test File | Tests | Reason |
|-------------|------:|--------|
| `testc.c` | 658 | EUC-JP encoding (ADR-002) |
| `testu.c` | 595 | UTF-16 encoding (ADR-002) |
| `testp.c` | 421 | POSIX API (ADR-007) |

### Naming Convention

All ported test files use the `compat_` prefix to indicate they are direct translations of C test cases. Each Rust `#[test]` function corresponds to one `x2()`/`x3()`/`n()`/`e()` call in the C original:

- `x2(pattern, string, from, to)` -- match expected at `[from, to)`
- `x3(pattern, string, from, to, mem)` -- match in capture group `mem`
- `n(pattern, string)` -- no match expected
- `e(pattern, string)` -- compile or match error expected

### The `conditional_recursion_complex` Test

One test is marked `#[ignore]` and must never be run in the full suite:

```rust
#[test]
#[ignore]
fn conditional_recursion_complex() { ... }
```

This test exercises deeply nested conditional recursion that causes the VM to run indefinitely (same behavior as the C original). It exists to document the pattern, not to be executed routinely.

## Consequences

- Running `cargo test -- --ignored` on the full suite will hang. This is documented in `CLAUDE.md` and the README.
- Debug builds require increased stack size for the full suite (see ADR-003).
- Recommended test commands:
  ```bash
  RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1
  cargo test --test compat_syntax
  cargo test --test compat_options
  cargo test --test compat_regset
  RUST_MIN_STACK=268435456 cargo test --test compat_back -- --test-threads=1
  ```
