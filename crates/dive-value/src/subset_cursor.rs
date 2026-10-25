//! Transactional integer positions for an already prepared concrete powerset.

use super::SubsetIter;
use crate::{ErrorKind, Result, Set, Value, ValueError};

/// A concrete powerset stream retaining its original normalized base and order.
///
/// Preparing an index restriction does not consume this stream. A prepared
/// commit only swaps integer storage; constructing its next Value stays separate.
/// No permission, predicate truth or ownership authority follows from this API.
///
/// ```
/// use dive_value::{Ctx, Value, SubsetScratch};
/// let ctx = Ctx::tlc();
/// let Value::Set(set) = Value::subset(Value::interval(1.into(), 3.into(), ctx), ctx)
///     else { return Err("set".into()); };
/// let mut stream = set.concrete_subset_stream(ctx)?.ok_or("concrete base")?;
/// assert_eq!(stream.next(), Some(Value::empty_set(ctx)));
/// let mut scratch = SubsetScratch::default();
/// assert!(stream.prepare_masked_next(&[0, 2], &mut scratch, || Ok(()))?);
/// scratch.prepare_commit(&mut stream).ok_or("position changed")?.commit();
/// assert_eq!(stream.current_indices(), None); // next Value has not been built
/// assert!(stream.next().is_some());
/// assert_eq!(stream.current_indices(), Some([0].as_slice()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ConcreteSubsetStream {
    source: Set,
    inner: SubsetIter,
    pending: bool,
    done: bool,
}

#[derive(Default)]
struct Position {
    indices: Vec<usize>,
    k: usize,
    started: bool,
    pending: bool,
    done: bool,
}

/// Reusable preparation storage containing integers only, never Value owners.
/// A failed or cancelled preparation leaves the original stream untouched.
#[derive(Default)]
pub struct SubsetScratch {
    from: Position,
    target: Position,
    base_len: usize,
    ready: bool,
}

/// An exclusively borrowed commit, validated before any caller acquires a lock.
/// Dropping it without committing leaves the original stream unchanged.
/// `commit` does not allocate, deallocate, build/drop a Value or invoke a callback.
pub struct SubsetCommit<'a> {
    stream: &'a mut ConcreteSubsetStream,
    scratch: &'a mut SubsetScratch,
}

impl ConcreteSubsetStream {
    pub(super) fn new(inner: SubsetIter, source: Set) -> Self {
        Self {
            source,
            inner,
            pending: false,
            done: false,
        }
    }

    /// Whether this cursor was prepared from this exact immutable set owner.
    /// This identifies enumeration data, not predicate or host authority.
    #[must_use]
    pub fn belongs_to(&self, source: &Set) -> bool {
        self.source.ptr_eq(source)
    }

    /// The actual retained normalized component, with no enumeration or callback.
    #[must_use]
    pub fn base_elements(&self) -> &[Value] {
        &self.inner.base
    }

    /// Original base indices of the last built candidate. A pending committed
    /// target has not yet built a candidate and returns `None`.
    #[must_use]
    pub fn current_indices(&self) -> Option<&[usize]> {
        (self.inner.started && !self.pending && !self.done).then_some(self.inner.idx.as_slice())
    }

