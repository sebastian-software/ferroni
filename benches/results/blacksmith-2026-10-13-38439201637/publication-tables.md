### Text processing, shared syntax

| Case | Ferroni | Oniguruma (C) | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `html_tags` | 302 µs<br />362 µs | 1.69<br />1.43 | 1.89<br />1.71 | 0.47<br />0.38 | 0.93<br />0.86 | 2.87<br />2.88 | 3.94<br />2.44 | 3.99<br />1.85 |
| `html_attributes` | 510 µs<br />568 µs | 3.21<br />2.88 | 4.20<br />3.78 | 0.39<br />0.31 | 2.14<br />2.22 | 0.57<br />0.58 | 0.60<br />0.58 | 0.47<br />0.47 |
| `html_comments` | 20.3 µs<br />23.3 µs | 3.71<br />2.82 | 3.36<br />3.00 | 0.70<br />0.68 | 6.76<br />5.06 | 6.94<br />5.65 | 7.01<br />5.57 | 7.34<br />5.41 |
| `hex_colors` | 16.5 µs<br />22.1 µs | 5.05<br />6.25 | 25.48<br />15.31 | 0.84<br />0.67 | 1.24<br />1.20 | 71.33<br />69.68 | 3.08<br />2.72 | 0.88<br />0.97 |
| `email_addresses` | 46.1 µs<br />45.7 µs | 30.92<br />32.62 | 57.29<br />57.77 | 2.69<br />1.87 | 27.20<br />29.28 | 0.55<br />0.81 | 0.55<br />0.78 | 0.40<br />0.55 |
| `emoji` | 114 µs<br />151 µs | 2.83<br />2.74 | 4.30<br />3.49 | 0.79<br />0.74 | 4.98<br />4.07 | 0.84<br />0.68 | 0.84<br />0.65 | 0.57<br />0.54 |
| `ascii_emoticons` | 67.2 µs<br />105 µs | 1.81<br />1.03 | 2.14<br />1.34 | 0.33<br />0.30 | 0.89<br />0.62 | 0.70<br />0.84 | 0.69<br />0.87 | 0.52<br />0.70 |
| `hashtags_mentions` | 75.5 µs<br />79.6 µs | 2.43<br />2.21 | 1.76<br />2.26 | 0.53<br />0.60 | 0.98<br />1.08 | 1.15<br />1.47 | 1.14<br />1.56 | 0.86<br />0.94 |
| `ipv4_addresses` | 154 µs<br />223 µs | 1.63<br />1.40 | 3.39<br />2.33 | 1.26<br />0.80 | 6.48<br />4.38 | 23.38<br />17.92 | 6.55<br />5.52 | 5.79<br />4.82 |
| `iso_timestamps` | 68.7 µs<br />85.6 µs | 2.06<br />1.85 | 4.71<br />4.39 | 0.38<br />0.38 | 6.72<br />8.73 | 24.47<br />29.38 | 2.31<br />2.47 | 0.85<br />0.77 |
| `log_keywords_ignorecase` | 79.7 µs<br />86.6 µs | 2.78<br />4.11 | 4.92<br />6.54 | 0.91<br />1.28 | 2.12<br />2.67 | 24.68<br />40.16 | 0.83<br />1.20 | 0.55<br />0.95 |
| `markdown_links` | 42.2 µs<br />70.7 µs | 3.04<br />2.48 | 5.78<br />4.13 | 0.84<br />0.56 | 1.59<br />1.27 | 6.06<br />4.13 | 6.04<br />3.93 | 5.70<br />3.49 |
| `semantic_versions` | 206 µs<br />294 µs | 1.06<br />1.14 | 1.78<br />1.67 | 0.31<br />0.21 | 0.81<br />0.69 | 5.53<br />5.17 | 0.64<br />0.59 | 0.36<br />0.29 |
| `json_strings` | 292 µs<br />403 µs | 1.99<br />1.88 | 1.50<br />1.27 | 0.51<br />0.54 | 2.26<br />2.06 | 1.14<br />1.43 | 1.14<br />1.36 | 0.74<br />0.91 |
| `csv_fields` | 181 µs<br />210 µs | 2.13<br />1.89 | 1.53<br />1.70 | 0.45<br />0.59 | 1.78<br />1.72 | 1.15<br />1.54 | 1.14<br />1.55 | 0.86<br />1.04 |
| `rfc5322_emails` | 3.12 ms<br />4.44 ms | 0.75<br />0.79 | 2.27<br />2.20 | 0.10<br />0.07 | 1.52<br />1.20 | 0.04<br />0.03 | 0.05<br />0.03 | 0.04<br />0.03 |
| `ipv6_addresses` | 1.79 ms<br />2.13 ms | 0.65<br />0.75 | 1.46<br />1.37 | 0.45<br />0.36 | 2.86<br />2.36 | 0.07<br />0.06 | 0.07<br />0.06 | 0.07<br />0.06 |
| `rfc3986_urls` | 80.3 µs<br />80.3 µs | 1.76<br />1.66 | 2.07<br />2.24 | 0.80<br />0.74 | 2.41<br />2.14 | 19.00<br />22.56 | 0.91<br />1.06 | 0.55<br />0.59 |
| `keyword_alternation_200` | 802 µs<br />924 µs | 4.33<br />4.46 | 5.17<br />5.03 | 0.61<br />0.66 | 11.46<br />9.63 | 24.24<br />19.59 | 4.19<br />4.81 | 0.48<br />0.57 |
| `csv_last_column_backtracking` | 139 µs<br />170 µs | 1.13<br />0.93 | 1.35<br />1.44 | 0.27<br />0.25 | 0.61<br />0.51 | 0.85<br />0.68 | 1.50<br />1.19 | 1.18<br />0.98 |
| `unicode_case_folding` | 134 µs<br />201 µs | 3.44<br />3.05 | 1.11<br />0.90 | n/a | n/a | n/a | n/a | n/a |
| `email_validation` | 4.84 µs<br />7.73 µs | 2.02<br />1.24 | 1.96<br />1.51 | 0.33<br />0.28 | 1.14<br />0.77 | 0.29<br />0.25 | 0.28<br />0.27 | 0.22<br />0.25 |
| `uuid_validation` | 3.63 µs<br />5.22 µs | 3.13<br />2.13 | 3.66<br />2.96 | 0.40<br />0.40 | 0.86<br />0.77 | 0.42<br />0.41 | 0.42<br />0.41 | 0.32<br />0.30 |
| `number_validation` | 4.14 µs<br />5.23 µs | 2.26<br />1.36 | 1.81<br />1.56 | 0.39<br />0.36 | 2.69<br />1.76 | 0.27<br />0.26 | 0.24<br />0.26 | 0.16<br />0.17 |
| `access_log_captures` | 17.5 µs<br />25.7 µs | 1.36<br />1.05 | 1.78<br />1.51 | 0.32<br />0.20 | 0.83<br />0.52 | 1.73<br />1.29 | 2.01<br />1.50 | 1.57<br />1.18 |
| `url_extraction` | 5.78 µs<br />6.35 µs | 2.72<br />2.05 | 2.65<br />2.36 | 0.56<br />0.56 | 1.34<br />1.29 | 1.98<br />2.05 | 1.95<br />1.98 | 1.58<br />1.51 |
| `unicode_words` | 31.7 µs<br />38.1 µs | 1.97<br />1.64 | 1.31<br />1.33 | 0.42<br />0.55 | 1.15<br />1.13 | 1.05<br />1.20 | 1.05<br />1.34 | 0.70<br />0.87 |
| `email_redaction` | 16.5 µs<br />19.2 µs | 4.17<br />4.26 | 6.52<br />7.40 | 0.61<br />0.65 | 3.81<br />4.31 | 0.83<br />1.13 | 0.85<br />1.14 | 0.63<br />0.79 |
| `literal_50k` | 42.3 ns<br />44 ns | 2.92<br />2.20 | 0.73<br />0.58 | 0.37<br />0.43 | 0.94<br />0.97 | 0.27<br />0.31 | 0.28<br />0.31 | 0.18<br />0.28 |
| `no_match_50k` | 1.12 µs<br />780 ns | 5.68<br />8.59 | 5.66<br />8.87 | 1.15<br />1.10 | 0.79<br />0.35 | 0.99<br />1.01 | 0.98<br />1.21 | 0.96<br />0.82 |
| `field_extract_50k` | 42.8 ns<br />55.9 ns | 2.96<br />1.81 | 1.11<br />0.81 | 0.53<br />0.45 | 1.37<br />1.28 | 0.91<br />1.03 | 0.92<br />0.90 | 0.89<br />0.92 |
| `timestamp_50k` | 65.9 ns<br />62.9 ns | 2.00<br />1.47 | 1.70<br />2.36 | 0.44<br />0.50 | 0.96<br />1.16 | 0.70<br />0.98 | 0.67<br />0.96 | 0.60<br />0.82 |

