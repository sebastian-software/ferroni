//! Opt-in memoization of failed matcher states (ADR-008).
//!
//! Enable the `match-cache` Cargo feature, then configure a regex or scanner
//! with [`MatchCacheConfig`]. Unsupported patterns keep the ordinary matcher.
//! This is not a replacement for search limits: backward searches, patterns
//! with stateful instructions, and searches that exhaust the cache budget
//! still use plain backtracking.
//!
//! ```
//! use ferroni::api::{Regex, SearchOptions};
//! use ferroni::match_cache::MatchCacheConfig;
//!
//! let regex = Regex::builder(r"(a+)+$")
//!     .match_cache(MatchCacheConfig::new())
//!     .build()?;
//! assert!(regex.is_linear_time());
//! let text = format!("{}!", "a".repeat(4096));
//! let limits = SearchOptions::new().retry_limit_in_search(1_000_000);
//! assert!(regex.find_with(&text, limits)?.is_none());
//! # Ok::<(), ferroni::error::RegexError>(())
//! ```

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::oniguruma::{ONIG_OPTION_FIND_LONGEST, ONIG_OPTION_FIND_NOT_EMPTY, OnigOptionType};
use crate::regint::{OpCode, OperationPayload, RegexType};

/// Resource policy for an explicitly enabled match cache.
///
/// The budget covers allocated failure bits and pending failure records,
/// shared by all patterns in a scanner. Compiled pattern metadata and the
/// ordinary VM stack are separate. Allocation failure or budget exhaustion
/// disables memoization for the current subject; existing search limits remain
/// in force. No cache allocation survives a change of subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchCacheConfig {
    pub(crate) memory_budget: usize,
    activation_threshold: Option<usize>,
}

impl Default for MatchCacheConfig {
    fn default() -> Self {
        Self {
            memory_budget: 16 * 1024 * 1024,
            activation_threshold: None,
        }
    }
}

impl MatchCacheConfig {
    /// Use the default 16 MiB budget and adaptive activation threshold.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the total cache allocation budget in bytes. Zero disables it.
    pub const fn memory_budget(mut self, bytes: usize) -> Self {
        self.memory_budget = bytes;
        self
    }

    /// Activate after this much VM work instead of the adaptive threshold.
    ///
    /// Work counts failures and the forward bytes traversed since the last
    /// backtrack, so a long greedy loop counts even with few backtracks. Zero enables
    /// the cache immediately. The adaptive threshold is eight times the
    /// subject length times the number of cache points, with a 4,096 minimum.
    pub const fn activation_threshold(mut self, work: usize) -> Self {
        self.activation_threshold = Some(work);
        self
    }
}

pub(crate) fn allowed_options(options: OnigOptionType) -> bool {
    !options.intersects(ONIG_OPTION_FIND_LONGEST | ONIG_OPTION_FIND_NOT_EMPTY)
}

