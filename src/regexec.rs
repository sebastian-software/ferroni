// regexec.rs - Port of regexec.c
// VM executor: bytecode interpreter, match_at, onig_match, onig_search.
//
// This is a 1:1 port of oniguruma's regexec.c (~7,000 LOC).
// Structure mirrors the C original: stack types → stack operations →
// match_at (opcode dispatch) → onig_match → onig_search.

#![allow(non_upper_case_globals)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_mut)]

use std::sync::atomic::{AtomicU64, AtomicU32, AtomicPtr, Ordering};
use std::time::Instant;

use crate::oniguruma::*;
use crate::regenc::*;
use crate::regint::*;

/// Callout function type. Receives args and optional user data.
/// Return ONIG_CALLOUT_SUCCESS (0) to continue, ONIG_CALLOUT_FAIL (1) to fail,
/// or a negative error code.
pub type OnigCalloutFunc = fn(args: &OnigCalloutArgs, user_data: *mut std::ffi::c_void) -> i32;

// ============================================================================
// Global Limits (port of C's onig_retry_limit_in_match etc.)
// ============================================================================

static RETRY_LIMIT_IN_MATCH: AtomicU64 = AtomicU64::new(DEFAULT_RETRY_LIMIT_IN_MATCH);
static RETRY_LIMIT_IN_SEARCH: AtomicU64 = AtomicU64::new(DEFAULT_RETRY_LIMIT_IN_SEARCH);
static MATCH_STACK_LIMIT: AtomicU32 = AtomicU32::new(DEFAULT_MATCH_STACK_LIMIT_SIZE);
static TIME_LIMIT: AtomicU64 = AtomicU64::new(DEFAULT_TIME_LIMIT_MSEC);

pub fn onig_set_retry_limit_in_match(n: u64) { RETRY_LIMIT_IN_MATCH.store(n, Ordering::Relaxed); }
pub fn onig_get_retry_limit_in_match() -> u64 { RETRY_LIMIT_IN_MATCH.load(Ordering::Relaxed) }
pub fn onig_set_retry_limit_in_search(n: u64) { RETRY_LIMIT_IN_SEARCH.store(n, Ordering::Relaxed); }
pub fn onig_get_retry_limit_in_search() -> u64 { RETRY_LIMIT_IN_SEARCH.load(Ordering::Relaxed) }
pub fn onig_set_match_stack_limit(n: u32) { MATCH_STACK_LIMIT.store(n, Ordering::Relaxed); }
pub fn onig_get_match_stack_limit() -> u32 { MATCH_STACK_LIMIT.load(Ordering::Relaxed) }
pub fn onig_set_time_limit(n: u64) { TIME_LIMIT.store(n, Ordering::Relaxed); }
pub fn onig_get_time_limit() -> u64 { TIME_LIMIT.load(Ordering::Relaxed) }

// ============================================================================
// Global Progress/Retraction Callout (port of C's global callout funcs)
// ============================================================================

static PROGRESS_CALLOUT: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static RETRACTION_CALLOUT: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

/// Get the global progress callout function.
pub fn onig_get_progress_callout() -> Option<OnigCalloutFunc> {
    let p = PROGRESS_CALLOUT.load(Ordering::Relaxed);
    if p.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute(p) })
    }
}

/// Set the global progress callout function.
pub fn onig_set_progress_callout(f: OnigCalloutFunc) -> i32 {
    let p: *mut () = f as *mut ();
    PROGRESS_CALLOUT.store(p, Ordering::Relaxed);
    ONIG_NORMAL
}

/// Get the global retraction callout function.
pub fn onig_get_retraction_callout() -> Option<OnigCalloutFunc> {
    let p = RETRACTION_CALLOUT.load(Ordering::Relaxed);
    if p.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute(p) })
    }
}

/// Set the global retraction callout function.
pub fn onig_set_retraction_callout(f: OnigCalloutFunc) -> i32 {
    let p: *mut () = f as *mut ();
    RETRACTION_CALLOUT.store(p, Ordering::Relaxed);
    ONIG_NORMAL
}

// ============================================================================
// Global Callback Each Match (port of C's CallbackEachMatch)
// ============================================================================

pub type OnigCallbackEachMatchFunc =
    fn(str_data: &[u8], region: &OnigRegion, user_data: *mut std::ffi::c_void) -> i32;

static CALLBACK_EACH_MATCH: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

pub fn onig_get_callback_each_match() -> Option<OnigCallbackEachMatchFunc> {
    let p = CALLBACK_EACH_MATCH.load(Ordering::Relaxed);
    if p.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute(p) })
    }
}

pub fn onig_set_callback_each_match(f: OnigCallbackEachMatchFunc) -> i32 {
    let p: *mut () = f as *mut ();
    CALLBACK_EACH_MATCH.store(p, Ordering::Relaxed);
    ONIG_NORMAL
}

// ============================================================================
// Region Management (port of C's onig_region_* functions)
// ============================================================================

pub fn onig_region_new() -> OnigRegion {
    OnigRegion::new()
}

pub fn onig_region_init(region: &mut OnigRegion) {
    region.init();
}

pub fn onig_region_clear(region: &mut OnigRegion) {
    region.clear();
}

pub fn onig_region_resize(region: &mut OnigRegion, n: i32) -> i32 {
    region.resize(n);
    ONIG_NORMAL
}

pub fn onig_region_set(region: &mut OnigRegion, at: i32, beg: i32, end: i32) -> i32 {
    region.set(at, beg, end)
}

pub fn onig_region_copy(to: &mut OnigRegion, from: &OnigRegion) {
    to.copy_from(from);
}

// ============================================================================
// Regex Accessors (port of C's onig_get_*/onig_number_of_* functions)
// ============================================================================

pub fn onig_get_encoding(reg: &RegexType) -> OnigEncoding {
    reg.enc
}

pub fn onig_get_options(reg: &RegexType) -> OnigOptionType {
    reg.options
}

pub fn onig_get_case_fold_flag(reg: &RegexType) -> OnigCaseFoldType {
    reg.case_fold_flag
}

pub fn onig_get_syntax(reg: &RegexType) -> *const OnigSyntaxType {
    reg.syntax
}

pub fn onig_number_of_captures(reg: &RegexType) -> i32 {
    reg.num_mem
}

pub fn onig_number_of_capture_histories(reg: &RegexType) -> i32 {
    let mut n = 0;
    for i in 0..=ONIG_MAX_CAPTURE_HISTORY_GROUP {
        if mem_status_at(reg.capture_history, i) {
            n += 1;
        }
    }
    n
}

pub fn onig_get_capture_tree(region: &OnigRegion) -> Option<&OnigCaptureTreeNode> {
    region.history_root.as_deref()
}

// ============================================================================
// Name Table Queries (port of C's onig_name_to_* / onig_foreach_name)
// ============================================================================

/// Returns the number of group numbers for the given name, and a slice of them.
pub fn onig_name_to_group_numbers<'a>(reg: &'a RegexType, name: &[u8]) -> Result<&'a [i32], i32> {
    if let Some(ref nt) = reg.name_table {
        if let Some(entry) = nt.find(name) {
            Ok(&entry.back_refs)
        } else {
            Err(ONIGERR_UNDEFINED_NAME_REFERENCE)
        }
    } else {
        Err(ONIGERR_UNDEFINED_NAME_REFERENCE)
    }
}

/// Resolve an ambiguous named backref to a single group number using region state.
/// If multiple groups share a name, returns the last one that participated in the match.
pub fn onig_name_to_backref_number(
    reg: &RegexType,
    name: &[u8],
    region: Option<&OnigRegion>,
) -> Result<i32, i32> {
    let nums = onig_name_to_group_numbers(reg, name)?;
    if nums.is_empty() {
        return Err(ONIGERR_PARSER_BUG);
    }
    if nums.len() == 1 {
        return Ok(nums[0]);
    }
    // Multiple groups share this name — pick the last one that matched
    if let Some(region) = region {
        for i in (0..nums.len()).rev() {
            let idx = nums[i] as usize;
            if idx < region.beg.len() && region.beg[idx] != ONIG_REGION_NOTPOS {
                return Ok(nums[i]);
            }
        }
    }
    Ok(nums[nums.len() - 1])
}

/// Iterate over all name entries. Callback receives (name, back_refs).
/// If callback returns non-zero, iteration stops and that value is returned.
pub fn onig_foreach_name<F>(reg: &RegexType, mut callback: F) -> i32
where
    F: FnMut(&[u8], &[i32]) -> i32,
{
    if let Some(ref nt) = reg.name_table {
        for entry in nt.entries.values() {
            let r = callback(&entry.name, &entry.back_refs);
            if r != 0 {
                return r;
            }
        }
    }
    ONIG_NORMAL
}

pub fn onig_number_of_names(reg: &RegexType) -> i32 {
    if let Some(ref nt) = reg.name_table {
        nt.entries.len() as i32
    } else {
        0
    }
}

pub fn onig_noname_group_capture_is_active(reg: &RegexType) -> bool {
    if opton_dont_capture_group(reg.options) {
        return false;
    }
    if onig_number_of_names(reg) > 0 {
        let syntax = unsafe { &*reg.syntax };
        if is_syntax_bv(syntax, ONIG_SYN_CAPTURE_ONLY_NAMED_GROUP)
            && !opton_capture_group(reg.options)
        {
            return false;
        }
    }
    true
}

// ============================================================================
// Utility + Version (port of regversion.c + regexec.c globals)
// ============================================================================

pub fn onig_version() -> &'static str {
    "6.9.10"
}

pub fn onig_copyright() -> &'static str {
    "Oniguruma 6.9.10 : Copyright (C) 2002-2024 K.Kosako"
}

pub fn onig_init() -> i32 { ONIG_NORMAL }
pub fn onig_initialize() -> i32 { ONIG_NORMAL }
pub fn onig_end() -> i32 { ONIG_NORMAL }

static SUBEXP_CALL_LIMIT_IN_SEARCH: AtomicU64 = AtomicU64::new(DEFAULT_SUBEXP_CALL_LIMIT_IN_SEARCH);
static SUBEXP_CALL_MAX_NEST_LEVEL: AtomicU32 = AtomicU32::new(DEFAULT_SUBEXP_CALL_MAX_NEST_LEVEL as u32);

pub fn onig_get_subexp_call_limit_in_search() -> u64 {
    SUBEXP_CALL_LIMIT_IN_SEARCH.load(Ordering::Relaxed)
}

pub fn onig_set_subexp_call_limit_in_search(n: u64) -> i32 {
    SUBEXP_CALL_LIMIT_IN_SEARCH.store(n, Ordering::Relaxed);
    ONIG_NORMAL
}

pub fn onig_get_subexp_call_max_nest_level() -> i32 {
    SUBEXP_CALL_MAX_NEST_LEVEL.load(Ordering::Relaxed) as i32
}

pub fn onig_set_subexp_call_max_nest_level(level: i32) -> i32 {
    SUBEXP_CALL_MAX_NEST_LEVEL.store(level as u32, Ordering::Relaxed);
    ONIG_NORMAL
}

/// Scan for all non-overlapping matches of `reg` in `str_data`.
/// For each match, calls `callback(match_count, match_position, region)`.
/// If callback returns non-zero, scanning stops and that value is returned.
/// Otherwise returns the total number of matches found.
pub fn onig_scan<F>(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    region: OnigRegion,
    option: OnigOptionType,
    mut callback: F,
) -> (i32, OnigRegion)
where
    F: FnMut(i32, i32, &OnigRegion) -> i32,
{
    let enc = reg.enc;
    let mut n: i32 = 0;
    let mut start = 0usize;
    let mut region = region;
    let mut option = option;

    if opton_check_validity_of_string(option) {
        if !enc.is_valid_mbc_string(&str_data[..end]) {
            return (ONIGERR_INVALID_WIDE_CHAR_VALUE, region);
        }
        option &= !ONIG_OPTION_CHECK_VALIDITY_OF_STRING;
    }

    loop {
        let (r, returned_region) = onig_search(reg, str_data, end, start, end, Some(region), option);
        region = returned_region.unwrap_or_else(OnigRegion::new);

        if r >= 0 {
            let rs = callback(n, r, &region);
            n += 1;
            if rs != 0 {
                return (rs, region);
            }
            if region.num_regs > 0 && region.end[0] == start as i32 {
                if start >= end {
                    break;
                }
                start += enclen(enc, str_data, start);
            } else if region.num_regs > 0 {
                start = region.end[0] as usize;
            } else {
                break;
            }
            if start > end {
                break;
            }
        } else if r == ONIG_MISMATCH {
            break;
        } else {
            // error
            return (r, region);
        }
    }

    (n, region)
}

// ============================================================================
// OnigMatchParam (per-search limit overrides)
// ============================================================================

pub struct OnigMatchParam {
    pub match_stack_limit: u32,
    pub retry_limit_in_match: u64,
    pub retry_limit_in_search: u64,
    pub time_limit: u64,
    pub progress_callout: Option<OnigCalloutFunc>,
    pub retraction_callout: Option<OnigCalloutFunc>,
    pub callout_user_data: *mut std::ffi::c_void,
}

pub fn onig_new_match_param() -> OnigMatchParam {
    let mut mp = OnigMatchParam {
        match_stack_limit: 0,
        retry_limit_in_match: 0,
        retry_limit_in_search: 0,
        time_limit: 0,
        progress_callout: None,
        retraction_callout: None,
        callout_user_data: std::ptr::null_mut(),
    };
    onig_initialize_match_param(&mut mp);
    mp
}

pub fn onig_initialize_match_param(mp: &mut OnigMatchParam) -> i32 {
    mp.match_stack_limit = MATCH_STACK_LIMIT.load(Ordering::Relaxed);
    mp.retry_limit_in_match = RETRY_LIMIT_IN_MATCH.load(Ordering::Relaxed);
    mp.retry_limit_in_search = RETRY_LIMIT_IN_SEARCH.load(Ordering::Relaxed);
    mp.time_limit = TIME_LIMIT.load(Ordering::Relaxed);
    mp.progress_callout = onig_get_progress_callout();
    mp.retraction_callout = onig_get_retraction_callout();
    mp.callout_user_data = std::ptr::null_mut();
    ONIG_NORMAL
}

pub fn onig_set_match_stack_limit_size_of_match_param(mp: &mut OnigMatchParam, limit: u32) -> i32 {
    mp.match_stack_limit = limit;
    ONIG_NORMAL
}

pub fn onig_set_retry_limit_in_match_of_match_param(mp: &mut OnigMatchParam, limit: u64) -> i32 {
    mp.retry_limit_in_match = limit;
    ONIG_NORMAL
}

pub fn onig_set_retry_limit_in_search_of_match_param(mp: &mut OnigMatchParam, limit: u64) -> i32 {
    mp.retry_limit_in_search = limit;
    ONIG_NORMAL
}

pub fn onig_set_time_limit_of_match_param(mp: &mut OnigMatchParam, limit: u64) -> i32 {
    mp.time_limit = limit;
    ONIG_NORMAL
}

pub fn onig_set_progress_callout_of_match_param(
    mp: &mut OnigMatchParam,
    f: Option<OnigCalloutFunc>,
) -> i32 {
    mp.progress_callout = f;
    ONIG_NORMAL
}

pub fn onig_set_retraction_callout_of_match_param(
    mp: &mut OnigMatchParam,
    f: Option<OnigCalloutFunc>,
) -> i32 {
    mp.retraction_callout = f;
    ONIG_NORMAL
}

pub fn onig_set_callout_user_data_of_match_param(
    mp: &mut OnigMatchParam,
    user_data: *mut std::ffi::c_void,
) -> i32 {
    mp.callout_user_data = user_data;
    ONIG_NORMAL
}

// ============================================================================
// OnigCalloutArgs (port of C's OnigCalloutArgsStruct)
// ============================================================================

/// Callout arguments passed to callout functions.
/// Provides access to match state at the point of the callout.
pub struct OnigCalloutArgs {
    pub callout_in: OnigCalloutIn,
    pub name_id: i32,
    pub num: i32,
    pub regex: *const RegexType,
    pub string: *const u8,
    pub string_end: *const u8,
    pub start: *const u8,
    pub right_range: *const u8,
    pub current: *const u8,
    pub retry_in_match_counter: u64,
    // String data for safe access
    str_data: *const u8,
    str_len: usize,
    // Callout data array (for by_callout_args accessor functions)
    pub(crate) callout_data: *mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
}

impl OnigCalloutArgs {
    pub(crate) fn new(
        callout_in: OnigCalloutIn,
        name_id: i32,
        num: i32,
        reg: &RegexType,
        str_data: &[u8],
        end: usize,
        start: usize,
        right_range: usize,
        current: usize,
        retry_counter: u64,
    ) -> Self {
        OnigCalloutArgs {
            callout_in,
            name_id,
            num,
            regex: reg as *const RegexType,
            string: str_data.as_ptr(),
            string_end: unsafe { str_data.as_ptr().add(end) },
            start: unsafe { str_data.as_ptr().add(start) },
            right_range: unsafe { str_data.as_ptr().add(right_range) },
            current: unsafe { str_data.as_ptr().add(current) },
            retry_in_match_counter: retry_counter,
            str_data: str_data.as_ptr(),
            str_len: str_data.len(),
            callout_data: std::ptr::null_mut(),
        }
    }
}

// --- OnigCalloutArgs accessor functions ---

pub fn onig_get_callout_num_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    args.num
}

pub fn onig_get_callout_in_by_callout_args(args: &OnigCalloutArgs) -> OnigCalloutIn {
    args.callout_in
}

pub fn onig_get_name_id_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    args.name_id
}

pub fn onig_get_contents_by_callout_args(_args: &OnigCalloutArgs) -> Option<&[u8]> {
    // Contents of (?{...}) callouts are not stored in CalloutListEntry
    // in the current implementation. Returns None.
    None
}

pub fn onig_get_args_num_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    let reg = unsafe { &*args.regex };
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].args.len() as i32;
        }
    }
    0
}

pub fn onig_get_passed_args_num_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    let reg = unsafe { &*args.regex };
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].args.len() as i32;
        }
    }
    0
}

pub fn onig_get_arg_by_callout_args(
    args: &OnigCalloutArgs,
    index: i32,
) -> Option<&CalloutArg> {
    let reg = unsafe { &*args.regex };
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            let entry = &ext.callout_list[idx];
            if (index as usize) < entry.args.len() {
                return Some(&entry.args[index as usize]);
            }
        }
    }
    None
}

pub fn onig_get_string_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.string
}

