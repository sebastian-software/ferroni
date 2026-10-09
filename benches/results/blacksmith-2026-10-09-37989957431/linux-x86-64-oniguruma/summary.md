### Engine comparison: linux-x86-64 (AMD EPYC)

Source `a38768bc8012`, Ubuntu 24.04.4 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 525 µs | 1.15 | 1.26 | 0.19 | 1.28 | 2.55 | 3.36 |
| `regex_tasks__html_attribute_values` | 950 µs | 0.92 | 1.09 | 0.15 | 0.81 | 2.09 | 0.74 |
| `regex_tasks__prices_lookbehind` | 240 µs | 1.48 | 2.09 | 0.23 | 0.87 | 7.81 | 1.93 |
| `regex_tasks__quoted_strings` | 346 µs | 1.12 | 1.10 | 0.21 | 1.25 | 3.79 | 2.15 |
| `regex_tasks__camel_case_words` | 773 µs | 1.78 | 1.50 | 0.52 | 0.94 | 2.42 | 1.75 |
| `regex_tasks__markdown_emphasis` | 206 µs | 0.90 | 1.45 | 0.22 | 0.95 | 9.33 | 1.30 |
| `regex_tasks__emoji_graphemes` | 183 µs | 2.29 | 3.35 | 0.61 | 3.97 | n/a | n/a |
| `regex_tasks__password_rules` | 20.6 µs | 0.91 | 1.65 | 0.17 | 1.18 | 0.57 | 0.54 |
| `regex_tasks__json_objects_recursive` | 372 µs | 1.05 | 2.20 | 0.15 | 2.20 | 2.16 | n/a |
| `regex_tasks__html_nested_divs_recursive` | 68.8 µs | 1.95 | 2.77 | 0.29 | 2.15 | 14.19 | 1676.65 |
| `regex_tasks__variable_lookbehind` | 1.25 ms | 1.12 | n/a | 0.14 | 0.60 | 1.64 | 0.48 |
| `oniguruma_features__atomic_possessive_strings` | 25.5 µs | 1.24 | 1.16 | 0.30 | 0.91 | 2.85 | 2.46 |
| `oniguruma_features__subexp_call_balanced` | 35.1 µs | 1.01 | 1.40 | 0.33 | 1.92 | n/a | n/a |
| `oniguruma_features__absent_comments` | 29.2 µs | 0.78 | 0.68 | n/a | n/a | 1.55 | 17.22 |
| `oniguruma_features__conditional_brackets` | 44.6 µs | 1.43 | 1.56 | 0.38 | 1.38 | 3.10 | n/a |
| `oniguruma_features__backref_ignorecase` | 84.4 µs | 1.07 | 1.03 | 0.31 | 1.16 | 1.87 | 1.59 |
| `oniguruma_features__lookbehind_alternation` | 17.8 µs | 3.45 | 1.27 | 0.66 | 4.06 | 4.90 | 1.66 |

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

