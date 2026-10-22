//! Indexed, versioned scanner programs. See ADR-018 for the compatibility contract.
mod schema;
mod validate;
mod wire;

use super::*;
use std::ops::Range;
use wire::{Reader, Wire};

/// Version of the compiled program format, independent of the crate version.
/// A bytecode, Unicode-data, seek, or optimizer semantic change must bump it.
pub const PRECOMPILED_FORMAT_VERSION: u32 = 2;
const MAGIC: &[u8; 8] = b"FERRONI\0";
const MAX_BLOB: usize = 256 * 1024 * 1024;
const MAX_PROGRAM: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
type Result<T> = std::result::Result<T, PrecompiledError>;

/// An incompatible, malformed, oversized, or nonportable precompiled archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrecompiledError {
    /// The magic or bytecode format version is not supported.
    UnsupportedFormat,
    /// The index, checksum, program, or seek failed validation.
    InvalidData,
    /// A format size, allocation, or program complexity bound was exceeded.
    ResourceLimit,
    /// The program uses process-local callbacks and cannot be exported.
    NonportablePattern,
}
impl fmt::Display for PrecompiledError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedFormat => "unsupported Ferroni precompiled format",
            Self::InvalidData => "invalid Ferroni precompiled data",
            Self::ResourceLimit => "Ferroni precompiled resource limit exceeded",
            Self::NonportablePattern => "pattern contains process-local callouts",
        })
    }
}
impl std::error::Error for PrecompiledError {}

/// An owned archive indexed by pattern text and compile settings. Opening it
/// checks only its header and index; programs are decoded and validated one at
/// a time, when requested. Clones share the bytes and index.
///
/// Enable the `precompiled` Cargo feature to use this API. Export with
/// [`ScannerPatternCache::export_precompiled`] and attach with
/// [`ScannerPatternCache::add_precompiled`]. Use archives from a trusted
/// producer: validation checks structure, bounds, and corruption, not whether a
/// producer's program implements the pattern text it claims to implement.
/// Search limits continue to apply as they do to compiled patterns.
#[derive(Clone)]
pub struct PrecompiledPatterns {
    bytes: Arc<[u8]>,
    index: Arc<HashMap<Vec<u8>, Entry>>,
}
#[derive(Clone)]
struct Entry {
    range: Range<usize>,
    checksum: u32,
}
impl fmt::Debug for PrecompiledPatterns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrecompiledPatterns")
            .field("len", &self.len())
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

