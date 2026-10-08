//! Comparing a result against a golden (ADR 0004).
//!
//! Goldens come from TLC with one worker. Every implementation is compared
//! against them on:
//! - the verdict class and the error class (the `EC` name of the deciding
//!   message);
//! - the violated property, when the golden names one;
//! - distinct, generated and queued state counts, and depth;
//! - the counterexample's length and how it ends (finite, lasso,
//!   stuttering).
//!
//! The counterexample's content is never compared. Implementations other
//! than TLC must instead report that their counterexample was validated
//! (`Implementation::trace_validated`).
//!
//! [`ComparePolicy::oracle_leniency`] applies only when the implementation is
//! itself a TLC oracle run with more than one worker. TLC's multi-worker
//! search is only approximately breadth-first (ADR 0004). So for such runs:
//! - counts are compared only if the oracle explored the whole state space
//!   (`success`, `liveness-violation`);
//! - depth, queue, counterexample and violated property are not compared.
//!
//! With four workers TLC even named a different set of violated temporal
//! properties (Examples `MCDistributedReplicatedLog`: `AllExtending and
//! InSync` vs `InSync`). Implementations other than TLC get no leniency: they
//! must match the single-worker golden at every worker count.
//!
//! [`ComparePolicy::may_omit_property`] lets an oracle that does not name
//! violated temporal properties (v1.7.4) omit the name without a diff.
//! Anyone else omitting a property the golden names is a diff.

use crate::corpus::ManifestExpectation;
use crate::sany::SanyResult;
use crate::tlc::result::{OracleResult, TraceEnding, Verdict};
use serde::{Deserialize, Serialize};

/// The outcome of one model or module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The implementation matches the golden on every compared field.
    Agree,
    /// At least one compared field differs; see the diff.
    Disagree,
    /// The implementation explicitly refused.
    Unsupported,
    /// No trustworthy oracle fact: the golden or an oracle run is missing,
    /// contradictory or unclassifiable.
    OracleError,
    /// The run hit the harness timeout.
    Timeout,
    /// Excluded by policy (symbolic, generate).
    OutOfScope,
    /// The implementation ran under a result-changing option (ADR 0010:
    /// TLC-compatible by default, better by option). Its result is not
    /// comparable with TLC goldens and is never counted as `agree`.
    DivergenceMode,
    /// A disagreement listed in `conformance/expected-divergences.toml` for
    /// the `dive` profile (ADR 0010). Never counted as `agree`.
    ExpectedDivergence,
}

impl Outcome {
    /// All outcomes, in report order.
    pub const ALL: [Self; 8] = [
        Self::Agree,
        Self::Disagree,
        Self::Unsupported,
        Self::OracleError,
        Self::Timeout,
        Self::OutOfScope,
        Self::DivergenceMode,
        Self::ExpectedDivergence,
    ];

    /// The kebab-case name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agree => "agree",
            Self::Disagree => "disagree",
            Self::Unsupported => "unsupported",
            Self::OracleError => "oracle-error",
            Self::Timeout => "timeout",
            Self::OutOfScope => "out-of-scope",
            Self::DivergenceMode => "divergence-mode",
            Self::ExpectedDivergence => "expected-divergence",
        }
    }
}

/// One differing field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDiff {
    /// Field name, e.g. `counts.distinct_states`.
    pub field: String,
    /// The golden's value.
    pub expected: serde_json::Value,
    /// The observed value.
    pub actual: serde_json::Value,
}

/// What may be compared for one run.
#[derive(Debug, Clone, Copy)]
pub struct CompareContext {
    /// Worker count of the observed run.
    pub workers: u32,
    /// Whether state counts are comparable at all for this model (false in
    /// simulation).
    pub counts_comparable: bool,
}

/// How strict a comparison is; see the module docs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparePolicy {
    /// Tolerate TLC's own multi-worker variance. Only for oracle
    /// implementations (`Implementation::is_oracle`).
    pub oracle_leniency: bool,
    /// The observed side may omit the violated property's name. Only for
    /// oracles that lack the capability.
    pub may_omit_property: bool,
}

