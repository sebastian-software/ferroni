// regint.rs - Port of regint.h
// Internal types, OpCode, Operation, BitSet, MemStatus, regex_t.

use std::collections::HashMap;

use crate::oniguruma::*;
use crate::regenc::OnigEncoding;

// === Feature Flags (C #define USE_*) ===
pub const USE_CALL: bool = true;
pub const USE_CALLOUT: bool = true;
pub const USE_BACKREF_WITH_LEVEL: bool = true;
pub const USE_CAPTURE_HISTORY: bool = true;

// === Config Constants ===
pub const DEFAULT_PARSE_DEPTH_LIMIT: u32 = 4096;
pub const INIT_MATCH_STACK_SIZE: usize = 160;
pub const DEFAULT_MATCH_STACK_LIMIT_SIZE: u32 = 0;
pub const DEFAULT_RETRY_LIMIT_IN_MATCH: u64 = 10_000_000;
pub const DEFAULT_RETRY_LIMIT_IN_SEARCH: u64 = 0;
pub const DEFAULT_TIME_LIMIT_MSEC: u64 = 0;
pub const DEFAULT_SUBEXP_CALL_LIMIT_IN_SEARCH: u64 = 0;
pub const DEFAULT_SUBEXP_CALL_MAX_NEST_LEVEL: i32 = 20;

// === Internal Constants ===
pub const CHAR_MAP_SIZE: usize = 256;
pub const INFINITE_LEN: OnigLen = ONIG_INFINITE_DISTANCE;
pub const STEP_BACK_MAX_CHAR_LEN: i32 = 65535;
pub const LOOK_BEHIND_MAX_CHAR_LEN: i32 = STEP_BACK_MAX_CHAR_LEN;
pub const INFINITE_REPEAT: i32 = -1;

#[inline]
pub fn is_infinite_repeat(n: i32) -> bool {
    n == INFINITE_REPEAT
}

// === Bytecode Types ===
pub type RelAddrType = i32;
pub type AbsAddrType = i32;
pub type LengthType = i32;
pub type RelPositionType = i32;
pub type RepeatNumType = i32;
pub type MemNumType = i32;
pub type ModeType = i32;

// === MemStatus (bit status for captures) ===
pub type MemStatusType = u32;

pub const MEM_STATUS_BITS_NUM: usize = 32;

#[inline]
pub fn mem_status_clear(stats: &mut MemStatusType) {
    *stats = 0;
}

#[inline]
pub fn mem_status_on_all(stats: &mut MemStatusType) {
    *stats = !0u32;
}

#[inline]
pub fn mem_status_at(stats: MemStatusType, n: usize) -> bool {
    if n < MEM_STATUS_BITS_NUM {
        (stats & (1u32 << n)) != 0
    } else {
        (stats & 1) != 0
    }
}

#[inline]
pub fn mem_status_at0(stats: MemStatusType, n: usize) -> bool {
    if n > 0 && n < MEM_STATUS_BITS_NUM {
        (stats & (1u32 << n)) != 0
    } else {
        (stats & 1) != 0
    }
}

#[inline]
pub fn mem_status_is_all_on(stats: MemStatusType) -> bool {
    (stats & 1) != 0
}

#[inline]
pub fn mem_status_on(stats: &mut MemStatusType, n: usize) {
    if n < MEM_STATUS_BITS_NUM {
        if n != 0 {
            *stats |= 1u32 << n;
        }
    } else {
        *stats |= 1;
    }
}

#[inline]
pub fn mem_status_on_simple(stats: &mut MemStatusType, n: usize) {
    if n < MEM_STATUS_BITS_NUM {
        *stats |= 1u32 << n;
    }
}

#[inline]
pub fn mem_status_limit_at(stats: MemStatusType, n: usize) -> bool {
    if n < MEM_STATUS_BITS_NUM {
        (stats & (1u32 << n)) != 0
    } else {
        false
    }
}

