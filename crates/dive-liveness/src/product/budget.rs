//! Requested acceptance-buffer layouts; excludes every other product owner.

use super::{CompactAcceptance, Labels, Limits, Ordered, ProductError, set_bit};
use crate::Graph;
use crate::formula::{AtomKind, Recurrence, Tableau};
use std::mem::size_of;

/// Admission policy for the coordinator's compact acceptance buffers.
/// Existing owned product and analysis entrypoints retain [`Self::Legacy`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptancePolicy {
    /// Preserve [`Limits::acceptance_cells`] and existing materialization behavior.
    Legacy,
    /// Replace only logical acceptance cells with covered buffer bytes and
    /// independent materialization work. Other product bounds retain their units.
    Buffers {
        /// Requested column headers, words, optional offsets and maximum live
        /// Boolean scratch. Actual capacities are checked after reservation.
        /// Allocator excess/overhead, runtime and all other owners are excluded;
        /// this is not a process-memory ceiling.
        bytes: usize,
        /// Metadata inspections, initialized entries, promise assignments,
        /// offsets, action rows/projections/writes, executed Boolean nodes and
        /// actually visited child references. This does not bound search work.
        work: usize,
    },
}

impl AcceptancePolicy {
    /// The file-checking default: the former nominal Boolean-payload allowance
    /// (8,000,000 bytes), with independent 32,000,000 materialization work units
    /// derived from the existing transition-inspection allowance.
    #[must_use]
    pub fn compact_default() -> Self {
        let limits = Limits::default();
        Self::Buffers {
            bytes: limits.acceptance_cells * size_of::<bool>(),
            work: limits.steps,
        }
    }
}

const BYTES: ProductError = ProductError::Limit("acceptance buffer bytes");
const ALLOCATION: ProductError = ProductError::Limit("acceptance buffer allocation");
const WORK: ProductError = ProductError::Limit("acceptance materialization work");

#[derive(Clone, Copy)]
pub(super) struct ActionDomain {
    vertices: usize,
    edges: usize,
    behavior: bool,
}

impl ActionDomain {
    pub(super) fn behavior(vertices: usize, edges: usize) -> Self {
        Self {
            vertices,
            edges,
            behavior: true,
        }
    }
}

pub(super) struct Layout {
    bytes: usize,
    state_words: usize,
    action_words: usize,
    offsets: usize,
    scratch: usize,
    domain: ActionDomain,
}

impl Layout {
    pub(super) fn new(
        vertices: usize,
        edges: usize,
        promises: usize,
        conditions: usize,
        scratch: usize,
    ) -> Result<Self, ProductError> {
        Self::with_domain(
            vertices,
            promises,
            conditions,
            scratch,
            ActionDomain {
                vertices,
                edges,
                behavior: false,
            },
        )
    }

    fn with_domain(
        vertices: usize,
        promises: usize,
        conditions: usize,
        scratch: usize,
        domain: ActionDomain,
    ) -> Result<Self, ProductError> {
        let state_words = vertices.div_ceil(64);
        let action_words = domain.edges.div_ceil(64);
        let offsets = if conditions == 0 {
            0
        } else {
            domain.vertices.checked_add(1).ok_or(BYTES)?
        };
        let scratch = if domain.edges == 0 || conditions == 0 {
            0
        } else {
            scratch
        };
        let mut bytes = 0_usize;
        for (count, width) in [
            (
                promises.checked_add(conditions).ok_or(BYTES)?,
                size_of::<Vec<u64>>(),
            ),
            (
                promises.checked_mul(state_words).ok_or(BYTES)?,
                size_of::<u64>(),
            ),
            (
                conditions.checked_mul(action_words).ok_or(BYTES)?,
                size_of::<u64>(),
            ),
            (offsets, size_of::<usize>()),
            (scratch, size_of::<bool>()),
        ] {
            bytes = bytes
                .checked_add(count.checked_mul(width).ok_or(BYTES)?)
                .ok_or(BYTES)?;
        }
        Ok(Self {
            bytes,
            state_words,
            action_words,
            offsets,
            scratch,
            domain,
        })
    }

    pub(super) fn check(&self, limit: usize) -> Result<(), ProductError> {
        if self.bytes > limit {
            Err(BYTES)
        } else {
            Ok(())
        }
    }
}

