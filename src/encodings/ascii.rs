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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regcomp::onig_new;
    use crate::regexec::onig_search;
    use crate::regsyntax::OnigSyntaxOniguruma;

    #[test]
    fn reports_single_byte_metadata() {
        let enc = &ONIG_ENCODING_ASCII;
        assert_eq!(enc.name(), "US-ASCII");
        assert_eq!(enc.max_enc_len(), 1);
        assert_eq!(enc.min_enc_len(), 1);
        assert_eq!(enc.mbc_enc_len(b"a"), 1);
        assert_eq!(enc.index(), 0);
        assert_eq!(enc.sb_range(), 0);
        assert_eq!(
            enc.flag(),
            ENC_FLAG_ASCII_COMPATIBLE | ENC_FLAG_SKIP_OFFSET_1
        );
        assert_eq!(enc.init(), ONIG_NORMAL);
        assert!(!enc.is_initialized());
        assert!(enc.is_valid_mbc_string(b"plain ascii"));
        assert!(enc.is_allowed_reverse_match(b"a"));
    }

    #[test]
    fn converts_between_bytes_and_code_points() {
        let enc = &ONIG_ENCODING_ASCII;
        assert_eq!(enc.mbc_to_code(b"A", 1), 0x41);
        assert_eq!(enc.code_to_mbclen(0x41), 1);

        let mut buf = [0u8; 1];
        assert_eq!(enc.code_to_mbc(0x41, &mut buf), 1);
        assert_eq!(&buf, b"A");
    }

    #[test]
    fn detects_newlines_and_adjusts_char_heads() {
        let enc = &ONIG_ENCODING_ASCII;
        assert!(enc.is_mbc_newline(b"\n", 1));
        assert!(!enc.is_mbc_newline(b"a", 1));
        // Every byte is a character head in a single-byte encoding.
        assert_eq!(enc.left_adjust_char_head(0, 3, b"abcd"), 3);
    }

    #[test]
    fn folds_ascii_case() {
        let enc = &ONIG_ENCODING_ASCII;

        let mut pos = 0usize;
        let mut fold_buf = [0u8; 1];
        assert_eq!(
            enc.mbc_case_fold(ONIGENC_CASE_FOLD_MIN, &mut pos, 1, b"A", &mut fold_buf),
            1
        );
        assert_eq!(pos, 1);
        assert_eq!(&fold_buf, b"a");

        let mut items = vec![
            OnigCaseFoldCodeItem {
                byte_len: 0,
                code_len: 0,
                code: [0; ONIGENC_MAX_COMP_CASE_FOLD_CODE_LEN],
            };
            4
        ];
        assert_eq!(
            enc.get_case_fold_codes_by_str(ONIGENC_CASE_FOLD_MIN, b"a", 1, &mut items),
            1
        );
        assert_eq!(items[0].code[0], 0x41);
        assert_eq!(
            enc.get_case_fold_codes_by_str(ONIGENC_CASE_FOLD_MIN, b"1", 1, &mut items),
            0
        );

        let mut pairs = 0usize;
        enc.apply_all_case_fold(ONIGENC_CASE_FOLD_MIN, &mut |_from, _to| {
            pairs += 1;
            0
        });
        assert_eq!(pairs, 52); // 26 letters, both directions
    }

    #[test]
    fn classifies_only_ascii_code_points() {
        let enc = &ONIG_ENCODING_ASCII;
        assert!(enc.is_code_ctype(b'a' as OnigCodePoint, ONIGENC_CTYPE_ALPHA));
        assert!(enc.is_code_ctype(b'7' as OnigCodePoint, ONIGENC_CTYPE_DIGIT));
        assert!(!enc.is_code_ctype(b'7' as OnigCodePoint, ONIGENC_CTYPE_ALPHA));
        // Non-ASCII code points and unknown ctypes never match.
        assert!(!enc.is_code_ctype(0x00E4, ONIGENC_CTYPE_ALPHA));
        assert!(!enc.is_code_ctype(b'a' as OnigCodePoint, ONIGENC_MAX_STD_CTYPE + 1));

        assert_eq!(
            enc.property_name_to_ctype(b"Greek"),
            ONIGERR_INVALID_CHAR_PROPERTY_NAME
        );
        let mut sb_out: OnigCodePoint = 0;
        assert!(enc
            .get_ctype_code_range(ONIGENC_CTYPE_ALPHA, &mut sb_out)
            .is_none());
    }

    #[test]
    fn compiles_and_searches_ascii_patterns() {
        let reg = onig_new(
            b"(?i)ab+c",
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_ASCII,
            &OnigSyntaxOniguruma,
        )
        .unwrap();

        let input = b"xx ABBBc yy";
        let (result, region) = onig_search(
            &reg,
            input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );

        assert_eq!(result, 3);
        let region = region.unwrap();
        assert_eq!(region.beg[0], 3);
        assert_eq!(region.end[0], 8);
    }
}
