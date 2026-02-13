// regenc.rs - Port of regenc.h + regenc.c
// Encoding trait (from OnigEncodingType) and shared encoding utility functions.

use crate::oniguruma::*;

// === Encoding type alias ===
// In C: OnigEncoding = OnigEncodingType*
// In Rust: a trait object reference
pub type OnigEncoding = &'static dyn Encoding;

// === Encoding flags ===
pub const ENC_FLAG_ASCII_COMPATIBLE: u32 = 1 << 0;
pub const ENC_FLAG_UNICODE: u32 = 1 << 1;
pub const ENC_FLAG_SKIP_OFFSET_MASK: u32 = 7 << 2;
pub const ENC_FLAG_SKIP_OFFSET_0: u32 = 0;
pub const ENC_FLAG_SKIP_OFFSET_1: u32 = 1 << 2;
pub const ENC_FLAG_SKIP_OFFSET_2: u32 = 2 << 2;
pub const ENC_FLAG_SKIP_OFFSET_3: u32 = 3 << 2;
pub const ENC_FLAG_SKIP_OFFSET_4: u32 = 4 << 2;
pub const ENC_SKIP_OFFSET_1_OR_0: u32 = 7;
pub const ENC_FLAG_SKIP_OFFSET_1_OR_0: u32 = ENC_SKIP_OFFSET_1_OR_0 << 2;

// === Constants ===
pub const MAX_CODE_POINT: OnigCodePoint = OnigCodePoint::MAX;
pub const ASCII_LIMIT: OnigCodePoint = 127;
pub const NEWLINE_CODE: OnigCodePoint = 0x0a;

// === Encoding Trait ===
// 1:1 mapping of OnigEncodingType function pointers to trait methods.
pub trait Encoding: Send + Sync {
    /// Returns the byte length of the multibyte character at position p.
    fn mbc_enc_len(&self, p: &[u8]) -> usize;

    /// Encoding name (e.g. "US-ASCII", "UTF-8")
    fn name(&self) -> &str;

    /// Maximum encoded character length in bytes
    fn max_enc_len(&self) -> usize;

    /// Minimum encoded character length in bytes
    fn min_enc_len(&self) -> usize;

    /// Is the byte at p a newline character?
    fn is_mbc_newline(&self, p: &[u8], end: usize) -> bool;

    /// Decode a multibyte character to a code point
    fn mbc_to_code(&self, p: &[u8], end: usize) -> OnigCodePoint;

    /// Returns the byte length needed to encode a code point
    fn code_to_mbclen(&self, code: OnigCodePoint) -> i32;

    /// Encode a code point into buf, returns number of bytes written
    fn code_to_mbc(&self, code: OnigCodePoint, buf: &mut [u8]) -> i32;

    /// Case fold the character at pp, advance pp, write folded to fold_buf.
    /// Returns the number of bytes written to fold_buf.
    fn mbc_case_fold(
        &self,
        flag: OnigCaseFoldType,
        pp: &mut usize,
        end: usize,
        source: &[u8],
        fold_buf: &mut [u8],
    ) -> i32;

    /// Apply function f to all case fold pairs.
    fn apply_all_case_fold(
        &self,
        flag: OnigCaseFoldType,
        f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
    ) -> i32;

    /// Get case fold code alternatives for the character at p.
    fn get_case_fold_codes_by_str(
        &self,
        flag: OnigCaseFoldType,
        p: &[u8],
        end: usize,
        items: &mut [OnigCaseFoldCodeItem],
    ) -> i32;

    /// Convert property name to ctype value.
    fn property_name_to_ctype(&self, p: &[u8]) -> i32;

    /// Is the code point of the given ctype?
    fn is_code_ctype(&self, code: OnigCodePoint, ctype: u32) -> bool;

    /// Get the code range for a ctype.
    fn get_ctype_code_range(
        &self,
        ctype: u32,
        sb_out: &mut OnigCodePoint,
    ) -> Option<&'static [OnigCodePoint]>;

    /// Left adjust char head: find the start of the character containing s
    /// within [start..].
    fn left_adjust_char_head(&self, start: usize, s: usize, data: &[u8]) -> usize;

