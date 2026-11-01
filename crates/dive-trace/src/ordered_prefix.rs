//! Independent finite admission history for the first noninitial source only.

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
    /// The actual diagnostic exception's deciding identity.
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

fn preflight(model: &Model) -> Option<String> {
    if let Some(feature) = model.unsupported.first() {
        return Some(format!(
            "ordered prefix unsupported model feature: {}",
            feature.feature
        ));
    }
    if model.symmetry.is_some()
        || model.alias.is_some()
        || !model.temporals.is_empty()
        || !model.implied_temporals.is_empty()
    {
        return Some(
            "ordered prefix excludes unsupported roles, symmetry, ALIAS and temporal obligations"
                .into(),
        );
    }
    let impure = |pred: &dive_model::Pred| {
        model
            .ir
            .node(pred.expr)
            .effects
            .intersects(dive_ir::Effects::IMPURE)
            || model.evaluator.scope_has_effects(&pred.scope)
    };
    if model
        .init_actions
        .iter()
        .chain(&model.actions)
        .any(|action| {
            model
                .ir
                .node(action.expr())
                .effects
                .intersects(dive_ir::Effects::IMPURE)
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
    if let Some(reason) = preflight(model) {
        return Ok(refuse(reason));
    }
    let expected_len = if matches!(trace.ending, Ending::InvariantError { .. }) {
        3
    } else {
        2
    };
    if trace.states.len() != expected_len {
        return Ok(refuse(
            "ordered prefix requires Init and its first admitted noninitial source".into(),
        ));
    }
    let initial = &trace.states[0];
    let source = &trace.states[1];
    let mut history = History::default();
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
    if emitted != 1 || history.admitted.first() != Some(initial) || history.admitted.len() != 1 {
        return Ok(refuse(
            "ordered prefix needs exactly one exhausted, admitted Init emission".into(),
        ));
    }
    let first = Driver {
        initial: false,
        model,
        source: initial,
        level: 1,
        host,
        control,
    };
    let (event, _) = first.primary_history(false, Some(&mut history))?;
    if !matches!(event, Event::End) {
        return Ok((event_validation(event), None));
    }
    if history.admitted.get(1) != Some(source) {
        return Ok((
            invalid(
                "terminal source is not the independently observed first admitted noninitial source",
            ),
            None,
        ));
    }
    let terminal = Driver {
        initial: false,
        model,
        source,
        level: 2,
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
    if error.value_method().is_some() || error.exception_kind().is_none() {
        return Ok(refuse(
            "ordered prefix primary receiver/constructor lifetime is unproved".into(),
        ));
    }
    // The primary insertion (including its failing invariant candidate) is
    // retained. Diagnostic generation uses the very same private runtime.
    let diagnostic = terminal.diagnostic_history(&mut history)?;
    let Event::Error { error, .. } = diagnostic else {
        return Ok((event_validation(diagnostic), None));
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
