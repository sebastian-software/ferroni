# Curated highlighting corpus

Selection accepted on September 30, 2026. This is the agreed scope for the
next Ferriki highlighting benchmark corpus; fixture and harness expansion
has not yet been implemented.

## Selected formats

The corpus contains exactly 20 formats. TypeScript and TSX are separate
cases because embedded JSX adds distinct highlighting work.

| # | Format |
| ---: | --- |
| 1 | TypeScript |
| 2 | TSX |
| 3 | Rust |
| 4 | CSS |
| 5 | HTML |
| 6 | C++ |
| 7 | Swift |
| 8 | Java |
| 9 | Markdown |
| 10 | TOML |
| 11 | YAML |
| 12 | JSON |
| 13 | Astro |
| 14 | Svelte |
| 15 | Ruby |
| 16 | Python |
| 17 | Vue |
| 18 | MDX |
| 19 | SCSS |
| 20 | Bash |

## Selection rationale

This manually curated selection targets typical highlighting workloads and
different grammar and scanner requirements. It covers programming languages,
styles, markup, configuration, documentation, and components with embedded
languages. Popularity rankings informed the discussion, but do not determine
membership or benchmark weights.

MDX combines Markdown with JSX and JavaScript expressions. SCSS adds nested
rules, variables, and mixins to the style workloads. Bash adds quoting,
command substitution, here-documents, and line continuations. Vue complements
the Astro and Svelte component cases.

## Fixture and measurement requirements

- Provide a small complete example and a larger realistic document for each
  format. Record source hashes, byte counts, and line counts.
- Exercise embedded languages in TSX, Vue, Astro, Svelte, Markdown, and MDX.
  Include YAML block scalars and anchors, and TOML tables and multiline strings.
- Measure warm token output and HTML output separately, retaining raw samples,
  median, and p95. Validate source preservation and output correctness before
  interpreting timing results.
- Keep corpus, grammars, theme, comparator versions, build settings, and
  hardware fixed during each Ferroni optimization comparison. Change the
  corpus in a separate commit and collect a new baseline afterward.
- Run complete production grammars through Ferriki for end-to-end measurements.
  Derive focused Ferroni scanner replays from observed hot workloads.

The existing TIOBE measurements remain historical evidence for their pinned
corpus. The [C++ scanner replay](cpp_scanner/README.md) and existing
backtracking regression workloads remain complementary diagnostics.

This selection complements the existing
[benchmark strategy](../docs/app/routes/adr/010-benchmark-strategy.mdx).
It does not replace the C-validated reference suite or its published results.
