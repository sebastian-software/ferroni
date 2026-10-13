### Engine comparison: linux-x86-64 (AMD EPYC)

Source `d791bf1b011f`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 264 µs | 2.45 | 2.56 | 0.42 | 2.66 | 5.19 | 7.87 |
| `regex_tasks__html_attribute_values` | 690 µs | 1.60 | 1.57 | 0.28 | 1.33 | 3.41 | 1.13 |
| `regex_tasks__prices_lookbehind` | 268 µs | 1.35 | 1.78 | 0.23 | 0.94 | 8.36 | 1.73 |
| `regex_tasks__quoted_strings` | 303 µs | 1.09 | 1.25 | 0.22 | 1.30 | 4.73 | 2.36 |
| `regex_tasks__camel_case_words` | 888 µs | 1.59 | 1.15 | 0.46 | 0.81 | 2.11 | 1.42 |
| `regex_tasks__markdown_emphasis` | 183 µs | 0.94 | 1.56 | 0.23 | 0.98 | 10.03 | 1.52 |
| `regex_tasks__emoji_graphemes` | 169 µs | 1.99 | 3.77 | 0.56 | 4.01 | n/a | n/a |
| `regex_tasks__password_rules` | 22 µs | 1.05 | 1.49 | 0.19 | 1.04 | 0.59 | 0.58 |
| `regex_tasks__json_objects_recursive` | 322 µs | 1.13 | 2.08 | 0.16 | 2.13 | 2.29 | n/a |
| `regex_tasks__html_nested_divs_recursive` | 74.9 µs | 1.79 | 2.60 | 0.24 | 1.76 | 14.20 | 1623.91 |
| `regex_tasks__variable_lookbehind` | 1.1 ms | 1.46 | n/a | 0.19 | 0.76 | 2.31 | 0.57 |
| `oniguruma_features__atomic_possessive_strings` | 26.8 µs | 1.28 | 1.33 | 0.39 | 1.09 | 3.51 | 3.02 |
| `oniguruma_features__subexp_call_balanced` | 46.3 µs | 0.87 | 1.20 | 0.31 | 1.57 | n/a | n/a |
| `oniguruma_features__absent_comments` | 36.8 µs | 0.74 | 0.64 | n/a | n/a | 1.43 | 14.01 |
| `oniguruma_features__conditional_brackets` | 50.4 µs | 1.26 | 1.30 | 0.33 | 1.00 | 2.68 | n/a |
| `oniguruma_features__backref_ignorecase` | 78.9 µs | 1.00 | 1.26 | 0.31 | 1.11 | 1.89 | 1.83 |
| `oniguruma_features__lookbehind_alternation` | 17.8 µs | 3.35 | 1.68 | 0.83 | 5.06 | 4.87 | 1.68 |

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

