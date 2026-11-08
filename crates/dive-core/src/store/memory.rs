//! Requested persistent-storage bytes, distinct from process RSS.

use crate::strings::StringsMemory;

/// A snapshot of known persistent-store storage and explicit accounting gaps.
///
/// Allocated slab slots and buffer capacities are measured, including unused
/// slots. Nonempty retired hash tables are counted once while any cached reader
/// retains them. This is a component snapshot, not an atomic process snapshot,
/// a peak-memory measurement or an allocation-time limit.
///
/// Known retained bytes exclude allocator rounding/metadata, Arc control blocks,
/// opaque string-index bucket overhead/spare capacity, and externally owned printer names,
/// random streams and registers. Worker arenas, caches and graph buffers belong
/// to their respective owners, not this snapshot. Whole-process RSS accounting
/// remains necessary and is itself sampled, not a hard allocation-time cap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoreMemory {
    /// Store object and allocated column/shard/generation/counter arrays.
    pub fixed_bytes: usize,
    /// Actual allocated `OnceLock` slab slots, including unused slots.
    pub slab_bytes: usize,
    /// Handle slices boxed by large or representation-preserving nodes.
    pub boxed_payload_bytes: usize,
    /// Payload bytes in the current deduplication tables.
    pub current_table_bytes: usize,
    /// Payload bytes in nonempty old tables still owned by cached readers.
    pub retired_table_bytes: usize,
    /// Number of nonempty old tables still owned by cached readers.
    pub retired_tables: usize,
    /// Allocated retirement-record vector capacity, including unused slots.
    pub retirement_tracking_bytes: usize,
    /// Cumulative temporary `Vec<u64>` bytes requested by index rehashes.
    /// These are allocation traffic, not currently retained bytes.
    pub rehash_scratch_bytes: usize,
    /// Largest individual rehash temporary vector capacity in bytes. This is
    /// not a concurrent or whole-store peak; old/new tables overlap it.
    pub largest_rehash_scratch_bytes: usize,
    /// String-table payload, slot storage and opaque index capacity.
    pub strings: StringsMemory,
    /// The store retains a shared external printer/name context.
    pub has_printer: bool,
    /// The store retains a shared external random-stream provider.
    pub has_draws: bool,
    /// The store retains a shared external register provider.
    pub has_registers: bool,
}

impl StoreMemory {
    /// A lower bound on retained bytes of the measured components, or `None`
    /// on arithmetic overflow. Rehash traffic is excluded; gaps are documented
    /// on [`StoreMemory`]. This must not be used as a complete memory budget.
    #[must_use]
    pub fn known_retained_bytes(self) -> Option<usize> {
        [
            self.fixed_bytes,
            self.slab_bytes,
            self.boxed_payload_bytes,
            self.current_table_bytes,
            self.retired_table_bytes,
            self.retirement_tracking_bytes,
            self.strings.known_retained_bytes()?,
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
    }
}
