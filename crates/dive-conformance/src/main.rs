//! The `dive-conformance` binary. Run with `--help` for usage.
#![forbid(unsafe_code)]

use dive_conformance::baseline::Baseline;
use dive_conformance::cache::Cache;
use dive_conformance::compare::Outcome;
use dive_conformance::config::Config;
use dive_conformance::divergence::Profile;
use dive_conformance::error::{Error, Result};
use dive_conformance::implementation::{
    DiveEngine, DiveImplementation, ExternalAdapter, ImplOption, Implementation, TlcImplementation,
    classify_with_trace_pass,
};
use dive_conformance::oracle::{CheckMode, Java, OracleEnv, TlcJob};
use dive_conformance::pipeline::{self, GenerateOptions, Limits, RunOptions, Selection};
use dive_conformance::sany::{self, SanyPhase};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "dive-conformance: differential conformance harness for dive

USAGE:
    dive-conformance [GLOBAL] <COMMAND> [OPTIONS]

GLOBAL:
    --repo-root <dir>      repository root (default: search upwards for conformance/oracles.toml)
    --cache-dir <dir>      cache directory (default: $DIVE_CACHE_DIR or ~/.cache/dive)

COMMANDS:
    oracle fetch           download and verify the gating oracles (or --oracle <id>), the
                           classpath jars, the JRE for this host, and the module libraries
    oracle verify          verify the cached gating oracles (or --oracle <id>) and print
                           their probed capabilities

    compare-models         compare --oracle-model FILE with --model FILE using pinned TLC
                           and dive in both profiles; identical --config FILE bytes,
                           --module-path DIR (repeatable), --dive-bin BIN, --out DIR
                           retaining raw observations, --timeout SECS (default 120)

    Reference oracles (role = \"reference\" in oracles.toml) are used only when named
    with --oracle, --golden-oracle or --impl tlc:<id>, and never gate: a run involving
    one exits 0 on findings and its report is marked reference.
    corpus fetch           check out pinned corpora (--corpus <id>, repeatable; default all)
    corpus list            list models with tier, mode and scope
    golden generate        generate goldens from an oracle
    run                    run an implementation against the goldens and write a report
    shadow                 run dive's core and reference engines on every selected model the
                           core accepts (default profile, --workers, default 1,4) and compare
                           verdict, error class, exit, property, counts and depth; the core's
                           counterexamples must validate. Exit 1 on any difference. Takes the
                           selection options, --dive-bin, --jobs, budgets and --report-dir
                           (default: no report file). --strict: a model the core refuses, or
                           a timeout, is a finding too (for fixed gate lists). --sample N
                           (default 32, 0 for none): compare the successors of N sampled
                           states per core run with the reference evaluator's
    study final            observe dive's schema-2/3 final/short mode against pinned goldens;
                           truth/count evidence only, never a strict conformance outcome
    report summarise <report.json>...
                           re-render the summary sections of existing reports into
                           --summary (default docs/conformance.md), one per implementation
    report baseline <report.json> --out <file>
                           write the compact baseline of a report (for `run --baseline`)
    trace-controls         run every golden counterexample through dive's independent trace
                           validator under --compat=tlc (positive controls) and check that
                           each by-construction mutant is rejected (negative controls;
                           ADR 0016). Takes the selection options and --golden-oracle

SELECTION (corpus list, golden generate, run, trace-controls, study final):
    --corpus <id>          corpus (default: examples); `run` takes several, into one report
    --tier <t>             small | deep | all (default: small)
    --filter <glob>        model/module path glob; repeatable (`*`, `**`, `?`)

GOLDEN GENERATE:
    --oracle <id>          oracle (default: the default oracle)
    --reason <text>        recorded reason; required to overwrite existing goldens
    --workers <list>       TLC -workers (default: 1). A count above 1 writes worker-specific
                           goldens (<model>.workers-<n>.json) beside the single-worker ones,
                           which it never touches, for worker-introspecting models only, and
                           requires --reason. `run --impl-profile tlc` compares those models
                           at that count with them, on verdict, error class and numeric exit (ADR 0004/0096)
    --no-sany | --sany-only

RUN:
    --impl <impl>          tlc | tlc:<oracle-id> | dive | dive:<engine> | adapter:<file.toml>
                           (dive engines: reference (default, or DIVE_ENGINE) | core | auto;
                           each report row records the engine that ran)
    --golden-oracle <id>   oracle whose goldens are compared (default: the default oracle)
    --workers <list>       worker counts, e.g. 1,4 (default: 1)
    --crosscheck-manifest  compare goldens with the Examples manifests (oracle drift)
    --no-sany              skip SANY goldens
    --sany-only            compare modules only, without running model checking
    --sany-phase <p>       full (default): compare full SANY (dive: `dive analyze`);
                           parse: compare SANY's parse-phase verdict of each module's own
                           file, derived from the same goldens (dive: `dive parse`)
    --dive-bin <path>      the dive binary for --impl dive (default: target/release/dive)
    --impl-profile P       dive profile: dive (default) | tlc | tlc+overrides=<f>,... (ADR 0010);
                           DIVE_COMPAT=tlc selects tlc when not given
    --impl-option N[=V]    run dive/adapter under an option (repeatable; ADR 0010). Options
                           the implementation does not declare result-preserving make every
                           would-be `agree` a `divergence-mode` outcome
    --report-dir <dir>     default: conformance/reports
    --baseline <file>      after the run, compare with this baseline (or full report) of the
                           same implementation, profile, tier, workers and corpora: exit 1 if
                           a model or module that agreed no longer does, or a model's
                           observables (verdict, error class/code, exit, property, counts,
                           depth) changed. Improvements are listed and pass
    --summary <file>       default: docs/conformance.md;  --no-summary to skip

FINAL STUDY:
    --dive-bin <path>      explicit schema-2/3 final/short executable
    --impl-profile P      dive | tlc (tlc reports named final/short overrides)
    --golden-oracle <id>   pinned single-worker final goldens
    --workers <list>       requested widths; observations run sequentially
    --out <path>          separate study JSON; never writes docs/conformance.md
                           seeds/fingerprint fixed at 0; no --jobs/--java/--impl

LIMITS (golden generate, run; timeout options also apply to study final):
    --jobs <n>             parallel oracle runs (default: min(4, cpus/2))
    --xmx <size>           JVM heap per run (default: 3g)
    --timeout <secs>       fixed per-model timeout (default: derived from recorded runtime)
    --timeout-cap <secs>   cap on derived timeouts (default: 3600)
    --tlc-budgets          derive each model's budget from its golden's TLC time
                           and pass it to dive as --time-limit, with --xmx as
                           --max-memory (ADR 0107); selected by model gates/nightly
    --budget-cap <secs>    hard cap on those budgets (default: 900)
    --java <path>          use this java instead of the pinned JRE (recorded in provenance)
    --raw-invocations <dir> retain original TLC model streams (golden generate only;
                           default off; excludes Java setup and capability probes)
    --raw-setup-status     also retain bounded setup status/streams in a separate schema
                           requires --raw-invocations; default off

EXIT STATUS:
    0 success; 1 findings (failed goldens, or disagree/oracle-error/timeout outcomes,
    except in runs involving a reference oracle, or `--baseline` regressions);
    2 usage error; 3 harness error.
    `known-upstream-change` and `expected-divergence` outcomes are not findings.
";

struct Args {
    items: Vec<String>,
    pos: usize,
}

impl Args {
    fn next(&mut self) -> Option<String> {
        let v = self.items.get(self.pos).cloned();
        self.pos += 1;
        v
    }
    fn value(&mut self, flag: &str) -> Result<String> {
        self.next()
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(|| Error::Usage(format!("{flag} needs a value")))
    }
}

#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent command-line switches"
)]
struct Opts {
    repo_root: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    corpus: Vec<String>,
    tier: Option<String>,
    filters: Vec<String>,
    oracle: Option<String>,
    reason: Option<String>,
    no_sany: bool,
    /// `shadow --strict`: refusals and timeouts are findings too.
    strict: bool,
    /// `shadow --sample N`: sampled states per core run.
    sample: Option<usize>,
    sany_only: bool,
    imp: Option<String>,
    golden_oracle: Option<String>,
    workers: Option<Vec<u32>>,
    crosscheck: bool,
    report_dir: Option<PathBuf>,
    summary: Option<PathBuf>,
    no_summary: bool,
    jobs: Option<usize>,
    xmx: Option<String>,
    timeout: Option<u64>,
    timeout_cap: Option<u64>,
    budget_cap: Option<u64>,
    tlc_budgets: bool,
    java: Option<PathBuf>,
    raw_invocations: Option<PathBuf>,
    raw_setup_status: bool,
    impl_options: Vec<String>,
    impl_profile: Option<String>,
    sany_phase: Option<String>,
    dive_bin: Option<PathBuf>,
    specs: Option<PathBuf>,
    out: Option<PathBuf>,
    baseline: Option<PathBuf>,
    seed: Option<i64>,
    lncheck: Option<String>,
    fp: Option<u32>,
    features: Vec<String>,
    oracle_model: Option<PathBuf>,
    model: Option<PathBuf>,
    model_config: Option<PathBuf>,
    model_id: Option<String>,
    module_paths: Vec<PathBuf>,
    overall_timeout: Option<u64>,
    positional: Vec<String>,
}

