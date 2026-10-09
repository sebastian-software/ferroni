### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `a38768bc8012`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 17.5 ms | 0.85 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 11.7 ms | 1.44 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 411 µs | 0.38 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 6.84 µs | 23.41 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 14 ms | 0.60 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.23 µs | 25.28 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 92.3 µs | 26.55 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 11.1 ms | 0.18 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 41.1 µs | 17.63 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 3.14 ms | 9.95 | n/a | n/a | n/a | 326.37 | 24.93 | 3.24 | 11.73 |
| `cpp_scanner__group_78` | 1.42 ms | 13.32 | n/a | n/a | n/a | 493.61 | 26.26 | 2.90 | 13.62 |
| `java_scanner__document` | 891 µs | 8.34 | 30.51 | 9.71 | 33.49 | 76.31 | 9.27 | 1.36 | 6.88 |
| `java_scanner__group_15` | 106 µs | 13.25 | 58.15 | 16.97 | 63.73 | 146.93 | 17.33 | 1.35 | 11.58 |
| `scss_scanner__document` | 1.32 ms | 24.24 | 16.28 | n/a | n/a | 94.46 | 8.51 | 1.60 | 2.87 |
| `scss_scanner__group_8` | 526 µs | 33.14 | 18.33 | n/a | n/a | 119.07 | 11.46 | 1.45 | 4.70 |
| `c_scanner__document` | 920 µs | 15.46 | 49.66 | 12.78 | 62.85 | 308.00 | 21.89 | 2.45 | 10.78 |
| `c_scanner__group_12` | 199 µs | 20.77 | 70.38 | 15.34 | 93.00 | 397.19 | 26.06 | 3.34 | 12.37 |
| `php_scanner__document` | 1.62 ms | 10.04 | 29.16 | 4.98 | 11.07 | 50.29 | 9.05 | 0.95 | 2.77 |
| `php_scanner__group_13` | 200 µs | 19.20 | 63.48 | 9.81 | 25.41 | 113.11 | 19.91 | 1.82 | 6.43 |

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

