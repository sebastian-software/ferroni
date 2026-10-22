# Published source-backed regex results

Measured revision: `817d2b349b88cec0b2c130e416a3caa90220e129`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/39852407433).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Both API paths reuse Ferroni's thread-cached `MatchArg` when they enter the matcher. The steady-use difference below is the historical measurement for this revision; its retained evidence does not profile the cause. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 23 | 13 | 1.812 | 17 | 0 | 6 |
| rust_erroraware | compile | 23 | 13 | 1.005 | 0 | 0 | 23 |
| rust_erroraware | first_use | 23 | 13 | 0.766 | 1 | 16 | 6 |
| c | steady | 23 | 13 | 3.342 | 23 | 0 | 0 |
| c | compile | 23 | 13 | 0.570 | 5 | 18 | 0 |
| c | first_use | 23 | 13 | 0.481 | 5 | 17 | 1 |
| onigmo | steady | 22 | 13 | 3.723 | 22 | 0 | 0 |
| onigmo | compile | 22 | 13 | 0.781 | 6 | 16 | 0 |
| onigmo | first_use | 22 | 13 | 0.609 | 6 | 16 | 0 |
| pcre2_jit | steady | 23 | 13 | 0.694 | 4 | 18 | 1 |
| pcre2_jit | compile | 23 | 13 | 0.424 | 0 | 22 | 1 |
| pcre2_jit | first_use | 23 | 13 | 0.368 | 1 | 21 | 1 |
| pcre2 | steady | 23 | 13 | 1.972 | 20 | 3 | 0 |
| pcre2 | compile | 23 | 13 | 0.135 | 0 | 23 | 0 |
| pcre2 | first_use | 23 | 13 | 0.124 | 0 | 23 | 0 |
| fancy_regex | steady | 19 | 11 | 3.095 | 17 | 0 | 2 |
| fancy_regex | compile | 19 | 11 | 3.092 | 16 | 2 | 1 |
| fancy_regex | first_use | 19 | 11 | 2.403 | 13 | 3 | 3 |
| fancy_regex_seek | steady | 19 | 11 | 3.034 | 17 | 0 | 2 |
| fancy_regex_seek | compile | 19 | 11 | 3.491 | 16 | 3 | 0 |
| fancy_regex_seek | first_use | 19 | 11 | 2.733 | 14 | 4 | 1 |
| regex | steady | 11 | 6 | 1.502 | 7 | 2 | 2 |
| regex | compile | 11 | 6 | 2.671 | 9 | 0 | 2 |
| regex | first_use | 11 | 6 | 2.028 | 7 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 23 | 13 | 1.783 | 11 | 0 | 12 |
| rust_erroraware | compile | 23 | 13 | 0.996 | 2 | 2 | 19 |
| rust_erroraware | first_use | 23 | 13 | 0.741 | 0 | 13 | 10 |
| c | steady | 23 | 13 | 2.535 | 19 | 1 | 3 |
| c | compile | 23 | 13 | 0.476 | 4 | 19 | 0 |
| c | first_use | 23 | 13 | 0.408 | 4 | 19 | 0 |
| onigmo | steady | 22 | 13 | 3.493 | 19 | 0 | 3 |
| onigmo | compile | 22 | 13 | 0.625 | 5 | 17 | 0 |
| onigmo | first_use | 22 | 13 | 0.486 | 5 | 17 | 0 |
| pcre2_jit | steady | 23 | 13 | 0.603 | 2 | 17 | 4 |
| pcre2_jit | compile | 23 | 13 | 0.398 | 0 | 21 | 2 |
| pcre2_jit | first_use | 23 | 13 | 0.357 | 1 | 19 | 3 |
| pcre2 | steady | 23 | 13 | 1.802 | 15 | 2 | 6 |
| pcre2 | compile | 23 | 13 | 0.104 | 0 | 23 | 0 |
| pcre2 | first_use | 23 | 13 | 0.132 | 0 | 22 | 1 |
| fancy_regex | steady | 19 | 11 | 2.894 | 14 | 0 | 5 |
| fancy_regex | compile | 19 | 11 | 2.301 | 15 | 2 | 2 |
| fancy_regex | first_use | 19 | 11 | 1.731 | 12 | 5 | 2 |
| fancy_regex_seek | steady | 19 | 11 | 2.922 | 16 | 0 | 3 |
| fancy_regex_seek | compile | 19 | 11 | 2.708 | 14 | 2 | 3 |
| fancy_regex_seek | first_use | 19 | 11 | 2.056 | 12 | 5 | 2 |
| regex | steady | 11 | 6 | 1.443 | 9 | 0 | 2 |
| regex | compile | 11 | 6 | 2.462 | 9 | 0 | 2 |
| regex | first_use | 11 | 6 | 1.897 | 7 | 2 | 2 |

The exact common cases and all absolute timings are available in `publication.json`.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Memory was not measured in this run.
