//! Runs a temporal module and configuration on Core, for the tests that used
//! the retired reference final route (`dive_api::temporal::check_final`, #791).
//!
//! [`check`] writes the source text to a fresh directory, checks it with
//! Core and returns the facts those tests assert: the verdict, the exploration
//! counts and options, and the independently validated word. Nothing here is
//! computed by the test; every fact comes from Core's report.
#![allow(dead_code, reason = "each test binary uses part of the adapter")]
#![allow(clippy::panic, reason = "test adapter: a failed setup ends the test")]

use dive_api::temporal::{Error, Failure, Outcome as Core};
use dive_api::{CheckReport, CheckRequest, Engine, Store};
use dive_check::{CheckOptions, CheckResult};
use dive_eval::Profile;
use dive_eval::next::StateVals;
use dive_trace::{Report as ValidationReport, TraceDoc};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A module and configuration written to a fresh directory, removed on drop.
pub struct Files {
    directory: PathBuf,
    /// The module file; its configuration sits beside it with `.cfg`.
    pub file: PathBuf,
}

impl Files {
    /// Writes `source` (whose third word is the module name) and `cfg`.
    ///
    /// # Panics
    /// When the directory or files cannot be written.
    #[must_use]
    pub fn new(source: &str, cfg: &str) -> Self {
        let name = source.split_whitespace().nth(2).unwrap_or("Spec");
        let directory = std::env::temp_dir().join(format!(
            "dive-core-final-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap_or_else(|e| panic!("{e}"));
        let file = directory.join(format!("{name}.tla"));
        std::fs::write(&file, source).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(file.with_extension("cfg"), cfg).unwrap_or_else(|e| panic!("{e}"));
        Self { directory, file }
    }

    /// The Core request equivalent to `opts` for these files.
    #[must_use]
    pub fn request(&self, opts: &CheckOptions) -> CheckRequest {
        CheckRequest {
            file: self.file.clone(),
            config: Some(self.file.with_extension("cfg")),
            ..request(opts)
        }
    }
}

/// The Core request equivalent to `opts`, without files.
#[must_use]
pub fn request(opts: &CheckOptions) -> CheckRequest {
    CheckRequest {
        compat_tlc: opts.profile == Profile::Tlc,
        store: match opts.store {
            dive_store::StoreMode::Exact => Store::Exact,
            dive_store::StoreMode::Fingerprint => Store::Fingerprint,
        },
        seed: Some(opts.seed),
        fp: Some(opts.fp_index),
        workers: Some(opts.workers),
        no_deadlock: opts.no_deadlock,
        sequential: opts.sequential,
        chaos: opts.chaos,
        symmetry_liveness: Some(opts.symmetry_liveness),
        ..CheckRequest::default()
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Checks `source` and `cfg` on Core with the options `opts` describes.
///
/// # Panics
/// When the files cannot be written or loaded.
#[must_use]
pub fn run(source: &str, cfg: &str, opts: &CheckOptions) -> CheckReport {
    let files = Files::new(source, cfg);
    dive_api::check_file_engine(&files.request(opts), Engine::Core)
        .unwrap_or_else(|e| panic!("Core check: {e}"))
}

/// Checks `source` and `cfg` on Core: the run when it reached a verdict
/// (including a safety result), otherwise the stopped report.
///
/// # Errors
/// A [`Stop`] when Core refused, rejected or failed the run.
pub fn check(
    source: impl AsRef<str>,
    cfg: impl AsRef<str>,
    opts: &CheckOptions,
) -> Result<Run, Stop> {
    let report = run(source.as_ref(), cfg.as_ref(), opts);
    let verdict = report.verdict();
    if verdict != "unsupported"
        && verdict != "rejected"
        && (report.temporal_result().is_some() || report.result().is_some())
    {
        let outcome = outcome(&report);
        Ok(Run { report, outcome })
    } else {
        Err(Stop {
            report: Box::new(report),
        })
    }
}

/// Exploration counts from Core's evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    /// States generated.
    pub generated: u64,
    /// Distinct states found.
    pub distinct: u64,
    /// Depth of the complete search.
    pub depth: u64,
}

/// Workers requested and used, from Core's report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Workers {
    /// Requested.
    pub requested: usize,
    /// Used.
    pub used: usize,
}

/// The exhausted exploration behind a temporal verdict.
#[derive(Clone, Debug)]
pub struct Exploration {
    /// Counts.
    pub stats: Stats,
    /// Workers.
    pub workers: Workers,
    /// The options Core explored with.
    pub options: CheckOptions,
}

/// The selected word of a validated violation.
#[derive(Debug)]
pub struct Candidate {
    /// The complete false-property set.
    pub properties: Vec<String>,
    /// Original states of the word.
    pub states: Vec<StateVals>,
    /// Loop position.
    pub back_to: usize,
    /// The exploration that produced it.
    pub exploration: Exploration,
}

/// An independently validated temporal violation.
#[derive(Debug)]
pub struct Violation {
    candidate: Candidate,
    document: TraceDoc,
    validation: ValidationReport,
}

impl Violation {
    /// The selected word.
    #[must_use]
    pub const fn candidate(&self) -> &Candidate {
        &self.candidate
    }
    /// The serialized word that passed independent validation.
    #[must_use]
    pub const fn document(&self) -> &TraceDoc {
        &self.document
    }
    /// The independent replay.
    #[must_use]
    pub const fn validation(&self) -> &ValidationReport {
        &self.validation
    }
}

/// A Core verdict.
#[derive(Debug)]
pub enum Outcome {
    /// Every property holds on the exhausted graph.
    Holds(Box<Exploration>),
    /// A validated word falsifies the named properties.
    Violation(Box<Violation>),
    /// The ordinary safety result (no temporal verdict, or a safety stop).
    Safety(Box<CheckResult>),
}

/// A Core run that reached a verdict.
#[derive(Debug)]
pub struct Run {
    report: CheckReport,
    outcome: Outcome,
}

impl Run {
    /// The full report.
    #[must_use]
    pub const fn report(&self) -> &CheckReport {
        &self.report
    }
    /// The verdict.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }
}

fn exploration(
    report: &CheckReport,
    stats: dive_check::Stats,
    options: &CheckOptions,
) -> Exploration {
    let workers = &report.to_json()["options"]["workers"];
    let count = |key: &str| {
        workers[key]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or_else(|| panic!("workers.{key}: {workers}"))
    };
    Exploration {
        stats: Stats {
            generated: stats.generated,
            distinct: stats.distinct,
            depth: stats.depth,
        },
        workers: Workers {
            requested: count("requested"),
            used: count("used"),
        },
        options: options.clone(),
    }
}

/// The verdict of a report that reached one. Core reports a prefix word only
/// under a cap, which these runs never set.
fn outcome(report: &CheckReport) -> Outcome {
    let Some(temporal) = report.temporal_result() else {
        let result = report.result().unwrap_or_else(|| panic!("no result"));
        return Outcome::Safety(Box::new(result.clone()));
    };
    match temporal.outcome() {
        Core::NativeHolds(e) => {
            Outcome::Holds(Box::new(exploration(report, e.stats(), e.options())))
        }
        Core::NativeViolation(v) => Outcome::Violation(Box::new(Violation {
            candidate: Candidate {
                properties: v.word().properties.clone(),
                states: v.word().states.clone(),
                back_to: v.word().back_to,
                exploration: exploration(report, v.evidence().stats(), v.evidence().options()),
            },
            document: v.document().clone(),
            validation: v.validation().clone(),
        })),
        Core::Safety(r) => Outcome::Safety(r.clone()),
        Core::NativePrefixViolation(v) => panic!("Core final checking returned a prefix: {v:?}"),
    }
}

/// A Core run that stopped without a verdict.
#[derive(Debug)]
pub struct Stop {
    report: Box<CheckReport>,
}

impl Stop {
    /// The full report.
    #[must_use]
    pub const fn report(&self) -> &CheckReport {
        &self.report
    }
    /// The typed temporal failure, when temporal checking failed.
    #[must_use]
    pub fn failure(&self) -> Option<&Failure> {
        self.report.temporal_failure()
    }
    /// Its temporal error, when temporal checking failed.
    #[must_use]
    pub fn error(&self) -> Option<&Error> {
        self.failure().map(Failure::error)
    }
    /// The verdict name (`unsupported`, `rejected`, ...).
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        self.report.verdict()
    }
    /// The temporal error's stable code, or the verdict when there is none.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.error().map_or_else(|| self.verdict(), Error::code)
    }
    /// The process exit code.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        self.report.exit_code()
    }
    /// The report document.
    #[must_use]
    pub fn json(&self) -> Value {
        self.report.to_json()
    }
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.error() {
            Some(e) => write!(f, "{e}"),
            None => write!(f, "{}", self.json()["message"].as_str().unwrap_or("")),
        }
    }
}
