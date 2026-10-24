# Portable load phases

Diagnostic feature builds: three rotating processes, 20 fresh loads per process. Cells are medians of per-process means in milliseconds. Nested time is subtracted from phases; total is inclusive. Instrumentation overhead is included here and excluded from the final Criterion matrix. Per-phase medians need not add up to the median total.

| Corpus | Total, before → new | Group checksum | Program binding checksum | Instruction decode | Instruction validation |
| --- | ---: | ---: | ---: | ---: | ---: |
| cpp | 25.365 → 19.814 | 5.767 → 1.714 | 0.409 → 0.121 | 3.099 → 1.900 | 3.642 → 3.671 |
| scss | 10.066 → 7.361 | 3.046 → 0.916 | 0.183 → 0.055 | 1.596 → 1.081 | 2.080 → 2.141 |
| java | 1.951 → 1.723 | 0.204 → 0.065 | 0.070 → 0.021 | 0.130 → 0.089 | 0.204 → 0.211 |
| json | 0.139 → 0.128 | 0.005 → 0.001 | 0.002 → 0.001 | 0.006 → 0.005 | 0.017 → 0.018 |
| rust | 0.834 → 0.768 | 0.063 → 0.019 | 0.012 → 0.004 | 0.038 → 0.023 | 0.066 → 0.067 |
| html | 14.803 → 12.170 | 2.038 → 0.613 | 1.111 → 0.330 | 1.151 → 0.777 | 1.506 → 1.538 |
| astro | 21.261 → 16.780 | 3.536 → 1.065 | 1.449 → 0.438 | 2.015 → 1.319 | 2.473 → 2.520 |
| tsx | 13.061 → 10.016 | 2.329 → 0.701 | 1.052 → 0.315 | 1.341 → 0.856 | 1.566 → 1.590 |
| c | 4.928 → 4.008 | 0.959 → 0.288 | 0.034 → 0.010 | 0.540 → 0.388 | 0.708 → 0.725 |
| php | 18.427 → 15.599 | 2.417 → 0.732 | 0.927 → 0.279 | 1.375 → 0.851 | 1.843 → 1.871 |

## Remaining exclusive costs

| Corpus | Phase | Time ms | Calls per load |
| --- | --- | ---: | ---: |
| cpp | portable_validate | 3.671 | 86 |
| cpp | scanner_assembly | 3.242 | 96 |
| cpp | program_decode | 2.683 | 250 |
| cpp | group_lookup | 1.993 | 44 |
| cpp | portable_decode | 1.900 | 86 |
| cpp | group_checksum | 1.714 | 43 |
| scss | portable_validate | 2.141 | 38 |
| scss | program_decode | 1.275 | 104 |
| scss | portable_decode | 1.081 | 38 |
| scss | group_checksum | 0.916 | 19 |
| scss | scanner_assembly | 0.451 | 20 |
| scss | group_lookup | 0.362 | 19 |
| java | scanner_assembly | 0.297 | 27 |
| java | portable_validate | 0.211 | 46 |
| java | group_lookup | 0.200 | 23 |
| java | eager_helpers | 0.190 | 866 |
| java | program_decode | 0.111 | 115 |
| java | portable_decode | 0.089 | 46 |
| json | scanner_assembly | 0.026 | 6 |
| json | portable_validate | 0.018 | 10 |
| json | scanner_scratch | 0.014 | 6 |
| json | group_lookup | 0.010 | 5 |
| json | program_lookup | 0.009 | 18 |
| json | program_decode | 0.009 | 18 |
| rust | eager_helpers | 0.130 | 393 |
| rust | group_lookup | 0.104 | 8 |
| rust | scanner_assembly | 0.102 | 8 |
| rust | program_decode | 0.069 | 88 |
| rust | portable_validate | 0.067 | 16 |
| rust | program_helpers | 0.059 | 88 |
| html | program_decode | 2.665 | 370 |
| html | portable_validate | 1.538 | 120 |
| html | program_checksum | 1.097 | 370 |
| html | scanner_assembly | 1.080 | 74 |
| html | portable_decode | 0.777 | 120 |
| html | program_validate | 0.702 | 370 |
| astro | program_decode | 2.577 | 364 |
| astro | portable_validate | 2.520 | 148 |
| astro | scanner_assembly | 2.114 | 153 |
| astro | program_checksum | 1.431 | 364 |
| astro | portable_decode | 1.319 | 148 |
| astro | group_lookup | 1.136 | 76 |
| tsx | portable_validate | 1.590 | 96 |
| tsx | scanner_assembly | 1.134 | 48 |
| tsx | program_checksum | 1.043 | 250 |
| tsx | program_decode | 0.922 | 250 |
| tsx | group_lookup | 0.880 | 48 |
| tsx | portable_decode | 0.856 | 96 |
| c | portable_validate | 0.725 | 38 |
| c | scanner_assembly | 0.700 | 29 |
| c | eager_helpers | 0.526 | 2031 |
| c | group_lookup | 0.416 | 19 |
| c | portable_decode | 0.388 | 38 |
| c | program_decode | 0.298 | 105 |
| php | program_decode | 5.041 | 315 |
| php | portable_validate | 1.871 | 64 |
| php | program_validate | 1.344 | 315 |
| php | scanner_assembly | 1.251 | 35 |
| php | program_checksum | 0.934 | 315 |
| php | portable_decode | 0.851 | 64 |
