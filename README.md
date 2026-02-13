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
| exact string | **136 ns** | 151 ns | 0.90 |
| anchored start | **106 ns** | 147 ns | 0.72 |
| anchored end | 167 ns | **157 ns** | 1.06 |
| word boundary | **121 ns** | 153 ns | 0.79 |
| **Quantifiers** | | | |
| greedy | **217 ns** | 259 ns | 0.84 |
| lazy | **193 ns** | 211 ns | 0.91 |
| possessive | **191 ns** | 230 ns | 0.83 |
| nested | **180 ns** | 226 ns | 0.80 |
| **Alternation** | | | |
| 2 branches | **106 ns** | 152 ns | 0.70 |
| 5 branches | **121 ns** | 170 ns | 0.71 |
| 10 branches | 237 ns | **218 ns** | 1.09 |
| nested | **128 ns** | 173 ns | 0.74 |
| **Backreferences** | | | |
| simple `(\w+) \1` | **149 ns** | 188 ns | 0.79 |
| nested | **154 ns** | 191 ns | 0.81 |
| named | **148 ns** | 187 ns | 0.79 |
| **Lookaround** | | | |
| positive lookahead | **127 ns** | 159 ns | 0.80 |
| negative lookahead | **138 ns** | 178 ns | 0.78 |
| positive lookbehind | 279 ns | **257 ns** | 1.09 |
| negative lookbehind | 353 ns | **326 ns** | 1.08 |
| combined | 301 ns | **280 ns** | 1.08 |
| **Unicode properties** | | | |
| `\p{Lu}+` | **93 ns** | 143 ns | 0.65 |
| `\p{Letter}+` | **128 ns** | 172 ns | 0.74 |
| `\p{Greek}+` | 339 ns | **240 ns** | 1.41 |
| `\p{Cyrillic}+` | 469 ns | **332 ns** | 1.41 |
| **Case-insensitive** | | | |
| single word | **107 ns** | 154 ns | 0.69 |
| phrase | **160 ns** | 186 ns | 0.86 |
| alternation | **114 ns** | 154 ns | 0.74 |
| **Named captures** | | | |
| date extraction | 456 ns | **273 ns** | 1.67 |
| **Large text (first match)** | | | |
| literal 10 KB | **113 ns** | 143 ns | 0.79 |
| literal 50 KB | **113 ns** | 143 ns | 0.79 |
| timestamp 10 KB | 235 ns | **174 ns** | 1.35 |
| timestamp 50 KB | 235 ns | **178 ns** | 1.32 |
| field extract 10 KB | **159 ns** | 168 ns | 0.95 |
| field extract 50 KB | **159 ns** | 170 ns | 0.93 |
| no match 10 KB | **375 ns** | 1.9 µs | 0.20 |
| no match 50 KB | **1.5 µs** | 9.3 µs | 0.16 |
| **RegSet** | | | |
| position-lead (5 patterns) | **149 ns** | 399 ns | 0.37 |
| regex-lead (5 patterns) | **162 ns** | 237 ns | 0.68 |
| **Match at position** | | | |
| `\d+` at offset 4 | **118 ns** | 155 ns | 0.76 |

### Regex Compilation

| Pattern | Rust | C | Ratio |
|---------|-----:|--:|------:|
| literal | **436 ns** | 452 ns | 0.96 |
| `.*` | 779 ns | **522 ns** | 1.49 |
| alternation | 1,761 ns | **1,410 ns** | 1.25 |
| char class | 656 ns | **628 ns** | 1.04 |
| quantifier | 1,385 ns | **1,031 ns** | 1.34 |
| group | 1,060 ns | **780 ns** | 1.36 |
| backref | 1,636 ns | **973 ns** | 1.68 |
| lookahead | 762 ns | **478 ns** | 1.59 |
| lookbehind | 709 ns | **542 ns** | 1.31 |
| named capture | 47,563 ns | **5,740 ns** | 8.29 |

### Analysis

**Where Rust wins (29 of 39 execution benchmarks):** Most execution
benchmarks are 10-30% faster than C. Quantifiers, backreferences,
case-insensitive matching, and RegSet searches all show consistent gains.
The likely explanation is Rust's `Vec<Operation>` layout (contiguous,
predictable) vs. C's pointer-chased operation arrays giving better cache
behavior in the VM loop.

**SIMD-accelerated forward search** is the standout result. The
`memchr` crate replaces hand-written byte loops in the search pipeline
with SIMD-vectorized scans (SSE2/AVX2 on x86-64, NEON on aarch64). The
impact is most visible in full-text no-match scanning, where the engine
must scan the entire haystack without finding a literal prefix:

- **no match 10 KB: 5.0x faster than C** (375 ns vs 1.9 µs)
- **no match 50 KB: 6.1x faster than C** (1.5 µs vs 9.3 µs)

**Where C wins:** No execution benchmark exceeds 1.67x. The remaining
gaps are:

1. **Named captures (1.67x)** -- the Rust capture-handling VM path has
   overhead from region ownership semantics (move in/out of search
   function) that C avoids with simple pointer passing.

2. **Script-specific Unicode properties (1.41x)** -- `\p{Greek}`
   and `\p{Cyrillic}` have overhead from Rust's bounds checking in the
   codepoint classification inner loop.

3. **Timestamp extraction (1.32-1.35x)** -- character-class map search
   for `\d` first-byte; C's byte-by-byte loop with the 256-entry map
   is hard to beat when the set has > 3 distinct bytes (SIMD dispatch
   only covers 1-3 byte sets).

4. **Lookbehind (1.08-1.09x)** -- effectively at parity with C.

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
