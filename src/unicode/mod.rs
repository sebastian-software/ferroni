// unicode/mod.rs - Port of unicode.c
// Unicode character properties, case folding, and related functions.
// Stub implementations for Phase 2; full data tables will be added later.

pub mod egcb_data;
mod fold_data;
mod property_data;
pub mod wb_data;

use crate::oniguruma::*;
use crate::regenc::*;
use egcb_data::{EgcbType, EGCB_RANGES};
use fold_data::*;
use property_data::{CODE_RANGES, CODE_RANGES_NUM, PROPERTY_NAMES};
use wb_data::{WbType, WB_RANGES};

// === Unicode ISO 8859-1 Ctype Table ===
// From unicode.c: EncUNICODE_ISO_8859_1_CtypeTable[256]
// Used by onigenc_unicode_is_code_ctype for code < 256.

pub static ENC_UNICODE_ISO_8859_1_CTYPE_TABLE: [u16; 256] = [
    0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x428c, 0x4289, 0x4288,
    0x4288, 0x4288, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008,
    0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4008, 0x4284, 0x41a0, 0x41a0, 0x41a0,
    0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0,
    0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x78b0, 0x41a0, 0x41a0,
    0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x7ca2, 0x74a2,
    0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2,
    0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x74a2, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x51a0,
    0x41a0, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x78e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2,
    0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2, 0x70e2,
    0x70e2, 0x70e2, 0x70e2, 0x41a0, 0x41a0, 0x41a0, 0x41a0, 0x4008, 0x0008, 0x0008, 0x0008, 0x0008,
    0x0008, 0x0288, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008,
    0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008, 0x0008,
    0x0008, 0x0008, 0x0008, 0x0008, 0x0284, 0x01a0, 0x01a0, 0x01a0, 0x01a0, 0x01a0, 0x01a0, 0x01a0,
    0x01a0, 0x01a0, 0x30e2, 0x01a0, 0x01a0, 0x00a8, 0x01a0, 0x01a0, 0x01a0, 0x01a0, 0x10a0, 0x10a0,
    0x01a0, 0x30e2, 0x01a0, 0x01a0, 0x01a0, 0x10a0, 0x30e2, 0x01a0, 0x10a0, 0x10a0, 0x10a0, 0x01a0,
    0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2,
    0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x01a0,
    0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x34a2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2,
    0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2,
    0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x30e2, 0x01a0, 0x30e2, 0x30e2, 0x30e2, 0x30e2,
    0x30e2, 0x30e2, 0x30e2, 0x30e2,
];

// === Unicode Case Fold Lookup Helpers ===

/// Binary search on UNFOLD_KEY: code -> (index, fold_len) or None.
/// Port of onigenc_unicode_unfold_key (gperf hash in C).
fn unfold_key(code: OnigCodePoint) -> Option<(usize, usize)> {
    UNFOLD_KEY
        .binary_search_by_key(&code, |&(c, _, _)| c)
        .ok()
        .map(|i| (UNFOLD_KEY[i].1 as usize, UNFOLD_KEY[i].2 as usize))
}

/// Binary search on FOLD1_KEY: fold codepoint -> index into UNICODE_FOLDS1.
fn fold1_key(code: OnigCodePoint) -> Option<usize> {
    FOLD1_KEY
        .binary_search_by_key(&code, |&(c, _)| c)
        .ok()
        .map(|i| FOLD1_KEY[i].1 as usize)
}

/// Binary search on FOLD2_KEY: (cp1, cp2) -> index into UNICODE_FOLDS2.
fn fold2_key(codes: &[OnigCodePoint]) -> Option<usize> {
    let key = [codes[0], codes[1]];
    FOLD2_KEY
        .binary_search_by_key(&key, |&(k, _)| k)
        .ok()
        .map(|i| FOLD2_KEY[i].1 as usize)
}

/// Binary search on FOLD3_KEY: (cp1, cp2, cp3) -> index into UNICODE_FOLDS3.
fn fold3_key(codes: &[OnigCodePoint]) -> Option<usize> {
    let key = [codes[0], codes[1], codes[2]];
    FOLD3_KEY
        .binary_search_by_key(&key, |&(k, _)| k)
        .ok()
        .map(|i| FOLD3_KEY[i].1 as usize)
}

// FOLDS accessor helpers (port of C macros from regenc.h)
#[inline]
fn folds1_fold(i: usize) -> OnigCodePoint {
    UNICODE_FOLDS1[i]
}
#[inline]
fn folds1_unfolds_num(i: usize) -> usize {
    UNICODE_FOLDS1[i + 1] as usize
}
#[inline]
fn folds1_unfolds(i: usize) -> &'static [u32] {
    let n = folds1_unfolds_num(i);
    &UNICODE_FOLDS1[i + 2..i + 2 + n]
}
#[inline]
fn folds1_next(i: usize) -> usize {
    i + 2 + folds1_unfolds_num(i)
}

