# Published source-backed regex results

Measured revision: `1c8d45f57677ddd9f52bd45c75297c113ceb9bd6`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38808376508).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Standard searches can reuse cached match workspace; parameterized searches currently create fresh workspace. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.845 | 17 | 0 | 4 |
| rust_erroraware | compile | 21 | 12 | 1.000 | 0 | 0 | 21 |
| rust_erroraware | first_use | 21 | 12 | 0.950 | 1 | 7 | 13 |
| c | steady | 21 | 12 | 3.441 | 21 | 0 | 0 |
| c | compile | 21 | 12 | 0.385 | 5 | 16 | 0 |
| c | first_use | 21 | 12 | 0.404 | 5 | 16 | 0 |
| onigmo | steady | 20 | 12 | 3.916 | 20 | 0 | 0 |
| onigmo | compile | 20 | 12 | 0.526 | 6 | 14 | 0 |
| onigmo | first_use | 20 | 12 | 0.510 | 6 | 14 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.704 | 4 | 16 | 1 |
| pcre2_jit | compile | 21 | 12 | 0.257 | 0 | 21 | 0 |
| pcre2_jit | first_use | 21 | 12 | 0.288 | 0 | 21 | 0 |
| pcre2 | steady | 21 | 12 | 2.073 | 18 | 3 | 0 |
| pcre2 | compile | 21 | 12 | 0.082 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.095 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 3.102 | 15 | 0 | 2 |
| fancy_regex | compile | 17 | 10 | 1.874 | 12 | 4 | 1 |
| fancy_regex | first_use | 17 | 10 | 1.907 | 12 | 4 | 1 |
| fancy_regex_seek | steady | 17 | 10 | 3.060 | 15 | 0 | 2 |
| fancy_regex_seek | compile | 17 | 10 | 2.076 | 12 | 5 | 0 |
| fancy_regex_seek | first_use | 17 | 10 | 2.117 | 12 | 5 | 0 |
| regex | steady | 10 | 5 | 1.364 | 5 | 3 | 2 |
| regex | compile | 10 | 5 | 1.776 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.801 | 6 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.801 | 13 | 0 | 8 |
| rust_erroraware | compile | 21 | 12 | 0.984 | 1 | 0 | 20 |
| rust_erroraware | first_use | 21 | 12 | 0.935 | 0 | 3 | 18 |
| c | steady | 21 | 12 | 2.750 | 16 | 0 | 5 |
| c | compile | 21 | 12 | 0.319 | 4 | 17 | 0 |
| c | first_use | 21 | 12 | 0.341 | 4 | 17 | 0 |
| onigmo | steady | 20 | 12 | 3.613 | 18 | 0 | 2 |
| onigmo | compile | 20 | 12 | 0.421 | 5 | 15 | 0 |
| onigmo | first_use | 20 | 12 | 0.422 | 5 | 15 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.625 | 4 | 14 | 3 |
| pcre2_jit | compile | 21 | 12 | 0.242 | 0 | 20 | 1 |
| pcre2_jit | first_use | 21 | 12 | 0.269 | 0 | 20 | 1 |
| pcre2 | steady | 21 | 12 | 1.853 | 16 | 2 | 3 |
| pcre2 | compile | 21 | 12 | 0.064 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.099 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 2.911 | 14 | 0 | 3 |
| fancy_regex | compile | 17 | 10 | 1.363 | 8 | 4 | 5 |
| fancy_regex | first_use | 17 | 10 | 1.414 | 9 | 3 | 5 |
| fancy_regex_seek | steady | 17 | 10 | 2.983 | 15 | 0 | 2 |
| fancy_regex_seek | compile | 17 | 10 | 1.555 | 10 | 5 | 2 |
| fancy_regex_seek | first_use | 17 | 10 | 1.571 | 9 | 5 | 3 |
| regex | steady | 10 | 5 | 1.278 | 7 | 1 | 2 |
| regex | compile | 10 | 5 | 1.665 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.645 | 5 | 4 | 1 |

The exact common cases and all absolute timings are available in `publication.json`.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Memory was not measured in this run.
