# Issue 197: opt-in decimal backtracking optimization

PR branch: `codex/197-backtracking-optimizer`.
Integration base: `f8870de29b55bcf783979fe07652c64fc6206194` (main, Ferroni 1.7.0).
Integration implementation: `fdbc123c955595e312f2b8e051557ee0271defcf`.
Selected lowering: experiment 6, `35c9e57`. Earlier attempts remain
separate local branches and commits; this PR includes the selected lowering,
tests, benchmarks and fresh measurements on current main.

## API and rules

```rust
use ferroni::api::Regex;
use ferroni::scanner::{Scanner, ScannerConfig};

let re = Regex::builder(r"\b(([0-9]+_?)*[0-9]+|0([Xx]\h+|[Oo][0-7]+))\b")
    .optimize_backtracking(true)
    .build()?;
println!("{:?}", re.backtracking_rewrites());

let scanner = Scanner::with_backtracking_optimization(
    &[r"([0-9]+(_?))+(\.)([0-9]+)"],
    &ScannerConfig::default(),
)?;
println!("{:?}", scanner.backtracking_rewrites());
```

Optimization defaults off and is restricted to UTF-8 compilation. Original
lint warnings remain available, and risk rejection still checks the original
pattern. Existing scanner configuration struct literals remain compatible.
Reports list recognized candidates as applied or refused. Unrecognized shapes
have no report; absence of a report does not certify safety.

The first rule recognizes greedy `(?:[0-9]+_?)+` immediately before a
mandatory literal dot. Primitive digit runs use choice-free possessive ASCII
class bytecode. Captures around runs remain intact; captured primitives keep
the general outer-atomic fallback.

The second rule recognizes greedy `(?:[0-9]+_?)*[0-9]+` immediately before a
word boundary. When captures wrap only the entire repeated body, a direct
prefix scan preserves its last successful capture assignment and the original
final run is compiled normally. Other capture placements keep the complete
pair atomic with its internal give-back choices. No captures are removed.

Both rules refuse whole patterns with capture reads, subroutine calls,
look-arounds, scoped options, existing atomic or conditional groups, callouts,
position checks such as `\G`, capture history or exhaustive compile-time
matching modes. The compiler refreshes parsed node references after rewriting.
Proof obligations live next to the rules in `src/backtrack_rewrite/ast.rs`.

## Implementation ownership

`src/backtrack_rewrite.rs` is the public diagnostic facade. Its private AST
pass and crate-private lowering and runtime modules own this optimization:

- `src/backtrack_rewrite/ast.rs`: safety guards, shape recognition, capture
  preservation and atomic fallbacks.
- `src/backtrack_rewrite/lowering.rs`: specialized length calculation and
  bytecode emission, using the same shape checks for both.
- `src/backtrack_rewrite/runtime.rs`: bounded decimal-prefix scanning and
  native capture stack entries; both helpers retain `#[inline(never)]`.

`regcomp` invokes the AST pass and delegates marked group lowering. `regexec`
retains opcode dispatch and the native stack types. Opcode payloads and AST
status bits remain with the engine's shared types. Public APIs and default-off
behavior are unchanged.

The redundant `backtrack_rewrite_bench` from the first experiment has been
removed. `backtrack_compiler_bench` retains its 16-digit failed-tail case along
with the 12- and 20-digit cases. The compiler and PureScript comparison benches
remain reproducible references for source rewrites; they are not production
pattern preprocessors. Earlier attempts and their commits remain in Git.

## Capture and boundary proof

Before the mandatory dot, every shorter decimal exit leaves a digit or
underscore where the dot must match. Repartitioning cannot enable a suffix
success that was not already tried with the first greedy capture assignment.
The whole-tree guard excludes features that could observe discarded choices.

For the decimal tail, every shorter exit lies between word characters, so the
required boundary fails. The first greedy success reserves one digit for the
original final run. The scan computes the same last repeated-body capture
without recording every intermediate assignment. Nested starts are recorded
outer first and ends inner first, with native memory push and restoration
entries. Zero prefix iterations preserve old capture values.

| Input   | Last repeated-body capture | Final run |
| ------- | -------------------------- | --------- |
| `1`     | No new assignment          | `1`       |
| `12`    | `1`                        | `2`       |
| `12_3`  | `12_`                      | `3`       |
| `12_34` | `3`                        | `4`       |

An underscore is crossed only when another digit follows inside the logical
right bound. At most one start/end entry is created per body capture,
independent of segment count. The byte scan adds no heap allocation or unsafe
code. Completed results preserve bounds, priority and captures. Retry, stack
and timeout errors may change because the matcher performs less work.
Unanchored searches can still try multiple starts; no general linear-time
claim is made.

