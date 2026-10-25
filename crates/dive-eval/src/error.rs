//! Evaluation errors with TLC's error classes and message texts.
//!
//! An [`EvalError`] is a value, never a panic. It carries:
//! - its [`ErrorClass`]: an evaluation error TLC also raises, an explicit
//!   refusal of a feature dive does not implement exactly, a resource limit,
//!   or an internal bug;
//! - for evaluation errors, TLC's error code ([`Ec`]) and TLC's message text,
//!   byte for byte. Messages that TLC builds from a semantic node's
//!   `toString()` contain the node's location string, as TLC prints it;
//! - the IR node it was raised at, when known.
//!
//! TLC reports an error in one of two shapes (`util.Assert`, `tlc2.output.MP`):
//! a `GENERAL` (1000) error is printed as its bare message; any other code is
//! printed as a nested `-tool` message with its code. [`EvalError::tool_text`]
//! renders both, so the harness can compare dive's report with TLC's
//! transcript.

use std::fmt;

use dive_ir::ExprId;
use dive_value::{ErrorKind, ValueError};

/// A TLC error code (`tlc2.output.EC`) that evaluation can raise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Ec {
    /// `GENERAL` (1000): `Assert.fail` with a plain message.
    General,
    /// `TLC_EXPECTED_VALUE` (2215).
    ExpectedValue,
    /// `TLC_MODULE_APPLY_EMPTY_SEQ` (2184).
    ApplyEmptySeq,
    /// `TLC_MODULE_ONE_ARGUMENT_ERROR` (2283).
    OneArgumentError,
    /// `TLC_MODULE_ARGUMENT_ERROR` (2169).
    ArgumentError,
    /// `TLC_MODULE_ARGUMENT_ERROR_AN` (2266).
    ArgumentErrorAn,
    /// `TLC_MODULE_EVALUATING` (2182).
    ModuleEvaluating,
    /// `TLC_MODULE_ARGUMENT_NOT_IN_DOMAIN` (2183).
    ArgumentNotInDomain,
    /// `TLC_MODULE_COMPUTING_CARDINALITY` (2181).
    ComputingCardinality,
    /// `TLC_VALUE_ASSERT_FAILED` (2132).
    AssertFailed,
    /// `TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR` (2280).
    UndefinedOrNoOperator,
    /// `TLC_ENCOUNTERED_FORMULA_IN_PREDICATE` (2261).
    FormulaInPredicate,
    /// `TLC_MODULE_DIVISION_BY_ZERO` (2179).
    DivisionByZero,
    /// `TLC_MODULE_OVERFLOW` (2178).
    Overflow,
    /// `TLC_MODULE_NULL_POWER_NULL` (2180).
    NullPowerNull,
    /// `TLC_INTEGER_TOO_BIG` (2265).
    IntegerTooBig,
    /// `TLC_MODULE_CHECK_MEMBER_OF` (2158).
    CheckMemberOf,
    /// `TLC_MODULE_COMPARE_VALUE` (2155).
    CompareValue,
    /// `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` (2154).
    JavaMethodOverride,
    /// `TLC_CANT_HANDLE_REAL_NUMBERS` (2244).
    CantHandleReal,
    /// `TLC_STATE_NOT_COMPLETELY_SPECIFIED_LIVE` (2148).
    NotCompletelySpecifiedLive,
    /// `TLC_ARGUMENT_MISMATCH` (2170).
    ArgumentMismatch,
    /// `TLC_MODULE_APPLYING_TO_WRONG_VALUE` (2176).
    ApplyingToWrongValue,
    /// `TLC_EXPECTED_EXPRESSION_IN_COMPUTING` (2247): the successor
    /// generator met a non-boolean where it expected a predicate.
    ExpectedExpressionInComputing,
    /// `TLC_EXPECTED_EXPRESSION_IN_COMPUTING2` (2248): the same for a
    /// function application.
    ExpectedExpressionInComputing2,
    /// `TLC_ENABLED_WRONG_FORMULA` (2260).
    EnabledWrongFormula,
    /// `TLC_MODULE_TLCGET_UNDEFINED` (2145).
    TlcGetUndefined,
    /// `TLC_MODULE_BAG_UNION1` (2177).
    BagUnion1,
}

