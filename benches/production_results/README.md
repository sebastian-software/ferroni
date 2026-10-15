# Published source-backed regex results

Measured revision: `80ff3b4b73cbcdd02407b34a8422eec1565061ee`. [Blacksmith run](https://github.com/sebastian-software/ferroni/actions/runs/38772515949).

These timings describe that measured revision. Later engine changes are not included.

The complete evidence for each platform is retained in `evidence.tar.gz`, including raw Criterion samples, logs, corpus, dependency lock, native source snapshots and execution order.

The Ferroni baseline (`rust`) uses standard public Regex methods. `rust_erroraware` uses fallible *_with searches/iteration with SearchOptions::new(); both profiles use standard replacement because no fallible replacement API is available. Standard searches can reuse cached match workspace; parameterized searches currently create fresh workspace. Both profiles pass the source oracles, with fallible preflight checks also guarding the baseline.

Ratios are comparator / Ferroni standard API: above 1 favors the standard API. Each row is a separate comparison over its listed common production cases. Families have equal weight. Controlled literal probes are excluded from these aggregates and remain in the per-case data. Rows with different case pools do not form a leaderboard.

Times use the median of the process mean estimates; ratios use the median of the ratios paired by repetition. Compile concerns one pattern, first use its first fixture, and steady reuse the entire fixture batch. A run interval envelope is a conservative descriptive check, not a cross-process significance test.

## macos-arm64

Host: macOS-26.3-arm64-arm-64bit-Mach-O; 6 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.816 | 12 | 0 | 9 |
| rust_erroraware | compile | 21 | 12 | 1.001 | 1 | 0 | 20 |
| rust_erroraware | first_use | 21 | 12 | 0.950 | 0 | 7 | 14 |
| c | steady | 21 | 12 | 3.289 | 21 | 0 | 0 |
| c | compile | 21 | 12 | 0.372 | 5 | 16 | 0 |
| c | first_use | 21 | 12 | 0.397 | 5 | 16 | 0 |
| onigmo | steady | 20 | 12 | 3.794 | 19 | 0 | 1 |
| onigmo | compile | 20 | 12 | 0.519 | 6 | 14 | 0 |
| onigmo | first_use | 20 | 12 | 0.517 | 6 | 14 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.677 | 4 | 16 | 1 |
| pcre2_jit | compile | 21 | 12 | 0.254 | 0 | 21 | 0 |
| pcre2_jit | first_use | 21 | 12 | 0.291 | 0 | 20 | 1 |
| pcre2 | steady | 21 | 12 | 1.971 | 16 | 3 | 2 |
| pcre2 | compile | 21 | 12 | 0.082 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.095 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 2.960 | 14 | 0 | 3 |
| fancy_regex | compile | 17 | 10 | 1.876 | 10 | 5 | 2 |
| fancy_regex | first_use | 17 | 10 | 1.898 | 12 | 5 | 0 |
| fancy_regex_seek | steady | 17 | 10 | 2.936 | 15 | 0 | 2 |
| fancy_regex_seek | compile | 17 | 10 | 2.057 | 12 | 5 | 0 |
| fancy_regex_seek | first_use | 17 | 10 | 2.080 | 12 | 5 | 0 |
| regex | steady | 10 | 5 | 1.313 | 6 | 3 | 1 |
| regex | compile | 10 | 5 | 1.766 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.766 | 6 | 4 | 0 |

The exact common cases and all absolute timings are available in `publication.json`.

## linux-x86-64

Host: Linux-6.6.141-x86_64-with-glibc2.39; 4 visible CPUs.

| Comparator profile | Phase | Common cases | Families | Family-weighted ratio | Standard API direction | Comparator direction | Inconclusive |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rust_erroraware | steady | 21 | 12 | 1.810 | 9 | 0 | 12 |
| rust_erroraware | compile | 21 | 12 | 1.009 | 0 | 0 | 21 |
| rust_erroraware | first_use | 21 | 12 | 0.929 | 0 | 4 | 17 |
| c | steady | 21 | 12 | 2.688 | 14 | 0 | 7 |
| c | compile | 21 | 12 | 0.331 | 4 | 17 | 0 |
| c | first_use | 21 | 12 | 0.345 | 4 | 17 | 0 |
| onigmo | steady | 20 | 12 | 3.547 | 17 | 1 | 2 |
| onigmo | compile | 20 | 12 | 0.411 | 5 | 15 | 0 |
| onigmo | first_use | 20 | 12 | 0.407 | 5 | 15 | 0 |
| pcre2_jit | steady | 21 | 12 | 0.620 | 4 | 15 | 2 |
| pcre2_jit | compile | 21 | 12 | 0.245 | 0 | 21 | 0 |
| pcre2_jit | first_use | 21 | 12 | 0.249 | 0 | 20 | 1 |
| pcre2 | steady | 21 | 12 | 1.816 | 16 | 3 | 2 |
| pcre2 | compile | 21 | 12 | 0.063 | 0 | 21 | 0 |
| pcre2 | first_use | 21 | 12 | 0.095 | 0 | 21 | 0 |
| fancy_regex | steady | 17 | 10 | 2.950 | 15 | 0 | 2 |
| fancy_regex | compile | 17 | 10 | 1.388 | 8 | 6 | 3 |
| fancy_regex | first_use | 17 | 10 | 1.441 | 8 | 5 | 4 |
| fancy_regex_seek | steady | 17 | 10 | 3.007 | 15 | 0 | 2 |
| fancy_regex_seek | compile | 17 | 10 | 1.551 | 9 | 5 | 3 |
| fancy_regex_seek | first_use | 17 | 10 | 1.597 | 9 | 5 | 3 |
| regex | steady | 10 | 5 | 1.364 | 8 | 1 | 1 |
| regex | compile | 10 | 5 | 1.669 | 6 | 4 | 0 |
| regex | first_use | 10 | 5 | 1.712 | 5 | 3 | 2 |

The exact common cases and all absolute timings are available in `publication.json`.

## Regenerate and check

```sh
python3 scripts/publish-production.py --check
```

Host power and thermal state were unavailable. Each of the three process repetitions ran on its own VM per platform; they are not a model of production traffic. Memory was not measured in this run.
