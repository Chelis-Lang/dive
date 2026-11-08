//! Internal source-binding bridge for a single privately owned temporal run.
//!
//! Unlike the borrowed-model analyzer, this bridge never exposes its runtime.
//! Host replacement on caller-owned evaluators keeps its existing behavior.

use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_model::{BindError, BindOptions, ControlledBindError, Model, SymmetryLivenessResolution};
use dive_runtime::{CheckOptions, budget::EvaluationControl};
use dive_stdlib::host::RunHost;
use dive_stdlib::random::{RandomMode, Streams};

use super::{Analysis, DetailedFailure, GraphLabelPolicy, Limits, periodic};
use crate::product::AcceptancePolicy;

pub use super::row_demand::SourceRowDriver;

#[cfg(test)]
#[path = "owned_tests.rs"]
mod tests;

/// A source-binding result which has never published a temporal runtime.
#[derive(Debug)]
pub enum Dispatch {
    /// Every bound obligation is a safety role. Nothing was checked; the
    /// caller keeps its own safety route.
    Safety,
    /// One freshly bound temporal runtime, consumed by its analysis.
    Temporal(Box<OwnedTemporal>),
}

/// A fresh source-bound runtime whose model and evaluator cannot escape.
///
/// The consuming analysis methods retain the constructor's options and binding
/// control. This is not a lease on public caller-owned evaluators. No model,
/// scope, host or arbitrary callback is accepted or exposed by this bridge.
/// It must be constructed and consumed on the dedicated evaluator stack.
///
/// ```no_run
/// # fn run(ir: &std::sync::Arc<dive_ir::Ir>, config: &dive_config::ModelConfig,
/// # options: &dive_runtime::CheckOptions, control: std::sync::Arc<dive_runtime::budget::EvaluationControl>) {
/// use dive_liveness::analyze::{owned, Limits, GraphLabelPolicy};
/// use dive_liveness::product::AcceptancePolicy;
/// if let Some(Ok(owned::Dispatch::Temporal(runtime))) =
///     owned::dispatch_controlled(ir, config, options, control) {
///     let _result = runtime.final_analysis(Limits::default(),
///         AcceptancePolicy::Legacy, GraphLabelPolicy::Cells(1000));
/// }
/// # }
/// ```
pub struct OwnedTemporal {
    model: Box<Model>,
    options: CheckOptions,
    _binding_control: Arc<EvaluationControl>,
    _thread: PhantomData<Rc<()>>,
}

impl fmt::Debug for OwnedTemporal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedTemporal")
            .finish_non_exhaustive()
    }
}

/// Actual private owner, created only while consuming the hidden fresh model.
pub(super) struct Authority<'a> {
    model: &'a Model,
}

impl Authority<'_> {
    pub(super) fn owns(&self, model: &Model) -> bool {
        std::ptr::eq(self.model, model)
    }
}

// Only this private Model identity selects the source-owned driver route.
// An operative public binding cannot manufacture this authority.
pub(super) fn native_hooks(
    authority: Option<&Authority<'_>>,
    model: &Model,
) -> dive_eval::NativeHooks {
    if authority.is_some_and(|authority| authority.owns(model)) {
        dive_eval::NativeHooks::Unrestricted
    } else {
        dive_eval::NativeHooks::Callback
    }
}

impl OwnedTemporal {
    /// Consumes the fresh hidden runtime for the bounded native producer's
    /// source-row error lane. Unknown source/provenance shapes return `None`.
    /// No ordinary row or endpoint validity is established by construction.
    ///
    /// ```no_run
    /// # fn select(runtime: dive_liveness::analyze::owned::OwnedTemporal)
    /// # -> Result<bool, dive_liveness::analyze::AnalysisError> {
    /// use dive_liveness::analyze::{GraphLabelPolicy, Limits};
    /// let driver = runtime.source_row_driver(Limits::default(), GraphLabelPolicy::NoCellLimit)?;
    /// // The native producer still supplies real events and validates any word.
    /// # Ok(driver.is_some()) }
    /// ```
    ///
    /// # Errors
    /// The existing controlled planning, inspection or resource failure.
    pub fn source_row_driver(
        self,
        limits: Limits,
        graph_labels: GraphLabelPolicy,
    ) -> Result<Option<SourceRowDriver>, super::AnalysisError> {
        let _budget = crate::budget::enter(&self.options.budget);
        let control = super::evaluator_control(&self.options)?;
        let registration = control
            .enter(&self.model.evaluator)
            .map_err(|_| super::AnalysisError::Inconsistent("owned source-row control conflict"))?;
        let plans = super::prepare(&self.model, limits, &control)?;
        let authority = Authority { model: &self.model };
        let program = super::row_demand::prepare_source(
            &self.model,
            &plans,
            &self.options,
            self.model.temporals.len(),
            limits.formula,
            graph_labels,
            (Some(&authority), &control),
        )?;
        drop(registration);
        Ok(program.map(|program| SourceRowDriver::new(self.model, plans, program, control)))
    }

