### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `6db6413e84b0`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 10.9 ms | 1.32 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 7.17 ms | 2.27 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 279 µs | 0.55 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.16 µs | 70.51 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 153 µs | 52.79 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 3.9 µs | 15.05 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 579 µs | 3.95 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 67.3 µs | 28.78 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 77 µs | 9.50 | – | – | – | – | – | – |
| `cpp_scanner__document` | 11.3 ms | 2.54 | n/a | n/a | n/a | 90.92 | – | 3.22 |
| `cpp_scanner__group_78` | 5.11 ms | 3.25 | n/a | n/a | n/a | 122.13 | – | 3.41 |
| `java_scanner__document` | 1.79 ms | 4.45 | 18.33 | 5.80 | 18.37 | 43.29 | – | 3.43 |
| `java_scanner__group_15` | 478 µs | 3.58 | 12.77 | 3.97 | 13.85 | 32.79 | – | 3.29 |
| `scss_scanner__document` | 2.2 ms | 14.77 | 9.20 | n/a | n/a | 54.94 | – | 1.63 |
| `scss_scanner__group_8` | 860 µs | 20.81 | 10.30 | n/a | n/a | 65.04 | – | 2.61 |
| `c_scanner__document` | 3.18 ms | 4.14 | 13.21 | 3.40 | 14.99 | 73.97 | – | 2.67 |
| `c_scanner__group_12` | 735 µs | 5.03 | 18.59 | 3.71 | 22.66 | 98.19 | – | 2.95 |
| `php_scanner__document` | 5.13 ms | 2.71 | 8.07 | 1.33 | 3.25 | 13.57 | – | 0.78 |
| `php_scanner__group_13` | 580 µs | 5.64 | 21.92 | 2.98 | 8.17 | 36.66 | – | 1.95 |

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

