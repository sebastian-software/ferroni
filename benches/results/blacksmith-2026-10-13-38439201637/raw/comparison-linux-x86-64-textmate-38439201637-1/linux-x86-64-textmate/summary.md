### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 20.6 ms | 0.52 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 13.4 ms | 1.20 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 592 µs | 0.33 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 3.24 µs | 48.55 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 152 µs | 75.18 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.14 µs | 32.13 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 116 µs | 24.83 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 125 µs | 26.20 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 67.6 µs | 16.60 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 4.73 ms | 8.77 | n/a | n/a | n/a | 333.89 | 39.58 | 2.93 | 9.74 |
| `cpp_scanner__group_78` | 1.89 ms | 13.47 | n/a | n/a | n/a | 491.98 | 24.11 | 2.80 | 17.52 |
| `java_scanner__document` | 1.69 ms | 8.48 | 26.14 | 12.34 | 37.85 | 77.95 | 9.56 | 1.07 | 6.80 |
| `java_scanner__group_15` | 201 µs | 10.46 | 41.08 | 12.03 | 42.16 | 98.32 | 12.87 | 1.16 | 10.84 |
| `scss_scanner__document` | 1.71 ms | 27.06 | 16.74 | n/a | n/a | 107.89 | 9.53 | 1.63 | 3.49 |
| `scss_scanner__group_8` | 657 µs | 32.49 | 19.76 | n/a | n/a | 120.20 | 12.21 | 1.77 | 5.44 |
| `c_scanner__document` | 1.28 ms | 16.61 | 39.19 | 17.14 | 47.45 | 274.80 | 20.84 | 1.96 | 10.81 |
| `c_scanner__group_12` | 282 µs | 16.26 | 57.47 | 12.95 | 62.82 | 308.03 | 26.06 | 3.12 | 10.25 |
| `php_scanner__document` | 1.86 ms | 11.75 | 29.51 | 7.03 | 12.18 | 57.22 | 12.57 | 1.08 | 3.76 |
| `php_scanner__group_13` | 271 µs | 17.16 | 67.96 | 9.40 | 25.64 | 118.71 | 19.80 | 2.00 | 7.21 |
| `json_scanner__document` | 294 µs | 4.25 | 5.62 | 1.69 | 3.04 | 9.17 | 5.52 | 3.10 | 3.86 |
| `json_scanner__group_1` | 91 µs | 4.90 | 5.99 | 2.35 | 4.04 | 10.68 | 5.67 | 2.55 | 3.41 |
| `astro_scanner__document` | 2.07 ms | 6.45 | n/a | n/a | n/a | n/a | n/a | n/a | 7.13 |
| `astro_scanner__group_22` | 143 µs | 4.14 | 4.60 | 2.62 | 3.38 | n/a | n/a | n/a | 4.62 |

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

