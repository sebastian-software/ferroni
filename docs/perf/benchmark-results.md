# Benchmark Results

Full benchmark tables for Ferroni (Rust) vs C Oniguruma at `-O3` vs the
[`regex`](https://crates.io/crates/regex) crate.
Measured with [Criterion](https://github.com/bheisler/criterion.rs) on Apple M1 Ultra.

> Re-run with `cargo bench --features ffi` to get current numbers on your
> hardware.

## Three-way comparison (Tier 1)

Where the pattern is compatible with the `regex` crate syntax, we include
it for comparison. **Bold** = fastest engine. A dash means the feature is
not supported by the `regex` crate.

### Text search and log scanning

| Scenario | Ferroni | C Oniguruma | `regex` |
|----------|--------:|------------:|--------:|
| Literal in 50 KB | 74 ns | 150 ns | **10 ns** |
| No match, 50 KB | 1.53 us | 9.5 us | **1.46 us** |
| No match, 10 KB | 357 ns | 1.96 us | **298 ns** |
| Field extract, 50 KB | 101 ns | 172 ns | **56 ns** |
| Timestamp, 50 KB | 182 ns | 180 ns | **54 ns** |
| RegSet multi-pattern (5) | **101 ns** | 395 ns | — |

### Pattern matching

| Category | Ferroni | C Oniguruma | `regex` |
|----------|--------:|------------:|--------:|
| Literal exact | 104 ns | 159 ns | **11 ns** |
| Quantifier greedy | 185 ns | 319 ns | **65 ns** |
| Lookaround combined | **83 ns** | 292 ns | — |
| Unicode `\p{Greek}+` | 96 ns | 251 ns | **60 ns** |
| Backref `(\w+) \1` | **79 ns** | 199 ns | — |
| Case-insensitive phrase | 101 ns | 188 ns | **62 ns** |
| Alternation, 2 branches | 62 ns | 157 ns | **48 ns** |
| Alternation, 10 branches | 204 ns | 223 ns | **21 ns** |
| Named capture date | 355 ns | 277 ns | **44 ns** |

### Compilation

| Pattern | Ferroni | C Oniguruma | `regex` |
|---------|--------:|------------:|--------:|
| Literal | **439 ns** | 448 ns | 2.33 us |
| Named capture | **4.67 us** | 5.78 us | 193 us |
| Lookbehind | 992 ns | **556 ns** | — |

## Regex execution (Ferroni vs C, detailed)

| Benchmark | Rust | C | Ratio |
|-----------|-----:|--:|------:|
| **Literal match** | | | |
| exact string | **144 ns** | 148 ns | 0.97 |
| anchored start | **111 ns** | 148 ns | 0.75 |
| anchored end | 176 ns | **163 ns** | 1.08 |
| word boundary | **117 ns** | 172 ns | 0.68 |
| **Quantifiers** | | | |
| greedy | **240 ns** | 279 ns | 0.86 |
| lazy | **213 ns** | 233 ns | 0.91 |
| possessive | **204 ns** | 244 ns | 0.84 |
| nested | **194 ns** | 238 ns | 0.82 |
| **Alternation** | | | |
| 2 branches | **116 ns** | 159 ns | 0.73 |
| 5 branches | 240 ns | **170 ns** | 1.42 |
| 10 branches | 253 ns | **231 ns** | 1.10 |
| nested | 241 ns | **173 ns** | 1.40 |
| **Backreferences** | | | |
| simple `(\w+) \1` | **135 ns** | 191 ns | 0.71 |
| nested | **137 ns** | 197 ns | 0.70 |
| named | **137 ns** | 192 ns | 0.71 |
| **Lookaround** | | | |
| positive lookahead | **123 ns** | 163 ns | 0.75 |
| negative lookahead | **130 ns** | 176 ns | 0.74 |
| positive lookbehind | **119 ns** | 264 ns | 0.45 |
| negative lookbehind | **157 ns** | 340 ns | 0.46 |
| combined | **136 ns** | 288 ns | 0.47 |
| **Unicode properties** | | | |
| `\p{Lu}+` | **100 ns** | 145 ns | 0.69 |
| `\p{Letter}+` | **104 ns** | 165 ns | 0.63 |
| `\p{Greek}+` | **146 ns** | 245 ns | 0.60 |
| `\p{Cyrillic}+` | **285 ns** | 339 ns | 0.84 |
| **Case-insensitive** | | | |
| single word | **106 ns** | 150 ns | 0.71 |
| phrase | **154 ns** | 187 ns | 0.82 |
| alternation | **112 ns** | 156 ns | 0.72 |
| **Named captures** | | | |
| date extraction | 499 ns | **277 ns** | 1.80 |
| **Large text (first match)** | | | |
| literal 10 KB | **120 ns** | 145 ns | 0.83 |
| literal 50 KB | **122 ns** | 147 ns | 0.83 |
| timestamp 10 KB | 238 ns | **177 ns** | 1.35 |
| timestamp 50 KB | 236 ns | **176 ns** | 1.34 |
| field extract 10 KB | **163 ns** | 174 ns | 0.94 |
| field extract 50 KB | **162 ns** | 172 ns | 0.94 |
| no match 10 KB | **385 ns** | 1.9 us | 0.20 |
| no match 50 KB | **1.55 us** | 9.4 us | 0.16 |
| **RegSet** | | | |
| position-lead (5 patterns) | **101 ns** | 400 ns | 0.25 |
| regex-lead (5 patterns) | **186 ns** | 238 ns | 0.78 |
| **Match at position** | | | |
| `\d+` at offset 4 | **92 ns** | 154 ns | 0.60 |
| **Scanner** (vs vscode-oniguruma C) | | | |
| short string (RegSet path) | **51 ns** | 418 ns | 0.12 |
| long string, cold (per-regex) | **51 ns** | 190 ns | 0.27 |
| long string, warm (cached) | 52 ns | **23 ns** | 2.24 |

## Scanner with full Shiki TextMate grammars

Full, unmodified grammars from [shikijs/textmate-grammars-themes](https://github.com/shikijs/textmate-grammars-themes).

| Scenario | Ferroni | C Oniguruma | Factor |
|----------|--------:|------------:|-------:|
| **TypeScript (279 patterns)** | | | |
| Compile | **10.3 ms** | 17.0 ms | **1.6x** |
| First match, short line | **421 ns** | 25.5 us | **61x** |
| Tokenize full line | **7.0 us** | 224 us | **32x** |
| **CSS (117 patterns)** | | | |
| Compile | 399 ms | **19.1 ms** | 0.05x |
| Tokenize (multi-line) | **1.67 ms** | 15.3 ms | **9.2x** |
| **Rust (81 patterns)** | | | |
| Compile | 256 us | **180 us** | 0.70x |
| First match | **184 ns** | 5.7 us | **31x** |
| Tokenize full line | **8.3 us** | 84.9 us | **10x** |

## Regex compilation

| Pattern | Rust | C | Ratio |
|---------|-----:|--:|------:|
| literal | **448 ns** | 479 ns | 0.94 |
| `.*` | 798 ns | **553 ns** | 1.44 |
| alternation | 1.7 us | **1.5 us** | 1.14 |
| char class | **652 ns** | 657 ns | 0.99 |
| quantifier | 1.4 us | **1.1 us** | 1.34 |
| group | 1.1 us | **823 ns** | 1.36 |
| backref | 1.7 us | **987 ns** | 1.70 |
| lookahead | 772 ns | **495 ns** | 1.56 |
| lookbehind | 991 ns | **563 ns** | 1.76 |
| named capture | **4.7 us** | 5.9 us | 0.78 |

## Reproducing

```bash
# Full suite with C comparison (~8 min)
cargo bench --features ffi

# Tier 1 only (real-world scenarios)
cargo bench --features ffi -- scanner_highlighting
cargo bench --features ffi -- text_scanning
cargo bench --features ffi -- single_pattern
cargo bench --features ffi -- compilation

# Tier 2 (regression coverage)
cargo bench --features ffi -- regression_

# HTML report
open target/criterion/report/index.html
```
