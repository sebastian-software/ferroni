### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `a38768bc8012`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 439 µs | 1.05 | 1.59 | 0.28 | 1.45 | 3.26 | 4.93 |
| `regex_tasks__html_attribute_values` | 914 µs | 1.03 | 1.31 | 0.22 | 0.89 | 2.22 | 0.92 |
| `regex_tasks__prices_lookbehind` | 244 µs | 1.60 | 2.40 | 0.26 | 1.12 | 7.78 | 1.76 |
| `regex_tasks__quoted_strings` | 290 µs | 1.26 | 1.52 | 0.24 | 1.46 | 4.83 | 2.67 |
| `regex_tasks__camel_case_words` | 807 µs | 2.12 | 1.31 | 0.40 | 0.83 | 2.52 | 1.66 |
| `regex_tasks__markdown_emphasis` | 199 µs | 1.17 | 1.81 | 0.27 | 0.90 | 10.57 | 1.42 |
| `regex_tasks__emoji_graphemes` | 181 µs | 2.32 | 3.50 | 0.53 | 4.14 | n/a | n/a |
| `regex_tasks__password_rules` | 20.3 µs | 1.16 | 1.54 | 0.21 | 1.35 | 0.53 | 0.51 |
| `regex_tasks__json_objects_recursive` | 313 µs | 1.47 | 2.82 | 0.28 | 2.85 | 2.49 | n/a |
| `regex_tasks__html_nested_divs_recursive` | 67.5 µs | 2.23 | 3.05 | 0.37 | 2.70 | 15.43 | 1570.26 |
| `regex_tasks__variable_lookbehind` | 1.23 ms | 1.12 | n/a | 0.17 | 0.66 | 1.52 | 0.41 |
| `oniguruma_features__atomic_possessive_strings` | 23 µs | 1.60 | 1.14 | 0.33 | 1.19 | 3.16 | 2.69 |
| `oniguruma_features__subexp_call_balanced` | 32.9 µs | 1.36 | 1.62 | 0.31 | 1.80 | n/a | n/a |
| `oniguruma_features__absent_comments` | 28.3 µs | 1.14 | 0.67 | n/a | n/a | 1.57 | 14.78 |
| `oniguruma_features__conditional_brackets` | 40.8 µs | 1.26 | 1.38 | 0.33 | 1.17 | 3.31 | n/a |
| `oniguruma_features__backref_ignorecase` | 72.3 µs | 0.97 | 1.08 | 0.27 | 0.95 | 1.81 | 1.57 |
| `oniguruma_features__lookbehind_alternation` | 14.4 µs | 3.58 | 1.37 | 0.71 | 5.83 | 5.14 | 1.61 |

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

