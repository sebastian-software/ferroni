### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `6db6413e84b0`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 456 µs | 1.13 | 1.54 | 0.27 | 1.36 | 3.27 | – | – |
| `regex_tasks__html_attribute_values` | 944 µs | 1.06 | 1.19 | 0.21 | 0.80 | 2.11 | – | – |
| `regex_tasks__prices_lookbehind` | 223 µs | 1.66 | 2.65 | 0.26 | 1.12 | 8.06 | – | – |
| `regex_tasks__quoted_strings` | 295 µs | 1.22 | 1.36 | 0.23 | 1.36 | 4.42 | – | – |
| `regex_tasks__camel_case_words` | 743 µs | 2.01 | 1.48 | 0.38 | 0.79 | 2.43 | – | – |
| `regex_tasks__markdown_emphasis` | 191 µs | 1.11 | 1.74 | 0.26 | 0.90 | 10.83 | – | – |
| `regex_tasks__emoji_graphemes` | 170 µs | 2.34 | 3.48 | 0.56 | 4.28 | n/a | – | – |
| `regex_tasks__password_rules` | 18.1 µs | 1.16 | 1.54 | 0.21 | 1.29 | 0.52 | – | – |
| `regex_tasks__json_objects_recursive` | 283 µs | 1.48 | 2.95 | 0.29 | 3.15 | 2.66 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 64.6 µs | 2.30 | 2.92 | 0.40 | 2.78 | 15.63 | – | – |
| `regex_tasks__variable_lookbehind` | 1.12 ms | 1.14 | n/a | 0.17 | 0.68 | 1.55 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 24 µs | 1.51 | 1.11 | 0.33 | 1.16 | 3.03 | – | – |
| `oniguruma_features__subexp_call_balanced` | 32.8 µs | 1.38 | 1.59 | 0.32 | 1.71 | n/a | – | – |
| `oniguruma_features__absent_comments` | 27.8 µs | 1.17 | 0.66 | n/a | n/a | 1.51 | – | – |
| `oniguruma_features__conditional_brackets` | 48.1 µs | 1.21 | 1.25 | 0.30 | 1.03 | 2.91 | – | – |
| `oniguruma_features__backref_ignorecase` | 75.1 µs | 0.98 | 1.09 | 0.27 | 0.96 | 1.83 | – | – |
| `oniguruma_features__lookbehind_alternation` | 18 µs | 3.13 | 1.19 | 0.61 | 4.97 | 4.33 | – | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |

