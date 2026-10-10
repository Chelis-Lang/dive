//! Seeded finite safety simulation (ADR 0020).
//!
//! Implements the pinned TLC's ordinary, single-worker `Simulator` and
//! `SimulationWorker` selection loop. A finished sample is explicitly not a
//! model-checking proof. Temporal checking, effects, and unsupported options
//! refuse by name. The independent trace validator remains outside this crate.
#![forbid(unsafe_code)]

use dive_eval::next::{Flow, StateVals};
use dive_eval::{AssumptionOutcome, ErrorClass, EvalError, St};
use dive_ir::Effects;
use dive_model::{Model, Named};
use dive_stdlib::random::{RandomMode, RandomSource, Stream, Streams};
use dive_value::Value;

/// A finite simulation request. No unbounded trace count is accepted.
#[derive(Clone, Debug)]
pub struct Options {
    /// Run seed, always recorded by callers.
    pub seed: i64,
    /// TLC's `-aril`: skip this many doubles before deriving the worker seed.
    /// Currently only supported in TLC mode; zero is required in dive mode.
    pub aril: u64,
    /// Maximum number of transitions in each trace (TLC's `-depth`).
    pub depth: usize,
    /// Number of traces to sample, stopping on the first error.
    pub traces: u64,
    /// TLC's worker stream or dive's stable trace identities.
    pub mode: RandomMode,
    /// Maximum initial states or candidates retained for one action.
    pub max_candidates: usize,
    /// Maximum states retained across all trace documents.
    pub max_retained: usize,
    /// Maximum generated states, including discarded candidates.
    pub max_generated: u64,
}

impl Options {
    /// Bounds with a finite, explicit number of traces and transitions.
    #[must_use]
    pub const fn new(seed: i64, mode: RandomMode, depth: usize, traces: u64) -> Self {
        Self {
            seed,
            aril: 0,
            depth,
            traces,
            mode,
            max_candidates: 1_000_000,
            max_retained: 1_000_000,
            max_generated: 10_000_000,
        }
    }
}

/// An independently checkable trace of concrete states.
#[derive(Clone, Debug)]
pub struct Trace {
    /// Stable identity, starting at zero.
    pub id: u64,
    /// Initial state and chosen transitions; the final state is included.
    pub states: Vec<StateVals>,
}

/// The result of a simulation; a sample never proves absence of violations.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Requested sampling completed without finding an error.
    Sampled,
    /// An invariant, action property or deadlock was found.
    Violation {
        /// `invariant`, `property`, or `deadlock`.
        kind: &'static str,
        /// Configured property name, if any.
        name: Option<String>,
        /// TLC's deciding message code.
        code: u32,
        /// Index into the report's traces.
        trace: usize,
    },
    /// An assumption or evaluated predicate failed, with a measured class.
    Error {
        /// TLC's deciding message code.
        code: u32,
        /// Diagnostic detail.
        message: String,
    },
    /// An unimplemented behavior; no simulation conclusion.
    Unsupported(String),
    /// A configured resource bound was reached; no conclusion.
    Limit(String),
    /// An internal error; no conclusion.
    Internal(String),
}

/// A deciding violation observed without retaining its full bounded trace.
/// This is evidence for diagnosis, not independently certified violation data.
#[derive(Clone, Debug)]
pub struct PendingViolation {
    /// Invariant or action property kind.
    pub kind: &'static str,
    /// Configured property name.
    pub name: Option<String>,
    /// TLC's deciding message code.
    pub code: u32,
}

/// A finite simulation result and complete sampled trace documents.
#[derive(Clone, Debug)]
pub struct Report {
    /// Last reached phase; preflight refusals perform no model evaluation.
    pub stage: &'static str,
    /// Result, explicitly distinguishing samples from proofs.
    pub outcome: Outcome,
    /// Generated initial states plus all complete successor candidates checked.
    pub generated: u64,
    /// Initial candidates collected, including a prefix when enumeration fails.
    pub initial_candidates: u64,
    /// Observed violation whose complete trace could not be retained.
    pub pending_violation: Option<PendingViolation>,
    /// Traces started, including a trace that ends with a violation.
    pub traces_started: u64,
    /// Concrete trace data, retained subject to the request's bound.
    pub traces: Vec<Trace>,
}

