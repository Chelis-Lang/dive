//! Committed corpus baselines and regression checks (`run --baseline`).
//!
//! A [`Baseline`] is a compact digest of one [`Report`]: each model's outcome
//! and its observables per worker count (verdict, error class and code,
//! physical exit, violated property, distinct and generated counts, depth),
//! and each module's outcome. It leaves out everything that legitimately
//! varies between runs (elapsed time, details, hosts, diffs), so it stays
//! small enough to commit under `conformance/baselines/`.
//!
//! [`compare`] checks a new report against a baseline of the same
//! implementation, profile, golden oracle, tier and worker counts. A
//! **regression** fails the run:
//! - a model or module that was `agree` is now anything else, or missing;
//! - a model run that had a result (a verdict) at some worker count now has
//!   none, or any of its observables changed.
//!
//! An **improvement** (anything to `agree`) and other outcome changes are
//! reported but pass. A refreshed baseline records them.
//!
//! ```
//! use dive_conformance::baseline::{Baseline, compare};
//!
//! let base: Baseline = serde_json::from_str(r#"{
//!     "schema_version": 1, "source_run_id": "r0", "implementation": "dive",
//!     "profile": "tlc", "golden_oracle": "o", "tier": "small", "workers": [1],
//!     "corpora": ["examples"], "filters": [],
//!     "models": {"examples:M.cfg": {"outcome": "agree", "runs": [
//!         {"workers": 1, "verdict": "success", "distinct_states": 3}]}},
//!     "modules": {}
//! }"#).unwrap();
//! let mut now = base.clone();
//! now.models.get_mut("examples:M.cfg").unwrap().runs[0].distinct_states = Some(4);
//! let diff = compare(&base, &now).unwrap();
//! assert_eq!(diff.regressions.len(), 1);
//! ```

use crate::compare::Outcome;
use crate::error::{Error, Result};
use crate::report::Report;
use crate::tlc::result::Verdict;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Version of the baseline schema. Bump on any incompatible change.
pub const BASELINE_SCHEMA: u32 = 1;

/// The observables of one model run at one worker count (ADR 0086).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observed {
    /// Worker count.
    pub workers: u32,
    /// Verdict class; absent when the implementation produced no result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,
    /// Deciding message code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict_code: Option<i32>,
    /// Error class name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// Physical exit code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Violated property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub violated_property: Option<String>,
    /// Distinct states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinct_states: Option<u64>,
    /// Generated states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub states_generated: Option<u64>,
    /// Depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u64>,
}

impl Observed {
    /// The observable fields that differ from `other`, by name.
    fn changed_fields(&self, other: &Self) -> Vec<String> {
        let mut out = Vec::new();
        let mut field = |name: &str, a: String, b: String| {
            if a != b {
                out.push(format!("{name} {a} -> {b}"));
            }
        };
        let show = |v: Option<String>| v.unwrap_or_else(|| "-".to_owned());
        field(
            "verdict",
            show(self.verdict.map(|v| v.as_str().to_owned())),
            show(other.verdict.map(|v| v.as_str().to_owned())),
        );
        field(
            "verdict_code",
            show(self.verdict_code.map(|v| v.to_string())),
            show(other.verdict_code.map(|v| v.to_string())),
        );
        field(
            "error_class",
            show(self.error_class.clone()),
            show(other.error_class.clone()),
        );
        field(
            "exit_code",
            show(self.exit_code.map(|v| v.to_string())),
            show(other.exit_code.map(|v| v.to_string())),
        );
        field(
            "violated_property",
            show(self.violated_property.clone()),
            show(other.violated_property.clone()),
        );
        field(
            "distinct_states",
            show(self.distinct_states.map(|v| v.to_string())),
            show(other.distinct_states.map(|v| v.to_string())),
        );
        field(
            "states_generated",
            show(self.states_generated.map(|v| v.to_string())),
            show(other.states_generated.map(|v| v.to_string())),
        );
        field(
            "depth",
            show(self.depth.map(|v| v.to_string())),
            show(other.depth.map(|v| v.to_string())),
        );
        out
    }
}

