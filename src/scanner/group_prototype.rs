//! Experimental scanner-group archives. This format is deliberately separate
//! from ADR-018 and has no compatibility promise. Only Eager consumes it.
use super::*;
use crate::dfa_prefilter::{AutomataCell, group_archive};
use std::ops::Range;
use std::sync::atomic::AtomicUsize;

pub(crate) type Result<T> = std::result::Result<T, PrecompiledError>;
const MAGIC: &[u8; 8] = b"FGRP002\0";
const PORTABLE_MAGIC: &[u8; 8] = b"FPRT001\0";
const MAX_ARCHIVE: usize = 512 << 20;

/// Representation to prepare before exporting a scanner-group experiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// Portable Thompson states; execution retains the existing hybrid DFAs.
    Nfa,
    /// Own portable instructions, interpreted natively with bounded lazy caches.
    #[cfg(feature = "portable-eager-prototype")]
    Portable,
    /// Fully built DFAs, with NFA fallback at the construction budget.
    Dfa,
}

/// Coverage of the immutable group archive, before any groups are decoded.
#[derive(Clone, Copy, Debug, Default)]
pub struct GroupStats {
    /// Groups with prepared NFAs, including bounded DFA build fallbacks.
    pub nfa: usize,
    /// Groups loaded into the native portable instruction interpreter.
    pub portable: usize,
    /// Groups with all three complete DFAs.
    pub dfa: usize,
    /// Groups for which the producer found no suitable prefilter.
    pub disabled: usize,
}

pub(crate) struct GroupSource {
    bytes: Arc<[u8]>,
    ids: HashMap<Vec<u8>, u32>,
    groups: HashMap<Vec<u32>, (Range<usize>, u32)>,
    stats: GroupStats,
    loads: AtomicUsize,
    failures: AtomicUsize,
}

impl GroupSource {
    fn open(bytes: Arc<[u8]>, programs: &PrecompiledPatterns) -> Result<Self> {
        if bytes.len() > MAX_ARCHIVE {
            return Err(PrecompiledError::ResourceLimit);
        }
        let mut input = Reader::new(&bytes);
        let magic = input.take(8)?;
        if (magic != MAGIC && magic != PORTABLE_MAGIC)
            || input.u32()? != programs.prototype_checksum()
        {
            return Err(PrecompiledError::UnsupportedFormat);
        }
        // regex-automata's DFA wire data uses native byte order. This
        // prototype accepts only the producing target's endianness.
        let endian = input.byte()?;
        if endian
            != if magic == PORTABLE_MAGIC {
                1
            } else {
                u8::from(cfg!(target_endian = "little"))
            }
        {
            return Err(PrecompiledError::UnsupportedFormat);
        }
        let keys = programs.prototype_keys();
        let ids: HashMap<_, _> = keys
            .into_iter()
            .enumerate()
            .map(|(i, k)| (k, i as u32))
            .collect();
        let mut stats = GroupStats::default();
        let count = input.count(100_000)?;
        let mut groups = HashMap::new();
        for _ in 0..count {
            let key = input.u32s(65_535)?;
            if key.iter().any(|&id| id as usize >= ids.len()) {
                return Err(PrecompiledError::InvalidData);
            }
            let checksum = input.u32()?;
            let len = input.count(64 << 20)?;
            let start = bytes.len() - input.bytes.len();
            let data = input.take(len)?;
            match data.first() {
                Some(0) => stats.disabled += 1,
                Some(1) => stats.nfa += 1,
                #[cfg(feature = "portable-eager-prototype")]
                Some(5) => stats.portable += 1,
                Some(2 | 3) => stats.dfa += 1,
                _ => return Err(PrecompiledError::InvalidData),
            }
            if groups.insert(key, (start..start + len, checksum)).is_some() {
                return Err(PrecompiledError::InvalidData);
            }
        }
        input.done()?;
        Ok(Self {
            bytes,
            ids,
            groups,
            stats,
            loads: AtomicUsize::new(0),
            failures: AtomicUsize::new(0),
        })
    }

