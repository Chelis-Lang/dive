//! A test-only stand-in for `dive-model` (WP-B1) and `dive-check` (WP-D2):
//! binds the configuration's roles the way TLC's `SpecProcessor` does for
//! the models it can handle, and runs TLC's single-worker breadth-first
//! search with no store and no parallelism, to check the successor
//! generator's counts against TLC's.
//!
//! Anything it does not handle is reported as unsupported, naming it. It
//! follows `ModelChecker.DoInitFunctor` and `Worker.addElement`/`run` for
//! what is counted and when the search stops (`tlc2/tool/ModelChecker.java`,
//! `tlc2/tool/Worker.java` at tlaplus `94d0c50`).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use dive_config::ModelConfig;
use dive_eval::next::{Action, Flow, Scope, StateVals, Successor};
use dive_eval::{ErrorClass, EvalError, Evaluator, Options, St};
use dive_ir::actions::{ActionName, split};
use dive_ir::{DefId, Expr, ExprId, Ir, JunctionKind, Level, OpRef, Symbol};
use dive_value::Value;

use super::{Loaded, bind};

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

fn name_of(ir: &Ir, name: &str) -> Result<DefId, String> {
    match ir.lookup(ir.root(), name) {
        Some(Symbol::Def(d)) => Ok(d),
        _ => Err(format!(
            "unsupported: {name} is not a definition of the root module"
        )),
    }
}

fn zero_arity(ir: &Ir, d: DefId, what: &str) -> Result<(), String> {
    if ir.def(d).params.is_empty() {
        Ok(())
    } else {
        Err(format!("{what} {} takes arguments", ir.def(d).name))
    }
}

/// What a `SPECIFICATION` or `PROPERTY` walk found.
#[derive(Default)]
struct Parts {
    inits: Vec<(Scope, ExprId, ActionName)>,
    /// The next-state relation, its scope, the definition whose frame it is
    /// in, and whether it is under a substitution (then one action).
    next: Option<(Scope, ExprId, DefId, bool)>,
    implied_actions: Vec<(Scope, ExprId)>,
    /// `PROPERTY []P` for a state predicate `P`, with the name TLC gives it.
    invariants: Vec<(Scope, ExprId, Option<String>)>,
    temporal: Vec<String>,
}

struct Walker<'a> {
    ev: &'a Evaluator,
    ir: &'a Ir,
    redirects: &'a HashMap<DefId, DefId>,
    props: bool,
}

