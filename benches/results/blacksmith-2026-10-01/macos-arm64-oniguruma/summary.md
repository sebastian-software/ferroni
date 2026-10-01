### Engine comparison: macos-arm64 (Apple M4 Pro (Virtual))

Source `11a7ba0248c9`, macOS 26.3, 6 CPUs

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__lookaround_combined` | 54.2 ns | 3.60 | 1.03 | 0.45 | 4.72 | 3.86 | – | – |
| `single_pattern__backref_simple` | 52.1 ns | 2.54 | 1.23 | 0.37 | 0.99 | 1.59 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 25 µs | 1.40 | 1.13 | 0.33 | 1.13 | 3.16 | – | – |
| `oniguruma_features__subexp_call_balanced` | 35.9 µs | 1.24 | 1.55 | 0.31 | 1.70 | n/a | – | – |
| `oniguruma_features__absent_comments` | 28.9 µs | 1.10 | 0.66 | n/a | n/a | 1.50 | – | – |
| `oniguruma_features__conditional_brackets` | 50.5 µs | 1.06 | 1.24 | 0.29 | 1.01 | 2.77 | – | – |
| `oniguruma_features__backref_ignorecase` | 86.2 µs | 0.94 | 1.04 | 0.25 | 0.92 | 1.62 | – | – |
| `oniguruma_features__lookbehind_alternation` | 18.2 µs | 3.07 | 1.16 | 0.66 | 5.37 | 4.52 | – | – |
| `compilation__lookbehind` | 1.27 µs | 0.36 | 0.29 | 1.69 | 0.20 | 115.44 | – | – |
| `text_scanning__regset_position_lead` | 75.2 ns | 4.17 | – | – | – | – | – | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |

