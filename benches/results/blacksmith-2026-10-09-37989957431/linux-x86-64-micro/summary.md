### Engine comparison: linux-x86-64 (AMD EPYC)

Source `a38768bc8012`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 37 ns | 2.25 | 0.75 | 0.46 | 1.08 | 0.36 | 0.35 | 0.28 |
| `single_pattern__quantifier_greedy` | 37.6 ns | 2.50 | 1.38 | 0.56 | 1.02 | 1.32 | 1.37 | 1.19 |
| `single_pattern__alternation_2_branch` | 40.8 ns | 1.74 | 0.75 | 0.40 | 1.17 | 1.18 | 1.12 | 0.84 |
| `single_pattern__alternation_10_branch` | 33.3 ns | 5.11 | 3.38 | 0.72 | 5.31 | 0.69 | 0.66 | 0.53 |
| `single_pattern__case_insensitive_phrase` | 94.4 ns | 1.53 | 0.80 | 0.23 | 0.58 | 0.67 | 0.72 | 0.60 |
| `single_pattern__named_capture_date` | 97.2 ns | 2.89 | 1.28 | 0.48 | 3.52 | 0.52 | 0.56 | 0.51 |
| `single_pattern__unicode_greek` | 112 ns | 1.97 | 2.08 | 0.38 | 1.88 | 0.66 | 0.61 | 0.62 |
| `single_pattern__lookaround_combined` | 86.1 ns | 3.09 | 0.95 | 0.40 | 3.47 | 3.54 | 1.59 | – |
| `single_pattern__backref_simple` | 86.7 ns | 1.58 | 1.10 | 0.37 | 0.85 | 1.53 | 1.96 | – |
| `text_scanning__regset_position_lead` | 103 ns | 2.68 | – | – | – | – | – | – |
| `compilation__literal` | 861 ns | 0.56 | 0.52 | 8.77 | 0.55 | 4.30 | 4.20 | 3.23 |
| `compilation__named_capture` | 3.88 µs | 1.16 | 1.09 | 2.26 | 0.18 | 49.85 | 43.75 | 51.75 |
| `compilation__lookbehind` | 1.96 µs | 0.27 | 0.19 | 1.96 | 0.16 | 95.70 | 215.81 | – |

