# Whole literal tries in negative lookahead

The isolated compiler change reduces C++ **finished inline HTML time by 35–36%**
on the large curated fixture and **32–33%** on the small example. Ferriki is now
faster than both Shiki backends for C++ in both complete runs and both sizes.
The primary target, **Ferriki HTML < min(Shiki WASM HTML, Shiki JS HTML)**,
improves from 13/20 to **14/20 formats** in both runs for both sizes.

The captured C++ scanner replay takes 40–44% less time. Uncached C++ scanner
construction plus teardown costs 3–5% more; SCSS construction changes by
+0.9% / +1.8%. Both ordinary native addon files are 2,157,936 bytes. These are
local measurements on one interactive desktop, not a portable speed guarantee.

## Change and guards

The existing literal-alternation compiler pass can now lower an entire plain
literal list inside an otherwise unanchored negative lookahead. C++ keyword
exclusion lists of 14, 108, and 114 branches identified in the post-#204 analysis
are the motivating workload. The grammar expressions are unchanged.

Eligibility retains the existing threshold of four literals, plain-string and
nonempty checks, branch-order preservation, and case-fold compatibility checks.
Positive lookahead, lookbehind, nested assertions, and mixed-run compaction
inside assertions retain their existing paths. No VM instructions, public API,
dependencies, unsafe blocks, or separate pre-optimizer are added. Existing trie
construction bounds storage by the accepted literal input; no nested-path
expansion becomes eligible. Capture wrappers and trailing word boundaries keep
their original structure. Retry/stack limits can observe fewer backtracks, as
already documented for literal tries in ADR-008.

## Fixed boundary and provenance

- Date: 2026-10-01, Apple M1 Ultra, Darwin 27.0.0, Node 24.21.0, Rust 1.96.0.
- Control: `cba34468374bc1ad3903c5b1ae3b95c95621d540`, merged #205.
- Candidate: `3caf0cbb6aac95aae89c9bc4145c77586bb31b2b`, isolated compiler change.
- Ferriki, harness, fixtures, assets, and dependencies remain on
  `0136c99d3aad9aad9eeaa025692124e94420153d`.
- Public addon: ordinary fat LTO, one codegen unit, no profiling flags.
  Full build receipts and binary/source hashes are retained. Building the local
  path patch modifies Cargo.lock; the original lockfile is restored before
  measurement. The original addon and lockfile are restored byte-for-byte after
  each experiment.
- Public schedule: control 1, candidate 1, candidate 2, control 2; sequential
  formats, fresh worker per format, rotating engine order, five warmups,
  at least 30 samples, 300 ms shared engine budget. Setup, compilation and
  parity validation are outside timing; output allocation is included.
- Native Criterion: ordinary Ferroni thin LTO, 30 samples, one-second warmup,
  three-second requested measurement; the same ABBA order. No builds, coverage,
  profiles or other benchmarks run concurrently with timed workloads.
- C oracle: Oniguruma `f95747b462de672b6f8dbdeb478245ddf061ca53`.
  Fixture hashes and corpus counts are in `measurement-contract.json`.

HTML is the primary product metric. Tokens remain a secondary diagnostic and
parity boundary. These runs measure **inline HTML only** on the pinned Ferriki
version; **HTML with generated CSS in classes mode is unmeasured**.

## Public HTML

Milliseconds per document, run 1 / run 2. Negative changes mean less time.
The best Shiki column selects the faster HTML backend independently in each
candidate run; a Ferriki / best Shiki ratio below one meets the target.
The C++ best backend is WASM. All small and large results, raw samples and
output hashes remain in the four compressed reports and `summary.json`.

