# Additional source-backed regex comparisons

Measured revision: `a51a2fd917eb4eacd679d565842b0ec695e3d2e6`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38702442347).

The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.

## tokenizers

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.043 |
| rust_erroraware | compile | 2 | 1.035 |
| rust_erroraware | first_use | 2 | 0.987 |
| c | steady | 2 | 2.782 |
| c | compile | 2 | 1.640 |
| c | first_use | 2 | 1.697 |
| onigmo | steady | 2 | 1.578 |
| onigmo | compile | 2 | 1.622 |
| onigmo | first_use | 2 | 1.592 |
| pcre2_jit | steady | 2 | 0.427 |
| pcre2_jit | compile | 2 | 0.185 |
| pcre2_jit | first_use | 2 | 0.207 |
| pcre2 | steady | 2 | 1.971 |
| pcre2 | compile | 2 | 0.086 |
| pcre2 | first_use | 2 | 0.089 |
| fancy_regex | steady | 2 | 1.552 |
| fancy_regex | compile | 2 | 8.928 |
| fancy_regex | first_use | 2 | 8.959 |
| fancy_regex_seek | steady | 2 | 2.129 |
| fancy_regex_seek | compile | 2 | 17.687 |
| fancy_regex_seek | first_use | 2 | 20.312 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.092 |
| rust_erroraware | compile | 2 | 1.021 |
| rust_erroraware | first_use | 2 | 1.016 |
| c | steady | 2 | 1.862 |
| c | compile | 2 | 0.596 |
| c | first_use | 2 | 0.577 |
| onigmo | steady | 2 | 1.565 |
| onigmo | compile | 2 | 0.599 |
| onigmo | first_use | 2 | 0.606 |
| pcre2_jit | steady | 2 | 0.472 |
| pcre2_jit | compile | 2 | 0.184 |
| pcre2_jit | first_use | 2 | 0.197 |
| pcre2 | steady | 2 | 1.733 |
| pcre2 | compile | 2 | 0.079 |
| pcre2 | first_use | 2 | 0.089 |
| fancy_regex | steady | 2 | 1.501 |
| fancy_regex | compile | 2 | 7.159 |
| fancy_regex | first_use | 2 | 7.428 |
| fancy_regex_seek | steady | 2 | 2.146 |
| fancy_regex_seek | compile | 2 | 14.263 |
| fancy_regex_seek | first_use | 2 | 16.069 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

Raw samples, logs, source snapshots, dependency locks and eligibility are retained per scope/platform in evidence.tar.gz. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Host power/thermal state is unavailable. Memory is not measured.

```sh
python3 scripts/publish-additional-comparisons.py --check
```