/// Exhaustive on purpose: adding an opcode requires an eligibility decision.
/// A cache key is only (instruction, byte position). Reject every instruction
/// whose continuation can read capture, repeat, assertion, or call state.
pub(crate) fn points(reg: &RegexType) -> Option<(Vec<Option<usize>>, usize, Vec<bool>)> {
    use OpCode::*;
    if !allowed_options(reg.options) || reg.enc.name() != "UTF-8" || reg.capture_history != 0 {
        return None;
    }
    // The compiler automatically makes disjoint character runs possessive
    // (for example \w* before a comma). Admit only a straight-line prefix
    // followed by ONE star and a matching cut; this region has one possible
    // successful exit. General atomic groups remain ineligible.
    let mut atomic_stars = vec![false; reg.ops.len()];
    let mut atomic_boundaries = vec![false; reg.ops.len()];
    for (pc, op) in reg.ops.iter().enumerate() {
        if op.opcode != Mark {
            continue;
        }
        let OperationPayload::Mark { id, .. } = op.payload else {
            return None;
        };
        let cut = (pc + 1..reg.ops.len()).find(|&i| reg.ops[i].opcode == CutToMark)?;
        if !matches!(reg.ops[cut].payload, OperationPayload::CutToMark { id: other, restore_pos: false } if other == id)
            || cut < pc + 2
            || !is_star(reg.ops[cut - 1].opcode)
        {
            return None;
        }
        for inner in &reg.ops[pc + 1..cut - 1] {
            if !matches!(
                inner.opcode,
                Str1 | Str2
                    | Str3
                    | Str4
                    | Str5
                    | StrN
                    | StrMb2n1
                    | StrMb2n2
                    | StrMb2n3
                    | StrMb2n
                    | StrMb3n
                    | StrMbn
                    | CClass
                    | CClassRun
                    | CClassMb
                    | CClassMix
                    | CClassNot
                    | CClassMbNot
                    | CClassMixNot
                    | AnyChar
                    | AnyCharMl
                    | Word
                    | WordAscii
                    | NoWord
                    | NoWordAscii
            ) {
                return None;
            }
        }
        atomic_boundaries[pc] = true;
        atomic_boundaries[cut] = true;
        atomic_stars[cut - 1] = true;
    }
    let mut count = 0;
    let mut points = Vec::with_capacity(reg.ops.len());
    for (pc, op) in reg.ops.iter().enumerate() {
        let branch = match op.opcode {
            Push
            | PushOrJumpExact1
            | PushIfPeekNext
            | PushOrJumpByteSet
            | AltLiterals
            | AnyCharStar
            | AnyCharMlStar
            | AnyCharStarPeekNext
            | AnyCharMlStarPeekNext
            | CClassStar
            | CClassMixStar
            | CClassMbStar
            | WordStar
            | WordAsciiStar
            | CClassStarPeekNext
            | WordAsciiStarPeekNext
            | CClassNotStar
            | CClassMbNotStar
            | CClassMixNotStar => true,
            Finish | End | Str1 | Str2 | Str3 | Str4 | Str5 | StrN | StrMb2n1 | StrMb2n2
            | StrMb2n3 | StrMb2n | StrMb3n | StrMbn | CClass | CClassRun | CClassMb | CClassMix
            | CClassNot | CClassMbNot | CClassMixNot | AnyChar | AnyCharMl | Word | WordAscii
            | NoWord | NoWordAscii | WordBoundary | NoWordBoundary | WordBegin | WordEnd
            | BeginBuf | EndBuf | BeginLine | EndLine | SemiEndBuf | MemStart | MemStartPush
            | MemEndPush | MemEnd | Fail | Jump => false,
            Mark | CutToMark => {
                if !atomic_boundaries[pc] {
                    return None;
                }
                false
            }
            TextSegmentBoundary
            | CheckPosition
            | BackRef1
            | BackRef2
            | BackRefN
            | BackRefNIc
            | BackRefMulti
            | BackRefMultiIc
            | BackRefWithLevel
            | BackRefWithLevelIc
            | BackRefCheck
            | BackRefCheckWithLevel
            | MemEndPushRec
            | MemEndRec
            | PushSuper
            | Pop
            | PopToMark
            | Repeat
            | RepeatNg
            | RepeatInc
            | RepeatIncNg
            | EmptyCheckStart
            | EmptyCheckEnd
            | EmptyCheckEndMemst
            | EmptyCheckEndMemstPush
            | Move
            | StepBackStart
            | StepBackNext
            | SaveVal
            | UpdateVar
            | Call
            | Return
            | CalloutContents
            | CalloutName
            | LookBehindOp => return None,
        };
        points.push(if branch || pc == 0 {
            let point = count;
            count += 1;
            Some(point)
        } else {
            None
        });
    }
    Some((points, count, atomic_stars))
}

pub(crate) fn is_star(opcode: OpCode) -> bool {
    use OpCode::*;
    matches!(
        opcode,
        AnyCharStar
            | AnyCharMlStar
            | AnyCharStarPeekNext
            | AnyCharMlStarPeekNext
            | CClassStar
            | CClassMixStar
            | CClassMbStar
            | WordStar
            | WordAsciiStar
            | CClassStarPeekNext
            | WordAsciiStarPeekNext
            | CClassNotStar
            | CClassMbNotStar
            | CClassMixNotStar
    )
}

pub(crate) struct Plan {
    pub(crate) points: Vec<Option<usize>>,
    pub(crate) count: usize,
    pub(crate) atomic_stars: Vec<bool>,
    pub(crate) config: MatchCacheConfig,
}

impl Plan {
    pub(crate) fn new(reg: &RegexType, config: MatchCacheConfig) -> Option<Self> {
        let (points, count, atomic_stars) = points(reg)?;
        // Acyclic bytecode already does bounded work per subject position.
        // Avoid all runtime bookkeeping for literals and finite alternatives,
        // which make up most ordinary grammar entries.
        if !reg.ops.iter().any(|op| {
            is_star(op.opcode) || matches!(op.payload, OperationPayload::Jump { addr } if addr < 0)
        }) {
            return None;
        }
        Some(Self {
            points,
            count,
            atomic_stars,
            config,
        })
    }
}

