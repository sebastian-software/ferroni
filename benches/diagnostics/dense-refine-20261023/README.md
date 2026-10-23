# Bounded adaptive single-regex DFA refinement

The refined implementation replaces the general dense builder with a bounded
bitset construction, preserves the hybrid route for capture/find calls, and
keeps optional promotion metadata outside the hybrid search object.

- Local-Part: -41.00% in the independent 16-pair focus comparison (-41.11% to -40.88%).
- UUID v4: -14.10% in the independent 16-pair focus comparison (-14.24% to -13.95%).
- Jinja Integer: -0.19% in the independent 16-pair focus comparison (-0.66% to -0.04%).
- Jinja String: +0.06% in the longer 16-pair capture comparison (-0.36% to +1.19%).
- Semver: -0.04% in the longer 16-pair capture comparison (-0.13% to +0.05%).

These are local elapsed-time changes against the frozen runtime baseline,
not end-to-end application throughput. The full warm matrix, construction
lanes, direct comparison with the previous adaptive prototype, and all smaller
costs are reported below. No arithmetic average is used to hide case costs.

For a fresh Local-Part regex with only 16 whole fixture batches, the change is +0.57% versus baseline and -43.13% versus the previous adaptive prototype. The setup cost is reduced, not eliminated for every short lifetime.

For a fresh UUID regex with only 16 whole fixture batches, the change is +23.40% versus baseline and -33.93% versus the previous adaptive prototype. The setup cost is reduced, not eliminated for every short lifetime.

Smaller warm costs remain outside the main targets, including IPv6 at +2.35%; the complete matrix follows.

## Implementation and scope

The runtime baseline is `9b85954dba9a37d58f93d5b62870b8788c8e521b` (2.4.0),
and the previous small adaptive prototype is
`37d406c75565408671c8192327b2630c4403114e`. The selected source is
`c5fa681dfb649d81bfe3e4070673bc9b15b26887`, following bitset construction at
`e61b25b` and reject-only dispatch at `50f8e6f`. This is not an exact-current-main
comparison. Main was `817d2b3` when initially checked; subsequent scanner and
archive changes are outside this frozen experiment.

- Build Boolean subsets of the existing public Thompson NFA with a `u64`;
  no new dependency, `unsafe`, or regex-automata `dfa-build` feature.
- Admit at most 64 actual NFA states, byte-mode empty-match semantics, and
  absolute text-start/text-end assertions. The separate 512-state HIR estimate
  is only an admission precheck. Unsupported NFAs stay hybrid.
- Preserve the original inline hybrid DFA and per-thread cache pool. Promote
  after 64 actual consultations of each eligible seek or accept DFA.
- Use dense seeks for complete Boolean decisions. Capture/find calls use the
  legacy hybrid search body without dense dispatch, counting, or promotion;
  positive results still go through the matcher for captures and positions.
- Publish one immutable table with the existing atomics/`OnceLock` policy.
  Release the cache guard before construction; concurrent readers can keep
  using hybrid. Memoize normal rejection and prevent retries after a panic.
- Bound table storage, including compaction headroom, by 64 KiB and subset
  workspace by 32 KiB. The budgets count data storage, excluding allocator
  bookkeeping; hybrid NFAs/caches and promotion metadata are additional.

Optional seek/accept promotion metadata lives in one boxed pair. Ineligible
NFAs allocate no pair and return directly to hybrid. A layout diagnostic on
this toolchain measured 1472 bytes for the baseline-shaped search object,
1600 with inline promotion fields, and 1488 with the boxed pair. The cache-pool
offset returns from 1584 to 1456, matching the baseline shape. An eligible pair
uses 128 additional heap bytes and one allocation. These are layout observations,
not Rust layout guarantees or proof that the offset alone caused a timing cost.
The 1029-state IPv6 NFA has no adaptive state.