#[inline]
fn folds2_fold(i: usize) -> &'static [u32] {
    &UNICODE_FOLDS2[i..i + 2]
}
#[inline]
fn folds2_unfolds_num(i: usize) -> usize {
    UNICODE_FOLDS2[i + 2] as usize
}
#[inline]
fn folds2_unfolds(i: usize) -> &'static [u32] {
    let n = folds2_unfolds_num(i);
    &UNICODE_FOLDS2[i + 3..i + 3 + n]
}
#[inline]
fn folds2_next(i: usize) -> usize {
    i + 3 + folds2_unfolds_num(i)
}

#[inline]
fn folds3_fold(i: usize) -> &'static [u32] {
    &UNICODE_FOLDS3[i..i + 3]
}
#[inline]
fn folds3_unfolds_num(i: usize) -> usize {
    UNICODE_FOLDS3[i + 3] as usize
}
#[inline]
fn folds3_unfolds(i: usize) -> &'static [u32] {
    let n = folds3_unfolds_num(i);
    &UNICODE_FOLDS3[i + 4..i + 4 + n]
}
#[inline]
fn folds3_next(i: usize) -> usize {
    i + 4 + folds3_unfolds_num(i)
}

/// Get the fold address for a given (index, fold_len) from unfold_key lookup.
fn folds_fold_addr(index: usize, fold_len: usize) -> &'static [u32] {
    match fold_len {
        1 => &UNICODE_FOLDS1[index..index + 1],
        2 => &UNICODE_FOLDS2[index..index + 2],
        3 => &UNICODE_FOLDS3[index..index + 3],
        _ => &[],
    }
}

// === Unicode Case Fold Functions ===

/// Case fold a multibyte character using Unicode rules.
/// Port of onigenc_unicode_mbc_case_fold from unicode.c lines 79-134
pub fn onigenc_unicode_mbc_case_fold(
    enc: &dyn Encoding,
    flag: OnigCaseFoldType,
    pp: &mut usize,
    end: usize,
    data: &[u8],
    fold: &mut [u8],
) -> i32 {
    let code = enc.mbc_to_code(&data[*pp..], end);
    let len = enc.mbc_enc_len(&data[*pp..]);
    let p_start = *pp;
    *pp += len;

    if case_fold_is_not_ascii_only(flag) || code < 128 {
        if let Some((index, fold_len)) = unfold_key(code) {
            if fold_len == 1 {
                let fold_code = folds1_fold(index);
                if case_fold_is_not_ascii_only(flag) || fold_code < 128 {
                    return enc.code_to_mbc(fold_code, fold);
                }
            } else {
                // Multi-char fold (fold_len == 2 or 3)
                let addr = folds_fold_addr(index, fold_len);
                let mut rlen = 0i32;
                for i in 0..fold_len {
                    let l = enc.code_to_mbc(addr[i], &mut fold[rlen as usize..]);
                    rlen += l;
                }
                return rlen;
            }
        }
    }

    // No fold found: copy original bytes unchanged
    for i in 0..len {
        fold[i] = data[p_start + i];
    }
    len as i32
}

/// Apply case fold pairs for FOLDS1 entries in range [from..to).
/// Port of apply_case_fold1 from unicode.c lines 136-174
fn apply_case_fold1(
    flag: OnigCaseFoldType,
    from: usize,
    to: usize,
    f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
) -> i32 {
    let mut i = from;
    while i < to {
        let fold = folds1_fold(i);
        if case_fold_is_ascii_only(flag) && fold >= 128 {
            break;
        }
        let unfolds = folds1_unfolds(i);
        let n = unfolds.len();
        for j in 0..n {
            let uf = unfolds[j];
            if case_fold_is_ascii_only(flag) && uf >= 128 {
                continue;
            }
            // fold -> unfold
            let r = f(fold, &[uf]);
            if r != 0 {
                return r;
            }
            // unfold -> fold
            let r = f(uf, &[fold]);
            if r != 0 {
                return r;
            }
            // pair each unfold with previously seen unfolds
            for k in 0..j {
                let uf2 = unfolds[k];
                if case_fold_is_ascii_only(flag) && uf2 >= 128 {
                    continue;
                }
                let r = f(uf, &[uf2]);
                if r != 0 {
                    return r;
                }
                let r = f(uf2, &[uf]);
                if r != 0 {
                    return r;
                }
            }
        }
        i = folds1_next(i);
    }
    0
}

/// Apply case fold pairs for FOLDS2 entries in range [from..to).
/// Port of apply_case_fold2 from unicode.c lines 176-203
fn apply_case_fold2(
    from: usize,
    to: usize,
    f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
) -> i32 {
    let mut i = from;
    while i < to {
        let fold = folds2_fold(i);
        let unfolds = folds2_unfolds(i);
        let n = unfolds.len();
        for j in 0..n {
            let uf = unfolds[j];
            // unfold -> fold (multi-char)
            let r = f(uf, fold);
            if r != 0 {
                return r;
            }
            // pair with previously seen unfolds
            for k in 0..j {
                let uf2 = unfolds[k];
                let r = f(uf, &[uf2]);
                if r != 0 {
                    return r;
                }
                let r = f(uf2, &[uf]);
                if r != 0 {
                    return r;
                }
            }
        }
        i = folds2_next(i);
    }
    0
}

