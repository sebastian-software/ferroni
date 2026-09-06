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
| `scanner-api` | `Scanner::new` over arbitrary patterns, then walking `find_next_match` across arbitrary text, checking that every match is in range and on a character boundary. |

`pattern-match` sets a per-call step budget (`retry_limit_in_match`,
`retry_limit_in_search`, `match_stack_limit`) through `OnigMatchParam`.
Catastrophic backtracking is a property of the pattern rather than a bug in the
engine, and the budget keeps the fuzzer hunting for crashes instead of timing
out on something like `(a+)+$`. The limits live on the match parameter, so
nothing is shared between fuzzer threads.

## Running

Install [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) and run a target
with nightly Rust:

```sh
cargo +nightly fuzz run pattern-compile -- -dict=fuzz/ferroni.dict -max_len=16384
cargo +nightly fuzz run pattern-match   -- -dict=fuzz/ferroni.dict -max_len=16384
cargo +nightly fuzz run scanner-api     -- -dict=fuzz/ferroni.dict -max_len=16384
```

Every target rejects oversized input on its own, and the workflow adds
libFuzzer time and RSS limits on top.

`.github/workflows/fuzz.yml` runs a 60-second smoke test per target on pull
requests and a longer run every week.

## What is tracked

Crash artifacts and local corpora live under `fuzz/artifacts` and `fuzz/corpus`
and are intentionally untracked. The dictionary is tracked, because it helps
mutation reach Oniguruma constructs such as `(?<name>`, `\p{L}`, and `(?~`
without committing the project to a large or stale corpus.