pub(crate) struct Budget {
    limit: usize,
    used: AtomicUsize,
}

impl Budget {
    pub(crate) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: AtomicUsize::new(0),
        })
    }

    fn reserve(&self, bytes: usize) -> bool {
        // This atomic only accounts for bytes. Cache contents are owned by
        // one matcher; no data publication depends on the counter's ordering.
        let mut used = self.used.load(Ordering::Relaxed);
        loop {
            let Some(next) = used.checked_add(bytes).filter(|&n| n <= self.limit) else {
                return false;
            };
            match self
                .used
                .compare_exchange_weak(used, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return true,
                Err(actual) => used = actual,
            }
        }
    }

    fn release(&self, bytes: usize) {
        self.used.fetch_sub(bytes, Ordering::Relaxed);
    }

    pub(crate) fn used(&self) -> usize {
        self.used.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Copy)]
struct Pending {
    bit: usize,
    stack_depth: usize,
}

/// Failure records are separate from the C VM stack. They are committed only
/// when backtracking passes their entry depth, after *all* alternatives of
/// that state failed. A successful return or a limit error discards pending
/// records without turning them into failures.
#[derive(Default)]
pub(crate) struct MatchCache {
    budget: Option<Arc<Budget>>,
    config: Option<MatchCacheConfig>,
    dimensions: Option<(usize, usize, usize)>,
    work: usize,
    threshold: usize,
    disabled: bool,
    bits: Vec<u64>,
    pending: Vec<Pending>,
    #[cfg(test)]
    pub(crate) hits: usize,
}

impl Drop for MatchCache {
    fn drop(&mut self) {
        self.release();
    }
}

impl MatchCache {
    pub(crate) fn new(config: MatchCacheConfig, budget: Arc<Budget>) -> Self {
        let mut cache = Self::default();
        cache.config = Some(config);
        cache.budget = Some(budget);
        cache
    }

    pub(crate) fn for_regex(reg: &RegexType) -> Option<Box<Self>> {
        reg.match_cache.as_ref().map(|plan| {
            Box::new(Self::new(
                plan.config,
                Budget::new(plan.config.memory_budget),
            ))
        })
    }

    fn release(&mut self) {
        let bytes = self.bits.capacity() * size_of::<u64>()
            + self.pending.capacity() * size_of::<Pending>();
        self.bits = Vec::new();
        self.pending = Vec::new();
        if bytes != 0 {
            if let Some(budget) = &self.budget {
                budget.release(bytes);
            }
        }
    }

    pub(crate) fn reset_subject(&mut self) {
        self.release();
        self.dimensions = None;
        self.disabled = false;
        self.work = 0;
    }

    pub(crate) fn disable(&mut self) {
        self.release();
        self.disabled = true;
    }

    pub(crate) fn prepare(&mut self, text: &[u8], end: usize, right: usize, points: usize) {
        let dimensions = (end, right, points);
        if self.dimensions != Some(dimensions) {
            self.release();
            self.dimensions = Some(dimensions);
            self.work = 0;
            self.disabled = std::str::from_utf8(&text[..end]).is_err();
            if let Some(config) = self.config {
                self.threshold = config
                    .activation_threshold
                    .unwrap_or_else(|| end.saturating_mul(points).saturating_mul(8).max(4096));
                self.disabled |= config.memory_budget == 0;
            } else {
                self.disabled = true;
            }
        }
        self.pending.clear();
    }

    /// Fallible growth charges actual vector capacity, including any allocator
    /// over-allocation. Failure releases the whole cache before falling back.
    fn grow<T>(budget: &Budget, vec: &mut Vec<T>, capacity: usize) -> bool {
        if capacity <= vec.capacity() {
            return true;
        }
        let old = vec.capacity();
        let Some(bytes) = (capacity - old).checked_mul(size_of::<T>()) else {
            return false;
        };
        if !budget.reserve(bytes) {
            return false;
        }
        if vec.try_reserve_exact(capacity - vec.len()).is_err() {
            budget.release(bytes);
            return false;
        }
        let extra = (vec.capacity() - capacity) * size_of::<T>();
        if extra != 0 && !budget.reserve(extra) {
            *vec = Vec::new();
            budget.release(old * size_of::<T>() + bytes);
            return false;
        }
        true
    }

