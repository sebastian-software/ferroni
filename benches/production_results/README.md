# Published source-backed regex results

Measured revision: `c08e0b99cc250408e911e5bd51e3a3df5198b124`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38924508724).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Standard searches can reuse cached match workspace; parameterized searches currently create fresh workspace. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.838 | 17 | 0 | 4 |
| rust_erroraware | compile | 21 | 12 | 1.002 | 0 | 0 | 21 |
| rust_erroraware | first_use | 21 | 12 | 0.949 | 0 | 7 | 14 |
| c | steady | 21 | 12 | 3.444 | 21 | 0 | 0 |
| c | compile | 21 | 12 | 0.385 | 5 | 16 | 0 |
| c | first_use | 21 | 12 | 0.403 | 5 | 16 | 0 |
| onigmo | steady | 20 | 12 | 3.928 | 19 | 0 | 1 |
| onigmo | compile | 20 | 12 | 0.523 | 6 | 14 | 0 |
| onigmo | first_use | 20 | 12 | 0.508 | 6 | 14 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.708 | 4 | 15 | 2 |
| pcre2_jit | compile | 21 | 12 | 0.261 | 0 | 21 | 0 |
| pcre2_jit | first_use | 21 | 12 | 0.288 | 0 | 20 | 1 |
| pcre2 | steady | 21 | 12 | 2.068 | 18 | 2 | 1 |
| pcre2 | compile | 21 | 12 | 0.083 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.095 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 3.056 | 15 | 0 | 2 |
| fancy_regex | compile | 17 | 10 | 1.867 | 12 | 5 | 0 |
| fancy_regex | first_use | 17 | 10 | 1.891 | 12 | 5 | 0 |
| fancy_regex_seek | steady | 17 | 10 | 3.067 | 15 | 0 | 2 |
| fancy_regex_seek | compile | 17 | 10 | 2.048 | 11 | 5 | 1 |
| fancy_regex_seek | first_use | 17 | 10 | 2.091 | 12 | 4 | 1 |
| regex | steady | 10 | 5 | 1.360 | 5 | 3 | 2 |
| regex | compile | 10 | 5 | 1.760 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.786 | 6 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.800 | 12 | 0 | 9 |
| rust_erroraware | compile | 21 | 12 | 1.007 | 0 | 0 | 21 |
| rust_erroraware | first_use | 21 | 12 | 0.945 | 0 | 6 | 15 |
| c | steady | 21 | 12 | 2.701 | 18 | 0 | 3 |
| c | compile | 21 | 12 | 0.343 | 4 | 17 | 0 |
| c | first_use | 21 | 12 | 0.364 | 4 | 17 | 0 |
| onigmo | steady | 20 | 12 | 3.622 | 18 | 2 | 0 |
| onigmo | compile | 20 | 12 | 0.420 | 5 | 15 | 0 |
| onigmo | first_use | 20 | 12 | 0.418 | 5 | 15 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.633 | 4 | 15 | 2 |
| pcre2_jit | compile | 21 | 12 | 0.238 | 0 | 21 | 0 |
| pcre2_jit | first_use | 21 | 12 | 0.259 | 0 | 20 | 1 |
| pcre2 | steady | 21 | 12 | 1.885 | 16 | 3 | 2 |
| pcre2 | compile | 21 | 12 | 0.064 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.099 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 2.937 | 16 | 0 | 1 |
| fancy_regex | compile | 17 | 10 | 1.390 | 9 | 5 | 3 |
| fancy_regex | first_use | 17 | 10 | 1.430 | 10 | 4 | 3 |
| fancy_regex_seek | steady | 17 | 10 | 2.981 | 17 | 0 | 0 |
| fancy_regex_seek | compile | 17 | 10 | 1.590 | 10 | 4 | 3 |
| fancy_regex_seek | first_use | 17 | 10 | 1.612 | 10 | 4 | 3 |
| regex | steady | 10 | 5 | 1.394 | 8 | 1 | 1 |
| regex | compile | 10 | 5 | 1.754 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.781 | 6 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Memory was not measured in this run.
