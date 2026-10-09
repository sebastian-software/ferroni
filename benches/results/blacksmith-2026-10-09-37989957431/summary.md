### Engine comparison: linux-x86-64 (AMD EPYC)

Source `a38768bc8012`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 452 µs | 1.26 | 1.36 | 0.34 | 0.72 | 2.39 | 2.02 | – | 1.78 | – |
| `regex_tasks__html_attributes` | 546 µs | 3.07 | 3.72 | 0.32 | 2.16 | 0.62 | 0.64 | – | 0.46 | – |
| `regex_tasks__html_comments` | 25.5 µs | 2.90 | 3.09 | 0.75 | 5.49 | 5.59 | 5.77 | – | 5.35 | – |
| `regex_tasks__hex_colors` | 22.3 µs | 6.41 | 14.91 | 0.68 | 0.99 | 51.05 | 2.68 | – | 0.86 | – |
| `regex_tasks__email_addresses` | 44.8 µs | 32.71 | 49.51 | 1.76 | 26.17 | 0.73 | 0.73 | – | 0.51 | – |
| `regex_tasks__emoji` | 134 µs | 2.46 | 3.35 | 0.72 | 4.34 | 0.73 | 0.71 | – | 0.53 | – |
| `regex_tasks__ascii_emoticons` | 77.4 µs | 1.25 | 1.76 | 0.34 | 0.76 | 1.13 | 1.10 | – | 0.89 | – |
| `regex_tasks__hashtags_mentions` | 80.6 µs | 2.58 | 2.58 | 0.59 | 1.34 | 1.72 | 1.84 | – | 1.02 | – |
| `regex_tasks__ipv4_addresses` | 256 µs | 1.45 | 2.76 | 0.93 | 5.47 | 21.33 | 6.07 | – | 5.38 | – |
| `regex_tasks__iso_timestamps` | 141 µs | 1.41 | 3.02 | 0.29 | 6.00 | 25.00 | 1.95 | – | 0.65 | – |
| `regex_tasks__log_keywords_ignorecase` | 112 µs | 4.30 | 5.35 | 1.05 | 2.63 | 39.17 | 1.26 | – | 0.86 | – |
| `regex_tasks__markdown_links` | 84.8 µs | 2.65 | 3.80 | 0.65 | 1.38 | 3.85 | 3.92 | – | 3.57 | – |
| `regex_tasks__semantic_versions` | 347 µs | 1.07 | 1.11 | 0.20 | 0.58 | 4.22 | 0.45 | – | 0.38 | – |
| `regex_tasks__json_strings` | 371 µs | 1.73 | 1.45 | 0.58 | 1.87 | 1.37 | 1.33 | – | 0.92 | – |
| `regex_tasks__csv_fields` | 222 µs | 1.82 | 1.58 | 0.51 | 1.54 | 1.42 | 1.50 | – | 1.00 | – |
| `regex_tasks__rfc5322_emails` | 4.28 ms | 0.67 | 1.76 | 0.06 | 0.98 | 0.03 | 0.03 | – | 0.03 | – |
| `regex_tasks__ipv6_addresses` | 1.75 ms | 0.64 | 1.47 | 0.38 | 2.25 | 0.07 | 0.07 | – | 0.07 | – |
| `regex_tasks__rfc3986_urls` | 80 µs | 1.86 | 2.30 | 0.71 | 2.11 | 40.49 | 1.13 | – | 0.63 | – |
| `regex_tasks__keyword_alternation_200` | 1.07 ms | 4.30 | 4.46 | 0.65 | 8.51 | 22.23 | 4.13 | – | 0.59 | – |
| `regex_tasks__csv_last_column_backtracking` | 257 µs | 0.89 | 1.04 | 0.16 | 0.42 | 0.60 | 0.94 | – | 0.78 | – |
| `regex_tasks__unicode_case_folding` | 235 µs | 3.05 | 0.81 | n/a | n/a | n/a | n/a | – | n/a | – |
| `general_regex__email_validation` | 8.34 µs | 0.96 | 1.17 | 0.18 | 0.59 | 0.20 | 0.22 | – | 0.15 | – |
| `general_regex__uuid_validation` | 4.84 µs | 2.16 | 3.20 | 0.36 | 0.73 | 0.46 | 0.47 | – | 0.33 | – |
| `general_regex__number_validation` | 4.88 µs | 1.56 | 1.87 | 0.39 | 1.86 | 0.29 | 0.31 | – | 0.17 | – |
| `general_regex__access_log_captures` | 26.2 µs | 1.04 | 1.21 | 0.18 | 0.52 | 1.18 | 1.39 | – | 1.15 | – |
| `general_regex__url_extraction` | 6.34 µs | 2.16 | 2.56 | 0.62 | 1.23 | 2.06 | 2.05 | – | 1.56 | – |
| `general_regex__unicode_words` | 45.5 µs | 1.68 | 1.44 | 0.48 | 1.03 | 1.28 | 1.23 | – | 0.87 | – |
| `general_regex__email_redaction` | 28.1 µs | 3.21 | 5.00 | 0.42 | 2.85 | 0.80 | 0.84 | – | 0.58 | – |
| `text_scanning__literal_50k` | 43.9 ns | 1.99 | 0.61 | 0.43 | 0.97 | 0.32 | 0.33 | – | 0.23 | – |
| `text_scanning__no_match_50k` | 753 ns | 9.91 | 9.69 | 1.37 | 0.36 | 1.08 | 1.09 | – | 0.95 | – |
| `text_scanning__field_extract_50k` | 53.9 ns | 1.53 | 1.03 | 0.51 | 1.18 | 1.01 | 0.95 | – | 0.89 | – |
| `text_scanning__timestamp_50k` | 68 ns | 1.70 | 2.15 | 0.53 | 1.12 | 0.91 | 0.92 | – | 0.84 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 525 µs | 1.15 | 1.26 | 0.19 | 1.28 | 2.55 | 3.36 | – | – | – |
| `regex_tasks__html_attribute_values` | 950 µs | 0.92 | 1.09 | 0.15 | 0.81 | 2.09 | 0.74 | – | – | – |
| `regex_tasks__prices_lookbehind` | 240 µs | 1.48 | 2.09 | 0.23 | 0.87 | 7.81 | 1.93 | – | – | – |
| `regex_tasks__quoted_strings` | 346 µs | 1.12 | 1.10 | 0.21 | 1.25 | 3.79 | 2.15 | – | – | – |
| `regex_tasks__camel_case_words` | 773 µs | 1.78 | 1.50 | 0.52 | 0.94 | 2.42 | 1.75 | – | – | – |
| `regex_tasks__markdown_emphasis` | 206 µs | 0.90 | 1.45 | 0.22 | 0.95 | 9.33 | 1.30 | – | – | – |
| `regex_tasks__emoji_graphemes` | 183 µs | 2.29 | 3.35 | 0.61 | 3.97 | n/a | n/a | – | – | – |
| `regex_tasks__password_rules` | 20.6 µs | 0.91 | 1.65 | 0.17 | 1.18 | 0.57 | 0.54 | – | – | – |
| `regex_tasks__json_objects_recursive` | 372 µs | 1.05 | 2.20 | 0.15 | 2.20 | 2.16 | n/a | – | – | – |
| `regex_tasks__html_nested_divs_recursive` | 68.8 µs | 1.95 | 2.77 | 0.29 | 2.15 | 14.19 | 1676.65 | – | – | – |
| `regex_tasks__variable_lookbehind` | 1.25 ms | 1.12 | n/a | 0.14 | 0.60 | 1.64 | 0.48 | – | – | – |
| `oniguruma_features__atomic_possessive_strings` | 25.5 µs | 1.24 | 1.16 | 0.30 | 0.91 | 2.85 | 2.46 | – | – | – |
| `oniguruma_features__subexp_call_balanced` | 35.1 µs | 1.01 | 1.40 | 0.33 | 1.92 | n/a | n/a | – | – | – |
| `oniguruma_features__absent_comments` | 29.2 µs | 0.78 | 0.68 | n/a | n/a | 1.55 | 17.22 | – | – | – |
| `oniguruma_features__conditional_brackets` | 44.6 µs | 1.43 | 1.56 | 0.38 | 1.38 | 3.10 | n/a | – | – | – |
| `oniguruma_features__backref_ignorecase` | 84.4 µs | 1.07 | 1.03 | 0.31 | 1.16 | 1.87 | 1.59 | – | – | – |
| `oniguruma_features__lookbehind_alternation` | 17.8 µs | 3.45 | 1.27 | 0.66 | 4.06 | 4.90 | 1.66 | – | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 37 ns | 2.25 | 0.75 | 0.46 | 1.08 | 0.36 | 0.35 | – | 0.28 | – |
| `single_pattern__quantifier_greedy` | 37.6 ns | 2.50 | 1.38 | 0.56 | 1.02 | 1.32 | 1.37 | – | 1.19 | – |
| `single_pattern__alternation_2_branch` | 40.8 ns | 1.74 | 0.75 | 0.40 | 1.17 | 1.18 | 1.12 | – | 0.84 | – |
| `single_pattern__alternation_10_branch` | 33.3 ns | 5.11 | 3.38 | 0.72 | 5.31 | 0.69 | 0.66 | – | 0.53 | – |
| `single_pattern__case_insensitive_phrase` | 94.4 ns | 1.53 | 0.80 | 0.23 | 0.58 | 0.67 | 0.72 | – | 0.60 | – |
| `single_pattern__named_capture_date` | 97.2 ns | 2.89 | 1.28 | 0.48 | 3.52 | 0.52 | 0.56 | – | 0.51 | – |
| `single_pattern__unicode_greek` | 112 ns | 1.97 | 2.08 | 0.38 | 1.88 | 0.66 | 0.61 | – | 0.62 | – |
| `single_pattern__lookaround_combined` | 86.1 ns | 3.09 | 0.95 | 0.40 | 3.47 | 3.54 | 1.59 | – | – | – |
| `single_pattern__backref_simple` | 86.7 ns | 1.58 | 1.10 | 0.37 | 0.85 | 1.53 | 1.96 | – | – | – |
| `text_scanning__regset_position_lead` | 103 ns | 2.68 | – | – | – | – | – | – | – | – |
| `compilation__literal` | 861 ns | 0.56 | 0.52 | 8.77 | 0.55 | 4.30 | 4.20 | – | 3.23 | – |
| `compilation__named_capture` | 3.88 µs | 1.16 | 1.09 | 2.26 | 0.18 | 49.85 | 43.75 | – | 51.75 | – |
| `compilation__lookbehind` | 1.96 µs | 0.27 | 0.19 | 1.96 | 0.16 | 95.70 | 215.81 | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 25 ms | 0.53 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 14.6 ms | 0.99 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 596 µs | 0.35 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 7.9 µs | 18.62 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 16 ms | 0.67 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.86 µs | 21.62 | – | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 105 µs | 22.40 | – | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 13.4 ms | 0.21 | – | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 48.5 µs | 19.43 | – | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 4.45 ms | 8.36 | n/a | n/a | n/a | 262.60 | 30.15 | 2.31 | – | 10.21 |
| `cpp_scanner__group_78` | 1.25 ms | 14.20 | n/a | n/a | n/a | 508.55 | 25.99 | 2.88 | – | 15.35 |
| `java_scanner__document` | 1.14 ms | 8.56 | 31.72 | 9.76 | 28.77 | 78.35 | 10.93 | 1.59 | – | 6.69 |
| `java_scanner__group_15` | 137 µs | 12.66 | 44.26 | 13.14 | 43.18 | 104.91 | 13.32 | 1.09 | – | 11.98 |
| `scss_scanner__document` | 1.16 ms | 29.39 | 18.61 | n/a | n/a | 121.05 | 10.58 | 1.83 | – | 3.69 |
| `scss_scanner__group_8` | 610 µs | 28.76 | 18.00 | n/a | n/a | 106.97 | 11.51 | 1.66 | – | 6.11 |
| `c_scanner__document` | 1.13 ms | 17.00 | 38.54 | 22.68 | 43.70 | 321.70 | 27.76 | 1.97 | – | 10.49 |
| `c_scanner__group_12` | 242 µs | 18.16 | 60.35 | 13.50 | 71.70 | 333.43 | 26.85 | 3.73 | – | 11.25 |
| `php_scanner__document` | 1.32 ms | 13.78 | 33.08 | 7.70 | 14.53 | 65.89 | 14.41 | 1.21 | – | 4.31 |
| `php_scanner__group_13` | 173 µs | 21.01 | 84.29 | 11.46 | 29.01 | 137.57 | 25.23 | 2.60 | – | 8.31 |

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
| `oniguruma_features/fancy_regex_seek/conditional_brackets` | match 1: [(68, 87), (68, 69)], Oniguruma [(26, 41), (-1, -1)] |
| `oniguruma_features/fancy_regex_seek/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex_seek/json_objects_recursive` | match 0: [(136, 152)], Oniguruma [(4, 208)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
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

Source `a38768bc8012`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 297 µs | 1.68 | 1.86 | 0.46 | 0.92 | 2.88 | 3.93 | – | 3.75 | – |
| `regex_tasks__html_attributes` | 489 µs | 3.01 | 4.06 | 0.38 | 2.10 | 0.56 | 0.57 | – | 0.43 | – |
| `regex_tasks__html_comments` | 18.4 µs | 3.48 | 3.28 | 0.66 | 6.68 | 6.68 | 6.63 | – | 6.52 | – |
| `regex_tasks__hex_colors` | 15.5 µs | 4.16 | 23.73 | 0.69 | 1.01 | 63.32 | 2.70 | – | 0.71 | – |
| `regex_tasks__email_addresses` | 37.1 µs | 33.14 | 61.41 | 2.99 | 30.95 | 0.65 | 0.64 | – | 0.42 | – |
| `regex_tasks__emoji` | 104 µs | 2.89 | 3.87 | 0.75 | 4.85 | 0.75 | 0.75 | – | 0.54 | – |
| `regex_tasks__ascii_emoticons` | 60.7 µs | 1.74 | 2.08 | 0.32 | 0.83 | 0.67 | 0.66 | – | 0.52 | – |
| `regex_tasks__hashtags_mentions` | 63.7 µs | 2.55 | 2.07 | 0.56 | 1.04 | 1.42 | 1.43 | – | 0.89 | – |
| `regex_tasks__ipv4_addresses` | 170 µs | 1.48 | 3.06 | 1.13 | 5.95 | 20.93 | 5.95 | – | 5.25 | – |
| `regex_tasks__iso_timestamps` | 68.5 µs | 2.06 | 4.91 | 0.38 | 6.72 | 24.79 | 2.35 | – | 0.85 | – |
| `regex_tasks__log_keywords_ignorecase` | 79 µs | 2.87 | 5.06 | 0.96 | 2.17 | 25.82 | 0.85 | – | 0.56 | – |
| `regex_tasks__markdown_links` | 43.7 µs | 3.12 | 5.73 | 0.83 | 1.54 | 5.99 | 5.95 | – | 5.63 | – |
| `regex_tasks__semantic_versions` | 224 µs | 1.22 | 1.66 | 0.28 | 0.74 | 5.07 | 0.59 | – | 0.33 | – |
| `regex_tasks__json_strings` | 300 µs | 2.03 | 1.47 | 0.50 | 2.20 | 1.25 | 1.23 | – | 0.70 | – |
| `regex_tasks__csv_fields` | 182 µs | 2.10 | 1.53 | 0.45 | 1.75 | 1.14 | 1.15 | – | 0.85 | – |
| `regex_tasks__rfc5322_emails` | 3.14 ms | 0.75 | 1.86 | 0.10 | 1.53 | 0.04 | 0.04 | – | 0.04 | – |
| `regex_tasks__ipv6_addresses` | 1.7 ms | 0.61 | 1.40 | 0.41 | 2.63 | 0.07 | 0.07 | – | 0.06 | – |
| `regex_tasks__rfc3986_urls` | 76.7 µs | 1.66 | 1.99 | 0.73 | 2.35 | 19.08 | 0.87 | – | 0.52 | – |
| `regex_tasks__keyword_alternation_200` | 742 µs | 3.94 | 4.96 | 0.56 | 10.99 | 22.36 | 4.08 | – | 0.46 | – |
| `regex_tasks__csv_last_column_backtracking` | 118 µs | 1.27 | 1.45 | 0.29 | 0.62 | 0.85 | 1.52 | – | 1.24 | – |
| `regex_tasks__unicode_case_folding` | 135 µs | 3.40 | 1.06 | n/a | n/a | n/a | n/a | – | n/a | – |
| `general_regex__email_validation` | 4.22 µs | 1.95 | 2.08 | 0.35 | 1.18 | 0.30 | 0.30 | – | 0.22 | – |
| `general_regex__uuid_validation` | 3.38 µs | 3.08 | 3.69 | 0.41 | 0.87 | 0.43 | 0.43 | – | 0.31 | – |
| `general_regex__number_validation` | 4.03 µs | 2.12 | 1.69 | 0.36 | 2.47 | 0.24 | 0.24 | – | 0.17 | – |
| `general_regex__access_log_captures` | 17.7 µs | 1.21 | 1.53 | 0.27 | 0.74 | 1.55 | 1.74 | – | 1.32 | – |
| `general_regex__url_extraction` | 5.27 µs | 2.58 | 2.55 | 0.56 | 1.37 | 1.92 | 1.93 | – | 1.57 | – |
| `general_regex__unicode_words` | 29.1 µs | 2.12 | 1.29 | 0.39 | 1.06 | 1.01 | 1.02 | – | 0.67 | – |
| `general_regex__email_redaction` | 14.5 µs | 4.15 | 6.47 | 0.59 | 3.73 | 0.81 | 0.81 | – | 0.61 | – |
| `text_scanning__literal_50k` | 37.9 ns | 2.42 | 0.67 | 0.34 | 0.84 | 0.25 | 0.26 | – | 0.18 | – |
| `text_scanning__no_match_50k` | 959 ns | 5.23 | 5.19 | 1.20 | 0.79 | 0.99 | 0.99 | – | 0.99 | – |
| `text_scanning__field_extract_50k` | 39.8 ns | 2.63 | 1.03 | 0.48 | 1.26 | 0.83 | 0.83 | – | 0.81 | – |
| `text_scanning__timestamp_50k` | 59 ns | 1.83 | 1.69 | 0.42 | 0.91 | 0.67 | 0.66 | – | 0.59 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 439 µs | 1.05 | 1.59 | 0.28 | 1.45 | 3.26 | 4.93 | – | – | – |
| `regex_tasks__html_attribute_values` | 914 µs | 1.03 | 1.31 | 0.22 | 0.89 | 2.22 | 0.92 | – | – | – |
| `regex_tasks__prices_lookbehind` | 244 µs | 1.60 | 2.40 | 0.26 | 1.12 | 7.78 | 1.76 | – | – | – |
| `regex_tasks__quoted_strings` | 290 µs | 1.26 | 1.52 | 0.24 | 1.46 | 4.83 | 2.67 | – | – | – |
| `regex_tasks__camel_case_words` | 807 µs | 2.12 | 1.31 | 0.40 | 0.83 | 2.52 | 1.66 | – | – | – |
| `regex_tasks__markdown_emphasis` | 199 µs | 1.17 | 1.81 | 0.27 | 0.90 | 10.57 | 1.42 | – | – | – |
| `regex_tasks__emoji_graphemes` | 181 µs | 2.32 | 3.50 | 0.53 | 4.14 | n/a | n/a | – | – | – |
| `regex_tasks__password_rules` | 20.3 µs | 1.16 | 1.54 | 0.21 | 1.35 | 0.53 | 0.51 | – | – | – |
| `regex_tasks__json_objects_recursive` | 313 µs | 1.47 | 2.82 | 0.28 | 2.85 | 2.49 | n/a | – | – | – |
| `regex_tasks__html_nested_divs_recursive` | 67.5 µs | 2.23 | 3.05 | 0.37 | 2.70 | 15.43 | 1570.26 | – | – | – |
| `regex_tasks__variable_lookbehind` | 1.23 ms | 1.12 | n/a | 0.17 | 0.66 | 1.52 | 0.41 | – | – | – |
| `oniguruma_features__atomic_possessive_strings` | 23 µs | 1.60 | 1.14 | 0.33 | 1.19 | 3.16 | 2.69 | – | – | – |
| `oniguruma_features__subexp_call_balanced` | 32.9 µs | 1.36 | 1.62 | 0.31 | 1.80 | n/a | n/a | – | – | – |
| `oniguruma_features__absent_comments` | 28.3 µs | 1.14 | 0.67 | n/a | n/a | 1.57 | 14.78 | – | – | – |
| `oniguruma_features__conditional_brackets` | 40.8 µs | 1.26 | 1.38 | 0.33 | 1.17 | 3.31 | n/a | – | – | – |
| `oniguruma_features__backref_ignorecase` | 72.3 µs | 0.97 | 1.08 | 0.27 | 0.95 | 1.81 | 1.57 | – | – | – |
| `oniguruma_features__lookbehind_alternation` | 14.4 µs | 3.58 | 1.37 | 0.71 | 5.83 | 5.14 | 1.61 | – | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 46.1 ns | 2.47 | 0.79 | 0.32 | 0.91 | 0.26 | 0.25 | – | 0.17 | – |
| `single_pattern__quantifier_greedy` | 46.7 ns | 3.20 | 1.49 | 0.45 | 0.94 | 1.16 | 1.03 | – | 0.91 | – |
| `single_pattern__alternation_2_branch` | 45.4 ns | 2.50 | 1.14 | 0.35 | 1.20 | 0.80 | 0.75 | – | 0.68 | – |
| `single_pattern__alternation_10_branch` | 25.9 ns | 6.48 | 14.76 | 0.74 | 8.07 | 0.74 | 0.69 | – | 0.52 | – |
| `single_pattern__case_insensitive_phrase` | 62 ns | 2.43 | 0.79 | 0.26 | 0.73 | 0.60 | 0.59 | – | 0.61 | – |
| `single_pattern__named_capture_date` | 60.4 ns | 3.50 | 1.81 | 0.73 | 4.44 | 0.51 | 0.51 | – | 0.44 | – |
| `single_pattern__unicode_greek` | 56.8 ns | 2.95 | 3.27 | 0.51 | 2.26 | 0.63 | 0.66 | – | 0.60 | – |
| `single_pattern__lookaround_combined` | 51.4 ns | 3.91 | 1.19 | 0.46 | 4.76 | 3.93 | 1.72 | – | – | – |
| `single_pattern__backref_simple` | 58.5 ns | 2.08 | 0.92 | 0.30 | 0.88 | 1.30 | 1.61 | – | – | – |
| `text_scanning__regset_position_lead` | 69.8 ns | 3.42 | – | – | – | – | – | – | – | – |
| `compilation__literal` | 540 ns | 0.60 | 0.57 | 3.32 | 0.53 | 4.18 | 4.17 | – | 2.90 | – |
| `compilation__named_capture` | 2.61 µs | 1.54 | 2.00 | 1.38 | 0.22 | 62.81 | 62.17 | – | 56.81 | – |
| `compilation__lookbehind` | 1.42 µs | 0.31 | 0.26 | 1.53 | 0.18 | 109.36 | 212.47 | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 17.5 ms | 0.85 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 11.7 ms | 1.44 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 411 µs | 0.38 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 6.84 µs | 23.41 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 14 ms | 0.60 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.23 µs | 25.28 | – | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 92.3 µs | 26.55 | – | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 11.1 ms | 0.18 | – | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 41.1 µs | 17.63 | – | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 3.14 ms | 9.95 | n/a | n/a | n/a | 326.37 | 24.93 | 3.24 | – | 11.73 |
| `cpp_scanner__group_78` | 1.42 ms | 13.32 | n/a | n/a | n/a | 493.61 | 26.26 | 2.90 | – | 13.62 |
| `java_scanner__document` | 891 µs | 8.34 | 30.51 | 9.71 | 33.49 | 76.31 | 9.27 | 1.36 | – | 6.88 |
| `java_scanner__group_15` | 106 µs | 13.25 | 58.15 | 16.97 | 63.73 | 146.93 | 17.33 | 1.35 | – | 11.58 |
| `scss_scanner__document` | 1.32 ms | 24.24 | 16.28 | n/a | n/a | 94.46 | 8.51 | 1.60 | – | 2.87 |
| `scss_scanner__group_8` | 526 µs | 33.14 | 18.33 | n/a | n/a | 119.07 | 11.46 | 1.45 | – | 4.70 |
| `c_scanner__document` | 920 µs | 15.46 | 49.66 | 12.78 | 62.85 | 308.00 | 21.89 | 2.45 | – | 10.78 |
| `c_scanner__group_12` | 199 µs | 20.77 | 70.38 | 15.34 | 93.00 | 397.19 | 26.06 | 3.34 | – | 12.37 |
| `php_scanner__document` | 1.62 ms | 10.04 | 29.16 | 4.98 | 11.07 | 50.29 | 9.05 | 0.95 | – | 2.77 |
| `php_scanner__group_13` | 200 µs | 19.20 | 63.48 | 9.81 | 25.41 | 113.11 | 19.91 | 1.82 | – | 6.43 |

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
| `oniguruma_features/fancy_regex_seek/conditional_brackets` | match 1: [(68, 87), (68, 69)], Oniguruma [(26, 41), (-1, -1)] |
| `oniguruma_features/fancy_regex_seek/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/fancy_regex_seek/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex_seek/json_objects_recursive` | match 0: [(136, 152)], Oniguruma [(4, 208)] |
| `regex_tasks/fancy_regex_seek/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |
| `regex_tasks/pcre2/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/pcre2_jit/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `regex_tasks/regex/unicode_case_folding` | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

