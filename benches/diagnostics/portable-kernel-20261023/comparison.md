# Portable kernel: complete local comparison

`portable` is the optimized fork-free kernel. `before` is the frozen original portable executable, `prepared` is the frozen dependency snapshot, and `control` runs ordinary Eager through the new portable-feature executable.

## Absolute measurements

Central timings are medians of three Criterion process means. RSS is the median of three fresh-process peaks. Brotli-11 sizes are reused from the previous experiment after verifying identical payload hashes; its codec timings were not rerun. The before image is identical to the optimized image, and control uses the Eager image. Absolute medians and median paired percentages can differ in sign (for example SCSS), because their middle repetitions need not be the same.

| Corpus | Variant | Load ms | Warm search ms | First document ms | Brotli-11 KiB | gzip-9 KiB | Peak RSS MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cpp | eager | 91.475 | 2.906 | 102.200 | 38.6 | 189.3 | 98.7 |
| cpp | prepared | 21.096 | 2.865 | 31.879 | 1460.6 | 3689.3 | 111.5 |
| cpp | before | 24.645 | 3.621 | 31.455 | 2420.6 | 5547.2 | 105.6 |
| cpp | portable | 24.705 | 2.985 | 31.498 | 2420.6 | 5547.2 | 106.0 |
| cpp | control | 91.549 | 2.925 | 103.113 | 38.6 | 189.3 | 98.4 |
| scss | eager | 48.793 | 1.203 | 55.198 | 38.6 | 82.7 | 46.1 |
| scss | prepared | 11.377 | 1.275 | 17.230 | 1091.4 | 2127.9 | 51.2 |
| scss | before | 9.868 | 1.843 | 14.423 | 1328.4 | 2863.7 | 51.0 |
| scss | portable | 10.204 | 1.179 | 13.151 | 1328.4 | 2863.7 | 51.9 |
| scss | control | 49.609 | 1.276 | 57.781 | 38.6 | 82.7 | 47.0 |
| java | eager | 5.295 | 1.028 | 8.557 | 11.4 | 17.3 | 12.7 |
| java | prepared | 1.458 | 1.008 | 4.387 | 77.0 | 151.5 | 12.5 |
| java | before | 1.754 | 2.100 | 4.812 | 106.7 | 223.4 | 12.9 |
| java | portable | 1.805 | 1.103 | 3.867 | 106.7 | 223.4 | 13.2 |
| java | control | 5.297 | 1.085 | 8.627 | 11.4 | 17.3 | 12.9 |
| json | eager | 0.173 | 0.504 | 0.732 | 1.6 | 1.7 | 6.8 |
| json | prepared | 0.107 | 0.507 | 0.679 | 4.0 | 4.5 | 6.6 |
| json | before | 0.123 | 1.040 | 1.235 | 4.5 | 5.2 | 6.8 |
| json | portable | 0.123 | 0.536 | 0.686 | 4.5 | 5.2 | 6.7 |
| json | control | 0.171 | 0.541 | 0.778 | 1.6 | 1.7 | 6.8 |
| rust | eager | 2.037 | 0.685 | 3.684 | 7.0 | 8.3 | 9.2 |
| rust | prepared | 0.633 | 0.685 | 2.168 | 30.5 | 50.2 | 9.4 |
| rust | before | 0.743 | 1.308 | 2.457 | 41.0 | 66.5 | 9.3 |
| rust | portable | 0.748 | 0.711 | 1.906 | 41.0 | 66.5 | 9.3 |
| rust | control | 2.021 | 0.697 | 3.651 | 7.0 | 8.3 | 9.5 |
| html | eager | 43.434 | 4.072 | 52.433 | 113.6 | 304.3 | 78.0 |
| html | prepared | 13.190 | 4.152 | 21.984 | 755.7 | 1590.9 | 77.7 |
| html | before | 14.456 | 8.447 | 25.688 | 1068.0 | 2235.2 | 80.1 |
| html | portable | 14.288 | 4.067 | 21.717 | 1068.0 | 2235.2 | 80.6 |
| html | control | 43.761 | 4.219 | 52.964 | 113.6 | 304.3 | 77.3 |
| astro | eager | 74.642 | 1.792 | 80.555 | 111.1 | 331.3 | 87.8 |
| astro | prepared | 18.058 | 1.767 | 23.176 | 1041.2 | 2543.9 | 94.1 |
| astro | before | 21.167 | 2.680 | 25.293 | 1636.1 | 3737.9 | 99.1 |
| astro | portable | 20.750 | 1.835 | 24.269 | 1636.1 | 3737.9 | 99.6 |
| astro | control | 74.357 | 1.795 | 80.178 | 111.1 | 331.3 | 86.9 |
| tsx | eager | 47.907 | 7.397 | 60.462 | 39.9 | 146.1 | 74.8 |
| tsx | prepared | 10.501 | 7.395 | 22.718 | 572.5 | 1582.8 | 75.8 |
| tsx | before | 12.059 | 17.440 | 32.902 | 952.7 | 2394.4 | 81.6 |
| tsx | portable | 12.214 | 7.448 | 23.535 | 952.7 | 2394.4 | 82.9 |
| tsx | control | 47.995 | 7.563 | 61.429 | 39.9 | 146.1 | 74.7 |
| c | eager | 13.524 | 0.982 | 17.625 | 17.4 | 25.1 | 24.9 |
| c | prepared | 3.460 | 0.966 | 7.316 | 201.7 | 543.9 | 26.1 |
| c | before | 4.648 | 2.090 | 8.099 | 267.7 | 855.0 | 26.4 |
| c | portable | 4.644 | 1.067 | 7.011 | 267.7 | 855.0 | 26.4 |
| c | control | 13.600 | 0.978 | 17.549 | 17.4 | 25.1 | 25.5 |
| php | eager | 49.386 | 0.765 | 52.914 | 169.1 | 331.8 | 60.4 |
| php | prepared | 15.754 | 0.773 | 19.036 | 958.7 | 1823.9 | 58.5 |
| php | before | 17.490 | 1.396 | 20.259 | 1263.8 | 2535.4 | 59.0 |
| php | portable | 17.705 | 0.808 | 19.601 | 1263.8 | 2535.4 | 59.4 |
| php | control | 49.539 | 0.804 | 53.038 | 169.1 | 331.8 | 59.9 |

