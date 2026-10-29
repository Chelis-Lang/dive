//! Pure liveness analysis through the shared safety engine.
//!
//! This library coordinator does not implement TLC's exact planning/selection
//! or symmetry. [`periodic`] adds deterministic completed-level checks. WF/SF expansion uses TLC-side enabledness. Counterexample candidates
//! still require independent validation; direct formula evaluation attributes
//! their complete configured property set.
//! Prefix candidates require independent validation and never prove `Holds`.

use dive_check::{CheckOptions, CheckResult, capture};
use dive_eval::{St, next::StateVals};
use dive_model::Model;

use crate::formula::{self, AtomKind, CycleConditions, Tableau};
use crate::plan::{self, Plan, PlanningError};
use crate::product::{self, AcceptancePolicy, CompactProduct, PackedTruth};

mod finish;
#[doc(hidden)]
pub mod owned;
mod row_demand;
mod state_source;

use state_source::{CapturedStates, StateSource};

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
    /// Shared legacy graph-label, attribution-row and word-truth cell ceiling.
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
    BudgetMeasurement(dive_check::budget::MeasurementError),
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
    /// The run's budget ran out after capture (ADR 0107). Never a verdict;
    /// a stop during capture is an [`Self::Exploration`] with
    /// [`dive_check::Verdict::Stopped`].
    Stopped(Box<dive_check::budget::Stopped>),
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
                    dive_check::Verdict::BudgetMeasurement { .. }
                ) =>
            {
                dive_check::budget::MEASUREMENT_FAILURE_CODE
            }
            Self::Exploration(_) => "DIVE-L0032",
            Self::Atom { .. } => "DIVE-L0033",
            Self::Limit(_) => "DIVE-L0034",
            Self::Unsupported(_) | Self::UnsupportedValue(_) => "DIVE-L0035",
            Self::Inconsistent(_) => "DIVE-L0036",
            Self::CaptureThread(_) => "DIVE-L0037",
            Self::BudgetStartup(_) => dive_check::budget::START_FAILURE_CODE,
            Self::BudgetMeasurement(_) => dive_check::budget::MEASUREMENT_FAILURE_CODE,
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
    pub stats: dive_check::Stats,
    /// Requested and actual capture workers.
    pub workers: dive_check::Workers,
    /// Collision qualification, absent for exact storage.
    pub collision: Option<dive_check::Collision>,
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
/// are retained separately. Legacy posthoc evidence has no demand authority.
/// A certified row additionally retains its finite producer diagnostic path,
/// never the scope, evaluator or complete behavior graph.
///
/// ```no_run
/// # fn example(model: &dive_model::Model) {
/// use dive_liveness::analyze::{analyze_final_with_action_errors, Limits};
/// if let Err(failure) = analyze_final_with_action_errors(model,
///     &dive_check::CheckOptions::tlc(0, 0), Limits::default()) {
///     if let Some(state) = failure.state_evaluation() {
///         let _actual_site = state.site();
///         let _original_cause = state.core_cause();
///         let _outer_source = state.source();
///     }
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
    control: &'a dive_check::budget::EvaluationControl,
    attribution: bool,
}

/// A failure retaining legacy errors and original typed evaluation causes.
/// Legacy entrypoints keep their existing error shapes; no diagnostic is parsed.
#[derive(Clone, Debug)]
pub struct DetailedFailure {
    failure: Failure,
    evaluations: EvaluationErrors,
}
impl DetailedFailure {
    /// Legacy failure and completed execution facts.
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
    /// Takes the state evidence before consuming the legacy two-part result.
    /// This transfers only the failure description; no runtime owner is retained.
    ///
    /// ```
    /// # fn example(mut failure: dive_liveness::analyze::DetailedFailure) {
    /// let state = failure.take_state_evaluation();
    /// let (legacy, action) = failure.into_parts();
    /// # let _ = (state, legacy, action);
    /// # }
    /// ```
    #[must_use]
    pub fn take_state_evaluation(&mut self) -> Option<Box<StateEvaluation>> {
        self.evaluations.state.take()
    }
    /// Consumes this description without changing the legacy public error shape.
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
) -> Result<std::sync::Arc<dive_check::budget::EvaluationControl>, AnalysisError> {
    dive_check::budget::EvaluationControl::new(&opts.budget, dive_check::budget::Phase::Temporal)
        .map_err(|_| AnalysisError::Inconsistent("evaluator control identity is unrepresentable"))
}

