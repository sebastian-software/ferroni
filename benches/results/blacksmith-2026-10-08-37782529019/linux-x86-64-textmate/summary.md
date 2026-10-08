### Engine comparison: linux-x86-64 (AMD EPYC)

Source `62892008bbf5`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 11.8 ms | 0.81 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 8.95 ms | 1.57 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 352 µs | 0.52 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.16 µs | 65.18 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 143 µs | 76.59 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 4.15 µs | 15.18 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 649 µs | 3.59 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 77.4 µs | 31.16 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 78.7 µs | 10.42 | – | – | – | – | – | – |
| `cpp_scanner__document` | 12.4 ms | 2.39 | n/a | n/a | n/a | 68.13 | – | 2.62 |
| `cpp_scanner__group_78` | 6.03 ms | 3.03 | n/a | n/a | n/a | 95.12 | – | 2.95 |
| `java_scanner__document` | 1.98 ms | 4.49 | 15.70 | 5.31 | 16.99 | 40.64 | – | 3.30 |
| `java_scanner__group_15` | 488 µs | 3.64 | 12.86 | 3.65 | 13.55 | 32.76 | – | 3.36 |
| `scss_scanner__document` | 2.27 ms | 15.42 | 9.12 | n/a | n/a | 49.28 | – | 1.96 |
| `scss_scanner__group_8` | 939 µs | 17.78 | 9.66 | n/a | n/a | 54.45 | – | 2.81 |
| `c_scanner__document` | 4.24 ms | 4.04 | 10.57 | 4.31 | 11.06 | 55.76 | – | 2.52 |
| `c_scanner__group_12` | 938 µs | 4.49 | 14.86 | 3.60 | 16.06 | 76.37 | – | 2.75 |
| `php_scanner__document` | 5.72 ms | 3.09 | 8.15 | 1.68 | 3.33 | 14.76 | – | 0.99 |
| `php_scanner__group_13` | 639 µs | 5.92 | 21.30 | 3.36 | 8.12 | 38.05 | – | 2.34 |

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

