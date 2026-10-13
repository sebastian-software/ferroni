# Current-main Blacksmith comparison — October 13, 2026

I ran every case set on both platforms after merging CSS PR #357. The full run is [38439201637](https://github.com/sebastian-software/ferroni/actions/runs/38439201637). I then repeated the complete suite on both platforms in [38442515829](https://github.com/sebastian-software/ferroni/actions/runs/38442515829), at the same commit, because the first x86 run showed large shifts in unchanged controls.

For publication, the later full run replaces the entire first run on both hosts. No individual cases are selected across the two runs; both original and repeated estimates remain available. Every published case comes from the second full run.

## Published position against other engines

| Ferroni compared with | Everyday patterns | Advanced patterns | Syntax highlighting |
| --- | ---: | ---: | ---: |
| Oniguruma (C) | 2.3× faster | 1.5× faster | 12× faster |
| Shiki JS | – | – | 6.7× faster |
| Onigmo | 2.7× faster | 1.7× faster\* | 35× faster |
| PCRE2 | 1.8× faster\* | 1.6× faster\* | 29× faster |
| PCRE2 JIT | 2.0× slower\* | 3.1× slower\* | 9.9× faster |
| fancy-regex | 1.0× slower\* | 3.4× faster\* | 1.5× faster |
| regex | 1.5× slower\* | – | – |

- **Everyday patterns** (32 tasks): search and extraction in HTML, logs, chat with emoji, Markdown, JSON, CSV and source code, in syntax the regex crate also runs.
- **Advanced patterns** (17 tasks): look-around, backreferences, possessive groups, subexpression calls, absent expressions and grapheme clusters.
- **Syntax highlighting** (3 documents): the scanner calls Shiki makes for whole C, Java and PHP documents, in grammars every engine runs.

\* Ran only some of the tasks: the engine rejects a pattern or finds different matches. The figure covers the tasks it ran.

– Not measured: the engine lacks the syntax, or the multi-pattern API the workload needs.

fancy-regex is quoted in its fastest configuration per column: seek mode for everyday patterns and advanced patterns; RegexSet for syntax highlighting.

## Matched changes since October 9

| Category | ARM cases | ARM time saved | x86 cases | x86 time saved |
| --- | --- | --- | --- | --- |
| Advanced patterns | 17 | +14.83% | 17 | -8.05% |
| Compilation | 6 | -2.05% | 6 | +9.15% |
| Complete grammar documents | 3 | -3.16% | 3 | -21.42% |
| Scanner document replays | 5 | +5.85% | 5 | -15.30% |
| Scanner group replays | 5 | -2.17% | 5 | +2.49% |
| Shared document searches | 32 | -6.00% | 32 | +6.23% |
| Short searches | 10 | +1.88% | 10 | -6.97% |
| TextMate line scans | 3 | -9.03% | 3 | -20.78% |

The full run's [162 individual comparisons](all-cases.md) remain unchanged by the repeat. [Publication comparisons](analysis/publication-comparisons.json) contain all 162 matched rows from the later full run. New JSON/Astro rows are retained in `analysis/comparison.json` and the current engine summary, outside historical averages.

## CSS on the cloud hosts

| Host | Oct 9, µs | Pre-CSS main, µs | Merged CSS main, µs | Vs. pre-CSS |
| --- | --- | --- | --- | --- |
| macOS arm64 | 72.621 | 73.811 | 76.439 | -3.56% |
| Linux x86-64 | 75.538 | 78.939 | 97.542 | -23.57% |

The local M1 Ultra comparison did not reproduce a material October 9 regression: 102.409 → 101.873 µs (+0.52%). The retained CSS candidate saved 5.33% in the paired native benchmark, 4.76% in the independent single-copy driver and 5.19% at four copies. These local measurements do not establish an identical benefit on virtual M4 or EPYC hosts. Cloud differences are observations from separate VMs, even when the source commit and software versions agree.

## Repeated same-commit cases

