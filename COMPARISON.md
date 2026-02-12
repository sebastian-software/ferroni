# C Oniguruma vs. Rust Port — Vollstaendiger Vergleich

> Stand: 2026-02-12 | 1468/1468 Tests bestanden, 0 ignoriert

---

## Gesamtbild

| Modul | C Zeilen | Rust Zeilen | Kern-Logik | API-Vollstaendigkeit |
|-------|----------|-------------|------------|----------------------|
| **Parser** (regparse.c) | 9.493 | 6.427 | ~95% | Kern komplett |
| **Compiler** (regcomp.c) | 8.589 | 5.587 | ~80% | Kompilierung + Optimierung komplett |
| **Executor** (regexec.c) | 7.002 | 3.731 | ~85% | Alle 70+ Opcodes, Vorwaerts-Suchoptimierung |
| **Types** (regint.h+regparse.h) | ~1.564 | ~2.116 | ~95% | Alle Kern-Typen inkl. OptNode/OptStr/OptMap |
| **Syntax** (regsyntax.c) | 373 | 367 | 100% | Komplett |
| **Fehlerbehandlung** (regerror.c) | 416 | 206 | 100% | Komplett (Rust braucht weniger Boilerplate) |
| **Encoding** (regenc.c+h) | ~1.284 | ~916 | ~70% | ASCII + UTF-8 (2 von 29) |
| **Unicode** | ~50.000 | ~50.000 | 100% | Komplett |

---

## 1. Was KOMPLETT ist

### Alle 70+ Opcodes im VM-Executor

STR_1..STR_N, STR_MB2N1..STR_MBN, CCLASS/CCLASS_MB/CCLASS_MIX (+ NOT-Varianten),
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

### Alle Escape-Sequenzen

`\a \b \t \n \v \f \r \e` (Steuerzeichen),
`\x{HH}` `\x{HHHH}` (Hex), `\o{OOO}` (Oktal), `\uHHHH` (Unicode),
`\w \W \d \D \s \S \h \H` (Zeichenklassen),
`\p{...} \P{...}` (Unicode-Properties),
`\k<name> \g<name>` (Backrefs/Calls),
`\A \z \Z \b \B \G` (Anker), `\K` (Keep),
`\R` (General Newline), `\N` (No-Newline), `\O` (True Anychar),
`\X` (Grapheme Cluster), `\y \Y` (Text Segment Boundaries)

### Alle Gruppen-Typen

`(?:...)` Non-Capturing, `(?=...)` Lookahead, `(?!...)` Neg. Lookahead,
`(?<=...)` Lookbehind, `(?<!...)` Neg. Lookbehind, `(?>...)` Atomic,
`(?<name>...)` Named Group, `(?'name'...)` Alternate Named,
`(?(cond)T|F)` Conditional, `(?~...)` Absent (3 Formen),
`(?{...})` Code Callout, `(*FAIL)` `(*MAX{n})` `(*COUNT[tag]{X})` `(*CMP{t1,op,t2})`,
`(?imxWDSPCL-imx:...)` Option Groups

### Optimierungs-Subsystem (NEU)

Vollstaendige Vorwaerts-Suchoptimierung wie im C-Original:
- **AST-Analyse**: `optimize_nodes` extrahiert OptStr/OptMap/OptAnc pro Knoten
- **Strategie-Auswahl**: `set_optimize_info_from_tree` waehlt beste Suchstrategie
  - `OptimizeType::StrFast` — Boyer-Moore-Horspool mit Skip-Tabelle
  - `OptimizeType::StrFastStepForward` — Multi-Byte-sichere BMH-Variante
  - `OptimizeType::Str` — Naive String-Suche
  - `OptimizeType::Map` — Zeichen-Map-Suche
- **Such-Dispatcher**: `forward_search` mit sub_anchor-Validierung und low/high-Berechnung
- **Anker-Optimierung**: `onig_search` nutzt ANCR_BEGIN_BUF, ANCR_BEGIN_POSITION,
  ANCR_END_BUF, ANCR_SEMI_END_BUF, ANCR_ANYCHAR_INF_ML fuer Positions-Eingrenzung