An earlier feature-only control was slower for Jinja Integer and Semver even
without a dense runtime change.
Feature selection and generated-code placement therefore remain attribution
uncertainties. This refinement removes that feature and unnecessary adaptive
work from capture/find calls; it does not assign the entire former regression
to one branch or allocation.

## Warm independent focus

Sixteen pairs use eight environment-size offsets, revisited with the opposite
binary order. Negative values mean less elapsed time. Intervals are descriptive
95% cluster-bootstrap intervals, retaining both order pairs at each offset;
they are not corrected for multiple comparisons or host activity.

| Case                      |  Median | Descriptive 95% interval | Cycle 1 / cycle 2 |
| ------------------------- | ------: | ------------------------ | ----------------- |
| `django_email_local_part` | -41.00% | -41.11% to -40.88%       | -41.23% / -40.98% |
| `uuid_v4`                 | -14.10% | -14.24% to -13.95%       | -14.13% / -14.06% |
| `jinja_string_token`      |  -0.18% | -1.29% to +0.76%         | -0.41% / -0.18%   |
| `jinja_integer_token`     |  -0.19% | -0.66% to -0.04%         | -0.43% / -0.14%   |
| `semver_version_parse`    |  +0.83% | -0.07% to +3.25%         | +0.19% / +2.03%   |
| `ferrolink_markup_scan`   |  -0.17% | -0.53% to +0.05%         | -0.18% / -0.17%   |
| `fluent_bit_apache`       |  -0.30% | -2.28% to +0.57%         | -1.06% / -0.16%   |
| `ipv6_validation`         |  +2.21% | +2.10% to +2.33%         | +2.23% / +2.16%   |

The Semver focus interval reached +3.25% and the independent broad Jinja
String interval reached +3.32%. The same frozen binaries therefore received
separately planned longer confirmation for both capture cases (16 pairs/eight
offsets, 1 s warm-up, 3 s measurement, 40 samples), plus an identical-binary
control. No phase is pooled or subtracted.

| Longer capture phase | Case                   | Median | Descriptive 95% interval | Cycle 1 / cycle 2 |
| -------------------- | ---------------------- | -----: | ------------------------ | ----------------- |
| Identical binary A/A | `semver_version_parse` | -0.08% | -0.32% to +0.26%         | +0.00% / -0.13%   |
| Identical binary A/A | `jinja_string_token`   | -0.04% | -0.17% to +0.10%         | +0.13% / -0.27%   |
| Refined vs baseline  | `semver_version_parse` | -0.04% | -0.13% to +0.05%         | -0.03% / -0.04%   |
| Refined vs baseline  | `jinja_string_token`   | +0.06% | -0.36% to +1.19%         | -0.14% / +0.35%   |

The CSV and decision details also retain pair ranges and the count of pairs
slower by at least 3%. A near-zero median is not a claim that every process
layout or individual pair is unchanged.

## All executable warm cases

This independent matrix has eight pairs per case over four offsets and their
opposite orders. It includes all 27 executable production cases and both
tokenizer cases. The two catalog entries without executable fixtures remain
pending and are not timed.

