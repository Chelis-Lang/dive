//! Non-gating final/short observations. These never count as strict conformance.
//! Verdict truth and complete-search counts are compared separately from TLC's
//! temporal classes, phase-L stopping, property selection and exact lasso.

use crate::cache::Cache;
use crate::config::Config;
use crate::corpus::ModelMode;
use crate::dive::{DiveImplementation, DiveProvenance};
use crate::divergence::Profile;
use crate::error::{Error, Result};
use crate::golden::{GoldenModel, ModelGolden};
use crate::implementation::{Implementation, ModelRequest};
use crate::pipeline::{Limits, Selection};
use crate::process::ProcessOutcome;
use crate::report::Host;
use crate::tlc::result::{Counts, OracleResult, Verdict};
use crate::trace_check::TraceCheck;
use dive_api::trace::{Ending, TraceDoc, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Scope of this evidence; it must never enter the strict conformance summary.
pub const CLAIM: &str = "Final verdict truth and counts only; no TLC temporal error-class, phase-L timing, property-set selection or exact-lasso conformance claim.";

/// Study outcome. No variant is the strict harness's `agree`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Truth and all recorded counts match; counterexamples validate independently.
    MatchesTruthAndCounts,
    /// Implementation names a feature or resource limitation.
    Unsupported,
    /// Independent validation cannot certify the counterexample.
    ValidationPending,
    /// A result differs or its counterexample is refuted.
    Disagree,
    /// Contradictory, malformed or unrecognized process output.
    MalformedReport,
    /// Harness timeout or exhausted run budget, never a model verdict.
    Timeout,
    /// Missing/stale/unusable pinned evidence.
    OracleError,
    /// Explicit corpus scope exclusion, never dropped.
    OutOfScope,
}
impl Outcome {
    /// Whether a finding should make this study exit nonzero.
    #[must_use]
    pub fn finding(self) -> bool {
        matches!(
            self,
            Self::Disagree | Self::MalformedReport | Self::Timeout | Self::OracleError
        )
    }
}

/// One selected model at one requested worker width.
#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
    /// Corpus id.
    pub corpus: String,
    /// Pinned corpus commit/tree digest.
    pub corpus_pin: String,
    /// Corpus-relative configuration path.
    pub model: String,
    /// Requested physical worker width; actual width remains in the raw report.
    pub workers: u32,
    /// Explicit observation outcome.
    pub outcome: Outcome,
    /// Explanation, including independent validation status.
    pub detail: String,
    /// Golden verdict, when available.
    pub golden_verdict: Option<Verdict>,
    /// Actual subprocess command, redacted like other harness reports.
    pub command: Vec<String>,
    /// Subprocess exit code, absent on timeout/signal.
    pub exit_code: Option<i32>,
    /// Measured wall time; no performance claim is implied.
    pub elapsed_ms: u64,
    /// Untrusted black-box schema-2/3 document, retained for auditing.
    pub document: Option<Value>,
}

/// A separate schema from strict conformance reports.
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    /// Format version, currently 1.
    pub schema_version: u32,
    /// Always `final-temporal-study`.
    pub kind: String,
    /// Explicit limits on the evidence.
    pub claim: String,
    /// Generator revision.
    pub generator: String,
    /// UTC timestamp.
    pub generated_at: String,
    /// Oracle id.
    pub oracle: String,
    /// Pinned oracle jar digest.
    pub oracle_sha256: String,
    /// Value profile; TLC values still run with named final/short overrides.
    pub base_profile: Profile,
    /// Binary and module search-path provenance.
    pub implementation: Option<DiveProvenance>,
    /// Machine details.
    pub host: Host,
    /// Timeout policy.
    pub timeout_policy: String,
    /// Every selected model/worker pair, including exclusions and missing goldens.
    pub rows: Vec<Row>,
    /// Totals reconstructed from the rows.
    pub totals: BTreeMap<Outcome, usize>,
}
impl Report {
    /// Number of failed observations; unsupported is visible, not a finding.
    #[must_use]
    pub fn findings(&self) -> usize {
        self.rows.iter().filter(|r| r.outcome.finding()).count()
    }
    /// Writes a study artifact. This never replaces `docs/conformance.md`.
    ///
    /// # Errors
    /// Serialization or filesystem failure.
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("cannot serialize final study: {e}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, &e))?;
        }
        std::fs::write(path, format!("{text}\n")).map_err(|e| Error::io(path, &e))
    }
}

