//! Bounded root selection and work attribution for exact cold searches.

use std::time::Duration;

use dive_core::{Error, handle::mix64};
use dive_store::trace::StateId;
use serde::Serialize;

/// Bounds for cold diversification in the original Init stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    roots: u8,
    emissions: u64,
    seed: u64,
}

impl Selection {
    /// Admit at most 64 roots and inspect at most one million emissions.
    /// One root retains the original fast path and needs no selection policy.
    #[must_use]
    pub fn new(roots: u8, emissions: u64) -> Option<Self> {
        ((2..=64).contains(&roots) && (u64::from(roots)..=1_000_000).contains(&emissions))
            .then_some(Self {
                roots,
                emissions,
                seed: 0,
            })
    }

    /// Use the resolved run seed to place the later candidate windows.
    #[must_use]
    pub const fn with_seed(self, seed: u64) -> Self {
        Self { seed, ..self }
    }
}

/// Initial stream inspected and distinct roots retained. Search work is
/// attributed to the root of each state's first discovery path.
#[derive(Clone, Debug, Serialize)]
pub struct Usage {
    /// Number of candidates emitted by the original Init.
    pub emissions: u64,
    /// Whether the selector stopped Init at a configured limit.
    pub selection_cut: bool,
    /// Time in Init, including generation, selection and root admission.
    pub init_ms: f64,
    /// Requested maximum number of roots.
    pub root_limit: u8,
    /// Requested maximum number of original emissions inspected.
    pub emission_limit: u64,
    /// Work attributed to each distinct root in admission order.
    pub roots: Vec<Root>,
}

/// Work by a selected original root of the merged exact graph.
#[derive(Clone, Debug, Serialize)]
pub struct Root {
    /// Zero-based original Init emission position.
    pub ordinal: u64,
    /// First discoveries rooted here, including this root.
    pub distinct: u64,
    /// Source expansions completed at commit time.
    pub expanded: u64,
    /// Successor emissions committed to the checking path, including repeats.
    pub generated: u64,
}

pub(crate) struct Selector {
    selection: Selection,
    earliest: u64,
    owners: Vec<u8>,
    roots: Vec<Root>,
    pub(crate) elapsed: Duration,
}

impl Selector {
    pub(crate) fn new(selection: Selection) -> Self {
        Self {
            selection,
            earliest: 1,
            owners: Vec::new(),
            roots: Vec::new(),
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn considers(&self, emission: u64) -> bool {
        emission >= self.earliest
    }

    pub(crate) fn reached_bound(&self, emission: u64) -> bool {
        emission >= self.selection.emissions
            || self.roots.len() >= usize::from(self.selection.roots)
    }

    pub(crate) fn admitted(
        &mut self,
        id: StateId,
        parent: Option<StateId>,
        emission: u64,
    ) -> Result<(), Error> {
        if usize::try_from(id).ok() != Some(self.owners.len()) {
            return Err(Error::internal("cold root attribution id out of order"));
        }
        self.owners
            .try_reserve(1)
            .map_err(|_| Error::unsupported("cold root attribution allocation"))?;
        let root = if let Some(parent) = parent {
            self.owner(parent)?
        } else {
            let index =
                u8::try_from(self.roots.len()).map_err(|_| Error::internal("cold root count"))?;
            self.roots.push(Root {
                ordinal: emission - 1,
                distinct: 0,
                expanded: 0,
                generated: 0,
            });
            let retained = u64::from(index) + 1;
            // Try another adjacent distinct root first. Later roots leave a
            // seed-dependent gap, with room before the emission ceiling.
            if retained >= 2 && retained < u64::from(self.selection.roots) {
                let left = u64::from(self.selection.roots) - retained;
                let stride =
                    (self.selection.emissions.saturating_sub(emission) / (left + 1)).max(1);
                let jitter = mix64(self.selection.seed.wrapping_add(retained)) % stride;
                self.earliest = emission.saturating_add(stride / 2 + jitter + 1);
            }
            index
        };
        self.owners.push(root);
        self.roots[usize::from(root)].distinct += 1;
        Ok(())
    }

    pub(crate) fn expanded(
        &mut self,
        id: StateId,
        generated: u64,
        completed: bool,
    ) -> Result<(), Error> {
        let root = usize::from(self.owner(id)?);
        self.roots[root].generated += generated;
        self.roots[root].expanded += u64::from(completed);
        Ok(())
    }

    fn owner(&self, id: StateId) -> Result<u8, Error> {
        usize::try_from(id)
            .ok()
            .and_then(|index| self.owners.get(index).copied())
            .ok_or_else(|| Error::internal("cold root attribution missing predecessor"))
    }

    pub(crate) fn finish(self, emissions: u64, selection_cut: bool) -> Usage {
        Usage {
            emissions,
            selection_cut,
            init_ms: self.elapsed.as_secs_f64() * 1000.,
            root_limit: self.selection.roots,
            emission_limit: self.selection.emissions,
            roots: self.roots,
        }
    }
}
