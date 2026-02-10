// oniguruma.rs - Port of oniguruma.h
// Public types, option flags, error codes, syntax flags, structs.

// === Version ===
pub const ONIGURUMA_VERSION_MAJOR: i32 = 6;
pub const ONIGURUMA_VERSION_MINOR: i32 = 9;
pub const ONIGURUMA_VERSION_TEENY: i32 = 10;
pub const ONIGURUMA_VERSION_INT: i32 = 60910;

// === Grundtypen ===
pub type OnigCodePoint = u32;
pub type OnigUChar = u8;
pub type OnigCtype = u32;
pub type OnigLen = u32;
pub type OnigCaseFoldType = u32;
pub type OnigOptionType = u32;

// === Konstanten ===
pub const ONIG_INFINITE_DISTANCE: OnigLen = OnigLen::MAX;

// === Case Fold Flags ===
pub const ONIGENC_CASE_FOLD_ASCII_ONLY: OnigCaseFoldType = 1;
pub const ONIGENC_CASE_FOLD_TURKISH_AZERI: OnigCaseFoldType = 1 << 20;
pub const INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR: OnigCaseFoldType = 1 << 30;
pub const ONIGENC_CASE_FOLD_MIN: OnigCaseFoldType = INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR;

// === Work Size ===
pub const ONIGENC_MAX_COMP_CASE_FOLD_CODE_LEN: usize = 3;
pub const ONIGENC_GET_CASE_FOLD_CODES_MAX_NUM: usize = 13;
pub const ONIGENC_CODE_TO_MBC_MAXLEN: usize = 7;
pub const ONIGENC_MBC_CASE_FOLD_MAXLEN: usize = 18;

// === Character Types ===
pub const ONIGENC_CTYPE_NEWLINE: u32 = 0;
pub const ONIGENC_CTYPE_ALPHA: u32 = 1;
pub const ONIGENC_CTYPE_BLANK: u32 = 2;
pub const ONIGENC_CTYPE_CNTRL: u32 = 3;
pub const ONIGENC_CTYPE_DIGIT: u32 = 4;
pub const ONIGENC_CTYPE_GRAPH: u32 = 5;
pub const ONIGENC_CTYPE_LOWER: u32 = 6;
pub const ONIGENC_CTYPE_PRINT: u32 = 7;
pub const ONIGENC_CTYPE_PUNCT: u32 = 8;
pub const ONIGENC_CTYPE_SPACE: u32 = 9;
pub const ONIGENC_CTYPE_UPPER: u32 = 10;
pub const ONIGENC_CTYPE_XDIGIT: u32 = 11;
pub const ONIGENC_CTYPE_WORD: u32 = 12;
pub const ONIGENC_CTYPE_ALNUM: u32 = 13;
pub const ONIGENC_CTYPE_ASCII: u32 = 14;
pub const ONIGENC_MAX_STD_CTYPE: u32 = ONIGENC_CTYPE_ASCII;

// === Case Fold Code Item ===
#[derive(Clone, Debug)]
pub struct OnigCaseFoldCodeItem {
    pub byte_len: i32,
    pub code_len: i32,
    pub code: [OnigCodePoint; ONIGENC_MAX_COMP_CASE_FOLD_CODE_LEN],
}

// === Meta Char Table ===
#[derive(Clone, Debug)]
pub struct OnigMetaCharTableType {
    pub esc: OnigCodePoint,
    pub anychar: OnigCodePoint,
    pub anytime: OnigCodePoint,
    pub zero_or_one_time: OnigCodePoint,
    pub one_or_more_time: OnigCodePoint,
    pub anychar_anytime: OnigCodePoint,
}

// === Config Parameters ===
pub const ONIG_NREGION: usize = 10;
pub const ONIG_MAX_CAPTURE_NUM: i32 = 2147483647;
pub const ONIG_MAX_BACKREF_NUM: i32 = 1000;
pub const ONIG_MAX_REPEAT_NUM: i32 = 100000;
pub const ONIG_MAX_MULTI_BYTE_RANGES_NUM: i32 = 10000;
pub const ONIG_MAX_ERROR_MESSAGE_LEN: usize = 90;

