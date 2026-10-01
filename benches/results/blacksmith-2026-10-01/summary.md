### Engine comparison: linux-x86-64 (AMD EPYC)

Source `11a7ba0248c9`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### trivial

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 41.8 ns | 2.08 | 0.79 | 0.46 | 1.15 | 0.37 | 0.26 | – |
| `single_pattern__quantifier_greedy` | 42.8 ns | 2.72 | 1.58 | 0.56 | 1.13 | 1.36 | 1.39 | – |
| `single_pattern__alternation_2_branch` | 43.1 ns | 2.00 | 0.73 | 0.44 | 1.24 | 1.09 | 0.91 | – |
| `single_pattern__alternation_10_branch` | 32.6 ns | 4.49 | 3.38 | 0.68 | 4.35 | 0.67 | 0.52 | – |
| `single_pattern__case_insensitive_phrase` | 90.3 ns | 1.34 | 0.70 | 0.23 | 0.59 | 0.66 | 0.63 | – |
| `single_pattern__named_capture_date` | 82.1 ns | 2.71 | 1.32 | 0.51 | 3.78 | 0.53 | 0.52 | – |
| `single_pattern__unicode_greek` | 73.7 ns | 1.92 | 2.03 | 0.40 | 2.27 | 0.71 | 0.67 | – |
| `text_scanning__literal_50k` | 33.6 ns | 2.18 | 0.69 | 0.49 | 1.11 | 0.37 | 0.26 | – |
| `text_scanning__no_match_50k` | 617 ns | 10.34 | 11.10 | 1.45 | 0.37 | 0.99 | 0.99 | – |
| `text_scanning__field_extract_50k` | 45.1 ns | 2.03 | 1.02 | 0.50 | 1.32 | 1.09 | 0.98 | – |
| `text_scanning__timestamp_50k` | 68.5 ns | 1.30 | 1.79 | 0.41 | 0.92 | 0.83 | 0.73 | – |
| `general_regex__email_validation` | 5.41 µs | 1.21 | 1.71 | 0.27 | 0.85 | 0.30 | 0.21 | – |
| `general_regex__uuid_validation` | 5.09 µs | 1.98 | 2.94 | 0.42 | 0.89 | 0.44 | 0.35 | – |
| `general_regex__number_validation` | 4.93 µs | 1.61 | 1.70 | 0.38 | 1.98 | 0.26 | 0.18 | – |
| `general_regex__access_log_captures` | 22 µs | 1.11 | 1.52 | 0.23 | 0.79 | 1.59 | 1.44 | – |
| `general_regex__url_extraction` | 8.26 µs | 1.72 | 2.07 | 0.49 | 1.17 | 1.65 | 1.23 | – |
| `general_regex__unicode_words` | 43.3 µs | 1.58 | 1.45 | 0.39 | 1.20 | 1.34 | 0.67 | – |
| `general_regex__email_redaction` | 23.3 µs | 3.19 | 4.73 | 0.46 | 2.58 | 0.86 | 0.57 | – |
| `compilation__literal` | 640 ns | 0.61 | 0.55 | 7.31 | 0.65 | 4.31 | 3.32 | – |
| `compilation__named_capture` | 2.74 µs | 1.36 | 1.40 | 2.12 | 0.20 | 58.67 | 62.55 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__lookaround_combined` | 54.9 ns | 3.65 | 1.01 | 0.48 | 4.52 | 3.61 | – | – |
| `single_pattern__backref_simple` | 50.3 ns | 2.17 | 1.12 | 0.41 | 1.24 | 2.08 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 26.5 µs | 1.27 | 1.06 | 0.31 | 0.84 | 2.48 | – | – |
| `oniguruma_features__subexp_call_balanced` | 34.1 µs | 1.09 | 1.27 | 0.31 | 1.79 | n/a | – | – |
| `oniguruma_features__absent_comments` | 27.1 µs | 0.85 | 0.67 | n/a | n/a | 1.50 | – | – |
| `oniguruma_features__conditional_brackets` | 49.4 µs | 1.07 | 1.13 | 0.28 | 0.91 | 2.41 | – | – |
| `oniguruma_features__backref_ignorecase` | 68.6 µs | 1.10 | 1.04 | 0.32 | 1.16 | 1.92 | – | – |
| `oniguruma_features__lookbehind_alternation` | 17.9 µs | 3.00 | 1.12 | 0.62 | 3.50 | 3.88 | – | – |
| `compilation__lookbehind` | 1.2 µs | 0.42 | 0.28 | 2.84 | 0.18 | 128.48 | – | – |
| `text_scanning__regset_position_lead` | 59.2 ns | 3.42 | – | – | – | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 14.2 ms | 0.74 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 8.98 ms | 1.46 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 311 µs | 0.59 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.23 µs | 61.88 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 168 µs | 61.04 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 4.84 µs | 13.10 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 731 µs | 3.25 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 71.6 µs | 32.80 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 73.7 µs | 10.70 | – | – | – | – | – | – |
| `cpp_scanner__document` | 13.5 ms | 2.13 | n/a | n/a | n/a | 91.72 | – | 3.28 |
| `cpp_scanner__group_78` | 6.4 ms | 2.64 | n/a | n/a | n/a | 96.72 | – | 2.97 |
| `java_scanner__document` | 1.98 ms | 4.35 | 15.42 | 4.98 | 15.40 | 39.39 | – | 3.46 |
| `java_scanner__group_15` | 450 µs | 3.44 | 12.34 | 3.87 | 14.29 | 32.26 | – | 3.30 |
| `scss_scanner__document` | 2.82 ms | 17.22 | 7.48 | n/a | n/a | 69.16 | – | 1.55 |
| `scss_scanner__group_8` | 1.2 ms | 13.42 | 7.73 | n/a | n/a | 48.68 | – | 2.55 |

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
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `11a7ba0248c9`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### trivial

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__literal_exact` | 36.1 ns | 3.04 | 1.05 | 0.42 | 1.17 | 0.32 | 0.21 | – |
| `single_pattern__quantifier_greedy` | 47 ns | 3.36 | 1.42 | 0.44 | 0.92 | 1.10 | 0.99 | – |
| `single_pattern__alternation_2_branch` | 42.3 ns | 2.84 | 0.86 | 0.39 | 1.36 | 0.84 | 0.76 | – |
| `single_pattern__alternation_10_branch` | 28.5 ns | 6.22 | 4.71 | 0.78 | 7.74 | 0.67 | 0.52 | – |
| `single_pattern__case_insensitive_phrase` | 70.1 ns | 2.11 | 0.81 | 0.27 | 0.76 | 0.61 | 0.56 | – |
| `single_pattern__named_capture_date` | 67.8 ns | 3.71 | 1.74 | 0.75 | 4.50 | 0.52 | 0.49 | – |
| `single_pattern__unicode_greek` | 63.1 ns | 2.98 | 2.47 | 0.47 | 2.09 | 0.62 | 0.61 | – |
| `text_scanning__literal_50k` | 39.1 ns | 2.82 | 0.78 | 0.41 | 0.96 | 0.30 | 0.21 | – |
| `text_scanning__no_match_50k` | 1.17 µs | 5.79 | 5.32 | 1.22 | 0.79 | 0.96 | 0.99 | – |
| `text_scanning__field_extract_50k` | 41 ns | 2.76 | 0.98 | 0.48 | 1.26 | 0.81 | 0.83 | – |
| `text_scanning__timestamp_50k` | 58.3 ns | 1.91 | 1.87 | 0.44 | 1.01 | 0.75 | 0.61 | – |
| `general_regex__email_validation` | 4.41 µs | 1.95 | 2.36 | 0.33 | 1.20 | 0.34 | 0.21 | – |
| `general_regex__uuid_validation` | 3.68 µs | 3.17 | 3.56 | 0.42 | 0.87 | 0.42 | 0.35 | – |
| `general_regex__number_validation` | 4.29 µs | 2.23 | 1.79 | 0.37 | 2.48 | 0.24 | 0.17 | – |
| `general_regex__access_log_captures` | 19.5 µs | 1.26 | 1.68 | 0.28 | 0.82 | 1.66 | 1.33 | – |
| `general_regex__url_extraction` | 6.86 µs | 2.46 | 2.26 | 0.49 | 1.21 | 1.70 | 1.41 | – |
| `general_regex__unicode_words` | 32.3 µs | 2.12 | 1.17 | 0.40 | 1.07 | 1.02 | 0.70 | – |
| `general_regex__email_redaction` | 14.5 µs | 4.58 | 7.40 | 0.67 | 4.17 | 0.93 | 0.73 | – |
| `compilation__literal` | 508 ns | 0.67 | 0.62 | 3.91 | 0.65 | 4.69 | 3.56 | – |
| `compilation__named_capture` | 2.61 µs | 1.70 | 1.98 | 1.26 | 0.20 | 61.98 | 57.38 | – |

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__lookaround_combined` | 54.2 ns | 3.60 | 1.03 | 0.45 | 4.72 | 3.86 | – | – |
| `single_pattern__backref_simple` | 52.1 ns | 2.54 | 1.23 | 0.37 | 0.99 | 1.59 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 25 µs | 1.40 | 1.13 | 0.33 | 1.13 | 3.16 | – | – |
| `oniguruma_features__subexp_call_balanced` | 35.9 µs | 1.24 | 1.55 | 0.31 | 1.70 | n/a | – | – |
| `oniguruma_features__absent_comments` | 28.9 µs | 1.10 | 0.66 | n/a | n/a | 1.50 | – | – |
| `oniguruma_features__conditional_brackets` | 50.5 µs | 1.06 | 1.24 | 0.29 | 1.01 | 2.77 | – | – |
| `oniguruma_features__backref_ignorecase` | 86.2 µs | 0.94 | 1.04 | 0.25 | 0.92 | 1.62 | – | – |
| `oniguruma_features__lookbehind_alternation` | 18.2 µs | 3.07 | 1.16 | 0.66 | 5.37 | 4.52 | – | – |
| `compilation__lookbehind` | 1.27 µs | 0.36 | 0.29 | 1.69 | 0.20 | 115.44 | – | – |
| `text_scanning__regset_position_lead` | 75.2 ns | 4.17 | – | – | – | – | – | – |

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 10.8 ms | 1.30 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 7.52 ms | 2.18 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 270 µs | 0.58 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.21 µs | 68.10 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 167 µs | 51.39 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 4.13 µs | 15.10 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 608 µs | 4.02 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 79 µs | 26.41 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 80.8 µs | 8.69 | – | – | – | – | – | – |
| `cpp_scanner__document` | 12.5 ms | 2.52 | n/a | n/a | n/a | 86.00 | – | 2.87 |
| `cpp_scanner__group_78` | 5.26 ms | 3.07 | n/a | n/a | n/a | 122.77 | – | 3.57 |
| `java_scanner__document` | 2.06 ms | 4.20 | 16.56 | 5.00 | 17.62 | 40.44 | – | 3.27 |
| `java_scanner__group_15` | 473 µs | 3.71 | 12.27 | 4.14 | 14.24 | 32.42 | – | 3.17 |
| `scss_scanner__document` | 2.02 ms | 14.84 | 9.43 | n/a | n/a | 53.58 | – | 1.67 |
| `scss_scanner__group_8` | 755 µs | 20.62 | 11.38 | n/a | n/a | 66.43 | – | 2.58 |

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
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

