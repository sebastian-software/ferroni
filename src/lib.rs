//! # Ferroni — Oniguruma's regex engine in pure Rust
//!
//! Ferroni is a line-by-line port of [Oniguruma](https://github.com/kkos/oniguruma),
//! the backtracking engine behind TextMate grammars, jq and PHP's mbregex. It
//! keeps Oniguruma's syntax and semantics for ASCII and UTF-8, and it needs no
//! C toolchain and no bindings.
//!
//! ## When to use Ferroni
//!
//! Choose Ferroni when a pattern must behave as it does in Oniguruma: look-behind,
//! backreferences, atomic and possessive groups, subexpression calls, or a
//! TextMate grammar that highlights the way Shiki and VS Code do. Oniguruma's
//! defaults differ from the [`regex`](https://crates.io/crates/regex) crate for
//! anchors, capture groups, empty matches, group counts and byte input.
//! [Coming from `regex` or `onig`](https://ferroni.dev/guide/coming-from-regex)
//! shows each difference in one call.
//!
//! Ferroni is a backtracking engine, so it does not guarantee linear-time
//! searches. Read [Untrusted input](#untrusted-input) before you search text or
//! compile patterns that users supply.
//!
//! ## Regex
//!
//! [`Regex`] compiles a pattern once and then searches as often as needed. The
//! crate root re-exports the prelude types, so `use ferroni::Regex;` works.
//! `use ferroni::prelude::*;` brings in all of them.
//!
//! ```
//! use ferroni::Regex;
//!
//! let re = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
//! let m = re.find("Date: 2026-02-12").unwrap();
//! assert_eq!(m.as_str(), "2026-02-12");
//! assert_eq!(m.start(), 6);
//! ```
//!
//! [`Regex::builder`] returns a [`RegexBuilder`] for options and syntax.
//! [`RegexBuilder::syntax_mode`] takes a typed [`Syntax`]:
//!
//! ```
//! use ferroni::{Regex, Syntax};
//!
//! let re = Regex::builder("hello").case_insensitive(true).build().unwrap();
//! assert!(re.is_match("Hello World"));
//!
//! let ruby = Regex::builder(r"\w+").syntax_mode(Syntax::Ruby).build().unwrap();
//! assert!(ruby.is_match("ok"));
//! ```
//!
//! [`Regex::captures`] returns [`Captures`], which indexes groups by number or
//! by name. Named groups come from the pattern itself:
//!
//! ```
//! use ferroni::Regex;
//!
//! let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})").unwrap();
//! let caps = re.captures("Released 2026-10.").unwrap();
//! assert_eq!(&caps[0], "2026-10");
//! assert_eq!(&caps["year"], "2026");
//! assert_eq!(caps.get(2).unwrap().as_str(), "10");
//! ```
//!
//! [`Regex::find_iter`] yields the non-overlapping matches, and
//! [`Regex::captures_iter`] yields the groups of each one:
//!
//! ```
//! use ferroni::Regex;
//!
//! let re = Regex::new(r"(\w+)=(\d+)").unwrap();
//! let pairs: Vec<(&str, &str)> = re
//!     .captures_iter("a=1, b=22")
//!     .map(|caps| (caps.get(1).unwrap().as_str(), caps.get(2).unwrap().as_str()))
//!     .collect();
//! assert_eq!(pairs, [("a", "1"), ("b", "22")]);
//! ```
//!
//! [`Regex::replace_all`] substitutes every match, with `$1` or `$name` in the
//! template naming a group, and [`Regex::split`] yields the text between
//! matches:
//!
//! ```
//! use ferroni::Regex;
//!
//! let re = Regex::new(r"(?<key>\w+)=(?<value>\d+)").unwrap();
//! assert_eq!(re.replace_all("a=1, b=22", "$value:$key"), "1:a, 22:b");
//!
//! let sep = Regex::new(r",\s*").unwrap();
//! assert_eq!(sep.split("a, b,c").collect::<Vec<_>>(), ["a", "b", "c"]);
//! ```
//!
//! Search from a byte offset with [`Regex::find_at`], [`Regex::captures_at`] and
//! [`Regex::is_match_at`]. The text before the offset stays in view, so a
//! look-behind sees it. A slice of the text is a new text with no such context:
//!
//! ```
//! use ferroni::Regex;
//!
//! let re = Regex::new(r"(?<=a)b").unwrap();
//! assert_eq!(re.find_at("ab", 1).unwrap().range(), 1..2);
//! assert!(re.find(&"ab"[1..]).is_none());
//! ```
//!
//! ## Scanner
//!
//! A tokenizer asks one question many times: which of these patterns matches
//! next, at or after this position? [`Scanner`] compiles the whole set once and
//! answers it. It has the shape of vscode-oniguruma's `OnigScanner`, which is
//! what TextMate grammars run on
//! ([ADR-006](https://ferroni.dev/adr/006-scanner-api)).
//!
//! ```
//! use ferroni::{Scanner, ScannerFindOptions};
//!
//! let mut scanner = Scanner::new(&[r"\bfn\b", r#""[^"]*""#, r"//.*$"]).unwrap();
//! let m = scanner.find_next_match(r#"fn f() "x""#, 0, ScannerFindOptions::NONE).unwrap();
//! assert_eq!(m.index, 0);
//! assert_eq!((m.capture_indices[0].start, m.capture_indices[0].end), (0, 2));
//! ```
//!
//! The earliest match wins. When two patterns match at the same position, the
//! one listed first wins:
//!
//! ```
//! use ferroni::{Scanner, ScannerFindOptions};
//!
//! let mut scanner = Scanner::new(&["ab", "a"]).unwrap();
//! let m = scanner.find_next_match("xab", 0, ScannerFindOptions::NONE).unwrap();
//! assert_eq!((m.index, m.capture_indices[0].start), (0, 1));
//! ```
//!
//! A [`ScannerMatch`] carries the winning pattern's index and the byte spans of
//! its capture groups; index 0 is the whole match. To count UTF-16 code units,
//! as editors do, search an [`OnigString`] with
//! [`Scanner::find_next_match_utf16`].
//!
//! A grammar builds one scanner per rule context, and those scanners repeat the
//! same patterns. Build them with one [`ScannerPatternCache`], which belongs to
//! the caller and is not process-wide. Each distinct pattern then compiles once:
//!
//! ```
//! use ferroni::{Scanner, ScannerConfig, ScannerPatternCache};
//!
//! let config = ScannerConfig::default();
//! let mut cache = ScannerPatternCache::new();
//! let _top = Scanner::with_pattern_cache(&[r"\bfn\b", r#""[^"]*""#], &config, &mut cache)
//!     .unwrap();
//! let _call = Scanner::with_pattern_cache(&[r"\)", r#""[^"]*""#], &config, &mut cache)
//!     .unwrap();
//! assert_eq!(cache.len(), 3); // the string pattern was compiled once
//! ```
//!
//! ## Untrusted input
//!
//! Some pattern and input pairs take exponential time in a backtracking engine.
//! `(a+)+b` against a long run of `a` is the classic case. When patterns or text
//! come from users, bound each search with [`SearchOptions`] and the `*_with`
//! methods. They report a search that hit a limit as an error, where the plain
//! methods report no match:
//!
//! ```
//! use std::time::Duration;
//! use ferroni::{Regex, RegexError, SearchOptions};
//!
//! let re = Regex::new(r"(a+)+b").unwrap();
//! let options = SearchOptions::new().timeout(Duration::from_millis(50));
//!
//! let hostile = "a".repeat(40);
//! assert!(matches!(re.find_with(&hostile, options), Err(RegexError::TimeLimitOver)));
//! ```
//!
//! [`SearchOptions::retry_limit_in_match`] counts backtracks instead of time, so
//! it stops a search at the same point on every machine. For patterns under
//! review, [`RegexBuilder::reject_backtracking_risks`] turns the compile-time
//! check into an error, and [`Regex::backtracking_warnings`] lists its findings:
//!
//! ```
//! use ferroni::Regex;
//!
//! assert!(Regex::builder(r"(a+)+$").reject_backtracking_risks(true).build().is_err());
//! assert_eq!(Regex::new(r"(a+)+$").unwrap().backtracking_warnings().len(), 1);
//! ```
//!
//! The [untrusted input guide](https://ferroni.dev/guide/untrusted-input) covers
//! the defaults and the other layers of protection.
//!
//! ## Thread safety
//!
//! [`Regex`] is `Send + Sync` and cheap to [`Clone`]. Clones share one compiled
//! program, so compile once and give each thread a clone. [`Scanner`] is `Send`
//! and `Sync` too, but its search takes `&mut self`, so each thread needs its own.
//!
//! ```
//! use std::thread;
//! use ferroni::Regex;
//!
//! fn assert_send_sync<T: Send + Sync>() {}
//! assert_send_sync::<Regex>();
//!
//! let re = Regex::new(r"\d+").unwrap();
//! let workers: Vec<_> = (0..4)
//!     .map(|i| {
//!         let re = re.clone();
//!         thread::spawn(move || re.is_match(&format!("job {i}")))
//!     })
//!     .collect();
//! assert!(workers.into_iter().all(|worker| worker.join().unwrap()));
//! ```
//!
//! ## Low-level C API
//!
//! Every C entry point is ported under its original name, so upstream code and
//! documentation translate directly. `onig_new` compiles a pattern and
//! `onig_search` searches it. Use these when you need something the idiomatic
//! layer does not expose yet
//! ([guide](https://ferroni.dev/guide/getting-started#the-low-level-c-api)):
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
//! ## Modules
//!
//! ### Start here
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`prelude`] | The types most programs need, also re-exported at the crate root |
//! | [`api`] | [`Regex`], [`RegexBuilder`], [`Match`], [`Captures`] and the iterators |
//! | [`replace`] | The [`Replacer`] trait and the iterators behind `replace` and `split` |
//! | [`scanner`] | [`Scanner`] and [`ScannerPatternCache`] for multi-pattern tokenization |
//! | [`error`] | [`RegexError`], the typed error of every fallible call |
//!
//! ### C port, 1:1 with Oniguruma ([ADR-001](https://ferroni.dev/adr/001-one-to-one-parity-with-c-original))
//!
//! Each C source file maps to one Rust module:
//!
//! | C File | Rust Module | Purpose |
//! |--------|-------------|---------|
//! | `oniguruma.h` | [`oniguruma`] | Public types, option flags and error codes |
//! | `regparse.c` | [`regparse`] | Pattern parser |
//! | `regparse.h` | [`regparse_types`] | Parse tree and parser types |
//! | `regcomp.c` | [`regcomp`] | AST-to-bytecode compiler (`onig_new`) |
//! | `regexec.c` | [`regexec`] | VM executor (`onig_search`) |
//! | `regexec.c`, RegSet section | [`regset`] | Multi-regex search (RegSet) |
//! | `regint.h` | [`regint`] | Internal types and opcodes |
//! | `regenc.h`, `regenc.c` | [`regenc`] | Encoding trait |
//! | `utf8.c`, `ascii.c` | [`encodings`] | UTF-8 and US-ASCII encodings |
//! | `unicode.c` | [`unicode`] | Character properties and case folding |
//! | `regsyntax.c` | [`regsyntax`] | Syntax definitions |
//! | `regerror.c` | [`regerror`] | Error messages |
//! | `regtrav.c` | [`regtrav`] | Capture tree traversal |
//!
//! `backtrack_lint` and `backtrack_rewrite` are Rust-only
//! ([ADR-008](https://ferroni.dev/adr/008-rust-only-optimizations)). They define
//! the types that [`Regex::backtracking_warnings`] and
//! [`Regex::backtracking_rewrites`] return.

