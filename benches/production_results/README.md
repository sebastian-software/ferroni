# Published source-backed regex results

Measured revision: `a51a2fd917eb4eacd679d565842b0ec695e3d2e6`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38702442347).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Standard searches can reuse cached match workspace; parameterized searches currently create fresh workspace. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.166 | 5 | 0 | 16 |
| rust_erroraware | compile | 21 | 12 | 1.004 | 1 | 0 | 20 |
| rust_erroraware | first_use | 21 | 12 | 0.975 | 0 | 2 | 19 |
| c | steady | 21 | 12 | 1.799 | 19 | 0 | 2 |
| c | compile | 21 | 12 | 0.399 | 5 | 16 | 0 |
| c | first_use | 21 | 12 | 0.427 | 5 | 16 | 0 |
| onigmo | steady | 20 | 12 | 2.050 | 18 | 1 | 1 |
| onigmo | compile | 20 | 12 | 0.538 | 6 | 14 | 0 |
| onigmo | first_use | 20 | 12 | 0.543 | 6 | 14 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.364 | 1 | 18 | 2 |
| pcre2_jit | compile | 21 | 12 | 0.268 | 0 | 21 | 0 |
| pcre2_jit | first_use | 21 | 12 | 0.309 | 0 | 20 | 1 |
| pcre2 | steady | 21 | 12 | 1.065 | 14 | 6 | 1 |
| pcre2 | compile | 21 | 12 | 0.087 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.103 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 1.461 | 13 | 1 | 3 |
| fancy_regex | compile | 17 | 10 | 2.017 | 12 | 5 | 0 |
| fancy_regex | first_use | 17 | 10 | 2.100 | 12 | 5 | 0 |
| fancy_regex_seek | steady | 17 | 10 | 1.453 | 13 | 2 | 2 |
| fancy_regex_seek | compile | 17 | 10 | 2.212 | 11 | 5 | 1 |
| fancy_regex_seek | first_use | 17 | 10 | 2.304 | 12 | 5 | 0 |
| regex | steady | 10 | 5 | 0.937 | 4 | 3 | 3 |
| regex | compile | 10 | 5 | 1.805 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.889 | 6 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json` and in the website's case tables.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.159 | 8 | 1 | 12 |
| rust_erroraware | compile | 21 | 12 | 0.995 | 1 | 0 | 20 |
| rust_erroraware | first_use | 21 | 12 | 0.971 | 0 | 3 | 18 |
| c | steady | 21 | 12 | 1.450 | 13 | 4 | 4 |
| c | compile | 21 | 12 | 0.341 | 4 | 17 | 0 |
| c | first_use | 21 | 12 | 0.381 | 4 | 17 | 0 |
| onigmo | steady | 20 | 12 | 1.926 | 17 | 2 | 1 |
| onigmo | compile | 20 | 12 | 0.442 | 5 | 15 | 0 |
| onigmo | first_use | 20 | 12 | 0.442 | 5 | 15 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.333 | 1 | 19 | 1 |
| pcre2_jit | compile | 21 | 12 | 0.253 | 0 | 20 | 1 |
| pcre2_jit | first_use | 21 | 12 | 0.278 | 0 | 20 | 1 |
| pcre2 | steady | 21 | 12 | 0.998 | 11 | 8 | 2 |
| pcre2 | compile | 21 | 12 | 0.067 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.102 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 1.403 | 15 | 1 | 1 |
| fancy_regex | compile | 17 | 10 | 1.462 | 9 | 5 | 3 |
| fancy_regex | first_use | 17 | 10 | 1.544 | 10 | 5 | 2 |
| fancy_regex_seek | steady | 17 | 10 | 1.419 | 14 | 1 | 2 |
| fancy_regex_seek | compile | 17 | 10 | 1.647 | 10 | 6 | 1 |
| fancy_regex_seek | first_use | 17 | 10 | 1.722 | 10 | 5 | 2 |
| regex | steady | 10 | 5 | 1.004 | 7 | 2 | 1 |
| regex | compile | 10 | 5 | 1.759 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.847 | 6 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json` and in the website's case tables.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Memory was not measured in this run.
