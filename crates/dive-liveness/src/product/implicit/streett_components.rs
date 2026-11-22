//! Emptiness of positive Streett clauses on a reachable tableau product.
//! A recurrent component that contains a grant can visit it infinitely often.
//! Where a grant never occurs, every recurrent edge must satisfy its paired
//! persistence condition. Remove other edges and recompute components. Each
//! surviving refinement makes another persistence condition mandatory.
//! A stable component admits one closed walk covering all its promises and
//! grants. The existing lasso finder constructs a concrete witness.
use super::condition_words::optional;
use super::{AcceptanceAccess, Adjacency, CycleConditions, ProductError, TruthAccess, csr};
use crate::formula::{AtomKind, Recurrence, StreettPair};
use crate::product::{BehaviorEdges, BehaviorView};
use crate::search::{Poll, SearchError, restricted_components};
use std::cell::Cell;

const DEAD: u64 = 1 << 63;

/// Original-edge truth shared by all residual products of this one property.
pub(crate) struct StreettWords<'a> {
    offsets: &'a [usize],
    words: Vec<u64>,
    conditions: Vec<&'a Recurrence>,
    pairs: Vec<(u64, u64)>,
    retained: usize,
}

pub(super) enum ComponentVerdict {
    Empty,
    Accept(u64),
    Unknown,
}
impl<'a> StreettWords<'a> {
    pub(crate) fn new(
        graph: BehaviorView<'a>,
        labels: &impl TruthAccess,
        pairs: &'a [StreettPair],
        alternatives: impl Iterator<Item = &'a CycleConditions>,
    ) -> Result<Option<Self>, ProductError> {
        if pairs.is_empty() || pairs.len() > 63 {
            return Ok(None);
        }
        let Some(offsets) = graph.csr_offsets() else {
            return Ok(None);
        };
        labels.validate_dimensions(graph)?;
        let count = offsets.last().copied().unwrap_or(0);
        let mut remaining = crate::budget::optional_buffer_limit();
        let (Some(mut conditions), Some(mut choices)) = (
            optional(63, &mut remaining),
            optional(pairs.len(), &mut remaining),
        ) else {
            return Ok(None);
        };
        let mut bit = |cond: &'a Recurrence| -> Option<u64> {
            let index = if let Some(i) = conditions.iter().position(|old| *old == cond) {
                i
            } else {
                if conditions.len() == 63 || cond.atoms().any(|a| !labels.contains(a)) {
                    return None;
                }
                conditions.push(cond);
                conditions.len() - 1
            };
            Some(1 << index)
        };
        for p in pairs {
            csr::poll()?;
            let (Some(a), Some(b)) = (bit(&p.persistent), bit(&p.recurring)) else {
                return Ok(None);
            };
            choices.push((a, b));
        }
        for base in alternatives {
            for cond in base.always.iter().chain(&base.recurring) {
                csr::poll()?;
                if bit(cond).is_none() {
                    return Ok(None);
                }
            }
        }
        let retained = crate::budget::optional_buffer_limit() - remaining;
        let (Some(mut state), Some(mut actions)) = (
            optional(conditions.len(), &mut remaining),
            optional(conditions.len(), &mut remaining),
        ) else {
            return Ok(None);
        };
        for (i, &condition) in conditions.iter().enumerate() {
            let list = if condition.atoms().all(|a| a.kind == AtomKind::State) {
                &mut state
            } else {
                &mut actions
            };
            list.push((1u64 << i, condition));
        }
        let max_scratch = conditions
            .iter()
            .map(|c| c.scratch_len())
            .max()
            .unwrap_or(0);
        let (Some(mut words), Some(mut scratch)) = (
            optional(count, &mut remaining),
            optional(max_scratch, &mut remaining),
        ) else {
            return Ok(None);
        };
        for (v, row) in offsets.windows(2).enumerate() {
            csr::poll()?;
            let mut bits = 0;
            for &(bit, cond) in &state {
                if cond.evaluate_with_scratch(|a| labels.truth(a, v, 0), &mut scratch) {
                    bits |= bit;
                }
            }
            for edge in 0..row[1] - row[0] {
                if edge.is_multiple_of(64) {
                    csr::poll()?;
                }
                let mut word = bits;
                for &(bit, cond) in &actions {
                    if cond.evaluate_with_scratch(|a| labels.truth(a, v, edge), &mut scratch) {
                        word |= bit;
                    }
                }
                words.push(word);
            }
        }
        Ok(Some(Self {
            offsets,
            words,
            conditions,
            pairs: choices,
            retained,
        }))
    }
    pub(super) fn bytes(&self) -> usize {
        self.retained + self.words.capacity() * size_of::<u64>()
    }
    pub(super) fn workspace(vertices: usize) -> usize {
        vertices.saturating_mul(size_of::<Cell<u64>>() + size_of::<usize>())
    }
    /// With no grant on an internal edge, this pair forces persistence on
    /// every edge of any accepting subcycle. A missing GF requirement rules
    /// out the entire component, including all of its subcycles.
    fn required_filter(
        &self,
        cyclic: bool,
        union: u64,
        mut required: u64,
        needed: u64,
    ) -> (u64, u64) {
        if !cyclic || union & needed != needed {
            return (DEAD, 0);
        }
        let mut grants = 0;
        for (i, &(persistent, grant)) in self.pairs.iter().enumerate() {
            if union & grant != 0 {
                grants |= 1 << i;
            } else if union & persistent != 0 {
                required |= persistent;
            } else {
                return (DEAD, 0);
            }
        }
        (required, grants)
    }

    pub(super) fn solve(
        &self,
        graph: &impl Adjacency,
        product: &(impl BehaviorEdges + AcceptanceAccess),
        base: &CycleConditions,
        mut bytes: usize,
    ) -> Result<ComponentVerdict, SearchError> {
        let n = graph.vertex_count();
        let mask = |conds: &[Recurrence]| {
            let mut bits = 0;
            for c in conds {
                bits |= 1 << self.conditions.iter().position(|old| *old == c)?;
            }
            Some(bits)
        };
        let (Some(always), Some(needed)) = (mask(&base.always), mask(&base.recurring)) else {
            return Ok(ComponentVerdict::Unknown);
        };
        let (Some(mut filters), Some(mut members)) =
            (optional(n, &mut bytes), optional(n, &mut bytes))
        else {
            return Ok(ComponentVerdict::Unknown);
        };
        filters.resize(n, Cell::new(always));
        members.resize(n, usize::MAX);
        let word = |v, e| {
            let (b, edge) = product.behavior_edge(v, e);
            self.words[self.offsets[b] + edge]
        };
        let mut poll = Poll::default();
        loop {
            let mut accepted = None;
            let mut split = false;
            restricted_components(
                graph,
                |v| filters[v].get() != DEAD,
                |v, e| {
                    let required = filters[v].get();
                    required == 0 || (required != DEAD && word(v, e) & required == required)
                },
                &mut poll,
                |component, poll| {
                    if accepted.is_some() {
                        return Ok(());
                    }
                    let stamp = component[0];
                    let old = filters[stamp].get();
                    if old == DEAD {
                        return Ok(());
                    }
                    for promise in 0..product.state_count() {
                        if !poll.any(&component, |&v| product.state(promise, v))? {
                            for v in component {
                                poll.step()?;
                                filters[v].set(DEAD);
                            }
                            return Ok(());
                        }
                    }
                    for &v in &component {
                        poll.step()?;
                        members[v] = stamp;
                    }
                    let mut union = 0u64;
                    let mut intersection = u64::MAX;
                    let mut cyclic = false;
                    for &v in &component {
                        let mut c = graph.cursor(v);
                        while let Some((dst, e)) = graph.advance(v, &mut c)? {
                            poll.step()?;
                            if members[dst] == stamp {
                                let bits = word(v, e);
                                if bits & old == old {
                                    cyclic = true;
                                    union |= bits;
                                    intersection &= bits;
                                }
                            }
                        }
                    }
                    let (next, grants) = self.required_filter(cyclic, union, old, needed);
                    if next != DEAD && next & !intersection == 0 {
                        accepted = Some(grants);
                        return Ok(());
                    }
                    split |= next != DEAD;
                    // An emitted SCC is complete. Tightening only its outgoing
                    // edges cannot merge it with a later SCC. All vertices of
                    // each child SCC inherit the same filter in the next pass.
                    for &v in &component {
                        poll.step()?;
                        filters[v].set(next);
                    }
                    Ok(())
                },
            )?;
            if let Some(choice) = accepted {
                return Ok(ComponentVerdict::Accept(choice));
            }
            if !split {
                return Ok(ComponentVerdict::Empty);
            }
            for chunk in members.chunks_mut(16_384) {
                poll.step()?;
                chunk.fill(usize::MAX);
            }
        }
    }
}
