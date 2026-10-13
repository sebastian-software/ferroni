### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 244 µs | 2.52 | 3.00 | 0.53 | 3.71 | 8.21 | 8.66 |
| `regex_tasks__html_attribute_values` | 778 µs | 1.80 | 1.50 | 0.27 | 1.12 | 2.93 | 0.98 |
| `regex_tasks__prices_lookbehind` | 257 µs | 1.55 | 1.96 | 0.24 | 0.90 | 7.37 | 2.08 |
| `regex_tasks__quoted_strings` | 337 µs | 1.27 | 1.34 | 0.26 | 1.51 | 4.17 | 2.79 |
| `regex_tasks__camel_case_words` | 1.19 ms | 1.67 | 1.31 | 0.42 | 0.73 | 2.01 | 1.47 |
| `regex_tasks__markdown_emphasis` | 308 µs | 0.83 | 1.11 | 0.19 | 0.82 | 8.02 | 1.03 |
| `regex_tasks__emoji_graphemes` | 226 µs | 1.86 | 3.65 | 0.59 | 3.57 | n/a | n/a |
| `regex_tasks__password_rules` | 26.3 µs | 0.98 | 1.53 | 0.16 | 1.02 | 0.44 | 0.52 |
| `regex_tasks__json_objects_recursive` | 347 µs | 1.24 | 2.35 | 0.19 | 2.38 | 2.59 | n/a |
| `regex_tasks__html_nested_divs_recursive` | 71.1 µs | 1.94 | 3.19 | 0.29 | 2.17 | 14.13 | 1767.37 |
| `regex_tasks__variable_lookbehind` | 1.24 ms | 1.51 | n/a | 0.21 | 0.71 | 1.99 | 0.62 |
| `oniguruma_features__atomic_possessive_strings` | 33.8 µs | 1.27 | 1.21 | 0.32 | 0.93 | 2.73 | 2.41 |
| `oniguruma_features__subexp_call_balanced` | 50.8 µs | 1.05 | 1.00 | 0.28 | 1.60 | n/a | n/a |
| `oniguruma_features__absent_comments` | 34.8 µs | 0.74 | 0.63 | n/a | n/a | 1.87 | 16.46 |
| `oniguruma_features__conditional_brackets` | 49.1 µs | 1.55 | 1.36 | 0.37 | 1.20 | 3.15 | n/a |
| `oniguruma_features__backref_ignorecase` | 82.5 µs | 1.17 | 1.35 | 0.34 | 1.39 | 2.51 | 2.33 |
| `oniguruma_features__lookbehind_alternation` | 19.5 µs | 3.86 | 1.35 | 0.78 | 4.95 | 5.34 | 1.81 |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/fancy_regex_seek/conditional_brackets` | match 1: [(68, 87), (68, 69)], Oniguruma [(26, 41), (-1, -1)] |
| `oniguruma_features/fancy_regex_seek/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex_seek/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/fancy_regex_seek/json_objects_recursive` | match 0: [(136, 152)], Oniguruma [(4, 208)] |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |

