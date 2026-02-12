# Project Guidelines

## Language

**Project language is US English.** All code, comments, variable/function names, commit messages, documentation files, ADRs, and test names MUST be in English. No German or other non-English content.

## Project

1:1 Rust port of the Oniguruma regex engine. See `PORTING_PLAN.md` for architecture overview and `COMPARISON.md` for current parity status.

## Testing

Run the full test suite with increased stack size (debug builds require it):

```bash
RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1
```

Or with multiple threads (lower stack needed):

```bash
RUST_MIN_STACK=67108864 cargo test --test compat_utf8 -- --test-threads=4
```

WARNING: Never run `cargo test -- --ignored` on the full suite — the `conditional_recursion_complex` test hangs. Test specific groups only.

## Architecture

- Each C source file maps to one Rust module
- Same structure, method names, and parameters as the C original
- Only deviations: enum vs union, indices vs pointers, `Result<T, i32>` vs error codes
