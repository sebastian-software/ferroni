### Engine comparison: linux-x86-64 (Intel(R) Xeon(R) Processor)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 375 µs | 1.59 | 1.61 | 0.40 | 1.00 | 2.37 | 2.77 | 2.96 |
| `regex_tasks__html_attributes` | 701 µs | 2.45 | 4.54 | 0.33 | 2.46 | 0.47 | 0.54 | 0.34 |
| `regex_tasks__html_comments` | 36.8 µs | 2.36 | 2.09 | 0.65 | 4.53 | 3.99 | 4.03 | 3.64 |
| `regex_tasks__hex_colors` | 33.1 µs | 4.47 | 10.29 | 0.47 | 0.85 | 35.74 | 1.56 | 0.59 |
| `regex_tasks__email_addresses` | 50 µs | 32.87 | 53.59 | 2.53 | 23.03 | 0.65 | 0.60 | 0.44 |
| `regex_tasks__emoji` | 165 µs | 2.26 | 3.72 | 0.62 | 3.78 | 0.55 | 0.63 | 0.45 |
| `regex_tasks__ascii_emoticons` | 160 µs | 1.04 | 0.92 | 0.32 | 0.61 | 0.61 | 0.57 | 0.51 |
| `regex_tasks__hashtags_mentions` | 115 µs | 1.91 | 1.69 | 0.50 | 0.82 | 1.03 | 1.09 | 0.66 |
| `regex_tasks__ipv4_addresses` | 289 µs | 1.45 | 1.92 | 0.77 | 4.51 | 13.51 | 4.25 | 3.83 |
| `regex_tasks__iso_timestamps` | 119 µs | 2.05 | 3.52 | 0.31 | 5.18 | 26.31 | 1.73 | 0.82 |
| `regex_tasks__log_keywords_ignorecase` | 83.6 µs | 4.44 | 5.62 | 1.27 | 2.48 | 31.96 | 1.14 | 0.77 |
| `regex_tasks__markdown_links` | 64.8 µs | 2.64 | 4.43 | 0.77 | 1.41 | 4.02 | 4.14 | 3.69 |
| `regex_tasks__semantic_versions` | 338 µs | 0.99 | 1.63 | 0.22 | 0.83 | 4.47 | 0.61 | 0.42 |
| `regex_tasks__json_strings` | 428 µs | 2.24 | 1.34 | 0.60 | 2.15 | 1.92 | 1.67 | 1.29 |
| `regex_tasks__csv_fields` | 407 µs | 1.69 | 1.37 | 0.49 | 1.42 | 1.23 | 1.04 | 0.81 |
| `regex_tasks__rfc5322_emails` | 7.05 ms | 0.70 | 1.64 | 0.07 | 1.06 | 0.02 | 0.02 | 0.02 |
| `regex_tasks__ipv6_addresses` | 2.8 ms | 0.60 | 1.22 | 0.54 | 2.69 | 0.06 | 0.05 | 0.04 |
| `regex_tasks__rfc3986_urls` | 157 µs | 1.42 | 1.33 | 0.61 | 1.64 | 17.17 | 0.84 | 0.56 |
| `regex_tasks__keyword_alternation_200` | 1.21 ms | 5.11 | 4.96 | 0.60 | 8.75 | 24.59 | 4.65 | 0.61 |
| `regex_tasks__csv_last_column_backtracking` | 280 µs | 0.91 | 0.85 | 0.26 | 0.51 | 0.42 | 0.88 | 0.69 |
| `regex_tasks__unicode_case_folding` | 305 µs | 2.80 | 0.62 | n/a | n/a | n/a | n/a | n/a |
| `general_regex__email_validation` | 11.1 µs | 1.58 | 1.87 | 0.28 | 0.77 | 0.25 | 0.23 | 0.17 |
| `general_regex__uuid_validation` | 8.67 µs | 1.89 | 2.14 | 0.32 | 0.73 | 0.30 | 0.31 | 0.24 |
| `general_regex__number_validation` | 9.61 µs | 1.42 | 1.43 | 0.36 | 1.62 | 0.21 | 0.20 | 0.12 |
| `general_regex__access_log_captures` | 32.5 µs | 1.54 | 1.52 | 0.25 | 0.85 | 1.83 | 1.45 | 1.08 |
| `general_regex__url_extraction` | 10.7 µs | 2.51 | 2.50 | 0.52 | 1.15 | 1.84 | 1.84 | 1.34 |
| `general_regex__unicode_words` | 77.5 µs | 1.70 | 1.21 | 0.42 | 0.92 | 1.03 | 1.03 | 0.68 |
| `general_regex__email_redaction` | 32.9 µs | 3.38 | 6.12 | 0.45 | 2.58 | 0.65 | 0.82 | 0.55 |
| `text_scanning__literal_50k` | 61 ns | 2.52 | 0.88 | 0.58 | 1.18 | 0.43 | 0.46 | 0.29 |
| `text_scanning__no_match_50k` | 1.9 µs | 4.21 | 3.87 | 1.40 | 0.33 | 0.97 | 0.97 | 0.78 |
| `text_scanning__field_extract_50k` | 93.6 ns | 1.97 | 0.52 | 0.38 | 1.22 | 0.56 | 0.88 | 0.76 |
| `text_scanning__timestamp_50k` | 103 ns | 1.23 | 1.17 | 0.43 | 1.26 | 0.71 | 0.62 | 0.66 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