### Text processing, Oniguruma syntax

| Case | Ferroni | Oniguruma (C) | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `html_element_pairs` | 178 µs<br />244 µs | 2.61<br />2.52 | 3.73<br />3.00 | 0.62<br />0.53 | 3.33<br />3.71 | 7.69<br />8.21 | 11.18<br />8.66 |
| `html_attribute_values` | 646 µs<br />778 µs | 1.57<br />1.80 | 1.83<br />1.50 | 0.29<br />0.27 | 1.21<br />1.12 | 2.99<br />2.93 | 1.27<br />0.98 |
| `prices_lookbehind` | 217 µs<br />257 µs | 1.69<br />1.55 | 2.60<br />1.96 | 0.26<br />0.24 | 1.14<br />0.90 | 7.74<br />7.37 | 1.76<br />2.08 |
| `quoted_strings` | 248 µs<br />337 µs | 1.30<br />1.27 | 1.60<br />1.34 | 0.24<br />0.26 | 1.44<br />1.51 | 5.01<br />4.17 | 2.84<br />2.79 |
| `camel_case_words` | 719 µs<br />1.19 ms | 2.11<br />1.67 | 1.34<br />1.31 | 0.39<br />0.42 | 0.80<br />0.73 | 2.52<br />2.01 | 1.77<br />1.47 |
| `markdown_emphasis` | 176 µs<br />308 µs | 1.27<br />0.83 | 1.91<br />1.11 | 0.32<br />0.19 | 1.00<br />0.82 | 11.14<br />8.02 | 1.49<br />1.03 |
| `emoji_graphemes` | 184 µs<br />226 µs | 2.16<br />1.86 | 3.04<br />3.65 | 0.50<br />0.59 | 3.78<br />3.57 | n/a | n/a |
| `password_rules` | 15.1 µs<br />26.3 µs | 1.32<br />0.98 | 2.05<br />1.53 | 0.25<br />0.16 | 1.67<br />1.02 | 0.65<br />0.44 | 0.68<br />0.52 |
| `json_objects_recursive` | 329 µs<br />347 µs | 1.36<br />1.24 | 2.51<br />2.35 | 0.25<br />0.19 | 2.56<br />2.38 | 2.28<br />2.59 | n/a |
| `html_nested_divs_recursive` | 63 µs<br />71.1 µs | 2.15<br />1.94 | 3.30<br />3.19 | 0.38<br />0.29 | 2.74<br />2.17 | 16.06<br />14.13 | 1703.86<br />1767.37 |
| `variable_lookbehind` | 1.03 ms<br />1.24 ms | 1.43<br />1.51 | n/a | 0.22<br />0.21 | 0.84<br />0.71 | 1.93<br />1.99 | 0.51<br />0.62 |
| `atomic_possessive_strings` | 23.2 µs<br />33.8 µs | 1.69<br />1.27 | 1.02<br />1.21 | 0.31<br />0.32 | 1.10<br />0.93 | 2.88<br />2.73 | 2.46<br />2.41 |
| `subexp_call_balanced` | 29.2 µs<br />50.8 µs | 1.33<br />1.05 | 1.61<br />1.00 | 0.30<br />0.28 | 1.71<br />1.60 | n/a | n/a |
| `absent_comments` | 23.6 µs<br />34.8 µs | 1.11<br />0.74 | 0.70<br />0.63 | n/a | n/a | 1.66<br />1.87 | 15.08<br />16.46 |
| `conditional_brackets` | 35.6 µs<br />49.1 µs | 1.40<br />1.55 | 1.71<br />1.36 | 0.36<br />0.37 | 1.23<br />1.20 | 3.58<br />3.15 | n/a |
| `backref_ignorecase` | 68.7 µs<br />82.5 µs | 1.08<br />1.17 | 1.13<br />1.35 | 0.28<br />0.34 | 0.97<br />1.39 | 1.85<br />2.51 | 1.61<br />2.33 |
| `lookbehind_alternation` | 14.3 µs<br />19.5 µs | 4.12<br />3.86 | 1.45<br />1.35 | 0.88<br />0.78 | 6.59<br />4.95 | 5.58<br />5.34 | 1.80<br />1.81 |

