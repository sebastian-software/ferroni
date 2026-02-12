# C Oniguruma vs. Rust Port — Full Comparison

> As of: 2026-02-12 | 1468/1468 tests passing, 0 ignored

---

## Overview

| Module | C Lines | Rust Lines | Core Logic | API Completeness |
|--------|---------|------------|------------|------------------|
| **Parser** (regparse.c) | 9,493 | ~6,600 | ~97% | Core complete, 6x6 ReduceTypeTable |
| **Compiler** (regcomp.c) | 8,589 | ~6,600 | ~92% | tune_tree + tune_next + call tuning + optimization |
| **Executor** (regexec.c) | 7,002 | ~4,060 | ~90% | All 70+ opcodes, forward+backward search, safety limits |
| **Types** (regint.h+regparse.h) | ~1,564 | ~2,120 | ~95% | All core types incl. OptNode/OptStr/OptMap |
| **Syntax** (regsyntax.c) | 373 | 367 | 100% | Complete |
| **Error handling** (regerror.c) | 416 | 206 | 100% | Complete (Rust needs less boilerplate) |
| **Encoding** (regenc.c+h) | ~1,284 | ~916 | ~70% | ASCII + UTF-8 (2 of 29) |
| **Unicode** | ~50,000 | ~50,000 | 100% | Complete |

---

## 1. What is COMPLETE

### All 70+ Opcodes in the VM Executor

STR_1..STR_N, STR_MB2N1..STR_MBN, CCLASS/CCLASS_MB/CCLASS_MIX (+ NOT variants),
ANYCHAR, ANYCHAR_ML, ANYCHAR_STAR, ANYCHAR_ML_STAR, ANYCHAR_STAR_PEEK_NEXT,
WORD/NO_WORD (+ ASCII), WORD_BOUNDARY/NO_WORD_BOUNDARY/WORD_BEGIN/WORD_END,
TEXT_SEGMENT_BOUNDARY, BEGIN_BUF/END_BUF/BEGIN_LINE/END_LINE/SEMI_END_BUF,
CHECK_POSITION, BACKREF1/BACKREF2/BACKREF_N/BACKREF_N_IC/BACKREF_MULTI/BACKREF_MULTI_IC,
BACKREF_WITH_LEVEL/BACKREF_WITH_LEVEL_IC, BACKREF_CHECK/BACKREF_CHECK_WITH_LEVEL,
MEM_START/MEM_START_PUSH/MEM_END/MEM_END_PUSH/MEM_END_REC/MEM_END_PUSH_REC,
FAIL, JUMP, PUSH, PUSH_SUPER, POP, POP_TO_MARK, PUSH_OR_JUMP_EXACT1, PUSH_IF_PEEK_NEXT,
REPEAT/REPEAT_NG/REPEAT_INC/REPEAT_INC_NG,
EMPTY_CHECK_START/EMPTY_CHECK_END/EMPTY_CHECK_END_MEMST/EMPTY_CHECK_END_MEMST_PUSH,
MOVE, STEP_BACK_START/STEP_BACK_NEXT, CUT_TO_MARK/MARK, SAVE_VAL/UPDATE_VAR,
CALL/RETURN, CALLOUT_CONTENTS/CALLOUT_NAME, FINISH/END

### All Escape Sequences

`\a \b \t \n \v \f \r \e` (control chars),
`\x{HH}` `\x{HHHH}` (hex), `\o{OOO}` (octal), `\uHHHH` (Unicode),
`\w \W \d \D \s \S \h \H` (character classes),
`\p{...} \P{...}` (Unicode properties),
`\k<name> \g<name>` (backrefs/calls),
`\A \z \Z \b \B \G` (anchors), `\K` (keep),
`\R` (general newline), `\N` (no-newline), `\O` (true anychar),
`\X` (grapheme cluster), `\y \Y` (text segment boundaries)

### All Group Types

`(?:...)` non-capturing, `(?=...)` lookahead, `(?!...)` neg. lookahead,
`(?<=...)` lookbehind, `(?<!...)` neg. lookbehind, `(?>...)` atomic,
`(?<name>...)` named group, `(?'name'...)` alternate named,
`(?(cond)T|F)` conditional, `(?~...)` absent (3 forms),
`(?{...})` code callout, `(*FAIL)` `(*MAX{n})` `(*COUNT[tag]{X})` `(*CMP{t1,op,t2})`,
`(?imxWDSPCL-imx:...)` option groups

### Optimization Subsystem