pub fn onig_get_string_end_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.string_end
}

pub fn onig_get_start_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.start
}

pub fn onig_get_right_range_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.right_range
}

pub fn onig_get_current_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.current
}

pub fn onig_get_regex_by_callout_args(args: &OnigCalloutArgs) -> *const RegexType {
    args.regex
}

pub fn onig_get_retry_counter_by_callout_args(args: &OnigCalloutArgs) -> u64 {
    args.retry_in_match_counter
}

/// Get the end position of callout contents.
/// For content callouts (?{...}), returns the end pointer of the content.
/// Returns null for name callouts.
pub fn onig_get_contents_end_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    let reg = unsafe { &*args.regex };
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            let entry = &ext.callout_list[idx];
            if entry.of == OnigCalloutOf::Contents as i32 {
                if let Some(ref content_end) = entry.content_end {
                    return content_end.as_ptr();
                }
            }
        }
    }
    std::ptr::null()
}

// ============================================================================
// Callout Data Access (port of C's onig_get/set_callout_data)
// ============================================================================

/// Get callout data from a callout's data slot.
/// `callout_num` is 1-based, `slot` ranges 0..ONIG_CALLOUT_DATA_SLOT_NUM.
pub fn onig_get_callout_data(
    reg: &RegexType,
    callout_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    callout_num: i32,
    slot: i32,
) -> Option<i64> {
    if callout_num < 1 || slot < 0 || slot >= ONIG_CALLOUT_DATA_SLOT_NUM as i32 {
        return None;
    }
    let idx = (callout_num - 1) as usize;
    if idx >= callout_data.len() {
        return None;
    }
    Some(callout_data[idx][slot as usize])
}

/// Set callout data in a callout's data slot.
pub fn onig_set_callout_data(
    callout_data: &mut [[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    callout_num: i32,
    slot: i32,
    val: i64,
) -> i32 {
    if callout_num < 1 || slot < 0 || slot >= ONIG_CALLOUT_DATA_SLOT_NUM as i32 {
        return ONIGERR_INVALID_ARGUMENT;
    }
    let idx = (callout_num - 1) as usize;
    if idx >= callout_data.len() {
        return ONIGERR_INVALID_ARGUMENT;
    }
    callout_data[idx][slot as usize] = val;
    ONIG_NORMAL
}

// ============================================================================
// Callout Tag Query Functions
// ============================================================================

/// Get the callout number for a given tag in the regex.
pub fn onig_get_callout_num_by_tag(
    reg: &RegexType,
    tag: &[u8],
) -> i32 {
    if let Some(ref ext) = reg.extp {
        if let Some(ref table) = ext.tag_table {
            if let Some(&num) = table.get(tag) {
                return num;
            }
        }
    }
    ONIGERR_INVALID_ARGUMENT
}

/// Check if a callout has a tag.
pub fn onig_callout_tag_is_exist_at_callout_num(
    reg: &RegexType,
    callout_num: i32,
) -> bool {
    if let Some(ref ext) = reg.extp {
        let idx = (callout_num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].tag.is_some();
        }
    }
    false
}

/// Get the tag for a given callout number.
pub fn onig_get_callout_tag(
    reg: &RegexType,
    callout_num: i32,
) -> Option<&[u8]> {
    if let Some(ref ext) = reg.extp {
        let idx = (callout_num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].tag.as_deref();
        }
    }
    None
}

// ============================================================================
// Callout Data by_callout_args variants
// (port of C's onig_get/set_callout_data_by_callout_args*)
// ============================================================================

pub fn onig_get_callout_data_by_callout_args(
    args: &OnigCalloutArgs,
    callout_num: i32,
    slot: i32,
) -> Option<i64> {
    let reg = unsafe { &*args.regex };
    if args.callout_data.is_null() { return None; }
    let cd = unsafe { &*args.callout_data };
    onig_get_callout_data(reg, cd, callout_num, slot)
}

pub fn onig_set_callout_data_by_callout_args(
    args: &OnigCalloutArgs,
    callout_num: i32,
    slot: i32,
    val: i64,
) -> i32 {
    if args.callout_data.is_null() { return ONIGERR_INVALID_ARGUMENT; }
    let cd = unsafe { &mut *args.callout_data };
    onig_set_callout_data(cd, callout_num, slot, val)
}

pub fn onig_get_callout_data_by_callout_args_self(
    args: &OnigCalloutArgs,
    slot: i32,
) -> Option<i64> {
    onig_get_callout_data_by_callout_args(args, args.num, slot)
}

pub fn onig_set_callout_data_by_callout_args_self(
    args: &OnigCalloutArgs,
    slot: i32,
    val: i64,
) -> i32 {
    onig_set_callout_data_by_callout_args(args, args.num, slot, val)
}

/// Get callout data without clearing old values.
/// In the Rust implementation, this behaves identically to onig_get_callout_data
/// since the Rust version does not implement the clear-on-access pattern.
pub fn onig_get_callout_data_dont_clear_old(
    reg: &RegexType,
    callout_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    callout_num: i32,
    slot: i32,
) -> Option<i64> {
    onig_get_callout_data(reg, callout_data, callout_num, slot)
}

pub fn onig_get_callout_data_by_callout_args_self_dont_clear_old(
    args: &OnigCalloutArgs,
    slot: i32,
) -> Option<i64> {
    onig_get_callout_data_by_callout_args_self(args, slot)
}

/// Get callout data by tag name.
pub fn onig_get_callout_data_by_tag(
    reg: &RegexType,
    callout_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    tag: &[u8],
    slot: i32,
) -> Option<i64> {
    let num = onig_get_callout_num_by_tag(reg, tag);
    if num < 1 { return None; }
    onig_get_callout_data(reg, callout_data, num, slot)
}

/// Set callout data by tag name.
pub fn onig_set_callout_data_by_tag(
    reg: &RegexType,
    callout_data: &mut [[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    tag: &[u8],
    slot: i32,
    val: i64,
) -> i32 {
    let num = onig_get_callout_num_by_tag(reg, tag);
    if num < 1 { return ONIGERR_INVALID_CALLOUT_TAG_NAME; }
    onig_set_callout_data(callout_data, num, slot, val)
}

/// Get callout data by tag name without clearing old values.
pub fn onig_get_callout_data_by_tag_dont_clear_old(
    reg: &RegexType,
    callout_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    tag: &[u8],
    slot: i32,
) -> Option<i64> {
    onig_get_callout_data_by_tag(reg, callout_data, tag, slot)
}

// ============================================================================
// Callout Introspection
// (port of C's onig_get_capture_range_in_callout, onig_get_used_stack_size_in_callout)
// ============================================================================

/// Get the capture range for a given memory number during a callout.
/// Returns (begin, end) byte offsets, or ONIG_REGION_NOTPOS if not matched.
///
/// Note: This function requires OnigCalloutArgs to be constructed with stack
/// state from the VM. Currently only available when user callout functions are
/// called from within the match execution.
pub fn onig_get_capture_range_in_callout(
    _args: &OnigCalloutArgs,
    mem_num: i32,
) -> Result<(i32, i32), i32> {
    if mem_num <= 0 {
        return Err(ONIGERR_INVALID_ARGUMENT);
    }
    // Stack-based capture tracking not yet exposed through OnigCalloutArgs.
    // Returns NOTPOS for now.
    Ok((ONIG_REGION_NOTPOS, ONIG_REGION_NOTPOS))
}

/// Get the current used stack size during a callout.
/// Returns (used_num, used_bytes).
///
/// Note: This function requires OnigCalloutArgs to be constructed with stack
/// state from the VM.
pub fn onig_get_used_stack_size_in_callout(
    _args: &OnigCalloutArgs,
) -> (i32, i32) {
    // Stack size tracking not yet exposed through OnigCalloutArgs.
    (0, 0)
}

// ============================================================================
// Builtin Callout Public API
// (port of C's onig_builtin_fail, onig_builtin_mismatch, etc.)
// ============================================================================

pub fn onig_builtin_fail(
    _args: &OnigCalloutArgs,
    _user_data: *mut std::ffi::c_void,
) -> i32 {
    ONIG_CALLOUT_FAIL
}

pub fn onig_builtin_mismatch(
    _args: &OnigCalloutArgs,
    _user_data: *mut std::ffi::c_void,
) -> i32 {
    ONIG_MISMATCH
}

pub fn onig_builtin_error(
    args: &OnigCalloutArgs,
    _user_data: *mut std::ffi::c_void,
) -> i32 {
    let reg = unsafe { &*args.regex };
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            let entry = &ext.callout_list[idx];
            if !entry.args.is_empty() {
                if let CalloutArg::Long(n) = &entry.args[0] {
                    let n = *n as i32;
                    if n >= 0 {
                        return ONIGERR_INVALID_CALLOUT_BODY;
                    }
                    return n;
                }
            }
        }
    }
    ONIGERR_INVALID_CALLOUT_BODY
}

pub fn onig_builtin_count(
    args: &OnigCalloutArgs,
    _user_data: *mut std::ffi::c_void,
) -> i32 {
    if args.callout_data.is_null() { return ONIG_CALLOUT_FAIL; }
    let cd = unsafe { &mut *args.callout_data };
    let num = args.num;
    if num < 1 { return ONIG_CALLOUT_FAIL; }
    let idx = (num - 1) as usize;
    if idx >= cd.len() { return ONIG_CALLOUT_FAIL; }

    let reg = unsafe { &*args.regex };
    let count_type = if let Some(ref ext) = reg.extp {
        if idx < ext.callout_list.len() && !ext.callout_list[idx].args.is_empty() {
            match &ext.callout_list[idx].args[0] {
                CalloutArg::Char(c) => *c,
                _ => b'>',
            }
        } else { b'>' }
    } else { b'>' };

    let is_retraction = args.callout_in == OnigCalloutIn::Retraction;
    let slots = &mut cd[idx];

    if is_retraction {
        if count_type == b'<' { slots[0] += 1; }
        else if count_type == b'X' { slots[0] -= 1; }
        slots[2] += 1;
    } else {
        if count_type != b'<' { slots[0] += 1; }
        slots[1] += 1;
    }

    ONIG_CALLOUT_SUCCESS
}

pub fn onig_builtin_total_count(
    args: &OnigCalloutArgs,
    _user_data: *mut std::ffi::c_void,
) -> i32 {
    // total_count is the same as count but without clearing old data.
    // In Rust, count already doesn't clear old data, so they are equivalent.
    onig_builtin_count(args, _user_data)
}

pub fn onig_builtin_max(
    args: &OnigCalloutArgs,
    _user_data: *mut std::ffi::c_void,
) -> i32 {
    if args.callout_data.is_null() { return ONIG_CALLOUT_FAIL; }
    let cd = unsafe { &mut *args.callout_data };
    let num = args.num;
    if num < 1 { return ONIG_CALLOUT_FAIL; }
    let idx = (num - 1) as usize;
    if idx >= cd.len() { return ONIG_CALLOUT_FAIL; }

    let reg = unsafe { &*args.regex };
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return ONIG_CALLOUT_FAIL,
    };
    if idx >= ext.callout_list.len() { return ONIG_CALLOUT_FAIL; }
    let entry = &ext.callout_list[idx];

    let max_val = if !entry.args.is_empty() {
        resolve_callout_arg(&entry.args[0], &ext.callout_list, cd)
    } else { 0 };

    let count_type = if entry.args.len() > 1 {
        match &entry.args[1] {
            CalloutArg::Char(c) => *c,
            _ => b'>',
        }
    } else { b'>' };

    let is_retraction = args.callout_in == OnigCalloutIn::Retraction;
    let slots = &mut cd[idx];

    if is_retraction {
        if count_type == b'<' {
            if slots[0] >= max_val { return ONIG_CALLOUT_FAIL; }
            slots[0] += 1;
        } else if count_type == b'X' {
            slots[0] -= 1;
        }
    } else {
        if count_type != b'<' {
            if slots[0] >= max_val { return ONIG_CALLOUT_FAIL; }
            slots[0] += 1;
        }
    }

    ONIG_CALLOUT_SUCCESS
}

pub fn onig_builtin_cmp(
    args: &OnigCalloutArgs,
    _user_data: *mut std::ffi::c_void,
) -> i32 {
    if args.callout_data.is_null() { return ONIG_CALLOUT_FAIL; }
    let cd = unsafe { &mut *args.callout_data };
    let num = args.num;
    if num < 1 { return ONIG_CALLOUT_FAIL; }
    let idx = (num - 1) as usize;
    if idx >= cd.len() { return ONIG_CALLOUT_FAIL; }

    let reg = unsafe { &*args.regex };
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return ONIG_CALLOUT_FAIL,
    };
    if idx >= ext.callout_list.len() || ext.callout_list[idx].args.len() < 3 {
        return ONIG_CALLOUT_FAIL;
    }
    let entry = &ext.callout_list[idx];

    let lv = resolve_callout_arg(&entry.args[0], &ext.callout_list, cd);
    let rv = resolve_callout_arg(&entry.args[2], &ext.callout_list, cd);

    // The op is stored in slot 0 after first parse; or read from args[1]
    let op = cd[idx][0];
    if op == 0 {
        // First call: parse the op string from args[1]
        let op_val = match &entry.args[1] {
            CalloutArg::Str(s) => {
                if s == b"==" { 1 }
                else if s == b"!=" { 2 }
                else if s == b"<" { 3 }
                else if s == b">" { 4 }
                else if s == b"<=" { 5 }
                else if s == b">=" { 6 }
                else { return ONIGERR_INVALID_CALLOUT_ARG; }
            }
            _ => return ONIGERR_INVALID_CALLOUT_ARG,
        };
        cd[idx][0] = op_val;
        let result = match op_val {
            1 => lv == rv,
            2 => lv != rv,
            3 => lv < rv,
            4 => lv > rv,
            5 => lv <= rv,
            6 => lv >= rv,
            _ => false,
        };
        if result { ONIG_CALLOUT_SUCCESS } else { ONIG_CALLOUT_FAIL }
    } else {
        let result = match op {
            1 => lv == rv,
            2 => lv != rv,
            3 => lv < rv,
            4 => lv > rv,
            5 => lv <= rv,
            6 => lv >= rv,
            _ => false,
        };
        if result { ONIG_CALLOUT_SUCCESS } else { ONIG_CALLOUT_FAIL }
    }
}

// ============================================================================
// Stack Types (port of StackType / STK_* constants)
// ============================================================================

/// Memory pointer - tracks capture group start/end positions.
/// Corresponds to C's StkPtrType union { StackIndex i; UChar* s; }
#[derive(Clone, Copy, Debug)]
enum MemPtr {
    /// Not yet matched / invalid
    Invalid,
    /// Index into the stack (for push_mem variants that use backtracking)
    StackIdx(usize),
    /// Direct string position (for non-push variants)
    Pos(usize),
}

/// Stack entry - corresponds to C's StackType struct.
/// Uses enum to distinguish entry types instead of C's type field + union.
#[derive(Clone)]
enum StackEntry {
    /// Choice point (STK_ALT / STK_SUPER_ALT) - alternate path for backtracking.
    Alt {
        pcode: usize,    // bytecode index to jump to on backtrack
        pstr: usize,     // string position to restore
        zid: i32,        // remaining count for StepBackNext (-1 = unused)
        is_super: bool,  // true for SUPER_ALT (survives CutToMark)
    },
    /// Capture group start (STK_MEM_START)
    MemStart {
        zid: usize,
        pstr: usize,
        prev_start: MemPtr,
        prev_end: MemPtr,
    },
    /// Capture group end (STK_MEM_END)
    MemEnd {
        zid: usize,
        pstr: usize,
        prev_start: MemPtr,
        prev_end: MemPtr,
    },
    /// Capture group end marker (STK_MEM_END_MARK) - for recursive groups
    MemEndMark {
        zid: usize,
    },
    /// Repeat counter (STK_REPEAT_INC)
    RepeatInc {
        zid: usize,
        count: i32,
    },
    /// Empty check start marker (STK_EMPTY_CHECK_START)
    EmptyCheckStart {
        zid: usize,
        pstr: usize,
    },
    /// Empty check end marker (STK_EMPTY_CHECK_END)
    EmptyCheckEnd {
        zid: usize,
    },
    /// Named checkpoint (STK_MARK) - for lookaheads/lookbehinds
    Mark {
        zid: usize,
        pos: Option<usize>, // saved string position (if save_pos)
    },
    /// Saved value (STK_SAVE_VAL)
    SaveVal {
        zid: usize,
        save_type: SaveType,
        v: usize,
    },
    /// Call frame return address (STK_CALL_FRAME)
    CallFrame {
        ret_addr: usize,
    },
    /// Return marker (STK_RETURN)
    Return,
    /// Callout entry (STK_CALLOUT) - for retraction callbacks
    Callout {
        num: i32,       // callout list index (1-based)
        id: i32,        // builtin id (for name callouts) or ONIG_NON_NAME_ID
    },
    /// Voided entry (STK_VOID) - dead space, skipped during pops
    Void,
}

impl StackEntry {
    /// Returns true if this entry is an ALT (choice point) that stops STACK_POP.
    #[inline]
    fn is_alt(&self) -> bool {
        matches!(self, StackEntry::Alt { .. })
    }

    /// Returns true if this entry needs handling during pop at ALL level.
    #[inline]
    fn is_pop_handled(&self) -> bool {
        matches!(
            self,
            StackEntry::MemStart { .. }
                | StackEntry::MemEnd { .. }
                | StackEntry::RepeatInc { .. }
                | StackEntry::EmptyCheckStart { .. }
                | StackEntry::CallFrame { .. }
                | StackEntry::Return
        )
    }
}

// Sentinel value for the bottom ALT entry's pcode
const FINISH_PCODE: usize = usize::MAX;

// ============================================================================
// MatchArg - runtime match state (port of C's MatchArg)
// ============================================================================

pub struct MatchArg {
    pub options: OnigOptionType,
    pub region: Option<OnigRegion>,
    pub start: usize, // search start position (for \G anchor)
    pub best_len: i32,
    pub best_s: usize,
    // Safety limits
    pub retry_limit_in_match: u64,
    pub retry_limit_in_search: u64,
    pub retry_limit_in_search_counter: u64,
    pub match_stack_limit: u32,
    pub time_limit: u64,  // milliseconds, 0 = unlimited
    /// Lazily-initialized search start time for time-limit checking.
    /// None until the first time check fires, then set to Instant::now().
    time_start: Option<Box<Instant>>,
    // Reusable VM state (avoids heap allocation per match_at call)
    stack: Vec<StackEntry>,
    mem_start_stk: Vec<MemPtr>,
    mem_end_stk: Vec<MemPtr>,
}

