//! TLC's error-class table (WP-C4): how the pinned TLC reports an evaluation
//! error, by the site where it is raised.
//!
//! Every rule here was read from TLC's source (tlaplus `94d0c50`) and checked
//! against captured runs of the pinned TLC: the matrix of error sites ×
//! error kinds in `tests/fixtures/errors/` (`tests/errors.rs`).
//!
//! **Two messages and a deciding code.** When an error stops the search,
//! TLC prints one message for it (the *reported* code and text) and, in
//! most cases, a second one from a replay; the code its run returns decides
//! the exit status. The observable primary class and physical exit are
//! separate for the qualified owned Init replay envelopes below.
//!
//! - **While exploring** (`Worker.run` → `ModelChecker.doNextFailed`): an
//!   `EvalException` is reported with its own code and message; anything
//!   else (`Assert.fail`, a Java override's wrapped error) as `GENERAL`,
//!   "TLC threw an unexpected exception ...". An invariant or action
//!   property that fails to evaluate is reported as
//!   `TLC_INVARIANT_EVALUATION_FAILED` or `TLC_ACTION_PROPERTY_EVALUATION_FAILED`
//!   around the error (`doNextEvalFailed`).
//! - **The replay** (`ModelChecker.modelCheckImpl`, `keepCallStack`): TLC
//!   expands the predecessor again with a `CallStackTool`, through
//!   `ModelChecker.doNext`, which re-inserts, re-checks and re-enqueues
//!   successors. If the error happens again, TLC prints
//!   `TLC_NESTED_EXPRESSION` and returns the `EvalException`'s code, or
//!   `TLC_NESTED_EXPRESSION` itself for any other exception. The replay
//!   gives values a source location, and the methods of a value with a
//!   source wrap their errors in a `FingerprintException`, which TLC reports
//!   as `TLC_FINGERPRINT_EXCEPTION` instead (fixtures `ErWrapTupleGuard`,
//!   `ErWrapRecord`, `ErWrapSetMember`, harness fixture `EvalError`).
//!   Whether a value has a source depends on where it was built. Actual lazy
//!   function construction and materialization catches retain that provenance;
//!   other unproved receiver lifetimes keep an unverified class.
//! - **Initial states** (`ModelChecker.modelCheckImpl`, the `doInit`
//!   catch): `GENERAL` with the error's text, or `TLC_INITIAL_STATE` naming
//!   the state when a check of an initial state raised a non-`EvalException`;
//!   the replay (`doInit(cTool, true)`) then returns `TLC_NESTED_EXPRESSION`,
//!   unless a source-bearing receiver raises `TLC_FINGERPRINT_EXCEPTION`.
//!   For owned NONE2215 or native2154/NONE2178, the source-qualified harness
//!   retains GENERAL1000 as primary and the actual replay exit 75. Native
//!   wrapper causes are retained as typed provenance, never parsed from text.
//!
//! What the replay does to the counts and to the verdict of an invariant
//! that fails to evaluate on a new state is `engine::Search::replay_after`'s.

use dive_eval::{Ec, ErrorClass, EvalError};

use crate::result::{ErrorSite, Verdict};

/// The text TLC prints for a non-`EvalException` caught while exploring
/// (`MP.printError(EC.GENERAL, Throwable)`), before the exception's message.
const GENERAL_PREFIX: &str = "TLC threw an unexpected exception.\nThis was probably caused by an error in the spec or model.\nSee the User Output or TLC Console for clues to what happened.\nThe exception was a java.lang.RuntimeException\n: ";

/// `EvalException` codes whose replay class is measured: the module
/// operators' own errors, never raised inside a value's method.
const MEASURED_EVAL_EXCEPTIONS: [u32; 3] = [
    2132, // TLC_VALUE_ASSERT_FAILED
    2178, // TLC_MODULE_OVERFLOW
    2179, // TLC_MODULE_DIVISION_BY_ZERO
];