/// Simulates a bound model with TLC's default (non-probabilistic) enumerator.
/// Run on a thread with [`dive_eval::RECOMMENDED_STACK`].
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "keep the pinned simulator selection and callback ordering together"
)]
pub fn simulate(model: &Model, opts: &Options) -> Report {
    let mut report = Report {
        stage: "preflight",
        outcome: Outcome::Sampled,
        generated: 0,
        initial_candidates: 0,
        pending_violation: None,
        traces_started: 0,
        traces: Vec::new(),
    };
    if let Some(outcome) = preflight(model, opts) {
        report.outcome = outcome;
        return report;
    }
    report.stage = "assumptions";
    match model.check_assumptions() {
        AssumptionOutcome::AllTrue => {}
        AssumptionOutcome::False { message, .. } => {
            report.outcome = Outcome::Error {
                code: 2104,
                message,
            };
            return report;
        }
        AssumptionOutcome::Error { error, message, .. } => {
            report.outcome = classify(&error, Some((2105, message)));
            return report;
        }
    }
    report.stage = "initialization";
    let mut inits = Vec::new();
    let mut failure = None;
    let enumeration = model.evaluator.init_states(&model.init_actions, &mut |s| {
        if inits.len() >= opts.max_candidates || report.initial_candidates >= opts.max_generated {
            failure = Some(Outcome::Limit("simulation initial-state bound".to_owned()));
            return Flow::Stop;
        }
        report.initial_candidates += 1;
        inits.push(StateVals::from(s.state));
        Flow::Continue
    });
    if let Err(e) = enumeration {
        // Simulator assigns numOfGenStates only after getInitStates completes.
        // A failure here has no current state and is reported as GENERAL.
        report.outcome = classify(&e.error, Some((1000, e.error.tool_text())));
        return report;
    }
    if let Some(failure) = failure {
        report.outcome = failure;
        return report;
    }
    report.generated = report.initial_candidates;
    let mut selected_inits = Vec::new();
    for init in &inits {
        if init.iter().any(Option::is_none) {
            report.outcome = Outcome::Error {
                code: 2106,
                message: "incomplete initial state".to_owned(),
            };
            return report;
        }
        let st = St::state(init.clone());
        if let Some(failure) = check_roles(model, &model.invariants, &st, "invariant", 2107, 2102) {
            if opts.max_retained == 0 {
                report.pending_violation = pending(&failure);
                report.outcome = Outcome::Limit("simulation retained-state bound".to_owned());
                return report;
            }
            report.traces.push(Trace {
                id: 0,
                states: vec![init.clone()],
            });
            report.outcome = failure;
            return report;
        }
        match allowed(model, &model.constraints, &st) {
            Ok(true) => selected_inits.push(init.clone()),
            Ok(false) => {}
            Err(e) => {
                report.outcome = classify(&e, None);
                return report;
            }
        }
    }
    if selected_inits.is_empty() {
        report.outcome = Outcome::Error {
            code: if inits.is_empty() { 2118 } else { 2256 },
            message: "no initial state satisfies the specification and constraints".to_owned(),
        };
        return report;
    }
    // Simulator.deepNormalize changes lazy cache state. Until the value API
    // exposes that operation, refuse compound initial values instead of guessing.
    if selected_inits
        .iter()
        .flat_map(|s| s.iter())
        .flatten()
        .any(|v| matches!(v, Value::Set(_) | Value::Fcn(_)))
    {
        report.outcome = Outcome::Unsupported(
            "simulation deep normalization of compound initial values".to_owned(),
        );
        return report;
    }
    let streams = Streams::new(opts.seed, opts.mode);
    let mut worker = Stream::Java(dive_stdlib::random::JavaRandom::new(
        streams.tlc_simulation_worker_seeds(opts.aril, 1)[0],
    ));
    let mut retained = 0usize;
    report.stage = "sampling";
    for id in 0..opts.traces {
        let mut rng = match opts.mode {
            RandomMode::Tlc => worker,
            RandomMode::Dive => streams.simulation_trace(id),
        };
        report.traces_started += 1;
        let mut states = vec![selected_inits[index(&mut rng, selected_inits.len())].clone()];
        if retained >= opts.max_retained {
            report.outcome = Outcome::Limit("simulation retained-state bound".to_owned());
            return report;
        }
        retained += 1;
        for _ in 0..opts.depth {
            let current = states[states.len() - 1].clone();
            let mut candidates = Vec::new();
            let mut action_index = index(&mut rng, model.actions.len());
            let prime = usize::try_from(rng.next_prime()).unwrap_or(0);
            let mut failed = None;
            for _ in 0..model.actions.len() {
                let action = &model.actions[action_index];
                let enumeration = model.evaluator.next_states(action, &current, 0, &mut |s| {
                    if candidates.len() >= opts.max_candidates
                        || report.generated >= opts.max_generated
                    {
                        failed = Some(Outcome::Limit("simulation successor bound".to_owned()));
                        return Flow::Stop;
                    }
                    if !s.complete() {
                        failed = Some(Outcome::Error {
                            code: 2109,
                            message: "incomplete successor state".to_owned(),
                        });
                        return Flow::Stop;
                    }
                    // SimulationWorker.collectPreSuccessor precedes properties and constraints.
                    report.generated += 1;
                    let next = StateVals::from(s.state);
                    let next_st = St::state(next.clone());
                    failed =
                        check_roles(model, &model.invariants, &next_st, "invariant", 2110, 2111)
                            .or_else(|| {
                                check_roles(
                                    model,
                                    &model.implied_actions,
                                    &St::action(current.clone(), next.clone()),
                                    "property",
                                    2112,
                                    2113,
                                )
                            });
                    if let Some(outcome) = &failed {
                        if retained >= opts.max_retained {
                            report.pending_violation = pending(outcome);
                            failed =
                                Some(Outcome::Limit("simulation retained-state bound".to_owned()));
                        } else {
                            retained += 1;
                            states.push(next);
                        }
                        return Flow::Stop;
                    }
                    match allowed(model, &model.constraints, &next_st).and_then(|yes| {
                        if yes {
                            allowed(
                                model,
                                &model.action_constraints,
                                &St::action(current.clone(), next.clone()),
                            )
                        } else {
                            Ok(false)
                        }
                    }) {
                        Ok(true) => candidates.push(next),
                        Ok(false) => {}
                        Err(e) => {
                            failed = Some(classify(&e, None));
                            return Flow::Stop;
                        }
                    }
                    Flow::Continue
                });
                if let Err(e) = enumeration {
                    failed = Some(classify(&e.error, None));
                }
                if failed.is_some() || !candidates.is_empty() {
                    break;
                }
                action_index = (action_index + prime) % model.actions.len();
            }
            if let Some(mut outcome) = failed {
                if let Outcome::Violation { trace, .. } = &mut outcome {
                    *trace = report.traces.len();
                }
                report.traces.push(Trace { id, states });
                report.outcome = outcome;
                return report;
            }
            if candidates.is_empty() {
                if model.settings.check_deadlock {
                    report.outcome = Outcome::Violation {
                        kind: "deadlock",
                        name: None,
                        code: 2114,
                        trace: report.traces.len(),
                    };
                    report.traces.push(Trace { id, states });
                    return report;
                }
                break;
            }
            if retained >= opts.max_retained {
                report.outcome = Outcome::Limit("simulation retained-state bound".to_owned());
                return report;
            }
            retained += 1;
            states.push(candidates[index(&mut rng, candidates.len())].clone());
        }
        worker = rng;
        report.traces.push(Trace { id, states });
    }
    report
}

