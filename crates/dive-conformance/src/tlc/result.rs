//! The typed result of one TLC run, and its classification.
//!
//! Classification is driven by the `-tool` message stream; the exit code is
//! a cross-check (TLC exits 255 both for crashes and for overflow and most
//! module evaluation errors, so the exit code alone cannot classify).
//!
//! The rules, in order:
//! 1. A run that hit the harness timeout is [`Verdict::Timeout`].
//! 2. No exit code (killed by a signal), an unknown exit code, a malformed
//!    message stream, or a deciding code that neither pinned TLC declares is
//!    [`Verdict::Unclassified`], with the raw data kept.
//! 3. Exit 0 must come with `TLC_SUCCESS` (2193) and no error messages;
//!    exit 0 with error messages is [`Verdict::OracleError`].
//! 4. Otherwise the *candidates* are the error messages whose code
//!    `errorConstantToExitStatus` maps to the observed exit code. The first
//!    candidate that is not mere context (trace header, nested-expression
//!    stack) decides the verdict. No candidate means the exit code and the
//!    messages disagree: [`Verdict::OracleError`], both recorded. Exit codes
//!    152 and 153 are raised outside that mapping and classify as
//!    [`Verdict::OtherError`]. A complete, finite temporal counterexample
//!    may exit 12: `LiveCheck.printErrorTrace` prints 2116 but stores 2110
//!    as the checker error. This specific stream shape is checked separately.
//! 5. Parse failures (3002, 2171) are split into [`Verdict::ParseError`] and
//!    [`Verdict::SemanticError`] using SANY's untagged text; if neither
//!    marker is present the result is unclassified.
//! 6. When both a `-dumpTrace json` file and the message stream carry a
//!    counterexample, their lengths must agree, or the result is an oracle
//!    error.
//!
//! Before rule 2, JVM resource exhaustion (out of memory, stack overflow)
//! is [`Verdict::ResourceLimit`], which is never an oracle fact. In rule 4,
//! an exit code explained only by context messages is unclassified. After
//! classification: a success without state statistics, or any stderr line
//! outside [`BENIGN_STDERR`], turns an oracle fact into
//! [`Verdict::OracleError`], keeping the stderr in the note.

use super::codes::{
    CONTEXT_CODES, PARSE_FAILURE_CODES, ec_name, exit_status_name, exit_statuses_for,
    verdict_for_code,
};
use super::messages::{Message, OwnedTranscript, Transcript, class};
use crate::hash::sha256_bytes;
use serde::{Deserialize, Serialize};

/// Traces whose canonical JSON exceeds this many bytes are stored as a
/// digest and a length only.
pub const TRACE_SIZE_CAP: usize = 1_000_000;

/// The verdict class of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Exhaustive search (or simulation) finished without error.
    Success,
    /// An `ASSUME` is false or failed to evaluate.
    AssumptionFailure,
    /// A reachable state has no successor and deadlock checking is on.
    Deadlock,
    /// An invariant is violated.
    SafetyViolation,
    /// An action property (`[][A]_v`) is violated. TLC exits 13 for this,
    /// like a liveness violation, but reports a distinct message code.
    ActionPropertyViolation,
    /// A temporal property is violated.
    LivenessViolation,
    /// `TLC!Assert` failed.
    AssertionFailure,
    /// Evaluating the spec, an invariant or a property failed.
    EvaluationError,
    /// SANY rejected the spec at the syntax level, or a module is missing.
    ParseError,
    /// SANY rejected the spec in semantic analysis or level checking.
    SemanticError,
    /// The `.cfg` file is invalid.
    ConfigError,
    /// 32-bit integer overflow (TLC raises an error, it does not wrap).
    Overflow,
    /// Another error TLC declares; see the verdict code.
    OtherError,
    /// TLC's own outputs contradict each other; see the note.
    OracleError,
    /// The harness could not classify the run; raw data is kept.
    Unclassified,
    /// The harness timeout fired and the process tree was killed.
    Timeout,
    /// The JVM ran out of a resource (heap, stack): `SYSTEM_OUT_OF_MEMORY*`
    /// (1001-1003), `SYSTEM_STACK_OVERFLOW` (1005), or `GENERAL` (1000)
    /// reporting an `OutOfMemoryError`/`StackOverflowError`. That depends on
    /// `-Xmx`/`-Xss` and the machine, so it is never an oracle fact and never
    /// becomes a golden.
    ResourceLimit,
}

impl Verdict {
    /// Whether this verdict is a real oracle fact (as opposed to the harness
    /// failing to obtain or interpret one).
    #[must_use]
    pub fn is_oracle_fact(self) -> bool {
        !matches!(
            self,
            Self::OracleError | Self::Unclassified | Self::Timeout | Self::ResourceLimit
        )
    }

    /// The kebab-case name, as serialised.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::AssumptionFailure => "assumption-failure",
            Self::Deadlock => "deadlock",
            Self::SafetyViolation => "safety-violation",
            Self::ActionPropertyViolation => "action-property-violation",
            Self::LivenessViolation => "liveness-violation",
            Self::AssertionFailure => "assertion-failure",
            Self::EvaluationError => "evaluation-error",
            Self::ParseError => "parse-error",
            Self::SemanticError => "semantic-error",
            Self::ConfigError => "config-error",
            Self::Overflow => "overflow",
            Self::OtherError => "other-error",
            Self::OracleError => "oracle-error",
            Self::Unclassified => "unclassified",
            Self::Timeout => "timeout",
            Self::ResourceLimit => "resource-limit",
        }
    }
}

/// How the model was run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunMode {
    /// Breadth-first exhaustive search.
    Exhaustive,
    /// `-simulate`; counts are random and not comparable.
    Simulate,
}

