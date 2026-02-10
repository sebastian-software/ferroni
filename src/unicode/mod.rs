// unicode/mod.rs - Port of unicode.c
// Unicode character properties, case folding, and related functions.
// Stub implementations for Phase 2; full data tables will be added later.

mod property_data;

use crate::oniguruma::*;
use crate::regenc::*;
use property_data::{CODE_RANGES, CODE_RANGES_NUM, PROPERTY_NAMES};

// === Unicode ISO 8859-1 Ctype Table ===
// From unicode.c: EncUNICODE_ISO_8859_1_CtypeTable[256]
// Used by onigenc_unicode_is_code_ctype for code < 256.

pub static ENC_UNICODE_ISO_8859_1_CTYPE_TABLE: [u16; 256] = [
    0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008,
    0x4008, 0x428c, 0x4289, 0x4288, 0x4288, 0x4288, 0x4008, 0x4008,
    0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008,
    0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008,
    0x4284, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0,
    0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0,
    0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0,
    0x78b0, 0x78b0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0,
    0x41a0, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x74a2,
    0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2,
    0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2,
    0x74a2, 0x74a2, 0x74a2, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x51a0,
    0x41a0, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x70e2,
    0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2,
    0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2,
    0x70e2, 0x70e2, 0x70e2, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x4008,
    0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0288, 0x0008, 0x0008,
    0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008,
    0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008,
    0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008,
    0x0284, 0x01a0, 0x01a0, 0x01a0, 0x01a0, 0x01a0, 0x01a0, 0x01a0,
    0x01a0, 0x01a0, 0x30e2, 0x01a0, 0x01a0, 0x00a8, 0x01a0, 0x01a0,
    0x01a0, 0x01a0, 0x10a0, 0x10a0, 0x01a0, 0x30e2, 0x01a0, 0x01a0,
    0x01a0, 0x10a0, 0x30e2, 0x01a0, 0x10a0, 0x10a0, 0x10a0, 0x01a0,
    0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2,
    0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2,
    0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x01a0,
    0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x30e2,
    0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2,
    0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2,
    0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x01a0,
    0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2,
];

// === Unicode Case Fold Functions ===
// These will be fully implemented when fold data tables are ported.

/// Case fold a multibyte character using Unicode rules.
/// Port of onigenc_unicode_mbc_case_fold from unicode.c
pub fn onigenc_unicode_mbc_case_fold(
    enc: &dyn Encoding,
    _flag: OnigCaseFoldType,
    pp: &mut usize,
    _end: usize,
    data: &[u8],
    fold: &mut [u8],
) -> i32 {
    // TODO: implement with Unicode fold data tables (OnigUnicodeFolds1/2/3)
    // For now: copy the character unchanged (identity fold)
    let len = enc.mbc_enc_len(&data[*pp..]);
    for i in 0..len {
        fold[i] = data[*pp + i];
    }
    *pp += len;
    len as i32
}

/// Apply all Unicode case fold pairs.
/// Port of onigenc_unicode_apply_all_case_fold from unicode.c
pub fn onigenc_unicode_apply_all_case_fold(
    _flag: OnigCaseFoldType,
    _f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
) -> i32 {
    // TODO: implement with Unicode fold data tables
    0
}

/// Get case fold code items for a string.
/// Port of onigenc_unicode_get_case_fold_codes_by_str from unicode.c
pub fn onigenc_unicode_get_case_fold_codes_by_str(
    _enc: &dyn Encoding,
    flag: OnigCaseFoldType,
    p: &[u8],
    end: usize,
    items: &mut [OnigCaseFoldCodeItem],
) -> i32 {
    // For ASCII bytes, delegate to ASCII case fold
    if p[0] < 128 {
        return crate::regenc::onigenc_ascii_get_case_fold_codes_by_str(flag, p, end, items);
    }
    // TODO: implement with Unicode fold data tables for non-ASCII
    0
}

// === Unicode Property Functions ===

/// Convert Unicode property name to ctype.
/// Port of onigenc_unicode_property_name_to_ctype from unicode.c
pub fn onigenc_unicode_property_name_to_ctype(p: &[u8]) -> i32 {
    // Normalize: strip spaces/hyphens/underscores, lowercase
    let mut buf = [0u8; 128];
    let mut len = 0;
    for &b in p {
        if b == b' ' || b == b'-' || b == b'_' {
            continue;
        }
        if b >= 0x80 {
            return ONIGERR_INVALID_CHAR_PROPERTY_NAME;
        }
        if len >= buf.len() {
            return ONIGERR_INVALID_CHAR_PROPERTY_NAME;
        }
        buf[len] = b.to_ascii_lowercase();
        len += 1;
    }
    let key = &buf[..len];
    // Binary search on sorted PROPERTY_NAMES
    match PROPERTY_NAMES.binary_search_by_key(&key, |(name, _)| name.as_bytes()) {
        Ok(idx) => PROPERTY_NAMES[idx].1 as i32,
        Err(_) => ONIGERR_INVALID_CHAR_PROPERTY_NAME,
    }
}

/// Check if code point is of the given Unicode ctype.
/// Port of onigenc_unicode_is_code_ctype from unicode.c
pub fn onigenc_unicode_is_code_ctype(code: OnigCodePoint, ctype: u32) -> bool {
    if ctype <= ONIGENC_MAX_STD_CTYPE && code < 256 {
        return (ENC_UNICODE_ISO_8859_1_CTYPE_TABLE[code as usize]
            & ctype_to_bit(ctype) as u16)
            != 0;
    }

    if (ctype as usize) >= CODE_RANGES_NUM {
        return false;
    }

    // Binary search on code range pairs
    let ranges = CODE_RANGES[ctype as usize];
    let n = ranges.len() / 2;
    let mut low = 0usize;
    let mut high = n;
    while low < high {
        let mid = (low + high) / 2;
        if code > ranges[mid * 2 + 1] {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low < n && code >= ranges[low * 2]
}

/// Get Unicode ctype code range.
/// Port of onigenc_unicode_ctype_code_range from unicode.c
pub fn onigenc_unicode_ctype_code_range(
    ctype: u32,
) -> Option<&'static [OnigCodePoint]> {
    if (ctype as usize) >= CODE_RANGES_NUM {
        return None;
    }
    Some(CODE_RANGES[ctype as usize])
}
