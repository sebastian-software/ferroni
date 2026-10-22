# Semver regression fix and local comparison

This investigation compares the possessive negated-class optimization with
`164bb809cea91c478fb5614d5c423922fc150197`, the previous version. The original
candidate's [broader verification](../possessive-negated-verification-20261022/README.md)
confirmed a 5.90% Semver hyphen-range slowdown.

The selected fix keeps HTML markup at **7.00% less elapsed time** in the full
29-case matrix. A separate, preplanned 64-pair confirmation puts Semver hyphen
at **−0.32%** (descriptive interval −0.67% to +0.17%), Jinja strings at **−4.82%**,
and Jinja integers at **+0.09%**. The original material Semver regression does
not repeat in that wider confirmation. The less precise full-matrix Semver
result remains reported below; the two datasets are not pooled.

## Cause and implementation

Neither Semver program contains the optimized `CClassNotStar` instruction.
The slowdown comes from changes to generated code in the shared matcher,
including instruction placement and register allocation. Disassembly and a
control that restores the original capture-VM instruction sequence isolate
this from extra matching work in the Semver expression. The precise hardware
cache or predictor mechanism has not been established.

The conservative `helper02` implementation puts possession selection in
`run_negated_byte_star`, returns the run end and next instruction by value,
and keeps the byte walker separately out of line. It still omits temporary
backtrack entries that the immediately following pop would discard. Explicit
configured stack limits keep the original entries and both instructions.
No opcode, compiled-pattern field, allocation, dependency, or unsafe code is
added. The linked benchmark's instruction section grows by 3,840 bytes (0.098%),
with no increase in its `__TEXT` segment.

The [investigation](INVESTIGATION.md) explains the rejected alternatives.
More aggressive inlining recovered additional HTML savings but regressed
Jinja integers. Smaller static VM bodies also failed to guarantee better
capture performance. Every completed experiment is retained.

## Full 29-case matrix

Negative changes mean less elapsed time. Each row contains 16 paired process
means, across eight environment-size offsets visited in opposite binary
orders. Intervals are descriptive and pointwise, not simultaneous guarantees.

| Case                                 | Median | Descriptive 95% interval | Cycle 1 / cycle 2 | Slower pairs |
| ------------------------------------ | -----: | ------------------------ | ----------------- | -----------: |
| `django_email_domain`                | +0.12% | -0.09% to +0.44%         | +0.40% / +0.07%   |         9/16 |
| `django_email_local_part`            | -0.07% | -0.36% to +0.28%         | -0.09% / -0.07%   |         7/16 |
| `django_url_regex`                   | -0.20% | -1.04% to +0.68%         | -0.30% / +0.59%   |         7/16 |
| `ferrolink_attribute_split`          | -2.88% | -3.84% to -0.39%         | -3.10% / -1.32%   |         4/16 |
| `ferrolink_markup_scan`              | -7.00% | -7.28% to -6.60%         | -7.20% / -6.73%   |         0/16 |
| `fluent_bit_apache`                  | -1.94% | -2.39% to -1.20%         | -1.97% / -1.57%   |         1/16 |
| `fluent_bit_nginx`                   | -2.22% | -3.27% to -2.15%         | -2.21% / -2.52%   |         0/16 |
| `ipv4_validation`                    | +0.20% | -0.15% to +0.92%         | +0.52% / +0.10%   |         9/16 |
| `ipv6_validation`                    | -0.13% | -1.28% to +0.11%         | -0.30% / -0.13%   |         7/16 |
| `jinja_float_token`                  | +0.15% | -0.41% to +0.88%         | -0.70% / +0.68%   |         9/16 |
| `jinja_integer_token`                | -0.13% | -0.50% to +0.01%         | -0.29% / -0.13%   |         6/16 |
| `jinja_string_token`                 | -3.07% | -5.21% to -1.49%         | -3.46% / -2.17%   |         2/16 |
| `literal_absent`                     | -0.17% | -0.61% to +0.25%         | -0.00% / -0.35%   |         7/16 |
| `literal_end`                        | -0.14% | -0.98% to +0.20%         | -0.24% / -0.07%   |         7/16 |
| `literal_first`                      | -0.13% | -0.78% to +0.40%         | +0.40% / -0.72%   |         8/16 |
| `literal_middle`                     | -0.50% | -2.36% to +0.23%         | +0.43% / -1.95%   |         7/16 |
| `mastodon_hashtag_regex_step`        | -0.17% | -0.71% to +1.30%         | -1.52% / +0.34%   |         7/16 |
| `python_markdown_smart_emphasis_re`  | +0.42% | -1.28% to +1.30%         | +0.42% / +0.08%   |        10/16 |
| `python_markdown_smart_strong_em_re` | +0.96% | -1.02% to +1.61%         | +1.24% / +0.60%   |        10/16 |
| `rails_underscore_regex_step`        | +0.64% | +0.48% to +1.38%         | +0.78% / +0.61%   |        12/16 |
| `semver_hyphen_range_captures`       | +1.13% | +0.58% to +6.20%         | +1.13% / +2.98%   |        11/16 |
| `semver_version_parse`               | -0.28% | -1.36% to -0.18%         | -0.24% / -0.42%   |         5/16 |
| `symfony_grapheme_chunks_1`          | +0.50% | -0.24% to +0.90%         | +0.50% / +0.31%   |         9/16 |
| `symfony_grapheme_chunks_2`          | -0.89% | -1.01% to -0.51%         | -0.61% / -0.89%   |         5/16 |
| `tiktoken_cl100k_base`               | -0.35% | -0.98% to +0.04%         | -0.28% / -0.36%   |         4/16 |
| `tiktoken_o200k_base`                | +0.16% | -0.43% to +0.91%         | +0.34% / -0.40%   |        10/16 |
| `uuid_shape`                         | +0.26% | -1.29% to +0.73%         | +0.73% / -0.76%   |         9/16 |
| `uuid_v4`                            | +0.03% | -0.72% to +0.88%         | +0.67% / -0.12%   |         8/16 |
| `wordpress_shortcode_captures`       | -0.85% | -2.21% to -0.25%         | -0.49% / -1.55%   |         5/16 |

