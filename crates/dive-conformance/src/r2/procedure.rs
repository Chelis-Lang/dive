//! The `r2` command: the R2 measurement procedure
//! (`docs/developer/performance.md`, "R2 measurement procedure") carried
//! out by the harness.
//!
//! - [`classify`]: one `--jobs 1` TLC pass over the selection, before any
//!   Core run, that fixes the size classes ([`Classes`]).
//! - [`small`]: six repetitions of the small class, TLC then Core, the order
//!   reversed in alternate repetitions; a model's ratio is the median.
//! - [`large`]: one pass of the large class; a ratio in a band or a budget
//!   stop gets three fresh alternating pairs at a configurable Core budget
//!   factor (2.5x by default), whose median is the ratio. A cached TLC first
//!   pass is reused only when its [`Controls`] match and the canaries pass.
//! - [`canary`]: re-times the three shortest large models of a cache.
//! - [`record`]: merges the parts into the compact R2 record.
//!
//! Every timed checker run is a fresh process through the ordinary `run`
//! pipeline at one worker and one job, so timing, budgets, golden
//! comparison and counterexample validation are exactly `run`'s.
//!
//! ```
//! use dive_conformance::r2::procedure::Category;
//! use dive_conformance::r2::stats::Ratio;
//!
//! assert_eq!(Category::of(Some(Ratio::exact(0.4))), Category::Agreeing);
//! assert_eq!(Category::of(Some(Ratio::at_least(2.0))), Category::Stopped);
//! ```

use super::host::{self, Binding, CdsArchive, Environment, HostFacts, OverheadProbe, Warmed};
use super::stats::{self, CanaryVerdict, Ratio, Side, SizeClass};
use crate::cache::Cache;
use crate::compare::Outcome;
use crate::config::Config;
use crate::dive::{DiveEngine, DiveImplementation, DiveProvenance};
use crate::error::{Error, Result};
use crate::golden::to_pretty;
use crate::implementation::TlcImplementation;
use crate::oracle::{Java, JavaSource, OracleEnv};
use crate::pipeline::{self, Limits, RunOptions, Selection};
use crate::util::{rfc3339, unix_now, write_atomic};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Schema of the `r2` command's JSON files.
pub const R2_SCHEMA: u32 = 1;

/// The classification pass's fixed TLC timeout: a model it stops is large.
pub const CLASSIFY_TIMEOUT: Duration = Duration::from_secs(15);

/// The default hard cap on every timed run.
pub const DEFAULT_BUDGET_CAP: Duration = Duration::from_mins(500);

/// The default `--xmx` of timed runs.
pub const DEFAULT_XMX: &str = "24g";

/// Everything that decides whether two timings are comparable. A cached TLC
/// time is reused only when every field matches ([`Controls::mismatches`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    /// The host's facts.
    pub host: HostFacts,
    /// The NUMA binding.
    pub binding: Binding,
    /// The environment, by name.
    pub environment: Environment,
    /// `--jobs` (always 1).
    pub jobs: usize,
    /// `--xmx`.
    pub xmx: String,
    /// The hard cap on every timed run, seconds.
    pub budget_cap_s: u64,
    /// The TLC oracle id.
    pub oracle: String,
    /// Its jar SHA-256.
    pub oracle_sha256: String,
    /// `java -version`'s first line.
    pub jre: String,
    /// The pinned JRE archive's SHA-256.
    pub jre_sha256: String,
    /// The JRE's stock CDS archives.
    pub cds: Vec<CdsArchive>,
    /// Corpus id.
    pub corpus: String,
    /// Corpus commit (its manifests are part of it).
    pub corpus_commit: String,
    /// The harness's TLC command line, with placeholders.
    pub tlc_command: Vec<String>,
    /// `dive-conformance <version>`.
    pub harness: String,
}

impl Controls {
    /// The fields that differ between a cached run's controls and these. A
    /// cache is valid only when this is empty; `classes_only` compares just
    /// what a classification depends on (host type, checker and inputs).
    #[must_use]
    pub fn mismatches(&self, cached: &Self, classes_only: bool) -> Vec<String> {
        let mut out = Vec::new();
        let mut cmp = |name: &str, a: String, b: String| {
            if a != b {
                out.push(format!("{name}: cached {b}, now {a}"));
            }
        };
        let (h, c) = (&self.host, &cached.host);
        cmp(
            "instance",
            format!("{:?}", h.instance),
            format!("{:?}", c.instance),
        );
        cmp("cpu", format!("{:?}", h.cpu), format!("{:?}", c.cpu));
        cmp("arch", h.arch.clone(), c.arch.clone());
        cmp("jobs", self.jobs.to_string(), cached.jobs.to_string());
        cmp("xmx", self.xmx.clone(), cached.xmx.clone());
        cmp(
            "oracle",
            self.oracle_sha256.clone(),
            cached.oracle_sha256.clone(),
        );
        cmp(
            "jre",
            format!("{} {}", self.jre_sha256, self.jre),
            format!("{} {}", cached.jre_sha256, cached.jre),
        );
        cmp(
            "corpus",
            format!("{}@{}", self.corpus, self.corpus_commit),
            format!("{}@{}", cached.corpus, cached.corpus_commit),
        );
        cmp(
            "TLC command line",
            self.tlc_command.join(" "),
            cached.tlc_command.join(" "),
        );
        if classes_only {
            return out;
        }
        cmp("cpus", h.cpus.to_string(), c.cpus.to_string());
        cmp(
            "memory",
            format!("{:?}", h.memory_bytes),
            format!("{:?}", c.memory_bytes),
        );
        cmp(
            "storage",
            format!("{:?}", h.storage),
            format!("{:?}", c.storage),
        );
        cmp("smt", format!("{:?}", h.smt), format!("{:?}", c.smt));
        cmp("turbo", format!("{:?}", h.turbo), format!("{:?}", c.turbo));
        cmp(
            "governor",
            format!("{:?}", h.governor),
            format!("{:?}", c.governor),
        );
        cmp(
            "frequency",
            format!("{:?}-{:?}", h.min_khz, h.max_khz),
            format!("{:?}-{:?}", c.min_khz, c.max_khz),
        );
        cmp(
            "binding",
            self.binding.description.clone(),
            cached.binding.description.clone(),
        );
        cmp(
            "CDS",
            format!("{:?}", self.cds),
            format!("{:?}", cached.cds),
        );
        out
    }

    /// Why a run under these controls is triage rather than a gate number.
    #[must_use]
    pub fn triage(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.binding.applied {
            out.push(format!("no NUMA binding ({})", self.binding.description));
        }
        if !self.environment.extra.is_empty() {
            out.push(format!(
                "not started under env -i: also {}",
                self.environment.extra.join(", ")
            ));
        }
        out.extend(self.host.shortfalls());
        out
    }
}

/// The options of the timed `r2` commands.
#[derive(Debug, Clone)]
pub struct Options {
    /// Corpus id.
    pub corpus: String,
    /// The pinned TLC timed (and whose goldens are compared).
    pub oracle: String,
    /// Which models; intersected with a class for `small` and `large`.
    pub selection: Selection,
    /// The Core binary (`dive`).
    pub dive_bin: PathBuf,
    /// `--xmx` for TLC and Core's `--max-memory`.
    pub xmx: String,
    /// Hard cap on every timed run.
    pub budget_cap: Duration,
    /// Output directory: the part files and every pass's harness report.
    pub out: PathBuf,
    /// The part's label (a shard or node name; may be empty).
    pub label: String,
    /// `r2 large --shard K/N`: time shard K (from 1) of N of the large
    /// class, fixed by longest processing time on the golden TLC times.
    pub shard: Option<(usize, usize)>,
    /// The binding in force.
    pub binding: Binding,
    /// The checked environment.
    pub environment: Environment,
}

impl Options {
    /// `r2-<step>.json`, or `r2-<step>-<label>.json` with a label.
    #[must_use]
    pub fn file_name(&self, step: &str) -> String {
        if self.label.is_empty() {
            format!("r2-{step}.json")
        } else {
            format!("r2-{step}-{}.json", self.label)
        }
    }
}

