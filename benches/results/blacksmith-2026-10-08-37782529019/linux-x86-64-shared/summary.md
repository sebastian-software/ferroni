### Engine comparison: linux-x86-64 (AMD EPYC)

Source `62892008bbf5`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 310 µs | 1.45 | 1.68 | 0.42 | 0.89 | 2.57 | 2.36 | – |
| `regex_tasks__html_attributes` | 509 µs | 2.77 | 3.51 | 0.30 | 1.88 | 0.61 | 0.46 | – |
| `regex_tasks__html_comments` | 21.7 µs | 3.14 | 2.98 | 0.67 | 4.78 | 6.18 | 5.92 | – |
| `regex_tasks__hex_colors` | 19 µs | 6.96 | 17.06 | 0.69 | 1.05 | 48.91 | 0.91 | – |
| `regex_tasks__email_addresses` | 41.8 µs | 31.98 | 52.45 | 1.82 | 25.57 | 0.81 | 0.57 | – |
| `regex_tasks__emoji` | 133 µs | 2.34 | 3.09 | 0.72 | 4.28 | 0.77 | 0.58 | – |
| `regex_tasks__ascii_emoticons` | 77.7 µs | 1.37 | 1.72 | 0.35 | 0.70 | 1.15 | 0.96 | – |
| `regex_tasks__hashtags_mentions` | 74.7 µs | 2.26 | 2.08 | 0.63 | 1.13 | 1.59 | 1.05 | – |
| `regex_tasks__ipv4_addresses` | 189 µs | 1.47 | 2.72 | 0.89 | 5.00 | 19.55 | 5.52 | – |
| `regex_tasks__iso_timestamps` | 77.7 µs | 2.19 | 4.20 | 0.35 | 7.59 | 27.01 | 0.85 | – |
| `regex_tasks__log_keywords_ignorecase` | 69.3 µs | 5.05 | 6.00 | 1.14 | 2.72 | 35.43 | 0.93 | – |
| `regex_tasks__markdown_links` | 51.8 µs | 3.00 | 4.77 | 0.67 | 1.38 | 4.79 | 4.34 | – |
| `regex_tasks__semantic_versions` | 252 µs | 1.24 | 1.56 | 0.24 | 0.75 | 5.56 | 0.33 | – |
| `regex_tasks__json_strings` | 353 µs | 1.78 | 1.57 | 0.57 | 1.97 | 1.50 | 0.96 | – |
| `regex_tasks__csv_fields` | 216 µs | 1.91 | 1.64 | 0.53 | 1.52 | 1.48 | 1.06 | – |
| `regex_tasks__rfc5322_emails` | 4.05 ms | 0.72 | 1.72 | 0.06 | 1.03 | 0.04 | 0.03 | – |
| `regex_tasks__ipv6_addresses` | 1.81 ms | 0.70 | 1.43 | 0.40 | 2.32 | 0.08 | 0.07 | – |
| `regex_tasks__rfc3986_urls` | 79.9 µs | 1.83 | 2.06 | 0.76 | 1.97 | 23.53 | 0.67 | – |
| `regex_tasks__keyword_alternation_200` | 849 µs | 5.65 | 3.41 | 0.52 | 8.27 | 15.38 | 0.56 | – |
| `regex_tasks__csv_last_column_backtracking` | 154 µs | 0.94 | 1.16 | 0.25 | 0.51 | 0.71 | 1.07 | – |
| `regex_tasks__unicode_case_folding` | 168 µs | 2.92 | 0.88 | n/a | n/a | n/a | n/a | – |
| `general_regex__email_validation` | 5.11 µs | 1.36 | 1.74 | 0.28 | 0.82 | 0.33 | 0.24 | – |
| `general_regex__uuid_validation` | 4.37 µs | 2.19 | 2.97 | 0.38 | 0.73 | 0.46 | 0.32 | – |
| `general_regex__number_validation` | 4.45 µs | 1.77 | 1.56 | 0.40 | 1.82 | 0.32 | 0.19 | – |
| `general_regex__access_log_captures` | 18.5 µs | 1.31 | 1.66 | 0.27 | 0.72 | 1.47 | 1.48 | – |
| `general_regex__url_extraction` | 6.15 µs | 2.33 | 2.49 | 0.53 | 1.17 | 2.03 | 1.57 | – |
| `general_regex__unicode_words` | 35.7 µs | 1.94 | 1.31 | 0.47 | 1.14 | 1.31 | 0.74 | – |
| `general_regex__email_redaction` | 17.7 µs | 3.46 | 5.81 | 0.50 | 3.02 | 1.05 | 0.74 | – |
| `text_scanning__literal_50k` | 37.5 ns | 2.29 | 0.66 | 0.46 | 1.01 | 0.35 | 0.24 | – |
| `text_scanning__no_match_50k` | 717 ns | 9.82 | 9.86 | 1.26 | 0.36 | 0.96 | 0.93 | – |
| `text_scanning__field_extract_50k` | 45.6 ns | 1.89 | 0.99 | 0.49 | 1.34 | 1.09 | 0.97 | – |
| `text_scanning__timestamp_50k` | 61.7 ns | 1.64 | 2.00 | 0.48 | 1.05 | 0.91 | 0.87 | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