fn evaluator_interruption(
    control: &dive_check::budget::EvaluationControl,
    error: &dive_eval::EvalError,
) -> Option<AnalysisError> {
    control.verdict(error).map(|verdict| match verdict {
        dive_check::Verdict::Stopped(stopped) => AnalysisError::Stopped(stopped),
        dive_check::Verdict::BudgetMeasurement { error } => AnalysisError::BudgetMeasurement(error),
        _ => AnalysisError::Inconsistent("unexpected evaluator interruption classification"),
    })
}

fn planning_failure(
    control: &dive_check::budget::EvaluationControl,
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
        }));
    }
    error
}

/// Original edge metadata without owning or reconstructing state rows.
#[derive(Clone, Copy)]
enum OriginalEdges<'a> {
    Legacy(&'a capture::BehaviorGraph),
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
        Self::Legacy(graph)
    }
}

impl<'a> OriginalEdges<'a> {
    fn states(self) -> usize {
        match self {
            Self::Legacy(graph) => graph.edges.len(),
            Self::Native(graph) => graph.state_count(),
        }
    }
    fn len(self, source: usize) -> Option<usize> {
        match self {
            Self::Legacy(graph) => graph.edges.get(source).map(Vec::len),
            Self::Native(graph) => graph.outgoing_range(source).map(|range| range.len()),
        }
    }
    fn edge(self, source: usize, position: usize) -> Option<OriginalEdge<'a>> {
        match self {
            Self::Legacy(graph) => {
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
                Some(OriginalEdge {
                    to: graph.target(at)?,
                    ordinal: graph.ordinal(at)?,
                    label: graph.label(at)?,
                })
            }
        }
    }
}

fn selected_targets<'a>(
    graph: impl Into<OriginalEdges<'a>>,
    source: usize,
    states: &mut (impl StateSource + ?Sized),
) -> Result<std::collections::BTreeMap<usize, StateVals>, AnalysisError> {
    let graph = graph.into();
    let malformed = || AnalysisError::Inconsistent("original action target row shape");
    let originals = states.concrete_count(source).ok_or_else(malformed)?;
    let mut original = 0;
    let mut selected = std::collections::BTreeMap::new();
    for position in 0..graph.len(source).ok_or_else(malformed)? {
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
                entry.insert(states.concrete(source, original)?);
            }
            original += 1;
        } else {
            if edge.to != source {
                return Err(malformed());
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = selected.entry(source) {
                entry.insert(states.representative(source)?.into_owned());
            }
        }
    }
    if original != originals {
        return Err(malformed());
    }
    Ok(selected)
}

