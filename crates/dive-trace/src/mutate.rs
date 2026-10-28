//! Negative controls: mutants of a valid counterexample that are invalid by
//! construction (ADR 0016 §3).
//!
//! The caller selects [`ControlMode`] explicitly. Dropping and swapping
//! states require a valid shortest breadth-first counterexample (ADR 0015):
//! state `k` is at distance exactly `k` from the initial states, and no two
//! states are equal. Simulation, multi-worker and liveness words have no
//! such guarantee; [`ControlMode::Counterexample`] omits these controls.
//! The remaining controls use the valid counterexample's first-violation,
//! constrained-variable and label obligations, without a distance premise.
//!
//! | Mutant | Why it is invalid |
//! |---|---|
//! | `drop-state-k`: remove a middle state | if `s(k-1) -> s(k+1)` were a step, `s(k+1)` would be at distance `k`, not `k+1` |
//! | `drop-first`: remove the initial state | if `s1` were initial, it would be at distance 0 |
//! | `swap-k`: exchange states `k` and `k+1` | the step into the earlier position would put `s(k+1)` at distance `k` (or make it initial) |
//! | `change-k-var`: one variable of a middle state becomes a model value with a reserved Boolean keyword | the step into the state would have to set the variable to a value no expression of the specification can produce |
//! | `truncate`: remove the last state, keep the claim (invariant, named invariant-error and property endings) | the claimed invariant or property held, or did not raise the claimed error, at the new last state, or the search would have stopped there |
//! | `claim-Q`: claim a different invariant or property `Q` | `Q` either holds at the end, or is listed after the violated one, which is then false first |
//! | `claim-error`: change a named invariant error's cause | the exact cause differs from the known valid claim |
//! | `relabel-k`: label step `k` with another action of the trace that is false on it | the label must hold on the step |
//!
//! Labels on the steps a mutation touches are removed, so a mutant is
//! rejected for its states, not its labels. The `change` mutants assume
//! every variable is constrained by the next-state relation, as TLC's
//! successor generator requires. The changed value is a model value named `TRUE`: the reserved
//! Boolean keyword cannot denote a model value in a specification or
//! configuration. Its atom already belongs to the startup name table, so
//! generating a control never interns a runtime name.

use dive_model::Model;
use dive_value::Value;

use crate::format::Ending;
use crate::trace::Trace;

/// The model value the `change` mutants use. The reserved Boolean keyword
/// cannot denote a model value in a specification or configuration, and its
/// name is already interned at startup.
pub const MUTANT_MODEL_VALUE: &str = "TRUE";

/// Which invalid-by-construction controls the caller's counterexample supports.
///
/// This choice declares the producer's contract; validation does not infer
/// shortestness from a supplied word or search for a shorter path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlMode {
    /// A known shortest breadth-first counterexample. Includes dropping and
    /// swapping states, whose invalidity relies on shortest distances.
    ShortestBfs,
    /// A known valid counterexample without a shortest-path guarantee.
    /// Includes changed values, truncated claims, changed claims and false
    /// action labels, subject to their existing construction preconditions.
    Counterexample,
}

/// A mutant: what was done, and the trace.
#[derive(Clone, Debug)]
pub struct Mutant {
    /// What was done, e.g. `swap-3`.
    pub name: String,
    /// The mutated trace.
    pub trace: Trace,
}

/// The by-construction mutants of a known valid shortest BFS `trace`.
///
/// Compatibility wrapper for [`mutants_with_mode`] with
/// [`ControlMode::ShortestBfs`]. Use the explicit mode for other producers.
#[must_use]
pub fn mutants(model: &Model, trace: &Trace) -> Vec<Mutant> {
    mutants_with_mode(model, trace, ControlMode::ShortestBfs)
}

/// The by-construction mutants supported by a known valid counterexample's
/// producer contract (see the module documentation).
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the table in the module documentation, one block per row"
)]
pub fn mutants_with_mode(model: &Model, trace: &Trace, mode: ControlMode) -> Vec<Mutant> {
    let n = trace.states.len();
    let mut out = Vec::new();
    let base = Trace {
        length: None,
        ..trace.clone()
    };
    let mut push = |name: String, t: Trace| out.push(Mutant { name, trace: t });
    if mode == ControlMode::ShortestBfs && n >= 3 {
        let k = n / 2;
        let mut t = base.clone();
        t.states.remove(k);
        t.labels.remove(k);
        t.labels[k] = None;
        push(format!("drop-state-{k}"), t);
    }
    if mode == ControlMode::ShortestBfs && n >= 2 {
        let mut t = base.clone();
        t.states.remove(0);
        t.labels.remove(0);
        t.labels[0] = None;
        push("drop-first".to_owned(), t);
    }
    if mode == ControlMode::ShortestBfs && n >= 2 {
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
    if n != 0
        && let Some(atom) = model.names.get(MUTANT_MODEL_VALUE)
        && (atom.tok() as usize) < model.evaluator.startup_names()
    {
        let k = if n >= 3 { n / 2 } else { 0 };
        let vars = crate::trace::variables(model);
        let fresh = Value::Model(atom);
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
    // Violations and named invariant errors must first arise at the original
    // end. Generic expansion-error and liveness endings retain their existing
    // validation boundary (ADR 0016, amendment 1).
    if n >= 2
        && matches!(
            trace.ending,
            Ending::Invariant(_) | Ending::Property(_) | Ending::InvariantError { .. }
        )
    {
        let mut t = base.clone();
        t.states.pop();
        t.labels.pop();
        push("truncate".to_owned(), t);
    }
    let names =
        |list: &[dive_model::Named]| list.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
    let claims: Vec<Ending> = match &trace.ending {
        Ending::InvariantError { name, ec } => names(&model.invariants)
            .into_iter()
            .filter(|other| other != name)
            .chain(std::iter::once(format!("{name}_dive_trace_mutant")))
            .map(|name| Ending::InvariantError {
                name,
                ec: ec.clone(),
            })
            .chain(std::iter::once(Ending::InvariantError {
                name: name.clone(),
                ec: if ec == "TLC_MODULE_DIVISION_BY_ZERO" {
                    "TLC_VALUE_ASSERT_FAILED"
                } else {
                    "TLC_MODULE_DIVISION_BY_ZERO"
                }
                .to_owned(),
            }))
            .collect(),
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
            .filter(|&d| {
                model.ir.def(d).params.is_empty()
                    && !ev.is_value_override(d)
                    && !model.ir.def(d).effects.intersects(dive_ir::Effects::IMPURE)
            })
    };
    for k in 1..trace.states.len() {
        let Some(current) = &trace.labels[k] else {
            continue;
        };
        for other in &names {
            if *other == current.name {
                continue;
            }
            if checkable(other).is_some() {
                let mut t = trace.clone();
                t.labels[k] = Some(crate::format::Label {
                    name: (*other).to_owned(),
                    location: None,
                });
                // Skip impure alternatives above, then use guarded validation
                // rather than evaluating a label in the caller's runtime.
                if matches!(crate::validate(model, &t).validation,
                    crate::Validation::Invalid(ref failure)
                        if failure.state == k && failure.check == crate::Check::Label)
                {
                    return Some(Mutant {
                        name: format!("relabel-{k}-{other}"),
                        trace: t,
                    });
                }
            }
        }
    }
    None
}
