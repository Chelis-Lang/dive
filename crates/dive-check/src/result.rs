//! The result of a check: verdict, statistics, counterexample, and how TLC
//! would classify and report it.

use std::time::Duration;

use dive_store::StoreMode;
use dive_value::Value;

/// Where an evaluation error was raised: the phase of TLC's search it
/// belongs to. TLC's error class and exit status depend on it (WP-C4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorSite {
    /// Processing the specification, before the assumptions.
    Startup,
    /// Evaluating an `ASSUME`.
    Assumption,
    /// Generating the initial states.
    Init,
    /// Generating the successors of a state.
    Next,
    /// Evaluating a `CONSTRAINT`.
    Constraint,
    /// Evaluating an `ACTION_CONSTRAINT`.
    ActionConstraint,
    /// Evaluating `VIEW`.
    View,
    /// Fingerprinting or encoding a state.
    Fingerprint,
    /// Evaluating an invariant.
    Invariant,
    /// Evaluating an implied initial predicate.
    ImpliedInit,
    /// Evaluating an implied action.
    ImpliedAction,
    /// Evaluating a `POSTCONDITION`.
    Postcondition,
}

impl ErrorSite {
    /// A short name for output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Assumption => "assumption",
            Self::Init => "initial-states",
            Self::Next => "next-state",
            Self::Constraint => "constraint",
            Self::ActionConstraint => "action-constraint",
            Self::View => "view",
            Self::Fingerprint => "fingerprint",
            Self::Invariant => "invariant",
            Self::ImpliedInit => "implied-init",
            Self::ImpliedAction => "implied-action",
            Self::Postcondition => "postcondition",
        }
    }
}

/// The verdict of a check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The search finished without finding a violation.
    Success,
    /// An invariant is violated, in an initial state (`initial`) or a later
    /// one.
    InvariantViolated {
        /// The invariant, as TLC names it.
        property: String,
        /// Whether the violating state is an initial state.
        initial: bool,
    },
    /// An implied initial predicate (a state-level part of a `PROPERTY`) is
    /// violated by an initial state.
    PropertyViolatedInitially {
        /// The property.
        property: String,
    },
    /// An implied action (`[][A]_v` in a `PROPERTY`) is violated by a step.
    ActionPropertyViolated {
        /// The property.
        property: String,
    },
    /// A reachable state has no successor at all.
    Deadlock,
    /// An `ASSUME` is false.
    AssumptionFalse {
        /// TLC's message.
        message: String,
    },
    /// A `POSTCONDITION` is not `TRUE` after a complete search.
    PostconditionFalse {
        /// The postcondition.
        property: String,
    },
    /// TLC's error while processing the model (configuration, constants,
    /// start-up evaluation), as the binding stage reports it.
    ModelError {
        /// TLC's message code.
        code: u32,
        /// Its `EC` name.
        name: String,
        /// TLC's message.
        message: String,
    },
    /// A successor leaves a variable unassigned.
    NotCompletelySpecified {
        /// Whether it is an initial state.
        initial: bool,
        /// The unassigned variables.
        variables: Vec<String>,
    },
    /// Evaluation failed with an error TLC also raises (including a failed
    /// `Assert` and integer overflow).
    EvaluationError {
        /// Where.
        site: ErrorSite,
        /// TLC's message code of the underlying error (`EC`).
        code: u32,
        /// Its `EC` name.
        name: &'static str,
        /// TLC's message.
        message: String,
    },
    /// dive refused, naming the feature. Never a verdict about the model.
    Unsupported {
        /// The feature.
        feature: String,
    },
    /// A resource limit (evaluation depth, memory) was hit. Never a verdict
    /// about the model.
    ResourceLimit {
        /// What.
        message: String,
    },
    /// An internal invariant of dive failed: a bug, never a verdict.
    InternalError {
        /// What.
        message: String,
    },
}

