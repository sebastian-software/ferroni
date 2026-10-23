# Scanner-group archive prototypes — October 23, 2026

This local experiment compares the current Eager scanner with archived group
NFAs and complete group DFAs. It covers all ten captured scanner corpora.
For execution-focused use, retain the current Eager implementation. Neither
candidate produced a consistent 3% warm-search gain on any captured corpus.

The isolated NFA archive reduces loading by 18.3–51.6% and the cold document
boundary by 6.6–47.7%. Its warm-search medians are 0.3–4.2% slower; the retained
per-run ranges distinguish small drift from stronger results. This is a startup
tradeoff, not a throughput improvement. It remains an optional direction if
startup matters enough to justify larger assets, but is not a production merge
recommendation from this experiment.

Complete Dense DFAs make warm search 1.6–11.9% slower in the medians. Loading
varies from a 51.0% reduction to a 574.1% increase. Complete Sparse DFAs were
worse on several corpora, including warm-search regressions near 31% for Java,
61% for Rust, and 84% for C. Dense improves that representation problem but
does not establish a benefit over the existing hybrid engine.

C++ illustrates the NFA tradeoff: load falls from about 92.6 ms to 49.5 ms,
peak RSS rises from 101.8 MiB to 115.7 MiB, and gzip grows from 189.3 KiB to
3,729.0 KiB. The additional RAM is modest on this host. CDN bytes and the lack
of a search-speed benefit are the stronger constraints. Conversely, the full
Dense C candidate reaches 596.1 MiB peak RSS versus 25.4 MiB for Eager, with
slower loading and searching. The original 24 MiB observation counted only
C++ immutable automata; peak RSS additionally includes scratch, programs,
archive bytes, decoding temporaries, fixtures, and allocator high-water marks.

[Final measurements, all corpora](measurements.md) contain paired ratio ranges,
absolute times, gzip, memory, and the feature control. [Summary data](summary.json)
retain the underlying estimates. [Initial Sparse matrix](sparse-measurements.md)
records the exploratory representation comparison. [Archive sizes, decompression, memory counters, and build coverage](archives-and-memory.md)
include the full Brotli comparison. [Brotli samples](brotli.json) retain all
40 payload hashes and round-trip timings. The [verified evidence bundle](evidence.tar.gz)
contains both source snapshots, raw Criterion samples, process memory,
validation logs, and a SHA-256 manifest.

Brotli does not remove the transfer tradeoff: C++ is 38.6 KiB for Eager,
1,453.5 KiB with NFAs, and 1,476.7 KiB with Dense tables. No CDN transfer is
included in the loading percentages above. The conclusions apply to these
representations, budgets, and captured workloads.

## Measurement contract

The baseline enables `precompiled` and constructs scanners in Eager mode from
an in-memory version 2 program archive. The candidates load the same program
bytes plus an indexed group archive. Every lane constructs every scanner in
the captured trace, including duplicate groups sharing immutable automata.
This is a captured-workload comparison, not complete grammar enumeration or
Ferriki end-to-end highlighting. Unseen groups retain runtime construction.

- **Load:** open indexes, verify checksums, decode and structurally validate
  requested programs and groups, initialize Eager helpers and scanner scratch.
  Input I/O, decompression, string preparation, and teardown are excluded.
- **Search:** three full warmup passes followed by the complete captured UTF-16
  calls and captures. Each timed document gets fresh `OnigString` identities
  prepared outside timing, following the existing replay boundary.
- **Cold document:** load plus the first complete replay. String preparation
  and teardown remain outside timing.
- **Memory:** one fresh driver child per corpus/variant/repetition. A separate
  Python wrapper reads `resource.getrusage(RUSAGE_CHILDREN).ru_maxrss` in bytes
  on Darwin. Peak RSS includes fixture parsing, archive bytes/indexes, decoded
  programs, all scanners, and four search passes. Immutable automata and
  per-scanner hybrid-cache counters are also retained. They are partial heap
  accounting, not total process memory.
- **Archive size:** one little-endian u32 program length, the ordinary program
  archive, and optional group bytes as a single payload. Common grammar/theme
  assets are excluded. gzip uses level 9 with a fixed timestamp. Brotli uses
  quality 11, generic mode. Compressed-byte counts are deterministic; no
  network request is timed.
- **Decompression:** gzip/zlib in Python and Brotli in Node, reported separately
  from Rust loading. These measurements do not constitute a Rust CDN loader.

Three process repetitions rotate variant and corpus order. Criterion retains
40 samples, a 300 ms warmup, a one-second target measurement window, and
10,000 bootstrap resamples per cell. Central values are medians of the three
Criterion mean estimates. Changes pair candidate and Eager within each
repetition; ranges retain all three changes. One additional Eager run with
the full-DFA feature compiled diagnoses feature dispatch/layout effects.

The host is a Mac Studio with an Apple M1 Ultra, 20 cores (16 performance,
4 efficiency), and 64 GB RAM, running macOS on arm64. The compiler is Rust
1.99.0; release/bench profiles use thin LTO. `regex-automata` is locked to
0.4.18. Background activity, core placement, and thermals were not controlled.
The evidence is local and exploratory; marginal changes need confirmation.

