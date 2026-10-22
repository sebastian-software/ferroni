# Additional source-backed regex comparisons

Measured revision: `817d2b349b88cec0b2c130e416a3caa90220e129`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/39852407433).

The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.

## tokenizers

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.073 |
| rust_erroraware | compile | 2 | 0.974 |
| rust_erroraware | first_use | 2 | 0.970 |
| c | steady | 2 | 2.751 |
| c | compile | 2 | 1.984 |
| c | first_use | 2 | 1.989 |
| onigmo | steady | 2 | 1.590 |
| onigmo | compile | 2 | 1.974 |
| onigmo | first_use | 2 | 1.963 |
| pcre2_jit | steady | 2 | 0.429 |
| pcre2_jit | compile | 2 | 0.232 |
| pcre2_jit | first_use | 2 | 0.240 |
| pcre2 | steady | 2 | 1.942 |
| pcre2 | compile | 2 | 0.102 |
| pcre2 | first_use | 2 | 0.109 |
| fancy_regex | steady | 2 | 1.549 |
| fancy_regex | compile | 2 | 10.347 |
| fancy_regex | first_use | 2 | 10.426 |
| fancy_regex_seek | steady | 2 | 2.096 |
| fancy_regex_seek | compile | 2 | 19.941 |
| fancy_regex_seek | first_use | 2 | 22.330 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.090 |
| rust_erroraware | compile | 2 | 0.952 |
| rust_erroraware | first_use | 2 | 0.947 |
| c | steady | 2 | 1.795 |
| c | compile | 2 | 0.682 |
| c | first_use | 2 | 0.660 |
| onigmo | steady | 2 | 1.634 |
| onigmo | compile | 2 | 0.675 |
| onigmo | first_use | 2 | 0.656 |
| pcre2_jit | steady | 2 | 0.455 |
| pcre2_jit | compile | 2 | 0.204 |
| pcre2_jit | first_use | 2 | 0.214 |
| pcre2 | steady | 2 | 1.699 |
| pcre2 | compile | 2 | 0.091 |
| pcre2 | first_use | 2 | 0.098 |
| fancy_regex | steady | 2 | 1.433 |
| fancy_regex | compile | 2 | 8.061 |
| fancy_regex | first_use | 2 | 8.165 |
| fancy_regex_seek | steady | 2 | 2.043 |
| fancy_regex_seek | compile | 2 | 15.966 |
| fancy_regex_seek | first_use | 2 | 18.289 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

Raw samples, logs, source snapshots, dependency locks and eligibility are retained per scope/platform in evidence.tar.gz. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Host power/thermal state is unavailable. Memory is not measured.

```sh
python3 scripts/publish-additional-comparisons.py --check
```
