# Archive size and process memory

The final NFA and Dense matrix. Peak RSS includes fixture parsing, source bytes and indexes, decoded programs, scanner scratch, and temporary allocations. Immutable/scratch counters are partial heap accounting. Sizes cover the regex payload, excluding common grammar/theme bytes.

| Corpus | Variant | Raw MiB | gzip-9 KiB | Brotli-11 KiB | Peak RSS MiB | Immutable automata MiB | Hybrid scratch MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cpp | eager | 1.60 | 189.3 | 38.6 | 101.8 | 23.87 | 34.27 |
| cpp | nfa | 11.02 | 3729.0 | 1453.5 | 115.7 | 23.87 | 34.27 |
| cpp | dense | 12.24 | 3764.7 | 1476.7 | 120.2 | 25.00 | 34.06 |
| scss | eager | 0.73 | 82.7 | 38.6 | 48.2 | 14.20 | 12.17 |
| scss | nfa | 6.68 | 2132.9 | 1089.4 | 55.1 | 14.20 | 12.17 |
| scss | dense | 6.76 | 2135.7 | 1090.2 | 55.0 | 14.26 | 12.15 |
| java | eager | 0.28 | 17.3 | 11.4 | 12.7 | 0.89 | 1.46 |
| java | nfa | 0.63 | 146.4 | 72.1 | 12.9 | 0.89 | 1.46 |
| java | dense | 69.88 | 2286.7 | 1166.0 | 256.9 | 69.84 | 0.91 |
| json | eager | 0.01 | 1.7 | 1.6 | 6.9 | 0.02 | 0.06 |
| json | nfa | 0.02 | 3.7 | 3.4 | 6.7 | 0.02 | 0.06 |
| json | dense | 0.34 | 13.2 | 8.8 | 7.5 | 0.32 | 0.00 |
| rust | eager | 0.05 | 8.3 | 7.0 | 9.5 | 0.29 | 0.51 |
| rust | nfa | 0.16 | 46.9 | 27.9 | 9.7 | 0.29 | 0.51 |
| rust | dense | 9.92 | 242.3 | 149.0 | 41.4 | 9.86 | 0.00 |
| html | eager | 4.36 | 304.3 | 113.6 | 77.6 | 8.64 | 11.54 |
| html | nfa | 7.83 | 1596.1 | 746.3 | 78.4 | 8.64 | 11.54 |
| html | dense | 21.17 | 1767.2 | 843.1 | 126.4 | 21.46 | 10.38 |
| astro | eager | 5.67 | 331.3 | 111.1 | 87.6 | 14.82 | 17.09 |
| astro | nfa | 11.59 | 2568.3 | 1026.9 | 100.6 | 14.82 | 17.09 |
| astro | dense | 43.20 | 2925.9 | 1345.1 | 196.7 | 45.46 | 14.62 |
| tsx | eager | 4.13 | 146.1 | 39.9 | 76.0 | 9.54 | 10.33 |
| tsx | nfa | 7.98 | 1600.5 | 562.1 | 76.9 | 9.54 | 10.33 |
| tsx | dense | 9.25 | 1637.5 | 586.7 | 84.5 | 10.71 | 10.15 |
| c | eager | 0.14 | 25.1 | 17.4 | 25.4 | 3.86 | 7.68 |
| c | nfa | 1.43 | 535.3 | 195.3 | 26.5 | 3.86 | 7.68 |
| c | dense | 173.49 | 3230.1 | 1868.2 | 596.1 | 173.32 | 0.02 |
| php | eager | 3.68 | 331.8 | 169.1 | 60.1 | 10.22 | 10.35 |
| php | nfa | 7.52 | 1821.8 | 950.4 | 59.8 | 10.22 | 10.35 |
| php | dense | 30.97 | 2224.7 | 1191.1 | 139.2 | 33.26 | 9.66 |

## Decompression diagnostics

Separate runtimes: gzip uses Python/zlib, Brotli uses Node 24.22.0. Values include decoding/allocation and are not added to the Rust benchmark as an end-to-end latency claim. Nine samples after a round-trip warmup; medians shown.

| Corpus | Variant | gzip decode ms | Brotli decode ms |
| --- | --- | ---: | ---: |
| cpp | eager | 1.611 | 0.953 |
| cpp | nfa | 19.158 | 25.619 |
| cpp | dense | 20.063 | 26.553 |
| scss | eager | 0.719 | 0.656 |
| scss | nfa | 10.671 | 16.502 |
| scss | dense | 10.627 | 17.308 |
| java | eager | 0.190 | 0.173 |
| java | nfa | 0.850 | 1.172 |
| java | dense | 44.457 | 32.552 |
| json | eager | 0.021 | 0.028 |
| json | nfa | 0.028 | 0.052 |
| json | dense | 0.199 | 0.240 |
| rust | eager | 0.051 | 0.092 |
| rust | nfa | 0.260 | 0.378 |
| rust | dense | 6.101 | 5.453 |
| html | eager | 3.210 | 2.119 |
| html | nfa | 10.360 | 11.333 |
| html | dense | 17.487 | 15.441 |
| astro | eager | 4.149 | 2.235 |
| astro | nfa | 16.760 | 16.635 |
| astro | dense | 32.793 | 24.783 |
| tsx | eager | 2.572 | 1.215 |
| tsx | nfa | 10.069 | 9.131 |
| tsx | dense | 10.860 | 10.566 |
| c | eager | 0.173 | 0.238 |
| c | nfa | 2.529 | 3.032 |
| c | dense | 97.757 | 53.851 |
| php | eager | 3.358 | 2.773 |
| php | nfa | 10.806 | 13.661 |
| php | dense | 23.672 | 21.598 |

## Complete-DFA build coverage

Both full-table representations use the same 16 MiB per-DFA and 64 MiB determinization budgets. These counts describe groups, not their share of search calls. Remaining suitable groups store NFAs. Groups absent from the archive use ordinary Eager construction.

| Corpus | Complete DFA groups | NFA fallback groups | Dense export seconds |
| --- | ---: | ---: | ---: |
| cpp | 17 | 26 | 86.54 |
| scss | 6 | 13 | 44.95 |
| java | 18 | 5 | 51.41 |
| json | 5 | 0 | 0.01 |
| rust | 8 | 0 | 0.98 |
| html | 37 | 23 | 36.64 |
| astro | 40 | 34 | 48.18 |
| tsx | 19 | 29 | 30.96 |
| c | 19 | 0 | 67.33 |
| php | 18 | 14 | 11.35 |

Export times were recorded during preparation and can overlap local validation work. They are not isolated build-throughput comparisons.
