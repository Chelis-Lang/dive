//! `ENABLED` and successor existence on a given state, decided by the core
//! evaluator's `ENABLED` (ADR 0102). The validator replays and checks the
//! counterexample's states and steps itself; whether an action has some
//! successor is the evaluator's TLC-exact search, the same semantics the
//! checker uses for `ENABLED`.

use dive_model::{Model, Pred};

use crate::State;

/// Why `ENABLED` could not be decided. A refusal is never a decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnabledError {
    /// A named feature or context the validator does not decide.
    Unsupported(String),
    /// A malformed state, or a TLC evaluation error raised by the search.
    Invalid(String),
}

impl EnabledError {
    /// Stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "DIVE-T0020",
            Self::Invalid(_) => "DIVE-T0021",
        }
    }
}

impl std::fmt::Display for EnabledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ENABLED: ", self.code())?;
        match self {
            Self::Unsupported(s) => write!(f, "unsupported {s}"),
            Self::Invalid(s) => write!(f, "invalid {s}"),
        }
    }
}

impl std::error::Error for EnabledError {}

/// Whether `action` has a successor from `state`: `ENABLED action`, or with
/// `nonstuttering`, `ENABLED <<action>>_nonstuttering`, which is the
/// enabledness that `WF`/`SF` refer to. `nonstuttering` is an expression in
/// `action`'s scope.
///
/// ```
/// use std::sync::Arc;
/// use dive_model::Pred;
/// use dive_value::Value;
/// let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x < 2 /\\ x' = x + 1\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", src, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis)?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc())?;
/// let next = model.next.clone().unwrap();
/// assert!(dive_trace::enabled::enabled(&model, &next, None, &Arc::from([Some(Value::from(1))]))?);
/// assert!(!dive_trace::enabled::enabled(&model, &next, None, &Arc::from([Some(Value::from(2))]))?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// A foreign or malformed model context, state or predicate; an evaluation
/// error or refusal during the search; and a string or model value created
/// after binding ([`crate::RUNTIME_NAMES_UNSUPPORTED`]), whatever the result.
pub fn enabled(
    model: &Model,
    action: &Pred,
    nonstuttering: Option<dive_ir::ExprId>,
    state: &State,
) -> Result<bool, EnabledError> {
    let unsupported = |e: dive_eval::EvalError| EnabledError::Unsupported(e.to_string());
    model.validate_context().map_err(unsupported)?;
    model
        .evaluator
        .validate_predicate(&action.scope, action.expr)
        .map_err(unsupported)?;
    if let Some(sub) = nonstuttering {
        model
            .evaluator
            .validate_predicate(&action.scope, sub)
            .map_err(unsupported)?;
    }
    if state.len() != model.ir.vars().len() || state.iter().any(Option::is_none) {
        return Err(EnabledError::Invalid(
            "state does not assign every variable".into(),
        ));
    }
    crate::trace::validate_state_names(model, state).map_err(EnabledError::Unsupported)?;
    // A search that prints, reads checker state or shares lazy side effects
    // with other evaluations is not a function of the given state.
    let impure = |e: dive_ir::ExprId| {
        model
            .ir
            .node(e)
            .effects
            .intersects(dive_ir::Effects::IMPURE)
    };
    if impure(action.expr)
        || nonstuttering.is_some_and(impure)
        || model.evaluator.scope_shares_side_effects(&action.scope)
    {
        return Err(EnabledError::Unsupported(
            "side effects or dynamically bound constants in ENABLED".into(),
        ));
    }
    let result =
        model
            .evaluator
            .enabled_scoped(&action.scope, action.expr, nonstuttering, state.clone());
    if model.has_runtime_names() {
        return Err(EnabledError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.to_owned(),
        ));
    }
    result.map_err(|e| {
        if e.class() == dive_eval::ErrorClass::Eval {
            EnabledError::Invalid(e.to_string())
        } else {
            EnabledError::Unsupported(e.to_string())
        }
    })
}