/// One timed run of one model in a pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timed {
    /// The run's outcome.
    pub outcome: String,
    /// Harness wall clock, milliseconds.
    pub elapsed_ms: u64,
    /// The outcome's detail, shortened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Timed {
    /// TLC completed with its golden's result: a TLC time.
    #[must_use]
    pub fn tlc_ms(&self) -> Option<u64> {
        matches!(self.outcome.as_str(), "agree" | "known-upstream-change")
            .then_some(self.elapsed_ms)
    }

    /// Core's ratio against `tlc_ms`: exact when it agreed, a lower bound
    /// when it stopped at its budget, none otherwise.
    #[must_use]
    pub fn ratio(&self, tlc_ms: Option<u64>) -> Option<Ratio> {
        let tlc = tlc_ms?;
        match self.outcome.as_str() {
            "agree" | "expected-divergence" => Some(Ratio::of(self.elapsed_ms, tlc, false)),
            "timeout" => Some(Ratio::of(self.elapsed_ms, tlc, true)),
            _ => None,
        }
    }
}

/// One harness pass (a `run` over some models with one checker).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pass {
    /// The pass's name in the output directory.
    pub tag: String,
    /// The checker.
    pub side: Side,
    /// The harness report's run id.
    pub run_id: String,
    /// When the pass started (RFC 3339).
    pub started_at: String,
    /// The TLC pass whose times set Core's budgets, and their factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budgets_from: Option<String>,
    /// Per model.
    #[serde(skip)]
    pub runs: BTreeMap<String, Timed>,
}

impl Pass {
    fn get(&self, model: &str) -> Option<&Timed> {
        self.runs.get(model)
    }

    fn tlc_times(&self) -> BTreeMap<String, u64> {
        self.runs
            .iter()
            .filter_map(|(m, t)| Some((m.clone(), t.tlc_ms()?)))
            .collect()
    }
}

/// The size classes, from one TLC pass recorded before any Core run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classes {
    /// [`R2_SCHEMA`].
    pub schema_version: u32,
    /// `r2-classes`.
    pub kind: String,
    /// The controls of the classification pass.
    pub controls: Controls,
    /// The TLC pass.
    pub pass: Pass,
    /// Its fixed TLC timeout, seconds.
    pub timeout_s: u64,
    /// The class boundary, milliseconds.
    pub small_below_ms: u64,
    /// Every classified model: its class and TLC time (`None` when the
    /// classification timeout stopped it).
    pub models: BTreeMap<String, ClassRow>,
    /// Models without a class, with the reason.
    pub not_timed: BTreeMap<String, String>,
}

/// One classified model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassRow {
    /// Its class.
    pub class: SizeClass,
    /// TLC's classification time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tlc_ms: Option<u64>,
}

impl Classes {
    /// The models of `class` that `selection`'s filters accept.
    #[must_use]
    pub fn of(&self, class: SizeClass, selection: &Selection) -> Vec<String> {
        self.models
            .iter()
            .filter(|(m, r)| {
                r.class == class
                    && (selection.filters.is_empty()
                        || crate::util::matches_any(&selection.filters, m))
            })
            .map(|(m, _)| m.clone())
            .collect()
    }
}

/// A model's category in the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Core agreed with the golden; the ratio is exact.
    Agreeing,
    /// Core stopped at its budget; the ratio is a lower bound.
    Stopped,
    /// Core neither agreed nor stopped (refused, disagreed, failed).
    Excluded,
    /// TLC gave no time for it.
    Untimed,
}

impl Category {
    /// The category of a model's final ratio (`None`: excluded).
    #[must_use]
    pub fn of(ratio: Option<Ratio>) -> Self {
        match ratio {
            Some(r) if r.lower_bound => Self::Stopped,
            Some(_) => Self::Agreeing,
            None => Self::Excluded,
        }
    }
}

fn r4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

/// One band re-run pair of a large model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairRow {
    /// `tlc-core` or `core-tlc`.
    pub order: String,
    /// TLC's time, if it completed.
    pub tlc_ms: Option<u64>,
    /// Core's time.
    pub core_ms: Option<u64>,
    /// Core's outcome.
    pub core_outcome: Option<String>,
    /// The TLC time Core's budget was set from.
    pub budget_tlc_ms: Option<u64>,
    /// Core/TLC.
    pub ratio: Option<f64>,
    /// Core stopped at its budget.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lower_bound: bool,
}

/// One model's result in a part or record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRow {
    /// `small` or `large`.
    pub cls: SizeClass,
    /// The part's label (shard or node).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shard: String,
    /// Its category.
    pub cat: Category,
    /// The ratio (the median for small and banded models).
    pub ratio: Option<f64>,
    /// `ratio` is a lower bound.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lower_bound: bool,
    /// TLC's time (the median of the repetitions for a small model).
    pub tlc_ms: Option<u64>,
    /// Core's time (likewise).
    pub core_ms: Option<u64>,
    /// Small: each repetition's ratio, in repetition order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reps: Vec<Option<f64>>,
    /// Small: the least and greatest repetition ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[f64; 2]>,
    /// Small: each repetition's Core outcome, when not all agreed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rep_outcomes: Vec<String>,
    /// Large, banded: the first pass's ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_pass: Option<f64>,
    /// Large, banded: the re-run pairs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<PairRow>,
    /// Why the model is excluded or untimed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// One repetition of the small class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Repetition {
    /// From 1.
    pub index: usize,
    /// Which checker went first.
    pub order: [Side; 2],
    /// The TLC pass.
    pub tlc: Pass,
    /// The Core pass.
    pub core: Pass,
}

/// How a large part got its first-pass TLC times.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheUse {
    /// The cache: the part file's label and its TLC pass run id.
    pub source: String,
    /// The canaries' verdict.
    pub canaries: CanaryVerdict,
    /// Whether the cache was used (else the whole first pass was re-taken).
    pub used: bool,
}

/// The output of `r2 small` or `r2 large`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    /// [`R2_SCHEMA`].
    pub schema_version: u32,
    /// `r2-part`.
    pub kind: String,
    /// The class timed.
    pub class: SizeClass,
    /// Shard or node label.
    pub label: String,
    /// Start and end (RFC 3339).
    pub started_at: String,
    /// End.
    pub finished_at: String,
    /// The controls.
    pub controls: Controls,
    /// The Core binary.
    pub dive: DiveProvenance,
    /// SHA-256 of the harness binary.
    pub harness_sha256: Option<String>,
    /// The classification pass's run id.
    pub classes_run_id: String,
    /// The harness-overhead probe.
    pub overhead: OverheadProbe,
    /// What the page-cache warm-up read.
    pub warmed: Warmed,
    /// Core's budget factor in band re-runs (large).
    pub budget_factor: f64,
    /// Small: the repetitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repetitions: Vec<Repetition>,
    /// Large: the first pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub first_pass: Vec<Pass>,
    /// Large: the first pass's TLC times (a cache for a later launch).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tlc_first_ms: BTreeMap<String, u64>,
    /// Large: the cached TLC times' provenance, if a cache was offered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tlc_cache: Option<CacheUse>,
    /// Per model.
    pub models: BTreeMap<String, ModelRow>,
    /// Why this part is triage.
    pub triage: Vec<String>,
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path, kind: &str) -> Result<T> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| Error::Config(format!("{}: not JSON: {e}", path.display())))?;
    if v["kind"] != kind || v["schema_version"] != R2_SCHEMA {
        return Err(Error::Config(format!(
            "{}: not an `{kind}` file of schema {R2_SCHEMA}",
            path.display()
        )));
    }
    serde_json::from_value(v).map_err(|e| Error::Config(format!("{}: {e}", path.display())))
}

impl Classes {
    /// Reads `r2 classify`'s output.
    ///
    /// # Errors
    /// Fails if the file is not a classes file.
    pub fn load(path: &Path) -> Result<Self> {
        load_json(path, "r2-classes")
    }
}

impl Part {
    /// The label, or the class when there is none.
    #[must_use]
    pub fn name(&self) -> &str {
        if self.label.is_empty() {
            self.class.as_str()
        } else {
            &self.label
        }
    }

