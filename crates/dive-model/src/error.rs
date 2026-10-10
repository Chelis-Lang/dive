//! Errors and messages of the binding stage, with TLC's codes and texts.
//!
//! The texts are TLC's `tlc2.output.MP.getMessage` templates at tlaplus
//! `94d0c50`, with `%n%` replaced by the parameters as TLC passes them.

use std::fmt;

/// TLC's message codes (`tlc2.output.EC`) the binding stage reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(
    missing_docs,
    reason = "each variant is TLC's EC constant of that name"
)]
pub enum Ec {
    General,
    TlcIntegerTooBig,
    TlcCantHandleRealNumbers,
    TlcConfigValueNotAssignedToConstantParam,
    TlcConfigRhsIdAppearedAfterLhsId,
    TlcConfigWrongSubstitution,
    TlcConfigWrongSubstitutionNumberOfArgs,
    TlcConfigSubstitutionNonConstant,
    TlcConfigIdDoesNotAppearInSpec,
    TlcConfigNotBothSpecAndInit,
    TlcConfigIdRequiresNoArg,
    TlcConfigSpecifiedNotDefined,
    TlcConfigIdHasValue,
    TlcConfigMissingInit,
    TlcConfigMissingNext,
    TlcConfigIdMustNotBeConstant,
    TlcConfigOpNoArgs,
    TlcConfigOpNotInSpec,
    TlcConfigOpIsEqual,
    TlcConfigSpecIsTrivial,
    TlcCantHandleSubscript,
    TlcCantHandleConjunct,
    TlcCantHandleTooManyNextStateRels,
    TlcConfigPropertyNotCorrectlyDefined,
    TlcConfigPropertyActionLevel,
    TlcConfigPropertyActionLevelSquareASubV,
    TlcConfigPropertyActionLevelAngleASubV,
    TlcConfigOpArityInconsistent,
    TlcNoModules,
    TlcInvariantViolatedLevel,
    TlcInvariantConstantLevel,
    TlcSpecificationFeaturesTemporalQuantifier,
    TlcLiveCannotHandleFormula,
    TlcLiveFormulaTautology,
    TlcLiveFormulaStateLevel,
    TlcCouldNotDetermineSubscript,
    TlcSubscriptContainNoStateVar,
    TlcSymmetrySetTooSmall,
}

