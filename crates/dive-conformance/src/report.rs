//! Conformance reports: `conformance/reports/<run-id>.json` (versioned schema)
//! and the generated summary `docs/conformance.md`.

use crate::compare::{DriftFinding, FieldDiff, Outcome, WorkerInvariance};
use crate::corpus::Tier;
use crate::error::{Error, Result};
use crate::oracle::OracleProvenance;
use crate::sany::SanyVerdict;
use crate::tlc::result::Verdict;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::Command;

/// Version of the report schema. Bump on any incompatible change.
pub const REPORT_SCHEMA: u32 = 1;

/// The machine a run happened on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    /// `std::env::consts::OS`.
    pub os: String,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// `uname -sr`, if available.
    pub kernel: Option<String>,
    /// Logical CPUs.
    pub cpus: usize,
    /// Physical memory in bytes, if known.
    pub memory_bytes: Option<u64>,
}

impl Host {
    /// Describes the current machine (no host name is recorded).
    #[must_use]
    pub fn current() -> Self {
        let kernel = crate::util::capture(Command::new("uname").arg("-sr"))
            .ok()
            .map(|s| s.trim().to_owned());
        let memory_bytes = if cfg!(target_os = "macos") {
            crate::util::capture(Command::new("sysctl").args(["-n", "hw.memsize"]))
                .ok()
                .and_then(|s| s.trim().parse().ok())
        } else {
            std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| {
                m.lines()
                    .find_map(|l| l.strip_prefix("MemTotal:"))
                    .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
                    .map(|kb| kb * 1024)
            })
        };
        Self {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            kernel,
            cpus: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
            memory_bytes,
        }
    }
}

/// One implementation run of one model at one worker count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    /// Worker count.
    pub workers: u32,
    /// Outcome of this run against the golden.
    pub outcome: Outcome,
    /// Verdict, if the implementation produced a result.
    pub verdict: Option<Verdict>,
    /// Deciding message code.
    pub verdict_code: Option<i32>,
    /// Exit code.
    pub exit_code: Option<i32>,
    /// Distinct states.
    pub distinct_states: Option<u64>,
    /// Generated states.
    pub states_generated: Option<u64>,
    /// Depth.
    pub depth: Option<u64>,
    /// Counterexample length.
    pub trace_length: Option<u64>,
    /// Field differences against the golden.
    pub diffs: Vec<FieldDiff>,
    /// Wall-clock milliseconds.
    pub elapsed_ms: u64,
    /// Refusal reason, classification note, or other detail.
    pub detail: Option<String>,
}

/// The report entry for one model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelReport {
    /// Corpus id.
    pub corpus: String,
    /// Model id.
    pub model: String,
    /// Module.
    pub module: String,
    /// Tier.
    pub tier: Tier,
    /// The single outcome for this model.
    pub outcome: Outcome,
    /// Why, when not `agree`.
    pub detail: Option<String>,
    /// The golden's verdict, if a golden exists.
    pub golden_verdict: Option<Verdict>,
    /// Per-worker-count runs.
    pub runs: Vec<RunRecord>,
    /// Worker invariance, when several worker counts were run.
    pub worker_invariance: Option<WorkerInvariance>,
    /// Manifest cross-check findings.
    pub manifest_drift: Vec<DriftFinding>,
    /// The module reads `TLCGet("config")` (which includes the worker
    /// count), so its result may legitimately depend on `-workers`.
    #[serde(default)]
    pub worker_introspecting: bool,
}

/// The report entry for one module (SANY).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleReport {
    /// Corpus id.
    pub corpus: String,
    /// Module path.
    pub module: String,
    /// Outcome.
    pub outcome: Outcome,
    /// Golden verdict.
    pub golden_verdict: Option<SanyVerdict>,
    /// Observed verdict.
    pub verdict: Option<SanyVerdict>,
    /// Differences.
    pub diffs: Vec<FieldDiff>,
    /// Detail.
    pub detail: Option<String>,
}

