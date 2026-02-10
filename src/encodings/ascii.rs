// encodings/ascii.rs - Port of ascii.c
// US-ASCII encoding implementation.

use crate::oniguruma::*;
use crate::regenc::*;

// === ASCII Encoding Struct ===
pub struct AsciiEncoding;

pub static ONIG_ENCODING_ASCII: AsciiEncoding = AsciiEncoding;

impl Encoding for AsciiEncoding {
    fn mbc_enc_len(&self, _p: &[u8]) -> usize {
        onigenc_single_byte_mbc_enc_len(_p)
    }

    fn name(&self) -> &str {
        "US-ASCII"
    }

    fn max_enc_len(&self) -> usize {
        1
    }

    fn min_enc_len(&self) -> usize {
        1
    }

    fn is_mbc_newline(&self, p: &[u8], end: usize) -> bool {
        onigenc_is_mbc_newline_0x0a(p, end)
    }

    fn mbc_to_code(&self, p: &[u8], end: usize) -> OnigCodePoint {
        onigenc_single_byte_mbc_to_code(p, end)
    }

    fn code_to_mbclen(&self, code: OnigCodePoint) -> i32 {
        onigenc_single_byte_code_to_mbclen(code)
    }

    fn code_to_mbc(&self, code: OnigCodePoint, buf: &mut [u8]) -> i32 {
        onigenc_single_byte_code_to_mbc(code, buf)
    }

    fn mbc_case_fold(
        &self,
        flag: OnigCaseFoldType,
        pp: &mut usize,
        end: usize,
        source: &[u8],
        fold_buf: &mut [u8],
    ) -> i32 {
        onigenc_ascii_mbc_case_fold(flag, pp, end, source, fold_buf)
    }

    fn apply_all_case_fold(
        &self,
        flag: OnigCaseFoldType,
        f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
    ) -> i32 {
        onigenc_ascii_apply_all_case_fold(flag, f)
    }

    fn get_case_fold_codes_by_str(
        &self,
        flag: OnigCaseFoldType,
        p: &[u8],
        end: usize,
        items: &mut [OnigCaseFoldCodeItem],
    ) -> i32 {
        onigenc_ascii_get_case_fold_codes_by_str(flag, p, end, items)
    }

    fn property_name_to_ctype(&self, p: &[u8]) -> i32 {
        onigenc_minimum_property_name_to_ctype(p)
    }

    fn is_code_ctype(&self, code: OnigCodePoint, ctype: u32) -> bool {
        // ascii_is_code_ctype from ascii.c
        if code < 128 {
            if ctype > ONIGENC_MAX_STD_CTYPE {
                false
            } else {
                onigenc_is_ascii_code_ctype(code, ctype)
            }
        } else {
            false
        }
    }

    fn get_ctype_code_range(
        &self,
        ctype: u32,
        sb_out: &mut OnigCodePoint,
    ) -> Option<&'static [OnigCodePoint]> {
        onigenc_not_support_get_ctype_code_range(ctype, sb_out)
    }

    fn left_adjust_char_head(&self, start: usize, s: usize, data: &[u8]) -> usize {
        onigenc_single_byte_left_adjust_char_head(start, s, data)
    }

    fn is_allowed_reverse_match(&self, p: &[u8]) -> bool {
        onigenc_always_true_is_allowed_reverse_match(p)
    }

    fn init(&self) -> i32 {
        // In C, ascii init() registers built-in callouts (FAIL, MISMATCH, etc.)
        // For now, return ONIG_NORMAL. Callout registration will be added later.
        ONIG_NORMAL
    }

    fn is_initialized(&self) -> bool {
        // Cannot answer (see ascii.c comment), return false
        false
    }

    fn is_valid_mbc_string(&self, s: &[u8]) -> bool {
        onigenc_always_true_is_valid_mbc_string(s)
    }

    fn flag(&self) -> u32 {
        ENC_FLAG_ASCII_COMPATIBLE | ENC_FLAG_SKIP_OFFSET_1
    }

    fn sb_range(&self) -> OnigCodePoint {
        0
    }

    fn index(&self) -> i32 {
        0
    }
}