fn pending(outcome: &Outcome) -> Option<PendingViolation> {
    if let Outcome::Violation {
        kind, name, code, ..
    } = outcome
    {
        Some(PendingViolation {
            kind,
            name: name.clone(),
            code: *code,
        })
    } else {
        None
    }
}

fn index(rng: &mut Stream, len: usize) -> usize {
    usize::try_from(rng.next_index(u64::try_from(len).unwrap_or(u64::MAX))).unwrap_or(0)
}

fn preflight(model: &Model, opts: &Options) -> Option<Outcome> {
    let refusal = |s: &str| Some(Outcome::Unsupported(s.to_owned()));
    if opts.traces == 0 {
        return refusal("simulation requires a positive finite trace count");
    }
    if opts.aril > 1_000_000 {
        return Some(Outcome::Limit("simulation aril skip bound".to_owned()));
    }
    if opts.mode == RandomMode::Dive && opts.aril != 0 {
        return refusal("nonzero aril outside TLC simulation mode");
    }
    if model.runtime_name_order {
        return refusal(
            "simulation runtime string encounter order requires independent namespace provenance",
        );
    }
    if let Some(u) = model.unsupported.first() {
        return refusal(&u.feature);
    }
    if !model.temporals.is_empty() || !model.implied_temporals.is_empty() {
        return refusal("simulation temporal properties and fairness");
    }
    if !model.implied_inits.is_empty() {
        return refusal(
            "simulation implied initial properties require a separate validation contract",
        );
    }
    if !model.implied_actions.is_empty()
        && (!model.constraints.is_empty() || !model.action_constraints.is_empty())
    {
        return refusal(
            "simulation implied action properties with state/action constraints require a separate validation contract",
        );
    }
    if model.symmetry.is_some() {
        return refusal("simulation SYMMETRY declarations");
    }
    if model.alias.is_some() {
        return refusal("simulation ALIAS");
    }
    if !matches!(&model.postconditions, Ok(p) if p.is_empty()) {
        return refusal("simulation POSTCONDITION");
    }
    if model.actions.len() > i32::MAX as usize {
        return Some(Outcome::Limit(
            "simulation action count exceeds TLC integer indexing".to_owned(),
        ));
    }
    let forbidden = Effects::IMPURE.union(Effects::OPEN);
    let predicates = model
        .init
        .iter()
        .map(|i| &i.pred)
        .chain(model.next.iter())
        .chain(model.invariants.iter().map(|n| &n.pred))
        .chain(model.implied_actions.iter().map(|n| &n.pred))
        .chain(model.constraints.iter().map(|n| &n.pred))
        .chain(model.action_constraints.iter().map(|n| &n.pred));
    if predicates.into_iter().any(|p| {
        model.ir.node(p.expr).effects.intersects(forbidden)
            || model.evaluator.scope_shares_side_effects(&p.scope)
    }) || model.assumptions.iter().any(|d| {
        model
            .ir
            .node(model.ir.def(*d).body)
            .effects
            .intersects(forbidden)
    }) {
        return refusal("simulation effects or unresolved constant-operator effects");
    }
    None
}

