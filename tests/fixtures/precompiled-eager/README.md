# Frozen prepared Eager pair

Native-produced conformance input for the public `PrecompiledEager` API. The
pair was generated on aarch64 macOS from the stabilization branch based on
Ferroni 2.6.0 (`5c27b0894211720f00714f17aecc7ab9952d49a5`), using ordinary program
format 2 and portable group format FPRT003. `tests/support/precompiled_eager.rs`
records the exact ordered pattern lists and exercises ordering, duplicates,
captures, lookbehind/backreference fallback, empty matches, UTF-8, and UTF-16.

These bytes are intentionally frozen. Native, WASI, and the browser-target
example consume the same files. Re-export must preserve the bytes. Do not
regenerate merely to make a changed writer pass: assess the format contract
and bump its version for incompatible changes.

Reproduce the original export with compatible source and settings:

```bash
cargo run --release --locked --features precompiled-eager --example precompiled_eager -- generate .cache/prepared-fixture
```

SHA-256:

```text
5ddbc5e434ddaa79ed5b8ddf3b7fd41240d319cfb90af84a3424571e25856fd1  patterns.portable
a4b4d19a6cd687972184085e82871799bb418fe1891d8ea5671c583a6ebbbbe6  patterns.programs
```
