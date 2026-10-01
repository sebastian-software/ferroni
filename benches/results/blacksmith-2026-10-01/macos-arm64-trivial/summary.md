### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `11a7ba0248c9`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### trivial

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 36.1 ns | 3.04 | 1.05 | 0.42 | 1.17 | 0.32 | 0.21 | – |
| `single_pattern__quantifier_greedy` | 47 ns | 3.36 | 1.42 | 0.44 | 0.92 | 1.10 | 0.99 | – |
| `single_pattern__alternation_2_branch` | 42.3 ns | 2.84 | 0.86 | 0.39 | 1.36 | 0.84 | 0.76 | – |
| `single_pattern__alternation_10_branch` | 28.5 ns | 6.22 | 4.71 | 0.78 | 7.74 | 0.67 | 0.52 | – |
| `single_pattern__case_insensitive_phrase` | 70.1 ns | 2.11 | 0.81 | 0.27 | 0.76 | 0.61 | 0.56 | – |
| `single_pattern__named_capture_date` | 67.8 ns | 3.71 | 1.74 | 0.75 | 4.50 | 0.52 | 0.49 | – |
| `single_pattern__unicode_greek` | 63.1 ns | 2.98 | 2.47 | 0.47 | 2.09 | 0.62 | 0.61 | – |
| `text_scanning__literal_50k` | 39.1 ns | 2.82 | 0.78 | 0.41 | 0.96 | 0.30 | 0.21 | – |
| `text_scanning__no_match_50k` | 1.17 µs | 5.79 | 5.32 | 1.22 | 0.79 | 0.96 | 0.99 | – |
| `text_scanning__field_extract_50k` | 41 ns | 2.76 | 0.98 | 0.48 | 1.26 | 0.81 | 0.83 | – |
| `text_scanning__timestamp_50k` | 58.3 ns | 1.91 | 1.87 | 0.44 | 1.01 | 0.75 | 0.61 | – |
| `general_regex__email_validation` | 4.41 µs | 1.95 | 2.36 | 0.33 | 1.20 | 0.34 | 0.21 | – |
| `general_regex__uuid_validation` | 3.68 µs | 3.17 | 3.56 | 0.42 | 0.87 | 0.42 | 0.35 | – |
| `general_regex__number_validation` | 4.29 µs | 2.23 | 1.79 | 0.37 | 2.48 | 0.24 | 0.17 | – |
| `general_regex__access_log_captures` | 19.5 µs | 1.26 | 1.68 | 0.28 | 0.82 | 1.66 | 1.33 | – |
| `general_regex__url_extraction` | 6.86 µs | 2.46 | 2.26 | 0.49 | 1.21 | 1.70 | 1.41 | – |
| `general_regex__unicode_words` | 32.3 µs | 2.12 | 1.17 | 0.40 | 1.07 | 1.02 | 0.70 | – |
| `general_regex__email_redaction` | 14.5 µs | 4.58 | 7.40 | 0.67 | 4.17 | 0.93 | 0.73 | – |
| `compilation__literal` | 508 ns | 0.67 | 0.62 | 3.91 | 0.65 | 4.69 | 3.56 | – |
| `compilation__named_capture` | 2.61 µs | 1.70 | 1.98 | 1.26 | 0.20 | 61.98 | 57.38 | – |

