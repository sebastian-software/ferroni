// encodings/utf8.rs - Port of utf8.c
// UTF-8 encoding implementation (RFC 3629 range: U+0000 - U+10FFFF).

use crate::oniguruma::*;
use crate::regenc::*;

// === UTF-8 Helpers ===

#[inline]
fn utf8_islead(c: u8) -> bool {
    (c & 0xc0) != 0x80
}

#[inline]
fn utf8_istail(c: u8) -> bool {
    (c & 0xc0) == 0x80
}

// === EncLen_UTF8 Table ===
// Maps first byte to character length (RFC 3629: max 4 bytes).

static ENC_LEN_UTF8: [u8; 256] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

// === UTF-8 Encoding Struct ===

pub struct Utf8Encoding;

pub static ONIG_ENCODING_UTF8: Utf8Encoding = Utf8Encoding;

impl Encoding for Utf8Encoding {
    fn mbc_enc_len(&self, p: &[u8]) -> usize {
        ENC_LEN_UTF8[p[0] as usize] as usize
    }

    fn name(&self) -> &str {
        "UTF-8"
    }

    fn max_enc_len(&self) -> usize {
        4
    }

    fn min_enc_len(&self) -> usize {
        1
    }

    fn is_mbc_newline(&self, p: &[u8], end: usize) -> bool {
        onigenc_is_mbc_newline_0x0a(p, end)
    }

    fn mbc_to_code(&self, p: &[u8], _end: usize) -> OnigCodePoint {
        let mut len = ENC_LEN_UTF8[p[0] as usize] as usize;
        if len > p.len() {
            len = p.len();
        }

        let c = p[0] as u32;
        if len > 1 {
            let remaining = len - 1;
            let mut n = c & ((1u32 << (6 - remaining)) - 1);
            for i in 1..len {
                n = (n << 6) | ((p[i] as u32) & 0x3f);
            }
            n
        } else {
            c
        }
    }

    fn code_to_mbclen(&self, code: OnigCodePoint) -> i32 {
        if (code & 0xffffff80) == 0 {
            1
        } else if (code & 0xfffff800) == 0 {
            2
        } else if (code & 0xffff0000) == 0 {
            3
        } else if (code & 0xffe00000) == 0 {
            4
        } else {
            ONIGERR_INVALID_CODE_POINT_VALUE
        }
    }

    fn code_to_mbc(&self, code: OnigCodePoint, buf: &mut [u8]) -> i32 {
        if (code & 0xffffff80) == 0 {
            buf[0] = code as u8;
            1
        } else {
            let mut i = 0;
            if (code & 0xfffff800) == 0 {
                buf[i] = ((code >> 6) & 0x1f) as u8 | 0xc0;
                i += 1;
            } else if (code & 0xffff0000) == 0 {
                buf[i] = ((code >> 12) & 0x0f) as u8 | 0xe0;
                i += 1;
                buf[i] = ((code >> 6) & 0x3f) as u8 | 0x80;
                i += 1;
            } else if (code & 0xffe00000) == 0 {
                buf[i] = ((code >> 18) & 0x07) as u8 | 0xf0;
                i += 1;
                buf[i] = ((code >> 12) & 0x3f) as u8 | 0x80;
                i += 1;
                buf[i] = ((code >> 6) & 0x3f) as u8 | 0x80;
                i += 1;
            } else {
                return ONIGERR_TOO_BIG_WIDE_CHAR_VALUE;
            }
            buf[i] = (code & 0x3f) as u8 | 0x80;
            i += 1;
            i as i32
        }
    }

    fn mbc_case_fold(
        &self,
        flag: OnigCaseFoldType,
        pp: &mut usize,
        end: usize,
        source: &[u8],
        fold_buf: &mut [u8],
    ) -> i32 {
        if source[*pp] < 128 {
            // ASCII range: direct lookup
            fold_buf[0] = onigenc_ascii_code_to_lower_case(source[*pp]);
            *pp += 1;
            1
        } else {
            // Non-ASCII: delegate to Unicode case fold
            crate::unicode::onigenc_unicode_mbc_case_fold(self, flag, pp, end, source, fold_buf)
        }
    }

    fn apply_all_case_fold(
        &self,
        flag: OnigCaseFoldType,
        f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
    ) -> i32 {
        crate::unicode::onigenc_unicode_apply_all_case_fold(flag, f)
    }

    fn get_case_fold_codes_by_str(
        &self,
        flag: OnigCaseFoldType,
        p: &[u8],
        end: usize,
        items: &mut [OnigCaseFoldCodeItem],
    ) -> i32 {
        crate::unicode::onigenc_unicode_get_case_fold_codes_by_str(self, flag, p, end, items)
    }

    fn property_name_to_ctype(&self, p: &[u8]) -> i32 {
        crate::unicode::onigenc_unicode_property_name_to_ctype(p)
    }

    fn is_code_ctype(&self, code: OnigCodePoint, ctype: u32) -> bool {
        crate::unicode::onigenc_unicode_is_code_ctype(code, ctype)
    }

    fn get_ctype_code_range(
        &self,
        ctype: u32,
        sb_out: &mut OnigCodePoint,
    ) -> Option<&'static [OnigCodePoint]> {
        *sb_out = 0x80;
        crate::unicode::onigenc_unicode_ctype_code_range(ctype)
    }

    fn left_adjust_char_head(&self, start: usize, s: usize, data: &[u8]) -> usize {
        if s <= start {
            return s;
        }
        let mut p = s;
        while !utf8_islead(data[p]) && p > start {
            p -= 1;
        }
        p
    }

    fn is_allowed_reverse_match(&self, _p: &[u8]) -> bool {
        true
    }

    fn is_valid_mbc_string(&self, s: &[u8]) -> bool {
        let mut p = 0;
        while p < s.len() {
            if s[p] > 0xf4 || (s[p] > 0x7f && s[p] < 0xc2) {
                return false;
            }
            let len = ENC_LEN_UTF8[s[p] as usize] as usize;
            p += 1;
            if len > 1 {
                for _ in 1..len {
                    if p >= s.len() {
                        return false;
                    }
                    if !utf8_istail(s[p]) {
                        return false;
                    }
                    p += 1;
                }
            }
        }
        true
    }

    fn flag(&self) -> u32 {
        ENC_FLAG_ASCII_COMPATIBLE | ENC_FLAG_UNICODE | ENC_FLAG_SKIP_OFFSET_1_OR_0
    }

    fn sb_range(&self) -> OnigCodePoint {
        0
    }

    fn index(&self) -> i32 {
        0
    }
}