/// One model's baseline entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEntry {
    /// The model's single outcome.
    pub outcome: Outcome,
    /// Observables per worker count.
    pub runs: Vec<Observed>,
}

/// A compact, committable digest of one conformance report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// [`BASELINE_SCHEMA`].
    pub schema_version: u32,
    /// The run the baseline was derived from.
    pub source_run_id: String,
    /// The implementation under test.
    pub implementation: String,
    /// The implementation's profile (`dive`, `tlc`, ...), if it has one.
    #[serde(default)]
    pub profile: Option<String>,
    /// The oracle whose goldens were compared against.
    pub golden_oracle: String,
    /// Tier selection.
    pub tier: String,
    /// Worker counts.
    pub workers: Vec<u32>,
    /// Corpus ids, sorted.
    pub corpora: Vec<String>,
    /// `--filter` globs.
    #[serde(default)]
    pub filters: Vec<String>,
    /// Models keyed by `<corpus>:<model>`.
    pub models: BTreeMap<String, ModelEntry>,
    /// Module paths keyed by `<corpus>:<module>`, with their outcome.
    pub modules: BTreeMap<String, Outcome>,
}

impl Baseline {
    /// Digests `report`.
    #[must_use]
    pub fn from_report(report: &Report) -> Self {
        let profile = report.settings.impl_profile.as_ref().map(|p| {
            serde_json::to_value(p)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| serde_json::to_string(p).unwrap_or_default())
        });
        let mut corpora: Vec<String> = report.corpora.iter().map(|c| c.id.clone()).collect();
        corpora.sort();
        let models = report
            .models
            .iter()
            .map(|m| {
                let runs = m
                    .runs
                    .iter()
                    .map(|r| Observed {
                        workers: r.workers,
                        verdict: r.verdict,
                        verdict_code: r.verdict_code,
                        error_class: r.error_class.clone(),
                        exit_code: r.exit_code,
                        violated_property: r.violated_property.clone(),
                        distinct_states: r.distinct_states,
                        states_generated: r.states_generated,
                        depth: r.depth,
                    })
                    .collect();
                (
                    format!("{}:{}", m.corpus, m.model),
                    ModelEntry {
                        outcome: m.outcome,
                        runs,
                    },
                )
            })
            .collect();
        let modules = report
            .modules
            .iter()
            .map(|m| (format!("{}:{}", m.corpus, m.module), m.outcome))
            .collect();
        Self {
            schema_version: BASELINE_SCHEMA,
            source_run_id: report.run_id.clone(),
            implementation: report.implementation.clone(),
            profile,
            golden_oracle: report.golden_oracle.clone(),
            tier: report.settings.tier.clone(),
            workers: report.settings.workers.clone(),
            corpora,
            filters: report.settings.filters.clone(),
            models,
            modules,
        }
    }

    /// Reads a baseline file, or derives one from a full report file.
    ///
    /// # Errors
    /// Fails if the file cannot be read or is neither format, or on a schema
    /// mismatch.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| Error::Config(format!("{}: not JSON: {e}", path.display())))?;
        let baseline = if value.get("source_run_id").is_some() {
            serde_json::from_value::<Self>(value)
                .map_err(|e| Error::Config(format!("{}: not a baseline: {e}", path.display())))?
        } else {
            let report: Report = serde_json::from_value(value).map_err(|e| {
                Error::Config(format!(
                    "{}: neither a baseline nor a report: {e}",
                    path.display()
                ))
            })?;
            Self::from_report(&report)
        };
        if baseline.schema_version != BASELINE_SCHEMA {
            return Err(Error::Config(format!(
                "{}: baseline schema {} (expected {BASELINE_SCHEMA})",
                path.display(),
                baseline.schema_version
            )));
        }
        Ok(baseline)
    }

    /// Writes the baseline as JSON, one model or module per line, so that a
    /// refresh diffs line by line.
    ///
    /// # Errors
    /// Fails if the file cannot be written.
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut out = String::from("{\n");
        let _ = writeln!(out, "  \"schema_version\": {},", self.schema_version);
        for (k, v) in [
            ("source_run_id", line(&self.source_run_id)),
            ("implementation", line(&self.implementation)),
            ("profile", line(&self.profile)),
            ("golden_oracle", line(&self.golden_oracle)),
            ("tier", line(&self.tier)),
            ("workers", line(&self.workers)),
            ("corpora", line(&self.corpora)),
            ("filters", line(&self.filters)),
        ] {
            let _ = writeln!(out, "  \"{k}\": {v},");
        }
        out.push_str("  \"models\": {");
        push_map(&mut out, self.models.iter().map(|(k, v)| (k, line(v))));
        out.push_str("},\n  \"modules\": {");
        push_map(&mut out, self.modules.iter().map(|(k, v)| (k, line(v))));
        out.push_str("}\n}\n");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, &e))?;
        }
        std::fs::write(path, out).map_err(|e| Error::io(path, &e))
    }
}

