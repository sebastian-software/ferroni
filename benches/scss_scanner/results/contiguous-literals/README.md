# Contiguous literal alternatives: compiler experiment

The SCSS tag, property and value patterns mix hundreds of literals with a
few character classes, optional suffixes or nested groups. The complete
alternation therefore misses the existing pure-literal trie optimization.

This experiment compacts only **consecutive plain literal branches** in
place, with at least 16 literals and at most 8,192 per trie. It reuses the
existing literal/folded trie compiler and VM. Non-literal branches, capture
groups, scoped option boundaries, Unicode folding and alternation order keep
their existing paths. It adds no opcode, public API, dependency or unsafe code.
The pass remains in the existing literal-alternation compiler section.
No original expression or grammar asset changes.

Nested path extraction remains excluded: the existing optional extractor
does not preserve every lazy/greedy priority or internal capture. Moving
literal branches ahead of non-literals would also be incorrect. Grouping
adjacent literal branches preserves the complete ordered choice instead.

## Initial scanner result

The ordinary thin-LTO Criterion build, toolchain, corpus, warmup and sample
settings match the retained [baseline](../ferroni-1.7.0/README.md). Two full
optimized runs put the SCSS document replay at **3.35 / 3.51 ms**, compared
with **25.77 / 26.12 ms** before, about 86–87% less scanner time. A later
baseline/optimized pair gives **30.88 / 3.49 ms**. The baseline varies;
all runs are retained, and this is a fixture-specific scanner result.
These timings do not establish a public Ferriki HTML/token speedup.

All 3,744 SCSS and 4,862 C++ replay calls retain exact matched indices and
captures, independently validated against pinned C Oniguruma before timing.
The full all-features test suite passes with the prescribed debug stack.
New differential tests compare mixed alternatives against unoptimized
equivalent expressions across captures, backreferences, repetition,
atomic repetition, forward/backward searches, Unicode folds and malformed
UTF-8. Lookaround bodies and scoped option boundaries retain their guards.

The implementation census changes intentionally: TypeScript gains three
literal tries and three outer byte-set guards; CSS gains 15 folded tries,
reduces internal byte-set guards from 2,764 to 1,275, and changes one entry
from table dispatch to fallback search through the existing conservative
folded-trie length summary. Rust's census stays unchanged. The behavior
suites and full captured C oracle remain unchanged.

## Checks still required for acceptance

Measure the complete Ferriki HTML/token pipeline on the curated 20 formats
and inspect compilation/setup costs and regressions. Keep all runs and
repeats, including unstable tails. C++ declaration/fallback search remains
a separate target; its cross-reference diagnosis is retained in the baseline.
