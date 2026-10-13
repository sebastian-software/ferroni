### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 302 µs | 1.69 | 1.89 | 0.47 | 0.93 | 2.87 | 3.94 | 3.99 |
| `regex_tasks__html_attributes` | 510 µs | 3.21 | 4.20 | 0.39 | 2.14 | 0.57 | 0.60 | 0.47 |
| `regex_tasks__html_comments` | 20.3 µs | 3.71 | 3.36 | 0.70 | 6.76 | 6.94 | 7.01 | 7.34 |
| `regex_tasks__hex_colors` | 16.5 µs | 5.05 | 25.48 | 0.84 | 1.24 | 71.33 | 3.08 | 0.88 |
| `regex_tasks__email_addresses` | 46.1 µs | 30.92 | 57.29 | 2.69 | 27.20 | 0.55 | 0.55 | 0.40 |
| `regex_tasks__emoji` | 114 µs | 2.83 | 4.30 | 0.79 | 4.98 | 0.84 | 0.84 | 0.57 |
| `regex_tasks__ascii_emoticons` | 67.2 µs | 1.81 | 2.14 | 0.33 | 0.89 | 0.70 | 0.69 | 0.52 |
| `regex_tasks__hashtags_mentions` | 75.5 µs | 2.43 | 1.76 | 0.53 | 0.98 | 1.15 | 1.14 | 0.86 |
| `regex_tasks__ipv4_addresses` | 154 µs | 1.63 | 3.39 | 1.26 | 6.48 | 23.38 | 6.55 | 5.79 |
| `regex_tasks__iso_timestamps` | 68.7 µs | 2.06 | 4.71 | 0.38 | 6.72 | 24.47 | 2.31 | 0.85 |
| `regex_tasks__log_keywords_ignorecase` | 79.7 µs | 2.78 | 4.92 | 0.91 | 2.12 | 24.68 | 0.83 | 0.55 |
| `regex_tasks__markdown_links` | 42.2 µs | 3.04 | 5.78 | 0.84 | 1.59 | 6.06 | 6.04 | 5.70 |
| `regex_tasks__semantic_versions` | 206 µs | 1.06 | 1.78 | 0.31 | 0.81 | 5.53 | 0.64 | 0.36 |
| `regex_tasks__json_strings` | 292 µs | 1.99 | 1.50 | 0.51 | 2.26 | 1.14 | 1.14 | 0.74 |
| `regex_tasks__csv_fields` | 181 µs | 2.13 | 1.53 | 0.45 | 1.78 | 1.15 | 1.14 | 0.86 |
| `regex_tasks__rfc5322_emails` | 3.12 ms | 0.75 | 2.27 | 0.10 | 1.52 | 0.04 | 0.05 | 0.04 |
| `regex_tasks__ipv6_addresses` | 1.79 ms | 0.65 | 1.46 | 0.45 | 2.86 | 0.07 | 0.07 | 0.07 |
| `regex_tasks__rfc3986_urls` | 80.3 µs | 1.76 | 2.07 | 0.80 | 2.41 | 19.00 | 0.91 | 0.55 |
| `regex_tasks__keyword_alternation_200` | 802 µs | 4.33 | 5.17 | 0.61 | 11.46 | 24.24 | 4.19 | 0.48 |
| `regex_tasks__csv_last_column_backtracking` | 139 µs | 1.13 | 1.35 | 0.27 | 0.61 | 0.85 | 1.50 | 1.18 |
| `regex_tasks__unicode_case_folding` | 134 µs | 3.44 | 1.11 | n/a | n/a | n/a | n/a | n/a |
| `general_regex__email_validation` | 4.84 µs | 2.02 | 1.96 | 0.33 | 1.14 | 0.29 | 0.28 | 0.22 |
| `general_regex__uuid_validation` | 3.63 µs | 3.13 | 3.66 | 0.40 | 0.86 | 0.42 | 0.42 | 0.32 |
| `general_regex__number_validation` | 4.14 µs | 2.26 | 1.81 | 0.39 | 2.69 | 0.27 | 0.24 | 0.16 |
| `general_regex__access_log_captures` | 17.5 µs | 1.36 | 1.78 | 0.32 | 0.83 | 1.73 | 2.01 | 1.57 |
| `general_regex__url_extraction` | 5.78 µs | 2.72 | 2.65 | 0.56 | 1.34 | 1.98 | 1.95 | 1.58 |
| `general_regex__unicode_words` | 31.7 µs | 1.97 | 1.31 | 0.42 | 1.15 | 1.05 | 1.05 | 0.70 |
| `general_regex__email_redaction` | 16.5 µs | 4.17 | 6.52 | 0.61 | 3.81 | 0.83 | 0.85 | 0.63 |
| `text_scanning__literal_50k` | 42.3 ns | 2.92 | 0.73 | 0.37 | 0.94 | 0.27 | 0.28 | 0.18 |
| `text_scanning__no_match_50k` | 1.12 µs | 5.68 | 5.66 | 1.15 | 0.79 | 0.99 | 0.98 | 0.96 |
| `text_scanning__field_extract_50k` | 42.8 ns | 2.96 | 1.11 | 0.53 | 1.37 | 0.91 | 0.92 | 0.89 |
| `text_scanning__timestamp_50k` | 65.9 ns | 2.00 | 1.70 | 0.44 | 0.96 | 0.70 | 0.67 | 0.60 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

