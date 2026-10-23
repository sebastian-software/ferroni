# Portable scanner bytecode: local comparison

This experiment replaces dependency-private scanner-group snapshots with an owned instruction format and a native interpreter. The producer and consumer were built against unmodified registry releases of regex-automata 0.4.18 and aho-corasick 1.1.5. The old snapshot executable is retained unchanged as the comparison.

## All ten corpora

Each time is the median of three Criterion process means. Sizes contain the ordinary v2 program archive plus the optional scanner-group image. RSS is the median of three fresh-process peaks, including retained archives, fixture parsing, programs, scanners, scratch, and four document passes.

| Corpus | Variant | Load ms | Warm search ms | Cold document ms | Brotli-11 KiB | gzip-9 KiB | Peak RSS MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cpp | eager | 89.456 | 2.866 | 102.237 | 38.6 | 189.3 | 99.3 |
| cpp | prepared | 20.546 | 2.962 | 31.812 | 1460.6 | 3689.3 | 111.2 |
| cpp | portable | 25.126 | 3.669 | 33.781 | 2420.6 | 5547.2 | 106.1 |
| scss | eager | 47.995 | 1.195 | 53.899 | 38.6 | 82.7 | 45.9 |
| scss | prepared | 11.131 | 1.210 | 16.662 | 1091.4 | 2127.9 | 51.2 |
| scss | portable | 9.621 | 1.842 | 13.691 | 1328.4 | 2863.7 | 50.6 |
| java | eager | 5.284 | 1.031 | 8.456 | 11.4 | 17.3 | 12.3 |
| java | prepared | 1.484 | 1.023 | 4.568 | 77.0 | 151.5 | 12.5 |
| java | portable | 1.776 | 2.162 | 4.824 | 106.7 | 223.4 | 12.9 |
| json | eager | 0.168 | 0.510 | 0.738 | 1.6 | 1.7 | 6.6 |
| json | prepared | 0.105 | 0.506 | 0.657 | 4.0 | 4.5 | 6.6 |
| json | portable | 0.122 | 1.049 | 1.214 | 4.5 | 5.2 | 6.7 |
| rust | eager | 2.076 | 0.692 | 3.626 | 7.0 | 8.3 | 9.3 |
| rust | prepared | 0.628 | 0.674 | 2.180 | 30.5 | 50.2 | 9.3 |
| rust | portable | 0.773 | 1.328 | 2.507 | 41.0 | 66.5 | 9.2 |
| html | eager | 43.579 | 4.054 | 52.372 | 113.6 | 304.3 | 76.8 |
| html | prepared | 12.974 | 3.942 | 21.258 | 755.7 | 1590.9 | 77.5 |
| html | portable | 14.043 | 8.271 | 25.529 | 1068.0 | 2235.2 | 79.9 |
| astro | eager | 73.794 | 1.747 | 79.015 | 111.1 | 331.3 | 85.8 |
| astro | prepared | 17.923 | 1.747 | 22.700 | 1041.2 | 2543.9 | 94.1 |
| astro | portable | 20.274 | 2.643 | 24.575 | 1636.1 | 3737.9 | 99.1 |
| tsx | eager | 47.203 | 7.334 | 59.390 | 39.9 | 146.1 | 73.6 |
| tsx | prepared | 10.352 | 7.301 | 22.449 | 572.5 | 1582.8 | 75.3 |
| tsx | portable | 12.386 | 17.640 | 32.981 | 952.7 | 2394.4 | 81.8 |
| c | eager | 14.074 | 1.011 | 18.117 | 17.4 | 25.1 | 25.2 |
| c | prepared | 3.460 | 0.959 | 7.256 | 201.7 | 543.9 | 25.9 |
| c | portable | 4.565 | 2.104 | 8.025 | 267.7 | 855.0 | 26.4 |
| php | eager | 50.455 | 0.778 | 53.521 | 169.1 | 331.8 | 59.4 |
| php | prepared | 15.822 | 0.792 | 19.284 | 958.7 | 1823.9 | 58.5 |
| php | portable | 17.542 | 1.415 | 20.127 | 1263.8 | 2535.4 | 59.0 |

## Portable versus both references

Negative percentages mean less time. Brackets retain the minimum and maximum paired change across all three repetitions; they are not confidence intervals.