// === Option Flags ===
pub const ONIG_OPTION_DEFAULT: OnigOptionType = ONIG_OPTION_NONE;
pub const ONIG_OPTION_NONE: OnigOptionType = 0;
// compile time
pub const ONIG_OPTION_IGNORECASE: OnigOptionType = 1;
pub const ONIG_OPTION_EXTEND: OnigOptionType = ONIG_OPTION_IGNORECASE << 1;
pub const ONIG_OPTION_MULTILINE: OnigOptionType = ONIG_OPTION_EXTEND << 1;
pub const ONIG_OPTION_SINGLELINE: OnigOptionType = ONIG_OPTION_MULTILINE << 1;
pub const ONIG_OPTION_FIND_LONGEST: OnigOptionType = ONIG_OPTION_SINGLELINE << 1;
pub const ONIG_OPTION_FIND_NOT_EMPTY: OnigOptionType = ONIG_OPTION_FIND_LONGEST << 1;
pub const ONIG_OPTION_NEGATE_SINGLELINE: OnigOptionType = ONIG_OPTION_FIND_NOT_EMPTY << 1;
pub const ONIG_OPTION_DONT_CAPTURE_GROUP: OnigOptionType = ONIG_OPTION_NEGATE_SINGLELINE << 1;
pub const ONIG_OPTION_CAPTURE_GROUP: OnigOptionType = ONIG_OPTION_DONT_CAPTURE_GROUP << 1;
// search time
pub const ONIG_OPTION_NOTBOL: OnigOptionType = ONIG_OPTION_CAPTURE_GROUP << 1;
pub const ONIG_OPTION_NOTEOL: OnigOptionType = ONIG_OPTION_NOTBOL << 1;
pub const ONIG_OPTION_POSIX_REGION: OnigOptionType = ONIG_OPTION_NOTEOL << 1;
pub const ONIG_OPTION_CHECK_VALIDITY_OF_STRING: OnigOptionType = ONIG_OPTION_POSIX_REGION << 1;
// compile time (continued, gap of 3 bits)
pub const ONIG_OPTION_IGNORECASE_IS_ASCII: OnigOptionType =
    ONIG_OPTION_CHECK_VALIDITY_OF_STRING << 3;
pub const ONIG_OPTION_WORD_IS_ASCII: OnigOptionType = ONIG_OPTION_IGNORECASE_IS_ASCII << 1;
pub const ONIG_OPTION_DIGIT_IS_ASCII: OnigOptionType = ONIG_OPTION_WORD_IS_ASCII << 1;
pub const ONIG_OPTION_SPACE_IS_ASCII: OnigOptionType = ONIG_OPTION_DIGIT_IS_ASCII << 1;
pub const ONIG_OPTION_POSIX_IS_ASCII: OnigOptionType = ONIG_OPTION_SPACE_IS_ASCII << 1;
pub const ONIG_OPTION_TEXT_SEGMENT_EXTENDED_GRAPHEME_CLUSTER: OnigOptionType =
    ONIG_OPTION_POSIX_IS_ASCII << 1;
pub const ONIG_OPTION_TEXT_SEGMENT_WORD: OnigOptionType =
    ONIG_OPTION_TEXT_SEGMENT_EXTENDED_GRAPHEME_CLUSTER << 1;
// search time (continued)
pub const ONIG_OPTION_NOT_BEGIN_STRING: OnigOptionType = ONIG_OPTION_TEXT_SEGMENT_WORD << 1;
pub const ONIG_OPTION_NOT_END_STRING: OnigOptionType = ONIG_OPTION_NOT_BEGIN_STRING << 1;
pub const ONIG_OPTION_NOT_BEGIN_POSITION: OnigOptionType = ONIG_OPTION_NOT_END_STRING << 1;
pub const ONIG_OPTION_CALLBACK_EACH_MATCH: OnigOptionType = ONIG_OPTION_NOT_BEGIN_POSITION << 1;
pub const ONIG_OPTION_MATCH_WHOLE_STRING: OnigOptionType = ONIG_OPTION_CALLBACK_EACH_MATCH << 1;

