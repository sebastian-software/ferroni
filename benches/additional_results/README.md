# Additional source-backed regex comparisons

Measured revision: `7b9933c07d5f888c816793d97449012fd7f872ba`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38571723690).

The regexr rows use a freshly measured Ferroni standard-API baseline on the same VM and process-repetition schedule. They do not divide new regexr timings by an older Ferroni run. The earlier engines retain their own recorded comparison run.

regexr 0.6.1 uses Unicode mode (`(?u)`), default SIMD and separate JIT-off/JIT-enabled profiles. JIT-enabled selection may fall back to a non-JIT backend; eligibility records retain the actual backend. Native Regex methods, capture iteration and literal replacement are timed. Its public API lacks context-preserving offset searches, which are reported as API gaps. First-match/capture fallible checks run outside timing; iteration/replacement do not expose all execution errors.

The separate tiktoken corpus measures complete regex pre-tokenization spans for the current cl100k_base and o200k_base patterns. It excludes BPE merges, token IDs, special-token processing and complete tokenizer throughput. Every profile must reproduce every original Python-regex oracle before timing.

## regexr

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| regexr | steady | 10 | 6.955 |
| regexr | compile | 10 | 3.857 |
| regexr | first_use | 10 | 4.553 |
| regexr_jit | steady | 10 | 7.231 |
| regexr_jit | compile | 10 | 3.955 |
| regexr_jit | first_use | 10 | 4.612 |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| regexr | steady | 10 | 7.436 |
| regexr | compile | 10 | 3.823 |
| regexr | first_use | 10 | 4.572 |
| regexr_jit | steady | 10 | 7.360 |
| regexr_jit | compile | 10 | 3.841 |
| regexr_jit | first_use | 10 | 4.509 |

## tokenizers

### macos-arm64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.728 |
| rust_erroraware | compile | 2 | 1.000 |
| rust_erroraware | first_use | 2 | 0.999 |
| c | steady | 2 | 2.725 |
| c | compile | 2 | 3.781 |
| c | first_use | 2 | 3.745 |
| onigmo | steady | 2 | 1.434 |
| onigmo | compile | 2 | 3.649 |
| onigmo | first_use | 2 | 3.676 |
| pcre2_jit | steady | 2 | 0.394 |
| pcre2_jit | compile | 2 | 0.407 |
| pcre2_jit | first_use | 2 | 0.443 |
| pcre2 | steady | 2 | 1.768 |
| pcre2 | compile | 2 | 0.195 |
| pcre2 | first_use | 2 | 0.206 |
| fancy_regex | steady | 2 | 1.450 |
| fancy_regex | compile | 2 | 19.309 |
| fancy_regex | first_use | 2 | 19.644 |
| fancy_regex_seek | steady | 2 | 1.936 |
| fancy_regex_seek | compile | 2 | 37.329 |
| fancy_regex_seek | first_use | 2 | 42.135 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |
| regexr | steady | 1 | 0.831 |
| regexr | compile | 1 | 6.739 |
| regexr | first_use | 1 | 6.613 |
| regexr_jit | steady | 1 | 0.832 |
| regexr_jit | compile | 1 | 7.001 |
| regexr_jit | first_use | 1 | 6.823 |

### linux-x86-64

| Profile | Phase | Common cases | Ratio: comparator / Ferroni |
| --- | --- | ---: | ---: |
| rust_erroraware | steady | 2 | 1.783 |
| rust_erroraware | compile | 2 | 0.945 |
| rust_erroraware | first_use | 2 | 0.968 |
| c | steady | 2 | 1.566 |
| c | compile | 2 | 1.552 |
| c | first_use | 2 | 1.611 |
| onigmo | steady | 2 | 1.323 |
| onigmo | compile | 2 | 1.406 |
| onigmo | first_use | 2 | 1.397 |
| pcre2_jit | steady | 2 | 0.386 |
| pcre2_jit | compile | 2 | 0.422 |
| pcre2_jit | first_use | 2 | 0.498 |
| pcre2 | steady | 2 | 1.520 |
| pcre2 | compile | 2 | 0.194 |
| pcre2 | first_use | 2 | 0.221 |
| fancy_regex | steady | 2 | 1.322 |
| fancy_regex | compile | 2 | 18.185 |
| fancy_regex | first_use | 2 | 18.523 |
| fancy_regex_seek | steady | 2 | 1.790 |
| fancy_regex_seek | compile | 2 | 35.261 |
| fancy_regex_seek | first_use | 2 | 41.059 |
| regex | steady | 0 | — |
| regex | compile | 0 | — |
| regex | first_use | 0 | — |
| regexr | steady | 1 | 0.828 |
| regexr | compile | 1 | 4.674 |
| regexr | first_use | 1 | 4.746 |
| regexr_jit | steady | 1 | 0.792 |
| regexr_jit | compile | 1 | 5.019 |
| regexr_jit | first_use | 1 | 5.046 |

Raw samples, logs, source snapshots, dependency locks, eligibility and selected backends are retained per scope/platform in evidence.tar.gz. Three process repetitions run on one VM per platform for this additional run; host power/thermal state is unavailable. Memory is not measured.

```sh
python3 scripts/publish-additional-comparisons.py --check
```