| Host | Case | Full run, µs | Repeat, µs | Repeat time saved |
| --- | --- | --- | --- | --- |
| macOS arm64 | literal_exact | 0.036 | 0.033 | +7.69% |
| macOS arm64 | quantifier_greedy | 0.041 | 0.043 | -4.66% |
| macOS arm64 | alternation_2_branch | 0.040 | 0.040 | +0.47% |
| macOS arm64 | alternation_10_branch | 0.025 | 0.025 | -0.67% |
| macOS arm64 | case_insensitive_phrase | 0.056 | 0.057 | -2.78% |
| macOS arm64 | named_capture_date | 0.062 | 0.063 | -1.48% |
| macOS arm64 | unicode_greek | 0.059 | 0.058 | +1.35% |
| macOS arm64 | lookaround_combined | 0.058 | 0.060 | -2.90% |
| macOS arm64 | backref_simple | 0.061 | 0.061 | +0.63% |
| macOS arm64 | regset_position_lead_rust | 0.083 | 0.087 | -4.90% |
| macOS arm64 | literal | 0.656 | 0.636 | +3.11% |
| macOS arm64 | named_capture | 2.988 | 2.621 | +12.28% |
| macOS arm64 | lookbehind | 1.484 | 1.310 | +11.74% |
| macOS arm64 | html_element_pairs | 172.637 | 177.533 | -2.84% |
| macOS arm64 | html_attribute_values | 513.147 | 645.994 | -25.89% |
| macOS arm64 | prices_lookbehind | 208.310 | 216.693 | -4.02% |
| macOS arm64 | quoted_strings | 237.481 | 248.138 | -4.49% |
| macOS arm64 | camel_case_words | 682.954 | 718.819 | -5.25% |
| macOS arm64 | markdown_emphasis | 157.952 | 175.519 | -11.12% |
| macOS arm64 | emoji_graphemes | 171.297 | 183.579 | -7.17% |
| macOS arm64 | password_rules | 17.476 | 15.068 | +13.78% |
| macOS arm64 | json_objects_recursive | 328.098 | 328.812 | -0.22% |
| macOS arm64 | html_nested_divs_recursive | 62.271 | 62.995 | -1.16% |
| macOS arm64 | variable_lookbehind | 984.172 | 1028.438 | -4.50% |
| macOS arm64 | atomic_possessive_strings | 21.872 | 23.191 | -6.03% |
| macOS arm64 | subexp_call_balanced | 32.839 | 29.233 | +10.98% |
| macOS arm64 | absent_comments | 26.015 | 23.606 | +9.26% |
| macOS arm64 | conditional_brackets | 35.131 | 35.581 | -1.28% |
| macOS arm64 | backref_ignorecase | 70.090 | 68.692 | +1.99% |
| macOS arm64 | lookbehind_alternation | 14.598 | 14.319 | +1.92% |
| macOS arm64 | html_tags | 296.202 | 302.488 | -2.12% |
| macOS arm64 | html_attributes | 487.404 | 510.437 | -4.73% |
| macOS arm64 | html_comments | 20.509 | 20.336 | +0.84% |
| macOS arm64 | hex_colors | 16.645 | 16.478 | +1.00% |
| macOS arm64 | email_addresses | 42.452 | 46.068 | -8.52% |
| macOS arm64 | emoji | 114.140 | 113.841 | +0.26% |
| macOS arm64 | ascii_emoticons | 64.450 | 67.179 | -4.23% |
| macOS arm64 | hashtags_mentions | 70.393 | 75.458 | -7.19% |
| macOS arm64 | ipv4_addresses | 178.509 | 154.416 | +13.50% |
| macOS arm64 | iso_timestamps | 68.767 | 68.722 | +0.07% |
| macOS arm64 | log_keywords_ignorecase | 92.097 | 79.680 | +13.48% |
| macOS arm64 | markdown_links | 43.661 | 42.208 | +3.33% |
| macOS arm64 | semantic_versions | 240.916 | 205.533 | +14.69% |
| macOS arm64 | json_strings | 291.199 | 291.876 | -0.23% |
| macOS arm64 | csv_fields | 182.460 | 180.811 | +0.90% |
| macOS arm64 | rfc5322_emails | 3179.189 | 3121.822 | +1.80% |
| macOS arm64 | ipv6_addresses | 1848.565 | 1794.276 | +2.94% |
| macOS arm64 | rfc3986_urls | 86.786 | 80.327 | +7.44% |
| macOS arm64 | keyword_alternation_200 | 850.554 | 802.479 | +5.65% |
| macOS arm64 | csv_last_column_backtracking | 142.191 | 138.982 | +2.26% |
| macOS arm64 | unicode_case_folding | 140.915 | 133.721 | +5.10% |
| macOS arm64 | email_validation | 4.670 | 4.841 | -3.67% |
| macOS arm64 | uuid_validation | 3.766 | 3.627 | +3.70% |
| macOS arm64 | number_validation | 3.977 | 4.136 | -4.00% |
| macOS arm64 | access_log_captures | 19.800 | 17.518 | +11.52% |
| macOS arm64 | url_extraction | 5.320 | 5.776 | -8.57% |
| macOS arm64 | unicode_words | 28.156 | 31.672 | -12.49% |
| macOS arm64 | email_redaction | 14.172 | 16.474 | -16.25% |
| macOS arm64 | literal_50k | 0.035 | 0.042 | -19.24% |
| macOS arm64 | no_match_50k | 1.036 | 1.120 | -8.14% |
| macOS arm64 | field_extract_50k | 0.043 | 0.043 | +1.69% |
| macOS arm64 | timestamp_50k | 0.066 | 0.066 | -0.56% |
| macOS arm64 | ts_279_compile_rust | 16917.361 | 16733.956 | +1.08% |
| macOS arm64 | css_117_compile_rust | 11360.218 | 10272.581 | +9.57% |
| macOS arm64 | rust_81_compile_rust | 379.686 | 390.421 | -2.83% |
| macOS arm64 | ts_279_tokenize_rust | 2.941 | 2.908 | +1.13% |
| macOS arm64 | css_117_tokenize_rust | 162.425 | 166.632 | -2.59% |
| macOS arm64 | rust_81_tokenize_rust | 1.695 | 1.616 | +4.64% |
| macOS arm64 | ts_279_document_28_lines_rust | 89.247 | 94.657 | -6.06% |
| macOS arm64 | css_117_document_19_lines_rust | 76.723 | 76.439 | +0.37% |
| macOS arm64 | rust_81_document_31_lines_rust | 39.799 | 38.699 | +2.76% |
| macOS arm64 | document | 3075.790 | 2718.956 | +11.60% |
| macOS arm64 | group_78 | 1374.089 | 1328.627 | +3.31% |
| macOS arm64 | document | 901.091 | 915.966 | -1.65% |
| macOS arm64 | group_15 | 122.871 | 114.495 | +6.82% |
| macOS arm64 | document | 1346.006 | 1172.705 | +12.88% |
| macOS arm64 | group_8 | 572.916 | 516.979 | +9.76% |
| macOS arm64 | document | 1035.684 | 974.129 | +5.94% |
| macOS arm64 | group_12 | 223.677 | 220.482 | +1.43% |
| macOS arm64 | document | 1605.162 | 1635.560 | -1.89% |
| macOS arm64 | group_13 | 193.106 | 194.815 | -0.88% |
| Linux x86-64 | literal_exact | 0.043 | 0.045 | -3.18% |
| Linux x86-64 | quantifier_greedy | 0.045 | 0.046 | -3.40% |
| Linux x86-64 | alternation_2_branch | 0.043 | 0.049 | -13.70% |
| Linux x86-64 | alternation_10_branch | 0.038 | 0.046 | -20.60% |
| Linux x86-64 | case_insensitive_phrase | 0.072 | 0.092 | -27.54% |
| Linux x86-64 | named_capture_date | 0.070 | 0.106 | -51.76% |
| Linux x86-64 | unicode_greek | 0.074 | 0.109 | -47.49% |
| Linux x86-64 | lookaround_combined | 0.097 | 0.097 | -0.07% |
| Linux x86-64 | backref_simple | 0.073 | 0.070 | +3.53% |
| Linux x86-64 | regset_position_lead_rust | 0.117 | 0.086 | +26.73% |
| Linux x86-64 | literal | 1.046 | 0.731 | +30.11% |
| Linux x86-64 | named_capture | 3.690 | 2.898 | +21.48% |
| Linux x86-64 | lookbehind | 1.654 | 1.471 | +11.12% |
| Linux x86-64 | html_element_pairs | 263.632 | 244.495 | +7.26% |
| Linux x86-64 | html_attribute_values | 689.669 | 778.379 | -12.86% |
| Linux x86-64 | prices_lookbehind | 268.066 | 256.747 | +4.22% |
| Linux x86-64 | quoted_strings | 302.593 | 337.252 | -11.45% |
| Linux x86-64 | camel_case_words | 888.059 | 1188.320 | -33.81% |
| Linux x86-64 | markdown_emphasis | 183.457 | 308.144 | -67.97% |
| Linux x86-64 | emoji_graphemes | 168.910 | 225.836 | -33.70% |
| Linux x86-64 | password_rules | 21.999 | 26.314 | -19.62% |
| Linux x86-64 | json_objects_recursive | 322.283 | 347.043 | -7.68% |
| Linux x86-64 | html_nested_divs_recursive | 74.857 | 71.114 | +5.00% |
| Linux x86-64 | variable_lookbehind | 1104.208 | 1240.287 | -12.32% |
| Linux x86-64 | atomic_possessive_strings | 26.828 | 33.774 | -25.89% |
| Linux x86-64 | subexp_call_balanced | 46.288 | 50.809 | -9.77% |
| Linux x86-64 | absent_comments | 36.845 | 34.842 | +5.44% |
| Linux x86-64 | conditional_brackets | 50.368 | 49.067 | +2.58% |
| Linux x86-64 | backref_ignorecase | 78.943 | 82.453 | -4.45% |
| Linux x86-64 | lookbehind_alternation | 17.834 | 19.463 | -9.13% |
| Linux x86-64 | html_tags | 374.636 | 361.834 | +3.42% |
| Linux x86-64 | html_attributes | 701.336 | 568.131 | +18.99% |
| Linux x86-64 | html_comments | 36.783 | 23.309 | +36.63% |
| Linux x86-64 | hex_colors | 33.080 | 22.113 | +33.15% |
| Linux x86-64 | email_addresses | 49.974 | 45.715 | +8.52% |
| Linux x86-64 | emoji | 165.406 | 150.615 | +8.94% |
| Linux x86-64 | ascii_emoticons | 160.414 | 104.575 | +34.81% |
| Linux x86-64 | hashtags_mentions | 114.651 | 79.597 | +30.57% |
| Linux x86-64 | ipv4_addresses | 288.805 | 222.686 | +22.89% |
| Linux x86-64 | iso_timestamps | 119.461 | 85.639 | +28.31% |
| Linux x86-64 | log_keywords_ignorecase | 83.579 | 86.587 | -3.60% |
| Linux x86-64 | markdown_links | 64.771 | 70.735 | -9.21% |
| Linux x86-64 | semantic_versions | 338.347 | 294.044 | +13.09% |
| Linux x86-64 | json_strings | 427.665 | 402.641 | +5.85% |
| Linux x86-64 | csv_fields | 407.082 | 210.323 | +48.33% |
| Linux x86-64 | rfc5322_emails | 7052.800 | 4435.220 | +37.11% |
| Linux x86-64 | ipv6_addresses | 2802.255 | 2126.487 | +24.12% |
| Linux x86-64 | rfc3986_urls | 157.046 | 80.300 | +48.87% |
| Linux x86-64 | keyword_alternation_200 | 1211.781 | 923.793 | +23.77% |
| Linux x86-64 | csv_last_column_backtracking | 280.384 | 170.206 | +39.30% |
| Linux x86-64 | unicode_case_folding | 305.433 | 200.645 | +34.31% |
| Linux x86-64 | email_validation | 11.126 | 7.727 | +30.55% |
| Linux x86-64 | uuid_validation | 8.668 | 5.225 | +39.72% |
| Linux x86-64 | number_validation | 9.613 | 5.227 | +45.63% |
| Linux x86-64 | access_log_captures | 32.505 | 25.741 | +20.81% |
| Linux x86-64 | url_extraction | 10.651 | 6.347 | +40.40% |
| Linux x86-64 | unicode_words | 77.521 | 38.149 | +50.79% |
| Linux x86-64 | email_redaction | 32.884 | 19.199 | +41.62% |
| Linux x86-64 | literal_50k | 0.061 | 0.044 | +27.83% |
| Linux x86-64 | no_match_50k | 1.902 | 0.780 | +58.97% |
| Linux x86-64 | field_extract_50k | 0.094 | 0.056 | +40.28% |
| Linux x86-64 | timestamp_50k | 0.103 | 0.063 | +38.84% |
| Linux x86-64 | ts_279_compile_rust | 20599.844 | 19838.933 | +3.69% |
| Linux x86-64 | css_117_compile_rust | 13388.481 | 13461.908 | -0.55% |
| Linux x86-64 | rust_81_compile_rust | 591.654 | 566.052 | +4.33% |
| Linux x86-64 | ts_279_tokenize_rust | 3.242 | 3.749 | -15.62% |
| Linux x86-64 | css_117_tokenize_rust | 152.397 | 151.183 | +0.80% |
| Linux x86-64 | rust_81_tokenize_rust | 2.135 | 2.416 | -13.15% |
| Linux x86-64 | ts_279_document_28_lines_rust | 116.130 | 113.190 | +2.53% |
| Linux x86-64 | css_117_document_19_lines_rust | 125.495 | 97.542 | +22.27% |
| Linux x86-64 | rust_81_document_31_lines_rust | 67.611 | 50.395 | +25.46% |
| Linux x86-64 | document | 4730.692 | 4553.980 | +3.74% |
| Linux x86-64 | group_78 | 1893.257 | 1222.875 | +35.41% |
| Linux x86-64 | document | 1685.602 | 1101.226 | +34.67% |
| Linux x86-64 | group_15 | 200.793 | 141.917 | +29.32% |
| Linux x86-64 | document | 1714.622 | 1289.969 | +24.77% |
| Linux x86-64 | group_8 | 657.255 | 537.431 | +18.23% |
| Linux x86-64 | document | 1280.441 | 1094.348 | +14.53% |
| Linux x86-64 | group_12 | 281.561 | 252.687 | +10.25% |
| Linux x86-64 | document | 1862.679 | 1615.589 | +13.27% |
| Linux x86-64 | group_13 | 270.763 | 193.053 | +28.70% |