/// Corpus identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusInfo {
    /// Id.
    pub id: String,
    /// Commit.
    pub commit: String,
    /// Declared but unfetched submodules.
    pub unfetched_submodules: Vec<String>,
    /// Spec directories and `.tla` files no manifest covers (warnings).
    #[serde(default)]
    pub uncovered: Vec<String>,
}

/// Settings of the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Tier selection.
    pub tier: String,
    /// `--filter` globs.
    pub filters: Vec<String>,
    /// Worker counts.
    pub workers: Vec<u32>,
    /// Parallel jobs.
    pub jobs: usize,
    /// JVM heap limit.
    pub xmx: String,
    /// Timeout policy.
    pub timeout_policy: String,
    /// Whether the manifest cross-check ran.
    pub crosscheck_manifest: bool,
    /// Implementation options active for the run (ADR 0010).
    #[serde(default)]
    pub impl_options: Vec<crate::implementation::ImplOption>,
    /// The implementation's profile (`dive`, `tlc`, `tlc+overrides`), if it
    /// has one (TLC oracles and external tools do not).
    #[serde(default)]
    pub impl_profile: Option<crate::divergence::Profile>,
    /// Whether agreement in this run counts for the strict tier: only the
    /// pure `tlc` profile without result-changing options. `None` when the
    /// implementation has no profile.
    #[serde(default)]
    pub strict_tier: Option<bool>,
}

/// Outcome counts.
pub type OutcomeCounts = BTreeMap<Outcome, usize>;

/// Summary counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Models selected for the run. The model outcome counts sum to this.
    pub models_selected: usize,
    /// Modules selected for the run. The module outcome counts sum to this.
    pub modules_selected: usize,
    /// Models by outcome (every outcome present, zero included).
    pub models: OutcomeCounts,
    /// Modules by outcome.
    pub modules: OutcomeCounts,
    /// Models with at least one non-informational manifest drift finding.
    pub models_with_oracle_drift: usize,
    /// Non-informational drift findings.
    pub oracle_drift_findings: usize,
    /// Informational drift findings (`stateDepth`).
    pub informational_drift_findings: usize,
    /// Models whose verdict varied with the worker count.
    pub worker_verdict_variant: usize,
    /// Models whose complete-search counts varied with the worker count.
    pub worker_counts_variant: usize,
    /// Models whose depth varied with the worker count.
    pub worker_depth_variant: usize,
    /// Models whose module reads `TLCGet("config")`.
    pub worker_introspecting: usize,
    /// Models whose named violated property varied with the worker count.
    pub worker_property_variant: usize,
}

/// A conformance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// [`REPORT_SCHEMA`].
    pub schema_version: u32,
    /// Run id (also the file name).
    pub run_id: String,
    /// RFC 3339 UTC.
    pub generated_at: String,
    /// `dive-conformance <version>`.
    pub generator: String,
    /// The implementation under test.
    pub implementation: String,
    /// The implementation's oracle provenance, if it is a TLC oracle.
    pub implementation_oracle: Option<OracleProvenance>,
    /// The oracle whose goldens were compared against.
    pub golden_oracle: String,
    /// That oracle's pinned SHA-256.
    pub golden_oracle_sha256: String,
    /// The golden oracle's role (`gating` or `reference`).
    #[serde(default)]
    pub golden_oracle_role: Option<crate::config::Role>,
    /// The implementation's oracle role, if it is a TLC oracle.
    #[serde(default)]
    pub implementation_oracle_role: Option<crate::config::Role>,
    /// Whether a reference oracle took part, on either side. Such a run
    /// never gates: its findings do not fail the command.
    #[serde(default)]
    pub reference: bool,
    /// Corpora.
    pub corpora: Vec<CorpusInfo>,
    /// Host.
    pub host: Host,
    /// Settings.
    pub settings: Settings,
    /// Summary.
    pub summary: Summary,
    /// Per-model results, sorted.
    pub models: Vec<ModelReport>,
    /// Per-module (SANY) results, sorted.
    pub modules: Vec<ModuleReport>,
}

