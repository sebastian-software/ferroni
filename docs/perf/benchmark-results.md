# Benchmark Results

Raw benchmark tables for the `battle_bench` reference suite:
Ferroni (Rust) vs Oniguruma at `-O3`, with
[`regex`](https://crates.io/crates/regex) included where the syntax is
compatible.

Measured with [Criterion](https://github.com/bheisler/criterion.rs) on Apple
M1 Ultra.

> Re-run with `cargo bench --features ffi --bench battle_bench` to get
> current numbers on your hardware.

The README intentionally rounds values for readability. This file keeps the
raw numbers.

## Reference suite (`battle_bench`)

Where the pattern is compatible with the `regex` crate syntax, we include it
for comparison. **Bold** = fastest engine. A dash means the feature is not
supported by the `regex` crate.

### Text search and log scanning

| Scenario | Ferroni | Oniguruma | `regex` |
|----------|--------:|------------:|--------:|
| Literal in 50 KB | 74 ns | 150 ns | **10 ns** |
| No match, 50 KB | 1.53 us | 9.5 us | **1.46 us** |
| No match, 10 KB | 357 ns | 1.96 us | **298 ns** |
| Field extract, 50 KB | 127 ns | 172 ns | **56 ns** |
| Timestamp, 50 KB | **120 ns** | 177 ns | **54 ns** |
| RegSet multi-pattern (5) | **101 ns** | 395 ns | — |

### Pattern matching

| Category | Ferroni | Oniguruma | `regex` |
|----------|--------:|------------:|--------:|
| Literal exact | 104 ns | 159 ns | **11 ns** |
| Quantifier greedy | 183 ns | 261 ns | **65 ns** |
| Lookaround combined | **83 ns** | 292 ns | — |
| Unicode `\p{Greek}+` | 96 ns | 251 ns | **60 ns** |
| Backref `(\w+) \1` | **79 ns** | 199 ns | — |
| Case-insensitive phrase | 101 ns | 188 ns | **62 ns** |
| Alternation, 2 branches | 62 ns | 157 ns | **48 ns** |
| Alternation, 10 branches | 49 ns | 225 ns | **21 ns** |
| Named capture date | 361 ns | 277 ns | **44 ns** |

### Compilation

| Pattern | Ferroni | Oniguruma | `regex` |
|---------|--------:|------------:|--------:|
| Literal | **439 ns** | 448 ns | 2.33 us |
| Named capture | **4.67 us** | 5.78 us | 193 us |
| Lookbehind | 992 ns | **556 ns** | — |

### Scanner with full Shiki TextMate grammars

Full, unmodified grammars from
[shikijs/textmate-grammars-themes](https://github.com/shikijs/textmate-grammars-themes).

| Scenario | Ferroni | Oniguruma | Factor |
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

## Reproducing

```bash
# Reference suite for publishable Ferroni-vs-C numbers
cargo bench --features ffi --bench battle_bench

# Internal Rust-only regression suite
cargo bench --bench codspeed_bench

# HTML report
open target/criterion/report/index.html
```
