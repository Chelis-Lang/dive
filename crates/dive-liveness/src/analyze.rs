//! Final-only pure liveness analysis through the shared safety engine.
//!
//! This library coordinator does not implement TLC's exact planning/selection,
//! periodic checks or symmetry. WF/SF expansion uses TLC-side enabledness. Counterexample candidates
//! still require independent validation; direct formula evaluation attributes
//! their complete configured property set.
//! The CLI retains its liveness refusal until that integration is complete.

use dive_check::{CheckOptions, CheckResult, capture};
use dive_eval::{St, next::StateVals};
use dive_model::Model;

use crate::Graph;
use crate::formula::{self, AtomKind, Recurrence, Tableau};
use crate::plan::{self, Plan, PlanningError};
use crate::product::{self, AcceptancePolicy, CompactProduct, Labels};

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
    /// Maximum state and action label cells for one property.
    pub atom_cells: usize,
    /// Resident-memory ceiling, in bytes, sampled while the tableau, labels
    /// and product are built (see [`crate::resident`]). `None` samples nothing.
    pub resident_bytes: Option<u64>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            formula: formula::Limits::default(),
            capture: capture::Limits::default(),
            product: product::Limits::default(),
            properties: 256,
            atom_cells: 8_000_000,
            resident_bytes: None,
        }
    }
}
impl Limits {
    /// TLC's setting: no fixed entry or work count in planning, capture,
    /// labelling or product construction. TLC bounds liveness checking only by
    /// memory and time. The resident-memory ceiling stands for TLC's heap:
    /// [`crate::resident::default_ceiling`], a quarter of physical memory, the
    /// JVM's default maximum heap. Exceeding it, or an external timeout, is
    /// never a holding result. The `usize::MAX`
    /// formula-node count is also the atoms' per-binder limit, which lifts the
    /// evaluator's pure predicate replay work ceilings as well.
    ///
    /// ```
    /// use dive_liveness::analyze::Limits;
    /// let limits = Limits::unbounded();
    /// assert_eq!(limits.atom_cells, usize::MAX);
    /// assert_eq!(limits.capture.generated, u64::MAX);
    /// ```
    #[must_use]
    pub fn unbounded() -> Self {
        Self {
            formula: formula::Limits::unbounded(),
            capture: capture::Limits::unbounded(),
            product: product::Limits::unbounded(),
            properties: usize::MAX,
            atom_cells: usize::MAX,
            resident_bytes: crate::resident::default_ceiling(),
        }
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
    /// Internal disagreement between acceptance and direct attribution.
    Inconsistent(&'static str),
    /// A capture thread failure lost execution evidence.
    CaptureThread(String),
}
impl AnalysisError {
    /// Stable diagnostics for the library analysis stages. These are dive
    /// diagnostics, not a claim of TLC's planning/error classification.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Planning(e) => e.code(),
            Self::Tableau(_) => "DIVE-L0030",
            Self::Product(_) => "DIVE-L0031",
            Self::Exploration(_) => "DIVE-L0032",
            Self::Atom { .. } => "DIVE-L0033",
            Self::Limit(_) => "DIVE-L0034",
            Self::Unsupported(_) => "DIVE-L0035",
            Self::Inconsistent(_) => "DIVE-L0036",
            Self::CaptureThread(_) => "DIVE-L0037",
        }
    }
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
            Self::Inconsistent(s) => write!(f, "internal liveness analysis error: {s}"),
            Self::CaptureThread(s) => write!(f, "liveness capture thread failure: {s}"),
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

/// A failure retaining the legacy error and an actual action cause, when present.
/// Legacy entrypoints keep their existing error shapes; no diagnostic is parsed.
#[derive(Clone, Debug)]
pub struct DetailedFailure {
    failure: Failure,
    action: Option<Box<ActionEvaluation>>,
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
        self.action.as_deref()
    }
    /// Consumes this description without changing the legacy public error shape.
    #[must_use]
    pub fn into_parts(self) -> (Failure, Option<Box<ActionEvaluation>>) {
        (self.failure, self.action)
    }
}
impl std::fmt::Display for DetailedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.failure.fmt(f)
    }
}
impl std::error::Error for DetailedFailure {}

fn atom(
    model: &Model,
    plan: &Plan,
    index: usize,
    st: &St,
    property: &str,
) -> Result<bool, AnalysisError> {
    let pred = &plan.atoms[index].pred;
    let failure = |detail| AnalysisError::Atom {
        property: property.into(),
        source: Box::new(plan::Source::new(model, pred.location)),
        detail,
    };
    plan.atoms[index]
        .evaluate(model, st)
        .map_err(|e| failure(e.to_string()))
}

