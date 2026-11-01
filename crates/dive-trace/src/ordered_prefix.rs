//! Independent bounded source admission and actual diagnostic execution.

use dive_eval::{EvalError, ExceptionKind, HostControl};
use dive_model::Model;

use crate::ordered::{Driver, Event, History, Site};
use crate::validate::TraceHost;
use crate::{Check, Ending, Failure, State, Trace, Validation, ViewSourceEndpoint};

/// Independently executed successor invariant primary and diagnostic identities.
/// Only a fresh [`crate::Session::invariant_error_class`] constructs this value;
/// it is never serialized or accepted as producer evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantErrorClass {
    reported: u32,
    deciding: u32,
}
impl InvariantErrorClass {
    /// The observed primary invariant wrapper.
    #[must_use]
    pub fn reported_code(&self) -> u32 {
        self.reported
    }
    /// The deciding identity after actual diagnostic completion or exception.
    #[must_use]
    pub fn deciding_code(&self) -> u32 {
        self.deciding
    }
}

pub(crate) fn invariant(
    model: &Model,
    trace: &Trace,
    host: &TraceHost,
    control: Option<&dyn HostControl>,
) -> Result<(Validation, Option<InvariantErrorClass>), EvalError> {
    let Ending::InvariantError { name, ec } = &trace.ending else {
        return Ok((invalid("expected a named invariant error"), None));
    };
    let (validation, observed) = replay(
        model,
        trace,
        host,
        control,
        |site, error, role, candidate| {
            site == Site::Invariant
                && role == Some(name.as_str())
                && error.ec().name() == ec
                && candidate == trace.states.last()
        },
    )?;
    Ok((
        validation,
        observed.map(|deciding| InvariantErrorClass {
            reported: 2111,
            deciding,
        }),
    ))
}

pub(crate) fn view(
    model: &Model,
    trace: &Trace,
    ec: &str,
    endpoint: &ViewSourceEndpoint,
    host: &TraceHost,
    control: Option<&dyn HostControl>,
) -> Result<Validation, EvalError> {
    let (validation, deciding) = replay(model, trace, host, control, |site, error, _, _| {
        site == Site::View
            && error.ec().name() == ec
            && endpoint.matches_error(&model.ir, error)
            && reported(error, site) == Some(endpoint.reported_code())
    })?;
    if validation != Validation::Valid {
        return Ok(validation);
    }
    Ok(if deciding == Some(endpoint.deciding_code()) {
        Validation::Valid
    } else {
        invalid("VIEW diagnostic execution has a different deciding identity")
    })
}

fn preflight(model: &Model, invariant: bool) -> Option<String> {
    if let Some(feature) = model.unsupported.iter().find(|feature| {
        !(invariant && feature.kind == dive_model::UnsupportedKind::TemporalProperty)
    }) {
        return Some(format!(
            "ordered prefix unsupported model feature: {}",
            feature.feature
        ));
    }
    if model.symmetry.is_some()
        || model.alias.is_some()
        || !model.temporals.is_empty()
        || !model.implied_temporals.is_empty() && !invariant
    {
        return Some(
            "ordered prefix excludes unsupported roles, symmetry, ALIAS and temporal obligations"
                .into(),
        );
    }
    let impure = |pred: &dive_model::Pred| {
        (!invariant
            && model
                .ir
                .node(pred.expr)
                .effects
                .intersects(dive_ir::Effects::IMPURE))
            || model.evaluator.scope_has_effects(&pred.scope)
    };
    if model
        .init_actions
        .iter()
        .chain(&model.actions)
        .any(|action| {
            (!invariant
                && model
                    .ir
                    .node(action.expr())
                    .effects
                    .intersects(dive_ir::Effects::IMPURE))
                || model.evaluator.action_shares_side_effects(action)
        })
        || model
            .constraints
            .iter()
            .chain(&model.action_constraints)
            .chain(&model.invariants)
            .chain(&model.implied_inits)
            .chain(&model.implied_actions)
            .chain(&model.view)
            .any(|named| impure(&named.pred))
    {
        return Some("ordered prefix excludes effects and dynamically shared scopes".into());
    }
    None
}