    /// Reads `r2 small` or `r2 large`'s output.
    ///
    /// # Errors
    /// Fails if the file is not a part.
    pub fn load(path: &Path) -> Result<Self> {
        load_json(path, "r2-part")
    }
}

/// A prepared timed session: the oracle, Core, the controls, a warm page
/// cache and the overhead probe.
struct Session<'a> {
    config: &'a Config,
    cache: &'a Cache,
    opts: &'a Options,
    tlc: TlcImplementation,
    dive: Option<DiveImplementation>,
    controls: Controls,
    overhead: OverheadProbe,
    warmed: Warmed,
}

impl<'a> Session<'a> {
    fn prepare(
        config: &'a Config,
        cache: &'a Cache,
        opts: &'a Options,
        core: bool,
    ) -> Result<Self> {
        if !cfg!(any(target_os = "linux", target_os = "macos")) {
            return Err(Error::Command(
                "R2 runs need a Linux or macOS host".to_owned(),
            ));
        }
        let java = Java::resolve(config, cache, None)?;
        let JavaSource::Pinned {
            sha256: jre_sha256, ..
        } = &java.source
        else {
            return Err(Error::Usage("R2 runs use the pinned JRE only".to_owned()));
        };
        let jre_sha256 = jre_sha256.clone();
        let env = OracleEnv::prepare(config, cache, &opts.oracle, &java)?;
        let tlc_command = env.tlc_command_template(&opts.xmx);
        if let Some(flag) = host::non_stock_cds_flag(&tlc_command) {
            return Err(Error::Config(format!(
                "the TLC command line selects a non-stock CDS archive ({flag})"
            )));
        }
        let jre = config.oracles.host_jre()?;
        let archive = cache.verified_artifact("JRE archive", &jre.url, &jre.sha256)?;
        let jre_dir = cache.jre_dir(&jre.sha256);
        std::fs::create_dir_all(cache.tmp_dir()).map_err(|e| Error::io(&cache.tmp_dir(), &e))?;
        let scratch = crate::util::TempDir::new_in(&cache.tmp_dir(), "r2")?;
        let cds = host::check_cds(&archive, &jre_dir, scratch.path())?;
        let corpus = pipeline::load_corpus(config, cache, &opts.corpus)?;
        let dive = if core {
            let mut d = DiveImplementation::prepare(config, cache, &opts.oracle, &opts.dive_bin)?;
            d.engine = DiveEngine::Core;
            d.profile = crate::divergence::Profile::Dive;
            Some(d)
        } else {
            None
        };
        let mut inputs = vec![corpus.root.clone(), env.jar.clone(), jre_dir];
        inputs.extend(env.classpath.iter().map(|c| c.path.clone()));
        inputs.extend(env.libraries.iter().map(|c| c.path.clone()));
        if core {
            inputs.push(opts.dive_bin.clone());
        }
        let warmed = host::warm(&inputs, cache.root())?;
        let overhead = host::overhead_probe(scratch.path())?;
        let controls = Controls {
            host: HostFacts::probe(cache.root()),
            binding: opts.binding.clone(),
            environment: opts.environment.clone(),
            jobs: 1,
            xmx: opts.xmx.clone(),
            budget_cap_s: opts.budget_cap.as_secs(),
            oracle: env.oracle.id.clone(),
            oracle_sha256: env.oracle.sha256.clone(),
            jre: java.version_line.clone(),
            jre_sha256,
            cds,
            corpus: corpus.id.clone(),
            corpus_commit: corpus.commit.clone(),
            tlc_command,
            harness: pipeline::generator(),
        };
        Ok(Self {
            config,
            cache,
            opts,
            tlc: TlcImplementation { env },
            dive,
            controls,
            overhead,
            warmed,
        })
    }

    /// Runs one pass of `side` over `models` in a fresh report directory.
    fn pass(
        &self,
        side: Side,
        tag: &str,
        models: &[String],
        timings: Option<(&str, BTreeMap<String, u64>, f64)>,
        fixed_timeout: Option<Duration>,
    ) -> Result<Pass> {
        let report_dir = self.opts.out.join("passes").join(tag);
        std::fs::create_dir_all(&report_dir).map_err(|e| Error::io(&report_dir, &e))?;
        let budgets_from = timings.as_ref().map(|(id, _, f)| format!("{id} at {f}x"));
        let tlc_timings = timings.map(|(id, times, factor)| {
            crate::r2::TlcTimings::new(
                id,
                crate::report::Host::current(),
                times
                    .into_iter()
                    .map(|(m, ms)| ((self.opts.corpus.clone(), m), ms)),
            )
            .with_factor(factor)
        });
        let opts = RunOptions {
            corpora: vec![self.opts.corpus.clone()],
            sany_phase: crate::sany::SanyPhase::Full,
            golden_oracle: self.opts.oracle.clone(),
            selection: Selection {
                tier: "all".to_owned(),
                filters: models.to_vec(),
            },
            workers: vec![1],
            limits: Limits {
                jobs: 1,
                xmx: self.opts.xmx.clone(),
                timeout: fixed_timeout,
                timeout_cap: self.opts.budget_cap,
                budget_cap: (side == Side::Core).then_some(self.opts.budget_cap),
            },
            crosscheck_manifest: false,
            observe_missing_goldens: false,
            models: true,
            sany: false,
            report_dir,
            summary_path: None,
            tlc_timings,
        };
        let started_at = rfc3339(unix_now());
        let report = match side {
            Side::Tlc => pipeline::run(
                self.config,
                self.cache,
                &self.tlc,
                Some(&self.tlc.env),
                &opts,
            )?,
            Side::Core => {
                let dive = self
                    .dive
                    .as_ref()
                    .ok_or_else(|| Error::Config("no Core binary prepared".to_owned()))?;
                pipeline::run(self.config, self.cache, dive, None, &opts)?
            }
        };
        let mut runs = BTreeMap::new();
        for m in &report.models {
            if !models.contains(&m.model) {
                continue;
            }
            let (outcome, elapsed_ms, detail) = match m.runs.iter().find(|r| r.workers == 1) {
                Some(r) => (
                    r.outcome,
                    r.elapsed_ms,
                    r.detail.clone().or_else(|| m.detail.clone()),
                ),
                None => (m.outcome, 0, m.detail.clone()),
            };
            runs.insert(
                m.model.clone(),
                Timed {
                    outcome: outcome.as_str().to_owned(),
                    elapsed_ms,
                    detail: (outcome != Outcome::Agree)
                        .then(|| detail.map(|d| d.chars().take(200).collect()))
                        .flatten(),
                },
            );
        }
        Ok(Pass {
            tag: tag.to_owned(),
            side,
            run_id: report.run_id,
            started_at,
            budgets_from,
            runs,
        })
    }

    fn part(&self, class: SizeClass, classes: &Classes, started_at: String) -> Result<Part> {
        let dive = self
            .dive
            .as_ref()
            .and_then(crate::implementation::Implementation::dive_provenance)
            .ok_or_else(|| Error::Config("no Core binary prepared".to_owned()))?;
        let mut triage = self.controls.triage();
        let stale = self.controls.mismatches(&classes.controls, true);
        if !stale.is_empty() {
            return Err(Error::Config(format!(
                "the classes come from a different host type or checker: {}",
                stale.join("; ")
            )));
        }
        triage.sort();
        Ok(Part {
            schema_version: R2_SCHEMA,
            kind: "r2-part".to_owned(),
            class,
            label: self.opts.label.clone(),
            started_at,
            finished_at: String::new(),
            controls: self.controls.clone(),
            dive,
            harness_sha256: std::env::current_exe()
                .ok()
                .and_then(|p| crate::hash::sha256_file(&p).ok()),
            classes_run_id: classes.pass.run_id.clone(),
            overhead: self.overhead.clone(),
            warmed: self.warmed.clone(),
            budget_factor: f64::from(crate::r2::FACTOR),
            repetitions: Vec::new(),
            first_pass: Vec::new(),
            tlc_first_ms: BTreeMap::new(),
            tlc_cache: None,
            models: BTreeMap::new(),
            triage,
        })
    }

