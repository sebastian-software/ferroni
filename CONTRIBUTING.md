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

Ferroni's MSRV is Rust 1.94, enforced by a dedicated CI lane.

## Running Tests

Debug builds require an increased stack size. The required values are stated
once in
[ADR-013](https://sebastian-software.github.io/ferroni/adr/013-stack-overflow-debug-builds);
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

> **Warning:** Never run `cargo test -- --ignored` -- the
> `conditional_recursion_complex` test intentionally hangs.

Test counts are derived from the tree by `./scripts/count-tests.sh`; the
README quotes the total in its [Test parity](README.md#test-parity) section.

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
```

CI also runs a `standards drift` lane that executes
`@sebastian-software/standards check`. Its version is pinned in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) and raised by Renovate;
run the same pinned command locally when you change repository-wide
configuration.

## The README Family Block

The "The Ferramenta family" section of [README.md](README.md) is generated. Its
source of truth is the family registry in
[ferramenta](https://github.com/sebastian-software/ferramenta), so a new
sibling, a renamed tool or a moved documentation URL is edited once there and
every family repository follows. Never hand-edit the block between the
`<!-- ferramenta-family:start -->` and `<!-- ferramenta-family:end -->` markers.

```bash
./scripts/readme-family.sh check   # what CI runs; exits 1 on drift
./scripts/readme-family.sh write   # regenerate the block in place
```

Both modes need pnpm and Node >= 22.13. The script pins the generator to a
commit of the ferramenta repository, so the check verifies the same registry
today and next month. To pick up a registry change, bump `FERRAMENTA_PIN` in
[`scripts/readme-family.sh`](scripts/readme-family.sh), run the `write` mode,
and commit the regenerated block together with the new pin.

## Running Benchmarks

`battle_bench` requires a local Oniguruma source snapshot for comparison:

```bash
./scripts/prepare-oniguruma-sources.sh
cargo bench --features ffi --bench battle_bench
```

Exact external input revisions for the publishable battle suite are pinned in
[`benches/battle_inputs.toml`](benches/battle_inputs.toml).

For process-isolated memory comparison on the large TypeScript scanner
workload, use:

```bash
./scripts/run-battle-memory.sh
```

## Regenerating Unicode Tables

The checked-in Unicode tables are generated from upstream Oniguruma sources.
These scripts are maintainer tools; normal `cargo build`, tests, and CI do not
run them automatically.

```bash
./scripts/prepare-oniguruma-sources.sh
python3 scripts/gen_unicode_property_data.py
python3 scripts/gen_unicode_fold_data.py
```

If you regenerate them, commit the generated files together with the source
change:

- `src/unicode/property_data.rs`
- `src/unicode/fold_data.rs`

## Guidelines

1. **Read the ADRs first.** The ADRs live in `docs/app/routes/adr/` and are
   published at
   [sebastian-software.github.io/ferroni/adr](https://sebastian-software.github.io/ferroni/adr/001-one-to-one-parity-with-c-original).
   They document all major architectural decisions. In particular:
   - [ADR-001](https://sebastian-software.github.io/ferroni/adr/001-one-to-one-parity-with-c-original): the 1:1
     parity goal -- same module mapping, same function names, same control flow.
   - [ADR-004](https://sebastian-software.github.io/ferroni/adr/004-c-to-rust-translation-patterns): the canonical
     C-to-Rust translation patterns used throughout the codebase.
   - [ADR-002](https://sebastian-software.github.io/ferroni/adr/002-unsafe-code-policy): the `unsafe` code policy.

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

## License

By contributing, you agree that your contributions will be licensed under the
[BSD-2-Clause License](LICENSE).
