# Fuzzing

Ferroni compiles and runs patterns that come from TextMate grammars and other
untrusted input, so the parser, the compiler, the matcher, and the Scanner all
have to survive arbitrary bytes. The `fuzz` crate is deliberately its own
workspace, so it is neither a member of the release workspace nor part of the
published `ferroni` package.

## Targets

| Target | What it exercises |
| --- | --- |
| `pattern-compile` | `onig_new` over arbitrary pattern bytes, with the first two bytes selecting one of six syntaxes and one of seven option sets. |
| `pattern-match` | Compiling an arbitrary pattern and searching arbitrary bytes with `onig_search_with_param`, then checking that the reported match and every capture group stay inside the haystack. |
| `scanner-api` | `Scanner::new` over arbitrary patterns, then walking `find_next_match` across arbitrary text from an input-chosen start position, checking that every match is in range and on a character boundary. |
| `prefilter-differential` | A scanner with the DFA pre-filter ([ADR-008](https://ferroni.dev/adr/008-rust-only-optimizations)) against one without it: the same patterns and text, `find_next_match` from every byte offset under every find option, a tokenizing loop through `find_next_match_with_id`, and `find_next_match_utf16` from every code unit. A first byte of zero makes a generator write one to six patterns over the constructs the seek approximates (negated and nested classes, folds, looks, back references, calls, conditionals, the absent operator, anchors) and a text over an alphabet of ASCII, newlines, folding and non-ASCII characters; any other first byte reads NUL-separated patterns and a text as they are. Any difference is a bug. |

The fuzz crate builds Ferroni with the `prefilter-self-check` feature: every
scanner search the pre-filter decides also runs the position-lead search on
the same set and panics unless both decide alike, captures included. The two
scanner targets therefore check the pre-filter on every search they make, not
only where the differential target compares results.

`pattern-match` sets a per-call step budget (`retry_limit_in_match`,
`retry_limit_in_search`, `match_stack_limit`) through `OnigMatchParam`.
Catastrophic backtracking is a property of the pattern rather than a bug in the
engine, and the budget keeps the fuzzer hunting for crashes instead of timing
out on something like `(a+)+$`. The limits live on the match parameter, so
nothing is shared between fuzzer threads.

Stack depth is not fuzzed. The `pattern-compile` target calls `onig_new` on
libFuzzer's main thread, which is 8 MiB in release builds. A pattern that needs
more stack than a thread has aborts the process, and the fuzzer cannot tell
that from a crash it should keep. The 2 MiB default of `std::thread::spawn`, of
Tokio and Rayon workers, and every debug build are therefore not covered here.
The nesting and AST-budget regression tests in `tests/api_test.rs` compile the
limit boundaries on a 2 MiB thread instead; ADR-013 gives the per-unit costs.

## Running

Install [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) and run a target
with nightly Rust:

```sh
cargo +nightly fuzz run pattern-compile -- -dict=fuzz/ferroni.dict -max_len=16384
cargo +nightly fuzz run pattern-match   -- -dict=fuzz/ferroni.dict -max_len=16384
cargo +nightly fuzz run scanner-api     -- -dict=fuzz/ferroni.dict -max_len=16384
cargo +nightly fuzz run prefilter-differential -- -dict=fuzz/ferroni.dict -max_len=16384
```

Every target rejects oversized input on its own, and the workflow adds
libFuzzer time and RSS limits on top.

`prefilter-differential` starts best from a seed corpus of the pattern/text
pairs the compat files hold and the review regressions of PR #321. The
`write_fuzz_seeds` test in `tests/compat_prefilter_differential.rs` writes
them in the target's raw layout to the directory `FERRONI_FUZZ_SEED_DIR`
names, which is where `cargo fuzz` looks for the target's corpus:

```sh
FERRONI_FUZZ_SEED_DIR=fuzz/corpus/prefilter-differential \
  cargo test --test compat_prefilter_differential write_fuzz_seeds -- --ignored
```

A crash of this target is a wrong result, never a limit: minimize it, add the
pattern set to `tests/scanner_prefilter_regressions.rs`, and fix the seek or
the search loop so that the seek stays a superset of its pattern. The one
difference ADR-008 accepts, the search without the pre-filter reaching the
default retry limit in an attempt the pre-filter leaves out (`(a+)+b` before
`c` on a run of `a`), the target tells apart through the C API's set, which
runs the same search and keeps the error code. Such a search takes seconds in
the instrumented build, so the target ends a case after one of them, and the
workflow gives this target a longer per-input timeout than the others.

`.github/workflows/fuzz.yml` runs a 60-second smoke test per target on pull
requests and a longer run every week, seeded as above for
`prefilter-differential`.

## What is tracked

Crash artifacts and local corpora live under `fuzz/artifacts` and `fuzz/corpus`
and are intentionally untracked. The dictionary is tracked, because it helps
mutation reach Oniguruma constructs such as `(?<name>`, `\p{L}`, and `(?~`
without committing the project to a large or stale corpus; the seeds above are
regenerated from the tests rather than tracked.