// During discovery either final representation may fit. No evaluation or
// speculative allocation selects a mode here; vertex/edge/inspection guards
// retain their positions. Mode selection occurs once on the complete product.
pub(super) fn fits_layout(
    vertices: usize,
    edges: usize,
    promises: usize,
    conditions: usize,
    scratch: usize,
    behavior: ActionDomain,
    bytes: usize,
) -> Result<(), ProductError> {
    if Layout::new(vertices, edges, promises, conditions, scratch)
        .is_ok_and(|layout| layout.bytes <= bytes)
    {
        return Ok(());
    }
    Layout::with_domain(vertices, promises, conditions, scratch, behavior)?.check(bytes)
}

// No covered buffer grows after one reservation. Check requested bytes before
// allocation and actual exposed capacity afterwards. Allocator excess can exist
// transiently before refusal; this is not a physical heap cap.
struct Requests {
    used: usize,
    limit: usize,
}

impl Requests {
    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, ProductError> {
        let requested = count.checked_mul(size_of::<T>()).ok_or(BYTES)?;
        self.add(requested)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(allocation_error)?;
        let actual = values.capacity().checked_mul(size_of::<T>()).ok_or(BYTES)?;
        self.add(actual.checked_sub(requested).ok_or(BYTES)?)?;
        Ok(values)
    }

    fn add(&mut self, bytes: usize) -> Result<(), ProductError> {
        self.used = self
            .used
            .checked_add(bytes)
            .filter(|n| *n <= self.limit)
            .ok_or(BYTES)?;
        Ok(())
    }
}

fn allocation_error(_: std::collections::TryReserveError) -> ProductError {
    ALLOCATION
}

struct Programs {
    nodes: usize,
    references: usize,
    scratch: usize,
    inspected: usize,
}

impl Programs {
    fn inspect(conditions: &[Recurrence], edges: usize, work: usize) -> Result<Self, ProductError> {
        // Include the builder's existing maximum-scratch column inspection.
        let mut result = Self {
            nodes: 0,
            references: 0,
            scratch: 0,
            inspected: conditions.len(),
        };
        if edges == 0 {
            return Ok(result);
        }
        for condition in conditions {
            let nodes = condition.scratch_len();
            result.inspected = result
                .inspected
                .checked_add(1)
                .and_then(|n| n.checked_add(nodes))
                .filter(|n| *n <= work)
                .ok_or(WORK)?;
            let (nodes, references) = condition.work_metadata().ok_or(WORK)?;
            result.nodes = result.nodes.checked_add(nodes).ok_or(WORK)?;
            result.references = result.references.checked_add(references).ok_or(WORK)?;
            result.scratch = result.scratch.max(nodes);
        }
        Ok(result)
    }
}

struct Plan {
    layout: Layout,
    base: usize,
    minimum: usize,
    maximum: Option<usize>,
}

impl Plan {
    fn new(
        layout: Layout,
        vertices: usize,
        promises: usize,
        conditions: usize,
        programs: &Programs,
    ) -> Result<Self, ProductError> {
        let mut base = programs.inspected;
        // One unit per initialized header/word/scratch entry, promise assignment,
        // offset entry, inspected action row, projected edge and written truth bit.
        for cost in [
            vertices.checked_mul(promises),
            Some(layout.offsets),
            promises.checked_add(conditions),
            promises.checked_mul(layout.state_words),
            conditions.checked_mul(layout.action_words),
            Some(layout.scratch),
            conditions.checked_mul(layout.domain.vertices),
            conditions
                .checked_mul(layout.domain.edges)
                .and_then(|n| n.checked_mul(2)),
        ] {
            base = base.checked_add(cost.ok_or(WORK)?).ok_or(WORK)?;
        }
        let minimum = layout
            .domain
            .edges
            .checked_mul(programs.nodes)
            .and_then(|n| base.checked_add(n))
            .ok_or(WORK)?;
        let maximum = programs
            .nodes
            .checked_add(programs.references)
            .and_then(|n| n.checked_mul(layout.domain.edges))
            .and_then(|n| base.checked_add(n));
        Ok(Self {
            layout,
            base,
            minimum,
            maximum,
        })
    }
}