| Format | Control HTML (ms) | Candidate HTML (ms) | Paired time change | Best Shiki HTML (ms) | Ferriki / best Shiki |
| --- | ---: | ---: | ---: | ---: | ---: |
| TypeScript | 25.71 / 25.86 | 25.63 / 25.95 | -0.32 / 0.36% | 41.82 / 42.54 | 0.61 / 0.61 |
| TSX | 26.94 / 26.45 | 26.15 / 26.38 | -2.93 / -0.25% | 39.99 / 39.86 | 0.65 / 0.66 |
| Rust | 13.42 / 13.29 | 13.24 / 13.40 | -1.36 / 0.89% | 19.30 / 19.66 | 0.69 / 0.68 |
| CSS | 16.16 / 15.87 | 16.22 / 16.04 | 0.36 / 1.06% | 9.83 / 9.95 | 1.65 / 1.61 |
| HTML | 25.96 / 25.47 | 25.50 / 25.07 | -1.77 / -1.58% | 35.03 / 34.85 | 0.73 / 0.72 |
| C++ | 78.70 / 76.85 | 51.03 / 48.85 | -35.15 / -36.44% | 56.65 / 55.71 | 0.90 / 0.88 |
| Swift | 16.07 / 15.87 | 16.16 / 15.91 | 0.55 / 0.25% | 21.54 / 21.15 | 0.75 / 0.75 |
| Java | 22.26 / 21.79 | 22.25 / 21.53 | -0.06 / -1.21% | 20.44 / 19.72 | 1.09 / 1.09 |
| Markdown | 15.55 / 14.36 | 14.23 / 14.45 | -8.47 / 0.63% | 21.76 / 22.31 | 0.65 / 0.65 |
| TOML | 5.22 / 5.48 | 5.11 / 5.16 | -2.04 / -5.89% | 2.82 / 2.85 | 1.81 / 1.81 |
| YAML | 8.35 / 8.55 | 8.49 / 8.23 | 1.71 / -3.73% | 7.25 / 7.10 | 1.17 / 1.16 |
| JSON | 9.57 / 9.24 | 9.17 / 9.14 | -4.21 / -1.08% | 5.80 / 5.92 | 1.58 / 1.54 |
| Astro | 24.02 / 23.71 | 23.84 / 24.92 | -0.74 / 5.11% | 45.86 / 47.72 | 0.52 / 0.52 |
| Svelte | 29.30 / 29.59 | 29.59 / 30.20 | 0.98 / 2.05% | 60.16 / 61.66 | 0.49 / 0.49 |
| Ruby | 13.16 / 13.18 | 13.24 / 13.07 | 0.58 / -0.83% | 31.42 / 31.85 | 0.42 / 0.41 |
| Python | 14.96 / 15.05 | 14.92 / 15.00 | -0.28 / -0.32% | 26.31 / 25.87 | 0.57 / 0.58 |
| Vue | 22.80 / 22.75 | 22.52 / 22.59 | -1.25 / -0.71% | 44.99 / 44.29 | 0.50 / 0.51 |
| MDX | 23.03 / 22.75 | 22.49 / 22.79 | -2.30 / 0.20% | 35.40 / 35.67 | 0.64 / 0.64 |
| SCSS | 14.61 / 14.28 | 15.16 / 14.30 | 3.76 / 0.17% | 13.62 / 12.46 | 1.11 / 1.15 |
| Bash | 13.32 / 13.83 | 13.67 / 13.59 | 2.59 / -1.71% | 14.45 / 13.87 | 0.95 / 0.98 |

The largest raw HTML increases in the complete runs are +5.3% for the small
Bash example in run 1 and +5.1% for large Astro in run 2. These do not repeat
across both full runs. Large SCSS changes by +3.8% in run 1 while its Shiki JS
comparator also increases by 6.9%. Small changes on this machine need repetition.

## Focused outlier repeats

A second ABBA sequence repeats Astro, SCSS, and Bash with the same saved ordinary
addons, public method, and both sizes. The initial increases do not reproduce:
all paired changes lie between -4.0% and +0.3%. The table also shows changes
relative to the faster Shiki backend to expose comparator/host movement.
There is no demonstrated new large regression in the measured corpus.

| Format | Size | Paired HTML time change | Change relative to best Shiki |
| --- | --- | ---: | ---: |
| astro | example | -4.01 / -1.75% | -1.44 / -1.78% |
| astro | large | -0.99 / 0.25% | -0.50 / 1.04% |
| scss | example | -1.58 / -0.45% | -2.17 / -2.09% |
| scss | large | -0.45 / -0.43% | 0.01 / 0.10% |
| bash | example | -0.19 / -0.12% | 0.47 / 0.60% |
| bash | large | -1.70 / -0.20% | -1.03 / -0.19% |

The two partial control reports before correcting the CLI's case-sensitive Bash
selector are retained under `focused-preflight/`. They are not part of the final
complete ABBA comparison. No candidate timing preceded that selector failure.

## Scanner replay and construction

Replay includes the recorded scanner searches and returned captures, with setup
outside timing. Construction loads JSON outside timing and constructs and drops
all captured scanners per iteration; this is not whole-app cold-start latency.