fn parse_num<T: std::str::FromStr>(flag: &str, v: &str) -> Result<T> {
    v.parse()
        .map_err(|_| Error::Usage(format!("{flag}: `{v}` is not a valid number")))
}

fn parse_opts(args: &mut Args) -> Result<Opts> {
    let mut o = Opts::default();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--repo-root" => o.repo_root = Some(args.value(&a)?.into()),
            "--cache-dir" => o.cache_dir = Some(args.value(&a)?.into()),
            "--corpus" => o.corpus.push(args.value(&a)?),
            "--tier" => o.tier = Some(args.value(&a)?),
            "--filter" => o.filters.push(args.value(&a)?),
            "--oracle" => o.oracle = Some(args.value(&a)?),
            "--reason" => o.reason = Some(args.value(&a)?),
            "--no-sany" => o.no_sany = true,
            "--strict" => o.strict = true,
            "--sample" => o.sample = Some(parse_num(&a, &args.value(&a)?)?),
            "--sany-only" => o.sany_only = true,
            "--impl" => o.imp = Some(args.value(&a)?),
            "--golden-oracle" => o.golden_oracle = Some(args.value(&a)?),
            "--workers" => {
                let v = args.value(&a)?;
                let list = v
                    .split(',')
                    .map(|w| parse_num::<u32>(&a, w.trim()))
                    .collect::<Result<Vec<_>>>()?;
                if list.is_empty() || list.contains(&0) {
                    return Err(Error::Usage("--workers needs positive counts".to_owned()));
                }
                o.workers = Some(list);
            }
            "--crosscheck-manifest" => o.crosscheck = true,
            "--report-dir" => o.report_dir = Some(args.value(&a)?.into()),
            "--summary" => o.summary = Some(args.value(&a)?.into()),
            "--no-summary" => o.no_summary = true,
            "--jobs" => o.jobs = Some(parse_num(&a, &args.value(&a)?)?),
            "--xmx" => {
                let v = args.value(&a)?;
                if !v.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Err(Error::Usage(format!("--xmx: bad size `{v}`")));
                }
                o.xmx = Some(v);
            }
            "--timeout" => o.timeout = Some(parse_num(&a, &args.value(&a)?)?),
            "--timeout-cap" => o.timeout_cap = Some(parse_num(&a, &args.value(&a)?)?),
            "--budget-cap" => o.budget_cap = Some(parse_num(&a, &args.value(&a)?)?),
            "--tlc-budgets" => o.tlc_budgets = true,
            "--java" => o.java = Some(args.value(&a)?.into()),
            "--raw-invocations" => o.raw_invocations = Some(args.value(&a)?.into()),
            "--raw-setup-status" => o.raw_setup_status = true,
            "--impl-option" => o.impl_options.push(args.value(&a)?),
            "--impl-profile" => o.impl_profile = Some(args.value(&a)?),
            "--sany-phase" => o.sany_phase = Some(args.value(&a)?),
            "--dive-bin" => o.dive_bin = Some(args.value(&a)?.into()),
            "--specs" => o.specs = Some(args.value(&a)?.into()),
            "--out" => o.out = Some(args.value(&a)?.into()),
            "--oracle-model" => o.oracle_model = Some(args.value(&a)?.into()),
            "--model" => o.model = Some(args.value(&a)?.into()),
            "--config" => o.model_config = Some(args.value(&a)?.into()),
            "--model-id" => o.model_id = Some(args.value(&a)?),
            "--module-path" => o.module_paths.push(args.value(&a)?.into()),
            "--overall-timeout" => o.overall_timeout = Some(parse_num(&a, &args.value(&a)?)?),
            "--baseline" => o.baseline = Some(args.value(&a)?.into()),
            // `next()`, not `value()`: a seed may be negative (`--seed -3`).
            "--seed" => {
                let v = args
                    .next()
                    .ok_or_else(|| Error::Usage("--seed needs a value".to_owned()))?;
                o.seed = Some(parse_num(&a, &v)?);
            }
            "--lncheck" => {
                let v = args.value(&a)?;
                if !matches!(v.as_str(), "final" | "default" | "off") {
                    return Err(Error::Usage(format!(
                        "--lncheck: `{v}` is not one of final, default, off"
                    )));
                }
                o.lncheck = Some(v);
            }
            "--fp" => o.fp = Some(parse_num(&a, &args.value(&a)?)?),
            "--feature" => o.features.push(args.value(&a)?),
            s if s.starts_with("--") => return Err(Error::Usage(format!("unknown option `{s}`"))),
            _ => o.positional.push(a),
        }
    }
    Ok(o)
}

