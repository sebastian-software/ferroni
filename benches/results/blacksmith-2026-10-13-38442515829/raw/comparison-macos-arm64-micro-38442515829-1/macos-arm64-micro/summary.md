### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 33.2 ns | 3.11 | 1.20 | 0.45 | 1.19 | 0.31 | 0.33 | 0.22 |
| `single_pattern__quantifier_greedy` | 43 ns | 3.47 | 1.56 | 0.51 | 0.97 | 1.14 | 1.13 | 1.07 |
| `single_pattern__alternation_2_branch` | 39.6 ns | 2.98 | 1.22 | 0.39 | 1.33 | 0.84 | 0.79 | 0.79 |
| `single_pattern__alternation_10_branch` | 25.2 ns | 6.18 | 14.66 | 0.76 | 7.86 | 0.69 | 0.67 | 0.49 |
| `single_pattern__case_insensitive_phrase` | 57.2 ns | 2.37 | 0.88 | 0.32 | 0.93 | 0.74 | 0.70 | 0.65 |
| `single_pattern__named_capture_date` | 62.5 ns | 3.38 | 1.76 | 0.72 | 4.59 | 0.52 | 0.51 | 0.46 |
| `single_pattern__unicode_greek` | 58.2 ns | 3.15 | 3.49 | 0.55 | 2.39 | 0.72 | 0.71 | 0.66 |
| `single_pattern__lookaround_combined` | 60 ns | 3.48 | 1.06 | 0.44 | 4.41 | 3.56 | 1.43 | – |
| `single_pattern__backref_simple` | 60.6 ns | 2.07 | 1.06 | 0.30 | 0.87 | 1.36 | 1.83 | – |
| `text_scanning__regset_position_lead` | 86.6 ns | 3.75 | – | – | – | – | – | – |
| `compilation__literal` | 636 ns | 0.56 | 0.48 | 3.24 | 0.48 | 3.70 | 3.65 | 2.92 |
| `compilation__named_capture` | 2.62 µs | 1.55 | 1.83 | 1.16 | 0.19 | 54.08 | 54.63 | 53.99 |
| `compilation__lookbehind` | 1.31 µs | 0.29 | 0.25 | 1.44 | 0.17 | 99.21 | 198.65 | – |