/// Options for this non-gating study. Seed/fingerprint are fixed to golden values.
pub struct Options {
    /// Corpora; every selected model is retained.
    pub corpora: Vec<String>,
    /// Tier and glob filters.
    pub selection: Selection,
    /// Pinned golden oracle id.
    pub oracle: String,
    /// Black-box binary under test.
    pub binary: PathBuf,
    /// Dive or TLC values; mixed profiles are not accepted here.
    pub profile: Profile,
    /// Requested worker widths, all positive.
    pub workers: Vec<u32>,
    /// Per-model timeout policy; runs are sequential.
    pub limits: Limits,
}

fn mode(v: &Value, profile: &Profile, workers: u32) -> std::result::Result<(), String> {
    if !matches!(v["schema_version"].as_u64(), Some(2 | 3))
        || v["command"] != "check"
        || v.get("check_kind").is_some_and(|kind| kind != "temporal")
        || (v["schema_version"] == 3 && v["check_kind"] != "temporal")
    {
        return Err("expected a schema-2 or schema-3 check document".into());
    }
    crate::dive::validate_temporal_bounds(v)?;
    let o = &v["options"];
    if !o["budget"]["max_states"].is_null() || !o["budget"]["max_depth"].is_null() {
        return Err("study report contains an unrequested search ceiling".into());
    }
    if o["lncheck"] != "final" || o["lasso"] != "short" || o["tlc_exact_temporal"] != false {
        return Err(
            "report does not declare final/short overrides without a TLC-exact claim".into(),
        );
    }
    let tlc = *profile == Profile::Tlc;
    for (field, expected) in [
        ("profile", if tlc { "tlc+overrides" } else { "dive" }),
        ("int", if tlc { "tlc" } else { "big" }),
        ("choose_order", if tlc { "tlc" } else { "canonical" }),
        ("explore_order", if tlc { "tlc" } else { "value" }),
        ("comparisons", if tlc { "tlc" } else { "tla" }),
        ("store", "fingerprint"),
    ] {
        if o[field] != expected {
            return Err(format!("unexpected {field} in study report"));
        }
    }
    if o["seed"].as_u64() != Some(0)
        || o["deadlock_flag"] != false
        || o["warnings"] != true
        || o["fingerprint"]["polynomial_index"].as_u64() != Some(0)
        || o["fingerprint"]["scheme"] != if tlc { "tlc" } else { "dive" }
        || o["workers"]["requested"].as_u64() != Some(u64::from(workers))
    {
        return Err(
            "report options contradict the requested seed/fingerprint/worker configuration".into(),
        );
    }
    Ok(())
}

fn counts(v: &Value) -> std::result::Result<Counts, String> {
    let s = &v["statistics"];
    let n = |k: &str| {
        s[k].as_u64()
            .ok_or_else(|| format!("missing/noninteger statistics.{k}"))
    };
    Ok(Counts {
        distinct_states: Some(n("distinct")?),
        states_generated: Some(n("generated")?),
        states_left_on_queue: Some(n("left_on_queue")?),
        depth: Some(n("depth")?),
    })
}

fn lasso_document(v: &Value) -> std::result::Result<TraceDoc, String> {
    let doc = TraceDoc::from_json(&v["counterexample"]["trace"])?;
    let Some(Ending::Lasso { back_to }) = doc.ending else {
        return Err("temporal result needs a lasso document".into());
    };
    let c = &v["counterexample"];
    if doc.states.is_empty()
        || back_to >= doc.states.len()
        || c["ending"] != "lasso"
        || c["length"].as_u64() != Some(doc.states.len() as u64)
        || c["back_to"].as_u64() != Some(back_to as u64 + 1)
    {
        return Err("lasso metadata contradicts its document".into());
    }
    let states = c["states"].as_array().ok_or("missing displayed states")?;
    if states.len() != doc.states.len()
        || states
            .iter()
            .zip(&doc.states)
            .enumerate()
            .any(|(i, (shown, actual))| {
                shown["index"].as_u64() != Some(i as u64 + 1)
                    || shown["values"] != Value::Object(actual.clone())
            })
    {
        return Err("displayed states contradict the lossless document".into());
    }
    let names = v["properties"].as_array().ok_or("missing property names")?;
    let names: Vec<&str> = names
        .iter()
        .map(|n| {
            n.as_str()
                .filter(|s| !s.is_empty())
                .ok_or("invalid property name")
        })
        .collect::<std::result::Result<_, _>>()?;
    let distinct: BTreeSet<_> = names.iter().copied().collect();
    let inner: BTreeSet<_> = doc.properties.iter().map(String::as_str).collect();
    if names.is_empty()
        || distinct.len() != names.len()
        || inner.len() != doc.properties.len()
        || distinct != inner
        || !v["property"].as_str().is_some_and(|n| distinct.contains(n))
    {
        return Err("lasso property claims are empty, duplicate or contradictory".into());
    }
    Ok(doc)
}

// Validation is supplied by the harness, never taken from the document's flag.
#[allow(
    clippy::too_many_lines,
    reason = "explicit process, mode, truth, count and independent-witness boundaries"
)]
fn assess(
    outcome: &ProcessOutcome,
    profile: &Profile,
    workers: u32,
    golden: &OracleResult,
    validate: impl FnOnce(&TraceDoc) -> TraceCheck,
) -> (Outcome, String, Option<Value>) {
    if outcome.timed_out {
        return (Outcome::Timeout, "study timeout; no verdict".into(), None);
    }
    let v: Value = match serde_json::from_str(outcome.stdout.trim()) {
        Ok(v) => v,
        Err(e) => {
            return (
                Outcome::MalformedReport,
                format!("not a JSON report: {e}; exit {:?}", outcome.exit_code),
                None,
            );
        }
    };
    let bad = |detail: String| (Outcome::MalformedReport, detail, Some(v.clone()));
    if let Err(e) = mode(&v, profile, workers) {
        return bad(e);
    }
    let kind = v["verdict"].as_str().unwrap_or("");
    if matches!(kind, "unsupported" | "resource-limit") {
        if outcome.exit_code != Some(3) {
            return bad("refusal has a contradictory exit code".into());
        }
        let Some(message) = v["message"].as_str().filter(|s| !s.trim().is_empty()) else {
            return bad("refusal must name its reason".into());
        };
        return (Outcome::Unsupported, format!("{kind}: {message}"), Some(v));
    }
    if kind == "stopped" {
        // Run budgets are findings, just as in the strict harness. A
        // memory blowup must not be hidden among unsupported features.
        if outcome.exit_code != Some(5) {
            return bad("budget stop has a contradictory exit code".into());
        }
        let message = v["message"].as_str().unwrap_or_default().to_owned();
        return match v["stopped"]["resource"].as_str() {
            Some("time" | "memory" | "states" | "depth") => {
                (Outcome::Timeout, format!("stopped: {message}"), Some(v))
            }
            _ => bad("budget stop must name its resource".into()),
        };
    }
    let tlc = *profile == Profile::Tlc;
    let (observed, observed_counts, doc) = match kind {
        "success" | "liveness-violation" => {
            let violation = kind == "liveness-violation";
            let exit = if !violation {
                0
            } else if tlc {
                13
            } else {
                10
            };
            if outcome.exit_code != Some(exit) || !v["tlc"].is_null() {
                return bad(
                    "final temporal truth has a contradictory exit or TLC-class claim".into(),
                );
            }
            if !v["options"]["workers"]["used"]
                .as_u64()
                .is_some_and(|n| n > 0 && n <= u64::from(workers))
            {
                return bad("completed temporal report has invalid actual worker width".into());
            }
            let counts = match counts(&v) {
                Ok(c) => c,
                Err(e) => return bad(e),
            };
            let doc = if violation {
                match lasso_document(&v) {
                    Ok(d) => Some(d),
                    Err(e) => return bad(e),
                }
            } else {
                if !v["counterexample"].is_null() {
                    return bad("success carries a counterexample".into());
                }
                None
            };
            (
                if violation {
                    Verdict::LivenessViolation
                } else {
                    Verdict::Success
                },
                counts,
                doc,
            )
        }
        // A safety stop carries the ordinary verified class/statistics shape.
        // Decode that shape separately; it still yields only a study observation.
        "invariant-violation" | "deadlock" | "action-property-violation" | "property-violation" => {
            let expected_exit = if tlc {
                None
            } else {
                Some(if kind == "deadlock" { 11 } else { 10 })
            };
            if expected_exit.is_some() && outcome.exit_code != expected_exit {
                return bad("safety stop contradicts the default-profile exit code".into());
            }
            match crate::dive::safety_result(outcome, tlc, &v) {
                crate::dive::CheckMapping::Refused(why) => {
                    return (Outcome::Unsupported, why, Some(v));
                }
                crate::dive::CheckMapping::Result(r) if r.verdict != Verdict::Unclassified => {
                    let doc = match TraceDoc::from_json(&v) {
                        Ok(d)
                            if match &d.ending {
                                Some(Ending::Invariant(n)) => {
                                    r.verdict == Verdict::SafetyViolation
                                        && r.violated_property.as_ref() == Some(n)
                                }
                                Some(Ending::Property(n)) => {
                                    matches!(
                                        r.verdict,
                                        Verdict::SafetyViolation | Verdict::ActionPropertyViolation
                                    ) && r.violated_property.as_ref() == Some(n)
                                }
                                Some(Ending::Deadlock) => r.verdict == Verdict::Deadlock,
                                _ => false,
                            } =>
                        {
                            Some(d)
                        }
                        Ok(_) => {
                            return bad(
                                "safety result contradicts the independently checked ending".into(),
                            );
                        }
                        Err(e) => return bad(e),
                    };
                    (r.verdict, r.counts, doc)
                }
                crate::dive::CheckMapping::Result(_) => {
                    return bad("unusable safety-stop report".into());
                }
            }
        }
        "internal-error" => {
            return (
                Outcome::Disagree,
                format!("internal error: {}", v["message"]),
                Some(v),
            );
        }
        "rejected" | "model-error" => {
            return (
                Outcome::Disagree,
                format!("model rejected: {}", v["message"]),
                Some(v),
            );
        }
        other => {
            return (
                Outcome::MalformedReport,
                format!(
                    "study does not map deciding result {other:?}; exit {:?}",
                    outcome.exit_code
                ),
                Some(v),
            );
        }
    };
    if observed != golden.verdict {
        return (
            Outcome::Disagree,
            format!(
                "verdict truth: {:?} vs golden {:?}",
                observed, golden.verdict
            ),
            Some(v),
        );
    }
    if observed_counts != golden.counts {
        return (
            Outcome::Disagree,
            format!("counts: {observed_counts:?} vs golden {:?}", golden.counts),
            Some(v),
        );
    }
    let detail = if let Some(doc) = doc {
        let checked = validate(&doc);
        if checked.refuted {
            return (Outcome::Disagree, checked.detail, Some(v));
        }
        if !checked.validated {
            return (Outcome::ValidationPending, checked.detail, Some(v));
        }
        format!(
            "truth/counts match; independently {}. {CLAIM}",
            checked.detail
        )
    } else {
        format!("truth/counts match. {CLAIM}")
    };
    (Outcome::MatchesTruthAndCounts, detail, Some(v))
}

