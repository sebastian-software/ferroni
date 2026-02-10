// encodings/mod.rs - Encoding registry
// Each C encoding file maps to one Rust module.

pub mod ascii;

pub use ascii::ONIG_ENCODING_ASCII;
