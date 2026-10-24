//! Optional experiment: portable NFA states or fully determinized group DFAs.
//! Never serializes a mutable hybrid cache or process-local pointer.
use super::*;
use crate::scanner::PrecompiledError;
#[cfg(feature = "scanner-group-prototype")]
use crate::scanner::group_prototype::put_bytes;
use crate::scanner::group_prototype::{GroupKind, Reader, Result, put_len, put_u32, put_u32s};
#[cfg(all(
    feature = "scanner-group-prototype",
    not(feature = "scanner-group-dense-prototype")
))]
use regex_automata::dfa::sparse;
#[cfg(feature = "scanner-group-prototype")]
use regex_automata::dfa::{Automaton, StartKind, dense, regex as full_regex};
use regex_automata::nfa::thompson::{State, Transition};
use regex_automata::util::{look::Look as NfaLook, primitives::StateID};

#[cfg(all(
    feature = "scanner-group-prototype",
    not(feature = "scanner-group-dense-prototype")
))]
type FullDfa = sparse::DFA<Vec<u8>>;
#[cfg(feature = "scanner-group-dense-prototype")]
type FullDfa = dense::DFA<Vec<u32>>;
#[cfg(feature = "scanner-group-dense-prototype")]
const FROZEN_TAG: u8 = 3;
#[cfg(all(
    feature = "scanner-group-prototype",
    not(feature = "scanner-group-dense-prototype")
))]
const FROZEN_TAG: u8 = 2;
const DFA_BYTES: usize = 16 << 20;
const DETERMINIZE_BYTES: usize = 64 << 20;

#[cfg(feature = "scanner-group-prototype")]
pub(crate) struct Frozen {
    pub(super) search: full_regex::Regex<FullDfa>,
    walk: FullDfa,
    pub(super) nfa_states: usize,
}
#[cfg(feature = "scanner-group-prototype")]
impl Frozen {
    pub(super) fn memory_usage(&self) -> usize {
        self.walk.memory_usage()
            + self.search.forward().memory_usage()
            + self.search.reverse().memory_usage()
    }
}

impl AutomataCell {
    pub(crate) fn prepared(automata: Option<Automata>) -> Self {
        Self(Arc::new(OnceLock::from(automata.map(Arc::new))))
    }
    pub(crate) fn encode(&self, kind: GroupKind) -> Result<Option<Vec<u8>>> {
        let Some(automata) = self.0.get() else {
            return Ok(None);
        };
        let Some(automata) = automata else {
            return Ok(Some(vec![0]));
        };
        encode(automata, kind).map(Some)
    }
}

#[cfg(feature = "scanner-group-prototype")]
fn full_dfa(nfa: &thompson::NFA, kind: MatchKind, start: StartKind) -> Option<FullDfa> {
    let dfa = dense::Builder::new()
        .configure(
            dense::Config::new()
                .match_kind(kind)
                .start_kind(start)
                .dfa_size_limit(Some(DFA_BYTES))
                .determinize_size_limit(Some(DETERMINIZE_BYTES)),
        )
        .build_from_nfa(nfa)
        .ok()?;
    #[cfg(feature = "scanner-group-dense-prototype")]
    {
        Some(dfa)
    }
    #[cfg(not(feature = "scanner-group-dense-prototype"))]
    {
        dfa.to_sparse().ok()
    }
}

