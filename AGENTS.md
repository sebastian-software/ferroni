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

## Website

The public site lives in `docs/` and uses Ardo 6 with `ferramenta-family`:
every page wears the Monografie header and footer, the homepage is built from
the Monografie entry, and the documentation uses Monografie's documentation
pages (`monografie-docs.css` and the Ardo adapter `monografie-ardo.css`). No
page may import or render the Ferra theme or the original kit; the build fails
on a `ferra-` or `fam-` class other than Ferra's identity assets. Keep
presentation in the family package; the only local CSS styles the homepage's
own specimens, beside their components in `docs/app/components/`, with every selector scoped
to an `fr-` class and colors from the `--mono-*` tokens. React and static routing come from Ardo, not React Router.

Run `pnpm install --frozen-lockfile` and `pnpm verify` in `docs/` for lint,
types, formatting, benchmark claims, the production build and rendered links.
`pnpm sample:check` also reruns the Rust examples behind the homepage's
specimens and the guide's syntax examples. Check mobile and desktop rendering before shipping a design
change, including the documentation menu, search and benchmark selectors.

`docs/app/data/site.json` is generated from `Cargo.toml` and the documentation
frontmatter by `docs/scripts/write-site-data.mjs`; do not edit it by hand.
The dev, typecheck and build commands regenerate it. Retain published route
and section URLs when reorganizing the site.

## Testing

[CONTRIBUTING.md](CONTRIBUTING.md#running-tests) carries the runnable command
list. Debug builds need a larger thread stack; the required `RUST_MIN_STACK`
values are stated once in ADR-013
(`docs/app/routes/adr/013-stack-overflow-debug-builds.mdx`) and are reused by
`.cargo/config.toml`, CONTRIBUTING.md and the CI workflow -- do not invent a
third value.

Test counts come from `./scripts/count-tests.sh`, not from memory.

Coverage is gated in CI by `./scripts/coverage.sh`; the threshold is defined
there and nowhere else. The README badge quotes the same number, so move both
together.

## Tooling ownership

This repository owns its formatter, linter, and dependency-policy configuration.
Keep the native feature profiles and checks above; generic all-features checks
require the separately prepared C sources for the benchmark-only FFI features.
Shared infrastructure guidance lives in [project-infra](https://github.com/sebastian-software/project-infra).
