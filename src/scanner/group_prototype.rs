//! Experimental scanner-group archives. This format is deliberately separate
//! from ADR-018 and has no compatibility promise. Only Eager consumes it.
use super::*;
use crate::dfa_prefilter::{AutomataCell, group_archive};
use std::ops::Range;
use std::sync::atomic::AtomicUsize;

pub(crate) type Result<T> = std::result::Result<T, PrecompiledError>;
const MAGIC: &[u8; 8] = b"FGRP002\0";
const PORTABLE_MAGIC_V1: &[u8; 8] = b"FPRT001\0";
const PORTABLE_MAGIC: &[u8; 8] = b"FPRT002\0";
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
    parallel_checksum: bool,
    loads: AtomicUsize,
    failures: AtomicUsize,
}

impl GroupSource {
    fn open(bytes: Arc<[u8]>, programs: &PrecompiledPatterns) -> Result<Self> {
        #[cfg(feature = "scanner-load-profile")]
        let _phase =
            crate::scanner::load_profile::scope(crate::scanner::load_profile::Phase::GroupIndex);
        if bytes.len() > MAX_ARCHIVE {
            return Err(PrecompiledError::ResourceLimit);
        }
        let mut input = Reader::new(&bytes);
        let magic = input.take(8)?;
        let parallel_checksum = magic == PORTABLE_MAGIC;
        if (magic != MAGIC && magic != PORTABLE_MAGIC && magic != PORTABLE_MAGIC_V1)
            || input.u32()? != programs.prototype_checksum(parallel_checksum)
        {
            return Err(PrecompiledError::UnsupportedFormat);
        }
        // regex-automata's DFA wire data uses native byte order. This
        // prototype accepts only the producing target's endianness.
        let endian = input.byte()?;
        if endian
            != if magic == PORTABLE_MAGIC || magic == PORTABLE_MAGIC_V1 {
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
            parallel_checksum,
            loads: AtomicUsize::new(0),
            failures: AtomicUsize::new(0),
        })
    }

    pub(super) fn load(
        &self,
        patterns: &[&str],
        settings: PatternSettings,
    ) -> Option<AutomataCell> {
        #[cfg(feature = "scanner-load-profile")]
        let _phase =
            crate::scanner::load_profile::scope(crate::scanner::load_profile::Phase::GroupLookup);
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
        let intact = {
            #[cfg(feature = "scanner-load-profile")]
            let _phase = crate::scanner::load_profile::scope(
                crate::scanner::load_profile::Phase::GroupChecksum,
            );
            record_checksum(&self.bytes[range.clone()], self.parallel_checksum) == *checksum
        };
        if !intact {
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

// Version 2 stripes the corruption check across four independent lanes. This
// removes the per-word dependency chain on large instruction images. It is
// deliberately not authentication; all structural validation still runs.
pub(super) fn record_checksum(bytes: &[u8], parallel: bool) -> u32 {
    if !parallel {
        return precompiled::checksum(bytes);
    }
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut lanes = [
        0xcbf2_9ce4_8422_2325u64,
        0x9e37_79b1_85eb_ca87,
        0xc2b2_ae3d_27d4_eb4f,
        0x1656_67b1_9e37_79f9,
    ];
    let (blocks, tail) = bytes.as_chunks::<32>();
    for block in blocks {
        for (lane, word) in lanes.iter_mut().zip(block.as_chunks::<8>().0) {
            *lane = (*lane ^ u64::from_le_bytes(*word)).wrapping_mul(PRIME);
            *lane ^= *lane >> 29;
        }
    }
    let mut hash = lanes[0]
        ^ lanes[1].rotate_left(13)
        ^ lanes[2].rotate_left(31)
        ^ lanes[3].rotate_left(47)
        ^ bytes.len() as u64;
    let (words, tail) = tail.as_chunks::<8>();
    for word in words {
        hash = (hash ^ u64::from_le_bytes(*word)).wrapping_mul(PRIME);
        hash ^= hash >> 29;
    }
    for &byte in tail {
        hash = (hash ^ u64::from(byte)).wrapping_mul(PRIME);
    }
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 33;
    (hash ^ (hash >> 32)) as u32
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
        put_u32(&mut out, programs.prototype_checksum(portable));
        out.push(if portable {
            1
        } else {
            u8::from(cfg!(target_endian = "little"))
        });
        put_len(&mut out, groups.len())?;
        for (key, data) in groups {
            put_u32s(&mut out, &key)?;
            put_u32(&mut out, record_checksum(&data, portable));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_checksum_covers_lanes_tails_and_order() {
        for len in 0..=96 {
            let bytes: Vec<_> = (0..len).map(|i| (i * 37) as u8).collect();
            let expected = record_checksum(&bytes, true);
            for bit in 0..len * 8 {
                let mut damaged = bytes.clone();
                damaged[bit / 8] ^= 1 << (bit % 8);
                assert_ne!(record_checksum(&damaged, true), expected, "{len} {bit}");
            }
            if len > 1 {
                let mut reversed = bytes.clone();
                reversed.reverse();
                assert_ne!(record_checksum(&reversed, true), expected);
            }
        }
    }

    #[cfg(feature = "portable-eager-prototype")]
    #[test]
    fn legacy_portable_checksums_remain_readable_and_invalid_code_falls_back() {
        let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
        let patterns = ["foo", "[0-9]+"];
        let mut producer = ScannerPatternCache::new();
        Scanner::with_pattern_cache(&patterns, &config, &mut producer).unwrap();
        let programs =
            PrecompiledPatterns::from_bytes(producer.export_precompiled().unwrap()).unwrap();
        let image = producer
            .export_group_prototype(GroupKind::Portable)
            .unwrap();
        assert_eq!(&image[..8], PORTABLE_MAGIC);
        let source = GroupSource::open(image.clone().into(), &programs).unwrap();
        let mut legacy = image.clone();
        legacy[..8].copy_from_slice(PORTABLE_MAGIC_V1);
        legacy[8..12].copy_from_slice(&programs.prototype_checksum(false).to_le_bytes());
        for (range, _) in source.groups.values() {
            let checksum = precompiled::checksum(&legacy[range.clone()]);
            legacy[range.start - 8..range.start - 4].copy_from_slice(&checksum.to_le_bytes());
        }
        for bytes in [image.clone(), legacy] {
            let mut consumer = ScannerPatternCache::new();
            consumer.add_precompiled(programs.clone());
            consumer.add_group_prototype(bytes).unwrap();
            let mut actual =
                Scanner::with_pattern_cache(&patterns, &config, &mut consumer).unwrap();
            let mut reference = Scanner::with_config(&patterns, &config).unwrap();
            for text in ["foo", "123", "bar foo", ""] {
                assert_eq!(
                    actual.find_next_match(text, 0, ScannerFindOptions::NONE),
                    reference.find_next_match(text, 0, ScannerFindOptions::NONE)
                );
            }
            assert_eq!(consumer.group_prototype_load_stats(), (1, 0));
        }
        let mut damaged = image;
        let (range, _) = source.groups.values().next().unwrap();
        // Tag 5 is followed by the forward program's direction and entry states.
        // A valid checksum must never bypass validation of those entry states.
        damaged[range.start + 2..range.start + 6].copy_from_slice(&u32::MAX.to_le_bytes());
        let checksum = record_checksum(&damaged[range.clone()], true);
        damaged[range.start - 8..range.start - 4].copy_from_slice(&checksum.to_le_bytes());
        let mut consumer = ScannerPatternCache::new();
        consumer.add_precompiled(programs);
        consumer.add_group_prototype(damaged).unwrap();
        Scanner::with_pattern_cache(&patterns, &config, &mut consumer).unwrap();
        assert_eq!(consumer.group_prototype_load_stats(), (0, 1));
    }
}