/// Apply case fold pairs for FOLDS3 entries in range [from..to).
/// Port of apply_case_fold3 from unicode.c lines 205-232
fn apply_case_fold3(
    from: usize,
    to: usize,
    f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
) -> i32 {
    let mut i = from;
    while i < to {
        let fold = folds3_fold(i);
        let unfolds = folds3_unfolds(i);
        let n = unfolds.len();
        for j in 0..n {
            let uf = unfolds[j];
            let r = f(uf, fold);
            if r != 0 {
                return r;
            }
            for k in 0..j {
                let uf2 = unfolds[k];
                let r = f(uf, &[uf2]);
                if r != 0 {
                    return r;
                }
                let r = f(uf2, &[uf]);
                if r != 0 {
                    return r;
                }
            }
        }
        i = folds3_next(i);
    }
    0
}

/// Apply all Unicode case fold pairs.
/// Port of onigenc_unicode_apply_all_case_fold from unicode.c lines 234-286
pub fn onigenc_unicode_apply_all_case_fold(
    flag: OnigCaseFoldType,
    f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
) -> i32 {
    // Normal FOLDS1 entries
    let mut r = apply_case_fold1(flag, 0, FOLDS1_NORMAL_END_INDEX, f);
    if r != 0 {
        return r;
    }

    // Locale entries (non-Turkish: include all)
    r = apply_case_fold1(flag, FOLDS1_NORMAL_END_INDEX, FOLDS1_END_INDEX, f);
    if r != 0 {
        return r;
    }

    // Multi-char folds only if MULTI_CHAR flag is set
    if (flag & INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR) == 0 {
        return 0;
    }

    r = apply_case_fold2(0, FOLDS2_NORMAL_END_INDEX, f);
    if r != 0 {
        return r;
    }
    r = apply_case_fold2(FOLDS2_NORMAL_END_INDEX, FOLDS2_END_INDEX, f);
    if r != 0 {
        return r;
    }

    r = apply_case_fold3(0, FOLDS3_NORMAL_END_INDEX, f);
    if r != 0 {
        return r;
    }

    0
}

