//! Non-gating simulation study (`study simulate`, ADR 0116 §6, ADR 0162 §14).
//!
//! For every selected model, runs pinned TLC `-simulate` and `dive simulate`
//! with the same trace count and depth over the same seeds, and compares
//! verdict classes with each other and with the model's exhaustive golden.
//! Every dive violation must validate independently. Simulation traces and
//! counts are never compared: the two tools draw from different streams.

use crate::cache::Cache;
use crate::config::Config;
use crate::corpus::{ModelEntry, ModelMode};
use crate::dive::{DiveImplementation, StudyParams, StudyRun};
use crate::divergence::Profile;
use crate::error::{Error, Result};
use crate::implementation::Implementation as _;
use crate::oracle::{CheckMode, Java, OracleEnv, TlcJob};
use crate::pipeline::{Limits, Selection};
use crate::report::Host;
use crate::tlc::messages;
use crate::tlc::result::{RunFacts, Verdict, classify};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Scope of this evidence; it never enters the strict conformance summary.
pub const CLAIM: &str = "Verdict classes of sampled runs only: dive and TLC draw different traces, so traces and counts are not compared. A sample never proves a property.";

/// Fraction of seeds at which TLC's class obliges dive to report it too.
const COVERAGE_SEEDS: f64 = 0.9;

/// Row outcome. Only the finding variants make the study exit nonzero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Classes agree with TLC's simulation and the exhaustive golden; every
    /// violation validates.
    Agree,
    /// dive reports a violation class the exhaustive golden does not, and so
    /// does TLC's simulation on the same model (for example a deadlock where
    /// the constraints exclude every successor).
    AgreeWithSimulator,
    /// TLC's simulation reports, in at least 90% of seeds, a class that the
    /// exhaustive golden does not report.
    TlcContradictsGolden,
    /// TLC's simulation reports a class dive does not, by a registered
    /// divergence (named in the detail).
    RegisteredDivergence,
    /// dive refuses by name.
    Unsupported,
    /// A dive violation could not be validated: refused by name or no document.
    ValidationPending,
    /// TLC's simulation could not run or be classified.
    OracleError,
    /// A run hit the study's time or memory limit.
    Budget,
    /// Out of corpus scope.
    OutOfScope,
    /// A dive violation is refuted by independent validation.
    Invalid,
    /// A dive class that neither the golden nor TLC's simulation reports.
    ClassMismatch,
    /// TLC's simulation reports a class in at least 90% of seeds that dive's
    /// seed-0 run does not.
    CoverageGap,
}

impl Outcome {
    /// Whether this outcome is a finding.
    #[must_use]
    pub fn finding(self) -> bool {
        matches!(self, Self::Invalid | Self::ClassMismatch | Self::CoverageGap)
    }
}

/// One temporal property's per-trace coverage on seed 0.
#[derive(Debug, Clone, Serialize)]
pub struct Coverage {
    /// Property name.
    pub name: String,
    /// Traces on which the property was decided, with the default extension.
    pub checked: u64,
    /// Traces left unchecked, with the default extension.
    pub unchecked: u64,
    /// Traces decided with `--liveness-extend 0`.
    pub checked_no_extend: Option<u64>,
    /// Traces left unchecked with `--liveness-extend 0`.
    pub unchecked_no_extend: Option<u64>,
}

