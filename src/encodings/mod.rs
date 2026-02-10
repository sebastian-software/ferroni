// encodings/mod.rs - Encoding registry
// Each C encoding file maps to one Rust module.

pub mod ascii;
pub mod utf8;

pub use ascii::ONIG_ENCODING_ASCII;
pub use utf8::ONIG_ENCODING_UTF8;