### Highlighting, portable grammars

| Case | Ferroni | Oniguruma (C) | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Java replay, document | 916 µs<br />1.1 ms | 8.91<br />9.30 | 33.53<br />29.19 | 10.31<br />11.02 | 36.48<br />37.13 | 84.10<br />93.31 | 10.85<br />10.41 | 1.46<br />1.26 | 7.57<br />9.13 |
| Java replay, scanner 15 | 114 µs<br />142 µs | 13.57<br />13.19 | 49.46<br />41.96 | 16.18<br />13.13 | 51.23<br />48.12 | 133.14<br />117.09 | 16.61<br />13.55 | 1.30<br />1.08 | 11.89<br />15.40 |
| C replay, document | 974 µs<br />1.09 ms | 15.98<br />16.69 | 47.44<br />43.10 | 13.55<br />17.30 | 57.90<br />46.38 | 270.68<br />333.44 | 21.09<br />25.68 | 2.29<br />2.45 | 10.12<br />10.88 |
| C replay, scanner 12 | 220 µs<br />253 µs | 20.02<br />16.55 | 68.49<br />57.05 | 15.18<br />13.40 | 88.32<br />60.77 | 375.35<br />284.23 | 27.53<br />22.08 | 3.37<br />2.84 | 12.13<br />12.00 |
| PHP replay, document | 1.64 ms<br />1.62 ms | 10.47<br />12.00 | 30.22<br />31.41 | 5.40<br />6.36 | 13.01<br />11.42 | 57.16<br />61.79 | 13.07<br />12.66 | 1.13<br />1.10 | 3.16<br />3.70 |
| PHP replay, scanner 13 | 195 µs<br />193 µs | 20.13<br />20.33 | 74.67<br />78.02 | 10.06<br />12.78 | 29.37<br />32.55 | 132.07<br />168.91 | 25.61<br />22.96 | 2.17<br />2.38 | 7.11<br />7.82 |

