# Fork-free portable Eager prototype

The portable scanner-group image loads with unmodified dependencies and reproduces all 75,501 recorded calls, including winning patterns and captures. This implementation is **not a merge candidate**: load time falls by 28.3–80.3% versus ordinary Eager, but warm search takes 25.7–141.0% more time (median paired change per corpus). A search regression of at least 3% occurs in every repetition for 10 of ten corpora (cpp, scss, java, json, rust, html, astro, tsx, c, php).

The [complete three-way comparison](comparison.md) covers load time, compressed size, fresh-process peak RSS, warm search, and the first document. [Paired ranges, the feature control, and raw per-process estimates](measurements.md) retain the variance. [summary.json](summary.json) contains all values, and [evidence.tar.gz](evidence.tar.gz) contains source overrides, provenance, raw samples, and successful checks.

## What runs without forks

The producer exports scanner seeks through public Thompson NFA APIs into Ferroni-owned instructions. `FPRT001` stores little-endian 32-bit instruction operands, byte classes, ordered branches, ASCII assertion IDs, match IDs, and forward/reverse entry points. It contains no Rust pointers, Arc internals, vtables, mutable caches, or machine code.

The consumer validates instruction tags, references, ranges, equivalence classes, lengths, and group identity. It decodes immutable arrays directly and executes them with the [native interpreter](../../../src/dfa_prefilter/portable.rs). Ordered epsilon closure and bounded lazy transition caches implement forward leftmost-first search, reverse start discovery, and overlapping candidate walks. The existing Ferroni VM still decides matches and captures, including constructs approximated by the scanner seek.

This is a scoped scanner-group prototype. Per-pattern literal helpers and search plans still use ordinary Eager preparation. The old snapshot also stores those helpers. Small unused compatibility handles/caches remain in the scanner wrapper and are included in the measurements. The custom runtime's transition caches start empty, as ordinary Eager's hybrid caches do; this is not complete ahead-of-time determinization. Off and Auto ignore the image. Missing or invalid individual groups retain the existing Eager fallback.

The isolated producer **and** consumer resolve regex-automata 0.4.18 and aho-corasick 1.1.5 from crates.io, with checksums in Cargo.lock. No snapshot feature or path patch is active there. The root manifest retains the previous snapshot experiment solely for the historical comparator and existing checks. The [build script](../../../scripts/prepare-portable-eager-comparison.py) removes those patches and snapshot-only feature edges in an isolated source copy, verifies Cargo metadata, and records hashes. The snapshot executable is reused byte-for-byte from the [previous experiment](../prepared-eager-20261023/README.md).

## Measurement contract

- Ten captured grammar corpora: C++, SCSS, Java, JSON, Rust, HTML, Astro, TSX, C, and PHP. Each candidate reproduces all recorded UTF-16 calls and capture arrays before timing and inside each fresh benchmark process.
- Three process repetitions, rotating corpus and variant order. Ordinary Eager and the portable feature control use registry builds; the existing prepared snapshot uses its frozen executable. The control loads the ordinary archive through the portable-feature executable, without a group image.
- Criterion: 40 samples, 300 ms warmup, 1 s target, and 10,000 bootstrap resamples. Load decodes in-memory uncompressed bytes and creates requested scanners and scratch; file I/O, codecs, strings, and teardown are excluded. Warm search follows three document passes and uses fresh string identities prepared outside timing. Cold document includes load and the first replay.
- Sizes include a four-byte program-length prefix, the ordinary program archive, and any group image. gzip uses level 9; Brotli uses quality 11. Both codecs verify round trips and payload SHA-256. Codec timings are separate diagnostics; the comparison includes an explicitly labeled decode-plus-load estimate.
- RSS uses a fresh wrapper and one driver child for each corpus/variant/repetition. It includes fixtures, retained input archives, decoded programs, scanners, construction high-water marks, and four search passes. Internal counters are partial estimates and do not replace RSS.
- macOS arm64 on an M1 Ultra Mac Studio with 64 GB, rustc 1.99.0, release/bench ThinLTO. Time Machine was idle. Core placement, thermals, and unrelated background activity were not controlled. No compilation or compression ran alongside the main Rust timing matrix.

