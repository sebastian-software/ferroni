# DFA candidate-walk boundaries: independent measurements

This October 17, 2026 experiment compares the candidate with `92c2363ed48c99f281805b6cfa4278126588a4c5`. Each branch starts directly from its repository's main; none includes Ferriki PR #279 or either of the other candidates. The packages use Node 24.21.0, rustc 1.99.0, Ferriki 1.1.1, Ferroni 2.2.2, and the ordinary stripped release profile (fat LTO, one codegen unit, no profiling flags) on an Apple M1 Ultra/macOS arm64 host. Control and candidate were built sequentially at the same worktree path and saved as independent packages and sidecars. Receipts record compiler, lockfile, source fingerprints, and addon SHA-256.

The 40-case curated screening (three workers, 12 warmups, 25 paired rounds) changes the equally weighted geometric mean by +0.54%. The most promising language was selected for an independent repetition with eight fresh workers, 12 warmups, and 60 rounds. The selection is exploratory; the repetition and the complete screening, including slower cases, are retained. The 3% threshold is case-specific and requires a benefit in both runs, with the repetition's upper 95% endpoint below zero.

| Selected native HTML case  | Screening | Repetition | Repetition 95% interval |
| -------------------------- | --------: | ---------: | ----------------------: |
| `curated/mdx/example/html` |    -1.46% |     +0.26% |        -0.85% to +1.50% |
| `curated/mdx/large/html`   |    -3.40% |     -0.40% |        -2.01% to +0.76% |

The independent single-theme homepage comparison uses 8 fresh workers, 12 warmups, 60 paired rounds, and 40 rotating cold-process pairs. It freezes all 14 input files at the published fixture revision `aea84dc7b911c79a86dd3631c698c991c36184b8`. Every warm worker first checks byte-for-byte HTML equality with Shiki/WASM. Small calls are batched and engine order rotates. Cold timing includes imports, corpus reading, grammar loading, and first renders; process creation is excluded. No builds or tests ran alongside the timed processes.

The single-theme warm total changes by -0.42%. The largest individual warm upper 95% endpoint is +2.32%. Cold start changes by -0.02% (-0.32% to +0.42%). The requested +2% regression bound is not established. The 3% gain criterion does not pass; this candidate remains a draft and is not recommended for merging on performance grounds.

| Single-theme homepage document                                           | Warm change |     95% interval |
| ------------------------------------------------------------------------ | ----------: | ---------------: |
| `src/highlighter.rs`                                                     |      -0.62% | -1.37% to +0.40% |
| `node/ferriki/src/api.mts`                                               |      -1.90% | -2.23% to -0.59% |
| `node/ferriki/src/index.mjs`                                             |      -0.41% | -0.57% to +0.16% |
| `docs/ferriki-api.md`                                                    |      -0.94% | -1.43% to +1.01% |
| `.github/workflows/ci.yml`                                               |      +0.02% | -0.65% to +0.94% |
| `node/compat/upstream/shiki/packages/twoslash/style-rich.css`            |      -1.14% | -2.59% to -0.74% |
| `node/package.json`                                                      |      -0.63% | -2.03% to +1.09% |
| `Cargo.toml`                                                             |      -0.73% | -1.34% to -0.00% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.py`   |      -0.85% | -3.95% to +0.99% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.html` |      +0.41% | -0.50% to +1.71% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.go`   |      -1.31% | -2.65% to -0.43% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.php`  |      -0.19% | -0.70% to +0.42% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.sh`   |      +0.91% | -3.02% to +2.32% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.sql`  |      +0.16% | -0.34% to +0.57% |

Intervals use 10,000 deterministic bootstrap samples of independent worker ratios or cold-process pairs. Warm totals are medians of worker ratios of summed document medians; native corpus summaries are geometric means. These are local measurements, not universal speed guarantees. Screening runs are kept separate from confirmation; no unfavorable worker is removed.

Ferriki is an unchanged main checkout with a temporary crates.io patch to this Ferroni worktree. Both control and candidate use the identical patched lockfile and path, differing only in `src/dfa_prefilter.rs`. The homepage harness explicitly allows the expected Ferroni source-fingerprint difference while checking identical Ferriki source, compiler, feature set, version, and build flags. This PR does not change Ferriki's dependency pin or imply that a published Ferriki package already contains the patch.

Validation commands and exit codes are in `validation.json`; complete logs are compressed alongside the raw timings. The source patch and measured harnesses are retained. Additional regression tests were added after the initial build; the validated release addon is byte-identical to the measured candidate. Both original and validated build receipts are retained. No upstream mirror is hand-edited.

To reproduce, build the parent and candidate sequentially in the same worktree with the pinned dependencies and ordinary `pnpm run build:native` command. Materialize assets with `node scripts/materialize-assets.mjs --payloads` and install the pinned pnpm 10.28.2 workspace dependencies first. Extract the archived `.py.gz` and `.mjs.gz` scripts with `gzip -dc`. Use `save-package.py` to save each package and its receipt to separate directories. For Ferroni, apply the same local crates.io patch to the consumer for both builds and retain its generated lockfile. Extract `homepage-harness.mjs.gz` to the consumer's `node/scripts/bench-independent-paired.mjs`; it uses repository-local Shiki and corpus helpers. The exact screening and confirmation arguments are in `run-screens.py.gz` and `run-finals.py.gz`; substitute local checkout and output paths. The native harness is also archived as `native-harness.mjs.gz`. Extract the published corpus revision before editing any measured file.

Run `shasum -a 256 -c SHA256SUMS` in this directory to verify the archived evidence.