/// Codes of `Assert.fail` errors that TLC's evaluator raises itself (not a
/// value's method), measured to replay as `TLC_NESTED_EXPRESSION`.
const TOOL_CODES: [u32; 5] = [
    2154, // TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE (fixtures `Er*TypeErr`)
    2215, // TLC_EXPECTED_VALUE (`ErConstrNonBool`)
    2247, // TLC_EXPECTED_EXPRESSION_IN_COMPUTING (`ErInitNonBool`, `ErNextAfterNonBool`)
    2248, // TLC_EXPECTED_EXPRESSION_IN_COMPUTING2
    2280, // TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR (`ErNextUnassigned`)
];

/// Beginnings of `GENERAL` messages that TLC's evaluator raises itself,
/// measured to replay as `TLC_NESTED_EXPRESSION` (fixtures `Er*Choose`,
/// `Er*NonEnum`, `Er*NonBoolIf`, `ErNextNonBool`, `Er*Domain`,
/// `ErWrapLambda*`, `ErWrapFilter`, `ErWrapRecursive`, `CdOff`).
const TOOL_MESSAGES: [&str; 8] = [
    "The current version of TLC does not support action composition.",
    "Attempted to compute the value of an expression of form\nCHOOSE",
    "TLC encountered the non-enumerable quantifier bound",
    "A non-boolean expression (",
    "In computing next states, a non-boolean expression (",
    "The next state relation is not a boolean expression",
    "In applying the function\n",
    "Attempted to evaluate an expression of form [A]_e, but A was not a boolean.",
];

/// Whether TLC's replay class of `e` is measured: an `EvalException` of a
/// module operator, or an error TLC's evaluator raises itself. Errors of a
/// value's own methods (a tuple index, a record field, a comparison) depend
/// on where the value was built (see the module documentation).
fn replay_class_known(e: &EvalError) -> bool {
    if let Some(receiver) = e.value_method() {
        return receiver.source().is_some();
    }
    let code = e.ec().code();
    if e.is_eval_exception() {
        MEASURED_EVAL_EXCEPTIONS.contains(&code)
    } else if e.ec() == Ec::General {
        TOOL_MESSAGES.iter().any(|p| e.message().starts_with(p))
    } else {
        TOOL_CODES.contains(&code)
    }
}

/// The primary message of the two source-qualified Init replay envelopes.
/// The internal replay code still supplies physical exit 75. Native wrapper
/// provenance is typed; neither a method signature nor nested message text
/// can establish this rule. Other Init errors retain their existing table.
pub(crate) fn init_primary_general(
    site: ErrorSite,
    code: u32,
    reported: u32,
    verified: bool,
    cause: &EvalError,
) -> bool {
    site == ErrorSite::Init
        && code == 2103
        && reported == 1000
        && verified
        && cause.is_eval()
        && !cause.is_eval_exception()
        && (cause.ec() == Ec::ExpectedValue
            || (cause.ec() == Ec::JavaMethodOverride
                && matches!(cause.override_cause(), Some((_, Ec::Overflow)))))
}

/// The `EC` name of a code this module or the evaluator reports.
#[must_use]
pub(crate) fn ec_name(code: u32) -> &'static str {
    match code {
        1000 => "GENERAL",
        2102 => "TLC_INITIAL_STATE",
        2103 => "TLC_NESTED_EXPRESSION",
        2105 => "TLC_ASSUMPTION_EVALUATION_ERROR",
        2111 => "TLC_INVARIANT_EVALUATION_FAILED",
        2113 => "TLC_ACTION_PROPERTY_EVALUATION_FAILED",
        2147 => "TLC_FINGERPRINT_EXCEPTION",
        2405 => "TLC_POSTCONDITION_EVALUATION_ERROR",
        c => crate::eval_code_name(c),
    }
}

