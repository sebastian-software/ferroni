//! Source-scoped selective import. The index is not a new execution format:
//! payloads retain the ordinary program and portable-group codecs.
use super::{
    PatternSettings, PrecompiledBytes, PrecompiledEager, PrecompiledError, RegexType,
    ScannerConfig, ScannerPatternCache, SharedSeek,
    group_prototype::{self, Reader, put_bytes, put_len, put_u32, put_u32s},
    precompiled::{self, Environment},
};
use crate::dfa_prefilter::{AutomataCell, group_archive};
use std::{
    collections::HashMap,
    fmt,
    ops::Range,
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

type Result<T> = std::result::Result<T, PrecompiledError>;
/// Version of the selective-import index, independent of program and group codecs.
/// Incompatible changes to the dictionary, references or binding require a bump.
pub const PRECOMPILED_EAGER_INDEX_FORMAT_VERSION: u32 = 1;
const MAGIC: &[u8; 8] = b"FREC001\0";
const MAX_PROGRAM_BYTES: usize = 256 << 20;
const MAX_GROUP_BYTES: usize = 512 << 20;

#[derive(Debug)]
struct Record {
    range: Range<usize>,
    checksum: u32,
}

struct Program {
    record: Record,
    settings: PatternSettings,
}
struct Group {
    record: Record,
    patterns: usize,
}
struct Index {
    programs: Vec<Program>,
    ids: HashMap<Vec<u8>, u32>,
    groups: Vec<Group>,
    group_ids: HashMap<Vec<u32>, u32>,
    parallel: bool,
}

/// Which original archive contains a record's byte range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrecompiledEagerRecordKind {
    /// An ordinary compiled regex and its prepared seek.
    Program,
    /// A portable prepared scanner group, including disabled groups.
    Group,
}

/// A source-scoped descriptor obtained from [`PrecompiledEagerRecords`].
///
/// IDs are local to this source. The range addresses the original program or
/// group archive; a host may instead map the ID to its own transport/storage.
/// This owns metadata only and never loads a payload.
#[derive(Clone)]
pub struct PrecompiledEagerRecord {
    source: Arc<Index>,
    kind: PrecompiledEagerRecordKind,
    id: usize,
}

impl fmt::Debug for PrecompiledEagerRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrecompiledEagerRecord")
            .field("kind", &self.kind)
            .field("id", &self.id)
            .field("range", &self.range())
            .finish()
    }
}

impl PrecompiledEagerRecord {
    /// Original source-local ID, never remapped for a scanner's pattern list.
    pub fn id(&self) -> usize {
        self.id
    }
    /// Archive containing this record.
    pub fn kind(&self) -> PrecompiledEagerRecordKind {
        self.kind
    }
    /// Byte range in the original archive, not a newly assembled subset.
    pub fn range(&self) -> Range<usize> {
        self.metadata().range.clone()
    }
    fn metadata(&self) -> &Record {
        match self.kind {
            PrecompiledEagerRecordKind::Program => &self.source.programs[self.id].record,
            PrecompiledEagerRecordKind::Group => &self.source.groups[self.id].record,
        }
    }
}

#[derive(Default)]
struct Payloads {
    programs: HashMap<usize, PrecompiledBytes>,
    groups: HashMap<usize, PrecompiledBytes>,
}
#[derive(Default)]
struct Counters {
    programs: AtomicUsize,
    bad_programs: AtomicUsize,
    groups: AtomicUsize,
    bad_groups: AtomicUsize,
}

/// Shared decoding observations for a record source. Missing payloads and cache
/// hits do not increment counters. Decoded engines live in `ScannerPatternCache`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrecompiledEagerRecordsStats {
    /// Successfully decoded ordinary programs.
    pub loaded_programs: usize,
    /// Requested ordinary programs rejected by validation.
    pub invalid_programs: usize,
    /// Successfully decoded prepared groups, including disabled groups.
    pub loaded_groups: usize,
    /// Requested prepared groups rejected by validation.
    pub invalid_groups: usize,
}

