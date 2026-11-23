# Additional source-backed regex comparisons

Measured revision: `bd688eea256da1e3255cc5790d8b582913457c27`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/45266137470).

The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.

## tokenizers

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.097 |
| rust_erroraware | compile | 2 | 1.022 |
| rust_erroraware | first_use | 2 | 1.009 |
| c | steady | 2 | 2.865 |
| c | compile | 2 | 1.969 |
| c | first_use | 2 | 1.956 |
| onigmo | steady | 2 | 1.571 |
| onigmo | compile | 2 | 1.958 |
| onigmo | first_use | 2 | 1.869 |
| pcre2_jit | steady | 2 | 0.420 |
| pcre2_jit | compile | 2 | 0.224 |
| pcre2_jit | first_use | 2 | 0.237 |
| pcre2 | steady | 2 | 1.952 |
| pcre2 | compile | 2 | 0.103 |
| pcre2 | first_use | 2 | 0.108 |
| fancy_regex | steady | 2 | 1.629 |
| fancy_regex | compile | 2 | 10.181 |
| fancy_regex | first_use | 2 | 10.279 |
| fancy_regex_seek | steady | 2 | 2.125 |
| fancy_regex_seek | compile | 2 | 20.033 |
| fancy_regex_seek | first_use | 2 | 22.048 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.105 |
| rust_erroraware | compile | 2 | 1.033 |
| rust_erroraware | first_use | 2 | 0.973 |
| c | steady | 2 | 1.913 |
| c | compile | 2 | 0.734 |
| c | first_use | 2 | 0.688 |
| onigmo | steady | 2 | 1.549 |
| onigmo | compile | 2 | 0.692 |
| onigmo | first_use | 2 | 0.676 |
| pcre2_jit | steady | 2 | 0.448 |
| pcre2_jit | compile | 2 | 0.213 |
| pcre2_jit | first_use | 2 | 0.212 |
| pcre2 | steady | 2 | 1.739 |
| pcre2 | compile | 2 | 0.098 |
| pcre2 | first_use | 2 | 0.104 |
| fancy_regex | steady | 2 | 1.562 |
| fancy_regex | compile | 2 | 8.972 |
| fancy_regex | first_use | 2 | 8.696 |
| fancy_regex_seek | steady | 2 | 2.278 |
| fancy_regex_seek | compile | 2 | 17.387 |
| fancy_regex_seek | first_use | 2 | 18.987 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

Raw samples, logs, source snapshots, dependency locks and eligibility are retained per scope/platform in evidence.tar.gz. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Host power/thermal state is unavailable. Memory is not measured.

```sh
python3 scripts/publish-additional-comparisons.py --check
```