pub const ONIG_OPTION_MAXBIT: OnigOptionType = ONIG_OPTION_MATCH_WHOLE_STRING;

#[inline]
pub fn onig_option_on(options: &mut OnigOptionType, regopt: OnigOptionType) {
    *options |= regopt;
}

#[inline]
pub fn onig_option_off(options: &mut OnigOptionType, regopt: OnigOptionType) {
    *options &= !regopt;
}

#[inline]
pub fn onig_is_option_on(options: OnigOptionType, option: OnigOptionType) -> bool {
    (options & option) != 0
}

// === Syntax Type ===
#[derive(Clone, Debug)]
pub struct OnigSyntaxType {
    pub op: u32,
    pub op2: u32,
    pub behavior: u32,
    pub options: OnigOptionType,
    pub meta_char_table: OnigMetaCharTableType,
}

// === Syntax Operator Flags (op) ===
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
pub const ONIG_SYN_OP_ESC_AZ_BUF_ANCHOR: u32 = 1 << 14;
pub const ONIG_SYN_OP_ESC_CAPITAL_G_BEGIN_ANCHOR: u32 = 1 << 15;
pub const ONIG_SYN_OP_DECIMAL_BACKREF: u32 = 1 << 16;
pub const ONIG_SYN_OP_BRACKET_CC: u32 = 1 << 17;
pub const ONIG_SYN_OP_ESC_W_WORD: u32 = 1 << 18;
pub const ONIG_SYN_OP_ESC_LTGT_WORD_BEGIN_END: u32 = 1 << 19;
pub const ONIG_SYN_OP_ESC_B_WORD_BOUND: u32 = 1 << 20;
pub const ONIG_SYN_OP_ESC_S_WHITE_SPACE: u32 = 1 << 21;
pub const ONIG_SYN_OP_ESC_D_DIGIT: u32 = 1 << 22;
pub const ONIG_SYN_OP_LINE_ANCHOR: u32 = 1 << 23;
pub const ONIG_SYN_OP_POSIX_BRACKET: u32 = 1 << 24;
pub const ONIG_SYN_OP_QMARK_NON_GREEDY: u32 = 1 << 25;
pub const ONIG_SYN_OP_ESC_CONTROL_CHARS: u32 = 1 << 26;
pub const ONIG_SYN_OP_ESC_C_CONTROL: u32 = 1 << 27;
pub const ONIG_SYN_OP_ESC_OCTAL3: u32 = 1 << 28;
pub const ONIG_SYN_OP_ESC_X_HEX2: u32 = 1 << 29;
pub const ONIG_SYN_OP_ESC_X_BRACE_HEX8: u32 = 1 << 30;
pub const ONIG_SYN_OP_ESC_O_BRACE_OCTAL: u32 = 1 << 31;

