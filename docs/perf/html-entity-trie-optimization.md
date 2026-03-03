# HTML Entity Trie Optimization — Status & Next Steps

## Context

HTML is the only language where Rust loses to JS in `codeToHtml` benchmarks.
The HTML TextMate grammar contains an entity pattern with **~1,700 alternations in ~11,600 chars** — a trie encoded as nested regex:

```
a(s(ymp(eq)?|cr|t)|n(d(slope|[dv]|and)?|g(...)))
```

Ferroni's existing trie optimizer (`detect_literal_alternations`) only handled **flat** alternations (`word1|word2|word3`). It couldn't detect nested trie structures because `check_literal_branch()` only recognized plain `String` nodes.

## What was implemented (2026-03-03)

All changes in `ferroni/src/regcomp.rs`:

### 1. `extract_literal_paths()` — recursive nested extraction

Walks a nested AST branch and extracts all possible literal byte sequences:

- **String**: append bytes to each prefix
- **List** (sequence): thread prefixes through car → cdr
- **Alt**: fork into each branch, collect all resulting paths
- **Bag** (Memory if not backreferenced, Option): recurse into body
- **Quant(0,1)** (optional `?`): fork into "with" and "without" paths
- **Anything else**: return `None` (not a literal)

Bounded by `MAX_NESTED_TRIE_PATHS = 8192` to prevent exponential blowup.

### 2. Top-down processing order

Changed `detect_literal_alternations_inner` from **bottom-up** to **top-down**:

- **Before**: recurse into children first, then try to optimize the current Alt. Inner Alts got trie-optimized first, making them opaque (`ND_ST_LITERAL_ALT`) to outer extraction.
- **After**: try `extract_literal_paths` on the current Alt first. If it succeeds, build one big trie covering many nested branches. Then recurse into remaining non-literal branches. Falls back to bottom-up for Alts where top-down extraction doesn't meet threshold.

Refactored into:
- `try_trie_optimize_alt()` — shared logic for branch classification + trie building
- `recurse_into_children()` — extracted recursion helper
- `classify_branch()` — tries fast path (`check_literal_branch`), then `extract_literal_paths`

### 3. `backrefed_mem` propagation

Added `backrefed_mem: MemStatusType` parameter through the detection chain so `extract_literal_paths` can safely skip backreferenced capture groups.

### 4. Tests (6 new, all passing)

- `nested_alt_trie_simple` — `a(b|c|d|e|f)g`
- `nested_alt_trie_optional` — branch with `?` quantifier
- `nested_alt_trie_partial_with_cclass` — mixed literal + CClass
- `nested_alt_trie_entity_like` — large nested pattern (pure-literal subset)
- `nested_alt_trie_backreferenced_capture_skipped` — backref safety
- `nested_alt_trie_non_capturing_group` — `(?:...)` transparency

### 5. Benchmark update (ferriki)

Updated `bench/engines/shiki-rust-napi.bench.ts`:
- Replaced tiny HTML fallback sample with a realistic Wikipedia-style page full of HTML entities (`&pi;`, `&mdash;`, `&amp;`, `&hellip;`, `&sum;`, `&infin;`, etc.)
- Added `createHtmlSample()` generator for ~100KB entity-heavy HTML
- Added `large-html-entities-100kb` benchmark section

## Results

### Pure-literal nested patterns: works perfectly

Entity-like pattern without CClass nodes → **1 trie, 1 AltLiterals op, 0 Push ops** (down from ~15 Push ops with bottom-up approach).

### Real HTML entity pattern: limited impact

The actual entity regex has character classes scattered within nested branches:
- `[dv]` (2 members) in `d(slope|[dv]|and)?`
- `[a-h]` (8 members) in `msd(a([a-h]))?`
- `[Ee]` (2 members) in `p(id|os|prox(eq)?|[Ee]|acir)?`
- `[DUdu]` (4 members) — directional variants (Down/Up)
- `[LRlr]` (4 members) — directional variants (Left/Right)
- `[HLRhlr]` (6 members) — multi-directional variants
- `[er]` (2 members) — suffix variants
- `[0-9]` (10 members) — numeric suffixes (e.g. `frac12`, `frac34`)

All CClass nodes in the entity pattern have **≤ 10 single-byte members**. None are negated, none contain multi-byte ranges.

These make `extract_literal_paths` return `None` for the containing branch, preventing full extraction. Result: partial optimization (some branches become trie, CClass-containing branches remain as backtracking).

### E2E benchmark numbers