/// A source-scoped dictionary and individually supplied prepared Eager records.
///
/// Enable `precompiled-eager`. The versioned FREC001 index is independent of the
/// program and group codecs. Generate the index once from a checked
/// [`PrecompiledEager`] pair using [`PrecompiledEager::export_record_index`].
/// Opening checks manifest integrity, versions, bounds, keys and references;
/// supplied records retain their checksums and structural validation on use.
/// The manifest binds ordered groups to original dictionary IDs and expected
/// record checksums instead of hashing a complete program image on each load.
/// Obtain the manifest and payloads from a trusted producer. Corruption checks
/// do not authenticate them or prove the claimed regex semantics.
///
/// Hosts own I/O, compression, authentication and optional asynchronous fetching.
/// Supply records before requesting a scanner: missing or invalid data falls
/// back to normal compilation/Eager preparation. Clones share metadata,
/// immutable payload views and counters. No lock or I/O is added to searches.
#[derive(Clone)]
pub struct PrecompiledEagerRecords {
    index: Arc<Index>,
    payloads: Arc<RwLock<Payloads>>,
    counters: Arc<Counters>,
}
impl fmt::Debug for PrecompiledEagerRecords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrecompiledEagerRecords")
            .field("programs", &self.index.programs.len())
            .field("groups", &self.index.groups.len())
            .finish_non_exhaustive()
    }
}

impl PrecompiledEager {
    /// Export a versioned selective-import index for this checked pair.
    ///
    /// Persist it with the original archives or individually addressable records.
    /// The manifest includes no payload bytes. This does not change either
    /// existing archive or its format version. It supports legacy portable
    /// group checksums as well as the current format.
    pub fn export_record_index(&self) -> Result<Vec<u8>> {
        let mut out = MAGIC.to_vec();
        put_u32(&mut out, 0); // Manifest checksum, filled after serialization.
        put_u32(&mut out, super::PRECOMPILED_FORMAT_VERSION);
        put_u32(&mut out, super::PRECOMPILED_EAGER_FORMAT_VERSION);
        out.push(u8::from(self.groups.parallel_checksum()));
        let programs = self.programs.records();
        put_len(&mut out, programs.len())?;
        for (key, range, checksum) in programs {
            put_bytes(&mut out, &key)?;
            write_record(&mut out, range, checksum)?;
        }
        let groups = self.groups.records();
        put_len(&mut out, groups.len())?;
        for (ids, range, checksum) in groups {
            put_u32s(&mut out, &ids)?;
            write_record(&mut out, range, checksum)?;
        }
        if out.len() > MAX_PROGRAM_BYTES {
            return Err(PrecompiledError::ResourceLimit);
        }
        let checksum = group_prototype::record_checksum(&out[12..], true);
        out[8..12].copy_from_slice(&checksum.to_le_bytes());
        Ok(out)
    }
}

fn write_record(out: &mut Vec<u8>, range: Range<usize>, checksum: u32) -> Result<()> {
    put_len(out, range.start)?;
    put_len(out, range.len())?;
    put_u32(out, checksum);
    Ok(())
}
fn read_record(
    input: &mut Reader<'_>,
    max_record: usize,
    max_total: usize,
    total: &mut usize,
) -> Result<Record> {
    let start = input.count(max_total)?;
    let len = input.count(max_record)?;
    let checksum = input.u32()?;
    let end = start
        .checked_add(len)
        .ok_or(PrecompiledError::InvalidData)?;
    if len == 0 || end > max_total {
        return Err(PrecompiledError::InvalidData);
    }
    *total = total
        .checked_add(len)
        .ok_or(PrecompiledError::ResourceLimit)?;
    if *total > max_total {
        return Err(PrecompiledError::ResourceLimit);
    }
    Ok(Record {
        range: start..end,
        checksum,
    })
}

