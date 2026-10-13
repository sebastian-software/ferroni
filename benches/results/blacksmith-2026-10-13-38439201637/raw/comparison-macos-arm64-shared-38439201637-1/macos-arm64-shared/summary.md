### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 296 µs | 1.67 | 1.89 | 0.52 | 0.97 | 2.97 | 3.80 | 3.91 |
| `regex_tasks__html_attributes` | 487 µs | 3.21 | 4.46 | 0.42 | 2.28 | 0.60 | 0.59 | 0.49 |
| `regex_tasks__html_comments` | 20.5 µs | 3.80 | 3.47 | 0.69 | 7.32 | 6.99 | 7.04 | 7.08 |
| `regex_tasks__hex_colors` | 16.6 µs | 5.07 | 25.80 | 0.78 | 1.12 | 67.34 | 2.93 | 0.78 |
| `regex_tasks__email_addresses` | 42.5 µs | 33.11 | 59.37 | 2.96 | 30.44 | 0.59 | 0.58 | 0.42 |
| `regex_tasks__emoji` | 114 µs | 3.01 | 4.24 | 0.77 | 5.12 | 0.86 | 0.85 | 0.58 |
| `regex_tasks__ascii_emoticons` | 64.5 µs | 1.85 | 2.12 | 0.35 | 0.88 | 0.69 | 0.68 | 0.54 |
| `regex_tasks__hashtags_mentions` | 70.4 µs | 2.53 | 2.14 | 0.56 | 1.04 | 1.28 | 1.31 | 0.88 |
| `regex_tasks__ipv4_addresses` | 179 µs | 1.54 | 2.92 | 1.12 | 5.78 | 20.42 | 5.69 | 5.21 |
| `regex_tasks__iso_timestamps` | 68.8 µs | 2.08 | 5.81 | 0.39 | 6.79 | 25.85 | 2.71 | 0.85 |
| `regex_tasks__log_keywords_ignorecase` | 92.1 µs | 2.73 | 4.69 | 0.90 | 2.03 | 23.61 | 0.80 | 0.58 |
| `regex_tasks__markdown_links` | 43.7 µs | 3.49 | 6.30 | 0.95 | 1.81 | 6.82 | 6.50 | 6.35 |
| `regex_tasks__semantic_versions` | 241 µs | 1.09 | 1.55 | 0.31 | 0.69 | 4.65 | 0.54 | 0.32 |
| `regex_tasks__json_strings` | 291 µs | 2.09 | 1.51 | 0.51 | 2.27 | 1.15 | 1.15 | 0.73 |
| `regex_tasks__csv_fields` | 182 µs | 2.09 | 1.52 | 0.45 | 1.78 | 1.13 | 1.13 | 0.83 |
| `regex_tasks__rfc5322_emails` | 3.18 ms | 0.74 | 2.17 | 0.10 | 1.52 | 0.05 | 0.05 | 0.04 |
| `regex_tasks__ipv6_addresses` | 1.85 ms | 0.63 | 1.61 | 0.44 | 2.70 | 0.07 | 0.08 | 0.07 |
| `regex_tasks__rfc3986_urls` | 86.8 µs | 1.71 | 1.93 | 0.76 | 2.27 | 19.85 | 0.88 | 0.54 |
| `regex_tasks__keyword_alternation_200` | 851 µs | 3.84 | 4.96 | 0.57 | 10.95 | 22.34 | 4.01 | 0.47 |
| `regex_tasks__csv_last_column_backtracking` | 142 µs | 1.15 | 1.34 | 0.27 | 0.57 | 0.78 | 1.36 | 1.16 |
| `regex_tasks__unicode_case_folding` | 141 µs | 3.62 | 1.11 | n/a | n/a | n/a | n/a | n/a |
| `general_regex__email_validation` | 4.67 µs | 1.99 | 2.00 | 0.34 | 1.15 | 0.33 | 0.29 | 0.22 |
| `general_regex__uuid_validation` | 3.77 µs | 3.03 | 3.32 | 0.39 | 0.79 | 0.39 | 0.39 | 0.31 |
| `general_regex__number_validation` | 3.98 µs | 2.14 | 2.04 | 0.37 | 2.49 | 0.26 | 0.28 | 0.17 |
| `general_regex__access_log_captures` | 19.8 µs | 1.34 | 1.54 | 0.27 | 0.72 | 1.49 | 1.75 | 1.29 |
| `general_regex__url_extraction` | 5.32 µs | 2.81 | 2.53 | 0.64 | 1.55 | 2.22 | 2.09 | 1.65 |
| `general_regex__unicode_words` | 28.2 µs | 1.89 | 1.31 | 0.39 | 1.09 | 1.03 | 1.04 | 0.67 |
| `general_regex__email_redaction` | 14.2 µs | 4.21 | 6.55 | 0.61 | 3.78 | 0.83 | 0.83 | 0.61 |
| `text_scanning__literal_50k` | 35.5 ns | 3.20 | 0.82 | 0.43 | 1.02 | 0.32 | 0.32 | 0.22 |
| `text_scanning__no_match_50k` | 1.04 µs | 5.86 | 5.66 | 1.25 | 0.80 | 1.03 | 1.03 | 0.99 |
| `text_scanning__field_extract_50k` | 43.5 ns | 2.81 | 1.04 | 0.52 | 1.41 | 0.89 | 0.91 | 0.88 |
| `text_scanning__timestamp_50k` | 65.5 ns | 1.86 | 1.74 | 0.43 | 0.94 | 0.69 | 0.68 | 0.60 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

