//! Bounded checks on actual occurrences before using a stored-state quotient.
//! This does not search a second state graph or certify arbitrary pure programs.

use dive_check::{
    CheckOptions,
    capture::{BehaviorGraph, ConcreteSuccessor},
};
use dive_eval::{
    EvalError, St,
    next::{Flow, StateVals},
};
use dive_ir::Effects;
use dive_model::{Model, Pred};
use dive_value::{Value, codec};

use crate::{
    analyze::{self, AnalysisError, Limits},
    formula::AtomKind,
    plan::{self, Plan},
    product::Labels,
};

struct Work {
    left: usize,
    generated: u64,
    generated_limit: u64,
}
impl Work {
    fn charge(&mut self, n: usize) -> Result<(), AnalysisError> {
        self.left = self.left.checked_sub(n).ok_or(AnalysisError::Limit(
            "concrete coherence cells/transport work",
        ))?;
        Ok(())
    }
}

fn refusal(model: &Model, pred: &Pred, property: &str, detail: &str) -> AnalysisError {
    AnalysisError::Concrete {
        property: property.into(),
        source: Box::new(plan::Source::new(model, pred.location)),
        detail: detail.into(),
    }
}
fn evaluation(model: &Model, pred: &Pred, property: &str, error: EvalError) -> AnalysisError {
    AnalysisError::Atom {
        property: property.into(),
        source: Box::new(plan::Source::new(model, pred.location)),
        detail: error.to_string(),
        error: Box::new(error),
    }
}

/// Compares actual lossless representations, without treating extensional Eq
/// or constructor flags as a semantic certificate. Deferred transport refuses.
fn same(
    model: &Model,
    pred: &Pred,
    property: &str,
    a: &StateVals,
    b: &StateVals,
    work: &mut Work,
) -> Result<bool, AnalysisError> {
    work.charge(1)?;
    if std::sync::Arc::ptr_eq(a, b) {
        return Ok(true);
    }
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b.iter()) {
        let (Some(a), Some(b)) = (a, b) else {
            return Err(refusal(
                model,
                pred,
                property,
                "incomplete concrete occurrence",
            ));
        };
        work.charge(1)?;
        match (a, b) {
            (Value::Fcn(a), Value::Fcn(b)) if a.ptr_eq(b) => continue,
            (Value::Set(a), Value::Set(b)) if a.ptr_eq(b) => continue,
            (Value::Fcn(_) | Value::Set(_), _) | (_, Value::Fcn(_) | Value::Set(_)) => {}
            _ if a == b => continue,
            _ => return Ok(false),
        }
        let mut encoded = [Vec::new(), Vec::new()];
        for (value, bytes) in [a, b].into_iter().zip(&mut encoded) {
            let bound = codec::frontier_size_bound(value, work.left).map_err(|error| {
                if error.kind() == dive_value::ErrorKind::DiveLimit {
                    AnalysisError::Limit("concrete lossless transport work")
                } else {
                    refusal(
                        model,
                        pred,
                        property,
                        "concrete alias transport requires deferred evaluation",
                    )
                }
            })?;
            work.charge(bound)?;
            codec::encode(value, model.evaluator.options().ctx, bytes).map_err(|_| {
                refusal(
                    model,
                    pred,
                    property,
                    "concrete alias lossless transport unavailable",
                )
            })?;
        }
        if encoded[0] != encoded[1] {
            return Ok(false);
        }
    }
    Ok(true)
}

fn target(graph: &BehaviorGraph, from: usize, edge: usize) -> Result<&StateVals, AnalysisError> {
    let e = &graph.edges[from][edge];
    if e.ordinal.is_none() {
        return Ok(&graph.states[from]);
    }
    graph.concrete[from]
        .get(edge)
        .map(|s| &s.state)
        .ok_or(AnalysisError::Inconsistent(
            "missing original generated target",
        ))
}