fn limits(o: &Opts) -> Limits {
    let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    Limits {
        jobs: o.jobs.unwrap_or_else(|| (cpus / 2).clamp(1, 4)).max(1),
        xmx: o.xmx.clone().unwrap_or_else(|| "3g".to_owned()),
        timeout: o.timeout.map(Duration::from_secs),
        timeout_cap: Duration::from_secs(o.timeout_cap.unwrap_or(3600)),
        budget_cap: o
            .tlc_budgets
            .then(|| Duration::from_secs(o.budget_cap.unwrap_or(900))),
    }
}

fn selection(o: &Opts) -> Selection {
    Selection {
        tier: o.tier.clone().unwrap_or_else(|| "small".to_owned()),
        filters: o.filters.clone(),
    }
}

fn corpus_id(o: &Opts) -> Result<String> {
    match o.corpus.as_slice() {
        [] => Ok("examples".to_owned()),
        [one] => Ok(one.clone()),
        _ => Err(Error::Usage(
            "give one --corpus for this command".to_owned(),
        )),
    }
}

fn out(s: &str) {
    let _ = write!(std::io::stdout().lock(), "{s}");
}

/// Runs the command; `Ok(true)` means findings (exit 1).
fn dispatch(cmd: &[String], o: &Opts) -> Result<bool> {
    if o.raw_invocations.is_some()
        && cmd.iter().map(String::as_str).collect::<Vec<_>>() != ["golden", "generate"]
    {
        return Err(Error::Usage(
            "--raw-invocations is only supported by golden generate".to_owned(),
        ));
    }
    if o.raw_setup_status && o.raw_invocations.is_none() {
        return Err(Error::Usage(
            "--raw-setup-status requires --raw-invocations".to_owned(),
        ));
    }
    let root = match &o.repo_root {
        Some(r) => r.clone(),
        None => Config::find_repo_root(
            &std::env::current_dir().map_err(|e| Error::Io(format!("current directory: {e}")))?,
        )?,
    };
    let config = Config::load(&root)?;
    let cache = o
        .cache_dir
        .clone()
        .map_or_else(Cache::from_env, |d| Ok(Cache::new(d)))?;
    let words: Vec<&str> = cmd.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["oracle", "fetch"] => {
            pipeline::oracle_fetch(&config, &cache, o.oracle.as_deref())?;
            Ok(false)
        }
        ["oracle", "verify"] => {
            let java = Java::resolve(&config, &cache, o.java.as_deref())?;
            for oracle in config.oracles.selected_or_gating(o.oracle.as_deref())? {
                let env = OracleEnv::prepare(&config, &cache, &oracle.id, &java)?;
                out(&format!(
                    "{}: verified; java: {}\n{}\n",
                    oracle.id,
                    java.version_line,
                    serde_json::to_string_pretty(&env.capabilities).unwrap_or_default()
                ));
            }
            Ok(false)
        }
        ["corpus", "fetch"] => {
            pipeline::corpus_fetch(&config, &cache, &o.corpus)?;
            Ok(false)
        }
        ["study", "final"] => cmd_final_study(&config, &cache, &root, o),
        ["corpus", "list"] => {
            out(&pipeline::corpus_list(
                &config,
                &cache,
                &corpus_id(o)?,
                &selection(o),
            )?);
            Ok(false)
        }
        ["golden", "generate"] => cmd_golden_generate(&config, &cache, o),
        ["run"] => cmd_run(&config, &cache, &root, o),
        ["shadow"] => cmd_shadow(&config, &cache, &root, o),
        ["report", "summarise"] => report_summarise(&root, o),
        ["report", "baseline"] => report_baseline(o),
        ["trace-controls"] => cmd_trace_controls(&config, &cache, &root, o),
        ["compare-models"] => cmd_compare_models(&config, &cache, o),
        ["dev", "capture"] => {
            dev_capture(&config, &cache, o)?;
            Ok(false)
        }
        _ => Err(Error::Usage(format!(
            "unknown command `{}`\n\n{USAGE}",
            cmd.join(" ")
        ))),
    }
}

