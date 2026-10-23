# Scanner-group archive measurements

Negative timing changes mean less time. Ratios pair the same process repetition. Each central value is the median of three Criterion mean estimates; ranges retain all three paired ratios.

## CONTROL versus current Eager

| Corpus | Load change | Search change | Cold document change | gzip KiB, Eager → candidate | Peak RSS MiB, Eager → candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| cpp | -0.6% [-1.4, -0.5] | +0.0% [-1.9, +0.5] | -0.6% [-2.5, +0.7] | 189.3 → 189.3 | 99.7 → 99.1 |
| scss | +0.1% [-2.7, +1.7] | -1.0% [-2.3, +1.5] | -1.2% [-2.4, +0.6] | 82.7 → 82.7 | 47.7 → 47.4 |
| java | -1.6% [-27.4, -0.5] | +1.5% [+0.1, +2.2] | -0.6% [-1.2, -0.4] | 17.3 → 17.3 | 12.6 → 12.6 |
| json | -0.6% [-2.6, -0.2] | -1.2% [-1.5, +1.3] | +0.6% [-0.1, +0.8] | 1.7 → 1.7 | 6.7 → 6.8 |
| rust | -1.9% [-1.9, +1.3] | +0.4% [-1.3, +1.5] | +0.6% [-1.7, +2.3] | 8.3 → 8.3 | 9.6 → 9.4 |
| html | -2.8% [-8.0, +0.1] | -1.2% [-2.6, +0.3] | +0.2% [-1.0, +37.8] | 304.3 → 304.3 | 78.7 → 78.0 |
| astro | +0.1% [-0.5, +6.8] | -0.1% [-1.1, +4.5] | -0.2% [-0.8, +0.8] | 331.3 → 331.3 | 88.0 → 88.2 |
| tsx | -0.3% [-0.4, +1.8] | +0.4% [-4.8, +0.6] | -0.7% [-1.1, +1.0] | 146.1 → 146.1 | 75.6 → 74.7 |
| c | +4.8% [+3.5, +7.1] | +2.0% [-5.7, +2.6] | -0.3% [-2.0, +0.5] | 25.1 → 25.1 | 25.9 → 25.2 |
| php | +3.6% [+0.4, +8.1] | -0.1% [-0.9, +3.7] | -0.9% [-15.3, +1.6] | 331.8 → 331.8 | 61.6 → 60.9 |

## PREPARED versus current Eager

| Corpus | Load change | Search change | Cold document change | gzip KiB, Eager → candidate | Peak RSS MiB, Eager → candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| cpp | -77.1% [-77.3, -76.9] | -2.7% [-4.8, +0.4] | -69.3% [-69.8, -69.1] | 189.3 → 3689.3 | 99.7 → 111.8 |
| scss | -77.3% [-77.5, -77.2] | -2.0% [-6.5, +1.7] | -69.6% [-69.7, -69.2] | 82.7 → 2127.9 | 47.7 → 51.2 |
| java | -71.2% [-78.7, -71.1] | +1.1% [+0.4, +2.1] | -44.9% [-45.9, -44.0] | 17.3 → 151.5 | 12.6 → 12.8 |
| json | -38.2% [-38.3, -38.0] | +1.4% [-0.1, +1.9] | -8.9% [-9.4, -8.1] | 1.7 → 4.5 | 6.7 → 6.7 |
| rust | -70.3% [-70.5, -69.1] | +0.1% [-1.4, +1.5] | -41.0% [-42.2, -40.8] | 8.3 → 50.2 | 9.6 → 9.6 |
| html | -70.3% [-72.0, -69.5] | +0.8% [+0.6, +3.6] | -57.8% [-58.5, -56.6] | 304.3 → 1590.9 | 78.7 → 77.6 |
| astro | -75.8% [-75.9, -75.3] | +0.9% [-0.7, +1.9] | -70.7% [-71.8, -69.8] | 331.3 → 2543.9 | 88.0 → 93.5 |
| tsx | -78.3% [-78.4, -78.3] | -2.0% [-7.1, -0.4] | -62.8% [-63.1, -62.6] | 146.1 → 1582.8 | 75.6 → 76.0 |
| c | -74.3% [-74.4, -74.2] | -0.0% [-0.4, +1.0] | -57.9% [-59.9, -57.2] | 25.1 → 543.9 | 25.9 → 26.4 |
| php | -68.5% [-68.6, -67.8] | +0.8% [-0.8, +9.0] | -64.3% [-68.5, -62.3] | 331.8 → 1823.9 | 61.6 → 58.8 |

