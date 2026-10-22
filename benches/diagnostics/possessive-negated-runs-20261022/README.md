# Local possessive negated-class investigation

The [broader verification](../possessive-negated-verification-20261022/README.md)
now confirms a material SemVer hyphen-range regression: +5.90% across 16 paired
comparisons, with +5.90% and +4.99% cycle medians under opposite binary orders.
It also confirms the markup gain at -10.29%. That follow-up recommends keeping
this candidate unmerged; the original measurements below remain unchanged.

The subsequent [root-cause investigation and revised implementation](../possessive-negated-fix-20261022/README.md)
retain this candidate's evidence and report separate comparisons against the
same previous version.

This follow-up measures Ferroni against `164bb809cea91c478fb5614d5c423922fc150197`,
after the first negated-class optimization was merged. It is a local Apple M1
Ultra diagnostic, not an update to the published Blacksmith benchmark results.

## Results

The clearest gain above the 3% target is HTML markup scanning: **11.08% less
elapsed time in the broad comparison and 10.58% less in the eight-layout
follow-up**. All eight follow-up pairs improve by 9.99% to 11.01%.

Jinja string tokenization improves by 1.79% in the broad comparison and 7.49%
in the follow-up. The latter ranges from 0.52% to 10.94%, so a 3% gain is not
consistent across its process layouts. Nginx improves by 2.53% in the broad
comparison, below the requested threshold.

There are measured tradeoffs. The broad run puts SemVer version parsing at
**+3.79%**, SemVer hyphen ranges at **+6.48%**, and Jinja integers at **+3.34%**.
The eight-layout medians are +0.82%, +0.26%, and +0.05%, respectively. The
hyphen-range pairs vary from -15.24% to +6.94%. Both runs are retained; the
follow-up does not erase the regressions or prove their absence on another
host. No process or sample was discarded, including the large Apache and
literal-first outliers in the broad run.

All changes below compare paired process means; negative means less time.
Each time describes a whole fixture batch. Cases differ in input count and
size, so they are not a traffic-weighted application benchmark.

### Broad comparison: all 29 executable cases

Three paired repetitions, 30 samples, 0.5 s warmup and 1.5 s measurement per
process; environment-size offsets 0, 24, and 48 bytes.

| Case                                 | Median change | Three paired changes      |
| ------------------------------------ | ------------: | ------------------------- |
| `uuid_shape`                         |        -0.18% | +1.23%, -0.18%, -1.00%    |
| `uuid_v4`                            |        +0.23% | +0.45%, +0.23%, -0.05%    |
| `ipv4_validation`                    |        +1.36% | +0.57%, +1.52%, +1.36%    |
| `ipv6_validation`                    |        +0.22% | -0.21%, +0.22%, +0.47%    |
| `semver_version_parse`               |        +3.79% | +3.00%, +4.30%, +3.79%    |
| `semver_hyphen_range_captures`       |        +6.48% | +6.48%, +5.65%, +6.61%    |
| `jinja_string_token`                 |        -1.79% | -2.92%, -1.79%, -1.09%    |
| `jinja_integer_token`                |        +3.34% | +3.39%, +3.34%, +2.25%    |
| `jinja_float_token`                  |        +1.18% | +1.18%, +0.97%, +2.09%    |
| `django_email_local_part`            |        +0.24% | +0.51%, +0.08%, +0.24%    |
| `django_email_domain`                |        -0.15% | -0.28%, -0.08%, -0.15%    |
| `django_url_regex`                   |        +0.42% | +0.94%, +0.36%, +0.42%    |
| `fluent_bit_apache`                  |        +0.51% | +32.88%, -0.39%, +0.51%   |
| `fluent_bit_nginx`                   |        -2.53% | -2.92%, -2.18%, -2.53%    |
| `rails_underscore_regex_step`        |        -0.47% | -1.11%, -0.47%, +0.48%    |
| `mastodon_hashtag_regex_step`        |        +0.80% | +0.80%, +3.20%, +0.03%    |
| `python_markdown_smart_emphasis_re`  |        +2.69% | +3.11%, +2.69%, +0.59%    |
| `python_markdown_smart_strong_em_re` |        +2.41% | +0.06%, +2.41%, +3.08%    |
| `wordpress_shortcode_captures`       |        -2.95% | -1.31%, -2.95%, -6.36%    |
| `symfony_grapheme_chunks_1`          |        +0.23% | +0.20%, +0.23%, +1.29%    |
| `symfony_grapheme_chunks_2`          |        -0.69% | -0.69%, -2.04%, +2.52%    |
| `literal_first`                      |        -0.38% | -34.29%, +0.06%, -0.38%   |
| `literal_middle`                     |        +0.33% | -0.04%, +0.33%, +1.48%    |
| `literal_end`                        |        -0.23% | +0.01%, -0.69%, -0.23%    |
| `literal_absent`                     |        -0.74% | -1.38%, -0.74%, +1.71%    |
| `ferrolink_markup_scan`              |       -11.08% | -10.93%, -11.39%, -11.08% |
| `ferrolink_attribute_split`          |        -2.23% | -1.67%, -4.17%, -2.23%    |
| `tiktoken_cl100k_base`               |        +0.95% | +0.95%, -0.70%, +1.22%    |
| `tiktoken_o200k_base`                |        +0.44% | +0.55%, +0.44%, -0.11%    |