fn push_map<'a>(out: &mut String, entries: impl Iterator<Item = (&'a String, String)>) {
    let mut first = true;
    for (k, v) in entries {
        out.push_str(if first { "\n" } else { ",\n" });
        first = false;
        let _ = write!(out, "    {}: {v}", serde_json::Value::String(k.clone()));
    }
    if !first {
        out.push_str("\n  ");
    }
}

/// Compact single-line JSON of `v`.
fn line<T: Serialize + ?Sized>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".to_owned())
}

/// One entry of a baseline comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// `model` or `module`.
    pub kind: &'static str,
    /// `<corpus>:<id>`.
    pub id: String,
    /// What changed.
    pub what: String,
}

/// The result of [`compare`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaselineDiff {
    /// Changes that fail the run.
    pub regressions: Vec<Change>,
    /// Outcomes that became `agree`.
    pub improvements: Vec<Change>,
    /// Other outcome changes, and entries new to the run.
    pub other: Vec<Change>,
}

impl BaselineDiff {
    /// A Markdown-ish, line-per-change rendering.
    #[must_use]
    pub fn render(&self, baseline: &str) -> String {
        let mut out = format!(
            "baseline {baseline}: {} regression(s), {} improvement(s), {} other change(s)\n",
            self.regressions.len(),
            self.improvements.len(),
            self.other.len()
        );
        for (label, list) in [
            ("REGRESSION", &self.regressions),
            ("improved", &self.improvements),
            ("changed", &self.other),
        ] {
            for c in list {
                let _ = writeln!(out, "  {label} {} {}: {}", c.kind, c.id, c.what);
            }
        }
        out
    }
}