/// One model.
#[derive(Debug, Serialize)]
pub struct Row {
    /// Corpus-relative configuration.
    pub model: String,
    /// Exhaustive (or simulation) golden verdict, error class and property.
    pub golden: Option<(String, Option<String>, Option<String>)>,
    /// dive classes over the seeds, with counts.
    pub dive: BTreeMap<String, u32>,
    /// TLC classes over the seeds, with counts.
    pub tlc: BTreeMap<String, u32>,
    /// Distinct violated names dive reported.
    pub dive_properties: Vec<String>,
    /// Distinct violated names TLC reported.
    pub tlc_properties: Vec<String>,
    /// dive violations validated / reported.
    pub validated: (u32, u32),
    /// Median wall time of a dive and a TLC run, in ms.
    pub median_ms: (u64, u64),
    /// Temporal coverage on seed 0.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub coverage: Vec<Coverage>,
    /// Outcome.
    pub outcome: Outcome,
    /// Explanation (refusal, validation detail, or the mismatch).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// The study report.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Format version.
    pub schema_version: u32,
    /// Always `simulation-study`.
    pub kind: String,
    /// Limits on the evidence.
    pub claim: String,
    /// Generator revision.
    pub generator: String,
    /// UTC timestamp.
    pub generated_at: String,
    /// Oracle id and jar digest.
    pub oracle: (String, String),
    /// Corpus id and pin.
    pub corpus: (String, String),
    /// dive's `--version`.
    pub dive_version: Option<String>,
    /// dive profile.
    pub profile: Profile,
    /// Traces per run, depth and seeds.
    pub traces: u64,
    /// Trace depth (TLC's default, 100).
    pub depth: u64,
    /// Seeds `0..seeds`, the same for both tools.
    pub seeds: u32,
    /// The commands, with `<module>`, `<config>` and `<seed>` placeholders.
    pub commands: BTreeMap<String, Vec<String>>,
    /// Machine details.
    pub host: Host,
    /// Totals by outcome.
    pub totals: BTreeMap<Outcome, usize>,
    /// Every selected model.
    pub rows: Vec<Row>,
}

impl Report {
    /// Number of findings.
    #[must_use]
    pub fn findings(&self) -> usize {
        self.rows.iter().filter(|r| r.outcome.finding()).count()
    }

    /// Writes the report with one row per line, so it stays compact.
    ///
    /// # Errors
    /// Serialization or filesystem failure.
    pub fn write(&self, path: &Path) -> Result<()> {
        let fail = |e: serde_json::Error| Error::Config(format!("cannot serialize simulation study: {e}"));
        let mut header = serde_json::to_value(self).map_err(fail)?;
        if let Some(map) = header.as_object_mut() {
            map.remove("rows");
        }
        let header = serde_json::to_string_pretty(&header).map_err(fail)?;
        let mut text = header.trim_end().trim_end_matches('}').trim_end().to_owned();
        text.push_str(",\n  \"rows\": [\n");
        for (i, row) in self.rows.iter().enumerate() {
            let sep = if i + 1 == self.rows.len() { "" } else { "," };
            let _ = writeln!(text, "    {}{sep}", serde_json::to_string(row).map_err(fail)?);
        }
        text.push_str("  ]\n}\n");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, &e))?;
        }
        std::fs::write(path, text).map_err(|e| Error::io(path, &e))
    }
}

/// Study options.
pub struct Options {
    /// Corpus id.
    pub corpus: String,
    /// Tier and filters.
    pub selection: Selection,
    /// Pinned oracle id (TLC and goldens).
    pub oracle: String,
    /// dive binary.
    pub binary: PathBuf,
    /// dive profile.
    pub profile: Profile,
    /// Traces per run.
    pub traces: u64,
    /// Seeds per tool.
    pub seeds: u32,
    /// Parallel models, timeout per run and TLC `-Xmx`.
    pub limits: Limits,
    /// Explicit `java`.
    pub java: Option<PathBuf>,
}

/// TLC's default `-depth`, which the harness's TLC command keeps.
const DEPTH: u64 = 100;

struct TlcRun {
    class: String,
    property: Option<String>,
    elapsed_ms: u64,
}

