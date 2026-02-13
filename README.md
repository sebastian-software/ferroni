# Ferroni

**Pure-Rust regex engine based on [Oniguruma](https://github.com/kkos/oniguruma), with SIMD-accelerated search.**

Ferroni started as a line-by-line port of Oniguruma's C source into safe,
idiomatic Rust -- same structure, same function names, same semantics. On top
of that foundation, the search pipeline has been re-engineered with
SIMD-vectorized scanning (NEON on ARM, SSE2/AVX2 on x86-64) via the `memchr`
crate, making it **up to 6x faster than C Oniguruma** on full-text search
workloads. No bindings, no FFI -- pure Rust, zero `unsafe`.

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
compiled at `-O3`. Run on Apple M1 Ultra. Lower is better; **bold** marks the
faster engine. Ratio >1.0 means Rust is slower.

```
cargo bench --features ffi
```

### Regex Execution

| Benchmark | Rust | C | Ratio |
|-----------|-----:|--:|------:|
| **Literal match** | | | |
| exact string | **134 ns** | 149 ns | 0.90 |
| anchored start | **103 ns** | 147 ns | 0.70 |
| anchored end | 165 ns | **160 ns** | 1.03 |
| word boundary | **118 ns** | 155 ns | 0.76 |
| **Quantifiers** | | | |
| greedy | **222 ns** | 257 ns | 0.86 |
| lazy | **194 ns** | 222 ns | 0.87 |
| possessive | **189 ns** | 235 ns | 0.80 |
| nested | **180 ns** | 234 ns | 0.77 |
| **Alternation** | | | |
| 2 branches | **106 ns** | 153 ns | 0.69 |
| 5 branches | **118 ns** | 186 ns | 0.64 |
| 10 branches | 255 ns | **223 ns** | 1.14 |
| nested | **125 ns** | 176 ns | 0.71 |
| **Backreferences** | | | |
| simple `(\w+) \1` | **143 ns** | 188 ns | 0.76 |
| nested | **148 ns** | 201 ns | 0.74 |
| named | **143 ns** | 189 ns | 0.76 |
| **Lookaround** | | | |
| positive lookahead | **124 ns** | 163 ns | 0.76 |
| negative lookahead | **137 ns** | 179 ns | 0.77 |
| positive lookbehind | 276 ns | **264 ns** | 1.05 |
| negative lookbehind | 353 ns | **332 ns** | 1.06 |
| combined | 299 ns | **286 ns** | 1.05 |
| **Unicode properties** | | | |
| `\p{Lu}+` | **93 ns** | 143 ns | 0.65 |
| `\p{Letter}+` | **126 ns** | 170 ns | 0.74 |
| `\p{Greek}+` | 320 ns | **239 ns** | 1.34 |
| `\p{Cyrillic}+` | 435 ns | **324 ns** | 1.34 |
| **Case-insensitive** | | | |
| single word | **106 ns** | 154 ns | 0.69 |
| phrase | **161 ns** | 185 ns | 0.87 |
| alternation | **112 ns** | 157 ns | 0.71 |
| **Named captures** | | | |
| date extraction | 454 ns | **272 ns** | 1.67 |
| **Large text (first match)** | | | |
| literal 10 KB | **112 ns** | 147 ns | 0.76 |
| literal 50 KB | **112 ns** | 142 ns | 0.79 |
| timestamp 10 KB | 230 ns | **180 ns** | 1.28 |
| timestamp 50 KB | 230 ns | **179 ns** | 1.28 |
| field extract 10 KB | **160 ns** | 176 ns | 0.91 |
| field extract 50 KB | **158 ns** | 173 ns | 0.91 |
| no match 10 KB | **381 ns** | 1.9 µs | 0.20 |
| no match 50 KB | **1.5 µs** | 9.3 µs | 0.16 |
| **RegSet** | | | |
| position-lead (5 patterns) | **148 ns** | 389 ns | 0.38 |
| regex-lead (5 patterns) | **162 ns** | 234 ns | 0.69 |
| **Match at position** | | | |
| `\d+` at offset 4 | **117 ns** | 152 ns | 0.77 |

### Regex Compilation

| Pattern | Rust | C | Ratio |
|---------|-----:|--:|------:|
| literal | **429 ns** | 466 ns | 0.92 |
| `.*` | 769 ns | **532 ns** | 1.45 |
| alternation | 1,791 ns | **1,449 ns** | 1.24 |
| char class | 673 ns | **636 ns** | 1.06 |
| quantifier | 1,403 ns | **1,049 ns** | 1.34 |
| group | 1,076 ns | **786 ns** | 1.37 |
| backref | 1,631 ns | **967 ns** | 1.69 |
| lookahead | 763 ns | **482 ns** | 1.58 |
| lookbehind | 721 ns | **552 ns** | 1.31 |
| named capture | 46,849 ns | **5,751 ns** | 8.15 |

### Analysis

**Where Rust wins (29 of 39 execution benchmarks):** Most execution
benchmarks are 10-30% faster than C. Quantifiers, backreferences,
case-insensitive matching, and RegSet searches all show consistent gains.
The likely explanation is Rust's `Vec<Operation>` layout (contiguous,
predictable) vs. C's pointer-chased operation arrays giving better cache
behavior in the VM loop. Thin LTO enables cross-module inlining (e.g.
Unicode table lookups) without the I-cache pressure of full LTO.

**SIMD-accelerated forward search** is the standout result. The
`memchr` crate replaces hand-written byte loops in the search pipeline
with SIMD-vectorized scans (SSE2/AVX2 on x86-64, NEON on aarch64). The
impact is most visible in full-text no-match scanning, where the engine
must scan the entire haystack without finding a literal prefix:

- **no match 10 KB: 4.9x faster than C** (381 ns vs 1.9 µs)
- **no match 50 KB: 6.1x faster than C** (1.5 µs vs 9.3 µs)

**Where C wins:** No execution benchmark exceeds 1.67x. The remaining
gaps are:

1. **Named captures (1.67x)** -- the Rust capture-handling VM path has
   overhead from region ownership semantics (move in/out of search
   function) that C avoids with simple pointer passing.

2. **Script-specific Unicode properties (1.34x)** -- `\p{Greek}`
   and `\p{Cyrillic}` have residual overhead from Rust's bounds checking
   in the codepoint classification inner loop.

3. **Timestamp extraction (1.28x)** -- character-class map search
   for `\d` first-byte; C's byte-by-byte loop with the 256-entry map
   is hard to beat when the set has > 3 distinct bytes (SIMD dispatch
   only covers 1-3 byte sets).

4. **Lookbehind (1.05-1.06x)** -- effectively at parity with C.

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
The execution gains directly reduce the time spent in the hot loop.

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
