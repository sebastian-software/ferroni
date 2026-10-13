### Engine comparison: linux-x86-64 (AMD EPYC / Intel(R) Xeon(R) Processor)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 375 µs | 1.59 | 1.61 | 0.40 | 1.00 | 2.37 | 2.77 | – | 2.96 | – |
| `regex_tasks__html_attributes` | 701 µs | 2.45 | 4.54 | 0.33 | 2.46 | 0.47 | 0.54 | – | 0.34 | – |
| `regex_tasks__html_comments` | 36.8 µs | 2.36 | 2.09 | 0.65 | 4.53 | 3.99 | 4.03 | – | 3.64 | – |
| `regex_tasks__hex_colors` | 33.1 µs | 4.47 | 10.29 | 0.47 | 0.85 | 35.74 | 1.56 | – | 0.59 | – |
| `regex_tasks__email_addresses` | 50 µs | 32.87 | 53.59 | 2.53 | 23.03 | 0.65 | 0.60 | – | 0.44 | – |
| `regex_tasks__emoji` | 165 µs | 2.26 | 3.72 | 0.62 | 3.78 | 0.55 | 0.63 | – | 0.45 | – |
| `regex_tasks__ascii_emoticons` | 160 µs | 1.04 | 0.92 | 0.32 | 0.61 | 0.61 | 0.57 | – | 0.51 | – |
| `regex_tasks__hashtags_mentions` | 115 µs | 1.91 | 1.69 | 0.50 | 0.82 | 1.03 | 1.09 | – | 0.66 | – |
| `regex_tasks__ipv4_addresses` | 289 µs | 1.45 | 1.92 | 0.77 | 4.51 | 13.51 | 4.25 | – | 3.83 | – |
| `regex_tasks__iso_timestamps` | 119 µs | 2.05 | 3.52 | 0.31 | 5.18 | 26.31 | 1.73 | – | 0.82 | – |
| `regex_tasks__log_keywords_ignorecase` | 83.6 µs | 4.44 | 5.62 | 1.27 | 2.48 | 31.96 | 1.14 | – | 0.77 | – |
| `regex_tasks__markdown_links` | 64.8 µs | 2.64 | 4.43 | 0.77 | 1.41 | 4.02 | 4.14 | – | 3.69 | – |
| `regex_tasks__semantic_versions` | 338 µs | 0.99 | 1.63 | 0.22 | 0.83 | 4.47 | 0.61 | – | 0.42 | – |
| `regex_tasks__json_strings` | 428 µs | 2.24 | 1.34 | 0.60 | 2.15 | 1.92 | 1.67 | – | 1.29 | – |
| `regex_tasks__csv_fields` | 407 µs | 1.69 | 1.37 | 0.49 | 1.42 | 1.23 | 1.04 | – | 0.81 | – |
| `regex_tasks__rfc5322_emails` | 7.05 ms | 0.70 | 1.64 | 0.07 | 1.06 | 0.02 | 0.02 | – | 0.02 | – |
| `regex_tasks__ipv6_addresses` | 2.8 ms | 0.60 | 1.22 | 0.54 | 2.69 | 0.06 | 0.05 | – | 0.04 | – |
| `regex_tasks__rfc3986_urls` | 157 µs | 1.42 | 1.33 | 0.61 | 1.64 | 17.17 | 0.84 | – | 0.56 | – |
| `regex_tasks__keyword_alternation_200` | 1.21 ms | 5.11 | 4.96 | 0.60 | 8.75 | 24.59 | 4.65 | – | 0.61 | – |
| `regex_tasks__csv_last_column_backtracking` | 280 µs | 0.91 | 0.85 | 0.26 | 0.51 | 0.42 | 0.88 | – | 0.69 | – |
| `regex_tasks__unicode_case_folding` | 305 µs | 2.80 | 0.62 | n/a | n/a | n/a | n/a | – | n/a | – |
| `general_regex__email_validation` | 11.1 µs | 1.58 | 1.87 | 0.28 | 0.77 | 0.25 | 0.23 | – | 0.17 | – |
| `general_regex__uuid_validation` | 8.67 µs | 1.89 | 2.14 | 0.32 | 0.73 | 0.30 | 0.31 | – | 0.24 | – |
| `general_regex__number_validation` | 9.61 µs | 1.42 | 1.43 | 0.36 | 1.62 | 0.21 | 0.20 | – | 0.12 | – |
| `general_regex__access_log_captures` | 32.5 µs | 1.54 | 1.52 | 0.25 | 0.85 | 1.83 | 1.45 | – | 1.08 | – |
| `general_regex__url_extraction` | 10.7 µs | 2.51 | 2.50 | 0.52 | 1.15 | 1.84 | 1.84 | – | 1.34 | – |
| `general_regex__unicode_words` | 77.5 µs | 1.70 | 1.21 | 0.42 | 0.92 | 1.03 | 1.03 | – | 0.68 | – |
| `general_regex__email_redaction` | 32.9 µs | 3.38 | 6.12 | 0.45 | 2.58 | 0.65 | 0.82 | – | 0.55 | – |
| `text_scanning__literal_50k` | 61 ns | 2.52 | 0.88 | 0.58 | 1.18 | 0.43 | 0.46 | – | 0.29 | – |
| `text_scanning__no_match_50k` | 1.9 µs | 4.21 | 3.87 | 1.40 | 0.33 | 0.97 | 0.97 | – | 0.78 | – |
| `text_scanning__field_extract_50k` | 93.6 ns | 1.97 | 0.52 | 0.38 | 1.22 | 0.56 | 0.88 | – | 0.76 | – |
| `text_scanning__timestamp_50k` | 103 ns | 1.23 | 1.17 | 0.43 | 1.26 | 0.71 | 0.62 | – | 0.66 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 264 µs | 2.45 | 2.56 | 0.42 | 2.66 | 5.19 | 7.87 | – | – | – |
| `regex_tasks__html_attribute_values` | 690 µs | 1.60 | 1.57 | 0.28 | 1.33 | 3.41 | 1.13 | – | – | – |
| `regex_tasks__prices_lookbehind` | 268 µs | 1.35 | 1.78 | 0.23 | 0.94 | 8.36 | 1.73 | – | – | – |
| `regex_tasks__quoted_strings` | 303 µs | 1.09 | 1.25 | 0.22 | 1.30 | 4.73 | 2.36 | – | – | – |
| `regex_tasks__camel_case_words` | 888 µs | 1.59 | 1.15 | 0.46 | 0.81 | 2.11 | 1.42 | – | – | – |
| `regex_tasks__markdown_emphasis` | 183 µs | 0.94 | 1.56 | 0.23 | 0.98 | 10.03 | 1.52 | – | – | – |
| `regex_tasks__emoji_graphemes` | 169 µs | 1.99 | 3.77 | 0.56 | 4.01 | n/a | n/a | – | – | – |
| `regex_tasks__password_rules` | 22 µs | 1.05 | 1.49 | 0.19 | 1.04 | 0.59 | 0.58 | – | – | – |
| `regex_tasks__json_objects_recursive` | 322 µs | 1.13 | 2.08 | 0.16 | 2.13 | 2.29 | n/a | – | – | – |
| `regex_tasks__html_nested_divs_recursive` | 74.9 µs | 1.79 | 2.60 | 0.24 | 1.76 | 14.20 | 1623.91 | – | – | – |
| `regex_tasks__variable_lookbehind` | 1.1 ms | 1.46 | n/a | 0.19 | 0.76 | 2.31 | 0.57 | – | – | – |
| `oniguruma_features__atomic_possessive_strings` | 26.8 µs | 1.28 | 1.33 | 0.39 | 1.09 | 3.51 | 3.02 | – | – | – |
| `oniguruma_features__subexp_call_balanced` | 46.3 µs | 0.87 | 1.20 | 0.31 | 1.57 | n/a | n/a | – | – | – |
| `oniguruma_features__absent_comments` | 36.8 µs | 0.74 | 0.64 | n/a | n/a | 1.43 | 14.01 | – | – | – |
| `oniguruma_features__conditional_brackets` | 50.4 µs | 1.26 | 1.30 | 0.33 | 1.00 | 2.68 | n/a | – | – | – |
| `oniguruma_features__backref_ignorecase` | 78.9 µs | 1.00 | 1.26 | 0.31 | 1.11 | 1.89 | 1.83 | – | – | – |
| `oniguruma_features__lookbehind_alternation` | 17.8 µs | 3.35 | 1.68 | 0.83 | 5.06 | 4.87 | 1.68 | – | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 43.4 ns | 1.87 | 0.71 | 0.46 | 1.16 | 0.36 | 0.36 | – | 0.24 | – |
| `single_pattern__quantifier_greedy` | 44.9 ns | 2.59 | 1.30 | 0.55 | 1.06 | 1.26 | 1.34 | – | 1.20 | – |
| `single_pattern__alternation_2_branch` | 43.4 ns | 2.02 | 0.71 | 0.41 | 1.29 | 1.03 | 1.30 | – | 0.85 | – |
| `single_pattern__alternation_10_branch` | 37.8 ns | 4.48 | 2.70 | 0.59 | 3.72 | 0.61 | 0.62 | – | 0.46 | – |
| `single_pattern__case_insensitive_phrase` | 72.2 ns | 1.84 | 0.81 | 0.25 | 0.67 | 0.91 | 0.98 | – | 0.76 | – |
| `single_pattern__named_capture_date` | 69.9 ns | 3.11 | 1.63 | 0.60 | 4.68 | 0.75 | 0.66 | – | 0.61 | – |
| `single_pattern__unicode_greek` | 73.9 ns | 2.38 | 3.15 | 0.47 | 2.77 | 1.11 | 0.98 | – | 0.87 | – |
| `single_pattern__lookaround_combined` | 97.3 ns | 3.20 | 0.94 | 0.42 | 2.99 | 2.82 | 1.39 | – | – | – |
| `single_pattern__backref_simple` | 72.6 ns | 1.74 | 1.27 | 0.41 | 1.03 | 1.81 | 2.51 | – | – | – |
| `text_scanning__regset_position_lead` | 117 ns | 2.67 | – | – | – | – | – | – | – | – |
| `compilation__literal` | 1.05 µs | 0.53 | 0.38 | 6.27 | 0.45 | 3.07 | 3.36 | – | 3.03 | – |
| `compilation__named_capture` | 3.69 µs | 1.14 | 1.24 | 1.61 | 0.16 | 47.97 | 46.80 | – | 48.87 | – |
| `compilation__lookbehind` | 1.65 µs | 0.35 | 0.23 | 2.31 | 0.14 | 101.25 | 215.73 | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 20.6 ms | 0.52 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 13.4 ms | 1.20 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 592 µs | 0.33 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 3.24 µs | 48.55 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 152 µs | 75.18 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.14 µs | 32.13 | – | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 116 µs | 24.83 | – | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 125 µs | 26.20 | – | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 67.6 µs | 16.60 | – | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 4.73 ms | 8.77 | n/a | n/a | n/a | 333.89 | 39.58 | 2.93 | – | 9.74 |
| `cpp_scanner__group_78` | 1.89 ms | 13.47 | n/a | n/a | n/a | 491.98 | 24.11 | 2.80 | – | 17.52 |
| `java_scanner__document` | 1.69 ms | 8.48 | 26.14 | 12.34 | 37.85 | 77.95 | 9.56 | 1.07 | – | 6.80 |
| `java_scanner__group_15` | 201 µs | 10.46 | 41.08 | 12.03 | 42.16 | 98.32 | 12.87 | 1.16 | – | 10.84 |
| `scss_scanner__document` | 1.71 ms | 27.06 | 16.74 | n/a | n/a | 107.89 | 9.53 | 1.63 | – | 3.49 |
| `scss_scanner__group_8` | 657 µs | 32.49 | 19.76 | n/a | n/a | 120.20 | 12.21 | 1.77 | – | 5.44 |
| `c_scanner__document` | 1.28 ms | 16.61 | 39.19 | 17.14 | 47.45 | 274.80 | 20.84 | 1.96 | – | 10.81 |
| `c_scanner__group_12` | 282 µs | 16.26 | 57.47 | 12.95 | 62.82 | 308.03 | 26.06 | 3.12 | – | 10.25 |
| `php_scanner__document` | 1.86 ms | 11.75 | 29.51 | 7.03 | 12.18 | 57.22 | 12.57 | 1.08 | – | 3.76 |
| `php_scanner__group_13` | 271 µs | 17.16 | 67.96 | 9.40 | 25.64 | 118.71 | 19.80 | 2.00 | – | 7.21 |
| `json_scanner__document` | 294 µs | 4.25 | 5.62 | 1.69 | 3.04 | 9.17 | 5.52 | 3.10 | – | 3.86 |
| `json_scanner__group_1` | 91 µs | 4.90 | 5.99 | 2.35 | 4.04 | 10.68 | 5.67 | 2.55 | – | 3.41 |
| `astro_scanner__document` | 2.07 ms | 6.45 | n/a | n/a | n/a | n/a | n/a | n/a | – | 7.13 |
| `astro_scanner__group_22` | 143 µs | 4.14 | 4.60 | 2.62 | 3.38 | n/a | n/a | n/a | – | 4.62 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `astro_scanner/document_fancy_regex` | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/document_fancy_regex_seek` | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/document_fancy_regex_set` | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/document_onigmo` | scanner 8, pattern "(?!(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)\|(?<!\\.))(?:\\b(export)\\s+)?(?:\\b(declare)\\s+)?\\b(const(?!\\s+enum\\b))(?![$_[:alnum:]])(?:(?=\\.\\.\\.)\|(?!\\.)))… |
| `astro_scanner/document_pcre2` | scanner 60, pattern "\\\u{ffff}\\uFEFF?(?i:(?=\\s*@charset\\b))": PCRE2 at offset 6: PCRE2 does not support \F, \L, \l, \N{name}, \U, or \u |
| `astro_scanner/document_pcre2_jit` | scanner 60, pattern "\\\u{ffff}\\uFEFF?(?i:(?=\\s*@charset\\b))": PCRE2 at offset 6: PCRE2 does not support \F, \L, \l, \N{name}, \U, or \u |
| `astro_scanner/group_22_fancy_regex` | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/group_22_fancy_regex_seek` | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/group_22_fancy_regex_set` | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
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

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 296 µs | 1.67 | 1.89 | 0.52 | 0.97 | 2.97 | 3.80 | – | 3.91 | – |
| `regex_tasks__html_attributes` | 487 µs | 3.21 | 4.46 | 0.42 | 2.28 | 0.60 | 0.59 | – | 0.49 | – |
| `regex_tasks__html_comments` | 20.5 µs | 3.80 | 3.47 | 0.69 | 7.32 | 6.99 | 7.04 | – | 7.08 | – |
| `regex_tasks__hex_colors` | 16.6 µs | 5.07 | 25.80 | 0.78 | 1.12 | 67.34 | 2.93 | – | 0.78 | – |
| `regex_tasks__email_addresses` | 42.5 µs | 33.11 | 59.37 | 2.96 | 30.44 | 0.59 | 0.58 | – | 0.42 | – |
| `regex_tasks__emoji` | 114 µs | 3.01 | 4.24 | 0.77 | 5.12 | 0.86 | 0.85 | – | 0.58 | – |
| `regex_tasks__ascii_emoticons` | 64.5 µs | 1.85 | 2.12 | 0.35 | 0.88 | 0.69 | 0.68 | – | 0.54 | – |
| `regex_tasks__hashtags_mentions` | 70.4 µs | 2.53 | 2.14 | 0.56 | 1.04 | 1.28 | 1.31 | – | 0.88 | – |
| `regex_tasks__ipv4_addresses` | 179 µs | 1.54 | 2.92 | 1.12 | 5.78 | 20.42 | 5.69 | – | 5.21 | – |
| `regex_tasks__iso_timestamps` | 68.8 µs | 2.08 | 5.81 | 0.39 | 6.79 | 25.85 | 2.71 | – | 0.85 | – |
| `regex_tasks__log_keywords_ignorecase` | 92.1 µs | 2.73 | 4.69 | 0.90 | 2.03 | 23.61 | 0.80 | – | 0.58 | – |
| `regex_tasks__markdown_links` | 43.7 µs | 3.49 | 6.30 | 0.95 | 1.81 | 6.82 | 6.50 | – | 6.35 | – |
| `regex_tasks__semantic_versions` | 241 µs | 1.09 | 1.55 | 0.31 | 0.69 | 4.65 | 0.54 | – | 0.32 | – |
| `regex_tasks__json_strings` | 291 µs | 2.09 | 1.51 | 0.51 | 2.27 | 1.15 | 1.15 | – | 0.73 | – |
| `regex_tasks__csv_fields` | 182 µs | 2.09 | 1.52 | 0.45 | 1.78 | 1.13 | 1.13 | – | 0.83 | – |
| `regex_tasks__rfc5322_emails` | 3.18 ms | 0.74 | 2.17 | 0.10 | 1.52 | 0.05 | 0.05 | – | 0.04 | – |
| `regex_tasks__ipv6_addresses` | 1.85 ms | 0.63 | 1.61 | 0.44 | 2.70 | 0.07 | 0.08 | – | 0.07 | – |
| `regex_tasks__rfc3986_urls` | 86.8 µs | 1.71 | 1.93 | 0.76 | 2.27 | 19.85 | 0.88 | – | 0.54 | – |
| `regex_tasks__keyword_alternation_200` | 851 µs | 3.84 | 4.96 | 0.57 | 10.95 | 22.34 | 4.01 | – | 0.47 | – |
| `regex_tasks__csv_last_column_backtracking` | 142 µs | 1.15 | 1.34 | 0.27 | 0.57 | 0.78 | 1.36 | – | 1.16 | – |
| `regex_tasks__unicode_case_folding` | 141 µs | 3.62 | 1.11 | n/a | n/a | n/a | n/a | – | n/a | – |
| `general_regex__email_validation` | 4.67 µs | 1.99 | 2.00 | 0.34 | 1.15 | 0.33 | 0.29 | – | 0.22 | – |
| `general_regex__uuid_validation` | 3.77 µs | 3.03 | 3.32 | 0.39 | 0.79 | 0.39 | 0.39 | – | 0.31 | – |
| `general_regex__number_validation` | 3.98 µs | 2.14 | 2.04 | 0.37 | 2.49 | 0.26 | 0.28 | – | 0.17 | – |
| `general_regex__access_log_captures` | 19.8 µs | 1.34 | 1.54 | 0.27 | 0.72 | 1.49 | 1.75 | – | 1.29 | – |
| `general_regex__url_extraction` | 5.32 µs | 2.81 | 2.53 | 0.64 | 1.55 | 2.22 | 2.09 | – | 1.65 | – |
| `general_regex__unicode_words` | 28.2 µs | 1.89 | 1.31 | 0.39 | 1.09 | 1.03 | 1.04 | – | 0.67 | – |
| `general_regex__email_redaction` | 14.2 µs | 4.21 | 6.55 | 0.61 | 3.78 | 0.83 | 0.83 | – | 0.61 | – |
| `text_scanning__literal_50k` | 35.5 ns | 3.20 | 0.82 | 0.43 | 1.02 | 0.32 | 0.32 | – | 0.22 | – |
| `text_scanning__no_match_50k` | 1.04 µs | 5.86 | 5.66 | 1.25 | 0.80 | 1.03 | 1.03 | – | 0.99 | – |
| `text_scanning__field_extract_50k` | 43.5 ns | 2.81 | 1.04 | 0.52 | 1.41 | 0.89 | 0.91 | – | 0.88 | – |
| `text_scanning__timestamp_50k` | 65.5 ns | 1.86 | 1.74 | 0.43 | 0.94 | 0.69 | 0.68 | – | 0.60 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 173 µs | 2.63 | 3.65 | 0.62 | 3.30 | 7.51 | 11.36 | – | – | – |
| `regex_tasks__html_attribute_values` | 513 µs | 1.69 | 2.10 | 0.37 | 1.40 | 3.68 | 1.48 | – | – | – |
| `regex_tasks__prices_lookbehind` | 208 µs | 1.66 | 2.44 | 0.26 | 1.13 | 7.89 | 1.69 | – | – | – |
| `regex_tasks__quoted_strings` | 237 µs | 1.35 | 1.66 | 0.24 | 1.52 | 5.05 | 2.80 | – | – | – |
| `regex_tasks__camel_case_words` | 683 µs | 2.10 | 1.39 | 0.42 | 0.87 | 2.51 | 1.79 | – | – | – |
| `regex_tasks__markdown_emphasis` | 158 µs | 1.20 | 1.99 | 0.29 | 1.02 | 11.77 | 1.59 | – | – | – |
| `regex_tasks__emoji_graphemes` | 171 µs | 2.37 | 3.64 | 0.60 | 4.66 | n/a | n/a | – | – | – |
| `regex_tasks__password_rules` | 17.5 µs | 1.24 | 1.86 | 0.24 | 1.54 | 0.63 | 0.68 | – | – | – |
| `regex_tasks__json_objects_recursive` | 328 µs | 1.38 | 2.49 | 0.26 | 2.62 | 2.30 | n/a | – | – | – |
| `regex_tasks__html_nested_divs_recursive` | 62.3 µs | 2.04 | 3.10 | 0.36 | 2.83 | 15.54 | 1619.16 | – | – | – |
| `regex_tasks__variable_lookbehind` | 984 µs | 1.35 | n/a | 0.21 | 0.78 | 1.82 | 0.48 | – | – | – |
| `oniguruma_features__atomic_possessive_strings` | 21.9 µs | 1.69 | 1.19 | 0.36 | 1.23 | 3.26 | 2.83 | – | – | – |
| `oniguruma_features__subexp_call_balanced` | 32.8 µs | 1.38 | 1.65 | 0.32 | 1.79 | n/a | n/a | – | – | – |
| `oniguruma_features__absent_comments` | 26 µs | 1.06 | 0.64 | n/a | n/a | 1.50 | 13.76 | – | – | – |
| `oniguruma_features__conditional_brackets` | 35.1 µs | 1.37 | 1.77 | 0.36 | 1.41 | 4.06 | n/a | – | – | – |
| `oniguruma_features__backref_ignorecase` | 70.1 µs | 1.06 | 1.20 | 0.30 | 1.06 | 2.09 | 1.73 | – | – | – |
| `oniguruma_features__lookbehind_alternation` | 14.6 µs | 3.73 | 1.48 | 0.81 | 6.17 | 5.61 | 1.82 | – | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 35.9 ns | 3.07 | 1.11 | 0.42 | 1.08 | 0.28 | 0.29 | – | 0.21 | – |
| `single_pattern__quantifier_greedy` | 41.1 ns | 3.52 | 1.58 | 0.53 | 1.12 | 1.24 | 1.18 | – | 1.02 | – |
| `single_pattern__alternation_2_branch` | 39.8 ns | 2.72 | 1.00 | 0.39 | 1.37 | 0.86 | 0.82 | – | 0.78 | – |
| `single_pattern__alternation_10_branch` | 25 ns | 5.83 | 15.17 | 0.75 | 7.65 | 0.66 | 0.70 | – | 0.50 | – |
| `single_pattern__case_insensitive_phrase` | 55.7 ns | 2.41 | 0.89 | 0.32 | 0.97 | 0.72 | 0.67 | – | 0.68 | – |
| `single_pattern__named_capture_date` | 61.6 ns | 3.33 | 1.77 | 0.74 | 4.58 | 0.52 | 0.51 | – | 0.45 | – |
| `single_pattern__unicode_greek` | 59 ns | 2.83 | 3.25 | 0.55 | 2.31 | 0.72 | 0.73 | – | 0.63 | – |
| `single_pattern__lookaround_combined` | 58.3 ns | 3.78 | 1.12 | 0.47 | 4.72 | 4.03 | 1.61 | – | – | – |
| `single_pattern__backref_simple` | 61 ns | 1.98 | 1.04 | 0.30 | 0.87 | 1.46 | 1.83 | – | – | – |
| `text_scanning__regset_position_lead` | 82.6 ns | 3.68 | – | – | – | – | – | – | – | – |
| `compilation__literal` | 656 ns | 0.61 | 0.50 | 3.12 | 0.53 | 4.42 | 4.03 | – | 3.10 | – |
| `compilation__named_capture` | 2.99 µs | 1.54 | 1.73 | 1.06 | 0.18 | 50.85 | 53.83 | – | 50.43 | – |
| `compilation__lookbehind` | 1.48 µs | 0.29 | 0.24 | 1.50 | 0.17 | 98.98 | 198.58 | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 16.9 ms | 0.85 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 11.4 ms | 1.42 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 380 µs | 0.39 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.94 µs | 52.44 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 162 µs | 51.18 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 1.69 µs | 32.31 | – | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 89.2 µs | 25.67 | – | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 76.7 µs | 25.30 | – | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 39.8 µs | 16.56 | – | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 3.08 ms | 9.08 | n/a | n/a | n/a | 306.69 | 26.47 | 3.36 | – | 10.25 |
| `cpp_scanner__group_78` | 1.37 ms | 12.70 | n/a | n/a | n/a | 490.87 | 27.81 | 3.00 | – | 13.88 |
| `java_scanner__document` | 901 µs | 9.38 | 34.90 | 11.79 | 35.65 | 80.16 | 9.70 | 1.60 | – | 6.97 |
| `java_scanner__group_15` | 123 µs | 12.54 | 50.02 | 15.33 | 48.76 | 118.29 | 13.93 | 1.14 | – | 11.57 |
| `scss_scanner__document` | 1.35 ms | 26.11 | 15.70 | n/a | n/a | 95.44 | 9.36 | 1.65 | – | 2.82 |
| `scss_scanner__group_8` | 573 µs | 32.12 | 17.92 | n/a | n/a | 111.52 | 12.14 | 1.45 | – | 3.83 |
| `c_scanner__document` | 1.04 ms | 15.23 | 47.66 | 13.18 | 51.49 | 254.73 | 20.62 | 2.21 | – | 10.14 |
| `c_scanner__group_12` | 224 µs | 19.59 | 69.27 | 15.37 | 89.13 | 377.56 | 26.72 | 3.21 | – | 11.90 |
| `php_scanner__document` | 1.61 ms | 10.48 | 29.58 | 5.16 | 12.50 | 54.26 | 11.90 | 1.13 | – | 3.08 |
| `php_scanner__group_13` | 193 µs | 21.03 | 65.56 | 10.80 | 31.09 | 140.12 | 24.56 | 1.89 | – | 6.97 |
| `json_scanner__document` | 214 µs | 7.02 | 7.14 | 2.41 | 3.79 | 10.71 | 5.77 | 3.38 | – | 3.32 |
| `json_scanner__group_1` | 74.3 µs | 6.57 | 6.90 | 2.44 | 4.42 | 11.06 | 5.00 | 2.30 | – | 3.34 |
| `astro_scanner__document` | 1.54 ms | 6.57 | n/a | n/a | n/a | n/a | n/a | n/a | – | 7.83 |
| `astro_scanner__group_22` | 127 µs | 4.21 | 4.94 | 2.75 | 6.11 | n/a | n/a | n/a | – | 3.23 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `astro_scanner/document_fancy_regex` | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/document_fancy_regex_seek` | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/document_fancy_regex_set` | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/document_onigmo` | scanner 8, pattern "(?!(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)\|(?<!\\.))(?:\\b(export)\\s+)?(?:\\b(declare)\\s+)?\\b(const(?!\\s+enum\\b))(?![$_[:alnum:]])(?:(?=\\.\\.\\.)\|(?!\\.)))… |
| `astro_scanner/document_pcre2` | scanner 60, pattern "\\\u{ffff}\\uFEFF?(?i:(?=\\s*@charset\\b))": PCRE2 at offset 6: PCRE2 does not support \F, \L, \l, \N{name}, \U, or \u |
| `astro_scanner/document_pcre2_jit` | scanner 60, pattern "\\\u{ffff}\\uFEFF?(?i:(?=\\s*@charset\\b))": PCRE2 at offset 6: PCRE2 does not support \F, \L, \l, \N{name}, \U, or \u |
| `astro_scanner/group_22_fancy_regex` | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/group_22_fancy_regex_seek` | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| `astro_scanner/group_22_fancy_regex_set` | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
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