### Follow-up: eight process layouts

Eight paired repetitions, 30 samples, 0.5 s warmup and 2 s measurement per
process; offsets 0, 8, 16, 24, 32, 40, 48, and 56 bytes.

| Case                           | Median change | Eight paired changes                                                  |
| ------------------------------ | ------------: | --------------------------------------------------------------------- |
| `ferrolink_markup_scan`        |       -10.58% | -9.99%, -10.69%, -10.98%, -10.63%, -11.01%, -10.28%, -10.30%, -10.52% |
| `semver_version_parse`         |        +0.82% | +0.75%, +2.06%, -0.48%, +1.04%, +0.90%, +0.34%, +0.51%, +4.34%        |
| `semver_hyphen_range_captures` |        +0.26% | +0.41%, -15.24%, -6.17%, +1.10%, +6.94%, -6.42%, +0.11%, +0.74%       |
| `jinja_integer_token`          |        +0.05% | +0.39%, -0.23%, +3.29%, -0.22%, -0.36%, -0.70%, +4.18%, +0.32%        |
| `jinja_string_token`           |        -7.49% | -0.52%, -6.57%, -8.70%, -8.47%, -7.88%, -7.10%, -10.94%, -1.29%       |

Absolute estimates are in [results.json](results.json). The evidence archive
also contains per-process confidence intervals, Criterion samples, order,
environment sizes, and process logs. Ratios of separate baseline/candidate
medians need not equal the median paired change reported here.

## Selected change

The compiler already represents a possessive negated byte-class loop as
`CClassNotStar; PopStarChoices`. The first instruction creates backtracking
choices; the second immediately deletes them. The matcher now recognizes
that pair and consumes the same characters without creating those choices
or checking the character boundaries needed only to retrace them. It then
advances past the pop.

The optimization requires an unset match stack limit. With a configured
limit, both original instructions execute and retain their temporary entries.
It removes no retry: the discarded choices could never be taken. Avoiding
a temporary entry can avoid the hard stack cap, as permitted for work left
out by ADR-017. Multibyte and mixed-class star instructions retain their
original paths, and byte input retains the same forward character decoder,
including its malformed-UTF-8 behavior.

The change adds no allocation, dependency, unsafe code, opcode, or compiled
pattern metadata. It specializes the existing walker with a const generic.
The linked harness instruction section grows by 3,712 bytes, from 3,931,384
to 3,935,096 bytes (0.094%). Its `__TEXT` segment remains 6,635,520 bytes.
Downstream size depends on the build.

### Binary provenance

The broad run measured `final001`; the layout follow-up measured `fusion01`.
The selected source builds as `select01`, with its differential switch gated
entirely behind `cfg(test)`. All three binaries have a byte-identical
`__TEXT,__text` instruction section, SHA-256
`9532959a5bfdbfc00b623219bb6ffdbc39266b3f626324248b2f6761a38ebf4e`.
Their complete binary hashes differ. [context.json](context.json) records
both kinds of hashes and the baseline. The retained patches and source
snapshots identify each build. In particular, the different SemVer results
in the broad and follow-up runs cannot be attributed to changed matcher
instructions between those candidate binaries.

## Other approaches measured

Five-second macOS profiles of markup, cl100k, and email-local-part searches
guided the experiments. In the markup sample, roughly half of main-thread
samples were in the VM, 15% in the generic negated-class walker, and 4% in
discarding star choices. Email-local-part searches spent much of their time
in the DFA prefilter; that path was not changed. Sampling is an approximate
attribution, not an instruction count.

| Trial      | Approach                                                               |  Markup | Jinja strings |
| ---------- | ---------------------------------------------------------------------- | ------: | ------------: |
| `three001` | Two/three excluded bytes via memchr2/memchr3                           |  -0.14% |        +4.41% |
| `direct01` | Direct comparisons for short two/three-byte classes                    |  +1.06% |        +3.98% |
| `fusion01` | Skip temporary choices and the following pop for possessive byte stars | -11.17% |        -7.24% |
| `fusion02` | Extend fusion to multibyte and mixed classes                           |  -4.99% |        +0.73% |
| `final001` | Byte-star fusion with differential-test toggle                         | -11.08% |        -1.79% |
| `compact1` | Out-of-line choice elision, retaining pop dispatch                     |  -1.97% |        -2.57% |
| `compact2` | Inline the byte walker into the out-of-line wrapper                    |  -4.48% |        -5.83% |
| `batch001` | Scan ASCII runs using Iterator::position                               |  +1.56% |        +5.87% |
| `unroll01` | Test four ASCII members per loop step                                  |  +6.82% |        +7.44% |

