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

pub use crate::api::{Captures, CapturesIter, FindIter, Match, Regex, RegexBuilder};
pub use crate::error::RegexError;
