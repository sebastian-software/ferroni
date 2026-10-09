//! Port of the `USE_REGSET` section of `regexec.c`: multi-regex search over a set
//! of compiled patterns, for syntax highlighters and text editors.
//! [`Scanner`](crate::scanner::Scanner) runs on it.

use crate::first_bytes::{
    ascii_before, characters_line_up, derive_start_byte_map, start_map_may_skip,
};
use crate::oniguruma::*;
use crate::regenc::{
    OnigEncoding, onigenc_get_prev_char_head, onigenc_is_ascii_compatible_encoding,
};
use crate::regexec::{
    FirstOpTest, MatchArg, OnigMatchParam, first_op_fails, first_op_test, forward_search,
    map_search_bypassed_for, may_skip_first_op_failures, onig_get_global_limit_revision,
    onig_get_match_stack_limit, onig_get_retry_limit_in_match, onig_get_retry_limit_in_search,
    onig_get_subexp_call_limit_in_search, onig_get_time_limit, onig_match,
    onig_match_with_msa_start, search_in_range, two_pass_capture_fill_pays,
};
use crate::regint::*;
use std::sync::Arc;

/// Search lead mode for regset search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnigRegSetLead {
    /// Position-lead: iterate positions, try all regexes at each position.
    /// Returns the first matching regex at the earliest position.
    PositionLead = 0,
    /// Regex-lead: iterate regexes, search full string for each.
    /// Returns the regex whose match starts earliest.
    RegexLead = 1,
    /// Like RegexLead but stops at the first regex that matches at the
    /// earliest position found so far (prioritizes regex order).
    PriorityToRegexOrder = 2,
}

struct RegSetEntry {
    /// The compiled regex, which other sets may share (Rust-only,
    /// `onig_regset_new_shared`). Searches only read it, and its deref
    /// costs what a `Box`'s does. Everything below is this entry's own.
    reg: Arc<RegexType>,
    region: Option<OnigRegion>,
    /// Caching a fallback search must not suppress observable callouts or
    /// position-sensitive bytecode such as partial `\G` anchors.
    fallback_memo_safe: bool,
    /// The entry's required literals may rule out attempts
    /// (`required_literals_are_safe`).
    required_literals_safe: bool,
    /// Start-byte map of a fallback entry whose optimizer cannot bound the
    /// match start (`dist_max` infinite). Its search would otherwise run the
    /// VM at every position; see `fallback_start_filter`.
    start_filter: Option<Box<[u8; CHAR_MAP_SIZE]>>,
    /// The entry is searched on its own after the table pass
    /// (`fallback_search_candidates`) rather than dispatched by the table.
    fallback: bool,
    /// A table entry dispatched by its bytecode start bytes whose optimizer
    /// has a distance: the table scan attempts it only where C's
    /// optimizer admits the position (`table_gate_admits`).
    gated: bool,
    /// Rust-only (ADR-008): the set's DFA pre-filter decides where this
    /// entry is attempted. The other entries of such a set are searched as
    /// the position-lead search searches them, interleaved by position.
    prefilter_covered: bool,
    /// The regex has callouts, which observe every attempt.
    has_callouts: bool,
    /// The first instruction, when the position loops can reject an attempt
    /// by it before entering the VM (`first_op_rejects`).
    first_op: Option<FirstOpTest>,
}

/// Pre-computed memchr needle for SIMD-accelerated position skipping.
///
/// When the dispatch table has only 1–3 non-empty byte slots (and zero
/// always-candidate patterns), we can use `memchr` to jump directly to the
/// next position where at least one table-dispatched pattern could match.
#[derive(Clone, Copy)]
enum SkipNeedle {
    /// No skipping possible (always-candidate patterns exist, or >3 bytes).
    None,
    One(u8),
    Two(u8, u8),
    Three(u8, u8, u8),
}

/// A set of compiled regexes that can be searched simultaneously.
pub struct OnigRegSet {
    entries: Vec<RegSetEntry>,
    enc: OnigEncoding,
    anchor: i32,
    anc_dmin: OnigLen,
    anc_dmax: OnigLen,
    all_low_high: bool,
    anychar_inf: bool,
    /// For each byte value 0..255, the list of entry indices whose first-byte
    /// pre-filter does not exclude that byte. Built at construction time.
    first_byte_candidates: Box<[Vec<u16>; 256]>,
    /// Number of entries routed through `first_byte_candidates`. A pure
    /// fallback set has no table work at any position.
    table_entry_count: usize,
    /// The entries routed through `first_byte_candidates`, in index order:
    /// the candidates at the logical end, where no byte dispatches.
    table_entries: Vec<u16>,
    /// Entries whose start byte cannot be derived safely from bytecode, in
    /// index order. They are searched independently with their own optimizer
    /// after the table pass, rather than routing on an optimizer byte that
    /// can occur later than the true match start.
    fallback_search_candidates: Vec<FallbackCandidate>,
    /// Scanner-only memoization for optimizer-backed fallback searches. The
    /// caller supplies a stable immutable string identity, so a no-match or
    /// a later match can be reused as tokenization advances.
    fallback_memo_key: Option<FallbackMemoKey>,
    fallback_memos: Vec<Vec<FallbackMemo>>,
    /// Global limits captured by `scratch_msa`. Identity changes invalidate
    /// cached results, but do not require discarding its reusable buffers.
    scratch_limits: Option<FallbackMemoLimits>,
    /// Revision of the process-global limits captured in `scratch_limits`.
    /// The revision check is intentionally cheaper than reloading all limits
    /// for table-only scanner calls.
    scratch_limits_revision: Option<u64>,
    /// Cumulative search retry budgets for table-routed entries. Oniguruma
    /// accumulates this budget across positions per regex, not across
    /// different regexes at the same position.
    scratch_table_retry_counters: Vec<u64>,
    /// SIMD-accelerated skip needle derived from the dispatch table.
    skip_needle: SkipNeedle,
    /// Table entries a Rust-only start map routes past a leading positive
    /// look-behind, in index order, and the most characters a table entry's
    /// leading look-behind steps back. Where those characters do not line up
    /// the table attempts these entries whatever the byte, and no entry with
    /// a leading look-behind by its threshold
    /// (`crate::first_bytes::start_map_may_skip`).
    look_behind_entries: Vec<u16>,
    look_behind_reach: u32,
    /// The subject of the current search is valid UTF-8 (`MatchArg::subject_utf8`).
    subject_utf8: bool,
    /// Reused MatchArg scratch space for position-lead searches.
    scratch_msa: Option<MatchArg>,
    /// Match length from the last successful position-lead search.
    last_match_len: i32,
    /// Per table entry, the bytes whose `first_byte_candidates` slot holds
    /// it (`None` for a fallback entry). Built on first use by
    /// `onig_regset_entry_search`; cleared whenever the table changes.
    table_start_bytes: Option<Vec<Option<[u64; 4]>>>,
    /// Positions the table scan has looked at, for tests of its cost.
    #[cfg(test)]
    table_positions_scanned: u64,
    /// Per entry, the positions a gated table entry may attempt.
    gates: Vec<EntryGate>,
    /// Gates of another generation are stale. It changes with every search
    /// except successive ones over the same identified subject
    /// (`gate_subject`), where a gate searched from an earlier start can
    /// still serve.
    gate_generation: u32,
    gate_subject: Option<(FallbackMemoIdentity, usize)>,
    /// Some table entry is gated.
    has_gated: bool,
    /// Some gated table entry has callouts.
    has_gated_callouts: bool,
    /// Rust-only (ADR-008): the DFA pre-filter of a scanner's set
    /// (`crate::dfa_prefilter`), built by `onig_regset_new_shared` and
    /// dropped with the first-byte table.
    #[cfg(feature = "dfa-prefilter")]
    prefilter: Option<Box<crate::dfa_prefilter::SetPrefilter>>,
    /// Some fallback entry is not covered by the pre-filter, which then
    /// searches it with the fallback memo.
    #[cfg(feature = "dfa-prefilter")]
    prefilter_own_fallback: bool,
}

/// A fallback entry as the position-lead search walks it on every call.
///
/// A warm scanner call visits every fallback entry, and most of them hold a
/// settled no-match result. Keeping the flags that decide a skip and that
/// result in one contiguous array makes such an entry cost a few loads
/// instead of dereferencing its regex and its memo vector.
#[derive(Clone, Copy, Debug)]
struct FallbackCandidate {
    index: u16,
    /// Copy of the entry's `fallback_memo_safe`.
    memo_safe: bool,
    /// The entry is anchored to the search start (`\G`).
    begin_position: bool,
    /// The entry starts with an any-char star (`ANCR_ANYCHAR_INF`): a
    /// position-lead search attempts it only at its first position and after
    /// a newline.
    after_newline_only: bool,
    /// Memoized: a search from this position found no match up to the end
    /// of the subject, so no later start can match either. `usize::MAX`
    /// when unknown. Valid for the current `fallback_memo_key` only.
    no_match_from: usize,
    /// Copy of the entry's newest `FallbackMemo::ExactStartMiss`, or
    /// `usize::MAX`, so a warm call settles without its memo vector.
    exact_miss: usize,
    /// Copy of the entry's newest `FallbackMemo::MatchAt` as
    /// `(searched_from, position)`, or `usize::MAX` in both.
    match_from: usize,
    match_at: usize,
}

impl FallbackCandidate {
    fn new(index: usize, entry: &RegSetEntry) -> Self {
        Self {
            index: index as u16,
            memo_safe: entry.fallback_memo_safe,
            begin_position: (entry.reg.anchor & ANCR_BEGIN_POSITION) != 0,
            after_newline_only: (entry.reg.anchor & ANCR_ANYCHAR_INF) != 0,
            no_match_from: usize::MAX,
            exact_miss: usize::MAX,
            match_from: usize::MAX,
            match_at: usize::MAX,
        }
    }
}

#[derive(Clone, Copy)]
enum FallbackMemo {
    /// A direct position-lead attempt failed at this exact start. It is safe
    /// to skip only an identical retry; a different start upgrades to an
    /// optimizer search so mixed table/fallback scans remain linear.
    ExactStartMiss(usize),
    MatchAt {
        searched_from: usize,
        position: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FallbackMemoKey {
    identity: FallbackMemoIdentity,
    end: usize,
    option: OnigOptionType,
    retry_limit_in_match: u64,
    retry_limit_in_search: u64,
    match_stack_limit: u32,
    time_limit: u64,
    subexp_call_limit_in_search: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FallbackMemoLimits {
    retry_limit_in_match: u64,
    retry_limit_in_search: u64,
    match_stack_limit: u32,
    time_limit: u64,
    subexp_call_limit_in_search: u64,
}

impl FallbackMemoLimits {
    fn current() -> Self {
        Self {
            retry_limit_in_match: onig_get_retry_limit_in_match(),
            retry_limit_in_search: onig_get_retry_limit_in_search(),
            match_stack_limit: onig_get_match_stack_limit(),
            time_limit: onig_get_time_limit(),
            subexp_call_limit_in_search: onig_get_subexp_call_limit_in_search(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FallbackMemoIdentity {
    Caller(u64),
    OnigString(u64),
}

const FALLBACK_MEMO_CAPACITY: usize = 8;

/// Position filter for a fallback entry's search.
///
/// With an infinite `dist_max`, Oniguruma's search checks the optimizer once
/// and then attempts a match at every position of the range. The bytecode's
/// start-byte map (the same proof that routes table entries) excludes the
/// positions where no match can start, so the search can step over them
/// without running the VM. Only used where skipping a failed attempt is
/// unobservable: no callouts or position checks (`fallback_memo_is_safe`).
fn fallback_start_filter(reg: &RegexType) -> Option<Box<[u8; CHAR_MAP_SIZE]>> {
    (has_variable_optimizer(reg) && reg.dist_max == INFINITE_LEN && fallback_memo_is_safe(reg))
        .then(|| derive_start_byte_map(reg))
        .flatten()
        .map(Box::new)
}

/// Rust-only (ADR-008): the required literals of a fallback entry, where
/// the search may leave out the attempts they rule out: the entry allows it
/// (`required_literals_are_safe`), and no limit observes the left-out
/// attempts (`RequiredLiterals::applies`).
#[inline]
fn required_literals<'a>(
    entry: &'a RegSetEntry,
    option: OnigOptionType,
    msa: &MatchArg,
) -> Option<&'a crate::required_literals::RequiredLiterals> {
    let reg = &entry.reg;
    reg.required_literals.as_deref().filter(|required| {
        entry.required_literals_safe
            && required.applies(msa, opton_find_longest(option | reg.options))
    })
}

/// Whether a fallback search may leave out the attempts `reg`'s required
/// literals rule out: no callouts, which observe every attempt, and no
/// look-behind that checks its trailing literal (`OpCode::Move`), after
/// which the match can go on before the attempt start (see
/// `required_literals_keep_attempts_of_entries_with_position_checks`).
/// Other position checks (`\G`, look-behinds without that check) leave the
/// position where it was (`crate::required_literals`).
fn required_literals_are_safe(reg: &RegexType) -> bool {
    reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0)
        && !reg.ops.iter().any(|op| op.opcode == OpCode::Move)
}

/// The first occurrence of `reg`'s required literals in `s..end`.
#[inline(never)]
fn next_required_literal(reg: &RegexType, str_data: &[u8], s: usize, end: usize) -> Option<usize> {
    reg.required_literals.as_deref()?.find(str_data, s, end)
}

#[inline]
fn fallback_memo_is_safe(reg: &RegexType) -> bool {
    reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0)
        && !reg.ops.iter().any(|op| op.opcode == OpCode::CheckPosition)
}

#[inline]
fn enclen(enc: OnigEncoding, str_data: &[u8], s: usize) -> usize {
    if s >= str_data.len() {
        return 1;
    }
    if str_data[s] < 0x80 && onigenc_is_ascii_compatible_encoding(enc) {
        return 1;
    }
    enc.mbc_enc_len(&str_data[s..])
}

#[inline]
fn has_variable_optimizer(reg: &RegexType) -> bool {
    reg.dist_max > 0
        && match reg.optimize {
            OptimizeType::Map => true,
            OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward => {
                !reg.exact.is_empty()
            }
            _ => false,
        }
}

/// The bytes that dispatch a table entry with a variable optimizer: its
/// bytecode start bytes, narrowed for a `start_dispatch` entry by the start
/// bytes the Rust-only maps give.
fn table_start_map(reg: &RegexType) -> Option<[u8; CHAR_MAP_SIZE]> {
    let mut start_map = derive_start_byte_map(reg)?;
    if reg.start_dispatch && reg.has_first_byte_map {
        for (byte, dispatch) in start_map.iter_mut().zip(&reg.first_byte_map) {
            *byte &= *dispatch;
        }
    }
    Some(start_map)
}

/// Whether an entry with a variable optimizer is dispatched by its
/// bytecode start bytes rather than searched on its own.
#[inline]
fn table_routes_by_start_map(reg: &RegexType) -> bool {
    reg.dist_max != INFINITE_LEN || reg.start_dispatch
}

#[inline]
fn has_finite_variable_optimizer(reg: &RegexType) -> bool {
    reg.dist_max != INFINITE_LEN && has_variable_optimizer(reg)
}

/// The entries of two lists in index order, each once.
fn merge_entry_lists(a: &[u16], b: &[u16]) -> Vec<u16> {
    let mut merged = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        let next = match (a.get(i), b.get(j)) {
            (Some(&x), Some(&y)) if x == y => {
                i += 1;
                j += 1;
                x
            }
            (Some(&x), Some(&y)) if x < y => {
                i += 1;
                x
            }
            (Some(_), Some(&y)) | (None, Some(&y)) => {
                j += 1;
                y
            }
            (Some(&x), None) => {
                i += 1;
                x
            }
            (None, None) => unreachable!(),
        };
        merged.push(next);
    }
    merged
}

/// The table entries a Rust-only start map routes past a leading positive
/// look-behind (`OnigRegSet::look_behind_entries`): C's own optimizer
/// windows hold such a look-behind as zero-width too, so only the bytecode
/// start bytes and the Rust-only first-byte maps need the check.
fn refresh_look_behind_entries(set: &mut OnigRegSet) {
    let routed_by_rust_map = |reg: &RegexType| {
        if has_variable_optimizer(reg) {
            true
        } else if reg.optimize == OptimizeType::Map && reg.dist_min == 0 {
            reg.extra_map_optimizer
        } else if reg.dist_min == 0 && !reg.exact.is_empty() {
            false
        } else {
            reg.has_first_byte_map
        }
    };
    set.look_behind_entries = set
        .table_entries
        .iter()
        .copied()
        .filter(|&i| {
            let reg = &set.entries[i as usize].reg;
            reg.look_behind_reach > 0 && routed_by_rust_map(reg)
        })
        .collect();
    set.look_behind_entries.sort_unstable();
    // The reach of every table entry: the threshold pre-filter is Rust-only
    // for C-routed entries too.
    set.look_behind_reach = set
        .table_entries
        .iter()
        .map(|&i| set.entries[i as usize].reg.look_behind_reach)
        .max()
        .unwrap_or(0);
}

fn add_entry_by_start_map(
    table: &mut [Vec<u16>; CHAR_MAP_SIZE],
    start_map: &[u8; CHAR_MAP_SIZE],
    idx: u16,
) {
    for (byte, entries) in table.iter_mut().enumerate() {
        if start_map[byte] != 0 {
            entries.push(idx);
        }
    }
}

/// Add a single non-variable entry to the appropriate first-byte slots.
fn add_entry_to_first_byte_table(table: &mut [Vec<u16>; 256], reg: &RegexType, idx: u16) {
    if reg.optimize == OptimizeType::Map && reg.dist_min == 0 {
        // Map-filterable: only bytes where map[b] != 0
        for (b, slot) in table.iter_mut().enumerate() {
            if reg.map[b] != 0 {
                slot.push(idx);
            }
        }
    } else if reg.dist_min == 0 && !reg.exact.is_empty() {
        // Exact-filterable: only the first byte of the exact string
        table[reg.exact[0] as usize].push(idx);
    } else if reg.has_first_byte_map {
        // Fallback: use the first-byte prefilter map.
        for (b, slot) in table.iter_mut().enumerate() {
            if reg.first_byte_map[b] != 0 {
                slot.push(idx);
            }
        }
    } else {
        // Always-candidate: appears in all 256 slots.
        for slot in table.iter_mut() {
            slot.push(idx);
        }
    }
}

/// Derive the skip needle from a completed dispatch table.
fn compute_skip_needle(table: &[Vec<u16>; 256]) -> SkipNeedle {
    let mut bytes: Vec<u8> = Vec::new();
    for (b, slot) in table.iter().enumerate() {
        if !slot.is_empty() {
            bytes.push(b as u8);
            if bytes.len() > 3 {
                return SkipNeedle::None;
            }
        }
    }
    match bytes.len() {
        0 => SkipNeedle::None,
        1 => SkipNeedle::One(bytes[0]),
        2 => SkipNeedle::Two(bytes[0], bytes[1]),
        3 => SkipNeedle::Three(bytes[0], bytes[1], bytes[2]),
        _ => SkipNeedle::None,
    }
}

/// Build the first-byte dispatch table from scratch for all entries.
fn build_first_byte_table(set: &mut OnigRegSet) {
    let mut table: Box<[Vec<u16>; 256]> = Box::new(std::array::from_fn(|_| Vec::new()));
    let mut fallback_search_candidates = Vec::new();
    let mut table_entries = Vec::new();
    for (i, entry) in set.entries.iter_mut().enumerate() {
        entry.fallback = false;
        entry.gated = false;
        if has_variable_optimizer(&entry.reg) {
            // A start-byte map proves semantic routing, but an unbounded
            // prefix could still re-run its VM at every matching byte. Keep
            // those entries on their optimizer-backed fallback; only a
            // finite prefix, or a match start the Rust-only maps pin down
            // (`start_dispatch`), has a bounded table-dispatch cost.
            if table_routes_by_start_map(&entry.reg) {
                if let Some(start_map) = table_start_map(&entry.reg) {
                    add_entry_by_start_map(&mut table, &start_map, i as u16);
                    table_entries.push(i as u16);
                    entry.gated = true;
                    continue;
                }
            }
            entry.fallback = true;
            fallback_search_candidates.push(FallbackCandidate::new(i, entry));
        } else {
            add_entry_to_first_byte_table(&mut table, &entry.reg, i as u16);
            table_entries.push(i as u16);
        }
    }
    set.skip_needle = compute_skip_needle(&table);
    set.first_byte_candidates = table;
    set.table_entry_count = table_entries.len();
    set.table_entries = table_entries;
    refresh_look_behind_entries(set);
    set.fallback_search_candidates = fallback_search_candidates;
    set.fallback_memo_key = None;
    set.fallback_memos = vec![Vec::new(); set.entries.len()];
    set.scratch_limits = None;
    set.scratch_limits_revision = None;
    set.scratch_table_retry_counters = vec![0; set.entries.len()];
    set.scratch_msa = None;
    set.table_start_bytes = None;
    #[cfg(feature = "dfa-prefilter")]
    {
        set.prefilter = None;
        set.prefilter_own_fallback = false;
    }
    set.has_gated = set.entries.iter().any(|entry| entry.gated);
    set.has_gated_callouts = set
        .entries
        .iter()
        .any(|entry| entry.gated && entry.has_callouts);
    set.gates = vec![EntryGate::STALE; set.entries.len()];
    set.gate_generation = 0;
    set.gate_subject = None;
}

/// Create a new regex set from an array of compiled regexes.
/// Returns (Some(set), ONIG_NORMAL) on success, (None, error_code) on failure.
pub fn onig_regset_new(regs: Vec<Box<RegexType>>) -> (Option<Box<OnigRegSet>>, i32) {
    let mut set = regset_alloc();

    for reg in regs {
        let r = onig_regset_add(&mut set, reg);
        if r != ONIG_NORMAL {
            return (None, r);
        }
    }

    build_first_byte_table(&mut set);

    (Some(set), ONIG_NORMAL)
}

/// Rust-only: `onig_regset_new` over compiled regexes that other sets may
/// hold as well. Through the scanner's pattern cache
/// (`crate::scanner::ScannerPatternCache`), the scanners that repeat a
/// pattern share its compiled regex. Each set still builds its own entries:
/// regions, memos, gates, start filters and first-instruction tests are
/// never shared, and a search only reads the regex.
///
/// `seeks` holds each regex's seek approximation (ADR-008,
/// `crate::dfa_prefilter`), from which the set builds its DFA pre-filter;
/// a set without one for any entry, without the feature, or whose automata
/// would be too large searches as before.
pub(crate) fn onig_regset_new_shared(
    regs: Vec<Arc<RegexType>>,
    seeks: &[SharedSeek],
) -> (Option<Box<OnigRegSet>>, i32) {
    debug_assert_eq!(regs.len(), seeks.len());
    let mut set = regset_alloc();

    for reg in regs {
        let r = onig_regset_add_shared(&mut set, reg);
        if r != ONIG_NORMAL {
            return (None, r);
        }
    }

    build_first_byte_table(&mut set);
    #[cfg(feature = "dfa-prefilter")]
    {
        let entries =
            (set.entries.iter().zip(seeks)).map(|(entry, seek)| (seek.as_deref(), &*entry.reg));
        set.prefilter = crate::dfa_prefilter::SetPrefilter::build(entries).map(Box::new);
        if let Some(mut prefilter) = set.prefilter.take() {
            for &index in prefilter.covered_entries() {
                set.entries[index as usize].prefilter_covered = true;
            }
            set.prefilter_own_fallback = prefilter
                .own()
                .iter()
                .any(|&index| set.entries[index as usize].fallback);
            prefilter.set_dispatch(|index| {
                if set.entries[index as usize].fallback {
                    [u64::MAX; 4]
                } else {
                    table_start_bytes(&mut set, index as usize)
                }
            });
            set.prefilter = Some(prefilter);
        }
    }

    (Some(set), ONIG_NORMAL)
}

/// A scanner pattern's seek approximation as scanners share it: the pattern
/// cache keeps one per distinct pattern (ADR-006), and every set built from
/// it reads the same. Nothing without the `dfa-prefilter` feature.
#[cfg(feature = "dfa-prefilter")]
pub(crate) type SharedSeek = Option<Arc<crate::dfa_prefilter::Seek>>;
/// See the definition with the `dfa-prefilter` feature.
#[cfg(not(feature = "dfa-prefilter"))]
pub(crate) type SharedSeek = ();

/// An empty set, as `onig_regset_new` allocates it before adding regexes.
fn regset_alloc() -> Box<OnigRegSet> {
    Box::new(OnigRegSet {
        entries: Vec::new(),
        enc: &crate::encodings::utf8::ONIG_ENCODING_UTF8,
        anchor: 0,
        anc_dmin: 0,
        anc_dmax: 0,
        all_low_high: false,
        anychar_inf: false,
        first_byte_candidates: Box::new(std::array::from_fn(|_| Vec::new())),
        table_entry_count: 0,
        table_entries: Vec::new(),
        fallback_search_candidates: Vec::new(),
        fallback_memo_key: None,
        fallback_memos: Vec::new(),
        scratch_limits: None,
        scratch_limits_revision: None,
        scratch_table_retry_counters: Vec::new(),
        skip_needle: SkipNeedle::None,
        look_behind_entries: Vec::new(),
        look_behind_reach: 0,
        subject_utf8: false,
        scratch_msa: None,
        last_match_len: ONIG_MISMATCH,
        table_start_bytes: None,
        #[cfg(test)]
        table_positions_scanned: 0,
        gates: Vec::new(),
        gate_generation: 0,
        gate_subject: None,
        has_gated: false,
        has_gated_callouts: false,
        #[cfg(feature = "dfa-prefilter")]
        prefilter: None,
        #[cfg(feature = "dfa-prefilter")]
        prefilter_own_fallback: false,
    })
}

/// Add a compiled regex to the set. Returns ONIG_NORMAL on success.
pub fn onig_regset_add(set: &mut OnigRegSet, reg: Box<RegexType>) -> i32 {
    onig_regset_add_shared(set, Arc::from(reg))
}

/// Rust-only: `onig_regset_add` for a compiled regex that other sets may
/// hold as well (`onig_regset_new_shared`).
pub(crate) fn onig_regset_add_shared(set: &mut OnigRegSet, reg: Arc<RegexType>) -> i32 {
    if opton_find_longest(reg.options) {
        return ONIGERR_INVALID_ARGUMENT;
    }

    if !set.entries.is_empty() && !std::ptr::eq(reg.enc, set.enc) {
        return ONIGERR_INVALID_ARGUMENT;
    }

    let region = Some(OnigRegion::new());
    let fallback_memo_safe = fallback_memo_is_safe(&reg);
    let required_literals_safe = required_literals_are_safe(&reg);
    let start_filter = fallback_start_filter(&reg);
    let first_op = first_op_test(&reg);
    set.entries.push(RegSetEntry {
        reg,
        region,
        fallback_memo_safe,
        required_literals_safe,
        start_filter,
        fallback: false,
        gated: false,
        prefilter_covered: false,
        has_callouts: false,
        first_op,
    });
    let entry = set.entries.last_mut().expect("just pushed");
    entry.has_callouts = entry
        .reg
        .extp
        .as_ref()
        .is_some_and(|ext| ext.callout_num != 0);
    set.table_start_bytes = None;
    #[cfg(feature = "dfa-prefilter")]
    {
        set.prefilter = None;
        set.prefilter_own_fallback = false;
    }
    set.fallback_memo_key = None;
    set.fallback_memos.resize_with(set.entries.len(), Vec::new);
    set.scratch_limits = None;
    set.scratch_limits_revision = None;
    set.scratch_table_retry_counters
        .resize(set.entries.len(), 0);
    set.scratch_table_retry_counters.fill(0);
    set.scratch_msa = None;

    // Add the new entry to the first-byte dispatch table
    let new_idx = (set.entries.len() - 1) as u16;
    if has_variable_optimizer(&set.entries[new_idx as usize].reg) {
        let start_map = table_routes_by_start_map(&set.entries[new_idx as usize].reg)
            .then(|| table_start_map(&set.entries[new_idx as usize].reg))
            .flatten();
        if let Some(start_map) = start_map {
            add_entry_by_start_map(&mut set.first_byte_candidates, &start_map, new_idx);
            set.table_entry_count += 1;
            set.table_entries.push(new_idx);
            set.entries[new_idx as usize].gated = true;
            set.has_gated = true;
            set.has_gated_callouts |= set.entries[new_idx as usize].has_callouts;
            set.gates.resize(set.entries.len(), EntryGate::STALE);
            set.skip_needle = compute_skip_needle(&set.first_byte_candidates);
            refresh_look_behind_entries(set);
        } else {
            set.entries[new_idx as usize].fallback = true;
            let candidate =
                FallbackCandidate::new(new_idx as usize, &set.entries[new_idx as usize]);
            set.fallback_search_candidates.push(candidate);
        }
    } else {
        add_entry_to_first_byte_table(
            &mut set.first_byte_candidates,
            &set.entries[new_idx as usize].reg,
            new_idx,
        );
        set.table_entry_count += 1;
        set.table_entries.push(new_idx);
        set.skip_needle = compute_skip_needle(&set.first_byte_candidates);
        refresh_look_behind_entries(set);
    }

    // Recompute: pass field values to avoid borrow conflict
    let n = set.entries.len();
    let reg_ref = &*set.entries[n - 1].reg;
    let anchor = reg_ref.anchor;
    let anc_dist_min = reg_ref.anc_dist_min;
    let anc_dist_max = reg_ref.anc_dist_max;
    let optimize = reg_ref.optimize;
    let dist_max = reg_ref.dist_max;

    if n == 1 {
        set.enc = reg_ref.enc;
        set.anchor = anchor;
        set.anc_dmin = anc_dist_min;
        set.anc_dmax = anc_dist_max;
        set.all_low_high = optimize != OptimizeType::None && dist_max != INFINITE_LEN;
        set.anychar_inf = (anchor & ANCR_ANYCHAR_INF) != 0;
    } else {
        let new_anchor = set.anchor & anchor;
        if new_anchor != 0 {
            if anc_dist_min < set.anc_dmin {
                set.anc_dmin = anc_dist_min;
            }
            if anc_dist_max > set.anc_dmax {
                set.anc_dmax = anc_dist_max;
            }
        }
        set.anchor = new_anchor;
        if optimize == OptimizeType::None || dist_max == INFINITE_LEN {
            set.all_low_high = false;
        }
        if (anchor & ANCR_ANYCHAR_INF) != 0 {
            set.anychar_inf = true;
        }
    }

    ONIG_NORMAL
}

/// Replace a regex at index `at`, or remove it if `reg` is None.
/// Returns ONIG_NORMAL on success.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_replace(set: &mut OnigRegSet, at: usize, reg: Option<Box<RegexType>>) -> i32 {
    if at >= set.entries.len() {
        return ONIGERR_INVALID_ARGUMENT;
    }

    match reg {
        None => {
            // Remove entry at `at`
            set.entries.remove(at);
        }
        Some(reg) => {
            if opton_find_longest(reg.options) {
                return ONIGERR_INVALID_ARGUMENT;
            }
            if set.entries.len() > 1 && !std::ptr::eq(reg.enc, set.enc) {
                return ONIGERR_INVALID_ARGUMENT;
            }
            set.entries[at].fallback_memo_safe = fallback_memo_is_safe(&reg);
            set.entries[at].required_literals_safe = required_literals_are_safe(&reg);
            set.entries[at].has_callouts =
                reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0);
            set.entries[at].start_filter = fallback_start_filter(&reg);
            set.entries[at].first_op = first_op_test(&reg);
            set.entries[at].reg = Arc::from(reg);
        }
    }

