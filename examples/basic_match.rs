//! Matching, capturing, and iterating with the idiomatic API.
//!
//! Run with:
//! cargo run --example basic_match

use ferroni::prelude::*;

fn main() -> Result<(), RegexError> {
    // A single match: `find` returns the leftmost match with its byte offsets.
    let re = Regex::new(r"\d{4}-\d{2}-\d{2}")?;
    let m = re
        .find("Released on 2026-09-05.")
        .expect("date in the text");
    println!("found {:?} at bytes {}..{}", m.as_str(), m.start(), m.end());

    // Named captures. Oniguruma accepts `(?<n>)`, `(?'n')`, and `(?P<n>)`;
    // all three name the same group.
    let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})")?;
    let caps = re
        .captures("Released on 2026-09-05.")
        .expect("date in the text");
    println!(
        "year={} month={} day={}",
        caps.name("year").unwrap().as_str(),
        caps.name("month").unwrap().as_str(),
        caps.name("day").unwrap().as_str(),
    );

    // Every match in the input.
    let re = Regex::new(r"\w+@\w+\.\w+")?;
    let text = "write to ada@example.com or grace@example.org";
    for (i, m) in re.find_iter(text).enumerate() {
        println!("address {}: {}", i, m.as_str());
    }

    // Options that have no inline flag go through the builder.
    let re = Regex::builder(r"hello").case_insensitive(true).build()?;
    println!("case-insensitive match: {}", re.is_match("Hello World"));

    // Features most Rust regex engines do not offer: a backreference and a
    // variable-length look-behind.
    let doubled = Regex::new(r"\b(\w+) \1\b")?;
    println!(
        "doubled word: {:?}",
        doubled.find("this this is repeated").map(|m| m.as_str()),
    );

    let after_label = Regex::new(r"(?<=version:\s*)\d+\.\d+\.\d+")?;
    println!(
        "version: {:?}",
        after_label.find("version:   1.3.3").map(|m| m.as_str()),
    );

    Ok(())
}
