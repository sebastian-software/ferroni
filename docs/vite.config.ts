import { defineConfig } from 'vite'
import { ardo } from 'ardo/vite'
import { readFileSync } from 'node:fs'

const cargoToml = readFileSync('../Cargo.toml', 'utf-8')
const version = cargoToml.match(/^version\s*=\s*"(.+)"/m)?.[1] ?? '0.0.0'

export default defineConfig({
  plugins: [
    ardo({
      title: 'Ferroni',
      description: 'Pure-Rust Oniguruma-compatible regex engine. Faster in the hot path, same feature class, no C toolchain.',

      project: { version },

      themeConfig: {
        siteTitle: 'Ferroni',

        nav: [
          { text: 'Guide', link: '/guide/getting-started' },
          { text: 'Performance', link: '/perf/benchmark-results' },
          { text: 'ADRs', link: '/adr/001-one-to-one-parity-with-c-original' },
        ],

        sidebar: [
          {
            text: 'Guide',
            items: [
              { text: 'Getting Started', link: '/guide/getting-started' },
            ],
          },
          {
            text: 'Performance',
            items: [
              { text: 'Benchmark Results', link: '/perf/benchmark-results' },
              { text: 'Memory Measurements', link: '/perf/memory-measurements' },
              { text: 'CSS Optimization Log', link: '/perf/css-optimization-log' },
              { text: 'HTML Entity Trie', link: '/perf/html-entity-trie-optimization' },
            ],
          },
          {
            text: 'Architecture Decision Records',
            items: [
              { text: 'ADR-001: 1:1 C Parity', link: '/adr/001-one-to-one-parity-with-c-original' },
              { text: 'ADR-002: Unsafe Code Policy', link: '/adr/002-unsafe-code-policy' },
              { text: 'ADR-003: Encoding Scope', link: '/adr/003-encoding-scope-ascii-and-utf8-only' },
              { text: 'ADR-004: C-to-Rust Patterns', link: '/adr/004-c-to-rust-translation-patterns' },
              { text: 'ADR-005: Idiomatic Rust API', link: '/adr/005-idiomatic-rust-api-layer' },
              { text: 'ADR-006: Scanner API', link: '/adr/006-scanner-api' },
              { text: 'ADR-007: SIMD Search', link: '/adr/007-simd-accelerated-search' },
              { text: 'ADR-008: Rust Optimizations', link: '/adr/008-rust-only-optimizations' },
              { text: 'ADR-009: Dependency Philosophy', link: '/adr/009-dependency-philosophy' },
              { text: 'ADR-010: Benchmark Strategy', link: '/adr/010-benchmark-strategy' },
              { text: 'ADR-011: Test Strategy', link: '/adr/011-test-strategy-and-c-test-parity' },
              { text: 'ADR-012: POSIX/GNU Not Ported', link: '/adr/012-posix-and-gnu-api-not-ported' },
              { text: 'ADR-013: Stack Overflow', link: '/adr/013-stack-overflow-debug-builds' },
              { text: 'ADR-014: Porting Bugs', link: '/adr/014-porting-bugs-lessons-learned' },
            ],
          },
        ],

        footer: {
          message: 'Released under the BSD-2-Clause License.',
        },

        search: {
          enabled: true,
        },
      },
    }),
  ],
})