/// Compares `now` with `base`.
///
/// # Errors
/// A usage error when the two do not describe the same kind of run
/// (implementation, profile, golden oracle, tier, worker counts or corpora).
pub fn compare(base: &Baseline, now: &Baseline) -> Result<BaselineDiff> {
    let mismatch = [
        ("implementation", base.implementation == now.implementation),
        ("profile", base.profile == now.profile),
        ("golden oracle", base.golden_oracle == now.golden_oracle),
        ("tier", base.tier == now.tier),
        ("workers", base.workers == now.workers),
        ("corpora", base.corpora == now.corpora),
    ]
    .into_iter()
    .filter(|(_, same)| !same)
    .map(|(name, _)| name)
    .collect::<Vec<_>>();
    if !mismatch.is_empty() {
        return Err(Error::Usage(format!(
            "--baseline {} does not describe this run: {} differ",
            base.source_run_id,
            mismatch.join(", ")
        )));
    }
    // A filtered run checks only what it selected.
    let partial = !now.filters.is_empty() || base.filters != now.filters;
    let mut d = BaselineDiff::default();
    let outcome = |o: Outcome| o.as_str();
    for (id, b) in &base.models {
        let change = |what: String| Change {
            kind: "model",
            id: id.clone(),
            what,
        };
        let Some(n) = now.models.get(id) else {
            if !partial {
                d.regressions
                    .push(change(format!("{} -> missing", outcome(b.outcome))));
            }
            continue;
        };
        let mut fields = Vec::new();
        for br in b.runs.iter().filter(|r| r.verdict.is_some()) {
            match n.runs.iter().find(|r| r.workers == br.workers) {
                Some(nr) if nr.verdict.is_some() => {
                    for f in br.changed_fields(nr) {
                        fields.push(format!("w{}: {f}", br.workers));
                    }
                }
                _ => fields.push(format!("w{}: result lost", br.workers)),
            }
        }
        let moved = b.outcome != n.outcome;
        let what = {
            let mut parts = Vec::new();
            if moved {
                parts.push(format!("{} -> {}", outcome(b.outcome), outcome(n.outcome)));
            }
            parts.extend(fields.iter().cloned());
            parts.join("; ")
        };
        if (b.outcome == Outcome::Agree && n.outcome != Outcome::Agree) || !fields.is_empty() {
            d.regressions.push(change(what));
        } else if moved && n.outcome == Outcome::Agree {
            d.improvements.push(change(what));
        } else if moved {
            d.other.push(change(what));
        }
    }
    for (id, n) in &now.models {
        if !base.models.contains_key(id) {
            d.other.push(Change {
                kind: "model",
                id: id.clone(),
                what: format!("new ({})", outcome(n.outcome)),
            });
        }
    }
    compare_modules(base, now, partial, &mut d);
    Ok(d)
}

