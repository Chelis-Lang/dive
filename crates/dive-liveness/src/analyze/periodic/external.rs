//! Checks of a behavior graph built outside `dive-check`'s capture, such as
//! by the execution core (`docs/developer/core/11-liveness.md`).
//!
//! The producer explores and builds the graph with
//! [`dive_check::capture::GraphBuilder`]; [`GraphCheck`] runs the same
//! planning, completed-level checks and final check as
//! [`super::analyze_periodic`], on the same graph, with the same results.

use dive_check::{CheckOptions, CheckResult, Stats, capture};
use dive_model::Model;

use super::{
    Checks, Decisions, Inputs, PeriodicAnalysis, PeriodicFailure, Schedule, budget_failure,
    close_budget, failure, planned,
};
use crate::analyze::{
    AnalysisError, EvaluationErrors, GraphLabelPolicy, Limits, PreparedProperty, Stage,
};
use crate::product::AcceptancePolicy;

pub use crate::analyze::finish::{Completed as NativeFinal, NativeWord, Prefix as NativePrefix};

/// Temporal analysis with graph-independent completed or prefix producer facts.
///
/// Candidates remain unvalidated words; this type never replaces independent
/// typed and serialized replay. A prefix cannot supply a holding result.
#[derive(Clone, Debug)]
pub enum NativeAnalysis {
    /// Completed exploration and final analysis.
    Final(NativeFinal),
    /// A selected word on a successful, incomplete committed prefix.
    Prefix(NativePrefix),
}

/// Reads an opaque retained-row id under this run's actual analysis control.
/// The producer retains the persistent roots; successful rows must contain all
/// model variables in IR order. A failed or interrupted read returns no row.
pub type NativeRead<'a> = dyn FnMut(
        usize,
        &dive_check::budget::EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError>
    + 'a;

