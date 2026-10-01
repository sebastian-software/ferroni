### Engine comparison: linux-x86-64 (AMD EPYC)

Source `6db6413e84b0`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 385 µs | 1.24 | 1.52 | 0.36 | 0.76 | 2.30 | 2.03 | – |
| `regex_tasks__html_attributes` | 1.2 ms | 1.28 | 1.61 | 0.13 | 0.88 | 0.27 | 0.20 | – |
| `regex_tasks__html_comments` | 24.6 µs | 2.81 | 2.64 | 0.59 | 4.43 | 5.45 | 5.29 | – |
| `regex_tasks__hex_colors` | 19.9 µs | 6.59 | 16.46 | 0.67 | 1.07 | 48.67 | 0.89 | – |
| `regex_tasks__email_addresses` | 48.6 µs | 32.10 | 53.35 | 1.67 | 26.15 | 0.81 | 0.49 | – |
| `regex_tasks__emoji` | 159 µs | 2.19 | 2.89 | 0.63 | 3.97 | 0.67 | 0.50 | – |
| `regex_tasks__ascii_emoticons` | 85.4 µs | 1.38 | 1.68 | 0.34 | 0.75 | 1.11 | 0.92 | – |
| `regex_tasks__hashtags_mentions` | 83.3 µs | 2.09 | 2.44 | 0.58 | 1.03 | 1.61 | 0.98 | – |
| `regex_tasks__ipv4_addresses` | 214 µs | 1.40 | 2.72 | 0.87 | 5.14 | 18.95 | 5.17 | – |
| `regex_tasks__iso_timestamps` | 85.2 µs | 2.05 | 3.97 | 0.35 | 7.38 | 24.21 | 0.86 | – |
| `regex_tasks__log_keywords_ignorecase` | 73.4 µs | 5.23 | 5.84 | 1.17 | 2.68 | 37.39 | 0.91 | – |
| `regex_tasks__markdown_links` | 95.4 µs | 1.91 | 2.74 | 0.41 | 0.91 | 2.81 | 2.82 | – |
| `regex_tasks__semantic_versions` | 302 µs | 1.03 | 1.40 | 0.22 | 0.66 | 5.19 | 0.30 | – |
| `regex_tasks__json_strings` | 408 µs | 1.72 | 1.32 | 0.55 | 1.85 | 1.41 | 0.88 | – |
| `regex_tasks__csv_fields` | 219 µs | 1.94 | 1.61 | 0.53 | 1.70 | 1.50 | 1.03 | – |
| `regex_tasks__rfc5322_emails` | 5.07 ms | 0.73 | 1.73 | 0.06 | 1.05 | 0.03 | 0.03 | – |
| `regex_tasks__ipv6_addresses` | 2.13 ms | 0.67 | 1.55 | 0.41 | 2.34 | 0.07 | 0.06 | – |
| `regex_tasks__rfc3986_urls` | 93.4 µs | 1.70 | 1.78 | 0.67 | 1.88 | 21.87 | 0.57 | – |
| `regex_tasks__keyword_alternation_200` | 907 µs | 4.46 | 3.59 | 0.49 | 7.13 | 14.18 | 0.52 | – |
| `regex_tasks__csv_last_column_backtracking` | 155 µs | 0.95 | 1.22 | 0.24 | 0.50 | 0.70 | 1.06 | – |
| `regex_tasks__unicode_case_folding` | 192 µs | 2.66 | 0.83 | n/a | n/a | n/a | n/a | – |
| `general_regex__email_validation` | 5.16 µs | 1.67 | 1.84 | 0.31 | 0.91 | 0.32 | 0.26 | – |
| `general_regex__uuid_validation` | 4.59 µs | 2.44 | 2.89 | 0.39 | 0.75 | 0.46 | 0.32 | – |
| `general_regex__number_validation` | 4.75 µs | 1.77 | 1.60 | 0.37 | 1.90 | 0.28 | 0.18 | – |
| `general_regex__access_log_captures` | 20.8 µs | 1.23 | 1.58 | 0.24 | 0.67 | 1.39 | 1.36 | – |
| `general_regex__url_extraction` | 6.18 µs | 2.45 | 2.34 | 0.54 | 1.17 | 2.04 | 1.66 | – |
| `general_regex__unicode_words` | 39.2 µs | 1.77 | 1.59 | 0.46 | 1.07 | 1.26 | 0.79 | – |
| `general_regex__email_redaction` | 25.2 µs | 2.89 | 4.48 | 0.39 | 2.19 | 0.76 | 0.54 | – |
| `text_scanning__literal_50k` | 41.2 ns | 2.31 | 0.62 | 0.48 | 1.06 | 0.33 | 0.27 | – |
| `text_scanning__no_match_50k` | 716 ns | 10.21 | 9.75 | 1.31 | 0.35 | 0.97 | 0.98 | – |
| `text_scanning__field_extract_50k` | 49.8 ns | 2.15 | 1.00 | 0.47 | 1.22 | 1.02 | 0.99 | – |
| `text_scanning__timestamp_50k` | 63.6 ns | 1.68 | 1.63 | 0.46 | 1.00 | 0.90 | 0.84 | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

