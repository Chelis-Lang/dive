//! Opt-in native safety storage. Exact identities are separate from receiver history.

use std::collections::HashMap;
use std::path::PathBuf;

use dive_core::{Error, Handle, Heap};

/// Storage policies for one native safety search. These do not bound total memory.
#[derive(Clone, Debug, Default)]
pub struct Storage {
    /// Resolve state-key collisions using complete mathematical state identities.
    pub exact: bool,
    /// Resident predecessor-log capacity before transient spill.
    pub predecessor_memory_bytes: Option<usize>,
    /// Parent directory for private predecessor files.
    pub predecessor_directory: Option<PathBuf>,
}

/// Actual native safety storage facts. These describe components, not RSS.
#[derive(Clone, Debug, Default)]
pub struct Usage {
    /// Exact mathematical identities retained by the collision index.
    pub exact_states: usize,
    /// Predecessor storage before explicit cleanup.
    pub predecessor: dive_store::predecessor::Usage,
    /// Actual predecessor storage failures.
    pub predecessor_failures: Vec<String>,
    /// Search event reached before predecessor storage prevented certification.
    pub predecessor_deciding_event: Option<String>,
    /// Explicit predecessor cleanup finished successfully.
    pub predecessor_cleanup_completed: bool,
}

/// Retained canonical rows, bucketed by the existing state hash. Hashes select
/// candidates only; even a complete collision cannot discard a distinct state.
#[derive(Debug, Default)]
pub(crate) struct Exact {
    rows: HashMap<u64, Vec<Vec<Handle>>>,
    len: usize,
}

impl Exact {
    pub(crate) const fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn contains(&self, key: u64, row: &[Handle], heap: &Heap) -> bool {
        self.rows.get(&key).is_some_and(|bucket| {
            bucket.iter().any(|kept| {
                kept.len() == row.len() && kept.iter().zip(row).all(|(&a, &b)| heap.eq(a, b))
            })
        })
    }

    pub(crate) fn insert(&mut self, key: u64, row: &[Handle], heap: &Heap) -> Result<bool, Error> {
        if self.contains(key, row, heap) {
            return Ok(false);
        }
        self.rows
            .try_reserve(1)
            .map_err(|_| Error::unsupported("exact state index allocation"))?;
        let mut kept = Vec::new();
        kept.try_reserve_exact(row.len())
            .map_err(|_| Error::unsupported("exact state row allocation"))?;
        kept.extend_from_slice(row);
        let bucket = self.rows.entry(key).or_default();
        bucket
            .try_reserve(1)
            .map_err(|_| Error::unsupported("exact state collision bucket allocation"))?;
        bucket.push(kept);
        self.len += 1;
        Ok(true)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "exact collision controls")]
mod tests {
    use std::sync::Arc;

    use dive_core::Store;

    use super::*;

    #[test]
    fn complete_hash_collision_preserves_distinct_rows_and_zero_key() {
        let mut heap = Heap::new(Arc::new(Store::new(true)), true, false);
        let mut seen = Exact::default();
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let a = heap.tuple(&[one, two]);
        let a = heap.intern(a).unwrap();
        let b = heap.tuple(&[two, one]);
        let b = heap.intern(b).unwrap();
        assert!(seen.insert(0, &[a], &heap).unwrap());
        assert!(seen.insert(0, &[b], &heap).unwrap());
        assert!(!seen.insert(0, &[a], &heap).unwrap());
        assert!(!seen.contains(1, &[a], &heap));
        assert_eq!(seen.len, 2);
    }

    #[test]
    fn exact_equality_ignores_literal_class_and_hashconsing_without_erasing_views() {
        for hashcons in [false, true] {
            let mut heap = Heap::new(Arc::new(Store::new(hashcons)), true, false);
            let one = Handle::int(1).unwrap();
            let two = Handle::int(2).unwrap();
            let tuple = heap.tuple_literal(&[one, two]);
            let original = heap.intern_exact(tuple).unwrap();
            let canonical_tuple = heap.intern(original).unwrap();
            let function = heap.fcn_sorted(&[one, two], &[one, two]);
            let canonical_function = heap.intern(function).unwrap();
            let mut seen = Exact::default();
            assert!(seen.insert(7, &[canonical_tuple], &heap).unwrap());
            assert!(!seen.insert(7, &[canonical_function], &heap).unwrap());
            assert!(heap.view(original).is_tuple_literal());
            assert!(!heap.view(canonical_tuple).is_tuple_literal());
            assert!(!heap.view(canonical_function).is_tuple_literal());
        }
    }
}
