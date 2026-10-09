### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `a38768bc8012`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 46.1 ns | 2.47 | 0.79 | 0.32 | 0.91 | 0.26 | 0.25 | 0.17 |
| `single_pattern__quantifier_greedy` | 46.7 ns | 3.20 | 1.49 | 0.45 | 0.94 | 1.16 | 1.03 | 0.91 |
| `single_pattern__alternation_2_branch` | 45.4 ns | 2.50 | 1.14 | 0.35 | 1.20 | 0.80 | 0.75 | 0.68 |
| `single_pattern__alternation_10_branch` | 25.9 ns | 6.48 | 14.76 | 0.74 | 8.07 | 0.74 | 0.69 | 0.52 |
| `single_pattern__case_insensitive_phrase` | 62 ns | 2.43 | 0.79 | 0.26 | 0.73 | 0.60 | 0.59 | 0.61 |
| `single_pattern__named_capture_date` | 60.4 ns | 3.50 | 1.81 | 0.73 | 4.44 | 0.51 | 0.51 | 0.44 |
| `single_pattern__unicode_greek` | 56.8 ns | 2.95 | 3.27 | 0.51 | 2.26 | 0.63 | 0.66 | 0.60 |
| `single_pattern__lookaround_combined` | 51.4 ns | 3.91 | 1.19 | 0.46 | 4.76 | 3.93 | 1.72 | – |
| `single_pattern__backref_simple` | 58.5 ns | 2.08 | 0.92 | 0.30 | 0.88 | 1.30 | 1.61 | – |
| `text_scanning__regset_position_lead` | 69.8 ns | 3.42 | – | – | – | – | – | – |
| `compilation__literal` | 540 ns | 0.60 | 0.57 | 3.32 | 0.53 | 4.18 | 4.17 | 2.90 |
| `compilation__named_capture` | 2.61 µs | 1.54 | 2.00 | 1.38 | 0.22 | 62.81 | 62.17 | 56.81 |
| `compilation__lookbehind` | 1.42 µs | 0.31 | 0.26 | 1.53 | 0.18 | 109.36 | 212.47 | – |

