//! Stable ownership and compatibility boundary for prepared Eager archives.

use super::{
    PrecompiledBytes, PrecompiledError, PrecompiledPatterns, ScannerPatternCache, group_prototype,
};
use std::{fmt, sync::Arc};

/// Current portable Eager group format, independent of the crate version.
///
/// Instruction meaning, byte classes, checksums, or group layout changes that
/// cannot preserve this contract require a new format version. The accompanying
/// programs have their own [`super::PRECOMPILED_FORMAT_VERSION`].
pub const PRECOMPILED_EAGER_FORMAT_VERSION: u32 = 3;

/// A matched pair of compiled programs and portable, prepared scanner groups.
///
/// Enable `precompiled-eager` to export once and reuse Eager preparation across
/// process starts or native/WebAssembly consumers. This contains no pointers,
/// dependency-private objects, machine code, or mutable search caches.
///
/// Opening checks the indexes and binds the groups to the exact program archive.
/// Individual programs and groups decode and validate when scanners request
/// them. Unsupported, missing, or invalid groups retain normal Eager fallback.
/// Keep original pattern strings and compatible scanner/compile settings.
/// Obtain both archives from a trusted producer: structural validation and
/// corruption checks do not authenticate a publisher or prove regex semantics.
///
/// Clones share bytes, indexes, and the group load counters. Already constructed
/// scanners keep their own search scratch space and warm transition caches.
#[derive(Clone)]
pub struct PrecompiledEager {
    programs: PrecompiledPatterns,
    groups: Arc<group_prototype::GroupSource>,
}

/// Group loading observations shared by clones of one [`PrecompiledEager`].
///
/// Cache hits and absent groups do not increment either counter. These count
/// archive decodes and invalid-record fallbacks, not searches or scanner count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrecompiledEagerStats {
    /// Successfully decoded group records, including prepared disabled groups.
    pub loaded_groups: usize,
    /// Requested records rejected by a checksum or structural validation.
    pub invalid_groups: usize,
}

impl PrecompiledEager {
    /// Open uncompressed program and portable-group archives as one bound pair.
    ///
    /// Accepts current FPRT003 and legacy FPRT001/FPRT002 portable groups. An
    /// unsupported format or mismatched pair returns an error without changing
    /// a cache. Leave the cache without this source to compile normally.
    /// Compression, HTTP, digests, and any filesystem access belong to the host.
    pub fn from_bytes(
        programs: impl Into<Arc<[u8]>>,
        groups: impl Into<Arc<[u8]>>,
    ) -> Result<Self, PrecompiledError> {
        Self::from_shared_bytes(
            PrecompiledBytes::from(programs.into()),
            PrecompiledBytes::from(groups.into()),
        )
    }

    /// Open shared program and group views without copying their backing bytes.
    ///
    /// Both views may refer to sections of one host-owned decoded container.
    /// Index, binding and record validation match [`Self::from_bytes`]; archive
    /// versions and the returned byte slices retain the same contract.
    pub fn from_shared_bytes(
        programs: PrecompiledBytes,
        groups: PrecompiledBytes,
    ) -> Result<Self, PrecompiledError> {
        let programs = PrecompiledPatterns::from_shared_bytes(programs)?;
        let groups = group_prototype::GroupSource::open_portable(groups, &programs)?;
        Ok(Self {
            programs,
            groups: Arc::new(groups),
        })
    }

    /// The ordinary program archive to persist or transport with [`Self::group_bytes`].
    pub fn program_bytes(&self) -> &[u8] {
        self.programs.archive_bytes()
    }

    /// The portable group archive, bound to [`Self::program_bytes`].
    pub fn group_bytes(&self) -> &[u8] {
        self.groups.archive_bytes()
    }

    /// Number of indexed scanner groups, including groups not decoded yet.
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Whether this archive indexes no prepared scanner groups.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Validate every group checksum and instruction record without executing it.
    ///
    /// This optional preflight does not retain decoded groups, change load
    /// counters, or validate individual ordinary programs; those retain their
    /// normal lazy validation. Avoid repeating this work on a startup path.
    pub fn validate_groups(&self) -> Result<(), PrecompiledError> {
        self.groups.validate_groups()
    }

    /// Return shared counters for this archive's group loads and invalid records.
    pub fn stats(&self) -> PrecompiledEagerStats {
        let (loaded_groups, invalid_groups) = self.groups.load_stats();
        PrecompiledEagerStats {
            loaded_groups,
            invalid_groups,
        }
    }
}

impl fmt::Debug for PrecompiledEager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrecompiledEager")
            .field("programs", &self.programs.len())
            .field("groups", &self.len())
            .finish()
    }
}

impl ScannerPatternCache {
    /// Export the cache's compiled programs and prepared portable groups together.
    ///
    /// Construct every desired ordered pattern list with Eager acceleration
    /// first. Only already prepared groups are exported; this does not enumerate
    /// a grammar or promise full coverage. Callouts and runtime-resolved patterns
    /// keep ordinary compilation fallback. Export ordering is deterministic.
    pub fn export_precompiled_eager(&self) -> Result<PrecompiledEager, PrecompiledError> {
        PrecompiledEager::from_bytes(
            self.export_precompiled()?,
            self.export_group_prototype(group_prototype::GroupKind::Portable)?,
        )
    }

    /// Attach both parts of a checked pair for later scanner construction.
    ///
    /// Programs join the existing archive sources in insertion order. This
    /// replaces the previous prepared-group source; decoded programs and groups
    /// stay cached. Only Eager consumes group records; Auto and Off retain their
    /// existing behavior. [`Self::clear`] keeps sources, while
    /// [`Self::clear_precompiled`] removes all attached program/group sources.
    ///
    /// ```
    /// # #[cfg(feature = "precompiled-eager")] {
    /// use ferroni::prelude::*;
    /// let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
    /// let patterns = [r"\bword\b", r"(\d+)"];
    /// let mut producer = ScannerPatternCache::new();
    /// Scanner::with_pattern_cache(&patterns, &config, &mut producer)?;
    /// let prepared = producer.export_precompiled_eager()?;
    /// let mut consumer = ScannerPatternCache::new();
    /// consumer.add_precompiled_eager(prepared.clone());
    /// let mut scanner = Scanner::with_pattern_cache(&patterns, &config, &mut consumer)?;
    /// assert_eq!(scanner.find_next_match("42", 0, ScannerFindOptions::NONE).unwrap().index, 1);
    /// assert_eq!(prepared.stats().invalid_groups, 0);
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_precompiled_eager(&mut self, archive: PrecompiledEager) {
        self.precompiled.push(archive.programs);
        self.group_source = Some(archive.groups);
    }
}
