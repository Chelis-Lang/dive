//! Controlled Graph admission and on-demand values for the compact producer.
//!
//! The first route deliberately retains no converted-value cache. Persistent
//! roots stay with `NativeGraph`; only a currently demanded row is converted.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_check::Verdict;
use dive_check::budget::{Budget, EvaluationControl, Phase};
use dive_core::{Handle, Heap, Store};
use dive_eval::next::StateVals;
use dive_eval::{EvalError, HostControl};
use dive_liveness::analyze::AnalysisError;
use dive_model::Model;

/// A run-local converter; it never owns the producer's row table or topology.
pub(crate) struct NativeValues<'a> {
    model: &'a Model,
    heap: Heap,
    capture: Arc<EvaluationControl>,
    admitted: u64,
    read: u64,
    converted: u64,
    conversion: Duration,
}

impl<'a> NativeValues<'a> {
    pub(crate) fn new(
        model: &'a Model,
        store: Arc<Store>,
        budget: &Budget,
    ) -> Result<Self, Verdict> {
        if !budget.is_unlimited() && !budget.is_armed() {
            return Err(Verdict::InternalError {
                message: "native graph values require the actual armed run budget".into(),
            });
        }
        let capture =
            EvaluationControl::new(budget, Phase::Capture).map_err(|_| Verdict::InternalError {
                message: "native graph conversion control identity is unrepresentable".into(),
            })?;
        Ok(Self {
            model,
            heap: Heap::new(store, false, false),
            capture,
            admitted: 0,
            read: 0,
            converted: 0,
            conversion: Duration::ZERO,
        })
    }

    /// Every accepted original is admitted before the producer retains metadata.
    pub(crate) fn admit(&mut self, row: &[Handle]) -> Result<(), Verdict> {
        let control = self.capture.clone();
        self.convert(row, &control).map_err(capture_error)?;
        self.admitted = self.admitted.saturating_add(1);
        Ok(())
    }

    pub(crate) fn read(
        &mut self,
        row: &[Handle],
        control: &EvaluationControl,
    ) -> Result<StateVals, AnalysisError> {
        let values = self.convert(row, control)?;
        self.read = self.read.saturating_add(1);
        Ok(values)
    }

    fn convert(
        &mut self,
        row: &[Handle],
        control: &EvaluationControl,
    ) -> Result<StateVals, AnalysisError> {
        check(&self.capture, control)?;
        if row.len() != self.model.ir.vars().len() {
            return Err(AnalysisError::Inconsistent(
                "native value row variable dimensions",
            ));
        }
        let started = Instant::now();
        let mut values = Vec::new();
        values
            .try_reserve_exact(row.len())
            .map_err(|_| AnalysisError::Limit("native conversion row storage"))?;
        for &handle in row {
            let mut interrupted = None;
            let mut proceed = || match self.capture.check().and_then(|()| control.check()) {
                Ok(()) => true,
                Err(cause) => {
                    interrupted = Some(EvalError::interrupted(cause));
                    false
                }
            };
            let value = dive_core::print::to_value_controlled(
                &mut self.heap,
                &self.model.names,
                handle,
                self.model.evaluator.options().ctx,
                dive_core::print::Conversion::Graph,
                &mut proceed,
            )
            .map_err(|error| match error {
                dive_core::print::ConversionError::Construction(
                    dive_value::ConstructionError::Interrupted,
                ) => interruption(&self.capture, control, interrupted),
                dive_core::print::ConversionError::Construction(
                    dive_value::ConstructionError::Allocation,
                ) => AnalysisError::Limit("native conversion scratch storage"),
                error => AnalysisError::UnsupportedValue(error.to_string()),
            })?;
            self.converted = self.converted.saturating_add(1);
            values.push(Some(value));
        }
        check(&self.capture, control)?;
        self.conversion += started.elapsed();
        Ok(values.into())
    }

    pub(crate) fn metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "admitted_rows": self.admitted, "read_rows": self.read,
            "converted_values": self.converted,
            "convert_ms": self.conversion.as_secs_f64() * 1e3,
            "cache_entries": 0, "cache_bytes": 0,
            "cache_enabled": false,
        })
    }
}

fn check(owner: &EvaluationControl, control: &EvaluationControl) -> Result<(), AnalysisError> {
    owner
        .check()
        .and_then(|()| control.check())
        .map_err(|cause| interruption(owner, control, Some(EvalError::interrupted(cause))))
}

