//! Self-contained bounded final-only pinned graph, SCC and witness analysis.
//!
//! This constructs its own ordered execution graph; caller-edited graph evidence
//! cannot become proof authority. It does not implement periodic TLC scheduling,
//! trace aliases/recovery, initial replay errors or accepting-sink shortcuts.
//! Counterexamples remain candidates until independent validation succeeds.
use crate::{formula, phase_execution, phase_graph, plan, tlc_scc, tlc_witness};
use dive_check::{CheckOptions, Workers};
use dive_eval::{Options, Profile, St, next::StateVals};
use dive_model::Model;
use std::collections::BTreeMap;

/// Evidence of the actually checked PEM, separate from model safety counters.
#[derive(Clone, Debug)]
pub struct Checked {
    /// OOS first-encounter index.
    pub group: usize,
    /// PEM first-encounter index within the group.
    pub pem: usize,
    /// Pinned first-encounter SCC and optional witness fields.
    pub selection: tlc_witness::Selection,
}
/// Self-constructed complete graph and actual final checking evidence.
#[derive(Clone, Debug)]
pub struct Evidence {
    /// Original concrete safety states, output, counts, workers and temporal work.
    pub execution: Box<phase_execution::Complete>,
    /// PEMs actually checked, stopping at the first acceptance.
    pub checked: Vec<Checked>,
}
/// A concrete periodic candidate; independent validation remains mandatory.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Word positions, retaining the configured IR variable order.
    pub states: Vec<StateVals>,
    /// Target of the last word transition, zero-based.
    pub back_to: usize,
    /// Complete original configured claims falsified on this word.
    pub properties: Vec<String>,
    /// Constructed graph and first deciding OOS/PEM/component fields.
    pub evidence: Evidence,
}
/// Final-only supported fingerprint-graph result, separate from CLI verdicts.
#[derive(Clone, Debug)]
pub enum Analysis {
    /// Every supported OOS/PEM was checked without an accepting component.
    /// This retains the safety engine's fingerprint collision qualification.
    Holds(Evidence),
    /// A pinned selected candidate; never an independently validated violation.
    Candidate(Candidate),
}
/// A refusal, resource/evaluation failure or inconsistency; never a holds result.
#[derive(Clone, Debug)]
pub enum Error {
    /// The explicit final-only/TLC/fingerprint boundary is not satisfied.
    Unsupported(&'static str),
    /// Actual ordered execution failed with its deciding worker/prefix evidence.
    Execution(Box<phase_execution::Failure>),
    /// Model graph projection failed after complete execution.
    Projection(phase_graph::Error),
    /// Bounded SCC or pinned witness selection failed.
    Selection(tlc_scc::Error),
    /// Scoped direct attribution planning failed.
    Planning(plan::PlanningError),
    /// Direct word attribution or concrete recovery is unavailable.
    Attribution(String),
    /// A fixed entry/cell/inspection ceiling was exceeded.
    Limit(&'static str),
}
impl Error {
    /// Stable diagnostic. This does not assert a TLC error-class mapping.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Execution(e) => e.error.code(),
            Self::Projection(e) => e.code(),
            Self::Selection(e) => e.code(),
            Self::Planning(e) => e.code(),
            Self::Unsupported(_) => "DIVE-L0063",
            Self::Attribution(_) => "DIVE-L0064",
            Self::Limit(_) => "DIVE-L0065",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(e) => write!(f, "unsupported final-only TLC check: {e}"),
            Self::Execution(e) => e.error.fmt(f),
            Self::Projection(e) => e.fmt(f),
            Self::Selection(e) => e.fmt(f),
            Self::Planning(e) => e.fmt(f),
            Self::Attribution(e) => write!(f, "final-only witness attribution: {e}"),
            Self::Limit(e) => write!(f, "final-only check resource limit: {e}"),
        }
    }
}
impl std::error::Error for Error {}
/// Failure evidence keeps completed graph facts without certifying a verdict.
#[derive(Clone, Debug)]
pub struct Failure {
    /// Deciding refusal or typed failure.
    pub error: Error,
    /// Constructed execution evidence if execution completed.
    pub evidence: Option<Box<Evidence>>,
    /// Actual width, including truthful zero for preflight admission failures.
    pub workers: Option<Workers>,
}
#[allow(
    clippy::unnecessary_box_returns,
    reason = "public failure API retains potentially large deciding execution evidence boxed"
)]
fn early(error: Error, options: &CheckOptions) -> Box<Failure> {
    Box::new(Failure {
        error,
        evidence: None,
        workers: Some(Workers {
            requested: options.workers,
            used: 0,
            sequential: false,
            reason: Some(
                "final-only parameter/ownership/profile admission before execution".into(),
            ),
        }),
    })
}
fn word(
    execution: &phase_execution::Complete,
    group: usize,
    candidate: &tlc_witness::Candidate,
    maximum: usize,
) -> Result<(Vec<StateVals>, usize), Error> {
    candidate
        .prefix_reverse
        .len()
        .checked_add(candidate.cycle_stack.len())
        .and_then(|n| n.checked_add(candidate.closure_reverse.len()))
        .filter(|n| *n <= maximum)
        .ok_or(Error::Limit("concrete witness positions"))?;
    let g = &execution.groups[group];
    let mut representatives = BTreeMap::new();
    for commit in &g.commits {
        let vals = execution
            .capture
            .graph
            .states
            .get(commit.source)
            .ok_or_else(|| Error::Attribution("concrete source reference".into()))?;
        representatives.entry(commit.fingerprint).or_insert(vals);
    }
    let mut prefix = Vec::new();
    for &id in candidate.prefix_reverse.iter().rev() {
        let fp = g.nodes[id].fingerprint;
        if prefix.last() != Some(&fp) {
            prefix.push(fp);
        }
    }
    let anchor = g.nodes[candidate.anchor].fingerprint;
    if prefix.last() != Some(&anchor) {
        return Err(Error::Attribution("prefix anchor identity".into()));
    }
    let back_to = prefix
        .len()
        .checked_sub(1)
        .ok_or_else(|| Error::Attribution("empty prefix".into()))?;
    let mut reverse = candidate.closure_reverse.clone();
    for &id in candidate.cycle_stack.iter().rev() {
        let fp = g.nodes[id].fingerprint;
        if reverse.last() != Some(&fp) {
            reverse.push(fp);
        }
    }
    reverse.dedup(); // pinned LongVec.pack removes contiguous identical fingerprints.
    if reverse.last() == Some(&anchor) {
        reverse.pop();
    }
    prefix.extend(reverse.into_iter().rev());
    let states = prefix
        .into_iter()
        .map(|fp| {
            representatives
                .get(&fp)
                .map(|v| (*v).clone())
                .ok_or_else(|| {
                    Error::Attribution(
                        "concrete TLC trace recovery is unavailable for this fingerprint".into(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((states, back_to))
}
fn attribute(
    model: &Model,
    states: &[StateVals],
    back: usize,
    cells: usize,
) -> Result<Vec<String>, Error> {
    let mut properties = Vec::new();
    for claim in model.property_claims() {
        let plan = plan::lower_violation(model, &claim.pred, formula::Limits::default())
            .map_err(Error::Planning)?;
        states
            .len()
            .checked_mul(plan.atoms.len())
            .filter(|n| *n <= cells)
            .ok_or(Error::Limit("attribution atom cells"))?;
        let mut rows = Vec::new();
        for (i, current) in states.iter().enumerate() {
            let next = if i + 1 == states.len() { back } else { i + 1 };
            let st = St::action(current.clone(), states[next].clone());
            rows.push(
                plan.atoms
                    .iter()
                    .map(|a| {
                        a.evaluate(model, &st)
                            .map_err(|e| Error::Attribution(e.to_string()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let truth = plan
            .formula
            .evaluate_word(&rows, &rows, back, cells)
            .map_err(|e| Error::Attribution(e.to_string()))?;
        if truth[0] && !properties.contains(&claim.name) {
            properties.push(claim.name.clone());
        }
    }
    if properties.is_empty() {
        return Err(Error::Attribution(
            "acceptance produced no false configured claim".into(),
        ));
    }
    Ok(properties)
}

/// Execute and check the explicitly final-only supported TLC fingerprint subset.
///
// The shipping context authority additionally admits captured ownership. Keep
// this legacy-base endpoint's arena/role guards callback-free; integration must
// coalesce them with Model::validate_context before Session planning.
fn role_metadata(model: &Model) -> bool {
    let valid = |p: &dive_model::Pred| {
        model.ir.nodes().get(p.expr.index()).is_some()
            && model.ir.nodes().get(p.location.index()).is_some()
            && model.ir.defs().get(p.scope.def().index()).is_some()
    };
    model.init.iter().all(|i| {
        valid(&i.pred)
            && i.def
                .is_none_or(|d| model.ir.defs().get(d.index()).is_some())
    }) && model.next.as_ref().is_none_or(valid)
        && model
            .invariants
            .iter()
            .chain(&model.implied_inits)
            .chain(&model.implied_actions)
            .chain(&model.temporals)
            .chain(&model.implied_temporals)
            .chain(&model.configured_properties)
            .chain(&model.constraints)
            .chain(&model.action_constraints)
            .chain(model.view.iter())
            .chain(model.alias.as_ref().and_then(|a| a.as_ref().ok()))
            .chain(model.postconditions.as_ref().into_iter().flatten())
            .all(|n| valid(&n.pred))
        && model
            .assumptions
            .iter()
            .all(|d| model.ir.defs().get(d.index()).is_some())
        && model.init_actions.iter().chain(&model.actions).all(|a| {
            model.ir.nodes().get(a.expr().index()).is_some()
                && model.ir.defs().get(a.scope().def().index()).is_some()
        })
}
/// This entry accepts a model, never a caller-supplied completed graph. It runs
/// actual ordered source commits, then OOS/PEM SCC checks and pinned witness
/// selection. `Holds` has the engine's collision qualifications; candidates
/// require independent validation before any violation report. Periodic checks
/// and CLI strict liveness integration remain unsupported.
///
/// # Errors
/// Named profile/exact/symmetry/alias boundaries, admission failures, actual
/// execution errors, fixed ceilings or unavailable concrete witness attribution.
#[allow(
    clippy::too_many_lines,
    reason = "retain admission/execution/SCC/attribution boundaries and failure evidence"
)]
pub fn check(
    model: &Model,
    options: &CheckOptions,
    limits: phase_graph::Limits,
) -> Result<Analysis, Box<Failure>> {
    if options.workers == 0 || options.workers > limits.workers || options.workers > 64 {
        return Err(early(Error::Limit("requested worker width"), options));
    }
    if model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .is_err()
    {
        return Err(early(
            Error::Unsupported("foreign IR/name table before model planning"),
            options,
        ));
    }
    if !role_metadata(model) {
        return Err(early(
            Error::Unsupported("malformed bound role metadata before planning"),
            options,
        ));
    }
    if options.sequential || !model.sequential.is_empty() {
        return Err(early(
            Error::Unsupported("sequential/fallback observed temporal execution is not certified"),
            options,
        ));
    }
    if options.profile != Profile::Tlc
        || options.eval != Options::tlc()
        || model.evaluator.options() != &Options::tlc()
        || !options.tlc_fingerprints
    {
        return Err(early(
            Error::Unsupported("pinned TLC profile and state fingerprints required"),
            options,
        ));
    }
    if options.store != CheckOptions::tlc(options.fp_index, options.seed).store {
        return Err(early(
            Error::Unsupported(
                "exact concrete identity is not certified by the fingerprint temporal graph",
            ),
            options,
        ));
    }
    if model.symmetry.is_some() || model.view.is_some() || model.alias.is_some() {
        return Err(early(
            Error::Unsupported("symmetry/VIEW/ALIAS concrete witness recovery"),
            options,
        ));
    }
    if model.property_claims().len() > 256 {
        return Err(early(Error::Limit("configured property count"), options));
    }
    let execution = phase_execution::execute(model, options, limits).map_err(|e| {
        Box::new(Failure {
            workers: e.workers.clone(),
            error: Error::Execution(e),
            evidence: None,
        })
    })?;
    let mut evidence = Evidence {
        execution: Box::new(execution),
        checked: Vec::new(),
    };
    let result = (|| {
        for group in 0..evidence.execution.groups.len() {
            for pem in 0..evidence
                .execution
                .pem_count(group)
                .map_err(Error::Projection)?
            {
                let (graph, labels) = evidence
                    .execution
                    .product_inputs(group, pem)
                    .map_err(Error::Projection)?;
                let identities = evidence.execution.groups[group]
                    .nodes
                    .iter()
                    .map(|n| tlc_witness::Identity {
                        fingerprint: n.fingerprint,
                        tableau: n.tableau,
                    })
                    .collect::<Vec<_>>();
                let selection = tlc_witness::select(&graph, &labels, &identities, limits.graph)
                    .map_err(Error::Selection)?;
                let candidate = selection.candidate.clone();
                evidence.checked.push(Checked {
                    group,
                    pem,
                    selection,
                });
                if let Some(candidate) = candidate {
                    let (states, back_to) = word(
                        &evidence.execution,
                        group,
                        &candidate,
                        limits.graph.vertices,
                    )?;
                    let properties =
                        attribute(model, &states, back_to, limits.graph.acceptance_cells)?;
                    return Ok(Some((states, back_to, properties)));
                }
            }
        }
        Ok(None)
    })();
    match result {
        Ok(Some((states, back_to, properties))) => Ok(Analysis::Candidate(Candidate {
            states,
            back_to,
            properties,
            evidence,
        })),
        Ok(None) => Ok(Analysis::Holds(evidence)),
        Err(error) => Err(Box::new(Failure {
            workers: Some(evidence.execution.capture.workers.clone()),
            error,
            evidence: Some(Box::new(evidence)),
        })),
    }
}