## Prototype boundaries

The production program archive is unchanged. A separate `FGRP002` experiment
stores ordered group keys as stable IDs into the sorted program-key index.
It is bound to the exact program archive checksum. Keys preserve duplicates,
pattern order, compile options, syntax, and compile environment. Warmup does
not change a program key. Only Eager consumes these group records; Auto and
Off retain their existing preparation behavior.

The NFA payload stores portable Thompson states, starts, pattern IDs, byte
ranges, unions, and look assertions. The public builder reconstructs checked
immutable NFAs and hybrid DFA handles. It avoids HIR decoding and Thompson
compilation, but still performs bounded reconstruction, validation, and cache
allocation. Mutable transition caches and input memos stay process-local.

The complete-DFA experiment stores three automata per group: a leftmost-first
forward search, an all-match reverse search, and an anchored all-match walk.
Each dense build has a 16 MiB DFA budget and a 64 MiB determinization-workspace
budget. A group uses prepared NFAs if any required build exceeds its budget.
The stored full-DFA representation is selected at build time:

- `scanner-group-nfa-prototype`: NFA archives without a full-DFA branch in the
  normal search path.
- `scanner-group-prototype`: complete Sparse DFAs with NFA fallback.
- `scanner-group-dense-prototype`: complete Dense DFAs with NFA fallback.

Full tables use the dependency's safe validated deserializers. Dense data is
copied into aligned temporary bytes before validation and ownership conversion;
these copies are included in loading and RSS. No new unsafe code or dependency
is introduced. Full-DFA records have native byte order; the experimental
container conservatively requires matching endianness. No cross-platform
performance or wire-compatibility promise follows from this experiment.

The full-DFA prototype retains tiny unused hybrid handles/caches to avoid
rewriting the production prefilter. Their costs remain in the measurements.
The public API and group wire format behind these features are experimental.

## Reproduce

Use fresh output directories. The cache keeps generated archives and binaries.
Both runners validate every recorded winning pattern and every capture before
any timing. The harness asserts that every archived group is consumed without
a silent decode fallback.

```sh
python3 scripts/compare-scanner-group-archives.py \
  .cache/group-repro/sparse --cache .cache/group-repro

python3 scripts/compare-scanner-group-archives.py \
  .cache/group-repro/dense --cache .cache/group-repro \
  --reuse-artifacts --prepare-dense --variants eager nfa dense

python3 scripts/report-scanner-group-archives.py \
  .cache/group-repro/dense .cache/group-repro/dense/report

node scripts/compress-scanner-group-archives.mjs \
  .cache/group-repro/artifacts .cache/group-repro/brotli.json
```

For the exact initial code state, overlay its source snapshot on the recorded
base commit in a separate checkout. The current runner uses isolated NFA
support in either matrix.

The initial exploratory matrix compiled the full-DFA feature for both the NFA
and Sparse candidates. The follow-up isolates NFA loading from full-DFA runtime
support and substitutes Dense tables. The reused ordinary-program and NFA
payloads are asserted byte-identical when preparing the follow-up.

Each matrix retains fixture hashes, source hashes, binary hashes, exact
boundaries, validation, preparation coverage, raw sizes, memory rows, and
Criterion estimates/samples. Source snapshots preserve the two code states.
The generated automata blobs remain local build products; their hashes and
reproduction inputs are retained instead of committing large binary payloads.

## Validation and interpretation

Every one of the 75,501 captured calls is checked for its winning pattern and
all capture ranges in each lane before timing. Missing groups, changed pattern
order/duplicates, captures, look assertions, invalid/truncated archives,
checksum corruption, and Auto/Off behavior have focused round-trip coverage.
The library and existing prefilter/archive regressions pass with
`prefilter-self-check` enabled. The prototype coverage assertion confirms that
valid archived groups are decoded rather than silently rebuilt at runtime.

Executed checks:

```sh
cargo fmt --all --check
cargo clippy --offline --workspace --all-targets --all-features -- -D warnings
cargo clippy --offline --features scanner-group-nfa-prototype --all-targets -- -D warnings
cargo test --offline --features scanner-group-nfa-prototype --test scanner_group_prototype
cargo test --offline --features scanner-group-prototype --test scanner_group_prototype
cargo test --offline --features scanner-group-dense-prototype --test scanner_group_prototype
cargo test --offline --features scanner-group-dense-prototype,prefilter-self-check \
  --lib --test compat_prefilter_differential --test scanner_prefilter_regressions \
  --test precompiled_scanner --test scanner_group_prototype -- --test-threads=1
```

The ignored external Ferriki grammar-corpus test was not run. The captured
corpora are the comparison's scope. The experiment does not establish complete
grammar coverage, behavior on new input families, or CDN first-visit latency.
A Ferriki integration would still need asset production/loading and a policy
for missing, dynamically resolved, and unobserved scanner groups.

This result supports keeping full determinization out of the default archive.
For a future startup-focused NFA extension, reduce duplicated group data and
confirm the small warm-search differences before proposing adoption. Pure
execution optimization should continue from the current Eager engine instead.