| Benchmark | Before | After | Change |
|-----------|--------|-------|--------|
| codeToHtml > html (small sample) | JS 1.20x faster | — | — |
| codeToHtml > html (entity-heavy) | — | JS 1.48x faster | Entities actively hurt Rust |
| large-html-entities-100kb | — | JS ~1.19x faster | — |

The entity-heavy sample makes the gap **worse**, confirming that entity matching is a real bottleneck and the current partial optimization is insufficient.

## Pre-existing ASAN bug (unrelated)

ASAN detected a heap-use-after-free in `node_max_byte_len` for patterns with named group captures (`(.)(((?<_>a)))\k<_>`). The freed memory comes from `disable_noname_group_capture`. This bug exists on `main` without any of these changes — confirmed by running ASAN on the clean baseline. The trie changes just shift allocation patterns enough to make it crash instead of silently reading stale memory.

## Assessment of current changes

**Keep.** The changes are:
- Correct (213/213 lib tests pass, 43/43 compat_syntax pass, compat_utf8 only has a pre-existing failure)
- Non-regressive (no performance penalty for non-entity patterns)
- Foundation for both next-step approaches below
- The benchmark improvements (realistic HTML sample, large-html benchmark) provide proper measurement infrastructure regardless of which optimization path is pursued

---

## Next Steps: Two approaches

### Approach A: Extend `extract_literal_paths` to handle simple CClass nodes

**Idea**: When `extract_literal_paths` encounters a `CClass` node containing only a small number of single-byte members, enumerate them and fork into one path per member — same as `Alt` forking but driven by the character class.

**Example**: `d(slope|[dv]|and)?` currently fails because `[dv]` is a CClass. With expansion:
- `[dv]` → fork into `d` and `v` paths
- The whole branch extracts to: `["dslope", "dd", "dv", "dand", "d"]`

**Implementation sketch** (all in `regcomp.rs`):

```rust
// In extract_literal_paths, add a new arm:
NodeInner::CClass(ref cc) => {
    // Only expand small, single-byte-per-member classes
    let members = cc.enumerate_single_bytes(); // new helper
    if members.is_empty() || members.len() > 32 {
        return None; // too large or contains multi-byte chars
    }
    let mut all = Vec::new();
    for byte in members {
        for prefix in &current_prefixes {
            let mut p = prefix.clone();
            p.push(byte);
            all.push(p);
        }
    }
    if all.len() > limit { return None; }
    Some(all)
}
```

The `enumerate_single_bytes()` helper on `CClassNode` would:
1. Check that the class uses only single-byte (ASCII) ranges
2. Return a `Vec<u8>` of all member bytes
3. Return empty if the class contains multi-byte Unicode ranges, negation, or is too large

**Risks**:
- Path explosion: `[a-z]` has 26 members, `[A-Za-z]` has 52. Combined with optionals and nested alts, this could blow past `MAX_NESTED_TRIE_PATHS`. Mitigation: limit individual CClass expansion to e.g. 8 members.
- The HTML entity regex has `[a-h]` (8 members) and `[dv]` (2 members) — both small. `[Ee]` is 2 members. So a limit of ~8-16 would cover all cases in the entity pattern.

**Expected impact**: High. Most CClass nodes in the entity pattern are small (2-8 members). Full extraction would collapse the entity alternation into a single trie with ~2000 entries, reducing entity matching from O(1700 * len) to O(len).

**Effort**: Small. The internal representation is simple and well-suited for enumeration.

#### CClassNode internals (verified)

`CClassNode` in `regparse_types.rs:481` has three fields:

```rust
pub struct CClassNode {
    pub flags: u32,        // FLAG_NCCLASS_NOT (negation), FLAG_NCCLASS_SHARE
    pub bs: BitSet,        // [u32; 8] = 256 bits, one per single-byte value
    pub mbuf: Option<BBuf>, // Multi-byte Unicode ranges (code points ≥ 256)
}
```

`BitSet = [u32; 8]` covers all 256 single-byte values. Membership check via `bitset_at(bs, pos)` in `regint.rs:153`.

**`enumerate_single_bytes()` implementation** — trivial:

```rust
impl CClassNode {
    fn enumerate_single_bytes(&self) -> Option<Vec<u8>> {
        // Bail on negated classes ([^...]) — would expand to ~250 entries
        if self.is_not() { return None; }
        // Bail if multi-byte ranges exist — can't represent in trie
        if self.mbuf.is_some() { return None; }

        let mut members = Vec::new();
        for i in 0u16..256 {
            if bitset_at(&self.bs, i as usize) {
                members.push(i as u8);
            }
        }
        if members.is_empty() { return None; }
        Some(members)
    }
}
```

