//! `ENABLED` and successor existence on a given state, decided by the core
//! evaluator's `ENABLED` (ADR 0103). The validator replays and checks the
//! counterexample's states and steps itself; whether an action has some
//! successor is the evaluator's TLC-exact search, the same semantics the
//! checker uses for `ENABLED`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dive_eval::{Counter, Host, HostError, RunConfig};
use dive_ir::StdOp;
use dive_model::{Model, Pred};
use dive_value::{Ctx, Value};

use crate::State;

/// Why `ENABLED` could not be decided. A refusal is never a decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnabledError {
    /// A cooperative caller interruption, whose authority belongs to the driver.
    Interrupted(dive_eval::EvalError),
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
            Self::Interrupted(_) => "DIVE-T0022",
        }
    }
}

impl std::fmt::Display for EnabledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ENABLED: ", self.code())?;
        match self {
            Self::Unsupported(s) => write!(f, "unsupported {s}"),
            Self::Invalid(s) => write!(f, "invalid {s}"),
            Self::Interrupted(error) => error.fmt(f),
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
/// error or refusal during the search, including action composition
/// ([`crate::COMPOSITION_UNSUPPORTED`], refused as replay refuses it); and a
/// string or model value created
/// after binding ([`crate::RUNTIME_NAMES_UNSUPPORTED`]), whatever the result.
pub fn enabled(
    model: &Model,
    action: &Pred,
    nonstuttering: Option<dive_ir::ExprId>,
    state: &State,
) -> Result<bool, EnabledError> {
    let unsupported = |e: dive_eval::EvalError| {
        if e.class() == dive_eval::ErrorClass::Interrupted {
            EnabledError::Interrupted(e)
        } else {
            EnabledError::Unsupported(e.to_string())
        }
    };
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
    let guard = ComposeGuard::install(model);
    let result =
        model
            .evaluator
            .enabled_scoped(&action.scope, action.expr, nonstuttering, state.clone());
    let composed = guard.composed();
    drop(guard);
    if let Err(error) = &result
        && error.class() == dive_eval::ErrorClass::Interrupted
    {
        return Err(EnabledError::Interrupted(error.clone()));
    }
    // Composition is refused whatever the search did with the refusal, as
    // replay refuses it: `[][A]_x` and `validate_doc` refuse the same action.
    if composed {
        return Err(EnabledError::Unsupported(
            crate::COMPOSITION_UNSUPPORTED.to_owned(),
        ));
    }
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

/// Typed source-row query after independent canonical/context admission.
pub(crate) fn source_enabled(
    model: &Model,
    action: &Pred,
    nonstuttering: Option<dive_ir::ExprId>,
    state: &State,
) -> Result<bool, dive_eval::EvalError> {
    model
        .evaluator
        .validate_predicate(&action.scope, action.expr)?;
    if let Some(sub) = nonstuttering {
        model.evaluator.validate_predicate(&action.scope, sub)?;
    }
    let guard = ComposeGuard::install(model);
    let result =
        model
            .evaluator
            .enabled_scoped(&action.scope, action.expr, nonstuttering, state.clone());
    let composed = guard.composed();
    drop(guard);
    if composed {
        return Err(dive_eval::EvalError::unsupported(
            crate::COMPOSITION_UNSUPPORTED,
        ));
    }
    if model.has_runtime_names() {
        return Err(dive_eval::EvalError::unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED,
        ));
    }
    result
}

/// The host an `ENABLED` search runs under: the installed host, except that
/// action composition is refused ([`crate::COMPOSITION_UNSUPPORTED`]) as
/// replay refuses it, and lazy values live in a cache domain of the
/// validator's own unless the installed host already has one (replay's). A
/// value cached by ordinary evaluation, such as a composition argument the
/// checker forced, therefore never stands in for the search (ADR 0103).
#[derive(Debug)]
struct ComposeGuardHost {
    inner: Option<Arc<dyn Host>>,
    composition: AtomicBool,
    cache_domain: u64,
}

impl Host for ComposeGuardHost {
    fn check_native_access(&self, op: StdOp) -> Result<(), HostError> {
        self.inner
            .as_ref()
            .map_or(Ok(()), |h| h.check_native_access(op))
    }

    fn print_output(&self, text: &str) -> Result<bool, HostError> {
        self.inner
            .as_ref()
            .map_or(Ok(true), |h| h.print_output(text))
    }

    fn check_tlc_access(&self, key: &str, write: bool) -> Result<(), HostError> {
        self.inner
            .as_ref()
            .map_or(Ok(()), |h| h.check_tlc_access(key, write))
    }

    fn random(&self, op: StdOp, args: &[Value], ctx: Ctx) -> Option<Result<Value, HostError>> {
        self.inner.as_ref().and_then(|h| h.random(op, args, ctx))
    }

    fn check_register_access(&self) -> Result<(), HostError> {
        self.inner
            .as_ref()
            .map_or(Ok(()), |h| h.check_register_access())
    }

    fn check_action_composition(&self) -> Result<(), HostError> {
        self.composition.store(true, Ordering::Relaxed);
        Err(HostError {
            code: 0,
            message: crate::COMPOSITION_UNSUPPORTED.to_owned(),
        })
    }

    fn expression_cache_domain(&self) -> u64 {
        self.cache_domain
    }

    fn level(&self) -> Option<i64> {
        self.inner.as_ref().and_then(|h| h.level())
    }

    fn counter(&self, counter: Counter) -> Option<i64> {
        self.inner.as_ref().and_then(|h| h.counter(counter))
    }

    fn config(&self) -> Option<RunConfig> {
        self.inner.as_ref().and_then(|h| h.config())
    }

    fn request_exit(&self) -> bool {
        // A side condition of a search is never a checker's exit request.
        false
    }
}

/// Installs a [`ComposeGuardHost`] for one search and restores the host
/// installed before it when dropped.
struct ComposeGuard<'a> {
    model: &'a Model,
    host: Arc<ComposeGuardHost>,
    inner: Option<Arc<dyn Host>>,
}

impl<'a> ComposeGuard<'a> {
    fn install(model: &'a Model) -> Self {
        let inner = model.evaluator.host();
        let cache_domain = inner
            .as_ref()
            .map(|h| h.expression_cache_domain())
            .filter(|domain| *domain != 0)
            .unwrap_or_else(crate::validate::next_cache_domain);
        let host = Arc::new(ComposeGuardHost {
            inner: inner.clone(),
            composition: AtomicBool::new(false),
            cache_domain,
        });
        let _installed_before = model.evaluator.replace_host(Some(host.clone()));
        Self { model, host, inner }
    }

    fn composed(&self) -> bool {
        self.host.composition.load(Ordering::Relaxed)
    }
}

impl Drop for ComposeGuard<'_> {
    fn drop(&mut self) {
        let _ = self.model.evaluator.replace_host(self.inner.take());
    }
}
