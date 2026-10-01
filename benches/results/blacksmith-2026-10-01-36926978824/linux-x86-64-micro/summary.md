### Engine comparison: linux-x86-64 (AMD EPYC)

Source `6db6413e84b0`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 77.9 ns | 1.84 | 0.82 | 0.41 | 1.16 | 0.30 | 0.23 | – |
| `single_pattern__quantifier_greedy` | 67.7 ns | 2.37 | 1.75 | 0.59 | 1.20 | 1.35 | 1.42 | – |
| `single_pattern__alternation_2_branch` | 72.9 ns | 1.98 | 0.63 | 0.42 | 1.28 | 0.96 | 0.97 | – |
| `single_pattern__alternation_10_branch` | 48.1 ns | 4.69 | 2.91 | 0.78 | 4.50 | 0.72 | 0.54 | – |
| `single_pattern__case_insensitive_phrase` | 127 ns | 1.68 | 0.79 | 0.24 | 0.64 | 0.71 | 0.63 | – |
| `single_pattern__named_capture_date` | 128 ns | 2.82 | 1.38 | 0.55 | 3.66 | 0.50 | 0.49 | – |
| `single_pattern__unicode_greek` | 91.5 ns | 2.67 | 2.72 | 0.51 | 3.23 | 1.02 | 1.01 | – |
| `single_pattern__lookaround_combined` | 98.6 ns | 2.79 | 0.97 | 0.40 | 3.21 | 2.95 | – | – |
| `single_pattern__backref_simple` | 80.5 ns | 1.62 | 1.09 | 0.37 | 0.95 | 1.48 | – | – |
| `text_scanning__regset_position_lead` | 101 ns | 2.74 | – | – | – | – | – | – |
| `compilation__literal` | 830 ns | 0.60 | 0.52 | 6.22 | 0.43 | 4.21 | 3.06 | – |
| `compilation__named_capture` | 3.44 µs | 1.39 | 1.62 | 2.05 | 0.22 | 63.01 | 64.22 | – |
| `compilation__lookbehind` | 1.76 µs | 0.39 | 0.28 | 2.67 | 0.18 | 123.81 | – | – |

