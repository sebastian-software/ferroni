# C scanner replay

The scanner calls Shiki makes while it highlights `orders.c`, the TIOBE
"Orders" program from Ferriki's benchmark fixtures, repeated 16 times
(17,296 bytes, 561 lines): 4,832 calls into 29 scanners
over 848 strings. The C grammar uses no Oniguruma-only syntax that
Onigmo, PCRE2 or fancy-regex reject or read differently, so every comparison
engine can run this replay.

[`benches/shiki_js/capture.mjs`](../shiki_js/capture.mjs) recorded `trace.json`
with Shiki 4.5.0 and its Oniguruma (WASM) engine, the engine Shiki uses
by default. The same tool reproduces the existing Java trace call for call.
`provenance.json` pins the document, the capture command and the hashes.
Pattern excerpts retain the upstream MIT license in
`LICENSE-textmate-grammars-themes.txt`.

The replay, its measurement boundary and the engine comparison work as for the
[C++ replay](../cpp_scanner/README.md). Scanner group 12 has the most
calls (1,120) and gets its own benchmark.

```sh
cargo bench --locked --features ffi --bench shiki_scanner_bench -- c_scanner --test
cargo bench --locked --bench shiki_scanner_bench -- c_scanner
```
