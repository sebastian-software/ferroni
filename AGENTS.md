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

[CONTRIBUTING.md](CONTRIBUTING.md#running-tests) carries the runnable command
list. Debug builds need a larger thread stack; the required `RUST_MIN_STACK`
values are stated once in ADR-013
(`docs/app/routes/adr/013-stack-overflow-debug-builds.mdx`) and are reused by
CONTRIBUTING.md and the CI workflow -- do not invent a third value.

Test counts come from `./scripts/count-tests.sh`, not from memory.

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
