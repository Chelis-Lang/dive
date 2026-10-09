//! Model checking (`dive check`): load, bind and check a specification, and
//! report the result as a versioned JSON document or TLC-style text.

use std::path::PathBuf;
use std::sync::Arc;

use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_eval::Profile;
use dive_sema::{Options, SearchPath};
use dive_store::StoreMode;
use serde_json::{Value, json};

use crate::{AnalyzeReport, ConfigReport};

/// Version of the JSON document produced by [`CheckReport::to_json`].
pub const CHECK_SCHEMA_VERSION: u32 = 1;

/// Exit codes of `dive check` in dive's default profile. Under
/// `--compat=tlc` a verdict exits with TLC's status instead
/// (`EC.ExitStatus`: 0, 10–14, 75–77, 150, 151, 255), and the codes below
/// marked "both" keep their meaning.
pub mod exit {
    /// No error found.
    pub const SUCCESS: u8 = 0;
    /// The specification or configuration was rejected, or TLC's binding
    /// of the model failed (dive profile; TLC's 150/151 under `--compat=tlc`).
    pub const REJECTED: u8 = 1;
    /// A malformed command line or unreadable input (both).
    pub const USAGE: u8 = 2;
    /// dive refused, naming the feature, or hit a resource limit; no verdict
    /// (both).
    pub const UNSUPPORTED: u8 = 3;
    /// An internal error in dive; no verdict (both).
    pub const INTERNAL: u8 = 4;
    /// An invariant, property, action property or postcondition is violated.
    pub const VIOLATION: u8 = 10;
    /// A deadlock was reached.
    pub const DEADLOCK: u8 = 11;
    /// An assumption is false or failed to evaluate.
    pub const ASSUMPTION: u8 = 12;
    /// Evaluation failed: an error, a failed `Assert`, an overflow, an
    /// incompletely specified state.
    pub const EVALUATION: u8 = 13;
}

/// The store `dive check` uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Store {
    /// Fingerprints (the default, as in TLC).
    #[default]
    Fingerprint,
    /// Full states (`--store exact`, D-007).
    Exact,
}

/// What to check and how.
#[derive(Clone, Debug, Default)]
pub struct CheckRequest {
    /// The root module file.
    pub file: PathBuf,
    /// The configuration; `<file stem>.cfg` beside it by default, as in TLC.
    pub config: Option<PathBuf>,
    /// Module directories searched before the standard modules.
    pub library_paths: Vec<PathBuf>,
    /// Module directories searched after the standard modules.
    pub module_paths: Vec<PathBuf>,
    /// `--compat=tlc`.
    pub compat_tlc: bool,
    /// The store.
    pub store: Store,
    /// `--seed`; a fresh seed otherwise (always reported).
    pub seed: Option<i64>,
    /// `--fp`; TLC picks one at random under `--compat=tlc`, dive uses 0.
    pub fp: Option<usize>,
    /// `--workers`; `None` is the profile's default (TLC's 1 under
    /// `--compat=tlc`, the available parallelism otherwise).
    pub workers: Option<usize>,
    /// TLC's `-deadlock`: do not check for deadlock.
    pub no_deadlock: bool,
    /// `--sequential`: every level inline with one logical worker, as
    /// sequential mode does (ADR 0015 §6). Results are identical.
    pub sequential: bool,
    /// Test-only chaos scheduling (`--chaos SEED`, or `DIVE_CHAOS`); see
    /// `dive_check::CheckOptions::chaos`. Results must be identical.
    pub chaos: Option<u64>,
}

/// The phase a check ended in.
#[derive(Debug)]
enum Phase {
    /// The specification was rejected by the front end.
    SpecRejected(Box<AnalyzeReport>),
    /// The configuration was rejected.
    ConfigRejected(Box<ConfigReport>),
    /// The specification could not be lowered (a refusal).
    Unlowered(String),
    /// The checker ran.
    Checked(Box<CheckResult>),
}

/// The outcome of `dive check`.
#[derive(Debug)]
pub struct CheckReport {
    file: String,
    config: String,
    profile: Profile,
    opts: CheckOptions,
    seed_given: bool,
    fp_given: bool,
    workers_requested: usize,
    phase: Phase,
}