## Optimized portable versus each reference

Negative changes mean less time. Brackets show the range of three paired process changes, not a confidence interval. A repeatable regression here means at least 3% in each repetition. Build and code-layout effects remain possible; the control helps expose them but cannot remove them from the frozen historical builds.

| Corpus | Reference | Load change | Warm search change | First document change |
| --- | --- | ---: | ---: | ---: |
| cpp | eager | -73.0% [-73.6, -72.6] | +2.7% [-4.0, +3.5] | -69.2% [-70.3, -68.2] |
| cpp | prepared | +17.1% [+16.9, +20.7] | +2.8% [-4.1, +4.2] | -1.2% [-20.3, +1.4] |
| cpp | before | +0.1% [-0.8, +1.3] | -17.6% [-17.8, -17.5] | +0.1% [-4.0, +1.4] |
| cpp | control | -73.0% [-73.0, -72.9] | -2.5% [-6.3, +3.2] | -69.5% [-71.0, -69.1] |
| scss | eager | -78.9% [-80.1, -78.4] | +1.5% [-5.1, +11.8] | -76.2% [-76.3, -75.4] |
| scss | prepared | -12.8% [-13.2, -7.7] | -7.6% [-7.7, +5.1] | -22.8% [-24.7, -21.0] |
| scss | before | +1.8% [+0.3, +3.4] | -36.1% [-37.5, -34.8] | -7.3% [-10.0, -7.2] |
| scss | control | -79.9% [-80.2, -79.4] | -5.1% [-10.0, +2.0] | -77.2% [-77.2, -77.0] |
| java | eager | -65.9% [-66.9, -65.8] | +5.6% [+4.9, +9.0] | -55.0% [-56.1, -52.7] |
| java | prepared | +21.6% [+15.4, +24.8] | +7.7% [+5.6, +12.9] | -13.8% [-16.5, -11.8] |
| java | before | +1.1% [-0.3, +5.0] | -47.9% [-48.9, -47.5] | -21.4% [-21.4, -19.6] |
| java | control | -66.1% [-66.1, -65.9] | +1.7% [-0.7, +3.3] | -55.1% [-56.6, -54.6] |
| json | eager | -27.1% [-28.8, -25.4] | +5.3% [+2.4, +7.5] | -7.0% [-9.9, -4.7] |
| json | prepared | +14.2% [+12.9, +23.7] | +4.9% [+3.2, +6.9] | +1.0% [-2.6, +4.6] |
| json | before | +0.7% [-2.3, +9.9] | -48.8% [-50.1, -47.9] | -44.5% [-45.2, -44.2] |
| json | control | -28.4% [-29.5, -22.5] | -2.0% [-3.4, +0.1] | -10.8% [-11.8, -9.9] |
| rust | eager | -63.4% [-63.4, -63.3] | +4.6% [+3.7, +5.2] | -48.2% [-48.8, -47.7] |
| rust | prepared | +20.6% [+15.0, +21.8] | +5.6% [+3.8, +6.8] | -12.6% [-13.9, -11.7] |
| rust | before | -0.2% [-1.0, +0.9] | -46.0% [-46.1, -45.7] | -22.4% [-25.0, -21.9] |
| rust | control | -63.3% [-63.5, -62.8] | +2.5% [+2.0, +2.7] | -48.1% [-49.0, -47.1] |
| html | eager | -66.8% [-68.1, -66.7] | +1.2% [-0.1, +3.2] | -58.4% [-61.9, -58.0] |
| html | prepared | +8.3% [-20.9, +9.9] | +1.3% [-35.4, +3.4] | +0.2% [-44.6, +1.0] |
| html | before | -0.6% [-4.3, +1.4] | -51.8% [-51.9, -49.5] | -14.4% [-20.2, -14.2] |
| html | control | -66.9% [-68.5, -66.5] | -2.9% [-3.6, -0.3] | -58.8% [-60.8, -58.4] |
| astro | eager | -72.1% [-72.6, -71.9] | +2.8% [+2.4, +4.4] | -69.3% [-69.9, -68.3] |
| astro | prepared | +14.9% [+13.4, +22.0] | +5.5% [+3.8, +10.5] | +4.7% [+3.9, +9.1] |
| astro | before | -0.7% [-3.5, +0.6] | -30.9% [-32.7, -29.7] | -4.0% [-5.0, +0.4] |
| astro | control | -72.1% [-73.2, -71.3] | +1.9% [+0.5, +2.2] | -69.7% [-69.9, -69.2] |
| tsx | eager | -74.0% [-74.7, -73.9] | +3.9% [-3.6, +4.6] | -60.8% [-63.9, -60.3] |
| tsx | prepared | +18.9% [+16.2, +19.2] | +2.2% [+0.3, +3.2] | +3.3% [+0.4, +3.6] |
| tsx | before | +1.3% [+0.9, +1.9] | -56.7% [-57.3, -55.8] | -29.1% [-29.4, -28.5] |
| tsx | control | -74.0% [-74.6, -73.2] | -0.8% [-7.8, +2.3] | -61.4% [-63.8, -60.4] |
| c | eager | -66.2% [-67.2, -65.5] | +7.8% [+5.4, +9.2] | -60.2% [-61.3, -59.5] |
| c | prepared | +32.8% [+29.9, +34.2] | +7.1% [+6.0, +10.5] | -4.2% [-4.7, -3.4] |
| c | before | -0.6% [-0.8, +1.1] | -49.4% [-50.4, -48.9] | -13.5% [-13.6, -10.6] |
| c | control | -66.0% [-67.1, -65.9] | +7.1% [+4.1, +9.1] | -60.0% [-60.1, -59.5] |
| php | eager | -64.1% [-64.4, -63.8] | +5.5% [+3.6, +5.6] | -62.8% [-64.3, -62.7] |
| php | prepared | +12.3% [+12.1, +12.7] | +4.6% [+3.8, +5.1] | +3.6% [+1.2, +4.9] |
| php | before | +1.4% [+0.8, +1.6] | -42.0% [-42.1, -41.3] | -3.3% [-4.2, -2.7] |
| php | control | -64.3% [-64.7, -63.8] | +0.3% [-0.6, +1.3] | -62.8% [-63.7, -62.8] |

