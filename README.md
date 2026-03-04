<p align="center">
  <strong>Ferroni</strong><br>
  Pure-Rust Oniguruma regex engine. Full feature set, no C toolchain, drop-in compatible.<br>
  Includes a multi-pattern scanner for TextMate grammar tokenization.
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

[Oniguruma](https://github.com/kkos/oniguruma) is the regex engine behind
[Ruby](https://www.ruby-lang.org/), [PHP](https://www.php.net/) (mbstring),
[TextMate](https://macromates.com/) grammars, and tools like
[jq](https://jqlang.github.io/jq/). It supports features that most regex
libraries don't: named captures with multiple syntaxes, look-behind of
variable length, conditional patterns, absent expressions, 886 Unicode
properties, subexpression calls, and 12 syntax modes from Perl to POSIX.

Ferroni is a line-by-line Rust port of this engine — same structure, same
opcodes, same optimization passes — with SIMD-vectorized search via
[`memchr`](https://crates.io/crates/memchr) layered on top. The result:
**up to 61x faster than C** on scanner first-match, while an idiomatic Rust
API (`Regex::new()`, typed errors, `Match`/`Captures`) keeps the ergonomics
clean.

For syntax highlighting, Ferroni also includes a multi-pattern
[Scanner API](#scanner-api) compatible with
[vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma),
used by [Shiki](https://shiki.style/), VS Code, and other TextMate-based
highlighters.

## Why Ferroni?

**Full Oniguruma, pure Rust.** Named captures, variable-length look-behind,
conditionals, absent expressions, Unicode properties, subexpression calls —
everything the C engine supports, without linking against C. If your pattern
works in Oniguruma, it works in Ferroni. Every opcode and optimization pass
is ported 1:1 and verified by [1,882 tests](#test-coverage) from three
independent sources.

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
[ADR-002](docs/adr/002-unsafe-code-policy.md).

**No C toolchain required.** Pure `cargo build`. Cross-compiles to
`wasm32-unknown-unknown`. Ship it as a Node.js native module via
[napi-rs](https://napi.rs/) without `node-gyp` or a C compiler on the
user's machine.

**Built-in multi-pattern scanner.** For syntax highlighting with TextMate
grammars, Ferroni includes a
[vscode-oniguruma-compatible Scanner API](#scanner-api) — regex engine and
scanner in a single dependency. `cargo add ferroni` and you're done.

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

All numbers compare Ferroni against C Oniguruma at `-O3`,
measured with [Criterion](https://github.com/bheisler/criterion.rs) on
Apple M1 Ultra. **Bold** = faster engine. See
[full tables](docs/perf/benchmark-results.md) for all benchmarks.

### Syntax highlighting

Syntax highlighters like [Shiki](https://shiki.style/) compile a full
TextMate grammar -- hundreds of regex patterns -- and scan each line
token by token. We benchmark against complete, unmodified Shiki grammars
for TypeScript (279 patterns), CSS (117 patterns), and Rust (81 patterns).
No cherry-picked subsets.

| Scenario | Ferroni | C Oniguruma | Speedup |
|----------|--------:|------------:|--------:|
| Compile 279 TS patterns | **10.3 ms** | 17.0 ms | **1.6x** |
| TS first match, short line | **421 ns** | 25.5 us | **61x** |
| TS tokenize full line | **7.0 us** | 224 us | **32x** |
| Compile 81 Rust patterns | 256 us | **180 us** | 0.7x |
| Rust first match | **184 ns** | 5.7 us | **31x** |
| Rust tokenize full line | **8.3 us** | 84.9 us | **10x** |
| Compile 117 CSS patterns | **14 ms** | 19 ms | **1.4x** |
| CSS tokenize (117 patterns) | **1.67 ms** | 15.3 ms | **9.2x** |
| Warm cache (steady-state) | 62 ns | **23 ns** | 0.4x |

The warm-cache path (all patterns served from cache) is the steady state in
a highlighter. Ferroni runs it at 62 ns with zero heap allocation; C is
faster here at 23 ns because its cache lookup is a single pointer comparison.

CSS grammar compilation was a known weak spot (399 ms vs 19 ms in C) but
has been resolved -- inverted case-fold iteration and skip-if-present
optimization brought it down to **14 ms** (1.4x faster than C).

### Text search and log scanning

First-match latency and full-scan rejection on log-sized inputs.

| Scenario | Ferroni | C Oniguruma | Speedup |
|----------|--------:|------------:|--------:|
| Literal in 50 KB | **122 ns** | 147 ns | **1.2x** |
| No match, 50 KB | **1.55 us** | 9.4 us | **6.1x** |
| No match, 10 KB | **385 ns** | 1.9 us | **5.0x** |
| Field extract, 50 KB | **162 ns** | 172 ns | **1.1x** |
| Timestamp, 50 KB | 236 ns | **176 ns** | 0.7x |
| RegSet multi-pattern (5) | **101 ns** | 400 ns | **4.0x** |

No-match rejection shows the largest SIMD gains --
[`memchr`](https://crates.io/crates/memchr) scans the entire buffer with
NEON/AVX2 vectorized byte matching instead of per-character loops. Timestamp
patterns without a literal prefix favor C's inner loop -- no leading byte
for SIMD to latch onto.

### Pattern matching

One representative pattern per regex feature. Ferroni is faster in 7 of 9
categories; C leads on long alternation and named capture extraction.

| Category | Ferroni | C | Speedup |
|----------|--------:|--:|--------:|
| Literal exact | **144 ns** | 148 ns | **1.0x** |
| Quantifier greedy | **240 ns** | 279 ns | **1.2x** |
| Lookaround combined | **136 ns** | 288 ns | **2.1x** |
| Unicode `\p{Greek}+` | **146 ns** | 245 ns | **1.7x** |
| Backref `(\w+) \1` | **135 ns** | 191 ns | **1.4x** |
| Case-insensitive phrase | **154 ns** | 187 ns | **1.2x** |
| Alternation, 2 branches | **116 ns** | 159 ns | **1.4x** |
| Alternation, 10 branches | 253 ns | **231 ns** | 0.9x |
| Named capture date | 499 ns | **277 ns** | 0.6x |

### Compilation

Simple patterns compile within 5% of C. Complex patterns with groups and
lookbehind run 1.3-1.8x slower. Named captures with Unicode character classes
(`\d`, `\w`) compile faster than C because Ferroni merges Unicode ranges
in a single pass instead of inserting them one at a time.

### Where Ferroni is slower

- **Alternation with 5+ branches** -- C advantage 1.1-1.4x
- **Named capture extraction** -- 1.8x (region bookkeeping overhead)
- **Timestamp in large text** -- 1.3x (no literal prefix for SIMD to latch onto)
- **Scanner warm cache** -- 2.7x (C's pointer comparison vs Ferroni's hash lookup)

### Ferroni vs the `regex` crate

If your patterns fit Perl-compatible syntax and you want guaranteed linear
time, use [regex](https://crates.io/crates/regex). Ferroni covers what
`regex` does not: named captures with multiple syntaxes
(`(?<n>)`, `(?'n')`, `(?P<n>)`), variable-length lookbehind, conditional
patterns, absent expressions, subexpression calls, TextMate grammar support,
or drop-in replacement for Ruby/PHP regex behavior.

<details>
<summary><strong>Running benchmarks</strong></summary>

```bash
cargo bench --features ffi                          # full suite (~8 min)
cargo bench --features ffi -- scanner_highlighting  # tier 1: highlighting
cargo bench --features ffi -- text_scanning         # tier 1: log scanning
cargo bench --features ffi -- single_pattern        # tier 1: per-feature
cargo bench --features ffi -- compilation           # tier 1: compile time
cargo bench --features ffi -- regression_           # tier 2: all regression
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

- **27 of 29 encodings** -- only ASCII and UTF-8 ([ADR-003](docs/adr/003-encoding-scope-ascii-and-utf8-only.md))
- **POSIX/GNU API** -- `regcomp`/`regexec`/`regfree` ([ADR-012](docs/adr/012-posix-and-gnu-api-not-ported.md))
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
| [002](docs/adr/002-unsafe-code-policy.md) | Unsafe code policy |
| [003](docs/adr/003-encoding-scope-ascii-and-utf8-only.md) | Encoding scope: ASCII and UTF-8 only |
| [004](docs/adr/004-c-to-rust-translation-patterns.md) | C-to-Rust translation patterns |
| [005](docs/adr/005-idiomatic-rust-api-layer.md) | Idiomatic Rust API layer |
| [006](docs/adr/006-scanner-api.md) | Scanner API for TextMate tokenization |
| [007](docs/adr/007-simd-accelerated-search.md) | SIMD-accelerated search via memchr |
| [008](docs/adr/008-rust-only-optimizations.md) | Rust-only optimizations and performance philosophy |
| [009](docs/adr/009-dependency-philosophy.md) | Dependency philosophy |
| [010](docs/adr/010-benchmark-strategy.md) | Benchmark strategy |
| [011](docs/adr/011-test-strategy-and-c-test-parity.md) | Test strategy and C test suite parity |
| [012](docs/adr/012-posix-and-gnu-api-not-ported.md) | POSIX and GNU API not ported |
| [013](docs/adr/013-stack-overflow-debug-builds.md) | Stack overflow mitigation in debug builds |
| [014](docs/adr/014-porting-bugs-lessons-learned.md) | Porting bugs: lessons learned |

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md)
and review the ADRs before submitting a PR.

## Acknowledgments

Ferroni is built on the work of [K. Kosako](https://github.com/kkos) and
the Oniguruma contributors. The C original powers regex in
[Ruby](https://www.ruby-lang.org/), [PHP](https://www.php.net/),
[TextMate](https://macromates.com/), [jq](https://jqlang.github.io/jq/),
and many other projects. The Scanner API and its test suite are based on
[vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma)
by [Nicol&ograve; Ribaudo](https://github.com/nicolo-ribaudo) and the
VS Code team.

## License

[BSD-2-Clause](LICENSE) (same as Oniguruma)

---

Copyright 2026 [Sebastian Software GmbH](https://www.sebastian-software.de/)
