//! The encodings that Ferroni supports: UTF-8 and US-ASCII, ported from `utf8.c`
//! and `ascii.c`. Pass [`ONIG_ENCODING_UTF8`] or [`ONIG_ENCODING_ASCII`] to
//! [`onig_new`](crate::regcomp::onig_new). No other encoding is ported
//! ([ADR-003](https://ferroni.dev/adr/003-encoding-scope-ascii-and-utf8-only)).

// Each C encoding file maps to one Rust module.

pub mod ascii;
pub mod utf8;

pub use ascii::ONIG_ENCODING_ASCII;
pub use utf8::ONIG_ENCODING_UTF8;
