# RegSet failed leading-run skips

The structural search change reduces Java **finished inline HTML time by
17.6% / 23.0%** on the large curated fixture, and **15.4% / 22.8%** on the
small example. Java meets **Ferriki HTML < min(Shiki WASM HTML, Shiki JS HTML)**
in both sizes and both runs: it takes 12.7% / 13.3% less time than the faster
Shiki backend on the large fixture. The full matrix improves from 14/20 to
**15/20 formats** in every size/run combination. CSS, TOML, YAML, JSON and
SCSS remain above the target.

The native Java document replay takes **35.5% / 36.7% less time**. This change
avoids repeated failed VM attempts rather than changing grammar expressions,
opcodes or C optimizer choices. The ordinary addon files have equal size
(2,157,936 bytes). These measurements are local to one interactive desktop.

## Search strategy and guards

After a failed attempt for a leading class run, a later start inside that run
tries a subset of the same tail positions. RegSet's unbounded position-lead
fallback can reuse the existing individual-search skip proof and helper.
A complete positive assertion beginning with an optional word and a star class
can use this plan only if the class contains every ASCII word byte. Its scan
stops before non-ASCII bytes. The expressions remain unchanged, including:

```regex
(?=\w?[-\w\s]*\b(?:class|(?<!@)interface|enum)\s+[$\w]+)
(?=\w?[\w\s]*\brecord\s+[$\w]+)
```

Restored starts followed by a consuming suffix do not receive a run plan.
FIND_NOT_EMPTY also disables assertion skips: a successful assertion can be
rejected after its cut without proving later attempts fail. With a per-match
retry limit, an optional-prefix assertion skips only after an ASCII word start
that tried both prefix paths. Nested leading marks disable that extension.
Callouts, back references, start-sensitive operations, FIND_LONGEST, search
retry budgets, stack/time limits and malformed-boundary guards retain the
existing path. At most eight scans of the compiled instructions inspect leading
marks; there is no pattern expansion or new dependency/unsafe code.

## Boundary and provenance

- Date: 2026-10-01; Apple M1 Ultra, Darwin 27.0.0, Node 24.21.0, Rust 1.96.0.
- Control: `ca89c466e0cd76c672f73a858ed82e10b63a742d`, adding the diagnostic
  Java replay to merged runtime `0ae25008711cdf1d7217e1775d0d84bff8bbed5d`.
- Candidate runtime: `ba2be54a3caff68c69f128435ecc1ccdf6c3318e`.
  The later evidence commit adds a regression example and documentation, with
  no further runtime changes.
- Ferriki/harness/fixtures/assets/dependencies:
  `0136c99d3aad9aad9eeaa025692124e94420153d` (Shiki 4.4.3, Prism 1.30.0).
- Public addon: ordinary fat LTO, one codegen unit, no profiler flags.
  Build receipts retain source and binary hashes. Cargo.lock is restored after
  the path-patched build, before measurement; the original addon and lockfile
  are restored byte-for-byte after the experiment.
- Public order: control 1, candidate 1, candidate 2, control 2; sequential
  formats, fresh worker per format, rotating engine order, five warmups, at
  least 30 samples, 300 ms shared engine budget. Compilation and parity checks
  occur outside timing; output allocation is included.
- Native replay and construction: ordinary thin LTO, same ABBA order,
  30 samples, one-second warmup, three-second requested measurement. Builds,
  coverage, profiling and other benchmarks do not run concurrently with timing.
- C oracle: Oniguruma `f95747b462de672b6f8dbdeb478245ddf061ca53`.

HTML is the primary product metric; tokens remain diagnostic. These runs
measure **inline HTML only**. HTML with generated CSS in classes mode,
Java uncached construction, and whole-app cold-start latency are unmeasured.
The construction probe includes complete C++/SCSS scanner sets and teardown;
JSON parsing and fixture loading occur outside that boundary.

## All 20 formats

Large documents, milliseconds per document, run 1 / run 2. Negative changes
mean less time. Best Shiki is chosen independently in each candidate run.
All small results, raw samples, output hashes, paired changes and comparator
movement are retained in the compressed reports and `summary.json`.

