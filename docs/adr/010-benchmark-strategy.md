# ADR-010: Benchmark Strategy

## Status

Accepted

## Context

Ferroni's performance claims need reproducible evidence. The question is what to benchmark, how to measure, and how to prevent regressions.

## Decision

A two-tier benchmark architecture:

### Tier 1: README-facing benchmarks

Real-world scenarios that produce the numbers we publish. Always Ferroni vs C Oniguruma at `-O3`.

| Category | What is measured |
|----------|-----------------|
| Syntax highlighting | Full, unmodified Shiki grammars -- TypeScript (279 patterns), CSS (117 patterns), Rust (81 patterns). Compile time, first-match latency, full-line tokenization. |
| Text search | Literal search, no-match rejection, field extraction, timestamp matching on 10-50 KB log inputs. |
| Pattern matching | One representative pattern per regex feature (quantifiers, lookaround, Unicode, backreferences, alternation, named captures). |
| Compilation | Simple to complex patterns, measuring compile latency. |

Key rule: **benchmark against complete, unmodified production grammars** -- no cherry-picked subsets. The Shiki grammars are committed as-is in `benches/grammars/`.

### Tier 2: Regression tracking

Per-feature micro-benchmarks tracked by [CodSpeed](https://codspeed.io/) in CI. These catch performance regressions before they reach `main`. Not published in the README -- they measure internal implementation details, not user-facing workloads.

### Tooling

- **Criterion.rs** for local measurement and HTML reports (`target/criterion/report/index.html`).
- **codspeed-criterion-compat** for CI integration -- same benchmark code, instrumented for CodSpeed's wall-time tracking.
- **C comparison** via optional `ffi` feature. The `cc` crate builds C Oniguruma from source in `oniguruma-orig/` for head-to-head measurement.

### Build profile

Both `release` and `bench` profiles use `lto = "thin"` to allow cross-crate inlining (especially for `memchr`) without the compile-time cost of full LTO. This matches realistic deployment conditions.

## Rationale

- **Real grammars prevent overfitting.** Benchmarking against subsets risks optimizing for patterns that don't matter.
- **C comparison keeps claims honest.** Every speedup number is relative to the same engine at `-O3`, not a strawman.
- **Two tiers separate concerns.** Tier 1 numbers are stable and publishable; Tier 2 catches implementation-level regressions without cluttering the README.
- **Compilation is part of the workload.** Syntax highlighters compile grammars at startup. Ignoring compile time gives an incomplete picture.

## Consequences

- Shiki grammar JSON files are committed to the repository (`benches/grammars/`). These are updated when Shiki releases new grammar versions.
- The `ffi` feature adds a C build step. Running `cargo bench --features ffi` requires a C compiler; `cargo bench` (without `ffi`) runs Ferroni-only benchmarks.
- Tier 1 benchmark results are documented in `docs/perf/benchmark-results.md` and summarized in `README.md`.
- New optimizations must include Tier 1 benchmark numbers in their commit or PR description (per ADR-008).