HTML takes 7.00% less time, with both cycle medians beyond the 3% target and all
16 pairs faster. Jinja strings improve by 3.07% overall, but the second cycle
takes only 2.17% less time: this matrix does not establish a 3% gain in both cycles.
Jinja integers are effectively unchanged (−0.13%). Semver version parsing is
−0.28%. Semver hyphen ranges improve from the original candidate's +5.90%
to +1.13% relative to the previous version, but the interval still reaches
+6.20%. No case has a median slowdown at or above 3%; Semver hyphen is the
only case whose interval leaves such a slowdown unresolved.

## Independent wider-layout confirmation

The candidate was selected and its exact binary reproduced before this
confirmation. All three cases receive 64 pairs: 32 environment offsets from
0 through 248 bytes in steps of eight, each visited with opposite binary
orders. The base is fixed at 4,288 bytes, matching the full matrix. Timing
budgets remain 30 samples, 0.5 s warmup, and 1.5 s measurement per process.

| Case                           | Median | Descriptive 95% interval | Cycle 1 / cycle 2 | Slower pairs |
| ------------------------------ | -----: | ------------------------ | ----------------- | -----------: |
| `jinja_integer_token`          | +0.09% | -0.19% to +0.36%         | -0.01% / +0.19%   |        35/64 |
| `jinja_string_token`           | -4.82% | -5.18% to -3.97%         | -4.65% / -5.05%   |         5/64 |
| `semver_hyphen_range_captures` | -0.32% | -0.67% to +0.17%         | -0.33% / -0.32%   |        27/64 |

The Semver interval is entirely inside ±3%, with nearly identical cycle
medians. Jinja integers are also inside ±3%. Jinja strings exceed the 3%
target in both cycles and improve in 59 of 64 pairs. These independent results
support the conservative fix while preserving the full matrix's wider Semver
interval as a limitation of that smaller sample.

A fresh 64-pair Semver A/A control over the same layouts reports **−0.24%**,
with an interval of **−0.94% to +0.04%** and cycle medians **−0.42% / −0.06%**.
Its individual pairs range from −8.19% to +7.52%. This is a check of host
variation, not a correction subtracted from the version comparison. The earlier
three-case A/A control is retained in `results.json`; the new A/A and A/B
results are in `extended-results.json`.

## Method and scope

The machine is an Apple M1 Ultra with 20 logical CPUs and 64 GiB of memory,
running macOS 27.0.1. Builds use Rust 1.99.0 / LLVM 23.1.1, the bench profile
with ThinLTO, default target CPU, default `dfa-prefilter` plus `ffi`, and
`PCRE2_SYS_STATIC=1`. The baseline commit, lockfile, corpus, source, and binary
hashes are retained in the evidence.

