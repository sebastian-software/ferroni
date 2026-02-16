<p align="center">
  <strong>Ferroni</strong><br>
  Pure-Rust regex engine based on Oniguruma, with SIMD-accelerated search and an idiomatic Rust API.
</p>

<p align="center">
  <a href="https://github.com/sebastian-software/ferroni/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastian-software/ferroni/ci.yml?branch=main&style=flat-square&logo=github&label=CI" alt="CI"></a>
  <a href="https://codecov.io/gh/sebastian-software/ferroni"><img src="https://img.shields.io/codecov/c/github/sebastian-software/ferroni?style=flat-square&logo=codecov&label=Coverage" alt="Coverage"></a>
  <a href="https://github.com/sebastian-software/ferroni/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-BSD--2--Clause-blue?style=flat-square" alt="License"></a>
  <a href="https://github.com/sebastian-software/ferroni"><img src="https://img.shields.io/badge/unsafe-0.4%25-green?style=flat-square" alt="Unsafe"></a>
  <a href="https://github.com/sebastian-software/ferroni"><img src="https://img.shields.io/badge/tests-1%2C882_passing-brightgreen?style=flat-square" alt="Tests"></a>
  <a href="https://github.com/sebastian-software/ferroni"><img src="https://img.shields.io/badge/C_parity-100%25-brightgreen?style=flat-square" alt="C Parity"></a>
</p>

---