impl Ec {
    /// The numeric code.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::General => 1000,
            Self::ExpectedValue => 2215,
            Self::ApplyEmptySeq => 2184,
            Self::OneArgumentError => 2283,
            Self::ArgumentError => 2169,
            Self::ArgumentErrorAn => 2266,
            Self::ModuleEvaluating => 2182,
            Self::ArgumentNotInDomain => 2183,
            Self::ComputingCardinality => 2181,
            Self::AssertFailed => 2132,
            Self::UndefinedOrNoOperator => 2280,
            Self::FormulaInPredicate => 2261,
            Self::DivisionByZero => 2179,
            Self::Overflow => 2178,
            Self::NullPowerNull => 2180,
            Self::IntegerTooBig => 2265,
            Self::CheckMemberOf => 2158,
            Self::CompareValue => 2155,
            Self::JavaMethodOverride => 2154,
            Self::CantHandleReal => 2244,
            Self::NotCompletelySpecifiedLive => 2148,
            Self::ArgumentMismatch => 2170,
            Self::ApplyingToWrongValue => 2176,
            Self::ExpectedExpressionInComputing => 2247,
            Self::ExpectedExpressionInComputing2 => 2248,
            Self::EnabledWrongFormula => 2260,
            Self::TlcGetUndefined => 2145,
            Self::BagUnion1 => 2177,
        }
    }

    /// The `EC` constant's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::General => "GENERAL",
            Self::ExpectedValue => "TLC_EXPECTED_VALUE",
            Self::ApplyEmptySeq => "TLC_MODULE_APPLY_EMPTY_SEQ",
            Self::OneArgumentError => "TLC_MODULE_ONE_ARGUMENT_ERROR",
            Self::ArgumentError => "TLC_MODULE_ARGUMENT_ERROR",
            Self::ArgumentErrorAn => "TLC_MODULE_ARGUMENT_ERROR_AN",
            Self::ModuleEvaluating => "TLC_MODULE_EVALUATING",
            Self::ArgumentNotInDomain => "TLC_MODULE_ARGUMENT_NOT_IN_DOMAIN",
            Self::ComputingCardinality => "TLC_MODULE_COMPUTING_CARDINALITY",
            Self::AssertFailed => "TLC_VALUE_ASSERT_FAILED",
            Self::UndefinedOrNoOperator => "TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR",
            Self::FormulaInPredicate => "TLC_ENCOUNTERED_FORMULA_IN_PREDICATE",
            Self::DivisionByZero => "TLC_MODULE_DIVISION_BY_ZERO",
            Self::Overflow => "TLC_MODULE_OVERFLOW",
            Self::NullPowerNull => "TLC_MODULE_NULL_POWER_NULL",
            Self::IntegerTooBig => "TLC_INTEGER_TOO_BIG",
            Self::CheckMemberOf => "TLC_MODULE_CHECK_MEMBER_OF",
            Self::CompareValue => "TLC_MODULE_COMPARE_VALUE",
            Self::JavaMethodOverride => "TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE",
            Self::CantHandleReal => "TLC_CANT_HANDLE_REAL_NUMBERS",
            Self::NotCompletelySpecifiedLive => "TLC_STATE_NOT_COMPLETELY_SPECIFIED_LIVE",
            Self::ArgumentMismatch => "TLC_ARGUMENT_MISMATCH",
            Self::ApplyingToWrongValue => "TLC_MODULE_APPLYING_TO_WRONG_VALUE",
            Self::ExpectedExpressionInComputing => "TLC_EXPECTED_EXPRESSION_IN_COMPUTING",
            Self::ExpectedExpressionInComputing2 => "TLC_EXPECTED_EXPRESSION_IN_COMPUTING2",
            Self::EnabledWrongFormula => "TLC_ENABLED_WRONG_FORMULA",
            Self::TlcGetUndefined => "TLC_MODULE_TLCGET_UNDEFINED",
            Self::BagUnion1 => "TLC_MODULE_BAG_UNION1",
        }
    }

    fn from_code(code: u32) -> Self {
        match code {
            2215 => Self::ExpectedValue,
            2184 => Self::ApplyEmptySeq,
            2283 => Self::OneArgumentError,
            2169 => Self::ArgumentError,
            2266 => Self::ArgumentErrorAn,
            2182 => Self::ModuleEvaluating,
            2183 => Self::ArgumentNotInDomain,
            2181 => Self::ComputingCardinality,
            2132 => Self::AssertFailed,
            2280 => Self::UndefinedOrNoOperator,
            2261 => Self::FormulaInPredicate,
            2179 => Self::DivisionByZero,
            2178 => Self::Overflow,
            2180 => Self::NullPowerNull,
            2265 => Self::IntegerTooBig,
            2158 => Self::CheckMemberOf,
            2155 => Self::CompareValue,
            2154 => Self::JavaMethodOverride,
            2244 => Self::CantHandleReal,
            2148 => Self::NotCompletelySpecifiedLive,
            2170 => Self::ArgumentMismatch,
            2176 => Self::ApplyingToWrongValue,
            2247 => Self::ExpectedExpressionInComputing,
            2248 => Self::ExpectedExpressionInComputing2,
            2260 => Self::EnabledWrongFormula,
            2145 => Self::TlcGetUndefined,
            2177 => Self::BagUnion1,
            _ => Self::General,
        }
    }

    /// The code for a numeric TLC code (`GENERAL` for one evaluation does
    /// not raise).
    pub(crate) fn from_tlc_code(code: u32) -> Self {
        Self::from_code(code)
    }
}

