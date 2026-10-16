// FFI bindings for C Oniguruma (benchmark comparison only)
//
// Minimal surface: just enough to compile patterns, run searches/matches,
// and manage regions+regsets. Gated behind `ffi` Cargo feature.

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::c_void;
use std::os::raw::{c_int, c_uint, c_ulong};
use std::ptr;
use std::sync::Once;

// --- Opaque types ---

#[repr(C)]
pub struct OnigRegexType {
    _opaque: [u8; 0],
}
pub type OnigRegex = *mut OnigRegexType;

#[repr(C)]
pub struct OnigSyntaxType {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct OnigEncodingType {
    _opaque: [u8; 0],
}
pub type OnigEncoding = *const OnigEncodingType;

#[repr(C)]
pub struct OnigRegSetType {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct OnigMatchParamType {
    _opaque: [u8; 0],
}

// --- OnigRegion ---

#[repr(C)]
pub struct OnigRegion {
    pub allocated: c_int,
    pub num_regs: c_int,
    pub beg: *mut c_int,
    pub end: *mut c_int,
    pub history_root: *mut c_void,
}

// --- OnigErrorInfo ---

#[repr(C)]
pub struct OnigErrorInfo {
    pub enc: OnigEncoding,
    pub par: *const u8,
    pub par_end: *const u8,
}

// --- Constants ---

pub const ONIG_OPTION_NONE: c_uint = 0;
pub const ONIG_OPTION_IGNORECASE: c_uint = 1;
pub const ONIG_OPTION_CAPTURE_GROUP: c_uint = 1 << 8;

pub const ONIG_REGSET_POSITION_LEAD: c_int = 0;
pub const ONIG_REGSET_REGEX_LEAD: c_int = 1;

// --- Process-wide limits, for the tests of the counted repeats ---

/// Sets C's retry limit in match for every later search (process-wide).
pub fn set_retry_limit_in_match(limit: u64) {
    // SAFETY: the setter stores one process-wide limit read by later searches.
    unsafe { onig_set_retry_limit_in_match(limit as c_ulong) };
}

/// C's retry limit in match (process-wide).
pub fn retry_limit_in_match() -> u64 {
    // SAFETY: the getter reads one process-wide value.
    unsafe { onig_get_retry_limit_in_match() as u64 }
}

/// Sets C's match stack limit for every later search (process-wide).
pub fn set_match_stack_limit_size(size: u32) {
    // SAFETY: the setter stores one process-wide limit read by later searches.
    unsafe { onig_set_match_stack_limit_size(size as c_uint) };
}

/// C's match stack limit (process-wide).
pub fn match_stack_limit_size() -> u32 {
    // SAFETY: the getter reads one process-wide value.
    unsafe { onig_get_match_stack_limit_size() as u32 }
}

// --- Extern functions ---

unsafe extern "C" {
    pub static OnigEncodingUTF8: OnigEncodingType;
    pub static OnigSyntaxOniguruma: OnigSyntaxType;

    pub fn onig_initialize(encodings: *const OnigEncoding, number_of_encodings: c_int) -> c_int;

    pub fn onig_end() -> c_int;

    pub fn onig_set_default_case_fold_flag(case_fold_flag: c_uint) -> c_int;
    pub fn onig_set_retry_limit_in_match(n: c_ulong) -> c_int;
    pub fn onig_set_match_stack_limit_size(size: c_uint) -> c_int;
    pub fn onig_get_retry_limit_in_match() -> c_ulong;
    pub fn onig_get_match_stack_limit_size() -> c_uint;

    pub fn onig_new(
        reg: *mut OnigRegex,
        pattern: *const u8,
        pattern_end: *const u8,
        option: c_uint,
        enc: OnigEncoding,
        syntax: *const OnigSyntaxType,
        einfo: *mut OnigErrorInfo,
    ) -> c_int;

    pub fn onig_free(reg: OnigRegex);

    pub fn onig_search(
        reg: OnigRegex,
        str: *const u8,
        end: *const u8,
        start: *const u8,
        range: *const u8,
        region: *mut OnigRegion,
        option: c_uint,
    ) -> c_int;

    pub fn onig_match(
        reg: OnigRegex,
        str: *const u8,
        end: *const u8,
        at: *const u8,
        region: *mut OnigRegion,
        option: c_uint,
    ) -> c_int;

    pub fn onig_new_match_param() -> *mut OnigMatchParamType;

    pub fn onig_free_match_param(mp: *mut OnigMatchParamType);

    pub fn onig_set_retry_limit_in_match_of_match_param(
        mp: *mut OnigMatchParamType,
        limit: c_ulong,
    ) -> c_int;

    pub fn onig_set_match_stack_limit_size_of_match_param(
        mp: *mut OnigMatchParamType,
        limit: c_uint,
    ) -> c_int;

    pub fn onig_search_with_param(
        reg: OnigRegex,
        str: *const u8,
        end: *const u8,
        start: *const u8,
        range: *const u8,
        region: *mut OnigRegion,
        option: c_uint,
        mp: *mut OnigMatchParamType,
    ) -> c_int;

    pub fn onig_region_new() -> *mut OnigRegion;
    pub fn onig_region_free(region: *mut OnigRegion, free_self: c_int);
    pub fn onig_region_clear(region: *mut OnigRegion);

    pub fn onig_regset_new(
        rset: *mut *mut OnigRegSetType,
        n: c_int,
        regs: *const OnigRegex,
    ) -> c_int;

    pub fn onig_regset_free(set: *mut OnigRegSetType);

    pub fn onig_regset_search(
        set: *mut OnigRegSetType,
        str: *const u8,
        end: *const u8,
        start: *const u8,
        range: *const u8,
        lead: c_int,
        option: c_uint,
        rmatch_pos: *mut c_int,
    ) -> c_int;
}

// --- RAII wrappers ---

static C_INIT: Once = Once::new();

/// Return C-compatible subject pointers after validating every caller-supplied
/// offset. Slice-derived pointers are valid at the corresponding byte position
/// (including the one-past-the-end position), without raw pointer arithmetic.
fn text_pointers(
    text: &[u8],
    start: usize,
    range: usize,
) -> Option<(*const u8, *const u8, *const u8, *const u8)> {
    if start > text.len() || range > text.len() {
        return None;
    }
    Some((
        text.as_ptr(),
        text.as_ptr_range().end,
        text[start..].as_ptr(),
        text[range..].as_ptr(),
    ))
}

/// Return a valid subject pointer for a caller-supplied match offset.
fn text_pointer_at(text: &[u8], at: usize) -> Option<(*const u8, *const u8, *const u8)> {
    if at > text.len() {
        return None;
    }
    Some((text.as_ptr(), text.as_ptr_range().end, text[at..].as_ptr()))
}

/// One-time init/end lifecycle for C Oniguruma.
pub struct COnigInstance;

impl COnigInstance {
    pub fn new() -> Self {
        // SAFETY: the imported encoding symbol and `onig_initialize` have the
        // declared C ABI; `enc` points to the static encoding object for this
        // one-time initialization call.
        C_INIT.call_once(|| unsafe {
            let enc = &OnigEncodingUTF8 as OnigEncoding;
            let r = onig_initialize(&enc as *const OnigEncoding, 1);
            assert!(r == 0, "onig_initialize failed: {r}");
        });
        COnigInstance
    }
}

impl Default for COnigInstance {
    fn default() -> Self {
        Self::new()
    }
}

/// Compiled C regex with auto-free on drop.
pub struct CRegex {
    raw: OnigRegex,
}

impl CRegex {
    pub fn new(pattern: &[u8], option: c_uint) -> Result<Self, c_int> {
        let _inst = COnigInstance::new();
        let mut reg: OnigRegex = ptr::null_mut();
        let mut einfo = OnigErrorInfo {
            enc: ptr::null(),
            par: ptr::null(),
            par_end: ptr::null(),
        };
        // SAFETY: pattern is a live Rust slice, so its start and one-past-end
        // pointers delimit a valid byte range for the duration of this call;
        // all other pointers refer to initialized local storage or C statics.
        let r = unsafe {
            onig_new(
                &mut reg,
                pattern.as_ptr(),
                pattern.as_ptr().add(pattern.len()),
                option,
                &OnigEncodingUTF8 as OnigEncoding,
                &OnigSyntaxOniguruma as *const OnigSyntaxType,
                &mut einfo,
            )
        };
        if r != 0 {
            return Err(r);
        }
        Ok(CRegex { raw: reg })
    }

