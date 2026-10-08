# Contributing to Ferroni

Thanks for your interest in contributing! Ferroni is a 1:1 Rust port of the
[Oniguruma](https://github.com/kkos/oniguruma) regex engine, and contributions
that maintain that structural fidelity are welcome.

## Getting Started

```bash
git clone https://github.com/sebastian-software/ferroni.git
cd ferroni
cargo build
```

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
[ADR-013](https://ferroni.dev/adr/013-stack-overflow-debug-builds);
the commands below use them.

```bash
# Full UTF-8 compat suite
RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1

# Other suites
cargo test --test compat_syntax
cargo test --test compat_options
cargo test --test compat_regset
RUST_MIN_STACK=268435456 cargo test --test compat_back -- --test-threads=1
```

Test counts are derived from the tree by `./scripts/count-tests.sh`; the
README quotes the total in its
[Correctness and safety](README.md#correctness-and-safety) section, and the
[compatibility guide](https://ferroni.dev/guide/compatibility#test-parity)
carries the per-file parity table.

The `unsafe` figures in the "Current State" section of
[ADR-002](https://ferroni.dev/adr/002-unsafe-code-policy) are derived the same
way, by `./scripts/count-unsafe.sh`. Run it after changing `unsafe` code and
update that section from its output.

### Code samples in the docs

`cargo test --doc` also runs the Rust samples in the README and in the guide
pages under `docs/app/routes/guide/`. They are included through hidden items at
the end of [`src/lib.rs`](src/lib.rs). Every `rust` fence must compile and pass.
A snippet that can only illustrate an API is marked `ignore`, with the reason in
the sentence before it. Label every other fence with its language (`bash`,
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
```

`--all-features` includes `ffi`, so run
`./scripts/prepare-oniguruma-sources.sh` before the clippy command. Pull
request titles must be Conventional Commits; a CI lane checks them.

The documentation site in `docs/` is a Node workspace declared in
`.repometa.json`. It carries the org formatter configuration, so format it from
that directory:

```bash
cd docs
pnpm install --frozen-lockfile
pnpm format:check   # pnpm format rewrites
pnpm verify         # lint, typecheck, format, benchmark claims, and production build
```

The home page's run sample is generated from `examples/website_sample.rs`.
After changing the example, run `pnpm sample:write` from `docs/` and commit
`docs/app/data/regex-sample.json`. `pnpm sample:check` runs the example again
and compares its source and actual stdout with the committed artifact. The
output caption retains the Ferroni version that originally generated it.

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

The agreed 20-format selection for the next Ferriki highlighting corpus is
recorded in [`benches/highlighting_corpus.md`](benches/highlighting_corpus.md),
including its rationale and fixture requirements.

`battle_bench` requires a local Oniguruma source snapshot for comparison:

```bash
./scripts/prepare-oniguruma-sources.sh
cargo bench --features ffi --bench battle_bench
# General-purpose validation, extraction, Unicode, and redaction tasks
cargo bench --features ffi --bench battle_bench -- general_regex
python3 scripts/gen_battle_tables.py --general-only
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
Generated tables use Ferroni versus Oniguruma as the primary comparison.
They retain `regex` results in a separate shared-syntax appendix: `regex` does
not support lookarounds or backreferences, so those timings cannot rank the
engines by their complete feature sets.

Exact external input revisions for the publishable battle suite are pinned in
[`benches/battle_inputs.toml`](benches/battle_inputs.toml).

The `oniguruma_features` group covers syntax only Oniguruma runs (atomic and
possessive groups, subexpression calls, the absent operator, conditionals,
case-insensitive backreferences, alternating lookbehind) against C.

With `--features ffi`, the C++, Java and SCSS scanner replays add a `_c`
case per replay: the same captured Shiki calls through the vscode-oniguruma C
scanner, checked against every captured result before timing.

`battle_bench` and the scanner replays also time further engines where they
reproduce Oniguruma's results (or the captured Shiki ones): PCRE2 with and
without JIT, fancy-regex in its Oniguruma mode and, with the `onigmo` feature,
Ruby's Onigmo (pinned in `benches/battle_inputs.toml`, fetched by
`scripts/prepare-onigmo-sources.sh`). A case an engine rejects or evaluates
differently prints an `UNSUPPORTED` line with the reason instead of a timing.
`benches/shiki_js` replays the same Shiki calls through Shiki's JavaScript
engine in Node, and its `capture.mjs` records new traces from Shiki itself: the
C and PHP replays (`shiki_scanner_bench`) come from it and run in every engine.

`scripts/compare-engines.py` validates and times a selection of cases for
every engine in an ordinary release build. The `shared` set holds everyday text
processing the `regex` crate also runs (`benches/regex_tasks.rs`: markup, logs,
chat with emoji, Markdown, JSON, CSV, and a few patterns at the limit of the
engines), the `oniguruma` set the same kind of work with Oniguruma syntax, the
`micro` set short single searches and compilation, and the `textmate` set the
grammar scanners of `battle_bench` and the Shiki scanner replays. The
[Blacksmith comparison](.github/workflows/blacksmith-comparison.yml)
workflow runs each set as its own job on `blacksmith-6vcpu-macos-26` and
`blacksmith-4vcpu-ubuntu-2404` and merges them into one summary per host.
CPU profiles are a local job; see the scanner READMEs below.

```bash
./scripts/prepare-onigmo-sources.sh
pnpm install --frozen-lockfile --dir benches/shiki_js
./scripts/compare-engines.py run /tmp/ferroni-compare --cases oniguruma textmate
./scripts/compare-engines.py report /tmp/ferroni-compare.md /tmp/ferroni-compare
```

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

For construction and destruction of the captured C++, SCSS and Java scanner
sets, uncached and through one `ScannerPatternCache` per set
(`<name>_pattern_cache`), use:

```sh
cargo bench --locked --bench scanner_compile_bench
```

JSON loading and pattern-list preparation are excluded from this measurement.
Set `FERRONI_SCSS_TRACE` to reuse the same SCSS fixture on an older checkout.
The [post-#204 comparison](benches/highlighting_results/post-204/README.md)
retains two complete 20-format highlighting runs, compiler measurements, and
native CPU profiles. Its performance target is the faster of Shiki's WASM and
JavaScript backends for each format and API.

## Regenerating Unicode Tables

The checked-in Unicode tables are generated directly from the versioned
Unicode Character Database (UCD), pinned in [unicode_data.toml](unicode_data.toml).
These scripts are maintainer tools; normal `cargo build`, tests, and CI do not
run them automatically. Python 3.11+ and rustfmt are required.

```bash
python3 scripts/prepare_unicode_data.py 17.0.0
python3 scripts/gen_unicode_tables.py --version 17.0.0
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
[project status](README.md#project-status) lists what the project maintains and
what is not planned.

## License

By contributing, you agree that your contributions will be licensed under the
[BSD-2-Clause License](LICENSE).

#
