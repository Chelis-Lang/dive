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
//! A difference, a refuted counterexample, or an unconfirmed counterexample is a finding: the command
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
    /// Sample this many expanded states per core run and compare their
    /// successors with the reference evaluator's (`--sample`; 0: none).
    pub sample: usize,
    /// The profile both engines run in (`--impl-profile`; default `dive`).
    pub profile: crate::divergence::Profile,
}

/// The outcome of one model at one worker count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShadowStatus {
    /// Both engines gave the same classified observables, and the core's
    /// counterexample (if any) validated.
    Agree,
    /// The core refused the model.
    Refused,
    /// The observables differ (a finding).
    Differ,
    /// The core's counterexample was refuted (a finding).
    TraceRefuted,
    /// The core's required counterexample could not be independently
    /// confirmed (a finding, even without `--strict`).
    TraceUnconfirmed,
    /// A sampled state's successors differ from the reference evaluator's
    /// (a finding).
    SuccessorsDiffer,
    /// Either engine hit its time or memory budget, or the harness timeout.
    Timeout,
}

impl ShadowStatus {
    /// Whether the status fails the run; under `strict`, a refusal or a
    /// timeout does too.
    #[must_use]
    pub const fn is_finding(self, strict: bool) -> bool {
        match self {
            Self::Differ | Self::TraceRefuted | Self::TraceUnconfirmed | Self::SuccessorsDiffer => {
                true
            }
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
    /// The core's successor sample, when one was taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<crate::tlc::result::CoreSample>,
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
    /// The profile both engines ran in.
    #[serde(default)]
    pub profile: crate::divergence::Profile,
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

fn nonsemantic_result(
    result: &OracleResult,
    engine: &str,
) -> Option<(ShadowStatus, Option<String>)> {
    if stopped(result) {
        return Some((ShadowStatus::Timeout, result.note.clone()));
    }
    (!result.verdict.is_oracle_fact()).then(|| {
        (
            ShadowStatus::Differ,
            Some(format!(
                "the {engine} result is {} and cannot establish agreement: {}",
                result.verdict.as_str(),
                result.note.as_deref().unwrap_or("no classified result")
            )),
        )
    })
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
        sample: None,
        core_ms: 0,
        reference_ms: 0,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "ordered engine comparison and independent replay remain in one driver"
)]
fn shadow_model(
    core: &impl Implementation,
    reference: &impl Implementation,
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
    out.sample = core_result.engine.as_ref().and_then(|e| e.sample.clone());
    out.core_ms = millis(core_elapsed);
    if let Some((status, detail)) = nonsemantic_result(&core_result, "core") {
        out.status = status;
        out.detail = detail;
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
    if let Some((status, detail)) = nonsemantic_result(&reference_result, "reference") {
        out.status = status;
        out.detail = detail;
        return out;
    }
    out.diffs = differences(&core_result, &reference_result, workers);
    let requires_word = matches!(
        core_result.verdict,
        Verdict::SafetyViolation
            | Verdict::ActionPropertyViolation
            | Verdict::LivenessViolation
            | Verdict::Deadlock
    ) || (core_result.verdict == reference_result.verdict
        && reference_result.trace.is_some());
    if core_result.trace.is_some() || requires_word {
        let check = match &core_result.trace {
            None => crate::trace_check::TraceCheck::no(
                "the verdict requires a counterexample, but the result carries none",
            ),
            Some(trace)
                if trace.json.as_ref().is_none_or(serde_json::Value::is_null)
                    && trace.steps.as_ref().is_none_or(Vec::is_empty) =>
            {
                crate::trace_check::TraceCheck::no("the counterexample carries no word payload")
            }
            Some(_) => core.trace_validated(&req, &core_result, &reference_result),
        };
        out.trace = Some(check.detail.clone());
        if reference_result.verdict == Verdict::LivenessViolation
            && check.confirms_liveness_property(&core_result)
            && out
                .diffs
                .iter()
                .any(|diff| diff.starts_with("violated_property:"))
        {
            // Neither implementation is an oracle here. Both actual words and
            // reported lists must independently validate before names may differ.
            let reference_check = reference.trace_validated(&req, &reference_result, &core_result);
            if reference_check.confirms_liveness_property(&reference_result) {
                out.diffs
                    .retain(|diff| !diff.starts_with("violated_property:"));
                out.trace = Some(
                    "both words and their exact reported liveness names independently validated"
                        .into(),
                );
            }
        }
        if !check.validated && out.status == ShadowStatus::Agree {
            out.status = if check.refuted {
                ShadowStatus::TraceRefuted
            } else {
                ShadowStatus::TraceUnconfirmed
            };
        }
    }
    if !out.diffs.is_empty() {
        out.status = ShadowStatus::Differ;
    } else if let Some(s) = out.sample.as_ref().filter(|s| s.mismatches > 0) {
        out.status = ShadowStatus::SuccessorsDiffer;
        out.diffs.push(format!(
            "successors: {} of {} sampled (state, action) pairs differ",
            s.mismatches, s.compared
        ));
        out.diffs.extend(s.shown.iter().cloned());
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
    core.profile = opts.profile.clone();
    let mut reference = core.clone();
    reference.engine = DiveEngine::Reference;
    core.core_sample = (opts.sample > 0).then_some(opts.sample);
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
        run_id: format!(
            "{}-shadow-{}{}",
            compact_stamp(now),
            opts.selection.tier,
            if opts.profile == crate::divergence::Profile::Tlc {
                "-tlc"
            } else {
                ""
            }
        ),
        generated_at: rfc3339(now),
        dive: core.dive_provenance(),
        corpora: opts.corpora.clone(),
        tier: opts.selection.tier.clone(),
        filters: opts.selection.filters.clone(),
        workers: opts.workers.clone(),
        strict: opts.strict,
        profile: opts.profile.clone(),
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
    use crate::tlc::result::{Counts, Trace, TraceEnding};
    use crate::trace_check::TraceCheck;
    use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions};
    use serde_json::json;

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

    struct Replay {
        result: OracleResult,
        refusal: Option<TraceCheck>,
    }

    impl Implementation for Replay {
        fn name(&self) -> String {
            "shadow-trace-control".into()
        }

        fn is_oracle(&self) -> bool {
            false
        }

        fn check_model(&self, _: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>> {
            Ok(ImplResult::Completed {
                result: self.result.clone(),
                command: vec![],
                elapsed: Duration::ZERO,
            })
        }

        fn parse_module(
            &self,
            _: &std::path::Path,
            _: &str,
            _: Duration,
        ) -> Result<ImplResult<crate::sany::SanyResult>> {
            Ok(ImplResult::Unsupported("not selected".into()))
        }

        fn trace_validated(
            &self,
            req: &ModelRequest<'_>,
            result: &OracleResult,
            reference: &OracleResult,
        ) -> TraceCheck {
            if let Some(refusal) = &self.refusal {
                return refusal.clone();
            }
            match SpecModel::load(
                &req.corpus_root.join(&req.model.module),
                &req.corpus_root.join(&req.model.id),
                &TraceOptions::default(),
            ) {
                Ok(spec) => crate::trace_check::validate_counterexample(&spec, result, reference),
                Err(error) => TraceCheck::no(error.to_string()),
            }
        }
    }

    #[test]
    fn unclassified_and_contradictory_results_never_earn_agreement()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let manifest = serde_json::from_value(json!({
            "sources": [], "authors": [], "tags": [],
            "modules": [{"path": "X.tla", "features": [], "models": [{
                "path": "X.cfg", "mode": "exhaustive search", "runtime": "unknown", "result": "unknown"
            }]}]
        }))?;
        let (modules, models) = crate::corpus::models_of(&manifest)?;
        let corpus = CorpusIndex {
            id: "verdict-controls".into(),
            commit: "test".into(),
            in_repo: true,
            root: PathBuf::new(),
            modules,
            models,
            unfetched_submodules: vec![],
            uncovered: vec![],
        };
        let model = corpus.models.first().ok_or("missing control model")?;
        let limits = Limits {
            jobs: 1,
            xmx: "1g".into(),
            timeout: Some(Duration::from_secs(5)),
            timeout_cap: Duration::from_secs(5),
            budget_cap: None,
        };
        let mut error = result(Verdict::EvaluationError, 0);
        error.verdict_code = Some(2179);
        error.error_class = Some("TLC_MODULE_DIVISION_BY_ZERO".into());
        error.exit_code = Some(255);
        let replay = |result| Replay {
            result,
            refusal: None,
        };
        for workers in [1, 4] {
            let classified = replay(error.clone());
            let row = shadow_model(&classified, &classified, &corpus, model, workers, &limits);
            assert_eq!(row.status, ShadowStatus::Agree, "{row:?}");
            for verdict in [Verdict::Unclassified, Verdict::OracleError] {
                let mut invalid = result(verdict, 0);
                invalid.note = Some("the native report could not be trusted".into());
                for (core_result, reference_result, engine) in [
                    (invalid.clone(), invalid.clone(), "core"),
                    (invalid.clone(), error.clone(), "core"),
                    (error.clone(), invalid.clone(), "reference"),
                ] {
                    let row = shadow_model(
                        &replay(core_result),
                        &replay(reference_result),
                        &corpus,
                        model,
                        workers,
                        &limits,
                    );
                    assert_eq!(row.status, ShadowStatus::Differ, "{verdict:?}: {row:?}");
                    let detail = row.detail.as_deref().ok_or("missing finding detail")?;
                    assert!(detail.contains(engine), "{detail}");
                    assert!(detail.contains(verdict.as_str()), "{detail}");
                    assert!(detail.contains("could not be trusted"), "{detail}");
                    assert!(row.status.is_finding(false) && row.status.is_finding(true));
                }
            }
            // Resource stops retain their existing separate classification.
            for verdict in [Verdict::Timeout, Verdict::ResourceLimit] {
                let stopped = replay(result(verdict, 0));
                let row = shadow_model(&stopped, &stopped, &corpus, model, workers, &limits);
                assert_eq!(row.status, ShadowStatus::Timeout, "{row:?}");
                let row = shadow_model(&classified, &stopped, &corpus, model, workers, &limits);
                assert_eq!(row.status, ShadowStatus::Timeout, "{row:?}");
            }
        }
        Ok(())
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one shadow run with paired trace controls"
    )]
    fn only_independently_validated_counterexamples_can_agree()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let tmp = crate::util::TempDir::new("shadow-traces")?;
        std::fs::write(
            tmp.path().join("X.tla"),
            "---- MODULE X ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=1\nInv == x<1\n====\n",
        )?;
        std::fs::write(
            tmp.path().join("X.cfg"),
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        )?;
        let manifest = serde_json::from_value(json!({
            "sources": [], "authors": [], "tags": [],
            "modules": [{"path": "X.tla", "features": [], "models": [{
                "path": "X.cfg", "mode": "exhaustive search", "runtime": "unknown", "result": "unknown"
            }]}]
        }))?;
        let (modules, models) = crate::corpus::models_of(&manifest)?;
        let corpus = CorpusIndex {
            id: "trace-controls".into(),
            commit: "test".into(),
            in_repo: true,
            root: tmp.path().into(),
            modules,
            models,
            unfetched_submodules: vec![],
            uncovered: vec![],
        };
        let model = corpus.models.first().ok_or("missing control model")?;
        let doc = TraceDoc::behavior(vec![json!({"x": 0}), json!({"x": 1})])?
            .with_ending(Ending::Invariant("Inv".into()));
        let mut valid = result(Verdict::SafetyViolation, 2);
        valid.error_class = Some("TLC_INVARIANT_VIOLATED_BEHAVIOR".into());
        valid.violated_property = Some("Inv".into());
        valid.exit_code = Some(12);
        valid.trace = Some(Trace {
            length: 2,
            ending: TraceEnding::Finite,
            json: Some(doc.to_json()),
            steps: None,
            omitted: None,
            json_unavailable: None,
        });
        let changed_trace = |json| {
            let mut changed = valid.clone();
            if let Some(trace) = changed.trace.as_mut() {
                trace.json = json;
            }
            changed
        };
        let mut invalid_doc = doc.clone();
        invalid_doc.states[0].insert("x".into(), json!(1));
        let mut missing = valid.clone();
        missing.trace = None;
        let reference = Replay {
            result: valid.clone(),
            refusal: None,
        };
        let limits = Limits {
            jobs: 1,
            xmx: "1g".into(),
            timeout: Some(Duration::from_secs(5)),
            timeout_cap: Duration::from_secs(5),
            budget_cap: None,
        };
        for (name, result, refusal, status, detail) in [
            (
                "valid",
                valid.clone(),
                None,
                ShadowStatus::Agree,
                "validated",
            ),
            (
                "refuted",
                changed_trace(Some(invalid_doc.to_json())),
                None,
                ShadowStatus::TraceRefuted,
                "invalid",
            ),
            (
                "unsupported",
                valid.clone(),
                Some(TraceCheck::no("unsupported: independent query")),
                ShadowStatus::TraceUnconfirmed,
                "unsupported",
            ),
            (
                "absent trace",
                missing,
                None,
                ShadowStatus::TraceUnconfirmed,
                "carries none",
            ),
            (
                "absent document",
                changed_trace(None),
                None,
                ShadowStatus::TraceUnconfirmed,
                "no word payload",
            ),
            (
                "unreadable",
                changed_trace(Some(json!({}))),
                None,
                ShadowStatus::TraceUnconfirmed,
                "unreadable",
            ),
        ] {
            let core = Replay { result, refusal };
            let row = shadow_model(&core, &reference, &corpus, model, 1, &limits);
            assert_eq!(row.status, status, "{name}: {row:?}");
            assert!(
                row.trace.as_deref().is_some_and(|s| s.contains(detail)),
                "{name}: {row:?}"
            );
            assert_eq!(row.status.is_finding(false), name != "valid", "{name}");
            assert_eq!(row.status.is_finding(true), name != "valid", "{name}");
        }
        // A verdict's mandatory word cannot disappear merely because both
        // engines omitted it, and an existing observable difference is kept.
        for verdict in [
            Verdict::SafetyViolation,
            Verdict::ActionPropertyViolation,
            Verdict::LivenessViolation,
            Verdict::Deadlock,
        ] {
            let absent = Replay {
                result: result(verdict, 2),
                refusal: None,
            };
            let row = shadow_model(&absent, &absent, &corpus, model, 1, &limits);
            assert_eq!(row.status, ShadowStatus::TraceUnconfirmed, "{verdict:?}");
        }
        let mut different = valid;
        different.trace = None;
        different.counts.distinct_states = Some(3);
        let core = Replay {
            result: different,
            refusal: None,
        };
        let row = shadow_model(&core, &reference, &corpus, model, 1, &limits);
        assert_eq!(row.status, ShadowStatus::Differ);
        assert_eq!(row.diffs.len(), 1);
        assert!(
            row.trace
                .as_deref()
                .is_some_and(|s| s.contains("carries none"))
        );
        Ok(())
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "paired genuine and unproved word controls"
    )]
    fn different_shadow_names_need_both_independent_words()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/corpus/word-relative-liveness");
        let manifest = serde_json::from_str(&std::fs::read_to_string(
            root.join("specifications/Mutual/manifest.json"),
        )?)?;
        let (modules, models) = crate::corpus::models_of(&manifest)?;
        let corpus = CorpusIndex {
            id: "word-relative-liveness".into(),
            commit: "test".into(),
            in_repo: true,
            root,
            modules,
            models,
            unfetched_submodules: vec![],
            uncovered: vec![],
        };
        let model = corpus.models.first().ok_or("missing model")?;
        let mut core = result(Verdict::LivenessViolation, 3);
        core.verdict_code = Some(2116);
        core.error_class = Some("TLC_TEMPORAL_PROPERTY_VIOLATED".into());
        core.exit_code = Some(13);
        core.violated_property = Some("P and Q".into());
        let doc = TraceDoc::behavior(vec![json!({"x":0})])?
            .with_ending(Ending::Lasso { back_to: 0 })
            .with_properties(vec!["P".into(), "Q".into()]);
        core.trace = Some(Trace {
            length: 1,
            ending: TraceEnding::Lasso { back_to: Some(1) },
            json: Some(doc.to_json()),
            steps: None,
            omitted: None,
            json_unavailable: None,
        });
        let mut reference = core.clone();
        reference.violated_property = Some("P".into());
        let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})])?
            .with_ending(Ending::Lasso { back_to: 1 })
            .with_properties(vec!["P".into()]);
        reference.trace = Some(Trace {
            length: 2,
            ending: TraceEnding::Lasso { back_to: Some(2) },
            json: Some(doc.to_json()),
            steps: None,
            omitted: None,
            json_unavailable: None,
        });
        // The context-free comparison still reports the name difference.
        assert_eq!(differences(&core, &reference, 1).len(), 1);
        let limits = Limits {
            jobs: 1,
            xmx: "1g".into(),
            timeout: Some(Duration::from_secs(5)),
            timeout_cap: Duration::from_secs(5),
            budget_cap: None,
        };
        for (core_claim, reference_claim, expected) in [
            (None, None, ShadowStatus::Agree),
            (
                Some(TraceCheck::yes("claimed valid")),
                None,
                ShadowStatus::Differ,
            ),
            (
                None,
                Some(TraceCheck::yes("claimed valid")),
                ShadowStatus::Differ,
            ),
            (Some(TraceCheck::no("refused")), None, ShadowStatus::Differ),
        ] {
            let row = shadow_model(
                &Replay {
                    result: core.clone(),
                    refusal: core_claim,
                },
                &Replay {
                    result: reference.clone(),
                    refusal: reference_claim,
                },
                &corpus,
                model,
                1,
                &limits,
            );
            assert_eq!(row.status, expected, "{row:?}");
        }
        let mut forged = reference;
        forged.violated_property = Some("Q".into());
        let row = shadow_model(
            &Replay {
                result: core,
                refusal: None,
            },
            &Replay {
                result: forged,
                refusal: None,
            },
            &corpus,
            model,
            1,
            &limits,
        );
        assert_eq!(row.status, ShadowStatus::Differ, "{row:?}");
        Ok(())
    }
}
