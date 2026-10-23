//! Construction policy for optional regex and scanner accelerators.

/// When optional DFA prefilters and deferred literal searchers are prepared.
///
/// This policy does not disable the matcher's ordinary compiler optimizations
/// or enable backtracking rewrites. DFA acceleration requires the
/// `dfa-prefilter` Cargo feature. Unsupported patterns and bounded build
/// failures retain their existing fallback; `Eager` does not remove size caps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccelerationMode {
    /// Disable optional DFA prefilters. Other search helpers keep their
    /// ordinary preparation policy.
    Off,
    /// Prepare helpers on demand and build eligible dense DFAs after enough
    /// use. This is the default.
    #[default]
    Auto,
    /// Prepare deferred literal searchers and eligible DFA accelerators before
    /// construction returns, accepting additional construction time and memory.
    ///
    /// Hybrid fallback DFAs still discover transitions while searching.
    /// Search scratch space, input-dependent decisions, and per-thread state
    /// are not fully precomputed. This mode does not guarantee allocation-free
    /// searches or a dense DFA for every pattern.
    Eager,
}

/// Prepare the program's immutable, input-independent deferred searchers.
/// Compiled programs may be shared by scanners or loaded from an archive.
pub(crate) fn prepare_program(program: &crate::regint::RegexType) {
    if let Some(jump) = &program.search_jump {
        jump.prepare(program);
    }
    if let Some(literals) = &program.required_literals {
        literals.prepare();
    }
}
