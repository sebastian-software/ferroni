# Post-#204 highlighting baseline and next candidates

The primary performance target is **Ferriki HTML < min(Shiki WASM HTML,
Shiki JS HTML)** for each format. Two sequential, complete curated runs on
merged Ferroni #204 beat that target for **13/20 large HTML fixtures** in both
runs. C++ has the largest absolute HTML gap (22–25 ms per large document),
followed by CSS (6.2–6.4 ms) and JSON (3.5–3.7 ms).

Tokens are retained as a secondary API and diagnostic boundary, not as an
equally weighted product score or the basis for ranking candidates. The
primary product workload is finished HTML; HTML plus generated CSS in classes
mode should be measured separately in the next Ferriki benchmark update.
These retained runs measure **inline HTML only** and predate classes mode.
They establish no result for that new output. No engine behavior changes are
included here.

All 20 formats, both sizes, and all three TextMate engines passed exact token
and HTML parity. Prism preserved the source for its 16 supported formats;
Astro, MDX, Svelte, and Vue are explicitly unsupported. Five strict report
comparisons each accepted all 80 Ferriki API/case pairs, with zero exclusions.

## Fixed measurement boundary

- Ferroni runtime: `abdf2ec905bd05c930b446b3678d763020d480b8` (merged #204).
- Ferriki, harness, dependencies, and assets:
  `0136c99d3aad9aad9eeaa025692124e94420153d` (benchmark PR #173).
  Ferriki main's later asset migration is deliberately excluded from this engine comparison.
- Apple M1 Ultra, 20 cores; Darwin 27.0.0; Node 24.21.0; Rust 1.96.0.
- Ferriki 0.7.0, Shiki 4.4.3, Prism 1.30.0; `github-dark` theme.
- Ordinary public addon: default fat LTO, one codegen unit, no profiling flags.
  Binary SHA-256: `bf52fbcb04a816ffca2d9e9dab7f8cf55f42a08ee11007c7115db10fea477eab`.
- Five warmups, at least 30 rounds, shared 300 ms engine budget, rotating engine
  order, one fresh worker per language, sequential languages. Medians include
  output allocation; loading, compilation, and parity validation are excluded.
- Tables refer to the large fixtures. Both small and large raw samples, parity,
  exact native build receipts, asset hashes, and source revisions remain in
  `public-{1,2}.json.gz`. Fixture byte counts are in `summary.json`.

The complete comparison uses the public HTML and token APIs. The token facade
requests scopes; native profiles below reproduce this with `--scopes`.
Historical `previous-control.json.gz` (runtime `16af64c`) and
`previous-candidate.json.gz` (runtime `1d0df6c`) are copied unchanged from the
#204 evidence. Their comparison confirms retained CSS/SCSS improvements,
but does not isolate host drift between measurement dates. In run 1 the largest
increase against the prior candidate is about 14%; in run 2 it is about 4%.
C++ and its Shiki comparators also move between runs. This is not evidence of
a newly introduced large runtime regression.

## Primary public HTML medians

Values are milliseconds per document, run 1 / run 2. The Shiki column selects
the faster HTML backend separately for each run. Lower is better, and a
Ferriki / best Shiki ratio below 1 is a win. Rows are ordered by the largest
absolute HTML gap against that best backend. This ranking uses the retained
samples; no timing was rerun for the change in prioritization.

| Format | Ferriki HTML | Best Shiki HTML | Ferriki / best Shiki |
| --- | ---: | ---: | ---: |
| C++ | 87.50 / 79.65 | 62.62 / 57.45 (WASM) | 1.40 / 1.39 |
| CSS | 16.56 / 16.14 | 10.20 / 9.89 (JS) | 1.62 / 1.63 |
| JSON | 9.69 / 9.39 | 6.03 / 5.88 (JS) | 1.61 / 1.60 |
| TOML | 5.56 / 5.26 | 3.01 / 2.83 (JS) | 1.85 / 1.86 |
| Java | 23.30 / 22.01 | 21.13 / 20.13 (JS) | 1.10 / 1.09 |
| SCSS | 14.41 / 14.53 | 12.88 / 12.77 (JS) | 1.12 / 1.14 |
| YAML | 9.09 / 8.53 | 7.69 / 7.27 (JS) | 1.18 / 1.17 |
| Bash | 13.58 / 13.75 | 14.09 / 14.28 (JS) | 0.96 / 0.96 |
| Swift | 17.08 / 16.15 | 23.38 / 21.40 (JS) | 0.73 / 0.75 |
| Rust | 13.73 / 13.29 | 20.28 / 19.42 (JS) | 0.68 / 0.68 |
| Markdown | 15.90 / 14.71 | 24.75 / 22.84 (WASM) | 0.64 / 0.64 |
| HTML | 28.06 / 26.12 | 38.28 / 35.49 (WASM) | 0.73 / 0.74 |
| Python | 15.18 / 14.94 | 26.45 / 26.20 (JS) | 0.57 / 0.57 |
| MDX | 23.50 / 22.90 | 36.68 / 36.13 (WASM) | 0.64 / 0.63 |
| TSX | 26.79 / 26.89 | 40.73 / 40.75 (WASM) | 0.66 / 0.66 |
| TypeScript | 26.62 / 25.94 | 43.43 / 42.27 (WASM) | 0.61 / 0.61 |
| Ruby | 13.47 / 13.12 | 32.25 / 31.47 (JS) | 0.42 / 0.42 |
| Vue | 23.04 / 22.55 | 45.19 / 44.06 (WASM) | 0.51 / 0.51 |
| Astro | 25.04 / 24.30 | 48.48 / 46.85 (WASM) | 0.52 / 0.52 |
| Svelte | 31.11 / 29.83 | 62.70 / 58.83 (WASM) | 0.50 / 0.51 |

## Secondary token diagnosis

These API medians are retained for integrations and profiling. They are not a
second product score: Ferriki's token facade also requests grammar-state scope
information, and the profiles distinguish that work from the inline HTML
path. Ferriki beats the best Shiki token backend in 5/20 formats in both runs
(Astro, Python, Ruby, Svelte, and Vue). This does not describe classes-mode HTML
performance. Rows retain the primary HTML ordering.

| Format | Ferriki tokens | Best Shiki tokens |
| --- | ---: | ---: |
| C++ | 151.59 / 139.03 | 58.53 / 52.19 (WASM) |
| CSS | 16.81 / 16.66 | 5.77 / 5.43 (JS) |
| JSON | 15.78 / 15.33 | 3.06 / 3.05 (JS) |
| TOML | 8.10 / 7.88 | 1.25 / 1.20 (JS) |
| Java | 38.90 / 37.50 | 17.97 / 17.39 (JS) |
| SCSS | 22.68 / 22.76 | 9.21 / 9.43 (JS) |
| YAML | 18.36 / 17.56 | 5.20 / 5.00 (JS) |
| Bash | 22.18 / 22.39 | 10.75 / 10.84 (JS) |
| Swift | 26.16 / 23.81 | 19.97 / 18.10 (JS) |
| Rust | 18.30 / 17.92 | 16.07 / 15.55 (JS) |
| Markdown | 23.78 / 22.40 | 20.39 / 18.89 (WASM) |
| HTML | 38.38 / 36.48 | 30.72 / 29.52 (WASM) |
| Python | 21.74 / 22.20 | 22.32 / 23.15 (JS) |
| MDX | 36.20 / 35.23 | 32.11 / 31.41 (WASM) |
| TSX | 46.43 / 45.94 | 35.21 / 34.60 (WASM) |
| TypeScript | 45.61 / 45.18 | 38.77 / 38.15 (WASM) |
| Ruby | 19.88 / 19.60 | 28.64 / 28.13 (JS) |
| Vue | 37.32 / 36.49 | 39.97 / 38.91 (WASM) |
| Astro | 33.09 / 31.80 | 44.52 / 42.22 (WASM) |
| Svelte | 52.05 / 50.85 | 57.34 / 54.87 (WASM) |

## Uncached scanner construction and teardown

`scanner_compile_bench` loads JSON and prepares borrowed pattern lists outside
timing, then constructs and drops every scanner in the captured fixture inside
each iteration. This is **construction plus destruction**, not a whole-app
cold-start or import/download benchmark.

The same benchmark is committed on the old runtime (`45f09aa`, based on
`16af64c`) and merged runtime (`565128a`, based on `abdf2ec`). The old runtime
uses the same SCSS fixture through `FERRONI_SCSS_TRACE`. The control commit is
preserved by tag `benchmarks/post-204-compile-control`.

Both variants use the ordinary Ferroni thin-LTO release profile, 30 Criterion
samples, one-second warmup, and three-second requested measurement. The order
is control 1, current 1, current 2, control 2; these timings run separately from
public measurements and CPU recordings.

| Captured set | Scanners | Pattern occurrences | Control means (ms) | Current means (ms) |
| --- | ---: | ---: | ---: | ---: |
| C++ | 96 | 4,026 | 227.18 / 231.46 | 234.15 / 233.88 |
| SCSS | 20 | 600 | 82.94 / 85.06 | 56.09 / 55.99 |

SCSS construction plus teardown is 32–34% faster. C++ means are approximately
1–3% higher; current run 1 contains outliers (mean 234.15 ms, median 226.29 ms,
95% mean interval 227.00–245.61 ms). The control medians are 226.97 / 231.48 ms
and current medians 226.29 / 233.61 ms. Treat this as a small possible tradeoff
with measurement drift, not a precise compiler regression. Full estimates,
confidence intervals, and samples are retained under `criterion/` and in
`compiler-summary.json`. Fixture hashes and ordering are in
`measurement-contract.json`.

Ordinary pure-scanner replay after #204 gives C++ document 59.56 ms, C++ group
78 batch 37.25 ms, SCSS document 3.70 ms, and SCSS group 8 batch 1.69 ms.
C++ remains close to its prior 60–61 ms document baseline. SCSS remains far
below its pre-#204 25.8–26.1 ms baseline, although 3.70 ms is above the earlier
3.35–3.51 ms candidate measurement. These are replay boundaries, not public
highlighting times. The pinned C oracle passed every call and full capture:
4,862 C++ calls and 3,744 SCSS calls.

## Native CPU diagnosis

These nine profiles use the same Rust source with debug line tables and no
symbol stripping, retaining fat LTO. They record a validated, warmed 20-second
native workload with macOS `sample` for 10 seconds at 1 ms intervals. Sampled
timings are diagnostic and must not be mixed with ordinary benchmark timings.
The original addon is restored after profiling. Each raw sample, workload
receipt, parity result, and profiler log is retained under `profiles/`.

Percentages are inclusive main-thread sample shares. Categories overlap and
must not be added. Scope and binary columns expose the two TextMate token
passes; HTML uses the binary pass only. Profiles cover the native boundary,
not all JavaScript facade work.

| Profile | Ferroni | Match VM | Scope pass | Binary pass | JSON serialization |
| --- | ---: | ---: | ---: | ---: | ---: |
| cpp-native-html | 84.2% | 75.8% | 0.0% | 89.0% | 3.8% |
| css-native-html | 13.0% | 8.1% | 0.0% | 28.8% | 21.8% |
| css-native-tokens | 32.0% | 19.5% | 44.4% | 34.7% | 0.1% |
| java-native-html | 56.8% | 45.4% | 0.0% | 65.8% | 11.4% |
| java-native-tokens | 77.3% | 61.8% | 49.2% | 43.8% | 0.0% |
| scss-native-html | 27.2% | 14.7% | 0.0% | 39.9% | 19.6% |
| scss-native-tokens | 53.9% | 29.8% | 47.3% | 38.3% | 0.0% |
| yaml-native-html | 20.2% | 12.7% | 0.0% | 36.0% | 19.6% |
| yaml-native-tokens | 45.6% | 28.1% | 46.3% | 40.7% | 0.0% |

## Next isolated experiments

1. **C++ compiler optimization inside negative lookahead.** It is the largest
   absolute HTML gap and Ferroni occupies 84% of the native HTML profile, including
   76% in the match VM. Group 78 remains the dominant captured replay batch.
   Its previously identified expensive patterns 19, 100, 101, 102, and 118
   contain pure keyword lists of up to 114 alternatives inside negative
   lookahead. The current `detect_literal_alternations_inner` excludes all
   anchor bodies through `in_anchor`, so these lists never reach trie
   optimization. `lookaround-candidates.json` records the original AST lists
   and hashes; it is a diagnosis, not a semantic or performance proof.
   The next trial should admit only a proven subset of plain literal lists in
   negative lookahead, retaining original patterns. Lookbehind remains a
   separate case: pattern 118 also contains a 171-alternative negative
   lookbehind. Do not remove the anchor guard globally. Validate assertion
   backtracking, captures, branch ordering, case/encoding behavior, and the
   full C-oracle replay before evaluating the full 20-format matrix.
2. **Java capture and replay.** HTML takes 22.0–23.3 ms against JS
   20.1–21.1 ms; Ferroni occupies 57% of the native HTML profile. The
   secondary token profile has 77% Ferroni share. VM,
   backtracking-stack, and word-boundary work dominate. Capture the actual
   ordered scanner calls and full C-validated captures before choosing a
   compiler or VM change. No particular Java pattern has been proven yet.
3. **SCSS and CSS still need work against JS.** HTML ratios are 1.12–1.14x
   for SCSS and 1.62–1.63x for CSS despite the #204 wins. SCSS has 27% native
   Ferroni share in HTML, CSS 13%. Keep engine candidates distinct from Ferriki's
   duplicated scope/binary passes and rendering/serialization work. Ferriki
   issue [#172](https://github.com/sebastian-software/ferriki/issues/172)
   tracks the duplicated tokenization. Even removing all regex time would not
   explain away the complete public CSS gap at the measured boundary.
4. **YAML and simple formats.** YAML HTML has 20% native Ferroni share;
   both TextMate and facade costs matter. JSON and TOML have HTML gaps of
   3.5–3.7 ms and 2.4–2.5 ms, respectively; the earlier pinned Ferriki
   `ferroni-1.7.0` profiles attributed much of their cost outside Ferroni.
   Those JSON/TOML profiles were not repeated in this round. Revisit them
   alongside Ferriki issue #172 rather than inferring regex trouble from a
   public ratio alone.

Each implementation trial should get its own branch and commit. Judge the
finished HTML result against **both** Shiki engines, preserving parity and
checking all 20 formats for new outliers. Retain tokens for correctness and
diagnosis. Compare classes-mode outputs only with equivalent output
requirements; scope trees and shared multi-theme CSS are different workloads
from ordinary inline HTML. This report and the construction benchmark change no regex
behavior.

## Reproduction

Use clean checkouts at the pins above. Install dependencies from the pinned
Ferriki lockfile using the repository's documented tooling. Build the ordinary
native addon against the selected Ferroni checkout, then run twice from the
Ferriki root:

```sh
FERRIKI_FERRONI_PATH=/absolute/path/to/ferroni node node/ferriki/scripts/build-native.mjs
node node/scripts/bench-tiobe.mjs --corpus curated --write /absolute/path/to/public-1.json
node node/scripts/bench-tiobe.mjs --corpus curated --write /absolute/path/to/public-2.json
```

Restore the Ferriki lockfile after the temporary local Cargo override, as in
the captured runs, and retain `.benchmark-build.json` with each result. Use
unmodified ordinary build flags for public measurements. Deterministically gzip
the two JSON files into this directory, then reproduce the checked comparison:

```sh
node summarize.mjs /absolute/path/to/ferriki .
node lookaround-candidates.mjs /absolute/path/to/ferriki ../../cpp_scanner/trace.json lookaround-candidates.json
```

Run the following on both compile benchmark commits, using the same SCSS file
for the control. Preserve the control/current/current/control ordering and
change the saved baseline name per run:

```sh
FERRONI_SCSS_TRACE=/absolute/path/to/merged-ferroni/benches/scss_scanner/trace.json cargo bench --locked --bench scanner_compile_bench -- --sample-size 30 --warm-up-time 1 --measurement-time 3 --save-baseline post-204-control-1
cargo bench --locked --features ffi --bench cpp_scanner_bench -- --test
cargo bench --locked --features ffi --bench scss_scanner_bench -- --test
cargo bench --locked --bench cpp_scanner_bench -- --sample-size 30 --warm-up-time 1 --measurement-time 3 --save-baseline post-204-replay
cargo bench --locked --bench scss_scanner_bench -- --sample-size 30 --warm-up-time 1 --measurement-time 3 --save-baseline post-204-replay
```

The oracle is Oniguruma `f95747b462de672b6f8dbdeb478245ddf061ca53`, configured
through `FERRONI_ONIGURUMA_DIR`. Build symbolized native code only for profiles:

```sh
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=none FERRIKI_FERRONI_PATH=/absolute/path/to/ferroni node node/ferriki/scripts/build-native.mjs
python3 record-cpu.py /absolute/path/to/ferriki/node java java-native-tokens.sample.txt --api tokens --scopes
python3 summarize-cpu.py profiles/*.sample.txt.gz > native-cpu-summary.json
```

Replace `java` and the API for each recorded row above, omit `--scopes` for
HTML, and restore the ordinary addon afterward. `record-cpu.py` and
`summarize-cpu.py` are unchanged copies of the pinned Ferriki profiling helpers.

## Validation

Validation commands, outcomes, and logs are retained in `validation/`.

| Check | Outcome |
| --- | --- |
| `cargo fmt --all --check` | PASSED |
| Clippy, workspace/all targets/all features, warnings denied | PASSED |
| Full locked all-feature tests, serial with documented debug stack | PASSED |
| Rustdoc, warnings denied | PASSED |
| Workflow action pins | PASSED |
| cargo-deny | PASSED on host; initial sandbox cache-lock failure retained |
| `mise run readme:check` | PASSED |
| Gzip/JSON integrity, Node syntax, exact result-summary reproduction | PASSED |
| Coverage / fuzzing | SKIPPED: no engine source changes |

The C oracle, two public parity matrices, five strict comparisons, and nine
native recording workloads passed. Coverage and fuzzing are skipped for this
benchmark/evidence-only change; engine source is unchanged. The public results
are machine-specific, warm medians, not a universal guarantee that every
input is faster. Native sample shares localize work, not its exact removable
cost.
