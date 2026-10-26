//! The per-worker transient arena (`03-value-model.md` §3.2).
//!
//! Values built while one state is expanded live here: node headers in one
//! vector and their contents in one pool of handles, so building a value does
//! not allocate once both vectors have grown to their steady size (S7). The
//! arena is reset after each expansion and its epoch incremented. A handle
//! carries the epoch it was created in; dereferencing a handle from an older
//! epoch is an internal error. The check runs in every build, so a stale
//! handle is reported as a bug, never silently misread.
//!
//! The `-arena` ablation keeps each node's contents in its own heap
//! allocation instead of the pool, freed at reset, which is what a design with
//! one allocation per value pays.

use crate::handle::Handle;
use crate::node::{Kind, View};

/// Epochs use 29 bits of the handle.
const EPOCH_MASK: u32 = (1 << 29) - 1;

#[derive(Clone, Copy, Debug)]
struct ANode {
    kind: Kind,
    epoch: u32,
    lo: i64,
    hash: u64,
    sum: u64,
    start: u32,
    len: u32,
}

/// A transient arena.
#[derive(Debug)]
pub struct Arena {
    epoch: u32,
    nodes: Vec<ANode>,
    pool: Vec<Handle>,
    boxed: Vec<Box<[Handle]>>,
    heap_mode: bool,
    /// For each node, its interned handle once it has been interned in this
    /// epoch (deep close shares interned subtrees).
    pub(crate) interned: Vec<Handle>,
    stale: std::cell::Cell<bool>,
    peak_nodes: usize,
    peak_pool: usize,
}

impl Arena {
    /// A new arena. `heap_mode` is the `-arena` ablation.
    #[must_use]
    pub fn new(heap_mode: bool) -> Self {
        Self {
            epoch: 1,
            nodes: Vec::new(),
            pool: Vec::new(),
            boxed: Vec::new(),
            heap_mode,
            interned: Vec::new(),
            stale: std::cell::Cell::new(false),
            peak_nodes: 0,
            peak_pool: 0,
        }
    }

    /// Frees every transient and starts a new epoch.
    pub fn reset(&mut self) {
        self.peak_nodes = self.peak_nodes.max(self.nodes.len());
        self.peak_pool = self.peak_pool.max(self.pool.len());
        self.nodes.clear();
        self.pool.clear();
        self.boxed.clear();
        self.interned.clear();
        self.epoch = (self.epoch + 1) & EPOCH_MASK;
        if self.epoch == 0 {
            self.epoch = 1;
        }
    }

    /// The current epoch.
    #[must_use]
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Whether a stale or foreign transient handle was dereferenced (a bug).
    #[must_use]
    pub fn stale(&self) -> bool {
        self.stale.get()
    }

    /// The number of live transient nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the arena holds no node.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The largest number of nodes and pool handles held in one epoch.
    #[must_use]
    pub fn peak(&self) -> (usize, usize) {
        (
            self.peak_nodes.max(self.nodes.len()),
            self.peak_pool.max(self.pool.len()),
        )
    }

    /// Allocates a node with the given contents.
    pub(crate) fn alloc(
        &mut self,
        kind: Kind,
        lo: i64,
        hash: u64,
        sum: u64,
        data: &[Handle],
    ) -> Handle {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bounded by the 32-bit index below"
        )]
        let index = self.nodes.len() as u32;
        let (start, len) = if self.heap_mode {
            #[allow(clippy::cast_possible_truncation, reason = "one box per node")]
            let start = self.boxed.len() as u32;
            self.boxed.push(data.into());
            (start, 0)
        } else {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "pools above 2^32 handles are refused by the engine"
            )]
            let start = self.pool.len() as u32;
            self.pool.extend_from_slice(data);
            #[allow(clippy::cast_possible_truncation, reason = "as above")]
            (start, data.len() as u32)
        };
        self.nodes.push(ANode {
            kind,
            epoch: self.epoch,
            lo,
            hash,
            sum,
            start,
            len,
        });
        self.interned.push(Handle::EMPTY);
        Handle::transient(self.epoch, index)
    }

    /// The node behind a transient handle, if it belongs to this epoch,
    /// without recording a miss.
    pub(crate) fn lookup(&self, h: Handle) -> Option<View<'_>> {
        let (epoch, index) = h.transient_parts();
        let n = self.nodes.get(index as usize)?;
        if n.epoch != epoch || epoch != self.epoch {
            return None;
        }
        let data: &[Handle] = if self.heap_mode {
            self.boxed.get(n.start as usize)?
        } else {
            self.pool
                .get(n.start as usize..(n.start + n.len) as usize)?
        };
        Some(View {
            kind: n.kind,
            lo: n.lo,
            hash: n.hash,
            sum: n.sum,
            data,
        })
    }

    /// The index of a transient handle of this epoch.
    pub(crate) fn index(&self, h: Handle) -> Option<usize> {
        let (epoch, index) = h.transient_parts();
        (epoch == self.epoch && (index as usize) < self.nodes.len()).then_some(index as usize)
    }

    /// Records a stale access found elsewhere.
    pub(crate) fn mark_stale(&self) {
        self.stale.set(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_from_an_older_epoch_are_refused() {
        for heap in [false, true] {
            let mut a = Arena::new(heap);
            let one = Handle::int(1).unwrap_or(Handle::EMPTY);
            let h = a.alloc(Kind::Set, 0, 7, 7, &[one]);
            assert_eq!(a.lookup(h).map(|v| v.data.len()), Some(1));
            a.reset();
            assert!(a.lookup(h).is_none(), "stale handle must not resolve");
            // A new node at the same index has a new epoch.
            let g = a.alloc(Kind::Set, 0, 9, 9, &[]);
            assert_ne!(g, h);
            assert!(a.lookup(h).is_none());
            assert!(!a.stale(), "a plain lookup does not record a miss");
            assert_eq!(a.lookup(g).map(|v| v.hash), Some(9));
        }
    }
}
