# Zstd compression-level comparison

Ten identical optimized portable scanner packages; three fresh Node processes per codec/corpus, three warmups and 15 timed decodes per process. Zstd frame checksums are enabled. Central times are medians of process means; percentage ranges retain all three paired process changes. Input/output identity and encoded hashes are checked. Dictionaries were not used.

## Sum of these ten packages

These sums describe this captured bundle mix, not an application startup time or a universal workload weighting. Encode time is irrelevant to these decode sums.

| Codec | Total MiB | Size vs 19 | Sum decode ms | Decode vs 19, median [range] |
| --- | ---: | ---: | ---: | ---: |
| brotli11 | 8.867 | -8.35% | 164.870 | +54.0% [+51.3, +55.0] |
| zstd9 | 12.874 | +33.07% | 95.615 | -11.0% [-12.9, -10.1] |
| zstd16 | 11.431 | +18.16% | 101.232 | -5.4% [-8.1, -4.8] |
| zstd17 | 10.957 | +13.25% | 100.588 | -6.8% [-8.0, -6.5] |
| zstd19 | 9.675 | +0.00% | 107.893 | +0.0% [+0.0, +0.0] |
| zstd21 | 9.471 | -2.11% | 102.913 | -3.2% [-5.0, -2.8] |
| zstd22 | 9.442 | -2.41% | 102.443 | -3.7% [-6.4, +5.7] |
| zstd21w23 | 9.669 | -0.07% | 108.477 | +2.0% [+0.0, +4.2] |
| zstd22w23 | 9.636 | -0.40% | 108.298 | +1.4% [-0.7, +1.8] |

## C++ example

| Codec | MiB | Decode ms | Window MiB | Encode s | Encode peak MiB | Decode peak MiB, median [range] |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| brotli11 | 2.359 | 44.824 | — | 57.088 | historical | 271.5 [262.7, 282.5] |
| zstd9 | 3.637 | 24.822 | 4.00 | 0.304 | historical | 272.5 [257.7, 296.9] |
| zstd16 | 3.226 | 27.039 | 4.00 | 4.181 | 138.9 | 284.5 [284.5, 288.1] |
| zstd17 | 3.104 | 26.608 | 8.00 | 4.695 | 158.7 | 275.7 [272.1, 278.8] |
| zstd19 | 2.658 | 29.270 | 8.00 | 8.490 | 190.4 | 280.5 [265.0, 282.2] |
| zstd21 | 2.522 | 27.877 | 24.22 | 8.988 | 446.0 | 278.8 [268.9, 288.3] |
| zstd22 | 2.510 | 27.520 | 24.22 | 10.260 | 510.1 | 266.2 [259.2, 289.7] |
| zstd21w23 | 2.660 | 29.075 | 8.00 | 8.588 | 238.4 | 283.8 [283.6, 292.7] |
| zstd22w23 | 2.650 | 29.194 | 8.00 | 9.572 | 238.1 | 274.7 [271.2, 292.0] |

## HTTP window constraint

