# Prepared Eager snapshots: local comparison

The prototype moves immutable Eager preparation into the archive producer while retaining the existing hybrid search engine. Across all ten captured corpora, loading takes 38.2–78.3% less time than ordinary Eager loading the existing version 2 program archive. Warm-search median changes range from -2.7% to +1.4%; no corpus regresses by 3% or more in all three repetitions. These small differences are not a claim of a new execution optimization.

The complete first document, including loading and initial searches, takes 8.9–70.7% less time. Archive transfer sizes increase substantially. This supports continuing with an optional prepared asset format; dependency support and real grammar/CDN integration are still prototype work.

## Requested metrics

Each arrow is ordinary Eager → prepared Eager. Times start from uncompressed archive bytes already in memory. Search changes pair the same process repetition; central values are medians of three Criterion mean estimates. Negative changes mean less time. RSS is the median process peak, not steady-state live heap.

| Corpus | Load ms | Warm search change | Brotli-11 KiB | Peak RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| cpp | 91.727 → 20.833 | -2.7% | 38.6 → 1460.6 | 99.7 → 111.8 |
| scss | 49.945 → 11.300 | -2.0% | 38.6 → 1091.4 | 47.7 → 51.2 |
| java | 5.387 → 1.556 | +1.1% | 11.4 → 77.0 | 12.6 → 12.8 |
| json | 0.175 → 0.108 | +1.4% | 1.6 → 4.0 | 6.7 → 6.7 |
| rust | 2.127 → 0.651 | +0.1% | 7.0 → 30.5 | 9.6 → 9.6 |
| html | 44.677 → 13.421 | +0.8% | 113.6 → 755.7 | 78.7 → 77.6 |
| astro | 74.573 → 18.181 | +0.9% | 111.1 → 1041.2 | 88.0 → 93.5 |
| tsx | 48.160 → 10.439 | -2.0% | 39.9 → 572.5 | 75.6 → 76.0 |
| c | 13.836 → 3.540 | -0.0% | 17.4 → 201.7 | 25.9 → 26.4 |
| php | 50.521 → 15.985 | +0.8% | 169.1 → 958.7 | 61.6 → 58.8 |

For C++, loading falls from 91.7 to 20.8 ms. Brotli grows from 38.6 KiB to about 1.43 MiB; peak RSS grows from 99.7 to 111.8 MiB. The retained immutable automata (23.9 MiB) and scanner scratch/caches (34.3 MiB) have exactly the same accounted sizes in all variants. Extra retained archive bytes and allocator high-water marks contribute to peak RSS. A lower measured peak in some other corpora does not establish a reduction in live memory.

All compared automata-state, covered-pattern, immutable-automata and scratch counters agree per corpus and repetition. Every exported program-helper and group record is consumed without a decode fallback.

Full timing ranges, absolute warm/cold times, gzip sizes and all three feature-control repetitions are in [measurements.md](measurements.md). Raw estimates and memory ranges are in [summary.json](summary.json). Brotli payload hashes and decoder samples are in [brotli.json](brotli.json). The [evidence bundle](evidence.tar.gz) retains source snapshots, raw samples, process logs and successful checks.

## What is prepared and loaded

- Final Thompson states retain their original variants, IDs, order, byte classes, pattern starts, look sets and empty/reverse metadata. No Thompson builder, finalizer, epsilon-closure analysis or shrinking runs on load.
- The three hybrid DFA handles load their finalized byte/start maps, quit sets and cache budgets. They share the restored forward/reverse NFAs and use the existing Eager search functions. No full determinization is added.
- Per-program leading-run/start-map/jump plans, optimizer helper flags, ASCII map ranges, leading-alternation and required-literal searchers are restored. Aho-Corasick tables keep the producer-selected DFA or contiguous-NFA representation and selected prefilter. No Aho-Corasick automaton compiler runs on load.
- `memmem` finders and packed literal-prefilter CPU dispatch are rebound locally from saved needles/small literal sets. Their cost is included in load time. Mutable transition caches, scanner scratch, ownership/index construction, validation and per-scanner assembly remain local. This is not a raw-memory or zero-copy image.
- The ordinary program format remains version 2. A separate experimental `FEGR001` sidecar binds the prepared data to its exact program checksum and native endianness. Only Eager consumes it; missing or invalid entries retain ordinary fallback behavior.

The optional feature is `prepared-eager-prototype`. The producer constructs the desired scanners in Eager, exports `export_precompiled()` and `export_group_prototype(GroupKind::PreparedEager)`, and distributes both. The consumer attaches the ordinary programs and the prepared sidecar before constructing scanners. The example command `prepare-eager` exercises this pipeline.

The snapshot APIs require local copies of the already pinned regex-automata 0.4.18 and aho-corasick 1.1.5 dependencies. Their search/compiler implementations are retained; optional safe snapshot extensions expose the private finalized structures. See [dependency notes](../../../prototype-deps/README.md) and [upstream hashes](../../../prototype-deps/UPSTREAM.json). This arrangement is for the local experiment. A production change needs upstream API support or an explicit dependency decision.

