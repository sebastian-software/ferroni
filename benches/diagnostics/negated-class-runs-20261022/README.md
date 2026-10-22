# Local negated-class loop investigation

This is a local diagnostic on an Apple M1 Ultra, not a replacement for the
published Blacksmith measurements. The baseline is
`7e67f04` (Ferroni 2.3.0). The measured candidate changes `src/regexec.rs` only;
the retained patch identifies its complete source change.

## Results

Time reductions below are medians of paired process means. The broad comparison
uses three repetitions, 30 samples, 0.5 s warmup, and 1.5 s measurement per
process. The alignment follow-up uses eight repetitions, 30 samples, 0.5 s
warmup, 2 s measurement, and offsets 0, 8, 16, 24, 32, 40, 48, and 56 bytes.

| Workload | Less elapsed time | Paired range | Evidence |
| --- | ---: | ---: | --- |
| `fluent_bit_nginx` | 14.26% | 13.13% to 14.39% | 3 pairs |
| `fluent_bit_apache` | 6.28% | 5.68% to 8.58% | 3 pairs |
| `ferrolink_attribute_split` | 3.88% | 3.77% to 5.72% | 3 pairs |
| `ferrolink_markup_scan` | 3.38% | 2.59% to 5.32% | 8 alignments |

These are four workload-specific gains above the 3% target, with measured
tradeoffs. In the eight-alignment follow-up, cl100k took **0.94% more time**
and the short two-grapheme case **1.86% more time**. Their broad-run medians
were +2.83% and +0.91%; one grapheme process pair reached +17.85%, which did
not repeat in the other broad pairs or the alignment follow-up. No sample or
process was removed. This is not a claim of a universal 3% improvement.

The generated untracked VM frame shrank by 32 bytes (0x250 to 0x230 bytes
beyond the register-save area), so even patterns that never execute the changed
opcode can move through code generation and stack placement. For example, the
SemVer hyphen-range case improved by 7.47% in the broad run but has no negated
class loop; it is retained below, not attributed to the delimiter optimization.

All paired changes from the broad comparison (negative means less time):

| Case | Median change | Three paired changes |
| --- | ---: | --- |
| `uuid_shape` | +0.05% | +0.05%, +0.44%, -0.12% |
| `uuid_v4` | -3.41% | -3.41%, -4.04%, -0.61% |
| `ipv4_validation` | +0.06% | +0.06%, +0.83%, -0.87% |
| `ipv6_validation` | -0.78% | -0.99%, +0.62%, -0.78% |
| `semver_version_parse` | +2.09% | +3.99%, -5.72%, +2.09% |
| `semver_hyphen_range_captures` | -7.47% | -9.79%, -6.84%, -7.47% |
| `jinja_string_token` | +0.22% | +0.22%, -3.09%, +2.79% |
| `jinja_integer_token` | -0.58% | -0.58%, -1.01%, -0.49% |
| `jinja_float_token` | -0.78% | -0.78%, +0.66%, -4.40% |
| `django_email_local_part` | +0.38% | +0.38%, +0.29%, +2.12% |
| `django_email_domain` | +0.64% | +0.15%, +0.64%, +1.11% |
| `django_url_regex` | +0.33% | +0.40%, +0.22%, +0.33% |
| `fluent_bit_apache` | -6.28% | -8.58%, -5.68%, -6.28% |
| `fluent_bit_nginx` | -14.26% | -14.26%, -13.13%, -14.39% |
| `rails_underscore_regex_step` | -0.42% | -0.02%, -0.42%, -0.45% |
| `mastodon_hashtag_regex_step` | -2.49% | -0.99%, -2.49%, -3.60% |
| `python_markdown_smart_emphasis_re` | -0.82% | -0.82%, -3.05%, +2.60% |
| `python_markdown_smart_strong_em_re` | -2.68% | -2.68%, -2.41%, -2.97% |
| `wordpress_shortcode_captures` | -0.44% | +0.75%, -3.57%, -0.44% |
| `symfony_grapheme_chunks_1` | -2.08% | +0.77%, -2.23%, -2.08% |
| `symfony_grapheme_chunks_2` | +0.91% | +17.85%, +0.91%, +0.89% |
| `literal_first` | -0.22% | -0.22%, -0.23%, +1.43% |
| `literal_middle` | +0.85% | +0.85%, +1.52%, +0.19% |
| `literal_end` | +0.68% | -0.86%, +0.68%, +1.00% |
| `literal_absent` | +0.32% | -1.30%, +0.32%, +1.20% |
| `ferrolink_markup_scan` | -4.00% | -4.00%, -4.53%, -2.93% |
| `ferrolink_attribute_split` | -3.88% | -5.72%, -3.77%, -3.88% |
| `tiktoken_cl100k_base` | +2.83% | +0.96%, +4.74%, +2.83% |
| `tiktoken_o200k_base` | +0.80% | +0.80%, -0.34%, +3.97% |

