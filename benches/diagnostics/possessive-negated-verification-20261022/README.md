# Broader local verification of possessive negated-class runs

This comparison tests the unchanged optimization from the [second local
investigation](../possessive-negated-runs-20261022/README.md) against its previous
version, `164bb809cea91c478fb5614d5c423922fc150197`. The question is whether the
HTML gain survives a broader matrix and whether the SemVer slowdowns repeat.

The subsequent [root-cause investigation and revised implementation](../possessive-negated-fix-20261022/README.md)
address this regression separately. The results and rejection below describe
the original candidate and remain unchanged.

## Result and recommendation

The broader comparison **confirms the SemVer hyphen-range regression**. The
candidate takes **5.90% more time** in the median, is slower in **15 of 16 pairs**,
and has cycle medians of **+5.90% and +4.99%** with opposite binary orders. Its
descriptive 95% interval is **+1.39% to +6.93%**. The A/A control for this case
has a median of +0.18% and an interval of -1.33% to +1.37%. The A/A result is not
subtracted from the version comparison.

The original caveat should therefore remain a delivery blocker for this
candidate as a general performance improvement. Leave it unmerged until the
regression is addressed or its tradeoff is explicitly accepted. This task does
not modify or retune the runtime implementation.

The gains also repeat: markup scanning takes **10.29% less time**, improving in
all 16 pairs, and Jinja strings take **3.67% less time**, improving in 14 of 16.
Their two cycle medians both exceed the 3% target in the favorable direction.
The earlier SemVer **version-parser** concern is much smaller here: +0.48%,
with an interval of +0.02% to +1.62%. It is distinct from the hyphen-range case.

Two further cases leave a 3% slowdown unresolved: Jinja integers at +1.35%
(interval -1.00% to +3.68%) and the one-grapheme case at +2.15% (interval +1.05%
to +3.08%). Several other cases show smaller slowdowns, including cl100k at
+1.39% and o200k at +0.95%. The gains cannot be described as universal.

The Apache case has a -3.23% overall median but cycle medians of -2.87% and
-3.55%; under the recorded rule it does not establish a repeatable 3% gain in
both cycles. All earlier measurements remain available in the first report.
The new comparison is additional evidence, not a replacement for unfavorable
runs.

## All 29 cases

Negative percentages mean less elapsed time. Intervals are descriptive and
pointwise; see the analysis limitations below. The last column counts slower
candidate processes out of 16 paired comparisons.