### Highlighting, grammars with Oniguruma-only syntax

| Case | Ferroni | Oniguruma (C) | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| C++ replay, document | 2.72 ms<br />4.55 ms | 10.33<br />8.67 | n/a | n/a | n/a | 363.30<br />252.93 | 32.56<br />34.62 | 3.56<br />2.61 | 12.15<br />8.82 |
| C++ replay, scanner 78 | 1.33 ms<br />1.22 ms | 13.43<br />13.98 | n/a | n/a | n/a | 487.77<br />556.65 | 28.38<br />31.44 | 3.19<br />3.62 | 12.95<br />18.12 |
| SCSS replay, document | 1.17 ms<br />1.29 ms | 30.68<br />30.15 | 18.49<br />16.60 | n/a | n/a | 111.01<br />135.02 | 10.40<br />11.22 | 1.99<br />1.87 | 2.73<br />3.41 |
| SCSS replay, scanner 8 | 517 µs<br />537 µs | 31.99<br />30.97 | 18.41<br />17.74 | n/a | n/a | 107.76<br />108.96 | 11.67<br />11.41 | 1.46<br />1.56 | 4.11<br />4.95 |

### Highlighting, Ferriki tokenizer replays

| Case | Ferroni | Oniguruma (C) | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | fancy-regex (RegexSet) | Shiki JS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| JSON replay, document | 243 µs<br />237 µs | 6.15<br />4.26 | 6.22<br />5.80 | 2.00<br />1.86 | 3.28<br />3.25 | 9.23<br />10.87 | 5.09<br />6.27 | 2.92<br />3.51 | 3.38<br />3.58 |
| JSON replay, scanner 1 | 80.1 µs<br />70.5 µs | 7.45<br />4.99 | 6.56<br />6.67 | 2.46<br />2.35 | 3.85<br />3.71 | 10.11<br />11.34 | 4.83<br />6.03 | 2.24<br />2.89 | 3.37<br />3.51 |
| Astro replay, document | 1.65 ms<br />1.69 ms | 7.11<br />6.55 | n/a | n/a | n/a | n/a | n/a | n/a | 7.39<br />7.75 |
| Astro replay, scanner 22 | 131 µs<br />157 µs | 4.59<br />4.29 | 4.65<br />3.86 | 2.99<br />3.42 | 5.54<br />3.65 | n/a | n/a | n/a | 3.28<br />3.91 |

### Grammar scanners