fn shape(graph: &BehaviorGraph) -> Result<(), AnalysisError> {
    let n = graph.states.len();
    if graph.edges.len() != n || graph.concrete.len() != n {
        return Err(AnalysisError::Inconsistent("concrete graph row dimensions"));
    }
    for (from, row) in graph.edges.iter().enumerate() {
        // Empty rows are unexpanded prefix frontier, never certified complete.
        if row.is_empty() {
            if !graph.concrete[from].is_empty() {
                return Err(AnalysisError::Inconsistent("unexpanded concrete row"));
            }
            continue;
        }
        if row.len() != graph.concrete[from].len().saturating_add(1)
            || row
                .last()
                .is_none_or(|e| e.to != from || e.ordinal.is_some() || e.label.is_some())
        {
            return Err(AnalysisError::Inconsistent("concrete row/stutter shape"));
        }
        for (edge, original) in row.iter().zip(&graph.concrete[from]) {
            if edge.to >= n
                || edge.ordinal != Some(original.ordinal)
                || edge.label.as_deref() != Some(original.label.as_str())
            {
                return Err(AnalysisError::Inconsistent(
                    "concrete row/ordinal/label alignment",
                ));
            }
        }
    }
    if graph
        .initials
        .iter()
        .any(|i| i.to >= n || graph.roots.binary_search(&i.to).is_err())
        || graph.roots.iter().any(|r| *r >= n)
    {
        return Err(AnalysisError::Inconsistent("concrete initial identities"));
    }
    Ok(())
}

fn variants(
    model: &Model,
    graph: &BehaviorGraph,
    pred: &Pred,
    property: &str,
    work: &mut Work,
) -> Result<Vec<Vec<StateVals>>, AnalysisError> {
    let mut variants: Vec<Vec<_>> = graph.states.iter().map(|s| vec![s.clone()]).collect();
    let incoming = graph.initials.iter().map(|i| (i.to, &i.state)).chain(
        graph
            .edges
            .iter()
            .zip(&graph.concrete)
            .flat_map(|(edges, originals)| {
                edges.iter().zip(originals).map(|(e, o)| (e.to, &o.state))
            }),
    );
    for (to, state) in incoming {
        let mut duplicate = false;
        for other in &variants[to] {
            if same(model, pred, property, state, other, work)? {
                duplicate = true;
                break;
            }
        }
        if !duplicate {
            work.charge(1)?;
            variants[to].push(state.clone());
        }
    }
    Ok(variants)
}

fn replay_admitted(model: &Model, pred: &Pred, property: &str) -> Result<(), AnalysisError> {
    // This guard applies only when an actual occurrence requires replay. It
    // grants no host authority and uses existing bound context/effect checks.
    let forbidden = Effects::IMPURE | Effects::LIVE_TLCGET;
    let unsafe_pred = |p: &Pred| {
        model.ir.node(p.expr).effects.intersects(forbidden)
            || model.evaluator.scope_has_effects(&p.scope)
    };
    if model.runtime_name_order
        || !model.sequential.is_empty()
        || model.next.as_ref().is_some_and(unsafe_pred)
        || model
            .constraints
            .iter()
            .chain(&model.action_constraints)
            .any(|c| unsafe_pred(&c.pred))
        || model.actions.iter().any(|a| {
            model.ir.node(a.expr()).effects.intersects(forbidden)
                || model.evaluator.scope_has_effects(&a.scope())
        })
    {
        return Err(refusal(
            model,
            pred,
            property,
            "actual alias replay with effects, runtime names or action composition",
        ));
    }
    Ok(())
}

fn holds(
    model: &Model,
    pred: &Pred,
    st: &St,
    property: &str,
    work: &mut Work,
) -> Result<bool, AnalysisError> {
    work.charge(1)?;
    match model
        .eval(pred, st)
        .map_err(|e| evaluation(model, pred, property, e))?
    {
        Value::Bool(b) => Ok(b),
        _ => Err(evaluation(
            model,
            pred,
            property,
            EvalError::general("concrete predicate is not boolean"),
        )),
    }
}

