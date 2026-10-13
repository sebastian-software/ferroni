### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 35.9 ns | 3.07 | 1.11 | 0.42 | 1.08 | 0.28 | 0.29 | 0.21 |
| `single_pattern__quantifier_greedy` | 41.1 ns | 3.52 | 1.58 | 0.53 | 1.12 | 1.24 | 1.18 | 1.02 |
| `single_pattern__alternation_2_branch` | 39.8 ns | 2.72 | 1.00 | 0.39 | 1.37 | 0.86 | 0.82 | 0.78 |
| `single_pattern__alternation_10_branch` | 25 ns | 5.83 | 15.17 | 0.75 | 7.65 | 0.66 | 0.70 | 0.50 |
| `single_pattern__case_insensitive_phrase` | 55.7 ns | 2.41 | 0.89 | 0.32 | 0.97 | 0.72 | 0.67 | 0.68 |
| `single_pattern__named_capture_date` | 61.6 ns | 3.33 | 1.77 | 0.74 | 4.58 | 0.52 | 0.51 | 0.45 |
| `single_pattern__unicode_greek` | 59 ns | 2.83 | 3.25 | 0.55 | 2.31 | 0.72 | 0.73 | 0.63 |
| `single_pattern__lookaround_combined` | 58.3 ns | 3.78 | 1.12 | 0.47 | 4.72 | 4.03 | 1.61 | – |
| `single_pattern__backref_simple` | 61 ns | 1.98 | 1.04 | 0.30 | 0.87 | 1.46 | 1.83 | – |
| `text_scanning__regset_position_lead` | 82.6 ns | 3.68 | – | – | – | – | – | – |
| `compilation__literal` | 656 ns | 0.61 | 0.50 | 3.12 | 0.53 | 4.42 | 4.03 | 3.10 |
| `compilation__named_capture` | 2.99 µs | 1.54 | 1.73 | 1.06 | 0.18 | 50.85 | 53.83 | 50.43 |
| `compilation__lookbehind` | 1.48 µs | 0.29 | 0.24 | 1.50 | 0.17 | 98.98 | 198.58 | – |

