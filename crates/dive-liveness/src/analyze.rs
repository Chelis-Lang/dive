//! Pure liveness analysis through the shared safety engine.
//!
//! This library coordinator does not implement TLC's exact planning/selection
//! or symmetry. [`periodic`] adds deterministic completed-level checks. WF/SF expansion uses TLC-side enabledness. Counterexample candidates
//! still require independent validation; direct formula evaluation attributes
//! their complete configured property set.
//! Prefix candidates require independent validation and never prove `Holds`.

use dive_eval::{St, next::StateVals};
use dive_model::Model;
use dive_runtime::{CheckOptions, CheckResult, capture};

use crate::formula::{self, AtomKind, CycleConditions, Tableau};
use crate::plan::{self, Plan, PlanningError};
use crate::product::{self, AcceptancePolicy, ImplicitProduct, PackedTruth};

mod fairness;
mod finish;
#[doc(hidden)]
pub mod owned;
mod row_demand;
mod state_source;

use state_source::{CapturedStates, OriginalTarget, StateSource};

/// Combined bounds on planning, exploration and explicit product construction.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Limits for each combined behavior/property formula and tableau.
    pub formula: formula::Limits,
    /// Safety graph retention limits.
    pub capture: capture::Limits,
    /// Per-property product limits.
    pub product: product::Limits,
    /// Maximum temporal properties planned before exploration.
    pub properties: usize,
    /// Shared graph-label, attribution-row and word-truth cell ceiling.
    /// Existing entrypoints retain it for every owner; explicit graph policy
    /// changes only graph labels, leaving the latter two owners unchanged.
    pub atom_cells: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            formula: formula::Limits::default(),
            capture: capture::Limits::default(),
            product: product::Limits::default(),
            properties: 256,
            atom_cells: 8_000_000,
        }
    }
}

/// Admission of logical cells for the captured graph's atom labels only.
/// Attribution rows, word truth, products and capture retain their own limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphLabelPolicy {
    /// Enforce this exact logical `(states + edges) * atoms` ceiling before
    /// allocation or predicate evaluation, including zero and default values.
    Cells(usize),
    /// Remove only the logical graph-label count ceiling. Existing configured
    /// run budgets, checked packed layout and fallible allocation still apply.
    /// This does not promise an allocation-time memory or wall-clock hard cap.
    NoCellLimit,
}

impl GraphLabelPolicy {
    fn admit(self, states: usize, edges: usize, atoms: usize) -> Result<(), AnalysisError> {
        if let Self::Cells(limit) = self {
            states
                .checked_add(edges)
                .and_then(|n| n.checked_mul(atoms))
                .filter(|n| *n <= limit)
                .ok_or(AnalysisError::Limit("atom cells"))?;
        }
        Ok(())
    }
}

/// The final-only analysis of the supported pure temporal subset.
#[derive(Clone, Debug)]
pub enum Analysis {
    /// Every configured temporal property holds on the captured behavior graph.
    /// The capture retains the store's collision qualifications and statistics.
    Holds(capture::Complete),
    /// A concrete candidate falsifies the target under the behavior conjuncts.
    /// The complete claim set is attributed, but still requires independent validation.
    Candidate(Candidate),
}

impl Analysis {
    /// Releases the captured graph and retains the selected word and original
    /// producer facts. Candidates still require independent validation.
    ///
    /// ```
    /// use dive_liveness::analyze::{Analysis, Failure};
    /// use dive_liveness::analyze::periodic::external::NativeFinal;
    /// fn release_graph(result: Result<Analysis, Failure>) -> Result<NativeFinal, Failure> {
    ///     result.map(Analysis::into_native)
    /// }
    /// ```
    #[must_use]
    pub fn into_native(self) -> periodic::external::NativeFinal {
        match self {
            Self::Holds(captured) => finish::Completed::Holds(captured.into_evidence()),
            Self::Candidate(candidate) => finish::Completed::Candidate(
                finish::NativeWord {
                    target_property: candidate.target_property,
                    properties: candidate.properties,
                    states: candidate.states,
                    back_to: candidate.back_to,
                    edges: candidate.edges,
                },
                candidate.exploration.into_evidence(),
            ),
        }
    }
}

/// A concrete periodic word projected from an accepting product walk.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// The property whose negation selected this candidate. Other properties
    /// may also be false and must be attributed before report validation.
    pub target_property: String,
    /// Complete set of configured temporal properties false on this word,
    /// attributed by direct formula evaluation, not accepting-product visits.
    pub properties: Vec<String>,
    /// Concrete word positions, without repeating the closing target.
    pub states: Vec<StateVals>,
    /// Zero-based target of the final transition.
    pub back_to: usize,
    /// Actual graph edges, including the closing edge and implicit stutters.
    pub edges: Vec<capture::Edge>,
    /// The complete safety exploration evidence, including collision figures.
    pub exploration: capture::Complete,
}

/// A named refusal or error, never evidence that a property holds.
#[derive(Clone, Debug)]
pub enum AnalysisError {
    /// A requested run budget could not be armed; no capture or analysis ran.
    BudgetStartup(String),
    /// Process-memory measurement failed; no invented exhaustion is reported.
    BudgetMeasurement(dive_runtime::budget::MeasurementError),
    /// Original combined TLC normalization produced no liveness solutions,
    /// before assumptions or exploration. Retains the original startup output.
    StartupTautology {
        /// Output from the one completed binding invocation.
        output: Vec<dive_model::Message>,
    },
    /// Pure predicate planning failed.
    Planning(PlanningError),
    /// Tableau construction failed.
    Tableau(formula::FormulaError),
    /// Product construction or acceptance search failed.
    Product(product::ProductError),
    /// Safety exploration stopped or refused.
    Exploration(Box<CheckResult>),
    /// Atom evaluation failed or was nonboolean, retaining property/source.
    Atom {
        /// Property being checked.
        property: String,
        /// Source span and location of the predicate.
        source: Box<plan::Source>,
        /// Error or unsupported value.
        detail: String,
    },
    /// A named coordinator resource bound.
    Limit(&'static str),
    /// The requested mode is not implemented by this coordinator.
    Unsupported(&'static str),
    /// A concrete producer value cannot be reconstructed exactly.
    UnsupportedValue(String),
    /// Internal disagreement between acceptance and direct attribution.
    Inconsistent(&'static str),
    /// A capture thread failure lost execution evidence.
    CaptureThread(String),
    /// The run's budget ran out after capture. Never a verdict;
    /// a stop during capture is an [`Self::Exploration`] with
    /// [`dive_runtime::Verdict::Stopped`].
    Stopped(Box<dive_runtime::budget::Stopped>),
}
impl AnalysisError {
    /// Stable diagnostics for the library analysis stages. These are dive
    /// diagnostics, not a claim of TLC's planning/error classification.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::StartupTautology { .. } => "TLC_LIVE_FORMULA_TAUTOLOGY",
            Self::Planning(e) => e.code(),
            Self::Tableau(_) => "DIVE-L0030",
            Self::Product(_) => "DIVE-L0031",
            Self::Exploration(result)
                if matches!(
                    result.verdict,
                    dive_runtime::Verdict::BudgetMeasurement { .. }
                ) =>
            {
                dive_runtime::budget::MEASUREMENT_FAILURE_CODE
            }
            Self::Exploration(_) => "DIVE-L0032",
            Self::Atom { .. } => "DIVE-L0033",
            Self::Limit(_) => "DIVE-L0034",
            Self::Unsupported(_) | Self::UnsupportedValue(_) => "DIVE-L0035",
            Self::Inconsistent(_) => "DIVE-L0036",
            Self::CaptureThread(_) => "DIVE-L0037",
            Self::BudgetStartup(_) => dive_runtime::budget::START_FAILURE_CODE,
            Self::BudgetMeasurement(_) => dive_runtime::budget::MEASUREMENT_FAILURE_CODE,
            Self::Stopped(_) => "DIVE-L0038",
        }
    }
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartupTautology { .. } => {
                dive_model::BindError::tlc(dive_model::Ec::TlcLiveFormulaTautology, &[]).fmt(f)
            }
            Self::Planning(e) => e.fmt(f),
            Self::Tableau(e) => e.fmt(f),
            Self::Product(e) => e.fmt(f),
            Self::Exploration(v) => write!(f, "liveness exploration stopped: {:?}", v.verdict),
            Self::Atom {
                property,
                source,
                detail,
            } => write!(
                f,
                "{}: liveness atom for {property} at {}: {detail}",
                self.code(),
                source.location
            ),
            Self::Limit(s) => write!(f, "liveness analysis resource limit: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported liveness analysis: {s}"),
            Self::UnsupportedValue(s) => write!(f, "unsupported liveness analysis: {s}"),
            Self::Inconsistent(s) => write!(f, "internal liveness analysis error: {s}"),
            Self::CaptureThread(s) => write!(f, "liveness capture thread failure: {s}"),
            Self::BudgetStartup(s) => f.write_str(s),
            Self::BudgetMeasurement(error) => error.fmt(f),
            Self::Stopped(s) => write!(f, "{}: {s}", self.code()),
        }
    }
}
impl std::error::Error for AnalysisError {}

/// The analysis stage reached, including on a refusal or resource failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Checking supported modes and compiling property plans.
    Planning,
    /// Capturing the behavior graph through the safety engine.
    Capture,
    /// Evaluating state and edge predicates on the completed graph.
    Atoms,
    /// Constructing or searching an accepting product.
    Product,
    /// Determining the complete false-property set on a candidate.
    Attribution,
    /// Resource sampling after the analysis completed.
    Finalization,
    /// A capture thread failure prevented recovery of actual execution facts.
    Unknown,
}
impl Stage {
    /// Stable name for structured execution reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Capture => "capture",
            Self::Atoms => "atoms",
            Self::Product => "product",
            Self::Attribution => "attribution",
            Self::Finalization => "finalization",
            Self::Unknown => "unknown",
        }
    }
}

/// Completed exploration facts, without retaining the potentially large graph.
/// These facts are not evidence that temporal analysis succeeded.
#[derive(Clone, Debug)]
pub struct ExplorationEvidence {
    /// The capture's complete state-space counters.
    pub stats: dive_runtime::Stats,
    /// Requested and actual capture workers.
    pub workers: dive_runtime::Workers,
    /// Collision qualification, absent for exact storage and when the
    /// producer computes none (Core); the safety result's store says which.
    pub collision: Option<dive_runtime::Collision>,
    /// Startup and exploration output in execution order.
    pub output: Vec<String>,
    /// Startup warnings with TLC message codes.
    pub warnings: Vec<(u32, String)>,
}
impl From<&capture::Complete> for ExplorationEvidence {
    fn from(capture: &capture::Complete) -> Self {
        Self {
            stats: capture.stats,
            workers: capture.workers.clone(),
            collision: capture.collision.clone(),
            output: capture.output.clone(),
            warnings: capture.warnings.clone(),
        }
    }
}

/// A failed temporal analysis retaining the work that actually completed.
#[derive(Clone, Debug)]
pub struct Failure {
    /// The refusal, bound or error; never a model-checking verdict.
    pub error: AnalysisError,
    /// The stage where the failure occurred.
    pub stage: Stage,
    /// Completed capture evidence, when analysis reached a later stage.
    pub exploration: Option<Box<ExplorationEvidence>>,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// The actual typed cause of an active selected-target action evaluation failure.
/// This is an error description, not a liveness verdict or admission receipt.
#[derive(Clone, Debug)]
pub struct ActionEvaluation {
    cause: dive_eval::EvalError,
    property: String,
    source: Box<plan::Source>,
    properties: usize,
    core: bool,
    direct: bool,
}
impl ActionEvaluation {
    /// Original class, code, message and expression origin.
    #[must_use]
    pub fn cause(&self) -> &dive_eval::EvalError {
        &self.cause
    }
    /// Actual core evaluation error, absent for a synthesized nonboolean result.
    #[must_use]
    pub fn core_cause(&self) -> Option<&dive_eval::EvalError> {
        self.core.then_some(&self.cause)
    }
    /// Whether the normalized formula is one EA/AE opaque action operand.
    /// This is syntax context, not a liveness verdict or evaluation-order claim.
    #[must_use]
    pub const fn direct_operand(&self) -> bool {
        self.direct
    }
    /// Property whose action label was being evaluated.
    #[must_use]
    pub fn property(&self) -> &str {
        &self.property
    }
    /// Bound predicate's source context.
    #[must_use]
    pub fn source(&self) -> &plan::Source {
        &self.source
    }
    /// Number of configured temporal claims, before per-property analysis.
    #[must_use]
    pub const fn properties(&self) -> usize {
        self.properties
    }
}

/// Where a state atom actually failed in the existing analysis schedule.
/// A capture site alone does not certify demand; use its separate producer proof.
///
/// ```
/// use dive_liveness::analyze::StateEvaluationSite;
/// let site = StateEvaluationSite::FinalLabels;
/// assert_ne!(site, StateEvaluationSite::PeriodicLabels);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateEvaluationSite {
    /// An actually reached source-row call, before the next source expands.
    CaptureRow,
    /// An actual singleton temporal source-row error after Next was processed,
    /// with its separate first-primary and diagnostic-suppression evidence.
    CaptureSourceRow,
    /// Graph labels evaluated after complete exploration.
    FinalLabels,
    /// Graph labels evaluated during a due completed-level prefix check.
    PeriodicLabels,
    /// Candidate-word attribution after complete exploration.
    FinalAttribution,
    /// Candidate-word attribution during a completed-level prefix check.
    PeriodicAttribution,
}

