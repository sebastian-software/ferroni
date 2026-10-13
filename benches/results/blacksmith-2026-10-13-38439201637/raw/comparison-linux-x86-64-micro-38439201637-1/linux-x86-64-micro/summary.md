### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 43.4 ns | 1.87 | 0.71 | 0.46 | 1.16 | 0.36 | 0.36 | 0.24 |
| `single_pattern__quantifier_greedy` | 44.9 ns | 2.59 | 1.30 | 0.55 | 1.06 | 1.26 | 1.34 | 1.20 |
| `single_pattern__alternation_2_branch` | 43.4 ns | 2.02 | 0.71 | 0.41 | 1.29 | 1.03 | 1.30 | 0.85 |
| `single_pattern__alternation_10_branch` | 37.8 ns | 4.48 | 2.70 | 0.59 | 3.72 | 0.61 | 0.62 | 0.46 |
| `single_pattern__case_insensitive_phrase` | 72.2 ns | 1.84 | 0.81 | 0.25 | 0.67 | 0.91 | 0.98 | 0.76 |
| `single_pattern__named_capture_date` | 69.9 ns | 3.11 | 1.63 | 0.60 | 4.68 | 0.75 | 0.66 | 0.61 |
| `single_pattern__unicode_greek` | 73.9 ns | 2.38 | 3.15 | 0.47 | 2.77 | 1.11 | 0.98 | 0.87 |
| `single_pattern__lookaround_combined` | 97.3 ns | 3.20 | 0.94 | 0.42 | 2.99 | 2.82 | 1.39 | – |
| `single_pattern__backref_simple` | 72.6 ns | 1.74 | 1.27 | 0.41 | 1.03 | 1.81 | 2.51 | – |
| `text_scanning__regset_position_lead` | 117 ns | 2.67 | – | – | – | – | – | – |
| `compilation__literal` | 1.05 µs | 0.53 | 0.38 | 6.27 | 0.45 | 3.07 | 3.36 | 3.03 |
| `compilation__named_capture` | 3.69 µs | 1.14 | 1.24 | 1.61 | 0.16 | 47.97 | 46.80 | 48.87 |
| `compilation__lookbehind` | 1.65 µs | 0.35 | 0.23 | 2.31 | 0.14 | 101.25 | 215.73 | – |

