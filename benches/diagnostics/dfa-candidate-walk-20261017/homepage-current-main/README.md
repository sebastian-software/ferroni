# Current-main homepage guard for #415

I am keeping the small runtime change. The confirmed scanner benefit is useful, the implementation adds only a local bound and no allocation or abstraction, and the current-main homepage guard passes the 2% limit.

This October 18, 2026 decision supersedes the earlier draft recommendation in the parent and scanner reports. I accept a useful local gain from a small implementation change; a 3% reduction in a complete Ferriki task is not a prerequisite for this decision. The 2% regression limit still applies separately to every homepage document and cold startup.

## Fixed comparison

- Ferriki: `e7a4fb17e1eb34b45d4ed1bcb8d36d8de4ce85d8`, after Ferriki #279 and #281.
- Ferroni baseline: `92c2363ed48c99f281805b6cfa4278126588a4c5`.
- Ferroni runtime candidate: `adb31da5001459a10d5456209823dbeb8bff38ae`. Later commits change evidence only.
- Apple M1 Ultra, macOS arm64, Node 24.21.0, rustc 1.99.0. Ordinary Ferriki fat-LTO release profile, one codegen unit, no profiling flags or instrumentation.
- The independent baseline/candidate packages and sidecars are the exact current-consumer builds retained in the scanner report. The harness verifies addon hashes, identical Ferriki source, lockfile, compiler, features, and flags. Facade hashes are identical too.
- Frozen 14-document homepage corpus: `aea84dc7b911c79a86dd3631c698c991c36184b8`.
- Before the first timed process I fixed 32 independent warm workers, 12 warm-ups, 60 rotating paired rounds, and 60 alternating cold process pairs. Every process is retained; no cohort is extended.

Warm timing calls the public Node `codeToHtml` API. Every worker first checks exact HTML against Shiki/WASM. Small calls are batched. Cold timing includes imports, corpus reading, grammar loading, and first highlights, excluding process creation. The driver checkout is `ec10cb9`; its four imported helpers are byte-identical to the current consumer revision, as checked before execution. The measured engines are independent saved packages, not that driver's addon.

## Results

Negative values mean less runtime. The estimate is the median independent paired ratio. The paired 95% percentile interval uses 10,000 deterministic bootstrap resamples (seed 20261017) of worker ratios or cold process pairs. These are local per-case intervals, not simultaneous bounds or portable guarantees.

| Homepage document | Runtime change | Paired 95% interval |
| --- | ---: | ---: |
| `src/highlighter.rs` | -0.52% | -0.81% to -0.21% |
| `node/ferriki/src/api.mts` | -0.55% | -0.95% to -0.27% |
| `node/ferriki/src/index.mjs` | +0.04% | -0.18% to +0.31% |
| `docs/ferriki-api.md` | -0.34% | -0.67% to -0.02% |
| `.github/workflows/ci.yml` | +0.29% | +0.07% to +0.45% |
| `node/compat/upstream/shiki/packages/twoslash/style-rich.css` | -0.47% | -0.74% to -0.04% |
| `node/package.json` | -0.38% | -0.52% to -0.09% |
| `Cargo.toml` | +0.01% | -0.28% to +0.20% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.py` | +0.09% | -0.49% to +0.66% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.html` | -0.33% | -0.57% to +0.02% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.go` | -0.96% | -1.14% to -0.81% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.php` | -0.35% | -0.58% to -0.09% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.sh` | -0.24% | -0.49% to +0.16% |
| `node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.sql` | -0.19% | -0.80% to -0.02% |

The warm total changes by +0.01%. The largest individual warm upper endpoint is +0.66% (`node/compat/upstream/vscode-textmate/test-cases/themes/tests/test.py`). Cold startup changes by -0.11% (-0.54% to +0.25%). The predeclared guard **passes**.

YAML is the one case with a small positive interval: +0.29% runtime (+0.07% to +0.45%). That is a measured local slowdown within the 2% tolerance; the guard does not mean every case becomes faster.

The earlier scanner evidence remains separate: TSX group 2 takes 3.35% less time (paired 95% interval -3.82% to -2.84%). Full native TSX HTML improves by 0.63% on the example and 0.15% on the large fixture. A separate ordinary-driver diagnostic uses 8.40% fewer instructions per TSX replay; this is not an equivalent runtime gain or a Ferriki-addon instruction claim. The outlined scanner function in the inspected thin-LTO binary shrinks from 2,688 to 2,296 bytes; this is not total binary size.

## Validation and artifacts

The runtime and regression tests are unchanged from the scanner follow-up. The candidate already passed the focused prefilter/self-check suites, Ferriki's mandatory core compatibility gate, and complete curated/TIOBE output parity checks. The validated addon is byte-identical to the measured candidate. All PR CI checks at `ca1bda1` passed before this evidence-only update. This comparison adds exact homepage HTML checks in every warm worker.

`result.json.gz` retains every warm sample, cold process pair, fixture/output hash, environment, and build provenance. `summary.json` is reproduced from those raw samples, including every bootstrap endpoint. Build receipts, driver/corpus fingerprints, the fixed plan, exact harness, and reproduction scripts are included. No unfavorable worker or cold pair is removed. `run.py` retains the original absolute paths and checkout assertions; adapt them to fresh owned paths when reproducing. It installs a temporary copy of the harness beside the repository helpers and removes only that exact file afterward. The driver checkout remains clean.

Verify this directory with `shasum -a 256 -c SHA256SUMS`.
