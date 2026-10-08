//! Sharing one compiled `Regex` across threads.
//!
//! Compile a pattern once and let every thread search with it. `LazyLock`
//! (stable since Rust 1.80) compiles the pattern on first use, and the static
//! keeps it for the life of the process. Each search allocates its own match
//! state, so no lock is needed around the `Regex`.
//!
//! Run with:
//! cargo run --example shared_regex

use std::sync::LazyLock;
use std::thread;

use ferroni::prelude::*;

static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})").unwrap());

fn main() {
    let lines = [
        "2026-09-05 deploy finished",
        "no date in this line",
        "rollback on 2026-09-06",
        "2026-10-01 next release",
    ];

    let dates: Vec<Option<String>> = thread::scope(|scope| {
        let handles: Vec<_> = lines
            .iter()
            .map(|line| {
                scope.spawn(move || {
                    // Every thread uses the same static `Regex`.
                    DATE.find(line).map(|m| m.as_str().to_owned())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("thread panicked"))
            .collect()
    });

    for (line, date) in lines.iter().zip(&dates) {
        println!("{line:?} -> {date:?}");
    }

    assert_eq!(dates[0].as_deref(), Some("2026-09-05"));
    assert_eq!(dates[1], None);
    assert_eq!(dates[2].as_deref(), Some("2026-09-06"));
    assert_eq!(dates[3].as_deref(), Some("2026-10-01"));
}