The repeated suite uses the same runtime source, fixtures and toolchains. Changes between those two runs cannot be attributed to a code change. The retained before/after engine and C-relative values help expose measurement variation without replacing it with a causal explanation.

## Verification and limits


I verified the merged runtime commit `d791bf1b011f6e98619969872a4d7cb7161912a7` in every clean build receipt, along with the pinned input and lockfile hashes. Every measured executable differs from its pre-CSS equivalent. Compiler and Node versions match the preceding main run; CPU descriptions can differ within the same runner label. In the first full run, the x86 shared job reported an Intel Xeon without AVX-512, replacing an AMD EPYC with AVX-512 in the preceding run. Physical VM placement and scheduling remain uncontrolled. The retained workflow metadata and artifact IDs identify each source run.

The full run covers 85 cases per platform: 32 shared, 17 advanced, 13 micro/compilation and 23 textmate cases. The full repeat covers the same 85 cases on each platform. Validation precedes timing. Unsupported syntax and different-result cases remain excluded, with reasons in `engine-notes.json`.

The workflow requests five seconds, 30 samples and 500 ms warmup. Existing `battle_bench` groups override measurement time to four seconds; captured scanner replays use five seconds, and Shiki JS uses its Node batch timer. Preparation, compilation, input construction and validation stay outside execution timing. Raw Criterion estimates and logs are retained; individual sample files and executables are not uploaded by this workflow, but executable hashes are recorded.