/// State-space statistics as TLC reports them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    /// Distinct states found (`TLC_STATS`).
    pub distinct_states: Option<u64>,
    /// States generated (`TLC_STATS`, or `TLC_STATS_SIMU` in simulation).
    pub states_generated: Option<u64>,
    /// States left on the queue (`TLC_STATS`).
    pub states_left_on_queue: Option<u64>,
    /// Depth of the state graph search (`TLC_SEARCH_DEPTH`).
    pub depth: Option<u64>,
}

/// Fingerprint collision probability estimates, verbatim as TLC prints them
/// in `TLC_SUCCESS`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollisionProbability {
    /// "calculated (optimistic)".
    pub optimistic: Option<String>,
    /// "based on the actual fingerprints", when printed.
    pub observed: Option<String>,
}

/// How a counterexample ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum TraceEnding {
    /// A finite prefix (safety, deadlock, assertion and evaluation errors).
    Finite,
    /// A lasso: the last state loops back to state `back_to` (1-based).
    Lasso {
        /// Target of the back edge, if TLC printed its index.
        back_to: Option<u64>,
    },
    /// The behaviour ends by stuttering forever.
    Stuttering,
}

/// One state of a counterexample as printed in the message stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceStep {
    /// The action label, e.g. `<Initial predicate>` or `<Next line 5, ...>`.
    pub label: String,
    /// The state as TLC prints it (a conjunction of `var = value`).
    pub state: String,
}

/// A counterexample, if TLC printed one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    /// Number of states, not counting the lasso or stuttering marker.
    pub length: u64,
    /// How the trace ends.
    pub ending: TraceEnding,
    /// The `-dumpTrace json` document, verbatim, when the oracle supports it
    /// and it fits under [`TRACE_SIZE_CAP`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
    /// The states from the message stream, only when no JSON trace exists
    /// and they fit under [`TRACE_SIZE_CAP`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<TraceStep>>,
    /// Set when the trace content was too large to store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted: Option<OmittedTrace>,
    /// Set when TLC's `-dumpTrace json` document is unavailable although the
    /// oracle supports it: the trace pass diverged from the clean run (issue
    /// #35); the text gives the reason and a summary of the difference.
    /// `json` is then absent and `steps` holds the clean run's printed
    /// states; the document is never rebuilt or guessed. Such a trace
    /// supports length and ending comparisons only, never trace-content ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_unavailable: Option<String>,
}

/// A stand-in for a trace too large to store in a golden file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OmittedTrace {
    /// Why it was omitted.
    pub reason: String,
    /// Size of the canonical (compact, key-sorted) JSON in bytes.
    pub canonical_bytes: u64,
    /// SHA-256 of the canonical JSON.
    pub canonical_sha256: String,
}

/// Which `dive check` engine ran a model (`--engine`, ADR 0117), and why
/// `--engine auto` fell back to the reference engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineRun {
    /// `reference` or `core`.
    pub engine: String,
    /// The core's refusal, when `auto` ran the reference engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<EngineFallback>,
    /// The core's successor sample (`dive check --core-sample`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<CoreSample>,
}

/// The core's successor sample: sampled states whose successors were
/// compared with the reference evaluator's, action by action.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreSample {
    /// States sampled.
    pub states: u64,
    /// (state, action) pairs compared.
    pub compared: u64,
    /// Successors compared.
    pub successors: u64,
    /// Pairs whose successor multisets differ.
    pub mismatches: u64,
    /// The first mismatches, described.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shown: Vec<String>,
}

/// An `--engine auto` fallback to the reference engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineFallback {
    /// `static` (refused before the search) or `dynamic` (during it).
    pub stage: String,
    /// The feature the core refused.
    pub reason: String,
}

/// Everything the harness extracts from one TLC run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleResult {
    /// Private native producer observation for the exact D-005 exception.
    /// Not an oracle observable, wire field, execution proof or persisted fact.
    /// Only the strict native reader sets it after validating bound policy data.
    #[serde(skip)]
    pub native_unreduced_symmetry: bool,
    /// The `dive check` engine that produced the result, and its fallback
    /// under `--engine auto`. A producer observation like the field above:
    /// never a golden field, never compared.
    #[serde(skip)]
    pub engine: Option<EngineRun>,
    /// The verdict class.
    pub verdict: Verdict,
    /// The message code that decided the verdict.
    pub verdict_code: Option<i32>,
    /// The symbolic `EC` name of `verdict_code`; the finer error class
    /// compared alongside the verdict.
    pub error_class: Option<String>,
    /// The process exit code (`EC.ExitStatus`), if it exited normally.
    pub exit_code: Option<i32>,
    /// The `EC.ExitStatus` name of the exit code.
    pub exit_status: Option<String>,
    /// The violated invariant, property or assumption, where TLC names it.
    pub violated_property: Option<String>,
    /// State-space statistics.
    pub counts: Counts,
    /// Fingerprint collision probability estimates.
    pub fingerprint_collision: Option<CollisionProbability>,
    /// The counterexample.
    pub trace: Option<Trace>,
    /// Codes of every error- and bug-class message, in order.
    pub error_codes: Vec<i32>,
    /// Codes of every warning-class message, in order.
    pub warning_codes: Vec<i32>,
    /// Modules SANY could not find, from its untagged text.
    pub unresolved_modules: Vec<String>,
    /// Why the result is an oracle error or unclassified, or other caveats.
    pub note: Option<String>,
}

/// Inputs to [`classify`] besides the transcript.
#[derive(Debug, Clone, Default)]
pub struct RunFacts<'a> {
    /// Exit code, `None` if the process was killed by a signal.
    pub exit_code: Option<i32>,
    /// Whether the harness timeout fired.
    pub timed_out: bool,
    /// The `-dumpTrace json` file contents, if one was written.
    pub trace_json: Option<&'a str>,
    /// Simulation mode.
    pub simulate: bool,
    /// TLC's stderr. Anything outside [`BENIGN_STDERR`] makes the result an
    /// oracle error, with the stderr kept in the note.
    pub stderr: Option<&'a str>,
}

