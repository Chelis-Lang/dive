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

/// On-demand rows supplied by the producer owning their persistent roots.
pub(super) struct NativeStates<'a, 'b> {
    pub(super) graph: &'a dive_check::capture::CsrCapture<'a>,
    pub(super) read: &'a mut super::periodic::external::NativeRead<'b>,
    pub(super) control: &'a dive_check::budget::EvaluationControl,
    pub(super) variables: usize,
}

impl NativeStates<'_, '_> {
    fn read_row(&mut self, row: usize) -> Result<StateVals, AnalysisError> {
        let values = (self.read)(row, self.control)?;
        if values.len() != self.variables || values.iter().any(Option::is_none) {
            return Err(AnalysisError::Inconsistent(
                "native value row variable dimensions",
            ));
        }
        Ok(values)
    }
}

impl StateSource for NativeStates<'_, '_> {
    fn representative_count(&self) -> usize {
        self.graph.state_count()
    }

    fn representative(&mut self, vertex: usize) -> Result<Cow<'_, StateVals>, AnalysisError> {
        let row = self
            .graph
            .representative_row(vertex)
            .ok_or(AnalysisError::Inconsistent("native representative row"))?;
        self.read_row(row).map(Cow::Owned)
    }

    fn concrete_count(&self, source: usize) -> Option<usize> {
        self.graph.original_count(source)
    }

    fn concrete_metadata(&self, source: usize, original: usize) -> Option<ConcreteMetadata<'_>> {
        let range = self.graph.outgoing_range(source)?;
        let edge = range.start.checked_add(original)?;
        if edge >= range.end {
            return None;
        }
        Some(ConcreteMetadata {
            ordinal: self.graph.ordinal(edge)??,
            label: self.graph.label(edge)??,
        })
    }

    fn concrete(&mut self, source: usize, original: usize) -> Result<StateVals, AnalysisError> {
        let row = self
            .graph
            .original_row(source, original)
            .ok_or(AnalysisError::Inconsistent("native original row"))?;
        self.read_row(row)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[allow(clippy::expect_used, reason = "checked native row demand fixtures")]
mod native_tests {
    use super::*;
    use dive_check::budget::{Budget, EvaluationControl, Phase};
    use dive_check::capture::{CsrCapture, CsrCaptureInput, Limits};

    #[test]
    fn first_original_per_destination_uses_physical_positions_not_ordinals() {
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        let graph = CsrCapture::checked(
            CsrCaptureInput {
                offsets: &[0, 3, 4],
                targets: &[1, 1, 0, 1],
                ordinals: &[Some(0), Some(3), None, None],
                actions: &[None; 4],
                edge_rows: &[2, 3, 0, 1],
                representatives: &[0, 1],
                roots: &[0],
                action_labels: &[],
                retained_rows: 4,
                closed_rows: 2,
            },
            Limits::default(),
            &control,
        )
        .expect("checked graph");
        let mut requests = Vec::new();
        let mut read = |row, _: &EvaluationControl| {
            requests.push(row);
            assert_ne!(
                row, 3,
                "the second original for a destination must not be converted"
            );
            Ok(vec![Some(dive_value::Value::Int(
                i64::try_from(row).expect("small row"),
            ))]
            .into())
        };
        let mut source = NativeStates {
            graph: &graph,
            read: &mut read,
            control: &control,
            variables: 1,
        };
        let selected = super::super::selected_targets(
            super::super::OriginalEdges::Native(&graph),
            0,
            &mut source,
        )
        .expect("selected targets");
        assert_eq!(selected[&1][0], Some(dive_value::Value::Int(2)));
        assert_eq!(selected[&0][0], Some(dive_value::Value::Int(0)));
        assert_eq!(requests, [2, 0]);
    }

    #[test]
    fn missing_variable_and_reader_failures_are_explicit() {
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        let graph = CsrCapture::checked(
            CsrCaptureInput {
                offsets: &[0, 1],
                targets: &[0],
                ordinals: &[None],
                actions: &[None],
                edge_rows: &[0],
                representatives: &[0],
                roots: &[0],
                action_labels: &[],
                retained_rows: 1,
                closed_rows: 1,
            },
            Limits::default(),
            &control,
        )
        .expect("graph");
        let mut missing = |_, _: &EvaluationControl| Ok(vec![None].into());
        let mut source = NativeStates {
            graph: &graph,
            read: &mut missing,
            control: &control,
            variables: 1,
        };
        assert!(matches!(
            source.representative(0),
            Err(AnalysisError::Inconsistent(_))
        ));
        let mut refused = |_, _: &EvaluationControl| {
            Err(AnalysisError::UnsupportedValue(
                "exact value refusal".into(),
            ))
        };
        let mut source = NativeStates {
            graph: &graph,
            read: &mut refused,
            control: &control,
            variables: 1,
        };
        assert!(
            matches!(source.representative(0), Err(AnalysisError::UnsupportedValue(reason)) if reason == "exact value refusal")
        );
    }
}
