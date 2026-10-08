### Engine comparison: linux-x86-64 (AMD EPYC)

Source `62892008bbf5`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 32.6 ns | 2.22 | 0.87 | 0.50 | 1.23 | 0.37 | 0.28 | – |
| `single_pattern__quantifier_greedy` | 36.5 ns | 2.43 | 1.47 | 0.54 | 1.04 | 1.33 | 1.19 | – |
| `single_pattern__alternation_2_branch` | 39.8 ns | 1.60 | 0.67 | 0.39 | 1.13 | 0.96 | 0.78 | – |
| `single_pattern__alternation_10_branch` | 29.8 ns | 4.35 | 2.89 | 0.66 | 4.03 | 0.65 | 0.51 | – |
| `single_pattern__case_insensitive_phrase` | 58 ns | 1.70 | 0.93 | 0.27 | 0.69 | 0.84 | 0.77 | – |
| `single_pattern__named_capture_date` | 58.2 ns | 3.76 | 1.61 | 0.62 | 4.61 | 0.69 | 0.65 | – |
| `single_pattern__unicode_greek` | 55.9 ns | 2.30 | 2.59 | 0.47 | 2.84 | 0.89 | 0.82 | – |
| `single_pattern__lookaround_combined` | 54.2 ns | 3.14 | 1.01 | 0.47 | 3.76 | 3.53 | – | – |
| `single_pattern__backref_simple` | 57.4 ns | 1.35 | 0.94 | 0.34 | 0.85 | 1.30 | – | – |
| `text_scanning__regset_position_lead` | 63.1 ns | 2.92 | – | – | – | – | – | – |
| `compilation__literal` | 561 ns | 0.55 | 0.50 | 6.50 | 0.47 | 4.09 | 3.20 | – |
| `compilation__named_capture` | 2.6 µs | 1.20 | 1.50 | 1.87 | 0.18 | 59.94 | 55.85 | – |
| `compilation__lookbehind` | 1.31 µs | 0.36 | 0.28 | 2.47 | 0.16 | 113.02 | – | – |