## Absolute times

Milliseconds per complete captured corpus; input I/O, gzip decompression, string setup, and teardown are outside these Rust measurements.

| Corpus | Variant | Load ms | Warm search ms | Cold document ms | gzip decode ms (Python) |
| --- | --- | ---: | ---: | ---: | ---: |
| cpp | eager | 91.727 | 2.933 | 104.138 | 1.476 |
| cpp | control | 90.571 | 2.878 | 103.524 | 1.525 |
| cpp | prepared | 20.833 | 2.854 | 32.078 | 19.778 |
| scss | eager | 49.945 | 1.231 | 56.040 | 0.702 |
| scss | control | 49.988 | 1.248 | 55.380 | 0.781 |
| scss | prepared | 11.300 | 1.207 | 17.196 | 10.441 |
| java | eager | 5.387 | 1.028 | 8.508 | 0.181 |
| java | control | 5.337 | 1.034 | 8.430 | 0.177 |
| java | prepared | 1.556 | 1.037 | 4.706 | 0.903 |
| json | eager | 0.175 | 0.508 | 0.747 | 0.022 |
| json | control | 0.174 | 0.514 | 0.752 | 0.021 |
| json | prepared | 0.108 | 0.518 | 0.677 | 0.037 |
| rust | eager | 2.127 | 0.688 | 3.823 | 0.054 |
| rust | control | 2.147 | 0.698 | 3.845 | 0.053 |
| rust | prepared | 0.651 | 0.698 | 2.211 | 0.299 |
| html | eager | 44.677 | 4.244 | 53.481 | 3.607 |
| html | control | 44.070 | 4.134 | 53.709 | 3.485 |
| html | prepared | 13.421 | 4.293 | 22.575 | 10.228 |
| astro | eager | 74.573 | 1.787 | 81.411 | 4.118 |
| astro | control | 74.690 | 1.785 | 80.918 | 4.326 |
| astro | prepared | 18.181 | 1.790 | 23.360 | 15.851 |
| tsx | eager | 48.160 | 7.433 | 61.318 | 2.536 |
| tsx | control | 48.287 | 7.479 | 60.659 | 2.507 |
| tsx | prepared | 10.439 | 7.325 | 22.835 | 9.984 |
| c | eager | 13.836 | 0.975 | 17.971 | 0.175 |
| c | control | 14.568 | 0.995 | 17.913 | 0.172 |
| c | prepared | 3.540 | 0.985 | 7.484 | 2.876 |
| php | eager | 50.521 | 0.792 | 54.310 | 3.284 |
| php | control | 51.411 | 0.796 | 53.841 | 3.444 |
| php | prepared | 15.985 | 0.803 | 19.559 | 10.870 |

## Feature control

Ordinary Eager with the prototype feature compiled and no prepared archive. This isolates feature/layout/code-generation effects; the context records the repetition count.

| Corpus | Load change | Search change | Cold document change |
| --- | ---: | ---: | ---: |
| cpp | -0.6% | +0.0% | -0.6% |
| scss | +0.1% | -1.0% | -1.2% |
| java | -1.6% | +1.5% | -0.6% |
| json | -0.6% | -1.2% | +0.6% |
| rust | -1.9% | +0.4% | +0.6% |
| html | -2.8% | -1.2% | +0.2% |
| astro | +0.1% | -0.1% | -0.2% |
| tsx | -0.3% | +0.4% | -0.7% |
| c | +4.8% | +2.0% | -0.3% |
| php | +3.6% | -0.1% | -0.9% |
