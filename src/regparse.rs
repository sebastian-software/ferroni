// regparse.rs - Port of regparse.c
// Parser: converts regex patterns (byte strings) into AST (Node trees).
//
// This is a 1:1 port of oniguruma's regparse.c (~9,500 LOC).
// Structure mirrors the C original: helpers → name table → env management →
// number parsing → code ranges → escape parsing → tokenizer → parser.

#![allow(non_upper_case_globals)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_mut)]

use crate::oniguruma::*;
use crate::regenc::*;
use crate::regint::*;
use crate::regparse_types::*;

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_MAX_CAPTURE_NUM: i32 = 32767;
const DEFAULT_PARSE_DEPTH_LIMIT: u32 = 4096;
const INIT_PARSEENV_MEMENV_ALLOC_SIZE: usize = 16;

// CSTATE: character class parsing state
const CS_VALUE: i32 = 0;
const CS_RANGE: i32 = 1;
const CS_COMPLETE: i32 = 2;
const CS_START: i32 = 3;

// CVAL: character class value type
const CV_UNDEF: i32 = 0;
const CV_SB: i32 = 1;
const CV_MB: i32 = 2;
const CV_CPROP: i32 = 3;

// REF_NUM: reference number type
const IS_NOT_NUM: i32 = 0;
const IS_ABS_NUM: i32 = 1;
const IS_REL_NUM: i32 = 2;

// CPS_STATE
const CPS_EMPTY: i32 = 0;
const CPS_START_VAL: i32 = 1;
const CPS_RANGE: i32 = 2;

const PEND_VALUE: OnigCodePoint = 0;

// ============================================================================
// Global State (matching C module-level statics)
// ============================================================================

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

static MAX_CAPTURE_NUM: AtomicI32 = AtomicI32::new(DEFAULT_MAX_CAPTURE_NUM);
static PARSE_DEPTH_LIMIT: AtomicU32 = AtomicU32::new(DEFAULT_PARSE_DEPTH_LIMIT);

pub fn onig_set_capture_num_limit(num: i32) -> i32 {
    if num < 0 {
        return -1;
    }
    MAX_CAPTURE_NUM.store(num, Ordering::Relaxed);
    0
}

pub fn onig_get_parse_depth_limit() -> u32 {
    PARSE_DEPTH_LIMIT.load(Ordering::Relaxed)
}

pub fn onig_set_parse_depth_limit(depth: u32) -> i32 {
    if depth == 0 {
        PARSE_DEPTH_LIMIT.store(DEFAULT_PARSE_DEPTH_LIMIT, Ordering::Relaxed);
    } else {
        PARSE_DEPTH_LIMIT.store(depth, Ordering::Relaxed);
    }
    0
}

// ============================================================================
// Syntax helper macros (matching C macros IS_SYNTAX_OP, etc.)
// ============================================================================

#[inline]
fn is_syntax_op(syn: &OnigSyntaxType, opm: u32) -> bool {
    (syn.op & opm) != 0
}

#[inline]
fn is_syntax_op2(syn: &OnigSyntaxType, opm: u32) -> bool {
    (syn.op2 & opm) != 0
}

#[inline]
fn is_syntax_bv(syn: &OnigSyntaxType, bvm: u32) -> bool {
    (syn.behavior & bvm) != 0
}

#[inline]
fn mc_esc(syn: &OnigSyntaxType) -> OnigCodePoint {
    syn.meta_char_table.esc
}

#[inline]
fn is_mc_esc_code(code: OnigCodePoint, syn: &OnigSyntaxType) -> bool {
    code == mc_esc(syn) && code != ONIG_INEFFECTIVE_META_CHAR
}

// ============================================================================
// Option helpers (matching C macros OPTON_*)
// ============================================================================

#[inline]
fn opton_singleline(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_SINGLELINE) != 0
}

#[inline]
fn opton_multiline(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_MULTILINE) != 0
}

#[inline]
fn opton_ignorecase(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_IGNORECASE) != 0
}

#[inline]
fn opton_extend(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_EXTEND) != 0
}

#[inline]
fn opton_word_ascii(option: OnigOptionType) -> bool {
    (option & (ONIG_OPTION_WORD_IS_ASCII | ONIG_OPTION_POSIX_IS_ASCII)) != 0
}

#[inline]
fn opton_digit_ascii(option: OnigOptionType) -> bool {
    (option & (ONIG_OPTION_DIGIT_IS_ASCII | ONIG_OPTION_POSIX_IS_ASCII)) != 0
}

#[inline]
fn opton_space_ascii(option: OnigOptionType) -> bool {
    (option & (ONIG_OPTION_SPACE_IS_ASCII | ONIG_OPTION_POSIX_IS_ASCII)) != 0
}

#[inline]
fn opton_posix_ascii(option: OnigOptionType) -> bool {
    (option & ONIG_OPTION_POSIX_IS_ASCII) != 0
}

#[inline]
fn opton_is_ascii_mode_ctype(ctype: i32, options: OnigOptionType) -> bool {
    ctype >= 0
        && ((ctype < ONIGENC_CTYPE_ASCII as i32 && opton_posix_ascii(options))
            || (ctype == ONIGENC_CTYPE_WORD as i32 && opton_word_ascii(options))
            || (ctype == ONIGENC_CTYPE_DIGIT as i32 && opton_digit_ascii(options))
            || (ctype == ONIGENC_CTYPE_SPACE as i32 && opton_space_ascii(options)))
}

// ============================================================================
// Pattern scanning helpers (replacing C macros PFETCH, PPEEK, etc.)
// ============================================================================

// In C: UChar* p, *end, *pfetch_prev with macros.
// In Rust: p is a mutable position index into pattern: &[u8].

#[inline]
fn p_end(p: usize, end: usize) -> bool {
    p >= end
}

#[inline]
fn pfetch(
    p: &mut usize,
    pfetch_prev: &mut usize,
    pattern: &[u8],
    end: usize,
    enc: OnigEncoding,
) -> OnigCodePoint {
    let c = enc.mbc_to_code(&pattern[*p..end], end - *p);
    *pfetch_prev = *p;
    *p += enc.mbc_enc_len(&pattern[*p..end]);
    c
}

#[inline]
fn pfetch_s(
    p: &mut usize,
    pattern: &[u8],
    end: usize,
    enc: OnigEncoding,
) -> OnigCodePoint {
    let c = enc.mbc_to_code(&pattern[*p..end], end - *p);
    *p += enc.mbc_enc_len(&pattern[*p..end]);
    c
}

#[inline]
fn ppeek(p: usize, pattern: &[u8], end: usize, enc: OnigEncoding) -> OnigCodePoint {
    if p < end {
        enc.mbc_to_code(&pattern[p..end], end - p)
    } else {
        PEND_VALUE
    }
}

#[inline]
fn ppeek_is(
    p: usize,
    pattern: &[u8],
    end: usize,
    enc: OnigEncoding,
    c: OnigCodePoint,
) -> bool {
    ppeek(p, pattern, end, enc) == c
}

#[inline]
fn pinc(p: &mut usize, pattern: &[u8], enc: OnigEncoding) {
    *p += enc.mbc_enc_len(&pattern[*p..]);
}

#[inline]
fn enclen(enc: OnigEncoding, p: &[u8]) -> usize {
    enc.mbc_enc_len(p)
}

// ============================================================================
// Character code helpers
// ============================================================================

#[inline]
fn is_code_digit_ascii(_enc: OnigEncoding, c: OnigCodePoint) -> bool {
    c >= '0' as u32 && c <= '9' as u32
}

#[inline]
fn is_code_xdigit_ascii(_enc: OnigEncoding, c: OnigCodePoint) -> bool {
    (c >= '0' as u32 && c <= '9' as u32)
        || (c >= 'a' as u32 && c <= 'f' as u32)
        || (c >= 'A' as u32 && c <= 'F' as u32)
}

#[inline]
fn digitval(c: OnigCodePoint) -> u32 {
    c - '0' as u32
}

#[inline]
fn xdigitval(_enc: OnigEncoding, c: OnigCodePoint) -> u32 {
    if c >= 'a' as u32 && c <= 'f' as u32 {
        c - 'a' as u32 + 10
    } else if c >= 'A' as u32 && c <= 'F' as u32 {
        c - 'A' as u32 + 10
    } else {
        c - '0' as u32
    }
}

#[inline]
fn odigitval(c: OnigCodePoint) -> u32 {
    c - '0' as u32
}

#[inline]
fn is_word_anchor_type(t: i32) -> bool {
    t == ANCR_WORD_BOUNDARY
        || t == ANCR_NO_WORD_BOUNDARY
        || t == ANCR_WORD_BEGIN
        || t == ANCR_WORD_END
}

// ============================================================================
// Misc helpers
// ============================================================================

fn backref_rel_to_abs(rel_no: i32, env: &ParseEnv) -> i32 {
    if rel_no > 0 {
        env.num_mem + rel_no
    } else {
        env.num_mem + 1 + rel_no
    }
}

fn enc_sb_out(enc: OnigEncoding) -> OnigCodePoint {
    if (enc.flag() & ENC_FLAG_UNICODE) != 0 {
        if enc.min_enc_len() == 1 {
            128 + 1
        } else {
            0
        }
    } else {
        0x100
    }
}

fn mbcode_start_pos(enc: OnigEncoding) -> OnigCodePoint {
    if enc.min_enc_len() > 1 {
        0
    } else {
        0x80
    }
}

// ============================================================================
// ParseEnv extensions
// ============================================================================

impl ParseEnv {
    pub fn clear(&mut self) {
        self.cap_history = 0;
        self.backtrack_mem = 0;
        self.backrefed_mem = 0;
        self.error = std::ptr::null();
        self.error_end = std::ptr::null();
        self.num_call = 0;
        self.num_mem = 0;
        self.num_named = 0;
        self.mem_alloc = 0;
        self.mem_env_dynamic = None;
        self.mem_env_static = Default::default();
        self.parse_depth = 0;
        self.backref_num = 0;
        self.keep_num = 0;
        self.id_num = 0;
        self.save_alloc_num = 0;
        self.saves = None;
        self.unset_addr_list = None;
        self.flags = 0;
    }

    pub fn add_mem_entry(&mut self) -> Result<i32, i32> {
        let need = self.num_mem + 1;
        let max_cap = MAX_CAPTURE_NUM.load(Ordering::Relaxed);
        if need > max_cap && max_cap != 0 {
            return Err(ONIGERR_TOO_MANY_CAPTURES);
        }

        if need as usize >= PARSEENV_MEMENV_SIZE {
            if let Some(ref mut dyn_env) = self.mem_env_dynamic {
                if need as usize >= dyn_env.len() {
                    let new_alloc = std::cmp::max(dyn_env.len() * 2, need as usize + 1);
                    dyn_env.resize_with(new_alloc, MemEnv::default);
                }
            } else {
                let alloc = std::cmp::max(INIT_PARSEENV_MEMENV_ALLOC_SIZE, need as usize + 1);
                let mut dyn_env = Vec::with_capacity(alloc);
                // Copy static entries
                for entry in &self.mem_env_static {
                    dyn_env.push(MemEnv {
                        mem_node: entry.mem_node,
                        empty_repeat_node: entry.empty_repeat_node,
                    });
                }
                dyn_env.resize_with(alloc, MemEnv::default);
                self.mem_env_dynamic = Some(dyn_env);
            }
        }

        self.num_mem += 1;
        Ok(self.num_mem)
    }

    pub fn mem_env(&self, num: usize) -> &MemEnv {
        if let Some(ref dyn_env) = self.mem_env_dynamic {
            &dyn_env[num]
        } else {
            &self.mem_env_static[num]
        }
    }

    pub fn mem_env_mut(&mut self, num: usize) -> &mut MemEnv {
        if let Some(ref mut dyn_env) = self.mem_env_dynamic {
            &mut dyn_env[num]
        } else {
            &mut self.mem_env_static[num]
        }
    }

    pub fn set_mem_node(&mut self, num: i32, node: *mut Node) -> i32 {
        if self.num_mem >= num {
            self.mem_env_mut(num as usize).mem_node = node;
            0
        } else {
            ONIGERR_PARSER_BUG
        }
    }

    pub fn set_error_string(&mut self, _ecode: i32, arg: *const u8, arg_end: *const u8) {
        self.error = arg;
        self.error_end = arg_end;
    }

    /// Allocate next id
    pub fn id_entry(&mut self) -> i32 {
        let id = self.id_num;
        self.id_num += 1;
        id
    }
}

// ============================================================================
// Number scanning
// ============================================================================

fn scan_number(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    enc: OnigEncoding,
) -> i32 {
    let mut num: i32 = 0;
    let mut pfetch_prev = *p;
    while !p_end(*p, end) {
        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        if is_code_digit_ascii(enc, c) {
            let val = digitval(c) as i32;
            if (i32::MAX - val) / 10 < num {
                return -1; // overflow
            }
            num = num * 10 + val;
        } else {
            *p = pfetch_prev; // PUNFETCH
            break;
        }
    }
    num
}

fn scan_hexadecimal_number(
    p: &mut usize,
    end: usize,
    minlen: i32,
    maxlen: i32,
    pattern: &[u8],
    enc: OnigEncoding,
    rcode: &mut OnigCodePoint,
) -> i32 {
    let mut code: OnigCodePoint = 0;
    let mut n: i32 = 0;
    let mut pfetch_prev = *p;
    while !p_end(*p, end) && n < maxlen {
        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        if is_code_xdigit_ascii(enc, c) {
            n += 1;
            let val = xdigitval(enc, c);
            if (u32::MAX - val) / 16 < code {
                return ONIGERR_TOO_BIG_NUMBER;
            }
            code = (code << 4) + val;
        } else {
            *p = pfetch_prev; // PUNFETCH
            break;
        }
    }
    if n < minlen {
        return ONIGERR_INVALID_CODE_POINT_VALUE;
    }
    *rcode = code;
    ONIG_NORMAL
}

fn scan_octal_number(
    p: &mut usize,
    end: usize,
    minlen: i32,
    maxlen: i32,
    pattern: &[u8],
    enc: OnigEncoding,
    rcode: &mut OnigCodePoint,
) -> i32 {
    let mut code: OnigCodePoint = 0;
    let mut n: i32 = 0;
    let mut pfetch_prev = *p;
    while !p_end(*p, end) && n < maxlen {
        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        if is_code_digit_ascii(enc, c) && c < '8' as u32 {
            n += 1;
            let val = odigitval(c);
            if (u32::MAX - val) / 8 < code {
                return ONIGERR_TOO_BIG_NUMBER;
            }
            code = (code << 3) + val;
        } else {
            *p = pfetch_prev; // PUNFETCH
            break;
        }
    }
    if n < minlen {
        return ONIGERR_INVALID_CODE_POINT_VALUE;
    }
    *rcode = code;
    ONIG_NORMAL
}