// === Syntax Operator Flags (op2) ===
pub const ONIG_SYN_OP2_ESC_CAPITAL_Q_QUOTE: u32 = 1 << 0;
pub const ONIG_SYN_OP2_QMARK_GROUP_EFFECT: u32 = 1 << 1;
pub const ONIG_SYN_OP2_OPTION_PERL: u32 = 1 << 2;
pub const ONIG_SYN_OP2_OPTION_RUBY: u32 = 1 << 3;
pub const ONIG_SYN_OP2_PLUS_POSSESSIVE_REPEAT: u32 = 1 << 4;
pub const ONIG_SYN_OP2_PLUS_POSSESSIVE_INTERVAL: u32 = 1 << 5;
pub const ONIG_SYN_OP2_CCLASS_SET_OP: u32 = 1 << 6;
pub const ONIG_SYN_OP2_QMARK_LT_NAMED_GROUP: u32 = 1 << 7;
pub const ONIG_SYN_OP2_ESC_K_NAMED_BACKREF: u32 = 1 << 8;
pub const ONIG_SYN_OP2_ESC_G_SUBEXP_CALL: u32 = 1 << 9;
pub const ONIG_SYN_OP2_ATMARK_CAPTURE_HISTORY: u32 = 1 << 10;
pub const ONIG_SYN_OP2_ESC_CAPITAL_C_BAR_CONTROL: u32 = 1 << 11;
pub const ONIG_SYN_OP2_ESC_CAPITAL_M_BAR_META: u32 = 1 << 12;
pub const ONIG_SYN_OP2_ESC_V_VTAB: u32 = 1 << 13;
pub const ONIG_SYN_OP2_ESC_U_HEX4: u32 = 1 << 14;
pub const ONIG_SYN_OP2_ESC_GNU_BUF_ANCHOR: u32 = 1 << 15;
pub const ONIG_SYN_OP2_ESC_P_BRACE_CHAR_PROPERTY: u32 = 1 << 16;
pub const ONIG_SYN_OP2_ESC_P_BRACE_CIRCUMFLEX_NOT: u32 = 1 << 17;
pub const ONIG_SYN_OP2_ESC_H_XDIGIT: u32 = 1 << 19;
pub const ONIG_SYN_OP2_INEFFECTIVE_ESCAPE: u32 = 1 << 20;
pub const ONIG_SYN_OP2_QMARK_LPAREN_IF_ELSE: u32 = 1 << 21;
pub const ONIG_SYN_OP2_ESC_CAPITAL_K_KEEP: u32 = 1 << 22;
pub const ONIG_SYN_OP2_ESC_CAPITAL_R_GENERAL_NEWLINE: u32 = 1 << 23;
pub const ONIG_SYN_OP2_ESC_CAPITAL_N_O_SUPER_DOT: u32 = 1 << 24;
pub const ONIG_SYN_OP2_QMARK_TILDE_ABSENT_GROUP: u32 = 1 << 25;
pub const ONIG_SYN_OP2_ESC_X_Y_TEXT_SEGMENT: u32 = 1 << 26;
pub const ONIG_SYN_OP2_QMARK_PERL_SUBEXP_CALL: u32 = 1 << 27;
pub const ONIG_SYN_OP2_QMARK_BRACE_CALLOUT_CONTENTS: u32 = 1 << 28;
pub const ONIG_SYN_OP2_ASTERISK_CALLOUT_NAME: u32 = 1 << 29;
pub const ONIG_SYN_OP2_OPTION_ONIGURUMA: u32 = 1 << 30;
pub const ONIG_SYN_OP2_QMARK_CAPITAL_P_NAME: u32 = 1 << 31;

