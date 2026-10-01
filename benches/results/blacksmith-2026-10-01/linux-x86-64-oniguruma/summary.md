### Engine comparison: linux-x86-64 (AMD EPYC)

Source `11a7ba0248c9`, Ubuntu 24.04.3 LTS, 4 CPUs, avx2 avx512bw avx512f bmi2 sse4_2

Ferroni is the Criterion mean. Every other column is that engine's time divided by Ferroni's: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case (reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.

#### oniguruma

| case | Ferroni | C | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | regex | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `single_pattern__lookaround_combined` | 54.9 ns | 3.65 | 1.01 | 0.48 | 4.52 | 3.61 | – | – |
| `single_pattern__backref_simple` | 50.3 ns | 2.17 | 1.12 | 0.41 | 1.24 | 2.08 | – | – |
| `oniguruma_features__atomic_possessive_strings` | 26.5 µs | 1.27 | 1.06 | 0.31 | 0.84 | 2.48 | – | – |
| `oniguruma_features__subexp_call_balanced` | 34.1 µs | 1.09 | 1.27 | 0.31 | 1.79 | n/a | – | – |
| `oniguruma_features__absent_comments` | 27.1 µs | 0.85 | 0.67 | n/a | n/a | 1.50 | – | – |
| `oniguruma_features__conditional_brackets` | 49.4 µs | 1.07 | 1.13 | 0.28 | 0.91 | 2.41 | – | – |
| `oniguruma_features__backref_ignorecase` | 68.6 µs | 1.10 | 1.04 | 0.32 | 1.16 | 1.92 | – | – |
| `oniguruma_features__lookbehind_alternation` | 17.9 µs | 3.00 | 1.12 | 0.62 | 3.50 | 3.88 | – | – |
| `compilation__lookbehind` | 1.2 µs | 0.42 | 0.28 | 2.84 | 0.18 | 128.48 | – | – |
| `text_scanning__regset_position_lead` | 59.2 ns | 3.42 | – | – | – | – | – | – |

#### Cases an engine cannot run

| benchmark | reason |
| --- | --- |
| `oniguruma_features/fancy_regex/subexp_call_balanced` | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `oniguruma_features/pcre2/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `oniguruma_features/pcre2_jit/absent_comments` | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |

