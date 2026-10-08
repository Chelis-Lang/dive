//! A test-only stand-in for `dive-check` (WP-D2): binds the model with
//! `dive-model`, the binding stage, and runs TLC's single-worker
//! breadth-first search with no store and no parallelism, to check the
//! successor generator's counts against TLC's.
//!
//! Anything it does not handle is reported as unsupported, naming it. It
//! follows `ModelChecker.DoInitFunctor` and `Worker.addElement`/`run` for
//! what is counted and when the search stops (`tlc2/tool/ModelChecker.java`,
//! `tlc2/tool/Worker.java` at tlaplus `94d0c50`).

use std::collections::{HashSet, VecDeque};

use dive_config::ModelConfig;
use dive_eval::next::{Action, Flow, StateVals, Successor};
use dive_eval::{ErrorClass, EvalError, Evaluator, Options, St};
use dive_ir::actions::ActionName;
use dive_value::Value;

use super::Loaded;

/// A predicate the checker evaluates, with its context.
#[derive(Clone, Debug)]
pub struct Check {
    pub name: String,
    pub pred: Action,
}

/// The bound roles.
pub struct Bound {
    pub ev: Evaluator,
    pub inits: Vec<Action>,
    pub actions: Vec<Action>,
    pub invariants: Vec<Check>,
    pub implied_inits: Vec<Check>,
    pub implied_actions: Vec<Check>,
    pub constraints: Vec<Check>,
    pub action_constraints: Vec<Check>,
    pub view: Option<Check>,
    pub deadlock: bool,
    /// Temporal properties that need the liveness checker: the counts are
    /// comparable (TLC's search is the same), the verdict is not.
    pub liveness: Vec<String>,
}

/// The outcome of the search, in the terms the goldens use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Success,
    /// An invariant (`initial`: in an initial state) or implied initial
    /// predicate is violated.
    Invariant {
        name: String,
        initial: bool,
    },
    /// An implied action (`PROPERTY [][A]_v`) is violated.
    ActionProperty {
        name: String,
    },
    Deadlock,
    /// An evaluation error: TLC's code and message.
    EvalError {
        code: u32,
        message: String,
    },
    /// A state is not completely specified.
    NotCompletelySpecified {
        initial: bool,
    },
    /// dive refuses something on the way.
    Unsupported(String),
    /// The search went over the budget.
    TooBig,
}

/// The search's result.
#[derive(Clone, Debug)]
pub struct Checked {
    pub verdict: Verdict,
    pub generated: u64,
    pub distinct: u64,
    pub queue: u64,
    pub depth: u64,
    /// The counterexample's number of states.
    pub trace_len: Option<u64>,
    pub inits: u64,
    /// What the run printed (`Print` output and warnings).
    pub output: Vec<String>,
}

/// Binds the model with `dive-model`, the binding stage, and adapts its
/// roles to the search below. The search refuses what it does not do:
/// `SYMMETRY`, post conditions and the other features `dive-model` lists,
/// except temporal properties, whose counts are still comparable.
pub fn bind_model(loaded: &Loaded, cfg: &ModelConfig, opts: Options) -> Result<Bound, String> {
    if !cfg.postconditions.is_empty() {
        return Err("unsupported: POSTCONDITION".into());
    }
    let bind_opts = dive_model::BindOptions {
        eval: opts,
        no_deadlock: false,
    };
    let model =
        dive_model::bind(loaded.ir.clone(), cfg, &bind_opts).map_err(|f| match &f.error.kind {
            dive_model::ErrorKind::Unsupported { .. } => f.error.message.clone(),
            _ => format!("binding: {}", f.error),
        })?;
    if let Some(u) = model
        .unsupported
        .iter()
        .find(|u| !u.feature.starts_with("temporal properties"))
    {
        return Err(format!("unsupported: {}", u.feature));
    }
    let ev = model.evaluator.clone();
    if let Some(e) = ev.startup_error() {
        return Err(format!("start-up error: {e}"));
    }
    let check = |n: &dive_model::Named| Check {
        name: n.name.clone(),
        pred: ev.predicate_in(&n.pred.scope, n.pred.expr, ActionName::Unnamed, 0),
    };
    let checks = |v: &[dive_model::Named]| v.iter().map(check).collect::<Vec<_>>();
    Ok(Bound {
        inits: model.init_actions.clone(),
        actions: model.actions.clone(),
        invariants: checks(&model.invariants),
        implied_inits: checks(&model.implied_inits),
        implied_actions: checks(&model.implied_actions),
        constraints: checks(&model.constraints),
        action_constraints: checks(&model.action_constraints),
        view: model.view.as_ref().map(check),
        deadlock: model.settings.check_deadlock,
        liveness: model
            .implied_temporals
            .iter()
            .map(|n| format!("PROPERTY {}", n.name))
            .collect(),
        ev,
    })
}

