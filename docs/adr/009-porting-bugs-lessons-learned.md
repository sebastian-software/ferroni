# ADR-009: Porting Bugs -- Lessons Learned

## Status

Accepted

## Context

During the 1:1 port from C to Rust, 28 non-trivial bugs were found and fixed. All were logic divergences between the C and Rust implementations, not Rust-specific issues. Documenting these serves future contributors and anyone attempting a similar C-to-Rust port.

## Bug Catalog

### Address and Offset Arithmetic (5 bugs)

| # | Bug | Root Cause |
|---|-----|------------|
| 1 | JUMP/PUSH address formulas | Bytecode jump offsets calculated relative to wrong position |
| 2 | u_offset patching | Unicode offset not applied after compilation |
| 3 | usize subtraction overflow | Unsigned subtraction wrapping (C uses signed int, Rust uses usize) |
| 9 | compile_length_quantifier_node off-by-body_len | Body length miscounted by one operation |
| 10 | Alternation JUMP addresses (>2 branches) | Jump target wrong when alternation has 3+ branches |

**Lesson:** Bytecode address arithmetic is the highest-risk area in the port. Every offset formula must be validated against the C original with multi-branch test cases. Rust's unsigned `usize` makes subtraction bugs surface as panics rather than silent wraparound -- this is actually helpful for debugging.

### Parser Edge Cases (5 bugs)

| # | Bug | Root Cause |
|---|-----|------------|
| 4 | Parser operates on `&[u8]`, not `&str` | UTF-8 validation rejected valid byte patterns |
| 5 | Multi-byte codepoint truncation | U+305B has low byte 0x5B = `[`, confusing the parser |
| 6 | C trigraph escaping in tests | `\?` in C means `?` (trigraph avoidance), not an escape |
| 19 | `\pL` single-char property tokenizer | Property name tokenizer didn't handle single-char names |
| 20 | `check_code_point_sequence` must not advance `p` | Function side-effected the position cursor |

**Lesson:** The parser must work on raw bytes, not Rust strings. Multi-byte UTF-8 codepoints can have byte values that coincide with ASCII syntax characters -- the parser must always check encoding boundaries.

### VM State Machine (10 bugs)

| # | Bug | Root Cause |
|---|-----|------------|
| 7 | Empty-loop detection | `tune_tree` must set `qn.emptiness` for quantifier bodies that can match empty |
| 8 | String-split for quantifiers | `check_quantifier` must split string nodes so the quantifier applies to the last character only |
| 11 | Missing `??` and greedy expansion paths | Non-greedy optional and greedy quantifier code paths were incomplete |
| 12 | EmptyCheckEnd skip-next | Empty check must skip the next operation (the repeat increment) |
| 13 | backtrack_mem / backrefed_mem | Bitmasks tracking which captures are involved in backtracking were not propagated |
| 14 | EmptyCheckEndMemst | Memory-aware empty check variant not dispatching to correct handler |
| 16 | FIXED_INTERVAL_IS_GREEDY_ONLY | Fixed-interval quantifiers must only apply the greedy optimization path |
| 17 | StrN payload mismatch | String operation payload length field disagreed with actual string length |
| 18 | RepeatIncNg stack ordering | Non-greedy repeat increment pushed stack entries in wrong order |
| 24 | EmptyCheckEnd mem-ID mismatch | Nested quantifiers generated mismatched empty-check IDs |

**Lesson:** The VM's empty-loop detection and backtracking state are deeply intertwined. A bug in `emptiness` detection cascades into infinite loops or missed matches. Test with patterns like `(a*)*`, `(a?)*`, `(a*)+` and their non-greedy variants.

### Optimizer and Compiler (5 bugs)

| # | Bug | Root Cause |
|---|-----|------------|
| 15 | Reversed intervals `{5,2}` | Interval with lower > upper must be swapped, not rejected |
| 22 | SaveVal ID collision | Save-value IDs for different purposes collided |
| 23 | `reduce_string_list` loses `ND_ST_SUPER` | String merge optimization dropped the "super" status flag |
| 25 | Call-node body vs target_node in optimizer | Optimizer followed wrong pointer (call body instead of target group) |
| 26 | `(?P:...)` parser fallthrough | `P` case must delegate to `prs_options` when `QMARK_CAPITAL_P_NAME` syntax is not set |

**Lesson:** Optimizer passes must preserve all node flags and status bits. When merging or transforming nodes, every flag on the source node must be checked for carry-over.

### Executor Dispatch (3 bugs)

| # | Bug | Root Cause |
|---|-----|------------|
| 21 | CUT_TO_MARK must void, not pop | Mark-cutting operation must void (discard) stack entries, not pop them |
| 27 | WordBoundary executor ignored mode parameter | ASCII-mode `(?W:\b)` always used Unicode word check instead of ASCII-only |
| 28 | TextSegmentBoundary compiler used `reg.options` | Should use per-node status `ND_ST_TEXT_SEGMENT_WORD`, not global regex options |

**Lesson:** The VM executor has mode-sensitive opcodes where behavior depends on flags embedded in the operation payload. Always verify that the correct flag source (node status vs. regex options vs. operation mode field) is consulted.

## Key Takeaways

1. **All 28 bugs were logic divergences**, not Rust-specific issues. The 1:1 porting approach (ADR-001) made every bug diagnosable by line-by-line comparison with the C source.

2. **Rust caught several bugs earlier than C would have.** Unsigned integer overflow panics (bug #3), exhaustive match requirements (bugs #11, #14), and the borrow checker (bug #20) all surfaced issues that might have been silent in C.

3. **The highest-risk areas** are bytecode address arithmetic, VM empty-loop detection, and optimizer flag preservation. These deserve the most careful review in any future changes.

4. **Multi-byte UTF-8 is a persistent source of parser bugs.** Any byte in a multi-byte sequence can coincide with an ASCII syntax character. The parser must always respect encoding boundaries.

## Consequences

- New contributions to the compiler or VM should include test cases for edge cases listed above (empty quantifiers, nested alternations, mode-sensitive opcodes).
- Code review should cross-reference the C original for any function in `regcomp.rs`, `regexec.rs`, or `regparse.rs`.