| Case                                 |  Median | Descriptive 95% interval | Cycle 1 / cycle 2 |
| ------------------------------------ | ------: | ------------------------ | ----------------- |
| `django_email_domain`                |  +1.41% | +0.84% to +2.27%         | +1.38% / +2.24%   |
| `django_email_local_part`            | -40.77% | -40.97% to -40.64%       | -40.91% / -40.77% |
| `django_url_regex`                   |  +0.73% | -0.04% to +1.47%         | +1.06% / -0.12%   |
| `ferrolink_attribute_split`          |  -0.63% | -1.34% to +0.31%         | -0.18% / -0.63%   |
| `ferrolink_markup_scan`              |  -0.08% | -0.44% to +0.05%         | +0.19% / -0.79%   |
| `fluent_bit_apache`                  |  -0.31% | -0.90% to +0.95%         | +0.02% / -0.63%   |
| `fluent_bit_nginx`                   |  -0.09% | -0.89% to +1.68%         | +0.65% / -0.20%   |
| `ipv4_validation`                    | -38.29% | -38.40% to -38.28%       | -38.39% / -38.29% |
| `ipv6_validation`                    |  +2.35% | +2.12% to +2.82%         | +2.59% / +2.35%   |
| `jinja_float_token`                  |  -0.05% | -0.35% to +0.96%         | +0.38% / -0.45%   |
| `jinja_integer_token`                |  -0.12% | -2.25% to +0.34%         | -0.35% / -0.08%   |
| `jinja_string_token`                 |  +0.74% | -0.24% to +3.32%         | +2.54% / +0.74%   |
| `literal_absent`                     |  -0.22% | -0.45% to +0.04%         | -0.22% / -0.06%   |
| `literal_end`                        |  +0.35% | -0.05% to +0.70%         | -0.23% / +0.57%   |
| `literal_first`                      |  -0.27% | -0.94% to +0.58%         | +0.41% / -1.30%   |
| `literal_middle`                     |  +0.37% | -0.07% to +0.89%         | +0.47% / -0.17%   |
| `mastodon_hashtag_regex_step`        |  +1.93% | +0.59% to +2.55%         | +1.27% / +1.93%   |
| `python_markdown_smart_emphasis_re`  |  +0.23% | -1.16% to +0.60%         | +0.04% / +0.57%   |
| `python_markdown_smart_strong_em_re` |  +0.05% | -0.37% to +2.28%         | -0.08% / +0.15%   |
| `rails_underscore_regex_step`        |  +1.33% | +0.42% to +1.59%         | +1.33% / +1.17%   |
| `semver_hyphen_range_captures`       |  +0.54% | -2.49% to +2.10%         | +0.06% / +0.54%   |
| `semver_version_parse`               |  -0.17% | -0.61% to +0.09%         | -0.53% / +0.18%   |
| `symfony_grapheme_chunks_1`          |  -0.47% | -0.94% to +0.23%         | -0.47% / -0.61%   |
| `symfony_grapheme_chunks_2`          |  +2.57% | +0.14% to +2.57%         | -2.35% / +3.20%   |
| `tiktoken_cl100k_base`               |  -0.34% | -1.34% to -0.14%         | -1.58% / -0.08%   |
| `tiktoken_o200k_base`                |  -0.21% | -0.52% to +0.34%         | -0.11% / -0.40%   |
| `uuid_shape`                         |  -8.26% | -8.96% to -7.91%         | -8.52% / -7.80%   |
| `uuid_v4`                            | -13.96% | -14.17% to -13.57%       | -13.99% / -13.72% |
| `wordpress_shortcode_captures`       |  -1.10% | -3.32% to +1.13%         | -1.65% / +0.67%   |

The strict local rule finds 4 case gains and
0 case costs of at least 3% in both cycles,
with the interval excluding zero. [decision.json](decision.json) records every
positive median as well; a smaller cost or an interval reaching 3% is not proof
of no regression.

## Previous small adaptive prototype

These are fresh direct comparisons with `37d406c`, not ratios reconstructed
from separate baseline phases. Each row has eight pairs/four offsets.

| Case                      |  Median | Descriptive 95% interval | Cycle 1 / cycle 2 |
| ------------------------- | ------: | ------------------------ | ----------------- |
| `django_email_local_part` | -31.90% | -32.66% to -30.43%       | -31.61% / -32.21% |
| `uuid_v4`                 |  -7.13% | -7.75% to -6.85%         | -7.36% / -7.13%   |
| `jinja_string_token`      |  +0.39% | -0.87% to +3.85%         | +0.02% / +1.87%   |
| `jinja_integer_token`     |  -3.38% | -5.16% to -1.94%         | -4.55% / -1.99%   |
| `semver_version_parse`    |  -1.09% | -4.78% to -0.41%         | -2.02% / -1.09%   |
| `ferrolink_markup_scan`   |  -0.75% | -0.87% to +0.26%         | -0.93% / +0.43%   |
| `fluent_bit_apache`       |  +0.12% | -0.86% to +0.48%         | +0.12% / -0.50%   |
| `ipv6_validation`         |  +1.67% | +1.21% to +1.95%         | +1.38% / +1.90%   |