fn selected_targets(
    graph: &capture::BehaviorGraph,
    source: usize,
) -> Result<std::collections::BTreeMap<usize, StateVals>, AnalysisError> {
    let malformed = || AnalysisError::Inconsistent("original action target row shape");
    let mut originals = graph.concrete.get(source).ok_or_else(malformed)?.iter();
    let mut selected = std::collections::BTreeMap::new();
    for edge in &graph.edges[source] {
        if edge.to >= graph.states.len() {
            return Err(malformed());
        }
        if let Some(ordinal) = edge.ordinal {
            let original = originals.next().ok_or_else(malformed)?;
            if original.ordinal != ordinal || edge.label.as_deref() != Some(original.label.as_str())
            {
                return Err(malformed());
            }
            selected
                .entry(edge.to)
                .or_insert_with(|| original.state.clone());
        } else {
            if edge.to != source {
                return Err(malformed());
            }
            selected
                .entry(source)
                .or_insert_with(|| graph.states[source].clone());
        }
    }
    if originals.next().is_some() {
        return Err(malformed());
    }
    Ok(selected)
}

fn labels(
    model: &Model,
    plan: &Plan,
    graph: &capture::BehaviorGraph,
    property: &str,
    limit: usize,
    active: &(std::collections::BTreeSet<usize>, Option<usize>),
    action_error: &mut Option<Box<ActionEvaluation>>,
) -> Result<(Labels, usize), AnalysisError> {
    let edges = graph
        .edges
        .iter()
        .try_fold(0_usize, |n, r| n.checked_add(r.len()))
        .ok_or(AnalysisError::Limit("atom cells"))?;
    graph
        .states
        .len()
        .checked_add(edges)
        .and_then(|n| n.checked_mul(plan.atoms.len()))
        .filter(|n| *n <= limit)
        .ok_or(AnalysisError::Limit("atom cells"))?;
    let mut states = Vec::new();
    let mut actions = Vec::new();
    for (index, pred) in plan.atoms.iter().enumerate() {
        let mut state_row = Vec::new();
        let mut action_rows = Vec::new();
        for (v, vals) in graph.states.iter().enumerate() {
            crate::resident::tick().map_err(|()| AnalysisError::Limit("resident memory"))?;
            state_row.push(if pred.kind == AtomKind::State {
                atom(model, plan, index, &St::state(vals.clone()), property)?
            } else {
                false
            });
            let mut row = Vec::new();
            let targets = if pred.kind == AtomKind::Action && active.0.contains(&index) {
                selected_targets(graph, v)?
            } else {
                std::collections::BTreeMap::new()
            };
            let mut evaluated = std::collections::BTreeMap::new();
            for edge in &graph.edges[v] {
                let value = if let Some(target) = targets.get(&edge.to) {
                    if let Some(value) = evaluated.get(&edge.to) {
                        *value
                    } else {
                        let value = pred
                            .evaluate_detailed(model, &St::action(vals.clone(), target.clone()))
                            .map_err(|failure| {
                                let core = matches!(&failure, plan::AtomError::Core(_));
                                let cause = failure.into_error();
                                let source = Box::new(plan::Source::new(model, pred.pred.location));
                                let error = AnalysisError::Atom {
                                    property: property.into(),
                                    source: source.clone(),
                                    detail: cause.to_string(),
                                };
                                *action_error = Some(Box::new(ActionEvaluation {
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
                row.push(value);
            }
            action_rows.push(row);
        }
        states.push(state_row);
        actions.push(action_rows);
    }
    Ok((Labels { states, actions }, edges))
}

fn attribute(
    model: &Model,
    plans: &[(Plan, Tableau, Vec<Recurrence>)],
    states: &[StateVals],
    back: usize,
    cells: usize,
) -> Result<Vec<String>, AnalysisError> {
    let mut violated = Vec::new();
    for (p, (plan, _, _)) in model.property_claims().iter().zip(plans) {
        states
            .len()
            .checked_mul(plan.atoms.len())
            .filter(|n| *n <= cells)
            .ok_or(AnalysisError::Limit("attribution atom cells"))?;
        let mut rows = Vec::new();
        for (i, state) in states.iter().enumerate() {
            let next = if i + 1 == states.len() { back } else { i + 1 };
            let st = St::action(state.clone(), states[next].clone());
            let row = (0..plan.atoms.len())
                .map(|a| atom(model, plan, a, &st, &p.name))
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
/// typed action cause when that evaluation actually failed.
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
    let mut stage = Stage::Planning;
    let mut exploration = None;
    let mut action = None;
    analyze_observed(
        model,
        opts,
        limits,
        policy,
        &mut stage,
        &mut exploration,
        &mut action,
    )
    .map_err(|error| DetailedFailure {
        failure: Failure {
            error,
            stage,
            exploration,
        },
        action,
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

fn analyze_observed(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    stage: &mut Stage,
    exploration: &mut Option<Box<ExplorationEvidence>>,
    action_error: &mut Option<Box<ActionEvaluation>>,
) -> Result<Analysis, AnalysisError> {
    let _resident = crate::resident::enter(limits.resident_bytes);
    validate_analysis_context(model)?;
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
    let plans: Vec<(Plan, Tableau, Vec<Recurrence>)> = model
        .property_claims()
        .iter()
        .map(|p| {
            let plan = plan::lower_violation(model, &p.pred, limits.formula)
                .map_err(AnalysisError::Planning)?;
            let (core, conditions) = plan
                .formula
                .factor_recurrences(limits.formula.formula_nodes)
                .map_err(AnalysisError::Tableau)?;
            let tableau =
                formula::compile(&core, limits.formula).map_err(AnalysisError::Tableau)?;
            Ok((plan, tableau, conditions))
        })
        .collect::<Result<_, AnalysisError>>()?;
    *stage = Stage::Capture;
    let captured =
        match capture::explore_with_evidence(model, opts, limits.capture).map_err(|message| {
            *stage = Stage::Unknown;
            AnalysisError::CaptureThread(message)
        })? {
            capture::Exploration::Complete(c) => *c,
            capture::Exploration::Stopped(v) => return Err(AnalysisError::Exploration(v)),
        };
    let word = analyze_captured(
        model,
        &plans,
        &captured,
        limits,
        policy,
        stage,
        action_error,
    )
    .inspect_err(|_| {
        *exploration = Some(Box::new(ExplorationEvidence::from(&captured)));
    })?;
    Ok(match word {
        Some(word) => Analysis::Candidate(Candidate {
            target_property: word.target_property,
            properties: word.properties,
            states: word.states,
            back_to: word.back_to,
            edges: word.edges,
            exploration: captured,
        }),
        None => Analysis::Holds(captured),
    })
}

struct CandidateWord {
    target_property: String,
    properties: Vec<String>,
    states: Vec<StateVals>,
    back_to: usize,
    edges: Vec<capture::Edge>,
}

fn analyze_captured(
    model: &Model,
    plans: &[(Plan, Tableau, Vec<Recurrence>)],
    captured: &capture::Complete,
    limits: Limits,
    policy: AcceptancePolicy,
    stage: &mut Stage,
    action_error: &mut Option<Box<ActionEvaluation>>,
) -> Result<Option<CandidateWord>, AnalysisError> {
    *stage = Stage::Product;
    let graph = Graph::new(
        captured
            .graph
            .edges
            .iter()
            .map(|r| r.iter().map(|e| e.to).collect())
            .collect(),
        captured.graph.roots.clone(),
    )
    .map_err(|e| AnalysisError::Product(product::ProductError::Shape(e.to_string())))?;
    for (p, (plan, tableau, conditions)) in model.property_claims().iter().zip(plans) {
        *stage = Stage::Atoms;
        let active = if plan.atoms.iter().any(|atom| atom.kind == AtomKind::Action) {
            crate::tlc_plan::active_actions(&plan.formula, limits.formula)
                .map_err(AnalysisError::Tableau)?
        } else {
            (std::collections::BTreeSet::new(), None)
        };
        let (labels, behavior_edges) = labels(
            model,
            plan,
            &captured.graph,
            &p.name,
            limits.atom_cells,
            &active,
            action_error,
        )?;
        *stage = Stage::Product;
        let product = CompactProduct::with_policy_and_edges(
            &graph,
            tableau,
            &labels,
            conditions,
            limits.product,
            policy,
            behavior_edges,
        )
        .map_err(AnalysisError::Product)?;
        if let Some(lasso) = product.find_lasso().map_err(AnalysisError::Product)? {
            let steps = lasso.prefix.iter().chain(&lasso.cycle);
            let states: Vec<StateVals> = steps
                .clone()
                .map(|s| captured.graph.states[s.from].clone())
                .collect();
            let edges = steps
                .map(|s| captured.graph.edges[s.from][s.edge].clone())
                .collect();
            *stage = Stage::Attribution;
            let properties =
                attribute(model, plans, &states, lasso.prefix.len(), limits.atom_cells)?;
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
    Ok(None)
}