const CHECK_TIME_INTERVAL: u64 = 512;

impl MatchArg {
    fn new(
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
    ) -> Self {
        MatchArg {
            options: option | reg.options,
            region,
            start,
            best_len: ONIG_MISMATCH,
            best_s: 0,
            retry_limit_in_match: RETRY_LIMIT_IN_MATCH.load(Ordering::Relaxed),
            retry_limit_in_search: RETRY_LIMIT_IN_SEARCH.load(Ordering::Relaxed),
            retry_limit_in_search_counter: 0,
            match_stack_limit: MATCH_STACK_LIMIT.load(Ordering::Relaxed),
            time_limit: TIME_LIMIT.load(Ordering::Relaxed),
            time_start: None,
            stack: Vec::with_capacity(INIT_MATCH_STACK_SIZE),
            mem_start_stk: Vec::new(),
            mem_end_stk: Vec::new(),
        }
    }

    fn from_param(
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
        mp: &OnigMatchParam,
    ) -> Self {
        MatchArg {
            options: option | reg.options,
            region,
            start,
            best_len: ONIG_MISMATCH,
            best_s: 0,
            retry_limit_in_match: mp.retry_limit_in_match,
            retry_limit_in_search: mp.retry_limit_in_search,
            retry_limit_in_search_counter: 0,
            match_stack_limit: mp.match_stack_limit,
            time_limit: mp.time_limit,
            time_start: None,
            stack: Vec::with_capacity(INIT_MATCH_STACK_SIZE),
            mem_start_stk: Vec::new(),
            mem_end_stk: Vec::new(),
        }
    }

    /// Check if the time limit has been exceeded. Returns true if over limit.
    /// On first call, initializes the start time.
    #[inline]
    fn check_time_limit(&mut self) -> bool {
        if self.time_limit == 0 { return false; }
        let start = self.time_start.get_or_insert_with(|| Box::new(Instant::now()));
        start.elapsed() >= std::time::Duration::from_millis(self.time_limit)
    }
}

// ============================================================================
// Stack limit check
// ============================================================================

/// Check stack size against match_stack_limit. Returns Err with error code if exceeded.
#[inline]
fn check_stack_limit(stack_len: usize, limit: u32) -> Result<(), i32> {
    if limit != 0 && stack_len >= limit as usize {
        return Err(ONIGERR_MATCH_STACK_LIMIT_OVER);
    }
    Ok(())
}

// ============================================================================
// Stack operations (port of STACK_PUSH_* / STACK_POP macros)
// ============================================================================

/// Pop stack entries until an ALT (choice point) is found.
/// Restores mem_start_stk/mem_end_stk as needed based on pop_level.
/// Handles callout retraction when reg/callout_data are provided.
/// Returns Some((pcode, pstr, zid)) from the ALT entry, or None if stack is empty.
fn stack_pop(
    stack: &mut Vec<StackEntry>,
    pop_level: StackPopLevel,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
    reg: &RegexType,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
) -> Option<(usize, usize, i32)> {
    loop {
        let entry = stack.pop()?;
        match entry {
            StackEntry::Alt { pcode, pstr, zid, .. } => {
                return Some((pcode, pstr, zid));
            }
            _ => match pop_level {
                StackPopLevel::Free => {
                    // Skip non-ALT entries without restoration
                }
                StackPopLevel::MemStart => {
                    // Restore MEM_START entries only
                    if let StackEntry::MemStart {
                        zid,
                        prev_start,
                        prev_end,
                        ..
                    } = &entry
                    {
                        mem_start_stk[*zid] = *prev_start;
                        mem_end_stk[*zid] = *prev_end;
                    }
                }
                StackPopLevel::All => {
                    // Restore all handled entries
                    match &entry {
                        StackEntry::MemStart {
                            zid,
                            prev_start,
                            prev_end,
                            ..
                        } => {
                            mem_start_stk[*zid] = *prev_start;
                            mem_end_stk[*zid] = *prev_end;
                        }
                        StackEntry::MemEnd {
                            zid,
                            prev_start,
                            prev_end,
                            ..
                        } => {
                            mem_start_stk[*zid] = *prev_start;
                            mem_end_stk[*zid] = *prev_end;
                        }
                        StackEntry::Callout { num, id } => {
                            // Retraction callback
                            run_builtin_callout_retraction(reg, *num, *id, callout_data);
                        }
                        // RepeatInc, EmptyCheckStart, CallFrame, Return:
                        // handled implicitly (popping removes them)
                        _ => {}
                    }
                }
            },
        }
    }
}

/// Pop stack entries until a Mark with matching zid is found (STACK_POP_TO_MARK).
/// Removes ALL entries. Restores mem_start_stk/mem_end_stk along the way.
/// Returns the saved position from the Mark entry (if any).
fn stack_pop_to_mark(
    stack: &mut Vec<StackEntry>,
    mark_id: usize,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
) -> Option<usize> {
    loop {
        let entry = stack.pop()?;
        match &entry {
            StackEntry::Mark { zid, pos } if *zid == mark_id => {
                return *pos;
            }
            StackEntry::MemStart {
                zid,
                prev_start,
                prev_end,
                ..
            } => {
                mem_start_stk[*zid] = *prev_start;
                mem_end_stk[*zid] = *prev_end;
            }
            StackEntry::MemEnd {
                zid,
                prev_start,
                prev_end,
                ..
            } => {
                mem_start_stk[*zid] = *prev_start;
                mem_end_stk[*zid] = *prev_end;
            }
            _ => {}
        }
    }
}

/// Void stack entries until a Mark with matching zid is found (STACK_TO_VOID_TO_MARK).
/// Only voids "void targets" (regular Alt, EmptyCheckStart, Mark) by setting them to Void.
/// Preserves non-void targets (SuperAlt, SaveVal, MemStart, MemEnd, RepeatInc, etc.) in place.
/// Void stack entries from top to the Mark with matching id (C: STACK_TO_VOID_TO_MARK).
/// Returns the mark's saved position. Voids regular Alt and EmptyCheckStart entries,
/// but preserves Super Alt entries and Marks with different IDs.
fn stack_void_to_mark(
    stack: &mut Vec<StackEntry>,
    mark_id: usize,
) -> Option<usize> {
    let mut i = stack.len();
    while i > 0 {
        i -= 1;
        // Check if this is the target Mark
        if let StackEntry::Mark { zid, pos } = &stack[i] {
            if *zid == mark_id {
                let saved_pos = *pos;
                stack[i] = StackEntry::Void;
                return saved_pos;
            }
            // Different id mark: don't void, just skip
            continue;
        }
        // Void targets: regular Alt and EmptyCheckStart
        // Super Alt (is_super=true) is NOT voided — it survives cuts
        let is_void_target = matches!(
            &stack[i],
            StackEntry::Alt { is_super: false, .. }
            | StackEntry::EmptyCheckStart { .. }
        );
        if is_void_target {
            stack[i] = StackEntry::Void;
        }
    }
    None
}

/// Search backwards through the stack for the most recent RepeatInc with matching zid.
/// Returns the count from that entry.
fn stack_get_repeat_count(stack: &[StackEntry], zid: usize) -> i32 {
    for entry in stack.iter().rev() {
        if let StackEntry::RepeatInc {
            zid: id,
            count,
            ..
        } = entry
        {
            if *id == zid {
                return *count;
            }
        }
    }
    0
}

/// Check if the empty check for the given zid matches the same string position.
/// Returns true if the string position hasn't advanced (empty match).
fn stack_empty_check(stack: &[StackEntry], zid: usize, s: usize) -> bool {
    for entry in stack.iter().rev() {
        if let StackEntry::EmptyCheckStart { zid: id, pstr } = entry {
            if *id == zid {
                return *pstr == s;
            }
        }
    }
    false
}

/// Memory-aware empty check. Returns true only if position is same AND no capture
/// groups (indicated by empty_status_mem) have changed since the EmptyCheckStart.
/// Mirrors C's STACK_EMPTY_CHECK_MEM.
/// Check if a quantifier iteration was empty (position unchanged).
/// Returns: false = not empty (position changed or captures changed),
///          true = truly empty (pos same AND captures same)
fn stack_empty_check_mem(
    stack: &[StackEntry],
    zid: usize,
    s: usize,
    empty_status_mem: u32,
    _reg: &RegexType,
    _mem_start_stk: &[MemPtr],
    _mem_end_stk: &[MemPtr],
) -> bool {
    // Find the EmptyCheckStart entry
    let mut klow_idx = None;
    for (i, entry) in stack.iter().enumerate().rev() {
        if let StackEntry::EmptyCheckStart { zid: id, pstr } = entry {
            if *id == zid {
                if *pstr != s {
                    return false; // position changed → not empty
                }
                klow_idx = Some(i);
                break;
            }
        }
    }

    let klow_idx = match klow_idx {
        Some(i) => i,
        None => return false,
    };

    // Position is the same. Check if any capture groups changed.
    let mut ms = empty_status_mem as u32;
    for k_idx in (klow_idx + 1..stack.len()).rev() {
        if let StackEntry::MemEnd { zid: mem_zid, pstr: end_pstr, .. } = &stack[k_idx] {
            if ms & (1u32 << *mem_zid) != 0 {
                // Found a MemEnd for a tracked group. Check if its value differs
                // from the previous iteration's value.
                // Look for the corresponding MemStart between klow and this MemEnd.
                for kk_idx in klow_idx + 1..k_idx {
                    if let StackEntry::MemStart { zid: start_zid, prev_end, .. } = &stack[kk_idx] {
                        if *start_zid == *mem_zid {
                            // Check if prev_end was invalid (group wasn't captured before)
                            match prev_end {
                                MemPtr::Invalid => {
                                    // Previously not captured, now captured → not empty
                                    return false;
                                }
                                MemPtr::Pos(prev_pos) => {
                                    if *prev_pos != *end_pstr {
                                        return false; // end position changed
                                    }
                                }
                                MemPtr::StackIdx(si) => {
                                    if let StackEntry::MemEnd { pstr: prev_pstr, .. } = &stack[*si] {
                                        if *prev_pstr != *end_pstr {
                                            return false;
                                        }
                                    }
                                }
                            }
                            ms &= !(1u32 << *mem_zid);
                            break;
                        }
                    }
                }
                if ms == 0 { break; }
            }
        }
    }

    true // position same AND no captures changed → truly empty
}

/// Get the saved value for a given save_type and zid from the stack.
/// Search stack for last SaveVal by type only (C: STACK_GET_SAVE_VAL_TYPE_LAST)
fn stack_get_save_val_type_last(
    stack: &[StackEntry],
    save_type: SaveType,
) -> Option<usize> {
    for entry in stack.iter().rev() {
        if let StackEntry::SaveVal {
            save_type: st,
            v,
            ..
        } = entry
        {
            if *st == save_type {
                return Some(*v);
            }
        }
    }
    None
}

/// Search stack for last SaveVal by type AND id (C: STACK_GET_SAVE_VAL_TYPE_LAST_ID)
fn stack_get_save_val_last(
    stack: &[StackEntry],
    save_type: SaveType,
    zid: usize,
) -> Option<usize> {
    for entry in stack.iter().rev() {
        if let StackEntry::SaveVal {
            zid: id,
            save_type: st,
            v,
        } = entry
        {
            if *id == zid && *st == save_type {
                return Some(*v);
            }
        }
    }
    None
}

/// Build capture history tree from the match stack.
/// Mirrors C's make_capture_history_tree().
/// Returns 0 on child node ending, 1 on root node ending, or negative error.
fn make_capture_history_tree(
    node: &mut OnigCaptureTreeNode,
    k: &mut usize,
    stack: &[StackEntry],
    stk_top: usize,
    reg: &RegexType,
) -> i32 {
    while *k < stk_top {
        match &stack[*k] {
            StackEntry::MemStart { zid, pstr, .. } => {
                let n = *zid;
                if n <= ONIG_MAX_CAPTURE_HISTORY_GROUP
                    && mem_status_at(reg.capture_history, n)
                {
                    let mut child = Box::new(OnigCaptureTreeNode::new());
                    child.group = n as i32;
                    child.beg = *pstr as i32;
                    node.add_child(child);
                    let child_idx = node.childs.len() - 1;
                    *k += 1;
                    let r = make_capture_history_tree(
                        &mut node.childs[child_idx], k, stack, stk_top, reg,
                    );
                    if r < 0 { return r; }
                    // After recursive call, k points to the matching MemEnd
                    if let StackEntry::MemEnd { pstr: end_pstr, .. } = &stack[*k] {
                        node.childs[child_idx].end = *end_pstr as i32;
                    }
                }
            }
            StackEntry::MemEnd { zid, pstr, .. } => {
                if *zid == node.group as usize {
                    node.end = *pstr as i32;
                    return 0;
                }
            }
            _ => {}
        }
        *k += 1;
    }
    1 // root node ending
}

/// Get the string position where a capture group starts.
fn get_mem_start(
    reg: &RegexType,
    stack: &[StackEntry],
    mem_start_stk: &[MemPtr],
    idx: usize,
) -> Option<usize> {
    match mem_start_stk[idx] {
        MemPtr::Invalid => None,
        MemPtr::Pos(pos) => Some(pos),
        MemPtr::StackIdx(si) => {
            if let StackEntry::MemStart { pstr, .. } = &stack[si] {
                Some(*pstr)
            } else {
                None
            }
        }
    }
}

/// Get the string position where a capture group ends.
fn get_mem_end(
    reg: &RegexType,
    stack: &[StackEntry],
    mem_end_stk: &[MemPtr],
    idx: usize,
) -> Option<usize> {
    match mem_end_stk[idx] {
        MemPtr::Invalid => None,
        MemPtr::Pos(pos) => Some(pos),
        MemPtr::StackIdx(si) => {
            if let StackEntry::MemEnd { pstr, .. } = &stack[si] {
                Some(*pstr)
            } else {
                None
            }
        }
    }
}

/// Level-aware scan for the matching MEM_START of a recursive capture group.
/// Mirrors C's STACK_GET_MEM_START macro — counts MemEnd/MemEndMark entries
/// to track nesting level, finds MemStart at level 0.
fn stack_get_mem_start_for_rec(
    stack: &[StackEntry],
    mem: usize,
    push_mem_start: u32,
) -> (MemPtr, usize) {
    let mut level: i32 = 0;
    let mut k = stack.len();
    while k > 0 {
        k -= 1;
        match &stack[k] {
            StackEntry::MemEnd { zid, .. } | StackEntry::MemEndMark { zid } if *zid == mem => {
                level += 1;
            }
            StackEntry::MemStart { zid, pstr, .. } if *zid == mem => {
                if level == 0 {
                    // Found matching start at correct level
                    if mem_status_at(push_mem_start, mem) {
                        return (MemPtr::StackIdx(k), *pstr);
                    } else {
                        return (MemPtr::Pos(*pstr), *pstr);
                    }
                }
                level -= 1;
            }
            _ => {}
        }
    }
    (MemPtr::Invalid, 0)
}

/// Match a backref at a specific nesting level in the recursion stack.
/// Walks the stack backwards counting CallFrame/Return to find the right level,
/// then matches the captured text at that level against current position.
fn backref_match_at_nested_level(
    reg: &RegexType,
    stack: &[StackEntry],
    ignore_case: bool,
    case_fold_flag: OnigCaseFoldType,
    nest: i32,
    mem_num: i32,
    mems: &[i32],
    s: &mut usize,
    str_data: &[u8],
    end: usize,
) -> bool {
    let mut level: i32 = 0;
    let mut pend: Option<usize> = None;
    let mut k = stack.len();

    while k > 0 {
        k -= 1;
        match &stack[k] {
            StackEntry::CallFrame { .. } => {
                level -= 1;
            }
            StackEntry::Return => {
                level += 1;
            }
            StackEntry::MemStart { zid, pstr, .. } if level == nest => {
                if mem_is_in_mems(*zid, mem_num, mems) {
                    if let Some(pe) = pend {
                        let pstart = *pstr;
                        let cap_len = pe - pstart;
                        if cap_len > end - *s {
                            return false;
                        }
                        if ignore_case {
                            if !string_cmp_ic(reg.enc, case_fold_flag, str_data, pstart, s, cap_len) {
                                return false;
                            }
                        } else {
                            if str_data[pstart..pe] != str_data[*s..*s + cap_len] {
                                return false;
                            }
                            *s += cap_len;
                        }
                        return true;
                    }
                }
            }
            StackEntry::MemEnd { zid, pstr, .. } if level == nest => {
                if mem_is_in_mems(*zid, mem_num, mems) {
                    pend = Some(*pstr);
                }
            }
            _ => {}
        }
    }
    false
}