The two prefix helpers remain non-inlined to limit their impact on general VM
dispatch. Extraction fixed the earlier inline variant's roughly 10% cost on an
untouched decimal lane. The separate VM-specialization attempt, `6ecbf56`, did
not establish reliable additional benefit and is not included. A small scanner
tradeoff is accepted where the affected expressions improve substantially.

## Cleanup measurements

The cleanup branch `codex/197-optimizer-cleanup` groups the implementation
without changing the AST or runtime algorithms. Four paired runs compare it
with merged main `2cff3d3701fcbb5303ab2ee90594da61de5391a9` on Apple M1 Ultra,
arm64, Rust 1.96.0, `ffi`, thin LTO. Retained PR-200 baseline binaries have
identical production and timed benchmark sources to merged main. Matching
uses 30 samples, 200 ms warmup and one-second measurement; document controls
use 30 samples, 500 ms warmup and four-second measurement. No build or test
ran concurrently with timing. Each harness validates captures before timing.

The first pair was unstable: even the unchanged baseline V exponent lane
took 86.180 µs, compared with about 51–54 µs in subsequent runs. Its large
differences cannot be attributed to this cleanup alone. All four runs and
their individual 95% confidence intervals remain in
`benches/results/197-optimizer-cleanup.csv`; source and binary hashes,
configuration and order remain in `197-optimizer-cleanup-metadata.json`.
The table summarizes pairs 2–4, including the extra reverse-order pair.

| Workload                                    | Cleanup/main change |
| ------------------------------------------- | ------------------: |
| `PureScript captures/success_long_digits`   |    -1.68% to +0.62% |
| `PureScript captures/success_long_segments` |    -0.48% to +1.21% |
| `PureScript captures/success_short`         |    -2.21% to +1.33% |
| `PureScript compile`                        |    +0.37% to +1.69% |
| `PureScript match/failed_letter_20`         |    -2.49% to -1.84% |
| `PureScript match/success_long_digits`      |    -0.63% to +2.17% |
| `PureScript match/success_long_segments`    |    -0.33% to +0.50% |
| `PureScript match/success_short`            |    -1.82% to +0.73% |
| `V v_exponent/success_long`                 |    -6.92% to -2.92% |
| `V v_exponent/success_long (ordinary)`      |    -3.05% to +0.51% |
| `V v_float/success_long`                    |    -7.95% to -3.45% |
| `V v_float/success_long (ordinary)`         |    -6.79% to -2.17% |
| `Document css_117_document_19_lines_rust`   |    -0.97% to +0.71% |
| `Document rust_81_document_31_lines_rust`   |    -0.42% to +1.57% |
| `Document ts_279_document_28_lines_rust`    |    +0.86% to +1.99% |

No large reproducible regression appears in these measured lanes. The largest
positive matching estimate in pairs 2–4 is +2.17%; document controls reach
+1.99%. These are bounded workload checks, not a claim about every input.

The cleanup passes the full all-feature suite and doctests, the release
comparison over 8,398,075 pattern/input combinations, the Rust 1.94 integration
suite, Clippy, rustdoc, formatting, README, workflow-pin and standards checks.
Coverage is 90.05%, above the unchanged 87% gate. Static test count remains
2,358; no tests or atomic fallbacks were removed.

## Fresh measurements

Platform: Apple M1 Ultra, arm64, Rust 1.96.0, `ffi`, thin LTO, pinned C
Oniguruma `f95747b462de672b6f8dbdeb478245ddf061ca53`. Production and benchmark
SHA256 values accompany the raw estimates and individual 95% confidence
intervals. No other build or test ran concurrently with timing.

`benches/results/197-optimizer-matching.json` records one full 48-lane
PureScript sweep, two additional long-segment matching/capture passes, and
three V long-valid-number passes. Repeated mode order is forward, reverse,
forward. Each lane has 30 samples, 200 ms warmup and a one-second measurement.
Original, compiler and capture-preserving source modes validate every capture
against C before timing. The flattened grammar-specific lane validates bounds
only because it changes captures.

`benches/results/197-optimizer-document-controls.json` compares the unchanged
`battle_bench` source in separate main and PR binaries. Engine order alternates
before/after, after/before, before/after. TypeScript, CSS and Rust documents
validate complete scanner traces against C. Each lane has 30 samples, 500 ms
warmup and a four-second measurement. Optimization is disabled in these
controls, so they measure effects on shared VM execution rather than benefits
of a rewrite.

