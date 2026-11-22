//! Necessary edge conditions for accepting cycles across fairness alternatives.
//! One bit names one Boolean condition. Each original behavior edge, including
//! parallel and stuttering edges, has its own truth word.

use super::{AcceptanceAccess, CycleConditions, ProductError, TruthAccess, csr};
use crate::formula::Recurrence;
use crate::product::BehaviorEdges;
use crate::search::{Adjacency, Poll, SearchError, restricted_components};

fn optional<T>(count: usize, remaining: &mut usize) -> Option<Vec<T>> {
    if count.checked_mul(size_of::<T>())? > *remaining {
        return None;
    }
    let mut values = Vec::new();
    values.try_reserve_exact(count).ok()?;
    *remaining = remaining.checked_sub(values.capacity().checked_mul(size_of::<T>())?)?;
    Some(values)
}

pub(super) struct ConditionWords<'a> {
    offsets: &'a [usize],
    words: Vec<u64>,
    // Bits that must always hold, and bits that must each occur, on a cycle.
    masks: Vec<(u64, u64)>,
}

impl<'a> ConditionWords<'a> {
    pub(super) fn build(
        offsets: &'a [usize],
        alternatives: &[CycleConditions],
        labels: &impl TruthAccess,
        remaining: &mut usize,
    ) -> Result<Option<Self>, ProductError> {
        if alternatives.len() < 8 {
            return Ok(None);
        }
        let Some(mut masks) = optional(alternatives.len(), remaining) else {
            return Ok(None);
        };
        let Some(mut words) = optional(offsets.last().copied().unwrap_or(0), remaining) else {
            return Ok(None);
        };
        let Some(mut distinct) = optional::<&Recurrence>(64, remaining) else {
            return Ok(None);
        };
        for alternative in alternatives {
            csr::poll()?;
            let mut always = 0;
            let mut needed = 0;
            for (i, condition) in alternative
                .always
                .iter()
                .chain(&alternative.recurring)
                .enumerate()
            {
                let index = if let Some(index) = distinct.iter().position(|c| *c == condition) {
                    index
                } else {
                    // Fall back to source-order admission of malformed alternatives.
                    if distinct.len() == 64 || condition.atoms().any(|atom| !labels.contains(atom))
                    {
                        return Ok(None);
                    }
                    distinct.push(condition);
                    distinct.len() - 1
                };
                needed |= 1 << index;
                if i < alternative.always.len() {
                    always |= 1 << index;
                }
            }
            masks.push((always, needed));
        }
        let scratch_len = distinct
            .iter()
            .map(|condition| condition.scratch_len())
            .max()
            .unwrap_or(0);
        let Some(mut scratch) = optional(scratch_len, remaining) else {
            return Ok(None);
        };
        for (vertex, row) in offsets.windows(2).enumerate() {
            csr::poll()?;
            for edge in 0..row[1] - row[0] {
                if edge.is_multiple_of(128) {
                    csr::poll()?;
                }
                let mut word = 0;
                for (index, condition) in distinct.iter().enumerate() {
                    if condition.evaluate_with_scratch(
                        |atom| labels.truth(atom, vertex, edge),
                        &mut scratch,
                    ) {
                        word |= 1 << index;
                    }
                }
                words.push(word);
            }
        }
        Ok(Some(Self {
            offsets,
            words,
            masks,
        }))
    }

    /// Every accepting cycle lies in one strongly connected component. Its FG
    /// and GF predicates must each occur on an internal edge. FG predicates
    /// shared by the remaining alternatives must hold on *every* edge of the
    /// cycle. Removing other edges and decomposing again preserves all answers.
    /// A surviving component goes back to the ordinary witness search.
    pub(super) fn rules_out(
        &self,
        graph: &impl Adjacency,
        product: &(impl AcceptanceAccess + BehaviorEdges),
        mut remaining: usize,
    ) -> Result<bool, SearchError> {
        let count = graph.vertex_count();
        let (Some(mut required), Some(mut next), Some(mut members)) = (
            optional(count, &mut remaining),
            optional(count, &mut remaining),
            optional(count, &mut remaining),
        ) else {
            return Ok(false);
        };
        required.resize(count, 0);
        next.resize(count, 0);
        members.resize(count, usize::MAX);
        let word = |v, edge| {
            let (source, label) = product.behavior_edge(v, edge);
            self.words[self.offsets[source] + label]
        };
        let mut poll = Poll::default();
        // Further refinements are optional. The ordinary search decides the rest.
        for _ in 0..8 {
            let mut has_candidate = false;
            let mut inconclusive = false;
            restricted_components(
                graph,
                |_| true,
                |v, e| word(v, e) & required[v] == required[v],
                &mut poll,
                |component, poll| {
                    if inconclusive {
                        return Ok(());
                    }
                    for promise in 0..product.state_count() {
                        if !poll.any(&component, |&v| product.state(promise, v))? {
                            return Ok(());
                        }
                    }
                    let stamp = component[0];
                    for &v in &component {
                        poll.step()?;
                        members[v] = stamp;
                    }
                    let mut union = 0;
                    let mut intersection = u64::MAX;
                    let mut cyclic = false;
                    for &v in &component {
                        let mut cursor = graph.cursor(v);
                        while let Some((target, e)) = graph.advance(v, &mut cursor)? {
                            poll.step()?;
                            if members[target] == stamp {
                                let bits = word(v, e);
                                if bits & required[v] == required[v] {
                                    cyclic = true;
                                    union |= bits;
                                    intersection &= bits;
                                }
                            }
                        }
                    }
                    if !cyclic {
                        return Ok(());
                    }
                    let mut common = u64::MAX;
                    let mut viable = false;
                    for &(always, needed) in &self.masks {
                        poll.step()?;
                        if union & needed == needed {
                            viable = true;
                            common &= always;
                        }
                    }
                    if viable {
                        has_candidate = true;
                        if common & !intersection == 0 {
                            inconclusive = true;
                        }
                        for v in component {
                            poll.step()?;
                            next[v] = required[v] | common;
                        }
                    }
                    Ok(())
                },
            )?;
            if !has_candidate {
                return Ok(true);
            }
            if inconclusive {
                return Ok(false);
            }
            std::mem::swap(&mut required, &mut next);
            next.copy_from_slice(&required);
            members.fill(usize::MAX);
        }
        Ok(false)
    }
}