/// An original state-atom cause, without an ordinary error verdict.
/// The inner cause's expression origin and the bound predicate's outer source
/// are retained separately. Post-hoc evidence has no demand authority.
/// A certified row additionally retains its finite producer diagnostic path,
/// never the scope, evaluator or complete behavior graph.
///
/// ```no_run
/// # fn example(failure: &dive_liveness::analyze::DetailedFailure) {
/// if let Some(state) = failure.state_evaluation() {
///     let _actual_site = state.site();
///     let _original_cause = state.core_cause();
///     let _outer_source = state.source();
/// }
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct StateEvaluation {
    cause: dive_eval::EvalError,
    property: String,
    source: Box<plan::Source>,
    core: bool,
    site: StateEvaluationSite,
    row: Option<Box<CaptureRowEvidence>>,
    source_row: Option<Box<SourceRowEvidence>>,
}

/// Opaque producer proof for an ordered, MUST-complete source-row demand.
/// Construction requires the private compiler certificate and the capture
/// producer's matching typed decision, actual statistics and diagnostic path.
/// This is not independent validation or an ordinary error verdict.
///
/// ```
/// use dive_liveness::analyze::StateEvaluation;
/// fn inspect(state: &StateEvaluation) {
///     if let Some(row) = state.capture_row() {
///         let _original_source = row.source();
///         let _actual_statistics = row.result().stats;
///     }
/// }
/// # let _ = inspect;
/// ```
#[derive(Clone, Debug)]
pub struct CaptureRowEvidence {
    source: usize,
    result: Box<CheckResult>,
    _proof: crate::tlc_plan::demand::Certificate,
}

/// Producer execution evidence for an error in known ordered temporal bins.
/// It proves no residual/tableau truth, graph completion or independent word.
#[derive(Clone, Debug)]
pub struct SourceRowEvidence {
    source: usize,
    result: Box<CheckResult>,
    primary: dive_eval::EvalError,
    diagnostic: dive_eval::EvalError,
    _proof: crate::tlc_plan::demand::SourceCertificate,
}

impl SourceRowEvidence {
    /// Actual source identity of the row event.
    #[must_use]
    pub const fn source(&self) -> usize {
        self.source
    }

    /// Original unsupported capture stop with actual counters and source word.
    #[must_use]
    pub fn result(&self) -> &CheckResult {
        &self.result
    }

    /// First actual primary: the preceding Next failure, or the row failure.
    #[must_use]
    pub fn primary(&self) -> &dive_eval::EvalError {
        &self.primary
    }

    /// Error actually reached by the row's diagnostic replay.
    #[must_use]
    pub fn diagnostic(&self) -> &dive_eval::EvalError {
        &self.diagnostic
    }
}

impl CaptureRowEvidence {
    /// Original dense capture source identity, not a projected product index.
    #[must_use]
    pub const fn source(&self) -> usize {
        self.source
    }

    /// Original unsupported stop with actual counters and finite source path.
    /// Its verdict is not rewritten by producing this evidence.
    #[must_use]
    pub fn result(&self) -> &CheckResult {
        &self.result
    }
}
impl StateEvaluation {
    /// Original class, code, message and inner expression origin.
    #[must_use]
    pub fn cause(&self) -> &dive_eval::EvalError {
        &self.cause
    }
    /// Actual core evaluation error, absent for a synthesized nonboolean result.
    #[must_use]
    pub fn core_cause(&self) -> Option<&dive_eval::EvalError> {
        self.core.then_some(&self.cause)
    }
    /// Property whose state atom failed.
    #[must_use]
    pub fn property(&self) -> &str {
        &self.property
    }
    /// Bound predicate's outer source, independent of the original inner cause.
    #[must_use]
    pub fn source(&self) -> &plan::Source {
        &self.source
    }
    /// Actual evaluation site; a capture site requires its separate proof.
    #[must_use]
    pub const fn site(&self) -> StateEvaluationSite {
        self.site
    }

    /// A sealed matching capture-row decision, absent for all posthoc failures.
    /// Callers still need independent finite-behavior validation and a qualified
    /// temporal error classification before reporting an ordinary error.
    #[must_use]
    pub fn capture_row(&self) -> Option<&CaptureRowEvidence> {
        self.row.as_deref()
    }

    /// Sealed actual singleton source-row error and diagnostic execution.
    /// Fresh independent source replay is still required before reporting it.
    #[must_use]
    pub fn capture_source_row(&self) -> Option<&SourceRowEvidence> {
        self.source_row.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
struct EvaluationErrors {
    action: Option<Box<ActionEvaluation>>,
    state: Option<Box<StateEvaluation>>,
    periodic: bool,
}

impl EvaluationErrors {
    fn state_site(&self, attribution: bool) -> StateEvaluationSite {
        match (self.periodic, attribution) {
            (false, false) => StateEvaluationSite::FinalLabels,
            (true, false) => StateEvaluationSite::PeriodicLabels,
            (false, true) => StateEvaluationSite::FinalAttribution,
            (true, true) => StateEvaluationSite::PeriodicAttribution,
        }
    }
}

struct StateContext<'a> {
    property: &'a str,
    errors: &'a mut EvaluationErrors,
    control: &'a dive_runtime::budget::EvaluationControl,
    attribution: bool,
}

/// A failure retaining the plain errors and original typed evaluation causes.
/// The existing entry points keep their error shapes; no diagnostic is parsed.
#[derive(Clone, Debug)]
pub struct DetailedFailure {
    failure: Failure,
    evaluations: EvaluationErrors,
}
impl DetailedFailure {
    /// The plain failure and completed execution facts.
    #[must_use]
    pub fn failure(&self) -> &Failure {
        &self.failure
    }
    /// Actual active action error, absent for state/attribution and other failures.
    #[must_use]
    pub fn action_evaluation(&self) -> Option<&ActionEvaluation> {
        self.evaluations.action.as_deref()
    }
    /// Original state-atom cause and actual site, without verdict/order parity.
    #[must_use]
    pub fn state_evaluation(&self) -> Option<&StateEvaluation> {
        self.evaluations.state.as_deref()
    }
    /// Takes the state evidence before consuming the plain two-part result.
    /// This transfers only the failure description; no runtime owner is retained.
    ///
    /// ```
    /// # fn example(mut failure: dive_liveness::analyze::DetailedFailure) {
    /// let state = failure.take_state_evaluation();
    /// let (plain, action) = failure.into_parts();
    /// # let _ = (state, plain, action);
    /// # }
    /// ```
    #[must_use]
    pub fn take_state_evaluation(&mut self) -> Option<Box<StateEvaluation>> {
        self.evaluations.state.take()
    }
    /// Consumes this description without changing the public error shape.
    #[must_use]
    pub fn into_parts(self) -> (Failure, Option<Box<ActionEvaluation>>) {
        (self.failure, self.evaluations.action)
    }
}
impl std::fmt::Display for DetailedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.failure.fmt(f)
    }
}
impl std::error::Error for DetailedFailure {}