    pub fn search(
        &self,
        text: &[u8],
        start: usize,
        range: usize,
        region: Option<&mut CRegion>,
        option: c_uint,
    ) -> c_int {
        let Some((str_ptr, end_ptr, start_ptr, range_ptr)) = text_pointers(text, start, range)
        else {
            return -1;
        };
        let region_ptr = region.map_or(ptr::null_mut(), |r| r.raw);
        // SAFETY: `self.raw` is owned by this wrapper, the subject pointers
        // were derived from validated slice offsets, and `region_ptr` is null
        // or an owned C region that outlives the call.
        unsafe {
            onig_search(
                self.raw, str_ptr, end_ptr, start_ptr, range_ptr, region_ptr, option,
            )
        }
    }

    /// `search` with a retry limit per match, through a match parameter
    /// (`onig_search_with_param`), which leaves the process-wide limits alone.
    pub fn search_with_retry_limit(
        &self,
        text: &[u8],
        start: usize,
        range: usize,
        region: Option<&mut CRegion>,
        option: c_uint,
        retry_limit_in_match: u64,
    ) -> c_int {
        let Some((str_ptr, end_ptr, start_ptr, range_ptr)) = text_pointers(text, start, range)
        else {
            return -1;
        };
        let region_ptr = region.map_or(ptr::null_mut(), |r| r.raw);
        // SAFETY: `self.raw` is owned by this wrapper, the subject pointers
        // were derived from validated slice offsets, `region_ptr` is null or
        // an owned C region that outlives the call, and the match parameter
        // is allocated, used and freed within this block.
        unsafe {
            let mp = onig_new_match_param();
            assert!(!mp.is_null(), "onig_new_match_param failed");
            onig_set_retry_limit_in_match_of_match_param(mp, retry_limit_in_match as c_ulong);
            let r = onig_search_with_param(
                self.raw, str_ptr, end_ptr, start_ptr, range_ptr, region_ptr, option, mp,
            );
            onig_free_match_param(mp);
            r
        }
    }

