# Portable Eager scanner groups

The opt-in `portable-eager-prototype` feature exports prepared scanner-group
seeks into a Ferroni-owned instruction image. A consumer loads the image and
executes it through the native kernel without reconstructing dependency-private
objects. Both producer and consumer use unmodified registry dependencies. The
default features and the ordinary precompiled-pattern format are unchanged.

This is an intermediate prototype. Its API is hidden from the default docs and
its group format has no compatibility promise. Only Eager consumes the image;
Auto and Off keep their existing behavior. Per-pattern literal helpers still
use ordinary Eager preparation, and mutable transition caches warm at runtime.

## Use the prototype

Build the desired scanners in Eager mode through one `ScannerPatternCache`.
Export `export_precompiled()` and
`export_group_prototype(GroupKind::Portable)` from that same cache. In a fresh
cache, attach the ordinary programs with `add_precompiled()` first, then attach
the group bytes with `add_group_prototype()`. Construct the scanners in Eager
mode through that cache. Missing or invalid individual groups retain normal
Eager construction as a fallback.

The local driver and Criterion harness use the existing captured scanner
fixtures. For example, from the repository root:

```sh
cargo run --release --locked --features portable-eager-prototype --example scanner_group_archive -- prepare-portable cpp .cache/portable-example
cargo run --release --locked --features portable-eager-prototype --example scanner_group_archive -- validate cpp .cache/portable-example portable
GROUP_ARCHIVE_CORPUS=cpp GROUP_ARCHIVE_VARIANT=portable GROUP_ARCHIVE_DIR="$PWD/.cache/portable-example" GROUP_ARCHIVE_OUTPUT="$PWD/.cache/portable-example/criterion" cargo bench --locked --features portable-eager-prototype --bench scanner_group_archive_bench
```

Change `GROUP_ARCHIVE_VARIANT` to `eager` to compare ordinary Eager using the same
program archive. The diagnostics runner retains raw samples, capture checks,
compression sizes, and fresh-process memory measurements.

## Evidence and scope

The [optimized-kernel comparison](portable-kernel-20261023/README.md) measures ten
corpora with three process repetitions. In that measured source snapshot, load
time falls by 27.1–78.9% against Eager and warm search takes 1.2–7.8% more time
(median paired changes). The report includes the frozen Snapshot comparator,
the original portable kernel, and a same-binary Eager control. All 75,501
captured calls agree, including winning patterns and captures.

The dated reports and evidence archives describe their exact measured source,
not every later edit on this branch. Their historical Snapshot comparator used
local dependency extensions. Those extensions and Snapshot-only source are
absent from the current build; their original sources remain in the evidence
archives so the comparison is still auditable. The PR cleanup removes that
comparator and its unused program-helper hooks, rather than claiming a new
timing run. The portable instruction kernel itself is unchanged.

The packages remain larger than ordinary program archives and Snapshot images.
Rust load timing excludes decompression, file I/O, and network transfer. Peak
RSS includes retained image bytes, decoded structures, scanners, and fixture
parsing; there is no general memory reduction. This is local diagnostic
evidence, not a published cross-platform speed claim.

The next performance work should split loading into validation, indexing,
decoding, literal-helper preparation, and scanner/cache construction. Prioritize
absolute time in the large corpora: the measured C++ load falls from 91.5 to
24.7 ms, while JSON falls from 0.173 to 0.123 ms. The smaller percentage gain in
JSON represents about 50 microseconds, so the percentage range alone does not
identify the best optimization target. Decompression should also be measured
with Rust load in the eventual application path.
