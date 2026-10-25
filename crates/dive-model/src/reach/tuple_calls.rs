//! Complete actual checking roots for the optional constructor-result proof.

use dive_eval::{EvalError, TupleConcatSources};
use dive_ir::DefId;

use super::{Binder, Look, Model, role_preds};
use crate::bind::R;
use crate::error::BindError;

fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), EvalError> {
    values
        .try_reserve(1)
        .map_err(|_| EvalError::resource_limit("runtime name roots allocation"))?;
    values.push(value);
    Ok(())
}

pub(super) fn prove(b: &Binder<'_>, model: &Model) -> R<Option<TupleConcatSources>> {
    inspect(b, model)
        .map_err(|error| b.eval_error(error, |e| BindError::unsupported(e.to_string())))
}

fn inspect(b: &Binder<'_>, model: &Model) -> Result<Option<TupleConcatSources>, EvalError> {
    let mut roots = Vec::new();
    for pred in role_preds(model) {
        push(&mut roots, (pred.expr, pred.scope.clone()))?;
    }
    for action in model.init_actions.iter().chain(&model.actions) {
        push(&mut roots, (action.expr(), action.scope()))?;
    }
    for named in model.temporals.iter().chain(model.property_claims()) {
        push(&mut roots, (named.pred.expr, named.pred.scope.clone()))?;
    }
    if let Some(alias) = &model.alias {
        let Ok(alias) = alias else {
            return Ok(None);
        };
        push(&mut roots, (alias.pred.expr, alias.pred.scope.clone()))?;
    }
    let Ok(postconditions) = &model.postconditions else {
        return Ok(None);
    };
    for named in postconditions {
        push(&mut roots, (named.pred.expr, named.pred.scope.clone()))?;
    }
    let mut definitions: Vec<DefId> = Vec::new();
    for &def in &model.assumptions {
        push(&mut definitions, def)?;
    }
    // These are actual bare binding roots, unlike a native definition reached
    // only because an Apply names it. A configured native loses its callsite
    // provenance and remains conservative even if other calls are tuples.
    for value in b.t.consts.iter().flatten() {
        if let Look::Def(def) | Look::Java(_, def) = value {
            push(&mut definitions, *def)?;
        }
    }
    for map in [&b.t.tool_obj, &b.t.body_obj] {
        for value in map.values() {
            if let Look::Def(def) | Look::Java(_, def) = value {
                push(&mut definitions, *def)?;
            }
        }
    }
    b.ev()
        .tuple_concat_sources(&roots, &definitions, b.control.as_deref())
}
