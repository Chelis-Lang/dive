//! State-value access independent of retained behavior topology.
//!
//! This private boundary is used by the reference adapter today. It does not
//! provide completion evidence, store retention or a bounded conversion cache.

use dive_check::capture::BehaviorGraph;
use dive_eval::next::StateVals;
use std::borrow::Cow;

use super::AnalysisError;

/// Original generated-edge metadata, separate from its potentially lazy values.
pub(super) struct ConcreteMetadata<'a> {
    pub(super) ordinal: u64,
    pub(super) label: &'a str,
}

/// Representatives and original successors, without assuming owned value rows.
///
/// Metadata access never converts values. The caller selects the first original
/// for each destination before requesting its values. Values must remain in IR
/// variable order; an original cannot be replaced by a destination representative.
/// Counts and metadata stay stable throughout an analysis call.
/// Future converting implementations must account retention and cooperate with
/// the run budget themselves; this access boundary supplies no resource bound.
pub(super) trait StateSource {
    fn representative_count(&self) -> usize;

    fn representative(&mut self, vertex: usize) -> Result<Cow<'_, StateVals>, AnalysisError>;

    /// `None` distinguishes a missing row from a present empty frontier row.
    fn concrete_count(&self, source: usize) -> Option<usize>;

    fn concrete_metadata(&self, source: usize, original: usize) -> Option<ConcreteMetadata<'_>>;

    fn concrete(&mut self, source: usize, original: usize) -> Result<StateVals, AnalysisError>;
}

/// The existing reference representation, borrowed without copying graph rows.
pub(super) struct CapturedStates<'a>(pub(super) &'a BehaviorGraph);

impl StateSource for CapturedStates<'_> {
    fn representative_count(&self) -> usize {
        self.0.states.len()
    }

    fn representative(&mut self, vertex: usize) -> Result<Cow<'_, StateVals>, AnalysisError> {
        self.0
            .states
            .get(vertex)
            .map(Cow::Borrowed)
            .ok_or(AnalysisError::Inconsistent("original state reference"))
    }

    fn concrete_count(&self, source: usize) -> Option<usize> {
        self.0.concrete.get(source).map(Vec::len)
    }

    fn concrete_metadata(&self, source: usize, original: usize) -> Option<ConcreteMetadata<'_>> {
        self.0
            .concrete
            .get(source)?
            .get(original)
            .map(|target| ConcreteMetadata {
                ordinal: target.ordinal,
                label: &target.label,
            })
    }

    fn concrete(&mut self, source: usize, original: usize) -> Result<StateVals, AnalysisError> {
        self.0
            .concrete
            .get(source)
            .and_then(|row| row.get(original))
            .map(|target| target.state.clone())
            .ok_or(AnalysisError::Inconsistent(
                "original action target row shape",
            ))
    }
}

#[cfg(test)]
mod tests;