fn evidence(
    g: &ModelGolden,
    corpus: &crate::corpus::CorpusIndex,
    model: &crate::corpus::ModelEntry,
    oracle: &str,
    sha: &str,
) -> std::result::Result<(), String> {
    if g.oracle != oracle
        || g.corpus != corpus.id
        || g.workers != 1
        || g.model != GoldenModel::from(model)
        || g.provenance.corpus_commit != corpus.commit
        || g.provenance.oracle.sha256 != sha
    {
        return Err("golden identity, model entry or pins differ from the requested study".into());
    }
    for (flag, value) in [
        ("-workers", "1"),
        ("-lncheck", "final"),
        ("-seed", "0"),
        ("-fp", "0"),
    ] {
        let pairs: Vec<_> = g
            .provenance
            .command
            .windows(2)
            .filter(|p| p[0] == flag)
            .collect();
        if pairs.len() != 1 || pairs[0][1] != value {
            return Err(format!(
                "golden does not declare {flag} {value} exactly once"
            ));
        }
    }
    if !g.result.verdict.is_oracle_fact() {
        return Err("golden has no usable oracle verdict".into());
    }
    Ok(())
}

/// Observes every selected model/worker pair. Strict gates and summaries are unchanged.
///
/// # Errors
/// Invalid options, unverified corpus/bundle, missing binary or subprocess failure.
#[allow(
    clippy::too_many_lines,
    reason = "complete selection census with pinned evidence and independently bound validation"
)]
pub fn run(config: &Config, cache: &Cache, opts: &Options) -> Result<Report> {
    if opts.workers.is_empty()
        || opts.workers.contains(&0)
        || opts.corpora.is_empty()
        || !matches!(opts.profile, Profile::Tlc | Profile::Dive)
    {
        return Err(Error::Usage(
            "study needs corpora, positive workers and a dive/tlc base profile".into(),
        ));
    }
    let oracle = config.oracles.oracle(&opts.oracle)?;
    let mut imp = DiveImplementation::prepare(config, cache, &opts.oracle, &opts.binary)?;
    imp.profile = opts.profile.clone();
    let paths = crate::dive::module_search_path(config, cache, &opts.oracle)?
        .into_iter()
        .map(|p| p.path)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for id in &opts.corpora {
        let corpus = crate::pipeline::load_corpus(config, cache, id)?;
        for m in opts.selection.models(&corpus)? {
            for &workers in &opts.workers {
                let mut row = Row {
                    corpus: id.clone(),
                    corpus_pin: corpus.commit.clone(),
                    model: m.id.clone(),
                    workers,
                    outcome: Outcome::OracleError,
                    detail: String::new(),
                    golden_verdict: None,
                    command: Vec::new(),
                    exit_code: None,
                    elapsed_ms: 0,
                    document: None,
                };
                if let Some(why) = &m.out_of_scope {
                    row.outcome = Outcome::OutOfScope;
                    row.detail.clone_from(why);
                    rows.push(row);
                    continue;
                }
                if m.mode != ModelMode::Exhaustive
                    || m.features.iter().any(|f| f == "action composition")
                {
                    row.outcome = Outcome::Unsupported;
                    row.detail = format!(
                        "mode {:?}/features {:?}: final study requires ordinary exhaustive checking",
                        m.mode, m.features
                    );
                    rows.push(row);
                    continue;
                }
                let path = crate::golden::model_golden_path(
                    &crate::golden::golden_root(&config.repo_root),
                    &opts.oracle,
                    id,
                    &m.id,
                );
                let g = match crate::golden::read_model_golden(&path) {
                    Ok(g) => g,
                    Err(e) => {
                        row.detail = e.to_string();
                        rows.push(row);
                        continue;
                    }
                };
                row.golden_verdict = Some(g.result.verdict);
                if let Err(e) = evidence(&g, &corpus, m, &opts.oracle, &oracle.sha256) {
                    row.detail = e;
                    rows.push(row);
                    continue;
                }
                let req = ModelRequest {
                    corpus_root: &corpus.root,
                    model: m,
                    workers,
                    timeout: opts.limits.timeout_for(m, Some(g.provenance.elapsed_ms)),
                    xmx: &opts.limits.xmx,
                    budgets: opts.limits.budget_cap.is_some(),
                };
                let (command, process) = imp.run_model(&req, true)?;
                row.command = command;
                row.exit_code = process.exit_code;
                row.elapsed_ms = u64::try_from(process.elapsed.as_millis()).unwrap_or(u64::MAX);
                let validate = |doc: &TraceDoc| match crate::trace_check::load_model(
                    &corpus.root,
                    m,
                    &paths,
                    opts.profile == Profile::Tlc,
                ) {
                    Ok(spec) => {
                        let (report, _) = spec.read(doc);
                        match report.validation {
                            Validation::Valid => {
                                TraceCheck::yes("validated against a fresh independent model")
                            }
                            Validation::Invalid(f) => TraceCheck::refuted(format!(
                                "invalid state {}: {}",
                                f.state, f.detail
                            )),
                            Validation::Unsupported(why) => {
                                TraceCheck::no(format!("independent validation refused: {why}"))
                            }
                        }
                    }
                    Err(e) => TraceCheck::no(format!("independent binding failed: {e}")),
                };
                (row.outcome, row.detail, row.document) =
                    assess(&process, &opts.profile, workers, &g.result, validate);
                rows.push(row);
            }
        }
        // Reads may not mutate the verified corpus (including host operators).
        let after = crate::pipeline::load_corpus(config, cache, id)?;
        if after.commit != corpus.commit {
            return Err(Error::Config("corpus changed during study".into()));
        }
    }
    if imp
        .dive_provenance()
        .is_none_or(|p| crate::hash::sha256_file(&opts.binary).ok().as_ref() != Some(&p.sha256))
    {
        return Err(Error::Config("binary changed during the study".into()));
    }
    let mut totals = BTreeMap::new();
    for row in &rows {
        *totals.entry(row.outcome).or_default() += 1;
    }
    Ok(Report {
        schema_version: 1,
        kind: "final-temporal-study".into(),
        claim: CLAIM.into(),
        generator: crate::pipeline::generator(),
        generated_at: crate::util::rfc3339(crate::util::unix_now()),
        oracle: opts.oracle.clone(),
        oracle_sha256: oracle.sha256.clone(),
        base_profile: opts.profile.clone(),
        implementation: imp.dive_provenance(),
        host: Host::current(),
        timeout_policy: opts.limits.policy(),
        rows,
        totals,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixed test data and controls fail immediately"
)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn success() -> Value {
        json!({"schema_version":2,"command":"check","verdict":"success","tlc":null,
            "counterexample":null,"statistics":{"distinct":2,"generated":3,"depth":2,"left_on_queue":0},
            "options":{"profile":"tlc+overrides","int":"tlc","choose_order":"tlc","explore_order":"tlc","comparisons":"tlc","store":"fingerprint","lncheck":"final","lasso":"short","tlc_exact_temporal":false,"seed":0,"deadlock_flag":false,"warnings":true,"fingerprint":{"polynomial_index":0,"scheme":"tlc"},"workers":{"requested":4,"used":1}}})
    }
    fn process(v: &Value, exit: i32) -> ProcessOutcome {
        ProcessOutcome {
            exit_code: Some(exit),
            signal: None,
            timed_out: false,
            stdout: v.to_string(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        }
    }
    fn golden(verdict: Verdict) -> OracleResult {
        let g:ModelGolden=serde_json::from_str(include_str!("../../../conformance/golden/tlc-2026.10.06.014338/examples/specifications/Moving_Cat_Puzzle/CatEvenBoxes.cfg.json")).unwrap();
        let mut r = g.result;
        r.verdict = verdict;
        r.counts = Counts {
            distinct_states: Some(2),
            states_generated: Some(3),
            depth: Some(2),
            states_left_on_queue: Some(0),
        };
        r
    }
    fn lasso() -> Value {
        let mut v = success();
        let doc = TraceDoc::behavior(vec![json!({"x":false}), json!({"x":true})])
            .unwrap()
            .with_ending(Ending::Lasso { back_to: 1 });
        let mut d = doc;
        d.properties = vec!["P".into()];
        v["verdict"] = json!("liveness-violation");
        v["properties"] = json!(["P"]);
        v["property"] = json!("P");
        v["counterexample"] = json!({"length":2,"ending":"lasso","back_to":2,"trace":d.to_json(),"validation":"valid","states":[{"index":1,"values":{"x":false}},{"index":2,"values":{"x":true}}]});
        v
    }
    #[test]
    fn real_initial_memory_stop_is_a_finding_in_the_qualified_final_study()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use dive_api::{BudgetSetting, CheckRequest, TemporalMode};
        let scratch =
            crate::util::TempDir::new_in(&std::env::temp_dir(), "study-initial-memory-stop")?;
        let file = scratch.path().join("Tiny.tla");
        std::fs::write(
            &file,
            "---- MODULE Tiny ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n",
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        let reference = golden(Verdict::Success);
        for profile in [Profile::Dive, Profile::Tlc] {
            let report = dive_api::check_file(&CheckRequest {
                file: file.clone(),
                compat_tlc: profile == Profile::Tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                temporal: TemporalMode::FinalShort,
                time_limit: BudgetSetting::Unlimited,
                max_memory: BudgetSetting::Explicit(1),
                ..CheckRequest::default()
            })?;
            let value = report.to_json();
            assert_eq!(value["check_kind"], "temporal");
            assert_eq!(value["execution"]["stage"], "check-startup");
            assert_eq!(value["execution"]["capture_completed"], false);
            assert_eq!(value["resources"]["checking_started"], false);
            let (observation, detail, _) = assess(
                &process(&value, i32::from(report.exit_code())),
                &profile,
                1,
                &reference,
                |_| panic!("an initial stop supplies no witness"),
            );
            assert_eq!(observation, Outcome::Timeout, "{detail}: {value}");
            assert!(observation.finding());
        }
        Ok(())
    }

    #[test]
    fn budget_stops_are_findings_never_observations_or_refusals() {
        let g = golden(Verdict::Success);
        let stopped = |resource: &str| {
            let mut v = success();
            v["verdict"] = json!("stopped");
            v["message"] = json!("stopped during graph-capture");
            v["stopped"] = json!({"resource": resource});
            v
        };
        let assess_at = |v: &Value, exit| {
            assess(&process(v, exit), &Profile::Tlc, 4, &g, |_| {
                panic!("a stop has no witness")
            })
            .0
        };
        for resource in ["time", "memory", "states", "depth"] {
            let outcome = assess_at(&stopped(resource), 5);
            assert_eq!(outcome, Outcome::Timeout);
            assert!(outcome.finding());
        }
        assert_eq!(assess_at(&stopped("time"), 3), Outcome::MalformedReport);
        assert_eq!(assess_at(&stopped("disk"), 5), Outcome::MalformedReport);
    }

    #[test]
    fn observations_reject_unrequested_search_ceilings() {
        for resource in ["max_states", "max_depth"] {
            let mut document = success();
            document["options"]["budget"][resource] = json!(0);
            assert_eq!(
                mode(&document, &Profile::Tlc, 4),
                Err("study report contains an unrequested search ceiling".into())
            );
        }
    }

    #[test]
    fn schema_three_final_mode_requires_real_owner_settings_and_explicit_family() {
        let legacy = success();
        assert!(mode(&legacy, &Profile::Tlc, 4).is_ok());
        let mut current = legacy.clone();
        current["schema_version"] = json!(3);
        current["check_kind"] = json!("temporal");
        assert!(mode(&current, &Profile::Tlc, 4).is_err());
        current["options"]["temporal_bounds"] = json!({"cell_limits":{"graph_labels":{"kind":"run-budgets"},"attribution_rows":{"kind":"cells","cells":8_000_000},"word_truth":{"kind":"cells","cells":8_000_000}},"acceptance_policy":{"kind":"buffers","bytes":8_000_000,"materialization_work":32_000_000},"acceptance_buffer_bytes":8_000_000,"acceptance_materialization_work":32_000_000});
        assert!(mode(&current, &Profile::Tlc, 4).is_ok());
        for field in ["check_kind", "options"] {
            let mut forged = current.clone();
            forged[field] = Value::Null;
            assert!(mode(&forged, &Profile::Tlc, 4).is_err());
        }
        current["options"]["temporal_bounds"]["atom_cells"] = json!(8_000_000);
        assert!(mode(&current, &Profile::Tlc, 4).is_err());
        assert!(mode(&legacy, &Profile::Tlc, 4).is_ok());
    }

    #[test]
    fn observations_require_the_requested_mode_truth_counts_and_exit() {
        let g = golden(Verdict::Success);
        let v = success();
        assert_eq!(
            assess(&process(&v, 0), &Profile::Tlc, 4, &g, |_| panic!(
                "success has no witness"
            ))
            .0,
            Outcome::MatchesTruthAndCounts
        );
        let mut current = v.clone();
        current["check_kind"] = json!("temporal");
        assert_eq!(
            assess(&process(&current, 0), &Profile::Tlc, 4, &g, |_| panic!(
                "success has no witness"
            ))
            .0,
            Outcome::MatchesTruthAndCounts
        );
        for (field, replacement) in [
            ("schema_version", json!(1)),
            ("schema_version", json!(99)),
            ("check_kind", json!("safety")),
            ("check_kind", json!("unknown")),
            ("check_kind", Value::Null),
            ("verdict", json!("imaginary")),
            ("tlc", json!({"code":2116})),
        ] {
            let mut bad = v.clone();
            bad[field] = replacement;
            assert_eq!(
                assess(
                    &process(&bad, 0),
                    &Profile::Tlc,
                    4,
                    &g,
                    |_| TraceCheck::yes("unused")
                )
                .0,
                Outcome::MalformedReport
            );
        }
        for (field, replacement) in [
            ("profile", json!("tlc")),
            ("seed", json!(1)),
            ("lncheck", json!("periodic")),
            ("tlc_exact_temporal", json!(true)),
        ] {
            let mut bad = v.clone();
            bad["options"][field] = replacement;
            assert_eq!(
                assess(
                    &process(&bad, 0),
                    &Profile::Tlc,
                    4,
                    &g,
                    |_| TraceCheck::yes("unused")
                )
                .0,
                Outcome::MalformedReport
            );
        }
        let mut bad = v.clone();
        bad["statistics"]["distinct"] = json!(1);
        assert_eq!(
            assess(
                &process(&bad, 0),
                &Profile::Tlc,
                4,
                &g,
                |_| TraceCheck::yes("unused")
            )
            .0,
            Outcome::Disagree
        );
        assert_eq!(
            assess(&process(&v, 13), &Profile::Tlc, 4, &g, |_| TraceCheck::yes(
                "unused"
            ))
            .0,
            Outcome::MalformedReport
        );
        assert_eq!(
            assess(&process(&v, 0), &Profile::Tlc, 8, &g, |_| TraceCheck::yes(
                "unused"
            ))
            .0,
            Outcome::MalformedReport
        );
    }
    #[test]
    fn claimed_validation_never_certifies_a_lasso() {
        let v = lasso();
        let g = golden(Verdict::LivenessViolation);
        let p = process(&v, 13);
        assert_eq!(
            assess(&p, &Profile::Tlc, 4, &g, |_| TraceCheck::yes("fresh proof")).0,
            Outcome::MatchesTruthAndCounts
        );
        assert_eq!(
            assess(&p, &Profile::Tlc, 4, &g, |_| TraceCheck::refuted(
                "broken transition"
            ))
            .0,
            Outcome::Disagree
        );
        assert_eq!(
            assess(&p, &Profile::Tlc, 4, &g, |_| TraceCheck::no(
                "missing cover"
            ))
            .0,
            Outcome::ValidationPending
        );
        for (path, replacement) in [
            ("back_to", json!(1)),
            ("length", json!(3)),
            ("ending", json!("finite")),
        ] {
            let mut bad = v.clone();
            bad["counterexample"][path] = replacement;
            assert_eq!(
                assess(&process(&bad, 13), &Profile::Tlc, 4, &g, |_| panic!(
                    "malformed witness reached validation"
                ))
                .0,
                Outcome::MalformedReport
            );
        }
        let mut bad = v.clone();
        bad["counterexample"]["states"][0]["values"]["x"] = json!(true);
        assert_eq!(
            assess(&process(&bad, 13), &Profile::Tlc, 4, &g, |_| panic!(
                "display mismatch reached validation"
            ))
            .0,
            Outcome::MalformedReport
        );
        let mut bad = v.clone();
        bad["properties"] = json!(["P", "P"]);
        assert_eq!(
            assess(&process(&bad, 13), &Profile::Tlc, 4, &g, |_| panic!(
                "duplicate claim reached validation"
            ))
            .0,
            Outcome::MalformedReport
        );
    }
    #[test]
    fn refusals_are_named_timeouts_visible_and_unknown_results_fail() {
        let mut v = success();
        let g = golden(Verdict::Success);
        v["verdict"] = json!("unsupported");
        v["message"] = json!("L0030: temporal state domain");
        assert_eq!(
            assess(&process(&v, 3), &Profile::Tlc, 4, &g, |_| panic!(
                "refusal reached validation"
            ))
            .0,
            Outcome::Unsupported
        );
        v["message"] = json!("");
        assert_eq!(
            assess(&process(&v, 3), &Profile::Tlc, 4, &g, |_| TraceCheck::yes(
                "unused"
            ))
            .0,
            Outcome::MalformedReport
        );
        let mut p = process(&Value::Null, 0);
        p.timed_out = true;
        assert_eq!(
            assess(&p, &Profile::Tlc, 4, &g, |_| TraceCheck::yes("unused")).0,
            Outcome::Timeout
        );
        assert!(Outcome::MalformedReport.finding());
        assert!(!Outcome::Unsupported.finding());
    }
}
