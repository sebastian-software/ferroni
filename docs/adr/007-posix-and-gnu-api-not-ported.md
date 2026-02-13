# ADR-007: POSIX and GNU API Not Ported

## Status

Accepted

## Context

C Oniguruma includes three compatibility layers:

- `regposix.c` (~300 LOC): POSIX `regcomp`/`regexec`/`regfree`/`regerror` API wrapper
- `regposerr.c` (~100 LOC): POSIX error message strings
- `reggnu.c` (~100 LOC): GNU regex compatibility (`re_compile_pattern`, `re_search`, etc.)

These exist so that C programs using the standard POSIX or GNU regex interfaces can be linked against Oniguruma as a drop-in replacement.

## Decision

None of these compatibility layers are ported.

## Rationale

- **No use case in Rust.** The POSIX regex API (`<regex.h>`) is a C-world interface. Rust programs do not call `regcomp`/`regexec` -- they use Rust-native APIs. The Oniguruma API (`onig_new`/`onig_search`) is the native interface and is fully ported.
- **The GNU API is equally irrelevant.** `re_compile_pattern` and friends are GNU extensions to the POSIX API, used in C programs like `grep`. Same reasoning applies.
- **The associated test suite (`testp.c`, 421 tests) tests the POSIX wrapper, not the engine.** Skipping it does not reduce engine test coverage.

## Consequences

- C programs that use Oniguruma through the POSIX API cannot migrate to this Rust port without changing their regex interface calls.
- This is consistent with ADR-002 (encoding scope) -- both decisions reduce the porting surface to what is actually useful in a Rust context.