// ============================================================================
// Code range operations (BBuf-based multi-byte ranges)
// ============================================================================

const SIZE_CODE_POINT: usize = std::mem::size_of::<OnigCodePoint>();

fn bbuf_write_code_point(bbuf: &mut BBuf, pos: usize, code: OnigCodePoint) {
    let bytes = code.to_ne_bytes();
    if pos + SIZE_CODE_POINT <= bbuf.data.len() {
        bbuf.data[pos..pos + SIZE_CODE_POINT].copy_from_slice(&bytes);
    } else {
        bbuf.data.resize(pos + SIZE_CODE_POINT, 0);
        bbuf.data[pos..pos + SIZE_CODE_POINT].copy_from_slice(&bytes);
    }
}

fn bbuf_read_code_point(bbuf: &BBuf, pos: usize) -> OnigCodePoint {
    let mut bytes = [0u8; SIZE_CODE_POINT];
    bytes.copy_from_slice(&bbuf.data[pos..pos + SIZE_CODE_POINT]);
    OnigCodePoint::from_ne_bytes(bytes)
}

fn new_code_range() -> BBuf {
    let mut bbuf = BBuf::with_capacity(SIZE_CODE_POINT * 5);
    bbuf_write_code_point(&mut bbuf, 0, 0); // n = 0
    bbuf
}

fn add_code_range_to_buf(
    pbuf: &mut Option<BBuf>,
    from: OnigCodePoint,
    to: OnigCodePoint,
) -> i32 {
    let mut from = from;
    let mut to = to;
    if from > to {
        std::mem::swap(&mut from, &mut to);
    }

    if pbuf.is_none() {
        *pbuf = Some(new_code_range());
    }
    let bbuf = pbuf.as_mut().unwrap();
    let n = bbuf_read_code_point(bbuf, 0) as usize;

    // Read existing range data
    let mut data = Vec::with_capacity(n * 2);
    for i in 0..n * 2 {
        data.push(bbuf_read_code_point(bbuf, SIZE_CODE_POINT * (1 + i)));
    }

    // Binary search for insertion point
    let mut low = 0usize;
    let mut bound = n;
    while low < bound {
        let x = (low + bound) >> 1;
        if from > data[x * 2 + 1] {
            low = x + 1;
        } else {
            bound = x;
        }
    }

    let mut high = if to == u32::MAX { n } else { low };
    bound = n;
    while high < bound {
        let x = (high + bound) >> 1;
        if to + 1 >= data[x * 2] {
            high = x + 1;
        } else {
            bound = x;
        }
    }

    let inc_n: i32 = low as i32 + 1 - high as i32;
    if (n as i32 + inc_n) > ONIG_MAX_MULTI_BYTE_RANGES_NUM {
        return ONIGERR_TOO_MANY_MULTI_BYTE_RANGES;
    }

    if inc_n != 1 {
        if low < data.len() / 2 && from > data[low * 2] {
            from = data[low * 2];
        }
        if high > 0 && high - 1 < data.len() / 2 && to < data[(high - 1) * 2 + 1] {
            to = data[(high - 1) * 2 + 1];
        }
    }

    // Rebuild the data array with the merged range
    let new_n = (n as i32 + inc_n) as usize;
    let mut new_data = Vec::with_capacity(new_n * 2);
    for i in 0..low {
        new_data.push(data[i * 2]);
        new_data.push(data[i * 2 + 1]);
    }
    new_data.push(from);
    new_data.push(to);
    for i in high..n {
        new_data.push(data[i * 2]);
        new_data.push(data[i * 2 + 1]);
    }

    // Write back to bbuf
    let total_size = SIZE_CODE_POINT * (1 + new_n * 2);
    bbuf.data.resize(total_size, 0);
    bbuf_write_code_point(bbuf, 0, new_n as OnigCodePoint);
    for i in 0..new_data.len() {
        bbuf_write_code_point(bbuf, SIZE_CODE_POINT * (1 + i), new_data[i]);
    }

    0
}

fn add_code_range(
    pbuf: &mut Option<BBuf>,
    env: &ParseEnv,
    from: OnigCodePoint,
    to: OnigCodePoint,
) -> i32 {
    if from > to {
        if is_syntax_bv(env.syntax, ONIG_SYN_ALLOW_EMPTY_RANGE_IN_CC) {
            return 0;
        } else {
            return ONIGERR_EMPTY_RANGE_IN_CHAR_CLASS;
        }
    }
    add_code_range_to_buf(pbuf, from, to)
}

// ============================================================================
// Character class helpers
// ============================================================================

fn add_ctype_to_cc_by_range(
    cc: &mut CClassNode,
    ctype: i32,
    not: bool,
    enc: OnigEncoding,
    sb_out: OnigCodePoint,
) -> i32 {
    let mut r: i32;
    let range_opt = enc.get_ctype_code_range(ctype as u32, &mut 0);
    if range_opt.is_none() {
        return ONIGERR_TYPE_BUG;
    }
    let range = range_opt.unwrap();

    let n = range.len() / 2;
    if not {
        // Inverted: add everything NOT in the ranges
        let mut prev = 0u32;
        for i in 0..n {
            let from = range[i * 2];
            let to = range[i * 2 + 1];
            if prev < from {
                if prev < sb_out {
                    let end = std::cmp::min(from - 1, sb_out - 1);
                    bitset_set_range(&mut cc.bs, prev as usize, end as usize);
                }
                if from > sb_out {
                    r = add_code_range_to_buf(&mut cc.mbuf, prev, from - 1);
                    if r != 0 {
                        return r;
                    }
                }
            }
            prev = to + 1;
        }
        if prev < sb_out {
            bitset_set_range(&mut cc.bs, prev as usize, (sb_out - 1) as usize);
        }
        if prev < u32::MAX {
            r = add_code_range_to_buf(&mut cc.mbuf, prev, u32::MAX);
            if r != 0 {
                return r;
            }
        }
    } else {
        for i in 0..n {
            let from = range[i * 2];
            let to = range[i * 2 + 1];
            if from < sb_out {
                let end = std::cmp::min(to, sb_out - 1);
                bitset_set_range(&mut cc.bs, from as usize, end as usize);
            }
            if to >= sb_out {
                let start = std::cmp::max(from, sb_out);
                r = add_code_range_to_buf(&mut cc.mbuf, start, to);
                if r != 0 {
                    return r;
                }
            }
        }
    }

    ONIG_NORMAL
}

fn add_ctype_to_cc(
    cc: &mut CClassNode,
    ctype: i32,
    not: bool,
    env: &ParseEnv,
) -> i32 {
    let enc = env.enc;
    let mut sb_out: OnigCodePoint = 0;
    let range = enc.get_ctype_code_range(ctype as u32, &mut sb_out);
    if let Some(_) = range {
        return add_ctype_to_cc_by_range(cc, ctype, not, enc, sb_out);
    }

    // Fallback: iterate over single-byte range
    let max_code = if enc.min_enc_len() > 1 { 0x80 } else { SINGLE_BYTE_SIZE as OnigCodePoint };
    for c in 0..max_code {
        if enc.is_code_ctype(c, ctype as u32) {
            if not {
                // Don't set bit
            } else {
                if (c as usize) < SINGLE_BYTE_SIZE {
                    bitset_set_bit(&mut cc.bs, c as usize);
                }
            }
        } else {
            if not {
                if (c as usize) < SINGLE_BYTE_SIZE {
                    bitset_set_bit(&mut cc.bs, c as usize);
                }
            }
        }
    }

    ONIG_NORMAL
}