The matrix covers all 27 executable production cases and both tokenizer cases.
It measures steady fixture batches through the standard Rust API. It does not
measure regex construction, first use, complete applications, or other CPUs.
Each process uses 30 Criterion samples, 0.5 s warmup, and 1.5 s measurement.
Case order is shuffled; each environment offset is revisited with the opposite
A/B order. Executable paths have equal lengths. The final full matrix uses a
4,288-byte environment base and offsets 0 through 56 in steps of eight.
Earlier focused phases used their own recorded bases; they must not be treated
as identical process layouts. Subsequent confirmations specify the base
explicitly.

The estimate is the median percentage change of paired process means.
Descriptive 95% intervals use 10,000 percentile-bootstrap resamples of layout
clusters, retaining both opposite-order pairs together. A repeatable effect
at the 3% target requires the overall median and both cycle medians to cross
that threshold, with the interval excluding zero. An interval reaching +3%
leaves a material slowdown unresolved. Intervals are not corrected for the
multiple cases, candidate selection, or uncontrolled host activity.

The host is interactive. Background applications, CPU affinity, and frequency
are uncontrolled. No task builds, tests, profiling, or other task benchmarks
run during timing. No process, sample, or outlier is dropped. A/A controls are
checks of variation; their values are not subtracted from A/B results. Each
benchmark process performs its source-oracle checks before measuring.

## Correctness and local gates

The exact selected source passed the full serial test command with
`ffi,prefilter-self-check`: **2,775 tests passed, four ignored**. This includes
155,520 fused/reference result, capture, and retry comparisons plus configured
stack-limit boundaries. `scripts/count-tests.sh` reports 546 source unit tests
and 2,124 integration tests; this source count is distinct from runtime results
including doctests.

Formatting, all-target/all-feature Clippy with warnings denied, Rustdoc with
warnings denied, cargo-deny, workflow pins, README family/figures, production
corpus, and benchmark-claim checks passed. The retained validation logs identify
which source was tested. Hosted CI, instrumented coverage, the full Node
documentation build, and the external Ferriki corpus were not run.

## Evidence and reproduction

The [plan](PLAN.md), [investigation](INVESTIGATION.md), runner `compare.py`,
analyzer `analyze.py`, disassembly extractor `codegen.py`, completeness audit
`audit.py`, and archive verifier `bundle.py` accompany this report. [results.json](results.json) contains the full matrix and its A/A control;
[extended-results.json](extended-results.json) contains the independent wider
confirmation. [context.json](context.json), [validation.json](validation.json),
[audit.json](audit.json), and [provenance-audit.json](provenance-audit.json)
record source/build provenance, correctness checks, completeness, execution
order, and hash verification.

The [evidence archive](evidence.tar.gz) retains all **3,968 completed timed
processes**, **1,984 pairs**, and **117,120 Criterion samples** across 16 phases,
including every rejected measured candidate, process log, raw sample,
profile, disassembled VM body, source snapshot, and candidate patch. A manifest
records every archived file's size and SHA-256 hash; the bundler reads the
archive back and verifies each entry. The outer checksum is in
[evidence.sha256](evidence.sha256), and the verification result is in
[archive-audit.json](archive-audit.json). Executables and redundant full-binary
disassembly dumps are excluded. The unmeasured stale `compact5` artifact is
explicitly labeled in the investigation and evidence.

Before and after the final measurement sequence, `pmset` reported no recorded
thermal or performance warning, and no recorded CPU power status.

Build the previous and selected versions with:

```sh
PCRE2_SYS_STATIC=1 cargo bench --locked --offline --features ffi \
  --bench production_regex_bench --no-run
```

Retain their executables at equally long paths. Run `compare.py` from the repo
root with Python 3.12 or newer, fresh output directories, and a fixed
`--environment-base` large enough for every planned child environment. Use
`--cases all --repetitions 16 --samples 30 --warmup 0.5 --measure 1.5` for the
full matrix. The default eight offsets are revisited in the opposite order.
A/A requires a byte-identical copy of the baseline. Reproduce the wider
confirmation with the explicit cases, offsets, repetitions, and base recorded
in its context. Run the analyzer with `--broad` and `--control` naming those
output directories; run the audit across every retained phase before bundling.
