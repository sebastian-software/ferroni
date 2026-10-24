//! Experimental, pointer-free scanner bytecode with a native lazy interpreter.
//! The producer uses only public Thompson NFA APIs. Loading never constructs
//! an upstream NFA, DFA, or private dependency object from these instructions.
use super::*;
use crate::scanner::PrecompiledError;
use crate::scanner::group_prototype::{Reader, Result, put_len, put_u32};
use regex_automata::nfa::thompson::State;
use regex_automata::util::look::Look;
use std::collections::HashMap;

const UNKNOWN: u32 = u32::MAX;
const MATCH: u32 = 1 << 30;
const DEAD: u32 = 1 << 29;
const INDEX: u32 = DEAD - 1;
fn index(sid: u32) -> usize {
    (sid & INDEX) as usize
}
const MAX_WORDS: usize = 8 << 20;
const _: () = assert!(MAX_NFA_STATES <= u16::MAX as usize + 1);

pub(super) struct Program {
    code: Vec<[u32; 4]>,
    pool: Vec<u32>,
    classes: [u8; 256],
    representatives: Vec<u8>,
    anchored: u32,
    unanchored: u32,
    reverse: bool,
    looks: bool,
}

pub(super) struct Portable {
    forward: Program,
    reverse: Program,
}

pub(super) struct Caches {
    walk: Machine,
    forward: Machine,
    reverse: Machine,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    roots: Vec<u32>,
    matches: Vec<u32>,
    previous: u8,
}

struct Cached {
    key: Key,
}

struct Machine {
    states: Vec<Cached>,
    transitions: Vec<u32>,
    stride2: u32,
    ids: HashMap<Key, u32>,
    seen: Vec<u32>,
    generation: u32,
    stack: Vec<u32>,
    roots: Vec<u32>,
    matches: Vec<u32>,
    bytes: usize,
    limit: usize,
    clears: usize,
    first: bool,
    starts: [[u32; 5]; 2],
}

impl Program {
    pub(super) fn write(nfa: &thompson::NFA, out: &mut Vec<u8>) -> Result<()> {
        let mut pool = Vec::new();
        let mut code = Vec::new();
        for state in nfa.states() {
            let start = pool.len() as u32;
            let op = match state {
                State::Fail => [0, 0, 0, 0],
                State::Match { pattern_id } => [4, pattern_id.as_usize() as u32, 0, 0],
                State::Look { look, next } => {
                    let bit = match look {
                        Look::Start => 0,
                        Look::End => 1,
                        Look::StartLF => 2,
                        Look::EndLF => 3,
                        Look::StartCRLF => 4,
                        Look::EndCRLF => 5,
                        Look::WordAscii => 6,
                        Look::WordAsciiNegate => 7,
                        Look::WordStartAscii => 10,
                        Look::WordEndAscii => 11,
                        Look::WordStartHalfAscii => 14,
                        Look::WordEndHalfAscii => 15,
                        _ => return Err(PrecompiledError::NonportablePattern),
                    };
                    [3, 1 << bit, next.as_usize() as u32, 0]
                }
                State::Union { alternates } => {
                    pool.extend(alternates.iter().map(|id| id.as_usize() as u32));
                    [2, start, alternates.len() as u32, 0]
                }
                State::BinaryUnion { alt1, alt2 } => {
                    pool.extend([alt1.as_usize() as u32, alt2.as_usize() as u32]);
                    [2, start, 2, 0]
                }
                State::ByteRange { trans } => {
                    pool.extend([
                        u32::from(trans.start) | u32::from(trans.end) << 8,
                        trans.next.as_usize() as u32,
                    ]);
                    [1, start, 1, 0]
                }
                State::Sparse(sparse) => {
                    for trans in &sparse.transitions {
                        pool.extend([
                            u32::from(trans.start) | u32::from(trans.end) << 8,
                            trans.next.as_usize() as u32,
                        ]);
                    }
                    [1, start, sparse.transitions.len() as u32, 0]
                }
                State::Dense(dense) => {
                    for (byte, next) in dense.transitions.iter().enumerate() {
                        if next.as_usize() != 0 {
                            pool.extend([byte as u32 | (byte as u32) << 8, next.as_usize() as u32]);
                        }
                    }
                    [1, start, (pool.len() as u32 - start) / 2, 0]
                }
                State::Capture { .. } => return Err(PrecompiledError::NonportablePattern),
            };
            code.push(op);
        }
        out.push(u8::from(nfa.is_reverse()));
        put_len(out, nfa.start_anchored().as_usize())?;
        put_len(out, nfa.start_unanchored().as_usize())?;
        // Split classes by context as well as byte transitions. That keeps
        // every assertion's answer constant within one transition column.
        let mut classes = [0u8; 256];
        let mut ids = HashMap::new();
        for byte in 0..=255u8 {
            let pair = (nfa.byte_classes().get(byte), context(Some(byte)));
            let next = ids.len() as u8;
            classes[byte as usize] = *ids.entry(pair).or_insert(next);
        }
        out.extend(classes);
        write_instructions(&code, out)?;
        write_pool(&pool, out)?;
        Ok(())
    }

