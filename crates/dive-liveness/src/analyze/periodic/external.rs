//! Checks of a behavior graph built by its producer, Core.
//!
//! The producer explores and builds the graph with
//! [`dive_runtime::capture::GraphBuilder`] or its native rows; [`GraphCheck`]
//! runs the planning, completed-level checks and final check.

use dive_model::Model;
use dive_runtime::{CheckOptions, CheckResult, Stats, capture};

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
        &dive_runtime::budget::EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError>
    + 'a;

/// Quiet native truths from an earlier check of the same append-only graph.
/// For state atoms `sources` counts discovered vertices. For action atoms
/// it counts fully expanded sources and `cells` includes their original
/// parallel edges and final stutters, in physical order.
pub struct LabelPrefix<'a> {
    /// Number of immutable source rows already labeled.
    pub sources: usize,
    /// Exact bit count (padding in the last word has no meaning).
    pub cells: usize,
    /// Packed completed truths, least significant bit first.
    pub words: &'a [u64],
}

/// Value rows and optional source-bound predicate evaluation from one producer.
/// This supplies values/truth only, never completion or independent validation.
///
/// ```
/// use dive_liveness::analyze::periodic::external::NativeReader;
/// use dive_runtime::budget::EvaluationControl;
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
        control: &dive_runtime::budget::EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError>;

    /// Whether every retained row was already converted by the producer before
    /// it was retained, so a read repeats that exact conversion and can fail
    /// only by interruption or a storage limit, interning no new name. A
    /// producer may also answer true when its first read admits the pending
    /// rows in retention order, interning what eager admission would have;
    /// that read may then refuse an unconvertible row. Label demand then
    /// reads rows only for reference fallbacks. Defaults to false, which keeps
    /// eager per-vertex row demand.
    fn reads_admitted(&self) -> bool {
        false
    }

    /// Whether this reader has prepared this exact retained predicate object.
    /// This is a scheduling hint only; it cannot grant semantic authority.
    fn has_predicate(&self, _atom: &crate::plan::PredicateAtom) -> bool {
        false
    }

    /// Whether this exact action is defined and false from `before` to
    /// every physical target row in `0..rows`. A successful proof must
    /// preserve the evaluation of the subscript and action, including
    /// errors, output and row admission. The default keeps individual
    /// edge demands.
    ///
    /// # Errors
    /// An owned interruption or resource stop.
    fn action_row_false(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _before: usize,
        _rows: usize,
        _control: &dive_runtime::budget::EvaluationControl,
    ) -> Result<bool, AnalysisError> {
        Ok(false)
    }

    /// Optional state truth certified by the actual source's closed producer row.
    /// Readers must establish their graph identity, original unrestricted action,
    /// bound scope, complete emission coverage and preservation of diagnostics.
    ///
    /// # Errors
    /// An owned interruption or resource stop.
    fn closed_state_predicate(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _graph: &dive_runtime::capture::CsrCapture<'_>,
        _source: usize,
        _before: usize,
        _control: &dive_runtime::budget::EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        Ok(None)
    }

    /// Optionally evaluates the exact retained atom on original row IDs.
    /// `None` preserves ordinary reference evaluation. Predicate wrappers,
    /// unknown scopes and unproved representations must use that fallback.
    ///
    /// # Errors
    /// Only an original owned interruption/resource failure; semantic errors
    /// must retain the dive-eval evaluator and its error evidence.
    fn predicate(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _before: usize,
        _after: Option<usize>,
        _control: &dive_runtime::budget::EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        Ok(None)
    }

    /// Validates current native admission and inputs for private planner-proved
    /// reuse. This supplies no truth: the caller owns the separate frozen
    /// recipe/graph proof. Existing readers decline by default.
    ///
    /// # Errors
    /// Only the original owned interruption or resource failure.
    fn validate_predicate_inputs(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _before: usize,
        _after: Option<usize>,
        _control: &dive_runtime::budget::EvaluationControl,
    ) -> Result<bool, AnalysisError> {
        Ok(false)
    }

    /// Records that label demand answered this exact retained atom on one
    /// state or edge by the dive-eval evaluator, because no native truth was
    /// available. A counter only; it supplies no truth. Ignored by default.
    fn reference_predicate(&mut self, _atom: &crate::plan::PredicateAtom) {}

    /// Returns an earlier fully native column only when this exact bound atom
    /// and its physical graph prefix still have the same admitted values.
    ///
    /// # Errors
    /// An owned interruption or invalid producer row.
    fn label_prefix(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _graph: &capture::CsrCapture<'_>,
        _control: &dive_runtime::budget::EvaluationControl,
    ) -> Result<Option<LabelPrefix<'_>>, AnalysisError> {
        Ok(None)
    }

    /// Offers a complete, quiet column to an append-only producer after all
    /// native demands for the atom succeeded. A reader may ignore it.
    ///
    /// # Errors
    /// An owned interruption while copying the column.
    fn save_label_prefix(
        &mut self,
        _atom: &crate::plan::PredicateAtom,
        _graph: &capture::CsrCapture<'_>,
        _words: &mut dyn ExactSizeIterator<Item = u64>,
        _byte_cap: usize,
        _control: &dive_runtime::budget::EvaluationControl,
    ) -> Result<(), AnalysisError> {
        Ok(())
    }

    /// Observes elapsed wall time in one contiguous atom-label, product-search
    /// or attribution phase. Periodic and final analyses share this callback.
    /// The observation has no effect on model checking.
    fn analysis_time(&mut self, _stage: Stage, _elapsed: std::time::Duration) {}

    /// Records optional per-analysis reuse storage and bypass observations.
    /// Bytes are retained capacity, not a whole-process or allocation-time cap.
    fn predicate_reuse_storage(
        &mut self,
        _peak_bytes: usize,
        _cap_bypasses: u64,
        _allocation_bypasses: u64,
        _stored_columns: u64,
    ) {
    }
}

