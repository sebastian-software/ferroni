# C++ scanner replay

This fixture isolates Ferroni's scanner cost from the C++ highlighting gap
measured in [Ferriki PR #169](https://github.com/sebastian-software/ferriki/pull/169).
It replays one actual native HTML invocation on the large TIOBE C++ fixture:
14,992 UTF-8 bytes, 561 lines, 4,862 calls, 96 distinct scanner instances,
and 1,621 distinct OnigString identities (including capture retokenization).

`trace.json` contains the exact compiled patterns in priority order, subjects,
UTF-16 search positions, find options, matched pattern indices, and every
capture. Subjects with identical content retain separate identities when
Ferriki created separate wrappers. The fixture was captured before TextMate's
post-match filtering. `provenance.json` pins both repositories, source and
asset hashes, and the replay hash. Pattern excerpts retain the upstream MIT
licenses alongside the fixture.

The six group benchmarks were selected by cumulative scanner time in the
instrumented capture. That ranking is diagnostic only: file I/O occurs after
each measured call and perturbs the workload. Confirm costs with the
uninstrumented benchmark before choosing an optimization. Group IDs refer to
scanner instances, not individual regex indices; they remain stable for this
fixture and may change after recapture.

## Measurement boundary

Scanners compile once, before measurement. JSON parsing and full trace
validation also happen before measurement. Every iteration creates fresh
OnigString wrappers outside Criterion's timed closure, then replays the exact
ordered calls through `find_next_match_utf16`, without a caller cache ID.
Scanner state remains reusable across documents as in Ferriki. Fresh string
identities prevent fallback memo hits from an earlier document. Returned
captures are materialized and consumed with `black_box`; result destruction
is part of scanner replay. There is no theme lookup, token assembly, rendering,
Node bridge, trace instrumentation, or runtime engine modification.

The document case preserves the complete call order. Group cases retain that
scanner's own ordered calls, allowing concentrated profiles without unrelated
scanner work. Their times are independent experiments, not additive shares of
the full document. This is a fixed-call replay: an engine behavior change fails
validation rather than changing the tokenizer's future call sequence.

All captured results are checked before each run. Enabling `ffi` additionally
checks every call against independent C Oniguruma searches, selecting the
earliest match with pattern-order tie breaking and comparing all captures.
The oracle converts UTF-16 positions to UTF-8 and C captures back to UTF-16.
The C source revision is pinned in `benches/battle_inputs.toml`.

```sh
# Correctness check, including C (requires the pinned source checkout).
./scripts/prepare-oniguruma-sources.sh
cargo bench --locked --features ffi --bench cpp_scanner_bench -- --test

# Capture a statistical baseline with the ordinary benchmark build.
cargo bench --locked --bench cpp_scanner_bench -- --save-baseline cpp-before
# After one separately committed engine change:
cargo bench --locked --bench cpp_scanner_bench -- --baseline cpp-before
# Focus one group:
cargo bench --locked --bench cpp_scanner_bench -- group_78
```

Keep compiler, build flags, corpus, and hardware fixed. Repeat both revisions
in alternating order and inspect the full Ferriki corpus before accepting an
engine optimization. These cases locate work; they do not measure public
highlighting throughput or establish a general speedup.

## CPU profiling

The bounded example uses the same fixture, validation, and scanner boundary.
It warms five complete replays, prints a ready marker with its PID, then runs
for the requested duration. Its JSON retains every scanner-only wall time.

```sh
# macOS: build with symbols and retain timings, build receipt, and CPU sample.
python3 scripts/profile-cpp-scanner.py /tmp/cpp-document --sample
python3 scripts/profile-cpp-scanner.py /tmp/cpp-group-78 --sample --group 78

# Other platforms: attach perf/samply after the ready marker, or record the
# warmed example. Compilation and validation precede the marker.
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=none \
  cargo build --locked --release --example profile_cpp_scanner
./target/release/examples/profile_cpp_scanner 20 78
```

The helper records compiler, flags, platform, source/fixture/lockfile/binary
hashes, and git status. Sampling requires permission to inspect the launched
process. A CPU sample includes string construction and loop bookkeeping;
individual JSON timings and Criterion exclude string construction. Symbol
builds are diagnostic: compare production timings with the ordinary bench
profile. Do not compare builds with different LTO/codegen settings.

## Recapturing

Capture instrumentation is kept only in `capture.patch` and on the separate
`codex/cpp-scanner-trace` diagnostic branch. It is never compiled into this
benchmark's engine. To reproduce from the pinned engine commit:

```sh
git worktree add -b codex/cpp-recapture /tmp/ferroni-cpp-recapture de32096
git -C /tmp/ferroni-cpp-recapture apply "$PWD/benches/cpp_scanner/capture.patch"
git -C /tmp/ferroni-cpp-recapture add Cargo.toml src/scanner.rs
git -C /tmp/ferroni-cpp-recapture commit -m 'chore(bench): instrument scanner capture'
# In the pinned Ferriki checkout's node directory:
FERRIKI_FERRONI_PATH=/tmp/ferroni-cpp-recapture node ferriki/scripts/build-native.mjs
# Back in this Ferroni checkout:
node benches/cpp_scanner/capture.mjs /path/to/pinned/ferriki /tmp/cpp-trace.jsonl
python3 benches/cpp_scanner/compact.py /tmp/cpp-trace.jsonl /tmp/cpp-trace.json
```

The raw JSONL duplicates patterns per call and is intentionally not retained
(about 179 MB). The compact fixture is about 2.3 MB and preserves every call.
Compaction removes only duplicate storage; it never deduplicates calls or
subject identities. Restore Ferriki's Cargo.lock after the local patched
build, rebuild against ordinary Ferroni, and keep instrumentation separate
from optimization measurements. On recapture, update provenance, licenses,
and group selections together, then repeat full Rust/C validation.