    /// `search` with a match stack limit, through a match parameter
    /// (`onig_search_with_param`), which leaves the process-wide limits alone.
    pub fn search_with_match_stack_limit(
        &self,
        text: &[u8],
        start: usize,
        range: usize,
        region: Option<&mut CRegion>,
        option: c_uint,
        match_stack_limit: u32,
    ) -> c_int {
        let Some((str_ptr, end_ptr, start_ptr, range_ptr)) = text_pointers(text, start, range)
        else {
            return -1;
        };
        let region_ptr = region.map_or(ptr::null_mut(), |r| r.raw);
        // SAFETY: as in `search_with_retry_limit`: owned regex, validated
        // subject pointers, a null or owned region, and a match parameter
        // allocated, used and freed within this block.
        unsafe {
            let mp = onig_new_match_param();
            assert!(!mp.is_null(), "onig_new_match_param failed");
            onig_set_match_stack_limit_size_of_match_param(mp, match_stack_limit as c_uint);
            let r = onig_search_with_param(
                self.raw, str_ptr, end_ptr, start_ptr, range_ptr, region_ptr, option, mp,
            );
            onig_free_match_param(mp);
            r
        }
    }

    pub fn match_at(
        &self,
        text: &[u8],
        at: usize,
        region: Option<&mut CRegion>,
        option: c_uint,
    ) -> c_int {
        let Some((str_ptr, end_ptr, at_ptr)) = text_pointer_at(text, at) else {
            return -1;
        };
        let region_ptr = region.map_or(ptr::null_mut(), |r| r.raw);
        // SAFETY: `self.raw` is owned by this wrapper, the subject pointers
        // were derived from a validated slice offset, and `region_ptr` is null
        // or an owned C region that outlives the call.
        unsafe { onig_match(self.raw, str_ptr, end_ptr, at_ptr, region_ptr, option) }
    }

    pub fn raw(&self) -> OnigRegex {
        self.raw
    }
}

impl Drop for CRegex {
    fn drop(&mut self) {
        // SAFETY: a successful `onig_new` returned this owned handle exactly
        // once, and Drop is its sole release path.
        unsafe { onig_free(self.raw) }
    }
}

/// C region with auto-free on drop.
pub struct CRegion {
    raw: *mut OnigRegion,
}

impl CRegion {
    pub fn new() -> Self {
        CRegion {
            // SAFETY: the C allocator returns an Oniguruma-owned region handle
            // suitable for the matching free and clear functions below.
            raw: unsafe { onig_region_new() },
        }
    }