fn cmd_compare_models(config: &Config, cache: &Cache, o: &Opts) -> Result<bool> {
    let required = |value: &Option<PathBuf>, flag: &str| {
        value
            .clone()
            .ok_or_else(|| Error::Usage(format!("compare-models requires {flag}")))
    };
    let oracle = required(&o.oracle_model, "--oracle-model")?;
    let native = required(&o.model, "--model")?;
    let cfg = required(&o.model_config, "--config")?;
    let dive = required(&o.dive_bin, "--dive-bin")?;
    let raw = required(&o.out, "--out")?;
    let report = dive_conformance::model_pair::compare(
        config,
        cache,
        &dive_conformance::model_pair::ModelPair {
            oracle_module: &oracle,
            native_module: &native,
            configuration: &cfg,
            model_id: o
                .model_id
                .as_deref()
                .unwrap_or("unregistered-translated-model"),
            module_paths: &o.module_paths,
            dive: &dive,
            timeout: Duration::from_secs(o.timeout.unwrap_or(120)),
            overall_timeout: o.overall_timeout.map_or_else(
                || {
                    Duration::from_secs(o.timeout.unwrap_or(120))
                        .saturating_mul(4)
                        .saturating_add(Duration::from_secs(60))
                },
                Duration::from_secs,
            ),
            raw_directory: &raw,
        },
    )?;
    out(&format!(
        "{}\n",
        serde_json::to_string_pretty(&report).map_err(|e| Error::Config(e.to_string()))?
    ));
    Ok(report["profiles"]
        .as_object()
        .is_none_or(|profiles| profiles.values().any(|row| row["outcome"] != "agree")))
}

/// Generates goldens through the default or explicitly retained setup path.
fn cmd_golden_generate(config: &Config, cache: &Cache, o: &Opts) -> Result<bool> {
    let java = if o.raw_setup_status {
        None
    } else {
        Some(Java::resolve(config, cache, o.java.as_deref())?)
    };
    let oracle = match &o.oracle {
        Some(id) => id.clone(),
        None => config.oracles.default_oracle()?.id.clone(),
    };
    let env = match java.as_ref() {
        Some(java) => {
            let mut env = OracleEnv::prepare(config, cache, &oracle, java)?;
            env.raw_invocations.clone_from(&o.raw_invocations);
            Some(env)
        }
        None => None,
    };
    if o.no_sany && o.sany_only {
        return Err(Error::Usage(
            "--no-sany and --sany-only conflict".to_owned(),
        ));
    }
    let opts = GenerateOptions {
        oracle,
        corpus: corpus_id(o)?,
        selection: selection(o),
        reason: o.reason.clone(),
        limits: limits(o),
        models: !o.sany_only,
        sany: !o.no_sany,
        workers: o.workers.clone().unwrap_or_else(|| vec![1]),
    };
    let failures = if o.raw_setup_status {
        let raw_root = o.raw_invocations.as_deref().ok_or_else(|| {
            Error::Usage("--raw-setup-status requires --raw-invocations".to_owned())
        })?;
        pipeline::golden_generate_with_setup_capture(
            config,
            cache,
            &opts,
            o.java.as_deref(),
            raw_root,
        )?
    } else {
        let env = env
            .as_ref()
            .ok_or_else(|| Error::Config("missing prepared oracle".to_owned()))?;
        pipeline::golden_generate(config, cache, env, &opts)?
    };
    if failures > 0 {
        eprintln_safe(&format!("{failures} golden(s) not written; see above"));
    }
    Ok(failures > 0)
}

