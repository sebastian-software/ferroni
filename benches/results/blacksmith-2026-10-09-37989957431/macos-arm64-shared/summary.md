### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `a38768bc8012`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 297 µs | 1.68 | 1.86 | 0.46 | 0.92 | 2.88 | 3.93 | 3.75 |
| `regex_tasks__html_attributes` | 489 µs | 3.01 | 4.06 | 0.38 | 2.10 | 0.56 | 0.57 | 0.43 |
| `regex_tasks__html_comments` | 18.4 µs | 3.48 | 3.28 | 0.66 | 6.68 | 6.68 | 6.63 | 6.52 |
| `regex_tasks__hex_colors` | 15.5 µs | 4.16 | 23.73 | 0.69 | 1.01 | 63.32 | 2.70 | 0.71 |
| `regex_tasks__email_addresses` | 37.1 µs | 33.14 | 61.41 | 2.99 | 30.95 | 0.65 | 0.64 | 0.42 |
| `regex_tasks__emoji` | 104 µs | 2.89 | 3.87 | 0.75 | 4.85 | 0.75 | 0.75 | 0.54 |
| `regex_tasks__ascii_emoticons` | 60.7 µs | 1.74 | 2.08 | 0.32 | 0.83 | 0.67 | 0.66 | 0.52 |
| `regex_tasks__hashtags_mentions` | 63.7 µs | 2.55 | 2.07 | 0.56 | 1.04 | 1.42 | 1.43 | 0.89 |
| `regex_tasks__ipv4_addresses` | 170 µs | 1.48 | 3.06 | 1.13 | 5.95 | 20.93 | 5.95 | 5.25 |
| `regex_tasks__iso_timestamps` | 68.5 µs | 2.06 | 4.91 | 0.38 | 6.72 | 24.79 | 2.35 | 0.85 |
| `regex_tasks__log_keywords_ignorecase` | 79 µs | 2.87 | 5.06 | 0.96 | 2.17 | 25.82 | 0.85 | 0.56 |
| `regex_tasks__markdown_links` | 43.7 µs | 3.12 | 5.73 | 0.83 | 1.54 | 5.99 | 5.95 | 5.63 |
| `regex_tasks__semantic_versions` | 224 µs | 1.22 | 1.66 | 0.28 | 0.74 | 5.07 | 0.59 | 0.33 |
| `regex_tasks__json_strings` | 300 µs | 2.03 | 1.47 | 0.50 | 2.20 | 1.25 | 1.23 | 0.70 |
| `regex_tasks__csv_fields` | 182 µs | 2.10 | 1.53 | 0.45 | 1.75 | 1.14 | 1.15 | 0.85 |
| `regex_tasks__rfc5322_emails` | 3.14 ms | 0.75 | 1.86 | 0.10 | 1.53 | 0.04 | 0.04 | 0.04 |
| `regex_tasks__ipv6_addresses` | 1.7 ms | 0.61 | 1.40 | 0.41 | 2.63 | 0.07 | 0.07 | 0.06 |
| `regex_tasks__rfc3986_urls` | 76.7 µs | 1.66 | 1.99 | 0.73 | 2.35 | 19.08 | 0.87 | 0.52 |
| `regex_tasks__keyword_alternation_200` | 742 µs | 3.94 | 4.96 | 0.56 | 10.99 | 22.36 | 4.08 | 0.46 |
| `regex_tasks__csv_last_column_backtracking` | 118 µs | 1.27 | 1.45 | 0.29 | 0.62 | 0.85 | 1.52 | 1.24 |
| `regex_tasks__unicode_case_folding` | 135 µs | 3.40 | 1.06 | n/a | n/a | n/a | n/a | n/a |
| `general_regex__email_validation` | 4.22 µs | 1.95 | 2.08 | 0.35 | 1.18 | 0.30 | 0.30 | 0.22 |
| `general_regex__uuid_validation` | 3.38 µs | 3.08 | 3.69 | 0.41 | 0.87 | 0.43 | 0.43 | 0.31 |
| `general_regex__number_validation` | 4.03 µs | 2.12 | 1.69 | 0.36 | 2.47 | 0.24 | 0.24 | 0.17 |
| `general_regex__access_log_captures` | 17.7 µs | 1.21 | 1.53 | 0.27 | 0.74 | 1.55 | 1.74 | 1.32 |
| `general_regex__url_extraction` | 5.27 µs | 2.58 | 2.55 | 0.56 | 1.37 | 1.92 | 1.93 | 1.57 |
| `general_regex__unicode_words` | 29.1 µs | 2.12 | 1.29 | 0.39 | 1.06 | 1.01 | 1.02 | 0.67 |
| `general_regex__email_redaction` | 14.5 µs | 4.15 | 6.47 | 0.59 | 3.73 | 0.81 | 0.81 | 0.61 |
| `text_scanning__literal_50k` | 37.9 ns | 2.42 | 0.67 | 0.34 | 0.84 | 0.25 | 0.26 | 0.18 |
| `text_scanning__no_match_50k` | 959 ns | 5.23 | 5.19 | 1.20 | 0.79 | 0.99 | 0.99 | 0.99 |
| `text_scanning__field_extract_50k` | 39.8 ns | 2.63 | 1.03 | 0.48 | 1.26 | 0.83 | 0.83 | 0.81 |
| `text_scanning__timestamp_50k` | 59 ns | 1.83 | 1.69 | 0.42 | 0.91 | 0.67 | 0.66 | 0.59 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