// The lint policy for the C port lives in the `[lints]` table of Cargo.toml so
// that it also covers tests, benches, and examples.
// Enable #[coverage(off)] attribute when running under cargo-llvm-cov on nightly.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod api;
pub mod backtrack_lint;
pub mod backtrack_rewrite;
pub mod encodings;
pub mod error;
mod first_bytes;
mod leading_run;
// Hidden: a Rust-only optimization (ADR-008) that no code outside the crate uses.
#[doc(hidden)]
pub mod literal_trie;
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
pub mod replace;
mod required_literals;
pub mod scanner;
pub mod unicode;

// The prelude types are also at the crate root: `use ferroni::Regex;`.
pub use prelude::*;

// Hidden: FFI bindings to C Oniguruma, used only by the benchmark harness.
#[cfg(feature = "ffi")]
#[doc(hidden)]
pub mod ffi;

// Doc tests for the README and the guide pages, run by `cargo test --doc`.
// The items exist only under `doctest`, so they are never rendered or built
// into a normal build. The guide pages live in `docs/`, which the published
// crate excludes; build.rs sets `ferroni_guide_docs` only when they are present.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(all(doctest, ferroni_guide_docs))]
#[doc = include_str!("../docs/app/routes/guide/getting-started.mdx")]
struct GettingStartedDoctests;

#[cfg(all(doctest, ferroni_guide_docs))]
#[doc = include_str!("../docs/app/routes/guide/untrusted-input.mdx")]
struct UntrustedInputDoctests;

#[cfg(all(doctest, ferroni_guide_docs))]
#[doc = include_str!("../docs/app/routes/guide/compatibility.mdx")]
struct CompatibilityDoctests;

#[cfg(all(doctest, ferroni_guide_docs))]
#[doc = include_str!("../docs/app/routes/guide/coming-from-regex.mdx")]
struct ComingFromRegexDoctests;
