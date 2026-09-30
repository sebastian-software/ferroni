# Contiguous literal alternatives: compiler experiment

The SCSS tag, property and value patterns mix hundreds of literals with a
few character classes, optional suffixes or nested groups. The complete
alternation therefore misses the existing pure-literal trie optimization.

This experiment compacts only **consecutive plain literal branches** in
place, with at least 16 literals and at most 8,192 per trie. It reuses the
existing literal/folded trie compiler and VM. Non-literal branches, capture
groups, scoped option boundaries, Unicode folding and alternation order keep
their existing paths. It adds no opcode, public API, dependency or unsafe code.
The pass remains in the existing literal-alternation compiler section.
No original expression or grammar asset changes.

Nested path extraction remains excluded: the existing optional extractor
does not preserve every lazy/greedy priority or internal capture. Moving
literal branches ahead of non-literals would also be incorrect. Grouping
adjacent literal branches preserves the complete ordered choice instead.

## Initial scanner result

The ordinary thin-LTO Criterion build, toolchain, corpus, warmup and sample
settings match the retained [baseline](../ferroni-1.7.0/README.md). Two full
optimized runs put the SCSS document replay at **3.35 / 3.51 ms**, compared
with **25.77 / 26.12 ms** before, about 86–87% less scanner time. A later
baseline/optimized pair gives **30.88 / 3.49 ms**. The baseline varies;
all runs are retained, and this is a fixture-specific scanner result.
The public pipeline is measured separately below.

All 3,744 SCSS and 4,862 C++ replay calls retain exact matched indices and
captures, independently validated against pinned C Oniguruma before timing.
The full all-features test suite passes with the prescribed debug stack.
New differential tests compare mixed alternatives against unoptimized
equivalent expressions across captures, backreferences, repetition,
atomic repetition, forward/backward searches, Unicode folds and malformed
UTF-8. Lookaround bodies and scoped option boundaries retain their guards.

The implementation census changes intentionally: TypeScript gains three
literal tries and three outer byte-set guards; CSS gains 15 folded tries,
reduces internal byte-set guards from 2,764 to 1,275, and changes one entry
from table dispatch to fallback search through the existing conservative
folded-trie length summary. Rust's census stays unchanged. The behavior
suites and full captured C oracle remain unchanged.

## Complete Ferriki pipeline

The unchanged Ferriki runtime at `0136c99` was built twice with its ordinary
release settings (fat LTO, one codegen unit): once against Ferroni `16af64c`
and once against compiler commit `1d0df6c`. Subsequent `bf95edb` only expands
test cases. Both build receipts and binary hashes are retained in `public/`.
The temporary Cargo.lock change comes from patching the Ferroni dependency;
the original lockfile was restored before every measurement, and the original
addon was restored afterward. No grammar, wrapper, engine option or native
compiler flag changed between the two builds.

One fresh control/candidate matrix measures all 20 formats in both sizes,
with five warmups, at least 30 samples and a 300 ms measurement budget per
API/engine, in fresh sequential language processes with rotating engine order.
All TextMate engines retain exact token and HTML parity for all 40 fixtures.
Prism preserves the source in its 16 supported formats; four formats explicitly
remain unsupported. Ferriki's strict comparison accepts all 80 HTML/token
comparisons with no exclusions.

The following are **warm public API medians in milliseconds for large fixtures**,
not scanner-only timings. Negative percentages mean less elapsed time.

| Format | HTML before → after | Change | Tokens before → after | Change |
| --- | ---: | ---: | ---: | ---: |
| Astro | 26.23 → 23.75 | -9.4% | 36.77 → 31.64 | -13.9% |
| Bash | 13.69 → 13.50 | -1.3% | 22.32 → 22.12 | -0.9% |
| C++ | 81.45 → 79.90 | -1.9% | 146.04 → 142.00 | -2.8% |
| CSS | 23.95 → 16.46 | -31.2% | 30.72 → 16.58 | -46.0% |
| HTML | 28.83 → 26.30 | -8.8% | 41.52 → 37.37 | -10.0% |
| JSON | 9.72 → 9.15 | -5.8% | 15.51 → 15.25 | -1.7% |
| Java | 22.86 → 22.32 | -2.4% | 39.07 → 37.55 | -3.9% |
| MDX | 24.07 → 24.29 | +0.9% | 37.29 → 36.08 | -3.2% |
| Markdown | 14.09 → 14.82 | +5.2% | 21.43 → 22.73 | +6.1% |
| Python | 15.38 → 15.48 | +0.7% | 21.91 → 22.23 | +1.5% |
| Ruby | 13.21 → 13.26 | +0.4% | 20.15 → 20.21 | +0.3% |
| Rust | 13.88 → 13.65 | -1.6% | 18.66 → 18.41 | -1.3% |
| SCSS | 38.60 → 14.73 | -61.8% | 72.54 → 23.06 | -68.2% |
| Svelte | 33.41 → 29.72 | -11.0% | 54.96 → 51.08 | -7.1% |
| Swift | 16.75 → 16.11 | -3.8% | 24.61 → 23.74 | -3.5% |
| TOML | 5.49 → 5.32 | -3.1% | 8.20 → 8.06 | -1.8% |
| TSX | 28.35 → 27.80 | -1.9% | 49.07 → 47.68 | -2.8% |
| TypeScript | 28.09 → 26.03 | -7.3% | 48.04 → 46.45 | -3.3% |
| Vue | 27.38 → 23.10 | -15.6% | 46.02 → 38.24 | -16.9% |
| YAML | 8.91 → 8.46 | -5.1% | 17.93 → 17.70 | -1.3% |

