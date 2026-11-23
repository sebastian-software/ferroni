# Published source-backed regex results

Measured revision: `bd688eea256da1e3255cc5790d8b582913457c27`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/45266137470).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. For this measured revision, `rust_erroraware` is recorded as: Ferroni fallible *_with searches, iteration and replacement with SearchOptions::new(). Both API paths reuse Ferroni's thread-cached `MatchArg` when they enter the matcher. The steady-use difference below is the historical measurement for this revision; its retained evidence does not profile the cause. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 23 | 13 | 1.885 | 17 | 0 | 6 |
| rust_erroraware | compile | 23 | 13 | 1.000 | 0 | 0 | 23 |
| rust_erroraware | first_use | 23 | 13 | 0.767 | 0 | 16 | 7 |
| c | steady | 23 | 13 | 3.475 | 23 | 0 | 0 |
| c | compile | 23 | 13 | 0.566 | 5 | 18 | 0 |
| c | first_use | 23 | 13 | 0.484 | 5 | 18 | 0 |
| onigmo | steady | 22 | 13 | 3.902 | 21 | 0 | 1 |
| onigmo | compile | 22 | 13 | 0.758 | 6 | 15 | 1 |
| onigmo | first_use | 22 | 13 | 0.602 | 6 | 16 | 0 |
| pcre2_jit | steady | 23 | 13 | 0.709 | 4 | 17 | 2 |
| pcre2_jit | compile | 23 | 13 | 0.417 | 0 | 22 | 1 |
| pcre2_jit | first_use | 23 | 13 | 0.369 | 1 | 22 | 0 |
| pcre2 | steady | 23 | 13 | 2.069 | 20 | 3 | 0 |
| pcre2 | compile | 23 | 13 | 0.132 | 0 | 23 | 0 |
| pcre2 | first_use | 23 | 13 | 0.123 | 0 | 23 | 0 |
| fancy_regex | steady | 19 | 11 | 3.201 | 17 | 0 | 2 |
| fancy_regex | compile | 19 | 11 | 3.002 | 15 | 3 | 1 |
| fancy_regex | first_use | 19 | 11 | 2.342 | 12 | 6 | 1 |
| fancy_regex_seek | steady | 19 | 11 | 3.103 | 17 | 0 | 2 |
| fancy_regex_seek | compile | 19 | 11 | 3.390 | 16 | 3 | 0 |
| fancy_regex_seek | first_use | 19 | 11 | 2.711 | 12 | 6 | 1 |
| regex | steady | 11 | 6 | 1.566 | 7 | 2 | 2 |
| regex | compile | 11 | 6 | 2.642 | 9 | 0 | 2 |
| regex | first_use | 11 | 6 | 2.010 | 7 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 23 | 13 | 1.885 | 14 | 0 | 9 |
| rust_erroraware | compile | 23 | 13 | 1.024 | 0 | 0 | 23 |
| rust_erroraware | first_use | 23 | 13 | 0.741 | 0 | 16 | 7 |
| c | steady | 23 | 13 | 2.659 | 19 | 0 | 4 |
| c | compile | 23 | 13 | 0.506 | 4 | 17 | 2 |
| c | first_use | 23 | 13 | 0.430 | 4 | 18 | 1 |
| onigmo | steady | 22 | 13 | 3.612 | 19 | 0 | 3 |
| onigmo | compile | 22 | 13 | 0.662 | 5 | 15 | 2 |
| onigmo | first_use | 22 | 13 | 0.493 | 5 | 17 | 0 |
| pcre2_jit | steady | 23 | 13 | 0.617 | 4 | 17 | 2 |
| pcre2_jit | compile | 23 | 13 | 0.410 | 0 | 21 | 2 |
| pcre2_jit | first_use | 23 | 13 | 0.330 | 1 | 20 | 2 |
| pcre2 | steady | 23 | 13 | 1.913 | 14 | 1 | 8 |
| pcre2 | compile | 23 | 13 | 0.109 | 0 | 23 | 0 |
| pcre2 | first_use | 23 | 13 | 0.126 | 0 | 23 | 0 |
| fancy_regex | steady | 19 | 11 | 3.091 | 17 | 0 | 2 |
| fancy_regex | compile | 19 | 11 | 2.283 | 13 | 2 | 4 |
| fancy_regex | first_use | 19 | 11 | 1.756 | 10 | 7 | 2 |
| fancy_regex_seek | steady | 19 | 11 | 2.966 | 17 | 0 | 2 |
| fancy_regex_seek | compile | 19 | 11 | 2.632 | 13 | 2 | 4 |
| fancy_regex_seek | first_use | 19 | 11 | 1.971 | 11 | 7 | 1 |
| regex | steady | 11 | 6 | 1.352 | 8 | 1 | 2 |
| regex | compile | 11 | 6 | 2.498 | 7 | 0 | 4 |
| regex | first_use | 11 | 6 | 1.813 | 6 | 4 | 1 |

The exact common cases and all absolute timings are available in `publication.json`.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Memory was not measured in this run.
