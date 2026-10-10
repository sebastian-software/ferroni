# JSON scanner replay

The scanner calls Ferriki's tokenizer makes while it highlights
`orders.json`, the curated "Orders" fixture of its benchmarks, repeated 16
times (12,176 bytes, 432 lines): 3,840 calls into 6 scanner instances (5
distinct pattern lists) over 432 strings. The JSON grammar is the simplest
of the captured corpora: 3 to 11 patterns per scanner, literals and small
classes, where the position-lead search has little to do and the DFA
pre-filter the least to save. It was recorded for
[issue #327](https://github.com/sebastian-software/ferroni/issues/327), where
Ferriki's JSON highlighting regressed with the pre-filter, and keeps such
grammars in the engine comparison.

`trace.json` has the layout of the [C++ replay](../cpp_scanner/README.md):
the compiled patterns of each scanner instance in priority order, subjects
with their identities, UTF-16 search positions, find options, matched
pattern indices and every capture, compacted with
`benches/cpp_scanner/compact.py` (hot groups trimmed to three). It was
captured through Ferriki's own tokenizer, not the Node addon: `capture.patch`
applied to the scanner adapter at the pinned Ferriki revision logs every
call, and `capture.rs`, placed as `examples/trace_curated.rs` in Ferriki's
root crate, drives `Highlighter::highlight_with_options` as the paired
native benchmark does (`github-dark`, `tokenizeTimeLimit` 500, tokens only).
The recorded pass is the second one: the first compiles the scanners, and
both tokenize alike. `provenance.json` pins both repositories and the hashes.
The grammar excerpts retain the upstream MIT license in
`LICENSE-textmate-grammars-themes.txt`; the fixture is Ferriki's own
(MIT or Apache-2.0).

The replay, its measurement boundary and the engine comparison work as for
the C++ replay. Scanner group 1, the object scanner, has the most calls
(1,504) and gets its own benchmark.

```sh
cargo bench --locked --features ffi --bench ferriki_scanner_bench -- json_scanner --test
cargo bench --locked --bench ferriki_scanner_bench -- json_scanner
```

## Recapturing

In a Ferriki checkout at the pinned revision, with its `ferroni` workspace
dependency pointed at the Ferroni checkout to capture with:

```sh
git apply /path/to/ferroni/benches/json_scanner/capture.patch
cp /path/to/ferroni/benches/json_scanner/capture.rs examples/trace_curated.rs
cargo run --release --example trace_curated -- assets/shiki json \
  node/benchmarks/curated/fixtures/orders.json 16 /tmp/json.jsonl
python3 /path/to/ferroni/benches/cpp_scanner/compact.py /tmp/json.jsonl trace.json
```

The raw JSONL duplicates the pattern lists per call and is not retained.
On recapture, update the provenance and the hot groups together, then
repeat the Rust and C validation.
