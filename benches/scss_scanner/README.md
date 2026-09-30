# SCSS scanner replay

This fixture isolates the SCSS regex cost found in the curated 20-format
Ferriki comparison. It contains 3,744 ordered scanner calls from one native
HTML invocation on a 13,696-byte document (16 copies of the original example),
20 scanner instances, and 720 OnigString identities. Every matched pattern
index and capture is retained. `provenance.json` pins the source, grammar,
capture branch, repositories, and hashes. Capture output has exact HTML parity
with Shiki WASM. Pattern excerpts retain the upstream MIT license.

The replay shares the existing C++ fixture loader and independent C oracle.
Setup and validation precede measurement. Each replay creates fresh string
identities outside the timed closure, preventing cross-document memo hits;
scanner instances remain reusable. Returned captures are materialized and
consumed. No highlighting, rendering, Node bridge, or capture instrumentation
belongs to the timing boundary. Group IDs are local to this captured fixture.
The six groups were selected by diagnostic capture timings, whose file I/O
perturbs the workload; confirm them with the uninstrumented Criterion cases.

```sh
./scripts/prepare-oniguruma-sources.sh
cargo bench --locked --features ffi --bench scss_scanner_bench -- --test
cargo bench --locked --bench scss_scanner_bench -- --save-baseline scss-before
# After a separately committed compiler experiment:
cargo bench --locked --bench scss_scanner_bench -- --baseline scss-before
```

## Per-pattern diagnosis and cross-reference

The bounded helper searches each pattern independently over every subject/start
used by a selected group, including captures. Enabling `ffi` validates each
individual result and all byte capture bounds against pinned C Oniguruma.
It warms once and retains every measured round. This does not reproduce
RegSet dispatch, memo hits, or the winning pattern's range restriction; these
times locate candidates and cannot be summed into actual scanner shares.

```sh
cargo build --locked --release --features ffi --example profile_scanner_patterns
./target/release/examples/profile_scanner_patterns benches/scss_scanner/trace.json 8
./target/release/examples/profile_scanner_patterns benches/cpp_scanner/trace.json 78
```

`cross-reference.mjs` inspects the pinned oniguruma-to-es 4.3.4 translation and
the separate optional oniguruma-parser 0.12.1 optimizer. Shiki does not enable
that optimizer. It records the full translated patterns, flags, locations,
unsupported cases, and optional optimized Oniguruma strings. The latter are
diagnostic inputs, never edits to grammars or Ferroni runtime code. They need
independent capture/semantic validation before any compiler implementation.

```sh
node benches/scss_scanner/cross-reference.mjs /path/to/pinned/ferriki \
  benches/scss_scanner/trace.json /tmp/scss-reference.json
```

## Recapturing

Use the separate `codex/scss-scanner-capture` diagnostic branch, or apply
`../cpp_scanner/capture.patch` to the pinned Ferroni commit in a new worktree.
Build the isolated Ferriki checkout with `FERRIKI_FERRONI_PATH` pointing at
that diagnostic tree. The capture helper scopes `FERRONI_CPP_TRACE` to one
SCSS HTML call; the historical environment name is shared with C++ capture.

```sh
node benches/scss_scanner/capture.mjs /path/to/pinned/ferriki /tmp/scss.jsonl
python3 benches/cpp_scanner/compact.py /tmp/scss.jsonl /tmp/scss-trace.json
```

Restore the ordinary addon and Cargo.lock before comparative measurements.
Update hashes and group selections together, then validate all calls against
both Ferroni and pinned C. No runtime optimization is included in this fixture.
