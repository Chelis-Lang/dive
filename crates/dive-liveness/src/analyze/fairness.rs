//! Optional packed columns from one literal cloned bound behavior WF pair.
//!
//! The owning `analyze_graph` invocation fixes the immutable graph namespace and
//! the planner batch. This stores no row keys, offsets, scopes or property graphs.

use super::{AnalysisError, PackedTruth};
use crate::plan::FrozenWfAtom;

pub(super) struct Cache {
    enabled: bool,
    initialized: bool,
    cap: usize,
    widths: [usize; 2],
    owner: Option<u64>,
    columns: [Option<Vec<u64>>; 2],
    pub(super) peak_bytes: usize,
    pub(super) cap_bypasses: u64,
    pub(super) allocation_bypasses: u64,
    pub(super) stored_columns: u64,
}

impl Cache {
    pub(super) fn new(enabled: bool, cap: usize) -> Self {
        Self {
            enabled,
            initialized: false,
            cap,
            widths: [0; 2],
            owner: None,
            columns: [None, None],
            peak_bytes: 0,
            cap_bypasses: 0,
            allocation_bypasses: 0,
            stored_columns: 0,
        }
    }

    pub(super) fn configure(&mut self, vertices: usize, edges: usize) {
        if !self.enabled || self.initialized {
            return;
        }
        self.initialized = true;
        self.widths = [
            vertices / 64 + usize::from(!vertices.is_multiple_of(64)),
            edges / 64 + usize::from(!edges.is_multiple_of(64)),
        ];
        let bytes = self.widths[0]
            .checked_add(self.widths[1])
            .and_then(|words| words.checked_mul(size_of::<u64>()))
            .and_then(|bytes| bytes.checked_add(size_of::<Self>()));
        if bytes.is_none_or(|bytes| bytes > self.cap) {
            self.enabled = false;
            self.cap_bypasses += 1;
        }
    }

    pub(super) fn get(&self, recipe: Option<FrozenWfAtom>, position: usize) -> Option<bool> {
        let recipe = recipe?;
        if self.owner != Some(recipe.owner()) {
            return None;
        }
        let words = self.columns[usize::from(recipe.action())].as_ref()?;
        words
            .get(position / 64)
            .map(|word| word & (1 << (position % 64)) != 0)
    }

    pub(super) fn complete(
        &mut self,
        recipe: Option<FrozenWfAtom>,
        atom: usize,
        labels: &PackedTruth,
        fully_native: bool,
    ) -> Result<(), AnalysisError> {
        self.complete_with(recipe, atom, labels, fully_native, |copy, len| {
            copy.try_reserve_exact(len).is_ok()
        })
    }

    pub(super) fn complete_with(
        &mut self,
        recipe: Option<FrozenWfAtom>,
        atom: usize,
        labels: &PackedTruth,
        fully_native: bool,
        reserve: impl FnOnce(&mut Vec<u64>, usize) -> bool,
    ) -> Result<(), AnalysisError> {
        let Some(recipe) = recipe.filter(|_| self.enabled && fully_native) else {
            return Ok(());
        };
        if self.owner.is_some_and(|owner| owner != recipe.owner()) {
            return Ok(());
        }
        let slot = usize::from(recipe.action());
        if self.columns[slot].is_some() {
            return Ok(());
        }
        let words = labels
            .column_words(atom, recipe.action())
            .filter(|words| words.len() == self.widths[slot])
            .ok_or(AnalysisError::Inconsistent("frozen WF column layout"))?;
        let mut copy = Vec::new();
        if !reserve(&mut copy, words.len()) || copy.capacity() < words.len() {
            self.allocation_bypasses += 1;
            return Ok(());
        }
        let bytes = self
            .columns
            .iter()
            .flatten()
            .try_fold(size_of::<Self>(), |bytes, column| {
                column
                    .capacity()
                    .checked_mul(size_of::<u64>())
                    .and_then(|n| bytes.checked_add(n))
            })
            .and_then(|bytes| {
                copy.capacity()
                    .checked_mul(size_of::<u64>())
                    .and_then(|n| bytes.checked_add(n))
            });
        let Some(bytes) = bytes.filter(|&bytes| bytes <= self.cap) else {
            self.cap_bypasses += 1;
            return Ok(());
        };
        // Count accepted construction storage too, even if a later poll stops
        // the copy before any truth becomes visible.
        self.peak_bytes = self.peak_bytes.max(bytes);
        for chunk in words.chunks(1024) {
            if crate::budget::exhausted() {
                return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
            }
            copy.extend_from_slice(chunk);
        }
        self.owner = Some(recipe.owner());
        self.columns[slot] = Some(copy);
        self.stored_columns += 1;
        Ok(())
    }
}
