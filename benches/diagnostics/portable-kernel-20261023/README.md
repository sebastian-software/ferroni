# Optimized fork-free portable Eager kernel

The optimized kernel cuts warm search time by 17.6–56.7% against the frozen original portable prototype. Load remains 27.1–78.9% faster than ordinary Eager, with byte-identical packages. Warm search changes versus Eager range from +1.2% to +7.8% (median paired changes across ten corpora). A regression of at least 3% in all three repetitions remains for 4 corpora versus Eager (java, rust, c, php) and 6 versus Snapshot (java, json, rust, astro, c, php). This remains an opt-in prototype, not a default-on release decision.

The [five-way comparison](comparison.md) covers load, warm search, the first document, compressed sizes, fresh-process RSS, and internal memory counters. [measurements.md](measurements.md) retains all paired ranges versus Eager and the feature control. [summary.json](summary.json) contains raw mean estimates and all reference pairs; [evidence.tar.gz](evidence.tar.gz) binds source, build provenance, profiles, ablations, validation, and raw Criterion samples.

## Cause and selected changes

The original TSX and HTML sampling profiles put 53.7% and 42.7% of leaf samples in the portable candidate walk. Hashing, state interning, and allocator paths were also visible on warm searches. This is execution overhead: each start rebuilt a vector and hash key even after warming, each cached transition followed separately allocated state storage, and the byte loop mixed the initial bounded walk with later candidate admission. This was not compilation work accidentally included in the warm timer.

The [owned runtime](../../../src/dfa_prefilter/portable.rs) now caches ten start states per machine (anchored/unanchored × five byte contexts), stores transition rows in one flat buffer with precomputed row offsets, and splits the bounded walk from candidate admission. Match recording stays outside the common byte loop. Cache clearing invalidates the start table before row IDs can be reused; row padding is included in the bounded cache budget. No unsafe code or dependency forks were introduced.

[ablation-summary.json](ablation-summary.json) retains the stepwise screen of starts, flat rows, and the split walk. These are rotating, three-process, document-replay screens, not the final Criterion matrix. They establish the large sequential contribution of each change; percentages are not additive. Context lookup tables, boxed caches, bulk self-loop skipping, and an outlined start miss were also screened and rejected because results were small or mixed. Their raw screens are in the evidence archive.

## Unchanged loading contract

The program archives, Snapshot images, and portable `FPRT001` images match the previous experiment byte-for-byte for every corpus. The producer and consumer still use public APIs with registry regex-automata 0.4.18 and aho-corasick 1.1.5. Cargo metadata, the transformed manifest/lock, source hashes, and all executable hashes are retained. The Snapshot and original portable executables are reused unchanged, and were rerun in this matrix rather than compared to historical timing numbers.

Only the native execution cache changed. The format still stores scanner-group seek instructions and forward/reverse entry points. Per-program literal helpers and plans still use normal Eager preparation; the older Snapshot includes those helpers too. Mutable transition caches warm during execution, as with normal Eager. Off/Auto and invalid-image fallback retain their existing behavior. Ferroni's existing VM still determines final matches and captures.

## Measurement contract and limits

- Ten captured corpora: C++, SCSS, Java, JSON, Rust, HTML, Astro, TSX, C, and PHP; all 75,501 UTF-16 calls, winning patterns, and capture arrays agree in every lane before timing. Fresh benchmark processes validate again.
- Five variants × ten corpora × three rotating process repetitions. Criterion uses 40 samples, 300 ms warmup, a 1 s measurement target, and 10,000 bootstrap resamples. Each ratio pairs the same repetition. Ranges are observed process ranges, not confidence intervals or universal bounds.
- Load includes in-memory structural validation, decoding, helpers, scanners, and initial scratch. It excludes file I/O, codec work, network transfer, strings, and teardown. Warm searches follow three document passes and create fresh string identities outside the clock. First document includes load and the first complete replay.
- gzip-9 is recalculated and round-trip checked. Brotli-11 sizes are reused only after payload SHA-256 identity checks against the previous compression run; historical codec timing fields are labeled as such. No decompression or CDN transfer claim follows from the Rust load numbers.
- RSS uses one fresh driver child per memory process, retaining fixture parsing, source images, decoded programs, scanners, construction high-water marks, and four document passes. Internal counters are partial; cache-clear counts cover the overlapping candidate walk, not the separate forward/reverse caches. Packages and decoded arrays are both retained.
- Local M1 Ultra Mac Studio, 64 GB, macOS arm64, rustc 1.99.0, release/bench ThinLTO. Time Machine was idle before the matrix. No compilation or compression ran beside the main timings. Core placement, thermals, and unrelated background activity were not controlled.

Only C remains at least 3% slower in every repetition against the same-binary Eager control (+4.1–9.1%). This suggests build or layout effects explain part of the other differences, but does not remove the measured gap against the leaner Eager build. C is still a concrete execution-kernel optimization target. SCSS changes versus Eager span −5.1% to +11.8%; the Snapshot HTML lane also has one slow repetition, producing a −35.4% to +3.4% search range. These cases do not establish exact warm-search parity.

The remaining single-digit differences must be assessed with their ranges and feature-control effects. This experiment does not establish cross-platform behavior, complete grammar coverage, actual Ferriki startup, authenticated distribution, or adversarial resource behavior. No CI run or stable archive/API promise is implied.

## Validation and reproduction

The isolated registry source passes:

```bash
cargo test --offline --release --features portable-eager-prototype,prefilter-self-check --lib --test scanner_group_prototype --test compat_prefilter_differential --test precompiled_scanner --test scanner_prefilter_regressions -- --test-threads=1
cargo clippy --offline --all-targets --features portable-eager-prototype -- -D warnings
```

The source checkout passes `cargo fmt --all --check`, `git diff --check`, and `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings`. The root all-features check retains the historical Snapshot dependency extensions only for that comparator. Test output and `scripts/count-tests.sh` output are in the evidence; the latter counts the source tree, including feature-gated tests, rather than only this selected run.

Differential coverage includes unseen inputs, arbitrary start/end spans, ASCII assertions, full candidate sets, early/late admission, long loops, stamp rollover, and deliberately tiny caches that force state-ID reuse. Archive tests cover truncation, round-trip re-export, captures/order/duplicates, fallback, and mode handling.

With the preceding caches retained:

```bash
python3 scripts/prepare-portable-eager-comparison.py .cache/portable-kernel-20261023 --snapshot-cache .cache/prepared-eager-20261023 --portable-reference-cache .cache/portable-eager-20261023
# Export each corpus through bin/portable prepare-portable CORPUS ARTIFACT_DIR.
python3 scripts/compare-scanner-group-archives.py .cache/portable-kernel-20261023/run-01 --cache .cache/portable-kernel-20261023 --prebuilt --reuse-artifacts --prepare-only --variants eager prepared before portable control
python3 scripts/compare-scanner-group-archives.py .cache/portable-kernel-20261023/run-01 --cache .cache/portable-kernel-20261023 --measure-only --repetitions 3
python3 scripts/report-portable-kernel.py .cache/portable-kernel-20261023/run-01 .cache/portable-eager-20261023/brotli.json benches/diagnostics/portable-kernel-20261023
```

Use a fresh run directory. `--refresh` explicitly updates the selected isolated build directory. Base commit and exact source overrides are recorded in the evidence. Previous reports remain intact.
