# Fork-free portable Eager: load-phase optimization

The optimized loader takes 4.6–28.2% less time than the frozen merged portable loader and 31.7–85.8% less than ordinary Eager across ten captured corpora (median paired changes from three processes). Changes versus the frozen Snapshot loader range from -37.5% to +13.6%. Every corpus gains at least 3% in each of the three repetitions versus merged Portable. These are in-memory Rust load times, excluding codecs, file I/O, and network transfer.

Warm-search changes versus merged Portable range from -1.5% to +1.3%; versus Eager, -3.8% to +10.3%; versus Snapshot, +0.9% to +11.1%. Cases at least 3% slower in every repetition: merged Portable: none; Eager: java, json, rust, astro, tsx, c, php; Snapshot: java, json, rust, tsx, c, php. This is still an opt-in prototype.

The [codec diagnostics](codecs.md) show decompression separately. The [complete comparison](comparison.md) retains all absolute times, paired ranges against each reference, Brotli/gzip sizes, peak RSS, and internal counters. [measurements.md](measurements.md) includes the feature control. [phases.md](phases.md) contains the before/after phase breakdown; [summary.json](summary.json) retains every Criterion mean estimate and reference pair. The [evidence archive](evidence.tar.gz) binds source overrides, build provenance, raw samples, validation, diagnostics, and checks.

## Load across all corpora

Milliseconds per captured corpus. Negative changes mean less time; the full comparison includes the three-process ranges.

| Corpus | Eager | Snapshot | Merged Portable | Optimized Portable | vs merged | vs Eager |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cpp | 92.384 | 21.890 | 25.157 | 19.835 | -22.5% | -78.5% |
| scss | 49.747 | 11.366 | 9.882 | 7.064 | -28.2% | -85.8% |
| java | 5.370 | 1.583 | 1.834 | 1.591 | -13.3% | -70.4% |
| json | 0.172 | 0.108 | 0.123 | 0.117 | -4.6% | -31.7% |
| rust | 2.140 | 0.634 | 0.785 | 0.705 | -9.5% | -66.0% |
| html | 43.794 | 13.304 | 14.470 | 11.856 | -18.5% | -73.1% |
| astro | 75.314 | 18.061 | 20.725 | 16.446 | -20.6% | -77.8% |
| tsx | 47.590 | 10.372 | 12.583 | 9.648 | -23.6% | -79.9% |
| c | 14.076 | 3.516 | 4.748 | 3.794 | -18.1% | -73.0% |
| php | 50.533 | 15.928 | 17.993 | 15.131 | -16.2% | -69.9% |

## Cause and implementation

The initial exclusive phase clocks put C++ group checksums at 5.77 ms (22.7% of load) and SCSS at 3.05 ms (30.3%). The serial word checksum had one dependent multiplication chain. The group format now uses four independent word lanes for both record checks and the binding to its program archive. C++ group checksum time falls to 1.71 ms and SCSS to 0.92 ms in the separate diagnostic runs.

The portable instruction reader preflights the complete `count × 16` byte span and decodes fixed-size chunks. It avoids four fallible reader calls per instruction and allocation before checking the complete span. C++ instruction decoding falls from 3.10 to 1.90 ms in the diagnostic builds. Full opcode, reference, range, assertion, and byte-class validation still runs. No unsafe code, dependencies, dependency forks, or execution-cache changes were added.

New exports use `FPRT002` with the new corruption checks; consumers continue to load `FPRT001` using its original checks. The ordinary precompiled program format remains version 2. Program archives and every group identity/instruction body match the merged packages byte-for-byte. Corruption checks are not authentication. Tests cover every single-bit change across lane/tail boundaries, legacy loading, and invalid entry states with a correctly recomputed checksum.

A fused decode/validation loop was rejected: a rotating three-process load screen made C++ 4.3–6.0% slower and produced mixed changes elsewhere. Its exact intermediate source and raw screen are retained. Diagnostic scopes are behind `scanner-load-profile` and compile out of all final measurement binaries.

## Size and memory

Raw package lengths are identical to merged Portable in every corpus. Brotli-11 changes range from -0.39% to +0.48%; the instruction payload is unchanged and only checksum/header bytes differ. Eager, Snapshot, and optimized Portable were freshly compressed and round-trip checked. Before sizes are reused only after payload SHA-256 identity checks; their historical codec times are labeled and are not a new measurement. gzip-9 and decode diagnostics were rerun serially before the main matrix.

For C++, optimized Portable is 24.22 MiB raw and 2.36 MiB with Brotli-11. Snapshot is 10.35 MiB raw / 1.43 MiB Brotli; Eager is 1.60 / 0.04 MiB.

Internal immutable and scanner scratch counters are identical to merged Portable across all corpora. Peak RSS retains fixture parsing, source images, decoded programs, scanners, and four document passes. It is a process high-water mark, not an allocation total; see all three process ranges in the JSON and the full comparison. There is no general memory-reduction claim.

## Method and verification

