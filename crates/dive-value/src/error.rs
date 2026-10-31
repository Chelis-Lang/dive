//! Value-layer errors with TLC's error classes and message texts.

use std::fmt;

/// The class of a value-layer error.
///
/// Specification error classes map to TLC's error codes (`tlc2.output.EC`), see
/// [`ErrorKind::tlc_code`]. [`ErrorKind::Interrupted`] is instead typed host
/// control flow; its legacy numeric accessor is not a TLC-error certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Typed cooperative host interruption transported through a deferred
    /// evaluator provider. This is not a TLC error or a resource refusal.
    Interrupted {
        /// Process-unique, nonreused identity of the evaluator's pending
        /// deferred-provider receipt. It is not a TLC error code.
        receipt: u32,
    },
    /// An evaluator error with sealed native-cause provenance. Only its
    /// original evaluator can recover the error; message text is insufficient.
    DeferredError {
        /// Process-unique, nonreused pending evaluator receipt.
        receipt: u32,
    },
    /// Two values that TLC refuses to compare or to test for equality
    /// (`Assert.fail` in a `compareTo`/`equals` method, `EC.GENERAL`).
    Incomparable,
    /// A membership test that TLC refuses (`member` methods, `EC.GENERAL`).
    Membership,
    /// A membership test in an infinite built-in set refused by its module
    /// (`EC.TLC_MODULE_CHECK_MEMBER_OF`).
    ModuleMembership,
    /// A comparison refused by a built-in infinite set (`EC.TLC_MODULE_COMPARE_VALUE`).
    ModuleCompare,
    /// 32-bit integer overflow (`EC.TLC_MODULE_OVERFLOW`).
    Overflow,
    /// An integer literal above 2^31 - 1 (`EC.TLC_INTEGER_TOO_BIG`).
    IntegerTooBig,
    /// `\div` by zero (`EC.TLC_MODULE_DIVISION_BY_ZERO`).
    DivisionByZero,
    /// `0^0` (`EC.TLC_MODULE_NULL_POWER_NULL`).
    NullPowerNull,
    /// An argument of the wrong kind or range (`EC.TLC_MODULE_ARGUMENT_ERROR`).
    Argument,
    /// A set too large to be represented or enumerated (`EC.GENERAL`).
    TooLarge,
    /// A set that cannot be enumerated because it is infinite or symbolic (`EC.GENERAL`).
    NotEnumerable,
    /// A duplicate in a function domain or a record's fields (`EC.GENERAL`).
    Duplicate,
    /// A value that cannot be fingerprinted, such as `Nat` (`EC.GENERAL`).
    NotFingerprintable,
    /// A value that cannot be converted to TLC's JSON form.
    Json,
    /// Bytes that are not a valid value encoding ([`crate::codec`]). This is an
    /// internal error, never a property of the spec: dive only decodes what it encoded.
    Codec,
    /// A dive-only limit, such as a big-integer result too large to allocate.
    /// TLC has no counterpart (it would have overflowed long before).
    DiveLimit,
}

impl ErrorKind {
    /// The TLC `EC` code number TLC reports for this class.
    #[must_use]
    pub const fn tlc_code(self) -> u32 {
        match self {
            Self::ModuleMembership => 2158,
            Self::ModuleCompare => 2155,
            Self::Overflow => 2178,
            Self::IntegerTooBig => 2265,
            Self::DivisionByZero => 2179,
            Self::NullPowerNull => 2180,
            Self::Argument => 2169,
            Self::Interrupted { .. }
            | Self::DeferredError { .. }
            | Self::Incomparable
            | Self::Membership
            | Self::TooLarge
            | Self::NotEnumerable
            | Self::Duplicate
            | Self::NotFingerprintable
            | Self::Json
            | Self::Codec
            | Self::DiveLimit => 1000,
        }
    }

    /// The TLC `EC` constant name for [`ErrorKind::tlc_code`].
    #[must_use]
    pub const fn tlc_name(self) -> &'static str {
        match self {
            Self::ModuleMembership => "TLC_MODULE_CHECK_MEMBER_OF",
            Self::ModuleCompare => "TLC_MODULE_COMPARE_VALUE",
            Self::Overflow => "TLC_MODULE_OVERFLOW",
            Self::IntegerTooBig => "TLC_INTEGER_TOO_BIG",
            Self::DivisionByZero => "TLC_MODULE_DIVISION_BY_ZERO",
            Self::NullPowerNull => "TLC_MODULE_NULL_POWER_NULL",
            Self::Argument => "TLC_MODULE_ARGUMENT_ERROR",
            _ => "GENERAL",
        }
    }

    /// The stable dive error code (`DIVE-Vnnnn`) for this class.
    #[must_use]
    pub const fn dive_code(self) -> &'static str {
        match self {
            Self::Interrupted { .. } => "DIVE-V0031",
            Self::DeferredError { .. } => "DIVE-V0032",
            Self::Incomparable => "DIVE-V0001",
            Self::Membership => "DIVE-V0002",
            Self::ModuleMembership => "DIVE-V0003",
            Self::ModuleCompare => "DIVE-V0004",
            Self::Overflow => "DIVE-V0010",
            Self::IntegerTooBig => "DIVE-V0011",
            Self::DivisionByZero => "DIVE-V0012",
            Self::NullPowerNull => "DIVE-V0013",
            Self::Argument => "DIVE-V0014",
            Self::TooLarge => "DIVE-V0020",
            Self::NotEnumerable => "DIVE-V0021",
            Self::Duplicate => "DIVE-V0022",
            Self::NotFingerprintable => "DIVE-V0023",
            Self::Json => "DIVE-V0024",
            Self::DiveLimit => "DIVE-V0030",
            Self::Codec => "DIVE-V0040",
        }
    }
}

/// A value-layer error: a class plus the exact message TLC prints for it.
///
/// The message is the text TLC puts in the error report body. For example, comparing
/// `1` with `"a"` while normalizing a set gives
/// `Attempted to compare integer 1 with non-integer:\n"a"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueError {
    kind: ErrorKind,
    // Keep ordinary Result slots small. This allocation occurs only when an
    // actual error is created, never for a successful value or comparison.
    details: Box<Details>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Details {
    message: String,
    receiver: Option<crate::ReceiverFailure>,
}

impl ValueError {
    /// Creates an error of the given class with the given message.
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            details: Box::new(Details {
                message: message.into(),
                receiver: None,
            }),
        }
    }

    /// Actual receiver-method provenance; arbitrary messages carry none.
    #[must_use]
    pub fn receiver_failure(&self) -> Option<crate::ReceiverFailure> {
        self.details.receiver
    }

    pub(crate) fn caught_by(mut self, function: &crate::Fcn) -> Self {
        if !matches!(
            self.kind,
            ErrorKind::Interrupted { .. } | ErrorKind::DiveLimit | ErrorKind::Codec
        ) && self
            .details
            .receiver
            .is_none_or(|failure| !failure.sourced())
            && let Some(failure) = crate::ReceiverFailure::equals(function)
        {
            self.details.receiver = Some(failure);
        }
        self
    }

    /// The error class.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The TLC-format message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.details.message
    }
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.dive_code(), self.details.message)
    }
}

impl std::error::Error for ValueError {}

/// Result alias for value-layer operations.
pub type Result<T, E = ValueError> = std::result::Result<T, E>;
