<p align="center">
  <strong>Ferroni</strong><br>
  Pure-Rust Oniguruma engine with built-in scanner for syntax highlighting.<br>
  One crate. No C toolchain. Drop-in compatible.
</p>

<p align="center">
  <a href="https://github.com/sebastian-software/ferroni/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastian-software/ferroni/ci.yml?branch=main&style=flat-square&logo=github&label=CI" alt="CI"></a>
  <a href="https://codspeed.io/sebastian-software/ferroni?utm_source=badge"><img src="https://img.shields.io/badge/CodSpeed-measured-blue?style=flat-square&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI+PHBhdGggZmlsbD0id2hpdGUiIGQ9Ik0yMy4zNSAxMi44NGEuODMuODMgMCAwIDAtLjE1LS42OWwtMS40LTEuNzdhLjgyLjgyIDAgMCAwLS42Ny0uMzJoLTEuNjFsLTEuNjQtMS44YS44My44MyAwIDAgMC0uNjItLjI3SDEwLjlhLjguOCAwIDAgMC0uNTguMjVsLTIuMiAyLjI1SDUuMzNhLjgzLjgzIDAgMCAwLS42LjI2TDIuMTYgMTMuNmEuODQuODQgMCAwIDAgLjYgMS40aDIuMjNsLTIuNjMgMi44YS44My44MyAwIDAgMCAuNjEgMS4zOWg0LjA1YS44My44MyAwIDAgMCAuNjEtLjI3bDMuMzMtMy42MWgyLjk2bC0zLjc5IDQuMDRhLjgyLjgyIDAgMCAwIC42MSAxLjM5aDQuMjRjLjIgMCAuNC0uMDguNTUtLjIybDMuNy0zLjZoMS4yN2wuOS43OGMuMi4yMy41Mi4zLjguMTdsMS44Mi0xLjE0YS44My44MyAwIDAgMCAuMzMtLjYxdi0xLjI3YS44My44MyAwIDAgMC0uMi0uNTN6Ii8+PC9zdmc+" alt="CodSpeed"></a>
  <a href="https://codecov.io/gh/sebastian-software/ferroni"><img src="https://img.shields.io/codecov/c/github/sebastian-software/ferroni?style=flat-square&logo=codecov&label=Coverage" alt="Coverage"></a>
  <a href="https://github.com/sebastian-software/ferroni/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-BSD--2--Clause-blue?style=flat-square" alt="License"></a>
  <a href="https://github.com/sebastian-software/ferroni"><img src="https://img.shields.io/badge/unsafe-0.4%25-green?style=flat-square" alt="Unsafe"></a>
  <a href="https://github.com/sebastian-software/ferroni"><img src="https://img.shields.io/badge/tests-1%2C882_passing-brightgreen?style=flat-square" alt="Tests"></a>
  <a href="https://github.com/sebastian-software/ferroni"><img src="https://img.shields.io/badge/C_parity-100%25-brightgreen?style=flat-square" alt="C Parity"></a>
</p>

---

