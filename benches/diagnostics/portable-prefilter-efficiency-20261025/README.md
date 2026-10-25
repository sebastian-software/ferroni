# Portable prefilter efficiency: issue #465

This diagnostic compares the unchanged [Ferriki replay and inputs](https://github.com/sebastian-software/ferriki/tree/5e12c5edab14d2d49d2d90f59e93a70008bbb46f/docs/benchmarks/precompiled-patterns/detailed-profile-2026-10-25) from [issue #465](https://github.com/sebastian-software/ferroni/issues/465) against Ferroni 2.7.0 (`9e6470df`) and the accompanying fix. Only the replay's Ferroni dependency changes, through a local Cargo patch. The original harness and lockfile otherwise remain unchanged.

`results.json` retains every timed sample, scanner state, and input checksum. Three fresh processes per lane run serially in rotating order. Each validates all 61,490 expected results, warms five passes, and times twenty. Medians below are medians of process medians, in milliseconds per replay. Archive export, loading, subject construction, and validation are outside search timing.

| Lane | Median ms | Process medians ms |
| --- | ---: | --- |
| 2.7.0 Prepared | 228.10 | 234.18, 227.61, 228.10 |
| Fixed Prepared | 15.51 | 15.53, 15.51, 15.36 |
| 2.7.0 Source Eager | 15.45 | 15.45, 15.56, 15.44 |
| 2.7.0 Off | 15.57 | 16.27, 15.57, 15.57 |

Environment: macOS ARM64, Rust 1.99.0, release profile, fat LTO, one codegen unit, no custom target CPU flags. Each baseline Prepared process finishes with 4,289 candidate-cache clears and an active prefilter. Every fixed Prepared process retires the prefilter and reports one loaded group with zero invalid groups.

To repeat, build the pinned replay with its retained lockfile, then build it again with a `[patch.crates-io]` entry pointing `ferroni` to this checkout. Keep separate baseline and candidate executables and use the pinned pattern/query files with the replay's `prepared`, `source`, and `off` modes. The JSON records SHA-256 hashes of those inputs and the accompanying three-file code/test diff.

This is a standalone diagnostic with one `OnigString` identity per captured query, reused across passes. It does not establish full-render performance, startup cost, or performance on other platforms. The complete Ferriki renderer was not rerun.