/// Stderr lines that are known and harmless. v1.7.4's module resolver prints
/// `File does not exist: ... while looking in these directories: ...` for
/// every module it cannot find; the parse failure itself is reported on
/// stdout.
pub const BENIGN_STDERR: &[&str] = &["File does not exist: "];

/// Stderr lines outside [`BENIGN_STDERR`].
#[must_use]
pub fn unexpected_stderr(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| !BENIGN_STDERR.iter().any(|b| l.starts_with(b)))
        .collect()
}

fn parse_u64(s: &str) -> Option<u64> {
    s.trim().trim_end_matches('.').replace(',', "").parse().ok()
}

/// Parses `TLC_STATS` / `TLC_STATS_SIMU` / `TLC_SEARCH_DEPTH`.
fn extract_counts(t: &Transcript) -> Counts {
    let mut counts = Counts::default();
    if let Some(m) = t.last(2199) {
        // "%1% states generated, %2% distinct states found, %3% states left on queue."
        let body = m
            .body
            .replace(" states generated, ", "|")
            .replace(" distinct states found, ", "|")
            .replace(" states left on queue.", "");
        if let [generated, distinct, queue] = body.split('|').collect::<Vec<_>>().as_slice() {
            counts.states_generated = parse_u64(generated);
            counts.distinct_states = parse_u64(distinct);
            counts.states_left_on_queue = parse_u64(queue);
        }
    }
    if let Some(m) = t.last(2210) {
        // "The number of states generated: %1%\nSimulation using seed %2% and aril %3%"
        counts.states_generated = m
            .body
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("The number of states generated:"))
            .and_then(parse_u64);
    }
    if let Some(m) = t.last(2194) {
        counts.depth = m
            .body
            .strip_prefix("The depth of the complete state graph search is ")
            .and_then(parse_u64);
    }
    counts
}

fn extract_collision(t: &Transcript) -> Option<CollisionProbability> {
    let m = t.first(2193)?;
    let field = |key: &str| {
        m.body
            .lines()
            .find_map(|l| l.trim().strip_prefix(key).map(|v| v.trim().to_owned()))
    };
    let optimistic = field("calculated (optimistic):");
    let observed = field("based on the actual fingerprints:");
    (optimistic.is_some() || observed.is_some()).then_some(CollisionProbability {
        optimistic,
        observed,
    })
}

fn between<'a>(s: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    s.strip_prefix(prefix)?.split_once(suffix).map(|(v, _)| v)
}

fn violated_property(m: &Message) -> Option<String> {
    let first = m.body.lines().next().unwrap_or_default();
    let v = match m.code {
        2107 => between(first, "Invariant ", " is violated by the initial state"),
        2110 => between(first, "Invariant ", " is violated."),
        2108 => between(first, "Property ", " is violated by the initial state"),
        2112 => between(first, "Action property ", " is violated."),
        2116 => between(first, "Temporal property ", " was violated.")
            .or_else(|| between(first, "Temporal properties ", " were violated.")),
        2104 => between(first, "Assumption ", " is false."),
        _ => None,
    };
    v.map(str::to_owned)
}

/// Splits a state-print body `"<n>: <label>\n<state>"`.
fn split_state(body: &str) -> (Option<u64>, String, String) {
    let (first, rest) = body.split_once('\n').unwrap_or((body, ""));
    let (idx, label) = first
        .split_once(": ")
        .map_or((None, first), |(i, l)| (i.trim().parse().ok(), l));
    (idx, label.to_owned(), rest.trim_end().to_owned())
}

/// The canonical form of a JSON value: compact, object keys sorted (serde
/// JSON's default map is ordered).
#[must_use]
pub fn canonical_json(v: &serde_json::Value) -> String {
    v.to_string()
}

fn omitted(reason: &str, canonical: &str) -> OmittedTrace {
    OmittedTrace {
        reason: reason.to_owned(),
        canonical_bytes: u64::try_from(canonical.len()).unwrap_or(u64::MAX),
        canonical_sha256: sha256_bytes(canonical.as_bytes()),
    }
}

/// Builds the trace from the message stream and the optional JSON dump.
/// Returns the trace and, if the two disagree, a description.
fn extract_trace(t: &Transcript, json_text: Option<&str>) -> (Option<Trace>, Option<String>) {
    let mut steps = Vec::new();
    let mut ending = TraceEnding::Finite;
    for m in t.messages() {
        match m.code {
            2217 => {
                let (_, label, state) = split_state(&m.body);
                steps.push(TraceStep { label, state });
            }
            2122 => {
                let (idx, ..) = split_state(&m.body);
                ending = TraceEnding::Lasso { back_to: idx };
            }
            2218 => ending = TraceEnding::Stuttering,
            // A violation in an initial state prints that state inside the
            // violation message ("... is violated by the initial state:\n<state>")
            // instead of as a separate state message.
            2107 | 2108 => {
                if let Some((_, state)) = m.body.split_once('\n')
                    && !state.trim().is_empty()
                {
                    steps.push(TraceStep {
                        label: "<Initial predicate>".to_owned(),
                        state: state.trim_end().to_owned(),
                    });
                }
            }
            _ => {}
        }
    }
    let json = match json_text.map(serde_json::from_str::<serde_json::Value>) {
        None => None,
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => return (None, Some(format!("-dumpTrace json file is not JSON: {e}"))),
    };
    if steps.is_empty() && json.is_none() {
        return (None, None);
    }
    let stream_len = u64::try_from(steps.len()).unwrap_or(u64::MAX);
    let mut problem = None;
    let json_len = json.as_ref().map(|v| {
        v.pointer("/counterexample/state")
            .and_then(serde_json::Value::as_array)
            .map(|a| u64::try_from(a.len()).unwrap_or(u64::MAX))
    });
    let length = match json_len {
        Some(Some(n)) => {
            if !steps.is_empty() && n != stream_len {
                problem = Some(format!(
                    "trace length differs between the message stream ({stream_len}) and -dumpTrace json ({n})"
                ));
            }
            n
        }
        Some(None) => {
            problem = Some("-dumpTrace json has no counterexample.state array".to_owned());
            stream_len
        }
        None => stream_len,
    };
    let mut trace = Trace {
        length,
        ending,
        json: None,
        steps: None,
        omitted: None,
        json_unavailable: None,
    };
    if let Some(v) = json {
        let canonical = canonical_json(&v);
        if canonical.len() > TRACE_SIZE_CAP {
            trace.omitted = Some(omitted(
                "-dumpTrace json exceeds the golden size cap",
                &canonical,
            ));
        } else {
            trace.json = Some(v);
        }
    } else {
        let canonical = serde_json::to_string(&steps).unwrap_or_default();
        if canonical.len() > TRACE_SIZE_CAP {
            trace.omitted = Some(omitted(
                "trace states exceed the golden size cap",
                &canonical,
            ));
        } else {
            trace.steps = Some(steps);
        }
    }
    (Some(trace), problem)
}

/// Module names SANY reports it cannot find.
#[must_use]
pub fn unresolved_modules(text: &str) -> Vec<String> {
    let mut v: Vec<String> = text
        .lines()
        .filter_map(|l| between(l.trim(), "Cannot find source file for module ", " "))
        .map(str::to_owned)
        .collect();
    v.sort();
    v.dedup();
    v
}

/// SANY's markers for a syntax-level failure.
pub const PARSE_MARKERS: &[&str] = &[
    "***Parse Error***",
    "Fatal errors while parsing",
    "Could not parse module",
    "Cannot find source file for module",
    "Lexical error",
];

/// SANY's marker for a semantic-analysis or level-checking failure.
pub const SEMANTIC_MARKERS: &[&str] = &["Semantic errors:"];

/// Splits a parse failure into parse vs semantic error from SANY's text.
fn parse_or_semantic(text: &str) -> Option<Verdict> {
    if PARSE_MARKERS.iter().any(|m| text.contains(m)) {
        Some(Verdict::ParseError)
    } else if SEMANTIC_MARKERS.iter().any(|m| text.contains(m)) {
        Some(Verdict::SemanticError)
    } else {
        None
    }
}

struct Decision {
    verdict: Verdict,
    code: Option<i32>,
    note: Option<String>,
}

impl Decision {
    fn new(verdict: Verdict, code: Option<i32>, note: Option<String>) -> Self {
        Self {
            verdict,
            code,
            note,
        }
    }
}

/// Resource-exhaustion codes: `SYSTEM_OUT_OF_MEMORY`,
/// `SYSTEM_OUT_OF_MEMORY_TOO_MANY_INIT`, `SYSTEM_OUT_OF_MEMORY_LIVENESS` and
/// `SYSTEM_STACK_OVERFLOW`.
pub const RESOURCE_CODES: &[i32] = &[1001, 1002, 1003, 1005];

/// JVM error names that mean a resource ran out.
const RESOURCE_ERRORS: &[&str] = &["OutOfMemoryError", "StackOverflowError"];

/// Detects JVM resource exhaustion: a resource code, a `GENERAL` (1000)
/// message naming `OutOfMemoryError` or `StackOverflowError`, or either name
/// on stderr.
fn resource_exhaustion(t: &Transcript, stderr: Option<&str>) -> Option<(Option<i32>, String)> {
    for m in t.messages() {
        if RESOURCE_CODES.contains(&m.code)
            || (m.code == 1000 && RESOURCE_ERRORS.iter().any(|e| m.body.contains(e)))
        {
            return Some((
                Some(m.code),
                format!(
                    "JVM resource exhausted ({}): {}",
                    ec_name(m.code).unwrap_or("?"),
                    m.body.lines().next().unwrap_or_default()
                ),
            ));
        }
    }
    let stderr = stderr?;
    RESOURCE_ERRORS
        .iter()
        .find(|e| stderr.contains(*e))
        .map(|e| (None, format!("JVM resource exhausted: {e} on stderr")))
}

/// The pinned `LiveCheck.printErrorTrace` safety-like temporal path prints
/// 2116, a header and a finite prefix, but records 2110 as the checker error.
/// Do not broaden the EC mapping: cyclic temporal failures must still exit 13.
fn finite_temporal_prefix(t: &Transcript, errors: &[&Message]) -> bool {
    if errors.iter().map(|m| m.code).collect::<Vec<_>>() != [2116, 2264]
        || t.first(2193).is_some()
        || t.first(2122).is_some()
        || t.first(2218).is_some()
        || t.first(2186).is_none()
        || t.first(2199).is_none()
    {
        return false;
    }
    let mut phase = 0;
    let mut states = 0_u64;
    for m in t.messages() {
        match m.code {
            2116 if phase == 0 && m.class == class::ERROR => phase = 1,
            2264 if phase == 1 && m.class == class::ERROR => phase = 2,
            2217 if phase == 2 && m.class == class::STATE => {
                let (index, label, state) = split_state(&m.body);
                states += 1;
                if index != Some(states)
                    || state.trim().is_empty()
                    || (states == 1 && label != "<Initial predicate>")
                {
                    return false;
                }
            }
            2116 | 2264 | 2217 => return false,
            _ => {}
        }
    }
    phase == 2 && states > 0
}

// Only the demonstrated Init catch/replay route can use NONE descendants.
// Public Transcript values cannot construct or retain this ownership authority.
fn init_replay_cause(parsed: &OwnedTranscript, facts: &RunFacts<'_>) -> Option<i32> {
    let t = parsed.transcript();
    if facts.exit_code != Some(75)
        || facts.simulate
        || facts.trace_json.is_some()
        || t.messages().any(|m| {
            m.class == class::STATE
                || ec_name(m.code).is_none()
                || !matches!(
                    m.class,
                    class::NONE | class::ERROR | class::WARNING | class::TLCBUG
                )
                || matches!(m.code, 2188 | 2190 | 2191 | 2193 | 2210)
        })
        || t.messages()
            .filter(|m| matches!(m.class, class::ERROR | class::TLCBUG))
            .map(|m| (m.code, m.class))
            .collect::<Vec<_>>()
            != [(1000, class::ERROR), (2103, class::ERROR)]
    {
        return None;
    }
    let phases = [2185, 2189, 1000, 2103, 2199, 2194, 2186]
        .map(|code| parsed.unique(code))
        .into_iter()
        .collect::<Option<Vec<_>>>()?;
    for (position, &index) in phases.iter().enumerate() {
        let frame = parsed.frames().get(index)?;
        let message = parsed.message(index)?;
        let expected_class = if matches!(message.code, 1000 | 2103) {
            class::ERROR
        } else {
            class::NONE
        };
        if frame.parent.is_some()
            || !frame.line_start
            || !frame.canonical
            || frame.end.is_none()
            || message.class != expected_class
            || (position > 0 && parsed.frames()[phases[position - 1]].end? > frame.start)
        {
            return None;
        }
    }
    if parsed.message(phases[1])?.body != "Computing initial states..."
        || !parsed.message(phases[0])?.body.starts_with("Starting... (")
        || !parsed.message(phases[0])?.body.ends_with(')')
        || !parsed.message(phases[6])?.body.starts_with("Finished in ")
        || !parsed.message(phases[6])?.body.ends_with(')')
        || parsed.frames()[phases[2]].own_body_len != 0
        || !parsed.message(phases[2])?.body.is_empty()
        || !replay_stack(&parsed.message(phases[3])?.body)
    {
        return None;
    }
    let counts = extract_counts(t);
    counts.distinct_states?;
    counts.states_generated?;
    counts.states_left_on_queue?;
    counts.depth?;
    owned_cause(parsed, phases[2])
}

fn owned_cause(parsed: &OwnedTranscript, general: usize) -> Option<i32> {
    let nested: Vec<usize> = parsed
        .frames()
        .iter()
        .enumerate()
        .filter_map(|(index, frame)| frame.parent.map(|_| index))
        .collect();
    let &first = nested.first()?;
    if parsed.frames()[first].parent != Some(general)
        || parsed.frames()[first].parent_body_prefix_len != 0
    {
        return None;
    }
    for &index in &nested {
        let frame = &parsed.frames()[index];
        if !frame.line_start
            || !frame.canonical
            || frame.end.is_none()
            || parsed.message(index)?.class != class::NONE
        {
            return None;
        }
    }
    let cause = match (parsed.message(first)?.code, nested.as_slice()) {
        (2215, [_]) => first,
        (2154, [_, leaf])
            if parsed.frames()[*leaf].parent == Some(first)
                && parsed.message(*leaf)?.code == 2178
                && parsed.frames()[*leaf].parent_body_prefix_len
                    == parsed.frames()[first].own_body_len
                && java_wrapper(&parsed.message(first)?.body) =>
        {
            *leaf
        }
        _ => return None,
    };
    if (0..parsed.frames().len()).any(|index| {
        parsed
            .message(index)
            .is_some_and(|message| matches!(message.code, 2154 | 2178 | 2215))
            && !nested.contains(&index)
    }) {
        return None;
    }
    let message = parsed.message(cause)?;
    match message.code {
        2178 if !message
            .body
            .strip_prefix("Overflow when computing ")?
            .is_empty() =>
        {
            Some(2178)
        }
        2215 => {
            let location = message
                .body
                .strip_prefix("TLC expected a boolean value, but did not find one. ")?;
            (!location.is_empty() && !location.contains('\n')).then_some(2215)
        }
        _ => None,
    }
}

fn java_wrapper(body: &str) -> bool {
    let Some(descriptor) = body
        .strip_prefix("Attempted to apply the operator overridden by the Java method\n")
        .and_then(|body| body.strip_suffix(",\nbut it produced the following error:"))
    else {
        return false;
    };
    !descriptor.is_empty() && !descriptor.contains('\n')
}

fn replay_stack(body: &str) -> bool {
    let Some(rows) = body.strip_prefix(
        "The error occurred when TLC was evaluating the nested\nexpressions at the following positions:\n"
    ) else {
        return false;
    };
    let rows = rows.trim_end_matches('\n');
    !rows.is_empty()
        && rows.lines().enumerate().all(|(index, row)| {
            let prefix = format!("{index}. Line ");
            row.strip_prefix(prefix.as_str())
                .is_some_and(stack_location)
        })
}

fn stack_location(row: &str) -> bool {
    let mut rest = row;
    for separator in [", column ", " to line ", ", column ", " in "] {
        let Some((number, tail)) = rest.split_once(separator) else {
            return false;
        };
        if !number.parse::<u32>().is_ok_and(|value| value > 0) {
            return false;
        }
        rest = tail;
    }
    !rest.is_empty()
}

#[allow(
    clippy::too_many_lines,
    reason = "the classification rules read best as one ordered sequence"
)]
fn decide(
    t: &Transcript,
    facts: &RunFacts<'_>,
    errors: &[&Message],
    owned: Option<&OwnedTranscript>,
) -> Decision {
    let codes: Vec<i32> = errors.iter().map(|m| m.code).collect();
    if facts.timed_out {
        return Decision::new(Verdict::Timeout, None, None);
    }
    if let Some((code, what)) = resource_exhaustion(t, facts.stderr) {
        return Decision::new(Verdict::ResourceLimit, code, Some(what));
    }
    let Some(exit) = facts.exit_code else {
        return Decision::new(
            Verdict::Unclassified,
            None,
            Some("TLC was terminated by a signal".to_owned()),
        );
    };
    if !t.anomalies.is_empty() {
        return Decision::new(
            Verdict::Unclassified,
            None,
            Some(format!(
                "malformed -tool stream: {}",
                t.anomalies.join("; ")
            )),
        );
    }
    if exit_status_name(exit).is_none() {
        return Decision::new(
            Verdict::Unclassified,
            codes.first().copied(),
            Some(format!("exit code {exit} is not an EC.ExitStatus value")),
        );
    }
    if exit == 0 {
        if !errors.is_empty() {
            return Decision::new(
                Verdict::OracleError,
                codes.first().copied(),
                Some(format!("exit code 0 but error messages {codes:?}")),
            );
        }
        if t.first(2193).is_some() {
            return Decision::new(Verdict::Success, Some(2193), None);
        }
        if facts.simulate && t.first(2210).is_some() {
            // Simulation reports TLC_STATS_SIMU rather than TLC_SUCCESS.
            return Decision::new(Verdict::Success, Some(2210), None);
        }
        return Decision::new(
            Verdict::Unclassified,
            None,
            Some("exit code 0 without a TLC_SUCCESS message".to_owned()),
        );
    }
    if exit == 12 && !facts.simulate && finite_temporal_prefix(t, errors) {
        return Decision::new(
            Verdict::LivenessViolation,
            Some(2116),
            Some("finite temporal prefix: LiveCheck prints 2116 but records safety error 2110 (exit 12)".to_owned()),
        );
    }
    let candidates: Vec<&&Message> = errors
        .iter()
        .filter(|m| exit_statuses_for(m.code).contains(&exit))
        .collect();
    if !candidates.is_empty() && candidates.iter().all(|m| CONTEXT_CODES.contains(&m.code)) {
        if let Some(cause) = owned.and_then(|parsed| init_replay_cause(parsed, facts)) {
            return Decision::new(
                Verdict::OtherError,
                Some(1000),
                Some(format!(
                    "GENERAL owns NONE cause {cause}; Init diagnostic replay records 2103 (physical exit 75)"
                )),
            );
        }
        // Only a context message (e.g. the nested-expression stack 2103 after
        // a GENERAL 1000) explains the exit code: the real error is not one
        // whose class we can name.
        return Decision::new(
            Verdict::Unclassified,
            codes.first().copied(),
            Some(format!(
                "exit code {exit} is explained only by context message(s) {:?}; error codes {codes:?}",
                candidates.iter().map(|m| m.code).collect::<Vec<_>>()
            )),
        );
    }
    let primary = candidates.iter().find(|m| !CONTEXT_CODES.contains(&m.code));
    let Some(primary) = primary else {
        if exit == 152 || exit == 153 {
            return Decision::new(
                Verdict::OtherError,
                codes.first().copied(),
                Some(format!(
                    "exit status {}",
                    exit_status_name(exit).unwrap_or_default()
                )),
            );
        }
        return Decision::new(
            Verdict::OracleError,
            codes.first().copied(),
            Some(format!(
                "exit code {exit} ({}) is not explained by any error message (codes {codes:?})",
                exit_status_name(exit).unwrap_or_default()
            )),
        );
    };
    let code = primary.code;
    if PARSE_FAILURE_CODES.contains(&code) {
        return match parse_or_semantic(&t.untagged_text()) {
            Some(v) => Decision::new(v, Some(code), None),
            None => Decision::new(
                Verdict::Unclassified,
                Some(code),
                Some(
                    "parse failure without a recognisable SANY parse or semantic error".to_owned(),
                ),
            ),
        };
    }
    match verdict_for_code(code) {
        Some(v) => Decision::new(v, Some(code), None),
        None => Decision::new(
            Verdict::Unclassified,
            Some(code),
            Some(format!(
                "message code {code} is not declared by any pinned TLC"
            )),
        ),
    }
}

/// Classifies one TLC run from its parsed stdout and the run facts.
#[must_use]
pub fn classify(t: &Transcript, facts: &RunFacts<'_>) -> OracleResult {
    classify_inner(t, facts, None)
}

/// Normalizes a parser-owned stream without exporting mutable framing evidence.
pub(crate) fn classify_owned(parsed: &OwnedTranscript, facts: &RunFacts<'_>) -> OracleResult {
    classify_inner(parsed.transcript(), facts, Some(parsed))
}

fn classify_inner(
    t: &Transcript,
    facts: &RunFacts<'_>,
    owned: Option<&OwnedTranscript>,
) -> OracleResult {
    let errors: Vec<&Message> = t
        .messages()
        .filter(|m| m.class == class::ERROR || m.class == class::TLCBUG)
        .collect();
    let decision = decide(t, facts, &errors, owned);
    let (trace, trace_problem) = extract_trace(t, facts.trace_json);
    let mut verdict = decision.verdict;
    let mut note = decision.note;
    if let Some(problem) = trace_problem {
        if verdict.is_oracle_fact() {
            verdict = Verdict::OracleError;
        }
        note = Some(note.map_or(problem.clone(), |n| format!("{n}; {problem}")));
    }
    let counts = extract_counts(t);
    if verdict == Verdict::Success {
        let has_stats = if facts.simulate {
            counts.states_generated.is_some()
        } else {
            counts.distinct_states.is_some() && counts.states_generated.is_some()
        };
        if !has_stats {
            verdict = Verdict::OracleError;
            note = Some("success reported without state statistics".to_owned());
        }
    }
    if let Some(stderr) = facts.stderr {
        let unexpected = unexpected_stderr(stderr);
        if !unexpected.is_empty() && verdict.is_oracle_fact() {
            verdict = Verdict::OracleError;
            let kept: String = stderr.chars().take(8000).collect();
            let problem = format!("unexpected output on stderr:\n{kept}");
            note = Some(note.map_or(problem.clone(), |n| format!("{n}; {problem}")));
        }
    }
    let violated = decision
        .code
        .and_then(|c| errors.iter().find(|m| m.code == c))
        .and_then(|m| violated_property(m));
    OracleResult {
        native_unreduced_symmetry: false,
        engine: None,
        verdict,
        verdict_code: decision.code,
        error_class: decision.code.and_then(ec_name).map(str::to_owned),
        exit_code: facts.exit_code,
        exit_status: facts
            .exit_code
            .and_then(exit_status_name)
            .map(str::to_owned),
        violated_property: violated,
        counts,
        fingerprint_collision: extract_collision(t),
        trace,
        error_codes: errors.iter().map(|m| m.code).collect(),
        warning_codes: t
            .messages()
            .filter(|m| m.class == class::WARNING)
            .map(|m| m.code)
            .collect(),
        unresolved_modules: unresolved_modules(&t.untagged_text()),
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::parse;
    use super::*;

    fn msg(code: i32, class: i32, body: &str) -> String {
        format!("@!@!@STARTMSG {code}:{class} @!@!@\n{body}\n@!@!@ENDMSG {code} @!@!@\n")
    }

    fn run(stdout: &str, exit: i32) -> OracleResult {
        classify(
            &parse(stdout),
            &RunFacts {
                exit_code: Some(exit),
                ..RunFacts::default()
            },
        )
    }

    #[test]
    fn success_requires_success_message() {
        let ok = msg(
            2193,
            0,
            "Model checking completed. No error has been found.\n  calculated (optimistic):  val = 2.2E-19",
        ) + &msg(
            2199,
            0,
            "1,234 states generated, 56 distinct states found, 0 states left on queue.",
        ) + &msg(
            2194,
            0,
            "The depth of the complete state graph search is 7.",
        );
        let r = run(&ok, 0);
        assert_eq!(r.verdict, Verdict::Success);
        assert_eq!(r.counts.states_generated, Some(1234));
        assert_eq!(r.counts.distinct_states, Some(56));
        assert_eq!(r.counts.depth, Some(7));
        assert_eq!(
            r.fingerprint_collision
                .and_then(|c| c.optimistic)
                .as_deref(),
            Some("val = 2.2E-19")
        );
        assert_eq!(run(&msg(2199, 0, "x"), 0).verdict, Verdict::Unclassified);
    }

    #[test]
    fn exit_zero_with_errors_is_oracle_error() {
        let s = msg(2193, 0, "done") + &msg(2110, 1, "Invariant Inv is violated.");
        assert_eq!(run(&s, 0).verdict, Verdict::OracleError);
    }

    #[test]
    fn inconsistent_exit_is_oracle_error() {
        let r = run(&msg(2110, 1, "Invariant Inv is violated."), 11);
        assert_eq!(r.verdict, Verdict::OracleError);
        assert!(r.note.is_some_and(|n| n.contains("not explained")));
    }

    #[test]
    fn unknown_exit_and_code_are_unclassified() {
        assert_eq!(run(&msg(2110, 1, "x"), 3).verdict, Verdict::Unclassified);
        let r = run(&msg(4242, 1, "new"), 255);
        assert_eq!(r.verdict, Verdict::Unclassified);
        assert_eq!(r.verdict_code, Some(4242));
    }

    #[test]
    fn context_codes_do_not_decide() {
        // Assert: 2132 (exit 14) then the nested-expression stack 2103 (75).
        let s = msg(2132, 1, "Assert failed")
            + &msg(2121, 1, "The behavior")
            + &msg(2103, 1, "0. Line");
        let r = run(&s, 14);
        assert_eq!(r.verdict, Verdict::AssertionFailure);
        assert_eq!(r.error_class.as_deref(), Some("TLC_VALUE_ASSERT_FAILED"));
        // Overflow: 2178 (255) with the same context.
        let s = msg(2178, 1, "Overflow") + &msg(2103, 1, "0. Line");
        assert_eq!(run(&s, 255).verdict, Verdict::Overflow);
    }

    #[test]
    fn parse_failures_split_on_text() {
        let s = "Semantic errors:\n\n*** Errors: 1\n".to_owned()
            + &msg(3002, 1, "Parsing or semantic analysis failed.");
        assert_eq!(run(&s, 150).verdict, Verdict::SemanticError);
        let s = "***Parse Error***\n".to_owned() + &msg(3002, 1, "failed");
        assert_eq!(run(&s, 150).verdict, Verdict::ParseError);
        assert_eq!(
            run(&msg(3002, 1, "failed"), 150).verdict,
            Verdict::Unclassified
        );
        let s = "Cannot find source file for module Foo imported in module Bar.\n".to_owned()
            + &msg(3002, 1, "failed");
        let r = run(&s, 150);
        assert_eq!(r.verdict, Verdict::ParseError);
        assert_eq!(r.unresolved_modules, vec!["Foo".to_owned()]);
    }

    #[test]
    fn timeout_and_signal() {
        let t = parse("");
        let r = classify(
            &t,
            &RunFacts {
                timed_out: true,
                ..RunFacts::default()
            },
        );
        assert_eq!(r.verdict, Verdict::Timeout);
        let r = classify(&t, &RunFacts::default());
        assert_eq!(r.verdict, Verdict::Unclassified);
    }

    #[test]
    fn trace_lengths_must_agree() {
        let s = msg(2110, 1, "Invariant I is violated.")
            + &msg(2217, 4, "1: <Initial predicate>\nx = 0\n")
            + &msg(2217, 4, "2: <Next>\nx = 1\n");
        let json = r#"{"counterexample":{"state":[[1,{"x":0}]]}}"#;
        let r = classify(
            &parse(&s),
            &RunFacts {
                exit_code: Some(12),
                trace_json: Some(json),
                ..RunFacts::default()
            },
        );
        assert_eq!(r.verdict, Verdict::OracleError);
        let ok = r#"{"counterexample":{"state":[[1,{"x":0}],[2,{"x":1}]]}}"#;
        let r = classify(
            &parse(&s),
            &RunFacts {
                exit_code: Some(12),
                trace_json: Some(ok),
                ..RunFacts::default()
            },
        );
        assert_eq!(r.verdict, Verdict::SafetyViolation);
        assert_eq!(r.violated_property.as_deref(), Some("I"));
        let trace = r.trace.ok_or("no trace");
        assert!(matches!(
            trace,
            Ok(Trace {
                length: 2,
                json: Some(_),
                steps: None,
                ..
            })
        ));
    }

    #[test]
    fn lasso_and_stutter_endings() {
        let s = msg(2116, 1, "Temporal property P was violated.")
            + &msg(2217, 4, "1: <Initial predicate>\nx = 0\n")
            + &msg(2122, 4, "1: Back to state: <Next>\n");
        let r = run(&s, 13);
        assert_eq!(r.verdict, Verdict::LivenessViolation);
        assert_eq!(r.violated_property.as_deref(), Some("P"));
        assert_eq!(
            r.trace.map(|t| (t.length, t.ending)),
            Some((1, TraceEnding::Lasso { back_to: Some(1) }))
        );
        let s = msg(2116, 1, "Temporal property P was violated.")
            + &msg(2217, 4, "1: <Initial predicate>\nx = 0\n")
            + &msg(2218, 4, "2: Stuttering");
        assert_eq!(
            run(&s, 13).trace.map(|t| t.ending),
            Some(TraceEnding::Stuttering)
        );
    }

    #[test]
    fn finite_temporal_prefix_matches_pinned_trace_and_rejects_mutants() {
        const CAPTURE: &str = include_str!(
            "../../tests/fixtures/temporal-prefix/tlc-2026.10.06.014338/QtEmpty.stdout"
        );
        let r = run(CAPTURE, 12);
        assert_eq!(r.verdict, Verdict::LivenessViolation);
        assert_eq!(r.verdict_code, Some(2116));
        assert_eq!(r.exit_code, Some(12));
        assert_eq!(r.violated_property.as_deref(), Some("P"));
        assert_eq!(
            r.error_class.as_deref(),
            Some("TLC_TEMPORAL_PROPERTY_VIOLATED")
        );
        assert_eq!(r.counts.states_generated, Some(2));
        assert_eq!(r.counts.distinct_states, Some(1));
        assert_eq!(r.counts.depth, Some(1));
        assert_eq!(
            r.trace.map(|t| (t.length, t.ending)),
            Some((1, TraceEnding::Finite))
        );
        // EC's general message-to-exit mapping remains unchanged.
        assert_eq!(exit_statuses_for(2116), &[13]);
        let mutants = [
            CAPTURE.replace("STARTMSG 2264:1", "STARTMSG 2264:0"),
            CAPTURE.replace("STARTMSG 2217:4", "STARTMSG 2217:0"),
            CAPTURE.replace("1: <Initial predicate>", "2: <Initial predicate>"),
            CAPTURE.replace("x = 0", ""),
            CAPTURE.replace("<Initial predicate>", "<Next>"),
            CAPTURE.replace("STARTMSG 2186:0", "STARTMSG 2186:1"),
            CAPTURE.to_owned() + &msg(2218, 4, "2: Stuttering"),
            CAPTURE.to_owned() + &msg(2122, 4, "1: Back to state: <Next>"),
            CAPTURE.to_owned() + &msg(2132, 1, "Assert failed"),
            CAPTURE.to_owned() + &msg(2193, 0, "Success"),
        ];
        for stdout in mutants {
            assert_eq!(run(&stdout, 12).verdict, Verdict::OracleError);
        }
        let minimal = msg(2116, 1, "Temporal property P was violated.")
            + &msg(2264, 1, "Counterexample")
            + &msg(2217, 4, "1: <Initial predicate>\nx = 0");
        assert_eq!(run(&minimal, 12).verdict, Verdict::OracleError);
        assert_eq!(run(CAPTURE, 0).verdict, Verdict::OracleError);
        // Exit 255 is explained only by the context header, not by 2116.
        assert_eq!(run(CAPTURE, 255).verdict, Verdict::Unclassified);
        let reordered = msg(2217, 4, "1: <Initial predicate>\nx = 0") + CAPTURE;
        assert_eq!(run(&reordered, 12).verdict, Verdict::OracleError);
    }

    #[test]
    fn oversized_trace_is_digested() {
        let big = format!(
            r#"{{"counterexample":{{"state":[[1,{{"x":"{}"}}]]}}}}"#,
            "a".repeat(TRACE_SIZE_CAP)
        );
        let s = msg(2110, 1, "Invariant I is violated.")
            + &msg(2217, 4, "1: <Initial predicate>\nx = 0\n");
        let r = classify(
            &parse(&s),
            &RunFacts {
                exit_code: Some(12),
                trace_json: Some(&big),
                ..RunFacts::default()
            },
        );
        let t = r.trace.ok_or("no trace");
        assert!(matches!(
            t,
            Ok(Trace {
                length: 1,
                json: None,
                omitted: Some(_),
                ..
            })
        ));
    }
}

#[cfg(test)]
#[path = "init_replay_tests.rs"]
mod init_replay_tests;
