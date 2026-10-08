//! The process-wide progress, retraction and each-match callbacks.
//
// C: onig_set_progress_callout, onig_set_retraction_callout and
// onig_set_callback_each_match store one function for the whole process, and
// onig_initialize_match_param copies the progress and retraction callbacks
// into every match parameter built afterwards. None of the setters can unset a
// callback again, so this file holds a single test that runs in a process of
// its own, as tests/subexp_call_limits.rs does for the call limits.

use ferroni::oniguruma::{ONIG_NORMAL, OnigRegion};
use ferroni::regexec::{
    OnigCallbackEachMatchFunc, OnigCalloutFunc, onig_builtin_fail, onig_builtin_mismatch,
    onig_get_callback_each_match, onig_get_progress_callout, onig_get_retraction_callout,
    onig_new_match_param, onig_set_callback_each_match, onig_set_progress_callout,
    onig_set_retraction_callout,
};

/// The callbacks round trip through their setters, and match parameters built
/// afterwards carry the progress and retraction callbacks.
#[test]
fn global_callout_callbacks_round_trip_through_their_setters() {
    fn each_match(_: &[u8], _: &OnigRegion, _: *mut std::ffi::c_void) -> i32 {
        ONIG_NORMAL
    }
    let progress: OnigCalloutFunc = onig_builtin_fail;
    let retraction: OnigCalloutFunc = onig_builtin_mismatch;
    let each: OnigCallbackEachMatchFunc = each_match;

    assert!(onig_get_progress_callout().is_none());
    assert!(onig_get_retraction_callout().is_none());
    assert!(onig_get_callback_each_match().is_none());

    assert_eq!(onig_set_progress_callout(progress), ONIG_NORMAL);
    assert_eq!(onig_set_retraction_callout(retraction), ONIG_NORMAL);
    assert_eq!(onig_set_callback_each_match(each), ONIG_NORMAL);

    assert_eq!(
        onig_get_progress_callout().map(|f| f as usize),
        Some(progress as usize)
    );
    assert_eq!(
        onig_get_retraction_callout().map(|f| f as usize),
        Some(retraction as usize)
    );
    assert_eq!(
        onig_get_callback_each_match().map(|f| f as usize),
        Some(each as usize)
    );

    let mp = onig_new_match_param();
    assert_eq!(
        mp.progress_callout.map(|f| f as usize),
        Some(progress as usize)
    );
    assert_eq!(
        mp.retraction_callout.map(|f| f as usize),
        Some(retraction as usize)
    );
}