Absolute per-process estimates and confidence intervals are in the evidence
archive; [results.json](results.json) also gives the separate medians of the
baseline and candidate times. A ratio of those separate medians need not equal
the median paired change reported here.

The [evidence archive](evidence.tar.gz) retains the raw Criterion samples, process logs, execution order,
binary hashes, baseline source snapshot, candidate patch, profiler samples,
build logs, and validation logs. [baseline.json](baseline.json) contains the
initial production characterization, including compilation and first use.
The final paired comparison measures steady reuse only; it does not establish
a construction-cost improvement. [context.json](context.json) records the host
and build context.

## What the profiles identified

The first pass ran all 27 executable production cases through their source
oracles and measured steady reuse, construction, and first use. The two pending
catalog entries have no timing. The subsequent comparison also includes both
tiktoken cases, for 29 executable cases in total.

A five-second macOS sample of `ferrolink_markup_scan` attributed approximately
30% of main-thread samples to the negated-class character loop, 47% to the VM
itself, and 4% to discarding star-loop choices. Its long negated classes include
quoted HTML attributes. The tiktoken o200k profile was dominated by the VM and
search machinery; it is a useful control for changes to shared VM code.

Disassembly of the original negated byte-class loop showed that every ASCII
byte loaded and dispatched on `CClassAsciiFastKind`, even though this value
cannot change during a run. Moving that decision outside the loop lets Rust
specialize the existing walker for equality, folded equality, and a bitset.
The bytecode, character boundaries, and backtrack entries are unchanged.

A second guarded path uses `memchr` or `memchr2` when at least 32 bytes remain
and the excluded class is one ASCII byte or an ASCII upper/lowercase pair.
It accepts the resulting run only when its entire prefix is ASCII. Otherwise
it uses the original character walker. This guard matters for malformed byte
input: a UTF-8 lead byte can consume an ASCII quote as a trail byte, so an
unconditional delimiter search would change the result. The fast path creates
the same lazy backtrack entry as the ASCII character walk.

Both changes are limited to the existing Rust-only `CClassNotStar` path.
They add no allocation, dependency, unsafe code, or compiled-pattern metadata.
The additional scans are linear in the current run's bytes. Other classes keep
the character walker, including classes containing non-ASCII members.

Discarding possessive-loop choices was another profile lead. This change does
not fuse those instructions: their retry and stack-limit behavior stays intact.

## Measurement method

- Apple M1 Ultra, 20 logical CPUs, 64 GiB memory, macOS arm64.
- Rust 1.99.0 / LLVM 23.1.1; the repository's bench profile, optimization level 3
  and ThinLTO; no native-CPU, PGO, allocator, or panic-strategy changes.
- Default `dfa-prefilter` plus `ffi`; `PCRE2_SYS_STATIC=1`; the locked dependencies
  and pinned C source inputs. The measured engine is Ferroni's standard API.
- Each timed process validates its case against the retained source oracle.
- Independent Criterion processes, balanced baseline/candidate order, shuffled
  case order, equal-length binary paths, and environments padded using
  `compare-production.py` with offsets of 0, 24, and 48 bytes across repetitions.
