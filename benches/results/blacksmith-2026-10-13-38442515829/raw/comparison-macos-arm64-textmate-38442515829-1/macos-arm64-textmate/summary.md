### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 16.7 ms | 0.83 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 10.3 ms | 1.50 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 390 µs | 0.37 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.91 µs | 51.15 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 167 µs | 48.45 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 1.62 µs | 34.29 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 94.7 µs | 23.71 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 76.4 µs | 25.23 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 38.7 µs | 17.31 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 2.72 ms | 10.33 | n/a | n/a | n/a | 363.30 | 32.56 | 3.56 | 12.15 |
| `cpp_scanner__group_78` | 1.33 ms | 13.43 | n/a | n/a | n/a | 487.77 | 28.38 | 3.19 | 12.95 |
| `java_scanner__document` | 916 µs | 8.91 | 33.53 | 10.31 | 36.48 | 84.10 | 10.85 | 1.46 | 7.57 |
| `java_scanner__group_15` | 114 µs | 13.57 | 49.46 | 16.18 | 51.23 | 133.14 | 16.61 | 1.30 | 11.89 |
| `scss_scanner__document` | 1.17 ms | 30.68 | 18.49 | n/a | n/a | 111.01 | 10.40 | 1.99 | 2.73 |
| `scss_scanner__group_8` | 517 µs | 31.99 | 18.41 | n/a | n/a | 107.76 | 11.67 | 1.46 | 4.11 |
| `c_scanner__document` | 974 µs | 15.98 | 47.44 | 13.55 | 57.90 | 270.68 | 21.09 | 2.29 | 10.12 |
| `c_scanner__group_12` | 220 µs | 20.02 | 68.49 | 15.18 | 88.32 | 375.35 | 27.53 | 3.37 | 12.13 |
| `php_scanner__document` | 1.64 ms | 10.47 | 30.22 | 5.40 | 13.01 | 57.16 | 13.07 | 1.13 | 3.16 |
| `php_scanner__group_13` | 195 µs | 20.13 | 74.67 | 10.06 | 29.37 | 132.07 | 25.61 | 2.17 | 7.11 |
| `json_scanner__document` | 243 µs | 6.15 | 6.22 | 2.00 | 3.28 | 9.23 | 5.09 | 2.92 | 3.38 |
| `json_scanner__group_1` | 80.1 µs | 7.45 | 6.56 | 2.46 | 3.85 | 10.11 | 4.83 | 2.24 | 3.37 |
| `astro_scanner__document` | 1.65 ms | 7.11 | n/a | n/a | n/a | n/a | n/a | n/a | 7.39 |
| `astro_scanner__group_22` | 131 µs | 4.59 | 4.65 | 2.99 | 5.54 | n/a | n/a | n/a | 3.28 |

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
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