## Compression and decoding

The measured payload is a u32 program length, the unchanged version 2 program archive, and the optional sidecar. Common grammar/theme JSON and a future CDN manifest are excluded. gzip uses Python level 9 with mtime 0. Brotli uses Node 24.22.0, generic mode, quality 11. Every payload round-trips and its hash matches the Rust comparison input. Decoder results below are medians of nine calls in their respective runtimes; they are separate diagnostics, not Rust CDN end-to-end latency.

| Corpus | gzip-9 KiB, Eager → prepared | gzip decode ms, Eager → prepared | Brotli decode ms, Eager → prepared |
| --- | ---: | ---: | ---: |
| cpp | 189.3 → 3689.3 | 1.476 → 19.778 | 0.968 → 26.544 |
| scss | 82.7 → 2127.9 | 0.702 → 10.441 | 0.662 → 17.142 |
| java | 17.3 → 151.5 | 0.181 → 0.903 | 0.216 → 1.213 |
| json | 1.7 → 4.5 | 0.022 → 0.037 | 0.036 → 0.063 |
| rust | 8.3 → 50.2 | 0.054 → 0.299 | 0.093 → 0.446 |
| html | 304.3 → 1590.9 | 3.607 → 10.228 | 2.696 → 12.255 |
| astro | 331.3 → 2543.9 | 4.118 → 15.851 | 2.487 → 17.927 |
| tsx | 146.1 → 1582.8 | 2.536 → 9.984 | 1.754 → 9.629 |
| c | 25.1 → 543.9 | 0.175 → 2.876 | 0.249 → 3.376 |
| php | 331.8 → 1823.9 | 3.284 → 10.870 | 2.472 → 14.414 |

A persistent local/CDN asset cache can reuse producer work across starts and machines. An initial uncached download still pays for the larger transfer. Neither network latency nor a Rust decompression integration was measured, so the in-memory loading gain is not a claim about first-visit CDN latency.

## Measurement and validation boundary

- Host: Mac Studio, Apple M1 Ultra, 20 cores (16 performance / 4 efficiency), 64 GB, macOS arm64. rustc 1.99.0; thin LTO release/bench profiles. Exact versions, build environment, fixture hashes and executable hashes are retained in the context.
- Ten captured scanner corpora, 75,501 recorded UTF-16 calls per variant, with expected winning pattern and every capture checked before timing. These are captured scanner groups, not full grammar enumeration or held-out documents. Dynamic or unseen groups would use ordinary fallback.
- Three process repetitions per corpus for ordinary Eager, Eager with the prepared feature compiled but no sidecar (control), and the prepared candidate: 90 process cells / 270 timing cells. Variant and corpus order rotate between repetitions.
- Criterion: 40 samples, 300 ms warmup, 1 s target per metric, 10,000 bootstrap resamples. Search follows three warm full-document passes. Fresh input-string identities are prepared outside the search timing. Load excludes teardown; cold document includes load and the first complete replay.
- Memory uses one fresh child under a fresh Python wrapper, reading Darwin `RUSAGE_CHILDREN.ru_maxrss` bytes after fixture parsing, archive retention, scanner construction and four document passes. Full raw process peaks/ranges and partial automata/scratch counters are retained.
- Time Machine was not running before measurement. CPU placement, thermals and unrelated host background work were not controlled. Retained paired ranges expose noise, including individual HTML/PHP outliers. No >=3% persistent warm-search regression appears; small median differences remain.
- Local validation passed: 567 library tests; the focused prefilter differential, existing precompiled and scanner regression suites; four snapshot tests including representation preservation, truncated-input rejection, captures, duplicates, mode isolation and fallback. One external grammar differential test remains ignored. fmt, diff checks and all-target/all-feature clippy passed. Vendored upstream deprecation/lifetime warnings and existing C warnings remain in logs.
- No CI, cross-platform wire validation, complete Ferriki grammar generation, standalone Regex archive API, actual CDN integration or production compatibility promise is included.

## Reproduce

```sh
python3 scripts/compare-scanner-group-archives.py .cache/prepared-eager-repro/run \
  --cache .cache/prepared-eager-repro --variants eager control prepared
python3 scripts/report-scanner-group-archives.py .cache/prepared-eager-repro/run \
  .cache/prepared-eager-repro/report
node scripts/compress-scanner-group-archives.mjs .cache/prepared-eager-repro/artifacts \
  .cache/prepared-eager-repro/brotli.json eager prepared
cargo test --offline --features prepared-eager-prototype,prefilter-self-check \
  --test scanner_group_prototype
cargo fmt --all --check
cargo clippy --offline --workspace --all-targets --all-features -- -D warnings
```

The measured source snapshot is retained. After measurement, only the reporting script and a focused test changed: the report now summarizes all three control repetitions, and the test additionally compares restored behavior with ordinary Eager loading version 2 programs. The library, benchmark sources, generated payloads and measured executables remain unchanged.