fn evaluator_control(
    opts: &CheckOptions,
) -> Result<std::sync::Arc<dive_runtime::budget::EvaluationControl>, AnalysisError> {
    dive_runtime::budget::EvaluationControl::new(
        &opts.budget,
        dive_runtime::budget::Phase::Temporal,
    )
    .map_err(|_| AnalysisError::Inconsistent("evaluator control identity is unrepresentable"))
}

fn evaluator_interruption(
    control: &dive_runtime::budget::EvaluationControl,
    error: &dive_eval::EvalError,
) -> Option<AnalysisError> {
    control.verdict(error).map(|verdict| match verdict {
        dive_runtime::Verdict::Stopped(stopped) => AnalysisError::Stopped(stopped),
        dive_runtime::Verdict::BudgetMeasurement { error } => {
            AnalysisError::BudgetMeasurement(error)
        }
        _ => AnalysisError::Inconsistent("unexpected evaluator interruption classification"),
    })
}

fn planning_failure(
    control: &dive_runtime::budget::EvaluationControl,
    error: plan::PlanningError,
) -> AnalysisError {
    if let plan::PlanningError::Evaluation { error: cause, .. } = &error
        && let Some(interruption) = evaluator_interruption(control, cause)
    {
        return interruption;
    }
    AnalysisError::Planning(error)
}

fn atom(
    model: &Model,
    plan: &Plan,
    index: usize,
    st: &St,
    context: &mut StateContext<'_>,
) -> Result<bool, AnalysisError> {
    let pred = &plan.atoms[index];
    pred.evaluate_detailed(model, st)
        .map_err(|failure| state_failure(model, pred, failure, context))
}

fn state_failure(
    model: &Model,
    pred: &plan::PredicateAtom,
    failure: plan::AtomError,
    context: &mut StateContext<'_>,
) -> AnalysisError {
    let core = matches!(&failure, plan::AtomError::Core(_));
    let cause = failure.into_error();
    if let Some(interruption) = evaluator_interruption(context.control, &cause) {
        return interruption;
    }
    let source = Box::new(plan::Source::new(model, pred.pred.location));
    let error = AnalysisError::Atom {
        property: context.property.into(),
        source: source.clone(),
        detail: cause.to_string(),
    };
    if pred.kind == AtomKind::State {
        context.errors.state = Some(Box::new(StateEvaluation {
            cause,
            property: context.property.into(),
            source,
            core,
            site: context.errors.state_site(context.attribution),
            row: None,
            source_row: None,
        }));
    }
    error
}

fn action_failure(
    model: &Model,
    pred: &plan::PredicateAtom,
    failure: plan::AtomError,
    context: &mut StateContext<'_>,
    direct: bool,
) -> AnalysisError {
    let core = matches!(&failure, plan::AtomError::Core(_));
    let cause = failure.into_error();
    if let Some(interruption) = evaluator_interruption(context.control, &cause) {
        return interruption;
    }
    let source = Box::new(plan::Source::new(model, pred.pred.location));
    let error = AnalysisError::Atom {
        property: context.property.into(),
        source: source.clone(),
        detail: cause.to_string(),
    };
    context.errors.action = Some(Box::new(ActionEvaluation {
        cause,
        property: context.property.into(),
        source,
        properties: model.property_claims().len(),
        core,
        direct,
    }));
    error
}

/// Original edge metadata without owning or reconstructing state rows.
#[derive(Clone, Copy)]
enum OriginalEdges<'a> {
    Behavior(&'a capture::BehaviorGraph),
    Native(&'a capture::CsrCapture<'a>),
}

#[derive(Clone, Copy)]
struct OriginalEdge<'a> {
    to: usize,
    ordinal: Option<u64>,
    label: Option<&'a str>,
}

impl<'a> From<&'a capture::BehaviorGraph> for OriginalEdges<'a> {
    fn from(graph: &'a capture::BehaviorGraph) -> Self {
        Self::Behavior(graph)
    }
}

impl<'a> OriginalEdges<'a> {
    fn states(self) -> usize {
        match self {
            Self::Behavior(graph) => graph.edges.len(),
            Self::Native(graph) => graph.state_count(),
        }
    }
    fn len(self, source: usize) -> Option<usize> {
        match self {
            Self::Behavior(graph) => graph.edges.get(source).map(Vec::len),
            Self::Native(graph) => graph.outgoing_range(source).map(|range| range.len()),
        }
    }
    fn edge(self, source: usize, position: usize) -> Option<OriginalEdge<'a>> {
        match self {
            Self::Behavior(graph) => {
                graph
                    .edges
                    .get(source)?
                    .get(position)
                    .map(|edge| OriginalEdge {
                        to: edge.to,
                        ordinal: edge.ordinal,
                        label: edge.label.as_deref(),
                    })
            }
            Self::Native(graph) => {
                let range = graph.outgoing_range(source)?;
                let at = range.start.checked_add(position)?;
                if at >= range.end {
                    return None;
                }
                let (ordinal, label) = graph.metadata(source, position)?;
                Some(OriginalEdge {
                    to: graph.target(at)?,
                    ordinal,
                    label,
                })
            }
        }
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
struct SelectedTarget {
    values: StateVals,
    original: OriginalTarget,
}

/// The first original per destination, with its values read eagerly (the
/// reference demand order).
#[cfg(test)]
fn selected_targets<'a>(
    graph: impl Into<OriginalEdges<'a>>,
    source: usize,
    states: &mut (impl StateSource + ?Sized),
) -> Result<std::collections::BTreeMap<usize, SelectedTarget>, AnalysisError> {
    selected_originals(graph, source, states, true)?
        .into_iter()
        .map(|(to, selected)| {
            let values = selected
                .values
                .ok_or(AnalysisError::Inconsistent("eager original values"))?;
            Ok((
                to,
                SelectedTarget {
                    values,
                    original: selected.original,
                },
            ))
        })
        .collect()
}

/// A selected original whose values are present only under eager demand.
#[derive(Debug, PartialEq)]
struct SelectedOriginal {
    values: Option<StateVals>,
    original: OriginalTarget,
}

