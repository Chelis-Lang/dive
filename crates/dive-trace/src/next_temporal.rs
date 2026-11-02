//! Independent primitive Next errors before a second temporal row completes.

use dive_eval::{ErrorClass, EvalError, ExceptionKind, HostControl};
use dive_model::Model;

use crate::ordered::{Driver, Event, Site};
use crate::{Check, Failure, NextStateEndpoint, State, Validation};

pub(crate) fn needed(model: &Model) -> bool {
    model
        .unsupported
        .iter()
        .any(|feature| feature.kind == dive_model::UnsupportedKind::TemporalProperty)
        || !model.temporals.is_empty()
        || !model.implied_temporals.is_empty()
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one fresh runtime owns both source reconstruction and terminal diagnostic execution"
)]
pub(crate) fn replay(
    model: &Model,
    claimed: &State,
    state_index: usize,
    ec: &str,
    endpoint: &NextStateEndpoint,
    host: &crate::validate::TraceHost,
    control: Option<&dyn HostControl>,
) -> Result<Validation, EvalError> {
    let refuse = |message: &str| Validation::Unsupported(message.into());
    if state_index != 1
        || endpoint.ordered_codes().is_none()
        || model.implied_temporals.is_empty()
        || !model.temporals.is_empty()
        || model.view.is_some()
        || model.symmetry.is_some()
        || model.alias.is_some()
        || !model.constraints.is_empty()
        || !model.action_constraints.is_empty()
        || !model.invariants.is_empty()
        || !model.implied_inits.is_empty()
        || !model.implied_actions.is_empty()
        || model
            .unsupported
            .iter()
            .any(|feature| feature.kind != dive_model::UnsupportedKind::TemporalProperty)
    {
        return Ok(refuse(
            "temporal Next error proof requires an ordinary two-source recurrence prefix",
        ));
    }
    for action in model.init_actions.iter().chain(&model.actions) {
        if let Some(control) = control {
            control.check().map_err(EvalError::interrupted)?;
        }
        if model
            .ir
            .node(action.expr())
            .effects
            .intersects(dive_ir::Effects::IMPURE)
            || model.evaluator.action_shares_side_effects(action)
            || action.scope().has_substitutions()
        {
            return Ok(refuse("temporal Next prefix has effects or dynamic scopes"));
        }
    }
    let initial = Driver {
        initial: true,
        model,
        source: claimed,
        level: 0,
        host,
        control,
    };
    let first = match initial.sole_source()? {
        Ok(source) => source,
        Err(event) => return Ok(event_result(event, 0)),
    };
    if !recurrences(model, &first, true) {
        return Ok(refuse(
            "temporal Next prefix requires total state recurrences true on its completed row",
        ));
    }
    let prior = Driver {
        initial: false,
        source: &first,
        level: 1,
        ..initial
    };
    let second = match prior.sole_source()? {
        Ok(source) => source,
        Err(event) => return Ok(event_result(event, 0)),
    };
    if second == first || &second != claimed {
        return Ok(invalid(
            "claimed terminal row is not the sole newly admitted successor",
        ));
    }
    if !recurrences(model, &second, false) {
        return Ok(refuse(
            "temporal Next terminal recurrence atom is not total",
        ));
    }
    // The sole completed source satisfies each recurrence. The next source
    // has no completed row or implicit stutter: its generator must fail before
    // any emission. No temporal accepting cycle or atom error precedes it.
    let terminal = Driver {
        initial: false,
        source: &second,
        level: 2,
        ..initial
    };
    let primary = match terminal.primary(true)? {
        Event::Error {
            site: Site::Next,
            error,
            ..
        } => error,
        event => return Ok(event_result(event, 1)),
    };
    if !primitive(&primary) {
        return Ok(refuse(
            "temporal Next primary constructor/lifetime is not proved",
        ));
    }
    if primary.ec().name() != ec || !endpoint.matches_error_source(&model.ir, &primary) {
        return Ok(invalid(
            "actual temporal Next cause/source differs from the claim",
        ));
    }
    let diagnostic = match terminal.diagnostic()? {
        Event::Error {
            site: Site::Next,
            error,
            ..
        } => error,
        event => return Ok(event_result(event, 1)),
    };
    if !primitive(&diagnostic)
        || diagnostic.origin() != primary.origin()
        || !endpoint.matches_error_source(&model.ir, &diagnostic)
    {
        return Ok(refuse(
            "temporal Next diagnostic constructor/source differs",
        ));
    }
    terminal.poll()?;
    Ok(if endpoint.ordered_codes() == Some((2179, 2179)) {
        Validation::Valid
    } else {
        invalid("temporal Next reported/deciding codes differ from actual execution")
    })
}

fn primitive(error: &EvalError) -> bool {
    error.class() == ErrorClass::Eval
        && error.ec().code() == 2179
        && error.exception_kind() == Some(ExceptionKind::EvalException)
        && error.value_method().is_none()
        && error.origin().is_some()
}

fn recurrences(model: &Model, state: &State, must_hold: bool) -> bool {
    model.implied_temporals.iter().all(|property| {
        let pred = &property.pred;
        if pred.scope.has_substitutions() || model.evaluator.scope_has_effects(&pred.scope) {
            return false;
        }
        let mut expression = pred.expr;
        for _ in 0..16 {
            let definition = match model.ir.expr(expression) {
                dive_ir::Expr::Def(definition) => *definition,
                dive_ir::Expr::Apply {
                    callee: dive_ir::OpRef::Def(definition),
                    args,
                } if args.is_empty() => *definition,
                _ => break,
            };
            if model.evaluator.redirected(definition) != definition
                || model.evaluator.is_value_override(definition)
            {
                return false;
            }
            let definition = model.ir.def(definition);
            if !definition.params.is_empty() || definition.recursive || definition.std.is_some() {
                return false;
            }
            expression = definition.body;
        }
        let dive_ir::Expr::Always(eventually) = model.ir.expr(expression) else {
            return false;
        };
        let dive_ir::Expr::Eventually(atom) = model.ir.expr(*eventually) else {
            return false;
        };
        let dive_ir::Expr::Builtin {
            op: dive_ir::BuiltinOp::Eq,
            args,
        } = model.ir.expr(*atom)
        else {
            return false;
        };
        if args.len() != 2 {
            return false;
        }
        let (variable, literal) = match (model.ir.expr(args[0]), model.ir.expr(args[1])) {
            (dive_ir::Expr::Var { var, primed: false }, dive_ir::Expr::Num(literal))
            | (dive_ir::Expr::Num(literal), dive_ir::Expr::Var { var, primed: false }) => {
                (*var, literal)
            }
            _ => return false,
        };
        let Ok(literal) = literal.parse::<i32>() else {
            return false;
        };
        matches!(state.get(variable.index()), Some(Some(dive_value::Value::Int(value)))
            if !must_hold || *value == i64::from(literal))
    })
}

fn invalid(detail: &str) -> Validation {
    Validation::Invalid(Failure {
        state: 1,
        check: Check::Ending,
        detail: detail.into(),
    })
}

fn event_result(event: Event, state: usize) -> Validation {
    match event {
        Event::Refused(reason) => Validation::Unsupported(reason),
        Event::Failed { check, detail } => Validation::Invalid(Failure {
            state,
            check,
            detail,
        }),
        _ => Validation::Invalid(Failure {
            state,
            check: Check::Ending,
            detail: "actual temporal prefix or terminal execution differs from the claim".into(),
        }),
    }
}