/// Check if a backref capture exists at a specific nesting level.
fn backref_check_at_nested_level(
    stack: &[StackEntry],
    nest: i32,
    mem_num: i32,
    mems: &[i32],
) -> bool {
    let mut level: i32 = 0;
    let mut k = stack.len();

    while k > 0 {
        k -= 1;
        match &stack[k] {
            StackEntry::CallFrame { .. } => { level -= 1; }
            StackEntry::Return => { level += 1; }
            StackEntry::MemStart { zid, .. } if level == nest => {
                if mem_is_in_mems(*zid, mem_num, mems) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[inline]
fn mem_is_in_mems(mem: usize, num: i32, mems: &[i32]) -> bool {
    for i in 0..num as usize {
        if mem == mems[i] as usize {
            return true;
        }
    }
    false
}


// ============================================================================
// Helper functions
// ============================================================================

/// Check if a byte is a word character (ASCII-only).
#[inline]
fn is_word_ascii(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Check if the character at position s is a word character (encoding-aware).
/// Uses the encoding's Unicode-aware is_code_ctype for multi-byte encodings.
#[inline]
fn is_word_char_at(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    if s >= end {
        return false;
    }
    let code = enc.mbc_to_code(&str_data[s..], end);
    enc.is_code_ctype(code, ONIGENC_CTYPE_WORD)
}

/// Check if position is a word character in ASCII-aware mode.
/// mode == 0: full Unicode word check, mode != 0: ASCII-only word check.
#[inline]
fn is_word_char_ascii_mode(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize, mode: ModeType) -> bool {
    if mode == 0 {
        return is_word_char_at(enc, str_data, s, end);
    }
    // ASCII-only: only a-z, A-Z, 0-9, _ are word characters
    if s >= end {
        return false;
    }
    let code = enc.mbc_to_code(&str_data[s..], end);
    if code > 0x7F {
        return false;
    }
    matches!(code as u8, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_')
}

/// Get the start of the previous character (left_adjust_char_head).
#[inline]
fn prev_char_head(enc: OnigEncoding, start: usize, s: usize, str_data: &[u8]) -> usize {
    if s <= start {
        return s;
    }
    enc.left_adjust_char_head(start, s - 1, str_data)
}

/// Check word boundary at position s (encoding-aware, mode-aware).
/// mode == 0: Unicode word, mode != 0: ASCII-only word.
fn is_word_boundary(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize, mode: ModeType) -> bool {
    let at_start = s == 0;
    let at_end = s >= end;

    if at_start && at_end {
        return false;
    }
    if at_start {
        return is_word_char_ascii_mode(enc, str_data, s, end, mode);
    }
    if at_end {
        let prev = prev_char_head(enc, 0, s, str_data);
        return is_word_char_ascii_mode(enc, str_data, prev, end, mode);
    }

    let prev = prev_char_head(enc, 0, s, str_data);
    let prev_word = is_word_char_ascii_mode(enc, str_data, prev, end, mode);
    let curr_word = is_word_char_ascii_mode(enc, str_data, s, end, mode);
    prev_word != curr_word
}

/// Check if position s is at the start of a word (encoding-aware, mode-aware).
fn is_word_begin(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize, mode: ModeType) -> bool {
    if s >= end {
        return false;
    }
    if !is_word_char_ascii_mode(enc, str_data, s, end, mode) {
        return false;
    }
    if s == 0 {
        return true;
    }
    let prev = prev_char_head(enc, 0, s, str_data);
    !is_word_char_ascii_mode(enc, str_data, prev, end, mode)
}

/// Check if position s is at the end of a word (encoding-aware, mode-aware).
fn is_word_end(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize, mode: ModeType) -> bool {
    if s == 0 {
        return false;
    }
    let prev = prev_char_head(enc, 0, s, str_data);
    if !is_word_char_ascii_mode(enc, str_data, prev, end, mode) {
        return false;
    }
    if s >= end {
        return true;
    }
    !is_word_char_ascii_mode(enc, str_data, s, end, mode)
}

/// Check if a code point is in a multi-byte range table.
/// The table format is: data[0] = n (range count), followed by n pairs of (from, to).
/// Binary search, matching C's onig_is_in_code_range exactly.
#[inline]
pub(crate) fn is_in_code_range(data: &[u32], code: OnigCodePoint) -> bool {
    if data.is_empty() {
        return false;
    }
    let n = data[0] as usize;
    let ranges = &data[1..];

    let mut low: usize = 0;
    let mut high: usize = n;
    while low < high {
        let x = (low + high) >> 1;
        if code > ranges[x * 2 + 1] {
            low = x + 1;
        } else {
            high = x;
        }
    }

    low < n && code >= ranges[low * 2]
}

/// Check if a code point is in a multi-byte range table stored as raw bytes.
/// Used at compile time (regparse) where data is still in BBuf byte format.
pub(crate) fn is_in_code_range_bytes(mb: &[u8], code: OnigCodePoint) -> bool {
    if mb.len() < 4 {
        return false;
    }
    let n = u32::from_ne_bytes([mb[0], mb[1], mb[2], mb[3]]) as usize;
    if mb.len() < 4 + n * 8 {
        return false;
    }

    let mut low: usize = 0;
    let mut high: usize = n;
    while low < high {
        let x = (low + high) >> 1;
        let off = 4 + x * 8;
        let range_high = u32::from_ne_bytes([mb[off + 4], mb[off + 5], mb[off + 6], mb[off + 7]]);
        if code > range_high {
            low = x + 1;
        } else {
            high = x;
        }
    }

    if low < n {
        let off = 4 + low * 8;
        let range_low = u32::from_ne_bytes([mb[off], mb[off + 1], mb[off + 2], mb[off + 3]]);
        code >= range_low
    } else {
        false
    }
}

/// Get the character length at position s for the given encoding.
#[inline]
fn enclen(enc: OnigEncoding, str_data: &[u8], s: usize) -> usize {
    if s >= str_data.len() {
        1
    } else {
        enc.mbc_enc_len(&str_data[s..])
    }
}

/// Case-insensitive string comparison using encoding-aware case folding.
/// Compares `mblen` bytes starting at `s1_pos` with bytes starting at `*s2_pos`.
/// Advances `*s2_pos` past consumed bytes on success. Returns true if equal.
fn string_cmp_ic(
    enc: OnigEncoding,
    case_fold_flag: OnigCaseFoldType,
    data: &[u8],
    s1_pos: usize,
    s2_pos: &mut usize,
    mblen: usize,
) -> bool {
    let mut buf1 = [0u8; ONIGENC_MBC_CASE_FOLD_MAXLEN];
    let mut buf2 = [0u8; ONIGENC_MBC_CASE_FOLD_MAXLEN];
    let end1 = s1_pos + mblen;
    let end2 = *s2_pos + mblen;
    let mut p1 = s1_pos;
    let mut p2 = *s2_pos;

    while p1 < end1 {
        let len1 = enc.mbc_case_fold(case_fold_flag, &mut p1, end1, data, &mut buf1);
        let len2 = enc.mbc_case_fold(case_fold_flag, &mut p2, end2, data, &mut buf2);
        if len1 != len2 {
            return false;
        }
        if buf1[..len1 as usize] != buf2[..len2 as usize] {
            return false;
        }
        if p2 >= end2 {
            if p1 < end1 {
                return false;
            }
            break;
        }
    }

    *s2_pos = p2;
    true
}

// ============================================================================
// Builtin callout functions
// ============================================================================

/// Run a builtin callout in the progress direction.
/// Returns ONIG_CALLOUT_SUCCESS or ONIG_CALLOUT_FAIL.
fn run_builtin_callout(
    reg: &RegexType,
    num: i32,
    _id: i32,
    is_retraction: bool,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
) -> i32 {
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return ONIG_CALLOUT_SUCCESS,
    };
    if num < 1 || (num as usize) > ext.callout_list.len() {
        return ONIG_CALLOUT_SUCCESS;
    }
    let entry = &ext.callout_list[(num - 1) as usize];
    let idx = (num - 1) as usize;

    match entry.builtin_id {
        CALLOUT_BUILTIN_MAX => {
            let max_val = resolve_callout_arg(&entry.args[0], &ext.callout_list, callout_data);
            builtin_max(entry, is_retraction, &mut callout_data[idx], max_val)
        }
        CALLOUT_BUILTIN_COUNT => builtin_count(entry, is_retraction, &mut callout_data[idx]),
        CALLOUT_BUILTIN_CMP => {
            let lv = resolve_callout_arg(&entry.args[0], &ext.callout_list, callout_data);
            let rv = resolve_callout_arg(&entry.args[2], &ext.callout_list, callout_data);
            builtin_cmp(entry, &mut callout_data[idx], lv, rv)
        }
        _ => ONIG_CALLOUT_SUCCESS,
    }
}

/// Run a builtin callout in the retraction direction (called from stack_pop).
fn run_builtin_callout_retraction(
    reg: &RegexType,
    num: i32,
    _id: i32,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
) {
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return,
    };
    if num < 1 || (num as usize) > ext.callout_list.len() {
        return;
    }
    let entry = &ext.callout_list[(num - 1) as usize];
    let slots = &mut callout_data[(num - 1) as usize];

    match entry.builtin_id {
        CALLOUT_BUILTIN_MAX => {
            // Retraction for MAX: same logic as progress but with is_retraction=true
            // We can't pass the full callout_data for tag resolution here since we already
            // have a mutable borrow. But MAX retraction only reads slots[0] (its own counter).
            let count_type = if entry.args.len() > 1 {
                match &entry.args[1] {
                    CalloutArg::Char(c) => *c,
                    _ => b'X',
                }
            } else {
                b'X'
            };
            if count_type == b'<' {
                // retraction + '<': increment and check
                let max_val = resolve_callout_arg(&entry.args[0], &[], &[]);
                if slots[0] >= max_val {
                    // fail — but retraction result is ignored per C code
                } else {
                    slots[0] += 1;
                }
            } else if count_type == b'X' {
                // retraction + 'X': decrement
                slots[0] -= 1;
            }
            // retraction + '>': no-op
        }
        CALLOUT_BUILTIN_COUNT => {
            let count_type = if !entry.args.is_empty() {
                match &entry.args[0] {
                    CalloutArg::Char(c) => *c,
                    _ => b'>',
                }
            } else {
                b'>'
            };
            // Retraction: slot 2
            if count_type == b'<' {
                slots[0] += 1;
            } else if count_type == b'X' {
                slots[0] -= 1;
            }
            // slot 2 (retraction counter) increment
            slots[2] += 1;
        }
        _ => {}
    }
}

/// Resolve a callout argument: Long value directly or Tag → lookup slot[0] of tagged callout.
fn resolve_callout_arg(
    arg: &CalloutArg,
    ext_callout_list: &[CalloutListEntry],
    all_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
) -> i64 {
    match arg {
        CalloutArg::Long(n) => *n,
        CalloutArg::Tag(tag) => {
            for (i, e) in ext_callout_list.iter().enumerate() {
                if let Some(ref t) = e.tag {
                    if t == tag {
                        if i < all_data.len() {
                            return all_data[i][0];
                        }
                    }
                }
            }
            0
        }
        _ => 0,
    }
}

fn builtin_max(
    entry: &CalloutListEntry,
    is_retraction: bool,
    slots: &mut [i64; ONIG_CALLOUT_DATA_SLOT_NUM],
    max_val: i64,
) -> i32 {
    // slots[0] = current count
    let count_type = if entry.args.len() > 1 {
        match &entry.args[1] {
            CalloutArg::Char(c) => *c,
            _ => b'X',
        }
    } else {
        b'X'
    };

    if is_retraction {
        if count_type == b'<' {
            if slots[0] >= max_val {
                return ONIG_CALLOUT_FAIL;
            }
            slots[0] += 1;
        } else if count_type == b'X' {
            slots[0] -= 1;
        }
    } else {
        if count_type != b'<' {
            if slots[0] >= max_val {
                return ONIG_CALLOUT_FAIL;
            }
            slots[0] += 1;
        }
    }

    ONIG_CALLOUT_SUCCESS
}

fn builtin_count(
    entry: &CalloutListEntry,
    is_retraction: bool,
    slots: &mut [i64; ONIG_CALLOUT_DATA_SLOT_NUM],
) -> i32 {
    // slots[0] = main counter (progress - retraction adjusted)
    // slots[1] = progress counter
    // slots[2] = retraction counter
    let count_type = if !entry.args.is_empty() {
        match &entry.args[0] {
            CalloutArg::Char(c) => *c,
            _ => b'>',
        }
    } else {
        b'>'
    };

    if is_retraction {
        if count_type == b'<' {
            slots[0] += 1;
        } else if count_type == b'X' {
            slots[0] -= 1;
        }
        // slot 2 (retraction counter)
        slots[2] += 1;
    } else {
        if count_type != b'<' {
            slots[0] += 1;
        }
        // slot 1 (progress counter)
        slots[1] += 1;
    }

    ONIG_CALLOUT_SUCCESS
}

fn builtin_cmp(
    entry: &CalloutListEntry,
    slots: &mut [i64; ONIG_CALLOUT_DATA_SLOT_NUM],
    lv: i64,
    rv: i64,
) -> i32 {
    // CMP is progress-only
    if entry.args.len() < 3 {
        return ONIG_CALLOUT_FAIL;
    }

    // Parse op on first call, cache in slots[0]
    let op = if slots[3] == 0 {
        // First call: parse operator string
        let op_val = match &entry.args[1] {
            CalloutArg::Str(s) => parse_cmp_op(s),
            CalloutArg::Char(c) => parse_cmp_op(&[*c]),
            _ => return ONIG_CALLOUT_FAIL,
        };
        slots[3] = 1; // mark as initialized
        slots[4] = op_val as i64;
        op_val
    } else {
        slots[4] as i32
    };

    let result = match op {
        0 => lv == rv, // ==
        1 => lv != rv, // !=
        2 => lv < rv,  // <
        3 => lv > rv,  // >
        4 => lv <= rv, // <=
        5 => lv >= rv, // >=
        _ => false,
    };

    if result { ONIG_CALLOUT_SUCCESS } else { ONIG_CALLOUT_FAIL }
}

fn parse_cmp_op(s: &[u8]) -> i32 {
    match s {
        b"==" => 0,
        b"!=" => 1,
        b"<" => 2,
        b">" => 3,
        b"<=" => 4,
        b">=" => 5,
        _ => -1,
    }
}

// ============================================================================
// match_at - the core VM executor (port of C's match_at function)
// ============================================================================

/// Execute the bytecode VM starting at position `sstart` in the string.
/// Returns match length on success, ONIG_MISMATCH (-1) on failure.
///
/// Parameters match C's match_at(reg, str, end, in_right_range, sstart, msa):
/// - reg: compiled regex with bytecode in reg.ops
/// - str_data: the input string bytes
/// - end: end position in str_data
/// - in_right_range: right boundary for matching
/// - sstart: position to start matching at
/// - msa: mutable match state (options, region, etc.)
fn match_at(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    in_right_range: usize,
    sstart: usize,
    msa: &mut MatchArg,
) -> i32 {
    let mut p: usize = 0; // bytecode index into reg.ops
    let mut s: usize = sstart; // current string position
    let mut right_range: usize = in_right_range;
    let pop_level = reg.stack_pop_level;
    let num_mem = reg.num_mem as usize;
    let enc = reg.enc;
    let options = msa.options;

    // Reuse stack and capture-group arrays from MatchArg (avoids heap alloc per call)
    let mut stack = std::mem::take(&mut msa.stack);
    stack.clear();
    let mut mem_start_stk = std::mem::take(&mut msa.mem_start_stk);
    mem_start_stk.clear();
    mem_start_stk.resize(num_mem + 1, MemPtr::Invalid);
    let mut mem_end_stk = std::mem::take(&mut msa.mem_end_stk);
    mem_end_stk.clear();
    mem_end_stk.resize(num_mem + 1, MemPtr::Invalid);

    let mut keep: usize = sstart;
    let mut best_len: i32 = ONIG_MISMATCH;
    let mut last_alt_zid: i32 = -1;

    // Safety limits
    let retry_limit_in_match = msa.retry_limit_in_match;
    let mut retry_in_match_counter: u64 = 0;
    let match_stack_limit = msa.match_stack_limit;
    let time_limit_ms = msa.time_limit;

    // Callout data: per-callout mutable slots (indexed by callout num - 1)
    let callout_count = reg.extp.as_ref().map_or(0, |e| e.callout_num as usize);
    let mut callout_data: Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]> = vec![[0i64; ONIG_CALLOUT_DATA_SLOT_NUM]; callout_count];

    // Push bottom sentinel (like C's STACK_PUSH_BOTTOM with FinishCode)
    stack.push(StackEntry::Alt {
        pcode: FINISH_PCODE,
        pstr: 0,
        zid: -1,
        is_super: false,
    });

    // ---- Main dispatch loop ----
    loop {
        if p >= reg.ops.len() {
            break;
        }

        // Stack limit check (checked once per opcode, like C's STACK_PUSH macro)
        if match_stack_limit != 0 && stack.len() >= match_stack_limit as usize {
            best_len = ONIGERR_MATCH_STACK_LIMIT_OVER;
            break;
        }

        let opcode = reg.ops[p].opcode;
        let mut goto_fail = false;

        match opcode {
            // ================================================================
            // OP_FINISH - reached bottom sentinel, return result
            // ================================================================
            OpCode::Finish => {
                break;
            }

            // ================================================================
            // OP_END - successful match, populate region
            // ================================================================
            OpCode::End => {
                // Check MATCH_WHOLE_STRING option
                if opton_match_whole_string(options) && s < end {
                    goto_fail = true;
                } else {
                    let n = (s - sstart) as i32;
                    if n == 0 && opton_find_not_empty(options) {
                        goto_fail = true;
                    } else if n > best_len {
                        best_len = n;

                        // Populate region with capture groups
                        if let Some(ref mut region) = msa.region {
                            region.resize(num_mem as i32 + 1);
                            region.beg[0] = (keep - 0) as i32; // offset from str start
                            region.end[0] = s as i32;

                            for i in 1..=num_mem {
                                if let Some(mem_end) =
                                    get_mem_end(reg, &stack, &mem_end_stk, i)
                                {
                                    let mem_start =
                                        get_mem_start(reg, &stack, &mem_start_stk, i);
                                    region.beg[i] = mem_start
                                        .map(|v| v as i32)
                                        .unwrap_or(ONIG_REGION_NOTPOS);
                                    region.end[i] = mem_end as i32;
                                } else {
                                    region.beg[i] = ONIG_REGION_NOTPOS;
                                    region.end[i] = ONIG_REGION_NOTPOS;
                                }
                            }

                            // Build capture history tree
                            if USE_CAPTURE_HISTORY && reg.capture_history != 0 {
                                let node = if region.history_root.is_none() {
                                    region.history_root = Some(Box::new(OnigCaptureTreeNode::new()));
                                    region.history_root.as_mut().unwrap()
                                } else {
                                    let root = region.history_root.as_mut().unwrap();
                                    root.clear();
                                    root
                                };
                                node.group = 0;
                                node.beg = keep as i32;
                                node.end = s as i32;
                                let mut stkp = 0usize;
                                let stk_top = stack.len();
                                let r = make_capture_history_tree(node, &mut stkp, &stack, stk_top, reg);
                                if r < 0 {
                                    best_len = r;
                                    break;
                                }
                            }
                        }

                        // For non-FIND_LONGEST, return immediately
                        if !opton_find_longest(options) {
                            msa.stack = stack;
                            msa.mem_start_stk = mem_start_stk;
                            msa.mem_end_stk = mem_end_stk;
                            return best_len;
                        }

                        // FIND_LONGEST: save best and continue searching
                        msa.best_len = best_len;
                        msa.best_s = sstart;
                        goto_fail = true; // backtrack to try longer matches
                    } else {
                        // FIND_LONGEST but shorter/equal match: backtrack for more
                        goto_fail = true;
                    }
                }
            }

            // ================================================================
            // OP_STR1..STR5 - match 1-5 literal bytes
            // ================================================================
            OpCode::Str1 => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s] {
                        goto_fail = true;
                    } else {
                        s += 1;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str2 => {
                if right_range.saturating_sub(s) < 2 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s] || exact[1] != str_data[s + 1] {
                        goto_fail = true;
                    } else {
                        s += 2;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str3 => {
                if right_range.saturating_sub(s) < 3 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s]
                        || exact[1] != str_data[s + 1]
                        || exact[2] != str_data[s + 2]
                    {
                        goto_fail = true;
                    } else {
                        s += 3;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str4 => {
                if right_range.saturating_sub(s) < 4 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s]
                        || exact[1] != str_data[s + 1]
                        || exact[2] != str_data[s + 2]
                        || exact[3] != str_data[s + 3]
                    {
                        goto_fail = true;
                    } else {
                        s += 4;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Str5 => {
                if right_range.saturating_sub(s) < 5 {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = reg.ops[p].payload {
                    if exact[0] != str_data[s]
                        || exact[1] != str_data[s + 1]
                        || exact[2] != str_data[s + 2]
                        || exact[3] != str_data[s + 3]
                        || exact[4] != str_data[s + 4]
                    {
                        goto_fail = true;
                    } else {
                        s += 5;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::StrN => {
                if let OperationPayload::ExactN { s: ref exact, n } = reg.ops[p].payload {
                    let n = n as usize;
                    if right_range.saturating_sub(s) < n {
                        goto_fail = true;
                    } else if str_data[s..s + n] != exact[..n] {
                        goto_fail = true;
                    } else {
                        s += n;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // MB string opcodes (for multibyte encodings)
            OpCode::StrMb2n1 | OpCode::StrMb2n2 | OpCode::StrMb2n3 | OpCode::StrMb2n
            | OpCode::StrMb3n | OpCode::StrMbn => {
                // Multi-byte string comparison. ExactLenN.n = total byte count.
                if let OperationPayload::ExactLenN { s: ref exact, n, .. } =
                    reg.ops[p].payload
                {
                    let byte_len = n as usize;
                    if right_range.saturating_sub(s) < byte_len {
                        goto_fail = true;
                    } else if str_data[s..s + byte_len] != exact[..byte_len] {
                        goto_fail = true;
                    } else {
                        s += byte_len;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_CCLASS / OP_CCLASS_NOT - character class matching
            // ================================================================
            OpCode::CClass => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if let OperationPayload::CClass { ref bsp } = reg.ops[p].payload {
                    if !bitset_at(bsp, str_data[s] as usize) {
                        goto_fail = true;
                    } else {
                        s += enclen(enc, str_data, s);
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassNot => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if let OperationPayload::CClass { ref bsp } = reg.ops[p].payload {
                    if bitset_at(bsp, str_data[s] as usize) {
                        goto_fail = true;
                    } else {
                        s += enclen(enc, str_data, s);
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // MB character class (multibyte)
            OpCode::CClassMb => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMb { ref mb } = reg.ops[p].payload {
                    let mb_len = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < mb_len {
                        goto_fail = true;
                    } else {
                        let code = enc.mbc_to_code(&str_data[s..], end);
                        if !is_in_code_range(mb, code) {
                            goto_fail = true;
                        } else {
                            s += mb_len;
                            p += 1;
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassMbNot => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMb { ref mb } = reg.ops[p].payload {
                    let mb_len = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < mb_len {
                        goto_fail = true;
                    } else {
                        let code = enc.mbc_to_code(&str_data[s..], end);
                        if is_in_code_range(mb, code) {
                            goto_fail = true;
                        } else {
                            s += mb_len;
                            p += 1;
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // Mixed character class (single-byte bitset + multibyte ranges)
            OpCode::CClassMix | OpCode::CClassMixNot => {
                let not = opcode == OpCode::CClassMixNot;
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMix { ref bsp, ref mb } = reg.ops[p].payload
                {
                    let in_class = if enc.mbc_enc_len(&str_data[s..]) > 1 {
                        let code = enc.mbc_to_code(&str_data[s..], end);
                        if is_in_code_range(mb, code) {
                            true
                        } else if (code as usize) < SINGLE_BYTE_SIZE {
                            bitset_at(bsp, code as usize)
                        } else {
                            false
                        }
                    } else {
                        let c = str_data[s];
                        if (c as usize) < SINGLE_BYTE_SIZE {
                            bitset_at(bsp, c as usize)
                        } else {
                            false
                        }
                    };
                    if in_class == not {
                        goto_fail = true;
                    } else {
                        s += enclen(enc, str_data, s);
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_ANYCHAR / OP_ANYCHAR_ML - match any character
            // ================================================================
            OpCode::AnyChar => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else {
                    let n = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < n {
                        goto_fail = true;
                    } else if enc.is_mbc_newline(&str_data[s..], end) {
                        goto_fail = true; // ANYCHAR doesn't match newline
                    } else {
                        s += n;
                        p += 1;
                    }
                }
            }

            OpCode::AnyCharMl => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else {
                    let n = enclen(enc, str_data, s);
                    if right_range.saturating_sub(s) < n {
                        goto_fail = true;
                    } else {
                        s += n; // ANYCHAR_ML matches newlines too
                        p += 1;
                    }
                }
            }

            // ================================================================
            // OP_ANYCHAR_STAR / OP_ANYCHAR_ML_STAR - .* optimization
            // ================================================================
            OpCode::AnyCharStar => {
                // Push alternation for each possible length
                // Greedy: try matching as many chars as possible
                while s < right_range {
                    let n = enclen(enc, str_data, s);
                    if s + n > right_range {
                        break;
                    }
                    if enc.is_mbc_newline(&str_data[s..], end) {
                        break;
                    }
                    stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                    s += n;
                }
                p += 1;
            }

            OpCode::AnyCharMlStar => {
                while s < right_range {
                    let n = enclen(enc, str_data, s);
                    if s + n > right_range {
                        break;
                    }
                    stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                    s += n;
                }
                p += 1;
            }

            OpCode::AnyCharStarPeekNext => {
                if let OperationPayload::AnyCharStarPeekNext { c } = reg.ops[p].payload {
                    while s < right_range {
                        let n = enclen(enc, str_data, s);
                        if s + n > right_range {
                            break;
                        }
                        if enc.is_mbc_newline(&str_data[s..], end) {
                            break;
                        }
                        if s < end && str_data[s] == c {
                            stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                        }
                        s += n;
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::AnyCharMlStarPeekNext => {
                if let OperationPayload::AnyCharStarPeekNext { c } = reg.ops[p].payload {
                    while s < right_range {
                        let n = enclen(enc, str_data, s);
                        if s + n > right_range {
                            break;
                        }
                        if s < end && str_data[s] == c {
                            stack.push(StackEntry::Alt { pcode: p + 1, pstr: s, zid: -1, is_super: false });
                        }
                        s += n;
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Word / NoWord - \w and \W character type matching
            // ================================================================
            OpCode::Word => {
                if s >= right_range {
                    goto_fail = true;
                } else if !is_word_char_at(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            OpCode::WordAscii => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if !is_word_ascii(str_data[s]) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            OpCode::NoWord => {
                if s >= right_range {
                    goto_fail = true;
                } else if is_word_char_at(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            OpCode::NoWordAscii => {
                if right_range.saturating_sub(s) < 1 {
                    goto_fail = true;
                } else if is_word_ascii(str_data[s]) {
                    goto_fail = true;
                } else {
                    s += enclen(enc, str_data, s);
                    p += 1;
                }
            }

            // ================================================================
            // Word boundary opcodes
            // ================================================================
            OpCode::WordBoundary => {
                let mode = if let OperationPayload::WordBoundary { mode } = reg.ops[p].payload { mode } else { 0 };
                if !is_word_boundary(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::NoWordBoundary => {
                let mode = if let OperationPayload::WordBoundary { mode } = reg.ops[p].payload { mode } else { 0 };
                if is_word_boundary(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::WordBegin => {
                let mode = if let OperationPayload::WordBoundary { mode } = reg.ops[p].payload { mode } else { 0 };
                if !is_word_begin(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::WordEnd => {
                let mode = if let OperationPayload::WordBoundary { mode } = reg.ops[p].payload { mode } else { 0 };
                if !is_word_end(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::TextSegmentBoundary => {
                if let OperationPayload::TextSegmentBoundary { boundary_type, not } = reg.ops[p].payload {
                    let is_break = match boundary_type {
                        TextSegmentBoundaryType::ExtendedGraphemeCluster => {
                            crate::unicode::onigenc_egcb_is_break_position(enc, str_data, s, 0, end)
                        }
                        TextSegmentBoundaryType::Word => {
                            crate::unicode::onigenc_wb_is_break_position(enc, str_data, s, 0, end)
                        }
                    };
                    let result = if not { !is_break } else { is_break };
                    if result {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Position anchors
            // ================================================================
            OpCode::BeginBuf => {
                if s != 0 {
                    goto_fail = true;
                } else if opton_notbol(options) {
                    goto_fail = true;
                } else if opton_not_begin_string(options) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::EndBuf => {
                if s != end {
                    goto_fail = true;
                } else if opton_noteol(options) {
                    goto_fail = true;
                } else if opton_not_end_string(options) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::BeginLine => {
                if s == 0 {
                    if opton_notbol(options) {
                        goto_fail = true;
                    } else {
                        p += 1;
                    }
                } else if s > 0 && str_data[s - 1] == b'\n' {
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EndLine => {
                if s == end {
                    if opton_noteol(options) {
                        goto_fail = true;
                    } else {
                        p += 1;
                    }
                } else if enc.is_mbc_newline(&str_data[s..], end) {
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::SemiEndBuf => {
                // Match end of string or before final newline
                if s == end {
                    if opton_noteol(options) || opton_not_end_string(options) {
                        goto_fail = true;
                    } else {
                        p += 1;
                    }
                } else if s + 1 == end && str_data[s] == b'\n' {
                    if opton_noteol(options) || opton_not_end_string(options) {
                        goto_fail = true;
                    } else {
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CheckPosition => {
                if let OperationPayload::CheckPosition { check_type } = reg.ops[p].payload {
                    match check_type {
                        CheckPositionType::SearchStart => {
                            if s != msa.start {
                                goto_fail = true;
                            } else {
                                p += 1;
                            }
                        }
                        CheckPositionType::CurrentRightRange => {
                            if s != right_range {
                                goto_fail = true;
                            } else {
                                p += 1;
                            }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Back references
            // ================================================================
            OpCode::BackRef1 => {
                if num_mem >= 1 {
                    if let (Some(ms), Some(me)) = (
                        get_mem_start(reg, &stack, &mem_start_stk, 1),
                        get_mem_end(reg, &stack, &mem_end_stk, 1),
                    ) {
                        let ref_len = me - ms;
                        if right_range.saturating_sub(s) < ref_len {
                            goto_fail = true;
                        } else if str_data[s..s + ref_len] != str_data[ms..me] {
                            goto_fail = true;
                        } else {
                            s += ref_len;
                            p += 1;
                        }
                    } else {
                        goto_fail = true; // group not yet matched
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRef2 => {
                if num_mem >= 2 {
                    if let (Some(ms), Some(me)) = (
                        get_mem_start(reg, &stack, &mem_start_stk, 2),
                        get_mem_end(reg, &stack, &mem_end_stk, 2),
                    ) {
                        let ref_len = me - ms;
                        if right_range.saturating_sub(s) < ref_len {
                            goto_fail = true;
                        } else if str_data[s..s + ref_len] != str_data[ms..me] {
                            goto_fail = true;
                        } else {
                            s += ref_len;
                            p += 1;
                        }
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefN => {
                if let OperationPayload::BackRefN { n1 } = reg.ops[p].payload {
                    let n1 = n1 as usize;
                    if n1 <= num_mem {
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, n1),
                            get_mem_end(reg, &stack, &mem_end_stk, n1),
                        ) {
                            let ref_len = me - ms;
                            if right_range.saturating_sub(s) < ref_len {
                                goto_fail = true;
                            } else if str_data[s..s + ref_len] != str_data[ms..me] {
                                goto_fail = true;
                            } else {
                                s += ref_len;
                                p += 1;
                            }
                        } else {
                            goto_fail = true;
                        }
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefNIc => {
                if let OperationPayload::BackRefN { n1 } = reg.ops[p].payload {
                    let n1 = n1 as usize;
                    if n1 <= num_mem {
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, n1),
                            get_mem_end(reg, &stack, &mem_end_stk, n1),
                        ) {
                            let ref_len = me - ms;
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len {
                                    goto_fail = true;
                                } else if !string_cmp_ic(enc, reg.case_fold_flag, str_data, ms, &mut s, ref_len) {
                                    goto_fail = true;
                                } else {
                                    p += 1;
                                }
                            } else {
                                p += 1;
                            }
                        } else {
                            goto_fail = true;
                        }
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefMulti => {
                if let OperationPayload::BackRefGeneral { num, ref ns, .. } = &reg.ops[p].payload {
                    let tlen = *num as usize;
                    let mut matched = false;
                    for i in 0..tlen {
                        let mem = ns[i] as usize;
                        if mem > num_mem { continue; }
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, mem),
                            get_mem_end(reg, &stack, &mem_end_stk, mem),
                        ) {
                            let ref_len = me - ms;
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len { continue; }
                                if str_data[s..s + ref_len] != str_data[ms..me] { continue; }
                                s += ref_len;
                            }
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefMultiIc => {
                if let OperationPayload::BackRefGeneral { num, ref ns, .. } = &reg.ops[p].payload {
                    let tlen = *num as usize;
                    let mut matched = false;
                    for i in 0..tlen {
                        let mem = ns[i] as usize;
                        if mem > num_mem { continue; }
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, &mem_start_stk, mem),
                            get_mem_end(reg, &stack, &mem_end_stk, mem),
                        ) {
                            let ref_len = me - ms;
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len { continue; }
                                let mut swork = s;
                                if !string_cmp_ic(enc, reg.case_fold_flag, str_data, ms, &mut swork, ref_len) { continue; }
                                s = swork;
                            }
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefCheck => {
                if let OperationPayload::BackRefGeneral { num, ref ns, .. } = &reg.ops[p].payload {
                    let tlen = *num as usize;
                    let mut found = false;
                    for i in 0..tlen {
                        let mem = ns[i] as usize;
                        if mem > num_mem { continue; }
                        if get_mem_start(reg, &stack, &mem_start_stk, mem).is_some()
                            && get_mem_end(reg, &stack, &mem_end_stk, mem).is_some()
                        {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefWithLevel | OpCode::BackRefWithLevelIc => {
                if let OperationPayload::BackRefGeneral { num, ref ns, nest_level } = &reg.ops[p].payload {
                    let ignore_case = reg.ops[p].opcode == OpCode::BackRefWithLevelIc;
                    if backref_match_at_nested_level(
                        reg, &stack, ignore_case, reg.case_fold_flag,
                        *nest_level, *num, ns, &mut s, str_data, end,
                    ) {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefCheckWithLevel => {
                if let OperationPayload::BackRefGeneral { num, ref ns, nest_level } = &reg.ops[p].payload {
                    let found = if backref_check_at_nested_level(&stack, *nest_level, *num, ns) {
                        true
                    } else if *nest_level == 0 {
                        // At level 0, also check mem arrays directly (group may use
                        // non-push MemStart with no stack entry)
                        let tlen = *num as usize;
                        let mut f = false;
                        for i in 0..tlen {
                            let mem = ns[i] as usize;
                            if mem > num_mem { continue; }
                            if get_mem_start(reg, &stack, &mem_start_stk, mem).is_some()
                                && get_mem_end(reg, &stack, &mem_end_stk, mem).is_some()
                            {
                                f = true;
                                break;
                            }
                        }
                        f
                    } else {
                        false
                    };
                    if found {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Memory (capture group) operations
            // ================================================================
            OpCode::MemStart => {
                if let OperationPayload::MemoryStart { num } = reg.ops[p].payload {
                    let num = num as usize;
                    mem_start_stk[num] = MemPtr::Pos(s);
                    mem_end_stk[num] = MemPtr::Invalid;
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemStartPush => {
                if let OperationPayload::MemoryStart { num } = reg.ops[p].payload {
                    let num = num as usize;
                    let prev_start = mem_start_stk[num];
                    let prev_end = mem_end_stk[num];
                    let si = stack.len();
                    stack.push(StackEntry::MemStart {
                        zid: num,
                        pstr: s,
                        prev_start,
                        prev_end,
                    });
                    mem_start_stk[num] = MemPtr::StackIdx(si);
                    mem_end_stk[num] = MemPtr::Invalid;
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEnd => {
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let num = num as usize;
                    mem_end_stk[num] = MemPtr::Pos(s);
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndPush => {
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let num = num as usize;
                    let prev_start = mem_start_stk[num];
                    let prev_end = mem_end_stk[num];
                    let si = stack.len();
                    stack.push(StackEntry::MemEnd {
                        zid: num,
                        pstr: s,
                        prev_start,
                        prev_end,
                    });
                    mem_end_stk[num] = MemPtr::StackIdx(si);
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndPushRec => {
                // Recursive capture end (push variant): find matching MEM_START,
                // push MEM_END, update start/end tracking
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let mem = num as usize;
                    let (start_ptr, _) = stack_get_mem_start_for_rec(&stack, mem, reg.push_mem_start);
                    let si = stack.len();
                    stack.push(StackEntry::MemEnd {
                        zid: mem,
                        pstr: s,
                        prev_start: mem_start_stk[mem],
                        prev_end: mem_end_stk[mem],
                    });
                    mem_start_stk[mem] = start_ptr;
                    mem_end_stk[mem] = MemPtr::StackIdx(si);
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndRec => {
                // Recursive capture end (non-push variant): find matching MEM_START,
                // update start/end, push MemEndMark for level tracking
                if let OperationPayload::MemoryEnd { num } = reg.ops[p].payload {
                    let mem = num as usize;
                    mem_end_stk[mem] = MemPtr::Pos(s);
                    let (start_ptr, _) = stack_get_mem_start_for_rec(&stack, mem, reg.push_mem_start);
                    mem_start_stk[mem] = start_ptr;
                    stack.push(StackEntry::MemEndMark { zid: mem });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_FAIL - backtrack
            // ================================================================
            OpCode::Fail => {
                goto_fail = true;
            }

            // ================================================================
            // OP_JUMP - unconditional jump
            // ================================================================
            OpCode::Jump => {
                if let OperationPayload::Jump { addr } = reg.ops[p].payload {
                    p = (p as i32 + addr) as usize;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH / OP_PUSH_SUPER - push choice point (alternation)
            // ================================================================
            OpCode::Push | OpCode::PushSuper => {
                if let OperationPayload::Push { addr } = reg.ops[p].payload {
                    let alt_target = (p as i32 + addr) as usize;
                    let is_super = reg.ops[p].opcode == OpCode::PushSuper;
                    stack.push(StackEntry::Alt {
                        pcode: alt_target,
                        pstr: s,
                        zid: -1,
                        is_super,
                    });
                    p += 1; // try main path first
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_POP - discard top stack entry
            // ================================================================
            OpCode::Pop => {
                stack.pop();
                p += 1;
            }

            // ================================================================
            // OP_POP_TO_MARK - pop until Mark with matching id
            // ================================================================
            OpCode::PopToMark => {
                if let OperationPayload::PopToMark { id } = reg.ops[p].payload {
                    let id = id as usize;
                    // Pop entries until we find the matching Mark, but don't
                    // restore positions (unlike CutToMark)
                    loop {
                        if let Some(entry) = stack.pop() {
                            if let StackEntry::Mark { zid, .. } = &entry {
                                if *zid == id {
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH_OR_JUMP_EXACT1 - optimized push for exact char
            // ================================================================
            OpCode::PushOrJumpExact1 => {
                if let OperationPayload::PushOrJumpExact1 { addr, c } = reg.ops[p].payload {
                    if s < right_range && str_data[s] == c {
                        // Character matches: push alternative and continue
                        let alt_target = (p as i32 + addr) as usize;
                        stack.push(StackEntry::Alt {
                            pcode: alt_target,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
                        p += 1;
                    } else {
                        // Character doesn't match: jump
                        p = (p as i32 + addr) as usize;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH_IF_PEEK_NEXT - push only if next char matches
            // ================================================================
            OpCode::PushIfPeekNext => {
                if let OperationPayload::PushIfPeekNext { addr, c } = reg.ops[p].payload {
                    if s < right_range && str_data[s] == c {
                        let alt_target = (p as i32 + addr) as usize;
                        stack.push(StackEntry::Alt {
                            pcode: alt_target,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_REPEAT / OP_REPEAT_NG - initialize repeat counter
            // ================================================================
            OpCode::Repeat | OpCode::RepeatNg => {
                if let OperationPayload::Repeat { id, addr } = reg.ops[p].payload {
                    let id = id as usize;
                    // Push initial repeat count = 0
                    stack.push(StackEntry::RepeatInc { zid: id, count: 0 });

                    if reg.repeat_range[id].lower == 0 {
                        // Can skip the loop body entirely
                        let alt_target = (p as i32 + addr) as usize;
                        if opcode == OpCode::Repeat {
                            // Greedy: push skip as alternative, try body first
                            stack.push(StackEntry::Alt {
                                pcode: alt_target,
                                pstr: s,
                                zid: -1,
                                is_super: false,
                            });
                        } else {
                            // Non-greedy: push body as alternative, try skip first
                            stack.push(StackEntry::Alt {
                                pcode: p + 1,
                                pstr: s,
                                zid: -1,
                                is_super: false,
                            });
                            p = alt_target;
                            continue;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_REPEAT_INC / OP_REPEAT_INC_NG - increment and check repeat
            // ================================================================
            OpCode::RepeatInc => {
                if let OperationPayload::RepeatInc { id } = reg.ops[p].payload {
                    let id = id as usize;
                    let count = stack_get_repeat_count(&stack, id) + 1;
                    let lower = reg.repeat_range[id].lower;
                    let upper = reg.repeat_range[id].upper;
                    let body_start = reg.repeat_range[id].u_offset as usize;

                    // C order for greedy: branch first, then push count
                    if upper != INFINITE_REPEAT && count >= upper {
                        p += 1;
                    } else if count >= lower {
                        p += 1;
                        stack.push(StackEntry::Alt { pcode: p, pstr: s, zid: -1, is_super: false });
                        p = body_start;
                    } else {
                        p = body_start;
                    }
                    // Count pushed AFTER Alt — gets popped on backtrack (correct for greedy)
                    stack.push(StackEntry::RepeatInc { zid: id, count });
                } else {
                    goto_fail = true;
                }
            }

            OpCode::RepeatIncNg => {
                if let OperationPayload::RepeatInc { id } = reg.ops[p].payload {
                    let id = id as usize;
                    let count = stack_get_repeat_count(&stack, id) + 1;
                    let lower = reg.repeat_range[id].lower;
                    let upper = reg.repeat_range[id].upper;
                    let body_start = reg.repeat_range[id].u_offset as usize;

                    // C order for non-greedy: push count FIRST, then branch
                    // Count pushed BEFORE Alt — survives backtrack (correct for lazy)
                    stack.push(StackEntry::RepeatInc { zid: id, count });

                    if upper != INFINITE_REPEAT && count as i32 == upper {
                        p += 1;
                    } else if count >= lower {
                        stack.push(StackEntry::Alt { pcode: body_start, pstr: s, zid: -1, is_super: false });
                        p += 1;
                    } else {
                        p = body_start;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_EMPTY_CHECK_START / END - detect empty match in loops
            // ================================================================
            OpCode::EmptyCheckStart => {
                if let OperationPayload::EmptyCheckStart { mem } = reg.ops[p].payload {
                    let mem = mem as usize;
                    stack.push(StackEntry::EmptyCheckStart {
                        zid: mem,
                        pstr: s,
                    });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EmptyCheckEnd => {
                if let OperationPayload::EmptyCheckEnd { mem, .. } = reg.ops[p].payload {
                    let mem = mem as usize;
                    let is_empty = stack_empty_check(&stack, mem, s);
                    p += 1;
                    if is_empty {
                        // Empty loop detected — skip the next instruction
                        // (JUMP, PUSH, REPEAT_INC, or REPEAT_INC_NG) to break the loop.
                        // Mirrors C: empty_check_found: INC_OP;
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EmptyCheckEndMemst | OpCode::EmptyCheckEndMemstPush => {
                if let OperationPayload::EmptyCheckEnd { mem, empty_status_mem } = reg.ops[p].payload {
                    let mem = mem as usize;
                    let is_empty = stack_empty_check_mem(&stack, mem, s, empty_status_mem as u32, reg,
                                                         &mem_start_stk, &mem_end_stk);
                    p += 1;
                    if is_empty {
                        // Truly empty → skip next op (JUMP back)
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_MOVE - move string position
            // ================================================================
            OpCode::Move => {
                if let OperationPayload::Move { n } = reg.ops[p].payload {
                    if n < 0 {
                        // Step back n characters (encoding-aware)
                        match onigenc_step_back(enc, 0, s, str_data, (-n) as usize) {
                            Some(new_s) => { s = new_s; p += 1; }
                            None => { goto_fail = true; }
                        }
                    } else {
                        // Step forward n characters
                        match onigenc_step(enc, s, end, str_data, n as usize) {
                            Some(new_s) => { s = new_s; p += 1; }
                            None => { goto_fail = true; }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_STEP_BACK_START / NEXT - lookbehind support
            // ================================================================
            OpCode::StepBackStart => {
                if let OperationPayload::StepBackStart {
                    initial,
                    remaining,
                    addr,
                } = reg.ops[p].payload
                {
                    let initial = initial as usize;
                    // Step back 'initial' characters (encoding-aware)
                    if initial != 0 {
                        match onigenc_step_back(enc, 0, s, str_data, initial) {
                            Some(new_s) => { s = new_s; }
                            None => { goto_fail = true; }
                        }
                    }
                    if !goto_fail {
                        if remaining != 0 {
                            // Variable-length: push Alt with remaining count, jump to addr
                            stack.push(StackEntry::Alt {
                                pcode: p + 1,
                                pstr: s,
                                zid: remaining,
                                is_super: false,
                            });
                            p = (p as i32 + addr as i32) as usize;
                        } else {
                            p += 1;
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::StepBackNext => {
                // last_alt_zid was set by the backtrack that jumped here
                let mut remaining = last_alt_zid;
                if remaining != INFINITE_LEN as i32 {
                    remaining -= 1;
                }
                match onigenc_step_back(enc, 0, s, str_data, 1) {
                    Some(new_s) => { s = new_s; }
                    None => { goto_fail = true; }
                }
                if !goto_fail {
                    if remaining != 0 {
                        stack.push(StackEntry::Alt {
                            pcode: p,
                            pstr: s,
                            zid: remaining,
                            is_super: false,
                        });
                    }
                    p += 1;
                }
            }

            // ================================================================
            // OP_MARK - push a named checkpoint
            // ================================================================
            OpCode::Mark => {
                if let OperationPayload::Mark { id, save_pos } = reg.ops[p].payload {
                    let id = id as usize;
                    let pos = if save_pos { Some(s) } else { None };
                    stack.push(StackEntry::Mark { zid: id, pos });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_CUT_TO_MARK - void entries to mark, optionally restore position
            // C always uses STACK_TO_VOID_TO_MARK (not POP_TO_MARK)
            // ================================================================
            OpCode::CutToMark => {
                if let OperationPayload::CutToMark { id, restore_pos } = reg.ops[p].payload {
                    let id = id as usize;
                    let saved_pos = stack_void_to_mark(&mut stack, id);
                    if restore_pos {
                        if let Some(pos) = saved_pos {
                            s = pos;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_SAVE_VAL - save a value on the stack
            // ================================================================
            OpCode::SaveVal => {
                if let OperationPayload::SaveVal { save_type, id } = reg.ops[p].payload {
                    let id = id as usize;
                    let v = match save_type {
                        SaveType::Keep => s,
                        SaveType::S => s,
                        SaveType::RightRange => right_range,
                    };
                    stack.push(StackEntry::SaveVal {
                        zid: id,
                        save_type,
                        v,
                    });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_UPDATE_VAR - update a variable from the stack
            // ================================================================
            OpCode::UpdateVar => {
                if let OperationPayload::UpdateVar {
                    var_type, id, clear,
                } = reg.ops[p].payload
                {
                    let id = id as usize;
                    match var_type {
                        UpdateVarType::KeepFromStackLast => {
                            if let Some(v) =
                                stack_get_save_val_type_last(&stack, SaveType::Keep)
                            {
                                keep = v;
                            }
                        }
                        UpdateVarType::SFromStack => {
                            if let Some(v) =
                                stack_get_save_val_last(&stack, SaveType::S, id)
                            {
                                s = v;
                            }
                        }
                        UpdateVarType::RightRangeFromStack => {
                            if let Some(v) =
                                stack_get_save_val_last(&stack, SaveType::RightRange, id)
                            {
                                right_range = v;
                            }
                        }
                        UpdateVarType::RightRangeFromSStack => {
                            if let Some(v) =
                                stack_get_save_val_last(&stack, SaveType::S, id)
                            {
                                right_range = v;
                            }
                        }
                        UpdateVarType::RightRangeToS => {
                            right_range = s;
                        }
                        UpdateVarType::RightRangeInit => {
                            right_range = in_right_range;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_CALL / OP_RETURN - subroutine call/return
            // ================================================================
            OpCode::Call => {
                if let OperationPayload::Call { addr } = reg.ops[p].payload {
                    let addr = addr as usize;
                    stack.push(StackEntry::CallFrame { ret_addr: p + 1 });
                    p = addr;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::Return => {
                // Search backwards for CallFrame, skipping nested Return markers.
                // Each STK_RETURN increments the level; each STK_CALL_FRAME decrements.
                let mut level = 0i32;
                let mut ret_addr = None;
                for i in (0..stack.len()).rev() {
                    match &stack[i] {
                        StackEntry::CallFrame { ret_addr: ra } => {
                            if level == 0 {
                                ret_addr = Some(*ra);
                                break;
                            }
                            level -= 1;
                        }
                        StackEntry::Return => {
                            level += 1;
                        }
                        _ => {}
                    }
                }
                if let Some(ra) = ret_addr {
                    stack.push(StackEntry::Return);
                    p = ra;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Callout opcodes
            // ================================================================
            OpCode::CalloutContents => {
                // Callout of contents: always succeeds (we don't execute user code)
                let num = match &reg.ops[p].payload {
                    OperationPayload::CalloutContents { num } => *num,
                    _ => 0,
                };
                if let Some(ref ext) = reg.extp {
                    if num >= 1 && (num as usize) <= ext.callout_list.len() {
                        let entry = &ext.callout_list[(num - 1) as usize];
                        if (entry.callout_in & CALLOUT_IN_RETRACTION) != 0 {
                            stack.push(StackEntry::Callout {
                                num,
                                id: ONIG_NON_NAME_ID,
                            });
                        }
                    }
                }
                p += 1;
            }
            OpCode::CalloutName => {
                let (num, id) = match &reg.ops[p].payload {
                    OperationPayload::CalloutName { num, id } => (*num, *id),
                    _ => (0, 0),
                };
                let call_result = run_builtin_callout(reg, num, id, false, &mut callout_data);
                if call_result == ONIG_CALLOUT_FAIL {
                    goto_fail = true;
                } else {
                    // Push retraction entry if needed
                    if let Some(ref ext) = reg.extp {
                        if num >= 1 && (num as usize) <= ext.callout_list.len() {
                            let entry = &ext.callout_list[(num - 1) as usize];
                            if (entry.callout_in & CALLOUT_IN_RETRACTION) != 0 {
                                stack.push(StackEntry::Callout { num, id });
                            }
                        }
                    }
                    p += 1;
                }
            }
        }

        // Handle failure (backtracking)
        if goto_fail {
            // Retry limit check
            retry_in_match_counter += 1;
            if retry_limit_in_match != 0 && retry_in_match_counter > retry_limit_in_match {
                best_len = ONIGERR_RETRY_LIMIT_IN_MATCH_OVER;
                break;
            }
            // Time limit check (every CHECK_TIME_INTERVAL retries)
            if time_limit_ms > 0 && (retry_in_match_counter % CHECK_TIME_INTERVAL) == 0 {
                if msa.check_time_limit() {
                    best_len = ONIGERR_TIME_LIMIT_OVER;
                    break;
                }
            }

            match stack_pop(
                &mut stack,
                pop_level,
                &mut mem_start_stk,
                &mut mem_end_stk,
                reg,
                &mut callout_data,
            ) {
                Some((pcode, pstr, alt_zid)) => {
                    if pcode == FINISH_PCODE {
                        // Hit bottom sentinel - no more alternatives
                        break;
                    }
                    p = pcode;
                    s = pstr;
                    last_alt_zid = alt_zid;
                }
                None => {
                    // Stack empty - match failed
                    break;
                }
            }
        }
    }

    // Accumulate retry counter into search counter
    msa.retry_limit_in_search_counter += retry_in_match_counter;

    // Return reusable buffers to MatchArg for next call
    msa.stack = stack;
    msa.mem_start_stk = mem_start_stk;
    msa.mem_end_stk = mem_end_stk;

    best_len
}

// ============================================================================
// onig_match - match at a specific position (port of C's onig_match)
// ============================================================================

/// Try to match the regex at exactly position `at` in the string.
/// Returns the match length on success, ONIG_MISMATCH (-1) on failure.
///
/// Parameters:
/// - reg: compiled regex
/// - str_data: the input string bytes
/// - end: end position (typically str_data.len())
/// - at: position to try matching at
/// - region: optional region to fill with capture group positions
/// - option: match options
pub fn onig_match(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    at: usize,
    region: Option<OnigRegion>,
    option: OnigOptionType,
) -> (i32, Option<OnigRegion>) {
    let mut msa = MatchArg::new(reg, option, region, at);

    if opton_check_validity_of_string(msa.options) {
        if !reg.enc.is_valid_mbc_string(&str_data[..end]) {
            return (ONIGERR_INVALID_WIDE_CHAR_VALUE, msa.region);
        }
    }

    if let Some(ref mut r) = msa.region {
        r.resize(reg.num_mem + 1);
        r.clear();
    }

    let result = match_at(reg, str_data, end, end, at, &mut msa);

    // Handle FIND_LONGEST
    let result = if opton_find_longest(msa.options) && result == ONIG_MISMATCH {
        if msa.best_len >= 0 {
            msa.best_len
        } else {
            result
        }
    } else {
        result
    };

    (result, msa.region)
}

pub fn onig_match_with_param(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    at: usize,
    region: Option<OnigRegion>,
    option: OnigOptionType,
    mp: &OnigMatchParam,
) -> (i32, Option<OnigRegion>) {
    let mut msa = MatchArg::from_param(reg, option, region, at, mp);

    if opton_check_validity_of_string(msa.options) {
        if !reg.enc.is_valid_mbc_string(&str_data[..end]) {
            return (ONIGERR_INVALID_WIDE_CHAR_VALUE, msa.region);
        }
    }

    if let Some(ref mut r) = msa.region {
        r.resize(reg.num_mem + 1);
        r.clear();
    }

    let result = match_at(reg, str_data, end, end, at, &mut msa);

    let result = if opton_find_longest(msa.options) && result == ONIG_MISMATCH {
        if msa.best_len >= 0 { msa.best_len } else { result }
    } else {
        result
    };

    (result, msa.region)
}

// ============================================================================
// onig_search - search for a match anywhere in the string
// ============================================================================

/// Search for the regex pattern in the string, trying each position from
/// `start` to `range`.
/// Returns the match position on success, ONIG_MISMATCH (-1) on failure.
///
/// Parameters:
/// - reg: compiled regex
// ============================================================================
// Search optimization functions — mirrors C's regexec.c lines 5168-5645
// ============================================================================

/// Naive string search. Mirrors C's slow_search.
fn slow_search(enc: OnigEncoding, target: &[u8], text: &[u8],
               text_start: usize, text_end: usize, text_range: usize) -> Option<usize> {
    if target.is_empty() { return Some(text_start); }
    let tlen = target.len();
    let mut limit = text_end.saturating_sub(tlen - 1);
    if limit > text_range { limit = text_range; }
    let mut s = text_start;
    while s < limit {
        if text[s] == target[0] {
            let mut ok = true;
            for i in 1..tlen {
                if s + i >= text_end || text[s + i] != target[i] {
                    ok = false;
                    break;
                }
            }
            if ok { return Some(s); }
        }
        s += enclen(enc, text, s);
    }
    None
}

/// Sunday quick search (BMH variant). Mirrors C's sunday_quick_search.
fn sunday_quick_search(reg: &RegexType, target: &[u8], text: &[u8],
                       text_start: usize, text_end: usize, text_range: usize) -> Option<usize> {
    let map_offset = reg.map_offset as usize;
    let tlen = target.len();
    if tlen == 0 { return Some(text_start); }
    let tail_idx = tlen - 1;

    let end = if tlen > text_end.saturating_sub(text_range) {
        if tlen > text_end.saturating_sub(text_start) {
            return None;
        }
        text_end
    } else {
        text_range + tlen
    };

    let mut s = text_start + tail_idx;
    while s < end {
        let mut p = s;
        let mut t = tail_idx;
        loop {
            if text[p] != target[t] { break; }
            if t == 0 { return Some(p); }
            p -= 1;
            t -= 1;
        }
        if s + map_offset >= text_end { break; }
        s += reg.map[text[s + map_offset] as usize] as usize;
    }
    None
}

/// Sunday quick search with step forward for multi-byte safe operation.
/// Mirrors C's sunday_quick_search_step_forward.
fn sunday_quick_search_step_forward(reg: &RegexType, target: &[u8], text: &[u8],
                                     text_start: usize, text_end: usize,
                                     text_range: usize) -> Option<usize> {
    let enc = reg.enc;
    let tlen = target.len();
    if tlen == 0 { return Some(text_start); }
    let tail_idx = tlen - 1;
    let mut end = text_range;
    if tail_idx as usize > text_end.saturating_sub(end) {
        end = text_end.saturating_sub(tail_idx);
    }

    let map_offset = reg.map_offset as usize;
    let mut s = text_start;
    while s < end {
        let se = s + tail_idx;
        let mut p = se;
        let mut t = tail_idx;
        loop {
            if text[p] != target[t] { break; }
            if t == 0 { return Some(s); }
            p -= 1;
            t -= 1;
        }
        if se + map_offset >= text_end { break; }
        let skip = reg.map[text[se + map_offset] as usize] as usize;
        let next = s + skip;
        if next < end {
            s = onigenc_get_right_adjust_char_head(enc, text, s, next);
        } else {
            break;
        }
    }
    None
}

/// Character map search. Mirrors C's map_search.
fn map_search(enc: OnigEncoding, map: &[u8; CHAR_MAP_SIZE], text: &[u8],
              text_start: usize, text_range: usize) -> Option<usize> {
    let mut s = text_start;
    while s < text_range {
        if map[text[s] as usize] != 0 { return Some(s); }
        s += enclen(enc, text, s);
    }
    None
}

/// Backward naive string search. Mirrors C's slow_search_backward.
fn slow_search_backward(enc: OnigEncoding, target: &[u8], text: &[u8],
                        text_start: usize, adjust_text: usize,
                        text_end: usize, search_start: usize) -> Option<usize> {
    let tlen = target.len();
    if tlen == 0 { return Some(search_start); }
    let mut s = text_end.saturating_sub(tlen);
    if s > search_start {
        s = search_start;
    } else {
        s = left_adjust_char_head(enc, text, adjust_text, s);
    }
    while s >= text_start {
        if text[s] == target[0] {
            let mut ok = true;
            for i in 1..tlen {
                if s + i >= text_end || text[s + i] != target[i] {
                    ok = false;
                    break;
                }
            }
            if ok { return Some(s); }
        }
        if s == 0 { break; }
        s = onigenc_get_prev_char_head(enc, text, adjust_text, s);
        if s < text_start { break; }
    }
    None
}

/// Backward character map search. Mirrors C's map_search_backward.
fn map_search_backward(enc: OnigEncoding, map: &[u8; CHAR_MAP_SIZE], text: &[u8],
                       text_start: usize, adjust_text: usize,
                       search_start: usize) -> Option<usize> {
    let mut s = search_start;
    loop {
        if map[text[s] as usize] != 0 { return Some(s); }
        if s <= text_start { break; }
        s = onigenc_get_prev_char_head(enc, text, adjust_text, s);
        if s < text_start { break; }
    }
    None
}

/// Left-adjust char head (ONIGENC_LEFT_ADJUST_CHAR_HEAD).
/// For UTF-8: walk backward from pos to find char boundary at or before pos.
fn left_adjust_char_head(_enc: OnigEncoding, _text: &[u8], start: usize, pos: usize) -> usize {
    if pos <= start { return start; }
    let mut p = pos;
    while p > start && _text[p] & 0xC0 == 0x80 {
        p -= 1;
    }
    p
}

/// Backward search using optimization strategy.
/// Returns Some((low, high)) if a candidate was found, None otherwise.
fn backward_search(reg: &RegexType, str_data: &[u8], end: usize,
                   search_start: usize, min_range: usize,
                   adjrange: usize) -> Option<(usize, usize)> {
    let mut p = search_start;
    loop {
        let found = match reg.optimize {
            OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward => {
                slow_search_backward(reg.enc, &reg.exact, str_data,
                                     min_range, adjrange, end, p)
            }
            OptimizeType::Map => {
                map_search_backward(reg.enc, &reg.map, str_data,
                                    min_range, adjrange, p)
            }
            OptimizeType::None => { return None; }
        };

        p = match found {
            Some(pos) => pos,
            None => return None,
        };

        // Validate sub_anchor
        if reg.sub_anchor != 0 {
            let mut retry = false;
            if (reg.sub_anchor & ANCR_BEGIN_LINE) != 0 {
                if p > 0 {
                    let prev = onigenc_get_prev_char_head(reg.enc, str_data, 0, p);
                    if !is_mbc_newline(reg.enc, str_data, prev, end) {
                        retry = true;
                    }
                }
            }
            if !retry && (reg.sub_anchor & ANCR_END_LINE) != 0 {
                if p >= end {
                    // at end, check previous
                    let prev = onigenc_get_prev_char_head(reg.enc, str_data, adjrange, p);
                    if prev < adjrange {
                        return None;
                    }
                    if is_mbc_newline(reg.enc, str_data, prev, end) {
                        p = prev;
                        continue;
                    }
                } else if !is_mbc_newline(reg.enc, str_data, p, end) {
                    retry = true;
                }
            }
            if retry {
                if p == 0 { return None; }
                p = onigenc_get_prev_char_head(reg.enc, str_data, adjrange, p);
                if p < adjrange { return None; }
                continue;
            }
        }

        // Calculate low/high range
        if reg.dist_max != INFINITE_LEN {
            let low = if p < reg.dist_max as usize {
                0
            } else {
                p - reg.dist_max as usize
            };

            let high = if reg.dist_min != 0 {
                if p < reg.dist_min as usize { 0 } else { p - reg.dist_min as usize }
            } else {
                p
            };

            let high = onigenc_get_right_adjust_char_head(reg.enc, str_data, adjrange, high);
            return Some((low, high));
        }

        return Some((0, p));
    }
}

/// Get right-adjusted char head for multi-byte encoding.
/// Mirrors C's onigenc_get_right_adjust_char_head: left-adjust first, then
/// advance by one character if the result is before s.
fn onigenc_get_right_adjust_char_head(enc: OnigEncoding, text: &[u8],
                                       start: usize, s: usize) -> usize {
    let p = left_adjust_char_head(enc, text, start, s);
    if p < s {
        p + enclen(enc, text, p)
    } else {
        p
    }
}

/// Forward search using optimization strategy.
/// Returns Some((low, high)) if a candidate was found, None otherwise.
fn forward_search(reg: &RegexType, str_data: &[u8], end: usize,
                  start: usize, range: usize) -> Option<(usize, usize)> {
    let mut p = start;
    if reg.dist_min != 0 {
        if end.saturating_sub(p) <= reg.dist_min as usize {
            return None;
        }
        if enc_is_singlebyte(reg.enc) {
            p += reg.dist_min as usize;
        } else {
            let target = p + reg.dist_min as usize;
            while p < target && p < end {
                p += enclen(reg.enc, str_data, p);
            }
        }
    }

    let mut pprev: Option<usize> = None;
    loop {
        // Search for the optimization target
        let found = match reg.optimize {
            OptimizeType::Str => {
                slow_search(reg.enc, &reg.exact, str_data, p, end, range)
            }
            OptimizeType::StrFast => {
                sunday_quick_search(reg, &reg.exact, str_data, p, end, range)
            }
            OptimizeType::StrFastStepForward => {
                sunday_quick_search_step_forward(reg, &reg.exact, str_data, p, end, range)
            }
            OptimizeType::Map => {
                map_search(reg.enc, &reg.map, str_data, p, range)
            }
            OptimizeType::None => { return None; }
        };

        p = match found {
            Some(pos) if pos < range => pos,
            _ => return None,
        };

        if p.saturating_sub(start) < reg.dist_min as usize {
            pprev = Some(p);
            p += enclen(reg.enc, str_data, p);
            continue; // retry
        }

        // Validate sub_anchor
        if reg.sub_anchor != 0 {
            let mut retry = false;
            if (reg.sub_anchor & ANCR_BEGIN_LINE) != 0 {
                if p > 0 {
                    let prev = pprev.unwrap_or(0);
                    let prev_pos = onigenc_get_prev_char_head(reg.enc, str_data, prev, p);
                    if !is_mbc_newline(reg.enc, str_data, prev_pos, end) {
                        retry = true;
                    }
                }
            }
            if !retry && (reg.sub_anchor & ANCR_END_LINE) != 0 {
                if p >= end {
                    // at end - OK for some cases
                } else if !is_mbc_newline(reg.enc, str_data, p, end) {
                    retry = true;
                }
            }
            if retry {
                pprev = Some(p);
                p += enclen(reg.enc, str_data, p);
                continue; // retry
            }
        }

        // Calculate low/high range
        let (low, high);
        if reg.dist_max == 0 {
            low = p;
            high = p;
        } else {
            if reg.dist_max != INFINITE_LEN {
                if p.saturating_sub(0) < reg.dist_max as usize {
                    low = 0;
                } else {
                    let mut l = p - reg.dist_max as usize;
                    if l > start {
                        l = onigenc_get_right_adjust_char_head(reg.enc, str_data, start, l);
                    }
                    low = l;
                }
            } else {
                low = start; // infinite dist_max: any position from start
            }
            if p.saturating_sub(0) < reg.dist_min as usize {
                high = 0;
            } else {
                high = p - reg.dist_min as usize;
            }
        }
        return Some((low, high));
    }
}

/// Check if encoding is single-byte.
fn enc_is_singlebyte(enc: OnigEncoding) -> bool {
    enc.max_enc_len() == 1
}

/// Get previous character head position.
fn onigenc_get_prev_char_head(_enc: OnigEncoding, _text: &[u8], _start: usize, pos: usize) -> usize {
    if pos == 0 { return 0; }
    // For UTF-8: walk backwards past continuation bytes
    let mut p = pos - 1;
    while p > 0 && _text[p] & 0xC0 == 0x80 {
        p -= 1;
    }
    p
}

/// Check if position is a newline.
fn is_mbc_newline(_enc: OnigEncoding, text: &[u8], pos: usize, end: usize) -> bool {
    if pos >= end { return false; }
    text[pos] == b'\n'
}

/// - str_data: the input string bytes
/// - end: end position (typically str_data.len())
/// - start: starting search position
/// - range: search range end (exclusive for forward search)
/// - region: optional region to fill with capture group positions
/// - option: match options
pub fn onig_search(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    region: Option<OnigRegion>,
    option: OnigOptionType,
) -> (i32, Option<OnigRegion>) {
    let msa = MatchArg::new(reg, option, region, start);
    onig_search_inner(reg, str_data, end, start, range, msa)
}

pub fn onig_search_with_param(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    region: Option<OnigRegion>,
    option: OnigOptionType,
    mp: &OnigMatchParam,
) -> (i32, Option<OnigRegion>) {
    let msa = MatchArg::from_param(reg, option, region, start, mp);
    onig_search_inner(reg, str_data, end, start, range, msa)
}

fn onig_search_inner(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    mut msa: MatchArg,
) -> (i32, Option<OnigRegion>) {
    let enc = reg.enc;
    let find_longest = opton_find_longest(msa.options);
    let mut best_start: i32 = ONIG_MISMATCH;
    let mut best_len: i32 = ONIG_MISMATCH;

    if opton_check_validity_of_string(msa.options) {
        if !enc.is_valid_mbc_string(&str_data[..end]) {
            return (ONIGERR_INVALID_WIDE_CHAR_VALUE, msa.region);
        }
    }

    if start > range {
        // Backward search: start > range, search from start down to range
        if end == 0 { return (ONIG_MISMATCH, msa.region); }

        // orig_start is the right boundary for matching (upper range)
        let orig_start = if start < end {
            let elen = enclen(enc, str_data, start);
            start + elen
        } else {
            end
        };

        // s starts at start (same as C: s = (UChar*)start)
        let mut s = start;

        // Macro-like helper for match_at + result handling in backward search
        macro_rules! backward_match_and_check {
            ($s:expr, $orig_start:expr) => {{
                if let Some(ref mut r) = msa.region {
                    r.resize(reg.num_mem + 1);
                    r.clear();
                }
                msa.best_len = ONIG_MISMATCH;
                msa.best_s = 0;
                let r = match_at(reg, str_data, end, $orig_start, $s, &mut msa);
                if r != ONIG_MISMATCH {
                    if r < 0 { return (r, msa.region); }
                    if find_longest {
                        let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                        if best_len == ONIG_MISMATCH || match_len > best_len {
                            best_start = $s as i32;
                            best_len = match_len;
                        }
                    } else {
                        return ($s as i32, msa.region);
                    }
                }
                if msa.retry_limit_in_search != 0
                    && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
                {
                    return (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, msa.region);
                }
            }};
        }

        if reg.optimize != OptimizeType::None {
            // Threshold length check (inside optimize branch, matching C)
            if (end as i32 - range as i32) < reg.threshold_len {
                return (ONIG_MISMATCH, msa.region);
            }

            let adjrange = if range < end {
                left_adjust_char_head(enc, str_data, 0, range)
            } else {
                end
            };

            let min_range = if end.saturating_sub(range) > reg.dist_min as usize {
                range + reg.dist_min as usize
            } else {
                end
            };

            if reg.dist_max != INFINITE_LEN {
                // C: do { ... } while (PTR_GE(s, range));
                // Use usize::MAX as sentinel for C's NULL (past-beginning)
                loop {
                    let sch_start = if end.saturating_sub(s) > reg.dist_max as usize {
                        s + reg.dist_max as usize
                    } else {
                        onigenc_get_prev_char_head(enc, str_data, 0, end)
                    };

                    if let Some((low, high)) = backward_search(reg, str_data, end,
                                                                sch_start, min_range, adjrange) {
                        if s > high { s = high; }
                        while s >= low {
                            backward_match_and_check!(s, orig_start);
                            if s == 0 {
                                s = usize::MAX; // sentinel: past beginning (C returns NULL)
                                break;
                            }
                            s = onigenc_get_prev_char_head(enc, str_data, 0, s);
                        }
                    } else {
                        return finish_search(find_longest, best_start, best_len, reg, str_data, end, &mut msa);
                    }

                    if s == usize::MAX || s < range { break; }
                }
                return finish_search(find_longest, best_start, best_len, reg, str_data, end, &mut msa);
            } else {
                // dist_max == INFINITE_LEN: single backward_search as gate
                let sch_start = onigenc_get_prev_char_head(enc, str_data, 0, end);
                if backward_search(reg, str_data, end, sch_start, min_range, adjrange).is_none() {
                    return (ONIG_MISMATCH, msa.region);
                }
            }
        }

        // Fallthrough: position-by-position loop (optimize == None or infinite dist_max gate passed)
        loop {
            backward_match_and_check!(s, orig_start);
            if s <= range { break; }
            s = onigenc_get_prev_char_head(enc, str_data, 0, s);
        }

        return finish_search(find_longest, best_start, best_len, reg, str_data, end, &mut msa);
    }

    let mut cur_start = start;
    let mut cur_range = range;
    let data_range = if range > start { range } else { end };

    // === Anchor optimization: narrow search range ===
    if reg.anchor != 0 && start < end {
        if (reg.anchor & ANCR_BEGIN_POSITION) != 0 {
            // search start-position only
            if range > start {
                cur_range = start + 1;
            } else {
                cur_range = start;
            }
        } else if (reg.anchor & ANCR_BEGIN_BUF) != 0 {
            // search str-position only (must start at 0)
            if range > start {
                if start != 0 {
                    return (ONIG_MISMATCH, msa.region);
                }
                cur_range = 1;
            } else {
                return (ONIG_MISMATCH, msa.region);
            }
        } else if (reg.anchor & ANCR_END_BUF) != 0 {
            let min_semi_end = end;
            let max_semi_end = end;
            if (max_semi_end as OnigLen) < reg.anc_dist_min {
                return (ONIG_MISMATCH, msa.region);
            }
            if range > start {
                if reg.anc_dist_max != INFINITE_LEN
                    && min_semi_end.saturating_sub(start) > reg.anc_dist_max as usize
                {
                    cur_start = min_semi_end - reg.anc_dist_max as usize;
                }
                if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < reg.anc_dist_min as usize {
                    if max_semi_end + 1 < reg.anc_dist_min as usize {
                        return (ONIG_MISMATCH, msa.region);
                    } else {
                        cur_range = max_semi_end - reg.anc_dist_min as usize + 1;
                    }
                }
                if cur_start > cur_range {
                    return (ONIG_MISMATCH, msa.region);
                }
            }
        } else if (reg.anchor & ANCR_SEMI_END_BUF) != 0 {
            let mut min_semi_end = end;
            let max_semi_end = end;
            // Check if last char before end is newline
            if end > 0 && str_data[end - 1] == b'\n' {
                min_semi_end = end - 1;
            }
            if (max_semi_end as OnigLen) < reg.anc_dist_min {
                return (ONIG_MISMATCH, msa.region);
            }
            if range > start {
                if reg.anc_dist_max != INFINITE_LEN
                    && min_semi_end.saturating_sub(start) > reg.anc_dist_max as usize
                {
                    cur_start = min_semi_end - reg.anc_dist_max as usize;
                }
                if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < reg.anc_dist_min as usize {
                    if max_semi_end + 1 < reg.anc_dist_min as usize {
                        return (ONIG_MISMATCH, msa.region);
                    } else {
                        cur_range = max_semi_end - reg.anc_dist_min as usize + 1;
                    }
                }
                if cur_start > cur_range {
                    return (ONIG_MISMATCH, msa.region);
                }
            }
        } else if (reg.anchor & ANCR_ANYCHAR_INF_ML) != 0 && range > start {
            // start-position only
            if range > start {
                cur_range = start + 1;
            }
        }
    } else if start == end {
        // Empty string
        if reg.threshold_len == 0 {
            let mut s = start;
            if let Some(ref mut r) = msa.region {
                r.resize(reg.num_mem + 1);
                r.clear();
            }
            msa.best_len = ONIG_MISMATCH;
            msa.best_s = 0;
            let r = match_at(reg, str_data, end, end, s, &mut msa);
            if r < ONIG_MISMATCH { return (r, msa.region); } // error
            if r != ONIG_MISMATCH {
                return (s as i32, msa.region);
            }
        }
        return (ONIG_MISMATCH, msa.region);
    }

    // === Threshold length check ===
    if (end as i32 - cur_start as i32) < reg.threshold_len {
        return (ONIG_MISMATCH, msa.region);
    }

    // === Forward search ===
    let mut s = cur_start;

    // Use optimization if available
    if reg.optimize != OptimizeType::None {
        // Calculate search range for optimization
        let sch_range = if reg.dist_max != 0 {
            if reg.dist_max == INFINITE_LEN {
                end
            } else if end.saturating_sub(cur_range) < reg.dist_max as usize {
                end
            } else {
                cur_range + reg.dist_max as usize
            }
        } else {
            cur_range
        };

        if reg.dist_max != INFINITE_LEN {
            // Finite dist_max: iterate with forward_search
            loop {
                let (low, high) = match forward_search(reg, str_data, end, s, sch_range) {
                    Some(lh) => lh,
                    None => break, // mismatch
                };
                if s < low { s = low; }
                while s <= high {
                    if let Some(ref mut r) = msa.region {
                        r.resize(reg.num_mem + 1);
                        r.clear();
                    }
                    msa.best_len = ONIG_MISMATCH;
                    msa.best_s = 0;
                    let r = match_at(reg, str_data, end, data_range, s, &mut msa);
                    if r != ONIG_MISMATCH {
                        if r < 0 { return (r, msa.region); } // error
                        if find_longest {
                            let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                            if best_len == ONIG_MISMATCH || match_len > best_len {
                                best_start = s as i32;
                                best_len = match_len;
                            }
                        } else {
                            return (s as i32, msa.region);
                        }
                    }
                    if msa.retry_limit_in_search != 0
                        && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
                    {
                        return (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, msa.region);
                    }
                    s += enclen(enc, str_data, s);
                }
                if s >= cur_range { break; }
            }
            // C: goto mismatch -- optimized search exhausted, do not fall
            // through to the byte-by-byte loop below.
            return finish_search(find_longest, best_start, best_len, reg,
                                str_data, end, &mut msa);
        } else {
            // Infinite dist_max: just check once, then fall through to normal loop
            if forward_search(reg, str_data, end, s, sch_range).is_none() {
                return finish_search(find_longest, best_start, best_len, reg,
                                    str_data, end, &mut msa);
            }
            // ANCR_ANYCHAR_INF: skip past newlines
            if (reg.anchor & ANCR_ANYCHAR_INF) != 0
                && (reg.anchor & (ANCR_LOOK_BEHIND | ANCR_PREC_READ_NOT)) == 0
            {
                while s < cur_range {
                    if let Some(ref mut r) = msa.region {
                        r.resize(reg.num_mem + 1);
                        r.clear();
                    }
                    msa.best_len = ONIG_MISMATCH;
                    msa.best_s = 0;
                    let r = match_at(reg, str_data, end, data_range, s, &mut msa);
                    if r != ONIG_MISMATCH {
                        if r < 0 { return (r, msa.region); }
                        if find_longest {
                            let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                            if best_len == ONIG_MISMATCH || match_len > best_len {
                                best_start = s as i32;
                                best_len = match_len;
                            }
                        } else {
                            return (s as i32, msa.region);
                        }
                    }
                    if msa.retry_limit_in_search != 0
                        && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
                    {
                        return (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, msa.region);
                    }
                    let prev = s;
                    s += enclen(enc, str_data, s);
                    // Skip past non-newline chars
                    while s < cur_range && !is_mbc_newline(enc, str_data, prev, end) {
                        let prev2 = s;
                        s += enclen(enc, str_data, s);
                        if is_mbc_newline(enc, str_data, prev2, end) { break; }
                    }
                }
                return finish_search(find_longest, best_start, best_len, reg,
                                    str_data, end, &mut msa);
            }
            // Fall through to normal position loop below
        }
    }

    // Normal position-by-position search (no optimization or fallthrough)
    if best_start == ONIG_MISMATCH {
        loop {
            if let Some(ref mut r) = msa.region {
                r.resize(reg.num_mem + 1);
                r.clear();
            }
            msa.best_len = ONIG_MISMATCH;
            msa.best_s = 0;
            let r = match_at(reg, str_data, end, data_range, s, &mut msa);
            if r != ONIG_MISMATCH {
                if r < 0 { return (r, msa.region); }
                if find_longest {
                    let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                    if best_len == ONIG_MISMATCH || match_len > best_len {
                        best_start = s as i32;
                        best_len = match_len;
                    }
                } else {
                    return (s as i32, msa.region);
                }
            }
            if msa.retry_limit_in_search != 0
                && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
            {
                return (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, msa.region);
            }
            if s >= cur_range { break; }
            if s >= end { break; }
            s += enclen(enc, str_data, s);
        }
    }

    finish_search(find_longest, best_start, best_len, reg, str_data, end, &mut msa)
}

fn finish_search(
    find_longest: bool, best_start: i32, best_len: i32,
    reg: &RegexType, str_data: &[u8], end: usize, msa: &mut MatchArg,
) -> (i32, Option<OnigRegion>) {
    if find_longest && best_start != ONIG_MISMATCH {
        if let Some(ref mut r) = msa.region {
            r.resize(reg.num_mem + 1);
            r.clear();
        }
        msa.best_len = ONIG_MISMATCH;
        msa.best_s = 0;
        match_at(reg, str_data, end, end, best_start as usize, msa);
        return (best_start, msa.region.take());
    }
    (ONIG_MISMATCH, msa.region.take())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regcomp;
    use crate::regparse;
    use crate::regparse_types::ParseEnv;
    use crate::regsyntax;

    fn make_test_context() -> (RegexType, ParseEnv) {
        use crate::regsyntax::OnigSyntaxOniguruma;
        let enc: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
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
            enc,
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
            dist_min: 0,
            dist_max: 0,
            called_addrs: vec![],
            unset_call_addrs: vec![],
            extp: None,
        };
        let env = ParseEnv {
            options: 0,
            case_fold_flag: 0,
            enc,
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

    fn compile_and_match(pattern: &[u8], input: &[u8]) -> (i32, Option<OnigRegion>) {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0, "compile failed for {:?}", std::str::from_utf8(pattern));
        onig_match(&reg, input, input.len(), 0, Some(OnigRegion::new()), ONIG_OPTION_NONE)
    }

    fn compile_and_search(pattern: &[u8], input: &[u8]) -> (i32, Option<OnigRegion>) {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0, "compile failed for {:?}", std::str::from_utf8(pattern));
        onig_search(
            &reg,
            input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        )
    }

    // ---- Basic literal matching ----

    #[test]
    fn match_literal_abc() {
        let (r, _) = compile_and_match(b"abc", b"abc");
        assert_eq!(r, 3); // matched 3 bytes
    }

    #[test]
    fn match_literal_fail() {
        let (r, _) = compile_and_match(b"abc", b"abd");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_literal_too_short() {
        let (r, _) = compile_and_match(b"abcd", b"abc");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_empty_pattern() {
        let (r, _) = compile_and_match(b"", b"abc");
        assert_eq!(r, 0); // empty pattern matches with length 0
    }

    #[test]
    fn match_single_char() {
        let (r, _) = compile_and_match(b"x", b"xyz");
        assert_eq!(r, 1);
    }


    // ---- Dot (anychar) ----

    #[test]
    fn match_dot() {
        let (r, _) = compile_and_match(b"a.c", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_dot_no_newline() {
        let (r, _) = compile_and_match(b"a.c", b"a\nc");
        assert_eq!(r, ONIG_MISMATCH); // dot doesn't match newline
    }

    // ---- Alternation ----

    #[test]
    fn match_alternation_first() {
        let (r, _) = compile_and_match(b"a|b", b"a");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_alternation_second() {
        let (r, _) = compile_and_match(b"a|b", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_alternation_fail() {
        let (r, _) = compile_and_match(b"a|b", b"c");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Quantifiers ----

    #[test]
    fn match_star() {
        let (r, _) = compile_and_match(b"a*", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_star_empty() {
        let (r, _) = compile_and_match(b"a*", b"bbb");
        assert_eq!(r, 0); // a* can match empty
    }

    #[test]
    fn match_plus() {
        let (r, _) = compile_and_match(b"a+", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_plus_fail() {
        let (r, _) = compile_and_match(b"a+", b"bbb");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_question() {
        let (r, _) = compile_and_match(b"a?b", b"ab");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_question_without() {
        let (r, _) = compile_and_match(b"a?b", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_lazy_star() {
        // a*? should match as few as possible from position 0
        let (r, _) = compile_and_match(b"a*?", b"aaa");
        assert_eq!(r, 0); // lazy: match empty
    }

    // ---- Character classes ----

    #[test]
    fn match_char_class() {
        let (r, _) = compile_and_match(b"[abc]", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_fail() {
        let (r, _) = compile_and_match(b"[abc]", b"d");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_char_class_range() {
        let (r, _) = compile_and_match(b"[a-z]", b"m");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_negated() {
        let (r, _) = compile_and_match(b"[^abc]", b"d");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_negated_fail() {
        let (r, _) = compile_and_match(b"[^abc]", b"a");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Anchors ----

    #[test]
    fn match_begin_anchor() {
        let (r, _) = compile_and_match(b"^abc", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_end_anchor() {
        let (r, _) = compile_and_match(b"abc$", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_begin_end_anchors() {
        let (r, _) = compile_and_match(b"^abc$", b"abc");
        assert_eq!(r, 3);
    }

    // ---- Capture groups ----

    #[test]
    fn match_capture_group() {
        let (r, region) = compile_and_match(b"(abc)", b"abc");
        assert_eq!(r, 3);
        let region = region.unwrap();
        assert!(region.num_regs >= 2);
        assert_eq!(region.beg[0], 0);
        assert_eq!(region.end[0], 3);
        assert_eq!(region.beg[1], 0);
        assert_eq!(region.end[1], 3);
    }

    #[test]
    fn match_multiple_captures() {
        let (r, region) = compile_and_match(b"(a)(b)(c)", b"abc");
        assert_eq!(r, 3);
        let region = region.unwrap();
        assert!(region.num_regs >= 4);
        // Group 0: full match
        assert_eq!(region.beg[0], 0);
        assert_eq!(region.end[0], 3);
        // Group 1: a
        assert_eq!(region.beg[1], 0);
        assert_eq!(region.end[1], 1);
        // Group 2: b
        assert_eq!(region.beg[2], 1);
        assert_eq!(region.end[2], 2);
        // Group 3: c
        assert_eq!(region.beg[3], 2);
        assert_eq!(region.end[3], 3);
    }

    #[test]
    fn match_non_capturing_group() {
        let (r, _) = compile_and_match(b"(?:abc)", b"abc");
        assert_eq!(r, 3);
    }

    // ---- Search (find anywhere in string) ----

    #[test]
    fn search_literal() {
        let (pos, _) = compile_and_search(b"bc", b"abcdef");
        assert_eq!(pos, 1); // found at position 1
    }

    #[test]
    fn search_literal_not_found() {
        let (pos, _) = compile_and_search(b"xyz", b"abcdef");
        assert_eq!(pos, ONIG_MISMATCH);
    }

    #[test]
    fn search_at_start() {
        let (pos, _) = compile_and_search(b"abc", b"abcdef");
        assert_eq!(pos, 0);
    }

    #[test]
    fn search_at_end() {
        let (pos, _) = compile_and_search(b"ef", b"abcdef");
        assert_eq!(pos, 4);
    }

    #[test]
    fn search_with_captures() {
        let (pos, region) = compile_and_search(b"(b)(c)", b"abcdef");
        assert_eq!(pos, 1);
        let region = region.unwrap();
        assert_eq!(region.beg[1], 1);
        assert_eq!(region.end[1], 2);
        assert_eq!(region.beg[2], 2);
        assert_eq!(region.end[2], 3);
    }

    #[test]
    fn search_with_quantifier() {
        let (pos, _) = compile_and_search(b"a+", b"bbaab");
        assert_eq!(pos, 2);
    }

    // ---- Complex patterns ----

    #[test]
    fn match_word_boundary() {
        let (pos, _) = compile_and_search(b"\\bfoo\\b", b"a foo b");
        assert_eq!(pos, 2);
    }

    #[test]
    fn match_complex_alternation() {
        let (r, _) = compile_and_match(b"abc|def|ghi", b"def");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_nested_groups() {
        let (r, region) = compile_and_match(b"((a)(b))", b"ab");
        assert_eq!(r, 2);
        let region = region.unwrap();
        // Group 1: ab
        assert_eq!(region.beg[1], 0);
        assert_eq!(region.end[1], 2);
        // Group 2: a
        assert_eq!(region.beg[2], 0);
        assert_eq!(region.end[2], 1);
        // Group 3: b
        assert_eq!(region.beg[3], 1);
        assert_eq!(region.end[3], 2);
    }

    #[test]
    fn match_dot_star() {
        let (r, _) = compile_and_match(b"a.*b", b"aXXXb");
        assert_eq!(r, 5);
    }

    #[test]
    fn match_backtracking() {
        // Pattern: a.*b matches "axb" - requires backtracking
        let (r, _) = compile_and_match(b"a.*b", b"axb");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_quantifier() {
        let (r, _) = compile_and_match(b"a{3}", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_quantifier_fail() {
        let (r, _) = compile_and_match(b"a{3}", b"aa");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_interval_range() {
        let (r, _) = compile_and_match(b"a{2,4}", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_range_min() {
        let (r, _) = compile_and_match(b"a{2,4}", b"aa");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_digit_class() {
        let (r, _) = compile_and_match(b"\\d+", b"123");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_word_class() {
        let (r, _) = compile_and_match(b"\\w+", b"abc123_");
        assert_eq!(r, 7);
    }

    #[test]
    fn search_email_like() {
        let (pos, _) = compile_and_search(b"\\w+@\\w+", b"send to user@host ok");
        assert_eq!(pos, 8); // "user@host" starts at position 8
    }

    #[test]
    fn match_escaped_special() {
        let (r, _) = compile_and_match(b"a\\.b", b"a.b");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_back_reference() {
        let (r, _) = compile_and_match(b"(a)\\1", b"aa");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_back_reference_fail() {
        let (r, _) = compile_and_match(b"(a)\\1", b"ab");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Safety limits ----

    // Safety limit tests use a lock to avoid interfering with each other
    // (since limits are global statics).
    use std::sync::Mutex;
    static LIMIT_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn retry_limit_in_match() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();

        // (a*)*b against "aaa..." causes catastrophic backtracking
        let (mut reg, mut env) = make_test_context();
        let pattern = b"(a*)*b";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"aaaaaaaaaa"; // 10 'a's, no 'b'

        // Save old limits, set low retry limit
        let old_retry = onig_get_retry_limit_in_match();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        onig_set_retry_limit_in_match(100);
        onig_set_match_stack_limit(0); // unlimited
        onig_set_time_limit(0); // unlimited

        let (result, _) = onig_search(
            &reg, input, input.len(), 0, input.len(),
            Some(OnigRegion::new()), ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIGERR_RETRY_LIMIT_IN_MATCH_OVER);

        // Restore
        onig_set_retry_limit_in_match(old_retry);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);
    }

    #[test]
    fn stack_limit_over() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();

        // Use onig_match directly to avoid forward_search optimization
        let (mut reg, mut env) = make_test_context();
        let pattern = b".*.*.*.*.*";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; // 30 chars

        // Set very low stack limit, disable retry limit
        let old_retry = onig_get_retry_limit_in_match();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        onig_set_retry_limit_in_match(0); // unlimited
        onig_set_match_stack_limit(20);
        onig_set_time_limit(0); // unlimited

        let (result, _) = onig_match(
            &reg, input, input.len(), 0,
            Some(OnigRegion::new()), ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIGERR_MATCH_STACK_LIMIT_OVER);

        // Restore
        onig_set_retry_limit_in_match(old_retry);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);
    }

    #[test]
    fn time_limit_over() {
        let _lock = LIMIT_TEST_LOCK.lock().unwrap();

        let (mut reg, mut env) = make_test_context();
        let pattern = b"(a*)*b";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; // 30 'a's

        // Set 1ms time limit, disable other limits
        let old_retry = onig_get_retry_limit_in_match();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        onig_set_retry_limit_in_match(0); // unlimited
        onig_set_match_stack_limit(0); // unlimited
        onig_set_time_limit(1);

        let (result, _) = onig_search(
            &reg, input, input.len(), 0, input.len(),
            Some(OnigRegion::new()), ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIGERR_TIME_LIMIT_OVER);

        // Restore
        onig_set_retry_limit_in_match(old_retry);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);
    }

    #[test]
    fn limits_zero_means_unlimited() {
        // Verify default limits (0 = unlimited) don't interfere with normal matching
        let (r, _) = compile_and_match(b"a*b", b"aaab");
        assert_eq!(r, 4);
    }

    // ---- Backward search ----

    #[test]
    fn backward_search_basic() {
        // Search backward from end to find last occurrence
        let (mut reg, mut env) = make_test_context();
        let pattern = b"ab";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"xxabxxabxx"; // "ab" at positions 2 and 6
        // Backward search: start=10 (end), range=0 (beginning)
        let (result, _) = onig_search(
            &reg, input, input.len(), input.len(), 0,
            Some(OnigRegion::new()), ONIG_OPTION_NONE,
        );
        // Should find the last "ab" at position 6
        assert_eq!(result, 6);
    }

    #[test]
    fn backward_search_at_start() {
        let (mut reg, mut env) = make_test_context();
        let pattern = b"he";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"hello";
        let (result, _) = onig_search(
            &reg, input, input.len(), input.len(), 0,
            Some(OnigRegion::new()), ONIG_OPTION_NONE,
        );
        assert_eq!(result, 0);
    }

    #[test]
    fn backward_search_no_match() {
        let (mut reg, mut env) = make_test_context();
        let pattern = b"xyz";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"hello world";
        let (result, _) = onig_search(
            &reg, input, input.len(), input.len(), 0,
            Some(OnigRegion::new()), ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIG_MISMATCH);
    }
}