/// Adds code point to character class (bitset or mbuf)
fn add_code_into_cc(cc: &mut CClassNode, code: OnigCodePoint, enc: OnigEncoding) {
    if code < SINGLE_BYTE_SIZE as u32 {
        bitset_set_bit(&mut cc.bs, code as usize);
    } else {
        add_code_range_to_buf(&mut cc.mbuf, code, code);
    }
}

// ============================================================================
// Escape parsing
// ============================================================================

fn conv_backslash_value(c: OnigCodePoint, env: &ParseEnv) -> OnigCodePoint {
    if is_syntax_op(env.syntax, ONIG_SYN_OP_ESC_CONTROL_CHARS) {
        match c {
            0x6E => return '\n' as u32, // 'n'
            0x74 => return '\t' as u32, // 't'
            0x72 => return '\r' as u32, // 'r'
            0x66 => return 0x0C,        // 'f' -> form feed
            0x61 => return 0x07,        // 'a' -> bell
            0x62 => return 0x08,        // 'b' -> backspace
            0x65 => return 0x1B,        // 'e' -> escape
            0x76 => {
                // 'v'
                if is_syntax_op2(env.syntax, ONIG_SYN_OP2_ESC_V_VTAB) {
                    return 0x0B; // vertical tab
                }
            }
            _ => {}
        }
    }
    c
}

fn fetch_escaped_value_raw(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
) -> Result<OnigCodePoint, i32> {
    let enc = env.enc;
    if p_end(*p, end) {
        return Err(ONIGERR_END_PATTERN_AT_ESCAPE);
    }

    let c = pfetch_s(p, pattern, end, enc);
    match c {
        0x4D => {
            // 'M'
            if is_syntax_op2(env.syntax, ONIG_SYN_OP2_ESC_CAPITAL_M_BAR_META) {
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_AT_META);
                }
                let c2 = pfetch_s(p, pattern, end, enc);
                if c2 != '-' as u32 {
                    return Err(ONIGERR_META_CODE_SYNTAX);
                }
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_AT_META);
                }
                let c3 = pfetch_s(p, pattern, end, enc);
                let val = if c3 == mc_esc(env.syntax) {
                    fetch_escaped_value_raw(p, end, pattern, env)?
                } else {
                    c3
                };
                return Ok((val & 0xff) | 0x80);
            }
            Ok(conv_backslash_value(c, env))
        }
        0x43 => {
            // 'C'
            if is_syntax_op2(env.syntax, ONIG_SYN_OP2_ESC_CAPITAL_C_BAR_CONTROL) {
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_AT_CONTROL);
                }
                let c2 = pfetch_s(p, pattern, end, enc);
                if c2 != '-' as u32 {
                    return Err(ONIGERR_CONTROL_CODE_SYNTAX);
                }
                // fall through to control handling
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_AT_CONTROL);
                }
                let c3 = pfetch_s(p, pattern, end, enc);
                if c3 == '?' as u32 {
                    return Ok(0x7F);
                }
                let val = if c3 == mc_esc(env.syntax) {
                    fetch_escaped_value_raw(p, end, pattern, env)?
                } else {
                    c3
                };
                return Ok(val & 0x9f);
            }
            Ok(conv_backslash_value(c, env))
        }
        0x63 => {
            // 'c'
            if is_syntax_op(env.syntax, ONIG_SYN_OP_ESC_C_CONTROL) {
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_AT_CONTROL);
                }
                let c2 = pfetch_s(p, pattern, end, enc);
                if c2 == '?' as u32 {
                    return Ok(0x7F);
                }
                let val = if c2 == mc_esc(env.syntax) {
                    fetch_escaped_value_raw(p, end, pattern, env)?
                } else {
                    c2
                };
                return Ok(val & 0x9f);
            }
            Ok(conv_backslash_value(c, env))
        }
        _ => Ok(conv_backslash_value(c, env)),
    }
}

fn fetch_escaped_value(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
) -> Result<OnigCodePoint, i32> {
    let val = fetch_escaped_value_raw(p, end, pattern, env)?;
    let len = env.enc.code_to_mbclen(val);
    if len < 0 {
        return Err(len);
    }
    Ok(val)
}

// ============================================================================
// Quantifier helpers
// ============================================================================

fn is_invalid_quantifier_target(node: &Node) -> bool {
    match node.node_type() {
        NodeType::Anchor | NodeType::Gimmick => true,
        NodeType::Bag => false,
        NodeType::List => {
            // Check all elements
            let mut n = node;
            loop {
                if let Some(cons) = n.as_cons() {
                    if !is_invalid_quantifier_target(&cons.car) {
                        return false;
                    }
                    match &cons.cdr {
                        Some(next) => n = next,
                        None => break,
                    }
                } else {
                    break;
                }
            }
            false
        }
        NodeType::Alt => {
            let mut n = node;
            loop {
                if let Some(cons) = n.as_cons() {
                    if is_invalid_quantifier_target(&cons.car) {
                        return true;
                    }
                    match &cons.cdr {
                        Some(next) => n = next,
                        None => break,
                    }
                } else {
                    break;
                }
            }
            false
        }
        _ => false,
    }
}

fn quantifier_type_num(q: &QuantNode) -> i32 {
    if q.greedy {
        if q.lower == 0 {
            if q.upper == 1 {
                return 0;
            } else if q.upper == INFINITE_REPEAT {
                return 1;
            }
        } else if q.lower == 1 && q.upper == INFINITE_REPEAT {
            return 2;
        }
    } else {
        if q.lower == 0 {
            if q.upper == 1 {
                return 3;
            } else if q.upper == INFINITE_REPEAT {
                return 4;
            }
        } else if q.lower == 1 && q.upper == INFINITE_REPEAT {
            return 5;
        }
    }
    -1
}

// ============================================================================
// Tokenizer: fetch_interval
// ============================================================================

fn fetch_interval(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    tok: &mut PToken,
    env: &ParseEnv,
) -> i32 {
    let enc = env.enc;
    let syn = env.syntax;
    let mut pfetch_prev = *p;
    let mut non_low = false;
    let syn_allow = is_syntax_bv(syn, ONIG_SYN_ALLOW_INVALID_INTERVAL);
    let save_p = *p;

    if p_end(*p, end) {
        return if syn_allow { 1 } else { ONIGERR_END_PATTERN_AT_LEFT_BRACE };
    }

    if !syn_allow {
        let c = ppeek(*p, pattern, end, enc);
        if c == ')' as u32 || c == '(' as u32 || c == '|' as u32 {
            return ONIGERR_END_PATTERN_AT_LEFT_BRACE;
        }
    }

    let mut low = scan_number(p, end, pattern, enc);
    if low < 0 {
        return ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE;
    }
    if low > ONIG_MAX_REPEAT_NUM {
        return ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE;
    }

    if *p == save_p {
        // Can't read low
        if is_syntax_bv(syn, ONIG_SYN_ALLOW_INTERVAL_LOW_ABBREV) {
            low = 0;
            non_low = true;
        } else {
            // invalid
            return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
        }
    }

    if p_end(*p, end) {
        return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
    }

    let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
    let mut up;
    let mut r;
    if c == ',' as u32 {
        let prev_p = *p;
        up = scan_number(p, end, pattern, enc);
        if up < 0 {
            return ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE;
        }
        if up > ONIG_MAX_REPEAT_NUM {
            return ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE;
        }
        if *p == prev_p {
            if non_low {
                return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
            }
            up = INFINITE_REPEAT;
        }
    } else {
        if non_low {
            return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
        }
        *p = pfetch_prev; // PUNFETCH
        up = low;
        r = 2; // fixed {n}
    }

    if p_end(*p, end) {
        return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
    }

    let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
    if is_syntax_op(syn, ONIG_SYN_OP_ESC_BRACE_INTERVAL) {
        if c != mc_esc(syn) || p_end(*p, end) {
            return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
        }
        let c2 = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        if c2 != '}' as u32 {
            return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
        }
    } else {
        if c != '}' as u32 {
            return if syn_allow { 1 } else { ONIGERR_INVALID_REPEAT_RANGE_PATTERN };
        }
    }

    if up != INFINITE_REPEAT && low > up {
        return ONIGERR_UPPER_SMALLER_THAN_LOWER_IN_REPEAT_RANGE;
    }

    tok.repeat_possessive = false;
    tok.token_type = TokenType::Interval;
    tok.repeat_lower = low;
    tok.repeat_upper = up;
    if up == low {
        r = 2; // fixed
    } else {
        r = 0; // normal
    }

    r
}