    pub fn clear(&mut self) {
        // SAFETY: `self.raw` is the live region allocated by `CRegion::new`.
        unsafe { onig_region_clear(self.raw) }
    }

    /// Copy capture bounds for differential tests and benchmark validation.
    pub fn capture_ranges(&self) -> Vec<(i32, i32)> {
        let (beg, end) = self.capture_bounds();
        beg.iter().copied().zip(end.iter().copied()).collect()
    }

    /// Whole-match bounds without allocating a vector of subgroup captures.
    pub fn match_range(&self) -> Option<(i32, i32)> {
        let (beg, end) = self.capture_bounds();
        Some((*beg.first()?, *end.first()?))
    }

    fn capture_bounds(&self) -> (&[i32], &[i32]) {
        // SAFETY: this wrapper owns the live region. Oniguruma initializes
        // both arrays for num_regs entries; an empty region can have null
        // arrays, so do not construct slices until its length is positive.
        unsafe {
            let region = &*self.raw;
            if region.num_regs <= 0 {
                return (&[], &[]);
            }
            let len = region.num_regs as usize;
            let beg = std::slice::from_raw_parts(region.beg, len);
            let end = std::slice::from_raw_parts(region.end, len);
            (beg, end)
        }
    }
}

impl Default for CRegion {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CRegion {
    fn drop(&mut self) {
        // SAFETY: `self.raw` is released exactly once by this owning wrapper.
        unsafe { onig_region_free(self.raw, 1) }
    }
}

/// C RegSet with auto-free on drop.
pub struct CRegSet {
    raw: *mut OnigRegSetType,
}

impl CRegSet {
    /// Create a new RegSet from pre-compiled CRegex handles.
    /// IMPORTANT: The caller must keep the CRegex objects alive; the
    /// C library does NOT copy them. The CRegex objects must NOT be
    /// freed before the RegSet.
    pub fn new(regs: &[OnigRegex]) -> Result<Self, c_int> {
        let _inst = COnigInstance::new();
        let mut set: *mut OnigRegSetType = ptr::null_mut();
        // SAFETY: `regs` remains live for the call, and `set` is writable local
        // storage for the C API to initialize.
        let r = unsafe { onig_regset_new(&mut set, regs.len() as c_int, regs.as_ptr()) };
        if r != 0 {
            return Err(r);
        }
        Ok(CRegSet { raw: set })
    }

    pub fn search(
        &mut self,
        text: &[u8],
        start: usize,
        range: usize,
        lead: c_int,
        option: c_uint,
    ) -> (c_int, c_int) {
        let Some((str_ptr, end_ptr, start_ptr, range_ptr)) = text_pointers(text, start, range)
        else {
            return (-1, -1);
        };
        let mut match_pos: c_int = -1;
        // SAFETY: `self.raw` is owned by this wrapper, subject pointers have
        // validated offsets, and `match_pos` is writable local storage.
        let idx = unsafe {
            onig_regset_search(
                self.raw,
                str_ptr,
                end_ptr,
                start_ptr,
                range_ptr,
                lead,
                option,
                &mut match_pos,
            )
        };
        (idx, match_pos)
    }
}

impl Drop for CRegSet {
    fn drop(&mut self) {
        // SAFETY: `self.raw` is released exactly once by this owning wrapper.
        unsafe { onig_regset_free(self.raw) }
    }
}

// --- vscode-oniguruma C Scanner ---
//
// FFI to the extracted scanner from vscode-oniguruma (benches/vscode_scanner_native.c).

/// Opaque C scanner type (OnigScanner_ struct).
#[repr(C)]
pub struct COnigScanner {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    fn createOnigScanner(
        patterns: *const *mut u8,
        lengths: *const c_int,
        count: c_int,
        options: c_int,
        syntax: *const OnigSyntaxType,
    ) -> *mut COnigScanner;

    fn findNextOnigScannerMatch(
        scanner: *mut COnigScanner,
        str_cache_id: c_int,
        str_data: *const u8,
        str_length: c_int,
        position: c_int,
        options: c_int,
    ) -> *const c_int;

