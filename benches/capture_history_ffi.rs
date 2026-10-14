//! Benchmark-only ownership and tree copying for the native capture-history
//! adapter. Its C helper is excluded from the published crate.

use std::os::raw::{c_int, c_uint};
use std::ptr;

use ferroni::ffi;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CaptureHistoryNode {
    pub group: i32,
    pub beg: i32,
    pub end: i32,
    pub children: Vec<CaptureHistoryNode>,
}

#[repr(C)]
struct COnigCaptureTreeNode {
    group: c_int,
    beg: c_int,
    end: c_int,
    allocated: c_int,
    num_childs: c_int,
    childs: *mut *mut COnigCaptureTreeNode,
}

/// Copy a C-owned tree before its region is reused or freed.
///
/// # Safety
/// `node` must point to a live tree from an exclusively owned Oniguruma region.
unsafe fn copy_capture_history(node: *const COnigCaptureTreeNode) -> CaptureHistoryNode {
    // SAFETY: the caller keeps the region and all of its tree nodes live.
    let node = unsafe { &*node };
    let children = if node.num_childs <= 0 {
        Vec::new()
    } else {
        // SAFETY: Oniguruma initializes `num_childs` pointers in this array;
        // every child belongs to the same live region tree.
        unsafe { std::slice::from_raw_parts(node.childs, node.num_childs as usize) }
            .iter()
            .map(|child| {
                // SAFETY: each child is a live node in the borrowed tree.
                unsafe { copy_capture_history(*child) }
            })
            .collect()
    };
    CaptureHistoryNode {
        group: node.group,
        beg: node.beg,
        end: node.end,
        children,
    }
}

unsafe extern "C" {
    fn ferroni_onig_new_capture_history(
        reg: *mut ffi::OnigRegex,
        pattern: *const u8,
        length: usize,
        option: c_uint,
        einfo: *mut ffi::OnigErrorInfo,
    ) -> c_int;
}

pub(super) struct CaptureHistoryRegex {
    raw: ffi::OnigRegex,
}

impl CaptureHistoryRegex {
    pub fn new(pattern: &[u8], option: c_uint) -> Result<Self, c_int> {
        let _instance = ffi::COnigInstance::new();
        let mut raw = ptr::null_mut();
        let mut error_info = ffi::OnigErrorInfo {
            enc: ptr::null(),
            par: ptr::null(),
            par_end: ptr::null(),
        };
        // SAFETY: the pattern slice and writable output storage live through
        // the call. The helper enables history in a per-regex syntax copy.
        let result = unsafe {
            ferroni_onig_new_capture_history(
                &mut raw,
                pattern.as_ptr(),
                pattern.len(),
                option,
                &mut error_info,
            )
        };
        if result != 0 {
            return Err(result);
        }
        Ok(Self { raw })
    }

    pub fn trace(&self, text: &[u8]) -> Vec<CaptureHistoryNode> {
        let mut trace = Vec::new();
        // SAFETY: Oniguruma has been initialized by the regex constructor.
        let region = OwnedRegion(unsafe { ffi::onig_region_new() });
        assert!(
            !region.0.is_null(),
            "C capture-history region allocation failed"
        );
        let mut start = 0;
        while start < text.len() {
            // SAFETY: the compiled regex and exclusively owned region are
            // live. Slice-derived pointers cover valid subject offsets.
            let pos = unsafe {
                ffi::onig_search(
                    self.raw,
                    text.as_ptr(),
                    text.as_ptr_range().end,
                    text[start..].as_ptr(),
                    text.as_ptr_range().end,
                    region.0,
                    ffi::ONIG_OPTION_NONE,
                )
            };
            assert!(pos >= -1, "C capture-history search failed: {pos}");
            if pos < 0 {
                break;
            }
            // SAFETY: the non-null region is live and Oniguruma initialized
            // its history root during the successful search.
            let root = unsafe { (*region.0).history_root.cast::<COnigCaptureTreeNode>() };
            assert!(
                !root.is_null(),
                "C capture-history pattern returned no tree"
            );
            // SAFETY: the region owns the live tree; it cannot be reused or
            // freed until this owned copy is complete.
            let node = unsafe { copy_capture_history(root) };
            assert!(node.end > node.beg && node.beg >= start as i32);
            start = node.end as usize;
            trace.push(node);
        }
        trace
    }
}

impl Drop for CaptureHistoryRegex {
    fn drop(&mut self) {
        // SAFETY: a successful compile transferred ownership of this handle;
        // this is its sole release path.
        unsafe { ffi::onig_free(self.raw) }
    }
}

struct OwnedRegion(*mut ffi::OnigRegion);

impl Drop for OwnedRegion {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: this region was allocated by Oniguruma and has one owner.
            unsafe { ffi::onig_region_free(self.0, 1) }
        }
    }
}