fn allowed(model: &Model, roles: &[Named], st: &St) -> Result<bool, EvalError> {
    for role in roles {
        match model.eval(&role.pred, st)? {
            Value::Bool(true) => {}
            Value::Bool(false) => return Ok(false),
            _ => {
                return Err(EvalError::unsupported(
                    "nonboolean simulation role diagnostic",
                ));
            }
        }
    }
    Ok(true)
}

fn check_roles(
    model: &Model,
    roles: &[Named],
    st: &St,
    kind: &'static str,
    code: u32,
    error_code: u32,
) -> Option<Outcome> {
    for role in roles {
        match model.eval(&role.pred, st) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => {
                return Some(Outcome::Violation {
                    kind,
                    name: Some(role.name.clone()),
                    code,
                    trace: 0,
                });
            }
            Ok(_) => {
                return Some(Outcome::Unsupported(
                    "nonboolean simulation property diagnostic".to_owned(),
                ));
            }
            Err(e) => return Some(classify(&e, Some((error_code, e.message().to_owned())))),
        }
    }
    None
}

fn classify(error: &EvalError, wrapper: Option<(u32, String)>) -> Outcome {
    match error.class() {
        ErrorClass::Unsupported => Outcome::Unsupported(error.message().to_owned()),
        ErrorClass::ResourceLimit => Outcome::Limit(error.message().to_owned()),
        ErrorClass::Bug => Outcome::Internal(error.message().to_owned()),
        ErrorClass::Eval => match wrapper {
            Some((code, message)) => Outcome::Error { code, message },
            None => Outcome::Unsupported(format!(
                "unverified simulation evaluation-error context: {}",
                error.message()
            )),
        },
    }
}
