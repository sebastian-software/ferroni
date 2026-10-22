//! Port of `regexec.c`: the bytecode interpreter, with [`onig_match`] and
//! [`onig_search`]. [`Regex`](crate::api::Regex) is the idiomatic search API.

// This is a 1:1 port of oniguruma's regexec.c (~7,000 LOC).
// Structure mirrors the C original: stack types → stack operations →
// match_at (opcode dispatch) → onig_match → onig_search.

#![allow(non_upper_case_globals)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_mut)]

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{PoisonError, RwLock};
use std::time::Instant;

use crate::backtrack_rewrite::runtime::{
    decimal_tail_prefix_bounds, record_decimal_prefix_captures,
};
use crate::first_bytes::start_map_may_skip;
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
/// Monotonically changing revision for consumers that cache the four global
/// limits. Setters publish their preceding relaxed limit stores with a Release
/// RMW; an Acquire revision read that observes that change may then reload all
/// four limits relaxed. This avoids four atomic loads on hot searches when no
/// caller has changed a global limit.
static GLOBAL_LIMIT_REVISION: AtomicU64 = AtomicU64::new(0);

/// Keeps the unit tests that change the process-wide limits apart from the
/// unit tests whose searches read them. A test that changes a limit holds
/// [`exclusive_limits`] for its whole run; every other test that searches, or
/// reads the limit revision, holds [`shared_limits`]. Without it the default
/// parallel test runner lets a changed limit leak into unrelated searches.
/// The callout callbacks cannot be unset, so the test that sets them runs in a
/// process of its own, in `tests/global_callouts.rs`.
#[cfg(test)]
static LIMIT_TEST_LOCK: RwLock<()> = RwLock::new(());

/// Held by a unit test whose searches read the process-wide limits, so that
/// none of them changes while it runs.
#[cfg(test)]
pub(crate) fn shared_limits() -> std::sync::RwLockReadGuard<'static, ()> {
    // A failed test restored the limits when it released the lock.
    LIMIT_TEST_LOCK
        .read()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Held by a unit test that changes a process-wide limit; see
/// [`exclusive_limits`].
#[cfg(test)]
pub(crate) struct ExclusiveLimits {
    saved: (u64, u64, u32, u64, u64, i32),
    _lock: std::sync::RwLockWriteGuard<'static, ()>,
}

/// Held by a unit test that changes a process-wide limit. Dropping the guard
/// restores the limits it found, so a test that fails changes none of them for
/// the tests after it.
#[cfg(test)]
pub(crate) fn exclusive_limits() -> ExclusiveLimits {
    let lock = LIMIT_TEST_LOCK
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    ExclusiveLimits {
        saved: (
            onig_get_retry_limit_in_match(),
            onig_get_retry_limit_in_search(),
            onig_get_match_stack_limit(),
            onig_get_time_limit(),
            onig_get_subexp_call_limit_in_search(),
            onig_get_subexp_call_max_nest_level(),
        ),
        _lock: lock,
    }
}

#[cfg(test)]
impl Drop for ExclusiveLimits {
    fn drop(&mut self) {
        let (retry_in_match, retry_in_search, stack, time, calls, nest) = self.saved;
        onig_set_retry_limit_in_match(retry_in_match);
        onig_set_retry_limit_in_search(retry_in_search);
        onig_set_match_stack_limit(stack);
        onig_set_time_limit(time);
        onig_set_subexp_call_limit_in_search(calls);
        onig_set_subexp_call_max_nest_level(nest);
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_retry_limit_in_match(n: u64) {
    RETRY_LIMIT_IN_MATCH.store(n, Ordering::Relaxed);
    GLOBAL_LIMIT_REVISION.fetch_add(1, Ordering::Release);
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_retry_limit_in_match() -> u64 {
    RETRY_LIMIT_IN_MATCH.load(Ordering::Relaxed)
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_retry_limit_in_search(n: u64) {
    RETRY_LIMIT_IN_SEARCH.store(n, Ordering::Relaxed);
    GLOBAL_LIMIT_REVISION.fetch_add(1, Ordering::Release);
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_retry_limit_in_search() -> u64 {
    RETRY_LIMIT_IN_SEARCH.load(Ordering::Relaxed)
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_match_stack_limit(n: u32) {
    MATCH_STACK_LIMIT.store(n, Ordering::Relaxed);
    GLOBAL_LIMIT_REVISION.fetch_add(1, Ordering::Release);
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_match_stack_limit() -> u32 {
    MATCH_STACK_LIMIT.load(Ordering::Relaxed)
}

/// Alias for `onig_set_match_stack_limit` (C API compatibility).
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_match_stack_limit_size(n: u32) {
    onig_set_match_stack_limit(n);
}
/// Alias for `onig_get_match_stack_limit` (C API compatibility).
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_match_stack_limit_size() -> u32 {
    onig_get_match_stack_limit()
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_time_limit(n: u64) {
    TIME_LIMIT.store(n, Ordering::Relaxed);
    GLOBAL_LIMIT_REVISION.fetch_add(1, Ordering::Release);
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_time_limit() -> u64 {
    TIME_LIMIT.load(Ordering::Relaxed)
}

/// Internal revision for caches of the process-global match limits.
#[inline]
pub(crate) fn onig_get_global_limit_revision() -> u64 {
    // Paired with the setters' Release RMWs: if this observes a new
    // revision, following relaxed limit loads see the limits published before
    // it. u64 wrapping is intentionally left to atomic arithmetic; reaching
    // it would require 2^64 process-global limit updates.
    GLOBAL_LIMIT_REVISION.load(Ordering::Acquire)
}

// ============================================================================
// Global Progress/Retraction Callout (port of C's global callout funcs)
// ============================================================================

/// Process-wide storage for one optional callback.
///
/// `is_set` lets the common case, a callback that was never set, return `None`
/// after a single atomic load. That matters because
/// `onig_initialize_match_param` reads both callouts each time it builds a
/// match parameter. The callback itself sits behind a `RwLock`, which is taken
/// only once `is_set` has been observed. The setter writes the lock before it
/// publishes the flag, so a reader that sees the flag also sees the callback.
struct GlobalCallbackSlot<F: Copy> {
    is_set: AtomicBool,
    callback: RwLock<Option<F>>,
}

impl<F: Copy> GlobalCallbackSlot<F> {
    const fn new() -> Self {
        GlobalCallbackSlot {
            is_set: AtomicBool::new(false),
            callback: RwLock::new(None),
        }
    }

    #[inline]
    fn get(&self) -> Option<F> {
        if !self.is_set.load(Ordering::Acquire) {
            return None;
        }
        *self.callback.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, f: F) {
        let mut callback = self
            .callback
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        *callback = Some(f);
        self.is_set.store(true, Ordering::Release);
    }
}

static PROGRESS_CALLOUT: GlobalCallbackSlot<OnigCalloutFunc> = GlobalCallbackSlot::new();
static RETRACTION_CALLOUT: GlobalCallbackSlot<OnigCalloutFunc> = GlobalCallbackSlot::new();

/// Get the global progress callout function.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_progress_callout() -> Option<OnigCalloutFunc> {
    PROGRESS_CALLOUT.get()
}

/// Set the global progress callout function.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_progress_callout(f: OnigCalloutFunc) -> i32 {
    PROGRESS_CALLOUT.set(f);
    ONIG_NORMAL
}

/// Get the global retraction callout function.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_retraction_callout() -> Option<OnigCalloutFunc> {
    RETRACTION_CALLOUT.get()
}

/// Set the global retraction callout function.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_retraction_callout(f: OnigCalloutFunc) -> i32 {
    RETRACTION_CALLOUT.set(f);
    ONIG_NORMAL
}

// ============================================================================
// Global Callback Each Match (port of C's CallbackEachMatch)
// ============================================================================

pub type OnigCallbackEachMatchFunc =
    fn(str_data: &[u8], region: &OnigRegion, user_data: *mut std::ffi::c_void) -> i32;

static CALLBACK_EACH_MATCH: GlobalCallbackSlot<OnigCallbackEachMatchFunc> =
    GlobalCallbackSlot::new();

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callback_each_match() -> Option<OnigCallbackEachMatchFunc> {
    CALLBACK_EACH_MATCH.get()
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_callback_each_match(f: OnigCallbackEachMatchFunc) -> i32 {
    CALLBACK_EACH_MATCH.set(f);
    ONIG_NORMAL
}

// ============================================================================
// Region Management (port of C's onig_region_* functions)
// ============================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_region_new() -> OnigRegion {
    OnigRegion::new()
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_region_init(region: &mut OnigRegion) {
    region.init();
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_region_clear(region: &mut OnigRegion) {
    region.clear();
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_region_resize(region: &mut OnigRegion, n: i32) -> i32 {
    region.resize(n);
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_region_set(region: &mut OnigRegion, at: i32, beg: i32, end: i32) -> i32 {
    region.set(at, beg, end)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_region_copy(to: &mut OnigRegion, from: &OnigRegion) {
    to.copy_from(from);
}

// ============================================================================
// Regex Accessors (port of C's onig_get_*/onig_number_of_* functions)
// ============================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_encoding(reg: &RegexType) -> OnigEncoding {
    reg.enc
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_options(reg: &RegexType) -> OnigOptionType {
    reg.options
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_case_fold_flag(reg: &RegexType) -> OnigCaseFoldType {
    reg.case_fold_flag
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_syntax(reg: &RegexType) -> &OnigSyntaxType {
    &reg.syntax
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_number_of_captures(reg: &RegexType) -> i32 {
    reg.num_mem
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_number_of_capture_histories(reg: &RegexType) -> i32 {
    let mut n = 0;
    for i in 0..=ONIG_MAX_CAPTURE_HISTORY_GROUP {
        if mem_status_at(reg.capture_history, i) {
            n += 1;
        }
    }
    n
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_capture_tree(region: &OnigRegion) -> Option<&OnigCaptureTreeNode> {
    region.history_root.as_deref()
}

// ============================================================================
// Name Table Queries (port of C's onig_name_to_* / onig_foreach_name)
// ============================================================================

/// Returns the number of group numbers for the given name, and a slice of them.
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_number_of_names(reg: &RegexType) -> i32 {
    if let Some(ref nt) = reg.name_table {
        nt.entries.len() as i32
    } else {
        0
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_noname_group_capture_is_active(reg: &RegexType) -> bool {
    if opton_dont_capture_group(reg.options) {
        return false;
    }
    if onig_number_of_names(reg) > 0 {
        if is_syntax_bv(&reg.syntax, ONIG_SYN_CAPTURE_ONLY_NAMED_GROUP)
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

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_version() -> &'static str {
    "6.9.10"
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_copyright() -> &'static str {
    "Oniguruma 6.9.10 : Copyright (C) 2002-2024 K.Kosako"
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_init() -> i32 {
    ONIG_NORMAL
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_initialize() -> i32 {
    ONIG_NORMAL
}
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_end() -> i32 {
    ONIG_NORMAL
}

static SUBEXP_CALL_LIMIT_IN_SEARCH: AtomicU64 = AtomicU64::new(DEFAULT_SUBEXP_CALL_LIMIT_IN_SEARCH);
static SUBEXP_CALL_MAX_NEST_LEVEL: AtomicU32 =
    AtomicU32::new(DEFAULT_SUBEXP_CALL_MAX_NEST_LEVEL as u32);

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_subexp_call_limit_in_search() -> u64 {
    SUBEXP_CALL_LIMIT_IN_SEARCH.load(Ordering::Relaxed)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_subexp_call_limit_in_search(n: u64) -> i32 {
    SUBEXP_CALL_LIMIT_IN_SEARCH.store(n, Ordering::Relaxed);
    // Rust-only: the RegSet caches the limits it reads (ADR-008's DFA
    // pre-filter stays off under a call budget) and reloads them on the bump.
    GLOBAL_LIMIT_REVISION.fetch_add(1, Ordering::Release);
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_subexp_call_max_nest_level() -> i32 {
    SUBEXP_CALL_MAX_NEST_LEVEL.load(Ordering::Relaxed) as i32
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_subexp_call_max_nest_level(level: i32) -> i32 {
    SUBEXP_CALL_MAX_NEST_LEVEL.store(level as u32, Ordering::Relaxed);
    ONIG_NORMAL
}

/// Scan for all non-overlapping matches of `reg` in `str_data`.
/// For each match, calls `callback(match_count, match_position, region)`.
/// If callback returns non-zero, scanning stops and that value is returned.
/// Otherwise returns the total number of matches found.
#[cfg_attr(coverage_nightly, coverage(off))]
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
        let (r, returned_region) =
            onig_search(reg, str_data, end, start, end, Some(region), option);
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_match_stack_limit_size_of_match_param(mp: &mut OnigMatchParam, limit: u32) -> i32 {
    mp.match_stack_limit = limit;
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_retry_limit_in_match_of_match_param(mp: &mut OnigMatchParam, limit: u64) -> i32 {
    mp.retry_limit_in_match = limit;
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_retry_limit_in_search_of_match_param(mp: &mut OnigMatchParam, limit: u64) -> i32 {
    mp.retry_limit_in_search = limit;
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_time_limit_of_match_param(mp: &mut OnigMatchParam, limit: u64) -> i32 {
    mp.time_limit = limit;
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_progress_callout_of_match_param(
    mp: &mut OnigMatchParam,
    f: Option<OnigCalloutFunc>,
) -> i32 {
    mp.progress_callout = f;
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_retraction_callout_of_match_param(
    mp: &mut OnigMatchParam,
    f: Option<OnigCalloutFunc>,
) -> i32 {
    mp.retraction_callout = f;
    ONIG_NORMAL
}

#[cfg_attr(coverage_nightly, coverage(off))]
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
///
/// The accessors reach the regex through `reg`, a shared reference that these
/// arguments borrow for `'a`. The raw pointer fields mirror C's
/// `OnigCalloutArgsStruct` for callers that want the positions. This module
/// stores and returns them and never dereferences them.
pub struct OnigCalloutArgs<'a> {
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
    // The regex behind `regex`, borrowed for the lifetime of the arguments.
    reg: &'a RegexType,
    // Byte offset of `current` in the subject, as the SKIP builtin records it.
    current_offset: usize,
    // Callout data array (for by_callout_args accessor functions).
    callout_data: Option<&'a RefCell<Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>>>,
    // Skip target of the running search, written back by the SKIP builtin.
    skip_search: Option<&'a Cell<usize>>,
}

impl<'a> OnigCalloutArgs<'a> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        callout_in: OnigCalloutIn,
        name_id: i32,
        num: i32,
        reg: &'a RegexType,
        str_data: &[u8],
        end: usize,
        start: usize,
        right_range: usize,
        current: usize,
        retry_counter: u64,
    ) -> Self {
        // wrapping_add yields the same pointer as add for the in-bounds
        // offsets the match engine passes, and it performs no memory access.
        let base = str_data.as_ptr();
        OnigCalloutArgs {
            callout_in,
            name_id,
            num,
            regex: reg as *const RegexType,
            string: base,
            string_end: base.wrapping_add(end),
            start: base.wrapping_add(start),
            right_range: base.wrapping_add(right_range),
            current: base.wrapping_add(current),
            retry_in_match_counter: retry_counter,
            reg,
            current_offset: current,
            callout_data: None,
            skip_search: None,
        }
    }

    /// Attaches the match engine's callout data vector, which the
    /// `*_by_callout_args` accessors and the count, max and cmp builtins use.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub(crate) fn with_callout_data(
        mut self,
        callout_data: &'a RefCell<Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>>,
    ) -> Self {
        self.callout_data = Some(callout_data);
        self
    }

    /// Attaches the skip target of the running search, which the SKIP builtin
    /// advances.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub(crate) fn with_skip_search(mut self, skip_search: &'a Cell<usize>) -> Self {
        self.skip_search = Some(skip_search);
        self
    }
}

// --- OnigCalloutArgs accessor functions ---

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_num_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    args.num
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_in_by_callout_args(args: &OnigCalloutArgs) -> OnigCalloutIn {
    args.callout_in
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_name_id_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    args.name_id
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_contents_by_callout_args<'a>(args: &OnigCalloutArgs<'a>) -> Option<&'a [u8]> {
    let ext = args.reg.extp.as_ref()?;
    let idx = (args.num - 1) as usize;
    let entry = ext.callout_list.get(idx)?;
    (entry.of == OnigCalloutOf::Contents as i32)
        .then_some(entry.content.as_deref())
        .flatten()
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_args_num_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    let reg = args.reg;
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].args.len() as i32;
        }
    }
    0
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_passed_args_num_by_callout_args(args: &OnigCalloutArgs) -> i32 {
    let reg = args.reg;
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].passed_args_num;
        }
    }
    0
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_arg_by_callout_args<'a>(
    args: &OnigCalloutArgs<'a>,
    index: i32,
) -> Option<&'a CalloutArg> {
    let reg = args.reg;
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

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_string_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.string
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_string_end_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.string_end
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_start_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.start
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_right_range_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.right_range
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_current_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    args.current
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_regex_by_callout_args(args: &OnigCalloutArgs) -> *const RegexType {
    args.regex
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_retry_counter_by_callout_args(args: &OnigCalloutArgs) -> u64 {
    args.retry_in_match_counter
}

/// Get the end position of callout contents.
/// For content callouts (?{...}), returns the end pointer of the content.
/// Returns null for name callouts.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_contents_end_by_callout_args(args: &OnigCalloutArgs) -> *const u8 {
    let reg = args.reg;
    if let Some(ref ext) = reg.extp {
        let idx = (args.num - 1) as usize;
        if idx < ext.callout_list.len() {
            let entry = &ext.callout_list[idx];
            if entry.of == OnigCalloutOf::Contents as i32 {
                if let Some(ref content) = entry.content {
                    return content.as_ptr().wrapping_add(content.len());
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Get the callout number for a given tag in the regex - mirrors C's
/// onig_get_callout_num_by_tag(), which reports an unknown tag as
/// ONIGERR_INVALID_CALLOUT_TAG_NAME.
pub fn onig_get_callout_num_by_tag(reg: &RegexType, tag: &[u8]) -> i32 {
    if let Some(ref ext) = reg.extp {
        if let Some(ref table) = ext.tag_table {
            if let Some(&num) = table.get(tag) {
                return num;
            }
        }
    }
    ONIGERR_INVALID_CALLOUT_TAG_NAME
}

/// Check if a callout has a tag.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_callout_tag_is_exist_at_callout_num(reg: &RegexType, callout_num: i32) -> bool {
    if let Some(ref ext) = reg.extp {
        let idx = (callout_num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].tag.is_some();
        }
    }
    false
}

/// Get the tag for a given callout number.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn onig_get_callout_tag(reg: &RegexType, callout_num: i32) -> Option<&[u8]> {
    if let Some(ref ext) = reg.extp {
        let idx = (callout_num - 1) as usize;
        if idx < ext.callout_list.len() {
            return ext.callout_list[idx].tag.as_deref();
        }
    }
    None
}

/// Get the byte offset of the tag start in the pattern for a given callout number.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_tag_start(reg: &RegexType, callout_num: i32) -> Option<usize> {
    if let Some(ref ext) = reg.extp {
        let idx = (callout_num - 1) as usize;
        if idx < ext.callout_list.len() && ext.callout_list[idx].tag.is_some() {
            return Some(ext.callout_list[idx].tag_start);
        }
    }
    None
}

/// Get the byte offset of the tag end in the pattern for a given callout number.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_tag_end(reg: &RegexType, callout_num: i32) -> Option<usize> {
    if let Some(ref ext) = reg.extp {
        let idx = (callout_num - 1) as usize;
        if idx < ext.callout_list.len() && ext.callout_list[idx].tag.is_some() {
            return Some(ext.callout_list[idx].tag_end);
        }
    }
    None
}

// ============================================================================
// Callout Data by_callout_args variants
// (port of C's onig_get/set_callout_data_by_callout_args*)
// ============================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_data_by_callout_args(
    args: &OnigCalloutArgs,
    callout_num: i32,
    slot: i32,
) -> Option<i64> {
    let callout_data = args.callout_data?;
    let cd = callout_data.borrow();
    onig_get_callout_data(args.reg, &cd, callout_num, slot)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_callout_data_by_callout_args(
    args: &OnigCalloutArgs,
    callout_num: i32,
    slot: i32,
    val: i64,
) -> i32 {
    let Some(callout_data) = args.callout_data else {
        return ONIGERR_INVALID_ARGUMENT;
    };
    let mut cd = callout_data.borrow_mut();
    onig_set_callout_data(&mut cd, callout_num, slot, val)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_data_by_callout_args_self(
    args: &OnigCalloutArgs,
    slot: i32,
) -> Option<i64> {
    onig_get_callout_data_by_callout_args(args, args.num, slot)
}

#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_data_dont_clear_old(
    reg: &RegexType,
    callout_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    callout_num: i32,
    slot: i32,
) -> Option<i64> {
    onig_get_callout_data(reg, callout_data, callout_num, slot)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_data_by_callout_args_self_dont_clear_old(
    args: &OnigCalloutArgs,
    slot: i32,
) -> Option<i64> {
    onig_get_callout_data_by_callout_args_self(args, slot)
}

/// Get callout data by tag name.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_callout_data_by_tag(
    reg: &RegexType,
    callout_data: &[[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    tag: &[u8],
    slot: i32,
) -> Option<i64> {
    let num = onig_get_callout_num_by_tag(reg, tag);
    if num < 1 {
        return None;
    }
    onig_get_callout_data(reg, callout_data, num, slot)
}

/// Set callout data by tag name.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_set_callout_data_by_tag(
    reg: &RegexType,
    callout_data: &mut [[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    tag: &[u8],
    slot: i32,
    val: i64,
) -> i32 {
    let num = onig_get_callout_num_by_tag(reg, tag);
    if num < 1 {
        return ONIGERR_INVALID_CALLOUT_TAG_NAME;
    }
    onig_set_callout_data(callout_data, num, slot, val)
}

/// Get callout data by tag name without clearing old values.
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_get_used_stack_size_in_callout(_args: &OnigCalloutArgs) -> (i32, i32) {
    // Stack size tracking not yet exposed through OnigCalloutArgs.
    (0, 0)
}

// ============================================================================
// Builtin Callout Public API
// (port of C's onig_builtin_fail, onig_builtin_mismatch, etc.)
// ============================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_fail(_args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    ONIG_CALLOUT_FAIL
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_mismatch(_args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    ONIG_MISMATCH
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_error(args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    let reg = args.reg;
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

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_count(args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    let Some(callout_data) = args.callout_data else {
        return ONIG_CALLOUT_FAIL;
    };
    let mut cd = callout_data.borrow_mut();
    let num = args.num;
    if num < 1 {
        return ONIG_CALLOUT_FAIL;
    }
    let idx = (num - 1) as usize;
    if idx >= cd.len() {
        return ONIG_CALLOUT_FAIL;
    }

    let reg = args.reg;
    let count_type = if let Some(ref ext) = reg.extp {
        if idx < ext.callout_list.len() && !ext.callout_list[idx].args.is_empty() {
            match &ext.callout_list[idx].args[0] {
                CalloutArg::Char(c) => *c,
                _ => b'>',
            }
        } else {
            b'>'
        }
    } else {
        b'>'
    };

    let is_retraction = args.callout_in == OnigCalloutIn::Retraction;
    let slots = &mut cd[idx];

    if is_retraction {
        if count_type == b'<' {
            slots[0] += 1;
        } else if count_type == b'X' {
            slots[0] -= 1;
        }
        slots[2] += 1;
    } else {
        if count_type != b'<' {
            slots[0] += 1;
        }
        slots[1] += 1;
    }

    ONIG_CALLOUT_SUCCESS
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_total_count(args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    // total_count is the same as count but without clearing old data.
    // In Rust, count already doesn't clear old data, so they are equivalent.
    onig_builtin_count(args, _user_data)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_max(args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    let Some(callout_data) = args.callout_data else {
        return ONIG_CALLOUT_FAIL;
    };
    let mut cd = callout_data.borrow_mut();
    let num = args.num;
    if num < 1 {
        return ONIG_CALLOUT_FAIL;
    }
    let idx = (num - 1) as usize;
    if idx >= cd.len() {
        return ONIG_CALLOUT_FAIL;
    }

    let reg = args.reg;
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return ONIG_CALLOUT_FAIL,
    };
    if idx >= ext.callout_list.len() {
        return ONIG_CALLOUT_FAIL;
    }
    let entry = &ext.callout_list[idx];

    let max_val = if !entry.args.is_empty() {
        resolve_callout_arg(&entry.args[0], &ext.callout_list, &cd)
    } else {
        0
    };

    let count_type = if entry.args.len() > 1 {
        match &entry.args[1] {
            CalloutArg::Char(c) => *c,
            _ => b'>',
        }
    } else {
        b'>'
    };

    let is_retraction = args.callout_in == OnigCalloutIn::Retraction;
    let slots = &mut cd[idx];

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

/// Built-in SKIP callout. When a match fails, the search loop advances
/// to at least the position recorded by SKIP, avoiding redundant attempts.
/// Port of C's onig_builtin_skip (regexec.c:6642-6648).
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_skip(args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    if args.callout_in == OnigCalloutIn::Retraction {
        return ONIG_CALLOUT_SUCCESS;
    }

    if let Some(skip_search) = args.skip_search {
        let current = args.current_offset;
        if current > skip_search.get() {
            skip_search.set(current);
        }
    }

    ONIG_CALLOUT_SUCCESS
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_builtin_cmp(args: &OnigCalloutArgs, _user_data: *mut std::ffi::c_void) -> i32 {
    let Some(callout_data) = args.callout_data else {
        return ONIG_CALLOUT_FAIL;
    };
    let mut cd = callout_data.borrow_mut();
    let num = args.num;
    if num < 1 {
        return ONIG_CALLOUT_FAIL;
    }
    let idx = (num - 1) as usize;
    if idx >= cd.len() {
        return ONIG_CALLOUT_FAIL;
    }

    let reg = args.reg;
    let ext = match reg.extp.as_ref() {
        Some(e) => e,
        None => return ONIG_CALLOUT_FAIL,
    };
    if idx >= ext.callout_list.len() || ext.callout_list[idx].args.len() < 3 {
        return ONIG_CALLOUT_FAIL;
    }
    let entry = &ext.callout_list[idx];

    let lv = resolve_callout_arg(&entry.args[0], &ext.callout_list, &cd);
    let rv = resolve_callout_arg(&entry.args[2], &ext.callout_list, &cd);

    // The op is stored in slot 0 after first parse; or read from args[1]
    let op = cd[idx][0];
    if op == 0 {
        // First call: parse the op string from args[1]
        let op_val = match &entry.args[1] {
            CalloutArg::Str(s) => {
                if s == b"==" {
                    1
                } else if s == b"!=" {
                    2
                } else if s == b"<" {
                    3
                } else if s == b">" {
                    4
                } else if s == b"<=" {
                    5
                } else if s == b">=" {
                    6
                } else {
                    return ONIGERR_INVALID_CALLOUT_ARG;
                }
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
        if result {
            ONIG_CALLOUT_SUCCESS
        } else {
            ONIG_CALLOUT_FAIL
        }
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
        if result {
            ONIG_CALLOUT_SUCCESS
        } else {
            ONIG_CALLOUT_FAIL
        }
    }
}

// ============================================================================
// Stack Types (port of StackType / STK_* constants)
// ============================================================================

/// Memory pointer - tracks capture group start/end positions.
/// Compact representation of C's StkPtrType union { StackIndex i; UChar* s; }
///
/// Encodes three states in a single `usize` to minimize StackEntry size:
/// - `usize::MAX` = Invalid (not yet matched)
/// - Bit 63 set = stack index (lower 63 bits)
/// - Bit 63 clear = string position
#[derive(Clone, Copy, Debug)]
pub(crate) struct MemPtr(usize);

const MEM_PTR_INVALID: usize = usize::MAX;
const MEM_PTR_STACK_BIT: usize = 1 << (usize::BITS - 1);

impl MemPtr {
    #[inline(always)]
    pub(crate) const fn invalid() -> Self {
        MemPtr(MEM_PTR_INVALID)
    }

    #[inline(always)]
    pub(crate) const fn pos(p: usize) -> Self {
        MemPtr(p)
    }

    #[inline(always)]
    pub(crate) const fn stack_idx(i: usize) -> Self {
        MemPtr(i | MEM_PTR_STACK_BIT)
    }

    #[inline(always)]
    const fn is_invalid(self) -> bool {
        self.0 == MEM_PTR_INVALID
    }

    #[inline(always)]
    const fn is_stack_idx(self) -> bool {
        !self.is_invalid() && (self.0 & MEM_PTR_STACK_BIT) != 0
    }

    #[inline(always)]
    const fn as_pos(self) -> Option<usize> {
        if self.is_invalid() || (self.0 & MEM_PTR_STACK_BIT) != 0 {
            None
        } else {
            Some(self.0)
        }
    }

    #[inline(always)]
    const fn as_stack_idx(self) -> Option<usize> {
        if self.is_stack_idx() {
            Some(self.0 & !MEM_PTR_STACK_BIT)
        } else {
            None
        }
    }
}

/// Stack entry - corresponds to C's StackType struct.
/// Uses enum to distinguish entry types instead of C's type field + union.
#[derive(Clone)]
pub(crate) enum StackEntry {
    /// Choice point (STK_ALT / STK_SUPER_ALT) - alternate path for backtracking.
    Alt {
        pcode: usize,   // bytecode index to jump to on backtrack
        pstr: usize,    // string position to restore
        zid: i32,       // remaining count for StepBackNext (-1 = unused)
        is_super: bool, // true for SUPER_ALT (survives CutToMark)
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
    MemEndMark { zid: usize },
    /// Repeat counter (STK_REPEAT_INC)
    RepeatInc { zid: usize, count: i32 },
    /// Empty check start marker (STK_EMPTY_CHECK_START)
    EmptyCheckStart { zid: usize, pstr: usize },
    /// Empty check end marker (STK_EMPTY_CHECK_END)
    EmptyCheckEnd { zid: usize },
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
    CallFrame { ret_addr: usize },
    /// Return marker (STK_RETURN)
    Return,
    /// Callout entry (STK_CALLOUT) - for retraction callbacks
    Callout {
        num: i32, // callout list index (1-based)
        builtin_id: i32,
        name_id: i32,
        callback: Option<OnigCalloutFunc>,
    },
    /// Lazy choice point for star opcodes - represents a range of backtrack positions.
    /// Instead of pushing one Alt per matched character, we push a single AltLazy
    /// covering the entire matched range. On backtrack, we peel off one character
    /// and re-push for the remaining range.
    AltLazy {
        pcode: usize,      // bytecode index to jump to on backtrack
        pstr: usize,       // current (rightmost) backtrack position
        pstr_start: usize, // leftmost position (don't go below this)
        ascii: bool,       // true = all chars are 1 byte (can decrement by 1)
        peek_byte: u8,     // if non-zero, only try positions where str_data[pos] == peek_byte
    },
    /// Voided entry (STK_VOID) - dead space, skipped during pops
    Void,
}

impl StackEntry {
    /// Returns true if this entry is an ALT (choice point) that stops STACK_POP.
    #[inline]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn is_alt(&self) -> bool {
        matches!(self, StackEntry::Alt { .. } | StackEntry::AltLazy { .. })
    }

    /// Returns true if this entry needs handling during pop at ALL level.
    #[inline]
    #[cfg_attr(coverage_nightly, coverage(off))]
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

/// Rust-only (ADR-008): the limits and callout settings one search applies.
/// A search takes them from `OnigMatchParam` (`onig_search_with_param`) or,
/// where a caller passes `None`, from the process-wide settings read when the
/// search starts. Keeping them out of `OnigMatchParam` lets the per-search API
/// reuse the thread-cached `MatchArg` without building the parameter block.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SearchLimits {
    pub(crate) match_stack_limit: u32,
    pub(crate) retry_limit_in_match: u64,
    pub(crate) retry_limit_in_search: u64,
    pub(crate) time_limit: u64,
    pub(crate) progress_callout: Option<OnigCalloutFunc>,
    pub(crate) retraction_callout: Option<OnigCalloutFunc>,
    // Store the opaque C user-data pointer as an integer so cached MatchArg
    // remains Send + Sync. It is cast back only for the callback invocation.
    pub(crate) callout_user_data: usize,
}

impl SearchLimits {
    /// The process-wide limits and callout settings, as
    /// `onig_initialize_match_param` reads them.
    #[inline]
    pub(crate) fn process_wide() -> Self {
        SearchLimits {
            match_stack_limit: MATCH_STACK_LIMIT.load(Ordering::Relaxed),
            retry_limit_in_match: RETRY_LIMIT_IN_MATCH.load(Ordering::Relaxed),
            retry_limit_in_search: RETRY_LIMIT_IN_SEARCH.load(Ordering::Relaxed),
            time_limit: TIME_LIMIT.load(Ordering::Relaxed),
            progress_callout: onig_get_progress_callout(),
            retraction_callout: onig_get_retraction_callout(),
            callout_user_data: 0,
        }
    }
}

impl From<&OnigMatchParam> for SearchLimits {
    #[inline]
    fn from(mp: &OnigMatchParam) -> Self {
        SearchLimits {
            match_stack_limit: mp.match_stack_limit,
            retry_limit_in_match: mp.retry_limit_in_match,
            retry_limit_in_search: mp.retry_limit_in_search,
            time_limit: mp.time_limit,
            progress_callout: mp.progress_callout,
            retraction_callout: mp.retraction_callout,
            callout_user_data: mp.callout_user_data as usize,
        }
    }
}

/// The position `MatchArg::capture_position` holds before the first attempt of a
/// two-pass search: that attempt records captures.
const FIRST_ATTEMPT: usize = usize::MAX;

pub(crate) struct MatchArg {
    pub options: OnigOptionType,
    pub region: Option<OnigRegion>,
    pub start: usize, // search start position (for \G anchor)
    pub best_len: i32,
    pub best_s: usize,
    /// Best match length reported to CALLBACK_EACH_MATCH during this search.
    /// C only invokes the callback for FIND_LONGEST when a search-level best
    /// improves, even when one attempt finds several progressively longer
    /// alternatives.
    callout_each_match_best_len: i32,
    /// Skip target position set by `onig_builtin_skip`. The search loop
    /// advances past this position after a mismatch.
    pub skip_search: usize,
    // Safety limits
    pub retry_limit_in_match: u64,
    pub retry_limit_in_search: u64,
    pub retry_limit_in_search_counter: u64,
    pub match_stack_limit: u32,
    pub time_limit: u64, // milliseconds, 0 = unlimited
    /// Progress and retraction callbacks copied from the match parameters.
    pub progress_callout: Option<OnigCalloutFunc>,
    pub retraction_callout: Option<OnigCalloutFunc>,
    pub callout_user_data: usize,
    /// OP_CALLs made by this search (C: `msa->subexp_call_in_search_counter`),
    /// counted only while `onig_set_subexp_call_limit_in_search` is non-zero.
    subexp_call_in_search_counter: u64,
    /// Backtracks since the last clock read (C: `msa->time_counter`). Shared by
    /// every start position of one search, like C's per-search counter.
    time_counter: u64,
    /// Deadline fixed when the search starts (C: `TIME_LIMIT_INIT`). `None`
    /// without a limit, or when the limit is too large to represent.
    time_end: Option<Instant>,
    /// Rust-only: the length of the last match without FIND_LONGEST. With `\K` the
    /// match may start after the attempt position, but it ends there plus
    /// this length (`onig_search_bounds`).
    pub(crate) match_len: i32,
    /// Rust-only: while set, a search with a region records captures only in
    /// one attempt: the attempt at this position, or, while it is
    /// `FIRST_ATTEMPT`, the first attempt the search makes, which then sets
    /// its own position. Other attempts run untracked and leave the region
    /// alone; a match at a position that did not record its captures runs
    /// again to record them (`onig_search_inner_two_pass`).
    pub(crate) capture_position: Option<usize>,
    // Reusable VM state (avoids heap allocation per match_at call)
    stack: Vec<StackEntry>,
    mem_start_stk: Vec<MemPtr>,
    mem_end_stk: Vec<MemPtr>,
}

const CHECK_TIME_INTERVAL: u64 = 512;

impl MatchArg {
    pub(crate) fn new(
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
    ) -> Self {
        Self::with_limits(reg, option, region, start, None)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_param(
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
        mp: &OnigMatchParam,
    ) -> Self {
        Self::with_limits(reg, option, region, start, Some(SearchLimits::from(mp)))
    }

    /// A new MatchArg for a search under `limits`; `None` reads the
    /// process-wide limits.
    fn with_limits(
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
        limits: Option<SearchLimits>,
    ) -> Self {
        let limits = limits.unwrap_or_else(SearchLimits::process_wide);
        let mut msa = MatchArg {
            options: option | reg.options,
            region,
            start,
            best_len: ONIG_MISMATCH,
            best_s: 0,
            callout_each_match_best_len: ONIG_MISMATCH,
            skip_search: 0,
            retry_limit_in_match: limits.retry_limit_in_match,
            retry_limit_in_search: limits.retry_limit_in_search,
            retry_limit_in_search_counter: 0,
            match_stack_limit: limits.match_stack_limit,
            time_limit: limits.time_limit,
            progress_callout: limits.progress_callout,
            retraction_callout: limits.retraction_callout,
            callout_user_data: limits.callout_user_data,
            time_counter: 0,
            time_end: None,
            subexp_call_in_search_counter: 0,
            match_len: 0,
            capture_position: None,
            stack: Vec::with_capacity(INIT_MATCH_STACK_SIZE),
            mem_start_stk: Vec::new(),
            mem_end_stk: Vec::new(),
        };
        msa.start_time_limit();
        msa
    }

    /// Full reset for thread-local reuse under `limits` (`None`: the
    /// process-wide limits, re-read here). Keeps allocated buffers.
    pub(crate) fn reset_with_limits(
        &mut self,
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
        limits: Option<SearchLimits>,
    ) {
        self.region = region;
        self.reset_search_state(reg, option, start, limits);
    }

    /// The reset of one search of an iterator that holds its `MatchArg` for
    /// all its searches (Rust-only, ADR-008). It is `reset_with_limits` without
    /// the region: the caller sets the region of the search, and takes it back
    /// after. Limits are read per search, as in every other search.
    #[inline]
    pub(crate) fn reset_for_iteration(
        &mut self,
        reg: &RegexType,
        start: usize,
        limits: Option<SearchLimits>,
    ) {
        self.reset_search_state(reg, ONIG_OPTION_NONE, start, limits);
    }

    /// The state of `reset_with_limits` other than the region.
    #[inline]
    fn reset_search_state(
        &mut self,
        reg: &RegexType,
        option: OnigOptionType,
        start: usize,
        limits: Option<SearchLimits>,
    ) {
        let limits = limits.unwrap_or_else(SearchLimits::process_wide);
        self.options = option | reg.options;
        self.start = start;
        self.best_len = ONIG_MISMATCH;
        self.best_s = 0;
        self.callout_each_match_best_len = ONIG_MISMATCH;
        self.skip_search = 0;
        self.retry_limit_in_match = limits.retry_limit_in_match;
        self.retry_limit_in_search = limits.retry_limit_in_search;
        self.retry_limit_in_search_counter = 0;
        self.subexp_call_in_search_counter = 0;
        self.match_stack_limit = limits.match_stack_limit;
        self.time_limit = limits.time_limit;
        self.progress_callout = limits.progress_callout;
        self.retraction_callout = limits.retraction_callout;
        self.callout_user_data = limits.callout_user_data;
        self.start_time_limit();
    }

    /// Reset mutable state for a new search, keeping allocated buffers.
    pub(crate) fn reset_for_search(
        &mut self,
        reg: &RegexType,
        option: OnigOptionType,
        region: Option<OnigRegion>,
        start: usize,
    ) {
        self.options = option | reg.options;
        self.region = region;
        self.start = start;
        self.best_len = ONIG_MISMATCH;
        self.best_s = 0;
        self.callout_each_match_best_len = ONIG_MISMATCH;
        self.skip_search = 0;
        let callbacks = SearchLimits::process_wide();
        self.progress_callout = callbacks.progress_callout;
        self.retraction_callout = callbacks.retraction_callout;
        self.callout_user_data = callbacks.callout_user_data;
        self.retry_limit_in_search_counter = 0;
        self.subexp_call_in_search_counter = 0;
        self.start_time_limit();
    }

    /// Light reset for reusing MatchArg across multiple onig_match calls
    /// within a regset loop. Keeps allocated buffers and region ownership.
    pub(crate) fn reset_for_match(
        &mut self,
        reg: &RegexType,
        option: OnigOptionType,
        start: usize,
    ) {
        self.options = option | reg.options;
        self.start = start;
        self.best_len = ONIG_MISMATCH;
        self.best_s = 0;
        self.callout_each_match_best_len = ONIG_MISMATCH;
    }

    /// Start the time limit for a new search (C: `TIME_LIMIT_INIT`).
    #[inline]
    fn start_time_limit(&mut self) {
        self.time_counter = 0;
        self.time_end = if self.time_limit == 0 {
            None
        } else {
            Instant::now().checked_add(std::time::Duration::from_millis(self.time_limit))
        };
    }

    /// Count one backtrack and read the clock every `CHECK_TIME_INTERVAL`
    /// backtracks (C: `CHECK_TIME_LIMIT_IN_MATCH`). Returns true once the
    /// deadline has passed.
    #[inline]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn check_time_limit(&mut self, backtracks: u64) -> bool {
        self.time_counter += backtracks;
        if self.time_counter < CHECK_TIME_INTERVAL {
            return false;
        }
        self.time_counter = 0;
        self.time_end.is_some_and(|end| Instant::now() > end)
    }
}

// ============================================================================
// Stack limit check
// ============================================================================

/// The number of entries the backtrack stack may hold under
/// `match_stack_limit` (C: `stack_double`), bottom sentinel included. A push
/// beyond it stops the match with `ONIGERR_MATCH_STACK_LIMIT_OVER`.
///
/// C checks the limit only when it reallocates a heap stack. Its first
/// stack of `INIT_MATCH_STACK_SIZE` entries lives on the C stack and doubles
/// to the heap without a check, and a limit below that size grants the push
/// that finds it full before the limit applies. Under any limit below
/// `2 * INIT_MATCH_STACK_SIZE` C therefore completes every search that holds
/// at most `2 * INIT_MATCH_STACK_SIZE + 1` entries, and Ferroni takes that
/// count as its cap (ADR-017, rule 3). C's allowance below it depends on its
/// allocation history, so a search that C stops there may complete in
/// Ferroni with the result C returns without the limit (rule 2).
#[inline]
pub(crate) fn match_stack_cap(match_stack_limit: u32) -> usize {
    match match_stack_limit {
        0 => HARD_MATCH_STACK_CAP,
        limit if (limit as usize) < MATCH_STACK_LIMIT_FLOOR => MATCH_STACK_LIMIT_FLOOR + 1,
        limit => limit as usize,
    }
}

/// Count `backtracks` backtracks against the retry and time limits (C's
/// `CHECK_RETRY_LIMIT_IN_MATCH` and `CHECK_TIME_LIMIT_IN_MATCH`). Returns
/// Err with the error code once a limit is reached: like C
/// (`++counter >= limit`), the backtrack that brings the count to the limit
/// already stops the match.
#[inline]
fn count_retry(
    backtracks: u64,
    retry_in_match_counter: &mut u64,
    retry_limit_in_match: u64,
    time_limit_ms: u64,
    msa: &mut MatchArg,
) -> Result<(), i32> {
    *retry_in_match_counter += backtracks;
    if retry_limit_in_match != 0 && *retry_in_match_counter >= retry_limit_in_match {
        return Err(
            if msa.retry_limit_in_match != 0 && *retry_in_match_counter >= msa.retry_limit_in_match
            {
                ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
            } else {
                ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER
            },
        );
    }
    // Time limit check (every CHECK_TIME_INTERVAL retries of the search)
    if time_limit_ms > 0 && msa.check_time_limit(backtracks) {
        return Err(ONIGERR_TIME_LIMIT_OVER);
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
#[allow(clippy::too_many_arguments)]
fn stack_pop(
    stack: &mut Vec<StackEntry>,
    pop_level: StackPopLevel,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
    reg: &RegexType,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
    str_data: &[u8],
    end: usize,
    start: usize,
    right_range: usize,
    current_s: usize,
    retry_counter: u64,
    msa: &mut MatchArg,
    subexp_call_nest_counter: &mut u64,
) -> Result<Option<(usize, usize, i32)>, i32> {
    loop {
        let Some(entry) = stack.pop() else {
            return Ok(None);
        };
        match entry {
            StackEntry::Alt {
                pcode, pstr, zid, ..
            } => {
                return Ok(Some((pcode, pstr, zid)));
            }
            StackEntry::AltLazy {
                pcode,
                pstr,
                pstr_start,
                ascii,
                peek_byte,
            } => {
                // Peel off one character and re-push for the remaining range.
                // If peek_byte is set, use memrchr to skip positions where the
                // next byte can't match, instead of decrementing by 1.
                if pstr > pstr_start {
                    let prev = if peek_byte != 0 {
                        // Search backward for the peek byte
                        let search_end = pstr - pstr_start;
                        memchr::memrchr(peek_byte, &str_data[pstr_start..pstr])
                    } else if ascii {
                        Some(pstr - 1)
                    } else {
                        Some(prev_char_head(reg.enc, pstr_start, pstr, str_data))
                    };
                    if let Some(prev_pos) = prev {
                        let prev_abs = if peek_byte != 0 {
                            pstr_start + prev_pos
                        } else {
                            prev_pos
                        };
                        if prev_abs >= pstr_start {
                            stack.push(StackEntry::AltLazy {
                                pcode,
                                pstr: prev_abs,
                                pstr_start,
                                ascii,
                                peek_byte,
                            });
                        }
                    }
                }
                return Ok(Some((pcode, pstr, -1)));
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
                        StackEntry::Callout {
                            num,
                            builtin_id,
                            name_id,
                            callback,
                        } => {
                            // Builtin state and user callbacks share the C
                            // STK_CALLOUT entry, but keep their dispatch data
                            // separate in the Rust stack value.
                            if *builtin_id >= 0 {
                                run_builtin_callout_retraction(
                                    reg,
                                    *num,
                                    *builtin_id,
                                    callout_data,
                                );
                            }
                            if let Some(callback) = callback {
                                let result = invoke_callout(
                                    *callback,
                                    OnigCalloutIn::Retraction,
                                    *name_id,
                                    *num,
                                    reg,
                                    str_data,
                                    end,
                                    start,
                                    right_range,
                                    current_s,
                                    retry_counter,
                                    msa,
                                    callout_data,
                                );
                                match result {
                                    ONIG_CALLOUT_SUCCESS | ONIG_CALLOUT_FAIL => {}
                                    result if result > 0 => {
                                        return Err(ONIGERR_INVALID_ARGUMENT);
                                    }
                                    result => return Err(result),
                                }
                            }
                        }
                        // C: POP_CALL
                        StackEntry::Return => {
                            *subexp_call_nest_counter = subexp_call_nest_counter.wrapping_add(1);
                        }
                        StackEntry::CallFrame { .. } => {
                            *subexp_call_nest_counter = subexp_call_nest_counter.wrapping_sub(1);
                        }
                        // RepeatInc, EmptyCheckStart: handled implicitly
                        // (popping removes them)
                        _ => {}
                    }
                }
            },
        }
    }
}

/// Pop stack entries until a Mark with matching zid is found (STACK_POP_TO_MARK).
/// Removes ALL entries. Restores mem_start_stk/mem_end_stk from every popped
/// MemStart/MemEnd entry, so captures set inside a failed (?!..) or (?<!..)
/// body are rolled back, and popped CallFrame/Return entries adjust the call
/// nest counter (C: POP_CALL). Like C, callouts are not retracted here.
/// Returns the saved position from the Mark entry (if any).
fn stack_pop_to_mark(
    stack: &mut Vec<StackEntry>,
    mark_id: usize,
    mem_start_stk: &mut [MemPtr],
    mem_end_stk: &mut [MemPtr],
    subexp_call_nest_counter: &mut u64,
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
            // C: POP_CALL
            StackEntry::Return => {
                *subexp_call_nest_counter = subexp_call_nest_counter.wrapping_add(1);
            }
            StackEntry::CallFrame { .. } => {
                *subexp_call_nest_counter = subexp_call_nest_counter.wrapping_sub(1);
            }
            _ => {}
        }
    }
}

/// Void stack entries until a Mark with matching zid is found (STACK_TO_VOID_TO_MARK).
/// Only voids "void targets" (regular Alt, EmptyCheckStart, EmptyCheckEnd, Mark) by setting
/// them to Void. Preserves non-void targets (SuperAlt, SaveVal, MemStart, MemEnd, RepeatInc,
/// etc.) in place.
/// Returns the mark's saved position. Preserves Super Alt entries and Marks with different IDs.
fn stack_void_to_mark(stack: &mut [StackEntry], mark_id: usize) -> Option<usize> {
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
        // Void targets (C: STK_MASK_TO_VOID_TARGET): regular Alt,
        // EmptyCheckStart and EmptyCheckEnd.
        // Super Alt (is_super=true) is NOT voided — it survives cuts
        let is_void_target = matches!(
            &stack[i],
            StackEntry::Alt {
                is_super: false,
                ..
            } | StackEntry::AltLazy { .. }
                | StackEntry::EmptyCheckStart { .. }
                | StackEntry::EmptyCheckEnd { .. }
        );
        if is_void_target {
            stack[i] = StackEntry::Void;
        }
    }
    None
}

/// Search backwards through the stack for the most recent RepeatInc with matching zid.
/// Returns the count from that entry.
///
/// C: STACK_GET_REPEAT_COUNT_SEARCH. The entries of a subexpression call
/// that has already returned (from its STK_CALL_FRAME up to the matching
/// STK_RETURN) belong to another run of the same repeat, so the scan skips
/// them.
fn stack_get_repeat_count(stack: &[StackEntry], zid: usize) -> i32 {
    let mut k = stack.len();
    while k > 0 {
        k -= 1;
        match &stack[k] {
            StackEntry::RepeatInc { zid: id, count } if *id == zid => return *count,
            StackEntry::Return => {
                let mut level: i32 = -1;
                while k > 0 {
                    k -= 1;
                    match &stack[k] {
                        StackEntry::CallFrame { .. } => {
                            level += 1;
                            if level == 0 {
                                break;
                            }
                        }
                        StackEntry::Return => level -= 1,
                        _ => {}
                    }
                }
            }
            _ => {}
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

/// C: `STACK_AT(ptr.i)->u.mem.pstr` -- the string position of the capture
/// start or end that a saved `mem_start_stk`/`mem_end_stk` value refers to.
/// C only ever reads stack indices here; the port also accepts a plain
/// position, which is what a non-push capture op stores.
fn mem_ptr_pstr(stack: &[StackEntry], ptr: MemPtr) -> Option<usize> {
    if let Some(pos) = ptr.as_pos() {
        return Some(pos);
    }
    match stack.get(ptr.as_stack_idx()?)? {
        StackEntry::MemStart { pstr, .. } | StackEntry::MemEnd { pstr, .. } => Some(*pstr),
        _ => None,
    }
}

/// The per-capture test of C's STACK_EMPTY_CHECK_MEM and
/// STACK_EMPTY_CHECK_MEM_REC (USE_RIGID_CHECK_CAPTURES_IN_EMPTY_REPEAT).
///
/// `kk_*` are the saved start/end of the MEM_START pushed inside the
/// iteration (the capture before the iteration), `k_*` the saved start and
/// the position of the MEM_END that closed it (the capture after it). The
/// iteration changed the capture unless both are the same range, or both are
/// empty ranges.
fn empty_check_mem_changed(
    stack: &[StackEntry],
    kk_prev_start: MemPtr,
    kk_prev_end: MemPtr,
    k_prev_start: MemPtr,
    k_pstr: usize,
) -> bool {
    if kk_prev_end.is_invalid() {
        return true;
    }
    let (Some(old_end), Some(old_start), Some(new_start)) = (
        mem_ptr_pstr(stack, kk_prev_end),
        mem_ptr_pstr(stack, kk_prev_start),
        mem_ptr_pstr(stack, k_prev_start),
    ) else {
        // Unreachable when the saved indices are valid, as in C. Treat the
        // capture as unchanged so the loop still terminates.
        return false;
    };
    (old_end != k_pstr || old_start != new_start) && (new_start != k_pstr || old_start != old_end)
}

/// Memory-aware empty check (C: STACK_EMPTY_CHECK_MEM).
///
/// Returns true only if the string position has not advanced since the
/// matching EMPTY_CHECK_START *and* no capture in `empty_status_mem` changed
/// during the iteration.
fn stack_empty_check_mem(
    stack: &[StackEntry],
    zid: usize,
    s: usize,
    empty_status_mem: u32,
) -> bool {
    // GET_EMPTY_CHECK_START
    let Some(klow) = stack
        .iter()
        .rposition(|e| matches!(e, StackEntry::EmptyCheckStart { zid: id, .. } if *id == zid))
    else {
        return false;
    };
    if let StackEntry::EmptyCheckStart { pstr, .. } = &stack[klow] {
        if *pstr != s {
            return false;
        }
    }

    let mut ms = empty_status_mem;
    let mut k = stack.len();
    while k > klow {
        k -= 1;
        if let StackEntry::MemEnd {
            zid: mem,
            pstr: k_pstr,
            prev_start: k_prev_start,
            ..
        } = &stack[k]
        {
            if mem_status_limit_at(ms, *mem) {
                for kk in klow..k {
                    if let StackEntry::MemStart {
                        zid: kk_mem,
                        prev_start,
                        prev_end,
                        ..
                    } = &stack[kk]
                    {
                        if kk_mem == mem {
                            if empty_check_mem_changed(
                                stack,
                                *prev_start,
                                *prev_end,
                                *k_prev_start,
                                *k_pstr,
                            ) {
                                return false;
                            }
                            ms &= !(1u32 << *mem);
                            break;
                        }
                    }
                }
                if ms == 0 {
                    break;
                }
            }
        }
    }
    true
}

/// Memory-aware empty check for a loop whose body may recurse
/// (C: STACK_EMPTY_CHECK_MEM_REC).
///
/// A recursive call can run the same loop again, so the stack may hold
/// EMPTY_CHECK_START entries of nested runs. Every non-empty iteration of
/// such a loop pushes an EMPTY_CHECK_END (see OP_EMPTY_CHECK_END_MEMST_PUSH),
/// and `level` pairs them up so the scan skips completed iterations.
fn stack_empty_check_mem_rec(
    stack: &[StackEntry],
    zid: usize,
    s: usize,
    empty_status_mem: u32,
) -> bool {
    let mut level: i32 = 0;
    let mut klow = stack.len();
    loop {
        // STACK_BASE_CHECK
        if klow == 0 {
            return false;
        }
        klow -= 1;
        match &stack[klow] {
            StackEntry::EmptyCheckStart { zid: id, pstr } if *id == zid => {
                if level != 0 {
                    level -= 1;
                    continue;
                }
                if *pstr != s {
                    return false;
                }
                if empty_status_mem == 0 {
                    return true;
                }
                let mut ms = empty_status_mem;
                let mut k = stack.len();
                while k > klow {
                    k -= 1;
                    match &stack[k] {
                        StackEntry::MemEnd {
                            zid: mem,
                            pstr: k_pstr,
                            prev_start: k_prev_start,
                            ..
                        } => {
                            if level == 0 && mem_status_limit_at(ms, *mem) {
                                for kk in klow + 1..k {
                                    match &stack[kk] {
                                        StackEntry::MemStart {
                                            zid: kk_mem,
                                            prev_start,
                                            prev_end,
                                            ..
                                        } if kk_mem == mem => {
                                            if empty_check_mem_changed(
                                                stack,
                                                *prev_start,
                                                *prev_end,
                                                *k_prev_start,
                                                *k_pstr,
                                            ) {
                                                return false;
                                            }
                                            ms &= !(1u32 << *mem);
                                            break;
                                        }
                                        StackEntry::EmptyCheckStart { zid: id, .. }
                                            if *id == zid =>
                                        {
                                            level += 1;
                                        }
                                        StackEntry::EmptyCheckEnd { zid: id } if *id == zid => {
                                            level -= 1;
                                        }
                                        _ => {}
                                    }
                                }
                                level = 0;
                                if ms == 0 {
                                    break;
                                }
                            }
                        }
                        StackEntry::EmptyCheckStart { zid: id, .. } if *id == zid => {
                            level += 1;
                        }
                        StackEntry::EmptyCheckEnd { zid: id } if *id == zid => {
                            level -= 1;
                        }
                        _ => {}
                    }
                }
                return true;
            }
            StackEntry::EmptyCheckEnd { zid: id } if *id == zid => {
                level += 1;
            }
            _ => {}
        }
    }
}

/// Get the saved value for a given save_type and zid from the stack.
/// Search stack for last SaveVal by type only (C: STACK_GET_SAVE_VAL_TYPE_LAST)
fn stack_get_save_val_type_last(stack: &[StackEntry], save_type: SaveType) -> Option<usize> {
    for entry in stack.iter().rev() {
        if let StackEntry::SaveVal {
            save_type: st, v, ..
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
fn stack_get_save_val_last(stack: &[StackEntry], save_type: SaveType, zid: usize) -> Option<usize> {
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
                if n <= ONIG_MAX_CAPTURE_HISTORY_GROUP && mem_status_at(reg.capture_history, n) {
                    let mut child = Box::new(OnigCaptureTreeNode::new());
                    child.group = n as i32;
                    child.beg = *pstr as i32;
                    node.add_child(child);
                    let child_idx = node.childs.len() - 1;
                    *k += 1;
                    let r = make_capture_history_tree(
                        &mut node.childs[child_idx],
                        k,
                        stack,
                        stk_top,
                        reg,
                    );
                    if r < 0 {
                        return r;
                    }
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
    let v = mem_start_stk[idx];
    if v.is_invalid() {
        None
    } else if let Some(pos) = v.as_pos() {
        Some(pos)
    } else if let Some(si) = v.as_stack_idx() {
        if si >= stack.len() {
            None
        } else if let StackEntry::MemStart { pstr, .. } = &stack[si] {
            Some(*pstr)
        } else {
            None
        }
    } else {
        None
    }
}

/// Get the string position where a capture group ends.
fn get_mem_end(
    reg: &RegexType,
    stack: &[StackEntry],
    mem_end_stk: &[MemPtr],
    idx: usize,
) -> Option<usize> {
    let v = mem_end_stk[idx];
    if v.is_invalid() {
        None
    } else if let Some(pos) = v.as_pos() {
        Some(pos)
    } else if let Some(si) = v.as_stack_idx() {
        if si >= stack.len() {
            None
        } else if let StackEntry::MemEnd { pstr, .. } = &stack[si] {
            Some(*pstr)
        } else {
            None
        }
    } else {
        None
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn populate_region_for_match(
    region: &mut OnigRegion,
    reg: &RegexType,
    num_mem: usize,
    keep: usize,
    s: usize,
    stack: &[StackEntry],
    mem_start_stk: &[MemPtr],
    mem_end_stk: &[MemPtr],
) -> Result<(), i32> {
    let region_regs = num_mem + 1;
    if region.num_regs != region_regs as i32 {
        region.resize(region_regs as i32);
    }
    region.beg[0] = keep as i32;
    region.end[0] = s as i32;

    if reg.push_mem_start == 0 && reg.push_mem_end == 0 {
        for i in 1..=num_mem {
            let mem_start = mem_start_stk[i].0;
            let mem_end = mem_end_stk[i].0;
            if mem_start != MEM_PTR_INVALID && mem_end != MEM_PTR_INVALID {
                region.beg[i] = mem_start as i32;
                region.end[i] = mem_end as i32;
            } else {
                region.beg[i] = ONIG_REGION_NOTPOS;
                region.end[i] = ONIG_REGION_NOTPOS;
            }
        }
    } else {
        for i in 1..=num_mem {
            if let Some(mem_end) = get_mem_end(reg, stack, mem_end_stk, i) {
                let mem_start = get_mem_start(reg, stack, mem_start_stk, i);
                region.beg[i] = mem_start.map(|v| v as i32).unwrap_or(ONIG_REGION_NOTPOS);
                region.end[i] = mem_end as i32;
            } else {
                region.beg[i] = ONIG_REGION_NOTPOS;
                region.end[i] = ONIG_REGION_NOTPOS;
            }
        }
    }

    if USE_CAPTURE_HISTORY && reg.capture_history != 0 {
        let node = region
            .history_root
            .get_or_insert_with(|| Box::new(OnigCaptureTreeNode::new()));
        node.clear();
        node.group = 0;
        node.beg = keep as i32;
        node.end = s as i32;
        let mut stkp = 0usize;
        let stk_top = stack.len();
        let r = make_capture_history_tree(node, &mut stkp, stack, stk_top, reg);
        if r < 0 {
            return Err(r);
        }
    }

    Ok(())
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
                        return (MemPtr::stack_idx(k), *pstr);
                    } else {
                        return (MemPtr::pos(*pstr), *pstr);
                    }
                }
                level -= 1;
            }
            _ => {}
        }
    }
    (MemPtr::invalid(), 0)
}

/// Match a backref at a specific nesting level in the recursion stack.
/// Walks the stack backwards counting CallFrame/Return to find the right level,
/// then matches the captured text at that level against current position.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_arguments)]
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
                            if !string_cmp_ic(reg.enc, case_fold_flag, str_data, pstart, s, cap_len)
                            {
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
///
/// Companion to `backref_match_at_nested_level` (already excluded).
/// Only reachable via `\k<name+level>` check opcodes in recursive patterns.
#[cfg_attr(coverage_nightly, coverage(off))]
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
            StackEntry::CallFrame { .. } => {
                level -= 1;
            }
            StackEntry::Return => {
                level += 1;
            }
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn mem_is_in_mems(mem: usize, num: i32, mems: &[i32]) -> bool {
    for candidate in mems.iter().take(num as usize) {
        if mem == *candidate as usize {
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
const fn is_word_ascii(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `is_word_ascii` as a byte table, for `is_word_boundary_ascii_fast`.
const WORD_ASCII_TABLE: [bool; 256] = {
    let mut table = [false; 256];
    let mut c = 0;
    while c < 256 {
        table[c] = is_word_ascii(c as u8);
        c += 1;
    }
    table
};

/// Check if the character at position s is a word character (encoding-aware).
/// Uses the encoding's Unicode-aware is_code_ctype for multi-byte encodings.
#[inline]
fn is_word_char_at(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> bool {
    if s >= end {
        return false;
    }
    let b = str_data[s];
    if b < 0x80 {
        return is_word_ascii(b);
    }
    let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
    enc.is_code_ctype(code, ONIGENC_CTYPE_WORD)
}

/// Check if position is a word character in ASCII-aware mode.
/// mode == 0: full Unicode word check, mode != 0: ASCII-only word check.
#[inline]
fn is_word_char_ascii_mode(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    end: usize,
    mode: ModeType,
) -> bool {
    if mode == 0 {
        return is_word_char_at(enc, str_data, s, end);
    }
    // ASCII-only: only a-z, A-Z, 0-9, _ are word characters
    if s >= end {
        return false;
    }
    let b = str_data[s];
    b < 0x80 && is_word_ascii(b)
}

/// Fast literal compare at `s` for VM exact-string opcodes.
///
/// For short lengths, a manual byte loop avoids libc `memcmp` call overhead
/// on the hottest matching path.
#[inline(always)]
fn exact_eq_at(str_data: &[u8], s: usize, exact: &[u8], len: usize) -> bool {
    match len {
        0 => true,
        1 => str_data[s] == exact[0],
        2 => str_data[s] == exact[0] && str_data[s + 1] == exact[1],
        3 => str_data[s] == exact[0] && str_data[s + 1] == exact[1] && str_data[s + 2] == exact[2],
        4 => {
            str_data[s] == exact[0]
                && str_data[s + 1] == exact[1]
                && str_data[s + 2] == exact[2]
                && str_data[s + 3] == exact[3]
        }
        5 => {
            str_data[s] == exact[0]
                && str_data[s + 1] == exact[1]
                && str_data[s + 2] == exact[2]
                && str_data[s + 3] == exact[3]
                && str_data[s + 4] == exact[4]
        }
        6 => {
            str_data[s] == exact[0]
                && str_data[s + 1] == exact[1]
                && str_data[s + 2] == exact[2]
                && str_data[s + 3] == exact[3]
                && str_data[s + 4] == exact[4]
                && str_data[s + 5] == exact[5]
        }
        7 => {
            str_data[s] == exact[0]
                && str_data[s + 1] == exact[1]
                && str_data[s + 2] == exact[2]
                && str_data[s + 3] == exact[3]
                && str_data[s + 4] == exact[4]
                && str_data[s + 5] == exact[5]
                && str_data[s + 6] == exact[6]
        }
        8 => {
            str_data[s] == exact[0]
                && str_data[s + 1] == exact[1]
                && str_data[s + 2] == exact[2]
                && str_data[s + 3] == exact[3]
                && str_data[s + 4] == exact[4]
                && str_data[s + 5] == exact[5]
                && str_data[s + 6] == exact[6]
                && str_data[s + 7] == exact[7]
        }
        9..=16 => {
            let mut i = 0usize;
            while i < len {
                if str_data[s + i] != exact[i] {
                    return false;
                }
                i += 1;
            }
            true
        }
        _ => str_data[s..s + len] == exact[..len],
    }
}

/// Get the start of the previous character (left_adjust_char_head).
#[inline]
fn prev_char_head(enc: OnigEncoding, start: usize, s: usize, str_data: &[u8]) -> usize {
    if s <= start {
        return s;
    }
    // Fast path for ASCII in ASCII-compatible encodings (UTF-8, etc.).
    if str_data[s - 1] < 0x80 && onigenc_is_ascii_compatible_encoding(enc) {
        return s - 1;
    }
    enc.left_adjust_char_head(start, s - 1, str_data)
}

/// Check word boundary at position s (encoding-aware, mode-aware).
/// mode == 0: Unicode word, mode != 0: ASCII-only word.
fn is_word_boundary(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    end: usize,
    mode: ModeType,
) -> bool {
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

/// `is_word_boundary` for the `WordBoundary` / `NoWordBoundary` opcodes,
/// with a Rust-only fast path (ADR-008) for an interior position between two
/// ASCII bytes. Both supported encodings, ASCII and UTF-8 (ADR-003), are
/// ASCII-compatible: a byte below 0x80 is a complete character. There
/// `prev_char_head` steps back exactly one byte, and both modes (Unicode and
/// ASCII word) classify each byte with `is_word_ascii`, so the fast path needs
/// no decoding and none of the encoding's virtual calls. The start, the end
/// and any non-ASCII neighbor take the full check.
#[inline(always)]
fn is_word_boundary_ascii_fast(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    end: usize,
    mode: ModeType,
) -> bool {
    if s > 0 && s < end {
        let prev = str_data[s - 1];
        let curr = str_data[s];
        if (prev | curr) < 0x80 {
            return WORD_ASCII_TABLE[prev as usize] != WORD_ASCII_TABLE[curr as usize];
        }
    }
    is_word_boundary(enc, str_data, s, end, mode)
}

/// Check if position s is at the start of a word (encoding-aware, mode-aware).
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
/// Binary search with C's inclusive membership semantics for sorted intervals.
#[inline]
pub(crate) fn is_in_code_range(data: &[u32], code: OnigCodePoint) -> bool {
    if data.len() < 3 {
        return false;
    }
    let n = data[0] as usize;
    if n == 0 {
        return false;
    }
    let pair_len = n.saturating_mul(2);
    let needed = 1usize.saturating_add(pair_len);
    if data.len() < needed {
        return false;
    }
    let ranges = &data[1..needed];

    let first_low = ranges[0];
    if code < first_low {
        return false;
    }
    let last_high = ranges[pair_len - 1];
    if code > last_high {
        return false;
    }

    // Common fast path for classes like [^\x00-\x7F] encoded as a single
    // multi-byte range [0x80..MAX].
    if n == 1 {
        return true;
    }

    // Small range tables are faster with a straight scan than binary search.
    if n <= 4 {
        let mut i = 0usize;
        while i < n {
            let lo = ranges[i * 2];
            let hi = ranges[i * 2 + 1];
            if code < lo {
                return false;
            }
            if code <= hi {
                return true;
            }
            i += 1;
        }
        return false;
    }

    // Search complete intervals so a hit can return before reaching a leaf.
    // Slicing keeps midpoint and tail accesses bounded without unchecked reads.
    let (mut pairs, _) = ranges.as_chunks::<2>();
    while !pairs.is_empty() {
        let (left, right) = pairs.split_at(pairs.len() / 2);
        // The midpoint of a nonempty slice is strictly below its length.
        let (range, tail) = right.split_first().unwrap();
        if code > range[1] {
            pairs = tail;
        } else if code < range[0] {
            pairs = left;
        } else {
            return true;
        }
    }
    false
}

/// Rust-only (ADR-008): where `\X` from `s` ends. The group is upstream's
/// atomic `\O(?:\Y\O)*`, and this takes the same steps without the loop's
/// instructions and backtrack entries. `\O` takes the first character, which
/// fails where no whole character fits before `right_range`. Each further
/// character is taken only where `\Y` passes (no extended grapheme cluster
/// boundary at the position, judged against `end`) and `\O` fits before
/// `right_range`. The loop ends at the first boundary, at `right_range` or
/// where a character does not fit, and the atomic group keeps that position.
#[inline]
fn grapheme_cluster_end(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    end: usize,
) -> Option<usize> {
    if right_range.saturating_sub(s) < 1 {
        return None;
    }
    let n = enclen(enc, str_data, s);
    if right_range.saturating_sub(s) < n {
        return None;
    }
    let mut s = s + n;
    loop {
        // `\O` cannot take a character at `right_range`, and `\Y` has no
        // effect there, so the loop ends without asking for the boundary.
        if s >= right_range
            || crate::unicode::onigenc_egcb_is_break_position(enc, str_data, s, 0, end)
        {
            return Some(s);
        }
        let n = enclen(enc, str_data, s);
        if right_range - s < n {
            return Some(s);
        }
        s += n;
    }
}

/// Get the character length at position s for the given encoding.
#[inline]
fn enclen(enc: OnigEncoding, str_data: &[u8], s: usize) -> usize {
    if s >= str_data.len() {
        return 1;
    }
    // Hot path: ASCII byte in ASCII-compatible encoding is always length 1.
    if str_data[s] < 0x80 && onigenc_is_ascii_compatible_encoding(enc) {
        return 1;
    }
    enc.mbc_enc_len(&str_data[s..])
}

/// Advance over one character without moving past the logical end of input.
///
/// A UTF-8 lead byte can report a character length larger than the remaining
/// input when callers use the byte API with truncated input. The matcher must
/// consume the available bytes in that case, rather than retaining an offset
/// beyond `end` for a later opcode or match region.
#[inline]
fn advance_char_to_end(enc: OnigEncoding, str_data: &[u8], s: usize, end: usize) -> usize {
    s.saturating_add(enclen(enc, str_data, s)).min(end)
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
    let end1 = s1_pos + mblen;
    let end2 = *s2_pos + mblen;
    let mut p1 = s1_pos;
    let mut p2 = *s2_pos;

    // These built-in encodings fold ASCII bytes through the same lowercase
    // table. Preserve C's byte limits and resume its fold loop at the first
    // non-ASCII pair; a failed comparison must not advance the caller's cursor.
    if ascii_case_insensitive_backrefs_enabled()
        && (std::ptr::addr_eq(enc, &crate::encodings::ONIG_ENCODING_UTF8 as OnigEncoding)
            || std::ptr::addr_eq(enc, &crate::encodings::ONIG_ENCODING_ASCII as OnigEncoding))
    {
        while p1 < end1 {
            let c1 = data[p1];
            let c2 = data[p2];
            if c1 >= 0x80 || c2 >= 0x80 {
                break;
            }
            if onigenc_ascii_code_to_lower_case(c1) != onigenc_ascii_code_to_lower_case(c2) {
                return false;
            }
            p1 += 1;
            p2 += 1;
        }
        if p1 == end1 {
            *s2_pos = p2;
            return true;
        }
    }

    let mut buf1 = [0u8; ONIGENC_MBC_CASE_FOLD_MAXLEN];
    let mut buf2 = [0u8; ONIGENC_MBC_CASE_FOLD_MAXLEN];
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
/// Returns a callout result or an error/mismatch code from the builtin.
fn run_builtin_callout(
    reg: &RegexType,
    num: i32,
    _id: i32,
    is_retraction: bool,
    callout_data: &mut [[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
    current_s: usize,
    msa: &mut MatchArg,
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
        CALLOUT_BUILTIN_MISMATCH => ONIG_MISMATCH,
        CALLOUT_BUILTIN_ERROR => {
            let Some(CalloutArg::Long(value)) = entry.args.first() else {
                return ONIGERR_INVALID_CALLOUT_ARG;
            };
            let code = *value as i32;
            if code >= 0 || crate::regerror::onig_is_error_code_needs_param(code) {
                ONIGERR_INVALID_CALLOUT_BODY
            } else {
                code
            }
        }
        CALLOUT_BUILTIN_CMP => {
            let lv = resolve_callout_arg(&entry.args[0], &ext.callout_list, callout_data);
            let rv = resolve_callout_arg(&entry.args[2], &ext.callout_list, callout_data);
            builtin_cmp(entry, &mut callout_data[idx], lv, rv)
        }
        CALLOUT_BUILTIN_SKIP => {
            if !is_retraction && current_s > msa.skip_search {
                msa.skip_search = current_s;
            }
            ONIG_CALLOUT_SUCCESS
        }
        _ => ONIG_CALLOUT_SUCCESS,
    }
}

/// Invoke a registered callback with the same match context as the C
/// `CALLOUT_BODY` macro. The temporary cell lets the public callout-args
/// accessors update the VM's callout-data and skip-search state without unsafe
/// pointers or holding a registry lock across user code.
#[allow(clippy::too_many_arguments)]
fn invoke_callout(
    callback: OnigCalloutFunc,
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
    msa: &mut MatchArg,
    callout_data: &mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
) -> i32 {
    let data = std::mem::take(callout_data);
    let restore = CalloutDataRestore {
        destination: callout_data,
        data: RefCell::new(data),
    };
    let skip_search = Cell::new(msa.skip_search);
    let args = OnigCalloutArgs::new(
        callout_in,
        name_id,
        num,
        reg,
        str_data,
        end,
        start,
        right_range,
        current,
        retry_counter,
    )
    .with_callout_data(&restore.data)
    .with_skip_search(&skip_search);
    let result = callback(&args, msa.callout_user_data as *mut std::ffi::c_void);
    msa.skip_search = skip_search.get();
    result
}

/// Restores the callout-data vector even if user code unwinds. C callbacks
/// cannot unwind through the ABI, but Rust callbacks can panic.
struct CalloutDataRestore<'a> {
    destination: &'a mut Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>,
    data: RefCell<Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>>,
}

impl Drop for CalloutDataRestore<'_> {
    fn drop(&mut self) {
        *self.destination = std::mem::take(self.data.get_mut());
    }
}

/// Run a builtin callout in the retraction direction (called from stack_pop).
///
/// Callout retraction is triggered during backtracking when a callout was
/// previously executed in the forward direction. This is internal engine
/// bookkeeping for `(*MAX)` / `(*COUNT)` callouts and not directly testable
/// without exposing callout internals.
#[cfg_attr(coverage_nightly, coverage(off))]
fn run_builtin_callout_retraction(
    reg: &RegexType,
    num: i32,
    _id: i32,
    callout_data: &mut [[i64; ONIG_CALLOUT_DATA_SLOT_NUM]],
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
                    if t == tag && i < all_data.len() {
                        return all_data[i][0];
                    }
                }
            }
            0
        }
        _ => 0,
    }
}

/// Builtin callout implementation for `(*MAX{n})`.
/// Internal engine bookkeeping not directly testable via the public API.
#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Builtin callout implementation for `(*COUNT)`.
#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Builtin callout implementation for `(*CMP{...})`.
#[cfg_attr(coverage_nightly, coverage(off))]
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

    if result {
        ONIG_CALLOUT_SUCCESS
    } else {
        ONIG_CALLOUT_FAIL
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Whether stepping back from `next` with `prev_char_head` lands on `x`, the
/// start of the character a star loop's forward scan consumed as `x..next`.
///
/// Star opcodes record a run as one `AltLazy` entry and retrace it with
/// `prev_char_head`. On malformed multibyte input that can skip a boundary or
/// land inside a sequence the scan consumed whole, so such runs fall back to
/// one backtrack entry per character (`push_char_boundaries`). In UTF-8 the
/// check reads the continuation bytes instead of calling into the encoding.
#[inline(always)]
fn steps_back_to(enc: OnigEncoding, str_data: &[u8], start: usize, x: usize, next: usize) -> bool {
    if std::ptr::addr_eq(enc, &crate::encodings::utf8::ONIG_ENCODING_UTF8) {
        str_data[x + 1..next].iter().all(|&b| b & 0xC0 == 0x80)
            && (x == start || str_data[x] & 0xC0 != 0x80)
    } else {
        prev_char_head(enc, start, next, str_data) == x
    }
}

/// Push the backtrack points of a greedy single-character loop that consumed
/// `start..end`: one `AltLazy` range when stepping back retraces the scan
/// (`exact_heads`), otherwise one entry per character boundary.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn push_char_run(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    enc: OnigEncoding,
    str_data: &[u8],
    start: usize,
    end: usize,
    single_byte: bool,
    exact_heads: bool,
) {
    if end <= start {
        return;
    }
    if single_byte {
        stack.push(StackEntry::AltLazy {
            pcode,
            pstr: end - 1,
            pstr_start: start,
            ascii: true,
            peek_byte: 0,
        });
    } else if exact_heads {
        stack.push(StackEntry::AltLazy {
            pcode,
            pstr: prev_char_head(enc, start, end, str_data),
            pstr_start: start,
            ascii: false,
            peek_byte: 0,
        });
    } else {
        push_char_boundaries(stack, pcode, enc, str_data, start, end);
    }
}

/// Rust-only (ADR-008): `PopStarChoices` at `pc`. Out of line, so that the
/// matcher's loop keeps its code for the patterns without it.
#[inline(never)]
fn pop_star_choices(stack: &mut Vec<StackEntry>, pc: usize) {
    while let Some(
        StackEntry::Alt {
            pcode,
            is_super: false,
            ..
        }
        | StackEntry::AltLazy { pcode, .. },
    ) = stack.last()
    {
        if *pcode != pc {
            break;
        }
        stack.pop();
    }
}

/// Greedy run of a negated single-character loop (`[^class]*`) from `start`.
///
/// `ascii_member(b)` and `member_end(x)` (for a non-ASCII byte at `x`, the
/// position after its character) mirror the loop's single-character opcode,
/// so the run consumes exactly what the unoptimized `PUSH; CCLASS_NOT; JUMP`
/// loop consumes. Out of line: negated loops are long (strings, comments),
/// and their code inlined into `match_at_impl` slowed unrelated patterns.
/// A possessive run (`BACKTRACK = false`) needs neither the choices nor the
/// checks that make retracing those choices safe on malformed byte input.
/// Returns the end of the run.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn greedy_char_loop<const BACKTRACK: bool>(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    enc: OnigEncoding,
    str_data: &[u8],
    start: usize,
    right_range: usize,
    ascii_member: impl Fn(u8) -> bool,
    member_end: impl Fn(usize) -> Option<usize>,
) -> usize {
    let mut s = start;
    let mut single_byte = true;
    let mut exact_heads = true;
    while s < right_range {
        let b = str_data[s];
        if b < 0x80 {
            if !ascii_member(b) {
                break;
            }
            s += 1;
            continue;
        }
        let Some(next) = member_end(s) else {
            break;
        };
        // A backward search can run the loop past the logical end, where
        // `member_end` decodes nothing and cannot advance.
        if next <= s {
            break;
        }
        if BACKTRACK {
            single_byte &= next == s + 1;
            exact_heads &= steps_back_to(enc, str_data, start, s, next);
        }
        s = next;
    }
    if BACKTRACK {
        push_char_run(
            stack,
            pcode,
            enc,
            str_data,
            start,
            s,
            single_byte,
            exact_heads,
        );
    }
    s
}

/// Keep possessive-run selection outside the shared VM dispatch. Explicit
/// stack limits retain the choices that the following pop would discard.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn run_negated_byte_star(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    reg: &RegexType,
    str_data: &[u8],
    start: usize,
    right_range: usize,
    end: usize,
    match_stack_limit: u32,
    bsp: &BitSet,
    ascii_fast: CClassAsciiFastKind,
) -> (usize, usize) {
    let fuse = match_stack_limit == 0
        && reg
            .ops
            .get(pcode)
            .is_some_and(|next| next.opcode == OpCode::PopStarChoices);
    #[cfg(test)]
    let fuse = fuse && !NEGATED_STAR_FUSION_DISABLED.with(Cell::get);
    let stop = if fuse {
        greedy_not_byte_class_loop::<false>(
            stack,
            pcode,
            reg.enc,
            str_data,
            start,
            right_range,
            end,
            bsp,
            ascii_fast,
        )
    } else {
        greedy_not_byte_class_loop::<true>(
            stack,
            pcode,
            reg.enc,
            str_data,
            start,
            right_range,
            end,
            bsp,
            ascii_fast,
        )
    };
    (stop, pcode + usize::from(fuse))
}

/// Choose the negated byte-class test once per run, outside the character
/// loop. Keeping the choice in its closure makes each byte dispatch on
/// `ascii_fast`, even though the class cannot change during the run.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn greedy_not_byte_class_loop<const BACKTRACK: bool>(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    enc: OnigEncoding,
    str_data: &[u8],
    start: usize,
    right_range: usize,
    end: usize,
    bsp: &BitSet,
    ascii_fast: CClassAsciiFastKind,
) -> usize {
    // A delimiter search can skip a long ASCII run. The ASCII check is
    // essential for byte input: a malformed multibyte head may consume an
    // ASCII delimiter as a trail byte, in which case the character walker
    // below must decide where the run ends.
    if right_range.saturating_sub(start) >= 32 {
        let text = &str_data[start..right_range];
        let found = match ascii_fast {
            CClassAsciiFastKind::Eq(c) => Some(memchr::memchr(c, text)),
            CClassAsciiFastKind::EqFoldLower(lower) => {
                Some(memchr::memchr2(lower, lower.to_ascii_uppercase(), text))
            }
            CClassAsciiFastKind::None => None,
        };
        if let Some(found) = found {
            let len = found.unwrap_or(text.len());
            if text[..len].is_ascii() {
                let stop = start + len;
                if BACKTRACK {
                    push_char_run(stack, pcode, enc, str_data, start, stop, true, true);
                }
                return stop;
            }
        }
    }
    macro_rules! run {
        ($member:expr) => {{
            let member = $member;
            greedy_char_loop::<BACKTRACK>(
                stack,
                pcode,
                enc,
                str_data,
                start,
                right_range,
                member,
                |x| member(str_data[x]).then(|| advance_char_to_end(enc, str_data, x, end)),
            )
        }};
    }
    match ascii_fast {
        CClassAsciiFastKind::Eq(c) => run!(|b| b != c),
        CClassAsciiFastKind::EqFoldLower(lower) => run!(|b: u8| b >= 0x80 || (b | 0x20) != lower),
        CClassAsciiFastKind::None => run!(|b| !bitset_at(bsp, b as usize)),
    }
}

/// Greedy run of a bounded single-character loop (`C{min,max}`, Rust-only
/// ADR-008) from `start`: up to `max` characters, and `None` when fewer than
/// `min` match.
///
/// The backtrack points are those of `greedy_char_loop`, from the position
/// after the first `min` characters on: the counter loop tries the exit after
/// `min` characters last, and nothing before it. `ascii_member(b)` and
/// `member_end(x)` mirror the body's single-character opcode, as in
/// `greedy_char_loop`.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn greedy_char_repeat(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    enc: OnigEncoding,
    str_data: &[u8],
    start: usize,
    right_range: usize,
    min: usize,
    max: usize,
    ascii_member: impl Fn(u8) -> bool,
    member_end: impl Fn(usize) -> Option<usize>,
) -> Option<usize> {
    let mut s = start;
    let mut count = 0;
    // The first position that may be given back: after `min` characters.
    let mut floor = start;
    let mut single_byte = true;
    let mut exact_heads = true;
    while count < max && s < right_range {
        let b = str_data[s];
        let next = if b < 0x80 {
            if !ascii_member(b) {
                break;
            }
            s + 1
        } else {
            let Some(next) = member_end(s) else {
                break;
            };
            if next <= s {
                break;
            }
            single_byte &= next == s + 1;
            exact_heads &= steps_back_to(enc, str_data, floor, s, next);
            next
        };
        s = next;
        count += 1;
        if count == min {
            floor = s;
            single_byte = true;
            exact_heads = true;
        }
    }
    if count < min {
        return None;
    }
    push_char_run(
        stack,
        pcode,
        enc,
        str_data,
        floor,
        s,
        single_byte,
        exact_heads,
    );
    Some(s)
}

/// Rust-only (ADR-008): the run of a counted character repeat (`CClassRepeat`
/// or `AnyCharRepeat`) from `start`, with its backtrack points pushed. `None`
/// when the run is shorter than the minimum. Out of line: the VM's dispatch
/// stays as it was for the patterns without these loops.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn counted_char_repeat_run(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    enc: OnigEncoding,
    str_data: &[u8],
    start: usize,
    right_range: usize,
    end: usize,
    payload: &OperationPayload,
) -> Option<usize> {
    match *payload {
        OperationPayload::CClassRepeat {
            ref bsp,
            ascii_fast,
            not,
            min,
            max,
            peek,
            ascii_only,
        } => {
            let excluded = |b: u8| match ascii_fast {
                CClassAsciiFastKind::Eq(c) => b == c,
                CClassAsciiFastKind::EqFoldLower(lower) => b < 0x80 && (b | 0x20) == lower,
                CClassAsciiFastKind::None => bitset_at(bsp, b as usize),
            };
            let member = |b: u8| excluded(b) != not;
            if ascii_only {
                greedy_ascii_repeat(
                    stack,
                    pcode,
                    str_data,
                    start,
                    right_range,
                    min as usize,
                    max as usize,
                    peek,
                    member,
                )
            } else {
                greedy_char_repeat(
                    stack,
                    pcode,
                    enc,
                    str_data,
                    start,
                    right_range,
                    min as usize,
                    max as usize,
                    member,
                    |x| member(str_data[x]).then(|| advance_char_to_end(enc, str_data, x, end)),
                )
            }
        }
        OperationPayload::AnyCharRepeat {
            multiline,
            min,
            max,
        } => {
            let member_end = |x: usize| {
                let n = enclen(enc, str_data, x);
                if right_range.saturating_sub(x) < n
                    || (!multiline && enc.is_mbc_newline(&str_data[x..], end))
                {
                    None
                } else {
                    Some(x + n)
                }
            };
            greedy_char_repeat(
                stack,
                pcode,
                enc,
                str_data,
                start,
                right_range,
                min as usize,
                max as usize,
                |b| multiline || b != b'\n',
                member_end,
            )
        }
        _ => None,
    }
}

/// `greedy_char_repeat` for a body that matches one-byte characters only
/// (`CClassRepeat` with `ascii_only`): the run is a plain byte scan, and its
/// backtrack points are one `AltLazy` range from `start + min`, or, with a
/// follower byte `peek`, only the positions holding it. Inlined into
/// `counted_char_repeat_run`, its only caller, so the run is one call.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn greedy_ascii_repeat(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    str_data: &[u8],
    start: usize,
    right_range: usize,
    min: usize,
    max: usize,
    peek: u8,
    member: impl Fn(u8) -> bool,
) -> Option<usize> {
    let limit = right_range.min(start.saturating_add(max));
    let mut s = start;
    while s < limit && member(str_data[s]) {
        s += 1;
    }
    if s - start < min {
        return None;
    }
    // The first position that may be given back: after `min` characters.
    let floor = start + min;
    if s > floor {
        if peek == 0 {
            stack.push(StackEntry::AltLazy {
                pcode,
                pstr: s - 1,
                pstr_start: floor,
                ascii: true,
                peek_byte: 0,
            });
        } else {
            // Runs are mostly a few bytes: a short scan beats the `memrchr` call.
            let run = &str_data[floor..s];
            let found = if run.len() <= 16 {
                run.iter().rposition(|&b| b == peek)
            } else {
                memchr::memrchr(peek, run)
            };
            if let Some(at) = found {
                stack.push(StackEntry::AltLazy {
                    pcode,
                    pstr: floor + at,
                    pstr_start: floor,
                    ascii: true,
                    peek_byte: peek,
                });
            }
        }
    }
    Some(s)
}

/// Cache ASCII compatibility for one dot-star run while retaining every
/// original backtrack entry and encoded-character boundary.
#[inline(never)]
fn greedy_anychar_loop(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    enc: OnigEncoding,
    text: &[u8],
    start: usize,
    limit: usize,
) -> usize {
    let ascii_compatible =
        dot_star_ascii_cache_enabled() && onigenc_is_ascii_compatible_encoding(enc);
    let mut at = start;
    while at < limit {
        let len = if ascii_compatible && text[at] < 0x80 {
            1
        } else {
            enclen(enc, text, at)
        };
        if at + len > limit {
            break;
        }
        stack.push(StackEntry::Alt {
            pcode,
            pstr: at,
            zid: -1,
            is_super: false,
        });
        at += len;
    }
    at
}

#[inline(always)]
fn dot_star_ascii_cache_enabled() -> bool {
    #[cfg(test)]
    {
        !DOT_STAR_ASCII_CACHE_DISABLED.with(Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// Push one backtrack entry per character boundary of `start..run_end`, as
/// the unoptimized loop does. Only reached for malformed multibyte input.
///
/// Every single-character opcode behind the star loops steps over an ASCII
/// byte by one and over anything else by `enclen`, where a character cut by
/// the range ends the run, so the boundaries are retraced without calling
/// the loop's character test again.
#[cold]
#[inline(never)]
fn push_char_boundaries(
    stack: &mut Vec<StackEntry>,
    pcode: usize,
    enc: OnigEncoding,
    str_data: &[u8],
    start: usize,
    run_end: usize,
) {
    let mut x = start;
    while x < run_end {
        stack.push(StackEntry::Alt {
            pcode,
            pstr: x,
            zid: -1,
            is_super: false,
        });
        x = x.saturating_add(enclen(enc, str_data, x)).min(run_end);
    }
}

/// Match a literal alternation compiled to a trie at `s`, as the ordered
/// alternation would: continue with the literal that comes first and, when
/// several literals match (one is a prefix of another), leave the others as
/// backtracking alternatives in alternation order, resuming at `next_pcode`
/// after their end. Returns the length of the first match.
#[inline(never)]
fn match_literal_trie(
    stack: &mut Vec<StackEntry>,
    trie: &crate::literal_trie::LiteralTrie,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    next_pcode: usize,
) -> Option<usize> {
    let (len, others) = trie.first_match(str_data, s, right_range)?;
    if others {
        let matches = trie.matches_in_order(str_data, s, right_range);
        for &(_, alternative) in matches[1..].iter().rev() {
            stack.push(StackEntry::Alt {
                pcode: next_pcode,
                pstr: s + alternative,
                zid: -1,
                is_super: false,
            });
        }
    }
    Some(len)
}

/// Batch short ASCII suffixes for the VM's initial look-behind step.
/// Out of line to keep the batch checks out of the VM dispatch layout.
/// The shared character walker keeps its original one-character path.
#[inline(never)]
fn step_back_look_behind(
    enc: OnigEncoding,
    start: usize,
    s: usize,
    data: &[u8],
    n: usize,
) -> Option<usize> {
    // ASCII bytes advance one at a time in an ASCII-compatible encoding.
    // The head check also excludes a suffix beginning on an ASCII-valued
    // trail byte, as Shift_JIS and Big5 can have. Limit the extra inspection
    // to short look-behind steps; mixed text keeps the original walk.
    if (2..=16).contains(&n)
        && let Some(target) = s.checked_sub(n)
        && target >= start
        && data.get(target..s).is_some_and(<[u8]>::is_ascii)
        && onigenc_is_ascii_compatible_encoding(enc)
        && enc.left_adjust_char_head(start, target, data) == target
    {
        return Some(target);
    }
    onigenc_step_back(enc, start, s, data, n)
}

/// Rust-only (ADR-008): decides a one-character look-behind whose body is a
/// word type (`\w`, `\W`) or a single-byte class, when the character before
/// `s` is ASCII. It gives what `look_behind_body_end` gives, without the
/// general step back and instruction dispatch. `None` leaves the look-behind
/// to `look_behind_body_end`: a longer look-behind, a multibyte encoding, a
/// non-ASCII or malformed character, or another body. The common word-boundary
/// and character-class tests of a grammar take this path.
#[inline(never)]
fn look_behind_ascii_end(
    body: &Operation,
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    end: usize,
) -> Option<Option<usize>> {
    if s == 0 {
        // Stepping back one character from the start fails.
        return Some(None);
    }
    let at = s - 1;
    let b = *str_data.get(at)?;
    if b >= 0x80 || at >= right_range || at >= end {
        return None;
    }
    // An ASCII byte is a character head in ASCII and UTF-8, so the step back
    // stays at `at`; the check keeps the encoding's own rule.
    if enc.left_adjust_char_head(0, at, str_data) != at {
        return None;
    }
    // Results as `single_op_end` gives them for the one-byte character `b`:
    // a one-byte step is `s`, a character advance is clamped to the end.
    let word = is_word_ascii(b);
    Some(match (body.opcode, &body.payload) {
        (OpCode::Word, _) => word.then(|| s.min(end)),
        (OpCode::NoWord | OpCode::NoWordAscii, _) => (!word).then(|| s.min(end)),
        (OpCode::WordAscii, _) => word.then_some(s),
        (OpCode::CClass | OpCode::CClassNot, OperationPayload::CClass { bsp, ascii_fast }) => {
            let not = body.opcode == OpCode::CClassNot;
            let (in_class, step) = match *ascii_fast {
                CClassAsciiFastKind::Eq(c) => (b == c, !not),
                CClassAsciiFastKind::EqFoldLower(lower) => ((b | 0x20) == lower, !not),
                CClassAsciiFastKind::None => (bitset_at(bsp, b as usize), false),
            };
            (in_class != not).then(|| if step { s } else { s.min(end) })
        }
        // A class the bytecode pass fused with the classes after it: the
        // look-behind evaluates only this one, as `single_op_end` does.
        (OpCode::CClassRun, OperationPayload::CClassRun { bsp, .. }) => {
            bitset_at(bsp, b as usize).then_some(s)
        }
        (OpCode::CClassMix | OpCode::CClassMixNot, OperationPayload::CClassMix { bsp, .. }) => {
            let not = body.opcode == OpCode::CClassMixNot;
            (bitset_at(bsp, b as usize) != not).then_some(s)
        }
        _ => return None,
    })
}

/// `LookBehindOp`: STEP_BACK_START by `char_len` characters from `s`, then
/// the body instruction `body` at that position; where the body ends if it
/// matches. Out of line: code added to
/// `match_at_impl` shifts its layout (see the performance analysis).
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn look_behind_body_end(
    body: &Operation,
    char_len: u32,
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    end: usize,
    ascii_compatible: bool,
) -> Option<usize> {
    // Batch the same short ASCII suffixes as STEP_BACK_START, without
    // another helper call. The head check excludes ASCII-valued trail
    // bytes in multibyte encodings; one-character bodies keep their walk.
    let at = if fused_look_behind_ascii_batches_enabled()
        && (2..=16).contains(&char_len)
        && let Some(at) = s.checked_sub(char_len as usize)
        && str_data.get(at..s).is_some_and(<[u8]>::is_ascii)
        && ascii_compatible
        && enc.left_adjust_char_head(0, at, str_data) == at
    {
        Some(at)
    } else {
        onigenc_step_back(enc, 0, s, str_data, char_len as usize)
    };
    at.and_then(|at| single_op_end(body, enc, str_data, at, right_range, end))
}

/// Whether a guarded push with `skipped_retries` may jump. One whose count
/// depends on position checks (`GUARD_RETRIES_BY_CHECKS`) counts its largest
/// count when it jumps, so it only jumps where that upper bound is good
/// enough (`guards_may_count_upper_bounds`); otherwise it pushes like the
/// unguarded instruction, which counts exactly.
#[inline(always)]
fn guard_may_jump(skipped_retries: u32, reg: &RegexType, msa: &MatchArg, exact: bool) -> bool {
    skipped_retries & GUARD_RETRIES_BY_CHECKS == 0
        || (!exact && guards_may_count_upper_bounds(reg, msa))
}

/// Whether guards may count an upper bound of their backtracks: only the
/// retry limit in match reads the count (no search budget, time limit,
/// callouts or counted subexpression calls), and FIND_LONGEST keeps no state
/// across the attempt. If the upper bound trips the limit, `match_at_impl`
/// starts the attempt over with exact counts.
fn guards_may_count_upper_bounds(reg: &RegexType, msa: &MatchArg) -> bool {
    msa.retry_limit_in_search == 0
        && msa.time_limit == 0
        && reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0)
        && onig_get_subexp_call_limit_in_search() == 0
        && !opton_find_longest(msa.options)
}

/// The backtracks a guard counts when it jumps: its fixed count, or the
/// largest count of one that depends on checks.
#[inline(always)]
fn guard_retries(skipped_retries: u32) -> u64 {
    u64::from(skipped_retries & !GUARD_RETRIES_BY_CHECKS)
}

/// Rust-only (ADR-008): the skip gate a guard refers to (`GuardSkipGate`).
#[inline(always)]
fn skip_gate_of(reg: &RegexType, index: Option<u32>) -> Option<&GuardSkipGate> {
    index.and_then(|index| reg.skip_gates.get(index as usize))
}

/// Rust-only (ADR-008): whether a guard chain's skip gate jumps at `s`
/// (`GuardSkipGate`). It does when the current byte is outside the gate's
/// set. Then the walk would miss every guard of the chain and count their
/// backtracks one by one, so the gate counts the sum at once. It does so only
/// when no count can reach the retry limit in match or a time limit is set:
/// `count_retry` then does no more than add, and the sum equals the one-by-one
/// counts. Otherwise the guard runs as before.
#[inline(always)]
fn skip_gate_taken(
    gate: &GuardSkipGate,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    retry_in_match_counter: u64,
    retry_limit_in_match: u64,
    time_limit_ms: u64,
) -> bool {
    (s >= right_range || !bitset_at(&gate.bsp, str_data[s] as usize))
        && time_limit_ms == 0
        && (retry_limit_in_match == 0
            || retry_in_match_counter + u64::from(gate.skipped_retries) < retry_limit_in_match)
}

/// Where the single-character or string instruction `op` ends if it matches
/// at `s`, exactly as the VM executes and advances it. `LookBehindOp` checks
/// its body with it at the position it stepped back to.
#[inline]
fn single_op_end(
    op: &Operation,
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    end: usize,
) -> Option<usize> {
    let available = right_range.saturating_sub(s);
    match (op.opcode, &op.payload) {
        (
            OpCode::Str1 | OpCode::Str2 | OpCode::Str3 | OpCode::Str4 | OpCode::Str5,
            OperationPayload::Exact { s: exact },
        ) => {
            let n = op.opcode as usize - OpCode::Str1 as usize + 1;
            // A byte loop: a slice comparison of at most five bytes would
            // call memcmp.
            (available >= n
                && exact[..n]
                    .iter()
                    .zip(&str_data[s..s + n])
                    .all(|(a, b)| a == b))
            .then_some(s + n)
        }
        (OpCode::StrN, OperationPayload::ExactN { s: exact, n }) => {
            let n = *n as usize;
            (available >= n && exact_eq_at(str_data, s, exact, n)).then_some(s + n)
        }
        (
            OpCode::StrMb2n1
            | OpCode::StrMb2n2
            | OpCode::StrMb2n3
            | OpCode::StrMb2n
            | OpCode::StrMb3n
            | OpCode::StrMbn,
            OperationPayload::ExactLenN { s: exact, n, .. },
        ) => {
            let n = *n as usize;
            (available >= n && exact_eq_at(str_data, s, exact, n)).then_some(s + n)
        }
        (OpCode::CClassRun, OperationPayload::CClassRun { bsp, .. }) => {
            // A fused look-behind evaluates just its original single class;
            // the adjacent forward classes remain at their original addresses.
            (available > 0 && bitset_at(bsp, str_data[s] as usize)).then_some(s + 1)
        }
        (OpCode::CClass | OpCode::CClassNot, OperationPayload::CClass { bsp, ascii_fast }) => {
            if available == 0 {
                return None;
            }
            let b = str_data[s];
            let not = op.opcode == OpCode::CClassNot;
            let (in_class, step) = match *ascii_fast {
                CClassAsciiFastKind::Eq(c) => (b == c, !not),
                CClassAsciiFastKind::EqFoldLower(lower) => (b < 0x80 && (b | 0x20) == lower, !not),
                CClassAsciiFastKind::None => (bitset_at(bsp, b as usize), false),
            };
            // A one-byte fast path steps one byte; the others one character.
            (in_class != not).then(|| {
                if step {
                    s + 1
                } else {
                    advance_char_to_end(enc, str_data, s, end)
                }
            })
        }
        (OpCode::CClassMb | OpCode::CClassMbNot, OperationPayload::CClassMb { mb }) => {
            let not = op.opcode == OpCode::CClassMbNot;
            if available == 0 {
                return None;
            }
            let b = str_data[s];
            if b < 0x80 {
                return (is_in_code_range(mb, b as OnigCodePoint) != not).then_some(s + 1);
            }
            let len = enclen(enc, str_data, s);
            if len == 1 {
                // A byte that starts no multibyte character matches only the
                // negated class.
                return not.then_some(s + 1);
            }
            if s + len > right_range {
                // A truncated character matches only the negated class,
                // which goes on at the end.
                return not.then_some(end);
            }
            let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
            (is_in_code_range(mb, code) != not).then_some(s + len)
        }
        (OpCode::CClassMix | OpCode::CClassMixNot, OperationPayload::CClassMix { bsp, mb }) => {
            let not = op.opcode == OpCode::CClassMixNot;
            if available == 0 {
                return None;
            }
            let b = str_data[s];
            if b < 0x80 {
                return (bitset_at(bsp, b as usize) != not).then_some(s + 1);
            }
            let len = enclen(enc, str_data, s);
            if s + len > right_range {
                return not.then_some(end);
            }
            let in_class = if len == 1 {
                bitset_at(bsp, b as usize)
            } else {
                let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
                is_in_code_range(mb, code)
            };
            (in_class != not).then_some(s + len)
        }
        // `\w` and `\W` as the VM's `Word`, `NoWord`, `WordAscii` and
        // `NoWordAscii` arms check and advance them.
        (OpCode::Word | OpCode::NoWord, OperationPayload::None) => {
            if available == 0 {
                return None;
            }
            let word = op.opcode == OpCode::Word;
            (is_word_char_at(enc, str_data, s, end) == word)
                .then(|| advance_char_to_end(enc, str_data, s, end))
        }
        (OpCode::WordAscii, OperationPayload::None) => {
            (available > 0 && is_word_ascii(str_data[s])).then_some(s + 1)
        }
        (OpCode::NoWordAscii, OperationPayload::None) => {
            let non_word = available > 0 && !is_word_ascii(str_data[s]);
            non_word.then(|| advance_char_to_end(enc, str_data, s, end))
        }
        _ => unreachable!("look-behind body is a single character or string instruction"),
    }
}

/// Rust-only (ADR-008): a regex's first instruction that a RegSet position
/// loop evaluates before an attempt (`first_op_fails`). These are the
/// zero-width tests that the first-byte dispatch cannot see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FirstOpTest {
    /// `WordBoundary`, or `NoWordBoundary` with `not`.
    WordBoundary { mode: ModeType, not: bool },
    /// `BeginLine`.
    BeginLine,
    /// `LookBehindOp` with its one-instruction body at `ops[1]`.
    LookBehind {
        char_len: u32,
        not: bool,
        ascii_compatible: bool,
    },
}

/// The test for `reg`'s first instruction, if it is one a RegSet position
/// loop may evaluate on its own. Callouts observe every attempt, so a regex
/// with callouts has none.
pub(crate) fn first_op_test(reg: &RegexType) -> Option<FirstOpTest> {
    if reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0) {
        return None;
    }
    let op = reg.ops.first()?;
    match (op.opcode, &op.payload) {
        (OpCode::WordBoundary, &OperationPayload::WordBoundary { mode }) => {
            Some(FirstOpTest::WordBoundary { mode, not: false })
        }
        (OpCode::NoWordBoundary, &OperationPayload::WordBoundary { mode }) => {
            Some(FirstOpTest::WordBoundary { mode, not: true })
        }
        (OpCode::BeginLine, _) => Some(FirstOpTest::BeginLine),
        (
            OpCode::LookBehindOp,
            &OperationPayload::LookBehindOp {
                char_len,
                not,
                ascii_compatible,
            },
        ) if reg.ops.len() > 1 => Some(FirstOpTest::LookBehind {
            char_len,
            not,
            ascii_compatible,
        }),
        _ => None,
    }
}

/// Whether an attempt of `reg` at `s` fails at its first instruction `test`
/// (`first_op_test(reg)`), decided as `match_at_impl` decides it for a RegSet
/// attempt: right range `end`, options `option | reg.options`. Such an
/// attempt counts one retry and changes nothing else; the caller decides
/// whether the limits could observe that (`may_skip_first_op_failures`).
#[inline(never)]
pub(crate) fn first_op_fails(
    test: FirstOpTest,
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    s: usize,
    option: OnigOptionType,
) -> bool {
    let enc = reg.enc;
    match test {
        FirstOpTest::WordBoundary { mode, not } => {
            is_word_boundary_ascii_fast(enc, str_data, s, end, mode) == not
        }
        // The `BeginLine` arm.
        FirstOpTest::BeginLine => {
            if s == 0 {
                opton_notbol(option | reg.options)
            } else if s != end {
                let sprev = onigenc_get_prev_char_head(enc, str_data, 0, s);
                !enc.is_mbc_newline(&str_data[sprev..], end)
            } else {
                true
            }
        }
        FirstOpTest::LookBehind {
            char_len,
            not,
            ascii_compatible,
        } => {
            look_behind_body_end(
                &reg.ops[1],
                char_len,
                enc,
                str_data,
                s,
                end,
                end,
                ascii_compatible,
            )
            .is_some()
                == not
        }
    }
}

/// Whether a RegSet search may leave out attempts that fail at their first
/// instruction (`first_op_fails`). Each would count one retry: the limits
/// `may_skip_attempts` excludes could observe that. FIND_LONGEST in the
/// search options (a RegSet regex cannot have it) keeps every attempt, as for
/// the other plans that leave attempts out.
#[inline]
pub(crate) fn may_skip_first_op_failures(msa: &MatchArg, option: OnigOptionType) -> bool {
    may_skip_attempts(msa) && !opton_find_longest(option)
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
#[inline(always)]
fn match_at(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    in_right_range: usize,
    sstart: usize,
    msa: &mut MatchArg,
) -> i32 {
    match_at_in_scan(reg, str_data, end, in_right_range, sstart, msa, None)
}

/// `match_at`, which, given a forward scan, goes on with the scan's next
/// attempt where this one fails (Rust-only, see `ForwardScan`). The start
/// of the attempt that ends the call is then in `scan.at`.
#[inline(always)]
fn match_at_in_scan(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    in_right_range: usize,
    sstart: usize,
    msa: &mut MatchArg,
    scan: Option<&mut ForwardScan<'_>>,
) -> i32 {
    // Rust-only (ADR-008): the first attempt of a two-pass search records its
    // captures and makes no further attempt. The scan goes on from its
    // position as the search loop does, so the attempts after it stay
    // untracked.
    if msa.region.is_some() && reg.num_mem > 0 && msa.capture_position == Some(FIRST_ATTEMPT) {
        msa.capture_position = Some(sstart);
        let result =
            match_at_impl::<true, false>(reg, str_data, end, in_right_range, sstart, msa, None);
        return result;
    }
    // Untracked runs skip the MemStartPush/MemEndPush stack entries. With a
    // match stack limit configured that would change when the limit trips,
    // so such runs keep the bookkeeping to stay observably identical to C.
    // A region without capture groups only needs the match bounds, which
    // OP_END records either way.
    if (msa.region.is_some()
        && reg.num_mem > 0
        && msa.capture_position.is_none_or(|at| at == sstart))
        || reg.needs_capture_tracking
        || (msa.options.contains(ONIG_OPTION_CALLBACK_EACH_MATCH)
            && onig_get_callback_each_match().is_some())
        || (msa.match_stack_limit != 0 && (reg.push_mem_start | reg.push_mem_end) != 0)
    {
        match_at_impl::<true, false>(reg, str_data, end, in_right_range, sstart, msa, scan)
    } else {
        match_at_impl::<false, false>(reg, str_data, end, in_right_range, sstart, msa, scan)
    }
}

/// Invalidates the capture arrays of more groups than `MEM_STK_INLINE` holds.
/// Out of line: the fixed-length invalidation of the inline arrays would
/// otherwise merge with this one into a call to memset.
#[cold]
#[inline(never)]
fn invalidate_mem_stacks(mem_start_stk: &mut [MemPtr], mem_end_stk: &mut [MemPtr]) {
    mem_start_stk.fill(MemPtr::invalid());
    mem_end_stk.fill(MemPtr::invalid());
}

/// The retry limit of one attempt. As in C, the budget left in the search
/// caps the retries of this match, so one start position cannot overrun the
/// search limit.
#[inline]
fn retry_limit_of_attempt(msa: &MatchArg) -> u64 {
    let mut retry_limit_in_match = msa.retry_limit_in_match;
    if msa.retry_limit_in_search != 0 {
        let rem = msa
            .retry_limit_in_search
            .saturating_sub(msa.retry_limit_in_search_counter);
        if rem < retry_limit_in_match || retry_limit_in_match == 0 {
            retry_limit_in_match = rem;
        }
    }
    retry_limit_in_match
}

#[cfg(test)]
thread_local! {
    /// VM attempts (`match_at`) on this thread, for tests that bound the
    /// work of a search.
    pub(crate) static VM_ATTEMPTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Bytes the VM moved over on this thread (forward and back), for tests
    /// that bound the work of a search.
    pub(crate) static VM_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Bytes the optimizer's forward searches read on this thread.
    pub(crate) static FORWARD_SEARCH_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Keeps the original per-attempt calls for bounded optimizer windows.
    static OPTIMIZER_WINDOW_SCANS_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Searches start-anchored patterns with the general forward search, as before
    /// `anchored_start_search`.
    static ANCHORED_START_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Searches pure literal patterns with the VM, as before `pure_literal_first`.
    static PURE_LITERAL_FAST_PATH_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Differential reference for captured zero-distance map searches.
    static CAPTURE_MAP_SCANS_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Differential reference for the atomic class prefix filter.
    static ATOMIC_CLASS_PREFIX_DISABLED: Cell<bool> = const { Cell::new(false) };
    /// Runs dot stars with the original per-character encoding queries.
    static DOT_STAR_ASCII_CACHE_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Differential reference for one-byte BackRef1 comparisons.
    static SINGLE_BYTE_BACKREFS_DISABLED: Cell<bool> = const { Cell::new(false) };
    /// Differential reference for ASCII case-insensitive backref comparisons.
    static ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED: Cell<bool> = const { Cell::new(false) };
    /// Differential reference for fused look-behind character walks.
    static FUSED_LOOK_BEHIND_ASCII_BATCHES_DISABLED: Cell<bool> = const { Cell::new(false) };
    /// Executes possessive negated byte stars as the original star/pop pair.
    static NEGATED_STAR_FUSION_DISABLED: Cell<bool> = const { Cell::new(false) };
}

#[inline(always)]
fn fused_look_behind_ascii_batches_enabled() -> bool {
    #[cfg(test)]
    {
        !FUSED_LOOK_BEHIND_ASCII_BATCHES_DISABLED.with(Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

#[inline(always)]
fn ascii_case_insensitive_backrefs_enabled() -> bool {
    #[cfg(test)]
    {
        !ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED.with(Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

#[inline(always)]
fn single_byte_backrefs_enabled() -> bool {
    #[cfg(test)]
    {
        !SINGLE_BYTE_BACKREFS_DISABLED.with(Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

#[inline(always)]
fn pure_literal_fast_path_enabled() -> bool {
    #[cfg(test)]
    {
        !PURE_LITERAL_FAST_PATH_DISABLED.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// Sets the test-only toggle of the anchored-start search (`true` keeps the
/// general path). For the tests of other modules.
#[cfg(test)]
pub(crate) fn set_anchored_start_disabled(disabled: bool) {
    ANCHORED_START_DISABLED.with(|v| v.set(disabled));
}

#[inline(always)]
fn anchored_start_enabled() -> bool {
    #[cfg(test)]
    {
        !ANCHORED_START_DISABLED.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

#[inline(always)]
fn optimizer_window_scans_enabled() -> bool {
    #[cfg(test)]
    {
        !OPTIMIZER_WINDOW_SCANS_DISABLED.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

// A separate map-continuation instantiation keeps its candidate-step checks
// out of the ordinary VM dispatch and preserves the existing ForwardScan layout.
fn match_at_impl<const TRACK_CAPTURES: bool, const OPTIMIZER_MAP: bool>(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    in_right_range: usize,
    mut sstart: usize,
    msa: &mut MatchArg,
    mut scan: Option<&mut ForwardScan<'_>>,
) -> i32 {
    #[cfg(test)]
    VM_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
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
    // C keeps both capture arrays in an alloca and invalidates them at the
    // start of every match_at. A tracked run with few groups keeps them on
    // the stack likewise, invalidated with a few fixed-size stores; one with
    // more groups reuses the buffers in `msa`. Untracked runs never touch
    // them.
    let mut mem_start_buf;
    let mut mem_end_buf;
    let mut mem_start_vec = Vec::new();
    let mut mem_end_vec = Vec::new();
    let (mem_start_stk, mem_end_stk): (&mut [MemPtr], &mut [MemPtr]) = if !TRACK_CAPTURES {
        (&mut [], &mut [])
    } else if num_mem < MEM_STK_INLINE {
        mem_start_buf = [MemPtr::invalid(); MEM_STK_INLINE];
        mem_end_buf = [MemPtr::invalid(); MEM_STK_INLINE];
        (&mut mem_start_buf, &mut mem_end_buf)
    } else {
        let need = num_mem + 1;
        mem_start_vec = std::mem::take(&mut msa.mem_start_stk);
        mem_start_vec.resize(need, MemPtr::invalid());
        mem_start_vec.fill(MemPtr::invalid());
        mem_end_vec = std::mem::take(&mut msa.mem_end_stk);
        mem_end_vec.resize(need, MemPtr::invalid());
        mem_end_vec.fill(MemPtr::invalid());
        (&mut mem_start_vec, &mut mem_end_vec)
    };

    let mut keep: usize = sstart;
    let mut best_len: i32 = ONIG_MISMATCH;
    let mut last_alt_zid: i32 = -1;

    // Safety limits.
    let mut retry_limit_in_match = retry_limit_of_attempt(msa);
    let mut retry_in_match_counter: u64 = 0;
    let match_stack_limit = msa.match_stack_limit;
    let stack_cap = match_stack_cap(match_stack_limit);
    let time_limit_ms = msa.time_limit;

    // Subexpression call limits (C: `subexp_call_nest_counter`,
    // `SubexpCallMaxNestLevel` and `SubexpCallLimitInSearch`). Like C, the
    // int nest level is compared as an unsigned long, so a negative level
    // never trips. Like C, OP_CALL reads both limits where it checks them.
    let mut subexp_call_nest_counter: u64 = 0;

    // Callout data: per-callout mutable slots (indexed by callout num - 1).
    // This is still attempt-local; C keeps these slots in MatchParam across
    // candidate starts. TOTAL_COUNT remains unsupported until that lifetime
    // is ported, so it is not recognized by prs_callout_of_name.
    let callout_count = reg.extp.as_ref().map_or(0, |e| e.callout_num as usize);
    let mut callout_data: Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]> = if callout_count == 0 {
        Vec::new()
    } else {
        vec![[0i64; ONIG_CALLOUT_DATA_SLOT_NUM]; callout_count]
    };

    // Push bottom sentinel (like C's STACK_PUSH_BOTTOM with FinishCode)
    stack.push(StackEntry::Alt {
        pcode: FINISH_PCODE,
        pstr: 0,
        zid: -1,
        is_super: false,
    });

    // Set once the attempt starts over because guards counted an upper
    // bound of their backtracks (see `guard_may_jump`).
    let mut exact_guard_retries = false;
    // Leaves the loop with a retry or time limit error. A retry limit that
    // an upper bound of the guards' backtracks reached may not be reached by
    // the exact count: the attempt then starts over from the same state with
    // those guards pushing, which counts exactly.
    macro_rules! stop_at_limit {
        ($err:expr) => {{
            let err = $err;
            if err == ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
                && reg.check_dependent_guards
                && !exact_guard_retries
                && guards_may_count_upper_bounds(reg, msa)
            {
                exact_guard_retries = true;
                p = 0;
                s = sstart;
                right_range = in_right_range;
                keep = sstart;
                last_alt_zid = -1;
                retry_in_match_counter = 0;
                subexp_call_nest_counter = 0;
                stack.clear();
                stack.push(StackEntry::Alt {
                    pcode: FINISH_PCODE,
                    pstr: 0,
                    zid: -1,
                    is_super: false,
                });
                if TRACK_CAPTURES {
                    mem_start_stk.fill(MemPtr::invalid());
                    mem_end_stk.fill(MemPtr::invalid());
                }
                continue;
            }
            best_len = err;
            break;
        }};
    }
    // Leaves the loop where the bottom of the stack is reached (C:
    // `finish`). Rust-only (ADR-008): where that ends a failed attempt of a
    // forward scan, the scan's next attempt starts here, with the state this
    // function's prologue sets up, instead of in a new call from the search
    // loop (see `ForwardScan`). Under FIND_LONGEST an attempt can reach the
    // bottom after a match; that ends the call as before. Per-attempt state
    // added to the prologue above must be reset here too.
    macro_rules! attempt_failed {
        () => {{
            if let Some(scan) = scan.as_deref_mut().filter(|_| best_len == ONIG_MISMATCH) {
                // C's match_at_end, then the search loop's step.
                msa.retry_limit_in_search_counter += retry_in_match_counter;
                retry_in_match_counter = 0;
                match scan.after_attempt::<OPTIMIZER_MAP>(reg, msa, str_data, sstart) {
                    Ok(Some(next)) => {
                        // Positions only grow, so this start runs with the
                        // same capture tracking as the scan's first one.
                        debug_assert!(msa.capture_position != Some(next));
                        scan.at = next;
                        sstart = next;
                        msa.best_len = ONIG_MISMATCH;
                        msa.best_s = 0;
                        p = 0;
                        s = next;
                        right_range = in_right_range;
                        keep = next;
                        last_alt_zid = -1;
                        // Only the search budget changes it between attempts.
                        if msa.retry_limit_in_search != 0 {
                            retry_limit_in_match = retry_limit_of_attempt(msa);
                        }
                        subexp_call_nest_counter = 0;
                        exact_guard_retries = false;
                        stack.clear();
                        stack.push(StackEntry::Alt {
                            pcode: FINISH_PCODE,
                            pstr: 0,
                            zid: -1,
                            is_super: false,
                        });
                        if TRACK_CAPTURES {
                            if num_mem < MEM_STK_INLINE {
                                // A fixed length stores in place, without
                                // a call to memset.
                                mem_start_stk[..MEM_STK_INLINE].fill(MemPtr::invalid());
                                mem_end_stk[..MEM_STK_INLINE].fill(MemPtr::invalid());
                            } else {
                                invalidate_mem_stacks(mem_start_stk, mem_end_stk);
                            }
                        }
                        continue;
                    }
                    Ok(None) => {}
                    Err(err) => best_len = err,
                }
            }
            break;
        }};
    }

    // ---- Main dispatch loop ----
    // Rust-only (ADR-008): the operation is fetched once per opcode, with one
    // bounds check, and the arms read the fetched one instead of indexing
    // `reg.ops[p]` again.
    let ops = &reg.ops[..];
    #[cfg(test)]
    let mut vm_previous = s;
    // Not a `while let`: the test-only byte count below must run before the
    // end-of-program check, as it did when this loop began with that check.
    #[allow(clippy::while_let_loop)]
    loop {
        #[cfg(test)]
        {
            VM_BYTES.with(|bytes| bytes.set(bytes.get() + s.abs_diff(vm_previous) as u64));
            vm_previous = s;
        }
        let Some(op) = ops.get(p) else {
            break;
        };

        // Stack limit check, once per opcode: C's STACK_PUSH stops at the push
        // that needs one entry more than the cap.
        if stack.len() > stack_cap {
            best_len = ONIGERR_MATCH_STACK_LIMIT_OVER;
            break;
        }

        let opcode = op.opcode;
        let mut goto_fail = false;

        match opcode {
            _ if goto_fail => {}
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
                    // C: `(int )(s - sstart)`. A look-behind that ends before
                    // its start over malformed UTF-8 can leave `s` before
                    // `sstart`; C then returns the negative length as is.
                    let n = s as i32 - sstart as i32;
                    if n == 0 && opton_find_not_empty(options) {
                        goto_fail = true;
                    } else if n > best_len || !opton_find_longest(options) {
                        best_len = n;

                        // Populate region with capture groups. An untracked
                        // run with groups has a region only while it defers
                        // the captures to a second pass (`capture_position`).
                        if let Some(region) = msa
                            .region
                            .as_mut()
                            .filter(|_| TRACK_CAPTURES || num_mem == 0)
                        {
                            if let Err(err) = populate_region_for_match(
                                region,
                                reg,
                                num_mem,
                                // C: `if (keep > s) keep = s;`
                                keep.min(s),
                                s,
                                &stack,
                                mem_start_stk,
                                mem_end_stk,
                            ) {
                                best_len = err;
                                break;
                            }
                        }

                        let each_match = if options.contains(ONIG_OPTION_CALLBACK_EACH_MATCH)
                            && (!opton_find_longest(options) || n > msa.callout_each_match_best_len)
                        {
                            onig_get_callback_each_match()
                        } else {
                            None
                        };
                        if let Some(callback) = each_match {
                            // The C callback accepts a nullable region. The
                            // Rust callback type is non-null, so synthesize a
                            // fully populated region when the caller omitted
                            // one.
                            let mut callback_region = OnigRegion::new();
                            let region = if let Some(region) = msa.region.as_ref() {
                                region
                            } else {
                                if let Err(err) = populate_region_for_match(
                                    &mut callback_region,
                                    reg,
                                    num_mem,
                                    keep.min(s),
                                    s,
                                    &stack,
                                    mem_start_stk,
                                    mem_end_stk,
                                ) {
                                    best_len = err;
                                    break;
                                }
                                &callback_region
                            };
                            let result = callback(
                                str_data,
                                region,
                                msa.callout_user_data as *mut std::ffi::c_void,
                            );
                            if result < 0 {
                                best_len = result;
                                break;
                            }
                            if !opton_find_longest(options) {
                                // C treats any nonnegative callback result as
                                // a request to backtrack and report the next
                                // match, rather than returning this one.
                                best_len = ONIG_MISMATCH;
                            } else {
                                msa.callout_each_match_best_len = n;
                                msa.best_len = best_len;
                                msa.best_s = sstart;
                            }
                            goto_fail = true;
                        // For non-FIND_LONGEST, return immediately when no
                        // each-match callback is installed.
                        } else if !opton_find_longest(options) {
                            msa.match_len = best_len;
                            // C leaves through match_at_end, which adds
                            // this match's retries to the search's count.
                            msa.retry_limit_in_search_counter += retry_in_match_counter;
                            msa.stack = stack;
                            if TRACK_CAPTURES && num_mem >= MEM_STK_INLINE {
                                msa.mem_start_stk = mem_start_vec;
                                msa.mem_end_stk = mem_end_vec;
                            }
                            return best_len;
                        } else {
                            // FIND_LONGEST: save best and continue searching
                            msa.best_len = best_len;
                            msa.best_s = sstart;
                            goto_fail = true; // backtrack to try longer matches
                        }
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
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::Exact { s: ref exact } = op.payload {
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
                } else if let OperationPayload::Exact { s: ref exact } = op.payload {
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
                } else if let OperationPayload::Exact { s: ref exact } = op.payload {
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
                } else if let OperationPayload::Exact { s: ref exact } = op.payload {
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
                } else if let OperationPayload::Exact { s: ref exact } = op.payload {
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
                if let OperationPayload::ExactN { s: ref exact, n } = op.payload {
                    let n = n as usize;
                    if right_range.saturating_sub(s) < n || !exact_eq_at(str_data, s, exact, n) {
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
            OpCode::StrMb2n1
            | OpCode::StrMb2n2
            | OpCode::StrMb2n3
            | OpCode::StrMb2n
            | OpCode::StrMb3n
            | OpCode::StrMbn => {
                // Multi-byte string comparison. ExactLenN.n = total byte count.
                if let OperationPayload::ExactLenN {
                    s: ref exact, n, ..
                } = op.payload
                {
                    let byte_len = n as usize;
                    if right_range.saturating_sub(s) < byte_len
                        || !exact_eq_at(str_data, s, exact, byte_len)
                    {
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
            OpCode::CClassRun => {
                if let OperationPayload::CClassRun { ref bsp, len } = op.payload {
                    // A backward search can extend past the logical end
                    // when its start is a truncated multibyte character.
                    // Keep the original per-instruction clamp in that mode.
                    if right_range > end {
                        if s < right_range && bitset_at(bsp, str_data[s] as usize) {
                            s = s.saturating_add(1).min(end);
                            p += 1;
                        } else {
                            goto_fail = true;
                        }
                    } else {
                        let len = usize::from(len);
                        let stop = s.saturating_add(len).min(right_range);
                        let mut remaining = len;
                        while s < stop && bitset_at(bsp, str_data[s] as usize) {
                            s += 1;
                            remaining -= 1;
                        }
                        if remaining == 0 {
                            p += len;
                        } else {
                            goto_fail = true;
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClass => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClass {
                    ref bsp,
                    ascii_fast,
                } = op.payload
                {
                    match ascii_fast {
                        CClassAsciiFastKind::Eq(c) => {
                            if str_data[s] != c {
                                goto_fail = true;
                            } else {
                                s += 1;
                                p += 1;
                            }
                        }
                        CClassAsciiFastKind::EqFoldLower(lower) => {
                            let b = str_data[s];
                            if b >= 0x80 || (b | 0x20) != lower {
                                goto_fail = true;
                            } else {
                                s += 1;
                                p += 1;
                            }
                        }
                        CClassAsciiFastKind::None => {
                            let b = str_data[s];
                            if !bitset_at(bsp, b as usize) {
                                goto_fail = true;
                            } else {
                                s = advance_char_to_end(enc, str_data, s, end);
                                p += 1;
                            }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassNot => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClass {
                    ref bsp,
                    ascii_fast,
                } = op.payload
                {
                    match ascii_fast {
                        CClassAsciiFastKind::Eq(c) => {
                            let b = str_data[s];
                            if b == c {
                                goto_fail = true;
                            } else {
                                s = advance_char_to_end(enc, str_data, s, end);
                                p += 1;
                            }
                        }
                        CClassAsciiFastKind::EqFoldLower(lower) => {
                            let b = str_data[s];
                            if b < 0x80 && (b | 0x20) == lower {
                                goto_fail = true;
                            } else {
                                s = advance_char_to_end(enc, str_data, s, end);
                                p += 1;
                            }
                        }
                        CClassAsciiFastKind::None => {
                            let b = str_data[s];
                            if bitset_at(bsp, b as usize) {
                                goto_fail = true;
                            } else {
                                s = advance_char_to_end(enc, str_data, s, end);
                                p += 1;
                            }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // MB character class (multibyte)
            OpCode::CClassMb => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMb { ref mb } = op.payload {
                    let b = str_data[s];
                    if b < 0x80 {
                        if !is_in_code_range(mb, b as OnigCodePoint) {
                            goto_fail = true;
                        } else {
                            s += 1;
                            p += 1;
                        }
                    } else {
                        let mb_len = enclen(enc, str_data, s);
                        // C: a byte that starts no multibyte character
                        // (`ONIGENC_IS_MBC_HEAD`) fails the class.
                        if mb_len == 1 || s + mb_len > right_range {
                            goto_fail = true;
                        } else {
                            let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
                            if !is_in_code_range(mb, code) {
                                goto_fail = true;
                            } else {
                                s += mb_len;
                                p += 1;
                            }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassMbNot => {
                if s >= right_range {
                    goto_fail = true;
                } else if let OperationPayload::CClassMb { ref mb } = op.payload {
                    let b = str_data[s];
                    if b < 0x80 {
                        if is_in_code_range(mb, b as OnigCodePoint) {
                            goto_fail = true;
                        } else {
                            s += 1;
                            p += 1;
                        }
                    } else {
                        let mb_len = enclen(enc, str_data, s);
                        if mb_len == 1 {
                            // C: a byte that starts no multibyte character
                            // matches the negated class as one byte.
                            s += 1;
                            p += 1;
                        } else if s + mb_len > right_range {
                            // Upstream treats a multibyte character truncated
                            // by the range as matching a negated class and
                            // goes on at the subject's end (C: `s = end`).
                            s = end;
                            p += 1;
                        } else {
                            let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
                            if is_in_code_range(mb, code) {
                                goto_fail = true;
                            } else {
                                s += mb_len;
                                p += 1;
                            }
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
                } else if let OperationPayload::CClassMix { ref bsp, ref mb } = op.payload {
                    let b = str_data[s];
                    if b < 0x80 {
                        let in_class = bitset_at(bsp, b as usize);
                        if in_class == not {
                            goto_fail = true;
                        } else {
                            s += 1;
                            p += 1;
                        }
                    } else {
                        let len = enclen(enc, str_data, s);
                        if s + len > right_range {
                            if not {
                                // Keep CClassMixNot consistent with CClassMbNot
                                // and consume the trailing truncated character.
                                s = end;
                                p += 1;
                            } else {
                                goto_fail = true;
                            }
                        } else {
                            let in_class = if len == 1 {
                                bitset_at(bsp, b as usize)
                            } else {
                                // C: cclass_mb. A multibyte character is only
                                // looked up in the code ranges.
                                let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
                                is_in_code_range(mb, code)
                            };
                            if in_class == not {
                                goto_fail = true;
                            } else {
                                s += len;
                                p += 1;
                            }
                        }
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
                // Use SIMD to find newline boundary, then push Alt entries up to that point.
                // In UTF-8/ASCII, 0x0a can only appear as a complete single-byte character,
                // so memchr finds the exact newline position.
                // An absent operator can lower right_range below s; C's loop
                // (DATA_ENSURE_CHECK1) then simply does not run.
                let nl_limit = if s < right_range {
                    memchr::memchr(b'\n', &str_data[s..right_range]).map_or(right_range, |i| s + i)
                } else {
                    s
                };
                if TRACK_CAPTURES {
                    while s < nl_limit {
                        let n = enclen(enc, str_data, s);
                        if s + n > nl_limit {
                            break;
                        }
                        stack.push(StackEntry::Alt {
                            pcode: p + 1,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
                        s += n;
                    }
                } else {
                    s = greedy_anychar_loop(&mut stack, p + 1, enc, str_data, s, nl_limit);
                }
                p += 1;
            }

            OpCode::AnyCharMlStar => {
                if TRACK_CAPTURES {
                    while s < right_range {
                        let n = enclen(enc, str_data, s);
                        if s + n > right_range {
                            break;
                        }
                        stack.push(StackEntry::Alt {
                            pcode: p + 1,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
                        s += n;
                    }
                } else {
                    s = greedy_anychar_loop(&mut stack, p + 1, enc, str_data, s, right_range);
                }
                p += 1;
            }

            OpCode::AnyCharStarPeekNext => {
                if let OperationPayload::AnyCharStarPeekNext { c } = op.payload {
                    // Find newline boundary with SIMD
                    // See AnyCharStar: right_range can be below s here.
                    let nl_limit = if s < right_range {
                        memchr::memchr(b'\n', &str_data[s..right_range])
                            .map_or(right_range, |i| s + i)
                    } else {
                        s
                    };
                    if c < 0x80 {
                        // ASCII peek byte: use SIMD to find all occurrences directly.
                        // In UTF-8, bytes < 0x80 can only be leading (single-byte) characters,
                        // so every memchr hit is a valid character start.
                        let mut iter_pos = s;
                        while let Some(i) = memchr::memchr(c, &str_data[iter_pos..nl_limit]) {
                            let pos = iter_pos + i;
                            stack.push(StackEntry::Alt {
                                pcode: p + 1,
                                pstr: pos,
                                zid: -1,
                                is_super: false,
                            });
                            iter_pos = pos + 1;
                        }
                        s = nl_limit;
                    } else {
                        // Non-ASCII peek: fall back to per-character loop
                        while s < nl_limit {
                            let n = enclen(enc, str_data, s);
                            if s + n > nl_limit {
                                break;
                            }
                            if s < end && str_data[s] == c {
                                stack.push(StackEntry::Alt {
                                    pcode: p + 1,
                                    pstr: s,
                                    zid: -1,
                                    is_super: false,
                                });
                            }
                            s += n;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::AnyCharMlStarPeekNext => {
                if let OperationPayload::AnyCharStarPeekNext { c } = op.payload {
                    if c < 0x80 {
                        // ASCII peek byte: use SIMD to find all occurrences directly.
                        // No newline check needed in multiline mode.
                        let mut iter_pos = s;
                        while let Some(i) = memchr::memchr(c, &str_data[iter_pos..right_range]) {
                            let pos = iter_pos + i;
                            stack.push(StackEntry::Alt {
                                pcode: p + 1,
                                pstr: pos,
                                zid: -1,
                                is_super: false,
                            });
                            iter_pos = pos + 1;
                        }
                        s = right_range;
                    } else {
                        // Non-ASCII peek: fall back to per-character loop
                        while s < right_range {
                            let n = enclen(enc, str_data, s);
                            if s + n > right_range {
                                break;
                            }
                            if s < end && str_data[s] == c {
                                stack.push(StackEntry::Alt {
                                    pcode: p + 1,
                                    pstr: s,
                                    zid: -1,
                                    is_super: false,
                                });
                            }
                            s += n;
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // CClass star opcodes - greedy [class]* / [class]+ optimization
            // ================================================================
            // Opt-in decimal prefix: reserve the final digit for the original
            // tail and record only the last successful repeated-body captures.
            OpCode::DecimalTailPrefix => {
                if let OperationPayload::DecimalTailPrefix { ref captures } = op.payload {
                    let (prefix_end, last) = decimal_tail_prefix_bounds(str_data, right_range, s);
                    s = prefix_end;
                    if TRACK_CAPTURES {
                        if let Some(bounds) = last {
                            record_decimal_prefix_captures(
                                reg,
                                captures,
                                bounds,
                                &mut stack,
                                mem_start_stk,
                                mem_end_stk,
                            );
                        }
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // Opt-in primitive possessive run. Its positive ASCII bitset
            // excludes malformed and multibyte UTF-8 heads. The mandatory
            // first character is compiled separately, just like [class]+.
            // No alternative is created, so no mark or cut is needed.
            OpCode::CClassPossessiveStar => {
                if let OperationPayload::CClass { ref bsp, .. } = op.payload {
                    while s < right_range && bitset_at(bsp, str_data[s] as usize) {
                        s += 1;
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassStar => {
                if let OperationPayload::CClass {
                    ref bsp,
                    ascii_fast,
                } = op.payload
                {
                    let start = s;
                    match ascii_fast {
                        CClassAsciiFastKind::Eq(c) => {
                            while s < right_range {
                                if str_data[s] != c {
                                    break;
                                }
                                s += 1;
                            }
                        }
                        CClassAsciiFastKind::EqFoldLower(lower) => {
                            while s < right_range {
                                let c = str_data[s];
                                if c >= 0x80 || (c | 0x20) != lower {
                                    break;
                                }
                                s += 1;
                            }
                        }
                        CClassAsciiFastKind::None => {
                            while s < right_range {
                                let c = str_data[s];
                                if !bitset_at(bsp, c as usize) {
                                    break;
                                }
                                s += 1;
                            }
                        }
                    }
                    if s > start {
                        stack.push(StackEntry::AltLazy {
                            pcode: p + 1,
                            pstr: s - 1,
                            pstr_start: start,
                            ascii: true,
                            peek_byte: 0,
                        });
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassMixStar => {
                if let OperationPayload::CClassMix { ref bsp, ref mb } = op.payload {
                    let start = s;
                    let mut exact_heads = true;
                    while s < right_range {
                        let b = str_data[s];
                        if b < 0x80 {
                            if !bitset_at(bsp, b as usize) {
                                break;
                            }
                            s += 1;
                            continue;
                        }

                        let len = enclen(enc, str_data, s);
                        if s + len > right_range {
                            break;
                        }

                        let in_class = if len == 1 {
                            bitset_at(bsp, b as usize)
                        } else {
                            let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
                            is_in_code_range(mb, code)
                        };
                        if !in_class {
                            break;
                        }
                        exact_heads &= steps_back_to(enc, str_data, start, s, s + len);
                        s += len;
                    }
                    push_char_run(
                        &mut stack,
                        p + 1,
                        enc,
                        str_data,
                        start,
                        s,
                        false,
                        exact_heads,
                    );
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassMbStar => {
                if let OperationPayload::CClassMb { ref mb } = op.payload {
                    let start = s;
                    let mut exact_heads = true;
                    while s < right_range {
                        let mb_len = enclen(enc, str_data, s);
                        // As `CClassMb`: a byte that starts no multibyte
                        // character ends the run.
                        if mb_len == 1 || s + mb_len > right_range {
                            break;
                        }
                        let code = enc.mbc_to_code(&str_data[s..], end.saturating_sub(s));
                        if !is_in_code_range(mb, code) {
                            break;
                        }
                        if str_data[s] >= 0x80 {
                            exact_heads &= steps_back_to(enc, str_data, start, s, s + mb_len);
                        }
                        s += mb_len;
                    }
                    push_char_run(
                        &mut stack,
                        p + 1,
                        enc,
                        str_data,
                        start,
                        s,
                        false,
                        exact_heads,
                    );
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // Negated class star opcodes: each mirrors its single-character
            // opcode (CClassNot / CClassMbNot / CClassMixNot) per character.
            OpCode::CClassNotStar => {
                if let OperationPayload::CClass {
                    ref bsp,
                    ascii_fast,
                } = op.payload
                {
                    (s, p) = run_negated_byte_star(
                        &mut stack,
                        p + 1,
                        reg,
                        str_data,
                        s,
                        right_range,
                        end,
                        match_stack_limit,
                        bsp,
                        ascii_fast,
                    );
                } else {
                    goto_fail = true;
                }
            }

            // Rust-only (ADR-008): `[class]{min,max}` and `.{min,max}`, the
            // counted loops of `CClassRepeat` and `AnyCharRepeat`.
            OpCode::CClassRepeat | OpCode::AnyCharRepeat => {
                match counted_char_repeat_run(
                    &mut stack,
                    p + 1,
                    enc,
                    str_data,
                    s,
                    right_range,
                    end,
                    &op.payload,
                ) {
                    Some(run_end) => {
                        s = run_end;
                        p += 1;
                    }
                    None => goto_fail = true,
                }
            }

            OpCode::CClassMbNotStar => {
                if let OperationPayload::CClassMb { ref mb } = op.payload {
                    s = greedy_char_loop::<true>(
                        &mut stack,
                        p + 1,
                        enc,
                        str_data,
                        s,
                        right_range,
                        |b| !is_in_code_range(mb, b as OnigCodePoint),
                        |x| {
                            let mb_len = enclen(enc, str_data, x);
                            if mb_len == 1 {
                                // As `CClassMbNot`: a byte that starts no
                                // multibyte character matches as one byte.
                                return Some(x + 1);
                            }
                            if x + mb_len > right_range {
                                // A truncated character matches a negated
                                // class, which goes on at the end.
                                return Some(end);
                            }
                            let code = enc.mbc_to_code(&str_data[x..], end.saturating_sub(x));
                            (!is_in_code_range(mb, code)).then_some(x + mb_len)
                        },
                    );
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::CClassMixNotStar => {
                if let OperationPayload::CClassMix { ref bsp, ref mb } = op.payload {
                    s = greedy_char_loop::<true>(
                        &mut stack,
                        p + 1,
                        enc,
                        str_data,
                        s,
                        right_range,
                        |b| !bitset_at(bsp, b as usize),
                        |x| {
                            let b = str_data[x];
                            let len = enclen(enc, str_data, x);
                            if x + len > right_range {
                                // A truncated character matches a negated
                                // class, which goes on at the end.
                                return Some(end);
                            }
                            let in_class = if len == 1 {
                                bitset_at(bsp, b as usize)
                            } else {
                                let code = enc.mbc_to_code(&str_data[x..], end.saturating_sub(x));
                                is_in_code_range(mb, code)
                            };
                            (!in_class).then_some(x + len)
                        },
                    );
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Word star opcodes - greedy \w* / \w+ optimization
            // ================================================================
            OpCode::WordStar => {
                let start = s;
                if enc_is_singlebyte(enc) {
                    while s < right_range {
                        if !is_word_char_at(enc, str_data, s, end) {
                            break;
                        }
                        s += 1;
                    }
                    if s > start {
                        stack.push(StackEntry::AltLazy {
                            pcode: p + 1,
                            pstr: s - 1,
                            pstr_start: start,
                            ascii: true,
                            peek_byte: 0,
                        });
                    }
                } else if onigenc_is_ascii_compatible_encoding(enc) {
                    let mut ascii_only = true;
                    let mut exact_heads = true;
                    while s < right_range {
                        let b = str_data[s];
                        if b < 0x80 {
                            if !is_word_ascii(b) {
                                break;
                            }
                            s += 1;
                        } else {
                            if !is_word_char_at(enc, str_data, s, end) {
                                break;
                            }
                            ascii_only = false;
                            let next = advance_char_to_end(enc, str_data, s, end);
                            exact_heads &= steps_back_to(enc, str_data, start, s, next);
                            s = next;
                        }
                    }
                    push_char_run(
                        &mut stack,
                        p + 1,
                        enc,
                        str_data,
                        start,
                        s,
                        ascii_only,
                        exact_heads,
                    );
                } else {
                    let mut exact_heads = true;
                    while s < right_range {
                        if !is_word_char_at(enc, str_data, s, end) {
                            break;
                        }
                        let next = advance_char_to_end(enc, str_data, s, end);
                        exact_heads &= steps_back_to(enc, str_data, start, s, next);
                        s = next;
                    }
                    push_char_run(
                        &mut stack,
                        p + 1,
                        enc,
                        str_data,
                        start,
                        s,
                        false,
                        exact_heads,
                    );
                }
                p += 1;
            }

            OpCode::WordAsciiStar => {
                let start = s;
                while s < right_range {
                    if !is_word_ascii(str_data[s]) {
                        break;
                    }
                    s += 1;
                }
                if s > start {
                    stack.push(StackEntry::AltLazy {
                        pcode: p + 1,
                        pstr: s - 1,
                        pstr_start: start,
                        ascii: true,
                        peek_byte: 0,
                    });
                }
                p += 1;
            }

            // ================================================================
            // PeekNext star variants - only backtrack to positions matching peek byte
            // ================================================================
            OpCode::CClassStarPeekNext => {
                if let OperationPayload::CClassStarPeekNext { ref bsp, c } = op.payload {
                    let start = s;
                    while s < right_range {
                        let ch = str_data[s];
                        if !bitset_at(bsp, ch as usize) {
                            break;
                        }
                        s += 1;
                    }
                    if s > start {
                        stack.push(StackEntry::AltLazy {
                            pcode: p + 1,
                            pstr: s - 1,
                            pstr_start: start,
                            ascii: true,
                            peek_byte: c,
                        });
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::WordAsciiStarPeekNext => {
                if let OperationPayload::WordAsciiStarPeekNext { c } = op.payload {
                    let start = s;
                    while s < right_range {
                        if !is_word_ascii(str_data[s]) {
                            break;
                        }
                        s += 1;
                    }
                    if s > start {
                        stack.push(StackEntry::AltLazy {
                            pcode: p + 1,
                            pstr: s - 1,
                            pstr_start: start,
                            ascii: true,
                            peek_byte: c,
                        });
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
                if s >= right_range || !is_word_char_at(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    s = advance_char_to_end(enc, str_data, s, end);
                    p += 1;
                }
            }

            OpCode::WordAscii => {
                if s >= right_range || !is_word_ascii(str_data[s]) {
                    goto_fail = true;
                } else {
                    s += 1;
                    p += 1;
                }
            }

            OpCode::NoWord => {
                if s >= right_range || is_word_char_at(enc, str_data, s, end) {
                    goto_fail = true;
                } else {
                    s = advance_char_to_end(enc, str_data, s, end);
                    p += 1;
                }
            }

            OpCode::NoWordAscii => {
                if s >= right_range || is_word_ascii(str_data[s]) {
                    goto_fail = true;
                } else {
                    s = advance_char_to_end(enc, str_data, s, end);
                    p += 1;
                }
            }

            // ================================================================
            // Word boundary opcodes
            // ================================================================
            OpCode::WordBoundary => {
                let mode = if let OperationPayload::WordBoundary { mode } = op.payload {
                    mode
                } else {
                    0
                };
                if !is_word_boundary_ascii_fast(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::NoWordBoundary => {
                let mode = if let OperationPayload::WordBoundary { mode } = op.payload {
                    mode
                } else {
                    0
                };
                if is_word_boundary_ascii_fast(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::WordBegin => {
                let mode = if let OperationPayload::WordBoundary { mode } = op.payload {
                    mode
                } else {
                    0
                };
                if !is_word_begin(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::WordEnd => {
                let mode = if let OperationPayload::WordBoundary { mode } = op.payload {
                    mode
                } else {
                    0
                };
                if !is_word_end(enc, str_data, s, end, mode) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            // ================================================================
            // Rust-only (ADR-008): `\X` outside look-behinds, one instruction
            // ================================================================
            OpCode::GraphemeCluster => {
                match grapheme_cluster_end(enc, str_data, s, right_range, end) {
                    Some(at) => {
                        s = at;
                        p += 1;
                    }
                    None => goto_fail = true,
                }
            }

            OpCode::TextSegmentBoundary => {
                if let OperationPayload::TextSegmentBoundary { boundary_type, not } = op.payload {
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
                if s != 0 || opton_notbol(options) || opton_not_begin_string(options) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::EndBuf => {
                if s != end || opton_noteol(options) || opton_not_end_string(options) {
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
                } else if s != end {
                    // C: `else if (! ON_STR_END(s))` -- a newline at the very
                    // end of the subject does not start another line.
                    let sprev = onigenc_get_prev_char_head(enc, str_data, 0, s);
                    if enc.is_mbc_newline(&str_data[sprev..], end) {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
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
                let is_before_end = s == end || (s + 1 == end && str_data[s] == b'\n');
                if !is_before_end || opton_noteol(options) || opton_not_end_string(options) {
                    goto_fail = true;
                } else {
                    p += 1;
                }
            }

            OpCode::CheckPosition => {
                if let OperationPayload::CheckPosition { check_type } = op.payload {
                    match check_type {
                        CheckPositionType::SearchStart => {
                            if s != msa.start || opton_not_begin_position(options) {
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
                if num_mem < 1 {
                    goto_fail = true;
                } else if let (Some(ms), Some(me)) = (
                    get_mem_start(reg, &stack, mem_start_stk, 1),
                    get_mem_end(reg, &stack, mem_end_stk, 1),
                ) {
                    let ref_len = me.saturating_sub(ms); // C: a negative n matches empty
                    // Rust-only (ADR-008): a one-byte capture needs no
                    // general slice comparison. Longer captures retain it.
                    if right_range.saturating_sub(s) < ref_len
                        || if ref_len == 1 && single_byte_backrefs_enabled() {
                            str_data[s] != str_data[ms]
                        } else {
                            str_data[s..s + ref_len] != str_data[ms..ms + ref_len]
                        }
                    {
                        goto_fail = true;
                    } else {
                        s += ref_len;
                        p += 1;
                    }
                } else if (reg.syntax.behavior & FERRONI_SYN_UNMATCHED_BACKREF_MATCHES_EMPTY) != 0 {
                    p += 1;
                } else {
                    goto_fail = true; // group not yet matched
                }
            }

            OpCode::BackRef2 => {
                if num_mem < 2 {
                    goto_fail = true;
                } else if let (Some(ms), Some(me)) = (
                    get_mem_start(reg, &stack, mem_start_stk, 2),
                    get_mem_end(reg, &stack, mem_end_stk, 2),
                ) {
                    let ref_len = me.saturating_sub(ms); // C: a negative n matches empty
                    if right_range.saturating_sub(s) < ref_len
                        || str_data[s..s + ref_len] != str_data[ms..ms + ref_len]
                    {
                        goto_fail = true;
                    } else {
                        s += ref_len;
                        p += 1;
                    }
                } else if (reg.syntax.behavior & FERRONI_SYN_UNMATCHED_BACKREF_MATCHES_EMPTY) != 0 {
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefN => {
                if let OperationPayload::BackRefN { n1 } = op.payload {
                    let n1 = n1 as usize;
                    if n1 > num_mem {
                        goto_fail = true;
                    } else if let (Some(ms), Some(me)) = (
                        get_mem_start(reg, &stack, mem_start_stk, n1),
                        get_mem_end(reg, &stack, mem_end_stk, n1),
                    ) {
                        let ref_len = me.saturating_sub(ms); // C: a negative n matches empty
                        if right_range.saturating_sub(s) < ref_len
                            || str_data[s..s + ref_len] != str_data[ms..ms + ref_len]
                        {
                            goto_fail = true;
                        } else {
                            s += ref_len;
                            p += 1;
                        }
                    } else if (reg.syntax.behavior & FERRONI_SYN_UNMATCHED_BACKREF_MATCHES_EMPTY)
                        != 0
                    {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefNIc => {
                if let OperationPayload::BackRefN { n1 } = op.payload {
                    let n1 = n1 as usize;
                    if n1 > num_mem {
                        goto_fail = true;
                    } else if let (Some(ms), Some(me)) = (
                        get_mem_start(reg, &stack, mem_start_stk, n1),
                        get_mem_end(reg, &stack, mem_end_stk, n1),
                    ) {
                        let ref_len = me.saturating_sub(ms); // C: a negative n matches empty
                        if ref_len != 0
                            && (right_range.saturating_sub(s) < ref_len
                                || !string_cmp_ic(
                                    enc,
                                    reg.case_fold_flag,
                                    str_data,
                                    ms,
                                    &mut s,
                                    ref_len,
                                ))
                        {
                            goto_fail = true;
                        } else {
                            p += 1;
                        }
                    } else if (reg.syntax.behavior & FERRONI_SYN_UNMATCHED_BACKREF_MATCHES_EMPTY)
                        != 0
                    {
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::BackRefMulti => {
                if let OperationPayload::BackRefGeneral { num, ns, .. } = &op.payload {
                    let tlen = *num as usize;
                    let mut matched = false;
                    let mut participated = false;
                    for mem in ns.iter().take(tlen).map(|mem| *mem as usize) {
                        if mem > num_mem {
                            continue;
                        }
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, mem_start_stk, mem),
                            get_mem_end(reg, &stack, mem_end_stk, mem),
                        ) {
                            participated = true;
                            let ref_len = me.saturating_sub(ms); // C: a negative n matches empty
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len {
                                    continue;
                                }
                                if str_data[s..s + ref_len] != str_data[ms..ms + ref_len] {
                                    continue;
                                }
                                s += ref_len;
                            }
                            matched = true;
                            break;
                        }
                    }
                    if !participated
                        && (reg.syntax.behavior & FERRONI_SYN_UNMATCHED_BACKREF_MATCHES_EMPTY) != 0
                    {
                        matched = true;
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
                if let OperationPayload::BackRefGeneral { num, ns, .. } = &op.payload {
                    let tlen = *num as usize;
                    let mut matched = false;
                    let mut participated = false;
                    for mem in ns.iter().take(tlen).map(|mem| *mem as usize) {
                        if mem > num_mem {
                            continue;
                        }
                        if let (Some(ms), Some(me)) = (
                            get_mem_start(reg, &stack, mem_start_stk, mem),
                            get_mem_end(reg, &stack, mem_end_stk, mem),
                        ) {
                            participated = true;
                            let ref_len = me.saturating_sub(ms); // C: a negative n matches empty
                            if ref_len != 0 {
                                if right_range.saturating_sub(s) < ref_len {
                                    continue;
                                }
                                let mut swork = s;
                                if !string_cmp_ic(
                                    enc,
                                    reg.case_fold_flag,
                                    str_data,
                                    ms,
                                    &mut swork,
                                    ref_len,
                                ) {
                                    continue;
                                }
                                s = swork;
                            }
                            matched = true;
                            break;
                        }
                    }
                    if !participated
                        && (reg.syntax.behavior & FERRONI_SYN_UNMATCHED_BACKREF_MATCHES_EMPTY) != 0
                    {
                        matched = true;
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
                if let OperationPayload::BackRefGeneral { num, ns, .. } = &op.payload {
                    let tlen = *num as usize;
                    let mut found = false;
                    for mem in ns.iter().take(tlen).map(|mem| *mem as usize) {
                        if mem > num_mem {
                            continue;
                        }
                        if get_mem_start(reg, &stack, mem_start_stk, mem).is_some()
                            && get_mem_end(reg, &stack, mem_end_stk, mem).is_some()
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
                if let OperationPayload::BackRefGeneral {
                    num,
                    ns,
                    nest_level,
                } = &op.payload
                {
                    let ignore_case = op.opcode == OpCode::BackRefWithLevelIc;
                    if backref_match_at_nested_level(
                        reg,
                        &stack,
                        ignore_case,
                        reg.case_fold_flag,
                        *nest_level,
                        *num,
                        ns,
                        &mut s,
                        str_data,
                        end,
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
                if let OperationPayload::BackRefGeneral {
                    num,
                    ns,
                    nest_level,
                } = &op.payload
                {
                    let found = if backref_check_at_nested_level(&stack, *nest_level, *num, ns) {
                        true
                    } else if *nest_level == 0 {
                        // At level 0, also check mem arrays directly (group may use
                        // non-push MemStart with no stack entry)
                        let tlen = *num as usize;
                        let mut f = false;
                        for mem in ns.iter().take(tlen).map(|mem| *mem as usize) {
                            if mem > num_mem {
                                continue;
                            }
                            if get_mem_start(reg, &stack, mem_start_stk, mem).is_some()
                                && get_mem_end(reg, &stack, mem_end_stk, mem).is_some()
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
                if let OperationPayload::MemoryStart { num } = op.payload {
                    // C sets only the start (regexec.c OP_MEM_START); the end
                    // of an earlier pass stays in place until MEM_END, so a
                    // capture can read as start > end if the pass fails.
                    if TRACK_CAPTURES {
                        mem_start_stk[num as usize] = MemPtr::pos(s);
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemStartPush => {
                if let OperationPayload::MemoryStart { num } = op.payload {
                    // The pushed entry only restores this capture on
                    // backtracking. Opcodes that read captures while matching
                    // force TRACK_CAPTURES (see `needs_capture_tracking`), so
                    // an untracked run can drop the bookkeeping.
                    if TRACK_CAPTURES {
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
                        mem_start_stk[num] = MemPtr::stack_idx(si);
                        mem_end_stk[num] = MemPtr::invalid();
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEnd => {
                if let OperationPayload::MemoryEnd { num } = op.payload {
                    if TRACK_CAPTURES {
                        let num = num as usize;
                        mem_end_stk[num] = MemPtr::pos(s);
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndPush => {
                if let OperationPayload::MemoryEnd { num } = op.payload {
                    // See OpCode::MemStartPush for why untracked runs skip this.
                    if TRACK_CAPTURES {
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
                        mem_end_stk[num] = MemPtr::stack_idx(si);
                    }
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndPushRec => {
                // Recursive capture end (push variant): find matching MEM_START,
                // push MEM_END, update start/end tracking
                if let OperationPayload::MemoryEnd { num } = op.payload {
                    let mem = num as usize;
                    let (start_ptr, _) =
                        stack_get_mem_start_for_rec(&stack, mem, reg.push_mem_start);
                    let si = stack.len();
                    stack.push(StackEntry::MemEnd {
                        zid: mem,
                        pstr: s,
                        prev_start: mem_start_stk[mem],
                        prev_end: mem_end_stk[mem],
                    });
                    mem_start_stk[mem] = start_ptr;
                    mem_end_stk[mem] = MemPtr::stack_idx(si);
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::MemEndRec => {
                // Recursive capture end (non-push variant): find matching MEM_START,
                // update start/end, push MemEndMark for level tracking
                if let OperationPayload::MemoryEnd { num } = op.payload {
                    let mem = num as usize;
                    mem_end_stk[mem] = MemPtr::pos(s);
                    let (start_ptr, _) =
                        stack_get_mem_start_for_rec(&stack, mem, reg.push_mem_start);
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
                if let OperationPayload::Jump { addr } = op.payload {
                    p = (p as i32 + addr) as usize;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH / OP_PUSH_SUPER - push choice point (alternation)
            // ================================================================
            OpCode::Push | OpCode::PushSuper => {
                if let OperationPayload::Push { addr } = op.payload {
                    let alt_target = (p as i32 + addr) as usize;
                    let is_super = op.opcode == OpCode::PushSuper;
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

            // Rust-only (ADR-008): the original lazy push, with a seek over
            // failed literal attempts across the following capture ends and
            // look-behinds.
            OpCode::PushLazyAnyChar => {
                if let OperationPayload::PushLazyAnyChar { addr, c } = op.payload {
                    // Rust-only (ADR-008): not under a match stack limit, where
                    // the instruction pushes the entries the limit counts.
                    if match_stack_limit == 0 && stack.len() + 1 < stack_cap {
                        if let Some(at) = lazy_captured_any_char_stop(
                            reg,
                            p,
                            str_data,
                            s,
                            right_range,
                            c,
                            &mut retry_in_match_counter,
                            retry_limit_in_match,
                            msa,
                        ) {
                            s = at;
                        }
                    }
                    stack.push(StackEntry::Alt {
                        pcode: (p as i32 + addr) as usize,
                        pstr: s,
                        zid: -1,
                        is_super: false,
                    });
                    p += 1;
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

            // Rust-only (ADR-008): the star just before this instruction
            // resumes here on backtracking, so the entries on top that resume
            // here are exactly its choices. Dropping them makes it possessive
            // where `possessify` proved that no shorter run can match; the
            // backtracks they would take are not counted (ADR-017).
            OpCode::PopStarChoices => {
                pop_star_choices(&mut stack, p);
                p += 1;
            }

            // ================================================================
            // OP_POP_TO_MARK - pop until Mark with matching id
            // ================================================================
            OpCode::PopToMark => {
                if let OperationPayload::PopToMark { id } = op.payload {
                    stack_pop_to_mark(
                        &mut stack,
                        id as usize,
                        mem_start_stk,
                        mem_end_stk,
                        &mut subexp_call_nest_counter,
                    );
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH_OR_JUMP_EXACT1 - optimized push for exact char
            // ================================================================
            OpCode::PushOrJumpExact1 => {
                if let OperationPayload::PushOrJumpExact1 {
                    addr,
                    c,
                    skipped_retries,
                    skip_gate,
                } = op.payload
                {
                    // Rust-only (ADR-008): the lazy `.*?c` loop (`AnyChar`
                    // before this instruction, which jumps back to it) runs
                    // to its next stop in one step.
                    if addr == -1
                        && match_stack_limit == 0
                        && matches!(reg.ops[p - 1].opcode, OpCode::AnyChar | OpCode::AnyCharMl)
                    {
                        match lazy_any_char_step(
                            reg,
                            p,
                            str_data,
                            s,
                            right_range,
                            &mut retry_in_match_counter,
                            retry_limit_in_match,
                            msa,
                        ) {
                            LazyStep::Stay => {}
                            LazyStep::At(at) => s = at,
                            LazyStep::Fail => goto_fail = true,
                            LazyStep::Limit(err) => stop_at_limit!(err),
                        }
                    }
                    if goto_fail {
                        // The loop's `AnyChar` fails before any `c`.
                    } else if let Some(gate) = skip_gate_of(reg, skip_gate).filter(|gate| {
                        match_stack_limit == 0
                            && skip_gate_taken(
                                gate,
                                str_data,
                                s,
                                right_range,
                                retry_in_match_counter,
                                retry_limit_in_match,
                                time_limit_ms,
                            )
                    }) {
                        // Rust-only (ADR-008): the rest of the chain misses
                        // as well; see `skip_gate_taken`.
                        p = (p as i32 + gate.addr) as usize;
                        retry_in_match_counter += u64::from(gate.skipped_retries);
                    } else if (s < right_range && str_data[s] == c)
                        // Rust-only (ADR-008): under a match stack limit the
                        // guard pushes like its unguarded instruction, as a
                        // skipped push would leave out an entry the limit counts.
                        || match_stack_limit != 0
                        || !guard_may_jump(skipped_retries, reg, msa, exact_guard_retries)
                    {
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
                        // A guarded `Push` counts the backtracks the push
                        // would take (see `guard_backtrack_pushes`).
                        if skipped_retries != 0 {
                            if let Err(err) = count_retry(
                                guard_retries(skipped_retries),
                                &mut retry_in_match_counter,
                                retry_limit_in_match,
                                time_limit_ms,
                                msa,
                            ) {
                                stop_at_limit!(err);
                            }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Rust-only (ADR-008): PUSH_OR_JUMP_EXACT1 with a byte set
            // ================================================================
            OpCode::PushOrJumpByteSet => {
                if let OperationPayload::PushOrJumpByteSet {
                    addr,
                    ref bsp,
                    skipped_retries,
                    skip_gate,
                } = op.payload
                {
                    if let Some(gate) = skip_gate_of(reg, skip_gate).filter(|gate| {
                        match_stack_limit == 0
                            && skip_gate_taken(
                                gate,
                                str_data,
                                s,
                                right_range,
                                retry_in_match_counter,
                                retry_limit_in_match,
                                time_limit_ms,
                            )
                    }) {
                        // Rust-only (ADR-008): the rest of the chain misses
                        // as well; see `skip_gate_taken`.
                        p = (p as i32 + gate.addr) as usize;
                        retry_in_match_counter += u64::from(gate.skipped_retries);
                    } else if (s < right_range && bitset_at(bsp, str_data[s] as usize))
                        // Rust-only (ADR-008): see the guard above.
                        || match_stack_limit != 0
                        || !guard_may_jump(skipped_retries, reg, msa, exact_guard_retries)
                    {
                        stack.push(StackEntry::Alt {
                            pcode: (p as i32 + addr) as usize,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
                        p += 1;
                    } else {
                        p = (p as i32 + addr) as usize;
                        // Counts the backtracks the push would take (see
                        // `guard_backtrack_pushes`).
                        if let Err(err) = count_retry(
                            guard_retries(skipped_retries),
                            &mut retry_in_match_counter,
                            retry_limit_in_match,
                            time_limit_ms,
                            msa,
                        ) {
                            stop_at_limit!(err);
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_PUSH_IF_PEEK_NEXT - push only if next char matches
            // ================================================================
            OpCode::PushIfPeekNext => {
                if let OperationPayload::PushIfPeekNext { addr, c } = op.payload {
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
                if let OperationPayload::Repeat { id, addr } = op.payload {
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
                if let OperationPayload::RepeatInc { id } = op.payload {
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
                        stack.push(StackEntry::Alt {
                            pcode: p,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
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
                if let OperationPayload::RepeatInc { id } = op.payload {
                    let id = id as usize;
                    let count = stack_get_repeat_count(&stack, id) + 1;
                    let lower = reg.repeat_range[id].lower;
                    let upper = reg.repeat_range[id].upper;
                    let body_start = reg.repeat_range[id].u_offset as usize;

                    // C order for non-greedy: push count FIRST, then branch
                    // Count pushed BEFORE Alt — survives backtrack (correct for lazy)
                    stack.push(StackEntry::RepeatInc { zid: id, count });

                    if upper != INFINITE_REPEAT && count == upper {
                        p += 1;
                    } else if count >= lower {
                        stack.push(StackEntry::Alt {
                            pcode: body_start,
                            pstr: s,
                            zid: -1,
                            is_super: false,
                        });
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
                if let OperationPayload::EmptyCheckStart { mem } = op.payload {
                    let mem = mem as usize;
                    stack.push(StackEntry::EmptyCheckStart { zid: mem, pstr: s });
                    p += 1;
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EmptyCheckEnd => {
                if let OperationPayload::EmptyCheckEnd { mem, .. } = op.payload {
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

            OpCode::EmptyCheckEndMemst => {
                if let OperationPayload::EmptyCheckEnd {
                    mem,
                    empty_status_mem,
                } = op.payload
                {
                    let is_empty = stack_empty_check_mem(&stack, mem as usize, s, empty_status_mem);
                    p += 1;
                    if is_empty {
                        // Truly empty → skip next op (JUMP back)
                        p += 1;
                    }
                } else {
                    goto_fail = true;
                }
            }

            OpCode::EmptyCheckEndMemstPush => {
                if let OperationPayload::EmptyCheckEnd {
                    mem,
                    empty_status_mem,
                } = op.payload
                {
                    let mem = mem as usize;
                    let is_empty = stack_empty_check_mem_rec(&stack, mem, s, empty_status_mem);
                    p += 1;
                    if is_empty {
                        // Truly empty → skip next op (JUMP back)
                        p += 1;
                    } else {
                        // C: STACK_PUSH_EMPTY_CHECK_END -- marks the iteration
                        // as complete for scans from recursive runs.
                        stack.push(StackEntry::EmptyCheckEnd { zid: mem });
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // OP_MOVE - move string position
            // ================================================================
            OpCode::Move => {
                if let OperationPayload::Move { n } = op.payload {
                    if n < 0 {
                        // Step back n characters (encoding-aware)
                        match onigenc_step_back(enc, 0, s, str_data, (-n) as usize) {
                            Some(new_s) => {
                                s = new_s;
                                p += 1;
                            }
                            None => {
                                goto_fail = true;
                            }
                        }
                    } else {
                        // Step forward n characters
                        match onigenc_step(enc, s, end, str_data, n as usize) {
                            Some(new_s) => {
                                s = new_s;
                                p += 1;
                            }
                            None => {
                                goto_fail = true;
                            }
                        }
                    }
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Rust-only (ADR-008): a look-behind with a one-instruction body
            // ================================================================
            OpCode::LookBehindOp => {
                if let OperationPayload::LookBehindOp {
                    char_len,
                    not,
                    ascii_compatible,
                } = op.payload
                {
                    // Rust-only (ADR-008): the upstream sequence pushes at its
                    // MARK and its PUSH (negative only), so it holds one entry
                    // more (two for a negative look-behind) and fails when
                    // that exceeds the cap.
                    let pushed = if not { 2 } else { 1 };
                    if stack.len() + pushed > stack_cap {
                        best_len = ONIGERR_MATCH_STACK_LIMIT_OVER;
                        break;
                    }
                    // A one-character step over the class or type of a
                    // one-byte look-behind, in an ASCII-compatible encoding.
                    let short_body = char_len == 1
                        && ascii_compatible
                        && matches!(
                            reg.ops[p + 1].opcode,
                            OpCode::Word
                                | OpCode::NoWord
                                | OpCode::WordAscii
                                | OpCode::NoWordAscii
                                | OpCode::CClass
                                | OpCode::CClassNot
                                | OpCode::CClassRun
                                | OpCode::CClassMix
                                | OpCode::CClassMixNot
                        );
                    // Rust-only (ADR-008): a negative one-character look-behind
                    // over `\w` or a one-byte string (`Str1`), in UTF-8, is
                    // decided inline, with `look_behind_ascii_end`'s decision:
                    // the byte before `s` must be ASCII, which is its own
                    // character head in UTF-8. `None` for `\w`, else the byte
                    // of the string. Other cases take the general path.
                    let inline_body = if not
                        && char_len == 1
                        && std::ptr::addr_eq(enc, &crate::encodings::utf8::ONIG_ENCODING_UTF8)
                    {
                        match (reg.ops[p + 1].opcode, &reg.ops[p + 1].payload) {
                            (OpCode::Word, _) => Some(None),
                            (OpCode::Str1, OperationPayload::Exact { s: lit }) => {
                                Some(Some(lit[0]))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let decided = if let Some(body_byte) = inline_body {
                        if s == 0 {
                            Some(None)
                        } else {
                            let before = (s - 1 < right_range && s - 1 < end)
                                .then(|| str_data.get(s - 1).copied())
                                .flatten();
                            match before.filter(|&b| b < 0x80) {
                                Some(b) => {
                                    let body_matches = match body_byte {
                                        Some(c) => c == b,
                                        None => is_word_ascii(b),
                                    };
                                    Some(body_matches.then_some(s))
                                }
                                None => None,
                            }
                        }
                    } else if short_body {
                        look_behind_ascii_end(&reg.ops[p + 1], enc, str_data, s, right_range, end)
                    } else {
                        None
                    };
                    let body_end = match decided {
                        Some(decided) => decided,
                        None => look_behind_body_end(
                            &reg.ops[p + 1],
                            char_len,
                            enc,
                            str_data,
                            s,
                            right_range,
                            end,
                            ascii_compatible,
                        ),
                    };
                    match body_end {
                        // C: CUT_TO_MARK without restoring the position, so
                        // the match goes on where the body ended. Over valid
                        // UTF-8 that is `s` itself.
                        Some(body_end) if !not => {
                            s = body_end;
                            p += 2;
                        }
                        None if not => {
                            // The body's failure backtracks once in the
                            // upstream sequence; the count is the same.
                            // count_retry(1) inline when no limit is near
                            // and no time limit is set.
                            if time_limit_ms == 0
                                && (retry_limit_in_match == 0
                                    || retry_in_match_counter + 1 < retry_limit_in_match)
                            {
                                retry_in_match_counter += 1;
                            } else if let Err(err) = count_retry(
                                1,
                                &mut retry_in_match_counter,
                                retry_limit_in_match,
                                time_limit_ms,
                                msa,
                            ) {
                                stop_at_limit!(err);
                            }
                            p += 2;
                        }
                        _ => goto_fail = true,
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
                } = op.payload
                {
                    let initial = initial as usize;
                    // Step back 'initial' characters (encoding-aware)
                    if initial != 0 {
                        match step_back_look_behind(enc, 0, s, str_data, initial) {
                            Some(new_s) => {
                                s = new_s;
                            }
                            None => {
                                goto_fail = true;
                            }
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
                            p = (p as i32 + addr) as usize;
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
                    Some(new_s) => {
                        s = new_s;
                    }
                    None => {
                        goto_fail = true;
                    }
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
                if let OperationPayload::Mark { id, save_pos } = op.payload {
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
                if let OperationPayload::CutToMark { id, restore_pos } = op.payload {
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
                if let OperationPayload::SaveVal { save_type, id } = op.payload {
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
                    var_type,
                    id,
                    clear,
                } = op.payload
                {
                    let id = id as usize;
                    match var_type {
                        UpdateVarType::KeepFromStackLast => {
                            if let Some(v) = stack_get_save_val_type_last(&stack, SaveType::Keep) {
                                keep = v;
                            }
                        }
                        UpdateVarType::SFromStack => {
                            if let Some(v) = stack_get_save_val_last(&stack, SaveType::S, id) {
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
                            if let Some(v) = stack_get_save_val_last(&stack, SaveType::S, id) {
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
                if let OperationPayload::Call { addr } = op.payload {
                    if subexp_call_nest_counter
                        == onig_get_subexp_call_max_nest_level() as i64 as u64
                    {
                        goto_fail = true;
                    } else {
                        subexp_call_nest_counter += 1;
                        let subexp_call_limit_in_search = onig_get_subexp_call_limit_in_search();
                        if subexp_call_limit_in_search != 0 {
                            msa.subexp_call_in_search_counter += 1;
                            if msa.subexp_call_in_search_counter > subexp_call_limit_in_search {
                                best_len = ONIGERR_SUBEXP_CALL_LIMIT_IN_SEARCH_OVER;
                                break;
                            }
                        }
                        let addr = addr as usize;
                        stack.push(StackEntry::CallFrame { ret_addr: p + 1 });
                        p = addr;
                    }
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
                    subexp_call_nest_counter = subexp_call_nest_counter.wrapping_sub(1);
                    p = ra;
                } else {
                    goto_fail = true;
                }
            }

            // ================================================================
            // Callout opcodes
            // ================================================================
            OpCode::CalloutContents => {
                let num = match &op.payload {
                    OperationPayload::CalloutContents { num } => *num,
                    _ => 0,
                };
                let callout_in = reg
                    .extp
                    .as_ref()
                    .and_then(|ext| ext.callout_list.get((num - 1) as usize))
                    .map_or(0, |entry| entry.callout_in);
                let mut call_result = ONIG_CALLOUT_SUCCESS;
                if (callout_in & CALLOUT_IN_PROGRESS) != 0
                    && let Some(callback) = msa.progress_callout
                {
                    call_result = invoke_callout(
                        callback,
                        OnigCalloutIn::Progress,
                        ONIG_NON_NAME_ID,
                        num,
                        reg,
                        str_data,
                        end,
                        sstart,
                        right_range,
                        s,
                        retry_in_match_counter,
                        msa,
                        &mut callout_data,
                    );
                }
                match call_result {
                    ONIG_CALLOUT_FAIL => goto_fail = true,
                    ONIG_CALLOUT_SUCCESS => {
                        if (callout_in & CALLOUT_IN_RETRACTION) != 0
                            && let Some(callback) = msa.retraction_callout
                        {
                            stack.push(StackEntry::Callout {
                                num,
                                builtin_id: -1,
                                name_id: ONIG_NON_NAME_ID,
                                callback: Some(callback),
                            });
                        }
                        p += 1;
                    }
                    result => {
                        best_len = if result > 0 {
                            ONIGERR_INVALID_ARGUMENT
                        } else {
                            result
                        };
                        break;
                    }
                }
            }
            OpCode::AltLiterals => {
                if let OperationPayload::AltLiterals { trie_idx } = op.payload {
                    let trie = &reg.literal_tries[trie_idx as usize];
                    if let Some(match_len) =
                        match_literal_trie(&mut stack, trie, str_data, s, right_range, p + 1)
                    {
                        s += match_len;
                        p += 1;
                    } else {
                        goto_fail = true;
                    }
                } else {
                    goto_fail = true;
                }
            }
            OpCode::CalloutName => {
                let (num, id) = match &op.payload {
                    OperationPayload::CalloutName { num, id } => (*num, *id),
                    _ => (0, 0),
                };
                let callout = reg
                    .extp
                    .as_ref()
                    .and_then(|ext| ext.callout_list.get((num - 1) as usize))
                    .map(|entry| {
                        (
                            entry.callout_in,
                            entry.builtin_id,
                            entry.name_id,
                            entry.start_func,
                        )
                    });
                let Some((callout_in, builtin_id, name_id, start_func)) = callout else {
                    p += 1;
                    continue;
                };
                let mut call_result = ONIG_CALLOUT_SUCCESS;
                if (callout_in & CALLOUT_IN_PROGRESS) != 0 {
                    if builtin_id >= 0 {
                        call_result =
                            run_builtin_callout(reg, num, id, false, &mut callout_data, s, msa);
                    } else if let Some(callback) = start_func {
                        call_result = invoke_callout(
                            callback,
                            OnigCalloutIn::Progress,
                            name_id,
                            num,
                            reg,
                            str_data,
                            end,
                            sstart,
                            right_range,
                            s,
                            retry_in_match_counter,
                            msa,
                            &mut callout_data,
                        );
                    }
                }
                match call_result {
                    ONIG_CALLOUT_FAIL => goto_fail = true,
                    ONIG_CALLOUT_SUCCESS => {
                        let callback = if builtin_id >= 0 { None } else { start_func };
                        if (callout_in & CALLOUT_IN_RETRACTION) != 0
                            && (builtin_id >= 0 || callback.is_some())
                        {
                            stack.push(StackEntry::Callout {
                                num,
                                builtin_id,
                                name_id,
                                callback,
                            });
                        }
                        p += 1;
                    }
                    result => {
                        best_len = if result > 0 {
                            ONIGERR_INVALID_ARGUMENT
                        } else {
                            result
                        };
                        break;
                    }
                }
            }
        }

        // Handle failure (backtracking)
        if goto_fail {
            if let Err(err) = count_retry(
                1,
                &mut retry_in_match_counter,
                retry_limit_in_match,
                time_limit_ms,
                msa,
            ) {
                stop_at_limit!(err);
            }

            // Most backtracks find a choice point on top of the stack, where
            // STACK_POP has nothing to skip or restore: take it here and
            // leave the other entries to `stack_pop`. The bottom sentinel
            // stays in place, as STACK_POP would return FINISH_PCODE. An
            // ASCII star run without a peek byte gives up its last character
            // in place, like the pop and push in `stack_pop`.
            let top = match stack.last_mut() {
                Some(StackEntry::Alt {
                    pcode, pstr, zid, ..
                }) => Some((*pcode, *pstr, *zid, true)),
                Some(StackEntry::AltLazy {
                    pcode,
                    pstr,
                    pstr_start,
                    ascii: true,
                    peek_byte: 0,
                }) => {
                    let at = *pstr;
                    let exhausted = at <= *pstr_start;
                    if !exhausted {
                        *pstr = at - 1;
                    }
                    Some((*pcode, at, -1, exhausted))
                }
                _ => None,
            };
            if let Some((pcode, pstr, alt_zid, pop)) = top {
                if pcode == FINISH_PCODE {
                    attempt_failed!();
                }
                if pop {
                    stack.pop();
                }
                p = pcode;
                s = pstr;
                last_alt_zid = alt_zid;
                continue;
            }
            let pop_result = stack_pop(
                &mut stack,
                pop_level,
                mem_start_stk,
                mem_end_stk,
                reg,
                &mut callout_data,
                str_data,
                end,
                sstart,
                right_range,
                s,
                retry_in_match_counter,
                msa,
                &mut subexp_call_nest_counter,
            );
            match pop_result {
                Ok(Some((pcode, pstr, alt_zid))) => {
                    if pcode == FINISH_PCODE {
                        // Hit bottom sentinel - no more alternatives
                        attempt_failed!();
                    }
                    p = pcode;
                    s = pstr;
                    last_alt_zid = alt_zid;
                }
                Ok(None) => {
                    // Stack empty - match failed
                    attempt_failed!();
                }
                Err(err) => {
                    best_len = err;
                    break;
                }
            }
        }
    }

    // Accumulate retry counter into search counter
    msa.retry_limit_in_search_counter += retry_in_match_counter;

    // Return reusable buffers to MatchArg for next call
    msa.stack = stack;
    if TRACK_CAPTURES && num_mem >= MEM_STK_INLINE {
        msa.mem_start_stk = mem_start_vec;
        msa.mem_end_stk = mem_end_vec;
    }

    best_len
}

/// Capture arrays of up to this many entries (group 0 included) live on the
/// stack of `match_at_impl`.
const MEM_STK_INLINE: usize = 8;

/// Stack entries a thread-cached `MatchArg` keeps allocated between searches.
const MAX_CACHED_STACK_ENTRIES: usize = 1 << 16;

/// Drop a match stack that grew past `MAX_CACHED_STACK_ENTRIES` before `msa`
/// goes back into a thread-local cache. C allocates the match stack in each
/// `match_at` and frees it on return; the port reuses it across start
/// positions and searches, but once a search is over it does not keep a stack
/// that ran into a limit (up to gigabytes under the default retry limit) for
/// the rest of the thread.
#[inline]
pub(crate) fn release_oversized_stack(msa: &mut MatchArg) {
    if msa.stack.capacity() > MAX_CACHED_STACK_ENTRIES {
        msa.stack = Vec::with_capacity(INIT_MATCH_STACK_SIZE);
    }
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
    // Box the cached state so each warm call moves only a pointer. Taking
    // ownership still leaves the cache available to re-entrant calls.
    thread_local! {
        static CACHED_MSA: RefCell<Option<Box<MatchArg>>> = const { RefCell::new(None) };
    }

    let mut msa = match CACHED_MSA.with(|c| c.borrow_mut().take()) {
        Some(mut cached) => {
            cached.reset_with_limits(reg, option, region, at, None);
            cached
        }
        None => Box::new(MatchArg::new(reg, option, region, at)),
    };

    if opton_check_validity_of_string(msa.options) && !reg.enc.is_valid_mbc_string(&str_data[..end])
    {
        let result = (ONIGERR_INVALID_WIDE_CHAR_VALUE, msa.region.take());
        CACHED_MSA.with(|c| *c.borrow_mut() = Some(msa));
        return result;
    }

    if let Some(ref mut r) = msa.region {
        r.resize_clear(reg.num_mem + 1);
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

    let region = msa.region.take();
    release_oversized_stack(&mut msa);
    CACHED_MSA.with(|c| *c.borrow_mut() = Some(msa));
    (result, region)
}

/// Fast match path for regset: reuses an existing MatchArg, skips string
/// validation (already done by caller), and skips region resize (caller manages).
pub(crate) fn onig_match_with_msa_start(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    at: usize,
    start: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> i32 {
    msa.reset_for_match(reg, option, start);

    match_at(reg, str_data, end, end, at, msa)
}

/// Fast match path where search start (`\\G` anchor position) equals `at`.
///
/// Thin wrapper around `onig_match_with_msa_start` used by regset search.
/// Covered indirectly via regset tests; excluded to avoid instrumentation overhead.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn onig_match_with_msa(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    at: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> i32 {
    onig_match_with_msa_start(reg, str_data, end, at, at, option, msa)
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

    if opton_check_validity_of_string(msa.options) && !reg.enc.is_valid_mbc_string(&str_data[..end])
    {
        return (ONIGERR_INVALID_WIDE_CHAR_VALUE, msa.region.take());
    }

    if let Some(ref mut r) = msa.region {
        r.resize_clear(reg.num_mem + 1);
    }

    let result = match_at(reg, str_data, end, end, at, &mut msa);

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

// ============================================================================
// onig_search - search for a match anywhere in the string
// ============================================================================

// Search for the regex pattern in the string, trying each position from
// `start` to `range`. Returns the match position on success, ONIG_MISMATCH
// (-1) on failure, given a compiled regex `reg`. The entry points themselves
// are `onig_search` and `onig_search_with_param` further down this file.
// ============================================================================
// Search optimization functions — mirrors C's regexec.c lines 5168-5645
// ============================================================================

/// Search helper.
///
///   Naive string search. Mirrors C's slow_search.
///   Uses SIMD-accelerated memchr::memmem for the actual byte search.
#[cfg_attr(coverage_nightly, coverage(off))]
fn slow_search(
    _enc: OnigEncoding,
    target: &[u8],
    text: &[u8],
    text_start: usize,
    text_end: usize,
    text_range: usize,
) -> Option<usize> {
    if target.is_empty() {
        return Some(text_start);
    }
    let tlen = target.len();
    // Search can find matches starting up to limit, needing tlen bytes available.
    let mut search_end = text_end.min(text_range + tlen.saturating_sub(1));
    if search_end > text_end {
        search_end = text_end;
    }
    if text_start + tlen > search_end {
        return None;
    }

    let haystack = &text[text_start..search_end];
    if tlen == 1 {
        memchr::memchr(target[0], haystack).map(|i| text_start + i)
    } else {
        memchr::memmem::find(haystack, target).map(|i| text_start + i)
    }
}

/// Sunday quick search (BMH variant). Mirrors C's sunday_quick_search.
/// Uses SIMD-accelerated memchr::memmem for the actual byte search.
fn sunday_quick_search(
    reg: &RegexType,
    target: &[u8],
    text: &[u8],
    text_start: usize,
    text_end: usize,
    text_range: usize,
) -> Option<usize> {
    let tlen = target.len();
    if tlen == 0 {
        return Some(text_start);
    }

    // Compute the search end: we can find matches starting up to text_range,
    // but need tlen bytes available, so search within text[text_start..search_end].
    let search_end = if tlen > text_end.saturating_sub(text_range) {
        if tlen > text_end.saturating_sub(text_start) {
            return None;
        }
        text_end
    } else {
        text_range + tlen
    };
    // C: `while (s < end)`; a start at or past the range finds nothing.
    if text_start >= search_end {
        return None;
    }

    let haystack = &text[text_start..search_end];
    if tlen == 1 {
        memchr::memchr(target[0], haystack).map(|i| text_start + i)
    } else if let Some(finder) = &reg.exact_finder {
        finder.find(haystack).map(|i| text_start + i)
    } else {
        memchr::memmem::find(haystack, target).map(|i| text_start + i)
    }
}

/// Sunday quick search with step forward for multi-byte safe operation.
/// Mirrors C's sunday_quick_search_step_forward.
#[cfg_attr(coverage_nightly, coverage(off))]
fn sunday_quick_search_step_forward(
    reg: &RegexType,
    target: &[u8],
    text: &[u8],
    text_start: usize,
    text_end: usize,
    text_range: usize,
) -> Option<usize> {
    let enc = reg.enc;
    let tlen = target.len();
    if tlen == 0 {
        return Some(text_start);
    }
    let tail_idx = tlen - 1;
    let mut end = text_range;
    if tail_idx > text_end.saturating_sub(end) {
        end = text_end.saturating_sub(tail_idx);
    }

    let map_offset = reg.map_offset as usize;
    let mut s = text_start;
    while s < end {
        let se = s + tail_idx;
        let mut p = se;
        let mut t = tail_idx;
        loop {
            if text[p] != target[t] {
                break;
            }
            if t == 0 {
                return Some(s);
            }
            p -= 1;
            t -= 1;
        }
        if se + map_offset >= text_end {
            break;
        }
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
/// Uses SIMD-accelerated memchr when the map has 1-3 distinct ASCII bytes
/// and the text it skips is valid UTF-8.
fn map_search(
    enc: OnigEncoding,
    reg: &RegexType,
    text: &[u8],
    text_start: usize,
    text_range: usize,
) -> Option<usize> {
    // C: `while (s < text_range)`; a start at or past the range finds
    // nothing. An optimizer retry can leave the start there after stepping
    // over a multibyte character.
    if text_start >= text_range {
        return None;
    }
    let haystack = &text[text_start..text_range];
    let found = match reg.map_byte_count {
        1 => memchr::memchr(reg.map_bytes[0], haystack),
        2 => memchr::memchr2(reg.map_bytes[0], reg.map_bytes[1], haystack),
        3 => memchr::memchr3(
            reg.map_bytes[0],
            reg.map_bytes[1],
            reg.map_bytes[2],
            haystack,
        ),
        _ => return map_search_chars(enc, reg, text, text_start, text_range),
    };
    // The byte search stops where C's character loop stops only if the bytes
    // it skips are valid UTF-8 in a multibyte encoding: C steps over a map
    // byte that a malformed lead byte's length covers.
    match found {
        Some(at) if enc.max_enc_len() > 1 && std::str::from_utf8(&haystack[..at]).is_err() => {
            map_search_chars(enc, reg, text, text_start, text_range)
        }
        found => found.map(|at| text_start + at),
    }
}

/// `map_search`'s character loop, as C's map_search steps.
#[inline(always)]
fn map_search_chars(
    enc: OnigEncoding,
    reg: &RegexType,
    text: &[u8],
    text_start: usize,
    text_range: usize,
) -> Option<usize> {
    let map = &reg.map;
    let mut s = text_start;
    // Rust-only (ADR-008): after 16 bytes without a candidate the
    // search goes on eight bytes at a time.
    let chunks_from = if reg.map_ascii_ranges.usable() {
        text_start.saturating_add(16)
    } else {
        usize::MAX
    };
    while s < text_range {
        if map[text[s] as usize] != 0 {
            return Some(s);
        }
        s += enclen(enc, text, s);
        if s >= chunks_from && onigenc_is_ascii_compatible_encoding(enc) {
            return map_search_ascii_chunks(enc, reg, text, s, text_range);
        }
    }
    None
}

/// Rust-only (ADR-008): `map_search`'s loop for an ASCII-compatible
/// encoding, eight bytes at a time while they are ASCII. Every ASCII byte is
/// a character of its own, so the character loop would visit each of them;
/// the first one inside the map's ASCII ranges is its answer. A chunk with a
/// non-ASCII byte is walked character by character, as before.
#[inline(never)]
fn map_search_ascii_chunks(
    enc: OnigEncoding,
    reg: &RegexType,
    text: &[u8],
    text_start: usize,
    text_range: usize,
) -> Option<usize> {
    const HIGH: u64 = 0x8080_8080_8080_8080;
    let map = &reg.map;
    let ranges = &reg.map_ascii_ranges;
    let mut s = text_start;
    while s < text_range {
        if let Some(chunk) = text[s..text_range].first_chunk::<8>() {
            let chunk = u64::from_le_bytes(*chunk);
            if chunk & HIGH == 0 {
                match ranges.first_member(chunk) {
                    Some(at) => return Some(s + at),
                    None => {
                        s += 8;
                        continue;
                    }
                }
            }
        }
        // A chunk holding a non-ASCII byte, or the last bytes before the
        // range: one character, as C steps.
        let byte = text[s];
        if map[byte as usize] != 0 {
            return Some(s);
        }
        s += if byte < 0x80 {
            1
        } else {
            enc.mbc_enc_len(&text[s..])
        };
    }
    None
}

/// Whether C's backward scans (`s = onigenc_get_prev_char_head(enc,
/// adjust_text, s)` starting at `first`) visit `pos`: the first position
/// itself, even inside a character, and every character head below it.
#[inline]
fn backward_scan_visits(
    enc: OnigEncoding,
    text: &[u8],
    adjust_text: usize,
    first: usize,
    pos: usize,
) -> bool {
    pos == first || left_adjust_char_head(enc, text, adjust_text, pos) == pos
}

/// Backward naive string search. Mirrors C's slow_search_backward: the scan
/// starts at `search_start` (or at the last position where the target fits,
/// left-adjusted) and steps back by characters while it is >= `text_start`.
/// Uses SIMD-accelerated memchr::memmem for the byte search and skips hits
/// that C's character stepping would not visit.
fn slow_search_backward(
    enc: OnigEncoding,
    target: &[u8],
    text: &[u8],
    text_start: usize,
    adjust_text: usize,
    text_end: usize,
    search_start: usize,
) -> Option<usize> {
    let tlen = target.len();
    if tlen == 0 {
        return Some(search_start);
    }
    // C: s = text_end - tlen; s > text_start ? text_start : LEFT_ADJUST(s)
    let last_fit = text_end.checked_sub(tlen)?;
    let first = if last_fit > search_start {
        search_start
    } else {
        left_adjust_char_head(enc, text, adjust_text, last_fit)
    };
    let mut right = first;
    while right >= text_start {
        let found = if tlen == 1 {
            memchr::memrchr(target[0], &text[text_start..=right])
        } else {
            memchr::memmem::rfind(&text[text_start..right + tlen], target)
        };
        let pos = text_start + found?;
        if backward_scan_visits(enc, text, adjust_text, first, pos) {
            return Some(pos);
        }
        if pos == 0 {
            break;
        }
        right = pos - 1;
    }
    None
}

/// Backward character map search. Mirrors C's map_search_backward: the scan
/// starts at `search_start` and steps back by characters while it is not
/// below `text_start`, so a start below `text_start` finds nothing.
/// Uses SIMD-accelerated memrchr when the map has 1-3 distinct ASCII bytes
/// and skips hits that C's character stepping would not visit.
#[cfg_attr(coverage_nightly, coverage(off))]
fn map_search_backward(
    enc: OnigEncoding,
    reg: &RegexType,
    text: &[u8],
    text_start: usize,
    adjust_text: usize,
    search_start: usize,
) -> Option<usize> {
    let first = search_start;
    let mut right = search_start;
    while right >= text_start {
        let haystack = &text[text_start..=right];
        let found = match reg.map_byte_count {
            1 => memchr::memrchr(reg.map_bytes[0], haystack),
            2 => memchr::memrchr2(reg.map_bytes[0], reg.map_bytes[1], haystack),
            3 => memchr::memrchr3(
                reg.map_bytes[0],
                reg.map_bytes[1],
                reg.map_bytes[2],
                haystack,
            ),
            _ => {
                // C's loop, one character at a time.
                let mut s = right;
                loop {
                    if reg.map[text[s] as usize] != 0 {
                        return Some(s);
                    }
                    if s <= adjust_text {
                        return None;
                    }
                    s = left_adjust_char_head(enc, text, adjust_text, s - 1);
                    if s < text_start {
                        return None;
                    }
                }
            }
        };
        let pos = text_start + found?;
        if backward_scan_visits(enc, text, adjust_text, first, pos) {
            return Some(pos);
        }
        if pos == 0 {
            break;
        }
        right = pos - 1;
    }
    None
}

/// Left-adjust char head (ONIGENC_LEFT_ADJUST_CHAR_HEAD).
/// For UTF-8: walk backward from pos to find char boundary at or before pos.
fn left_adjust_char_head(_enc: OnigEncoding, _text: &[u8], start: usize, pos: usize) -> usize {
    if pos <= start {
        return start;
    }
    let mut p = pos;
    while p > start && _text[p] & 0xC0 == 0x80 {
        p -= 1;
    }
    p
}

/// Backward search using optimization strategy.
/// Returns Some((low, high)) if a candidate was found, None otherwise.
fn backward_search(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    search_start: usize,
    min_range: usize,
    adjrange: usize,
) -> Option<(usize, usize)> {
    let mut p = search_start;
    loop {
        let found = match reg.optimize {
            OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward => {
                slow_search_backward(reg.enc, &reg.exact, str_data, min_range, adjrange, end, p)
            }
            OptimizeType::Map => {
                map_search_backward(reg.enc, reg, str_data, min_range, adjrange, p)
            }
            OptimizeType::None => {
                return None;
            }
        };

        p = found?;

        // Validate sub_anchor
        if reg.sub_anchor != 0 {
            let mut retry = false;
            if (reg.sub_anchor & ANCR_BEGIN_LINE) != 0 && p > 0 {
                let prev = onigenc_get_prev_char_head(reg.enc, str_data, 0, p);
                if !is_mbc_newline(reg.enc, str_data, prev, end) {
                    retry = true;
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
                if p == 0 {
                    return None;
                }
                p = onigenc_get_prev_char_head(reg.enc, str_data, adjrange, p);
                if p < adjrange {
                    return None;
                }
                continue;
            }
        }

        // Calculate low/high range
        if reg.dist_max != INFINITE_LEN {
            let low = p.saturating_sub(reg.dist_max as usize);

            let high = if reg.dist_min != 0 {
                p.saturating_sub(reg.dist_min as usize)
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
fn onigenc_get_right_adjust_char_head(
    enc: OnigEncoding,
    text: &[u8],
    start: usize,
    s: usize,
) -> usize {
    let p = left_adjust_char_head(enc, text, start, s);
    // Clamped: a truncated character at the end would otherwise point past
    // the subject (C returns that out-of-range pointer).
    if p < s {
        (p + enclen(enc, text, p)).min(text.len())
    } else {
        p
    }
}

/// Forward search using optimization strategy.
/// Returns Some((low, high)) if a candidate was found, None otherwise.
pub(crate) fn forward_search(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
) -> Option<(usize, usize)> {
    let found = forward_search_impl(reg, str_data, end, start, range);
    #[cfg(test)]
    FORWARD_SEARCH_BYTES.with(|bytes| {
        let read = found.map_or(range, |(_, high)| high).saturating_sub(start);
        bytes.set(bytes.get() + read as u64);
    });
    found
}

/// Rust-only (ADR-008): the match bounds of a pure literal pattern
/// (`RegexType::pure_literal`) from `start`, found by the forward literal
/// search alone. `Some(None)` is no match and `None` leaves the search to the
/// VM. Only a search whose attempts may consume the whole subject is
/// answered (`right_range == end`), with a start before `range`. Then the
/// first occurrence of the string at or after `start` is the match the
/// optimizer loop would attempt first, and the attempt succeeds: nothing
/// consumes or backtracks, and no failed attempt can count toward a limit.
/// A later occurrence is never tried, as the first success ends the search.
/// The options that change a success (FIND_LONGEST, FIND_NOT_EMPTY,
/// MATCH_WHOLE_STRING) and the string validity check keep the VM.
#[inline]
pub(crate) fn pure_literal_first(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    right_range: usize,
    options: OnigOptionType,
) -> Option<Option<(usize, usize)>> {
    if !reg.pure_literal
        || right_range != end
        || start >= range
        || range > end
        || opton_find_longest(options)
        || opton_find_not_empty(options)
        || opton_match_whole_string(options)
        || opton_check_validity_of_string(options)
        || (options.contains(ONIG_OPTION_CALLBACK_EACH_MATCH)
            && onig_get_callback_each_match().is_some())
        || !pure_literal_fast_path_enabled()
    {
        return None;
    }
    let haystack = str_data.get(start..end)?;
    let literal = reg.exact.as_slice();
    // The same finder as the optimizer's `StrFast` search (`sunday_quick_search`).
    let at = if literal.len() == 1 {
        memchr::memchr(literal[0], haystack)
    } else if let Some(finder) = &reg.exact_finder {
        finder.find(haystack)
    } else {
        memchr::memmem::find(haystack, literal)
    };
    Some(
        at.map(|i| start + i)
            .filter(|&pos| pos < range)
            .map(|pos| (pos, pos + literal.len())),
    )
}

fn forward_search_impl(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
) -> Option<(usize, usize)> {
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
                p = advance_char_to_end(reg.enc, str_data, p, end);
            }
        }
    }

    let mut pprev: Option<usize> = None;
    loop {
        // Search for the optimization target
        let found = match reg.optimize {
            OptimizeType::Str => slow_search(reg.enc, &reg.exact, str_data, p, end, range),
            OptimizeType::StrFast => sunday_quick_search(reg, &reg.exact, str_data, p, end, range),
            OptimizeType::StrFastStepForward => {
                sunday_quick_search_step_forward(reg, &reg.exact, str_data, p, end, range)
            }
            OptimizeType::Map => map_search(reg.enc, reg, str_data, p, range),
            OptimizeType::None => {
                return None;
            }
        };

        p = match found {
            Some(pos) if pos < range => pos,
            _ => return None,
        };

        if p.saturating_sub(start) < reg.dist_min as usize {
            pprev = Some(p);
            p = advance_char_to_end(reg.enc, str_data, p, end);
            continue; // retry
        }

        // Validate sub_anchor
        if reg.sub_anchor != 0 {
            let mut retry = false;
            if (reg.sub_anchor & ANCR_BEGIN_LINE) != 0 && p > 0 {
                let prev = pprev.unwrap_or(0);
                let prev_pos = onigenc_get_prev_char_head(reg.enc, str_data, prev, p);
                if !is_mbc_newline(reg.enc, str_data, prev_pos, end) {
                    retry = true;
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
                // A truncated multibyte lead byte reports a length past
                // `end`; stay inside the haystack as the outer loops do.
                pprev = Some(p);
                p = advance_char_to_end(reg.enc, str_data, p, end);
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

/// The character loop of the `ANCR_ANYCHAR_INF` search from `s`: it stops
/// behind the first newline, or at the first character boundary at or past
/// `cur_range`. Out of line, like the C loop's cold path.
#[inline(never)]
fn skip_past_newline(
    enc: OnigEncoding,
    str_data: &[u8],
    mut s: usize,
    cur_range: usize,
    end: usize,
) -> usize {
    if let Some(next) = after_next_newline(enc, str_data, s, cur_range, end) {
        return next;
    }
    while s < cur_range {
        let prev = s;
        s = advance_char_to_end(enc, str_data, s, end);
        if is_mbc_newline(enc, str_data, prev, end) {
            break;
        }
    }
    s
}

/// Rust-only (ADR-008): where [`skip_past_newline`]'s character loop
/// stops, found with `memchr`. `None` where that loop could step
/// differently: over bytes that are not valid UTF-8 in a multibyte
/// encoding, or with `cur_range` past `end`.
fn after_next_newline(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    cur_range: usize,
    end: usize,
) -> Option<usize> {
    if s >= cur_range || cur_range > end {
        return None;
    }
    let newline = memchr::memchr(b'\n', &str_data[s..cur_range]).map(|off| s + off);
    let stop = newline.unwrap_or(cur_range);
    if enc.max_enc_len() > 1 && std::str::from_utf8(&str_data[s..stop]).is_err() {
        return None;
    }
    Some(newline.map_or(cur_range, |at| at + 1))
}

#[cfg(test)]
thread_local! {
    /// Runs the lazy `.*?c` loop one instruction at a time, as the
    /// reference for `lazy_any_char_stop` in differential tests.
    static LAZY_STOP_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// What the fused lazy `.*?c` loop does at its `PushOrJumpExact1`.
enum LazyStep {
    /// Not fused: the instruction runs as usual.
    Stay,
    /// The loop reaches `c` here; the instruction pushes as usual.
    At(usize),
    /// `AnyChar` fails before any `c`.
    Fail,
    /// The retries of the loop's jumps reach a limit.
    Limit(i32),
}

/// The fused lazy `.*?c` loop at the `PushOrJumpExact1` at `p` (which jumps
/// back to the `AnyChar` before it): its stop, after counting the retries
/// its jumps count. Out of line: code added to `match_at_vm` shifts its
/// layout.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn lazy_any_char_step(
    reg: &RegexType,
    p: usize,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    retry_in_match_counter: &mut u64,
    retry_limit_in_match: u64,
    msa: &mut MatchArg,
) -> LazyStep {
    #[cfg(test)]
    if LAZY_STOP_DISABLED.with(|disabled| disabled.get()) {
        return LazyStep::Stay;
    }
    let OperationPayload::PushOrJumpExact1 {
        c, skipped_retries, ..
    } = reg.ops[p].payload
    else {
        return LazyStep::Stay;
    };
    // A time limit reads the clock by the number of retries counted.
    if msa.time_limit != 0 {
        return LazyStep::Stay;
    }
    let multiline = match p.checked_sub(1).map(|body| reg.ops[body].opcode) {
        Some(OpCode::AnyChar) => false,
        Some(OpCode::AnyCharMl) => true,
        _ => return LazyStep::Stay,
    };
    if skipped_retries & GUARD_RETRIES_BY_CHECKS != 0 {
        return LazyStep::Stay;
    }
    let (jumps, at) = match lazy_any_char_stop(reg.enc, str_data, s, right_range, c, multiline) {
        LazyStop::At(at, jumps) => (jumps, Some(at)),
        LazyStop::Fails(_, jumps) => (jumps, None),
        LazyStop::Unknown => return LazyStep::Stay,
    };
    if let Err(err) = count_retries_at_once(
        jumps * guard_retries(skipped_retries),
        retry_in_match_counter,
        retry_limit_in_match,
        msa,
    ) {
        return LazyStep::Limit(err);
    }
    match at {
        Some(at) => LazyStep::At(at),
        None => LazyStep::Fail,
    }
}

/// Seek across failed literal attempts of `AnyChar; Push(-1); MemEnd; ...;
/// c`, where the instructions after the `MemEnd` are capture ends,
/// negative one-character look-behinds and the literal, which starts with
/// `c` (`lazy_literal_byte` at compile time). Every position skipped fails
/// at the literal: the look-behinds only reject, and a back reference to a
/// literal group compares `c` first. The capture ends of a skipped position
/// are written again by the attempt at the returned stop, before any other
/// instruction reads them: that attempt runs the same capture ends first, and
/// the loop's last visited position is always a stop. Newline and range stops
/// still run the original attempt. Retries are counted for the loop's jumps
/// only, as the attempts that fail at the look-behinds and back references
/// need not be counted (ADR-017). If a budget could expire inside the skipped
/// attempts, keep the original instructions and state.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn lazy_captured_any_char_stop(
    reg: &RegexType,
    p: usize,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    c: u8,
    retry_in_match_counter: &mut u64,
    retry_limit_in_match: u64,
    msa: &mut MatchArg,
) -> Option<usize> {
    #[cfg(test)]
    if LAZY_STOP_DISABLED.with(|disabled| disabled.get()) {
        return None;
    }
    if msa.time_limit != 0
        || !(std::ptr::addr_eq(reg.enc, &crate::encodings::utf8::ONIG_ENCODING_UTF8)
            || std::ptr::addr_eq(reg.enc, &crate::encodings::ascii::ONIG_ENCODING_ASCII))
    {
        return None;
    }
    let multiline = match p.checked_sub(1).map(|body| reg.ops[body].opcode) {
        Some(OpCode::AnyChar) => false,
        Some(OpCode::AnyCharMl) => true,
        _ => return None,
    };
    // Bound speculative reads when malformed input makes the seek decline.
    // A complete UTF-8 prefix ends at a character boundary; otherwise the
    // original character instruction runs, including its malformed-byte rules.
    let seek_range = right_range.min(s.saturating_add(4096));
    let (at, jumps) = match lazy_any_char_stop(reg.enc, str_data, s, seek_range, c, multiline) {
        LazyStop::At(at, jumps) => (at, jumps),
        // Still execute the literal attempt and dot failure at this stop,
        // so capture state and both final retries remain instruction-exact.
        LazyStop::Fails(at, jumps) => (at, jumps - 1),
        LazyStop::Unknown => return None,
    };
    // A skipped literal fails once per character. Falling back before any
    // limit can trip preserves the capture end at the failing instruction.
    if (retry_limit_in_match != 0
        && jumps >= retry_limit_in_match.saturating_sub(*retry_in_match_counter))
        || (msa.retry_limit_in_search != 0
            && jumps
                >= msa
                    .retry_limit_in_search
                    .saturating_sub(msa.retry_limit_in_search_counter))
    {
        return None;
    }
    count_retries_at_once(jumps, retry_in_match_counter, retry_limit_in_match, msa).ok()?;
    Some(at)
}

/// Where the lazy loop `L: AnyChar; PushOrJumpExact1(c, L)` entered at
/// its `PushOrJumpExact1` from `s` stops: at the first `c` it reaches, or
/// failing at a newline (unless `multiline`) or at `right_range`.
enum LazyStop {
    /// The loop reaches `c` here and pushes its alternative, after jumping
    /// this many times.
    At(usize, u64),
    /// `AnyChar` fails before any `c`, after this many jumps.
    Fails(usize, u64),
    /// The character steps could land elsewhere: the bytes are not valid
    /// UTF-8 in a multibyte encoding, or `c` is not ASCII.
    Unknown,
}

/// Rust-only (ADR-008): [`LazyStop`] found with `memchr`. Between `s` and
/// the stop, the loop pushes nothing and counts no retries, so running it
/// in one step is unobservable. An ASCII byte in valid UTF-8 is a character
/// head, so the character steps land on the stop.
#[inline(never)]
fn lazy_any_char_stop(
    enc: OnigEncoding,
    str_data: &[u8],
    s: usize,
    right_range: usize,
    c: u8,
    multiline: bool,
) -> LazyStop {
    let right_range = right_range.min(str_data.len());
    if c >= 0x80 || s >= right_range {
        return LazyStop::Unknown;
    }
    let hay = &str_data[s..right_range];
    let found = if multiline || c == b'\n' {
        memchr::memchr(c, hay)
    } else {
        memchr::memchr2(c, b'\n', hay)
    };
    let stop = found.map_or(right_range, |off| s + off);
    let skipped = &str_data[s..stop];
    // One jump per character before the stop.
    let jumps = if enc.max_enc_len() == 1 {
        skipped.len()
    } else {
        match std::str::from_utf8(skipped) {
            Ok(text) => text.chars().count(),
            Err(_) => return LazyStop::Unknown,
        }
    } as u64;
    match found {
        Some(_) if str_data[stop] == c => LazyStop::At(stop, jumps),
        // A newline, or the range's end: the instruction jumps once more,
        // and `AnyChar` fails there.
        _ => LazyStop::Fails(stop, jumps + 1),
    }
}

/// `count_retry` for `backtracks` retries counted one at a time, without a
/// time limit: the count stops at the limit, where the single retries
/// would have stopped.
#[inline]
fn count_retries_at_once(
    backtracks: u64,
    retry_in_match_counter: &mut u64,
    retry_limit_in_match: u64,
    msa: &mut MatchArg,
) -> Result<(), i32> {
    if backtracks == 0 {
        return Ok(());
    }
    let backtracks = if retry_limit_in_match != 0 {
        backtracks.min(retry_limit_in_match.saturating_sub(*retry_in_match_counter))
    } else {
        backtracks
    };
    count_retry(
        backtracks,
        retry_in_match_counter,
        retry_limit_in_match,
        0,
        msa,
    )
}

/// Check if encoding is single-byte.
fn enc_is_singlebyte(enc: OnigEncoding) -> bool {
    enc.max_enc_len() == 1
}

/// Get previous character head position.
fn onigenc_get_prev_char_head(
    _enc: OnigEncoding,
    _text: &[u8],
    _start: usize,
    pos: usize,
) -> usize {
    if pos == 0 {
        return 0;
    }
    // For UTF-8: walk backwards past continuation bytes
    let mut p = pos - 1;
    while p > 0 && _text[p] & 0xC0 == 0x80 {
        p -= 1;
    }
    p
}

/// Check if position is a newline.
fn is_mbc_newline(_enc: OnigEncoding, text: &[u8], pos: usize, end: usize) -> bool {
    if pos >= end {
        return false;
    }
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
    // The following is an expanded code of onig_search_with_param() (C).
    let data_range = if range > start { range } else { end };
    search_in_range(
        reg, str_data, end, start, range, data_range, region, option, None,
    )
}

/// Port of C's `search_in_range()`: match starts are taken from
/// `[start, range]`, while every attempt may consume the subject up to
/// `data_range` (C: `MATCH_AND_RETURN_CHECK(data_range)`). `onig_search`
/// passes `range` itself (or `end` for a backward search); a regex-lead
/// RegSet search narrows `range` to its current winner but keeps the
/// original range as `data_range`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn search_in_range(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    data_range: usize,
    region: Option<OnigRegion>,
    option: OnigOptionType,
    limits: Option<SearchLimits>,
) -> (i32, Option<OnigRegion>) {
    let mut msa = take_cached_msa(reg, option, region, start, limits);
    let r = search_in_range_inner(reg, str_data, end, start, range, data_range, &mut msa);
    let region = msa.region.take();
    cache_msa(msa);
    (r, region)
}

// Box the cached state so each warm call moves only a pointer. Taking
// ownership still leaves the cache available to re-entrant calls.
thread_local! {
    static CACHED_MSA: Cell<Option<Box<MatchArg>>> = const { Cell::new(None) };
}

/// The thread's cached MatchArg, reset for a search under `limits` (`None`:
/// the process-wide limits), or a new one. Reusing it avoids a heap
/// allocation per search; `.take()` makes this safe for re-entrant calls
/// (e.g. from callouts). Every reset writes all four limits, so a search's
/// limits never reach a later search that takes the cache.
#[inline]
pub(crate) fn take_cached_msa(
    reg: &RegexType,
    option: OnigOptionType,
    region: Option<OnigRegion>,
    start: usize,
    limits: Option<SearchLimits>,
) -> Box<MatchArg> {
    match CACHED_MSA.try_with(Cell::take).ok().flatten() {
        Some(mut cached) => {
            cached.reset_with_limits(reg, option, region, start, limits);
            cached
        }
        None => Box::new(MatchArg::with_limits(reg, option, region, start, limits)),
    }
}

/// Give a MatchArg back to the thread's cache.
#[inline]
pub(crate) fn cache_msa(mut msa: Box<MatchArg>) {
    release_oversized_stack(&mut msa);
    // Drop can run after this thread-local's destructor during thread
    // teardown; the MatchArg is then simply freed.
    let _ = CACHED_MSA.try_with(|c| c.set(Some(msa)));
}

/// Rust-only (API layer): the bounds of the first match from `start` to the
/// end of `text`, as `onig_search` with no options would find it, without a
/// region. `msa` is reused across the searches of one iteration and must be
/// reset for this search first (`take_cached_msa` or `reset_with_limits`).
/// Not for expressions whose match can start away from its attempt (`\K`) or
/// with FIND_LONGEST, where only the region carries the match.
#[inline]
pub(crate) fn onig_search_bounds(
    reg: &RegexType,
    text: &[u8],
    start: usize,
    msa: &mut MatchArg,
) -> Result<Option<(usize, usize)>, i32> {
    debug_assert!(!reg.keep_moves_match_start && !opton_find_longest(reg.options));
    let end = text.len();
    // The guard keeps the call out of the common case; `pure_literal_first`
    // answers `None` for any other pattern.
    if reg.pure_literal
        && let Some(found) = pure_literal_first(reg, text, end, start, end, end, reg.options)
    {
        return Ok(found);
    }
    // Rust-only (ADR-008): the search runs the core directly. With no region
    // `search_in_range_inner` only clamps the bounds (already `end`) and
    // takes the core's `right_range == end`; a start past `end` is no match.
    let r = if start > end {
        ONIG_MISMATCH
    } else if can_use_two_pass_capture_fill(reg, start, end, msa) {
        search_in_range_inner(reg, text, end, start, end, end, msa)
    } else {
        onig_search_inner_core_with_right_range(reg, text, end, start, end, end, true, None, msa)
    };
    release_oversized_stack(msa);
    match r {
        r if r >= 0 => Ok(Some((r as usize, r as usize + msa.match_len as usize))),
        ONIG_MISMATCH => Ok(None),
        err => Err(err),
    }
}

/// Rust-only (ADR-008): the forward search of a standard capture method
/// (`captures*`, which passes no options), with `region` the caller's cached
/// region, returned with the result. A pattern that can match only at its start
/// makes its one attempt directly (`anchored_captures_search`); every other
/// search is `onig_search` over the whole subject, as before.
#[inline]
pub(crate) fn onig_search_captures(
    reg: &RegexType,
    text: &[u8],
    start: usize,
    region: OnigRegion,
) -> (i32, Option<OnigRegion>) {
    if anchored_captures_apply(reg, text.len(), start) {
        return anchored_captures_search(reg, text, start, region);
    }
    onig_search(
        reg,
        text,
        text.len(),
        start,
        text.len(),
        Some(region),
        ONIG_OPTION_NONE,
    )
}

/// Whether `onig_search_captures` takes the direct path: `onig_search` reaches
/// the start-anchor block of `onig_search_inner_core_impl` for this search, and
/// the code before that block returns nothing for it. The search runs forward
/// over the whole subject (`start < end`), so the empty-range and backward cases
/// do not arise. The AC and pure-literal paths need `ac_alt` or `pure_literal`,
/// and `ac_alt` is never set on an anchored pattern. FIND_LONGEST and the
/// validity check come from the regex's options, since captures pass none. A
/// `\A` anchor attempts only at 0, so any other start is left to the general
/// path.
#[inline(always)]
fn anchored_captures_apply(reg: &RegexType, end: usize, start: usize) -> bool {
    let anchored_here = (reg.anchor & ANCR_BEGIN_POSITION) != 0
        || ((reg.anchor & ANCR_BEGIN_BUF) != 0 && start == 0);
    start < end
        && anchored_here
        && reg.ac_alt.is_none()
        && !reg.pure_literal
        && !opton_find_longest(reg.options)
        && !opton_check_validity_of_string(reg.options)
        && anchored_start_applies(reg)
}

/// The search of `onig_search_captures` once `anchored_captures_apply` holds.
/// `onig_search` reaches the same attempt through `search_in_range_inner`, the
/// two-pass capture fill and the core loop. A single attempt needs none of
/// those layers: it records the captures of a region with groups as the fill's
/// first attempt does, and the fill has no second pass to make. The core loop
/// clears the region before its attempt. A match rewrites every register it
/// reports (`populate_region_for_match` writes `0..=num_mem`), so the clear is
/// made here only when no match is found. `history_root` is emptied before the
/// attempt, as that clear empties it.
#[inline(never)]
fn anchored_captures_search(
    reg: &RegexType,
    text: &[u8],
    start: usize,
    region: OnigRegion,
) -> (i32, Option<OnigRegion>) {
    let end = text.len();
    let mut msa = take_cached_msa(reg, ONIG_OPTION_NONE, Some(region), start, None);
    if let Some(ref mut r) = msa.region {
        r.history_root = None;
    }
    let r = anchored_start_run(reg, text, end, start, end, &mut msa);
    if r <= ONIG_MISMATCH
        && let Some(ref mut region) = msa.region
    {
        // No match, or a limit error: the registers are left cleared, as the
        // core loop leaves them.
        region.resize_clear(reg.num_mem + 1);
    }
    let region = msa.region.take();
    cache_msa(msa);
    (r, region)
}

/// Search reusing a pre-allocated MatchArg. Preserves buffer capacity.
pub(crate) fn onig_search_with_msa(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    msa: &mut MatchArg,
) -> (i32, Option<OnigRegion>) {
    let data_range = if range > start { range } else { end };
    let r = search_in_range_inner(reg, str_data, end, start, range, data_range, msa);
    (r, msa.region.take())
}

/// Search positions in `[start, range]` while allowing a match to consume up
/// to `right_range`. RegSet uses this to stop considering candidate starts
/// once they can no longer beat its current winner without truncating a match
/// that begins before that winner.
#[allow(clippy::too_many_arguments)]
///
/// `start_filter`, when given, is a start-byte map proven from the bytecode:
/// positions whose byte it excludes cannot start a match and are stepped over
/// by the position-by-position loop (see `onig_search_inner_core_with_right_range`).
pub(crate) fn onig_search_with_msa_and_right_range(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    right_range: usize,
    start_filter: Option<&[u8; CHAR_MAP_SIZE]>,
    msa: &mut MatchArg,
) -> (i32, Option<OnigRegion>) {
    let end = end.min(str_data.len());
    let range = range.min(end);
    let right_range = right_range.min(end);
    if start > end || right_range < start {
        return (ONIG_MISMATCH, msa.region.take());
    }

    let r = if can_use_two_pass_capture_fill(reg, start, range, msa) {
        onig_search_inner_two_pass(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            false,
            start_filter,
            msa,
        )
    } else {
        onig_search_inner_core_with_right_range(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            false,
            start_filter,
            msa,
        )
    };
    (r, msa.region.take())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_arguments)]
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
    let data_range = if range > start { range } else { end };
    search_in_range(
        reg,
        str_data,
        end,
        start,
        range,
        data_range,
        region,
        option,
        Some(SearchLimits::from(mp)),
    )
}

/// Rust-only (ADR-008): whether the search goes without the optimizer's
/// extra map (`RegexType::extra_map_optimizer`): C's map search would leave
/// out starts a leading positive look-behind can match from where its
/// characters do not line up (`crate::first_bytes::start_map_may_skip`). C
/// itself has no optimizer there and attempts every position.
#[inline]
pub(crate) fn map_search_bypassed(reg: &RegexType) -> bool {
    map_search_bypassed_for(reg, false)
}

/// Rust-only (ADR-008): whether the map optimizer's check from `start` (the
/// `forward_search` call of an unbounded search) finds `start` itself, so
/// that the call would return `Some` with `low == high == start` and read no
/// bytes. The map has a candidate at `start` when its byte is set; the
/// distance and sub-anchor checks then pass at `start`. Only then is the
/// search called away.
#[inline]
fn map_candidate_at(reg: &RegexType, str_data: &[u8], start: usize, range: usize) -> bool {
    reg.optimize == OptimizeType::Map
        && reg.dist_min == 0
        && reg.dist_max == INFINITE_LEN
        && reg.sub_anchor == 0
        && start < range
        && start < str_data.len()
        && reg.map[str_data[start] as usize] != 0
}

/// `map_search_bypassed` for a subject that is (`subject_utf8`) or may not
/// be valid UTF-8.
#[inline]
pub(crate) fn map_search_bypassed_for(reg: &RegexType, subject_utf8: bool) -> bool {
    reg.extra_map_optimizer && reg.look_behind_reach > 0 && !subject_utf8
}

#[inline]
fn can_use_two_pass_capture_fill(
    reg: &RegexType,
    start: usize,
    range: usize,
    msa: &MatchArg,
) -> bool {
    // Without capture groups a single pass already runs untracked (see
    // `match_at`) and records the region at OP_END; a second pass would only
    // repeat the winning attempt.
    start < range
        && reg.num_mem > 0
        && msa.region.is_some()
        && !opton_find_longest(msa.options)
        && !(msa.options.contains(ONIG_OPTION_CALLBACK_EACH_MATCH)
            && onig_get_callback_each_match().is_some())
        && !reg.needs_capture_tracking
        && reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0)
        // Keep this off when wall-time limiting is active: second pass is extra work.
        && msa.time_limit == 0
        && two_pass_capture_fill_pays(reg)
}

/// Whether attempts that run untracked, with a match among them run again to
/// record its captures, can save work (Rust-only, ADR-008). The second pass
/// repeats the successful attempt; the untracked ones save the stack entries
/// of push captures and the capture bookkeeping of each attempt.
/// - With push captures, unless the optimizer finds a literal at the match
///   start: its candidates often match, so the second pass would repeat
///   whole successful attempts while few attempts fail.
/// - Without them an untracked attempt saves little, which pays only while
///   the repeated match stays short: not where `.` repeats without bound and
///   a match may run to the end of the line.
pub(crate) fn two_pass_capture_fill_pays(reg: &RegexType) -> bool {
    if (reg.push_mem_start | reg.push_mem_end) != 0 {
        !(matches!(
            reg.optimize,
            OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward
        ) && reg.dist_max == 0)
    } else {
        !reg.anychar_run
    }
}

#[allow(clippy::too_many_arguments)]
fn onig_search_inner_two_pass(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    right_range: usize,
    find_longest_across_positions: bool,
    start_filter: Option<&[u8; CHAR_MAP_SIZE]>,
    msa: &mut MatchArg,
) -> i32 {
    if msa.region.is_none() {
        return onig_search_inner_core_with_right_range(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            find_longest_across_positions,
            start_filter,
            msa,
        );
    }

    // The first attempt the search makes records its captures as it matches:
    // a match there, the common case of a scanner search, needs no second
    // pass. Later attempts run untracked.
    msa.capture_position = Some(FIRST_ATTEMPT);
    // Rust-only (ADR-008): the map search's own first checks, read here so
    // that a pattern they reject does not pay for the call.
    let mapped = if capture_map_may_apply(reg) {
        capture_map_search(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            start_filter,
            msa,
        )
    } else {
        None
    };
    let match_start = mapped.unwrap_or_else(|| {
        onig_search_inner_core_with_right_range(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            find_longest_across_positions,
            start_filter,
            msa,
        )
    });
    let tracked_at = msa.capture_position.take();
    if match_start < 0 || Some(match_start as usize) == tracked_at {
        return match_start;
    }

    msa.best_len = ONIG_MISMATCH;
    msa.best_s = 0;

    let retry_counter_before = msa.retry_limit_in_search_counter;
    let retry_limit_in_match_before = msa.retry_limit_in_match;
    let retry_limit_in_search_before = msa.retry_limit_in_search;
    // Second pass is implementation-only; avoid consuming retry budget.
    msa.retry_limit_in_match = 0;
    msa.retry_limit_in_search = 0;
    // Forward searches attempt every candidate with `right_range` as the
    // match boundary, so the second pass uses the same one.
    let r = match_at(reg, str_data, end, right_range, match_start as usize, msa);
    msa.retry_limit_in_match = retry_limit_in_match_before;
    msa.retry_limit_in_search = retry_limit_in_search_before;
    msa.retry_limit_in_search_counter = retry_counter_before;
    if r < ONIG_MISMATCH {
        return r;
    }
    if r == ONIG_MISMATCH {
        // Conservative fallback: preserve semantics if second pass diverges.
        return onig_search_inner_core_with_right_range(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            find_longest_across_positions,
            start_filter,
            msa,
        );
    }

    match_start
}

#[inline(always)]
fn capture_map_scans_enabled() -> bool {
    #[cfg(test)]
    {
        !CAPTURE_MAP_SCANS_DISABLED.with(Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// Rust-only (ADR-008): necessary conditions of `capture_map_search`, the
/// fields it tests first and that do not change during a search. A pattern
/// that fails one of them is searched by the core loop as before.
#[inline(always)]
fn capture_map_may_apply(reg: &RegexType) -> bool {
    reg.optimize == OptimizeType::Map
        && reg.anchor == 0
        && reg.sub_anchor == 0
        && reg.dist_min == 0
        && reg.dist_max == 0
        && reg.ac_alt.is_none()
        && !reg.needs_capture_tracking
        && !reg.anychar_run
}

/// Continue a captured zero-distance map search on C's admitted positions.
/// The winning attempt still runs again to record its captures.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn capture_map_search(
    reg: &RegexType,
    text: &[u8],
    end: usize,
    start: usize,
    range: usize,
    right_range: usize,
    start_filter: Option<&[u8; CHAR_MAP_SIZE]>,
    msa: &mut MatchArg,
) -> Option<i32> {
    if !capture_map_scans_enabled()
        || reg.optimize != OptimizeType::Map
        || map_search_bypassed(reg)
        || reg.ac_alt.is_some()
        || reg.needs_capture_tracking
        || reg.dist_min != 0
        || reg.dist_max != 0
        || reg.anchor != 0
        || reg.sub_anchor != 0
        || reg.anychar_run
        || reg.leading_run.is_some()
        || reg.search_jump.is_some()
        || reg.search_start_map.is_some()
        || start_filter.is_some()
        || opton_find_longest(msa.options)
        || (msa.options.contains(ONIG_OPTION_CALLBACK_EACH_MATCH)
            && onig_get_callback_each_match().is_some())
        || msa.match_stack_limit != 0
    {
        return None;
    }
    let enc = reg.enc;
    if opton_check_validity_of_string(msa.options) && !enc.is_valid_mbc_string(&text[..end]) {
        return Some(ONIGERR_INVALID_WIDE_CHAR_VALUE);
    }
    if let Some(region) = msa.region.as_mut() {
        region.resize_clear(reg.num_mem + 1);
    }
    if (end as i32 - start as i32) < reg.threshold_len {
        return Some(ONIG_MISMATCH);
    }
    let Some(mut at) = map_search(enc, reg, text, start, range) else {
        return Some(ONIG_MISMATCH);
    };
    let mut scan = ForwardScan {
        end,
        cur_range: range,
        data_range: right_range,
        find_longest: false,
        ascii_compatible: onigenc_is_ascii_compatible_encoding(enc),
        search_jump: None,
        class_prefix: None,
        class_prefix_end: at,
        start_filter: None,
        filters: false,
        at,
    };
    loop {
        msa.best_len = ONIG_MISMATCH;
        msa.best_s = 0;
        // The first candidate may be the attempt that records captures.
        let result = if msa
            .capture_position
            .is_some_and(|tracked| tracked == at || tracked == FIRST_ATTEMPT)
        {
            match_at(reg, text, end, right_range, at, msa)
        } else {
            scan.at = at;
            let result =
                match_at_impl::<false, true>(reg, text, end, right_range, at, msa, Some(&mut scan));
            at = scan.at;
            result
        };
        if result != ONIG_MISMATCH {
            return Some(if result < 0 { result } else { at as i32 });
        }
        match scan.after_attempt::<true>(reg, msa, text, at) {
            Ok(Some(next)) => at = next,
            Ok(None) => return Some(ONIG_MISMATCH),
            Err(err) => return Some(err),
        }
    }
}

pub(crate) fn search_in_range_inner(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    data_range: usize,
    msa: &mut MatchArg,
) -> i32 {
    // The C API accepts an end pointer into the supplied buffer. Normalize
    // Rust indices once at the public-search boundary so every optimizer sees
    // a valid slice, and reject a start outside the logical string.
    let end = end.min(str_data.len());
    let range = range.min(end);
    let data_range = data_range.min(end);
    if start > end {
        return ONIG_MISMATCH;
    }

    if can_use_two_pass_capture_fill(reg, start, range, msa) {
        // A forward range bounds each attempt by `data_range`; see
        // `onig_search_inner_core`.
        return onig_search_inner_two_pass(
            reg, str_data, end, start, range, data_range, true, None, msa,
        );
    }
    onig_search_inner_core(reg, str_data, end, start, range, data_range, msa)
}

fn onig_search_inner_core(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    data_range: usize,
    msa: &mut MatchArg,
) -> i32 {
    let right_range = if start == range && start < end {
        (start + enclen(reg.enc, str_data, start)).min(end)
    } else if range > start {
        data_range
    } else {
        end
    };
    onig_search_inner_core_with_right_range(
        reg,
        str_data,
        end,
        start,
        range,
        right_range,
        true,
        None,
        msa,
    )
}

// Separate the bounded-window variant so single-position optimizer windows
// retain the original search loop without scan state or its per-attempt branch.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn onig_search_inner_core_with_right_range(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    right_range: usize,
    find_longest_across_positions: bool,
    start_filter: Option<&[u8; CHAR_MAP_SIZE]>,
    msa: &mut MatchArg,
) -> i32 {
    if reg.dist_max != INFINITE_LEN && reg.dist_max > reg.dist_min {
        onig_search_inner_core_impl::<true>(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            find_longest_across_positions,
            start_filter,
            msa,
        )
    } else {
        onig_search_inner_core_impl::<false>(
            reg,
            str_data,
            end,
            start,
            range,
            right_range,
            find_longest_across_positions,
            start_filter,
            msa,
        )
    }
}

/// Rust-only (ADR-008): a forward search from `start` (with `start < end`,
/// `start` before the search range, and no FIND_LONGEST across positions)
/// whose pattern begins with `\A` (`ANCR_BEGIN_BUF`) or `\G`
/// (`ANCR_BEGIN_POSITION`) can match only at `start`, so it makes that one
/// attempt. The caller has applied the threshold check. The optimizer's
/// answer on `start` comes first, as C's forward search would give it: an
/// O(1) byte test of the map where the map is the only filter, and otherwise
/// `forward_search` over `start` alone. A rejection is the result, as the
/// general loop would reach it, without the loop's setup and its attempts.
/// Returns `None` for a pattern with callouts or where the map search is
/// bypassed (both keep the general path), so it never answers a search the
/// general path would not reach.
///
/// Results are those of the general loop, which attempts `start` and no
/// position that an anchor excludes. The retry budget of the search is checked
/// after the one attempt, as the loop checks it, so a limit reached there is
/// reported; attempts the loop would make at other positions are not made.
#[inline(never)]
fn anchored_start_search(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    data_range: usize,
    msa: &mut MatchArg,
) -> Option<i32> {
    if !anchored_start_applies(reg) {
        return None;
    }
    Some(anchored_start_run(
        reg, str_data, end, start, data_range, msa,
    ))
}

/// Whether `anchored_start_run` may make the search's one attempt: the
/// optimizer-independent conditions of `anchored_start_search`.
#[inline(always)]
fn anchored_start_applies(reg: &RegexType) -> bool {
    anchored_start_enabled()
        && !map_search_bypassed(reg)
        && !reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0)
}

/// The attempt of `anchored_start_search` once `anchored_start_applies` holds.
#[inline(always)]
fn anchored_start_run(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    data_range: usize,
    msa: &mut MatchArg,
) -> i32 {
    // The threshold check of the general path (`map_search_bypassed` is off).
    if (end as i32 - start as i32) < reg.threshold_len {
        return ONIG_MISMATCH;
    }
    // The optimizer's answer on `start`, as the general loop's forward search
    // gives it. The attempt ends at `start + 1`, the bound that loop gives
    // its optimizer for a pattern with no distance (`cur_range`).
    let admitted = match reg.optimize {
        OptimizeType::None => true,
        // With no distance before `start` and no sub-anchor, a map candidate
        // at `start` is the match start the forward search returns.
        OptimizeType::Map
            if reg.dist_min == 0
                && reg.sub_anchor == 0
                && reg.map[str_data[start] as usize] != 0 =>
        {
            true
        }
        // Distance 0 and no candidate byte: the forward search has nothing to find
        // in `[start, start + 1)`.
        OptimizeType::Map if reg.dist_min == 0 && reg.dist_max == 0 && reg.sub_anchor == 0 => false,
        _ => {
            let range = start + 1;
            let sch_range = if reg.dist_max != 0 {
                if reg.dist_max == INFINITE_LEN || end.saturating_sub(range) < reg.dist_max as usize
                {
                    end
                } else {
                    range + reg.dist_max as usize
                }
            } else {
                range
            };
            matches!(
                forward_search(reg, str_data, end, start, sch_range),
                Some((low, high)) if low <= start && start <= high
            )
        }
    };
    if !admitted {
        return ONIG_MISMATCH;
    }
    msa.best_len = ONIG_MISMATCH;
    msa.best_s = 0;
    let r = match_at(reg, str_data, end, data_range, start, msa);
    if r != ONIG_MISMATCH {
        return if r < 0 { r } else { start as i32 };
    }
    if msa.retry_limit_in_search != 0
        && msa.retry_limit_in_search_counter >= msa.retry_limit_in_search
    {
        return ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER;
    }
    ONIG_MISMATCH
}

#[allow(clippy::too_many_arguments)]
fn onig_search_inner_core_impl<const WINDOW_SCAN: bool>(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    right_range: usize,
    find_longest_across_positions: bool,
    start_filter: Option<&[u8; CHAR_MAP_SIZE]>,
    msa: &mut MatchArg,
) -> i32 {
    let enc = reg.enc;
    // Skipping an attempt is unobservable except through the search retry
    // budget, which counts every failed attempt.
    let mut start_filter = start_filter.filter(|_| msa.retry_limit_in_search == 0);
    // Position-led RegSet searches still honor FIND_LONGEST within each
    // attempted position, but must return the earliest successful position.
    // Public onig_search retains its historical global-longest behavior.
    let find_longest = find_longest_across_positions && opton_find_longest(msa.options);
    let mut best_start: i32 = ONIG_MISMATCH;
    let mut best_len: i32 = ONIG_MISMATCH;

    if opton_check_validity_of_string(msa.options) && !enc.is_valid_mbc_string(&str_data[..end]) {
        return ONIGERR_INVALID_WIDE_CHAR_VALUE;
    }

    // Resize region once before entering search loops (matches C behavior)
    if let Some(ref mut r) = msa.region {
        r.resize_clear(reg.num_mem + 1);
    }

    // C treats start == range (before the logical end) as an anchored attempt
    // at exactly start, whose match extent is one encoded character. It is
    // neither a forward empty range nor a backward search.
    //
    // Deliberate divergence: C sends this case down its backward path, whose
    // optimized loop (`while (PTR_GE(s, low))`) is not bounded by `range`
    // and can return a match before `start` (`^.{2}c` on "abc\nabc\n" with
    // start = range = 4 yields 0 in C). Ferroni only tries `start`.
    if start == range && start < end {
        msa.best_len = ONIG_MISMATCH;
        msa.best_s = 0;
        let r = match_at(reg, str_data, end, right_range, start, msa);
        if r < ONIG_MISMATCH {
            return r;
        }
        // C: `goto mismatch`, see `search_returns_longest`.
        if r != ONIG_MISMATCH && (!find_longest || search_returns_longest(reg)) {
            return start as i32;
        }
        return ONIG_MISMATCH;
    }

    // Aho-Corasick fast path for literal alternations.
    // Single-pass scan replaces the position-by-position loop entirely.
    // Works for both bare `alpha|beta` and captured `(alpha|beta)`.
    if let Some(ref ac) = reg.ac_alt {
        if start <= range
            && !find_longest
            && !(msa.options.contains(ONIG_OPTION_CALLBACK_EACH_MATCH)
                && onig_get_callback_each_match().is_some())
        {
            // Matches start in `[start, range]` and may extend to
            // `right_range` (C's `data_range`).
            let search_end = right_range.min(end);
            if let Some(mat) = ac
                .find(&str_data[start..search_end])
                .filter(|mat| start + mat.start() <= range)
            {
                let match_start = start + mat.start();
                let match_end = start + mat.end();
                msa.match_len = (match_end - match_start) as i32;
                if let Some(ref mut r) = msa.region {
                    r.beg[0] = match_start as i32;
                    r.end[0] = match_end as i32;
                    if reg.ac_alt_has_capture {
                        r.beg[1] = match_start as i32;
                        r.end[1] = match_end as i32;
                    }
                }
                return match_start as i32;
            }
            return ONIG_MISMATCH;
        }
        // backward search or find_longest: fall through to normal path
    }

    // Rust-only (ADR-008): a pure literal pattern is answered by the literal
    // search, with the region filled as its one attempt would fill it.
    if reg.pure_literal
        && let Some(found) =
            pure_literal_first(reg, str_data, end, start, range, right_range, msa.options)
    {
        let Some((match_start, match_end)) = found else {
            return ONIG_MISMATCH;
        };
        msa.match_len = (match_end - match_start) as i32;
        if let Some(ref mut r) = msa.region {
            r.beg[0] = match_start as i32;
            r.end[0] = match_end as i32;
        }
        return match_start as i32;
    }

    if start > range {
        // Backward search: start > range, search from start down to range
        if end == 0 {
            return ONIG_MISMATCH;
        }

        // orig_start is the right boundary for matching (upper range). As in
        // C it may run past the logical end into the rest of the buffer, but
        // C adds the character length unclamped (`orig_start += enclen(...)`)
        // and reads past the buffer when a truncated character ends it;
        // Ferroni stops at the buffer.
        let orig_start = if start < end {
            let elen = enclen(enc, str_data, start);
            (start + elen).min(str_data.len())
        } else {
            end
        };

        // s starts at start (same as C: s = (UChar*)start)
        let mut s = start;

        // Macro-like helper for match_at + result handling in backward search
        macro_rules! backward_match_and_check {
            ($s:expr, $orig_start:expr) => {{
                msa.best_len = ONIG_MISMATCH;
                msa.best_s = 0;
                let r = match_at(reg, str_data, end, $orig_start, $s, msa);
                if r != ONIG_MISMATCH {
                    if r < 0 {
                        return r;
                    }
                    if find_longest {
                        let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                        if best_len == ONIG_MISMATCH || match_len > best_len {
                            best_start = $s as i32;
                            best_len = match_len;
                        }
                    } else {
                        return $s as i32;
                    }
                }
                if msa.retry_limit_in_search != 0
                    && msa.retry_limit_in_search_counter >= msa.retry_limit_in_search
                {
                    return ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER;
                }
            }};
        }

        if reg.optimize != OptimizeType::None && !map_search_bypassed(reg) {
            // Threshold length check (inside optimize branch, matching C)
            if (end as i32 - range as i32) < reg.threshold_len {
                return ONIG_MISMATCH;
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

                    if let Some((low, high)) =
                        backward_search(reg, str_data, end, sch_start, min_range, adjrange)
                    {
                        if s > high {
                            s = high;
                        }
                        while s >= low {
                            backward_match_and_check!(s, orig_start);
                            if s == 0 {
                                s = usize::MAX; // sentinel: past beginning (C returns NULL)
                                break;
                            }
                            s = onigenc_get_prev_char_head(enc, str_data, 0, s);
                        }
                    } else {
                        return finish_search(
                            find_longest,
                            best_start,
                            best_len,
                            reg,
                            str_data,
                            end,
                            orig_start,
                            msa,
                        );
                    }

                    if s == usize::MAX || s < range {
                        break;
                    }
                }
                return finish_search(
                    find_longest,
                    best_start,
                    best_len,
                    reg,
                    str_data,
                    end,
                    orig_start,
                    msa,
                );
            } else {
                // dist_max == INFINITE_LEN: single backward_search as gate
                let sch_start = onigenc_get_prev_char_head(enc, str_data, 0, end);
                if backward_search(reg, str_data, end, sch_start, min_range, adjrange).is_none() {
                    return ONIG_MISMATCH;
                }
            }
        }

        // Fallthrough: position-by-position loop (optimize == None or infinite dist_max gate passed)
        // C: do { MATCH; s = onigenc_get_prev_char_head(enc, str, s); }
        //    while (PTR_GE(s, range));
        // A range inside a character is not rounded down: the head below it
        // is not tried.
        loop {
            backward_match_and_check!(s, orig_start);
            if s == 0 {
                break; // C: prev_char_head returns NULL at str
            }
            s = onigenc_get_prev_char_head(enc, str_data, 0, s);
            if s < range {
                break;
            }
        }

        return finish_search(
            find_longest,
            best_start,
            best_len,
            reg,
            str_data,
            end,
            orig_start,
            msa,
        );
    }

    let mut cur_start = start;
    let mut cur_range = range;
    let data_range = right_range;

    // === Anchor optimization: narrow search range ===
    if reg.anchor != 0 && start < end {
        // Rust-only (ADR-008): a start-anchored pattern attempts only `start`;
        // see `anchored_start_search`. The general path below is unchanged.
        let anchored_start = !find_longest && start_filter.is_none() && range > start;
        if (reg.anchor & ANCR_BEGIN_POSITION) != 0 {
            // search start-position only
            if range > start {
                if anchored_start
                    && let Some(result) =
                        anchored_start_search(reg, str_data, end, start, data_range, msa)
                {
                    return result;
                }
                cur_range = start + 1;
            } else {
                cur_range = start;
            }
        } else if (reg.anchor & ANCR_BEGIN_BUF) != 0 {
            // search str-position only (must start at 0)
            if range > start {
                if start != 0 {
                    return ONIG_MISMATCH;
                }
                if anchored_start
                    && let Some(result) =
                        anchored_start_search(reg, str_data, end, start, data_range, msa)
                {
                    return result;
                }
                cur_range = 1;
            } else {
                return ONIG_MISMATCH;
            }
        } else if (reg.anchor & ANCR_END_BUF) != 0 {
            let min_semi_end = end;
            let max_semi_end = end;
            if (max_semi_end as OnigLen) < reg.anc_dist_min {
                return ONIG_MISMATCH;
            }
            if range > start {
                if reg.anc_dist_max != INFINITE_LEN
                    && min_semi_end.saturating_sub(start) > reg.anc_dist_max as usize
                {
                    cur_start = min_semi_end - reg.anc_dist_max as usize;
                }
                if max_semi_end.saturating_sub(cur_range.saturating_sub(1))
                    < reg.anc_dist_min as usize
                {
                    if max_semi_end + 1 < reg.anc_dist_min as usize {
                        return ONIG_MISMATCH;
                    } else {
                        cur_range = max_semi_end - reg.anc_dist_min as usize + 1;
                    }
                }
                if cur_start > cur_range {
                    return ONIG_MISMATCH;
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
                return ONIG_MISMATCH;
            }
            if range > start {
                if reg.anc_dist_max != INFINITE_LEN
                    && min_semi_end.saturating_sub(start) > reg.anc_dist_max as usize
                {
                    cur_start = min_semi_end - reg.anc_dist_max as usize;
                }
                if max_semi_end.saturating_sub(cur_range.saturating_sub(1))
                    < reg.anc_dist_min as usize
                {
                    if max_semi_end + 1 < reg.anc_dist_min as usize {
                        return ONIG_MISMATCH;
                    } else {
                        cur_range = max_semi_end - reg.anc_dist_min as usize + 1;
                    }
                }
                if cur_start > cur_range {
                    return ONIG_MISMATCH;
                }
            }
        } else if (reg.anchor & ANCR_ANYCHAR_INF_ML) != 0 && range > start {
            // start-position only
            if range > start {
                cur_range = start + 1;
            }
        }
    } else if start == end {
        // Empty string. Without the extra map C has no optimizer and no
        // threshold (`map_search_bypassed`).
        if reg.threshold_len == 0 || map_search_bypassed(reg) {
            let mut s = start;
            msa.best_len = ONIG_MISMATCH;
            msa.best_s = 0;
            let r = match_at(reg, str_data, end, end, s, msa);
            if r < ONIG_MISMATCH {
                return r;
            } // error
            // C: `goto mismatch`, see `search_returns_longest`.
            if r != ONIG_MISMATCH && (!find_longest || search_returns_longest(reg)) {
                return s as i32;
            }
        }
        return ONIG_MISMATCH;
    }

    // === Threshold length check ===
    if (end as i32 - cur_start as i32) < reg.threshold_len && !map_search_bypassed(reg) {
        return ONIG_MISMATCH;
    }

    // === Forward search ===
    let mut s = cur_start;
    // Rust-only (ADR-008): the search moves its start to where an attempt can
    // pass the expression's leading checks (a line start, leading classes or
    // literals), where no limit or FIND_LONGEST can observe a skipped attempt.
    let search_jump = match reg.search_jump.as_deref() {
        Some(jump) if !find_longest && may_skip_attempts(msa) => Some(jump),
        _ => None,
    };
    // Rust-only (ADR-008): the bytecode start map narrows the optimizer's
    // windows where no limit or FIND_LONGEST can observe a skipped attempt.
    let window_start_map = match reg.search_start_map.as_deref() {
        Some(map) if map.in_windows => window_start_map(map, find_longest, msa),
        _ => None,
    };

    // Use optimization if available
    if reg.optimize != OptimizeType::None && !map_search_bypassed(reg) {
        // Calculate search range for optimization
        let sch_range = if reg.dist_max != 0 {
            if reg.dist_max == INFINITE_LEN || end.saturating_sub(cur_range) < reg.dist_max as usize
            {
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
                if let Some(jump) = search_jump {
                    s = jump.next_start(reg, str_data, s, end, data_range, msa.options, None);
                }
                let Some((low, high)) = forward_search(reg, str_data, end, s, sch_range) else {
                    break;
                };
                if s < low {
                    s = low;
                }
                // Keep the optimizer's bounded attempt window, but reuse the
                // existing in-VM forward scan between its failed attempts.
                // This leaves every admitted position and limit count intact.
                let mut window_scan = (WINDOW_SCAN
                    && high > s
                    && high < end
                    && optimizer_window_scans_enabled()
                    && !find_longest
                    && reg.num_mem == 0
                    && !reg.needs_capture_tracking
                    && !reg.anychar_run
                    && reg.leading_run.is_none()
                    && search_jump.is_none()
                    && window_start_map.is_none()
                    && reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0))
                .then(|| ForwardScan {
                    end,
                    cur_range: high,
                    data_range,
                    find_longest: false,
                    ascii_compatible: onigenc_is_ascii_compatible_encoding(enc),
                    search_jump: None,
                    class_prefix: None,
                    class_prefix_end: s,
                    start_filter: None,
                    filters: false,
                    at: s,
                });
                while s <= high {
                    if let Some(map) = window_start_map {
                        if s < end
                            && map[str_data[s] as usize] == 0
                            && start_map_may_skip(reg, str_data, s)
                        {
                            s = advance_char_to_end(enc, str_data, s, end);
                            continue;
                        }
                    }
                    msa.best_len = ONIG_MISMATCH;
                    msa.best_s = 0;
                    let r = if let Some(scan) = window_scan.as_mut() {
                        scan.at = s;
                        let result =
                            match_at_in_scan(reg, str_data, end, data_range, s, msa, Some(scan));
                        s = scan.at;
                        result
                    } else {
                        match_at(reg, str_data, end, data_range, s, msa)
                    };
                    if r != ONIG_MISMATCH {
                        if r < 0 {
                            return r;
                        } // error
                        if find_longest {
                            let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                            if best_len == ONIG_MISMATCH || match_len > best_len {
                                best_start = s as i32;
                                best_len = match_len;
                            }
                        } else {
                            return s as i32;
                        }
                    }
                    if msa.retry_limit_in_search != 0
                        && msa.retry_limit_in_search_counter >= msa.retry_limit_in_search
                    {
                        return ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER;
                    }
                    // A truncated multibyte lead byte reports a length past
                    // `end`. C steps past the buffer here and its NUL
                    // terminator hides that; Rust stays inside the haystack
                    // and lets the next attempt run at the logical end.
                    s = if reg.leading_run.is_some() {
                        after_failed_run(
                            reg,
                            find_longest,
                            msa,
                            str_data,
                            end,
                            data_range,
                            cur_range,
                            s,
                        )
                    } else {
                        advance_char_to_end(enc, str_data, s, end)
                    };
                    if msa.skip_search > s {
                        s = msa.skip_search;
                    }
                }
                if s >= cur_range {
                    break;
                }
            }
            // C: goto mismatch -- optimized search exhausted, do not fall
            // through to the byte-by-byte loop below.
            return finish_search(
                find_longest,
                best_start,
                best_len,
                reg,
                str_data,
                end,
                data_range,
                msa,
            );
        } else {
            // Infinite dist_max: just check once, then fall through to normal loop
            if !map_candidate_at(reg, str_data, s, sch_range)
                && forward_search(reg, str_data, end, s, sch_range).is_none()
            {
                return finish_search(
                    find_longest,
                    best_start,
                    best_len,
                    reg,
                    str_data,
                    end,
                    data_range,
                    msa,
                );
            }
            // ANCR_ANYCHAR_INF: skip past newlines
            if (reg.anchor & ANCR_ANYCHAR_INF) != 0
                && (reg.anchor & (ANCR_LOOK_BEHIND | ANCR_PREC_READ_NOT)) == 0
            {
                while s < cur_range {
                    msa.best_len = ONIG_MISMATCH;
                    msa.best_s = 0;
                    let r = match_at(reg, str_data, end, data_range, s, msa);
                    if r != ONIG_MISMATCH {
                        if r < 0 {
                            return r;
                        }
                        if find_longest {
                            let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                            if best_len == ONIG_MISMATCH || match_len > best_len {
                                best_start = s as i32;
                                best_len = match_len;
                            }
                        } else {
                            return s as i32;
                        }
                    }
                    if msa.retry_limit_in_search != 0
                        && msa.retry_limit_in_search_counter >= msa.retry_limit_in_search
                    {
                        return ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER;
                    }
                    let prev = s;
                    s = advance_char_to_end(enc, str_data, s, end);
                    if msa.skip_search > s {
                        s = msa.skip_search;
                    }
                    // Skip past non-newline chars
                    if !is_mbc_newline(enc, str_data, prev, end) {
                        s = skip_past_newline(enc, str_data, s, cur_range, end);
                    }
                }
                return finish_search(
                    find_longest,
                    best_start,
                    best_len,
                    reg,
                    str_data,
                    end,
                    data_range,
                    msa,
                );
            }
            // Fall through to normal position loop below. Only this path
            // looks up the Rust-only filter, which keeps it off the others.
            if msa.retry_limit_in_search == 0 {
                // The same as `or_else` on each source in turn, without the
                // closure calls on every search.
                if start_filter.is_none() {
                    start_filter = unbounded_optimizer_start_bytes(reg);
                }
                if start_filter.is_none() {
                    start_filter = reg
                        .search_start_map
                        .as_deref()
                        .filter(|map| start_map_applies(map, find_longest, msa))
                        .map(|map| &map.bytes);
                }
            }
        }
    }

    // Rust-only (ADR-008): an extra map that C's map search may not use
    // after a leading look-behind filters the position loop where it sits at
    // the match start; the loop leaves a start to the look-behind where its
    // characters do not line up.
    if map_search_bypassed(reg) && reg.dist_max == 0 && !find_longest && may_skip_attempts(msa) {
        start_filter = start_filter.or(Some(&reg.map));
    }

    if reg.leading_run.is_some() && best_start == ONIG_MISMATCH {
        if let Some(result) = literal_run_search(
            reg,
            find_longest,
            str_data,
            end,
            s,
            cur_range,
            data_range,
            start_filter,
            msa,
        ) {
            return result;
        }
    }

    if reg.literal_prefix.is_some() && best_start == ONIG_MISMATCH {
        if let Some(result) = literal_prefix_search(
            reg,
            find_longest,
            str_data,
            end,
            s,
            cur_range,
            data_range,
            start_filter,
            msa,
        ) {
            return result;
        }
    }

    // Normal position-by-position search (no optimization or fallthrough)
    if best_start == ONIG_MISMATCH {
        let class_prefix = atomic_ascii_class_prefix(reg, msa);
        let mut scan = ForwardScan {
            end,
            cur_range,
            data_range,
            find_longest,
            ascii_compatible: onigenc_is_ascii_compatible_encoding(enc),
            search_jump,
            class_prefix,
            class_prefix_end: s,
            start_filter,
            filters: search_jump.is_some() || class_prefix.is_some() || start_filter.is_some(),
            at: s,
        };
        // Callouts can observe the attempts and set `skip_search`: each
        // attempt keeps its own call there.
        let in_vm = reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0);
        if scan.filters {
            s = scan.attempt_start(reg, msa, str_data, s);
        }
        loop {
            msa.best_len = ONIG_MISMATCH;
            msa.best_s = 0;
            // The attempt at the start of a two-pass search records its
            // captures, the later ones do not (see `match_at`); only
            // attempts with the same tracking continue in the VM.
            let r = if in_vm && msa.capture_position != Some(s) {
                scan.at = s;
                let r = match_at_in_scan(reg, str_data, end, data_range, s, msa, Some(&mut scan));
                s = scan.at;
                r
            } else {
                match_at(reg, str_data, end, data_range, s, msa)
            };
            if r != ONIG_MISMATCH {
                if r < 0 {
                    return r;
                }
                if find_longest {
                    let match_len = if msa.best_len >= 0 { msa.best_len } else { r };
                    if best_len == ONIG_MISMATCH || match_len > best_len {
                        best_start = s as i32;
                        best_len = match_len;
                    }
                } else {
                    return s as i32;
                }
            }
            match scan.after_attempt::<false>(reg, msa, str_data, s) {
                Ok(Some(next)) => s = next,
                Ok(None) => break,
                Err(err) => return err,
            }
        }
    }

    finish_search(
        find_longest,
        best_start,
        best_len,
        reg,
        str_data,
        end,
        data_range,
        msa,
    )
}

/// The steps between two attempts of the forward position-by-position loop
/// of `onig_search_inner_core_with_right_range` (C: the search-limit check
/// of `MATCH_AND_RETURN_CHECK`, the loop condition and
/// `s += enclen(reg->enc, s)`), with the Rust-only start filters at the loop
/// head.
///
/// Rust-only (ADR-008): the search loop runs them after an attempt returns,
/// and `match_at_impl` runs them where an attempt fails, to start the next
/// attempt without leaving the VM. A failing attempt then costs no call,
/// prologue or epilogue; the VM restores the state its prologue sets up, and
/// both run the same steps, so every attempt, retry count and limit stays
/// as it was.
pub(crate) struct ForwardScan<'a> {
    end: usize,
    /// Inclusive in ordinary scans, exclusive for optimizer map candidates.
    cur_range: usize,
    data_range: usize,
    find_longest: bool,
    /// `onigenc_is_ascii_compatible_encoding`, read once per search.
    ascii_compatible: bool,
    search_jump: Option<&'a crate::leading_run::SearchJump>,
    class_prefix: Option<(&'a BitSet, u8)>,
    class_prefix_end: usize,
    start_filter: Option<&'a [u8; CHAR_MAP_SIZE]>,
    /// One of the three filters above is set.
    filters: bool,
    /// The start of the attempt the VM made last.
    at: usize,
}

impl ForwardScan<'_> {
    /// The loop head: moves `s` past the starts the Rust-only filters leave
    /// out.
    #[inline(never)]
    fn attempt_start(
        &mut self,
        reg: &RegexType,
        msa: &MatchArg,
        str_data: &[u8],
        mut s: usize,
    ) -> usize {
        if let Some(jump) = self.search_jump {
            s = jump.next_start(
                reg,
                str_data,
                s,
                self.end,
                self.data_range,
                msa.options,
                Some(self.cur_range),
            );
        }
        if let Some((bsp, delimiter)) = self.class_prefix {
            if s >= self.class_prefix_end {
                (s, self.class_prefix_end) = skip_nonmatching_class_prefix(
                    reg.enc,
                    str_data,
                    self.end,
                    self.cur_range,
                    s,
                    bsp,
                    delimiter,
                );
            }
        }
        if let Some(filter) = self.start_filter {
            // No match starts on an excluded byte. The range limit and the
            // logical end are still attempted, as the loop does.
            while s < self.cur_range
                && s < self.end
                && filter[str_data[s] as usize] == 0
                && start_map_may_skip(reg, str_data, s)
            {
                s = self.advance(reg.enc, str_data, s);
            }
        }
        s
    }

    /// After the attempt at `s` (that failed, or matched under
    /// FIND_LONGEST): the start of the next attempt, `None` at the end of
    /// the range, or the search retry limit error.
    #[inline(always)]
    fn after_attempt<const OPTIMIZER_MAP: bool>(
        &mut self,
        reg: &RegexType,
        msa: &MatchArg,
        str_data: &[u8],
        s: usize,
    ) -> Result<Option<usize>, i32> {
        if msa.retry_limit_in_search != 0
            && msa.retry_limit_in_search_counter >= msa.retry_limit_in_search
        {
            return Err(ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER);
        }
        if s >= self.cur_range || s >= self.end {
            return Ok(None);
        }
        let next = self.advance(reg.enc, str_data, s);
        let mut s = if reg.leading_run.is_some() {
            after_failed_run_from(
                reg,
                self.find_longest,
                msa,
                str_data,
                self.end,
                self.data_range,
                self.cur_range,
                s,
                next,
            )
        } else {
            next
        };
        if msa.skip_search > s {
            s = msa.skip_search;
        }
        if OPTIMIZER_MAP {
            return Ok(next_optimizer_map_start(reg, str_data, s, self.cur_range));
        }
        if self.filters {
            s = self.attempt_start(reg, msa, str_data, s);
        }
        Ok(Some(s))
    }

    /// `advance_char_to_end` with the encoding's ASCII compatibility read
    /// once per search.
    #[inline]
    fn advance(&self, enc: OnigEncoding, str_data: &[u8], s: usize) -> usize {
        if self.ascii_compatible && s < str_data.len() && str_data[s] < 0x80 {
            (s + 1).min(self.end)
        } else {
            advance_char_to_end(enc, str_data, s, self.end)
        }
    }
}

/// Keep the map scan out of the VM's attempt-failure dispatch.
#[inline(never)]
fn next_optimizer_map_start(
    reg: &RegexType,
    text: &[u8],
    start: usize,
    range: usize,
) -> Option<usize> {
    map_search(reg.enc, reg, text, start, range)
}

/// A leading atomic `[class]+literal` has only one possible exit from its
/// ASCII run. A run ending at a different byte cannot match from any suffix.
/// Recognize the exact auto-possessive sequence (the class, its star and
/// `PopStarChoices`, as `compile_strict_real_repeat` emits it), leaving
/// captures, anchors, alternations, and every other prefix on the ordinary VM
/// path.
fn atomic_ascii_class_prefix<'a>(reg: &'a RegexType, msa: &MatchArg) -> Option<(&'a BitSet, u8)> {
    let [head, star, pop, literal, ..] = reg.ops.as_slice() else {
        return None;
    };
    let (
        OpCode::CClass,
        OperationPayload::CClass { bsp, .. },
        OpCode::CClassStar,
        OperationPayload::CClass { bsp: tail, .. },
        OpCode::PopStarChoices,
        OpCode::Str1,
        OperationPayload::Exact { s },
    ) = (
        head.opcode,
        &head.payload,
        star.opcode,
        &star.payload,
        pop.opcode,
        literal.opcode,
        &literal.payload,
    )
    else {
        return None;
    };
    #[cfg(test)]
    if ATOMIC_CLASS_PREFIX_DISABLED.with(Cell::get) {
        return None;
    }
    // Every rejected prefix fails before reaching the tail and pops only the
    // bottom sentinel: `PopStarChoices` removes the lazy star choice. That
    // costs one retry per attempt. A per-match limit of one or an
    // accumulating search budget can therefore observe the attempts; so can
    // stack/time limits.
    if msa.retry_limit_in_search != 0
        || msa.retry_limit_in_match == 1
        || msa.match_stack_limit != 0
        || msa.time_limit != 0
        || reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0)
        || !onigenc_is_ascii_compatible_encoding(reg.enc)
    {
        return None;
    }
    // C's `tune_next` gives a star no peek-next byte `\0`, and the filter
    // keeps to the literals the peek-next sequence had.
    let c = s[0];
    (bsp == tail
        && c != 0
        && !bitset_at(bsp, c as usize)
        && bsp[128 / BITS_IN_ROOM..].iter().all(|&bits| bits == 0))
    .then_some((bsp, c))
}

fn skip_nonmatching_class_prefix(
    enc: OnigEncoding,
    text: &[u8],
    end: usize,
    range: usize,
    mut start: usize,
    bsp: &BitSet,
    delimiter: u8,
) -> (usize, usize) {
    while start < range && start < end {
        let mut run_end = start;
        while run_end < end && bitset_at(bsp, text[run_end] as usize) {
            run_end += 1;
        }
        if run_end > start {
            if run_end < end && text[run_end] == delimiter {
                // If the tail fails, the remaining starts in this same run
                // already satisfy the prefix. Do not scan it again per suffix.
                return (start, run_end);
            }
            start = run_end.min(range);
        } else {
            start = advance_char_to_end(enc, text, start, end);
        }
    }
    (start, start)
}

/// Rust-only start filter of a search whose optimizer (C's) sits at an
/// unbounded distance, so that the search attempts every position after one
/// successful optimizer check: the bytes a match can start with, where the
/// extra byte maps pin them down (`first_byte_map`, set only for a map at
/// distance 0). Skipping a position it excludes cannot lose a match; without
/// callouts it is unobservable but for the search retry budget, which the
/// caller checks.
#[inline]
fn unbounded_optimizer_start_bytes(reg: &RegexType) -> Option<&[u8; CHAR_MAP_SIZE]> {
    (reg.has_first_byte_map
        && reg.dist_max == INFINITE_LEN
        && reg.optimize != OptimizeType::None
        && reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0))
    .then_some(&reg.first_byte_map)
}

/// Whether the forward search may leave out attempts that cannot match
/// (`crate::leading_run`). Each attempt it leaves out would fail with at most
/// the backtracks of an attempt that already failed, or with a count known
/// at compile time (one for the leading-run skips, `miss_retries` for the
/// start map): a per-match retry limit that low, the search retry budget, and
/// stack or time limits could observe that.
#[inline]
fn may_skip_attempts(msa: &MatchArg) -> bool {
    msa.retry_limit_in_search == 0
        && msa.retry_limit_in_match != 1
        && msa.match_stack_limit == 0
        && msa.time_limit == 0
}

/// Whether the bytecode start map may leave out the attempts it excludes:
/// each of them would count `miss_retries` backtracks.
#[inline]
fn start_map_applies(
    map: &crate::leading_run::SearchStartMap,
    find_longest: bool,
    msa: &MatchArg,
) -> bool {
    !find_longest && may_skip_attempts(msa) && map.skippable(msa.retry_limit_in_match)
}

/// The bytecode start map for the positions of bounded optimizer windows.
/// Out of line, like the other leading-run helpers: code added to the search
/// loop shifts its layout.
#[inline(never)]
fn window_start_map<'a>(
    map: &'a crate::leading_run::SearchStartMap,
    find_longest: bool,
    msa: &MatchArg,
) -> Option<&'a [u8; CHAR_MAP_SIZE]> {
    start_map_applies(map, find_longest, msa).then_some(&map.bytes)
}

/// The position after a failed attempt at `failed`: the end of its leading
/// run, since every start inside the run tries a subset of the same end
/// positions (`crate::leading_run`), or else the next character. A run
/// reaching past `cur_range` is not skipped: the loop's last attempt is the
/// first character boundary at or past `cur_range`, which need not be the
/// run's end.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn after_failed_run(
    reg: &RegexType,
    find_longest: bool,
    msa: &MatchArg,
    str_data: &[u8],
    end: usize,
    data_range: usize,
    cur_range: usize,
    failed: usize,
) -> usize {
    let next = advance_char_to_end(reg.enc, str_data, failed, end);
    after_failed_run_from(
        reg,
        find_longest,
        msa,
        str_data,
        end,
        data_range,
        cur_range,
        failed,
        next,
    )
}

/// `after_failed_run` given the next character's position `next`.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn after_failed_run_from(
    reg: &RegexType,
    find_longest: bool,
    msa: &MatchArg,
    str_data: &[u8],
    end: usize,
    data_range: usize,
    cur_range: usize,
    failed: usize,
    next: usize,
) -> usize {
    let Some(run) = reg.leading_run.as_deref() else {
        return next;
    };
    // Cheap checks first: a run that the next byte already ends has nothing
    // to skip, and a failure at a multibyte character, whose class lookup
    // costs about as much as the attempt did, keeps the plain step.
    if failed >= end
        || str_data[failed] >= 0x80
        || next >= data_range
        || !crate::leading_run::read_head(run, str_data[failed])
        || !crate::leading_run::may_continue(reg, run, str_data[next])
        // A word start tries both optional-prefix paths. Its failed
        // attempts bound the retries of every later ASCII start; a nonword
        // start tries only one path, so keep it when a match limit is set.
        || (run.optional_word_prefix
            && msa.retry_limit_in_match != 0
            && !(str_data[failed].is_ascii_alphanumeric() || str_data[failed] == b'_'))
        // FIND_NOT_EMPTY can reject a successful assertion after its cut;
        // that failure does not bound later assertions' retry counts.
        || (run.whole_lookahead && opton_find_not_empty(msa.options))
        || find_longest
        || !may_skip_attempts(msa)
    {
        return next;
    }
    let run_end = crate::leading_run::run_end(reg, run, str_data, data_range, failed);
    if run_end > next && run_end <= cur_range {
        run_end
    } else {
        next
    }
}

/// Rust-only (ADR-008): the forward position loop for an expression that
/// starts with an atomic class run followed by a literal the class does not
/// hold. A match starts in the run that ends at an occurrence of the literal,
/// and only that run's first position needs an attempt: every later start in
/// it tries a subset of the same end positions. Starts before the run end
/// their own runs elsewhere, where no occurrence follows. A segment that is
/// not valid UTF-8 falls back to attempting each position, as the plain loop
/// does, because a malformed multibyte sequence can carry the VM's run past a
/// byte the scan stops at.
///
/// `None` where the expression has no such literal or a limit could observe
/// the skipped attempts; the plain loop then runs.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn literal_run_search(
    reg: &RegexType,
    find_longest: bool,
    str_data: &[u8],
    end: usize,
    mut s: usize,
    cur_range: usize,
    data_range: usize,
    start_filter: Option<&[u8; CHAR_MAP_SIZE]>,
    msa: &mut MatchArg,
) -> Option<i32> {
    let run = reg.leading_run.as_deref()?;
    let literal = run.literal.as_ref()?;
    if find_longest || !may_skip_attempts(msa) {
        return None;
    }
    let enc = reg.enc;
    let admitted = |x: usize| {
        x >= end
            || start_filter.is_none_or(|filter| {
                filter[str_data[x] as usize] != 0 || !start_map_may_skip(reg, str_data, x)
            })
    };
    macro_rules! attempt {
        ($x:expr) => {{
            let x = $x;
            if admitted(x) {
                msa.best_len = ONIG_MISMATCH;
                msa.best_s = 0;
                let r = match_at(reg, str_data, end, data_range, x, msa);
                if r != ONIG_MISMATCH {
                    if r < 0 {
                        return Some(r);
                    }
                    return Some(x as i32);
                }
            }
        }};
    }
    // `s` is always a position the plain loop steps to. That loop's last
    // attempt is the first character boundary at or past `cur_range`.
    loop {
        if s > cur_range {
            // Stepped past `cur_range` over a malformed sequence: this is the
            // loop's last attempt.
            attempt!(s);
            break;
        }
        let Some(k) = literal.find(str_data, s, data_range) else {
            break;
        };
        // A start filter that rejects the run's first position leaves no
        // failed attempt there to stand for the rest of the run.
        let run_start = crate::leading_run::run_start_before(reg, run, str_data, s, k)
            .filter(|&r| (r == k && !run.min_zero) || admitted(r));
        match run_start {
            Some(r) => {
                // `s..k` is well formed: its boundaries are the loop's steps.
                let attempted = r <= cur_range
                    || (enc.max_enc_len() > 1
                        && str_data[cur_range..r].iter().all(|&b| (b & 0xC0) == 0x80));
                // An empty `C*` run starts at the occurrence itself.
                if (r < k || run.min_zero) && attempted {
                    attempt!(r);
                }
                if k >= cur_range {
                    break;
                }
                s = k + 1;
            }
            None => {
                // Attempt every position before the occurrence, as the plain
                // loop does.
                let mut x = s;
                while x < k {
                    attempt!(x);
                    if x >= cur_range {
                        return Some(ONIG_MISMATCH);
                    }
                    x = advance_char_to_end(enc, str_data, x, end);
                }
                if x == k {
                    // The occurrence's byte ends every run, so only an empty
                    // `C*` run can start there.
                    if run.min_zero {
                        attempt!(k);
                    }
                    if k >= cur_range {
                        break;
                    }
                    x = k + 1;
                }
                s = x;
            }
        }
    }
    Some(ONIG_MISMATCH)
}

/// Rust-only (ADR-008): the forward position loop for an expression that
/// reads only zero-width checks and class repetitions before a literal
/// outside those classes (`crate::leading_run::LiteralPrefix`). A start
/// whose class run does not end at an occurrence of the literal cannot
/// match, so only the starts in the run before each occurrence are
/// attempted, in order. Where `s..k` is not valid UTF-8 every position there
/// is attempted, as the plain loop does.
///
/// `None` where a limit or FIND_LONGEST could observe the skipped attempts;
/// the plain loop then runs.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn literal_prefix_search(
    reg: &RegexType,
    find_longest: bool,
    str_data: &[u8],
    end: usize,
    mut s: usize,
    cur_range: usize,
    data_range: usize,
    start_filter: Option<&[u8; CHAR_MAP_SIZE]>,
    msa: &mut MatchArg,
) -> Option<i32> {
    let prefix = reg.literal_prefix.as_deref()?;
    // A per-match retry limit at or below the bound of a left-out attempt's
    // backtracks could observe it.
    if find_longest || !may_skip_attempts(msa) || !prefix.skippable(msa.retry_limit_in_match) {
        return None;
    }
    let enc = reg.enc;
    let admitted = |x: usize| {
        x >= end
            || start_filter.is_none_or(|filter| {
                filter[str_data[x] as usize] != 0 || !start_map_may_skip(reg, str_data, x)
            })
    };
    macro_rules! attempt {
        ($x:expr) => {{
            let x = $x;
            if admitted(x) {
                msa.best_len = ONIG_MISMATCH;
                msa.best_s = 0;
                let r = match_at(reg, str_data, end, data_range, x, msa);
                if r != ONIG_MISMATCH {
                    if r < 0 {
                        return Some(r);
                    }
                    return Some(x as i32);
                }
            }
        }};
    }
    // `s` is always a position the plain loop steps to. That loop's last
    // attempt is the first character boundary at or past `cur_range`.
    loop {
        if s > cur_range {
            // Stepped past `cur_range` over a malformed sequence: this is the
            // loop's last attempt.
            attempt!(s);
            break;
        }
        // A well-formed `s..k` makes every character head in it a step of
        // the plain loop; otherwise every position from `s` is attempted.
        let Some((first, k)) = prefix.window(enc, str_data, s, cur_range, data_range) else {
            break;
        };
        let mut x = first;
        while x <= k {
            attempt!(x);
            if x >= cur_range {
                return Some(ONIG_MISMATCH);
            }
            x = advance_char_to_end(enc, str_data, x, end);
        }
        // Go on where the plain loop would: right after the occurrence's
        // ASCII byte, or past it where a malformed sequence stepped over it.
        s = x;
    }
    Some(ONIG_MISMATCH)
}

/// The end of a search without a match (C: `mismatch`), or, under
/// FIND_LONGEST, the replay of the longest match. Inline: most searches end
/// here without a match, and the call alone cost more than the check.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn finish_search(
    find_longest: bool,
    best_start: i32,
    best_len: i32,
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    upper_range: usize,
    msa: &mut MatchArg,
) -> i32 {
    if find_longest && best_start != ONIG_MISMATCH && search_returns_longest(reg) {
        return finish_longest_search(best_start, reg, str_data, end, upper_range, msa);
    }
    ONIG_MISMATCH
}

/// Whether a FIND_LONGEST search returns the longest match its attempts
/// collected. C's attempts collect it under the search's options
/// (`msa->options`) and then fail, but `search_in_range` returns it
/// (`mismatch:`) only for a FIND_LONGEST compiled into the expression
/// (`OPTON_FIND_LONGEST(reg->options)`). With FIND_LONGEST passed to the
/// search alone, the search reports no match.
#[inline]
fn search_returns_longest(reg: &RegexType) -> bool {
    opton_find_longest(reg.options)
}

/// `finish_search` with a FIND_LONGEST match.
#[inline(never)]
fn finish_longest_search(
    best_start: i32,
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    upper_range: usize,
    msa: &mut MatchArg,
) -> i32 {
    if let Some(ref mut r) = msa.region {
        r.clear();
    }
    msa.best_len = ONIG_MISMATCH;
    msa.best_s = 0;
    // Replay the winner with the upper range every attempt used (C:
    // MATCH_AND_RETURN_CHECK(orig_start / data_range)); C keeps the
    // region that attempt recorded.
    match_at(reg, str_data, end, upper_range, best_start as usize, msa);
    best_start
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {

    #[test]
    fn guard_chain_skip_gates_preserve_captures_unicode_and_retry_limits() {
        let _limits = shared_limits();
        // Django's broad domain character class generates full case-fold
        // alternatives. The final class forces the greedy repeat to backtrack.
        let compile = |gated: bool| {
            regcomp::SKIP_GATES_DISABLED.with(|disabled| disabled.set(!gated));
            let reg = regcomp::onig_new(
                r"\A([a-z¡-\x{ffff}0-9]{1,63})[a-z¡-\x{ffff}0-9]\z".as_bytes(),
                ONIG_OPTION_IGNORECASE | ONIG_OPTION_CAPTURE_GROUP,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap();
            regcomp::SKIP_GATES_DISABLED.with(|disabled| disabled.set(false));
            reg
        };
        let gated = compile(true);
        let ungated = compile(false);
        assert!(
            !gated.skip_gates.is_empty() && ungated.skip_gates.is_empty(),
            "the regression must exercise a gated chain"
        );
        let mut texts = vec![
            b"".to_vec(),
            b"a".to_vec(),
            b"ab".to_vec(),
            b"ss".to_vec(),
            b"ff".to_vec(),
            b"fi".to_vec(),
            b"!".to_vec(),
            b"abc!".to_vec(),
            "aß".as_bytes().to_vec(),
            "aﬃ".as_bytes().to_vec(),
            "a漢".as_bytes().to_vec(),
            vec![b'a', 0xff],
            vec![b'a', 0xc3],
            vec![b's'; 8],
        ];
        for length in [2, 62, 63, 64, 65, 100] {
            texts.push(vec![b'a'; length]);
        }
        for text in texts {
            for retry in [0, 1, 2, 8, 64, 512, 10_000_000] {
                for search in [0, 1, 100] {
                    for stack in [0, 8] {
                        for option in [ONIG_OPTION_NONE, ONIG_OPTION_FIND_LONGEST] {
                            let mut params = onig_new_match_param();
                            params.retry_limit_in_match = retry;
                            params.retry_limit_in_search = search;
                            params.match_stack_limit = stack;
                            params.time_limit = 0;
                            let run = |reg: &RegexType| {
                                let (result, region) = onig_search_with_param(
                                    reg,
                                    &text,
                                    text.len(),
                                    0,
                                    text.len(),
                                    Some(OnigRegion::new()),
                                    option,
                                    &params,
                                );
                                let captures = region.map(|region| {
                                    (
                                        region.beg[..region.num_regs as usize].to_vec(),
                                        region.end[..region.num_regs as usize].to_vec(),
                                    )
                                });
                                (result, captures)
                            };
                            assert_eq!(
                                run(&gated),
                                run(&ungated),
                                "{text:?}, match={retry}, search={search}, stack={stack}, option={option:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ascii_case_insensitive_backrefs_preserve_folded_bytes_and_cursor() {
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for flag in [
                0,
                ONIGENC_CASE_FOLD_MIN,
                ONIGENC_CASE_FOLD_ASCII_ONLY,
                ONIGENC_CASE_FOLD_TURKISH_AZERI,
            ] {
                for c1 in 0u8..0x80 {
                    for c2 in 0u8..0x80 {
                        let data = [c1, c2];
                        let run = |disabled| {
                            ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED.with(|v| v.set(disabled));
                            let mut next = 1;
                            let matched = string_cmp_ic(enc, flag, &data, 0, &mut next, 1);
                            (matched, next)
                        };
                        let actual = run(false);
                        assert_eq!(actual, run(true), "{} {flag} {data:?}", enc.name());
                        let equal = c1.eq_ignore_ascii_case(&c2);
                        assert_eq!(actual, (equal, if equal { 2 } else { 1 }));
                        comparisons += 1;
                    }
                }
            }
        }
        ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED.with(|v| v.set(false));
        println!("{comparisons} ASCII byte-pair result/cursor comparisons");
    }

    #[test]
    fn ascii_case_insensitive_backrefs_preserve_searches_captures_and_limits() {
        let _limits = shared_limits();
        let patterns = [
            r"(?i)\b(\w+)\s+\1\b",
            r"(?i)(.)\1",
            r"(?i)(a?)\1",
            r"(?i)(a|bc)\1",
            r"(?i)(.*)\1",
            r"(?i)(a|b)+\1",
            r"(?i)(?!(.)\1)..",
            r"(?i)(?=(.)\1).",
            r"(?i)(?<n>kk)\k<n>",
            r"(?i)(?:(?<n>kk)|(?<n>ss))\k<n>",
            r"(?i)(a)?\1",
            r"(?i)(?<=(a))\1",
            r"(?i)(a|)\1b",
            r"(?i)((.)\1)*",
            r"(?i)(a)(b)\2\1",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"aa",
            b"aA",
            b"Ab",
            b"bcBC",
            b"aABbA",
            b"The the",
            b"\xff\xffA",
            b"\xc2A",
            b"\xe2\x82A",
            b"\0\0aA",
            "éÉ".as_bytes(),
            "aéAÉ".as_bytes(),
            "kkkK".as_bytes(),
            "ssẞ".as_bytes(),
            "ßß".as_bytes(),
            "ﬀff".as_bytes(),
            "sſ".as_bytes(),
            "Iı".as_bytes(),
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                    ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                ] {
                                    for (retry, search_retry, stack) in [
                                        (0, 0, 0),
                                        (1, 0, 0),
                                        (2, 0, 0),
                                        (0, 1, 0),
                                        (0, 2, 0),
                                        (0, 7, 0),
                                        (0, 0, 1),
                                        (0, 0, 8),
                                    ] {
                                        let mp = OnigMatchParam {
                                            retry_limit_in_match: retry,
                                            retry_limit_in_search: search_retry,
                                            match_stack_limit: stack,
                                            ..onig_new_match_param()
                                        };
                                        for with_region in [false, true] {
                                            let run = |disabled| {
                                                ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED
                                                    .with(|v| v.set(disabled));
                                                let mut msa = MatchArg::from_param(
                                                    &reg,
                                                    option,
                                                    with_region.then(OnigRegion::new),
                                                    start,
                                                    &mp,
                                                );
                                                let result = search_in_range_inner(
                                                    &reg, input, end, start, range, end, &mut msa,
                                                );
                                                (
                                                    result,
                                                    msa.region.map(|r| (r.num_regs, r.beg, r.end)),
                                                    msa.retry_limit_in_search_counter,
                                                )
                                            };
                                            assert_eq!(
                                                run(false),
                                                run(true),
                                                "{} {pattern} {input:?} end={end} start={start} range={range} option={option:?} limits={retry}/{search_retry}/{stack} region={with_region}",
                                                enc.name()
                                            );
                                            ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED
                                                .with(|v| v.set(false));
                                            comparisons += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!(
            "{comparisons} ASCII case-insensitive backref result/capture/retry-counter comparisons"
        );
    }

    #[test]
    fn ascii_case_insensitive_backrefs_preserve_nested_levels() {
        let _limits = shared_limits();
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in [
                r"(?i)(?<n>..)\k<n+0>",
                r"(?i)(?<n>(?:a|b)(?:\g<n>)?)\k<n+0>",
            ] {
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                for input in ["", "aA", "abAB", "aabb", "éÉ", "aéAÉ", "kkkK", "ssẞ", "ßß"]
                {
                    for option in [
                        ONIG_OPTION_NONE,
                        ONIG_OPTION_FIND_LONGEST,
                        ONIG_OPTION_FIND_NOT_EMPTY,
                    ] {
                        for with_region in [false, true] {
                            let run = |disabled| {
                                ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED.with(|v| v.set(disabled));
                                let mut msa = MatchArg::new(
                                    &reg,
                                    option,
                                    with_region.then(OnigRegion::new),
                                    0,
                                );
                                let result = search_in_range_inner(
                                    &reg,
                                    input.as_bytes(),
                                    input.len(),
                                    0,
                                    input.len(),
                                    input.len(),
                                    &mut msa,
                                );
                                (result, msa.region.map(|r| (r.num_regs, r.beg, r.end)))
                            };
                            assert_eq!(run(false), run(true), "{} {pattern} {input:?}", enc.name());
                            ASCII_CASE_INSENSITIVE_BACKREFS_DISABLED.with(|v| v.set(false));
                            comparisons += 1;
                        }
                    }
                }
            }
        }
        println!("{comparisons} nested-level result/capture comparisons");
    }

    #[test]
    fn single_byte_backrefs_preserve_searches_captures_and_limits() {
        let _limits = shared_limits();
        let patterns = [
            r#"(["'])(?:\\.|(?!\1)[^\\\n])*\1"#,
            r"(.)\1",
            r"(a?)\1",
            r"(a|bc)\1",
            r"(.*)\1",
            r"(a|b)+\1",
            r"(?!(.)\1)..",
            r"(?=(.)\1).",
            r#"(?<q>["'])(?:\\.|(?!\k<q>)[^\\\n])*\k<q>"#,
            r"(a)?\1",
            r"(?<=(a))\1",
            r"(a|)\1b",
            r"((.)\1)*",
            r"(a)(b)\2\1",
            r"(?i:(a)\1)",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"aa",
            b"ab",
            b"bcbc",
            b"aabba",
            b"'ab'",
            b"\"a\\\"b\"",
            b"'a\nb'",
            b"'a\r\nb'",
            b"'ab",
            b"\xff\xffa",
            b"\xc2b",
            b"\xe2\x82b",
            b"\0\0ab",
            "ééa".as_bytes(),
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                    ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                ] {
                                    for (retry, search_retry, stack) in [
                                        (0, 0, 0),
                                        (1, 0, 0),
                                        (2, 0, 0),
                                        (0, 1, 0),
                                        (0, 2, 0),
                                        (0, 7, 0),
                                        (0, 0, 1),
                                        (0, 0, 8),
                                    ] {
                                        let mp = OnigMatchParam {
                                            retry_limit_in_match: retry,
                                            retry_limit_in_search: search_retry,
                                            match_stack_limit: stack,
                                            ..onig_new_match_param()
                                        };
                                        for with_region in [false, true] {
                                            let run = |disabled| {
                                                SINGLE_BYTE_BACKREFS_DISABLED
                                                    .with(|v| v.set(disabled));
                                                let mut msa = MatchArg::from_param(
                                                    &reg,
                                                    option,
                                                    with_region.then(OnigRegion::new),
                                                    start,
                                                    &mp,
                                                );
                                                let result = search_in_range_inner(
                                                    &reg, input, end, start, range, end, &mut msa,
                                                );
                                                (
                                                    result,
                                                    msa.region.map(|r| (r.num_regs, r.beg, r.end)),
                                                    msa.retry_limit_in_search_counter,
                                                )
                                            };
                                            assert_eq!(
                                                run(false),
                                                run(true),
                                                "{} {pattern} {input:?} end={end} start={start} range={range} option={option:?} limits={retry}/{search_retry}/{stack} region={with_region}",
                                                enc.name()
                                            );
                                            SINGLE_BYTE_BACKREFS_DISABLED.with(|v| v.set(false));
                                            comparisons += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{comparisons} single-byte backref result/capture/retry-counter comparisons");
    }

    #[test]
    fn dot_star_ascii_cache_preserves_searches_captures_and_limits() {
        let _limits = shared_limits();
        let patterns = [
            r"\A(?=.*[0-9])(?=.*[a-z])(?=.*[A-Z])(?=.*[^0-9A-Za-z\s]).{8,64}\z",
            r"\A(?=.*[ab])(?=.*[cd]).{0,16}\z",
            r"(?=.*[ab])(.*)[cd]",
            r"(.*)(.*)[ab]",
            r"(?m)(.*)[ab]",
            r"(?!(.*)[ab])(.*)",
            r"(?<=a).*[ab]",
            r".*(?:a|bc)",
            r".*",
            r"(?m).*",
            r"(.*)\1",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"ab",
            b"aaabcb",
            b"a\nb",
            b"a\r\nb\n",
            b"1234aA!x",
            b"xx a1A!z b",
            b"ab\xffcd",
            b"\xc2b",
            b"\xe2\x82b",
            b"\x80\x80a",
            b"\0a\0b",
            "éaKb".as_bytes(),
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                    ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                ] {
                                    for (retry, search_retry, stack) in [
                                        (0, 0, 0),
                                        (1, 0, 0),
                                        (2, 0, 0),
                                        (0, 1, 0),
                                        (0, 2, 0),
                                        (0, 7, 0),
                                        (0, 0, 1),
                                        (0, 0, 8),
                                    ] {
                                        let mp = OnigMatchParam {
                                            retry_limit_in_match: retry,
                                            retry_limit_in_search: search_retry,
                                            match_stack_limit: stack,
                                            ..onig_new_match_param()
                                        };
                                        for with_region in [false, true] {
                                            let run = |disabled| {
                                                DOT_STAR_ASCII_CACHE_DISABLED
                                                    .with(|v| v.set(disabled));
                                                let mut msa = MatchArg::from_param(
                                                    &reg,
                                                    option,
                                                    with_region.then(OnigRegion::new),
                                                    start,
                                                    &mp,
                                                );
                                                let result = search_in_range_inner(
                                                    &reg, input, end, start, range, end, &mut msa,
                                                );
                                                (
                                                    result,
                                                    msa.region.map(|r| (r.num_regs, r.beg, r.end)),
                                                    msa.retry_limit_in_search_counter,
                                                )
                                            };
                                            assert_eq!(
                                                run(false),
                                                run(true),
                                                "{} {pattern} {input:?} end={end} start={start} range={range} option={option:?} limits={retry}/{search_retry}/{stack} region={with_region}",
                                                enc.name()
                                            );
                                            DOT_STAR_ASCII_CACHE_DISABLED.with(|v| v.set(false));
                                            comparisons += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{comparisons} dot-star result/capture/retry-counter comparisons");
    }

    #[test]
    fn captured_map_continuation_preserves_searches_captures_and_limits() {
        let _limits = shared_limits();
        let utf8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
        let ascii: OnigEncoding = &crate::encodings::ascii::ONIG_ENCODING_ASCII;
        let diagnostic = compile_full(r#"\b([a-z-]+)=(?:"[^"]*+"|'[^']*+'|[^\s>]++)"#.as_bytes());
        let text = b"letters words text data-kind='x' more text class=valid";
        let count = |disabled| {
            CAPTURE_MAP_SCANS_DISABLED.with(|v| v.set(disabled));
            VM_ATTEMPTS.with(|v| v.set(0));
            let result = onig_search(
                &diagnostic,
                text,
                text.len(),
                0,
                text.len(),
                Some(OnigRegion::new()),
                ONIG_OPTION_NONE,
            );
            (result.0, VM_ATTEMPTS.with(Cell::get))
        };
        let reference = count(true);
        let continued = count(false);
        assert_eq!(continued.0, reference.0);
        assert!(continued.1 < reference.1, "{continued:?} vs {reference:?}");

        let patterns = [
            r#"\b([a-z-]+)=(?:"[^"]*+"|'[^']*+'|[^\s>]++)"#,
            r"\b([a-z-]+)=",
            r"([a-z]+)x",
            r"([a-z]*)z?",
            r"((?:[a-f]+|[0-9]+))=",
            r"(\b([a-z]+))=",
            r"(?i)([a-z]+)=",
            r"([éa-z]+)=",
            r"([^\n]+)=",
            r"^([a-z]+)=",
            r"\A([a-z]+)=",
            r"(\G[a-z]+)=",
            r"((?<=a)[a-z]+)=",
            r"(?<=a{0,2})([a-z]+)=",
            r"([a-z]+)=.*",
            r"([a-z])\1=",
            r"(alpha|beta|gamma|delta)=",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b" a ",
            b"ab=",
            b"word first=one x second='two'",
            b"a=1\n ab=23\n A=X",
            b"a=\"unterminated",
            b"aa=a ab=x",
            b"\xc2a=1 ab=2",
            b"ab\xffc=2",
            b"x\xe2\x82=3",
            b"\x80\x80aa=1",
            b"a=\0a9",
            "éa=2 K=x".as_bytes(),
        ];
        let mut comparisons = 0;
        for enc in [utf8, ascii] {
            for pattern in patterns {
                let Ok(reg) = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                ) else {
                    continue;
                };
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                    ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                ] {
                                    for (retry, search_retry, stack) in [
                                        (0, 0, 0),
                                        (1, 0, 0),
                                        (2, 0, 0),
                                        (0, 1, 0),
                                        (0, 2, 0),
                                        (0, 7, 0),
                                        (0, 0, 1),
                                        (0, 0, 8),
                                    ] {
                                        let mp = OnigMatchParam {
                                            retry_limit_in_match: retry,
                                            retry_limit_in_search: search_retry,
                                            match_stack_limit: stack,
                                            ..onig_new_match_param()
                                        };
                                        let run = |disabled| {
                                            CAPTURE_MAP_SCANS_DISABLED.with(|v| v.set(disabled));
                                            let mut msa = MatchArg::from_param(
                                                &reg,
                                                option,
                                                Some(OnigRegion::new()),
                                                start,
                                                &mp,
                                            );
                                            let result = search_in_range_inner(
                                                &reg, input, end, start, range, end, &mut msa,
                                            );
                                            (
                                                result,
                                                msa.region.map(|r| (r.num_regs, r.beg, r.end)),
                                                msa.retry_limit_in_search_counter,
                                            )
                                        };
                                        let expected = run(true);
                                        let actual = run(false);
                                        assert_eq!(
                                            actual,
                                            expected,
                                            "{} {pattern} {input:?} end={end} start={start} range={range} option={option:?} limits={retry}/{search_retry}/{stack}",
                                            enc.name()
                                        );
                                        comparisons += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{comparisons} captured-map result/capture/retry-counter comparisons");
    }

    fn character_walk(
        enc: OnigEncoding,
        start: usize,
        mut at: usize,
        text: &[u8],
        count: usize,
    ) -> Option<usize> {
        for _ in 0..count {
            if at <= start {
                return None;
            }
            at = enc.left_adjust_char_head(start, at - 1, text);
        }
        Some(at)
    }

    #[test]
    fn ascii_batches_preserve_every_backward_character_boundary() {
        use crate::encodings::{ONIG_ENCODING_ASCII, ONIG_ENCODING_UTF8};
        let inputs: &[&[u8]] = &[
            b"",
            b"a",
            b"0123456789abcdefghijklmnopqrstuv",
            "éabc €abcdef 👩🏽‍💻abcdefghi".as_bytes(),
            b"\0abcdefghi\x7f",
            b"\x80abcdefgh",
            b"\xc2abcdefgh",
            b"ab\xe2\x82cdefgh",
            b"abcdef\xffghijkl",
            b"\xf5\x80\x80\x80abcdefgh",
            b"\x80\x80\x80\x80abcdef",
        ];
        let mut comparisons = 0;
        for enc in [
            &ONIG_ENCODING_UTF8 as OnigEncoding,
            &ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for &text in inputs {
                for start in 0..=text.len() {
                    for at in start..=text.len() {
                        for count in (0..=18).chain([31, 32, 64]) {
                            assert_eq!(
                                step_back_look_behind(enc, start, at, text, count),
                                character_walk(enc, start, at, text, count),
                                "{} start={start} at={at} count={count} text={text:?}",
                                enc.name()
                            );
                            comparisons += 1;
                        }
                    }
                }
            }
        }
        println!("{comparisons} backward character-boundary comparisons");
    }

    #[test]
    fn fused_ascii_look_behinds_preserve_body_endpoints() {
        let _limits = shared_limits();
        let bodies: Vec<_> = (1..=18)
            .map(|n| "a".repeat(n))
            .chain(["[a-z]", "[^a]", "[aé]", "[^é]"].map(String::from))
            .collect();
        let inputs: &[&[u8]] = &[
            b"",
            b"a",
            b"aaaaaaaaaaaaaaaaaaaab",
            b"\0aaaaaaaaaaaaaaaa\x7f",
            "éaaaaaaaaaaaaaaaaaéb".as_bytes(),
            b"\x80aaaaaaaaaaaaaaaaa",
            b"\xc2aaaaaaaaaaaaaaaaa",
            b"aaaa\xe2\x82aaaaaaaaaaaa",
            b"\xf5\x80\x80\x80aaaaaaaaaaaaaaaaa",
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for body in &bodies {
                let reg = regcomp::onig_new(
                    format!("(?<={body})").as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                let (index, char_len) = reg
                    .ops
                    .iter()
                    .enumerate()
                    .find_map(|(i, op)| match op.payload {
                        OperationPayload::LookBehindOp { char_len, .. } => Some((i, char_len)),
                        _ => None,
                    })
                    .expect("one-instruction look-behind body");
                let op = &reg.ops[index + 1];
                for &text in inputs {
                    for end in 0..=text.len() {
                        for at in 0..=end {
                            for right_range in [at, end] {
                                let expected = character_walk(enc, 0, at, text, char_len as usize)
                                    .and_then(|start| {
                                        single_op_end(op, enc, text, start, right_range, end)
                                    });
                                assert_eq!(
                                    look_behind_body_end(
                                        op,
                                        char_len,
                                        enc,
                                        text,
                                        at,
                                        right_range,
                                        end,
                                        onigenc_is_ascii_compatible_encoding(enc)
                                    ),
                                    expected,
                                    "{} {body} {text:?} end={end} at={at} right_range={right_range}",
                                    enc.name()
                                );
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
        println!("{comparisons} fused look-behind body-endpoint comparisons");
    }

    #[test]
    fn fused_ascii_look_behinds_do_not_start_on_ascii_valued_trail_bytes() {
        use crate::regenc::Encoding;

        // A small ASCII-compatible encoding whose 0x81 lead consumes an
        // ASCII-valued trail byte. Other operations delegate to UTF-8;
        // this test exercises only backward heads and literal bodies.
        struct TrailEncoding;
        impl Encoding for TrailEncoding {
            fn mbc_enc_len(&self, p: &[u8]) -> usize {
                if p.first() == Some(&0x81) { 2 } else { 1 }
            }
            fn name(&self) -> &str {
                "ASCII-trail test encoding"
            }
            fn max_enc_len(&self) -> usize {
                2
            }
            fn min_enc_len(&self) -> usize {
                1
            }
            fn is_mbc_newline(&self, p: &[u8], end: usize) -> bool {
                crate::encodings::ONIG_ENCODING_UTF8.is_mbc_newline(p, end)
            }
            fn mbc_to_code(&self, p: &[u8], end: usize) -> OnigCodePoint {
                crate::encodings::ONIG_ENCODING_UTF8.mbc_to_code(p, end)
            }
            fn code_to_mbclen(&self, code: OnigCodePoint) -> i32 {
                crate::encodings::ONIG_ENCODING_UTF8.code_to_mbclen(code)
            }
            fn code_to_mbc(&self, code: OnigCodePoint, buf: &mut [u8]) -> i32 {
                crate::encodings::ONIG_ENCODING_UTF8.code_to_mbc(code, buf)
            }
            fn mbc_case_fold(
                &self,
                flag: OnigCaseFoldType,
                pp: &mut usize,
                end: usize,
                source: &[u8],
                buf: &mut [u8],
            ) -> i32 {
                crate::encodings::ONIG_ENCODING_UTF8.mbc_case_fold(flag, pp, end, source, buf)
            }
            fn apply_all_case_fold(
                &self,
                flag: OnigCaseFoldType,
                f: &mut dyn FnMut(OnigCodePoint, &[OnigCodePoint]) -> i32,
            ) -> i32 {
                crate::encodings::ONIG_ENCODING_UTF8.apply_all_case_fold(flag, f)
            }
            fn get_case_fold_codes_by_str(
                &self,
                flag: OnigCaseFoldType,
                p: &[u8],
                end: usize,
                items: &mut [OnigCaseFoldCodeItem],
            ) -> i32 {
                crate::encodings::ONIG_ENCODING_UTF8.get_case_fold_codes_by_str(flag, p, end, items)
            }
            fn property_name_to_ctype(&self, p: &[u8]) -> i32 {
                crate::encodings::ONIG_ENCODING_UTF8.property_name_to_ctype(p)
            }
            fn is_code_ctype(&self, code: OnigCodePoint, ctype: u32) -> bool {
                crate::encodings::ONIG_ENCODING_UTF8.is_code_ctype(code, ctype)
            }
            fn get_ctype_code_range(
                &self,
                ctype: u32,
                sb_out: &mut OnigCodePoint,
            ) -> Option<&'static [OnigCodePoint]> {
                crate::encodings::ONIG_ENCODING_UTF8.get_ctype_code_range(ctype, sb_out)
            }
            fn left_adjust_char_head(&self, start: usize, at: usize, data: &[u8]) -> usize {
                if at > start && data[at - 1] == 0x81 {
                    at - 1
                } else {
                    at
                }
            }
            fn is_allowed_reverse_match(&self, p: &[u8]) -> bool {
                crate::encodings::ONIG_ENCODING_UTF8.is_allowed_reverse_match(p)
            }
            fn is_valid_mbc_string(&self, s: &[u8]) -> bool {
                crate::encodings::ONIG_ENCODING_UTF8.is_valid_mbc_string(s)
            }
            fn flag(&self) -> u32 {
                crate::regenc::ENC_FLAG_ASCII_COMPATIBLE
            }
        }
        static ENCODING: TrailEncoding = TrailEncoding;
        let _limits = shared_limits();
        let mut comparisons = 0;
        for width in 2..=18 {
            let body = "a".repeat(width);
            let reg = regcomp::onig_new(
                format!("(?<={body})").as_bytes(),
                ONIG_OPTION_NONE,
                &crate::encodings::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap();
            let (index, char_len) = reg
                .ops
                .iter()
                .enumerate()
                .find_map(|(i, op)| match op.payload {
                    OperationPayload::LookBehindOp { char_len, .. } => Some((i, char_len)),
                    _ => None,
                })
                .unwrap();
            for text in [
                [vec![0x81], vec![b'a'; 20]].concat(),
                [vec![b'a', 0x81, b'a'], vec![b'a'; 20]].concat(),
            ] {
                // The ASCII-looking suffix begins on the trail byte. The
                // true character walk lands one byte earlier than subtraction.
                let from = text.iter().position(|&b| b == 0x81).unwrap() + 1;
                let at = from + width;
                assert_ne!(character_walk(&ENCODING, 0, at, &text, width), Some(from));
                for end in at..=text.len() {
                    let expected =
                        character_walk(&ENCODING, 0, at, &text, width).and_then(|start| {
                            single_op_end(&reg.ops[index + 1], &ENCODING, &text, start, end, end)
                        });
                    assert_eq!(
                        look_behind_body_end(
                            &reg.ops[index + 1],
                            char_len,
                            &ENCODING,
                            &text,
                            at,
                            end,
                            end,
                            true
                        ),
                        expected,
                        "width={width} end={end} {text:?}"
                    );
                    comparisons += 1;
                }
            }
        }
        println!("{comparisons} ASCII-valued trail-byte comparisons");
    }

    #[test]
    fn fused_ascii_look_behinds_preserve_searches_captures_and_limits() {
        let _limits = shared_limits();
        let inputs: &[&[u8]] = &[
            b"",
            b"ab",
            b"aaaaaaaaaaaaaaaaab",
            b"xaaaaaaaaab aaaaaaaab",
            b"@charset x @charsetb",
            "éaaaaaaaab éaaaaaaaaab".as_bytes(),
            b"\xc2aaaaaaaaab",
            b"\x80aaaaaaaab",
            b"aa\xe2\x82aaaaaaab",
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for body in [1, 2, 8, 9, 16, 17]
                .map(|n| "a".repeat(n))
                .into_iter()
                .chain(["@charset".to_string(), "@charset ".to_string()])
            {
                for not in ["", "!"] {
                    let pattern = if not.is_empty() {
                        format!("(?<={body})(b?)|x")
                    } else {
                        format!("(?<!{body})(b?)|x")
                    };
                    let reg = regcomp::onig_new(
                        pattern.as_bytes(),
                        ONIG_OPTION_NONE,
                        enc,
                        &crate::regsyntax::OnigSyntaxOniguruma,
                    )
                    .unwrap();
                    assert!(reg.ops.iter().any(|op| op.opcode == OpCode::LookBehindOp));
                    for &input in inputs {
                        for end in [0, input.len().saturating_sub(1), input.len()] {
                            for start in 0..=end {
                                for range in [start, end] {
                                    for option in [
                                        ONIG_OPTION_NONE,
                                        ONIG_OPTION_FIND_LONGEST,
                                        ONIG_OPTION_FIND_NOT_EMPTY,
                                        ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                    ] {
                                        for (retry, search_retry, stack, time) in [
                                            (0, 0, 0, 0),
                                            (1, 0, 0, 0),
                                            (2, 0, 0, 0),
                                            (0, 1, 0, 0),
                                            (0, 9, 0, 0),
                                            (0, 0, 1, 0),
                                            (0, 0, 8, 0),
                                            (0, 0, 0, 1),
                                        ] {
                                            for with_region in [false, true] {
                                                let run = |disabled| {
                                                    FUSED_LOOK_BEHIND_ASCII_BATCHES_DISABLED
                                                        .with(|v| v.set(disabled));
                                                    let mp = OnigMatchParam {
                                                        retry_limit_in_match: retry,
                                                        retry_limit_in_search: search_retry,
                                                        match_stack_limit: stack,
                                                        time_limit: time,
                                                        ..onig_new_match_param()
                                                    };
                                                    let mut msa = MatchArg::from_param(
                                                        &reg,
                                                        option,
                                                        with_region.then(OnigRegion::new),
                                                        start,
                                                        &mp,
                                                    );
                                                    if time != 0 {
                                                        // Deterministic deadline; never compare wall-clock races.
                                                        msa.time_end = Some(
                                                            Instant::now()
                                                                - std::time::Duration::from_secs(1),
                                                        );
                                                        msa.time_counter = CHECK_TIME_INTERVAL - 1;
                                                    }
                                                    let result = search_in_range_inner(
                                                        &reg, input, end, start, range, end,
                                                        &mut msa,
                                                    );
                                                    (
                                                        result,
                                                        msa.region
                                                            .map(|r| (r.num_regs, r.beg, r.end)),
                                                        msa.retry_limit_in_search_counter,
                                                        msa.time_counter,
                                                    )
                                                };
                                                let expected = run(true);
                                                let actual = run(false);
                                                FUSED_LOOK_BEHIND_ASCII_BATCHES_DISABLED
                                                    .with(|v| v.set(false));
                                                assert_eq!(
                                                    actual,
                                                    expected,
                                                    "{} {pattern} {input:?} end={end} start={start} range={range} option={option:?} limits={retry}/{search_retry}/{stack}/{time} region={with_region}",
                                                    enc.name()
                                                );
                                                comparisons += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{comparisons} fused look-behind result/capture/retry/time-counter comparisons");
    }

    #[test]
    fn bounded_optimizer_windows_preserve_search_results_and_limits() {
        let _limits = shared_limits();

        let diagnostic_reg = regcomp::onig_new(
            r"(?<=\b(?:price|total):\s{0,3})[$€£]?[0-9]+(?:[.,][0-9]{2})?".as_bytes(),
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &crate::regsyntax::OnigSyntaxOniguruma,
        )
        .unwrap();
        let text = b"other:12 other:34 other:56 price: $78";
        let count_entries = |disabled| {
            OPTIMIZER_WINDOW_SCANS_DISABLED.with(|v| v.set(disabled));
            VM_ATTEMPTS.with(|v| v.set(0));
            let result = onig_search(
                &diagnostic_reg,
                text,
                text.len(),
                0,
                text.len(),
                Some(OnigRegion::new()),
                ONIG_OPTION_NONE,
            );
            (result.0, VM_ATTEMPTS.with(std::cell::Cell::get))
        };
        let reference_entries = count_entries(true);
        let scan_entries = count_entries(false);
        assert_eq!(scan_entries.0, reference_entries.0);
        assert!(
            scan_entries.1 < reference_entries.1,
            "{scan_entries:?} vs {reference_entries:?}"
        );
        let patterns = [
            r"(?<=\b(?:price|total):\s{0,3})[$]?[0-9]+(?:[.,][0-9]{2})?",
            r"(?<=a{1,3})b?c[0-9]",
            r"(?<!a{1,3})b?c[0-9]",
            r"(?:a|bc)?[0-9]{2}",
            r"(?<=a|bc).?z",
            r"a{0,3}z",
            r"\b(?:a|b)?[0-9]",
            r"(a?)[0-9]",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"1",
            b"a1bc22",
            b"aaabc1bc2",
            b"price: $12",
            b"total:   34",
            b"xx:12 price:56",
            b"aazbcz",
            b"\xc2a1bc2",
            b"a\xff1bc2",
            b"ab\xe2\x82z1",
            b"\xc2\xa1a12",
            b"ab\0a9",
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                ] {
                                    for (retry, search_retry, stack) in [
                                        (0, 0, 0),
                                        (1, 0, 0),
                                        (2, 0, 0),
                                        (0, 1, 0),
                                        (0, 0, 1),
                                        (0, 0, 8),
                                    ] {
                                        let mut mp = onig_new_match_param();
                                        mp.retry_limit_in_match = retry;
                                        mp.retry_limit_in_search = search_retry;
                                        mp.match_stack_limit = stack;
                                        let search = |disabled| {
                                            OPTIMIZER_WINDOW_SCANS_DISABLED
                                                .with(|v| v.set(disabled));
                                            let (pos, region) = onig_search_with_param(
                                                &reg,
                                                input,
                                                end,
                                                start,
                                                range,
                                                Some(OnigRegion::new()),
                                                option,
                                                &mp,
                                            );
                                            (pos, region.map(|r| (r.num_regs, r.beg, r.end)))
                                        };
                                        let expected = search(true);
                                        let actual = search(false);
                                        assert_eq!(
                                            actual,
                                            expected,
                                            "{} {pattern} {input:?} end={end} start={start} range={range} option={option:?} limits={retry}/{search_retry}/{stack}",
                                            enc.name()
                                        );
                                        comparisons += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{comparisons} bounded optimizer-window comparisons");
    }
    use super::*;
    use crate::regcomp;
    use crate::regparse;
    use crate::regparse_types::ParseEnv;

    /// The chunked map search finds what the character loop finds, for maps
    /// with ASCII ranges and high bytes, over ASCII, UTF-8 and malformed
    /// input, from every start and to every range.
    #[test]
    fn chunked_map_search_matches_the_character_loop() {
        let utf8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let pieces: &[&[u8]] = &[
            b"a",
            b"Z",
            b"0",
            b"9",
            b" ",
            b"-",
            b"_",
            b"@",
            b".",
            b"~",
            b"\x7f",
            b"\x00",
            "\u{e9}".as_bytes(),
            "\u{20ac}".as_bytes(),
            b"\xff",
            b"\xc3",
            b"\xe2\x82",
        ];
        let mut searched = 0;
        for pattern in [
            r"\d+",
            r"\w+",
            r"[a-z]+",
            r"[\w.+-]+@",
            r"[0-9A-F_~]+x",
            r"[\x00\x7f ]+y",
            r"[a-c]\w",
            r"(?i)[a-f]+-",
        ] {
            let reg = crate::regcomp::onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_NONE,
                utf8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap();
            if reg.optimize != OptimizeType::Map || !reg.map_ascii_ranges.usable() {
                continue;
            }
            for _ in 0..300 {
                let text: Vec<u8> = (0..next(40))
                    .flat_map(|_| pieces[next(pieces.len())].to_vec())
                    .collect();
                for start in 0..=text.len() {
                    for range in start..=text.len() {
                        let mut expected = None;
                        let mut s = start;
                        while s < range {
                            if reg.map[text[s] as usize] != 0 {
                                expected = Some(s);
                                break;
                            }
                            s += enclen(utf8, &text, s);
                        }
                        assert_eq!(
                            map_search_ascii_chunks(utf8, &reg, &text, start, range),
                            expected,
                            "{pattern} {text:?} {start} {range}"
                        );
                        searched += 1;
                    }
                }
            }
        }
        assert!(searched > 100_000, "{searched}");
    }

    /// The fused lazy `.*?c` loop gives the results, captures and limit
    /// errors of the loop run one instruction at a time.
    #[test]
    fn lazy_any_char_stop_matches_the_instruction_loop() {
        let _limits = shared_limits();
        let utf8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
        let ascii: OnigEncoding = &crate::encodings::ascii::ONIG_ENCODING_ASCII;
        let patterns = [
            r"\[.*?\]",
            r#"".*?""#,
            r"<.*?>",
            r#"(".*?")x"#,
            r"(?m)<.*?>",
            r"a.*?b.*?c",
            r"(\w).*?\1",
            r".*?x",
            r"(?:.*?,){2}",
            r"x.*?\n",
            r"\[(.*?)\](.*?);",
            r"(?m)\[.*?\](.*?)x",
        ];
        let pieces: &[&[u8]] = &[
            b"[",
            b"]",
            b"\"",
            b"<",
            b">",
            b"x",
            b",",
            b";",
            b"a",
            b"b",
            b"c",
            b"\n",
            b" ",
            "\u{e9}".as_bytes(),
            "\u{20ac}".as_bytes(),
            b"\xff",
            b"\xe2\x82",
            b"\xc3",
        ];
        let mut seed: u64 = 0x5851_F42D_4C95_7F2D;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let run = |reg: &RegexType, text: &[u8], fused: bool, limits: (u64, u64, u32)| {
            LAZY_STOP_DISABLED.with(|disabled| disabled.set(!fused));
            let mut mp = onig_new_match_param();
            (
                mp.retry_limit_in_match,
                mp.retry_limit_in_search,
                mp.match_stack_limit,
            ) = limits;
            let mut out = Vec::new();
            let mut start = 0;
            while start <= text.len() {
                let (r, region) = onig_search_with_param(
                    reg,
                    text,
                    text.len(),
                    start,
                    text.len(),
                    Some(OnigRegion::new()),
                    ONIG_OPTION_NONE,
                    &mp,
                );
                let region = region.unwrap();
                out.push((r, region.beg.clone(), region.end.clone()));
                if r < 0 {
                    break;
                }
                let e = region.end[0] as usize;
                start = if e > r as usize { e } else { r as usize + 1 };
            }
            LAZY_STOP_DISABLED.with(|disabled| disabled.set(false));
            out
        };
        let mut compared = 0;
        for enc in [utf8, ascii] {
            for pattern in patterns {
                let reg = crate::regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                for _ in 0..150 {
                    let text: Vec<u8> = (0..next(50))
                        .flat_map(|_| pieces[next(pieces.len())].to_vec())
                        .collect();
                    for limits in [
                        (0, 0, 0),
                        (1, 0, 0),
                        (3, 0, 0),
                        (0, 5, 0),
                        (7, 5, 0),
                        (4, 10, 0),
                        (0, 0, 2),
                    ] {
                        assert_eq!(
                            run(&reg, &text, true, limits),
                            run(&reg, &text, false, limits),
                            "{} {pattern:?} {text:?} {limits:?}",
                            enc.name()
                        );
                        compared += 1;
                    }
                }
            }
        }
        assert!(compared > 20_000, "{compared}");
    }

    /// `PushLazyAnyChar` with a literal seek (across capture ends,
    /// negative one-character look-behinds and back references to literal
    /// groups) gives the results and captures of the loop with the seek off,
    /// and of a program compiled without push guards. Under limits (ADR-017)
    /// the seek may leave out retries, so it finishes where the loop with
    /// the seek off finishes, with the unlimited result, and it never stops
    /// where the unlimited search finishes.
    #[test]
    fn lazy_literal_seek_matches_the_instruction_loop() {
        let _limits = shared_limits();
        let utf8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
        let ascii: OnigEncoding = &crate::encodings::ascii::ONIG_ENCODING_ASCII;
        // (pattern, whether the seek is planned)
        let patterns = [
            // The Python-Markdown emphasis patterns: literal group `_`.
            (r"(\_)\1(?!\1)(.+?)(?<!\w)\1(?!\1)(.+?)\1{3}(?!\w)", true),
            (r"(?<!\w)(_)(?!_)(.+?)(?<!_)\1(?!\w)", true),
            (r"(a)(.*?)(?<!\w)a", true),
            (r"(a)(.*?)(?<!b)\1", true),
            (r"(a)(.*?)x(?<!\w)\1", true),
            (r"(a)(.*?)\1", true),
            (r"(ab)(.*?)(?<!\w)\1", true),
            (r"(abcdef)(.*?)(?<!b)\1", true),
            (r"(?m)(.*?)(?<!\w)\n", true),
            (r"(\n)(.*?)(?<!\w)\1", true),
            (r"(é)(.*?)(?<!\w)\1", true),
            (r"(a)(.*?)(?<!\w)\2", false),
            // Not planned: case-insensitive groups and back references,
            // a positive look-behind, empty, optional or alternative groups,
            // a back reference to the loop's own group, a group that is not
            // a literal, and a look-behind that is not one character.
            (r"(?i)(a)(.*?)(?<!\w)\1", false),
            (r"(a)(.*?)(?<!\w)(?i)\1", false),
            (r"(a)(.*?)(?<=\w)\1", false),
            (r"(|a)(.*?)(?<!\w)\1", false),
            (r"()(.*?)(?<!\w)\1", false),
            (r"(?:(a)|b)(.*?)(?<!\w)\1", false),
            (r"(a)?(.*?)(?<!\w)\1", false),
            (r"(.*?)(?<!\w)\1", false),
            (r"(a)(.*?)(?<!\w)(b)\3", false),
            (r"(a)(.*?)(?<!\w)(?:a|b)", false),
            // A capture end after the look-behind: the seek needs the
            // capture ends before it, so that every position writes them.
            (r"(_)((.*?)(?<!\w))\1", false),
            // A read before the group: the group may not be set there.
            (r"(?:\1)?(_)(.*?)(?<!\w)\1", false),
        ];
        let edges: &[&[u8]] = &[
            b"",
            b"_",
            b"__",
            b"_a_",
            b"a_b_c_",
            b"a\n_",
            "é_é_".as_bytes(),
            "_é_".as_bytes(),
            b"x_\n_x",
            b"\xff_",
            b"_\xc3",
            b"____",
            b"\n",
            b"_\n",
            b"a\n\na_a_",
            "aé_".as_bytes(),
            b"_\xe2\x82",
            b"a_\xff_a",
            b"aaaa",
            b"aXa",
            b"abab\n",
            b"a\nb\na",
            b"aa b a\xc3",
        ];
        let pieces: &[&[u8]] = &[
            b"_",
            b"a",
            b"b",
            b"x",
            b"\n",
            b" ",
            "é".as_bytes(),
            b"\xff",
            b"\xc3",
            b"\xe2\x82",
            b"1",
        ];
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let search_all = |reg: &RegexType, text: &[u8], seek: bool, limits: (u64, u64, u32)| {
            LAZY_STOP_DISABLED.with(|disabled| disabled.set(!seek));
            let mut mp = onig_new_match_param();
            (
                mp.retry_limit_in_match,
                mp.retry_limit_in_search,
                mp.match_stack_limit,
            ) = limits;
            let mut out = Vec::new();
            let mut start = 0;
            while start <= text.len() {
                let (r, region) = onig_search_with_param(
                    reg,
                    text,
                    text.len(),
                    start,
                    text.len(),
                    Some(OnigRegion::new()),
                    ONIG_OPTION_NONE,
                    &mp,
                );
                let region = region.unwrap();
                out.push((r, region.beg.clone(), region.end.clone()));
                if r < 0 {
                    break;
                }
                let e = region.end[0] as usize;
                start = if e > r as usize { e } else { r as usize + 1 };
            }
            LAZY_STOP_DISABLED.with(|disabled| disabled.set(false));
            out
        };
        let is_limit_error = |out: &[(i32, Vec<i32>, Vec<i32>)]| {
            out.last().is_some_and(|(r, _, _)| {
                matches!(
                    *r,
                    ONIGERR_MATCH_STACK_LIMIT_OVER
                        | ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
                        | ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER
                )
            })
        };
        let limit_sets = [
            (1, 0, 0),
            (2, 0, 0),
            (4, 0, 0),
            (9, 0, 0),
            (0, 3, 0),
            (0, 0, 1),
            (0, 0, 3),
        ];
        let mut compared = 0;
        let mut planned_patterns = 0;
        for enc in [utf8, ascii] {
            for (pattern, planned) in patterns {
                let reg = crate::regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                // The reference: the same pattern compiled without push guards.
                crate::regcomp::PUSH_GUARDS_DISABLED.with(|disabled| disabled.set(true));
                let plain = crate::regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                crate::regcomp::PUSH_GUARDS_DISABLED.with(|disabled| disabled.set(false));
                assert_eq!(
                    has_opcode(&reg, OpCode::PushLazyAnyChar),
                    planned,
                    "{} {pattern:?}",
                    enc.name()
                );
                planned_patterns += usize::from(planned);
                let mut subjects: Vec<Vec<u8>> = edges.iter().map(|e| e.to_vec()).collect();
                for _ in 0..90 {
                    subjects.push(
                        (0..next(24))
                            .flat_map(|_| pieces[next(pieces.len())].to_vec())
                            .collect(),
                    );
                }
                for text in &subjects {
                    let unlimited = search_all(&plain, text, false, (0, 0, 0));
                    assert_eq!(
                        search_all(&reg, text, true, (0, 0, 0)),
                        unlimited,
                        "{} {pattern:?} {text:?} unlimited",
                        enc.name()
                    );
                    for limits in limit_sets {
                        let slow = search_all(&reg, text, false, limits);
                        let fast = search_all(&reg, text, true, limits);
                        if !is_limit_error(&slow) {
                            assert_eq!(
                                fast,
                                unlimited,
                                "{} {pattern:?} {text:?} {limits:?}",
                                enc.name()
                            );
                        }
                        if !is_limit_error(&fast) {
                            assert_eq!(
                                fast,
                                unlimited,
                                "{} {pattern:?} {text:?} {limits:?}",
                                enc.name()
                            );
                        }
                        compared += 1;
                    }
                }
            }
        }
        assert!(planned_patterns >= 14, "{planned_patterns}");
        assert!(compared > 25_000, "{compared}");
    }

    #[test]
    fn captured_lazy_dots_preserve_searches_captures_and_limits() {
        let _limits = shared_limits();
        let patterns = [
            r"<([a-z][a-z0-9]*)\b[^>]*>(.*?)</\1>",
            r"(.*?)</",
            r"(?m)(.*?)</",
            r"(.*?)xyz",
            r"(.*?)abcdef",
            r"(.*?)\n",
            r"(.*?)é",
            r"(?=(.*?)xx)",
            r"(?:(.*?)</x|(.))+\2",
            r"(?:((.*?)</)a|b\2)",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"x",
            b"</",
            b"abc</",
            b"<a>x</a>",
            b"<a><b>x</b></a>",
            b"<a>x\n</a>",
            b"<a>x</b>",
            b"x</x</a",
            b"abcxyzabcdef",
            b"abc\n</",
            b"abc\nxyz",
            b"abcxx",
            b"abc\0</",
            b"a\xff</",
            b"a\xc3</",
            b"a\xe2\x82</",
            "aé€</".as_bytes(),
            "aéxyz".as_bytes(),
        ];
        let mut comparisons = 0;
        let mut planned = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                planned += usize::from(has_opcode(&reg, OpCode::PushLazyAnyChar));
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                    ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                ] {
                                    for (retry, search_retry, stack, time) in [
                                        (0, 0, 0, 0),
                                        (1, 0, 0, 0),
                                        (3, 0, 0, 0),
                                        (0, 1, 0, 0),
                                        (0, 7, 0, 0),
                                        (4, 10, 0, 0),
                                        (0, 0, 1, 0),
                                        (0, 0, 3, 0),
                                        (0, 0, 8, 0),
                                        (0, 0, 0, 1000),
                                    ] {
                                        let mp = OnigMatchParam {
                                            retry_limit_in_match: retry,
                                            retry_limit_in_search: search_retry,
                                            match_stack_limit: stack,
                                            time_limit: time,
                                            ..onig_new_match_param()
                                        };
                                        for with_region in [false, true] {
                                            let run = |disabled| {
                                                LAZY_STOP_DISABLED.with(|v| v.set(disabled));
                                                let mut msa = MatchArg::from_param(
                                                    &reg,
                                                    option,
                                                    with_region.then(OnigRegion::new),
                                                    start,
                                                    &mp,
                                                );
                                                let result = search_in_range_inner(
                                                    &reg, input, end, start, range, end, &mut msa,
                                                );
                                                (
                                                    result,
                                                    msa.region.map(|r| (r.num_regs, r.beg, r.end)),
                                                    msa.retry_limit_in_search_counter,
                                                    msa.time_counter,
                                                )
                                            };
                                            assert_eq!(
                                                run(false),
                                                run(true),
                                                "{} {pattern} {input:?} end={end} start={start} range={range} option={option:?} limits={retry}/{search_retry}/{stack}/{time} region={with_region}",
                                                enc.name()
                                            );
                                            LAZY_STOP_DISABLED.with(|v| v.set(false));
                                            comparisons += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(planned >= 12, "{planned} planned captured lazy loops");
        println!("{comparisons} captured-lazy result/capture/retry/time comparisons");
    }

    #[test]
    fn captured_lazy_dots_preserve_long_and_malformed_subjects() {
        let _limits = shared_limits();
        let mut comparisons = 0;
        for pattern in [r"(.*?)</", r"(?m)(.*?)</", r"(?:((.*?)</)x|a\2)"] {
            let reg = compile_regex(pattern.as_bytes());
            for len in [4094, 4095, 4096, 4097, 8192] {
                for tail in [
                    b"</".as_slice(),
                    b"",
                    b"\n</",
                    "é</".as_bytes(),
                    b"\xc3</",
                    b"\xff",
                ] {
                    let mut text = vec![b'a'; len];
                    text.extend_from_slice(tail);
                    for option in [ONIG_OPTION_NONE, ONIG_OPTION_FIND_LONGEST] {
                        for (retry, search_retry, stack) in [
                            (0, 0, 0),
                            (len as u64, 0, 0),
                            (len as u64 + 1, 0, 0),
                            (0, len as u64 + 2, 0),
                            (0, 0, 2),
                        ] {
                            let mp = OnigMatchParam {
                                retry_limit_in_match: retry,
                                retry_limit_in_search: search_retry,
                                match_stack_limit: stack,
                                time_limit: 0,
                                ..onig_new_match_param()
                            };
                            let run = |disabled| {
                                LAZY_STOP_DISABLED.with(|v| v.set(disabled));
                                let mut msa = MatchArg::from_param(
                                    &reg,
                                    option,
                                    Some(OnigRegion::new()),
                                    0,
                                    &mp,
                                );
                                // One anchored attempt keeps the missing-literal case bounded.
                                let result = search_in_range_inner(
                                    &reg,
                                    &text,
                                    text.len(),
                                    0,
                                    0,
                                    text.len(),
                                    &mut msa,
                                );
                                (
                                    result,
                                    msa.region.map(|r| (r.num_regs, r.beg, r.end)),
                                    msa.retry_limit_in_search_counter,
                                )
                            };
                            assert_eq!(
                                run(false),
                                run(true),
                                "{pattern} {len} {tail:?} {option:?} {retry}/{search_retry}/{stack}"
                            );
                            LAZY_STOP_DISABLED.with(|v| v.set(false));
                            comparisons += 1;
                        }
                    }
                }
            }
        }
        println!("{comparisons} long captured-lazy comparisons");
    }

    /// `after_next_newline` stops where the character loop it replaces
    /// stops, and declines where it cannot vouch for that loop's steps.
    #[test]
    fn after_next_newline_matches_the_character_loop() {
        let utf8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
        let ascii: OnigEncoding = &crate::encodings::ascii::ONIG_ENCODING_ASCII;
        let texts: &[&[u8]] = &[
            b"",
            b"abc",
            b"ab\ncd\n",
            b"\n\n",
            "caf\u{e9}\nx\u{20ac}y\n".as_bytes(),
            b"a\xe2\nb\nc",
            b"\xff\xfe\n",
            b"x\xc3",
            b"\xe2\x82\xac\n",
        ];
        let mut fast = 0;
        for enc in [utf8, ascii] {
            for &text in texts {
                for end in 0..=text.len() {
                    for s in 0..=end {
                        for cur_range in s..=end + 1 {
                            let mut expected = s;
                            while expected < cur_range {
                                let at = expected;
                                expected = advance_char_to_end(enc, text, expected, end);
                                if is_mbc_newline(enc, text, at, end) || expected == at {
                                    break;
                                }
                            }
                            if let Some(found) = after_next_newline(enc, text, s, cur_range, end) {
                                assert_eq!(found, expected, "{text:?} {s} {cur_range} {end}");
                                fast += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(fast > 200, "{fast}");
    }

    fn make_test_context() -> (RegexType, ParseEnv) {
        use crate::regsyntax::OnigSyntaxOniguruma;
        let enc: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
        let reg = RegexType {
            backtrack_warnings: Vec::new(),
            backtrack_rewrites: Vec::new(),
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
            syntax: OnigSyntaxOniguruma.clone(),
            case_fold_flag: ONIGENC_CASE_FOLD_MIN,
            name_table: None,
            optimize: OptimizeType::None,
            threshold_len: 0,
            anchor: 0,
            anc_dist_min: 0,
            anc_dist_max: 0,
            sub_anchor: 0,
            exact: Vec::new(),
            exact_finder: None,
            pure_literal: false,
            skip_repeated_empty_search: false,
            map: [0u8; CHAR_MAP_SIZE],
            map_offset: 0,
            map_bytes: [0u8; 3],
            map_byte_count: 0,
            map_ascii_ranges: crate::regint::MapAsciiRanges::default(),
            dist_min: 0,
            dist_max: 0,
            needs_capture_tracking: false,
            keep_moves_match_start: false,
            first_byte_map: [0u8; CHAR_MAP_SIZE],
            has_first_byte_map: false,
            start_dispatch: false,
            called_addrs: vec![],
            unset_call_addrs: vec![],
            extp: None,
            literal_tries: Vec::new(),
            check_dependent_guards: false,
            skip_gates: Vec::new(),
            ac_alt: None,
            ac_alt_has_capture: false,
            leading_run: None,
            literal_prefix: None,
            anychar_run: false,
            look_behind_reach: 0,
            extra_map_optimizer: false,
            search_start_map: None,
            search_jump: None,
            required_literals: None,
        };
        let env = ParseEnv {
            options: OnigOptionType::empty(),
            case_fold_flag: 0,
            enc,
            syntax: OnigSyntaxOniguruma.clone(),
            cap_history: 0,
            backtrack_mem: 0,
            backrefed_mem: 0,
            pattern: std::ptr::null(),
            pattern_end: std::ptr::null(),
            error: None,
            reg: std::ptr::null_mut(),
            reg_state: crate::regparse_types::RegexParseState::default(),
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
            look_behind_depth: 0,
            ast_node_count: 0,
            flags: 0,
            recursive_mem: Vec::new(),
            group_min_len: Vec::new(),
            group_max_len: Vec::new(),
        };
        (reg, env)
    }

    fn compile_and_match(pattern: &[u8], input: &[u8]) -> (i32, Option<OnigRegion>) {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(
            r,
            0,
            "compile failed for {:?}",
            std::str::from_utf8(pattern)
        );
        onig_match(
            &reg,
            input,
            input.len(),
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        )
    }

    fn compile_and_search(pattern: &[u8], input: &[u8]) -> (i32, Option<OnigRegion>) {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(
            r,
            0,
            "compile failed for {:?}",
            std::str::from_utf8(pattern)
        );
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

    fn compile_regex(pattern: &[u8]) -> RegexType {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(
            r,
            0,
            "compile failed for {:?}",
            std::str::from_utf8(pattern)
        );
        reg
    }

    /// Attempts run untracked, with a second pass for a match among them,
    /// only where the repeated match is likely to cost less than tracking
    /// every attempt.
    #[test]
    fn two_pass_capture_fill_needs_push_captures_away_from_a_leading_literal() {
        let pays = |pattern: &[u8]| two_pass_capture_fill_pays(&compile_full(pattern));
        // Push captures, no literal or one at an unbounded distance.
        assert!(pays(br"(.)*x"));
        assert!(pays(br"(\w)+;"));
        // Plain captures: the repeated match is short unless `.` repeats
        // without bound.
        assert!(pays(br"\b(abs|acos|ceil)\b"));
        assert!(pays(br"(\d+)-(\d+)"));
        assert!(pays(br"(.{1,3})x"));
        assert!(!pays(br"\s*([^#(.\\_[:alpha:]\s].*?)(?=#|$)"));
        assert!(!pays(br"(a)(?=.*b)"));
        // The literal at the match start makes candidates likely matches.
        assert!(!pays(br"\((.(?!\.\.))+\)"));
    }

    #[test]
    fn compiled_literal_finder_preserves_bounded_searches_and_limits() {
        let _limits = shared_limits();
        let patterns = [
            r"ab",
            r"(ab)(c?)",
            r"ab(?:c|d)",
            r"(ab)c\1",
            r"(?<=x)ab",
            r"ab\K(c?)",
            r".*ab",
            r"ab(?=c)",
            r"\Aab",
            r"abcdefghijklmnopqrstuvwxyz",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"a",
            b"ab",
            b"xab",
            b"abcab",
            b"abdab",
            b"ab\0ab",
            b"\xc2ab",
            b"a\xffab",
            b"ab\xe2\x82",
            b"\xc2\xa1ab",
            b"abcdefghijklmnopqrstuvwxyz!",
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let compile = || {
                    regcomp::onig_new(
                        pattern.as_bytes(),
                        ONIG_OPTION_NONE,
                        enc,
                        &crate::regsyntax::OnigSyntaxOniguruma,
                    )
                    .unwrap()
                };
                let optimized = compile();
                let mut reference = compile();
                // Retain the same optimizer and bytecode; only rebuild the
                // substring search plan on each search, as before this change.
                assert!(reference.exact_finder.take().is_some(), "{pattern}");
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                    ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                ] {
                                    for (retry, search_retry, stack) in
                                        [(0, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1)]
                                    {
                                        let mut mp = onig_new_match_param();
                                        mp.retry_limit_in_match = retry;
                                        mp.retry_limit_in_search = search_retry;
                                        mp.match_stack_limit = stack;
                                        mp.time_limit = 0;
                                        let search = |reg| {
                                            let (result, region) = onig_search_with_param(
                                                reg,
                                                input,
                                                end,
                                                start,
                                                range,
                                                Some(OnigRegion::new()),
                                                option,
                                                &mp,
                                            );
                                            let region = region.unwrap();
                                            (result, region.beg, region.end)
                                        };
                                        assert_eq!(
                                            search(&optimized),
                                            {
                                                ATOMIC_CLASS_PREFIX_DISABLED
                                                    .with(|disabled| disabled.set(true));
                                                let result = search(&reference);
                                                ATOMIC_CLASS_PREFIX_DISABLED
                                                    .with(|disabled| disabled.set(false));
                                                result
                                            },
                                            "{} {pattern:?} {input:?} end={end} start={start} range={range} {option:?} retry={retry} search_retry={search_retry} stack={stack}",
                                            enc.name()
                                        );
                                        comparisons += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!("{comparisons} compiled/rebuilt literal finder comparisons");
    }

    #[test]
    fn atomic_class_prefix_filter_falls_back_for_observable_modes() {
        let _limits = shared_limits();
        let compile = |pattern: &str| {
            regcomp::onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_NONE,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap()
        };
        for pattern in [
            r"([ab]+):",
            r"[ab]*:",
            r"[ab]+a",
            r"[^x]+:",
            r"\p{L}+:",
            r"\A[ab]+:",
            r"[ab]+:(?{x})",
            r"[ab]+:\x00",
            r"[ab]+\x00",
            r"[ab]+:|other",
        ] {
            let reg = compile(pattern);
            let msa = MatchArg::new(&reg, ONIG_OPTION_NONE, None, 0);
            assert!(atomic_ascii_class_prefix(&reg, &msa).is_none(), "{pattern}");
        }
        let reg = compile(r"[ab]+:");
        for (retry, search_retry, stack, time) in
            [(1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1)]
        {
            let mut msa = MatchArg::new(&reg, ONIG_OPTION_NONE, None, 0);
            msa.retry_limit_in_match = retry;
            msa.retry_limit_in_search = search_retry;
            msa.match_stack_limit = stack;
            msa.time_limit = time;
            assert!(atomic_ascii_class_prefix(&reg, &msa).is_none());
        }
    }

    /// `first_op_fails` decides each supported first instruction exactly as
    /// the VM does in a RegSet attempt, at every byte position of valid,
    /// multibyte and malformed subjects and of shortened logical ends. A
    /// pattern that is only that instruction matches exactly where it holds,
    /// and an attempt failing there counts exactly one retry.
    #[test]
    fn first_op_test_agrees_with_the_vm_attempt() {
        // The attempts below read the process-global limits.
        let _limits = shared_limits();
        let compile = |pattern: &str, option: OnigOptionType| {
            regcomp::onig_new(
                pattern.as_bytes(),
                option,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap()
        };
        let word_ascii = ONIG_OPTION_WORD_IS_ASCII;
        let cases: [(&str, OnigOptionType, &str); 22] = [
            (r"\b", ONIG_OPTION_NONE, "boundary"),
            (r"\B", ONIG_OPTION_NONE, "no-boundary"),
            (r"\b", word_ascii, "boundary"),
            (r"\B", word_ascii, "no-boundary"),
            (r"^", ONIG_OPTION_NONE, "line"),
            (r"(?<=a)", ONIG_OPTION_NONE, "behind"),
            (r"(?<!a)", ONIG_OPTION_NONE, "behind"),
            (r"(?<=ab)", ONIG_OPTION_NONE, "behind"),
            (r"(?<!ab)", ONIG_OPTION_NONE, "behind"),
            (r"(?<=abcdefgh)", ONIG_OPTION_NONE, "behind"),
            (r"(?<=[a-c])", ONIG_OPTION_NONE, "behind"),
            (r"(?<![a-c\n])", ONIG_OPTION_NONE, "behind"),
            (r"(?<=[^a-c])", ONIG_OPTION_NONE, "behind"),
            (r"(?<=é)", ONIG_OPTION_NONE, "behind"),
            (r"(?<!日本)", ONIG_OPTION_NONE, "behind"),
            (r"(?<=[éa])", ONIG_OPTION_NONE, "behind"),
            (r"(?<![é日])", ONIG_OPTION_NONE, "behind"),
            (r"(?<=[\p{L}])", ONIG_OPTION_NONE, "behind"),
            (r"(?<=\.)", ONIG_OPTION_NONE, "behind"),
            (r"(?<![\w$])", ONIG_OPTION_NONE, "behind"),
            (r"(?i)(?<=x)", ONIG_OPTION_NONE, "behind"),
            (r"(?<=\n)", ONIG_OPTION_NONE, "behind"),
        ];
        let subjects: [&[u8]; 18] = [
            b"",
            b"a",
            b"ab",
            b"ab cd_e.f",
            b"\n",
            b"a\nb\n",
            b"\n\n",
            b"x\r\ny\n",
            "é a_b ü".as_bytes(),
            "日本語 x日本 abcdefgh".as_bytes(),
            b"\xff\xfe a",
            b"a\xe2\x82 b",
            b"\x80abc",
            b"\xc3",
            b"\xc3\n\xa9a",
            b"abcdefgh abc$1",
            "kK\u{212a}X x".as_bytes(),
            b"$a.b!c",
        ];
        let mut rejected = 0;
        let mut kept = 0;
        for (pattern, compile_option, kind) in cases {
            let reg = compile(pattern, compile_option);
            let test = first_op_test(&reg)
                .unwrap_or_else(|| panic!("{pattern} has no first-instruction test"));
            let expected_kind = match test {
                FirstOpTest::WordBoundary { not: false, .. } => "boundary",
                FirstOpTest::WordBoundary { not: true, .. } => "no-boundary",
                FirstOpTest::BeginLine => "line",
                FirstOpTest::LookBehind { .. } => "behind",
            };
            assert_eq!(expected_kind, kind, "{pattern}");
            if let FirstOpTest::WordBoundary { mode, .. } = test {
                assert_eq!(mode != 0, compile_option == word_ascii, "{pattern}");
            }
            for subject in subjects {
                let ends = [subject.len(), subject.len().saturating_sub(1), 0];
                for end in ends {
                    for s in 0..=end {
                        for option in [ONIG_OPTION_NONE, ONIG_OPTION_NOTBOL] {
                            let fails = first_op_fails(test, &reg, subject, end, s, option);
                            let mut msa = MatchArg::new(&reg, option, None, s);
                            let result = onig_match_with_msa_start(
                                &reg, subject, end, s, s, option, &mut msa,
                            );
                            let context =
                                format!("{pattern} {subject:?} end={end} s={s} option={option:?}");
                            if fails {
                                assert_eq!(result, ONIG_MISMATCH, "{context}");
                                assert_eq!(msa.retry_limit_in_search_counter, 1, "{context}");
                                rejected += 1;
                            } else {
                                // A positive look-behind goes on where its body
                                // ends, as in C: away from `s` mid-character or
                                // over malformed UTF-8.
                                let expected = match test {
                                    FirstOpTest::LookBehind {
                                        char_len,
                                        not: false,
                                        ascii_compatible,
                                    } => {
                                        let body_end = look_behind_body_end(
                                            &reg.ops[1],
                                            char_len,
                                            reg.enc,
                                            subject,
                                            s,
                                            end,
                                            end,
                                            ascii_compatible,
                                        )
                                        .expect("the look-behind holds");
                                        body_end as i32 - s as i32
                                    }
                                    _ => 0,
                                };
                                assert_eq!(result, expected, "{context}");
                                kept += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(rejected > 1_000 && kept > 1_000, "{rejected} {kept}");
    }

    /// Only the listed zero-width first instructions get a test, and never
    /// with callouts: everything else takes the plain attempt.
    #[test]
    fn first_op_test_leaves_other_first_instructions_to_the_vm() {
        let compile = |pattern: &str| {
            regcomp::onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_NONE,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap()
        };
        for pattern in [
            r"a\b",
            r"x",
            r"abc",
            r"[ab]",
            r"[^ab]",
            r"\w",
            r"(?:\b|x)",
            r"(?<=a|bc)",
            r"(?<=.)",
            r"(?=a)",
            r"(?!a)",
            r"\G\b",
            r"\A",
            r"\z",
            r"$",
            r"\b(*COUNT)x",
            r"^(*FAIL)",
            r"(?<=a)(?{x})",
            r"(?:)",
        ] {
            let reg = compile(pattern);
            assert_eq!(first_op_test(&reg), None, "{pattern}");
        }
        // A `\w` look-behind is fused (`LookBehindOp`), so it is a test too.
        let reg = compile(r"(?<!\w)");
        assert_eq!(
            first_op_test(&reg),
            Some(FirstOpTest::LookBehind {
                char_len: 1,
                not: true,
                ascii_compatible: true,
            })
        );
    }

    #[test]
    fn atomic_class_prefix_filter_preserves_search_results_and_limits() {
        let _limits = shared_limits();
        let patterns = [
            r"[ab]+:",
            r"[ab]+:(a+)\1",
            r"[ab]+:(?<!a:)(c?)",
            r"[ab]+:\K(c?)",
            r"[ab]+:(?:c|cc)",
            r"[ab]+:\G",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"a",
            b"ab",
            b"ab:",
            b"aab!ab:cc",
            b"a:aaa ab:aa",
            b"ba:ccab:c",
            b"!b:\0",
            b"ab\0",
            b"\xc2ab:",
            b"a\xffab:",
            b"ab\xe2\x82",
            b"\xc2\xa1ab:",
            b"aaaa!",
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let compile = || {
                    regcomp::onig_new(
                        pattern.as_bytes(),
                        ONIG_OPTION_NONE,
                        enc,
                        &crate::regsyntax::OnigSyntaxOniguruma,
                    )
                    .unwrap()
                };
                let optimized = compile();
                let reference = compile();
                assert!(
                    atomic_ascii_class_prefix(
                        &optimized,
                        &MatchArg::new(&optimized, ONIG_OPTION_NONE, None, 0),
                    )
                    .is_some(),
                    "{pattern}"
                );
                for &input in inputs {
                    for end in 0..=input.len() {
                        for start in 0..=end {
                            for range in [0, start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                    ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                                ] {
                                    for (retry, search_retry, stack) in [
                                        (0, 0, 0),
                                        (1, 0, 0),
                                        (2, 0, 0),
                                        (3, 0, 0),
                                        (0, 1, 0),
                                        (0, 3, 0),
                                        (0, 0, 1),
                                        (0, 0, 3),
                                    ] {
                                        let mut mp = onig_new_match_param();
                                        mp.retry_limit_in_match = retry;
                                        mp.retry_limit_in_search = search_retry;
                                        mp.match_stack_limit = stack;
                                        mp.time_limit = 0;
                                        let search = |reg| {
                                            let (result, region) = onig_search_with_param(
                                                reg,
                                                input,
                                                end,
                                                start,
                                                range,
                                                Some(OnigRegion::new()),
                                                option,
                                                &mp,
                                            );
                                            let region = region.unwrap();
                                            (result, region.beg, region.end)
                                        };
                                        assert_eq!(
                                            search(&optimized),
                                            search(&reference),
                                            "{} {pattern:?} {input:?} end={end} start={start} range={range} {option:?} retry={retry} search_retry={search_retry} stack={stack}",
                                            enc.name()
                                        );
                                        comparisons += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!("{comparisons} prefix-filter/original comparisons");
    }

    /// Class runs must preserve entry into a suffix (the optional prefix),
    /// partial-failure positions, captures, and match/retry limit outcomes.
    #[test]
    fn ascii_class_runs_match_unbatched_instructions() {
        let _limits = shared_limits();
        let patterns = [
            r"[ab]{4}",
            r"[ab]?[ab]{3}",
            r"[ab]{2,4}b",
            r"([ab]{2}){2}\1",
            r"([ab]{2}|a)[ab]{2}",
            r"(?<=[ab])[ab]{3}",
            r"(?<=[ab]{2,3})[ab]{2}",
            r"(?<![ab]{2})[ab]{2}",
            r"(?=[ab]{4})([ab]{2})",
            r"(?>[ab]{2})[ab]{2}",
            r"(?:[ab]{2})+b",
            r"([ab]{2})?([ab]{2})",
            r"\A[0-9]{3}-[0-9]{2}\z",
            r"[ab]{2}\K[ab]{2}",
            r"(?i)[ab]{4}",
            r"(?:[^x][ab]{2}|)",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"a",
            b"ab",
            b"aba",
            b"abba",
            b"aababb",
            b"aaaaab!",
            b"!aababb",
            b"abababab",
            b"AbAB",
            b"123-45",
            b"12x-45",
            b"ab\xffab",
            b"ab\xc2\xa1ab",
            b"\xc2",
            b"\xe2\x82",
            b"xx\xc2aa",
        ];
        let mut comparisons = 0;
        for pattern in patterns {
            let optimized = regcomp::onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_NONE,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap();
            let mut reference = regcomp::onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_NONE,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap();
            let mut runs = 0;
            for op in &mut reference.ops {
                if let OperationPayload::CClassRun { bsp, .. } = &op.payload {
                    op.payload = OperationPayload::CClass {
                        bsp: bsp.clone(),
                        ascii_fast: CClassAsciiFastKind::None,
                    };
                    op.opcode = OpCode::CClass;
                    runs += 1;
                }
            }
            assert!(runs > 0, "{pattern}: no class run exercised");
            for &input in inputs {
                for end in 0..=input.len() {
                    for start in 0..=end {
                        for range in [0, start, end] {
                            for option in [
                                ONIG_OPTION_NONE,
                                ONIG_OPTION_FIND_LONGEST,
                                ONIG_OPTION_FIND_NOT_EMPTY,
                                ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
                            ] {
                                for (retry, stack) in [(0, 0), (1, 0), (3, 0), (0, 1), (0, 3)] {
                                    let mut mp = onig_new_match_param();
                                    mp.retry_limit_in_match = retry;
                                    mp.retry_limit_in_search = retry;
                                    mp.match_stack_limit = stack;
                                    mp.time_limit = 0;
                                    let search = |reg| {
                                        let (result, region) = onig_search_with_param(
                                            reg,
                                            input,
                                            end,
                                            start,
                                            range,
                                            Some(OnigRegion::new()),
                                            option,
                                            &mp,
                                        );
                                        let region = region.unwrap();
                                        (result, region.beg, region.end)
                                    };
                                    assert_eq!(
                                        search(&optimized),
                                        search(&reference),
                                        "{pattern:?} {input:?} end={end} start={start} range={range} {option:?} retry={retry} stack={stack}",
                                    );
                                    comparisons += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!("{comparisons} class-run/unbatched comparisons");
    }

    #[test]
    fn ascii_class_runs_split_at_the_metadata_bound() {
        let _limits = shared_limits();
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for count in [254, 255, 256, 300] {
                let pattern = format!(r"\A{}\z", "[ab]".repeat(count));
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                let longest = reg
                    .ops
                    .iter()
                    .filter_map(|op| match op.payload {
                        OperationPayload::CClassRun { len: n, .. } => Some(n),
                        _ => None,
                    })
                    .max()
                    .unwrap();
                assert_eq!(usize::from(longest), count.min(usize::from(u8::MAX)));
                for (len, last, expected) in [
                    (count, b'b', 0),
                    (count - 1, b'b', ONIG_MISMATCH),
                    (count + 1, b'b', ONIG_MISMATCH),
                    (count, b'!', ONIG_MISMATCH),
                    (count, 0xff, ONIG_MISMATCH),
                ] {
                    let mut text = vec![b'a'; len];
                    text[len - 1] = last;
                    let (result, region) = onig_search(
                        &reg,
                        &text,
                        len,
                        0,
                        len,
                        Some(OnigRegion::new()),
                        ONIG_OPTION_NONE,
                    );
                    assert_eq!(result, expected, "{} {count} {len} {last}", enc.name());
                    if result >= 0 {
                        let region = region.unwrap();
                        assert_eq!((region.beg[0], region.end[0]), (0, count as i32));
                    }
                }
            }
        }
    }

    #[test]
    fn ascii_class_runs_leave_multibyte_and_negated_classes_unbatched() {
        for pattern in [
            r"[^ab]{4}",
            r"[aé]{4}",
            r"(?i)[k]{4}",
            r"[\p{L}]{4}",
            r"[a]{4}",
            r"[aA]{4}",
        ] {
            let reg = regcomp::onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_NONE,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap();
            assert!(
                reg.ops
                    .iter()
                    .all(|op| !matches!(op.payload, OperationPayload::CClassRun { .. })),
                "{pattern}"
            );
        }
    }

    #[test]
    fn search_start_equal_range_is_a_bounded_anchored_attempt() {
        let _limits = shared_limits();
        let text = b"aaaa";
        let reg = compile_regex(b"a");
        let (position, region) = onig_search(
            &reg,
            text,
            text.len(),
            2,
            2,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(position, 2);
        assert_eq!(region.unwrap().end[0], 3);

        let text = b"aab";
        let reg = compile_regex(b"a*");
        let (position, region) = onig_search(
            &reg,
            text,
            text.len(),
            0,
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(position, 0);
        assert_eq!(region.unwrap().end[0], 1);
    }

    #[test]
    fn search_normalizes_out_of_range_endpoints() {
        let _limits = shared_limits();
        let text = b"abc";
        let reg = compile_regex(b"(?=b)");

        for (end, start, range, expected) in [
            (text.len(), 0, 100, 1),
            (100, 0, 100, 1),
            (text.len(), 100, 0, ONIG_MISMATCH),
        ] {
            let (position, _) = onig_search(
                &reg,
                text,
                end,
                start,
                range,
                Some(OnigRegion::new()),
                ONIG_OPTION_NONE,
            );
            assert_eq!(
                position, expected,
                "end={end}, start={start}, range={range}"
            );
        }
    }

    fn has_opcode(reg: &RegexType, opcode: OpCode) -> bool {
        reg.ops.iter().any(|op| op.opcode == opcode)
    }

    fn opcode_names(reg: &RegexType) -> String {
        reg.ops
            .iter()
            .map(|op| format!("{:?}", op.opcode))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn make_code_range_table(ranges: &[(u32, u32)]) -> Vec<u32> {
        let mut out = Vec::with_capacity(1 + ranges.len() * 2);
        out.push(ranges.len() as u32);
        for (lo, hi) in ranges {
            out.push(*lo);
            out.push(*hi);
        }
        out
    }

    #[test]
    fn named_capture_can_skip_tracking_when_region_is_none() {
        let _limits = shared_limits();
        let reg = compile_regex(b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})");
        assert!(
            !reg.needs_capture_tracking,
            "plain captures without backrefs should not require tracking when region is absent"
        );

        let text = b"Event on 2025-12-31 at venue, next on 2026-01-15.";
        let (pos, region) = onig_search(
            &reg,
            text,
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        assert_eq!(pos, 9);
        assert!(region.is_none());
    }

    #[test]
    fn named_capture_region_reuse_mismatch_resets_slots() {
        let _limits = shared_limits();
        let reg = compile_regex(b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})");
        assert!(!reg.needs_capture_tracking);
        assert_eq!(reg.push_mem_start, 0);
        assert_eq!(reg.push_mem_end, 0);

        let mut region = Some(onig_region_new());

        let text_hit = b"Event on 2025-12-31 at venue.";
        let (pos_hit, returned) = onig_search(
            &reg,
            text_hit,
            text_hit.len(),
            0,
            text_hit.len(),
            region,
            ONIG_OPTION_NONE,
        );
        assert_eq!(pos_hit, 9);
        region = returned;
        let hit = region.as_ref().expect("region after hit");
        assert_eq!(hit.beg[0], 9);
        assert_eq!(hit.end[0], 19);
        assert_eq!(hit.beg[1], 9);
        assert_eq!(hit.end[1], 13);
        assert_eq!(hit.beg[2], 14);
        assert_eq!(hit.end[2], 16);
        assert_eq!(hit.beg[3], 17);
        assert_eq!(hit.end[3], 19);

        let text_miss = b"Event on xx-yy-zz at venue.";
        let (pos_miss, returned) = onig_search(
            &reg,
            text_miss,
            text_miss.len(),
            0,
            text_miss.len(),
            region,
            ONIG_OPTION_NONE,
        );
        assert_eq!(pos_miss, ONIG_MISMATCH);
        let miss = returned.expect("region after miss");
        for i in 0..=reg.num_mem as usize {
            assert_eq!(miss.beg[i], ONIG_REGION_NOTPOS, "beg[{}]", i);
            assert_eq!(miss.end[i], ONIG_REGION_NOTPOS, "end[{}]", i);
        }
    }

    #[test]
    fn backref_still_requires_capture_tracking_without_region() {
        let _limits = shared_limits();
        let reg = compile_regex(b"(\\w+) \\1");
        assert!(
            reg.needs_capture_tracking,
            "backrefs depend on capture tracking even when region is absent"
        );

        let text = b"the the quick";
        let (pos, region) = onig_search(
            &reg,
            text,
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        assert_eq!(pos, 0);
        assert!(region.is_none());
    }

    fn compile_full(pattern: &[u8]) -> RegexType {
        crate::regcomp::onig_new(
            pattern,
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &crate::regsyntax::OnigSyntaxOniguruma,
        )
        .expect("pattern compiles")
    }

    /// Multibyte class instructions read a byte that starts no multibyte
    /// character as C does (#261): the class fails, its negation takes the
    /// byte alone. A character the range truncates matches the negation,
    /// which goes on at the subject's end. Every forward start and range
    /// against C.
    #[cfg(feature = "ffi")]
    #[test]
    fn multibyte_classes_match_c_over_malformed_utf8_and_truncation() {
        let _limits = shared_limits();
        let patterns = [
            r"[\x{80}\x{81}]",
            r"[^\x{80}]",
            r"[^\x{100}]",
            r"[^a\x{100}]",
            r"[a\x{100}]",
            r"[\x{80}-\x{10ffff}]+",
            r"[\x{80}-\x{10ffff}]*x",
            r"[^\x{100}]*",
            r"[^\x{100}]+x",
            r"[^a\x{100}]*$",
            r"[^a\x{100}]+",
            r"(?<=[^\x{100}])x?",
            r"(?<=[\x{80}\x{81}]).",
            r"x[^\x{100}]{2}",
            r"[^\x{80}]*",
            r"[^\x{80}]+x",
            r"(?<=[^\x{80}])x?",
        ];
        let subjects: [&[u8]; 11] = [
            b"\x80x",
            b"\x80\x80x",
            b"\x80",
            b"(\x80xz",
            b"a\x80\x80b",
            b"\xe3\x81\x82",
            b"x\xe3\x81\x82\xc3",
            "é\u{80}x".as_bytes(),
            b"\xc3\xa9\x80\xc3\xa9",
            b"\xff\xbf\xc3x",
            b"xx\xf0\x9f\x98",
        ];
        for pattern in patterns {
            let reg = compile_full(pattern.as_bytes());
            let c =
                crate::ffi::CRegex::new(pattern.as_bytes(), crate::ffi::ONIG_OPTION_NONE).unwrap();
            for subject in subjects {
                // Forward searches: with `range == start` C searches backward,
                // where a truncated last character reads past the subject.
                for start in 0..subject.len() {
                    for range in start + 1..=subject.len() {
                        let (r, region) = onig_search(
                            &reg,
                            subject,
                            subject.len(),
                            start,
                            range,
                            Some(OnigRegion::new()),
                            ONIG_OPTION_NONE,
                        );
                        let mut c_region = crate::ffi::CRegion::new();
                        let c_r = c.search(subject, start, range, Some(&mut c_region), 0);
                        let context = format!("{pattern} on {subject:?} {start}..{range}");
                        assert_eq!(r, c_r, "{context}");
                        if r >= 0 {
                            let region = region.unwrap();
                            let spans: Vec<(i32, i32)> = region
                                .beg
                                .iter()
                                .zip(&region.end)
                                .take(region.num_regs as usize)
                                .map(|(&b, &e)| (b, e))
                                .collect();
                            assert_eq!(spans, c_region.capture_ranges(), "{context}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn backtracked_push_captures_do_not_require_tracking() {
        let _limits = shared_limits();
        // A capture inside a repeat compiles to MEM_START_PUSH/MEM_END_PUSH so
        // that backtracking can restore it. Only the region observes that, so
        // a region-free search must not pay for the bookkeeping.
        let reg = compile_full(b"(?:(a)|b)*ab");
        assert_ne!(reg.push_mem_start | reg.push_mem_end, 0);
        assert!(reg.ops.iter().any(|op| op.opcode == OpCode::MemStartPush));
        assert!(!reg.needs_capture_tracking);

        // (input, match start, match end, group 1 start, group 1 end)
        type Case = (&'static [u8], i32, i32, i32, i32);
        let cases: [Case; 2] = [
            // The iteration that captured `a` is backtracked away, so its
            // capture must be restored to unset.
            (b"xbab", 1, 4, ONIG_REGION_NOTPOS, ONIG_REGION_NOTPOS),
            (b"xaab", 1, 4, 1, 2),
        ];
        for (text, beg, end, g1_beg, g1_end) in cases {
            let (pos, region) = onig_search(
                &reg,
                text,
                text.len(),
                0,
                text.len(),
                None,
                ONIG_OPTION_NONE,
            );
            assert_eq!(pos, beg);
            assert!(region.is_none());

            let (pos, region) = onig_search(
                &reg,
                text,
                text.len(),
                0,
                text.len(),
                Some(OnigRegion::new()),
                ONIG_OPTION_NONE,
            );
            assert_eq!(pos, beg);
            let region = region.expect("region");
            assert_eq!((region.beg[0], region.end[0]), (beg, end));
            assert_eq!((region.beg[1], region.end[1]), (g1_beg, g1_end));
        }
    }

    #[test]
    fn captures_match_c_on_stack_and_reused_capture_arrays() {
        let _limits = shared_limits();
        // A tracked run keeps up to MEM_STK_INLINE capture slots on its own
        // stack and reuses the MatchArg's buffers for more. Groups that never
        // participate move a pattern to the reused buffers; patterns of both
        // kinds and sizes alternate on one thread. Expected regions are C
        // Oniguruma's.
        type Case = (&'static str, &'static [u8], &'static [(i32, i32)]);
        let cases: [Case; 6] = [
            // Groups in a loop keep their last iteration.
            ("((a)|(b))*c", b"xabac", &[(1, 5), (3, 4), (3, 4), (2, 3)]),
            ("(.)*x", b"abxcx", &[(0, 5), (3, 4)]),
            // Backtracking restores the captures of abandoned paths.
            (
                "(a|ab)(c|bcd)(d*)",
                b"abcd",
                &[(0, 4), (0, 1), (1, 4), (4, 4)],
            ),
            ("(?:(a)|b)*", b"aba", &[(0, 3), (2, 3)]),
            ("((a)|b)*?c", b"abbc", &[(0, 4), (2, 3), (0, 1)]),
            ("(a)|(b)", b"b", &[(0, 1), (-1, -1), (0, 1)]),
        ];
        let pads = [0, MEM_STK_INLINE, 4 * MEM_STK_INLINE];
        for round in 0..pads.len() {
            for (pattern, text, expected) in cases {
                for tier in 0..pads.len() {
                    let pad = pads[(round + tier) % pads.len()];
                    let pattern = format!("{pattern}{}", "(x)?".repeat(pad));
                    let reg = compile_full(pattern.as_bytes());
                    let num_mem = reg.num_mem as usize;
                    assert_eq!(num_mem >= MEM_STK_INLINE, pad > 0);
                    let (pos, region) = onig_search(
                        &reg,
                        text,
                        text.len(),
                        0,
                        text.len(),
                        Some(OnigRegion::new()),
                        ONIG_OPTION_NONE,
                    );
                    assert_eq!(pos, expected[0].0, "{pattern}");
                    let region = region.expect("region");
                    for (i, &bounds) in expected.iter().enumerate() {
                        assert_eq!(
                            (region.beg[i], region.end[i]),
                            bounds,
                            "{pattern} group {i}"
                        );
                    }
                    for i in expected.len()..region.num_regs as usize {
                        assert_eq!(region.beg[i], ONIG_REGION_NOTPOS, "{pattern} group {i}");
                    }
                }
            }
        }
    }

    #[test]
    fn match_stack_limit_counts_push_captures_with_and_without_region() {
        let _limits = shared_limits();
        // Untracked runs drop the push-capture stack entries. A configured
        // stack limit must still trip exactly where a region-tracking run
        // (and C) trips it.
        let reg = compile_full(b"(?:(a)|b)*c");
        let mut text = vec![b'a'; 160];
        text.push(b'c');

        let mut saw_limit = false;
        let mut saw_match = false;
        for limit in (1..8).chain(300..1000) {
            let mut mp = onig_new_match_param();
            onig_set_match_stack_limit_size_of_match_param(&mut mp, limit);
            let (without_region, _) = onig_search_with_param(
                &reg,
                &text,
                text.len(),
                0,
                text.len(),
                None,
                ONIG_OPTION_NONE,
                &mp,
            );
            let (with_region, _) = onig_search_with_param(
                &reg,
                &text,
                text.len(),
                0,
                text.len(),
                Some(OnigRegion::new()),
                ONIG_OPTION_NONE,
                &mp,
            );
            assert_eq!(without_region, with_region, "stack limit {limit}");
            saw_limit |= with_region == ONIGERR_MATCH_STACK_LIMIT_OVER;
            saw_match |= with_region == 0;
        }
        assert!(
            saw_limit && saw_match,
            "the limit range must cover both outcomes"
        );
    }

    #[test]
    fn negated_class_loops_compile_to_star_opcodes() {
        for (pattern, opcode) in [
            (&b"[^\"]+"[..], OpCode::CClassNotStar),
            (b"[^\\x{100}]*x", OpCode::CClassMbNotStar),
            (b"[^a\\x{100}]*x", OpCode::CClassMixNotStar),
            // Alt-CClass fusion over a negated first branch.
            (b"(?:[^\"\\\\]|\\\\.)*\"", OpCode::CClassNotStar),
        ] {
            let reg = compile_full(pattern);
            assert!(
                reg.ops.iter().any(|op| op.opcode == opcode),
                "{:?} should use {opcode:?}",
                std::str::from_utf8(pattern)
            );
        }
    }

    #[test]
    fn negated_class_star_backtracks_through_every_character() {
        let _limits = shared_limits();
        // (pattern, input, expected match)
        type Case = (&'static [u8], &'static [u8], (i32, i32));
        let cases: [Case; 5] = [
            (b"[^\"]*x", b"abxcdx\"x", (0, 6)),
            (b"(?:[^\"\\\\]|\\\\.)+\"", b"a\\\"b\"c", (0, 5)),
            (
                b"[^\\x{100}]+\\x{e9}",
                "d\u{e9}j\u{e0} \u{e9}t\u{e9}".as_bytes(),
                (0, 12),
            ),
            (
                b"[^a\\x{100}]*b",
                "x\u{e9}b\u{e9}b\u{100}b".as_bytes(),
                (0, 7),
            ),
            // A stray continuation byte after `\u{e9}` is a one-byte character
            // at 3, the only boundary followed by `.b`. Stepping back from the
            // end with prev_char_head would skip it (4 -> 1).
            (b"[^\"]*(?=.b)", b"a\xc3\xa9\x80b", (0, 3)),
        ];
        for (pattern, input, expected) in cases {
            let reg = compile_full(pattern);
            let (pos, region) = onig_search(
                &reg,
                input,
                input.len(),
                0,
                input.len(),
                Some(OnigRegion::new()),
                ONIG_OPTION_NONE,
            );
            let region = region.expect("region");
            assert_eq!(
                (pos, (region.beg[0], region.end[0])),
                (expected.0, expected),
                "{:?}",
                std::str::from_utf8(pattern)
            );
        }
    }

    #[test]
    fn star_opcodes_match_the_generic_loop_on_malformed_utf8() {
        let _limits = shared_limits();
        // `{0,50}` keeps the generic PUSH/JUMP loop, whose backtrack points
        // are exactly the boundaries of the forward scan. Truncated and
        // stray multibyte sequences make `prev_char_head` disagree with
        // those boundaries, so the star opcodes must not rely on it there.
        let inputs: [&[u8]; 8] = [
            b"x\xc3ab",
            b"\xc3\xa9\xa9ab",
            b"ab\xe2\x82a b",
            b"\xf0\x9f\x98a b",
            b"a\xc3\xa9\x80b",
            b"abcQq\"\\ab",
            b"ab\xc3\"q",
            "aéQ𝄞b".as_bytes(),
        ];
        for (class, opcode) in [
            ("[\\x{80}-\\x{10ffff}]", OpCode::CClassMbStar),
            ("[\\x{80}-\\x{10ffff}a-z]", OpCode::CClassMixStar),
            ("[^\\x{100}]", OpCode::CClassMbNotStar),
            ("[^\"\\x{100}]", OpCode::CClassMixNotStar),
            ("[^\"]", OpCode::CClassNotStar),
            ("[^\"\\\\]", OpCode::CClassNotStar),
            ("[^qQ]", OpCode::CClassNotStar),
            ("\\w", OpCode::WordStar),
        ] {
            for tail in ["(?=.b)", "(?=a)", "(?= )"] {
                let star = format!("{class}*{tail}");
                let generic = format!("{class}{{0,50}}{tail}");
                let star_reg = compile_full(star.as_bytes());
                assert!(star_reg.ops.iter().any(|op| op.opcode == opcode), "{star}");
                let generic_reg = compile_full(generic.as_bytes());
                for input in inputs {
                    let run = |reg: &RegexType| {
                        let (pos, region) = onig_search(
                            reg,
                            input,
                            input.len(),
                            0,
                            input.len(),
                            Some(OnigRegion::new()),
                            ONIG_OPTION_NONE,
                        );
                        let region = region.expect("region");
                        (pos, region.beg[0], region.end[0])
                    };
                    assert_eq!(run(&star_reg), run(&generic_reg), "{star} on {input:x?}");
                }
            }
        }
    }

    #[test]
    fn negated_star_fusion_preserves_results_captures_and_retries() {
        let _limits = shared_limits();
        let patterns = [
            r#"([^\"]*+)"#,
            r#"([^\"]*+)\"x|([^\"]*)b"#,
            r#"([^qQ]*+)Q"#,
            r#"([^\"\\]*+)(?:x|y)"#,
            r#"(?:(a)|([^\"]*+))\""#,
            r#"(?=([^\"]*+))a"#,
            r#"([^\"]*+)\1"#,
            r#"(?:[^\"]*+z|a*)"#,
        ];
        let suffixes: &[&[u8]] = &[
            b"",
            b"xb\"x",
            b"abQqb",
            b"a\0b",
            b"\xc3\"b",
            b"\xe2\x82",
            b"\x80ab",
            b"\xffab",
            "éa𝄞b\"x".as_bytes(),
        ];
        let mut comparisons = 0;
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for pattern in patterns {
                let reg = regcomp::onig_new(
                    pattern.as_bytes(),
                    ONIG_OPTION_NONE,
                    enc,
                    &crate::regsyntax::OnigSyntaxOniguruma,
                )
                .unwrap();
                assert!(
                    reg.ops.windows(2).any(|pair| {
                        pair[0].opcode == OpCode::CClassNotStar
                            && pair[1].opcode == OpCode::PopStarChoices
                    }),
                    "{pattern}"
                );
                for prefix in [0, 31, 32, 33, 127] {
                    for suffix in suffixes {
                        let mut input = vec![b'a'; prefix];
                        input.extend_from_slice(suffix);
                        for end in [input.len(), input.len().saturating_sub(1)] {
                            for (start, range) in [(0, end), (end, 0), (0, prefix.min(end))] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                ] {
                                    for (retry, search_retry, stack) in [
                                        (0, 0, 0),
                                        (1, 0, 0),
                                        (0, 1, 0),
                                        (0, 7, 0),
                                        (0, 0, 1),
                                        (0, 0, 320),
                                    ] {
                                        let mp = OnigMatchParam {
                                            retry_limit_in_match: retry,
                                            retry_limit_in_search: search_retry,
                                            match_stack_limit: stack,
                                            ..onig_new_match_param()
                                        };
                                        for with_region in [false, true] {
                                            let run = |disabled| {
                                                NEGATED_STAR_FUSION_DISABLED
                                                    .with(|v| v.set(disabled));
                                                let mut msa = MatchArg::from_param(
                                                    &reg,
                                                    option,
                                                    with_region.then(OnigRegion::new),
                                                    start,
                                                    &mp,
                                                );
                                                let result = search_in_range_inner(
                                                    &reg, &input, end, start, range, end, &mut msa,
                                                );
                                                NEGATED_STAR_FUSION_DISABLED.with(|v| v.set(false));
                                                (
                                                    result,
                                                    msa.region.map(|r| (r.num_regs, r.beg, r.end)),
                                                    msa.retry_limit_in_search_counter,
                                                )
                                            };
                                            assert_eq!(
                                                run(false),
                                                run(true),
                                                "{} {pattern}, {input:x?}, end={end}, start={start}, range={range}, option={option:?}, limits={retry}/{search_retry}/{stack}, region={with_region}",
                                                enc.name()
                                            );
                                            comparisons += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{comparisons} possessive negated-star result/capture/retry comparisons");
    }

    #[test]
    fn negated_star_fusion_keeps_configured_stack_limit_boundaries() {
        let _limits = shared_limits();
        let reg = compile_full(br#"\A(?:([^\"]*+)\"b)+z\z"#);
        let mut input = b"a\"b".repeat(400);
        input.push(b'z');
        let mut saw_limit = false;
        let mut saw_match = false;
        for limit in [0, 1, 319, 320, 321, 400, 800, 1200, 2000] {
            let mp = OnigMatchParam {
                match_stack_limit: limit,
                ..onig_new_match_param()
            };
            let run = |disabled| {
                NEGATED_STAR_FUSION_DISABLED.with(|v| v.set(disabled));
                let (result, region) = onig_search_with_param(
                    &reg,
                    &input,
                    input.len(),
                    0,
                    input.len(),
                    Some(OnigRegion::new()),
                    ONIG_OPTION_NONE,
                    &mp,
                );
                NEGATED_STAR_FUSION_DISABLED.with(|v| v.set(false));
                (result, region.map(|r| (r.num_regs, r.beg, r.end)))
            };
            let fused = run(false);
            assert_eq!(fused, run(true), "stack limit {limit}");
            saw_limit |= fused.0 == ONIGERR_MATCH_STACK_LIMIT_OVER;
            saw_match |= fused.0 == 0;
        }
        assert!(saw_limit && saw_match);
    }

    #[test]
    fn negated_byte_class_ascii_runs_preserve_captures_and_backtracking() {
        let _limits = shared_limits();
        for class in ["[^\"]", "[^qQ]", "[^\"\\\\]"] {
            let star = compile_full(format!("({class}*)(?=.b)").as_bytes());
            let counted = compile_full(format!("({class}{{0,512}})(?=.b)").as_bytes());
            assert!(has_opcode(&star, OpCode::CClassNotStar), "{class}");
            for prefix_len in [0, 31, 32, 33, 127] {
                for suffix in [
                    &b"xb\"b"[..],
                    b"xbQqb",
                    b"xb\0b",
                    b"\xc3\"xb",
                    b"\xc3\xa9\x80xb",
                    "éxb𝄞b".as_bytes(),
                ] {
                    let mut input = vec![b'a'; prefix_len];
                    input.extend_from_slice(suffix);
                    // Include a delimiter just outside the search extent and
                    // a backward search. A byte API start may split UTF-8.
                    for (start, range) in [
                        (0, input.len()),
                        (prefix_len, input.len()),
                        (input.len(), 0),
                        (0, prefix_len),
                    ] {
                        let run = |reg: &RegexType| {
                            let (pos, region) = onig_search(
                                reg,
                                &input,
                                input.len(),
                                start,
                                range,
                                Some(OnigRegion::new()),
                                ONIG_OPTION_NONE,
                            );
                            let region = region.unwrap();
                            (pos, region.beg, region.end)
                        };
                        assert_eq!(
                            run(&star),
                            run(&counted),
                            "{class}, {input:x?}, start={start}, range={range}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn populate_region_ignores_out_of_bounds_capture_stack_refs() {
        let (mut reg, _) = make_test_context();
        reg.num_mem = 1;
        reg.push_mem_start = 1;
        reg.push_mem_end = 1;

        let stack = vec![StackEntry::Alt {
            pcode: FINISH_PCODE,
            pstr: 0,
            zid: -1,
            is_super: false,
        }];
        let mem_start_stk = vec![MemPtr::invalid(), MemPtr::stack_idx(4)];
        let mem_end_stk = vec![MemPtr::invalid(), MemPtr::stack_idx(5)];

        let mut region = OnigRegion::new();
        populate_region_for_match(
            &mut region,
            &reg,
            1,
            0,
            1,
            &stack,
            &mem_start_stk,
            &mem_end_stk,
        )
        .unwrap();

        assert_eq!(region.beg[0], 0);
        assert_eq!(region.end[0], 1);
        assert_eq!(region.beg[1], ONIG_REGION_NOTPOS);
        assert_eq!(region.end[1], ONIG_REGION_NOTPOS);
    }

    #[test]
    fn is_in_code_range_fast_paths() {
        assert!(!is_in_code_range(&[], 0x41));
        assert!(!is_in_code_range(&[0], 0x41));

        let single = make_code_range_table(&[(0x80, 0x10FFFF)]);
        assert!(!is_in_code_range(&single, 0x7F));
        assert!(is_in_code_range(&single, 0x80));
        assert!(is_in_code_range(&single, 0x4E00));
        assert!(!is_in_code_range(&single, 0x110000));

        let small = make_code_range_table(&[(0x20, 0x2F), (0x40, 0x4F)]);
        assert!(!is_in_code_range(&small, 0x10));
        assert!(is_in_code_range(&small, 0x20));
        assert!(!is_in_code_range(&small, 0x35));
        assert!(is_in_code_range(&small, 0x45));

        let many = make_code_range_table(&[
            (0x0100, 0x010F),
            (0x0200, 0x020F),
            (0x0300, 0x030F),
            (0x0400, 0x040F),
            (0x0500, 0x050F),
        ]);
        assert!(!is_in_code_range(&many, 0x00FF));
        assert!(is_in_code_range(&many, 0x0405));
        assert!(!is_in_code_range(&many, 0x0600));
    }

    #[test]
    fn code_range_lookup_preserves_intervals_and_table_bounds() {
        // Sweep independently through compiled intervals, including surrogate
        // code points: raw encodings are not restricted to Rust `char` values.
        for pattern in [r"\p{L}", r"\p{M}", r"[\p{L}\p{M}]", r"\p{Greek}"] {
            let reg = compile_regex(pattern.as_bytes());
            let table =
                reg.ops
                    .iter()
                    .find_map(|op| match &op.payload {
                        OperationPayload::CClassMb { mb }
                        | OperationPayload::CClassMix { mb, .. } => Some(mb.as_slice()),
                        _ => None,
                    })
                    .expect("property must compile to a multibyte class");
            let n = table[0] as usize;
            let mut interval = 0;
            for code in 0..=0x110000 {
                while interval < n && table[interval * 2 + 2] < code {
                    interval += 1;
                }
                let expected = interval < n && table[interval * 2 + 1] <= code;
                assert_eq!(
                    is_in_code_range(table, code),
                    expected,
                    "{pattern}, {code:#x}"
                );
            }
            assert!(!is_in_code_range(table, u32::MAX));
        }

        // The declared count, not spare trailing words, bounds the table.
        // Exercise both sides of the small-table / binary-search boundary.
        for n in 0..=64 {
            let pairs: Vec<_> = (0..n).map(|i| (i * 7 + 2, i * 7 + 4)).collect();
            let table = make_code_range_table(&pairs);
            let mut extra = table.clone();
            extra.extend([0, u32::MAX]);
            for code in 0..=n * 7 + 7 {
                let expected = pairs.iter().any(|&(lo, hi)| lo <= code && code <= hi);
                assert_eq!(is_in_code_range(&table, code), expected);
                assert_eq!(is_in_code_range(&extra, code), expected);
            }
            for len in 0..table.len() {
                assert!(!is_in_code_range(&table[..len], 2));
            }
        }
        for table in [&[][..], &[0], &[0, 0, u32::MAX], &[u32::MAX, 0, u32::MAX]] {
            for code in [0, 1, 0x10FFFF, u32::MAX] {
                assert!(!is_in_code_range(table, code));
            }
        }
        let edge = make_code_range_table(&[(0, 0), (2, 2), (4, 4), (6, 6), (u32::MAX, u32::MAX)]);
        for code in [0, 2, 4, 6, u32::MAX] {
            assert!(is_in_code_range(&edge, code));
        }
        for code in [1, 3, 5, 7, u32::MAX - 1] {
            assert!(!is_in_code_range(&edge, code));
        }
    }

    #[test]
    fn cclass_mb_ascii_fast_path_matches_and_misses() {
        let _limits = shared_limits();
        let reg = compile_regex("[ぁ-ん]".as_bytes());
        assert!(
            has_opcode(&reg, OpCode::CClassMb),
            "expected CClassMb, got: {}",
            opcode_names(&reg)
        );

        let (ascii_miss, _) = compile_and_match("[ぁ-ん]".as_bytes(), b"a");
        assert_eq!(ascii_miss, ONIG_MISMATCH);

        let (multibyte_hit, _) = compile_and_match("[ぁ-ん]".as_bytes(), "あ".as_bytes());
        assert_eq!(multibyte_hit, "あ".len() as i32);
    }

    #[test]
    fn cclass_mb_not_ascii_fast_path_matches_and_misses() {
        let _limits = shared_limits();
        let reg = compile_regex("[^ぁ-ん]".as_bytes());
        assert!(
            has_opcode(&reg, OpCode::CClassMbNot) || has_opcode(&reg, OpCode::CClassMixNot),
            "expected CClassMbNot/CClassMixNot, got: {}",
            opcode_names(&reg)
        );

        let (ascii_hit, _) = compile_and_match("[^ぁ-ん]".as_bytes(), b"a");
        assert_eq!(ascii_hit, 1);

        let (multibyte_miss, _) = compile_and_match("[^ぁ-ん]".as_bytes(), "あ".as_bytes());
        assert_eq!(multibyte_miss, ONIG_MISMATCH);
    }

    #[test]
    fn logical_end_truncates_multibyte_decoding_and_matches_negated_classes() {
        let _limits = shared_limits();
        let truncated = [0xE2, 0x82];
        let reg = compile_regex(br"\S+");
        let (position, region) = onig_search(
            &reg,
            &truncated,
            truncated.len(),
            0,
            truncated.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(position, 0);
        assert_eq!(region.unwrap().end[0], truncated.len() as i32);

        // `\xC2\xA1` is U+00A1 (not a word character), but its leading byte
        // alone decodes as U+00C2 (a word character). This distinguishes the
        // logical end from the physical slice length.
        let split_character = [0xC2, 0xA1];
        let reg = compile_regex(br"\w");
        let (position, region) = onig_search(
            &reg,
            &split_character,
            1,
            0,
            1,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(position, 0);
        assert_eq!(region.unwrap().end[0], 1);
    }

    #[test]
    fn cclass_mix_and_star_fast_paths() {
        let _limits = shared_limits();
        let reg = compile_regex("[aぁ]".as_bytes());
        assert!(
            has_opcode(&reg, OpCode::CClassMix),
            "expected CClassMix, got: {}",
            opcode_names(&reg)
        );

        let (ascii_hit, _) = compile_and_match("[aぁ]".as_bytes(), b"a");
        assert_eq!(ascii_hit, 1);
        let (multibyte_hit, _) = compile_and_match("[aぁ]".as_bytes(), "ぁ".as_bytes());
        assert_eq!(multibyte_hit, "ぁ".len() as i32);

        let reg_star = compile_regex("[aぁ]*".as_bytes());
        assert!(
            has_opcode(&reg_star, OpCode::CClassMixStar),
            "expected CClassMixStar, got: {}",
            opcode_names(&reg_star)
        );
        let (star_hit, _) = compile_and_match("[aぁ]*".as_bytes(), "aaぁ".as_bytes());
        assert_eq!(star_hit, "aaぁ".len() as i32);
    }

    #[test]
    fn cclass_ascii_fast_kind_for_fold_pair() {
        let _limits = shared_limits();
        let reg = compile_regex("[xX]".as_bytes());
        let cclass_op = reg
            .ops
            .iter()
            .find(|op| op.opcode == OpCode::CClass)
            .expect("expected CClass opcode");
        if let OperationPayload::CClass { ascii_fast, .. } = cclass_op.payload {
            assert_eq!(ascii_fast, CClassAsciiFastKind::EqFoldLower(b'x'));
        } else {
            panic!("expected CClass payload");
        }

        let (lower_hit, _) = compile_and_match("[xX]".as_bytes(), b"x");
        assert_eq!(lower_hit, 1);
        let (upper_hit, _) = compile_and_match("[xX]".as_bytes(), b"X");
        assert_eq!(upper_hit, 1);
        let (miss, _) = compile_and_match("[xX]".as_bytes(), b"y");
        assert_eq!(miss, ONIG_MISMATCH);
    }

    #[test]
    fn cclass_not_ascii_fast_kind_for_fold_pair() {
        let _limits = shared_limits();
        let reg = compile_regex("[^xX]".as_bytes());
        let cclass_not_op = reg
            .ops
            .iter()
            .find(|op| op.opcode == OpCode::CClassNot)
            .expect("expected CClassNot opcode");
        if let OperationPayload::CClass { ascii_fast, .. } = cclass_not_op.payload {
            assert_eq!(ascii_fast, CClassAsciiFastKind::EqFoldLower(b'x'));
        } else {
            panic!("expected CClass payload");
        }

        let (miss, _) = compile_and_match("[^xX]".as_bytes(), b"X");
        assert_eq!(miss, ONIG_MISMATCH);
        let (hit, _) = compile_and_match("[^xX]".as_bytes(), b"y");
        assert_eq!(hit, 1);
    }

    #[test]
    fn cclass_star_ascii_fast_kind_for_fold_pair() {
        let _limits = shared_limits();
        let reg = compile_regex("[xX]*".as_bytes());
        let cclass_star_op = reg
            .ops
            .iter()
            .find(|op| op.opcode == OpCode::CClassStar)
            .expect("expected CClassStar opcode");
        if let OperationPayload::CClass { ascii_fast, .. } = cclass_star_op.payload {
            assert_eq!(ascii_fast, CClassAsciiFastKind::EqFoldLower(b'x'));
        } else {
            panic!("expected CClass payload");
        }

        let (star_hit, _) = compile_and_match("[xX]*".as_bytes(), b"XxXy");
        assert_eq!(star_hit, 3);
    }

    // ---- Basic literal matching ----

    #[test]
    fn match_literal_abc() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"abc", b"abc");
        assert_eq!(r, 3); // matched 3 bytes
    }

    #[test]
    fn match_literal_fail() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"abc", b"abd");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_literal_too_short() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"abcd", b"abc");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_empty_pattern() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"", b"abc");
        assert_eq!(r, 0); // empty pattern matches with length 0
    }

    #[test]
    fn match_single_char() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"x", b"xyz");
        assert_eq!(r, 1);
    }

    // ---- Dot (anychar) ----

    #[test]
    fn match_dot() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a.c", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_dot_no_newline() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a.c", b"a\nc");
        assert_eq!(r, ONIG_MISMATCH); // dot doesn't match newline
    }

    // ---- Alternation ----

    #[test]
    fn match_alternation_first() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a|b", b"a");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_alternation_second() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a|b", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_alternation_fail() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a|b", b"c");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Quantifiers ----

    #[test]
    fn match_star() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a*", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_star_empty() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a*", b"bbb");
        assert_eq!(r, 0); // a* can match empty
    }

    #[test]
    fn match_plus() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a+", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_plus_fail() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a+", b"bbb");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_question() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a?b", b"ab");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_question_without() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a?b", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_lazy_star() {
        let _limits = shared_limits();
        // a*? should match as few as possible from position 0
        let (r, _) = compile_and_match(b"a*?", b"aaa");
        assert_eq!(r, 0); // lazy: match empty
    }

    // ---- Character classes ----

    #[test]
    fn match_char_class() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"[abc]", b"b");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_fail() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"[abc]", b"d");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_char_class_range() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"[a-z]", b"m");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_negated() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"[^abc]", b"d");
        assert_eq!(r, 1);
    }

    #[test]
    fn match_char_class_negated_fail() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"[^abc]", b"a");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Anchors ----

    #[test]
    fn match_begin_anchor() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"^abc", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_end_anchor() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"abc$", b"abc");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_begin_end_anchors() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"^abc$", b"abc");
        assert_eq!(r, 3);
    }

    // ---- Capture groups ----

    #[test]
    fn match_capture_group() {
        let _limits = shared_limits();
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
        let _limits = shared_limits();
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
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"(?:abc)", b"abc");
        assert_eq!(r, 3);
    }

    // ---- Search (find anywhere in string) ----

    #[test]
    fn search_literal() {
        let _limits = shared_limits();
        let (pos, _) = compile_and_search(b"bc", b"abcdef");
        assert_eq!(pos, 1); // found at position 1
    }

    #[test]
    fn search_literal_not_found() {
        let _limits = shared_limits();
        let (pos, _) = compile_and_search(b"xyz", b"abcdef");
        assert_eq!(pos, ONIG_MISMATCH);
    }

    #[test]
    fn search_at_start() {
        let _limits = shared_limits();
        let (pos, _) = compile_and_search(b"abc", b"abcdef");
        assert_eq!(pos, 0);
    }

    #[test]
    fn search_at_end() {
        let _limits = shared_limits();
        let (pos, _) = compile_and_search(b"ef", b"abcdef");
        assert_eq!(pos, 4);
    }

    #[test]
    fn search_with_captures() {
        let _limits = shared_limits();
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
        let _limits = shared_limits();
        let (pos, _) = compile_and_search(b"a+", b"bbaab");
        assert_eq!(pos, 2);
    }

    // ---- Complex patterns ----

    #[test]
    fn match_word_boundary() {
        let _limits = shared_limits();
        let (pos, _) = compile_and_search(b"\\bfoo\\b", b"a foo b");
        assert_eq!(pos, 2);
    }

    /// The ASCII fast path of `WordBoundary` / `NoWordBoundary` decides as
    /// `is_word_boundary` does: every byte pair, at the start, between the
    /// bytes and at the end (including an end before the end of the data),
    /// in both word modes and both encodings.
    #[test]
    fn word_boundary_ascii_fast_path_matches_full_check() {
        for enc in [
            &crate::encodings::utf8::ONIG_ENCODING_UTF8 as OnigEncoding,
            &crate::encodings::ascii::ONIG_ENCODING_ASCII as OnigEncoding,
        ] {
            for mode in [0, 1] {
                assert_eq!(
                    is_word_boundary_ascii_fast(enc, b"", 0, 0, mode),
                    is_word_boundary(enc, b"", 0, 0, mode)
                );
                for a in 0..=u8::MAX {
                    for b in 0..=u8::MAX {
                        let data = [a, b];
                        for end in 0..=data.len() {
                            for s in 0..=end {
                                assert_eq!(
                                    is_word_boundary_ascii_fast(enc, &data, s, end, mode),
                                    is_word_boundary(enc, &data, s, end, mode),
                                    "bytes {a:#04x} {b:#04x}, s {s}, end {end}, mode {mode}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn match_complex_alternation() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"abc|def|ghi", b"def");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_nested_groups() {
        let _limits = shared_limits();
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
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a.*b", b"aXXXb");
        assert_eq!(r, 5);
    }

    #[test]
    fn match_backtracking() {
        let _limits = shared_limits();
        // Pattern: a.*b matches "axb" - requires backtracking
        let (r, _) = compile_and_match(b"a.*b", b"axb");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_quantifier() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a{3}", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_quantifier_fail() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a{3}", b"aa");
        assert_eq!(r, ONIG_MISMATCH);
    }

    #[test]
    fn match_interval_range() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a{2,4}", b"aaa");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_interval_range_min() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a{2,4}", b"aa");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_digit_class() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"\\d+", b"123");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_word_class() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"\\w+", b"abc123_");
        assert_eq!(r, 7);
    }

    #[test]
    fn search_email_like() {
        let _limits = shared_limits();
        let (pos, _) = compile_and_search(b"\\w+@\\w+", b"send to user@host ok");
        assert_eq!(pos, 8); // "user@host" starts at position 8
    }

    #[test]
    fn match_escaped_special() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"a\\.b", b"a.b");
        assert_eq!(r, 3);
    }

    #[test]
    fn match_back_reference() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"(a)\\1", b"aa");
        assert_eq!(r, 2);
    }

    #[test]
    fn match_back_reference_fail() {
        let _limits = shared_limits();
        let (r, _) = compile_and_match(b"(a)\\1", b"ab");
        assert_eq!(r, ONIG_MISMATCH);
    }

    // ---- Safety limits ----

    // Safety limit tests use a lock to avoid interfering with each other
    // (since limits are global statics).
    #[test]
    fn global_limit_setters_publish_a_new_revision() {
        let _limits = exclusive_limits();
        let old_retry_match = onig_get_retry_limit_in_match();
        let old_retry_search = onig_get_retry_limit_in_search();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        let mut revision = onig_get_global_limit_revision();

        onig_set_retry_limit_in_match(old_retry_match);
        assert_ne!(onig_get_global_limit_revision(), revision);
        revision = onig_get_global_limit_revision();
        onig_set_retry_limit_in_search(old_retry_search);
        assert_ne!(onig_get_global_limit_revision(), revision);
        revision = onig_get_global_limit_revision();
        onig_set_match_stack_limit(old_stack);
        assert_ne!(onig_get_global_limit_revision(), revision);
        revision = onig_get_global_limit_revision();
        onig_set_time_limit(old_time);
        assert_ne!(onig_get_global_limit_revision(), revision);
    }

    #[test]
    fn retry_limit_in_match() {
        let _limits = exclusive_limits();

        // (a+)*b against "aaa..." causes catastrophic backtracking. The tree
        // is compiled without tuning, so the loop has no empty check and its
        // body must consume.
        let (mut reg, mut env) = make_test_context();
        let pattern = b"(a+)*b";
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
            &reg,
            input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIGERR_RETRY_LIMIT_IN_MATCH_OVER);

        // Restore
        onig_set_retry_limit_in_match(old_retry);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);
    }

    /// The plain search path (process-wide limits, cached `MatchArg`) of the
    /// runaway loop in `tests/api_test.rs`
    /// (`guarded_pushes_spend_the_retry_budget_like_c`): C stops it with the
    /// retry limit at about 433k stack entries per 100k retries. A guarded
    /// push that skipped C's backtrack left three times as many.
    #[test]
    fn guarded_pushes_spend_the_retry_budget_on_the_plain_path() {
        let _limits = exclusive_limits();
        let reg = crate::regcomp::onig_new(
            br"((?=a\g<0>)|(?:\k<1>*?(?=a)()))*",
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &crate::regsyntax::OnigSyntaxOniguruma,
        )
        .unwrap();
        let input = b"aa";

        let old_retry = onig_get_retry_limit_in_match();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        onig_set_retry_limit_in_match(100_000);
        onig_set_match_stack_limit(1_000_000);
        onig_set_time_limit(0);

        for region in [Some(OnigRegion::new()), None] {
            let (result, _) = onig_search(
                &reg,
                input,
                input.len(),
                0,
                input.len(),
                region,
                ONIG_OPTION_NONE,
            );
            assert_eq!(result, ONIGERR_RETRY_LIMIT_IN_MATCH_OVER);
        }

        onig_set_retry_limit_in_match(old_retry);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);
    }

    #[test]
    fn stack_limit_over() {
        let _limits = exclusive_limits();

        // Use onig_match directly to avoid forward_search optimization
        let (mut reg, mut env) = make_test_context();
        let pattern = b".*.*.*.*.*";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        // Deeper than the 321 entries any limit below 320 allows.
        let input = &[b'a'; 400];

        // Set very low stack limit, disable retry limit
        let old_retry = onig_get_retry_limit_in_match();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        onig_set_retry_limit_in_match(0); // unlimited
        onig_set_match_stack_limit(20);
        onig_set_time_limit(0); // unlimited

        let (result, _) = onig_match(
            &reg,
            input,
            input.len(),
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIGERR_MATCH_STACK_LIMIT_OVER);

        // Restore
        onig_set_retry_limit_in_match(old_retry);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);
    }

    #[test]
    fn time_limit_over() {
        let _limits = exclusive_limits();

        // Untuned, so the loop body must consume (see retry_limit_in_match).
        let (mut reg, mut env) = make_test_context();
        let pattern = b"(a+)*b";
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
            &reg,
            input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIGERR_TIME_LIMIT_OVER);

        // Restore
        onig_set_retry_limit_in_match(old_retry);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);
    }

    #[test]
    fn retry_limit_in_search_caps_the_first_match_attempt() {
        let _limits = shared_limits();
        // C caps retry_limit_in_match by the search budget still left, so the
        // very first start position already stops at the search limit.
        let (mut reg, mut env) = make_test_context();
        let pattern = b"(a+)+b";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = vec![b'a'; 40];
        let mut mp = onig_new_match_param();
        // Other tests can change the global default while these params are created.
        mp.match_stack_limit = 0;
        mp.retry_limit_in_match = 0;
        mp.retry_limit_in_search = 1_000;
        mp.time_limit = 0;

        let started = Instant::now();
        let (result, _) = onig_search_with_param(
            &reg,
            &input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
            &mp,
        );
        assert_eq!(result, ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));

        // A tighter per-match limit still reports itself.
        mp.retry_limit_in_match = 100;
        let (result, _) = onig_search_with_param(
            &reg,
            &input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
            &mp,
        );
        assert_eq!(result, ONIGERR_RETRY_LIMIT_IN_MATCH_OVER);
    }

    #[test]
    fn time_limit_counts_backtracks_across_start_positions() {
        let _limits = shared_limits();
        // Each start position backtracks fewer than CHECK_TIME_INTERVAL times,
        // so a per-position counter never reads the clock. C keeps the counter
        // and the deadline per search (TIME_LIMIT_INIT, CHECK_TIME_LIMIT_IN_MATCH).
        let (mut reg, mut env) = make_test_context();
        let pattern = b"a{1,400}?(?=b)";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = vec![b'a'; 500_000];
        let mut mp = onig_new_match_param();
        // Isolate the timeout from stack limits changed by parallel tests.
        mp.match_stack_limit = 0;
        mp.retry_limit_in_match = 0;
        mp.retry_limit_in_search = 0;
        onig_set_time_limit_of_match_param(&mut mp, 10);

        let started = Instant::now();
        let (result, _) = onig_search_with_param(
            &reg,
            &input,
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
            &mp,
        );
        assert_eq!(result, ONIGERR_TIME_LIMIT_OVER);
        // Without the per-search counter this search runs for seconds.
        assert!(started.elapsed() < std::time::Duration::from_millis(1000));
    }

    #[test]
    fn limits_zero_means_unlimited() {
        let _limits = shared_limits();
        // Verify default limits (0 = unlimited) don't interfere with normal matching
        let (r, _) = compile_and_match(b"a*b", b"aaab");
        assert_eq!(r, 4);
    }

    // ---- Backward search ----

    #[test]
    fn backward_search_basic() {
        let _limits = shared_limits();
        // Search backward from end to find last occurrence
        let (mut reg, mut env) = make_test_context();
        let pattern = b"ab";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"xxabxxabxx"; // "ab" at positions 2 and 6
        // Backward search: start=10 (end), range=0 (beginning)
        let (result, _) = onig_search(
            &reg,
            input,
            input.len(),
            input.len(),
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        // Should find the last "ab" at position 6
        assert_eq!(result, 6);
    }

    #[test]
    fn backward_search_at_start() {
        let _limits = shared_limits();
        let (mut reg, mut env) = make_test_context();
        let pattern = b"he";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"hello";
        let (result, _) = onig_search(
            &reg,
            input,
            input.len(),
            input.len(),
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(result, 0);
    }

    #[test]
    fn backward_search_no_match() {
        let _limits = shared_limits();
        let (mut reg, mut env) = make_test_context();
        let pattern = b"xyz";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        let r = regcomp::compile_from_tree(&root, &mut reg, &env);
        assert_eq!(r, 0);

        let input = b"hello world";
        let (result, _) = onig_search(
            &reg,
            input,
            input.len(),
            input.len(),
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(result, ONIG_MISMATCH);
    }

    /// Compiles `pattern` as UTF-8 with the given compile options.
    fn compile_utf8(pattern: &[u8], option: OnigOptionType) -> RegexType {
        regcomp::onig_new(
            pattern,
            option,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &crate::regsyntax::OnigSyntaxOniguruma,
        )
        .unwrap()
    }

    /// Runs `search` with the pure literal fast path on or off.
    fn with_pure_literal_fast_path<T>(enabled: bool, search: impl FnOnce() -> T) -> T {
        PURE_LITERAL_FAST_PATH_DISABLED.with(|v| v.set(!enabled));
        let out = search();
        PURE_LITERAL_FAST_PATH_DISABLED.with(|v| v.set(false));
        out
    }

    #[test]
    fn pure_literal_flag_follows_the_program() {
        let _limits = shared_limits();
        let cases: &[(&[u8], OnigOptionType, bool)] = &[
            (b"GET", ONIG_OPTION_NONE, true),
            (b"a", ONIG_OPTION_NONE, true),
            (b"12345678901", ONIG_OPTION_NONE, true),
            // Multibyte string ops (`StrMb*`) keep the VM.
            ("caf\u{e9}".as_bytes(), ONIG_OPTION_NONE, false),
            (b"", ONIG_OPTION_NONE, false),
            (b"GET", ONIG_OPTION_IGNORECASE, false),
            (b"(?i)get", ONIG_OPTION_NONE, false),
            (b"G.T", ONIG_OPTION_NONE, false),
            (b"GET$", ONIG_OPTION_NONE, false),
            (b"^GET", ONIG_OPTION_NONE, false),
            (b"(GET)", ONIG_OPTION_NONE, false),
            (b"GE+T", ONIG_OPTION_NONE, false),
            (b"GET|PUT", ONIG_OPTION_NONE, false),
            (b"GET(?=X)", ONIG_OPTION_NONE, false),
            (b"GET\\b", ONIG_OPTION_NONE, false),
            (b"G\\Ket", ONIG_OPTION_NONE, false),
        ];
        for &(pattern, option, expected) in cases {
            let reg = compile_utf8(pattern, option);
            assert_eq!(
                reg.pure_literal,
                expected,
                "{}",
                String::from_utf8_lossy(pattern)
            );
        }
    }

    #[test]
    fn pure_literal_search_matches_the_vm_path() {
        let _limits = shared_limits();
        let patterns: &[&[u8]] = &[b"GET", b"a", b"aa", b"ab", b"12345678901"];
        let subjects: &[&[u8]] = &[
            b"",
            b"G",
            b"GET",
            b"xGETGET",
            b"GETGE",
            b"aaaa",
            b"aabaab",
            b"12345678901 1234567890",
            b"ab\xffab\xc3",
        ];
        let options = [
            ONIG_OPTION_NONE,
            ONIG_OPTION_FIND_LONGEST,
            ONIG_OPTION_FIND_NOT_EMPTY,
            ONIG_OPTION_MATCH_WHOLE_STRING,
            ONIG_OPTION_CHECK_VALIDITY_OF_STRING,
        ];
        for pattern in patterns {
            let reg = compile_utf8(pattern, ONIG_OPTION_NONE);
            assert!(reg.pure_literal, "{}", String::from_utf8_lossy(pattern));
            for subject in subjects {
                // A subject cut at `end` checks the right range as well.
                for end in 0..=subject.len() {
                    let text = &subject[..end];
                    for start in 0..=end {
                        for range in 0..=end {
                            for option in options {
                                // A narrower start range with the whole subject as the
                                // right range: the RegSet route (`onig_search_with_msa_and_right_range`).
                                let narrowed = |enabled| {
                                    with_pure_literal_fast_path(enabled, || {
                                        let mut msa = MatchArg::new(
                                            &reg,
                                            option,
                                            Some(OnigRegion::new()),
                                            start,
                                        );
                                        let (result, region) = onig_search_with_msa_and_right_range(
                                            &reg, text, end, start, range, end, None, &mut msa,
                                        );
                                        (result, region.map(|r| (r.num_regs, r.beg, r.end)))
                                    })
                                };
                                assert_eq!(
                                    narrowed(true),
                                    narrowed(false),
                                    "{} on {:?} end {end} start {start} range {range} option {option:?} narrowed",
                                    String::from_utf8_lossy(pattern),
                                    String::from_utf8_lossy(text),
                                );
                                // With and without a region, as `is_match` searches.
                                for with_region in [true, false] {
                                    let run = |enabled| {
                                        with_pure_literal_fast_path(enabled, || {
                                            let region = with_region.then(OnigRegion::new);
                                            let (result, region) = onig_search(
                                                &reg, text, end, start, range, region, option,
                                            );
                                            (result, region.map(|r| (r.num_regs, r.beg, r.end)))
                                        })
                                    };
                                    assert_eq!(
                                        run(true),
                                        run(false),
                                        "{} on {:?} end {end} start {start} range {range} option {option:?} region {with_region}",
                                        String::from_utf8_lossy(pattern),
                                        String::from_utf8_lossy(text),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn pure_literal_bounds_and_region_match_the_vm_path() {
        let _limits = shared_limits();
        let reg = compile_utf8(b"GET", ONIG_OPTION_NONE);
        let text = b"xGETGET GE";
        for start in 0..=text.len() {
            let bounds = |enabled| {
                with_pure_literal_fast_path(enabled, || {
                    let mut msa = MatchArg::new(&reg, ONIG_OPTION_NONE, None, start);
                    onig_search_bounds(&reg, text, start, &mut msa).unwrap()
                })
            };
            assert_eq!(bounds(true), bounds(false), "start {start}");
        }
    }

    #[test]
    fn pure_literal_bounds_keep_the_validity_check() {
        let _limits = shared_limits();
        // The validity option comes with the compile options, so the bounds
        // search (which takes `reg.options`) sees it as well.
        let reg = compile_utf8(b"ab", ONIG_OPTION_CHECK_VALIDITY_OF_STRING);
        assert!(reg.pure_literal);
        let text = b"ab\xff";
        let bounds = |enabled| {
            with_pure_literal_fast_path(enabled, || {
                let mut msa = MatchArg::new(&reg, ONIG_OPTION_NONE, None, 0);
                onig_search_bounds(&reg, text, 0, &mut msa)
            })
        };
        assert_eq!(bounds(true), bounds(false));
        assert_eq!(bounds(false), Err(ONIGERR_INVALID_WIDE_CHAR_VALUE));
    }

    #[test]
    fn pure_literal_search_keeps_the_vm_for_limits_it_cannot_skip() {
        let _limits = exclusive_limits();
        let reg = compile_utf8(b"GET", ONIG_OPTION_NONE);
        let text = b"xxGET";
        // A matching search answers without an attempt in the VM.
        VM_ATTEMPTS.with(|v| v.set(0));
        let (result, _) = onig_search(
            &reg,
            text,
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        assert_eq!(result, 2);
        assert_eq!(VM_ATTEMPTS.with(std::cell::Cell::get), 0);

        // The disabled path runs the VM, which finds the same match.
        VM_ATTEMPTS.with(|v| v.set(0));
        let (result, _) = with_pure_literal_fast_path(false, || {
            onig_search(
                &reg,
                text,
                text.len(),
                0,
                text.len(),
                None,
                ONIG_OPTION_NONE,
            )
        });
        assert_eq!(result, 2);
        assert_eq!(VM_ATTEMPTS.with(std::cell::Cell::get), 1);

        // Under a search retry limit, the answer is still the same.
        onig_set_retry_limit_in_search(1);
        let limited = onig_search(
            &reg,
            text,
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        onig_set_retry_limit_in_search(0);
        assert_eq!(limited.0, 2);
    }

    #[test]
    fn forward_search_rejects_min_distance_beyond_the_slice() {
        let (mut reg, mut env) = make_test_context();
        let pattern = b"a";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        assert_eq!(regcomp::compile_from_tree(&root, &mut reg, &env), 0);

        // This models optimizer metadata whose minimum distance exceeds the
        // available input, without constructing an invalid C pointer range.
        reg.dist_min = OnigLen::MAX - 1;
        assert_eq!(forward_search(&reg, b"a", 1, 0, 1), None);
    }

    #[test]
    fn backward_search_saturates_distance_bounds_at_the_slice_start() {
        let (mut reg, mut env) = make_test_context();
        let pattern = b"ab";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        assert_eq!(regcomp::compile_from_tree(&root, &mut reg, &env), 0);

        reg.optimize = OptimizeType::Str;
        reg.exact = pattern.to_vec();
        reg.dist_min = OnigLen::MAX - 1;
        reg.dist_max = OnigLen::MAX - 1;
        assert_eq!(backward_search(&reg, b"ab", 2, 0, 0, 0), Some((0, 0)));
    }

    #[test]
    fn optimized_search_bounds_large_finite_distances_to_the_slice() {
        let _limits = shared_limits();
        let (mut reg, mut env) = make_test_context();
        let pattern = b"a";
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env).unwrap();
        assert_eq!(regcomp::compile_from_tree(&root, &mut reg, &env), 0);
        reg.dist_max = OnigLen::MAX - 1;

        let input = b"a";
        let (forward, _) = onig_search(
            &reg,
            input,
            input.len(),
            0,
            input.len(),
            None,
            ONIG_OPTION_NONE,
        );
        assert_eq!(forward, 0);

        let (backward, _) = onig_search(
            &reg,
            input,
            input.len(),
            input.len(),
            0,
            None,
            ONIG_OPTION_NONE,
        );
        assert_eq!(backward, 0);
    }

    fn callout_args_at<'a>(
        reg: &'a RegexType,
        subject: &[u8],
        data: &'a RefCell<Vec<[i64; ONIG_CALLOUT_DATA_SLOT_NUM]>>,
        callout_in: OnigCalloutIn,
        num: i32,
    ) -> OnigCalloutArgs<'a> {
        OnigCalloutArgs::new(
            callout_in,
            0,
            num,
            reg,
            subject,
            subject.len(),
            0,
            subject.len(),
            0,
            0,
        )
        .with_callout_data(data)
    }

    /// The global callout slots start empty and keep the callback they were
    /// last given.
    #[test]
    fn global_callback_slot_is_empty_until_set_then_keeps_the_last_callback() {
        let slot: GlobalCallbackSlot<OnigCalloutFunc> = GlobalCallbackSlot::new();
        assert!(slot.get().is_none());

        let fail: OnigCalloutFunc = onig_builtin_fail;
        let mismatch: OnigCalloutFunc = onig_builtin_mismatch;
        slot.set(fail);
        assert_eq!(slot.get().map(|f| f as usize), Some(fail as usize));
        slot.set(mismatch);
        assert_eq!(slot.get().map(|f| f as usize), Some(mismatch as usize));
    }

    /// The `*_by_callout_args` accessors report the regex and the subject
    /// positions they were built from, and reach the callout data only through
    /// the cell attached to the arguments.
    #[test]
    fn callout_args_accessors_read_the_regex_and_the_attached_callout_data() {
        let reg = compile_regex(br"a(*COUNT)(*MAX{3})");
        let list = &reg.extp.as_ref().unwrap().callout_list;
        assert_eq!(list.len(), 2);
        let max_args = list[1].args.len() as i32;

        let subject = b"abc";
        let data = RefCell::new(vec![[0i64; ONIG_CALLOUT_DATA_SLOT_NUM]; 2]);
        let args =
            OnigCalloutArgs::new(OnigCalloutIn::Progress, 4, 2, &reg, subject, 3, 1, 3, 2, 7)
                .with_callout_data(&data);

        assert_eq!(onig_get_callout_num_by_callout_args(&args), 2);
        assert_eq!(onig_get_name_id_by_callout_args(&args), 4);
        assert!(matches!(
            onig_get_callout_in_by_callout_args(&args),
            OnigCalloutIn::Progress
        ));
        assert_eq!(onig_get_args_num_by_callout_args(&args), max_args);
        assert_eq!(
            onig_get_passed_args_num_by_callout_args(&args),
            list[1].passed_args_num
        );
        assert_eq!(
            onig_get_passed_args_num_by_callout_args(&args),
            max_args - 1
        );
        assert_eq!(
            onig_get_arg_by_callout_args(&args, 0).is_some(),
            max_args > 0
        );
        assert!(onig_get_arg_by_callout_args(&args, max_args).is_none());
        assert!(onig_get_contents_by_callout_args(&args).is_none());
        assert!(onig_get_contents_end_by_callout_args(&args).is_null());
        assert_eq!(onig_get_retry_counter_by_callout_args(&args), 7);
        assert_eq!(
            onig_get_regex_by_callout_args(&args),
            &reg as *const RegexType
        );

        let base = subject.as_ptr();
        assert_eq!(onig_get_string_by_callout_args(&args), base);
        assert_eq!(
            onig_get_string_end_by_callout_args(&args),
            base.wrapping_add(3)
        );
        assert_eq!(onig_get_start_by_callout_args(&args), base.wrapping_add(1));
        assert_eq!(
            onig_get_right_range_by_callout_args(&args),
            base.wrapping_add(3)
        );
        assert_eq!(
            onig_get_current_by_callout_args(&args),
            base.wrapping_add(2)
        );

        assert_eq!(onig_get_callout_data_by_callout_args(&args, 2, 0), Some(0));
        assert_eq!(
            onig_set_callout_data_by_callout_args(&args, 2, 1, 42),
            ONIG_NORMAL
        );
        assert_eq!(onig_get_callout_data_by_callout_args(&args, 2, 1), Some(42));
        assert_eq!(
            onig_get_callout_data_by_callout_args_self(&args, 1),
            Some(42)
        );
        assert_eq!(onig_get_callout_data_by_callout_args(&args, 3, 0), None);
        assert_eq!(data.borrow()[1][1], 42);

        // Without an attached cell the callout data is unreachable.
        let bare =
            OnigCalloutArgs::new(OnigCalloutIn::Progress, 0, 2, &reg, subject, 3, 1, 3, 2, 0);
        assert_eq!(onig_get_callout_data_by_callout_args(&bare, 2, 0), None);
        assert_eq!(
            onig_set_callout_data_by_callout_args(&bare, 2, 0, 1),
            ONIGERR_INVALID_ARGUMENT
        );
        assert_eq!(
            onig_builtin_count(&bare, std::ptr::null_mut()),
            ONIG_CALLOUT_FAIL
        );
        assert_eq!(
            onig_builtin_max(&bare, std::ptr::null_mut()),
            ONIG_CALLOUT_FAIL
        );
        assert_eq!(
            onig_builtin_cmp(&bare, std::ptr::null_mut()),
            ONIG_CALLOUT_FAIL
        );
    }

    /// The count and max builtins record progress and retraction calls in the
    /// attached callout data, and max stops passing once its limit is reached.
    #[test]
    fn count_and_max_builtins_update_the_attached_callout_data() {
        let reg = compile_regex(br"a(*COUNT)(*MAX{3})");
        let subject = b"abc";
        let data = RefCell::new(vec![[0i64; ONIG_CALLOUT_DATA_SLOT_NUM]; 2]);
        let user = std::ptr::null_mut();

        assert_eq!(
            onig_builtin_count(
                &callout_args_at(&reg, subject, &data, OnigCalloutIn::Progress, 1),
                user
            ),
            ONIG_CALLOUT_SUCCESS
        );
        assert_eq!(
            onig_builtin_count(
                &callout_args_at(&reg, subject, &data, OnigCalloutIn::Retraction, 1),
                user
            ),
            ONIG_CALLOUT_SUCCESS
        );
        assert_eq!(data.borrow()[0][..3], [1, 1, 1]);

        for _ in 0..3 {
            assert_eq!(
                onig_builtin_max(
                    &callout_args_at(&reg, subject, &data, OnigCalloutIn::Progress, 2),
                    user
                ),
                ONIG_CALLOUT_SUCCESS
            );
        }
        assert_eq!(
            onig_builtin_max(
                &callout_args_at(&reg, subject, &data, OnigCalloutIn::Progress, 2),
                user
            ),
            ONIG_CALLOUT_FAIL
        );
        assert_eq!(data.borrow()[1][0], 3);
    }

    /// The SKIP builtin records the current offset in the attached skip target,
    /// never moves it backwards, and ignores retraction.
    #[test]
    fn skip_builtin_records_the_current_offset_in_the_attached_target() {
        let reg = compile_regex(br"a(*COUNT)");
        let subject = b"abcd";
        let user = std::ptr::null_mut();
        let skip = Cell::new(0usize);
        let at = |callout_in, current| {
            OnigCalloutArgs::new(callout_in, 0, 1, &reg, subject, 4, 0, 4, current, 0)
                .with_skip_search(&skip)
        };

        assert_eq!(
            onig_builtin_skip(&at(OnigCalloutIn::Progress, 2), user),
            ONIG_CALLOUT_SUCCESS
        );
        assert_eq!(skip.get(), 2);

        assert_eq!(
            onig_builtin_skip(&at(OnigCalloutIn::Progress, 1), user),
            ONIG_CALLOUT_SUCCESS
        );
        assert_eq!(skip.get(), 2);

        assert_eq!(
            onig_builtin_skip(&at(OnigCalloutIn::Retraction, 3), user),
            ONIG_CALLOUT_SUCCESS
        );
        assert_eq!(skip.get(), 2);

        let bare =
            OnigCalloutArgs::new(OnigCalloutIn::Progress, 0, 1, &reg, subject, 4, 0, 4, 3, 0);
        assert_eq!(onig_builtin_skip(&bare, user), ONIG_CALLOUT_SUCCESS);
        assert_eq!(skip.get(), 2);
    }

    /// Patterns whose first checks anchor them at the search start: `\A` and
    /// `\G` with and without optimizer maps, literal prefixes, distances,
    /// alternations, look-arounds, `\K`, folds and multibyte classes.
    const ANCHORED_PATTERNS: [&str; 22] = [
        r"\Aab",
        r"\Aa|\Ab",
        r"\A(a)(b?)",
        r"\Ax*",
        r"\A.",
        r"\A(?=b)",
        r"\A\b",
        r"\A[^a]+",
        r"\Aé+",
        r"\A\w+X",
        r"(?i)\Aabc",
        r"\A\d{1,3}\.",
        r"\A.{0,3}x",
        r"\A(?:ab|a)c",
        r"\A(?!x)a",
        r"\A\Kab",
        r"\Gab",
        r"\G(?<=a)b",
        r"\Gb|\Ga",
        r"\G[^\n]*\n",
        r"\A(a|b)\1",
        r"\A\x{1F600}|é",
    ];

    /// Subjects for the anchored patterns: empty, short, multibyte, with a
    /// newline, and one malformed UTF-8 sequence.
    const ANCHORED_SUBJECTS: [&[u8]; 14] = [
        b"",
        b"a",
        b"ab",
        b"abc",
        b"xab",
        b"aab",
        "é".as_bytes(),
        "éab".as_bytes(),
        "aéb".as_bytes(),
        b"ab\nab",
        b"x\n",
        b"1.x",
        "\u{1F600}a\u{e9}".as_bytes(),
        b"a\xffb",
    ];

    /// The search of `reg` with the anchored-start path on or off, returning
    /// the result and the bounds of every register.
    fn anchored_search(
        reg: &RegexType,
        subject: &[u8],
        start: usize,
        range: usize,
        option: OnigOptionType,
        enabled: bool,
    ) -> (i32, Vec<i32>, Vec<i32>) {
        ANCHORED_START_DISABLED.with(|v| v.set(!enabled));
        let (r, region) = onig_search(
            reg,
            subject,
            subject.len(),
            start,
            range,
            Some(OnigRegion::new()),
            option,
        );
        ANCHORED_START_DISABLED.with(|v| v.set(false));
        match region {
            Some(region) if r >= 0 => {
                let len = region.num_regs as usize;
                (r, region.beg[..len].to_vec(), region.end[..len].to_vec())
            }
            _ => (r, Vec::new(), Vec::new()),
        }
    }

    /// Anchored-start searches equal the general path (the test toggle) at
    /// every start and range, under the options the search reads, on
    /// multibyte and malformed subjects, and through the bounds-only path.
    #[test]
    fn anchored_start_search_matches_the_general_path() {
        let _limits = shared_limits();
        let options = [
            ONIG_OPTION_NONE,
            ONIG_OPTION_FIND_NOT_EMPTY,
            ONIG_OPTION_FIND_LONGEST,
            ONIG_OPTION_MATCH_WHOLE_STRING,
        ];
        for pattern in ANCHORED_PATTERNS {
            let reg = compile_full(pattern.as_bytes());
            for subject in ANCHORED_SUBJECTS {
                for option in options {
                    for start in 0..=subject.len() {
                        for range in start + 1..=subject.len() {
                            let fast = anchored_search(&reg, subject, start, range, option, true);
                            let general =
                                anchored_search(&reg, subject, start, range, option, false);
                            assert_eq!(
                                fast, general,
                                "{pattern} on {subject:?} {start}..{range} option {option:?}"
                            );
                        }
                    }
                }
                // The bounds-only path (`find`) takes the core directly.
                if reg.keep_moves_match_start {
                    continue;
                }
                for option in [ONIG_OPTION_NONE, ONIG_OPTION_FIND_NOT_EMPTY] {
                    for start in 0..=subject.len() {
                        let bounds = |enabled: bool| {
                            ANCHORED_START_DISABLED.with(|v| v.set(!enabled));
                            let mut msa = take_cached_msa(&reg, option, None, start, None);
                            let found = onig_search_bounds(&reg, subject, start, &mut msa);
                            cache_msa(msa);
                            ANCHORED_START_DISABLED.with(|v| v.set(false));
                            found
                        };
                        assert_eq!(
                            bounds(true),
                            bounds(false),
                            "{pattern} bounds on {subject:?} from {start} option {option:?}"
                        );
                    }
                }
            }
        }
    }

    /// The result and the registers of a search that starts with a region
    /// holding stale values, `primed`: the direct capture path, or `onig_search`
    /// with the anchored-start path on or off (the test toggle).
    fn capture_search(
        reg: &RegexType,
        subject: &[u8],
        start: usize,
        mode: CaptureSearch,
    ) -> (i32, Vec<i32>, Vec<i32>, bool) {
        let mut primed = OnigRegion::new();
        primed.resize(6);
        primed.beg.fill(7);
        primed.end.fill(7);
        primed.history_root = Some(Box::new(OnigCaptureTreeNode::new()));
        let end = subject.len();
        let (r, region) = match mode {
            CaptureSearch::Direct => onig_search_captures(reg, subject, start, primed),
            CaptureSearch::General | CaptureSearch::GeneralWithoutAnchor => {
                ANCHORED_START_DISABLED
                    .with(|v| v.set(mode == CaptureSearch::GeneralWithoutAnchor));
                let found = onig_search(
                    reg,
                    subject,
                    end,
                    start,
                    end,
                    Some(primed),
                    ONIG_OPTION_NONE,
                );
                ANCHORED_START_DISABLED.with(|v| v.set(false));
                found
            }
        };
        let region = region.expect("a capture search returns its region");
        let len = region.num_regs as usize;
        (
            r,
            region.beg[..len].to_vec(),
            region.end[..len].to_vec(),
            region.history_root.is_none(),
        )
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum CaptureSearch {
        Direct,
        General,
        GeneralWithoutAnchor,
    }

    /// The direct capture path (`onig_search_captures`) returns the match, every
    /// register and the empty history of the general path, at every start, for
    /// the anchored patterns and the subjects, over a region that holds stale
    /// registers. The general path is `onig_search` with the anchored-start path,
    /// and with it off (the test toggle).
    #[test]
    fn anchored_captures_match_the_general_path() {
        let _limits = shared_limits();
        for pattern in ANCHORED_PATTERNS {
            let reg = compile_full(pattern.as_bytes());
            for subject in ANCHORED_SUBJECTS {
                for start in 0..=subject.len() {
                    let direct = capture_search(&reg, subject, start, CaptureSearch::Direct);
                    let general = capture_search(&reg, subject, start, CaptureSearch::General);
                    let plain =
                        capture_search(&reg, subject, start, CaptureSearch::GeneralWithoutAnchor);
                    let context = format!("{pattern} on {subject:?} from {start}");
                    assert_eq!(direct, general, "{context}");
                    assert_eq!(direct, plain, "{context}");
                }
            }
        }
    }

    /// `\A` at a later start is a mismatch that makes no attempt, as the general
    /// path finds it: under a search retry limit an attempt would be counted and
    /// reported as the limit's error.
    #[test]
    fn anchored_captures_at_a_later_begin_buf_make_no_attempt() {
        let _limits = exclusive_limits();
        let reg = compile_full(br"\Ab");
        onig_set_retry_limit_in_search(1);
        let (r, _) = onig_search_captures(&reg, b"ab", 1, OnigRegion::new());
        assert_eq!(r, ONIG_MISMATCH);
        let (general, _) = onig_search(
            &reg,
            b"ab",
            2,
            1,
            2,
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(general, ONIG_MISMATCH);
    }

    /// A pattern compiled with FIND_LONGEST keeps the general path, as the
    /// anchored-start path does for it; the direct path answers as that loop does.
    #[test]
    fn anchored_captures_of_a_find_longest_pattern_match_the_general_path() {
        let _limits = shared_limits();
        for pattern in [r"\Aab", r"\A(a|ab)", r"\A(a|ab)(c|bcd)?", r"\Gab|\Gabc"] {
            let reg = crate::regcomp::onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_FIND_LONGEST,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .expect("pattern compiles");
            for subject in [&b"ab"[..], b"abc", b"abcd", b"a", b""] {
                for start in 0..=subject.len() {
                    let direct = capture_search(&reg, subject, start, CaptureSearch::Direct);
                    let general =
                        capture_search(&reg, subject, start, CaptureSearch::GeneralWithoutAnchor);
                    assert_eq!(direct, general, "{pattern} on {subject:?} from {start}");
                }
            }
        }
    }

    /// A search with no match leaves the registers of the region cleared to
    /// `ONIG_REGION_NOTPOS`, as the core loop leaves them. The direct path clears
    /// only in that case, so a stale region must not show through.
    #[test]
    fn anchored_captures_clear_the_region_when_nothing_matches() {
        let _limits = shared_limits();
        let reg = compile_full(br"\A(a)(b)c");
        let subject = b"xxxx";
        let (r, beg, end, _) = capture_search(&reg, subject, 0, CaptureSearch::Direct);
        assert_eq!(r, ONIG_MISMATCH);
        assert_eq!(beg, vec![ONIG_REGION_NOTPOS; 3]);
        assert_eq!(end, vec![ONIG_REGION_NOTPOS; 3]);
    }

    /// `\A` cannot match from a later start, and the fast path answers that
    /// without an attempt; an unanchored pattern from the same start matches.
    #[test]
    fn begin_buf_pattern_does_not_match_a_later_start() {
        let _limits = shared_limits();
        let subject = b"xab";
        let anchored = compile_full(br"\Aab");
        assert_eq!(
            anchored_search(&anchored, subject, 1, 3, ONIG_OPTION_NONE, true).0,
            ONIG_MISMATCH
        );
        let plain = compile_full(br"ab");
        assert_eq!(
            anchored_search(&plain, subject, 1, 3, ONIG_OPTION_NONE, true).0,
            1
        );
    }

    /// Anchored-start searches equal C's `onig_search`, its match and every
    /// group, at every start and range.
    #[cfg(feature = "ffi")]
    #[test]
    fn anchored_start_search_matches_c() {
        let _limits = shared_limits();
        // FIND_LONGEST is left to the general path (see `anchored_start_search`).
        let options = [ONIG_OPTION_NONE, ONIG_OPTION_FIND_NOT_EMPTY];
        for pattern in ANCHORED_PATTERNS {
            let reg = compile_full(pattern.as_bytes());
            let c =
                crate::ffi::CRegex::new(pattern.as_bytes(), crate::ffi::ONIG_OPTION_NONE).unwrap();
            for subject in ANCHORED_SUBJECTS {
                // Valid UTF-8 only: the malformed subject is the general
                // path's own comparison above.
                if std::str::from_utf8(subject).is_err() {
                    continue;
                }
                for option in options {
                    for start in 0..=subject.len() {
                        if start < subject.len() && !is_char_head(subject, start) {
                            continue;
                        }
                        for range in start + 1..=subject.len() {
                            let (r, region) = onig_search(
                                &reg,
                                subject,
                                subject.len(),
                                start,
                                range,
                                Some(OnigRegion::new()),
                                option,
                            );
                            let mut c_region = crate::ffi::CRegion::new();
                            let c_r =
                                c.search(subject, start, range, Some(&mut c_region), option.bits());
                            let context =
                                format!("{pattern} on {subject:?} {start}..{range} {option:?}");
                            assert_eq!(r, c_r, "{context}");
                            if r >= 0 {
                                let region = region.unwrap();
                                let spans: Vec<(i32, i32)> = region
                                    .beg
                                    .iter()
                                    .zip(&region.end)
                                    .map(|(&b, &e)| (b, e))
                                    .take(region.num_regs as usize)
                                    .collect();
                                assert_eq!(spans, c_region.capture_ranges(), "{context}");
                            }
                        }
                    }
                }
            }
        }
    }

    /// Whether `at` is the start of a UTF-8 character in `text`.
    #[cfg(feature = "ffi")]
    fn is_char_head(text: &[u8], at: usize) -> bool {
        text.get(at).is_none_or(|&byte| (byte as i8) >= -0x40)
    }
}