Full forward search optimization matching the C original:
- **AST analysis**: `optimize_nodes` extracts OptStr/OptMap/OptAnc per node
- **Strategy selection**: `set_optimize_info_from_tree` chooses best search strategy
  - `OptimizeType::StrFast` — Boyer-Moore-Horspool with skip table
  - `OptimizeType::StrFastStepForward` — multi-byte-safe BMH variant
  - `OptimizeType::Str` — naive string search
  - `OptimizeType::Map` — character map search
- **Search dispatcher**: `forward_search` with sub_anchor validation and low/high calculation
- **Backward search**: `backward_search` with position-by-position backward scan
- **Anchor optimization**: `onig_search` uses ANCR_BEGIN_BUF, ANCR_BEGIN_POSITION,
  ANCR_END_BUF, ANCR_SEMI_END_BUF, ANCR_ANYCHAR_INF_ML for position narrowing
- **~35 helper functions**: scoring (distance_value, comp_distance_value),
  string ops (concat_opt_exact, alt_merge_opt_exact, select_opt_exact),
  map ops (add_char_opt_map, select_opt_map), anchor ops (concat_opt_anc_info)
- **Data structures**: MinMaxLen, OptAnc, OptStr, OptMap, OptNode

### Safety Limits

- **Retry limit in match**: `onig_set/get_retry_limit_in_match` (default: 10,000,000)
- **Retry limit in search**: `onig_set/get_retry_limit_in_search`
- **Time limit**: `onig_set/get_time_limit` (millisecond timeout, checked every 512 ops)
- **Stack limit**: `onig_set/get_match_stack_limit_size`

### tune_tree Pipeline (complete)

- **tune_next + automatic possessification**: `is_exclusive` detects mutually
  exclusive nodes, `a*b` -> `(?>a*)b` when a and b are disjoint
- **Head-exact extraction**: `get_tree_head_literal` -> PushOrJumpExact1, PushIfPeekNext,
  AnyCharStarPeekNext
- **6x6 quantifier reduction**: Full ReduceTypeTable (?, *, +, ??, *?, +?)
  — e.g. `(?:a+)?` -> `a*`, `(?:a*)*` -> `a*`
- **String expansion**: `"abc"{3}` -> `"abcabcabc"` (up to 100 bytes)
- **Lookbehind reduction**: `node_reduce_in_look_behind` sets upper=lower
- **Call-node tuning**: 3-pass (tune_call -> tune_call2 -> tune_called_state)
  — entry counting, IN_MULTI_ENTRY/IN_REAL_REPEAT propagation
- **Empty-loop detection**: `qn.emptiness = MayBeEmpty` for `(?:x?)*`
- **backtrack_mem / backrefed_mem**: captures in Alt/Repeat -> MemStartPush/MemEndPush

### Other Complete Features

- **Syntax definitions**: ASIS, PosixBasic, PosixExtended, Emacs, Grep, GnuRegex, Java, Perl, Perl_NG, Python, Oniguruma, Ruby
- **All ~100 error codes** with parameterized messages
- **Unicode**: 629 code range tables, 886 property names, grapheme cluster, case folding
- **Recursion**: `\g<n>`, `\k<n+level>`, infinite recursion detection, MEM_END_REC
- **Variable-length lookbehind**: STEP_BACK_START/NEXT, Alt with zid
- **Absent functions**: repeater, expression, range cutter
- **Built-in callouts**: *MAX, *COUNT, *CMP (internal, not as public API)
- **FIND_LONGEST** / **FIND_NOT_EMPTY** options
- **CAPTURE_ONLY_NAMED_GROUP** (disable unnamed captures)

---

## 2. What is MISSING — by Priority

### Tier 1: API & Integration

#### A. Public API (~80 missing functions)

**OnigMatchParam (completely missing):**
- `onig_new/free/initialize_match_param`
- `onig_set_match_stack_limit_size_of_match_param`
- `onig_set_retry_limit_in_match/search_of_match_param`
- `onig_set_time_limit_of_match_param`
- `onig_set_progress/retraction_callout_of_match_param`
- `onig_match_with_param` / `onig_search_with_param`

**RegSet (completely missing):**
- `onig_regset_new/add/replace/free`
- `onig_regset_number_of_regex/get_regex/get_region`
- `onig_regset_search/search_with_param`

**Name/Capture queries:**
- `onig_name_to_group_numbers` / `onig_name_to_backref_number`
- `onig_foreach_name` / `onig_number_of_names`
- `onig_number_of_captures` / `onig_number_of_capture_histories`
- `onig_noname_group_capture_is_active`

**Regex accessors:**
- `onig_get_encoding/options/case_fold_flag/syntax`