#[inline]
pub fn mem_status_limit_on(stats: &mut MemStatusType, n: usize) {
    if n < MEM_STATUS_BITS_NUM && n != 0 {
        *stats |= 1u32 << n;
    }
}

// === BitSet (256 bits for ASCII character classes) ===
pub const BITS_PER_BYTE: usize = 8;
pub const SINGLE_BYTE_SIZE: usize = 1 << BITS_PER_BYTE;
pub const BITS_IN_ROOM: usize = 32;
pub const BITSET_REAL_SIZE: usize = SINGLE_BYTE_SIZE / BITS_IN_ROOM;
pub type Bits = u32;
pub type BitSet = [Bits; BITSET_REAL_SIZE];

pub const SIZE_BITSET: usize = std::mem::size_of::<BitSet>();

#[inline]
pub fn bitset_clear(bs: &mut BitSet) {
    for i in 0..BITSET_REAL_SIZE {
        bs[i] = 0;
    }
}

#[inline]
pub fn bs_room(pos: usize) -> usize {
    pos >> 5
}

#[inline]
pub fn bs_bit(pos: usize) -> u32 {
    1u32 << (pos & 0x1f)
}

#[inline]
pub fn bitset_at(bs: &BitSet, pos: usize) -> bool {
    (bs[bs_room(pos)] & bs_bit(pos)) != 0
}

#[inline]
pub fn bitset_set_bit(bs: &mut BitSet, pos: usize) {
    bs[bs_room(pos)] |= bs_bit(pos);
}

#[inline]
pub fn bitset_clear_bit(bs: &mut BitSet, pos: usize) {
    bs[bs_room(pos)] &= !bs_bit(pos);
}

#[inline]
pub fn bitset_invert_bit(bs: &mut BitSet, pos: usize) {
    bs[bs_room(pos)] ^= bs_bit(pos);
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

#[inline]
pub fn anchor_has_body(anchor_type: i32) -> bool {
    anchor_type < ANCR_BEGIN_BUF
}

#[inline]
pub fn is_word_anchor_type(anchor_type: i32) -> bool {
    anchor_type == ANCR_WORD_BOUNDARY
        || anchor_type == ANCR_NO_WORD_BOUNDARY
        || anchor_type == ANCR_WORD_BEGIN
        || anchor_type == ANCR_WORD_END
}

// === OpCode Enum ===
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

// === SaveType ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SaveType {
    Keep = 0,
    S = 1,
    RightRange = 2,
}

// === UpdateVarType ===
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

// === CheckPositionType ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CheckPositionType {
    SearchStart = 0,
    CurrentRightRange = 1,
}

// === TextSegmentBoundaryType ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TextSegmentBoundaryType {
    ExtendedGraphemeCluster = 0,
    Word = 1,
}

// === Stack Pop Level ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum StackPopLevel {
    Free = 0,
    MemStart = 1,
    All = 2,
}

// === Optimize Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizeType {
    None,
    Str,
    StrFast,
    StrFastStepForward,
    Map,
}

// === CClass Flags ===
pub const FLAG_NCCLASS_NOT: u32 = 1 << 0;
pub const FLAG_NCCLASS_SHARE: u32 = 1 << 1;

// === Operation (Bytecode Instruction) ===
//
// In C this is a struct with opcode + union. In Rust we use struct + enum payload.
// The opcode field is stored separately for direct access (needed for dispatch).
pub struct Operation {
    pub opcode: OpCode,
    pub payload: OperationPayload,
}

