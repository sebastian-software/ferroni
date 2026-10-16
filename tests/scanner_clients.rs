//! Contracts of the retained application replay, independent of the big traces.
#![cfg(not(target_family = "wasm"))]
#[path = "../benches/scanner_clients/mod.rs"]
mod clients;
#[path = "../benches/cpp_scanner/mod.rs"]
mod scanner_replay;

fn fixture() -> serde_json::Value {
    serde_json::json!({
        "format_version":1, "caches":1,
        "scanners":[{"patterns":["(😀)", "!"],"cache":0},
                    {"patterns":["(😀)", "!"],"cache":0}],
        "subjects":["é😀!", "é😀!"],
        "events":[["create",0], ["find",0,0,0,0,0,[[1,3],[1,3]]],
                  ["drop",0], ["clear",0], ["create",1],
                  ["find",1,1,3,0,1,[[3,4]]], ["find",1,1,4,0,null,[]],
                  ["drop",1], ["drop_cache",0]]
    })
}

#[test]
fn replays_utf16_captures_misses_cache_clear_and_distinct_equal_subjects() {
    let trace = clients::Trace::parse(&fixture().to_string());
    assert_eq!(trace.subjects.len(), 2);
    assert_eq!(trace.subjects[0], trace.subjects[1]);
    trace.validate(true);
    trace.validate(false);
    trace.lifecycle(&trace.strings(), true, false, false);
}

#[test]
#[should_panic(expected = "search of dead scanner")]
fn rejects_search_after_scanner_destruction() {
    let mut input = fixture();
    input["events"][1] = serde_json::json!(["drop", 0]);
    input["events"][2] = serde_json::json!(["find", 0, 0, 0, 0, null, []]);
    clients::Trace::parse(&input.to_string());
}

#[test]
#[should_panic(expected = "reuse of destroyed cache")]
fn rejects_reuse_of_destroyed_pattern_cache() {
    let mut input = fixture();
    input["events"][3] = serde_json::json!(["drop_cache", 0]);
    clients::Trace::parse(&input.to_string());
}