impl Ec {
    /// TLC's number.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::General => 1000,
            Self::TlcIntegerTooBig => 2265,
            Self::TlcCantHandleRealNumbers => 2244,
            Self::TlcConfigValueNotAssignedToConstantParam => 2222,
            Self::TlcConfigRhsIdAppearedAfterLhsId => 2223,
            Self::TlcConfigWrongSubstitution => 2224,
            Self::TlcConfigWrongSubstitutionNumberOfArgs => 2225,
            Self::TlcConfigSubstitutionNonConstant => 2281,
            Self::TlcConfigIdDoesNotAppearInSpec => 2226,
            Self::TlcConfigNotBothSpecAndInit => 2227,
            Self::TlcConfigIdRequiresNoArg => 2228,
            Self::TlcConfigSpecifiedNotDefined => 2229,
            Self::TlcConfigIdHasValue => 2230,
            Self::TlcConfigMissingInit => 2231,
            Self::TlcConfigMissingNext => 2232,
            Self::TlcConfigIdMustNotBeConstant => 2233,
            Self::TlcConfigOpNoArgs => 2234,
            Self::TlcConfigOpNotInSpec => 2235,
            Self::TlcConfigOpIsEqual => 2236,
            Self::TlcConfigSpecIsTrivial => 2237,
            Self::TlcCantHandleSubscript => 2238,
            Self::TlcCantHandleConjunct => 2239,
            Self::TlcCantHandleTooManyNextStateRels => 2240,
            Self::TlcConfigPropertyNotCorrectlyDefined => 2241,
            Self::TlcConfigPropertyActionLevel => 2272,
            Self::TlcConfigPropertyActionLevelSquareASubV => 2273,
            Self::TlcConfigPropertyActionLevelAngleASubV => 2274,
            Self::TlcConfigOpArityInconsistent => 2242,
            Self::TlcNoModules => 2245,
            Self::TlcInvariantViolatedLevel => 2146,
            Self::TlcInvariantConstantLevel => 2149,
            Self::TlcSpecificationFeaturesTemporalQuantifier => 2301,
            Self::TlcLiveCannotHandleFormula => 2213,
            Self::TlcLiveFormulaTautology => 2253,
            Self::TlcLiveFormulaStateLevel => 2255,
            Self::TlcCouldNotDetermineSubscript => 2139,
            Self::TlcSubscriptContainNoStateVar => 2140,
            Self::TlcSymmetrySetTooSmall => 2300,
        }
    }

    /// TLC's constant name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::General => "GENERAL",
            Self::TlcIntegerTooBig => "TLC_INTEGER_TOO_BIG",
            Self::TlcCantHandleRealNumbers => "TLC_CANT_HANDLE_REAL_NUMBERS",
            Self::TlcConfigValueNotAssignedToConstantParam => {
                "TLC_CONFIG_VALUE_NOT_ASSIGNED_TO_CONSTANT_PARAM"
            }
            Self::TlcConfigRhsIdAppearedAfterLhsId => "TLC_CONFIG_RHS_ID_APPEARED_AFTER_LHS_ID",
            Self::TlcConfigWrongSubstitution => "TLC_CONFIG_WRONG_SUBSTITUTION",
            Self::TlcConfigWrongSubstitutionNumberOfArgs => {
                "TLC_CONFIG_WRONG_SUBSTITUTION_NUMBER_OF_ARGS"
            }
            Self::TlcConfigSubstitutionNonConstant => "TLC_CONFIG_SUBSTITUTION_NON_CONSTANT",
            Self::TlcConfigIdDoesNotAppearInSpec => "TLC_CONFIG_ID_DOES_NOT_APPEAR_IN_SPEC",
            Self::TlcConfigNotBothSpecAndInit => "TLC_CONFIG_NOT_BOTH_SPEC_AND_INIT",
            Self::TlcConfigIdRequiresNoArg => "TLC_CONFIG_ID_REQUIRES_NO_ARG",
            Self::TlcConfigSpecifiedNotDefined => "TLC_CONFIG_SPECIFIED_NOT_DEFINED",
            Self::TlcConfigIdHasValue => "TLC_CONFIG_ID_HAS_VALUE",
            Self::TlcConfigMissingInit => "TLC_CONFIG_MISSING_INIT",
            Self::TlcConfigMissingNext => "TLC_CONFIG_MISSING_NEXT",
            Self::TlcConfigIdMustNotBeConstant => "TLC_CONFIG_ID_MUST_NOT_BE_CONSTANT",
            Self::TlcConfigOpNoArgs => "TLC_CONFIG_OP_NO_ARGS",
            Self::TlcConfigOpNotInSpec => "TLC_CONFIG_OP_NOT_IN_SPEC",
            Self::TlcConfigOpIsEqual => "TLC_CONFIG_OP_IS_EQUAL",
            Self::TlcConfigSpecIsTrivial => "TLC_CONFIG_SPEC_IS_TRIVIAL",
            Self::TlcCantHandleSubscript => "TLC_CANT_HANDLE_SUBSCRIPT",
            Self::TlcCantHandleConjunct => "TLC_CANT_HANDLE_CONJUNCT",
            Self::TlcCantHandleTooManyNextStateRels => "TLC_CANT_HANDLE_TOO_MANY_NEXT_STATE_RELS",
            Self::TlcConfigPropertyNotCorrectlyDefined => {
                "TLC_CONFIG_PROPERTY_NOT_CORRECTLY_DEFINED"
            }
            Self::TlcConfigPropertyActionLevel => "TLC_CONFIG_PROPERTY_ACTION_LEVEL",
            Self::TlcConfigPropertyActionLevelSquareASubV => {
                "TLC_CONFIG_PROPERTY_ACTION_LEVEL_SQUARE_A_SUB_V"
            }
            Self::TlcConfigPropertyActionLevelAngleASubV => {
                "TLC_CONFIG_PROPERTY_ACTION_LEVEL_ANGLE_A_SUB_V"
            }
            Self::TlcConfigOpArityInconsistent => "TLC_CONFIG_OP_ARITY_INCONSISTENT",
            Self::TlcNoModules => "TLC_NO_MODULES",
            Self::TlcInvariantViolatedLevel => "TLC_INVARIANT_VIOLATED_LEVEL",
            Self::TlcInvariantConstantLevel => "TLC_INVARIANT_CONSTANT_LEVEL",
            Self::TlcSpecificationFeaturesTemporalQuantifier => {
                "TLC_SPECIFICATION_FEATURES_TEMPORAL_QUANTIFIER"
            }
            Self::TlcLiveCannotHandleFormula => "TLC_LIVE_CANNOT_HANDLE_FORMULA",
            Self::TlcLiveFormulaTautology => "TLC_LIVE_FORMULA_TAUTOLOGY",
            Self::TlcLiveFormulaStateLevel => "TLC_LIVE_FORMULA_STATE_LEVEL",
            Self::TlcCouldNotDetermineSubscript => "TLC_COULD_NOT_DETERMINE_SUBSCRIPT",
            Self::TlcSubscriptContainNoStateVar => "TLC_SUBSCRIPT_CONTAIN_NO_STATE_VAR",
            Self::TlcSymmetrySetTooSmall => "TLC_SYMMETRY_SET_TOO_SMALL",
        }
    }

    /// TLC's message template (`MP.getMessage`).
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "one arm per message, TLC's texts")]
    pub fn template(self, nparams: usize) -> &'static str {
        match self {
            Self::General => "%1%",
            Self::TlcSymmetrySetTooSmall => {
                "The set%1% %2% %3% been defined to be a symmetry set but contain%4% less than two elements."
            }
            Self::TlcIntegerTooBig => "TLC can't handle a number this big.\n%1%",
            Self::TlcCantHandleRealNumbers => "TLC can't handle real numbers.\n%1%",
            Self::TlcConfigValueNotAssignedToConstantParam => {
                "The constant parameter %1% is not assigned a value by the configuration file."
            }
            Self::TlcConfigRhsIdAppearedAfterLhsId => {
                "In the configuration file, the identifier %1% appears\non the right-hand side of a <- after already appearing on the\nleft-hand side of one."
            }
            Self::TlcConfigWrongSubstitution => {
                "The configuration file substitutes for %1% with the undefined identifier %2%."
            }
            Self::TlcConfigWrongSubstitutionNumberOfArgs => {
                "The configuration file substitutes for %1% with %2% of different number of arguments."
            }
            Self::TlcConfigSubstitutionNonConstant => {
                "The configuration file substitutes constant %1% with non-constant %2%%3%"
            }
            Self::TlcConfigIdDoesNotAppearInSpec => {
                "In the configuration file, the identifier %1% does not appear in the specification."
            }
            Self::TlcConfigNotBothSpecAndInit => {
                "The configuration file cannot specify both INIT/NEXT and SPECIFICATION fields."
            }
            Self::TlcConfigIdRequiresNoArg => {
                if nparams == 2 {
                    "TLC requires %1% not to take any argument, but one was given: %2%"
                } else {
                    "TLC requires %1% not to take any argument."
                }
            }
            Self::TlcConfigSpecifiedNotDefined => {
                "The %1% %2% specified in the configuration file\nis not defined in the specification."
            }
            Self::TlcConfigIdHasValue => "The %1% of %2% is equal to %3%",
            Self::TlcConfigMissingInit => {
                "The configuration file did not specify the initial state predicate."
            }
            Self::TlcConfigMissingNext => {
                "The configuration file did not specify the next state predicate."
            }
            Self::TlcConfigIdMustNotBeConstant => "The %1% %2% cannot be a constant.",
            Self::TlcConfigOpNoArgs => "The operator %1% cannot take any argument.",
            Self::TlcConfigOpNotInSpec => "The operator %1% is not defined in the spec.",
            Self::TlcConfigOpIsEqual => {
                "The operator %1%, which equals %2%,\ncannot be used as a %3%"
            }
            Self::TlcConfigSpecIsTrivial => "The spec is trivially false because %1% is false.",
            Self::TlcCantHandleSubscript => "TLC cannot handle subscript %1%",
            Self::TlcCantHandleConjunct => "TLC cannot handle this conjunct of the spec:\n%1%",
            Self::TlcCantHandleTooManyNextStateRels => {
                "The specification contains more than one conjunct of the form [][Next]_v,\nbut TLC can handle only specifications with one next-state relation."
            }
            Self::TlcConfigPropertyNotCorrectlyDefined => {
                "The property %1% is not correctly defined."
            }
            Self::TlcConfigPropertyActionLevel => {
                "The formula %1% at %2% is an action-level formula (i.e., it contains no temporal operators). Only temporal-level (or state-level) formulas are allowed under PROPERTY or PROPERTIES. To check that action %1% holds in *every* step of the behavior, define a temporal property: Let A be the formula at %2%; rewrite A as [][A]_v, where v is a state function (typically the tuple of variables) (compare page 90 of Specifying Systems at https://lamport.azurewebsites.net/tla/book.html)."
            }
            Self::TlcConfigPropertyActionLevelSquareASubV => {
                "The formula %1% at %2% is an action-level formula (i.e., it contains no temporal operators). Only temporal-level (or state-level) formulas are allowed under PROPERTY or PROPERTIES. To check that action %1% holds in *every* step of the behavior, define a temporal property by applying the \"always\" temporal operator ([]) to the formula at %2% (compare page 90 of Specifying Systems at https://lamport.azurewebsites.net/tla/book.html)."
            }
            Self::TlcConfigPropertyActionLevelAngleASubV => {
                "The formula %1% at %2% is an action-level formula (i.e., it contains no temporal operators). Only temporal-level (or state-level) formulas are allowed under PROPERTY or PROPERTIES. To check that infinitely many %1% steps occur, define a temporal property by applying the \"always eventually\" temporal operator combination ([]<>) to the formula at %2% (compare page 91 of Specifying Systems at https://lamport.azurewebsites.net/tla/book.html)."
            }
            Self::TlcConfigOpArityInconsistent => {
                "The arity of the operator %1% is inconsistent in the configuration file."
            }
            Self::TlcNoModules => {
                "In the configuration file, the module name %1% is not a module in the specification."
            }
            Self::TlcInvariantViolatedLevel => {
                if nparams > 1 {
                    "The invariant %1% is not a state predicate (one with no primes or temporal operators).\nNote that a bug can cause TLC to incorrectly report this error.\nIf you believe your TLA+ or PlusCal specification to be correct,\nplease check if this bug described in LevelNode.java starting at line 590ff affects you."
                } else {
                    "The invariant %1% is not a state predicate (one with no primes or temporal operators)."
                }
            }
            Self::TlcInvariantConstantLevel => {
                "The invariant %1% is a constant-level formula (i.e., it contains no variables, primes, or temporal operators) and evaluates to %2%. To assert constant-level formulas in your spec, use ASSUME ConstInv. If you optionally want to give the assumption a name, write ASSUME YourAssumption == ConstInv instead. See https://explain.tlapl.us/assumptions-and-invariants for additional details."
            }
            Self::TlcSpecificationFeaturesTemporalQuantifier => {
                "TLC does not support temporal existential, nor universal, quantification over state variables."
            }
            Self::TlcLiveCannotHandleFormula => {
                if nparams > 1 {
                    "TLC cannot handle the temporal formula %1%:\n%2%"
                } else {
                    "TLC cannot handle the temporal formula %1%"
                }
            }
            Self::TlcLiveFormulaTautology => {
                "Temporal formula is a tautology (its negation is unsatisfiable)."
            }
            Self::TlcLiveFormulaStateLevel => {
                "The formula %1% is a state-level formula, but it was used as a PROPERTY (or PROPERTIES), where a temporal formula is typically expected. State-level formulas used as PROPERTY or PROPERTIES are only checked in the initial state. To verify that the formula %1% holds in all states of every behavior, use INVARIANT %1% instead. Alternatively, applying the \"always\" temporal operator ([]) to the state-level formula %1% changes it into a temporal formula, asserting that %1% holds in all states of every behavior. See https://explain.tlapl.us/invariants-and-properties for additional details."
            }
            Self::TlcCouldNotDetermineSubscript => {
                "TLC could not determine if the subscript of the next-state relation contains\nall state variables. Proceed with fingers crossed."
            }
            Self::TlcSubscriptContainNoStateVar => {
                "The subscript of the next-state relation specified by the specification\ndoes not seem to contain the state variable %1%"
            }
        }
    }

    /// The message with `params` substituted, as `MP.getMessage` builds it.
    #[must_use]
    pub fn render(self, params: &[String]) -> String {
        let mut text = self.template(params.len()).to_owned();
        for (i, p) in params.iter().enumerate() {
            text = text.replace(&format!("%{}%", i + 1), p);
        }
        text
    }
}

