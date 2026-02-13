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
//! use ferroni::regcomp::onig_new;
//! use ferroni::regexec::onig_search;
//! use ferroni::oniguruma::*;
//! use ferroni::regsyntax::OnigSyntaxOniguruma;
//!
//! let reg = onig_new(
//!     b"\\d{4}-\\d{2}-\\d{2}",
//!     ONIG_OPTION_NONE,
//!     &ferroni::encodings::utf8::ONIG_ENCODING_UTF8,
//!     &OnigSyntaxOniguruma as *const OnigSyntaxType,
//! ).unwrap();
//!
//! let input = b"Date: 2026-02-12";
//! let (result, region) = onig_search(
//!     &reg, input, input.len(), input.len(), 0,
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

pub mod oniguruma;
pub mod regint;
pub mod regenc;
pub mod regsyntax;
pub mod regparse_types;
pub mod unicode;
pub mod encodings;
pub mod regparse;
pub mod regcomp;
pub mod regexec;
pub mod regtrav;
pub mod regset;
pub mod regerror;

#[cfg(feature = "ffi")]
pub mod ffi;