/// Runs the study.
///
/// # Errors
/// Fails on harness errors (pins, cache, processes); model outcomes are rows.
pub fn run(config: &Config, cache: &Cache, opts: &Options) -> Result<Report> {
    if opts.seeds == 0 || opts.traces == 0 {
        return Err(Error::Usage("study simulate needs positive --seeds and --traces".into()));
    }
    let oracle = config.oracles.oracle(&opts.oracle)?;
    let java = Java::resolve(config, cache, opts.java.as_deref())?;
    let env = OracleEnv::prepare(config, cache, &opts.oracle, &java)?;
    let mut imp = DiveImplementation::prepare(config, cache, &opts.oracle, &opts.binary)?;
    imp.profile = opts.profile.clone();
    let corpus = crate::pipeline::load_corpus(config, cache, &opts.corpus)?;
    let models = opts.selection.models(&corpus)?;
    let timeout = opts.limits.timeout.unwrap_or(Duration::from_secs(120));
    let golden_root = crate::golden::golden_root(&config.repo_root);
    let rows = crate::pipeline::parallel_map(&models, opts.limits.jobs, |i, m| {
        let row = study_model(&env, &imp, &corpus.root, m, opts, timeout, &golden_root);
        let line = format!(
            "[{}/{}] {} {:?}",
            i + 1,
            models.len(),
            m.id,
            row.as_ref().map(|r| r.outcome)
        );
        let _ = writeln!(std::io::stderr().lock(), "{line}");
        row
    })
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    let mut totals = BTreeMap::new();
    for row in &rows {
        *totals.entry(row.outcome).or_default() += 1;
    }
    let mut commands = BTreeMap::new();
    commands.insert(
        "dive".into(),
        vec![
            "dive simulate <module> --config <config> --traces N --depth 100 --seed <seed> --workers 1 --keep-traces 0 --time-limit T --max-memory 4g --exit-codes tlc --engine core --output json".into(),
        ],
    );
    commands.insert(
        "tlc".into(),
        vec![format!(
            "{} (-depth 100 is TLC's default)",
            env.tlc_command_template(&opts.limits.xmx)
                .join(" ")
                .replace("-seed 0", "-seed <seed>")
                .replace("tlc2.TLC <module>", "tlc2.TLC <module> -simulate num=N")
        )],
    );
    Ok(Report {
        schema_version: 1,
        kind: "simulation-study".into(),
        claim: CLAIM.into(),
        generator: crate::pipeline::generator(),
        generated_at: crate::util::rfc3339(crate::util::unix_now()),
        oracle: (opts.oracle.clone(), oracle.sha256.clone()),
        corpus: (opts.corpus.clone(), corpus.commit.clone()),
        dive_version: imp.dive_provenance().map(|p| p.version.clone()),
        profile: opts.profile.clone(),
        traces: opts.traces,
        depth: DEPTH,
        seeds: opts.seeds,
        commands,
        host: Host::current(),
        totals,
        rows,
    })
}