- **~35 Hilfsfunktionen**: Scoring (distance_value, comp_distance_value),
  String-Ops (concat_opt_exact, alt_merge_opt_exact, select_opt_exact),
  Map-Ops (add_char_opt_map, select_opt_map), Anker-Ops (concat_opt_anc_info)
- **Datenstrukturen**: MinMaxLen, OptAnc, OptStr, OptMap, OptNode

### Weitere vollstaendige Features

- **Syntax-Definitionen**: ASIS, PosixBasic, PosixExtended, Emacs, Grep, GnuRegex, Java, Perl, Perl_NG, Python, Oniguruma, Ruby
- **Alle ~100 Fehlercodes** mit parametrisierten Meldungen
- **Unicode**: 629 Code-Range-Tabellen, 886 Property-Namen, Grapheme Cluster, Case Folding
- **Rekursion**: `\g<n>`, `\k<n+level>`, Endlosrekursions-Erkennung, MEM_END_REC
- **Variable-length Lookbehind**: STEP_BACK_START/NEXT, Alt mit zid
- **Absent Functions**: Repeater, Expression, Range Cutter
- **Built-in Callouts**: *MAX, *COUNT, *CMP (intern, nicht als oeffentliche API)
- **FIND_LONGEST** / **FIND_NOT_EMPTY** Optionen
- **CAPTURE_ONLY_NAMED_GROUP** (noname capture deaktivieren)

---

## 2. Was FEHLT — nach Prioritaet

### Tier 1: Performance-kritisch

#### A. Retry/Time/Stack-Limits (~15 Funktionen)

**Fehlende Funktionen:**
- `onig_get/set_retry_limit_in_match` (C-Default: 10.000.000)
- `onig_get/set_retry_limit_in_search`
- `onig_get/set_time_limit` (Millisekunden-Timeout)
- `onig_get/set_match_stack_limit_size`
- `CHECK_RETRY_LIMIT_IN_MATCH` Makro (prueft jeden Opcode)
- `CHECK_TIME_LIMIT_IN_MATCH` Makro (prueft alle 512 Ops)
- `set_limit_end_time` / `time_is_running_out`

**Impact:** Kein Schutz gegen katastrophales Backtracking. Patterns koennen endlos haengen.

#### B. Backward Search (~4 Funktionen)

**Fehlende Funktionen:**
- `backward_search` / `backward_search_range` — Optimierter Rueckwaerts-Dispatcher
- `slow_search_backward` / `slow_search_backward_ic` — Naive String-Suche (rueckwaerts)
- `map_search_backward` — Zeichenmap-Suche (rueckwaerts)

**Impact:** Patterns mit End-Anker (`$`, `\z`) koennen nicht rueckwaerts suchen.
Forward-Search ist implementiert und reicht fuer die meisten Faelle.

---

### Tier 2: Fehlende tune_tree-Passes

#### C. tune_next / Automatische Possessivierung (~190 Zeilen C)

C's `tune_next` + `is_exclusive` erkennt gegenseitig ausschliessende aufeinanderfolgende
Knoten und wandelt `a*b` automatisch in `(?>a*)b` um (Possessivierung).
Zusaetzlich setzt es `qn.next_head_exact` fuer ANYCHAR_STAR_PEEK_NEXT.

**Fehlende Funktionen:**
- `tune_next` — Nachbar-Knoten-Analyse und Possessivierung
- `is_exclusive` — Gegenseitige Ausschluss-Pruefung
- `get_tree_head_literal` — Fuehrendes Literal extrahieren

**Impact:** Korrektheit nicht betroffen. Performance-Einbusse bei Patterns wie `.*foo`
(kein PEEK_NEXT) und `[a-z]*[0-9]` (keine automatische Possessivierung).

#### D. String-Expansion in Quantoren

C expandiert `"abc"{3}` zu `"abcabcabc"` (bis 100 Zeichen).
Benutzt `node_conv_to_str_node` und `node_str_node_cat`.

**Impact:** Niedrig. Quantoren funktionieren korrekt, nur minimaler Overhead.

#### E. Head-Exact-Extraktion

C setzt `qn.head_exact` fuer greedy Quantoren: `(abc)+` -> head_exact="abc".
Wird von `get_tree_head_literal` berechnet. Ermoeglicht schnelle String-Suche.