fn accepted(
    model: &Model,
    from: &StateVals,
    to: &StateVals,
    property: &str,
    work: &mut Work,
) -> Result<bool, AnalysisError> {
    for c in &model.constraints {
        if !holds(model, &c.pred, &St::state(to.clone()), property, work)? {
            return Ok(false);
        }
    }
    for c in &model.action_constraints {
        if !holds(
            model,
            &c.pred,
            &St::action(from.clone(), to.clone()),
            property,
            work,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(
    clippy::too_many_arguments,
    reason = "actual source/options and shared work budget are explicit"
)]
fn replay(
    model: &Model,
    graph: &BehaviorGraph,
    from: usize,
    alias: &StateVals,
    options: &CheckOptions,
    property: &str,
    limits: Limits,
    work: &mut Work,
) -> Result<(), AnalysisError> {
    let next = model
        .next
        .as_ref()
        .ok_or(AnalysisError::Inconsistent("alias source without Next"))?;
    replay_admitted(model, next, property)?;
    model
        .evaluator
        .check_finite_bindings(
            &next.scope,
            next.expr,
            &St::state(alias.clone()),
            usize::try_from(limits.capture.successors_per_unit)
                .map_err(|_| AnalysisError::Limit("alias binding bound"))?,
        )
        .map_err(|e| evaluation(model, next, property, e))?;
    let mut row: Vec<ConcreteSuccessor> = Vec::new();
    let mut failure = None;
    let result = model.evaluator.successors(&model.actions, alias, &mut |s| {
        let result = (|| {
            work.generated = work
                .generated
                .checked_add(1)
                .filter(|n| *n <= work.generated_limit)
                .ok_or(AnalysisError::Limit("alias replay generated work"))?;
            work.charge(1)?;
            if s.ordinal >= limits.capture.successors_per_unit {
                return Err(AnalysisError::Limit("alias replay successors per source"));
            }
            if !s.complete() {
                return Err(refusal(
                    model,
                    next,
                    property,
                    "alias Next emitted an incomplete state",
                ));
            }
            let state = s.state.into();
            if accepted(model, alias, &state, property, work)? {
                let charge =
                    dive_check::capture::prepare_replay_identity(model, options, &state, work.left)
                        .map_err(|error| evaluation(model, next, property, error))?;
                work.charge(charge)?;
                row.push(ConcreteSuccessor {
                    state,
                    ordinal: s.ordinal,
                    label: s.action.label().into(),
                });
            }
            Ok(())
        })();
        match result {
            Ok(()) => Flow::Continue,
            Err(error) => {
                failure = Some(error);
                Flow::Stop
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    result.map_err(|e| evaluation(model, next, property, e.error))?;
    let captured = &graph.concrete[from];
    if row.len() != captured.len() {
        return Err(refusal(
            model,
            next,
            property,
            "alias Next/constraints changed the accepted successor row",
        ));
    }
    for (actual, original) in row.iter().zip(captured) {
        if actual.ordinal != original.ordinal
            || actual.label != original.label
            || !same(model, next, property, &actual.state, &original.state, work)?
        {
            return Err(refusal(
                model,
                next,
                property,
                "alias Next/constraints changed an accepted concrete occurrence",
            ));
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "ordered original evaluations followed by bounded coherence checks"
)]
pub(crate) fn labels(
    model: &Model,
    plan: &Plan,
    graph: &BehaviorGraph,
    options: &CheckOptions,
    property: &str,
    limits: Limits,
) -> Result<Labels, AnalysisError> {
    shape(graph)?;
    let edges = graph
        .edges
        .iter()
        .try_fold(0usize, |n, r| n.checked_add(r.len()))
        .ok_or(AnalysisError::Limit("atom cells"))?;
    let base = graph
        .states
        .len()
        .checked_add(edges)
        .and_then(|n| n.checked_mul(plan.atoms.len()))
        .ok_or(AnalysisError::Limit("atom cells"))?;
    let mut work = Work {
        left: limits.atom_cells,
        generated: 0,
        generated_limit: limits.capture.generated,
    };
    work.charge(base)?;
    let mut labels = Labels {
        states: Vec::new(),
        actions: Vec::new(),
    };
    // Original targets first: a typed error must not be replaced by a quotient
    // refusal or by a successful representative evaluation.
    for (index, pred) in plan.atoms.iter().enumerate() {
        let mut states = Vec::new();
        let mut actions = Vec::new();
        for (v, state) in graph.states.iter().enumerate() {
            states.push(if pred.kind == AtomKind::State {
                analyze::atom(model, plan, index, &St::state(state.clone()), property)?
            } else {
                false
            });
            let mut row = Vec::new();
            for e in 0..graph.edges[v].len() {
                row.push(if pred.kind == AtomKind::Action {
                    analyze::atom(
                        model,
                        plan,
                        index,
                        &St::action(state.clone(), target(graph, v, e)?.clone()),
                        property,
                    )?
                } else {
                    false
                });
            }
            actions.push(row);
        }
        labels.states.push(states);
        labels.actions.push(actions);
    }
    let origin = plan
        .atoms
        .first()
        .map(|atom| &atom.pred)
        .or(model.next.as_ref())
        .ok_or(AnalysisError::Inconsistent(
            "concrete graph has no predicate origin",
        ))?;
    let variants = variants(model, graph, origin, property, &mut work)?;
    let mut mismatch = None;
    for (index, pred) in plan.atoms.iter().enumerate() {
        for (v, sources) in variants.iter().enumerate() {
            for source in sources {
                if pred.kind == AtomKind::State {
                    work.charge(1)?;
                    if analyze::atom(model, plan, index, &St::state(source.clone()), property)?
                        != labels.states[index][v]
                    {
                        mismatch = Some(refusal(
                            model,
                            &pred.pred,
                            property,
                            "state atom differs on actual equal-identity occurrences",
                        ));
                    }
                } else {
                    for (e, edge) in graph.edges[v].iter().enumerate() {
                        let to = if edge.ordinal.is_none() {
                            source
                        } else {
                            target(graph, v, e)?
                        };
                        work.charge(1)?;
                        if analyze::atom(
                            model,
                            plan,
                            index,
                            &St::action(source.clone(), to.clone()),
                            property,
                        )? != labels.actions[index][v][e]
                        {
                            mismatch = Some(refusal(
                                model,
                                &pred.pred,
                                property,
                                "action atom differs on actual source occurrences",
                            ));
                        }
                        work.charge(1)?;
                        if analyze::atom(
                            model,
                            plan,
                            index,
                            &St::action(graph.states[v].clone(), graph.states[edge.to].clone()),
                            property,
                        )? != labels.actions[index][v][e]
                        {
                            mismatch = Some(refusal(
                                model,
                                &pred.pred,
                                property,
                                "action atom differs on representative word projection",
                            ));
                        }
                    }
                }
            }
        }
    }
    if let Some(error) = mismatch {
        return Err(error);
    }
    for (v, sources) in variants.iter().enumerate() {
        if graph.edges[v].is_empty() {
            continue;
        }
        for source in sources.iter().skip(1) {
            replay(
                model, graph, v, source, options, property, limits, &mut work,
            )?;
        }
        for (e, edge) in graph.edges[v].iter().enumerate() {
            if edge.ordinal.is_none()
                || same(
                    model,
                    origin,
                    property,
                    target(graph, v, e)?,
                    &graph.states[edge.to],
                    &mut work,
                )?
            {
                continue;
            }
            let next = model
                .next
                .as_ref()
                .ok_or(AnalysisError::Inconsistent("projected edge without Next"))?;
            replay_admitted(model, next, property)?;
            let st = St::action(graph.states[v].clone(), graph.states[edge.to].clone());
            if !holds(model, next, &st, property, &mut work)?
                || !accepted(
                    model,
                    &graph.states[v],
                    &graph.states[edge.to],
                    property,
                    &mut work,
                )?
            {
                return Err(refusal(
                    model,
                    next,
                    property,
                    "original Next/constraints reject representative word projection",
                ));
            }
        }
    }
    Ok(labels)
}