// ============================================================================
// Tokenizer: fetch_token
// ============================================================================

fn fetch_token(
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
) -> i32 {
    let enc = env.enc;
    let syn = env.syntax;
    let mut pfetch_prev = *p;

    if tok.code_point_continue {
        tok.code_point_continue = false;
    }

    if p_end(*p, end) {
        tok.token_type = TokenType::Eot;
        return tok.token_type as i32;
    }

    tok.token_type = TokenType::String;
    tok.base_num = 0;
    tok.backp = *p;

    let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);

    if is_mc_esc_code(c, syn) {
        if p_end(*p, end) {
            return ONIGERR_END_PATTERN_AT_ESCAPE;
        }

        tok.backp = *p;
        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        tok.code = c;
        tok.escaped = true;

        match c as u8 as char {
            '*' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_ASTERISK_ZERO_INF) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Repeat;
                tok.repeat_lower = 0;
                tok.repeat_upper = INFINITE_REPEAT;
                tok.repeat_possessive = false;
                return greedy_check(tok, p, end, pattern, enc, syn);
            }
            '+' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_PLUS_ONE_INF) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Repeat;
                tok.repeat_lower = 1;
                tok.repeat_upper = INFINITE_REPEAT;
                tok.repeat_possessive = false;
                return greedy_check(tok, p, end, pattern, enc, syn);
            }
            '?' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_QMARK_ZERO_ONE) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Repeat;
                tok.repeat_lower = 0;
                tok.repeat_upper = 1;
                tok.repeat_possessive = false;
                return greedy_check(tok, p, end, pattern, enc, syn);
            }
            '{' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_BRACE_INTERVAL) {
                    return tok.token_type as i32;
                }
                let r = fetch_interval(p, end, pattern, tok, env);
                if r < 0 {
                    return r;
                }
                if r == 0 || r == 2 {
                    return greedy_check2(tok, p, end, pattern, enc, syn);
                }
                // r == 1: normal char
            }
            '|' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_VBAR_ALT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Alt;
            }
            '(' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_LPAREN_SUBEXP) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::SubexpOpen;
            }
            ')' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_LPAREN_SUBEXP) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::SubexpClose;
            }
            'w' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_W_WORD) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_WORD as i32;
                tok.prop_not = false;
            }
            'W' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_W_WORD) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_WORD as i32;
                tok.prop_not = true;
            }
            'b' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_B_WORD_BOUND) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = ANCR_WORD_BOUNDARY;
            }
            'B' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_B_WORD_BOUND) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = ANCR_NO_WORD_BOUNDARY;
            }
            's' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_S_WHITE_SPACE) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_SPACE as i32;
                tok.prop_not = false;
            }
            'S' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_S_WHITE_SPACE) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_SPACE as i32;
                tok.prop_not = true;
            }
            'd' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_D_DIGIT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_DIGIT as i32;
                tok.prop_not = false;
            }
            'D' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_D_DIGIT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_DIGIT as i32;
                tok.prop_not = true;
            }
            'h' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_H_XDIGIT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_XDIGIT as i32;
                tok.prop_not = false;
            }
            'H' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_H_XDIGIT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_XDIGIT as i32;
                tok.prop_not = true;
            }
            'K' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_CAPITAL_K_KEEP) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Keep;
            }
            'R' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_CAPITAL_R_GENERAL_NEWLINE) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::GeneralNewline;
            }
            'N' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_CAPITAL_N_O_SUPER_DOT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::NoNewline;
            }
            'O' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_CAPITAL_N_O_SUPER_DOT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::TrueAnychar;
            }
            'X' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_X_Y_TEXT_SEGMENT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::TextSegment;
            }
            'A' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_AZ_BUF_ANCHOR) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = ANCR_BEGIN_BUF;
            }
            'Z' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_AZ_BUF_ANCHOR) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = ANCR_SEMI_END_BUF;
            }
            'z' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_AZ_BUF_ANCHOR) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = ANCR_END_BUF;
            }
            'G' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ESC_CAPITAL_G_BEGIN_ANCHOR) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = ANCR_BEGIN_POSITION;
            }
            'Q' => {
                if is_syntax_op2(syn, ONIG_SYN_OP2_ESC_CAPITAL_Q_QUOTE) {
                    tok.token_type = TokenType::QuoteOpen;
                }
            }
            'p' | 'P' => {
                if !p_end(*p, end) && ppeek_is(*p, pattern, end, enc, '{' as u32) {
                    if is_syntax_op2(syn, ONIG_SYN_OP2_ESC_P_BRACE_CHAR_PROPERTY) {
                        pinc(p, pattern, enc); // skip '{'
                        tok.token_type = TokenType::CharProperty;
                        tok.prop_not = c == 'P' as u32;
                        tok.prop_braces = true;

                        if !p_end(*p, end)
                            && is_syntax_op2(
                                syn,
                                ONIG_SYN_OP2_ESC_P_BRACE_CIRCUMFLEX_NOT,
                            )
                        {
                            let c2 = pfetch(p, &mut pfetch_prev, pattern, end, enc);
                            if c2 == '^' as u32 {
                                tok.prop_not = !tok.prop_not;
                            } else {
                                *p = pfetch_prev; // PUNFETCH
                            }
                        }
                    }
                }
            }
            'x' => {
                let prev = *p;
                if !p_end(*p, end)
                    && ppeek_is(*p, pattern, end, enc, '{' as u32)
                    && is_syntax_op(syn, ONIG_SYN_OP_ESC_X_BRACE_HEX8)
                {
                    pinc(p, pattern, enc); // skip '{'
                    let mut code = 0;
                    let r = scan_hexadecimal_number(p, end, 0, 8, pattern, enc, &mut code);
                    if r < 0 {
                        return r;
                    }
                    if *p > prev + enclen(enc, &pattern[prev..]) {
                        if p_end(*p, end) {
                            return ONIGERR_INVALID_CODE_POINT_VALUE;
                        }
                        if ppeek_is(*p, pattern, end, enc, '}' as u32) {
                            pinc(p, pattern, enc);
                        } else {
                            // TODO: check_code_point_sequence
                            return ONIGERR_INVALID_CODE_POINT_VALUE;
                        }
                        tok.token_type = TokenType::CodePoint;
                        tok.code = code;
                    } else {
                        *p = prev;
                    }
                } else if is_syntax_op(syn, ONIG_SYN_OP_ESC_X_HEX2) {
                    let mut code = 0;
                    let r = scan_hexadecimal_number(p, end, 0, 2, pattern, enc, &mut code);
                    if r < 0 {
                        return r;
                    }
                    if *p == prev {
                        code = 0;
                    }
                    tok.token_type = TokenType::CrudeByte;
                    tok.base_num = 16;
                    tok.code = code;
                }
            }
            'u' => {
                if is_syntax_op2(syn, ONIG_SYN_OP2_ESC_U_HEX4) {
                    let mut code = 0;
                    let r = scan_hexadecimal_number(p, end, 4, 4, pattern, enc, &mut code);
                    if r < 0 {
                        return r;
                    }
                    tok.token_type = TokenType::CodePoint;
                    tok.base_num = 16;
                    tok.code = code;
                }
            }
            'o' => {
                let prev = *p;
                if !p_end(*p, end)
                    && ppeek_is(*p, pattern, end, enc, '{' as u32)
                    && is_syntax_op(syn, ONIG_SYN_OP_ESC_O_BRACE_OCTAL)
                {
                    pinc(p, pattern, enc); // skip '{'
                    let mut code = 0;
                    let r = scan_octal_number(p, end, 0, 11, pattern, enc, &mut code);
                    if r < 0 {
                        return r;
                    }
                    if *p > prev + enclen(enc, &pattern[prev..]) {
                        if p_end(*p, end) {
                            return ONIGERR_INVALID_CODE_POINT_VALUE;
                        }
                        if ppeek_is(*p, pattern, end, enc, '}' as u32) {
                            pinc(p, pattern, enc);
                        } else {
                            return ONIGERR_INVALID_CODE_POINT_VALUE;
                        }
                        tok.token_type = TokenType::CodePoint;
                        tok.code = code;
                    } else {
                        *p = prev;
                    }
                }
            }
            '1'..='9' => {
                *p = pfetch_prev; // PUNFETCH
                let prev = *p;
                let r = scan_number(p, end, pattern, enc);
                if r >= 0
                    && r <= ONIG_MAX_BACKREF_NUM
                    && is_syntax_op(syn, ONIG_SYN_OP_DECIMAL_BACKREF)
                    && (r <= env.num_mem || r <= 9)
                {
                    tok.token_type = TokenType::Backref;
                    tok.backref_num = 1;
                    tok.backref_ref1 = r;
                    tok.backref_by_name = false;
                    tok.backref_exist_level = false;
                    tok.backref_level = 0;
                } else {
                    // fall through to octal
                    *p = prev;
                    let cc = c as u8 as char;
                    if cc == '8' || cc == '9' {
                        *p = prev;
                        pinc(p, pattern, enc);
                    } else {
                        // octal
                        if is_syntax_op(syn, ONIG_SYN_OP_ESC_OCTAL3) {
                            let mut code = 0;
                            let r = scan_octal_number(p, end, 0, 3, pattern, enc, &mut code);
                            if r < 0 || code >= 256 {
                                return ONIGERR_TOO_BIG_NUMBER;
                            }
                            tok.token_type = TokenType::CrudeByte;
                            tok.base_num = 8;
                            tok.code = code;
                        }
                    }
                }
            }
            '0' => {
                if is_syntax_op(syn, ONIG_SYN_OP_ESC_OCTAL3) {
                    let prev = *p;
                    let mut code = 0;
                    let r = scan_octal_number(p, end, 0, 2, pattern, enc, &mut code);
                    if r < 0 || code >= 256 {
                        return ONIGERR_TOO_BIG_NUMBER;
                    }
                    if *p == prev {
                        code = 0;
                    }
                    tok.token_type = TokenType::CrudeByte;
                    tok.base_num = 8;
                    tok.code = code;
                }
            }
            _ => {
                *p = pfetch_prev; // PUNFETCH
                let c2 = match fetch_escaped_value(p, end, pattern, env) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if tok.code != c2 {
                    tok.token_type = TokenType::CodePoint;
                    tok.code = c2;
                } else {
                    *p = tok.backp + enclen(enc, &pattern[tok.backp..]);
                }
            }
        }
    } else {
        tok.code = c;
        tok.escaped = false;

        match c as u8 as char {
            '.' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_DOT_ANYCHAR) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::AnyChar;
            }
            '*' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_ASTERISK_ZERO_INF) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Repeat;
                tok.repeat_lower = 0;
                tok.repeat_upper = INFINITE_REPEAT;
                tok.repeat_possessive = false;
                return greedy_check(tok, p, end, pattern, enc, syn);
            }
            '+' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_PLUS_ONE_INF) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Repeat;
                tok.repeat_lower = 1;
                tok.repeat_upper = INFINITE_REPEAT;
                tok.repeat_possessive = false;
                return greedy_check(tok, p, end, pattern, enc, syn);
            }
            '?' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_QMARK_ZERO_ONE) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Repeat;
                tok.repeat_lower = 0;
                tok.repeat_upper = 1;
                tok.repeat_possessive = false;
                return greedy_check(tok, p, end, pattern, enc, syn);
            }
            '{' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_BRACE_INTERVAL) {
                    return tok.token_type as i32;
                }
                let r = fetch_interval(p, end, pattern, tok, env);
                if r < 0 {
                    return r;
                }
                if r == 0 || r == 2 {
                    return greedy_check2(tok, p, end, pattern, enc, syn);
                }
                // r == 1: normal char
            }
            '|' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_VBAR_ALT) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Alt;
            }
            '(' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_LPAREN_SUBEXP) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::SubexpOpen;
            }
            ')' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_LPAREN_SUBEXP) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::SubexpClose;
            }
            '^' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_LINE_ANCHOR) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = if opton_singleline(env.options) {
                    ANCR_BEGIN_BUF
                } else {
                    ANCR_BEGIN_LINE
                };
            }
            '$' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_LINE_ANCHOR) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::Anchor;
                tok.anchor = if opton_singleline(env.options) {
                    ANCR_SEMI_END_BUF
                } else {
                    ANCR_END_LINE
                };
            }
            '[' => {
                if !is_syntax_op(syn, ONIG_SYN_OP_BRACKET_CC) {
                    return tok.token_type as i32;
                }
                tok.token_type = TokenType::OpenCC;
            }
            ']' => {
                // Normally this is handled in CC context.
                // Outside CC: warn or treat as literal
            }
            '#' => {
                if opton_extend(env.options) {
                    // Skip comment to end of line
                    while !p_end(*p, end) {
                        let c2 = pfetch(p, &mut pfetch_prev, pattern, end, enc);
                        if c2 == '\n' as u32 || c2 == '\r' as u32 {
                            break;
                        }
                    }
                    // goto start - re-enter the tokenizer
                    return fetch_token(tok, p, end, pattern, env);
                }
            }
            ' ' | '\t' | '\n' | '\r' => {
                if opton_extend(env.options) {
                    // Skip whitespace
                    return fetch_token(tok, p, end, pattern, env);
                }
            }
            _ => {}
        }
    }

    tok.token_type as i32
}

