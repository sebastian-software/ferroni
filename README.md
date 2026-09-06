<p align="center">
  <strong>Ferroni</strong><br>
  Pure-Rust Oniguruma-compatible regex engine &mdash; same feature class, no C toolchain.
</p>

<p align="center">
  <a href="https://crates.io/crates/ferroni"><img src="https://img.shields.io/crates/v/ferroni?style=flat-square&logo=rust&label=crates.io" alt="crates.io"></a>
  <a href="https://docs.rs/ferroni"><img src="https://img.shields.io/docsrs/ferroni?style=flat-square&logo=docsdotrs&label=docs.rs" alt="docs.rs"></a>
  <a href="https://github.com/sebastian-software/ferroni/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastian-software/ferroni/ci.yml?branch=main&style=flat-square&logo=github&label=CI" alt="CI"></a>
  <a href="https://codecov.io/gh/sebastian-software/ferroni"><img src="https://img.shields.io/codecov/c/github/sebastian-software/ferroni?style=flat-square&logo=codecov&label=Coverage" alt="Coverage"></a>
  <a href="CONTRIBUTING.md#getting-started"><img src="https://img.shields.io/badge/MSRV-1.94-blue?style=flat-square&logo=rust" alt="MSRV 1.94"></a>
  <a href="https://github.com/sebastian-software/ferroni/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-BSD--2--Clause-blue?style=flat-square" alt="License"></a>
  <a href="https://codspeed.io/sebastian-software/ferroni?utm_source=badge"><img src="https://img.shields.io/badge/CodSpeed-measured-blue?style=flat-square&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI+PHBhdGggZmlsbD0id2hpdGUiIGQ9Ik0yMy4zNSAxMi44NGEuODMuODMgMCAwIDAtLjE1LS42OWwtMS40LTEuNzdhLjgyLjgyIDAgMCAwLS42Ny0uMzJoLTEuNjFsLTEuNjQtMS44YS44My44MyAwIDAgMC0uNjItLjI3SDEwLjlhLjguOCAwIDAgMC0uNTguMjVsLTIuMiAyLjI1SDUuMzNhLjgzLjgzIDAgMCAwLS42LjI2TDIuMTYgMTMuNmEuODQuODQgMCAwIDAgLjYgMS40aDIuMjNsLTIuNjMgMi44YS44My44MyAwIDAgMCAuNjEgMS4zOWg0LjA1YS44My44MyAwIDAgMCAuNjEtLjI3bDMuMzMtMy42MWgyLjk2bC0zLjc5IDQuMDRhLjgyLjgyIDAgMCAwIC42MSAxLjM5aDQuMjRjLjIgMCAuNC0uMDguNTUtLjIybDMuNy0zLjZoMS4yN2wuOS43OGMuMi4yMy41Mi4zLjguMTdsMS44Mi0xLjE0YS44My44MyAwIDAgMCAuMzMtLjYxdi0xLjI3YS44My44MyAwIDAgMC0uMi0uNTN6Ii8+PC9zdmc+" alt="CodSpeed"></a>
  <a href="https://oss.sebastian-software.com"><img src="https://img.shields.io/badge/Powered%20by-Sebastian%20Software-00718d?style=flat-square" alt="Powered by Sebastian Software"></a>
</p>

<p align="center">
  Evidence, not badges:
  <a href="#test-parity">test parity with the C suite</a> &middot;
  <a href="https://codecov.io/gh/sebastian-software/ferroni">coverage report</a> &middot;
  <a href="https://sebastian-software.github.io/ferroni/adr/002-unsafe-code-policy">unsafe code policy</a>
</p>

---

