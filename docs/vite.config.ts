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
    }),
  ],
})
