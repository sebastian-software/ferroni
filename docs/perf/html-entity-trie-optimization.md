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
- `[dv]` in `d(slope|[dv]|and)?`
- `[a-h]` in `msd(a([a-h]))?`
- `[Ee]` in `p(id|os|prox(eq)?|[Ee]|acir)?`

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

**Effort**: Small-medium. Need `enumerate_single_bytes()` on `CClassNode` (requires understanding the bitset/range representation) + one new match arm in `extract_literal_paths`.

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