    fn save<T: Serialize>(&self, name: &str, value: &T) -> Result<PathBuf> {
        let path = self.opts.out.join(name);
        write_atomic(&path, to_pretty(value)?.as_bytes())?;
        Ok(path)
    }
}

fn selected(config: &Config, cache: &Cache, opts: &Options) -> Result<Vec<String>> {
    let corpus = pipeline::load_corpus(config, cache, &opts.corpus)?;
    Ok(opts
        .selection
        .models(&corpus)?
        .into_iter()
        .map(|m| m.id.clone())
        .collect())
}

/// `r2 classify`: one `--jobs 1` TLC pass with a fixed `timeout`; a model
/// TLC completes in under 10 s is small, one it completes later or the
/// timeout stops is large, and any other outcome has no class. Writes
/// `<out>/r2-classes.json`.
///
/// # Errors
/// Fails on harness errors or an empty selection.
pub fn classify(
    config: &Config,
    cache: &Cache,
    opts: &Options,
    timeout: Duration,
) -> Result<Classes> {
    let models = selected(config, cache, opts)?;
    let s = Session::prepare(config, cache, opts, false)?;
    let pass = s.pass(Side::Tlc, "classify", &models, None, Some(timeout))?;
    let mut rows = BTreeMap::new();
    let mut not_timed = BTreeMap::new();
    for m in &models {
        match pass.get(m) {
            Some(t) if t.tlc_ms().is_some() => {
                rows.insert(
                    m.clone(),
                    ClassRow {
                        class: stats::size_class(t.tlc_ms()),
                        tlc_ms: t.tlc_ms(),
                    },
                );
            }
            Some(t) if t.outcome == "timeout" => {
                rows.insert(
                    m.clone(),
                    ClassRow {
                        class: SizeClass::Large,
                        tlc_ms: None,
                    },
                );
            }
            Some(t) => {
                not_timed.insert(
                    m.clone(),
                    format!(
                        "TLC {}{}",
                        t.outcome,
                        t.detail
                            .as_deref()
                            .map(|d| format!(": {d}"))
                            .unwrap_or_default()
                    ),
                );
            }
            None => {
                not_timed.insert(m.clone(), "not run".to_owned());
            }
        }
    }
    let classes = Classes {
        schema_version: R2_SCHEMA,
        kind: "r2-classes".to_owned(),
        controls: s.controls.clone(),
        pass,
        timeout_s: timeout.as_secs(),
        small_below_ms: stats::SMALL_BELOW_MS,
        models: rows,
        not_timed,
    };
    s.save("r2-classes.json", &classes)?;
    Ok(classes)
}

fn side_name(order: [Side; 2]) -> String {
    order
        .iter()
        .map(|s| match s {
            Side::Tlc => "tlc",
            Side::Core => "core",
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// `r2 small`: `reps` repetitions (six for the gate) of the small class,
/// each timing TLC and Core over it, the order reversed in alternate
/// repetitions. A Core-first repetition takes its budgets from the previous
/// repetition's TLC times. Writes `<out>/r2-small-<label>.json` after every
/// repetition.
///
/// # Errors
/// Fails on harness errors or an empty class.
pub fn small(
    config: &Config,
    cache: &Cache,
    opts: &Options,
    classes: &Classes,
    reps: usize,
) -> Result<Part> {
    let models = classes.of(SizeClass::Small, &opts.selection);
    if models.is_empty() {
        return Err(Error::Usage("no small-class model is selected".to_owned()));
    }
    if reps == 0 {
        return Err(Error::Usage("--reps must be positive".to_owned()));
    }
    let started_at = rfc3339(unix_now());
    let s = Session::prepare(config, cache, opts, true)?;
    let mut part = s.part(SizeClass::Small, classes, started_at)?;
    if reps != stats::SMALL_REPETITIONS {
        part.triage.push(format!(
            "{reps} small-class repetitions, not {}",
            stats::SMALL_REPETITIONS
        ));
    }
    let name = opts.file_name("small");
    let mut last_tlc: Option<Pass> = None;
    for i in 0..reps {
        let order = stats::order(i);
        let (mut tlc, mut core) = (None, None);
        for side in order {
            match side {
                Side::Tlc => {
                    tlc = Some(s.pass(
                        Side::Tlc,
                        &format!("rep{}-tlc", i + 1),
                        &models,
                        None,
                        None,
                    )?);
                }
                Side::Core => {
                    let from = tlc.as_ref().or(last_tlc.as_ref());
                    let timings = from.map(|p| {
                        (
                            p.run_id.as_str(),
                            p.tlc_times(),
                            f64::from(crate::r2::FACTOR),
                        )
                    });
                    core = Some(s.pass(
                        Side::Core,
                        &format!("rep{}-core", i + 1),
                        &models,
                        timings,
                        None,
                    )?);
                }
            }
        }
        let (Some(tlc), Some(core)) = (tlc, core) else {
            return Err(Error::Config("a repetition lost a pass".to_owned()));
        };
        last_tlc = Some(tlc.clone());
        part.repetitions.push(Repetition {
            index: i + 1,
            order,
            tlc,
            core,
        });
        part.models = small_rows(&models, &part.repetitions, &opts.label);
        part.finished_at = rfc3339(unix_now());
        s.save(&name, &part)?;
    }
    Ok(part)
}

/// The rows of the small class from its repetitions.
#[must_use]
pub fn small_rows(
    models: &[String],
    reps: &[Repetition],
    label: &str,
) -> BTreeMap<String, ModelRow> {
    let mut rows = BTreeMap::new();
    for m in models {
        let cells: Vec<(Option<&Timed>, Option<&Timed>)> =
            reps.iter().map(|r| (r.tlc.get(m), r.core.get(m))).collect();
        let tlc_ms: Vec<Option<u64>> = cells
            .iter()
            .map(|(t, _)| t.and_then(Timed::tlc_ms))
            .collect();
        let ratios: Vec<Option<Ratio>> = cells
            .iter()
            .zip(&tlc_ms)
            .map(|((_, c), t)| c.and_then(|c| c.ratio(*t)))
            .collect();
        let outcomes: Vec<String> = cells
            .iter()
            .map(|(_, c)| c.map_or_else(|| "not-run".to_owned(), |c| c.outcome.clone()))
            .collect();
        let med = |v: Vec<u64>| {
            #[allow(
                clippy::cast_precision_loss,
                reason = "millisecond wall times are far below 2^52"
            )]
            let f: Vec<f64> = v.iter().map(|&x| x as f64).collect();
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a median of non-negative milliseconds"
            )]
            stats::median(&f).map(|x| x.round() as u64)
        };
        let (cat, ratio, detail) = if tlc_ms.iter().any(Option::is_none) {
            let bad = cells
                .iter()
                .find_map(|(t, _)| {
                    t.filter(|t| t.tlc_ms().is_none())
                        .map(|t| t.outcome.clone())
                })
                .unwrap_or_else(|| "not-run".to_owned());
            (
                Category::Untimed,
                None,
                Some(format!("TLC {bad} in a repetition")),
            )
        } else if ratios.iter().all(Option::is_some) {
            let r = stats::median_bound(&ratios.iter().flatten().copied().collect::<Vec<_>>());
            (Category::of(r), r, None)
        } else {
            let first_bad = cells.iter().find_map(|(_, c)| {
                c.filter(|c| c.ratio(Some(1)).is_none())
                    .and_then(|c| c.detail.clone())
            });
            (Category::Excluded, None, first_bad)
        };
        let all_agree = outcomes
            .iter()
            .all(|o| o == "agree" || o == "expected-divergence");
        rows.insert(
            m.clone(),
            ModelRow {
                cls: SizeClass::Small,
                shard: label.to_owned(),
                cat,
                ratio: ratio.map(|r| r4(r.value)),
                lower_bound: ratio.is_some_and(|r| r.lower_bound),
                tlc_ms: med(tlc_ms.iter().flatten().copied().collect()),
                core_ms: med(cells
                    .iter()
                    .filter_map(|(_, c)| c.map(|c| c.elapsed_ms))
                    .collect()),
                reps: ratios.iter().map(|r| r.map(|r| r4(r.value))).collect(),
                range: {
                    let v: Vec<f64> = ratios.iter().flatten().map(|r| r4(r.value)).collect();
                    v.iter()
                        .copied()
                        .reduce(f64::min)
                        .zip(v.iter().copied().reduce(f64::max))
                        .map(|(a, b)| [a, b])
                },
                rep_outcomes: if all_agree { Vec::new() } else { outcomes },
                first_pass: None,
                pairs: Vec::new(),
                detail,
            },
        );
    }
    rows
}