/// `report summarise <report.json>...`: merges each report's section into
/// the summary file.
fn report_summarise(root: &Path, o: &Opts) -> Result<bool> {
    let md = o
        .summary
        .clone()
        .unwrap_or_else(|| root.join("docs").join("conformance.md"));
    if o.positional.is_empty() {
        return Err(Error::Usage(
            "`report summarise` needs one or more report files".to_owned(),
        ));
    }
    for file in &o.positional {
        let text = std::fs::read_to_string(file).map_err(|e| Error::Io(format!("{file}: {e}")))?;
        let report: dive_conformance::report::Report = serde_json::from_str(&text)
            .map_err(|e| Error::Config(format!("{file}: not a report: {e}")))?;
        let existing = std::fs::read_to_string(&md).ok();
        let merged = dive_conformance::report::merge_summary(
            existing.as_deref(),
            &dive_conformance::report::summary_key(&report),
            &dive_conformance::report::render_markdown(&report)?,
        );
        std::fs::write(&md, merged).map_err(|e| Error::Io(format!("{}: {e}", md.display())))?;
    }
    Ok(false)
}

/// `report baseline <report.json> --out <file>`: writes the report's
/// compact baseline.
fn report_baseline(o: &Opts) -> Result<bool> {
    let ([file], Some(dest)) = (o.positional.as_slice(), &o.out) else {
        return Err(Error::Usage(
            "`report baseline` needs one report file and --out <file>".to_owned(),
        ));
    };
    let baseline = Baseline::load(Path::new(file))?;
    baseline.write(dest)?;
    out(&format!(
        "{}: {} model(s), {} module(s) from {}\n",
        dest.display(),
        baseline.models.len(),
        baseline.modules.len(),
        baseline.source_run_id
    ));
    Ok(false)
}

/// `trace-controls`: the validator's positive and negative controls over
/// the golden counterexamples (ADR 0016 §3).
fn cmd_trace_controls(config: &Config, cache: &Cache, root: &Path, o: &Opts) -> Result<bool> {
    let golden_oracle = match &o.golden_oracle {
        Some(id) => id.clone(),
        None => config.oracles.default_oracle()?.id.clone(),
    };
    let corpora = if o.corpus.is_empty() {
        vec!["examples".to_owned()]
    } else {
        o.corpus.clone()
    };
    let summary = dive_conformance::trace_check::run_controls(
        config,
        cache,
        root,
        &corpora,
        &selection(o),
        &golden_oracle,
    )?;
    out(&summary.render());
    let findings = summary.findings();
    for f in &findings {
        eprintln_safe(&format!("finding: {f}"));
    }
    Ok(!findings.is_empty())
}

/// Non-gating final/short observation; does not write the conformance summary.
fn cmd_final_study(config: &Config, cache: &Cache, root: &Path, o: &Opts) -> Result<bool> {
    if o.imp.is_some()
        || !o.impl_options.is_empty()
        || o.java.is_some()
        || o.no_sany
        || o.sany_only
        || o.sany_phase.is_some()
        || o.crosscheck
        || o.summary.is_some()
        || o.lncheck.is_some()
        || o.seed.is_some()
        || o.fp.is_some()
        || o.specs.is_some()
        || o.jobs.is_some()
        || o.oracle.is_some()
        || o.reason.is_some()
        || !o.positional.is_empty()
    {
        return Err(Error::Usage("study final takes selection, --dive-bin, --impl-profile dive|tlc, --workers, timeout and --out options; modes/seeds are fixed".into()));
    }
    let profile = match &o.impl_profile {
        Some(p) => Profile::parse(p)?,
        None => Profile::from_env()?.unwrap_or_default(),
    };
    let opts = dive_conformance::final_study::Options {
        corpora: if o.corpus.is_empty() {
            vec!["examples".into()]
        } else {
            o.corpus.clone()
        },
        selection: selection(o),
        oracle: o
            .golden_oracle
            .clone()
            .unwrap_or(config.oracles.default_oracle()?.id.clone()),
        binary: o
            .dive_bin
            .clone()
            .unwrap_or_else(|| root.join("target/release/dive")),
        profile,
        workers: o.workers.clone().unwrap_or_else(|| vec![1]),
        limits: limits(o),
    };
    let report = dive_conformance::final_study::run(config, cache, &opts)?;
    let path = o.out.clone().unwrap_or_else(|| {
        o.report_dir
            .clone()
            .unwrap_or_else(|| root.join("conformance/reports/studies"))
            .join(format!(
                "{}-final-short.json",
                dive_conformance::util::unix_now()
            ))
    });
    report.write(&path)?;
    out(&format!(
        "{}\n{} model/worker observations: {:?}; {} findings\nreport: {}\n",
        report.claim,
        report.rows.len(),
        report.totals,
        report.findings(),
        path.display()
    ));
    Ok(report.findings() > 0)
}

/// `--sany-phase full|parse`.
fn sany_phase(o: &Opts) -> Result<SanyPhase> {
    match o.sany_phase.as_deref() {
        None | Some("full") => Ok(SanyPhase::Full),
        Some("parse") if o.no_sany => Err(Error::Usage(
            "--sany-phase parse and --no-sany conflict".to_owned(),
        )),
        Some("parse") => Ok(SanyPhase::Parse),
        Some(other) => Err(Error::Usage(format!(
            "--sany-phase must be full or parse (got `{other}`)"
        ))),
    }
}

