//! Execution tests for named, contents, match-param, and each-match callbacks.
//!
//! The process-wide callback setters cannot unset their values, so this file
//! holds one test and runs in its own integration-test process.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};

use ferroni::encodings::{ascii::ONIG_ENCODING_ASCII, utf8::ONIG_ENCODING_UTF8};
use ferroni::oniguruma::{
    CalloutArg, ONIG_ABORT, ONIG_CALLOUT_IN_BOTH, ONIG_MISMATCH, ONIG_NORMAL,
    ONIG_OPTION_CALLBACK_EACH_MATCH, ONIG_OPTION_FIND_LONGEST, ONIG_OPTION_NONE,
    ONIGERR_INVALID_ARGUMENT, ONIGERR_INVALID_CALLOUT_BODY, ONIGERR_UNDEFINED_CALLOUT_NAME,
    OnigCalloutIn, OnigCalloutType, OnigRegion, OnigSyntaxType, OnigType, OnigValue,
};
use ferroni::regcomp::onig_new;
use ferroni::regexec::{
    OnigCallbackEachMatchFunc, OnigCalloutArgs, onig_get_arg_by_callout_args,
    onig_get_args_num_by_callout_args, onig_get_callback_each_match,
    onig_get_callout_in_by_callout_args, onig_get_contents_by_callout_args,
    onig_get_name_id_by_callout_args, onig_get_passed_args_num_by_callout_args,
    onig_get_progress_callout, onig_get_retraction_callout, onig_match, onig_match_with_param,
    onig_new_match_param, onig_search, onig_search_with_param, onig_set_callback_each_match,
    onig_set_callout_user_data_of_match_param, onig_set_progress_callout,
    onig_set_progress_callout_of_match_param, onig_set_retraction_callout,
    onig_set_retraction_callout_of_match_param,
};
use ferroni::regparse::{onig_get_callout_name_by_name_id, onig_set_callout_of_name};
use ferroni::regsyntax::OnigSyntaxOniguruma;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallEvent {
    source: &'static str,
    direction: OnigCalloutIn,
    name_id: i32,
    args_num: i32,
    passed_args_num: i32,
    first_arg: Option<i64>,
    contents: Option<Vec<u8>>,
    user_data: usize,
}

static EVENTS: Mutex<Vec<CallEvent>> = Mutex::new(Vec::new());
static EACH_MATCHES: Mutex<Vec<(i32, i32)>> = Mutex::new(Vec::new());
static NAMED_RESULT: AtomicI32 = AtomicI32::new(ONIG_NORMAL);
static GLOBAL_PROGRESS_RESULT: AtomicI32 = AtomicI32::new(ONIG_NORMAL);
static RETRACTION_RESULT: AtomicI32 = AtomicI32::new(ONIG_NORMAL);
static EACH_MATCH_RESULT: AtomicI32 = AtomicI32::new(ONIG_NORMAL);
static END_CALLS: AtomicI32 = AtomicI32::new(0);

fn record_callout(source: &'static str, args: &OnigCalloutArgs, user_data: *mut c_void) -> i32 {
    let first_arg = match onig_get_arg_by_callout_args(args, 0) {
        Some(CalloutArg::Long(value)) => Some(*value),
        _ => None,
    };
    EVENTS.lock().unwrap().push(CallEvent {
        source,
        direction: onig_get_callout_in_by_callout_args(args),
        name_id: onig_get_name_id_by_callout_args(args),
        args_num: onig_get_args_num_by_callout_args(args),
        passed_args_num: onig_get_passed_args_num_by_callout_args(args),
        first_arg,
        contents: onig_get_contents_by_callout_args(args).map(<[u8]>::to_vec),
        user_data: user_data as usize,
    });
    ONIG_NORMAL
}

fn named_callback(args: &OnigCalloutArgs, user_data: *mut c_void) -> i32 {
    let direction = onig_get_callout_in_by_callout_args(args);
    let _ = record_callout("named", args, user_data);
    if direction == OnigCalloutIn::Retraction {
        RETRACTION_RESULT.load(Ordering::Relaxed)
    } else {
        NAMED_RESULT.load(Ordering::Relaxed)
    }
}

fn named_end_callback(_: &OnigCalloutArgs, _: *mut c_void) -> i32 {
    END_CALLS.fetch_add(1, Ordering::Relaxed);
    ONIG_NORMAL
}

fn global_progress(args: &OnigCalloutArgs, user_data: *mut c_void) -> i32 {
    let _ = record_callout("global-progress", args, user_data);
    GLOBAL_PROGRESS_RESULT.load(Ordering::Relaxed)
}

fn global_retraction(args: &OnigCalloutArgs, user_data: *mut c_void) -> i32 {
    record_callout("global-retraction", args, user_data)
}