/// The module half of [`compare`].
fn compare_modules(base: &Baseline, now: &Baseline, partial: bool, d: &mut BaselineDiff) {
    let outcome = |o: Outcome| o.as_str();
    for (id, b) in &base.modules {
        let change = |what: String| Change {
            kind: "module",
            id: id.clone(),
            what,
        };
        match now.modules.get(id) {
            None if partial => {}
            None => d
                .regressions
                .push(change(format!("{} -> missing", outcome(*b)))),
            Some(n) if n == b => {}
            Some(n) => {
                let what = format!("{} -> {}", outcome(*b), outcome(*n));
                if *b == Outcome::Agree {
                    d.regressions.push(change(what));
                } else if *n == Outcome::Agree {
                    d.improvements.push(change(what));
                } else {
                    d.other.push(change(what));
                }
            }
        }
    }
    for (id, n) in &now.modules {
        if !base.modules.contains_key(id) {
            d.other.push(Change {
                kind: "module",
                id: id.clone(),
                what: format!("new ({})", outcome(*n)),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(workers: u32, distinct: Option<u64>) -> Observed {
        Observed {
            workers,
            verdict: distinct.map(|_| Verdict::Success),
            verdict_code: distinct.map(|_| 2193),
            exit_code: distinct.map(|_| 0),
            distinct_states: distinct,
            states_generated: distinct.map(|d| d * 3),
            depth: distinct.map(|_| 4),
            ..Observed::default()
        }
    }

    fn entry(outcome: Outcome, distinct: Option<u64>) -> ModelEntry {
        ModelEntry {
            outcome,
            runs: vec![obs(1, distinct)],
        }
    }

    fn base() -> Baseline {
        Baseline {
            schema_version: BASELINE_SCHEMA,
            source_run_id: "r".to_owned(),
            implementation: "dive".to_owned(),
            profile: Some("tlc".to_owned()),
            golden_oracle: "o".to_owned(),
            tier: "small".to_owned(),
            workers: vec![1],
            corpora: vec!["examples".to_owned()],
            filters: Vec::new(),
            models: [
                ("e:A".to_owned(), entry(Outcome::Agree, Some(10))),
                ("e:B".to_owned(), entry(Outcome::Unsupported, None)),
                (
                    "e:C".to_owned(),
                    entry(Outcome::ExpectedDivergence, Some(7)),
                ),
            ]
            .into_iter()
            .collect(),
            modules: [
                ("e:M.tla".to_owned(), Outcome::Agree),
                ("e:N.tla".to_owned(), Outcome::Unsupported),
            ]
            .into_iter()
            .collect(),
        }
    }

    fn ids(list: &[Change]) -> Vec<&str> {
        list.iter().map(|c| c.id.as_str()).collect()
    }

    #[test]
    fn identical_runs_have_no_changes() -> Result<()> {
        assert_eq!(compare(&base(), &base())?, BaselineDiff::default());
        Ok(())
    }

    #[test]
    fn agree_to_unsupported_is_a_regression() -> Result<()> {
        let mut now = base();
        now.models
            .insert("e:A".to_owned(), entry(Outcome::Unsupported, None));
        let d = compare(&base(), &now)?;
        assert_eq!(ids(&d.regressions), ["e:A"]);
        assert!(d.regressions[0].what.contains("agree -> unsupported"));
        assert!(d.regressions[0].what.contains("w1: result lost"));
        Ok(())
    }

    #[test]
    fn changed_observables_are_regressions_even_without_outcome_change() -> Result<()> {
        let mut now = base();
        now.models.insert(
            "e:C".to_owned(),
            entry(Outcome::ExpectedDivergence, Some(8)),
        );
        let d = compare(&base(), &now)?;
        assert_eq!(ids(&d.regressions), ["e:C"]);
        assert!(
            d.regressions[0].what.contains("distinct_states 7 -> 8"),
            "{}",
            d.regressions[0].what
        );
        Ok(())
    }

    #[test]
    fn improvements_pass() -> Result<()> {
        let mut now = base();
        now.models
            .insert("e:B".to_owned(), entry(Outcome::Agree, Some(3)));
        now.modules.insert("e:N.tla".to_owned(), Outcome::Agree);
        let d = compare(&base(), &now)?;
        assert_eq!(d.regressions, []);
        assert_eq!(ids(&d.improvements), ["e:B", "e:N.tla"]);
        Ok(())
    }

    #[test]
    fn module_and_missing_regressions() -> Result<()> {
        let mut now = base();
        now.modules.insert("e:M.tla".to_owned(), Outcome::Disagree);
        now.models.remove("e:A");
        let d = compare(&base(), &now)?;
        assert_eq!(ids(&d.regressions), ["e:A", "e:M.tla"]);
        // A filtered run checks only what it selected.
        now.filters = vec!["e/*".to_owned()];
        now.modules.remove("e:M.tla");
        let d = compare(&base(), &now)?;
        assert_eq!(d.regressions, []);
        Ok(())
    }

    #[test]
    fn new_entries_are_reported() -> Result<()> {
        let mut now = base();
        now.models
            .insert("e:D".to_owned(), entry(Outcome::Agree, Some(1)));
        let d = compare(&base(), &now)?;
        assert_eq!(d.regressions, []);
        assert_eq!(ids(&d.other), ["e:D"]);
        Ok(())
    }

    #[test]
    fn mismatched_runs_are_refused() {
        let mut now = base();
        now.workers = vec![4];
        now.profile = Some("dive".to_owned());
        let err = compare(&base(), &now)
            .map(|_| ())
            .err()
            .map(|e| e.to_string());
        let err = err.unwrap_or_default();
        assert!(err.contains("profile, workers"), "{err}");
    }

    #[test]
    fn write_and_load_round_trip() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("dive-baseline-{}", std::process::id()));
        let path = dir.join("b.json");
        let b = base();
        b.write(&path)?;
        let back = Baseline::load(&path);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, &e));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back?, b);
        assert!(text?.contains("\n    \"e:A\": {\"outcome\":\"agree\""));
        Ok(())
    }
}
