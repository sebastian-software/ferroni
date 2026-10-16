# TSX scanner replay

The scanner calls Ferriki's tokenizer makes while it highlights
`Orders.tsx`, the curated "Orders" component of its benchmarks, repeated
16 times (25,088 bytes, 816 lines): 23,088 calls into 48 scanner instances
over 784 strings. 16,080 of the calls find nothing. The instances range from
one-pattern `end` scanners to the TypeScript statement and expression
scanners of 105 to 129 patterns. All 48 exist after the first pass, so the
recorded second pass creates none. Ferriki compiles them through one shared
`ScannerPatternCache`; the replay compiles each instance on its own.

It was recorded after Ferriki's TypeScript and TSX highlighting took about
10% longer with Ferroni 2.2.1 than with 2.1.0. That regression came from
register allocation in Ferriki's arm64 addon (fat LTO, one codegen unit)
and does not show here. With the bench profile on an M1 Ultra, 2.1.0
runs this replay with 4.8% more instructions and about 5% more cycles than
2.2.1. The replay keeps the large ordered pattern lists, the failed searches
and the UTF-16 captures of a TypeScript grammar in Ferroni's regression and
engine benchmarks. Effects of a consumer's build stay with the consumer's
benchmark.

`trace.json` has the layout of the [C++ replay](../cpp_scanner/README.md)
and was captured as the [JSON replay](../json_scanner/README.md) describes,
with `benches/json_scanner/capture.patch` and `capture.rs` at the pinned
Ferriki revision (language `tsx`, 16 copies, tokenize time limit off),
compacted with hot groups trimmed to three. `provenance.json` pins both
repositories and the hashes. The grammar is Ferriki's curated copy of VS
Code's `TypeScriptReact.tmLanguage.json`; its MIT license is retained in
`LICENSE-vscode.txt`. The fixture is Ferriki's own (MIT or Apache-2.0).

The replay, its measurement boundary and the engine comparison work as for
the C++ replay; `cold_document` compiles fresh scanners and replays the
document once. Scanner groups 2 and 1, the injections of the embedded HTML
and CSS template grammars, have the most calls (7,648 each) and get their own
benchmarks, with group 29, an 86-pattern expression scanner.

```sh
cargo bench --locked --features ffi --bench ferriki_scanner_bench -- tsx_scanner --test
cargo bench --locked --bench ferriki_scanner_bench -- tsx_scanner
```