fn encode(automata: &Automata, kind: GroupKind) -> Result<Vec<u8>> {
    #[cfg(feature = "precompiled-eager")]
    if automata.portable.is_some() && kind != GroupKind::Portable {
        return Err(PrecompiledError::NonportablePattern);
    }

    #[cfg(feature = "scanner-group-prototype")]
    let full = if kind == GroupKind::Dfa {
        full_dfa(automata.dfa.get_nfa(), MatchKind::All, StartKind::Anchored).and_then(|walk| {
            Some((
                walk,
                full_dfa(
                    automata.search.forward().get_nfa(),
                    MatchKind::LeftmostFirst,
                    StartKind::Both,
                )?,
                full_dfa(
                    automata.search.reverse().get_nfa(),
                    MatchKind::All,
                    StartKind::Anchored,
                )?,
            ))
        })
    } else {
        None
    };
    #[cfg(not(feature = "scanner-group-prototype"))]
    if kind == GroupKind::Dfa {
        return Err(PrecompiledError::UnsupportedFormat);
    }
    #[cfg(feature = "precompiled-eager")]
    let portable = kind == GroupKind::Portable;
    #[cfg(not(feature = "precompiled-eager"))]
    let portable = false;
    let mut out = vec![if portable { 6 } else { 1 }];
    #[cfg(feature = "scanner-group-prototype")]
    if full.is_some() {
        out[0] = FROZEN_TAG;
    }
    for entries in [&automata.entries, &automata.own, &automata.long] {
        put_u32s(
            &mut out,
            &entries.iter().map(|&n| u32::from(n)).collect::<Vec<_>>(),
        )?;
    }
    put_len(&mut out, automata.settle)?;
    #[cfg(feature = "precompiled-eager")]
    if portable {
        if let Some(program) = &automata.portable {
            program.write(&mut out)?;
        } else {
            portable::Program::write(automata.dfa.get_nfa(), &mut out)?;
            portable::Program::write(automata.search.reverse().get_nfa(), &mut out)?;
        }
        return Ok(out);
    }
    #[cfg(feature = "scanner-group-prototype")]
    if let Some((walk, forward, reverse)) = full {
        put_len(&mut out, automata.dfa.get_nfa().states().len())?;
        for dfa in [walk, forward, reverse] {
            #[cfg(feature = "scanner-group-dense-prototype")]
            {
                let (bytes, padding) = dfa.to_bytes_native_endian();
                put_bytes(&mut out, &bytes[padding..])?;
            }
            #[cfg(not(feature = "scanner-group-dense-prototype"))]
            put_bytes(&mut out, &dfa.to_bytes_native_endian())?;
        }
        return Ok(out);
    }
    write_nfa(automata.dfa.get_nfa(), &mut out)?;
    write_nfa(automata.search.reverse().get_nfa(), &mut out)?;
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8], count: usize) -> Result<Option<Automata>> {
    #[cfg(feature = "scanner-load-profile")]
    let _phase =
        crate::scanner::load_profile::scope(crate::scanner::load_profile::Phase::GroupDecode);
    let mut input = Reader::new(bytes);
    let tag = input.byte()?;
    if tag == 0 {
        input.done()?;
        return Ok(None);
    }
    let mut indices = |max| -> Result<Vec<u16>> {
        input
            .u32s(65_535)?
            .into_iter()
            .map(|n| {
                if n as usize >= max {
                    return Err(PrecompiledError::InvalidData);
                }
                u16::try_from(n).map_err(|_| PrecompiledError::InvalidData)
            })
            .collect()
    };
    let entries = indices(count)?;
    let own = indices(count)?;
    let long = indices(entries.len())?;
    if entries.is_empty()
        || entries.len() + own.len() != count
        || !entries.windows(2).all(|w| w[0] < w[1])
        || !own.windows(2).all(|w| w[0] < w[1])
        || !long.windows(2).all(|w| w[0] < w[1])
        || entries.iter().any(|e| own.binary_search(e).is_ok())
    {
        return Err(PrecompiledError::InvalidData);
    }
    let settle = input.count(LONG_SEEK_BYTES)?;
    #[cfg(feature = "precompiled-eager")]
    let mut portable = None;
    #[cfg(feature = "precompiled-eager")]
    if matches!(tag, 5 | 6) {
        portable = Some(Arc::new(portable::Portable::read(
            &mut input,
            entries.len(),
            tag == 6,
        )?));
    }
    #[cfg(feature = "scanner-group-prototype")]
    let mut frozen = None;
    let (nfa, nfarev) = match tag {
        #[cfg(feature = "precompiled-eager")]
        5 | 6 => (thompson::NFA::never_match(), thompson::NFA::never_match()),
        1 => {
            let nfa = read_nfa(&mut input)?;
            let reverse = read_nfa(&mut input)?;
            if nfa.pattern_len() != entries.len()
                || reverse.pattern_len() != entries.len()
                || nfa.is_reverse()
                || !reverse.is_reverse()
            {
                return Err(PrecompiledError::InvalidData);
            }
            (nfa, reverse)
        }
        #[cfg(feature = "scanner-group-prototype")]
        tag if tag == FROZEN_TAG => {
            let nfa_states = input.count(MAX_NFA_STATES)?;
            let mut read = || -> Result<FullDfa> {
                let bytes = input.blob()?;
                #[cfg(feature = "scanner-group-dense-prototype")]
                let aligned = {
                    let mut aligned = vec![0u8; bytes.len() + 3];
                    let padding = aligned.as_ptr().align_offset(std::mem::align_of::<u32>());
                    aligned[padding..padding + bytes.len()].copy_from_slice(bytes);
                    (aligned, padding)
                };
                #[cfg(feature = "scanner-group-dense-prototype")]
                let (dfa, len) =
                    dense::DFA::from_bytes(&aligned.0[aligned.1..aligned.1 + bytes.len()])
                        .map_err(|_| PrecompiledError::InvalidData)?;
                #[cfg(not(feature = "scanner-group-dense-prototype"))]
                let (dfa, len) =
                    sparse::DFA::from_bytes(bytes).map_err(|_| PrecompiledError::InvalidData)?;
                if len != bytes.len() || dfa.pattern_len() != entries.len() {
                    return Err(PrecompiledError::InvalidData);
                }
                Ok(dfa.to_owned())
            };
            let walk = read()?;
            let forward = read()?;
            let reverse = read()?;
            let prepared = Frozen {
                walk,
                search: full_regex::Builder::new().build_from_dfas(forward, reverse),
                nfa_states,
            };
            // Keep the normal prefilter's fields intact. These tiny unused
            // hybrid handles/caches are included in the prototype's memory
            // and load measurements; a production enum could remove them.
            frozen = Some(Arc::new(prepared));
            (thompson::NFA::never_match(), thompson::NFA::never_match())
        }
        _ => return Err(PrecompiledError::InvalidData),
    };
    input.done()?;
    #[cfg(feature = "scanner-load-profile")]
    let _phase = crate::scanner::load_profile::scope(
        crate::scanner::load_profile::Phase::CompatibilityHandles,
    );
    let forward =
        search_dfa(nfa.clone(), MatchKind::LeftmostFirst).ok_or(PrecompiledError::ResourceLimit)?;
    let reverse = search_dfa(nfarev, MatchKind::All).ok_or(PrecompiledError::ResourceLimit)?;
    let search = Arc::new(hybrid_regex::Builder::new().build_from_dfas(forward, reverse));
    let dfa = hybrid::Builder::new()
        .configure(overlapping_config())
        .build_from_nfa(nfa)
        .map_err(|_| PrecompiledError::ResourceLimit)?;
    Ok(Some(Automata {
        search,
        dfa,
        entries,
        own,
        long,
        settle,
        #[cfg(feature = "scanner-group-prototype")]
        frozen,
        #[cfg(feature = "precompiled-eager")]
        portable,
    }))
}

fn write_nfa(nfa: &thompson::NFA, out: &mut Vec<u8>) -> Result<()> {
    out.extend([
        u8::from(nfa.is_utf8()),
        u8::from(nfa.is_reverse()),
        nfa.look_matcher().get_line_terminator(),
    ]);
    put_len(out, nfa.states().len())?;
    put_len(out, nfa.pattern_len())?;
    put_len(out, nfa.start_anchored().as_usize())?;
    put_len(out, nfa.start_unanchored().as_usize())?;
    for pid in nfa.patterns() {
        put_len(out, nfa.start_pattern(pid).unwrap().as_usize())?;
    }
    for state in nfa.states() {
        match state {
            State::ByteRange { trans } => {
                out.push(0);
                write_trans(out, trans)?;
            }
            State::Sparse(sparse) => {
                out.push(1);
                put_len(out, sparse.transitions.len())?;
                for trans in &sparse.transitions {
                    write_trans(out, trans)?;
                }
            }
            State::Dense(dense) => {
                out.push(1);
                let mut ranges: Vec<Transition> = Vec::new();
                for (byte, &next) in dense.transitions.iter().enumerate() {
                    if next == StateID::ZERO {
                        continue;
                    }
                    if let Some(last) = ranges.last_mut() {
                        if last.next == next && usize::from(last.end) + 1 == byte {
                            last.end = byte as u8;
                            continue;
                        }
                    }
                    ranges.push(Transition {
                        start: byte as u8,
                        end: byte as u8,
                        next,
                    });
                }
                put_len(out, ranges.len())?;
                for trans in ranges {
                    write_trans(out, &trans)?;
                }
            }
            State::Look { look, next } => {
                out.push(2);
                put_u32(out, look.as_repr());
                put_len(out, next.as_usize())?;
            }
            State::Union { alternates } => {
                out.push(3);
                put_u32s(
                    out,
                    &alternates
                        .iter()
                        .map(|s| s.as_usize() as u32)
                        .collect::<Vec<_>>(),
                )?;
            }
            State::BinaryUnion { alt1, alt2 } => {
                out.push(3);
                put_u32s(out, &[alt1.as_usize() as u32, alt2.as_usize() as u32])?;
            }
            State::Fail => out.push(4),
            State::Match { pattern_id } => {
                out.push(5);
                put_len(out, pattern_id.as_usize())?;
            }
            State::Capture { .. } => return Err(PrecompiledError::NonportablePattern),
        }
    }
    Ok(())
}
fn write_trans(out: &mut Vec<u8>, trans: &Transition) -> Result<()> {
    out.extend([trans.start, trans.end]);
    put_len(out, trans.next.as_usize())
}

