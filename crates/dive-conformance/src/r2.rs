//! Same-host TLC timings and the R2 speed summary (`docs/RELEASE.md`, R2).
//!
//! `run --tlc-timings <report.json>` reads a harness report of pinned TLC at
//! one worker on this host, caps each timed model's run at
//! [`time_limit`] (twice TLC's time, at least [`FLOOR`]), and after the run
//! summarises dive/TLC wall-clock ratios: per model, the geometric mean over
//! the agreeing models, and the models above 1x and 2x. A run that stops at
//! such a budget is already more than 2x slower, so it is listed with its
//! lower bound instead of being run to completion. Models without a cached
//! timing keep the ordinary budget policy and are listed as untimed.
//!
//! ```
//! use dive_conformance::r2::{FLOOR, time_limit};
//! use std::time::Duration;
//!
//! let cap = Duration::from_mins(15);
//! assert_eq!(time_limit(30_000, cap), Duration::from_secs(60));
//! assert_eq!(time_limit(1_000, cap), FLOOR);
//! assert_eq!(time_limit(3_600_000, cap), cap);
//! ```

use crate::compare::Outcome;
use crate::error::{Error, Result};
use crate::report::{Host, Report};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

/// Least time limit a timed model gets: process start-up dominates below it.
pub const FLOOR: Duration = Duration::from_secs(10);

/// The R2 blocking ratio: a model above it blocks the release.
pub const FACTOR: u32 = 2;

/// The Core time limit for a model TLC checked in `tlc_ms`:
/// `max(FLOOR, FACTOR x tlc_ms)`, hard-capped by `cap`.
#[must_use]
pub fn time_limit(tlc_ms: u64, cap: Duration) -> Duration {
    Duration::from_millis(tlc_ms)
        .saturating_mul(FACTOR)
        .max(FLOOR)
        .min(cap)
}

/// Single-worker TLC wall-clock times from one harness report.
#[derive(Debug, Clone)]
pub struct TlcTimings {
    /// The source report's run id.
    pub run_id: String,
    /// The implementation the source report ran (a `tlc:<oracle>`).
    pub implementation: String,
    /// The machine the source report ran on.
    pub host: Host,
    /// `(corpus, model)` to elapsed milliseconds.
    times: BTreeMap<(String, String), u64>,
}

impl TlcTimings {
    /// Reads a harness report of a TLC oracle run.
    ///
    /// # Errors
    /// Fails if the file is not a report, is not a TLC report, or holds no
    /// usable single-worker time.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
        let report: Report = serde_json::from_str(&text)
            .map_err(|e| Error::Config(format!("{}: not a harness report: {e}", path.display())))?;
        Self::from_report(&report)
            .map_err(|why| Error::Config(format!("{}: {why}", path.display())))
    }

    /// Takes the single-worker time of every model the report's TLC run
    /// completed with its golden's result (`agree` or
    /// `known-upstream-change`). A time-out or oracle error is no TLC time.
    ///
    /// # Errors
    /// Fails if the report is not of a TLC oracle or yields no time.
    pub fn from_report(report: &Report) -> std::result::Result<Self, String> {
        if !(report.implementation == "tlc" || report.implementation.starts_with("tlc:")) {
            return Err(format!(
                "a TLC timing report must be of `--impl tlc`, not `{}`",
                report.implementation
            ));
        }
        let mut times = BTreeMap::new();
        for m in &report.models {
            let run = m.runs.iter().find(|r| {
                r.workers == 1 && matches!(r.outcome, Outcome::Agree | Outcome::KnownUpstreamChange)
            });
            if let Some(run) = run {
                times.insert((m.corpus.clone(), m.model.clone()), run.elapsed_ms);
            }
        }
        if times.is_empty() {
            return Err("the report has no completed single-worker TLC run".to_owned());
        }
        Ok(Self {
            run_id: report.run_id.clone(),
            implementation: report.implementation.clone(),
            host: report.host.clone(),
            times,
        })
    }

    /// Timings from explicit `((corpus, model), elapsed ms)` pairs.
    #[must_use]
    pub fn new(
        run_id: &str,
        host: Host,
        times: impl IntoIterator<Item = ((String, String), u64)>,
    ) -> Self {
        Self {
            run_id: run_id.to_owned(),
            implementation: "tlc".to_owned(),
            host,
            times: times.into_iter().collect(),
        }
    }

    /// TLC's single-worker milliseconds for a model, if cached.
    #[must_use]
    pub fn elapsed_ms(&self, corpus: &str, model: &str) -> Option<u64> {
        self.times
            .get(&(corpus.to_owned(), model.to_owned()))
            .copied()
    }

    /// Number of timed models.
    #[must_use]
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Whether no model is timed (never true for a loaded report).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// The timeout policy line recorded in the run's settings.
    #[must_use]
    pub fn policy(&self, cap: Duration) -> String {
        format!(
            "--tlc-timings {}: a model with a cached TLC time gets max({} s, {FACTOR} x TLC elapsed), hard cap {} s",
            self.run_id,
            FLOOR.as_secs(),
            cap.as_secs()
        )
    }
}