// === Syntax Behavior Flags ===
pub const ONIG_SYN_CONTEXT_INDEP_REPEAT_OPS: u32 = 1 << 0;
pub const ONIG_SYN_CONTEXT_INVALID_REPEAT_OPS: u32 = 1 << 1;
pub const ONIG_SYN_ALLOW_UNMATCHED_CLOSE_SUBEXP: u32 = 1 << 2;
pub const ONIG_SYN_ALLOW_INVALID_INTERVAL: u32 = 1 << 3;
pub const ONIG_SYN_ALLOW_INTERVAL_LOW_ABBREV: u32 = 1 << 4;
pub const ONIG_SYN_STRICT_CHECK_BACKREF: u32 = 1 << 5;
pub const ONIG_SYN_DIFFERENT_LEN_ALT_LOOK_BEHIND: u32 = 1 << 6;
pub const ONIG_SYN_CAPTURE_ONLY_NAMED_GROUP: u32 = 1 << 7;
pub const ONIG_SYN_ALLOW_MULTIPLEX_DEFINITION_NAME: u32 = 1 << 8;
pub const ONIG_SYN_FIXED_INTERVAL_IS_GREEDY_ONLY: u32 = 1 << 9;
pub const ONIG_SYN_ISOLATED_OPTION_CONTINUE_BRANCH: u32 = 1 << 10;
pub const ONIG_SYN_VARIABLE_LEN_LOOK_BEHIND: u32 = 1 << 11;
pub const ONIG_SYN_PYTHON: u32 = 1 << 12;
pub const ONIG_SYN_WHOLE_OPTIONS: u32 = 1 << 13;
pub const ONIG_SYN_BRE_ANCHOR_AT_EDGE_OF_SUBEXP: u32 = 1 << 14;
pub const ONIG_SYN_ESC_P_WITH_ONE_CHAR_PROP: u32 = 1 << 15;
// in char class [...]
pub const ONIG_SYN_NOT_NEWLINE_IN_NEGATIVE_CC: u32 = 1 << 20;
pub const ONIG_SYN_BACKSLASH_ESCAPE_IN_CC: u32 = 1 << 21;
pub const ONIG_SYN_ALLOW_EMPTY_RANGE_IN_CC: u32 = 1 << 22;
pub const ONIG_SYN_ALLOW_DOUBLE_RANGE_OP_IN_CC: u32 = 1 << 23;
pub const ONIG_SYN_WARN_CC_OP_NOT_ESCAPED: u32 = 1 << 24;
pub const ONIG_SYN_WARN_REDUNDANT_NESTED_REPEAT: u32 = 1 << 25;
pub const ONIG_SYN_ALLOW_INVALID_CODE_END_OF_RANGE_IN_CC: u32 = 1 << 26;
pub const ONIG_SYN_ALLOW_CHAR_TYPE_FOLLOWED_BY_MINUS_IN_CC: u32 = 1 << 27;
pub const ONIG_SYN_CONTEXT_INDEP_ANCHORS: u32 = 1 << 31;

// === Meta Char Specifiers ===
pub const ONIG_META_CHAR_ESCAPE: u32 = 0;
pub const ONIG_META_CHAR_ANYCHAR: u32 = 1;
pub const ONIG_META_CHAR_ANYTIME: u32 = 2;
pub const ONIG_META_CHAR_ZERO_OR_ONE_TIME: u32 = 3;
pub const ONIG_META_CHAR_ONE_OR_MORE_TIME: u32 = 4;
pub const ONIG_META_CHAR_ANYCHAR_ANYTIME: u32 = 5;
pub const ONIG_INEFFECTIVE_META_CHAR: OnigCodePoint = 0;

// === Error Codes ===
// normal return
pub const ONIG_NORMAL: i32 = 0;
pub const ONIG_VALUE_IS_NOT_SET: i32 = 1;
pub const ONIG_MISMATCH: i32 = -1;
pub const ONIG_NO_SUPPORT_CONFIG: i32 = -2;
pub const ONIG_ABORT: i32 = -3;

// internal error
pub const ONIGERR_MEMORY: i32 = -5;
pub const ONIGERR_TYPE_BUG: i32 = -6;
pub const ONIGERR_PARSER_BUG: i32 = -11;
pub const ONIGERR_STACK_BUG: i32 = -12;
pub const ONIGERR_UNDEFINED_BYTECODE: i32 = -13;
pub const ONIGERR_UNEXPECTED_BYTECODE: i32 = -14;
pub const ONIGERR_MATCH_STACK_LIMIT_OVER: i32 = -15;
pub const ONIGERR_PARSE_DEPTH_LIMIT_OVER: i32 = -16;
pub const ONIGERR_RETRY_LIMIT_IN_MATCH_OVER: i32 = -17;
pub const ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER: i32 = -18;
pub const ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER: i32 = -19;
pub const ONIGERR_TIME_LIMIT_OVER: i32 = -20;
pub const ONIGERR_DEFAULT_ENCODING_IS_NOT_SET: i32 = -21;
pub const ONIGERR_SPECIFIED_ENCODING_CANT_CONVERT_TO_WIDE_CHAR: i32 = -22;
pub const ONIGERR_FAIL_TO_INITIALIZE: i32 = -23;