enum SavedState {
    Range(Transition),
    Sparse(Vec<Transition>),
    Look(NfaLook, StateID),
    Union(Vec<StateID>),
    Fail,
    Match(usize),
}
fn read_nfa(input: &mut Reader<'_>) -> Result<thompson::NFA> {
    let utf8 = input.byte()?;
    let reverse = input.byte()?;
    if utf8 != 0 || reverse > 1 {
        return Err(PrecompiledError::InvalidData);
    }
    let terminator = input.byte()?;
    let count = input.count(MAX_NFA_STATES)?;
    let patterns = input.count(65_535)?;
    if count == 0 || patterns == 0 {
        return Err(PrecompiledError::InvalidData);
    }
    fn state_id(input: &mut Reader<'_>, count: usize) -> Result<StateID> {
        let id = input.count(count - 1)?;
        StateID::new(id).map_err(|_| PrecompiledError::InvalidData)
    }
    fn transition(input: &mut Reader<'_>, count: usize) -> Result<Transition> {
        let start = input.byte()?;
        let end = input.byte()?;
        if start > end {
            return Err(PrecompiledError::InvalidData);
        }
        Ok(Transition {
            start,
            end,
            next: state_id(input, count)?,
        })
    }
    let anchored = state_id(input, count)?;
    let unanchored = state_id(input, count)?;
    let starts: Vec<_> = (0..patterns)
        .map(|_| state_id(input, count))
        .collect::<Result<_>>()?;
    let mut states = Vec::with_capacity(count);
    let mut edge_budget = 4_000_000usize;
    for _ in 0..count {
        let state = match input.byte()? {
            0 => SavedState::Range(transition(input, count)?),
            1 => {
                let len = input.count(256)?;
                let transitions: Vec<_> = (0..len)
                    .map(|_| transition(input, count))
                    .collect::<Result<_>>()?;
                if transitions.windows(2).any(|w| w[0].end >= w[1].start) {
                    return Err(PrecompiledError::InvalidData);
                }
                SavedState::Sparse(transitions)
            }
            2 => SavedState::Look(
                NfaLook::from_repr(input.u32()?).ok_or(PrecompiledError::InvalidData)?,
                state_id(input, count)?,
            ),
            3 => {
                let len = input.count(count)?;
                edge_budget = edge_budget
                    .checked_sub(len)
                    .ok_or(PrecompiledError::ResourceLimit)?;
                SavedState::Union(
                    (0..len)
                        .map(|_| state_id(input, count))
                        .collect::<Result<_>>()?,
                )
            }
            4 => SavedState::Fail,
            5 => SavedState::Match(input.count(patterns - 1)?),
            _ => return Err(PrecompiledError::InvalidData),
        };
        states.push(state);
    }
    let mut builder = thompson::Builder::new();
    builder.set_utf8(false);
    builder.set_reverse(reverse != 0);
    builder
        .set_size_limit(Some(64 << 20))
        .map_err(|_| PrecompiledError::ResourceLimit)?;
    let mut look = regex_automata::util::look::LookMatcher::new();
    look.set_line_terminator(terminator);
    builder.set_look_matcher(look);
    let build_error = |_| PrecompiledError::ResourceLimit;
    // The producer's compiler emits match states in pattern order. Preserve
    // its state IDs directly, including forward references, instead of
    // allocating an empty indirection for every state. Retain the general
    // reconstruction below for a valid record with reordered match states.
    let match_ids: Vec<_> = states
        .iter()
        .filter_map(|s| match s {
            SavedState::Match(pid) => Some(*pid),
            _ => None,
        })
        .collect();
    if match_ids.windows(2).all(|w| w[0] <= w[1]) {
        let mut pid = 0;
        builder.start_pattern().map_err(build_error)?;
        for (i, state) in states.into_iter().enumerate() {
            if let SavedState::Match(next) = &state {
                while pid < *next {
                    builder.finish_pattern(starts[pid]).map_err(build_error)?;
                    pid += 1;
                    builder.start_pattern().map_err(build_error)?;
                }
            }
            let sid = match state {
                SavedState::Range(trans) => builder.add_range(trans),
                SavedState::Sparse(transitions) => builder.add_sparse(transitions),
                SavedState::Look(look, next) => builder.add_look(next, look),
                SavedState::Union(alternates) => builder.add_union(alternates),
                SavedState::Fail => builder.add_fail(),
                SavedState::Match(_) => builder.add_match(),
            }
            .map_err(build_error)?;
            if sid.as_usize() != i {
                return Err(PrecompiledError::InvalidData);
            }
        }
        for (pid, &start) in starts.iter().enumerate().skip(pid) {
            builder.finish_pattern(start).map_err(build_error)?;
            if pid + 1 < patterns {
                builder.start_pattern().map_err(build_error)?;
            }
        }
        return builder.build(anchored, unanchored).map_err(build_error);
    }
    // Allocate stable forward-reference slots first. The builder removes
    // these empty indirections while constructing its final immutable NFA.
    for _ in 0..count {
        builder.add_empty().map_err(build_error)?;
    }
    let mut matches: Vec<Vec<usize>> = vec![Vec::new(); patterns];
    for (i, state) in states.into_iter().enumerate() {
        let sid = match state {
            SavedState::Range(trans) => builder.add_range(trans),
            SavedState::Sparse(transitions) => builder.add_sparse(transitions),
            SavedState::Look(look, next) => builder.add_look(next, look),
            SavedState::Union(alternates) => builder.add_union(alternates),
            SavedState::Fail => builder.add_fail(),
            SavedState::Match(pid) => {
                matches[pid].push(i);
                continue;
            }
        }
        .map_err(build_error)?;
        builder
            .patch(StateID::new(i).unwrap(), sid)
            .map_err(build_error)?;
    }
    for (pid, matches) in matches.into_iter().enumerate() {
        builder.start_pattern().map_err(build_error)?;
        for i in matches {
            let sid = builder.add_match().map_err(build_error)?;
            builder
                .patch(StateID::new(i).unwrap(), sid)
                .map_err(build_error)?;
        }
        builder.finish_pattern(starts[pid]).map_err(build_error)?;
    }
    builder.build(anchored, unanchored).map_err(build_error)
}