/// Value rows and optional source-bound predicate evaluation from one producer.
/// This supplies values/truth only, never completion or independent validation.
///
/// ```
/// use dive_liveness::analyze::periodic::external::NativeReader;
/// use dive_check::budget::EvaluationControl;
/// use dive_liveness::analyze::AnalysisError;
/// let mut read = |_: usize, _: &EvaluationControl| {
///     Ok::<_, AnalysisError>(vec![Some(dive_value::Value::Int(0))].into())
/// };
/// let _reader: &mut dyn NativeReader = &mut read;
/// ```
pub trait NativeReader {
    /// Reads a complete original row under the actual analysis control.
    ///
    /// # Errors
    /// The original read refusal, interruption or resource failure.
    fn read(
        &mut self,
        row: usize,
        control: &dive_check::budget::EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError>;

    /// Whether this reader has prepared this exact retained predicate object.
    /// This is a scheduling hint only; it cannot grant semantic authority.
    fn has_predicate(&self, _atom: &crate::plan::PredicateAtom) -> bool {
        false
    }

    /// Optionally evaluates the exact retained atom on original row IDs.
    /// `None` preserves ordinary reference evaluation. Predicate wrappers,
    /// unknown scopes and unproved representations must use that fallback.
    ///
    /// # Errors
    /// Only an original owned interruption/resource failure; semantic errors
    /// must retain the existing reference evaluator and its error evidence.
    fn predicate(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _before: usize,
        _after: Option<usize>,
        _control: &dive_check::budget::EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        Ok(None)
    }
}

impl<F> NativeReader for F
where
    F: FnMut(
        usize,
        &dive_check::budget::EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError>,
{
    fn read(
        &mut self,
        row: usize,
        control: &dive_check::budget::EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError> {
        self(row, control)
    }
}

mod native;

/// The temporal checks of one run over a graph supplied by its producer.
///
/// [`Self::prepare`] plans every property before the search. During the
/// search the producer calls [`Self::level`] after each completed level,
/// with the graph so far, and stops after a level when it returns `true`.
/// It then calls [`Self::finish_prefix`], or [`Self::finish`] once the
/// exploration is complete, or [`Self::stopped`] after a safety stop. With
/// [`Schedule`] `None` (`--compat=tlc`), only the final check runs.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{CheckOptions, capture::{self, GraphBuilder}};
/// use dive_liveness::analyze::{Analysis, GraphLabelPolicy, Limits};
/// use dive_liveness::analyze::periodic::{PeriodicAnalysis, external::GraphCheck};
/// use dive_liveness::product::AcceptancePolicy;
/// let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nP == <>(~x)\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", src, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis)?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n").into_config().unwrap();
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::dive())?;
/// let options = CheckOptions::dive(0);
/// let check = GraphCheck::prepare(&model, &options, Limits::default(),
///     AcceptancePolicy::compact_default(), GraphLabelPolicy::NoCellLimit, None)
///     .map_err(|_| "planning failed")?;
/// // Here the reference capture plays the producer: one state, its step to
/// // itself, then its stutter.
/// let capture::Exploration::Complete(reference) =
///     capture::explore(&model, &options, capture::Limits::default()) else { unreachable!() };
/// let s = reference.graph.states[0].clone();
/// let mut graph = GraphBuilder::new(capture::Limits::default());
/// let built = (|| {
///     graph.record(1, s.clone(), Some(0), None, 0, "Init")?;
///     graph.record(1, s, None, Some(0), 0, "Next")?;
///     graph.stutter(0)?;
///     graph.complete(reference.safety_result().clone(), options.clone())
/// })();
/// let complete = built.map_err(|v| format!("{v:?}"))?;
/// let (result, _) = check.finish(complete).map_err(|_| "analysis failed")?;
/// assert!(matches!(result, PeriodicAnalysis::Final(Analysis::Candidate(_))));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct GraphCheck<'m> {
    model: &'m Model,
    options: CheckOptions,
    watchdog: Option<dive_check::budget::Watchdog>,
    control: std::sync::Arc<dive_check::budget::EvaluationControl>,
    plans: Vec<PreparedProperty>,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    periodic: bool,
    d: Decisions,
}

impl std::fmt::Debug for GraphCheck<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphCheck")
            .field("properties", &self.plans.len())
            .field("periodic", &self.periodic)
            .field("checks", &self.d.checks)
            .finish_non_exhaustive()
    }
}

impl<'m> GraphCheck<'m> {
    /// Arms the run budget (unless the caller already did) and plans every
    /// configured property of `model`, as [`super::analyze_periodic`] does
    /// before its capture. `schedule` selects the completed levels to check;
    /// `None` checks only the complete graph.
    ///
    /// # Errors
    /// The planning failures of [`super::analyze_periodic`], with incomplete
    /// capture.
    pub fn prepare(
        model: &'m Model,
        options: &CheckOptions,
        limits: Limits,
        policy: AcceptancePolicy,
        graph_labels: GraphLabelPolicy,
        schedule: Option<Schedule>,
    ) -> Result<Self, Box<PeriodicFailure>> {
        let checks = Checks {
            schedule: schedule.unwrap_or_default(),
            ..Checks::default()
        };
        let (watchdog, armed) = dive_check::budget::arm(options).map_err(|error| {
            Box::new(failure(
                match error {
                    dive_check::budget::StartError::Measurement(error) => {
                        AnalysisError::BudgetMeasurement(error)
                    }
                    error @ dive_check::budget::StartError::Thread(_) => {
                        AnalysisError::BudgetStartup(error.to_string())
                    }
                },
                Stage::Planning,
                None,
                EvaluationErrors::default(),
                Some(false),
                checks,
            ))
        })?;
        let armed = armed.into_owned();
        let planned = {
            let _scope = crate::budget::enter(&armed.budget);
            planned(model, &armed, limits, checks)
                .map_err(|failure| Box::new(budget_failure(failure, &armed.budget)))?
        };
        let (control, plans) = planned;
        Ok(Self {
            model,
            options: armed,
            watchdog,
            control,
            plans,
            limits,
            policy,
            graph_labels,
            periodic: schedule.is_some(),
            d: Decisions::new(checks),
        })
    }