impl PrecompiledEagerRecords {
    /// Open a manifest without reading or retaining any program/group payload.
    pub fn from_index(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_PROGRAM_BYTES {
            return Err(PrecompiledError::ResourceLimit);
        }
        let mut input = Reader::new(bytes);
        if input.take(8)? != MAGIC {
            return Err(PrecompiledError::UnsupportedFormat);
        }
        let checksum = input.u32()?;
        if group_prototype::record_checksum(input.bytes, true) != checksum {
            return Err(PrecompiledError::InvalidData);
        }
        if input.u32()? != super::PRECOMPILED_FORMAT_VERSION
            || input.u32()? != super::PRECOMPILED_EAGER_FORMAT_VERSION
        {
            return Err(PrecompiledError::UnsupportedFormat);
        }
        let parallel = match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(PrecompiledError::InvalidData),
        };
        let count = input.count(100_000)?;
        let mut programs = Vec::new();
        let mut ids = HashMap::new();
        let mut last = Vec::new();
        let mut total = 0;
        for id in 0..count {
            let key = input.blob()?.to_vec();
            let settings = precompiled::key_settings(&key)?;
            if id != 0 && key <= last {
                return Err(PrecompiledError::InvalidData);
            }
            last.clone_from(&key);
            ids.insert(key, id as u32);
            programs.push(Program {
                record: read_record(&mut input, 16 << 20, MAX_PROGRAM_BYTES, &mut total)?,
                settings,
            });
        }
        let count = input.count(100_000)?;
        let mut groups = Vec::new();
        let mut group_ids = HashMap::new();
        total = 0;
        for id in 0..count {
            let key = input.u32s(65_535)?;
            let patterns = key.len();
            if key.iter().any(|&id| id as usize >= programs.len())
                || group_ids.insert(key, id as u32).is_some()
            {
                return Err(PrecompiledError::InvalidData);
            }
            groups.push(Group {
                record: read_record(&mut input, 64 << 20, MAX_GROUP_BYTES, &mut total)?,
                patterns,
            });
        }
        input.done()?;
        Ok(Self {
            index: Arc::new(Index {
                programs,
                ids,
                groups,
                group_ids,
                parallel,
            }),
            payloads: Arc::default(),
            counters: Arc::default(),
        })
    }

    fn descriptor(&self, kind: PrecompiledEagerRecordKind, id: usize) -> PrecompiledEagerRecord {
        PrecompiledEagerRecord {
            source: self.index.clone(),
            kind,
            id,
        }
    }
    /// Enumerate original program descriptors without loading their bytes.
    pub fn programs(&self) -> impl ExactSizeIterator<Item = PrecompiledEagerRecord> + '_ {
        (0..self.index.programs.len())
            .map(|id| self.descriptor(PrecompiledEagerRecordKind::Program, id))
    }
    /// Enumerate original group descriptors without loading their bytes.
    pub fn groups(&self) -> impl ExactSizeIterator<Item = PrecompiledEagerRecord> + '_ {
        (0..self.index.groups.len())
            .map(|id| self.descriptor(PrecompiledEagerRecordKind::Group, id))
    }
    /// Find a program with matching pattern text, compile settings and environment.
    pub fn program(
        &self,
        pattern: &str,
        config: &ScannerConfig,
        optimize_backtracking: bool,
    ) -> Option<PrecompiledEagerRecord> {
        let key = precompiled::key(
            pattern,
            PatternSettings::of(config, optimize_backtracking),
            &Environment::current(),
        )
        .ok()?;
        self.index
            .ids
            .get(&key)
            .map(|&id| self.descriptor(PrecompiledEagerRecordKind::Program, id as usize))
    }
    fn group_id(&self, patterns: &[&str], settings: PatternSettings) -> Option<usize> {
        let env = Environment::current();
        let ids: Option<Vec<_>> = patterns
            .iter()
            .map(|pattern| {
                self.index
                    .ids
                    .get(&precompiled::key(pattern, settings, &env).ok()?)
                    .copied()
            })
            .collect();
        self.index.group_ids.get(&ids?).map(|&id| id as usize)
    }
    /// Find an ordered group; pattern order and duplicates are significant.
    pub fn group(
        &self,
        patterns: &[&str],
        config: &ScannerConfig,
        optimize_backtracking: bool,
    ) -> Option<PrecompiledEagerRecord> {
        self.group_id(patterns, PatternSettings::of(config, optimize_backtracking))
            .map(|id| self.descriptor(PrecompiledEagerRecordKind::Group, id))
    }
    /// Whether this record has already been supplied, including through a clone.
    pub fn contains(&self, record: &PrecompiledEagerRecord) -> bool {
        Arc::ptr_eq(&self.index, &record.source) && self.payload(record).is_some()
    }
    /// Supply/replace an immutable payload view without copying it. A descriptor
    /// from another opened source, or a wrong byte length, is rejected. Checksums
    /// and structural decoding remain lazy. Replacing bytes does not invalidate
    /// engines already decoded in a pattern cache.
    pub fn insert(&self, record: &PrecompiledEagerRecord, bytes: PrecompiledBytes) -> Result<()> {
        if !Arc::ptr_eq(&self.index, &record.source) || bytes.len() != record.range().len() {
            return Err(PrecompiledError::InvalidData);
        }
        let mut payloads = self
            .payloads
            .write()
            .map_err(|_| PrecompiledError::InvalidData)?;
        match record.kind {
            PrecompiledEagerRecordKind::Program => {
                payloads.programs.insert(record.id, bytes);
            }
            PrecompiledEagerRecordKind::Group => {
                payloads.groups.insert(record.id, bytes);
            }
        }
        Ok(())
    }
    fn payload(&self, record: &PrecompiledEagerRecord) -> Option<PrecompiledBytes> {
        let payloads = self.payloads.read().ok()?;
        match record.kind {
            PrecompiledEagerRecordKind::Program => payloads.programs.get(&record.id),
            PrecompiledEagerRecordKind::Group => payloads.groups.get(&record.id),
        }
        .cloned()
    }
    /// Validate a supplied record without executing or retaining decoded code.
    ///
    /// Returns `false` when the payload has not been supplied, and an error for
    /// a foreign descriptor or invalid payload. This optional diagnostic leaves
    /// load counters unchanged. Scanner construction still validates on use;
    /// avoid preflighting the same data on a latency-sensitive startup path.
    pub fn validate_record(&self, record: &PrecompiledEagerRecord) -> Result<bool> {
        if !Arc::ptr_eq(&self.index, &record.source) {
            return Err(PrecompiledError::InvalidData);
        }
        let Some(bytes) = self.payload(record) else {
            return Ok(false);
        };
        match record.kind {
            PrecompiledEagerRecordKind::Program => {
                let program = &self.index.programs[record.id];
                precompiled::decode_record(&bytes, program.record.checksum, program.settings)?;
            }
            PrecompiledEagerRecordKind::Group => {
                self.decode_group(record, &bytes)?;
            }
        }
        Ok(true)
    }
    fn decode_group(
        &self,
        record: &PrecompiledEagerRecord,
        bytes: &[u8],
    ) -> Result<Option<crate::dfa_prefilter::Automata>> {
        if !matches!(bytes.first(), Some(0 | 5 | 6))
            || group_prototype::record_checksum(bytes, self.index.parallel)
                != record.metadata().checksum
        {
            return Err(PrecompiledError::InvalidData);
        }
        group_archive::decode(bytes, self.index.groups[record.id].patterns)
    }
    /// Return shared decode counters, excluding missing payloads and cache hits.
    pub fn stats(&self) -> PrecompiledEagerRecordsStats {
        PrecompiledEagerRecordsStats {
            loaded_programs: self.counters.programs.load(Ordering::Relaxed),
            invalid_programs: self.counters.bad_programs.load(Ordering::Relaxed),
            loaded_groups: self.counters.groups.load(Ordering::Relaxed),
            invalid_groups: self.counters.bad_groups.load(Ordering::Relaxed),
        }
    }
    pub(super) fn load_program(
        &self,
        pattern: &str,
        settings: PatternSettings,
        env: &Environment,
    ) -> Result<Option<(Arc<RegexType>, SharedSeek)>> {
        let Some(&id) = self
            .index
            .ids
            .get(&precompiled::key(pattern, settings, env)?)
        else {
            return Ok(None);
        };
        let record = self.descriptor(PrecompiledEagerRecordKind::Program, id as usize);
        let Some(bytes) = self.payload(&record) else {
            return Ok(None);
        };
        match precompiled::decode_record(&bytes, record.metadata().checksum, settings) {
            Ok(program) => {
                self.counters.programs.fetch_add(1, Ordering::Relaxed);
                Ok(Some(program))
            }
            Err(error) => {
                self.counters.bad_programs.fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }
    pub(super) fn load_group(
        &self,
        patterns: &[&str],
        settings: PatternSettings,
    ) -> Option<AutomataCell> {
        let record = self.descriptor(
            PrecompiledEagerRecordKind::Group,
            self.group_id(patterns, settings)?,
        );
        let bytes = self.payload(&record)?;
        let result = self.decode_group(&record, &bytes);
        match result {
            Ok(automata) => {
                self.counters.groups.fetch_add(1, Ordering::Relaxed);
                Some(AutomataCell::prepared(automata))
            }
            Err(_) => {
                self.counters.bad_groups.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }
}

impl ScannerPatternCache {
    /// Attach a selective record source. Program sources retain insertion
    /// order; this replaces the prepared-group source, matching the archive API.
    /// Already decoded entries stay cached. Sources survive `clear`; remove them
    /// with `clear_precompiled`. Only Eager consumes prepared groups.
    pub fn add_precompiled_eager_records(&mut self, source: PrecompiledEagerRecords) {
        self.precompiled
            .push(precompiled::ProgramSource::Records(source.clone()));
        self.group_source = Some(group_prototype::GroupSourceRef::Records(source));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PrecompiledEager {
        PrecompiledEager::from_bytes(
            &include_bytes!("../../tests/fixtures/precompiled-eager/patterns.programs")[..],
            &include_bytes!("../../tests/fixtures/precompiled-eager/patterns.portable")[..],
        )
        .unwrap()
    }
    fn repair(bytes: &mut [u8]) {
        let checksum = group_prototype::record_checksum(&bytes[12..], true);
        bytes[8..12].copy_from_slice(&checksum.to_le_bytes());
    }
    #[test]
    fn repaired_checksums_still_require_valid_versions_keys_references_and_bounds() {
        let bytes = fixture().export_record_index().unwrap();
        let mut input = Reader::new(&bytes[21..]);
        let count = input.u32().unwrap() as usize;
        let mut first_record = 0;
        for id in 0..count {
            input.blob().unwrap();
            if id == 0 {
                first_record = bytes.len() - input.bytes.len();
            }
            input.take(12).unwrap();
        }
        let group_count = input.u32().unwrap();
        assert!(group_count > 0);
        let group_start = bytes.len() - input.bytes.len();
        let ids = input.u32().unwrap();
        assert!(ids > 0);
        let bad_words = [
            (12, 999),                   // Program codec version.
            (16, 999),                   // Group codec version.
            (21, u32::MAX),              // Program count.
            (first_record, u32::MAX),    // Program offset.
            (first_record + 4, 0),       // Empty program record.
            (group_start + 4, u32::MAX), // Group dictionary reference.
        ];
        for (at, value) in bad_words {
            let mut changed = bytes.clone();
            changed[at..at + 4].copy_from_slice(&value.to_le_bytes());
            repair(&mut changed);
            assert!(
                PrecompiledEagerRecords::from_index(&changed).is_err(),
                "offset {at}"
            );
        }
        for (at, value) in [(20, 2), (33, 255)] {
            // Checksum kind and syntax key tag.
            let mut changed = bytes.clone();
            changed[at] = value;
            repair(&mut changed);
            assert!(PrecompiledEagerRecords::from_index(&changed).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        repair(&mut trailing);
        assert!(PrecompiledEagerRecords::from_index(&trailing).is_err());
    }
}
