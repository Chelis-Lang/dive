//! State-value access independent of retained behavior topology.
//!
//! This private boundary is used by the reference adapter today. It does not
//! provide completion evidence, store retention or a bounded conversion cache.

use dive_eval::next::StateVals;
use dive_runtime::capture::BehaviorGraph;
use std::borrow::Cow;

use super::AnalysisError;

/// Original generated-edge metadata, separate from its potentially lazy values.
pub(super) struct ConcreteMetadata<'a> {
    pub(super) ordinal: u64,
    pub(super) label: &'a str,
}

/// Physical original selected after validating the complete source edge row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OriginalTarget {
    /// Position in the source's generated originals, never its TLC ordinal.
    Generated(usize),
    /// The implicit stutter uses the source representative itself.
    Stuttering,
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

    /// True only when every representative and original was already admitted
    /// (converted once, interning names in event order) before analysis, so a
    /// later read can fail only by an interruption or a storage limit. Label
    /// demand may then read a row only when a reference fallback needs it.
    /// Other sources keep their eager per-vertex demand order.
    fn defers_reads(&self) -> bool {
        false
    }

    fn predicate(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _source: usize,
        _target: Option<OriginalTarget>,
    ) -> Result<Option<bool>, AnalysisError> {
        Ok(None)
    }

    fn validate_predicate_inputs(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _source: usize,
        _target: Option<OriginalTarget>,
    ) -> Result<bool, AnalysisError> {
        Ok(false)
    }

    /// Records one label demand answered by the dive-eval evaluator.
    fn reference_predicate(&mut self, _atom: &crate::plan::PredicateAtom) {}

    fn predicate_reuse_storage(
        &mut self,
        _bytes: usize,
        _cap: u64,
        _allocation: u64,
        _columns: u64,
    ) {
    }
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
    pub(super) graph: &'a dive_runtime::capture::CsrCapture<'a>,
    pub(super) read: &'a mut (dyn super::periodic::external::NativeReader + 'b),
    pub(super) control: &'a dive_runtime::budget::EvaluationControl,
    pub(super) variables: usize,
}

impl NativeStates<'_, '_> {
    fn predicate_rows(
        &self,
        source: usize,
        target: Option<OriginalTarget>,
    ) -> Result<(usize, Option<usize>), AnalysisError> {
        let malformed = || AnalysisError::Inconsistent("native predicate original row");
        let before = self
            .graph
            .representative_row(source)
            .ok_or_else(malformed)?;
        let after = match target {
            Some(OriginalTarget::Generated(original)) => Some(
                self.graph
                    .original_row(source, original)
                    .ok_or_else(malformed)?,
            ),
            Some(OriginalTarget::Stuttering) => Some(before),
            None => None,
        };
        Ok((before, after))
    }

    fn read_row(&mut self, row: usize) -> Result<StateVals, AnalysisError> {
        let values = self.read.read(row, self.control)?;
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

    fn defers_reads(&self) -> bool {
        self.read.reads_admitted()
    }

    fn predicate(
        &mut self,
        atom: &crate::plan::PredicateAtom,
        source: usize,
        target: Option<OriginalTarget>,
    ) -> Result<Option<bool>, AnalysisError> {
        if !self.read.has_predicate(atom) {
            return Ok(None);
        }
        // selected_targets already checked every ordinal/label and selected
        // the first original per destination. Retain that physical position;
        // searching the outgoing row again for every destination is quadratic.
        let (before, after) = self.predicate_rows(source, target)?;
        self.read.predicate(atom, before, after, self.control)
    }

    fn validate_predicate_inputs(
        &mut self,
        atom: &crate::plan::PredicateAtom,
        source: usize,
        target: Option<OriginalTarget>,
    ) -> Result<bool, AnalysisError> {
        if !self.read.has_predicate(atom) {
            return Ok(false);
        }
        let (before, after) = self.predicate_rows(source, target)?;
        self.read
            .validate_predicate_inputs(atom, before, after, self.control)
    }

    fn reference_predicate(&mut self, atom: &crate::plan::PredicateAtom) {
        self.read.reference_predicate(atom);
    }

    fn predicate_reuse_storage(&mut self, bytes: usize, cap: u64, allocation: u64, columns: u64) {
        self.read
            .predicate_reuse_storage(bytes, cap, allocation, columns);
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
    use dive_runtime::budget::{Budget, EvaluationControl, Phase};
    use dive_runtime::capture::{CsrCapture, CsrCaptureInput, Limits};

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
        assert_eq!(selected[&1].values[0], Some(dive_value::Value::Int(2)));
        assert_eq!(selected[&1].original, OriginalTarget::Generated(0));
        assert_eq!(selected[&0].values[0], Some(dive_value::Value::Int(0)));
        assert_eq!(selected[&0].original, OriginalTarget::Stuttering);
        assert_eq!(requests, [2, 0]);
    }

    #[test]
    fn native_predicates_receive_first_original_rows_and_never_destination_representatives() {
        use crate::analyze::periodic::external::NativeReader;
        use crate::formula::AtomKind;
        use crate::plan::{AtomTest, PredicateAtom};
        struct Probe(Vec<(usize, Option<usize>)>);
        impl NativeReader for Probe {
            fn read(
                &mut self,
                _: usize,
                _: &EvaluationControl,
            ) -> Result<StateVals, AnalysisError> {
                Err(AnalysisError::Inconsistent(
                    "predicate metadata must not read values",
                ))
            }
            fn has_predicate(&self, _: &PredicateAtom) -> bool {
                true
            }
            fn predicate(
                &mut self,
                _: &PredicateAtom,
                before: usize,
                after: Option<usize>,
                _: &EvaluationControl,
            ) -> Result<Option<bool>, AnalysisError> {
                self.0.push((before, after));
                Ok(Some(true))
            }
        }
        let analysis = dive_sema::analyze_source(
            "Rows.tla",
            "---- MODULE Rows ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\n====\n",
            &dive_sema::Options::default(),
        );
        let ir = std::sync::Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let config = dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .expect("config");
        let model =
            dive_model::bind(ir, &config, &dive_model::BindOptions::default()).expect("model");
        let atom = PredicateAtom {
            pred: model.init[0].pred.clone(),
            kind: AtomKind::Action,
            test: AtomTest::Predicate,
        };
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
        .expect("graph");
        let mut probe = Probe(Vec::new());
        let mut source = NativeStates {
            graph: &graph,
            read: &mut probe,
            control: &control,
            variables: 1,
        };
        assert_eq!(
            source
                .predicate(&atom, 0, Some(OriginalTarget::Generated(0)))
                .expect("original target"),
            Some(true)
        );
        assert_eq!(
            source
                .predicate(&atom, 0, Some(OriginalTarget::Stuttering))
                .expect("implicit stutter"),
            Some(true)
        );
        assert_eq!(
            source.predicate(&atom, 0, None).expect("state predicate"),
            Some(true)
        );
        for original in [2, usize::MAX] {
            assert!(matches!(
                source.predicate(&atom, 0, Some(OriginalTarget::Generated(original))),
                Err(AnalysisError::Inconsistent("native predicate original row"))
            ));
        }
        assert_eq!(probe.0, [(0, Some(2)), (0, Some(0)), (0, None)]);
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
