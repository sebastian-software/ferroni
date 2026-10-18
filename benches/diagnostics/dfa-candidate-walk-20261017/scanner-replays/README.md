# Scanner replay follow-up for #415

I found a repeatable local benefit in a TSX injection scanner, while complete Ferriki gains remain below 1%. The [October 18 current-main homepage guard](../homepage-current-main/README.md) passes the 2% regression limit, and I am keeping the small runtime change. That decision supersedes this investigation's earlier draft recommendation; the original experiment and its fixed decision rule are retained below.

The isolated TSX group 2 replay takes 3.35% less time at the central estimate. Complete TSX scanner replay uses about 8.40% fewer retired instructions in a separate diagnostic. In current Ferriki, however, full native HTML takes only 0.63% less time on the TSX example and 0.15% less time on the large TSX fixture. The large fixture's interval includes a small slowdown.

## Revisions and environment

- Ferroni baseline: `92c2363ed48c99f281805b6cfa4278126588a4c5` (2.2.2).
- Ferroni candidate: `adb31da5001459a10d5456209823dbeb8bff38ae`, the original runtime change in this PR. This follow-up changes evidence only.
- Ferriki consumer: `e7a4fb17e1eb34b45d4ed1bcb8d36d8de4ce85d8`, after Ferriki #279 and #281 were merged.
- Apple M1 Ultra, macOS arm64, rustc 1.99.0, LLVM 23.1.1, Node 24.21.0, pnpm 10.28.2.
- Scanner binaries use the ordinary thin-LTO bench/release profiles. Ferriki addons use its ordinary fat-LTO release profile with one codegen unit. No CPU-specific flags, instrumentation, or alternative allocator are used for latency measurements.
- Each pair is built sequentially at the same source and target paths, with the same compiler, features, and lockfile. Immutable binaries and build receipts are retained locally; their hashes are in the receipts here.

These are local diagnostic results. They do not replace the published Blacksmith comparisons or establish portability to another CPU or toolchain.

## Real scanner traces

The checked-in C++ trace contains 4,862 calls, 96 scanner instances, and 1,621 subject identities. TSX contains 23,088 calls, 48 scanner instances, and 784 identities. The existing hot-group selection is retained. Both replay and Ferriki's tokenizer call `find_next_match_utf16`.

Every timed process first validates all captured matches and captures, cached and uncached scanners, hot groups, and the independent C Oniguruma oracle. Compilation, JSON parsing, validation, and fresh string construction are outside Criterion's timer. Returned scanner results are consumed. Scanner state persists across replays; subject wrappers are fresh.

I fixed eight baseline/candidate process pairs for all eight cases before the first timed process. Engine order alternates, and case order rotates. Each process uses 1 second of warm-up, 3 seconds of measurement, and 50 Criterion samples. All 128 processes and their raw samples are retained. No processes are discarded and no confirmation cohort is added.

Negative changes mean less time. The central estimate is the median paired process-mean ratio. The interval is a deterministic 10,000-resample percentile bootstrap over the eight process pairs, not over individual Criterion samples.

| Case | Runtime change | Paired 95% interval |
| --- | ---: | ---: |
| `cpp_document` | -0.86% | -5.49% to +3.60% |
| `cpp_group_78` | -0.27% | -1.51% to +0.56% |
| `tsx_document` | -1.07% | -1.97% to +0.59% |
| `tsx_group_2` | -3.35% | -3.82% to -2.84% |
| `tsx_group_1` | -2.61% | -3.15% to -1.44% |
| `tsx_group_29` | -1.55% | -2.36% to -1.23% |
| `cpp_document_prefilter_off` | -0.41% | -1.64% to -0.01% |
| `tsx_document_prefilter_off` | -0.49% | -1.57% to +0.93% |

The prefilter-off documents are controls for layout and environmental variation. They also fluctuate: individual TSX control pairs range from -10.59% to +6.25%. Background backup processes were observed during the run. This does not identify the cause of any individual outlier. The full C++ estimate is especially uncertain. The hot-group times are independent experiments and cannot be added to reconstruct document time.

## Current Ferriki confirmation

The confirmed TSX hotspot justified a separate consumer check. Its plan was fixed before the first consumer timing. Both addons contain identical current Ferriki source and the same patched lockfile, differing only in the Ferroni runtime change. Their package snapshots contain independent sidecar copies.

