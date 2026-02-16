//! # Ferroni
//!
//! Pure-Rust regex engine based on [Oniguruma](https://github.com/kkos/oniguruma),
//! with SIMD-accelerated search via [`memchr`](https://crates.io/crates/memchr).
//!
//! Ferroni is a line-by-line port of Oniguruma's C source into Rust -- same
//! structure, same function names, same semantics. No bindings, no FFI.
//!
//! ## Quick Start
//!
//! ```rust
//! use ferroni::prelude::*;
//!
//! let re = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
//! let m = re.find("Date: 2026-02-12").unwrap();
//! assert_eq!(m.as_str(), "2026-02-12");
//! assert_eq!(m.start(), 6);
//! ```
//!
//! For fine-grained control, use [`RegexBuilder`]:
//!
//! ```rust
//! use ferroni::prelude::*;
//!
//! let re = Regex::builder(r"hello")
//!     .case_insensitive(true)
//!     .build()
//!     .unwrap();
//! assert!(re.is_match("Hello World"));
//! ```
//!
//! ## Low-Level C-Style API
//!
//! The full C-ported API is also available for advanced usage:
//!
//! ```rust
//! use ferroni::regcomp::onig_new;
//! use ferroni::regexec::onig_search;
//! use ferroni::oniguruma::*;
//! use ferroni::regsyntax::OnigSyntaxOniguruma;
//!
//! let reg = onig_new(
//!     b"\\d{4}-\\d{2}-\\d{2}",
//!     ONIG_OPTION_NONE,
//!     &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
//!     &OnigSyntaxOniguruma,
//! ).unwrap();
//!
//! let input = b"Date: 2026-02-12";
//! let (result, region) = onig_search(
//!     &reg, input, input.len(), 0, input.len(),
//!     Some(OnigRegion::new()), ONIG_OPTION_NONE,
//! );
//!
//! assert!(result >= 0);
//! assert_eq!(result, 6); // match starts at byte 6
//! ```
//!
//! ## Module Structure
//!
//! Each C source file maps 1:1 to a Rust module:
//!
//! | C File | Rust Module | Purpose |
//! |--------|-------------|---------|
//! | `regparse.c` | [`regparse`] | Pattern parser |
//! | `regcomp.c` | [`regcomp`] | AST-to-bytecode compiler |
//! | `regexec.c` | [`regexec`] | VM executor |
//! | `regint.h` | [`regint`] | Internal types and opcodes |
//! | `oniguruma.h` | [`oniguruma`] | Public types and constants |
//! | `regenc.c` | [`regenc`] | Encoding trait |
//! | `regsyntax.c` | [`regsyntax`] | 12 syntax definitions |
//! | `regset.c` | [`regset`] | Multi-regex search (RegSet) |
//! | `regerror.c` | [`regerror`] | Error messages |
//! | `regtrav.c` | [`regtrav`] | Capture tree traversal |

// Allow patterns inherent to the C port.
#![allow(dead_code)]
#![allow(clippy::empty_line_after_doc_comments)]
#![allow(clippy::missing_transmute_annotations)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
// Enable #[coverage(off)] attribute when running under cargo-llvm-cov on nightly.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod api;
pub mod encodings;
pub mod error;
pub mod oniguruma;
pub mod prelude;
pub mod regcomp;
pub mod regenc;
pub mod regerror;
pub mod regexec;
pub mod regint;
pub mod regparse;
pub mod regparse_types;
pub mod regset;
pub mod regsyntax;
pub mod regtrav;
pub mod scanner;
pub mod unicode;

#[cfg(feature = "ffi")]
pub mod ffi;