pub enum OperationPayload {
    None,
    Exact {
        s: [u8; 16],
    },
    ExactN {
        s: Vec<u8>,
        n: LengthType,
    },
    ExactLenN {
        s: Vec<u8>,
        n: LengthType,
        len: LengthType,
    },
    CClass {
        bsp: Box<BitSet>,
    },
    CClassMb {
        mb: Vec<u8>,
    },
    CClassMix {
        mb: Vec<u8>,
        bsp: Box<BitSet>,
    },
    AnyCharStarPeekNext {
        c: u8,
    },
    WordBoundary {
        mode: ModeType,
    },
    TextSegmentBoundary {
        boundary_type: TextSegmentBoundaryType,
        not: bool,
    },
    CheckPosition {
        check_type: CheckPositionType,
    },
    BackRefN {
        n1: MemNumType,
    },
    BackRefGeneral {
        num: i32,
        ns: Vec<MemNumType>,
        nest_level: i32,
    },
    MemoryStart {
        num: MemNumType,
    },
    MemoryEnd {
        num: MemNumType,
    },
    Jump {
        addr: RelAddrType,
    },
    Push {
        addr: RelAddrType,
    },
    PushOrJumpExact1 {
        addr: RelAddrType,
        c: u8,
    },
    PushIfPeekNext {
        addr: RelAddrType,
        c: u8,
    },
    PopToMark {
        id: MemNumType,
    },
    Repeat {
        id: MemNumType,
        addr: RelAddrType,
    },
    RepeatInc {
        id: MemNumType,
    },
    EmptyCheckStart {
        mem: MemNumType,
    },
    EmptyCheckEnd {
        mem: MemNumType,
        empty_status_mem: MemStatusType,
    },
    Move {
        n: RelPositionType,
    },
    StepBackStart {
        initial: LengthType,
        remaining: LengthType,
        addr: RelAddrType,
    },
    CutToMark {
        id: MemNumType,
        restore_pos: bool,
    },
    Mark {
        id: MemNumType,
        save_pos: bool,
    },
    SaveVal {
        save_type: SaveType,
        id: MemNumType,
    },
    UpdateVar {
        var_type: UpdateVarType,
        id: MemNumType,
        clear: bool,
    },
    Call {
        addr: AbsAddrType,
    },
    Return,
    CalloutContents {
        num: MemNumType,
    },
    CalloutName {
        num: MemNumType,
        id: MemNumType,
    },
}

// === CalloutListEntry ===
pub struct CalloutListEntry {
    pub flag: i32,
    pub of: OnigCalloutOf,
    pub callout_in: i32,
    pub name_id: i32,
    pub tag_start: Vec<u8>,
    pub tag_end: Vec<u8>,
    pub callout_type: OnigCalloutType,
    pub content: CalloutContent,
}

pub enum CalloutContent {
    Contents { start: Vec<u8> },
    Args {
        num: i32,
        passed_num: i32,
        types: [OnigType; ONIG_CALLOUT_MAX_ARGS_NUM],
        vals: [OnigValue; ONIG_CALLOUT_MAX_ARGS_NUM],
    },
}

// === RepeatRange ===
#[derive(Clone, Debug)]
pub struct RepeatRange {
    pub lower: i32,
    pub upper: i32,
    pub u_offset: i32,
}

// === RegexExt (Callout Extension) ===
pub struct RegexExt {
    pub pattern: Vec<u8>,
    pub tag_table: Option<HashMap<Vec<u8>, i32>>,
    pub callout_num: i32,
    pub callout_list: Vec<CalloutListEntry>,
}

// === regex_t (re_pattern_buffer) ===
pub struct RegexType {
    // bytecode
    pub ops: Vec<Operation>,
    pub string_pool: Vec<u8>,

    // capture info
    pub num_mem: i32,
    pub num_repeat: i32,
    pub num_empty_check: i32,
    pub num_call: i32,
    pub capture_history: MemStatusType,
    pub push_mem_start: MemStatusType,
    pub push_mem_end: MemStatusType,
    pub stack_pop_level: StackPopLevel,
    pub repeat_range: Vec<RepeatRange>,

    // metadata
    pub enc: OnigEncoding,
    pub options: OnigOptionType,
    pub syntax: *const OnigSyntaxType,
    pub case_fold_flag: OnigCaseFoldType,
    pub name_table: Option<HashMap<Vec<u8>, Vec<i32>>>,