| Case | Control mean (ms) | Candidate mean (ms) | Paired time change |
| --- | ---: | ---: | ---: |
| document | 58.50 / 60.85 | 34.99 / 34.23 | -40.19 / -43.76% |
| group_17 | 1.40 / 1.45 | 0.77 / 0.77 | -45.11 / -46.84% |
| group_58 | 8.32 / 8.38 | 3.81 / 3.79 | -54.17 / -54.80% |
| group_61 | 1.04 / 1.04 | 0.62 / 0.62 | -40.66 / -40.40% |
| group_70 | 2.19 / 2.17 | 1.52 / 1.52 | -30.50 / -30.11% |
| group_77 | 0.45 / 0.46 | 0.36 / 0.36 | -20.14 / -20.42% |
| group_78 | 36.46 / 37.26 | 21.52 / 21.58 | -40.99 / -42.08% |
| cpp | 230.02 / 229.30 | 237.30 / 240.67 | 3.16 / 4.96% |
| scss | 54.86 / 55.25 | 55.37 / 56.27 | 0.94 / 1.84% |

`criterion/` retains samples, mean/median estimates, 95% confidence intervals,
and benchmark identities. `native-summary.json` keeps both mean and median
estimates; individual pattern timings are not summed into scanner costs.

## Correctness and checks

The full all-feature suite passes: 2,360 tests and 14 doctests; two existing
opt-in tests remain ignored. The tree contains 2,362 test functions.
The targeted assertion matrix compares results and full captures with a
semantically equivalent blocked-trie path for 5,760 searches, covering prefixes,
branch order, repeated/empty literals, case folding, Unicode/malformed input,
quantifiers, back references, and forward/bounded/backward searches. The guard
regression fails on the control compiler and passes on the candidate.

The C oracle validates every captured call and full capture: 4,862 C++ and
3,744 SCSS calls. All 20 formats, both sizes, and all three TextMate engines
retain exact token and HTML parity in all four full matrices. Prism preserves
the source in its 16 supported formats; Astro, MDX, Svelte and Vue remain
explicitly unsupported. Four strict full-report comparisons each accept all
80 Ferriki API/case pairs with zero exclusions. Twelve focused comparisons
accept four API/case pairs each with zero exclusions.

Formatting, all-target/all-feature Clippy, rustdoc, cargo-deny, workflow pins,
README generation, focused MSRV 1.94 tests, and docs formatting/lint/typecheck/
build pass. Coverage is **89.97%** against the existing 87% gate, using installed
nightly-2026-07-20; the benchmark toolchain remains stable Rust 1.96.0.
Final quality commands and outcomes are retained in `validation/`. The
TypeScript grammar census gains one literal trie; all other census counts
remain unchanged. README counts come from `scripts/count-tests.sh` and the
source/generated README pair is regenerated with the repository's native tool.

## Reproduction

Use the pinned Ferriki checkout and build each Ferroni commit with its ordinary
native build script. Run the following sequentially for each control/candidate
matrix, saving reports in ABBA order:

```sh
FERRIKI_FERRONI_PATH=/path/to/ferroni node node/ferriki/scripts/build-native.mjs
node node/scripts/bench-tiobe.mjs --corpus curated --write report.json
# Focused repeats use --language astro, --language scss, or --language Bash.
```

Restore the original Ferriki Cargo.lock after building the local path patch,
before running the public harness. Retain the build receipt and use identical
Ferriki source, dependencies, assets, features, and native flags.

From each Ferroni checkout:

```sh
RUST_MIN_STACK=268435456 cargo test --locked --all-features -- --test-threads=1
FERRONI_ONIGURUMA_DIR=/path/to/pinned/oniguruma cargo bench --locked --features ffi --bench cpp_scanner_bench -- --test
FERRONI_ONIGURUMA_DIR=/path/to/pinned/oniguruma cargo bench --locked --features ffi --bench scss_scanner_bench -- --test
cargo bench --locked --bench cpp_scanner_bench -- --sample-size 30 --warm-up-time 1 --measurement-time 3
cargo bench --locked --bench scanner_compile_bench -- --sample-size 30 --warm-up-time 1 --measurement-time 3
```

Recompute the retained summaries without running timings:

```sh
node summarize.mjs /path/to/pinned/ferriki /path/to/this/directory
node summarize-focused.mjs /path/to/pinned/ferriki /path/to/this/directory
python3 summarize-native.py
shasum -a 256 -c SHA256SUMS
```

C++ meets the HTML target on these fixtures. The next Ferroni candidate is Java,
whose previous native HTML profile still spends 57% in Ferroni. CSS, JSON and
TOML have larger remaining HTML gaps but their earlier profiles put most of the
cost outside the regex engine; those need a Ferriki-side investigation.
