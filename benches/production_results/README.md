# Published source-backed regex results

Measured revision: `1452dce543acb2f74b5988e713a57fb1d3615559`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38531377256).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Standard searches can reuse cached match workspace; parameterized searches currently create fresh workspace. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.276 | 18 | 0 | 3 |
| rust_erroraware | compile | 21 | 12 | 1.005 | 0 | 0 | 21 |
| rust_erroraware | first_use | 21 | 12 | 1.010 | 2 | 0 | 19 |
| c | steady | 21 | 12 | 1.402 | 20 | 1 | 0 |
| c | compile | 21 | 12 | 0.915 | 6 | 14 | 1 |
| c | first_use | 21 | 12 | 0.978 | 6 | 14 | 1 |
| onigmo | steady | 20 | 12 | 1.586 | 15 | 2 | 3 |
| onigmo | compile | 20 | 12 | 1.246 | 7 | 12 | 1 |
| onigmo | first_use | 20 | 12 | 1.255 | 7 | 12 | 1 |
| pcre2_jit | steady | 21 | 12 | 0.285 | 0 | 21 | 0 |
| pcre2_jit | compile | 21 | 12 | 0.619 | 3 | 16 | 2 |
| pcre2_jit | first_use | 21 | 12 | 0.703 | 6 | 12 | 3 |
| pcre2 | steady | 21 | 12 | 0.832 | 10 | 6 | 5 |
| pcre2 | compile | 21 | 12 | 0.199 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.243 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 1.012 | 11 | 5 | 1 |
| fancy_regex | compile | 17 | 10 | 4.303 | 16 | 1 | 0 |
| fancy_regex | first_use | 17 | 10 | 4.587 | 16 | 1 | 0 |
| fancy_regex_seek | steady | 17 | 10 | 1.012 | 11 | 5 | 1 |
| fancy_regex_seek | compile | 17 | 10 | 4.741 | 16 | 1 | 0 |
| fancy_regex_seek | first_use | 17 | 10 | 5.043 | 16 | 1 | 0 |
| regex | steady | 10 | 5 | 0.665 | 4 | 5 | 1 |
| regex | compile | 10 | 5 | 4.232 | 10 | 0 | 0 |
| regex | first_use | 10 | 5 | 4.696 | 10 | 0 | 0 |

The exact common cases and all absolute timings are available in `publication.json` and in the website's case tables.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.285 | 18 | 0 | 3 |
| rust_erroraware | compile | 21 | 12 | 1.008 | 0 | 0 | 21 |
| rust_erroraware | first_use | 21 | 12 | 1.023 | 3 | 0 | 18 |
| c | steady | 21 | 12 | 1.127 | 9 | 7 | 5 |
| c | compile | 21 | 12 | 0.867 | 6 | 15 | 0 |
| c | first_use | 21 | 12 | 0.947 | 6 | 14 | 1 |
| onigmo | steady | 20 | 12 | 1.513 | 14 | 3 | 3 |
| onigmo | compile | 20 | 12 | 1.120 | 7 | 13 | 0 |
| onigmo | first_use | 20 | 12 | 1.133 | 7 | 13 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.258 | 0 | 21 | 0 |
| pcre2_jit | compile | 21 | 12 | 0.617 | 5 | 15 | 1 |
| pcre2_jit | first_use | 21 | 12 | 0.691 | 6 | 11 | 4 |
| pcre2 | steady | 21 | 12 | 0.784 | 11 | 10 | 0 |
| pcre2 | compile | 21 | 12 | 0.164 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.266 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 0.998 | 11 | 5 | 1 |
| fancy_regex | compile | 17 | 10 | 3.392 | 16 | 1 | 0 |
| fancy_regex | first_use | 17 | 10 | 3.600 | 16 | 1 | 0 |
| fancy_regex_seek | steady | 17 | 10 | 0.994 | 11 | 5 | 1 |
| fancy_regex_seek | compile | 17 | 10 | 3.785 | 16 | 1 | 0 |
| fancy_regex_seek | first_use | 17 | 10 | 4.097 | 16 | 1 | 0 |
| regex | steady | 10 | 5 | 0.672 | 3 | 5 | 2 |
| regex | compile | 10 | 5 | 4.428 | 10 | 0 | 0 |
| regex | first_use | 10 | 5 | 4.908 | 10 | 0 | 0 |

The exact common cases and all absolute timings are available in `publication.json` and in the website's case tables.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. These are three process repetitions on one VM per platform, not measurements across multiple independent hosts or a model of production traffic. Memory was not measured in this run.
