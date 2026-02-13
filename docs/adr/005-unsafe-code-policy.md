# ADR-005: Unsafe Code Policy

## Status

Accepted

## Context

A key motivation for the Rust port is eliminating the memory safety vulnerabilities that have affected C Oniguruma (CVE-2019-13224, CVE-2019-19204, CVE-2019-19246, CVE-2019-19012, CVE-2019-13225). However, some C patterns in Oniguruma cannot be expressed in safe Rust without a fundamental redesign that would violate the 1:1 parity goal (ADR-001).

## Decision

The codebase permits `unsafe` blocks under two narrowly scoped patterns only:

### Pattern 1: AST Raw Pointers (regcomp.rs)

Call nodes (`Node::Call`) share references to their target group nodes. In C, this is a simple pointer assignment. In Rust, the borrow checker cannot express "multiple nodes referencing the same mutable tree node" without `Rc<RefCell<>>` or arena allocation -- both of which would require redesigning the entire AST.

These raw pointers are:
- Set once during parsing (`prs_call`)
- Never freed independently (the AST owns all nodes)
- Valid for the lifetime of the regex compilation

### Pattern 2: Global Function Pointer Storage (regexec.rs)

Global callout callbacks (progress, retraction) and warn functions are stored as `AtomicPtr` with `transmute` for type erasure. This matches the C pattern of global function pointers and is necessary because Rust's type system cannot store `fn` pointers with different signatures in a single atomic.

### What is NOT allowed

No `unsafe` blocks for:
- Buffer arithmetic or bounds-skipping
- Memory allocation or deallocation
- String processing or encoding conversion
- Transmuting data types (only function pointers)

These are precisely the areas where C Oniguruma's CVEs occurred.

## Current State

86 `unsafe` blocks across ~20,400 LOC (0.4% of lines). All concentrated in the two patterns above.

## Consequences

- The port eliminates buffer over-read/write, use-after-free, double-free, NULL dereference, and uninitialized memory vulnerabilities structurally.
- The remaining `unsafe` blocks should be reviewed periodically. If Rust's type system evolves (e.g. better support for self-referential structs), these could potentially be eliminated.
- Any new `unsafe` block requires explicit justification and must fall into one of the two permitted patterns.
