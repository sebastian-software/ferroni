# TextMate startup: compact portable byte planes

The target is the remaining startup gap between source Off and prepared Eager
for C++, SCSS, and Astro. The implementation reduces transport, verification,
and instruction decoding work without changing the execution kernel or adding
dependency forks. The same ten full TextMate workloads are measured again.

## Change

The existing runtime instructions have four 32-bit words, one reserved. The
new FPRT003 wire image packs the opcode into a bounded operand and writes two
words per instruction. The branch/range pool uses 16-bit words under the
existing 65,536-state bound. Equal byte positions are stored together in byte
planes so Zstd can compress repeated high bytes. The loader widens directly
into the same runtime arrays, then runs the existing structural validator.

The ordinary compiled-program format stays at version 2. FPRT001/FPRT002 remain
readable, integrity checks remain enabled, and Auto/Off behavior is unchanged.
The outer package retains Zstd level 19 and its existing limits and SHA-256
checks. Packages need regeneration to obtain the new representation.

## Startup including the first highlighted document

| Language | Before ms | After ms | Paired change | Process range | Every run gains ≥3% |
|---|---:|---:|---:|---:|---|
| C++ | 79.513 | 61.716 | -22.8% | -22.9% to -22.4% | yes |
| SCSS | 36.194 | 25.260 | -29.5% | -30.2% to -28.9% | yes |
| Java | 8.491 | 7.930 | -6.9% | -7.9% to -6.4% | yes |
| JSON | 4.302 | 4.339 | +1.7% | -4.2% to +1.7% | no |
| Rust | 4.767 | 4.517 | -4.4% | -6.0% to -4.1% | yes |
| HTML | 46.703 | 40.520 | -13.4% | -15.8% to -12.8% | yes |
| Astro | 77.301 | 66.070 | -14.7% | -15.1% to -14.5% | yes |
| TSX | 53.305 | 45.952 | -13.8% | -13.9% to -12.8% | yes |
| C | 13.318 | 11.979 | -10.1% | -12.1% to -9.5% | yes |
| PHP | 49.783 | 44.442 | -10.7% | -11.2% to -9.2% | yes |

## Execution after three warm document passes

| Language | Before ms | After ms | Paired change | Process range | Every run gains ≥3% |
|---|---:|---:|---:|---:|---|
| C++ | 8.162 | 8.149 | -0.2% | -2.5% to +2.7% | no |
| SCSS | 3.797 | 3.825 | -0.3% | -1.5% to +0.7% | no |
| Java | 4.324 | 4.333 | +0.4% | -0.0% to +1.4% | no |
| JSON | 4.460 | 4.533 | +1.6% | +1.1% to +1.6% | no |
| Rust | 3.017 | 2.960 | -0.6% | -1.9% to +0.6% | no |
| HTML | 11.915 | 11.804 | -0.9% | -1.2% to +1.6% | no |
| Astro | 23.529 | 24.314 | +3.3% | +0.4% to +7.9% | no |
| TSX | 18.243 | 18.264 | +0.6% | -5.1% to +1.5% | no |
| C | 3.711 | 3.723 | -1.1% | -1.6% to +0.4% | no |
| PHP | 3.398 | 3.400 | +0.1% | -1.1% to +1.1% | no |

Astro's initial warm-search median is +3.3%, with a +0.4% to +7.9% process
range. A targeted follow-up selects only `warm_search` in five fresh alternating
pairs using the same frozen binaries and inputs, retaining initialization and
correctness checks. Its median paired change is +0.8%, range -1.1% to +2.5%.
That follow-up does not reproduce a consistent slowdown of at least 3%; the
initial result remains in the table rather than being replaced. Search uses
the same runtime instructions, pool word types, and execution kernel.

## Package sizes

Total download includes the unchanged compressed TextMate grammar dependency
closure plus the prepared sidecar. Times above exclude actual CDN transfer.

