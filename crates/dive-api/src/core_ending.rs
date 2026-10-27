//! Error endings of `--engine=core` (`docs/developer/core/07-engine.md` §5,
//! `02-semantic-contract.md` §3(e)).
//!
//! The core records where an evaluation error ended its search (the site,
//! the parent state, the state checked), but its errors carry no TLC class.
//! The class is TLC's, by the site where the error is raised, and it depends
//! on the error's own code and on whether it is an `EvalException`. So the
//! ending is reproduced with the reference evaluator, on the same states:
//! the failing action, or the failing check, is evaluated again, and the
//! reference engine's table (`dive_check::classify`) classifies the error it
//! raises. The search itself, its counts and TLC's call-stack replay are the
//! core's.
//!
//! The reproduction is also a cross-check. If the reference evaluator does
//! not raise an error there, or its action emits a different number of
//! states before the error, the run is refused rather than reported.

use std::sync::Arc;

use dive_check::{ErrorSite, Verdict, classify};
use dive_eval::next::{Flow, Successor};
use dive_eval::{Ec, EvalError, St};
use dive_explore::{Failure, Replayed, Site};
use dive_model::{Model, Named};
use dive_value::Value;

/// How a core run that failed ends, as the reference engine reports it.
pub(crate) struct Ending {
    /// The verdict.
    pub verdict: Verdict,
    /// Whether the counterexample ends with the state checked.
    pub with: bool,
    /// Whether the search stopped in the initial states.
    pub in_init: bool,
}

/// A state of handles as the reference evaluator's values.
pub(crate) fn state(
    model: &Model,
    store: &Arc<dive_core::Store>,
    hs: &[dive_core::Handle],
) -> Result<Arc<[Option<Value>]>, String> {
    let ctx = model.evaluator.options().ctx;
    let mut heap = dive_core::Heap::new(Arc::clone(store), false, false);
    let mut out = Vec::with_capacity(hs.len());
    for &h in hs {
        out.push(if h.is_empty() {
            None
        } else {
            Some(crate::core_engine::to_value(&mut heap, model, h, ctx)?)
        });
    }
    Ok(out.into())
}

/// TLC's text of a state (`TLCStateMut.toString`), as the reference engine
/// prints it.
pub(crate) fn state_text(model: &Model, s: &[Option<Value>]) -> String {
    use std::fmt::Write as _;
    let ctx = model.evaluator.options().ctx;
    let order = model.ir.tlc_var_order();
    let single = order.len() == 1;
    let mut out = String::new();
    for v in order {
        let name = &model.ir.decl(model.ir.vars()[v.index()]).name;
        let val = s.get(v.index()).and_then(Option::as_ref).map_or_else(
            || "null".to_owned(),
            |x| {
                dive_value::print::to_tlc_string(x, ctx).map_or_else(
                    |_| dive_value::print::to_tlc_string_lenient(x, ctx),
                    |t| dive_value::print::ppr(&t),
                )
            },
        );
        if single {
            let _ = writeln!(out, "{name} = {val}");
        } else {
            let _ = writeln!(out, "/\\ {name} = {val}");
        }
    }
    out
}

/// The check at a site, with its reference-engine site.
fn check_of(model: &Model, site: Site) -> Option<(&Named, ErrorSite)> {
    match site {
        Site::Constraint(i) => model.constraints.get(i).map(|c| (c, ErrorSite::Constraint)),
        Site::ActionConstraint(i) => model
            .action_constraints
            .get(i)
            .map(|c| (c, ErrorSite::ActionConstraint)),
        Site::Invariant(i) => model.invariants.get(i).map(|c| (c, ErrorSite::Invariant)),
        Site::Init | Site::Next(_) | Site::Incomplete | Site::Fingerprint => None,
    }
}