Syntax highlighting in [VS Code](https://code.visualstudio.com/),
[Shiki](https://shiki.style/), and every editor built on
[TextMate grammars](https://macromates.com/manual/en/language_grammars)
runs on two things: an Oniguruma regex engine and a multi-pattern scanner.
Today, that means C code with native bindings via
[vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma).

Ferroni puts both into a single Rust crate. Same regex semantics, same
Scanner API, no C compiler needed. Just `cargo build`.

It is a line-by-line port of Oniguruma's C source -- same structure, same
opcodes, same optimization passes -- with SIMD-vectorized search via
[`memchr`](https://crates.io/crates/memchr) layered on top. The result:
**up to 6x faster than C** on full-text scanning, while an idiomatic Rust
API (`Regex::new()`, typed errors, `Match`/`Captures`) keeps the ergonomics
clean.

## Why Ferroni?

**Regex engine + scanner in one crate.** If you're building a syntax
highlighter, a TextMate grammar host, or anything that matches multiple
patterns against source code, you used to need C Oniguruma plus native
bindings. Ferroni gives you both the regex engine and the
[vscode-oniguruma-compatible Scanner API](#scanner-api) in a single
dependency. `cargo add ferroni` and you're done.

**No more CVEs from C.** C Oniguruma has a track record of memory safety
vulnerabilities --
[CVE-2019-13224](https://nvd.nist.gov/vuln/detail/CVE-2019-13224) (CVSS 9.8),
[CVE-2019-19204](https://nvd.nist.gov/vuln/detail/CVE-2019-19204),
[CVE-2019-19246](https://nvd.nist.gov/vuln/detail/CVE-2019-19246),
[CVE-2019-19012](https://nvd.nist.gov/vuln/detail/CVE-2019-19012),
[CVE-2019-13225](https://nvd.nist.gov/vuln/detail/CVE-2019-13225) --
affecting Ruby, PHP, and anything linking against it. Ferroni eliminates
buffer overflows, use-after-free, and NULL dereferences structurally through
Rust's type system. 0.4% unsafe code, all documented in
[ADR-005](docs/adr/005-unsafe-code-policy.md).

**Drop-in compatible.** If your pattern works in Oniguruma, it works in
Ferroni. Every opcode, every optimization pass is ported 1:1 from C and
verified by [1,882 tests](#test-coverage) from three independent sources.

**No C toolchain required.** Pure `cargo build`. Cross-compiles to
`wasm32-unknown-unknown`. Ship it as a Node.js native module via
[napi-rs](https://napi.rs/) without `node-gyp` or a C compiler on the
user's machine.

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
ferroni = "1"
```

### Regex

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

### Scanner API

The Scanner matches multiple patterns simultaneously -- the core operation
behind TextMate-based syntax highlighting. Results include UTF-16 position
mapping for direct use with vscode-textmate and Shiki.

```rust
use ferroni::scanner::{Scanner, ScannerFindOptions};

let mut scanner = Scanner::new(&[
    r"\b(function|const|let|var)\b",  // keywords
    r#""[^"]*""#,                      // strings
    r"//.*$",                          // comments
]).unwrap();

let code = r#"const x = "hello" // greeting"#;
let m = scanner.find_next_match(code, 0, ScannerFindOptions::NONE).unwrap();

assert_eq!(m.index, 0); // pattern 0 matched first ("const")
assert_eq!(m.capture_indices[0].start, 0);
assert_eq!(m.capture_indices[0].end, 5);
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
<summary><strong>Low-level C-style API</strong></summary>

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

## Supported features

**Scanner** -- multi-pattern matching with result caching, two search
strategies (RegSet for short strings, per-regex for long strings), and
automatic UTF-16 position mapping. API-compatible with
[vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma).

**Full Oniguruma regex** -- every feature from the C engine:

- All Perl/Ruby/Python syntax -- `(?:...)`, `(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)`, `(?>...)`
- Named captures -- `(?<name>...)`, `(?'name'...)`, `(?P<name>...)`
- Backreferences -- `\k<name>`, `\g<name>`, relative `\g<-1>`
- Conditionals -- `(?(cond)T|F)`
- Absent expressions -- `(?~...)`
- Unicode properties -- `\p{Script_Extensions=Greek}`, `\p{Lu}`, `\p{Emoji}` (886 names)
- Grapheme clusters -- `\X`, text segment boundaries `\y`, `\Y`
- Callouts -- `(?{...})`, `(*FAIL)`, `(*MAX{n})`, `(*COUNT)`, `(*CMP)`
- 12 syntax modes -- Oniguruma, Ruby, Perl, Perl_NG, Python, Java, Emacs, Grep, GNU, POSIX Basic/Extended, ASIS
- Safety limits -- retry, time, stack, subexp call depth (global + per-search)

## Performance

Ferroni wins **31 of 42** execution benchmarks against C Oniguruma at `-O3`.
Of the remaining 11, five are within noise (<10%) and only four show
measurable differences. Criterion, Apple M1 Ultra. **Bold** = faster engine.

### Highlights

| Scenario | Ferroni | C Oniguruma | Factor |
|----------|--------:|------------:|-------:|
| Full-text scan, no match, 50 KB | **1.6 us** | 9.4 us | **5.9x** |
| Full-text scan, no match, 10 KB | **384 ns** | 1.9 us | **4.9x** |
| Scanner, short string | **168 ns** | 424 ns | **2.5x** |
| Multi-pattern RegSet | **153 ns** | 404 ns | **2.6x** |
| Scanner, warm cache | 24 ns | **23 ns** | 1.04x |

### Scanner with real TextMate grammars (62 patterns)

Syntax highlighters like [Shiki](https://shiki.style/) compile 50-150+
patterns per grammar rule. These benchmarks use 62 actual TypeScript
expression patterns from a Shiki grammar:

| Scenario | Ferroni | C Oniguruma | Factor |
|----------|--------:|------------:|-------:|
| Compile 62 patterns | **1.2 ms** | 2.8 ms | **2.3x** |
| Match, short line (72 chars) | **505 ns** | 6.0 us | **11.9x** |
| Tokenize full line (13 tokens) | **46 us** | 101 us | **2.2x** |

The largest gains come from SIMD-vectorized search via
[`memchr`](https://crates.io/crates/memchr) -- NEON on ARM, SSE2/AVX2 on
x86-64 -- replacing C's hand-written byte loops with vectorized scans.
See [ADR-006](docs/adr/006-simd-accelerated-search.md).

The Scanner warm path (all patterns served from cache, the steady-state in a
syntax highlighter) runs at 24 ns -- within 4% of the C implementation. No
heap allocation on cache hits.

Compilation is 0.9-1.4x of C for simple patterns. Named captures with
Unicode character classes (e.g. `\d`, `\w`) benefit from batch range
compilation and are now faster than C.

<details>
<summary><strong>Full benchmark tables</strong></summary>

### Regex execution

| Benchmark | Rust | C | Ratio |
|-----------|-----:|--:|------:|
| **Literal match** | | | |
| exact string | **139 ns** | 154 ns | 0.90 |
| anchored start | **108 ns** | 147 ns | 0.73 |
| anchored end | 171 ns | **157 ns** | 1.09 |
| word boundary | **123 ns** | 155 ns | 0.79 |
| **Quantifiers** | | | |
| greedy | **220 ns** | 264 ns | 0.83 |
| lazy | **198 ns** | 222 ns | 0.89 |
| possessive | **202 ns** | 237 ns | 0.85 |
| nested | **205 ns** | 241 ns | 0.85 |
| **Alternation** | | | |
| 2 branches | **110 ns** | 155 ns | 0.71 |
| 5 branches | **124 ns** | 180 ns | 0.69 |
| 10 branches | 250 ns | **227 ns** | 1.10 |
| nested | **131 ns** | 176 ns | 0.74 |
| **Backreferences** | | | |
| simple `(\w+) \1` | **155 ns** | 190 ns | 0.82 |
| nested | **161 ns** | 199 ns | 0.81 |
| named | **155 ns** | 194 ns | 0.80 |
| **Lookaround** | | | |
| positive lookahead | **132 ns** | 166 ns | 0.80 |
| negative lookahead | **147 ns** | 183 ns | 0.80 |
| positive lookbehind | 286 ns | **264 ns** | 1.08 |
| negative lookbehind | 375 ns | **336 ns** | 1.12 |
| combined | 311 ns | **290 ns** | 1.07 |
| **Unicode properties** | | | |
| `\p{Lu}+` | **95 ns** | 147 ns | 0.65 |
| `\p{Letter}+` | **133 ns** | 160 ns | 0.83 |
| `\p{Greek}+` | 328 ns | **246 ns** | 1.33 |
| `\p{Cyrillic}+` | 454 ns | **338 ns** | 1.34 |
| **Case-insensitive** | | | |
| single word | **109 ns** | 161 ns | 0.68 |
| phrase | **164 ns** | 214 ns | 0.77 |
| alternation | **116 ns** | 160 ns | 0.73 |
| **Named captures** | | | |
| date extraction | 472 ns | **277 ns** | 1.70 |
| **Large text (first match)** | | | |
| literal 10 KB | **118 ns** | 153 ns | 0.77 |
| literal 50 KB | **118 ns** | 153 ns | 0.77 |
| timestamp 10 KB | 252 ns | **186 ns** | 1.35 |
| timestamp 50 KB | 252 ns | **188 ns** | 1.34 |
| field extract 10 KB | **165 ns** | 172 ns | 0.96 |
| field extract 50 KB | **167 ns** | 182 ns | 0.92 |
| no match 10 KB | **384 ns** | 1.9 us | 0.20 |
| no match 50 KB | **1.6 us** | 9.4 us | 0.17 |
| **RegSet** | | | |
| position-lead (5 patterns) | **153 ns** | 404 ns | 0.38 |
| regex-lead (5 patterns) | **167 ns** | 227 ns | 0.74 |
| **Match at position** | | | |
| `\d+` at offset 4 | **121 ns** | 150 ns | 0.81 |
| **Scanner** (vs vscode-oniguruma C) | | | |
| short string (RegSet path) | **168 ns** | 424 ns | 0.40 |
| long string, cold (per-regex) | 196 ns | **187 ns** | 1.05 |
| long string, warm (cached) | 24 ns | **23 ns** | 1.04 |

### Regex compilation

| Pattern | Rust | C | Ratio |
|---------|-----:|--:|------:|
| literal | **421 ns** | 458 ns | 0.92 |
| `.*` | 754 ns | **533 ns** | 1.41 |
| alternation | 1,800 ns | **1,446 ns** | 1.24 |
| char class | 660 ns | **645 ns** | 1.02 |
| quantifier | 1,376 ns | **1,048 ns** | 1.31 |
| group | 1,054 ns | **788 ns** | 1.34 |
| backref | 1,157 ns | **990 ns** | 1.17 |
| lookahead | 761 ns | **489 ns** | 1.56 |
| lookbehind | 712 ns | **565 ns** | 1.26 |
| named capture | **4,100 ns** | 6,000 ns | 0.68 |

### Running benchmarks

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
| -- | `scanner.rs` | Multi-pattern scanner for syntax highlighting |

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

## Running tests

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

## Test coverage

1,882 tests from three independent sources:

- **1,554** ported 1:1 from C Oniguruma's test suite
- **25** from [vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma)'s
  TypeScript tests (Scanner API, UTF-16 mapping)
- **303** Rust-specific tests for edge cases, error paths, and gaps in the
  upstream suites

C Oniguruma has no coverage reporting. Ferroni's test suite is a strict
superset.

| Metric | Value | Notes |
|--------|------:|-------|
| Function coverage | >94% | All reachable API and internal functions |
| Line coverage | ~82% | 42 deeply recursive tests overflow under LLVM instrumentation |
| Tests executed | 1,840 of 1,882 | All 1,882 pass in normal `cargo test` |

Coverage measured with
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov), reported to
[Codecov](https://codecov.io/gh/sebastian-software/ferroni).

## Architecture decision records

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
[TextMate](https://macromates.com/), and many other projects. The Scanner
API and its test suite are based on
[vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma)
by [Nicol&ograve; Ribaudo](https://github.com/nicolo-ribaudo) and the
VS Code team.

## License

[BSD-2-Clause](LICENSE) (same as Oniguruma)

---

Copyright 2026 [Sebastian Software GmbH](https://www.sebastian-software.de/)