    #[inline(always)]
    pub(crate) fn add_work(&mut self, work: usize) {
        self.work = self.work.saturating_add(work);
        if self.disabled || !self.bits.is_empty() || self.work < self.threshold {
            return;
        }
        self.activate();
    }

    #[cold]
    #[inline(never)]
    fn activate(&mut self) {
        let Some((end, _, points)) = self.dimensions else {
            return;
        };
        let words = end
            .checked_add(1)
            .and_then(|n| n.checked_mul(points))
            .and_then(|n| n.checked_add(63))
            .map(|n| n / 64);
        if let (Some(words), Some(budget)) = (words, self.budget.as_ref()) {
            if Self::grow(budget, &mut self.bits, words) {
                self.bits.resize(words, 0);
                return;
            }
        }
        self.disable();
    }

    #[inline(always)]
    pub(crate) fn active(&self) -> bool {
        !self.bits.is_empty()
    }

    #[inline]
    pub(crate) fn enter(&mut self, point: usize, position: usize, depth: usize) -> bool {
        if !self.active() {
            return false;
        }
        let bit = point * (self.dimensions.unwrap().0 + 1) + position;
        if self.bits[bit / 64] & (1 << (bit % 64)) != 0 {
            #[cfg(test)]
            {
                self.hits += 1;
            }
            return true;
        }
        if self.pending.len() == self.pending.capacity() {
            let capacity = self.pending.capacity().saturating_mul(2).max(64);
            if !Self::grow(self.budget.as_ref().unwrap(), &mut self.pending, capacity) {
                self.disable();
                return false;
            }
        }
        self.pending.push(Pending {
            bit,
            stack_depth: depth,
        });
        false
    }

    pub(crate) fn unwind(&mut self, depth: usize) {
        while self
            .pending
            .last()
            .is_some_and(|entry| entry.stack_depth > depth)
        {
            let entry = self.pending.pop().unwrap();
            self.bits[entry.bit / 64] |= 1 << (entry.bit % 64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grammar_loops_with_possessive_character_runs_are_eligible() {
        for pattern in [r"(?:\w*,)*x", r"(?:\w+\s*,\s*)*\w+\s*$"] {
            let reg = crate::regcomp::onig_new(
                pattern.as_bytes(),
                crate::oniguruma::ONIG_OPTION_NONE,
                &crate::encodings::utf8::ONIG_ENCODING_UTF8,
                &crate::regsyntax::OnigSyntaxOniguruma,
            )
            .unwrap();
            assert!(
                points(&reg).is_some(),
                "{pattern}: {:?}",
                reg.ops.iter().map(|op| op.opcode).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn forward_work_activates_the_cache_without_many_backtracks() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        use crate::oniguruma::*;
        use crate::regexec::{MatchArg, onig_search_with_msa};
        let mut reg = crate::regcomp::onig_new(
            br"(?:\w*,)*x",
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &crate::regsyntax::OnigSyntaxOniguruma,
        )
        .unwrap();
        // Isolate VM work from the required-literal prefilter. A retry-only
        // activation threshold leaves this search quadratic.
        reg.optimize = crate::regint::OptimizeType::None;
        reg.match_cache = Plan::new(&reg, MatchCacheConfig::new());
        for len in [20_000, 40_000, 80_000] {
            let text = vec![b'a'; len];
            let mut msa = MatchArg::new(&reg, ONIG_OPTION_NONE, None, 0);
            let result = onig_search_with_msa(&reg, &text, len, 0, len, &mut msa).0;
            assert_eq!(result, ONIG_MISMATCH);
            assert!(msa.match_cache.as_ref().unwrap().hits > len / 2);
            assert!(
                msa.match_cache.as_ref().unwrap().work < 100 * len,
                "{} units for {len} bytes",
                msa.match_cache.as_ref().unwrap().work
            );
        }
    }

    #[test]
    fn budget_accounts_for_all_buffers_and_releases_on_drop() {
        let config = MatchCacheConfig::new()
            .memory_budget(4096)
            .activation_threshold(0);
        let budget = Budget::new(config.memory_budget);
        {
            let mut first = MatchCache::new(config, budget.clone());
            let mut second = MatchCache::new(config, budget.clone());
            for cache in [&mut first, &mut second] {
                cache.prepare(b"aaaa", 4, 4, 2);
                cache.add_work(1);
                for i in 0..1000 {
                    cache.enter(0, 0, i);
                }
                assert!(budget.used() <= config.memory_budget);
            }
        }
        assert_eq!(budget.used(), 0);
    }
}
