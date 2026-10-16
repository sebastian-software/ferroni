# Rust scanner replay

The scanner calls Ferriki's tokenizer makes while it highlights
`orders.rs`, the curated "Orders" module of its benchmarks, repeated 16 times
(15,440 bytes, 480 lines): 3,936 calls into 8 scanner instances over 432
strings. 432 of the calls find nothing. Six of the eight instances hold 52 to
80 patterns, and a call searches 74 patterns on average. All 8 exist after the
first pass, so the recorded second pass creates none. Ferriki compiles them
through one shared `ScannerPatternCache`; the replay compiles each instance on
its own.

It is the dense one of the three scanner workloads beside
[JSON](../json_scanner/README.md) and [TSX](../tsx_scanner/README.md): few
scanners, but 88% of the calls search more than 70 patterns, and 89% find a
match. JSON covers small grammars and TSX many large scanners with mostly
failed searches. Rust is the third format of the
[curated highlighting corpus](../highlighting_corpus.md).

`trace.json` has the layout of the [C++ replay](../cpp_scanner/README.md)
and was captured as the [JSON replay](../json_scanner/README.md) describes,
with `benches/json_scanner/capture.patch` and `capture.rs` at the pinned
Ferriki revision (language `rust`, 16 copies, tokenize time limit off),
compacted with hot groups trimmed to three. `provenance.json` pins both
repositories and the hashes. The grammar is Ferriki's curated copy of VS
Code's `rust.tmLanguage.json`; its MIT license is retained in
`LICENSE-vscode.txt`. The fixture is Ferriki's own (MIT or Apache-2.0).

The replay, its measurement boundary and the engine comparison work as for
the C++ replay; `cold_document` compiles fresh scanners and replays the
document once. Scanner group 0, the 80-pattern source scanner, has the most
calls (2,800) and gets its own benchmark, with groups 5 and 3, two scanners of
73 and 74 patterns inside parentheses and declarations.

```sh
cargo bench --locked --features ffi --bench ferriki_scanner_bench -- rust_scanner --test
cargo bench --locked --bench ferriki_scanner_bench -- rust_scanner
```