/// Get case fold code items for a string.
/// Port of onigenc_unicode_get_case_fold_codes_by_str from unicode.c lines 288-585
pub fn onigenc_unicode_get_case_fold_codes_by_str(
    enc: &dyn Encoding,
    flag: OnigCaseFoldType,
    p: &[u8],
    _end: usize,
    items: &mut [OnigCaseFoldCodeItem],
) -> i32 {
    let remaining = p.len(); // use slice length, not end parameter
    let mut n = 0usize; // number of items accumulated

    let code = enc.mbc_to_code(p, remaining);
    if case_fold_is_ascii_only(flag) && code >= 128 {
        return 0;
    }
    let len0 = enc.mbc_enc_len(p);
    let mut orig_codes = [0u32; 3];
    let mut codes = [0u32; 3];
    let mut lens = [0usize; 3]; // cumulative byte lengths

    orig_codes[0] = code;
    lens[0] = len0;

    // Get canonical form of first codepoint
    let buk1 = unfold_key(orig_codes[0]);
    if let Some((index, fold_len)) = buk1 {
        if fold_len == 1 {
            codes[0] = folds1_fold(index);
        } else {
            codes[0] = orig_codes[0];
        }
    } else {
        codes[0] = orig_codes[0];
    }

    // Multi-char fold handling
    if (flag & INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR) == 0 {
        // Skip multi-char, go directly to fold1
    } else if len0 < remaining {
        let p1 = &p[len0..];
        let code1 = enc.mbc_to_code(p1, p1.len());
        orig_codes[1] = code1;
        let len1 = enc.mbc_enc_len(p1);
        lens[1] = lens[0] + len1;

        if let Some((idx, fl)) = unfold_key(orig_codes[1]) {
            if fl == 1 {
                codes[1] = folds1_fold(idx);
            } else {
                codes[1] = orig_codes[1];
            }
        } else {
            codes[1] = orig_codes[1];
        }

        // Try 3-char fold
        if lens[1] < remaining {
            let p2 = &p[lens[1]..];
            let code2 = enc.mbc_to_code(p2, p2.len());
            orig_codes[2] = code2;
            let len2 = enc.mbc_enc_len(p2);
            lens[2] = lens[1] + len2;

            if let Some((idx, fl)) = unfold_key(orig_codes[2]) {
                if fl == 1 {
                    codes[2] = folds1_fold(idx);
                } else {
                    codes[2] = orig_codes[2];
                }
            } else {
                codes[2] = orig_codes[2];
            }

            if let Some(index) = fold3_key(&codes) {
                // Add single-codepoint unfolds
                let unfolds = folds3_unfolds(index);
                for uf in unfolds {
                    items[n].byte_len = lens[2] as i32;
                    items[n].code_len = 1;
                    items[n].code[0] = *uf;
                    n += 1;
                }

                // Build variant combinations at each position
                let mut cs = [[0u32; 4]; 3];
                let mut ncs = [0usize; 3];
                let fold3 = folds3_fold(index);
                for fn_idx in 0..3 {
                    cs[fn_idx][0] = fold3[fn_idx];
                    ncs[fn_idx] = 1;
                    if let Some(sidx) = fold1_key(cs[fn_idx][0]) {
                        let sunfolds = folds1_unfolds(sidx);
                        for (si, &su) in sunfolds.iter().enumerate() {
                            cs[fn_idx][si + 1] = su;
                        }
                        ncs[fn_idx] += sunfolds.len();
                    }
                }

                for i in 0..ncs[0] {
                    for j in 0..ncs[1] {
                        for k in 0..ncs[2] {
                            if cs[0][i] == orig_codes[0]
                                && cs[1][j] == orig_codes[1]
                                && cs[2][k] == orig_codes[2]
                            {
                                continue;
                            }
                            items[n].byte_len = lens[2] as i32;
                            items[n].code_len = 3;
                            items[n].code[0] = cs[0][i];
                            items[n].code[1] = cs[1][j];
                            items[n].code[2] = cs[2][k];
                            n += 1;
                        }
                    }
                }
                return n as i32;
            }
        }

        // Try 2-char fold
        if let Some(index) = fold2_key(&codes) {
            let unfolds = folds2_unfolds(index);
            for uf in unfolds {
                items[n].byte_len = lens[1] as i32;
                items[n].code_len = 1;
                items[n].code[0] = *uf;
                n += 1;
            }

            let mut cs = [[0u32; 4]; 2];
            let mut ncs = [0usize; 2];
            let fold2 = folds2_fold(index);
            for fn_idx in 0..2 {
                cs[fn_idx][0] = fold2[fn_idx];
                ncs[fn_idx] = 1;
                if let Some(sidx) = fold1_key(cs[fn_idx][0]) {
                    let sunfolds = folds1_unfolds(sidx);
                    for (si, &su) in sunfolds.iter().enumerate() {
                        cs[fn_idx][si + 1] = su;
                    }
                    ncs[fn_idx] += sunfolds.len();
                }
            }

            for i in 0..ncs[0] {
                for j in 0..ncs[1] {
                    if cs[0][i] == orig_codes[0] && cs[1][j] == orig_codes[1] {
                        continue;
                    }
                    items[n].byte_len = lens[1] as i32;
                    items[n].code_len = 2;
                    items[n].code[0] = cs[0][i];
                    items[n].code[1] = cs[1][j];
                    n += 1;
                }
            }
            return n as i32;
        }
    }

    // === Single-char fold handling (fold1) ===
    if let Some((buk_index, buk_fold_len)) = buk1 {
        if buk_fold_len == 1 {
            // Input has a 1-to-1 fold
            let fold_code = folds1_fold(buk_index);
            if case_fold_is_not_ascii_only(flag) || fold_code < 128 {
                items[n].byte_len = lens[0] as i32;
                items[n].code_len = 1;
                items[n].code[0] = fold_code;
                n += 1;
            }
            // Add all unfold variants (excluding the original)
            let unfolds = folds1_unfolds(buk_index);
            for &uf in unfolds {
                if uf != orig_codes[0] {
                    if case_fold_is_not_ascii_only(flag) || uf < 128 {
                        items[n].byte_len = lens[0] as i32;
                        items[n].code_len = 1;
                        items[n].code[0] = uf;
                        n += 1;
                    }
                }
            }
        } else if (flag & INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR) != 0 {
            // Input codepoint is itself a multi-char fold target (e.g., sharp-s -> "ss")
            if buk_fold_len == 2 {
                // Add other single-codepoint unfolds
                let unfolds = folds2_unfolds(buk_index);
                for &uf in unfolds {
                    if uf == orig_codes[0] {
                        continue;
                    }
                    items[n].byte_len = lens[0] as i32;
                    items[n].code_len = 1;
                    items[n].code[0] = uf;
                    n += 1;
                }
                // Build 2-char variant combinations
                let mut cs = [[0u32; 4]; 2];
                let mut ncs = [0usize; 2];
                let fold2 = folds2_fold(buk_index);
                for fn_idx in 0..2 {
                    cs[fn_idx][0] = fold2[fn_idx];
                    ncs[fn_idx] = 1;
                    if let Some(sidx) = fold1_key(cs[fn_idx][0]) {
                        let sunfolds = folds1_unfolds(sidx);
                        for (si, &su) in sunfolds.iter().enumerate() {
                            cs[fn_idx][si + 1] = su;
                        }
                        ncs[fn_idx] += sunfolds.len();
                    }
                }
                for i in 0..ncs[0] {
                    for j in 0..ncs[1] {
                        items[n].byte_len = lens[0] as i32;
                        items[n].code_len = 2;
                        items[n].code[0] = cs[0][i];
                        items[n].code[1] = cs[1][j];
                        n += 1;
                    }
                }
            } else if buk_fold_len == 3 {
                let unfolds = folds3_unfolds(buk_index);
                for &uf in unfolds {
                    if uf == orig_codes[0] {
                        continue;
                    }
                    items[n].byte_len = lens[0] as i32;
                    items[n].code_len = 1;
                    items[n].code[0] = uf;
                    n += 1;
                }
                let mut cs = [[0u32; 4]; 3];
                let mut ncs = [0usize; 3];
                let fold3 = folds3_fold(buk_index);
                for fn_idx in 0..3 {
                    cs[fn_idx][0] = fold3[fn_idx];
                    ncs[fn_idx] = 1;
                    if let Some(sidx) = fold1_key(cs[fn_idx][0]) {
                        let sunfolds = folds1_unfolds(sidx);
                        for (si, &su) in sunfolds.iter().enumerate() {
                            cs[fn_idx][si + 1] = su;
                        }
                        ncs[fn_idx] += sunfolds.len();
                    }
                }
                for i in 0..ncs[0] {
                    for j in 0..ncs[1] {
                        for k in 0..ncs[2] {
                            items[n].byte_len = lens[0] as i32;
                            items[n].code_len = 3;
                            items[n].code[0] = cs[0][i];
                            items[n].code[1] = cs[1][j];
                            items[n].code[2] = cs[2][k];
                            n += 1;
                        }
                    }
                }
            }
        }
    } else {
        // Input is already a canonical fold form — look it up as a fold1 key
        if let Some(index) = fold1_key(orig_codes[0]) {
            let unfolds = folds1_unfolds(index);
            for &uf in unfolds {
                if case_fold_is_not_ascii_only(flag) || uf < 128 {
                    items[n].byte_len = lens[0] as i32;
                    items[n].code_len = 1;
                    items[n].code[0] = uf;
                    n += 1;
                }
            }
        }
    }

    n as i32
}

