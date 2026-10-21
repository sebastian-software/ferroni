# Source-backed regex corpus

This corpus asks how native regex libraries perform the same observable text
operation. General regex work has three levels: basic expressions, complex
expressions using broadly shared constructs, and advanced expressions with
lookaround, backreferences, possessive repeats, Unicode boundaries or other
Oniguruma features. Complexity is a workload description; it does not determine
which engines are allowed into a timing comparison.

TextMate Scanner call replays are a separate application workload. Full
highlighting, including grammar state, belongs to Ferriki's separate benchmarks.

## Current selection

[`production_regex/catalog.json`](production_regex/catalog.json) is the complete
machine-readable record: pinned source files and hashes, original and normalized
patterns, flags, measured boundary, fixture provenance, source-runtime oracles,
expected UTF-8 byte output, and license notices. It contains 21 production regex
steps with 513 upstream-derived inputs, four explicitly controlled literal probes,
and two candidates awaiting admission. Selection precedes timing the new corpus.

| Family | Cases | Operation and boundary |
| --- | --- | --- |
| Literal search | first, middle, end, absent | First span of `GET`; controlled position variants of a genuine Apache test record. These are elementary probes, not claimed production patterns. |
| UUID | validator.js loose shape and v4 | Boolean regex step. Loose shape is not full UUID validity. |
| IP addresses | validator.js IPv4 and IPv6 | Boolean anchored address expressions; excludes wrapper prechecks. |
| Versions | node-semver safe FULL and HYPHENRANGE | Capture extraction; range callback and canonicalization are excluded. |
| Template tokens | Jinja string, integer, float | Match at the original lexer candidate offset; retains preceding context where needed. No template rendering or full lexer timing. |
| Email | Django local-part and domain | Separate boolean regex steps. No whole-address validation, allowlist or IDNA fallback claim. |
| URL | Django URL regex | Boolean regex step, excluding scheme/IP/IDNA and length checks. |
| Access logs | Fluent Bit Apache and NGINX | One record's field captures. Original Fluent Bit and Elastic Beats records; timestamp conversion and the logging pipeline are excluded. |
| Case conversion | Rails underscore boundary | Literal `_` replacement, including zero-width matches; excludes the remaining underscore transformations. |
| Hashtags | Mastodon | All regex matches and capture bounds; excludes precheck, suffix trimming and entity indexing. |
| Markup | Python-Markdown smart emphasis and strong/emphasis | Capture match at an actual candidate offset; excludes tree construction. Coverage is currently only five and two fixtures respectively. |
| Shortcodes | WordPress | All six captures and whole spans for the upstream registered tag set; excludes shortcode callbacks/rendering. |
| Graphemes | Symfony String chunk(1) and chunk(2) | Chunk strings including an unmatched tail; already NFC input, with normalization/object construction excluded. |

Upstream regression fixtures establish expected semantics. They do **not**
establish production traffic frequencies, typical document sizes, or a realistic
positive/negative ratio. This first version keeps the source tests' input mix
explicit. Family aggregates give each case equal weight within its family and
each family equal weight. The 195 Django URL fixtures do not give URL validation
195 times the aggregate weight of another case.

### Pending candidates

- **Linkify-it HTTP tail:** the exact emitted ECMAScript `/iy` expression has
  UTF-16 surrogate branches. A native UTF-8 translation and parity check are
  still needed. Replacing them with current Unicode properties would be a new
  adapted task, not an unchanged import.
- **PHP-CS-Fixer recursive types:** a diagnostic PCRE2/Oniguruma translation
  matched the same whole subject but changed a capture the parser consumes.
  For the upstream `? callable(string): bool` input, `nullable` was `"? "` in
  the original and `""` after translation. The catalog retains source, runtime
  limits and reproduction; it is not admitted by a boolean-only check.

Pending cases appear in eligibility output but have no timing or aggregate weight.

## What changes from the earlier suite