The short probes selected an implementation for broader checking; they are
not substitutes for the two comparisons above. Extending delimiter searches
to two/three excluded bytes missed the target. Extending possessive fusion
to mixed/multibyte classes diluted the gain. An out-of-line wrapper that kept
the pop dispatch reduced the benefit. Iterator-based ASCII scanning and
explicit four-byte membership batches regressed the target workloads. Their
patches and full measurements remain in [trials.json](trials.json) and the
[evidence archive](evidence.tar.gz).

## Measurement method and reproduction

- Apple M1 Ultra, 20 logical CPUs, 64 GiB, macOS 27.0.1 arm64.
- Rust 1.99.0 / LLVM 23.1.1; bench profile, optimization level 3, ThinLTO.
- Default `dfa-prefilter` plus `ffi`; `PCRE2_SYS_STATIC=1`; locked dependencies
  and pinned C sources. No native-CPU, allocator, PGO, or panic-mode changes.
- Standard Ferroni API, steady reuse only. No construction-cost improvement
  is established. Both catalogs were checked against the embedded catalogs;
  each timed process verifies the source oracle before measuring.
- Independent Criterion processes, balanced candidate/baseline order,
  shuffled cases, equal-length executable paths and padded environments.
- Builds, tests, profiles and timings ran in separate phases. This was an
  interactive host: external background load and CPU affinity were not
  controlled, and thermal queries were unavailable inside the sandbox.
- No Linux, x86, end-to-end highlighting, or external Ferriki corpus timing.

Build the baseline and the candidate separately at the same source path,
with the same lockfile and toolchain, and retain each executable:

```sh
PCRE2_SYS_STATIC=1 cargo bench --locked --offline --features ffi \
  --bench production_regex_bench --no-run
```

Prepare the pinned C inputs using the repository scripts first. Save the
binaries to equally long paths such as `/tmp/ferroni-perf/base0001` and
`/tmp/ferroni-perf/cand0001`, then use the diagnostic runner from the first
round with a fresh output directory:

```sh
python3 benches/diagnostics/negated-class-runs-20261022/compare.py \
  /tmp/ferroni-perf/base0001 /tmp/ferroni-perf/cand0001 /tmp/ferroni-perf/broad \
  --cases all --repetitions 3 --samples 30 --warmup 0.5 --measure 1.5

python3 benches/diagnostics/negated-class-runs-20261022/compare.py \
  /tmp/ferroni-perf/base0001 /tmp/ferroni-perf/cand0001 /tmp/ferroni-perf/layouts \
  --cases ferrolink_markup_scan,semver_version_parse,semver_hyphen_range_captures,jinja_integer_token,jinja_string_token \
  --repetitions 8 --samples 30 --warmup 0.5 --measure 2 \
  --alignment-offsets 0,8,16,24,32,40,48,56
```

The runner needs Python 3.12 or newer; this run used Python 3.14. Do not run
builds, tests, profiles, or other benchmarks concurrently with timing.

## Correctness and repository checks

The differential test compares the same compiled program with fusion enabled
and disabled over 155,520 combinations: UTF-8 and ASCII, short/long runs,
NUL and malformed bytes, captures, backreferences, look-aheads, logical
ends, forward/backward/bounded searches, find options, match/search retry
limits, stack limits, and optional regions. It compares the result, every
capture, and the search retry counter. A separate test crosses stack-limit
boundaries with repeated captured possessive runs.

The candidate passed the full `ffi` test run (including integration and doc
tests), the `prefilter-self-check` unit/scanner suites, `cargo fmt`, Clippy over
all workspace targets and features with warnings denied, and Rustdoc with
warnings denied. Corpus integrity, benchmark claims, workflow pins, README
consistency, and the offline cargo-deny check also passed.

[validation.json](validation.json) records the commands and log names. The
C compiler emitted existing warnings; cargo-deny reported unused license
allowances. Sandbox access failures for README theme fetching and the advisory
cache were resolved by rerunning those checks with host permissions; both
attempts are retained. Coverage instrumentation, the full documentation-site
build, and hosted CI were not run in this local round. The [evidence
archive](evidence.tar.gz) retains every trial's raw samples, source patches,
profiles, build logs, and completed validation logs; `evidence.sha256` records
its checksum.
Existing ignored opt-in tests remain ignored. Local checks do not establish
the status of hosted CI.