/// Loads, binds and checks the request.
///
/// # Errors
/// [`crate::ApiError`] if the root file or the configuration cannot be read.
pub fn check_file(req: &CheckRequest) -> Result<CheckReport, crate::ApiError> {
    let profile = if req.compat_tlc {
        Profile::Tlc
    } else {
        Profile::Dive
    };
    let seed = req.seed.unwrap_or_else(dive_stdlib::random::fresh_seed);
    let fp = req.fp.unwrap_or_else(|| {
        if req.compat_tlc {
            // TLC: a random polynomial per run unless `-fp` is given
            // (`TLC.java`); reported either way.
            usize::try_from(seed.unsigned_abs() % 131).unwrap_or(0)
        } else {
            0
        }
    });
    let workers_requested = req.workers.unwrap_or_else(|| {
        if req.compat_tlc {
            1
        } else {
            std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
        }
    });
    let mut opts = if req.compat_tlc {
        CheckOptions::tlc(fp, seed)
    } else {
        let mut o = CheckOptions::dive(seed);
        o.fp_index = fp;
        o
    };
    opts.store = match req.store {
        Store::Fingerprint => StoreMode::Fingerprint,
        Store::Exact => StoreMode::Exact,
    };
    opts.no_deadlock = req.no_deadlock;
    opts.workers = workers_requested;
    opts.sequential = req.sequential;
    opts.chaos = req.chaos;
    let cfg_path = req
        .config
        .clone()
        .unwrap_or_else(|| req.file.with_extension("cfg"));
    let mut report = CheckReport {
        file: req.file.display().to_string(),
        config: cfg_path.display().to_string(),
        profile,
        opts,
        seed_given: req.seed.is_some(),
        fp_given: req.fp.is_some(),
        workers_requested,
        phase: Phase::Unlowered(String::new()),
    };
    let search = Options {
        search: SearchPath {
            library_paths: req.library_paths.clone(),
            after_standard: req.module_paths.clone(),
        },
        ..Options::default()
    };
    let analysis =
        dive_sema::analyze_file(&req.file, &search).map_err(|error| crate::ApiError::Io {
            path: report.file.clone(),
            error,
        })?;
    if !analysis.accepted() {
        report.phase = Phase::SpecRejected(Box::new(AnalyzeReport::from_analysis(
            report.file.clone(),
            analysis,
        )));
        return Ok(report);
    }
    let cfg = crate::parse_config_file(&cfg_path)?;
    let Some(model) = cfg.config().cloned() else {
        report.phase = Phase::ConfigRejected(Box::new(cfg));
        return Ok(report);
    };
    let ir = match dive_ir::lower(&analysis) {
        Ok(ir) => Arc::new(ir),
        Err(e) => {
            report.phase = Phase::Unlowered(format!("lowering the specification: {e:?}"));
            return Ok(report);
        }
    };
    let result = dive_check::check(&ir, &model, &report.opts);
    report.phase = Phase::Checked(Box::new(result));
    Ok(report)
}

impl CheckReport {
    /// The checker's result, when the checker ran.
    #[must_use]
    pub fn result(&self) -> Option<&CheckResult> {
        match &self.phase {
            Phase::Checked(r) => Some(r),
            _ => None,
        }
    }