| Earlier case or group | Disposition |
| --- | --- |
| Literal search / no-match | Keep the question; use explicit controlled position variants with a genuine record as the text source. |
| `uuid_validation` | Replace with named loose-shape and v4 source steps. |
| `ipv4_addresses`, `ipv6_addresses`, RFC-like demos | Replace with actual validator.js expressions and validation inputs. Extraction and validation are different tasks. |
| `semantic_versions` | Replace with node-semver's bounded safe FULL capture step. |
| Log fields, timestamps and keyword demos | Replace with Fluent Bit parser expressions. A broad keyword list is not a log parser. |
| Toy quoted strings and camel-case lookbehind | Replace with Jinja token matching and Rails replacement respectively. |
| Toy Markdown, hashtags, emoji clusters | Replace with Python-Markdown, Mastodon and Symfony source steps. |
| `html_element_pairs`, recursive divs/JSON, conditional and absent demos, password lookaheads, price lookbehind, 200-keyword alternation, CSV last-column backtracking | Keep internal feature diagnostics. Require source-backed tasks before public admission; extra syntax alone does not make a meaningful workload. |
| Generic HTML/JSON/CSV/email-redaction/emoticon extraction | Diagnostic until a specific production operation and its output contract are established. |
| Isolated Unicode classes and full folding | Keep capability and regression diagnostics, reported separately. |
| Flattened TypeScript/CSS/Rust grammar scans | Internal engine diagnostics. Flattening/deduplicating patterns and skipping compile failures is not complete TextMate execution. |
| TextMate replays | Separate scanner workload; full captured document calls are the publication candidates, hot scanner subsets are diagnostics. |
| Compiler microbenchmarks | Internal diagnostics. Public compile/first-use rows use the admitted production patterns. |

Existing `battle_bench` and `regression_bench` remain internal diagnostic tools.
Superseded measurement pages and stored result artifacts are removed from the
current tree; Git history retains them.

## Semantics and measured work

Every engine must reproduce the source-runtime oracle for **every** fixture of
a case before timing it. Statuses distinguish `pending`, `syntax_rejected`,
`semantic_difference`, `execution_error` and `timeout`. The latter two are never
ordinary negative matches. Unset captures remain distinct from empty captures.
Offsets and spans are UTF-8 bytes in all outputs.

The main task uses one explicitly recorded dialect recipe for every engine.
Examples are Python strict-end `\Z` to native `\z`, ECMAScript `\d` to ASCII
`[0-9]`, and WordPress's ASCII word class. Python match-at-position uses a
noncapturing `\G` wrapper with the complete subject where lookbehind needs it.
Jinja's context-free string/integer steps use a `\A` wrapper on a borrowed
suffix and rebase capture bounds; this works through all libraries' public APIs.
These are documented adapters, not invisible per-engine pattern optimizations.
Future equivalent-task implementations must have separate IDs/profiles and
include their preprocessing and postprocessing costs.

Ferroni's baseline (`rust`) uses its standard public `Regex` methods. The separate
`rust_erroraware` profile uses fallible `*_with` searches and iteration with
`SearchOptions::new()`. Both use standard `replace_all` because there is no
fallible replacement API. Both profiles pass the complete output oracle; the
baseline additionally receives a fallible preflight check so execution errors
cannot silently become negative matches.

Standard searches can reuse the engine's cached match workspace. Parameterized
searches currently create fresh workspace, even with empty options. The API
profiles expose this cost without changing the engine implementation; both
compile through the same builder. Rust regex and fancy-regex
also use their library APIs; C libraries use their public C APIs through the
existing safe wrappers. Capture outputs materialize every requested span. Native
replacement APIs are used where available; C replacement loops search spans and
build the same owned string. Grapheme splitting includes assembling owned chunk
strings and the unmatched tail. Fixture loading, source checks and serialization
are outside timing. Each fixture's output is consumed and dropped inside timing;
the harness does not add a batch-result vector to boolean or first-span calls.
Input `&str` is trusted UTF-8; validation policy is recorded per engine.

Compile means fresh pattern/context construction, including selected PCRE2 JIT,
with destruction outside timing. First use means construction plus the first
listed fixture's operation, also with destruction outside timing. These are
fresh-pattern costs in a warmed process, not OS-process startup. Steady reuse
means one full fixture batch with a reused compiled object. Do not compare the
batch's nanoseconds directly with one-call first-use nanoseconds. An amortization
estimate must use whole batches and the measured input mix, not invent a universal
number of production calls.

The main profiles use default public-library behavior plus source flags;
fancy-regex's Oniguruma dialect is explicit. Seek mode and PCRE2 JIT are separate
profiles. Syntax and Unicode policy differences remain visible rather than being
papered over by changing the expected output.

Every timed process of a repetition starts with an environment of the same
size. The environment sits above a process's initial stack, so its size moves
the alignment of every stack frame, and with it a steady lane by a few percent:
padding the environment of one Ferroni profile by 0 to 60 bytes moved it
between 93.2 and 98.4 µs on an Apple M1 Ultra. Engine names, case IDs and the
Criterion directory differ in length, which used to give every engine a fixed
offset of its own; `rust_erroraware` measured up to 10% faster than `rust` on
the same iterator. The runner pads each environment with
`FERRONI_BENCH_ALIGNMENT_PAD` to one size and adds 0, 24 or 48 bytes per
repetition, so the repetitions sample different alignments. A ratio that
depends on alignment then reads as inconclusive instead of as a direction.
`context.json` records the method and the padded size of each repetition.

