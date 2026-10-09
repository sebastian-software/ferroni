# Blacksmith engine comparison, 2026-10-09, run 37989957431

Raw data behind [Engine Comparison](https://ferroni.dev/perf/engine-comparison):
workflow run
[37989957431](https://github.com/sebastian-software/ferroni/actions/runs/37989957431)
of `.github/workflows/blacksmith-comparison.yml` on Ferroni
`a38768bc80121a1bd72e3e280f12063dc407bc04`, with every case set on both runner
profiles. It is the first published run with the DFA pre-filter of the
scanner's RegSet searches ([#321](https://github.com/sebastian-software/ferroni/pull/321),
ADR-008) and with fancy-regex in seek mode and as a `RegexSet`
([#316](https://github.com/sebastian-software/ferroni/pull/316)).

The published comparison takes `shared`, `oniguruma` and `micro` from this
run. Its `textmate` set is superseded by
[`blacksmith-2026-10-09-37999607702`](../blacksmith-2026-10-09-37999607702/README.md):
it was measured before [#324](https://github.com/sebastian-software/ferroni/pull/324)
retired the pre-filter on sets whose DFA cache thrashes. The flat CSS scanner
of `battle_bench` is such a set, and its rows show the regression:
`css_117_tokenize` took 14 ms on Apple Silicon and 16 ms on x86-64 (C at 0.60
and 0.67 of Ferroni), `css_117_document_19_lines` 11.1 ms and 13.4 ms (C at
0.18 and 0.21), against 158 µs and 79.8 µs before the pre-filter. The replays
and the other grammar scanners are unaffected and agree with the later run
within noise.

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

`summary.md` at the top merges all eight jobs, as the workflow's summary job
does. The figures and tables are regenerated over both runs, the later one
replacing the `textmate` set:

```bash
./scripts/compare-engines.py report summary.md benches/results/blacksmith-2026-10-09-37989957431
./scripts/compare-engines.py figures benches/results/blacksmith-2026-10-09-37989957431 benches/results/blacksmith-2026-10-09-37999607702 docs/app/data/engine-comparison.json
./scripts/compare-engines.py tables benches/results/blacksmith-2026-10-09-37989957431 benches/results/blacksmith-2026-10-09-37999607702
```

`SHA256SUMS` covers every file except this README.
