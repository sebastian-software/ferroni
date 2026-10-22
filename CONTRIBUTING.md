# Contributing to Ferroni

Welcome! Help keep Oniguruma's regex engine going strong in Rust. Bug reports,
clearer examples, Unicode updates and fixes are all useful contributions.
Ferroni is feature-complete; the focus is correctness and maintenance.

The engine is a 1:1 port of [Oniguruma](https://github.com/kkos/oniguruma).
Changes to ported code preserve its module mapping and control flow. Start
with [ADR-001](https://ferroni.dev/adr/001-one-to-one-parity-with-c-original)
before changing engine code; documentation fixes are a good way to get familiar
with the project.

## Getting Started

```bash
git clone https://github.com/sebastian-software/ferroni.git
cd ferroni
cargo build
cargo test
```

That is enough to build and run the default test suites. C sources are only
needed for comparison benchmarks and checks that enable the `ffi` or `onigmo`
features. Before opening a pull request, run the [local checks](#local-checks).

Ferroni's MSRV is frozen at Rust 1.94 while the crate is in maintenance mode.
Ferroni no longer follows the family's rolling four-releases-below-stable floor
(decision D4, set in
[#104](https://github.com/sebastian-software/ferroni/pull/104)); the MSRV
moves only when a fix or a dependency requires a newer compiler, and such a
bump is called out in the release notes. A dedicated CI lane enforces 1.94.
`rust-version` in `Cargo.toml` is the source; the README badge and the CI lane
copy it.

## Running Tests

Debug builds require an increased stack size. The required values are stated
once in
[ADR-013](https://ferroni.dev/adr/013-stack-overflow-debug-builds).
[`.cargo/config.toml`](.cargo/config.toml) sets the ADR-013 value for every test
binary Cargo runs, so a plain test run works:

```bash
cargo test
```

To run a single suite, or to set a different stack size, use the per-suite
commands:

```bash
# Full UTF-8 compat suite
RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1

# Other suites
cargo test --test compat_syntax
cargo test --test compat_options
cargo test --test compat_regset
RUST_MIN_STACK=268435456 cargo test --test compat_back -- --test-threads=1
```

### The DFA pre-filter

The scanner's DFA pre-filter
([ADR-008](https://ferroni.dev/adr/008-rust-only-optimizations)) must find
what the position-lead search finds. Three checks hold it to that, beyond
the unit tests and `tests/compat_prefilter_differential.rs`:

- The `prefilter-self-check` Cargo feature (not a default; it implies
  `dfa-prefilter`) makes every search the pre-filter decides also run the
  position-lead search on the same set, and panic with the pattern set, the
  subject, the start and the options unless both decide alike, captures
  included. CI runs the scanner suites with it; locally:

  ```bash
  cargo test --features prefilter-self-check --lib --test api_test --test compat_prefilter_differential --test scanner_prefilter_regressions
  ```

- The `prefilter-differential` fuzz target compares a scanner with the
  pre-filter against one without it over generated and raw pattern sets;
  [fuzz/README.md](fuzz/README.md) describes it and how to seed its corpus
  from the compat pairs.

- `tests/ferriki_corpus_differential.rs` does the same over a TextMate
  grammar corpus: every rule's pattern list of every grammar JSON in
  `FERRONI_GRAMMAR_DIR`, tokenizing the sample files in `FERRONI_SAMPLE_DIR`
  line by line, from every match end, with the pre-filter on and off. The
  grammars and samples are not part of this repository; Ferriki's checkout
  carries them (its `assets/upstream/textmate-grammars-themes/grammars` and
  the fixtures under `node/benchmarks`). The test is ignored and meant for a
  release build; it prints the grammar, scanner, pattern and call counts:

  ```bash
  FERRONI_GRAMMAR_DIR=/path/to/grammars FERRONI_SAMPLE_DIR=/path/to/fixtures \
    cargo test --release --test ferriki_corpus_differential -- --ignored --nocapture
  ```

Test counts are derived from the tree by `./scripts/count-tests.sh`. The
[compatibility guide](https://ferroni.dev/guide/compatibility#test-parity)
quotes the total rounded down to the hundred and carries the per-file parity
table. `scripts/check-readme-figures.sh` (run in CI) enforces the total.

The `unsafe` figures in the "Current State" section of
[ADR-002](https://ferroni.dev/adr/002-unsafe-code-policy) are derived by
`./scripts/count-unsafe.sh`. Run it after changing `unsafe` code and update
that section from its output. `scripts/check-readme-figures.sh` also enforces
that section's exact site count.

### Code samples in the docs

`cargo test --doc` also runs the Rust samples in the README and in the guide
pages under `docs/app/routes/guide/`. They are included through hidden items at
the end of [`src/lib.rs`](src/lib.rs). Every `rust` fence must compile and pass.
A snippet that can only illustrate an API is marked `ignore`, with the reason in
the sentence before it. Code that must not compile, such as the 1.x code in the
migration guide, is marked `compile_fail` with the error codes it must raise
(`rust compile_fail,E0639`). Label every other fence with its language (`bash`,
`toml`, `text`): an unlabeled fence is compiled as Rust. Edit the README in
`README.md.src` and regenerate it, as described below.

`docs/` is not part of the published crate, so `build.rs` sets the
`ferroni_guide_docs` cfg only when the guide pages are present. A packaged copy
therefore tests the README alone. A new guide page needs a hidden item in
`src/lib.rs` behind that cfg, and the page added to the list in `build.rs`.

## Coverage

CI measures line coverage on every pull request and fails the build below a
hard threshold. [`scripts/coverage.sh`](scripts/coverage.sh) *is* that gate: the
workflow runs it unchanged, so the same command reproduces CI locally.

```bash
rustup toolchain install nightly
cargo install cargo-llvm-cov --locked
./scripts/coverage.sh
```

The threshold, the files excluded from the measurement (generated Unicode
tables, the FFI bindings, the test files themselves) and the stack-heavy tests
that have to be skipped under LLVM instrumentation are defined once, in that
script -- the workflow runs it rather than repeating any of it. Raising the
threshold is a normal change; lowering it needs a reason in the pull request.
The README badge quotes the number, so move it in the same change.

The run prints `Line coverage: X% (gate: ≥ N%)` and repeats that line in the
GitHub run summary, so a failing gate says how far off it was.

The former per-change (patch) coverage target was retired together with
Codecov: CI carries no diff-coverage tooling, so the aggregate gate above is
the only automated coverage check. Whether a change brings its own tests is
judged by reviewers, on the diff.

## Local Checks

These are the exact commands CI runs; run them before opening a pull request:

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
cargo deny --all-features --locked check
./scripts/check-workflow-pins.sh
./scripts/readme-family.sh check
./scripts/check-readme-figures.sh
```

`--all-features` includes the C comparison features. Prepare both pinned
source checkouts before the clippy command:

```bash
./scripts/prepare-oniguruma-sources.sh
./scripts/prepare-onigmo-sources.sh
```

Pull request titles must be Conventional Commits; a CI lane checks them.

The documentation site in `docs/` is a Node workspace declared in
`.repometa.json`. It carries the org formatter configuration, so format it from
that directory:

```bash
cd docs
pnpm install --frozen-lockfile
pnpm format:check   # pnpm format rewrites
pnpm verify         # lint, typecheck, format, benchmark claims, and production build
```

If syntax highlighting fails while downloading assets, use the same pinned
GitHub mirror as CI. The legacy asset host can return HTTP 403:

```bash
export FERRIKI_ASSETS_BASE_URL=https://raw.githubusercontent.com/sebastian-software/ferriki
pnpm verify
```

The setting also applies to `pnpm dev`. Ferriki still verifies the assets
against the release's pinned hashes.

The home page's run sample is generated from `examples/website_sample.rs`
into `docs/app/data/regex-sample.json`. Its highlighted syntax examples and
scanner steps are generated from `examples/website_walkthrough.rs` into
`docs/app/data/regex-walkthrough.json`.

After changing either example, run `pnpm sample:write` and `pnpm format` from
`docs/`, then commit the generated artifacts. `pnpm sample:check` runs both
examples again: it compares the run sample's source and stdout, and the
walkthrough's matching results, with the committed artifacts. The run sample's
caption retains the Ferroni version that originally generated it.
CI runs the same comparison in the `sample` job of
[`.github/workflows/ci.yml`](.github/workflows/ci.yml), as
`node docs/scripts/render-regex-sample.mjs --check`.

CI also runs a `standards drift` lane that executes
`@sebastian-software/standards check`. Its version is pinned in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) and raised by Renovate;
run the same pinned command locally when you change repository-wide
configuration.

## The Ferramenta family block

The root README is generated by native mdtheme from `README.md.src`. Sebastian
Software is the outer frame and Ferramenta the inner frame. Edit project prose
in the source, then run `mise run readme:write`; `mise run readme:check` checks
the entire result. See [README themes](docs/readme-theme.md) for installation,
CI, and the pre-push command. Standards explicitly delegates README
ownership to mdtheme and does not append a company footer.

The existing commands remain aliases for the native tasks:

```sh
./scripts/readme-family.sh write
./scripts/readme-family.sh check
```

The CLI pin is in `mise.toml`; the two Git theme revisions are in
`mdtheme.yaml`. These commands require mise, Git, and network access.

The React documentation site uses `ferramenta-family` from `docs/package.json`.
Pin the published npm version and keep the Ferramenta theme in `mdtheme.yaml`
at that release's source revision. Update the lockfile and run `pnpm verify`
from `docs/`. The shared header and footer
exclude this project from sibling links and include sibling descriptions.

## Running Benchmarks

The general regex publication workload is now the
[source-backed corpus](benches/regex_corpus.md). Regex tasks, TextMate Scanner
replays, and Ferriki end-to-end highlighting have separate boundaries. The
earlier `battle_bench` corpus remains an internal diagnostic tool. Superseded
published measurements and stored artifacts are retained in Git history.

```sh
./scripts/prepare-oniguruma-sources.sh
./scripts/prepare-onigmo-sources.sh
python3 scripts/check-production-corpus.py
# Independent source regex-step replay; needs Node, Ruby and libpcre2-8
python3 scripts/verify-production-oracles.py
python3 -m unittest discover -s scripts/tests -p 'test_production*.py'
python3 scripts/compare-production.py validate /tmp/production-validation
python3 scripts/compare-production.py run /tmp/production-results
```

Use a fresh output directory. The runner retains per-engine eligibility reasons,
source/corpus/build context, three process repetitions with environments padded
to one size and offset per repetition, balanced execution order,
and Criterion raw samples and intervals. Short runs require `--diagnostic` and
cannot support publication. See the corpus guide for filters, optional memory
measurements, operation contracts and interpretation limits.

To profile one case's steady lane without Criterion, run the benchmark binary
with `PRODUCTION_CASE=<id> PRODUCTION_ENGINE=<engine>` and `--steady N`: it
validates the pair, runs one warm-up batch and then N batches. Under a profiler,
the difference between `--steady N` and `--steady 0` is N batches.

Steady reuse measures 3 s after a 1 s warm-up; compile and first use measure
1 s after 0.5 s, since each of their iterations is one construction. The
Blacksmith workflow runs each repetition on its own VM with `run --only-run N`
and joins a platform's repetitions with
`compare-production.py merge OUT RUN1 RUN2 RUN3`. A Ferroni time is only ever
divided by a comparator time from the same repetition, and so from the same VM.

Published tables describe their linked measured commit. Publication requires
the run's library source to match the current tree; `--check` verifies the
retained snapshot and raw results after later engine changes. Local diagnostic
improvements do not update public Blacksmith numbers.

The agreed 20-format selection for the next Ferriki highlighting corpus is
recorded in [`benches/highlighting_corpus.md`](benches/highlighting_corpus.md),
including its rationale and fixture requirements.

The website's compact feature matrix is checked separately with untimed syntax
examples. These are capability checks, not production benchmark cases or speed
measurements. With the pinned C sources prepared, verify every advertised cell:

```sh
cargo test --locked --features ffi,onigmo --test regex_feature_matrix
```

The matrix covers the illustrated native syntax, not all possible task-equivalent
rewrites. An unsupported cell must reject the illustrated syntax or demonstrate
the recorded semantic difference; an accepted cell must return the expected
UTF-8 span. Execution failures cannot count as a missing feature.

The Blacksmith workflow also measures the separate tokenizer corpus, which
compares all profiles on the current tiktoken patterns:

```sh
python3 scripts/compare-production.py validate /tmp/tokenizer-check --corpus tokenizer
python3 scripts/publish-additional-comparisons.py --check
```

To replay the original tokenizer oracles, install `regex==2026.1.15` in an
isolated Python environment and run `scripts/verify-tokenizer-oracles.py`.
[Comparator notes](benches/comparator-notes.md) record the library source review
and why regexr is no longer compared.

`regression_bench` tracks Ferroni-only regressions. Its `shared_regex_threads`
group runs ferrolink's two corpus steps (`captures_iter`, and lexer-style
`captures_at` and `find_at` loops) on 1, 4 and 8 threads with the same work per
thread. The times should stay nearly flat as threads are added; a write to
shared state per match or per call shows as growth with the thread count. Before
[#417](https://github.com/sebastian-software/ferroni/pull/417), `captures_iter`
took 4.5 ms on one thread and 39 ms on eight; after it, 5.0 ms. Before
[#418](https://github.com/sebastian-software/ferroni/pull/418) the `find_at`
loop grew from 6.1 to 12.2 ms; after it, from 3.6 to 4.0 ms (Apple M1 Ultra).

```bash
cargo bench --bench regression_bench -- shared_regex_threads
```

`battle_bench` requires a local Oniguruma source snapshot for comparison:

```bash
./scripts/prepare-oniguruma-sources.sh
cargo bench --features ffi --bench battle_bench
# General-purpose validation, extraction, Unicode, and redaction tasks
cargo bench --features ffi --bench battle_bench -- general_regex
# Practical extraction tasks, including recursive templates and log secrets
cargo bench --features ffi --bench battle_bench -- regex_tasks
# Nested capture-history trees in HTML-like markup
cargo bench --features ffi --bench battle_bench -- capture_history
# Complex syntax specific to Oniguruma
cargo bench --features ffi --bench battle_bench -- oniguruma_features
# Short and long Unicode class workloads, with C-validated capture traces
cargo bench --locked --features ffi --bench battle_bench -- unicode_classes
# Same document workloads through the scanner
cargo bench --features ffi --bench battle_bench -- scanner_documents
```

The reference suite validates match positions, capture bounds, and scanner
token traces before timing. Text-search and single-pattern timings request
no capture output from either Ferroni or C; scanner timings include captures.
The `general_regex` group validates mixed accepted/rejected batches and full
capture traces against both C and the `regex` crate. Extraction materializes
all matches; redaction uses an identical output builder for each engine.
These diagnostic tasks do not supply the public source-backed comparison.

Exact external input revisions for the native build and diagnostic inputs are pinned in
[`benches/battle_inputs.toml`](benches/battle_inputs.toml).

The `regex_tasks` group includes larger practical documents such as nested
template interpolation, Rust comments, HTTP tokenization, HTML script blocks,
CSV near-misses, and authentication logs. Its HTTP request, environment
assignment, and CSV cases require matching captures from the `regex` crate,
FancyRegex, and PCRE2 with and without JIT. The `capture_history` group compares
the full nested capture tree for markup against C. The `oniguruma_features`
group covers syntax only Oniguruma runs (atomic and possessive groups,
recursive subexpression calls, the absent operator, conditionals,
case-insensitive backreferences, and alternating lookbehind) against C.

With `--features ffi`, every scanner replay adds a `_c` case: the same
captured calls through the vscode-oniguruma C scanner, checked against every
captured result before timing. Every engine of a replay has the call boundary
of Ferroni's `find_next_match_utf16`, recorded in
[ADR-010](https://ferroni.dev/adr/010-benchmark-strategy).

`battle_bench` and the scanner replays also time further engines where they
reproduce Oniguruma's results (or the captured Shiki ones): PCRE2 with and
without JIT, fancy-regex in its Oniguruma mode and, with the `onigmo` feature,
Ruby's Onigmo (pinned in `benches/battle_inputs.toml`, fetched by
`scripts/prepare-onigmo-sources.sh`). A case an engine rejects or evaluates
differently prints an `UNSUPPORTED` line with the reason instead of a timing.
`benches/shiki_js` replays the same Shiki calls through Shiki's JavaScript
engine in Node, and its `capture.mjs` records new traces from Shiki itself: the
C and PHP replays (`shiki_scanner_bench`) come from it and run in every engine.
The JSON, Astro, TSX, Rust and HTML replays (`ferriki_scanner_bench`,
`benches/json_scanner`, `benches/astro_scanner`, `benches/tsx_scanner`,
`benches/rust_scanner` and `benches/html_scanner`) were recorded from
Ferriki's own tokenizer. JSON keeps a simple grammar, where the pre-filter has
the least to save, in the comparison, TSX the large TypeScript statement and
expression scanners, Rust a few dense scanners of more than 70 patterns, and
HTML embedded JavaScript and CSS. Astro, recorded for issue #327, stays a local
regression replay: no comparison engine reproduces it.

`scripts/compare-engines.py` validates and times a selection of cases for
every engine in an ordinary release build. The `shared` set holds everyday text
processing the `regex` crate also runs (`benches/regex_tasks.rs`: markup, logs,
chat with emoji, Markdown, JSON, CSV, and a few patterns at the limit of the
engines), the `oniguruma` set the same kind of work with Oniguruma syntax, the
`micro` set short single searches and compilation, and the `textmate` set the
grammar scanners of `battle_bench` and the scanner replays. CPU profiles are a local job; see the scanner READMEs below.

```bash
./scripts/prepare-onigmo-sources.sh
pnpm install --frozen-lockfile --dir benches/shiki_js
./scripts/compare-engines.py run /tmp/ferroni-compare --cases oniguruma textmate
./scripts/compare-engines.py report /tmp/ferroni-compare.md /tmp/ferroni-compare
```

The Blacksmith workflow measures four scanner replays as a separate lane, one
per complexity tier: JSON, Rust, TSX and HTML (`REPLAYS` in
`scripts/compare-scanner-replays.py`); `lanes: scanner` runs it alone. The
other replays stay local diagnostics. It times each replay's `document` and
`cold_document` lanes for Ferroni, the C scanner, Onigmo, PCRE2 with and
without JIT and the fancy-regex variants. An engine is timed only where it
reproduces every captured result; the others are listed with the reason. Each
platform gets its own per-case report without an aggregate, outside the regex
ranking and the website.

```sh
./scripts/prepare-onigmo-sources.sh
python3 -m unittest discover -s scripts/tests -p 'test_scanner*.py'
python3 scripts/compare-scanner-replays.py validate /tmp/scanner-validation
python3 scripts/compare-scanner-replays.py run /tmp/scanner-results
```

`compare-scanner-replays.py` follows `compare-production.py`: a fresh output
directory, validation of every case and engine before any timing, three
process repetitions with balanced order, one process per case and engine,
retained Criterion samples, and `--diagnostic` for short runs. On Blacksmith
each repetition runs on its own VM (`run --only-run N`), and
`compare-scanner-replays.py merge OUT RUN1 RUN2 RUN3` rejects a missing cell or
a repetition built from different inputs. The lane measures the default scanner
configuration, so it refuses to run with `FERRONI_BENCH_PREFILTER` set.

For process-isolated memory comparison on the large TypeScript scanner
workload, use:

```bash
./scripts/run-battle-memory.sh
```

For a pure Ferroni replay of actual C++ TextMate scanner calls, including
focused hot groups and a bounded CPU profiling driver, see
[`benches/cpp_scanner/README.md`](benches/cpp_scanner/README.md):

```sh
cargo bench --locked --features ffi --bench cpp_scanner_bench -- --test
cargo bench --locked --bench cpp_scanner_bench
python3 scripts/profile-cpp-scanner.py /tmp/cpp-scanner --sample --group 78
```

For the Java replay, C parity and bounded CPU/pattern diagnosis, see
[`benches/java_scanner/README.md`](benches/java_scanner/README.md):

```sh
cargo bench --locked --features ffi --bench java_scanner_bench -- --test
cargo bench --locked --bench java_scanner_bench
```

The C++, SCSS, Java, JSON, Astro, TSX, Rust and HTML replays also have a
`cold_document` case: each iteration compiles fresh scanners and replays the
whole document once, so it measures one-shot highlighting, including the
pre-filter warm-up and build. The C scanner and the other engines have the
same lane (`cold_document_c`, `cold_document_<engine>`), with PCRE2's JIT
compilation timed. Run only the cold cases with, for example:

```sh
cargo bench --locked --bench cpp_scanner_bench -- cold
```

For construction and destruction of the captured C++, SCSS and Java scanner
sets, uncached and through one `ScannerPatternCache` per set
(`<name>_pattern_cache`), use:

```sh
cargo bench --locked --bench scanner_compile_bench
```

JSON loading and pattern-list preparation are excluded from this measurement.
Set `FERRONI_SCSS_TRACE` to reuse the same SCSS fixture on an older checkout.

The scanner replays and the compile bench build their scanners with the DFA
pre-filter of [ADR-008](https://ferroni.dev/adr/008-rust-only-optimizations)
as every scanner does by default; `FERRONI_BENCH_PREFILTER=0` builds them
without it, so one binary measures both (ABBA runs). What the pre-filter
covers and costs per trace, per scanner group, and in a fixed-iteration
replay for `/usr/bin/time -l`:

```sh
cargo build --release --example prefilter_census
target/release/examples/prefilter_census census
target/release/examples/prefilter_census groups benches/scss_scanner/trace.json
/usr/bin/time -l target/release/examples/prefilter_census replay benches/cpp_scanner/trace.json 10 on
```

## Regenerating Unicode Tables

The checked-in Unicode tables are generated directly from the versioned
Unicode Character Database (UCD), pinned in [unicode_data.toml](unicode_data.toml).
These scripts are maintainer tools; normal `cargo build`, tests, and CI do not
run them automatically. Python 3.11+ and rustfmt are required.

```bash
python3 scripts/prepare_unicode_data.py 18.0.0
python3 scripts/gen_unicode_tables.py --version 18.0.0
```

To verify that the pipeline still reproduces the recorded Unicode 16.0 baseline
(the previous tables, except the intentionally completed `InCB` property; see
ADR-015):

```bash
python3 scripts/prepare_unicode_data.py 16.0.0
python3 scripts/check_unicode_16.py
```

Commit all four generated files together with the source change:

- `src/unicode/property_data.rs`
- `src/unicode/fold_data.rs`
- `src/unicode/egcb_data.rs`
- `src/unicode/wb_data.rs`

### Annual Unicode update

The Unicode Consortium publishes a new UCD each September. Ferroni follows it
once a year, in a single change:

1. Add the new archive to `unicode_data.toml`. Its `sha256` is the SHA-256 of
   the complete `UCD.zip` as downloaded from `https://www.unicode.org/Public/`.
   Keep the older pins; the 16.0.0 pin backs the baseline check.
2. Run the two commands above for the new version, and run
   `scripts/check_unicode_16.py` to confirm the baseline still holds.
3. Update the version and property-name count wherever the README, the
   documentation site and ADR-015 quote them.
4. Commit it as a `feat`, for example:

   ```text
   feat(unicode): move the Unicode tables to Unicode 18.0
   ```

   Because Ferroni is past 1.0, release-please turns that into a minor release.

## Guidelines

1. **Read the ADRs first.** The ADRs live in `docs/app/routes/adr/` and are
   published at
   [ferroni.dev/adr](https://ferroni.dev/adr/001-one-to-one-parity-with-c-original).
   They document all major architectural decisions. In particular:
   - [ADR-001](https://ferroni.dev/adr/001-one-to-one-parity-with-c-original): the 1:1
     parity goal -- same module mapping, same function names, same control flow.
   - [ADR-004](https://ferroni.dev/adr/004-c-to-rust-translation-patterns): the canonical
     C-to-Rust translation patterns used throughout the codebase.
   - [ADR-002](https://ferroni.dev/adr/002-unsafe-code-policy): the `unsafe` code policy.

2. **Cross-reference the C original.** When modifying `regcomp.rs`,
   `regexec.rs`, or `regparse.rs`, compare against the corresponding
   upstream Oniguruma source file. Run
   `./scripts/prepare-oniguruma-sources.sh` if you want a local checkout.
   The pinned benchmark input revisions live in `benches/battle_inputs.toml`.

3. **US English only.** All code, comments, commit messages, and documentation
   must be in English.

4. **Test your changes.** Run the full test suite before submitting a PR.

5. **Keep it focused.** One concern per PR. Don't mix bug fixes with
   refactoring or feature additions.

## Reporting Issues

Please open an issue on GitHub with:
- The regex pattern and input string that triggers the bug
- Expected vs. actual behavior
- If possible, the corresponding C Oniguruma behavior for comparison

## Triage and Review

Issues are welcome and are triaged as time allows. Pull requests are reviewed
against the [architecture decisions](https://ferroni.dev/adr/001-one-to-one-parity-with-c-original),
so a change has to keep the structural parity with C Oniguruma that ADR-001
describes. The `help wanted` label marks the issues that are waiting for a
contributor. Report a suspected vulnerability privately, as
[SECURITY.md](SECURITY.md) describes, and not in a public issue. The
[project status](https://ferroni.dev/#project-status) lists what the project maintains and
what is not planned.

## License

By contributing, you agree that your contributions will be licensed under the
[BSD-2-Clause License](LICENSE).

#
