### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `11a7ba0248c9`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

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
| `scss_scanner/document_pcre2` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/document_pcre2_jit` | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| `scss_scanner/group_8_pcre2` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| `scss_scanner/group_8_pcre2_jit` | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |

Accepted with empty capture groups reported as unset (or the reverse), which vscode-textmate skips either way: `cpp_scanner/document_shiki_js` (96 calls), `java_scanner/document_shiki_js` (16 calls), `java_scanner/group_15_shiki_js` (16 calls)

