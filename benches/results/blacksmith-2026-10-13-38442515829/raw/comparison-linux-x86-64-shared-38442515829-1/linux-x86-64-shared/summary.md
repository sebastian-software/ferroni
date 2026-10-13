### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 362 µs | 1.43 | 1.71 | 0.38 | 0.86 | 2.88 | 2.44 | 1.85 |
| `regex_tasks__html_attributes` | 568 µs | 2.88 | 3.78 | 0.31 | 2.22 | 0.58 | 0.58 | 0.47 |
| `regex_tasks__html_comments` | 23.3 µs | 2.82 | 3.00 | 0.68 | 5.06 | 5.65 | 5.57 | 5.41 |
| `regex_tasks__hex_colors` | 22.1 µs | 6.25 | 15.31 | 0.67 | 1.20 | 69.68 | 2.72 | 0.97 |
| `regex_tasks__email_addresses` | 45.7 µs | 32.62 | 57.77 | 1.87 | 29.28 | 0.81 | 0.78 | 0.55 |
| `regex_tasks__emoji` | 151 µs | 2.74 | 3.49 | 0.74 | 4.07 | 0.68 | 0.65 | 0.54 |
| `regex_tasks__ascii_emoticons` | 105 µs | 1.03 | 1.34 | 0.30 | 0.62 | 0.84 | 0.87 | 0.70 |
| `regex_tasks__hashtags_mentions` | 79.6 µs | 2.21 | 2.26 | 0.60 | 1.08 | 1.47 | 1.56 | 0.94 |
| `regex_tasks__ipv4_addresses` | 223 µs | 1.40 | 2.33 | 0.80 | 4.38 | 17.92 | 5.52 | 4.82 |
| `regex_tasks__iso_timestamps` | 85.6 µs | 1.85 | 4.39 | 0.38 | 8.73 | 29.38 | 2.47 | 0.77 |
| `regex_tasks__log_keywords_ignorecase` | 86.6 µs | 4.11 | 6.54 | 1.28 | 2.67 | 40.16 | 1.20 | 0.95 |
| `regex_tasks__markdown_links` | 70.7 µs | 2.48 | 4.13 | 0.56 | 1.27 | 4.13 | 3.93 | 3.49 |
| `regex_tasks__semantic_versions` | 294 µs | 1.14 | 1.67 | 0.21 | 0.69 | 5.17 | 0.59 | 0.29 |
| `regex_tasks__json_strings` | 403 µs | 1.88 | 1.27 | 0.54 | 2.06 | 1.43 | 1.36 | 0.91 |
| `regex_tasks__csv_fields` | 210 µs | 1.89 | 1.70 | 0.59 | 1.72 | 1.54 | 1.55 | 1.04 |
| `regex_tasks__rfc5322_emails` | 4.44 ms | 0.79 | 2.20 | 0.07 | 1.20 | 0.03 | 0.03 | 0.03 |
| `regex_tasks__ipv6_addresses` | 2.13 ms | 0.75 | 1.37 | 0.36 | 2.36 | 0.06 | 0.06 | 0.06 |
| `regex_tasks__rfc3986_urls` | 80.3 µs | 1.66 | 2.24 | 0.74 | 2.14 | 22.56 | 1.06 | 0.59 |
| `regex_tasks__keyword_alternation_200` | 924 µs | 4.46 | 5.03 | 0.66 | 9.63 | 19.59 | 4.81 | 0.57 |
| `regex_tasks__csv_last_column_backtracking` | 170 µs | 0.93 | 1.44 | 0.25 | 0.51 | 0.68 | 1.19 | 0.98 |
| `regex_tasks__unicode_case_folding` | 201 µs | 3.05 | 0.90 | n/a | n/a | n/a | n/a | n/a |
| `general_regex__email_validation` | 7.73 µs | 1.24 | 1.51 | 0.28 | 0.77 | 0.25 | 0.27 | 0.25 |
| `general_regex__uuid_validation` | 5.22 µs | 2.13 | 2.96 | 0.40 | 0.77 | 0.41 | 0.41 | 0.30 |
| `general_regex__number_validation` | 5.23 µs | 1.36 | 1.56 | 0.36 | 1.76 | 0.26 | 0.26 | 0.17 |
| `general_regex__access_log_captures` | 25.7 µs | 1.05 | 1.51 | 0.20 | 0.52 | 1.29 | 1.50 | 1.18 |
| `general_regex__url_extraction` | 6.35 µs | 2.05 | 2.36 | 0.56 | 1.29 | 2.05 | 1.98 | 1.51 |
| `general_regex__unicode_words` | 38.1 µs | 1.64 | 1.33 | 0.55 | 1.13 | 1.20 | 1.34 | 0.87 |
| `general_regex__email_redaction` | 19.2 µs | 4.26 | 7.40 | 0.65 | 4.31 | 1.13 | 1.14 | 0.79 |
| `text_scanning__literal_50k` | 44 ns | 2.20 | 0.58 | 0.43 | 0.97 | 0.31 | 0.31 | 0.28 |
| `text_scanning__no_match_50k` | 780 ns | 8.59 | 8.87 | 1.10 | 0.35 | 1.01 | 1.21 | 0.82 |
| `text_scanning__field_extract_50k` | 55.9 ns | 1.81 | 0.81 | 0.45 | 1.28 | 1.03 | 0.90 | 0.92 |
| `text_scanning__timestamp_50k` | 62.9 ns | 1.47 | 2.36 | 0.50 | 1.16 | 0.98 | 0.96 | 0.82 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

