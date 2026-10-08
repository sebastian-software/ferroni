### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `62892008bbf5`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 47.9 ns | 2.31 | 0.70 | 0.34 | 0.82 | 0.22 | 0.17 | – |
| `single_pattern__quantifier_greedy` | 41.4 ns | 3.45 | 1.68 | 0.50 | 1.05 | 1.25 | 1.01 | – |
| `single_pattern__alternation_2_branch` | 45.2 ns | 2.54 | 0.81 | 0.37 | 1.24 | 0.82 | 0.70 | – |
| `single_pattern__alternation_10_branch` | 27.9 ns | 5.64 | 4.49 | 0.72 | 7.20 | 0.67 | 0.45 | – |
| `single_pattern__case_insensitive_phrase` | 62.4 ns | 2.13 | 0.86 | 0.30 | 0.80 | 0.66 | 0.65 | – |
| `single_pattern__named_capture_date` | 65.7 ns | 3.43 | 1.83 | 0.72 | 4.69 | 0.55 | 0.48 | – |
| `single_pattern__unicode_greek` | 65.4 ns | 2.70 | 2.52 | 0.54 | 2.19 | 0.64 | 0.61 | – |
| `single_pattern__lookaround_combined` | 59.2 ns | 3.55 | 1.01 | 0.45 | 4.43 | 3.71 | – | – |
| `single_pattern__backref_simple` | 61.7 ns | 2.04 | 1.02 | 0.33 | 0.86 | 1.38 | – | – |
| `text_scanning__regset_position_lead` | 77.9 ns | 3.81 | – | – | – | – | – | – |
| `compilation__literal` | 608 ns | 0.63 | 0.59 | 3.49 | 0.54 | 4.38 | 3.29 | – |
| `compilation__named_capture` | 3.04 µs | 1.58 | 1.71 | 1.15 | 0.19 | 55.54 | 53.91 | – |
| `compilation__lookbehind` | 1.3 µs | 0.33 | 0.27 | 1.66 | 0.19 | 113.05 | – | – |

