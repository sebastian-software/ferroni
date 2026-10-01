### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `6db6413e84b0`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 409 µs | 1.31 | 1.39 | 0.35 | 0.68 | 2.11 | 2.84 | – |
| `regex_tasks__html_attributes` | 1.18 ms | 1.26 | 1.79 | 0.17 | 0.91 | 0.24 | 0.19 | – |
| `regex_tasks__html_comments` | 20.9 µs | 3.56 | 3.39 | 0.73 | 7.49 | 7.17 | 7.19 | – |
| `regex_tasks__hex_colors` | 18.1 µs | 4.04 | 20.36 | 0.67 | 0.90 | 54.57 | 0.69 | – |
| `regex_tasks__email_addresses` | 38.2 µs | 36.38 | 67.95 | 3.27 | 32.73 | 0.68 | 0.48 | – |
| `regex_tasks__emoji` | 108 µs | 2.94 | 4.08 | 0.80 | 5.15 | 0.79 | 0.58 | – |
| `regex_tasks__ascii_emoticons` | 66.3 µs | 1.70 | 2.08 | 0.33 | 0.85 | 0.67 | 0.52 | – |
| `regex_tasks__hashtags_mentions` | 72.3 µs | 2.38 | 2.06 | 0.56 | 1.03 | 1.31 | 0.89 | – |
| `regex_tasks__ipv4_addresses` | 193 µs | 1.44 | 2.99 | 1.12 | 5.89 | 20.50 | 5.23 | – |
| `regex_tasks__iso_timestamps` | 79.1 µs | 1.91 | 5.02 | 0.35 | 6.49 | 24.41 | 0.82 | – |
| `regex_tasks__log_keywords_ignorecase` | 89.7 µs | 2.67 | 4.50 | 0.85 | 1.90 | 22.12 | 0.54 | – |
| `regex_tasks__markdown_links` | 68.3 µs | 1.93 | 3.66 | 0.54 | 0.97 | 3.88 | 3.68 | – |
| `regex_tasks__semantic_versions` | 225 µs | 0.98 | 1.66 | 0.29 | 0.73 | 5.06 | 0.32 | – |
| `regex_tasks__json_strings` | 315 µs | 1.81 | 1.40 | 0.47 | 2.10 | 1.05 | 0.68 | – |
| `regex_tasks__csv_fields` | 186 µs | 2.09 | 1.51 | 0.44 | 1.73 | 1.13 | 0.84 | – |
| `regex_tasks__rfc5322_emails` | 3.38 ms | 0.70 | 1.72 | 0.09 | 1.42 | 0.04 | 0.04 | – |
| `regex_tasks__ipv6_addresses` | 1.82 ms | 0.57 | 1.50 | 0.38 | 2.45 | 0.07 | 0.06 | – |
| `regex_tasks__rfc3986_urls` | 91.2 µs | 1.57 | 1.99 | 0.76 | 2.42 | 18.92 | 0.50 | – |
| `regex_tasks__keyword_alternation_200` | 852 µs | 3.72 | 4.74 | 0.53 | 10.59 | 21.67 | 0.45 | – |
| `regex_tasks__csv_last_column_backtracking` | 142 µs | 1.06 | 1.29 | 0.26 | 0.53 | 0.75 | 1.15 | – |
| `regex_tasks__unicode_case_folding` | 151 µs | 3.28 | 1.00 | n/a | n/a | n/a | n/a | – |
| `general_regex__email_validation` | 4.11 µs | 2.26 | 2.47 | 0.40 | 1.31 | 0.34 | 0.25 | – |
| `general_regex__uuid_validation` | 3.85 µs | 3.17 | 3.25 | 0.41 | 0.83 | 0.39 | 0.32 | – |
| `general_regex__number_validation` | 4 µs | 2.44 | 2.10 | 0.42 | 2.82 | 0.28 | 0.20 | – |
| `general_regex__access_log_captures` | 21.7 µs | 1.12 | 1.57 | 0.26 | 0.75 | 1.60 | 1.18 | – |
| `general_regex__url_extraction` | 6.6 µs | 2.49 | 2.38 | 0.53 | 1.31 | 1.85 | 1.48 | – |
| `general_regex__unicode_words` | 33.4 µs | 1.68 | 1.18 | 0.38 | 1.05 | 0.95 | 0.62 | – |
| `general_regex__email_redaction` | 15.6 µs | 3.72 | 5.95 | 0.55 | 3.41 | 0.75 | 0.56 | – |
| `text_scanning__literal_50k` | 35.4 ns | 2.50 | 0.74 | 0.36 | 0.90 | 0.28 | 0.19 | – |
| `text_scanning__no_match_50k` | 957 ns | 5.64 | 5.47 | 1.22 | 0.80 | 0.99 | 0.99 | – |
| `text_scanning__field_extract_50k` | 38.4 ns | 2.49 | 1.13 | 0.50 | 1.30 | 0.88 | 0.83 | – |
| `text_scanning__timestamp_50k` | 67.8 ns | 1.93 | 1.57 | 0.43 | 0.88 | 0.65 | 0.59 | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |

