//! Source-backed library operations, checked against retained upstream oracles.
//!
//! `--catalog` emits the immutable case contract. `--validate` emits eligibility
//! records without measuring. Criterion only registers eligible engine/case
//! pairs, so a syntax rejection or an incorrect answer cannot become a timing.

#[path = "engines.rs"]
mod engines;
#[path = "production_regex/runner.rs"]
mod production;

fn main() {
    production::main();
}
