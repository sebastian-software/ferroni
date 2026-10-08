# Blacksmith engine comparison, 2026-10-08, run 37782529019

Raw data behind [Engine Comparison](https://ferroni.dev/perf/engine-comparison):
workflow run
[37782529019](https://github.com/sebastian-software/ferroni/actions/runs/37782529019)
of `.github/workflows/blacksmith-comparison.yml` on Ferroni
`62892008bbf59e3d06b9c8599a4decdf1e8ba979`, with every case set on both runner
profiles.

| Directory | Runner | Host |
| --- | --- | --- |
| `macos-arm64-*` | `blacksmith-6vcpu-macos-26` | Apple M4 Pro (virtual), 6 vCPUs, 24 GB, macOS 26.3 |
| `linux-x86-64-*` | `blacksmith-4vcpu-ubuntu-2404` | AMD EPYC with AVX-512, 4 vCPUs, 16 GB, Ubuntu 24.04.4 |

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
./scripts/compare-engines.py report summary.md benches/results/blacksmith-2026-10-08-37782529019
./scripts/compare-engines.py figures benches/results/blacksmith-2026-10-08-37782529019 docs/app/data/engine-comparison.json
./scripts/compare-engines.py tables benches/results/blacksmith-2026-10-08-37782529019
```

`SHA256SUMS` covers every file except this README.
