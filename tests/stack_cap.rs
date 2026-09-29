//! The backtrack stack has a hard cap even when no match-stack limit is set.
//!
//! Ignored by default: it grows the stack to the cap, about half a gigabyte.
//! Run with `cargo test --release --test stack_cap -- --ignored`.

use ferroni::api::{Regex, SearchOptions};
use ferroni::error::RegexError;

#[test]
#[ignore = "allocates about half a gigabyte"]
fn unbounded_stack_growth_stops_at_the_cap() {
    // The loop pushes one entry per character; the look-behind then fails, so
    // nothing ends the attempt but the cap.
    let re = Regex::new(r"(?:a|b)*(?<=b)").unwrap();
    let text = "a".repeat(20 << 20);
    let result = re.find_with(&text, SearchOptions::new());
    assert_eq!(result.unwrap_err(), RegexError::MatchStackLimitOver);
}

#[test]
fn a_larger_explicit_limit_is_honored() {
    let re = Regex::new(r"(?:a|b)*c").unwrap();
    let text = "ab".repeat(1000) + "c";
    let options = SearchOptions::new().match_stack_limit(50_000_000);
    assert_eq!(
        re.find_with(&text, options).unwrap().unwrap().end(),
        text.len()
    );
}
