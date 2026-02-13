# Ferroni

**1:1 Rust port of the [Oniguruma](https://github.com/kkos/oniguruma) regular expression engine.**

Ferroni is a line-by-line translation of Oniguruma's C source into safe, idiomatic
Rust. Same structure, same function names, same semantics. No bindings, no FFI --
pure Rust.

## Status

| Category | Parity |
|----------|--------|
| VM opcodes | 84/84 (100%) |
| Regex features | 100% -- all escapes, groups, options, lookbehind |
| Syntax definitions | 12/12 (100%) -- ASIS through Ruby |
| Unicode tables | 100% -- 629 tables, 886 properties, EGCB + WB segmentation |
| Error codes | 66/66 (100%) |
| Optimization passes | 100% -- BMH, auto-possessification, quantifier reduction |
| Public API | 96/103 (93%) |
| C test parity | 1,695 tests across 5 suites (100%) |
| Encodings | ASCII + UTF-8 (intentional scope) |

**Feature-complete for ASCII/UTF-8 workloads.**

## Supported Regex Features

- All Perl/Ruby/Python syntax: `(?:...)`, `(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)`, `(?>...)`
- Named captures: `(?<name>...)`, `(?'name'...)`, `(?P<name>...)`
- Backreferences: `\k<name>`, `\g<name>`, relative `\g<-1>`
- Conditionals: `(?(cond)T|F)`
- Absent expressions: `(?~...)`
- Unicode properties: `\p{Script_Extensions=Greek}`, `\p{Lu}`, `\p{Emoji}` (886 property names)
- Grapheme clusters: `\X`, text segment boundaries: `\y`, `\Y`
- Callouts: `(?{...})`, `(*FAIL)`, `(*MAX{n})`, `(*COUNT)`, `(*CMP)`
- 12 syntax modes: Oniguruma, Ruby, Perl, Perl_NG, Python, Java, Emacs, Grep, GNU, POSIX Basic/Extended, ASIS
- Safety limits: retry, time, stack, subexp call depth (global + per-search)

## Quick Start

Add to `Cargo.toml`:

```toml
[dependencies]
ferroni = { path = "." }
```

Basic usage:

```rust
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::oniguruma::*;
use ferroni::regsyntax::OnigSyntaxOniguruma;

fn main() {
    // Compile a regex
    let reg = onig_new(
        b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
        ONIG_OPTION_NONE,
        &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma as *const OnigSyntaxType,
    ).unwrap();

    // Search
    let input = b"Date: 2026-02-12";
    let (result, region) = onig_search(
        &reg, input, input.len(), input.len(), 0,
        Some(OnigRegion::new()), ONIG_OPTION_NONE,
    );

    let region = region.unwrap();
    assert!(result >= 0);
    assert_eq!(region.beg[0], 6);  // "2026-02-12" starts at byte 6
    assert_eq!(region.end[0], 16);
}
```

## Running Tests

The test suite requires increased stack size for debug builds:

```bash
# Recommended: multi-threaded with 256MB stack
RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1

# Other test suites (no special stack needed)
cargo test --test compat_syntax
cargo test --test compat_options
cargo test --test compat_regset
RUST_MIN_STACK=268435456 cargo test --test compat_back -- --test-threads=1
```

> **Warning:** Never run `cargo test -- --ignored` on the full suite -- the
> `conditional_recursion_complex` test intentionally hangs.

## Architecture

Each C source file maps 1:1 to a Rust module:

| C File | Rust Module | Purpose |
|--------|-------------|---------|
| regparse.c | `regparse.rs` | Pattern parser (6,648 LOC) |
| regcomp.c | `regcomp.rs` | AST-to-bytecode compiler (6,803 LOC) |
| regexec.c | `regexec.rs` | VM executor (5,005 LOC) |
| regint.h | `regint.rs` | Internal types & opcodes |
| oniguruma.h | `oniguruma.rs` | Public types & constants |
| regenc.c | `regenc.rs` | Encoding trait |
| regsyntax.c | `regsyntax.rs` | 12 syntax definitions |
| regset.c | `regset.rs` | Multi-regex search (RegSet) |
| regerror.c | `regerror.rs` | Error messages |
| regtrav.c | `regtrav.rs` | Capture tree traversal |
| unicode.c | `unicode/mod.rs` | Unicode tables & segmentation |

**Compilation pipeline** (same as C):
```
onig_new() -> onig_compile()
  -> onig_parse_tree()     (pattern -> AST)
  -> reduce_string_list()  (merge adjacent strings)
  -> tune_tree()           (6 optimization sub-passes)
  -> compile_tree()        (AST -> VM bytecode)
  -> set_optimize_info()   (extract search strategy)
```

## Key Differences from C

| Aspect | C | Rust |
|--------|---|------|
| Memory | Manual malloc/free | Owned types, Drop |
| Nodes | Union (`node_u`) | Enum (`Node`) |
| Operations | Union (`OpArg`) | Struct + enum payload |
| Errors | Negative int codes | `Result<T, i32>` |
| Strings | `UChar* p, *end` | `&[u8]` or `(pos, &[u8])` |
| `goto fail` | `goto` chains | `loop + break` or `return Err(...)` |
| Encodings | 29 encoding files | 2 (ASCII + UTF-8) |

## Documentation

- [`COMPARISON.md`](COMPARISON.md) -- detailed parity status vs. C original
- [`PORTING_PLAN.md`](PORTING_PLAN.md) -- module-by-module porting plan
- [`TODO_API_PARITY.md`](TODO_API_PARITY.md) -- remaining API gaps (3 of 31 open)
- [`docs/adr/`](docs/adr/) -- architecture decision records

## License

BSD-2-Clause (same as Oniguruma)