- Raw samples and per-process mean confidence intervals are retained. Reported
  changes are medians of paired process-mean changes, not the fastest sample.
- Agent builds, tests, profiling, and timing ran in separate phases. This was
  an interactive local host; other system activity and CPU affinity were not
  controlled. No thermal or performance warning was reported by `pmset`.
- Steady times describe a whole case's fixture batch. Different cases have
  different fixture counts and input sizes; their absolute times are not a
  ranking of regex complexity or a traffic-weighted application result.

The exploratory specialization-only comparison used three paired repetitions,
20 samples, 0.3 s warmup, and 1 s measurement. The median time reductions were
6.6% for ferrolink attribute splitting and 6.8% for Fluent Bit Apache parsing.
The ASCII scan then reduced HTML markup scanning by another 4.8% in two short
paired repetitions. These probes selected the candidate; the final comparison
below is the evidence for the combined change.

## Reproduction

Build the baseline revision and the retained candidate patch with the same
source path, toolchain, lockfile, and command, saving each executable before the
next build:

```sh
PCRE2_SYS_STATIC=1 cargo bench --locked --offline --features ffi \
  --bench production_regex_bench --no-run
```

The C inputs must first be prepared with the repository's preparation scripts.
Copy the resulting executables to equally long paths, for example
`/tmp/ferroni-perf/base0001` and `/tmp/ferroni-perf/cand0001`. Do not overwrite
an executable while measuring it. Run from the repository root, with a fresh
output directory:

```sh
python3 benches/diagnostics/negated-class-runs-20261022/compare.py \
  /tmp/ferroni-perf/base0001 /tmp/ferroni-perf/cand0001 /tmp/ferroni-perf/paired \
  --cases all --repetitions 3 --samples 30 --warmup 0.5 --measure 1.5
```

`--cases` also accepts comma-separated case IDs; `--engines` accepts `rust` and
`rust_erroraware`; `--lanes` accepts `steady`, `compile`, and `first_use`.
The script deliberately labels every run diagnostic. It does not publish or
modify the repository's published numbers.

## Correctness and cost checks

The candidate passed:

- `cargo fmt --all --check`.
- `PCRE2_SYS_STATIC=1 cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`.
- `PCRE2_SYS_STATIC=1 cargo test --locked --offline --features ffi --lib`.
- Every integration test target in `tests/*.rs`, with `ffi` and
  `--test-threads=1`. The retained `integration-command.json` lists the exact
  command. This includes the C compatibility suites and malformed UTF-8,
  capture, retry/stack-limit, scanner, and shared-regex regression tests.
- `cargo test --locked --offline --doc`.
- `cargo test --locked --offline --features prefilter-self-check --lib --test api_test --test compat_prefilter_differential --test scanner_prefilter_regressions -- --test-threads=1`.
- `python3 scripts/check-production-corpus.py` and Python compilation of the
  diagnostic runner.

Existing ignored opt-in tests remain ignored, including the external Ferriki
corpus. No Linux timing or end-to-end highlighting timing was collected.
The C dependencies emitted their existing compiler warnings; the Rust Clippy
check completed with warnings denied.

The original parallel prefilter test run failed
`regcomp::tests::nested_counted_loops_compile_to_bounded_bytecode` with stack-limit
error -15 instead of a match. That test does not use `CClassNotStar` and does not
hold the shared global-limit test lock. The complete serial repetition passed
without changing the test or engine code. Both logs are retained rather than
hiding the failed run.

The expanded differential tests compare negated-star instructions with bounded
character repeats, including equality, ASCII upper/lowercase pairs, bitsets,
ASCII runs around the threshold, NUL, Unicode, malformed/truncated UTF-8,
backtracking, captures, constrained search ranges, and backward searches.

The measured benchmark executable's `__TEXT` segment grew by 16 KiB, from
6,602,752 to 6,619,136 bytes (0.25%). This is a size observation for this linked
harness, not a promise about the size of a downstream application.