**Impact:** Niedrig. forward_search kompensiert dies weitgehend.

#### F. Vollstaendige Nested-Quantifier-Reduktion

C hat eine 6x6 `ReduceTypeTable`:
```
        ?    *    +    ??   *?   +?
  ?   | ??   ??   DEL  ??   ??   DEL
  *   | *    *    *    *    *    *
  +   | DEL  *    DEL  P_QQ *    DEL
  ??  | ??   AQ   P_QQ ??   AQ   P_QQ
  *?  | *?   *?   *?   *?   *?   *?
  +?  | QQ   AQ   DEL  QQ   AQ   DEL
```
Rust implementiert nur `{n}{m}` -> `{n*m}` (feste Multiplikation).

**Impact:** Niedrig. Korrektheit nicht betroffen, nur minimale Optimierung fehlt.

#### G. Lookbehind-Spezialbehandlungen

- `node_reduce_in_look_behind` / `list_reduce_in_look_behind` / `alt_reduce_in_look_behind`
  — C optimiert Knoten innerhalb von Lookbehind (entfernt unnoetigen Overhead)
- `unravel_cf_look_behind_add` — Case-Fold in Lookbehind
- `check_node_in_look_behind` — C prueft umfassend (erlaubte Typen, Bag-Typen, Anker-Typen,
  Gimmick/Call/Recursion), Rust prueft nur `ABSENT_WITH_SIDE_EFFECTS`

**Impact:** Niedrig. Edge Cases bei exotischen Lookbehind-Patterns.

#### H. Call-Node-Tuning (Multi-Pass)

C macht 3 separate Passes: `tune_call` -> `tune_call2` -> `tune_called_state`.
Rust macht 1 Pass (`resolve_call_references`).

**Impact:** Niedrig. Alle Rekursion/Call-Tests bestehen.

---

### Tier 3: API & Module

#### I. Oeffentliche API (~80 fehlende Funktionen)

**OnigMatchParam (komplett fehlend):**
- `onig_new/free/initialize_match_param`
- `onig_set_match_stack_limit_size_of_match_param`
- `onig_set_retry_limit_in_match/search_of_match_param`
- `onig_set_time_limit_of_match_param`
- `onig_set_progress/retraction_callout_of_match_param`
- `onig_match_with_param` / `onig_search_with_param`

**RegSet (komplett fehlend):**
- `onig_regset_new/add/replace/free`
- `onig_regset_number_of_regex/get_regex/get_region`
- `onig_regset_search/search_with_param`

**Name/Capture-Queries:**
- `onig_name_to_group_numbers` / `onig_name_to_backref_number`
- `onig_foreach_name` / `onig_number_of_names`
- `onig_number_of_captures` / `onig_number_of_capture_histories`
- `onig_noname_group_capture_is_active`

**Regex-Accessors:**
- `onig_get_encoding/options/case_fold_flag/syntax`

**Sonstige:**
- `onig_scan` (Callback-basiertes Scanning)
- `onig_new_deluxe` / `onig_new_without_alloc`
- `onig_init` / `onig_end` (Bibliotheks-Initialisierung)
- `onig_version` / `onig_copyright`
- `onig_set_warn_func` / `onig_set_verb_warn_func`

#### J. Encodings (2 von 29 implementiert)

```
Implementiert:  ASCII, UTF-8
Fehlend:        UTF-16 BE/LE, UTF-32 BE/LE,
                ISO-8859-1..16,
                EUC-JP, EUC-TW, EUC-KR, EUC-CN,
                SJIS, KOI8, KOI8-R, CP1251,
                BIG5, GB18030
```

#### K. regtrav.c (Capture Tree Traversal)

Komplett fehlendes Modul:
- `onig_get_capture_tree` / `onig_capture_tree_traverse`
- Wird fuer `(?@...)` Capture History benoetigt

#### L. Callout External API (~50 Funktionen)

Interne Callout-Logik funktioniert (*MAX, *COUNT, *CMP), aber die oeffentliche API
fuer benutzerdefinierte Callouts fehlt komplett:
- `onig_set_callout_of_name` / `onig_get_callout_data*`
- `onig_get_*_by_callout_args` (~15 Accessor-Funktionen)
- `onig_set/get_progress/retraction_callout`
- `onig_builtin_fail/mismatch/error/skip` (nur max/count/cmp intern)