fn choose(
    input: &Ordered<'_>,
    tableau: &Tableau,
    conditions: &[Recurrence],
    behavior: ActionDomain,
    bytes: usize,
    work: usize,
) -> Result<Plan, ProductError> {
    let vertices = input.order.len();
    let scratch = input.scratch_len;
    let product = Layout::new(
        vertices,
        input.edge_count,
        tableau.promise_count,
        conditions.len(),
        scratch,
    )
    .ok()
    .filter(|l| l.bytes <= bytes);
    let original = if conditions.is_empty() || input.edge_count == 0 {
        None
    } else {
        Layout::with_domain(
            vertices,
            tableau.promise_count,
            conditions.len(),
            scratch,
            behavior,
        )
        .ok()
        .filter(|l| l.bytes <= bytes)
    };
    if product.is_none() && original.is_none() {
        return Err(BYTES);
    }
    let programs = Programs::inspect(conditions, input.edge_count, work)?;
    let product = product.and_then(|l| {
        Plan::new(
            l,
            vertices,
            tableau.promise_count,
            conditions.len(),
            &programs,
        )
        .ok()
    });
    let original = original.and_then(|l| {
        Plan::new(
            l,
            vertices,
            tableau.promise_count,
            conditions.len(),
            &programs,
        )
        .ok()
    });
    let selected = match (product, original) {
        (Some(p), Some(b))
            if b.maximum.is_some_and(|n| n <= work)
                && (p.maximum.is_none() || b.maximum < p.maximum) =>
        {
            b
        }
        (Some(p), Some(b)) if p.minimum > work && b.minimum < p.minimum => b,
        (Some(p), _) => p,
        (None, Some(b)) => b,
        (None, None) => return Err(WORK),
    };
    if selected.minimum > work {
        return Err(WORK);
    }
    Ok(selected)
}

fn promise_columns(
    input: &Ordered<'_>,
    tableau: &Tableau,
    layout: &Layout,
    requests: &mut Requests,
) -> Result<Vec<Vec<u64>>, ProductError> {
    let mut states = requests.reserve(tableau.promise_count)?;
    for p in 0..tableau.promise_count {
        let mut words = requests.reserve(layout.state_words)?;
        words.resize(layout.state_words, 0);
        for (v, &old) in input.order.iter().enumerate() {
            set_bit(
                &mut words,
                v,
                tableau.nodes[input.pairs[old].1].fulfilled[p],
            );
        }
        states.push(words);
    }
    Ok(states)
}

pub(super) fn materialize(
    input: &Ordered<'_>,
    tableau: &Tableau,
    labels: &Labels,
    conditions: &[Recurrence],
    bytes: usize,
    work: usize,
    behavior: (&Graph, usize),
) -> Result<CompactAcceptance, ProductError> {
    let plan = choose(
        input,
        tableau,
        conditions,
        ActionDomain::behavior(behavior.0.edges.len(), behavior.1),
        bytes,
        work,
    )?;
    let layout = plan.layout;
    let mut remaining = work.checked_sub(plan.base).ok_or(WORK)?;
    let mut requests = Requests {
        used: 0,
        limit: bytes,
    };
    let states = promise_columns(input, tableau, &layout, &mut requests)?;
    let rows: &[Vec<usize>] = if layout.domain.behavior {
        &behavior.0.edges
    } else {
        input.edges
    };
    let mut edge_offsets = requests.reserve(layout.offsets)?;
    if layout.offsets != 0 {
        edge_offsets.push(0);
        let mut offset = 0_usize;
        for row in rows {
            offset = offset.checked_add(row.len()).ok_or(BYTES)?;
            edge_offsets.push(offset);
        }
        if offset != layout.domain.edges {
            return Err(ProductError::Shape("acceptance edge count".into()));
        }
    }
    let mut scratch = requests.reserve(layout.scratch)?;
    scratch.resize(layout.scratch, false);
    let mut actions = requests.reserve(conditions.len())?;
    for condition in conditions {
        let mut words = requests.reserve(layout.action_words)?;
        words.resize(layout.action_words, 0);
        for (id, row) in rows.iter().enumerate() {
            let v = if layout.domain.behavior {
                id
            } else {
                input.pairs[input.order[id]].0
            };
            for position in 0..row.len() {
                let e = if layout.domain.behavior {
                    position
                } else {
                    input.original_edges[id][position]
                };
                let truth = condition
                    .evaluate_bounded(
                        |atom| match atom.kind {
                            AtomKind::State => labels.states[atom.index][v],
                            AtomKind::Action => labels.actions[atom.index][v][e],
                        },
                        &mut scratch,
                        &mut remaining,
                    )
                    .ok_or(WORK)?;
                set_bit(&mut words, edge_offsets[id] + position, truth);
            }
        }
        actions.push(words);
    }
    Ok(CompactAcceptance {
        states,
        actions,
        edge_offsets,
        behavior_actions: layout.domain.behavior,
        materialization_work: work - remaining,
    })
}

#[cfg(test)]
mod tests;