SCSS improves from **38.60 to 14.73 ms** for HTML and **72.54 to 23.06 ms**
for tokens. CSS improves from **23.95 to 16.46 ms** and **30.72 to 16.58 ms**.
In the candidate run, Shiki WASM takes 61.26 / 57.62 ms for SCSS HTML/tokens,
and Shiki JS 12.96 / 9.64 ms. Ferriki closes much of this gap; JS still leads
on this SCSS fixture.

## Repeats and measurement limits

The first matrix shows approximately 5–8% slower Markdown/MDX cases, while
Shiki controls in the same run also slow by similar amounts. Four focused
repeats reverse build order (candidate first, control second), still checking
both sizes and exact parity:

| Large fixture | HTML change | Token change |
| --- | ---: | ---: |
| SCSS | −62.9% | −68.0% |
| CSS | −30.0% | −41.1% |
| Markdown | −7.8% | −3.2% |
| MDX | −7.3% | −5.9% |

The initial Markdown/MDX regressions do not reproduce. Shiki controls also
improve in these repeats, so neither their initial regression nor repeated
improvement establishes a compiler effect. Small Markdown tokens change by
only +0.1% in the repeat. No new large regression is established by these
measurements; this is not a guarantee across arbitrary inputs or machines.

All raw distributions, sample counts, p95 values, engine controls and exact
provenance remain in the compressed reports. The initial full control is also
retained under `initial-control-old-build-receipt.json.gz`: it used an earlier
Ferriki build receipt. Rebuilding the control at `0136c99` produced the exact
same binary hash, but a new full control was measured for the strict paired
comparison rather than changing that earlier report's provenance.

Compilation is lazy in Ferriki. `importAndSetupMs` therefore does **not** isolate
scanner compilation. These results establish warm highlighting gains; a separate
cold scanner-compilation measurement is still needed before drawing conclusions
about startup costs. C++ declaration/fallback search remains a separate target:
its small changes in this matrix also appear in the controls and do not establish
a speedup. Its pattern diagnosis is retained in the baseline folder.

## Reproduce the public comparison

Check out Ferriki `0136c99d3aad9aad9eeaa025692124e94420153d` with its committed
Node dependencies and ordinary native release profile. Build its addon against
each pinned Ferroni revision via Cargo's `patch.crates-io.ferroni.path`, using
`FERRIKI_FERRONI_PATH=/path/to/ferroni node node/ferriki/scripts/build-native.mjs`
to build and record the actual artifact. Restore
the committed Cargo.lock before measurement. Do not run another benchmark,
compiler or test suite concurrently with the timings.

Run `node node/scripts/bench-tiobe.mjs --corpus curated --write REPORT.json`
for each artifact. For the reverse-order focused repeats add `--language scss`
(or `css`, `markdown`, `mdx`). Preserve each report and restore the ordinary addon
afterward. Compare the retained reports with the existing Ferriki gates:

```sh
node benches/scss_scanner/results/contiguous-literals/summarize-public.mjs \
  /path/to/ferriki \
  benches/scss_scanner/results/contiguous-literals/public/control.json.gz \
  benches/scss_scanner/results/contiguous-literals/public/candidate.json.gz \
  /tmp/scss-public-comparison.json
```

The scanner benchmark uses the same Criterion command and C validation as the
baseline, saving each run independently. `manifest.json` records SHA-256 hashes
of the compressed evidence. No diagnostic scanner capture is compiled into the
measured engine.

## Validation

The complete all-features Rust tests, Clippy with warnings denied, Rustdoc with
warnings denied, formatting, workflow pins, README generation and cargo-deny
pass locally. Ferriki's supported core compatibility runner also passes with
the candidate addon, including its facade contracts and nine mandatory upstream
test files; its documented deferred contracts remain deferred. The packaged
file listing excludes the standalone profiling example and benchmarks as intended.

The host's September nightly Cargo emits test executables into a new `build/`
layout that installed cargo-llvm-cov 0.8.4 cannot discover. This is a local
coverage tooling mismatch, not a failed test. Coverage is repeated with the
already-installed `nightly-2026-07-20` and its LLVM tools in a fresh target
directory to avoid mixing LLVM profile formats; the unchanged gate passes
with **89.90% line coverage** (required 87%). The expanded tests also pass on
MSRV Rust 1.94.
