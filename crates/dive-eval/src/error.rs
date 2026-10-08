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
            _ => Self::General,
        }
    }
}

/// What kind of failure an [`EvalError`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorClass {
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
}

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

    /// An explicit refusal: dive does not implement `feature` exactly.
    #[must_use]
    pub fn unsupported(feature: impl Into<String>) -> Self {
        let feature = feature.into();
        Self::make(ErrorClass::Unsupported, Ec::General, feature, false)
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
            ErrorClass::Unsupported => write!(f, "unsupported: {}", self.0.message),
            ErrorClass::ResourceLimit => write!(f, "resource limit: {}", self.0.message),
            ErrorClass::Bug => write!(f, "internal error: {}", self.0.message),
        }
    }
}

impl std::error::Error for EvalError {}

thread_local! {
    /// An evaluation error raised inside a lazy set while `dive-value` was
    /// enumerating it or testing membership, waiting to be recovered when the
    /// value layer hands the error back.
    static STASH: std::cell::RefCell<Option<EvalError>> = const { std::cell::RefCell::new(None) };
}

/// Passes an evaluation error through `dive-value` (which only knows
/// [`ValueError`]), so that [`EvalError::from`] can recover it intact.
pub(crate) fn stash(e: EvalError) -> ValueError {
    let kind = match (e.class(), e.ec()) {
        (ErrorClass::Eval, Ec::General) => ErrorKind::NotEnumerable,
        (ErrorClass::Eval, _) => ErrorKind::Argument,
        _ => ErrorKind::DiveLimit,
    };
    let ve = ValueError::new(kind, e.message().to_owned());
    STASH.with(|s| *s.borrow_mut() = Some(e));
    ve
}

impl From<ValueError> for EvalError {
    fn from(e: ValueError) -> Self {
        if let Some(stashed) = STASH.with(|s| s.borrow_mut().take())
            && stashed.message() == e.message()
        {
            return stashed;
        }
        match e.kind() {
            ErrorKind::DiveLimit => Self::resource_limit(e.message().to_owned()),
            kind => {
                let ec = Ec::from_code(kind.tlc_code());
                // TLC raises the value layer's coded errors as `EvalException`s
                // (`Naturals`, `Integers`, the `member`/`compareTo` of the module
                // sets), and its `GENERAL` ones with `Assert.fail`.
                // The size overflow of a set (`Overflow when computing the
                // number of elements in: ...`) is an `Assert.fail`, so a Java
                // override wraps it.
                let eval_exception = ec != Ec::General
                    && !e
                        .message()
                        .starts_with("Overflow when computing the number of elements");
                Self::make(ErrorClass::Eval, ec, e.message().to_owned(), eval_exception)
            }
        }
    }
}

/// Result alias for evaluation.
pub type Result<T, E = EvalError> = std::result::Result<T, E>;