/// `run`: returns whether there were findings.
fn cmd_run(config: &Config, cache: &Cache, root: &Path, o: &Opts) -> Result<bool> {
    let imp_spec = o
        .imp
        .clone()
        .ok_or_else(|| Error::Usage("run needs --impl".to_owned()))?;
    let golden_oracle = match &o.golden_oracle {
        Some(id) => id.clone(),
        None => config.oracles.default_oracle()?.id.clone(),
    };
    let dive_engine = dive_engine(&imp_spec)?;
    if o.dive_bin.is_some() && dive_engine.is_none() {
        return Err(Error::Usage(
            "--dive-bin applies to --impl dive only".to_owned(),
        ));
    }
    if o.no_sany && o.sany_only {
        return Err(Error::Usage(
            "--no-sany and --sany-only conflict".to_owned(),
        ));
    }
    // Load the baseline first: a bad path should not cost a whole run.
    let baseline = o.baseline.as_deref().map(Baseline::load).transpose()?;
    let opts = RunOptions {
        corpora: if o.corpus.is_empty() {
            vec!["examples".to_owned()]
        } else {
            o.corpus.clone()
        },
        sany_phase: sany_phase(o)?,
        golden_oracle: golden_oracle.clone(),
        selection: selection(o),
        workers: o.workers.clone().unwrap_or_else(|| vec![1]),
        limits: limits(o),
        crosscheck_manifest: o.crosscheck,
        models: !o.sany_only,
        sany: !o.no_sany,
        report_dir: o
            .report_dir
            .clone()
            .unwrap_or_else(|| root.join("conformance").join("reports")),
        summary_path: if o.no_summary {
            None
        } else {
            Some(
                o.summary
                    .clone()
                    .unwrap_or_else(|| root.join("docs").join("conformance.md")),
            )
        },
    };
    let report = if imp_spec == "tlc" || imp_spec.starts_with("tlc:") {
        let id = imp_spec
            .strip_prefix("tlc:")
            .unwrap_or(&golden_oracle)
            .to_owned();
        let java = Java::resolve(config, cache, o.java.as_deref())?;
        let env = OracleEnv::prepare(config, cache, &id, &java)?;
        if !o.impl_options.is_empty() || o.impl_profile.is_some() {
            return Err(Error::Usage(
                "--impl-option/--impl-profile apply to dive (and options to adapters); TLC oracles take none".to_owned(),
            ));
        }
        let imp = TlcImplementation { env };
        pipeline::run(config, cache, &imp, Some(&imp.env), &opts)?
    } else if let Some(engine) = dive_engine {
        let bin = o.dive_bin.clone().unwrap_or_else(|| {
            root.join("target")
                .join("release")
                .join(format!("dive{}", std::env::consts::EXE_SUFFIX))
        });
        let mut imp = DiveImplementation::prepare(config, cache, &golden_oracle, &bin)?;
        imp.engine = engine;
        imp.profile = match &o.impl_profile {
            Some(p) => Profile::parse(p)?,
            None => Profile::from_env()?.unwrap_or_default(),
        };
        imp.options = parse_impl_options(&o.impl_options, &imp)?;
        pipeline::run(config, cache, &imp, None, &opts)?
    } else if let Some(path) = imp_spec.strip_prefix("adapter:") {
        if o.impl_profile.is_some() {
            return Err(Error::Usage(
                "--impl-profile applies to dive only".to_owned(),
            ));
        }
        let mut adapter = ExternalAdapter::load(Path::new(path))?;
        adapter.options = parse_impl_options(&o.impl_options, &adapter)?;
        pipeline::run(config, cache, &adapter, None, &opts)?
    } else {
        return Err(Error::Usage(format!("unknown --impl `{imp_spec}`")));
    };
    let findings = print_and_gate(&report);
    let Some(base) = baseline else {
        return Ok(findings);
    };
    let diff = dive_conformance::baseline::compare(&base, &Baseline::from_report(&report))?;
    let rendered = diff.render(&base.source_run_id);
    if diff.regressions.is_empty() {
        out(&rendered);
    } else {
        eprintln_safe(rendered.trim_end());
    }
    Ok(findings || !diff.regressions.is_empty())
}

/// Prints the run's summary and returns whether its findings gate (exit 1).
fn print_and_gate(report: &dive_conformance::report::Report) -> bool {
    let s = &report.summary;
    out(&format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({
            "run_id": report.run_id,
            "models": s.models,
            "modules": s.modules,
            "oracle_drift_findings": s.oracle_drift_findings,
            "informational_drift_findings": s.informational_drift_findings,
            "worker_verdict_variant": s.worker_verdict_variant,
            "worker_counts_variant": s.worker_counts_variant,
            "worker_depth_variant": s.worker_depth_variant,
            "worker_property_variant": s.worker_property_variant,
            "by_corpus": s.by_corpus,
        }))
        .unwrap_or_default()
    ));
    let bad = |x: Outcome| {
        s.models.get(&x).copied().unwrap_or(0) + s.modules.get(&x).copied().unwrap_or(0)
    };
    let findings = bad(Outcome::Disagree) + bad(Outcome::OracleError) + bad(Outcome::Timeout);
    if report.reference {
        // A reference oracle never gates (ADR 0002).
        if findings > 0 {
            eprintln_safe(&format!(
                "reference run: {findings} finding(s) reported, not gating"
            ));
        }
        return false;
    }
    findings > 0
}

/// The default `shadow --sample`.
const SHADOW_SAMPLE: usize = 32;