## Memory counters

These are partial internal estimates after warm searches, not all heap allocations. Flat rows include power-of-two padding; retained input images and allocator high-water marks are represented by process RSS above.

| Corpus | Variant | Immutable MiB | Scanner scratch MiB | Candidate-walk cache clears, three processes | Retired groups |
| --- | --- | ---: | ---: | --- | --- |
| cpp | eager | 23.87 | 34.27 | [0, 0, 0] | [0, 0, 0] |
| cpp | prepared | 23.87 | 34.27 | [0, 0, 0] | [0, 0, 0] |
| cpp | before | 26.88 | 9.19 | [0, 0, 0] | [0, 0, 0] |
| cpp | portable | 26.88 | 9.30 | [0, 0, 0] | [0, 0, 0] |
| cpp | control | 23.87 | 34.27 | [0, 0, 0] | [0, 0, 0] |
| scss | eager | 14.20 | 12.17 | [0, 0, 0] | [0, 0, 0] |
| scss | prepared | 14.20 | 12.17 | [0, 0, 0] | [0, 0, 0] |
| scss | before | 15.76 | 4.00 | [0, 0, 0] | [0, 0, 0] |
| scss | portable | 15.76 | 4.51 | [0, 0, 0] | [0, 0, 0] |
| scss | control | 14.20 | 12.17 | [0, 0, 0] | [0, 0, 0] |
| java | eager | 0.89 | 1.46 | [0, 0, 0] | [0, 0, 0] |
| java | prepared | 0.89 | 1.46 | [0, 0, 0] | [0, 0, 0] |
| java | before | 0.94 | 0.64 | [0, 0, 0] | [0, 0, 0] |
| java | portable | 0.94 | 0.73 | [0, 0, 0] | [0, 0, 0] |
| java | control | 0.89 | 1.46 | [0, 0, 0] | [0, 0, 0] |
| json | eager | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| json | prepared | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| json | before | 0.02 | 0.05 | [0, 0, 0] | [0, 0, 0] |
| json | portable | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| json | control | 0.02 | 0.06 | [0, 0, 0] | [0, 0, 0] |
| rust | eager | 0.29 | 0.51 | [0, 0, 0] | [0, 0, 0] |
| rust | prepared | 0.29 | 0.51 | [0, 0, 0] | [0, 0, 0] |
| rust | before | 0.28 | 0.27 | [0, 0, 0] | [0, 0, 0] |
| rust | portable | 0.28 | 0.33 | [0, 0, 0] | [0, 0, 0] |
| rust | control | 0.29 | 0.51 | [0, 0, 0] | [0, 0, 0] |
| html | eager | 8.64 | 11.54 | [0, 0, 0] | [0, 0, 0] |
| html | prepared | 8.64 | 11.54 | [0, 0, 0] | [0, 0, 0] |
| html | before | 9.44 | 4.21 | [0, 0, 0] | [0, 0, 0] |
| html | portable | 9.44 | 4.49 | [0, 0, 0] | [0, 0, 0] |
| html | control | 8.64 | 11.54 | [0, 0, 0] | [0, 0, 0] |
| astro | eager | 14.82 | 17.09 | [0, 0, 0] | [0, 0, 0] |
| astro | prepared | 14.82 | 17.09 | [0, 0, 0] | [0, 0, 0] |
| astro | before | 16.11 | 4.87 | [0, 0, 0] | [0, 0, 0] |
| astro | portable | 16.11 | 5.03 | [0, 0, 0] | [0, 0, 0] |
| astro | control | 14.82 | 17.09 | [0, 0, 0] | [0, 0, 0] |
| tsx | eager | 9.54 | 10.33 | [0, 0, 0] | [0, 0, 0] |
| tsx | prepared | 9.54 | 10.33 | [0, 0, 0] | [0, 0, 0] |
| tsx | before | 10.14 | 3.83 | [0, 0, 0] | [0, 0, 0] |
| tsx | portable | 10.14 | 4.43 | [0, 0, 0] | [0, 0, 0] |
| tsx | control | 9.54 | 10.33 | [0, 0, 0] | [0, 0, 0] |
| c | eager | 3.86 | 7.68 | [0, 0, 0] | [0, 0, 0] |
| c | prepared | 3.86 | 7.68 | [0, 0, 0] | [0, 0, 0] |
| c | before | 4.23 | 2.29 | [0, 0, 0] | [0, 0, 0] |
| c | portable | 4.23 | 2.35 | [0, 0, 0] | [0, 0, 0] |
| c | control | 3.86 | 7.68 | [0, 0, 0] | [0, 0, 0] |
| php | eager | 10.22 | 10.35 | [0, 0, 0] | [0, 0, 0] |
| php | prepared | 10.22 | 10.35 | [0, 0, 0] | [0, 0, 0] |
| php | before | 11.34 | 2.98 | [0, 0, 0] | [0, 0, 0] |
| php | portable | 11.34 | 3.30 | [0, 0, 0] | [0, 0, 0] |
| php | control | 10.22 | 10.35 | [0, 0, 0] | [0, 0, 0] |