    /// The actual bound symmetry resolution, with no runtime handle.
    #[must_use]
    pub fn symmetry_liveness(&self) -> &SymmetryLivenessResolution {
        self.model.symmetry_liveness()
    }

    /// Consumes this runtime for the existing final analysis and owner policies.
    ///
    /// # Errors
    /// Existing detailed analysis failures, including unqualified legacy stops.
    pub fn final_analysis(
        self,
        limits: Limits,
        policy: AcceptancePolicy,
        graph_labels: GraphLabelPolicy,
    ) -> Result<Analysis, DetailedFailure> {
        let authority = Authority { model: &self.model };
        super::analyze_final_bound(
            &self.model,
            &self.options,
            limits,
            policy,
            graph_labels,
            Some(&authority),
        )
    }

    /// Consumes this runtime for the existing completed-level schedule.
    ///
    /// # Errors
    /// Existing periodic failures and actual prefix/check evidence.
    pub fn periodic_analysis(
        self,
        limits: Limits,
        policy: AcceptancePolicy,
        graph_labels: GraphLabelPolicy,
        schedule: periodic::Schedule,
    ) -> Result<(periodic::PeriodicAnalysis, periodic::Checks), periodic::PeriodicFailure> {
        let authority = Authority { model: &self.model };
        periodic::analyze_periodic_bound(
            &self.model,
            &self.options,
            limits,
            policy,
            graph_labels,
            schedule,
            Some(&authority),
        )
    }
}

// The run host of a fresh binding: the run's random stream and inputs, as the
// checker installs them, so startup evaluation sees the same run.
fn run_host(options: &CheckOptions) -> RunHost {
    let mode = match options.profile {
        dive_eval::Profile::Tlc => RandomMode::Tlc,
        dive_eval::Profile::Dive => RandomMode::Dive,
    };
    let host = RunHost::new(Streams::new(options.seed, mode), None);
    if let Some(inputs) = &options.inputs {
        host.with_inputs(inputs.clone())
    } else {
        host
    }
}

fn unsupported(
    feature: impl Into<String>,
    output: Vec<dive_model::Message>,
) -> ControlledBindError {
    Box::new(dive_model::Failure {
        error: BindError::unsupported(feature),
        output,
    })
    .into()
}

fn bind(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    control: Arc<EvaluationControl>,
) -> Result<Model, ControlledBindError> {
    let bind_options = BindOptions {
        eval: options.eval,
        symmetry_liveness: options.symmetry_liveness,
        no_deadlock: options.no_deadlock,
        host: Some(Arc::new(run_host(options))),
    };
    let model = if let Some(coverage) = &options.source_coverage {
        dive_model::bind_controlled_source_covered(
            ir.clone(),
            config,
            &bind_options,
            control,
            coverage.clone(),
        )
    } else {
        dive_model::bind_controlled(ir.clone(), config, &bind_options, control)
    }?;
    if let Some(feature) = model.evaluator.external_input_refusal() {
        return Err(unsupported(feature, model.output));
    }
    if let Some(Err(error)) = options
        .inputs
        .as_ref()
        .map(|inputs| inputs.finish_binding())
    {
        return Err(unsupported(error.message, model.output));
    }
    Ok(model)
}

/// Binds the source once and dispatches on the actual bound roles.
///
/// `None` means the request is outside this route
/// ([`dive_runtime::admission::route_exclusion`], or no configured property):
/// no binding or source evaluation ran. A binding whose roles are all safety
/// is returned as [`Dispatch::Safety`] without checking anything. Otherwise
/// the one fresh temporal runtime is returned. The caller must use the
/// dedicated evaluator stack. This takes no prebound runtime or host.
///
/// # Errors
/// The controlled binding error and its original startup output.
#[must_use]
pub fn dispatch_controlled(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    control: Arc<EvaluationControl>,
) -> Option<Result<Dispatch, ControlledBindError>> {
    if config.properties.is_empty()
        || dive_runtime::admission::route_exclusion(ir, config, options).is_some()
    {
        return None;
    }
    // A caller cannot supply a prebound Dispatch::Temporal: these private
    // fields are filled only from this fresh source binding.
    Some(bind(ir, config, options, control.clone()).map(|model| {
        if model.implied_temporals.is_empty() {
            Dispatch::Safety
        } else {
            Dispatch::Temporal(Box::new(OwnedTemporal {
                model: Box::new(model),
                options: options.clone(),
                _binding_control: control,
                _thread: PhantomData,
            }))
        }
    }))
}