| Language | Decoded payload MiB, before → after | Prepared Zstd KiB, before → after | Total download change |
|---|---:|---:|---:|
| C++ | 25.23 → 13.99 | 2673.5 → 956.6 | -63.3% |
| SCSS | 12.29 → 6.50 | 1315.5 → 797.3 | -37.4% |
| Java | 1.08 → 0.69 | 116.2 → 83.9 | -26.6% |
| JSON | 0.03 → 0.02 | 4.5 → 4.2 | -6.3% |
| Rust | 0.30 → 0.18 | 42.7 → 35.2 | -16.1% |
| HTML | 12.30 → 8.35 | 1154.4 → 698.3 | -37.5% |
| Astro | 19.86 → 12.90 | 1782.3 → 831.1 | -50.2% |
| TSX | 13.20 → 8.69 | 1035.8 → 404.6 | -56.7% |
| C | 3.00 → 1.58 | 286.6 → 231.5 | -18.5% |
| PHP | 12.56 → 8.12 | 1304.5 → 929.8 | -26.5% |

## Diagnosis and rejected layouts

The initial instrumented runs identified verified decompression and decoded
SHA-256 as the largest avoidable startup costs. A variable-length opcode layout
reduced bytes but increased instruction decoding work. Fixed packed records and
a narrower pool improved startup but enlarged the SCSS Zstd asset. Compression
screens compared column layouts and byte planes; the selected byte planes
reduce both decoded size and compressed transport size. Compression-only screen
images retain placeholder old record checksums and are never used as runtime
validation evidence. Final images are exported and checked by the Rust producer.

`profile-before.json`, `profile-compact.json`, `profile-packed.json`, and
`profile-shuffled.json` retain the diagnostic phase clocks. Their instrumentation
overhead is not part of the final Criterion comparison. Final benchmarks use
ordinary release builds with phase clocks compiled out. The search kernel and
runtime instruction/pool layouts are unchanged.

## Method and validation

The baseline and candidate use the same isolated TextMate harness and lockfile,
release ThinLTO profile, hardware SHA-256, libzstd, source grammar definitions,
dependency closures, documents, and expected scope/binary tokens. Each matrix
cell has three fresh Criterion processes; corpus and variant order rotate.
Each process uses 40 samples, a 300 ms warmup, a one-second target, and 10,000
bootstrap resamples. Absolute cells are medians of process mean estimates;
percentage changes are computed within each repetition before aggregation.
There are 60 process runs and 180 metric series. Pure grammar construction is
retained separately as `construct` but excludes deferred scanner work.

Before timing, both versions and the new consumer with all ten old FPRT002
archives check complete scope/binary output over four document passes. Every
prepared group is consumed, with zero group fallbacks. Every benchmark process
also independently checks its first document. All three validation lanes
produce the same 58,103 tokens across 5,984 lines. The new ordinary program
archives and expected output JSON match the old producer byte-for-byte.
All ten new program/group images and expected output files also re-export
byte-for-byte after loading and executing the prepared representation.

Source, input, and executable SHA-256 hashes are recorded before timing and
rechecked afterward. No build, compression, or test task runs during final
timing. Time Machine is checked before and after the matrix. Hardware is the
same local Apple M1 Ultra with 64 GB RAM. Network, file I/O, theme application,
and HTML rendering are outside timing. Prepared groups cover the encountered
static scanners; dynamic patterns and unseen grammar paths retain fallback.
`peakRssBytes` in the JSON is a single fresh validation-process high-water mark,
including inputs, transport buffers, allocator retention, and repeated document
passes. It is not an isolated live-heap measurement or a timing series.

Local validation includes the portable-feature library suite, precompiled and
scanner-group integration suites, the NFA-only group suite, formatting, and
all-target/all-feature Clippy. `validation.json`, raw Criterion samples and
estimates, build/test logs, source patch, and diagnostic scripts are retained
in `evidence.tar.gz`, together with the frozen input/harness evidence.
