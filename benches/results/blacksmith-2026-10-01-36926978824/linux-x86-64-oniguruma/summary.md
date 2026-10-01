### Engine comparison: linux-x86-64 (AMD EPYC)

Source `6db6413e84b0`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `regex_tasks__html_element_pairs` | 609 µs | 1.02 | 1.29 | 0.19 | 1.14 | 2.43 | – | – |
| `regex_tasks__html_attribute_values` | 1.08 ms | 1.15 | 1.06 | 0.17 | 0.83 | 2.09 | – | – |
| `regex_tasks__prices_lookbehind` | 314 µs | 1.30 | 2.09 | 0.25 | 1.05 | 8.92 | – | – |
| `regex_tasks__quoted_strings` | 325 µs | 1.39 | 1.15 | 0.25 | 1.53 | 3.94 | – | – |
| `regex_tasks__camel_case_words` | 843 µs | 1.83 | 1.35 | 0.47 | 0.83 | 2.19 | – | – |
| `regex_tasks__markdown_emphasis` | 210 µs | 1.03 | 1.41 | 0.24 | 0.93 | 9.12 | – | – |
| `regex_tasks__emoji_graphemes` | 185 µs | 1.84 | 2.93 | 0.54 | 3.75 | n/a | – | – |
| `regex_tasks__password_rules` | 23.6 µs | 0.85 | 1.39 | 0.18 | 1.01 | 0.47 | – | – |
| `regex_tasks__json_objects_recursive` | 383 µs | 1.29 | 2.00 | 0.15 | 2.45 | 2.17 | – | – |
| `regex_tasks__html_nested_divs_recursive` | 84.1 µs | 1.82 | 2.29 | 0.31 | 1.97 | 14.93 | – | – |
| `regex_tasks__variable_lookbehind` | 1.45 ms | 1.18 | n/a | 0.14 | 0.60 | 1.45 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 26.1 µs | 1.42 | 1.23 | 0.35 | 1.08 | 2.86 | – | – |
| `oniguruma_features__subexp_call_balanced` | 36.3 µs | 1.09 | 1.24 | 0.33 | 1.82 | n/a | – | – |
| `oniguruma_features__absent_comments` | 32.3 µs | 0.77 | 0.63 | n/a | n/a | 1.31 | – | – |
| `oniguruma_features__conditional_brackets` | 53 µs | 1.12 | 1.19 | 0.29 | 0.95 | 2.50 | – | – |
| `oniguruma_features__backref_ignorecase` | 76.8 µs | 1.01 | 1.06 | 0.32 | 1.16 | 1.89 | – | – |
| `oniguruma_features__lookbehind_alternation` | 20.5 µs | 2.98 | 1.07 | 0.58 | 3.49 | 3.57 | – | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `regex_tasks/fancy_regex/emoji_graphemes` | compile: Parsing error at position 52: Invalid escape: \X |
| `regex_tasks/onigmo/variable_lookbehind` | compile: Onigmo error -122 |