    /// The verdict's name in the JSON document.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        match &self.phase {
            Phase::SpecRejected(_) | Phase::ConfigRejected(_) => "rejected",
            Phase::Unlowered(_) => "unsupported",
            Phase::Checked(r) => r.verdict.kind(),
        }
    }

    /// The process exit code (see [`exit`]).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        let tlc = self.profile == Profile::Tlc;
        match &self.phase {
            Phase::SpecRejected(_) => {
                if tlc {
                    150
                } else {
                    exit::REJECTED
                }
            }
            Phase::ConfigRejected(_) => {
                if tlc {
                    151
                } else {
                    exit::REJECTED
                }
            }
            Phase::Unlowered(_) => exit::UNSUPPORTED,
            Phase::Checked(r) => {
                let v = &r.verdict;
                match v {
                    Verdict::Unsupported { .. } | Verdict::ResourceLimit { .. } => {
                        exit::UNSUPPORTED
                    }
                    Verdict::InternalError { .. } => exit::INTERNAL,
                    _ if tlc => v.tlc().map_or(exit::INTERNAL, |c| c.exit),
                    Verdict::Success => exit::SUCCESS,
                    Verdict::InvariantViolated { .. }
                    | Verdict::PropertyViolatedInitially { .. }
                    | Verdict::ActionPropertyViolated { .. }
                    | Verdict::PostconditionFalse { .. } => exit::VIOLATION,
                    Verdict::Deadlock => exit::DEADLOCK,
                    Verdict::AssumptionFalse { .. }
                    | Verdict::EvaluationError {
                        site: dive_check::ErrorSite::Assumption,
                        ..
                    } => exit::ASSUMPTION,
                    Verdict::ModelError { .. } => exit::REJECTED,
                    Verdict::EvaluationError { .. } | Verdict::NotCompletelySpecified { .. } => {
                        exit::EVALUATION
                    }
                }
            }
        }
    }

    fn profile_name(&self) -> &'static str {
        match self.profile {
            Profile::Tlc => "tlc",
            Profile::Dive => "dive",
        }
    }

    /// The `options` block: profile, options and seed (ADR 0010).
    fn options_json(&self) -> Value {
        let o = &self.opts;
        let e = &o.eval;
        let int = match e.ctx.int_mode {
            dive_value::IntMode::Tlc => "tlc",
            dive_value::IntMode::Big => "big",
        };
        let order = match e.ctx.order {
            dive_value::ValueOrder::Tlc => "tlc",
            dive_value::ValueOrder::Canonical => "canonical",
        };
        let explore = match e.exploration {
            dive_eval::ExplorationOrder::Tlc => "tlc",
            dive_eval::ExplorationOrder::Value => "value",
        };
        let comparisons = match e.comparisons {
            dive_eval::Comparisons::Tlc => "tlc",
            dive_eval::Comparisons::Tla => "tla",
        };
        let fingerprint = (o.store == StoreMode::Fingerprint).then(|| {
            json!({
                "polynomial_index": o.fp_index,
                "polynomial_source": if self.fp_given { "flag" } else if self.profile == Profile::Tlc { "random" } else { "default" },
                "scheme": if o.tlc_fingerprints { "tlc" } else { "dive" },
            })
        });
        json!({
            "profile": self.profile_name(),
            "int": int,
            "choose_order": order,
            "explore_order": explore,
            "comparisons": comparisons,
            "warnings": e.warnings,
            "store": o.store.name(),
            "fingerprint": fingerprint,
            "seed": o.seed,
            "seed_source": if self.seed_given { "flag" } else { "fresh" },
            "deadlock_flag": o.no_deadlock,
            "workers": self.workers_json(),
        })
    }

    /// The workers requested and used: `{ requested, used, sequential,
    /// reason, chaos }`. These, with the timings, are the only fields of the
    /// document that may differ between worker counts.
    fn workers_json(&self) -> Value {
        let w = match &self.phase {
            Phase::Checked(r) => r.workers.clone(),
            _ => dive_check::Workers {
                requested: self.workers_requested,
                used: 0,
                sequential: false,
                reason: Some("no search ran".to_owned()),
            },
        };
        json!({
            "requested": w.requested,
            "used": w.used,
            "sequential": w.sequential,
            "reason": w.reason,
            "chaos": self.opts.chaos,
        })
    }

    /// The workers used, for the human header.
    fn workers_used(&self) -> usize {
        match &self.phase {
            Phase::Checked(r) => r.workers.used,
            _ => 0,
        }
    }

    /// The JSON document (schema [`CHECK_SCHEMA_VERSION`]):
    ///
    /// ```text
    /// { "schema_version": 1, "command": "check", "file", "config",
    ///   "verdict": "success" | "invariant-violation" | "property-violation"
    ///            | "action-property-violation" | "deadlock" | "assumption-failure"
    ///            | "postcondition-failure" | "assertion-failure" | "overflow"
    ///            | "evaluation-error" | "incomplete-state" | "model-error"
    ///            | "rejected" | "unsupported" | "resource-limit" | "internal-error",
    ///   "property": string | null, "message": string | null,
    ///   "error_site": string | null,
    ///   "tlc": { "code", "name", "exit_status", "verified" } | null,
    ///   "statistics": { "generated", "distinct", "left_on_queue", "depth",
    ///                   "initial_generated", "reported_by_tlc" } | null,
    ///   "counterexample": { "length", "ending": "finite",
    ///                       "states": [ { "index", "label", "action",
    ///                                     "values": { var: TLC JSON }, "text" } ],
    ///                       "trace": dive-trace document | null } | null,
    ///   "collision": { "store", "scheme", "optimistic", "actual", "assumption" } | null,
    ///   "output": [ string ], "warnings": [ { "code", "message" } ],
    ///   "diagnostics": [ ... ], "notes": [ string ],
    ///   "options": { "profile", "int", "choose_order", "explore_order",
    ///                "comparisons", "warnings", "store", "fingerprint",
    ///                "seed", "seed_source", "deadlock_flag",
    ///                "workers": { "requested", "used", "sequential", "reason",
    ///                             "chaos" } },
    ///   "elapsed_ms" }
    /// ```
    ///
    /// `tlc` is how TLC classifies the result; `verified: false` means dive
    /// does not yet reproduce TLC's error class at that site and does not
    /// claim it. `reported_by_tlc` is false where TLC prints no statistics
    /// (a stop while generating initial states).
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "one document")]
    pub fn to_json(&self) -> Value {
        let mut doc = json!({
            "schema_version": CHECK_SCHEMA_VERSION,
            "command": "check",
            "file": self.file,
            "config": self.config,
            "verdict": self.verdict(),
            "property": Value::Null,
            "message": Value::Null,
            "error_site": Value::Null,
            "tlc": Value::Null,
            "statistics": Value::Null,
            "counterexample": Value::Null,
            "collision": Value::Null,
            "output": [],
            "warnings": [],
            "diagnostics": [],
            "notes": [],
            "options": self.options_json(),
            "elapsed_ms": 0,
        });
        match &self.phase {
            Phase::SpecRejected(a) => {
                doc["diagnostics"] = a.to_json()["diagnostics"].clone();
                // TLC reports every spec SANY rejects, in its parse or its
                // semantic phase, as `TLC_PARSING_FAILED` (3002), exit 150.
                // Verified against captured runs of the pinned TLC: harness
                // fixtures `ParseError` and `SemanticError`, and the goldens
                // of the probes `M1Syntax` (a syntax error) and `M2Undefined`
                // (an undefined name) in `conformance/corpus/probes/`. Like
                // every other verdict's class, it is reported in both
                // profiles.
                doc["tlc"] = json!({"code": 3002, "name": "TLC_PARSING_FAILED", "exit_status": 150, "verified": true});
            }
            Phase::ConfigRejected(c) => {
                doc["diagnostics"] = c.to_json()["diagnostics"].clone();
            }
            Phase::Unlowered(why) => doc["message"] = Value::from(why.as_str()),
            Phase::Checked(r) => {
                let ctx = self.opts.eval.ctx;
                let v = &r.verdict;
                doc["property"] = v.property().map_or(Value::Null, Value::from);
                doc["message"] = match v {
                    Verdict::AssumptionFalse { message }
                    | Verdict::EvaluationError { message, .. }
                    | Verdict::ModelError { message, .. }
                    | Verdict::ResourceLimit { message }
                    | Verdict::InternalError { message } => Value::from(message.as_str()),
                    Verdict::Unsupported { feature } => Value::from(feature.as_str()),
                    Verdict::NotCompletelySpecified { variables, .. } => {
                        Value::from(format!("unassigned: {}", variables.join(", ")))
                    }
                    _ => Value::Null,
                };
                if let Verdict::EvaluationError { site, .. } = v {
                    doc["error_site"] = Value::from(site.name());
                }
                if let Some(c) = v.tlc() {
                    doc["tlc"] = json!({
                        "code": c.code,
                        "name": c.name,
                        "exit_status": c.exit,
                        "verified": c.verified,
                    });
                }
                if let Some(s) = &r.stats {
                    doc["statistics"] = json!({
                        "generated": s.generated,
                        "distinct": s.distinct,
                        "left_on_queue": s.left_on_queue,
                        "depth": s.depth,
                        "initial_generated": s.initial_generated,
                        "reported_by_tlc": r.tlc_prints_stats,
                    });
                }
                if let Some(cx) = &r.counterexample {
                    let states: Vec<Value> = cx
                        .states
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let values: serde_json::Map<String, Value> = r
                                .tlc_var_order
                                .iter()
                                .filter_map(|&k| {
                                    let name = r.var_names.get(k)?.clone();
                                    let v = s.values.get(k)?;
                                    let j = dive_value::json::to_tlc_json(v, ctx)
                                        .ok()
                                        .and_then(|t| serde_json::from_str(&t).ok())
                                        .unwrap_or(Value::Null);
                                    Some((name, j))
                                })
                                .collect();
                            json!({
                                "index": i + 1,
                                "label": s.label,
                                "action": s.action,
                                "values": values,
                                "text": dive_check::format_trace_state(r, s, ctx),
                            })
                        })
                        .collect();
                    doc["counterexample"] = json!({
                        "length": states.len(),
                        "ending": "finite",
                        "states": states,
                        "trace": trace_doc(r, cx, ctx),
                    });
                }
                if let Some(c) = &r.collision {
                    doc["collision"] = json!({
                        "store": "fingerprint",
                        "scheme": if r.tlc_fingerprints { "tlc" } else { "dive" },
                        "optimistic": c.optimistic,
                        "actual": c.actual,
                        "assumption": "fingerprints behave as random 64-bit values (TLC's assumption too)",
                    });
                }
                doc["output"] = json!(r.output);
                doc["warnings"] = r
                    .warnings
                    .iter()
                    .map(|(c, m)| json!({"code": c, "message": m}))
                    .collect();
                doc["notes"] = json!(r.notes);
                doc["elapsed_ms"] = json!(u64::try_from(r.elapsed.as_millis()).unwrap_or(u64::MAX));
            }
        }
        doc
    }

    /// The TLC-style text report (see `dive_check::render_human`), preceded
    /// by the run's options and the `Print` output.
    #[must_use]
    pub fn render_human(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let o = &self.opts;
        let fp = if o.store == StoreMode::Fingerprint {
            format!(" (fp {})", o.fp_index)
        } else {
            String::new()
        };
        let _ = writeln!(
            out,
            "dive check {}: profile {}, store {}{fp}, seed {}, {} worker(s) requested, {} used",
            self.file,
            self.profile_name(),
            o.store.name(),
            o.seed,
            self.workers_requested,
            self.workers_used()
        );
        if let Phase::Checked(r) = &self.phase
            && let Some(why) = &r.workers.reason
        {
            let _ = writeln!(out, "{why}");
        }
        match &self.phase {
            Phase::SpecRejected(a) => out.push_str(&a.render_human()),
            Phase::ConfigRejected(c) => out.push_str(&c.render_human()),
            Phase::Unlowered(why) => {
                let _ = writeln!(out, "unsupported: {why}; no verdict produced");
            }
            Phase::Checked(r) => {
                for (_, w) in &r.warnings {
                    let _ = writeln!(out, "Warning: {w}");
                }
                for line in &r.output {
                    let _ = writeln!(out, "{line}");
                }
                out.push_str(&dive_check::render_human(r, o.eval.ctx));
            }
        }
        out
    }
}

