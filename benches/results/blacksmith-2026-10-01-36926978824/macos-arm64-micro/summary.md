### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `6db6413e84b0`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 35.7 ns | 2.83 | 1.06 | 0.43 | 1.12 | 0.31 | 0.21 | – |
| `single_pattern__quantifier_greedy` | 44.3 ns | 3.43 | 1.50 | 0.47 | 0.94 | 1.01 | 0.95 | – |
| `single_pattern__alternation_2_branch` | 40.7 ns | 2.63 | 0.83 | 0.38 | 1.27 | 0.78 | 0.76 | – |
| `single_pattern__alternation_10_branch` | 25.2 ns | 6.47 | 4.61 | 0.81 | 7.89 | 0.64 | 0.51 | – |
| `single_pattern__case_insensitive_phrase` | 57.7 ns | 2.35 | 0.89 | 0.28 | 0.82 | 0.66 | 0.61 | – |
| `single_pattern__named_capture_date` | 57.7 ns | 3.73 | 2.08 | 0.82 | 4.73 | 0.58 | 0.47 | – |
| `single_pattern__unicode_greek` | 61.7 ns | 2.71 | 2.74 | 0.52 | 2.35 | 0.72 | 0.57 | – |
| `single_pattern__lookaround_combined` | 56.7 ns | 3.60 | 1.08 | 0.53 | 4.99 | 3.93 | – | – |
| `single_pattern__backref_simple` | 56.6 ns | 2.35 | 1.09 | 0.33 | 0.89 | 1.40 | – | – |
| `text_scanning__regset_position_lead` | 72.1 ns | 3.82 | – | – | – | – | – | – |
| `compilation__literal` | 535 ns | 0.62 | 0.59 | 3.55 | 0.59 | 4.65 | 3.26 | – |
| `compilation__named_capture` | 2.68 µs | 1.58 | 1.86 | 1.19 | 0.19 | 55.92 | 55.41 | – |
| `compilation__lookbehind` | 1.13 µs | 0.38 | 0.30 | 1.73 | 0.21 | 121.96 | – | – |