fn count(outcomes: impl Iterator<Item = Outcome>) -> OutcomeCounts {
    let mut m: OutcomeCounts = Outcome::ALL.iter().map(|o| (*o, 0)).collect();
    for o in outcomes {
        *m.entry(o).or_default() += 1;
    }
    m
}

/// Computes the summary of `models` and `modules`, which must account for
/// exactly `models_selected` models and `modules_selected` modules.
///
/// # Errors
/// Fails (as an internal error) if the outcome counts do not add up to the
/// selection: every selected model and module must have exactly one outcome.
pub fn summarise(
    models: &[ModelReport],
    modules: &[ModuleReport],
    models_selected: usize,
    modules_selected: usize,
) -> Result<Summary> {
    let drift = |m: &ModelReport, informational: bool| {
        m.manifest_drift
            .iter()
            .filter(|d| d.informational == informational)
            .count()
    };
    let inv = |f: fn(&WorkerInvariance) -> bool| {
        models
            .iter()
            .filter(|m| m.worker_invariance.as_ref().is_some_and(f))
            .count()
    };
    let summary = Summary {
        models_selected,
        modules_selected,
        models: count(models.iter().map(|m| m.outcome)),
        modules: count(modules.iter().map(|m| m.outcome)),
        models_with_oracle_drift: models.iter().filter(|m| drift(m, false) > 0).count(),
        oracle_drift_findings: models.iter().map(|m| drift(m, false)).sum(),
        informational_drift_findings: models.iter().map(|m| drift(m, true)).sum(),
        worker_verdict_variant: inv(|w| !w.verdict_invariant),
        worker_counts_variant: inv(|w| w.counts_invariant == Some(false)),
        worker_depth_variant: inv(|w| w.depth_varies),
        worker_property_variant: inv(|w| w.property_varies),
        worker_introspecting: models.iter().filter(|m| m.worker_introspecting).count(),
    };
    let model_total: usize = summary.models.values().sum();
    let module_total: usize = summary.modules.values().sum();
    if model_total != models_selected || module_total != modules_selected {
        return Err(Error::Command(format!(
            "internal error: outcomes cover {model_total}/{models_selected} models and {module_total}/{modules_selected} modules"
        )));
    }
    Ok(summary)
}