/// `shadow`: returns whether there were findings.
fn cmd_shadow(config: &Config, cache: &Cache, root: &Path, o: &Opts) -> Result<bool> {
    if o.imp.is_some() || !o.impl_options.is_empty() {
        return Err(Error::Usage(
            "shadow runs dive's two engines; it takes no --impl or --impl-option".to_owned(),
        ));
    }
    let profile = match &o.impl_profile {
        Some(p) => Profile::parse(p)?,
        None => Profile::default(),
    };
    let golden_oracle = match &o.golden_oracle {
        Some(id) => id.clone(),
        None => config.oracles.default_oracle()?.id.clone(),
    };
    let bin = o.dive_bin.clone().unwrap_or_else(|| {
        root.join("target")
            .join("release")
            .join(format!("dive{}", std::env::consts::EXE_SUFFIX))
    });
    let opts = dive_conformance::shadow::ShadowOptions {
        corpora: if o.corpus.is_empty() {
            vec!["examples".to_owned()]
        } else {
            o.corpus.clone()
        },
        selection: selection(o),
        workers: o.workers.clone().unwrap_or_else(|| vec![1, 4]),
        limits: limits(o),
        report_dir: o.report_dir.clone(),
        strict: o.strict,
        sample: o.sample.unwrap_or(SHADOW_SAMPLE),
        profile,
    };
    let report = dive_conformance::shadow::run(config, cache, &golden_oracle, &bin, &opts)?;
    out(&format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({
            "run_id": report.run_id,
            "summary": report.summary,
        }))
        .unwrap_or_default()
    ));
    let findings: Vec<_> = report.findings().collect();
    for r in &findings {
        eprintln_safe(&format!(
            "shadow: {:?} {}:{} at {} worker(s): {}{}",
            r.status,
            r.corpus,
            r.model,
            r.workers,
            r.diffs.join("; "),
            r.detail
                .as_deref()
                .or(r.trace.as_deref())
                .map(|d| format!(" ({d})"))
                .unwrap_or_default()
        ));
    }
    Ok(!findings.is_empty())
}

/// The engine of `--impl dive` (`DIVE_ENGINE`, else the reference
/// engine) or `--impl dive:<engine>`; `None` for any other implementation.
fn dive_engine(imp_spec: &str) -> Result<Option<DiveEngine>> {
    Ok(match imp_spec.strip_prefix("dive:") {
        Some(engine) => Some(DiveEngine::parse(engine)?),
        None if imp_spec == "dive" => Some(DiveEngine::from_env()?.unwrap_or_default()),
        None => None,
    })
}

fn parse_impl_options(texts: &[String], imp: &dyn Implementation) -> Result<Vec<ImplOption>> {
    texts.iter().map(|t| ImplOption::parse(t, imp)).collect()
}

fn eprintln_safe(s: &str) {
    let _ = writeln!(std::io::stderr().lock(), "{s}");
}

