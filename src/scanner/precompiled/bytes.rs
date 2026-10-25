//! Shared storage for indexed archives without copying host-owned containers.

use super::PrecompiledError;
use std::{
    fmt,
    ops::{Deref, Range},
    sync::Arc,
};

#[derive(Clone)]
enum Storage {
    Bytes(Arc<[u8]>),
    Owner(Arc<dyn AsRef<[u8]> + Send + Sync>),
}

/// A shared, immutable byte image or a checked view into one.
///
/// Hosts may retain a decoded container, an existing allocation, or another
/// owned byte image without copying its program and group sections. Clones and
/// [`Self::slice`] retain the same owner. This adds no transport or file I/O.
/// The owner's `AsRef` must keep returning the same immutable byte image for
/// its lifetime. Any storage-specific safety requirements remain with its host.
#[derive(Clone)]
pub struct PrecompiledBytes {
    storage: Storage,
    range: Range<usize>,
}

impl PrecompiledBytes {
    /// Retain an owned byte image without copying its contents.
    ///
    /// `Vec<u8>` and `Box<[u8]>` are accepted. For an existing shared
    /// allocation, use [`Self::from_shared_owner`]. The owner must expose a
    /// consistent, immutable image while retained. Opening still validates it.
    pub fn new(owner: impl AsRef<[u8]> + Send + Sync + 'static) -> Self {
        Self::from_shared_owner(Arc::new(owner))
    }

    /// Retain an existing shared byte owner, such as `Arc<Vec<u8>>`.
    ///
    /// The image must remain consistent and immutable while any view exists.
    pub fn from_shared_owner<T: AsRef<[u8]> + Send + Sync + 'static>(owner: Arc<T>) -> Self {
        let len = owner.as_ref().as_ref().len();
        Self {
            storage: Storage::Owner(owner),
            range: 0..len,
        }
    }

    /// Retain a subrange, relative to this view, without copying any bytes.
    ///
    /// Reversed or out-of-bounds ranges return [`PrecompiledError::InvalidData`].
    pub fn slice(&self, range: Range<usize>) -> Result<Self, PrecompiledError> {
        if range.start > range.end || range.end > self.len() {
            return Err(PrecompiledError::InvalidData);
        }
        Ok(Self {
            storage: self.storage.clone(),
            range: self.range.start + range.start..self.range.start + range.end,
        })
    }
}

impl From<Vec<u8>> for PrecompiledBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl From<Arc<[u8]>> for PrecompiledBytes {
    fn from(bytes: Arc<[u8]>) -> Self {
        let len = bytes.len();
        Self {
            storage: Storage::Bytes(bytes),
            range: 0..len,
        }
    }
}

impl AsRef<[u8]> for PrecompiledBytes {
    fn as_ref(&self) -> &[u8] {
        let bytes: &[u8] = match &self.storage {
            Storage::Bytes(bytes) => bytes,
            Storage::Owner(owner) => owner.as_ref().as_ref(),
        };
        &bytes[self.range.clone()]
    }
}

impl Deref for PrecompiledBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl fmt::Debug for PrecompiledBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrecompiledBytes")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_views_share_storage_and_check_relative_bounds() {
        let owner = Arc::new(vec![1, 2, 3, 4, 5]);
        let weak = Arc::downgrade(&owner);
        let image = PrecompiledBytes::from_shared_owner(Arc::clone(&owner));
        let view = image.slice(1..5).unwrap().slice(1..3).unwrap();
        assert_eq!(&*view, &[3, 4]);
        assert_eq!(view.as_ptr(), owner[2..].as_ptr());
        assert!(image.slice(Range { start: 4, end: 3 }).is_err());
        assert!(image.slice(0..6).is_err());
        assert!(image.slice(0..usize::MAX).is_err());
        assert!(image.slice(5..5).unwrap().is_empty());
        drop(owner);
        drop(image);
        assert!(weak.upgrade().is_some());
        drop(view);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn existing_arc_slice_is_retained_without_copying() {
        let bytes: Arc<[u8]> = Arc::from([1, 2, 3]);
        let image = PrecompiledBytes::from(Arc::clone(&bytes));
        assert_eq!(image.as_ptr(), bytes.as_ptr());
        assert_eq!(format!("{image:?}"), "PrecompiledBytes { len: 3, .. }");
    }
}