## Construction and reuse

Each lane starts with a fresh compiled regex. `compile` measures compilation
and required engine context; lazy `SingleSeek` construction happens later.
`first_use` adds execution on the first fixture input. Reuse lanes add N full
fixture batches, not N individual searches. Local-Part has 58 inputs per batch;
UUID has 11. Regex destruction is outside the measured region in all these
lanes, matching the diagnostic harness. Each row has eight counterbalanced pairs.

| Case                      | Compile | First use | 1 batch | 16 batches | 256 batches | 1024 batches |
| ------------------------- | ------: | --------: | ------: | ---------: | ----------: | -----------: |
| `django_email_local_part` |  -0.55% |    +0.59% |  +0.66% |     +0.57% |     -33.06% |      -38.92% |
| `uuid_v4`                 |  +0.70% |    +0.69% |  +0.30% |    +23.40% |      -1.51% |      -10.14% |
| `jinja_string_token`      |  -0.31% |    +0.03% |  +0.82% |     +0.53% |      +0.25% |       +0.41% |
| `jinja_integer_token`     |  +0.02% |    -0.76% |  +0.09% |     -0.13% |      -0.30% |       -0.97% |
| `semver_version_parse`    |  -0.75% |    -0.08% |  -0.05% |     +0.28% |      -0.41% |       +0.82% |
| `django_email_domain`     |  +0.57% |    +0.19% |  +0.29% |     +0.50% |      +0.53% |       +0.68% |
| `django_url_regex`        |  +0.32% |    -0.04% |  +0.00% |     -0.26% |      +0.71% |       +0.48% |

Direct fresh-regex reuse comparison with the previous adaptive prototype:

| Case                      | Batches |  Median | Descriptive 95% interval | Cycle 1 / cycle 2 |
| ------------------------- | ------: | ------: | ------------------------ | ----------------- |
| `django_email_local_part` |       1 |  +0.86% | +0.12% to +1.60%         | +0.44% / +1.11%   |
| `django_email_local_part` |      16 | -43.13% | -43.66% to -43.00%       | -43.01% / -43.30% |
| `django_email_local_part` |     256 | -35.63% | -36.01% to -35.53%       | -35.44% / -35.73% |
| `django_email_local_part` |    1024 | -33.55% | -33.56% to -33.23%       | -33.41% / -33.55% |
| `uuid_v4`                 |       1 |  +0.53% | +0.23% to +1.55%         | -0.41% / +0.89%   |
| `uuid_v4`                 |      16 | -33.93% | -34.23% to -33.37%       | -33.82% / -33.99% |
| `uuid_v4`                 |     256 | -20.44% | -20.61% to -20.43%       | -20.92% / -20.27% |
| `uuid_v4`                 |    1024 | -11.86% | -12.30% to -11.30%       | -11.86% / -11.77% |

[results.csv](results.csv) includes every construction/reuse interval, cycle,
paired count, and absolute process-median duration, including the other cold
cases. Warm results alone do not establish a win for short-lived regexes.

The motivating in-process builder probe measured the first bitset version at
about 6.38 µs for Local-Part and 9.92 µs for UUID, versus 47.13 and 35.08 µs for
the generic dense builder (128 builds each). That diagnostic predates final
compaction/integration and is not substituted for the fresh-regex evidence.
Changing generic-builder start kinds or acceleration was insufficient. Direct
hybrid-cache state export was rejected because retained IDs violate its public
API contract; replaying paths to respect that contract removed the advantage.

