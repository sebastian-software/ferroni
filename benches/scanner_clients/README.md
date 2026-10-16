# Scanner application workloads

These three small workloads exercise the public `Scanner` API as a client uses
it. They complement Ferroni's general regex suite. The measured executable has
no Ferriki, TextMate, asset-loading or rendering dependency.

| Case | Captured work | Scanners | Search calls | Measured phases |
| --- | --- | ---: | ---: | --- |
| TSX | Ordered competing patterns, failed searches, UTF-16 captures | 48 | 23,088 | Compile, first use, steady search |
| Astro | Interleaved creation/search/destruction, including 70 scanners used at most twice | 155 | 17,088 | Compile, first use |
| Related languages | TypeScript then TSX in one registry, preserving its shared pattern cache | 94 | 43,616 | Compile, first use; recorded cache and uncached control |

Each fixture is a complete event stream, including late-created scanners and
destruction. Compilation includes the captured cache clear/drop events and all
Scanner construction/destruction, while skipping searches. First use includes
every event, allocating fresh scanners/caches for each iteration. These phases
include the replay scheduler and native result destruction. They measure a
fresh document session in a warmed process, not process startup or one regex.

The TSX steady lane prebuilds its scanners, then repeats the fixed recorded
search sequence with persistent scanners. This is explicitly a warmed search
diagnostic; it does not represent scanner lifetimes or full highlighting.
Astro and related languages have no steady lifecycle claim. All phases prepare
fresh `OnigString` objects outside timing, retaining distinct subject identities
even when text is equal. UTF-16 starts, option bits, original pattern ordering,
misses, winning indices and all capture spans are preserved. Automatic default
prefiltering is used, including its normal activation threshold.

## Validate and run

```sh
./scripts/prepare-oniguruma-sources.sh
python3 -m unittest discover -s scripts/tests -p 'test_scanner_clients*.py'
cargo test --features ffi --test scanner_clients
python3 scripts/compare-scanner-clients.py validate /tmp/clients-validation
python3 scripts/compare-scanner-clients.py run /tmp/clients-run1 --only-run 1
python3 scripts/compare-scanner-clients.py run /tmp/clients-run2 --only-run 2
python3 scripts/compare-scanner-clients.py run /tmp/clients-run3 --only-run 3
python3 scripts/compare-scanner-clients.py merge /tmp/clients-merged /tmp/clients-run1 /tmp/clients-run2 /tmp/clients-run3
```

Use fresh output directories. The runner builds with `ffi` and validates every
trace against independent C Oniguruma winner/capture results before any timing.
Errors fail the run; there is no fallback or retry that turns them into misses.
The captured event stream is also replayed with the selected cache policy.
C is an untimed oracle, not an equivalent timed Scanner implementation.

Defaults are 30 samples, three seconds measurement and one second warm-up per
phase. Each invocation starts one independent process per case/cache policy.
Case order is seeded; the two related-language cache modes alternate order by
repetition. The manual Blacksmith workflow uses one VM per repetition and
retains this lane separately from production regex and tokenizer results.
ARM and x86 reports stay separate. Merge requires exactly repetitions 1–3,
matching source/build identities, complete cells and retained Criterion data.
Per-cell tables contain the median of process mean estimates and their range;
they do not provide a cross-case aggregate or statistical significance test.
Review raw intervals and host conditions before drawing performance conclusions.

For a local smoke test, use `--diagnostic --samples 10 --measure-seconds 0.02
--warmup-seconds 0.02`. Short runs do not establish a performance claim.
Evidence includes source snapshots, lockfile, C source pins, binary hash,
toolchain, environment overrides, validation logs and raw Criterion samples.
No website or general regex ranking is updated by this lane.

## Capture and provenance

[`provenance.json`](provenance.json) pins the Ferriki/Ferroni revisions, fixtures,
asset catalog and capture instrumentation by hash. The source documents are
Ferriki's curated `orders.ts`, `Orders.tsx` and `Orders.astro`, repeated 16 times.
The driver loads the relevant languages and `github-dark` up front, disables
tokenization time limits, and explicitly drops the registry after highlighting.
Related languages share one registry and one pattern cache. Phase markers
identify where TypeScript and TSX begin. Captures are collected directly at
Ferroni's public API boundary, before TextMate adjusts any capture spans.

To regenerate, check out the pinned Ferriki runtime revision, materialize its
locked assets, and run from Ferroni:

```sh
python3 benches/scanner_clients/capture.py /path/to/ferriki --ferriki-ref 513a27aa967980d701d2acc1e485ad5a05a0f100
```

The script copies both source trees to a new temporary directory, applies
`capture.patch` only to that engine copy, then runs `capture.rs` as a Ferriki
example. Neither source checkout is instrumented. Regeneration requires Ferriki's
dependencies cached for an offline release build. Raw JSONL captures and source
copies remain in the printed temporary directory. Review and commit the
regenerated JSON and provenance together. The instrumentation is never linked
into the measured executable. Future source changes may require refreshing the
diagnostic patch; its hash makes that change visible.

`GRAMMAR-NOTICES.md` is the pinned curated catalog's complete grammar notice
file (a superset of the grammars used here). Ferriki's MIT and Apache license
texts are retained alongside it for the source fixtures and driver context.
The fixture captures contain source regex patterns, not a vendored tokenizer.