impl<F> NativeReader for F
where
    F: FnMut(
        usize,
        &dive_runtime::budget::EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError>,
{
    fn read(
        &mut self,
        row: usize,
        control: &dive_runtime::budget::EvaluationControl,
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
/// Core's behavior-graph producer drives these calls; planning needs only
/// the bound model.
///
/// ```
/// use std::sync::Arc;
/// use dive_runtime::CheckOptions;
/// use dive_liveness::analyze::{GraphLabelPolicy, Limits};
/// use dive_liveness::analyze::periodic::external::GraphCheck;
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
/// assert_eq!(check.options().workers, options.workers);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct GraphCheck<'m> {
    model: &'m Model,
    options: CheckOptions,
    watchdog: Option<dive_runtime::budget::Watchdog>,
    control: std::sync::Arc<dive_runtime::budget::EvaluationControl>,
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
    /// configured property of `model` before the search. `schedule` selects the completed levels to check;
    /// `None` checks only the complete graph.
    ///
    /// # Errors
    /// Planning failures, with incomplete capture.
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
        let (watchdog, armed) = dive_runtime::budget::arm(options).map_err(|error| {
            Box::new(failure(
                match error {
                    dive_runtime::budget::StartError::Measurement(error) => {
                        AnalysisError::BudgetMeasurement(error)
                    }
                    error => AnalysisError::BudgetStartup(error.to_string()),
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
            planned(
                model,
                &armed,
                limits,
                checks,
                armed.profile == dive_eval::Profile::Dive
                    && super::uncapped_component_search(limits, policy, graph_labels),
            )
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

    /// Search all configured properties on each due prefix, in configuration
    /// order. A candidate still requires attribution and independent replay.
    /// This can find a later property's word before an earlier property fails.
    #[must_use]
    pub fn with_all_prefix_properties(mut self) -> Self {
        self.d.prefix_properties = self.plans.len();
        self
    }

    /// The actual first original claim lowered for final product analysis,
    /// including its behavior fairness and every bound atom. Inspection of a
    /// plan establishes neither reachability nor a verdict.
    #[must_use]
    pub fn first_plan(&self) -> Option<&crate::plan::Plan> {
        self.plans.first().map(|property| &property.plan)
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

    /// Whether a graph producer may evaluate this run's atoms during its
    /// search, in TLC's row order (one worker, uncapped labels, only
    /// state-predicate atoms). Holding and violated results do not depend on
    /// it, but which atom error is reported first can: a producer that
    /// evaluates atoms after its search refuses such an error rather than
    /// report it in another order.
    #[must_use]
    pub fn row_order(&self) -> bool {
        crate::analyze::row_demand::may_apply(&self.plans, &self.options, self.graph_labels)
    }

    /// Callback-free optional source-row admission hint. This borrowed driver
    /// supplies no ownership certificate; the actual lane must consume a fresh
    /// [`crate::analyze::owned::OwnedTemporal`] before evaluating any row.
    ///
    /// # Errors
    /// The original controlled inspection/resource failure.
    pub fn source_rows_possible(&self) -> Result<bool, AnalysisError> {
        let _budget = crate::budget::enter(&self.options.budget);
        crate::analyze::row_demand::source_possible(
            self.model,
            &self.plans,
            &self.options,
            self.limits.formula,
            self.graph_labels,
            &self.control,
        )
    }

    /// The run options with the armed budget, for the producer's search.
    #[must_use]
    pub fn options(&self) -> &CheckOptions {
        &self.options
    }

    /// Actual completed-level check facts, or `None` for final-only analysis.
    /// A snapshot records execution so far and supplies no verdict authority.
    ///
    /// ```
    /// # fn inspect(driver: &dive_liveness::analyze::periodic::external::GraphCheck<'_>) {
    /// if let Some(checks) = driver.periodic_checks() {
    ///     let completed_prefix_checks = checks.prefixes;
    ///     # let _ = completed_prefix_checks;
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn periodic_checks(&self) -> Option<Checks> {
        self.periodic.then_some(self.d.checks)
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
    /// The final check's failures.
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
            .complete(inputs, complete)
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