/// One model's dive/TLC comparison.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct R2Row {
    /// Corpus id.
    pub corpus: String,
    /// Model id.
    pub model: String,
    /// The model's single-worker outcome.
    pub outcome: String,
    /// dive's wall-clock milliseconds, including start-up.
    pub dive_ms: u64,
    /// TLC's cached wall-clock milliseconds, including JVM start-up.
    pub tlc_ms: u64,
    /// `dive_ms / tlc_ms`.
    pub ratio: f64,
    /// The run stopped at its budget, so `ratio` is only a lower bound.
    pub lower_bound: bool,
}

impl R2Row {
    fn label(&self) -> String {
        format!(
            "{} {}{:.2}x ({} / {} ms)",
            self.model,
            if self.lower_bound { ">=" } else { "" },
            self.ratio,
            self.dive_ms,
            self.tlc_ms
        )
    }
}

/// The R2 summary of one run.
#[derive(Debug, Clone, Serialize)]
pub struct R2Summary {
    /// The TLC timing report's run id.
    pub tlc_report: String,
    /// Whether the timing report came from this host (same OS, arch,
    /// kernel, CPU count and memory). Times from another host are triage
    /// only.
    pub same_host: bool,
    /// Agreeing models (`agree` or `expected-divergence`) with a TLC time:
    /// the R2 set.
    pub agreeing: Vec<R2Row>,
    /// Geometric mean of dive/TLC over `agreeing`.
    pub geomean: Option<f64>,
    /// Runs stopped at their budget (`timeout`), with lower-bound ratios.
    pub stopped: Vec<R2Row>,
    /// Agreeing or stopped models above 1x.
    pub over_1x: Vec<R2Row>,
    /// Agreeing or stopped models above 2x: release blockers.
    pub over_2x: Vec<R2Row>,
    /// Models without a cached TLC time (ordinary budget policy).
    pub untimed: Vec<String>,
    /// Timed models with neither an agreeing nor a stopped single-worker
    /// run (refused, disagreeing, harness errors), by outcome.
    pub excluded: BTreeMap<String, usize>,
}

/// Builds the R2 summary of `report`'s single-worker runs against `timings`.
#[must_use]
pub fn summarise(report: &Report, timings: &TlcTimings, this_host: &Host) -> R2Summary {
    let mut agreeing = Vec::new();
    let mut stopped = Vec::new();
    let mut untimed = Vec::new();
    let mut excluded = BTreeMap::new();
    for m in &report.models {
        if m.outcome == Outcome::OutOfScope {
            continue;
        }
        let Some(tlc_ms) = timings.elapsed_ms(&m.corpus, &m.model) else {
            untimed.push(format!("{}:{}", m.corpus, m.model));
            continue;
        };
        let Some(run) = m.runs.iter().find(|r| r.workers == 1) else {
            *excluded.entry("no-1-worker-run".to_owned()).or_insert(0) += 1;
            continue;
        };
        #[allow(
            clippy::cast_precision_loss,
            reason = "millisecond wall times are far below 2^52"
        )]
        let ratio = run.elapsed_ms as f64 / tlc_ms.max(1) as f64;
        let row = |lower_bound| R2Row {
            corpus: m.corpus.clone(),
            model: m.model.clone(),
            outcome: run.outcome.as_str().to_owned(),
            dive_ms: run.elapsed_ms,
            tlc_ms,
            ratio,
            lower_bound,
        };
        match run.outcome {
            Outcome::Agree | Outcome::ExpectedDivergence => agreeing.push(row(false)),
            Outcome::Timeout => stopped.push(row(true)),
            other => *excluded.entry(other.as_str().to_owned()).or_insert(0) += 1,
        }
    }
    let geomean = (!agreeing.is_empty()).then(|| {
        #[allow(
            clippy::cast_precision_loss,
            reason = "the model count is far below 2^52"
        )]
        let n = agreeing.len() as f64;
        (agreeing
            .iter()
            .map(|r| r.ratio.max(f64::MIN_POSITIVE).ln())
            .sum::<f64>()
            / n)
            .exp()
    });
    let mut ranked: Vec<R2Row> = agreeing.iter().chain(&stopped).cloned().collect();
    ranked.sort_by(|a, b| b.ratio.total_cmp(&a.ratio));
    // A stopped run did not finish within its limit, so a lower bound
    // equal to the threshold is already above it.
    let over = |x: f64| {
        ranked
            .iter()
            .filter(|r| r.ratio > x || (r.lower_bound && r.ratio >= x))
            .cloned()
            .collect()
    };
    R2Summary {
        tlc_report: timings.run_id.clone(),
        same_host: same_host(&timings.host, this_host),
        geomean,
        over_1x: over(1.0),
        over_2x: over(f64::from(FACTOR)),
        agreeing,
        stopped,
        untimed,
        excluded,
    }
}