// general error
pub const ONIGERR_INVALID_ARGUMENT: i32 = -30;

// syntax error
pub const ONIGERR_END_PATTERN_AT_LEFT_BRACE: i32 = -100;
pub const ONIGERR_END_PATTERN_AT_LEFT_BRACKET: i32 = -101;
pub const ONIGERR_EMPTY_CHAR_CLASS: i32 = -102;
pub const ONIGERR_PREMATURE_END_OF_CHAR_CLASS: i32 = -103;
pub const ONIGERR_END_PATTERN_AT_ESCAPE: i32 = -104;
pub const ONIGERR_END_PATTERN_AT_META: i32 = -105;
pub const ONIGERR_END_PATTERN_AT_CONTROL: i32 = -106;
pub const ONIGERR_META_CODE_SYNTAX: i32 = -108;
pub const ONIGERR_CONTROL_CODE_SYNTAX: i32 = -109;
pub const ONIGERR_CHAR_CLASS_VALUE_AT_END_OF_RANGE: i32 = -110;
pub const ONIGERR_CHAR_CLASS_VALUE_AT_START_OF_RANGE: i32 = -111;
pub const ONIGERR_UNMATCHED_RANGE_SPECIFIER_IN_CHAR_CLASS: i32 = -112;
pub const ONIGERR_TARGET_OF_REPEAT_OPERATOR_NOT_SPECIFIED: i32 = -113;
pub const ONIGERR_TARGET_OF_REPEAT_OPERATOR_INVALID: i32 = -114;
pub const ONIGERR_NESTED_REPEAT_OPERATOR: i32 = -115;
pub const ONIGERR_UNMATCHED_CLOSE_PARENTHESIS: i32 = -116;
pub const ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS: i32 = -117;
pub const ONIGERR_END_PATTERN_IN_GROUP: i32 = -118;
pub const ONIGERR_UNDEFINED_GROUP_OPTION: i32 = -119;
pub const ONIGERR_INVALID_GROUP_OPTION: i32 = -120;
pub const ONIGERR_INVALID_POSIX_BRACKET_TYPE: i32 = -121;
pub const ONIGERR_INVALID_LOOK_BEHIND_PATTERN: i32 = -122;
pub const ONIGERR_INVALID_REPEAT_RANGE_PATTERN: i32 = -123;