The October 9 baseline combines full run 37989957431 (`a38768bc80121a1bd72e3e280f12063dc407bc04`) for shared/oniguruma/micro with later run 37999607702 (`b93c6d2035f4919480f43d9b7d4134541e4cb9b5`) for textmate. The later CSS baseline already includes pre-filter retirement. There are 81 historical matches per platform; four JSON/Astro cases were added later and carry no historical speedup claim.

Positive percentages mean less execution time. Category changes are equally weighted geometric means of matched case ratios. C-relative changes use `100 × (1 - (new_Ferroni / new_C) / (old_Ferroni / old_C))`; they are diagnostic rather than a causal correction. Separate VMs, sequential engine timings and code layout can affect results. These data do not isolate the effect of an individual optimization, and there is no paired significance test.

`verify.py` checks full-run completeness, exact source SHA, clean builds, hashes, versions, accepted-case coverage and positive finite means. `analyze.py` derives the historical comparisons. `analysis/fixture-comparability.json` retains the unchanged fixture/harness hashes against the pre-CSS main run. The scripts and retained inputs make the analysis reviewable without changing older evidence.

The homepage figures and all 85 case tables are generated from both retained runs with `scripts/compare-engines.py figures` and `tables`. README text summarizes the same values. SHA256SUMS covers every retained evidence file except the manifest itself. Existing sealed reports remain unchanged.