    /// The actual retained source-scoped predicates, in configured order.
    /// Inspection grants no evaluation or completion authority. References
    /// remain in these fixed plan allocations until this driver is consumed.
    ///
    /// ```
    /// # fn inspect(driver: &dive_liveness::analyze::periodic::external::GraphCheck<'_>) {
    /// let atom_count = driver.native_predicates().count();
    /// # let _ = atom_count;
    /// # }
    /// ```
    pub fn native_predicates(&self) -> impl Iterator<Item = &crate::plan::PredicateAtom> {
        self.plans.iter().flat_map(|property| &property.plan.atoms)
    }

    /// Whether the reference engine may evaluate this run's atoms during its
    /// search, in TLC's row order (one worker, uncapped labels, only
    /// state-predicate atoms). Holding and violated results do not depend on
    /// it, but which atom error is reported first can: a producer that
    /// evaluates atoms after its search refuses such an error rather than
    /// report it in another order.
    #[must_use]
    pub fn row_order(&self) -> bool {
        crate::analyze::row_demand::may_apply(&self.plans, &self.options, self.graph_labels)
    }

    /// The run options with the armed budget, for the producer's search.
    #[must_use]
    pub fn options(&self) -> &CheckOptions {
        &self.options
    }

    /// Checks `graph` after the completed `level`, whose counters are
    /// `stats`, when the schedule says so: whether the search stops after
    /// this level, to finish with [`Self::finish_prefix`].
    pub fn level(&mut self, level: u64, graph: &capture::BehaviorGraph, stats: Stats) -> bool {
        if self.periodic && self.d.decision.is_none() {
            let inputs = Inputs {
                budget: &self.options.budget,
                control: &self.control,
                model: self.model,
                plans: &self.plans,
                limits: self.limits,
                policy: self.policy,
                graph_labels: self.graph_labels,
            };
            self.d.level(inputs, level, graph, stats);
        }
        self.d.decision.is_some()
    }

    /// The result after the search stopped at `prefix` because
    /// [`Self::level`] returned `true`.
    ///
    /// # Errors
    /// The level check's failure, or a prefix without a decision.
    pub fn finish_prefix(
        mut self,
        prefix: capture::Prefix,
    ) -> Result<(PeriodicAnalysis, Checks), Box<PeriodicFailure>> {
        let _scope = crate::budget::enter(&self.options.budget);
        let budget = self.options.budget.clone();
        let mut watchdog = self.watchdog.take();
        let result = self
            .d
            .prefix(prefix)
            .map_err(|failure| budget_failure(failure, &budget));
        close_budget(watchdog.as_mut(), result).map_err(Box::new)
    }

    /// The final check of the complete exploration `complete`.
    ///
    /// # Errors
    /// The final check's failures, as [`super::analyze_periodic`] reports
    /// them.
    pub fn finish(
        self,
        complete: capture::Complete,
    ) -> Result<(PeriodicAnalysis, Checks), Box<PeriodicFailure>> {
        let Self {
            model,
            options,
            mut watchdog,
            control,
            plans,
            limits,
            policy,
            graph_labels,
            d,
            ..
        } = self;
        let _scope = crate::budget::enter(&options.budget);
        let inputs = Inputs {
            budget: &options.budget,
            control: &control,
            model,
            plans: &plans,
            limits,
            policy,
            graph_labels,
        };
        let result = d
            .complete(inputs, complete, None)
            .map_err(|failure| budget_failure(failure, &options.budget));
        close_budget(watchdog.as_mut(), result).map_err(Box::new)
    }

    /// The result after the producer's search stopped with `result` (a
    /// safety ending, a resource limit or a stop): never a temporal verdict.
    #[must_use]
    pub fn stopped(self, result: CheckResult) -> Box<PeriodicFailure> {
        Box::new(failure(
            AnalysisError::Exploration(Box::new(result)),
            Stage::Capture,
            None,
            EvaluationErrors::default(),
            Some(false),
            self.d.checks,
        ))
    }
}
