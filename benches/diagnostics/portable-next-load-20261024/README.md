# Load follow-up after PR #458

PR #458 merged as `a00cf155eff90547617a477c59e5a9e7baaac0c7` after all 33 checks passed. This follow-up implements the two mechanisms measured below. The original prototype evidence remains frozen; final-source verification is recorded separately in the integration section.

Two opportunities have measured gains: a sized Zstd output buffer with an explicit frame preflight, and direct string serialization in the Rust archive codec. The former reduces verified Node decode time by 8.8% for this ten-package mix; the latter reduces in-memory Rust load by 4.4% for C++ and 3.4% for C, meeting the 3% threshold in each of three processes for those two corpora. The boundaries are measured separately.

## Verified Zstd package decode

The original helper decodes through Node's default 16 KiB output chunks. Its synchronous adapter accumulates chunks and concatenates the results. A buffer of the declared payload size plus one byte avoids that concatenation and the extra allocation made when an output chunk fills exactly. The existing payload limit and all digest, prefix, and output checks remain.

An unguarded sized-buffer candidate failed the oversized-window test: with Node 24.22.0 and Zstd 1.5.7, the one-shot path can accept a large frame window despite `ZSTD_d_windowLogMax=23`. That candidate is rejected. The retained candidate explicitly checks the frame window, known content size, dictionary-free header, block extents, and single-frame boundary before decoding. Its transport subset matches the existing producer. The frame preflight follows [RFC 8878](https://www.rfc-editor.org/rfc/rfc8878.html#section-3.1.1.1). It does not decode compressed instructions or replace libzstd's content validation.

The guarded candidate passes nine tests, including a tiny-content frame with an oversized window that the native one-shot decoder accepts, plus corruption, truncation, alternate size fields, output bounds, trailing/concatenated frames, and CLI checks. Every timed output also matches both original archives byte-for-byte.

Three fresh processes per corpus and variant, three warmups and 15 timed calls each. Variant and corpus order rotate. This includes `decodePackage()` digest checks, bounded decompression, frame preflight in the candidate, manifest/prefix validation, and splitting. Input I/O and full output comparisons are outside the clock.

| Corpus | Default ms | Guarded sized ms | Paired change | Three-process range | Peak RSS MiB, default → sized |
| --- | ---: | ---: | ---: | ---: | ---: |
| cpp | 39.651 | 35.908 | -9.9% | -10.7% to -9.3% | 253.9 → 221.5 |
| scss | 23.316 | 21.519 | -7.0% | -7.8% to -7.0% | 198.4 → 177.3 |
| java | 1.702 | 1.517 | -9.0% | -11.1% to -6.0% | 77.8 → 69.1 |
| json | 0.090 | 0.077 | -14.1% | -22.5% to -9.6% | 49.2 → 48.6 |
| rust | 0.543 | 0.496 | -8.7% | -11.9% to -2.7% | 60.5 → 54.0 |
| html | 17.618 | 16.070 | -8.8% | -8.8% to -8.5% | 182.4 → 173.3 |
| astro | 28.397 | 25.245 | -11.1% | -11.4% to -9.8% | 246.7 → 207.6 |
| tsx | 17.822 | 16.114 | -9.1% | -10.6% to -8.3% | 183.6 → 169.2 |
| c | 5.288 | 5.215 | -4.0% | -4.8% to -0.2% | 125.9 → 120.5 |
| php | 18.324 | 17.002 | -7.4% | -7.6% to -7.2% | 187.0 → 168.6 |

The paired sums of per-corpus process means fall by 8.8% (range -9.25% to -8.75%). This is an unweighted package mix, not application startup time. Eight corpora gain at least 3% in all three processes; Rust and C do not meet that floor. The tiny JSON times are especially sensitive to runtime overhead.

RSS is a process high-water mark including source inputs, decoded buffers, repeated calls, allocator retention, and garbage collection. This is Node transport memory; Rust runtime memory was not remeasured. Encoded assets, manifests, and raw archive bytes are unchanged. Browser content decoding and Ferriki startup are not measured.

## Direct Rust string writes

The baseline writer clones a string into a temporary byte vector, then writes its bytes one at a time. The candidate writes the same checked byte length followed by `extend_from_slice()`. Pattern-key construction still creates its owned string; eliminating that remaining copy is a separate opportunity. The archive format and runtime instruction kernel are unchanged.

A fresh baseline and candidate were built from merged main with the same toolchain, portable feature, and ThinLTO profile. The diagnostic load-probe screen is retained separately. Final load-only results use the existing Criterion harness: 40 samples, 300 ms warmup, 1 s target, 10,000 bootstrap resamples, three rotating fresh processes per corpus/variant. Fixture parsing, source image reads, and scanner teardown remain outside load timing. Capture validation runs before every timing process.

| Corpus | Before ms | Direct string ms | Paired change | Three-process range | All runs ≥3% gain |
| --- | ---: | ---: | ---: | ---: | --- |
| cpp | 18.581 | 17.771 | -4.4% | -4.7% to -3.5% | yes |
| scss | 6.817 | 6.662 | -2.3% | -2.3% to -2.1% | no |
| java | 1.510 | 1.490 | -1.3% | -1.6% to -0.2% | no |
| json | 0.114 | 0.113 | -2.0% | -2.3% to -0.3% | no |
| rust | 0.684 | 0.664 | -3.2% | -3.5% to -1.9% | no |
| html | 11.379 | 11.177 | -2.0% | -3.1% to -1.7% | no |
| astro | 15.803 | 15.421 | -2.8% | -3.1% to -1.2% | no |
| tsx | 9.425 | 8.878 | -5.8% | -5.8% to -1.7% | no |
| c | 3.571 | 3.459 | -3.4% | -4.3% to -3.2% | yes |
| php | 14.543 | 14.492 | -0.4% | -2.3% to -0.3% | no |

The paired sum improves by 2.7% (range -3.03% to -2.66%). All central results are positive, but only C++ and C exceed 3% in every repetition. TSX has a larger median gain with a weaker repetition. Do not substitute diagnostic probe values for this final matrix.

All ten re-exported program and group archives match their originals byte-for-byte. All 75,501 captured calls match, including winning patterns and captures, without a group load fallback. Five focused precompiled/group integration tests, `cargo fmt --all --check`, and all-target/all-feature Clippy pass. No new search-speed claim is made.

## Integration and final-source verification

Both mechanisms are now implemented in `src/scanner/precompiled/wire.rs` and the Node package helper. The frame preflight lives in `scripts/portable-eager-zstd-frame.mjs`. The producer settings, manifest profile, Rust archive versions, and bytes are unchanged. The reader accepts a single dictionary-free frame with a declared content size, as produced by the package writer. Oversized windows raise `RangeError` before output allocation.

The final helper was compared with merged main again across all ten packages, three fresh processes per variant and corpus, three warmups and 15 timed calls. The measured boundary is unchanged. The paired package sums fall by 8.8% (range -9.25% to -8.56%). Eight corpora exceed 3% in every repetition; Java and Rust do not meet that floor. [shipping-summary.json](shipping-summary.json) contains every final-source estimate.

The integrated suite has ten tests. Its raw-frame fixtures are checked by libzstd and cover all four content-size field widths, small content with a large window, malformed blocks, unknown sizes, truncation, unsupported headers and frame boundaries. The tests require rejection by the helper without requiring the native decoder bug to persist in future versions. Existing checksum, payload-limit and CLI checks also pass.

Actual CLI unpacking reproduces both original archives for every corpus, then the native consumer replays all 75,501 captured calls without group fallbacks. Five focused Rust integration tests and the prescribed local format, Clippy, rustdoc, dependency, workflow and README checks pass. [shipping-validation.json](shipping-validation.json) records the final hashes and bridge checks; [shipping-evidence.tar.gz](shipping-evidence.tar.gz) retains the exact final helper, regression tests, fresh samples, and check logs.

## Remaining opportunities

- Evaluate a trained Zstd dictionary on held-out scanner samples, including dictionary transfer, preparation, memory, and reuse. This needs an explicit dictionary identity in the transport profile; no gain is established here. See the [Zstd training guidance](https://github.com/facebook/zstd#the-case-for-small-data-compression).
- Avoid the remaining owned-string allocation in pattern-key construction, then measure it independently. Group lookup accounted for about 1.99 ms of the earlier C++ phase profile.
- Ordinary program decoding remains substantial, especially PHP (about 5.04 ms in the earlier phase profile); the string writer change does not target that decoder.
- Reserved zero words occupy 13.3–16.3% of raw group archive bytes. Omitting them could reduce decompressed image traffic, but would need a new group format and an inflation step. These zeros already compress cheaply; no transfer-size or load-time gain is established. The measured byte layout is retained in `archive-layout.json`.

## Evidence and reproduction

Local macOS arm64, Node 24.22.0 / Zstd 1.5.7 and Rust 1.99.0, release/bench ThinLTO. Time Machine reported idle. No compilation or compression ran alongside the timed matrices. CPU placement, thermals, and unrelated activity are uncontrolled. Ranges describe three paired observations, not confidence intervals.

[node-summary.json](node-summary.json), [rust-summary.json](rust-summary.json), and [validation.json](validation.json) retain estimates, hashes, validation, toolchains, and measured boundaries. [evidence.tar.gz](evidence.tar.gz) includes the exact local modules, rejected variants, tests, source before/after, raw samples, Criterion artifacts, build logs, and binary hashes. The baseline helper and wire source match merged main exactly.

After extracting `experiment/` from the evidence and preparing the same raw and packaged assets:

```sh
node --test experiment/frame-guard.test.mjs experiment/guarded-tests/portable-eager-package.test.mjs
node experiment/measure-buffers.mjs PACKAGES RAW cpp guarded 1
```

For the Rust matrix, build and retain the existing benchmark twice, first at merged main and then with the `wire-after.rs` source from the evidence. For each corpus and variant, use a fresh output directory:

```sh
GROUP_ARCHIVE_CORPUS=cpp GROUP_ARCHIVE_VARIANT=portable GROUP_ARCHIVE_DIR=RAW GROUP_ARCHIVE_OUTPUT=OUTPUT BENCH_BINARY --bench --noplot /load
```

Repeat three times with rotating variant/corpus order. Re-export and compare both raw archives, then replay every captured call before interpreting the timings.
