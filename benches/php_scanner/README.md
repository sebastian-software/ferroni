# PHP scanner replay

The scanner calls Shiki makes while it highlights `orders.php`, the TIOBE
"Orders" program from Ferriki's benchmark fixtures, repeated 16 times
(14,304 bytes, 545 lines): 3,952 calls into 35 scanners
over 816 strings. The PHP grammar uses no Oniguruma-only syntax that
Onigmo, PCRE2 or fancy-regex reject or read differently, so every comparison
engine can run this replay.

[`benches/shiki_js/capture.mjs`](../shiki_js/capture.mjs) recorded `trace.json`
with Shiki 4.5.0 and its Oniguruma (WASM) engine, the engine Shiki uses
by default. The same tool reproduces the existing Java trace call for call.
`provenance.json` pins the document, the capture command and the hashes.
Pattern excerpts retain the upstream MIT license in
`LICENSE-textmate-grammars-themes.txt`.

The replay, its measurement boundary and the engine comparison work as for the
[C++ replay](../cpp_scanner/README.md). Scanner group 13 has the most
calls (752) and gets its own benchmark.

```sh
cargo bench --locked --features ffi --bench shiki_scanner_bench -- php_scanner --test
cargo bench --locked --bench shiki_scanner_bench -- php_scanner
```
