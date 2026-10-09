### Engine comparison: linux-x86-64 (AMD EPYC)

Source `a38768bc8012`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### textmate

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `scanner_highlighting__ts_279_compile` | 25 ms | 0.53 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_compile` | 14.6 ms | 0.99 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_compile` | 596 µs | 0.35 | – | – | – | – | – | – | – |
| `scanner_highlighting__ts_279_tokenize` | 7.9 µs | 18.62 | – | – | – | – | – | – | – |
| `scanner_highlighting__css_117_tokenize` | 16 ms | 0.67 | – | – | – | – | – | – | – |
| `scanner_highlighting__rust_81_tokenize` | 2.86 µs | 21.62 | – | – | – | – | – | – | – |
| `scanner_documents__ts_279_document_28_lines` | 105 µs | 22.40 | – | – | – | – | – | – | – |
| `scanner_documents__css_117_document_19_lines` | 13.4 ms | 0.21 | – | – | – | – | – | – | – |
| `scanner_documents__rust_81_document_31_lines` | 48.5 µs | 19.43 | – | – | – | – | – | – | – |
| `cpp_scanner__document` | 4.45 ms | 8.36 | n/a | n/a | n/a | 262.60 | 30.15 | 2.31 | 10.21 |
| `cpp_scanner__group_78` | 1.25 ms | 14.20 | n/a | n/a | n/a | 508.55 | 25.99 | 2.88 | 15.35 |
| `java_scanner__document` | 1.14 ms | 8.56 | 31.72 | 9.76 | 28.77 | 78.35 | 10.93 | 1.59 | 6.69 |
| `java_scanner__group_15` | 137 µs | 12.66 | 44.26 | 13.14 | 43.18 | 104.91 | 13.32 | 1.09 | 11.98 |
| `scss_scanner__document` | 1.16 ms | 29.39 | 18.61 | n/a | n/a | 121.05 | 10.58 | 1.83 | 3.69 |
| `scss_scanner__group_8` | 610 µs | 28.76 | 18.00 | n/a | n/a | 106.97 | 11.51 | 1.66 | 6.11 |
| `c_scanner__document` | 1.13 ms | 17.00 | 38.54 | 22.68 | 43.70 | 321.70 | 27.76 | 1.97 | 10.49 |
| `c_scanner__group_12` | 242 µs | 18.16 | 60.35 | 13.50 | 71.70 | 333.43 | 26.85 | 3.73 | 11.25 |
| `php_scanner__document` | 1.32 ms | 13.78 | 33.08 | 7.70 | 14.53 | 65.89 | 14.41 | 1.21 | 4.31 |
| `php_scanner__group_13` | 173 µs | 21.01 | 84.29 | 11.46 | 29.01 | 137.57 | 25.23 | 2.60 | 8.31 |

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