## Correctness and controls

Formatting and all-target/all-feature Clippy with `-D warnings` passed. The
broad feature-enabled library/API/compatibility/scanner suites, matcher/C
catalog oracles, and the full parallel library suite without default features
passed for the bitset implementation. After each API/layout follow-up, the
focused promotion/differential tests, reject-only policy test, 314 API tests,
all six matcher/C catalog tests, Clippy, and the no-default-features all-target
check passed again. The gates and their exact logs are retained.

After timing finished, the selected immutable binary also passed 237568 repeated oracle-fixture assertions: 8192 public operations for each of all 29 executable cases. This covers repeated use through promotion. Those individual trace durations include timer/harness overhead and are not performance acceptance evidence.

The extra parallel library run exposed an existing test race: a counted-loop
test observed another test's temporary global match-stack limit (`-15`).
Commit `700d6eb` adds the existing `shared_limits` read guard to that test.
The full parallel rerun passed 513 tests. This changes no production code.

Bitset tests cover all 256 predecessor bytes, empty input, Unicode bytes,
nonzero starts, EOI-delayed anchors, the 64/65-state boundary, unsupported
assertions, bounded rejection, panic fallback, and concurrent promotion. The
reject-only test checks no counting or promotion before and after Boolean calls
promote the same automaton. No archive/wire format changes are made.

All immutable binaries were built at the same canonical package path with
Rust 1.99.0 / LLVM 23.1.1, optimized bench profile with ThinLTO, default features
plus `ffi`, locked dependencies, and one common diagnostic harness. Both catalogs
and all executable oracle-validation records agree byte-for-byte. The baseline
and previous prototype were rebuilt after the test-only guard; their retained
benchmark hashes are unchanged.

The identical-binary A/A medians range from -0.27%
to +0.57% across seven cases; some descriptive
intervals remain wider than 3%. No control result is subtracted.

## Host conditions and retained exploration

This is an interactive M1 Ultra host (20 logical CPUs, 64 GiB), not an isolated
benchmark machine. Before each selected pair, the runner requires one-minute
load at most 10 and at least 20 seconds since the last observed value above
that threshold. Both processes then run consecutively. Every wait, process
load, order, and environment offset is retained. The gate cannot prevent a new
background burst inside a pair; CPU affinity and frequency were not controlled.
No task-owned builds, tests, or profiling overlap selected timings. External
processes were never stopped for this work.

The first bitset screen remains diagnostic. Two later screen labels
accidentally reused that first executable; hashes and an attribution correction
are retained, and those phases provide no evidence for their intended source
changes. The first correctly attributed `bits0004` screen was strongly loaded
and is excluded from selection. Its broad run (114 complete pairs) was
superseded by the reject-only API fix.

The complete `bits0005` focus had Local-Part -41.29%, UUID -13.72%, Jinja
Integer +0.19%, and Semver +0.07%. Its partial broad run (101 complete pairs)
suggested an IPv6 cost, prompting the compact metadata change. Neither partial
broad phase is presented as a completed eight-pair matrix. Both were stopped
during existing capacity waits at complete-pair boundaries, with no timed
Criterion child interrupted. All rows and raw samples remain in the archive.

The integrity audit verifies 2446 completed Criterion
processes, 1223 complete pairs, and
76830 raw sample records across selected and retained
exploratory phases. All selected phases are complete; the two superseded broad
phases are explicitly incomplete. Frozen binary identities and all archived
file hashes are checked.

## Evidence

- [Raw evidence archive](evidence.tar.gz) and [SHA-256](evidence.sha256).
- [All selected results](results.csv) and [decision details](decision.json).
- The archive includes source snapshots, manifests, build/oracle/gate logs,
  runners, dependent analysis tools, contexts, phase status, all raw Criterion
  samples and estimates, and integrity audits. Frozen executable bytes and
  redundant build targets are excluded; their hashes are retained.
