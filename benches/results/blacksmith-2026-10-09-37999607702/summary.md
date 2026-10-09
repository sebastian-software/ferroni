### Engine comparison: linux-x86-64 (AMD EPYC)

Source `b93c6d2035f4`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 20.3 ms | 0.48 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 12.7 ms | 1.13 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 496 µs | 0.37 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 3.03 µs | 52.46 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 143 µs | 74.37 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 1.79 µs | 33.34 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 93.8 µs | 25.28 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 75.5 µs | 32.33 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 43.9 µs | 18.59 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 3.25 ms | 9.76 | n/a | n/a | n/a | 294.74 | 19.64 | 2.58 | 9.70 |
| `cpp_scanner__group_78` | 1.19 ms | 14.41 | n/a | n/a | n/a | 475.53 | 25.88 | 2.73 | 14.60 |
| `java_scanner__document` | 939 µs | 9.96 | 31.93 | 10.48 | 36.55 | 91.64 | 10.92 | 1.30 | 6.97 |
| `java_scanner__group_15` | 145 µs | 11.32 | 38.09 | 12.11 | 40.14 | 99.48 | 12.63 | 0.99 | 11.16 |
| `scss_scanner__document` | 1.26 ms | 28.75 | 16.72 | n/a | n/a | 112.66 | 10.58 | 1.68 | 3.46 |
| `scss_scanner__group_8` | 603 µs | 27.81 | 15.96 | n/a | n/a | 89.45 | 10.80 | 1.37 | 4.52 |
| `c_scanner__document` | 1.02 ms | 17.58 | 40.95 | 18.82 | 44.76 | 244.49 | 22.14 | 1.98 | 10.89 |
| `c_scanner__group_12` | 245 µs | 16.65 | 54.15 | 13.11 | 61.95 | 281.09 | 24.49 | 2.91 | 10.58 |
| `php_scanner__document` | 1.43 ms | 12.08 | 32.30 | 6.78 | 13.45 | 56.36 | 13.28 | 1.21 | 3.63 |
| `php_scanner__group_13` | 202 µs | 18.51 | 62.68 | 10.27 | 26.22 | 119.13 | 21.54 | 2.20 | 7.23 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
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

### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `b93c6d2035f4`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 16.1 ms | 0.83 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 10.5 ms | 1.53 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 384 µs | 0.36 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.64 µs | 50.26 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 149 µs | 48.61 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 1.54 µs | 33.12 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 83.3 µs | 23.22 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 72.6 µs | 27.98 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 42.1 µs | 15.84 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 2.81 ms | 10.35 | n/a | n/a | n/a | 338.27 | 24.42 | 3.79 | 12.01 |
| `cpp_scanner__group_78` | 1.4 ms | 12.63 | n/a | n/a | n/a | 473.55 | 27.87 | 3.12 | 14.08 |
| `java_scanner__document` | 1.08 ms | 8.54 | 29.52 | 9.96 | 33.67 | 78.27 | 10.17 | 1.32 | 6.74 |
| `java_scanner__group_15` | 124 µs | 13.20 | 48.90 | 14.88 | 52.26 | 126.09 | 14.65 | 1.18 | 11.86 |
| `scss_scanner__document` | 1.36 ms | 25.50 | 16.52 | n/a | n/a | 98.19 | 9.87 | 1.76 | 2.67 |
| `scss_scanner__group_8` | 536 µs | 32.69 | 19.23 | n/a | n/a | 116.31 | 12.74 | 1.57 | 4.47 |
| `c_scanner__document` | 1.09 ms | 14.52 | 43.96 | 12.61 | 49.26 | 248.78 | 22.12 | 2.25 | 9.90 |
| `c_scanner__group_12` | 196 µs | 19.83 | 83.93 | 15.22 | 89.33 | 381.76 | 28.12 | 3.73 | 11.79 |
| `php_scanner__document` | 1.4 ms | 10.08 | 29.39 | 4.87 | 11.95 | 50.83 | 10.84 | 1.09 | 2.92 |
| `php_scanner__group_13` | 166 µs | 19.60 | 91.53 | 10.09 | 28.50 | 130.03 | 26.48 | 2.50 | 6.76 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
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