/// What kind of failure a [`BindError`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// TLC reports this error too, with this code (`EC`).
    Tlc(Ec),
    /// An evaluation error TLC reports while it processes the model (for
    /// example a value in the configuration TLC cannot normalize), with
    /// TLC's code.
    Eval {
        /// TLC's code.
        code: u32,
    },
    /// Something dive does not implement exactly. Never a verdict.
    Unsupported {
        /// The feature, by name.
        feature: String,
    },
    /// An internal inconsistency (a bug in dive), never a verdict.
    Bug,
}

/// An error binding the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindError {
    /// What kind of error.
    pub kind: ErrorKind,
    /// TLC's message text (for unsupported features and bugs, dive's).
    pub message: String,
}

impl BindError {
    /// A TLC error with its parameters.
    #[must_use]
    pub fn tlc(ec: Ec, params: &[&str]) -> Self {
        let params: Vec<String> = params.iter().map(|s| (*s).to_owned()).collect();
        Self {
            kind: ErrorKind::Tlc(ec),
            message: ec.render(&params),
        }
    }

    /// An unsupported feature.
    #[must_use]
    pub fn unsupported(feature: impl Into<String>) -> Self {
        let feature = feature.into();
        Self {
            message: format!("unsupported: {feature}"),
            kind: ErrorKind::Unsupported { feature },
        }
    }