    // Recompute aggregate fields from all entries
    if !set.entries.is_empty() {
        // Reset and recompute by replaying updates
        let first_enc = set.entries[0].reg.enc;
        set.enc = first_enc;
        set.anchor = 0;
        set.anc_dmin = 0;
        set.anc_dmax = 0;
        set.all_low_high = false;
        set.anychar_inf = false;

        // Temporarily collect reg references to avoid borrow issues
        let reg_data: Vec<(i32, OnigLen, OnigLen, OptimizeType, OnigLen, i32)> = set
            .entries
            .iter()
            .map(|e| {
                (
                    e.reg.anchor,
                    e.reg.anc_dist_min,
                    e.reg.anc_dist_max,
                    e.reg.optimize,
                    e.reg.dist_max,
                    0, // placeholder
                )
            })
            .collect();

        for (i, (anchor, anc_dist_min, anc_dist_max, optimize, dist_max, _)) in
            reg_data.iter().enumerate()
        {
            if i == 0 {
                set.anchor = *anchor;
                set.anc_dmin = *anc_dist_min;
                set.anc_dmax = *anc_dist_max;
                set.all_low_high = *optimize != OptimizeType::None && *dist_max != INFINITE_LEN;
                set.anychar_inf = (*anchor & ANCR_ANYCHAR_INF) != 0;
            } else {
                let new_anchor = set.anchor & anchor;
                if new_anchor != 0 {
                    if *anc_dist_min < set.anc_dmin {
                        set.anc_dmin = *anc_dist_min;
                    }
                    if *anc_dist_max > set.anc_dmax {
                        set.anc_dmax = *anc_dist_max;
                    }
                }
                set.anchor = new_anchor;
                if *optimize == OptimizeType::None || *dist_max == INFINITE_LEN {
                    set.all_low_high = false;
                }
                if (*anchor & ANCR_ANYCHAR_INF) != 0 {
                    set.anychar_inf = true;
                }
            }
        }
    }

    // Rebuild first-byte dispatch table from scratch
    build_first_byte_table(set);

    ONIG_NORMAL
}

/// Return the number of regexes in the set.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_number_of_regex(set: &OnigRegSet) -> i32 {
    set.entries.len() as i32
}

/// Get a reference to the regex at index `at`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_get_regex(set: &OnigRegSet, at: usize) -> Option<&RegexType> {
    set.entries.get(at).map(|e| e.reg.as_ref())
}

/// Get a reference to the region at index `at`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn onig_regset_get_region(set: &OnigRegSet, at: usize) -> Option<&OnigRegion> {
    set.entries.get(at).and_then(|e| e.region.as_ref())
}

/// Return the match length from the last successful position-lead search.
///
/// The length is measured from the position the winning attempt began at --
/// the position the search itself returns. For a pattern that uses `\K` the
/// match starts elsewhere, so the pair describes the match only while the
/// winning regex has neither a capture group nor `\K`; every other caller
/// reads that regex's region.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn onig_regset_last_match_len(set: &OnigRegSet) -> i32 {
    set.last_match_len
}

/// Rust-only (ADR-008): the set's DFA pre-filter, if it built one.
#[cfg(feature = "dfa-prefilter")]
pub(crate) fn onig_regset_prefilter(
    set: &OnigRegSet,
) -> Option<&crate::dfa_prefilter::SetPrefilter> {
    set.prefilter.as_deref()
}

#[derive(Clone, Copy)]
struct RegSetWinner {
    index: i32,
    position: i32,
    match_len: i32,
}

#[derive(Clone, Copy)]
struct RegSetError {
    code: i32,
    index: i32,
    position: i32,
}

#[derive(Clone, Copy)]
enum RegSetDecision {
    Match(RegSetWinner),
    Error(RegSetError),
}

#[inline]
fn decision_position_and_index(decision: RegSetDecision) -> (i32, i32) {
    match decision {
        RegSetDecision::Match(candidate) => (candidate.position, candidate.index),
        RegSetDecision::Error(error) => (error.position, error.index),
    }
}

#[inline]
fn decision_is_better(candidate: RegSetDecision, current: Option<RegSetDecision>) -> bool {
    current.is_none_or(|current| {
        decision_position_and_index(candidate) < decision_position_and_index(current)
    })
}

fn clear_regset_entry_region(set: &mut OnigRegSet, index: i32) {
    if let Some(region) = set.entries[index as usize].region.as_mut() {
        region.clear();
    }
}

fn record_regset_decision(
    set: &mut OnigRegSet,
    current: &mut Option<RegSetDecision>,
    candidate: RegSetDecision,
) {
    if decision_is_better(candidate, *current) {
        if let Some(RegSetDecision::Match(previous)) = current {
            clear_regset_entry_region(set, previous.index);
        }
        *current = Some(candidate);
    } else if let RegSetDecision::Match(candidate) = candidate {
        clear_regset_entry_region(set, candidate.index);
    }
}

/// True when a match of `reg` is fully described by the position its attempt
/// began at plus the length `onig_match` reports, so the position-lead search
/// can skip populating a region.
///
/// Capture groups need the region for their own spans, and `\K` moves the
/// whole match's start away from the attempt position, which neither the
/// returned position nor the length carries.
#[inline]
fn region_is_redundant(reg: &RegexType) -> bool {
    reg.num_mem == 0 && !reg.keep_moves_match_start
}

/// How an attempt of a fallback entry fills its region. Like `onig_search`,
/// the entry's search always leaves its match in the region: a regex without
/// capture groups records only the match bounds, which costs nothing extra.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryRegion {
    /// Every attempt records its captures.
    Fill,
    /// The attempt at the search start records its captures; later attempts
    /// run without a region, and only a successful one runs again to record
    /// its captures (`can_use_two_pass_capture_fill` in regexec).
    FillOnMatch,
}

