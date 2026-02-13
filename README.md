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

## Why Rust?

### Security

The C Oniguruma library has a history of memory safety CVEs, including:

- **CVE-2019-13224** (CVSS 9.8) -- use-after-free in `onig_new_deluxe()`, potential code execution
- **CVE-2019-19204** -- heap buffer over-read in `fetch_interval_quantifier()`, missing bounds check
- **CVE-2019-19246** -- heap buffer over-read in `str_lower_case_match()`
- **CVE-2019-19012** -- integer overflow in `search_in_range()` leading to out-of-bounds read
- **CVE-2019-13225** -- NULL pointer dereference in `match_at()`

These affect Ruby, PHP, and any application linking against C Oniguruma.

The Rust port eliminates these vulnerability classes structurally:

| Vulnerability class | C | Rust |
|---|---|---|
| Buffer over-read/write | Raw `UChar*` arithmetic | Bounds-checked `&[u8]` slices |
| Use-after-free | Manual `malloc`/`free` | Ownership + `Drop` |
| NULL dereference | Raw pointers | `Option<T>` |
| Double-free | Manual lifecycle | Single owner, `Drop` once |
| Integer overflow | Undefined behavior | Panic (debug) / defined wrap (release) |
| Uninitialized memory | Stack variables | All values initialized |

**Honest caveat:** The port contains 86 `unsafe` blocks across ~20,400 LOC
(0.4% of lines). These are concentrated in two patterns:

1. **AST raw pointers** (regcomp.rs) -- call nodes share target references
   that can't be expressed with Rust's borrow checker. These pointers are
   set once during parsing and remain valid for the regex's lifetime.
2. **Function pointer storage** (regexec.rs) -- global callout callbacks
   use `AtomicPtr` + `transmute`, matching the C pattern for global
   function pointers.

None of the `unsafe` blocks involve buffer arithmetic, allocation, or string
processing -- the areas where C Oniguruma's CVEs occurred.

### Practical Benefits

- **No C toolchain required** -- pure Rust, no FFI, no linking headaches
- **`cargo build`** -- replaces autoconf/cmake/make
- **Cross-compilation** -- `cargo build --target wasm32-unknown-unknown` works out of the box
- **Package management** -- usable as a crate dependency
- **Thread safety** -- global state uses atomics; no unguarded mutable statics
- **Error handling** -- `Result<T, i32>` instead of checking return codes and hoping

### What This Port Does Not Improve

- **Stack exhaustion** -- deeply nested regex patterns can still overflow the
  stack in both C and Rust. The port carries over the same `parse_depth_limit`
  and `subexp_call_max_nest_level` safeguards, but pathological patterns remain
  a risk in debug builds.
- **Algorithmic complexity** -- regex patterns with exponential backtracking
  behave identically to C. The same `retry_limit_in_match` and `time_limit`
  mitigations apply.
