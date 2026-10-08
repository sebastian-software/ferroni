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

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 493 µs | 1.21 | 1.18 | 0.17 | 1.08 | 2.45 | – | – |
| `regex_tasks__html_attribute_values` | 837 µs | 1.04 | 1.09 | 0.18 | 0.86 | 2.13 | – | – |
| `regex_tasks__prices_lookbehind` | 208 µs | 1.49 | 2.04 | 0.23 | 0.89 | 7.76 | – | – |
| `regex_tasks__quoted_strings` | 273 µs | 1.16 | 1.24 | 0.23 | 1.33 | 4.21 | – | – |
| `regex_tasks__camel_case_words` | 701 µs | 1.72 | 1.40 | 0.50 | 0.90 | 2.55 | – | – |
| `regex_tasks__markdown_emphasis` | 181 µs | 0.98 | 1.52 | 0.23 | 0.98 | 9.52 | – | – |
| `regex_tasks__emoji_graphemes` | 157 µs | 2.07 | 3.33 | 0.57 | 3.94 | n/a | – | – |
| `regex_tasks__password_rules` | 18.2 µs | 0.90 | 1.52 | 0.16 | 0.99 | 0.50 | – | – |
| `regex_tasks__json_objects_recursive` | 266 µs | 1.24 | 2.44 | 0.17 | 2.45 | 2.50 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 54.2 µs | 2.00 | 2.98 | 0.31 | 2.24 | 15.92 | – | – |
| `regex_tasks__variable_lookbehind` | 1.05 ms | 1.21 | n/a | 0.15 | 0.65 | 1.69 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 22.3 µs | 1.20 | 1.30 | 0.38 | 0.97 | 3.01 | – | – |
| `oniguruma_features__subexp_call_balanced` | 32.4 µs | 1.06 | 1.37 | 0.32 | 1.86 | n/a | – | – |
| `oniguruma_features__absent_comments` | 26.9 µs | 0.80 | 0.70 | n/a | n/a | 1.55 | – | – |
| `oniguruma_features__conditional_brackets` | 41 µs | 1.35 | 1.47 | 0.35 | 1.10 | 2.95 | – | – |
| `oniguruma_features__backref_ignorecase` | 68.8 µs | 1.11 | 1.12 | 0.36 | 1.20 | 2.03 | – | – |
| `oniguruma_features__lookbehind_alternation` | 15.5 µs | 3.38 | 1.34 | 0.67 | 4.29 | 4.95 | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 32.6 ns | 2.22 | 0.87 | 0.50 | 1.23 | 0.37 | 0.28 | – |
| `single_pattern__quantifier_greedy` | 36.5 ns | 2.43 | 1.47 | 0.54 | 1.04 | 1.33 | 1.19 | – |
| `single_pattern__alternation_2_branch` | 39.8 ns | 1.60 | 0.67 | 0.39 | 1.13 | 0.96 | 0.78 | – |
| `single_pattern__alternation_10_branch` | 29.8 ns | 4.35 | 2.89 | 0.66 | 4.03 | 0.65 | 0.51 | – |
| `single_pattern__case_insensitive_phrase` | 58 ns | 1.70 | 0.93 | 0.27 | 0.69 | 0.84 | 0.77 | – |
| `single_pattern__named_capture_date` | 58.2 ns | 3.76 | 1.61 | 0.62 | 4.61 | 0.69 | 0.65 | – |
| `single_pattern__unicode_greek` | 55.9 ns | 2.30 | 2.59 | 0.47 | 2.84 | 0.89 | 0.82 | – |
| `single_pattern__lookaround_combined` | 54.2 ns | 3.14 | 1.01 | 0.47 | 3.76 | 3.53 | – | – |
| `single_pattern__backref_simple` | 57.4 ns | 1.35 | 0.94 | 0.34 | 0.85 | 1.30 | – | – |
| `text_scanning__regset_position_lead` | 63.1 ns | 2.92 | – | – | – | – | – | – |
| `compilation__literal` | 561 ns | 0.55 | 0.50 | 6.50 | 0.47 | 4.09 | 3.20 | – |
| `compilation__named_capture` | 2.6 µs | 1.20 | 1.50 | 1.87 | 0.18 | 59.94 | 55.85 | – |
| `compilation__lookbehind` | 1.31 µs | 0.36 | 0.28 | 2.47 | 0.16 | 113.02 | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 11.8 ms | 0.81 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 8.95 ms | 1.57 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 352 µs | 0.52 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.16 µs | 65.18 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 143 µs | 76.59 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 4.15 µs | 15.18 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 649 µs | 3.59 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 77.4 µs | 31.16 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 78.7 µs | 10.42 | – | – | – | – | – | – |
| `cpp_scanner__document` | 12.4 ms | 2.39 | n/a | n/a | n/a | 68.13 | – | 2.62 |
| `cpp_scanner__group_78` | 6.03 ms | 3.03 | n/a | n/a | n/a | 95.12 | – | 2.95 |
| `java_scanner__document` | 1.98 ms | 4.49 | 15.70 | 5.31 | 16.99 | 40.64 | – | 3.30 |
| `java_scanner__group_15` | 488 µs | 3.64 | 12.86 | 3.65 | 13.55 | 32.76 | – | 3.36 |
| `scss_scanner__document` | 2.27 ms | 15.42 | 9.12 | n/a | n/a | 49.28 | – | 1.96 |
| `scss_scanner__group_8` | 939 µs | 17.78 | 9.66 | n/a | n/a | 54.45 | – | 2.81 |
| `c_scanner__document` | 4.24 ms | 4.04 | 10.57 | 4.31 | 11.06 | 55.76 | – | 2.52 |
| `c_scanner__group_12` | 938 µs | 4.49 | 14.86 | 3.60 | 16.06 | 76.37 | – | 2.75 |
| `php_scanner__document` | 5.72 ms | 3.09 | 8.15 | 1.68 | 3.33 | 14.76 | – | 0.99 |
| `php_scanner__group_13` | 639 µs | 5.92 | 21.30 | 3.36 | 8.12 | 38.05 | – | 2.34 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `cpp_scanner/document_onigmo` | scanner 0, pattern "^\\s+{1,0}(//[!/]+)": Onigmo error -202 |
| `cpp_scanner/document_pcre2` | scanner 0, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\A\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset 89: qua… |
| `cpp_scanner/document_pcre2_jit` | scanner 0, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\A\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset 89: qua… |
| `cpp_scanner/group_78_onigmo` | scanner 78, pattern "^\\s+{1,0}(//[!/]+)": Onigmo error -202 |
| `cpp_scanner/group_78_pcre2` | scanner 78, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\\u{ffff}\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset… |
| `cpp_scanner/group_78_pcre2_jit` | scanner 78, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\\u{ffff}\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset… |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

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

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 490 µs | 0.95 | 1.44 | 0.25 | 1.33 | 2.96 | – | – |
| `regex_tasks__html_attribute_values` | 987 µs | 0.96 | 1.18 | 0.20 | 0.75 | 1.99 | – | – |
| `regex_tasks__prices_lookbehind` | 225 µs | 1.64 | 2.71 | 0.24 | 1.09 | 7.84 | – | – |
| `regex_tasks__quoted_strings` | 305 µs | 1.16 | 1.44 | 0.21 | 1.27 | 4.52 | – | – |
| `regex_tasks__camel_case_words` | 736 µs | 2.30 | 1.48 | 0.44 | 0.89 | 2.70 | – | – |
| `regex_tasks__markdown_emphasis` | 193 µs | 1.21 | 1.76 | 0.29 | 0.96 | 10.79 | – | – |
| `regex_tasks__emoji_graphemes` | 182 µs | 2.31 | 3.42 | 0.55 | 4.18 | n/a | – | – |
| `regex_tasks__password_rules` | 19.6 µs | 1.09 | 1.66 | 0.20 | 1.27 | 0.54 | – | – |
| `regex_tasks__json_objects_recursive` | 321 µs | 1.41 | 2.64 | 0.26 | 2.76 | 2.49 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 64.7 µs | 2.18 | 3.14 | 0.38 | 2.74 | 16.14 | – | – |
| `regex_tasks__variable_lookbehind` | 1.26 ms | 1.10 | n/a | 0.17 | 0.67 | 1.47 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 23.7 µs | 1.55 | 1.13 | 0.32 | 1.13 | 3.05 | – | – |
| `oniguruma_features__subexp_call_balanced` | 33.1 µs | 1.33 | 1.61 | 0.31 | 1.76 | n/a | – | – |
| `oniguruma_features__absent_comments` | 27.4 µs | 1.22 | 0.71 | n/a | n/a | 1.59 | – | – |
| `oniguruma_features__conditional_brackets` | 41 µs | 1.36 | 1.50 | 0.35 | 1.20 | 3.42 | – | – |
| `oniguruma_features__backref_ignorecase` | 77.8 µs | 0.98 | 1.05 | 0.27 | 0.98 | 1.79 | – | – |
| `oniguruma_features__lookbehind_alternation` | 15.6 µs | 3.62 | 1.29 | 0.73 | 5.94 | 4.92 | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 47.9 ns | 2.31 | 0.70 | 0.34 | 0.82 | 0.22 | 0.17 | – |
| `single_pattern__quantifier_greedy` | 41.4 ns | 3.45 | 1.68 | 0.50 | 1.05 | 1.25 | 1.01 | – |
| `single_pattern__alternation_2_branch` | 45.2 ns | 2.54 | 0.81 | 0.37 | 1.24 | 0.82 | 0.70 | – |
| `single_pattern__alternation_10_branch` | 27.9 ns | 5.64 | 4.49 | 0.72 | 7.20 | 0.67 | 0.45 | – |
| `single_pattern__case_insensitive_phrase` | 62.4 ns | 2.13 | 0.86 | 0.30 | 0.80 | 0.66 | 0.65 | – |
| `single_pattern__named_capture_date` | 65.7 ns | 3.43 | 1.83 | 0.72 | 4.69 | 0.55 | 0.48 | – |
| `single_pattern__unicode_greek` | 65.4 ns | 2.70 | 2.52 | 0.54 | 2.19 | 0.64 | 0.61 | – |
| `single_pattern__lookaround_combined` | 59.2 ns | 3.55 | 1.01 | 0.45 | 4.43 | 3.71 | – | – |
| `single_pattern__backref_simple` | 61.7 ns | 2.04 | 1.02 | 0.33 | 0.86 | 1.38 | – | – |
| `text_scanning__regset_position_lead` | 77.9 ns | 3.81 | – | – | – | – | – | – |
| `compilation__literal` | 608 ns | 0.63 | 0.59 | 3.49 | 0.54 | 4.38 | 3.29 | – |
| `compilation__named_capture` | 3.04 µs | 1.58 | 1.71 | 1.15 | 0.19 | 55.54 | 53.91 | – |
| `compilation__lookbehind` | 1.3 µs | 0.33 | 0.27 | 1.66 | 0.19 | 113.05 | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 12.2 ms | 1.19 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 8.32 ms | 1.93 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 276 µs | 0.55 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.04 µs | 72.67 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 165 µs | 51.08 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 3.96 µs | 13.74 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 520 µs | 4.25 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 80.7 µs | 25.93 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 78.5 µs | 8.59 | – | – | – | – | – | – |
| `cpp_scanner__document` | 11.9 ms | 2.47 | n/a | n/a | n/a | 81.73 | – | 2.72 |
| `cpp_scanner__group_78` | 5.17 ms | 3.15 | n/a | n/a | n/a | 133.78 | – | 3.34 |
| `java_scanner__document` | 1.95 ms | 4.50 | 17.15 | 5.63 | 18.47 | 42.58 | – | 3.51 |
| `java_scanner__group_15` | 439 µs | 3.61 | 13.25 | 4.12 | 14.05 | 33.19 | – | 3.58 |
| `scss_scanner__document` | 2.12 ms | 14.92 | 9.76 | n/a | n/a | 56.25 | – | 1.72 |
| `scss_scanner__group_8` | 852 µs | 19.37 | 10.80 | n/a | n/a | 65.69 | – | 2.54 |
| `c_scanner__document` | 3.77 ms | 4.16 | 12.72 | 3.56 | 14.09 | 72.41 | – | 2.54 |
| `c_scanner__group_12` | 884 µs | 4.72 | 17.24 | 3.52 | 20.72 | 89.28 | – | 2.65 |
| `php_scanner__document` | 5.51 ms | 2.81 | 8.12 | 1.37 | 3.26 | 14.41 | – | 0.87 |
| `php_scanner__group_13` | 644 µs | 5.57 | 21.15 | 2.89 | 8.02 | 36.10 | – | 1.94 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `cpp_scanner/document_onigmo` | scanner 0, pattern "^\\s+{1,0}(//[!/]+)": Onigmo error -202 |
| `cpp_scanner/document_pcre2` | scanner 0, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\A\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset 89: qua… |
| `cpp_scanner/document_pcre2_jit` | scanner 0, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\A\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset 89: qua… |
| `cpp_scanner/group_78_onigmo` | scanner 78, pattern "^\\s+{1,0}(//[!/]+)": Onigmo error -202 |
| `cpp_scanner/group_78_pcre2` | scanner 78, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\\u{ffff}\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset… |
| `cpp_scanner/group_78_pcre2_jit` | scanner 78, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\\u{ffff}\|\\Z)(#)\\s+{0,1}pragma\\s+mark)\\s+(.*)": PCRE2 at offset… |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

