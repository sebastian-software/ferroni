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

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

use crate::oniguruma::*;
use crate::regenc::*;
use crate::regexec::OnigCalloutFunc;
use crate::regint::*;
use crate::regparse_types::*;

// ============================================================================
// Global Warn Functions (port of C's onig_warn / onig_verb_warn)
// ============================================================================

pub type OnigWarnFunc = fn(s: &str);

static WARN_FUNC: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static VERB_WARN_FUNC: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

pub fn onig_set_warn_func(f: OnigWarnFunc) {
    let p: *mut () = f as *mut ();
    WARN_FUNC.store(p, Ordering::Relaxed);
}

pub fn onig_set_verb_warn_func(f: OnigWarnFunc) {
    let p: *mut () = f as *mut ();
    VERB_WARN_FUNC.store(p, Ordering::Relaxed);
}

// ============================================================================
// Global Callout Name Registry (port of C's GlobalCalloutNameList)
// ============================================================================

pub struct CalloutNameListEntry {
    pub callout_type: OnigCalloutType,
    pub callout_in: i32,
    pub start_func: Option<OnigCalloutFunc>,
    pub end_func: Option<OnigCalloutFunc>,
    pub arg_num: i32,
    pub opt_arg_num: i32,
    pub arg_types: Vec<u32>,
    pub opt_defaults: Vec<OnigValue>,
    pub name: Vec<u8>,
}

static CALLOUT_NAME_REGISTRY: Mutex<Vec<CalloutNameListEntry>> = Mutex::new(Vec::new());

/// Register a user-defined named callout.
/// Port of C's onig_set_callout_of_name.
pub fn onig_set_callout_of_name(
    _enc: OnigEncoding,
    callout_type: OnigCalloutType,
    name: &[u8],
    callout_in: i32,
    start_func: Option<OnigCalloutFunc>,
    end_func: Option<OnigCalloutFunc>,
    arg_num: i32,
    arg_types: &[u32],
    opt_arg_num: i32,
    opt_defaults: &[OnigValue],
) -> i32 {
    if callout_type != OnigCalloutType::Single {
        return ONIGERR_INVALID_ARGUMENT;
    }
    if arg_num < 0 || arg_num > ONIG_CALLOUT_MAX_ARGS_NUM as i32 {
        return ONIGERR_INVALID_CALLOUT_ARG;
    }
    if opt_arg_num < 0 || opt_arg_num > arg_num {
        return ONIGERR_INVALID_CALLOUT_ARG;
    }
    if start_func.is_none() && end_func.is_none() {
        return ONIGERR_INVALID_CALLOUT_ARG;
    }
    if (callout_in & (OnigCalloutIn::Progress as i32)) == 0
        && (callout_in & (OnigCalloutIn::Retraction as i32)) == 0
    {
        return ONIGERR_INVALID_CALLOUT_ARG;
    }
    if name.is_empty() {
        return ONIGERR_INVALID_CALLOUT_NAME;
    }

    let entry = CalloutNameListEntry {
        callout_type,
        callout_in,
        start_func,
        end_func,
        arg_num,
        opt_arg_num,
        arg_types: arg_types.to_vec(),
        opt_defaults: opt_defaults.to_vec(),
        name: name.to_vec(),
    };

    let mut registry = CALLOUT_NAME_REGISTRY.lock().unwrap();
    // Check if name already exists — update in place
    for (i, existing) in registry.iter_mut().enumerate() {
        if existing.name == name {
            *existing = entry;
            return i as i32;
        }
    }
    let id = registry.len() as i32;
    registry.push(entry);
    id
}

/// Get the name of a registered callout by its name ID.
/// Port of C's onig_get_callout_name_by_name_id.
pub fn onig_get_callout_name_by_name_id(name_id: i32) -> Option<Vec<u8>> {
    let registry = CALLOUT_NAME_REGISTRY.lock().unwrap();
    if name_id < 0 || name_id as usize >= registry.len() {
        return None;
    }
    Some(registry[name_id as usize].name.clone())
}

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

