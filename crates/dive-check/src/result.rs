//! The result of a check: verdict, statistics, counterexample, and how TLC
//! would classify and report it.

use std::time::Duration;

use dive_eval::EvalError;
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
        /// TLC's message (`TLC_POSTCONDITION_FALSE`).
        message: String,
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
        /// TLC's message (`TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT`, or
        /// `TLC_INITIAL_STATE` for an initial state).
        message: String,
    },
    /// Evaluation failed with an error TLC also raises (including a failed
    /// `Assert` and integer overflow), classified as TLC classifies it at
    /// `site` (WP-C4's error-class table, `class.rs`).
    EvaluationError {
        /// Where.
        site: ErrorSite,
        /// TLC's deciding code: what its run returns, which decides its exit
        /// status (`ModelChecker.modelCheck`). After an error while
        /// exploring, TLC replays the step with its call stack recorded, and
        /// the replay decides it (`TLC_NESTED_EXPRESSION`, or the code of the
        /// `EvalException` it meets again).
        code: u32,
        /// Its `EC` name.
        name: &'static str,
        /// The code of the error message TLC prints first: the site's
        /// wrapper (`TLC_INVARIANT_EVALUATION_FAILED`, `GENERAL`, ...) or the
        /// error's own code.
        reported: u32,
        /// The text of that message, as TLC prints it.
        message: String,
        /// Whether `code` is checked against captured TLC runs for this site
        /// and this kind of error. An unverified class is reported but not
        /// claimed.
        verified: bool,
        /// The evaluation error itself.
        cause: EvalError,
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
    /// The run's budget (`--time-limit`, `--max-memory`) was exhausted
    /// (ADR 0107). Never a verdict about the model; the statistics are those
    /// at the stop.
    Stopped(Box<crate::budget::Stopped>),
    /// A required process-memory observation failed; never a model verdict.
    BudgetMeasurement {
        /// Typed measurement failure, without an invented observed byte count.
        error: crate::budget::MeasurementError,
    },
    /// An internal invariant of dive failed: a bug, never a verdict.
    InternalError {
        /// What.
        message: String,
    },
}

/// How TLC classifies a result: its observable primary code, `EC` name,
/// and physical process exit status. A source-qualified Init replay may
/// retain GENERAL as primary while its diagnostic context supplies exit 75.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlcClass {
    /// The observable deciding message code, distinct from a replay exit code.
    pub code: u32,
    /// Its `EC` name.
    pub name: String,
    /// TLC's physical exit status, including qualified diagnostic replay paths.
    pub exit: u8,
    /// The code of the error message TLC prints first, where it differs
    /// from `code` (an evaluation error wrapped by its site); otherwise
    /// `code`.
    pub reported: u32,
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
            Self::Stopped(_) => "stopped",
            Self::InternalError { .. } | Self::BudgetMeasurement { .. } => "internal-error",
        }
    }

    /// The violated property, where there is one.
    #[must_use]
    pub fn property(&self) -> Option<&str> {
        match self {
            Self::InvariantViolated { property, .. }
            | Self::PropertyViolatedInitially { property }
            | Self::ActionPropertyViolated { property }
            | Self::PostconditionFalse { property, .. } => Some(property),
            _ => None,
        }
    }

    /// Whether this is a verdict about the model (as opposed to a refusal,
    /// a resource limit or a bug).
    #[must_use]
    pub const fn is_verdict(&self) -> bool {
        !matches!(
            self,
            Self::Unsupported { .. }
                | Self::ResourceLimit { .. }
                | Self::Stopped(_)
                | Self::BudgetMeasurement { .. }
                | Self::InternalError { .. }
        )
    }

    /// TLC's classification of this verdict, where TLC has one.
    #[must_use]
    pub fn tlc(&self) -> Option<TlcClass> {
        let class = |code: u32, name: &str, verified: bool| TlcClass {
            code,
            name: name.to_owned(),
            exit: tlc_exit_status(code),
            reported: code,
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
            // Fixture `ErPostFalse` (WP-C4).
            Self::PostconditionFalse { .. } => class(2404, "TLC_POSTCONDITION_FALSE", true),
            // The binding stage's errors are checked against TLC (WP-B1);
            // `TLC_STATES_AND_NO_NEXT_ACTION` (2115) is read from TLC's source
            // only (`ModelChecker.java:233-245`).
            Self::ModelError { code, name, .. } => class(*code, name, *code != 2115),
            // Fixtures `ErInitIncomplete` and `ErNextIncomplete` (WP-C4).
            Self::NotCompletelySpecified { initial: true, .. } => {
                class(2102, "TLC_INITIAL_STATE", true)
            }
            Self::NotCompletelySpecified { initial: false, .. } => {
                class(2109, "TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT", true)
            }
            // The error-class table (`class.rs`) decides the code and
            // whether it is verified.
            Self::EvaluationError {
                site,
                code,
                name,
                reported,
                verified,
                cause,
                ..
            } => {
                if crate::class::init_primary_general(*site, *code, *reported, *verified, cause) {
                    TlcClass {
                        code: 1000,
                        name: "GENERAL".to_owned(),
                        exit: 75,
                        reported: 1000,
                        verified: true,
                    }
                } else {
                    TlcClass {
                        reported: *reported,
                        ..class(*code, name, *verified)
                    }
                }
            }
            Self::Unsupported { .. }
            | Self::ResourceLimit { .. }
            | Self::Stopped(_)
            | Self::BudgetMeasurement { .. }
            | Self::InternalError { .. } => {
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
    /// The state's values, by `VarId`. Empty for a state that leaves
    /// variables unassigned, which only `text` describes.
    pub values: Vec<Value>,
    /// TLC's text of a state that leaves variables unassigned (`null` for
    /// them), the last state of a `TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT`
    /// counterexample; `None` otherwise.
    pub text: Option<String>,
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

/// Evidence for the separately budgeted transient predecessor log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PredecessorUsage {
    /// Retained capacity/confirmed complete disk writes before cleanup.
    pub storage: dive_store::predecessor::Usage,
    /// Explicit storage/reconstruction/cleanup failures.
    pub failures: Vec<String>,
    /// Search event reached before storage prevented certification.
    pub deciding_event_before_failure: Option<String>,
    /// Explicit cleanup completed. False for an unstarted or failed close.
    pub cleanup_completed: bool,
}

/// The result of a check.
#[derive(Clone, Debug)]
pub struct CheckResult {
    /// Original declared symmetry and effective temporal policy after binding.
    /// `None` means no model was successfully bound.
    pub symmetry_liveness: Option<dive_model::SymmetryLivenessResolution>,
    /// Opt-in transient disk seen-state operation and explicit cleanup facts.
    pub seen: Option<crate::SeenUsage>,
    /// Frontier-only encoded/disk retention and cleanup evidence.
    pub frontier: crate::FrontierUsage,
    /// Predecessor storage evidence; not a bound on total search memory.
    pub predecessor: PredecessorUsage,
    /// The verdict.
    pub verdict: Verdict,
    /// Whether a model error originated during binding or symmetry preparation,
    /// before the engine started. Engine model errors leave this false.
    pub binding_model_error: bool,
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
    /// The worker threads requested and used. Thread fields: the only part
    /// of a result, with [`CheckResult::elapsed`], that may differ between
    /// worker counts.
    pub workers: Workers,
    /// Wall-clock time of the check (binding and search).
    pub elapsed: Duration,
}

/// Worker threads requested and used (ADR 0015 §6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workers {
    /// Requested (`--workers`).
    pub requested: usize,
    /// Used for the search: at most the requested count, 1 in sequential
    /// mode, fewer if the system would not start more threads, 0 when the
    /// run ended before the search.
    pub used: usize,
    /// Whether the search ran in sequential mode: every level inline, with
    /// one logical worker evaluating in TLC's order.
    pub sequential: bool,
    /// Why fewer workers were used than requested.
    pub reason: Option<String>,
}

impl Workers {
    /// One worker, as requested.
    #[must_use]
    pub const fn one() -> Self {
        Self {
            requested: 1,
            used: 1,
            sequential: false,
            reason: None,
        }
    }
}
