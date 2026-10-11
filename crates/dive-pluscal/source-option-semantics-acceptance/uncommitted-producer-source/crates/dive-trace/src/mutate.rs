//! Negative controls: mutants of a valid counterexample that are invalid by
//! construction (ADR 0016 §3).
//!
//! Each mutant is invalid *given* that the original is a valid trace found
//! by breadth-first search, as TLC's single-worker counterexamples and
//! dive's are (ADR 0015): such a trace is a shortest path, so state `k` is
//! at distance exactly `k` from the initial states, and no two of its
//! states are equal.
//!
//! | Mutant | Why it is invalid |
//! |---|---|
//! | `drop-state-k`: remove a middle state | if `s(k-1) -> s(k+1)` were a step, `s(k+1)` would be at distance `k`, not `k+1` |
//! | `drop-first`: remove the initial state | if `s1` were initial, it would be at distance 0 |
//! | `swap-k`: exchange states `k` and `k+1` | the step into the earlier position would put `s(k+1)` at distance `k` (or make it initial) |
//! | `change-k-var`: one variable of a middle state becomes a model value that appears nowhere in the specification | the step into the state would have to set the variable to a value no expression of the specification can produce |
//! | `truncate`: remove the last state, keep the claim (invariant and property endings only) | the claimed invariant or property held at the new last state, or the search would have stopped there |
//! | `claim-Q`: claim a different invariant or property `Q` | `Q` either holds at the end, or is listed after the violated one, which is then false first |
//! | `relabel-k`: label step `k` with another action of the trace that is false on it | the label must hold on the step |
//! | `redraw-k-v`: a directly assigned random value is replaced by a different draw under another seed (only with the run's [`Replay`]) | a certified stream position fixes the assigned value; the independent validator must refute the mutation before it is included |
//!
//! Labels on the steps a mutation touches are removed, so a mutant is
//! rejected for its states, not its labels. The `change` mutants assume
//! every variable is constrained by the next-state relation, as TLC's
//! successor generator requires. The `redraw` mutants assume that the
//! variable gets its value only from that draw. They are produced only
//! for traces whose draw positions are certified, and checked individually.

use std::collections::BTreeSet;

use dive_model::Model;
use dive_stdlib::host::RunHost;
use dive_stdlib::random::Streams;
use dive_value::Value;

use crate::format::Ending;
use crate::trace::Trace;
use crate::validate::{Replay, Run, Validation};

/// The model value the `change` mutants use. It must not occur in the
/// specification or configuration.
pub const MUTANT_MODEL_VALUE: &str = "dive_trace_mutant";

/// A mutant: what was done, and the trace.
#[derive(Clone, Debug)]
pub struct Mutant {
    /// What was done, e.g. `swap-3`.
    pub name: String,
    /// The mutated trace.
    pub trace: Trace,
}

/// The by-construction mutants of `trace` (see the module documentation).
/// `replay` is the checking run's random streams, if known: with it, the
/// `redraw` mutants change values the trace drew.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the table in the module documentation, one block per row"
)]
pub fn mutants(model: &Model, trace: &Trace, replay: Option<&Replay>) -> Vec<Mutant> {
    let n = trace.states.len();
    let mut out = Vec::new();
    let base = Trace {
        length: None,
        ..trace.clone()
    };
    let mut push = |name: String, t: Trace| out.push(Mutant { name, trace: t });
    if n >= 3 {
        let k = n / 2;
        let mut t = base.clone();
        t.states.remove(k);
        t.labels.remove(k);
        t.labels[k] = None;
        push(format!("drop-state-{k}"), t);
    }
    if n >= 2 {
        let mut t = base.clone();
        t.states.remove(0);
        t.labels.remove(0);
        t.labels[0] = None;
        push("drop-first".to_owned(), t);
    }
    if n >= 2 {
        let k = if n >= 3 { (n - 1) / 2 } else { 0 };
        if trace.states[k] != trace.states[k + 1] {
            let mut t = base.clone();
            t.states.swap(k, k + 1);
            for j in k..(k + 3).min(n) {
                t.labels[j] = None;
            }
            push(format!("swap-{k}"), t);
        }
    }
    {
        let k = if n >= 3 { n / 2 } else { 0 };
        let vars = crate::trace::variables(model);
        let fresh = Value::model(&model.names, MUTANT_MODEL_VALUE);
        for (i, name) in vars.iter().enumerate() {
            let mut t = base.clone();
            let mut s: Vec<Option<Value>> = t.states[k].to_vec();
            s[i] = Some(fresh.clone());
            t.states[k] = s.into();
            t.labels[k] = None;
            if k + 1 < n {
                t.labels[k + 1] = None;
            }
            push(format!("change-{k}-{name}"), t);
        }
    }
    // Only a violation can be refuted at the new end: for a deadlock, an
    // error or a liveness ending, the validator cannot check the claim
    // itself (ADR 0016, amendment 1).
    if n >= 2 && matches!(trace.ending, Ending::Invariant(_) | Ending::Property(_)) {
        let mut t = base.clone();
        t.states.pop();
        t.labels.pop();
        push("truncate".to_owned(), t);
    }
    let names =
        |list: &[dive_model::Named]| list.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
    let claims: Vec<Ending> = match &trace.ending {
        Ending::Invariant(p) => names(&model.invariants)
            .into_iter()
            .filter(|q| q != p)
            .map(Ending::Invariant)
            .chain(std::iter::once(Ending::Invariant(format!(
                "{p}_dive_trace_mutant"
            ))))
            .collect(),
        Ending::Property(p) => {
            let list = if n == 1 {
                &model.implied_inits
            } else {
                &model.implied_actions
            };
            names(list)
                .into_iter()
                .filter(|q| q != p)
                .map(Ending::Property)
                .chain(std::iter::once(Ending::Property(format!(
                    "{p}_dive_trace_mutant"
                ))))
                .collect()
        }
        _ => names(&model.invariants)
            .into_iter()
            .take(1)
            .map(Ending::Invariant)
            .collect(),
    };
    for c in claims {
        let label = match &c {
            Ending::Invariant(q) | Ending::Property(q) => q.clone(),
            other => format!("{other:?}"),
        };
        push(
            format!("claim-{label}"),
            Trace {
                ending: c,
                ..base.clone()
            },
        );
    }
    if let Some(t) = relabel(model, &base) {
        out.push(t);
    }
    if let Some(r) = replay {
        out.extend(redraws(model, &base, r));
    }
    out
}

