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

## Running Tests

Debug builds require increased stack size:

```bash
# Full UTF-8 test suite (1,554 tests)
RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1

# Other suites
cargo test --test compat_syntax
cargo test --test compat_options
cargo test --test compat_regset
RUST_MIN_STACK=268435456 cargo test --test compat_back -- --test-threads=1
```

> **Warning:** Never run `cargo test -- --ignored` -- the
> `conditional_recursion_complex` test intentionally hangs.

## Running Benchmarks

`battle_bench` requires a local Oniguruma source snapshot for comparison:

```bash
./scripts/prepare-oniguruma-sources.sh
cargo bench --features ffi --bench battle_bench
```

Exact external input revisions for the publishable battle suite are pinned in
[`benches/battle_inputs.toml`](benches/battle_inputs.toml).

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

1. **Read the ADRs first.** The [`docs/adr/`](docs/adr/) directory documents
   all major architectural decisions. In particular:
   - [ADR-001](docs/adr/001-one-to-one-parity-with-c-original.md): the 1:1
     parity goal -- same module mapping, same function names, same control flow.
   - [ADR-004](docs/adr/004-c-to-rust-translation-patterns.md): the canonical
     C-to-Rust translation patterns used throughout the codebase.
   - [ADR-002](docs/adr/002-unsafe-code-policy.md): the `unsafe` code policy.

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