fn param_progress(args: &OnigCalloutArgs, user_data: *mut c_void) -> i32 {
    record_callout("param-progress", args, user_data)
}

fn param_retraction(args: &OnigCalloutArgs, user_data: *mut c_void) -> i32 {
    record_callout("param-retraction", args, user_data)
}

fn each_match(_: &[u8], region: &OnigRegion, _: *mut c_void) -> i32 {
    let end = region.end.first().copied().unwrap_or(-1);
    let start = region.beg.first().copied().unwrap_or(-1);
    EACH_MATCHES.lock().unwrap().push((start, end));
    EACH_MATCH_RESULT.load(Ordering::Relaxed)
}

fn clear_events() {
    EVENTS.lock().unwrap().clear();
    EACH_MATCHES.lock().unwrap().clear();
}

fn compile(
    pattern: &[u8],
    enc: &'static dyn ferroni::regenc::Encoding,
) -> ferroni::oniguruma::RegexType {
    onig_new(
        pattern,
        ONIG_OPTION_NONE,
        enc,
        &OnigSyntaxOniguruma as &OnigSyntaxType,
    )
    .unwrap()
}

#[test]
fn registered_global_param_and_each_match_callbacks_execute() {
    assert!(onig_get_progress_callout().is_none());
    assert!(onig_get_retraction_callout().is_none());
    assert!(onig_get_callback_each_match().is_none());

    assert_eq!(onig_set_progress_callout(global_progress), ONIG_NORMAL);
    assert_eq!(onig_set_retraction_callout(global_retraction), ONIG_NORMAL);
    let each: OnigCallbackEachMatchFunc = each_match;
    assert_eq!(onig_set_callback_each_match(each), ONIG_NORMAL);
    assert_eq!(
        onig_get_progress_callout().map(|callback| callback as usize),
        Some(global_progress as *const () as usize)
    );
    assert_eq!(
        onig_get_retraction_callout().map(|callback| callback as usize),
        Some(global_retraction as *const () as usize)
    );
    assert_eq!(
        onig_get_callback_each_match().map(|callback| callback as usize),
        Some(each as usize)
    );

    let arg_types = [OnigType::Long as u32, OnigType::Long as u32];
    let defaults = [OnigValue::Long(42)];
    let name_id = onig_set_callout_of_name(
        &ONIG_ENCODING_ASCII,
        OnigCalloutType::Single,
        b"USER",
        ONIG_CALLOUT_IN_BOTH,
        Some(named_callback),
        Some(named_end_callback),
        2,
        &arg_types,
        1,
        &defaults,
    );
    assert_eq!(name_id, 9);
    // C's ASCII-compatible registry fallback updates the same callout when
    // it is registered through UTF-8.
    let same_id = onig_set_callout_of_name(
        &ONIG_ENCODING_UTF8,
        OnigCalloutType::Single,
        b"USER",
        ONIG_CALLOUT_IN_BOTH,
        Some(named_callback),
        Some(named_end_callback),
        2,
        &arg_types,
        1,
        &defaults,
    );
    assert_eq!(same_id, name_id);
    assert_eq!(onig_get_callout_name_by_name_id(1), Some(b"FAIL".to_vec()));
    assert_eq!(
        onig_get_callout_name_by_name_id(name_id),
        Some(b"USER".to_vec())
    );

    let utf8_only_id = onig_set_callout_of_name(
        &ONIG_ENCODING_UTF8,
        OnigCalloutType::Single,
        b"UTFONLY",
        ONIG_CALLOUT_IN_BOTH,
        Some(named_callback),
        Some(named_end_callback),
        0,
        &[],
        0,
        &[],
    );
    assert_eq!(utf8_only_id, 10);
    let ascii_only_lookup = onig_new(
        b"(*UTFONLY)",
        ONIG_OPTION_NONE,
        &ONIG_ENCODING_ASCII,
        &OnigSyntaxOniguruma,
    );
    assert_eq!(
        ascii_only_lookup.err().unwrap().code(),
        ONIGERR_UNDEFINED_CALLOUT_NAME
    );
    let ascii_named = compile(b"(*USER{5})ab|ac", &ONIG_ENCODING_ASCII);
    assert_eq!(
        onig_match(&ascii_named, b"ac", 2, 0, None, ONIG_OPTION_NONE).0,
        2
    );

    // The named callback is invoked in both directions. C snapshots end_func
    // but stores start_func in STK_CALLOUT and invokes that same function on
    // retraction.
    let named = compile(b"(*USER{5})ab|ac", &ONIG_ENCODING_UTF8);
    clear_events();
    assert_eq!(onig_match(&named, b"ac", 2, 0, None, ONIG_OPTION_NONE).0, 2);
    let named_events = EVENTS.lock().unwrap().clone();
    assert_eq!(
        named_events.len(),
        2,
        "progress and retraction callbacks run"
    );
    assert_eq!(named_events[0].source, "named");
    assert_eq!(named_events[0].direction, OnigCalloutIn::Progress);
    assert_eq!(named_events[0].name_id, name_id);
    assert_eq!(named_events[0].args_num, 2);
    assert_eq!(named_events[0].passed_args_num, 1);
    assert_eq!(named_events[0].first_arg, Some(5));
    assert_eq!(named_events[1].direction, OnigCalloutIn::Retraction);
    assert_eq!(END_CALLS.load(Ordering::Relaxed), 0);

    // FAIL backtracks without scheduling retraction; invalid positive return
    // codes become ONIGERR_INVALID_ARGUMENT, and negative errors propagate.
    clear_events();
    NAMED_RESULT.store(1, Ordering::Relaxed);
    RETRACTION_RESULT.store(ONIG_NORMAL, Ordering::Relaxed);
    assert_eq!(onig_match(&named, b"ac", 2, 0, None, ONIG_OPTION_NONE).0, 2);
    let fail_events = EVENTS.lock().unwrap().clone();
    assert_eq!(fail_events.len(), 1);
    assert_eq!(fail_events[0].direction, OnigCalloutIn::Progress);
    NAMED_RESULT.store(2, Ordering::Relaxed);
    assert_eq!(
        onig_match(&named, b"ac", 2, 0, None, ONIG_OPTION_NONE).0,
        ONIGERR_INVALID_ARGUMENT
    );
    NAMED_RESULT.store(ONIG_NORMAL, Ordering::Relaxed);
    RETRACTION_RESULT.store(-777, Ordering::Relaxed);
    assert_eq!(
        onig_match(&named, b"ac", 2, 0, None, ONIG_OPTION_NONE).0,
        -777
    );
    RETRACTION_RESULT.store(ONIG_NORMAL, Ordering::Relaxed);

    // These builtins share the public name-ID registry and execute their C
    // progress contracts.
    let mismatch = compile(b"(*MISMATCH)", &ONIG_ENCODING_UTF8);
    assert_eq!(
        onig_match(&mismatch, b"", 0, 0, None, ONIG_OPTION_NONE).0,
        ONIG_MISMATCH
    );
    let error_default = compile(b"(*ERROR)", &ONIG_ENCODING_UTF8);
    assert_eq!(
        onig_match(&error_default, b"", 0, 0, None, ONIG_OPTION_NONE).0,
        ONIG_ABORT
    );
    let error_positive = compile(b"(*ERROR{1})", &ONIG_ENCODING_UTF8);
    assert_eq!(
        onig_match(&error_positive, b"", 0, 0, None, ONIG_OPTION_NONE).0,
        ONIGERR_INVALID_CALLOUT_BODY
    );

    // Contents callouts invoke the process-wide callbacks for direct matching
    // and search. A progress failure must not create a retraction callback.
    let contents = compile(b"(?{body}X)ab|ac", &ONIG_ENCODING_UTF8);
    clear_events();
    assert_eq!(
        onig_match(&contents, b"ac", 2, 0, None, ONIG_OPTION_NONE).0,
        2
    );
    let contents_events = EVENTS.lock().unwrap().clone();
    assert_eq!(contents_events.len(), 2);
    assert_eq!(contents_events[0].source, "global-progress");
    assert_eq!(contents_events[0].direction, OnigCalloutIn::Progress);
    assert_eq!(contents_events[0].contents.as_deref(), Some(&b"body"[..]));
    assert_eq!(contents_events[1].source, "global-retraction");
    assert_eq!(contents_events[1].direction, OnigCalloutIn::Retraction);

    let search_contents = compile(b"(?{search})a", &ONIG_ENCODING_UTF8);
    clear_events();
    assert_eq!(
        onig_search(&search_contents, b"ba", 2, 0, 2, None, ONIG_OPTION_NONE,).0,
        1
    );
    let search_events = EVENTS.lock().unwrap().clone();
    assert_eq!(search_events.len(), 1);
    assert_eq!(search_events[0].source, "global-progress");
    assert_eq!(search_events[0].contents.as_deref(), Some(&b"search"[..]));

    clear_events();
    GLOBAL_PROGRESS_RESULT.store(1, Ordering::Relaxed);
    let fail_on_progress = compile(b"(?{fail}X)a", &ONIG_ENCODING_UTF8);
    let (result, _) = onig_match(&fail_on_progress, b"a", 1, 0, None, ONIG_OPTION_NONE);
    assert_eq!(result, ONIG_MISMATCH);
    let failed_progress_events = EVENTS.lock().unwrap().clone();
    assert_eq!(failed_progress_events.len(), 1);
    assert_eq!(failed_progress_events[0].source, "global-progress");
    GLOBAL_PROGRESS_RESULT.store(ONIG_NORMAL, Ordering::Relaxed);

    // Per-match callbacks override the process-wide callbacks and receive the
    // user-data pointer through both direct-match and search entry points.
    let mut mp = onig_new_match_param();
    assert_eq!(
        mp.progress_callout.map(|callback| callback as usize),
        Some(global_progress as *const () as usize)
    );
    assert_eq!(
        onig_set_progress_callout_of_match_param(&mut mp, Some(param_progress)),
        ONIG_NORMAL
    );
    assert_eq!(
        onig_set_retraction_callout_of_match_param(&mut mp, Some(param_retraction)),
        ONIG_NORMAL
    );
    let token = 0xCAFEusize as *mut c_void;
    assert_eq!(
        onig_set_callout_user_data_of_match_param(&mut mp, token),
        ONIG_NORMAL
    );
    clear_events();
    assert_eq!(
        onig_match_with_param(&contents, b"ac", 2, 0, None, ONIG_OPTION_NONE, &mp).0,
        2
    );
    let param_events = EVENTS.lock().unwrap().clone();
    assert_eq!(param_events.len(), 2);
    assert_eq!(param_events[0].source, "param-progress");
    assert_eq!(param_events[1].source, "param-retraction");
    assert!(
        param_events
            .iter()
            .all(|event| event.user_data == token as usize)
    );

    let mut retraction_only = onig_new_match_param();
    assert_eq!(
        onig_set_progress_callout_of_match_param(&mut retraction_only, None),
        ONIG_NORMAL
    );
    assert_eq!(
        onig_set_retraction_callout_of_match_param(&mut retraction_only, Some(param_retraction)),
        ONIG_NORMAL
    );
    clear_events();
    assert_eq!(
        onig_match_with_param(
            &contents,
            b"ac",
            2,
            0,
            None,
            ONIG_OPTION_NONE,
            &retraction_only,
        )
        .0,
        2
    );
    let retraction_only_events = EVENTS.lock().unwrap().clone();
    assert_eq!(retraction_only_events.len(), 1);
    assert_eq!(retraction_only_events[0].source, "param-retraction");
    assert_eq!(
        retraction_only_events[0].direction,
        OnigCalloutIn::Retraction
    );

    clear_events();
    assert_eq!(
        onig_search_with_param(
            &search_contents,
            b"ba",
            2,
            0,
            2,
            None,
            ONIG_OPTION_NONE,
            &mp,
        )
        .0,
        1
    );
    let param_search_events = EVENTS.lock().unwrap().clone();
    assert_eq!(param_search_events.len(), 1);
    assert_eq!(param_search_events[0].source, "param-progress");

    // Each-match callbacks execute at OP_END. FIND_LONGEST reports local
    // improvements as they occur, including in direct match and search paths.
    let alternatives = compile(b"a|ab", &ONIG_ENCODING_UTF8);
    let search_alternatives = onig_new(
        b"a|ab",
        ONIG_OPTION_FIND_LONGEST,
        &ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
    )
    .unwrap();
    clear_events();
    EACH_MATCH_RESULT.store(ONIG_NORMAL, Ordering::Relaxed);
    let (direct_result, _) = onig_match(
        &alternatives,
        b"ab",
        2,
        0,
        Some(OnigRegion::new()),
        ONIG_OPTION_CALLBACK_EACH_MATCH | ONIG_OPTION_FIND_LONGEST,
    );
    assert_eq!(direct_result, 2);
    assert_eq!(EACH_MATCHES.lock().unwrap().as_slice(), &[(0, 1), (0, 2)]);

    clear_events();
    assert_eq!(
        onig_match(
            &alternatives,
            b"ab",
            2,
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_CALLBACK_EACH_MATCH,
        )
        .0,
        ONIG_MISMATCH
    );
    assert_eq!(EACH_MATCHES.lock().unwrap().as_slice(), &[(0, 1), (0, 2)]);

    clear_events();
    let search_result = onig_search(
        &search_alternatives,
        b"abxaba",
        6,
        0,
        6,
        Some(OnigRegion::new()),
        ONIG_OPTION_CALLBACK_EACH_MATCH | ONIG_OPTION_FIND_LONGEST,
    )
    .0;
    assert_eq!(
        search_result,
        0,
        "each-match sequence: {:?}",
        EACH_MATCHES.lock().unwrap()
    );
    assert_eq!(EACH_MATCHES.lock().unwrap().as_slice(), &[(0, 1), (0, 2)]);

    EACH_MATCH_RESULT.store(-778, Ordering::Relaxed);
    assert_eq!(
        onig_match(
            &alternatives,
            b"ab",
            2,
            0,
            Some(OnigRegion::new()),
            ONIG_OPTION_CALLBACK_EACH_MATCH | ONIG_OPTION_FIND_LONGEST,
        )
        .0,
        -778
    );
}