impl EntryRegion {
    fn of(reg: &RegexType, option: OnigOptionType, msa: &MatchArg) -> Self {
        if reg.num_mem > 0
            && !reg.needs_capture_tracking
            && reg.extp.as_ref().is_none_or(|ext| ext.callout_num == 0)
            && msa.time_limit == 0
            && !opton_find_longest(option | reg.options)
            && two_pass_capture_fill_pays(reg)
        {
            EntryRegion::FillOnMatch
        } else {
            EntryRegion::Fill
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn attempt_entry_match(
    entry: &mut RegSetEntry,
    text: &[u8],
    end: usize,
    at: usize,
    start: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> i32 {
    onig_match_with_msa_start(&entry.reg, text, end, at, start, option, msa)
}

/// Whether the attempt of `entry` at `position` of a search that began at
/// `search_start` fails at its first instruction, so the position loop can
/// leave it out (`skips`: `may_skip_first_op_failures`). Like that failed
/// attempt, this resets `msa` for the attempt and counts one retry; the
/// region stays as it was.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn first_op_rejects(
    entry: &RegSetEntry,
    skips: bool,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> bool {
    let Some(test) = entry.first_op.filter(|_| skips) else {
        return false;
    };
    if !first_op_fails(test, &entry.reg, str_data, end, position, option) {
        return false;
    }
    msa.reset_for_match(&entry.reg, option, search_start);
    msa.retry_limit_in_search_counter += 1;
    #[cfg(test)]
    FIRST_OP_REJECTS.with(|rejects| rejects.set(rejects.get() + 1));
    true
}

#[cfg(test)]
thread_local! {
    /// Attempts `first_op_rejects` left out, for tests of its gates.
    static FIRST_OP_REJECTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Bytes the table scans read on this thread (skipped ones included),
    /// for tests that bound the work of a search.
    pub(crate) static TABLE_SCAN_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// One `match_at` of a fallback entry at `position` of a search that began
/// at `search_start` (C: `REGSET_MATCH_AND_RETURN_CHECK`).
#[allow(clippy::too_many_arguments)]
fn attempt_fallback_entry(
    entry: &mut RegSetEntry,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    option: OnigOptionType,
    fill: EntryRegion,
    msa: &mut MatchArg,
) -> i32 {
    if fill == EntryRegion::Fill || position == search_start {
        msa.region = entry.region.take();
        let result = attempt_entry_match(entry, str_data, end, position, search_start, option, msa);
        entry.region = msa.region.take();
        return result;
    }
    msa.region = None;
    let result = attempt_entry_match(entry, str_data, end, position, search_start, option, msa);
    if result < 0 {
        return result;
    }
    // The capture pass takes the path the first pass took; like the second
    // pass of a two-pass search it must not consume the retry budgets.
    let limits = (
        msa.retry_limit_in_match,
        msa.retry_limit_in_search,
        msa.retry_limit_in_search_counter,
    );
    msa.retry_limit_in_match = 0;
    msa.retry_limit_in_search = 0;
    msa.region = entry.region.take();
    let captured = attempt_entry_match(entry, str_data, end, position, search_start, option, msa);
    entry.region = msa.region.take();
    (
        msa.retry_limit_in_match,
        msa.retry_limit_in_search,
        msa.retry_limit_in_search_counter,
    ) = limits;
    if captured >= 0 {
        return captured;
    }
    // Keep the result exact should the passes ever disagree.
    msa.region = entry.region.take();
    let result = attempt_entry_match(entry, str_data, end, position, search_start, option, msa);
    entry.region = msa.region.take();
    result
}

/// Positions one regex may attempt in C's `regset_search_body_position_lead`
/// (its `SearchRange`, without `SRS_DEAD`, which is `None` below).
#[derive(Clone, Copy)]
enum EntrySearchRange {
    /// `SRS_ALL_RANGE`: every position. A search from any start up to
    /// `until` would find the same optimizer hit.
    AllRange { until: usize },
    /// `SRS_LOW_HIGH`: positions from `low` on; a position at or past `high`
    /// runs the optimizer again from there.
    LowHigh {
        low: usize,
        high: usize,
        sch_range: usize,
    },
}

/// C's `sr[i]` initialization in `regset_search_body_position_lead`. `None`
/// is `SRS_DEAD`: the optimizer finds nothing from `start`, so the regex
/// attempts no position at all.
fn entry_search_range(
    reg: &RegexType,
    subject_utf8: bool,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
) -> Option<EntrySearchRange> {
    if reg.optimize == OptimizeType::None || map_search_bypassed_for(reg, subject_utf8) {
        return Some(EntrySearchRange::AllRange { until: usize::MAX });
    }
    if reg.dist_max != INFINITE_LEN {
        let sch_range = if end - range > reg.dist_max as usize {
            range + reg.dist_max as usize
        } else {
            end
        };
        let (low, high) = forward_search(reg, str_data, end, start, sch_range)?;
        Some(EntrySearchRange::LowHigh {
            low,
            high,
            sch_range,
        })
    } else {
        let (_, until) = forward_search(reg, str_data, end, start, end)?;
        Some(EntrySearchRange::AllRange { until })
    }
}

/// A gated table entry's search range (`table_gate_admits`) as last
/// searched from `from`.
#[derive(Clone, Copy)]
struct EntryGate {
    generation: u32,
    from: usize,
    range: Option<EntrySearchRange>,
}

impl EntryGate {
    const STALE: EntryGate = EntryGate {
        generation: 0,
        from: 0,
        range: None,
    };

    /// Whether a search from `start` would find the same range: the
    /// optimizer's next hit from `from` is also the next one from `start`.
    #[inline]
    fn serves(&self, generation: u32, start: usize) -> bool {
        self.generation == generation
            && self.from <= start
            && start
                <= match self.range {
                    None => usize::MAX,
                    Some(EntrySearchRange::AllRange { until }) => until,
                    Some(EntrySearchRange::LowHigh { high, .. }) => high,
                }
    }
}

/// The event, if any, of an attempt of fallback entry `index` at `position`
/// that returned `result`.
fn fallback_attempt_decision(
    result: i32,
    index: usize,
    position: usize,
    msa: &MatchArg,
) -> Option<RegSetDecision> {
    if result >= 0 {
        return Some(RegSetDecision::Match(RegSetWinner {
            index: index as i32,
            position: position as i32,
            match_len: result,
        }));
    }
    let code = if result != ONIG_MISMATCH {
        result
    } else if msa.retry_limit_in_search != 0
        && msa.retry_limit_in_search_counter >= msa.retry_limit_in_search
    {
        ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER
    } else {
        return None;
    };
    Some(RegSetDecision::Error(RegSetError {
        code,
        index: index as i32,
        position: position as i32,
    }))
}

/// `search_fallback_entry` for the single position `start`, in the cheaper
/// order: the attempt first, and the optimizer only for an attempt that
/// matched or failed with an error. An attempt that finds nothing decides
/// nothing whether or not C would have made it, and without callouts an
/// attempt leaves no trace; an entry with callouts takes the general path,
/// which asks the optimizer first as C does.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn attempt_fallback_entry_at_start(
    entry: &mut RegSetEntry,
    subject_utf8: bool,
    index: usize,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> Option<RegSetDecision> {
    // Only an entry with an unbounded optimizer has a start filter, and its
    // search attempts every position the optimizer lets through.
    if msa.retry_limit_in_search == 0
        && start < end
        && entry
            .start_filter
            .as_deref()
            .is_some_and(|filter| filter[str_data[start] as usize] == 0)
        && (subject_utf8 || start_map_may_skip(&entry.reg, str_data, start))
    {
        return None;
    }
    if required_literals(entry, option, msa)
        .is_some_and(|required| required.find(str_data, start, end).is_none())
    {
        return None;
    }
    let fill = EntryRegion::of(&entry.reg, option, msa);
    msa.retry_limit_in_search_counter = 0;
    let skips = may_skip_first_op_failures(msa, option);
    if first_op_rejects(entry, skips, str_data, end, start, start, option, msa) {
        return None;
    }
    let result = attempt_fallback_entry(entry, str_data, end, start, start, option, fill, msa);
    let decision = fallback_attempt_decision(result, index, start, msa)?;
    let admitted = match entry_search_range(&entry.reg, subject_utf8, str_data, end, start, range) {
        None => false,
        Some(EntrySearchRange::AllRange { .. }) => true,
        Some(EntrySearchRange::LowHigh { low, .. }) => start >= low,
    };
    if admitted {
        Some(decision)
    } else {
        // C never made this attempt: leave no match behind.
        if let Some(region) = entry.region.as_mut() {
            region.clear();
        }
        None
    }
}

/// The first match or error of fallback entry `index` in a position-lead
/// search from `start` whose match start range is `range`, looking at the
/// positions up to `stop` (`stop <= range`).
///
/// This is C's `regset_search_body_position_lead` for that one regex: the
/// regex attempts exactly the positions its optimizer admits (`sr[i]`), an
/// `ANYCHAR_INF` regex only the first position and those after a newline,
/// and its search retry budget accumulates over its own attempts (C:
/// `msas[i]`). The regexes of a set do not influence each other's
/// attempts, so the set's result is the earliest of these per-regex results,
/// the lower index first on a tie. `onig_search` would attempt other
/// positions (its per-regex anchors and any-char-star skipping), which shows
/// when an attempt stops at a retry limit.
///
/// The Rust-only differences leave out attempts that cannot match: positions
/// whose byte cannot start a match (see `fallback_start_filter`), the rest of
/// a failed leading run (`after_failed_run`), and the positions after the
/// last occurrence of the entry's required literals (`required_literals`).
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn search_fallback_entry(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    stop: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> Option<RegSetDecision> {
    let enc = set.enc;
    let anychar_inf = set.anychar_inf;
    let subject_utf8 = set.subject_utf8;
    let entry = &mut set.entries[index];
    if stop == start
        && entry
            .reg
            .extp
            .as_ref()
            .is_none_or(|ext| ext.callout_num == 0)
    {
        return attempt_fallback_entry_at_start(
            entry,
            subject_utf8,
            index,
            str_data,
            end,
            start,
            range,
            option,
            msa,
        );
    }
    // An attempt needs an occurrence of a required literal at or after its
    // start: the first one from `start` admits the positions up to it, and
    // a position past it looks for the next one. This runs before the
    // optimizer's search: where the literal is the optimizer's own string,
    // `memchr`/`memmem` rule out a subject without it sooner.
    let mut required_hit = match required_literals(entry, option, msa) {
        Some(required) => Some(required.find(str_data, start, end)?),
        None => None,
    };
    let mut search_range =
        entry_search_range(&entry.reg, subject_utf8, str_data, end, start, range)?;
    // C updates `prev_is_newline` only while some regex of the set has
    // `ANCR_ANYCHAR_INF`, and the first position counts as after a newline.
    let after_newline_only = anychar_inf && (entry.reg.anchor & ANCR_ANYCHAR_INF) != 0;
    let fill = EntryRegion::of(&entry.reg, option, msa);
    // Skipping an attempt is unobservable except through the search retry
    // budget, which counts every failed attempt.
    let has_start_filter = entry.start_filter.is_some() && msa.retry_limit_in_search == 0;
    let first_op_skips = may_skip_first_op_failures(msa, option);
    msa.retry_limit_in_search_counter = 0;
    // Rust-only (ADR-008): a match starts in the class run before an
    // occurrence of the literal that follows the expression's zero-width
    // and class repetition part (`LiteralPrefix`). No limit may observe the
    // starts left out: their zero-width checks backtrack without a known
    // bound.
    let literal_prefix = entry
        .reg
        .literal_prefix
        .as_deref()
        .is_some_and(|prefix| prefix.skippable(msa.retry_limit_in_match))
        && {
            msa.retry_limit_in_search == 0
                && msa.match_stack_limit == 0
                && msa.time_limit == 0
                && !opton_find_longest(option | entry.reg.options)
        };
    let mut prefix_window: Option<(usize, usize)> = None;

    let mut s = start;
    loop {
        if s > stop {
            return None;
        }
        if required_hit.is_some_and(|hit| s > hit) {
            required_hit = Some(next_required_literal(&entry.reg, str_data, s, end)?);
        }
        if let Some(prefix) = entry
            .reg
            .literal_prefix
            .as_deref()
            .filter(|_| literal_prefix)
        {
            if prefix_window.is_none_or(|(_, k)| s > k) {
                prefix_window = Some(prefix.window(enc, str_data, s, stop, end)?);
            }
            if let Some((first, _)) = prefix_window.filter(|&(first, _)| s < first) {
                s = first;
                continue;
            }
        }
        let admitted = match search_range {
            EntrySearchRange::LowHigh {
                low,
                high,
                sch_range,
            } => {
                // C steps over the positions before `low` one at a time
                // without attempting them.
                if s < low {
                    s = low;
                    continue;
                }
                if s >= high {
                    let (low, high) = forward_search(&entry.reg, str_data, end, s, sch_range)?;
                    search_range = EntrySearchRange::LowHigh {
                        low,
                        high,
                        sch_range,
                    };
                    if s < low {
                        s = low;
                        continue;
                    }
                }
                true
            }
            EntrySearchRange::AllRange { .. } => {
                !has_start_filter
                    || s >= end
                    || entry
                        .start_filter
                        .as_deref()
                        .is_none_or(|filter| filter[str_data[s] as usize] != 0)
                    || (!subject_utf8 && !start_map_may_skip(&entry.reg, str_data, s))
            }
        } && !(after_newline_only && s > start && str_data[s - 1] != b'\n');

        // An attempt that fails at its first instruction is left as failed.
        if admitted
            && !first_op_rejects(entry, first_op_skips, str_data, end, s, start, option, msa)
        {
            let result = attempt_fallback_entry(entry, str_data, end, s, start, option, fill, msa);
            if let Some(decision) = fallback_attempt_decision(result, index, s, msa) {
                return Some(decision);
            }
        }
        if s >= stop || s >= end {
            return None;
        }
        s = if admitted
            && !after_newline_only
            && matches!(search_range, EntrySearchRange::AllRange { .. })
            && entry.reg.leading_run.is_some()
        {
            // The failed attempt read the whole subject, even when a
            // competing regex bounds match starts at `stop`. Starts inside
            // its run cannot succeed; crossing `stop` finishes this entry.
            crate::regexec::after_failed_run(
                &entry.reg,
                opton_find_longest(option | entry.reg.options),
                msa,
                str_data,
                end,
                end,
                end,
                s,
            )
        } else {
            (s + enclen(enc, str_data, s)).min(end)
        };
    }
}

#[inline]
fn regset_decision_result(set: &mut OnigRegSet, decision: Option<RegSetDecision>) -> (i32, i32) {
    match decision {
        Some(RegSetDecision::Match(winner)) => {
            set.last_match_len = winner.match_len;
            (winner.index, winner.position)
        }
        Some(RegSetDecision::Error(error)) => {
            set.last_match_len = ONIG_MISMATCH;
            (error.code, 0)
        }
        None => {
            set.last_match_len = ONIG_MISMATCH;
            (ONIG_MISMATCH, 0)
        }
    }
}

/// Position-lead search: iterate positions, try each regex at each position.
#[inline]
fn regset_search_body_position_lead_table(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
) -> Option<RegSetDecision> {
    if table_scan_gates_eagerly(set) {
        regset_table_scan::<true>(
            set,
            str_data,
            end,
            start,
            range,
            start,
            range,
            option,
            skip_region_for_nomem,
        )
        .0
    } else {
        regset_table_scan::<false>(
            set,
            str_data,
            end,
            start,
            range,
            start,
            range,
            option,
            skip_region_for_nomem,
        )
        .0
    }
}

/// Whether the table scan has to ask gated entries' optimizers before their
/// attempts rather than after an event: a gated entry has callouts, or a
/// search retry budget is set.
#[inline]
fn table_scan_gates_eagerly(set: &OnigRegSet) -> bool {
    set.has_gated
        && (set.has_gated_callouts
            || set
                .scratch_limits
                .is_some_and(|limits| limits.retry_limit_in_search != 0))
}

/// Whether gated table entry `index` may attempt position `s` of the
/// position-lead search from `start` with match start range `range`: C's
/// `sr[i]` check in `regset_search_body_position_lead`, evaluated lazily
/// and kept for the rest of the search (and for later searches of the same
/// subject that it still serves).
fn table_gate_admits(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    s: usize,
) -> bool {
    let reg = &*set.entries[index].reg;
    let gate = &mut set.gates[index];
    if !gate.serves(set.gate_generation, start) {
        *gate = EntryGate {
            generation: set.gate_generation,
            from: start,
            range: entry_search_range(reg, set.subject_utf8, str_data, end, start, range),
        };
    }
    entry_range_admits(reg, str_data, end, &mut gate.range, &mut gate.from, s)
}

/// Whether a regex whose search range (C: `sr[i]`) is `search_range` attempts
/// position `s`, the positions being visited in increasing order. Moves the
/// range on as C does: a position at or past `high` searches the optimizer
/// again from there, and a failed search leaves nothing (`SRS_DEAD`).
#[inline]
fn entry_range_admits(
    reg: &RegexType,
    str_data: &[u8],
    end: usize,
    search_range: &mut Option<EntrySearchRange>,
    searched_from: &mut usize,
    s: usize,
) -> bool {
    match *search_range {
        None => false,
        Some(EntrySearchRange::AllRange { .. }) => true,
        Some(EntrySearchRange::LowHigh {
            low,
            high,
            sch_range,
        }) => {
            if s < low {
                return false;
            }
            if s < high {
                return true;
            }
            *searched_from = s;
            match forward_search(reg, str_data, end, s, sch_range) {
                Some((low, high)) => {
                    *search_range = Some(EntrySearchRange::LowHigh {
                        low,
                        high,
                        sch_range,
                    });
                    s >= low
                }
                None => {
                    *search_range = None;
                    false
                }
            }
        }
    }
}

/// The table part of a position-lead search from `start`, over the
/// positions from `from` up to `to`: the first match or error among the
/// table entries there, and the position after the last one looked at. A
/// search continues where the previous scan stopped by passing that
/// position as the next `from`.
/// `EAGER_GATES` asks a gated entry's optimizer before every attempt, which
/// C's order requires where an attempt can be observed: through a callout,
/// or through the search retry budget it consumes (`table_scan_gates_eagerly`).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn regset_table_scan<const EAGER_GATES: bool>(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    search_range: usize,
    from: usize,
    to: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
) -> (Option<RegSetDecision>, usize) {
    // All entries are optimizer-backed fallbacks. They are searched below in
    // one pass each; walking every byte here would add an otherwise empty
    // O(n) table pass to every cached no-match lookup.
    if set.table_entry_count == 0 {
        return (None, to.saturating_add(1));
    }

    // rmatch_pos, regex_index
    let enc = set.enc;
    let mut s = from;
    let range = to;

    // Reuse a single MatchArg across patterns and across regset calls.
    let mut msa = if let Some(msa) = set.scratch_msa.take() {
        msa
    } else {
        let first_reg = &*set.entries[0].reg;
        MatchArg::new(first_reg, option, None, start)
    };
    // Search retry budgets start fresh for this public API call and then
    // accumulate independently per regex as position-lead advances. The
    // default global limit is zero, so avoid touching the per-entry scratch
    // vector on the table-only fast path where it cannot be observed.
    let track_search_retry_limit = msa.retry_limit_in_search != 0;
    if track_search_retry_limit && from == start {
        if set.scratch_table_retry_counters.len() != set.entries.len() {
            set.scratch_table_retry_counters
                .resize(set.entries.len(), 0);
        }
        set.scratch_table_retry_counters.fill(0);
    }

    let prev_is_newline_check = set.anychar_inf;
    let first_op_skips = may_skip_first_op_failures(&msa, option);
    let mut result = None;
    // Look-behind entries need a look only after a non-ASCII byte: before
    // the first one from `look_behind_from` on, every start lines up.
    let look_behind_from = if set.look_behind_reach == 0 || set.subject_utf8 {
        usize::MAX
    } else {
        let from = start.saturating_sub(set.look_behind_reach as usize);
        let to = range.min(end);
        if from >= to || str_data[from..to].is_ascii() {
            usize::MAX
        } else {
            from + str_data[from..to]
                .iter()
                .position(|&b| b >= 0x80)
                .unwrap_or(0)
        }
    };
    // Whether the needle must stop near non-ASCII bytes for look-behind
    // entries the table routes by Rust-only maps.
    let needle_stops = look_behind_from < range
        && !set.look_behind_entries.is_empty()
        && !matches!(set.skip_needle, SkipNeedle::None);
    let mut next_non_ascii: Option<usize> = None;
    let mut visit_until = 0;
    #[cfg(test)]
    let mut scanned_to = s;

    'search: loop {
        if s > range {
            break;
        }

        // SIMD-accelerated position skip: jump to next byte that could match.
        // The range position itself must still be attempted (matching
        // Oniguruma's do-while loop), so a failed skip lands on `range`.
        // With look-behind entries it stops at the next non-ASCII byte and
        // skips nothing within `look_behind_reach` bytes after one, where
        // their characters may not line up.
        if s < range && s >= visit_until {
            let stop = if !needle_stops {
                range
            } else {
                if next_non_ascii.is_none_or(|at| at < s) {
                    next_non_ascii = Some(
                        str_data[s..range]
                            .iter()
                            .position(|&b| b >= 0x80)
                            .map_or(range, |off| s + off),
                    );
                }
                next_non_ascii.unwrap_or(range)
            };
            s = match set.skip_needle {
                SkipNeedle::None => s,
                SkipNeedle::One(b) => {
                    memchr::memchr(b, &str_data[s..stop]).map_or(stop, |off| s + off)
                }
                SkipNeedle::Two(b1, b2) => {
                    memchr::memchr2(b1, b2, &str_data[s..stop]).map_or(stop, |off| s + off)
                }
                SkipNeedle::Three(b1, b2, b3) => {
                    memchr::memchr3(b1, b2, b3, &str_data[s..stop]).map_or(stop, |off| s + off)
                }
            };
        }
        if needle_stops && s < end && str_data[s] >= 0x80 {
            visit_until =
                visit_until.max(s + enclen(enc, str_data, s) + set.look_behind_reach as usize);
        }

        #[cfg(test)]
        {
            set.table_positions_scanned += 1;
            TABLE_SCAN_BYTES.with(|bytes| {
                bytes.set(bytes.get() + (s + 1).saturating_sub(scanned_to) as u64);
            });
            scanned_to = s + 1;
        }

        // Oniguruma starts with prev_is_newline = 1: the first attempted
        // position may always match, whatever precedes it in the subject.
        let prev_is_newline = if prev_is_newline_check && s > start {
            str_data[s - 1] == b'\n'
        } else {
            true // default: allow matching
        };

        let remaining = end - s;

        // At the logical end there is no first byte to dispatch on. Try all
        // table entries there; the threshold check cheaply rejects non-empty
        // ones. A fallback entry's own search attempts the end where its
        // optimizer admits it.
        let at_end = s == end;
        // Where a leading look-behind's characters do not line up, its
        // entries are attempted whatever the byte, in index order.
        let misaligned = s > look_behind_from
            && !ascii_before(str_data, s, set.look_behind_reach)
            && !characters_line_up(enc, set.look_behind_reach, str_data, s);
        let unaligned = (!at_end && misaligned && !set.look_behind_entries.is_empty()).then(|| {
            merge_entry_lists(
                &set.first_byte_candidates[str_data[s] as usize],
                &set.look_behind_entries,
            )
        });
        let candidate_count = if at_end {
            set.table_entries.len()
        } else if let Some(merged) = &unaligned {
            merged.len()
        } else {
            set.first_byte_candidates[str_data[s] as usize].len()
        };

        for candidate_at in 0..candidate_count {
            let i = if at_end {
                set.table_entries[candidate_at] as usize
            } else if let Some(merged) = &unaligned {
                merged[candidate_at] as usize
            } else {
                set.first_byte_candidates[str_data[s] as usize][candidate_at] as usize
            };

            // ANCR_ANYCHAR_INF optimization: skip if previous char is not newline
            if (set.entries[i].reg.anchor & ANCR_ANYCHAR_INF) != 0 && !prev_is_newline {
                continue;
            }

            // Pre-filter: remaining text too short for this pattern. Where a
            // leading look-behind's characters do not line up, the match
            // can read bytes before its start.
            if set.entries[i].reg.threshold_len > 0
                && remaining < set.entries[i].reg.threshold_len as usize
                && !(misaligned && set.entries[i].reg.look_behind_reach > 0)
            {
                continue;
            }
            // A gated entry is attempted where C's optimizer admits the
            // position. An attempt that finds nothing decides nothing either
            // way, so the optimizer is asked only after an event -- unless
            // the attempt itself can be observed (`EAGER_GATES`).
            if EAGER_GATES
                && set.entries[i].gated
                && !table_gate_admits(set, i, str_data, end, start, search_range, s)
            {
                continue;
            }
            if track_search_retry_limit {
                msa.retry_limit_in_search_counter = set.scratch_table_retry_counters[i];
            }
            // An attempt that fails at its first instruction has no event.
            if first_op_rejects(
                &set.entries[i],
                first_op_skips,
                str_data,
                end,
                s,
                start,
                option,
                &mut msa,
            ) {
                continue;
            }
            let r = if skip_region_for_nomem && region_is_redundant(&set.entries[i].reg) {
                // No capture groups and no `\K`: the caller can rebuild the
                // whole match from the attempt position and the match length,
                // so avoid region take/clear/restore on this hot path.
                msa.region = None;
                attempt_entry_match(
                    &mut set.entries[i],
                    str_data,
                    end,
                    s,
                    start,
                    option,
                    &mut msa,
                )
            } else {
                // Swap region into msa for this match, then swap back.
                msa.region = set.entries[i].region.take();
                let r = attempt_entry_match(
                    &mut set.entries[i],
                    str_data,
                    end,
                    s,
                    start,
                    option,
                    &mut msa,
                );
                set.entries[i].region = msa.region.take();
                r
            };
            if track_search_retry_limit {
                set.scratch_table_retry_counters[i] = msa.retry_limit_in_search_counter;
            }

            if !EAGER_GATES
                && r != ONIG_MISMATCH
                && set.entries[i].gated
                && !table_gate_admits(set, i, str_data, end, start, search_range, s)
            {
                // C never makes this attempt.
                if r >= 0 {
                    clear_regset_entry_region(set, i as i32);
                }
                continue;
            }
            if r >= 0 {
                result = Some(RegSetDecision::Match(RegSetWinner {
                    index: i as i32,
                    position: s as i32,
                    match_len: r,
                }));
                break 'search;
            }
            if r != ONIG_MISMATCH {
                result = Some(RegSetDecision::Error(RegSetError {
                    code: r,
                    index: i as i32,
                    position: s as i32,
                }));
                break 'search;
            }
            if track_search_retry_limit
                && set.scratch_table_retry_counters[i] > msa.retry_limit_in_search
            {
                result = Some(RegSetDecision::Error(RegSetError {
                    code: ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER,
                    index: i as i32,
                    position: s as i32,
                }));
                break 'search;
            }
        }

        if s >= range {
            s = s.saturating_add(enclen(enc, str_data, s));
            break;
        }
        s += enclen(enc, str_data, s);
    }

    set.scratch_msa = Some(msa);

    (result, s)
}

/// The scratch `MatchArg`, or a new one.
#[inline(never)]
fn take_scratch_msa(set: &mut OnigRegSet, option: OnigOptionType, start: usize) -> MatchArg {
    set.scratch_msa
        .take()
        .unwrap_or_else(|| MatchArg::new(&set.entries[0].reg, option, None, start))
}

/// Bring the scratch `MatchArg` in line with the process-global limits and
/// return them.
///
/// MatchArg captures process-global limits when created. The Acquire
/// revision read synchronizes with a setter's Release revision bump before
/// this changed path reloads the tuple, while unchanged table-only scanner
/// calls avoid all four limit atomics. A non-zero time limit additionally
/// requires a fresh search clock for each public search.
#[inline]
fn refresh_scratch_limits(set: &mut OnigRegSet) -> FallbackMemoLimits {
    let limit_revision = onig_get_global_limit_revision();
    if set.scratch_limits_revision != Some(limit_revision) {
        set.scratch_limits = Some(FallbackMemoLimits::current());
        set.scratch_limits_revision = Some(limit_revision);
        set.scratch_msa = None;
    }
    let limits = set
        .scratch_limits
        .expect("global limit revision always initializes cached limits");
    if limits.time_limit != 0 {
        set.scratch_msa = None;
    }
    limits
}

/// The first match or error of one table entry in a position-lead search
/// from `start`, over the positions up to `stop`: the attempts
/// `regset_search_body_position_lead_table` makes for that entry.
#[allow(clippy::too_many_arguments)]
fn search_table_entry(
    set: &mut OnigRegSet,
    index: usize,
    start_bytes: &[u64; 4],
    str_data: &[u8],
    end: usize,
    start: usize,
    stop: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> Option<RegSetDecision> {
    let enc = set.enc;
    let prev_is_newline_check = set.anychar_inf;
    let subject_utf8 = set.subject_utf8;
    let entry = &mut set.entries[index];
    let after_newline_only = (entry.reg.anchor & ANCR_ANYCHAR_INF) != 0;
    let threshold_len = entry.reg.threshold_len.max(0) as usize;
    // The search range is the whole subject (`onig_regset_entry_search`).
    // As in the table scan, the optimizer is asked only once an attempt has
    // an event, unless the attempts themselves are observable.
    let gate_first = entry.gated && (msa.retry_limit_in_search != 0 || entry.has_callouts);
    let mut gate_from = start;
    let mut gate: Option<Option<EntrySearchRange>> = None;
    msa.retry_limit_in_search_counter = 0;
    let mut s = start;
    loop {
        // A leading look-behind whose characters do not line up can make
        // the match read bytes before `s` (`start_map_may_skip`).
        let unaligned = !subject_utf8 && s < end && !start_map_may_skip(&entry.reg, str_data, s);
        let mut admitted = (s == end
            || (start_bytes[str_data[s] as usize / 64] >> (str_data[s] % 64)) & 1 != 0
            || unaligned)
            && !(after_newline_only
                && prev_is_newline_check
                && s > start
                && str_data[s - 1] != b'\n')
            && (end - s >= threshold_len || unaligned);
        if admitted && gate_first {
            let range = gate.get_or_insert_with(|| {
                entry_search_range(&entry.reg, subject_utf8, str_data, end, start, end)
            });
            admitted = entry_range_admits(&entry.reg, str_data, end, range, &mut gate_from, s);
        }
        if admitted {
            let result = attempt_fallback_entry(
                entry,
                str_data,
                end,
                s,
                start,
                option,
                EntryRegion::Fill,
                msa,
            );
            let decision = if result >= 0 {
                Some(RegSetDecision::Match(RegSetWinner {
                    index: index as i32,
                    position: s as i32,
                    match_len: result,
                }))
            } else if result != ONIG_MISMATCH {
                Some(RegSetDecision::Error(RegSetError {
                    code: result,
                    index: index as i32,
                    position: s as i32,
                }))
            } else if msa.retry_limit_in_search != 0
                && msa.retry_limit_in_search_counter > msa.retry_limit_in_search
            {
                Some(RegSetDecision::Error(RegSetError {
                    code: ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER,
                    index: index as i32,
                    position: s as i32,
                }))
            } else {
                None
            };
            if let Some(decision) = decision {
                let admits = !entry.gated || gate_first || {
                    let range = gate.get_or_insert_with(|| {
                        entry_search_range(&entry.reg, subject_utf8, str_data, end, start, end)
                    });
                    entry_range_admits(&entry.reg, str_data, end, range, &mut gate_from, s)
                };
                if admits {
                    return Some(decision);
                }
                // C never makes this attempt.
                if let Some(region) = entry.region.as_mut() {
                    region.clear();
                }
            }
        }
        if s >= stop || s >= end {
            return None;
        }
        s = (s + enclen(enc, str_data, s)).min(end);
    }
}

/// The first event of one regex in a position-lead search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegSetEntryEvent {
    /// No attempt up to the stop position matched or failed with an error.
    None,
    /// A match whose attempt began at `position`; the entry's region holds it.
    Match { position: usize },
    /// An attempt at `position` stopped with the error `code`.
    Error { position: usize, code: i32 },
}

/// One regex of the set on its own: the first match or error that
/// `onig_regset_search_fast(.., PositionLead, ..)` would meet for entry
/// `index` from `start`, looking at the start positions up to `stop`.
///
/// A position-lead search attempts each regex at its own positions, so the
/// set's result is the earliest of these events, the lower index first on a
/// tie; a caller that combines them that way gets exactly the set's result,
/// errors included. The entry's region receives a match. `subject_utf8`
/// says the subject is valid UTF-8, as a scanner's is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn onig_regset_entry_search(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    start: usize,
    stop: usize,
    option: OnigOptionType,
    subject_utf8: bool,
) -> RegSetEntryEvent {
    let end = end.min(str_data.len());
    if start > end {
        return RegSetEntryEvent::None;
    }
    let stop = stop.min(end);
    refresh_scratch_limits(set);
    set.subject_utf8 = subject_utf8;
    match regset_entry_decision(set, index, str_data, end, start, end, stop, option) {
        None => RegSetEntryEvent::None,
        Some(RegSetDecision::Match(winner)) => RegSetEntryEvent::Match {
            position: winner.position as usize,
        },
        Some(RegSetDecision::Error(error)) => RegSetEntryEvent::Error {
            position: error.position as usize,
            code: error.code,
        },
    }
}

/// `onig_regset_entry_search` as a decision, for a search whose match start
/// range is `range`, with the scratch limits refreshed and `subject_utf8`
/// set by the caller.
// The entry searches take the search's bounds as `onig_regset_entry_search`
// does.
#[allow(clippy::too_many_arguments)]
fn regset_entry_decision(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    stop: usize,
    option: OnigOptionType,
) -> Option<RegSetDecision> {
    let mut msa = take_scratch_msa(set, option, start);
    let decision = if set.entries[index].fallback {
        search_fallback_entry(
            set, index, str_data, end, start, range, stop, option, &mut msa,
        )
    } else {
        let start_bytes = table_start_bytes(set, index);
        search_table_entry(
            set,
            index,
            &start_bytes,
            str_data,
            end,
            start,
            stop,
            option,
            &mut msa,
        )
    };
    set.scratch_msa = Some(msa);
    decision
}

/// Rust-only (ADR-008): whether the DFA pre-filter decides this search.
///
/// The pre-filter leaves out attempts that cannot match, whose backtracks
/// and calls a limit would count; C makes them and may stop at the limit.
/// So it decides only a search under Ferroni's defaults (10,000,000 retries
/// per match, no search retry budget, stack, time or subexpression call
/// limit), and a search under a limit of the caller's own, or without any,
/// keeps C's attempts. It stays off for FIND_LONGEST as the other skips do,
/// and reads the subject as UTF-8 (scanner searches), from a character
/// boundary: the seeks read whole characters, and a search that starts
/// inside one (`find_next_match` takes any byte offset) attempts that
/// position as C does.
#[cfg(feature = "dfa-prefilter")]
#[inline]
fn prefilter_decides(
    set: &OnigRegSet,
    limits: FallbackMemoLimits,
    option: OnigOptionType,
    str_data: &[u8],
    start: usize,
) -> bool {
    set.subject_utf8 && starts_a_character(str_data, start) && prefilter_admits(set, limits, option)
}

/// Whether `start` is the end of the subject or the first byte of a
/// character (not a UTF-8 continuation byte).
#[cfg(feature = "dfa-prefilter")]
#[inline]
fn starts_a_character(str_data: &[u8], start: usize) -> bool {
    str_data.get(start).is_none_or(|&byte| byte & 0xC0 != 0x80)
}

/// `prefilter_decides` apart from the subject: the set has the automata and
/// the limits and options admit them.
#[cfg(feature = "dfa-prefilter")]
#[inline]
fn prefilter_admits(set: &OnigRegSet, limits: FallbackMemoLimits, option: OnigOptionType) -> bool {
    set.prefilter.is_some()
        && limits.retry_limit_in_match == DEFAULT_RETRY_LIMIT_IN_MATCH
        && limits.retry_limit_in_search == 0
        && limits.match_stack_limit == 0
        && limits.time_limit == 0
        && limits.subexp_call_limit_in_search == 0
        && !opton_find_longest(option)
}

/// Rust-only (ADR-008): whether the DFA pre-filter would decide a search of
/// a UTF-8 subject under `option` and the current limits. The scanner keeps
/// such a call on the RegSet route: its per-regex route has no pre-filter,
/// and a skipped attempt can reach the retry limit there, so the two routes
/// could answer an identical call differently.
pub(crate) fn onig_regset_prefilter_decides(
    set: &mut OnigRegSet,
    option: OnigOptionType,
    str_data: &[u8],
    start: usize,
) -> bool {
    #[cfg(feature = "dfa-prefilter")]
    {
        let limits = refresh_scratch_limits(set);
        starts_a_character(str_data, start) && prefilter_admits(set, limits, option)
    }
    #[cfg(not(feature = "dfa-prefilter"))]
    {
        let _ = (set, option, str_data, start);
        false
    }
}

/// Candidate positions in a row whose attempts all fail before the DFA
/// pre-filter gives the search up to the position-lead search. The
/// candidate scan reads from each candidate position again (at most
/// `CANDIDATE_SCAN_BYTES`), and a seek that matches at every position of a
/// long run (`\w+` for `(?<=\.)\w+`) would make a failing search cost a
/// multiple of its length; the budget keeps it at a constant plus the
/// search without the pre-filter. Over the captured replays the longest
/// such run is 21 (PHP; C++ 13, Java 10, SCSS and C 6), so the budget is
/// never reached there (see ADR-008).
#[cfg(feature = "dfa-prefilter")]
const PREFILTER_FAILED_POSITIONS: u32 = 64;

/// Rust-only (ADR-008): the position-lead search decided by the DFA
/// pre-filter, or `None` where the pre-filter gave it up
/// (`PREFILTER_FAILED_POSITIONS`) and the position-lead search takes it
/// from the start.
///
/// The entries the automata cover are attempted where the automata say a
/// seek matches, the others (`own`: their seek matches everywhere) as the
/// position-lead search attempts them, interleaved by position, and at a
/// position every entry in index order: the own entries ahead of every
/// covered one first, then the covered candidates the overlapping DFA
/// names there merged with the other own entries; the first event at the
/// position decides, and an own entry behind a candidate that matched is
/// never attempted. Where no seek matches at the position, the own entries
/// are searched over a window first (the fallback ones through
/// `search_fallback_entries`, with the memo and the growing windows of the
/// position-lead search, over the uncovered entries only; the table ones
/// position by position): an own event within reach bounds what the
/// covered entries can still decide, and bounded walks at the positions up
/// to it settle that. Only without one, the meta regex finds the next
/// position where a seek matches, past any stretch where none does (one
/// search per subject for a stable identity, `earliest_memo`), and the
/// own entries are searched over that stretch. A decision bounds
/// everything after it. Every attempt left out would fail: the seek
/// matches wherever its entry can. The result is therefore the one
/// `regset_search_body_position_lead` finds without the pre-filter, which
/// a search under a limit of the caller's own still takes
/// (`prefilter_decides`).
#[cfg(feature = "dfa-prefilter")]
#[allow(clippy::too_many_arguments)]
fn regset_search_body_prefilter(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    memo_enabled: bool,
    subject: Option<(FallbackMemoIdentity, usize)>,
) -> Option<Option<RegSetDecision>> {
    const FIRST_OWN_WINDOW: usize = 256;
    /// Positions up to an own event that bounded walks settle for the
    /// covered entries, instead of a meta regex search past it: the whole
    /// first window, so an own event inside it never costs a read past it.
    const OWN_EVENT_WALKS: usize = FIRST_OWN_WINDOW;
    let mut prefilter = set.prefilter.take()?;
    let mut decision: Option<RegSetDecision> = None;
    let mut failed_positions = 0u32;
    let has_own = !prefilter.own().is_empty();
    let has_own_fallback = set.prefilter_own_fallback;
    let has_own_table = prefilter
        .own()
        .iter()
        .any(|&index| !set.entries[index as usize].fallback);
    // The own entries ahead of every covered one, attempted before the
    // automata are asked.
    let covered_first = prefilter.covered_entries()[0];
    let own_ahead = prefilter
        .own()
        .iter()
        .take_while(|&&index| index < covered_first)
        .count();
    // How far from `start` the own fallback entries have been searched, and
    // the first position whose own table entries have not been attempted.
    let mut own_searched_to: Option<usize> = None;
    let mut own_table_to = start;

    let haystack = &str_data[..end];
    let mut msa = take_scratch_msa(set, option, start);
    let mut s = start;
    while s <= range {
        // Position `s` in index order: the own entries ahead of every
        // covered one, then the covered candidates merged with the rest.
        let mut decided_here = false;
        let mut seek_matches_here = false;
        if !has_own {
            seek_matches_here = prefilter
                .candidates_at(haystack, s, &mut |index, position| {
                    covered_entry_admits_at(
                        set,
                        index as usize,
                        str_data,
                        end,
                        start,
                        range,
                        position,
                    )
                })
                .is_some();
            if seek_matches_here {
                decided_here = attempt_candidates_at(
                    set,
                    &prefilter,
                    str_data,
                    end,
                    s,
                    start,
                    range,
                    option,
                    &mut msa,
                    &mut decision,
                );
            }
        } else {
            let own_fallback_done = own_searched_to.is_some_and(|searched| searched >= s);
            let own_table_done = own_table_to > s;
            decided_here = own_ahead > 0
                && attempt_merged_at(
                    set,
                    &prefilter.own()[..own_ahead],
                    &[],
                    own_fallback_done,
                    own_table_done,
                    str_data,
                    end,
                    s,
                    start,
                    range,
                    option,
                    &mut msa,
                    &mut decision,
                    memo_enabled,
                );
            if !decided_here {
                seek_matches_here = prefilter
                    .candidates_at(haystack, s, &mut |index, position| {
                        covered_entry_admits_at(
                            set,
                            index as usize,
                            str_data,
                            end,
                            start,
                            range,
                            position,
                        )
                    })
                    .is_some();
                decided_here = attempt_merged_at(
                    set,
                    &prefilter.own()[own_ahead..],
                    prefilter.candidates(),
                    own_fallback_done,
                    own_table_done,
                    str_data,
                    end,
                    s,
                    start,
                    range,
                    option,
                    &mut msa,
                    &mut decision,
                    memo_enabled,
                );
            }
        }
        if decided_here
            || decision
                .map(decision_position_and_index)
                .is_some_and(|(position, _)| position as usize <= s)
            || s >= end
        {
            break;
        }
        let next = s + enclen(set.enc, str_data, s);
        own_table_to = own_table_to.max(next);
        if seek_matches_here {
            // Every attempt at `s` failed where a seek matched. The
            // anchored scan is bounded, and where some seek matches at the
            // next character too the meta regex would only confirm that,
            // at the cost of finding where its match ends: the search goes
            // on one character later and asks the DFA there first.
            failed_positions += 1;
            if failed_positions > PREFILTER_FAILED_POSITIONS {
                // The position-lead search takes the search from the start:
                // leave no match of this one behind.
                if let Some(RegSetDecision::Match(found)) = decision {
                    clear_regset_entry_region(set, found.index);
                }
                set.scratch_msa = Some(msa);
                set.prefilter = Some(prefilter);
                return None;
            }
            s = next;
            continue;
        }
        if next > range {
            break;
        }
        // No seek matches at `s`. The window: the first FIRST_OWN_WINDOW
        // positions, then four times the positions looked at so far, so a
        // search that finds nothing reads its line in a few rounds.
        let window = FIRST_OWN_WINDOW.max((next - start).saturating_mul(4));
        let window_last = range.min(next.saturating_add(window - 1));
        // The own entries first, over the window: the earliest own event
        // bounds what the covered entries can still decide.
        if has_own_fallback && own_searched_to.is_none_or(|searched| searched < window_last) {
            let own_window = own_searched_to.map_or(FIRST_OWN_WINDOW, |searched| {
                (searched - start).saturating_mul(4)
            });
            let searched_to = range.min(start.saturating_add(own_window)).max(window_last);
            own_searched_to = Some(searched_to);
            set.scratch_msa = Some(msa);
            decision = search_fallback_entries(
                set,
                str_data,
                end,
                start,
                range,
                searched_to,
                option,
                memo_enabled,
                true,
                decision,
            );
            msa = take_scratch_msa(set, option, start);
        }
        if has_own_table && own_table_to <= window_last {
            own_table_to = attempt_own_table_entries_over(
                set,
                &prefilter,
                str_data,
                end,
                own_table_to,
                window_last + 1,
                start,
                range,
                option,
                &mut msa,
                &mut decision,
            );
        }
        let event = decision
            .map(decision_position_and_index)
            .map(|(position, _)| position as usize);
        // An own event within reach: the covered entries can only decide
        // at the positions up to it, which bounded walks, where some
        // entry's bytes dispatch, settle without a read past it.
        if let Some(event) = event {
            if event < next.saturating_add(OWN_EVENT_WALKS) {
                let dispatch = prefilter.any_dispatch();
                let mut q = next;
                while q <= event {
                    if decision
                        .map(decision_position_and_index)
                        .is_some_and(|(position, _)| (position as usize) < q)
                    {
                        break;
                    }
                    let admitted = str_data
                        .get(q)
                        .is_none_or(|&byte| (dispatch[byte as usize / 64] >> (byte % 64)) & 1 == 1);
                    if admitted
                        && prefilter
                            .candidates_at(haystack, q, &mut |index, position| {
                                covered_entry_admits_at(
                                    set,
                                    index as usize,
                                    str_data,
                                    end,
                                    start,
                                    range,
                                    position,
                                )
                            })
                            .is_some()
                    {
                        attempt_candidates_at(
                            set,
                            &prefilter,
                            str_data,
                            end,
                            q,
                            start,
                            range,
                            option,
                            &mut msa,
                            &mut decision,
                        );
                    }
                    q += enclen(set.enc, str_data, q);
                }
                break;
            }
        }
        // The covered entries: with a stable identity, the meta regex once
        // per subject; otherwise within the window, up to the own event,
        // reading no further (`candidate_in`). The own entries are searched
        // over the stretch up to the find.
        let (at, covered_to) = if subject.is_some() {
            let at = match prefilter.earliest_memo(haystack, next, subject) {
                Some(at) if at <= range => Some(at),
                _ => None,
            };
            (at, range)
        } else {
            let mut until = window_last + 1;
            if let Some(event) = event {
                until = until.min(event + 1);
            }
            while until < end && (str_data[until] & 0xC0) == 0x80 {
                until += 1;
            }
            let at = prefilter.candidate_in(haystack, next, until, &mut |index, position| {
                covered_entry_admits_at(set, index as usize, str_data, end, start, range, position)
            });
            (at, until - 1)
        };
        let stretch_to = at.unwrap_or(covered_to);
        if has_own_fallback && own_searched_to.is_none_or(|searched| searched < stretch_to) {
            let own_window = own_searched_to.map_or(FIRST_OWN_WINDOW, |searched| {
                (searched - start).saturating_mul(4)
            });
            let searched_to = range.min(start.saturating_add(own_window)).max(stretch_to);
            own_searched_to = Some(searched_to);
            set.scratch_msa = Some(msa);
            decision = search_fallback_entries(
                set,
                str_data,
                end,
                start,
                range,
                searched_to,
                option,
                memo_enabled,
                true,
                decision,
            );
            msa = take_scratch_msa(set, option, start);
        }
        if has_own_table {
            // Up to `at`, where the next round attempts every own entry.
            let until = at.unwrap_or(covered_to + 1);
            if own_table_to < until {
                own_table_to = attempt_own_table_entries_over(
                    set,
                    &prefilter,
                    str_data,
                    end,
                    own_table_to,
                    until,
                    start,
                    range,
                    option,
                    &mut msa,
                    &mut decision,
                );
            }
        }
        let Some(at) = at else {
            // Nothing up to `covered_to`: the next window, unless the range
            // ends there or an own event decides within it.
            if covered_to >= range
                || decision
                    .map(decision_position_and_index)
                    .is_some_and(|(position, _)| (position as usize) <= covered_to)
            {
                break;
            }
            s = covered_to + 1;
            continue;
        };
        // No match starts inside a character, where a seek with a raw byte
        // or an ASCII half boundary behind a non-ASCII byte can: the search
        // goes on at the next character.
        if at < end && (str_data[at] & 0xC0) == 0x80 {
            s = at + 1;
            while s < end && (str_data[s] & 0xC0) == 0x80 {
                s += 1;
            }
            continue;
        }
        if decision
            .map(decision_position_and_index)
            .is_some_and(|(position, _)| at > position as usize)
        {
            break;
        }
        s = at;
    }
    set.scratch_msa = Some(msa);
    set.prefilter = Some(prefilter);
    Some(decision)
}

/// Whether the fallback memo of own entry `index` rules an attempt at `s`
/// out, as `search_fallback_entries` reads it: a search from an earlier
/// start found no match from `s` on or its first match past `s`, or a
/// direct attempt at exactly `s` failed.
#[cfg(feature = "dfa-prefilter")]
fn own_memo_skips(
    set: &OnigRegSet,
    index: usize,
    str_data: &[u8],
    start: usize,
    s: usize,
    memo_enabled: bool,
) -> bool {
    if !memo_enabled {
        return false;
    }
    let Ok(slot) = set
        .fallback_search_candidates
        .binary_search_by_key(&(index as u16), |candidate| candidate.index)
    else {
        return false;
    };
    let candidate = &set.fallback_search_candidates[slot];
    if !candidate.memo_safe
        || (candidate.after_newline_only && start > 0 && str_data[start - 1] != b'\n')
    {
        return false;
    }
    s >= candidate.no_match_from
        || candidate.exact_miss == s
        || (candidate.match_from <= s && s < candidate.match_at)
}

/// Attempts the own entries `own` and the covered candidates `candidates`
/// (both ascending) at `position` in index order under the decision's
/// bound, recording the first event; whether one was recorded. An own
/// fallback entry is left out where its memo rules the attempt out or the
/// own fallback search covered the position (`own_fallback_done`), an own
/// table entry where the position-by-position scan did (`own_table_done`).
#[cfg(feature = "dfa-prefilter")]
// The attempts take the search's bounds as `attempt_entry_at` does.
#[allow(clippy::too_many_arguments)]
#[inline]
fn attempt_merged_at(
    set: &mut OnigRegSet,
    own: &[u16],
    candidates: &[u16],
    own_fallback_done: bool,
    own_table_done: bool,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
    decision: &mut Option<RegSetDecision>,
    memo_enabled: bool,
) -> bool {
    let (mut i, mut j) = (0, 0);
    loop {
        let (index, is_own) = match (own.get(i), candidates.get(j)) {
            (Some(&o), Some(&c)) if o < c => {
                i += 1;
                (o as usize, true)
            }
            (Some(&o), None) => {
                i += 1;
                (o as usize, true)
            }
            (_, Some(&c)) => {
                j += 1;
                (c as usize, false)
            }
            (None, None) => return false,
        };
        if decision
            .map(decision_position_and_index)
            .is_some_and(|bound| (position as i32, index as i32) >= bound)
        {
            return false;
        }
        if is_own {
            if set.entries[index].fallback {
                if own_fallback_done
                    || own_memo_skips(set, index, str_data, search_start, position, memo_enabled)
                {
                    continue;
                }
            } else if own_table_done {
                continue;
            }
        }
        if let Some(found) = attempt_entry_at(
            set,
            index,
            str_data,
            end,
            position,
            search_start,
            range,
            option,
            msa,
        ) {
            record_regset_decision(set, decision, found);
            return true;
        }
    }
}

/// The own table entries at the positions from `from` up to `until`
/// (character heads, within the range), in increasing order under the
/// decision's bound; the first position not attempted.
#[cfg(feature = "dfa-prefilter")]
// The attempts take the search's bounds as `attempt_entry_at` does.
#[allow(clippy::too_many_arguments)]
fn attempt_own_table_entries_over(
    set: &mut OnigRegSet,
    prefilter: &crate::dfa_prefilter::SetPrefilter,
    str_data: &[u8],
    end: usize,
    from: usize,
    until: usize,
    search_start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
    decision: &mut Option<RegSetDecision>,
) -> usize {
    let mut p = from;
    while p < until && p <= range.min(end) {
        if decision
            .map(decision_position_and_index)
            .is_some_and(|(position, _)| (position as usize) < p)
        {
            break;
        }
        attempt_own_table_entries_at(
            set,
            prefilter,
            str_data,
            end,
            p,
            search_start,
            range,
            option,
            msa,
            decision,
        );
        p += enclen(set.enc, str_data, p);
    }
    p
}

/// The own table entries at `position`: the own fallback entries are
/// searched over a stretch by `search_fallback_entries`.
#[cfg(feature = "dfa-prefilter")]
// The attempts take the search's bounds as `attempt_entry_at` does.
#[allow(clippy::too_many_arguments)]
#[inline]
fn attempt_own_table_entries_at(
    set: &mut OnigRegSet,
    prefilter: &crate::dfa_prefilter::SetPrefilter,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
    decision: &mut Option<RegSetDecision>,
) -> bool {
    attempt_entries_at(
        set,
        prefilter.own(),
        true,
        str_data,
        end,
        position,
        search_start,
        range,
        option,
        msa,
        decision,
    )
}

/// The candidates the automata named last (`candidates_at`) at `position`.
#[cfg(feature = "dfa-prefilter")]
// The attempts take the search's bounds as `attempt_entry_at` does.
#[allow(clippy::too_many_arguments)]
#[inline]
fn attempt_candidates_at(
    set: &mut OnigRegSet,
    prefilter: &crate::dfa_prefilter::SetPrefilter,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
    decision: &mut Option<RegSetDecision>,
) -> bool {
    attempt_entries_at(
        set,
        prefilter.candidates(),
        false,
        str_data,
        end,
        position,
        search_start,
        range,
        option,
        msa,
        decision,
    )
}

/// Attempts `indexes` (ascending) at `position` under the decision's bound,
/// recording the first event; `table_only` leaves the fallback entries
/// out. Whether an event was recorded.
#[cfg(feature = "dfa-prefilter")]
// The attempts take the search's bounds as `attempt_entry_at` does.
#[allow(clippy::too_many_arguments)]
#[inline]
fn attempt_entries_at(
    set: &mut OnigRegSet,
    indexes: &[u16],
    table_only: bool,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
    decision: &mut Option<RegSetDecision>,
) -> bool {
    for &index in indexes {
        let index = index as usize;
        if decision
            .map(decision_position_and_index)
            .is_some_and(|bound| (position as i32, index as i32) >= bound)
        {
            break;
        }
        if table_only && set.entries[index].fallback {
            continue;
        }
        if let Some(found) = attempt_entry_at(
            set,
            index,
            str_data,
            end,
            position,
            search_start,
            range,
            option,
            msa,
        ) {
            record_regset_decision(set, decision, found);
            return true;
        }
    }
    false
}

/// One attempt of entry `index` at `position` in a position-lead search
/// from `search_start`, as that search would make it: a table entry only
/// where the first-byte table dispatches it (its optimizer's first byte
/// or start map; every table entry at the end), an any-char-star entry
/// only at the first position or after a newline, and a fallback or gated
/// entry only where C's optimizer admits the position (`entry_search_range`,
/// kept per search in `gates` and asked after an attempt with an event, as
/// the table scan and `attempt_fallback_entry_at_start` do).
#[cfg(feature = "dfa-prefilter")]
// The attempt takes the search's bounds as the fallback attempts do.
#[allow(clippy::too_many_arguments)]
fn attempt_entry_at(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
    range: usize,
    option: OnigOptionType,
    msa: &mut MatchArg,
) -> Option<RegSetDecision> {
    if !set.entries[index].fallback && position < end {
        let byte = str_data[position];
        let start_bytes = table_start_bytes(set, index);
        if (start_bytes[byte as usize / 64] >> (byte % 64)) & 1 == 0 {
            return None;
        }
    }
    if entry_skips_position(set, index, str_data, end, position, search_start) {
        return None;
    }
    let entry = &mut set.entries[index];
    let fill = EntryRegion::of(&entry.reg, option, msa);
    msa.retry_limit_in_search_counter = 0;
    let skips = may_skip_first_op_failures(msa, option);
    if first_op_rejects(
        entry,
        skips,
        str_data,
        end,
        position,
        search_start,
        option,
        msa,
    ) {
        return None;
    }
    let result = attempt_fallback_entry(
        entry,
        str_data,
        end,
        position,
        search_start,
        option,
        fill,
        msa,
    );
    let decision = fallback_attempt_decision(result, index, position, msa)?;
    let optimizer_decides = entry.fallback || entry.gated;
    if !optimizer_decides
        || table_gate_admits(set, index, str_data, end, search_start, range, position)
    {
        Some(decision)
    } else {
        // C never made this attempt: leave no match behind.
        clear_regset_entry_region(set, index as i32);
        None
    }
}

/// Whether entry `index` skips `position` in a position-lead search from
/// `search_start` before any attempt: an any-char-star entry attempts only
/// the first position and those after a newline, a `\G` entry only the
/// first, an entry needs its threshold length, and a start filter excludes
/// the byte.
#[cfg(feature = "dfa-prefilter")]
#[inline(always)]
fn entry_skips_position(
    set: &OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    position: usize,
    search_start: usize,
) -> bool {
    let entry = &set.entries[index];
    let anchor = entry.reg.anchor;
    if set.anychar_inf
        && (anchor & ANCR_ANYCHAR_INF) != 0
        && position > search_start
        && str_data[position - 1] != b'\n'
    {
        return true;
    }
    if (anchor & ANCR_BEGIN_POSITION) != 0 && position != search_start {
        return true;
    }
    if end - position < entry.reg.threshold_len.max(0) as usize {
        return true;
    }
    position < end
        && entry
            .start_filter
            .as_deref()
            .is_some_and(|filter| filter[str_data[position] as usize] == 0)
        && (set.subject_utf8 || start_map_may_skip(&entry.reg, str_data, position))
}

/// Whether a position-lead search from `start` attempts covered entry
/// `index` at `s`, a table entry the first-byte table dispatches on the
/// byte there: `attempt_entry_at`'s admission, decided ahead of the
/// attempt. No entry that skips the position (`entry_skips_position`), and
/// a fallback or gated entry only where C's optimizer admits `s` (the
/// per-search gate cache, which the positions visit in increasing order
/// as the attempts do).
#[cfg(feature = "dfa-prefilter")]
#[allow(clippy::too_many_arguments)]
fn covered_entry_admits_at(
    set: &mut OnigRegSet,
    index: usize,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    s: usize,
) -> bool {
    if entry_skips_position(set, index, str_data, end, s, start) {
        return false;
    }
    let entry = &set.entries[index];
    if !(entry.fallback || entry.gated) {
        return true;
    }
    table_gate_admits(set, index, str_data, end, start, range, s)
}

/// The bytes that dispatch table entry `index`.
fn table_start_bytes(set: &mut OnigRegSet, index: usize) -> [u64; 4] {
    let n = set.entries.len();
    let table = &set.first_byte_candidates;
    let bytes = set.table_start_bytes.get_or_insert_with(|| {
        let mut bytes = vec![Some([0u64; 4]); n];
        for (byte, slot) in table.iter().enumerate() {
            for &i in slot {
                if let Some(bits) = bytes[i as usize].as_mut() {
                    bits[byte / 64] |= 1 << (byte % 64);
                }
            }
        }
        bytes
    });
    bytes[index].unwrap_or([0; 4])
}

/// Swap entry `index`'s region with `region`, so a caller can keep the match
/// `onig_regset_entry_search` recorded without copying it.
pub(crate) fn onig_regset_swap_region(
    set: &mut OnigRegSet,
    index: usize,
    region: &mut Option<OnigRegion>,
) {
    std::mem::swap(&mut set.entries[index].region, region);
    // Every entry keeps a region for the next match to fill.
    set.entries[index]
        .region
        .get_or_insert_with(OnigRegion::new);
}

/// Run the established table scan first, then independently search only the
/// entries whose start byte is not provable from their bytecode.
///
/// A delayed optimizer byte is not a safe position-lead dispatch key: it may
/// occur after a match's real start. Such an entry is searched on its own
/// with the positions its optimizer admits (`search_fallback_entry`), which
/// yields its earliest match or error. The current table winner bounds each
/// fallback search: a later-index entry only needs positions strictly before
/// the winner, while an earlier-index entry also needs the winner's position
/// to resolve a tie.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn regset_search_body_position_lead(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    option: OnigOptionType,
    skip_region_for_nomem: bool,
    fallback_memo_id: Option<FallbackMemoIdentity>,
) -> (i32, i32) {
    let limits = refresh_scratch_limits(set);
    // The gates keep each gated entry's optimizer range per subject; the
    // DFA pre-filter keeps every fallback entry's there as well.
    #[cfg(feature = "dfa-prefilter")]
    let keeps_gates = set.has_gated || set.prefilter.is_some();
    #[cfg(not(feature = "dfa-prefilter"))]
    let keeps_gates = set.has_gated;
    if keeps_gates {
        let subject = fallback_memo_id
            .filter(|_| range == end)
            .map(|identity| (identity, end));
        if subject.is_none() || subject != set.gate_subject {
            set.gate_subject = subject;
            set.gate_generation = set.gate_generation.wrapping_add(1);
            if set.gate_generation == 0 {
                set.gates.fill(EntryGate::STALE);
                set.gate_generation = 1;
            }
        }
    }
    // A search retry budget makes a result depend on where its search began,
    // so a result from an earlier start cannot stand in for a later one.
    let memo_enabled = fallback_memo_id.is_some()
        && range == end
        && limits.retry_limit_in_search == 0
        && !set.fallback_search_candidates.is_empty();
    let memo_key = fallback_memo_id
        .filter(|_| memo_enabled)
        .map(|identity| FallbackMemoKey {
            identity,
            end,
            option,
            retry_limit_in_match: limits.retry_limit_in_match,
            retry_limit_in_search: limits.retry_limit_in_search,
            match_stack_limit: limits.match_stack_limit,
            time_limit: limits.time_limit,
            subexp_call_limit_in_search: limits.subexp_call_limit_in_search,
        });

    // Rust-only (ADR-008): the DFA pre-filter decides which entries attempt
    // which positions; the rest of this function is the search it stands
    // in for, and takes over a search the pre-filter gives up.
    #[cfg(feature = "dfa-prefilter")]
    if prefilter_decides(set, limits, option, str_data, start) {
        // The memo serves the fallback entries the pre-filter does not
        // cover; a set without those keeps it untouched.
        if set.prefilter_own_fallback {
            prepare_fallback_memo(set, memo_key);
        }
        if let Some(decision) = regset_search_body_prefilter(
            set,
            str_data,
            end,
            start,
            range,
            option,
            memo_enabled,
            fallback_memo_id.map(|identity| (identity, end)),
        ) {
            return regset_decision_result(set, decision);
        }
    }
    prepare_fallback_memo(set, memo_key);

    if set.fallback_search_candidates.is_empty() {
        let decision = regset_search_body_position_lead_table(
            set,
            str_data,
            end,
            start,
            range,
            option,
            skip_region_for_nomem,
        );
        return regset_decision_result(set, decision);
    }

    // The table scan stops at its first event, but without one it would
    // walk the rest of the subject before a fallback entry gets a say, even
    // where a fallback entry matches at the start. C looks at every regex
    // position by position. Scanning in growing windows, with the fallback
    // entries asked for events inside each window, keeps a search that
    // ends early short, like C's.
    const FIRST_WINDOW: usize = 256;
    let mut from = start;
    let mut to = if range - start > FIRST_WINDOW {
        start + FIRST_WINDOW
    } else {
        range
    };
    // A fallback event found past the window (a memoized search covers the
    // whole range) bounds the rest of the table scan; the final pass over
    // the fallback entries starts from it.
    let mut fallback_bound: Option<RegSetDecision> = None;
    let decision = loop {
        let bound_position =
            fallback_bound.map(|found| decision_position_and_index(found).0 as usize);
        let scan_to = bound_position.map_or(to, |position| to.min(position));
        let (table, next) = if table_scan_gates_eagerly(set) {
            regset_table_scan::<true>(
                set,
                str_data,
                end,
                start,
                range,
                from,
                scan_to,
                option,
                skip_region_for_nomem,
            )
        } else {
            regset_table_scan::<false>(
                set,
                str_data,
                end,
                start,
                range,
                from,
                scan_to,
                option,
                skip_region_for_nomem,
            )
        };
        if table.is_some()
            || scan_to >= range
            || bound_position.is_some_and(|position| next > position)
        {
            break search_fallback_entries(
                set,
                str_data,
                end,
                start,
                range,
                range,
                option,
                memo_enabled,
                false,
                table.or(fallback_bound),
            );
        }
        if fallback_bound.is_none() {
            // No table entry has an event up to `to`, so a fallback event up
            // to `to` decides the search.
            fallback_bound = search_fallback_entries(
                set,
                str_data,
                end,
                start,
                range,
                to,
                option,
                memo_enabled,
                false,
                None,
            );
            if let Some(found) = fallback_bound {
                if decision_position_and_index(found).0 as usize <= to {
                    break Some(found);
                }
            }
        }
        from = next;
        to = range.min(start + (to - start).saturating_mul(4));
    };
    regset_decision_result(set, decision)
}

/// The fallback entries' part of a position-lead search from `start` whose
/// match start range is `range`: each entry's first event, bounded by the
/// current `decision` and by `limit`, merged into that decision.
///
/// With `memo_enabled`, an entry's search runs over the whole range so its
/// result can serve later starts; an event past `limit` may then come back.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn search_fallback_entries(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    limit: usize,
    option: OnigOptionType,
    memo_enabled: bool,
    uncovered_only: bool,
    mut decision: Option<RegSetDecision>,
) -> Option<RegSetDecision> {
    let mut fallback_msa: Option<MatchArg> = None;
    // The character head before the current decision's position, which
    // bounds every later-index entry. Cached per decision position.
    let mut before_decision: Option<(usize, Option<usize>)> = None;

    for candidate_at in 0..set.fallback_search_candidates.len() {
        let candidate = set.fallback_search_candidates[candidate_at];
        let index = candidate.index as usize;
        if uncovered_only && set.entries[index].prefilter_covered {
            continue;
        }
        let bound = decision.map(decision_position_and_index);
        // Candidates run in index order and a decision only moves to an
        // earlier (position, index). Once it sits at `start` ahead of this
        // index, no remaining entry can win: each would be skipped below.
        if bound
            .is_some_and(|(position, winner)| position as usize <= start && index as i32 >= winner)
        {
            break;
        }
        // Callouts and position-sensitive bytecode can observe each attempt,
        // so replay those entries rather than reusing a cached result.
        let memo_enabled = memo_enabled && candidate.memo_safe;
        // A search attempts its first position even where an any-char-star
        // entry skips positions that do not follow a newline, so a result
        // from an earlier start does not cover such a start.
        let memo_reusable = memo_enabled
            && !(candidate.after_newline_only && start > 0 && str_data[start - 1] != b'\n');
        if memo_reusable && start >= candidate.no_match_from {
            continue;
        }
        // The scratch MatchArg is taken only by an entry that attempts a
        // match; most entries of a warm call settle from their memo.
        macro_rules! fallback_msa {
            () => {
                fallback_msa.get_or_insert_with(|| take_scratch_msa(set, option, start))
            };
        }
        if candidate.begin_position {
            // `\G` fails at every position but the start; C attempts the
            // start only where the optimizer admits it.
            if bound.is_some_and(|bound| (start as i32, index as i32) >= bound) {
                continue;
            }
            let msa = fallback_msa!();
            if let Some(candidate) =
                search_fallback_entry(set, index, str_data, end, start, range, start, option, msa)
            {
                record_regset_decision(set, &mut decision, candidate);
            }
            continue;
        }

        let search_range = match bound {
            Some((position, winner)) if index as i32 >= winner => {
                let position = position as usize;
                let before = match before_decision {
                    Some((cached, before)) if cached == position => before,
                    _ => {
                        let before = onigenc_get_prev_char_head(set.enc, start, position, str_data);
                        before_decision = Some((position, before));
                        before
                    }
                };
                match before {
                    Some(position) => position,
                    None => continue,
                }
            }
            Some((position, _)) => position as usize,
            None => limit,
        };
        if search_range < start {
            continue;
        }

        if memo_reusable {
            // The newest results sit in the candidate itself; every valid
            // `MatchAt` for this start names the same position.
            let newest_match = (candidate.match_from <= start && start <= candidate.match_at)
                .then_some((candidate.match_from, candidate.match_at));
            if newest_match.is_none() && candidate.exact_miss == start {
                continue;
            }
            if let Some((searched_from, position)) = newest_match.or_else(|| {
                set.fallback_memos[index]
                    .iter()
                    .filter_map(|memo| match *memo {
                        FallbackMemo::MatchAt {
                            searched_from,
                            position,
                        } if start >= searched_from && position >= start => {
                            Some((searched_from, position))
                        }
                        _ => None,
                    })
                    .min_by_key(|(_, position)| *position)
            }) {
                // The search from `searched_from` attempted every position
                // this search attempts and found nothing before `position`.
                if position > search_range
                    || decision.is_some_and(|current| {
                        (position as i32, index as i32) >= decision_position_and_index(current)
                    })
                {
                    continue;
                }
                // Known to match: one pass that records the captures.
                let msa = fallback_msa!();
                let result = attempt_fallback_entry(
                    &mut set.entries[index],
                    str_data,
                    end,
                    position,
                    start,
                    option,
                    EntryRegion::Fill,
                    msa,
                );
                if result >= 0 {
                    record_regset_decision(
                        set,
                        &mut decision,
                        RegSetDecision::Match(RegSetWinner {
                            index: index as i32,
                            position: position as i32,
                            match_len: result,
                        }),
                    );
                    continue;
                }
                // Not reproducible: forget it and search afresh.
                set.fallback_memos[index].retain(|memo| {
                    !matches!(memo, FallbackMemo::MatchAt { searched_from: from, .. } if *from == searched_from)
                });
                let candidate = &mut set.fallback_search_candidates[candidate_at];
                if candidate.match_from == searched_from {
                    candidate.match_from = usize::MAX;
                    candidate.match_at = usize::MAX;
                }
            } else if set.fallback_memos[index]
                .iter()
                .any(|memo| matches!(memo, FallbackMemo::ExactStartMiss(miss) if *miss == start))
            {
                continue;
            }
        }

        // A single position (the decision sits right after `start`, or at
        // `start` behind a later entry) needs no search over the rest of the
        // subject. Its miss is remembered; a miss at another start upgrades
        // to one search over the remaining text, whose match or no-match
        // result then serves as the advancing cursor.
        let has_different_exact_start_miss = memo_enabled
            && set.fallback_memos[index].iter().any(|memo| {
                matches!(memo, FallbackMemo::ExactStartMiss(exact_start) if *exact_start != start)
            });
        let msa = fallback_msa!();
        if search_range == start && !has_different_exact_start_miss {
            let found =
                search_fallback_entry(set, index, str_data, end, start, range, start, option, msa);
            if let Some(candidate) = found {
                record_regset_decision(set, &mut decision, candidate);
            } else if memo_enabled {
                let memos = &mut set.fallback_memos[index];
                if memos.len() == FALLBACK_MEMO_CAPACITY {
                    memos.remove(0);
                }
                memos.push(FallbackMemo::ExactStartMiss(start));
                set.fallback_search_candidates[candidate_at].exact_miss = start;
            }
            continue;
        }

        // With the memo, search the whole remaining range so the result can
        // serve later starts; a match or an error past the current bound
        // then loses against the decision.
        let found = search_fallback_entry(
            set,
            index,
            str_data,
            end,
            start,
            range,
            if memo_enabled { range } else { search_range },
            option,
            msa,
        );
        match found {
            Some(RegSetDecision::Match(winner)) => {
                if memo_enabled {
                    let memos = &mut set.fallback_memos[index];
                    memos.retain(|memo| {
                        !matches!(memo, FallbackMemo::ExactStartMiss(_))
                            && !matches!(memo, FallbackMemo::MatchAt { searched_from, .. } if *searched_from == start)
                    });
                    if memos.len() == FALLBACK_MEMO_CAPACITY {
                        memos.remove(0);
                    }
                    memos.push(FallbackMemo::MatchAt {
                        searched_from: start,
                        position: winner.position as usize,
                    });
                    let candidate = &mut set.fallback_search_candidates[candidate_at];
                    candidate.exact_miss = usize::MAX;
                    candidate.match_from = start;
                    candidate.match_at = winner.position as usize;
                }
                record_regset_decision(set, &mut decision, RegSetDecision::Match(winner));
            }
            // An error is not remembered: whether a later start reaches the
            // same attempt depends on that start's optimizer search.
            Some(error @ RegSetDecision::Error(_)) => {
                record_regset_decision(set, &mut decision, error);
            }
            None => {
                if memo_enabled {
                    set.fallback_memos[index].clear();
                    let candidate = &mut set.fallback_search_candidates[candidate_at];
                    candidate.no_match_from = start;
                    candidate.exact_miss = usize::MAX;
                    candidate.match_from = usize::MAX;
                    candidate.match_at = usize::MAX;
                }
            }
        }
    }

    if fallback_msa.is_some() {
        set.scratch_msa = fallback_msa;
    }
    decision
}

/// Readies the fallback memo for a search under `memo_key`: a key other
/// than the one the memos were recorded under drops them.
fn prepare_fallback_memo(set: &mut OnigRegSet, memo_key: Option<FallbackMemoKey>) {
    let Some(key) = memo_key else {
        return;
    };
    if set.fallback_memo_key != Some(key) {
        set.fallback_memo_key = Some(key);
        for memos in &mut set.fallback_memos {
            memos.clear();
        }
        for candidate in &mut set.fallback_search_candidates {
            candidate.no_match_from = usize::MAX;
            candidate.exact_miss = usize::MAX;
            candidate.match_from = usize::MAX;
            candidate.match_at = usize::MAX;
        }
    }
}

/// Regex-lead search: iterate regexes, find earliest match.
fn regset_search_body_regex_lead(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    orig_range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
) -> (i32, i32) {
    let n = set.entries.len();
    let mut match_index: i32 = ONIG_MISMATCH;
    let mut match_pos: i32 = 0;
    let mut ep = orig_range;

    for i in 0..n {
        let region = set.entries[i].region.take();
        // C: search_in_range(reg, str, end, start, ep, orig_range, ...) --
        // only the start range narrows; a match may still run to orig_range.
        let (r, returned_region) = search_in_range(
            &set.entries[i].reg,
            str_data,
            end,
            start,
            ep,
            orig_range,
            region,
            option,
            None,
        );
        set.entries[i].region = returned_region;

        if r > 0 {
            if (r as usize) < ep {
                match_index = i as i32;
                match_pos = r;
                if lead == OnigRegSetLead::PriorityToRegexOrder {
                    break;
                }
                ep = r as usize;
            }
        } else if r == 0 {
            match_index = i as i32;
            match_pos = 0;
            break;
        }
    }

    (match_index, match_pos)
}

#[allow(clippy::too_many_arguments)]
fn onig_regset_search_impl(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    eager_region_reset: bool,
    fallback_memo_id: Option<FallbackMemoIdentity>,
    subject_utf8: bool,
) -> (i32, i32) {
    let n = set.entries.len();
    if n == 0 {
        return (ONIG_MISMATCH, 0);
    }
    set.last_match_len = ONIG_MISMATCH;
    set.subject_utf8 = subject_utf8;

    let end = end.min(str_data.len());
    let range = range.min(end);
    if start > end {
        return (ONIG_MISMATCH, 0);
    }

    // Forward search only
    if !str_data.is_empty() && range < start {
        return (ONIGERR_INVALID_ARGUMENT, 0);
    }

    if eager_region_reset {
        // Preserve classic regset behavior: all regions are reset on each call.
        for entry in &mut set.entries {
            if let Some(ref mut region) = entry.region {
                region.resize(entry.reg.num_mem + 1);
                region.clear();
            }
        }
    }

    // Empty logical string handling. A non-empty string searched from its end
    // must continue through the lead-specific path, matching upstream.
    if end == 0 {
        for i in 0..n {
            if set.entries[i].reg.threshold_len == 0 {
                let region = set.entries[i].region.take();
                let (r, returned_region) =
                    onig_match(&set.entries[i].reg, str_data, end, start, region, option);
                set.entries[i].region = returned_region;
                if r >= 0 {
                    set.last_match_len = r;
                    return (i as i32, start as i32);
                }
                if r != ONIG_MISMATCH {
                    return (r, 0); // error
                }
            }
        }
        return (ONIG_MISMATCH, 0);
    }

    // Anchor optimization
    let mut cur_start = start;
    let mut cur_range = range;
    let orig_range = range;

    if set.anchor != 0 && !str_data.is_empty() {
        if (set.anchor & ANCR_BEGIN_POSITION) != 0 {
            cur_range = start + 1;
        } else if (set.anchor & ANCR_BEGIN_BUF) != 0 {
            if start != 0 {
                return (ONIG_MISMATCH, 0);
            }
            cur_range = 1;
        } else if (set.anchor & ANCR_END_BUF) != 0 {
            let min_semi_end = end;
            let max_semi_end = end;

            if (max_semi_end as OnigLen) < set.anc_dmin {
                return (ONIG_MISMATCH, 0);
            }
            if min_semi_end.saturating_sub(start) > set.anc_dmax as usize
                && set.anc_dmax != INFINITE_LEN
            {
                cur_start = min_semi_end - set.anc_dmax as usize;
            }
            if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < set.anc_dmin as usize {
                cur_range = max_semi_end.saturating_sub(set.anc_dmin as usize) + 1;
            }
            if cur_start > cur_range {
                return (ONIG_MISMATCH, 0);
            }
        } else if (set.anchor & ANCR_SEMI_END_BUF) != 0 {
            let max_semi_end = end;
            let mut min_semi_end = end;
            if end > 0 && str_data[end - 1] == b'\n' {
                min_semi_end = end - 1;
            }

            if (max_semi_end as OnigLen) < set.anc_dmin {
                return (ONIG_MISMATCH, 0);
            }
            if min_semi_end.saturating_sub(start) > set.anc_dmax as usize
                && set.anc_dmax != INFINITE_LEN
            {
                cur_start = min_semi_end - set.anc_dmax as usize;
            }
            if max_semi_end.saturating_sub(cur_range.saturating_sub(1)) < set.anc_dmin as usize {
                cur_range = max_semi_end.saturating_sub(set.anc_dmin as usize) + 1;
            }
            if cur_start > cur_range {
                return (ONIG_MISMATCH, 0);
            }
        } else if (set.anchor & ANCR_ANYCHAR_INF_ML) != 0 {
            cur_range = start + 1;
        }
    }

    // A begin-position anchor sets `cur_range = start + 1` even when `start`
    // already sits at the logical end, so the range can point one past the
    // haystack. C tolerates that because its buffer is NUL-terminated; the
    // position loops below index the haystack up to `cur_range`, so keep it
    // inside the string. The only attempt that survives is the one at `end`.
    let cur_range = cur_range.min(end);

    let (result, match_pos) = if lead == OnigRegSetLead::PositionLead {
        regset_search_body_position_lead(
            set,
            str_data,
            end,
            cur_start,
            cur_range,
            option,
            !eager_region_reset,
            fallback_memo_id,
        )
    } else {
        regset_search_body_regex_lead(set, str_data, end, cur_start, orig_range, lead, option)
    };

    if eager_region_reset {
        // Clear regions for non-matching regexes with FIND_NOT_EMPTY.
        if result >= 0 {
            for i in 0..n {
                if opton_find_not_empty(set.entries[i].reg.options) {
                    if let Some(ref mut region) = set.entries[i].region {
                        if (i as i32) != result {
                            region.clear();
                        }
                    }
                }
            }
        }
    }

    (result, match_pos)
}

/// Search the set of regexes against a string.
///
/// Returns (regex_index, match_position) where:
/// - regex_index >= 0: index of the matching regex
/// - regex_index == ONIG_MISMATCH (-1): no match
/// - regex_index < -1: error code
pub fn onig_regset_search(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
) -> (i32, i32) {
    onig_regset_search_impl(
        set, str_data, end, start, range, lead, option, true, None, false,
    )
}

/// Fast regset search for high-frequency callers (e.g. scanner tokenization).
///
/// Semantics of match index/position are identical to `onig_regset_search`.
/// Only the matched regex's region is guaranteed to be up-to-date; non-matching
/// regex regions may remain from previous calls.
pub(crate) fn onig_regset_search_fast(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
) -> (i32, i32) {
    onig_regset_search_impl(
        set, str_data, end, start, range, lead, option, false, None, false,
    )
}

/// Fast RegSet search with a stable immutable string identity for caching
/// optimizer-backed fallback results across advancing scanner positions.
#[allow(clippy::too_many_arguments)]
pub(crate) fn onig_regset_search_fast_with_id(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    identity: FallbackMemoIdentity,
) -> (i32, i32) {
    onig_regset_search_impl(
        set,
        str_data,
        end,
        start,
        range,
        lead,
        option,
        false,
        Some(identity),
        false,
    )
}

/// `onig_regset_search_fast` for a scanner's subject, which is valid UTF-8
/// (`MatchArg::subject_utf8`), with its string identity if it has one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn onig_regset_search_utf8(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    identity: Option<FallbackMemoIdentity>,
) -> (i32, i32) {
    debug_assert!(std::str::from_utf8(str_data).is_ok());
    onig_regset_search_impl(
        set, str_data, end, start, range, lead, option, false, identity, true,
    )
}

