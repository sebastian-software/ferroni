# Additional source-backed regex comparisons

Measured revision: `1c8d45f57677ddd9f52bd45c75297c113ceb9bd6`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38808376508).

The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.

## tokenizers

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.053 |
| rust_erroraware | compile | 2 | 1.003 |
| rust_erroraware | first_use | 2 | 0.987 |
| c | steady | 2 | 2.827 |
| c | compile | 2 | 1.643 |
| c | first_use | 2 | 1.631 |
| onigmo | steady | 2 | 1.582 |
| onigmo | compile | 2 | 1.657 |
| onigmo | first_use | 2 | 1.658 |
| pcre2_jit | steady | 2 | 0.442 |
| pcre2_jit | compile | 2 | 0.191 |
| pcre2_jit | first_use | 2 | 0.208 |
| pcre2 | steady | 2 | 2.001 |
| pcre2 | compile | 2 | 0.087 |
| pcre2 | first_use | 2 | 0.092 |
| fancy_regex | steady | 2 | 1.595 |
| fancy_regex | compile | 2 | 8.842 |
| fancy_regex | first_use | 2 | 8.789 |
| fancy_regex_seek | steady | 2 | 2.182 |
| fancy_regex_seek | compile | 2 | 16.648 |
| fancy_regex_seek | first_use | 2 | 18.931 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.144 |
| rust_erroraware | compile | 2 | 1.050 |
| rust_erroraware | first_use | 2 | 0.956 |
| c | steady | 2 | 1.870 |
| c | compile | 2 | 0.604 |
| c | first_use | 2 | 0.589 |
| onigmo | steady | 2 | 1.753 |
| onigmo | compile | 2 | 0.628 |
| onigmo | first_use | 2 | 0.559 |
| pcre2_jit | steady | 2 | 0.484 |
| pcre2_jit | compile | 2 | 0.177 |
| pcre2_jit | first_use | 2 | 0.199 |
| pcre2 | steady | 2 | 1.804 |
| pcre2 | compile | 2 | 0.078 |
| pcre2 | first_use | 2 | 0.092 |
| fancy_regex | steady | 2 | 1.533 |
| fancy_regex | compile | 2 | 7.396 |
| fancy_regex | first_use | 2 | 7.312 |
| fancy_regex_seek | steady | 2 | 2.181 |
| fancy_regex_seek | compile | 2 | 14.488 |
| fancy_regex_seek | first_use | 2 | 16.291 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |

Raw samples, logs, source snapshots, dependency locks and eligibility are retained per scope/platform in evidence.tar.gz. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Host power/thermal state is unavailable. Memory is not measured.

```sh
python3 scripts/publish-additional-comparisons.py --check
```
