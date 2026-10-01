### Engine comparison: linux-x86-64 (AMD EPYC)

Source `11a7ba0248c9`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### trivial

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 41.8 ns | 2.08 | 0.79 | 0.46 | 1.15 | 0.37 | 0.26 | – |
| `single_pattern__quantifier_greedy` | 42.8 ns | 2.72 | 1.58 | 0.56 | 1.13 | 1.36 | 1.39 | – |
| `single_pattern__alternation_2_branch` | 43.1 ns | 2.00 | 0.73 | 0.44 | 1.24 | 1.09 | 0.91 | – |
| `single_pattern__alternation_10_branch` | 32.6 ns | 4.49 | 3.38 | 0.68 | 4.35 | 0.67 | 0.52 | – |
| `single_pattern__case_insensitive_phrase` | 90.3 ns | 1.34 | 0.70 | 0.23 | 0.59 | 0.66 | 0.63 | – |
| `single_pattern__named_capture_date` | 82.1 ns | 2.71 | 1.32 | 0.51 | 3.78 | 0.53 | 0.52 | – |
| `single_pattern__unicode_greek` | 73.7 ns | 1.92 | 2.03 | 0.40 | 2.27 | 0.71 | 0.67 | – |
| `text_scanning__literal_50k` | 33.6 ns | 2.18 | 0.69 | 0.49 | 1.11 | 0.37 | 0.26 | – |
| `text_scanning__no_match_50k` | 617 ns | 10.34 | 11.10 | 1.45 | 0.37 | 0.99 | 0.99 | – |
| `text_scanning__field_extract_50k` | 45.1 ns | 2.03 | 1.02 | 0.50 | 1.32 | 1.09 | 0.98 | – |
| `text_scanning__timestamp_50k` | 68.5 ns | 1.30 | 1.79 | 0.41 | 0.92 | 0.83 | 0.73 | – |
| `general_regex__email_validation` | 5.41 µs | 1.21 | 1.71 | 0.27 | 0.85 | 0.30 | 0.21 | – |
| `general_regex__uuid_validation` | 5.09 µs | 1.98 | 2.94 | 0.42 | 0.89 | 0.44 | 0.35 | – |
| `general_regex__number_validation` | 4.93 µs | 1.61 | 1.70 | 0.38 | 1.98 | 0.26 | 0.18 | – |
| `general_regex__access_log_captures` | 22 µs | 1.11 | 1.52 | 0.23 | 0.79 | 1.59 | 1.44 | – |
| `general_regex__url_extraction` | 8.26 µs | 1.72 | 2.07 | 0.49 | 1.17 | 1.65 | 1.23 | – |
| `general_regex__unicode_words` | 43.3 µs | 1.58 | 1.45 | 0.39 | 1.20 | 1.34 | 0.67 | – |
| `general_regex__email_redaction` | 23.3 µs | 3.19 | 4.73 | 0.46 | 2.58 | 0.86 | 0.57 | – |
| `compilation__literal` | 640 ns | 0.61 | 0.55 | 7.31 | 0.65 | 4.31 | 3.32 | – |
| `compilation__named_capture` | 2.74 µs | 1.36 | 1.40 | 2.12 | 0.20 | 58.67 | 62.55 | – |