- Base and frozen merged baseline: `0953789a0a777163c9497478d39166203e410e6e`. The 198 baseline build inputs match that commit. Old caches, binaries, and reports remain intact.
- Ten captured corpora; all 75,501 calls, winning patterns, and capture arrays agree in all five lanes before timing. New-reader replay of legacy packages also reproduces all calls. Every archived group is consumed without a decode fallback. The refreshed producer deterministically reproduces every new image.
- Five variants × ten corpora × three rotating process repetitions. Criterion: 40 samples, 300 ms warmup, 1 s target, 10,000 bootstrap resamples. Criterion extends the measurement duration where 40 iterations need more time. Ratios pair the same repetition; observed ranges are not confidence intervals.
- Load includes indexing, checksum/structural validation, decoding, helper preparation, and scanner/cache construction. Warm search follows three complete document passes; fresh string identities and teardown stay outside timing. First document includes load plus its first replay.
- Separate phase diagnostics: three rotating processes, 20 fresh loads per process, nested exclusive clocks. Instrumentation adds overhead; these explain causes and do not replace final Criterion timings.
- Local Mac Studio M1 Ultra, 64 GB, macOS arm64, rustc 1.99.0, release/bench ThinLTO. Time Machine was idle before the matrix. No experiment compilation or compression ran beside main timings. Core placement, thermals, and unrelated background activity were not controlled.
- Current producer and consumer resolve registry regex-automata 0.4.18 and aho-corasick 1.1.5. Snapshot is an unchanged historical executable with its old dependency extensions; those are not in the current dependency graph. The feature control runs ordinary Eager through the optimized Portable binary without a group image.
- Local release validation: 580 focused tests pass (one test remains ignored), plus the diagnostic test without default features. Coverage includes portable differential searches with tiny cache resets, malformed/truncated archives, legacy checksums, re-export, captures/order/duplicates, and mode fallback. Formatting passes; Clippy passes with all features and with `precompiled,scanner-load-profile` without defaults. Test counts come from `scripts/count-tests.sh` and are retained separately; no new CI claim is implied.

Within Rust, the largest remaining costs depend on the corpus: PHP spends about 5.04 ms in ordinary program decoding, SCSS about 2.14 ms in instruction validation, and C++ about 3.24 ms in scanner assembly plus 1.99 ms in group lookup. These are concrete next profiling targets. This comparison does not establish cross-platform behavior, full grammar coverage, authenticated distribution, or Ferriki/CDN startup time.

Compressed inputs make codec work a larger target now. The fresh C++ standalone Brotli diagnostic takes 46.24 ms, compared with 19.84 ms of optimized Rust load; gzip decoding takes 33.32 ms. These are separate runtimes and boundaries, not a measured end-to-end sum. Format/codec layout and the actual application decode path deserve priority before further small loader changes. See the codec table for every corpus.

## Zstd follow-up

The [same-runtime codec comparison](zstd.md) tests Brotli-11 and Zstd levels 3, 9, and 19 on all ten optimized portable packages, with three rotating Node processes and 15 samples per codec/process. The decompressed payloads match the Rust matrix by SHA-256; every decode also checks every byte. Rust source, archive instructions, and search behavior are unchanged by this outer compression experiment.

Zstd-19 cuts median decode time by 25.4–55.5% versus Brotli-11 while package size changes range from −4.6% to +13.0%. For C++, Brotli-11 is 2.36 MiB / 44.17 ms decode; Zstd-19 is 2.66 MiB / 27.27 ms. Level 9 decodes C++ in 24.05 ms but takes 3.64 MiB. Level 19 is a promising balance for preproduced packages. Zstd frame checksums are enabled. These native Node measurements do not establish browser or Rust codec speed, or a complete CDN startup time.

[zstd.json](zstd.json) retains process means, paired ranges, encode diagnostics, runtime versions, sizes, and hashes. [zstd-evidence.tar.gz](zstd-evidence.tar.gz) retains the exact source and raw samples separately from the Rust measurement evidence.

## Reproduction

Retain the historical caches referenced by the build provenance. Use fresh output directories and omit `scanner-load-profile` from final binaries. Frozen Before reads its original FPRT001 artifact directory; optimized Portable reads FPRT002.

```sh
python3 scripts/prepare-portable-eager-comparison.py NEW_CACHE --snapshot-cache SNAPSHOT_CACHE --portable-reference-cache MERGED_BASELINE_CACHE
# Export each corpus with NEW_CACHE/bin/portable prepare-portable CORPUS NEW_CACHE/artifacts.
python3 scripts/compare-scanner-group-archives.py NEW_CACHE/run-01 --cache NEW_CACHE --prebuilt --reuse-artifacts --prepare-only --before-artifacts MERGED_ARTIFACTS --variants eager prepared before portable control
node scripts/compress-scanner-group-archives.mjs NEW_CACHE/artifacts BROTLI_JSON eager prepared portable
python3 scripts/compare-scanner-group-archives.py NEW_CACHE/run-01 --cache NEW_CACHE --measure-only --repetitions 3
python3 scripts/report-portable-load.py NEW_CACHE/run-01 COMBINED_BROTLI_JSON REPORT_DIR --phases PHASE_DIR
```

The evidence includes the helper scripts that combine the verified reference codec rows, capture diagnostic phases, and retain final source overrides. The current tree cannot rebuild the historical Snapshot without its historical sources; its original evidence is retained in the preceding dated reports.

## Additional compression levels

The [level 16/17/21/22 follow-up](../portable-codec-levels-20261024/README.md) reruns codec-isolated processes and checks actual frame windows. It confirms 19 as the preferred HTTP delivery balance: 17 decodes faster with more bytes, while 21/22 require an explicit 8 MiB window for larger HTTP content-encoded packages and then save less than 0.5% total size.