// === User-Defined Unicode Properties ===
// Port of C's UserDefinedPropertyValue + onig_unicode_define_user_property

use std::sync::Mutex;

/// Maximum number of user-defined properties (matches C's USER_DEFINED_PROPERTY_MAX_NUM).
const USER_DEFINED_PROPERTY_MAX_NUM: usize = 32;

struct UserProperty {
    /// Normalized name (lowercase, no spaces/hyphens/underscores).
    name: Vec<u8>,
    /// Code point ranges in [start, end, start, end, ...] pair format.
    ranges: Vec<OnigCodePoint>,
}

static USER_DEFINED_PROPERTIES: Mutex<Vec<UserProperty>> = Mutex::new(Vec::new());

/// Normalize a property name: strip spaces/hyphens/underscores, lowercase.
/// Returns None if the name contains non-ASCII bytes or exceeds buffer size.
fn normalize_property_name(name: &[u8]) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity(name.len());
    for &b in name {
        if b == b' ' || b == b'-' || b == b'_' {
            continue;
        }
        if b >= 0x80 {
            return None;
        }
        buf.push(b.to_ascii_lowercase());
    }
    if buf.is_empty() {
        return None;
    }
    Some(buf)
}

/// Register a user-defined Unicode property with associated code point ranges.
/// Ranges should be in `[start, end, start, end, ...]` pair format.
/// Returns `Ok(())` on success, or `Err(error_code)` on failure.
pub fn onig_unicode_define_user_property(name: &[u8], ranges: &[OnigCodePoint]) -> Result<(), i32> {
    let normalized = normalize_property_name(name)
        .ok_or(ONIGERR_INVALID_CHAR_PROPERTY_NAME)?;

    let mut props = USER_DEFINED_PROPERTIES.lock().unwrap();

    // Check for duplicate
    for prop in props.iter() {
        if prop.name == normalized {
            return Err(ONIGERR_INVALID_CHAR_PROPERTY_NAME);
        }
    }

    if props.len() >= USER_DEFINED_PROPERTY_MAX_NUM {
        return Err(ONIGERR_TOO_MANY_USER_DEFINED_OBJECTS);
    }

    props.push(UserProperty {
        name: normalized,
        ranges: ranges.to_vec(),
    });

    Ok(())
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
        Err(_) => {
            // Check user-defined properties
            if let Ok(props) = USER_DEFINED_PROPERTIES.lock() {
                for (i, prop) in props.iter().enumerate() {
                    if prop.name == key {
                        return (CODE_RANGES_NUM + i) as i32;
                    }
                }
            }
            ONIGERR_INVALID_CHAR_PROPERTY_NAME
        }
    }
}