/// `r2 canary`: re-times the three shortest large models of `cached` three
/// times each, after checking its controls against this host's.
///
/// # Errors
/// Fails if the controls differ, or on harness errors.
pub fn canary(
    config: &Config,
    cache: &Cache,
    opts: &Options,
    cached: &Part,
) -> Result<CanaryVerdict> {
    let s = Session::prepare(config, cache, opts, false)?;
    let v = run_canaries(&s, cached)?;
    s.save(&opts.file_name("canary"), &v)?;
    Ok(v)
}

fn check_cache(s: &Session<'_>, cached: &Part) -> Result<()> {
    if cached.class != SizeClass::Large || cached.tlc_first_ms.is_empty() {
        return Err(Error::Usage(
            "a TLC cache is a large-class part with first-pass TLC times".to_owned(),
        ));
    }
    let diff = s.controls.mismatches(&cached.controls, false);
    if diff.is_empty() {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "the cached TLC times were taken under different controls: {}",
            diff.join("; ")
        )))
    }
}

fn run_canaries(s: &Session<'_>, cached: &Part) -> Result<CanaryVerdict> {
    check_cache(s, cached)?;
    let mut shortest: Vec<(&String, &u64)> = cached.tlc_first_ms.iter().collect();
    shortest.sort_by_key(|(m, ms)| (**ms, (*m).clone()));
    let mut runs = Vec::new();
    for (i, (m, &ms)) in shortest.into_iter().take(stats::CANARIES).enumerate() {
        let mut fresh = Vec::new();
        for k in 0..stats::CANARY_RUNS {
            let p = s.pass(
                Side::Tlc,
                &format!("canary{}-{}", i + 1, k + 1),
                std::slice::from_ref(m),
                None,
                None,
            )?;
            fresh.extend(p.get(m).and_then(Timed::tlc_ms));
        }
        runs.push((s.opts.corpus.clone(), m.clone(), ms, fresh));
    }
    Ok(stats::canary_verdict(runs))
}

/// `r2 large`: one pass of the large class (TLC from `cached` when its
/// controls match and the canaries pass, otherwise fresh), then three fresh
/// alternating pairs, Core's budget at `factor` x TLC, for every model whose
/// first ratio is in a band or a budget stop. A TLC-first pair takes Core's
/// budget from that pair's TLC run; a Core-first pair from the model's most
/// recent TLC time. Writes `<out>/r2-large-<label>.json`.
///
/// # Errors
/// Fails on harness errors, an empty class or a cache with other controls.
#[allow(
    clippy::too_many_lines,
    reason = "one linear procedure: cache, first pass, then band pairs per model"
)]
pub fn large(
    config: &Config,
    cache: &Cache,
    opts: &Options,
    classes: &Classes,
    factor: f64,
    cached: Option<&Part>,
) -> Result<Part> {
    let mut models = classes.of(SizeClass::Large, &opts.selection);
    if let Some((k, n)) = opts.shard {
        let root = crate::golden::golden_root(&config.repo_root);
        let est = models
            .iter()
            .map(|m| {
                Ok((
                    m.clone(),
                    golden_elapsed_ms(&root, &opts.oracle, &opts.corpus, m)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        models = stats::lpt_shards(&est, n).swap_remove(k - 1);
    }
    if models.is_empty() {
        return Err(Error::Usage("no large-class model is selected".to_owned()));
    }
    let started_at = rfc3339(unix_now());
    let s = Session::prepare(config, cache, opts, true)?;
    let mut part = s.part(SizeClass::Large, classes, started_at)?;
    part.budget_factor = factor;
    #[allow(clippy::float_cmp, reason = "BANDED_FACTOR is an exact literal")]
    if factor != crate::r2::BANDED_FACTOR {
        part.triage.push(format!(
            "band re-run budget factor {factor}, not {}",
            crate::r2::BANDED_FACTOR
        ));
    }
    let name = opts.file_name("large");
    let cached_times = match cached {
        Some(c) => {
            let canaries = run_canaries(&s, c)?;
            let used = canaries.valid;
            let source = format!(
                "{} ({})",
                c.label,
                c.first_pass.first().map_or("", |p| p.run_id.as_str())
            );
            part.tlc_cache = Some(CacheUse {
                source,
                canaries,
                used,
            });
            used.then(|| c.tlc_first_ms.clone())
        }
        None => None,
    };
    let (tlc_from, tlc_times) = if let Some(times) = cached_times {
        let missing: Vec<String> = models
            .iter()
            .filter(|m| !times.contains_key(*m))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(Error::Config(format!(
                "the TLC cache has no time for {}",
                missing.join(", ")
            )));
        }
        let source = part
            .tlc_cache
            .as_ref()
            .map_or_else(String::new, |c| c.source.clone());
        (format!("cache {source}"), times)
    } else {
        let tlc = s.pass(Side::Tlc, "first-tlc", &models, None, None)?;
        let times = tlc.tlc_times();
        let id = tlc.run_id.clone();
        part.first_pass.push(tlc);
        (id, times)
    };
    part.tlc_first_ms.clone_from(&tlc_times);
    let timed: Vec<String> = models
        .iter()
        .filter(|m| tlc_times.contains_key(*m))
        .cloned()
        .collect();
    let core = if timed.is_empty() {
        None
    } else {
        Some(s.pass(
            Side::Core,
            "first-core",
            &timed,
            Some((
                tlc_from.as_str(),
                tlc_times.clone(),
                f64::from(crate::r2::FACTOR),
            )),
            None,
        )?)
    };
    if let Some(c) = &core {
        part.first_pass.push(c.clone());
    }
    for m in &models {
        let tlc_ms = tlc_times.get(m).copied();
        let core_run = core.as_ref().and_then(|c| c.get(m));
        let first = core_run.and_then(|c| c.ratio(tlc_ms));
        let mut row = ModelRow {
            cls: SizeClass::Large,
            shard: opts.label.clone(),
            cat: if tlc_ms.is_none() {
                Category::Untimed
            } else {
                Category::of(first)
            },
            ratio: first.map(|r| r4(r.value)),
            lower_bound: first.is_some_and(|r| r.lower_bound),
            tlc_ms,
            core_ms: core_run.map(|c| c.elapsed_ms),
            reps: Vec::new(),
            range: None,
            rep_outcomes: Vec::new(),
            first_pass: None,
            pairs: Vec::new(),
            detail: if tlc_ms.is_none() {
                part.first_pass
                    .first()
                    .and_then(|p| p.get(m))
                    .map(|t| format!("TLC {}", t.outcome))
                    .or_else(|| Some("no TLC time".to_owned()))
            } else if first.is_none() {
                core_run.and_then(|c| c.detail.clone().or_else(|| Some(c.outcome.clone())))
            } else {
                None
            },
        };
        if let Some(r) = first
            && stats::needs_pairs(r)
        {
            let pairs = band_pairs(&s, m, tlc_ms, factor)?;
            row.first_pass = row.ratio;
            let ratios: Vec<Option<Ratio>> = pairs
                .iter()
                .map(|p| {
                    p.ratio.map(|v| Ratio {
                        value: v,
                        lower_bound: p.lower_bound,
                    })
                })
                .collect();
            if ratios.iter().all(Option::is_some) {
                let med =
                    stats::median_bound(&ratios.iter().flatten().copied().collect::<Vec<_>>());
                row.cat = Category::of(med);
                row.ratio = med.map(|r| r4(r.value));
                row.lower_bound = med.is_some_and(|r| r.lower_bound);
            } else {
                row.cat = Category::Excluded;
                row.ratio = None;
                row.lower_bound = false;
                row.detail = Some("a band re-run pair has no ratio".to_owned());
            }
            row.pairs = pairs;
        }
        part.models.insert(m.clone(), row);
        part.finished_at = rfc3339(unix_now());
        s.save(&name, &part)?;
    }
    part.finished_at = rfc3339(unix_now());
    s.save(&name, &part)?;
    Ok(part)
}

fn golden_elapsed_ms(root: &Path, oracle: &str, corpus: &str, model: &str) -> Result<u64> {
    let path = crate::golden::model_golden_path(root, oracle, corpus, model);
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, &e))?;
    let g: crate::golden::ModelGolden = serde_json::from_str(&text)
        .map_err(|e| Error::Golden(format!("{}: {e}", path.display())))?;
    Ok(g.provenance.elapsed_ms)
}

fn band_pairs(
    s: &Session<'_>,
    m: &str,
    first_tlc: Option<u64>,
    factor: f64,
) -> Result<Vec<PairRow>> {
    let safe: String = m
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let one = [m.to_owned()];
    let mut latest = first_tlc;
    let mut out = Vec::new();
    for k in 0..stats::BAND_PAIRS {
        let order = stats::order(k);
        let (mut tlc, mut core, mut budget) = (None, None, None);
        for side in order {
            match side {
                Side::Tlc => {
                    let p = s.pass(
                        Side::Tlc,
                        &format!("pair-{safe}-{}-tlc", k + 1),
                        &one,
                        None,
                        None,
                    )?;
                    tlc = p.get(m).cloned();
                    if let Some(ms) = tlc.as_ref().and_then(Timed::tlc_ms) {
                        latest = Some(ms);
                    }
                }
                Side::Core => {
                    budget = latest;
                    let times = latest
                        .map(|ms| BTreeMap::from([(m.to_owned(), ms)]))
                        .unwrap_or_default();
                    let p = s.pass(
                        Side::Core,
                        &format!("pair-{safe}-{}-core", k + 1),
                        &one,
                        Some(("pair", times, factor)),
                        None,
                    )?;
                    core = p.get(m).cloned();
                }
            }
        }
        let tlc_ms = tlc.as_ref().and_then(Timed::tlc_ms);
        let ratio = core.as_ref().and_then(|c| c.ratio(tlc_ms));
        out.push(PairRow {
            order: side_name(order),
            tlc_ms,
            core_ms: core.as_ref().map(|c| c.elapsed_ms),
            core_outcome: core.map(|c| c.outcome),
            budget_tlc_ms: budget,
            ratio: ratio.map(|r| r4(r.value)),
            lower_bound: ratio.is_some_and(|r| r.lower_bound),
        });
    }
    Ok(out)
}

/// The R2 record's summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Agreeing models.
    pub agreeing: usize,
    /// Of which small.
    pub small: usize,
    /// Of which large.
    pub large: usize,
    /// Geometric mean over the agreeing models.
    pub geomean: Option<f64>,
    /// Geometric mean over every model with a TLC time: stops at max(2,
    /// lower bound), excluded rows at 2.
    pub fixed_set_geomean: Option<f64>,
    /// Rows in the fixed set.
    pub fixed_set_rows: usize,
    /// Agreeing models above 1x.
    pub above_1x: BTreeMap<String, f64>,
    /// Agreeing models above 2x, and stops at or above 2x.
    pub above_2x: Vec<String>,
    /// Stopped models.
    pub stopped: Vec<String>,
    /// Excluded models.
    pub excluded: Vec<String>,
    /// Median, minimum and maximum over the agreeing models.
    pub median: Option<f64>,
    /// Minimum.
    pub min: Option<f64>,
    /// Maximum.
    pub max: Option<f64>,
    /// `pass`, `fail`, or `triage` when the run broke a procedure rule.
    pub gate_result: String,
}

