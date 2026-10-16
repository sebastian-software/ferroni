# Additional source-backed regex comparisons

Measured revision: `c08e0b99cc250408e911e5bd51e3a3df5198b124`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38924508724).

The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.

## tokenizers

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.034 |
| rust_erroraware | compile | 2 | 1.010 |
| rust_erroraware | first_use | 2 | 1.003 |
| c | steady | 2 | 2.767 |
| c | compile | 2 | 1.669 |
| c | first_use | 2 | 1.671 |
| onigmo | steady | 2 | 1.620 |
| onigmo | compile | 2 | 1.655 |
| onigmo | first_use | 2 | 1.661 |
| pcre2_jit | steady | 2 | 0.435 |
| pcre2_jit | compile | 2 | 0.191 |
| pcre2_jit | first_use | 2 | 0.207 |
| pcre2 | steady | 2 | 1.996 |
| pcre2 | compile | 2 | 0.085 |
| pcre2 | first_use | 2 | 0.091 |
| fancy_regex | steady | 2 | 1.572 |
| fancy_regex | compile | 2 | 8.667 |
| fancy_regex | first_use | 2 | 8.815 |
| fancy_regex_seek | steady | 2 | 2.150 |
| fancy_regex_seek | compile | 2 | 16.831 |
| fancy_regex_seek | first_use | 2 | 19.064 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.106 |
| rust_erroraware | compile | 2 | 1.027 |
| rust_erroraware | first_use | 2 | 1.038 |
| c | steady | 2 | 1.873 |
| c | compile | 2 | 0.658 |
| c | first_use | 2 | 0.593 |
| onigmo | steady | 2 | 1.597 |
| onigmo | compile | 2 | 0.574 |
| onigmo | first_use | 2 | 0.591 |
| pcre2_jit | steady | 2 | 0.469 |
| pcre2_jit | compile | 2 | 0.174 |
| pcre2_jit | first_use | 2 | 0.183 |
| pcre2 | steady | 2 | 1.822 |
| pcre2 | compile | 2 | 0.079 |
| pcre2 | first_use | 2 | 0.083 |
| fancy_regex | steady | 2 | 1.604 |
| fancy_regex | compile | 2 | 7.483 |
| fancy_regex | first_use | 2 | 7.053 |
| fancy_regex_seek | steady | 2 | 2.216 |
| fancy_regex_seek | compile | 2 | 14.241 |
| fancy_regex_seek | first_use | 2 | 15.046 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

Raw samples, logs, source snapshots, dependency locks and eligibility are retained per scope/platform in evidence.tar.gz. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Host power/thermal state is unavailable. Memory is not measured.

```sh
python3 scripts/publish-additional-comparisons.py --check
```
