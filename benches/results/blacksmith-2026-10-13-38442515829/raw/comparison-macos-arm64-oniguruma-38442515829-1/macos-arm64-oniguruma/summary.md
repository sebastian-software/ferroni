### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `d791bf1b011f`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 178 µs | 2.61 | 3.73 | 0.62 | 3.33 | 7.69 | 11.18 |
| `regex_tasks__html_attribute_values` | 646 µs | 1.57 | 1.83 | 0.29 | 1.21 | 2.99 | 1.27 |
| `regex_tasks__prices_lookbehind` | 217 µs | 1.69 | 2.60 | 0.26 | 1.14 | 7.74 | 1.76 |
| `regex_tasks__quoted_strings` | 248 µs | 1.30 | 1.60 | 0.24 | 1.44 | 5.01 | 2.84 |
| `regex_tasks__camel_case_words` | 719 µs | 2.11 | 1.34 | 0.39 | 0.80 | 2.52 | 1.77 |
| `regex_tasks__markdown_emphasis` | 176 µs | 1.27 | 1.91 | 0.32 | 1.00 | 11.14 | 1.49 |
| `regex_tasks__emoji_graphemes` | 184 µs | 2.16 | 3.04 | 0.50 | 3.78 | n/a | n/a |
| `regex_tasks__password_rules` | 15.1 µs | 1.32 | 2.05 | 0.25 | 1.67 | 0.65 | 0.68 |
| `regex_tasks__json_objects_recursive` | 329 µs | 1.36 | 2.51 | 0.25 | 2.56 | 2.28 | n/a |
| `regex_tasks__html_nested_divs_recursive` | 63 µs | 2.15 | 3.30 | 0.38 | 2.74 | 16.06 | 1703.86 |
| `regex_tasks__variable_lookbehind` | 1.03 ms | 1.43 | n/a | 0.22 | 0.84 | 1.93 | 0.51 |
| `oniguruma_features__atomic_possessive_strings` | 23.2 µs | 1.69 | 1.02 | 0.31 | 1.10 | 2.88 | 2.46 |
| `oniguruma_features__subexp_call_balanced` | 29.2 µs | 1.33 | 1.61 | 0.30 | 1.71 | n/a | n/a |
| `oniguruma_features__absent_comments` | 23.6 µs | 1.11 | 0.70 | n/a | n/a | 1.66 | 15.08 |
| `oniguruma_features__conditional_brackets` | 35.6 µs | 1.40 | 1.71 | 0.36 | 1.23 | 3.58 | n/a |
| `oniguruma_features__backref_ignorecase` | 68.7 µs | 1.08 | 1.13 | 0.28 | 0.97 | 1.85 | 1.61 |
| `oniguruma_features__lookbehind_alternation` | 14.3 µs | 4.12 | 1.45 | 0.88 | 6.59 | 5.58 | 1.80 |

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