| Format     | Control HTML (ms) | Candidate HTML (ms) | Paired time change | Best Shiki HTML (ms) | Ferriki / best Shiki |
| ---------- | ----------------: | ------------------: | -----------------: | -------------------: | -------------------: |
| TypeScript |     25.48 / 25.27 |       25.45 / 25.07 |     -0.10 / -0.78% |        41.82 / 41.52 |          0.61 / 0.60 |
| TSX        |     26.35 / 26.01 |       25.73 / 26.06 |      -2.32 / 0.18% |        38.64 / 38.81 |          0.67 / 0.67 |
| Rust       |     14.19 / 13.02 |       12.66 / 12.73 |    -10.80 / -2.20% |        18.77 / 18.82 |          0.67 / 0.68 |
| CSS        |     15.51 / 15.51 |       15.81 / 15.44 |      1.98 / -0.41% |          9.62 / 9.42 |          1.64 / 1.64 |
| HTML       |     25.08 / 25.10 |       24.60 / 24.75 |     -1.90 / -1.40% |        34.04 / 34.10 |          0.72 / 0.73 |
| C++        |     47.92 / 49.81 |       49.83 / 46.94 |      4.00 / -5.76% |        55.63 / 53.67 |          0.90 / 0.87 |
| Swift      |     15.50 / 16.20 |       15.67 / 15.31 |      1.11 / -5.54% |        21.25 / 20.57 |          0.74 / 0.74 |
| Java       |     20.86 / 21.60 |       17.19 / 16.64 |   -17.61 / -22.98% |        19.69 / 19.20 |          0.87 / 0.87 |
| Markdown   |     13.93 / 15.05 |       14.14 / 14.02 |      1.57 / -6.80% |        21.73 / 21.17 |          0.65 / 0.66 |
| TOML       |       5.10 / 5.05 |         5.15 / 4.97 |      1.02 / -1.68% |          2.81 / 2.77 |          1.83 / 1.79 |
| YAML       |       8.11 / 8.35 |         8.26 / 8.00 |      1.87 / -4.21% |          7.08 / 7.04 |          1.17 / 1.14 |
| JSON       |       8.95 / 9.47 |         9.29 / 8.89 |      3.84 / -6.07% |          5.83 / 5.70 |          1.59 / 1.56 |
| Astro      |     23.40 / 24.10 |       24.15 / 23.18 |      3.21 / -3.82% |        46.44 / 45.26 |          0.52 / 0.51 |
| Svelte     |     29.69 / 29.74 |       29.20 / 28.93 |     -1.63 / -2.74% |        59.32 / 58.95 |          0.49 / 0.49 |
| Ruby       |     12.86 / 13.34 |       12.56 / 13.15 |     -2.40 / -1.45% |        30.64 / 31.83 |          0.41 / 0.41 |
| Python     |     14.78 / 14.96 |       14.51 / 14.92 |     -1.82 / -0.26% |        25.76 / 25.82 |          0.56 / 0.58 |
| Vue        |     21.89 / 22.56 |       21.99 / 22.34 |      0.49 / -0.98% |        43.24 / 43.56 |          0.51 / 0.51 |
| MDX        |     22.31 / 22.89 |       22.25 / 22.13 |     -0.28 / -3.30% |        35.17 / 35.02 |          0.63 / 0.63 |
| SCSS       |     14.15 / 14.76 |       13.93 / 13.89 |     -1.60 / -5.87% |        12.17 / 11.95 |          1.14 / 1.16 |
| Bash       |     13.17 / 13.47 |       13.38 / 13.01 |      1.56 / -3.43% |        13.93 / 13.47 |          0.96 / 0.97 |

The largest raw HTML increase is +4.03% (small YAML, pair 1), followed by
+4.00% (large C++, pair 1); the same cases decrease by 3.01% and 5.76% in
pair 2. No new large repeated regression is demonstrated in this corpus.
Small Swift changes relative to best Shiki are +3.00% / +1.69%; its raw
changes are +3.72% / -2.64%. Small differences need further repetition before
attributing them to the engine. The full corpus does not regress in target wins.

## Replay and construction

Mean milliseconds, run 1 / run 2. Criterion retains samples, estimates and
95% confidence intervals; separate pattern timings are not added together.