**Other:**
- `onig_scan` (callback-based scanning)
- `onig_new_deluxe` / `onig_new_without_alloc`
- `onig_init` / `onig_end` (library initialization)
- `onig_version` / `onig_copyright`
- `onig_set_warn_func` / `onig_set_verb_warn_func`

#### B. Encodings (2 of 29 implemented)

```
Implemented:  ASCII, UTF-8
Missing:      UTF-16 BE/LE, UTF-32 BE/LE,
              ISO-8859-1..16,
              EUC-JP, EUC-TW, EUC-KR, EUC-CN,
              SJIS, KOI8, KOI8-R, CP1251,
              BIG5, GB18030
```

#### C. regtrav.c (Capture Tree Traversal)

Completely missing module:
- `onig_get_capture_tree` / `onig_capture_tree_traverse`
- Required for `(?@...)` capture history

#### D. Callout External API (~50 functions)

Internal callout logic works (*MAX, *COUNT, *CMP), but the public API
for user-defined callouts is completely missing:
- `onig_set_callout_of_name` / `onig_get_callout_data*`
- `onig_get_*_by_callout_args` (~15 accessor functions)
- `onig_set/get_progress/retraction_callout`
- `onig_builtin_fail/mismatch/error/skip` (only max/count/cmp internally)

---

### Tier 2: Remaining Optimizations

#### E. Case-Fold in Lookbehind

- `unravel_cf_look_behind_add` — filters multi-char folds in lookbehind
- `clear_not_flag_cclass` — negated CClass + case-fold expansion
- Only edge cases with exotic lookbehind patterns affected

#### F. check_node_in_look_behind (comprehensive)

C checks comprehensively (allowed types, Bag types, anchor types, Gimmick/Call/Recursion).
Rust only checks `ABSENT_WITH_SIDE_EFFECTS`. Affects exotic lookbehind patterns.

#### G. Direct-Threaded Code

C uses computed goto for ~20% faster opcode dispatch.
Rust uses standard `match`. Not portable (compiler-specific).

---

## 3. Subtle Differences in Existing Code

| Area | C | Rust | Impact |
|------|---|------|--------|
| Lookbehind case-fold | `unravel_cf_look_behind_add` | Missing | Edge cases with case-insensitive lookbehind |
| `check_node_in_look_behind` | Comprehensive (type masks, Bag/Anchor/Gimmick/Call) | Only `ABSENT_WITH_SIDE_EFFECTS` | Exotic lookbehind patterns |
| Negated CClass + case-fold | `clear_not_flag_cclass` | Missing | `[^a-z]/i` may not expand correctly |
| Direct-threaded code | Computed goto optimization | Standard `match` dispatch | ~20% interpreter overhead |
| POSIX API | `regposix.c` / `reggnu.c` | Not ported | Intentionally not ported |

---

## 4. Recommended Next Steps (by Impact)

1. **Public API functions** — OnigMatchParam, Name/Capture queries, accessors (Tier 1A)
2. **Case-fold in lookbehind** — unravel_cf_look_behind_add (Tier 2E)
3. **Comprehensive check_node_in_look_behind** (Tier 2F)
4. **Additional encodings** (UTF-16, Latin1 etc.) as needed (Tier 1B)
5. **regtrav.c** — Capture Tree Traversal (Tier 1C)
6. **Callout External API** — user-defined callouts (Tier 1D)

---

## 5. Bugs Found & Fixed (During Porting)

1. JUMP/PUSH address formulas
2. u_offset patching
3. usize subtraction overflow
4. Parser operates on &[u8], not &str
5. Multi-byte codepoint truncation (U+305B low byte = '[')
6. C trigraph escaping in tests (\? = ?)
7. Empty-loop detection (tune_tree sets qn.emptiness)
8. String-split for quantifiers (check_quantifier)
9. compile_length_quantifier_node off-by-body_len
10. Alternation JUMP addresses (>2 branches)
11. Missing ?? and greedy expansion paths
12. EmptyCheckEnd skip-next
13. backtrack_mem / backrefed_mem
14. EmptyCheckEndMemst (memory-aware empty check)
15. Reversed intervals ({5,2} swap)
16. FIXED_INTERVAL_IS_GREEDY_ONLY
17. StrN payload mismatch
18. RepeatIncNg stack ordering
19. \pL single-char property tokenizer
20. check_code_point_sequence must not advance p
21. CUT_TO_MARK must void, not pop
22. SaveVal ID collision
23. reduce_string_list loses ND_ST_SUPER
24. EmptyCheckEnd mem-ID mismatch with nested quantifiers
25. Call-node body vs target_node in optimizer