fn greedy_check(
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    enc: OnigEncoding,
    syn: &OnigSyntaxType,
) -> i32 {
    if !p_end(*p, end)
        && ppeek_is(*p, pattern, end, enc, '?' as u32)
        && is_syntax_op(syn, ONIG_SYN_OP_QMARK_NON_GREEDY)
        && !tok.repeat_possessive
    {
        let mut pfetch_prev = *p;
        pfetch(p, &mut pfetch_prev, pattern, end, enc); // consume '?'
        tok.repeat_greedy = false;
        tok.repeat_possessive = false;
    } else {
        tok.repeat_greedy = true;
        if !p_end(*p, end)
            && ppeek_is(*p, pattern, end, enc, '+' as u32)
            && is_syntax_op2(syn, ONIG_SYN_OP2_PLUS_POSSESSIVE_REPEAT)
            && tok.token_type != TokenType::Interval
            && !tok.repeat_possessive
        {
            let mut pfetch_prev = *p;
            pfetch(p, &mut pfetch_prev, pattern, end, enc); // consume '+'
            tok.repeat_possessive = true;
        }
    }
    tok.token_type as i32
}

fn greedy_check2(
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    enc: OnigEncoding,
    syn: &OnigSyntaxType,
) -> i32 {
    greedy_check(tok, p, end, pattern, enc, syn)
}

