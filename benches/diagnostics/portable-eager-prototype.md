# Portable Eager scanner groups

This document records the prototype and its measurements. For the supported
`precompiled-eager` API, compatibility rules, and producer/consumer examples,
see the [prepared Eager guide](../../docs/app/routes/guide/precompiled-eager.mdx).
The legacy feature name remains an alias.

The opt-in `portable-eager-prototype` feature exports prepared scanner-group
seeks into a Ferroni-owned instruction image. A consumer loads the image and
executes it through the native kernel without reconstructing dependency-private
objects. Both producer and consumer use unmodified registry dependencies. The
default features and the ordinary precompiled-pattern format are unchanged.

The experimental methods below remain hidden. The public paired archive API
now defines the portable format compatibility contract. New exports use `FPRT003` with
compact instructions, 16-bit branch/range words, and byte planes that expose
repeated high bytes to transport compression. Loading widens directly into the
existing execution layout and runs the same structural validator. The program
binding and group records retain their four-lane corruption checks; the
consumer also accepts legacy `FPRT001` and `FPRT002` images. The ordinary program format stays
at version 2. These checks detect corruption, not producer authenticity, and
structural validation still runs. Only Eager consumes the image;
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

## Package with Zstd-19

The transport prototype uses Zstd level 19 with an explicit 8 MiB window and
frame checksums. The [codec comparison](portable-codec-levels-20261024/README.md)
explains this choice. Compression runs once on the producer; no codec dependency
is added to the Rust engine. The Node helper requires built-in Zstd support and
is tested with Node 24.22.0.

After exporting the two archives above:

```sh
node scripts/portable-eager-package.mjs pack .cache/portable-example cpp .cache/portable-packages
node scripts/portable-eager-package.mjs unpack .cache/portable-packages cpp .cache/portable-consumer
cargo run --release --locked --features portable-eager-prototype --example scanner_group_archive -- validate cpp .cache/portable-consumer portable
node --test scripts/portable-eager-package.test.mjs
```

`pack` writes `cpp.eager.zst` and `cpp.eager.json`. The decoded layout matches the
codec benchmark: a little-endian `u32` program length, followed by the ordinary
program archive and the portable group archive. The version 1 JSON manifest
records the transport profile, lengths, and SHA-256 digests of the compressed
and decoded bytes. It does not change either Rust archive's version. `unpack`
verifies both digests and the length prefix before writing the original files.
The decoder preflights a single dictionary-free frame with a known content size,
as emitted by `pack`. It checks the 8 MiB window, frame/block boundaries, and
manifest length before decoding. This explicit check also covers native
one-shot paths that can bypass their configured window limit when output fits.
The bounded decoder uses one output buffer sized from the manifest plus a spare
byte, avoiding chunk concatenation and an extra allocation at the exact output
boundary. The default payload limit is 256 MiB; module callers can supply a
different `maxPayloadBytes`. Digest and Rust archive validation still run.

For a separately downloaded `.zst` asset, serve `Content-Type: application/zstd`
without `Content-Encoding`; pass the encoded `Buffer` and manifest to the
exported `decodePackage()` helper. If an HTTP client has already decompressed a
`Content-Encoding: zstd` response, pass its decoded `Buffer` to `splitPayload()`
instead. Both return `programs` and `groups` views of the decoded buffer for the
existing cache APIs. Do not decompress an already-decoded response a second
time. The helper is Node tooling, not a browser SDK or a CDN deployment.

Obtain the manifest through a trusted channel: the hashes detect byte changes
relative to that manifest; they do not authenticate its publisher. Rust still
validates the archive structures and program/group binding. The wrapper and
manifest remain prototypes with no compatibility promise. Transport digest
checks, network transfer, and file I/O are outside the earlier codec timings.

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

## Load-phase diagnostics

The optional `scanner-load-profile` feature records nested thread-local phase
clocks for indexing, checksums, decoding, validation, helpers, and scanner/cache
construction. It is excluded from ordinary builds and final Criterion timings.
For example:

```sh
cargo run --release --locked --features portable-eager-prototype,scanner-load-profile --example scanner_group_archive -- load-phases cpp .cache/portable-example portable 20
```

`load-probe` is a short diagnostic screen with fresh scanner caches and teardown
outside the clock. Use the Criterion matrix for final comparisons. The
[load optimization report](portable-load-20261023/README.md) retains phase
measurements, rejected experiments, frozen references, and the broader results.

The comparison runner accepts `--before-artifacts` when a frozen executable
needs an older group format. Program payloads must match across artifact
directories; all combined payload hashes are checked again before timing.