The existing paired native-boundary harness runs eight independent processes, 12 warm-ups, and 60 rotating paired rounds over the two curated TSX fixtures. It checks exact output equality before timing. The boundary includes tokenization and complete native inline HTML rendering; it excludes the unchanged Node facade. The intervals below resample independent worker ratios.

| Case | Runtime change | Paired 95% interval |
| --- | ---: | ---: |
| `curated/tsx/example/html` | -0.63% | -1.31% to -0.21% |
| `curated/tsx/large/html` | -0.15% | -0.31% to +0.63% |

Neither target case reached a 3% reduction. Under this investigation's original fixed decision rule I therefore did not run the conditional homepage regression guard at that stage. The separate [October 18 comparison](../homepage-current-main/README.md) accepts a useful local gain from a small change, reruns the complete guard on current Ferriki, and establishes the 2% bound in that experiment. The earlier homepage experiment remains in the parent report as history.

## Why the hotspot benefit is real but limited

Separate temporary counters are retained as a patch in the archive. They are never compiled into a timed candidate. All captured results, including C parity, pass before the counter run. Counters are reset after five warm replays, then normalized by completed replays.

- Complete TSX visits the changed first-phase limit site about 1.21 million times per replay, compared with about 40,675 for C++. About 97.3% of TSX's walked bytes are in that first phase.
- TSX group 2 walks 454,880 bytes per replay, entirely in the first phase. It never enters the long-pattern admission phase in this diagnostic. Its gain therefore does not depend on the second-phase boundary change.
- The ordinary C++ benchmark's outlined `scan_candidates` function shrinks from 2,688 to 2,296 bytes on this toolchain. The retained ARM assembly shows the boundary consolidation. This is one function, not total binary size or a speed measurement.
- Warm CPU samples put the baseline candidate-search subtree at 13.1% for C++ and 58.9% for TSX. The main C++ regex VM leaf alone accounts for about half its samples. Profiles include driver bookkeeping and string construction and are diagnostic, not latency ratios.

I also measured retired instructions with the ordinary bounded replay driver. Each of two balanced repetitions runs a 1-second process and a 5-second process, subtracts their total instruction counts, and divides by the difference in completed replays. This removes repeated initialization and validation approximately. It includes string construction, timer calls, and timing-vector bookkeeping, which Criterion excludes. The two repetitions give about **-1.03% instructions per C++ replay** and **-8.40% per TSX replay**. These instruction savings do not imply the same runtime reduction, and do not describe Ferriki's fat-LTO addon.

## Validation

- `cargo test --offline --locked --features prefilter-self-check --test scanner_prefilter_regressions --test compat_prefilter_differential`: passed.
- `cargo fmt -- --check`: passed.
- Current Ferriki's mandatory `pnpm run test:ferriki-compat:core`: passed with the candidate dependency.
- Complete `check:bench-curated` and `check:bench-tiobe` output parity gates: passed.
- The addon validated by those gates is byte-identical to the measured candidate.
- Every retained Criterion estimate is checked against its raw JSON. The paired scanner summaries are reproduced exactly, and every archived member is verified against its SHA-256 manifest.

The first consumer gate stopped in dependency preflight: a build-only Cargo patch was not visible to `cargo tree --locked`. I repeated the gate with the same local patch in temporary `.cargo/config.toml`. Both attempts are retained. The patch configuration and original lockfile were restored afterward; no production source or dependency pin changed.

## Artifacts and reproduction

`summary.json` collects all derived results. `plan.json` and `consumer-plan.json` preserve the two predeclared latency plans. `raw-results.tar.gz` contains all Criterion samples and process logs, build receipts/logs, instruction-counter runs, four warmed CPU profiles and their symbols, counter instrumentation and output, assembly, consumer samples, and both validation attempts. Its `RAW_SHA256SUMS` verifies every member. The outer `SHA256SUMS` verifies this report and its artifacts.

The exact run scripts are under `reproduce/`. They retain their original absolute paths and worktree safety assertions. For another checkout, use the pinned revisions above and adapt those paths and assertions to fresh owned worktrees before running them. `profile-cpp-scanner` and Criterion boundaries are documented in `benches/cpp_scanner/README.md`; the TSX fixture is documented in `benches/tsx_scanner/README.md`. The consumer uses the unchanged `node/scripts/bench-native-boundary-paired.mjs` at the pinned Ferriki revision.

The diagnostic patch adds counters and temporary exports only for that diagnostic build. The ordinary binaries are built and copied before instrumentation, and production source is restored before profiling and consumer builds. Do not use counter-build timings as performance evidence.
