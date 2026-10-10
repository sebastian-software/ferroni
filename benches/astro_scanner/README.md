# Astro scanner replay

The scanner calls Ferriki's tokenizer makes while it highlights
`Orders.astro`, the curated "Orders" component of its benchmarks, repeated
16 times (17,296 bytes, 496 lines): 7,151 calls into 153 scanner instances
(76 distinct pattern lists) over 960 strings. The Astro grammar embeds
JSON, JavaScript, TypeScript, CSS, PostCSS and TSX, which Ferriki's loader
pulls in, so the instances range from one-pattern `end` scanners to the
TypeScript expression scanners of 130 patterns. It was recorded for
[issue #327](https://github.com/sebastian-software/ferroni/issues/327), where
Ferriki's Astro highlighting regressed with the DFA pre-filter.

64 of the 153 instances are created during the recorded pass: Ferriki
recompiles an `end` rule whose back references resolve to new text (the
`</script\s*>|/>` and `</style\s*>|/>` lists, four per copy of the fixture),
and each such instance answers one or two calls before the next one
replaces it. The replay builds every instance once, so it measures the
steady-state search; the construction and first-search cost of the
short-lived instances is what ADR-008's warm-up keeps off the automata.

`trace.json` has the layout of the [C++ replay](../cpp_scanner/README.md)
and was captured as the [JSON replay](../json_scanner/README.md) was, with
`benches/json_scanner/capture.patch` and `capture.rs` at the pinned Ferriki
revision (language `astro`, 16 copies), compacted with hot groups trimmed to
three. `provenance.json` pins both repositories and the hashes. The grammar
excerpts retain the upstream MIT license in
`LICENSE-textmate-grammars-themes.txt`; the fixture is Ferriki's own
(MIT or Apache-2.0).

The replay, its measurement boundary and the engine comparison work as for
the C++ replay. Scanner group 22, the TypeScript expression scanner of the
frontmatter, has the most calls (735) and gets its own benchmark, with
groups 23 and 34.

```sh
cargo bench --locked --features ffi --bench ferriki_scanner_bench -- astro_scanner --test
cargo bench --locked --bench ferriki_scanner_bench -- astro_scanner
```