| Case     | Control mean (ms) | Candidate mean (ms) | Paired time change |
| -------- | ----------------: | ------------------: | -----------------: |
| document |   11.813 / 11.943 |       7.623 / 7.564 |   -35.47 / -36.66% |
| cpp      | 241.092 / 237.754 |   235.131 / 236.840 |     -2.47 / -0.38% |
| scss     |   56.909 / 55.837 |     55.121 / 56.070 |      -3.14 / 0.42% |

## Correctness and quality

All 3,920 captured Java calls, winner indices and full captures match pinned C
Oniguruma. Existing C++ (4,862 calls) and SCSS (3,744 calls) replay checks also
pass. Every full public report has exact token and HTML parity for all three
TextMate engines, all 20 formats and both sizes. Prism preserves source in
its 16 supported formats; Astro, Svelte, Vue and MDX remain explicitly
unsupported. Four strict report comparisons accept all 80 API/case pairs each,
with zero exclusions.

The full all-feature suite passes 2,362 tests and 14 doctests, with two existing
opt-in tests ignored. The tree contains 2,364 test functions, counted by the
repository script. The new RegSet matrix compares **968,436 outcomes** against the same
compiled program with only its leading-run plan disabled, including captures,
competing winners, memoized/plain searches, bounded starts, malformed input,
Unicode, FIND_NOT_EMPTY/FIND_LONGEST, and match/search/stack limits. A consuming
suffix regression checks restored assertion starts independently.

Formatting, all-feature/all-target Clippy, rustdoc, cargo-deny, workflow pins,
README generation and docs format/lint/typecheck/build pass. All 24 core CI
checks pass, including the stable/nightly/MSRV 1.94 platform matrix, fuzzing and
**90.17% line coverage** against the existing 87% gate. Local sandbox retries
for the advisory database, theme fetch and docs preview server are recorded in
`validation/`; these retries use the same checks. CI evidence refers to the
measured runtime commit; later evidence commits trigger another CI run.

## Rejected inlining experiments

`rejected-inlining/` retains independent signed branches, raw reports and
samples. None of their runtime changes are included here.

- `codex/java-word-boundary-inline` at
  `07ffe198b1f32d1617a42f322fd9ce8560bbc20a`: one ordinary inline hint.
  Native document time changes by about -0.3%; Java large HTML changes by
  -3.1% / +3.3%. The completed full ABBA matrix shows no stable benefit and
  stays at 14/20. The experiment is rejected.
- `codex/java-word-boundary-force-inline` at
  `3c385d057234183d7907ebef341fecd0abedec2a`: forced inlining saves about 4.6%
  in the native document replay. The public sequence is intentionally
  incomplete (control 1 and candidate 1 complete, candidate 2 partial, control
  2 absent), so no repeated HTML conclusion is drawn. It is unaccepted.

The Java parser cross-reference in `diagnosis/` finds no optimizer rewrite
for any of the 115 unique patterns. Search-plan dumps retain the motivating
unbounded-distance maps (c/e/i for declarations, r for record). They are
analysis artifacts, not profiler builds used for ordinary timing.

## Reproduction

Build each pinned Ferroni variant from the pinned Ferriki checkout, restoring
Cargo.lock before measuring. Run four sequential public matrices in ABBA order:

```sh
FERRIKI_FERRONI_PATH=/path/to/ferroni node node/ferriki/scripts/build-native.mjs
node node/scripts/bench-tiobe.mjs --corpus curated --write report.json
```

From each Ferroni checkout, with its captured Java replay present:

```sh
FERRONI_ONIGURUMA_DIR=/path/to/pinned/oniguruma cargo bench --locked --features ffi --bench java_scanner_bench -- --test
cargo bench --locked --bench java_scanner_bench -- java_scanner/document --sample-size 30 --warm-up-time 1 --measurement-time 3
cargo bench --locked --bench scanner_compile_bench -- --sample-size 30 --warm-up-time 1 --measurement-time 3
RUST_MIN_STACK=268435456 cargo test --locked --all-features -- --test-threads=1
```

An all-feature build also requires the prepared C oracle sources (see
CONTRIBUTING.md). Recompute the retained summaries without rerunning timings:

```sh
node summarize.mjs /path/to/pinned/ferriki /path/to/this/directory
python3 summarize-native.py
shasum -a 256 -c SHA256SUMS
```