[RFC 9659](https://www.rfc-editor.org/info/rfc9659/) limits `Content-Encoding: zstd` frames to an 8 MiB window. Default levels 21 and 22 exceed that bound on some larger packages. The `w23` variants explicitly set `ZSTD_c_windowLog=23`. Every actual frame header is inspected with zstd CLI 1.5.7, and all Zstd packages are tested with an 8 MiB Node streaming decoder limit. Oversized frames fail with the expected window-too-large error; capped frames pass and match the original payload hash. Larger windows remain a separate option for downloaded assets decoded by a client that supports them, without that HTTP content coding. Browser behavior and CDN integration are not benchmarked.

## Every corpus and level

| Corpus | Codec | KiB | Size vs 19 | Decode ms | Decode vs 19, median [range] | Frame window MiB | HTTP window limit |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| cpp | brotli11 | 2416.0 | -11.25% | 44.824 | +56.6% [+53.1, +56.9] | — | not Zstd |
| cpp | zstd9 | 3724.4 | +36.82% | 24.822 | -11.7% [-15.2, -9.8] | 4.00 | yes |
| cpp | zstd16 | 3303.9 | +21.37% | 27.039 | -6.9% [-7.6, -5.1] | 4.00 | yes |
| cpp | zstd17 | 3178.9 | +16.78% | 26.608 | -8.5% [-9.1, -8.5] | 8.00 | yes |
| cpp | zstd19 | 2722.2 | +0.00% | 29.270 | +0.0% [+0.0, +0.0] | 8.00 | yes |
| cpp | zstd21 | 2582.1 | -5.15% | 27.877 | -2.7% [-5.8, +1.2] | 24.22 | no |
| cpp | zstd22 | 2570.2 | -5.58% | 27.520 | -2.2% [-6.0, +29.4] | 24.22 | no |
| cpp | zstd21w23 | 2724.1 | +0.07% | 29.075 | +4.5% [-0.7, +11.5] | 8.00 | yes |
| cpp | zstd22w23 | 2713.4 | -0.32% | 29.194 | +0.5% [-0.3, +1.6] | 8.00 | yes |
| scss | brotli11 | 1330.6 | -3.30% | 23.567 | +34.5% [+34.3, +38.0] | — | not Zstd |
| scss | zstd9 | 1927.8 | +40.11% | 14.309 | -18.9% [-21.2, -18.5] | 4.00 | yes |
| scss | zstd16 | 1647.5 | +19.74% | 15.502 | -11.2% [-11.7, -8.2] | 4.00 | yes |
| scss | zstd17 | 1503.5 | +9.28% | 16.260 | -8.2% [-12.1, -7.2] | 8.00 | yes |
| scss | zstd19 | 1375.9 | +0.00% | 17.554 | +0.0% [+0.0, +0.0] | 8.00 | yes |
| scss | zstd21 | 1350.0 | -1.88% | 16.797 | -4.3% [-7.0, -1.5] | 12.66 | no |
| scss | zstd22 | 1348.4 | -2.00% | 17.083 | -2.7% [-5.3, -1.1] | 12.66 | no |
| scss | zstd21w23 | 1369.6 | -0.46% | 17.897 | +2.0% [+0.1, +3.7] | 8.00 | yes |
| scss | zstd22w23 | 1366.8 | -0.66% | 17.761 | +0.3% [-0.9, +9.7] | 8.00 | yes |
| java | brotli11 | 106.3 | -8.84% | 1.918 | +71.0% [+61.8, +71.5] | — | not Zstd |
| java | zstd9 | 141.2 | +21.10% | 1.066 | -3.7% [-11.1, +5.3] | 1.09 | yes |
| java | zstd16 | 129.9 | +11.42% | 1.094 | -1.1% [-14.9, +1.9] | 1.09 | yes |
| java | zstd17 | 130.2 | +11.68% | 1.198 | +8.3% [-6.3, +15.3] | 1.09 | yes |
| java | zstd19 | 116.6 | +0.00% | 1.122 | +0.0% [+0.0, +0.0] | 1.09 | yes |
| java | zstd21 | 116.5 | -0.03% | 1.267 | +13.0% [+4.5, +26.5] | 1.09 | yes |
| java | zstd22 | 116.3 | -0.20% | 1.167 | +5.5% [+2.1, +7.2] | 1.09 | yes |
| java | zstd21w23 | 116.5 | -0.03% | 1.186 | +5.7% [+1.7, +10.4] | 1.09 | yes |
| java | zstd22w23 | 116.3 | -0.20% | 1.184 | +0.9% [-0.5, +7.0] | 1.09 | yes |
| json | brotli11 | 4.5 | +4.80% | 0.103 | +85.0% [+56.4, +127.4] | — | not Zstd |
| json | zstd9 | 4.9 | +15.61% | 0.067 | +33.3% [-12.2, +39.1] | 0.03 | yes |
| json | zstd16 | 4.3 | +1.42% | 0.051 | +0.1% [-3.1, +13.4] | 0.03 | yes |
| json | zstd17 | 4.3 | +1.33% | 0.054 | +0.7% [-15.3, +7.8] | 0.03 | yes |
| json | zstd19 | 4.3 | +0.00% | 0.052 | +0.0% [+0.0, +0.0] | 0.03 | yes |
| json | zstd21 | 4.2 | -0.53% | 0.051 | -1.8% [-23.0, +1.5] | 0.03 | yes |
| json | zstd22 | 4.2 | -0.59% | 0.051 | -1.7% [-13.5, +1.2] | 0.03 | yes |
| json | zstd21w23 | 4.2 | -0.53% | 0.055 | +0.6% [-11.5, +5.8] | 0.03 | yes |
| json | zstd22w23 | 4.2 | -0.59% | 0.068 | +4.8% [+2.2, +31.1] | 0.03 | yes |
| rust | brotli11 | 41.0 | -4.01% | 0.701 | +88.7% [+78.7, +96.5] | — | not Zstd |
| rust | zstd9 | 51.2 | +19.69% | 0.402 | +0.8% [-14.5, +10.2] | 0.30 | yes |
| rust | zstd16 | 47.9 | +11.97% | 0.359 | -9.8% [-16.3, -0.9] | 0.30 | yes |
| rust | zstd17 | 48.4 | +13.21% | 0.417 | +6.4% [-9.5, +17.9] | 0.30 | yes |
| rust | zstd19 | 42.8 | +0.00% | 0.392 | +0.0% [+0.0, +0.0] | 0.30 | yes |
| rust | zstd21 | 42.7 | -0.07% | 0.360 | -4.9% [-9.7, -3.7] | 0.30 | yes |
| rust | zstd22 | 42.7 | -0.16% | 0.374 | -3.6% [-4.8, +3.8] | 0.30 | yes |
| rust | zstd21w23 | 42.7 | -0.07% | 0.384 | -2.2% [-5.2, +6.0] | 0.30 | yes |
| rust | zstd22w23 | 42.7 | -0.16% | 0.407 | +10.2% [-3.2, +12.9] | 0.30 | yes |
| html | brotli11 | 1064.2 | -7.82% | 18.997 | +58.0% [+57.0, +60.9] | — | not Zstd |
| html | zstd9 | 1466.6 | +27.05% | 10.728 | -11.3% [-11.4, -6.1] | 4.00 | yes |
| html | zstd16 | 1319.1 | +14.27% | 11.070 | -7.3% [-8.5, +0.4] | 4.00 | yes |
| html | zstd17 | 1268.3 | +9.86% | 10.955 | -8.2% [-9.5, -6.6] | 8.00 | yes |
| html | zstd19 | 1154.4 | +0.00% | 11.997 | +0.0% [+0.0, +0.0] | 8.00 | yes |
| html | zstd21 | 1155.4 | +0.09% | 11.609 | -3.6% [-4.1, -3.2] | 12.30 | no |
| html | zstd22 | 1152.0 | -0.21% | 11.365 | -6.1% [-6.6, +1.1] | 12.30 | no |
| html | zstd21w23 | 1155.5 | +0.09% | 12.321 | +2.7% [+2.0, +3.4] | 8.00 | yes |
| html | zstd22w23 | 1151.8 | -0.23% | 12.056 | +2.3% [-2.6, +5.9] | 8.00 | yes |
| astro | brotli11 | 1635.1 | -8.10% | 28.869 | +51.4% [+48.2, +53.9] | — | not Zstd |
| astro | zstd9 | 2372.5 | +33.35% | 17.517 | -8.5% [-9.8, -3.9] | 4.00 | yes |
| astro | zstd16 | 2105.9 | +18.37% | 17.735 | -7.3% [-7.8, -1.9] | 4.00 | yes |
| astro | zstd17 | 2018.2 | +13.43% | 17.452 | -9.8% [-10.1, -6.9] | 8.00 | yes |
| astro | zstd19 | 1779.2 | +0.00% | 19.140 | +0.0% [+0.0, +0.0] | 8.00 | yes |
| astro | zstd21 | 1737.4 | -2.35% | 17.799 | -7.2% [-8.3, -6.2] | 19.59 | no |
| astro | zstd22 | 1730.9 | -2.71% | 17.321 | -9.5% [-10.4, -9.4] | 19.59 | no |
| astro | zstd21w23 | 1778.1 | -0.06% | 19.019 | -1.1% [-1.3, +1.4] | 8.00 | yes |
| astro | zstd22w23 | 1767.3 | -0.67% | 18.630 | -2.7% [-4.5, +2.1] | 8.00 | yes |
| tsx | brotli11 | 951.7 | -8.11% | 17.440 | +48.3% [+38.3, +53.6] | — | not Zstd |
| tsx | zstd9 | 1400.9 | +35.26% | 10.816 | -12.2% [-13.1, -11.6] | 4.00 | yes |
| tsx | zstd16 | 1248.0 | +20.49% | 11.177 | -7.8% [-11.7, -5.0] | 4.00 | yes |
| tsx | zstd17 | 1198.5 | +15.71% | 12.204 | -3.0% [-6.8, -2.0] | 8.00 | yes |
| tsx | zstd19 | 1035.8 | +0.00% | 12.450 | +0.0% [+0.0, +0.0] | 8.00 | yes |
| tsx | zstd21 | 1036.5 | +0.07% | 11.505 | -2.2% [-10.8, -1.8] | 13.20 | no |
| tsx | zstd22 | 1033.9 | -0.18% | 11.144 | -5.3% [-15.4, -3.0] | 13.20 | no |
| tsx | zstd21w23 | 1035.9 | +0.01% | 12.443 | +2.6% [-5.5, +5.8] | 8.00 | yes |
| tsx | zstd22w23 | 1034.2 | -0.15% | 12.542 | +2.7% [-4.2, +6.6] | 8.00 | yes |
| c | brotli11 | 267.2 | -11.48% | 5.374 | +38.2% [+37.9, +40.4] | — | not Zstd |
| c | zstd9 | 381.2 | +26.30% | 3.349 | -11.3% [-15.2, -10.4] | 3.88 | yes |
| c | zstd16 | 346.5 | +14.79% | 3.426 | -10.1% [-11.8, -6.4] | 3.88 | yes |
| c | zstd17 | 338.7 | +12.20% | 3.413 | -10.8% [-11.2, -9.3] | 3.88 | yes |
| c | zstd19 | 301.9 | +0.00% | 3.827 | +0.0% [+0.0, +0.0] | 3.88 | yes |
| c | zstd21 | 300.8 | -0.35% | 3.754 | -0.2% [-1.9, +0.8] | 3.88 | yes |
| c | zstd22 | 302.1 | +0.08% | 3.777 | -1.3% [-2.9, +1.6] | 3.88 | yes |
| c | zstd21w23 | 300.8 | -0.35% | 3.793 | -0.9% [-1.0, +0.4] | 3.88 | yes |
| c | zstd22w23 | 302.1 | +0.08% | 3.761 | -1.5% [-1.7, +4.2] | 3.88 | yes |
| php | brotli11 | 1263.7 | -8.03% | 22.007 | +75.4% [+69.7, +94.9] | — | not Zstd |
| php | zstd9 | 1712.3 | +24.62% | 12.027 | -5.3% [-8.1, -3.5] | 4.00 | yes |
| php | zstd16 | 1552.8 | +13.01% | 13.048 | +2.7% [-2.0, +5.4] | 4.00 | yes |
| php | zstd17 | 1531.0 | +11.43% | 12.800 | +0.8% [-3.1, +0.9] | 8.00 | yes |
| php | zstd19 | 1374.0 | +0.00% | 12.701 | +0.0% [+0.0, +0.0] | 8.00 | yes |
| php | zstd21 | 1372.3 | -0.13% | 12.565 | -1.8% [-3.5, +0.2] | 13.11 | no |
| php | zstd22 | 1367.7 | -0.46% | 12.593 | -0.8% [-2.5, +7.6] | 13.11 | no |
| php | zstd21w23 | 1373.1 | -0.07% | 12.878 | -0.4% [-0.5, +1.4] | 8.00 | yes |
| php | zstd22w23 | 1368.2 | -0.43% | 12.837 | +0.3% [-0.4, +1.1] | 8.00 | yes |

## Measurement scope and reproduction

- Node 24.22.0, Zstd 1.5.7, Brotli 1.2.0; local macOS arm64 on the same M1 Ultra host as the preceding Rust matrix.
- The previous comparison grouped codecs in one process. This round isolates every codec process, including Brotli, 9, and 19 controls; compare within this matrix rather than splicing old absolute decode times.
- Single encode timings and encoder RSS are retained for the five new/rebuilt Zstd levels. Brotli and 9 encode times are historical and no comparable isolated encoder RSS is claimed for them. Decoder RSS includes 18 decodes, source-buffer construction, allocation retention and GC; it is not a codec workspace measurement.
- The Rust engine, search kernel, and raw archive payloads are unchanged. No dependency forks or new Cargo dependencies are involved.
- Existing reports, encoded baselines, and raw samples were preserved. [summary.json](summary.json) includes all per-process means, paired comparisons against both Brotli and 19, memory observations, runtime versions, window checks, and hashes. [evidence.tar.gz](evidence.tar.gz) includes exact preparation/measurement sources, phase source versions, and all raw decode samples.

Use `scripts/compare-portable-codecs.mjs prepare-one ARTIFACTS CACHE CORPUS CODEC` for each encoding and `PORTABLE_CODECS=CODEC node scripts/compare-portable-codecs.mjs measure ARTIFACTS CACHE CORPUS REPETITION` for repetitions 0–2. The archived orchestration scripts retain the exact ordering, frozen reference selection and checks.