/// The counterexample in `dive-trace`'s format (version 1, exact values), the
/// document the independent validator reads (ADR 0016): `dive
/// validate-trace` and the conformance harness take it from
/// `counterexample.trace`. `null` when a value has no JSON form or the
/// verdict has no ending the format names.
fn trace_doc(
    r: &dive_check::CheckResult,
    cx: &dive_check::Counterexample,
    ctx: dive_value::Ctx,
) -> Value {
    use dive_check::Verdict as V;
    use dive_trace::Ending;
    let ending = match &r.verdict {
        V::InvariantViolated { property, .. } => Ending::Invariant(property.clone()),
        V::PropertyViolatedInitially { property } | V::ActionPropertyViolated { property } => {
            Ending::Property(property.clone())
        }
        V::Deadlock => Ending::Deadlock,
        V::EvaluationError { name, .. } => {
            if *name == "TLC_VALUE_ASSERT_FAILED" {
                Ending::Assertion
            } else {
                Ending::Error {
                    ec: Some((*name).to_owned()),
                }
            }
        }
        _ => return Value::Null,
    };
    let mut states = Vec::with_capacity(cx.states.len());
    let mut labels = Vec::with_capacity(cx.states.len());
    for (i, s) in cx.states.iter().enumerate() {
        let mut m = serde_json::Map::new();
        for (k, name) in r.var_names.iter().enumerate() {
            let Some(v) = s.values.get(k) else {
                return Value::Null;
            };
            let Ok(j) = dive_trace::value_json::to_json(v, ctx) else {
                return Value::Null;
            };
            m.insert(name.clone(), j);
        }
        states.push(m);
        labels.push(if i == 0 {
            None
        } else {
            s.action.as_ref().map(|name| dive_trace::Label {
                name: name.clone(),
                location: Some(s.label.clone()),
            })
        });
    }
    dive_trace::TraceDoc {
        variables: r.var_names.clone(),
        states,
        labels,
        ending: Some(ending),
        length: Some(cx.states.len()),
        source: dive_trace::Source::Dive,
    }
    .to_json()
}
