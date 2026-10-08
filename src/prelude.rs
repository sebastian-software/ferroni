// prelude.rs - Convenient re-exports for the idiomatic API.
//
//! # Prelude
//!
//! ```
//! use ferroni::prelude::*;
//!
//! let re = Regex::new(r"\d+").unwrap();
//! let m = re.find("answer: 42").unwrap();
//! assert_eq!(m.as_str(), "42");
//! ```

pub use crate::api::{
    CaptureNames, Captures, CapturesIter, FindIter, Match, Regex, RegexBuilder, SearchOptions,
    Syntax, TryFindIter,
};
pub use crate::error::RegexError;
pub use crate::scanner::{
    CaptureIndex, OnigString, Scanner, ScannerConfig, ScannerFindOptions, ScannerMatch,
    ScannerPatternCache, ScannerSyntax,
};