    fn read(input: &mut Reader<'_>, patterns: usize, compact: bool) -> Result<Self> {
        #[cfg(feature = "scanner-load-profile")]
        let _phase = crate::scanner::load_profile::scope(
            crate::scanner::load_profile::Phase::PortableDecode,
        );
        let reverse = match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(PrecompiledError::InvalidData),
        };
        let anchored = input.u32()?;
        let unanchored = input.u32()?;
        let classes: [u8; 256] = input.take(256)?.try_into().unwrap();
        let count = input.count(MAX_NFA_STATES)?;
        let code: Vec<[u32; 4]> = if compact {
            // The opcode occupies three bits of the first operand word. The
            // byte planes group repeated high bytes for transport compression.
            // Decode directly into the ordinary, validated runtime layout.
            let columns = read_columns::<8>(input, count)?;
            (0..count)
                .map(|i| {
                    let packed = u32::from_le_bytes([
                        columns[0][i],
                        columns[1][i],
                        columns[2][i],
                        columns[3][i],
                    ]);
                    let b = u32::from_le_bytes([
                        columns[4][i],
                        columns[5][i],
                        columns[6][i],
                        columns[7][i],
                    ]);
                    [packed & 7, packed >> 3, b, 0]
                })
                .collect()
        } else {
            let bytes = input.take(
                count
                    .checked_mul(16)
                    .ok_or(PrecompiledError::ResourceLimit)?,
            )?;
            bytes
                .as_chunks::<16>()
                .0
                .iter()
                .map(|op| {
                    let words = op.as_chunks::<4>().0;
                    [
                        u32::from_le_bytes(words[0]),
                        u32::from_le_bytes(words[1]),
                        u32::from_le_bytes(words[2]),
                        u32::from_le_bytes(words[3]),
                    ]
                })
                .collect()
        };
        let pool = if compact {
            // Range endpoints and every state ID fit in 16 bits under the
            // existing 65,536-state limit. Widen before the shared validator.
            let len = input.count(MAX_WORDS)?;
            let columns = read_columns::<2>(input, len)?;
            (0..len)
                .map(|i| u32::from(u16::from_le_bytes([columns[0][i], columns[1][i]])))
                .collect()
        } else {
            input.u32s(MAX_WORDS)?
        };
        #[cfg(feature = "scanner-load-profile")]
        let _phase = crate::scanner::load_profile::scope(
            crate::scanner::load_profile::Phase::PortableValidate,
        );
        if anchored as usize >= count || unanchored as usize >= count {
            return Err(PrecompiledError::InvalidData);
        }
        let mut looks = false;
        let mut boundaries = [false; 257];
        for &[op, a, b, reserved] in &code {
            let valid = match op {
                0 => a == 0 && b == 0,
                1 | 2 => {
                    let len = (b as usize)
                        .checked_mul(if op == 1 { 2 } else { 1 })
                        .ok_or(PrecompiledError::ResourceLimit)?;
                    let end = (a as usize)
                        .checked_add(len)
                        .ok_or(PrecompiledError::ResourceLimit)?;
                    let data = pool
                        .get(a as usize..end)
                        .ok_or(PrecompiledError::InvalidData)?;
                    if op == 2 {
                        data.iter().all(|&id| (id as usize) < count)
                    } else {
                        let mut last = None;
                        data.as_chunks::<2>().0.iter().all(|t| {
                            let lo = (t[0] & 255) as u8;
                            let hi = (t[0] >> 8) as u8;
                            let ordered = last.is_none_or(|prev| prev < lo);
                            last = Some(hi);
                            boundaries[lo as usize] = true;
                            boundaries[hi as usize + 1] = true;
                            t[0] <= 65535 && lo <= hi && ordered && (t[1] as usize) < count
                        })
                    }
                }
                3 => {
                    looks = true;
                    a.is_power_of_two() && a & !0xCCFF == 0 && (b as usize) < count
                }
                4 => (a as usize) < patterns && b == 0,
                _ => false,
            };
            if !valid || reserved != 0 {
                return Err(PrecompiledError::InvalidData);
            }
        }
        let columns = classes.iter().copied().max().unwrap() as usize + 1;
        let mut representatives = vec![0; columns];
        let mut present = vec![false; columns];
        for (byte, &class) in classes.iter().enumerate() {
            representatives[class as usize] = byte as u8;
            present[class as usize] = true;
        }
        if present.contains(&false) {
            return Err(PrecompiledError::InvalidData);
        }
        // Validate producer-supplied equivalence classes against all consuming
        // instructions and assertion contexts before trusting cached columns.
        for byte in 0..=255u8 {
            let rep = representatives[classes[byte as usize] as usize];
            if context(Some(byte)) != context(Some(rep)) {
                return Err(PrecompiledError::InvalidData);
            }
        }
        let mut intervals = [0u16; 256];
        let mut interval = 0;
        for byte in 0..256 {
            interval += u16::from(boundaries[byte]);
            intervals[byte] = interval;
        }
        for byte in 0..256 {
            if intervals[byte] != intervals[representatives[classes[byte] as usize] as usize] {
                return Err(PrecompiledError::InvalidData);
            }
        }
        Ok(Self {
            code,
            pool,
            classes,
            representatives,
            anchored,
            unanchored,
            reverse,
            looks,
        })
    }

    fn write_loaded(&self, out: &mut Vec<u8>) -> Result<()> {
        out.push(u8::from(self.reverse));
        put_u32(out, self.anchored);
        put_u32(out, self.unanchored);
        out.extend(self.classes);
        write_instructions(&self.code, out)?;
        write_pool(&self.pool, out)?;
        Ok(())
    }

    fn memory_usage(&self) -> usize {
        self.code.capacity() * 16 + self.pool.capacity() * 4 + self.representatives.capacity()
    }
}

