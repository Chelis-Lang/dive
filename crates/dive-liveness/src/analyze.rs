//! Scoped temporal planning and graph checking through the shared safety engine.
//!
//! Final analysis requires exhaustive capture. The prepared graph kernel also
//! finds candidates on completed-level prefixes; no absence of a prefix lasso
//! proves Holds. Candidates always require fresh independent validation.

use dive_check::{CheckOptions, CheckResult, capture};
use dive_eval::{St, next::StateVals};
use dive_model::Model;

use crate::Graph;
use crate::formula::{self, Recurrence, Tableau};
use crate::plan::{self, Plan, PlanningError};
use crate::product::{self, Product};

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
    /// Maximum state/action cells and additional concrete-coherence work for
    /// one property, including bounded lossless transport bytes.
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
    /// TLC's observable formula classification, without tableau identities.
    Classification(crate::tlc_plan::ProjectionError),
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
        /// Typed evaluator cause, preserving error class and TLC code.
        error: Box<dive_eval::EvalError>,
    },
    /// Actual concrete occurrences cannot safely use the stored quotient.
    Concrete {
        /// Property whose evaluation reached this point.
        property: String,
        /// Original predicate or Next source.
        source: Box<plan::Source>,
        /// The observed incoherence or unavailable bounded replay.
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
            Self::Classification(e) => e.code(),
            Self::Tableau(_) => "DIVE-L0030",
            Self::Product(_) => "DIVE-L0031",
            Self::Exploration(_) => "DIVE-L0032",
            Self::Atom { .. } => "DIVE-L0033",
            Self::Limit(_) => "DIVE-L0034",
            Self::Unsupported(_) => "DIVE-L0035",
            Self::Inconsistent(_) => "DIVE-L0036",
            Self::CaptureThread(_) => "DIVE-L0037",
            Self::Concrete { .. } => "DIVE-L0038",
        }
    }
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Planning(e) => e.fmt(f),
            Self::Classification(e) => e.fmt(f),
            Self::Tableau(e) => e.fmt(f),
            Self::Product(e) => e.fmt(f),
            Self::Exploration(v) => write!(f, "liveness exploration stopped: {:?}", v.verdict),
            Self::Atom {
                property,
                source,
                detail,
                ..
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
            Self::Concrete {
                property,
                source,
                detail,
            } => write!(
                f,
                "{}: unsupported concrete temporal quotient for {property} at {}: {detail}",
                self.code(),
                source.location
            ),
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
    /// None for exhausted exploration; Some for a completed-level prefix.
    pub boundary: Option<u64>,
}
impl From<&capture::Complete> for ExplorationEvidence {
    fn from(capture: &capture::Complete) -> Self {
        Self {
            stats: capture.stats,
            workers: capture.workers.clone(),
            collision: capture.collision.clone(),
            output: capture.output.clone(),
            warnings: capture.warnings.clone(),
            boundary: None,
        }
    }
}

impl From<&capture::Prefix> for ExplorationEvidence {
    fn from(prefix: &capture::Prefix) -> Self {
        Self {
            stats: prefix.stats,
            workers: prefix.workers.clone(),
            collision: prefix.collision.clone(),
            output: prefix.output.clone(),
            warnings: prefix.warnings.clone(),
            boundary: Some(prefix.level),
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

pub(crate) fn atom(
    model: &Model,
    plan: &Plan,
    index: usize,
    st: &St,
    property: &str,
) -> Result<bool, AnalysisError> {
    let pred = &plan.atoms[index].pred;
    let failure = |error: dive_eval::EvalError| AnalysisError::Atom {
        property: property.into(),
        source: Box::new(plan::Source::new(model, pred.location)),
        detail: error.to_string(),
        error: Box::new(error),
    };
    plan.atoms[index].evaluate(model, st).map_err(failure)
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
    let mut stage = Stage::Planning;
    let mut exploration = None;
    analyze_observed(model, opts, limits, &mut stage, &mut exploration).map_err(|error| Failure {
        error,
        stage,
        exploration,
    })
}

/// Prepared scoped temporal formulas, compiled once for a run.
/// Successful preparation alone is not a verdict.
pub struct Prepared {
    plans: Vec<(Plan, Tableau, Vec<Recurrence>)>,
    limits: Limits,
}

/// Prepares every behavior conjunct and property before graph exploration.
/// TLC's classifier is applied to original fairness and properties before
/// negation; its resource bounds remain separate from semantic refusals.
///
/// # Errors
/// Unsupported language/context, evaluator errors or explicit formula limits.
pub fn prepare(model: &Model, limits: Limits) -> Result<Prepared, AnalysisError> {
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
    let lowered = model
        .property_claims()
        .iter()
        .map(|p| {
            plan::lower_violation(model, &p.pred, limits.formula).map_err(AnalysisError::Planning)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fairness = model
        .temporals
        .iter()
        .map(|p| {
            plan::lower(model, &p.pred, limits.formula)
                .map(|p| p.formula)
                .map_err(AnalysisError::Planning)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // TLC parseLiveness receives getImpliedTemporals after SpecProcessor
    // separates initial, invariant and action properties. Original configured
    // claims remain in the graph plans and independent attribution below.
    let properties = model
        .implied_temporals
        .iter()
        .map(|p| {
            plan::lower(model, &p.pred, limits.formula)
                .map(|p| p.formula)
                .map_err(AnalysisError::Planning)
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::tlc_plan::classify(
        &fairness.iter().collect::<Vec<_>>(),
        &properties.iter().collect::<Vec<_>>(),
        limits.formula,
    )
    .map_err(AnalysisError::Classification)?;
    let plans = lowered
        .into_iter()
        .map(|plan| {
            let (core, conditions) = plan
                .formula
                .factor_recurrences(limits.formula.formula_nodes)
                .map_err(AnalysisError::Tableau)?;
            let tableau =
                formula::compile(&core, limits.formula).map_err(AnalysisError::Tableau)?;
            Ok((plan, tableau, conditions))
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;
    Ok(Prepared { plans, limits })
}

fn analyze_observed(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    stage: &mut Stage,
    exploration: &mut Option<Box<ExplorationEvidence>>,
) -> Result<Analysis, AnalysisError> {
    let prepared = prepare(model, limits)?;
    *stage = Stage::Capture;
    let captured =
        match capture::explore_with_evidence(model, opts, limits.capture).map_err(|message| {
            *stage = Stage::Unknown;
            AnalysisError::CaptureThread(message)
        })? {
            capture::Exploration::Complete(c) => *c,
            capture::Exploration::Stopped(v) => return Err(AnalysisError::Exploration(v)),
            capture::Exploration::Prefix(_) => {
                return Err(AnalysisError::Inconsistent(
                    "unobserved capture returned a prefix",
                ));
            }
        };
    let word = check_graph(model, &prepared, &captured.graph, &captured.options, stage)
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

/// An attributed accepting word; it still requires independent validation.
#[derive(Clone, Debug)]
pub struct CandidateWord {
    /// Property whose negation selected this word.
    pub target_property: String,
    /// Every configured claim false on this word.
    pub properties: Vec<String>,
    /// Concrete word positions in IR variable order.
    pub states: Vec<StateVals>,
    /// Zero-based closing target.
    pub back_to: usize,
    /// Real captured transitions, including closure.
    pub edges: Vec<capture::Edge>,
}

/// Finds and attributes an accepting word on a committed graph.
/// None on a prefix does not establish Holds; only exhaustive capture does.
/// `options` must be the actual producer options; alias identity preparation
/// never infers a producer profile from stored values or public graph fields.
///
/// # Errors
/// Exact labeling, product search, attribution or an explicit bound failed.
pub fn check_graph(
    model: &Model,
    prepared: &Prepared,
    behavior: &capture::BehaviorGraph,
    options: &CheckOptions,
    stage: &mut Stage,
) -> Result<Option<CandidateWord>, AnalysisError> {
    let plans = &prepared.plans;
    let limits = prepared.limits;
    *stage = Stage::Product;
    let graph = Graph::new(
        behavior
            .edges
            .iter()
            .map(|r| r.iter().map(|e| e.to).collect())
            .collect(),
        behavior.roots.clone(),
    )
    .map_err(|e| AnalysisError::Product(product::ProductError::Shape(e.to_string())))?;
    for (p, (plan, tableau, conditions)) in model.property_claims().iter().zip(plans) {
        *stage = Stage::Atoms;
        let labels = crate::concrete::labels(model, plan, behavior, options, &p.name, limits)?;
        *stage = Stage::Product;
        let product =
            Product::with_recurrences(&graph, tableau, &labels, conditions, limits.product)
                .map_err(AnalysisError::Product)?;
        if let Some(lasso) = product.find_lasso().map_err(AnalysisError::Product)? {
            let steps = lasso.prefix.iter().chain(&lasso.cycle);
            let states: Vec<StateVals> = steps
                .clone()
                .map(|s| behavior.states[s.from].clone())
                .collect();
            let edges = steps
                .map(|s| behavior.edges[s.from][s.edge].clone())
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
