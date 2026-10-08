//! Re-exports of the types most programs need. `use ferroni::prelude::*;` brings
//! in [`Regex`], [`RegexBuilder`], [`Match`], [`Captures`], the replacement and
//! split types, and the scanner types.
//!
//! ```
//! use ferroni::prelude::*;
//!
//! let re = Regex::new(r"\d+").unwrap();
//! let m = re.find("answer: 42").unwrap();
//! assert_eq!(m.as_str(), "42");
//! ```

pub use crate::api::{
    CaptureMatches, CaptureNames, Captures, CapturesIter, FindIter, Match, Regex, RegexBuilder,
    SearchOptions, Syntax, TryCaptureMatches, TryFindIter,
};
pub use crate::error::RegexError;
pub use crate::replace::{
    NoExpand, NoExpandBytes, Replacer, ReplacerBytes, Split, SplitBytes, SplitN, SplitNBytes,
};
pub use crate::scanner::{
    CaptureIndex, OnigString, Scanner, ScannerConfig, ScannerFindOptions, ScannerMatch,
    ScannerPatternCache, ScannerSyntax,
};
