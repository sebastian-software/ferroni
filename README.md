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
  <a href="https://github.com/sebastian-software/ferroni"><img src="https://img.shields.io/badge/tests-2%2C090_passing-brightgreen?style=flat-square" alt="Tests"></a>
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
is ported 1:1 and verified by [2,090 tests](#test-coverage) from three
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
| **TypeScript** (279 patterns) | | | |
| Compile | **10.1 ms** | 16.8 ms | **1.7x** |
| First match | **414 ns** | 25.3 us | **61x** |
| Tokenize full line | **7.0 us** | 221 us | **32x** |
| **Rust** (81 patterns) | | | |
| Compile | 257 us | **181 us** | 0.7x |
| First match | **181 ns** | 5.6 us | **31x** |
| Tokenize full line | **8.2 us** | 82.2 us | **10x** |
| **CSS** (117 patterns) | | | |
| Compile | **13.7 ms** | 19.0 ms | **1.4x** |
| Tokenize full line | **1.60 ms** | 14.9 ms | **9.3x** |

### Text search and log scanning

First-match latency and full-scan rejection on log-sized inputs. The
[`regex`](https://crates.io/crates/regex) crate is included where the
pattern is compatible with its syntax.

| Scenario | Ferroni | C Oniguruma | `regex` |
|----------|--------:|------------:|--------:|
| Literal in 50 KB | 74 ns | 150 ns | **10 ns** |
| No match, 50 KB | 1.53 us | 9.5 us | **1.46 us** |
| No match, 10 KB | 357 ns | 1.96 us | **298 ns** |
| Field extract, 50 KB | 127 ns | 172 ns | **56 ns** |
| Timestamp, 50 KB | **120 ns** | 177 ns | **54 ns** |
| RegSet multi-pattern (5) | **101 ns** | 395 ns | — |

The `regex` crate's DFA engine gives it a clear advantage on text search
workloads. [`memchr`](https://crates.io/crates/memchr) (shared by both
Ferroni and `regex`) enables SIMD-accelerated literal scans, but `regex`
goes further with full DFA-based matching that avoids per-character
backtracking. RegSet multi-pattern has no direct `regex` equivalent.

### Pattern matching

One representative pattern per regex feature. **Bold** = fastest engine.
`regex` is omitted for features it does not support (lookaround,
backreferences).

| Category | Ferroni | C Oniguruma | `regex` |
|----------|--------:|------------:|--------:|
| Literal exact | 104 ns | 159 ns | **11 ns** |
| Quantifier greedy | 183 ns | 261 ns | **65 ns** |
| Lookaround combined | **83 ns** | 292 ns | — |
| Unicode `\p{Greek}+` | 96 ns | 251 ns | **60 ns** |
| Backref `(\w+) \1` | **79 ns** | 199 ns | — |
| Case-insensitive phrase | 101 ns | 188 ns | **62 ns** |
| Alternation, 2 branches | 62 ns | 157 ns | **48 ns** |
| Alternation, 10 branches | 49 ns | 225 ns | **21 ns** |
| Named capture date | 361 ns | 277 ns | **44 ns** |

### Compilation

Simple patterns compile within 5% of C. The `regex` crate compiles
significantly slower due to DFA construction -- the cost of its faster
matching. Lookbehind is not supported by `regex`.

| Pattern | Ferroni | C Oniguruma | `regex` |
|---------|--------:|------------:|--------:|
| Literal | **439 ns** | 448 ns | 2.33 us |
| Named capture | **4.67 us** | 5.78 us | 193 us |
| Lookbehind | 992 ns | **556 ns** | — |

### Where Ferroni is slower

- **vs `regex` crate** -- for patterns that `regex` supports, its DFA engine
  is 2-10x faster at matching (but 5-40x slower to compile)
- **Named capture extraction** -- 1.3x vs C (region bookkeeping overhead)
- **Scanner warm cache** -- 2.2x vs C (C's pointer comparison vs hash lookup)

### Ferroni vs the `regex` crate

The `regex` crate is faster at matching for all patterns it supports, thanks
to its DFA-based engine with guaranteed linear time. However, it compiles
5-40x slower and does not support: variable-length lookbehind,
backreferences, conditional patterns, absent expressions, subexpression
calls, named captures with multiple syntaxes (`(?<n>)`, `(?'n')`,
`(?P<n>)`), TextMate grammar support, or drop-in replacement for Ruby/PHP
regex behavior. Use [`regex`](https://crates.io/crates/regex) when your
patterns fit its syntax and compilation cost is amortized. Use Ferroni when
you need full Oniguruma compatibility.

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

2,090 tests from three independent sources verify every opcode, optimization
pass, and API surface:

| Source | Tests | What it covers |
|--------|------:|----------------|
| C Oniguruma test suite | 1,700 | Ported 1:1 -- UTF-8, syntax modes, options, backrefs, RegSet |
| [vscode-oniguruma](https://github.com/nicolo-ribaudo/vscode-oniguruma) | 25 | Scanner API, UTF-16 mapping, error handling |
| Rust-native tests | 365 | API integration, edge cases, error paths, coverage gaps |

C Oniguruma has no coverage reporting. Ferroni's test suite is a strict
superset of the upstream tests.

**Line coverage: 87%.** Measured with
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov), reported to
[Codecov](https://codecov.io/gh/sebastian-software/ferroni). 42 deeply
recursive tests are skipped under LLVM instrumentation (stack overflow from
coverage bookkeeping) but pass in normal `cargo test`.

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
