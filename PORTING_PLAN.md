# Oniguruma -> Rust: Detaillierter 1:1-Portierungsplan

> Arbeitsdokument fuer die systematische Portierung von Oniguruma (C) nach Rust.
> Ziel: Maximale Naehe zum Original in Architektur, Strukturen, Methodennamen und Parametern.

---

## Inhaltsverzeichnis

1. [Ueberblick: C-Original](#1-ueberblick-c-original)
2. [Rust-Projektstruktur](#2-rust-projektstruktur)
3. [Phase 1: Grundtypen & Konstanten](#3-phase-1-grundtypen--konstanten)
4. [Phase 2: Encoding-System](#4-phase-2-encoding-system)
5. [Phase 3: Parser (regparse)](#5-phase-3-parser-regparse)
6. [Phase 4: Compiler (regcomp)](#6-phase-4-compiler-regcomp)
7. [Phase 5: Execution Engine (regexec)](#7-phase-5-execution-engine-regexec)
8. [Phase 6: Nebenmodule](#8-phase-6-nebenmodule)
9. [Phase 7: RegSet & Callouts](#9-phase-7-regset--callouts)
10. [C-zu-Rust Uebersetzungsregeln](#10-c-zu-rust-uebersetzungsregeln)
11. [Datei-fuer-Datei Mapping](#11-datei-fuer-datei-mapping)
12. [Testplan](#12-testplan)

---

## 1. Ueberblick: C-Original

### Dateistatistik

| Datei | Zeilen | Rolle |
|-------|--------|-------|
| **regparse.c** | 9.493 | Parser: Pattern-Bytes -> AST |
| **regcomp.c** | 8.589 | Compiler: AST -> Bytecode (Operation[]) |
| **regexec.c** | 7.002 | Execution: Bytecode + String -> Matches |
| **regenc.c** | ~1.200 | Encoding-Utilities |
| **unicode.c** | ~2.500 | Unicode-Properties, Case-Folding |
| **regsyntax.c** | ~500 | Syntax-Definitionen (Oniguruma, Perl, POSIX, ...) |
| **regerror.c** | ~300 | Fehlermeldungen |
| **regext.c** | ~200 | Callout-Extension |
| **regtrav.c** | ~80 | Capture-History-Traversal |
| **regversion.c** | ~30 | Version |
| **st.c** | ~500 | Hash-Table (Named Groups) |
| **onig_init.c** | ~50 | Initialisierung |
| **regposix.c** | ~300 | POSIX-API-Wrapper |
| **regposerr.c** | ~100 | POSIX-Fehlertexte |
| **reggnu.c** | ~100 | GNU-Kompatibilitaet |
| **Encodings (30 Dateien)** | ~40.000 | ascii.c, utf8.c, utf16_be/le.c, utf32_be/le.c, euc_jp.c, sjis.c, iso8859_*.c, ... |
| **Unicode-Daten (7 Dateien)** | ~15.000 | unicode_fold_data.c, unicode_property_data.c, unicode_*_key.c, ... |
| **Header (7 Dateien)** | ~3.300 | oniguruma.h, regint.h, regparse.h, regenc.h, st.h, oniggnu.h, onigposix.h |
| **Gesamt** | **~89.000** | |

### Kompilierungspipeline

```
Pattern (&[u8])
    |
    v
[regparse.c] fetch_token() -> prs_alts() -> prs_atom()
    |
    v
AST (Node-Baum: StrNode, CClassNode, QuantNode, BagNode, ...)
    |
    v
[regcomp.c] parse_and_tune() -> compile_tree()
    |
    v
Bytecode (Operation[] + string_pool)
    |
    v
[regexec.c] match_at() / search_in_range()
    |
    v
OnigRegion (Capture-Ergebnisse)
```

---

## 2. Rust-Projektstruktur

Die Rust-Struktur soll die C-Dateien 1:1 abbilden. Jede C-Datei wird zu einem
Rust-Modul. Die Header werden zu gemeinsamen Typ-Definitionen.

```
ferroni/
  Cargo.toml
  src/
    lib.rs                    # Oeffentliche API, re-exports

    # === Kerntypen (aus den Headern) ===
    oniguruma.rs              # <- oniguruma.h: Oeffentliche Typen, Optionen, Konstanten
    regint.rs                 # <- regint.h: Interne Typen, OpCode, Operation, regex_t
    regparse_types.rs         # <- regparse.h: Node, ParseEnv, AST-Typen

    # === Kern-Engine ===
    regparse.rs               # <- regparse.c: Parser (fetch_token, prs_alts, prs_atom, ...)
    regcomp.rs                # <- regcomp.c: Compiler (compile_tree, optimize, ...)
    regexec.rs                # <- regexec.c: Executor (match_at, search_in_range, ...)

    # === Encoding-System ===
    regenc.rs                 # <- regenc.c + regenc.h: Encoding-Trait + Utilities
    encodings/
      mod.rs                  # Encoding-Registry
      ascii.rs                # <- ascii.c
      utf8.rs                 # <- utf8.c
      utf16_be.rs             # <- utf16_be.c
      utf16_le.rs             # <- utf16_le.c
      utf32_be.rs             # <- utf32_be.c
      utf32_le.rs             # <- utf32_le.c
      euc_jp.rs               # <- euc_jp.c
      sjis.rs                 # <- sjis.c
      iso8859_1.rs            # <- iso8859_1.c
      iso8859_2.rs            # <- iso8859_2.c
      ... (alle 30 Encodings)
      euc_kr.rs               # <- euc_kr.c
      euc_tw.rs               # <- euc_tw.c
      gb18030.rs              # <- gb18030.c
      big5.rs                 # <- big5.c
      cp1251.rs               # <- cp1251.c
      koi8.rs                 # <- koi8.c
      koi8_r.rs               # <- koi8_r.c

    # === Unicode-Daten ===
    unicode/
      mod.rs                  # <- unicode.c: case_fold, is_code_ctype, property lookup
      fold_data.rs            # <- unicode_fold_data.c: OnigUnicodeFolds1/2/3
      property_data.rs        # <- unicode_property_data.c: Generierte Property-Tabellen
      property_data_posix.rs  # <- unicode_property_data_posix.c
      fold1_key.rs            # <- unicode_fold1_key.c (gperf Hash)
      fold2_key.rs            # <- unicode_fold2_key.c
      fold3_key.rs            # <- unicode_fold3_key.c
      unfold_key.rs           # <- unicode_unfold_key.c
      egcb_data.rs            # <- unicode_egcb_data.c
      wb_data.rs              # <- unicode_wb_data.c

    # === Nebenmodule ===
    regsyntax.rs              # <- regsyntax.c: OnigSyntaxOniguruma, POSIX, Perl, ...
    regerror.rs               # <- regerror.c: Fehlermeldungen
    regext.rs                 # <- regext.c: Callout-Extension
    regtrav.rs                # <- regtrav.c: Capture-History
    st.rs                     # <- st.c/st.h: Hash-Table (oder std::HashMap verwenden)
    regposix.rs               # <- regposix.c: POSIX-API
    regversion.rs             # <- regversion.c: Version

  tests/
    compat_utf8.rs            # <- test/test_utf8.c
    compat_back.rs            # <- test/test_back.c
    compat_syntax.rs          # <- test/test_syntax.c
    compat_options.rs         # <- test/test_options.c
    compat_testc.rs           # <- test/testc.c (EUC-JP)
    compat_testu.rs           # <- test/testu.c (UTF-16)
    compat_testp.rs           # <- test/testp.c (POSIX)
    compat_regset.rs          # <- test/test_regset.c
```

---

## 3. Phase 1: Grundtypen & Konstanten

### 3.1 oniguruma.rs (aus oniguruma.h)

Diese Datei definiert alle oeffentlichen Typen, exakt wie im C-Header.

```rust
// === Grundtypen ===
pub type OnigCodePoint = u32;
pub type OnigUChar = u8;
pub type OnigCtype = u32;
pub type OnigLen = u32;
pub type OnigCaseFoldType = u32;
pub type OnigOptionType = u32;

// === Konstanten ===
pub const ONIG_INFINITE_DISTANCE: OnigLen = OnigLen::MAX;
pub const ONIG_NREGION: usize = 10;
pub const ONIG_MAX_CAPTURE_NUM: i32 = 2147483647;
pub const ONIG_MAX_BACKREF_NUM: i32 = 1000;
pub const ONIG_MAX_REPEAT_NUM: i32 = 100000;
pub const ONIG_MAX_MULTI_BYTE_RANGES_NUM: i32 = 10000;
pub const ONIG_MAX_ERROR_MESSAGE_LEN: usize = 90;
pub const ONIGENC_MAX_COMP_CASE_FOLD_CODE_LEN: usize = 3;
pub const ONIGENC_GET_CASE_FOLD_CODES_MAX_NUM: usize = 13;
pub const ONIGENC_CODE_TO_MBC_MAXLEN: usize = 7;
pub const ONIGENC_MBC_CASE_FOLD_MAXLEN: usize = 18;
pub const ONIG_MAX_CAPTURE_HISTORY_GROUP: usize = 31;
pub const ONIG_CALLOUT_MAX_ARGS_NUM: usize = 4;
pub const ONIG_CALLOUT_DATA_SLOT_NUM: usize = 5;
pub const ONIG_REGION_NOTPOS: i32 = -1;

// === Option Flags (Compile Time) ===
pub const ONIG_OPTION_NONE: OnigOptionType = 0;
pub const ONIG_OPTION_IGNORECASE: OnigOptionType = 1;
pub const ONIG_OPTION_EXTEND: OnigOptionType = 1 << 1;
pub const ONIG_OPTION_MULTILINE: OnigOptionType = 1 << 2;
pub const ONIG_OPTION_SINGLELINE: OnigOptionType = 1 << 3;
pub const ONIG_OPTION_FIND_LONGEST: OnigOptionType = 1 << 4;
pub const ONIG_OPTION_FIND_NOT_EMPTY: OnigOptionType = 1 << 5;
pub const ONIG_OPTION_NEGATE_SINGLELINE: OnigOptionType = 1 << 6;
pub const ONIG_OPTION_DONT_CAPTURE_GROUP: OnigOptionType = 1 << 7;
pub const ONIG_OPTION_CAPTURE_GROUP: OnigOptionType = 1 << 8;
// ... alle weiteren aus oniguruma.h

// === Option Flags (Search Time) ===
pub const ONIG_OPTION_NOTBOL: OnigOptionType = 1 << 9;
pub const ONIG_OPTION_NOTEOL: OnigOptionType = 1 << 10;
pub const ONIG_OPTION_NOT_BEGIN_STRING: OnigOptionType = 1 << 18;
pub const ONIG_OPTION_NOT_END_STRING: OnigOptionType = 1 << 19;
pub const ONIG_OPTION_NOT_BEGIN_POSITION: OnigOptionType = 1 << 20;
pub const ONIG_OPTION_MATCH_WHOLE_STRING: OnigOptionType = 1 << 22;
// ... etc.

// === Error Codes ===
pub const ONIG_NORMAL: i32 = 0;
pub const ONIG_MISMATCH: i32 = -1;
pub const ONIG_NO_SUPPORT_CONFIG: i32 = -2;
pub const ONIG_ABORT: i32 = -3;
pub const ONIGERR_MEMORY: i32 = -5;
pub const ONIGERR_TYPE_BUG: i32 = -6;
pub const ONIGERR_PARSER_BUG: i32 = -11;
pub const ONIGERR_STACK_BUG: i32 = -12;
pub const ONIGERR_MATCH_STACK_LIMIT_OVER: i32 = -15;
pub const ONIGERR_PARSE_DEPTH_LIMIT_OVER: i32 = -16;
pub const ONIGERR_RETRY_LIMIT_IN_MATCH_OVER: i32 = -17;
// ... alle Fehlercodes aus oniguruma.h (ca. 60 Stueck)

// === Character Types ===
#[repr(u32)]
pub enum OnigEncCtype {
    Newline = 0,
    Alpha = 1,
    Blank = 2,
    Cntrl = 3,
    Digit = 4,
    Graph = 5,
    Lower = 6,
    Print = 7,
    Punct = 8,
    Space = 9,
    Upper = 10,
    Xdigit = 11,
    Word = 12,
    Alnum = 13,
    Ascii = 14,
}

// === Case Fold Code Item (exakt wie C) ===
#[derive(Clone, Debug)]
pub struct OnigCaseFoldCodeItem {
    pub byte_len: i32,     // Original-Zeichen Byte-Laenge
    pub code_len: i32,     // Anzahl Code-Points im Ergebnis
    pub code: [OnigCodePoint; ONIGENC_MAX_COMP_CASE_FOLD_CODE_LEN],
}

// === OnigRegion (exakt wie re_registers in C) ===
pub struct OnigRegion {
    pub allocated: i32,
    pub num_regs: i32,
    pub beg: Vec<i32>,     // C: int* beg
    pub end: Vec<i32>,     // C: int* end
    pub history_root: Option<Box<OnigCaptureTreeNode>>,
}

// === Capture Tree Node ===
pub struct OnigCaptureTreeNode {
    pub group: i32,
    pub beg: i32,
    pub end: i32,
    pub childs: Vec<Box<OnigCaptureTreeNode>>,
}

// === Repeat Range ===
#[derive(Clone, Debug)]
pub struct OnigRepeatRange {
    pub lower: i32,
    pub upper: i32,
}

// === Syntax Type ===
pub struct OnigSyntaxType {
    pub op: u32,           // Syntax-Operatoren Bitfeld 1
    pub op2: u32,          // Syntax-Operatoren Bitfeld 2
    pub behavior: u32,     // Syntax-Verhalten Bitfeld
    pub options: OnigOptionType,
    pub meta_char_table: OnigMetaCharTableType,
}

// === Meta Char Table ===
pub struct OnigMetaCharTableType {
    pub esc: OnigCodePoint,
    pub anychar: OnigCodePoint,
    pub anytime: OnigCodePoint,
    pub zero_or_one_time: OnigCodePoint,
    pub one_or_more_time: OnigCodePoint,
    pub anychar_anytime: OnigCodePoint,
}

// === Error Info ===
pub struct OnigErrorInfo {
    pub enc: OnigEncoding,
    pub par: *const OnigUChar,   // In Rust: &[u8] Referenz
    pub par_end: *const OnigUChar,
}

// === Syntax Operator Flags (op) - ALLE aus oniguruma.h ===
pub const ONIG_SYN_OP_VARIABLE_META_CHARACTERS: u32 = 1 << 0;
pub const ONIG_SYN_OP_DOT_ANYCHAR: u32 = 1 << 1;
pub const ONIG_SYN_OP_ASTERISK_ZERO_INF: u32 = 1 << 2;
pub const ONIG_SYN_OP_ESC_ASTERISK_ZERO_INF: u32 = 1 << 3;
pub const ONIG_SYN_OP_PLUS_ONE_INF: u32 = 1 << 4;
pub const ONIG_SYN_OP_ESC_PLUS_ONE_INF: u32 = 1 << 5;
pub const ONIG_SYN_OP_QMARK_ZERO_ONE: u32 = 1 << 6;
pub const ONIG_SYN_OP_ESC_QMARK_ZERO_ONE: u32 = 1 << 7;
pub const ONIG_SYN_OP_BRACE_INTERVAL: u32 = 1 << 8;
pub const ONIG_SYN_OP_ESC_BRACE_INTERVAL: u32 = 1 << 9;
pub const ONIG_SYN_OP_VBAR_ALT: u32 = 1 << 10;
pub const ONIG_SYN_OP_ESC_VBAR_ALT: u32 = 1 << 11;
pub const ONIG_SYN_OP_LPAREN_SUBEXP: u32 = 1 << 12;
pub const ONIG_SYN_OP_ESC_LPAREN_SUBEXP: u32 = 1 << 13;
// ... alle 32 Bits aus oniguruma.h

// === Syntax Operator Flags (op2) - ALLE aus oniguruma.h ===
pub const ONIG_SYN_OP2_ESC_CAPITAL_Q_QUOTE: u32 = 1 << 0;
pub const ONIG_SYN_OP2_QMARK_GROUP_EFFECT: u32 = 1 << 1;
pub const ONIG_SYN_OP2_OPTION_PERL: u32 = 1 << 2;
pub const ONIG_SYN_OP2_OPTION_RUBY: u32 = 1 << 3;
// ... alle 32 Bits aus oniguruma.h

// === Syntax Behavior Flags ===
pub const ONIG_SYN_CONTEXT_INDEP_REPEAT_OPS: u32 = 1 << 0;
pub const ONIG_SYN_CONTEXT_INVALID_REPEAT_OPS: u32 = 1 << 1;
pub const ONIG_SYN_ALLOW_UNMATCHED_CLOSE_SUBEXP: u32 = 1 << 2;
// ... alle aus oniguruma.h
```

### 3.2 regint.rs (aus regint.h)

```rust
// === Konfigurationskonstanten ===
pub const DEFAULT_PARSE_DEPTH_LIMIT: u32 = 4096;
pub const INIT_MATCH_STACK_SIZE: usize = 160;
pub const DEFAULT_MATCH_STACK_LIMIT_SIZE: u32 = 0;
pub const DEFAULT_RETRY_LIMIT_IN_MATCH: u64 = 10_000_000;
pub const DEFAULT_RETRY_LIMIT_IN_SEARCH: u64 = 0;
pub const DEFAULT_SUBEXP_CALL_MAX_NEST_LEVEL: i32 = 20;

// === Bytecode-Typen ===
pub type RelAddrType = i32;
pub type AbsAddrType = i32;
pub type LengthType = i32;
pub type RelPositionType = i32;
pub type RepeatNumType = i32;
pub type MemNumType = i32;
pub type ModeType = i32;
pub type MemStatusType = u32;

// === Bitset (256 Bits fuer ASCII-Zeichenklassen) ===
pub const SINGLE_BYTE_SIZE: usize = 256;
pub const BITSET_SIZE: usize = SINGLE_BYTE_SIZE / 32;  // 8 u32s
pub type BitSet = [u32; BITSET_SIZE];

#[inline]
pub fn bitset_at(bs: &BitSet, pos: usize) -> bool {
    (bs[pos >> 5] & (1 << (pos & 0x1f))) != 0
}

#[inline]
pub fn bitset_set_bit(bs: &mut BitSet, pos: usize) {
    bs[pos >> 5] |= 1 << (pos & 0x1f);
}

#[inline]
pub fn bitset_clear_bit(bs: &mut BitSet, pos: usize) {
    bs[pos >> 5] &= !(1 << (pos & 0x1f));
}

// === MemStatus Bitset (32 Bits fuer Capture-Tracking) ===
pub const MEM_STATUS_BITS_NUM: usize = 32;

#[inline]
pub fn mem_status_at(stats: MemStatusType, n: usize) -> bool {
    if n < MEM_STATUS_BITS_NUM {
        (stats & (1 << n)) != 0
    } else {
        false
    }
}

// === Anchor Flags ===
pub const ANCR_PREC_READ: i32 = 1 << 0;
pub const ANCR_PREC_READ_NOT: i32 = 1 << 1;
pub const ANCR_LOOK_BEHIND: i32 = 1 << 2;
pub const ANCR_LOOK_BEHIND_NOT: i32 = 1 << 3;
pub const ANCR_BEGIN_BUF: i32 = 1 << 4;
pub const ANCR_BEGIN_LINE: i32 = 1 << 5;
pub const ANCR_BEGIN_POSITION: i32 = 1 << 6;
pub const ANCR_END_BUF: i32 = 1 << 7;
pub const ANCR_SEMI_END_BUF: i32 = 1 << 8;
pub const ANCR_END_LINE: i32 = 1 << 9;
pub const ANCR_WORD_BOUNDARY: i32 = 1 << 10;
pub const ANCR_NO_WORD_BOUNDARY: i32 = 1 << 11;
pub const ANCR_WORD_BEGIN: i32 = 1 << 12;
pub const ANCR_WORD_END: i32 = 1 << 13;
pub const ANCR_ANYCHAR_INF: i32 = 1 << 14;
pub const ANCR_ANYCHAR_INF_ML: i32 = 1 << 15;
pub const ANCR_TEXT_SEGMENT_BOUNDARY: i32 = 1 << 16;
pub const ANCR_NO_TEXT_SEGMENT_BOUNDARY: i32 = 1 << 17;

// === OpCode Enum (ALLE 93+ Opcodes, exakt wie C) ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    Finish = 0,
    End = 1,
    Str1 = 2,
    Str2 = 3,
    Str3 = 4,
    Str4 = 5,
    Str5 = 6,
    StrN = 7,
    StrMb2n1 = 8,
    StrMb2n2 = 9,
    StrMb2n3 = 10,
    StrMb2n = 11,
    StrMb3n = 12,
    StrMbn = 13,
    CClass = 14,
    CClassMb = 15,
    CClassMix = 16,
    CClassNot = 17,
    CClassMbNot = 18,
    CClassMixNot = 19,
    AnyChar = 20,
    AnyCharMl = 21,
    AnyCharStar = 22,
    AnyCharMlStar = 23,
    AnyCharStarPeekNext = 24,
    AnyCharMlStarPeekNext = 25,
    Word = 26,
    WordAscii = 27,
    NoWord = 28,
    NoWordAscii = 29,
    WordBoundary = 30,
    NoWordBoundary = 31,
    WordBegin = 32,
    WordEnd = 33,
    TextSegmentBoundary = 34,
    BeginBuf = 35,
    EndBuf = 36,
    BeginLine = 37,
    EndLine = 38,
    SemiEndBuf = 39,
    CheckPosition = 40,
    BackRef1 = 41,
    BackRef2 = 42,
    BackRefN = 43,
    BackRefNIc = 44,
    BackRefMulti = 45,
    BackRefMultiIc = 46,
    BackRefWithLevel = 47,
    BackRefWithLevelIc = 48,
    BackRefCheck = 49,
    BackRefCheckWithLevel = 50,
    MemStart = 51,
    MemStartPush = 52,
    MemEndPush = 53,
    MemEndPushRec = 54,
    MemEnd = 55,
    MemEndRec = 56,
    Fail = 57,
    Jump = 58,
    Push = 59,
    PushSuper = 60,
    Pop = 61,
    PopToMark = 62,
    PushOrJumpExact1 = 63,
    PushIfPeekNext = 64,
    Repeat = 65,
    RepeatNg = 66,
    RepeatInc = 67,
    RepeatIncNg = 68,
    EmptyCheckStart = 69,
    EmptyCheckEnd = 70,
    EmptyCheckEndMemst = 71,
    EmptyCheckEndMemstPush = 72,
    Move = 73,
    StepBackStart = 74,
    StepBackNext = 75,
    CutToMark = 76,
    Mark = 77,
    SaveVal = 78,
    UpdateVar = 79,
    Call = 80,
    Return = 81,
    CalloutContents = 82,
    CalloutName = 83,
}

// === Save Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SaveType {
    Keep = 0,
    S = 1,
    RightRange = 2,
}

// === Update Var Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum UpdateVarType {
    KeepFromStackLast = 0,
    SFromStack = 1,
    RightRangeFromStack = 2,
    RightRangeFromSStack = 3,
    RightRangeToS = 4,
    RightRangeInit = 5,
}

// === Check Position Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CheckPositionType {
    SearchStart = 0,
    CurrentRightRange = 1,
}

// === Text Segment Boundary Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TextSegmentBoundaryType {
    ExtendedGraphemeCluster = 0,
    Word = 1,
}

// === Optimize Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizeType {
    None,
    Str,              // Slow Search
    StrFast,          // Sunday quick search / BMH
    StrFastStepForward,
    Map,              // char map
}

// === Stack Pop Level ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackPopLevel {
    Free = 0,
    MemStart = 1,
    All = 2,
}

// === Operation (Bytecode-Instruktion) ===
// In Rust als enum statt C-Union, da Rust tagged unions idiomatischer sind.
// ABER: Fuer 1:1 Naehe zum C-Original verwenden wir eine aehnliche Struktur.
//
// Die C-Struktur ist ein `Operation` mit `opcode` + einem union fuer die Operanden.
// In Rust bilden wir das als struct mit opcode + enum Payload ab.

pub struct Operation {
    pub opcode: OpCode,
    pub payload: OperationPayload,
}

// Die Payload ist ein enum, das exakt die C-Union abbildet:
pub enum OperationPayload {
    None,
    Exact { s: [u8; 16] },
    ExactN { s: Vec<u8>, n: i32 },
    ExactLenN { s: Vec<u8>, n: i32, len: i32 },
    CClass { bsp: Box<BitSet> },
    CClassMb { mb: Vec<OnigCodeRange> },
    CClassMix { mb: Vec<OnigCodeRange>, bsp: Box<BitSet> },
    AnyCharStarPeekNext { c: u8 },
    WordBoundary { mode: ModeType },
    TextSegmentBoundary { boundary_type: TextSegmentBoundaryType, not: bool },
    CheckPosition { check_type: CheckPositionType },
    BackRefN { n1: MemNumType },
    BackRefGeneral { ns: Vec<MemNumType>, num: i32, nest_level: i32 },
    MemoryStart { num: MemNumType },
    MemoryEnd { num: MemNumType },
    Jump { addr: RelAddrType },
    Push { addr: RelAddrType },
    PushOrJumpExact1 { addr: RelAddrType, c: u8 },
    PushIfPeekNext { addr: RelAddrType, c: u8 },
    PopToMark { id: MemNumType },
    Repeat { id: MemNumType, addr: RelAddrType },
    RepeatInc { id: MemNumType },
    EmptyCheckStart { mem: MemNumType },
    EmptyCheckEnd { mem: MemNumType, empty_status_mem: MemStatusType },
    Move { n: RelPositionType },
    StepBackStart { initial: LengthType, remaining: LengthType, addr: RelAddrType },
    CutToMark { id: MemNumType, restore_pos: bool },
    Mark { id: MemNumType, save_pos: bool },
    SaveVal { save_type: SaveType, id: MemNumType },
    UpdateVar { var_type: UpdateVarType, id: MemNumType, clear: bool },
    Call { addr: AbsAddrType },
    CalloutContents { num: MemNumType },
    CalloutName { num: MemNumType, id: MemNumType },
}

// === Code Range (fuer Multibyte-Zeichenklassen) ===
#[derive(Clone, Debug)]
pub struct OnigCodeRange {
    pub from: OnigCodePoint,
    pub to: OnigCodePoint,
}

// === regex_t (re_pattern_buffer) ===
// Das ist die zentrale Struktur. 1:1 Abbildung der C-Felder.
pub struct RegexType {
    // Bytecode
    pub ops: Vec<Operation>,
    pub string_pool: Vec<u8>,

    // Capture info
    pub num_mem: i32,
    pub num_repeat: i32,
    pub num_empty_check: i32,
    pub num_call: i32,
    pub capture_history: MemStatusType,
    pub push_mem_start: MemStatusType,
    pub push_mem_end: MemStatusType,
    pub stack_pop_level: StackPopLevel,
    pub repeat_range: Vec<RepeatRange>,

    // Metadata
    pub enc: OnigEncoding,       // -> &dyn Encoding
    pub options: OnigOptionType,
    pub syntax: &'static OnigSyntaxType,
    pub case_fold_flag: OnigCaseFoldType,
    pub name_table: Option<HashMap<String, Vec<i32>>>,

    // Optimization
    pub optimize: OptimizeType,
    pub threshold_len: i32,
    pub anchor: i32,
    pub anc_dist_min: OnigLen,
    pub anc_dist_max: OnigLen,
    pub sub_anchor: i32,
    pub exact: Vec<u8>,
    pub map: [u8; 256],  // CHAR_MAP_SIZE
    pub map_offset: i32,
    pub dist_min: OnigLen,
    pub dist_max: OnigLen,

    // Extension (Callouts)
    pub extp: Option<RegexExt>,
}

// === RepeatRange (erweitert mit Body-Adresse) ===
pub struct RepeatRange {
    pub lower: i32,
    pub upper: i32,
}

// === Regex Extension (Callouts) ===
pub struct RegexExt {
    pub pattern: Vec<u8>,
    pub callout_num: i32,
    pub callout_list: Vec<CalloutListEntry>,
}
```

### 3.3 regparse_types.rs (aus regparse.h)

```rust
// === Node Type Enum ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeType {
    String = 0,
    CClass = 1,
    CType = 2,
    BackRef = 3,
    Quant = 4,
    Bag = 5,
    Anchor = 6,
    List = 7,
    Alt = 8,
    Call = 9,
    Gimmick = 10,
}

// === Bag Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BagType {
    Memory = 0,
    Option = 1,
    StopBacktrack = 2,
    IfElse = 3,
}

// === Gimmick Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GimmickType {
    Fail = 0,
    Save = 1,
    UpdateVar = 2,
    Callout = 3,
}

// === Body Empty Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyEmptyType {
    NotEmpty = 0,
    MayBeEmpty = 1,
    MayBeEmptyMem = 2,
    MayBeEmptyRec = 3,
}

// === Node Status Flags ===
pub const ND_ST_FIXED_MIN: u32 = 1 << 0;
pub const ND_ST_FIXED_MAX: u32 = 1 << 1;
pub const ND_ST_FIXED_CLEN: u32 = 1 << 2;
pub const ND_ST_MARK1: u32 = 1 << 3;
pub const ND_ST_MARK2: u32 = 1 << 4;
pub const ND_ST_STRICT_REAL_REPEAT: u32 = 1 << 5;
pub const ND_ST_RECURSION: u32 = 1 << 6;
pub const ND_ST_CALLED: u32 = 1 << 7;
pub const ND_ST_FIXED_ADDR: u32 = 1 << 8;
pub const ND_ST_NAMED_GROUP: u32 = 1 << 9;
pub const ND_ST_IN_REAL_REPEAT: u32 = 1 << 10;
pub const ND_ST_IN_ZERO_REPEAT: u32 = 1 << 11;
pub const ND_ST_IN_MULTI_ENTRY: u32 = 1 << 12;
pub const ND_ST_NEST_LEVEL: u32 = 1 << 13;
pub const ND_ST_BY_NUMBER: u32 = 1 << 14;
pub const ND_ST_BY_NAME: u32 = 1 << 15;
pub const ND_ST_BACKREF: u32 = 1 << 16;
pub const ND_ST_CHECKER: u32 = 1 << 17;
pub const ND_ST_PROHIBIT_RECURSION: u32 = 1 << 18;
pub const ND_ST_SUPER: u32 = 1 << 19;
pub const ND_ST_EMPTY_STATUS_CHECK: u32 = 1 << 20;
pub const ND_ST_IGNORECASE: u32 = 1 << 21;
pub const ND_ST_MULTILINE: u32 = 1 << 22;
pub const ND_ST_TEXT_SEGMENT_WORD: u32 = 1 << 23;
pub const ND_ST_ABSENT_WITH_SIDE_EFFECTS: u32 = 1 << 24;
pub const ND_ST_FIXED_CLEN_MIN_SURE: u32 = 1 << 25;
pub const ND_ST_REFERENCED: u32 = 1 << 26;
pub const ND_ST_INPEEK: u32 = 1 << 27;
pub const ND_ST_WHOLE_OPTIONS: u32 = 1 << 28;

// === BBuf (Byte Buffer, wie in C) ===
pub struct BBuf {
    pub data: Vec<u8>,
}

// === AST Node (als Rust enum mit Box fuer Rekursion) ===
// C benutzt eine Union. In Rust verwenden wir ein enum.
// Jeder Node hat gemeinsame Felder (status, parent), die wir in die
// Varianten einbauen oder als separate NodeBase extrahieren.

pub struct NodeBase {
    pub node_type: NodeType,
    pub status: u32,
    // parent: In C ein raw pointer. In Rust verwenden wir node IDs oder indices.
    // Fuer den Parser verwenden wir eine Arena oder Box-basierte Baumstruktur.
}

// Der Node-Baum. C verwendet `Node` als Union, Rust als enum:
pub enum Node {
    String(StrNode),
    CClass(CClassNode),
    CType(CtypeNode),
    BackRef(BackRefNode),
    Quant(QuantNode),
    Bag(BagNode),
    Anchor(AnchorNode),
    List(ConsAltNode),    // ND_LIST
    Alt(ConsAltNode),     // ND_ALT
    Call(CallNode),
    Gimmick(GimmickNode),
}

// Jede Node-Variante mit ihren spezifischen Feldern:

pub struct StrNode {
    pub status: u32,
    pub s: Vec<u8>,           // C: UChar* s .. UChar* end
    pub flag: u32,
    // In C: buf[24] fuer kleine Strings. In Rust nutzen wir SmallVec oder Vec.
}
// StrNode flag:
pub const ND_STRING_FLAG_CRUDE: u32 = 1 << 0;
pub const ND_STRING_FLAG_CASE_FOLD: u32 = 1 << 1;

pub struct CClassNode {
    pub status: u32,
    pub flags: u32,
    pub bs: BitSet,           // 256-Bit fuer Single-Byte
    pub mbuf: Option<BBuf>,   // Multi-Byte Ranges oder None
}
pub const CCLASS_FLAG_NOT: u32 = 1 << 0;

pub struct CtypeNode {
    pub status: u32,
    pub ctype: i32,
    pub not: bool,
    pub ascii_mode: bool,
}

pub struct QuantNode {
    pub status: u32,
    pub body: Box<Node>,
    pub lower: i32,
    pub upper: i32,
    pub greedy: bool,
    pub emptiness: BodyEmptyType,
    pub head_exact: Option<Box<Node>>,
    pub next_head_exact: Option<Box<Node>>,
    pub include_referred: i32,
    pub empty_status_mem: MemStatusType,
}

pub struct BagNode {
    pub status: u32,
    pub body: Option<Box<Node>>,
    pub bag_type: BagType,
    // Union-Felder als enum:
    pub bag_data: BagData,
    pub min_len: OnigLen,
    pub max_len: OnigLen,
    pub min_char_len: OnigLen,
    pub max_char_len: OnigLen,
    pub opt_count: i32,
}

pub enum BagData {
    Memory {
        regnum: i32,
        called_addr: AbsAddrType,
        entry_count: i32,
        called_state: i32,
    },
    Option {
        options: OnigOptionType,
    },
    StopBacktrack,
    IfElse {
        then_node: Option<Box<Node>>,
        else_node: Option<Box<Node>>,
    },
}

pub struct BackRefNode {
    pub status: u32,
    pub back_num: i32,
    pub back_static: [i32; 6],   // ND_BACKREFS_SIZE = 6
    pub back_dynamic: Option<Vec<i32>>,
    pub nest_level: i32,
}

pub struct AnchorNode {
    pub status: u32,
    pub body: Option<Box<Node>>,
    pub anchor_type: i32,
    pub char_min_len: OnigLen,
    pub char_max_len: OnigLen,
    pub ascii_mode: bool,
    pub lead_node: Option<Box<Node>>,
}

pub struct ConsAltNode {
    pub status: u32,
    pub car: Box<Node>,          // head
    pub cdr: Option<Box<Node>>,  // tail (None = end of list)
}

pub struct CallNode {
    pub status: u32,
    pub body: Option<Box<Node>>,
    pub by_number: bool,
    pub called_gnum: i32,
    pub name: Vec<u8>,
    pub entry_count: i32,
}

pub struct GimmickNode {
    pub status: u32,
    pub gimmick_type: GimmickType,
    pub detail_type: i32,
    pub num: i32,
    pub id: i32,
}

// === Parse Environment ===
pub struct ParseEnv {
    pub options: OnigOptionType,
    pub case_fold_flag: OnigCaseFoldType,
    pub enc: OnigEncoding,
    pub syntax: &'static OnigSyntaxType,
    pub cap_history: MemStatusType,
    pub backtrack_mem: MemStatusType,
    pub backrefed_mem: MemStatusType,
    pub pattern: Vec<u8>,  // Owned copy
    pub error: Option<String>,
    pub reg: *mut RegexType,  // Mutable reference to regex being built
    pub num_call: i32,
    pub num_mem: i32,
    pub num_named: i32,
    pub mem_env_static: [MemEnv; 8],  // PARSEENV_MEMENV_SIZE
    pub mem_env_dynamic: Option<Vec<MemEnv>>,
    pub backref_num: i32,
    pub keep_num: i32,
    pub id_num: i32,
    pub saves: Vec<SaveItem>,
    pub unset_addr_list: Vec<UnsetAddrEntry>,
    pub parse_depth: u32,
    pub max_parse_depth: u32,
    pub flags: u32,
}

pub const PE_FLAG_HAS_CALL_ZERO: u32 = 1 << 0;
pub const PE_FLAG_HAS_WHOLE_OPTIONS: u32 = 1 << 1;
pub const PE_FLAG_HAS_ABSENT_STOPPER: u32 = 1 << 2;

pub struct MemEnv {
    pub mem_node: Option<*mut Node>,  // In Rust: NodeId oder Index
    pub empty_repeat_node: Option<*mut Node>,
}

pub struct SaveItem {
    pub save_type: SaveType,
    pub id: i32,
}

pub struct UnsetAddrEntry {
    pub offset: i32,
    pub node: *mut Node,
}
```

---

## 4. Phase 2: Encoding-System

### 4.1 regenc.rs - Encoding Trait

Das C-Original verwendet einen `OnigEncodingType` struct mit 20 Funktionspointern.
In Rust wird daraus ein Trait:

```rust
/// Exakte Abbildung von OnigEncodingTypeST (oniguruma.h)
/// Jede Methode entspricht einem Funktionspointer im C-Original.
pub trait Encoding: Send + Sync {
    /// Name des Encodings (z.B. "UTF-8")
    fn name(&self) -> &str;

    /// Maximale Byte-Laenge eines Zeichens
    fn max_enc_len(&self) -> usize;

    /// Minimale Byte-Laenge eines Zeichens
    fn min_enc_len(&self) -> usize;

    /// Byte-Laenge des Zeichens ab Position p
    /// C: int (*mbc_enc_len)(const OnigUChar* p)
    fn mbc_enc_len(&self, p: &[u8]) -> usize;

    /// Pruefen ob Newline an Position
    /// C: int (*is_mbc_newline)(const OnigUChar* p, const OnigUChar* end)
    fn is_mbc_newline(&self, p: &[u8]) -> bool;

    /// Multibyte -> Codepoint
    /// C: OnigCodePoint (*mbc_to_code)(const OnigUChar* p, const OnigUChar* end)
    fn mbc_to_code(&self, p: &[u8]) -> OnigCodePoint;

    /// Codepoint -> benoetigte Byte-Laenge
    /// C: int (*code_to_mbclen)(OnigCodePoint code)
    fn code_to_mbclen(&self, code: OnigCodePoint) -> usize;

    /// Codepoint -> Bytes schreiben
    /// C: int (*code_to_mbc)(OnigCodePoint code, OnigUChar *buf)
    fn code_to_mbc(&self, code: OnigCodePoint, buf: &mut [u8]) -> usize;

    /// Case-Fold eines Zeichens
    /// C: int (*mbc_case_fold)(OnigCaseFoldType flag, const OnigUChar** pp,
    ///                         const OnigUChar* end, OnigUChar* to)
    fn mbc_case_fold(&self, flag: OnigCaseFoldType, p: &[u8], to: &mut [u8]) -> (usize, usize);
    // Returns: (bytes_consumed, bytes_written)

    /// Alle Case-Fold-Mappings anwenden (Callback-basiert)
    /// C: int (*apply_all_case_fold)(OnigCaseFoldType flag,
    ///                               OnigApplyAllCaseFoldFunc f, void* arg)
    fn apply_all_case_fold<F>(&self, flag: OnigCaseFoldType, f: &mut F) -> i32
    where F: FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32;

    /// Case-Fold-Codes fuer einen String holen
    /// C: int (*get_case_fold_codes_by_str)(OnigCaseFoldType flag,
    ///         const OnigUChar* p, const OnigUChar* end, OnigCaseFoldCodeItem acs[])
    fn get_case_fold_codes_by_str(&self, flag: OnigCaseFoldType, p: &[u8])
        -> Vec<OnigCaseFoldCodeItem>;

    /// Property-Name -> Ctype
    /// C: int (*property_name_to_ctype)(OnigEncoding enc,
    ///         OnigUChar* p, OnigUChar* end)
    fn property_name_to_ctype(&self, name: &[u8]) -> Result<i32, i32>;

    /// Pruefen ob Codepoint einen Typ hat
    /// C: int (*is_code_ctype)(OnigCodePoint code, OnigCtype ctype)
    fn is_code_ctype(&self, code: OnigCodePoint, ctype: u32) -> bool;

    /// Ctype -> Code-Ranges
    /// C: int (*get_ctype_code_range)(OnigCtype ctype, OnigCodePoint* sb_out,
    ///         const OnigCodePoint* ranges[])
    fn get_ctype_code_range(&self, ctype: u32) -> Option<(OnigCodePoint, &[OnigCodePoint])>;

    /// Links-Adjustment des Zeichenkopfs
    /// C: OnigUChar* (*left_adjust_char_head)(const OnigUChar* start,
    ///                                        const OnigUChar* p)
    fn left_adjust_char_head(&self, start: &[u8], p_offset: usize) -> usize;

    /// Erlaubt Reverse-Match?
    /// C: int (*is_allowed_reverse_match)(const OnigUChar* p, const OnigUChar* end)
    fn is_allowed_reverse_match(&self, p: &[u8]) -> bool;

    /// Initialisierung (optional)
    fn init(&self) -> i32 { 0 }

    /// Ist initialisiert?
    fn is_initialized(&self) -> bool { true }

    /// String-Validierung
    fn is_valid_mbc_string(&self, s: &[u8]) -> bool;

    /// Encoding Flags
    fn flag(&self) -> u32;

    /// Single-Byte Range
    fn sb_range(&self) -> OnigCodePoint { 0x80 }

    // === Hilfs-Methoden (aus regenc.c) ===

    /// Encoding-bewusste String-Laenge (Zeichenanzahl)
    fn strlen(&self, s: &[u8]) -> usize {
        let mut count = 0;
        let mut i = 0;
        while i < s.len() {
            i += self.mbc_enc_len(&s[i..]);
            count += 1;
        }
        count
    }

    /// N Zeichen vorwaerts gehen
    fn step(&self, s: &[u8], pos: usize, n: usize) -> usize {
        let mut p = pos;
        for _ in 0..n {
            if p >= s.len() { break; }
            p += self.mbc_enc_len(&s[p..]);
        }
        p
    }

    /// N Zeichen rueckwaerts gehen
    fn step_back(&self, s: &[u8], pos: usize, n: usize) -> Option<usize> {
        let mut p = pos;
        for _ in 0..n {
            if p == 0 { return None; }
            p = self.left_adjust_char_head(s, p - 1);
        }
        Some(p)
    }

    /// ASCII-kompatibel?
    fn is_ascii_compatible(&self) -> bool {
        (self.flag() & ENC_FLAG_ASCII_COMPATIBLE) != 0
    }

    /// Unicode-basiert?
    fn is_unicode(&self) -> bool {
        (self.flag() & ENC_FLAG_UNICODE) != 0
    }
}

pub const ENC_FLAG_ASCII_COMPATIBLE: u32 = 1 << 0;
pub const ENC_FLAG_UNICODE: u32 = 1 << 1;
pub const ENC_FLAG_SKIP_OFFSET_MASK: u32 = 7 << 2;

// OnigEncoding ist ein Trait-Object-Pointer
pub type OnigEncoding = &'static dyn Encoding;
```

### 4.2 Encoding-Implementierungen

Jede Encoding-Datei folgt dem gleichen Muster wie im C-Original:

**ascii.rs** (Beispiel):
```rust
pub struct AsciiEncoding;

impl Encoding for AsciiEncoding {
    fn name(&self) -> &str { "US-ASCII" }
    fn max_enc_len(&self) -> usize { 1 }
    fn min_enc_len(&self) -> usize { 1 }
    fn mbc_enc_len(&self, _p: &[u8]) -> usize { 1 }
    fn is_mbc_newline(&self, p: &[u8]) -> bool { !p.is_empty() && p[0] == 0x0a }
    fn mbc_to_code(&self, p: &[u8]) -> OnigCodePoint { p[0] as OnigCodePoint }
    fn code_to_mbclen(&self, _code: OnigCodePoint) -> usize { 1 }
    fn code_to_mbc(&self, code: OnigCodePoint, buf: &mut [u8]) -> usize {
        buf[0] = code as u8;
        1
    }
    // ... alle weiteren Trait-Methoden
}

pub static ONIG_ENCODING_ASCII: AsciiEncoding = AsciiEncoding;
```

**utf8.rs** (Beispiel):
```rust
// Exakte Abbildung der EncLen_UTF8 Tabelle
static ENC_LEN_UTF8: [u8; 256] = [
    1,1,1,1,1,1,1,1, 1,1,1,1,1,1,1,1,  // 0x00-0x0F
    1,1,1,1,1,1,1,1, 1,1,1,1,1,1,1,1,  // 0x10-0x1F
    // ... exakt wie in utf8.c
    2,2,2,2,2,2,2,2, 2,2,2,2,2,2,2,2,  // 0xC0-0xCF
    2,2,2,2,2,2,2,2, 2,2,2,2,2,2,2,2,  // 0xD0-0xDF
    3,3,3,3,3,3,3,3, 3,3,3,3,3,3,3,3,  // 0xE0-0xEF
    4,4,4,4,4,4,4,4, 1,1,1,1,1,1,1,1,  // 0xF0-0xFF
];

pub struct Utf8Encoding;

impl Encoding for Utf8Encoding {
    fn name(&self) -> &str { "UTF-8" }
    fn max_enc_len(&self) -> usize { 4 }
    fn min_enc_len(&self) -> usize { 1 }
    fn mbc_enc_len(&self, p: &[u8]) -> usize {
        ENC_LEN_UTF8[p[0] as usize] as usize
    }
    fn mbc_to_code(&self, p: &[u8]) -> OnigCodePoint {
        let len = ENC_LEN_UTF8[p[0] as usize] as usize;
        match len {
            1 => p[0] as OnigCodePoint,
            2 => ((p[0] as u32 & 0x1f) << 6) | (p[1] as u32 & 0x3f),
            3 => ((p[0] as u32 & 0x0f) << 12) | ((p[1] as u32 & 0x3f) << 6)
                 | (p[2] as u32 & 0x3f),
            4 => ((p[0] as u32 & 0x07) << 18) | ((p[1] as u32 & 0x3f) << 12)
                 | ((p[2] as u32 & 0x3f) << 6) | (p[3] as u32 & 0x3f),
            _ => p[0] as OnigCodePoint,
        }
    }
    // ... exakt wie in utf8.c
}
```

### 4.3 Reihenfolge der Encoding-Portierung

1. **ascii.rs** - Einfachstes Encoding, Basis fuer Tests
2. **utf8.rs** - Wichtigstes Encoding, benoetigt Unicode-Modul
3. **iso8859_1.rs** - Einfaches Single-Byte mit Case-Fold
4. **utf16_be.rs** / **utf16_le.rs** - Surrogate-Pair Handling
5. **utf32_be.rs** / **utf32_le.rs** - Einfachstes Multi-Byte (fixe 4 Bytes)
6. **euc_jp.rs** / **sjis.rs** - Komplexe Multi-Byte
7. Alle restlichen iso8859_*, koi8*, cp1251, euc_*, gb18030, big5

---

## 5. Phase 3: Parser (regparse)

### 5.1 regparse.rs - 1:1 Abbildung von regparse.c

Der Parser ist eine rekursive-Abstiegs-Implementierung. Jede C-Funktion wird
zu einer Rust-Funktion mit moeglichst identischem Namen.

**Zentrale Datenstrukturen:**

```rust
// === Token (PToken aus C) ===
pub struct PToken {
    pub token_type: TokenType,
    pub escaped: bool,
    pub code_point_continue: bool,
    pub data: TokenData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenType {
    // Exakt wie TokenSyms in regparse.c
    Eol,
    Raw,
    String,
    CharClass,
    AnyChar,
    Backref,
    Subexp,
    Anchor,
    Repeat,
    Interval,
    AltBar,
    CcOpen,
    CcClose,
    CcRange,
    CcIntersect,
    CcDifference,
    PosixBracketOpen,
    // ... alle Token-Typen aus regparse.c
    CallOf,
    KeepMark,
    GeneralNewline,
    NoNewline,
    TrueAnychar,
    TextSegment,
    LineBreak,
}

pub enum TokenData {
    None,
    Codepoint(OnigCodePoint),
    Backref { nums: Vec<i32>, by_name: bool },
    Repeat { lower: i32, upper: i32, greedy: bool, possessive: bool },
    Anchor(i32),
    // ... alle token-spezifischen Daten
}
```

**Hauptfunktionen (exakt wie regparse.c):**

```rust
// regparse.rs

/// Haupteinstieg - entspricht onig_parse_tree()
pub fn onig_parse_tree(
    root: &mut Option<Box<Node>>,
    pattern: &[u8],
    env: &mut ParseEnv,
) -> Result<(), i32> { ... }

/// Tokenizer - entspricht fetch_token()
fn fetch_token(tok: &mut PToken, src: &mut &[u8], end: &[u8], env: &ParseEnv) -> i32 { ... }

/// Top-Level Alternation - entspricht prs_alts()
fn prs_alts(tok: &mut PToken, src: &mut &[u8], end: &[u8], env: &mut ParseEnv) -> Result<Box<Node>, i32> { ... }

/// Sequenz - entspricht prs_branch() / prs_list()
fn prs_branch(tok: &mut PToken, src: &mut &[u8], end: &[u8], env: &mut ParseEnv) -> Result<Box<Node>, i32> { ... }

/// Quantifier - Wraps prs_atom() mit ?, *, +, {n,m}
fn prs_exp(tok: &mut PToken, src: &mut &[u8], end: &[u8], env: &mut ParseEnv, term: TokenType, group_head: bool) -> Result<Box<Node>, i32> { ... }

/// Atomare Elemente: Literale, Gruppen, Zeichenklassen, Anker, ...
fn prs_atom(tok: &mut PToken, src: &mut &[u8], end: &[u8], env: &mut ParseEnv) -> Result<Box<Node>, i32> { ... }

/// Zeichenklasse [a-z] - entspricht parse_cc()
fn parse_cc(src: &mut &[u8], end: &[u8], env: &mut ParseEnv) -> Result<Box<Node>, i32> { ... }

/// Escape-Sequenz - entspricht fetch_token_cc() und Teile von prs_atom()
fn parse_escape(src: &mut &[u8], end: &[u8], env: &ParseEnv) -> Result<OnigCodePoint, i32> { ... }

/// Named Group Management
fn onig_name_to_group_numbers(reg: &RegexType, name: &[u8]) -> Option<&[i32]> { ... }

/// Node-Erstellung (entspricht node_new_* Funktionen)
fn node_new_str(s: &[u8]) -> Box<Node> { ... }
fn node_new_cclass() -> Box<Node> { ... }
fn node_new_quant(body: Box<Node>, lower: i32, upper: i32, greedy: bool) -> Box<Node> { ... }
fn node_new_bag_memory(regnum: i32) -> Box<Node> { ... }
fn node_new_anchor(anchor_type: i32) -> Box<Node> { ... }
fn node_new_alt(car: Box<Node>, cdr: Option<Box<Node>>) -> Box<Node> { ... }
fn node_new_list(car: Box<Node>, cdr: Option<Box<Node>>) -> Box<Node> { ... }
// ... alle node_new_* aus regparse.c

/// Quantifier-Reduktion - entspricht onig_reduce_nested_quantifier()
fn reduce_nested_quantifier(qnode: &mut QuantNode, child_qn: &QuantNode) { ... }
```

### 5.2 Wichtige C-zu-Rust Anpassungen im Parser

| C-Pattern | Rust-Umsetzung |
|-----------|----------------|
| `UChar* p; p++` | `&[u8]` Slice mit `src = &src[1..]` oder Index-Variable |
| `PFETCH(c)` Macro | `let c = src[0]; src = &src[1..];` |
| `PPEEK` Macro | `src[0]` (peek) |
| `PUNFETCH` Macro | `src = &original_src[pos - 1..];` (muss Position merken) |
| `NULL` Return | `Result<T, i32>` oder `Option<T>` |
| `goto` Statements | Rust `loop` + `break`/`continue` oder benannte Bloecke |
| `node->u.str.s` | Pattern-Match: `if let Node::String(ref mut sn) = *node { ... }` |
| Malloc/Free | Automatisch durch Box/Vec Ownership |
| `st_table*` Hash | `HashMap<Vec<u8>, Vec<i32>>` |

---

## 6. Phase 4: Compiler (regcomp)

### 6.1 regcomp.rs - 1:1 Abbildung von regcomp.c

**Hauptfunktionen:**

```rust
// === Oeffentliche API ===

/// Haupteinstieg - entspricht onig_compile()
pub fn onig_compile(
    reg: &mut RegexType,
    pattern: &[u8],
    enc: OnigEncoding,
    syntax: &'static OnigSyntaxType,
    option: OnigOptionType,
) -> Result<(), i32> { ... }

// === Parse + Optimize + Compile Pipeline ===

/// Entspricht parse_and_tune() (regcomp.c:7381)
fn parse_and_tune(
    root: &mut Option<Box<Node>>,
    pattern: &[u8],
    reg: &mut RegexType,
    env: &mut ParseEnv,
) -> Result<(), i32> {
    // 1. Parse
    onig_parse_tree(root, pattern, env)?;
    // 2. Reduce string list
    reduce_string_list(root);
    // 3. Tune tree (Optimierungen)
    tune_tree(root, reg, env)?;
    Ok(())
}

// === Compile Phase ===

/// Bytecode-Laenge berechnen - entspricht compile_length_tree()
fn compile_length_tree(node: &Node, reg: &RegexType, env: &ParseEnv) -> Result<usize, i32> { ... }

/// Bytecode generieren - entspricht compile_tree()
fn compile_tree(node: &Node, reg: &mut RegexType, env: &ParseEnv) -> Result<(), i32> { ... }

/// String-Opcode waehlen - entspricht select_str_opcode()
fn select_str_opcode(mb_len: usize, str_len: usize) -> OpCode { ... }

/// String kompilieren - entspricht compile_string_node()
fn compile_string_node(node: &StrNode, reg: &mut RegexType) -> Result<(), i32> { ... }

/// Zeichenklasse kompilieren
fn compile_cclass_node(node: &CClassNode, reg: &mut RegexType) -> Result<(), i32> { ... }

/// Quantifier kompilieren
fn compile_range_repeat_node(qn: &QuantNode, target_len: usize, empty_info: BodyEmptyType,
                              reg: &mut RegexType, env: &ParseEnv) -> Result<(), i32> { ... }

/// Nullable-Quantifier mit Empty-Check
fn compile_quant_body_with_empty_check(qn: &QuantNode, reg: &mut RegexType, env: &ParseEnv) -> Result<(), i32> { ... }

/// Operation hinzufuegen - entspricht add_op()
fn add_op(reg: &mut RegexType, op: Operation) -> Result<(), i32> { ... }

// === Tune/Optimize Phase ===

/// Hauptoptimierung - entspricht tune_tree()
fn tune_tree(node: &mut Box<Node>, reg: &mut RegexType, env: &mut ParseEnv) -> Result<(), i32> { ... }

/// String-Listen mergen
fn reduce_string_list(node: &mut Option<Box<Node>>) { ... }

/// Optimierungsinfo extrahieren - entspricht set_optimize_info_from_tree()
fn set_optimize_info_from_tree(reg: &mut RegexType, node: &Node, env: &ParseEnv) -> Result<(), i32> { ... }

/// String-Pool erstellen - entspricht ops_make_string_pool()
fn ops_make_string_pool(reg: &mut RegexType) -> Result<(), i32> { ... }
```

### 6.2 Compile-Tree Dispatching

```rust
fn compile_tree(node: &Node, reg: &mut RegexType, env: &ParseEnv) -> Result<(), i32> {
    match node {
        Node::String(sn) => {
            if sn.flag & ND_STRING_FLAG_CRUDE != 0 {
                compile_string_crude_node(sn, reg)
            } else {
                compile_string_node_encoding(sn, reg)
            }
        }
        Node::CClass(cc) => compile_cclass_node(cc, reg),
        Node::CType(ct) => compile_ctype_node(ct, reg),
        Node::BackRef(br) => compile_backref_node(br, reg),
        Node::Quant(qn) => compile_quantifier_node(qn, reg, env),
        Node::Bag(bn) => compile_bag_node(bn, reg, env),
        Node::Anchor(an) => compile_anchor_node(an, reg, env),
        Node::List(cn) => compile_list_node(cn, reg, env),
        Node::Alt(cn) => compile_alt_node(cn, reg, env),
        Node::Call(cn) => compile_call_node(cn, reg, env),
        Node::Gimmick(gn) => compile_gimmick_node(gn, reg),
    }
}
```

---

## 7. Phase 5: Execution Engine (regexec)

### 7.1 regexec.rs - 1:1 Abbildung von regexec.c

**Stack-Typ (exakt wie C):**

```rust
// === Stack Entry Types ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum StackType {
    Void = 0x0000,
    SuperAlt = 0x0001,
    Alt = 0x0002,
    MemStart = 0x0010,
    MemEnd = 0x8030,
    RepeatInc = 0x0040,
    EmptyCheckStart = 0x3000,
    EmptyCheckEnd = 0x5000,
    MemEndMark = 0x8100,
    CallFrame = 0x0400,
    Return = 0x0500,
    SaveVal = 0x0600,
    Mark = 0x0704,
    Callout = 0x0070,
}

// === Stack Entry (C union -> Rust enum) ===
pub enum StackEntry {
    Alt {
        op_index: usize,          // C: Operation* pcode -> Index in ops[]
        str_pos: usize,           // C: UChar* pstr -> Index in Eingabestring
    },
    Mem {
        num: MemNumType,
        str_pos: usize,
        prev_start: StkPtr,
        prev_end: StkPtr,
    },
    RepeatInc {
        id: MemNumType,
        count: i32,
        prev_index: i32,          // Stack-Index
    },
    EmptyCheck {
        id: MemNumType,
        str_pos: usize,
        prev_index: i32,
    },
    CallFrame {
        ret_addr: usize,          // Op-Index
        str_pos: usize,
    },
    SaveVal {
        save_type: SaveType,
        id: MemNumType,
        v: usize,                 // gespeicherte Position
        v2: usize,                // sprev
    },
    Mark {
        id: MemNumType,
        str_pos: usize,
    },
    Void,
}

// StkPtr: entweder String-Position oder Stack-Index
pub enum StkPtr {
    Pos(usize),
    StackIndex(i32),
    Invalid,
}
```

**Hauptfunktionen:**

```rust
// === Oeffentliche API ===

/// Suche - entspricht onig_search()
pub fn onig_search(
    reg: &RegexType,
    str_bytes: &[u8],
    start: usize,
    range: usize,
    region: &mut OnigRegion,
    option: OnigOptionType,
) -> Result<i32, i32> { ... }

/// Match an Position - entspricht onig_match()
pub fn onig_match(
    reg: &RegexType,
    str_bytes: &[u8],
    at: usize,
    region: &mut OnigRegion,
    option: OnigOptionType,
) -> Result<i32, i32> { ... }

// === Interne Funktionen ===

/// Vorwaerts-Suche Optimierung - entspricht forward_search()
fn forward_search(
    reg: &RegexType,
    str_bytes: &[u8],
    start: usize,
    range: usize,
) -> Option<(usize, usize)> { ... }  // Returns (low, high)

/// Suche im Bereich - entspricht search_in_range()
fn search_in_range(
    reg: &RegexType,
    str_bytes: &[u8],
    start: usize,
    range: usize,
    data_range: usize,
    region: &mut OnigRegion,
    option: OnigOptionType,
    mp: &MatchParam,
) -> Result<i32, i32> { ... }

/// Kern-Interpreter - entspricht match_at()
/// Dies ist die groesste und kritischste Funktion (~3000 Zeilen in C)
fn match_at(
    reg: &RegexType,
    str_bytes: &[u8],
    right_range: usize,
    sstart: usize,
    msa: &mut MatchArg,
) -> i32 {
    let mut stack: Vec<StackEntry> = Vec::with_capacity(INIT_MATCH_STACK_SIZE);
    let mut s = sstart;           // aktuelle String-Position
    let mut p = 0usize;           // aktuelle Operation-Position (Index in reg.ops)
    let mut best_len = ONIG_MISMATCH;
    let mut keep = sstart;

    // Capture Arrays initialisieren
    let num_mem = reg.num_mem as usize;
    let mut mem_start_stk = vec![StkPtr::Invalid; num_mem + 1];
    let mut mem_end_stk = vec![StkPtr::Invalid; num_mem + 1];

    // Bottom Stack Push
    stack.push(StackEntry::Alt { op_index: 0 /* FinishCode */, str_pos: s });

    loop {
        if p >= reg.ops.len() { break; }
        let op = &reg.ops[p];

        match op.opcode {
            OpCode::End => { /* ... Match-Ende-Logik ... */ }
            OpCode::Str1 => { /* 1-Byte Vergleich */ }
            OpCode::Str2 => { /* 2-Byte Vergleich */ }
            // ... alle 93+ Opcodes ...
            OpCode::Fail => {
                // STACK_POP: letzten Alt-Eintrag finden und restaurieren
                if let Some(entry) = stack_pop(&mut stack) {
                    match entry {
                        StackEntry::Alt { op_index, str_pos } => {
                            p = op_index;
                            s = str_pos;
                            continue;
                        }
                        // ... andere Stack-Typen
                    }
                } else {
                    break; // Stack leer -> kein Match
                }
            }
            OpCode::Jump => {
                if let OperationPayload::Jump { addr } = op.payload {
                    p = (p as i32 + addr) as usize;
                    continue;
                }
            }
            OpCode::Push => {
                if let OperationPayload::Push { addr } = op.payload {
                    let alt_target = (p as i32 + addr) as usize;
                    stack.push(StackEntry::Alt { op_index: alt_target, str_pos: s });
                }
            }
            // ... ALLE weiteren Opcodes 1:1 wie match_at() in regexec.c
            OpCode::Finish => { break; }
            _ => { return ONIGERR_UNDEFINED_BYTECODE; }
        }
        p += 1; // INC_OP
    }

    best_len
}
```

### 7.2 Match-Param Struktur

```rust
pub struct MatchParam {
    pub match_stack_limit: u32,
    pub retry_limit_in_match: u64,
    pub retry_limit_in_search: u64,
    pub time_limit: u64,  // Millisekunden
    pub callout_user_data: Option<Box<dyn std::any::Any>>,
}

pub struct MatchArg {
    pub stack: Vec<StackEntry>,
    pub options: OnigOptionType,
    pub region: *mut OnigRegion,
    pub start: usize,
    pub best_len: i32,
    pub best_s: usize,
    pub mp: MatchParam,
    pub retry_limit_in_search_counter: u64,
}
```

---

## 8. Phase 6: Nebenmodule

### 8.1 regsyntax.rs (aus regsyntax.c)

Alle vordefinierten Syntax-Definitionen als `static`-Konstanten:

```rust
pub static ONIG_SYNTAX_ONIGURUMA: OnigSyntaxType = OnigSyntaxType {
    op: ONIG_SYN_OP_DOT_ANYCHAR | ONIG_SYN_OP_ASTERISK_ZERO_INF | ... ,
    op2: ONIG_SYN_OP2_QMARK_GROUP_EFFECT | ... ,
    behavior: ONIG_SYN_CONTEXT_INDEP_REPEAT_OPS | ... ,
    options: ONIG_OPTION_NONE,
    meta_char_table: OnigMetaCharTableType {
        esc: '\\' as u32,
        anychar: '.' as u32,
        // ...
    },
};

pub static ONIG_SYNTAX_PERL: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_JAVA: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_PYTHON: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_RUBY: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_POSIX_BASIC: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_POSIX_EXTENDED: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_EMACS: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_GREP: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_GNU: OnigSyntaxType = ...;
pub static ONIG_SYNTAX_ASIS: OnigSyntaxType = ...;
// Die exakten Flag-Kombinationen muessen 1:1 aus regsyntax.c uebernommen werden!
```

### 8.2 regerror.rs (aus regerror.c)

```rust
/// Fehlermeldung generieren - entspricht onig_error_code_to_str()
pub fn onig_error_code_to_str(code: i32, info: Option<&OnigErrorInfo>) -> String { ... }

// Fehlertexte als Konstanten, exakt wie in regerror.c
static ERROR_MESSAGES: &[(i32, &str)] = &[
    (ONIGERR_MEMORY, "fail to memory allocation"),
    (ONIGERR_MATCH_STACK_LIMIT_OVER, "match-stack limit over"),
    // ... alle ~60 Fehlercodes
];
```

### 8.3 st.rs (aus st.c)

**Option A: std::HashMap verwenden** (empfohlen fuer Rust)
```rust
// Named Groups werden als HashMap verwaltet
pub type NameTable = HashMap<Vec<u8>, Vec<i32>>;
```

**Option B: 1:1 Port von st.c** (nur wenn Kompatibilitaet noetig)
```rust
pub struct StTable<K, V> {
    bins: Vec<Option<Box<StEntry<K, V>>>>,
    num_bins: usize,
    num_entries: usize,
}
```

**Empfehlung:** HashMap verwenden, da st.c ein generischer Hash-Table ist und
Rust bereits eine hervorragende Implementierung hat. Das aendert nicht die
Semantik, nur die interne Datenstruktur.

### 8.4 regtrav.rs (aus regtrav.c)

```rust
/// Capture-History traversieren - entspricht onig_capture_tree_traverse()
pub fn onig_capture_tree_traverse(
    region: &OnigRegion,
    at: i32,  // ONIG_TRAVERSE_CALLBACK_AT_FIRST / LAST / BOTH
    callback: &mut dyn FnMut(i32, usize, usize, i32, &OnigCaptureTreeNode) -> i32,
) -> i32 { ... }
```

---

## 9. Phase 7: RegSet & Callouts

### 9.1 RegSet (aus regexec.c, USE_REGSET Block)

```rust
pub struct OnigRegSet {
    rs: Vec<RegSetEntry>,
    enc: OnigEncoding,
    anchor: i32,
    anc_dmin: OnigLen,
    anc_dmax: OnigLen,
    all_low_high: i32,
    anychar_inf: i32,
}

struct RegSetEntry {
    reg: RegexType,
    region: OnigRegion,
}

/// RegSet-Suche - entspricht onig_regset_search()
pub fn onig_regset_search(
    set: &OnigRegSet,
    str_bytes: &[u8],
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    match_pos: &mut usize,
) -> Result<i32, i32> { ... }
```

### 9.2 Callouts (aus regext.c, USE_CALLOUT Block)

```rust
pub type OnigCalloutFunc = fn(
    args: &OnigCalloutArgs,
    user_data: Option<&dyn std::any::Any>,
) -> i32;

pub struct OnigCalloutArgs {
    pub string: *const u8,
    pub string_end: *const u8,
    pub start: *const u8,
    pub right_range: *const u8,
    pub current: *const u8,
    pub regex: *const RegexType,
    pub match_stack_top: usize,
}

pub struct CalloutListEntry {
    pub flag: i32,
    pub of: OnigCalloutOf,
    pub in_flag: i32,
    pub name_id: i32,
    pub callout_type: OnigCalloutType,
    pub start_func: Option<OnigCalloutFunc>,
    pub end_func: Option<OnigCalloutFunc>,
    // ... content/arg union
}
```

---

## 10. C-zu-Rust Uebersetzungsregeln

### 10.1 Allgemeine Regeln

| C-Muster | Rust-Aequivalent | Anmerkung |
|----------|-------------------|-----------|
| `int func()` return 0/error | `fn func() -> Result<(), i32>` | Fehlercode als Err |
| `int func()` return length/-1 | `fn func() -> i32` | Direkt, wenn -1 = MISMATCH |
| `NULL` pointer | `Option<T>` | None statt NULL |
| `malloc/free` | `Box<T>`, `Vec<T>` | Automatisch |
| `goto fail;` | `break 'label;` oder `return Err(...)` | Benannte Bloecke |
| `switch(x) { case A: ... }` | `match x { A => ... }` | 1:1 |
| `#define MACRO(x) ...` | `#[inline] fn macro_name(x) -> ...` | Oder `macro_rules!` |
| `UChar* p, *end` | `&[u8]` oder `(p: usize, data: &[u8])` | Index-basiert |
| `p++; c = *p` | `c = data[p]; p += 1;` | Expliziter Index |
| `struct { union { ... } }` | `enum Variant { ... }` | Tagged Union |
| `(type)cast` | `as type` oder `From/Into` | Explizit |
| `memcpy(dst, src, n)` | `dst[..n].copy_from_slice(&src[..n])` | Slice-Copy |
| `memset(p, 0, n)` | `p.fill(0)` oder `vec![0; n]` | |
| `memcmp(a, b, n)` | `a[..n] == b[..n]` | |
| `static int x = 0;` | `static X: AtomicI32 = ...` (mutable) oder `const X: i32 = 0` | |
| Funktionspointer | `fn(args) -> ret` oder `&dyn Fn(args) -> ret` | |
| Bit-Flags `x & FLAG` | Gleich: `x & FLAG != 0` | Bitflags identisch |

### 10.2 Pointer-Arithmetik

Das C-Original nutzt extensiv Pointer-Arithmetik (`p - reg->ops`, `s - str`).
In Rust verwenden wir ueberall **Indices** statt Pointer:

```rust
// C:
//   Operation* p = reg->ops;
//   p++;
//   offset = p - reg->ops;

// Rust:
let mut p: usize = 0;  // Index in reg.ops
p += 1;
let offset = p;

// C:
//   UChar* s = str + pos;
//   c = *s++;

// Rust:
let mut s: usize = pos;  // Index in str_bytes
let c = str_bytes[s];
s += 1;
```

### 10.3 Stack-Management

```rust
// C: alloca() fuer kleine Stacks, malloc fuer grosse
// Rust: Immer Vec, da Vec auf dem Heap allokiert und automatisch waechst

let mut stack: Vec<StackEntry> = Vec::with_capacity(INIT_MATCH_STACK_SIZE);

// C: STACK_PUSH_ALT(p, s)
stack.push(StackEntry::Alt { op_index: p, str_pos: s });

// C: STACK_POP -> sucht rueckwaerts nach einem ALT-Eintrag
fn stack_pop(stack: &mut Vec<StackEntry>) -> Option<StackEntry> {
    while let Some(entry) = stack.pop() {
        match entry {
            StackEntry::Alt { .. } | StackEntry::Void => return Some(entry),
            _ => continue, // Non-ALT Eintraege ueberspringen
        }
    }
    None
}
```

### 10.4 Fehlerbehandlung

```rust
// C:
//   r = some_function();
//   if (r != 0) return r;
//   CHECK_NULL_RETURN_MEMERR(p);

// Rust:
some_function()?;                        // ? Operator fuer Result
let p = some_alloc().ok_or(ONIGERR_MEMORY)?;
```

---

## 11. Datei-fuer-Datei Mapping

| # | C-Datei | Rust-Datei | Prioritaet | Abhaengigkeiten | Geschaetzte Zeilen |
|---|---------|------------|------------|-----------------|-------------------|
| 1 | oniguruma.h | oniguruma.rs | P0 | - | ~800 |
| 2 | regint.h | regint.rs | P0 | oniguruma.rs | ~600 |
| 3 | regparse.h | regparse_types.rs | P0 | regint.rs | ~500 |
| 4 | regenc.h + regenc.c | regenc.rs | P1 | oniguruma.rs | ~800 |
| 5 | ascii.c | encodings/ascii.rs | P1 | regenc.rs | ~200 |
| 6 | utf8.c | encodings/utf8.rs | P1 | regenc.rs, unicode/ | ~400 |
| 7 | unicode.c | unicode/mod.rs | P1 | regenc.rs | ~1500 |
| 8 | unicode_fold_data.c | unicode/fold_data.rs | P1 | - (Daten) | ~3000 |
| 9 | unicode_property_data.c | unicode/property_data.rs | P1 | - (Daten) | ~8000 |
| 10 | unicode_fold*_key.c | unicode/fold*_key.rs | P1 | - (gperf->phf) | ~500 |
| 11 | unicode_unfold_key.c | unicode/unfold_key.rs | P1 | - (gperf->phf) | ~500 |
| 12 | unicode_egcb_data.c | unicode/egcb_data.rs | P2 | - (Daten) | ~1000 |
| 13 | unicode_wb_data.c | unicode/wb_data.rs | P2 | - (Daten) | ~1000 |
| 14 | regsyntax.c | regsyntax.rs | P1 | oniguruma.rs | ~400 |
| 15 | st.c + st.h | st.rs | P1 | - | ~100 (HashMap) |
| 16 | **regparse.c** | **regparse.rs** | **P2** | Alles oben | **~12000** |
| 17 | **regcomp.c** | **regcomp.rs** | **P3** | regparse.rs | **~10000** |
| 18 | **regexec.c** | **regexec.rs** | **P4** | regcomp.rs | **~8000** |
| 19 | regerror.c | regerror.rs | P2 | oniguruma.rs | ~300 |
| 20 | regext.c | regext.rs | P5 | regint.rs | ~200 |
| 21 | regtrav.c | regtrav.rs | P5 | oniguruma.rs | ~100 |
| 22 | regversion.c | regversion.rs | P5 | - | ~30 |
| 23-52 | iso8859_*.c, euc_*.c, etc. | encodings/*.rs | P3-P5 | regenc.rs | ~200-800 je |
| 53 | regposix.c + regposerr.c | regposix.rs | P5 | regexec.rs | ~400 |
| 54 | reggnu.c | (optional) | P5 | - | ~100 |

**Prioritaeten:**
- **P0**: Grundtypen - muessen zuerst stehen
- **P1**: Encoding-Infrastruktur + Unicode-Daten
- **P2**: Parser
- **P3**: Compiler + weitere Encodings
- **P4**: Execution Engine
- **P5**: Optionale Features (Callouts, POSIX, GNU)

---

## 12. Testplan

### 12.1 Teststrategie

Fuer jede Phase werden Tests geschrieben, die identisches Verhalten zum C-Original sicherstellen:

1. **Unit-Tests pro Modul**: Kleine Tests fuer einzelne Funktionen
2. **Compat-Tests**: 1:1-Port der C-Testdateien (test_utf8.c, test_back.c, etc.)
3. **Fuzz-Tests**: Zufaellige Patterns gegen C-Original vergleichen

### 12.2 Test-Harness (tests/common/mod.rs)

```rust
/// x2(pattern, target, from, to) - Match mit erwarteter Position
pub fn x2(pattern: &str, target: &str, from: usize, to: usize) {
    let enc = &ONIG_ENCODING_UTF8;
    let syntax = &ONIG_SYNTAX_ONIGURUMA;
    let mut reg = RegexType::new();
    let r = onig_compile(&mut reg, pattern.as_bytes(), enc, syntax, ONIG_OPTION_NONE);
    assert!(r.is_ok(), "Compile failed for pattern: {}", pattern);

    let mut region = OnigRegion::new();
    let r = onig_search(&reg, target.as_bytes(), 0, target.len(), &mut region, ONIG_OPTION_NONE);
    assert!(r.is_ok(), "Search failed for pattern: {}", pattern);
    let match_pos = r.unwrap();
    assert!(match_pos >= 0, "No match for pattern: {} against: {}", pattern, target);
    assert_eq!(region.beg[0] as usize, from, "Start mismatch");
    assert_eq!(region.end[0] as usize, to, "End mismatch");
}

/// x3(pattern, target, from, to, mem) - Match mit Capture-Gruppe
pub fn x3(pattern: &str, target: &str, from: usize, to: usize, mem: usize) { ... }

/// n(pattern, target) - Kein Match erwartet
pub fn n(pattern: &str, target: &str) { ... }

/// e(pattern) - Compile-Fehler erwartet
pub fn e(pattern: &str) { ... }
```

### 12.3 Compat-Test-Dateien

Die C-Testdateien werden mechanisch konvertiert:

```c
// C (test_utf8.c):
x2("", "", 0, 0);
x2("^", "", 0, 0);
x2("^a", "\na", 1, 2);
n(".", "\n");
x2("\\d\\d", "12", 0, 2);
```

```rust
// Rust (compat_utf8.rs):
#[test] fn utf8_001() { x2("", "", 0, 0); }
#[test] fn utf8_002() { x2("^", "", 0, 0); }
#[test] fn utf8_003() { x2("^a", "\na", 1, 2); }
#[test] fn utf8_004() { n(".", "\n"); }
#[test] fn utf8_005() { x2("\\d\\d", "12", 0, 2); }
```

### 12.4 Meilensteine

| Meilenstein | Kriterium | Geschaetzte Tests |
|-------------|-----------|-------------------|
| M1: Kompiliert | `cargo build` ohne Fehler | - |
| M2: Einfache Literale | `"abc"` matched `"abc"` | ~50 |
| M3: Zeichenklassen | `[a-z]`, `\d`, `\w` | ~100 |
| M4: Quantifier | `a*`, `a+`, `a?`, `a{2,4}` | ~200 |
| M5: Gruppen & Alternation | `(a|b)`, `(?:...)`, Captures | ~300 |
| M6: Anker | `^`, `$`, `\b`, `\A`, `\z` | ~100 |
| M7: Backreferences | `\1`, `\k<name>` | ~200 |
| M8: Lookaround | `(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)` | ~200 |
| M9: Unicode | Properties, Case-Fold, Grapheme-Cluster | ~500 |
| M10: Alle Encodings | UTF-8/16/32, EUC-JP, SJIS, ISO-8859-*, ... | ~1000 |
| M11: Compat 100% | Alle test_utf8.c Tests bestehen | ~1500 |
| M12: Callouts & RegSet | Erweiterte Features | ~200 |

---

## Anhang A: Kritische Algorithmen

### A.1 Boyer-Moore-Horspool Suche (forward_search)

In regexec.c `forward_search()` wird ein BMH-Algorithmus fuer die schnelle
Prefix-Suche verwendet. Die `map[256]` Skip-Tabelle wird in `regcomp.c`
waehrend der Optimierungsphase erstellt.

### A.2 Empty-Match-Prevention

Nullable Quantifier (`(x?)*`, `(x*)*`) koennen Endlosschleifen verursachen.
Die Loesung aus dem C-Original:
1. Compiler: `OP_EMPTY_CHECK_START(id)` vor dem Body, `OP_EMPTY_CHECK_END(id)` danach
2. Executor: Vergleicht String-Position vor/nach Body-Ausfuehrung
3. Wenn Position unveraendert -> Skip (kein weiteres Iterieren)

### A.3 Subexpression-Calls (\\g<name>)

Rekursive Patterns verwenden einen Call-Stack:
1. `OP_CALL(addr)` - Push Return-Adresse, Jump zu Pattern-Body
2. `OP_RETURN` - Pop Return-Adresse, Continue
3. `OP_MEM_END_REC` / `OP_MEM_END_PUSH_REC` - Spezielle Capture-Behandlung bei Rekursion

### A.4 Variable-Length Lookbehind

Oniguruma unterstuetzt Lookbehind mit variabler Laenge:
1. `OP_STEP_BACK_START(initial, remaining, addr)` - Initiales Zurueckgehen
2. `OP_STEP_BACK_NEXT` - Weiteres Zurueckgehen (iterativ)
3. `OP_CUT_TO_MARK` / `OP_MARK` / `OP_SAVE_VAL` / `OP_UPDATE_VAR` - Zustandsverwaltung

### A.5 Absent-Gruppen (?~...)

Negation einer Sequenz (z.B. `(?~abc)` = alles ausser "abc"):
1. Verwendet MARK/SAVE/UPDATE_VAR Opcodes
2. Schiebt `right_range` waehrend der Suche
3. Komplex, aber das C-Original hat die Logik vollstaendig

---

## Anhang B: Unicode-Daten-Generierung

Die Unicode-Datendateien (`unicode_fold_data.c`, `unicode_property_data.c`, etc.)
werden vom C-Original durch ein Tool (`mktable.c`) generiert.

Fuer Rust gibt es zwei Optionen:

**Option 1: Daten direkt portieren** (empfohlen fuer 1:1)
- Die generierten C-Arrays werden zu Rust `static` Arrays konvertiert
- Kann teilweise automatisiert werden (Python-Skript)
- Behaelt exakt die gleichen Daten wie das C-Original

**Option 2: Rust build.rs Generator**
- Port von `mktable.c` nach Rust als Build-Script
- Generiert die Daten aus Unicode-Datenbank-Dateien
- Flexibler, aber mehr Aufwand

**Empfehlung:** Option 1 fuer den initialen Port, dann spaeter optional auf
Option 2 migrieren.

Die gperf-generierten Hash-Funktionen (fold1_key, fold2_key, fold3_key, unfold_key)
koennen durch `phf` (Perfect Hash Function) Crate ersetzt werden, da die
Semantik identisch ist: statische Lookup-Tabelle mit perfektem Hash.

---

## Anhang C: Unterschiede C vs. Rust die unvermeidbar sind

| Aspekt | C-Original | Rust-Umsetzung | Grund |
|--------|-----------|----------------|-------|
| Memory | malloc/free | Box/Vec (RAII) | Rust Ownership |
| Pointer | Raw pointers | Indices (usize) | Borrow Checker |
| Union | Untagged union | Enum (tagged) | Type Safety |
| goto | goto label | loop+break / ? | Kein goto in Rust |
| Alloca | Stack-Allokation | Vec (Heap) | Kein alloca in safe Rust |
| Direct Threading | goto *label | match (switch) | Kein computed goto |
| Global State | static vars | Thread-local / Mutex | Thread Safety |
| String Pool | Raw bytes | Vec<u8> | Automatic cleanup |
| Callback | Function pointers | Fn trait / closures | Idiomatic Rust |
| Error Return | Negative int | Result<T, E> | Idiomatic Rust |

**Wichtig:** Diese Unterschiede aendern NICHT die Algorithmen oder die Semantik.
Sie sind rein technische Anpassungen an die Rust-Sprache. Die Logik in jeder
Funktion bleibt identisch zum C-Original.
