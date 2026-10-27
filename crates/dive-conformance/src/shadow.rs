//! `shadow`: the core against the reference engine (`docs/developer/core/
//! 08-correctness.md` §4).
//!
//! For every selected model and worker count, `dive check --engine core`
//! runs first. A model the core refuses (`unsupported`) is counted and
//! skipped. Otherwise `dive check --engine reference` runs the same model,
//! and the two results must agree on the verdict, error class, exit code,
//! violated property and, where the search is complete or at one worker,
//! the distinct and generated counts and the depth. The core's
//! counterexample, if any, must validate with `dive-trace` against the
//! reference result's ending. No golden is involved, so any corpus model
//! can be shadowed.
//!
//! A difference or a refuted counterexample is a finding: the command
//! exits non-zero. A timeout on either side is reported, not a finding.

use crate::cache::Cache;
use crate::config::Config;
use crate::corpus::{CorpusIndex, ModelEntry};
use crate::dive::{DiveEngine, DiveImplementation};
use crate::error::Result;
use crate::implementation::{ImplResult, Implementation, ModelRequest};
use crate::pipeline::{Limits, Selection, load_corpus, parallel_map};
use crate::tlc::result::{OracleResult, Verdict};
use crate::util::{compact_stamp, rfc3339, unix_now, write_atomic};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// Version of the shadow report schema.
pub const SHADOW_SCHEMA: u32 = 1;

/// What a shadow run compares.
#[derive(Debug, Clone)]
pub struct ShadowOptions {
    /// Corpus ids.
    pub corpora: Vec<String>,
    /// Tier and filters.
    pub selection: Selection,
    /// Worker counts.
    pub workers: Vec<u32>,
    /// Jobs, timeouts and budgets.
    pub limits: Limits,
    /// Where to write the JSON report, if anywhere.
    pub report_dir: Option<PathBuf>,
    /// Whether a refusal or a timeout is a finding too: for a fixed list of
    /// models the core is expected to run (`--strict`).
    pub strict: bool,
}

/// The outcome of one model at one worker count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShadowStatus {
    /// Both engines gave the same observables, and the core's
    /// counterexample (if any) validated.
    Agree,
    /// The core refused the model.
    Refused,
    /// The observables differ (a finding).
    Differ,
    /// The core's counterexample was refuted (a finding).
    TraceRefuted,
    /// Either engine hit its time or memory budget, or the harness timeout.
    Timeout,
}

impl ShadowStatus {
    /// Whether the status fails the run; under `strict`, a refusal or a
    /// timeout does too.
    #[must_use]
    pub const fn is_finding(self, strict: bool) -> bool {
        match self {
            Self::Differ | Self::TraceRefuted => true,
            Self::Refused | Self::Timeout => strict,
            Self::Agree => false,
        }
    }
}

/// One model at one worker count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowRow {
    /// Corpus id.
    pub corpus: String,
    /// Model (config) path within the corpus.
    pub model: String,
    /// Worker count.
    pub workers: u32,
    /// The outcome.
    pub status: ShadowStatus,
    /// The differing fields, as `field: core X, reference Y`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diffs: Vec<String>,
    /// The core's verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,
    /// What the trace validator said, when the core reported a
    /// counterexample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
    /// The refusal, or another detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Wall-clock milliseconds of the core run.
    pub core_ms: u64,
    /// Wall-clock milliseconds of the reference run (0 if it did not run).
    pub reference_ms: u64,
}

/// A shadow run's report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowReport {
    /// [`SHADOW_SCHEMA`].
    pub schema_version: u32,
    /// Run id (also the file name).
    pub run_id: String,
    /// RFC 3339 UTC.
    pub generated_at: String,
    /// The `dive` binary.
    pub dive: Option<crate::dive::DiveProvenance>,
    /// Corpus ids.
    pub corpora: Vec<String>,
    /// Tier.
    pub tier: String,
    /// Filters.
    pub filters: Vec<String>,
    /// Worker counts.
    pub workers: Vec<u32>,
    /// Whether refusals and timeouts were findings (`--strict`).
    #[serde(default)]
    pub strict: bool,
    /// Rows by status.
    pub summary: BTreeMap<ShadowStatus, usize>,
    /// The rows, sorted.
    pub rows: Vec<ShadowRow>,
}

