# ADR-006: SIMD-Accelerated Search via memchr

## Status

Accepted

## Context

ADR-001 establishes 1:1 structural parity with C Oniguruma. However, the C original's forward search pipeline uses hand-written byte-by-byte loops for literal prefix scanning (`forward_search_range` in `regexec.c`). Modern CPUs provide SIMD instructions (SSE2/AVX2 on x86-64, NEON on aarch64) that can scan memory significantly faster, but C Oniguruma does not use them.

The question is whether to deviate from the C original's search implementation for a performance improvement.

## Decision

Replace the hand-written byte scan loops in the search pipeline with the `memchr` crate, which provides SIMD-vectorized implementations of single-byte, two-byte, and three-byte search.

Specifically:
- **1-byte literal prefix:** `memchr::memchr` (replaces byte-by-byte scan)
- **2-byte set:** `memchr::memchr2` (replaces manual two-candidate loop)
- **3-byte set:** `memchr::memchr3` (replaces manual three-candidate loop)
- **Boyer-Moore-Horspool (BMH):** Kept as-is from C (already efficient for multi-byte patterns)
- **Map search (>3 distinct bytes):** Kept as-is from C (256-entry lookup table)

The structural shape of the search pipeline is unchanged -- the same functions exist, the same decision logic selects the strategy, and the same fallbacks apply. Only the inner byte-scanning loops are replaced.

## Rationale

- **Measurable impact:** The largest gains appear in full-text no-match scenarios where the engine must scan the entire haystack:
  - 10 KB no-match: 381 ns vs 1.9 us (5x faster)
  - 50 KB no-match: 1.5 us vs 9.3 us (6x faster)
- **Minimal code change:** The `memchr` crate is a single dependency with zero transitive dependencies and zero `unsafe` in its public API.
- **No behavioral change:** The SIMD paths find the same bytes at the same positions. All existing tests pass unchanged.
- **This is the kind of deviation ADR-001 allows:** The control flow and API are identical; only the inner scan loop implementation differs, similar to how a C compiler might auto-vectorize a loop.

## Consequences

- The `memchr` crate is a build dependency.
- Benchmarks for literal and RegSet search are 20-60% faster than C. No-match full-text scans are 5-6x faster.
- The Map search path (>3 distinct first bytes) still uses a 256-entry byte map, same as C. SIMD dispatch only covers 1-3 byte sets.
- Thin LTO (`lto = "thin"` in release profile) is enabled to allow cross-crate inlining of `memchr` calls without the compile-time cost of full LTO.
