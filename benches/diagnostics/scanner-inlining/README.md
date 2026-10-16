# Keep pattern insertion out of the candidate byte walk

Local diagnostic, October 16, 2026: Apple M1 Ultra, macOS 27.0.1 arm64,
Node 24.21.0, rustc 1.99.0. This does not update published benchmark numbers.

After moving Ferriki from Ferroni 2.1.0 to 2.2.1, repeated TypeScript/TSX HTML
calls regressed while other languages improved. Sampling located the extra
work in `SetPrefilter::candidates_at`. Its self CPU share changed from 27.02%
to 34.04% for TypeScript and from 26.07% to 33.05% for TSX. Summaries discard
the first two seconds of each 15-second capture, after setup and warmup.

The 2.2.1 hot loop stores and reloads its DFA state ID through a stack slot on
each byte, while the 2.1.0 loop keeps it in a register. Keeping
`PatternBits::insert` out of line reduces register pressure from bitset fields
in the large walk. Candidate selection, ordering, cache-clear tracking and
DFA warmup are unchanged. This is not a semantic fix or a revert of the bitset.

## Measurement

Profiles use Ferriki's existing `profile-tiobe.mjs`, large curated fixtures,
`github-dark`, native HTML, samply 0.13.1, and release builds with symbols.
Exact Shiki HTML agreement is checked before profiling. These instrumented
timings are not used for performance ratios.

Ordinary stripped release addons compare the unmodified local Ferroni 2.2.1
control against the one-attribute patch, built at the same source path with
the same compiler, flags and dependencies. Registry 2.1.0 and 2.2.1 controls
also reproduce the original regression. Three fresh workers rotate addon
order, check exact outputs, run 12 warmups and retain raw timed samples.

| Large case | Unchanged 2.2.1, ms | Patched 2.2.1, ms | Median worker change |
| --- | ---: | ---: | ---: |
| TypeScript | 15.687 | 14.085 | -10.57% |
| TSX | 19.531 | 17.571 | -9.69% |
| Astro | 21.022 | 19.916 | -5.90% |
| Svelte | 23.487 | 21.728 | -6.32% |
| Vue | 11.200 | 10.590 | -5.22% |
| CSS | 5.677 | 5.674 | -0.01% |
| Ruby | 3.637 | 3.641 | +0.19% |
| YAML | 31.797 | 32.074 | +0.73% |

The broad run covers 20 languages and both fixture sizes, 20 rounds per
worker. The median worker geometric mean changes by -2.77% versus unchanged
2.2.1, with a -3.31% to -2.33% range. Patched large TypeScript is 2.14%
below 2.1.0, TSX 1.12% below it. Not every case improves: small MDX and C++
have central changes around +3%, with worker ranges crossing zero. Time
columns pool samples; change columns take the median of worker ratios.

## Evidence and limits

`evidence.tar.gz` contains the complete Ferriki investigation: raw ordinary
comparison samples, symbolic profiles and symbol sidecars, build receipts,
input hashes, summaries, reproduction commands and compatibility logs.
Its README describes temporary paths from the capture, not runtime defaults.
The source/runtime consumer is Ferriki commit
`513a27aa967980d701d2acc1e485ad5a05a0f100`. The archived
context and receipts retain the complete source and build identities.

All compared HTML outputs match. The patched Ferriki core compatibility gate
passed, as did Ferroni's scanner differential/API integration suites with
`prefilter-self-check` and its 527 library tests run serially. The initial
parallel library run had fixture omissions and a single-Regex accept assertion;
the isolated unmodified test and complete serial patched run passed. Those
limitations and logs are retained, rather than claiming a green parallel run.

Inlining and register allocation depend on architecture and compiler. The
measurements establish an arm64 improvement on this host; Linux/x86-64 and
general-regex effects still need separate measurement. The general regex
comparison remains the publication baseline. No scanner/highlighter aggregate
is mixed into it by this change.