/// The verdict for an error that is not an evaluation error TLC raises.
fn not_eval(e: &EvalError) -> Option<Verdict> {
    match e.class() {
        ErrorClass::Eval => None,
        ErrorClass::Unsupported => Some(Verdict::Unsupported {
            feature: e.message().to_owned(),
        }),
        ErrorClass::ResourceLimit => Some(Verdict::ResourceLimit {
            message: e.message().to_owned(),
        }),
        ErrorClass::Interrupted => Some(Verdict::InternalError {
            message: "host interruption has no matching explicit run context".into(),
        }),
        ErrorClass::Bug => Some(Verdict::InternalError {
            message: e.message().to_owned(),
        }),
    }
}

fn verdict(
    site: ErrorSite,
    code: u32,
    verified: bool,
    reported: u32,
    message: String,
    e: &EvalError,
    invariant: Option<String>,
) -> Verdict {
    Verdict::EvaluationError {
        site,
        code,
        name: ec_name(code),
        reported,
        message,
        verified,
        cause: e.clone(),
        invariant,
    }
}

/// The deciding code when TLC's replay raises `e` again, and whether it is
/// verified.
fn replayed(e: &EvalError) -> (u32, bool) {
    let code = if e
        .value_method()
        .is_some_and(|failure| failure.source().is_some())
    {
        2147
    } else if e.is_eval_exception() {
        e.ec().code()
    } else {
        2103
    };
    (code, replay_class_known(e))
}

/// An error raised at `site` while exploring, as TLC classifies it.
///
/// - `Next`, `Constraint`, `ActionConstraint`, `View` and `Fingerprint`
///   (and `Print` output, which is part of them): reported with the
///   error's own code or as `GENERAL`, decided by the replay, which meets
///   the error again.
/// - `Invariant` (named `name`): reported as
///   `TLC_INVARIANT_EVALUATION_FAILED`, which also decides, because the
///   replay finds the state already inserted and does not check it again.
///   For a state outside the model, which the replay checks again, apply
///   [`recurring`]. The replay can still change the class
///   ([`replay_error`]).
/// - `ImpliedAction` (named `name`): reported as
///   `TLC_ACTION_PROPERTY_EVALUATION_FAILED`, decided by the replay, which
///   checks the step again.
/// - `Startup`: as raised (the binding stage's errors, WP-C1).
#[must_use]
pub(crate) fn search_error(e: &EvalError, site: ErrorSite, name: Option<&str>) -> Verdict {
    if let Some(v) = not_eval(e) {
        return v;
    }
    let invariant = name;
    let name = name.unwrap_or("?");
    match site {
        ErrorSite::Invariant => verdict(
            site,
            2111,
            true,
            2111,
            format!("Evaluating invariant {name} failed.\n{}", e.tool_text()),
            e,
            invariant.map(str::to_owned),
        ),
        ErrorSite::ImpliedAction => {
            let (code, verified) = replayed(e);
            verdict(
                site,
                code,
                verified,
                2113,
                format!(
                    "Evaluating action property {name} failed.\n{}",
                    e.tool_text()
                ),
                e,
                None,
            )
        }
        ErrorSite::Startup => {
            let code = e.ec().code();
            verdict(site, code, true, code, e.message().to_owned(), e, None)
        }
        _ => {
            let (code, verified) = replayed(e);
            let (reported, message) = if e.is_eval_exception() {
                (e.ec().code(), e.message().to_owned())
            } else {
                (1000, format!("{GENERAL_PREFIX}{}", e.tool_text()))
            };
            verdict(site, code, verified, reported, message, e, None)
        }
    }
}

/// An invariant that failed to evaluate on a state TLC's replay checks
/// again (a state outside the model): the replay decides.
#[must_use]
pub(crate) fn recurring(v: Verdict) -> Verdict {
    match v {
        Verdict::EvaluationError {
            site,
            reported,
            message,
            cause,
            invariant,
            ..
        } => {
            let (code, verified) = replayed(&cause);
            verdict(site, code, verified, reported, message, &cause, invariant)
        }
        other => other,
    }
}