    pub(super) fn load(
        &self,
        patterns: &[&str],
        settings: PatternSettings,
    ) -> Option<AutomataCell> {
        let env = precompiled::Environment::current();
        let key: Option<Vec<_>> = patterns
            .iter()
            .map(|p| {
                self.ids
                    .get(&precompiled::key(p, settings, &env).ok()?)
                    .copied()
            })
            .collect();
        let (range, checksum) = self.groups.get(&key?)?;
        if precompiled::checksum(&self.bytes[range.clone()]) != *checksum {
            self.failures.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        match group_archive::decode(&self.bytes[range.clone()], patterns.len()) {
            Ok(automata) => {
                self.loads.fetch_add(1, Ordering::Relaxed);
                Some(AutomataCell::prepared(automata))
            }
            Err(_) => {
                self.failures.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }
}

impl ScannerPatternCache {
    /// Export already prepared ordered scanner groups. Call this after
    /// constructing every desired scanner in Eager mode. Missing groups keep
    /// the ordinary runtime fallback. Programs are exported separately with
    /// `export_precompiled`; both objects must come from the same cache state.
    pub fn export_group_prototype(&self, kind: GroupKind) -> Result<Vec<u8>> {
        let programs = PrecompiledPatterns::from_bytes(self.export_precompiled()?)?;
        let stable: HashMap<_, _> = programs
            .prototype_keys()
            .into_iter()
            .enumerate()
            .map(|(i, k)| (k, i as u32))
            .collect();
        let mut ids = HashMap::new();
        for (settings, patterns) in &self.compiled {
            for (pattern, cached) in patterns {
                let key = precompiled::key(pattern, *settings, &cached.environment)?;
                if let Some(&stable) = stable.get(&key) {
                    ids.insert(cached.id, stable);
                }
            }
        }
        let mut groups = Vec::new();
        for (local, automata) in &self.automata {
            let key: Option<Vec<u32>> = local.iter().map(|id| ids.get(id).copied()).collect();
            if let (Some(key), Some(data)) = (key, automata.encode(kind)?) {
                groups.push((key, data));
            }
        }
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        groups.dedup_by(|a, b| a.0 == b.0);
        #[cfg(feature = "portable-eager-prototype")]
        let portable = kind == GroupKind::Portable;
        #[cfg(not(feature = "portable-eager-prototype"))]
        let portable = false;
        let mut out = if portable { PORTABLE_MAGIC } else { MAGIC }.to_vec();
        put_u32(&mut out, programs.prototype_checksum());
        out.push(if portable {
            1
        } else {
            u8::from(cfg!(target_endian = "little"))
        });
        put_len(&mut out, groups.len())?;
        for (key, data) in groups {
            put_u32s(&mut out, &key)?;
            put_u32(&mut out, precompiled::checksum(&data));
            put_bytes(&mut out, &data)?;
            if out.len() > MAX_ARCHIVE {
                return Err(PrecompiledError::ResourceLimit);
            }
        }
        Ok(out)
    }

    /// Attach a group experiment bound to the last attached program archive.
    /// A malformed or absent individual group falls back to runtime building.
    /// Off and Auto ignore these experimental group records.
    pub fn add_group_prototype(&mut self, bytes: impl Into<Arc<[u8]>>) -> Result<GroupStats> {
        let programs = self
            .precompiled
            .last()
            .ok_or(PrecompiledError::InvalidData)?;
        let source = GroupSource::open(bytes.into(), programs)?;
        let stats = source.stats;
        self.group_source = Some(source);
        Ok(stats)
    }

    /// Successfully decoded groups and invalid groups that fell back to a
    /// runtime build. Intended to keep benchmark coverage honest.
    pub fn group_prototype_load_stats(&self) -> (usize, usize) {
        self.group_source.as_ref().map_or((0, 0), |source| {
            (
                source.loads.load(Ordering::Relaxed),
                source.failures.load(Ordering::Relaxed),
            )
        })
    }
}

pub(crate) fn put_u32(out: &mut Vec<u8>, n: u32) {
    out.extend(n.to_le_bytes());
}
pub(crate) fn put_len(out: &mut Vec<u8>, n: usize) -> Result<()> {
    put_u32(
        out,
        u32::try_from(n).map_err(|_| PrecompiledError::ResourceLimit)?,
    );
    Ok(())
}
pub(crate) fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    put_len(out, bytes.len())?;
    out.extend(bytes);
    Ok(())
}
pub(crate) fn put_u32s(out: &mut Vec<u8>, values: &[u32]) -> Result<()> {
    put_len(out, values.len())?;
    for &value in values {
        put_u32(out, value);
    }
    Ok(())
}
pub(crate) struct Reader<'a> {
    pub(crate) bytes: &'a [u8],
}
impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if len > self.bytes.len() {
            return Err(PrecompiledError::InvalidData);
        }
        let (value, tail) = self.bytes.split_at(len);
        self.bytes = tail;
        Ok(value)
    }
    pub(crate) fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(crate) fn count(&mut self, max: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        if n > max {
            return Err(PrecompiledError::ResourceLimit);
        }
        Ok(n)
    }
    pub(crate) fn blob(&mut self) -> Result<&'a [u8]> {
        let len = self.count(64 << 20)?;
        self.take(len)
    }
    pub(crate) fn u32s(&mut self, max: usize) -> Result<Vec<u32>> {
        let len = self.count(max)?;
        self.take(len.checked_mul(4).ok_or(PrecompiledError::ResourceLimit)?)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| Ok(u32::from_le_bytes(*b)))
            .collect()
    }
    pub(crate) fn done(&self) -> Result<()> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(PrecompiledError::InvalidData)
        }
    }
}