// Retained on each compiled entry, so exporting after a process-global setting
// change cannot label old bytecode with the new environment. Callout registries
// need no fingerprint: those programs are deliberately not portable.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Environment {
    fold: u32,
    captures: i32,
    nesting: u32,
    nodes: u32,
    prefilter: bool,
    properties: Vec<(Vec<u8>, Vec<u32>)>,
}
impl Environment {
    pub(super) fn current() -> Self {
        Self {
            fold: crate::regcomp::onig_get_default_case_fold_flag(),
            captures: crate::regparse::capture_num_limit(),
            nesting: crate::regparse::onig_get_parse_depth_limit(),
            nodes: crate::regparse::onig_get_ast_node_limit(),
            prefilter: cfg!(feature = "dfa-prefilter"),
            properties: crate::unicode::precompiled_user_properties(),
        }
    }
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.fold.write(out)?;
        self.captures.write(out)?;
        self.nesting.write(out)?;
        self.nodes.write(out)?;
        self.prefilter.write(out)?;
        self.properties.write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            fold: Wire::read(input)?,
            captures: Wire::read(input)?,
            nesting: Wire::read(input)?,
            nodes: Wire::read(input)?,
            prefilter: Wire::read(input)?,
            properties: Wire::read(input)?,
        })
    }
}
fn key(pattern: &str, settings: PatternSettings, env: &Environment) -> Result<Vec<u8>> {
    let mut key = Vec::new();
    settings.options.bits().write(&mut key)?;
    syntax_tag(settings.syntax).write(&mut key)?;
    settings.optimize_backtracking.write(&mut key)?;
    settings.prefilter.write(&mut key)?;
    env.write(&mut key)?;
    pattern.to_owned().write(&mut key)?;
    Ok(key)
}
fn syntax_tag(syntax: Syntax) -> u8 {
    match syntax {
        Syntax::Oniguruma => 0,
        Syntax::Asis => 1,
        Syntax::PosixBasic => 2,
        Syntax::PosixExtended => 3,
        Syntax::Emacs => 4,
        Syntax::Grep => 5,
        Syntax::GnuRegex => 6,
        Syntax::Java => 7,
        Syntax::Perl => 8,
        Syntax::PerlNg => 9,
        Syntax::Ruby => 10,
        Syntax::Python => 11,
    }
}
fn check_key(bytes: &[u8]) -> Result<()> {
    let mut input = Reader::new(bytes);
    OnigOptionType::from_bits(u32::read(&mut input)?).ok_or(PrecompiledError::InvalidData)?;
    if u8::read(&mut input)? > 11 {
        return Err(PrecompiledError::InvalidData);
    }
    bool::read(&mut input)?;
    bool::read(&mut input)?;
    Environment::read(&mut input)?;
    String::read(&mut input)?;
    input.done()
}
// A corruption check, not authentication. The structural validator still runs
// after it, including for a maliciously recomputed checksum. FNV-1a over
// little-endian 64-bit words with an extra shift per word, then the tail bytes,
// folded to 32 bits: every load checks its program, and a byte-wise hash made
// that a third of the load time (format version 1).
fn checksum(bytes: &[u8]) -> u32 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let (words, tail) = bytes.as_chunks::<8>();
    for word in words {
        hash = (hash ^ u64::from_le_bytes(*word)).wrapping_mul(PRIME);
        hash ^= hash >> 29;
    }
    for &byte in tail {
        hash = (hash ^ u64::from(byte)).wrapping_mul(PRIME);
    }
    (hash ^ (hash >> 32)) as u32
}
impl PrecompiledPatterns {
    /// Open an archive, retaining its bytes and indexing its entries. No regex
    /// or pre-filter seek is decoded here. Unsupported versions and malformed
    /// indexes return an error; a bad individual program is reported by
    /// [`validate_pattern`](Self::validate_pattern), or ignored by the cache.
    pub fn from_bytes(bytes: impl Into<Arc<[u8]>>) -> Result<Self> {
        let bytes = bytes.into();
        if bytes.len() > MAX_BLOB {
            return Err(PrecompiledError::ResourceLimit);
        }
        let mut input = Reader::new(&bytes);
        if input.take(8)? != MAGIC || u32::read(&mut input)? != PRECOMPILED_FORMAT_VERSION {
            return Err(PrecompiledError::UnsupportedFormat);
        }
        let index_len = usize::read(&mut input)?;
        let mut index_input = Reader::new(input.take(index_len)?);
        let count = usize::read(&mut index_input)?;
        if count > MAX_ENTRIES {
            return Err(PrecompiledError::ResourceLimit);
        }
        let data_start = bytes.len() - input.bytes.len();
        let mut index = HashMap::new();
        let mut expected = data_start;
        for _ in 0..count {
            let key = Vec::<u8>::read(&mut index_input)?;
            check_key(&key)?;
            let offset = usize::read(&mut index_input)?;
            let len = usize::read(&mut index_input)?;
            let checksum = u32::read(&mut index_input)?;
            if len > MAX_PROGRAM {
                return Err(PrecompiledError::ResourceLimit);
            }
            let start = data_start
                .checked_add(offset)
                .ok_or(PrecompiledError::InvalidData)?;
            let end = start
                .checked_add(len)
                .ok_or(PrecompiledError::InvalidData)?;
            if start != expected || end > bytes.len() || len == 0 {
                return Err(PrecompiledError::InvalidData);
            }
            expected = end;
            if index
                .insert(
                    key,
                    Entry {
                        range: start..end,
                        checksum,
                    },
                )
                .is_some()
            {
                return Err(PrecompiledError::InvalidData);
            }
        }
        index_input.done()?;
        if expected != bytes.len() {
            return Err(PrecompiledError::InvalidData);
        }
        Ok(Self {
            bytes,
            index: Arc::new(index),
        })
    }
    /// Number of indexed programs, including ones not decoded yet.
    pub fn len(&self) -> usize {
        self.index.len()
    }
    /// Whether the archive indexes no programs.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
    /// Decode and validate the matching entry without retaining it. Returns
    /// `false` if the text, configuration, or process-wide compile settings do
    /// not match an entry. Use this for diagnostics; cache misses automatically
    /// fall back to compilation on an entry error.
    ///
    /// `optimize_backtracking` selects the experimental rewrite mode used by
    /// [`Scanner::with_backtracking_optimization_and_pattern_cache`].
    pub fn validate_pattern(
        &self,
        pattern: &str,
        config: &ScannerConfig,
        optimize_backtracking: bool,
    ) -> Result<bool> {
        self.load(
            pattern,
            PatternSettings::of(config, optimize_backtracking),
            &Environment::current(),
        )
        .map(|reg| reg.is_some())
    }
    pub(super) fn load(
        &self,
        pattern: &str,
        settings: PatternSettings,
        env: &Environment,
    ) -> Result<Option<(Arc<RegexType>, SharedSeek)>> {
        let Some(entry) = self.index.get(&key(pattern, settings, env)?) else {
            return Ok(None);
        };
        let bytes = &self.bytes[entry.range.clone()];
        if checksum(bytes) != entry.checksum {
            return Err(PrecompiledError::InvalidData);
        }
        let mut input = Reader::new(bytes);
        let reg = schema::read_reg(&mut input, settings)?;
        let seek = schema::read_seek(&mut input)?;
        input.done()?;
        Ok(Some((Arc::new(reg), seek)))
    }
}
impl ScannerPatternCache {
    /// Attach a source to consult on cache misses. Existing decoded entries
    /// stay in use. Sources are tried in insertion order; absent, incompatible,
    /// or corrupt entries fall back to the next source and then the compiler.
    /// Attaching a source never compiles or decodes a pattern.
    ///
    /// ```
    /// # #[cfg(feature = "precompiled")] {
    /// use ferroni::{Scanner, ScannerConfig, ScannerPatternCache};
    /// use ferroni::scanner::PrecompiledPatterns;
    /// let config = ScannerConfig::default();
    /// let mut build = ScannerPatternCache::new();
    /// Scanner::with_pattern_cache(&[r"\w+", r"\d+"], &config, &mut build)?;
    /// let blob = build.export_precompiled()?;
    /// let mut runtime = ScannerPatternCache::new();
    /// runtime.add_precompiled(PrecompiledPatterns::from_bytes(blob)?);
    /// assert!(runtime.is_empty()); // Only the index has been loaded.
    /// let scanner = Scanner::with_pattern_cache(&[r"\d+"], &config, &mut runtime)?;
    /// assert_eq!(runtime.len(), 1);
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_precompiled(&mut self, source: PrecompiledPatterns) {
        self.precompiled.push(source);
    }
    /// Remove attached archives, keeping decoded programs and existing scanners.
    /// [`clear`](Self::clear) clears decoded programs and automata but retains
    /// these sources, so subsequent misses can load them again.
    pub fn clear_precompiled(&mut self) {
        self.precompiled.clear();
    }
    /// Export the cache's decoded programs in deterministic pattern/settings
    /// order. Callout programs are omitted: their callbacks belong to the
    /// producing process. Missing and runtime-resolved patterns compile normally.
    /// The archive carries bytecode, compact maps and seeks; lazy automata are
    /// built at runtime. Export can fail if a format resource bound is exceeded.
    pub fn export_precompiled(&self) -> Result<Vec<u8>> {
        let mut programs = Vec::new();
        for (settings, patterns) in &self.compiled {
            for (pattern, cached) in patterns {
                if cached.reg.extp.as_ref().is_some_and(|e| e.callout_num != 0) {
                    continue;
                }
                let key = key(pattern, *settings, &cached.environment)?;
                let mut bytes = Vec::new();
                schema::write_reg(&cached.reg, &mut bytes)?;
                schema::write_seek(&cached.seek, &mut bytes)?;
                if bytes.len() > MAX_PROGRAM {
                    return Err(PrecompiledError::ResourceLimit);
                }
                programs.push((key, bytes));
            }
        }
        programs.sort_by(|a, b| a.0.cmp(&b.0));
        // Warmup changes automata scheduling, not compiled programs. Two cache
        // settings that differ only in warmup export one identical program.
        programs.dedup_by(|a, b| a.0 == b.0);
        if programs.len() > MAX_ENTRIES {
            return Err(PrecompiledError::ResourceLimit);
        }
        let mut index = Vec::new();
        programs.len().write(&mut index)?;
        let mut offset = 0usize;
        for (key, bytes) in &programs {
            key.write(&mut index)?;
            offset.write(&mut index)?;
            bytes.len().write(&mut index)?;
            checksum(bytes).write(&mut index)?;
            offset = offset
                .checked_add(bytes.len())
                .ok_or(PrecompiledError::ResourceLimit)?;
        }
        if index.len().saturating_add(offset).saturating_add(16) > MAX_BLOB {
            return Err(PrecompiledError::ResourceLimit);
        }
        let mut out = MAGIC.to_vec();
        PRECOMPILED_FORMAT_VERSION.write(&mut out)?;
        index.len().write(&mut out)?;
        out.extend(index);
        for (_, bytes) in programs {
            out.extend(bytes);
        }
        Ok(out)
    }
}

