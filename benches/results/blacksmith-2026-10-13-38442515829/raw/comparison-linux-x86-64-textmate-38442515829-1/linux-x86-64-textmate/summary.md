### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 19.8 ms | 0.54 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 13.5 ms | 1.13 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 566 µs | 0.32 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 3.75 µs | 40.52 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 151 µs | 68.83 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.42 µs | 28.82 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 113 µs | 22.50 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 97.5 µs | 27.44 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 50.4 µs | 18.48 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 4.55 ms | 8.67 | n/a | n/a | n/a | 252.93 | 34.62 | 2.61 | 8.82 |
| `cpp_scanner__group_78` | 1.22 ms | 13.98 | n/a | n/a | n/a | 556.65 | 31.44 | 3.62 | 18.12 |
| `java_scanner__document` | 1.1 ms | 9.30 | 29.19 | 11.02 | 37.13 | 93.31 | 10.41 | 1.26 | 9.13 |
| `java_scanner__group_15` | 142 µs | 13.19 | 41.96 | 13.13 | 48.12 | 117.09 | 13.55 | 1.08 | 15.40 |
| `scss_scanner__document` | 1.29 ms | 30.15 | 16.60 | n/a | n/a | 135.02 | 11.22 | 1.87 | 3.41 |
| `scss_scanner__group_8` | 537 µs | 30.97 | 17.74 | n/a | n/a | 108.96 | 11.41 | 1.56 | 4.95 |
| `c_scanner__document` | 1.09 ms | 16.69 | 43.10 | 17.30 | 46.38 | 333.44 | 25.68 | 2.45 | 10.88 |
| `c_scanner__group_12` | 253 µs | 16.55 | 57.05 | 13.40 | 60.77 | 284.23 | 22.08 | 2.84 | 12.00 |
| `php_scanner__document` | 1.62 ms | 12.00 | 31.41 | 6.36 | 11.42 | 61.79 | 12.66 | 1.10 | 3.70 |
| `php_scanner__group_13` | 193 µs | 20.33 | 78.02 | 12.78 | 32.55 | 168.91 | 22.96 | 2.38 | 7.82 |
| `json_scanner__document` | 237 µs | 4.26 | 5.80 | 1.86 | 3.25 | 10.87 | 6.27 | 3.51 | 3.58 |
| `json_scanner__group_1` | 70.5 µs | 4.99 | 6.67 | 2.35 | 3.71 | 11.34 | 6.03 | 2.89 | 3.51 |
| `astro_scanner__document` | 1.69 ms | 6.55 | n/a | n/a | n/a | n/a | n/a | n/a | 7.75 |
| `astro_scanner__group_22` | 157 µs | 4.29 | 3.86 | 3.42 | 3.65 | n/a | n/a | n/a | 3.91 |

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