    /// Prepare the next subset using only `allowed` base indices, without
    /// consuming anything. `allowed` must be strictly increasing/in range.
    /// The requested order is the retained subsequence of the original size/lex
    /// order, strictly after the last built candidate. A pending/end/unstarted
    /// stream returns `false`. Preparation and its polling occur outside locks.
    ///
    /// # Errors
    /// Invalid explicit indices, failed scratch reservation or the original
    /// error returned by `poll`; no cursor position is changed on any error.
    pub fn prepare_masked_next(
        &self,
        allowed: &[usize],
        scratch: &mut SubsetScratch,
        mut poll: impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        scratch.ready = false;
        let Some(current) = self.current_indices() else {
            return Ok(false);
        };
        poll()?;
        let mut previous = None;
        for &index in allowed {
            poll()?;
            if index >= self.inner.base.len() || previous.is_some_and(|p| p >= index) {
                return Err(ValueError::new(
                    ErrorKind::Argument,
                    "subset indices must be increasing and in range",
                ));
            }
            previous = Some(index);
        }
        copy_indices(&mut scratch.from.indices, current, &mut poll)?;
        scratch.from.k = self.inner.k;
        scratch.from.started = self.inner.started;
        scratch.from.pending = self.pending;
        scratch.from.done = self.done;
        scratch.base_len = self.inner.base.len();
        scratch.target.indices.clear();
        scratch
            .target
            .indices
            .try_reserve(allowed.len())
            .map_err(|error| reservation(&error))?;
        let k = current.len();
        let ranks = &mut scratch.target.indices;
        let mut greater = false;
        if k <= allowed.len() {
            for (position, &index) in current.iter().enumerate() {
                poll()?;
                let lower = allowed.partition_point(|candidate| *candidate < index);
                let rank = ranks.last().map_or(lower, |last| lower.max(last + 1));
                if rank >= allowed.len() || allowed.len() - rank < k - position {
                    break;
                }
                ranks.push(rank);
                if allowed[rank] > index {
                    greater = true;
                    fill_ranks(ranks, k, &mut poll)?;
                    break;
                }
            }
        }
        if !greater {
            // Equal prefix (possibly complete): carry right to left. A strict
            // successor is needed even when the current tuple is fully allowed.
            while let Some(last) = ranks.last().copied() {
                poll()?;
                let position = ranks.len() - 1;
                if last < allowed.len() - k + position {
                    ranks[position] += 1;
                    fill_ranks(ranks, k, &mut poll)?;
                    greater = true;
                    break;
                }
                ranks.pop();
            }
        }
        if !greater {
            ranks.clear();
            if k < allowed.len() {
                for rank in 0..=k {
                    poll()?;
                    ranks.push(rank);
                }
            } else {
                scratch.target.done = true;
                scratch.target.pending = false;
                scratch.target.started = true;
                scratch.target.k = k;
                scratch.ready = true;
                return Ok(true);
            }
        }
        for rank in ranks.iter_mut() {
            poll()?;
            *rank = allowed[*rank];
        }
        scratch.target.k = ranks.len();
        scratch.target.started = true;
        scratch.target.pending = true;
        scratch.target.done = false;
        scratch.ready = true;
        Ok(true)
    }
}

fn reservation(error: &std::collections::TryReserveError) -> ValueError {
    ValueError::new(
        ErrorKind::DiveLimit,
        format!("subset index scratch: {error}"),
    )
}
fn copy_indices(
    to: &mut Vec<usize>,
    from: &[usize],
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    to.clear();
    to.try_reserve(from.len())
        .map_err(|error| reservation(&error))?;
    for &index in from {
        poll()?;
        to.push(index);
    }
    Ok(())
}
fn fill_ranks(
    ranks: &mut Vec<usize>,
    length: usize,
    poll: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    while ranks.len() < length {
        poll()?;
        let next = ranks.last().map_or(0, |last| last + 1);
        ranks.push(next);
    }
    Ok(())
}

impl SubsetScratch {
    /// Validate the prepared position value/base length and borrow exclusively
    /// before locking. This is not a source-identity certificate; a caller
    /// binding predicate truth must also match [`ConcreteSubsetStream::belongs_to`].
    /// The returned token contains references only, not Value owners.
    #[must_use]
    pub fn prepare_commit<'a>(
        &'a mut self,
        stream: &'a mut ConcreteSubsetStream,
    ) -> Option<SubsetCommit<'a>> {
        if !self.ready
            || self.base_len != stream.inner.base.len()
            || self.from.k != stream.inner.k
            || self.from.started != stream.inner.started
            || self.from.pending != stream.pending
            || self.from.done != stream.done
            || self.from.indices != stream.inner.idx
        {
            return None;
        }
        Some(SubsetCommit {
            stream,
            scratch: self,
        })
    }
}

impl SubsetCommit<'_> {
    /// Commit a previously prepared position in bounded constant work. Old
    /// integer scratch remains owned by the caller and is not freed here.
    pub fn commit(self) {
        std::mem::swap(&mut self.stream.inner.idx, &mut self.scratch.target.indices);
        self.stream.inner.k = self.scratch.target.k;
        self.stream.inner.started = self.scratch.target.started;
        self.stream.pending = self.scratch.target.pending;
        self.stream.done = self.scratch.target.done;
        self.scratch.ready = false;
    }
}

impl Iterator for ConcreteSubsetStream {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        if self.done {
            return None;
        }
        if self.pending {
            self.pending = false;
            return Some(self.inner.current_value());
        }
        let value = self.inner.next();
        if value.is_none() {
            self.done = true;
        }
        value
    }
}

#[cfg(test)]
#[path = "subset_cursor_tests.rs"]
mod tests;