fn study_model(
    env: &OracleEnv,
    imp: &DiveImplementation,
    root: &Path,
    m: &ModelEntry,
    opts: &Options,
    timeout: Duration,
    golden_root: &Path,
) -> Result<Row> {
    let mut row = Row {
        model: m.id.clone(),
        golden: None,
        dive: BTreeMap::new(),
        tlc: BTreeMap::new(),
        dive_properties: Vec::new(),
        tlc_properties: Vec::new(),
        validated: (0, 0),
        median_ms: (0, 0),
        coverage: Vec::new(),
        outcome: Outcome::OutOfScope,
        detail: String::new(),
    };
    if let Some(why) = &m.out_of_scope {
        row.detail.clone_from(why);
        return Ok(row);
    }
    if m.mode == ModelMode::Symbolic {
        "symbolic model".clone_into(&mut row.detail);
        return Ok(row);
    }
    let path = crate::golden::model_golden_path(golden_root, &opts.oracle, &opts.corpus, &m.id);
    let golden = crate::golden::read_model_golden(&path).ok().map(|g| g.result);
    row.golden = golden.as_ref().map(|g| {
        (
            g.verdict.as_str().to_owned(),
            g.error_class.clone(),
            g.violated_property.clone(),
        )
    });
    let mut dive_runs = Vec::new();
    let mut tlc_runs = Vec::new();
    // A refusal, CLI error or budget stop decides the row at its first
    // seed, so later seeds of that tool are not run.
    let stops = |class: &str| {
        matches!(class, "unsupported" | "cli-error" | "timeout" | "resource-limit" | "oracle-error" | "unclassified")
    };
    for seed in 0..opts.seeds {
        let params = StudyParams {
            traces: opts.traces,
            depth: DEPTH,
            seed: i64::from(seed),
            extend: None,
            timeout,
            memory: "4g",
        };
        if !dive_runs.last().is_some_and(|r: &StudyRun| stops(&r.class)) {
            dive_runs.push(imp.simulation_study_run(root, m, params)?);
        }
        if !tlc_runs.last().is_some_and(|r: &TlcRun| stops(&r.class)) {
            tlc_runs.push(tlc_run(env, root, m, opts, seed, timeout)?);
        }
    }
    if let Some(first) = dive_runs.first()
        && !first.temporal.is_empty()
    {
        let params = StudyParams {
            traces: opts.traces,
            depth: DEPTH,
            seed: 0,
            extend: Some("0"),
            timeout,
            memory: "4g",
        };
        let flat = imp.simulation_study_run(root, m, params)?;
        row.coverage = first
            .temporal
            .iter()
            .map(|(name, checked, unchecked)| {
                let other = flat.temporal.iter().find(|(n, _, _)| n == name);
                Coverage {
                    name: name.clone(),
                    checked: *checked,
                    unchecked: *unchecked,
                    checked_no_extend: other.map(|o| o.1),
                    unchecked_no_extend: other.map(|o| o.2),
                }
            })
            .collect();
    }
    let quotient = std::fs::read_to_string(root.join(&m.id)).is_ok_and(|cfg| {
        cfg.lines()
            .any(|l| matches!(l.split_whitespace().next(), Some("VIEW" | "SYMMETRY")))
    });
    summarize(&mut row, &dive_runs, &tlc_runs, golden.map(|g| g.verdict), quotient);
    Ok(row)
}

fn tlc_run(
    env: &OracleEnv,
    root: &Path,
    m: &ModelEntry,
    opts: &Options,
    seed: u32,
    timeout: Duration,
) -> Result<TlcRun> {
    let job = TlcJob {
        corpus_root: root,
        module: &m.module,
        config: &m.id,
        mode: CheckMode::Simulate {
            traces: opts.traces,
        },
        features: &m.features,
        workers: 1,
        timeout,
        xmx: &opts.limits.xmx,
        seed: i64::from(seed),
        lncheck: "final",
        fp: 0,
        dump_trace: false,
    };
    let inv = env.run_tlc(&job)?;
    let result = classify(
        &messages::parse(&inv.outcome.stdout),
        &RunFacts {
            exit_code: inv.outcome.exit_code,
            timed_out: inv.outcome.timed_out,
            trace_json: None,
            simulate: true,
            stderr: Some(&inv.outcome.stderr),
        },
    );
    Ok(TlcRun {
        class: result.verdict.as_str().to_owned(),
        property: result.violated_property,
        elapsed_ms: u64::try_from(inv.outcome.elapsed.as_millis()).unwrap_or(u64::MAX),
    })
}

fn median(mut v: Vec<u64>) -> u64 {
    v.sort_unstable();
    v.get(v.len() / 2).copied().unwrap_or(0)
}

const BUDGET: [&str; 2] = ["timeout", "resource-limit"];
const ORACLE_FAILURE: [&str; 3] = ["oracle-error", "unclassified", "resource-limit"];

