### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### shared

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_tags` | 362 µs | 1.43 | 1.71 | 0.38 | 0.86 | 2.88 | 2.44 | – | 1.85 | – |
| `regex_tasks__html_attributes` | 568 µs | 2.88 | 3.78 | 0.31 | 2.22 | 0.58 | 0.58 | – | 0.47 | – |
| `regex_tasks__html_comments` | 23.3 µs | 2.82 | 3.00 | 0.68 | 5.06 | 5.65 | 5.57 | – | 5.41 | – |
| `regex_tasks__hex_colors` | 22.1 µs | 6.25 | 15.31 | 0.67 | 1.20 | 69.68 | 2.72 | – | 0.97 | – |
| `regex_tasks__email_addresses` | 45.7 µs | 32.62 | 57.77 | 1.87 | 29.28 | 0.81 | 0.78 | – | 0.55 | – |
| `regex_tasks__emoji` | 151 µs | 2.74 | 3.49 | 0.74 | 4.07 | 0.68 | 0.65 | – | 0.54 | – |
| `regex_tasks__ascii_emoticons` | 105 µs | 1.03 | 1.34 | 0.30 | 0.62 | 0.84 | 0.87 | – | 0.70 | – |
| `regex_tasks__hashtags_mentions` | 79.6 µs | 2.21 | 2.26 | 0.60 | 1.08 | 1.47 | 1.56 | – | 0.94 | – |
| `regex_tasks__ipv4_addresses` | 223 µs | 1.40 | 2.33 | 0.80 | 4.38 | 17.92 | 5.52 | – | 4.82 | – |
| `regex_tasks__iso_timestamps` | 85.6 µs | 1.85 | 4.39 | 0.38 | 8.73 | 29.38 | 2.47 | – | 0.77 | – |
| `regex_tasks__log_keywords_ignorecase` | 86.6 µs | 4.11 | 6.54 | 1.28 | 2.67 | 40.16 | 1.20 | – | 0.95 | – |
| `regex_tasks__markdown_links` | 70.7 µs | 2.48 | 4.13 | 0.56 | 1.27 | 4.13 | 3.93 | – | 3.49 | – |
| `regex_tasks__semantic_versions` | 294 µs | 1.14 | 1.67 | 0.21 | 0.69 | 5.17 | 0.59 | – | 0.29 | – |
| `regex_tasks__json_strings` | 403 µs | 1.88 | 1.27 | 0.54 | 2.06 | 1.43 | 1.36 | – | 0.91 | – |
| `regex_tasks__csv_fields` | 210 µs | 1.89 | 1.70 | 0.59 | 1.72 | 1.54 | 1.55 | – | 1.04 | – |
| `regex_tasks__rfc5322_emails` | 4.44 ms | 0.79 | 2.20 | 0.07 | 1.20 | 0.03 | 0.03 | – | 0.03 | – |
| `regex_tasks__ipv6_addresses` | 2.13 ms | 0.75 | 1.37 | 0.36 | 2.36 | 0.06 | 0.06 | – | 0.06 | – |
| `regex_tasks__rfc3986_urls` | 80.3 µs | 1.66 | 2.24 | 0.74 | 2.14 | 22.56 | 1.06 | – | 0.59 | – |
| `regex_tasks__keyword_alternation_200` | 924 µs | 4.46 | 5.03 | 0.66 | 9.63 | 19.59 | 4.81 | – | 0.57 | – |
| `regex_tasks__csv_last_column_backtracking` | 170 µs | 0.93 | 1.44 | 0.25 | 0.51 | 0.68 | 1.19 | – | 0.98 | – |
| `regex_tasks__unicode_case_folding` | 201 µs | 3.05 | 0.90 | n/a | n/a | n/a | n/a | – | n/a | – |
| `general_regex__email_validation` | 7.73 µs | 1.24 | 1.51 | 0.28 | 0.77 | 0.25 | 0.27 | – | 0.25 | – |
| `general_regex__uuid_validation` | 5.22 µs | 2.13 | 2.96 | 0.40 | 0.77 | 0.41 | 0.41 | – | 0.30 | – |
| `general_regex__number_validation` | 5.23 µs | 1.36 | 1.56 | 0.36 | 1.76 | 0.26 | 0.26 | – | 0.17 | – |
| `general_regex__access_log_captures` | 25.7 µs | 1.05 | 1.51 | 0.20 | 0.52 | 1.29 | 1.50 | – | 1.18 | – |
| `general_regex__url_extraction` | 6.35 µs | 2.05 | 2.36 | 0.56 | 1.29 | 2.05 | 1.98 | – | 1.51 | – |
| `general_regex__unicode_words` | 38.1 µs | 1.64 | 1.33 | 0.55 | 1.13 | 1.20 | 1.34 | – | 0.87 | – |
| `general_regex__email_redaction` | 19.2 µs | 4.26 | 7.40 | 0.65 | 4.31 | 1.13 | 1.14 | – | 0.79 | – |
| `text_scanning__literal_50k` | 44 ns | 2.20 | 0.58 | 0.43 | 0.97 | 0.31 | 0.31 | – | 0.28 | – |
| `text_scanning__no_match_50k` | 780 ns | 8.59 | 8.87 | 1.10 | 0.35 | 1.01 | 1.21 | – | 0.82 | – |
| `text_scanning__field_extract_50k` | 55.9 ns | 1.81 | 0.81 | 0.45 | 1.28 | 1.03 | 0.90 | – | 0.92 | – |
| `text_scanning__timestamp_50k` | 62.9 ns | 1.47 | 2.36 | 0.50 | 1.16 | 0.98 | 0.96 | – | 0.82 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 244 µs | 2.52 | 3.00 | 0.53 | 3.71 | 8.21 | 8.66 | – | – | – |
| `regex_tasks__html_attribute_values` | 778 µs | 1.80 | 1.50 | 0.27 | 1.12 | 2.93 | 0.98 | – | – | – |
| `regex_tasks__prices_lookbehind` | 257 µs | 1.55 | 1.96 | 0.24 | 0.90 | 7.37 | 2.08 | – | – | – |
| `regex_tasks__quoted_strings` | 337 µs | 1.27 | 1.34 | 0.26 | 1.51 | 4.17 | 2.79 | – | – | – |
| `regex_tasks__camel_case_words` | 1.19 ms | 1.67 | 1.31 | 0.42 | 0.73 | 2.01 | 1.47 | – | – | – |
| `regex_tasks__markdown_emphasis` | 308 µs | 0.83 | 1.11 | 0.19 | 0.82 | 8.02 | 1.03 | – | – | – |
| `regex_tasks__emoji_graphemes` | 226 µs | 1.86 | 3.65 | 0.59 | 3.57 | n/a | n/a | – | – | – |
| `regex_tasks__password_rules` | 26.3 µs | 0.98 | 1.53 | 0.16 | 1.02 | 0.44 | 0.52 | – | – | – |
| `regex_tasks__json_objects_recursive` | 347 µs | 1.24 | 2.35 | 0.19 | 2.38 | 2.59 | n/a | – | – | – |
| `regex_tasks__html_nested_divs_recursive` | 71.1 µs | 1.94 | 3.19 | 0.29 | 2.17 | 14.13 | 1767.37 | – | – | – |
| `regex_tasks__variable_lookbehind` | 1.24 ms | 1.51 | n/a | 0.21 | 0.71 | 1.99 | 0.62 | – | – | – |
| `oniguruma_features__atomic_possessive_strings` | 33.8 µs | 1.27 | 1.21 | 0.32 | 0.93 | 2.73 | 2.41 | – | – | – |
| `oniguruma_features__subexp_call_balanced` | 50.8 µs | 1.05 | 1.00 | 0.28 | 1.60 | n/a | n/a | – | – | – |
| `oniguruma_features__absent_comments` | 34.8 µs | 0.74 | 0.63 | n/a | n/a | 1.87 | 16.46 | – | – | – |
| `oniguruma_features__conditional_brackets` | 49.1 µs | 1.55 | 1.36 | 0.37 | 1.20 | 3.15 | n/a | – | – | – |
| `oniguruma_features__backref_ignorecase` | 82.5 µs | 1.17 | 1.35 | 0.34 | 1.39 | 2.51 | 2.33 | – | – | – |
| `oniguruma_features__lookbehind_alternation` | 19.5 µs | 3.86 | 1.35 | 0.78 | 4.95 | 5.34 | 1.81 | – | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 44.8 ns | 2.17 | 0.80 | 0.48 | 1.22 | 0.38 | 0.41 | – | 0.30 | – |
| `single_pattern__quantifier_greedy` | 46.5 ns | 2.43 | 1.47 | 0.52 | 1.05 | 1.34 | 1.33 | – | 1.18 | – |
| `single_pattern__alternation_2_branch` | 49.4 ns | 1.47 | 0.92 | 0.45 | 1.44 | 1.21 | 1.15 | – | 0.87 | – |
| `single_pattern__alternation_10_branch` | 45.6 ns | 3.68 | 3.14 | 0.66 | 4.04 | 0.61 | 0.57 | – | 0.46 | – |
| `single_pattern__case_insensitive_phrase` | 92.1 ns | 1.60 | 0.85 | 0.25 | 0.61 | 0.76 | 0.86 | – | 0.73 | – |
| `single_pattern__named_capture_date` | 106 ns | 2.37 | 1.41 | 0.53 | 3.79 | 0.51 | 0.53 | – | 0.49 | – |
| `single_pattern__unicode_greek` | 109 ns | 2.14 | 1.92 | 0.34 | 1.96 | 0.74 | 0.73 | – | 0.65 | – |
| `single_pattern__lookaround_combined` | 97.4 ns | 2.29 | 0.65 | 0.31 | 2.79 | 2.29 | 1.13 | – | – | – |
| `single_pattern__backref_simple` | 70.1 ns | 1.59 | 0.94 | 0.34 | 0.87 | 1.38 | 1.91 | – | – | – |
| `text_scanning__regset_position_lead` | 86 ns | 2.53 | – | – | – | – | – | – | – | – |
| `compilation__literal` | 731 ns | 0.52 | 0.46 | 5.94 | 0.41 | 3.95 | 3.68 | – | 2.89 | – |
| `compilation__named_capture` | 2.9 µs | 1.31 | 1.51 | 2.03 | 0.20 | 60.47 | 58.08 | – | 60.62 | – |
| `compilation__lookbehind` | 1.47 µs | 0.46 | 0.34 | 3.00 | 0.20 | 131.83 | 270.03 | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 19.8 ms | 0.54 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 13.5 ms | 1.13 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 566 µs | 0.32 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 3.75 µs | 40.52 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 151 µs | 68.83 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.42 µs | 28.82 | – | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 113 µs | 22.50 | – | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 97.5 µs | 27.44 | – | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 50.4 µs | 18.48 | – | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 4.55 ms | 8.67 | n/a | n/a | n/a | 252.93 | 34.62 | 2.61 | – | 8.82 |
| `cpp_scanner__group_78` | 1.22 ms | 13.98 | n/a | n/a | n/a | 556.65 | 31.44 | 3.62 | – | 18.12 |
| `java_scanner__document` | 1.1 ms | 9.30 | 29.19 | 11.02 | 37.13 | 93.31 | 10.41 | 1.26 | – | 9.13 |
| `java_scanner__group_15` | 142 µs | 13.19 | 41.96 | 13.13 | 48.12 | 117.09 | 13.55 | 1.08 | – | 15.40 |
| `scss_scanner__document` | 1.29 ms | 30.15 | 16.60 | n/a | n/a | 135.02 | 11.22 | 1.87 | – | 3.41 |
| `scss_scanner__group_8` | 537 µs | 30.97 | 17.74 | n/a | n/a | 108.96 | 11.41 | 1.56 | – | 4.95 |
| `c_scanner__document` | 1.09 ms | 16.69 | 43.10 | 17.30 | 46.38 | 333.44 | 25.68 | 2.45 | – | 10.88 |
| `c_scanner__group_12` | 253 µs | 16.55 | 57.05 | 13.40 | 60.77 | 284.23 | 22.08 | 2.84 | – | 12.00 |
| `php_scanner__document` | 1.62 ms | 12.00 | 31.41 | 6.36 | 11.42 | 61.79 | 12.66 | 1.10 | – | 3.70 |
| `php_scanner__group_13` | 193 µs | 20.33 | 78.02 | 12.78 | 32.55 | 168.91 | 22.96 | 2.38 | – | 7.82 |
| `json_scanner__document` | 237 µs | 4.26 | 5.80 | 1.86 | 3.25 | 10.87 | 6.27 | 3.51 | – | 3.58 |
| `json_scanner__group_1` | 70.5 µs | 4.99 | 6.67 | 2.35 | 3.71 | 11.34 | 6.03 | 2.89 | – | 3.51 |
| `astro_scanner__document` | 1.69 ms | 6.55 | n/a | n/a | n/a | n/a | n/a | n/a | – | 7.75 |
| `astro_scanner__group_22` | 157 µs | 4.29 | 3.86 | 3.42 | 3.65 | n/a | n/a | n/a | – | 3.91 |

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
| `regex_tasks__html_tags` | 302 µs | 1.69 | 1.89 | 0.47 | 0.93 | 2.87 | 3.94 | – | 3.99 | – |
| `regex_tasks__html_attributes` | 510 µs | 3.21 | 4.20 | 0.39 | 2.14 | 0.57 | 0.60 | – | 0.47 | – |
| `regex_tasks__html_comments` | 20.3 µs | 3.71 | 3.36 | 0.70 | 6.76 | 6.94 | 7.01 | – | 7.34 | – |
| `regex_tasks__hex_colors` | 16.5 µs | 5.05 | 25.48 | 0.84 | 1.24 | 71.33 | 3.08 | – | 0.88 | – |
| `regex_tasks__email_addresses` | 46.1 µs | 30.92 | 57.29 | 2.69 | 27.20 | 0.55 | 0.55 | – | 0.40 | – |
| `regex_tasks__emoji` | 114 µs | 2.83 | 4.30 | 0.79 | 4.98 | 0.84 | 0.84 | – | 0.57 | – |
| `regex_tasks__ascii_emoticons` | 67.2 µs | 1.81 | 2.14 | 0.33 | 0.89 | 0.70 | 0.69 | – | 0.52 | – |
| `regex_tasks__hashtags_mentions` | 75.5 µs | 2.43 | 1.76 | 0.53 | 0.98 | 1.15 | 1.14 | – | 0.86 | – |
| `regex_tasks__ipv4_addresses` | 154 µs | 1.63 | 3.39 | 1.26 | 6.48 | 23.38 | 6.55 | – | 5.79 | – |
| `regex_tasks__iso_timestamps` | 68.7 µs | 2.06 | 4.71 | 0.38 | 6.72 | 24.47 | 2.31 | – | 0.85 | – |
| `regex_tasks__log_keywords_ignorecase` | 79.7 µs | 2.78 | 4.92 | 0.91 | 2.12 | 24.68 | 0.83 | – | 0.55 | – |
| `regex_tasks__markdown_links` | 42.2 µs | 3.04 | 5.78 | 0.84 | 1.59 | 6.06 | 6.04 | – | 5.70 | – |
| `regex_tasks__semantic_versions` | 206 µs | 1.06 | 1.78 | 0.31 | 0.81 | 5.53 | 0.64 | – | 0.36 | – |
| `regex_tasks__json_strings` | 292 µs | 1.99 | 1.50 | 0.51 | 2.26 | 1.14 | 1.14 | – | 0.74 | – |
| `regex_tasks__csv_fields` | 181 µs | 2.13 | 1.53 | 0.45 | 1.78 | 1.15 | 1.14 | – | 0.86 | – |
| `regex_tasks__rfc5322_emails` | 3.12 ms | 0.75 | 2.27 | 0.10 | 1.52 | 0.04 | 0.05 | – | 0.04 | – |
| `regex_tasks__ipv6_addresses` | 1.79 ms | 0.65 | 1.46 | 0.45 | 2.86 | 0.07 | 0.07 | – | 0.07 | – |
| `regex_tasks__rfc3986_urls` | 80.3 µs | 1.76 | 2.07 | 0.80 | 2.41 | 19.00 | 0.91 | – | 0.55 | – |
| `regex_tasks__keyword_alternation_200` | 802 µs | 4.33 | 5.17 | 0.61 | 11.46 | 24.24 | 4.19 | – | 0.48 | – |
| `regex_tasks__csv_last_column_backtracking` | 139 µs | 1.13 | 1.35 | 0.27 | 0.61 | 0.85 | 1.50 | – | 1.18 | – |
| `regex_tasks__unicode_case_folding` | 134 µs | 3.44 | 1.11 | n/a | n/a | n/a | n/a | – | n/a | – |
| `general_regex__email_validation` | 4.84 µs | 2.02 | 1.96 | 0.33 | 1.14 | 0.29 | 0.28 | – | 0.22 | – |
| `general_regex__uuid_validation` | 3.63 µs | 3.13 | 3.66 | 0.40 | 0.86 | 0.42 | 0.42 | – | 0.32 | – |
| `general_regex__number_validation` | 4.14 µs | 2.26 | 1.81 | 0.39 | 2.69 | 0.27 | 0.24 | – | 0.16 | – |
| `general_regex__access_log_captures` | 17.5 µs | 1.36 | 1.78 | 0.32 | 0.83 | 1.73 | 2.01 | – | 1.57 | – |
| `general_regex__url_extraction` | 5.78 µs | 2.72 | 2.65 | 0.56 | 1.34 | 1.98 | 1.95 | – | 1.58 | – |
| `general_regex__unicode_words` | 31.7 µs | 1.97 | 1.31 | 0.42 | 1.15 | 1.05 | 1.05 | – | 0.70 | – |
| `general_regex__email_redaction` | 16.5 µs | 4.17 | 6.52 | 0.61 | 3.81 | 0.83 | 0.85 | – | 0.63 | – |
| `text_scanning__literal_50k` | 42.3 ns | 2.92 | 0.73 | 0.37 | 0.94 | 0.27 | 0.28 | – | 0.18 | – |
| `text_scanning__no_match_50k` | 1.12 µs | 5.68 | 5.66 | 1.15 | 0.79 | 0.99 | 0.98 | – | 0.96 | – |
| `text_scanning__field_extract_50k` | 42.8 ns | 2.96 | 1.11 | 0.53 | 1.37 | 0.91 | 0.92 | – | 0.89 | – |
| `text_scanning__timestamp_50k` | 65.9 ns | 2.00 | 1.70 | 0.44 | 0.96 | 0.70 | 0.67 | – | 0.60 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 178 µs | 2.61 | 3.73 | 0.62 | 3.33 | 7.69 | 11.18 | – | – | – |
| `regex_tasks__html_attribute_values` | 646 µs | 1.57 | 1.83 | 0.29 | 1.21 | 2.99 | 1.27 | – | – | – |
| `regex_tasks__prices_lookbehind` | 217 µs | 1.69 | 2.60 | 0.26 | 1.14 | 7.74 | 1.76 | – | – | – |
| `regex_tasks__quoted_strings` | 248 µs | 1.30 | 1.60 | 0.24 | 1.44 | 5.01 | 2.84 | – | – | – |
| `regex_tasks__camel_case_words` | 719 µs | 2.11 | 1.34 | 0.39 | 0.80 | 2.52 | 1.77 | – | – | – |
| `regex_tasks__markdown_emphasis` | 176 µs | 1.27 | 1.91 | 0.32 | 1.00 | 11.14 | 1.49 | – | – | – |
| `regex_tasks__emoji_graphemes` | 184 µs | 2.16 | 3.04 | 0.50 | 3.78 | n/a | n/a | – | – | – |
| `regex_tasks__password_rules` | 15.1 µs | 1.32 | 2.05 | 0.25 | 1.67 | 0.65 | 0.68 | – | – | – |
| `regex_tasks__json_objects_recursive` | 329 µs | 1.36 | 2.51 | 0.25 | 2.56 | 2.28 | n/a | – | – | – |
| `regex_tasks__html_nested_divs_recursive` | 63 µs | 2.15 | 3.30 | 0.38 | 2.74 | 16.06 | 1703.86 | – | – | – |
| `regex_tasks__variable_lookbehind` | 1.03 ms | 1.43 | n/a | 0.22 | 0.84 | 1.93 | 0.51 | – | – | – |
| `oniguruma_features__atomic_possessive_strings` | 23.2 µs | 1.69 | 1.02 | 0.31 | 1.10 | 2.88 | 2.46 | – | – | – |
| `oniguruma_features__subexp_call_balanced` | 29.2 µs | 1.33 | 1.61 | 0.30 | 1.71 | n/a | n/a | – | – | – |
| `oniguruma_features__absent_comments` | 23.6 µs | 1.11 | 0.70 | n/a | n/a | 1.66 | 15.08 | – | – | – |
| `oniguruma_features__conditional_brackets` | 35.6 µs | 1.40 | 1.71 | 0.36 | 1.23 | 3.58 | n/a | – | – | – |
| `oniguruma_features__backref_ignorecase` | 68.7 µs | 1.08 | 1.13 | 0.28 | 0.97 | 1.85 | 1.61 | – | – | – |
| `oniguruma_features__lookbehind_alternation` | 14.3 µs | 4.12 | 1.45 | 0.88 | 6.59 | 5.58 | 1.80 | – | – | – |

#### micro

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 33.2 ns | 3.11 | 1.20 | 0.45 | 1.19 | 0.31 | 0.33 | – | 0.22 | – |
| `single_pattern__quantifier_greedy` | 43 ns | 3.47 | 1.56 | 0.51 | 0.97 | 1.14 | 1.13 | – | 1.07 | – |
| `single_pattern__alternation_2_branch` | 39.6 ns | 2.98 | 1.22 | 0.39 | 1.33 | 0.84 | 0.79 | – | 0.79 | – |
| `single_pattern__alternation_10_branch` | 25.2 ns | 6.18 | 14.66 | 0.76 | 7.86 | 0.69 | 0.67 | – | 0.49 | – |
| `single_pattern__case_insensitive_phrase` | 57.2 ns | 2.37 | 0.88 | 0.32 | 0.93 | 0.74 | 0.70 | – | 0.65 | – |
| `single_pattern__named_capture_date` | 62.5 ns | 3.38 | 1.76 | 0.72 | 4.59 | 0.52 | 0.51 | – | 0.46 | – |
| `single_pattern__unicode_greek` | 58.2 ns | 3.15 | 3.49 | 0.55 | 2.39 | 0.72 | 0.71 | – | 0.66 | – |
| `single_pattern__lookaround_combined` | 60 ns | 3.48 | 1.06 | 0.44 | 4.41 | 3.56 | 1.43 | – | – | – |
| `single_pattern__backref_simple` | 60.6 ns | 2.07 | 1.06 | 0.30 | 0.87 | 1.36 | 1.83 | – | – | – |
| `text_scanning__regset_position_lead` | 86.6 ns | 3.75 | – | – | – | – | – | – | – | – |
| `compilation__literal` | 636 ns | 0.56 | 0.48 | 3.24 | 0.48 | 3.70 | 3.65 | – | 2.92 | – |
| `compilation__named_capture` | 2.62 µs | 1.55 | 1.83 | 1.16 | 0.19 | 54.08 | 54.63 | – | 53.99 | – |
| `compilation__lookbehind` | 1.31 µs | 0.29 | 0.25 | 1.44 | 0.17 | 99.21 | 198.65 | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 16.7 ms | 0.83 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 10.3 ms | 1.50 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 390 µs | 0.37 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.91 µs | 51.15 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 167 µs | 48.45 | – | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 1.62 µs | 34.29 | – | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 94.7 µs | 23.71 | – | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 76.4 µs | 25.23 | – | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 38.7 µs | 17.31 | – | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 2.72 ms | 10.33 | n/a | n/a | n/a | 363.30 | 32.56 | 3.56 | – | 12.15 |
| `cpp_scanner__group_78` | 1.33 ms | 13.43 | n/a | n/a | n/a | 487.77 | 28.38 | 3.19 | – | 12.95 |
| `java_scanner__document` | 916 µs | 8.91 | 33.53 | 10.31 | 36.48 | 84.10 | 10.85 | 1.46 | – | 7.57 |
| `java_scanner__group_15` | 114 µs | 13.57 | 49.46 | 16.18 | 51.23 | 133.14 | 16.61 | 1.30 | – | 11.89 |
| `scss_scanner__document` | 1.17 ms | 30.68 | 18.49 | n/a | n/a | 111.01 | 10.40 | 1.99 | – | 2.73 |
| `scss_scanner__group_8` | 517 µs | 31.99 | 18.41 | n/a | n/a | 107.76 | 11.67 | 1.46 | – | 4.11 |
| `c_scanner__document` | 974 µs | 15.98 | 47.44 | 13.55 | 57.90 | 270.68 | 21.09 | 2.29 | – | 10.12 |
| `c_scanner__group_12` | 220 µs | 20.02 | 68.49 | 15.18 | 88.32 | 375.35 | 27.53 | 3.37 | – | 12.13 |
| `php_scanner__document` | 1.64 ms | 10.47 | 30.22 | 5.40 | 13.01 | 57.16 | 13.07 | 1.13 | – | 3.16 |
| `php_scanner__group_13` | 195 µs | 20.13 | 74.67 | 10.06 | 29.37 | 132.07 | 25.61 | 2.17 | – | 7.11 |
| `json_scanner__document` | 243 µs | 6.15 | 6.22 | 2.00 | 3.28 | 9.23 | 5.09 | 2.92 | – | 3.38 |
| `json_scanner__group_1` | 80.1 µs | 7.45 | 6.56 | 2.46 | 3.85 | 10.11 | 4.83 | 2.24 | – | 3.37 |
| `astro_scanner__document` | 1.65 ms | 7.11 | n/a | n/a | n/a | n/a | n/a | n/a | – | 7.39 |
| `astro_scanner__group_22` | 131 µs | 4.59 | 4.65 | 2.99 | 5.54 | n/a | n/a | n/a | – | 3.28 |

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