fn same_host(a: &Host, b: &Host) -> bool {
    a.os == b.os
        && a.arch == b.arch
        && a.kernel == b.kernel
        && a.cpus == b.cpus
        && a.memory_bytes == b.memory_bytes
}

impl R2Summary {
    /// Plain-text rendering for the run's standard output.
    #[must_use]
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "R2 (dive/TLC wall clock, 1 worker; TLC times from {}):",
            self.tlc_report
        );
        if !self.same_host {
            let _ = writeln!(
                s,
                "  warning: the TLC timings come from a different host; triage only"
            );
        }
        let _ = writeln!(
            s,
            "  geomean over {} agreeing model(s): {}",
            self.agreeing.len(),
            self.geomean
                .map_or_else(|| "n/a".to_owned(), |g| format!("{g:.3}x"))
        );
        let mut rows = self.agreeing.clone();
        rows.sort_by(|a, b| b.ratio.total_cmp(&a.ratio));
        for r in &rows {
            let _ = writeln!(s, "    {}", r.label());
        }
        let list = |s: &mut String, title: &str, rows: &[R2Row]| {
            let _ = writeln!(s, "  {title}: {}", rows.len());
            for r in rows {
                let _ = writeln!(s, "    {} [{}]", r.label(), r.outcome);
            }
        };
        list(&mut s, "stopped at budget (lower bounds)", &self.stopped);
        list(&mut s, "above 1x", &self.over_1x);
        list(&mut s, "above 2x (release blockers)", &self.over_2x);
        let _ = writeln!(
            s,
            "  untimed (no cached TLC time; ordinary budget policy): {}",
            self.untimed.len()
        );
        for m in &self.untimed {
            let _ = writeln!(s, "    {m}");
        }
        if !self.excluded.is_empty() {
            let parts: Vec<String> = self
                .excluded
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect();
            let _ = writeln!(s, "  timed but not compared: {}", parts.join(", "));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{ModelReport, Settings};

    fn model(id: &str, outcome: Outcome, elapsed_ms: u64) -> Result<ModelReport> {
        serde_json::from_value(serde_json::json!({
            "corpus": "examples", "model": id, "module": "M.tla", "tier": "small",
            "outcome": outcome, "detail": null, "golden_verdict": null,
            "worker_invariance": null, "manifest_drift": [],
            "runs": [{
                "workers": 1, "outcome": outcome, "verdict": null,
                "verdict_code": null, "exit_code": null, "distinct_states": null,
                "states_generated": null, "depth": null, "trace_length": null,
                "diffs": [], "elapsed_ms": elapsed_ms, "detail": null
            }]
        }))
        .map_err(|e| Error::Config(e.to_string()))
    }

    fn report(implementation: &str, models: Vec<ModelReport>) -> Result<Report> {
        let n = models.len();
        Ok(Report {
            schema_version: crate::report::REPORT_SCHEMA,
            run_id: format!("run-{implementation}"),
            generated_at: "2026-11-06T00:00:00Z".to_owned(),
            generator: "test".to_owned(),
            implementation: implementation.to_owned(),
            implementation_oracle: None,
            golden_oracle: "tlc".to_owned(),
            golden_oracle_sha256: String::new(),
            golden_oracle_role: None,
            implementation_oracle_role: None,
            reference: false,
            implementation_dive: None,
            corpora: Vec::new(),
            host: host(),
            settings: Settings {
                tier: "all".to_owned(),
                filters: Vec::new(),
                workers: vec![1],
                jobs: 1,
                xmx: "3g".to_owned(),
                timeout_policy: String::new(),
                crosscheck_manifest: false,
                sany_phase: crate::sany::SanyPhase::Full,
                impl_options: Vec::new(),
                impl_profile: None,
                strict_tier: None,
                impl_engine: None,
            },
            summary: crate::report::summarise(&models, &[], n, 0)?,
            models,
            modules: Vec::new(),
        })
    }

    fn host() -> Host {
        Host {
            os: "linux".to_owned(),
            arch: "x86_64".to_owned(),
            kernel: Some("Linux 7".to_owned()),
            cpus: 32,
            memory_bytes: Some(1 << 30),
        }
    }

    fn timings() -> Result<TlcTimings> {
        let tlc = report(
            "tlc:pinned",
            vec![
                model("Fast.cfg", Outcome::Agree, 10_000)?,
                model("Slow.cfg", Outcome::Agree, 4_000)?,
                model("Stopped.cfg", Outcome::Agree, 30_000)?,
                model("Refused.cfg", Outcome::KnownUpstreamChange, 1_000)?,
                // A TLC time-out is no TLC time: the model stays untimed.
                model("TlcTimeout.cfg", Outcome::Timeout, 600_000)?,
            ],
        )?;
        TlcTimings::from_report(&tlc).map_err(Error::Config)
    }

    #[test]
    fn limits_are_twice_tlc_with_a_floor_and_the_hard_cap() {
        let cap = Duration::from_mins(15);
        assert_eq!(time_limit(0, cap), FLOOR);
        assert_eq!(time_limit(4_999, cap), FLOOR);
        assert_eq!(time_limit(7_500, cap), Duration::from_secs(15));
        assert_eq!(time_limit(600_000, cap), cap, "the cap still holds");
        assert_eq!(time_limit(u64::MAX, cap), cap);
        assert_eq!(
            time_limit(1_000, Duration::from_secs(5)),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn only_completed_single_worker_tlc_runs_are_timings() -> Result<()> {
        let t = timings()?;
        assert_eq!(t.len(), 4);
        assert_eq!(t.elapsed_ms("examples", "Fast.cfg"), Some(10_000));
        assert_eq!(t.elapsed_ms("examples", "Refused.cfg"), Some(1_000));
        assert_eq!(t.elapsed_ms("examples", "TlcTimeout.cfg"), None);
        assert_eq!(t.elapsed_ms("probes", "Fast.cfg"), None, "keyed by corpus");
        let dive = report("dive (dive)", vec![model("Fast.cfg", Outcome::Agree, 1)?])?;
        assert!(TlcTimings::from_report(&dive).is_err(), "not a TLC report");
        let empty = report("tlc", vec![model("X.cfg", Outcome::Timeout, 1)?])?;
        assert!(TlcTimings::from_report(&empty).is_err(), "no usable time");
        Ok(())
    }

    #[test]
    fn r2_summary_ratios_geomean_and_threshold_lists() -> Result<()> {
        let t = timings()?;
        let mut out_of_scope = model("Skipped.cfg", Outcome::OutOfScope, 0)?;
        out_of_scope.runs.clear();
        let dive = report(
            "dive (dive)",
            vec![
                model("Fast.cfg", Outcome::Agree, 2_500)?,       // 0.25x
                model("Slow.cfg", Outcome::Agree, 16_000)?,      // 4x
                model("Stopped.cfg", Outcome::Timeout, 60_000)?, // >= 2x
                model("Refused.cfg", Outcome::Unsupported, 5)?,
                model("TlcTimeout.cfg", Outcome::Agree, 1_000)?,
                model("New.cfg", Outcome::Agree, 1_000)?,
                out_of_scope,
            ],
        )?;
        let r2 = summarise(&dive, &t, &host());
        assert!(r2.same_host);
        let names = |rows: &[R2Row]| rows.iter().map(|r| r.model.clone()).collect::<Vec<_>>();
        assert_eq!(names(&r2.agreeing), ["Fast.cfg", "Slow.cfg"]);
        let g = r2
            .geomean
            .ok_or_else(|| Error::Config("no geomean".into()))?;
        assert!((g - 1.0).abs() < 1e-12, "sqrt(0.25 * 4) = 1, got {g}");
        assert_eq!(names(&r2.stopped), ["Stopped.cfg"]);
        assert!(r2.stopped[0].lower_bound);
        assert_eq!(names(&r2.over_1x), ["Slow.cfg", "Stopped.cfg"]);
        assert_eq!(
            names(&r2.over_2x),
            ["Slow.cfg", "Stopped.cfg"],
            "a run stopped at 2x did not finish within 2x"
        );
        let mut exact = dive.clone();
        exact.models[1].runs[0].elapsed_ms = 8_000;
        let exact = summarise(&exact, &t, &host());
        assert_eq!(
            names(&exact.over_2x),
            ["Stopped.cfg"],
            "a finished run at exactly 2x is not above 2x"
        );
        assert_eq!(r2.untimed, ["examples:TlcTimeout.cfg", "examples:New.cfg"]);
        assert_eq!(r2.excluded.get("unsupported"), Some(&1));
        let text = r2.render();
        assert!(
            text.contains("geomean over 2 agreeing model(s): 1.000x"),
            "{text}"
        );
        assert!(text.contains("Stopped.cfg >=2.00x"), "{text}");
        assert!(!text.contains("different host"), "{text}");
        let other = Host { cpus: 8, ..host() };
        assert!(!summarise(&dive, &t, &other).same_host);
        Ok(())
    }
}