#[cfg(test)]
thread_local! { pub(super) static COMPILES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multibyte_optional_regression() {
        let config = ScannerConfig::default();
        let source = PrecompiledPatterns::from_bytes(archive(&["(?:雪+)??霜"], &config)).unwrap();
        let entry = source.index.values().next().unwrap();
        let mut input = Reader::new(&source.bytes[entry.range.clone()]);
        schema::read_reg(&mut input, PatternSettings::of(&config, false)).expect("regex");
        schema::read_seek(&mut input).expect("seek");
    }
    #[test]
    fn format_v2_literal_schema_is_pinned() {
        let settings = PatternSettings::of(&ScannerConfig::default(), false);
        let (reg, seek) = settings.compile("a").unwrap();
        let mut bytes = Vec::new();
        schema::write_reg(&reg, &mut bytes).unwrap();
        assert_eq!(PRECOMPILED_FORMAT_VERSION, 2);
        assert_eq!((checksum(&bytes), bytes.len()), (3_233_327_443, 240));
        let mut bytes = Vec::new();
        schema::write_seek(&seek, &mut bytes).unwrap();
        #[cfg(feature = "dfa-prefilter")]
        assert_eq!(bytes, [2, 4, 0, 0, 0, 1, 1, 1, 97]);
        #[cfg(not(feature = "dfa-prefilter"))]
        assert_eq!(bytes, [0]);
    }
    fn archive(patterns: &[&str], config: &ScannerConfig) -> Vec<u8> {
        let mut cache = ScannerPatternCache::new();
        Scanner::with_pattern_cache(patterns, config, &mut cache).unwrap();
        cache.export_precompiled().unwrap()
    }
    #[test]
    fn indexes_without_decoding_and_loads_only_requested_programs() {
        let config = ScannerConfig::default();
        let source = PrecompiledPatterns::from_bytes(archive(
            &[r"\w+", r"\d+", "alpha|beta|gamma|delta"],
            &config,
        ))
        .unwrap();
        assert_eq!(source.len(), 3);
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled(source);
        assert!(cache.is_empty());
        COMPILES.with(|n| n.set(0));
        let mut scanner = Scanner::with_pattern_cache(&[r"\d+"], &config, &mut cache).unwrap();
        assert_eq!(COMPILES.with(|n| n.get()), 0);
        assert_eq!(cache.len(), 1);
        let m = scanner
            .find_next_match("a42", 0, ScannerFindOptions::NONE)
            .unwrap();
        assert_eq!((m.captures()[0].start, m.captures()[0].end), (1, 3));
        cache.clear();
        Scanner::with_pattern_cache(&[r"\d+"], &config, &mut cache).unwrap();
        assert_eq!(COMPILES.with(|n| n.get()), 0);
        cache.clear();
        cache.clear_precompiled();
        Scanner::with_pattern_cache(&[r"\d+"], &config, &mut cache).unwrap();
        assert_eq!(COMPILES.with(|n| n.get()), 1);
        assert!(
            scanner
                .find_next_match("42", 0, ScannerFindOptions::NONE)
                .is_some()
        );
    }
    #[test]
    fn corrupt_unused_program_stays_lazy_and_requested_program_falls_back() {
        let config = ScannerConfig::default();
        let mut bytes = archive(&["abc", "def"], &config);
        let source = PrecompiledPatterns::from_bytes(bytes.clone()).unwrap();
        let damaged = source
            .index
            .get(
                &key(
                    "def",
                    PatternSettings::of(&config, false),
                    &Environment::current(),
                )
                .unwrap(),
            )
            .unwrap()
            .range
            .start;
        bytes[damaged] ^= 0x40;
        let source = PrecompiledPatterns::from_bytes(bytes).unwrap();
        assert_eq!(
            source.validate_pattern("def", &config, false),
            Err(PrecompiledError::InvalidData)
        );
        assert_eq!(source.validate_pattern("abc", &config, false), Ok(true));
        let mut cache = ScannerPatternCache::new();
        cache.add_precompiled(source);
        COMPILES.with(|n| n.set(0));
        Scanner::with_pattern_cache(&["abc"], &config, &mut cache).unwrap();
        assert_eq!(COMPILES.with(|n| n.get()), 0);
        Scanner::with_pattern_cache(&["def"], &config, &mut cache).unwrap();
        assert_eq!(COMPILES.with(|n| n.get()), 1);
    }
    #[test]
    fn compile_modes_and_environment_do_not_alias() {
        let config = ScannerConfig::default();
        let source = PrecompiledPatterns::from_bytes(archive(&["a+"], &config)).unwrap();
        assert_eq!(
            source.validate_pattern("a+", &config.clone().options(ONIG_OPTION_IGNORECASE), false),
            Ok(false)
        );
        assert_eq!(
            source.validate_pattern("a+", &config.clone().syntax(ScannerSyntax::Ruby), false),
            Ok(false)
        );
        assert_eq!(source.validate_pattern("a+", &config, true), Ok(false));
        assert_eq!(
            source.validate_pattern("a+", &config.clone().prefilter_warmup(17), false),
            Ok(true)
        );
        let mut env = Environment::current();
        env.fold ^= 1;
        assert!(
            source
                .load("a+", PatternSettings::of(&config, false), &env)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn deterministic_export_and_rewrite_reports() {
        let config = ScannerConfig::default();
        assert_eq!(
            archive(&["abc", "def"], &config),
            archive(&["def", "abc"], &config)
        );
        let patterns = [r"((\d+)_?)+\.\d+\b", "(a+)+$"];
        let mut cache = ScannerPatternCache::new();
        let before = Scanner::with_backtracking_optimization_and_pattern_cache(
            &patterns, &config, &mut cache,
        )
        .unwrap();
        let source = PrecompiledPatterns::from_bytes(cache.export_precompiled().unwrap()).unwrap();
        for p in patterns {
            assert_eq!(source.validate_pattern(p, &config, true), Ok(true), "{p}");
        }
        let mut runtime = ScannerPatternCache::new();
        runtime.add_precompiled(source);
        COMPILES.with(|n| n.set(0));
        let after = Scanner::with_backtracking_optimization_and_pattern_cache(
            &patterns,
            &config,
            &mut runtime,
        )
        .unwrap();
        assert_eq!(COMPILES.with(|n| n.get()), 0);
        assert_eq!(before.warnings(), after.warnings());
        assert_eq!(
            before.backtracking_rewrites(),
            after.backtracking_rewrites()
        );
    }
    #[test]
    fn rejects_version_truncation_and_oversized_index() {
        let config = ScannerConfig::default();
        let bytes = archive(&["abc"], &config);
        for len in 0..bytes.len() {
            assert!(
                PrecompiledPatterns::from_bytes(bytes[..len].to_vec()).is_err(),
                "{len}"
            );
        }
        let mut version = bytes.clone();
        version[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            PrecompiledPatterns::from_bytes(version).unwrap_err(),
            PrecompiledError::UnsupportedFormat
        );
        let mut count = bytes;
        count[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            PrecompiledPatterns::from_bytes(count).unwrap_err(),
            PrecompiledError::ResourceLimit
        );
    }
    #[test]
    fn binary_and_bmh_maps_round_trip_compactly() {
        for map in [
            [0; 256],
            [1; 256],
            std::array::from_fn(|i| (i % 2) as u8),
            [7; 256],
            std::array::from_fn(|i| i as u8),
        ] {
            let mut out = Vec::new();
            wire::write_map(&map, &mut out).unwrap();
            assert_eq!(wire::read_map(&mut Reader::new(&out)).unwrap(), map);
            if map.iter().all(|b| *b <= 1) {
                assert_eq!(out.len(), 33);
            }
        }
    }
}