| Workload                                  |     Ordinary |  Compiler | Compiler/ordinary change |
| ----------------------------------------- | -----------: | --------: | -----------------------: |
| PureScript captures/success_long_digits   |     4.545 µs |  4.270 µs |                   -6.06% |
| PureScript captures/success_long_segments |    47.124 µs |  6.615 µs |       -86.19% to -85.69% |
| PureScript captures/success_short         |     0.403 µs |  0.244 µs |                  -39.43% |
| PureScript compile                        |     8.386 µs |  9.792 µs |                  +16.76% |
| PureScript match/failed_letter_12         |   163.311 µs |  0.365 µs |                  -99.78% |
| PureScript match/failed_letter_20         | 41537.178 µs |  0.492 µs |              -99.998816% |
| PureScript match/failed_underscore_20     | 50694.321 µs |  0.484 µs |              -99.999044% |
| PureScript match/hex_control              |     0.360 µs |  0.301 µs |                  -16.28% |
| PureScript match/success_long_digits      |     4.563 µs |  4.317 µs |                   -5.38% |
| PureScript match/success_long_segments    |    47.807 µs |  6.664 µs |       -86.15% to -84.82% |
| PureScript match/success_short            |     0.390 µs |  0.239 µs |                  -38.67% |
| PureScript scanner                        |     4.737 µs |  4.507 µs |                   -4.85% |
| V v_exponent/success_long                 |    54.338 µs | 52.575 µs |         -3.43% to -2.79% |
| V v_float/success_long                    |    54.173 µs | 52.387 µs |         -5.46% to -2.11% |

For repeated cases the times are medians of the three point estimates and the
change range contains each within-run comparison. Other rows come from the
single full sweep. These are separate workloads, not an overall weighted
speedup. Long segmented PureScript matching and full captures improve by
about 85–86%. Failed 20-digit cases drop from tens of milliseconds to around
0.5 microseconds. Opt-in compilation adds about 1.4 microseconds (+16.76%)
in the single sweep. Captured-primitive fallback placements retain their
general atomic lowering; this is not a claim that every capture layout has
the specialized speedup.

| Unchanged document workload |        Main |          PR | Paired PR/main change |
| --------------------------- | ----------: | ----------: | --------------------: |
| css                         |   91.987 µs |   91.674 µs |    -0.710% to -0.289% |
| rust                        |  105.659 µs |  104.400 µs |    -1.192% to +0.165% |
| ts                          | 1048.124 µs | 1027.770 µs |    -4.390% to -0.264% |

No material regression appears in these three measured document controls.
The largest positive paired estimate is +0.165% on Rust; TypeScript and CSS
are slightly lower in every pair. These results use the current cache-free
base and the native document-tokenization benchmark. Earlier experiment-6
controls used the older base and materialized scanner traces, so their 2–4%
TypeScript cost is not presented as a fresh measurement here. No claim is
made about every possible pattern or input.

## Census and validation

The grammar snapshot contains 260 JSON files, 33,878 pattern occurrences and
22,425 patterns deduplicated within each grammar. Of these, 22,305 compile;
120 have identical pre-existing compilation errors in both modes. Three
patterns are rewritten (two V, one PureScript); one candidate is refused.
All 153 capture comparisons for rewritten patterns agree on the 51-input
corpus. The census does not compare every unmodified pattern on a generated
corpus. Complete diagnostics and the snapshot SHA256 manifest are retained in
`benches/results/197-optimizer-census.json`.

The differential suite checks 25 variants over 8,398,075 pattern/input
combinations, including every string through length seven over
`0`, `1`, `_`, `.`, `x`, `é`. Ordinary Ferroni, optimized Ferroni and pinned C
Oniguruma agree on full capture traces. Checks cover nested and 34-level
captures, captured final runs, fallback placements, stale capture retention,
enclosing alternatives and repeats, bounded/backward/raw searches, invalid
UTF-8, scanner priority and UTF-16 offsets. Long valid PureScript inputs
complete with captures under a 64-entry stack limit.

The full all-feature suite, extended release comparisons, Rust 1.94
integration suite, Clippy and rustdoc pass on this integration. Coverage is
90.08%, above the unchanged 87% gate. `scripts/count-tests.sh` reports 2,358
static test functions, and the native README generator records that count.
Native README, workflow-pin, dependency-policy and standards checks pass.
The removed match-cache feature is not reintroduced.

```sh
RUST_MIN_STACK=268435456 cargo test --locked --all-features -- --test-threads=1
RUST_MIN_STACK=268435456 cargo test --release --locked --features ffi \
  --test backtrack_rewrite -- --ignored --nocapture --test-threads=1
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo bench --locked --features ffi --bench purescript_rewrite_bench
cargo bench --locked --features ffi --bench backtrack_compiler_bench
cargo bench --locked --features ffi --bench battle_bench -- 'scanner_documents/.*_rust$'
```