    /// An internal error.
    #[must_use]
    pub fn bug(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Bug,
            message: message.into(),
        }
    }

    /// TLC's code, if TLC reports this error.
    #[must_use]
    pub fn code(&self) -> Option<u32> {
        match &self.kind {
            ErrorKind::Tlc(ec) => Some(ec.code()),
            ErrorKind::Eval { code } => Some(*code),
            ErrorKind::Unsupported { .. } | ErrorKind::Bug => None,
        }
    }

    /// The error as TLC prints it with `-tool`.
    #[must_use]
    pub fn tool_text(&self) -> String {
        match self.code() {
            Some(c) => format!(
                "@!@!@STARTMSG {c}:1 @!@!@\n{}\n@!@!@ENDMSG {c} @!@!@",
                self.message
            ),
            None => self.message.clone(),
        }
    }
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Tlc(ec) => write!(f, "{} ({}): {}", ec.name(), ec.code(), self.message),
            ErrorKind::Eval { code } => write!(f, "evaluation error ({code}): {}", self.message),
            ErrorKind::Unsupported { .. } | ErrorKind::Bug => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for BindError {}

/// Something TLC prints while it processes the model, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Output of `Print` or `PrintT` (one `println`).
    Print(String),
    /// A warning, with TLC's code.
    Warning {
        /// TLC's code.
        code: Ec,
        /// TLC's text.
        message: String,
    },
}

impl Message {
    /// A warning with its parameters.
    #[must_use]
    pub fn warning(code: Ec, params: &[&str]) -> Self {
        let params: Vec<String> = params.iter().map(|s| (*s).to_owned()).collect();
        Self::Warning {
            message: code.render(&params),
            code,
        }
    }
}

/// A failed binding: the error, and what TLC printed before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The error.
    pub error: BindError,
    /// What was printed before it, in order.
    pub output: Vec<Message>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for Failure {}