/// One part's place in the record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartInfo {
    /// Label.
    pub label: String,
    /// Class.
    pub class: SizeClass,
    /// Start.
    pub started_at: String,
    /// End.
    pub finished_at: String,
    /// The host's facts.
    pub host: HostFacts,
    /// The binding.
    pub binding: String,
    /// The overhead probe's median, milliseconds.
    pub overhead_ms: f64,
    /// Bytes the page-cache warm-up read.
    pub warmed_bytes: u64,
    /// Band re-run budget factor.
    pub budget_factor: f64,
    /// Every pass, in order, with its start time.
    pub passes: Vec<PassInfo>,
    /// Cached TLC times' provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tlc_cache: Option<CacheUse>,
}

/// One pass in the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassInfo {
    /// Its tag (`rep3-core`, `first-tlc`, ...).
    pub tag: String,
    /// Its harness run id.
    pub run_id: String,
    /// Start.
    pub started_at: String,
}

/// The R2 record (`conformance/reports/perf/<date>-r2-procedure-<src>.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// `R2`.
    pub gate: String,
    /// The procedure.
    pub procedure: String,
    /// The harness that built the record.
    pub generator: String,
    /// When.
    pub generated_at: String,
    /// `dive --version` of the Core binary.
    pub source: String,
    /// Its SHA-256.
    pub dive_binary_sha256: String,
    /// The harness binaries' SHA-256.
    pub harness_binary_sha256: Vec<String>,
    /// Oracle id and SHA-256.
    pub oracle: String,
    /// JRE and its CDS archives.
    pub jre: String,
    /// Corpus `id@commit`.
    pub corpus: String,
    /// The controls common to every part (the first part's host facts).
    pub controls: Controls,
    /// The classification.
    pub classification: String,
    /// Each part.
    pub parts: Vec<PartInfo>,
    /// Whether every procedure rule held.
    pub conforming: bool,
    /// Why not.
    pub triage: Vec<String>,
    /// The summary.
    pub summary: Summary,
    /// Classified models no part timed, and unclassified ones, with reasons.
    pub not_timed: BTreeMap<String, String>,
    /// Free-form notes (`--note`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Every model.
    pub models: BTreeMap<String, ModelRow>,
}

fn passes(p: &Part) -> Vec<PassInfo> {
    let info = |x: &Pass| PassInfo {
        tag: x.tag.clone(),
        run_id: x.run_id.clone(),
        started_at: x.started_at.clone(),
    };
    let mut out: Vec<PassInfo> = Vec::new();
    for r in &p.repetitions {
        let (a, b) = match r.order[0] {
            Side::Tlc => (&r.tlc, &r.core),
            Side::Core => (&r.core, &r.tlc),
        };
        out.push(info(a));
        out.push(info(b));
    }
    out.extend(p.first_pass.iter().map(info));
    out
}