#[cfg(feature = "scanner-group-prototype")]
impl Frozen {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn scan(
        &self,
        patset: &mut PatternBits,
        entries: &[u16],
        long: &[u16],
        settle: usize,
        ask_early: bool,
        dispatch: &[[u64; 4]],
        stamps: &mut [u32],
        stamp: &mut u32,
        haystack: &[u8],
        at: usize,
        admits: &mut dyn FnMut(u16, usize) -> bool,
    ) -> (ScanEnd, bool) {
        let dfa = &self.walk;
        let mut position = at;
        let i = &mut position;

        let end = haystack.len();
        let input = Input::new(haystack).span(at..end).anchored(Anchored::Yes);
        let Ok(mut sid) = dfa.start_state_forward(&input) else {
            return (ScanEnd::Bound, false);
        };
        let cut = end.min(at.saturating_add(CANDIDATE_SCAN_BYTES));
        // The position at which the walk asks about the long seeks.
        let ask_at = if ask_early {
            settle
        } else {
            settle.max(LONG_WALK_BYTES)
        };
        let ask_i = at.saturating_add(ask_at);
        // A match state stands for its patterns wherever the walk reaches
        // it; a run of word characters keeps the walk in one match state
        // for every identifier-like seek, which is recorded once.
        // The match state last recorded, with the cache's clear count: a
        // clear reassigns state IDs, so the same ID may name another state.
        let mut recorded: Option<(StateID, usize)> = None;
        macro_rules! at_end {
            ($asked:expr) => {
                let eoi = dfa.next_eoi_state(sid);
                if dfa.is_match_state(eoi) {
                    record_frozen(dfa, patset, eoi);
                }
                return (ScanEnd::Finished, $asked);
            };
        }
        // Up to `ask_i`, the plain walk. All three byte limits are fixed
        // during the walk; compare their minimum once per byte, then retain
        // end-of-input's precedence over the scan bound at that boundary.
        let plain_end = cut.min(ask_i);
        loop {
            if dfa.is_dead_state(sid) {
                return (ScanEnd::Finished, false);
            }
            if dfa.is_quit_state(sid) {
                return (ScanEnd::Bound, false);
            }
            // A match state stands for the matches that ended before the
            // byte it was reached by (or at the end of the haystack).
            if dfa.is_match_state(sid) && recorded != Some((sid, 0)) {
                recorded = Some((sid, 0));
                record_frozen(dfa, patset, sid);
                if patset.is_full() {
                    return (ScanEnd::Settled, false);
                }
            }
            if *i >= plain_end {
                if *i == end {
                    at_end!(false);
                }
                if *i == cut {
                    return (ScanEnd::Bound, false);
                }
                break;
            }
            #[cfg(test)]
            SCAN_STEPS.with(|steps| steps.set(steps.get() + 1));
            sid = dfa.next_state(sid, haystack[*i]);
            *i += 1;
        }
        // The bounded seeks have settled: only an admissible long seek not
        // yet recorded can add a candidate.
        *stamp = stamp.wrapping_add(1);
        if *stamp == 0 {
            stamps.fill(0);
            *stamp = 1;
        }
        let byte = haystack[at];
        let mut needed = 0usize;
        for &pid in long.iter() {
            let bits = dispatch[pid as usize];
            if patset.contains(PatternID::new_unchecked(pid as usize))
                || (bits[byte as usize / 64] >> (byte % 64)) & 1 == 0
            {
                continue;
            }
            if admits(entries[pid as usize], at) {
                stamps[pid as usize] = *stamp;
                needed += 1;
            }
        }
        if needed == 0 {
            return (ScanEnd::Settled, true);
        }
        // On, until every admissible long seek has been recorded.
        loop {
            #[cfg(test)]
            SCAN_STEPS.with(|steps| steps.set(steps.get() + 1));
            sid = dfa.next_state(sid, haystack[*i]);
            *i += 1;
            if dfa.is_dead_state(sid) {
                return (ScanEnd::Finished, true);
            }
            if dfa.is_quit_state(sid) {
                return (ScanEnd::Bound, true);
            }
            if dfa.is_match_state(sid) && recorded != Some((sid, 0)) {
                recorded = Some((sid, 0));
                needed -= record_frozen_needed(dfa, patset, sid, stamps, *stamp);
                if needed == 0 || patset.is_full() {
                    return (ScanEnd::Settled, true);
                }
            }
            if *i == cut {
                if *i == end {
                    at_end!(true);
                }
                return (ScanEnd::Bound, true);
            }
        }
    }
}

#[cfg(feature = "scanner-group-prototype")]
#[inline(never)]
fn record_frozen(dfa: &FullDfa, patset: &mut PatternBits, sid: StateID) {
    for k in 0..dfa.match_len(sid) {
        patset.insert(dfa.match_pattern(sid, k));
    }
}
#[cfg(feature = "scanner-group-prototype")]
#[inline(never)]
fn record_frozen_needed(
    dfa: &FullDfa,
    patset: &mut PatternBits,
    sid: StateID,
    stamps: &[u32],
    stamp: u32,
) -> usize {
    let mut found = 0;
    for k in 0..dfa.match_len(sid) {
        let pid = dfa.match_pattern(sid, k);
        if patset.insert(pid) && stamps[pid.as_usize()] == stamp {
            found += 1;
        }
    }
    found
}