/// Check if code point is of the given Unicode ctype.
/// Port of onigenc_unicode_is_code_ctype from unicode.c
pub fn onigenc_unicode_is_code_ctype(code: OnigCodePoint, ctype: u32) -> bool {
    if ctype <= ONIGENC_MAX_STD_CTYPE && code < 256 {
        return (ENC_UNICODE_ISO_8859_1_CTYPE_TABLE[code as usize] & ctype_to_bit(ctype) as u16)
            != 0;
    }

    if (ctype as usize) >= CODE_RANGES_NUM {
        // Check user-defined properties
        let user_idx = (ctype as usize) - CODE_RANGES_NUM;
        if let Ok(props) = USER_DEFINED_PROPERTIES.lock() {
            if user_idx < props.len() {
                let ranges = &props[user_idx].ranges;
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
                return low < n && code >= ranges[low * 2];
            }
        }
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
pub fn onigenc_unicode_ctype_code_range(ctype: u32) -> Option<&'static [OnigCodePoint]> {
    if (ctype as usize) >= CODE_RANGES_NUM {
        // User-defined properties cannot return &'static references since they
        // are dynamically allocated. Callers should use is_code_ctype instead.
        return None;
    }
    Some(CODE_RANGES[ctype as usize])
}

// ============================================================================
// Extended Grapheme Cluster Break (EGCB) algorithm
// Port of onigenc_egcb_is_break_position from unicode.c
// ============================================================================

/// EGCB break result from two-character rule table
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EgcbBreakType {
    NotBreak,
    Break,
    BreakUndefGB11,
    BreakUndefRiRi,
}

/// Binary search EGCB_RANGES for the EGCB type of a codepoint.
fn egcb_get_type(code: u32) -> EgcbType {
    let mut low: usize = 0;
    let mut high: usize = EGCB_RANGES.len();
    while low < high {
        let x = (low + high) >> 1;
        if code > EGCB_RANGES[x].end {
            low = x + 1;
        } else {
            high = x;
        }
    }
    if low < EGCB_RANGES.len() && code >= EGCB_RANGES[low].start {
        EGCB_RANGES[low].prop
    } else {
        EgcbType::Other
    }
}

#[inline]
fn is_control_cr_lf(t: EgcbType) -> bool {
    matches!(t, EgcbType::CR | EgcbType::LF | EgcbType::Control)
}

#[inline]
fn is_hangul(t: EgcbType) -> bool {
    matches!(
        t,
        EgcbType::L | EgcbType::LV | EgcbType::LVT | EgcbType::T | EgcbType::V
    )
}

/// PROP_INDEX_EXTENDEDPICTOGRAPHIC = 81 in property_data.rs
const PROP_INDEX_EXTENDEDPICTOGRAPHIC: u32 = 81;

/// GB1/GB2 are handled outside. This applies GB3-GB13 two-char rules.
fn unicode_egcb_is_break_2code(from_code: u32, to_code: u32) -> EgcbBreakType {
    let from = egcb_get_type(from_code);
    let to = egcb_get_type(to_code);

    // Short cut: both Other
    if from == EgcbType::Other && to == EgcbType::Other {
        return EgcbBreakType::Break; // GB999
    }

    // GB3: CR + LF
    if from == EgcbType::CR && to == EgcbType::LF {
        return EgcbBreakType::NotBreak;
    }
    // GB4: Break after Control/CR/LF
    if is_control_cr_lf(from) {
        return EgcbBreakType::Break;
    }
    // GB5: Break before Control/CR/LF
    if is_control_cr_lf(to) {
        return EgcbBreakType::Break;
    }

    // GB6-GB8: Hangul rules
    if is_hangul(from) && is_hangul(to) {
        // GB6: L x (L | V | LV | LVT)
        if from == EgcbType::L && to != EgcbType::T {
            return EgcbBreakType::NotBreak;
        }
        // GB7: (LV | V) x (V | T)
        if (from == EgcbType::LV || from == EgcbType::V) && (to == EgcbType::V || to == EgcbType::T)
        {
            return EgcbBreakType::NotBreak;
        }
        // GB8: (LVT | T) x T
        if to == EgcbType::T && (from == EgcbType::LVT || from == EgcbType::T) {
            return EgcbBreakType::NotBreak;
        }
        return EgcbBreakType::Break; // GB999
    }

    // GB9: x (Extend | ZWJ)
    if to == EgcbType::Extend || to == EgcbType::ZWJ {
        return EgcbBreakType::NotBreak;
    }
    // GB9a: x SpacingMark
    if to == EgcbType::SpacingMark {
        return EgcbBreakType::NotBreak;
    }
    // GB9b: Prepend x
    if from == EgcbType::Prepend {
        return EgcbBreakType::NotBreak;
    }

    // GB11: ZWJ x Extended_Pictographic (needs backward context)
    if from == EgcbType::ZWJ {
        if onigenc_unicode_is_code_ctype(to_code, PROP_INDEX_EXTENDEDPICTOGRAPHIC) {
            return EgcbBreakType::BreakUndefGB11;
        }
        return EgcbBreakType::Break; // GB999
    }

    // GB12/GB13: RI x RI (needs backward context)
    if from == EgcbType::RegionalIndicator && to == EgcbType::RegionalIndicator {
        return EgcbBreakType::BreakUndefRiRi;
    }

    // GB999
    EgcbBreakType::Break
}

/// Full EGCB break position check.
/// Port of onigenc_egcb_is_break_position from unicode.c:998.
pub fn onigenc_egcb_is_break_position(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    start: usize,
    end: usize,
) -> bool {
    // GB1: Break at start of text
    if s <= start {
        return true;
    }
    // GB2: Break at end of text
    if s >= end {
        return true;
    }

    let mut prev = enc.left_adjust_char_head(start, s - 1, str_data);
    if prev < start {
        return true;
    }

    let from = enc.mbc_to_code(&str_data[prev..], end);
    let to = enc.mbc_to_code(&str_data[s..], end);

    let btype = unicode_egcb_is_break_2code(from, to);
    match btype {
        EgcbBreakType::NotBreak => false,
        EgcbBreakType::Break => true,

        EgcbBreakType::BreakUndefGB11 => {
            // GB11: {ExtPict} Extend* ZWJ x {ExtPict}
            // Scan backward past Extend characters looking for ExtPict
            loop {
                if prev <= start {
                    break;
                }
                prev = enc.left_adjust_char_head(start, prev - 1, str_data);
                if prev < start {
                    break;
                }
                let code = enc.mbc_to_code(&str_data[prev..], end);
                if onigenc_unicode_is_code_ctype(code, PROP_INDEX_EXTENDEDPICTOGRAPHIC) {
                    return false; // Found ExtPict before ZWJ
                }
                let t = egcb_get_type(code);
                if t != EgcbType::Extend {
                    break; // Not Extend, stop scanning
                }
            }
            true // Break (no ExtPict found)
        }

        EgcbBreakType::BreakUndefRiRi => {
            // GB12/GB13: Count consecutive RI chars backward
            let mut n: usize = 0;
            loop {
                if prev <= start {
                    break;
                }
                prev = enc.left_adjust_char_head(start, prev - 1, str_data);
                if prev < start {
                    break;
                }
                let code = enc.mbc_to_code(&str_data[prev..], end);
                let t = egcb_get_type(code);
                if t != EgcbType::RegionalIndicator {
                    break;
                }
                n += 1;
            }
            // Even count of preceding RI = no break, odd = break
            (n % 2) != 0
        }
    }
}

// ============================================================================
// Word Break (WB) algorithm
// Port of onigenc_wb_is_break_position from unicode.c:675
// ============================================================================

/// Binary search WB_RANGES for the WB type of a codepoint.
fn wb_get_type(code: u32) -> WbType {
    let mut low: usize = 0;
    let mut high: usize = WB_RANGES.len();
    while low < high {
        let x = (low + high) >> 1;
        if code > WB_RANGES[x].end {
            low = x + 1;
        } else {
            high = x;
        }
    }
    if low < WB_RANGES.len() && code >= WB_RANGES[low].start {
        WB_RANGES[low].prop
    } else {
        WbType::Any
    }
}

#[inline]
fn is_wb_ignore_tail(t: WbType) -> bool {
    matches!(t, WbType::Extend | WbType::Format | WbType::ZWJ)
}

#[inline]
fn is_wb_ahletter(t: WbType) -> bool {
    matches!(t, WbType::ALetter | WbType::HebrewLetter)
}

#[inline]
fn is_wb_midnumletq(t: WbType) -> bool {
    matches!(t, WbType::MidNumLet | WbType::SingleQuote)
}

/// Skip forward past Extend/Format/ZWJ to find next "main" code.
fn wb_get_next_main_code(
    enc: OnigEncoding,
    str_data: &[u8],
    mut pos: usize,
    end: usize,
) -> Option<(u32, WbType)> {
    loop {
        pos += enc.mbc_enc_len(&str_data[pos..]);
        if pos >= end {
            break;
        }
        let code = enc.mbc_to_code(&str_data[pos..], end);
        let t = wb_get_type(code);
        if !is_wb_ignore_tail(t) {
            return Some((code, t));
        }
    }
    None
}

/// Full WB break position check.
/// Port of onigenc_wb_is_break_position from unicode.c:675.
pub fn onigenc_wb_is_break_position(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    start: usize,
    end: usize,
) -> bool {
    // WB1: sot / Any
    if s <= start {
        return true;
    }
    // WB2: Any / eot
    if s >= end {
        return true;
    }

    let mut prev = enc.left_adjust_char_head(start, s - 1, str_data);
    if prev < start {
        return true;
    }

    let cfrom = enc.mbc_to_code(&str_data[prev..], end);
    let cto = enc.mbc_to_code(&str_data[s..], end);

    let mut from = wb_get_type(cfrom);
    let to = wb_get_type(cto);

    // Short cut: both Any
    if from == WbType::Any && to == WbType::Any {
        return true; // WB999
    }

    // WB3: CR + LF
    if from == WbType::CR && to == WbType::LF {
        return false;
    }

    // WB3a: (Newline|CR|LF) /
    if matches!(from, WbType::Newline | WbType::CR | WbType::LF) {
        return true;
    }
    // WB3b: / (Newline|CR|LF)
    if matches!(to, WbType::Newline | WbType::CR | WbType::LF) {
        return true;
    }

    // WB3c: ZWJ x {Extended_Pictographic}
    if from == WbType::ZWJ {
        if onigenc_unicode_is_code_ctype(cto, PROP_INDEX_EXTENDEDPICTOGRAPHIC) {
            return false;
        }
    }

    // WB3d: WSegSpace x WSegSpace
    if from == WbType::WSegSpace && to == WbType::WSegSpace {
        return false;
    }

    // WB4: X (Extend|Format|ZWJ)* -> X
    if is_wb_ignore_tail(to) {
        return false;
    }
    if is_wb_ignore_tail(from) {
        // Scan backward past Extend/Format/ZWJ
        loop {
            if prev <= start {
                break;
            }
            let pp = enc.left_adjust_char_head(start, prev - 1, str_data);
            if pp < start {
                break;
            }
            prev = pp;
            let cf = enc.mbc_to_code(&str_data[prev..], end);
            from = wb_get_type(cf);
            if !is_wb_ignore_tail(from) {
                break;
            }
        }
    }

    // WB5: AHLetter x AHLetter
    if is_wb_ahletter(from) {
        if is_wb_ahletter(to) {
            return false;
        }

        // WB6: AHLetter x (MidLetter | MidNumLetQ) AHLetter
        if to == WbType::MidLetter || is_wb_midnumletq(to) {
            if let Some((_cto2, to2)) = wb_get_next_main_code(enc, str_data, s, end) {
                if is_wb_ahletter(to2) {
                    return false;
                }
            }
        }
    }

    // WB7: AHLetter (MidLetter | MidNumLetQ) x AHLetter
    if from == WbType::MidLetter || is_wb_midnumletq(from) {
        if is_wb_ahletter(to) {
            let mut from2 = WbType::Any;
            let mut pp = prev;
            loop {
                if pp <= start {
                    break;
                }
                pp = enc.left_adjust_char_head(start, pp - 1, str_data);
                if pp < start {
                    break;
                }
                let cf2 = enc.mbc_to_code(&str_data[pp..], end);
                from2 = wb_get_type(cf2);
                if !is_wb_ignore_tail(from2) {
                    break;
                }
            }
            if is_wb_ahletter(from2) {
                return false;
            }
        }
    }

    if from == WbType::HebrewLetter {
        // WB7a: Hebrew_Letter x Single_Quote
        if to == WbType::SingleQuote {
            return false;
        }

        // WB7b: Hebrew_Letter x Double_Quote Hebrew_Letter
        if to == WbType::DoubleQuote {
            if let Some((_cto2, to2)) = wb_get_next_main_code(enc, str_data, s, end) {
                if to2 == WbType::HebrewLetter {
                    return false;
                }
            }
        }
    }

    // WB7c: Hebrew_Letter Double_Quote x Hebrew_Letter
    if from == WbType::DoubleQuote {
        if to == WbType::HebrewLetter {
            let mut from2 = WbType::Any;
            let mut pp = prev;
            loop {
                if pp <= start {
                    break;
                }
                pp = enc.left_adjust_char_head(start, pp - 1, str_data);
                if pp < start {
                    break;
                }
                let cf2 = enc.mbc_to_code(&str_data[pp..], end);
                from2 = wb_get_type(cf2);
                if !is_wb_ignore_tail(from2) {
                    break;
                }
            }
            if from2 == WbType::HebrewLetter {
                return false;
            }
        }
    }

    if to == WbType::Numeric {
        // WB8: Numeric x Numeric
        if from == WbType::Numeric {
            return false;
        }
        // WB9: AHLetter x Numeric
        if is_wb_ahletter(from) {
            return false;
        }

        // WB11: Numeric (MidNum | MidNumLetQ) x Numeric
        if from == WbType::MidNum || is_wb_midnumletq(from) {
            let mut from2 = WbType::Any;
            let mut pp = prev;
            loop {
                if pp <= start {
                    break;
                }
                pp = enc.left_adjust_char_head(start, pp - 1, str_data);
                if pp < start {
                    break;
                }
                let cf2 = enc.mbc_to_code(&str_data[pp..], end);
                from2 = wb_get_type(cf2);
                if !is_wb_ignore_tail(from2) {
                    break;
                }
            }
            if from2 == WbType::Numeric {
                return false;
            }
        }
    }

    if from == WbType::Numeric {
        // WB10: Numeric x AHLetter
        if is_wb_ahletter(to) {
            return false;
        }

        // WB12: Numeric x (MidNum | MidNumLetQ) Numeric
        if to == WbType::MidNum || is_wb_midnumletq(to) {
            if let Some((_cto2, to2)) = wb_get_next_main_code(enc, str_data, s, end) {
                if to2 == WbType::Numeric {
                    return false;
                }
            }
        }
    }

    // WB13: Katakana x Katakana
    if from == WbType::Katakana && to == WbType::Katakana {
        return false;
    }

    // WB13a: (AHLetter | Numeric | Katakana | ExtendNumLet) x ExtendNumLet
    if to == WbType::ExtendNumLet {
        if is_wb_ahletter(from)
            || from == WbType::Numeric
            || from == WbType::Katakana
            || from == WbType::ExtendNumLet
        {
            return false;
        }
    }

    // WB13b: ExtendNumLet x (AHLetter | Numeric | Katakana)
    if from == WbType::ExtendNumLet {
        if is_wb_ahletter(to) || to == WbType::Numeric || to == WbType::Katakana {
            return false;
        }
    }

    // WB15/WB16: RI x RI (count consecutive RI backward)
    if from == WbType::RegionalIndicator && to == WbType::RegionalIndicator {
        let mut n: usize = 0;
        let mut pp = prev;
        loop {
            if pp <= start {
                break;
            }
            pp = enc.left_adjust_char_head(start, pp - 1, str_data);
            if pp < start {
                break;
            }
            let cf2 = enc.mbc_to_code(&str_data[pp..], end);
            let from2 = wb_get_type(cf2);
            if from2 != WbType::RegionalIndicator {
                break;
            }
            n += 1;
        }
        if (n % 2) == 0 {
            return false;
        }
    }

    // WB999: Any / Any
    true
}
