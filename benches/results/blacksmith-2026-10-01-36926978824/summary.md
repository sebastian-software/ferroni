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

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 609 µs | 1.02 | 1.29 | 0.19 | 1.14 | 2.43 | – | – |
| `regex_tasks__html_attribute_values` | 1.08 ms | 1.15 | 1.06 | 0.17 | 0.83 | 2.09 | – | – |
| `regex_tasks__prices_lookbehind` | 314 µs | 1.30 | 2.09 | 0.25 | 1.05 | 8.92 | – | – |
| `regex_tasks__quoted_strings` | 325 µs | 1.39 | 1.15 | 0.25 | 1.53 | 3.94 | – | – |
| `regex_tasks__camel_case_words` | 843 µs | 1.83 | 1.35 | 0.47 | 0.83 | 2.19 | – | – |
| `regex_tasks__markdown_emphasis` | 210 µs | 1.03 | 1.41 | 0.24 | 0.93 | 9.12 | – | – |
| `regex_tasks__emoji_graphemes` | 185 µs | 1.84 | 2.93 | 0.54 | 3.75 | n/a | – | – |
| `regex_tasks__password_rules` | 23.6 µs | 0.85 | 1.39 | 0.18 | 1.01 | 0.47 | – | – |
| `regex_tasks__json_objects_recursive` | 383 µs | 1.29 | 2.00 | 0.15 | 2.45 | 2.17 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 84.1 µs | 1.82 | 2.29 | 0.31 | 1.97 | 14.93 | – | – |
| `regex_tasks__variable_lookbehind` | 1.45 ms | 1.18 | n/a | 0.14 | 0.60 | 1.45 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 26.1 µs | 1.42 | 1.23 | 0.35 | 1.08 | 2.86 | – | – |
| `oniguruma_features__subexp_call_balanced` | 36.3 µs | 1.09 | 1.24 | 0.33 | 1.82 | n/a | – | – |
| `oniguruma_features__absent_comments` | 32.3 µs | 0.77 | 0.63 | n/a | n/a | 1.31 | – | – |
| `oniguruma_features__conditional_brackets` | 53 µs | 1.12 | 1.19 | 0.29 | 0.95 | 2.50 | – | – |
| `oniguruma_features__backref_ignorecase` | 76.8 µs | 1.01 | 1.06 | 0.32 | 1.16 | 1.89 | – | – |
| `oniguruma_features__lookbehind_alternation` | 20.5 µs | 2.98 | 1.07 | 0.58 | 3.49 | 3.57 | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 77.9 ns | 1.84 | 0.82 | 0.41 | 1.16 | 0.30 | 0.23 | – |
| `single_pattern__quantifier_greedy` | 67.7 ns | 2.37 | 1.75 | 0.59 | 1.20 | 1.35 | 1.42 | – |
| `single_pattern__alternation_2_branch` | 72.9 ns | 1.98 | 0.63 | 0.42 | 1.28 | 0.96 | 0.97 | – |
| `single_pattern__alternation_10_branch` | 48.1 ns | 4.69 | 2.91 | 0.78 | 4.50 | 0.72 | 0.54 | – |
| `single_pattern__case_insensitive_phrase` | 127 ns | 1.68 | 0.79 | 0.24 | 0.64 | 0.71 | 0.63 | – |
| `single_pattern__named_capture_date` | 128 ns | 2.82 | 1.38 | 0.55 | 3.66 | 0.50 | 0.49 | – |
| `single_pattern__unicode_greek` | 91.5 ns | 2.67 | 2.72 | 0.51 | 3.23 | 1.02 | 1.01 | – |
| `single_pattern__lookaround_combined` | 98.6 ns | 2.79 | 0.97 | 0.40 | 3.21 | 2.95 | – | – |
| `single_pattern__backref_simple` | 80.5 ns | 1.62 | 1.09 | 0.37 | 0.95 | 1.48 | – | – |
| `text_scanning__regset_position_lead` | 101 ns | 2.74 | – | – | – | – | – | – |
| `compilation__literal` | 830 ns | 0.60 | 0.52 | 6.22 | 0.43 | 4.21 | 3.06 | – |
| `compilation__named_capture` | 3.44 µs | 1.39 | 1.62 | 2.05 | 0.22 | 63.01 | 64.22 | – |
| `compilation__lookbehind` | 1.76 µs | 0.39 | 0.28 | 2.67 | 0.18 | 123.81 | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 11.5 ms | 0.82 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 7.61 ms | 1.71 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 301 µs | 0.58 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.15 µs | 62.44 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 128 µs | 76.64 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 4.58 µs | 13.92 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 699 µs | 3.34 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 73.5 µs | 31.54 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 79.2 µs | 10.31 | – | – | – | – | – | – |
| `cpp_scanner__document` | 12.6 ms | 2.19 | n/a | n/a | n/a | 74.64 | – | 2.58 |
| `cpp_scanner__group_78` | 5.87 ms | 2.94 | n/a | n/a | n/a | 108.25 | – | 3.14 |
| `java_scanner__document` | 2.64 ms | 3.66 | 11.35 | 4.12 | 12.48 | 30.95 | – | 3.06 |
| `java_scanner__group_15` | 466 µs | 3.41 | 12.70 | 3.75 | 12.81 | 33.35 | – | 3.01 |
| `scss_scanner__document` | 1.97 ms | 16.74 | 10.27 | n/a | n/a | 60.54 | – | 2.06 |
| `scss_scanner__group_8` | 840 µs | 17.75 | 10.33 | n/a | n/a | 57.94 | – | 2.84 |
| `c_scanner__document` | 3.83 ms | 4.15 | 12.62 | 4.72 | 15.20 | 83.26 | – | 2.44 |
| `c_scanner__group_12` | 932 µs | 4.29 | 15.24 | 3.59 | 17.19 | 88.41 | – | 3.34 |
| `php_scanner__document` | 5.18 ms | 3.16 | 8.23 | 1.73 | 3.51 | 17.88 | – | 1.00 |
| `php_scanner__group_13` | 604 µs | 5.71 | 22.12 | 3.34 | 8.19 | 42.31 | – | 2.24 |

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

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 456 µs | 1.13 | 1.54 | 0.27 | 1.36 | 3.27 | – | – |
| `regex_tasks__html_attribute_values` | 944 µs | 1.06 | 1.19 | 0.21 | 0.80 | 2.11 | – | – |
| `regex_tasks__prices_lookbehind` | 223 µs | 1.66 | 2.65 | 0.26 | 1.12 | 8.06 | – | – |
| `regex_tasks__quoted_strings` | 295 µs | 1.22 | 1.36 | 0.23 | 1.36 | 4.42 | – | – |
| `regex_tasks__camel_case_words` | 743 µs | 2.01 | 1.48 | 0.38 | 0.79 | 2.43 | – | – |
| `regex_tasks__markdown_emphasis` | 191 µs | 1.11 | 1.74 | 0.26 | 0.90 | 10.83 | – | – |
| `regex_tasks__emoji_graphemes` | 170 µs | 2.34 | 3.48 | 0.56 | 4.28 | n/a | – | – |
| `regex_tasks__password_rules` | 18.1 µs | 1.16 | 1.54 | 0.21 | 1.29 | 0.52 | – | – |
| `regex_tasks__json_objects_recursive` | 283 µs | 1.48 | 2.95 | 0.29 | 3.15 | 2.66 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 64.6 µs | 2.30 | 2.92 | 0.40 | 2.78 | 15.63 | – | – |
| `regex_tasks__variable_lookbehind` | 1.12 ms | 1.14 | n/a | 0.17 | 0.68 | 1.55 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 24 µs | 1.51 | 1.11 | 0.33 | 1.16 | 3.03 | – | – |
| `oniguruma_features__subexp_call_balanced` | 32.8 µs | 1.38 | 1.59 | 0.32 | 1.71 | n/a | – | – |
| `oniguruma_features__absent_comments` | 27.8 µs | 1.17 | 0.66 | n/a | n/a | 1.51 | – | – |
| `oniguruma_features__conditional_brackets` | 48.1 µs | 1.21 | 1.25 | 0.30 | 1.03 | 2.91 | – | – |
| `oniguruma_features__backref_ignorecase` | 75.1 µs | 0.98 | 1.09 | 0.27 | 0.96 | 1.83 | – | – |
| `oniguruma_features__lookbehind_alternation` | 18 µs | 3.13 | 1.19 | 0.61 | 4.97 | 4.33 | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 35.7 ns | 2.83 | 1.06 | 0.43 | 1.12 | 0.31 | 0.21 | – |
| `single_pattern__quantifier_greedy` | 44.3 ns | 3.43 | 1.50 | 0.47 | 0.94 | 1.01 | 0.95 | – |
| `single_pattern__alternation_2_branch` | 40.7 ns | 2.63 | 0.83 | 0.38 | 1.27 | 0.78 | 0.76 | – |
| `single_pattern__alternation_10_branch` | 25.2 ns | 6.47 | 4.61 | 0.81 | 7.89 | 0.64 | 0.51 | – |
| `single_pattern__case_insensitive_phrase` | 57.7 ns | 2.35 | 0.89 | 0.28 | 0.82 | 0.66 | 0.61 | – |
| `single_pattern__named_capture_date` | 57.7 ns | 3.73 | 2.08 | 0.82 | 4.73 | 0.58 | 0.47 | – |
| `single_pattern__unicode_greek` | 61.7 ns | 2.71 | 2.74 | 0.52 | 2.35 | 0.72 | 0.57 | – |
| `single_pattern__lookaround_combined` | 56.7 ns | 3.60 | 1.08 | 0.53 | 4.99 | 3.93 | – | – |
| `single_pattern__backref_simple` | 56.6 ns | 2.35 | 1.09 | 0.33 | 0.89 | 1.40 | – | – |
| `text_scanning__regset_position_lead` | 72.1 ns | 3.82 | – | – | – | – | – | – |
| `compilation__literal` | 535 ns | 0.62 | 0.59 | 3.55 | 0.59 | 4.65 | 3.26 | – |
| `compilation__named_capture` | 2.68 µs | 1.58 | 1.86 | 1.19 | 0.19 | 55.92 | 55.41 | – |
| `compilation__lookbehind` | 1.13 µs | 0.38 | 0.30 | 1.73 | 0.21 | 121.96 | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 10.9 ms | 1.32 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 7.17 ms | 2.27 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 279 µs | 0.55 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.16 µs | 70.51 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 153 µs | 52.79 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 3.9 µs | 15.05 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 579 µs | 3.95 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 67.3 µs | 28.78 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 77 µs | 9.50 | – | – | – | – | – | – |
| `cpp_scanner__document` | 11.3 ms | 2.54 | n/a | n/a | n/a | 90.92 | – | 3.22 |
| `cpp_scanner__group_78` | 5.11 ms | 3.25 | n/a | n/a | n/a | 122.13 | – | 3.41 |
| `java_scanner__document` | 1.79 ms | 4.45 | 18.33 | 5.80 | 18.37 | 43.29 | – | 3.43 |
| `java_scanner__group_15` | 478 µs | 3.58 | 12.77 | 3.97 | 13.85 | 32.79 | – | 3.29 |
| `scss_scanner__document` | 2.2 ms | 14.77 | 9.20 | n/a | n/a | 54.94 | – | 1.63 |
| `scss_scanner__group_8` | 860 µs | 20.81 | 10.30 | n/a | n/a | 65.04 | – | 2.61 |
| `c_scanner__document` | 3.18 ms | 4.14 | 13.21 | 3.40 | 14.99 | 73.97 | – | 2.67 |
| `c_scanner__group_12` | 735 µs | 5.03 | 18.59 | 3.71 | 22.66 | 98.19 | – | 2.95 |
| `php_scanner__document` | 5.13 ms | 2.71 | 8.07 | 1.33 | 3.25 | 13.57 | – | 0.78 |
| `php_scanner__group_13` | 580 µs | 5.64 | 21.92 | 2.98 | 8.17 | 36.66 | – | 1.95 |

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