/// Validates the complete source edge row and selects the first physical
/// original per destination. With `eager`, each selected row is read in edge
/// order exactly as reference demand does; otherwise no value is read.
fn selected_originals<'a>(
    graph: impl Into<OriginalEdges<'a>>,
    source: usize,
    states: &mut (impl StateSource + ?Sized),
    eager: bool,
) -> Result<std::collections::BTreeMap<usize, SelectedOriginal>, AnalysisError> {
    let graph = graph.into();
    let malformed = || AnalysisError::Inconsistent("original action target row shape");
    let originals = states.concrete_count(source).ok_or_else(malformed)?;
    let mut original = 0;
    let mut selected = std::collections::BTreeMap::new();
    for position in 0..graph.len(source).ok_or_else(malformed)? {
        if position.is_multiple_of(1024) && crate::budget::exhausted() {
            return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
        }
        let edge = graph.edge(source, position).ok_or_else(malformed)?;
        if edge.to >= states.representative_count() {
            return Err(malformed());
        }
        if let Some(ordinal) = edge.ordinal {
            let metadata = states
                .concrete_metadata(source, original)
                .ok_or_else(malformed)?;
            if metadata.ordinal != ordinal || edge.label != Some(metadata.label) {
                return Err(malformed());
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = selected.entry(edge.to) {
                entry.insert(SelectedOriginal {
                    values: if eager {
                        Some(states.concrete(source, original)?)
                    } else {
                        None
                    },
                    original: OriginalTarget::Generated(original),
                });
            }
            original += 1;
        } else {
            if edge.to != source {
                return Err(malformed());
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = selected.entry(source) {
                entry.insert(SelectedOriginal {
                    values: if eager {
                        Some(states.representative(source)?.into_owned())
                    } else {
                        None
                    },
                    original: OriginalTarget::Stuttering,
                });
            }
        }
    }
    if original != originals {
        return Err(malformed());
    }
    Ok(selected)
}

/// Reads a selected original's values for a reference fallback.
fn original_values(
    states: &mut (impl StateSource + ?Sized),
    source: usize,
    target: &SelectedOriginal,
    before: &StateVals,
) -> Result<StateVals, AnalysisError> {
    if let Some(values) = &target.values {
        return Ok(values.clone());
    }
    match target.original {
        OriginalTarget::Generated(original) => states.concrete(source, original),
        // The implicit stutter's after-state is the source representative.
        OriginalTarget::Stuttering => Ok(before.clone()),
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments, reason = "original label test entrypoint")]
fn labels<'a>(
    model: &Model,
    plan: &Plan,
    graph: impl Into<OriginalEdges<'a>>,
    states: &mut (impl StateSource + ?Sized),
    shape: product::BehaviorView<'a>,
    property: &str,
    policy: GraphLabelPolicy,
    active: &(std::collections::BTreeSet<usize>, Option<usize>),
    evaluations: &mut EvaluationErrors,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<PackedTruth<'a>, AnalysisError> {
    let mut reuse = fairness::Cache::new(false, 0);
    labels_with_reuse(
        model,
        plan,
        graph,
        states,
        shape,
        property,
        policy,
        active,
        evaluations,
        control,
        None,
        &mut reuse,
    )
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "concrete values and validated original-edge shape have separate owners"
)]
fn labels_with_reuse<'a>(
    model: &Model,
    plan: &Plan,
    graph: impl Into<OriginalEdges<'a>>,
    states: &mut (impl StateSource + ?Sized),
    shape: product::BehaviorView<'a>,
    property: &str,
    policy: GraphLabelPolicy,
    active: &(std::collections::BTreeSet<usize>, Option<usize>),
    evaluations: &mut EvaluationErrors,
    control: &dive_runtime::budget::EvaluationControl,
    frozen_prefix: Option<plan::FrozenPrefix>,
    reuse: &mut fairness::Cache,
) -> Result<PackedTruth<'a>, AnalysisError> {
    let graph = graph.into();
    let edges = (0..graph.states())
        .try_fold(0_usize, |total, source| {
            total.checked_add(graph.len(source)?)
        })
        .ok_or(AnalysisError::Limit("atom cells"))?;
    policy.admit(states.representative_count(), edges, plan.atoms.len())?;
    // Owned rows took their vertex dimension from actual captured states.
    // Packing must not turn a missing state row into unwritten FALSE bits.
    if states.representative_count() != shape.vertex_count()
        || graph.states() != shape.vertex_count()
    {
        return Err(AnalysisError::Product(product::ProductError::Shape(
            "state predicate dimensions".into(),
        )));
    }
    reuse.configure(states.representative_count(), edges);
    let kinds = plan.atoms.iter().map(|atom| atom.kind).collect::<Vec<_>>();
    let mut labels = PackedTruth::new(shape, &kinds).map_err(AnalysisError::Product)?;
    let mut context = StateContext {
        property,
        errors: evaluations,
        control,
        attribution: false,
    };
    // Admitted native rows are read only for a reference fallback; other
    // sources keep their eager per-vertex demand order.
    let eager = !states.defers_reads();
    for (index, pred) in plan.atoms.iter().enumerate() {
        let recipe = frozen_prefix.and_then(|prefix| prefix.atom(index, pred.kind));
        let mut fully_native = true;
        let mut consumed = false;
        let mut start = 0;
        if !eager
            && let OriginalEdges::Native(csr) = graph
            && let Some(prefix) = states.label_prefix(pred)?
        {
            let limit = match pred.kind {
                AtomKind::State => (csr.state_count() >= prefix.sources).then_some(prefix.sources),
                AtomKind::Action => csr
                    .offsets()
                    .get(prefix.sources)
                    .copied()
                    .filter(|_| prefix.sources <= csr.closed_rows()),
            };
            if limit == Some(prefix.cells) && prefix.words.len() == prefix.cells.div_ceil(64) {
                labels
                    .restore_prefix(index, pred.kind, prefix.cells, prefix.words)
                    .map_err(AnalysisError::Product)?;
                start = prefix.sources;
                consumed = start != 0;
            }
        }
        for v in start..states.representative_count() {
            if crate::budget::exhausted() {
                return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
            }
            if pred.kind == AtomKind::State {
                let mut vals = if eager {
                    Some(states.representative(v)?.into_owned())
                } else {
                    None
                };
                let native = if let Some(value) = reuse.get(recipe, v)
                    && states.validate_predicate_inputs(pred, v, None)?
                {
                    Some(value)
                } else {
                    states.predicate(pred, v, None)?
                };
                consumed |= native.is_some();
                fully_native &= native.is_some();
                let value = if let Some(value) = native {
                    value
                } else {
                    states.reference_predicate(pred);
                    let vals = match vals.take() {
                        Some(vals) => vals,
                        None => states.representative(v)?.into_owned(),
                    };
                    atom(model, plan, index, &St::state(vals), &mut context)?
                };
                labels
                    .set_state(index, v, value)
                    .map_err(AnalysisError::Product)?;
                continue;
            }
            if !active.0.contains(&index) {
                fully_native = false;
                continue; // The actual action-domain column is already FALSE.
            }
            // The checked CSR already validates every original ordinal,
            // destination and stutter. A proved false row leaves its
            // original edge bits FALSE, including duplicate destinations.
            if !eager
                && matches!(graph, OriginalEdges::Native(_))
                && graph.len(v).is_some_and(|n| n > 1)
                && states.action_row_false(pred, v)?
            {
                consumed = true;
                continue;
            }
            let targets = selected_originals(graph, v, states, eager)?;
            if graph.len(v) == Some(0) {
                continue;
            }
            let mut vals = if eager {
                Some(states.representative(v)?.into_owned())
            } else {
                None
            };
            let mut evaluated = std::collections::BTreeMap::new();
            for position in 0..graph
                .len(v)
                .ok_or(AnalysisError::Inconsistent("original edge row"))?
            {
                let edge = graph
                    .edge(v, position)
                    .ok_or(AnalysisError::Inconsistent("original edge reference"))?;
                let value = if let Some(target) = targets.get(&edge.to) {
                    if let Some(value) = evaluated.get(&edge.to) {
                        *value
                    } else {
                        let flat = labels
                            .action_position(v, position)
                            .ok_or(AnalysisError::Inconsistent("frozen prefix action position"))?;
                        let native = if let Some(value) = reuse.get(recipe, flat)
                            && states.validate_predicate_inputs(pred, v, Some(target.original))?
                        {
                            Some(value)
                        } else {
                            states.predicate(pred, v, Some(target.original))?
                        };
                        consumed |= native.is_some();
                        fully_native &= native.is_some();
                        let value = if let Some(value) = native {
                            value
                        } else {
                            states.reference_predicate(pred);
                            let before = if let Some(before) = &vals {
                                before.clone()
                            } else {
                                let before = states.representative(v)?.into_owned();
                                vals = Some(before.clone());
                                before
                            };
                            let after = original_values(states, v, target, &before)?;
                            pred.evaluate_detailed(model, &St::action(before, after))
                                .map_err(|failure| {
                                    action_failure(
                                        model,
                                        pred,
                                        failure,
                                        &mut context,
                                        active.1 == Some(index),
                                    )
                                })?
                        };
                        evaluated.insert(edge.to, value);
                        value
                    }
                } else {
                    false
                };
                labels
                    .set_action(index, v, position, value)
                    .map_err(AnalysisError::Product)?;
            }
        }
        if fully_native
            && consumed
            && !eager
            && matches!(graph, OriginalEdges::Native(_))
            && let Some(mut words) = labels.column_words(index, pred.kind == AtomKind::Action)
        {
            states.save_label_prefix(pred, &mut words)?;
        }
        reuse.complete(recipe, index, &labels, fully_native && consumed)?;
    }
    Ok(labels)
}

