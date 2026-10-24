# Portable load optimization: complete local comparison

`portable` is the load-optimized fork-free kernel. `before` is the frozen merged portable executable, `prepared` is the frozen dependency snapshot, and `control` runs ordinary Eager through the new portable-feature executable.

## Absolute measurements

Central timings are medians of three Criterion process means. RSS is the median of three fresh-process peaks. Portable Brotli-11 sizes are new; unchanged reference sizes are reused after payload hash checks. Before uses FPRT001, optimized uses FPRT002, and control uses only the Eager image.

| Corpus | Variant | Load ms | Warm search ms | First document ms | Brotli-11 KiB | gzip-9 KiB | Peak RSS MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cpp | eager | 92.384 | 2.984 | 105.963 | 38.6 | 189.3 | 98.2 |
| cpp | prepared | 21.890 | 3.056 | 33.361 | 1460.6 | 3689.3 | 111.7 |
| cpp | before | 25.157 | 3.085 | 32.329 | 2420.6 | 5547.2 | 105.6 |
| cpp | portable | 19.835 | 3.200 | 26.717 | 2416.0 | 5547.2 | 105.5 |
| cpp | control | 93.471 | 3.100 | 105.790 | 38.6 | 189.3 | 98.2 |
| scss | eager | 49.747 | 1.258 | 56.495 | 38.6 | 82.7 | 44.9 |
| scss | prepared | 11.366 | 1.238 | 17.138 | 1091.4 | 2127.9 | 51.8 |
| scss | before | 9.882 | 1.191 | 13.383 | 1328.4 | 2863.7 | 51.8 |
| scss | portable | 7.064 | 1.221 | 10.618 | 1330.6 | 2863.7 | 51.9 |
| scss | control | 49.543 | 1.297 | 55.980 | 38.6 | 82.7 | 47.5 |
| java | eager | 5.370 | 1.062 | 8.720 | 11.4 | 17.3 | 12.5 |
| java | prepared | 1.583 | 1.070 | 4.683 | 77.0 | 151.5 | 12.8 |
| java | before | 1.834 | 1.137 | 3.866 | 106.7 | 223.4 | 13.1 |
| java | portable | 1.591 | 1.143 | 3.666 | 106.3 | 223.4 | 13.1 |
| java | control | 5.277 | 1.116 | 8.633 | 11.4 | 17.3 | 12.8 |
| json | eager | 0.172 | 0.521 | 0.746 | 1.6 | 1.7 | 6.6 |
| json | prepared | 0.108 | 0.517 | 0.682 | 4.0 | 4.5 | 6.7 |
| json | before | 0.123 | 0.561 | 0.688 | 4.5 | 5.2 | 6.6 |
| json | portable | 0.117 | 0.556 | 0.681 | 4.5 | 5.2 | 6.8 |
| json | control | 0.173 | 0.581 | 0.776 | 1.6 | 1.7 | 6.7 |
| rust | eager | 2.140 | 0.707 | 3.790 | 7.0 | 8.3 | 9.5 |
| rust | prepared | 0.634 | 0.700 | 2.240 | 30.5 | 50.2 | 9.3 |
| rust | before | 0.785 | 0.774 | 1.919 | 41.0 | 66.5 | 9.2 |
| rust | portable | 0.705 | 0.757 | 1.878 | 41.0 | 66.5 | 9.4 |
| rust | control | 2.127 | 0.741 | 3.786 | 7.0 | 8.3 | 9.4 |
| html | eager | 43.794 | 4.187 | 53.028 | 113.6 | 304.3 | 77.9 |
| html | prepared | 13.304 | 4.268 | 22.022 | 755.7 | 1590.9 | 76.8 |
| html | before | 14.470 | 4.394 | 21.867 | 1068.0 | 2235.2 | 79.6 |
| html | portable | 11.856 | 4.413 | 19.190 | 1064.2 | 2235.2 | 81.0 |
| html | control | 43.575 | 4.267 | 52.117 | 113.6 | 304.3 | 78.1 |
| astro | eager | 75.314 | 1.789 | 81.528 | 111.1 | 331.3 | 87.0 |
| astro | prepared | 18.061 | 1.793 | 23.499 | 1041.2 | 2543.9 | 94.0 |
| astro | before | 20.725 | 1.909 | 25.108 | 1636.1 | 3737.9 | 99.8 |
| astro | portable | 16.446 | 1.943 | 20.369 | 1635.1 | 3737.9 | 98.3 |
| astro | control | 75.405 | 1.892 | 81.239 | 111.1 | 331.3 | 87.3 |
| tsx | eager | 47.590 | 7.559 | 61.538 | 39.9 | 146.1 | 75.6 |
| tsx | prepared | 10.372 | 7.471 | 22.951 | 572.5 | 1582.8 | 75.5 |
| tsx | before | 12.583 | 8.173 | 23.908 | 952.7 | 2394.4 | 82.2 |
| tsx | portable | 9.648 | 8.109 | 20.969 | 951.7 | 2394.4 | 82.5 |
| tsx | control | 49.140 | 7.836 | 62.768 | 39.9 | 146.1 | 76.0 |
| c | eager | 14.076 | 0.989 | 18.080 | 17.4 | 25.1 | 25.3 |
| c | prepared | 3.516 | 0.988 | 7.491 | 201.7 | 543.9 | 27.6 |
| c | before | 4.748 | 1.092 | 7.073 | 267.7 | 855.0 | 26.5 |
| c | portable | 3.794 | 1.091 | 6.211 | 267.2 | 855.0 | 26.5 |
| c | control | 13.985 | 1.063 | 18.045 | 17.4 | 25.1 | 25.4 |
| php | eager | 50.533 | 0.810 | 54.321 | 169.1 | 331.8 | 60.5 |
| php | prepared | 15.928 | 0.790 | 19.179 | 958.7 | 1823.9 | 58.5 |
| php | before | 17.993 | 0.872 | 20.054 | 1263.8 | 2535.4 | 59.2 |
| php | portable | 15.131 | 0.859 | 17.138 | 1263.7 | 2535.4 | 59.5 |
| php | control | 50.324 | 0.842 | 53.812 | 169.1 | 331.8 | 61.3 |

