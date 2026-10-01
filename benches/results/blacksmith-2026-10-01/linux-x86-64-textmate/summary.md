### Engine comparison: linux-x86-64 (AMD EPYC)

Source `11a7ba0248c9`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

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
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

