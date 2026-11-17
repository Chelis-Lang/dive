//! Typed control transport for the additive controlled binding entry points.

use dive_eval::{EvalError, HostControlConflict};

use crate::{BindError, Failure, Message};

/// A controlled binding failed, preserving startup output and run ownership.
///
/// This does not classify a resource or produce a checker verdict. The caller
/// must match the original interruption against its own active run control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlledBindError {
    /// The ordinary binding failure, with unchanged diagnostics and output.
    Binding(Box<Failure>),
    /// Evaluator work returned its driver's typed interruption.
    Interrupted {
        /// The original error, including its opaque identity and source origin.
        error: EvalError,
        /// Output before interruption, in original startup order.
        output: Vec<Message>,
    },
    /// Fallible evaluation or optional source-proof planning failed intrinsically.
    /// Controlled binding retains the original error. The plain boxed binding
    /// API has no exception channel and uses its existing fail-closed bug wrapper.
    EvaluationFailure {
        /// Original resource-limit or internal evaluator error.
        error: EvalError,
        /// Startup output produced before the failure, in original order.
        output: Vec<Message>,
    },
    /// The evaluator could not admit this control registration.
    ControlConflict {
        /// The registration error; it is not resource exhaustion.
        error: HostControlConflict,
        /// Output before registration failed.
        output: Vec<Message>,
    },
}

impl ControlledBindError {
    /// Startup output emitted before the failure, in order.
    #[must_use]
    pub fn output(&self) -> &[Message] {
        match self {
            Self::Binding(failure) => &failure.output,
            Self::Interrupted { output, .. }
            | Self::EvaluationFailure { output, .. }
            | Self::ControlConflict { output, .. } => output,
        }
    }

    #[allow(
        clippy::unnecessary_box_returns,
        reason = "preserve the uncontrolled public boxed-error contract without reallocating existing failures"
    )]
    pub(crate) fn legacy(self) -> Box<Failure> {
        match self {
            Self::Binding(failure) => failure,
            // The plain boxed error has no exception channel. Preserve its
            // fail-closed bug projection rather than minting a semantic refusal.
            Self::Interrupted { output, .. }
            | Self::EvaluationFailure { output, .. }
            | Self::ControlConflict { output, .. } => Box::new(Failure {
                error: BindError::bug("uncontrolled binding returned a controlled failure"),
                output,
            }),
        }
    }
}

impl std::fmt::Display for ControlledBindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(failure) => failure.fmt(f),
            Self::Interrupted { error, .. } | Self::EvaluationFailure { error, .. } => error.fmt(f),
            Self::ControlConflict { error, .. } => error.fmt(f),
        }
    }
}
impl std::error::Error for ControlledBindError {}

impl From<Box<Failure>> for ControlledBindError {
    fn from(failure: Box<Failure>) -> Self {
        Self::Binding(failure)
    }
}

#[derive(Debug)]
pub(crate) enum StepError {
    Binding(BindError),
    Interrupted(EvalError),
    EvaluationFailure(EvalError),
    ControlConflict(HostControlConflict),
}

impl StepError {
    pub(crate) fn with_output(self, output: Vec<Message>) -> ControlledBindError {
        match self {
            Self::Binding(error) => {
                ControlledBindError::Binding(Box::new(Failure { error, output }))
            }
            Self::Interrupted(error) => ControlledBindError::Interrupted { error, output },
            Self::EvaluationFailure(error) => {
                ControlledBindError::EvaluationFailure { error, output }
            }
            Self::ControlConflict(error) => ControlledBindError::ControlConflict { error, output },
        }
    }

    pub(crate) fn ordinary(self) -> Result<BindError, Self> {
        match self {
            Self::Binding(error) => Ok(error),
            other => Err(other),
        }
    }

    pub(crate) fn is_exception(&self) -> bool {
        matches!(self, Self::Interrupted(_) | Self::EvaluationFailure(_))
    }
}

impl From<BindError> for StepError {
    fn from(error: BindError) -> Self {
        Self::Binding(error)
    }
}
impl From<HostControlConflict> for StepError {
    fn from(error: HostControlConflict) -> Self {
        Self::ControlConflict(error)
    }
}

pub(crate) fn eval_error(
    controlled: bool,
    fallible_errors: bool,
    error: EvalError,
    legacy: impl FnOnce(EvalError) -> BindError,
) -> StepError {
    if (controlled || fallible_errors) && error.class() == dive_eval::ErrorClass::Interrupted {
        StepError::Interrupted(error)
    } else if fallible_errors
        && matches!(
            error.class(),
            dive_eval::ErrorClass::ResourceLimit | dive_eval::ErrorClass::Bug
        )
    {
        StepError::EvaluationFailure(error)
    } else {
        StepError::Binding(legacy(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallible_intrinsic_errors_retain_original_class_origin_and_output() {
        for original in [
            EvalError::resource_limit("actual allocation failure"),
            EvalError::bug("actual internal failure"),
        ] {
            let original = original.at(dive_ir::ExprId(17));
            let legacy = eval_error(false, false, original.clone(), |error| {
                BindError::unsupported(error.to_string())
            });
            assert!(matches!(legacy, StepError::Binding(_)));
            let output = vec![Message::Print("before".into())];
            let result = eval_error(false, true, original.clone(), |error| {
                BindError::unsupported(error.to_string())
            })
            .with_output(output.clone());
            assert!(
                matches!(result,ControlledBindError::EvaluationFailure{error,output:actual} if error==original&&actual==output)
            );
        }
    }
}