/// How TLC classifies a result: its deciding message code, that code's
/// `EC` name, and TLC's process exit status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlcClass {
    /// The deciding message code.
    pub code: u32,
    /// Its `EC` name.
    pub name: String,
    /// TLC's exit status (`EC.ExitStatus.errorConstantToExitStatus`).
    pub exit: u8,
    /// Whether dive reproduces TLC's class here, checked against a captured
    /// TLC run. An unverified class is reported but not claimed (WP-C4 owns
    /// the error-class table).
    pub verified: bool,
}

impl Verdict {
    /// The verdict's kebab-case name in dive's output.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::InvariantViolated { .. } => "invariant-violation",
            Self::PropertyViolatedInitially { .. } => "property-violation",
            Self::ActionPropertyViolated { .. } => "action-property-violation",
            Self::Deadlock => "deadlock",
            Self::AssumptionFalse { .. } => "assumption-failure",
            Self::PostconditionFalse { .. } => "postcondition-failure",
            Self::ModelError { .. } => "model-error",
            Self::NotCompletelySpecified { .. } => "incomplete-state",
            Self::EvaluationError { code: 2132, .. } => "assertion-failure",
            Self::EvaluationError {
                code: 2178 | 2265, ..
            } => "overflow",
            Self::EvaluationError { .. } => "evaluation-error",
            Self::Unsupported { .. } => "unsupported",
            Self::ResourceLimit { .. } => "resource-limit",
            Self::InternalError { .. } => "internal-error",
        }
    }

    /// The violated property, where there is one.
    #[must_use]
    pub fn property(&self) -> Option<&str> {
        match self {
            Self::InvariantViolated { property, .. }
            | Self::PropertyViolatedInitially { property }
            | Self::ActionPropertyViolated { property }
            | Self::PostconditionFalse { property } => Some(property),
            _ => None,
        }
    }

    /// Whether this is a verdict about the model (as opposed to a refusal,
    /// a resource limit or a bug).
    #[must_use]
    pub const fn is_verdict(&self) -> bool {
        !matches!(
            self,
            Self::Unsupported { .. } | Self::ResourceLimit { .. } | Self::InternalError { .. }
        )
    }

    /// TLC's classification of this verdict, where TLC has one.
    #[must_use]
    pub fn tlc(&self) -> Option<TlcClass> {
        let class = |code: u32, name: &str, verified: bool| TlcClass {
            code,
            name: name.to_owned(),
            exit: tlc_exit_status(code),
            verified,
        };
        Some(match self {
            Self::Success => class(2193, "TLC_SUCCESS", true),
            Self::InvariantViolated { initial: true, .. } => {
                class(2107, "TLC_INVARIANT_VIOLATED_INITIAL", true)
            }
            Self::InvariantViolated { initial: false, .. } => {
                class(2110, "TLC_INVARIANT_VIOLATED_BEHAVIOR", true)
            }
            Self::PropertyViolatedInitially { .. } => {
                class(2108, "TLC_PROPERTY_VIOLATED_INITIAL", true)
            }
            Self::ActionPropertyViolated { .. } => {
                class(2112, "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR", true)
            }
            Self::Deadlock => class(2114, "TLC_DEADLOCK_REACHED", true),
            Self::AssumptionFalse { .. } => class(2104, "TLC_ASSUMPTION_FALSE", true),
            // Read from TLC's source (`Tool.checkPostConditionWithContext`),
            // not yet checked against a captured run.
            Self::PostconditionFalse { .. } => class(2404, "TLC_POSTCONDITION_FALSE", false),
            // The binding stage's errors are checked against TLC (WP-B1).
            Self::ModelError { code, name, .. } => class(*code, name, true),
            Self::NotCompletelySpecified { initial: true, .. } => {
                class(2102, "TLC_INITIAL_STATE", false)
            }
            Self::NotCompletelySpecified { initial: false, .. } => {
                class(2109, "TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT", false)
            }
            Self::EvaluationError {
                site, code, name, ..
            } => {
                // Verified against captured TLC runs: a coded error raised
                // while generating successors is reported with its own code
                // (probe `EvalErrorMidLevel`, 2179; harness fixtures
                // `AssertFailure`, 2132, and `Overflow`, 2178); an assumption
                // that fails to evaluate is 2105 (fixtures of WP-C1);
                // start-up errors are reported as raised (WP-C1). Every
                // other site and an uncoded (`GENERAL`) error in `Next`,
                // which TLC reports as 1000 and then 2147 (fixture
                // `EvalError`), wait for the error-class table (WP-C4).
                let verified = match site {
                    ErrorSite::Next => *code != 1000,
                    ErrorSite::Startup | ErrorSite::Assumption => true,
                    _ => false,
                };
                class(*code, name, verified)
            }
            Self::Unsupported { .. } | Self::ResourceLimit { .. } | Self::InternalError { .. } => {
                return None;
            }
        })
    }
}

