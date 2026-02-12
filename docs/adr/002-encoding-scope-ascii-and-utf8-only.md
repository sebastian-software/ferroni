# ADR-002: Encoding Scope — ASCII and UTF-8 Only

## Status

Accepted

## Context

The C original supports 29 character encodings: ASCII, UTF-8, UTF-16 BE/LE, UTF-32 BE/LE, 15 ISO-8859 variants, EUC-JP/KR/TW, Shift-JIS, Big5, GB18030, KOI8, KOI8-R, and CP1251. These are implemented across 32 source files totaling ~7,000 LOC, plus encoding-specific property tables (e.g. `euc_jp_prop.c`, `sjis_prop.c`).

The question is which encodings to port.

## Decision

The Rust port supports **only ASCII and UTF-8**. No other encodings will be ported.

### What this means

- `ONIG_ENCODING_ASCII` and `ONIG_ENCODING_UTF8` are the only two available encodings.
- The `Encoding` trait and dispatch infrastructure exist and are fully functional — additional encodings *could* be added, but we choose not to.
- The 27 missing encodings (~6,600 LOC of C code) are intentionally excluded.
- Encoding-specific public API functions (`onigenc_set_default_encoding`, `onig_initialize_encoding`, `onigenc_strlen_null`, etc.) are not ported.
- C test files that depend on other encodings (`testc.c` for EUC-JP, `testu.c` for UTF-16, `testp.c` for POSIX) are not ported.

## Rationale

- **UTF-8 has won.** In modern software, UTF-8 is the dominant encoding for text processing. ASCII is its subset. Together they cover virtually all contemporary use cases.
- **Effort vs. value.** Porting 27 encodings would add ~6,600 LOC of mechanical translation work with near-zero benefit for the target audience. Each encoding requires implementing 15+ trait methods, porting lookup tables, and writing or adapting tests.
- **The encoding trait is the escape hatch.** If a specific encoding is ever needed, the infrastructure is in place — it is a contained, additive change that does not affect the core engine.
- **Reduced attack surface.** Fewer encodings mean fewer code paths to audit and fewer potential encoding-related bugs.

## Consequences

- Users who need non-UTF-8 encodings (e.g. Japanese legacy systems using EUC-JP or Shift-JIS) cannot use this port. They should use the C original via FFI bindings.
- `onig_new_deluxe` (which allows specifying different pattern and string encodings) is not ported — it is only useful with multiple encodings.
- Test parity is 100% for UTF-8 (`test_utf8.c`: 1,554/1,554), but overall C test coverage is partial (~1,554 of ~3,970 encoding-dependent test cases).