## Interpretation and remaining work

The architecture works without upstream serialization APIs. An independent format also makes Ferroni responsible for a fast interpreter and its correctness. This prototype uses per-state transition allocations and more checks in its byte loops than the existing hybrid implementation; the search regression is real in this matrix and does not preserve ordinary Eager search performance. The native feature control in the measurements separates part of the build/layout effect from the image execution path. The frozen snapshot's small search differences remain subject to process and build variation.

The raw image and decoded arrays are both retained, so smaller mutable caches do not imply a lower process peak. Compressed images are also larger than ordinary program archives. CDN transfer, codec cost, and a local download cache must be considered alongside the in-memory load gain.

The next focused experiment would flatten transition storage and tighten the byte loop, then repeat this matrix. Preparing per-program literal helpers without dependency-private objects is a separate missing part. Keeping the existing hybrid runtime with the older public-API NFA archive is another available no-fork tradeoff; its [earlier measurements](../scanner-group-archives-20261023/README.md) should not be confused with this new interpreter.

The wire version is experimental. Cross-platform execution, complete grammar enumeration, the actual Ferriki startup path, authenticated artifact distribution, and adversarial resource behavior have not been validated. The existing record checksum detects accidental damage; it is not authenticity verification. No new unsafe code was added.

## Validation and reproduction

PASSED in the isolated registry source:

```bash
cargo test --offline --release --features portable-eager-prototype,prefilter-self-check --lib --test scanner_group_prototype --test compat_prefilter_differential --test precompiled_scanner --test scanner_prefilter_regressions -- --test-threads=1
cargo clippy --offline --all-targets --features portable-eager-prototype -- -D warnings
```

The tests report 568 unit tests, three compatibility differential tests (one external-corpus test ignored), two precompiled scanner tests, three archive tests, and one scanner regression test. They include deterministic unseen inputs and spans, full candidate-set parity, small-cache state-ID reuse, truncation rejection, order/duplicates/captures, fallback, Auto/Off, and re-export after execution.

PASSED in the source checkout: `cargo fmt --all --check`, `git diff --check`, and `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings`. The latter retains the historical snapshot dependencies; its upstream Rust/C warnings are preserved in the check log. No CI run or cross-platform claim is implied.

With the previous experiment's local cache available, build the isolated source and immutable reference binaries using:

```bash
python3 scripts/prepare-portable-eager-comparison.py .cache/portable-eager-20261023 --snapshot-cache .cache/prepared-eager-20261023
```

Export each corpus with the isolated `bin/portable prepare-portable CORPUS ARTIFACT_DIR`, retaining identical ordinary program bytes. Then validate/prepare and run the matrix:

```bash
python3 scripts/compare-scanner-group-archives.py .cache/portable-eager-20261023/run-01 --cache .cache/portable-eager-20261023 --prebuilt --reuse-artifacts --prepare-only --variants eager prepared portable control
node scripts/compress-scanner-group-archives.mjs .cache/portable-eager-20261023/artifacts .cache/portable-eager-20261023/brotli.json eager prepared portable
python3 scripts/compare-scanner-group-archives.py .cache/portable-eager-20261023/run-01 --cache .cache/portable-eager-20261023 --measure-only --variants eager prepared portable control --repetitions 3
python3 scripts/report-portable-eager.py .cache/portable-eager-20261023/run-01 .cache/portable-eager-20261023/brotli.json benches/diagnostics/portable-eager-20261023
```

Use a fresh output/cache directory, or the build script's explicit `--refresh` for its own isolated build directory. The evidence retains the transformed manifest/lock and measured source overrides against base commit 4a6679e6df2cca2ac28238f2a8335b731d8b3d32. The previous snapshot's source and binary hashes are retained separately.
