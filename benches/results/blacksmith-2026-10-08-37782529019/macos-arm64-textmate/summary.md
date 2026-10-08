### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `62892008bbf5`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 12.2 ms | 1.19 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 8.32 ms | 1.93 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 276 µs | 0.55 | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 2.04 µs | 72.67 | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 165 µs | 51.08 | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 3.96 µs | 13.74 | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 520 µs | 4.25 | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 80.7 µs | 25.93 | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 78.5 µs | 8.59 | – | – | – | – | – | – |
| `cpp_scanner__document` | 11.9 ms | 2.47 | n/a | n/a | n/a | 81.73 | – | 2.72 |
| `cpp_scanner__group_78` | 5.17 ms | 3.15 | n/a | n/a | n/a | 133.78 | – | 3.34 |
| `java_scanner__document` | 1.95 ms | 4.50 | 17.15 | 5.63 | 18.47 | 42.58 | – | 3.51 |
| `java_scanner__group_15` | 439 µs | 3.61 | 13.25 | 4.12 | 14.05 | 33.19 | – | 3.58 |
| `scss_scanner__document` | 2.12 ms | 14.92 | 9.76 | n/a | n/a | 56.25 | – | 1.72 |
| `scss_scanner__group_8` | 852 µs | 19.37 | 10.80 | n/a | n/a | 65.69 | – | 2.54 |
| `c_scanner__document` | 3.77 ms | 4.16 | 12.72 | 3.56 | 14.09 | 72.41 | – | 2.54 |
| `c_scanner__group_12` | 884 µs | 4.72 | 17.24 | 3.52 | 20.72 | 89.28 | – | 2.65 |
| `php_scanner__document` | 5.51 ms | 2.81 | 8.12 | 1.37 | 3.26 | 14.41 | – | 0.87 |
| `php_scanner__group_13` | 644 µs | 5.57 | 21.15 | 2.89 | 8.02 | 36.10 | – | 1.94 |

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