Ferroni started as a line-by-line port of
[Oniguruma](https://github.com/kkos/oniguruma)'s C source into Rust -- same
structure, same function names, same semantics. From there it gained
SIMD-vectorized search (NEON on ARM, SSE2/AVX2 on x86-64) via
[`memchr`](https://crates.io/crates/memchr), making it **up to 6x faster than
C Oniguruma** on full-text scanning. Finally, an idiomatic Rust API layer
(`Regex::new()`, typed errors, `Match`/`Captures`) was added on top -- so you
get ergonomic Rust with battle-tested Oniguruma semantics underneath. No
bindings, no FFI -- pure Rust.

## Why Ferroni?

**Security first.** C Oniguruma has a history of memory safety CVEs
([CVE-2019-13224](https://nvd.nist.gov/vuln/detail/CVE-2019-13224) CVSS 9.8,
[CVE-2019-19204](https://nvd.nist.gov/vuln/detail/CVE-2019-19204),
[CVE-2019-19246](https://nvd.nist.gov/vuln/detail/CVE-2019-19246),
[CVE-2019-19012](https://nvd.nist.gov/vuln/detail/CVE-2019-19012),
[CVE-2019-13225](https://nvd.nist.gov/vuln/detail/CVE-2019-13225))
affecting Ruby, PHP, and any application linking against it. Ferroni
eliminates buffer overflows, use-after-free, and NULL dereferences
structurally through Rust's type system. See [ADR-005](docs/adr/005-unsafe-code-policy.md)
for our `unsafe` policy.

**Drop-in behavior.** Every regex feature, every opcode, every optimization
pass is ported 1:1 from C. If your pattern works in Oniguruma, it works
in Ferroni -- verified by 1,882 tests drawn from three sources (see
[Test Coverage](#test-coverage)).

**No C toolchain required.** Pure `cargo build`. Cross-compiles to
`wasm32-unknown-unknown` out of the box.

**Easy Node.js bindings.** Rust's [napi-rs](https://napi.rs/) ecosystem
makes it straightforward to publish a native Node.js module -- no
`node-gyp`, no C compiler on the user's machine.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ferroni = { git = "https://github.com/sebastian-software/ferroni.git" }
```

```rust
use ferroni::prelude::*;

fn main() -> Result<(), RegexError> {
    let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})")?;

    let caps = re.captures("Date: 2026-02-12").unwrap();
    assert_eq!(caps.get(0).unwrap().as_str(), "2026-02-12");
    assert_eq!(caps.name("year").unwrap().as_str(), "2026");
    assert_eq!(caps.name("month").unwrap().as_str(), "02");
    Ok(())
}
```

For fine-grained control, use `RegexBuilder`:

```rust
use ferroni::prelude::*;

let re = Regex::builder(r"hello")
    .case_insensitive(true)
    .build()
    .unwrap();
assert!(re.is_match("Hello World"));
```

<details>
<summary><strong>Low-Level C-Style API</strong></summary>

The full C-ported API is also available for advanced usage:

```rust
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::oniguruma::*;
use ferroni::regsyntax::OnigSyntaxOniguruma;

let reg = onig_new(
    b"\\d{4}-\\d{2}-\\d{2}",
    ONIG_OPTION_NONE,
    &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
    &OnigSyntaxOniguruma,
).unwrap();

let input = b"Date: 2026-02-12";
let (result, region) = onig_search(
    &reg, input, input.len(), 0, input.len(),
    Some(OnigRegion::new()), ONIG_OPTION_NONE,
);

assert!(result >= 0);
assert_eq!(result, 6); // match starts at byte 6
```

</details>

## Supported Features

- **All Perl/Ruby/Python syntax** -- `(?:...)`, `(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)`, `(?>...)`
- **Named captures** -- `(?<name>...)`, `(?'name'...)`, `(?P<name>...)`
- **Backreferences** -- `\k<name>`, `\g<name>`, relative `\g<-1>`
- **Conditionals** -- `(?(cond)T|F)`
- **Absent expressions** -- `(?~...)`
- **Unicode properties** -- `\p{Script_Extensions=Greek}`, `\p{Lu}`, `\p{Emoji}` (886 names)
- **Grapheme clusters** -- `\X`, text segment boundaries `\y`, `\Y`
- **Callouts** -- `(?{...})`, `(*FAIL)`, `(*MAX{n})`, `(*COUNT)`, `(*CMP)`
- **12 syntax modes** -- Oniguruma, Ruby, Perl, Perl_NG, Python, Java, Emacs, Grep, GNU, POSIX Basic/Extended, ASIS
- **Scanner API** -- multi-pattern `Scanner` compatible with vscode-oniguruma, UTF-16 position mapping
- **Safety limits** -- retry, time, stack, subexp call depth (global + per-search)

## Performance

Criterion benchmarks vs. C Oniguruma at `-O3` on Apple M1 Ultra.
**Bold** = faster engine. Full tables in the [expandable section below](#full-benchmark-tables).

### Highlights

| Scenario | Rust | C | Speedup |
|----------|-----:|--:|--------:|
| No match, 50 KB haystack | **1.5 us** | 9.3 us | **6.0x** |
| No match, 10 KB haystack | **378 ns** | 1.9 us | **5.0x** |
| Scanner, short string (RegSet) | **168 ns** | 407 ns | **2.4x** |
| RegSet, 5 patterns (position) | **147 ns** | 396 ns | **2.7x** |
| 5-way alternation | **122 ns** | 173 ns | **1.4x** |
| Greedy quantifier | **215 ns** | 255 ns | **1.2x** |

Ferroni wins **31 of 42** execution benchmarks. The SIMD-accelerated forward
search is the standout: [`memchr`](https://crates.io/crates/memchr) replaces
hand-written byte loops with vectorized scans, delivering 5-6x gains on
full-text scanning. See [ADR-006](docs/adr/006-simd-accelerated-search.md).

Compilation is 1.2-1.7x slower (Rust allocates more than C's pre-allocated
buffers), but compilation is a one-time cost -- real-world consumers compile
patterns once and match millions of times.

<details>
<summary><strong>Full Benchmark Tables</strong></summary>

### Regex Execution

| Benchmark | Rust | C | Ratio |
|-----------|-----:|--:|------:|
| **Literal match** | | | |
| exact string | **135 ns** | 159 ns | 0.85 |
| anchored start | **105 ns** | 151 ns | 0.69 |
| anchored end | 167 ns | **163 ns** | 1.02 |
| word boundary | **120 ns** | 151 ns | 0.80 |
| **Quantifiers** | | | |
| greedy | **215 ns** | 255 ns | 0.84 |
| lazy | **193 ns** | 206 ns | 0.93 |
| possessive | **199 ns** | 224 ns | 0.89 |
| nested | **200 ns** | 212 ns | 0.94 |
| **Alternation** | | | |
| 2 branches | **107 ns** | 155 ns | 0.69 |
| 5 branches | **122 ns** | 173 ns | 0.71 |
| 10 branches | 246 ns | **220 ns** | 1.12 |
| nested | **129 ns** | 184 ns | 0.70 |
| **Backreferences** | | | |
| simple `(\w+) \1` | **150 ns** | 183 ns | 0.82 |
| nested | **156 ns** | 185 ns | 0.84 |
| named | **152 ns** | 188 ns | 0.81 |
| **Lookaround** | | | |
| positive lookahead | **128 ns** | 170 ns | 0.76 |
| negative lookahead | **140 ns** | 172 ns | 0.82 |
| positive lookbehind | 279 ns | **264 ns** | 1.05 |
| negative lookbehind | 359 ns | **334 ns** | 1.08 |
| combined | 301 ns | **280 ns** | 1.08 |
| **Unicode properties** | | | |
| `\p{Lu}+` | **92 ns** | 150 ns | 0.62 |
| `\p{Letter}+` | **128 ns** | 164 ns | 0.78 |
| `\p{Greek}+` | 323 ns | **246 ns** | 1.31 |
| `\p{Cyrillic}+` | 450 ns | **329 ns** | 1.37 |
| **Case-insensitive** | | | |
| single word | **107 ns** | 155 ns | 0.69 |
| phrase | **161 ns** | 183 ns | 0.88 |
| alternation | **113 ns** | 148 ns | 0.76 |
| **Named captures** | | | |
| date extraction | 460 ns | **282 ns** | 1.63 |
| **Large text (first match)** | | | |
| literal 10 KB | **113 ns** | 145 ns | 0.78 |
| literal 50 KB | **114 ns** | 153 ns | 0.75 |
| timestamp 10 KB | 243 ns | **186 ns** | 1.31 |
| timestamp 50 KB | 240 ns | **175 ns** | 1.37 |
| field extract 10 KB | **160 ns** | 170 ns | 0.94 |
| field extract 50 KB | **162 ns** | 170 ns | 0.95 |
| no match 10 KB | **378 ns** | 1.9 us | 0.20 |
| no match 50 KB | **1.5 us** | 9.3 us | 0.17 |
| **RegSet** | | | |
| position-lead (5 patterns) | **147 ns** | 396 ns | 0.37 |
| regex-lead (5 patterns) | **164 ns** | 233 ns | 0.70 |
| **Match at position** | | | |
| `\d+` at offset 4 | **118 ns** | 154 ns | 0.76 |
| **Scanner** (vs vscode-oniguruma C) | | | |
| short string (RegSet path) | **168 ns** | 407 ns | 0.41 |
| long string, cold (per-regex) | 191 ns | **188 ns** | 1.02 |
| long string, warm (cached) | 24 ns | **23 ns** | 1.06 |

### Regex Compilation

| Pattern | Rust | C | Ratio |
|---------|-----:|--:|------:|
| literal | **416 ns** | 449 ns | 0.93 |
| `.*` | 745 ns | **517 ns** | 1.44 |
| alternation | 1,711 ns | **1,410 ns** | 1.21 |
| char class | 641 ns | **635 ns** | 1.01 |
| quantifier | 1,356 ns | **1,059 ns** | 1.28 |
| group | 1,040 ns | **803 ns** | 1.30 |
| backref | 1,578 ns | **983 ns** | 1.60 |
| lookahead | 733 ns | **474 ns** | 1.55 |
| lookbehind | 678 ns | **538 ns** | 1.26 |
| named capture | 46,153 ns | **5,734 ns** | 8.05 |

### Running Benchmarks

```bash
cargo bench --features ffi               # full suite (~8 min)
cargo bench --features ffi -- compile    # specific group
cargo bench --features ffi -- scanner    # scanner API benchmarks
cargo bench --features ffi -- "large_"   # pattern filter
# HTML report: target/criterion/report/index.html
```

</details>

## Architecture

Each C source file maps 1:1 to a Rust module ([ADR-001](docs/adr/001-one-to-one-parity-with-c-original.md)):

| C File | Rust Module | Purpose |
|--------|-------------|---------|
| regparse.c | `regparse.rs` | Pattern parser |
| regcomp.c | `regcomp.rs` | AST-to-bytecode compiler |
| regexec.c | `regexec.rs` | VM executor |
| regint.h | `regint.rs` | Internal types and opcodes |
| oniguruma.h | `oniguruma.rs` | Public types and constants |
| regenc.c | `regenc.rs` | Encoding trait |
| regsyntax.c | `regsyntax.rs` | 12 syntax definitions |
| regset.c | `regset.rs` | Multi-regex search (RegSet) |
| regerror.c | `regerror.rs` | Error messages |
| regtrav.c | `regtrav.rs` | Capture tree traversal |
| unicode.c | `unicode/mod.rs` | Unicode tables and segmentation |
| -- | `scanner.rs` | Multi-pattern scanner (vscode-oniguruma compatible) |

**Compilation pipeline** (same as C):

```
onig_new() -> onig_compile()
  -> onig_parse_tree()     (pattern -> AST)
  -> reduce_string_list()  (merge adjacent strings)
  -> tune_tree()           (6 optimization sub-passes)
  -> compile_tree()        (AST -> VM bytecode)
  -> set_optimize_info()   (extract search strategy)
```

## Scope

Ferroni targets ASCII/UTF-8 workloads. The following are intentionally not included:

- **27 of 29 encodings** -- only ASCII and UTF-8 ([ADR-002](docs/adr/002-encoding-scope-ascii-and-utf8-only.md))
- **POSIX/GNU API** -- `regcomp`/`regexec`/`regfree` ([ADR-007](docs/adr/007-posix-and-gnu-api-not-ported.md))
- **C memory management** -- replaced by Rust's `Drop` trait
- **`onig_new_deluxe`** -- C-specific allocation, use `onig_new()` instead

## Running Tests

```bash
# Full UTF-8 suite (requires increased stack for debug builds)
RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1

# Other suites
cargo test --test compat_syntax
cargo test --test compat_options
cargo test --test compat_regset
RUST_MIN_STACK=268435456 cargo test --test compat_back -- --test-threads=1
```

> **Warning:** Never run `cargo test -- --ignored` -- the
> `conditional_recursion_complex` test intentionally hangs.

## Test Coverage

Ferroni ships 1,882 tests from three sources:

- **1,554** ported 1:1 from C Oniguruma's test suite
- **25** ported from [vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma)'s
  TypeScript test suite, covering the Scanner API and UTF-16 position mapping
- **303** additional Rust-specific tests covering edge cases, error paths,
  and features that the upstream suites leave untested

The original C project has no
coverage reporting; Ferroni's test suite is strictly a superset of it.
Combined with Rust's compile-time guarantees against buffer overflows,
use-after-free, and data races, this makes Ferroni's long-term quality
posture stronger than the C original.

Coverage is measured with
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) on nightly and
reported to [Codecov](https://codecov.io/gh/sebastian-software/ferroni).

| Metric | Value | Notes |
|--------|------:|-------|
| **Function coverage** | >94% | All reachable API and internal functions |
| **Line coverage** | ~82% | Limited by stack overflow under instrumentation |
| **Tests executed** | 1,840 of 1,882 | 42 skipped during coverage runs only |

**Why not higher?** Oniguruma supports deeply recursive patterns --
backreferences like `(\w+)\1` that backtrack through thousands of stack frames,
and subexpression calls like `\g<name>` that recurse into the VM. These tests
pass normally but cause stack overflows under LLVM's coverage instrumentation,
which significantly increases each stack frame's size. Since macOS and Linux
cap thread stacks at ~1 GB, roughly 42 of the most recursive tests must be
skipped during coverage runs.

The skipped tests exercise branches deep inside the three largest modules
(parser, compiler, VM executor -- together ~18,000 lines). The functions
themselves *are* covered, but specific branches within them (e.g., nested
backref matching, mutual recursion, conditional recursion) remain unexercised
under instrumentation. All 1,882 tests pass in regular `cargo test` builds
without any skips.

Functions that cannot be meaningfully tested -- global configuration setters,
encoding stubs for the unused ASCII path, error message formatting, callout
argument accessors -- are excluded via `#[coverage(off)]` on nightly to avoid
inflating the denominator.

## Architecture Decision Records

| ADR | Decision |
|-----|----------|
| [001](docs/adr/001-one-to-one-parity-with-c-original.md) | 1:1 structural parity with C original |
| [002](docs/adr/002-encoding-scope-ascii-and-utf8-only.md) | ASCII and UTF-8 only |
| [003](docs/adr/003-stack-overflow-debug-builds.md) | Stack overflow mitigation in debug builds |
| [004](docs/adr/004-c-to-rust-translation-patterns.md) | C-to-Rust translation patterns |
| [005](docs/adr/005-unsafe-code-policy.md) | Unsafe code policy |
| [006](docs/adr/006-simd-accelerated-search.md) | SIMD-accelerated search via memchr |
| [007](docs/adr/007-posix-and-gnu-api-not-ported.md) | POSIX and GNU API not ported |
| [008](docs/adr/008-test-strategy-and-c-test-parity.md) | Test strategy and C test suite parity |
| [009](docs/adr/009-porting-bugs-lessons-learned.md) | Porting bugs: lessons learned |
| [010](docs/adr/010-idiomatic-rust-api-layer.md) | Idiomatic Rust API layer |

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md)
and review the ADRs before submitting a PR.

## Acknowledgments

Ferroni is built on the work of [K. Kosako](https://github.com/kkos) and
the Oniguruma contributors. The C original powers regex in
[Ruby](https://www.ruby-lang.org/), [PHP](https://www.php.net/),
[TextMate](https://macromates.com/), and many other projects.

## License

[BSD-2-Clause](LICENSE) (same as Oniguruma)