fn read_columns<'a, const N: usize>(input: &mut Reader<'a>, len: usize) -> Result<[&'a [u8]; N]> {
    let bytes = input.take(len.checked_mul(N).ok_or(PrecompiledError::ResourceLimit)?)?;
    Ok(std::array::from_fn(|i| &bytes[i * len..(i + 1) * len]))
}

// Omit the reserved runtime word, pack the opcode into the bounded first
// operand, and group equal byte positions. This exposes repeated high bytes
// to the outer codec while preserving branch-free fixed-width decoding.
fn write_instructions(code: &[[u32; 4]], out: &mut Vec<u8>) -> Result<()> {
    put_len(out, code.len())?;
    if code
        .iter()
        .any(|&[op, a, _, _]| op > 4 || a > u32::MAX >> 3)
    {
        return Err(PrecompiledError::ResourceLimit);
    }
    for byte in 0..4 {
        out.extend(
            code.iter()
                .map(|&[op, a, _, _]| ((a << 3 | op) >> (byte * 8)) as u8),
        );
    }
    for byte in 0..4 {
        out.extend(code.iter().map(|op| (op[2] >> (byte * 8)) as u8));
    }
    Ok(())
}

fn write_pool(pool: &[u32], out: &mut Vec<u8>) -> Result<()> {
    put_len(out, pool.len())?;
    if pool.iter().any(|&word| word > u32::from(u16::MAX)) {
        return Err(PrecompiledError::ResourceLimit);
    }
    for byte in 0..2 {
        out.extend(pool.iter().map(|word| (word >> (byte * 8)) as u8));
    }
    Ok(())
}

// Contexts are invariant across all bytes in a transition class.
fn context(byte: Option<u8>) -> u8 {
    match byte {
        None => 0,
        Some(b'\n') => 3,
        Some(b'\r') => 4,
        Some(b) if b.is_ascii_alphanumeric() || b == b'_' => 2,
        Some(_) => 1,
    }
}