/// Search the set with per-regex match parameters.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_arguments)]
pub fn onig_regset_search_with_param(
    set: &mut OnigRegSet,
    str_data: &[u8],
    end: usize,
    start: usize,
    range: usize,
    lead: OnigRegSetLead,
    option: OnigOptionType,
    mps: &[OnigMatchParam],
) -> (i32, i32) {
    let n = set.entries.len();
    if n == 0 {
        return (ONIG_MISMATCH, 0);
    }
    if mps.len() < n {
        return (ONIGERR_INVALID_ARGUMENT, 0);
    }
    set.last_match_len = ONIG_MISMATCH;

    let end = end.min(str_data.len());
    let range = range.min(end);
    if start > end {
        return (ONIG_MISMATCH, 0);
    }

    // Forward search only
    if !str_data.is_empty() && range < start {
        return (ONIGERR_INVALID_ARGUMENT, 0);
    }

    // Resize and clear all regions
    for entry in &mut set.entries {
        if let Some(ref mut region) = entry.region {
            region.resize(entry.reg.num_mem + 1);
            region.clear();
        }
    }

    // Empty logical string handling. A non-empty string searched from its end
    // must continue through the lead-specific path, matching upstream.
    if end == 0 {
        for i in 0..n {
            if set.entries[i].reg.threshold_len == 0 {
                let region = set.entries[i].region.take();
                let (r, returned_region) =
                    onig_match(&set.entries[i].reg, str_data, end, start, region, option);
                set.entries[i].region = returned_region;
                if r >= 0 {
                    set.last_match_len = r;
                    return (i as i32, start as i32);
                }
                if r != ONIG_MISMATCH {
                    return (r, 0);
                }
            }
        }
        return (ONIG_MISMATCH, 0);
    }

    // For regex-lead with params, use search_with_param per regex
    if lead != OnigRegSetLead::PositionLead {
        let orig_range = range;
        let mut match_index: i32 = ONIG_MISMATCH;
        let mut match_pos: i32 = 0;
        let mut ep = orig_range;

        for (i, entry) in set.entries.iter_mut().take(n).enumerate() {
            let region = entry.region.take();
            let (r, returned_region) = search_in_range(
                &entry.reg,
                str_data,
                end,
                start,
                ep,
                orig_range,
                region,
                option,
                Some(&mps[i]),
            );
            entry.region = returned_region;

            if r > 0 {
                if (r as usize) < ep {
                    match_index = i as i32;
                    match_pos = r;
                    if lead == OnigRegSetLead::PriorityToRegexOrder {
                        break;
                    }
                    ep = r as usize;
                }
            } else if r == 0 {
                match_index = i as i32;
                match_pos = 0;
                break;
            }
        }

        return (match_index, match_pos);
    }

    // Position-lead with params: delegate to non-param position-lead
    // (params mainly affect limits which are checked within onig_match)
    regset_search_body_position_lead(set, str_data, end, start, range, option, false, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encodings::utf8::ONIG_ENCODING_UTF8;
    use crate::regcomp::onig_new;
    use crate::regexec::onig_search;
    use crate::regexec::{
        exclusive_limits, onig_get_global_limit_revision, onig_get_match_stack_limit,
        onig_get_retry_limit_in_match, onig_get_retry_limit_in_search, onig_get_time_limit,
        onig_set_match_stack_limit, onig_set_retry_limit_in_match, onig_set_retry_limit_in_search,
        onig_set_time_limit, shared_limits,
    };
    use crate::regsyntax::OnigSyntaxOniguruma;

    /// The committed Shiki grammars, as the benchmarks load them.
    #[allow(dead_code)]
    mod grammar_loader {
        use crate as ferroni;
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/grammar_loader.rs"
        ));
    }

    /// The regex of an entry the test owns alone, to build a reference set
    /// without one of its Rust-only plans.
    fn reg_mut(entry: &mut RegSetEntry) -> &mut RegexType {
        Arc::get_mut(&mut entry.reg).expect("a test set owns its regexes")
    }

    fn fallback_indices(set: &OnigRegSet) -> Vec<u16> {
        set.fallback_search_candidates
            .iter()
            .map(|candidate| candidate.index)
            .collect()
    }

    /// The single fallback entry holds exactly a settled no-match result
    /// from `start`.
    fn assert_settled_from(set: &OnigRegSet, start: usize) {
        assert_eq!(set.fallback_search_candidates[0].no_match_from, start);
        assert!(set.fallback_memos[0].is_empty());
    }

    /// Which fast paths the committed grammars reach, compiled as a scanner
    /// compiles them. Eligibility checks fail quietly: an overly cautious
    /// one passes every behavioral test while disabling its optimization
    /// for real patterns (a literal trie once dropped its pattern's start
    /// map; region-free matching once never applied to grammar captures).
    /// When a change moves these numbers on purpose, update them and say
    /// why in the commit.
    #[test]
    fn grammar_fast_path_census() {
        #[derive(Debug, PartialEq)]
        struct Census {
            patterns: usize,
            table_entries: usize,
            fallback_entries: usize,
            fallback_start_filters: usize,
            literal_tries: usize,
            folded_literal_tries: usize,
            without_optimizer: usize,
            capture_tracking: usize,
            fused_look_behinds: usize,
            stepping_look_behinds: usize,
            byte_set_push_guards: usize,
            unguarded_pushes: usize,
        }
        let census = |patterns: Vec<String>| {
            let regs: Vec<Box<RegexType>> =
                patterns.iter().map(|p| compile(p.as_bytes())).collect();
            // Push guards rewrite bytecode that the start maps read.
            crate::regcomp::PUSH_GUARDS_DISABLED.with(|disabled| disabled.set(true));
            let unguarded: Vec<Box<RegexType>> =
                patterns.iter().map(|p| compile(p.as_bytes())).collect();
            crate::regcomp::PUSH_GUARDS_DISABLED.with(|disabled| disabled.set(false));
            for ((pattern, reg), reference) in patterns.iter().zip(&regs).zip(&unguarded) {
                assert_eq!(
                    derive_start_byte_map(reg),
                    derive_start_byte_map(reference),
                    "{pattern}"
                );
            }
            let tries = || regs.iter().flat_map(|reg| reg.literal_tries.iter());
            let literal_tries = tries().count();
            let folded_literal_tries = tries().filter(|trie| trie.is_case_insensitive()).count();
            let without_optimizer = regs
                .iter()
                .filter(|reg| reg.optimize == OptimizeType::None)
                .count();
            let capture_tracking = regs.iter().filter(|reg| reg.needs_capture_tracking).count();
            let ops = |opcode: OpCode| {
                regs.iter()
                    .flat_map(|reg| reg.ops.iter())
                    .filter(|op| op.opcode == opcode)
                    .count()
            };
            let fused_look_behinds = ops(OpCode::LookBehindOp);
            let stepping_look_behinds = ops(OpCode::StepBackStart);
            let byte_set_push_guards = ops(OpCode::PushOrJumpByteSet);
            let unguarded_pushes = ops(OpCode::Push);
            let (set, r) = onig_regset_new(regs);
            assert_eq!(r, ONIG_NORMAL);
            let set = set.unwrap();
            Census {
                patterns: patterns.len(),
                table_entries: set.table_entry_count,
                fallback_entries: set.fallback_search_candidates.len(),
                fallback_start_filters: set
                    .fallback_search_candidates
                    .iter()
                    .filter(|c| set.entries[c.index as usize].start_filter.is_some())
                    .count(),
                literal_tries,
                folded_literal_tries,
                without_optimizer,
                capture_tracking,
                fused_look_behinds,
                stepping_look_behinds,
                byte_set_push_guards,
                unguarded_pushes,
            }
        };
        assert_eq!(
            census(grammar_loader::typescript_patterns()),
            Census {
                patterns: 279,
                table_entries: 200,
                fallback_entries: 79,
                fallback_start_filters: 79,
                // Whole literal lists in negative lookahead are eligible too.
                literal_tries: 24,
                folded_literal_tries: 0,
                without_optimizer: 3,
                capture_tracking: 0,
                fused_look_behinds: 443,
                stepping_look_behinds: 50,
                // A positive look-behind on a push's path ends its guard
                // (#259): over malformed UTF-8 it goes on where its body
                // ends, which is not the byte the guard reads.
                byte_set_push_guards: 2136,
                unguarded_pushes: 237,
            }
        );
        assert_eq!(
            census(grammar_loader::css_patterns()),
            Census {
                patterns: 117,
                // Two look-behind entries ahead of a case-insensitive trie
                // get C's bounded distance (`LiteralAltSummary`) and join
                // the table.
                table_entries: 109,
                fallback_entries: 8,
                fallback_start_filters: 6,
                literal_tries: 33,
                folded_literal_tries: 33,
                // Four of them have a start map too weak to search
                // (`extra_start_map_filters`), which still dispatches them.
                without_optimizer: 10,
                capture_tracking: 0,
                fused_look_behinds: 64,
                stepping_look_behinds: 11,
                // A positive look-behind ends a guard (#259).
                byte_set_push_guards: 1264,
                unguarded_pushes: 81,
            }
        );
        assert_eq!(
            census(grammar_loader::rust_patterns()),
            Census {
                patterns: 81,
                table_entries: 78,
                fallback_entries: 3,
                fallback_start_filters: 2,
                literal_tries: 8,
                folded_literal_tries: 0,
                without_optimizer: 0,
                capture_tracking: 0,
                fused_look_behinds: 6,
                stepping_look_behinds: 2,
                byte_set_push_guards: 28,
                unguarded_pushes: 4,
            }
        );
    }

    fn compile(pattern: &[u8]) -> Box<RegexType> {
        let reg = onig_new(
            pattern,
            ONIG_OPTION_NONE,
            &ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma,
        );
        match reg {
            Ok(r) => Box::new(r),
            Err(e) => panic!(
                "failed to compile {:?}: error {}",
                std::str::from_utf8(pattern),
                e
            ),
        }
    }

    #[test]
    fn regset_basic_position_lead() {
        let _limits = shared_limits();
        let regs = vec![compile(b"abc"), compile(b"def"), compile(b"ghi")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 1); // "def" matched
        assert_eq!(pos, 3); // at position 3
    }

    #[test]
    fn start_byte_map_includes_optional_prefix_and_required_byte() {
        let reg = compile(b"a?[bcd]");
        assert!(has_finite_variable_optimizer(&reg));

        let map = derive_start_byte_map(&reg).expect("simple optional prefix is analyzable");
        for byte in *b"abcd" {
            assert_ne!(map[byte as usize], 0, "missing byte {byte:?}");
        }
        assert_eq!(map[b'x' as usize], 0);
    }

    #[test]
    fn start_byte_map_handles_compiled_consumer_and_control_flow_shapes() {
        fn str1(byte: u8) -> Operation {
            let mut exact = [0; 16];
            exact[0] = byte;
            Operation {
                opcode: OpCode::Str1,
                payload: OperationPayload::Exact { s: exact },
            }
        }

        fn map_for(ops: Vec<Operation>, repeat_range: Vec<RepeatRange>) -> [u8; CHAR_MAP_SIZE] {
            let mut reg = compile(b"a");
            reg.ops = ops;
            reg.repeat_range = repeat_range;
            derive_start_byte_map(&reg).expect("analyzable bytecode")
        }

        let consumer_cases = [
            Operation {
                opcode: OpCode::StrN,
                payload: OperationPayload::ExactN {
                    s: b"long literal".to_vec(),
                    n: 12,
                },
            },
            Operation {
                opcode: OpCode::StrMb2n1,
                payload: OperationPayload::ExactLenN {
                    s: "é".as_bytes().to_vec(),
                    n: 2,
                    len: 1,
                },
            },
            Operation {
                opcode: OpCode::CClassMb,
                payload: OperationPayload::CClassMb { mb: Vec::new() },
            },
            Operation {
                opcode: OpCode::CClassMix,
                payload: OperationPayload::CClassMix {
                    mb: Vec::new(),
                    bsp: Box::new([0; BITSET_REAL_SIZE]),
                },
            },
            Operation {
                opcode: OpCode::Word,
                payload: OperationPayload::None,
            },
            Operation {
                opcode: OpCode::WordAscii,
                payload: OperationPayload::None,
            },
            Operation {
                opcode: OpCode::AnyCharStar,
                payload: OperationPayload::None,
            },
        ];
        for operation in consumer_cases {
            assert!(
                map_for(vec![operation], Vec::new())
                    .iter()
                    .any(|&value| value != 0)
            );
        }

        for opcode in [
            OpCode::Push,
            OpCode::PushOrJumpExact1,
            OpCode::PushIfPeekNext,
        ] {
            let payload = match opcode {
                OpCode::Push => OperationPayload::Push { addr: 2 },
                OpCode::PushOrJumpExact1 => OperationPayload::PushOrJumpExact1 {
                    addr: 2,
                    c: b'a',
                    skipped_retries: 0,
                },
                _ => OperationPayload::PushIfPeekNext { addr: 2, c: b'a' },
            };
            let map = map_for(
                vec![Operation { opcode, payload }, str1(b'a'), str1(b'b')],
                Vec::new(),
            );
            assert_ne!(map[b'a' as usize], 0);
            assert_ne!(map[b'b' as usize], 0);
        }

        let jump_map = map_for(
            vec![
                Operation {
                    opcode: OpCode::Jump,
                    payload: OperationPayload::Jump { addr: 1 },
                },
                str1(b'j'),
            ],
            Vec::new(),
        );
        assert_ne!(jump_map[b'j' as usize], 0);

        let repeat_map = map_for(
            vec![
                Operation {
                    opcode: OpCode::Repeat,
                    payload: OperationPayload::Repeat { id: 0, addr: 2 },
                },
                str1(b'r'),
                str1(b's'),
            ],
            vec![RepeatRange {
                lower: 0,
                upper: 1,
                u_offset: 0,
            }],
        );
        assert_ne!(repeat_map[b'r' as usize], 0);
        assert_ne!(repeat_map[b's' as usize], 0);
    }

    #[test]
    fn lookbehind_start_map_ignores_bytes_before_the_match_start() {
        let _limits = shared_limits();
        let reg = compile(b"(?<=x)a?bc");
        assert!(has_finite_variable_optimizer(&reg));
        let map = derive_start_byte_map(&reg).expect("lookbehind is analyzable");
        assert_ne!(map[b'a' as usize], 0);
        assert_ne!(map[b'b' as usize], 0);
        assert_eq!(map[b'x' as usize], 0);

        let (set, result) = onig_regset_new(vec![reg]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(set.fallback_search_candidates.is_empty());

        let input = b"xabc";
        let (index, position) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!((index, position), (0, 1));
    }

    #[test]
    fn fallback_run_skips_preserve_winners_captures_bounds_and_limits() {
        let _limits = exclusive_limits();
        struct RestoreLimits(u64, u64, u32, u64);
        impl Drop for RestoreLimits {
            fn drop(&mut self) {
                onig_set_retry_limit_in_match(self.0);
                onig_set_retry_limit_in_search(self.1);
                onig_set_match_stack_limit(self.2);
                onig_set_time_limit(self.3);
            }
        }
        let _restore = RestoreLimits(
            onig_get_retry_limit_in_match(),
            onig_get_retry_limit_in_search(),
            onig_get_match_stack_limit(),
            onig_get_time_limit(),
        );
        onig_set_time_limit(0);
        let patterns = [
            r"(?=\w?[\w\s]*\brecord\s+[$\w]+)",
            r"(?=\w?[-\w\s]*\b(?:class|(?<!@)interface|enum)\s+[$\w]+)",
            r"(?=\w?[\w\s]*\b(record)\s+(\w+))",
            r"(?=[\w\s]*\brecord\s+\w+)",
            r"(?=\w?[a-z]*record)",
            r"(?=[a-z]*a)b",
            r"[a-z]+(x+)+z",
            r"a*bc",
            r"(\w+)@([\w-]+\.\w+)",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"private int id = 1;",
            b"  private red green;",
            b"   record Order",
            b"recordX record Order;",
            b"@interface Api",
            b"public class Order",
            b"enum Status",
            b"aba",
            b"aaaa bc",
            b"aa xxxxx y",
            b"user@example.org",
            "é abc record 名".as_bytes(),
            b"a\xc3\xa9 abc",
            b"aa\xffbbb record R",
            b"a\xe2\x82 record R",
        ];
        let limits = [
            (0, 0, 0),
            (1, 0, 0),
            (2, 0, 0),
            (3, 0, 0),
            (4, 0, 0),
            (8, 0, 0),
            (16, 0, 0),
            (32, 0, 0),
            (64, 0, 0),
            (10_000_000, 0, 0),
            (0, 2, 0),
            (0, 8, 0),
            (0, 0, 2),
            (0, 0, 8),
        ];
        let outcome = |set: &mut OnigRegSet, input: &[u8], end, start, range, option, fast| {
            let found = if fast {
                onig_regset_search_fast_with_id(
                    set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    option,
                    FallbackMemoIdentity::Caller(71),
                )
            } else {
                onig_regset_search(
                    set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    option,
                )
            };
            let region = if found.0 >= 0 {
                let region = onig_regset_get_region(set, found.0 as usize).unwrap();
                (region.beg.clone(), region.end.clone())
            } else {
                (Vec::new(), Vec::new())
            };
            (found, onig_regset_last_match_len(set), region)
        };
        let mut checked = 0;
        for pattern in patterns {
            for &input in inputs {
                for end in [input.len(), input.len().saturating_sub(1)] {
                    for (retry, search_retry, stack) in limits {
                        onig_set_retry_limit_in_match(retry);
                        onig_set_retry_limit_in_search(search_retry);
                        onig_set_match_stack_limit(stack);
                        for fast in [false, true] {
                            let (mut optimized, status) = onig_regset_new(vec![
                                compile(pattern.as_bytes()),
                                compile(b"(id|green|Order|b)"),
                            ]);
                            assert_eq!(status, ONIG_NORMAL);
                            let (mut reference, status) = onig_regset_new(vec![
                                compile(pattern.as_bytes()),
                                compile(b"(id|green|Order|b)"),
                            ]);
                            assert_eq!(status, ONIG_NORMAL);
                            let optimized = optimized.as_mut().unwrap();
                            let reference = reference.as_mut().unwrap();
                            for entry in &mut reference.entries {
                                reg_mut(entry).leading_run = None;
                            }
                            for start in 0..=end {
                                for range in [start, (start + 2).min(end), end] {
                                    for option in [
                                        ONIG_OPTION_NONE,
                                        ONIG_OPTION_FIND_LONGEST,
                                        ONIG_OPTION_FIND_NOT_EMPTY,
                                    ] {
                                        assert_eq!(
                                            outcome(
                                                optimized, input, end, start, range, option, fast
                                            ),
                                            outcome(
                                                reference, input, end, start, range, option, fast
                                            ),
                                            "{pattern} {input:?} end={end} start={start} range={range} option={option:?} fast={fast} retry={retry} search_retry={search_retry} stack={stack}"
                                        );
                                        checked += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!("Compared {checked} RegSet search outcomes, including captures and limit errors");
        assert!(checked > 100_000, "{checked}");
    }

    /// Fallback searches with and without the required-literal filter. The
    /// filter leaves out attempts that cannot match; with a retry limit in
    /// match, such an attempt may still have stopped at the limit, which
    /// only the reference then reports (ADR-008's deliberate difference).
    #[test]
    fn required_literals_preserve_winners_captures_bounds_and_limits() {
        let _limits = exclusive_limits();
        struct RestoreLimits(u64, u64, u32, u64, u64);
        impl Drop for RestoreLimits {
            fn drop(&mut self) {
                onig_set_retry_limit_in_match(self.0);
                onig_set_retry_limit_in_search(self.1);
                onig_set_match_stack_limit(self.2);
                onig_set_time_limit(self.3);
                crate::regexec::onig_set_subexp_call_limit_in_search(self.4);
            }
        }
        let _restore = RestoreLimits(
            onig_get_retry_limit_in_match(),
            onig_get_retry_limit_in_search(),
            onig_get_match_stack_limit(),
            onig_get_time_limit(),
            crate::regexec::onig_get_subexp_call_limit_in_search(),
        );
        onig_set_time_limit(0);
        // Each pattern is a fallback entry with required literals.
        let patterns = [
            // The optimizer's exact string.
            r"\s*(\{)",
            r"\s*é(\w?)",
            // Literal tries and their union.
            r"(?:\s+|^)(?:foo|bar)_x(\w*)",
            r"(?:\s+|(?<=\W)|^)((?<!\w)(?:reinterpret|dynamic|static|const)_cast(?!\w))",
            // A positive look-ahead.
            r"(?:\s+|^)\w*(?=\s*\{)",
            // A class of one-byte literals.
            r"(?:\s+|^)[{(]x?",
            // Exact alternatives of a case-insensitive string.
            r"(?:\s+|^)(?i:st)\w*",
            // A recursive call: its group's own literals.
            r"(?:\s+|^)(?<p>\((?:[^()]|\g<p>)*\))",
            // A back reference does not count, its group does.
            r"(?:\s+|^)(ab)\w*\1",
            // Nor does the text of a look-behind or a negative look-ahead.
            r"(?:\s+|^|(?<=b))(?<=ab)(?:c|d)",
            r"(?:\s+|^|(?<=b))(?!ab)(?:c|d)",
            // Backtracks a lot before failing (the retry-limit example).
            r"(?:\s+|^)(?:a*a?)*x",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"{",
            b"  {x",
            b"foo_x bar_xy foo_",
            b" static_cast<int>(x); dynamic_cast",
            b"abcd { efgh",
            "\u{17f}t ST \u{fb06}x sT".as_bytes(),
            b"a(b(c)d)e) f(",
            b"ab abxab ab",
            b"abc ab c abc",
            b"\xc3\xa9x \xc3\xa9\xc3",
            b"\xff{\xc3( \xc3\xa9",
            b"aaaaaaaaaaaa",
            b"aaaaaaaaaa x a",
            b"no literal here at all",
        ];
        // Position checks that leave the position alone: `\G`, and
        // look-behinds of variable length without a trailing literal to
        // check, in front of, behind and around the required literal.
        let position_patterns = [
            r"\G\s*(\{)",
            r"(?:\s+|^|\G)(?:foo|bar)_x(\w*)",
            r"\s*(?<=(?:\W|^)(?:ab|cd))(x\w*)",
            r"\s*(?<!(?:\W|^)(?:ab|cd))(x\w*)",
            r"\s*(x\w*)(?<=(?:\W|^)(?:ab|cd)x\w*)",
            r"\s*(x)(?<!(?:\W|^)(?:ax|bx))",
            r"\s*(?<=(?:\W|^)(?:é|ab))(x\w*)(?<!(?:\W|^)(?:yx|zx))",
        ];
        // Well-formed, multibyte, stray trailing bytes, F4 and F5 leads.
        let position_inputs: &[&[u8]] = &[
            b"",
            b"abx foo_x{ cdxx {",
            b"yx zx ax bx abx",
            "\u{e9}x abx \u{e9}{ zx".as_bytes(),
            b"ab\xa9x cd\x80\x80x {\xa9 x",
            b"\xf4\x8f\xbf\xbfx ab\xf5\x80\x80\x80x",
        ];
        // Retry limit in match, search retry budget, match stack limit,
        // calls per search.
        let limits = [
            (0, 0, 0, 0),
            (1, 0, 0, 0),
            (2, 0, 0, 0),
            (8, 0, 0, 0),
            (64, 0, 0, 0),
            (10_000_000, 0, 0, 0),
            (0, 2, 0, 0),
            (0, 8, 0, 0),
            (0, 0, 2, 0),
            (0, 0, 8, 0),
            (0, 0, 0, 2),
        ];
        let outcome = |set: &mut OnigRegSet, input: &[u8], end, start, range, option, fast| {
            let found = if fast {
                onig_regset_search_fast_with_id(
                    set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    option,
                    FallbackMemoIdentity::Caller(73),
                )
            } else {
                onig_regset_search(
                    set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    option,
                )
            };
            let region = if found.0 >= 0 {
                let region = onig_regset_get_region(set, found.0 as usize).unwrap();
                (region.beg.clone(), region.end.clone())
            } else {
                (Vec::new(), Vec::new())
            };
            (found, onig_regset_last_match_len(set), region)
        };
        let sets = |pattern: &str| {
            let build = || {
                let (set, status) = onig_regset_new(vec![
                    compile(pattern.as_bytes()),
                    compile(b"(id|green|Order|b)"),
                ]);
                assert_eq!(status, ONIG_NORMAL);
                set.unwrap()
            };
            let optimized = build();
            assert_eq!(fallback_indices(&optimized), [0], "{pattern}");
            let entry = &optimized.entries[0];
            assert!(
                entry.required_literals_safe && entry.reg.required_literals.is_some(),
                "{pattern}"
            );
            let mut reference = build();
            reg_mut(&mut reference.entries[0]).required_literals = None;
            (optimized, reference, build())
        };
        let (mut checked, mut limit_differences) = (0, 0);
        let cases = (patterns.iter().map(|&pattern| (pattern, inputs))).chain(
            position_patterns
                .iter()
                .map(|&pattern| (pattern, position_inputs)),
        );
        for (pattern, inputs) in cases {
            for &input in inputs {
                for end in [input.len(), input.len().saturating_sub(1)] {
                    for (retry, search_retry, stack, calls) in limits {
                        onig_set_retry_limit_in_match(retry);
                        onig_set_retry_limit_in_search(search_retry);
                        onig_set_match_stack_limit(stack);
                        crate::regexec::onig_set_subexp_call_limit_in_search(calls);
                        for fast in [false, true] {
                            let (mut optimized, mut reference, mut unlimited) = sets(pattern);
                            for start in 0..=end {
                                for range in [start, (start + 2).min(end), end] {
                                    for option in [
                                        ONIG_OPTION_NONE,
                                        ONIG_OPTION_FIND_NOT_EMPTY,
                                        ONIG_OPTION_NOTBOL,
                                    ] {
                                        let expected = outcome(
                                            &mut reference,
                                            input,
                                            end,
                                            start,
                                            range,
                                            option,
                                            fast,
                                        );
                                        let actual = outcome(
                                            &mut optimized,
                                            input,
                                            end,
                                            start,
                                            range,
                                            option,
                                            fast,
                                        );
                                        if actual != expected
                                            && retry != 0
                                            && expected.0.0 == ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
                                        {
                                            // Without the left-out attempts,
                                            // a match or no match is what a
                                            // search without the limit
                                            // finds.
                                            onig_set_retry_limit_in_match(0);
                                            let unlimited = outcome(
                                                &mut unlimited,
                                                input,
                                                end,
                                                start,
                                                range,
                                                option,
                                                false,
                                            );
                                            onig_set_retry_limit_in_match(retry);
                                            assert_eq!(
                                                actual, unlimited,
                                                "{pattern} {input:?} end={end} start={start} range={range} option={option:?} fast={fast} retry={retry}"
                                            );
                                            limit_differences += 1;
                                            continue;
                                        }
                                        assert_eq!(
                                            actual, expected,
                                            "{pattern} {input:?} end={end} start={start} range={range} option={option:?} fast={fast} retry={retry} search_retry={search_retry} stack={stack} calls={calls}"
                                        );
                                        checked += 1;
                                    }
                                }
                            }
                            // One regex on its own, as the scanner's cache
                            // route asks it.
                            for start in 0..=end {
                                for stop in [start, end] {
                                    let event = |set: &mut OnigRegSet| {
                                        let event = onig_regset_entry_search(
                                            set,
                                            0,
                                            input,
                                            end,
                                            start,
                                            stop,
                                            ONIG_OPTION_NONE,
                                            false,
                                        );
                                        let region = onig_regset_get_region(set, 0).unwrap();
                                        (event, region.beg.clone(), region.end.clone())
                                    };
                                    let expected = event(&mut reference);
                                    let actual = event(&mut optimized);
                                    if actual.0 != expected.0
                                        && retry != 0
                                        && matches!(
                                            expected.0,
                                            RegSetEntryEvent::Error {
                                                code: ONIGERR_RETRY_LIMIT_IN_MATCH_OVER,
                                                ..
                                            }
                                        )
                                    {
                                        match actual.0 {
                                            RegSetEntryEvent::Error { position, code } => {
                                                // The limit, at a later
                                                // attempt.
                                                assert_eq!(code, ONIGERR_RETRY_LIMIT_IN_MATCH_OVER);
                                                assert!(matches!(
                                                    expected.0,
                                                    RegSetEntryEvent::Error { position: first, .. }
                                                        if first < position
                                                ));
                                            }
                                            _ => {
                                                onig_set_retry_limit_in_match(0);
                                                let unlimited = event(&mut unlimited);
                                                onig_set_retry_limit_in_match(retry);
                                                assert_eq!(
                                                    actual.0, unlimited.0,
                                                    "{pattern} {input:?} end={end} start={start} stop={stop} retry={retry}"
                                                );
                                                if matches!(
                                                    actual.0,
                                                    RegSetEntryEvent::Match { .. }
                                                ) {
                                                    assert_eq!(actual, unlimited);
                                                }
                                            }
                                        }
                                        limit_differences += 1;
                                        continue;
                                    }
                                    assert_eq!(
                                        actual.0, expected.0,
                                        "{pattern} {input:?} end={end} start={start} stop={stop} retry={retry} search_retry={search_retry} stack={stack} calls={calls}"
                                    );
                                    if matches!(actual.0, RegSetEntryEvent::Match { .. }) {
                                        assert_eq!(actual, expected);
                                    }
                                    checked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!(
            "Compared {checked} RegSet outcomes with and without required literals; \
             {limit_differences} retry-limit errors left out"
        );
        assert!(checked > 100_000, "{checked}");
        // The retry-limit example does leave out failing attempts that
        // reach the limit.
        assert!(limit_differences > 0);
    }

    /// Entries whose look-behind checks a trailing literal (`OpCode::Move`)
    /// keep the attempts their required literals would rule out (ADR-008).
    /// A variable-length look-behind ending in a literal checks that literal
    /// first: it steps back as many characters as the literal has, matches
    /// its bytes forward and goes on where they end. `\x{140000}` encodes
    /// as `F5 80 80 80`, four one-byte characters to the encoding's length
    /// table, but stepping back passes all four bytes at once: from 10 it
    /// stops at 9, 8, 7 and 3, the literal matches 3..7, and the attempt at
    /// 10 matches `ABC` at 7..10 without an occurrence from 10 on. C rejects
    /// such strings (`USE_CHECK_VALIDITY_OF_STRING_IN_TREE`); for the
    /// strings it accepts the check ends no earlier than the trailing bytes
    /// before the look-behind's position, where no literal starts.
    #[test]
    fn required_literals_keep_attempts_of_entries_with_position_checks() {
        let _limits = shared_limits();
        let pattern = br"\s*(?<=y*\x{140000})ABC";
        let subject = b"yyy\xf5\x80\x80\x80ABCD";
        let end = subject.len();
        let reg = compile(pattern);
        let required = reg.required_literals.as_deref().unwrap();
        assert_eq!(required.literal_list(), [b"ABC".to_vec()]);
        assert_eq!(
            onig_search(&reg, subject, end, 0, end, None, ONIG_OPTION_NONE).0,
            10
        );
        assert_eq!(required.find(subject, 10, end), None);

        let (set, status) = onig_regset_new(vec![compile(pattern), compile(b"(id|green|Order|b)")]);
        assert_eq!(status, ONIG_NORMAL);
        let mut set = set.unwrap();
        assert_eq!(fallback_indices(&set), [0]);
        let msa = MatchArg::new(&set.entries[0].reg, ONIG_OPTION_NONE, None, 0);
        assert!(required_literals(&set.entries[0], ONIG_OPTION_NONE, &msa).is_none());
        for start in 0..=7 {
            assert_eq!(
                onig_regset_search(
                    &mut set,
                    subject,
                    end,
                    start,
                    end,
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ),
                (0, 10),
                "start={start}"
            );
            assert_eq!(
                onig_regset_search_fast_with_id(
                    &mut set,
                    subject,
                    end,
                    start,
                    end,
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                    FallbackMemoIdentity::Caller(181),
                ),
                (0, 10),
                "start={start}"
            );
            assert_eq!(
                onig_regset_entry_search(
                    &mut set,
                    0,
                    subject,
                    end,
                    start,
                    end,
                    ONIG_OPTION_NONE,
                    false
                ),
                RegSetEntryEvent::Match { position: 10 },
                "start={start}"
            );
        }
    }

    /// The required-literal gate is set when an entry is added or replaced.
    /// Callouts (which also give no set) and a look-behind that checks a
    /// trailing literal close it, a negative one too although it returns to
    /// its position; `\G` and look-behinds without the check leave it open
    /// but keep the entry out of the memo.
    #[test]
    fn required_literals_gate_closes_for_callouts_and_look_behind_leads() {
        let _limits = shared_limits();
        let (set, status) = onig_regset_new(vec![compile(br"\s*(\{)")]);
        assert_eq!(status, ONIG_NORMAL);
        let mut set = set.unwrap();
        for (pattern, filtered, memo_safe) in [
            (&br"\s*(\{)(?{x})"[..], false, false),
            (br"\G\s*(\{)", true, false),
            (br"\s*(?<!(?:\W|^)(?:ab|cd))(x\w*)", true, false),
            (br"\s*(?<=(?:\W|^)return)\s*(\[)", false, false),
            (br"\s*(?<!(?:\W|^)return)\s*(\[)", false, false),
            (br"\s*(\{)", true, true),
        ] {
            assert_eq!(
                onig_regset_replace(&mut set, 0, Some(compile(pattern))),
                ONIG_NORMAL
            );
            let entry = &set.entries[0];
            let msa = MatchArg::new(&entry.reg, ONIG_OPTION_NONE, None, 0);
            let pattern = String::from_utf8_lossy(pattern);
            assert_eq!(
                entry.reg.required_literals.is_some(),
                !entry.has_callouts,
                "{pattern}"
            );
            assert_eq!(entry.required_literals_safe, filtered, "{pattern}");
            assert_eq!(
                required_literals(entry, ONIG_OPTION_NONE, &msa).is_some(),
                filtered,
                "{pattern}"
            );
            assert_eq!(entry.fallback_memo_safe, memo_safe, "{pattern}");
        }
    }

    /// A "no event" that the required literals decide for an entry the memo
    /// leaves out stays out of the memo, as that of a full search would.
    #[test]
    fn required_literals_no_event_of_memo_unsafe_entry_is_not_memoized() {
        let _limits = shared_limits();
        // `{` at 2 fails; past it no occurrence is left.
        let input = b"ab{ cd x";
        for (pattern, memo_safe) in [
            (&br"(?:\s+|^)(?<!(?:\W|^)(?:ab|cd))(\{)"[..], false),
            (br"(?:\s+|^)(\{)", true),
        ] {
            let (set, status) = onig_regset_new(vec![compile(pattern)]);
            assert_eq!(status, ONIG_NORMAL);
            let mut set = set.unwrap();
            assert_eq!(fallback_indices(&set), [0]);
            let msa = MatchArg::new(&set.entries[0].reg, ONIG_OPTION_NONE, None, 0);
            assert!(required_literals(&set.entries[0], ONIG_OPTION_NONE, &msa).is_some());
            assert_eq!(set.entries[0].fallback_memo_safe, memo_safe);
            for start in [0, 3] {
                assert_eq!(
                    onig_regset_search_fast_with_id(
                        &mut set,
                        input,
                        input.len(),
                        start,
                        input.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                        FallbackMemoIdentity::OnigString(181),
                    ),
                    (ONIG_MISMATCH, 0)
                );
            }
            if memo_safe {
                assert_settled_from(&set, 0);
            } else {
                let candidate = set.fallback_search_candidates[0];
                assert!(set.fallback_memos[0].is_empty());
                assert_eq!(
                    [
                        candidate.no_match_from,
                        candidate.exact_miss,
                        candidate.match_from,
                        candidate.match_at
                    ],
                    [usize::MAX; 4]
                );
            }
        }
    }

    /// Generated combinations of `\G` and variable-length look-behinds,
    /// with and without a trailing literal to check, in front of and behind
    /// required literals, over subjects with stray trailing bytes and F4 or
    /// F5 sequences: every search agrees with the one without the filter.
    /// (`\x{140000}` checks are left to
    /// `required_literals_keep_attempts_of_entries_with_position_checks`:
    /// their attempts can end before they start.)
    #[test]
    fn required_literals_agree_on_generated_position_checks() {
        let _limits = shared_limits();
        let subjects: &[&[u8]] = &[
            b"A\xa9B yA\xa9\xa9C",
            "y\u{e9}\u{a9}B ab\u{e9}C".as_bytes(),
            b"ab\xa9AB y\xf4\x8f\xbf\xbfB",
            b"yC \xf5\x80\x80AB\xa9",
            b"abAB yAB C\tB",
            // Back-to-back occurrences: the first fails behind `ab`.
            b"abABAB abBB",
            b"",
        ];
        let (mut filtered, mut kept) = (0, 0);
        for prefix in [r"\s*", r"(?:\s+|\G)"] {
            for behind in [
                "",
                r"(?<=(?:\W|^)(?:ab|é))",
                r"(?<!(?:\W|^)(?:ab|é))",
                r"(?<=y*A)",
                r"(?<!y*A)",
            ] {
                for middle in ["", "."] {
                    for literal in ["B", "(?:AB|é)C?"] {
                        for after in ["", r"(?<=(?:\W|^)(?:yB|C))", r"(?<!y*B)"] {
                            let pattern = format!("{prefix}{behind}{middle}{literal}{after}");
                            let build = || {
                                let (set, status) =
                                    onig_regset_new(vec![compile(pattern.as_bytes())]);
                                assert_eq!(status, ONIG_NORMAL);
                                set.unwrap()
                            };
                            let mut optimized = build();
                            let entry = &optimized.entries[0];
                            if fallback_indices(&optimized) != [0]
                                || entry.reg.required_literals.is_none()
                            {
                                continue;
                            }
                            if entry.required_literals_safe {
                                filtered += 1;
                            } else {
                                kept += 1;
                            }
                            let mut reference = build();
                            reg_mut(&mut reference.entries[0]).required_literals = None;
                            for (id, &subject) in (0..).zip(subjects) {
                                let end = subject.len();
                                // Plain searches, then memoized ones with
                                // advancing starts, as a scanner makes them.
                                for fast in [false, true] {
                                    for start in 0..=end {
                                        let outcome = |set: &mut OnigRegSet| {
                                            let found = if fast {
                                                onig_regset_search_fast_with_id(
                                                    set,
                                                    subject,
                                                    end,
                                                    start,
                                                    end,
                                                    OnigRegSetLead::PositionLead,
                                                    ONIG_OPTION_NONE,
                                                    FallbackMemoIdentity::Caller(id),
                                                )
                                            } else {
                                                onig_regset_search(
                                                    set,
                                                    subject,
                                                    end,
                                                    start,
                                                    end,
                                                    OnigRegSetLead::PositionLead,
                                                    ONIG_OPTION_NONE,
                                                )
                                            };
                                            let region = onig_regset_get_region(set, 0).unwrap();
                                            (found, (found.0 == 0).then(|| region.beg.clone()))
                                        };
                                        assert_eq!(
                                            outcome(&mut optimized),
                                            outcome(&mut reference),
                                            "{pattern} {subject:?} start={start} fast={fast}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Lead checks keep their attempts; the rest are filtered.
        assert_eq!((filtered, kept), (48, 72));
    }

    /// Attempts left out because their first instruction fails
    /// (`first_op_rejects`) change no result, winner, capture or limit
    /// error: every search agrees with the same set without the tests, in
    /// the table scan and in the fallback searches. Limits that could
    /// observe the left-out retries, and FIND_LONGEST, turn them off.
    #[test]
    fn first_op_rejects_preserve_winners_captures_and_limits() {
        let _limits = exclusive_limits();
        struct RestoreLimits(u64, u64, u32, u64);
        impl Drop for RestoreLimits {
            fn drop(&mut self) {
                onig_set_retry_limit_in_match(self.0);
                onig_set_retry_limit_in_search(self.1);
                onig_set_match_stack_limit(self.2);
                onig_set_time_limit(self.3);
            }
        }
        let _restore = RestoreLimits(
            onig_get_retry_limit_in_match(),
            onig_get_retry_limit_in_search(),
            onig_get_match_stack_limit(),
            onig_get_time_limit(),
        );
        // (pattern, dispatched by the table rather than searched on its own)
        let patterns: [(&str, bool); 13] = [
            (r"\b(?:if|for|while)\b", true),
            (r"\b\w*\(", false),
            (r"^\s*#\s*(\w+)", false),
            (r"^[a-z]+:", true),
            (r"(?<=\.)(\w+)", true),
            (r"(?<![\w$])\d+(?:\.\d+)?", true),
            (r"(?<=[=:])\s*(\w*);", false),
            (r"\B\w*-", false),
            (r"(?W)\b[a-zé]+", true),
            (r"(?<=é)a", true),
            (r"\b", true),
            (r"^", true),
            (r"(?<!a)", true),
        ];
        let mut sets: Vec<Vec<&str>> = patterns.iter().map(|&(p, _)| vec![p]).collect();
        let mut mixed: Vec<&str> = patterns[..10].iter().map(|&(p, _)| p).collect();
        mixed.insert(4, "(id|b)");
        sets.push(mixed);
        let inputs: &[&[u8]] = &[
            b"",
            b"if (x) for",
            b"  #  include <a>\n#define B",
            b"a.b = 1.5; c: d;",
            b"x -a- -b-c d-e-",
            "é aé \u{e9}a 名.x".as_bytes(),
            b"aa\xffbbb if(",
            b"a\xe2\x82 while (",
            b"\n\nab:\n cd:",
            b"$1 a1 2.3 id",
        ];
        // (retry in match, search retry budget, stack, time, skips apply)
        let limits = [
            (0, 0, 0, 0, true),
            (2, 0, 0, 0, true),
            (10_000_000, 0, 0, 0, true),
            (1, 0, 0, 0, false),
            (0, 1, 0, 0, false),
            (0, 6, 0, 0, false),
            (0, 0, 1, 0, false),
            (0, 0, 6, 0, false),
            (0, 0, 0, 1_000_000, false),
        ];
        let rejects = || FIRST_OP_REJECTS.with(|rejects| rejects.get());
        let outcome = |set: &mut OnigRegSet, input: &[u8], id, end, start, range, option, mode| {
            let found = match mode {
                0 => onig_regset_search(
                    set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    option,
                ),
                1 => onig_regset_search_fast(
                    set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    option,
                ),
                _ => onig_regset_search_fast_with_id(
                    set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    option,
                    FallbackMemoIdentity::Caller(id),
                ),
            };
            let region = if found.0 >= 0 {
                let region = onig_regset_get_region(set, found.0 as usize).unwrap();
                (region.beg.clone(), region.end.clone())
            } else {
                (Vec::new(), Vec::new())
            };
            (found, onig_regset_last_match_len(set), region)
        };
        let mut checked = 0;
        let mut rejected_by_set = vec![0u64; sets.len()];
        for (retry, search_retry, stack, time, skips) in limits {
            onig_set_retry_limit_in_match(retry);
            onig_set_retry_limit_in_search(search_retry);
            onig_set_match_stack_limit(stack);
            onig_set_time_limit(time);
            for (set_at, set_patterns) in sets.iter().enumerate() {
                let build = || {
                    let regs = set_patterns.iter().map(|p| compile(p.as_bytes())).collect();
                    let (set, status) = onig_regset_new(regs);
                    assert_eq!(status, ONIG_NORMAL);
                    set.unwrap()
                };
                let mut optimized = build();
                let mut reference = build();
                for entry in &mut reference.entries {
                    entry.first_op = None;
                }
                // The single-pattern sets come first, in pattern order.
                if let Some(&(pattern, table)) = patterns.get(set_at) {
                    assert!(optimized.entries[0].first_op.is_some(), "{pattern}");
                    assert_eq!(optimized.entries[0].fallback, !table, "{pattern}");
                }
                for mode in 0..3 {
                    for (id, &input) in inputs.iter().enumerate() {
                        for end in [input.len(), input.len().saturating_sub(1)] {
                            for option in [
                                ONIG_OPTION_NONE,
                                ONIG_OPTION_FIND_NOT_EMPTY,
                                ONIG_OPTION_NOTBOL,
                                ONIG_OPTION_FIND_LONGEST,
                            ] {
                                for start in 0..=end {
                                    for range in [start, (start + 2).min(end), end] {
                                        let before = rejects();
                                        let expected = outcome(
                                            &mut reference,
                                            input,
                                            id as u64,
                                            end,
                                            start,
                                            range,
                                            option,
                                            mode,
                                        );
                                        assert_eq!(rejects(), before);
                                        let found = outcome(
                                            &mut optimized,
                                            input,
                                            id as u64,
                                            end,
                                            start,
                                            range,
                                            option,
                                            mode,
                                        );
                                        let rejected = rejects() - before;
                                        let context = format!(
                                            "{set_patterns:?} {input:?} end={end} start={start} range={range} option={option:?} mode={mode} limits={:?}",
                                            (retry, search_retry, stack, time)
                                        );
                                        assert_eq!(found, expected, "{context}");
                                        if !skips || option == ONIG_OPTION_FIND_LONGEST {
                                            assert_eq!(rejected, 0, "{context}");
                                        }
                                        rejected_by_set[set_at] += rejected;
                                        checked += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Every pattern's test rejected attempts, in the table scan and in
        // the fallback searches alike.
        for (set_patterns, rejected) in sets.iter().zip(&rejected_by_set) {
            assert!(*rejected > 0, "{set_patterns:?}");
        }
        eprintln!(
            "Compared {checked} RegSet search outcomes; {} attempts left out",
            rejected_by_set.iter().sum::<u64>()
        );
        assert!(checked > 100_000, "{checked}");
    }

    /// Which committed grammar patterns get a first-instruction test: word
    /// boundaries, line starts and look-behinds, among the table and the
    /// fallback entries. Like `grammar_fast_path_census`, this catches an
    /// eligibility check that quietly turns the tests off.
    #[test]
    fn grammar_first_op_test_census() {
        let census = |patterns: Vec<String>| {
            let regs = patterns.iter().map(|p| compile(p.as_bytes())).collect();
            let (set, r) = onig_regset_new(regs);
            assert_eq!(r, ONIG_NORMAL);
            let set = set.unwrap();
            let mut counts = [[0; 3]; 2];
            for entry in &set.entries {
                let kind = match entry.first_op {
                    None => continue,
                    Some(FirstOpTest::WordBoundary { .. }) => 0,
                    Some(FirstOpTest::BeginLine) => 1,
                    Some(FirstOpTest::LookBehind { .. }) => 2,
                };
                counts[usize::from(entry.fallback)][kind] += 1;
            }
            counts
        };
        // [table, fallback] x [word boundary, line start, look-behind]
        assert_eq!(
            census(grammar_loader::typescript_patterns()),
            [[5, 2, 73], [0, 0, 43]]
        );
        assert_eq!(
            census(grammar_loader::css_patterns()),
            [[0, 0, 34], [0, 0, 1]]
        );
        assert_eq!(
            census(grammar_loader::rust_patterns()),
            [[36, 0, 3], [0, 0, 1]]
        );
    }

    #[test]
    fn unbounded_optimizer_stays_on_the_search_fallback() {
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let set = set.expect("regset");

        assert_eq!(fallback_indices(&set), [0]);
        assert_eq!(set.table_entry_count, 0);
    }

    #[test]
    fn fallback_search_fills_backtracked_push_captures() {
        let _limits = shared_limits();
        // The fallback search runs region-free and fills the region only for
        // the winning position; captures restored by backtracking must still
        // come out unset.
        let (set, result) = onig_regset_new(vec![compile(b"(?:(a)|b)*ab")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(fallback_indices(&set), [0]);

        // (input, group 1 start, group 1 end)
        let cases: [(&[u8], i32, i32); 2] = [
            (b"xbab", ONIG_REGION_NOTPOS, ONIG_REGION_NOTPOS),
            (b"xaab", 1, 2),
        ];
        for (input, g1_beg, g1_end) in cases {
            let (index, position) = onig_regset_search(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            );
            assert_eq!((index, position), (0, 1));
            let region = onig_regset_get_region(&set, 0).expect("region");
            assert_eq!((region.beg[0], region.end[0]), (1, 4));
            assert_eq!((region.beg[1], region.end[1]), (g1_beg, g1_end));
        }
    }

    #[test]
    fn fallback_start_filter_only_skips_impossible_starts() {
        let _limits = shared_limits();
        // `\s*` in front of `[` leaves the optimizer an unbounded distance,
        // so the entry stays on the fallback search, filtered by start byte.
        let patterns: [&[u8]; 2] = [b"\\s*(\\[)", b"x"];
        let (set, result) = onig_regset_new(patterns.iter().map(|p| compile(p)).collect());
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(fallback_indices(&set), [0]);
        let filter = set.entries[0].start_filter.as_deref().expect("filter");
        assert!(filter[b' ' as usize] != 0 && filter[b'[' as usize] != 0);
        assert_eq!(filter[b'a' as usize], 0);

        // Callouts observe every attempt, so they keep the unfiltered search.
        assert!(fallback_start_filter(&compile(b"\\s*(*COUNT)\\[")).is_none());

        let input = b"ab  [c \xc3\xa9[ x [";
        for start in 0..=input.len() {
            // Position-lead: earliest start, ties to the lower index.
            let expected = patterns
                .iter()
                .enumerate()
                .filter_map(|(index, pattern)| {
                    let reg = compile(pattern);
                    let (pos, _) = onig_search(
                        &reg,
                        input,
                        input.len(),
                        start,
                        input.len(),
                        None,
                        ONIG_OPTION_NONE,
                    );
                    (pos >= 0).then_some((pos, index as i32))
                })
                .min()
                .map_or((ONIG_MISMATCH, 0), |(pos, index)| (index, pos));
            let found = onig_regset_search(
                &mut set,
                input,
                input.len(),
                start,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            );
            assert_eq!(found, expected, "start {start}");
        }
    }

    /// A leading positive look-behind steps back over a stray run of
    /// continuation bytes and goes on before the start, as in C (#259):
    /// the dispatch table, fallback start filters and the scan's skips still
    /// attempt such a start, so the set finds C's winner.
    #[cfg(feature = "ffi")]
    #[test]
    fn look_behind_entries_keep_c_s_winners_over_malformed_utf8() {
        let _limits = shared_limits();
        let sets: [&[&[u8]]; 10] = [
            &[b"(?<=\\()\\W.*z", b"q"],
            &[b"(?<=[(\\[])\\W.*z", b"q"],
            &[b"(?<=[(\\[])[^a-z0-9 ].*z", b"q"],
            &[b"(?<=[(\\[])\\W", b"zz"],
            &[b"(?<=\\()\\W", b"zz"],
            &[b"(?<=\\(|\\*/)\\W?x", b"y"],
            &[b"(?i)(?<=^|[(\\s]|\\*/)(?:width|\\W)", b"\\w+"],
            &[b"(?<=\\.)\\W\\w*", b"[a-z]+"],
            &[b"x(?:(?<=\\()\\W|y)", b"(?<=ab)\\W"],
            &[b"(?<=\\()[^)]", b"\\)", b"(?<=[(,])\\s*\\W"],
        ];
        let subjects: [&[u8]; 12] = [
            b"(\x80",
            b",\x85",
            b"(\x80y",
            b"(\x80xz",
            b"(\x80x",
            b"(\x80\x80\x80\x80x",
            b"a (\x80) b",
            b"*/\x85x (width",
            b".\x80\x80abc",
            b"x(\x80 ab\xbf)",
            "( é \u{80} ".as_bytes(),
            b"\xc3(\x80)\xa9,\x85",
        ];
        for patterns in sets {
            let regs: Vec<_> = patterns.iter().map(|p| compile(p)).collect();
            let (set, r) = onig_regset_new(regs);
            assert_eq!(r, ONIG_NORMAL);
            let mut set = set.unwrap();
            let c_regs: Vec<_> = patterns
                .iter()
                .map(|p| crate::ffi::CRegex::new(p, crate::ffi::ONIG_OPTION_NONE).unwrap())
                .collect();
            let raw: Vec<_> = c_regs.iter().map(|reg| reg.raw()).collect();
            let mut c_set = crate::ffi::CRegSet::new(&raw).unwrap();
            // The C set owns and frees its regexes.
            std::mem::forget(c_regs);
            for subject in subjects {
                for start in 0..=subject.len() {
                    let found = onig_regset_search(
                        &mut set,
                        subject,
                        subject.len(),
                        start,
                        subject.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                    );
                    let expected = c_set.search(subject, start, subject.len(), 0, 0);
                    let context = format!("{patterns:?} on {subject:?} from {start}");
                    assert_eq!(found.0, expected.0, "{context}");
                    if found.0 >= 0 {
                        assert_eq!(found.1, expected.1, "{context}");
                    }
                }
            }
        }
    }

    /// A fallback entry whose literal follows only zero-width checks and
    /// class repetitions (`LiteralPrefix`) attempts only the starts in the
    /// class run before each occurrence; the set's results stay those of
    /// the same set without the plan, for every start.
    #[test]
    fn literal_prefix_starts_keep_position_lead_results() {
        let _limits = shared_limits();
        let patterns: [&[u8]; 6] = [
            b"(?<!\\+\\+|--)(?<=[!(+,:=>?\\[]|^await|[^$._[:alnum:]]await|^return)\\s*(\\{)",
            b"(?:^|(?<=[&(,]|[;\\s]if\\s))\\s*((/))(?![*+?{}])",
            b"\\s*(;)",
            b"[ \\t]*//",
            b"zz",
            b"q\\w+",
        ];
        let regs = || patterns.iter().map(|p| compile(p)).collect::<Vec<_>>();
        let (set, result) = onig_regset_new(regs());
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(
            set.entries
                .iter()
                .take(4)
                .all(|entry| entry.reg.literal_prefix.is_some())
        );
        let (plain, _) = onig_regset_new(
            regs()
                .into_iter()
                .map(|mut reg| {
                    reg.literal_prefix = None;
                    reg
                })
                .collect(),
        );
        let mut plain = plain.expect("regset");
        let long = [
            &b"return"[..],
            &b" ".repeat(80),
            b"{ q1",
            &b" ".repeat(70),
            b"// zz",
            &b" ".repeat(90),
            b";",
        ]
        .concat();
        let inputs: [&[u8]; 6] = [
            b"return {a}; if /x/ , /y/ ;z // q1",
            b"x = ( { ; \xc2\xa0{ \t// zz",
            b"\xe0 { \xc3; \xff//x qq",
            b"await{ ++{ --{ ,{",
            b"",
            &long,
        ];
        for input in inputs {
            for start in 0..=input.len() {
                for range in [input.len(), start + (input.len() - start) / 2] {
                    let found = onig_regset_search(
                        &mut set,
                        input,
                        input.len(),
                        start,
                        range,
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                    );
                    let expected = onig_regset_search(
                        &mut plain,
                        input,
                        input.len(),
                        start,
                        range,
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                    );
                    assert_eq!(found, expected, "{input:?} start {start} range {range}");
                    assert_eq!(set.last_match_len, plain.last_match_len);
                }
            }
        }
    }

    /// A positive look-ahead whose body cannot match empty fixes the start
    /// byte, so zero-width grammar patterns such as `(?<=:)(?=\s*\{)` get a
    /// start filter. Nullable bodies and look-behinds fix nothing.
    #[test]
    fn start_map_reads_through_positive_lookaheads() {
        let map_of = |pattern: &[u8]| derive_start_byte_map(&compile(pattern));
        let members = |map: [u8; CHAR_MAP_SIZE]| -> Vec<u8> {
            (0..=255u8).filter(|&b| map[b as usize] != 0).collect()
        };

        let map = map_of(b"(?<=:)(?=\\s*\\{)").expect("look-ahead fixes the start");
        assert!(map[b' ' as usize] != 0 && map[b'\t' as usize] != 0 && map[b'{' as usize] != 0);
        assert_eq!(map[b':' as usize], 0);
        assert_eq!(map[b'a' as usize], 0);
        assert_eq!(members(map_of(b"(?=a|b)").unwrap()), b"ab");
        assert_eq!(members(map_of(b"(?=(?=ab)a)").unwrap()), b"a");
        assert_eq!(members(map_of(b"(?=a?)x").unwrap()), b"x");
        assert_eq!(members(map_of(b"(?>ab)").unwrap()), b"a");
        // The negative look-ahead's push is guarded by its body's first byte.
        assert_eq!(members(map_of(b"(?!x)b").unwrap()), b"b");
        for zero_width in [
            &b"(?=a?)"[..],
            b"(?=)",
            b"(?<=ab)",
            b"(?<=a|bc)",
            b"(?!a)",
            b"(?>a|)",
        ] {
            assert_eq!(
                map_of(zero_width),
                None,
                "{:?}",
                std::str::from_utf8(zero_width)
            );
        }
    }

    #[test]
    fn lookahead_start_filters_keep_regset_results() {
        let _limits = shared_limits();
        let patterns: [&[u8]; 5] = [
            b"(?<=:)(?=\\s*\\{)",
            b"(?=(\\w+)\\s*<)",
            b"(?=a?)b",
            b"(?<![a-z])(?=[a-z]+\\()",
            b"x",
        ];
        let (set, result) = onig_regset_new(patterns.iter().map(|p| compile(p)).collect());
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"a: {b} f(x) :  {\xc3\xa9< ab <c> x:{";
        for start in 0..=input.len() {
            let expected = patterns
                .iter()
                .enumerate()
                .filter_map(|(index, pattern)| {
                    let reg = compile(pattern);
                    let (pos, _) = onig_search(
                        &reg,
                        input,
                        input.len(),
                        start,
                        input.len(),
                        None,
                        ONIG_OPTION_NONE,
                    );
                    (pos >= 0).then_some((pos, index as i32))
                })
                .min()
                .map_or((ONIG_MISMATCH, 0), |(pos, index)| (index, pos));
            let found = onig_regset_search(
                &mut set,
                input,
                input.len(),
                start,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            );
            assert_eq!(found, expected, "start {start}");
        }
    }

    #[test]
    fn table_entry_count_tracks_mixed_add_remove_and_replace_sets() {
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.table_entry_count, 0);

        assert_eq!(onig_regset_add(&mut set, compile(b"x")), ONIG_NORMAL);
        assert_eq!(set.table_entry_count, 1);
        assert_eq!(
            onig_regset_replace(&mut set, 1, None),
            ONIG_NORMAL,
            "removing the sole table entry restores a pure fallback set"
        );
        assert_eq!(set.table_entry_count, 0);

        assert_eq!(
            onig_regset_replace(&mut set, 0, Some(compile(b"x"))),
            ONIG_NORMAL
        );
        assert_eq!(set.table_entry_count, 1);
        assert!(set.fallback_search_candidates.is_empty());

        assert_eq!(onig_regset_add(&mut set, compile(b"a*bc")), ONIG_NORMAL);
        assert_eq!(set.table_entry_count, 1);
        assert_eq!(fallback_indices(&set), [1]);
    }

    #[test]
    fn all_fallback_memo_hit_skips_the_empty_table_pass() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = vec![b'a'; 80_000];
        let identity = FallbackMemoIdentity::OnigString(11);

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert_settled_from(&set, 0);

        // A cached fallback miss needs neither a table MatchArg nor a
        // byte-by-byte table walk. Leaving this empty distinguishes the O(1)
        // bypass from the former empty-table scan.
        set.scratch_msa = None;
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert!(set.scratch_msa.is_none());
    }

    #[test]
    fn mixed_set_keeps_its_table_search() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.table_entry_count, 1);

        let input = b"x";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::OnigString(12),
            ),
            (1, 0)
        );
    }

    #[test]
    fn fallback_memo_preserves_progressing_match_regions_and_lengths() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"aabcxxbc";

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(41),
            ),
            (0, 0)
        );
        assert_eq!(onig_regset_last_match_len(&set), 4);
        let region = onig_regset_get_region(&set, 0).expect("first region");
        assert_eq!((region.beg[0], region.end[0]), (0, 4));

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                4,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(41),
            ),
            (0, 6)
        );
        assert_eq!(onig_regset_last_match_len(&set), 2);
        let region = onig_regset_get_region(&set, 0).expect("second region");
        assert_eq!((region.beg[0], region.end[0]), (6, 8));
        assert_eq!(set.fallback_memos[0].len(), 2);
    }

    #[test]
    fn fallback_memo_invalidates_ids_limits_and_regset_changes() {
        let _limits = exclusive_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"aaaa";
        let search = |set: &mut OnigRegSet, id| {
            onig_regset_search_fast_with_id(
                set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                id,
            )
        };

        assert_eq!(
            search(&mut set, FallbackMemoIdentity::Caller(42)),
            (ONIG_MISMATCH, 0)
        );
        assert_settled_from(&set, 0);
        let first_key = set.fallback_memo_key.expect("memo key");
        let first_revision = set
            .scratch_limits_revision
            .expect("first search captures the global-limit revision");
        assert_eq!(
            search(&mut set, FallbackMemoIdentity::Caller(43)),
            (ONIG_MISMATCH, 0)
        );
        assert_ne!(
            set.fallback_memo_key.expect("new ID key").identity,
            first_key.identity
        );
        assert!(set.scratch_msa.is_some());
        assert_eq!(
            set.scratch_limits,
            Some(FallbackMemoLimits::current()),
            "identity changes clear result memos, not reusable MatchArg buffers"
        );

        let old_retry_match = onig_get_retry_limit_in_match();
        let old_retry_search = onig_get_retry_limit_in_search();
        let old_stack = onig_get_match_stack_limit();
        let old_time = onig_get_time_limit();
        onig_set_retry_limit_in_match(old_retry_match.saturating_add(1));
        onig_set_match_stack_limit(old_stack.saturating_add(1));
        onig_set_time_limit(old_time.saturating_add(1));
        assert_ne!(
            onig_get_global_limit_revision(),
            first_revision,
            "every global limit setter invalidates cached MatchArg limits"
        );
        assert_eq!(
            search(&mut set, FallbackMemoIdentity::Caller(43)),
            (ONIG_MISMATCH, 0)
        );
        let changed_key = set.fallback_memo_key.expect("limit key");
        assert_ne!(
            changed_key.retry_limit_in_match,
            first_key.retry_limit_in_match
        );
        assert_ne!(changed_key.match_stack_limit, first_key.match_stack_limit);
        assert_ne!(changed_key.time_limit, first_key.time_limit);
        onig_set_retry_limit_in_match(old_retry_match);
        onig_set_match_stack_limit(old_stack);
        onig_set_time_limit(old_time);

        // A search retry budget makes a result depend on where its search
        // began, so such searches bypass the memo.
        onig_set_retry_limit_in_search(old_retry_search.saturating_add(1));
        let key_before = set.fallback_memo_key;
        assert_eq!(
            search(&mut set, FallbackMemoIdentity::Caller(44)),
            (ONIG_MISMATCH, 0)
        );
        assert!(set.fallback_memo_key == key_before);
        onig_set_retry_limit_in_search(old_retry_search);

        assert_eq!(onig_regset_add(&mut set, compile(b"x")), ONIG_NORMAL);
        assert!(set.fallback_memo_key.is_none());
        assert_eq!(set.fallback_memos.len(), 2);
        assert_eq!(onig_regset_replace(&mut set, 1, None), ONIG_NORMAL);
        assert!(set.fallback_memo_key.is_none());
        assert_eq!(set.fallback_memos.len(), 1);
    }

    #[test]
    fn fallback_memo_separates_caller_and_onig_string_id_domains() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");

        let no_match = b"aaaa";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                no_match,
                no_match.len(),
                0,
                no_match.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(1),
            ),
            (ONIG_MISMATCH, 0)
        );

        // The numeric values intentionally collide. Their sources do not:
        // caller-managed IDs must never poison an internally allocated
        // `OnigString` identity.
        let matching = b"aabc";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                matching,
                matching.len(),
                0,
                matching.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::OnigString(1),
            ),
            (0, 0)
        );
    }

    #[test]
    fn fallback_memo_rebuilds_match_arg_after_limit_change() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"aaaa";
        let identity = FallbackMemoIdentity::OnigString(7);

        onig_set_retry_limit_in_match(old_limit.saturating_add(1));
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert_eq!(
            set.scratch_msa
                .as_ref()
                .expect("first search leaves scratch state")
                .retry_limit_in_match,
            old_limit.saturating_add(1)
        );

        let lowered = old_limit.saturating_sub(1);
        onig_set_retry_limit_in_match(lowered);
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert_eq!(
            set.scratch_msa
                .as_ref()
                .expect("limit change rebuilds scratch state")
                .retry_limit_in_match,
            lowered
        );
        onig_set_retry_limit_in_match(old_limit);
    }

    #[test]
    fn fallback_memo_limit_change_matches_a_fresh_regset_error() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        let input = format!("x{}b", "a".repeat(1_001));
        let identity = FallbackMemoIdentity::OnigString(10);
        let new_set = || {
            let (set, result) = onig_regset_new(vec![compile(br".*x(a+)+b")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = new_set();

        onig_set_retry_limit_in_match(old_limit.max(1_000_000));
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut reused,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (0, 0)
        );

        onig_set_retry_limit_in_match(1);
        let reused_result = onig_regset_search_fast_with_id(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
            identity,
        );
        let mut fresh = new_set();
        let fresh_result = onig_regset_search_fast_with_id(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
            identity,
        );
        onig_set_retry_limit_in_match(old_limit);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn position_lead_refreshes_retry_limits_for_all_fallback_no_id_searches() {
        let _limits = exclusive_limits();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        let input = format!("{}bx", "a".repeat(1_001));
        let make = || {
            let (set, result) = onig_regset_new(vec![compile(br"(a*)\1b")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = make();

        onig_set_retry_limit_in_match(old_match.max(1_000_000));
        onig_set_retry_limit_in_search(0);
        let _ = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(100);
        let reused_result = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        let mut fresh = make();
        let fresh_result = onig_regset_search(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn table_position_lead_refreshes_retry_limits_and_reports_search_over() {
        let _limits = exclusive_limits();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        let input = format!("{}b", "a".repeat(32));
        let make = || {
            let (set, result) = onig_regset_new(vec![compile(br"(?:a|aa)*\z")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = make();
        assert_eq!(reused.table_entry_count, 1);

        onig_set_retry_limit_in_match(0);
        onig_set_retry_limit_in_search(1_000_000);
        let _ = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_search(100);
        let reused_result = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        let mut fresh = make();
        let fresh_result = onig_regset_search(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (ONIGERR_RETRY_LIMIT_IN_SEARCH_OVER, 0));
    }

    #[test]
    fn table_position_lead_skips_retry_scratch_when_search_limit_is_disabled() {
        let _limits = exclusive_limits();
        let old_search = onig_get_retry_limit_in_search();
        onig_set_retry_limit_in_search(0);

        let (set, result) = onig_regset_new(vec![compile(b"a")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(set.table_entry_count, 1);
        // A nonzero sentinel proves the default path neither initializes nor
        // clears the vector whose contents are irrelevant when the limit is 0.
        set.scratch_table_retry_counters[0] = 123;
        assert_eq!(
            onig_regset_search(
                &mut set,
                b"a",
                1,
                0,
                1,
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 0)
        );
        assert_eq!(set.scratch_table_retry_counters, vec![123]);

        onig_set_retry_limit_in_search(old_search);
    }

    #[test]
    fn table_retry_search_budget_is_isolated_per_regex() {
        let _limits = exclusive_limits();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        onig_set_retry_limit_in_match(0);
        // The smallest budget under which C's onig_search finishes this
        // search: it stops once the count reaches the limit.
        onig_set_retry_limit_in_search(137);

        let input = format!("{}c", "a".repeat(16));
        let upstream = compile(br"(a+)\1bc");
        let (upstream_result, _) = onig_search(
            &upstream,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            Some(OnigRegion::new()),
            ONIG_OPTION_NONE,
        );
        assert_eq!(upstream_result, ONIG_MISMATCH);
        for regs in [
            vec![compile(br"(a+)\1bc")],
            vec![compile(br"(a+)\1bc"), compile(br"(a+)\1bc")],
        ] {
            let (set, result) = onig_regset_new(regs);
            assert_eq!(result, ONIG_NORMAL);
            let mut set = set.expect("regset");
            assert!(set.table_entry_count >= 1);
            assert_eq!(
                onig_regset_search(
                    &mut set,
                    input.as_bytes(),
                    input.len(),
                    0,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ),
                (ONIG_MISMATCH, 0),
                "each regex receives its own upstream-compatible search budget"
            );
        }

        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);
    }

    #[test]
    fn nonzero_time_limit_starts_a_fresh_position_lead_search_clock() {
        let _limits = exclusive_limits();
        let old_time = onig_get_time_limit();
        let old_match = onig_get_retry_limit_in_match();
        let old_search = onig_get_retry_limit_in_search();
        let input = "a".repeat(600);
        let make = || {
            let (set, result) = onig_regset_new(vec![compile(br"(?:a|aa)*\z")]);
            assert_eq!(result, ONIG_NORMAL);
            set.expect("regset")
        };
        let mut reused = make();

        onig_set_retry_limit_in_match(0);
        onig_set_retry_limit_in_search(0);
        onig_set_time_limit(50);
        assert_eq!(
            onig_regset_search(
                &mut reused,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 0)
        );
        std::thread::sleep(std::time::Duration::from_millis(75));

        let reused_result = onig_regset_search(
            &mut reused,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        let mut fresh = make();
        let fresh_result = onig_regset_search(
            &mut fresh,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        onig_set_time_limit(old_time);
        onig_set_retry_limit_in_match(old_match);
        onig_set_retry_limit_in_search(old_search);

        assert_eq!(reused_result, fresh_result);
        assert_eq!(reused_result, (0, 0));
    }

    #[test]
    fn fallback_memo_skips_position_dependent_patterns() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(br"a*(?:\Gx|y)")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(fallback_indices(&set), [0]);
        assert!(
            set.entries[0]
                .reg
                .ops
                .iter()
                .any(|op| op.opcode == OpCode::CheckPosition)
        );

        let input = b"axc";
        let identity = FallbackMemoIdentity::OnigString(8);
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (ONIG_MISMATCH, 0)
        );
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (0, 1)
        );
        assert!(set.fallback_memos[0].is_empty());
    }

    #[test]
    fn fallback_memo_safety_is_precomputed_when_entries_change() {
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(set.entries[0].fallback_memo_safe);

        assert_eq!(
            onig_regset_replace(&mut set, 0, Some(compile(br".*(?{x})a"))),
            ONIG_NORMAL
        );
        assert!(!set.entries[0].fallback_memo_safe);

        assert_eq!(
            onig_regset_replace(&mut set, 0, Some(compile(br"a*(?:\Gx|y)"))),
            ONIG_NORMAL
        );
        assert!(!set.entries[0].fallback_memo_safe);
    }

    #[test]
    fn exact_start_fallback_checks_do_not_prefetch_the_suffix() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"xaaaaaaaaaaaaaaaa";

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::OnigString(9),
            ),
            (1, 0)
        );
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::ExactStartMiss(0)]
        ));
    }

    #[test]
    fn identical_direct_miss_is_cached_without_a_second_vm_attempt() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"x";
        let identity = FallbackMemoIdentity::OnigString(13);

        for _ in 0..2 {
            assert_eq!(
                onig_regset_search_fast_with_id(
                    &mut set,
                    input,
                    input.len(),
                    0,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                    identity,
                ),
                (1, 0)
            );
        }
        assert!(
            set.fallback_memos[0]
                .iter()
                .any(|memo| matches!(memo, FallbackMemo::ExactStartMiss(0)))
        );
    }

    #[test]
    fn second_direct_miss_upgrades_to_an_advancing_optimizer_cursor() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = vec![b'x'; 80_000];
        let identity = FallbackMemoIdentity::OnigString(14);

        // The first table winner causes one cheap exact fallback attempt.
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        assert!(matches!(
            set.fallback_memos[0].as_slice(),
            [FallbackMemo::ExactStartMiss(0)]
        ));

        // The next start replaces the exact-only result with a full-search
        // cursor. Every later table winner can reuse its no-match result.
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                &input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 1)
        );
        assert_settled_from(&set, 1);
        for start in [2, input.len() - 1] {
            assert_eq!(
                onig_regset_search_fast_with_id(
                    &mut set,
                    &input,
                    input.len(),
                    start,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                    identity,
                ),
                (1, start as i32)
            );
        }
    }

    #[test]
    fn direct_miss_upgrade_preserves_fallback_match_region_and_restart_passes() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc"), compile(b"x"), compile(b"a")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = b"xaabc";
        let identity = FallbackMemoIdentity::OnigString(15);

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                1,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (0, 1)
        );
        assert_eq!(onig_regset_last_match_len(&set), 4);
        let region = onig_regset_get_region(&set, 0).expect("fallback region");
        assert_eq!((region.beg[0], region.end[0]), (1, 5));

        // A pass restarted at zero cannot reuse a cursor that began at one.
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        assert!(
            set.fallback_memos[0]
                .iter()
                .any(|memo| matches!(memo, FallbackMemo::ExactStartMiss(0)))
        );
    }

    #[test]
    fn direct_miss_upgrade_preserves_a_same_position_retry_error() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let (set, result) =
            onig_regset_new(vec![compile(br"a*x(a+)+b"), compile(b"y"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(fallback_indices(&set), [0]);
        let input = format!("yx{}c", "a".repeat(1_001));
        let identity = FallbackMemoIdentity::OnigString(16);

        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                identity,
            ),
            (1, 0)
        );
        let error = onig_regset_search_fast_with_id(
            &mut set,
            input.as_bytes(),
            input.len(),
            1,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
            identity,
        );
        onig_set_retry_limit_in_match(old_limit);
        assert_eq!(error, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn fallback_memo_skips_callout_patterns() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(br".*(?{x})a")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(
            set.entries[0]
                .reg
                .extp
                .as_ref()
                .is_some_and(|ext| ext.callout_num != 0)
        );
        assert_eq!(fallback_indices(&set), [0]);

        let input = b"x";
        assert_eq!(
            onig_regset_search_fast_with_id(
                &mut set,
                input,
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(5),
            ),
            (ONIG_MISMATCH, 0)
        );
        assert!(set.fallback_memos[0].is_empty());
    }

    #[test]
    fn unbounded_no_match_uses_one_optimizer_scan_then_memoizes() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(b"a*bc")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(fallback_indices(&set), [0]);
        assert!(set.first_byte_candidates[b'a' as usize].is_empty());

        let input = vec![b'a'; 80_000];
        for start in [0, 1] {
            assert_eq!(
                onig_regset_search_fast_with_id(
                    &mut set,
                    &input,
                    input.len(),
                    start,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                    FallbackMemoIdentity::Caller(99),
                ),
                (ONIG_MISMATCH, 0)
            );
        }
        assert_settled_from(&set, 0);
    }

    #[test]
    fn fallback_does_not_probe_a_start_after_an_unbeatable_table_winner() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let (set, result) = onig_regset_new(vec![compile(br"(a*)\1b"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let input = format!("x{}b", "a".repeat(1_001));
        let (index, position) = onig_regset_search(
            &mut set,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!((index, position), (1, 0));
    }

    #[test]
    fn fallback_match_precedes_a_later_table_retry_error() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let input = format!("bx{}c", "a".repeat(1_001));
        for (patterns, expected_index) in [
            ([br"x(a+)+b".as_slice(), br"(a*)\1b".as_slice()], 1),
            ([br"(a*)\1b".as_slice(), br"x(a+)+b".as_slice()], 0),
        ] {
            let (set, result) = onig_regset_new(patterns.into_iter().map(compile).collect());
            assert_eq!(result, ONIG_NORMAL);
            let mut set = set.expect("regset");

            let (index, position) = onig_regset_search(
                &mut set,
                input.as_bytes(),
                input.len(),
                0,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            );
            assert_eq!((index, position), (expected_index, 0));
            assert_eq!(onig_regset_last_match_len(&set), 1);
        }

        onig_set_retry_limit_in_match(old_limit);
    }

    #[test]
    fn table_retry_error_wins_a_same_start_later_fallback_match() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let input = format!("x{}c", "a".repeat(1_001));
        let (set, result) = onig_regset_new(vec![compile(br"x(a+)+b"), compile(br"(x*)\1?")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");

        let (index, position) = onig_regset_search(
            &mut set,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!((index, position), (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
    }

    #[test]
    fn fallback_error_clears_a_superseded_winner_and_match_length() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(100);

        let input = format!("{}bx", "a".repeat(1_001));
        let (set, result) = onig_regset_new(vec![compile(br"(a*)\1b"), compile(b"x")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        let search_result = onig_regset_search(
            &mut set,
            input.as_bytes(),
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!(search_result, (ONIGERR_RETRY_LIMIT_IN_MATCH_OVER, 0));
        assert_eq!(onig_regset_last_match_len(&set), ONIG_MISMATCH);
        for index in 0..2 {
            let region = onig_regset_get_region(&set, index).expect("entry region");
            assert_eq!(
                (region.beg[0], region.end[0]),
                (ONIG_REGION_NOTPOS, ONIG_REGION_NOTPOS)
            );
        }
    }

    #[test]
    fn fallback_search_preserves_g_anchor_and_beats_a_later_table_match() {
        let _limits = shared_limits();
        let (set, result) = onig_regset_new(vec![compile(br"\["), compile(br"\G\s*\[")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert_eq!(fallback_indices(&set), [1]);

        let input = b"xx [";
        let (index, position) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            2,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );

        assert_eq!((index, position), (1, 2));
        let region = onig_regset_get_region(&set, index as usize).expect("winning region");
        assert_eq!((region.beg[0], region.end[0]), (2, 4));
    }

    #[test]
    fn regset_basic_regex_lead() {
        let _limits = shared_limits();
        let regs = vec![compile(b"abc"), compile(b"def"), compile(b"ghi")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::RegexLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 1); // "def" matched
        assert_eq!(pos, 3); // at position 3
    }

    #[test]
    fn regset_earliest_match_regex_lead() {
        let _limits = shared_limits();
        let regs = vec![compile(b"yyy"), compile(b"def"), compile(b"xxx")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::RegexLead,
            ONIG_OPTION_NONE,
        );
        // "xxx" matches at position 0, which is earliest
        assert_eq!(idx, 2);
        assert_eq!(pos, 0);
    }

    #[test]
    fn regset_priority_to_regex_order() {
        let _limits = shared_limits();
        let regs = vec![compile(b"def"), compile(b"xxx")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xxxdefyyy";
        let (idx, pos) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PriorityToRegexOrder,
            ONIG_OPTION_NONE,
        );
        // "def" is first regex, matches at position 3.
        // "xxx" is second regex, matches at position 0 (earlier).
        // PriorityToRegexOrder: first regex "def" finds match at 3,
        // then since PRIORITY mode, stops after finding first match.
        // Actually: PRIORITY mode still finds earliest, but stops once
        // a later regex can't beat the current best. Let me re-check...
        // In C: it searches all regexes but narrows ep. "def" at 3 sets ep=3.
        // "xxx" searches with ep=3, finds at 0 < 3, updates to idx=1,pos=0.
        // Wait no, PRIORITY_TO_REGEX_ORDER breaks on first match found.
        // So "def" at 3 is found first → break. idx=0, pos=3.
        assert_eq!(idx, 0);
        assert_eq!(pos, 3);
    }

    #[test]
    fn regset_no_match() {
        let _limits = shared_limits();
        let regs = vec![compile(b"abc"), compile(b"def")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xyz";
        let (idx, _pos) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, ONIG_MISMATCH);
    }

    #[test]
    fn regset_empty_string() {
        let _limits = shared_limits();
        let regs = vec![compile(b""), compile(b"x")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"";
        let (idx, pos) = onig_regset_search(
            &mut set,
            input,
            0,
            0,
            0,
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 0); // empty pattern matches empty string
        assert_eq!(pos, 0);
    }

    #[test]
    fn regset_fast_empty_string_records_zero_match_length() {
        let _limits = shared_limits();
        let (set, r) = onig_regset_new(vec![compile(b"$")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        assert_eq!(
            onig_regset_search_fast(
                &mut set,
                b"",
                0,
                0,
                0,
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 0)
        );
        assert_eq!(onig_regset_last_match_len(&set), 0);
    }

    /// C attempts a regex only where its optimizer admits the position.
    /// `(\w+)+x` carries the exact optimizer `x` at an unbounded distance;
    /// with no `x` in the subject, C's `forward_search` fails, the regex is
    /// `SRS_DEAD`, and `a` wins at every start (checked against C's
    /// `onig_regset_search` with a retry limit of 10,000). An attempt at the
    /// start without the optimizer stopped at the retry limit instead.
    #[test]
    fn fallback_attempts_only_positions_its_optimizer_admits() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(10_000);

        let input = "a".repeat(30);
        let mut results = Vec::new();
        for start in [0, 1, 5] {
            let (set, result) = onig_regset_new(vec![compile(br"(\w+)+x"), compile(b"a")]);
            assert_eq!(result, ONIG_NORMAL);
            let mut set = set.expect("regset");
            assert_eq!(fallback_indices(&set), [0]);
            results.push(onig_regset_search(
                &mut set,
                input.as_bytes(),
                input.len(),
                start,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ));
            results.push(onig_regset_search_fast_with_id(
                &mut set,
                input.as_bytes(),
                input.len(),
                start,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(17),
            ));
        }

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!(results, [(1, 0), (1, 0), (1, 1), (1, 1), (1, 5), (1, 5)]);
    }

    /// C's optimizer for `(?i)([^\s]+)+x` looks for `x`/`X` (a negated class
    /// has no byte map in C), and the one for the alternation below for `=`
    /// or `b`; with neither in the subject C never attempts these regexes,
    /// and `a` wins at every start (checked against C's `onig_regset_search`
    /// with a retry limit of 10,000). A byte map taken from the negated
    /// class routed them to every position, where they stopped at the limit.
    /// Tokenizing `[(?=\s*[^;{]), \s+]` over "a"×n + "(" finds the zero-width
    /// look-ahead at every start (as vscode-oniguruma's scanner over C
    /// does). The look-ahead is a fallback entry and `\s+` a table entry that
    /// never matches; the table scan used to walk the rest of the subject on
    /// every call before the fallback entry could decide, which made the
    /// tokenization quadratic. It now scans in windows and stops at the
    /// fallback entry's match, as C's position-by-position search does.
    #[test]
    fn a_fallback_match_at_the_start_ends_the_table_scan() {
        let _limits = shared_limits();
        let n = 2_000;
        let input = format!("{}(", "a".repeat(n));
        for identity in [None, Some(FallbackMemoIdentity::Caller(19))] {
            let (set, result) = onig_regset_new(vec![compile(br"(?=\s*[^;{])"), compile(br"\s+")]);
            assert_eq!(result, ONIG_NORMAL);
            let mut set = set.expect("regset");
            assert_eq!(fallback_indices(&set), [0]);
            for start in 0..=input.len() {
                let found = match identity {
                    None => onig_regset_search_fast(
                        &mut set,
                        input.as_bytes(),
                        input.len(),
                        start,
                        input.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                    ),
                    Some(identity) => onig_regset_search_fast_with_id(
                        &mut set,
                        input.as_bytes(),
                        input.len(),
                        start,
                        input.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                        identity,
                    ),
                };
                let expected = if start < input.len() {
                    (0, start as i32)
                } else {
                    (ONIG_MISMATCH, 0)
                };
                assert_eq!(found, expected, "start {start}");
            }
            // One window per call, not the rest of the subject.
            assert!(
                set.table_positions_scanned < 300 * (n as u64 + 2),
                "{} positions",
                set.table_positions_scanned
            );
        }
    }

    /// A table entry dispatched by its start bytes still attempts only the
    /// positions C's optimizer admits. `(?:a|a){0,25}zzz` searches for `zzz`
    /// within 25 bytes; without one in the subject C's regex is `SRS_DEAD`
    /// and `a` wins at every start (checked against C's `onig_regset_search`
    /// with a retry limit of 10,000). Dispatched on `a`, it stopped at the
    /// retry limit instead. Every route agrees: the plain and memoized
    /// searches and the per-regex search of the Scanner's cache route.
    #[test]
    fn start_byte_dispatch_keeps_c_s_optimizer_windows() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(10_000);

        let input = "a".repeat(30);
        let (set, result) = onig_regset_new(vec![compile(b"(?:a|a){0,25}zzz"), compile(b"a")]);
        assert_eq!(result, ONIG_NORMAL);
        let mut set = set.expect("regset");
        assert!(fallback_indices(&set).is_empty());
        assert!(set.entries[0].gated);
        let mut results = Vec::new();
        for start in [0, 5, 29] {
            results.push(onig_regset_search(
                &mut set,
                input.as_bytes(),
                input.len(),
                start,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ));
            results.push(onig_regset_search_fast_with_id(
                &mut set,
                input.as_bytes(),
                input.len(),
                start,
                input.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
                FallbackMemoIdentity::Caller(20),
            ));
        }
        let entry_events: Vec<_> = [0, 5]
            .into_iter()
            .map(|start| {
                onig_regset_entry_search(
                    &mut set,
                    0,
                    input.as_bytes(),
                    input.len(),
                    start,
                    input.len(),
                    ONIG_OPTION_NONE,
                    true,
                )
            })
            .collect();

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!(results, [(1, 0), (1, 0), (1, 5), (1, 5), (1, 29), (1, 29)]);
        assert_eq!(
            entry_events,
            [RegSetEntryEvent::None, RegSetEntryEvent::None]
        );
    }

    #[test]
    fn negated_class_maps_do_not_widen_c_s_optimizer() {
        let _limits = exclusive_limits();
        let old_limit = onig_get_retry_limit_in_match();
        onig_set_retry_limit_in_match(10_000);

        let input = "a".repeat(30);
        let mut results = Vec::new();
        for pattern in [
            &br"(?i)([^\s]+)+x"[..],
            br"(?:\d|.[ab]*)*[^\s]b+.|b*(?:=+) *",
        ] {
            for start in [0, 5] {
                let (set, result) = onig_regset_new(vec![compile(pattern), compile(b"a")]);
                assert_eq!(result, ONIG_NORMAL);
                let mut set = set.expect("regset");
                results.push(onig_regset_search(
                    &mut set,
                    input.as_bytes(),
                    input.len(),
                    start,
                    input.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ));
            }
        }

        onig_set_retry_limit_in_match(old_limit);
        assert_eq!(results, [(1, 0), (1, 5), (1, 0), (1, 5)]);
    }

    /// C's position-lead search attempts an `ANCR_ANYCHAR_INF` regex only at
    /// its first position and after a newline, even where a look-behind
    /// precedes the any-char star (unlike `onig_search`). Checked against C's
    /// `onig_regset_search`: `(?<=b).*x` on "abx" matches from 2 only.
    #[test]
    fn anychar_star_fallback_attempts_follow_the_regset_newline_rule() {
        let _limits = shared_limits();
        let input = b"abx";
        for (start, expected) in [
            (0, (ONIG_MISMATCH, 0)),
            (1, (ONIG_MISMATCH, 0)),
            (2, (0, 2)),
        ] {
            let (set, result) = onig_regset_new(vec![compile(br"(?<=b).*x"), compile(b"q")]);
            assert_eq!(result, ONIG_NORMAL);
            let mut set = set.expect("regset");
            assert_eq!(fallback_indices(&set), [0]);
            for identity in [None, Some(FallbackMemoIdentity::Caller(18))] {
                let found = match identity {
                    None => onig_regset_search(
                        &mut set,
                        input,
                        input.len(),
                        start,
                        input.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                    ),
                    Some(identity) => onig_regset_search_fast_with_id(
                        &mut set,
                        input,
                        input.len(),
                        start,
                        input.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                        identity,
                    ),
                };
                assert_eq!(found, expected, "start {start}, memo {identity:?}");
            }
        }
    }

    #[test]
    fn regset_position_lead_attempts_the_range_position() {
        let _limits = shared_limits();
        let (set, r) = onig_regset_new(vec![compile(b"(?=b)")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        assert_eq!(
            onig_regset_search(
                &mut set,
                b"abc",
                3,
                0,
                1,
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            ),
            (0, 1)
        );
    }

    #[test]
    fn regset_position_lead_finds_zero_width_matches_at_end() {
        let _limits = shared_limits();
        for pattern in [b"$".as_slice(), b"\\z".as_slice(), b"a*".as_slice()] {
            let (set, r) = onig_regset_new(vec![compile(pattern)]);
            assert_eq!(r, ONIG_NORMAL);
            let mut set = set.unwrap();

            assert_eq!(
                onig_regset_search(
                    &mut set,
                    b"abc",
                    3,
                    3,
                    3,
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ),
                (0, 3),
                "pattern {:?}",
                std::str::from_utf8(pattern).unwrap()
            );
        }
    }

    #[test]
    fn regset_nonempty_eos_keeps_regex_lead_semantics() {
        let _limits = shared_limits();
        for lead in [
            OnigRegSetLead::RegexLead,
            OnigRegSetLead::PriorityToRegexOrder,
        ] {
            let (set, r) = onig_regset_new(vec![compile(b"a*")]);
            assert_eq!(r, ONIG_NORMAL);
            let mut set = set.unwrap();

            assert_eq!(
                onig_regset_search(&mut set, b"\na", 2, 2, 2, lead, ONIG_OPTION_NONE),
                (ONIG_MISMATCH, 0),
                "lead {lead:?}"
            );
        }
    }

    #[test]
    fn regset_empty_set() {
        let (set, r) = onig_regset_new(vec![]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"abc";
        let (idx, _) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, ONIG_MISMATCH);
    }

    #[test]
    fn regset_search_normalizes_out_of_range_endpoints() {
        let _limits = shared_limits();
        let (set, r) = onig_regset_new(vec![compile(b"(?=b)")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();
        let input = b"abc";

        for (end, start, range, expected) in [
            (input.len(), 0, 100, (0, 1)),
            (100, 0, 100, (0, 1)),
            (input.len(), 100, 0, (ONIG_MISMATCH, 0)),
        ] {
            assert_eq!(
                onig_regset_search(
                    &mut set,
                    input,
                    end,
                    start,
                    range,
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                ),
                expected,
                "end={end}, start={start}, range={range}"
            );
        }
    }

    #[test]
    fn regset_add_and_replace() {
        let _limits = shared_limits();
        let (set, r) = onig_regset_new(vec![compile(b"abc")]);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        assert_eq!(onig_regset_number_of_regex(&set), 1);

        // Add another regex
        let r = onig_regset_add(&mut set, compile(b"def"));
        assert_eq!(r, ONIG_NORMAL);
        assert_eq!(onig_regset_number_of_regex(&set), 2);

        // Replace first with None (remove)
        let r = onig_regset_replace(&mut set, 0, None);
        assert_eq!(r, ONIG_NORMAL);
        assert_eq!(onig_regset_number_of_regex(&set), 1);

        // The remaining regex should be "def"
        let input = b"def";
        let (idx, pos) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 0);
        assert_eq!(pos, 0);
    }

    #[test]
    fn regset_captures() {
        let _limits = shared_limits();
        let regs = vec![compile(b"a(b)c"), compile(b"(d)(e)f")];
        let (set, r) = onig_regset_new(regs);
        assert_eq!(r, ONIG_NORMAL);
        let mut set = set.unwrap();

        let input = b"xdefx";
        let (idx, pos) = onig_regset_search(
            &mut set,
            input,
            input.len(),
            0,
            input.len(),
            OnigRegSetLead::PositionLead,
            ONIG_OPTION_NONE,
        );
        assert_eq!(idx, 1);
        assert_eq!(pos, 1);

        // Check capture groups in the matching regex's region
        let region = onig_regset_get_region(&set, 1).unwrap();
        assert_eq!(region.beg[0], 1); // full match start
        assert_eq!(region.end[0], 4); // full match end
        assert_eq!(region.beg[1], 1); // group 1 "d"
        assert_eq!(region.end[1], 2);
        assert_eq!(region.beg[2], 2); // group 2 "e"
        assert_eq!(region.end[2], 3);
    }

    /// `\K` moves the match start, and C keeps reporting the position the
    /// winning attempt began at. The length recorded next to it is measured
    /// from that same position, so the two add up to the match end and not to
    /// the kept start plus the kept length. Every number below is C
    /// Oniguruma's, read through the `ffi` feature.
    #[test]
    fn keep_patterns_report_attempt_relative_positions_and_lengths() {
        // Regset searches inherit global limits changed by other tests.
        let _limits = shared_limits();
        let input = b"xxabxx";

        // `a\Kb` dispatches straight from the table route: its first byte is
        // provable. `.*\Kb` delays the optimizer byte, so its entry takes the
        // fallback route and runs its own search.
        for (pattern, position, match_len, region) in [
            (br"a\Kb".as_slice(), 2, 2, (3, 4)),
            (br".*\Kb".as_slice(), 0, 4, (3, 4)),
        ] {
            for eager in [false, true] {
                let (set, result) = onig_regset_new(vec![compile(pattern)]);
                assert_eq!(result, ONIG_NORMAL);
                let mut set = set.expect("regset");
                let where_ = format!("{} eager={eager}", String::from_utf8_lossy(pattern));

                let found = if eager {
                    onig_regset_search(
                        &mut set,
                        input,
                        input.len(),
                        0,
                        input.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                    )
                } else {
                    onig_regset_search_fast(
                        &mut set,
                        input,
                        input.len(),
                        0,
                        input.len(),
                        OnigRegSetLead::PositionLead,
                        ONIG_OPTION_NONE,
                    )
                };
                assert_eq!(found, (0, position), "{where_}");
                assert_eq!(onig_regset_last_match_len(&set), match_len, "{where_}");

                // A pattern that moves its match start keeps its region, so
                // the caller that needs the kept span can read it.
                let found_region = onig_regset_get_region(&set, 0).expect("region");
                assert_eq!(
                    (found_region.beg[0], found_region.end[0]),
                    region,
                    "{where_}"
                );
                assert_eq!(position + match_len, found_region.end[0], "{where_}");
                assert_ne!(position, found_region.beg[0], "{where_}");
            }
        }
    }
}