// ============================================================================
// Main Parser: recursive descent
// ============================================================================

/// Parse a single expression element (atom + quantifier)
fn prs_exp(
    tok: &mut PToken,
    term: i32,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
    group_head: bool,
) -> Result<(Box<Node>, i32), i32> {
    let mut group = 0;

    if tok.token_type as i32 == term {
        return Ok((node_new_empty(), tok.token_type as i32));
    }

    let parse_depth = env.parse_depth;

    let node: Box<Node> = match tok.token_type {
        TokenType::Alt | TokenType::Eot => {
            return Ok((node_new_empty(), tok.token_type as i32));
        }
        TokenType::SubexpOpen => {
            // TODO: prs_bag implementation
            return Err(ONIGERR_PARSER_BUG);
        }
        TokenType::SubexpClose => {
            if !is_syntax_bv(env.syntax, ONIG_SYN_ALLOW_UNMATCHED_CLOSE_SUBEXP) {
                return Err(ONIGERR_UNMATCHED_CLOSE_PARENTHESIS);
            }
            // Treat as literal byte
            node_new_str(&pattern[tok.backp..*p])
        }
        TokenType::String => {
            let mut np = node_new_str(&pattern[tok.backp..*p]);
            if opton_ignorecase(env.options) {
                np.status_add(ND_ST_IGNORECASE);
            }
            // Collect consecutive string tokens
            loop {
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                if tok.token_type != TokenType::String {
                    break;
                }
                node_str_cat(&mut np, &pattern[tok.backp..*p]);
            }
            // Check for quantifier
            return check_quantifier(np, tok, p, end, pattern, env, group, parse_depth);
        }
        TokenType::CrudeByte => {
            let byte = tok.code as u8;
            let np = node_new_str_crude(&[byte]);
            np
        }
        TokenType::CodePoint => {
            let mut buf = [0u8; ONIGENC_CODE_TO_MBC_MAXLEN];
            let len = env.enc.code_to_mbclen(tok.code);
            if len < 0 {
                return Err(len);
            }
            let len = env.enc.code_to_mbc(tok.code, &mut buf);
            if len < 0 {
                return Err(len);
            }
            let mut np = node_new_str(&buf[..len as usize]);
            if opton_ignorecase(env.options) {
                np.status_add(ND_ST_IGNORECASE);
            }
            np
        }
        TokenType::AnyChar => {
            let mut np = node_new_ctype(CTYPE_ANYCHAR, false, false);
            if opton_multiline(env.options) {
                np.status_add(ND_ST_MULTILINE);
            }
            np
        }
        TokenType::CharType => {
            let ctype = tok.prop_ctype;
            let not = tok.prop_not;
            if ctype == ONIGENC_CTYPE_WORD as i32 {
                let ascii_mode = opton_is_ascii_mode_ctype(ctype, env.options);
                node_new_ctype(ctype, not, ascii_mode)
            } else {
                // SPACE, DIGIT, XDIGIT -> build character class
                let mut np = node_new_cclass();
                if let Some(cc) = np.as_cclass_mut() {
                    let r = add_ctype_to_cc(cc, ctype, false, env);
                    if r != 0 {
                        return Err(r);
                    }
                    if not {
                        cc.set_not();
                    }
                }
                np
            }
        }
        TokenType::CharProperty => {
            // TODO: prs_char_property
            return Err(ONIGERR_PARSER_BUG);
        }
        TokenType::OpenCC => {
            // TODO: prs_cc (character class parsing)
            return Err(ONIGERR_PARSER_BUG);
        }
        TokenType::Anchor => {
            let ascii_mode = opton_word_ascii(env.options) && is_word_anchor_type(tok.anchor);
            let mut np = node_new_anchor(tok.anchor);
            if let Some(an) = np.as_anchor_mut() {
                an.ascii_mode = ascii_mode;
            }
            np
        }
        TokenType::Backref => {
            let back_num = tok.backref_num;
            let refs = if back_num == 1 {
                vec![tok.backref_ref1]
            } else {
                tok.backref_refs.clone()
            };
            let mut np = node_new_backref(back_num, &refs, tok.backref_by_name, tok.backref_level);
            if opton_ignorecase(env.options) {
                np.status_add(ND_ST_IGNORECASE);
            }
            env.backref_num += 1;
            np
        }
        TokenType::Keep => {
            let id = env.id_entry();
            env.keep_num += 1;
            node_new_save_gimmick(SaveType::Keep, id)
        }
        TokenType::GeneralNewline => {
            // TODO: node_new_general_newline
            node_new_anychar()
        }
        TokenType::NoNewline => {
            node_new_ctype(CTYPE_ANYCHAR, false, false)
        }
        TokenType::TrueAnychar => {
            let mut np = node_new_ctype(CTYPE_ANYCHAR, false, false);
            np.status_add(ND_ST_MULTILINE);
            np
        }
        TokenType::TextSegment => {
            // TODO: make_text_segment
            node_new_empty()
        }
        TokenType::QuoteOpen => {
            // Collect all chars until \E
            let qstart = *p;
            let mut qend = end;
            let esc = mc_esc(env.syntax);
            while !p_end(*p, end) {
                let save = *p;
                let mut pfv = *p;
                let c = pfetch(p, &mut pfv, pattern, end, env.enc);
                if c == esc && !p_end(*p, end) {
                    let c2 = ppeek(*p, pattern, end, env.enc);
                    if c2 == 'E' as u32 {
                        qend = save;
                        pinc(p, pattern, env.enc); // skip 'E'
                        break;
                    }
                }
            }
            let mut np = node_new_str(&pattern[qstart..qend]);
            if opton_ignorecase(env.options) {
                np.status_add(ND_ST_IGNORECASE);
            }
            np
        }
        TokenType::Repeat | TokenType::Interval => {
            if is_syntax_bv(env.syntax, ONIG_SYN_CONTEXT_INDEP_REPEAT_OPS) {
                if is_syntax_bv(env.syntax, ONIG_SYN_CONTEXT_INVALID_REPEAT_OPS) {
                    return Err(ONIGERR_TARGET_OF_REPEAT_OPERATOR_NOT_SPECIFIED);
                }
                node_new_empty()
            } else {
                node_new_str(&pattern[tok.backp..*p])
            }
        }
        _ => {
            return Err(ONIGERR_PARSER_BUG);
        }
    };

    // Fetch next token and check for quantifier
    let r = fetch_token(tok, p, end, pattern, env);
    if r < 0 {
        return Err(r);
    }
    check_quantifier(node, tok, p, end, pattern, env, group, parse_depth)
}

