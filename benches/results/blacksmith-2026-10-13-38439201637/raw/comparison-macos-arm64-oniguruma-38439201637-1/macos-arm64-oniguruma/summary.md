### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 173 µs | 2.63 | 3.65 | 0.62 | 3.30 | 7.51 | 11.36 |
| `regex_tasks__html_attribute_values` | 513 µs | 1.69 | 2.10 | 0.37 | 1.40 | 3.68 | 1.48 |
| `regex_tasks__prices_lookbehind` | 208 µs | 1.66 | 2.44 | 0.26 | 1.13 | 7.89 | 1.69 |
| `regex_tasks__quoted_strings` | 237 µs | 1.35 | 1.66 | 0.24 | 1.52 | 5.05 | 2.80 |
| `regex_tasks__camel_case_words` | 683 µs | 2.10 | 1.39 | 0.42 | 0.87 | 2.51 | 1.79 |
| `regex_tasks__markdown_emphasis` | 158 µs | 1.20 | 1.99 | 0.29 | 1.02 | 11.77 | 1.59 |
| `regex_tasks__emoji_graphemes` | 171 µs | 2.37 | 3.64 | 0.60 | 4.66 | n/a | n/a |
| `regex_tasks__password_rules` | 17.5 µs | 1.24 | 1.86 | 0.24 | 1.54 | 0.63 | 0.68 |
| `regex_tasks__json_objects_recursive` | 328 µs | 1.38 | 2.49 | 0.26 | 2.62 | 2.30 | n/a |
| `regex_tasks__html_nested_divs_recursive` | 62.3 µs | 2.04 | 3.10 | 0.36 | 2.83 | 15.54 | 1619.16 |
| `regex_tasks__variable_lookbehind` | 984 µs | 1.35 | n/a | 0.21 | 0.78 | 1.82 | 0.48 |
| `oniguruma_features__atomic_possessive_strings` | 21.9 µs | 1.69 | 1.19 | 0.36 | 1.23 | 3.26 | 2.83 |
| `oniguruma_features__subexp_call_balanced` | 32.8 µs | 1.38 | 1.65 | 0.32 | 1.79 | n/a | n/a |
| `oniguruma_features__absent_comments` | 26 µs | 1.06 | 0.64 | n/a | n/a | 1.50 | 13.76 |
| `oniguruma_features__conditional_brackets` | 35.1 µs | 1.37 | 1.77 | 0.36 | 1.41 | 4.06 | n/a |
| `oniguruma_features__backref_ignorecase` | 70.1 µs | 1.06 | 1.20 | 0.30 | 1.06 | 2.09 | 1.73 |
| `oniguruma_features__lookbehind_alternation` | 14.6 µs | 3.73 | 1.48 | 0.81 | 6.17 | 5.61 | 1.82 |

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