fn assertions(previous: u8, next: u8, reverse: bool) -> u32 {
    let (left, right) = if reverse {
        (next, previous)
    } else {
        (previous, next)
    };
    let word_left = left == 2;
    let word_right = right == 2;
    let tests = [
        left == 0,
        right == 0,
        left == 0 || left == 3,
        right == 0 || right == 3,
        left == 0 || left == 3 || (left == 4 && right != 3),
        right == 0 || right == 4 || (right == 3 && left != 4),
        word_left != word_right,
        word_left == word_right,
    ];
    let mut bits = tests
        .iter()
        .enumerate()
        .fold(0, |bits, (i, &yes)| bits | (u32::from(yes) << i));
    bits |= u32::from(!word_left && word_right) << 10;
    bits |= u32::from(word_left && !word_right) << 11;
    bits |= u32::from(!word_left) << 14;
    bits |= u32::from(!word_right) << 15;
    if reverse {
        let pairs = [(0, 1), (2, 3), (4, 5), (10, 11), (14, 15)];
        for (a, b) in pairs {
            let different = ((bits >> a) ^ (bits >> b)) & 1;
            bits ^= (different << a) | (different << b);
        }
    }
    bits
}

impl Machine {
    fn new(program: &Program, first: bool, limit: usize) -> Self {
        Self {
            states: Vec::new(),
            transitions: Vec::new(),
            stride2: (program.representatives.len() + 1)
                .next_power_of_two()
                .trailing_zeros(),
            ids: HashMap::new(),
            seen: vec![0; program.code.len()],
            generation: 0,
            stack: Vec::new(),
            roots: Vec::new(),
            matches: Vec::new(),
            bytes: 0,
            limit,
            clears: 0,
            first,
            starts: [[UNKNOWN; 5]; 2],
        }
    }

    fn intern(&mut self, _program: &Program, key: Key) -> u32 {
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        let bytes =
            2 * (key.roots.len() + key.matches.len()) * 4 + (1usize << self.stride2) * 4 + 128;
        if self.bytes + bytes > self.limit && !self.states.is_empty() {
            self.states.clear();
            self.transitions.clear();
            self.ids.clear();
            self.starts = [[UNKNOWN; 5]; 2];
            self.bytes = 0;
            self.clears += 1;
        }
        let mut id = self.transitions.len() as u32;
        if !key.matches.is_empty() {
            id |= MATCH;
        }
        if key.matches.is_empty() && key.roots.is_empty() {
            id |= DEAD;
        }
        self.ids.insert(key.clone(), id);
        self.states.push(Cached { key });
        self.transitions
            .resize(self.transitions.len() + (1 << self.stride2), UNKNOWN);
        self.bytes += bytes;
        id
    }

    #[inline]
    fn start(&mut self, program: &Program, anchored: bool, previous: Option<u8>) -> u32 {
        let previous = if program.looks { context(previous) } else { 0 };
        let slot = usize::from(anchored);
        let cached = self.starts[slot][previous as usize];
        if cached != UNKNOWN {
            return cached;
        }
        let sid = self.intern(
            program,
            Key {
                roots: vec![if anchored {
                    program.anchored
                } else {
                    program.unanchored
                }],
                matches: Vec::new(),
                previous,
            },
        );
        self.starts[slot][previous as usize] = sid;
        sid
    }

    #[inline]
    fn step(&mut self, program: &Program, sid: u32, byte: Option<u8>) -> u32 {
        let class = byte.map_or(program.representatives.len(), |b| {
            program.classes[b as usize] as usize
        });
        let next = self.transitions[index(sid) + class];
        if next != UNKNOWN {
            return next;
        }
        self.transition(program, sid, byte, class)
    }

