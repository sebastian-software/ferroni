### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `62892008bbf5`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 490 µs | 0.95 | 1.44 | 0.25 | 1.33 | 2.96 | – | – |
| `regex_tasks__html_attribute_values` | 987 µs | 0.96 | 1.18 | 0.20 | 0.75 | 1.99 | – | – |
| `regex_tasks__prices_lookbehind` | 225 µs | 1.64 | 2.71 | 0.24 | 1.09 | 7.84 | – | – |
| `regex_tasks__quoted_strings` | 305 µs | 1.16 | 1.44 | 0.21 | 1.27 | 4.52 | – | – |
| `regex_tasks__camel_case_words` | 736 µs | 2.30 | 1.48 | 0.44 | 0.89 | 2.70 | – | – |
| `regex_tasks__markdown_emphasis` | 193 µs | 1.21 | 1.76 | 0.29 | 0.96 | 10.79 | – | – |
| `regex_tasks__emoji_graphemes` | 182 µs | 2.31 | 3.42 | 0.55 | 4.18 | n/a | – | – |
| `regex_tasks__password_rules` | 19.6 µs | 1.09 | 1.66 | 0.20 | 1.27 | 0.54 | – | – |
| `regex_tasks__json_objects_recursive` | 321 µs | 1.41 | 2.64 | 0.26 | 2.76 | 2.49 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 64.7 µs | 2.18 | 3.14 | 0.38 | 2.74 | 16.14 | – | – |
| `regex_tasks__variable_lookbehind` | 1.26 ms | 1.10 | n/a | 0.17 | 0.67 | 1.47 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 23.7 µs | 1.55 | 1.13 | 0.32 | 1.13 | 3.05 | – | – |
| `oniguruma_features__subexp_call_balanced` | 33.1 µs | 1.33 | 1.61 | 0.31 | 1.76 | n/a | – | – |
| `oniguruma_features__absent_comments` | 27.4 µs | 1.22 | 0.71 | n/a | n/a | 1.59 | – | – |
| `oniguruma_features__conditional_brackets` | 41 µs | 1.36 | 1.50 | 0.35 | 1.20 | 3.42 | – | – |
| `oniguruma_features__backref_ignorecase` | 77.8 µs | 0.98 | 1.05 | 0.27 | 0.98 | 1.79 | – | – |
| `oniguruma_features__lookbehind_alternation` | 15.6 µs | 3.62 | 1.29 | 0.73 | 5.94 | 4.92 | – | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |

