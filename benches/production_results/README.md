# Published source-backed regex results

Measured revision: `8796aef33af2f874a0fdf56685f831cf175a7cd0`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/39588030348).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Standard searches can reuse cached match workspace; parameterized searches currently create fresh workspace. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 23 | 13 | 1.767 | 16 | 0 | 7 |
| rust_erroraware | compile | 23 | 13 | 1.001 | 0 | 0 | 23 |
| rust_erroraware | first_use | 23 | 13 | 0.941 | 0 | 10 | 13 |
| c | steady | 23 | 13 | 3.262 | 23 | 0 | 0 |
| c | compile | 23 | 13 | 0.353 | 5 | 18 | 0 |
| c | first_use | 23 | 13 | 0.376 | 5 | 18 | 0 |
| onigmo | steady | 22 | 13 | 3.671 | 20 | 0 | 2 |
| onigmo | compile | 22 | 13 | 0.489 | 6 | 16 | 0 |
| onigmo | first_use | 22 | 13 | 0.483 | 6 | 16 | 0 |
| pcre2_jit | steady | 23 | 13 | 0.677 | 4 | 18 | 1 |
| pcre2_jit | compile | 23 | 13 | 0.260 | 0 | 23 | 0 |
| pcre2_jit | first_use | 23 | 13 | 0.291 | 0 | 22 | 1 |
| pcre2 | steady | 23 | 13 | 1.957 | 20 | 3 | 0 |
| pcre2 | compile | 23 | 13 | 0.083 | 0 | 23 | 0 |
| pcre2 | first_use | 23 | 13 | 0.098 | 0 | 23 | 0 |
| fancy_regex | steady | 19 | 11 | 3.106 | 17 | 0 | 2 |
| fancy_regex | compile | 19 | 11 | 2.000 | 13 | 4 | 2 |
| fancy_regex | first_use | 19 | 11 | 2.036 | 14 | 3 | 2 |
| fancy_regex_seek | steady | 19 | 11 | 3.062 | 17 | 0 | 2 |
| fancy_regex_seek | compile | 19 | 11 | 2.267 | 13 | 4 | 2 |
| fancy_regex_seek | first_use | 19 | 11 | 2.302 | 14 | 3 | 2 |
| regex | steady | 11 | 6 | 1.476 | 6 | 1 | 4 |
| regex | compile | 11 | 6 | 1.833 | 7 | 3 | 1 |
| regex | first_use | 11 | 6 | 1.914 | 7 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 23 | 13 | 1.744 | 18 | 0 | 5 |
| rust_erroraware | compile | 23 | 13 | 1.003 | 0 | 0 | 23 |
| rust_erroraware | first_use | 23 | 13 | 0.955 | 0 | 6 | 17 |
| c | steady | 23 | 13 | 2.607 | 21 | 0 | 2 |
| c | compile | 23 | 13 | 0.312 | 4 | 19 | 0 |
| c | first_use | 23 | 13 | 0.339 | 4 | 19 | 0 |
| onigmo | steady | 22 | 13 | 3.465 | 20 | 1 | 1 |
| onigmo | compile | 22 | 13 | 0.397 | 5 | 17 | 0 |
| onigmo | first_use | 22 | 13 | 0.396 | 5 | 17 | 0 |
| pcre2_jit | steady | 23 | 13 | 0.609 | 4 | 18 | 1 |
| pcre2_jit | compile | 23 | 13 | 0.238 | 0 | 23 | 0 |
| pcre2_jit | first_use | 23 | 13 | 0.263 | 0 | 22 | 1 |
| pcre2 | steady | 23 | 13 | 1.810 | 16 | 4 | 3 |
| pcre2 | compile | 23 | 13 | 0.064 | 0 | 23 | 0 |
| pcre2 | first_use | 23 | 13 | 0.098 | 0 | 23 | 0 |
| fancy_regex | steady | 19 | 11 | 2.996 | 19 | 0 | 0 |
| fancy_regex | compile | 19 | 11 | 1.464 | 11 | 6 | 2 |
| fancy_regex | first_use | 19 | 11 | 1.509 | 12 | 5 | 2 |
| fancy_regex_seek | steady | 19 | 11 | 2.967 | 19 | 0 | 0 |
| fancy_regex_seek | compile | 19 | 11 | 1.680 | 12 | 5 | 2 |
| fancy_regex_seek | first_use | 19 | 11 | 1.779 | 12 | 4 | 3 |
| regex | steady | 11 | 6 | 1.473 | 10 | 1 | 0 |
| regex | compile | 11 | 6 | 1.733 | 7 | 4 | 0 |
| regex | first_use | 11 | 6 | 1.800 | 7 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Memory was not measured in this run.
