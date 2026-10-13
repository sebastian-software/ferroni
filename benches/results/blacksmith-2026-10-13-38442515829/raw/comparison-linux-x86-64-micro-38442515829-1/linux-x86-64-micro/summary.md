### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 44.8 ns | 2.17 | 0.80 | 0.48 | 1.22 | 0.38 | 0.41 | 0.30 |
| `single_pattern__quantifier_greedy` | 46.5 ns | 2.43 | 1.47 | 0.52 | 1.05 | 1.34 | 1.33 | 1.18 |
| `single_pattern__alternation_2_branch` | 49.4 ns | 1.47 | 0.92 | 0.45 | 1.44 | 1.21 | 1.15 | 0.87 |
| `single_pattern__alternation_10_branch` | 45.6 ns | 3.68 | 3.14 | 0.66 | 4.04 | 0.61 | 0.57 | 0.46 |
| `single_pattern__case_insensitive_phrase` | 92.1 ns | 1.60 | 0.85 | 0.25 | 0.61 | 0.76 | 0.86 | 0.73 |
| `single_pattern__named_capture_date` | 106 ns | 2.37 | 1.41 | 0.53 | 3.79 | 0.51 | 0.53 | 0.49 |
| `single_pattern__unicode_greek` | 109 ns | 2.14 | 1.92 | 0.34 | 1.96 | 0.74 | 0.73 | 0.65 |
| `single_pattern__lookaround_combined` | 97.4 ns | 2.29 | 0.65 | 0.31 | 2.79 | 2.29 | 1.13 | – |
| `single_pattern__backref_simple` | 70.1 ns | 1.59 | 0.94 | 0.34 | 0.87 | 1.38 | 1.91 | – |
| `text_scanning__regset_position_lead` | 86 ns | 2.53 | – | – | – | – | – | – |
| `compilation__literal` | 731 ns | 0.52 | 0.46 | 5.94 | 0.41 | 3.95 | 3.68 | 2.89 |
| `compilation__named_capture` | 2.9 µs | 1.31 | 1.51 | 2.03 | 0.20 | 60.47 | 58.08 | 60.62 |
| `compilation__lookbehind` | 1.47 µs | 0.46 | 0.34 | 3.00 | 0.20 | 131.83 | 270.03 | – |