    // optimization
    pub optimize: OptimizeType,
    pub threshold_len: i32,
    pub anchor: i32,
    pub anc_dist_min: OnigLen,
    pub anc_dist_max: OnigLen,
    pub sub_anchor: i32,
    pub exact: Vec<u8>,
    pub map: [u8; CHAR_MAP_SIZE],
    pub map_offset: i32,
    pub dist_min: OnigLen,
    pub dist_max: OnigLen,

    // extension (callouts)
    pub extp: Option<RegexExt>,
}

// === Option check macros as functions ===
#[inline]
pub fn opton_ignorecase(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_IGNORECASE) != 0
}

#[inline]
pub fn opton_extend(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_EXTEND) != 0
}

#[inline]
pub fn opton_multiline(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_MULTILINE) != 0
}

#[inline]
pub fn opton_singleline(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_SINGLELINE) != 0
}

#[inline]
pub fn opton_find_longest(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_FIND_LONGEST) != 0
}

#[inline]
pub fn opton_find_not_empty(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_FIND_NOT_EMPTY) != 0
}

#[inline]
pub fn opton_negate_singleline(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_NEGATE_SINGLELINE) != 0
}

#[inline]
pub fn opton_dont_capture_group(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_DONT_CAPTURE_GROUP) != 0
}

#[inline]
pub fn opton_capture_group(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_CAPTURE_GROUP) != 0
}

#[inline]
pub fn opton_notbol(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_NOTBOL) != 0
}

#[inline]
pub fn opton_noteol(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_NOTEOL) != 0
}

#[inline]
pub fn opton_posix_region(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_POSIX_REGION) != 0
}

#[inline]
pub fn opton_check_validity_of_string(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_CHECK_VALIDITY_OF_STRING) != 0
}

#[inline]
pub fn opton_callback_each_match(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_CALLBACK_EACH_MATCH) != 0
}

#[inline]
pub fn opton_not_begin_string(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_NOT_BEGIN_STRING) != 0
}

#[inline]
pub fn opton_not_end_string(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_NOT_END_STRING) != 0
}

#[inline]
pub fn opton_not_begin_position(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_NOT_BEGIN_POSITION) != 0
}

#[inline]
pub fn opton_match_whole_string(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_MATCH_WHOLE_STRING) != 0
}

// === Syntax macros ===
#[inline]
pub fn mc_esc(syn: &OnigSyntaxType) -> OnigCodePoint {
    syn.meta_char_table.esc
}

#[inline]
pub fn mc_anychar(syn: &OnigSyntaxType) -> OnigCodePoint {
    syn.meta_char_table.anychar
}

#[inline]
pub fn mc_anytime(syn: &OnigSyntaxType) -> OnigCodePoint {
    syn.meta_char_table.anytime
}

#[inline]
pub fn mc_zero_or_one_time(syn: &OnigSyntaxType) -> OnigCodePoint {
    syn.meta_char_table.zero_or_one_time
}

#[inline]
pub fn mc_one_or_more_time(syn: &OnigSyntaxType) -> OnigCodePoint {
    syn.meta_char_table.one_or_more_time
}

#[inline]
pub fn mc_anychar_anytime(syn: &OnigSyntaxType) -> OnigCodePoint {
    syn.meta_char_table.anychar_anytime
}

#[inline]
pub fn is_mc_esc_code(code: OnigCodePoint, syn: &OnigSyntaxType) -> bool {
    code == mc_esc(syn)
        && !is_syntax_op2(syn, ONIG_SYN_OP2_INEFFECTIVE_ESCAPE)
}

// === Value helpers ===
#[inline]
pub fn digitval(code: OnigCodePoint) -> u32 {
    code - b'0' as u32
}

#[inline]
pub fn odigitval(code: OnigCodePoint) -> u32 {
    digitval(code)
}

#[inline]
pub fn is_code_word_ascii(code: OnigCodePoint) -> bool {
    code < 128
}

#[inline]
pub fn is_code_digit_ascii(code: OnigCodePoint) -> bool {
    code < 128 && (code >= b'0' as u32 && code <= b'9' as u32)
}