/// Renders the generated `docs/conformance.md` from a report.
///
/// # Errors
/// Never in practice (writing to a `String`); kept fallible for symmetry.
#[allow(clippy::too_many_lines, reason = "a linear document template")]
pub fn render_markdown(r: &Report) -> Result<String> {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "<!-- GENERATED by dive-conformance from conformance/reports/{}.json. DO NOT EDIT. -->\n",
        r.run_id
    );
    let _ = writeln!(s, "# Conformance summary\n");
    let _ = writeln!(
        s,
        "This file is generated by `dive-conformance run`. It summarises the latest report; the JSON report is the source of truth.\n"
    );
    let _ = writeln!(s, "- **Run:** `{}` ({})", r.run_id, r.generated_at);
    let role = |x: Option<crate::config::Role>| {
        x.map(|x| format!(", {} oracle", x.as_str()))
            .unwrap_or_default()
    };
    let _ = writeln!(
        s,
        "- **Implementation:** `{}`{}",
        r.implementation,
        role(r.implementation_oracle_role)
    );
    let _ = writeln!(
        s,
        "- **Goldens:** `{}` (jar SHA-256 `{}`{})",
        r.golden_oracle,
        r.golden_oracle_sha256,
        role(r.golden_oracle_role)
    );
    if r.reference {
        let _ = writeln!(
            s,
            "- **Reference run:** a reference oracle took part. Its results are for comparison only and never gate (ADR 0002)."
        );
    }
    for c in &r.corpora {
        let _ = writeln!(s, "- **Corpus:** `{}` at `{}`", c.id, c.commit);
        if !c.unfetched_submodules.is_empty() {
            let _ = writeln!(
                s,
                "  - unfetched submodules: {}",
                c.unfetched_submodules
                    .iter()
                    .map(|p| format!("`{p}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    for c in &r.corpora {
        if !c.uncovered.is_empty() {
            let _ = writeln!(
                s,
                "- **Not covered by any manifest ({}):** {}",
                c.uncovered.len(),
                c.uncovered
                    .iter()
                    .take(20)
                    .map(|p| format!("`{p}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
                    + if c.uncovered.len() > 20 { ", ..." } else { "" }
            );
        }
    }
    let _ = writeln!(
        s,
        "- **Settings:** tier `{}`, workers {:?}, filters {:?}, manifest cross-check {}",
        r.settings.tier, r.settings.workers, r.settings.filters, r.settings.crosscheck_manifest
    );
    if let Some(p) = &r.settings.impl_profile {
        let _ = writeln!(
            s,
            "- **Profile:** `{}`; strict tier: {}",
            serde_json::to_string(p).unwrap_or_default(),
            r.settings.strict_tier.unwrap_or(false)
        );
    }
    if !r.settings.impl_options.is_empty() {
        let _ = writeln!(
            s,
            "- **Implementation options:** {}",
            r.settings
                .impl_options
                .iter()
                .map(|o| format!(
                    "`{}{}`{}",
                    o.name,
                    o.value
                        .as_ref()
                        .map(|v| format!("={v}"))
                        .unwrap_or_default(),
                    if o.result_changing {
                        " (result-changing)"
                    } else {
                        ""
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let _ = writeln!(
        s,
        "- **Host:** {} {} ({}), {} CPUs\n",
        r.host.os,
        r.host.arch,
        r.host.kernel.as_deref().unwrap_or("unknown kernel"),
        r.host.cpus
    );
    let _ = writeln!(
        s,
        "{} model(s) and {} module(s) selected; every one has exactly one outcome below.\n",
        r.summary.models_selected, r.summary.modules_selected
    );
    let _ = writeln!(s, "## Outcomes\n");
    let _ = writeln!(s, "| Outcome | Models | Modules (SANY) |\n|---|---:|---:|");
    for o in Outcome::ALL {
        let _ = writeln!(
            s,
            "| {} | {} | {} |",
            o.as_str(),
            r.summary.models.get(&o).copied().unwrap_or(0),
            r.summary.modules.get(&o).copied().unwrap_or(0)
        );
    }
    let _ = writeln!(
        s,
        "\nOracle drift against the Examples manifests: {} finding(s) in {} model(s), plus {} informational `stateDepth` finding(s).",
        r.summary.oracle_drift_findings,
        r.summary.models_with_oracle_drift,
        r.summary.informational_drift_findings
    );
    if r.settings.workers.len() > 1 {
        let _ = writeln!(
            s,
            "Worker invariance ({:?}, TLC's own; reported, not counted as disagreement): verdict varied in {}, complete-search counts varied in {}, depth varied in {}, violated property varied in {} model(s). {} model(s) read `TLCGet(\"config\")`.",
            r.settings.workers,
            r.summary.worker_verdict_variant,
            r.summary.worker_counts_variant,
            r.summary.worker_depth_variant,
            r.summary.worker_property_variant,
            r.summary.worker_introspecting
        );
    }
    let _ = writeln!(s, "\n## Models\n");
    let _ = writeln!(
        s,
        "| Model | Tier | Outcome | Golden verdict | Distinct | Generated | Depth | Detail |\n|---|---|---|---|---:|---:|---:|---|"
    );
    for m in &r.models {
        let first = m.runs.first();
        let num = |v: Option<u64>| v.map_or_else(String::new, |n| n.to_string());
        let mut detail = m.detail.clone().unwrap_or_default();
        if m.worker_introspecting {
            detail.push_str(" [worker-introspecting: reads TLCGet(\"config\")]");
        }
        if let Some(w) = &m.worker_invariance {
            let mut v = Vec::new();
            if !w.verdict_invariant {
                v.push("verdict");
            }
            if w.counts_invariant == Some(false) {
                v.push("counts");
            }
            if w.depth_varies {
                v.push("depth");
            }
            if w.property_varies {
                v.push("violated property");
            }
            if !v.is_empty() {
                let _ = write!(
                    detail,
                    " TLC worker variance: {} ({});",
                    v.join(", "),
                    w.points
                        .iter()
                        .map(|p| format!(
                            "w{}: {} {}/{} depth {}",
                            p.workers,
                            p.verdict.as_str(),
                            p.distinct_states
                                .map_or_else(|| "-".to_owned(), |n| n.to_string()),
                            p.states_generated
                                .map_or_else(|| "-".to_owned(), |n| n.to_string()),
                            p.depth.map_or_else(|| "-".to_owned(), |n| n.to_string())
                        ))
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            }
        }
        for d in m
            .runs
            .iter()
            .flat_map(|r| r.diffs.iter().map(move |d| (r.workers, d)))
        {
            let _ = write!(
                detail,
                " w{}:{} {}≠{};",
                d.0, d.1.field, d.1.expected, d.1.actual
            );
        }
        for d in &m.manifest_drift {
            let _ = write!(
                detail,
                " manifest {}: {} vs golden {}{};",
                d.field,
                d.manifest,
                d.golden,
                if d.informational {
                    " (informational)"
                } else {
                    ""
                }
            );
        }
        let _ = writeln!(
            s,
            "| `{}` | {:?} | {} | {} | {} | {} | {} | {} |",
            m.model,
            m.tier,
            m.outcome.as_str(),
            m.golden_verdict.map_or("", Verdict::as_str),
            num(first.and_then(|r| r.distinct_states)),
            num(first.and_then(|r| r.states_generated)),
            num(first.and_then(|r| r.depth)),
            detail.trim().replace('|', "\\|")
        );
    }
    if !r.modules.is_empty() {
        let _ = writeln!(s, "\n## Modules (SANY)\n");
        let _ = writeln!(
            s,
            "| Module | Outcome | Golden | Observed | Detail |\n|---|---|---|---|---|"
        );
        for m in &r.modules {
            let v = |x: Option<SanyVerdict>| {
                x.and_then(|v| serde_json::to_value(v).ok())
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            };
            let _ = writeln!(
                s,
                "| `{}` | {} | {} | {} | {} |",
                m.module,
                m.outcome.as_str(),
                v(m.golden_verdict),
                v(m.verdict),
                m.detail.clone().unwrap_or_default().replace('|', "\\|")
            );
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_every_outcome() -> Result<()> {
        let m = ModelReport {
            corpus: "c".into(),
            model: "m".into(),
            module: "M".into(),
            tier: Tier::Small,
            outcome: Outcome::Agree,
            detail: None,
            golden_verdict: Some(Verdict::Success),
            runs: vec![],
            worker_invariance: None,
            worker_introspecting: false,
            manifest_drift: vec![DriftFinding {
                field: "stateDepth".into(),
                manifest: 1.into(),
                golden: 2.into(),
                informational: true,
            }],
        };
        let s = summarise(std::slice::from_ref(&m), &[], 1, 0)?;
        assert!(summarise(&[m], &[], 2, 0).is_err());
        assert_eq!(s.models.len(), Outcome::ALL.len());
        assert_eq!(s.models.get(&Outcome::Agree), Some(&1));
        assert_eq!(
            (s.oracle_drift_findings, s.informational_drift_findings),
            (0, 1)
        );
        Ok(())
    }
}