fn summarize(
    row: &mut Row,
    dive: &[StudyRun],
    tlc: &[TlcRun],
    golden: Option<Verdict>,
    quotient: bool,
) {
    let success = Verdict::Success.as_str();
    for r in dive {
        *row.dive.entry(r.class.clone()).or_default() += 1;
        if let Some(p) = &r.property
            && !row.dive_properties.contains(p)
        {
            row.dive_properties.push(p.clone());
        }
        if let Some(v) = &r.validation {
            row.validated.1 += 1;
            row.validated.0 += u32::from(v.validated);
        }
    }
    for r in tlc {
        *row.tlc.entry(r.class.clone()).or_default() += 1;
        if let Some(p) = &r.property
            && !row.tlc_properties.contains(p)
        {
            row.tlc_properties.push(p.clone());
        }
    }
    row.median_ms = (
        median(dive.iter().map(|r| r.elapsed_ms).collect()),
        median(tlc.iter().map(|r| r.elapsed_ms).collect()),
    );
    let golden = golden.map(Verdict::as_str);
    let first = &dive[0];
    let (outcome, detail) = if let Some(r) = dive.iter().find(|r| r.class == "unsupported") {
        (Outcome::Unsupported, r.message.clone().unwrap_or_default())
    } else if let Some(r) = dive.iter().find(|r| r.validation.as_ref().is_some_and(|v| v.refuted)) {
        (
            Outcome::Invalid,
            r.validation.as_ref().map(|v| v.detail.clone()).unwrap_or_default(),
        )
    } else if let Some(r) = dive
        .iter()
        .find(|r| r.validation.as_ref().is_some_and(|v| !v.validated))
    {
        (
            Outcome::ValidationPending,
            r.validation.as_ref().map(|v| v.detail.clone()).unwrap_or_default(),
        )
    } else if let Some(r) = dive.iter().find(|r| BUDGET.contains(&r.class.as_str())) {
        (
            Outcome::Budget,
            format!("dive {}: {}", r.class, r.message.clone().unwrap_or_default()),
        )
    } else if dive.iter().any(|r| r.class == "cli-error") {
        (
            Outcome::Unsupported,
            format!(
                "CLI error: {}",
                dive.iter()
                    .find_map(|r| r.message.clone())
                    .unwrap_or_default()
            ),
        )
    } else if let Some(r) = tlc.iter().find(|r| ORACLE_FAILURE.contains(&r.class.as_str()) || r.class == "timeout") {
        (Outcome::OracleError, format!("TLC {}", r.class))
    } else if let Some(r) = dive
        .iter()
        .find(|r| r.class != success && Some(r.class.as_str()) != golden && !row.tlc.contains_key(&r.class))
    {
        (
            Outcome::ClassMismatch,
            format!(
                "dive reports {} {}; golden {golden:?}; TLC simulate {:?}",
                r.class,
                r.message.clone().or_else(|| r.property.clone()).unwrap_or_default(),
                row.tlc
            ),
        )
    } else if let Some((class, _)) = row.tlc.iter().find(|(c, n)| {
        c.as_str() != success
            && f64::from(**n) >= COVERAGE_SEEDS * f64::from(u32::try_from(tlc.len()).unwrap_or(1))
            && first.class != **c
    }) {
        if golden.is_some() && golden != Some(class.as_str()) {
            (
                Outcome::TlcContradictsGolden,
                format!(
                    "TLC simulate reports {class}, which the exhaustive golden ({}) does not; dive seed 0 reports {}",
                    golden.unwrap_or_default(),
                    first.class
                ),
            )
        } else if class == Verdict::LivenessViolation.as_str() && quotient {
            (
                Outcome::RegisteredDivergence,
                format!(
                    "D-037: TLC closes loops on VIEW or symmetric fingerprints; dive closes them only on equal states and reports {}",
                    first.class
                ),
            )
        } else {
            (
                Outcome::CoverageGap,
                format!("TLC simulate reports {class} in {:?}; dive seed 0 reports {}", row.tlc, first.class),
            )
        }
    } else if dive
        .iter()
        .any(|r| r.class != success && Some(r.class.as_str()) != golden)
    {
        (
            Outcome::AgreeWithSimulator,
            format!("golden {golden:?}; both simulators report {:?}", row.dive.keys().collect::<Vec<_>>()),
        )
    } else {
        (Outcome::Agree, String::new())
    };
    row.outcome = outcome;
    row.detail = detail;
}