/// `dev capture --oracle <id> --specs <dir> --out <dir> [--seed N]
/// [--lncheck MODE] [--workers N] [--fp N] [--feature F]... [NAME...]`:
/// runs the oracle exactly as the harness does on `<dir>/<NAME>/<NAME>.tla`
/// (+`.cfg`) and records stdout, stderr, exit code and the JSON trace as test
/// fixtures, plus SANY's output on the module. `--seed` replaces the harness's
/// `-seed 0` (for randomization fixtures), `--lncheck` its `-lncheck final`
/// `--workers N` (one count) its `-workers 1` and `--fp N` its `-fp 0` (for
/// liveness probes). `--feature F` gives the specs an Examples manifest
/// feature, as golden generation would (`action composition` adds
/// `-Dtlc2.tool.impl.Tool.cdot=true`). A
/// multi-worker capture records no JSON trace: the trace pass runs only at
/// one worker.
/// The transcript is the clean run's (no `-dumpTrace`); a counterexample's
/// JSON comes from the cross-checked trace pass, run with the same seed,
/// whose command is recorded in `.trace.command`.
fn dev_capture(config: &Config, cache: &Cache, o: &Opts) -> Result<()> {
    let specs = o
        .specs
        .clone()
        .ok_or_else(|| Error::Usage("--specs <dir> needed".to_owned()))?;
    let out_dir = o
        .out
        .clone()
        .ok_or_else(|| Error::Usage("--out <dir> needed".to_owned()))?;
    let oracle = o
        .oracle
        .clone()
        .ok_or_else(|| Error::Usage("--oracle <id> needed".to_owned()))?;
    let specs = specs.canonicalize().map_err(|e| Error::io(&specs, &e))?;
    let java = Java::resolve(config, cache, o.java.as_deref())?;
    let env = OracleEnv::prepare(config, cache, &oracle, &java)?;
    let dir = out_dir.join(&oracle);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, &e))?;
    let lim = limits(o);
    let workers = match o.workers.as_deref() {
        None => 1,
        Some([n]) => *n,
        Some(_) => {
            return Err(Error::Usage(
                "dev capture takes one --workers count".to_owned(),
            ));
        }
    };
    let lncheck = o.lncheck.as_deref().unwrap_or("final");
    for name in &o.positional {
        let module = format!("{name}/{name}.tla");
        let config_path = format!("{name}/{name}.cfg");
        let job = TlcJob {
            corpus_root: &specs,
            module: &module,
            config: &config_path,
            mode: CheckMode::Exhaustive,
            features: &o.features,
            workers,
            timeout: Duration::from_secs(300),
            xmx: &lim.xmx,
            seed: o.seed.unwrap_or(0),
            lncheck,
            fp: o.fp.unwrap_or(0),
            dump_trace: false,
        };
        let inv = env.run_tlc(&job)?;
        // The transcript is the clean run's; a counterexample's JSON comes
        // from the cross-checked trace pass, exactly as for goldens (#35).
        let (result, traced) = classify_with_trace_pass(&env, &job, &inv)?;
        let write = |suffix: &str, text: &str| {
            let p = dir.join(format!("{name}.{suffix}"));
            std::fs::write(&p, text).map_err(|e| Error::io(&p, &e))
        };
        write("stdout", &inv.outcome.stdout)?;
        write("stderr", &inv.outcome.stderr)?;
        write(
            "exit",
            &inv.outcome
                .exit_code
                .map_or_else(|| "none\n".to_owned(), |c| format!("{c}\n")),
        )?;
        // A recapture must not leave an earlier run's trace files behind.
        for suffix in ["trace.json", "trace.command"] {
            let p = dir.join(format!("{name}.{suffix}"));
            if p.exists() {
                std::fs::remove_file(&p).map_err(|e| Error::io(&p, &e))?;
            }
        }
        let json = result.trace.as_ref().is_some_and(|t| t.json.is_some());
        if let Some(t) = traced.as_ref().and_then(|t| t.trace_json.as_ref())
            && json
        {
            write("trace.json", t)?;
        }
        write("command", &(inv.command.join(" ") + "\n"))?;
        if let Some(t) = &traced {
            write("trace.command", &(t.command.join(" ") + "\n"))?;
        }
        let sany_inv = env.run_sany(
            &specs,
            &format!("{name}/{name}.tla"),
            Duration::from_secs(120),
        )?;
        let sany_result = sany::classify(&sany_inv.outcome);
        write("sany.stdout", &sany_inv.outcome.stdout)?;
        write("sany.stderr", &sany_inv.outcome.stderr)?;
        write(
            "sany.exit",
            &sany_inv
                .outcome
                .exit_code
                .map_or_else(|| "none\n".to_owned(), |c| format!("{c}\n")),
        )?;
        eprintln_safe(&format!(
            "{oracle} {name}: TLC exit {:?} -> {} ({}); SANY exit {:?} -> {:?}",
            result.exit_code,
            result.verdict.as_str(),
            result.error_class.as_deref().unwrap_or("-"),
            sany_result.exit_code,
            sany_result.verdict
        ));
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut args = Args {
        items: std::env::args().skip(1).collect(),
        pos: 0,
    };
    if args.items.is_empty()
        || args
            .items
            .iter()
            .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        out(USAGE);
        return ExitCode::SUCCESS;
    }
    if args.items.iter().any(|a| a == "--version" || a == "-V") {
        out(&format!("{}\n", pipeline::generator()));
        return ExitCode::SUCCESS;
    }
    // The command is one word (`run`) or two (`oracle fetch`); any further
    // positionals are arguments (only `dev capture` takes them).
    let result = dive_conformance::process::install_signal_handler()
        .and_then(|()| parse_opts(&mut args))
        .and_then(|mut o| {
            let n = if matches!(
                o.positional.first().map(String::as_str),
                Some("run" | "shadow" | "compare-models")
            ) {
                1
            } else {
                2
            };
            let rest = o.positional.split_off(n.min(o.positional.len()));
            let cmd = std::mem::replace(&mut o.positional, rest);
            let takes_args = matches!(cmd.first().map(String::as_str), Some("dev" | "report"));
            if !takes_args && !o.positional.is_empty() {
                return Err(Error::Usage(format!(
                    "unexpected argument(s) {:?} after `{}`",
                    o.positional,
                    cmd.join(" ")
                )));
            }
            dispatch(&cmd, &o)
        });
    match result {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::from(1),
        Err(e) => {
            eprintln_safe(&format!("error: {e}"));
            ExitCode::from(if matches!(e, Error::Usage(_)) { 2 } else { 3 })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_invocation_option_is_off_by_default_and_scoped_to_generation() -> Result<()> {
        let mut args = Args {
            items: Vec::new(),
            pos: 0,
        };
        assert!(parse_opts(&mut args)?.raw_invocations.is_none());
        args = Args {
            items: vec!["--raw-invocations".into(), "external-raw".into()],
            pos: 0,
        };
        let options = parse_opts(&mut args)?;
        assert_eq!(options.raw_invocations, Some(PathBuf::from("external-raw")));
        assert!(matches!(
            dispatch(&["run".into()], &options),
            Err(Error::Usage(_))
        ));
        Ok(())
    }
    #[test]
    fn setup_status_requires_raw_capture_and_defaults_off() -> Result<()> {
        let mut args = Args {
            items: Vec::new(),
            pos: 0,
        };
        assert!(!parse_opts(&mut args)?.raw_setup_status);
        args = Args {
            items: vec!["--raw-setup-status".into()],
            pos: 0,
        };
        let options = parse_opts(&mut args)?;
        assert!(options.raw_setup_status);
        assert!(matches!(
            dispatch(&["golden".into(), "generate".into()], &options),
            Err(Error::Usage(_))
        ));
        args = Args {
            items: vec![
                "--raw-setup-status".into(),
                "--raw-invocations".into(),
                "raw".into(),
            ],
            pos: 0,
        };
        let options = parse_opts(&mut args)?;
        assert!(options.raw_setup_status);
        assert_eq!(options.raw_invocations, Some(PathBuf::from("raw")));
        assert!(matches!(
            dispatch(&["run".into()], &options),
            Err(Error::Usage(_))
        ));
        Ok(())
    }
}