| Corpus | Baseline | Load change | Warm search change | Cold document change |
| --- | --- | ---: | ---: | ---: |
| cpp | eager | -73.2% [-75.8, -71.3] | +25.7% [+17.5, +28.0] | -67.0% [-67.3, -53.6] |
| cpp | prepared | +20.2% [+16.8, +27.9] | +24.5% [+10.8, +30.6] | +6.3% [+6.2, +51.4] |
| scss | eager | -80.3% [-80.3, -79.7] | +53.6% [+53.2, +55.9] | -75.0% [-75.1, -74.3] |
| scss | prepared | -14.2% [-14.9, -12.4] | +52.2% [+47.8, +56.0] | -17.8% [-19.2, -17.5] |
| java | eager | -66.1% [-66.4, -65.8] | +108.9% [+106.2, +109.6] | -42.6% [-43.2, -42.1] |
| java | prepared | +17.5% [+15.6, +21.6] | +108.8% [+108.5, +111.3] | +5.3% [+3.3, +6.3] |
| json | eager | -28.3% [-28.7, -19.3] | +105.5% [+102.6, +108.6] | +65.5% [+64.1, +66.4] |
| json | prepared | +16.2% [+13.8, +26.9] | +107.3% [+107.1, +112.1] | +84.8% [+79.6, +88.0] |
| rust | eager | -62.8% [-67.0, -61.9] | +92.8% [+90.0, +94.4] | -31.9% [-32.4, -30.8] |
| rust | prepared | +24.2% [+21.0, +25.2] | +95.1% [+94.0, +98.4] | +14.7% [+13.0, +15.0] |
| html | eager | -67.3% [-68.3, -67.2] | +104.0% [+100.8, +108.1] | -51.1% [-53.5, -51.1] |
| html | prepared | +7.8% [+7.1, +10.2] | +109.0% [+108.9, +109.8] | +18.4% [+15.9, +20.1] |
| astro | eager | -72.4% [-72.6, -72.2] | +50.5% [+49.6, +51.3] | -69.0% [-69.3, -68.8] |
| astro | prepared | +13.3% [+13.1, +13.5] | +48.6% [+47.4, +53.0] | +8.8% [+7.4, +8.8] |
| tsx | eager | -73.8% [-73.8, -73.5] | +141.0% [+138.8, +246.2] | -44.4% [-45.1, -21.2] |
| tsx | prepared | +19.6% [+18.4, +22.0] | +146.1% [+139.9, +255.7] | +47.7% [+45.3, +127.6] |
| c | eager | -67.6% [-78.5, -67.0] | +106.2% [+71.3, +114.6] | -55.8% [-56.0, -53.9] |
| c | prepared | +31.5% [+8.0, +32.5] | +117.5% [+95.0, +121.1] | +10.6% [+3.4, +10.7] |
| php | eager | -64.9% [-66.2, -64.7] | +81.9% [+71.4, +85.9] | -62.1% [-63.2, -61.7] |
| php | prepared | +11.9% [+9.9, +12.7] | +78.6% [+77.5, +82.4] | +6.3% [+3.7, +7.5] |

## Decompression is additional work

Rust load excludes decompression, file I/O, and transfer. The following sum combines separately measured medians; it is an estimate, not a measured application-startup duration. Codec timings are diagnostic (Node Brotli, CPython gzip); compression ran during validation, separately from the Rust comparison. Network transfer and the actual Ferriki integration remain unmeasured.

| Corpus | Variant | Brotli decode ms | Rust load ms | Sum ms |
| --- | --- | ---: | ---: | ---: |
| cpp | eager | 1.056 | 89.456 | 90.512 |
| cpp | prepared | 25.795 | 20.546 | 46.341 |
| cpp | portable | 43.208 | 25.126 | 68.334 |
| scss | eager | 0.606 | 47.995 | 48.601 |
| scss | prepared | 16.795 | 11.131 | 27.925 |
| scss | portable | 22.916 | 9.621 | 32.537 |
| java | eager | 0.160 | 5.284 | 5.443 |
| java | prepared | 1.130 | 1.484 | 2.614 |
| java | portable | 1.686 | 1.776 | 3.461 |
| json | eager | 0.026 | 0.168 | 0.193 |
| json | prepared | 0.058 | 0.105 | 0.162 |
| json | portable | 0.084 | 0.122 | 0.205 |
| rust | eager | 0.116 | 2.076 | 2.193 |
| rust | prepared | 0.474 | 0.628 | 1.101 |
| rust | portable | 0.620 | 0.773 | 1.393 |
| html | eager | 2.173 | 43.579 | 45.752 |
| html | prepared | 12.060 | 12.974 | 25.034 |
| html | portable | 18.712 | 14.043 | 32.755 |
| astro | eager | 2.254 | 73.794 | 76.048 |
| astro | prepared | 38.136 | 17.923 | 56.059 |
| astro | portable | 29.686 | 20.274 | 49.959 |
| tsx | eager | 1.732 | 47.203 | 48.935 |
| tsx | prepared | 10.120 | 10.352 | 20.472 |
| tsx | portable | 16.748 | 12.386 | 29.134 |
| c | eager | 0.244 | 14.074 | 14.318 |
| c | prepared | 3.414 | 3.460 | 6.874 |
| c | portable | 4.789 | 4.565 | 9.355 |
| php | eager | 2.562 | 50.455 | 53.017 |
| php | prepared | 14.990 | 15.822 | 30.812 |
| php | portable | 19.721 | 17.542 | 37.263 |