/// One mutant per variable that a replayed draw set: the variable gets
/// another value the same operator, applied to the same arguments, draws
/// from a stream of another seed. Include it only when the independent
/// validator refutes it at a certified stream position.
fn redraws(model: &Model, trace: &Trace, replay: &Replay) -> Vec<Mutant> {
    let draws = {
        let mut run = Run::new(model, Some(replay));
        run.record_draws();
        if !run.validate(trace).is_valid() {
            return Vec::new();
        }
        std::mem::take(&mut run.draws)
    };
    let ctx = model.evaluator.options().ctx;
    let vars = crate::trace::variables(model);
    let len = trace.states.len();
    let streams = replay.streams();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for d in &draws {
        let at = d.state;
        for (i, v) in trace.states[at].iter().enumerate() {
            if v.as_ref() != Some(&d.value)
                || (at > 0 && trace.states[at - 1][i].as_ref() == Some(&d.value))
                || seen.contains(&(at, i))
            {
                continue;
            }
            let draw = |host: &RunHost| match dive_eval::Host::random(host, d.op, &d.args, ctx) {
                Some(Ok(value)) => Some(value),
                _ => None,
            };
            let other = (1..=32).find_map(|j| {
                let host = RunHost::new(
                    Streams::new(streams.seed().wrapping_add(j), streams.mode()),
                    None,
                );
                draw(&host).filter(|value| *value != d.value)
            });
            let Some(replacement) = other else {
                continue;
            };
            seen.insert((at, i));
            let mut mutant = trace.clone();
            let mut values: Vec<Option<Value>> = mutant.states[at].to_vec();
            values[i] = Some(replacement);
            mutant.states[at] = values.into();
            mutant.labels[at] = None;
            if at + 1 < len {
                mutant.labels[at + 1] = None;
            }
            if !matches!(
                Run::new(model, Some(replay)).validate(&mutant),
                Validation::Invalid(_)
            ) {
                continue;
            }
            out.push(Mutant {
                name: format!("redraw-{at}-{}", vars[i]),
                trace: mutant,
            });
        }
    }
    out
}

/// Labels the first step it can with another of the trace's action names
/// that is false on that step.
fn relabel(model: &Model, trace: &Trace) -> Option<Mutant> {
    let ev = &model.evaluator;
    let mut names: Vec<&str> = trace
        .labels
        .iter()
        .flatten()
        .map(|l| l.name.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    let checkable = |name: &str| {
        model
            .ir
            .root_def(name)
            .map(|d| ev.redirected(d))
            .filter(|&d| model.ir.def(d).params.is_empty() && !ev.is_value_override(d))
    };
    for k in 1..trace.states.len() {
        let Some(current) = &trace.labels[k] else {
            continue;
        };
        let step = dive_eval::St::action(trace.states[k - 1].clone(), trace.states[k].clone());
        for other in &names {
            if *other == current.name {
                continue;
            }
            if let Some(d) = checkable(other)
                && matches!(ev.eval_def(d, &step), Ok(Value::Bool(false)))
            {
                let mut t = trace.clone();
                t.labels[k] = Some(crate::format::Label {
                    name: (*other).to_owned(),
                    location: None,
                });
                return Some(Mutant {
                    name: format!("relabel-{k}-{other}"),
                    trace: t,
                });
            }
        }
    }
    None
}