/// What kind of failure an [`EvalError`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorClass {
    /// A cooperative host interruption, classified by the driver, never a
    /// specification failure or an arbitrary resource refusal.
    Interrupted,
    /// An evaluation error that TLC raises in the same situation, with TLC's
    /// code and message.
    Eval,
    /// dive does not implement the construct exactly and refuses it. The
    /// message names the feature (charter soundness rule: never approximate).
    Unsupported,
    /// A resource limit: evaluation depth, value nesting, or a dive-only size
    /// limit. TLC's counterpart is a JVM `StackOverflowError` or
    /// `OutOfMemoryError`, which the harness never treats as an oracle fact.
    ResourceLimit,
    /// An internal invariant was violated: a bug in dive.
    Bug,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Inner {
    class: ErrorClass,
    ec: Ec,
    message: String,
    origin: Option<ExprId>,
    /// Whether TLC raises this as an `EvalException` (not wrapped by a Java
    /// override) rather than as a `TLCRuntimeException`.
    eval_exception: bool,
    /// Static-analysis binders are not runtime values. Keep this typed so an
    /// unrelated refusal with the same text can never authorize lazy retry.
    analysis_only: bool,
    interruption: Option<crate::host::Interrupted>,
    /// Original typed cause of a built-in Java-override wrapper. Rendered
    /// message text, including apparent tool frames, grants no provenance.
    override_cause: Option<(dive_ir::StdOp, Ec)>,
}

const ANALYSIS_ONLY: &str = "analysis-only binder has no runtime value";

/// An evaluation error: its class, TLC's code and message, and the node it was
/// raised at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalError(Box<Inner>);

impl EvalError {
    fn make(class: ErrorClass, ec: Ec, message: String, eval_exception: bool) -> Self {
        Self(Box::new(Inner {
            class,
            ec,
            message,
            origin: None,
            eval_exception,
            analysis_only: false,
            interruption: None,
            override_cause: None,
        }))
    }

    /// A `GENERAL` evaluation error with TLC's message, as `Assert.fail(msg)`
    /// raises it.
    #[must_use]
    pub fn general(message: impl Into<String>) -> Self {
        Self::make(ErrorClass::Eval, Ec::General, message.into(), false)
    }

    /// An evaluation error with a specific code, as `Assert.fail(EC, params)`
    /// raises it (a `TLCRuntimeException`).
    #[must_use]
    pub fn coded(ec: Ec, message: impl Into<String>) -> Self {
        Self::make(ErrorClass::Eval, ec, message.into(), false)
    }

    /// An evaluation error raised by a module as an `EvalException`: a Java
    /// override passes it through unwrapped.
    #[must_use]
    pub fn eval_exception(ec: Ec, message: impl Into<String>) -> Self {
        Self::make(ErrorClass::Eval, ec, message.into(), true)
    }

    /// Constructs the actual native wrapper without recovering its cause from
    /// presentation text. Only the evaluator's native dispatch uses this path.
    pub(crate) fn wrapped_override(
        operator: dive_ir::StdOp,
        message: String,
        cause: &Self,
    ) -> Self {
        let mut error = Self::coded(Ec::JavaMethodOverride, message);
        error.0.override_cause = Some((operator, cause.ec()));
        error
    }

    /// The original operator and error code retained by native wrapping.
    ///
    /// An error made with [`Self::coded`] has no such provenance, even when
    /// its message contains the same method text and nested tool frames.
    ///
    /// ```
    /// use dive_eval::{Ec, EvalError};
    /// let error = EvalError::coded(Ec::JavaMethodOverride, "nested tool text");
    /// assert_eq!(error.override_cause(), None);
    /// ```
    #[must_use]
    pub fn override_cause(&self) -> Option<(dive_ir::StdOp, Ec)> {
        self.0.override_cause
    }

