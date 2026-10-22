# Precompiled scanner construction: local diagnostic

The optional `precompiled` feature reduces fresh-cache construction time on all
three captured Ferriki scanner workloads in this local comparison. This is a
construction measurement, not CDN download time or full highlighting latency.

| Corpus | Compile, runs 1 / 2 | Archive, runs 1 / 2 |  Reduction |
| ------ | ------------------: | ------------------: | ---------: |
| C++    |    31.31 / 33.35 ms |    14.07 / 13.92 ms | 55.1–58.3% |
| SCSS   |    10.89 / 10.60 ms |      4.23 / 4.18 ms | 60.5–61.1% |
| Java   |      1.39 / 1.41 ms |      1.22 / 1.21 ms | 12.7–13.8% |

Each estimate is Criterion's slope estimate where available, otherwise its mean.
[Run 1](run-1.json) and [run 2](run-2.json) retain all raw sample times, iteration
counts, estimates, and confidence intervals. C++ compiler construction drifted
between repetitions; use the observed range rather than one headline ratio.
The second run measured archive lanes before compiler lanes, reversing the
first run's per-corpus order. These short local runs support a diagnostic
comparison, not a portable performance guarantee.

Both lanes build every scanner list in the same committed trace with one fresh
pattern cache per iteration. The archive lane also opens the complete index,
validates and decodes requested programs, rebuilds process-local accelerators,
assembles scanners, and drops the scanners, decoded programs, and index. The
input bytes are shared in an `Arc`; filesystem I/O, downloads, export, JSON
parsing, and fixture loading are outside the measurement. Lazy DFA automata
are built by subsequent searches, outside both construction lanes.

The archives contain 250 C++ programs (1,678,334 bytes), 104 SCSS programs
(763,589 bytes), and 115 Java programs (293,333 bytes). The benchmark checks
that every requested pattern can actually decode before timing, so a silent
compiler fallback cannot explain a result. Separately,
`tests/precompiled_scanner.rs` replays all 12,526 captured UTF-16 calls and
checks their pattern indices and captures, with prefilter self-check enabled.

Environment: Mac Studio, Apple M1 Ultra (20 cores), macOS 27.0.1, arm64,
Rust 1.99.0, bench profile with thin LTO, default DFA feature plus `precompiled`.
The measured tree is the issue #444 implementation on top of
`727497bfdfaf10fb3f951a35d5b4cdb0fcfa28a2`; it includes the uncommitted feature
implementation delivered with these results.

```sh
cargo bench --features precompiled --bench scanner_compile_bench -- \
  '(_pattern_cache|_precompiled)$' \
  --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot

# Second repetition, with the archive lanes first:
cargo bench --features precompiled --bench scanner_compile_bench -- \
  '_precompiled$' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench --features precompiled --bench scanner_compile_bench -- \
  '_pattern_cache$' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
```