[Oniguruma](https://github.com/kkos/oniguruma) is the regex engine behind
[Ruby](https://www.ruby-lang.org/), [PHP](https://www.php.net/) (mbstring),
[TextMate](https://macromates.com/) grammars, and tools like
[jq](https://jqlang.github.io/jq/). It supports features that most regex
libraries don't: named captures with multiple syntaxes, look-behind of
variable length, conditional patterns, absent expressions, 886 Unicode
properties, subexpression calls, and 12 syntax modes from Perl to POSIX.

Ferroni started with a practical goal: build a fast Rust core for
syntax-highlighting and other scanner-heavy workloads without falling back to
C. That requirement led straight to Oniguruma compatibility, because the
surrounding ecosystems depend on features most regex engines skip.

So Ferroni does not wrap Oniguruma. It ports the engine into Rust, keeps the
same structure and optimization pipeline, and then tunes the runtime path
hard with tools like [`memchr`](https://crates.io/crates/memchr). In the
current reference suite, Ferroni is ahead of Oniguruma across the measured
runtime cases while staying in the same peak-memory class on the large
TypeScript scanner workload.

For syntax highlighting, Ferroni also includes a multi-pattern
[Scanner API](#scanner-api) compatible with
[vscode-oniguruma](https://github.com/microsoft/vscode-oniguruma),
used by [Shiki](https://shiki.style/), VS Code, and other TextMate-based
highlighters.

## Why Ferroni?

**Built for runtime performance.** Ferroni was driven by the need for a fast
Rust core for syntax highlighting, not by a generic "rewrite C in Rust"
exercise. In the current `battle_bench` reference suite it is ahead of
Oniguruma across all measured runtime cases: scanner first-match, full-line
tokenization, practical text scanning, and representative feature-heavy
matching. On the measured large TypeScript scanner workload, peak RSS stays
in the same ~15 MB class as Oniguruma.

**Full Oniguruma compatibility.** Named captures, variable-length
look-behind, conditionals, absent expressions, Unicode properties,
subexpression calls — everything the C engine supports, without linking
against C. If your pattern works in Oniguruma, it works in Ferroni. Every
opcode and optimization pass is ported 1:1 and verified by the
[ported upstream test suite](#test-parity) -- including every upstream UTF-8
test from both Oniguruma and vscode-oniguruma.

**Rust improves the operational story.** Pure `cargo build`.
Cross-compiles to `wasm32-unknown-unknown`. Easier to package in Rust-native
stacks and downstream bindings, including N-API modules, without `node-gyp`
or a local C compiler.
Only the optional `ffi` feature — the C-vs-Rust benchmark harness, not
needed to use the library — requires a local Oniguruma source snapshot
(`./scripts/prepare-oniguruma-sources.sh` or `FERRONI_ONIGURUMA_DIR`). Rust also removes whole classes of C
memory bugs structurally; C Oniguruma has a long history of memory-safety
CVEs, while Ferroni keeps `unsafe` at 0.4%, all documented in
[ADR-002](https://sebastian-software.github.io/ferroni/adr/002-unsafe-code-policy).

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

MSRV: Rust 1.94 (enforced in CI).

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

Both APIs have a runnable example in [`examples/`](examples):

```bash
cargo run --example basic_match       # matching, captures, iteration
cargo run --example scanner_tokenize  # multi-pattern scanner, UTF-16 offsets
```

## Supported features

**Scanner** -- multi-pattern matching with result caching, two search
strategies (RegSet for short strings, per-regex for long strings), and
automatic UTF-16 position mapping. API-compatible with
[vscode-oniguruma](https://github.com/microsoft/vscode-oniguruma).

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

This section is based on a fresh `battle_bench` run on a quiet machine. The
README keeps the numbers rounded and human-readable. Exact raw tables and
measurement context live in
[Benchmark Results](https://sebastian-software.github.io/ferroni/perf/benchmark-results).

The headline is simple: once compiled, Ferroni is ahead of Oniguruma across
the measured runtime paths in the current reference suite. The detail below
shows where that lead comes from, and where compile time still needs work.

### Syntax highlighting

Syntax highlighters like [Shiki](https://shiki.style/) compile a full
TextMate grammar -- hundreds of regex patterns -- and scan each line
token by token. We benchmark against complete, unmodified Shiki grammars
for TypeScript (279 patterns), CSS (117 patterns), and Rust (81 patterns).
No cherry-picked subsets.

| Scenario | Why it matters | Ferroni | Oniguruma | Takeaway |
|----------|----------------|--------:|------------:|----------|
| TypeScript grammar compile | Startup cost for a full Shiki grammar | ~12 ms | ~17 ms | Ferroni starts faster even on a full production grammar |
| TypeScript first match | Time to find the next token on a real grammar | ~425 ns | ~25 us | First-token latency is dramatically lower |
| TypeScript tokenize full line | End-to-end line tokenization cost | ~6.9 us | ~217 us | Real scanner throughput is in a different class |
| Rust grammar compile | Compile cost on a smaller, real grammar | ~315 us | ~195 us | One of the smaller-grammar startup cases that still favors Oniguruma |
| Rust first match | First-token latency on another production grammar | ~165 ns | ~5.5 us | The scanner win is not TypeScript-only |
| Rust tokenize full line | Whole-line scanner work on a real grammar | ~8.3 us | ~78 us | Whole-line scanner work still stays much faster |
| CSS tokenize representative input | Heavier multi-pattern scanner workload | ~1.3 ms | ~14.7 ms | Even heavier scanner workloads stay about an order of magnitude faster |

### Text search and log scanning

First-match latency and rejection speed on log-sized inputs:

| Scenario | Why it matters | Ferroni | Oniguruma | Takeaway |
|----------|----------------|--------:|------------:|----------|
| Literal in 50 KB | Plain substring-like scanning in a real log buffer | <75 ns | ~130 ns | Both are instant; Ferroni is still ahead |
| No match, 50 KB | Rejection cost when the pattern is absent | ~1.5 us | ~9.2 us | Rejection speed is a very strong Ferroni win |
| No match, 10 KB | Same rejection story on smaller log chunks | ~370 ns | ~1.9 us | The no-match advantage also holds on smaller buffers |
| Field extract, 50 KB | Practical capture-based scanning | ~105 ns | ~165 ns | Useful extraction stays cheap |
| Timestamp, 50 KB | Structured log parsing | <85 ns | ~160 ns | Everyday log parsing remains very fast |
| RegSet multi-pattern (5) | Multi-pattern search, relevant for scanners | <100 ns | ~385 ns | One of the clearest Ferroni-vs-Oniguruma wins |

For plain-text workloads that fit Rust's
[`regex`](https://crates.io/crates/regex) syntax, `regex` still wins on raw
matching speed. Ferroni's goal here is "full Oniguruma compatibility without
giving up practical throughput", not "beat a DFA engine at its own game."

### Pattern matching

One representative pattern per feature family:

| Pattern | Why it matters | Ferroni | Oniguruma | Takeaway |
|---------|----------------|--------:|------------:|----------|
| Literal exact | Baseline single-pattern matching | ~95 ns | ~135 ns | Ferroni is ahead, but this is not the headline story |
| Quantifier greedy | Classic backtracking-heavy pattern | ~150 ns | ~240 ns | Everyday regex engine work is also faster |
| Lookaround combined | A feature many Rust regex engines do not support | <80 ns | ~280 ns | Full features do not mean slow by default |
| Unicode `\p{Greek}+` | Unicode-property support on real text | ~96 ns | ~235 ns | Unicode-property support stays fast |
| Backref `(\w+) \1` | Backreferences are a real compatibility differentiator | <80 ns | ~170 ns | A strong compatibility showcase without a speed penalty |
| Alternation, 10 branches | Branch-heavy matching | ~50 ns | ~230 ns | Optimized search paths pay off strongly |
| Named capture date | Practical extraction pattern | ~240 ns | ~280 ns | Still close, but Ferroni keeps a lead in this sample |

### Compilation

Compilation should stay short and pragmatic in the README. We only need three
reference points:

| Pattern | Why it matters | Ferroni | Oniguruma | Takeaway |
|---------|----------------|--------:|------------:|----------|
| Literal | Smallest possible compile path | ~520 ns | ~535 ns | Effectively tied |
| Named capture | More realistic structured pattern | ~6.2 us | ~6.4 us | Ferroni stays competitive on practical compile paths |
| Lookbehind | Feature-heavy compile path | ~1.2 us | ~640 ns | One of the compile paths that still favors Oniguruma |

### Memory footprint

Speed is only half the story. We also measure peak RSS in isolated Rust and
Oniguruma processes on a large TypeScript scanner workload: compile the full
TypeScript grammar, then scan a large TypeScript file line by line. Exact
method and raw numbers live in
[Memory Measurements](https://sebastian-software.github.io/ferroni/perf/memory-measurements).

| Scenario | Why it matters | Ferroni | Oniguruma | Takeaway |
|---------|----------------|--------:|------------:|----------|
| Full TS grammar compile | Memory cost before any scanning starts | ~15 MB | ~14.5 MB | Same ballpark; Rust is not paying a large memory tax |
| Compile + scan large TS file | Practical peak RSS for a realistic scanner pass | ~15 MB | ~14.5 MB | Ferroni stays memory-competitive while being much faster on scanner workloads |

Oniguruma is slightly lower in the current local sample, but the important
point is that both engines land in the same range rather than a different
memory class.

### Where Ferroni is slower

- **vs `regex` crate** -- if your pattern fits `regex`, its DFA engine is still
  the right tool for maximum plain-text match throughput
- **Feature-heavy compile paths** -- lookbehind compile still favors
  Oniguruma
- **Some smaller grammar startup cases** -- Rust grammar compile is still one
  of the places where Oniguruma can look better

### Ferroni vs the `regex` crate

The `regex` crate is usually faster on pure matching for the patterns it
supports, thanks to its DFA-based engine with guaranteed linear time.
However, it compiles 5-40x slower and does not support: variable-length lookbehind,
backreferences, conditional patterns, absent expressions, subexpression
calls, named captures with multiple syntaxes (`(?<n>)`, `(?'n')`,
`(?P<n>)`), TextMate grammar support, or drop-in replacement for Ruby/PHP
regex behavior. Use [`regex`](https://crates.io/crates/regex) when your
patterns fit its syntax and compilation cost is amortized. Use Ferroni when
you need full Oniguruma compatibility.

### Refresh Checklist

When we refresh `battle_bench` on a clean machine, these are the README rows
to revisit:

- Syntax highlighting: TypeScript compile/first match/tokenize, Rust compile/first match/tokenize, CSS tokenize
- Text scanning: literal 50 KB, no-match 50 KB, no-match 10 KB, field extract 50 KB, timestamp 50 KB, RegSet position-lead
- Pattern matching: literal exact, quantifier greedy, lookaround combined, Unicode Greek, backref simple, alternation 10 branches, named capture date
- Compilation: literal, named capture, lookbehind
- Memory: TypeScript grammar compile peak RSS, compile + scan peak RSS

<details>
<summary><strong>Running benchmarks</strong></summary>

Ferroni keeps benchmark suites separated by purpose:

- **Internal suite (`codspeed_bench`, Rust-only, regression/optimization work):**
  ```bash
  cargo bench
  cargo bench --bench codspeed_bench

  # compare two local baselines
  cargo bench --bench codspeed_bench -- --baseline main
  cargo bench --bench codspeed_bench -- --baseline feature-branch
  ```

- **Reference suite (`battle_bench`, Ferroni vs Oniguruma for publishable numbers):**
  ```bash
  # one-time setup
  ./scripts/prepare-oniguruma-sources.sh

  cargo bench --features ffi --bench battle_bench
  ```
  Exact external input pins for this suite live in
  [`benches/battle_inputs.toml`](benches/battle_inputs.toml).

- **Memory comparison (process-isolated peak RSS on the large TypeScript scanner workload):**
  ```bash
  ./scripts/run-battle-memory.sh
  ```

- **HTML report:**
  ```bash
  open target/criterion/report/index.html
  ```

</details>

## Architecture

Each C source file maps 1:1 to a Rust module ([ADR-001](https://sebastian-software.github.io/ferroni/adr/001-one-to-one-parity-with-c-original)):

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

- **27 of 29 encodings** -- only ASCII and UTF-8 ([ADR-003](https://sebastian-software.github.io/ferroni/adr/003-encoding-scope-ascii-and-utf8-only))
- **POSIX/GNU API** -- `regcomp`/`regexec`/`regfree` ([ADR-012](https://sebastian-software.github.io/ferroni/adr/012-posix-and-gnu-api-not-ported))
- **C memory management** -- replaced by Rust's `Drop` trait
- **`onig_new_deluxe`** -- C-specific allocation, use `onig_new()` instead

## Running tests

Debug builds need a larger thread stack; the required sizes are stated once in
[ADR-013](https://sebastian-software.github.io/ferroni/adr/013-stack-overflow-debug-builds).

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

## Test parity

Every upstream test that targets UTF-8 is ported 1:1. EUC-JP tests are
out of scope ([ADR-003](https://sebastian-software.github.io/ferroni/adr/003-encoding-scope-ascii-and-utf8-only)).

| Upstream file | Encoding | Upstream | Ferroni | Status |
|---------------|----------|--------:|--------:|--------|
| test_utf8.c | UTF-8 | 1,554 | 1,561 | 100% |
| test_back.c | UTF-8 | 1,225 | 1,225 | 100% |
| test_syntax.c | UTF-8 | 144 | 144 | 100% |
| test_options.c | UTF-8 | 47 | 48 | 100% |
| test_regset.c | UTF-8 | 4 | 15 | 100% |
| testc.c | EUC-JP | 785 | — | out of scope |
| **C total (UTF-8)** | | **2,974** | **2,993** | **100%** |
| [vscode-oniguruma](https://github.com/microsoft/vscode-oniguruma) tests | UTF-16 | 15 | 25 | 100% |

On top of the ported upstream cases, Ferroni adds Rust-native tests for
API integration, edge cases, error paths, and coverage gaps. The tree holds
**2,184 `#[test]` functions** in total (some compat functions bundle multiple
upstream cases, so this is higher than the parity totals above). The count is
derived from the tree by [`scripts/count-tests.sh`](scripts/count-tests.sh),
which is the single source for the test counts quoted in this README and in
CONTRIBUTING.

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
| [001](https://sebastian-software.github.io/ferroni/adr/001-one-to-one-parity-with-c-original) | 1:1 structural parity with C original |
| [002](https://sebastian-software.github.io/ferroni/adr/002-unsafe-code-policy) | Unsafe code policy |
| [003](https://sebastian-software.github.io/ferroni/adr/003-encoding-scope-ascii-and-utf8-only) | Encoding scope: ASCII and UTF-8 only |
| [004](https://sebastian-software.github.io/ferroni/adr/004-c-to-rust-translation-patterns) | C-to-Rust translation patterns |
| [005](https://sebastian-software.github.io/ferroni/adr/005-idiomatic-rust-api-layer) | Idiomatic Rust API layer |
| [006](https://sebastian-software.github.io/ferroni/adr/006-scanner-api) | Scanner API for TextMate tokenization |
| [007](https://sebastian-software.github.io/ferroni/adr/007-simd-accelerated-search) | SIMD-accelerated search via memchr |
| [008](https://sebastian-software.github.io/ferroni/adr/008-rust-only-optimizations) | Rust-only optimizations and performance philosophy |
| [009](https://sebastian-software.github.io/ferroni/adr/009-dependency-philosophy) | Dependency philosophy |
| [010](https://sebastian-software.github.io/ferroni/adr/010-benchmark-strategy) | Benchmark strategy |
| [011](https://sebastian-software.github.io/ferroni/adr/011-test-strategy-and-c-test-parity) | Test strategy and C test suite parity |
| [012](https://sebastian-software.github.io/ferroni/adr/012-posix-and-gnu-api-not-ported) | POSIX and GNU API not ported |
| [013](https://sebastian-software.github.io/ferroni/adr/013-stack-overflow-debug-builds) | Stack overflow mitigation in debug builds |
| [014](https://sebastian-software.github.io/ferroni/adr/014-porting-bugs-lessons-learned) | Porting bugs: lessons learned |

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md)
and review the ADRs before submitting a PR.

## Acknowledgments

Ferroni is built on the work of [K. Kosako](https://github.com/kkos) and
the Oniguruma contributors. The C original powers regex in
[Ruby](https://www.ruby-lang.org/), [PHP](https://www.php.net/),
[TextMate](https://macromates.com/), [jq](https://jqlang.github.io/jq/),
and many other projects. The Scanner API and its test suite are based on
[vscode-oniguruma](https://github.com/microsoft/vscode-oniguruma)
by Microsoft and the VS Code team.

## License

[BSD-2-Clause](LICENSE) (same as Oniguruma)

Ferroni is the one Ferramenta tool that is not published under `MIT OR
Apache-2.0`: it is a line-by-line port of Oniguruma's C source, so it stays
under the BSD-2-Clause license of the code it is derived from.

---

<!-- ferramenta-family:start -->
## The Ferramenta family

This project is part of [Ferramenta](https://ferramenta.dev) — the family of Rust-native developer tools by [Sebastian Software](https://oss.sebastian-software.com) that keep the APIs the ecosystem already knows:

| Tool | Job |
| --- | --- |
| **[ferroni](https://github.com/sebastian-software/ferroni)** | Oniguruma-compatible regex engine |
| [ferriki](https://github.com/sebastian-software/ferriki) | Shiki-compatible syntax highlighting |
| [ferromark](https://github.com/sebastian-software/ferromark) | CommonMark/GFM Markdown to HTML |
| [ferrovia](https://github.com/sebastian-software/ferrovia) | SVGO-compatible SVG optimizer |
| [ferrocat](https://github.com/sebastian-software/ferrocat) | Translation catalog engine |
| [ferrolex](https://github.com/sebastian-software/ferrolex) | Spell, dictionary, and brand validation |
| [ferrugo](https://github.com/sebastian-software/ferrugo) | Rust-native PDF previews |
<!-- ferramenta-family:end -->

<!-- sebastian-software-branding:start -->
<p align="center">
  <a href="https://oss.sebastian-software.com">
    <img src="https://sebastian-brand.vercel.app/sebastian-software/logo-software.svg" alt="Sebastian Software" width="240" />
  </a>
</p>

<p align="center">
  <strong>Built by Sebastian Software</strong> — consulting for TypeScript, React &amp; Rust.<br />
  <a href="https://sebastian-software.de">Work with us</a> · <a href="https://oss.sebastian-software.com">More open source</a>
</p>

<p align="center">Copyright &copy; 2026 Sebastian Software GmbH</p>
<!-- sebastian-software-branding:end -->