    /// An explicit refusal: dive does not implement `feature` exactly.
    #[must_use]
    pub fn unsupported(feature: impl Into<String>) -> Self {
        let feature = feature.into();
        Self::make(ErrorClass::Unsupported, Ec::General, feature, false)
    }

    /// The refusal raised when evaluation forces a static-analysis binder,
    /// which has no runtime value. See [`Self::is_analysis_only`].
    #[must_use]
    pub fn analysis_only() -> Self {
        let mut error = Self::unsupported(ANALYSIS_ONLY);
        error.0.analysis_only = true;
        error
    }

    /// Whether this is the [`Self::analysis_only`] refusal: the evaluated
    /// expression depends on a static-analysis binder. Planners use it to
    /// keep such an argument lazy instead of freezing a value.
    #[must_use]
    pub fn is_analysis_only(&self) -> bool {
        self.0.analysis_only
    }

    /// Cooperative host control flow. The driver retains the stop evidence.
    #[must_use]
    pub fn interrupted(interruption: crate::host::Interrupted) -> Self {
        let mut error = Self::make(
            ErrorClass::Interrupted,
            Ec::General,
            "host interrupted evaluation".into(),
            false,
        );
        error.0.interruption = Some(interruption);
        error
    }

    /// The typed host interruption, including its opaque driver identity.
    #[must_use]
    pub fn interruption(&self) -> Option<crate::host::Interrupted> {
        self.0.interruption
    }

    /// A resource limit.
    #[must_use]
    pub fn resource_limit(message: impl Into<String>) -> Self {
        Self::make(
            ErrorClass::ResourceLimit,
            Ec::General,
            message.into(),
            false,
        )
    }

    /// An internal error (a bug in dive), with enough context to reproduce it.
    #[must_use]
    pub fn bug(message: impl Into<String>) -> Self {
        Self::make(ErrorClass::Bug, Ec::General, message.into(), false)
    }

    /// Attaches the node the error was raised at, unless one is already set.
    #[must_use]
    pub fn at(mut self, origin: ExprId) -> Self {
        if self.0.origin.is_none() {
            self.0.origin = Some(origin);
        }
        self
    }

    /// The class.
    #[must_use]
    pub fn class(&self) -> ErrorClass {
        self.0.class
    }

    /// TLC's error code (meaningful for [`ErrorClass::Eval`]).
    #[must_use]
    pub fn ec(&self) -> Ec {
        self.0.ec
    }

    /// The message: TLC's text for an evaluation error, the feature for a
    /// refusal.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0.message
    }

    /// The node the error was raised at.
    #[must_use]
    pub fn origin(&self) -> Option<ExprId> {
        self.0.origin
    }

    /// Whether this is an evaluation error TLC also raises.
    #[must_use]
    pub fn is_eval(&self) -> bool {
        self.0.class == ErrorClass::Eval
    }

    /// Whether TLC raises this error as an `EvalException` rather than as a
    /// `TLCRuntimeException` (`Assert.fail`). Some of TLC's messages depend
    /// on it (`SpecProcessor.processConstantDefns`).
    #[must_use]
    pub fn is_eval_exception(&self) -> bool {
        self.0.eval_exception
    }

    /// Whether a Java override would pass the error through unchanged
    /// (TLC's `EvalException`), rather than wrapping it in
    /// `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE`.
    #[must_use]
    pub(crate) fn passes_override(&self) -> bool {
        self.0.eval_exception || self.0.class != ErrorClass::Eval
    }

    /// The error as TLC prints it nested in another `-tool` message: the bare
    /// message for `GENERAL`, or the message wrapped in its own
    /// `@!@!@STARTMSG code:0 @!@!@` block.
    #[must_use]
    pub fn tool_text(&self) -> String {
        match self.0.ec {
            Ec::General => self.0.message.clone(),
            ec => format!(
                "@!@!@STARTMSG {}:0 @!@!@\n{}\n@!@!@ENDMSG {} @!@!@",
                ec.code(),
                self.0.message,
                ec.code()
            ),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.class {
            ErrorClass::Eval => write!(
                f,
                "{} ({}): {}",
                self.0.ec.name(),
                self.0.ec.code(),
                self.0.message
            ),
            ErrorClass::Interrupted => write!(f, "evaluation interrupted by its host"),
            ErrorClass::Unsupported => write!(f, "unsupported: {}", self.0.message),
            ErrorClass::ResourceLimit => write!(f, "resource limit: {}", self.0.message),
            ErrorClass::Bug => write!(f, "internal error: {}", self.0.message),
        }
    }
}

impl std::error::Error for EvalError {}

#[derive(Debug)]
struct Stashed {
    receipt: Option<u32>,
    error: EvalError,
}

thread_local! {
    /// A deferred provider's error, recovered only by the matching transport.
    static STASH: std::cell::RefCell<Option<Stashed>> = const { std::cell::RefCell::new(None) };
}

// Process uniqueness is necessary: a ValueError can cross a worker boundary.
// Zero marks exhausted identity space, never a receipt. Interruption and sealed
// native-cause errors take this cold path; successful evaluation does not.
static NEXT_RECEIPT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

fn receipt(counter: &std::sync::atomic::AtomicU32) -> Option<u32> {
    use std::sync::atomic::Ordering;
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            (next != 0).then(|| next.checked_add(1).unwrap_or(0))
        })
        .ok()
}

/// Passes an evaluation error through `dive-value` (which only knows
/// [`ValueError`]), so that [`EvalError::from`] can recover it intact.
pub(crate) fn stash(error: EvalError) -> ValueError {
    stash_with_counter(error, &NEXT_RECEIPT)
}

fn stash_with_counter(error: EvalError, counter: &std::sync::atomic::AtomicU32) -> ValueError {
    let (error, receipt) =
        if error.class() == ErrorClass::Interrupted || error.override_cause().is_some() {
            match receipt(counter) {
                Some(receipt) => (error, Some(receipt)),
                None => (
                    EvalError::bug("deferred evaluator receipt identity space exhausted"),
                    None,
                ),
            }
        } else {
            (error, None)
        };
    let kind = match (error.class(), error.ec(), receipt) {
        (ErrorClass::Interrupted, _, Some(receipt)) => ErrorKind::Interrupted { receipt },
        (_, _, Some(receipt)) => ErrorKind::DeferredError { receipt },
        (ErrorClass::Eval, Ec::General, _) => ErrorKind::NotEnumerable,
        (ErrorClass::Eval, _, _) => ErrorKind::Argument,
        _ => ErrorKind::DiveLimit,
    };
    let value_error = ValueError::new(kind, error.message().to_owned());
    STASH.with(|stash| *stash.borrow_mut() = Some(Stashed { receipt, error }));
    value_error
}

impl From<ValueError> for EvalError {
    fn from(error: ValueError) -> Self {
        if let ErrorKind::Interrupted { receipt } | ErrorKind::DeferredError { receipt } =
            error.kind()
        {
            let interruption = matches!(error.kind(), ErrorKind::Interrupted { .. });
            return STASH
                .with(|stash| {
                    let mut stash = stash.borrow_mut();
                    if stash.as_ref().is_some_and(|pending| {
                        pending.receipt == Some(receipt)
                            && if interruption {
                                pending.error.class() == ErrorClass::Interrupted
                            } else {
                                pending.error.override_cause().is_some()
                            }
                    }) {
                        stash.take().map(|pending| pending.error)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    Self::bug("value-layer error has no matching typed evaluator receipt")
                });
        }
        let recovered = STASH.with(|stash| {
            let mut stash = stash.borrow_mut();
            // An ordinary value error cannot consume a pending interruption,
            // even when its text happens to equal the provider's message.
            if stash
                .as_ref()
                .is_some_and(|pending| pending.receipt.is_some())
            {
                return None;
            }
            stash.take().map(|pending| pending.error)
        });
        if let Some(stashed) = recovered
            && stashed.message() == error.message()
        {
            return stashed;
        }
        match error.kind() {
            ErrorKind::DiveLimit => Self::resource_limit(error.message().to_owned()),
            kind => {
                let ec = Ec::from_code(kind.tlc_code());
                // Preserve the existing TLC module/override wrapping contract.
                let eval_exception = ec != Ec::General
                    && !error
                        .message()
                        .starts_with("Overflow when computing the number of elements");
                Self::make(
                    ErrorClass::Eval,
                    ec,
                    error.message().to_owned(),
                    eval_exception,
                )
            }
        }
    }
}

/// Result alias for evaluation.
pub type Result<T, E = EvalError> = std::result::Result<T, E>;

#[cfg(test)]
#[allow(clippy::expect_used, reason = "typed interruption transport controls")]
mod interruption_transport {
    use super::*;
    use crate::Interrupted;

    #[test]
    fn stale_receipts_and_ordinary_errors_do_not_consume_another_interruption() {
        let stale = stash(EvalError::interrupted(Interrupted::new(10)));
        let current = stash(EvalError::interrupted(Interrupted::new(20)));
        assert_ne!(stale.kind(), current.kind());
        assert_eq!(EvalError::from(stale).class(), ErrorClass::Bug);
        let ordinary = ValueError::new(ErrorKind::Argument, "host interrupted evaluation");
        assert_eq!(EvalError::from(ordinary).class(), ErrorClass::Eval);
        let duplicate = current.clone();
        let recovered = EvalError::from(current);
        assert_eq!(recovered.interruption(), Some(Interrupted::new(20)));
        assert_eq!(EvalError::from(duplicate).class(), ErrorClass::Bug);
    }

    #[test]
    fn transport_to_another_thread_fails_closed_without_consuming_either_owner() {
        let original = stash(EvalError::interrupted(Interrupted::new(10)));
        let moved = original.clone();
        std::thread::spawn(move || {
            let local = stash(EvalError::interrupted(Interrupted::new(20)));
            assert_ne!(moved.kind(), local.kind());
            assert_eq!(EvalError::from(moved).class(), ErrorClass::Bug);
            assert_eq!(
                EvalError::from(local).interruption(),
                Some(Interrupted::new(20))
            );
        })
        .join()
        .expect("worker");
        assert_eq!(
            EvalError::from(original).interruption(),
            Some(Interrupted::new(10))
        );
    }

    #[test]
    fn receipt_ids_never_wrap_or_reuse_and_transport_layout_stays_small() {
        let counter = std::sync::atomic::AtomicU32::new(u32::MAX - 1);
        assert_eq!(receipt(&counter), Some(u32::MAX - 1));
        assert_eq!(receipt(&counter), Some(u32::MAX));
        assert_eq!(receipt(&counter), None);
        assert_eq!(receipt(&counter), None);
        let exhausted = stash_with_counter(EvalError::interrupted(Interrupted::new(10)), &counter);
        assert_eq!(EvalError::from(exhausted).class(), ErrorClass::Bug);
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(std::mem::size_of::<ValueError>(), 32);
            assert_eq!(std::mem::size_of::<ErrorKind>(), 8);
        }
    }
}

#[cfg(test)]
mod native_cause_transport {
    use super::*;
    use dive_ir::StdOp;

    fn wrapped() -> EvalError {
        EvalError::wrapped_override(
            StdOp::FiniteSetsCardinality,
            "original wrapper text".to_owned(),
            &EvalError::coded(Ec::Overflow, "original size overflow"),
        )
    }

    #[test]
    fn native_origin_survives_clone_and_real_receipt_but_not_rendered_forgery() {
        let original = wrapped().at(ExprId(7));
        assert_eq!(
            original.clone().override_cause(),
            Some((StdOp::FiniteSetsCardinality, Ec::Overflow))
        );
        let forged = EvalError::coded(Ec::JavaMethodOverride, original.message());
        assert_eq!(forged.override_cause(), None);
        let transported = stash(original.clone());
        assert!(matches!(
            transported.kind(),
            ErrorKind::DeferredError { .. }
        ));
        let text_forgery = ValueError::new(ErrorKind::Argument, original.message());
        assert_eq!(EvalError::from(text_forgery).override_cause(), None);
        let duplicate = transported.clone();
        assert_eq!(EvalError::from(transported), original);
        assert_eq!(EvalError::from(duplicate).class(), ErrorClass::Bug);
    }

    #[test]
    fn native_receipts_do_not_cross_threads_or_exhaust_into_a_verdict()
    -> Result<(), Box<dyn std::error::Error>> {
        let value = stash(wrapped());
        let moved = value.clone();
        std::thread::spawn(move || {
            assert_eq!(EvalError::from(moved).class(), ErrorClass::Bug);
        })
        .join()
        .map_err(|_| "worker failed")?;
        assert!(EvalError::from(value).override_cause().is_some());
        let exhausted = std::sync::atomic::AtomicU32::new(0);
        assert_eq!(
            EvalError::from(stash_with_counter(wrapped(), &exhausted)).class(),
            ErrorClass::Bug
        );
        Ok(())
    }
}