/// `r2 record`: merges the parts of one classification into the record.
///
/// # Errors
/// Fails if the parts disagree on the binary, oracle, JRE, corpus, TLC
/// command line or classification, or a model is in two parts.
#[allow(clippy::too_many_lines, reason = "checks, then one record literal")]
pub fn record(classes: &Classes, parts: &[Part], notes: Vec<String>) -> Result<Record> {
    let Some(first) = parts.first() else {
        return Err(Error::Usage("r2 record needs at least one part".to_owned()));
    };
    let mut triage = Vec::new();
    let mut models: BTreeMap<String, ModelRow> = BTreeMap::new();
    for p in parts {
        if p.classes_run_id != classes.pass.run_id {
            return Err(Error::Config(format!(
                "part `{}` was timed against classification {}, not {}",
                p.name(),
                p.classes_run_id,
                classes.pass.run_id
            )));
        }
        if p.dive.sha256 != first.dive.sha256 {
            return Err(Error::Config(format!(
                "part `{}` timed another Core binary",
                p.name()
            )));
        }
        let diff = p.controls.mismatches(&first.controls, true);
        if !diff.is_empty() {
            return Err(Error::Config(format!(
                "part `{}` ran under other controls: {}",
                p.name(),
                diff.join("; ")
            )));
        }
        for t in &p.triage {
            triage.push(format!("{}: {t}", p.name()));
        }
        for (m, row) in &p.models {
            if models.insert(m.clone(), row.clone()).is_some() {
                return Err(Error::Config(format!("{m} is in two parts")));
            }
        }
    }
    let mut not_timed = classes.not_timed.clone();
    for m in classes.models.keys() {
        if !models.contains_key(m) {
            not_timed.insert(m.clone(), "classified but not timed by any part".to_owned());
        }
    }
    let missing = classes
        .models
        .keys()
        .filter(|m| !models.contains_key(*m))
        .count();
    if missing > 0 {
        triage.push(format!(
            "{missing} classified model(s) not timed by any part"
        ));
    }
    let summary = summarise(&models, triage.is_empty());
    let harness_binary_sha256 = {
        let mut v: Vec<String> = parts
            .iter()
            .filter_map(|p| p.harness_sha256.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let c = &first.controls;
    let small = classes
        .models
        .values()
        .filter(|r| r.class == SizeClass::Small)
        .count();
    Ok(Record {
        gate: "R2".to_owned(),
        procedure: "docs/developer/performance.md#r2-measurement-procedure".to_owned(),
        generator: pipeline::generator(),
        generated_at: rfc3339(unix_now()),
        source: first.dive.version.clone(),
        dive_binary_sha256: first.dive.sha256.clone(),
        harness_binary_sha256,
        oracle: format!("{} (SHA-256 {})", c.oracle, c.oracle_sha256),
        jre: format!(
            "{} (archive SHA-256 {}; {} stock CDS archive(s) verified)",
            c.jre,
            c.jre_sha256,
            c.cds.len()
        ),
        corpus: format!("{}@{}", c.corpus, c.corpus_commit),
        controls: c.clone(),
        classification: format!(
            "TLC pass {} at --jobs 1 before any Core run, timeout {} s: {} small (< {} ms), {} large, {} without a class",
            classes.pass.run_id,
            classes.timeout_s,
            small,
            classes.small_below_ms,
            classes.models.len() - small,
            classes.not_timed.len()
        ),
        parts: parts
            .iter()
            .map(|p| PartInfo {
                label: p.label.clone(),
                class: p.class,
                started_at: p.started_at.clone(),
                finished_at: p.finished_at.clone(),
                host: p.controls.host.clone(),
                binding: p.controls.binding.description.clone(),
                overhead_ms: p.overhead.median_ms,
                warmed_bytes: p.warmed.bytes,
                budget_factor: p.budget_factor,
                passes: passes(p),
                tlc_cache: p.tlc_cache.clone(),
            })
            .collect(),
        conforming: triage.is_empty(),
        triage,
        summary,
        not_timed,
        notes,
        models,
    })
}

/// The summary over a record's rows. `conforming` decides whether the gate
/// result can be `pass` or `fail` at all.
#[must_use]
pub fn summarise(models: &BTreeMap<String, ModelRow>, conforming: bool) -> Summary {
    let agree: BTreeMap<&String, f64> = models
        .iter()
        .filter(|(_, r)| r.cat == Category::Agreeing)
        .filter_map(|(m, r)| Some((m, r.ratio?)))
        .collect();
    let values: Vec<f64> = agree.values().copied().collect();
    let fixed: Vec<f64> = models
        .values()
        .filter(|r| r.cat != Category::Untimed)
        .map(|r| {
            stats::fixed_set_value(match r.cat {
                Category::Agreeing => r.ratio.map(Ratio::exact),
                Category::Stopped => r.ratio.map(Ratio::at_least),
                _ => None,
            })
        })
        .collect();
    let names = |cat: Category| {
        models
            .iter()
            .filter(|(_, r)| r.cat == cat)
            .map(|(m, _)| m.clone())
            .collect::<Vec<_>>()
    };
    let stopped = names(Category::Stopped);
    let mut above_2x: Vec<String> = agree
        .iter()
        .filter(|(_, x)| **x > 2.0)
        .map(|(m, _)| (*m).clone())
        .collect();
    above_2x.extend(
        models
            .iter()
            .filter(|(_, r)| r.cat == Category::Stopped && r.ratio.is_some_and(|x| x >= 2.0))
            .map(|(m, _)| m.clone()),
    );
    above_2x.sort();
    let geomean = stats::geomean(&values).map(r4);
    let gate_result = if !conforming {
        "triage"
    } else if geomean.is_some_and(|g| g < 1.0) && above_2x.is_empty() && stopped.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let count = |cls: SizeClass| {
        models
            .values()
            .filter(|r| r.cls == cls && r.cat == Category::Agreeing)
            .count()
    };
    Summary {
        agreeing: values.len(),
        small: count(SizeClass::Small),
        large: count(SizeClass::Large),
        geomean,
        fixed_set_geomean: stats::geomean(&fixed).map(r4),
        fixed_set_rows: fixed.len(),
        above_1x: agree
            .iter()
            .filter(|(_, x)| **x > 1.0)
            .map(|(m, x)| ((*m).clone(), *x))
            .collect(),
        above_2x,
        stopped,
        excluded: names(Category::Excluded),
        median: stats::median(&values).map(r4),
        min: values.iter().copied().reduce(f64::min),
        max: values.iter().copied().reduce(f64::max),
        gate_result: gate_result.to_owned(),
    }
}

/// Plain-text rendering of a part or record summary for standard output.
#[must_use]
pub fn render(models: &BTreeMap<String, ModelRow>, summary: &Summary, triage: &[String]) -> String {
    let mut s = String::new();
    let opt = |x: Option<f64>| x.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.4}"));
    let _ = writeln!(
        s,
        "R2: {} agreeing ({} small, {} large); geomean {}x; fixed-set geomean {}x over {} rows; gate {}",
        summary.agreeing,
        summary.small,
        summary.large,
        opt(summary.geomean),
        opt(summary.fixed_set_geomean),
        summary.fixed_set_rows,
        summary.gate_result
    );
    for (m, r) in models {
        let _ = writeln!(
            s,
            "  {:<9} {:<5} {}{} {m}",
            format!("{:?}", r.cat).to_lowercase(),
            r.cls.as_str(),
            if r.lower_bound { ">=" } else { "" },
            opt(r.ratio)
        );
    }
    for t in triage {
        let _ = writeln!(s, "  triage: {t}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed(outcome: &str, ms: u64) -> Timed {
        Timed {
            outcome: outcome.to_owned(),
            elapsed_ms: ms,
            detail: None,
        }
    }

    fn pass(side: Side, runs: &[(&str, Timed)]) -> Pass {
        Pass {
            tag: String::new(),
            side,
            run_id: "id".to_owned(),
            started_at: String::new(),
            budgets_from: None,
            runs: runs
                .iter()
                .map(|(m, t)| ((*m).to_owned(), t.clone()))
                .collect(),
        }
    }

    fn controls() -> Controls {
        Controls {
            host: HostFacts {
                instance: Some("c7i.metal-24xl".into()),
                cpu: Some("Xeon".into()),
                arch: "x86_64".into(),
                cpus: 48,
                smt: Some("off".into()),
                turbo: Some("off".into()),
                governor: Some("performance".into()),
                min_khz: Some(2_400_000),
                max_khz: Some(2_400_000),
                ..HostFacts::default()
            },
            binding: Binding {
                applied: true,
                description: "numactl --cpunodebind=0 --membind=0 (48 CPUs)".into(),
            },
            environment: Environment {
                names: vec!["HOME".into(), "PATH".into()],
                extra: Vec::new(),
            },
            jobs: 1,
            xmx: "24g".into(),
            budget_cap_s: 30_000,
            oracle: "tlc".into(),
            oracle_sha256: "aa".into(),
            jre: "openjdk 17".into(),
            jre_sha256: "bb".into(),
            cds: Vec::new(),
            corpus: "examples".into(),
            corpus_commit: "cc".into(),
            tlc_command: vec!["java".into(), "-Xmx24g".into()],
            harness: "dive-conformance 0.1.0".into(),
        }
    }

    #[test]
    fn timed_runs_give_tlc_times_and_ratios() {
        assert_eq!(timed("agree", 100).tlc_ms(), Some(100));
        assert_eq!(timed("known-upstream-change", 100).tlc_ms(), Some(100));
        assert_eq!(timed("timeout", 100).tlc_ms(), None);
        assert_eq!(timed("agree", 50).ratio(Some(100)), Some(Ratio::exact(0.5)));
        assert_eq!(
            timed("expected-divergence", 50).ratio(Some(100)),
            Some(Ratio::exact(0.5))
        );
        assert_eq!(
            timed("timeout", 250).ratio(Some(100)),
            Some(Ratio::at_least(2.5))
        );
        assert_eq!(timed("unsupported", 5).ratio(Some(100)), None);
        assert_eq!(timed("agree", 5).ratio(None), None);
    }

    type Change = Box<dyn Fn(&mut Controls)>;

    #[test]
    fn matching_controls_reuse_a_cache_and_any_difference_refuses_it() {
        let a = controls();
        assert_eq!(a.mismatches(&a, false), Vec::<String>::new());
        assert_eq!(a.triage(), Vec::<String>::new());
        let cases: Vec<(&str, Change)> = vec![
            ("oracle", Box::new(|c| c.oracle_sha256 = "zz".into())),
            ("jre", Box::new(|c| c.jre_sha256 = "zz".into())),
            ("corpus", Box::new(|c| c.corpus_commit = "zz".into())),
            (
                "TLC command line",
                Box::new(|c| c.tlc_command.push("-Dx".into())),
            ),
            ("jobs", Box::new(|c| c.jobs = 4)),
            ("xmx", Box::new(|c| c.xmx = "3g".into())),
            (
                "binding",
                Box::new(|c| c.binding.description = "absent".into()),
            ),
            ("frequency", Box::new(|c| c.host.max_khz = Some(3_500_000))),
            ("turbo", Box::new(|c| c.host.turbo = Some("on".into()))),
            (
                "governor",
                Box::new(|c| c.host.governor = Some("powersave".into())),
            ),
            (
                "storage",
                Box::new(|c| c.host.storage = Some("/dev/xvda".into())),
            ),
            (
                "instance",
                Box::new(|c| c.host.instance = Some("m7i".into())),
            ),
        ];
        for (field, change) in cases {
            let mut b = controls();
            change(&mut b);
            let diff = b.mismatches(&a, false);
            assert_eq!(diff.len(), 1, "{field}: {diff:?}");
            assert!(diff[0].starts_with(field), "{field}: {diff:?}");
        }
        // A classification ignores the frequency, binding and storage.
        let mut b = controls();
        b.host.max_khz = None;
        b.binding.description = "absent".into();
        assert_eq!(b.mismatches(&a, true), Vec::<String>::new());
        b.xmx = "3g".into();
        assert_eq!(b.mismatches(&a, true).len(), 1);
    }

    #[test]
    fn triage_names_every_broken_rule() {
        let mut c = controls();
        c.binding = Binding {
            applied: false,
            description: "absent: numactl is Linux-only (macos)".into(),
        };
        c.environment.extra = vec!["TERM".into()];
        c.host.turbo = None;
        let t = c.triage();
        assert_eq!(t.len(), 3, "{t:?}");
        assert!(t[0].contains("no NUMA binding"));
        assert!(t[1].contains("TERM"));
    }

    fn rep(index: usize, tlc: &[(&str, Timed)], core: &[(&str, Timed)]) -> Repetition {
        Repetition {
            index,
            order: stats::order(index - 1),
            tlc: pass(Side::Tlc, tlc),
            core: pass(Side::Core, core),
        }
    }

    #[test]
    fn small_rows_take_the_median_of_every_repetition() {
        let reps: Vec<Repetition> = (1..=6)
            .map(|i| {
                let i = u64::try_from(i).unwrap_or_default();
                rep(
                    usize::try_from(i).unwrap_or_default(),
                    &[
                        ("A", timed("agree", 1000)),
                        ("B", timed("agree", 1000)),
                        ("C", timed("agree", 1000)),
                    ],
                    &[
                        ("A", timed("agree", 100 * i)),
                        (
                            "B",
                            timed(if i == 3 { "unsupported" } else { "agree" }, 100),
                        ),
                        ("C", timed("agree", 100)),
                    ],
                )
            })
            .collect();
        let mut reps = reps;
        reps[4].tlc.runs.insert("C".into(), timed("timeout", 9000));
        let rows = small_rows(&["A".into(), "B".into(), "C".into()], &reps, "small");
        let a = &rows["A"];
        assert_eq!(a.cat, Category::Agreeing);
        assert_eq!(a.ratio, Some(0.35), "median of 0.1..0.6");
        assert_eq!(a.reps.len(), 6);
        assert_eq!(a.range, Some([0.1, 0.6]));
        assert_eq!(a.rep_outcomes, Vec::<String>::new());
        assert_eq!(a.tlc_ms, Some(1000));
        let b = &rows["B"];
        assert_eq!(b.cat, Category::Excluded);
        assert_eq!(b.ratio, None);
        assert_eq!(b.rep_outcomes[2], "unsupported");
        assert_eq!(rows["C"].cat, Category::Untimed);
    }

    fn row(cls: SizeClass, cat: Category, ratio: Option<f64>) -> ModelRow {
        ModelRow {
            cls,
            shard: String::new(),
            cat,
            ratio,
            lower_bound: cat == Category::Stopped,
            tlc_ms: Some(1),
            core_ms: Some(1),
            reps: Vec::new(),
            range: None,
            rep_outcomes: Vec::new(),
            first_pass: None,
            pairs: Vec::new(),
            detail: None,
        }
    }

    #[test]
    fn the_summary_has_both_geomeans_and_the_gate() {
        let mut m = BTreeMap::new();
        m.insert(
            "a".to_owned(),
            row(SizeClass::Small, Category::Agreeing, Some(0.25)),
        );
        m.insert(
            "b".to_owned(),
            row(SizeClass::Large, Category::Agreeing, Some(1.2)),
        );
        m.insert(
            "c".to_owned(),
            row(SizeClass::Small, Category::Excluded, None),
        );
        m.insert(
            "d".to_owned(),
            row(SizeClass::Small, Category::Untimed, None),
        );
        let s = summarise(&m, true);
        assert_eq!((s.agreeing, s.small, s.large), (2, 1, 1));
        let g = (0.25_f64 * 1.2).sqrt();
        assert_eq!(s.geomean, Some(r4(g)));
        assert_eq!(
            s.fixed_set_rows, 3,
            "untimed rows are outside the fixed set"
        );
        assert_eq!(s.fixed_set_geomean, Some(r4((0.25_f64 * 1.2 * 2.0).cbrt())));
        assert_eq!(s.above_1x.keys().collect::<Vec<_>>(), ["b"]);
        assert_eq!(s.excluded, ["c"]);
        assert_eq!(s.gate_result, "pass");
        assert_eq!(summarise(&m, false).gate_result, "triage");
        m.insert(
            "e".to_owned(),
            row(SizeClass::Large, Category::Stopped, Some(2.5)),
        );
        let s = summarise(&m, true);
        assert_eq!(s.stopped, ["e"]);
        assert_eq!(s.above_2x, ["e"]);
        assert_eq!(s.gate_result, "fail");
    }

    #[test]
    fn classes_filter_by_class_and_selection() {
        let mut models = BTreeMap::new();
        for (m, class) in [
            ("x/A.cfg", SizeClass::Small),
            ("x/B.cfg", SizeClass::Large),
            ("y/C.cfg", SizeClass::Small),
        ] {
            models.insert(
                m.to_owned(),
                ClassRow {
                    class,
                    tlc_ms: None,
                },
            );
        }
        let classes = Classes {
            schema_version: R2_SCHEMA,
            kind: "r2-classes".into(),
            controls: controls(),
            pass: pass(Side::Tlc, &[]),
            timeout_s: 15,
            small_below_ms: stats::SMALL_BELOW_MS,
            models,
            not_timed: BTreeMap::new(),
        };
        let all = Selection {
            tier: "all".into(),
            filters: Vec::new(),
        };
        assert_eq!(classes.of(SizeClass::Small, &all), ["x/A.cfg", "y/C.cfg"]);
        let x = Selection {
            tier: "all".into(),
            filters: vec!["x/*".into()],
        };
        assert_eq!(classes.of(SizeClass::Small, &x), ["x/A.cfg"]);
        assert_eq!(classes.of(SizeClass::Large, &x), ["x/B.cfg"]);
    }
}
