// FFI bindings for C Oniguruma (benchmark comparison only)
//
// Minimal surface: just enough to compile patterns, run searches/matches,
// and manage regions+regsets. Gated behind `ffi` Cargo feature.

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_uint};
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

pub const ONIG_REGSET_POSITION_LEAD: c_int = 0;
pub const ONIG_REGSET_REGEX_LEAD: c_int = 1;

// --- Extern functions ---

extern "C" {
    pub static OnigEncodingUTF8: OnigEncodingType;
    pub static OnigSyntaxOniguruma: OnigSyntaxType;

    pub fn onig_initialize(encodings: *const OnigEncoding, number_of_encodings: c_int) -> c_int;

    pub fn onig_end() -> c_int;

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

/// One-time init/end lifecycle for C Oniguruma.
pub struct COnigInstance;

impl COnigInstance {
    pub fn new() -> Self {
        C_INIT.call_once(|| unsafe {
            let enc = &OnigEncodingUTF8 as OnigEncoding;
            let r = onig_initialize(&enc as *const OnigEncoding, 1);
            assert!(r == 0, "onig_initialize failed: {r}");
        });
        COnigInstance
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
        let str_ptr = text.as_ptr();
        let end_ptr = unsafe { str_ptr.add(text.len()) };
        let start_ptr = unsafe { str_ptr.add(start) };
        let range_ptr = unsafe { str_ptr.add(range) };
        let region_ptr = region.map_or(ptr::null_mut(), |r| r.raw);
        unsafe {
            onig_search(
                self.raw, str_ptr, end_ptr, start_ptr, range_ptr, region_ptr, option,
            )
        }
    }

    pub fn match_at(
        &self,
        text: &[u8],
        at: usize,
        region: Option<&mut CRegion>,
        option: c_uint,
    ) -> c_int {
        let str_ptr = text.as_ptr();
        let end_ptr = unsafe { str_ptr.add(text.len()) };
        let at_ptr = unsafe { str_ptr.add(at) };
        let region_ptr = region.map_or(ptr::null_mut(), |r| r.raw);
        unsafe { onig_match(self.raw, str_ptr, end_ptr, at_ptr, region_ptr, option) }
    }

    pub fn raw(&self) -> OnigRegex {
        self.raw
    }
}

impl Drop for CRegex {
    fn drop(&mut self) {
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
            raw: unsafe { onig_region_new() },
        }
    }

    pub fn clear(&mut self) {
        unsafe { onig_region_clear(self.raw) }
    }
}

impl Drop for CRegion {
    fn drop(&mut self) {
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
    /// freed before the RegSet. Use `into_raw()` on CRegex to transfer ownership.
    pub fn new(regs: &[OnigRegex]) -> Result<Self, c_int> {
        let _inst = COnigInstance::new();
        let mut set: *mut OnigRegSetType = ptr::null_mut();
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
        let str_ptr = text.as_ptr();
        let end_ptr = unsafe { str_ptr.add(text.len()) };
        let start_ptr = unsafe { str_ptr.add(start) };
        let range_ptr = unsafe { str_ptr.add(range) };
        let mut match_pos: c_int = -1;
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
        unsafe { onig_regset_free(self.raw) }
    }
}
