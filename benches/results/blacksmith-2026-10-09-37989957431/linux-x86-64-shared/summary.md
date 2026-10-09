### Engine comparison: linux-x86-64 (AMD EPYC)

Source `a38768bc8012`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 452 µs | 1.26 | 1.36 | 0.34 | 0.72 | 2.39 | 2.02 | 1.78 |
| `regex_tasks__html_attributes` | 546 µs | 3.07 | 3.72 | 0.32 | 2.16 | 0.62 | 0.64 | 0.46 |
| `regex_tasks__html_comments` | 25.5 µs | 2.90 | 3.09 | 0.75 | 5.49 | 5.59 | 5.77 | 5.35 |
| `regex_tasks__hex_colors` | 22.3 µs | 6.41 | 14.91 | 0.68 | 0.99 | 51.05 | 2.68 | 0.86 |
| `regex_tasks__email_addresses` | 44.8 µs | 32.71 | 49.51 | 1.76 | 26.17 | 0.73 | 0.73 | 0.51 |
| `regex_tasks__emoji` | 134 µs | 2.46 | 3.35 | 0.72 | 4.34 | 0.73 | 0.71 | 0.53 |
| `regex_tasks__ascii_emoticons` | 77.4 µs | 1.25 | 1.76 | 0.34 | 0.76 | 1.13 | 1.10 | 0.89 |
| `regex_tasks__hashtags_mentions` | 80.6 µs | 2.58 | 2.58 | 0.59 | 1.34 | 1.72 | 1.84 | 1.02 |
| `regex_tasks__ipv4_addresses` | 256 µs | 1.45 | 2.76 | 0.93 | 5.47 | 21.33 | 6.07 | 5.38 |
| `regex_tasks__iso_timestamps` | 141 µs | 1.41 | 3.02 | 0.29 | 6.00 | 25.00 | 1.95 | 0.65 |
| `regex_tasks__log_keywords_ignorecase` | 112 µs | 4.30 | 5.35 | 1.05 | 2.63 | 39.17 | 1.26 | 0.86 |
| `regex_tasks__markdown_links` | 84.8 µs | 2.65 | 3.80 | 0.65 | 1.38 | 3.85 | 3.92 | 3.57 |
| `regex_tasks__semantic_versions` | 347 µs | 1.07 | 1.11 | 0.20 | 0.58 | 4.22 | 0.45 | 0.38 |
| `regex_tasks__json_strings` | 371 µs | 1.73 | 1.45 | 0.58 | 1.87 | 1.37 | 1.33 | 0.92 |
| `regex_tasks__csv_fields` | 222 µs | 1.82 | 1.58 | 0.51 | 1.54 | 1.42 | 1.50 | 1.00 |
| `regex_tasks__rfc5322_emails` | 4.28 ms | 0.67 | 1.76 | 0.06 | 0.98 | 0.03 | 0.03 | 0.03 |
| `regex_tasks__ipv6_addresses` | 1.75 ms | 0.64 | 1.47 | 0.38 | 2.25 | 0.07 | 0.07 | 0.07 |
| `regex_tasks__rfc3986_urls` | 80 µs | 1.86 | 2.30 | 0.71 | 2.11 | 40.49 | 1.13 | 0.63 |
| `regex_tasks__keyword_alternation_200` | 1.07 ms | 4.30 | 4.46 | 0.65 | 8.51 | 22.23 | 4.13 | 0.59 |
| `regex_tasks__csv_last_column_backtracking` | 257 µs | 0.89 | 1.04 | 0.16 | 0.42 | 0.60 | 0.94 | 0.78 |
| `regex_tasks__unicode_case_folding` | 235 µs | 3.05 | 0.81 | n/a | n/a | n/a | n/a | n/a |
| `general_regex__email_validation` | 8.34 µs | 0.96 | 1.17 | 0.18 | 0.59 | 0.20 | 0.22 | 0.15 |
| `general_regex__uuid_validation` | 4.84 µs | 2.16 | 3.20 | 0.36 | 0.73 | 0.46 | 0.47 | 0.33 |
| `general_regex__number_validation` | 4.88 µs | 1.56 | 1.87 | 0.39 | 1.86 | 0.29 | 0.31 | 0.17 |
| `general_regex__access_log_captures` | 26.2 µs | 1.04 | 1.21 | 0.18 | 0.52 | 1.18 | 1.39 | 1.15 |
| `general_regex__url_extraction` | 6.34 µs | 2.16 | 2.56 | 0.62 | 1.23 | 2.06 | 2.05 | 1.56 |
| `general_regex__unicode_words` | 45.5 µs | 1.68 | 1.44 | 0.48 | 1.03 | 1.28 | 1.23 | 0.87 |
| `general_regex__email_redaction` | 28.1 µs | 3.21 | 5.00 | 0.42 | 2.85 | 0.80 | 0.84 | 0.58 |
| `text_scanning__literal_50k` | 43.9 ns | 1.99 | 0.61 | 0.43 | 0.97 | 0.32 | 0.33 | 0.23 |
| `text_scanning__no_match_50k` | 753 ns | 9.91 | 9.69 | 1.37 | 0.36 | 1.08 | 1.09 | 0.95 |
| `text_scanning__field_extract_50k` | 53.9 ns | 1.53 | 1.03 | 0.51 | 1.18 | 1.01 | 0.95 | 0.89 |
| `text_scanning__timestamp_50k` | 68 ns | 1.70 | 2.15 | 0.53 | 1.12 | 0.91 | 0.92 | 0.84 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

