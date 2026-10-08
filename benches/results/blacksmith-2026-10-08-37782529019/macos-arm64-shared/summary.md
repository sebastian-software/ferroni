### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `62892008bbf5`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 326 µs | 1.37 | 1.59 | 0.39 | 0.77 | 2.37 | 3.24 | – |
| `regex_tasks__html_attributes` | 513 µs | 2.97 | 3.94 | 0.37 | 1.98 | 0.55 | 0.45 | – |
| `regex_tasks__html_comments` | 21.1 µs | 3.18 | 3.45 | 0.63 | 6.71 | 7.08 | 6.00 | – |
| `regex_tasks__hex_colors` | 17.6 µs | 4.70 | 22.86 | 0.70 | 1.05 | 62.28 | 0.72 | – |
| `regex_tasks__email_addresses` | 41.7 µs | 32.16 | 61.74 | 3.33 | 33.09 | 0.62 | 0.44 | – |
| `regex_tasks__emoji` | 115 µs | 2.95 | 3.84 | 0.77 | 4.84 | 0.77 | 0.55 | – |
| `regex_tasks__ascii_emoticons` | 64.4 µs | 1.82 | 2.23 | 0.38 | 0.95 | 0.75 | 0.57 | – |
| `regex_tasks__hashtags_mentions` | 76.4 µs | 2.60 | 1.96 | 0.55 | 0.98 | 1.21 | 0.91 | – |
| `regex_tasks__ipv4_addresses` | 205 µs | 1.37 | 2.90 | 1.05 | 5.59 | 19.83 | 4.85 | – |
| `regex_tasks__iso_timestamps` | 77.7 µs | 2.11 | 5.10 | 0.39 | 6.92 | 25.31 | 0.86 | – |
| `regex_tasks__log_keywords_ignorecase` | 95.5 µs | 2.64 | 4.83 | 0.93 | 2.00 | 23.24 | 0.55 | – |
| `regex_tasks__markdown_links` | 51 µs | 3.15 | 5.32 | 0.80 | 1.48 | 5.62 | 5.34 | – |
| `regex_tasks__semantic_versions` | 240 µs | 1.04 | 1.70 | 0.29 | 0.75 | 5.08 | 0.32 | – |
| `regex_tasks__json_strings` | 321 µs | 2.00 | 1.59 | 0.47 | 2.06 | 1.13 | 0.66 | – |
| `regex_tasks__csv_fields` | 213 µs | 2.09 | 1.57 | 0.45 | 1.84 | 1.16 | 0.85 | – |
| `regex_tasks__rfc5322_emails` | 3.58 ms | 0.76 | 1.95 | 0.10 | 1.56 | 0.04 | 0.04 | – |
| `regex_tasks__ipv6_addresses` | 2.04 ms | 0.58 | 1.31 | 0.39 | 2.37 | 0.07 | 0.06 | – |
| `regex_tasks__rfc3986_urls` | 84 µs | 1.64 | 2.07 | 0.76 | 2.28 | 18.80 | 0.51 | – |
| `regex_tasks__keyword_alternation_200` | 864 µs | 3.79 | 4.94 | 0.56 | 9.82 | 19.91 | 0.45 | – |
| `regex_tasks__csv_last_column_backtracking` | 140 µs | 1.17 | 1.51 | 0.27 | 0.57 | 0.82 | 1.15 | – |
| `regex_tasks__unicode_case_folding` | 151 µs | 3.24 | 1.04 | n/a | n/a | n/a | n/a | – |
| `general_regex__email_validation` | 4.49 µs | 1.98 | 2.13 | 0.36 | 1.19 | 0.31 | 0.22 | – |
| `general_regex__uuid_validation` | 3.68 µs | 3.22 | 3.40 | 0.43 | 0.84 | 0.42 | 0.34 | – |
| `general_regex__number_validation` | 4.01 µs | 2.18 | 1.98 | 0.37 | 2.73 | 0.28 | 0.17 | – |
| `general_regex__access_log_captures` | 20.4 µs | 1.32 | 1.54 | 0.25 | 0.74 | 1.46 | 1.34 | – |
| `general_regex__url_extraction` | 5.73 µs | 2.83 | 2.64 | 0.62 | 1.53 | 2.14 | 1.69 | – |
| `general_regex__unicode_words` | 33.3 µs | 1.89 | 1.29 | 0.38 | 1.04 | 0.99 | 0.66 | – |
| `general_regex__email_redaction` | 16 µs | 4.29 | 6.10 | 0.60 | 3.75 | 0.79 | 0.60 | – |
| `text_scanning__literal_50k` | 41.4 ns | 2.41 | 0.66 | 0.34 | 0.80 | 0.25 | 0.17 | – |
| `text_scanning__no_match_50k` | 1.09 µs | 5.56 | 5.43 | 1.16 | 0.79 | 0.99 | 0.96 | – |
| `text_scanning__field_extract_50k` | 43.6 ns | 2.74 | 1.03 | 0.49 | 1.32 | 0.86 | 0.81 | – |
| `text_scanning__timestamp_50k` | 66.8 ns | 1.86 | 1.63 | 0.43 | 0.93 | 0.67 | 0.59 | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

