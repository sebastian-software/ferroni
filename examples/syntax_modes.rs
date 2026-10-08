//! Choosing a syntax mode for a pattern.
//!
//! Ferroni compiles every pattern with Oniguruma syntax unless told otherwise.
//! `RegexBuilder::syntax` selects one of the twelve syntax definitions in
//! `ferroni::regsyntax`, which change how characters such as `+` and `\(` read.
//!
//! Run with:
//! cargo run --example syntax_modes

use ferroni::prelude::*;
use ferroni::regsyntax::{OnigSyntaxPosixBasic, OnigSyntaxRuby};

fn main() -> Result<(), RegexError> {
    // Oniguruma syntax is the default: `+` is a quantifier.
    let oniguruma = Regex::new(r"a+")?;
    let found = oniguruma.find("caaat").map(|m| m.as_str());
    println!("Oniguruma (default): {found:?}");
    assert_eq!(found, Some("aaa"));

    // POSIX Basic syntax: `+` is an ordinary character, so `a+` matches the
    // two characters "a+" and not a run of `a`.
    let posix = Regex::builder(r"a+")
        .syntax(&OnigSyntaxPosixBasic)
        .build()?;
    let found = posix.find("caaat").map(|m| m.as_str());
    println!("POSIX Basic, a+ on caaat: {found:?}");
    assert_eq!(found, None);
    let found = posix.find("c a+ t").map(|m| m.as_str());
    println!("POSIX Basic, a+ on c a+ t: {found:?}");
    assert_eq!(found, Some("a+"));

    // Ruby syntax for patterns that come from Ruby code or Ruby-flavored
    // grammars. The same builder call selects it.
    let ruby = Regex::builder(r"\d+").syntax(&OnigSyntaxRuby).build()?;
    let found = ruby.find("order 66").map(|m| m.as_str());
    println!("Ruby, \\d+ on order 66: {found:?}");
    assert_eq!(found, Some("66"));
    Ok(())
}
