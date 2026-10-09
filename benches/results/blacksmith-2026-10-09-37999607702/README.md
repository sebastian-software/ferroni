# Blacksmith engine comparison, 2026-10-09, run 37999607702

Raw data behind [Engine Comparison](https://ferroni.dev/perf/engine-comparison):
workflow run
[37999607702](https://github.com/sebastian-software/ferroni/actions/runs/37999607702)
of `.github/workflows/blacksmith-comparison.yml` on Ferroni
`b93c6d2035f4919480f43d9b7d4134541e4cb9b5`, with the `textmate` case set on
both runner profiles.

It repeats the highlighting set of
[`blacksmith-2026-10-09-37989957431`](../blacksmith-2026-10-09-37989957431/README.md),
measured one commit earlier, after
[#324](https://github.com/sebastian-software/ferroni/pull/324) retired the
DFA pre-filter on sets whose DFA cache thrashes (ADR-008). The flat CSS scanner
of `battle_bench` is such a set: its rows had regressed to 11 to 16 ms in the
earlier run and are back at 73 to 149 µs here. The replays and the other
grammar scanners agree with the earlier run within noise. The published
comparison takes `textmate` from this run and the other three case sets from
the earlier one; #324 changes nothing they measure.

| Directory | Runner | Host |
| --- | --- | --- |
| `macos-arm64-*` | `blacksmith-6vcpu-macos-26` | Apple M4 Pro (virtual), 6 vCPUs, 24 GB, macOS 26.3 |
| `linux-x86-64-*` | `blacksmith-4vcpu-ubuntu-2404` | AMD EPYC with AVX-512, 4 vCPUs, 16 GB, Ubuntu 24.04.4 |

Each directory holds one job of `scripts/compare-engines.py run`:

- `measurements.json`: host, toolchain and binary receipt, settings, and the
  Criterion mean, median and confidence interval per case and engine.
- `engine-notes.json`: the cases an engine could not run, with the reason, and
  the replays accepted with empty captures reported as unset.
- `summary.md`: that job's step summary.
- `logs.tar.gz`: validation and Criterion output.

`summary.md` at the top merges both jobs, as the workflow's summary job does.
The figures and tables are regenerated over both runs, this one replacing the
`textmate` set:

```bash
./scripts/compare-engines.py report summary.md benches/results/blacksmith-2026-10-09-37999607702
./scripts/compare-engines.py figures benches/results/blacksmith-2026-10-09-37989957431 benches/results/blacksmith-2026-10-09-37999607702 docs/app/data/engine-comparison.json
./scripts/compare-engines.py tables benches/results/blacksmith-2026-10-09-37989957431 benches/results/blacksmith-2026-10-09-37999607702
```

`SHA256SUMS` covers every file except this README.