#[allow(
    clippy::too_many_arguments,
    reason = "concrete values and validated original-edge shape have separate owners"
)]
fn labels<'a>(
    model: &Model,
    plan: &Plan,
    graph: impl Into<OriginalEdges<'a>>,
    states: &mut (impl StateSource + ?Sized),
    shape: product::BehaviorView<'_>,
    property: &str,
    policy: GraphLabelPolicy,
    active: &(std::collections::BTreeSet<usize>, Option<usize>),
    evaluations: &mut EvaluationErrors,
    control: &dive_check::budget::EvaluationControl,
) -> Result<(PackedTruth, usize), AnalysisError> {
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
    let kinds = plan.atoms.iter().map(|atom| atom.kind).collect::<Vec<_>>();
    let mut labels = PackedTruth::new(shape, &kinds).map_err(AnalysisError::Product)?;
    let mut context = StateContext {
        property,
        errors: evaluations,
        control,
        attribution: false,
    };
    for (index, pred) in plan.atoms.iter().enumerate() {
        for v in 0..states.representative_count() {
            if crate::budget::exhausted() {
                return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
            }
            if pred.kind == AtomKind::State {
                let vals = states.representative(v)?.into_owned();
                let value = atom(model, plan, index, &St::state(vals), &mut context)?;
                labels
                    .set_state(index, v, value)
                    .map_err(AnalysisError::Product)?;
                continue;
            }
            if !active.0.contains(&index) {
                continue; // The actual action-domain column is already FALSE.
            }
            let targets = selected_targets(graph, v, states)?;
            if graph.len(v) == Some(0) {
                continue;
            }
            let vals = states.representative(v)?;
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
                        let value = pred
                            .evaluate_detailed(
                                model,
                                &St::action(vals.as_ref().clone(), target.clone()),
                            )
                            .map_err(|failure| {
                                let core = matches!(&failure, plan::AtomError::Core(_));
                                let cause = failure.into_error();
                                if let Some(interruption) = evaluator_interruption(control, &cause)
                                {
                                    return interruption;
                                }
                                let source = Box::new(plan::Source::new(model, pred.pred.location));
                                let error = AnalysisError::Atom {
                                    property: property.into(),
                                    source: source.clone(),
                                    detail: cause.to_string(),
                                };
                                context.errors.action = Some(Box::new(ActionEvaluation {
                                    cause,
                                    property: property.into(),
                                    source,
                                    properties: model.property_claims().len(),
                                    core,
                                    direct: active.1 == Some(index),
                                }));
                                error
                            })?;
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
    }
    Ok((labels, edges))
}

fn cached_labels(
    property: usize,
    plan: &Plan,
    original: &capture::BehaviorGraph,
    projection: &row_demand::Projection,
    cache: &row_demand::Cache,
    policy: GraphLabelPolicy,
) -> Result<(PackedTruth, usize), AnalysisError> {
    let original_edges = original
        .edges
        .iter()
        .try_fold(0_usize, |n, row| n.checked_add(row.len()))
        .ok_or(AnalysisError::Limit("atom cells"))?;
    // Keep explicit caller cell ceilings on their original captured domain.
    // Projection changes queried topology, not the configured admission policy.
    policy.admit(original.states.len(), original_edges, plan.atoms.len())?;
    let kinds = plan.atoms.iter().map(|atom| atom.kind).collect::<Vec<_>>();
    let mut truth = PackedTruth::new(&projection.graph, &kinds).map_err(AnalysisError::Product)?;
    let mut edges = 0_usize;
    for (vertex, &source) in projection.sources.iter().enumerate() {
        if crate::budget::exhausted() {
            return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
        }
        let row = &projection.edges[vertex];
        edges = edges
            .checked_add(row.len())
            .ok_or(AnalysisError::Limit("atom cells"))?;
        for (atom, kind) in kinds.iter().enumerate() {
            let value = cache.read(property, atom, source)?;
            if *kind == AtomKind::State {
                truth
                    .set_state(atom, vertex, value)
                    .map_err(AnalysisError::Product)?;
            } else {
                for position in 0..row.len() {
                    truth
                        .set_action(atom, vertex, position, value)
                        .map_err(AnalysisError::Product)?;
                }
            }
        }
    }
    Ok((truth, edges))
}

fn attribute(
    model: &Model,
    plans: &[PreparedProperty],
    states: &[StateVals],
    back: usize,
    cells: usize,
    evaluation: (
        &dive_check::budget::EvaluationControl,
        &mut EvaluationErrors,
    ),
    cached: Option<(&row_demand::Cache, &[usize])>,
) -> Result<Vec<String>, AnalysisError> {
    let (control, evaluations) = evaluation;
    let mut violated = Vec::new();
    for (property, (p, prepared)) in model.property_claims().iter().zip(plans).enumerate() {
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
        for (i, state) in states.iter().enumerate() {
            let next = if i + 1 == states.len() { back } else { i + 1 };
            let st = St::action(state.clone(), states[next].clone());
            let row = (0..plan.atoms.len())
                .map(|a| {
                    if let Some((cache, sources)) = cached {
                        let source = *sources
                            .get(i)
                            .ok_or(AnalysisError::Inconsistent("attribution source map"))?;
                        cache.read(property, a, source)
                    } else {
                        atom(model, plan, a, &st, &mut context)
                    }
                })
                .collect::<Result<_, _>>()?;
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

/// Checks the supported pure temporal properties after complete exploration.
/// All behavior conjuncts and properties are planned before any search; one
/// unsupported conjunct aborts the analysis. The graph is captured by the
/// shared safety engine, then each behavior AND negated-property tableau is
/// searched in property order. This selects a deterministic short candidate,
/// not TLC's exact lasso. No periodic or CLI verdict is enabled by this API.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::CheckOptions;
/// use dive_liveness::analyze::{analyze_final, Analysis, Limits};
/// let source = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nP == []<>x\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
/// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n").into_config().expect("config");
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// assert!(matches!(analyze_final(&model, &CheckOptions::tlc(0, 0), Limits::default())?, Analysis::Holds(_)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Planning, graph capture, atom evaluation, explicit resource bounds or
/// product search failed. Postconditions are refused until their temporal
/// failure priority is integrated.
pub fn analyze_final(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
) -> Result<Analysis, AnalysisError> {
    analyze_final_with_evidence(model, opts, limits).map_err(|failure| failure.error)
}

/// Like [`analyze_final`], retaining stage and completed capture evidence on
/// failure. The graph itself is not copied into a failure report.
///
/// # Errors
/// The same failures as [`analyze_final`], with execution evidence.
pub fn analyze_final_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
) -> Result<Analysis, Failure> {
    analyze_final_with_action_errors(model, opts, limits).map_err(|failure| failure.into_parts().0)
}

/// Like [`analyze_final_with_evidence`], retaining actual selected action errors.
/// The legacy public error constructors remain unchanged. State and attribution
/// failures do not acquire an action cause. A typed cause alone does not establish
/// TLC error-order parity, especially across multiple configured properties.
///
/// ```no_run
/// # fn example(model: &dive_model::Model) {
/// use dive_liveness::analyze::{analyze_final_with_action_errors, Limits};
/// if let Err(failure) = analyze_final_with_action_errors(model, &dive_check::CheckOptions::tlc(0, 0), Limits::default()) {
///     if let Some(action) = failure.action_evaluation() {
///         let _original_class = action.cause().class();
///     }
/// }
/// # }
/// ```
///
/// # Errors
/// The same failures as [`analyze_final_with_evidence`], with their original
/// typed action or state cause when that evaluation actually failed. State
/// evidence retains its actual analysis site, without broadening verdict policy.
pub fn analyze_final_with_action_errors(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
) -> Result<Analysis, DetailedFailure> {
    analyze_final_with_policy(model, opts, limits, AcceptancePolicy::Legacy)
}

/// Like [`analyze_final_with_action_errors`], with an explicit compact
/// acceptance admission policy. All other supplied limit fields keep their
/// units. [`AcceptancePolicy::Buffers`] replaces only `acceptance_cells` and
/// adds an independent acceptance materialization work bound.
///
/// # Errors
/// The same failures as [`analyze_final_with_action_errors`], plus named covered
/// buffer-byte, allocation and materialization-work refusals in buffer mode.
pub fn analyze_final_with_policy(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
) -> Result<Analysis, DetailedFailure> {
    analyze_final_with_policies(
        model,
        opts,
        limits,
        policy,
        GraphLabelPolicy::Cells(limits.atom_cells),
    )
}

/// Like [`analyze_final_with_policy`], explicitly choosing the graph-label
/// policy independently of acceptance. Every existing entrypoint retains its
/// supplied `Limits::atom_cells` ceiling. No other limit changes its scope.
///
/// ```no_run
/// # fn example(model: &dive_model::Model) {
/// use dive_liveness::{analyze::{self, GraphLabelPolicy, Limits}, product::AcceptancePolicy};
/// let result = analyze::analyze_final_with_policies(model,
///     &dive_check::CheckOptions::tlc(0, 0), Limits::default(),
///     AcceptancePolicy::compact_default(), GraphLabelPolicy::NoCellLimit);
/// // A candidate still requires independent trace validation.
/// # let _ = result;
/// # }
/// ```
///
/// # Errors
/// The same typed failures as [`analyze_final_with_policy`].
pub fn analyze_final_with_policies(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
) -> Result<Analysis, DetailedFailure> {
    analyze_final_bound(model, opts, limits, policy, graph_labels, None)
}

fn analyze_final_bound(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    authority: Option<&owned::Authority<'_>>,
) -> Result<Analysis, DetailedFailure> {
    let mut stage = Stage::Planning;
    let mut exploration = None;
    let mut evaluations = EvaluationErrors::default();
    analyze_observed(
        model,
        opts,
        limits,
        policy,
        graph_labels,
        &mut stage,
        &mut exploration,
        &mut evaluations,
        authority,
    )
    .map_err(|error| DetailedFailure {
        failure: Failure {
            error,
            stage,
            exploration,
        },
        evaluations,
    })
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
/// it is (ADR 0107); a stop during capture stays the capture's own result.
#[allow(
    clippy::too_many_arguments,
    reason = "explicit owner policies and separate typed execution evidence"
)]
fn analyze_observed(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    exploration: &mut Option<Box<ExplorationEvidence>>,
    evaluations: &mut EvaluationErrors,
    authority: Option<&owned::Authority<'_>>,
) -> Result<Analysis, AnalysisError> {
    let (mut watchdog, armed) = dive_check::budget::arm(opts).map_err(|error| match error {
        dive_check::budget::StartError::Measurement(error) => {
            AnalysisError::BudgetMeasurement(error)
        }
        error @ dive_check::budget::StartError::Thread(_) => {
            AnalysisError::BudgetStartup(error.to_string())
        }
    })?;
    let _scope = crate::budget::enter(&armed.budget);
    let result = analyze_unbudgeted(
        model,
        &armed,
        limits,
        policy,
        graph_labels,
        stage,
        exploration,
        evaluations,
        authority,
    )
    .map_err(|error| match armed.budget.poll() {
        // Only the budget's own limit is a stop; every other failure,
        // including one raised just as the budget ran out, keeps its class.
        Ok(Some(exhausted)) if budget_limit(&error) => {
            *evaluations = EvaluationErrors::default();
            AnalysisError::Stopped(Box::new(
                exhausted.during(dive_check::budget::Phase::Temporal),
            ))
        }
        Err(measurement) if budget_limit(&error) => AnalysisError::BudgetMeasurement(measurement),
        _ => error,
    });
    if let Some(watchdog) = &mut watchdog {
        let finished = watchdog.finish();
        if let Ok(analysis) = &result {
            let error = match finished {
                Err(error) => Some(AnalysisError::BudgetMeasurement(error)),
                Ok(Some(exhausted)) => Some(AnalysisError::Stopped(Box::new(
                    exhausted.during(dive_check::budget::Phase::Finalization),
                ))),
                Ok(None) => None,
            };
            if let Some(error) = error {
                *stage = Stage::Finalization;
                let capture = match analysis {
                    Analysis::Holds(capture) => capture,
                    Analysis::Candidate(candidate) => &candidate.exploration,
                };
                *exploration = Some(Box::new(capture.into()));
                return Err(error);
            }
        }
    }
    result
}

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
fn analyze_unbudgeted(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    exploration: &mut Option<Box<ExplorationEvidence>>,
    evaluations: &mut EvaluationErrors,
    authority: Option<&owned::Authority<'_>>,
) -> Result<Analysis, AnalysisError> {
    validate_analysis_context(model)?;
    let control = evaluator_control(opts)?;
    let plans = {
        let _control = control
            .enter(&model.evaluator)
            .map_err(|_| AnalysisError::Inconsistent("another driver owns evaluator control"))?;
        prepare(model, limits, &control)?
    };
    let demand = row_demand::prepare(
        model,
        &plans,
        opts,
        model.temporals.len(),
        limits.formula,
        graph_labels,
        (authority, &control),
    )?;
    let cache = demand
        .as_ref()
        .map(|program| std::sync::Mutex::new(row_demand::Cache::new(program)));
    let native_hooks = owned::native_hooks(authority, model);
    *stage = Stage::Capture;
    let captured = if let (Some(program), Some(cache)) = (&demand, &cache) {
        match row_demand::Observe::new(model, &plans, program, cache).explore(
            opts,
            limits.capture,
            None,
            evaluations,
            exploration,
            native_hooks,
        )? {
            capture::ControlledExploration::Complete(complete) => *complete,
            capture::ControlledExploration::Stopped(result) => {
                return Err(AnalysisError::Exploration(result));
            }
            capture::ControlledExploration::Prefix(_) => {
                return Err(AnalysisError::Inconsistent(
                    "final row capture returned a prefix",
                ));
            }
        }
    } else {
        match capture::explore_with_native_hooks(model, opts, limits.capture, native_hooks)
            .map_err(|message| {
                *stage = Stage::Unknown;
                AnalysisError::CaptureThread(message)
            })? {
            capture::Exploration::Complete(c) => *c,
            capture::Exploration::Stopped(v) => return Err(AnalysisError::Exploration(v)),
        }
    };
    let _control = control
        .enter(&model.evaluator)
        .map_err(|_| AnalysisError::Inconsistent("another driver owns evaluator control"))?;
    let word = analyze_captured(
        model,
        &plans,
        usize::MAX,
        &captured.graph,
        limits,
        policy,
        graph_labels,
        stage,
        evaluations,
        &control,
        cache
            .as_ref()
            .map(|cache| {
                cache
                    .lock()
                    .map_err(|_| AnalysisError::Inconsistent("temporal cache lock poisoned"))
            })
            .transpose()?
            .filter(|cache| cache.active())
            .as_deref(),
    )
    .inspect_err(|_| {
        *exploration = Some(Box::new(ExplorationEvidence::from(&captured)));
    });
    finish::complete(word, captured.evidence().clone()).map(|result| result.reference(captured))
}

struct PreparedProperty {
    plan: Plan,
    groups: Vec<PreparedGroup>,
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

fn prepare(
    model: &Model,
    limits: Limits,
    control: &dive_check::budget::EvaluationControl,
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
    {
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
    }
    // These are different semantic inputs: original configured claims carry
    // initial/safety roles needed for attribution. No scope-cycle identity or
    // expression equality permits replacing them with the temporal-role plans.
    // The row consumer retains its original complete MUST/producer authority.
    let plans = model
        .property_claims()
        .iter()
        .map(|p| {
            let plan = plan::lower_violation(model, &p.pred, limits.formula)
                .map_err(|error| planning_failure(control, error))?;
            let groups = formula::decompose(&plan.formula, limits.formula)
                .map_err(AnalysisError::Tableau)?
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
            Ok(PreparedProperty { plan, groups })
        })
        .collect::<Result<_, AnalysisError>>()?;
    Ok(plans)
}

type CandidateWord = finish::NativeWord;

type OriginalWord = (Vec<StateVals>, Vec<usize>, Vec<capture::Edge>);

fn original_word<'a>(
    captured: impl Into<OriginalEdges<'a>>,
    source: &mut (impl StateSource + ?Sized),
    projection: Option<&row_demand::Projection>,
    lasso: &crate::search::Lasso,
) -> Result<OriginalWord, AnalysisError> {
    let captured = captured.into();
    let steps = lasso.prefix.iter().chain(&lasso.cycle);
    let states = steps
        .clone()
        .map(|s| {
            source
                .representative(projection.map_or(s.from, |p| p.sources[s.from]))
                .map(std::borrow::Cow::into_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_ids = steps
        .clone()
        .map(|s| projection.map_or(s.from, |p| p.sources[s.from]))
        .collect::<Vec<_>>();
    let edges = steps
        .map(|s| {
            let (source, edge) = projection.map_or((s.from, s.edge), |p| {
                (p.sources[s.from], p.edges[s.from][s.edge])
            });
            let edge = captured
                .edge(source, edge)
                .ok_or(AnalysisError::Inconsistent("original word edge"))?;
            Ok(capture::Edge {
                to: edge.to,
                ordinal: edge.ordinal,
                label: edge.label.map(str::to_owned),
            })
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;
    Ok((states, source_ids, edges))
}

/// Searches the first `searched` properties in configuration order for an
/// accepting word; attribution always covers every property. A complete graph
/// searches all of them. A prefix searches only the first, because a word for a
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
    control: &dive_check::budget::EvaluationControl,
    cache: Option<&row_demand::Cache>,
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
        cache,
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
    control: &dive_check::budget::EvaluationControl,
    cache: Option<&row_demand::Cache>,
) -> Result<Option<CandidateWord>, AnalysisError> {
    *stage = Stage::Product;
    let cache = cache.filter(|cache| cache.active());
    let original = product::BehaviorView::checked_captured(captured)
        .map_err(|e| AnalysisError::Product(product::ProductError::Shape(e.to_string())))?;
    let projection = cache
        .map(|cache| row_demand::Projection::new(captured, cache))
        .transpose()?;
    let graph = projection.as_ref().map_or(original, |projection| {
        product::BehaviorView::from(&projection.graph)
    });
    analyze_graph(
        model,
        plans,
        searched,
        OriginalEdges::Legacy(captured),
        states,
        graph,
        projection.as_ref(),
        cache,
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
fn analyze_graph(
    model: &Model,
    plans: &[PreparedProperty],
    searched: usize,
    captured: OriginalEdges<'_>,
    states: &mut (impl StateSource + ?Sized),
    graph: product::BehaviorView<'_>,
    projection: Option<&row_demand::Projection>,
    cache: Option<&row_demand::Cache>,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    evaluations: &mut EvaluationErrors,
    control: &dive_check::budget::EvaluationControl,
) -> Result<Option<CandidateWord>, AnalysisError> {
    for (property, (p, prepared)) in model
        .property_claims()
        .iter()
        .zip(plans)
        .take(searched)
        .enumerate()
    {
        let plan = &prepared.plan;
        *stage = Stage::Atoms;
        let active = if plan.atoms.iter().any(|atom| atom.kind == AtomKind::Action) {
            crate::tlc_plan::active_actions(&plan.formula, limits.formula)
                .map_err(AnalysisError::Tableau)?
        } else {
            (std::collections::BTreeSet::new(), None)
        };
        let (labels, behavior_edges) =
            if let (Some(cache), Some(projection), OriginalEdges::Legacy(captured)) =
                (cache, projection, captured)
            {
                cached_labels(property, plan, captured, projection, cache, graph_labels)?
            } else {
                labels(
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
                )?
            };
        *stage = Stage::Product;
        for group in &prepared.groups {
            let product = CompactProduct::with_policy_and_edges(
                graph,
                &group.tableau,
                &labels,
                &[],
                limits.product,
                policy,
                behavior_edges,
            )
            .map_err(AnalysisError::Product)?;
            for conditions in &group.alternatives {
                if let Some(lasso) = product
                    .find_lasso_with_conditions(&labels, conditions)
                    .map_err(AnalysisError::Product)?
                {
                    let (states, source_ids, edges) =
                        original_word(captured, states, projection, &lasso)?;
                    *stage = Stage::Attribution;
                    let properties = attribute(
                        model,
                        plans,
                        &states,
                        lasso.prefix.len(),
                        limits.atom_cells,
                        (control, evaluations),
                        cache.map(|cache| (cache, source_ids.as_slice())),
                    )?;
                    if !properties.contains(&p.name) {
                        return Err(AnalysisError::Inconsistent(
                            "accepting candidate failed target-property attribution",
                        ));
                    }
                    return Ok(Some(CandidateWord {
                        target_property: p.name.clone(),
                        properties,
                        states,
                        back_to: lasso.prefix.len(),
                        edges,
                    }));
                }
            }
        }
    }
    Ok(None)
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
    read: &mut periodic::external::NativeRead<'_>,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
    stage: &mut Stage,
    evaluations: &mut EvaluationErrors,
    control: &dive_check::budget::EvaluationControl,
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
        None,
        None,
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