Earlier published aggregates are not a performance baseline for this corpus.
Most general tasks already materialized all matches and capture bounds through
direct engine calls; individual search probes requested only a position. The
new corpus uses the public `Regex` API and different patterns, inputs, operation
contracts and task weights. An initial new-corpus run used only Ferroni's
fallible API; the published comparison measures both API profiles alongside the
other engines on the same runners.

Eligibility refers to the documented configuration, not every possible library
configuration. In particular, Rust regex uses its default disabled octal syntax;
it offers `RegexBuilder::octal(true)`. Its rejection of Django's email local-part
expression is therefore a profile/dialect difference, not evidence that the
library lacks an octal option. Task-equivalent alternatives and other profiles
need separate measured rows and the same independent output oracle.

## Running and reporting

```sh
./scripts/prepare-oniguruma-sources.sh
./scripts/prepare-onigmo-sources.sh
python3 scripts/check-production-corpus.py
python3 scripts/verify-production-oracles.py
python3 scripts/compare-production.py validate /tmp/production-validation
python3 scripts/compare-production.py run /tmp/production-results \
  --memory-cases ipv6_validation,rails_underscore_regex_step,symfony_grapheme_chunks_1
```

The independent oracle replay needs Node with match indices, Python, Ruby and
the PCRE2 8-bit shared library. It replays the exact isolated source expressions;
it does not install or execute the complete upstream applications. Oracle runtime
versions matter, especially for Unicode segmentation. Corpus integrity and engine
eligibility work offline after the pinned C build inputs are prepared.

The measurement runner builds once, snapshots the embedded corpus, refuses stale
binaries and nonempty output directories, and bounds each engine/case process.
Failed Ferroni oracle checks or any engine's execution error/timeout stop the run
before timing. Ordinary comparator capability gaps remain listed and excluded.
Default measurement uses three independent processes per pair, 30 Criterion
samples per lane, one second warmup and three seconds measurement. Case order is
seeded; engine order rotates and reverses between repetitions. Increase timeout
when selecting a longer measurement window. A short smoke run must use
`--diagnostic`; it cannot support publication.

Retain the whole output directory: corpus, Cargo lock, engine/harness source
snapshot (including prepared native inputs), C source pins, machine and
toolchain context, build hash/flags, eligibility reasons, process logs, raw samples,
intervals, measured order and summaries. Record host load/power/thermal conditions
with `--host-note`. ARM and x86 results remain separate directories/reports.

The manual `blacksmith-comparison.yml` workflow runs the complete suite on both
provider-managed platforms. After both jobs succeed, install the documentation
dependencies, then retain and verify the run with:

```sh
pnpm install --frozen-lockfile --dir docs
python3 scripts/publish-production.py --run-id <github-run-id>
python3 scripts/publish-production.py --check
```

The publisher downloads the workflow artifacts with `gh`, rejects incomplete or
diagnostic evidence, compares estimates against the original Criterion samples,
and regenerates the repository tables and the website's summary. It formats that
summary JSON with the repository's installed formatter. Verification with
`--check` needs only Python's standard library. Complete evidence stays
compressed in the current results directory so artifact expiry does not break
reproduction. The public production aggregates exclude the four controlled
literal probes; their individual results remain visible. Update the explanatory
text after reviewing the generated results, then run the documentation checks.

The runner pins PCRE2's bundled source through the locked `pcre2-sys` crate,
using `PCRE2_SYS_STATIC=1` instead of whichever system PCRE2 happens to be
installed. Externally supplied `--binary` runs are always labeled diagnostic
because their build flags are unverified. Source snapshots retain a dirty
checkout's actual engine/harness code without requiring a cleanup or commit.

Ratios are comparator/Ferroni. Each pair's aggregate lists its exact common cases
and gives families equal weight. Do not rank engines using aggregates over
different case pools. Reports show repetition ranges; a direction is inconclusive
unless all three process-run interval comparisons agree. This conservative rule
is a screening aid, not a cross-process statistical significance test. Review raw
samples and unstable results before publication. Unexpected timeouts/errors
remain recorded and fail the run.

Optional memory rows measure fresh-process peak RSS after compilation or one
execution batch. They include the binary, runtime, complete parsed catalog,
allocator and outputs; they do not estimate retained compiled-pattern bytes.

## Extending coverage

Add representative large documents, density ranges, source-backed near misses
and realistic nesting only with documented inputs or a clearly described
controlled derivation. Keep selection independent of which engine wins. External
benchmark corpora can form separately identified strata, with their own licenses
and contracts; none is silently claimed by this first source-test corpus.

Scanner publication work must include full C, Java, PHP, C++, SCSS, JSON and
Astro call traces, with equivalent input preparation, UTF-16/byte conversion and
output materialization boundaries. New scanner claims require a separate repeated-run measurement. Ferriki's full highlighter measurements remain a
separate publication.