fn attribute(
    model: &Model,
    plans: &[PreparedProperty],
    states: &[StateVals],
    back: usize,
    cells: usize,
    evaluation: (
        &dive_runtime::budget::EvaluationControl,
        &mut EvaluationErrors,
    ),
) -> Result<Vec<String>, AnalysisError> {
    let (control, evaluations) = evaluation;
    let mut violated = Vec::new();
    violated
        .try_reserve(plans.len())
        .map_err(|_| AnalysisError::Limit("attribution allocation"))?;
    for (p, prepared) in model.property_claims().iter().zip(plans) {
        let plan = &prepared.plan;
        states
            .len()
            .checked_mul(plan.atoms.len())
            .filter(|n| *n <= cells)
            .ok_or(AnalysisError::Limit("attribution atom cells"))?;
        let mut context = StateContext {
            property: &p.name,
            errors: evaluations,
            control,
            attribution: true,
        };
        let mut rows = Vec::new();
        rows.try_reserve_exact(states.len())
            .map_err(|_| AnalysisError::Limit("attribution allocation"))?;
        for (i, state) in states.iter().enumerate() {
            if crate::budget::exhausted() {
                return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
            }
            let next = if i + 1 == states.len() { back } else { i + 1 };
            let st = St::action(state.clone(), states[next].clone());
            let mut row = Vec::new();
            row.try_reserve_exact(plan.atoms.len())
                .map_err(|_| AnalysisError::Limit("attribution allocation"))?;
            for a in 0..plan.atoms.len() {
                row.push(atom(model, plan, a, &st, &mut context)?);
            }
            rows.push(row);
        }
        let truth = plan
            .formula
            .evaluate_word(&rows, &rows, back, cells)
            .map_err(AnalysisError::Tableau)?;
        // Each plan is behavior AND NOT property. The selected candidate
        // must satisfy behavior, so its truth attributes precisely violations.
        if truth[0] && !violated.contains(&p.name) {
            violated.push(p.name.clone());
        }
    }
    Ok(violated)
}

fn validate_analysis_context(model: &Model) -> Result<(), AnalysisError> {
    model.validate_context().map_err(|error| {
        let expr = model
            .property_claims()
            .first()
            .map_or(dive_ir::ExprId(0), |property| property.pred.location);
        // Neither the public IR nor the public predicate location is admitted
        // yet. Do not resolve this location through the evaluator's different
        // IR, or consult a captured scope while reporting the context refusal.
        let source = plan::Source {
            expr,
            origin: dive_ir::Origin::NONE,
            location: format!("unadmitted model context at expression {}", expr.index()),
        };
        AnalysisError::Planning(plan::PlanningError::Unsupported(Box::new(plan::Refusal {
            request: source.clone(),
            cause: source,
            detail: error.to_string(),
        })))
    })
}

/// Runs [`analyze_unbudgeted`] under the run's budget: armed once for capture
/// and analysis alike, and installed on this thread for the analysis
/// stages. Any failure while the budget is exhausted is reported as the stop
/// it is; a stop during capture stays the capture's own result.
#[allow(
    clippy::too_many_arguments,
    reason = "explicit owner policies and separate typed execution evidence"
)]
/// Whether `error` is the limit a stage reports for an exhausted budget.
fn budget_limit(error: &AnalysisError) -> bool {
    use crate::budget::EXHAUSTED;
    match error {
        AnalysisError::Limit(limit)
        | AnalysisError::Product(product::ProductError::Limit(limit))
        | AnalysisError::Tableau(formula::FormulaError::Limit(limit))
        | AnalysisError::Planning(plan::PlanningError::Formula {
            error: formula::FormulaError::Limit(limit),
            ..
        }) => *limit == EXHAUSTED,
        _ => false,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "explicit owner policies and separate typed execution evidence"
)]
struct PreparedProperty {
    plan: Plan,
    groups: Vec<PreparedGroup>,
    streett: Vec<formula::StreettPair>,
    frozen_prefix: Option<plan::FrozenPrefix>,
}

struct PreparedGroup {
    tableau: Tableau,
    alternatives: Vec<CycleConditions>,
}

