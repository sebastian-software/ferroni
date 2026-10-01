### Engine comparison: linux-x86-64 (AMD EPYC)

Source `6db6413e84b0`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 11.5 ms | 0.82 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 7.61 ms | 1.71 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 301 µs | 0.58 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.15 µs | 62.44 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 128 µs | 76.64 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 4.58 µs | 13.92 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 699 µs | 3.34 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 73.5 µs | 31.54 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 79.2 µs | 10.31 | – | – | – | – | – | – |
| `cpp_scanner__document` | 12.6 ms | 2.19 | n/a | n/a | n/a | 74.64 | – | 2.58 |
| `cpp_scanner__group_78` | 5.87 ms | 2.94 | n/a | n/a | n/a | 108.25 | – | 3.14 |
| `java_scanner__document` | 2.64 ms | 3.66 | 11.35 | 4.12 | 12.48 | 30.95 | – | 3.06 |
| `java_scanner__group_15` | 466 µs | 3.41 | 12.70 | 3.75 | 12.81 | 33.35 | – | 3.01 |
| `scss_scanner__document` | 1.97 ms | 16.74 | 10.27 | n/a | n/a | 60.54 | – | 2.06 |
| `scss_scanner__group_8` | 840 µs | 17.75 | 10.33 | n/a | n/a | 57.94 | – | 2.84 |
| `c_scanner__document` | 3.83 ms | 4.15 | 12.62 | 4.72 | 15.20 | 83.26 | – | 2.44 |
| `c_scanner__group_12` | 932 µs | 4.29 | 15.24 | 3.59 | 17.19 | 88.41 | – | 3.34 |
| `php_scanner__document` | 5.18 ms | 3.16 | 8.23 | 1.73 | 3.51 | 17.88 | – | 1.00 |
| `php_scanner__group_13` | 604 µs | 5.71 | 22.12 | 3.34 | 8.19 | 42.31 | – | 2.24 |

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