| Case | Ferroni | Oniguruma (C) |
| --- | ---: | ---: |
| TypeScript grammar, compile | 16.7 ms<br />19.8 ms | 0.83<br />0.54 |
| CSS grammar, compile | 10.3 ms<br />13.5 ms | 1.50<br />1.13 |
| Rust grammar, compile | 390 µs<br />566 µs | 0.37<br />0.32 |
| TypeScript grammar, re-scan line | 2.91 µs<br />3.75 µs | 51.15<br />40.52 |
| CSS grammar, re-scan line | 167 µs<br />151 µs | 48.45<br />68.83 |
| Rust grammar, re-scan line | 1.62 µs<br />2.42 µs | 34.29<br />28.82 |
| TypeScript document, 28 lines | 94.7 µs<br />113 µs | 23.71<br />22.50 |
| CSS document, 19 lines | 76.4 µs<br />97.5 µs | 25.23<br />27.44 |
| Rust document, 31 lines | 38.7 µs<br />50.4 µs | 17.31<br />18.48 |

### Short searches and compilation

| Case | Ferroni | Oniguruma (C) | Onigmo | PCRE2 JIT | PCRE2 | fancy-regex | fancy-regex (seek) | regex |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `literal_exact` | 33.2 ns<br />44.8 ns | 3.11<br />2.17 | 1.20<br />0.80 | 0.45<br />0.48 | 1.19<br />1.22 | 0.31<br />0.38 | 0.33<br />0.41 | 0.22<br />0.30 |
| `quantifier_greedy` | 43 ns<br />46.5 ns | 3.47<br />2.43 | 1.56<br />1.47 | 0.51<br />0.52 | 0.97<br />1.05 | 1.14<br />1.34 | 1.13<br />1.33 | 1.07<br />1.18 |
| `alternation_2_branch` | 39.6 ns<br />49.4 ns | 2.98<br />1.47 | 1.22<br />0.92 | 0.39<br />0.45 | 1.33<br />1.44 | 0.84<br />1.21 | 0.79<br />1.15 | 0.79<br />0.87 |
| `alternation_10_branch` | 25.2 ns<br />45.6 ns | 6.18<br />3.68 | 14.66<br />3.14 | 0.76<br />0.66 | 7.86<br />4.04 | 0.69<br />0.61 | 0.67<br />0.57 | 0.49<br />0.46 |
| `case_insensitive_phrase` | 57.2 ns<br />92.1 ns | 2.37<br />1.60 | 0.88<br />0.85 | 0.32<br />0.25 | 0.93<br />0.61 | 0.74<br />0.76 | 0.70<br />0.86 | 0.65<br />0.73 |
| `named_capture_date` | 62.5 ns<br />106 ns | 3.38<br />2.37 | 1.76<br />1.41 | 0.72<br />0.53 | 4.59<br />3.79 | 0.52<br />0.51 | 0.51<br />0.53 | 0.46<br />0.49 |
| `unicode_greek` | 58.2 ns<br />109 ns | 3.15<br />2.14 | 3.49<br />1.92 | 0.55<br />0.34 | 2.39<br />1.96 | 0.72<br />0.74 | 0.71<br />0.73 | 0.66<br />0.65 |
| `lookaround_combined` | 60 ns<br />97.4 ns | 3.48<br />2.29 | 1.06<br />0.65 | 0.44<br />0.31 | 4.41<br />2.79 | 3.56<br />2.29 | 1.43<br />1.13 | – |
| `backref_simple` | 60.6 ns<br />70.1 ns | 2.07<br />1.59 | 1.06<br />0.94 | 0.30<br />0.34 | 0.87<br />0.87 | 1.36<br />1.38 | 1.83<br />1.91 | – |
| `regset_position_lead` | 86.6 ns<br />86 ns | 3.75<br />2.53 | – | – | – | – | – | – |
| compile `literal` | 636 ns<br />731 ns | 0.56<br />0.52 | 0.48<br />0.46 | 3.24<br />5.94 | 0.48<br />0.41 | 3.70<br />3.95 | 3.65<br />3.68 | 2.92<br />2.89 |
| compile `named_capture` | 2.62 µs<br />2.9 µs | 1.55<br />1.31 | 1.83<br />1.51 | 1.16<br />2.03 | 0.19<br />0.20 | 54.08<br />60.47 | 54.63<br />58.08 | 53.99<br />60.62 |
| compile `lookbehind` | 1.31 µs<br />1.47 µs | 0.29<br />0.46 | 0.25<br />0.34 | 1.44<br />3.00 | 0.17<br />0.20 | 99.21<br />131.83 | 198.65<br />270.03 | – |

