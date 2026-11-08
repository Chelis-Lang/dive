//! Private row consumer; incomplete certificates never install a callback.

use dive_eval::EvalError;
use dive_model::Model;
use dive_runtime::CheckOptions;

use super::PreparedProperty;
use crate::formula::AtomKind;

mod source;
pub use source::SourceRowDriver;
pub(super) use source::prepare_source;
pub(in crate::analyze) use source::source_possible;

/// Whether a row program can apply to these plans: one worker, uncapped
/// labels and only state-predicate atoms. An owned model whose producers
/// are stable is further required; this is the part that needs no model.
pub(super) fn may_apply(
    plans: &[PreparedProperty],
    options: &CheckOptions,
    graph_labels: super::GraphLabelPolicy,
) -> bool {
    matches!(graph_labels, super::GraphLabelPolicy::NoCellLimit)
        && options.workers == 1
        && plans.iter().flat_map(|p| &p.plan.atoms).all(|atom| {
            atom.kind == AtomKind::State && matches!(atom.test, crate::plan::AtomTest::Predicate)
        })
}

// No new evaluations: inspect every actual producer/retained scope, including
// safety/constraint work which can share values with a later predicate.
fn stable_producers(
    model: &Model,
    plans: &[PreparedProperty],
    control: &dive_runtime::budget::EvaluationControl,
) -> Result<Option<dive_eval::ProducerStability>, EvalError> {
    let mut producers = Vec::new();
    for init in &model.init {
        root(&mut producers, init.pred.expr, init.pred.scope.clone())?;
    }
    if let Some(next) = &model.next {
        root(&mut producers, next.expr, next.scope.clone())?;
    }
    for action in model.init_actions.iter().chain(&model.actions) {
        root(&mut producers, action.expr(), action.scope())?;
    }
    for named in model
        .invariants
        .iter()
        .chain(&model.implied_inits)
        .chain(&model.implied_actions)
        .chain(&model.constraints)
        .chain(&model.action_constraints)
        .chain(model.view.iter())
    {
        root(&mut producers, named.pred.expr, named.pred.scope.clone())?;
    }
    if let Some(alias) = &model.alias {
        let Ok(alias) = alias.as_ref() else {
            return Ok(None);
        };
        root(&mut producers, alias.pred.expr, alias.pred.scope.clone())?;
    }
    let Ok(postconditions) = &model.postconditions else {
        return Ok(None);
    };
    for named in postconditions {
        root(&mut producers, named.pred.expr, named.pred.scope.clone())?;
    }
    for &def in &model.assumptions {
        root(
            &mut producers,
            model.ir.def(def).body,
            model.evaluator.scope(def)?,
        )?;
    }
    let mut predicates = Vec::new();
    for atom in plans.iter().flat_map(|prepared| &prepared.plan.atoms) {
        let expression = match atom.test {
            crate::plan::AtomTest::NegatedEnabled { action } => action,
            _ => atom.pred.expr,
        };
        root(&mut predicates, expression, atom.pred.scope.clone())?;
        if let crate::plan::AtomTest::Enabled { sub: Some(sub) }
        | crate::plan::AtomTest::Nonstuttering { sub } = atom.test
        {
            root(&mut predicates, sub, atom.pred.scope.clone())?;
        }
    }
    let proof = model.evaluator.canonical_producer_stability_budgeted(
        &producers,
        &predicates,
        Some(control),
    )?;
    Ok(proof.filter(|proof| proof.belongs_to(&model.evaluator)))
}
fn root(
    roots: &mut Vec<(dive_ir::ExprId, dive_eval::Scope)>,
    expr: dive_ir::ExprId,
    scope: dive_eval::Scope,
) -> Result<(), EvalError> {
    roots
        .try_reserve(1)
        .map_err(|_| EvalError::resource_limit("canonical producer roots allocation"))?;
    roots.push((expr, scope));
    Ok(())
}

struct Failed {
    global: usize,
    cause: EvalError,
    core: bool,
}