fn verdict_of(e: &EvalError) -> Verdict {
    match e.class() {
        ErrorClass::Eval => Verdict::EvalError {
            code: e.ec().code(),
            message: e.message().to_owned(),
        },
        _ => Verdict::Unsupported(e.to_string()),
    }
}

/// Evaluates a check as a boolean.
fn holds(b: &Bound, c: &Check, st: &St) -> Result<bool, Verdict> {
    match b.ev.eval_action(&c.pred, st) {
        Ok(Value::Bool(v)) => Ok(v),
        Ok(_) => Err(Verdict::Unsupported(format!(
            "{} is not a boolean (TLC's error not reproduced)",
            c.name
        ))),
        Err(e) => Err(verdict_of(&e)),
    }
}

fn complete(s: &[Option<Value>]) -> Vec<Value> {
    s.iter()
        .map(|v| v.clone().unwrap_or(Value::FALSE))
        .collect()
}

/// TLC's single-worker breadth-first search over the bound model, stopping
/// once more than `budget` states have been generated.
#[allow(clippy::too_many_lines, reason = "one search loop")]
pub fn bfs(b: &Bound, budget: u64) -> Checked {
    #[allow(
        clippy::mutable_key_type,
        reason = "a value's interior caches do not change its hash or equality"
    )]
    let mut seen: HashSet<Vec<Value>> = HashSet::new();
    let mut queue: VecDeque<(StateVals, u64)> = VecDeque::new();
    let mut generated = 0u64;
    let mut depth = 0u64;
    let mut stop: Option<(Verdict, Option<u64>)> = None;
    let key = |s: &StateVals| -> Result<Vec<Value>, Verdict> {
        match &b.view {
            None => Ok(complete(s)),
            Some(v) => match b.ev.eval_action(&v.pred, &St::state(s.clone())) {
                Ok(x) => Ok(vec![x]),
                Err(e) => Err(verdict_of(&e)),
            },
        }
    };

    // Initial states: `DoInitFunctor.addElement`.
    let r = b.ev.init_states(&b.inits, &mut |succ: &Successor<'_>| {
        generated += 1;
        if !succ.complete() {
            stop = Some((Verdict::NotCompletelySpecified { initial: true }, Some(1)));
            return Flow::Stop;
        }
        let s: StateVals = succ.state.into();
        let st = St::state(s.clone());
        let mut in_model = true;
        for c in &b.constraints {
            match holds(b, c, &st) {
                Ok(true) => {}
                Ok(false) => {
                    in_model = false;
                    break;
                }
                Err(v) => {
                    stop = Some((v, Some(1)));
                    return Flow::Stop;
                }
            }
        }
        let mut was_seen = false;
        if in_model {
            let k = match key(&s) {
                Ok(k) => k,
                Err(v) => {
                    stop = Some((v, Some(1)));
                    return Flow::Stop;
                }
            };
            if seen.insert(k) {
                depth = depth.max(1);
                queue.push_back((s.clone(), 1));
            } else {
                was_seen = true;
            }
        }
        if !was_seen {
            for c in b.invariants.iter().chain(&b.implied_inits) {
                match holds(b, c, &st) {
                    Ok(true) => {}
                    Ok(false) => {
                        stop = Some((
                            Verdict::Invariant {
                                name: c.name.clone(),
                                initial: true,
                            },
                            Some(1),
                        ));
                        return Flow::Stop;
                    }
                    Err(v) => {
                        stop = Some((v, Some(1)));
                        return Flow::Stop;
                    }
                }
            }
        }
        if generated > budget {
            stop = Some((Verdict::TooBig, None));
            return Flow::Stop;
        }
        Flow::Continue
    });
    let inits = generated;
    if let Err(e) = r {
        stop = Some((verdict_of(&e.error), None));
    }
    let finish = |verdict: Verdict,
                  trace_len,
                  generated,
                  seen: &HashSet<Vec<Value>>,
                  queue: &VecDeque<(StateVals, u64)>,
                  depth| Checked {
        verdict,
        generated,
        distinct: seen.len() as u64,
        queue: queue.len() as u64,
        depth,
        trace_len,
        inits,
        output: b.ev.take_output(),
    };
    if let Some((v, t)) = stop {
        return finish(v, t, generated, &seen, &queue, depth);
    }

    while let Some((s0, level)) = queue.pop_front() {
        let pre = generated;
        let st0 = s0.clone();
        let r =
            b.ev.successors(&b.actions, &s0, &mut |succ: &Successor<'_>| {
                generated += 1;
                if !succ.complete() {
                    stop = Some((
                        Verdict::NotCompletelySpecified { initial: false },
                        Some(level + 1),
                    ));
                    return Flow::Stop;
                }
                let s: StateVals = succ.state.into();
                let st1 = St::state(s.clone());
                let st_step = St::action(st0.clone(), s.clone());
                let mut in_model = true;
                for c in &b.constraints {
                    match holds(b, c, &st1) {
                        Ok(true) => {}
                        Ok(false) => {
                            in_model = false;
                            break;
                        }
                        Err(v) => {
                            stop = Some((v, Some(level)));
                            return Flow::Stop;
                        }
                    }
                }
                if in_model {
                    for c in &b.action_constraints {
                        match holds(b, c, &st_step) {
                            Ok(true) => {}
                            Ok(false) => {
                                in_model = false;
                                break;
                            }
                            Err(v) => {
                                stop = Some((v, Some(level)));
                                return Flow::Stop;
                            }
                        }
                    }
                }
                let mut unseen = true;
                if in_model {
                    let k = match key(&s) {
                        Ok(k) => k,
                        Err(v) => {
                            stop = Some((v, Some(level)));
                            return Flow::Stop;
                        }
                    };
                    if seen.insert(k) {
                        depth = depth.max(level + 1);
                    } else {
                        unseen = false;
                    }
                }
                if unseen {
                    for c in &b.invariants {
                        match holds(b, c, &st1) {
                            Ok(true) => {}
                            Ok(false) => {
                                stop = Some((
                                    Verdict::Invariant {
                                        name: c.name.clone(),
                                        initial: false,
                                    },
                                    Some(level + 1),
                                ));
                                return Flow::Stop;
                            }
                            Err(v) => {
                                stop = Some((v, Some(level + 1)));
                                return Flow::Stop;
                            }
                        }
                    }
                }
                for c in &b.implied_actions {
                    match holds(b, c, &st_step) {
                        Ok(true) => {}
                        Ok(false) => {
                            stop = Some((
                                Verdict::ActionProperty {
                                    name: c.name.clone(),
                                },
                                Some(level + 1),
                            ));
                            return Flow::Stop;
                        }
                        Err(v) => {
                            stop = Some((v, Some(level + 1)));
                            return Flow::Stop;
                        }
                    }
                }
                if in_model && unseen {
                    queue.push_back((s, level + 1));
                }
                if generated > budget {
                    stop = Some((Verdict::TooBig, None));
                    return Flow::Stop;
                }
                Flow::Continue
            });
        if let Err(e) = r {
            // The trace ends at the state being expanded.
            stop = Some((verdict_of(&e.error), Some(level)));
        }
        if let Some((v, t)) = stop.take() {
            return finish(v, t, generated, &seen, &queue, depth);
        }
        if b.deadlock && generated == pre {
            return finish(
                Verdict::Deadlock,
                Some(level),
                generated,
                &seen,
                &queue,
                depth,
            );
        }
    }
    finish(Verdict::Success, None, generated, &seen, &queue, depth)
}