fn push<T: Serialize + PartialEq>(out: &mut Vec<FieldDiff>, field: &str, expected: &T, actual: &T) {
    if expected != actual {
        out.push(FieldDiff {
            field: field.to_owned(),
            expected: serde_json::to_value(expected).unwrap_or(serde_json::Value::Null),
            actual: serde_json::to_value(actual).unwrap_or(serde_json::Value::Null),
        });
    }
}

fn ending_kind(r: &OracleResult) -> Option<&'static str> {
    r.trace.as_ref().map(|t| match t.ending {
        TraceEnding::Finite => "finite",
        TraceEnding::Lasso { .. } => "lasso",
        TraceEnding::Stuttering => "stuttering",
    })
}

/// Field-by-field differences under the strict policy (no leniency), as
/// for any implementation that is not TLC.
#[must_use]
pub fn diff_results(
    golden: &OracleResult,
    observed: &OracleResult,
    ctx: CompareContext,
) -> Vec<FieldDiff> {
    diff_results_with(golden, observed, ctx, ComparePolicy::default())
}

/// Field-by-field differences between a golden result and an observed one.
#[must_use]
pub fn diff_results_with(
    golden: &OracleResult,
    observed: &OracleResult,
    ctx: CompareContext,
    policy: ComparePolicy,
) -> Vec<FieldDiff> {
    let mut d = Vec::new();
    let lenient = policy.oracle_leniency && ctx.workers > 1;
    push(&mut d, "verdict", &golden.verdict, &observed.verdict);
    push(
        &mut d,
        "error_class",
        &golden.error_class,
        &observed.error_class,
    );
    let omitted = observed.violated_property.is_none() && policy.may_omit_property;
    if golden.violated_property.is_some() && !omitted && !lenient {
        push(
            &mut d,
            "violated_property",
            &golden.violated_property,
            &observed.violated_property,
        );
    }
    let complete = matches!(
        golden.verdict,
        Verdict::Success | Verdict::LivenessViolation
    );
    if ctx.counts_comparable && (!lenient || complete) {
        push(
            &mut d,
            "counts.distinct_states",
            &golden.counts.distinct_states,
            &observed.counts.distinct_states,
        );
        push(
            &mut d,
            "counts.states_generated",
            &golden.counts.states_generated,
            &observed.counts.states_generated,
        );
    }
    if ctx.counts_comparable && !lenient {
        push(
            &mut d,
            "counts.states_left_on_queue",
            &golden.counts.states_left_on_queue,
            &observed.counts.states_left_on_queue,
        );
        push(
            &mut d,
            "counts.depth",
            &golden.counts.depth,
            &observed.counts.depth,
        );
        push(
            &mut d,
            "trace.length",
            &golden.trace.as_ref().map(|t| t.length),
            &observed.trace.as_ref().map(|t| t.length),
        );
        push(
            &mut d,
            "trace.ending",
            &ending_kind(golden),
            &ending_kind(observed),
        );
    }
    d
}

/// Field-by-field differences between SANY results: acceptance and verdict.
#[must_use]
pub fn diff_sany(golden: &SanyResult, observed: &SanyResult) -> Vec<FieldDiff> {
    let mut d = Vec::new();
    push(&mut d, "accepted", &golden.accepted, &observed.accepted);
    push(&mut d, "verdict", &golden.verdict, &observed.verdict);
    push(
        &mut d,
        "unresolved_modules",
        &golden.unresolved_modules,
        &observed.unresolved_modules,
    );
    d
}

/// Per-worker-count facts for the invariance check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerPoint {
    /// Worker count.
    pub workers: u32,
    /// Verdict.
    pub verdict: Verdict,
    /// Distinct states.
    pub distinct_states: Option<u64>,
    /// Generated states.
    pub states_generated: Option<u64>,
    /// Depth.
    pub depth: Option<u64>,
    /// The violated property, as named.
    pub violated_property: Option<String>,
}

/// TLC's own worker-count invariance for one model (ADR 0004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerInvariance {
    /// The observations.
    pub points: Vec<WorkerPoint>,
    /// Whether the verdict was the same at every worker count.
    pub verdict_invariant: bool,
    /// Whether distinct and generated counts were the same at every worker
    /// count. `None` when not applicable (the run did not explore the full
    /// state space, so counts are expected to vary).
    pub counts_invariant: Option<bool>,
    /// Whether the reported depth varied (expected with several workers;
    /// reported, not counted as a disagreement).
    pub depth_varies: bool,
    /// Whether the named violated property varied (reported, not counted as
    /// a disagreement).
    pub property_varies: bool,
    /// Points left out of the judgements because they are not oracle facts
    /// (timeout, resource limit, unclassified, oracle error).
    pub excluded: usize,
}

/// Summarises worker invariance over `points`.
#[must_use]
pub fn worker_invariance(points: Vec<WorkerPoint>) -> WorkerInvariance {
    // Timeouts, resource limits and unclassified runs are not oracle facts;
    // they are left out of the variance judgements (they are still listed).
    let facts: Vec<&WorkerPoint> = points
        .iter()
        .filter(|p| p.verdict.is_oracle_fact())
        .collect();
    let verdict_invariant = facts.windows(2).all(|w| w[0].verdict == w[1].verdict);
    let complete = verdict_invariant
        && facts
            .first()
            .is_some_and(|p| matches!(p.verdict, Verdict::Success | Verdict::LivenessViolation));
    let counts_invariant = complete.then(|| {
        facts.windows(2).all(|w| {
            w[0].distinct_states == w[1].distinct_states
                && w[0].states_generated == w[1].states_generated
        })
    });
    let depth_varies = facts.windows(2).any(|w| w[0].depth != w[1].depth);
    let property_varies = facts
        .windows(2)
        .any(|w| w[0].violated_property != w[1].violated_property);
    let excluded = points.len() - facts.len();
    WorkerInvariance {
        points,
        verdict_invariant,
        counts_invariant,
        depth_varies,
        property_varies,
        excluded,
    }
}

/// A disagreement between a golden and the Examples manifest. Recorded as an
/// oracle-drift finding; never counted as agree or disagree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftFinding {
    /// Field (`result`, `distinctStates`, `totalStates`, `stateDepth`).
    pub field: String,
    /// The manifest's value.
    pub manifest: serde_json::Value,
    /// The golden's value.
    pub golden: serde_json::Value,
    /// True for findings upstream itself does not treat as errors
    /// (`stateDepth`, which upstream does not compare: tlaplus#883).
    pub informational: bool,
}

/// Upstream's own mapping of TLC exit codes to manifest results
/// (`tla_utils.resolve_tlc_exit_code`).
#[must_use]
pub fn manifest_result_for_exit(exit: i32) -> String {
    match exit {
        0 => "success".to_owned(),
        10 => "assumption failure".to_owned(),
        11 => "deadlock failure".to_owned(),
        12 => "safety failure".to_owned(),
        13 => "liveness failure".to_owned(),
        other => other.to_string(),
    }
}

/// Cross-checks a golden against the manifest's recorded expectations, the
/// way upstream's `check_small_models.py` does: the result through the exit
/// code; counts only for exhaustive models whose recorded result is
/// `success`.
#[must_use]
pub fn crosscheck_manifest(
    expected: &ManifestExpectation,
    golden: &OracleResult,
    exhaustive: bool,
) -> Vec<DriftFinding> {
    let mut out = Vec::new();
    if !golden.verdict.is_oracle_fact() {
        return out;
    }
    if expected.result != "unknown"
        && let Some(exit) = golden.exit_code
    {
        let got = manifest_result_for_exit(exit);
        if got != expected.result {
            out.push(DriftFinding {
                field: "result".to_owned(),
                manifest: expected.result.clone().into(),
                golden: got.into(),
                informational: false,
            });
        }
    }
    if exhaustive && expected.result == "success" && golden.verdict == Verdict::Success {
        let mut cmp = |field: &str, m: Option<u64>, g: Option<u64>, informational: bool| {
            if let Some(m) = m
                && Some(m) != g
            {
                out.push(DriftFinding {
                    field: field.to_owned(),
                    manifest: m.into(),
                    golden: g.map_or(serde_json::Value::Null, Into::into),
                    informational,
                });
            }
        };
        cmp(
            "distinctStates",
            expected.distinct_states,
            golden.counts.distinct_states,
            false,
        );
        cmp(
            "totalStates",
            expected.total_states,
            golden.counts.states_generated,
            false,
        );
        cmp(
            "stateDepth",
            expected.state_depth,
            golden.counts.depth,
            true,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlc::result::{Counts, Trace};

    fn result(
        verdict: Verdict,
        distinct: u64,
        generated: u64,
        depth: u64,
        trace: Option<u64>,
    ) -> OracleResult {
        OracleResult {
            verdict,
            verdict_code: None,
            error_class: None,
            exit_code: Some(0),
            exit_status: None,
            violated_property: None,
            counts: Counts {
                distinct_states: Some(distinct),
                states_generated: Some(generated),
                states_left_on_queue: Some(0),
                depth: Some(depth),
            },
            fingerprint_collision: None,
            trace: trace.map(|length| Trace {
                length,
                ending: TraceEnding::Finite,
                json: None,
                steps: None,
                omitted: None,
            }),
            error_codes: vec![],
            warning_codes: vec![],
            unresolved_modules: vec![],
            note: None,
        }
    }

    const ONE: CompareContext = CompareContext {
        workers: 1,
        counts_comparable: true,
    };
    const FOUR: CompareContext = CompareContext {
        workers: 4,
        counts_comparable: true,
    };

    #[test]
    fn identical_results_agree() {
        let a = result(Verdict::Success, 4, 5, 4, None);
        assert_eq!(diff_results(&a, &a.clone(), ONE), Vec::<FieldDiff>::new());
    }

    #[test]
    fn differences_are_reported_by_field() {
        let a = result(Verdict::SafetyViolation, 4, 5, 4, Some(4));
        let b = result(Verdict::SafetyViolation, 4, 6, 3, Some(3));
        let fields: Vec<String> = diff_results(&a, &b, ONE)
            .into_iter()
            .map(|d| d.field)
            .collect();
        assert_eq!(
            fields,
            ["counts.states_generated", "counts.depth", "trace.length"]
        );
    }

    #[test]
    fn multi_worker_rules() {
        let lenient = ComparePolicy {
            oracle_leniency: true,
            may_omit_property: false,
        };
        // TLC safety violation with 4 workers: counts, depth, trace not compared.
        let a = result(Verdict::SafetyViolation, 4, 5, 4, Some(4));
        let b = result(Verdict::SafetyViolation, 9, 9, 2, Some(2));
        assert_eq!(
            diff_results_with(&a, &b, FOUR, lenient),
            Vec::<FieldDiff>::new()
        );
        // The same run from an implementation that is not TLC: no leniency.
        let fields: Vec<String> = diff_results(&a, &b, FOUR)
            .into_iter()
            .map(|d| d.field)
            .collect();
        assert_eq!(
            fields,
            [
                "counts.distinct_states",
                "counts.states_generated",
                "counts.depth",
                "trace.length"
            ]
        );
        // Complete search with 4 workers: counts compared, depth not.
        let a = result(Verdict::Success, 4, 5, 4, None);
        let b = result(Verdict::Success, 4, 6, 3, None);
        let fields: Vec<String> = diff_results_with(&a, &b, FOUR, lenient)
            .into_iter()
            .map(|d| d.field)
            .collect();
        assert_eq!(fields, ["counts.states_generated"]);
        // Simulation: only verdict-level fields.
        let sim = CompareContext {
            workers: 1,
            counts_comparable: false,
        };
        assert_eq!(diff_results(&a, &b, sim), Vec::<FieldDiff>::new());
        assert_eq!(
            diff_results(&a, &result(Verdict::Deadlock, 4, 5, 4, None), sim).len(),
            1
        );
    }

    #[test]
    fn property_rules() {
        let mut a = result(Verdict::LivenessViolation, 3, 4, 3, Some(3));
        let mut b = a.clone();
        a.violated_property = Some("InSync".to_owned());
        b.violated_property = Some("AllExtending and InSync".to_owned());
        let fields: Vec<String> = diff_results(&a, &b, ONE)
            .into_iter()
            .map(|d| d.field)
            .collect();
        assert_eq!(fields, ["violated_property"]);
        let lenient = ComparePolicy {
            oracle_leniency: true,
            may_omit_property: false,
        };
        assert_eq!(
            diff_results_with(&a, &b, FOUR, lenient),
            Vec::<FieldDiff>::new()
        );
        assert_eq!(diff_results(&a, &b, FOUR).len(), 1);
        // A name vs none is a diff, unless an oracle lacks the capability.
        b.violated_property = None;
        assert_eq!(diff_results(&a, &b, ONE).len(), 1);
        let omit = ComparePolicy {
            oracle_leniency: true,
            may_omit_property: true,
        };
        assert_eq!(
            diff_results_with(&a, &b, ONE, omit),
            Vec::<FieldDiff>::new()
        );
    }

    #[test]
    fn ending_and_queue_are_compared() {
        let a = result(Verdict::LivenessViolation, 3, 4, 3, Some(3));
        let mut b = a.clone();
        if let Some(t) = b.trace.as_mut() {
            t.ending = TraceEnding::Stuttering;
        }
        b.counts.states_left_on_queue = Some(1);
        let fields: Vec<String> = diff_results(&a, &b, ONE)
            .into_iter()
            .map(|d| d.field)
            .collect();
        assert_eq!(fields, ["counts.states_left_on_queue", "trace.ending"]);
    }

    #[test]
    fn invariance_ignores_non_facts() {
        let p = |workers, verdict| WorkerPoint {
            workers,
            verdict,
            distinct_states: Some(1),
            states_generated: Some(1),
            depth: Some(1),
            violated_property: None,
        };
        let w = worker_invariance(vec![p(1, Verdict::Success), p(4, Verdict::Timeout)]);
        assert!(w.verdict_invariant);
        assert_eq!(w.excluded, 1);
    }

    #[test]
    fn invariance_summary() {
        let p = |workers, distinct, depth| WorkerPoint {
            workers,
            verdict: Verdict::Success,
            distinct_states: Some(distinct),
            states_generated: Some(10),
            depth: Some(depth),
            violated_property: None,
        };
        let w = worker_invariance(vec![p(1, 5, 7), p(4, 5, 8)]);
        assert_eq!(
            (w.verdict_invariant, w.counts_invariant, w.depth_varies),
            (true, Some(true), true)
        );
        let w = worker_invariance(vec![p(1, 5, 7), p(4, 6, 7)]);
        assert_eq!(w.counts_invariant, Some(false));
    }

    #[test]
    fn manifest_drift() {
        let exp = ManifestExpectation {
            result: "success".to_owned(),
            runtime_seconds: Some(1),
            distinct_states: Some(4),
            total_states: Some(6),
            state_depth: Some(9),
            workers: None,
        };
        let g = result(Verdict::Success, 4, 5, 4, None);
        let f = crosscheck_manifest(&exp, &g, true);
        assert_eq!(f.len(), 2);
        assert_eq!(
            (f[0].field.as_str(), f[0].informational),
            ("totalStates", false)
        );
        assert_eq!(
            (f[1].field.as_str(), f[1].informational),
            ("stateDepth", true)
        );
        let mut bad = g.clone();
        bad.verdict = Verdict::SafetyViolation;
        bad.exit_code = Some(12);
        let f = crosscheck_manifest(&exp, &bad, true);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].golden, serde_json::Value::from("safety failure"));
    }
}
