# Additional source-backed regex comparisons

Measured revision: `80ff3b4b73cbcdd02407b34a8422eec1565061ee`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38772515949).

The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.

## tokenizers

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.067 |
| rust_erroraware | compile | 2 | 1.005 |
| rust_erroraware | first_use | 2 | 1.040 |
| c | steady | 2 | 2.917 |
| c | compile | 2 | 1.657 |
| c | first_use | 2 | 1.647 |
| onigmo | steady | 2 | 1.589 |
| onigmo | compile | 2 | 1.580 |
| onigmo | first_use | 2 | 1.604 |
| pcre2_jit | steady | 2 | 0.423 |
| pcre2_jit | compile | 2 | 0.181 |
| pcre2_jit | first_use | 2 | 0.207 |
| pcre2 | steady | 2 | 1.986 |
| pcre2 | compile | 2 | 0.083 |
| pcre2 | first_use | 2 | 0.088 |
| fancy_regex | steady | 2 | 1.547 |
| fancy_regex | compile | 2 | 8.398 |
| fancy_regex | first_use | 2 | 8.450 |
| fancy_regex_seek | steady | 2 | 2.102 |
| fancy_regex_seek | compile | 2 | 15.933 |
| fancy_regex_seek | first_use | 2 | 18.516 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.001 |
| rust_erroraware | compile | 2 | 0.984 |
| rust_erroraware | first_use | 2 | 1.048 |
| c | steady | 2 | 1.660 |
| c | compile | 2 | 0.616 |
| c | first_use | 2 | 0.598 |
| onigmo | steady | 2 | 1.551 |
| onigmo | compile | 2 | 0.627 |
| onigmo | first_use | 2 | 0.726 |
| pcre2_jit | steady | 2 | 0.516 |
| pcre2_jit | compile | 2 | 0.190 |
| pcre2_jit | first_use | 2 | 0.197 |
| pcre2 | steady | 2 | 1.792 |
| pcre2 | compile | 2 | 0.076 |
| pcre2 | first_use | 2 | 0.091 |
| fancy_regex | steady | 2 | 1.456 |
| fancy_regex | compile | 2 | 6.951 |
| fancy_regex | first_use | 2 | 7.445 |
| fancy_regex_seek | steady | 2 | 2.321 |
| fancy_regex_seek | compile | 2 | 14.189 |
| fancy_regex_seek | first_use | 2 | 16.819 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

Raw samples, logs, source snapshots, dependency locks and eligibility are retained per scope/platform in evidence.tar.gz. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Host power/thermal state is unavailable. Memory is not measured.

```sh
python3 scripts/publish-additional-comparisons.py --check
```
