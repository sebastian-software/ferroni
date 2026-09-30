# Ferroni 1.7.0 C++ scanner baseline

The uninstrumented replay reproduces the regex bottleneck independently of
Ferriki. All 4,862 calls match the captured pattern indices and complete
captures; C Oniguruma independently agrees on every call. This is a baseline,
not an optimization result.

Two sequential Criterion runs on an Apple M1 Ultra, macOS 27.0, use the
ordinary bench profile (thin LTO), 30 samples per case, one second of warmup,
and three seconds of requested measurement. No Rust builds or tests ran in
parallel. `baseline-build.json` records the toolchain, source, corpus, binary,
and lockfile. The implementation is pinned to commit `978b211`; the later
package-exclusion fix does not change engine or benchmark code.

| Case | Calls | Patterns | Run 1 mean (ms) | Run 2 mean (ms) |
| --- | ---: | ---: | ---: | ---: |
| document | 4862 | 96 scanners | 61.475 | 60.689 |
| group_78 | 1545 | 125 | 37.818 | 37.372 |
| group_58 | 125 | 78 | 8.507 | 8.606 |
| group_17 | 160 | 132 | 1.476 | 1.447 |
| group_70 | 144 | 125 | 2.212 | 2.225 |
| group_61 | 61 | 76 | 1.040 | 1.060 |
| group_77 | 139 | 125 | 0.458 | 0.470 |

Group 78 takes about 37–38 ms compared with 61 ms for the complete replay.
It is the strongest first target for follow-up diagnosis. Group timings are
separate replays and cannot be added into a precise breakdown of the document.

## CPU evidence

Separate symbolized release builds keep thin LTO and add line tables without
stripping symbols. The helper warms five replays and samples the next ten
seconds of a twenty-second loop. These timings are diagnostic and must not be
mixed with ordinary Criterion timings. The build receipts record the dirty
state: only untracked evidence/summarizer files were present, with engine and
benchmark source matching the pinned implementation.

| Main-thread inclusive category | Document | Group 78 |
| --- | ---: | ---: |
| Scanner | 99.57% | 99.40% |
| Regex VM `match_at_impl` | 89.35% | 89.91% |
| Fallback search | 77.77% | 81.25% |
| OnigString construction | 0.05% | 0.22% |

Inclusive categories overlap. They identify nested hot paths, not independent
percentages to add together. The document sample has 7,937 main-thread samples;
the focused sample has 8,041. `match_at_impl` is also the dominant exclusive
symbol in both. This supports investigating VM work and fallback searches in
group 78 before changing scanner dispatch. It does not yet identify which
individual pattern is responsible or establish a proposed optimization.

## Retained artifacts

- `criterion-{1,2}.json.gz`: complete Criterion sample, estimate, benchmark,
  and Tukey data for every case; logs are retained separately.
- `{document,group-78}.json.gz`: every scanner timing and build receipt.
- `{document,group-78}.sample.txt.gz`: symbolized macOS samples.
- `cpu-summary.json`: main-thread inclusive shares and exclusive hot symbols.

Reproduce the summary with:

```sh
python3 benches/cpp_scanner/summarize-cpu.py \
  benches/cpp_scanner/results/ferroni-1.7.0/document.sample.txt.gz \
  benches/cpp_scanner/results/ferroni-1.7.0/group-78.sample.txt.gz
```

The initial exploratory run overlapped development builds and was discarded.
Use the two retained ordinary-profile runs for comparisons. All measurements
are specific to this fixture and machine; do not extrapolate them to all
languages or public API throughput.

Validation completed: full library/integration suite and doctests; full
Rust/C replay and six focused cases; formatting; all-target/all-feature
Clippy; rustdoc with warnings denied; cargo-deny; workflow pins; README theme
check; profiling helper and package-file exclusion. No runtime engine files
were changed.