/// Check if current token is a quantifier and apply it to node
fn check_quantifier(
    mut node: Box<Node>,
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
    group: i32,
    parse_depth: u32,
) -> Result<(Box<Node>, i32), i32> {
    let r = tok.token_type as i32;

    if tok.token_type == TokenType::Repeat || tok.token_type == TokenType::Interval {
        if is_invalid_quantifier_target(&node) {
            if is_syntax_bv(env.syntax, ONIG_SYN_CONTEXT_INDEP_REPEAT_OPS) {
                if is_syntax_bv(env.syntax, ONIG_SYN_CONTEXT_INVALID_REPEAT_OPS) {
                    return Err(ONIGERR_TARGET_OF_REPEAT_OPERATOR_INVALID);
                }
            }
            return Ok((node, r));
        }

        // Check parse depth
        let depth = parse_depth + 1;
        if depth > PARSE_DEPTH_LIMIT.load(Ordering::Relaxed) {
            return Err(ONIGERR_PARSE_DEPTH_LIMIT_OVER);
        }

        let mut qn = node_new_quantifier(tok.repeat_lower, tok.repeat_upper, tok.repeat_greedy);
        qn.set_body(Some(node));

        if tok.repeat_possessive {
            let mut en = node_new_bag(BagType::StopBacktrack);
            en.set_body(Some(qn));
            qn = en;
        }

        // Fetch next token and check for more quantifiers
        let r = fetch_token(tok, p, end, pattern, env);
        if r < 0 {
            return Err(r);
        }
        // Recursively check for stacked quantifiers
        return check_quantifier(qn, tok, p, end, pattern, env, 0, depth);
    }

    Ok((node, r))
}

/// Parse a branch (sequence of expressions)
fn prs_branch(
    tok: &mut PToken,
    term: i32,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
    group_head: bool,
) -> Result<(Box<Node>, i32), i32> {
    env.parse_depth += 1;
    if env.parse_depth > PARSE_DEPTH_LIMIT.load(Ordering::Relaxed) {
        return Err(ONIGERR_PARSE_DEPTH_LIMIT_OVER);
    }

    let (node, mut r) = prs_exp(tok, term, p, end, pattern, env, group_head)?;

    if r == TokenType::Eot as i32 || r == term || r == TokenType::Alt as i32 {
        env.parse_depth -= 1;
        return Ok((node, r));
    }

    let top = node_new_list(node, None);
    let mut headp: *mut Option<Box<Node>>;
    // We need to build a linked list. Use unsafe pointer to the cdr slot.
    unsafe {
        let top_ptr = &*top as *const Node as *mut Node;
        if let NodeInner::List(ref mut cons) = (*top_ptr).inner {
            headp = &mut cons.cdr as *mut Option<Box<Node>>;
        } else {
            env.parse_depth -= 1;
            return Ok((top, r));
        }
    }

    while r != TokenType::Eot as i32 && r != term && r != TokenType::Alt as i32 {
        let (node2, r2) = prs_exp(tok, term, p, end, pattern, env, false)?;
        r = r2;

        let new_cell = node_new_list(node2, None);
        unsafe {
            *headp = Some(new_cell);
            // Advance headp to the new cell's cdr
            if let Some(ref mut cell) = *headp {
                let cell_ptr = cell.as_mut() as *mut Node;
                if let NodeInner::List(ref mut cons) = (*cell_ptr).inner {
                    headp = &mut cons.cdr as *mut Option<Box<Node>>;
                }
            }
        }
    }

    env.parse_depth -= 1;
    Ok((top, r))
}

/// Parse alternations (top-level: handles |)
fn prs_alts(
    tok: &mut PToken,
    term: i32,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
    group_head: bool,
) -> Result<(Box<Node>, i32), i32> {
    env.parse_depth += 1;
    if env.parse_depth > PARSE_DEPTH_LIMIT.load(Ordering::Relaxed) {
        return Err(ONIGERR_PARSE_DEPTH_LIMIT_OVER);
    }

    let save_options = env.options;

    let (node, mut r) = prs_branch(tok, term, p, end, pattern, env, group_head)?;

    if r == term {
        env.options = save_options;
        env.parse_depth -= 1;
        return Ok((node, r));
    } else if r == TokenType::Alt as i32 {
        let top = node_new_alt(node, None);
        let mut headp: *mut Option<Box<Node>>;
        unsafe {
            let top_ptr = &*top as *const Node as *mut Node;
            if let NodeInner::Alt(ref mut cons) = (*top_ptr).inner {
                headp = &mut cons.cdr as *mut Option<Box<Node>>;
            } else {
                env.parse_depth -= 1;
                return Ok((top, r));
            }
        }

        while r == TokenType::Alt as i32 {
            let r2 = fetch_token(tok, p, end, pattern, env);
            if r2 < 0 {
                return Err(r2);
            }
            let (node2, r2) = prs_branch(tok, term, p, end, pattern, env, false)?;
            r = r2;

            let new_cell = node_new_alt(node2, None);
            unsafe {
                *headp = Some(new_cell);
                if let Some(ref mut cell) = *headp {
                    let cell_ptr = cell.as_mut() as *mut Node;
                    if let NodeInner::Alt(ref mut cons) = (*cell_ptr).inner {
                        headp = &mut cons.cdr as *mut Option<Box<Node>>;
                    }
                }
            }
        }

        if tok.token_type as i32 != term {
            if term == TokenType::SubexpClose as i32 {
                return Err(ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS);
            } else {
                return Err(ONIGERR_PARSER_BUG);
            }
        }

        env.options = save_options;
        env.parse_depth -= 1;
        Ok((top, r))
    } else {
        if term == TokenType::SubexpClose as i32 {
            return Err(ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS);
        }
        env.options = save_options;
        env.parse_depth -= 1;
        Err(ONIGERR_PARSER_BUG)
    }
}

/// Parse a complete regexp
fn prs_regexp(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
) -> Result<Box<Node>, i32> {
    let mut tok = PToken::new();
    tok.init();
    let r = fetch_token(&mut tok, p, end, pattern, env);
    if r < 0 {
        return Err(r);
    }
    let (top, _) = prs_alts(&mut tok, TokenType::Eot as i32, p, end, pattern, env, false)?;
    Ok(top)
}

// ============================================================================
// Entry point: onig_parse_tree
// ============================================================================

pub fn onig_parse_tree(
    pattern: &[u8],
    reg: &mut RegexType,
    env: &mut ParseEnv,
) -> Result<Box<Node>, i32> {
    // Initialize regex fields
    reg.num_mem = 0;
    reg.num_repeat = 0;
    reg.num_empty_check = 0;
    reg.repeat_range = Vec::new();

    // Clear name table
    reg.name_table = Some(NameTable::new());

    // Initialize parse environment
    env.clear();
    env.options = reg.options;
    env.case_fold_flag = reg.case_fold_flag;
    env.enc = reg.enc;
    env.syntax = unsafe { &*reg.syntax };
    env.pattern = pattern.as_ptr();
    env.pattern_end = unsafe { pattern.as_ptr().add(pattern.len()) };
    env.reg = reg as *mut RegexType;

    // Validate pattern encoding
    if !env.enc.is_valid_mbc_string(pattern) {
        return Err(ONIGERR_INVALID_WIDE_CHAR_VALUE);
    }

    let mut p: usize = 0;
    let end = pattern.len();
    let root = prs_regexp(&mut p, end, pattern, env)?;

    reg.num_mem = env.num_mem;

    Ok(root)
}
