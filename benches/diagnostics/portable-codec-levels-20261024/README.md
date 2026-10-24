# Zstd levels 16, 17, 19, 21, and 22

Level 19 remains the preferred balance for HTTP delivery of these prepared scanner packages. Across the ten captured packages, level 17 uses 13.3% more bytes and saves 6.8% decode time versus 19. Default levels 21 and 22 reduce the total encoded size by 2.1–2.4%, but their frame windows exceed the HTTP limit on larger inputs. With an explicit 8 MiB window, their size advantage shrinks to 0.07–0.40% and there is no decode gain.

The [full measurements](measurements.md) include every corpus, all paired ranges, actual frame windows, encode diagnostics, and process memory. [summary.json](summary.json) retains all process means and comparisons against both 19 and Brotli-11; [evidence.tar.gz](evidence.tar.gz) contains exact sources and raw samples. Earlier reports and encoded baselines remain intact.

## Zstd-19 package integration

The subsequent [package workflow](../portable-eager-prototype.md#package-with-zstd-19)
uses level 19, an explicit 8 MiB window, and frame checksums. It adds a manifest
and verified unpacking outside the Rust engine. The integration check rebuilt
all ten packages, unpacked them into a separate consumer directory, compared
both original archives byte-for-byte, and ran the native portable loader and
complete capture replay. All 75,501 calls matched, with no group decode
fallbacks. Six Node tests also cover the real codec, corruption, window and
output limits, manifest boundaries, and CLI behavior.

The ten encoded packages total 10,144,768 bytes (9.675 MiB), excluding the small
JSON manifests. Every encoded SHA-256 matches the measured level 19 package
above, despite now setting the window cap explicitly. The Rust loader and
execution kernel did not change in this packaging step. This is a correctness
check, not a new load or search timing run; the new transport digest checks are
outside the original decode timings. [package-validation.json](package-validation.json)
records the hashes and native replay results;
[package-evidence.tar.gz](package-evidence.tar.gz) retains the helper and test
source separately from the earlier benchmark evidence.

## Ten-package comparison

Size is the sum of the ten encoded packages. Decode changes pair the sums of per-corpus process means in each repetition, then take the median. These are the captured bundle mix, not a universal workload weighting or a measured application startup time.

| Variant | Total size vs 19 | Decode time vs 19 | Observed decode range |
| --- | ---: | ---: | ---: |
| 16 | +18.16% | −5.4% | −8.1% to −4.8% |
| 17 | +13.25% | −6.8% | −8.0% to −6.5% |
| 19 | reference | reference | reference |
| 21, default window | −2.11% | −3.2% | −5.0% to −2.8% |
| 22, default window | −2.41% | −3.7% | −6.4% to +5.7% |
| 21, window capped at 8 MiB | −0.07% | +2.0% | +0.0% to +4.2% |
| 22, window capped at 8 MiB | −0.40% | +1.4% | −0.7% to +1.8% |

For this bundle mix, 17 is a useful alternative when decode time matters more than transfer size. Level 16 offers no improvement over 17 in the bundle sums, although individual corpora differ. Default level 21 is worth considering for a downloaded asset with an explicitly compatible decoder. Level 22 adds little size benefit over 21 and its decode result is noisy. For HTTP content coding, 19 avoids an additional window override and performs as well as the capped higher levels.

## Why the window matters

[RFC 9659](https://www.rfc-editor.org/info/rfc9659/) limits frames used with `Content-Encoding: zstd` to an 8 MiB window. The default 21/22 C++ frames request 24.22 MiB. Their compressed sizes are 2.522 and 2.510 MiB, versus 2.658 MiB at level 19. Setting `ZSTD_c_windowLog=23` produces 8 MiB frames of 2.660 and 2.650 MiB instead.

Every Zstd frame was inspected with zstd CLI 1.5.7 and decoded with `ZSTD_d_windowLogMax=23`. Oversized frames fail with the expected window-too-large error. The capped variants succeed and match the original payload SHA-256. This verifies the window requirement; it does not establish browser support or complete CDN integration. A separately downloaded `.zst` asset with its own sufficiently capable decoder is a different transport choice.

## Method and limits

- Same ten portable payloads as the preceding Rust loader comparison: C++, SCSS, Java, JSON, Rust, HTML, Astro, TSX, C, and PHP. All input hashes match; every decoded output is compared byte-for-byte outside timing. Zstd frame checksums are enabled. No dictionaries are used.
- Nine configurations, including Brotli-11 and level 9 controls, with three fresh Node processes per corpus/configuration: 270 process measurements, three warmups and 15 timed decodes each. Corpus and codec order rotate. Compression finishes before decoding is measured.
- Node 24.22.0, Zstd 1.5.7, Brotli 1.2.0, local macOS arm64 on the same M1 Ultra host as the preceding comparison. Timing includes output allocation, excludes input I/O, output comparisons, Rust loading, and teardown. Core placement, thermals, and unrelated background work are not controlled.
- The preceding codec screen grouped codecs within a process. All controls are rerun here with isolation per codec. Compare within this matrix rather than combining old and new absolute decode times. Observed ranges are not confidence intervals.
- Encode times and encoder RSS are one observation per corpus/level, not repeated encoder benchmarks. At C++, encoder peaks range from 190 MiB for 19 to 510 MiB for default 22; capped 21/22 use about 238 MiB. Those peaks include input construction. Decoder peaks include retained source data, 18 decodes, allocator retention, and garbage collection; they are not intrinsic codec workspace sizes.
- Frozen Brotli-11 and level 9 encodings are reused only after matching runtime versions, input and encoded hashes, and a full round trip. Level 19 is rebuilt and its encoded hash matches the preceding experiment. Exact preparation and timing source versions are archived separately.
- The Rust engine, raw archive format, and search kernel are unchanged in this follow-up. No dependency forks or new Cargo dependencies are involved. These Node measurements do not establish Rust/browser decoding speed or an end-to-end startup result.