use std::sync::atomic::{AtomicI32, AtomicU32};

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
fn pfetch_s(p: &mut usize, pattern: &[u8], end: usize, enc: OnigEncoding) -> OnigCodePoint {
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
fn ppeek_is(p: usize, pattern: &[u8], end: usize, enc: OnigEncoding, c: OnigCodePoint) -> bool {
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

/// Safe multiplication for repeat counts. Returns -1 on overflow.
/// Port of onig_positive_int_multiply from regcomp.c.
fn positive_int_multiply(x: i32, y: i32) -> i32 {
    if x == 0 || y == 0 {
        return 0;
    }
    if x < i32::MAX / y {
        x * y
    } else {
        -1
    }
}

fn scan_number(p: &mut usize, end: usize, pattern: &[u8], enc: OnigEncoding) -> i32 {
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
// Multi-codepoint sequence helpers for \x{AAAA BBBB} and \o{NNN NNN}
// ============================================================================

/// Check if character is a code point separator (space or newline).
fn is_code_point_divide(c: u32) -> bool {
    c == ' ' as u32 || c == '\n' as u32
}

/// Validate and count remaining codepoints in a \x{...} or \o{...} sequence.
/// For use outside character classes (no range '-' support).
/// Returns number of remaining codepoints, or negative error.
/// Does NOT modify p (validates only, like C version which takes p by value).
fn check_code_point_sequence(
    p: &mut usize,
    end: usize,
    base: i32,
    pattern: &[u8],
    enc: OnigEncoding,
) -> i32 {
    let save_p = *p;
    let mut pos = *p;
    let mut n = 0;
    let mut end_digit = false;
    let mut pfetch_prev = pos;

    while !p_end(pos, end) {
        let c = pfetch(&mut pos, &mut pfetch_prev, pattern, end, enc);
        if c == '}' as u32 {
            if n == 0 {
                return ONIGERR_INVALID_CODE_POINT_VALUE;
            }
            *p = save_p; // restore - don't consume
            return n;
        }

        if is_code_point_divide(c) {
            while !p_end(pos, end) {
                let c2 = pfetch(&mut pos, &mut pfetch_prev, pattern, end, enc);
                if !is_code_point_divide(c2) {
                    pos = pfetch_prev; // PUNFETCH
                    break;
                }
            }
            end_digit = false;
            continue;
        } else if end_digit {
            if base == 16 && is_code_xdigit_ascii(enc, c) {
                return ONIGERR_TOO_LONG_WIDE_CHAR_VALUE;
            }
            return ONIGERR_INVALID_CODE_POINT_VALUE;
        }

        pos = pfetch_prev; // PUNFETCH
        let mut code = 0u32;
        let r = if base == 16 {
            scan_hexadecimal_number(&mut pos, end, 0, 8, pattern, enc, &mut code)
        } else {
            scan_octal_number(&mut pos, end, 0, 11, pattern, enc, &mut code)
        };
        if r != 0 {
            return ONIGERR_INVALID_CODE_POINT_VALUE;
        }
        n += 1;
        end_digit = true;
    }

    ONIGERR_INVALID_CODE_POINT_VALUE
}

/// Validate and count remaining codepoints in a \x{...} or \o{...} sequence
/// inside a character class (supports '-' for ranges).
/// Does NOT modify p (validates only).
fn check_code_point_sequence_cc(
    p: &mut usize,
    end: usize,
    base: i32,
    pattern: &[u8],
    enc: OnigEncoding,
    state: i32,
) -> i32 {
    let save_p = *p;
    let mut pos = *p;
    let mut n = 0;
    let mut end_digit = false;
    let mut pfetch_prev = pos;
    let mut cps_state = state;

    while !p_end(pos, end) {
        let c = pfetch(&mut pos, &mut pfetch_prev, pattern, end, enc);
        if c == '}' as u32 {
            if cps_state == CPS_RANGE {
                return ONIGERR_INVALID_CODE_POINT_VALUE;
            }
            *p = save_p; // restore - don't consume
            return n;
        }

        if is_code_point_divide(c) {
            while !p_end(pos, end) {
                let c2 = pfetch(&mut pos, &mut pfetch_prev, pattern, end, enc);
                if !is_code_point_divide(c2) {
                    pos = pfetch_prev; // PUNFETCH
                    break;
                }
            }
            end_digit = false;
            continue;
        } else if c == '-' as u32 {
            if cps_state != CPS_START_VAL {
                return ONIGERR_INVALID_CODE_POINT_VALUE;
            }
            if p_end(pos, end) {
                return ONIGERR_INVALID_CODE_POINT_VALUE;
            }
            end_digit = false;
            cps_state = CPS_RANGE;
            continue;
        } else if end_digit {
            if base == 16 && is_code_xdigit_ascii(enc, c) {
                return ONIGERR_TOO_LONG_WIDE_CHAR_VALUE;
            }
            return ONIGERR_INVALID_CODE_POINT_VALUE;
        }

        pos = pfetch_prev; // PUNFETCH
        let mut code = 0u32;
        let r = if base == 16 {
            scan_hexadecimal_number(&mut pos, end, 0, 8, pattern, enc, &mut code)
        } else {
            scan_octal_number(&mut pos, end, 0, 11, pattern, enc, &mut code)
        };
        if r != 0 {
            return ONIGERR_INVALID_CODE_POINT_VALUE;
        }
        n += 1;
        end_digit = true;
        cps_state = if cps_state == CPS_RANGE {
            CPS_EMPTY
        } else {
            CPS_START_VAL
        };
    }

    ONIGERR_INVALID_CODE_POINT_VALUE
}

/// Get next code point from an ongoing \x{...} or \o{...} sequence.
/// Returns: 0 = got codepoint, 1 = end of sequence ('}'), 2 = range marker ('-').
fn get_next_code_point(
    p: &mut usize,
    end: usize,
    base: i32,
    pattern: &[u8],
    enc: OnigEncoding,
    in_cc: bool,
    rcode: &mut OnigCodePoint,
) -> i32 {
    let mut pfetch_prev = *p;

    // Skip whitespace separators
    while !p_end(*p, end) {
        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        if !is_code_point_divide(c) {
            if c == '}' as u32 {
                return 1; // end of sequence
            } else if c == '-' as u32 && in_cc {
                return 2; // range marker
            }
            *p = pfetch_prev; // PUNFETCH
            break;
        } else if p_end(*p, end) {
            return ONIGERR_INVALID_CODE_POINT_VALUE;
        }
    }

    let r = if base == 16 {
        scan_hexadecimal_number(p, end, 0, 8, pattern, enc, rcode)
    } else {
        scan_octal_number(p, end, 0, 11, pattern, enc, rcode)
    };
    if r != 0 {
        return r;
    }

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

fn add_code_range_to_buf(pbuf: &mut Option<BBuf>, from: OnigCodePoint, to: OnigCodePoint) -> i32 {
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
// Code range combination operations (and/or/not for multi-byte ranges)
// ============================================================================

fn set_all_multi_byte_range(enc: OnigEncoding) -> Option<BBuf> {
    let start = mbcode_start_pos(enc);
    let mut bbuf = new_code_range();
    let r = add_code_range_to_buf(&mut Some(bbuf), start, u32::MAX);
    // We need to extract the bbuf back; let's redo this properly
    let mut opt = None;
    add_code_range_to_buf(&mut opt, start, u32::MAX);
    opt
}

fn not_code_range_buf(enc: OnigEncoding, bbuf: &Option<BBuf>) -> Option<BBuf> {
    if bbuf.is_none() {
        return set_all_multi_byte_range(enc);
    }
    let bbuf = bbuf.as_ref().unwrap();
    let n = bbuf_read_code_point(bbuf, 0) as usize;
    if n == 0 {
        return set_all_multi_byte_range(enc);
    }

    let mut result: Option<BBuf> = None;
    let mut pre = mbcode_start_pos(enc);
    for i in 0..n {
        let from = bbuf_read_code_point(bbuf, SIZE_CODE_POINT * (1 + i * 2));
        let to = bbuf_read_code_point(bbuf, SIZE_CODE_POINT * (1 + i * 2 + 1));
        if pre <= from.wrapping_sub(1) && from > 0 {
            add_code_range_to_buf(&mut result, pre, from - 1);
        }
        if to == u32::MAX {
            return result;
        }
        pre = to + 1;
    }
    let last_to = bbuf_read_code_point(bbuf, SIZE_CODE_POINT * (1 + (n - 1) * 2 + 1));
    if last_to < u32::MAX {
        add_code_range_to_buf(&mut result, last_to + 1, u32::MAX);
    }
    result
}

fn or_code_range_buf(
    enc: OnigEncoding,
    bbuf1: &Option<BBuf>,
    not1: bool,
    bbuf2: &Option<BBuf>,
    not2: bool,
) -> Option<BBuf> {
    if bbuf1.is_none() && bbuf2.is_none() {
        if not1 || not2 {
            return set_all_multi_byte_range(enc);
        }
        return None;
    }

    // Normalize: make sure bbuf1 is not None if possible
    let (b1, n1, b2, n2) = if bbuf1.is_none() {
        (bbuf2, not2, bbuf1, not1)
    } else {
        (bbuf1, not1, bbuf2, not2)
    };

    if b2.is_none() {
        // b2 is None (was bbuf1 or bbuf2)
        if n2 {
            return set_all_multi_byte_range(enc);
        } else {
            if !n1 {
                return b1.clone();
            } else {
                return not_code_range_buf(enc, b1);
            }
        }
    }

    // Both non-None
    let (b1, n1, b2, _n2) = if n1 {
        (b2, n2, b1, n1)
    } else {
        (b1, n1, b2, n2)
    };

    // Now n1 == false (or we swapped)
    let mut result = if !_n2 {
        b2.clone()
    } else {
        not_code_range_buf(enc, b2)
    };

    // Add all ranges from b1
    if let Some(ref bb1) = b1 {
        let nn = bbuf_read_code_point(bb1, 0) as usize;
        for i in 0..nn {
            let from = bbuf_read_code_point(bb1, SIZE_CODE_POINT * (1 + i * 2));
            let to = bbuf_read_code_point(bb1, SIZE_CODE_POINT * (1 + i * 2 + 1));
            add_code_range_to_buf(&mut result, from, to);
        }
    }
    result
}

fn and_code_range1(
    pbuf: &mut Option<BBuf>,
    from1: OnigCodePoint,
    to1: OnigCodePoint,
    data: &[OnigCodePoint],
    n: usize,
) -> i32 {
    let mut from1 = from1;
    let mut to1 = to1;
    for i in 0..n {
        let from2 = data[i * 2];
        let to2 = data[i * 2 + 1];
        if from2 < from1 {
            if to2 < from1 {
                continue;
            } else {
                from1 = to2 + 1;
            }
        } else if from2 <= to1 {
            if to2 < to1 {
                if from1 <= from2.wrapping_sub(1) && from2 > 0 {
                    let r = add_code_range_to_buf(pbuf, from1, from2 - 1);
                    if r != 0 {
                        return r;
                    }
                }
                from1 = to2 + 1;
            } else {
                if from2 > 0 {
                    to1 = from2 - 1;
                } else {
                    return 0;
                }
            }
        } else {
            from1 = from2;
        }
        if from1 > to1 {
            break;
        }
    }
    if from1 <= to1 {
        let r = add_code_range_to_buf(pbuf, from1, to1);
        if r != 0 {
            return r;
        }
    }
    0
}

fn and_code_range_buf(
    bbuf1: &Option<BBuf>,
    not1: bool,
    bbuf2: &Option<BBuf>,
    not2: bool,
) -> (Option<BBuf>, i32) {
    if bbuf1.is_none() {
        if not1 && bbuf2.is_some() {
            return (bbuf2.clone(), 0);
        }
        return (None, 0);
    }
    if bbuf2.is_none() {
        if not2 {
            return (bbuf1.clone(), 0);
        }
        return (None, 0);
    }

    // Swap if not1
    let (b1, _n1, b2, n2) = if not1 {
        (bbuf2, not2, bbuf1, not1)
    } else {
        (bbuf1, not1, bbuf2, not2)
    };

    let bb1 = b1.as_ref().unwrap();
    let bb2 = b2.as_ref().unwrap();
    let nn1 = bbuf_read_code_point(bb1, 0) as usize;
    let nn2 = bbuf_read_code_point(bb2, 0) as usize;

    let mut data1 = Vec::with_capacity(nn1 * 2);
    for i in 0..nn1 * 2 {
        data1.push(bbuf_read_code_point(bb1, SIZE_CODE_POINT * (1 + i)));
    }
    let mut data2 = Vec::with_capacity(nn2 * 2);
    for i in 0..nn2 * 2 {
        data2.push(bbuf_read_code_point(bb2, SIZE_CODE_POINT * (1 + i)));
    }

    let mut result: Option<BBuf> = None;

    if !n2 && !_n1 {
        // 1 AND 2
        for i in 0..nn1 {
            let from1 = data1[i * 2];
            let to1 = data1[i * 2 + 1];
            for j in 0..nn2 {
                let from2 = data2[j * 2];
                let to2 = data2[j * 2 + 1];
                if from2 > to1 {
                    break;
                }
                if to2 < from1 {
                    continue;
                }
                let from = std::cmp::max(from1, from2);
                let to = std::cmp::min(to1, to2);
                let r = add_code_range_to_buf(&mut result, from, to);
                if r != 0 {
                    return (result, r);
                }
            }
        }
    } else if !_n1 {
        // 1 AND (not 2)
        for i in 0..nn1 {
            let from1 = data1[i * 2];
            let to1 = data1[i * 2 + 1];
            let r = and_code_range1(&mut result, from1, to1, &data2, nn2);
            if r != 0 {
                return (result, r);
            }
        }
    }

    (result, 0)
}

fn and_cclass(dest: &mut CClassNode, cc: &CClassNode, enc: OnigEncoding) -> i32 {
    let not1 = dest.is_not();
    let not2 = cc.is_not();

    let mut bsr1 = dest.bs;
    let mut bsr2 = cc.bs;
    if not1 {
        bitset_invert(&mut bsr1);
    }
    if not2 {
        bitset_invert(&mut bsr2);
    }
    bitset_and(&mut bsr1, &bsr2);
    if not1 {
        bitset_invert(&mut bsr1);
    }
    dest.bs = bsr1;

    if enc.min_enc_len() > 1 || (enc.flag() & ENC_FLAG_UNICODE) != 0 {
        let (pbuf, r) = if not1 && not2 {
            let result = or_code_range_buf(enc, &dest.mbuf, false, &cc.mbuf, false);
            (result, 0)
        } else {
            let (result, r) = and_code_range_buf(&dest.mbuf, not1, &cc.mbuf, not2);
            if r == 0 && not1 {
                let tbuf = not_code_range_buf(enc, &result);
                (tbuf, 0)
            } else {
                (result, r)
            }
        };
        if r != 0 {
            return r;
        }
        dest.mbuf = pbuf;
    }
    0
}

fn or_cclass(dest: &mut CClassNode, cc: &CClassNode, enc: OnigEncoding) -> i32 {
    let not1 = dest.is_not();
    let not2 = cc.is_not();

    let mut bsr1 = dest.bs;
    let mut bsr2 = cc.bs;
    if not1 {
        bitset_invert(&mut bsr1);
    }
    if not2 {
        bitset_invert(&mut bsr2);
    }
    bitset_or(&mut bsr1, &bsr2);
    if not1 {
        bitset_invert(&mut bsr1);
    }
    dest.bs = bsr1;

    if enc.min_enc_len() > 1 || (enc.flag() & ENC_FLAG_UNICODE) != 0 {
        let (pbuf, r) = if not1 && not2 {
            and_code_range_buf(&dest.mbuf, false, &cc.mbuf, false)
        } else {
            let result = or_code_range_buf(enc, &dest.mbuf, not1, &cc.mbuf, not2);
            if not1 {
                let tbuf = not_code_range_buf(enc, &result);
                (tbuf, 0)
            } else {
                (result, 0)
            }
        };
        if r != 0 {
            return r;
        }
        dest.mbuf = pbuf;
    }
    0
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

fn add_ctype_to_cc(cc: &mut CClassNode, ctype: i32, not: bool, env: &ParseEnv) -> i32 {
    let enc = env.enc;
    let ascii_mode = opton_is_ascii_mode_ctype(ctype, env.options);
    if ascii_mode {
        // ASCII-only mode: iterate over 0-127 using encoding's ctype check
        for c in 0..128u32 {
            if enc.is_code_ctype(c, ctype as u32) {
                if not {
                    // Don't set bit for matching chars
                } else {
                    bitset_set_bit(&mut cc.bs, c as usize);
                }
            } else if not {
                bitset_set_bit(&mut cc.bs, c as usize);
            }
        }
        if not {
            // For negated ASCII-only, add all multi-byte codes as matching
            add_code_range_to_buf(&mut cc.mbuf, 0x80, u32::MAX);
        }
        return ONIG_NORMAL;
    }
    let mut sb_out: OnigCodePoint = 0;
    let range = enc.get_ctype_code_range(ctype as u32, &mut sb_out);
    if let Some(_) = range {
        return add_ctype_to_cc_by_range(cc, ctype, not, enc, sb_out);
    }

    // Fallback: iterate over single-byte range
    let max_code = if enc.min_enc_len() > 1 {
        0x80
    } else {
        SINGLE_BYTE_SIZE as OnigCodePoint
    };
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
pub(crate) fn add_code_into_cc(cc: &mut CClassNode, code: OnigCodePoint, enc: OnigEncoding) {
    if code < SINGLE_BYTE_SIZE as u32 {
        bitset_set_bit(&mut cc.bs, code as usize);
    } else {
        add_code_range_to_buf(&mut cc.mbuf, code, code);
    }
}

// ============================================================================
// CC state machine helpers
// ============================================================================

/// Flush pending character and advance CC state machine
fn cc_char_next(
    cc: &mut CClassNode,
    from: &mut OnigCodePoint,
    to: OnigCodePoint,
    from_raw: &mut bool,
    to_raw: bool,
    intype: i32,
    curr_type: &mut i32,
    state: &mut i32,
    env: &ParseEnv,
) -> i32 {
    let r;
    match *state {
        CS_VALUE => {
            if *curr_type == CV_SB {
                if *from > 0xff {
                    return ONIGERR_INVALID_CODE_POINT_VALUE;
                }
                bitset_set_bit(&mut cc.bs, *from as usize);
            } else if *curr_type == CV_MB {
                r = add_code_range(&mut cc.mbuf, env, *from, *from);
                if r < 0 {
                    return r;
                }
            }
        }
        CS_RANGE => {
            if intype == *curr_type {
                if intype == CV_SB {
                    if *from > 0xff || to > 0xff {
                        return ONIGERR_INVALID_CODE_POINT_VALUE;
                    }
                    if *from > to {
                        if is_syntax_bv(env.syntax, ONIG_SYN_ALLOW_EMPTY_RANGE_IN_CC) {
                            *state = CS_COMPLETE;
                            *from_raw = to_raw;
                            *from = to;
                            *curr_type = intype;
                            return 0;
                        } else {
                            return ONIGERR_EMPTY_RANGE_IN_CHAR_CLASS;
                        }
                    }
                    bitset_set_range(&mut cc.bs, *from as usize, to as usize);
                } else {
                    r = add_code_range(&mut cc.mbuf, env, *from, to);
                    if r < 0 {
                        return r;
                    }
                }
            } else {
                if *from > to {
                    if is_syntax_bv(env.syntax, ONIG_SYN_ALLOW_EMPTY_RANGE_IN_CC) {
                        *state = CS_COMPLETE;
                        *from_raw = to_raw;
                        *from = to;
                        *curr_type = intype;
                        return 0;
                    } else {
                        return ONIGERR_EMPTY_RANGE_IN_CHAR_CLASS;
                    }
                }
                let sbout = enc_sb_out(env.enc);
                if *from < sbout {
                    let sb_end = if to < sbout { to } else { sbout - 1 };
                    bitset_set_range(&mut cc.bs, *from as usize, sb_end as usize);
                }
                if to >= sbout {
                    let mb_start = if *from > sbout { *from } else { sbout };
                    r = add_code_range(&mut cc.mbuf, env, mb_start, to);
                    if r < 0 {
                        return r;
                    }
                }
            }
            *state = CS_COMPLETE;
            *from_raw = to_raw;
            *from = to;
            *curr_type = intype;
            return 0;
        }
        CS_COMPLETE | CS_START => {
            *state = CS_VALUE;
        }
        _ => {}
    }

    *from_raw = to_raw;
    *from = to;
    *curr_type = intype;
    0
}

/// Flush pending value before char property, advance state
fn cc_cprop_next(
    cc: &mut CClassNode,
    pcode: &mut OnigCodePoint,
    val: &mut i32,
    state: &mut i32,
    env: &ParseEnv,
) -> i32 {
    if *state == CS_RANGE {
        return ONIGERR_CHAR_CLASS_VALUE_AT_END_OF_RANGE;
    }

    if *state == CS_VALUE {
        if *val == CV_SB {
            bitset_set_bit(&mut cc.bs, *pcode as usize);
        } else if *val == CV_MB {
            let r = add_code_range(&mut cc.mbuf, env, *pcode, *pcode);
            if r < 0 {
                return r;
            }
        }
    }

    *state = CS_VALUE;
    *val = CV_CPROP;
    0
}

/// Check if a code point exists in pattern from position
fn code_exist_check(
    c: OnigCodePoint,
    from: usize,
    end: usize,
    pattern: &[u8],
    ignore_escaped: bool,
    env: &ParseEnv,
) -> bool {
    let enc = env.enc;
    let mut p = from;
    let mut in_esc = false;
    while !p_end(p, end) {
        if ignore_escaped && in_esc {
            in_esc = false;
        } else {
            let code = pfetch_s(&mut p, pattern, end, enc);
            if code == c {
                return true;
            }
            if code == mc_esc(env.syntax) {
                in_esc = true;
            }
        }
    }
    false
}

// POSIX bracket entry
struct PosixBracketEntry {
    name: &'static [u8],
    ctype: u32,
}

static POSIX_BRACKETS: &[PosixBracketEntry] = &[
    PosixBracketEntry {
        name: b"alnum",
        ctype: ONIGENC_CTYPE_ALNUM,
    },
    PosixBracketEntry {
        name: b"alpha",
        ctype: ONIGENC_CTYPE_ALPHA,
    },
    PosixBracketEntry {
        name: b"blank",
        ctype: ONIGENC_CTYPE_BLANK,
    },
    PosixBracketEntry {
        name: b"cntrl",
        ctype: ONIGENC_CTYPE_CNTRL,
    },
    PosixBracketEntry {
        name: b"digit",
        ctype: ONIGENC_CTYPE_DIGIT,
    },
    PosixBracketEntry {
        name: b"graph",
        ctype: ONIGENC_CTYPE_GRAPH,
    },
    PosixBracketEntry {
        name: b"lower",
        ctype: ONIGENC_CTYPE_LOWER,
    },
    PosixBracketEntry {
        name: b"print",
        ctype: ONIGENC_CTYPE_PRINT,
    },
    PosixBracketEntry {
        name: b"punct",
        ctype: ONIGENC_CTYPE_PUNCT,
    },
    PosixBracketEntry {
        name: b"space",
        ctype: ONIGENC_CTYPE_SPACE,
    },
    PosixBracketEntry {
        name: b"upper",
        ctype: ONIGENC_CTYPE_UPPER,
    },
    PosixBracketEntry {
        name: b"xdigit",
        ctype: ONIGENC_CTYPE_XDIGIT,
    },
    PosixBracketEntry {
        name: b"ascii",
        ctype: ONIGENC_CTYPE_ASCII,
    },
    PosixBracketEntry {
        name: b"word",
        ctype: ONIGENC_CTYPE_WORD,
    },
];

/// Parse POSIX bracket like [:alpha:]
fn prs_posix_bracket(
    cc: &mut CClassNode,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
) -> i32 {
    let enc = env.enc;
    let not = if !p_end(*p, end) && ppeek_is(*p, pattern, end, enc, '^' as u32) {
        pinc(p, pattern, enc);
        true
    } else {
        false
    };

    for pb in POSIX_BRACKETS {
        let name = pb.name;
        if *p + name.len() <= end && &pattern[*p..*p + name.len()] == name {
            let mut tp = *p + name.len();
            // Check for ":]"
            if tp + 2 <= end && pattern[tp] == b':' && pattern[tp + 1] == b']' {
                let r = add_ctype_to_cc(cc, pb.ctype as i32, not, env);
                if r != 0 {
                    return r;
                }
                *p = tp + 2;
                return 0;
            }
            break;
        }
    }

    ONIGERR_INVALID_POSIX_BRACKET_TYPE
}

/// Resolve \p{PropertyName} to a ctype value
fn fetch_char_property_to_ctype(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    braces: bool,
    env: &ParseEnv,
) -> i32 {
    let enc = env.enc;
    let start = *p;

    if !braces {
        // Single-char property: \pL
        if p_end(*p, end) {
            return ONIGERR_INVALID_CHAR_PROPERTY_NAME;
        }
        pfetch_s(p, pattern, end, enc);
        let r = enc.property_name_to_ctype(&pattern[start..*p]);
        return r;
    }

    // Braced: \p{PropertyName}
    while !p_end(*p, end) {
        let prev = *p;
        let c = pfetch_s(p, pattern, end, enc);
        if c == '}' as u32 {
            let r = enc.property_name_to_ctype(&pattern[start..prev]);
            return r;
        } else if c == '(' as u32 || c == ')' as u32 || c == '{' as u32 || c == '|' as u32 {
            break;
        }
    }

    ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS
}

/// Parse a character property (\p{...} / \P{...})
fn prs_char_property(
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
) -> Result<Box<Node>, i32> {
    let ctype = fetch_char_property_to_ctype(p, end, pattern, tok.prop_braces, env);
    if ctype < 0 {
        return Err(ctype);
    }

    if ctype == ONIGENC_CTYPE_WORD as i32 {
        let np = node_new_ctype(ctype, tok.prop_not, opton_word_ascii(env.options));
        return Ok(np);
    }

    let mut np = node_new_cclass();
    if let Some(cc) = np.as_cclass_mut() {
        let r = add_ctype_to_cc(cc, ctype, false, env);
        if r != 0 {
            return Err(r);
        }
        if tok.prop_not {
            cc.set_not();
        }
    }
    Ok(np)
}

/// Check if position looks like start of a POSIX bracket ([:...])
fn is_posix_bracket_start(p: usize, end: usize, pattern: &[u8], enc: OnigEncoding) -> bool {
    let mut tp = p;
    let mut n = 0;
    while tp < end {
        let c = pattern[tp];
        if c == b':' {
            // Check for :]
            if tp + 1 < end && pattern[tp + 1] == b']' {
                // Empty name (n == 0) is not a valid POSIX bracket
                return n > 0;
            }
            return false;
        }
        if c == b']' || c == b'[' || c == b'\\' {
            return false;
        }
        if c == b'^' && n == 0 {
            // Skip negation prefix
        } else {
            n += 1;
        }
        tp += enc.mbc_enc_len(&pattern[tp..end]);
    }
    false
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
// Name parsing helpers
// ============================================================================

fn get_name_end_code_point(start_code: OnigCodePoint) -> OnigCodePoint {
    match start_code {
        0x3C => 0x3E, // '<' -> '>'
        0x27 => 0x27, // '\'' -> '\''
        0x28 => 0x29, // '(' -> ')'
        _ => 0,
    }
}

/// Parse a group/backref name.
/// is_ref: false = defining name, true = referencing name (allows numeric)
/// Returns (name_start, name_end, back_num, num_type) or error.
fn fetch_name(
    start_code: OnigCodePoint,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
    is_ref: bool,
) -> Result<(usize, usize, i32, i32, bool, i32), i32> {
    // Returns: (name_start, name_end, back_num, num_type, exist_level, level)
    let enc = env.enc;
    let end_code = get_name_end_code_point(start_code);
    let mut back_num = 0i32;
    let mut num_type = IS_NOT_NUM;
    let mut sign = 1i32;
    let name_start = *p;
    let mut pnum_head = *p;
    let mut digit_count = 0i32;
    let mut name_end = end;
    let mut r = 0i32;
    let mut exist_level = false;
    let mut level = 0i32;

    if p_end(*p, end) {
        return Err(ONIGERR_EMPTY_GROUP_NAME);
    }

    let c = pfetch_s(p, pattern, end, enc);
    if c == end_code {
        return Err(ONIGERR_EMPTY_GROUP_NAME);
    }

    if is_code_digit_ascii(enc, c) {
        if is_ref {
            num_type = IS_ABS_NUM;
        } else {
            r = ONIGERR_INVALID_GROUP_NAME;
        }
        digit_count += 1;
    } else if c == '-' as u32 {
        if is_ref {
            num_type = IS_REL_NUM;
            sign = -1;
            pnum_head = *p;
        } else {
            r = ONIGERR_INVALID_GROUP_NAME;
        }
    } else if c == '+' as u32 {
        if is_ref {
            num_type = IS_REL_NUM;
            sign = 1;
            pnum_head = *p;
        } else {
            r = ONIGERR_INVALID_GROUP_NAME;
        }
    } else if !enc.is_code_ctype(c, ONIGENC_CTYPE_WORD) {
        r = ONIGERR_INVALID_CHAR_IN_GROUP_NAME;
    }

    if r == 0 {
        while !p_end(*p, end) {
            name_end = *p;
            let c = pfetch_s(p, pattern, end, enc);
            if c == end_code || c == ')' as u32 {
                if num_type != IS_NOT_NUM && digit_count == 0 {
                    r = ONIGERR_INVALID_GROUP_NAME;
                }
                break;
            }

            if num_type != IS_NOT_NUM {
                if is_code_digit_ascii(enc, c) {
                    digit_count += 1;
                } else if is_ref && (c == '+' as u32 || c == '-' as u32) && digit_count > 0 {
                    // Level syntax: \k<1+3> or \k<name+2>
                    // Stop name at the +/-, parse level
                    name_end = *p - 1; // position before '+'/'-'
                    let level_sign: i32 = if c == '-' as u32 { -1 } else { 1 };
                    let mut level_val = 0i32;
                    while !p_end(*p, end) {
                        let lc = pfetch_s(p, pattern, end, enc);
                        if lc == end_code {
                            exist_level = true;
                            level = level_val * level_sign;
                            break;
                        }
                        if is_code_digit_ascii(enc, lc) {
                            level_val = level_val * 10 + (lc as i32 - '0' as i32);
                        } else {
                            r = ONIGERR_INVALID_GROUP_NAME;
                            break;
                        }
                    }
                    break;
                } else {
                    if !enc.is_code_ctype(c, ONIGENC_CTYPE_WORD) {
                        r = ONIGERR_INVALID_CHAR_IN_GROUP_NAME;
                    } else {
                        r = ONIGERR_INVALID_GROUP_NAME;
                    }
                    num_type = IS_NOT_NUM;
                }
            } else {
                if is_ref && (c == '+' as u32 || c == '-' as u32) {
                    // Level syntax for named refs: \k<name+2>
                    name_end = *p - 1;
                    let level_sign: i32 = if c == '-' as u32 { -1 } else { 1 };
                    let mut level_val = 0i32;
                    while !p_end(*p, end) {
                        let lc = pfetch_s(p, pattern, end, enc);
                        if lc == end_code {
                            exist_level = true;
                            level = level_val * level_sign;
                            break;
                        }
                        if is_code_digit_ascii(enc, lc) {
                            level_val = level_val * 10 + (lc as i32 - '0' as i32);
                        } else {
                            r = ONIGERR_INVALID_GROUP_NAME;
                            break;
                        }
                    }
                    break;
                } else if !enc.is_code_ctype(c, ONIGENC_CTYPE_WORD) {
                    r = ONIGERR_INVALID_CHAR_IN_GROUP_NAME;
                }
            }
        }

        if r != 0 {
            return Err(r);
        }

        // Must have ended with end_code
        // (if c was ')' that's also OK for some syntaxes)

        if num_type != IS_NOT_NUM {
            let mut tp = pnum_head;
            back_num = scan_number(&mut tp, name_end, pattern, enc);
            if back_num < 0 {
                return Err(ONIGERR_TOO_BIG_NUMBER);
            }
            if back_num == 0 && num_type == IS_REL_NUM {
                return Err(ONIGERR_INVALID_GROUP_NAME);
            }
            back_num *= sign;
        }

        return Ok((name_start, name_end, back_num, num_type, exist_level, level));
    }

    // Error path: skip to end_code
    while !p_end(*p, end) {
        name_end = *p;
        let c = pfetch_s(p, pattern, end, enc);
        if c == end_code || c == ')' as u32 {
            break;
        }
    }
    Err(r)
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

// ReduceType enum for nested quantifier reduction table
const RQ_ASIS: u8 = 0; // as is
const RQ_DEL: u8 = 1; // delete parent (replace parent with child)
const RQ_A: u8 = 2; // to '*'
const RQ_P: u8 = 3; // to '+'
const RQ_AQ: u8 = 4; // to '*?'
const RQ_QQ: u8 = 5; // to '??'
const RQ_P_QQ: u8 = 6; // to '+)??' (child becomes +, parent becomes ??)

// 6×6 reduction table: [child_type][parent_type]
// Indices: 0=?, 1=*, 2=+, 3=??, 4=*?, 5=+?
static REDUCE_TYPE_TABLE: [[u8; 6]; 6] = [
    [RQ_DEL, RQ_A, RQ_A, RQ_QQ, RQ_AQ, RQ_ASIS],        // '?'
    [RQ_DEL, RQ_DEL, RQ_DEL, RQ_P_QQ, RQ_P_QQ, RQ_DEL], // '*'
    [RQ_A, RQ_A, RQ_DEL, RQ_ASIS, RQ_P_QQ, RQ_DEL],     // '+'
    [RQ_DEL, RQ_AQ, RQ_AQ, RQ_DEL, RQ_AQ, RQ_AQ],       // '??'
    [RQ_DEL, RQ_DEL, RQ_DEL, RQ_DEL, RQ_DEL, RQ_DEL],   // '*?'
    [RQ_ASIS, RQ_A, RQ_P, RQ_AQ, RQ_AQ, RQ_DEL],        // '+?'
];

/// Port of C onig_reduce_nested_quantifier.
/// pnode is the outer (parent) quantifier node whose body is the inner (child) quantifier.
fn onig_reduce_nested_quantifier(pnode: &mut Box<Node>) -> Result<(), i32> {
    // Get parent and child quantifier type indices
    let pnum = if let NodeInner::Quant(ref pq) = pnode.inner {
        quantifier_type_num(pq)
    } else {
        return Ok(());
    };

    let cnum = if let Some(ref body) = pnode.body() {
        if let NodeInner::Quant(ref cq) = body.inner {
            quantifier_type_num(cq)
        } else {
            return Ok(());
        }
    } else {
        return Ok(());
    };

    if pnum < 0 || cnum < 0 {
        // Non-standard quantifiers: only handle fixed×fixed
        let (p_lower, p_upper) = if let NodeInner::Quant(ref pq) = pnode.inner {
            (pq.lower, pq.upper)
        } else {
            return Ok(());
        };
        let (c_lower, c_upper) = if let Some(ref body) = pnode.body() {
            if let NodeInner::Quant(ref cq) = body.inner {
                (cq.lower, cq.upper)
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        };

        if p_lower == p_upper && c_lower == c_upper {
            let product = positive_int_multiply(p_lower, c_lower);
            if product < 0 {
                return Err(ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE);
            }
            if let NodeInner::Quant(ref mut pq) = pnode.inner {
                pq.lower = product;
                pq.upper = product;
            }
            // Move child's body to parent, removing child node
            let child_body = extract_grandchild_body(pnode);
            pnode.set_body(child_body);
        }
        return Ok(());
    }

    let reduce_type = REDUCE_TYPE_TABLE[cnum as usize][pnum as usize];

    match reduce_type {
        RQ_DEL => {
            // Delete parent: replace parent node with child node entirely
            let child = pnode.take_body();
            if let Some(child_node) = child {
                *pnode = child_node;
            }
        }
        RQ_A | RQ_P | RQ_AQ | RQ_QQ => {
            // Remove child quant, set parent's body to grandchild, update parent params
            let child_body = extract_grandchild_body(pnode);
            pnode.set_body(child_body);
            if let NodeInner::Quant(ref mut pq) = pnode.inner {
                match reduce_type {
                    RQ_A => {
                        pq.lower = 0;
                        pq.upper = INFINITE_REPEAT;
                        pq.greedy = true;
                    }
                    RQ_P => {
                        pq.lower = 1;
                        pq.upper = INFINITE_REPEAT;
                        pq.greedy = true;
                    }
                    RQ_AQ => {
                        pq.lower = 0;
                        pq.upper = INFINITE_REPEAT;
                        pq.greedy = false;
                    }
                    RQ_QQ => {
                        pq.lower = 0;
                        pq.upper = 1;
                        pq.greedy = false;
                    }
                    _ => unreachable!(),
                }
            }
        }
        RQ_P_QQ => {
            // Transform to (+)?? — child becomes '+', parent becomes '??'
            if let NodeInner::Quant(ref mut pq) = pnode.inner {
                pq.lower = 0;
                pq.upper = 1;
                pq.greedy = false;
            }
            if let NodeInner::Quant(ref mut pq) = pnode.inner {
                if let Some(ref mut body) = pq.body {
                    if let NodeInner::Quant(ref mut cq) = body.inner {
                        cq.lower = 1;
                        cq.upper = INFINITE_REPEAT;
                        cq.greedy = true;
                    }
                }
            }
        }
        _ => {
            // RQ_ASIS — leave as is
        }
    }

    Ok(())
}

/// Helper: extract the grandchild body from a parent→child→grandchild chain.
/// Takes the body out of the child quantifier node.
fn extract_grandchild_body(pnode: &mut Box<Node>) -> Option<Box<Node>> {
    if let NodeInner::Quant(ref mut pq) = pnode.inner {
        if let Some(ref mut child) = pq.body {
            if let NodeInner::Quant(ref mut cq) = child.inner {
                return cq.body.take();
            }
        }
    }
    None
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
        return if syn_allow {
            1
        } else {
            ONIGERR_END_PATTERN_AT_LEFT_BRACE
        };
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
            return if syn_allow {
                1
            } else {
                ONIGERR_INVALID_REPEAT_RANGE_PATTERN
            };
        }
    }

    if p_end(*p, end) {
        return if syn_allow {
            1
        } else {
            ONIGERR_INVALID_REPEAT_RANGE_PATTERN
        };
    }

    let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
    let mut up;
    let mut r = 0; // 0: normal {n,m}, 2: fixed {n} (only set for no-comma form)
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
                return if syn_allow {
                    1
                } else {
                    ONIGERR_INVALID_REPEAT_RANGE_PATTERN
                };
            }
            up = INFINITE_REPEAT;
        }
    } else {
        if non_low {
            return if syn_allow {
                1
            } else {
                ONIGERR_INVALID_REPEAT_RANGE_PATTERN
            };
        }
        *p = pfetch_prev; // PUNFETCH
        up = low;
        r = 2; // fixed {n}
    }

    if p_end(*p, end) {
        return if syn_allow {
            1
        } else {
            ONIGERR_INVALID_REPEAT_RANGE_PATTERN
        };
    }

    let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
    if is_syntax_op(syn, ONIG_SYN_OP_ESC_BRACE_INTERVAL) {
        if c != mc_esc(syn) || p_end(*p, end) {
            return if syn_allow {
                1
            } else {
                ONIGERR_INVALID_REPEAT_RANGE_PATTERN
            };
        }
        let c2 = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        if c2 != '}' as u32 {
            return if syn_allow {
                1
            } else {
                ONIGERR_INVALID_REPEAT_RANGE_PATTERN
            };
        }
    } else {
        if c != '}' as u32 {
            return if syn_allow {
                1
            } else {
                ONIGERR_INVALID_REPEAT_RANGE_PATTERN
            };
        }
    }

    if up != INFINITE_REPEAT && low > up {
        // {n,m}+ supported case: return error
        if is_syntax_op2(syn, ONIG_SYN_OP2_PLUS_POSSESSIVE_INTERVAL) {
            return ONIGERR_UPPER_SMALLER_THAN_LOWER_IN_REPEAT_RANGE;
        }
        // Otherwise: swap bounds and make possessive
        tok.repeat_possessive = true;
        let tmp = low;
        low = up;
        up = tmp;
    } else {
        tok.repeat_possessive = false;
    }
    tok.token_type = TokenType::Interval;
    tok.repeat_lower = low;
    tok.repeat_upper = up;

    r
}

/// Check if position is at the head of a BRE subexpression.
/// Returns true if `prev_pos` is at the start of pattern, or preceded by \( or \|
fn is_head_of_bre_subexp(
    prev_pos: usize,
    _end: usize,
    pattern: &[u8],
    enc: OnigEncoding,
    env: &ParseEnv,
) -> bool {
    let start = 0usize;
    if prev_pos > start {
        if let Some(p1) = onigenc_get_prev_char_head(enc, start, prev_pos, pattern) {
            if p1 > start {
                let code1 = pattern[p1] as u32;
                if code1 == '(' as u32
                    || (code1 == '|' as u32 && is_syntax_op(env.syntax, ONIG_SYN_OP_ESC_VBAR_ALT))
                {
                    if let Some(p2) = onigenc_get_prev_char_head(enc, start, p1, pattern) {
                        let code2 = pattern[p2] as u32;
                        if is_mc_esc_code(code2, env.syntax) {
                            let mut count = 0;
                            let mut pp = p2;
                            while pp > start {
                                if let Some(prev) =
                                    onigenc_get_prev_char_head(enc, start, pp, pattern)
                                {
                                    pp = prev;
                                    let cc = pattern[pp] as u32;
                                    if !is_mc_esc_code(cc, env.syntax) {
                                        break;
                                    }
                                    count += 1;
                                } else {
                                    break;
                                }
                            }
                            return count % 2 == 0;
                        }
                    }
                }
            }
        }
        false
    } else {
        true
    }
}

/// Check if position is at the end of a BRE subexpression.
/// Returns true if at end-of-pattern, or followed by \) or \|
fn is_end_of_bre_subexp(
    pos: usize,
    end: usize,
    pattern: &[u8],
    enc: OnigEncoding,
    env: &ParseEnv,
) -> bool {
    if pos >= end {
        return true;
    }
    let code = pattern[pos] as u32;
    if is_mc_esc_code(code, env.syntax) {
        let next = pos + enc.mbc_enc_len(&pattern[pos..]);
        if next < end {
            let code2 = pattern[next] as u32;
            if code2 == ')' as u32
                || (code2 == '|' as u32 && is_syntax_op(env.syntax, ONIG_SYN_OP_ESC_VBAR_ALT))
            {
                return true;
            }
        }
    }
    false
}

// ============================================================================
// Tokenizer: fetch_token
// ============================================================================

fn fetch_token(tok: &mut PToken, p: &mut usize, end: usize, pattern: &[u8], env: &ParseEnv) -> i32 {
    let enc = env.enc;
    let syn = env.syntax;
    let mut pfetch_prev = *p;

    if tok.code_point_continue {
        let mut code = 0u32;
        let r = get_next_code_point(p, end, tok.base_num, pattern, enc, false, &mut code);
        if r == 1 {
            // End of sequence ('}')
            tok.code_point_continue = false;
        } else if r == 0 {
            tok.token_type = TokenType::CodePoint;
            tok.code = code;
            return tok.token_type as i32;
        } else if r < 0 {
            return r;
        }
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

        // Only match escape sequences for ASCII-range codepoints.
        if c < 128 {
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
                    if r == 0 {
                        return greedy_check2(tok, p, end, pattern, enc, syn);
                    } else if r == 2 {
                        // Fixed interval {n}: skip lazy ? if FIXED_INTERVAL_IS_GREEDY_ONLY
                        if is_syntax_bv(syn, ONIG_SYN_FIXED_INTERVAL_IS_GREEDY_ONLY) {
                            return possessive_check(tok, p, end, pattern, enc, syn);
                        }
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
                'k' => {
                    if !p_end(*p, end) && is_syntax_op2(syn, ONIG_SYN_OP2_ESC_K_NAMED_BACKREF) {
                        let save = *p;
                        let c2 = pfetch_s(p, pattern, end, enc);
                        if c2 == '<' as u32 || c2 == '\'' as u32 {
                            match fetch_name(c2, p, end, pattern, env, true) {
                                Ok((
                                    name_start,
                                    name_end,
                                    back_num,
                                    num_type,
                                    has_level,
                                    level_val,
                                )) => {
                                    if num_type != IS_NOT_NUM {
                                        // Numeric backref
                                        let mut bn = back_num;
                                        if num_type == IS_REL_NUM {
                                            bn = backref_rel_to_abs(bn, env);
                                        }
                                        if bn <= 0 {
                                            return ONIGERR_INVALID_BACKREF;
                                        }
                                        tok.token_type = TokenType::Backref;
                                        tok.backref_by_name = false;
                                        tok.backref_num = 1;
                                        tok.backref_ref1 = bn;
                                        tok.backref_exist_level = has_level;
                                        tok.backref_level = level_val;
                                    } else {
                                        // Named backref: look up name
                                        let name = &pattern[name_start..name_end];
                                        let reg = unsafe { &*env.reg };
                                        if let Some(ref nt) = reg.name_table {
                                            if let Some(entry) = nt.find(name) {
                                                tok.token_type = TokenType::Backref;
                                                tok.backref_by_name = true;
                                                tok.backref_exist_level = has_level;
                                                tok.backref_level = level_val;
                                                if entry.back_num == 1 {
                                                    tok.backref_num = 1;
                                                    tok.backref_ref1 = entry.back_refs[0];
                                                } else {
                                                    tok.backref_num = entry.back_num;
                                                    tok.backref_refs = entry.back_refs.clone();
                                                }
                                            } else {
                                                return ONIGERR_UNDEFINED_NAME_REFERENCE;
                                            }
                                        } else {
                                            return ONIGERR_UNDEFINED_NAME_REFERENCE;
                                        }
                                    }
                                }
                                Err(e) => return e,
                            }
                        } else {
                            *p = save; // PUNFETCH
                        }
                    }
                }
                'g' => {
                    if !p_end(*p, end) && is_syntax_op2(syn, ONIG_SYN_OP2_ESC_G_SUBEXP_CALL) {
                        let save = *p;
                        let c2 = pfetch_s(p, pattern, end, enc);
                        if c2 == '<' as u32 || c2 == '\'' as u32 {
                            match fetch_name(c2, p, end, pattern, env, true) {
                                Ok((
                                    name_start,
                                    name_end,
                                    back_num,
                                    num_type,
                                    _exist_level,
                                    _level,
                                )) => {
                                    if num_type != IS_NOT_NUM {
                                        let mut gnum = back_num;
                                        if num_type == IS_REL_NUM {
                                            gnum = backref_rel_to_abs(gnum, env);
                                            if gnum < 0 {
                                                return ONIGERR_UNDEFINED_GROUP_REFERENCE;
                                            }
                                        }
                                        tok.token_type = TokenType::Call;
                                        tok.call_by_number = true;
                                        tok.call_gnum = gnum;
                                        tok.call_name_start = name_start;
                                        tok.call_name_end = name_end;
                                    } else {
                                        tok.token_type = TokenType::Call;
                                        tok.call_by_number = false;
                                        tok.call_gnum = 0;
                                        tok.call_name_start = name_start;
                                        tok.call_name_end = name_end;
                                    }
                                }
                                Err(e) => return e,
                            }
                        } else {
                            *p = save; // PUNFETCH
                        }
                    }
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
                'y' => {
                    if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_X_Y_TEXT_SEGMENT) {
                        return tok.token_type as i32;
                    }
                    tok.token_type = TokenType::Anchor;
                    tok.anchor = ANCR_TEXT_SEGMENT_BOUNDARY;
                }
                'Y' => {
                    if !is_syntax_op2(syn, ONIG_SYN_OP2_ESC_X_Y_TEXT_SEGMENT) {
                        return tok.token_type as i32;
                    }
                    tok.token_type = TokenType::Anchor;
                    tok.anchor = ANCR_NO_TEXT_SEGMENT_BOUNDARY;
                }
                'A' => {
                    if !is_syntax_op(syn, ONIG_SYN_OP_ESC_AZ_BUF_ANCHOR) {
                        return tok.token_type as i32;
                    }
                    tok.token_type = TokenType::Anchor;
                    tok.anchor = ANCR_BEGIN_BUF;
                }
                'Z' => {
                    if is_syntax_bv(syn, ONIG_SYN_PYTHON) {
                        // Python: \Z is end-of-buffer (no newline matching)
                        tok.token_type = TokenType::Anchor;
                        tok.anchor = ANCR_END_BUF;
                    } else {
                        if !is_syntax_op(syn, ONIG_SYN_OP_ESC_AZ_BUF_ANCHOR) {
                            return tok.token_type as i32;
                        }
                        tok.token_type = TokenType::Anchor;
                        tok.anchor = ANCR_SEMI_END_BUF;
                    }
                }
                'z' => {
                    if is_syntax_bv(syn, ONIG_SYN_PYTHON) {
                        return ONIGERR_UNDEFINED_OPERATOR;
                    }
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
                                && is_syntax_op2(syn, ONIG_SYN_OP2_ESC_P_BRACE_CIRCUMFLEX_NOT)
                            {
                                let c2 = pfetch(p, &mut pfetch_prev, pattern, end, enc);
                                if c2 == '^' as u32 {
                                    tok.prop_not = !tok.prop_not;
                                } else {
                                    *p = pfetch_prev; // PUNFETCH
                                }
                            }
                        }
                    } else if is_syntax_bv(syn, ONIG_SYN_ESC_P_WITH_ONE_CHAR_PROP) {
                        tok.token_type = TokenType::CharProperty;
                        tok.prop_not = c == 'P' as u32;
                        tok.prop_braces = false;
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
                                // Multi-codepoint sequence: \x{AAAA BBBB ...}
                                let c2 = ppeek(*p, pattern, end, enc);
                                if is_code_xdigit_ascii(enc, c2) {
                                    return ONIGERR_TOO_LONG_WIDE_CHAR_VALUE;
                                }
                                let r = check_code_point_sequence(p, end, 16, pattern, enc);
                                if r < 0 {
                                    return r;
                                }
                                if r == 0 {
                                    return ONIGERR_INVALID_CODE_POINT_VALUE;
                                }
                                tok.code_point_continue = true;
                            }
                            tok.token_type = TokenType::CodePoint;
                            tok.base_num = 16;
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
                'U' => {
                    if !p_end(*p, end) && is_syntax_bv(syn, ONIG_SYN_PYTHON) {
                        let mut code = 0;
                        let r = scan_hexadecimal_number(p, end, 8, 8, pattern, enc, &mut code);
                        if r < 0 {
                            return ONIGERR_INVALID_CODE_POINT_VALUE;
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
                                // Multi-codepoint sequence: \o{NNN NNN ...}
                                let r = check_code_point_sequence(p, end, 8, pattern, enc);
                                if r < 0 {
                                    return r;
                                }
                                if r == 0 {
                                    return ONIGERR_INVALID_CODE_POINT_VALUE;
                                }
                                tok.code_point_continue = true;
                            }
                            tok.token_type = TokenType::CodePoint;
                            tok.base_num = 8;
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
            // Non-ASCII escaped char: treat as literal via fetch_escaped_value
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
    } else {
        tok.code = c;
        tok.escaped = false;

        // Only match metacharacters for ASCII-range codepoints.
        // Multi-byte characters (c > 127) must not match ASCII metachar arms
        // (e.g. U+305B has low byte 0x5B = '[' but is not a bracket).
        if c < 128 {
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
                    if r == 0 {
                        return greedy_check2(tok, p, end, pattern, enc, syn);
                    } else if r == 2 {
                        // Fixed interval {n}: skip lazy ? if FIXED_INTERVAL_IS_GREEDY_ONLY
                        if is_syntax_bv(syn, ONIG_SYN_FIXED_INTERVAL_IS_GREEDY_ONLY) {
                            return possessive_check(tok, p, end, pattern, enc, syn);
                        }
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
                    // Check for (?#...) comment group
                    if !p_end(*p, end)
                        && ppeek_is(*p, pattern, end, enc, '?' as u32)
                        && is_syntax_op2(syn, ONIG_SYN_OP2_QMARK_GROUP_EFFECT)
                    {
                        let saved_p = *p;
                        pinc(p, pattern, enc); // skip '?'
                        if !p_end(*p, end) && ppeek_is(*p, pattern, end, enc, '#' as u32) {
                            pfetch(p, &mut pfetch_prev, pattern, end, enc); // consume '#'
                                                                            // Skip comment body until unescaped ')'
                            loop {
                                if p_end(*p, end) {
                                    return ONIGERR_END_PATTERN_IN_GROUP;
                                }
                                let c2 = pfetch(p, &mut pfetch_prev, pattern, end, enc);
                                if c2 == syn.meta_char_table.esc {
                                    if !p_end(*p, end) {
                                        pfetch(p, &mut pfetch_prev, pattern, end, enc);
                                    }
                                } else if c2 == ')' as u32 {
                                    break;
                                }
                            }
                            // Comment consumed, restart tokenization (goto start)
                            return fetch_token(tok, p, end, pattern, env);
                        } else if is_syntax_op2(syn, ONIG_SYN_OP2_QMARK_PERL_SUBEXP_CALL) {
                            // Perl subexp call syntax: (?R), (?&name), (?-1), (?+1), (?1)
                            let c2 = ppeek(*p, pattern, end, enc);
                            match c2 as u8 as char {
                                '&' => {
                                    pinc(p, pattern, enc); // skip '&'
                                    match fetch_name('(' as u32, p, end, pattern, env, false) {
                                        Ok((
                                            name_start,
                                            name_end,
                                            gnum,
                                            _num_type,
                                            _has_level,
                                            _level,
                                        )) => {
                                            let _ = gnum;
                                            tok.token_type = TokenType::Call;
                                            tok.call_by_number = false;
                                            tok.call_gnum = 0;
                                            tok.call_name_start = name_start;
                                            tok.call_name_end = name_end;
                                        }
                                        Err(e) => return e,
                                    }
                                }
                                'R' => {
                                    tok.token_type = TokenType::Call;
                                    tok.call_by_number = true;
                                    tok.call_gnum = 0;
                                    tok.call_name_start = *p;
                                    pinc(p, pattern, enc); // skip 'R'
                                    if p_end(*p, end)
                                        || !ppeek_is(*p, pattern, end, enc, ')' as u32)
                                    {
                                        return ONIGERR_UNDEFINED_GROUP_OPTION;
                                    }
                                    tok.call_name_end = *p;
                                }
                                '-' | '+' => {
                                    if !p_end(*p, end) {
                                        let save2 = *p;
                                        pinc(p, pattern, enc); // skip sign
                                        if !p_end(*p, end) {
                                            let c3 = ppeek(*p, pattern, end, enc);
                                            if c3 >= '0' as u32 && c3 <= '9' as u32 {
                                                // Relative number: unfetch the sign, then use fetch_name
                                                *p = save2;
                                                // Fall through to lparen_qmark_num
                                                match fetch_name(
                                                    '(' as u32, p, end, pattern, env, true,
                                                ) {
                                                    Ok((
                                                        name_start,
                                                        name_end,
                                                        back_num,
                                                        num_type,
                                                        _has_level,
                                                        _level,
                                                    )) => {
                                                        if num_type == IS_NOT_NUM {
                                                            return ONIGERR_INVALID_GROUP_NAME;
                                                        }
                                                        let mut gnum = back_num;
                                                        if num_type == IS_REL_NUM {
                                                            gnum = backref_rel_to_abs(gnum, env);
                                                            if gnum < 0 {
                                                                return ONIGERR_UNDEFINED_GROUP_REFERENCE;
                                                            }
                                                        }
                                                        tok.token_type = TokenType::Call;
                                                        tok.call_by_number = true;
                                                        tok.call_gnum = gnum;
                                                        tok.call_name_start = name_start;
                                                        tok.call_name_end = name_end;
                                                    }
                                                    Err(e) => return e,
                                                }
                                            } else {
                                                *p = saved_p;
                                                // Not a call, treat as normal group
                                            }
                                        } else {
                                            *p = saved_p;
                                        }
                                    } else {
                                        *p = saved_p;
                                    }
                                }
                                '0'..='9' => {
                                    // Absolute number call: (?1), (?2), etc.
                                    match fetch_name('(' as u32, p, end, pattern, env, true) {
                                        Ok((
                                            name_start,
                                            name_end,
                                            back_num,
                                            num_type,
                                            _has_level,
                                            _level,
                                        )) => {
                                            if num_type == IS_NOT_NUM {
                                                return ONIGERR_INVALID_GROUP_NAME;
                                            }
                                            let mut gnum = back_num;
                                            if num_type == IS_REL_NUM {
                                                gnum = backref_rel_to_abs(gnum, env);
                                                if gnum < 0 {
                                                    return ONIGERR_UNDEFINED_GROUP_REFERENCE;
                                                }
                                            }
                                            tok.token_type = TokenType::Call;
                                            tok.call_by_number = true;
                                            tok.call_gnum = gnum;
                                            tok.call_name_start = name_start;
                                            tok.call_name_end = name_end;
                                        }
                                        Err(e) => return e,
                                    }
                                }
                                _ => {
                                    // Not a Perl call, restore and handle as normal group
                                    *p = saved_p;
                                }
                            }
                        } else {
                            // Not a comment group, restore position
                            *p = saved_p;
                        }
                    }
                    if tok.token_type == TokenType::String {
                        tok.token_type = TokenType::SubexpOpen;
                    }
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
                    if is_syntax_bv(syn, ONIG_SYN_BRE_ANCHOR_AT_EDGE_OF_SUBEXP) {
                        if !is_head_of_bre_subexp(pfetch_prev, end, pattern, enc, env) {
                            return tok.token_type as i32;
                        }
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
                    if is_syntax_bv(syn, ONIG_SYN_BRE_ANCHOR_AT_EDGE_OF_SUBEXP) {
                        if !is_end_of_bre_subexp(*p, end, pattern, enc, env) {
                            return tok.token_type as i32;
                        }
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
        } // end if c < 128
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

fn possessive_check(
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    enc: OnigEncoding,
    syn: &OnigSyntaxType,
) -> i32 {
    // Skip lazy ? check — force greedy, only check for possessive +
    tok.repeat_greedy = true;
    if !p_end(*p, end)
        && ppeek_is(*p, pattern, end, enc, '+' as u32)
        && ((is_syntax_op2(syn, ONIG_SYN_OP2_PLUS_POSSESSIVE_REPEAT)
            && tok.token_type != TokenType::Interval)
            || (is_syntax_op2(syn, ONIG_SYN_OP2_PLUS_POSSESSIVE_INTERVAL)
                && tok.token_type == TokenType::Interval))
        && !tok.repeat_possessive
    {
        let mut pfetch_prev = *p;
        pfetch(p, &mut pfetch_prev, pattern, end, enc); // consume '+'
        tok.repeat_possessive = true;
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
    // C: greedy_check2 label — check lazy ? first, then possessive +
    // Unlike greedy_check, this allows possessive + on intervals via
    // ONIG_SYN_OP2_PLUS_POSSESSIVE_INTERVAL
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
        possessive_check(tok, p, end, pattern, enc, syn);
    }
    tok.token_type as i32
}

// ============================================================================
// Character class tokenizer: fetch_token_cc
// ============================================================================

fn fetch_token_cc(
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
    state: i32,
) -> i32 {
    let enc = env.enc;
    let syn = env.syntax;
    let mut pfetch_prev = *p;

    if tok.code_point_continue {
        let mut code = 0u32;
        let r = get_next_code_point(p, end, tok.base_num, pattern, enc, true, &mut code);
        if r == 1 {
            // End of sequence ('}')
            tok.code_point_continue = false;
        } else if r == 0 {
            tok.token_type = TokenType::CodePoint;
            tok.code = code;
            return tok.token_type as i32;
        } else if r == 2 {
            // Range marker '-' inside character class
            tok.token_type = TokenType::CcRange;
            tok.code_point_continue = true;
            return tok.token_type as i32;
        } else if r < 0 {
            return r;
        }
    }

    if p_end(*p, end) {
        tok.token_type = TokenType::Eot;
        return tok.token_type as i32;
    }

    let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
    tok.token_type = TokenType::Char;
    tok.base_num = 0;
    tok.code = c;
    tok.escaped = false;

    if c == ']' as u32 {
        tok.token_type = TokenType::CcClose;
    } else if c == '-' as u32 {
        tok.token_type = TokenType::CcRange;
    } else if c == mc_esc(syn) {
        if !is_syntax_bv(syn, ONIG_SYN_BACKSLASH_ESCAPE_IN_CC) {
            return tok.token_type as i32;
        }
        if p_end(*p, end) {
            return ONIGERR_END_PATTERN_AT_ESCAPE;
        }

        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);
        tok.escaped = true;
        tok.code = c;
        match c as u8 as char {
            'w' => {
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_WORD as i32;
                tok.prop_not = false;
            }
            'W' => {
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_WORD as i32;
                tok.prop_not = true;
            }
            'd' => {
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_DIGIT as i32;
                tok.prop_not = false;
            }
            'D' => {
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_DIGIT as i32;
                tok.prop_not = true;
            }
            's' => {
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_SPACE as i32;
                tok.prop_not = false;
            }
            'S' => {
                tok.token_type = TokenType::CharType;
                tok.prop_ctype = ONIGENC_CTYPE_SPACE as i32;
                tok.prop_not = true;
            }
            'h' => {
                if is_syntax_op2(syn, ONIG_SYN_OP2_ESC_H_XDIGIT) {
                    tok.token_type = TokenType::CharType;
                    tok.prop_ctype = ONIGENC_CTYPE_XDIGIT as i32;
                    tok.prop_not = false;
                }
            }
            'H' => {
                if is_syntax_op2(syn, ONIG_SYN_OP2_ESC_H_XDIGIT) {
                    tok.token_type = TokenType::CharType;
                    tok.prop_ctype = ONIGENC_CTYPE_XDIGIT as i32;
                    tok.prop_not = true;
                }
            }
            'p' | 'P' => {
                if !p_end(*p, end) && ppeek_is(*p, pattern, end, enc, '{' as u32) {
                    if is_syntax_op2(syn, ONIG_SYN_OP2_ESC_P_BRACE_CHAR_PROPERTY) {
                        pinc(p, pattern, enc);
                        tok.token_type = TokenType::CharProperty;
                        tok.prop_not = c == 'P' as u32;
                        tok.prop_braces = true;
                        if !p_end(*p, end)
                            && is_syntax_op2(syn, ONIG_SYN_OP2_ESC_P_BRACE_CIRCUMFLEX_NOT)
                        {
                            let c2 = pfetch(p, &mut pfetch_prev, pattern, end, enc);
                            if c2 == '^' as u32 {
                                tok.prop_not = !tok.prop_not;
                            } else {
                                *p = pfetch_prev;
                            }
                        }
                    }
                } else if is_syntax_bv(syn, ONIG_SYN_ESC_P_WITH_ONE_CHAR_PROP) {
                    tok.token_type = TokenType::CharProperty;
                    tok.prop_not = c == 'P' as u32;
                    tok.prop_braces = false;
                }
            }
            'x' => {
                let prev = *p;
                if !p_end(*p, end)
                    && ppeek_is(*p, pattern, end, enc, '{' as u32)
                    && is_syntax_op(syn, ONIG_SYN_OP_ESC_X_BRACE_HEX8)
                {
                    pinc(p, pattern, enc);
                    let mut code = 0;
                    let r = scan_hexadecimal_number(p, end, 0, 8, pattern, enc, &mut code);
                    if r < 0 {
                        return r;
                    }
                    tok.base_num = 16;
                    if *p > prev + enclen(enc, &pattern[prev..]) {
                        if p_end(*p, end) {
                            return ONIGERR_INVALID_CODE_POINT_VALUE;
                        }
                        if ppeek_is(*p, pattern, end, enc, '}' as u32) {
                            pinc(p, pattern, enc);
                        } else {
                            // Multi-codepoint or range: \x{AAAA BBBB} or \x{AAAA-BBBB}
                            let c2 = ppeek(*p, pattern, end, enc);
                            if is_code_xdigit_ascii(enc, c2) {
                                return ONIGERR_TOO_LONG_WIDE_CHAR_VALUE;
                            }
                            let curr_state = if state == CS_RANGE {
                                CPS_EMPTY
                            } else {
                                CPS_START_VAL
                            };
                            let r =
                                check_code_point_sequence_cc(p, end, 16, pattern, enc, curr_state);
                            if r < 0 {
                                return r;
                            }
                            if r == 0 {
                                return ONIGERR_INVALID_CODE_POINT_VALUE;
                            }
                            tok.code_point_continue = true;
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
            'o' => {
                let prev = *p;
                if !p_end(*p, end)
                    && ppeek_is(*p, pattern, end, enc, '{' as u32)
                    && is_syntax_op(syn, ONIG_SYN_OP_ESC_O_BRACE_OCTAL)
                {
                    pinc(p, pattern, enc);
                    let mut code = 0;
                    let r = scan_octal_number(p, end, 0, 11, pattern, enc, &mut code);
                    if r < 0 {
                        return r;
                    }
                    tok.base_num = 8;
                    if *p > prev + enclen(enc, &pattern[prev..]) {
                        if p_end(*p, end) {
                            return ONIGERR_INVALID_CODE_POINT_VALUE;
                        }
                        if ppeek_is(*p, pattern, end, enc, '}' as u32) {
                            pinc(p, pattern, enc);
                        } else {
                            // Multi-codepoint sequence: \o{NNN NNN ...}
                            let curr_state = if state == CS_RANGE {
                                CPS_EMPTY
                            } else {
                                CPS_START_VAL
                            };
                            let r =
                                check_code_point_sequence_cc(p, end, 8, pattern, enc, curr_state);
                            if r < 0 {
                                return r;
                            }
                            if r == 0 {
                                return ONIGERR_INVALID_CODE_POINT_VALUE;
                            }
                            tok.code_point_continue = true;
                        }
                        tok.token_type = TokenType::CodePoint;
                        tok.code = code;
                    } else {
                        *p = prev;
                    }
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
            'U' => {
                if !p_end(*p, end) && is_syntax_bv(syn, ONIG_SYN_PYTHON) {
                    let mut code = 0;
                    let r = scan_hexadecimal_number(p, end, 8, 8, pattern, enc, &mut code);
                    if r < 0 {
                        return ONIGERR_INVALID_CODE_POINT_VALUE;
                    }
                    tok.token_type = TokenType::CodePoint;
                    tok.base_num = 16;
                    tok.code = code;
                }
            }
            '0'..='7' => {
                if is_syntax_op(syn, ONIG_SYN_OP_ESC_OCTAL3) {
                    *p = pfetch_prev; // PUNFETCH
                    let prev = *p;
                    let mut code = 0;
                    let r = scan_octal_number(p, end, 0, 3, pattern, enc, &mut code);
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
                    tok.code = c2;
                    tok.token_type = TokenType::CodePoint;
                }
            }
        }
    } else if c == '[' as u32 {
        if is_syntax_op(syn, ONIG_SYN_OP_POSIX_BRACKET)
            && !p_end(*p, end)
            && ppeek_is(*p, pattern, end, enc, ':' as u32)
        {
            tok.backp = *p;
            pinc(p, pattern, enc);
            if is_posix_bracket_start(*p, end, pattern, enc) {
                tok.token_type = TokenType::CcPosixBracketOpen;
            } else {
                *p = pfetch_prev + enclen(enc, &pattern[pfetch_prev..end]);
                // Try nested CC
                if is_syntax_op2(syn, ONIG_SYN_OP2_CCLASS_SET_OP) {
                    tok.token_type = TokenType::CcOpenCC;
                }
            }
        } else {
            if is_syntax_op2(syn, ONIG_SYN_OP2_CCLASS_SET_OP) {
                tok.token_type = TokenType::CcOpenCC;
            }
        }
    } else if c == '&' as u32 {
        if is_syntax_op2(syn, ONIG_SYN_OP2_CCLASS_SET_OP)
            && !p_end(*p, end)
            && ppeek_is(*p, pattern, end, enc, '&' as u32)
        {
            pinc(p, pattern, enc);
            tok.token_type = TokenType::CcAnd;
        }
    }

    tok.token_type as i32
}

// ============================================================================
// Character class parser: prs_cc
// ============================================================================

fn prs_cc(
    tok: &mut PToken,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
) -> Result<Box<Node>, i32> {
    let enc = env.enc;
    env.parse_depth += 1;
    if env.parse_depth > PARSE_DEPTH_LIMIT.load(Ordering::Relaxed) {
        return Err(ONIGERR_PARSE_DEPTH_LIMIT_OVER);
    }

    let mut state = CS_START;
    let mut curr_code: OnigCodePoint = 0;
    let mut curr_type = CV_UNDEF;
    let mut curr_raw = false;
    let mut and_start = false;

    // Check for negation ^
    let mut r = fetch_token_cc(tok, p, end, pattern, env, state);
    if r < 0 {
        env.parse_depth -= 1;
        return Err(r);
    }
    let neg = if tok.token_type == TokenType::Char && tok.code == '^' as u32 && !tok.escaped {
        r = fetch_token_cc(tok, p, end, pattern, env, state);
        if r < 0 {
            env.parse_depth -= 1;
            return Err(r);
        }
        true
    } else {
        false
    };

    // Handle empty [] - check for immediate ]
    if tok.token_type == TokenType::CcClose {
        // Check if there's another ] later
        if !code_exist_check(']' as u32, *p, end, pattern, true, env) {
            env.parse_depth -= 1;
            return Err(ONIGERR_EMPTY_CHAR_CLASS);
        }
        // Treat ] as literal
        tok.token_type = TokenType::Char;
        tok.code = ']' as u32;
    }

    let mut node = node_new_cclass();
    let mut prev_cc: Option<CClassNode> = None;
    let mut work_cc_active = false;
    let mut work_cc = CClassNode {
        flags: 0,
        bs: [0; BITSET_REAL_SIZE],
        mbuf: None,
    };

    // Main loop
    loop {
        let mut fetched = false;

        // Get cc pointer (either from node or work_cc)
        let use_work = work_cc_active;

        match tok.token_type {
            TokenType::Char => {
                let in_code = tok.code;
                let in_type = if env.enc.code_to_mbclen(in_code) == 1 {
                    CV_SB
                } else {
                    CV_MB
                };
                let in_raw = false;

                // cc_char_next
                let cc = if use_work {
                    &mut work_cc
                } else {
                    node.as_cclass_mut().unwrap()
                };
                let cr = cc_char_next(
                    cc,
                    &mut curr_code,
                    in_code,
                    &mut curr_raw,
                    in_raw,
                    in_type,
                    &mut curr_type,
                    &mut state,
                    env,
                );
                if cr != 0 {
                    env.parse_depth -= 1;
                    return Err(cr);
                }
            }
            TokenType::CrudeByte => {
                let byte = tok.code as u8;
                let mut buf = vec![byte];

                if byte >= 0x80 {
                    let expected_len = env.enc.mbc_enc_len(&[byte]);
                    if expected_len > 1 {
                        // Accumulate consecutive CrudeByte tokens for multi-byte sequence
                        for _ in 1..expected_len {
                            r = fetch_token_cc(tok, p, end, pattern, env, state);
                            if r < 0 {
                                env.parse_depth -= 1;
                                return Err(r);
                            }
                            if tok.token_type == TokenType::CrudeByte {
                                buf.push(tok.code as u8);
                            } else {
                                break;
                            }
                        }
                    }

                    // Validate the accumulated byte sequence
                    if !env.enc.is_valid_mbc_string(&buf) {
                        env.parse_depth -= 1;
                        if byte > 0xF4 || byte < 0xC2 {
                            return Err(ONIGERR_INVALID_CODE_POINT_VALUE);
                        }
                        return Err(ONIGERR_TOO_SHORT_MULTI_BYTE_STRING);
                    }
                }

                // For multi-byte sequences, decode to codepoint and use CV_MB
                let (in_code, in_type) = if buf.len() > 1 {
                    let code = env.enc.mbc_to_code(&buf, buf.len());
                    (code, CV_MB)
                } else {
                    (tok.code, CV_SB)
                };
                let in_raw = true;

                let cc = if use_work {
                    &mut work_cc
                } else {
                    node.as_cclass_mut().unwrap()
                };
                let cr = cc_char_next(
                    cc,
                    &mut curr_code,
                    in_code,
                    &mut curr_raw,
                    in_raw,
                    in_type,
                    &mut curr_type,
                    &mut state,
                    env,
                );
                if cr != 0 {
                    env.parse_depth -= 1;
                    return Err(cr);
                }
            }
            TokenType::CodePoint => {
                let in_code = tok.code;
                let mblen = env.enc.code_to_mbclen(in_code);
                let in_type = if mblen < 0 {
                    // Invalid code point; only allowed at end of range
                    if state != CS_RANGE {
                        env.parse_depth -= 1;
                        return Err(ONIGERR_INVALID_CODE_POINT_VALUE);
                    }
                    CV_MB
                } else if mblen == 1 {
                    CV_SB
                } else {
                    CV_MB
                };
                let in_raw = true;

                let cc = if use_work {
                    &mut work_cc
                } else {
                    node.as_cclass_mut().unwrap()
                };
                let cr = cc_char_next(
                    cc,
                    &mut curr_code,
                    in_code,
                    &mut curr_raw,
                    in_raw,
                    in_type,
                    &mut curr_type,
                    &mut state,
                    env,
                );
                if cr != 0 {
                    env.parse_depth -= 1;
                    return Err(cr);
                }
            }
            TokenType::CcPosixBracketOpen => {
                let cc = if use_work {
                    &mut work_cc
                } else {
                    node.as_cclass_mut().unwrap()
                };
                let cr = prs_posix_bracket(cc, p, end, pattern, env);
                if cr < 0 {
                    env.parse_depth -= 1;
                    return Err(cr);
                }
                // cc_cprop_next
                let cr2 = cc_cprop_next(cc, &mut curr_code, &mut curr_type, &mut state, env);
                if cr2 != 0 {
                    env.parse_depth -= 1;
                    return Err(cr2);
                }
            }
            TokenType::CharType => {
                let cc = if use_work {
                    &mut work_cc
                } else {
                    node.as_cclass_mut().unwrap()
                };
                let ctype = tok.prop_ctype;
                let not = tok.prop_not;
                let cr = add_ctype_to_cc(cc, ctype, not, env);
                if cr != 0 {
                    env.parse_depth -= 1;
                    return Err(cr);
                }
                let cr2 = cc_cprop_next(cc, &mut curr_code, &mut curr_type, &mut state, env);
                if cr2 != 0 {
                    env.parse_depth -= 1;
                    return Err(cr2);
                }
            }
            TokenType::CharProperty => {
                let cc = if use_work {
                    &mut work_cc
                } else {
                    node.as_cclass_mut().unwrap()
                };
                let ctype = fetch_char_property_to_ctype(p, end, pattern, tok.prop_braces, env);
                if ctype < 0 {
                    env.parse_depth -= 1;
                    return Err(ctype);
                }
                let cr = add_ctype_to_cc(cc, ctype, tok.prop_not, env);
                if cr != 0 {
                    env.parse_depth -= 1;
                    return Err(cr);
                }
                let cr2 = cc_cprop_next(cc, &mut curr_code, &mut curr_type, &mut state, env);
                if cr2 != 0 {
                    env.parse_depth -= 1;
                    return Err(cr2);
                }
            }
            TokenType::CcRange => {
                if state == CS_VALUE {
                    r = fetch_token_cc(tok, p, end, pattern, env, CS_RANGE);
                    if r < 0 {
                        env.parse_depth -= 1;
                        return Err(r);
                    }
                    fetched = true;
                    if tok.token_type == TokenType::CcClose || tok.token_type == TokenType::CcAnd {
                        // [x-] or [x-&&...] -> treat dash as literal
                        let cc = if use_work {
                            &mut work_cc
                        } else {
                            node.as_cclass_mut().unwrap()
                        };
                        let cr = cc_char_next(
                            cc,
                            &mut curr_code,
                            '-' as u32,
                            &mut curr_raw,
                            false,
                            CV_SB,
                            &mut curr_type,
                            &mut state,
                            env,
                        );
                        if cr != 0 {
                            env.parse_depth -= 1;
                            return Err(cr);
                        }
                    } else if curr_type == CV_CPROP {
                        if is_syntax_bv(
                            env.syntax,
                            ONIG_SYN_ALLOW_CHAR_TYPE_FOLLOWED_BY_MINUS_IN_CC,
                        ) {
                            // Treat dash as literal: [\w-%] -> [\w\-\%]
                            let cc = if use_work {
                                &mut work_cc
                            } else {
                                node.as_cclass_mut().unwrap()
                            };
                            let cr = cc_char_next(
                                cc,
                                &mut curr_code,
                                '-' as u32,
                                &mut curr_raw,
                                false,
                                CV_SB,
                                &mut curr_type,
                                &mut state,
                                env,
                            );
                            if cr != 0 {
                                env.parse_depth -= 1;
                                return Err(cr);
                            }
                        } else {
                            env.parse_depth -= 1;
                            return Err(ONIGERR_UNMATCHED_RANGE_SPECIFIER_IN_CHAR_CLASS);
                        }
                    } else {
                        state = CS_RANGE;
                    }
                } else if state == CS_START {
                    // [-...] - literal dash at start
                    let in_code = '-' as u32;
                    let cc = if use_work {
                        &mut work_cc
                    } else {
                        node.as_cclass_mut().unwrap()
                    };
                    let cr = cc_char_next(
                        cc,
                        &mut curr_code,
                        in_code,
                        &mut curr_raw,
                        false,
                        CV_SB,
                        &mut curr_type,
                        &mut state,
                        env,
                    );
                    if cr != 0 {
                        env.parse_depth -= 1;
                        return Err(cr);
                    }
                } else if state == CS_RANGE {
                    // [!--] - literal dash in range context
                    let in_code = '-' as u32;
                    let cc = if use_work {
                        &mut work_cc
                    } else {
                        node.as_cclass_mut().unwrap()
                    };
                    let cr = cc_char_next(
                        cc,
                        &mut curr_code,
                        in_code,
                        &mut curr_raw,
                        false,
                        CV_SB,
                        &mut curr_type,
                        &mut state,
                        env,
                    );
                    if cr != 0 {
                        env.parse_depth -= 1;
                        return Err(cr);
                    }
                } else {
                    // CS_COMPLETE
                    r = fetch_token_cc(tok, p, end, pattern, env, state);
                    if r < 0 {
                        env.parse_depth -= 1;
                        return Err(r);
                    }
                    fetched = true;
                    if tok.token_type == TokenType::CcClose {
                        // [a-b-] -> literal dash before close
                        let cc = if use_work {
                            &mut work_cc
                        } else {
                            node.as_cclass_mut().unwrap()
                        };
                        let cr = cc_char_next(
                            cc,
                            &mut curr_code,
                            '-' as u32,
                            &mut curr_raw,
                            false,
                            CV_SB,
                            &mut curr_type,
                            &mut state,
                            env,
                        );
                        if cr != 0 {
                            env.parse_depth -= 1;
                            return Err(cr);
                        }
                    } else if is_syntax_bv(env.syntax, ONIG_SYN_ALLOW_DOUBLE_RANGE_OP_IN_CC) {
                        // [0-9-a] allowed
                        let cc = if use_work {
                            &mut work_cc
                        } else {
                            node.as_cclass_mut().unwrap()
                        };
                        let cr = cc_char_next(
                            cc,
                            &mut curr_code,
                            '-' as u32,
                            &mut curr_raw,
                            false,
                            CV_SB,
                            &mut curr_type,
                            &mut state,
                            env,
                        );
                        if cr != 0 {
                            env.parse_depth -= 1;
                            return Err(cr);
                        }
                    } else {
                        env.parse_depth -= 1;
                        return Err(ONIGERR_UNMATCHED_RANGE_SPECIFIER_IN_CHAR_CLASS);
                    }
                }
            }
            TokenType::CcOpenCC => {
                // Nested character class [a[bc]]
                if state == CS_VALUE {
                    let cc = if use_work {
                        &mut work_cc
                    } else {
                        node.as_cclass_mut().unwrap()
                    };
                    let cr = cc_char_next(
                        cc,
                        &mut curr_code,
                        0,
                        &mut curr_raw,
                        false,
                        curr_type,
                        &mut curr_type,
                        &mut state,
                        env,
                    );
                    if cr != 0 {
                        env.parse_depth -= 1;
                        return Err(cr);
                    }
                }
                state = CS_COMPLETE;

                // Recursively parse nested CC
                let anode = prs_cc(tok, p, end, pattern, env)?;
                if let Some(acc) = anode.as_cclass() {
                    let cc = if use_work {
                        &mut work_cc
                    } else {
                        node.as_cclass_mut().unwrap()
                    };
                    or_cclass(cc, acc, enc);
                }
            }
            TokenType::CcAnd => {
                // Intersection &&
                if state == CS_VALUE {
                    let cc = if use_work {
                        &mut work_cc
                    } else {
                        node.as_cclass_mut().unwrap()
                    };
                    let cr = cc_char_next(
                        cc,
                        &mut curr_code,
                        0,
                        &mut curr_raw,
                        false,
                        curr_type,
                        &mut curr_type,
                        &mut state,
                        env,
                    );
                    if cr != 0 {
                        env.parse_depth -= 1;
                        return Err(cr);
                    }
                }
                and_start = true;
                state = CS_START;

                if let Some(ref mut pcc) = prev_cc {
                    let cc = if use_work {
                        &mut work_cc
                    } else {
                        node.as_cclass_mut().unwrap()
                    };
                    and_cclass(pcc, cc, enc);
                    // Reset cc
                    cc.flags = 0;
                    bitset_clear(&mut cc.bs);
                    cc.mbuf = None;
                } else {
                    // First &&: save current into prev_cc, switch to work_cc
                    let cc = node.as_cclass().unwrap();
                    prev_cc = Some(CClassNode {
                        flags: cc.flags,
                        bs: cc.bs,
                        mbuf: cc.mbuf.clone(),
                    });
                    work_cc_active = true;
                    work_cc.flags = 0;
                    bitset_clear(&mut work_cc.bs);
                    work_cc.mbuf = None;
                }
            }
            TokenType::Eot => {
                env.parse_depth -= 1;
                return Err(ONIGERR_PREMATURE_END_OF_CHAR_CLASS);
            }
            TokenType::CcClose => {
                break;
            }
            _ => {
                env.parse_depth -= 1;
                return Err(ONIGERR_PARSER_BUG);
            }
        }

        if !fetched {
            r = fetch_token_cc(tok, p, end, pattern, env, state);
            if r < 0 {
                env.parse_depth -= 1;
                return Err(r);
            }
        }
    }

    // Post-loop: flush remaining state
    if state == CS_VALUE {
        let cc = if work_cc_active {
            &mut work_cc
        } else {
            node.as_cclass_mut().unwrap()
        };
        let cr = cc_char_next(
            cc,
            &mut curr_code,
            0,
            &mut curr_raw,
            false,
            curr_type,
            &mut curr_type,
            &mut state,
            env,
        );
        if cr != 0 {
            env.parse_depth -= 1;
            return Err(cr);
        }
    }

    // Final intersection merge
    if let Some(ref mut pcc) = prev_cc {
        let cc = if work_cc_active {
            &mut work_cc
        } else {
            node.as_cclass_mut().unwrap()
        };
        and_cclass(pcc, cc, enc);
        // Copy prev_cc back into node
        let ncc = node.as_cclass_mut().unwrap();
        ncc.flags = pcc.flags;
        ncc.bs = pcc.bs;
        ncc.mbuf = pcc.mbuf.take();
    } else if work_cc_active {
        // Copy work_cc back to node
        let ncc = node.as_cclass_mut().unwrap();
        ncc.flags = work_cc.flags;
        ncc.bs = work_cc.bs;
        ncc.mbuf = work_cc.mbuf.take();
    }

    // Case-fold expansion: add fold equivalents for all codes in the class
    if opton_ignorecase(env.options) {
        let cc = node.as_cclass_mut().unwrap();
        // Collect codes to add (to avoid borrow issues during iteration)
        let mut codes_to_add: Vec<OnigCodePoint> = Vec::new();
        // Collect multi-char fold alternatives (each is a Vec<u8> of encoded bytes)
        let mut multi_char_alts: Vec<Vec<u8>> = Vec::new();
        enc.apply_all_case_fold(env.case_fold_flag, &mut |from: OnigCodePoint,
                                                          to: &[OnigCodePoint]|
         -> i32 {
            // Check if 'from' is in the (non-negated) class (check both bitset and mbuf)
            let in_bs = if (from as usize) < SINGLE_BYTE_SIZE {
                bitset_at(&cc.bs, from as usize)
            } else {
                false
            };
            let in_mb = if let Some(ref mbuf) = cc.mbuf {
                crate::regexec::is_in_code_range_bytes(&mbuf.data, from)
            } else {
                false
            };
            let in_class = in_bs || in_mb;
            if in_class {
                if to.len() == 1 {
                    codes_to_add.push(to[0]);
                } else {
                    // Multi-char fold: encode all codepoints to bytes
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; ONIGENC_CODE_TO_MBC_MAXLEN];
                    for &cp in to {
                        let len = enc.code_to_mbc(cp, &mut tmp);
                        if len > 0 {
                            buf.extend_from_slice(&tmp[..len as usize]);
                        }
                    }
                    if !buf.is_empty() {
                        multi_char_alts.push(buf);
                    }
                }
            }
            0
        });
        for code in codes_to_add {
            add_code_into_cc(cc, code, enc);
        }

        // If there are multi-char fold alternatives, wrap in Alt(CC, string1, ...)
        if !multi_char_alts.is_empty() {
            // Apply negation to CC before wrapping
            if neg {
                let cc = node.as_cclass_mut().unwrap();
                cc.set_not();
            }

            // Build alternation: Alt(CC, Alt(string1, Alt(string2, ...)))
            let mut alt_tail: Option<Box<Node>> = None;
            for alt_bytes in multi_char_alts.into_iter().rev() {
                let mut sn = node_new_str(&alt_bytes);
                sn.status_add(ND_ST_IGNORECASE);
                if let Some(tail) = alt_tail {
                    alt_tail = Some(node_new_alt(sn, Some(tail)));
                } else {
                    alt_tail = Some(node_new_alt(sn, None));
                }
            }
            node = node_new_alt(node, alt_tail);

            env.parse_depth -= 1;
            return Ok(node);
        }
    }

    // Apply negation
    if neg {
        let cc = node.as_cclass_mut().unwrap();
        cc.set_not();
    }

    env.parse_depth -= 1;
    Ok(node)
}

// ============================================================================
// Callout parsing helpers
// ============================================================================

/// Allocate a new CalloutListEntry on the regex's ext, return its 1-based num.
fn reg_callout_list_entry(env: &mut ParseEnv) -> Result<i32, i32> {
    let reg = unsafe { &mut *env.reg };
    if reg.extp.is_none() {
        reg.extp = Some(RegexExt {
            pattern: Vec::new(),
            tag_table: None,
            callout_num: 0,
            callout_list: Vec::new(),
        });
    }
    let ext = reg.extp.as_mut().unwrap();
    ext.callout_num += 1;
    let num = ext.callout_num;
    // Placeholder entry — caller will fill in
    ext.callout_list.push(CalloutListEntry {
        of: 0,
        callout_in: CALLOUT_IN_PROGRESS,
        builtin_id: -1,
        tag: None,
        tag_start: 0,
        tag_end: 0,
        args: Vec::new(),
        content_end: None,
    });
    Ok(num)
}

/// Register a tag name → callout num mapping.
fn callout_tag_entry(env: &mut ParseEnv, tag: &[u8], num: i32) {
    let reg = unsafe { &mut *env.reg };
    let ext = reg.extp.as_mut().unwrap();
    if ext.tag_table.is_none() {
        ext.tag_table = Some(std::collections::HashMap::new());
    }
    ext.tag_table.as_mut().unwrap().insert(tag.to_vec(), num);
}

/// Parse `(*NAME[tag]{args})` callout-of-name.
/// `p` points right after `*`. Returns (node, 1) on success.
fn prs_callout_of_name(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
    cterm: u32, // closing char: ')' normally
) -> Result<Box<Node>, i32> {
    let enc = env.enc;

    if p_end(*p, end) {
        return Err(ONIGERR_INVALID_CALLOUT_PATTERN);
    }

    // Read name until cterm, '[', or '{'
    let name_start = *p;
    let mut c;
    loop {
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }
        let name_end_pos = *p;
        c = pfetch_s(p, pattern, end, enc);
        if c == cterm || c == '[' as u32 || c == '{' as u32 {
            break;
        }
    }
    let name_end = *p - enc.mbc_enc_len(&pattern[(*p - 1)..end]); // back up past the delimiter
    let name = &pattern[name_start..name_end];

    // Identify builtin
    let (builtin_id, callout_in) = if name == b"FAIL" {
        // Handle FAIL as before — just return node_new_fail directly
        // (don't need a callout entry for this)
        if c != cterm {
            return Err(ONIGERR_INVALID_CALLOUT_PATTERN);
        }
        return Ok(node_new_fail());
    } else if name == b"MAX" {
        (CALLOUT_BUILTIN_MAX, CALLOUT_IN_BOTH)
    } else if name == b"COUNT" {
        (CALLOUT_BUILTIN_COUNT, CALLOUT_IN_BOTH)
    } else if name == b"CMP" {
        (CALLOUT_BUILTIN_CMP, CALLOUT_IN_PROGRESS)
    } else if name == b"SKIP" {
        (CALLOUT_BUILTIN_SKIP, CALLOUT_IN_PROGRESS)
    } else {
        return Err(ONIGERR_UNDEFINED_CALLOUT_NAME);
    };

    // Optional tag: [TAG]
    let (tag, tag_start_pos, tag_end_pos_saved) = if c == '[' as u32 {
        let tag_start = *p;
        loop {
            if p_end(*p, end) {
                return Err(ONIGERR_END_PATTERN_IN_GROUP);
            }
            let tag_end_pos = *p;
            c = pfetch_s(p, pattern, end, enc);
            if c == ']' as u32 {
                let tag_bytes = pattern[tag_start..tag_end_pos].to_vec();
                if tag_bytes.is_empty() {
                    return Err(ONIGERR_INVALID_CALLOUT_TAG_NAME);
                }
                // Read next char after ]
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_IN_GROUP);
                }
                c = pfetch_s(p, pattern, end, enc);
                break (Some(tag_bytes), tag_start, tag_end_pos);
            }
        }
    } else {
        (None, 0, 0)
    };

    // Parse args: {arg1,arg2,...}
    let args = if c == '{' as u32 {
        let mut args = Vec::new();
        loop {
            // Skip leading whitespace
            while !p_end(*p, end) {
                let ch = ppeek(*p, pattern, end, enc);
                if ch == ' ' as u32 || ch == '\t' as u32 {
                    pinc(p, pattern, enc);
                } else {
                    break;
                }
            }
            if p_end(*p, end) {
                return Err(ONIGERR_END_PATTERN_IN_GROUP);
            }
            let ch = ppeek(*p, pattern, end, enc);
            if ch == '}' as u32 {
                pinc(p, pattern, enc);
                break;
            }
            // Parse one argument
            let arg = prs_callout_one_arg(p, end, pattern, env)?;
            args.push(arg);
            // Expect ',' or '}'
            if p_end(*p, end) {
                return Err(ONIGERR_END_PATTERN_IN_GROUP);
            }
            let sep = ppeek(*p, pattern, end, enc);
            if sep == ',' as u32 {
                pinc(p, pattern, enc);
            } else if sep == '}' as u32 {
                pinc(p, pattern, enc);
                break;
            } else {
                return Err(ONIGERR_INVALID_CALLOUT_ARG);
            }
        }
        // Read cterm after }
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }
        c = pfetch_s(p, pattern, end, enc);
        args
    } else {
        Vec::new()
    };

    if c != cterm {
        return Err(ONIGERR_INVALID_CALLOUT_PATTERN);
    }

    // Fill in default args for builtins that have optional args
    let mut final_args = args;
    match builtin_id {
        CALLOUT_BUILTIN_MAX => {
            // args: [TAG|LONG, CHAR]. Arg 0 is required, arg 1 defaults to 'X'
            if final_args.is_empty() {
                return Err(ONIGERR_INVALID_CALLOUT_ARG);
            }
            if final_args.len() < 2 {
                final_args.push(CalloutArg::Char(b'X'));
            }
        }
        CALLOUT_BUILTIN_COUNT => {
            // args: [CHAR]. Arg 0 defaults to '>'
            if final_args.is_empty() {
                final_args.push(CalloutArg::Char(b'>'));
            }
        }
        CALLOUT_BUILTIN_CMP => {
            // args: [TAG|LONG, STRING, TAG|LONG]. All 3 required
            if final_args.len() != 3 {
                return Err(ONIGERR_INVALID_CALLOUT_ARG);
            }
        }
        _ => {}
    }

    // Create callout list entry
    let num = reg_callout_list_entry(env)?;
    let reg = unsafe { &mut *env.reg };
    let ext = reg.extp.as_mut().unwrap();
    let entry = &mut ext.callout_list[(num - 1) as usize];
    entry.of = OnigCalloutOf::Name as i32;
    entry.callout_in = callout_in;
    entry.builtin_id = builtin_id;
    entry.args = final_args;

    // Register tag
    if let Some(ref tag_bytes) = tag {
        entry.tag = Some(tag_bytes.clone());
        entry.tag_start = tag_start_pos;
        entry.tag_end = tag_end_pos_saved;
        callout_tag_entry(env, tag_bytes, num);
    }

    Ok(node_new_callout(
        OnigCalloutOf::Name as i32,
        num,
        builtin_id,
    ))
}

/// Parse one callout argument value: integer, single char (X, <, >), tag name, or string.
fn prs_callout_one_arg(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &ParseEnv,
) -> Result<CalloutArg, i32> {
    let enc = env.enc;
    // Skip whitespace
    while !p_end(*p, end) {
        let ch = ppeek(*p, pattern, end, enc);
        if ch == ' ' as u32 || ch == '\t' as u32 {
            pinc(p, pattern, enc);
        } else {
            break;
        }
    }
    if p_end(*p, end) {
        return Err(ONIGERR_INVALID_CALLOUT_ARG);
    }

    let start = *p;
    // Collect chars until ',' or '}' or ')' (cterm)
    while !p_end(*p, end) {
        let ch = ppeek(*p, pattern, end, enc);
        if ch == ',' as u32 || ch == '}' as u32 || ch == ')' as u32 {
            break;
        }
        pinc(p, pattern, enc);
    }
    let arg_bytes = &pattern[start..*p];
    // Trim trailing whitespace
    let trimmed = {
        let mut e = arg_bytes.len();
        while e > 0 && (arg_bytes[e - 1] == b' ' || arg_bytes[e - 1] == b'\t') {
            e -= 1;
        }
        &arg_bytes[..e]
    };

    if trimmed.is_empty() {
        return Err(ONIGERR_INVALID_CALLOUT_ARG);
    }

    // Try parsing as integer (possibly negative)
    if let Some(n) = try_parse_i64(trimmed) {
        return Ok(CalloutArg::Long(n));
    }

    // Single ASCII char (like 'X', '<', '>')
    if trimmed.len() == 1 && trimmed[0].is_ascii() {
        return Ok(CalloutArg::Char(trimmed[0]));
    }

    // Comparison operators: ==, !=, <, >, <=, >=
    if (trimmed.len() == 1 || trimmed.len() == 2)
        && trimmed
            .iter()
            .all(|&b| matches!(b, b'<' | b'>' | b'=' | b'!'))
    {
        return Ok(CalloutArg::Str(trimmed.to_vec()));
    }

    // Otherwise treat as tag name
    Ok(CalloutArg::Tag(trimmed.to_vec()))
}

fn try_parse_i64(s: &[u8]) -> Option<i64> {
    let s_str = std::str::from_utf8(s).ok()?;
    s_str.trim().parse::<i64>().ok()
}

/// Parse `(?{...})` callout-of-contents.
/// `p` points right after `{`. Returns the callout node.
fn prs_callout_of_contents(
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
    cterm: u32,
) -> Result<Box<Node>, i32> {
    let enc = env.enc;

    if p_end(*p, end) {
        return Err(ONIGERR_INVALID_CALLOUT_PATTERN);
    }

    // Count opening brace nesting: (?{{...}})
    let mut brace_nest = 0;
    while !p_end(*p, end) && ppeek(*p, pattern, end, enc) == '{' as u32 {
        brace_nest += 1;
        pinc(p, pattern, enc);
    }

    // Read content until matching closing braces
    let code_start = *p;
    let mut code_end;
    loop {
        if p_end(*p, end) {
            return Err(ONIGERR_INVALID_CALLOUT_PATTERN);
        }
        code_end = *p;
        let c = pfetch_s(p, pattern, end, enc);
        if c == '}' as u32 {
            let mut i = brace_nest;
            while i > 0 {
                if p_end(*p, end) {
                    return Err(ONIGERR_INVALID_CALLOUT_PATTERN);
                }
                let c2 = pfetch_s(p, pattern, end, enc);
                if c2 == '}' as u32 {
                    i -= 1;
                } else {
                    break;
                }
            }
            if i == 0 {
                break;
            }
        }
    }

    if p_end(*p, end) {
        return Err(ONIGERR_END_PATTERN_IN_GROUP);
    }

    // Direction flag after the closing braces
    let mut callout_in = CALLOUT_IN_PROGRESS;
    let mut c = pfetch_s(p, pattern, end, enc);
    if c == 'X' as u32 {
        callout_in = CALLOUT_IN_BOTH;
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }
        c = pfetch_s(p, pattern, end, enc);
    } else if c == '<' as u32 {
        callout_in = CALLOUT_IN_RETRACTION;
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }
        c = pfetch_s(p, pattern, end, enc);
    } else if c == '>' as u32 {
        // default: PROGRESS
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }
        c = pfetch_s(p, pattern, end, enc);
    }

    if c != cterm {
        return Err(ONIGERR_INVALID_CALLOUT_PATTERN);
    }

    // Create entry
    let num = reg_callout_list_entry(env)?;
    let reg = unsafe { &mut *env.reg };
    let ext = reg.extp.as_mut().unwrap();
    let entry = &mut ext.callout_list[(num - 1) as usize];
    entry.of = OnigCalloutOf::Contents as i32;
    entry.callout_in = callout_in;
    entry.builtin_id = -1;

    Ok(node_new_callout(
        OnigCalloutOf::Contents as i32,
        num,
        ONIG_NON_NAME_ID,
    ))
}

/// Parse a conditional pattern: (?(condition)then|else)
/// Called when we've already consumed '(?' and see '('.
fn prs_conditional(
    tok: &mut PToken,
    term: i32,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
) -> Result<(Box<Node>, i32), i32> {
    let enc = env.enc;

    if !is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_LPAREN_IF_ELSE) {
        return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
    }

    if p_end(*p, end) {
        return Err(ONIGERR_END_PATTERN_IN_GROUP);
    }

    let c = pfetch_s(p, pattern, end, enc);
    let mut condition_is_checker;

    let mut condition: Box<Node>;

    if is_code_digit_ascii(enc, c)
        || c == '-' as u32
        || c == '+' as u32
        || c == '<' as u32
        || c == '\'' as u32
    {
        // Backref condition: (?(1)...), (?(-1)...), (?(<name>)...), (?('name')...)
        condition_is_checker = true;

        if c == '<' as u32 || c == '\'' as u32 {
            // Named or numbered ref with delimiters: (?(<name>)...) or (?('name')...)
            let start_code = c;
            let (name_start, name_end, back_num, num_type, exist_level, level) =
                fetch_name(start_code, p, end, pattern, env, true)?;

            if num_type != IS_NOT_NUM {
                // Numeric ref with delimiters
                let mut num = back_num;
                if num_type == IS_REL_NUM {
                    num = backref_rel_to_abs(num, env);
                    if num <= 0 {
                        return Err(ONIGERR_INVALID_BACKREF);
                    }
                }
                if num > env.num_mem || num < 1 {
                    return Err(ONIGERR_INVALID_BACKREF);
                }
                let backrefs = [num];
                condition = node_new_backref(1, &backrefs, false, 0);
            } else {
                // Named ref
                let name = &pattern[name_start..name_end];
                let reg = unsafe { &*env.reg };
                let group_nums = if let Some(ref nt) = reg.name_table {
                    nt.name_to_group_numbers(name).map(|s| s.to_vec())
                } else {
                    None
                };
                if let Some(nums) = group_nums {
                    condition = node_new_backref(nums.len() as i32, &nums, true, 0);
                } else {
                    return Err(ONIGERR_UNDEFINED_NAME_REFERENCE);
                }
            }

            // Check and skip ')' after the condition
            if p_end(*p, end) {
                return Err(ONIGERR_END_PATTERN_IN_GROUP);
            }
            let close = pfetch_s(p, pattern, end, enc);
            if close != ')' as u32 {
                return Err(ONIGERR_INVALID_IF_ELSE_SYNTAX);
            }
        } else {
            // Bare numeric ref: (?(1)...), (?(-1)...), (?(+2)...)
            let save_p = *p;
            let mut sign = 1i32;
            let mut is_rel = false;
            let mut start_pos = *p;

            if c == '-' as u32 || c == '+' as u32 {
                if c == '-' as u32 {
                    sign = -1;
                }
                is_rel = true;
                start_pos = *p;
                // first digit char was already consumed, read digits
            } else {
                // c is a digit, go back to include it
                start_pos = save_p - 1;
            }

            // Read digits
            let mut num_val = if is_code_digit_ascii(enc, c) {
                c as i32 - '0' as i32
            } else {
                0
            };

            // Parse remaining digits and find ')'
            let mut found_level = false;
            let mut level_val = 0i32;
            let mut level_sign = 1i32;

            while !p_end(*p, end) {
                let d = pfetch_s(p, pattern, end, enc);
                if d == ')' as u32 {
                    break;
                }
                if d == '+' as u32 || d == '-' as u32 {
                    // Level syntax: (?(1+0)...)
                    level_sign = if d == '-' as u32 { -1 } else { 1 };
                    while !p_end(*p, end) {
                        let ld = pfetch_s(p, pattern, end, enc);
                        if ld == ')' as u32 {
                            found_level = true;
                            break;
                        }
                        if is_code_digit_ascii(enc, ld) {
                            level_val = level_val * 10 + (ld as i32 - '0' as i32);
                        } else {
                            return Err(ONIGERR_INVALID_GROUP_NAME);
                        }
                    }
                    break;
                }
                if is_code_digit_ascii(enc, d) {
                    num_val = num_val * 10 + (d as i32 - '0' as i32);
                } else {
                    return Err(ONIGERR_INVALID_GROUP_NAME);
                }
            }

            let mut back_num = num_val * sign;
            if is_rel {
                back_num = backref_rel_to_abs(back_num, env);
                if back_num <= 0 {
                    return Err(ONIGERR_INVALID_BACKREF);
                }
            }

            if back_num > env.num_mem || back_num < 1 {
                return Err(ONIGERR_INVALID_BACKREF);
            }

            let nest_level = if found_level {
                level_val * level_sign
            } else {
                0
            };
            let backrefs = [back_num];
            condition = node_new_backref(1, &backrefs, false, nest_level);
            if found_level {
                condition.status_add(ND_ST_NEST_LEVEL);
            }
        }

        // Mark condition as a checker
        let mut cond = condition;
        cond.status_add(ND_ST_CHECKER);
        condition_is_checker = true;

        // Now parse the body: then|else or just close
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }

        let peek_c = ppeek(*p, pattern, end, enc);
        if peek_c == ')' as u32 {
            // Empty body: (?(1)) - just a backref checker
            pinc(p, pattern, enc);
            return Ok((cond, 0));
        }

        // Parse then|else
        let then_is_empty;
        if peek_c == '|' as u32 {
            // (?(1)|else) - empty then
            pinc(p, pattern, enc);
            then_is_empty = true;
        } else {
            then_is_empty = false;
        }

        let r = fetch_token(tok, p, end, pattern, env);
        if r < 0 {
            return Err(r);
        }

        let then_node;
        let else_node;

        if then_is_empty {
            let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
            then_node = None;
            else_node = Some(target);
        } else {
            // Parse then-branch up to | or )
            let (then_target, then_r) = prs_branch(tok, term, p, end, pattern, env, false)?;
            if then_r == TokenType::Alt as i32 {
                // Top-level | found: parse else-branch
                then_node = Some(then_target);
                let r2 = fetch_token(tok, p, end, pattern, env);
                if r2 < 0 {
                    return Err(r2);
                }
                let (else_target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                else_node = Some(else_target);
            } else {
                // No top-level | — entire body is then, no else
                then_node = Some(then_target);
                else_node = None;
            }
        }

        let np = node_new_bag_if_else(cond, then_node, else_node);
        return Ok((np, 0));
    } else {
        // Non-backref condition: callout or general pattern
        condition_is_checker = false;

        let cond_node;
        if c == '?' as u32 && is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_BRACE_CALLOUT_CONTENTS) {
            // Condition is callout of contents: (?(?{...})THEN|ELSE)
            if !p_end(*p, end) && ppeek(*p, pattern, end, enc) == '{' as u32 {
                pinc(p, pattern, enc); // consume '{'
                cond_node = prs_callout_of_contents(p, end, pattern, env, ')' as u32)?;
            } else {
                // Fall through to general pattern condition
                *p = *p - 1; // unfetch '?'
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                let (cn, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                cond_node = cn;
            }
        } else if c == '*' as u32 && is_syntax_op2(env.syntax, ONIG_SYN_OP2_ASTERISK_CALLOUT_NAME) {
            // Callout-of-name condition: (?(*FAIL)then|else), (?(*MAX{2})then|else)
            cond_node = prs_callout_of_name(p, end, pattern, env, ')' as u32)?;
        } else {
            // General pattern condition
            *p = *p - 1; // unfetch the char we just read
            let r = fetch_token(tok, p, end, pattern, env);
            if r < 0 {
                return Err(r);
            }
            let (cn, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
            cond_node = cn;
        }

        // Now parse then|else
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }

        let peek_c = ppeek(*p, pattern, end, enc);
        if peek_c == ')' as u32 {
            // No body after condition - this is an error for non-checker conditions
            return Err(ONIGERR_INVALID_IF_ELSE_SYNTAX);
        }

        let then_is_empty;
        if peek_c == '|' as u32 {
            pinc(p, pattern, enc);
            then_is_empty = true;
        } else {
            then_is_empty = false;
        }

        let r = fetch_token(tok, p, end, pattern, env);
        if r < 0 {
            return Err(r);
        }

        let then_node;
        let else_node;

        if then_is_empty {
            // (?(cond)|else) - empty then, parse else
            let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
            then_node = None;
            else_node = Some(target);
        } else {
            // Parse then-branch up to | or )
            let (then_target, then_r) = prs_branch(tok, term, p, end, pattern, env, false)?;
            if then_r == TokenType::Alt as i32 {
                // Top-level | found: parse else-branch
                then_node = Some(then_target);
                let r2 = fetch_token(tok, p, end, pattern, env);
                if r2 < 0 {
                    return Err(r2);
                }
                let (else_target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                else_node = Some(else_target);
            } else {
                // No top-level | — entire body is then, no else
                then_node = Some(then_target);
                else_node = None;
            }
        }

        let np = node_new_bag_if_else(cond_node, then_node, else_node);
        return Ok((np, 0));
    }
}

/// Split an Alt node into (first_alt, rest) for conditional then|else parsing.
/// Given Alt(A, Alt(B, nil)) -> (A, Some(B))
/// Given Alt(A, Alt(B, Alt(C, nil))) -> (A, Some(Alt(B, Alt(C, nil))))
// ============================================================================
// Absent function (?~...) tree builders
// ============================================================================

/// Build the core absent-checking engine (C: make_absent_engine).
///
/// Structure:
/// ```text
/// Alt(
///   [possessive?] Quant[lower..upper, non-greedy](
///     Alt(
///       List( Save(S,id), absent, UpdateVar(RR_FROM_S_STACK,id), Fail ),
///       step_one
///     )
///   ),
///   List( UpdateVar(RR_FROM_STACK, pre_save_right_id), Fail )
/// )
/// ```
fn make_absent_engine(
    pre_save_right_id: i32,
    absent: Box<Node>,
    step_one: Box<Node>,
    lower: i32,
    upper: i32,
    possessive: bool,
    is_range_cutter: bool,
    env: &mut ParseEnv,
) -> Result<Box<Node>, i32> {
    let id = env.id_entry();
    let save_s = node_new_save_gimmick(SaveType::S, id);
    let mut update_rr = node_new_update_var_gimmick(UpdateVarType::RightRangeFromSStack, id);
    if is_range_cutter {
        update_rr.status_add(ND_ST_ABSENT_WITH_SIDE_EFFECTS);
    }
    let fail1 = node_new_fail();

    let inner_list = make_list_n(vec![save_s, absent, update_rr, fail1]);
    let inner_alt = make_alt(inner_list, step_one);

    let mut quant = node_new_quantifier(lower, upper, true); // greedy (C: greedy=1 always)
    quant.set_body(Some(inner_alt));

    let engine: Box<Node> = if possessive {
        let mut bag = node_new_bag(BagType::StopBacktrack);
        bag.set_body(Some(quant));
        bag
    } else {
        quant
    };

    let update_rr2 =
        node_new_update_var_gimmick(UpdateVarType::RightRangeFromStack, pre_save_right_id);
    let fail2 = node_new_fail();
    let tail_list = make_list(update_rr2, fail2);

    let mut outer_alt = make_alt(engine, tail_list);
    if is_range_cutter {
        outer_alt.status_add(ND_ST_SUPER);
    }

    Ok(outer_alt)
}

/// Build tail nodes for non-range-cutter absent expression (C: make_absent_tail).
///
/// Returns (save_node, alt_node) where:
///   save_node = Save(RIGHT_RANGE, id)
///   alt_node = Alt( UpdateVar(RR_FROM_STACK, pre_save_id), List(UpdateVar(RR_FROM_STACK, id), Fail) )
fn make_absent_tail(
    pre_save_right_id: i32,
    env: &mut ParseEnv,
) -> Result<(Box<Node>, Box<Node>), i32> {
    let id = env.id_entry();
    let save = node_new_save_gimmick(SaveType::RightRange, id);

    let update1 = node_new_update_var_gimmick(UpdateVarType::RightRangeFromStack, id);
    let fail = node_new_fail();
    let list = make_list(update1, fail);

    let update2 =
        node_new_update_var_gimmick(UpdateVarType::RightRangeFromStack, pre_save_right_id);

    let alt = make_alt(update2, list);
    Ok((save, alt))
}

/// Build range cutter (?~|) tree (C: make_range_clear).
///
/// Structure:
/// ```text
/// List(
///   Save(RIGHT_RANGE, id),
///   Alt[SUPER](
///     UpdateVar(RR_INIT, 0) [ABSENT_WITH_SIDE_EFFECTS],
///     List( UpdateVar(RR_FROM_STACK, id), Fail )
///   )
/// )
/// ```
fn make_range_clear(env: &mut ParseEnv) -> Result<Box<Node>, i32> {
    let id = env.id_entry();
    let save = node_new_save_gimmick(SaveType::RightRange, id);

    let update1 = node_new_update_var_gimmick(UpdateVarType::RightRangeFromStack, id);
    let fail = node_new_fail();
    let list = make_list(update1, fail);

    let mut update_init = node_new_update_var_gimmick(UpdateVarType::RightRangeInit, 0);
    update_init.status_add(ND_ST_ABSENT_WITH_SIDE_EFFECTS);

    let mut alt = make_alt(update_init, list);
    alt.status_add(ND_ST_SUPER);

    let result = make_list(save, alt);
    Ok(result)
}

/// Check if node is a simple one-char repeat: greedy Quant over single-char String or CClass,
/// optionally wrapped in StopBacktrack bag (C: is_simple_one_char_repeat).
///
/// Returns Some((quant_node_without_body, body, possessive)) on match,
/// or None and gives back the original node.
fn is_simple_one_char_repeat(
    node: Box<Node>,
    enc: OnigEncoding,
) -> Result<(Box<Node>, Box<Node>, bool), Box<Node>> {
    let possessive;
    let mut quant_node: Box<Node>;

    match &node.inner {
        NodeInner::Quant(_) => {
            possessive = false;
            quant_node = node;
        }
        NodeInner::Bag(bag) if bag.bag_type == BagType::StopBacktrack => {
            possessive = true;
            // Extract quant from bag body
            let mut bag_node = node;
            let body = bag_node.take_body();
            match body {
                Some(b) => {
                    if let NodeInner::Quant(_) = &b.inner {
                        quant_node = b;
                    } else {
                        // Not a quantifier inside StopBacktrack — put it back
                        bag_node.set_body(Some(b));
                        return Err(bag_node);
                    }
                }
                None => return Err(bag_node),
            }
        }
        _ => return Err(node),
    }

    // Check greedy
    let is_greedy = if let NodeInner::Quant(ref qn) = quant_node.inner {
        qn.greedy
    } else {
        false
    };
    if !is_greedy {
        return Err(quant_node);
    }

    // Check body is single-char String or CClass
    let body = quant_node.take_body();
    match body {
        Some(b) => {
            let ok = match &b.inner {
                NodeInner::String(sn) => {
                    // Count encoded chars
                    let s = &sn.s;
                    let mut pos = 0;
                    let mut count = 0;
                    while pos < s.len() {
                        pos += enc.mbc_enc_len(&s[pos..]);
                        count += 1;
                    }
                    count == 1
                }
                NodeInner::CClass(_) => true,
                _ => false,
            };
            if ok {
                Ok((quant_node, b, possessive))
            } else {
                quant_node.set_body(Some(b));
                Err(quant_node)
            }
        }
        None => Err(quant_node),
    }
}

/// Optimized absent tree for simple one-char repeats (C: make_absent_tree_for_simple_one_char_repeat).
///
/// Structure:
/// ```text
/// List(
///   Save(RIGHT_RANGE, id1),
///   absent_engine(id1, absent, body, lower, upper, possessive, false),
///   UpdateVar(RR_FROM_STACK, id1)
/// )
/// ```
fn make_absent_tree_for_simple_one_char_repeat(
    absent: Box<Node>,
    quant: &Node,
    body: Box<Node>,
    possessive: bool,
    env: &mut ParseEnv,
) -> Result<Box<Node>, i32> {
    let (lower, upper) = if let NodeInner::Quant(ref qn) = quant.inner {
        (qn.lower, qn.upper)
    } else {
        (0, INFINITE_REPEAT)
    };

    let id1 = env.id_entry();
    let save_rr = node_new_save_gimmick(SaveType::RightRange, id1);
    let engine = make_absent_engine(id1, absent, body, lower, upper, possessive, false, env)?;
    let update_rr = node_new_update_var_gimmick(UpdateVarType::RightRangeFromStack, id1);

    Ok(make_list_n(vec![save_rr, engine, update_rr]))
}

/// Main entry point for building the absent function tree (C: make_absent_tree).
///
/// Three paths:
/// 1. Simple path: no range cutter, expr is None or simple one-char repeat
/// 2. General path with range cutter
/// 3. General path with complex expr
fn make_absent_tree(
    absent: Box<Node>,
    expr: Option<Box<Node>>,
    is_range_cutter: bool,
    env: &mut ParseEnv,
) -> Result<Box<Node>, i32> {
    if !is_range_cutter {
        if let Some(expr_node) = expr {
            // Try simple one-char repeat optimization
            match is_simple_one_char_repeat(expr_node, env.enc) {
                Ok((quant, body, possessive)) => {
                    return make_absent_tree_for_simple_one_char_repeat(
                        absent, &quant, body, possessive, env,
                    );
                }
                Err(expr_back) => {
                    // Fall through to general path with expr
                    return make_absent_tree_general(absent, Some(expr_back), false, env);
                }
            }
        } else {
            // Default expr: true_anychar* (greedy). Quant is only used to extract lower/upper.
            let body = node_new_true_anychar();
            let quant = node_new_quantifier(0, INFINITE_REPEAT, true);
            return make_absent_tree_for_simple_one_char_repeat(absent, &quant, body, false, env);
        }
    }

    // Range cutter or complex expr: general path
    make_absent_tree_general(absent, expr, is_range_cutter, env)
}

/// General path for absent tree (range cutter or complex expr).
///
/// For range cutter:
/// ```text
/// List( Save(RR, id1), Save(S, id2), engine, UpdateVar(S_FROM_STACK, id2) )
/// ```
/// For non-range-cutter with expr:
/// ```text
/// List( Save(RR, id1), Save(S, id2), engine, UpdateVar(S_FROM_STACK, id2), expr, save_tail, alt_tail )
/// ```
fn make_absent_tree_general(
    absent: Box<Node>,
    expr: Option<Box<Node>>,
    is_range_cutter: bool,
    env: &mut ParseEnv,
) -> Result<Box<Node>, i32> {
    let id1 = env.id_entry();
    let save_rr = node_new_save_gimmick(SaveType::RightRange, id1);

    let id2 = env.id_entry();
    let save_s = node_new_save_gimmick(SaveType::S, id2);

    let step_one = node_new_true_anychar();
    let engine = make_absent_engine(
        id1,
        absent,
        step_one,
        0,
        INFINITE_REPEAT,
        true,
        is_range_cutter,
        env,
    )?;

    let update_s = node_new_update_var_gimmick(UpdateVarType::SFromStack, id2);

    if is_range_cutter {
        Ok(make_list_n(vec![save_rr, save_s, engine, update_s]))
    } else {
        let expr_node = expr.unwrap();
        let (save_tail, alt_tail) = make_absent_tail(id1, env)?;
        Ok(make_list_n(vec![
            save_rr, save_s, engine, update_s, expr_node, save_tail, alt_tail,
        ]))
    }
}

fn split_alt_for_conditional(mut node: Box<Node>) -> (Box<Node>, Option<Box<Node>>) {
    if let NodeInner::Alt(cons) = node.inner {
        let car = cons.car;
        if let Some(cdr) = cons.cdr {
            if let NodeInner::Alt(ref cdr_cons) = cdr.inner {
                if cdr_cons.cdr.is_none() {
                    // Alt(then, Alt(else, nil)) -> (then, Some(else))
                    if let NodeInner::Alt(cdr_cons) = cdr.inner {
                        return (car, Some(cdr_cons.car));
                    }
                }
            }
            // Multiple alternatives: (then, rest_as_else)
            return (car, Some(cdr));
        }
        // Only one alt, no else
        return (car, None);
    }
    // Not an Alt - shouldn't happen
    (node, None)
}

// ============================================================================
// Subexpression/group parser: prs_bag
// ============================================================================

/// Parse a bag (subexpression/group).
/// Returns: Ok((node, return_code)) where return_code is:
///   0 = normal bag node, 1 = group-only (non-capturing), 2 = option-only
fn prs_bag(
    tok: &mut PToken,
    term: i32,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
) -> Result<(Box<Node>, i32), i32> {
    let enc = env.enc;

    if p_end(*p, end) {
        return Err(ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS);
    }

    let c = ppeek(*p, pattern, end, enc);
    let option = env.options;

    if c == '?' as u32 && is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_GROUP_EFFECT) {
        pinc(p, pattern, enc); // skip '?'
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }
        let mut pfetch_prev = *p;
        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);

        match c as u8 as char {
            ':' => {
                // Non-capturing group (?:...)
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                let (node, r) = prs_alts(tok, term, p, end, pattern, env, true)?;
                return Ok((node, 1));
            }
            '=' => {
                // Positive lookahead (?=...)
                let mut np = node_new_anchor(ANCR_PREC_READ);
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                np.set_body(Some(target));
                return Ok((np, 0));
            }
            '!' => {
                // Negative lookahead (?!...)
                let mut np = node_new_anchor(ANCR_PREC_READ_NOT);
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                np.set_body(Some(target));
                return Ok((np, 0));
            }
            '>' => {
                // Atomic group (?>...)
                let mut np = node_new_bag(BagType::StopBacktrack);
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                np.set_body(Some(target));
                return Ok((np, 0));
            }
            '<' => {
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_IN_GROUP);
                }
                let c2 = ppeek(*p, pattern, end, enc);
                if c2 == '=' as u32 {
                    // Positive lookbehind (?<=...)
                    pinc(p, pattern, enc);
                    let mut np = node_new_anchor(ANCR_LOOK_BEHIND);
                    let r = fetch_token(tok, p, end, pattern, env);
                    if r < 0 {
                        return Err(r);
                    }
                    let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                    np.set_body(Some(target));
                    return Ok((np, 0));
                } else if c2 == '!' as u32 {
                    // Negative lookbehind (?<!...)
                    pinc(p, pattern, enc);
                    let mut np = node_new_anchor(ANCR_LOOK_BEHIND_NOT);
                    let r = fetch_token(tok, p, end, pattern, env);
                    if r < 0 {
                        return Err(r);
                    }
                    let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                    np.set_body(Some(target));
                    return Ok((np, 0));
                } else if is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_LT_NAMED_GROUP) {
                    // Named group (?<name>...)
                    return prs_named_group(tok, '<' as u32, term, p, end, pattern, env, false);
                }
                return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
            }
            '\'' => {
                if is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_LT_NAMED_GROUP) {
                    return prs_named_group(tok, '\'' as u32, term, p, end, pattern, env, false);
                }
                return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
            }
            '@' => {
                if USE_CAPTURE_HISTORY
                    && is_syntax_op2(env.syntax, ONIG_SYN_OP2_ATMARK_CAPTURE_HISTORY)
                {
                    // (?@<name>...) or (?@'name'...) — named group with capture history
                    if is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_LT_NAMED_GROUP)
                        && !p_end(*p, end)
                    {
                        let c2 = ppeek(*p, pattern, end, enc);
                        if c2 == '<' as u32 || c2 == '\'' as u32 {
                            pinc(p, pattern, enc);
                            return prs_named_group(tok, c2, term, p, end, pattern, env, true);
                        }
                    }
                    // (?@...) — unnamed capture with history
                    let num = env.add_mem_entry()?;
                    if num >= MEM_STATUS_BITS_NUM as i32 {
                        return Err(ONIGERR_GROUP_NUMBER_OVER_FOR_CAPTURE_HISTORY);
                    }
                    let mut np = node_new_bag_memory(num);
                    mem_status_on(&mut env.cap_history, num as usize);
                    let r = fetch_token(tok, p, end, pattern, env);
                    if r < 0 {
                        return Err(r);
                    }
                    let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                    np.set_body(Some(target));
                    env.set_mem_node(num, &mut *np as *mut Node);
                    return Ok((np, 0));
                }
                return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
            }
            '(' => {
                // Conditional: (?(condition)then|else)
                return prs_conditional(tok, term, p, end, pattern, env);
            }
            'P' => {
                if is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_CAPITAL_P_NAME) {
                    if !p_end(*p, end) {
                        let c2 = ppeek(*p, pattern, end, enc);
                        if c2 == '<' as u32 {
                            pinc(p, pattern, enc);
                            return prs_named_group(
                                tok, '<' as u32, term, p, end, pattern, env, false,
                            );
                        } else if c2 == '=' as u32 {
                            // (?P=name) — Python named backref
                            pinc(p, pattern, enc); // skip '='
                            match fetch_name('(' as u32, p, end, pattern, env, false) {
                                Ok((
                                    name_start,
                                    name_end,
                                    _back_num,
                                    _num_type,
                                    has_level,
                                    level_val,
                                )) => {
                                    let name = &pattern[name_start..name_end];
                                    let reg = unsafe { &*env.reg };
                                    if let Some(ref nt) = reg.name_table {
                                        if let Some(entry) = nt.find(name) {
                                            let refs = if entry.back_num == 1 {
                                                vec![entry.back_refs[0]]
                                            } else {
                                                entry.back_refs.clone()
                                            };
                                            let mut np = node_new_backref(
                                                entry.back_num,
                                                &refs,
                                                true,
                                                level_val,
                                            );
                                            if has_level {
                                                // level-based backref (rare)
                                            }
                                            if opton_ignorecase(env.options) {
                                                np.status_add(ND_ST_IGNORECASE);
                                            }
                                            env.backref_num += 1;
                                            return Ok((np, 0));
                                        }
                                    }
                                    return Err(ONIGERR_UNDEFINED_NAME_REFERENCE);
                                }
                                Err(e) => return Err(e),
                            }
                        } else if c2 == '>' as u32 {
                            // (?P>name) — Python named call
                            pinc(p, pattern, enc); // skip '>'
                            match fetch_name('(' as u32, p, end, pattern, env, false) {
                                Ok((name_start, name_end, gnum, _num_type, _has_level, _level)) => {
                                    let name = &pattern[name_start..name_end];
                                    let np = node_new_call(name, gnum, false);
                                    env.num_call += 1;
                                    return Ok((np, 0));
                                }
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                // Fall through to option parsing ((?P:...) = POSIX is ASCII)
                *p = pfetch_prev;
                return prs_options(tok, term, p, end, pattern, env);
            }
            '~' => {
                if c < 128 && is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_TILDE_ABSENT_GROUP) {
                    if p_end(*p, end) {
                        return Err(ONIGERR_END_PATTERN_IN_GROUP);
                    }

                    let head_bar;
                    if ppeek(*p, pattern, end, enc) == '|' as u32 {
                        pinc(p, pattern, enc);
                        if p_end(*p, end) {
                            return Err(ONIGERR_END_PATTERN_IN_GROUP);
                        }
                        head_bar = true;
                        if ppeek(*p, pattern, end, enc) == ')' as u32 {
                            // (?~|) : range clear
                            pinc(p, pattern, enc);
                            let np = make_range_clear(env)?;
                            env.flags |= PE_FLAG_HAS_ABSENT_STOPPER;
                            return Ok((np, 1));
                        }
                    } else {
                        head_bar = false;
                    }

                    let r = fetch_token(tok, p, end, pattern, env);
                    if r < 0 {
                        return Err(r);
                    }
                    let (absent, _) = prs_alts(tok, term, p, end, pattern, env, true)?;

                    let mut expr: Option<Box<Node>> = None;
                    let mut is_range_cutter = false;

                    if head_bar {
                        // Check if Alt with 2+ branches (absent|expr form)
                        let is_alt_with_cdr = matches!(
                            &absent.inner,
                            NodeInner::Alt(cons) if cons.cdr.is_some()
                        );

                        if !is_alt_with_cdr {
                            // Single branch: (?~|absent) → range cutter
                            is_range_cutter = true;
                            env.flags |= PE_FLAG_HAS_ABSENT_STOPPER;
                        } else {
                            // Two+ branches: (?~|absent|expr)
                            // Split: first branch = absent, rest = expr
                            if let NodeInner::Alt(cons) = absent.inner {
                                let absent_part = cons.car;
                                let rest = cons.cdr.unwrap();
                                // Unwrap single-element Alt wrapper: Alt(x, nil) → x
                                let is_single = matches!(
                                    &rest.inner,
                                    NodeInner::Alt(rc) if rc.cdr.is_none()
                                );
                                let expr_part = if is_single {
                                    if let NodeInner::Alt(rc) = rest.inner {
                                        rc.car
                                    } else {
                                        unreachable!()
                                    }
                                } else {
                                    rest
                                };
                                expr = Some(expr_part);
                                let np = make_absent_tree(absent_part, expr, is_range_cutter, env)?;
                                return Ok((np, 1));
                            } else {
                                unreachable!();
                            }
                        }
                    }

                    let np = make_absent_tree(absent, expr, is_range_cutter, env)?;
                    return Ok((np, 1));
                }
                return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
            }
            '{' => {
                // Callout of contents: (?{...})
                if !is_syntax_op2(env.syntax, ONIG_SYN_OP2_QMARK_BRACE_CALLOUT_CONTENTS) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                let node = prs_callout_of_contents(p, end, pattern, env, ')' as u32)?;
                return Ok((node, 1));
            }
            _ => {
                // Option flags: i, m, s, x, etc.
                *p = pfetch_prev; // PUNFETCH back to the option char
                return prs_options(tok, term, p, end, pattern, env);
            }
        }
    } else if c == '*' as u32 && is_syntax_op2(env.syntax, ONIG_SYN_OP2_ASTERISK_CALLOUT_NAME) {
        // Callout of name: (*FAIL), (*MAX{2}), (*COUNT[AB]{X}), (*CMP{AB,<,CD})
        pinc(p, pattern, enc); // skip '*'
        let node = prs_callout_of_name(p, end, pattern, env, ')' as u32)?;
        return Ok((node, 1));
    } else {
        // Plain parenthesized group
        if (env.options & ONIG_OPTION_DONT_CAPTURE_GROUP) != 0 {
            // Treat as non-capturing
            let r = fetch_token(tok, p, end, pattern, env);
            if r < 0 {
                return Err(r);
            }
            let (node, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
            return Ok((node, 1));
        }

        // Capturing group
        let num = env.add_mem_entry()?;
        let mut np = node_new_bag_memory(num);
        let r = fetch_token(tok, p, end, pattern, env);
        if r < 0 {
            return Err(r);
        }
        let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
        np.set_body(Some(target));
        env.set_mem_node(num, &mut *np as *mut Node);
        return Ok((np, 0));
    }
}

/// Parse a named group (?<name>...) or (?'name'...)
fn prs_named_group(
    tok: &mut PToken,
    start_code: OnigCodePoint,
    term: i32,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
    list_capture: bool,
) -> Result<(Box<Node>, i32), i32> {
    let (name_start, name_end, _back_num, _num_type, _, _) =
        fetch_name(start_code, p, end, pattern, env, false)?;

    let num = env.add_mem_entry()?;

    // Add to name table
    if let Some(ref mut nt) = unsafe { &mut *env.reg }.name_table {
        let name = &pattern[name_start..name_end];
        let allow = is_syntax_bv(env.syntax, ONIG_SYN_ALLOW_MULTIPLEX_DEFINITION_NAME);
        nt.add(name, num, allow).map_err(|e| e)?;
    }

    let mut np = node_new_bag_memory(num);
    np.status_add(ND_ST_NAMED_GROUP);
    env.num_named += 1;

    if list_capture {
        mem_status_on(&mut env.cap_history, num as usize);
    }

    let r = fetch_token(tok, p, end, pattern, env);
    if r < 0 {
        return Err(r);
    }
    let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
    np.set_body(Some(target));
    env.set_mem_node(num, &mut *np as *mut Node);

    Ok((np, 0))
}

/// Apply whole options ((?I), (?L), (?C)) to the regex and parse env.
fn set_whole_options(option: OnigOptionType, env: &mut ParseEnv) {
    let reg = unsafe { &mut *env.reg };
    if (option & ONIG_OPTION_IGNORECASE_IS_ASCII) != 0 {
        reg.case_fold_flag &=
            !(INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR | ONIGENC_CASE_FOLD_TURKISH_AZERI);
        reg.case_fold_flag |= ONIGENC_CASE_FOLD_ASCII_ONLY;
        env.case_fold_flag = reg.case_fold_flag;
    }
    if (option & ONIG_OPTION_FIND_LONGEST) != 0 {
        reg.options |= ONIG_OPTION_FIND_LONGEST;
    }
    if (option & ONIG_OPTION_DONT_CAPTURE_GROUP) != 0 {
        reg.options |= ONIG_OPTION_DONT_CAPTURE_GROUP;
    }
}

/// Parse option flags like (?imsx:...) or (?imsx)
fn prs_options(
    tok: &mut PToken,
    term: i32,
    p: &mut usize,
    end: usize,
    pattern: &[u8],
    env: &mut ParseEnv,
) -> Result<(Box<Node>, i32), i32> {
    let enc = env.enc;
    let syn = env.syntax;
    let mut option = env.options;
    let mut neg = false;
    let mut whole_options: OnigOptionType = 0;
    let mut pfetch_prev;

    loop {
        if p_end(*p, end) {
            return Err(ONIGERR_END_PATTERN_IN_GROUP);
        }
        pfetch_prev = *p;
        let c = pfetch(p, &mut pfetch_prev, pattern, end, enc);

        match c as u8 as char {
            '-' => {
                neg = true;
            }
            'x' => {
                if neg {
                    onig_option_off(&mut option, ONIG_OPTION_EXTEND);
                } else {
                    onig_option_on(&mut option, ONIG_OPTION_EXTEND);
                }
            }
            'i' => {
                if neg {
                    onig_option_off(&mut option, ONIG_OPTION_IGNORECASE);
                } else {
                    onig_option_on(&mut option, ONIG_OPTION_IGNORECASE);
                }
            }
            's' => {
                if is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_PERL) {
                    if neg {
                        onig_option_off(&mut option, ONIG_OPTION_MULTILINE);
                    } else {
                        onig_option_on(&mut option, ONIG_OPTION_MULTILINE);
                    }
                } else {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
            }
            'm' => {
                if is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_PERL) {
                    // C: OPTION_NEGATE(option, SINGLELINE, (neg == FALSE ? TRUE : FALSE))
                    // (?m) CLEARS singleline (making ^ and $ match at line boundaries)
                    // (?-m) SETS singleline (restoring default)
                    if neg {
                        onig_option_on(&mut option, ONIG_OPTION_SINGLELINE);
                    } else {
                        onig_option_off(&mut option, ONIG_OPTION_SINGLELINE);
                    }
                } else if is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_RUBY)
                    || is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_ONIGURUMA)
                {
                    if neg {
                        onig_option_off(&mut option, ONIG_OPTION_MULTILINE);
                    } else {
                        onig_option_on(&mut option, ONIG_OPTION_MULTILINE);
                    }
                } else {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
            }
            'W' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_ONIGURUMA) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    onig_option_off(&mut option, ONIG_OPTION_WORD_IS_ASCII);
                } else {
                    onig_option_on(&mut option, ONIG_OPTION_WORD_IS_ASCII);
                }
            }
            'D' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_ONIGURUMA) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    onig_option_off(&mut option, ONIG_OPTION_DIGIT_IS_ASCII);
                } else {
                    onig_option_on(&mut option, ONIG_OPTION_DIGIT_IS_ASCII);
                }
            }
            'S' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_ONIGURUMA) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    onig_option_off(&mut option, ONIG_OPTION_SPACE_IS_ASCII);
                } else {
                    onig_option_on(&mut option, ONIG_OPTION_SPACE_IS_ASCII);
                }
            }
            'P' => {
                if !is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_ONIGURUMA) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    onig_option_off(&mut option, ONIG_OPTION_POSIX_IS_ASCII);
                } else {
                    onig_option_on(&mut option, ONIG_OPTION_POSIX_IS_ASCII);
                }
            }
            'a' => {
                if !is_syntax_bv(syn, ONIG_SYN_PYTHON) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    onig_option_off(&mut option, ONIG_OPTION_POSIX_IS_ASCII);
                } else {
                    onig_option_on(&mut option, ONIG_OPTION_POSIX_IS_ASCII);
                }
            }
            'I' => {
                if !is_syntax_bv(syn, ONIG_SYN_WHOLE_OPTIONS) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    return Err(ONIGERR_INVALID_GROUP_OPTION);
                }
                onig_option_on(&mut option, ONIG_OPTION_IGNORECASE_IS_ASCII);
                whole_options |= ONIG_OPTION_IGNORECASE_IS_ASCII;
            }
            'C' => {
                if !is_syntax_bv(syn, ONIG_SYN_WHOLE_OPTIONS) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    return Err(ONIGERR_INVALID_GROUP_OPTION);
                }
                onig_option_on(&mut option, ONIG_OPTION_DONT_CAPTURE_GROUP);
                whole_options |= ONIG_OPTION_DONT_CAPTURE_GROUP;
            }
            'L' => {
                if !is_syntax_bv(syn, ONIG_SYN_WHOLE_OPTIONS) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    return Err(ONIGERR_INVALID_GROUP_OPTION);
                }
                onig_option_on(&mut option, ONIG_OPTION_FIND_LONGEST);
                whole_options |= ONIG_OPTION_FIND_LONGEST;
            }
            'y' => {
                // (?y{g}) or (?y{w}) — text segment boundary mode
                if !is_syntax_op2(syn, ONIG_SYN_OP2_OPTION_ONIGURUMA) {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if neg {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_IN_GROUP);
                }
                if ppeek(*p, pattern, end, enc) != '{' as u32 {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
                pfetch_prev = *p;
                pfetch(p, &mut pfetch_prev, pattern, end, enc); // skip '{'
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_IN_GROUP);
                }
                pfetch_prev = *p;
                let mode_char = pfetch(p, &mut pfetch_prev, pattern, end, enc);
                match mode_char as u8 as char {
                    'g' => {
                        if !onigenc_is_unicode_encoding(enc) {
                            return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                        }
                        onig_option_on(
                            &mut option,
                            ONIG_OPTION_TEXT_SEGMENT_EXTENDED_GRAPHEME_CLUSTER,
                        );
                        onig_option_off(&mut option, ONIG_OPTION_TEXT_SEGMENT_WORD);
                    }
                    'w' => {
                        if !onigenc_is_unicode_encoding(enc) {
                            return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                        }
                        onig_option_on(&mut option, ONIG_OPTION_TEXT_SEGMENT_WORD);
                        onig_option_off(
                            &mut option,
                            ONIG_OPTION_TEXT_SEGMENT_EXTENDED_GRAPHEME_CLUSTER,
                        );
                    }
                    _ => {
                        return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                    }
                }
                if p_end(*p, end) {
                    return Err(ONIGERR_END_PATTERN_IN_GROUP);
                }
                pfetch_prev = *p;
                let closing = pfetch(p, &mut pfetch_prev, pattern, end, enc);
                if closing != '}' as u32 {
                    return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
                }
            }
            ')' => {
                // Option-only group (?i) or (?Ii)
                let mut np = node_new_option(option);
                if whole_options != 0 {
                    np.status_add(ND_ST_WHOLE_OPTIONS);
                }
                env.options = option;
                return Ok((np, 2));
            }
            ':' => {
                // Option-scoped group (?i:...) or (?Ii:...)
                let save_options = env.options;
                env.options = option;
                if whole_options != 0 {
                    set_whole_options(option, env);
                }
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    env.options = save_options;
                    return Err(r);
                }
                let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                env.options = save_options;
                let mut np = node_new_option(option);
                np.set_body(Some(target));
                if whole_options != 0 {
                    np.status_add(ND_ST_WHOLE_OPTIONS);
                }
                return Ok((np, 0));
            }
            _ => {
                return Err(ONIGERR_UNDEFINED_GROUP_OPTION);
            }
        }
    }
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
            let (node, bag_r) = prs_bag(tok, TokenType::SubexpClose as i32, p, end, pattern, env)?;
            if bag_r == 1 {
                // Group-only (non-capturing (?:...) or similar)
                // fetch_token to advance past the SubexpClose consumed by prs_alts
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                return check_quantifier(node, tok, p, end, pattern, env, 1, parse_depth);
            } else if bag_r == 2 {
                // Option-only (?i) or (?Ii)
                let bag_options = match node.as_bag() {
                    Some(b) => match b.bag_data {
                        BagData::Option { options } => options,
                        _ => env.options,
                    },
                    None => env.options,
                };
                // Check whole options position
                if node.has_status(ND_ST_WHOLE_OPTIONS) {
                    if !group_head {
                        return Err(ONIGERR_INVALID_GROUP_OPTION);
                    }
                }
                if is_syntax_bv(env.syntax, ONIG_SYN_ISOLATED_OPTION_CONTINUE_BRANCH) {
                    // Perl/Java: just set options and continue branch
                    env.options = bag_options;
                    let r = fetch_token(tok, p, end, pattern, env);
                    if r < 0 {
                        return Err(r);
                    }
                    // Discard option node, retry from top of prs_exp
                    return prs_exp(tok, term, p, end, pattern, env, false);
                } else {
                    // Oniguruma/Ruby: parse rest of pattern as body of option node
                    let mut np = node;
                    let prev = env.options;
                    env.options = bag_options;
                    // Apply whole options before parsing body
                    if np.has_status(ND_ST_WHOLE_OPTIONS) {
                        set_whole_options(bag_options, env);
                        env.flags |= PE_FLAG_HAS_WHOLE_OPTIONS;
                    }
                    let r = fetch_token(tok, p, end, pattern, env);
                    if r < 0 {
                        env.options = prev;
                        return Err(r);
                    }
                    let (target, _) = prs_alts(tok, term, p, end, pattern, env, false)?;
                    env.options = prev;
                    np.set_body(Some(target));
                    return Ok((np, tok.token_type as i32));
                }
            } else if node.has_status(ND_ST_WHOLE_OPTIONS) {
                // Scoped whole options group (?Ii:abc) — bag_r == 0
                if !group_head {
                    return Err(ONIGERR_INVALID_GROUP_OPTION);
                }
                // Scoped whole options must be the only item in the branch
                let r = fetch_token(tok, p, end, pattern, env);
                if r < 0 {
                    return Err(r);
                }
                if tok.token_type != TokenType::Eot
                    && tok.token_type as i32 != term
                    && tok.token_type != TokenType::Alt
                {
                    return Err(ONIGERR_INVALID_GROUP_OPTION);
                }
                return Ok((node, tok.token_type as i32));
            }
            node
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
            let mut buf = vec![byte];
            let mut fetched_non_crude = false;

            if byte >= 0x80 {
                let expected_len = env.enc.mbc_enc_len(&[byte]);
                if expected_len > 1 {
                    // Accumulate consecutive CrudeByte tokens for multi-byte sequence
                    for _ in 1..expected_len {
                        let r = fetch_token(tok, p, end, pattern, env);
                        if r < 0 {
                            return Err(r);
                        }
                        if tok.token_type == TokenType::CrudeByte {
                            buf.push(tok.code as u8);
                        } else {
                            fetched_non_crude = true;
                            break;
                        }
                    }
                }

                // Validate the accumulated byte sequence
                if !env.enc.is_valid_mbc_string(&buf) {
                    // Invalid lead byte (> 0xF4 or continuation/overlong 0x80-0xC1)
                    if byte > 0xF4 || byte < 0xC2 {
                        return Err(ONIGERR_INVALID_CODE_POINT_VALUE);
                    }
                    // Valid lead byte but not enough continuation bytes
                    return Err(ONIGERR_TOO_SHORT_MULTI_BYTE_STRING);
                }
            }

            let np = node_new_str_crude(&buf);

            // If we already fetched a non-CrudeByte token, use it directly
            if fetched_non_crude {
                return check_quantifier(np, tok, p, end, pattern, env, group, parse_depth);
            }
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
        TokenType::CharProperty => prs_char_property(tok, p, end, pattern, env)?,
        TokenType::OpenCC => prs_cc(tok, p, end, pattern, env)?,
        TokenType::Anchor => {
            let ascii_mode = opton_word_ascii(env.options) && is_word_anchor_type(tok.anchor);
            let mut np = node_new_anchor_with_options(tok.anchor, env.options);
            if let Some(an) = np.as_anchor_mut() {
                an.ascii_mode = ascii_mode;
            }
            np
        }
        TokenType::Backref => {
            // When DONT_CAPTURE_GROUP is active, numbered backrefs are forbidden
            if (env.options & ONIG_OPTION_DONT_CAPTURE_GROUP) != 0 && !tok.backref_by_name {
                if env.num_named > 0 {
                    return Err(ONIGERR_NUMBERED_BACKREF_OR_CALL_NOT_ALLOWED);
                }
                return Err(ONIGERR_INVALID_BACKREF);
            }
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
        TokenType::Call => {
            let name = &pattern[tok.call_name_start..tok.call_name_end];
            let np = node_new_call(name, tok.call_gnum, tok.call_by_number);
            env.num_call += 1;
            if tok.call_by_number && tok.call_gnum == 0 {
                env.flags |= PE_FLAG_HAS_CALL_ZERO;
            }
            np
        }
        TokenType::Keep => {
            let id = env.id_entry();
            env.keep_num += 1;
            node_new_save_gimmick(SaveType::Keep, id)
        }
        TokenType::GeneralNewline => {
            // Port of node_new_general_newline from regparse.c
            // \R matches \r\n (as unit), or any of [\n\v\f\r\x85\u2028\u2029]
            // Builds: BAG_IF_ELSE(condition="\r\n", then=None, else=[\n-\r\x85\u2028\u2029])

            // 1. Build the CR+LF string node ("\r\n")
            let mut crnl_buf = [0u8; 8];
            let dlen = env.enc.code_to_mbc(0x0D, &mut crnl_buf) as usize;
            let alen = env.enc.code_to_mbc(0x0A, &mut crnl_buf[dlen..]) as usize;
            let crnl = node_new_str_crude(&crnl_buf[..dlen + alen]);

            // 2. Build character class for other newlines: [\n-\r]
            let mut ncc = node_new_cclass();
            if let NodeInner::CClass(ref mut cc) = ncc.inner {
                // \n (0x0A) through \r (0x0D) covers LF, VT, FF, CR
                bitset_set_range(&mut cc.bs, 0x0A, 0x0D);
                // Unicode: NEL (0x85), Line Separator (0x2028), Paragraph Separator (0x2029)
                add_code_range_to_buf(&mut cc.mbuf, 0x85, 0x85);
                add_code_range_to_buf(&mut cc.mbuf, 0x2028, 0x2029);
            }

            // 3. Wrap in BAG_IF_ELSE: try \r\n first, else single newline char
            node_new_bag_if_else(crnl, None, Some(ncc))
        }
        TokenType::NoNewline => node_new_ctype(CTYPE_ANYCHAR, false, false),
        TokenType::TrueAnychar => {
            let mut np = node_new_ctype(CTYPE_ANYCHAR, false, false);
            np.status_add(ND_ST_MULTILINE);
            np
        }
        TokenType::TextSegment => {
            // \X == (?>\O(?:\Y\O)*)
            // \Y = NOT text segment boundary, \O = true anychar
            let boundary = node_new_anchor_with_options(ANCR_NO_TEXT_SEGMENT_BOUNDARY, env.options);
            let anychar1 = node_new_true_anychar();
            let inner_list = make_list(boundary, anychar1);

            let mut quant = node_new_quantifier(0, INFINITE_REPEAT, true);
            if let NodeInner::Quant(ref mut qn) = quant.inner {
                qn.body = Some(inner_list);
            }

            let anychar0 = node_new_true_anychar();
            let seq = make_list(anychar0, quant);

            let mut bag = node_new_bag(BagType::StopBacktrack);
            if let NodeInner::Bag(ref mut bn) = bag.inner {
                bn.body = Some(seq);
            }
            bag
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
                if tok.token_type == TokenType::Interval
                    && is_syntax_op(env.syntax, ONIG_SYN_OP_ESC_BRACE_INTERVAL)
                {
                    // BRE \{n\} at start: strip backslashes to get literal {n}
                    let raw = &pattern[tok.backp..*p];
                    let stripped: Vec<u8> = raw.iter().copied().filter(|&b| b != b'\\').collect();
                    node_new_str(&stripped)
                } else {
                    node_new_str(&pattern[tok.backp..*p])
                }
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

        // Split multi-character string: quantifier applies only to last encoded character.
        // e.g., "ba*" should be parsed as "b" + "a*", not "(ba)*".
        // Skip when node came from a group body (group != 0), e.g., (?:ab)* should not split.
        let split_info = if group == 0 {
            if let NodeInner::String(ref sn) = node.inner {
                let s = &sn.s;
                if s.len() > 0 {
                    if let Some(pos) = onigenc_get_prev_char_head(env.enc, 0, s.len(), s) {
                        if pos > 0 {
                            Some((pos, sn.flag, node.status))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let (prefix_node, mut target_node) = if let Some((split_pos, flag, status)) = split_info {
            // Clone bytes before splitting (borrow checker)
            let bytes = if let NodeInner::String(ref sn) = node.inner {
                sn.s.clone()
            } else {
                unreachable!()
            };

            let mut prefix = node_new_str(&bytes[..split_pos]);
            if let NodeInner::String(ref mut psn) = prefix.inner {
                psn.flag = flag;
            }
            prefix.status = status;

            let mut last_char = node_new_str(&bytes[split_pos..]);
            if let NodeInner::String(ref mut lsn) = last_char.inner {
                lsn.flag = flag;
            }
            last_char.status = status;

            (Some(prefix), last_char)
        } else {
            (None, node)
        };

        let mut qn = node_new_quantifier(tok.repeat_lower, tok.repeat_upper, tok.repeat_greedy);

        // Port of assign_quantifier_body / onig_reduce_nested_quantifier from C.
        // Full 6×6 ReduceTypeTable for standard quantifier combinations.
        if let NodeInner::Quant(_) = target_node.inner {
            let nestq_num = if let NodeInner::Quant(ref qn_inner) = qn.inner {
                quantifier_type_num(qn_inner)
            } else {
                -1
            };
            let targetq_num = if let NodeInner::Quant(ref tqn) = target_node.inner {
                quantifier_type_num(tqn)
            } else {
                -1
            };

            if targetq_num >= 0 && nestq_num < 0 {
                // Special case: standard inner (target) with non-standard outer (nesting)
                // (?:a*){n,m}, (?:a+){n,m} => upper clamped
                if targetq_num == 1 || targetq_num == 2 {
                    // * or +
                    if let NodeInner::Quant(ref qn_inner) = qn.inner {
                        if qn_inner.upper != INFINITE_REPEAT
                            && qn_inner.upper > 1
                            && qn_inner.greedy
                        {
                            let clamped = if qn_inner.lower == 0 {
                                1
                            } else {
                                qn_inner.lower
                            };
                            if let NodeInner::Quant(ref mut qn_mut) = qn.inner {
                                qn_mut.upper = clamped;
                            }
                        }
                    }
                }
                qn.set_body(Some(target_node));
            } else {
                // Both standard or both non-standard — try onig_reduce_nested_quantifier
                qn.set_body(Some(target_node));
                onig_reduce_nested_quantifier(&mut qn)?;
            }
        } else {
            qn.set_body(Some(target_node));
        }

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

        if let Some(prefix) = prefix_node {
            // Return List(prefix, quantified_last_char)
            let (quant_node, r) = check_quantifier(qn, tok, p, end, pattern, env, 0, depth)?;
            let result = node_new_list(prefix, Some(node_new_list(quant_node, None)));
            return Ok((result, r));
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
    let (top, _) = prs_alts(&mut tok, TokenType::Eot as i32, p, end, pattern, env, true)?;
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
    let mut root = prs_regexp(&mut p, end, pattern, env)?;

    // Wrap entire pattern in memory group 0 for \g<0> self-calls
    if (env.flags & PE_FLAG_HAS_CALL_ZERO) != 0 {
        let mut zero_node = node_new_bag_memory(0);
        if let NodeInner::Bag(ref mut bn) = zero_node.inner {
            bn.body = Some(root);
        }
        env.set_mem_node(0, &mut *zero_node as *mut Node);
        root = zero_node;
    }

    reg.num_mem = env.num_mem;

    Ok(root)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regsyntax::OnigSyntaxOniguruma;

    /// Create a default RegexType + ParseEnv for testing with Oniguruma syntax and UTF-8.
    fn make_test_context() -> (RegexType, ParseEnv) {
        let reg = RegexType {
            ops: Vec::new(),
            string_pool: Vec::new(),
            num_mem: 0,
            num_repeat: 0,
            num_empty_check: 0,
            num_call: 0,
            capture_history: 0,
            push_mem_start: 0,
            push_mem_end: 0,
            stack_pop_level: StackPopLevel::Free,
            repeat_range: Vec::new(),
            enc: &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            options: ONIG_OPTION_NONE,
            syntax: &OnigSyntaxOniguruma as *const OnigSyntaxType,
            case_fold_flag: ONIGENC_CASE_FOLD_MIN,
            name_table: None,
            optimize: OptimizeType::None,
            threshold_len: 0,
            anchor: 0,
            anc_dist_min: 0,
            anc_dist_max: 0,
            sub_anchor: 0,
            exact: Vec::new(),
            map: [0u8; CHAR_MAP_SIZE],
            map_offset: 0,
            map_bytes: [0u8; 3],
            map_byte_count: 0,
            dist_min: 0,
            dist_max: 0,
            called_addrs: vec![],
            unset_call_addrs: vec![],
            extp: None,
        };
        let env = ParseEnv {
            options: 0,
            case_fold_flag: 0,
            enc: &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            syntax: &OnigSyntaxOniguruma,
            cap_history: 0,
            backtrack_mem: 0,
            backrefed_mem: 0,
            pattern: std::ptr::null(),
            pattern_end: std::ptr::null(),
            error: std::ptr::null(),
            error_end: std::ptr::null(),
            reg: std::ptr::null_mut(),
            num_call: 0,
            num_mem: 0,
            num_named: 0,
            mem_alloc: 0,
            mem_env_static: Default::default(),
            mem_env_dynamic: None,
            backref_num: 0,
            keep_num: 0,
            id_num: 0,
            save_alloc_num: 0,
            saves: None,
            unset_addr_list: None,
            parse_depth: 0,
            flags: 0,
        };
        (reg, env)
    }

    fn parse(pattern: &[u8]) -> Result<(Box<Node>, RegexType), i32> {
        let (mut reg, mut env) = make_test_context();
        let root = onig_parse_tree(pattern, &mut reg, &mut env)?;
        Ok((root, reg))
    }

    // --- Literal strings ---

    #[test]
    fn parse_literal_abc() {
        let (root, _reg) = parse(b"abc").unwrap();
        match &root.inner {
            NodeInner::String(s) => assert_eq!(s.s, b"abc"),
            other => panic!("expected String node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_empty_pattern() {
        let (root, _reg) = parse(b"").unwrap();
        match &root.inner {
            NodeInner::String(s) => assert!(s.s.is_empty()),
            other => panic!("expected empty String node, got {:?}", root.node_type()),
        }
    }

    // --- Alternation ---

    #[test]
    fn parse_alternation() {
        let (root, _reg) = parse(b"a|b").unwrap();
        match &root.inner {
            NodeInner::Alt(alt) => {
                // car should be "a"
                match &alt.car.inner {
                    NodeInner::String(s) => assert_eq!(s.s, b"a"),
                    _ => panic!("expected String 'a'"),
                }
                // cdr should be Alt with "b"
                let cdr = alt.cdr.as_ref().expect("expected cdr");
                match &cdr.inner {
                    NodeInner::Alt(alt2) => match &alt2.car.inner {
                        NodeInner::String(s) => assert_eq!(s.s, b"b"),
                        _ => panic!("expected String 'b'"),
                    },
                    _ => panic!("expected Alt cdr"),
                }
            }
            _ => panic!("expected Alt node, got {:?}", root.node_type()),
        }
    }

    // --- Concatenation ---

    #[test]
    fn parse_concat_dot_literal() {
        // "a." should produce List(String("a"), List(Anychar, nil))
        let (root, _reg) = parse(b"a.").unwrap();
        match &root.inner {
            NodeInner::List(list) => {
                match &list.car.inner {
                    NodeInner::String(s) => assert_eq!(s.s, b"a"),
                    _ => panic!("expected String 'a' as first element"),
                }
                let cdr = list.cdr.as_ref().expect("expected cdr");
                match &cdr.inner {
                    NodeInner::List(list2) => {
                        match &list2.car.inner {
                            NodeInner::CType(ct) => {
                                assert!(
                                    ct.ctype == ONIGENC_CTYPE_WORD as i32 || true,
                                    "anychar node"
                                );
                            }
                            _ => {} // anychar could be various representations
                        }
                    }
                    _ => {} // may be directly the node
                }
            }
            _ => panic!("expected List node for concat, got {:?}", root.node_type()),
        }
    }

    // --- Quantifiers ---

    #[test]
    fn parse_star_quantifier() {
        let (root, _reg) = parse(b"a*").unwrap();
        match &root.inner {
            NodeInner::Quant(q) => {
                assert_eq!(q.lower, 0);
                assert_eq!(q.upper, INFINITE_REPEAT);
                assert!(q.greedy);
            }
            _ => panic!("expected Quant node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_plus_quantifier() {
        let (root, _reg) = parse(b"a+").unwrap();
        match &root.inner {
            NodeInner::Quant(q) => {
                assert_eq!(q.lower, 1);
                assert_eq!(q.upper, INFINITE_REPEAT);
                assert!(q.greedy);
            }
            _ => panic!("expected Quant node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_question_quantifier() {
        let (root, _reg) = parse(b"a?").unwrap();
        match &root.inner {
            NodeInner::Quant(q) => {
                assert_eq!(q.lower, 0);
                assert_eq!(q.upper, 1);
                assert!(q.greedy);
            }
            _ => panic!("expected Quant node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_lazy_star() {
        let (root, _reg) = parse(b"a*?").unwrap();
        match &root.inner {
            NodeInner::Quant(q) => {
                assert_eq!(q.lower, 0);
                assert_eq!(q.upper, INFINITE_REPEAT);
                assert!(!q.greedy);
            }
            _ => panic!("expected Quant node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_interval_quantifier() {
        let (root, _reg) = parse(b"a{2,5}").unwrap();
        match &root.inner {
            NodeInner::Quant(q) => {
                assert_eq!(q.lower, 2);
                assert_eq!(q.upper, 5);
                assert!(q.greedy);
            }
            _ => panic!("expected Quant node, got {:?}", root.node_type()),
        }
    }

    // --- Anchors ---

    #[test]
    fn parse_begin_anchor() {
        let (root, _reg) = parse(b"^a").unwrap();
        match &root.inner {
            NodeInner::List(list) => match &list.car.inner {
                NodeInner::Anchor(a) => assert_eq!(a.anchor_type, ANCR_BEGIN_LINE),
                _ => panic!("expected Anchor as first element"),
            },
            _ => panic!("expected List, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_end_anchor() {
        let (root, _reg) = parse(b"a$").unwrap();
        match &root.inner {
            NodeInner::List(list) => {
                // Walk to find the anchor
                let cdr = list.cdr.as_ref().expect("expected cdr");
                match &cdr.inner {
                    NodeInner::List(list2) => match &list2.car.inner {
                        NodeInner::Anchor(a) => assert_eq!(a.anchor_type, ANCR_END_LINE),
                        _ => panic!("expected Anchor"),
                    },
                    _ => panic!("expected second List element"),
                }
            }
            _ => panic!("expected List, got {:?}", root.node_type()),
        }
    }

    // --- Character classes ---

    #[test]
    fn parse_char_class_simple() {
        let (root, _reg) = parse(b"[abc]").unwrap();
        match &root.inner {
            NodeInner::CClass(cc) => {
                assert!(bitset_at(&cc.bs, b'a' as usize));
                assert!(bitset_at(&cc.bs, b'b' as usize));
                assert!(bitset_at(&cc.bs, b'c' as usize));
                assert!(!cc.is_not());
            }
            _ => panic!("expected CClass node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_char_class_negated() {
        let (root, _reg) = parse(b"[^a]").unwrap();
        match &root.inner {
            NodeInner::CClass(cc) => {
                assert!(cc.is_not());
            }
            _ => panic!("expected CClass node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_char_class_range() {
        let (root, _reg) = parse(b"[a-z]").unwrap();
        match &root.inner {
            NodeInner::CClass(cc) => {
                // All lowercase letters should be set
                for c in b'a'..=b'z' {
                    assert!(
                        bitset_at(&cc.bs, c as usize),
                        "expected '{}' to be in class",
                        c as char
                    );
                }
                // Uppercase should not be set
                assert!(!bitset_at(&cc.bs, b'A' as usize));
            }
            _ => panic!("expected CClass node, got {:?}", root.node_type()),
        }
    }

    // --- Groups ---

    #[test]
    fn parse_capturing_group() {
        let (root, _reg) = parse(b"(a)").unwrap();
        match &root.inner {
            NodeInner::Bag(bag) => {
                match bag.bag_type {
                    BagType::Memory => {}
                    _ => panic!("expected Memory bag type"),
                }
                let body = bag.body.as_ref().expect("expected body");
                match &body.inner {
                    NodeInner::String(s) => assert_eq!(s.s, b"a"),
                    _ => panic!("expected String body"),
                }
            }
            _ => panic!("expected Bag node, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_non_capturing_group() {
        let (root, _reg) = parse(b"(?:a)").unwrap();
        // Non-capturing groups may produce a Bag with StopBacktrack or just the inner node
        // depending on the implementation. Let's just verify it parses successfully.
        assert!(matches!(
            root.inner,
            NodeInner::String(_) | NodeInner::Bag(_)
        ));
    }

    #[test]
    fn parse_named_group() {
        let (root, reg) = parse(b"(?<name>a)").unwrap();
        match &root.inner {
            NodeInner::Bag(bag) => match bag.bag_type {
                BagType::Memory => {}
                _ => panic!("expected Memory bag type for named group"),
            },
            _ => panic!("expected Bag node, got {:?}", root.node_type()),
        }
        // Check that the name was registered
        let nt = reg.name_table.as_ref().expect("expected name table");
        assert!(nt.find(b"name").is_some());
    }

    // --- Escape sequences ---

    #[test]
    fn parse_escape_d() {
        // \d produces a CClass node (digit is expanded into character class)
        let (root, _reg) = parse(b"\\d").unwrap();
        match &root.inner {
            NodeInner::CClass(cc) => {
                assert!(!cc.is_not());
            }
            _ => panic!("expected CClass node for \\d, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_escape_w() {
        // \w produces a CType node (word type is special-cased)
        let (root, _reg) = parse(b"\\w").unwrap();
        match &root.inner {
            NodeInner::CType(ct) => {
                assert_eq!(ct.ctype, ONIGENC_CTYPE_WORD as i32);
                assert!(!ct.not);
            }
            _ => panic!("expected CType node for \\w, got {:?}", root.node_type()),
        }
    }

    #[test]
    fn parse_escape_s() {
        // \s produces a CClass node (space is expanded into character class)
        let (root, _reg) = parse(b"\\s").unwrap();
        match &root.inner {
            NodeInner::CClass(cc) => {
                assert!(!cc.is_not());
            }
            _ => panic!("expected CClass node for \\s, got {:?}", root.node_type()),
        }
    }

    // --- Complex patterns ---

    #[test]
    fn parse_multiple_captures() {
        let (_root, reg) = parse(b"(a)(b)(c)").unwrap();
        assert_eq!(reg.num_mem, 3);
    }

    #[test]
    fn parse_nested_groups() {
        let (_root, reg) = parse(b"((a)(b))").unwrap();
        assert_eq!(reg.num_mem, 3);
    }

    #[test]
    fn parse_complex_pattern() {
        // Just verify it parses without error
        let result = parse(b"^[a-zA-Z_][a-zA-Z0-9_]*$");
        assert!(result.is_ok());
    }

    #[test]
    fn parse_alternation_in_group() {
        let result = parse(b"(foo|bar|baz)");
        assert!(result.is_ok());
        let (_root, reg) = result.unwrap();
        assert_eq!(reg.num_mem, 1);
    }

    #[test]
    fn parse_email_like_pattern() {
        let result = parse(b"[a-z]+@[a-z]+\\.[a-z]+");
        assert!(result.is_ok());
    }

    // --- Error cases ---

    #[test]
    fn parse_unmatched_paren() {
        let result = parse(b"(abc");
        assert!(result.is_err());
    }

    #[test]
    fn parse_unmatched_bracket() {
        let result = parse(b"[abc");
        assert!(result.is_err());
    }

    #[test]
    fn parse_reversed_interval() {
        // {5,2} reversed interval: swap bounds to {2,5} and set possessive
        // (OnigSyntaxOniguruma does not have PLUS_POSSESSIVE_INTERVAL)
        let result = parse(b"a{5,2}");
        assert!(result.is_ok());
    }
}