/// Counts the emissions of `run` until its error; the error, or why the
/// reference evaluator disagrees.
fn reproduce_actions(
    emitted: u64,
    run: impl FnOnce(&mut dyn FnMut(&Successor<'_>) -> Flow) -> Result<(), EvalError>,
) -> Result<EvalError, String> {
    let mut n = 0u64;
    let r = run(&mut |_| {
        n += 1;
        Flow::Continue
    });
    match r {
        Err(e) if n == emitted => Ok(e),
        Err(_) => Err(format!(
            "the reference evaluator emits {n} states before the error, the core {emitted}"
        )),
        Ok(()) => Err("the reference evaluator raises no error there".into()),
    }
}

/// The reference evaluator's error at the failure: its own code and
/// provenance, which decide TLC's class.
fn reproduce(
    model: &Model,
    store: &Arc<dive_core::Store>,
    f: &Failure,
) -> Result<EvalError, String> {
    let ev = &model.evaluator;
    let parent = f
        .parent
        .as_deref()
        .map(|p| state(model, store, p))
        .transpose()?;
    match f.site {
        Site::Init => reproduce_actions(f.emitted, |sink| {
            ev.init_states(&model.init_actions, sink)
                .map(|_| ())
                .map_err(|e| e.error)
        }),
        Site::Fingerprint => {
            let t = f
                .state
                .as_deref()
                .ok_or("a fingerprint failure without its state")?;
            let t = state(model, store, t)?;
            let vals: Vec<Value> = t
                .iter()
                .map(|v| v.clone().ok_or("an unassigned variable"))
                .collect::<Result<_, _>>()?;
            let fp = dive_store::state::StateFingerprinter::new(0)
                .ok_or("no fingerprint polynomial 0")?;
            match fp.fingerprint(&vals, ev.options().ctx) {
                Err(dive_store::StoreError::Value(v)) => Ok(EvalError::from(v)),
                Err(e) => Err(format!("the reference store fails otherwise: {e}")),
                Ok(_) => Err("the reference engine fingerprints the state".into()),
            }
        }
        Site::Next(i) => {
            let a = model
                .actions
                .get(i)
                .ok_or_else(|| format!("action {i} out of range"))?;
            let s0 = parent.ok_or("a successor failure without its parent")?;
            reproduce_actions(f.emitted, |sink| {
                ev.next_states(a, &s0, 0, sink)
                    .map(|_| ())
                    .map_err(|e| e.error)
            })
        }
        site => {
            let (c, _) = check_of(model, site).ok_or("a check out of range")?;
            let t = state(
                model,
                store,
                f.state
                    .as_deref()
                    .ok_or("a check failure without its state")?,
            )?;
            let st = match (site, parent) {
                (Site::ActionConstraint(_), Some(s0)) => St::action(s0, t),
                _ => St::state(t),
            };
            match model.eval(&c.pred, &st) {
                Err(e) => Ok(e),
                // `Tool.isValid`: TLC's `TLC_EXPECTED_VALUE` at the check.
                Ok(v) if !matches!(v, Value::Bool(_)) => Ok(EvalError::coded(
                    Ec::ExpectedValue,
                    format!(
                        "TLC expected a boolean value, but did not find one. {}",
                        ev.location(c.pred.location)
                    ),
                )),
                Ok(_) => Err(format!("the reference evaluator answers {} there", c.name)),
            }
        }
    }
}

/// TLC's verdict for a state that leaves variables unassigned
/// (`ModelChecker.doNextSetErr`; the reference engine's `incomplete`).
fn incomplete(model: &Model, f: &Failure, t: &[Option<Value>]) -> Verdict {
    let variables: Vec<String> = t
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_none())
        .map(|(i, _)| model.ir.decl(model.ir.vars()[i]).name.clone())
        .collect();
    let message = if f.initial {
        format!(
            "current state is not a legal state\nWhile working on the initial state:\n{}",
            state_text(model, t)
        )
    } else {
        let are = if variables.len() > 1 { "s are" } else { " is" };
        let list = variables.join(", ");
        if model.actions.len() == 1 {
            format!(
                "Successor state is not completely specified by the next-state action. The following variable{are} not defined: {list}."
            )
        } else {
            let a = f.action.and_then(|i| model.actions.get(i));
            let name = a
                .and_then(|a| crate::core_engine::action_name(model, a))
                .or_else(|| {
                    a.map(|a| {
                        a.label()
                            .trim_start_matches('<')
                            .split(' ')
                            .next()
                            .unwrap_or_default()
                            .to_owned()
                    })
                })
                .unwrap_or_default();
            format!(
                "Successor state is not completely specified by action {name} of the next-state relation. The following variable{are} not defined: {list}."
            )
        }
    };
    Verdict::NotCompletelySpecified {
        initial: f.initial,
        variables,
        message,
    }
}

/// The verdict of an error the reference evaluator reproduced at `f`.
fn classified(model: &Model, f: &Failure, e: &EvalError) -> Verdict {
    match f.site {
        Site::Init => classify::search_error(e, ErrorSite::Init, None),
        Site::Next(_) | Site::Incomplete => classify::search_error(e, ErrorSite::Next, None),
        Site::Fingerprint => classify::search_error(e, ErrorSite::Fingerprint, None),
        site => match check_of(model, site) {
            Some((c, s)) => classify::search_error(e, s, Some(&c.name)),
            None => classify::search_error(e, ErrorSite::Next, None),
        },
    }
}

/// How the failed core run ends: the verdict as the reference engine
/// classifies it, and whether its trace ends with the state checked.
///
/// # Errors
/// The refusal when the reference evaluator does not reproduce the error.
pub(crate) fn ending(
    model: &Model,
    store: &Arc<dive_core::Store>,
    f: &Failure,
    replayed: Option<&Replayed>,
) -> Result<Ending, Verdict> {
    let refuse = |why: String| Verdict::Unsupported {
        feature: format!(
            "an evaluation error the reference evaluator does not reproduce (core engine): {why} (the core raised: {})",
            f.message
        ),
    };
    let t = f
        .state
        .as_deref()
        .map(|s| state(model, store, s))
        .transpose()
        .map_err(refuse)?;
    if f.site == Site::Incomplete {
        let t = t.ok_or_else(|| refuse("an incomplete state without its values".into()))?;
        return Ok(Ending {
            verdict: incomplete(model, f, &t),
            with: !f.initial,
            in_init: f.initial,
        });
    }
    let e = reproduce(model, store, f).map_err(refuse)?;
    let mut verdict = classified(model, f, &e);
    let mut with = check_of(model, f.site).is_some();
    if f.initial {
        // `DoInitFunctor` records the state checked; `doInit` prints it for
        // a `RuntimeException` only (`class::init_error`).
        let text = t.as_deref().map(|t| state_text(model, t));
        verdict = classify::init_error(verdict, text.as_deref());
        with = with
            && !matches!(
                verdict,
                Verdict::EvaluationError { reported: 1000, .. }
                    | Verdict::EvaluationError {
                        site: ErrorSite::ImpliedInit,
                        ..
                    }
            );
    } else if matches!(f.site, Site::Invariant(_)) {
        if f.in_model {
            if let Some(err) = replayed.and_then(|r| r.error.as_ref()) {
                let e2 = if err.site == Site::Incomplete {
                    None
                } else {
                    Some(reproduce(model, store, err).map_err(refuse)?)
                };
                if let Some(e2) = e2 {
                    verdict = classify::replay_error(verdict, &e2);
                }
            }
        } else {
            // TLC's replay checks a state outside the model again.
            verdict = classify::recurring(verdict);
        }
    }
    Ok(Ending {
        verdict,
        with,
        in_init: f.initial,
    })
}
