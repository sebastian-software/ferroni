# Blacksmith engine comparison, 2026-10-01, run 36926978824

Raw data behind [Engine Comparison](https://ferroni.dev/perf/engine-comparison):
workflow run
[36926978824](https://github.com/sebastian-software/ferroni/actions/runs/36926978824)
of `.github/workflows/blacksmith-comparison.yml` on Ferroni
`6db6413e84b03cb806f46cc96ccd972b5eb8c670`, with every case set on both runner
profiles.

| Directory | Runner | Host |
| --- | --- | --- |
| `macos-arm64-*` | `blacksmith-6vcpu-macos-26` | Apple M4 Pro (virtual), 6 vCPUs, 24 GB, macOS 26.3 |
| `linux-x86-64-*` | `blacksmith-4vcpu-ubuntu-2404` | AMD EPYC with AVX-512, 4 vCPUs, 16 GB, Ubuntu 24.04.3 |

Every case set (`shared`, `oniguruma`, `micro`, `textmate`) ran on both
profiles; each directory holds one job of `scripts/compare-engines.py run`:

- `measurements.json`: host, toolchain and binary receipt, settings, and the
  Criterion mean, median and confidence interval per case and engine.
- `engine-notes.json`: the cases an engine could not run, with the reason, and
  the replays accepted with empty captures reported as unset.
- `summary.md`: that job's step summary.
- `logs.tar.gz`: validation and Criterion output.

`summary.md` at the top merges all eight jobs, as the workflow's summary job does:

```bash
./scripts/compare-engines.py report summary.md benches/results/blacksmith-2026-10-01-36926978824
./scripts/compare-engines.py figures benches/results/blacksmith-2026-10-01-36926978824 docs/app/data/engine-comparison.json
./scripts/compare-engines.py tables benches/results/blacksmith-2026-10-01-36926978824
```

`SHA256SUMS` covers every file except this README.
