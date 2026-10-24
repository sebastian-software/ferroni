# Zstd versus Brotli on the portable packages

Same Node runtime, native builtin codecs, identical payloads: three rotating processes, three warmups and 15 timed decodes per codec/process. Central times are medians of process means. Ranges show all three paired changes versus Brotli-11. Every decoded byte agrees. Zstd frame checksums are enabled at every level. These are local codec diagnostics, excluding Rust load, I/O, transport, bridge costs, and teardown; no browser/CDN speed claim follows.

| Corpus | Codec | Compressed KiB | Size vs Brotli | Decode ms | Decode vs Brotli, median [range] |
| --- | --- | ---: | ---: | ---: | ---: |
| cpp | brotli11 | 2416.0 | +0.0% | 44.173 | +0.0% [+0.0, +0.0] |
| cpp | zstd3 | 4638.3 | +92.0% | 25.811 | -41.6% [-44.4, -37.2] |
| cpp | zstd9 | 3724.4 | +54.2% | 24.052 | -44.0% [-48.7, -43.0] |
| cpp | zstd19 | 2722.2 | +12.7% | 27.266 | -37.9% [-41.0, -36.7] |
| scss | brotli11 | 1330.6 | +0.0% | 22.640 | +0.0% [+0.0, +0.0] |
| scss | zstd3 | 2365.3 | +77.8% | 14.107 | -39.2% [-39.7, -32.1] |
| scss | zstd9 | 1927.8 | +44.9% | 13.444 | -40.6% [-43.9, -40.5] |
| scss | zstd19 | 1375.9 | +3.4% | 16.899 | -25.4% [-29.5, -21.4] |
| java | brotli11 | 106.3 | +0.0% | 1.828 | +0.0% [+0.0, +0.0] |
| java | zstd3 | 157.6 | +48.3% | 1.044 | -42.9% [-48.7, -35.0] |
| java | zstd9 | 141.2 | +32.8% | 1.065 | -47.4% [-50.2, -35.9] |
| java | zstd19 | 116.6 | +9.7% | 1.089 | -38.3% [-46.2, -38.1] |
| json | brotli11 | 4.5 | +0.0% | 0.091 | +0.0% [+0.0, +0.0] |
| json | zstd3 | 5.3 | +18.2% | 0.041 | -51.1% [-59.3, -43.4] |
| json | zstd9 | 4.9 | +10.3% | 0.038 | -60.8% [-62.3, -31.1] |
| json | zstd19 | 4.3 | -4.6% | 0.040 | -55.5% [-60.8, -55.4] |
| rust | brotli11 | 41.0 | +0.0% | 0.686 | +0.0% [+0.0, +0.0] |
| rust | zstd3 | 55.9 | +36.3% | 0.357 | -48.6% [-53.3, -47.9] |
| rust | zstd9 | 51.2 | +24.7% | 0.318 | -53.1% [-57.9, -47.4] |
| rust | zstd19 | 42.8 | +4.2% | 0.342 | -50.2% [-53.8, -44.5] |
| html | brotli11 | 1064.2 | +0.0% | 19.027 | +0.0% [+0.0, +0.0] |
| html | zstd3 | 1786.3 | +67.9% | 10.872 | -43.3% [-43.4, -38.8] |
| html | zstd9 | 1466.6 | +37.8% | 9.991 | -46.9% [-49.6, -45.9] |
| html | zstd19 | 1154.4 | +8.5% | 11.439 | -39.2% [-41.1, -37.8] |
| astro | brotli11 | 1635.1 | +0.0% | 28.256 | +0.0% [+0.0, +0.0] |
| astro | zstd3 | 3179.3 | +94.4% | 18.670 | -34.1% [-35.7, -31.4] |
| astro | zstd9 | 2372.5 | +45.1% | 16.778 | -40.6% [-45.3, -39.3] |
| astro | zstd19 | 1779.2 | +8.8% | 18.274 | -35.3% [-38.5, -34.5] |
| tsx | brotli11 | 951.7 | +0.0% | 17.648 | +0.0% [+0.0, +0.0] |
| tsx | zstd3 | 1750.0 | +83.9% | 11.191 | -36.7% [-38.1, -26.9] |
| tsx | zstd9 | 1400.9 | +47.2% | 10.308 | -39.6% [-43.0, -37.7] |
| tsx | zstd19 | 1035.8 | +8.8% | 11.399 | -33.6% [-36.9, -31.9] |
| c | brotli11 | 267.2 | +0.0% | 5.012 | +0.0% [+0.0, +0.0] |
| c | zstd3 | 508.3 | +90.2% | 3.470 | -32.3% [-38.5, -25.9] |
| c | zstd9 | 381.2 | +42.7% | 2.947 | -42.4% [-47.7, -36.9] |
| c | zstd19 | 301.9 | +13.0% | 3.338 | -34.0% [-40.8, -30.0] |
| php | brotli11 | 1263.7 | +0.0% | 20.186 | +0.0% [+0.0, +0.0] |
| php | zstd3 | 2043.3 | +61.7% | 12.219 | -42.9% [-45.7, -37.1] |
| php | zstd9 | 1712.3 | +35.5% | 11.142 | -45.8% [-50.5, -42.4] |
| php | zstd19 | 1374.0 | +8.7% | 11.824 | -41.2% [-41.4, -38.1] |

Compression is performed once before any timing process. Its costs are retained in JSON but are single diagnostics, not repeated encode benchmarks. Higher levels trade producer time for package size; decode speed must be measured separately. Dictionaries and custom compression parameters were not used.

Node 24.22.0; Zstd 1.5.7; Brotli 1.2.0. See [Zstd](https://github.com/facebook/zstd) for the upstream compression-level model.

Reproduce with `scripts/compare-portable-codecs.mjs prepare ARTIFACTS CACHE`, then `measure ARTIFACTS CACHE CORPUS REPETITION` for each corpus and repetitions 0–2. The source, orchestration, raw samples, and source/input/encoded SHA-256 hashes are retained in [zstd-evidence.tar.gz](zstd-evidence.tar.gz).