    #[inline(never)]
    fn transition(&mut self, program: &Program, sid: u32, byte: Option<u8>, class: usize) -> u32 {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.seen.fill(0);
            self.generation = 1;
        }
        let have = assertions(
            self.states[index(sid) >> self.stride2].key.previous,
            context(byte),
            program.reverse,
        );
        self.stack.clear();
        self.roots.clear();
        self.matches.clear();
        self.stack.extend(
            self.states[index(sid) >> self.stride2]
                .key
                .roots
                .iter()
                .rev(),
        );
        while let Some(id) = self.stack.pop() {
            if self.seen[id as usize] == self.generation {
                continue;
            }
            self.seen[id as usize] = self.generation;
            let [op, a, b, _] = program.code[id as usize];
            match op {
                0 => {}
                1 => {
                    if let Some(byte) = byte {
                        for t in program.pool[a as usize..a as usize + b as usize * 2]
                            .as_chunks::<2>()
                            .0
                        {
                            if u32::from(byte) >= t[0] & 255 && u32::from(byte) <= t[0] >> 8 {
                                self.roots.push(t[1]);
                                break;
                            }
                        }
                    }
                }
                2 => self.stack.extend(
                    program.pool[a as usize..a as usize + b as usize]
                        .iter()
                        .rev(),
                ),
                3 => {
                    if have & a != 0 {
                        self.stack.push(b);
                    }
                }
                4 => {
                    self.matches.push(a);
                    if self.first {
                        break;
                    }
                }
                _ => unreachable!(),
            }
        }
        let key = Key {
            roots: self.roots.clone(),
            matches: self.matches.clone(),
            previous: if program.looks && !self.roots.is_empty() {
                context(byte)
            } else {
                0
            },
        };
        let clears = self.clears;
        let next = self.intern(program, key);
        if clears == self.clears {
            self.transitions[index(sid) + class] = next;
        }
        next
    }

    fn dead(&self, sid: u32) -> bool {
        sid & DEAD != 0
    }
    fn matched(&self, sid: u32) -> bool {
        sid & MATCH != 0
    }
    #[inline(never)]
    fn record(&self, sid: u32, patset: &mut PatternBits) {
        for &pid in &self.states[index(sid) >> self.stride2].key.matches {
            patset.insert(PatternID::new_unchecked(pid as usize));
        }
    }
    #[inline(never)]
    fn record_needed(
        &self,
        sid: u32,
        patset: &mut PatternBits,
        stamps: &[u32],
        stamp: u32,
    ) -> usize {
        let mut found = 0;
        for &pid in &self.states[index(sid) >> self.stride2].key.matches {
            if patset.insert(PatternID::new_unchecked(pid as usize))
                && stamps[pid as usize] == stamp
            {
                found += 1;
            }
        }
        found
    }
    fn memory_usage(&self) -> usize {
        self.bytes
            + (self.seen.capacity()
                + self.stack.capacity()
                + self.roots.capacity()
                + self.matches.capacity())
                * 4
    }
}

