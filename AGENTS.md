# AGENTS.md

Guidance for coding agents working in this repository. Humans welcome too.

## What this is

Ferroni is a 1:1 Rust port of the
[Oniguruma](https://github.com/kkos/oniguruma) regex engine. Structural
fidelity to the C original is the point: same module mapping, same function
names, same control flow.

## Language

**Project language is US English.** All code, comments, variable/function
names, commit messages, documentation files, ADRs, and test names MUST be in
English.

## Orientation

- [CONTRIBUTING.md](CONTRIBUTING.md) is the canonical preflight: build, test
  suites, benchmarks, and the Unicode table regeneration scripts.
- The ADRs in `docs/app/routes/adr/` are project constraints, not decisions to
  re-litigate in routine work. Read ADR-001 (1:1 parity with the C original),
  ADR-004 (C-to-Rust translation patterns) and ADR-002 (`unsafe` code policy)
  before changing direction.
- Conventional commits without exception; release-please depends on them.

## Testing

Run the full test suite with increased stack size (debug builds require it):

```bash
RUST_MIN_STACK=268435456 cargo test --test compat_utf8 -- --test-threads=1
```

Or with multiple threads (lower stack needed):

```bash
RUST_MIN_STACK=67108864 cargo test --test compat_utf8 -- --test-threads=4
```

WARNING: Never run `cargo test -- --ignored` on the full suite -- the
`conditional_recursion_complex` test hangs.

---

<!-- sebastian-software-consumer-agents:start -->

# Standards-managed repo guardrails

- Do not hand-edit managed files or standards-owned marker sections.
- If `standards check` reports drift, run `standards apply` or update standards.
- `pnpm agent:check` may omit `standards check`; CI can still fail on drift.
- Fix or format every file reported by `oxfmt` whenever practical.
- For generated files, prefer formatting in the generator step.
- If formatting is not viable, use repo-local `.prettierignore`.
- Never add repo-specific ignores to managed `.oxfmtrc.json`.
<!-- sebastian-software-consumer-agents:end -->
