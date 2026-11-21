//! The common resident fingerprint maps to one packed identity row.
//! Keep actual collisions and non-consecutive discovery IDs in side tables.

use std::collections::{HashMap, TryReserveError, hash_map::Entry};

use dive_core::Error;

const SHARDS: usize = 256;

#[derive(Debug, Default)]
pub(super) struct ResidentIndex {
    primary: Vec<HashMap<u64, usize>>,
    collisions: HashMap<u64, Vec<usize>>,
    other_ids: HashMap<usize, u64>,
}

impl ResidentIndex {
    pub(super) fn positions(&self, hash: u64) -> impl Iterator<Item = usize> + '_ {
        let first = self.primary.get(shard(hash)).and_then(|map| map.get(&hash));
        first
            .into_iter()
            .chain(self.collisions.get(&hash).into_iter().flatten())
            .copied()
    }

    pub(super) fn id(&self, position: usize) -> u64 {
        self.other_ids
            .get(&position)
            .copied()
            .unwrap_or(position as u64)
    }

    pub(super) fn insert(&mut self, hash: u64, position: usize, id: u64) -> Result<(), Error> {
        // Reserve all fallible storage before publishing a position. The caller
        // has already reserved the corresponding row and appends it on success.
        if self.primary.is_empty() {
            self.primary.try_reserve_exact(SHARDS).map_err(allocation)?;
            self.primary.resize_with(SHARDS, HashMap::new);
        }
        if id != position as u64 {
            self.other_ids.try_reserve(1).map_err(allocation)?;
        }
        let map = &mut self.primary[shard(hash)];
        map.try_reserve(1).map_err(allocation)?;
        match map.entry(hash) {
            Entry::Vacant(slot) => {
                slot.insert(position);
            }
            Entry::Occupied(_) => {
                self.collisions.try_reserve(1).map_err(allocation)?;
                match self.collisions.entry(hash) {
                    Entry::Vacant(slot) => {
                        let mut extra = Vec::new();
                        extra.try_reserve(1).map_err(allocation)?;
                        extra.push(position);
                        slot.insert(extra);
                    }
                    Entry::Occupied(mut slot) => {
                        slot.get_mut().try_reserve(1).map_err(allocation)?;
                        slot.get_mut().push(position);
                    }
                }
            }
        }
        if id != position as u64 {
            self.other_ids.insert(position, id);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.primary.iter().all(HashMap::is_empty)
    }
}

fn shard(hash: u64) -> usize {
    usize::from(hash.to_be_bytes()[0])
}

fn allocation(_: TryReserveError) -> Error {
    Error::unsupported("exact state index allocation")
}