impl Portable {
    pub(super) fn read(input: &mut Reader<'_>, patterns: usize, compact: bool) -> Result<Self> {
        let forward = Program::read(input, patterns, compact)?;
        let reverse = Program::read(input, patterns, compact)?;
        if forward.reverse || !reverse.reverse {
            return Err(PrecompiledError::InvalidData);
        }
        Ok(Self { forward, reverse })
    }
    pub(super) fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.forward.write_loaded(out)?;
        self.reverse.write_loaded(out)
    }
    pub(super) fn caches(&self) -> Caches {
        #[cfg(feature = "scanner-load-profile")]
        let _phase = crate::scanner::load_profile::scope(
            crate::scanner::load_profile::Phase::PortableScratch,
        );
        Caches {
            walk: Machine::new(&self.forward, false, OVERLAPPING_CACHE_CAPACITY),
            forward: Machine::new(&self.forward, true, SEARCH_CACHE_CAPACITY),
            reverse: Machine::new(&self.reverse, false, SEARCH_CACHE_CAPACITY),
        }
    }
    pub(super) fn nfa_states(&self) -> usize {
        self.forward.code.len()
    }
    pub(super) fn memory_usage(&self) -> usize {
        self.forward.memory_usage() + self.reverse.memory_usage()
    }

    pub(super) fn search(
        &self,
        cache: &mut Caches,
        haystack: &[u8],
        from: usize,
        until: usize,
    ) -> Option<regex_automata::Match> {
        let machine = &mut cache.forward;
        let mut sid = machine.start(
            &self.forward,
            false,
            from.checked_sub(1).map(|p| haystack[p]),
        );
        let mut end = None;
        let mut p = from;
        while p < until {
            sid = machine.step(&self.forward, sid, Some(haystack[p]));
            if machine.matched(sid) {
                end = Some(p);
            }
            if machine.dead(sid) {
                break;
            }
            p += 1;
        }
        if p == until {
            sid = machine.step(&self.forward, sid, haystack.get(p).copied());
            if machine.matched(sid) {
                end = Some(p);
            }
        }
        let end = end?;
        let machine = &mut cache.reverse;
        let mut sid = machine.start(&self.reverse, true, haystack.get(end).copied());
        let mut start = None;
        let mut p = end;
        while p > from {
            sid = machine.step(&self.reverse, sid, Some(haystack[p - 1]));
            if machine.matched(sid) {
                start = Some(p);
            }
            if machine.dead(sid) {
                break;
            }
            p -= 1;
        }
        if p == from {
            sid = machine.step(&self.reverse, sid, p.checked_sub(1).map(|p| haystack[p]));
            if machine.matched(sid) {
                start = Some(p);
            }
        }
        start.map(|start| regex_automata::Match::new(PatternID::ZERO, start..end))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn scan(
        &self,
        caches: &mut Caches,
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
        let cache = &mut caches.walk;
        let mut sid = cache.start(&self.forward, true, at.checked_sub(1).map(|p| haystack[p]));
        let end = haystack.len();
        let cut = end.min(at.saturating_add(CANDIDATE_SCAN_BYTES));
        let ask = at.saturating_add(if ask_early {
            settle
        } else {
            settle.max(LONG_WALK_BYTES)
        });
        let mut i = at;
        let mut recorded = None;
        macro_rules! finish {
            ($asked:expr) => {{
                sid = cache.step(&self.forward, sid, None);
                cache.record(sid, patset);
                return (ScanEnd::Finished, $asked);
            }};
        }
        // Keep the bounded walk free of admission checks and match-bit loops.
        // Recording out of line also keeps its scratch out of the byte loop.
        let plain_end = cut.min(ask);
        loop {
            if cache.dead(sid) {
                return (ScanEnd::Finished, false);
            }
            if cache.matched(sid) && recorded != Some((sid, cache.clears)) {
                recorded = Some((sid, cache.clears));
                cache.record(sid, patset);
                if patset.is_full() {
                    return (ScanEnd::Settled, false);
                }
            }
            if i >= plain_end {
                if i == end {
                    finish!(false);
                }
                if i == cut {
                    return (ScanEnd::Bound, false);
                }
                break;
            }
            sid = cache.step(&self.forward, sid, Some(haystack[i]));
            i += 1;
        }
        *stamp = stamp.wrapping_add(1);
        if *stamp == 0 {
            stamps.fill(0);
            *stamp = 1;
        }
        let byte = haystack[at];
        let mut needed = 0;
        for &pid in long {
            if !patset.contains(PatternID::new_unchecked(pid as usize))
                && (dispatch[pid as usize][byte as usize / 64] >> (byte % 64)) & 1 != 0
                && admits(entries[pid as usize], at)
            {
                stamps[pid as usize] = *stamp;
                needed += 1;
            }
        }
        if needed == 0 {
            return (ScanEnd::Settled, true);
        }
        loop {
            sid = cache.step(&self.forward, sid, Some(haystack[i]));
            i += 1;
            if cache.dead(sid) {
                return (ScanEnd::Finished, true);
            }
            if cache.matched(sid) && recorded != Some((sid, cache.clears)) {
                recorded = Some((sid, cache.clears));
                needed -= cache.record_needed(sid, patset, stamps, *stamp);
                if needed == 0 || patset.is_full() {
                    return (ScanEnd::Settled, true);
                }
            }
            if i == cut {
                if i == end {
                    finish!(true);
                }
                return (ScanEnd::Bound, true);
            }
        }
    }
}