No complex parsing needed — just iterate 256 bits. The guard clauses (`is_not()`, `mbuf.is_some()`) handle all edge cases.

#### Path explosion analysis for the HTML entity pattern

Worst-case multiplicative effect of CClass expansion on the entity pattern:

| CClass | Members | Context | Multiplier |
|--------|---------|---------|------------|
| `[dv]` | 2 | inside optional branch | ×2 |
| `[Ee]` | 2 | inside optional branch | ×2 |
| `[er]` | 2 | suffix variant | ×2 |
| `[DUdu]` | 4 | directional | ×4 |
| `[LRlr]` | 4 | directional | ×4 |
| `[HLRhlr]` | 6 | multi-directional | ×6 |
| `[a-h]` | 8 | inside optional+capture | ×8 |
| `[0-9]` | 10 | numeric suffix | ×10 |

These CClass nodes appear in **separate top-level branches** of the entity trie (different initial letters), so their multipliers don't compound with each other. The base ~1,700 literal paths would grow to roughly **~2,000-2,200 paths** — well within the `MAX_NESTED_TRIE_PATHS = 8192` limit.

**Recommended CClass expansion limit**: **16 members per class**. This covers all entity-pattern cases while rejecting `[a-z]` (26) and `[A-Za-z]` (52).

### Approach B: Profile the actual HTML bottleneck

**Idea**: The entity pattern might not be the primary bottleneck. The 1.48x JS advantage on entity-heavy HTML could come from other grammar rules, the tokenizer loop overhead, or the HTML-to-string conversion cost. Profile before optimizing further.

**Implementation sketch**:

1. **Micro-benchmark the entity regex directly in ferroni**:
   ```rust
   // In ferroni/benches/onig_bench.rs: add a benchmark that compiles the
   // full entity regex and matches it against entity-heavy input
   let re = Regex::new(ENTITY_PATTERN).unwrap();
   b.iter(|| re.find("&pi; &mdash; &amp; &hellip; ..."));
   ```
   This isolates regex matching cost from tokenizer/rendering overhead.

2. **Instrument ferriki's tokenizer loop**:
   - Count how many times the entity regex is attempted vs. matches
   - Measure cumulative time in `match_at` for the entity pattern vs. other patterns
   - Use `perf`/`Instruments` on the native benchmark

3. **Compare JS engine's entity handling**:
   - The JS regex engine (Oniguruma WASM or native `RegExp`) may handle the entity pattern differently
   - JS `RegExp` with the same trie-encoded pattern might use V8's internal trie optimization automatically

**Expected impact**: May reveal that the bottleneck is not the entity regex at all, redirecting effort to where it matters.

**Effort**: Medium. Requires profiling tooling and analysis.

### Recommendation

**Start with Approach A** — it's a focused, incremental change that directly addresses the known gap (CClass nodes blocking extraction). If CClass expansion doesn't close the gap, Approach B provides the diagnostic data to find the real bottleneck.

The two approaches are complementary: A is a concrete optimization, B is a diagnostic. Even if A provides the expected improvement, B's profiling infrastructure is valuable for future work on other languages.

### Decision gate

After implementing Approach A, run the entity-heavy benchmark:
- **If Rust closes the gap to ≤ 1.05x JS**: Entity trie was the bottleneck. Done.
- **If gap remains > 1.10x JS**: Entity trie was only part of the problem. Proceed to Approach B.
- **If gap is unchanged**: CClass branches were rarely hit in practice. Approach B becomes critical to find the real bottleneck.

### Additional context: Pattern structure (912 capture groups)

The full entity pattern structure is: `(&)(?=[A-Za-z])((nested_trie))(;)` with **912 capture groups** total. The lookahead `(?=[A-Za-z])` provides minimal early filtering — it only checks that the next character is a letter, but doesn't narrow down which of the ~1,700 branches will match. This means the regex engine must attempt the full trie on every `&` followed by a letter in the input.

Note: Shiki JS uses the same Oniguruma engine (compiled to WASM via `vscode-oniguruma`), not V8's native `RegExp`. This means the performance difference is not due to V8's regex optimizations but rather differences in the Oniguruma compilation/execution path between native Rust FFI and WASM. This is worth investigating in Approach B — WASM Oniguruma may have different memory access patterns or the JS wrapper may batch regex operations differently.