| Case                                 |  Median | 95% interval      | Cycle 1 / cycle 2 | Slower pairs |
| ------------------------------------ | ------: | ----------------- | ----------------- | -----------: |
| `django_email_domain`                |  +0.04% | -0.47% to +0.43%  | +0.04% / -0.19%   |         9/16 |
| `django_email_local_part`            |  +0.54% | +0.32% to +0.79%  | +0.88% / +0.54%   |        12/16 |
| `django_url_regex`                   |  +0.19% | -0.11% to +0.60%  | +0.19% / +0.48%   |        10/16 |
| `ferrolink_attribute_split`          |  -1.55% | -2.01% to -0.55%  | -1.32% / -1.97%   |         3/16 |
| `ferrolink_markup_scan`              | -10.29% | -10.39% to -9.10% | -10.31% / -10.27% |         0/16 |
| `fluent_bit_apache`                  |  -3.23% | -3.93% to -2.76%  | -2.87% / -3.55%   |         2/16 |
| `fluent_bit_nginx`                   |  -2.31% | -3.28% to -1.81%  | -2.31% / -2.62%   |         1/16 |
| `ipv4_validation`                    |  +0.37% | +0.13% to +0.86%  | +0.38% / +0.30%   |        11/16 |
| `ipv6_validation`                    |  +0.61% | -0.08% to +1.09%  | +1.24% / +0.48%   |        11/16 |
| `jinja_float_token`                  |  -0.31% | -1.25% to +0.67%  | +0.12% / -0.34%   |         7/16 |
| `jinja_integer_token`                |  +1.35% | -1.00% to +3.68%  | +0.52% / +2.38%   |        10/16 |
| `jinja_string_token`                 |  -3.67% | -6.51% to -0.88%  | -3.67% / -4.45%   |         2/16 |
| `literal_absent`                     |  -0.01% | -0.03% to +0.92%  | +0.03% / -0.01%   |         8/16 |
| `literal_end`                        |  -0.66% | -1.63% to +0.16%  | -0.53% / -0.66%   |         7/16 |
| `literal_first`                      |  +1.41% | +0.42% to +1.65%  | +0.55% / +1.74%   |        10/16 |
| `literal_middle`                     |  -0.05% | -0.63% to +0.12%  | -0.67% / +0.24%   |         8/16 |
| `mastodon_hashtag_regex_step`        |  +0.07% | -0.82% to +1.77%  | +0.28% / -0.26%   |         8/16 |
| `python_markdown_smart_emphasis_re`  |  +1.28% | +1.04% to +1.54%  | +2.07% / +1.07%   |        13/16 |
| `python_markdown_smart_strong_em_re` |  +0.75% | +0.13% to +1.15%  | +1.01% / +0.54%   |        10/16 |
| `rails_underscore_regex_step`        |  +0.59% | +0.14% to +1.06%  | -0.67% / +0.94%   |         9/16 |
| `semver_hyphen_range_captures`       |  +5.90% | +1.39% to +6.93%  | +5.90% / +4.99%   |        15/16 |
| `semver_version_parse`               |  +0.48% | +0.02% to +1.62%  | +1.34% / +0.23%   |        12/16 |
| `symfony_grapheme_chunks_1`          |  +2.15% | +1.05% to +3.08%  | +1.77% / +3.60%   |        14/16 |
| `symfony_grapheme_chunks_2`          |  +1.60% | +0.71% to +2.80%  | +1.66% / +1.32%   |        14/16 |
| `tiktoken_cl100k_base`               |  +1.39% | +0.74% to +1.66%  | +0.83% / +1.46%   |        13/16 |
| `tiktoken_o200k_base`                |  +0.95% | +0.47% to +2.20%  | +1.04% / +0.95%   |        14/16 |
| `uuid_shape`                         |  +0.26% | -0.37% to +0.90%  | +0.05% / +0.44%   |         9/16 |
| `uuid_v4`                            |  +0.47% | -0.13% to +0.65%  | +0.16% / +0.88%   |        10/16 |
| `wordpress_shortcode_captures`       |  +0.27% | -0.64% to +1.39%  | +0.27% / +0.60%   |        10/16 |

## A/A control

The two executables are byte-identical copies of the baseline. These results
show the variation present on this host even without a version change.

| Case                                | Median | 95% interval     | Pair range       |
| ----------------------------------- | -----: | ---------------- | ---------------- |
| `ferrolink_markup_scan`             | -0.60% | -1.61% to +0.51% | -1.90% to +2.89% |
| `jinja_integer_token`               | +0.35% | -0.38% to +1.13% | -0.50% to +2.36% |
| `jinja_string_token`                | -0.23% | -2.58% to +0.46% | -4.16% to +5.47% |
| `python_markdown_smart_emphasis_re` | +0.39% | -2.75% to +1.66% | -3.72% to +3.30% |
| `semver_hyphen_range_captures`      | +0.18% | -1.33% to +1.37% | -4.73% to +1.71% |
| `semver_version_parse`              | -0.13% | -1.20% to +4.11% | -2.75% to +4.11% |
| `tiktoken_cl100k_base`              | -0.14% | -1.05% to +0.89% | -1.22% to +1.42% |

## Retained evidence

[results.json](results.json) contains every pair, both cycles, order and layout
splits, intervals, and classifications under the recorded plan. [context.json](context.json)
identifies the source, build, lockfile, and binaries. The [evidence archive](evidence.tar.gz)
contains all **1,040 completed processes**: 928 A/B and 112 A/A, including raw
Criterion samples, process logs, execution order, timestamps, load averages,
the unchanged candidate patch, and the analysis audit. No sample or process
was removed. The archive checksum is in `evidence.sha256`.