impl Caches {
    pub(super) fn memory_usage(&self) -> usize {
        self.walk.memory_usage() + self.forward.memory_usage() + self.reverse.memory_usage()
    }
    pub(super) fn clears(&self) -> usize {
        self.walk.clears
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::group_prototype::put_u32s;

    fn small_program() -> Program {
        Program {
            code: vec![
                [0, 0, 0, 0],
                [1, 0, 1, 0],
                [2, 2, 1, 0],
                [3, 1, 4, 0],
                [4, 0, 0, 0],
            ],
            pool: vec![u32::from(b'a') | u32::from(b'z') << 8, 4, 3],
            classes: std::array::from_fn(|i| i as u8),
            representatives: (0..=255).collect(),
            anchored: 1,
            unanchored: 2,
            reverse: false,
            looks: true,
        }
    }

    #[test]
    fn compact_and_legacy_instructions_decode_to_the_same_runtime_program() {
        let program = small_program();
        let mut compact = Vec::new();
        program.write_loaded(&mut compact).unwrap();
        let mut legacy = compact[..265].to_vec();
        put_len(&mut legacy, program.code.len()).unwrap();
        for op in &program.code {
            for &word in op {
                put_u32(&mut legacy, word);
            }
        }
        put_u32s(&mut legacy, &program.pool).unwrap();
        assert!(compact.len() < legacy.len());
        for (bytes, compact) in [(&compact, true), (&legacy, false)] {
            let mut input = Reader::new(bytes);
            let loaded = Program::read(&mut input, 1, compact).unwrap();
            input.done().unwrap();
            assert_eq!(loaded.code, program.code);
            assert_eq!(loaded.pool, program.pool);
            assert_eq!(loaded.classes, program.classes);
            assert_eq!(loaded.representatives, program.representatives);
            assert_eq!(loaded.anchored, program.anchored);
            assert_eq!(loaded.unanchored, program.unanchored);
            assert_eq!(loaded.reverse, program.reverse);
            assert_eq!(loaded.looks, program.looks);
        }
        // The reserved legacy word remains validated rather than discarded.
        legacy[269 + 12] = 1;
        assert!(Program::read(&mut Reader::new(&legacy), 1, false).is_err());
    }

    #[test]
    fn compact_instructions_reject_truncation_bad_opcodes_and_invalid_operands() {
        let mut bytes = Vec::new();
        small_program().write_loaded(&mut bytes).unwrap();
        for end in 0..bytes.len() {
            assert!(Program::read(&mut Reader::new(&bytes[..end]), 1, true).is_err());
        }
        let mut damaged = bytes.clone();
        damaged[269] = 5;
        assert!(Program::read(&mut Reader::new(&damaged), 1, true).is_err());
        let mut damaged = bytes.clone();
        damaged[265..269].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Program::read(&mut Reader::new(&damaged), 1, true),
            Err(PrecompiledError::ResourceLimit)
        ));
        let mut damaged = bytes;
        // Low byte of the range instruction's packed first operand: the
        // opcode remains valid, but the range extends beyond the pool.
        damaged[270] = 3 << 3 | 1;
        assert!(Program::read(&mut Reader::new(&damaged), 1, true).is_err());
    }

    #[test]
    fn compact_pool_preserves_the_largest_state_id_and_checks_narrowing() {
        let mut program = small_program();
        program.code.resize(MAX_NFA_STATES, [0, 0, 0, 0]);
        program.pool[1] = u32::from(u16::MAX);
        program.pool[2] = u32::from(u16::MAX);
        let mut bytes = Vec::new();
        program.write_loaded(&mut bytes).unwrap();
        let loaded = Program::read(&mut Reader::new(&bytes), 1, true).unwrap();
        assert_eq!(loaded.pool, program.pool);
        assert_eq!(loaded.code.len(), MAX_NFA_STATES);
        assert!(write_pool(&[u32::from(u16::MAX) + 1], &mut Vec::new()).is_err());
    }

    #[test]
    fn native_search_matches_public_hybrid_on_unseen_inputs_and_spans() {
        for patterns in [
            vec![r"\bfoo\b", r"bar|baz", r"[a-z]+[0-9]"],
            vec![r"a*", r"ab+?", r"^foo$", r"(?m:^bar$)"],
            vec![r"(?:\b|%)+", r"a(?:bc)*d", r"(?mR:^a$)"],
            vec![r"(?-u:\b{start-half})foo", r"(?-u:bar\b{end-half})"],
            vec![r"\Afoo", r"bar\z", r"(?s:.*)baz", r"(?:abc|a)"],
        ] {
            let config = thompson::Config::new()
                .utf8(false)
                .which_captures(thompson::WhichCaptures::None);
            let forward = thompson::Compiler::new()
                .configure(config.clone())
                .syntax(
                    regex_automata::util::syntax::Config::new()
                        .unicode(false)
                        .utf8(false),
                )
                .build_many(&patterns)
                .unwrap();
            let reverse = thompson::Compiler::new()
                .configure(config.reverse(true))
                .syntax(
                    regex_automata::util::syntax::Config::new()
                        .unicode(false)
                        .utf8(false),
                )
                .build_many(&patterns)
                .unwrap();
            let reference = hybrid_regex::Builder::new().build_from_dfas(
                search_dfa(forward.clone(), MatchKind::LeftmostFirst).unwrap(),
                search_dfa(reverse.clone(), MatchKind::All).unwrap(),
            );
            let mut bytes = Vec::new();
            Program::write(&forward, &mut bytes).unwrap();
            Program::write(&reverse, &mut bytes).unwrap();
            let mut input = Reader::new(&bytes);
            let portable = Portable::read(&mut input, patterns.len(), true).unwrap();
            input.done().unwrap();
            let mut restored = Vec::new();
            portable.write(&mut restored).unwrap();
            assert_eq!(bytes, restored);
            let walk = hybrid::Builder::new()
                .configure(overlapping_config())
                .build_from_nfa(forward)
                .unwrap();
            let mut walk_cache = walk.create_cache();
            let entries: Vec<_> = (0..patterns.len() as u16).collect();
            let dispatch = vec![[u64::MAX; 4]; entries.len()];
            let mut caches = portable.caches();
            caches.walk.limit = 512;
            // A tiny limit exercises state-ID reuse while searches are active.
            caches.forward.limit = 512;
            caches.reverse.limit = 512;
            let mut reference_cache = reference.create_cache();
            let mut seed = 0x64F0_1234u32;
            let alphabet = b"abcdforsz012% \n\r\0\xC3\xA9";
            for sample in 0..400 {
                let mut text = Vec::new();
                for _ in 0..sample % 31 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    text.push(alphabet[seed as usize % alphabet.len()]);
                }
                if sample % 4 == 0 {
                    text.extend(b"foo bar baz");
                }
                if sample % 20 == 0 {
                    text.extend(b"abcdefghijklmnopqrstuvwxyz0123456789".repeat(9));
                }
                for start in 0..=text.len() {
                    let mut expected = PatternBits::new(patterns.len());
                    let mut actual = PatternBits::new(patterns.len());
                    let mut stamps = vec![0; patterns.len()];
                    let mut stamp = 0;
                    let mut i = start;
                    SetPrefilter::walk(
                        &walk,
                        &mut walk_cache,
                        &mut expected,
                        &entries,
                        &[],
                        CANDIDATE_SCAN_BYTES + 1,
                        false,
                        &dispatch,
                        &mut stamps,
                        &mut stamp,
                        &text,
                        start,
                        &mut i,
                        &mut |_, _| true,
                    );
                    portable.scan(
                        &mut caches,
                        &mut actual,
                        &entries,
                        &[],
                        CANDIDATE_SCAN_BYTES + 1,
                        false,
                        &dispatch,
                        &mut stamps,
                        &mut stamp,
                        &text,
                        start,
                        &mut |_, _| true,
                    );
                    assert_eq!(
                        actual.iter().collect::<Vec<_>>(),
                        expected.iter().collect::<Vec<_>>(),
                        "walk {patterns:?} {text:?} {start}"
                    );
                    // All seeks may be long here. Compare early admission,
                    // both phases, scan bounds, and stamp rollover as well.
                    let mut expected = PatternBits::new(patterns.len());
                    let mut actual = PatternBits::new(patterns.len());
                    let mut i = start;
                    let mut stamp = u32::MAX;
                    let mut stamps = vec![0; patterns.len()];
                    let mut admits = |entry: u16, at: usize| {
                        !(usize::from(entry) + at + sample).is_multiple_of(3)
                    };
                    let a = SetPrefilter::walk(
                        &walk,
                        &mut walk_cache,
                        &mut expected,
                        &entries,
                        &entries,
                        0,
                        sample % 2 == 0,
                        &dispatch,
                        &mut stamps,
                        &mut stamp,
                        &text,
                        start,
                        &mut i,
                        &mut admits,
                    );
                    let mut stamp = u32::MAX;
                    let b = portable.scan(
                        &mut caches,
                        &mut actual,
                        &entries,
                        &entries,
                        0,
                        sample % 2 == 0,
                        &dispatch,
                        &mut stamps,
                        &mut stamp,
                        &text,
                        start,
                        &mut admits,
                    );
                    assert!(a == b, "walk phase {patterns:?} {text:?} {start}");
                    assert_eq!(
                        actual.iter().collect::<Vec<_>>(),
                        expected.iter().collect::<Vec<_>>(),
                        "admitted walk {patterns:?} {text:?} {start}"
                    );
                    for end in [start, text.len()] {
                        let input = Input::new(&text).span(start..end);
                        let expected = reference
                            .try_search(&mut reference_cache, &input)
                            .unwrap()
                            .map(|m| m.range());
                        let actual = portable
                            .search(&mut caches, &text, start, end)
                            .map(|m| m.range());
                        assert_eq!(actual, expected, "{patterns:?} {text:?} {start}..{end}");
                    }
                }
            }
            assert!(caches.walk.clears + caches.forward.clears + caches.reverse.clears > 0);
            for end in 0..bytes.len() {
                assert!(
                    Portable::read(&mut Reader::new(&bytes[..end]), patterns.len(), true).is_err()
                );
            }
        }
    }
}