---

## 3. Subtile Unterschiede in existierendem Code

| Bereich | C | Rust | Impact |
|---------|---|------|--------|
| Quantifier-Reduktion | 6x6 ReduceTypeTable | Nur fixed x fixed | Niedrig (korrekt, aber nicht optimal) |
| tune_next / Possessivierung | `a*b` -> `(?>a*)b` automatisch | Fehlt | Mittel (Performance bei `.*foo` Patterns) |
| Lookbehind Case-Fold | `unravel_cf_look_behind_add` | Fehlt | Edge Cases bei case-insensitivem Lookbehind |
| `check_node_in_look_behind` | Umfassend (Typen-Masken, Bag/Anchor/Gimmick/Call) | Nur `ABSENT_WITH_SIDE_EFFECTS` | Exotische Lookbehind-Patterns |
| Negated CClass + Case-Fold | `clear_not_flag_cclass` | Fehlt | `[^a-z]/i` koennte nicht korrekt expandieren |
| Direct-Threaded Code | Computed goto Optimierung | Standard `match` Dispatch | ~20% Interpreter-Overhead |
| Backward Search | `backward_search` + `slow_search_backward` | Nicht implementiert | End-Anker-Patterns |
| POSIX API | `regposix.c` / `reggnu.c` | Nicht portiert | Intentionell nicht portiert |
| String-Node-Ops | `str_node_split_last_char` etc. | Inline in `check_quantifier` | Funktioniert (Bug #8 behoben) |

---

## 4. Empfohlene naechste Schritte (nach Impact)

1. **Retry/Time-Limits** — Sicherheit gegen Endlosschleifen (Tier 1A)
2. **tune_next + is_exclusive** — Automatische Possessivierung, PEEK_NEXT (Tier 2C)
3. **Vollstaendige ReduceTypeTable** — Bessere Quantifier-Optimierung (Tier 2F)
4. **Fehlende tune_tree-Passes** (head_exact, string expansion) (Tier 2D/E)
5. **Backward Search** — Fuer End-Anker-Patterns (Tier 1B)
6. **Oeffentliche API-Funktionen** nach Bedarf (Tier 3)
7. **Weitere Encodings** (UTF-16, Latin1 etc.) nach Bedarf (Tier 3)

---

## 5. Gefundene & behobene Bugs (waehrend der Portierung)

1. JUMP/PUSH Adressformeln
2. u_offset Patching
3. usize Subtraktions-Overflow
4. Parser arbeitet auf &[u8], nicht &str
5. Multi-Byte-Codepoint-Trunkierung (U+305B Low-Byte = '[')
6. C Trigraph-Escaping in Tests (\? = ?)
7. Empty-Loop-Erkennung (tune_tree setzt qn.emptiness)
8. String-Split fuer Quantoren (check_quantifier)
9. compile_length_quantifier_node off-by-body_len
10. Alternation JUMP-Adressen (>2 Branches)
11. Fehlende ?? und greedy Expansion Pfade
12. EmptyCheckEnd skip-next
13. backtrack_mem / backrefed_mem
14. EmptyCheckEndMemst (memory-aware empty check)
15. Reversed Intervals ({5,2} swap)
16. FIXED_INTERVAL_IS_GREEDY_ONLY
17. StrN Payload Mismatch
18. RepeatIncNg Stack-Reihenfolge
19. \pL Single-Char Property Tokenizer
20. check_code_point_sequence darf p nicht vorruecken
21. CUT_TO_MARK muss void, nicht pop
22. SaveVal ID-Kollision
23. reduce_string_list verliert ND_ST_SUPER
24. EmptyCheckEnd mem-ID Mismatch bei verschachtelten Quantoren
25. **Call-Node body vs target_node im Optimizer** — `optimize_nodes` und `node_max_byte_len` benutzten `cn.body` (immer None bei Call-Nodes) statt `cn.target_node` (Raw-Pointer zum Ziel-Knoten). Verursachte falsche dist_min/dist_max-Werte bei Subroutine-Calls wie `\g<+2>(abc)(ABC){0}`.