// values error (syntax error)
pub const ONIGERR_TOO_BIG_NUMBER: i32 = -200;
pub const ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE: i32 = -201;
pub const ONIGERR_UPPER_SMALLER_THAN_LOWER_IN_REPEAT_RANGE: i32 = -202;
pub const ONIGERR_EMPTY_RANGE_IN_CHAR_CLASS: i32 = -203;
pub const ONIGERR_MISMATCH_CODE_LENGTH_IN_CLASS_RANGE: i32 = -204;
pub const ONIGERR_TOO_MANY_MULTI_BYTE_RANGES: i32 = -205;
pub const ONIGERR_TOO_SHORT_MULTI_BYTE_STRING: i32 = -206;
pub const ONIGERR_TOO_BIG_BACKREF_NUMBER: i32 = -207;
pub const ONIGERR_INVALID_BACKREF: i32 = -208;
pub const ONIGERR_NUMBERED_BACKREF_OR_CALL_NOT_ALLOWED: i32 = -209;
pub const ONIGERR_TOO_MANY_CAPTURES: i32 = -210;
pub const ONIGERR_TOO_LONG_WIDE_CHAR_VALUE: i32 = -212;
pub const ONIGERR_UNDEFINED_OPERATOR: i32 = -213;
pub const ONIGERR_EMPTY_GROUP_NAME: i32 = -214;
pub const ONIGERR_INVALID_GROUP_NAME: i32 = -215;
pub const ONIGERR_INVALID_CHAR_IN_GROUP_NAME: i32 = -216;
pub const ONIGERR_UNDEFINED_NAME_REFERENCE: i32 = -217;
pub const ONIGERR_UNDEFINED_GROUP_REFERENCE: i32 = -218;
pub const ONIGERR_MULTIPLEX_DEFINED_NAME: i32 = -219;
pub const ONIGERR_MULTIPLEX_DEFINITION_NAME_CALL: i32 = -220;
pub const ONIGERR_NEVER_ENDING_RECURSION: i32 = -221;
pub const ONIGERR_GROUP_NUMBER_OVER_FOR_CAPTURE_HISTORY: i32 = -222;
pub const ONIGERR_INVALID_CHAR_PROPERTY_NAME: i32 = -223;
pub const ONIGERR_INVALID_IF_ELSE_SYNTAX: i32 = -224;
pub const ONIGERR_INVALID_ABSENT_GROUP_PATTERN: i32 = -225;
pub const ONIGERR_INVALID_ABSENT_GROUP_GENERATOR_PATTERN: i32 = -226;
pub const ONIGERR_INVALID_CALLOUT_PATTERN: i32 = -227;
pub const ONIGERR_INVALID_CALLOUT_NAME: i32 = -228;
pub const ONIGERR_UNDEFINED_CALLOUT_NAME: i32 = -229;
pub const ONIGERR_INVALID_CALLOUT_BODY: i32 = -230;
pub const ONIGERR_INVALID_CALLOUT_TAG_NAME: i32 = -231;
pub const ONIGERR_INVALID_CALLOUT_ARG: i32 = -232;
pub const ONIGERR_INVALID_CODE_POINT_VALUE: i32 = -400;
pub const ONIGERR_INVALID_WIDE_CHAR_VALUE: i32 = -400;
pub const ONIGERR_TOO_BIG_WIDE_CHAR_VALUE: i32 = -401;
pub const ONIGERR_NOT_SUPPORTED_ENCODING_COMBINATION: i32 = -402;
pub const ONIGERR_INVALID_COMBINATION_OF_OPTIONS: i32 = -403;
pub const ONIGERR_TOO_MANY_USER_DEFINED_OBJECTS: i32 = -404;
pub const ONIGERR_TOO_LONG_PROPERTY_NAME: i32 = -405;
pub const ONIGERR_VERY_INEFFICIENT_PATTERN: i32 = -406;
pub const ONIGERR_LIBRARY_IS_NOT_INITIALIZED: i32 = -500;

#[inline]
pub fn onig_is_pattern_error(ecode: i32) -> bool {
    ecode <= -100 && ecode > -1000
}

// === Capture History ===
pub const ONIG_MAX_CAPTURE_HISTORY_GROUP: usize = 31;

// === Capture Tree Node ===
pub struct OnigCaptureTreeNode {
    pub group: i32,
    pub beg: i32,
    pub end: i32,
    pub childs: Vec<Box<OnigCaptureTreeNode>>,
}

// === OnigRegion (match result) ===
pub struct OnigRegion {
    pub allocated: i32,
    pub num_regs: i32,
    pub beg: Vec<i32>,
    pub end: Vec<i32>,
    pub history_root: Option<Box<OnigCaptureTreeNode>>,
}

impl OnigRegion {
    pub fn new() -> Self {
        OnigRegion {
            allocated: 0,
            num_regs: 0,
            beg: Vec::new(),
            end: Vec::new(),
            history_root: None,
        }
    }

    pub fn init(&mut self) {
        self.allocated = 0;
        self.num_regs = 0;
        self.beg.clear();
        self.end.clear();
        self.history_root = None;
    }

    pub fn clear(&mut self) {
        for i in 0..self.num_regs as usize {
            self.beg[i] = ONIG_REGION_NOTPOS;
            self.end[i] = ONIG_REGION_NOTPOS;
        }
        self.history_root = None;
    }

