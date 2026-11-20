//! Ordered discovery rows and the counters at each invariant-check boundary.
use super::{Counts, Miss, miss};
use dive_core::{Handle, Heap};
use dive_explore::graph::{GraphSink, Step, Stopped};
use std::num::NonZeroUsize;

#[derive(Clone, Copy, Debug)]
struct Visit {
    parent: Option<NonZeroUsize>,
    action: Option<NonZeroUsize>,
    generated: u64,
    depth: u64,
}
pub(super) struct Rows {
    width: usize,
    cap: usize,
    values: Vec<Handle>,
    visits: Vec<Visit>,
    generated: u64,
    initial_generated: u64,
    refusal: Option<Miss>,
}
impl Rows {
    pub(super) fn new(width: usize, cap: usize) -> Self {
        Self {
            width,
            cap,
            values: Vec::new(),
            visits: Vec::new(),
            generated: 0,
            initial_generated: 0,
            refusal: None,
        }
    }
    pub(super) fn len(&self) -> usize {
        self.visits.len()
    }
    pub(super) fn row(&self, id: usize) -> &[Handle] {
        &self.values[id * self.width..(id + 1) * self.width]
    }
    pub(super) fn counts(&self, id: usize) -> Counts {
        let visit = self.visits[id];
        let distinct = id as u64 + 1;
        Counts {
            generated: visit.generated,
            distinct,
            depth: visit.depth,
            // Init enqueues before checking. A successor fails before enqueue,
            // after the current parent has been removed for expansion.
            left_on_queue: visit.parent.map_or(distinct, |p| (id - p.get()) as u64),
            initial_generated: if visit.parent.is_none() {
                visit.generated
            } else {
                self.initial_generated
            },
        }
    }
    pub(super) fn parent(&self, id: usize) -> Option<usize> {
        self.visits[id].parent.map(|i| i.get() - 1)
    }
    pub(super) fn action(&self, id: usize) -> Option<usize> {
        self.visits[id].action.map(|i| i.get() - 1)
    }
    pub(super) fn refusal(&mut self) -> Option<Miss> {
        self.refusal.take()
    }
    fn push(
        &mut self,
        row: &[Handle],
        parent: Option<usize>,
        action: Option<usize>,
    ) -> Result<(), Miss> {
        if row.len() != self.width
            || row
                .iter()
                .any(|v| v.as_int().is_none() && v.as_bool().is_none())
        {
            return Err(miss(
                "state rows must contain only inline integers or Booleans",
            ));
        }
        if self.len() >= self.cap {
            return Err(miss("capture row limit"));
        }
        let depth = match parent {
            None => 1,
            Some(id) => {
                self.visits
                    .get(id)
                    .ok_or_else(|| miss("invalid discovery parent"))?
                    .depth
                    + 1
            }
        };
        self.values
            .try_reserve(self.width)
            .map_err(|_| miss("row allocation"))?;
        self.visits
            .try_reserve(1)
            .map_err(|_| miss("parent allocation"))?;
        self.values.extend_from_slice(row);
        self.visits.push(Visit {
            parent: parent.and_then(|i| NonZeroUsize::new(i.saturating_add(1))),
            action: action.and_then(|i| NonZeroUsize::new(i.saturating_add(1))),
            generated: self.generated,
            depth,
        });
        Ok(())
    }
    // The graph observer sees successful invariant checks only. The deciding
    // row of a refutation comes from Core's real trace, at its actual stop.
    pub(super) fn append_violation(
        &mut self,
        trace: &[dive_explore::TraceState],
    ) -> Result<(), Miss> {
        let (last, path) = trace
            .split_last()
            .ok_or_else(|| miss("empty producer trace"))?;
        let parent = if let Some(previous) = path.last() {
            Some(
                (0..self.len())
                    .find(|id| self.row(*id) == previous.values)
                    .ok_or_else(|| miss("producer trace parent absent"))?,
            )
        } else {
            None
        };
        self.push(&last.values, parent, last.action)
    }
}
impl GraphSink for Rows {
    fn max_expansion_units(&self) -> usize {
        1
    }
    fn max_successors_per_unit(&self) -> u64 {
        u64::MAX
    }
    fn emitted(&mut self, generated: u64, ordinal: Option<u64>) -> Result<(), Stopped> {
        self.generated = generated;
        if ordinal.is_none() {
            self.initial_generated = generated;
        }
        Ok(())
    }
    fn state(&mut self, _: &mut Heap, step: Step<'_>) -> Result<(), Stopped> {
        if let Some((id, _)) = step.new {
            let added = usize::try_from(id)
                .ok()
                .filter(|i| *i == self.len())
                .ok_or_else(|| miss("discovery order"))
                .and_then(|_| {
                    let parent = step
                        .parent
                        .map(usize::try_from)
                        .transpose()
                        .map_err(|_| miss("parent index"))?;
                    self.push(step.original, parent, step.action)
                });
            if let Err(error) = added {
                self.refusal = Some(error);
                return Err(Stopped);
            }
        }
        Ok(())
    }
    fn stutter(&mut self, _: u64) -> Result<(), Stopped> {
        Ok(())
    }
    fn level_done(&mut self, _: u64, _: dive_explore::Stats) -> Result<bool, Stopped> {
        Ok(false)
    }
}
