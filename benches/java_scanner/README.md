# Java scanner replay

This fixture captures 3,920 ordered scanner calls from one native Ferriki
inline HTML invocation on the curated large Java example: 16 copies,
17,856 bytes, 513 lines, 27 scanners and 544 string identities. It retains
pattern order, UTF-16 positions, options, winner indices and every capture.
`provenance.json` pins both repositories, source and grammar hashes, the
capture source, its actual build receipt, and exact Shiki WASM HTML parity.
The upstream pattern excerpts retain their MIT license.

The benchmark reuses the C++ replay loader and independent C oracle. Setup
and validation precede timing. Each replay gets fresh string identities
outside the timed closure; scanners are reused. Returned captures are
materialized and consumed. Highlighting, rendering, Node and instrumentation
are excluded. Group IDs belong to this fixture. The six selected groups came
from diagnostic capture timings, which include instrumentation overhead;
the uninstrumented Criterion cases independently measure their runtime.

```sh
./scripts/prepare-oniguruma-sources.sh
cargo bench --locked --features ffi --bench java_scanner_bench -- --test
cargo bench --locked --bench java_scanner_bench -- --save-baseline java-before
# After a separately committed experiment:
cargo bench --locked --bench java_scanner_bench -- --baseline java-before
```

## Baseline diagnosis

On merged Ferroni `0ae2500`, all 3,920 scanner results and capture ranges
match pinned C Oniguruma `f95747b`. Two ordinary release runs use 30 samples,
one-second warmup and three-second measurement per case. The document means
are 12.15 and 11.97 ms; group 15 is 2.97/2.99 ms, group 19 is 2.88/2.82 ms,
and group 5 is 2.42/2.37 ms. The raw samples and estimates live in `results/`.
These scanner timings are not HTML timings and do not change the six
remaining HTML misses in the curated comparison.

Independent per-pattern searches, with each result also validated against C,
identify two unsuccessful lookaheads in groups 15 and 19:

```regex
(?=\w?[-\w\s]*\b(?:class|(?<!@)interface|enum)\s+[$\w]+)
(?=\w?[\w\s]*\brecord\s+[$\w]+)
```

They are candidates for further diagnosis. Independent searches do not
reproduce RegSet dispatch, memoization or winner range restriction. Their
times cannot be summed into actual scanner shares.

A separate bounded CPU sample with line tables locates 78.3% of document
samples inside `match_at_impl`. Exclusive samples include the VM (52.9%),
`stack_pop` (17.5%) and `is_word_boundary` (11.5%). Group 15 has a similar
shape, with 13.1% exclusive word-boundary samples. The instrumented symbol
build supports attribution only; ordinary Criterion builds support timings.
Raw profiles, summaries and pattern results are retained together.

```sh
cargo build --locked --release --features ffi --example profile_scanner_patterns
./target/release/examples/profile_scanner_patterns benches/java_scanner/trace.json 15 3
# Bounded warm CPU workload; use a separate target directory for symbol builds.
cargo build --locked --release --example profile_cpp_scanner
./target/release/examples/profile_cpp_scanner 20 all benches/java_scanner/trace.json
./target/release/examples/profile_cpp_scanner 20 15 benches/java_scanner/trace.json
```

## Recapturing

Create an isolated worktree at the pinned base commit and apply
`capture.patch`, or use the separately committed diagnostic source
`60bf4a7` on `codex/java-scanner-capture`. Build the pinned Ferriki checkout
with `FERRIKI_FERRONI_PATH` pointing to that tree. The historical
`FERRONI_CPP_TRACE` name is shared by the capture instrumentation; the helper
scopes it to one Java HTML invocation.

```sh
node benches/java_scanner/capture.mjs /path/to/pinned/ferriki /tmp/java.jsonl
python3 benches/cpp_scanner/compact.py /tmp/java.jsonl /tmp/java-trace.json
```

Restore the ordinary addon and Cargo.lock before comparative measurements.
Update all hashes, receipts and group selections together when replacing the
fixture, then validate every result against Ferroni and C. The retained
receipt records the source as staged when it was built; the later signed
capture commit contains that exact source. No runtime optimization or grammar
rewrite is included in this baseline.