    /// Is reverse matching allowed at this position?
    fn is_allowed_reverse_match(&self, p: &[u8]) -> bool;

    /// Initialize encoding (for callout registration etc.)
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&self) -> i32 {
        ONIG_NORMAL
    }

    /// Is this encoding initialized?
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn is_initialized(&self) -> bool {
        true
    }

    /// Validate that the byte string is valid for this encoding
    fn is_valid_mbc_string(&self, s: &[u8]) -> bool;

    /// Encoding flags
    fn flag(&self) -> u32;

    /// Single-byte range boundary
    fn sb_range(&self) -> OnigCodePoint {
        0
    }

    /// Encoding index
    fn index(&self) -> i32 {
        0
    }
}

// === Encoding query helpers ===

#[inline]
pub fn enc_get_skip_offset(enc: OnigEncoding) -> u32 {
    (enc.flag() & ENC_FLAG_SKIP_OFFSET_MASK) >> 2
}

#[inline]
pub fn onigenc_is_unicode_encoding(enc: OnigEncoding) -> bool {
    (enc.flag() & ENC_FLAG_UNICODE) != 0
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_ascii_compatible_encoding(enc: OnigEncoding) -> bool {
    (enc.flag() & ENC_FLAG_ASCII_COMPATIBLE) != 0
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_singlebyte(enc: OnigEncoding) -> bool {
    enc.max_enc_len() == 1
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_mbc_head(enc: OnigEncoding, p: &[u8]) -> bool {
    enc.mbc_enc_len(p) != 1
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_mbc_ascii(p: &[u8]) -> bool {
    p[0] < 128
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_code_ascii(code: OnigCodePoint) -> bool {
    code < 128
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_code_word(enc: OnigEncoding, code: OnigCodePoint) -> bool {
    enc.is_code_ctype(code, ONIGENC_CTYPE_WORD)
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_code_newline(enc: OnigEncoding, code: OnigCodePoint) -> bool {
    enc.is_code_ctype(code, ONIGENC_CTYPE_NEWLINE)
}

// === Case Fold Helpers ===

#[inline]
pub fn case_fold_is_ascii_only(flag: OnigCaseFoldType) -> bool {
    (flag & ONIGENC_CASE_FOLD_ASCII_ONLY) != 0
}

#[inline]
pub fn case_fold_is_not_ascii_only(flag: OnigCaseFoldType) -> bool {
    (flag & ONIGENC_CASE_FOLD_ASCII_ONLY) == 0
}

// === Ctype bit helpers (from regenc.h) ===

pub const BIT_CTYPE_NEWLINE: u32 = 1 << ONIGENC_CTYPE_NEWLINE;
pub const BIT_CTYPE_ALPHA: u32 = 1 << ONIGENC_CTYPE_ALPHA;
pub const BIT_CTYPE_BLANK: u32 = 1 << ONIGENC_CTYPE_BLANK;
pub const BIT_CTYPE_CNTRL: u32 = 1 << ONIGENC_CTYPE_CNTRL;
pub const BIT_CTYPE_DIGIT: u32 = 1 << ONIGENC_CTYPE_DIGIT;
pub const BIT_CTYPE_GRAPH: u32 = 1 << ONIGENC_CTYPE_GRAPH;
pub const BIT_CTYPE_LOWER: u32 = 1 << ONIGENC_CTYPE_LOWER;
pub const BIT_CTYPE_PRINT: u32 = 1 << ONIGENC_CTYPE_PRINT;
pub const BIT_CTYPE_PUNCT: u32 = 1 << ONIGENC_CTYPE_PUNCT;
pub const BIT_CTYPE_SPACE: u32 = 1 << ONIGENC_CTYPE_SPACE;
pub const BIT_CTYPE_UPPER: u32 = 1 << ONIGENC_CTYPE_UPPER;
pub const BIT_CTYPE_XDIGIT: u32 = 1 << ONIGENC_CTYPE_XDIGIT;
pub const BIT_CTYPE_WORD: u32 = 1 << ONIGENC_CTYPE_WORD;
pub const BIT_CTYPE_ALNUM: u32 = 1 << ONIGENC_CTYPE_ALNUM;
pub const BIT_CTYPE_ASCII: u32 = 1 << ONIGENC_CTYPE_ASCII;

#[inline]
pub fn ctype_to_bit(ctype: u32) -> u32 {
    1 << ctype
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn ctype_is_word_graph_print(ctype: u32) -> bool {
    ctype == ONIGENC_CTYPE_WORD || ctype == ONIGENC_CTYPE_GRAPH || ctype == ONIGENC_CTYPE_PRINT
}

// === ASCII Tables (from regenc.c) ===

pub static ONIG_ENC_ASCII_TO_LOWER_CASE_TABLE: [u8; 256] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
    0x40, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, // A-G -> a-g
    0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, // H-O -> h-o
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, // P-W -> p-w
    0x78, 0x79, 0x7a, 0x5b, 0x5c, 0x5d, 0x5e, 0x5f, // X-Z -> x-z, then [\]^_
    0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f,
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f,
    0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f,
    0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f,
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
    0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf,
    0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xcb, 0xcc, 0xcd, 0xce, 0xcf,
    0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xdb, 0xdc, 0xdd, 0xde, 0xdf,
    0xe0, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xeb, 0xec, 0xed, 0xee, 0xef,
    0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff,
];

pub static ONIG_ENC_ASCII_CTYPE_TABLE: [u16; 256] = [
    0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x420c, 0x420c, 0x4209, 0x4208,
    0x4208, 0x4208, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008,
    0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4284, 0x41a0, 0x41a0, 0x41a0,
    0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0,
    0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x41a0, 0x41a0,
    0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x74a2,
    0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2,
    0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x51a0,
    0x41a0, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2,
    0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2,
    0x70e2, 0x70e2, 0x70e2, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x4008, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000,
];

// ASCII Lower Map (A-Z -> a-z pairs)
pub static ONIG_ASCII_LOWER_MAP: [OnigPairCaseFoldCodes; 26] = [
    OnigPairCaseFoldCodes {
        from: 0x41,
        to: 0x61,
    },
    OnigPairCaseFoldCodes {
        from: 0x42,
        to: 0x62,
    },
    OnigPairCaseFoldCodes {
        from: 0x43,
        to: 0x63,
    },
    OnigPairCaseFoldCodes {
        from: 0x44,
        to: 0x64,
    },
    OnigPairCaseFoldCodes {
        from: 0x45,
        to: 0x65,
    },
    OnigPairCaseFoldCodes {
        from: 0x46,
        to: 0x66,
    },
    OnigPairCaseFoldCodes {
        from: 0x47,
        to: 0x67,
    },
    OnigPairCaseFoldCodes {
        from: 0x48,
        to: 0x68,
    },
    OnigPairCaseFoldCodes {
        from: 0x49,
        to: 0x69,
    },
    OnigPairCaseFoldCodes {
        from: 0x4a,
        to: 0x6a,
    },
    OnigPairCaseFoldCodes {
        from: 0x4b,
        to: 0x6b,
    },
    OnigPairCaseFoldCodes {
        from: 0x4c,
        to: 0x6c,
    },
    OnigPairCaseFoldCodes {
        from: 0x4d,
        to: 0x6d,
    },
    OnigPairCaseFoldCodes {
        from: 0x4e,
        to: 0x6e,
    },
    OnigPairCaseFoldCodes {
        from: 0x4f,
        to: 0x6f,
    },
    OnigPairCaseFoldCodes {
        from: 0x50,
        to: 0x70,
    },
    OnigPairCaseFoldCodes {
        from: 0x51,
        to: 0x71,
    },
    OnigPairCaseFoldCodes {
        from: 0x52,
        to: 0x72,
    },
    OnigPairCaseFoldCodes {
        from: 0x53,
        to: 0x73,
    },
    OnigPairCaseFoldCodes {
        from: 0x54,
        to: 0x74,
    },
    OnigPairCaseFoldCodes {
        from: 0x55,
        to: 0x75,
    },
    OnigPairCaseFoldCodes {
        from: 0x56,
        to: 0x76,
    },
    OnigPairCaseFoldCodes {
        from: 0x57,
        to: 0x77,
    },
    OnigPairCaseFoldCodes {
        from: 0x58,
        to: 0x78,
    },
    OnigPairCaseFoldCodes {
        from: 0x59,
        to: 0x79,
    },
    OnigPairCaseFoldCodes {
        from: 0x5a,
        to: 0x7a,
    },
];

// === ASCII Ctype check helpers ===

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_ascii_code_ctype(code: u32, ctype: u32) -> bool {
    if code < 256 {
        (ONIG_ENC_ASCII_CTYPE_TABLE[code as usize] & ctype_to_bit(ctype) as u16) != 0
    } else {
        false
    }
}

#[inline]
pub fn onigenc_ascii_code_to_lower_case(c: u8) -> u8 {
    ONIG_ENC_ASCII_TO_LOWER_CASE_TABLE[c as usize]
}

#[inline]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_ascii_code_case_ambig(code: u32) -> bool {
    onigenc_is_ascii_code_ctype(code, ONIGENC_CTYPE_UPPER)
        || onigenc_is_ascii_code_ctype(code, ONIGENC_CTYPE_LOWER)
}

// === Shared Encoding Functions (from regenc.c) ===
// These are used by multiple encoding implementations.

/// Single byte: mbc_enc_len always returns 1
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_single_byte_mbc_enc_len(_p: &[u8]) -> usize {
    1
}

/// Single byte: mbc_to_code returns the byte value
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_single_byte_mbc_to_code(p: &[u8], _end: usize) -> OnigCodePoint {
    p[0] as OnigCodePoint
}

/// Single byte: code_to_mbclen always returns 1
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_single_byte_code_to_mbclen(code: OnigCodePoint) -> i32 {
    if code < 256 {
        1
    } else {
        ONIGERR_INVALID_CODE_POINT_VALUE
    }
}

/// Single byte: code_to_mbc writes one byte
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_single_byte_code_to_mbc(code: OnigCodePoint, buf: &mut [u8]) -> i32 {
    buf[0] = (code & 0xff) as u8;
    1
}

/// Single byte: left_adjust_char_head returns s unchanged
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_single_byte_left_adjust_char_head(_start: usize, s: usize, _data: &[u8]) -> usize {
    s
}

/// Always returns true for is_allowed_reverse_match
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_always_true_is_allowed_reverse_match(_p: &[u8]) -> bool {
    true
}

/// Always returns false for is_allowed_reverse_match
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_always_false_is_allowed_reverse_match(_p: &[u8]) -> bool {
    false
}

/// Always returns true for is_valid_mbc_string
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_always_true_is_valid_mbc_string(_s: &[u8]) -> bool {
    true
}

/// Check if byte at p is 0x0a newline
pub fn onigenc_is_mbc_newline_0x0a(p: &[u8], end: usize) -> bool {
    !p.is_empty() && p.len() > 0 && (end - 0) > 0 && p[0] == NEWLINE_CODE as u8
}

/// ASCII mbc_case_fold: fold a single ASCII character
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_ascii_mbc_case_fold(
    _flag: OnigCaseFoldType,
    pp: &mut usize,
    _end: usize,
    source: &[u8],
    fold_buf: &mut [u8],
) -> i32 {
    fold_buf[0] = ONIG_ENC_ASCII_TO_LOWER_CASE_TABLE[source[*pp] as usize];
    *pp += 1;
    1
}

/// ASCII apply_all_case_fold: iterate all A-Z <-> a-z pairs
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_ascii_apply_all_case_fold(
    _flag: OnigCaseFoldType,
    f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
) -> i32 {
    for pair in &ONIG_ASCII_LOWER_MAP {
        let code = pair.to;
        let r = f(pair.from, &[code]);
        if r != 0 {
            return r;
        }

        let code = pair.from;
        let r = f(pair.to, &[code]);
        if r != 0 {
            return r;
        }
    }
    0
}

/// ASCII get_case_fold_codes_by_str
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_ascii_get_case_fold_codes_by_str(
    _flag: OnigCaseFoldType,
    p: &[u8],
    _end: usize,
    items: &mut [OnigCaseFoldCodeItem],
) -> i32 {
    let c = p[0];
    if (0x41..=0x5a).contains(&c) {
        // A-Z -> a-z
        items[0].byte_len = 1;
        items[0].code_len = 1;
        items[0].code[0] = (c + 0x20) as OnigCodePoint;
        1
    } else if (0x61..=0x7a).contains(&c) {
        // a-z -> A-Z
        items[0].byte_len = 1;
        items[0].code_len = 1;
        items[0].code[0] = (c - 0x20) as OnigCodePoint;
        1
    } else {
        0
    }
}

/// Minimum property name to ctype (only basic POSIX names)
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_minimum_property_name_to_ctype(_p: &[u8]) -> i32 {
    ONIGERR_INVALID_CHAR_PROPERTY_NAME
}

/// Not supported get_ctype_code_range
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_not_support_get_ctype_code_range(
    _ctype: u32,
    _sb_out: &mut OnigCodePoint,
) -> Option<&'static [OnigCodePoint]> {
    None
}

// === Encoding Utility Functions (from regenc.c) ===

/// Step back n characters from s within [start..]
pub fn onigenc_step_back(
    enc: OnigEncoding,
    start: usize,
    s: usize,
    data: &[u8],
    n: usize,
) -> Option<usize> {
    let mut s = s;
    for _ in 0..n {
        if s <= start {
            return None;
        }
        s = enc.left_adjust_char_head(start, s - 1, data);
    }
    Some(s)
}

/// Step forward n characters from p, returns None if past end
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_step(
    enc: OnigEncoding,
    p: usize,
    end: usize,
    data: &[u8],
    n: usize,
) -> Option<usize> {
    let mut q = p;
    for _ in 0..n {
        q += enc.mbc_enc_len(&data[q..]);
    }
    if q <= end {
        Some(q)
    } else {
        None
    }
}

/// Count characters in [p..end)
pub fn onigenc_strlen(enc: OnigEncoding, data: &[u8], p: usize, end: usize) -> usize {
    let mut n = 0;
    let mut q = p;
    while q < end {
        q += enc.mbc_enc_len(&data[q..]);
        n += 1;
    }
    n
}

/// Get previous character head
pub fn onigenc_get_prev_char_head(
    enc: OnigEncoding,
    start: usize,
    s: usize,
    data: &[u8],
) -> Option<usize> {
    if s <= start {
        None
    } else {
        Some(enc.left_adjust_char_head(start, s - 1, data))
    }
}

/// Get right-adjusted char head
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_get_right_adjust_char_head(
    enc: OnigEncoding,
    start: usize,
    s: usize,
    data: &[u8],
) -> usize {
    let p = enc.left_adjust_char_head(start, s, data);
    if p < s {
        p + enc.mbc_enc_len(&data[p..])
    } else {
        p
    }
}

/// Count characters in a null-terminated byte string (C API compatibility).
/// Finds the first `\0` byte and counts characters up to that point.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_strlen_null(enc: OnigEncoding, data: &[u8]) -> usize {
    let null_pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    onigenc_strlen(enc, data, 0, null_pos)
}

/// Free function wrapper for `Encoding::is_valid_mbc_string` (C API compatibility).
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_valid_mbc_string(enc: OnigEncoding, data: &[u8]) -> bool {
    enc.is_valid_mbc_string(data)
}

/// Free function wrapper for `Encoding::left_adjust_char_head` (C API compatibility).
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_get_left_adjust_char_head(
    enc: OnigEncoding,
    start: usize,
    s: usize,
    data: &[u8],
) -> usize {
    enc.left_adjust_char_head(start, s, data)
}

/// Is word ASCII check (used by encodings)
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onigenc_is_mbc_word_ascii(enc: OnigEncoding, data: &[u8], s: usize, _end: usize) -> bool {
    if data[s] < 128 {
        let code = enc.mbc_to_code(&data[s..], data.len());
        onigenc_is_code_word(enc, code)
    } else {
        false
    }
}
