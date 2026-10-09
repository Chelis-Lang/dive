//! Final-only pure liveness analysis through the shared safety engine.
//!
//! This library coordinator does not implement TLC's exact planning/selection,
//! periodic checks or symmetry. WF/SF expansion uses TLC-side enabledness. Counterexample candidates
//! still require independent validation; direct formula evaluation attributes
//! their complete configured property set.
//! The CLI retains its liveness refusal until that integration is complete.

use dive_check::{CheckOptions, Verdict, capture};
use dive_eval::{St, next::StateVals};
use dive_model::Model;

use crate::Graph;
use crate::formula::{self, AtomKind, Tableau};
use crate::plan::{self, Plan, PlanningError};
use crate::product::{self, Labels, Product};

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
    /// Tableau construction failed.
    Tableau(formula::FormulaError),
    /// Product construction or acceptance search failed.
    Product(product::ProductError),
    /// Safety exploration stopped or refused.
    Exploration(Box<Verdict>),
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
        }
    }
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Planning(e) => e.fmt(f),
            Self::Tableau(e) => e.fmt(f),
            Self::Product(e) => e.fmt(f),
            Self::Exploration(v) => write!(f, "liveness exploration stopped: {v:?}"),
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
        }
    }
}
impl std::error::Error for AnalysisError {}

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

fn labels(
    model: &Model,
    plan: &Plan,
    graph: &capture::BehaviorGraph,
    property: &str,
    limit: usize,
) -> Result<Labels, AnalysisError> {
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
            state_row.push(if pred.kind == AtomKind::State {
                atom(model, plan, index, &St::state(vals.clone()), property)?
            } else {
                false
            });
            let mut row = Vec::new();
            for edge in &graph.edges[v] {
                row.push(if pred.kind == AtomKind::Action {
                    atom(
                        model,
                        plan,
                        index,
                        &St::action(vals.clone(), graph.states[edge.to].clone()),
                        property,
                    )?
                } else {
                    false
                });
            }
            action_rows.push(row);
        }
        states.push(state_row);
        actions.push(action_rows);
    }
    Ok(Labels { states, actions })
}

fn attribute(
    model: &Model,
    plans: &[(Plan, Tableau)],
    states: &[StateVals],
    back: usize,
    cells: usize,
) -> Result<Vec<String>, AnalysisError> {
    let mut violated = Vec::new();
    for (p, (plan, _)) in model.implied_temporals.iter().zip(plans) {
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
    if model.implied_temporals.is_empty() {
        return Err(AnalysisError::Unsupported("no temporal properties"));
    }
    if !matches!(&model.postconditions, Ok(p) if p.is_empty()) {
        return Err(AnalysisError::Unsupported(
            "postconditions with temporal checking",
        ));
    }
    if model.implied_temporals.len() > limits.properties {
        return Err(AnalysisError::Limit("properties"));
    }
    let plans: Vec<(Plan, Tableau)> = model
        .implied_temporals
        .iter()
        .map(|p| {
            let plan = plan::lower_violation(model, &p.pred, limits.formula)
                .map_err(AnalysisError::Planning)?;
            let tableau =
                formula::compile(&plan.formula, limits.formula).map_err(AnalysisError::Tableau)?;
            Ok((plan, tableau))
        })
        .collect::<Result<_, AnalysisError>>()?;
    let captured = match capture::explore(model, opts, limits.capture) {
        capture::Exploration::Complete(c) => *c,
        capture::Exploration::Stopped(v) => return Err(AnalysisError::Exploration(Box::new(v))),
    };
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
    for (p, (plan, tableau)) in model.implied_temporals.iter().zip(&plans) {
        let labels = labels(model, plan, &captured.graph, &p.name, limits.atom_cells)?;
        let product = Product::new(&graph, tableau, &labels, limits.product)
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
            let properties = attribute(
                model,
                &plans,
                &states,
                lasso.prefix.len(),
                limits.atom_cells,
            )?;
            if !properties.contains(&p.name) {
                return Err(AnalysisError::Inconsistent(
                    "accepting candidate failed target-property attribution",
                ));
            }
            return Ok(Analysis::Candidate(Candidate {
                target_property: p.name.clone(),
                properties,
                states,
                back_to: lasso.prefix.len(),
                edges,
                exploration: captured,
            }));
        }
    }
    Ok(Analysis::Holds(captured))
}