All process oracle checks passed. The analyzer verified that all pairs are
complete and that each case visits each layout with opposite binary order.
The runner and analyzer compile with Python 3.14; constant-effect sanity checks
for the interval calculation pass. Source, lockfile, corpus and binary hashes
still match their pre-run snapshots. The existing full runtime tests, Clippy,
and Rustdoc results from the earlier report remain applicable; they were not
rerun because this task changes only diagnostics and documentation. Hosted CI,
coverage instrumentation and the full documentation-site build were not run.

## Method

The [recorded plan](PLAN.md) defines the matrix and interpretation rules. All
29 executable production/tokenizer cases receive 16 paired measurements: eight
environment-size offsets, each revisited with the opposite binary order. Case
order is shuffled per repetition. The environment padding base is fixed across
the entire run, and executable paths have equal length. Each Criterion process
uses 30 samples, 0.5 seconds of warmup and 1.5 seconds of steady measurement.

Before that matrix, seven selected cases receive eight A/A pairs using two
byte-identical copies of the old binary. This is a check of uncontrolled
variation, not a correction subtracted from A/B. Every benchmark process
validates its source oracle before measuring. Raw samples, process confidence
intervals, execution order, timestamps, load averages, and all outliers remain
in the evidence.

Results describe the standard Rust API's steady fixture batches, not regex
construction, first use, an entire application, or another platform. The host
is an Apple M1 Ultra with 20 logical CPUs and 64 GiB of memory, running macOS
27.0.1. The retained binaries were built with Rust 1.99.0 / LLVM 23.1.1, the bench
profile and ThinLTO, default `dfa-prefilter` plus `ffi`, and `PCRE2_SYS_STATIC=1`.
The lockfile, source patch, binary hashes and prior correctness evidence still
match. No rebuild, runtime edit, or dependency update occurs in this verification.

The machine is interactive, with other user and system processes running.
CPU affinity and background load are uncontrolled. Before timing, `pmset`
reported no recorded thermal/performance warning and no recorded CPU power
status. No builds, tests or profiles from this task run during measurement.

## Analysis

Percent changes compare each candidate process mean with its paired baseline;
negative means less elapsed time. The point estimate is the median of all
16 pair changes. Both eight-pair cycle medians, pair counts, layout/order splits
and every pair are retained.

Descriptive 95% intervals use 10,000 percentile-bootstrap resamples, grouped by
environment offset. Both opposite-order pairs stay together in each resampled
layout cluster. These intervals are not corrected for 29 comparisons or
uncontrolled host activity and are not simultaneous or cross-platform guarantees.
The plan requires the median and both cycle medians to cross the 3% target,
with the interval excluding zero, before calling a target-sized effect repeatable.
An interval crossing +3% leaves a material slowdown unresolved.

## Reproduction

Retain the previous and optimized benchmark executables at equally long paths.
The build command and source patches are in the previous report. Run these
commands from the repository root with Python 3.12 or newer and fresh output
directories. `control1` must be a byte-identical copy of `baseline`.

```sh
python3 benches/diagnostics/possessive-negated-verification-20261022/compare.py \
  /tmp/ferroni-perf/baseline /tmp/ferroni-perf/control1 /tmp/ferroni-perf/control-aa \
  --cases semver_version_parse,semver_hyphen_range_captures,jinja_integer_token,jinja_string_token,ferrolink_markup_scan,python_markdown_smart_emphasis_re,tiktoken_cl100k_base \
  --repetitions 8 --samples 30 --warmup 0.5 --measure 1.5

python3 benches/diagnostics/possessive-negated-verification-20261022/compare.py \
  /tmp/ferroni-perf/baseline /tmp/ferroni-perf/select01 /tmp/ferroni-perf/broad \
  --cases all --repetitions 16 --samples 30 --warmup 0.5 --measure 1.5

python3 benches/diagnostics/possessive-negated-verification-20261022/analyze.py \
  /tmp/ferroni-perf /tmp/ferroni-perf/results.json
```

The runner reuses the repository's catalog validation, environment padding,
and estimate reader. It adds a whole-run padding base, opposite orders for
each case at the same layout, two-digit repetition directories, and per-process
load/timestamp metadata. The analyzer audits pair completeness and order balance.
The existing runtime test results remain applicable because the source and
executables are unchanged. This task does not run hosted CI or alter published
Blacksmith figures.
