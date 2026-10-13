### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 16.9 ms | 0.85 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 11.4 ms | 1.42 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 380 µs | 0.39 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.94 µs | 52.44 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 162 µs | 51.18 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 1.69 µs | 32.31 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 89.2 µs | 25.67 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 76.7 µs | 25.30 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 39.8 µs | 16.56 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 3.08 ms | 9.08 | n/a | n/a | n/a | 306.69 | 26.47 | 3.36 | 10.25 |
| `cpp_scanner__group_78` | 1.37 ms | 12.70 | n/a | n/a | n/a | 490.87 | 27.81 | 3.00 | 13.88 |
| `java_scanner__document` | 901 µs | 9.38 | 34.90 | 11.79 | 35.65 | 80.16 | 9.70 | 1.60 | 6.97 |
| `java_scanner__group_15` | 123 µs | 12.54 | 50.02 | 15.33 | 48.76 | 118.29 | 13.93 | 1.14 | 11.57 |
| `scss_scanner__document` | 1.35 ms | 26.11 | 15.70 | n/a | n/a | 95.44 | 9.36 | 1.65 | 2.82 |
| `scss_scanner__group_8` | 573 µs | 32.12 | 17.92 | n/a | n/a | 111.52 | 12.14 | 1.45 | 3.83 |
| `c_scanner__document` | 1.04 ms | 15.23 | 47.66 | 13.18 | 51.49 | 254.73 | 20.62 | 2.21 | 10.14 |
| `c_scanner__group_12` | 224 µs | 19.59 | 69.27 | 15.37 | 89.13 | 377.56 | 26.72 | 3.21 | 11.90 |
| `php_scanner__document` | 1.61 ms | 10.48 | 29.58 | 5.16 | 12.50 | 54.26 | 11.90 | 1.13 | 3.08 |
| `php_scanner__group_13` | 193 µs | 21.03 | 65.56 | 10.80 | 31.09 | 140.12 | 24.56 | 1.89 | 6.97 |
| `json_scanner__document` | 214 µs | 7.02 | 7.14 | 2.41 | 3.79 | 10.71 | 5.77 | 3.38 | 3.32 |
| `json_scanner__group_1` | 74.3 µs | 6.57 | 6.90 | 2.44 | 4.42 | 11.06 | 5.00 | 2.30 | 3.34 |
| `astro_scanner__document` | 1.54 ms | 6.57 | n/a | n/a | n/a | n/a | n/a | n/a | 7.83 |
| `astro_scanner__group_22` | 127 µs | 4.21 | 4.94 | 2.75 | 6.11 | n/a | n/a | n/a | 3.23 |

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