    pub fn resize(&mut self, n: i32) {
        let n = n as usize;
        self.beg.resize(n, ONIG_REGION_NOTPOS);
        self.end.resize(n, ONIG_REGION_NOTPOS);
        self.allocated = n as i32;
        self.num_regs = n as i32;
    }
}

impl Default for OnigRegion {
    fn default() -> Self {
        Self::new()
    }
}

// === Capture Traverse Constants ===
pub const ONIG_TRAVERSE_CALLBACK_AT_FIRST: i32 = 1;
pub const ONIG_TRAVERSE_CALLBACK_AT_LAST: i32 = 2;
pub const ONIG_TRAVERSE_CALLBACK_AT_BOTH: i32 =
    ONIG_TRAVERSE_CALLBACK_AT_FIRST | ONIG_TRAVERSE_CALLBACK_AT_LAST;

pub const ONIG_REGION_NOTPOS: i32 = -1;

// === Error Info ===
pub struct OnigErrorInfo {
    pub par: Vec<u8>,
}

// === Repeat Range ===
#[derive(Clone, Debug)]
pub struct OnigRepeatRange {
    pub lower: i32,
    pub upper: i32,
}

// === Char Table Size ===
pub const ONIG_CHAR_TABLE_SIZE: usize = 256;

// === RegSet Lead ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum OnigRegSetLead {
    PositionLead = 0,
    RegexLead = 1,
    PriorityToRegexOrder = 2,
}

// === Compile Info ===
pub struct OnigCompileInfo {
    pub num_of_elements: i32,
    pub syntax: *const OnigSyntaxType,
    pub option: OnigOptionType,
    pub case_fold_flag: OnigCaseFoldType,
}

// === Callout Types ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum OnigCalloutIn {
    Progress = 1,
    Retraction = 2,
}

pub const ONIG_CALLOUT_IN_BOTH: i32 =
    OnigCalloutIn::Progress as i32 | OnigCalloutIn::Retraction as i32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum OnigCalloutOf {
    Contents = 0,
    Name = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum OnigCalloutType {
    Single = 0,
    StartCall = 1,
    BothCall = 2,
    StartMarkEndCall = 3,
}

pub const ONIG_NON_NAME_ID: i32 = -1;
pub const ONIG_NON_CALLOUT_NUM: i32 = 0;
pub const ONIG_CALLOUT_MAX_ARGS_NUM: usize = 4;
pub const ONIG_CALLOUT_DATA_SLOT_NUM: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum OnigCalloutResult {
    Fail = 1,
    Success = 0,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum OnigType {
    Void = 0,
    Long = 1 << 0,
    Char = 1 << 1,
    String = 1 << 2,
    Pointer = 1 << 3,
    Tag = 1 << 4,
}

// === OnigValue (C union -> Rust enum) ===
#[derive(Clone, Debug)]
pub enum OnigValue {
    Long(i64),
    Char(OnigCodePoint),
    String { start: Vec<u8> },
    Tag(i32),
    Void,
}

// === Pair Case Fold Codes (from regenc.h) ===
#[derive(Clone, Copy, Debug)]
pub struct OnigPairCaseFoldCodes {
    pub from: OnigCodePoint,
    pub to: OnigCodePoint,
}

// === Code Range ===
#[derive(Clone, Debug)]
pub struct OnigCodeRange {
    pub from: OnigCodePoint,
    pub to: OnigCodePoint,
}

// === Syntax check macros as functions ===
#[inline]
pub fn is_syntax_op(syntax: &OnigSyntaxType, opm: u32) -> bool {
    (syntax.op & opm) != 0
}

#[inline]
pub fn is_syntax_op2(syntax: &OnigSyntaxType, opm: u32) -> bool {
    (syntax.op2 & opm) != 0
}

#[inline]
pub fn is_syntax_bv(syntax: &OnigSyntaxType, bvm: u32) -> bool {
    (syntax.behavior & bvm) != 0
}