fn interruption(
    owner: &EvaluationControl,
    control: &EvaluationControl,
    cause: Option<EvalError>,
) -> AnalysisError {
    match cause
        .filter(|cause| owner.owns(cause))
        .and_then(|cause| control.verdict(&cause))
    {
        Some(Verdict::Stopped(stop)) => AnalysisError::Stopped(stop),
        Some(Verdict::BudgetMeasurement { error }) => AnalysisError::BudgetMeasurement(error),
        _ => AnalysisError::Inconsistent(
            "native conversion interruption lacks matching run evidence",
        ),
    }
}

fn capture_error(error: AnalysisError) -> Verdict {
    match error {
        AnalysisError::Stopped(stopped) => Verdict::Stopped(stopped),
        AnalysisError::BudgetMeasurement(error) => Verdict::BudgetMeasurement { error },
        AnalysisError::UnsupportedValue(message) => Verdict::Unsupported {
            feature: format!(
                "a state value outside the reference values (core liveness): {message}"
            ),
        },
        AnalysisError::Limit(bound) => Verdict::Unsupported {
            feature: format!("native graph conversion storage: {bound}"),
        },
        error => Verdict::InternalError {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "finite native admission fixtures")]
mod tests {
    use super::*;

    fn model() -> Model {
        let parsed = dive_sema::analyze_source(
            "V.tla",
            "---- MODULE V ----\nVARIABLE x\nInit == x = 0\nNext == x' = x\nP == <>(x = 0)\n====\n",
            &dive_sema::Options::default(),
        );
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config");
        dive_model::bind(
            Arc::new(dive_ir::lower(&parsed).expect("IR")),
            &cfg,
            &dive_model::BindOptions::dive(),
        )
        .expect("model")
    }

    #[test]
    fn graph_admission_preserves_classes_refuses_unknown_history_and_retains_no_values() {
        let model = model();
        let store = Arc::new(Store::default());
        let mut heap = Heap::new(store.clone(), false, false);
        let key = heap.string("field").expect("field");
        let zero = Handle::int(0).expect("integer");
        let function = heap.fcn_sorted(&[key], &[zero]);
        let function = heap.intern_exact(function).expect("persistent function");
        let record = heap.record(&[key], &[zero]);
        let record = heap.intern_exact(record).expect("persistent record");
        let mut values = NativeValues::new(&model, store, &Budget::unlimited()).expect("converter");
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        for (handle, literal) in [(function, false), (record, true)] {
            values.admit(&[handle]).expect("admission");
            let row = values.read(&[handle], &control).expect("read");
            assert_eq!(
                row[0]
                    .as_ref()
                    .expect("value")
                    .as_fcn()
                    .expect("function")
                    .is_record_literal(),
                literal
            );
            let released = Arc::downgrade(&row);
            drop(row);
            assert!(
                released.upgrade().is_none(),
                "the converter retains no converted row"
            );
        }
        let raw = heap.set_enum(&[zero]);
        let raw = heap.intern_exact(raw).expect("persistent raw");
        assert!(
            matches!(values.admit(&[raw]), Err(Verdict::Unsupported { feature }) if feature.contains("normalization history"))
        );
        assert_eq!(values.metrics()["cache_entries"], 0);
        assert_eq!(values.metrics()["cache_bytes"], 0);
        assert_eq!(values.metrics()["admitted_rows"], 2);
    }

    #[test]
    fn foreign_interruption_and_bad_rows_cannot_become_success_or_this_runs_stop() {
        let model = model();
        let mut values =
            NativeValues::new(&model, Arc::new(Store::default()), &Budget::unlimited())
                .expect("converter");
        let watchdog = Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("start")
            .expect("watchdog");
        let foreign =
            EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("foreign control");
        assert!(matches!(
            values.read(&[Handle::bool(true)], &foreign),
            Err(AnalysisError::Inconsistent(_))
        ));
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        assert!(matches!(
            values.read(&[], &control),
            Err(AnalysisError::Inconsistent(_))
        ));
        let mut stopped = NativeValues::new(&model, Arc::new(Store::default()), watchdog.budget())
            .expect("armed converter");
        assert!(
            matches!(stopped.admit(&[Handle::bool(true)]), Err(Verdict::Stopped(stop)) if stop.phase == Phase::Capture)
        );
        assert!(
            matches!(stopped.read(&[Handle::bool(true)], &foreign), Err(AnalysisError::Stopped(stop)) if stop.phase == Phase::Temporal)
        );
    }
}