## Optimized portable versus each reference

Negative changes mean less time. Brackets show the range of three paired process changes, not a confidence interval. A repeatable regression here means at least 3% in each repetition. Build and code-layout effects remain possible; the control helps expose them but cannot remove them from the frozen historical builds.

| Corpus | Reference | Load change | Warm search change | First document change |
| --- | --- | ---: | ---: | ---: |
| cpp | eager | -78.5% [-79.0, -78.2] | +7.7% [+0.2, +8.3] | -74.5% [-75.7, -74.4] |
| cpp | prepared | -9.4% [-10.3, -8.7] | +2.2% [+1.6, +5.8] | -18.8% [-22.2, -18.1] |
| cpp | before | -22.5% [-22.7, -20.7] | +1.0% [-0.0, +6.0] | -18.1% [-22.2, -16.7] |
| cpp | control | -78.6% [-79.3, -78.6] | +3.6% [-1.6, +4.3] | -74.7% [-75.0, -74.6] |
| scss | eager | -85.8% [-85.8, -85.8] | -3.8% [-4.6, +0.8] | -81.2% [-81.7, -80.6] |
| scss | prepared | -37.5% [-38.9, -36.8] | +0.9% [-3.1, +0.9] | -38.1% [-38.2, -35.6] |
| scss | before | -28.2% [-28.5, -27.9] | +1.3% [-1.4, +5.3] | -20.4% [-20.9, -19.4] |
| scss | control | -85.7% [-85.8, -85.7] | -7.1% [-7.3, -3.3] | -80.9% [-81.0, -80.9] |
| java | eager | -70.4% [-70.7, -70.1] | +8.4% [+6.4, +12.3] | -57.8% [-58.4, -56.9] |
| java | prepared | -0.5% [-1.2, +1.8] | +7.7% [+5.7, +10.3] | -21.7% [-21.7, -21.5] |
| java | before | -13.3% [-13.8, -12.8] | +0.1% [-0.4, +1.0] | -5.2% [-6.1, -3.1] |
| java | control | -70.3% [-70.3, -69.5] | +1.5% [+0.9, +3.9] | -57.8% [-57.8, -57.5] |
| json | eager | -31.7% [-33.0, -30.3] | +7.9% [+5.9, +8.1] | -9.9% [-11.3, -8.6] |
| json | prepared | +8.6% [+6.4, +10.7] | +8.7% [+7.6, +11.6] | -0.6% [-0.8, -0.1] |
| json | before | -4.6% [-6.9, -4.3] | -0.9% [-1.5, -0.8] | -3.2% [-3.4, -0.9] |
| json | control | -32.1% [-32.2, -30.2] | -4.3% [-4.4, -3.9] | -13.0% [-13.4, -12.2] |
| rust | eager | -66.0% [-67.1, -65.5] | +8.3% [+7.1, +9.3] | -51.0% [-52.0, -50.3] |
| rust | prepared | +13.6% [+11.2, +13.9] | +10.6% [+8.0, +10.8] | -16.1% [-17.7, -15.9] |
| rust | before | -9.5% [-10.3, -7.4] | -0.7% [-2.2, +0.2] | -3.5% [-3.6, -1.9] |
| rust | control | -65.8% [-67.4, -65.7] | +3.4% [+2.2, +3.7] | -50.4% [-51.7, -50.2] |
| html | eager | -73.1% [-73.9, -72.9] | +5.4% [+2.1, +7.6] | -63.9% [-65.4, -63.6] |
| html | prepared | -10.9% [-11.8, -10.7] | +5.3% [+2.9, +7.6] | -13.2% [-14.1, -12.9] |
| html | before | -18.5% [-19.1, -17.0] | +0.4% [+0.3, +1.1] | -12.5% [-13.0, -12.2] |
| html | control | -73.2% [-73.4, -72.8] | +2.1% [+0.4, +3.4] | -64.3% [-64.8, -63.0] |
| astro | eager | -77.8% [-78.2, -77.7] | +7.6% [+3.3, +9.3] | -75.0% [-75.1, -75.0] |
| astro | prepared | -8.0% [-10.8, -7.6] | +8.5% [+2.3, +8.7] | -13.1% [-13.5, -13.0] |
| astro | before | -20.6% [-25.1, -19.2] | -0.6% [-1.3, +1.8] | -18.9% [-24.3, -16.2] |
| astro | control | -78.2% [-78.5, -77.7] | +2.7% [-1.1, +2.8] | -75.3% [-75.5, -74.1] |
| tsx | eager | -79.9% [-80.1, -79.7] | +8.0% [+7.2, +8.6] | -65.6% [-66.6, -65.5] |
| tsx | prepared | -7.9% [-9.2, -6.0] | +8.5% [+7.3, +9.7] | -8.6% [-9.1, -7.5] |
| tsx | before | -23.6% [-24.2, -22.8] | +0.4% [-1.7, +1.3] | -11.3% [-12.3, -11.0] |
| tsx | control | -80.2% [-81.1, -80.2] | +3.5% [-7.7, +5.2] | -66.2% [-68.1, -65.5] |
| c | eager | -73.0% [-73.6, -71.7] | +10.3% [+9.5, +13.3] | -65.6% [-66.1, -65.1] |
| c | prepared | +7.5% [+6.1, +11.0] | +11.1% [+9.7, +11.3] | -16.9% [-17.3, -15.8] |
| c | before | -18.1% [-21.4, -18.1] | -0.1% [-1.2, +3.1] | -12.2% [-13.6, -11.5] |
| c | control | -73.1% [-73.3, -71.4] | +2.7% [+1.9, +5.3] | -65.6% [-66.0, -65.3] |
| php | eager | -69.9% [-71.0, -69.3] | +7.6% [+4.9, +9.4] | -68.2% [-69.8, -68.2] |
| php | prepared | -4.7% [-7.3, -2.6] | +8.9% [+8.7, +11.4] | -11.1% [-14.9, -10.6] |
| php | before | -16.2% [-17.5, -13.8] | -1.5% [-4.0, +2.7] | -14.3% [-15.6, -13.9] |
| php | control | -69.9% [-70.1, -69.4] | +1.9% [+1.3, +5.4] | -68.1% [-68.2, -67.9] |

