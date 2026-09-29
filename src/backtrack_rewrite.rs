//! Opt-in, result-preserving AST rewrites (ADR-008).
//!
//! Recognizes only greedy `(?:[0-9]+_?)+` immediately before
//! a mandatory literal dot, and `(?:[0-9]+_?)*[0-9]+` before a word boundary.
//! Capture wrappers are retained verbatim. This is
//! deliberately independent of the heuristic in `backtrack_lint`.
//!
//! This module owns the complete opt-in pipeline: `ast` recognizes safe shapes
//! and retains captures, `lowering` calculates and emits specialized bytecode,
//! and `runtime` executes the decimal prefix with native capture bookkeeping.
//! `regcomp` and `regexec` supply the compiler and VM integration points.
//! Public diagnostic types stay here; implementation modules are crate-private.

mod ast;
pub(crate) mod lowering;
pub(crate) mod runtime;

pub(crate) use ast::apply;

#[cfg(test)]
mod tests;

/// Why a recognized decimal-loop candidate was left unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BacktrackingRewriteRefusal {
    /// The next node does not require a literal dot immediately.
    NoMandatoryDot,
    /// The decimal tail is not immediately followed by a word boundary.
    NoWordBoundary,
    /// The pattern contains capture reads, calls, look-arounds, scoped
    /// options, position checks, callouts, or other stateful constructs.
    UnsupportedConstruct,
    /// Capture history or an exhaustive matching option is enabled.
    UnsupportedMode,
}

/// One recognized candidate, in AST traversal order. Unsupported shapes
/// have no entry; absence of a report does not mean a pattern is safe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BacktrackingRewrite {
    /// Wrapped a decimal loop in an atomic group, retaining every capture.
    AtomicDecimalLoop,
    /// Made the primitive digit run possessive using choice-free bytecode.
    PossessiveDecimalDigits,
    /// Made the complete decimal tail atomic, retaining its internal captures.
    AtomicDecimalTail,
    /// Evaluate the decimal prefix directly, preserving its last iteration's captures.
    DeterministicDecimalTail,
    /// Recognized the decimal loop but could not establish its safety.
    Refused(BacktrackingRewriteRefusal),
}