fn startup_normalization(
    inputs: &[(&crate::formula::Formula, usize)],
    fairness: usize,
    limits: formula::Limits,
) -> Result<Option<crate::tlc_plan::demand::Normalized>, AnalysisError> {
    match crate::tlc_plan::demand::normalize(inputs, fairness, limits) {
        Ok(normalized) => Ok(normalized),
        // Proven Java-int DNF overflow is an intrinsic TLC refusal. It must
        // not fall through to a potentially holding native backend. Its code
        // is not newly promoted here; retain a conservative typed failure.
        Err(error @ formula::FormulaError::Limit("TLC DNF cardinality")) => {
            Err(AnalysisError::Tableau(error))
        }
        // Incomplete optional work/term proof introduces no admission limit
        // and cannot establish either zero solutions or row authority.
        Err(formula::FormulaError::Limit(_)) => Ok(None),
        Err(error) => Err(AnalysisError::Tableau(error)),
    }
}

/// TLC's start-up normalization of the temporal roles: a formula whose
/// normalization has no solutions is TLC's tautology error (2253).
fn startup_normalization_check(
    model: &Model,
    limits: Limits,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<(), AnalysisError> {
    // TLC's startup input is the actual temporal role sequence after binding,
    // not the original configured roots retained for later attribution.
    let claims = model
        .implied_temporals
        .iter()
        .map(|p| p.pred.clone())
        .collect::<Vec<_>>();
    let converted = plan::lower_claims(model, &claims, limits.formula)
        .map_err(|error| planning_failure(control, error))?;
    if crate::budget::exhausted() {
        return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
    }
    let inputs = converted
        .iter()
        .map(|p| (&p.formula, p.atoms.len()))
        .collect::<Vec<_>>();
    let normalized = startup_normalization(&inputs, model.temporals.len(), limits.formula)?;
    if crate::budget::exhausted() {
        return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
    }
    if normalized
        .as_ref()
        .is_some_and(crate::tlc_plan::demand::Normalized::zero_solutions)
    {
        return Err(AnalysisError::StartupTautology {
            output: model.output.clone(),
        });
    }
    Ok(())
}

/// Whether TLC's start-up normalization of `model`'s temporal properties
/// has no solutions, its tautology error (2253,
/// `TLC_LIVE_FORMULA_TAUTOLOGY`). TLC decides this before it evaluates the
/// ASSUMEs, so a caller that checks ASSUMEs first asks here beforehand.
///
/// Planning is effect-free: it refuses effects before they can execute.
/// Any other planning outcome, failures included, is `false` and is left to
/// the ordinary analysis, in its own order.
///
/// ```
/// # fn ask(model: &dive_model::Model) {
/// let options = dive_runtime::CheckOptions::tlc(0, 0);
/// let limits = dive_liveness::analyze::Limits::default();
/// if dive_liveness::analyze::startup_tautology(model, &options, limits) {
///     // Report TLC's start-up error, or refuse it, before any ASSUME.
/// }
/// # }
/// ```
#[must_use]
pub fn startup_tautology(model: &Model, opts: &CheckOptions, limits: Limits) -> bool {
    if model.implied_temporals.is_empty() || validate_analysis_context(model).is_err() {
        return false;
    }
    let Ok(control) = evaluator_control(opts) else {
        return false;
    };
    let Ok(_entered) = control.enter(&model.evaluator) else {
        return false;
    };
    matches!(
        startup_normalization_check(model, limits, &control),
        Err(AnalysisError::StartupTautology { .. })
    )
}

fn prepare(
    model: &Model,
    limits: Limits,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<Vec<PreparedProperty>, AnalysisError> {
    if model.implied_temporals.is_empty() {
        return Err(AnalysisError::Unsupported("no temporal properties"));
    }
    if !matches!(&model.postconditions, Ok(p) if p.is_empty()) {
        return Err(AnalysisError::Unsupported(
            "postconditions with temporal checking",
        ));
    }
    if model.property_claims().len() > limits.properties {
        return Err(AnalysisError::Limit("properties"));
    }
    startup_normalization_check(model, limits, control)?;
    // These are different semantic inputs: original configured claims carry
    // initial/safety roles needed for attribution. No scope-cycle identity or
    // expression equality permits replacing them with the temporal-role plans.
    // The row consumer retains its original complete MUST/producer authority.
    let (original_plans, frozen_prefix) = plan::lower_original_claims(model, limits.formula)
        .map_err(|error| planning_failure(control, error))?;
    let plans = original_plans
        .into_iter()
        .map(|plan| {
            let formula::FactoredSolutions {
                groups,
                pairs: streett,
            } = match formula::decompose(&plan.formula, limits.formula) {
                Ok(groups) => formula::FactoredSolutions {
                    groups,
                    pairs: Vec::new(),
                },
                Err(formula::FormulaError::Limit("solution decomposition work")) => {
                    formula::streett(&plan.formula, limits.formula)
                        .map_err(AnalysisError::Tableau)?
                }
                Err(e) => return Err(AnalysisError::Tableau(e)),
            };
            let groups = groups
                .into_iter()
                .map(|group| {
                    let tableau = formula::compile_residual(&group.core, limits.formula)
                        .map_err(AnalysisError::Tableau)?;
                    Ok(PreparedGroup {
                        tableau,
                        alternatives: group.alternatives,
                    })
                })
                .collect::<Result<_, AnalysisError>>()?;
            Ok(PreparedProperty {
                plan,
                groups,
                streett,
                frozen_prefix,
            })
        })
        .collect::<Result<_, AnalysisError>>()?;
    Ok(plans)
}

type CandidateWord = finish::NativeWord;

type OriginalWord = (Vec<StateVals>, Vec<usize>, Vec<capture::Edge>);

fn original_word<'a>(
    captured: impl Into<OriginalEdges<'a>>,
    source: &mut (impl StateSource + ?Sized),
    lasso: &crate::search::Lasso,
) -> Result<OriginalWord, AnalysisError> {
    let captured = captured.into();
    let count = lasso
        .prefix
        .len()
        .checked_add(lasso.cycle.len())
        .ok_or(AnalysisError::Limit("word layout"))?;
    let mut states = Vec::new();
    let mut source_ids = Vec::new();
    let mut edges = Vec::new();
    states
        .try_reserve_exact(count)
        .map_err(|_| AnalysisError::Limit("word allocation"))?;
    source_ids
        .try_reserve_exact(count)
        .map_err(|_| AnalysisError::Limit("word allocation"))?;
    edges
        .try_reserve_exact(count)
        .map_err(|_| AnalysisError::Limit("word allocation"))?;
    for step in lasso.prefix.iter().chain(&lasso.cycle) {
        if crate::budget::exhausted() {
            return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
        }
        let id = step.from;
        states.push(source.representative(id)?.into_owned());
        source_ids.push(id);
    }
    for step in lasso.prefix.iter().chain(&lasso.cycle) {
        if crate::budget::exhausted() {
            return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
        }
        let (source, edge) = (step.from, step.edge);
        let edge = captured
            .edge(source, edge)
            .ok_or(AnalysisError::Inconsistent("original word edge"))?;
        let label = edge
            .label
            .map(|label| {
                let mut copy = String::new();
                copy.try_reserve_exact(label.len())
                    .map_err(|_| AnalysisError::Limit("word label allocation"))?;
                copy.push_str(label);
                Ok::<_, AnalysisError>(copy)
            })
            .transpose()?;
        edges.push(capture::Edge {
            to: edge.to,
            ordinal: edge.ordinal,
            label,
        });
    }
    Ok((states, source_ids, edges))
}

/// Searches the first `searched` properties in configuration order for an
/// accepting word; attribution always covers every property. A complete graph
/// searches all of them. The default prefix searches only the first, because a word for a
/// later property on a prefix does not show which property the complete
/// graph's search would name first.
#[allow(
    clippy::too_many_arguments,
    reason = "the search width is the only difference between prefix and final checks"
)]
fn analyze_captured(
    model: &Model,
    plans: &[PreparedProperty],
    searched: usize,
    captured: &capture::BehaviorGraph,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    evaluations: &mut EvaluationErrors,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<Option<CandidateWord>, AnalysisError> {
    analyze_source(
        model,
        plans,
        searched,
        captured,
        &mut CapturedStates(captured),
        limits,
        policy,
        graph_labels,
        stage,
        evaluations,
        control,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the private source boundary preserves existing analysis inputs and ownership"
)]
fn analyze_source(
    model: &Model,
    plans: &[PreparedProperty],
    searched: usize,
    captured: &capture::BehaviorGraph,
    states: &mut (impl StateSource + ?Sized),
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    evaluations: &mut EvaluationErrors,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<Option<CandidateWord>, AnalysisError> {
    *stage = Stage::Product;
    let graph = product::BehaviorView::checked_captured(captured)
        .map_err(|e| AnalysisError::Product(product::ProductError::Shape(e.to_string())))?;
    analyze_graph(
        model,
        plans,
        searched,
        OriginalEdges::Behavior(captured),
        states,
        graph,
        limits,
        policy,
        graph_labels,
        stage,
        evaluations,
        control,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared existing product loop over separate topology, metadata and state owners"
)]
fn analyze_graph<S: StateSource + ?Sized>(
    model: &Model,
    plans: &[PreparedProperty],
    searched: usize,
    captured: OriginalEdges<'_>,
    states: &mut S,
    graph: product::BehaviorView<'_>,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    evaluations: &mut EvaluationErrors,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<Option<CandidateWord>, AnalysisError> {
    let mut reuse = fairness::Cache::new(
        searched.min(plans.len()) > 1
            && plans
                .first()
                .is_some_and(|plan| plan.frozen_prefix.is_some()),
        crate::budget::optional_buffer_limit(),
    );
    let mut segment = None;
    let mut enter = |next: Stage, source: &mut S| {
        let now = std::time::Instant::now();
        if let Some((previous, start)) = segment.replace((next, now)) {
            source.analysis_time(previous, now.duration_since(start));
        }
        *stage = next;
    };
    let result = (|| {
        for (p, prepared) in model.property_claims().iter().zip(plans).take(searched) {
            let plan = &prepared.plan;
            enter(Stage::Atoms, states);
            let active = if plan.atoms.iter().any(|atom| atom.kind == AtomKind::Action) {
                crate::tlc_plan::active_actions(&plan.formula, limits.formula)
                    .map_err(AnalysisError::Tableau)?
            } else {
                (std::collections::BTreeSet::new(), None)
            };
            let labels = labels_with_reuse(
                model,
                plan,
                captured,
                states,
                graph,
                &p.name,
                graph_labels,
                &active,
                evaluations,
                control,
                prepared.frozen_prefix,
                &mut reuse,
            )?;
            enter(Stage::Product, states);
            let words = product::StreettWords::new(
                graph,
                &labels,
                &prepared.streett,
                prepared.groups.iter().flat_map(|g| &g.alternatives),
            )
            .map_err(AnalysisError::Product)?;
            for group in &prepared.groups {
                let product =
                    ImplicitProduct::new(graph, &group.tableau, &labels, limits.product, policy)
                        .map_err(AnalysisError::Product)?;
                if let Some(lasso) = product
                    .find_lasso_with_streett(
                        &labels,
                        &group.alternatives,
                        &prepared.streett,
                        words.as_ref(),
                    )
                    .map_err(AnalysisError::Product)?
                {
                    let (word_states, _, edges) = original_word(captured, states, &lasso)?;
                    enter(Stage::Attribution, states);
                    let properties = attribute(
                        model,
                        plans,
                        &word_states,
                        lasso.prefix.len(),
                        limits.atom_cells,
                        (control, evaluations),
                    )?;
                    if !properties.contains(&p.name) {
                        return Err(AnalysisError::Inconsistent(
                            "accepting candidate failed target-property attribution",
                        ));
                    }
                    return Ok(Some(CandidateWord {
                        target_property: p.name.clone(),
                        properties,
                        states: word_states,
                        back_to: lasso.prefix.len(),
                        edges,
                    }));
                }
            }
        }
        Ok(None)
    })();
    if let Some((last, start)) = segment {
        states.analysis_time(last, start.elapsed());
    }
    states.predicate_reuse_storage(
        reuse.peak_bytes,
        reuse.cap_bypasses,
        reuse.allocation_bypasses,
        reuse.stored_columns,
    );
    // Columns are local to this exact immutable graph/batch, and are released
    // before any caller can start fresh independent candidate replay.
    drop(reuse);
    result
}

#[allow(
    clippy::too_many_arguments,
    reason = "native adapter preserves the same analysis inputs without a retained Value graph"
)]
fn analyze_native(
    model: &Model,
    plans: &[PreparedProperty],
    searched: usize,
    captured: &capture::CsrCapture<'_>,
    read: &mut dyn periodic::external::NativeReader,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    evaluations: &mut EvaluationErrors,
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<Option<CandidateWord>, AnalysisError> {
    *stage = Stage::Product;
    let topology = product::CsrTopology::from_capture(captured);
    let mut states = state_source::NativeStates {
        graph: captured,
        read,
        control,
        variables: model.ir.vars().len(),
    };
    analyze_graph(
        model,
        plans,
        searched,
        OriginalEdges::Native(captured),
        &mut states,
        product::BehaviorView::from(&topology),
        limits,
        policy,
        graph_labels,
        stage,
        evaluations,
        control,
    )
}

/// Completed-level deterministic scheduling without weakening final completion.
pub mod periodic;

#[cfg(test)]
mod label_shapes;

#[cfg(test)]
mod borrowed_topology;

#[cfg(test)]
mod state_errors;

#[cfg(test)]
#[path = "analyze/startup_tests.rs"]
mod startup_tests;