### Cases an engine cannot run

| Case | Engine | Reason |
| --- | --- | --- |
| Astro replay, document | fancy-regex | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| Astro replay, document | fancy-regex (seek) | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| Astro replay, document | fancy-regex (RegexSet) | call 575 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| Astro replay, document | Onigmo | scanner 8, pattern "(?!(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)\|(?<!\\.))(?:\\b(export)\\s+)?(?:\\b(declare)\\s+)?\\b(const(?!\\s+enum\\b))(?![… |
| Astro replay, document | PCRE2 | scanner 60, pattern "\\\u{ffff}\\uFEFF?(?i:(?=\\s*@charset\\b))": PCRE2 at offset 6: PCRE2 does not support \F, \L, \l, \N{name}, \U, or \u |
| Astro replay, document | PCRE2 JIT | scanner 60, pattern "\\\u{ffff}\\uFEFF?(?i:(?=\\s*@charset\\b))": PCRE2 at offset 6: PCRE2 does not support \F, \L, \l, \N{name}, \U, or \u |
| Astro replay, scanner 22 | fancy-regex | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| Astro replay, scanner 22 | fancy-regex (seek) | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| Astro replay, scanner 22 | fancy-regex (RegexSet) | call 37 (scanner 22): Some((12, [(39, 39)])), Shiki None |
| C++ replay, document | Onigmo | scanner 0, pattern "^\\s+{1,0}(//[!/]+)": Onigmo error -202 |
| C++ replay, document | PCRE2 | scanner 0, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\A\|\\Z)(#)\\s+{0,1}pragma\\s… |
| C++ replay, document | PCRE2 JIT | scanner 0, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\A\|\\Z)(#)\\s+{0,1}pragma\\s… |
| C++ replay, scanner 78 | Onigmo | scanner 78, pattern "^\\s+{1,0}(//[!/]+)": Onigmo error -202 |
| C++ replay, scanner 78 | PCRE2 | scanner 78, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\\u{ffff}\|\\Z)(#)\\s+{0,1}p… |
| C++ replay, scanner 78 | PCRE2 JIT | scanner 78, pattern "^((((?:\\s*+/\\*(?:[^*]++\|\\*+(?!/))*+\\*/\\s*+)+)\|\\s++\|(?<=\\W)\|(?=\\W)\|^\|\\n?$\|\\\u{ffff}\|\\Z)(#)\\s+{0,1}p… |
| `subexp_call_balanced` | fancy-regex | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `conditional_brackets` | fancy-regex (seek) | match 1: [(68, 87), (68, 69)], Oniguruma [(26, 41), (-1, -1)] |
| `subexp_call_balanced` | fancy-regex (seek) | match 0: [(4, 21), (17, 21)], Oniguruma [(4, 21), (4, 21)] |
| `absent_comments` | PCRE2 | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `absent_comments` | PCRE2 JIT | compile: PCRE2 at offset 5: unrecognized character after (? or (?- |
| `emoji_graphemes` | fancy-regex | compile: Parsing error at position 52: Invalid escape: \X |
| `unicode_case_folding` | fancy-regex | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `emoji_graphemes` | fancy-regex (seek) | compile: Parsing error at position 52: Invalid escape: \X |
| `json_objects_recursive` | fancy-regex (seek) | match 0: [(136, 152)], Oniguruma [(4, 208)] |
| `unicode_case_folding` | fancy-regex (seek) | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `variable_lookbehind` | Onigmo | compile: Onigmo error -122 |
| `unicode_case_folding` | PCRE2 | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `unicode_case_folding` | PCRE2 JIT | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| `unicode_case_folding` | regex | match 0: [(374, 381)], Oniguruma [(211, 218)] |
| SCSS replay, document | PCRE2 | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| SCSS replay, document | PCRE2 JIT | call 12 (scanner 5): Some((12, [(10, 14), (0, 0), (0, 0)])), Shiki Some((11, [(9, 16), (9, 10)])) |
| SCSS replay, scanner 8 | PCRE2 | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
| SCSS replay, scanner 8 | PCRE2 JIT | call 51 (scanner 8): Some((36, [(5, 5)])), Shiki Some((21, [(4, 22), (4, 5), (5, 22)])) |