fn simple_recurrences(model: &Model, initial: &State) -> bool {
    model.implied_temporals.iter().all(|property| {
        let pred = &property.pred;
        if pred.scope.has_substitutions() {
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
        let pair = (model.ir.expr(args[0]), model.ir.expr(args[1]));
        let (variable, literal) = match pair {
            (dive_ir::Expr::Var { var, primed: false }, dive_ir::Expr::Num(literal))
            | (dive_ir::Expr::Num(literal), dive_ir::Expr::Var { var, primed: false }) => {
                (*var, literal)
            }
            _ => return false,
        };
        // Planning is a single nonconstant state recurrence. Its only source
        // atom is total on the initial source, before this source's safety
        // error prevents any row completion or temporal analysis.
        literal.parse::<i32>().is_ok()
            && matches!(
                initial.get(variable.index()),
                Some(Some(dive_value::Value::Int(_)))
            )
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one finite replay retains its actual admission and diagnostic history"
)]
fn replay(
    model: &Model,
    trace: &Trace,
    host: &TraceHost,
    control: Option<&dyn HostControl>,
    matches: impl FnOnce(Site, &EvalError, Option<&str>, Option<&State>) -> bool,
) -> Result<(Validation, Option<u32>), EvalError> {
    let refuse = |reason| (Validation::Unsupported(reason), None);
    let invariant = matches!(trace.ending, Ending::InvariantError { .. });
    if let Some(reason) = preflight(model, invariant) {
        return Ok(refuse(reason));
    }
    let source_index = trace
        .states
        .len()
        .checked_sub(if invariant { 2 } else { 1 });
    let Some(source) = source_index.and_then(|index| trace.states.get(index)) else {
        return Ok(refuse(
            "ordered replay needs an initial and terminal source".into(),
        ));
    };
    let Some(initial) = trace.states.first() else {
        return Ok(refuse("ordered replay needs an initial state".into()));
    };
    if !model.implied_temporals.is_empty()
        && (source != initial || !simple_recurrences(model, initial))
    {
        return Ok(refuse("invariant temporal configuration requires a first-source safety error and pure state recurrences".into()));
    }
    let mut history = History::default();
    history.invariant = invariant;
    let init = Driver {
        initial: true,
        model,
        source: initial,
        level: 0,
        host,
        control,
    };
    let (event, emitted) = init.primary_history(false, Some(&mut history))?;
    if !matches!(event, Event::End) {
        return Ok((event_validation(event), None));
    }
    if if invariant {
        emitted == 0 || !history.admitted.contains(initial)
    } else {
        emitted != 1 || history.admitted.first() != Some(initial) || history.admitted.len() != 1
    } {
        return Ok(refuse(
            if invariant {
                "ordered invariant prefix needs exhausted Init and an admitted initial word state"
            } else {
                "ordered prefix needs exactly one exhausted, admitted Init emission"
            }
            .into(),
        ));
    }
    // The complete admission ledger is independently reconstructed. Word rows
    // do not assert global newness or omit off-word siblings from the replay.
    let mut index = 0;
    let (terminal_source, terminal_level) = loop {
        let Some(current) = history.admitted.get(index).cloned() else {
            return Ok((
                invalid("terminal source was not independently admitted"),
                None,
            ));
        };
        let level = history.levels[index];
        if &current == source {
            break (current, level);
        }
        let driver = Driver {
            initial: false,
            model,
            source: &current,
            level,
            host,
            control,
        };
        let (event, _) = driver.primary_history(false, Some(&mut history))?;
        if !matches!(event, Event::End) {
            return Ok((event_validation(event), None));
        }
        index += 1;
    };
    // Equality selects the actual admitted object, never the word decoder's
    // separately constructed receiver or its normalization lifetime.
    let terminal = Driver {
        initial: false,
        model,
        source: &terminal_source,
        level: terminal_level,
        host,
        control,
    };
    let (event, _) = terminal.primary_history(false, Some(&mut history))?;
    let Event::Error {
        site,
        error,
        candidate,
        name,
    } = event
    else {
        return Ok((event_validation(event), None));
    };
    if !matches(site, &error, name.as_deref(), candidate.as_ref()) {
        return Ok((
            invalid(
                "the first actual terminal error differs in site, role, cause, source or candidate",
            ),
            None,
        ));
    }
    // A primary invariant's value-method receiver does not decide its class:
    // insertion can make the diagnostic skip it entirely. Only an actually
    // repeated diagnostic receiver requires its separate lifetime proof.
    if !invariant && (error.value_method().is_some() || error.exception_kind().is_none()) {
        return Ok(refuse(
            "ordered prefix primary receiver/constructor lifetime is unproved".into(),
        ));
    }
    let diagnostic = terminal.diagnostic_history(&mut history)?;
    let error = match diagnostic {
        Event::Error { error, .. } => error,
        // TLC retains the original invariant wrapper when diagnostic execution
        // completes or quietly stops at a false predicate/incomplete candidate.
        Event::End
        | Event::Failed {
            check: Check::Invariant | Check::ImpliedAction,
            ..
        }
        | Event::Partial { .. }
            if invariant =>
        {
            terminal.poll()?;
            return Ok((Validation::Valid, Some(2111)));
        }
        other => return Ok((event_validation(other), None)),
    };
    if error.value_method().is_some() {
        return Ok(refuse(
            "ordered prefix diagnostic receiver lifetime is unproved".into(),
        ));
    }
    terminal.poll()?;
    let deciding = match error.exception_kind() {
        Some(ExceptionKind::ToolRuntime) => 2103,
        Some(ExceptionKind::EvalException) => error.ec().code(),
        None => {
            return Ok(refuse(
                "ordered prefix diagnostic constructor is unproved".into(),
            ));
        }
    };
    Ok((Validation::Valid, Some(deciding)))
}

fn reported(error: &EvalError, site: Site) -> Option<u32> {
    if site == Site::Invariant {
        return Some(2111);
    }
    match error.exception_kind()? {
        ExceptionKind::ToolRuntime => Some(1000),
        ExceptionKind::EvalException => Some(error.ec().code()),
    }
}
fn event_validation(event: Event) -> Validation {
    match event {
        Event::Refused(reason) => Validation::Unsupported(reason),
        Event::Failed { check, detail } => Validation::Invalid(Failure {
            state: 1,
            check,
            detail,
        }),
        _ => invalid(
            "an earlier error, incomplete candidate or completed expansion contradicts the terminal claim",
        ),
    }
}
fn invalid(detail: &str) -> Validation {
    Validation::Invalid(Failure {
        state: 1,
        check: Check::Ending,
        detail: detail.into(),
    })
}