impl ShadowReport {
    /// The rows that fail the run.
    pub fn findings(&self) -> impl Iterator<Item = &ShadowRow> {
        self.rows
            .iter()
            .filter(|r| r.status.is_finding(self.strict))
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

fn stopped(r: &OracleResult) -> bool {
    matches!(r.verdict, Verdict::Timeout | Verdict::ResourceLimit)
}

/// The fields on which `core` and `reference` differ. Counts and depth are
/// compared at one worker, and at any worker count for a complete search
/// (`success`): a violation found by several workers may end at a
/// different state count in the reference engine (ADR 0004), while the
/// core's are always its one-worker counts.
#[must_use]
pub fn differences(core: &OracleResult, reference: &OracleResult, workers: u32) -> Vec<String> {
    let mut diffs = Vec::new();
    let mut field = |name: &str, c: String, r: String| {
        if c != r {
            diffs.push(format!("{name}: core {c}, reference {r}"));
        }
    };
    field(
        "verdict",
        format!("{:?}", core.verdict),
        format!("{:?}", reference.verdict),
    );
    field(
        "error_class",
        format!("{:?}", core.error_class),
        format!("{:?}", reference.error_class),
    );
    field(
        "exit_code",
        format!("{:?}", core.exit_code),
        format!("{:?}", reference.exit_code),
    );
    field(
        "violated_property",
        format!("{:?}", core.violated_property),
        format!("{:?}", reference.violated_property),
    );
    if workers == 1 || reference.verdict == Verdict::Success {
        field(
            "distinct_states",
            format!("{:?}", core.counts.distinct_states),
            format!("{:?}", reference.counts.distinct_states),
        );
        field(
            "states_generated",
            format!("{:?}", core.counts.states_generated),
            format!("{:?}", reference.counts.states_generated),
        );
        field(
            "depth",
            format!("{:?}", core.counts.depth),
            format!("{:?}", reference.counts.depth),
        );
    }
    diffs
}

fn row(corpus: &CorpusIndex, m: &ModelEntry, workers: u32, status: ShadowStatus) -> ShadowRow {
    ShadowRow {
        corpus: corpus.id.clone(),
        model: m.id.clone(),
        workers,
        status,
        diffs: Vec::new(),
        verdict: None,
        trace: None,
        detail: None,
        core_ms: 0,
        reference_ms: 0,
    }
}

fn shadow_model(
    core: &DiveImplementation,
    reference: &DiveImplementation,
    corpus: &CorpusIndex,
    m: &ModelEntry,
    workers: u32,
    limits: &Limits,
) -> ShadowRow {
    let req = ModelRequest {
        corpus_root: &corpus.root,
        model: m,
        workers,
        timeout: limits.timeout_for(m, None),
        xmx: &limits.xmx,
        budgets: limits.budget_cap.is_some(),
    };
    let fail = |status, detail: String| {
        let mut r = row(corpus, m, workers, status);
        r.detail = Some(detail);
        r
    };
    let (core_result, core_elapsed) = match core.check_model(&req) {
        Err(e) => return fail(ShadowStatus::Differ, format!("the core run failed: {e}")),
        Ok(ImplResult::Unsupported(why)) => return fail(ShadowStatus::Refused, why),
        Ok(ImplResult::Completed {
            result, elapsed, ..
        }) => (result, elapsed),
    };
    let mut out = row(corpus, m, workers, ShadowStatus::Agree);
    out.verdict = Some(core_result.verdict);
    out.core_ms = millis(core_elapsed);
    if stopped(&core_result) {
        out.status = ShadowStatus::Timeout;
        out.detail.clone_from(&core_result.note);
        return out;
    }
    let (reference_result, reference_elapsed) = match reference.check_model(&req) {
        Err(e) => {
            out.status = ShadowStatus::Differ;
            out.detail = Some(format!("the reference run failed: {e}"));
            return out;
        }
        Ok(ImplResult::Unsupported(why)) => {
            out.status = ShadowStatus::Differ;
            out.detail = Some(format!(
                "the reference engine refused what the core ran: {why}"
            ));
            return out;
        }
        Ok(ImplResult::Completed {
            result, elapsed, ..
        }) => (result, elapsed),
    };
    out.reference_ms = millis(reference_elapsed);
    if stopped(&reference_result) {
        out.status = ShadowStatus::Timeout;
        out.detail.clone_from(&reference_result.note);
        return out;
    }
    out.diffs = differences(&core_result, &reference_result, workers);
    if !out.diffs.is_empty() {
        out.status = ShadowStatus::Differ;
    }
    if core_result.trace.is_some() {
        let check = core.trace_validated(&req, &core_result, &reference_result);
        out.trace = Some(check.detail.clone());
        if check.refuted && out.status == ShadowStatus::Agree {
            out.status = ShadowStatus::TraceRefuted;
        }
    }
    out
}

/// Runs the shadow comparison. `bin` is the `dive` binary; both engines
/// run it, in the default profile.
///
/// # Errors
/// Fails on an unknown corpus, an empty selection, a missing binary, or a
/// report that cannot be written; differences are rows, not errors.
pub fn run(
    config: &Config,
    cache: &Cache,
    golden_oracle: &str,
    bin: &std::path::Path,
    opts: &ShadowOptions,
) -> Result<ShadowReport> {
    let mut core = DiveImplementation::prepare(config, cache, golden_oracle, bin)?;
    core.engine = DiveEngine::Core;
    let mut reference = core.clone();
    reference.engine = DiveEngine::Reference;
    let corpora = opts
        .corpora
        .iter()
        .map(|id| load_corpus(config, cache, id))
        .collect::<Result<Vec<_>>>()?;
    let mut jobs = Vec::new();
    for corpus in &corpora {
        for m in opts.selection.models(corpus)? {
            for &w in &opts.workers {
                jobs.push((corpus, m, w));
            }
        }
    }
    if jobs.is_empty() {
        return Err(crate::error::Error::Usage(
            "the selection is empty: no model matches the tier and filters".to_owned(),
        ));
    }
    let total = jobs.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let mut rows = parallel_map(&jobs, opts.limits.jobs, |_, &(corpus, m, w)| {
        let r = shadow_model(&core, &reference, corpus, m, w, &opts.limits);
        let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let status = serde_json::to_value(r.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        eprintln!(
            "[{n}/{total}] {status:<13} {} at {w} worker(s){}",
            m.id,
            if r.diffs.is_empty() {
                String::new()
            } else {
                format!(" ({})", r.diffs.join("; "))
            }
        );
        r
    });
    rows.sort_by(|a, b| (&a.corpus, &a.model, a.workers).cmp(&(&b.corpus, &b.model, b.workers)));
    let mut summary = BTreeMap::new();
    for r in &rows {
        *summary.entry(r.status).or_insert(0) += 1;
    }
    let now = unix_now();
    let report = ShadowReport {
        schema_version: SHADOW_SCHEMA,
        run_id: format!("{}-shadow-{}", compact_stamp(now), opts.selection.tier),
        generated_at: rfc3339(now),
        dive: core.dive_provenance(),
        corpora: opts.corpora.clone(),
        tier: opts.selection.tier.clone(),
        filters: opts.selection.filters.clone(),
        workers: opts.workers.clone(),
        strict: opts.strict,
        summary,
        rows,
    };
    if let Some(dir) = &opts.report_dir {
        let path = dir.join(format!("{}.json", report.run_id));
        let text = serde_json::to_vec_pretty(&report)
            .map_err(|e| crate::error::Error::Config(format!("shadow report: {e}")))?;
        write_atomic(&path, &text)?;
        eprintln!("shadow report written to {}", path.display());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlc::result::Counts;

    fn result(verdict: Verdict, distinct: u64) -> OracleResult {
        OracleResult {
            native_unreduced_symmetry: false,
            engine: None,
            verdict,
            verdict_code: None,
            error_class: None,
            exit_code: Some(0),
            exit_status: None,
            violated_property: None,
            counts: Counts {
                distinct_states: Some(distinct),
                ..Counts::default()
            },
            fingerprint_collision: None,
            trace: None,
            error_codes: vec![],
            warning_codes: vec![],
            unresolved_modules: vec![],
            note: None,
        }
    }

    #[test]
    fn counts_are_compared_where_they_are_deterministic() {
        let ok = result(Verdict::Success, 3);
        assert_eq!(differences(&ok, &ok, 1), Vec::<String>::new());
        let more = result(Verdict::Success, 4);
        assert_eq!(differences(&ok, &more, 4).len(), 1);
        // A violation found by several workers may stop at another count.
        let v1 = result(Verdict::SafetyViolation, 3);
        let v2 = result(Verdict::SafetyViolation, 5);
        assert_eq!(differences(&v1, &v2, 4), Vec::<String>::new());
        assert_eq!(differences(&v1, &v2, 1).len(), 1);
        let d = differences(&ok, &v1, 1);
        assert!(d.iter().any(|d| d.starts_with("verdict:")), "{d:?}");
        assert!(ShadowStatus::Differ.is_finding(false));
        assert!(!ShadowStatus::Refused.is_finding(false) && ShadowStatus::Refused.is_finding(true));
        assert!(!ShadowStatus::Agree.is_finding(true));
    }
}