    fn freeOnigScanner(scanner: *mut COnigScanner);
}

/// RAII wrapper around the vscode-oniguruma C scanner.
pub struct CScanner {
    handle: *mut COnigScanner,
    /// Owned copies of pattern data (must outlive the scanner).
    _patterns: Vec<Vec<u8>>,
}

impl CScanner {
    /// Create a new C scanner from pattern byte slices.
    pub fn new(patterns: &[&[u8]]) -> Result<Self, c_int> {
        let _inst = COnigInstance::new();
        let mut owned: Vec<Vec<u8>> = patterns.iter().map(|p| p.to_vec()).collect();
        let ptrs: Vec<*mut u8> = owned.iter_mut().map(|v| v.as_mut_ptr()).collect();
        let lengths: Vec<c_int> = patterns.iter().map(|p| p.len() as c_int).collect();

        // SAFETY: the pattern-pointer and length arrays reference owned local
        // vectors that remain live for the constructor call; the syntax is a
        // valid imported C static.
        let handle = unsafe {
            createOnigScanner(
                ptrs.as_ptr(),
                lengths.as_ptr(),
                patterns.len() as c_int,
                ONIG_OPTION_CAPTURE_GROUP as c_int,
                &OnigSyntaxOniguruma as *const OnigSyntaxType,
            )
        };
        if handle.is_null() {
            return Err(-1);
        }
        Ok(CScanner {
            handle,
            _patterns: owned,
        })
    }

    /// Find the next match. Returns `(pattern_index, [(beg, end), ...])` or `None`.
    pub fn find_next_match(
        &self,
        text: &[u8],
        str_cache_id: i32,
        position: usize,
    ) -> Option<(usize, Vec<(i32, i32)>)> {
        self.find_next_match_with_options(text, str_cache_id, position, ONIG_OPTION_NONE)
    }

    /// [`Self::find_next_match`] with Oniguruma search options, as
    /// vscode-oniguruma passes them for its `FindOption` flags.
    pub fn find_next_match_with_options(
        &self,
        text: &[u8],
        str_cache_id: i32,
        position: usize,
        options: c_uint,
    ) -> Option<(usize, Vec<(i32, i32)>)> {
        self.find_next_match_encoded(text, str_cache_id, position, options, |index, bounds| {
            let captures = bounds
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[beg, end]| (beg, end));
            (index, captures.collect())
        })
    }

    /// [`Self::find_next_match_with_options`] without copying the result:
    /// `read` gets the pattern index and the capture bounds
    /// `[beg0, end0, beg1, end1, ...]` in the scanner's own result buffer,
    /// as vscode-oniguruma's JavaScript side reads them.
    pub fn find_next_match_encoded<R>(
        &self,
        text: &[u8],
        str_cache_id: i32,
        position: usize,
        options: c_uint,
        read: impl FnOnce(usize, &[c_int]) -> R,
    ) -> Option<R> {
        // SAFETY: `self.handle` is owned by this wrapper and `text` remains
        // live for the synchronous C call.
        let encoded = unsafe {
            findNextOnigScannerMatch(
                self.handle,
                str_cache_id as c_int,
                text.as_ptr(),
                text.len() as c_int,
                position as c_int,
                options as c_int,
            )
        };
        if encoded.is_null() {
            return None;
        }
        // The encoded region: [index, num_regs, beg0, end0, beg1, end1, ...]
        // SAFETY: a non-null result is the scanner's documented encoded region
        // layout, containing `num_regs` begin/end pairs following its header.
        // The buffer is only rewritten by the next search, after `read`.
        unsafe {
            let index = *encoded as usize;
            let num_regs = *encoded.add(1) as usize;
            Some(read(
                index,
                std::slice::from_raw_parts(encoded.add(2), 2 * num_regs),
            ))
        }
    }
}

impl Drop for CScanner {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is released exactly once by this owning wrapper.
        unsafe {
            freeOnigScanner(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_pointers_reject_out_of_bounds_offsets() {
        let text = b"abc";
        assert!(text_pointers(text, 0, text.len()).is_some());
        assert!(text_pointers(text, text.len(), 0).is_some());
        assert!(text_pointers(text, text.len() + 1, 0).is_none());
        assert!(text_pointers(text, 0, text.len() + 1).is_none());
        assert!(text_pointer_at(text, text.len()).is_some());
        assert!(text_pointer_at(text, text.len() + 1).is_none());
    }
}
