### Engine comparison: linux-x86-64 (AMD EPYC)

Source `62892008bbf5`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 493 µs | 1.21 | 1.18 | 0.17 | 1.08 | 2.45 | – | – |
| `regex_tasks__html_attribute_values` | 837 µs | 1.04 | 1.09 | 0.18 | 0.86 | 2.13 | – | – |
| `regex_tasks__prices_lookbehind` | 208 µs | 1.49 | 2.04 | 0.23 | 0.89 | 7.76 | – | – |
| `regex_tasks__quoted_strings` | 273 µs | 1.16 | 1.24 | 0.23 | 1.33 | 4.21 | – | – |
| `regex_tasks__camel_case_words` | 701 µs | 1.72 | 1.40 | 0.50 | 0.90 | 2.55 | – | – |
| `regex_tasks__markdown_emphasis` | 181 µs | 0.98 | 1.52 | 0.23 | 0.98 | 9.52 | – | – |
| `regex_tasks__emoji_graphemes` | 157 µs | 2.07 | 3.33 | 0.57 | 3.94 | n/a | – | – |
| `regex_tasks__password_rules` | 18.2 µs | 0.90 | 1.52 | 0.16 | 0.99 | 0.50 | – | – |
| `regex_tasks__json_objects_recursive` | 266 µs | 1.24 | 2.44 | 0.17 | 2.45 | 2.50 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 54.2 µs | 2.00 | 2.98 | 0.31 | 2.24 | 15.92 | – | – |
| `regex_tasks__variable_lookbehind` | 1.05 ms | 1.21 | n/a | 0.15 | 0.65 | 1.69 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 22.3 µs | 1.20 | 1.30 | 0.38 | 0.97 | 3.01 | – | – |
| `oniguruma_features__subexp_call_balanced` | 32.4 µs | 1.06 | 1.37 | 0.32 | 1.86 | n/a | – | – |
| `oniguruma_features__absent_comments` | 26.9 µs | 0.80 | 0.70 | n/a | n/a | 1.55 | – | – |
| `oniguruma_features__conditional_brackets` | 41 µs | 1.35 | 1.47 | 0.35 | 1.10 | 2.95 | – | – |
| `oniguruma_features__backref_ignorecase` | 68.8 µs | 1.11 | 1.12 | 0.36 | 1.20 | 2.03 | – | – |
| `oniguruma_features__lookbehind_alternation` | 15.5 µs | 3.38 | 1.34 | 0.67 | 4.29 | 4.95 | – | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |

