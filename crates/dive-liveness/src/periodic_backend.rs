//! Source-owned pointer-table growth and stopping receipts at explicit checkpoints.
//!
//! This backend diagnostic does not select an ordinary clock schedule, suspend
//! TLC's queue, certify a candidate or report temporal holds.

use crate::phase_execution::{self, Complete, Failure, PartialCheck};
use crate::phase_graph::Limits;
use dive_check::capture::SourceBarrier;
use dive_check::{CheckOptions, Stats};
use dive_model::Model;

/// Pinned `LiveCheck.check` policy for explicit source-prefix requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Schedule {
    /// Unforced checking uses actual pointer growth.
    Default,
    /// Unforced checking is skipped; forcing still performs a partial check.
    Final,
}
/// Explicit request at completed initialization and every whole source commit.
/// This is not a wall-clock periodic schedule or `ModelChecker.doPeriodicWork`.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// Equivalent to `LiveCheck.check(tool, forceCheck)` for each request.
    /// Unlike the outer periodic controller, this field is not cleared.
    pub force: bool,
    /// Pinned default/final unforced-check branch.
    pub schedule: Schedule,
    /// Actual `TLCGlobals.livenessThreshold` counterpart (default 0.1).
    /// Floating-point comparisons, including NaN/infinity, are preserved.
    pub threshold: f64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            force: false,
            schedule: Schedule::Default,
            threshold: 0.1,
        }
    }
}
/// One OOS's actual pointer-bucket count and recorded check-size snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Growth {
    pub(crate) size: u64,
    pub(crate) last: u64,
}
impl Growth {
    /// Null written fingerprint keys or tableau fingerprint buckets, including
    /// registered/done-only buckets. Never bytes or product-record counts.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }
    /// Initially one; updated after this OOS's completed PEM loop, including a
    /// found candidate, but not a thrown/bounded check or an unprocessed OOS.
    #[must_use]
    pub fn last_check_size(&self) -> u64 {
        self.last
    }
    /// Pinned strict growth comparison. Zero/zero remains NaN and is not due.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "source counts are bounded to Java's signed int table count before retention"
    )]
    pub fn due(&self, threshold: f64) -> bool {
        (self.size as f64 - self.last as f64) / self.last as f64 > threshold
    }
}
/// Actual outcome of one explicit backend checkpoint, never a model verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    /// No partial SCC invocation was requested by the pinned policy.
    Skipped,
    /// Partial SCCs finished without a candidate; does not imply holds.
    NoCandidate,
    /// First ordered OOS/PEM candidate stops further source commits.
    Candidate,
    /// A typed error/bound stopped before a completed checkpoint.
    Failed,
}
/// Producer-owned source checkpoint with exact growth and pool-prefix facts.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub(crate) sources: usize,
    pub(crate) stats: Stats,
    pub(crate) barrier: Option<SourceBarrier>,
    pub(crate) before: Vec<Growth>,
    pub(crate) after: Vec<Growth>,
    pub(crate) due: bool,
    pub(crate) decision: Decision,
    pub(crate) candidate: Option<usize>,
}
impl Checkpoint {
    /// Number of fully committed source callbacks; zero means completed Init.
    #[must_use]
    pub fn sources(&self) -> usize {
        self.sources
    }
    /// Actual logical prefix counters, separate from speculative pool work.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Real submitted wave jobs joined before this source callback. Initialization
    /// has no worker pool and returns None. This is not Java `suspendAll`.
    #[must_use]
    pub fn barrier(&self) -> Option<SourceBarrier> {
        self.barrier
    }
    /// Pinned OOS-ordered size/snapshot values before the request.
    #[must_use]
    pub fn before(&self) -> &[Growth] {
        &self.before
    }
    /// Values after processed PEM loops; untouched groups retain their snapshots.
    #[must_use]
    pub fn after(&self) -> &[Growth] {
        &self.after
    }
    /// Actual `doLiveCheck` result, separate from forced/final check admission.
    #[must_use]
    pub fn growth_due(&self) -> bool {
        self.due
    }
    /// Diagnostic decision, never holds or a validated violation.
    #[must_use]
    pub fn decision(&self) -> Decision {
        self.decision
    }
    /// Index of the first deciding selection in [`Run::checks`]; no validation claim.
    #[must_use]
    pub fn candidate_check(&self) -> Option<usize> {
        self.candidate
    }
    /// No ordinary schedule or complete temporal-model proof is certified.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
}
/// Closed source-owned run, either complete safety exploration or an actual stop.
/// None of its outcomes certifies temporal completion or a violation.
#[derive(Clone, Debug)]
pub struct Run {
    pub(crate) complete: Option<Complete>,
    pub(crate) failure: Option<Box<Failure>>,
    pub(crate) checkpoints: Vec<Checkpoint>,
    pub(crate) checks: Vec<PartialCheck>,
    pub(crate) inspections: usize,
    pub(crate) graph_inspections: usize,
    pub(crate) options: CheckOptions,
    pub(crate) policy: Policy,
    pub(crate) limits: Limits,
}
impl Run {
    /// Actual requested search options retained by the producer.
    #[must_use]
    pub fn options(&self) -> &CheckOptions {
        &self.options
    }
    /// Explicit check request used by this diagnostic, without an ordinary clock schedule.
    #[must_use]
    pub fn policy(&self) -> Policy {
        self.policy
    }
    /// Actual caller-supplied retention/work bounds, never substituted with defaults.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }
    /// Complete safety/graph evidence, absent on any actual stopping event.
    #[must_use]
    pub fn complete(&self) -> Option<&Complete> {
        self.complete.as_ref()
    }
    /// Actual deciding stop (including diagnostic candidate stop), never holds.
    #[must_use]
    pub fn stop(&self) -> Option<&Failure> {
        self.failure.as_deref()
    }
    /// Ordered source-owned checkpoint receipts, including a failed final attempt.
    #[must_use]
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }
    /// Actually completed OOS/PEM selections, in pinned first-encounter order.
    #[must_use]
    pub fn checks(&self) -> &[PartialCheck] {
        &self.checks
    }
    /// Completed reservations/selection work, as a subtotal of [`Run::graph_inspections`].
    /// A failed stage may perform unreported work; this is not aggregate memory.
    #[must_use]
    pub fn inspections(&self) -> usize {
        self.inspections
    }
    /// Shared existing graph-step quota used by graph and checkpoint operations.
    #[must_use]
    pub fn graph_inspections(&self) -> usize {
        self.graph_inspections
    }
    /// No ordinary schedule or temporal-model proof is certified.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
}
/// Execute real initialization, graph/source commits and explicit partial checks.
/// Stops actual capture at the first candidate, error or fixed bound. Caller
/// supplied graphs, write masks, sizes and claimed validation flags are excluded.
///
/// This diagnostic does not implement ordinary periodic clock/ratio/checkpoint
/// scheduling, Java queue suspension, or a model-checking verdict. Even complete
/// safety exploration with no candidate never reports temporal holds.
///
/// No error is converted to success: [`Run::stop`] retains typed admission, capture,
/// graph and checkpoint failures. The return value itself is diagnostic evidence.
#[must_use]
pub fn execute(model: &Model, options: &CheckOptions, limits: Limits, policy: Policy) -> Run {
    phase_execution::execute_periodic_backend(model, options, limits, policy)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "complete pinned numeric fixture assertions"
)]
mod tests {
    use super::Growth;
    #[test]
    fn growth_threshold_matches_complete_pinned_numeric_projection() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/probes/periodic-backend/capture-tables.json"
        ))
        .expect("tracked reference JSON");
        let fields = reference["rows"][0]["fields"]
            .as_object()
            .expect("tracked capture fields");
        let mut checked = 0;
        for (key, value) in fields {
            if let Some(k) = key.strip_prefix("due.") {
                let mut split = k.splitn(3, '.');
                let size = split
                    .next()
                    .expect("count component")
                    .parse()
                    .expect("numeric count");
                let last = split
                    .next()
                    .expect("count component")
                    .parse()
                    .expect("numeric count");
                let t = split.next().expect("threshold");
                let threshold = match t {
                    "NaN" => f64::NAN,
                    "Infinity" => f64::INFINITY,
                    "-Infinity" => f64::NEG_INFINITY,
                    _ => t.parse().expect("numeric threshold"),
                };
                assert_eq!(
                    Growth { size, last }.due(threshold).to_string(),
                    value.as_str().expect("reference bool")
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 30);
    }
}
