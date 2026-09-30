# SCSS baseline and cross-reference diagnosis

The engine and replay are fixed at `5d22758` (Ferroni 1.7.0). Two sequential
ordinary Criterion runs use thin LTO, 30 samples, one second of warmup and
three seconds of requested measurement on Apple M1 Ultra / macOS 27.0,
Rust 1.96.0. No builds, tests or other benchmark processes run concurrently.
`summary.json` retains both means and confidence intervals. All 3,744 calls
and full captures agree with the captured output and pinned C Oniguruma
`f95747b462de672b6f8dbdeb478245ddf061ca53` before timing.

| Scanner case | Calls | Run 1 estimate (ms) | Run 2 estimate (ms) |
| --- | ---: | ---: | ---: |
| document | 3744 | 25.77 | 26.12 |
| group_8 | 1264 | 8.66 | 8.79 |
| group_10 | 416 | 4.13 | 4.13 |
| group_14 | 208 | 4.59 | 4.53 |
| group_6 | 224 | 3.74 | 3.73 |
| group_13 | 224 | 0.651 | 0.654 |
| group_5 | 96 | 0.467 | 0.470 |

These are the central Criterion estimates printed by the retained logs;
the JSON also keeps the mean, median and slope intervals. Group cases are
independent replays, not additive shares of the document. This boundary
excludes Ferriki's token/scopes work and must not be compared directly with
its public API timings.

## Individual pattern candidates

The independent full-range per-pattern helper validates every individual
search and every capture against the same C reference before measuring.
Its timing boundary differs from RegSet scheduling and memoization; results
rank candidate patterns without establishing their actual scanner shares.
The original document identity, starts, options, and pattern indices remain
available in the replay fixture. Each entry retains all three measured rounds.

| Corpus / group | Pattern index | Median independent batch (ms) | Candidate |
| --- | ---: | ---: | --- |
| SCSS / 8 | 19 | 7.27 | Case-insensitive HTML/SVG/MathML tag list |
| SCSS / 8 | 68 | 6.65 | Same tag list repeated in the scanner |
| SCSS / 10 | 23 | 4.34 | Case-insensitive CSS property list |
| SCSS / 10 | 15 | 2.63 | Case-insensitive CSS value list |
| SCSS / 14 | 22 | 3.05 | CSS property list |
| C++ / 78 | 100 | 17.85 | Complex declaration recognition |
| C++ / 78 | 101 | 17.25 | Related declaration recognition |
| C++ / 78 | 19 / 102 | 7.28 / 7.22 | Attribute/comment/declaration prefixes |

The tag list includes `h[1-6]`, `rtc??`, and `ul??` among hundreds of literal
alternatives. The existing trie detector requires the entire alternation
to contain plain strings, so these branches exclude the complete list.
The first compiler experiment should compact only long contiguous literal
runs in place, preserving all non-literal branches and priority boundaries.
Nested extraction remains disabled: optional ordering and capture retention
need separate proofs.

## oniguruma-to-es and optional optimizer

Pinned oniguruma-to-es 4.3.4 translates all 104 distinct captured SCSS patterns
and all 250 distinct C++ patterns without errors. Its generated forms retain
the large SCSS alternatives; the speed gap is not explained by replacing the
tag list with a visibly smaller source expression.

The separate oniguruma-parser 0.12.1 default optimizer changes **zero** SCSS
patterns and 88 C++ patterns. The principal C++ candidates shrink by only
1–7 source characters. A smaller expression is not evidence of less VM work.
Shiki does not invoke this optional optimizer. Prefix/suffix extraction,
alternation-to-class conversion, and group removal are useful references,
but none is an established performance fix for these captured SCSS inputs.
Generated JavaScript patterns cannot be passed to Ferroni directly: flags,
Unicode classes, anchors and emulation constructs have different contracts.

The complete translations, optimized expressions, locations and failures
remain in `*-cross-reference.json.gz`. Reproduce them using the pinned
Ferriki checkout and `cross-reference.mjs` from the parent directory.
`scss-capture-receipt.json.gz` records the diagnostic addon; those timings
are not baseline performance evidence. The ordinary addon was restored
byte-for-byte after capture and the Ferriki checkout remains clean.