impl Walker<'_> {
    /// `SpecProcessor.processConfigSpec` (and `processConfigProps` with
    /// `props`), for the shapes the corpus uses. `frame` is the definition
    /// whose frame `e` is in; `wrapped`, whether a substitution was entered.
    fn walk(
        &self,
        e: ExprId,
        scope: &Scope,
        frame: DefId,
        wrapped: bool,
        out: &mut Parts,
    ) -> Result<(), String> {
        let ir = self.ir;
        let node = ir.node(e);
        let err = |e: dive_eval::EvalError| e.to_string();
        match &node.expr {
            Expr::Subst { bindings, body } if bindings.is_empty() => {
                self.walk(*body, scope, frame, wrapped, out)
            }
            Expr::Subst { body, .. } => {
                let s = self.ev.scope_subst(scope, e).map_err(err)?;
                self.walk(*body, &s, frame, true, out)
            }
            Expr::Label { body, .. } | Expr::Selected(body) => {
                self.walk(*body, scope, frame, wrapped, out)
            }
            Expr::Let { .. } => {
                Err("unsupported: a LET in a SPECIFICATION or PROPERTY (WP-B1)".into())
            }
            Expr::Def(d)
            | Expr::Apply {
                callee: OpRef::Def(d),
                ..
            } => {
                let d = self.redirects.get(d).copied().unwrap_or(*d);
                let def = ir.def(d);
                if def.std.is_some() {
                    return Err(
                        "unsupported: an overridden operator as a SPECIFICATION conjunct".into(),
                    );
                }
                if def.scope != dive_ir::DefScope::TopLevel {
                    return Err("unsupported: a non-top-level definition in a SPECIFICATION".into());
                }
                let (s, d) = self.ev.scope_call(scope, e).map_err(err)?;
                let def = ir.def(d);
                if matches!(node.expr, Expr::Def(_)) && ir.node(def.body).level == Level::State {
                    out.inits.push((s, def.body, ActionName::Def(d)));
                    return Ok(());
                }
                self.walk(def.body, &s, d, wrapped, out)
            }
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for &i in items {
                    self.walk(i, scope, frame, wrapped, out)?;
                }
                Ok(())
            }
            Expr::Always(a) => {
                if self.props && ir.node(*a).level <= Level::State {
                    // `[]P` for a state predicate P is checked as an invariant.
                    let name = match &ir.node(*a).expr {
                        Expr::Def(d) => Some(ir.def(*d).name.clone()),
                        _ => None,
                    };
                    out.invariants.push((scope.clone(), *a, name));
                } else if let Expr::StepBox { action, .. } = &ir.node(*a).expr {
                    if self.props {
                        out.implied_actions.push((scope.clone(), *a));
                    } else if out.next.is_some() {
                        return Err("TLC_CANT_HANDLE_TOO_MANY_NEXT_STATE_RELS".into());
                    } else {
                        out.next = Some((scope.clone(), *action, frame, wrapped));
                    }
                } else {
                    out.temporal.push(ir.location(&node.origin));
                }
                Ok(())
            }
            _ => {
                if node.level <= Level::State {
                    out.inits.push((scope.clone(), e, ActionName::Unnamed));
                } else if node.level == Level::Temporal {
                    out.temporal.push(ir.location(&node.origin));
                } else {
                    return Err(format!(
                        "TLC_CANT_HANDLE_CONJUNCT {}",
                        ir.location(&node.origin)
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Binds the model's roles.
#[allow(clippy::too_many_lines, reason = "one pass over the configuration")]
pub fn bind_model(loaded: &Loaded, cfg: &ModelConfig, opts: Options) -> Result<Bound, String> {
    let ir = loaded.ir.clone();
    for (what, v) in [
        ("SYMMETRY", &cfg.symmetry),
        ("PERIODIC", &cfg.periodic),
        ("RL_REWARD", &cfg.rl_reward),
    ] {
        if v.is_some() {
            return Err(format!("unsupported: {what}"));
        }
    }
    if !cfg.postconditions.is_empty() {
        return Err("unsupported: POSTCONDITION".into());
    }
    if !cfg.possible.is_empty() {
        return Err("unsupported: POSSIBLE".into());
    }
    let model = bind(loaded, Some(cfg), opts)?;
    let redirects = model.redirects.clone();
    let ev = Evaluator::new(model, opts);
    if let Some(e) = ev.startup_error() {
        return Err(format!("start-up error: {e}"));
    }
    let err = |e: dive_eval::EvalError| e.to_string();
    let mut init_preds: Vec<Action> = Vec::new();
    let mut next: Option<(Scope, ExprId, DefId, ActionName, bool)> = None;
    if let Some(spec) = &cfg.specification {
        let d = name_of(&ir, &spec.text)?;
        zero_arity(&ir, d, "SPECIFICATION")?;
        let w = Walker {
            ev: &ev,
            ir: &ir,
            redirects: &redirects,
            props: false,
        };
        let mut parts = Parts::default();
        w.walk(ir.def(d).body, &ev.scope(d), d, false, &mut parts)?;
        for (i, (scope, expr, name)) in parts.inits.iter().enumerate() {
            init_preds.push(ev.predicate_in(scope, *expr, *name, i));
        }
        if let Some((s, a, f, wrapped)) = parts.next {
            next = Some((s, a, f, ActionName::Unnamed, wrapped));
        }
    } else {
        if let Some(init) = &cfg.init {
            let d = name_of(&ir, &init.text)?;
            zero_arity(&ir, d, "INIT")?;
            init_preds.push(ev.init_predicate(d, 0).map_err(err)?);
        }
        if let Some(n) = &cfg.next {
            let d = name_of(&ir, &n.text)?;
            zero_arity(&ir, d, "NEXT")?;
            next = Some((ev.scope(d), ir.def(d).body, d, ActionName::Def(d), false));
        }
    }
    let actions = match next {
        Some((scope, expr, frame, name, wrapped)) => {
            if wrapped {
                // `Specs.addSubsts`' wrapper is one action (`getActions`).
                vec![ev.predicate_in(&scope, expr, name, init_preds.len())]
            } else {
                let r: BTreeMap<DefId, DefId> = redirects.iter().map(|(a, b)| (*a, *b)).collect();
                let sp = split(&ir, expr, name, frame, &r);
                ev.actions_in(&scope, &sp, init_preds.len()).map_err(err)?
            }
        }
        None => Vec::new(),
    };
    let def_check = |n: &dive_config::Name| -> Result<Check, String> {
        let d = name_of(&ir, &n.text)?;
        let d = redirects.get(&d).copied().unwrap_or(d);
        zero_arity(&ir, d, "the predicate")?;
        Ok(Check {
            name: n.text.clone(),
            pred: ev.predicate(d, ir.def(d).body, ActionName::Def(d), 0),
        })
    };
    let mut invariants = cfg
        .invariants
        .iter()
        .map(def_check)
        .collect::<Result<Vec<_>, _>>()?;
    let constraints = cfg
        .constraints
        .iter()
        .map(def_check)
        .collect::<Result<Vec<_>, _>>()?;
    let action_constraints = cfg
        .action_constraints
        .iter()
        .map(def_check)
        .collect::<Result<Vec<_>, _>>()?;
    let view = cfg.view.as_ref().map(def_check).transpose()?;
    let mut implied_inits = Vec::new();
    let mut implied_actions = Vec::new();
    // Fairness conjuncts of the SPECIFICATION alone do not make TLC check
    // liveness; temporal PROPERTYs do.
    let mut liveness = Vec::new();
    for p in &cfg.properties {
        let d = name_of(&ir, &p.text)?;
        let w = Walker {
            ev: &ev,
            ir: &ir,
            redirects: &redirects,
            props: true,
        };
        let mut pp = Parts::default();
        w.walk(ir.def(d).body, &ev.scope(d), d, false, &mut pp)?;
        for (scope, expr, _) in pp.inits {
            implied_inits.push(Check {
                name: p.text.clone(),
                pred: ev.predicate_in(&scope, expr, ActionName::Unnamed, 0),
            });
        }
        for (scope, expr, name) in pp.invariants {
            invariants.push(Check {
                name: name.unwrap_or_else(|| p.text.clone()),
                pred: ev.predicate_in(&scope, expr, ActionName::Unnamed, 0),
            });
        }
        for (scope, expr) in pp.implied_actions {
            implied_actions.push(Check {
                name: p.text.clone(),
                pred: ev.predicate_in(&scope, expr, ActionName::Unnamed, 0),
            });
        }
        if !pp.temporal.is_empty() {
            liveness.push(format!("PROPERTY {}", p.text));
        }
    }
    Ok(Bound {
        ev,
        inits: init_preds,
        actions,
        invariants,
        implied_inits,
        implied_actions,
        constraints,
        action_constraints,
        view,
        deadlock: cfg.checks_deadlock(),
        liveness,
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
