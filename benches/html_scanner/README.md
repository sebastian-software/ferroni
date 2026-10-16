# HTML scanner replay

The scanner calls Ferriki's tokenizer makes while it highlights
`orders.html`, the curated "Orders" page of its benchmarks with inline
scripts and styles, repeated 16 times (22,352 bytes, 720 lines): 16,176 calls
into 74 scanner instances (60 distinct pattern lists) over 784 strings.
8,336 of the calls find nothing. The HTML grammar embeds JavaScript and CSS,
and the JavaScript grammar injects the HTML and CSS template grammars, so the
instances range from one-pattern `end` scanners to the 132-pattern JavaScript
scanners of the `<script>` bodies; a call searches 15 patterns on average.
All 74 exist after the first pass, so the recorded second pass creates none.
Ferriki compiles them through one shared `ScannerPatternCache`; the replay
compiles each instance on its own.

It is the embedded-language workload beside [JSON](../json_scanner/README.md),
[Rust](../rust_scanner/README.md) and [TSX](../tsx_scanner/README.md), and it
replaces [Astro](../astro_scanner/README.md) in the engine comparison: every
comparison engine reproduces all of its calls, while none reproduces Astro.
HTML is the fifth format of the
[curated highlighting corpus](../highlighting_corpus.md).

`trace.json` has the layout of the [C++ replay](../cpp_scanner/README.md)
and was captured as the [JSON replay](../json_scanner/README.md) describes,
with `benches/json_scanner/capture.patch` and `capture.rs` at the pinned
Ferriki revision (language `html`, 16 copies, tokenize time limit off),
compacted with hot groups trimmed to three. `provenance.json` pins both
repositories and the hashes. The grammars are Ferriki's curated copies of VS
Code's HTML, JavaScript and CSS grammars and the template injections of
vscode-styled-components and es6-string-html; their MIT licenses are retained
in `LICENSE-vscode.txt`, `LICENSE-vscode-styled-components.txt` and
`LICENSE-es6-string-html.txt`. The fixture is Ferriki's own (MIT or
Apache-2.0).

The replay, its measurement boundary and the engine comparison work as for
the C++ replay; `cold_document` compiles fresh scanners and replays the
document once. Scanner groups 51 and 16, the HTML and CSS template injections
of the inline scripts, have the most calls (3,360 and 3,008) and get their own
benchmarks, with group 0, the 37-pattern scanner of the document's top level.

```sh
cargo bench --locked --features ffi --bench ferriki_scanner_bench -- html_scanner --test
cargo bench --locked --bench ferriki_scanner_bench -- html_scanner
```
