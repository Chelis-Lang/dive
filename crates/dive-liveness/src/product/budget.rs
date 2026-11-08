//! Requested acceptance-buffer layouts; excludes every other product owner.

use super::{CompactAcceptance, Limits, Ordered, ProductError, set_bit};
use crate::formula::Tableau;
use std::mem::size_of;

/// Admission policy for the coordinator's compact acceptance buffers.
/// [`Self::Legacy`] keeps the logical acceptance-cell bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptancePolicy {
    /// Preserve [`Limits::acceptance_cells`] and existing materialization behavior.
    Legacy,
    /// Replace only logical acceptance cells with covered buffer bytes and
    /// independent materialization work. Other product bounds retain their units.
    Buffers {
        /// Requested promise-column headers and words, plus the maximum live
        /// Boolean scratch of one cycle-condition alternative. Actual
        /// capacities are checked after reservation.
        /// Allocator excess/overhead, runtime and all other owners are excluded;
        /// this is not a process-memory ceiling.
        bytes: usize,
        /// Promise assignments, initialized headers, words and scratch entries,
        /// and inspected cycle conditions. Evaluating conditions during the
        /// search is not counted; this does not bound search work.
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

// Covered bytes of the promise columns: one header and one packed word row
// per promise. On-the-fly products carry no action columns or offsets.
pub(super) struct Layout {
    bytes: usize,
    state_words: usize,
}

impl Layout {
    pub(super) fn new(vertices: usize, promises: usize) -> Result<Self, ProductError> {
        let state_words = vertices.div_ceil(64);
        let headers = promises.checked_mul(size_of::<Vec<u64>>()).ok_or(BYTES)?;
        let words = promises
            .checked_mul(state_words)
            .and_then(|n| n.checked_mul(size_of::<u64>()))
            .ok_or(BYTES)?;
        Ok(Self {
            bytes: headers.checked_add(words).ok_or(BYTES)?,
            state_words,
        })
    }

    pub(super) fn check(&self, limit: usize) -> Result<(), ProductError> {
        if self.bytes > limit {
            Err(BYTES)
        } else {
            Ok(())
        }
    }

    // One unit per promise assignment, initialized header and zeroed word.
    fn work(&self, vertices: usize, promises: usize) -> Option<usize> {
        vertices
            .checked_mul(promises)?
            .checked_add(promises)?
            .checked_add(promises.checked_mul(self.state_words)?)
    }
}

// During discovery the final promise columns must still fit. No evaluation or
// speculative allocation happens here; vertex/edge/inspection guards retain
// their positions.
pub(super) fn fits_layout(
    vertices: usize,
    promises: usize,
    bytes: usize,
) -> Result<(), ProductError> {
    Layout::new(vertices, promises)?.check(bytes)
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
        super::csr::poll()?;
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

fn promise_columns(
    input: &Ordered<'_>,
    tableau: &Tableau,
    layout: &Layout,
    requests: &mut Requests,
) -> Result<Vec<Vec<u64>>, ProductError> {
    let mut states = requests.reserve(tableau.promise_count)?;
    for p in 0..tableau.promise_count {
        let mut words = requests.reserve(layout.state_words)?;
        initialize(&mut words, layout.state_words, 0)?;
        for (v, &old) in input.order.iter().enumerate() {
            if v.is_multiple_of(1024) {
                super::csr::poll()?;
            }
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
    bytes: usize,
    work: usize,
) -> Result<CompactAcceptance, ProductError> {
    let vertices = input.order.len();
    let layout = Layout::new(vertices, tableau.promise_count)
        .ok()
        .filter(|layout| layout.bytes <= bytes)
        .ok_or(BYTES)?;
    let materialization_work = layout
        .work(vertices, tableau.promise_count)
        .filter(|used| *used <= work)
        .ok_or(WORK)?;
    let mut requests = Requests {
        used: 0,
        limit: bytes,
    };
    let states = promise_columns(input, tableau, &layout, &mut requests)?;
    Ok(CompactAcceptance {
        states,
        materialization_work,
    })
}

fn initialize<T: Clone>(values: &mut Vec<T>, len: usize, value: T) -> Result<(), ProductError> {
    // Reservation already succeeded; bounded fills keep actual budgets observable.
    while values.len() < len {
        super::csr::poll()?;
        values.resize(values.len().saturating_add(1024).min(len), value.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