/// TLC's exit status for a deciding message code
/// (`EC.ExitStatus.errorConstantToExitStatus`, tlaplus `94d0c50`).
#[must_use]
pub const fn tlc_exit_status(code: u32) -> u8 {
    match code {
        2193 => 0,
        2253 => 77,
        2103 | 2109 | 2115 | 2147 => 75,
        2111 | 2146 => 76,
        2107 | 2110 => 12,
        2108 | 2112 | 2113 | 2116 => 13,
        2114 => 11,
        2104 | 2105 | 2404 | 2405 | 2780 => 10,
        2132 => 14,
        2171 | 3002 => 150,
        2222..=2245 | 2272..=2274 | 2280 | 2281 | 5001..=5007 => 151,
        _ => 255,
    }
}

/// State-space statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// States generated, duplicates and states outside the model included.
    pub generated: u64,
    /// Distinct states found.
    pub distinct: u64,
    /// States left on the queue.
    pub left_on_queue: u64,
    /// The depth of the search, as TLC reports it.
    pub depth: u64,
    /// Initial states generated.
    pub initial_generated: u64,
}

/// One state of a counterexample.
#[derive(Clone, Debug)]
pub struct TraceState {
    /// TLC's label: `<Initial predicate>` or the action's location label.
    pub label: String,
    /// The action's name, when the state was produced by a named action.
    pub action: Option<String>,
    /// The state's values, by `VarId`.
    pub values: Vec<Value>,
}

/// A counterexample: a finite behavior prefix.
#[derive(Clone, Debug, Default)]
pub struct Counterexample {
    /// The states, from an initial state.
    pub states: Vec<TraceState>,
}

/// The fingerprint-collision figures TLC prints on success, as their text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collision {
    /// "calculated (optimistic)".
    pub optimistic: String,
    /// "based on the actual fingerprints", when TLC computes it.
    pub actual: Option<String>,
}

/// The result of a check.
#[derive(Clone, Debug)]
pub struct CheckResult {
    /// The verdict.
    pub verdict: Verdict,
    /// Statistics at the stop, when the search started.
    pub stats: Option<Stats>,
    /// Whether TLC prints statistics for this result. TLC prints none for a
    /// stop while generating initial states (`ModelChecker.java:152-160`,
    /// probe `InitViolation`) or before the search.
    pub tlc_prints_stats: bool,
    /// The counterexample, for violations, deadlock and errors in a state.
    pub counterexample: Option<Counterexample>,
    /// Collision figures (fingerprint mode, on success).
    pub collision: Option<Collision>,
    /// The store used.
    pub store: StoreMode,
    /// The fingerprint polynomial index (fingerprint mode).
    pub fp_index: Option<usize>,
    /// Whether the fingerprints are TLC's own (`--compat=tlc`) or dive's.
    pub tlc_fingerprints: bool,
    /// The variable names, by `VarId`.
    pub var_names: Vec<String>,
    /// TLC's variable order, as `VarId` indices: states print in it.
    pub tlc_var_order: Vec<usize>,
    /// `Print`/`PrintT` output, in order (start-up, assumptions, search).
    pub output: Vec<String>,
    /// TLC's warnings while processing the model: code and text.
    pub warnings: Vec<(u32, String)>,
    /// Notes for the user: anything that qualifies the result.
    pub notes: Vec<String>,
    /// Wall-clock time of the check (binding and search).
    pub elapsed: Duration,
}
