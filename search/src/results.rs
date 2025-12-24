//! Search results types.

use keva_core::types::Key;
use nucleo::Snapshot;
use std::collections::HashSet;

/// Search results snapshot that provides zero-copy iteration.
///
/// Borrows from the SearchEngine. Use `iter()` to iterate over
/// matched keys without collecting.
pub struct SearchResults<'a> {
    pub(crate) snapshot: &'a Snapshot<Key>,
    pub(crate) tombstones: &'a HashSet<Key>,
}

impl<'a> SearchResults<'a> {
    /// Iterates over matched keys, filtering out tombstoned entries.
    pub fn iter(&self) -> impl Iterator<Item = &Key> + '_ {
        self.snapshot
            .matched_items(..)
            .filter(|item| !self.tombstones.contains(item.data))
            .map(|item| item.data)
    }
}