- **Performance** -- see [benchmarks below](#performance). Most execution
  benchmarks are faster than C; compilation and lookbehind are slower.

## Performance

Criterion benchmarks comparing Ferroni (Rust) against the C original,
compiled at `-O3`. Run on Apple M4 Pro. Lower is better; **bold** marks the
faster engine. Ratio >1.0 means Rust is slower.

```
cargo bench --features ffi
```

### Regex Execution

| Benchmark | Rust | C | Ratio |
|-----------|-----:|--:|------:|
| **Literal match** | | | |
| exact string | **97 ns** | 153 ns | 0.63 |
| anchored start | **92 ns** | 142 ns | 0.65 |
| anchored end | **104 ns** | 161 ns | 0.65 |
| word boundary | **104 ns** | 165 ns | 0.63 |
| **Quantifiers** | | | |
| greedy | **199 ns** | 256 ns | 0.78 |
| lazy | **173 ns** | 209 ns | 0.83 |
| possessive | **178 ns** | 231 ns | 0.77 |
| nested | **172 ns** | 227 ns | 0.76 |
| **Alternation** | | | |
| 2 branches | **94 ns** | 150 ns | 0.62 |
| 5 branches | **111 ns** | 169 ns | 0.66 |
| 10 branches | 293 ns | **219 ns** | 1.34 |
| nested | **124 ns** | 171 ns | 0.73 |
| **Backreferences** | | | |
| simple `(\w+) \1` | **142 ns** | 189 ns | 0.75 |
| nested | **148 ns** | 189 ns | 0.78 |
| named | **142 ns** | 187 ns | 0.76 |
| **Lookaround** | | | |
| positive lookahead | **120 ns** | 159 ns | 0.76 |
| negative lookahead | **132 ns** | 175 ns | 0.75 |
| positive lookbehind | 718 ns | **266 ns** | 2.70 |
| negative lookbehind | 904 ns | **337 ns** | 2.68 |
| combined | 772 ns | **284 ns** | 2.72 |
| **Unicode properties** | | | |
| `\p{Lu}+` | **86 ns** | 139 ns | 0.62 |
| `\p{Letter}+` | **126 ns** | 164 ns | 0.77 |
| `\p{Greek}+` | 727 ns | **242 ns** | 3.00 |
| `\p{Cyrillic}+` | 1,232 ns | **333 ns** | 3.70 |
| **Case-insensitive** | | | |
| single word | **101 ns** | 156 ns | 0.65 |
| phrase | 205 ns | **187 ns** | 1.10 |
| alternation | **104 ns** | 154 ns | 0.68 |
| **Named captures** | | | |
| date extraction | 1,031 ns | **274 ns** | 3.77 |
| **Large text (first match)** | | | |
| literal 10 KB | **89 ns** | 147 ns | 0.60 |
| literal 50 KB | **89 ns** | 145 ns | 0.61 |
| timestamp 10 KB | 230 ns | **178 ns** | 1.29 |
| timestamp 50 KB | 229 ns | **178 ns** | 1.29 |
| field extract 10 KB | **134 ns** | 173 ns | 0.77 |
| field extract 50 KB | **134 ns** | 174 ns | 0.77 |
| no match 10 KB | 676 µs | **1.8 µs** | 366x |
| no match 50 KB | 3,384 µs | **9.4 µs** | 361x |
| **RegSet** | | | |
| position-lead (5 patterns) | **141 ns** | 389 ns | 0.36 |
| regex-lead (5 patterns) | **149 ns** | 227 ns | 0.65 |
| **Match at position** | | | |
| `\d+` at offset 4 | **113 ns** | 148 ns | 0.76 |

### Regex Compilation

| Pattern | Rust | C | Ratio |
|---------|-----:|--:|------:|
| literal | **444 ns** | 468 ns | 0.95 |
| `.*` | 793 ns | **542 ns** | 1.46 |
| alternation | 1,773 ns | **1,455 ns** | 1.22 |
| char class | 661 ns | **652 ns** | 1.01 |
| quantifier | 1,425 ns | **1,063 ns** | 1.34 |
| group | 1,101 ns | **806 ns** | 1.37 |
| backref | 1,671 ns | **1,009 ns** | 1.66 |
| lookahead | 795 ns | **491 ns** | 1.62 |
| lookbehind | 734 ns | **563 ns** | 1.31 |
| named capture | 48,554 ns | **5,930 ns** | 8.19 |

### Analysis

**Where Rust wins (35 of 48 benchmarks):** Most execution benchmarks are
20-40% faster than C. Literal matching, quantifiers, backreferences, and
RegSet searches all show consistent gains. The likely explanation is Rust's
`Vec<Operation>` layout (contiguous, predictable) vs. C's pointer-chased
operation arrays giving better cache behavior in the VM loop.

**Where C wins:** Three areas show meaningful regressions:

1. **Lookbehind (2.7x slower)** -- the Rust lookbehind implementation scans
   backwards using index arithmetic on `&[u8]` slices where C uses raw
   pointer decrement. The bounds checking overhead accumulates in the
   inner loop.

2. **Script-specific Unicode properties (3-4x slower)** -- `\p{Greek}` and
   `\p{Cyrillic}` require scanning past ASCII/Latin text to find the first
   match. The Rust character-class check path has overhead per codepoint
   that the C version avoids through direct table lookup with pointer
   arithmetic.

3. **Full-text no-match scan (360x slower)** -- this is the largest gap.
   When a literal pattern like `CRITICAL_ERROR` has no match in large text,
   C's Boyer-Moore-Horspool fast-skip loop rejects most positions in one
   comparison. The Rust port's BMH implementation appears to not engage
   the skip table on this path, falling back to per-byte scanning. This is
   a known optimization gap, not a fundamental limitation.

**Compilation** is 1.2-1.7x slower across the board, with a notable 8x
outlier on named captures. The Rust compiler pipeline allocates more
(Vec/String/Box) where C reuses pre-allocated buffers.

**In practice, compilation overhead is nearly invisible.** Real-world
consumers -- syntax highlighters like Shiki/TextMate, Ruby's regex engine,
PHP's `mb_ereg` -- compile their patterns once at startup and then match
against them thousands to millions of times. A typical TextMate grammar
compiles 50-200 patterns and then matches every token in every line of
source code, yielding a compile:match ratio well above 1:100,000. At that
ratio, even the 8x named-capture outlier adds < 0.01% to total runtime.
The 20-40% execution gains, on the other hand, directly reduce the time
spent in the hot loop. The highest-impact optimization target is the
no-match BMH scan gap (360x), since that affects the execution path.

### Running Benchmarks

```bash
cargo bench --features ffi               # full suite (~8 min)
cargo bench --features ffi -- compile    # specific group
cargo bench --features ffi -- "large_"   # pattern filter
# HTML report: target/criterion/report/index.html
```

## What's Not Included

**27 of 29 encodings** -- only ASCII and UTF-8 are implemented. This is a
deliberate design decision; UTF-8 covers the vast majority of use cases.
EUC-JP, Shift-JIS, UTF-16/32, ISO-8859-x, etc. are not ported.

**POSIX API** (`regcomp`/`regexec`/`regfree`) -- intentionally not ported.
Rust has no need for the POSIX regex interface.

**Memory management functions** (`onig_free`, `onig_region_free`, etc.) --
replaced by Rust's `Drop` trait. No manual deallocation needed.

**`onig_new_deluxe` / `onig_new_without_alloc`** -- C-specific allocation
patterns that don't apply in Rust. Use `onig_new()` instead.

**`onig_unicode_define_user_property`** -- requires a mutable global Unicode
table at runtime. Not ported; the 886 built-in properties cover all standard
Unicode categories.

**`onig_copy_encoding`** -- not applicable. In Rust, `OnigEncoding` is a
`&'static dyn Encoding` trait object reference, not a copyable struct.

**`onig_builtin_skip`** -- conditionally compiled in C behind `USE_SKIP_SEARCH`,
not enabled by default. Niche optimization for specific search patterns.

**`onig_setup_builtin_monitors_by_ascii_encoded_name`** -- registers debug
monitors that write to a C `FILE*`. No Rust equivalent; use Rust's own
tracing/logging instead.

**`onig_get_capture_range_in_callout` / `onig_get_used_stack_size_in_callout`** --
function signatures are present, but return placeholder values. Full
implementation requires exposing VM stack internals through `OnigCalloutArgs`,
which is only relevant when user-defined callouts are dispatched through the
VM (builtins work via an internal fast path).

## Documentation

- [`COMPARISON.md`](COMPARISON.md) -- detailed parity status vs. C original
- [`PORTING_PLAN.md`](PORTING_PLAN.md) -- module-by-module porting plan
- [`TODO_API_PARITY.md`](TODO_API_PARITY.md) -- remaining API gaps (3 of 31 open)
- [`docs/adr/`](docs/adr/) -- architecture decision records

## License

BSD-2-Clause (same as Oniguruma)