/// The class once TLC's replay meets the further error `e2` after an
/// invariant failed to evaluate on a new state (`Search::replay_after`).
#[must_use]
pub(crate) fn replay_error(v: Verdict, e2: &EvalError) -> Verdict {
    match v {
        Verdict::EvaluationError {
            site,
            reported,
            message,
            cause,
            invariant,
            ..
        } => {
            if let Some(refused) = not_eval(e2) {
                return refused;
            }
            let (code, verified) = replayed(e2);
            verdict(site, code, verified, reported, message, &cause, invariant)
        }
        other => other,
    }
}

/// An error raised while TLC generates or checks the initial states
/// (`ModelChecker.modelCheckImpl`'s `doInit` catch). `state` is TLC's text
/// of the initial state being checked, for an error in a check of a state
/// (constraints, fingerprint, invariants, implied initial predicates), and
/// `None` for an error while generating them.
#[must_use]
pub(crate) fn init_error(
    v: Verdict,
    state: Option<&str>,
    diagnostic: Option<&EvalError>,
) -> Verdict {
    let Verdict::EvaluationError {
        site,
        cause,
        invariant,
        ..
    } = v
    else {
        return v;
    };
    let (reported, message) = match state {
        // `DoInitFunctor` records the state, and `doInit` passes it on
        // for a `TLCRuntimeException` only.
        Some(s) if !cause.is_eval_exception() => (
            2102,
            format!(
                "{}\nWhile working on the initial state:\n{s}",
                cause.tool_text()
            ),
        ),
        _ => (1000, cause.tool_text()),
    };
    // The replay (`doInit(cTool, true)`) normally reports exceptions as
    // `TLC_NESTED_EXPRESSION` (fixtures `ErInit*`, `ErPropInit*`). An actual
    // source-bearing value-method catch takes the separate outer 2147 branch.
    let sourced_receiver = diagnostic.filter(|replay| {
        replay.is_eval()
            && replay.ec() == cause.ec()
            && replay.exception_kind() == cause.exception_kind()
            && cause.value_method().is_some_and(|original| {
                replay.value_method().is_some_and(|actual| {
                    original.method() == actual.method()
                        && original.constructor() == actual.constructor()
                        && actual.source().is_some()
                })
            })
    });
    // A recorded initial-invariant runtime context is not a deciding-class
    // certificate. In particular InitialTool's pinned stream contains 2102 and
    // context-only 2103, with no canonical deciding primary (#685). Preserve
    // generation/module exceptions and actual sourced receiver proof separately.
    let unproved_context =
        state.is_some() && site == ErrorSite::Invariant && !cause.is_eval_exception();
    let (code, verified) = sourced_receiver.map_or_else(
        || (2103, !unproved_context && replay_class_known(&cause)),
        |_| (2147, true),
    );
    verdict(site, code, verified, reported, message, &cause, invariant)
}

/// An `ASSUME` that fails to evaluate: `TLC_ASSUMPTION_EVALUATION_ERROR`
/// with TLC's message (WP-C1).
#[must_use]
pub(crate) fn assumption_error(e: &EvalError, message: String) -> Verdict {
    if let Some(v) = not_eval(e) {
        return v;
    }
    verdict(ErrorSite::Assumption, 2105, true, 2105, message, e, None)
}

/// A `POSTCONDITION` that fails to evaluate after a complete search
/// (`Tool.checkPostConditionWithContext`): `TLC_POSTCONDITION_EVALUATION_ERROR`
/// around the error (fixtures `ErPost*`).
#[must_use]
pub(crate) fn postcondition_error(e: &EvalError, name: &str, location: &str) -> Verdict {
    if let Some(v) = not_eval(e) {
        return v;
    }
    verdict(
        ErrorSite::Postcondition,
        2405,
        true,
        2405,
        format!(
            "Evaluating postcondition {name} at {location} failed.\n{}",
            e.tool_text()
        ),
        e,
        None,
    )
}

/// TLC's `GENERAL` report of a non-`EvalException` caught by its outermost
/// handler (`MP.printError(EC.GENERAL, Throwable)`), around `inner`.
#[must_use]
pub(crate) fn general_text(inner: &str) -> String {
    format!("{GENERAL_PREFIX}{inner}")
}