/// Runs `f` on a thread with the evaluator's recommended stack.
pub fn on_big_stack<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(f)
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "panicked".to_owned())
}
/// The golden's verdict in the harness's terms.
pub fn golden_verdict(g: &serde_json::Value) -> Option<Verdict> {
    let r = &g["result"];
    let name = r["violated_property"].as_str().unwrap_or("").to_owned();
    Some(match r["error_class"].as_str()? {
        "TLC_SUCCESS" => Verdict::Success,
        "TLC_INVARIANT_VIOLATED_BEHAVIOR" => Verdict::Invariant {
            name,
            initial: false,
        },
        "TLC_INVARIANT_VIOLATED_INITIAL" => Verdict::Invariant {
            name,
            initial: true,
        },
        "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR" => Verdict::ActionProperty { name },
        "TLC_DEADLOCK_REACHED" => Verdict::Deadlock,
        _ => return None,
    })
}

/// Compares; `None` when they agree.
pub fn diff(g: &serde_json::Value, d: &Checked, liveness: bool) -> Option<String> {
    let c = &g["result"]["counts"];
    let want = (
        c["states_generated"].as_u64(),
        c["distinct_states"].as_u64(),
        c["states_left_on_queue"].as_u64(),
        c["depth"].as_u64(),
    );
    let got = (
        Some(d.generated),
        Some(d.distinct),
        Some(d.queue),
        Some(d.depth),
    );
    if want != got {
        return Some(format!(
            "counts (generated, distinct, queue, depth): TLC {want:?}, dive {got:?}"
        ));
    }
    if !liveness {
        match golden_verdict(g) {
            Some(v) if v == d.verdict => {}
            v => return Some(format!("verdict: TLC {v:?}, dive {:?}", d.verdict)),
        }
        let len = g["result"]["trace"]["length"].as_u64();
        if len != d.trace_len {
            return Some(format!("trace length: TLC {len:?}, dive {:?}", d.trace_len));
        }
    }
    None
}
