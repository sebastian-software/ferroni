# Blacksmith engine comparison, 2026-10-01

The first engine comparison, superseded on the same day by
[`blacksmith-2026-10-01-36926978824`](../blacksmith-2026-10-01-36926978824/README.md),
which adds the everyday regex tasks and the portable C and PHP replays. Raw data of
workflow run
[36915129576](https://github.com/sebastian-software/ferroni/actions/runs/36915129576)
of `.github/workflows/blacksmith-comparison.yml` on Ferroni
`11a7ba0248c920d4874d169349633bc8d761e8db`, with every case set on both runner
profiles.

| Directory | Runner | Host |
| --- | --- | --- |
| `macos-arm64-*` | `blacksmith-6vcpu-macos-26` | Apple M4 Pro (virtual), 6 vCPUs, 24 GB, macOS 26.3 |
| `linux-x86-64-*` | `blacksmith-4vcpu-ubuntu-2404` | AMD EPYC with AVX-512, 4 vCPUs, 16 GB, Ubuntu 24.04.3 |

Each directory holds one job of `scripts/compare-engines.py run`:

- `measurements.json`: host, toolchain and binary receipt, settings, and the
  Criterion mean, median and confidence interval per case and engine.
- `engine-notes.json`: the cases an engine could not run, with the reason, and
  the replays accepted with empty captures reported as unset.
- `summary.md`: that job's step summary.
- `logs.tar.gz`: validation and Criterion output.

`summary.md` at the top merges all six jobs, as the workflow's summary job does:

```bash
./scripts/compare-engines.py report summary.md benches/results/blacksmith-2026-10-01
```

`SHA256SUMS` covers every file except this README.