## Memory counters

These are partial internal estimates after warm searches, not all heap allocations. Flat rows include power-of-two padding; retained input images and allocator high-water marks are represented by process RSS above.

| Corpus | Variant | Immutable MiB | Scanner scratch MiB | Cache clears, three processes | Retired groups |
| --- | --- | ---: | ---: | --- | --- |
| cpp | eager | 23.87 | 34.27 | [0, 0, 0] | [0, 0, 0] |
| cpp | prepared | 23.87 | 34.27 | [0, 0, 0] | [0, 0, 0] |
| cpp | before | 26.88 | 9.30 | [0, 0, 0] | [0, 0, 0] |
| cpp | portable | 26.88 | 9.30 | [0, 0, 0] | [0, 0, 0] |
| cpp | control | 23.87 | 34.27 | [0, 0, 0] | [0, 0, 0] |
| scss | eager | 14.20 | 12.17 | [0, 0, 0] | [0, 0, 0] |
| scss | prepared | 14.20 | 12.17 | [0, 0, 0] | [0, 0, 0] |
| scss | before | 15.76 | 4.51 | [0, 0, 0] | [0, 0, 0] |
| scss | portable | 15.76 | 4.51 | [0, 0, 0] | [0, 0, 0] |
| scss | control | 14.20 | 12.17 | [0, 0, 0] | [0, 0, 0] |
| java | eager | 0.89 | 1.46 | [0, 0, 0] | [0, 0, 0] |
| java | prepared | 0.89 | 1.46 | [0, 0, 0] | [0, 0, 0] |
| java | before | 0.94 | 0.73 | [0, 0, 0] | [0, 0, 0] |
| java | portable | 0.94 | 0.73 | [0, 0, 0] | [0, 0, 0] |
| java | control | 0.89 | 1.46 | [0, 0, 0] | [0, 0, 0] |
| json | eager | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| json | prepared | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| json | before | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| json | portable | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| json | control | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| rust | eager | 0.29 | 0.51 | [0, 0, 0] | [0, 0, 0] |
| rust | prepared | 0.29 | 0.51 | [0, 0, 0] | [0, 0, 0] |
| rust | before | 0.28 | 0.33 | [0, 0, 0] | [0, 0, 0] |
| rust | portable | 0.28 | 0.33 | [0, 0, 0] | [0, 0, 0] |
| rust | control | 0.29 | 0.51 | [0, 0, 0] | [0, 0, 0] |
| html | eager | 8.64 | 11.54 | [0, 0, 0] | [0, 0, 0] |
| html | prepared | 8.64 | 11.54 | [0, 0, 0] | [0, 0, 0] |
| html | before | 9.44 | 4.49 | [0, 0, 0] | [0, 0, 0] |
| html | portable | 9.44 | 4.49 | [0, 0, 0] | [0, 0, 0] |
| html | control | 8.64 | 11.54 | [0, 0, 0] | [0, 0, 0] |
| astro | eager | 14.82 | 17.09 | [0, 0, 0] | [0, 0, 0] |
| astro | prepared | 14.82 | 17.09 | [0, 0, 0] | [0, 0, 0] |
| astro | before | 16.11 | 5.03 | [0, 0, 0] | [0, 0, 0] |
| astro | portable | 16.11 | 5.03 | [0, 0, 0] | [0, 0, 0] |
| astro | control | 14.82 | 17.09 | [0, 0, 0] | [0, 0, 0] |
| tsx | eager | 9.54 | 10.33 | [0, 0, 0] | [0, 0, 0] |
| tsx | prepared | 9.54 | 10.33 | [0, 0, 0] | [0, 0, 0] |
| tsx | before | 10.14 | 4.43 | [0, 0, 0] | [0, 0, 0] |
| tsx | portable | 10.14 | 4.43 | [0, 0, 0] | [0, 0, 0] |
| tsx | control | 9.54 | 10.33 | [0, 0, 0] | [0, 0, 0] |
| c | eager | 3.86 | 7.68 | [0, 0, 0] | [0, 0, 0] |
| c | prepared | 3.86 | 7.68 | [0, 0, 0] | [0, 0, 0] |
| c | before | 4.23 | 2.35 | [0, 0, 0] | [0, 0, 0] |
| c | portable | 4.23 | 2.35 | [0, 0, 0] | [0, 0, 0] |
| c | control | 3.86 | 7.68 | [0, 0, 0] | [0, 0, 0] |
| php | eager | 10.22 | 10.35 | [0, 0, 0] | [0, 0, 0] |
| php | prepared | 10.22 | 10.35 | [0, 0, 0] | [0, 0, 0] |
| php | before | 11.34 | 3.30 | [0, 0, 0] | [0, 0, 0] |
| php | portable | 11.34 | 3.30 | [0, 0, 0] | [0, 0, 0] |
| php | control | 10.22 | 10.35 | [0, 0, 0] | [0, 0, 0] |
