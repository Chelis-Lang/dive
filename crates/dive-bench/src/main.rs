//! `dive-bench`: times `dive check` (and optionally TLC) on the benchmark set.
//! See the library documentation and `--help`.
#![forbid(unsafe_code)]

mod stats;

use serde::Serialize;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use dive_bench::{Bench, Usage, median, parse_time_output, range, time_args};
use dive_conformance::cache::Cache;
use dive_conformance::config::Config;
use dive_conformance::oracle::{CheckMode, Java, OracleEnv, OracleProvenance, TlcJob};
use dive_conformance::process::run_with_timeout;
use dive_conformance::tlc::{
    messages,
    result::{RunFacts, classify},
};
use dive_conformance::util::TempDir;
use serde_json::{Value, json};

const USAGE: &str = "\
dive-bench: time `dive check` on the ten-model benchmark set

usage: dive-bench [options]

  --dive-bin <path>   the dive binary (default: target/release/dive)
  --dive-revision <id> source label for the candidate (unknown if omitted)
  --baseline-bin <p>  optional frozen previous dive binary
  --baseline-revision <id> required source label when using --baseline-bin
  --summary <file>    write compact current-run summaries as JSON
  --reps <n>          interleaved repetitions (default: 3)
  --profile <p>       tlc, default or both (default: both)
  --filter <name>     run only this benchmark (repeatable)
  --workers <list>    matched worker counts, e.g. 1,4,8 (default: 1)
  --tlc               also time pinned TLC at every selected worker count
  --xmx <size>        TLC heap (default: 3g, as the harness)
  --timeout <secs>    per run (default: 1800)
  --runs <file>       append every run as a JSON line to <file>
  --set <file>        the benchmark set (default: crates/dive-bench/benchmarks.toml)
  --list              list the benchmark set and exit

Needs `DIVE_CACHE_DIR` (or ~/.cache/dive) with the oracles and the Examples
corpus fetched (`dive-conformance oracle fetch`, `corpus fetch`), and
/usr/bin/time. Every run's verdict and counts are checked against the TLC
golden; a mismatch is reported and makes the exit status 1.
";

struct Opts {
    dive_bin: PathBuf,
    dive_revision: Option<String>,
    baseline_bin: Option<PathBuf>,
    baseline_revision: Option<String>,
    summary: Option<PathBuf>,
    reps: usize,
    profiles: Vec<&'static str>,
    workers: Vec<usize>,
    filters: Vec<String>,
    tlc: bool,
    xmx: String,
    timeout: Duration,
    runs: Option<PathBuf>,
    set: Option<PathBuf>,
    list: bool,
}

fn parse_args() -> Result<Opts, String> {
    let mut o = Opts {
        dive_bin: PathBuf::from("target/release/dive"),
        dive_revision: None,
        baseline_bin: None,
        baseline_revision: None,
        summary: None,
        reps: 3,
        profiles: vec!["tlc", "default"],
        workers: vec![1],
        filters: Vec::new(),
        tlc: false,
        xmx: "3g".to_owned(),
        timeout: Duration::from_mins(30),
        runs: None,
        set: None,
        list: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{a} expects a value"));
        match a.as_str() {
            "--dive-bin" => o.dive_bin = val()?.into(),
            "--dive-revision" => o.dive_revision = Some(val()?),
            "--baseline-bin" => o.baseline_bin = Some(val()?.into()),
            "--baseline-revision" => o.baseline_revision = Some(val()?),
            "--summary" => o.summary = Some(val()?.into()),
            "--reps" => o.reps = val()?.parse().map_err(|_| "--reps expects a number")?,
            "--profile" => {
                o.profiles = match val()?.as_str() {
                    "tlc" => vec!["tlc"],
                    "default" => vec!["default"],
                    "both" => vec!["tlc", "default"],
                    p => return Err(format!("--profile: unknown profile `{p}`")),
                }
            }
            "--filter" => o.filters.push(val()?),
            "--workers" => {
                o.workers = val()?
                    .split(',')
                    .map(|w| {
                        w.trim()
                            .parse::<usize>()
                            .ok()
                            .filter(|&n| n > 0 && u32::try_from(n).is_ok())
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or("--workers expects positive numbers, e.g. 1,2,4,8")?;
            }
            "--tlc" => o.tlc = true,
            "--xmx" => o.xmx = val()?,
            "--timeout" => {
                o.timeout =
                    Duration::from_secs(val()?.parse().map_err(|_| "--timeout expects seconds")?);
            }
            "--runs" => o.runs = Some(val()?.into()),
            "--set" => o.set = Some(val()?.into()),
            "--list" => o.list = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    if o.workers
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != o.workers.len()
    {
        return Err("--workers contains duplicates".to_owned());
    }
    if o.timeout.is_zero() {
        return Err("--timeout must be positive".to_owned());
    }
    if o.baseline_bin.is_some() != o.baseline_revision.is_some() {
        return Err("--baseline-bin and --baseline-revision must be supplied together".to_owned());
    }
    if o.reps == 0 {
        return Err("--reps must be positive".to_owned());
    }
    Ok(o)
}

/// What one timed run measured.
#[derive(Clone)]
struct Run {
    bench: String,
    implementation: String,
    rep: usize,
    usage: Option<Usage>,
    wall_s: f64,
    verdict: String,
    distinct: Option<u64>,
    generated: Option<u64>,
    correct: bool,
    note: String,
    workers: usize,
    used_workers: Option<usize>,
    sequential_reason: Option<String>,
    depth: Option<u64>,
    error_class: Option<String>,
    exit_code: Option<i32>,
    property: Option<String>,
    metadata: Value,
}

/// The golden facts a run must reproduce.
#[derive(Serialize)]
struct Expected {
    verdict: String,
    distinct: Option<u64>,
    generated: Option<u64>,
    depth: Option<u64>,
    error_class: Option<String>,
    exit_code: Option<i32>,
    property: Option<String>,
}

/// Where a benchmark runs: a corpus root, the module and the cfg relative to
/// it (a scratch copy when the configuration is replaced).
struct Prepared {
    bench: Bench,
    corpus_root: PathBuf,
    module: String,
    expected: Expected,
    provenance: Value,
    _scratch: Option<TempDir>,
}

fn main() -> ExitCode {
    #[cfg(unix)]
    if let Err(e) = dive_conformance::process::install_signal_handler() {
        eprintln!("error: {e}");
        return ExitCode::from(2);
    }
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) if e.is_empty() => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&opts) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[allow(clippy::too_many_lines, reason = "the run loop, kept in one place")]
fn run(opts: &Opts) -> Result<bool, String> {
    let cwd = std::env::current_dir().map_err(err)?;
    let repo = Config::find_repo_root(&cwd).map_err(err)?;
    let config = Config::load(&repo).map_err(err)?;
    let set_path = opts
        .set
        .clone()
        .unwrap_or_else(|| repo.join("crates/dive-bench/benchmarks.toml"));
    let set = dive_bench::load_benchmarks(
        &std::fs::read_to_string(&set_path).map_err(|e| format!("{}: {e}", set_path.display()))?,
    )?;
    let set: Vec<Bench> = set
        .into_iter()
        .filter(|b| opts.filters.is_empty() || opts.filters.iter().any(|f| f == &b.name))
        .collect();
    if opts.list {
        for b in &set {
            println!("{:<20} {}  ({})", b.name, b.model, b.why);
        }
        return Ok(true);
    }
    if set.is_empty() {
        return Err("no benchmark matches --filter".to_owned());
    }
    let cache = Cache::from_env().map_err(err)?;
    let oracle = config.oracles.default_oracle().map_err(err)?.id.clone();
    let corpus = config.corpora.corpus("examples").map_err(err)?;
    let corpus_root = cache.checkout_dir(&corpus.id, &corpus.commit);
    dive_conformance::fetch::verified_checkout(&corpus_root, &corpus.commit, "Examples")
        .map_err(err)?;
    let module_path =
        dive_conformance::dive::module_search_path(&config, &cache, &oracle).map_err(err)?;
    let dive_bin = if opts.dive_bin.is_absolute() {
        opts.dive_bin.clone()
    } else {
        repo.join(&opts.dive_bin)
    };
    if !dive_bin.is_file() {
        return Err(format!(
            "{} does not exist; build it with `cargo build --release -p dive-cli`",
            dive_bin.display()
        ));
    }
    let scratch = TempDir::new_in(&cache.tmp_dir(), "bench").map_err(err)?;
    let golden_root = dive_conformance::golden::golden_root(&repo);

    let mut prepared = Vec::new();
    for b in &set {
        let gpath = dive_conformance::golden::model_golden_path(
            &golden_root,
            &oracle,
            "examples",
            &b.model,
        );
        let g = dive_conformance::golden::read_model_golden(&gpath).map_err(err)?;
        validate_oracle_pins(&g.provenance.oracle, &config, &oracle)
            .map_err(|e| format!("{}: {e}", gpath.display()))?;
        if g.oracle != oracle
            || g.corpus != "examples"
            || g.model.id != b.model
            || g.workers != 1
            || g.provenance.corpus_commit != corpus.commit
            || g.result.verdict != dive_conformance::tlc::result::Verdict::Success
            || g.result.counts.distinct_states.is_none()
            || g.result.counts.states_generated.is_none()
            || g.result.counts.depth.is_none()
            || g.result.error_class.as_deref() != Some("TLC_SUCCESS")
            || g.result.exit_code != Some(0)
        {
            return Err(format!(
                "{}: benchmark requires a successful, counted golden matching the pins",
                gpath.display()
            ));
        }
        let module = g.model.module.clone();
        let expected = Expected {
            verdict: g.result.verdict.as_str().to_owned(),
            distinct: g.result.counts.distinct_states,
            generated: g.result.counts.states_generated,
            depth: g.result.counts.depth,
            error_class: g.result.error_class.clone(),
            exit_code: g.result.exit_code,
            property: g.result.violated_property.clone(),
        };
        let (root, own) = if let Some(cfg) = &b.cfg {
            let dir = TempDir::new_in(scratch.path(), "corpus").map_err(err)?;
            let rel = Path::new(&module)
                .parent()
                .ok_or("module without directory")?;
            let src = corpus_root.join(rel);
            let dst = dir.path().join(rel);
            std::fs::create_dir_all(&dst).map_err(err)?;
            for e in std::fs::read_dir(&src).map_err(err)? {
                let e = e.map_err(err)?;
                if e.file_type().map_err(err)?.is_file() {
                    std::fs::copy(e.path(), dst.join(e.file_name())).map_err(err)?;
                }
            }
            std::fs::write(dir.path().join(&b.model), cfg).map_err(err)?;
            (dir.path().to_path_buf(), Some(dir))
        } else {
            (corpus_root.clone(), None)
        };
        let provenance = json!({
            "model": b.model, "module": module,
            "effective_cfg_sha256": dive_conformance::hash::sha256_file(&root.join(&b.model)).map_err(err)?,
            "module_sha256": dive_conformance::hash::sha256_file(&root.join(&module)).map_err(err)?,
            "golden_sha256": dive_conformance::hash::sha256_file(&gpath).map_err(err)?,
            "cfg_scope": if b.cfg.is_some() { "safety variant; no original-property parity credit" } else { "original pinned configuration" },
            "source_files": source_hashes(&root.join(&module))?,
            "scaling_population_at_least_400000_distinct": expected.distinct.is_some_and(|n| n >= 400_000),
        });
        prepared.push(Prepared {
            bench: b.clone(),
            corpus_root: root,
            module,
            expected,
            provenance,
            _scratch: own,
        });
    }

    let baseline_bin = opts.baseline_bin.as_ref().map(|p| {
        if p.is_absolute() {
            p.clone()
        } else {
            repo.join(p)
        }
    });
    if baseline_bin.as_ref().is_some_and(|p| !p.is_file()) {
        return Err("baseline binary does not exist".to_owned());
    }
    let time_out = scratch.path().join("time.txt");
    let tlc_env = if opts.tlc {
        let real = Java::resolve(&config, &cache, None).map_err(err)?;
        let wrapper = scratch.path().join("java-timed");
        let mut script = String::from("#!/bin/sh\nexec /usr/bin/time");
        for a in time_args(&time_out) {
            let _ = write!(script, " {}", shell_quote(&a.to_string_lossy()));
        }
        let _ = writeln!(
            script,
            " {} \"$@\"",
            shell_quote(&real.path.to_string_lossy())
        );
        std::fs::write(&wrapper, script).map_err(err)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
                .map_err(err)?;
        }
        let java = Java::resolve(&config, &cache, Some(&wrapper)).map_err(err)?;
        let mut env = OracleEnv::prepare(&config, &cache, &oracle, &java).map_err(err)?;
        // Timing wrapper changes only execution; provenance identifies the pinned runtime.
        env.java.source = real.source;
        env.java.version_line = real.version_line;
        Some(env)
    } else {
        None
    };

    let impls = implementations(opts);
    let mut provenance = print_machine(&dive_bin);
    provenance["campaign_id"] = json!(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(err)?
            .as_nanos()
    ));
    provenance["binary_sha256"] =
        json!(dive_conformance::hash::sha256_file(&dive_bin).map_err(err)?);
    provenance["oracle"] =
        json!({"id":oracle, "sha256":config.oracles.oracle(&oracle).map_err(err)?.sha256});
    provenance["corpus_commit"] = json!(corpus.commit);
    provenance["module_path"] = json!(module_path);
    provenance["oracle_runtime"] = json!(tlc_env.as_ref().map(OracleEnv::provenance));
    provenance["candidate"] = binary_identity(&dive_bin, opts.dive_revision.as_deref())?;
    provenance["baseline"] = json!(
        baseline_bin
            .as_ref()
            .map(|p| binary_identity(p, opts.baseline_revision.as_deref()))
            .transpose()?
    );
    provenance["benchmark_set_sha256"] =
        json!(dive_conformance::hash::sha256_file(&set_path).map_err(err)?);
    provenance["timeout_seconds"] = json!(opts.timeout.as_secs());
    provenance["xmx"] = json!(opts.xmx);
    provenance["schedule"] = json!(
        "model order fixed; worker/arm blocks reversed on odd repetitions; one process at a time"
    );
    provenance["seed"] = json!(0);
    provenance["fp"] = json!(0);
    println!("provenance: {provenance}");
    let mut runs: Vec<Run> = Vec::new();
    let mut all_correct = true;
    for rep in 0..opts.reps {
        for p in &prepared {
            for (ordinal, imp) in schedule(&impls, rep).into_iter().enumerate() {
                let mut r = if imp.starts_with("tlc-w") {
                    let env = tlc_env.as_ref().ok_or("no TLC environment")?;
                    run_tlc(env, p, imp, rep, &time_out, opts)?
                } else {
                    let bin = if imp.starts_with("baseline-") {
                        baseline_bin.as_ref().ok_or("missing baseline binary")?
                    } else {
                        &dive_bin
                    };
                    run_dive(bin, &module_path, p, imp, rep, &time_out, opts)?
                };
                r.metadata["schedule_ordinal_within_model"] = json!(ordinal);
                all_correct &= r.correct;
                eprintln!(
                    "[rep {}] {:<20} {:<12} cpu {:>8} wall {:>8.2}s rss {:>6} {} {}",
                    rep + 1,
                    r.bench,
                    r.implementation,
                    r.usage
                        .map_or("?".to_owned(), |u| format!("{:.2}s", u.cpu_s())),
                    r.wall_s,
                    r.usage
                        .map_or("?".to_owned(), |u| format!("{}MB", u.max_rss_bytes >> 20)),
                    if r.correct { "ok" } else { "WRONG" },
                    r.note
                );
                if let Some(path) = &opts.runs {
                    append_json(path, &r, &provenance)?;
                }
                runs.push(r);
            }
        }
    }
    stats::validate(&runs)?;
    let summary = report(&prepared, &impls, &runs, opts)?;
    if let Some(path) = &opts.summary {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(
                &json!({"schema_version":2, "provenance":provenance, "summary":summary}),
            )
            .map_err(err)?,
        )
        .map_err(err)?;
    }

    Ok(all_correct)
}

/// Compare semantic inputs, preserving their resolution order. Historical Java,
/// capabilities and physical paths are recorded context, not pin equality.
fn validate_oracle_pins(
    recorded: &OracleProvenance,
    config: &Config,
    oracle: &str,
) -> Result<(), String> {
    let expected = config.oracles.oracle(oracle).map_err(err)?;
    if recorded.id != expected.id
        || recorded.upstream_commit != expected.upstream_commit
        || recorded.sha256 != expected.sha256
    {
        return Err(
            "golden oracle id, upstream commit or jar SHA does not match the pins".to_owned(),
        );
    }
    let classpath = expected
        .classpath
        .iter()
        .map(|id| {
            config
                .oracles
                .classpath_artifact(id)
                .map(|artifact| (artifact.id.as_str(), artifact.sha256.as_str()))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    if !recorded
        .classpath
        .iter()
        .map(|entry| (entry.id.as_str(), entry.pin.as_str()))
        .eq(classpath)
    {
        return Err("golden ordered classpath ids/SHA pins do not match the pins".to_owned());
    }
    if !recorded
        .module_libraries
        .iter()
        .map(|entry| (entry.id.as_str(), entry.pin.as_str()))
        .eq(config
            .oracles
            .module_library
            .iter()
            .map(|library| (library.id.as_str(), library.commit.as_str())))
    {
        return Err("golden ordered module-library ids/commits do not match the pins".to_owned());
    }
    Ok(())
}

fn print_machine(dive_bin: &Path) -> Value {
    let capture = |prog: &str, args: &[&str]| {
        Command::new(prog)
            .args(args)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let cpu = if cfg!(target_os = "macos") {
        capture("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else {
        capture(
            "sh",
            &["-c", "grep -m1 'model name' /proc/cpuinfo | cut -d: -f2"],
        )
    };
    let load = if cfg!(target_os = "macos") {
        capture("sysctl", &["-n", "vm.loadavg"])
    } else {
        capture("cat", &["/proc/loadavg"])
    };
    let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let version = capture(&dive_bin.to_string_lossy(), &["--version"]);
    println!(
        "machine: {cpu}, {cpus} CPUs, {}",
        capture("uname", &["-sm"])
    );
    println!("load average at start: {load}");
    println!("dive: {version} ({})", dive_bin.display());
    println!();
    json!({"cpu":cpu, "cpus":cpus, "os":capture("uname", &["-sm"]),
        "load_at_start":load, "binary_version":version,
        "runner_revision":capture("git", &["rev-parse", "HEAD"]),
        "runner_dirty": !capture("git", &["status", "--porcelain"]).is_empty(),
        "memory":capture("sh", &["-c", "head -n 3 /proc/meminfo 2>/dev/null"]),
        "cpu_topology":capture("lscpu", &["-p=CPU,CORE,SOCKET,ONLINE"]),
        "affinity":capture("sh", &["-c", "sed -n '/Cpus_allowed_list/p' /proc/self/status 2>/dev/null"]),
        "rustc":capture("rustc", &["--version"]),
        "quiet_window_certified":false})
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn read_usage(path: &Path) -> Option<Usage> {
    let u = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| parse_time_output(&t));
    let _ = std::fs::remove_file(path);
    u
}

fn check(p: &Prepared, r: &Run) -> (bool, String) {
    let e = &p.expected;
    let mut bad = Vec::new();
    if r.verdict != e.verdict {
        bad.push(format!("verdict {} (golden {})", r.verdict, e.verdict));
    }
    for (name, actual, expected) in [
        ("distinct", r.distinct, e.distinct),
        ("depth", r.depth, e.depth),
    ] {
        if actual != expected {
            bad.push(format!("{name} {actual:?} (golden {expected:?})"));
        }
    }
    if r.workers == 1 && r.generated != e.generated {
        bad.push(format!(
            "generated {:?} (golden {:?})",
            r.generated, e.generated
        ));
    }
    if r.error_class != e.error_class {
        bad.push(format!(
            "error class {:?} (golden {:?})",
            r.error_class, e.error_class
        ));
    }
    if r.exit_code != e.exit_code {
        bad.push(format!("exit {:?} (golden {:?})", r.exit_code, e.exit_code));
    }
    if r.property != e.property {
        bad.push(format!(
            "property {:?} (golden {:?})",
            r.property, e.property
        ));
    }
    if !stats::positive(r.wall_s) {
        bad.push("nonpositive/nonfinite wall time".to_owned());
    }
    (bad.is_empty(), bad.join("; "))
}

fn run_dive(
    bin: &Path,
    module_path: &[dive_conformance::oracle::ClasspathEntry],
    p: &Prepared,
    imp: &str,
    rep: usize,
    time_out: &Path,
    opts: &Opts,
) -> Result<Run, String> {
    let module = p.corpus_root.join(&p.module);
    let dir = module.parent().ok_or("module without directory")?;
    let file = module.file_name().ok_or("module without file name")?;
    let mut args: Vec<OsString> = time_args(time_out);
    args.push(bin.as_os_str().to_owned());
    let (profile, workers) = parse_impl(imp);
    for a in ["check", "--output", "json", "--seed", "0", "--fp", "0"] {
        args.push(a.into());
    }
    args.push("--workers".into());
    args.push(workers.to_string().into());
    if profile == "tlc" {
        args.push("--compat=tlc".into());
    }
    args.push("--config".into());
    args.push(p.corpus_root.join(&p.bench.model).into_os_string());
    for m in module_path {
        args.push("--module-path".into());
        args.push(m.path.clone().into_os_string());
    }
    args.push(file.to_owned());
    let mut cmd = Command::new("/usr/bin/time");
    cmd.current_dir(dir)
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_CHAOS")
        .args(&args);
    let out_dir = TempDir::new_in(&std::env::temp_dir(), "dive-bench-run").map_err(err)?;
    let _ = std::fs::remove_file(time_out);
    let load_before = load_average();
    let invocation_args: Vec<String> = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let outcome = run_with_timeout(&mut cmd, opts.timeout, out_dir.path()).map_err(err)?;
    let load_after = load_average();
    let usage = read_usage(time_out);
    let v: Value = serde_json::from_str(outcome.stdout.trim()).unwrap_or(Value::Null);
    let verdict = if outcome.timed_out {
        "timeout".to_owned()
    } else {
        v["verdict"].as_str().unwrap_or("no-report").to_owned()
    };
    let distinct = v["statistics"]["distinct"].as_u64();
    let generated = v["statistics"]["generated"].as_u64();
    let mut r = Run {
        bench: p.bench.name.clone(),
        implementation: imp.to_owned(),
        rep,
        usage,
        wall_s: outcome.elapsed.as_secs_f64(),
        verdict,
        distinct,
        generated,
        correct: false,
        note: String::new(),
        workers,
        used_workers: v["options"]["workers"]["used"]
            .as_u64()
            .and_then(|w| usize::try_from(w).ok()),
        sequential_reason: v["options"]["workers"]["reason"]
            .as_str()
            .map(str::to_owned),
        depth: v["statistics"]["depth"].as_u64(),
        error_class: v["tlc"]["name"].as_str().map(str::to_owned),
        exit_code: outcome.exit_code,
        property: v["property"].as_str().map(str::to_owned),
        metadata: json!({"argv":invocation_args,"cwd":dir,"environment_removed":["DIVE_COMPAT","DIVE_CHAOS"],"reported_options":v["options"],"load_before":load_before,"load_after":load_after,"inputs":p.provenance}),
    };
    (r.correct, r.note) = check(p, &r);
    if v["tlc"]["verified"] != true
        || v["tlc"]["code"].as_i64() != Some(2193)
        || v["tlc"]["exit_status"].as_i64() != Some(0)
    {
        r.correct = false;
        r.note
            .push_str("; dive did not report verified TLC success");
    }
    Ok(r)
}

fn run_tlc(
    env: &OracleEnv,
    p: &Prepared,
    imp: &str,
    rep: usize,
    time_out: &Path,
    opts: &Opts,
) -> Result<Run, String> {
    let (_, workers) = parse_impl(imp);
    let job = TlcJob {
        corpus_root: &p.corpus_root,
        module: &p.module,
        config: &p.bench.model,
        mode: CheckMode::Exhaustive,
        features: &[],
        workers: u32::try_from(workers).map_err(|_| "worker count exceeds TLC limit")?,
        timeout: opts.timeout,
        xmx: &opts.xmx,
        seed: 0,
        fp: 0,
        lncheck: "final",
        dump_trace: false,
    };
    let _ = std::fs::remove_file(time_out);
    let load_before = load_average();
    let inv = env.run_tlc(&job).map_err(err)?;
    let load_after = load_average();
    let usage = read_usage(time_out);
    let result = classify(
        &messages::parse(&inv.outcome.stdout),
        &RunFacts {
            exit_code: inv.outcome.exit_code,
            timed_out: inv.outcome.timed_out,
            trace_json: None,
            simulate: false,
            stderr: Some(&inv.outcome.stderr),
        },
    );
    let mut r = Run {
        bench: p.bench.name.clone(),
        implementation: imp.to_owned(),
        rep,
        usage,
        wall_s: inv.outcome.elapsed.as_secs_f64(),
        verdict: result.verdict.as_str().to_owned(),
        distinct: result.counts.distinct_states,
        generated: result.counts.states_generated,
        depth: result.counts.depth,
        error_class: result.error_class,
        exit_code: result.exit_code,
        property: result.violated_property,
        workers,
        used_workers: tlc_workers(&inv.outcome.stdout),
        sequential_reason: None,
        correct: false,
        note: String::new(),
        metadata: json!({"argv":inv.command,"cwd":"$TMP/work","workers_used_source":"TLC BFS startup message, not inferred from request","load_before":load_before,"load_after":load_after,"inputs":p.provenance}),
    };
    (r.correct, r.note) = check(p, &r);
    Ok(r)
}

fn tlc_workers(stdout: &str) -> Option<usize> {
    let observed: Vec<usize> = stdout
        .lines()
        .filter(|l| l.starts_with("Running breadth-first search Model-Checking "))
        .filter_map(|l| {
            l.split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .find(|w| matches!(w[1], "worker" | "workers"))
                .and_then(|w| w[0].parse().ok())
        })
        .collect();
    match observed.as_slice() {
        [workers] if *workers > 0 => Some(*workers),
        _ => None,
    }
}

fn append_json(path: &Path, r: &Run, provenance: &Value) -> Result<(), String> {
    use std::io::Write as _;
    let line = json!({
        "schema_version": 2,
        "provenance": provenance,
        "bench": r.bench,
        "impl": r.implementation,
        "rep": r.rep + 1,
        "user_s": r.usage.map(|u| u.user_s),
        "sys_s": r.usage.map(|u| u.sys_s),
        "max_rss_bytes": r.usage.map(|u| u.max_rss_bytes),
        "wall_s": r.wall_s,
        "verdict": r.verdict,
        "distinct": r.distinct,
        "generated": r.generated,
        "generated_parity_required": r.workers == 1,
        "correct": r.correct,
        "note": r.note,
        "workers_requested": r.workers, "workers_used": r.used_workers,
        "sequential_reason": r.sequential_reason, "depth":r.depth,
        "error_class":r.error_class,"exit_code":r.exit_code,"property":r.property,
        "run":r.metadata,
    });
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(f, "{line}").map_err(err)
}

#[derive(Serialize)]
struct Summary {
    cpu_s: Option<f64>,
    cpu_range_s: Option<(f64, f64)>,
    wall_s: Option<f64>,
    wall_range_s: Option<(f64, f64)>,
    rss_mb: Option<f64>,
    generated_per_wall_s: Option<f64>,
    completed: usize,
    intended: usize,
    missing: usize,
    censored: usize,
    refused: usize,
    failed: usize,
    workers_used: Vec<Option<usize>>,
    sequential_reasons: Vec<String>,
}

fn summarise(runs: &[&Run], intended: usize) -> Summary {
    let valid = |r: &&&Run| r.correct && stats::positive(r.wall_s);
    let cpu: Vec<f64> = runs
        .iter()
        .filter(valid)
        .filter_map(|r| r.usage.map(|u| u.cpu_s()))
        .filter(|&s| stats::positive(s))
        .collect();
    let wall: Vec<f64> = runs.iter().filter(valid).map(|r| r.wall_s).collect();
    #[allow(clippy::cast_precision_loss, reason = "megabytes for display")]
    let rss: Vec<f64> = runs
        .iter()
        .filter(valid)
        .filter_map(|r| r.usage.map(|u| u.max_rss_bytes as f64 / f64::from(1 << 20)))
        .collect();
    let censored = runs.iter().filter(|r| r.verdict == "timeout").count();
    let refused = runs.iter().filter(|r| r.verdict == "unsupported").count();
    Summary {
        cpu_s: median(&cpu),
        cpu_range_s: range(&cpu),
        wall_s: median(&wall),
        wall_range_s: range(&wall),
        rss_mb: median(&rss),
        generated_per_wall_s: median(
            &runs
                .iter()
                .filter(valid)
                .filter_map(|r| rate(r.generated, Some(r.wall_s)))
                .collect::<Vec<_>>(),
        ),
        completed: wall.len(),
        intended,
        missing: intended.saturating_sub(runs.len()),
        censored,
        refused,
        failed: runs.len().saturating_sub(wall.len() + censored + refused),
        workers_used: runs.iter().map(|r| r.used_workers).collect(),
        sequential_reasons: runs
            .iter()
            .filter_map(|r| r.sequential_reason.clone())
            .collect(),
    }
}

fn dive_impl(profile: &str, workers: usize) -> String {
    format!("dive-{profile}-w{workers}")
}
fn tlc_impl(workers: usize) -> String {
    format!("tlc-w{workers}")
}
fn baseline_impl(profile: &str, workers: usize) -> String {
    format!("baseline-{profile}-w{workers}")
}
fn parse_impl(imp: &str) -> (&str, usize) {
    let rest = imp
        .strip_prefix("dive-")
        .or_else(|| imp.strip_prefix("baseline-"))
        .unwrap_or(imp);
    rest.rsplit_once("-w")
        .map_or((rest, 1), |(p, w)| (p, w.parse().unwrap_or(1)))
}

fn implementations(opts: &Opts) -> Vec<String> {
    let mut arms = Vec::new();
    for &w in &opts.workers {
        if opts.tlc {
            arms.push(tlc_impl(w));
        }
        for p in &opts.profiles {
            if opts.baseline_bin.is_some() {
                arms.push(baseline_impl(p, w));
            }
            arms.push(dive_impl(p, w));
        }
    }
    arms
}

fn schedule(arms: &[String], repetition: usize) -> Vec<&String> {
    let mut order: Vec<_> = arms.iter().collect();
    if repetition % 2 == 1 {
        order.reverse();
    }
    order
}

fn capture(prog: &str, args: &[&str]) -> String {
    Command::new(prog)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

fn load_average() -> String {
    if cfg!(target_os = "macos") {
        capture("sysctl", &["-n", "vm.loadavg"])
    } else {
        std::fs::read_to_string("/proc/loadavg")
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
}

fn binary_identity(path: &Path, source_label: Option<&str>) -> Result<Value, String> {
    Ok(
        json!({"path":path,"sha256":dive_conformance::hash::sha256_file(path).map_err(err)?,"source_label_supplied_by_user":source_label}),
    )
}

fn source_hashes(module: &Path) -> Result<Value, String> {
    let mut files = std::collections::BTreeMap::new();
    for e in std::fs::read_dir(module.parent().ok_or("module without parent")?).map_err(err)? {
        let p = e.map_err(err)?.path();
        if p.extension().is_some_and(|e| e == "tla") {
            let name = p
                .file_name()
                .ok_or("source file without name")?
                .to_string_lossy()
                .into_owned();
            files.insert(name, dive_conformance::hash::sha256_file(&p).map_err(err)?);
        }
    }
    Ok(json!(files))
}

fn fmt_opt(x: Option<f64>, digits: usize) -> String {
    x.map_or("—".to_owned(), |v| format!("{v:.digits$}"))
}

#[allow(clippy::cast_precision_loss, reason = "state rates for display")]
fn rate(states: Option<u64>, seconds: Option<f64>) -> Option<f64> {
    states
        .zip(seconds.filter(|&s| stats::positive(s)))
        .map(|(n, s)| n as f64 / s)
}

#[allow(
    clippy::too_many_lines,
    reason = "report all selected arms and paired comparisons together"
)]
fn report(
    prepared: &[Prepared],
    impls: &[String],
    runs: &[Run],
    opts: &Opts,
) -> Result<Value, String> {
    println!("\n## Current runs (whole-process wall time; CPU = user + sys)\n");
    println!(
        "| Model | arm | wall s | CPU s | distinct/wall s | peak RSS MiB | completed/intended | timeout/refused/failed/missing |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    let mut rows = Vec::new();
    for p in prepared {
        for imp in impls {
            let rs: Vec<_> = runs
                .iter()
                .filter(|r| r.bench == p.bench.name && &r.implementation == imp)
                .collect();
            let s = summarise(&rs, opts.reps);
            println!(
                "| {} | {} | {} | {} | {} | {} | {}/{} | {}/{}/{}/{} |",
                p.bench.name,
                imp,
                fmt_opt(s.wall_s, 2),
                fmt_opt(s.cpu_s, 2),
                fmt_opt(rate(p.expected.distinct, s.wall_s), 0),
                fmt_opt(s.rss_mb, 0),
                s.completed,
                s.intended,
                s.censored,
                s.refused,
                s.failed,
                s.missing
            );
            rows.push(json!({"bench":p.bench.name,"arm":imp,"statistics":s,"distinct_per_wall_s":rate(p.expected.distinct,s.wall_s),"distinct_per_cpu_s":rate(p.expected.distinct,s.cpu_s)}));
        }
    }
    println!("\n## Paired wall ratios (baseline / candidate, same repetition)\n");
    println!(
        "Partial-pair medians are descriptive only. Missing, failed, refused or censored pairs prevent the complete-suite geometric mean. Scaling also requires requested workers to be used.\n"
    );
    println!(
        "| Model | profile | workers | comparison | ratio | pairs/intended | timeout/refused/failed/missing/unused-workers |"
    );
    println!("|---|---|---|---|---|---|---|");
    let mut comparisons = Vec::new();
    let mut aggregates = Vec::new();
    for profile in &opts.profiles {
        for &workers in &opts.workers {
            let candidate = dive_impl(profile, workers);
            let mut baselines = vec![("TLC throughput", tlc_impl(workers), false)];
            if opts.baseline_bin.is_some() {
                baselines.push(("previous dive", baseline_impl(profile, workers), false));
            }
            if workers != 1 {
                baselines.push(("1-worker scaling", dive_impl(profile, 1), true));
            }
            for (comparison, baseline, scaling) in baselines {
                let mut population = Vec::new();
                for p in prepared {
                    let pairs = stats::pair(
                        runs,
                        &p.bench.name,
                        &baseline,
                        &candidate,
                        opts.reps,
                        scaling,
                    )?;
                    println!(
                        "| {} | {} | {} | {} | {} | {}/{} | {}/{}/{}/{}/{} |",
                        p.bench.name,
                        profile,
                        workers,
                        comparison,
                        fmt_opt(pairs.median, 3),
                        pairs.paired,
                        pairs.intended,
                        pairs.censored,
                        pairs.refused,
                        pairs.failed,
                        pairs.missing,
                        pairs.unused_workers
                    );
                    comparisons.push(json!({"bench":p.bench.name,"profile":profile,"workers":workers,"comparison":comparison,"pairs":pairs}));
                    population.push(pairs);
                }
                let gm = stats::geometric_mean(&population);
                let complete = population.iter().filter(|p| p.complete()).count();
                println!(
                    "{profile}, {workers} workers, {comparison}: equal-model geometric mean {}, complete models {complete}/{}.",
                    fmt_opt(gm, 3),
                    prepared.len()
                );
                aggregates.push(json!({"profile":profile,"workers":workers,"comparison":comparison,"geometric_mean":gm,"complete_models":complete,"selected_models":prepared.len(),"purpose":"descriptive all-selected-model aggregate; no performance-target acceptance assessment"}));
            }
        }
    }
    println!(
        "Performance target acceptance is not assessed by this runner. The >=400,000-distinct population is reported in each input row; no acceptance population or threshold is redefined."
    );
    Ok(
        json!({"models_selected":prepared.len(),"models":prepared.iter().map(|p|json!({"bench":p.bench.name,"inputs":p.provenance,"expected":p.expected})).collect::<Vec<_>>(),"runs":rows,"comparisons":comparisons,"aggregates":aggregates,"target_acceptance":"not-assessed"}),
    )
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "synthetic test assertions require successful construction"
)]
mod tests {
    use super::*;

    fn oracle_fixture() -> Result<(Config, OracleProvenance), String> {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config = Config::load(&repo).map_err(err)?;
        let id = &config.oracles.default_oracle().map_err(err)?.id;
        let path = dive_conformance::golden::model_golden_path(
            &dive_conformance::golden::golden_root(&repo),
            id,
            "examples",
            "specifications/GameOfLife/GameOfLife.cfg",
        );
        let golden = dive_conformance::golden::read_model_golden(&path).map_err(err)?;
        Ok((config, golden.provenance.oracle))
    }

    #[test]
    fn semantic_pins_allow_historical_runtime_and_paths() -> Result<(), String> {
        let (config, mut recorded) = oracle_fixture()?;
        let id = config.oracles.default_oracle().map_err(err)?.id.clone();
        validate_oracle_pins(&recorded, &config, &id)?;
        for entry in recorded
            .classpath
            .iter_mut()
            .chain(&mut recorded.module_libraries)
        {
            entry.path = PathBuf::from("/historical/machine/module-or-jar");
        }
        recorded.java.path = PathBuf::from("/historical/machine/java");
        recorded.java.version_line = "historical runtime on a different platform".to_owned();
        recorded.java.source = dive_conformance::oracle::JavaSource::Pinned {
            version: "historical version".to_owned(),
            sha256: "0".repeat(64),
            platform: "historical-other-platform".to_owned(),
        };
        validate_oracle_pins(&recorded, &config, &id)
    }

    #[test]
    fn changed_oracle_identity_commit_and_jar_are_rejected() -> Result<(), String> {
        let (config, recorded) = oracle_fixture()?;
        let id = &config.oracles.default_oracle().map_err(err)?.id;
        for field in ["id", "commit", "jar"] {
            let mut changed = recorded.clone();
            match field {
                "id" => changed.id.push_str("-changed"),
                "commit" => changed.upstream_commit = "0".repeat(40),
                _ => changed.sha256 = "0".repeat(64),
            }
            assert!(
                validate_oracle_pins(&changed, &config, id).is_err(),
                "{field}"
            );
        }
        Ok(())
    }

    #[test]
    fn changed_missing_extra_and_reordered_dependency_pins_are_rejected() -> Result<(), String> {
        let (config, recorded) = oracle_fixture()?;
        let id = &config.oracles.default_oracle().map_err(err)?.id;
        for libraries in [false, true] {
            let original = if libraries {
                &recorded.module_libraries
            } else {
                &recorded.classpath
            };
            assert!(original.len() >= 2, "fixture must expose resolution order");
            let mut changed_pin = original.clone();
            changed_pin[0].pin = "0".repeat(if libraries { 40 } else { 64 });
            let mut changed_id = original.clone();
            changed_id[0].id.push_str("-changed");
            let mut missing = original.clone();
            let _ = missing.pop();
            let mut extra = original.clone();
            extra.push(original[0].clone());
            let mut reordered = original.clone();
            reordered.swap(0, 1);
            for entries in [changed_pin, changed_id, missing, extra, reordered] {
                let mut changed = recorded.clone();
                if libraries {
                    changed.module_libraries = entries;
                } else {
                    changed.classpath = entries;
                }
                assert!(validate_oracle_pins(&changed, &config, id).is_err());
            }
        }
        Ok(())
    }

    fn sample(implementation: &str, rep: usize, wall_s: f64) -> Run {
        let (_, workers) = parse_impl(implementation);
        Run {
            bench: "model".to_owned(),
            implementation: implementation.to_owned(),
            rep,
            usage: None,
            wall_s,
            verdict: "success".to_owned(),
            distinct: Some(1),
            generated: Some(2),
            correct: true,
            note: String::new(),
            workers,
            used_workers: Some(workers),
            sequential_reason: None,
            depth: Some(1),
            error_class: Some("TLC_SUCCESS".to_owned()),
            exit_code: Some(0),
            property: None,
            metadata: json!({}),
        }
    }

    #[test]
    fn ratios_require_successful_current_pairs() -> Result<(), String> {
        let mut runs = [
            sample("tlc-w1", 0, 10.0),
            sample("dive-tlc-w1", 0, 5.0),
            sample("tlc-w1", 1, 100.0),
            sample("dive-tlc-w1", 1, 1.0),
            sample("dive-tlc-w1", 2, 1.0),
        ];
        runs[2].correct = false;
        assert_eq!(
            stats::pair(&runs, "model", "tlc-w1", "dive-tlc-w1", 3, false)?.median,
            Some(2.0)
        );
        assert_eq!(
            stats::pair(&runs, "model", "historical", "dive-tlc-w1", 3, false)?.median,
            None
        );
        let summary = summarise(&runs.iter().collect::<Vec<_>>(), 5);
        assert_eq!(summary.intended, 5);
        assert_eq!(summary.completed, 4);
        assert_eq!(summary.wall_s, Some(3.0));
        Ok(())
    }

    #[test]
    fn scaling_pairs_before_taking_median() {
        let runs = [(10.0, 1.0), (100.0, 1000.0), (1000.0, 100.0)]
            .into_iter()
            .enumerate()
            .flat_map(|(i, (a, b))| {
                [
                    sample("dive-default-w1", i, a),
                    sample("dive-default-w8", i, b),
                ]
            })
            .collect::<Vec<_>>();
        let p = stats::pair(
            &runs,
            "model",
            "dive-default-w1",
            "dive-default-w8",
            3,
            true,
        )
        .expect("valid pairs");
        assert_eq!(p.median, Some(10.0));
        assert!(p.complete());
        assert_eq!(
            median(&[10.0, 100.0, 1000.0])
                .zip(median(&[1.0, 1000.0, 100.0]))
                .map(|(a, b)| a / b),
            Some(1.0)
        );
    }

    #[test]
    fn missing_censored_refused_failed_pairs_prevent_aggregate() {
        let mut runs = vec![sample("tlc-w1", 0, 10.0), sample("dive-default-w1", 0, 5.0)];
        for (i, verdict) in [(1, "timeout"), (2, "unsupported"), (3, "success")] {
            let mut bad = sample("dive-default-w1", i, 20.0);
            bad.verdict = verdict.to_owned();
            bad.correct = false;
            runs.extend([sample("tlc-w1", i, 10.0), bad]);
        }
        let p = stats::pair(&runs, "model", "tlc-w1", "dive-default-w1", 5, false)
            .expect("unambiguous");
        assert_eq!(
            (p.paired, p.censored, p.refused, p.failed, p.missing),
            (1, 1, 1, 1, 1)
        );
        assert_eq!(p.median, Some(2.0));
        assert_eq!(stats::geometric_mean(&[p]), None);
    }

    #[test]
    fn duplicate_and_invalid_time_are_errors_even_when_failed() {
        let r = sample("tlc-w1", 0, 1.0);
        let mut duplicate = r.clone();
        duplicate.correct = false;
        assert!(stats::validate(&[r.clone(), duplicate]).is_err());
        for t in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut invalid = r.clone();
            invalid.wall_s = t;
            assert!(stats::validate(&[invalid]).is_err());
        }
    }

    #[test]
    fn workers_are_matched_and_sequential_scaling_is_not_credit() {
        let runs = [sample("tlc-w1", 0, 10.0), sample("dive-default-w8", 0, 1.0)];
        assert!(stats::pair(&runs, "model", "tlc-w1", "dive-default-w8", 1, false).is_err());
        let mut parallel = sample("dive-default-w8", 0, 1.0);
        parallel.used_workers = Some(1);
        let p = stats::pair(
            &[sample("dive-default-w1", 0, 10.0), parallel],
            "model",
            "dive-default-w1",
            "dive-default-w8",
            1,
            true,
        )
        .expect("valid identities");
        assert_eq!(p.unused_workers, 1);
        assert_eq!(p.median, None);
    }

    #[test]
    fn geometric_mean_gives_each_complete_model_one_weight() {
        let p = |ratio| stats::Pairs {
            median: Some(ratio),
            paired: 3,
            intended: 3,
            ..stats::Pairs::default()
        };
        assert!((stats::geometric_mean(&[p(2.0), p(8.0)]).expect("complete") - 4.0).abs() < 1e-12);
    }

    fn prepared() -> Prepared {
        Prepared {
            bench: Bench {
                name: "model".to_owned(),
                model: "model.cfg".to_owned(),
                survey: None,
                why: "test".to_owned(),
                cfg: None,
            },
            corpus_root: PathBuf::new(),
            module: "model.tla".to_owned(),
            expected: Expected {
                verdict: "success".to_owned(),
                distinct: Some(1),
                generated: Some(2),
                depth: Some(1),
                error_class: Some("TLC_SUCCESS".to_owned()),
                exit_code: Some(0),
                property: None,
            },
            provenance: json!({}),
            _scratch: None,
        }
    }

    #[test]
    fn golden_counts_require_generated_only_at_one_worker() {
        let p = prepared();
        let mut r = sample("tlc-w8", 0, 1.0);
        r.generated = Some(99);
        assert!(check(&p, &r).0);
        r.workers = 1;
        assert!(!check(&p, &r).0);
        r.generated = Some(2);
        assert!(check(&p, &r).0);
        r.depth = Some(2);
        assert!(!check(&p, &r).0);
        r.depth = Some(1);
        r.error_class = None;
        assert!(!check(&p, &r).0);
        r.error_class = Some("TLC_SUCCESS".to_owned());
        r.exit_code = Some(3);
        assert!(!check(&p, &r).0);
    }

    #[test]
    fn schedule_balances_every_arm_without_duplicates() {
        let arms = vec![
            "tlc-w1".to_owned(),
            "dive-tlc-w1".to_owned(),
            "dive-default-w1".to_owned(),
            "tlc-w8".to_owned(),
            "dive-default-w8".to_owned(),
        ];
        assert_eq!(schedule(&arms, 0), arms.iter().collect::<Vec<_>>());
        assert_eq!(schedule(&arms, 1), arms.iter().rev().collect::<Vec<_>>());
        assert_eq!(schedule(&arms, 2), schedule(&arms, 0));
    }

    #[test]
    fn both_profiles_report_matched_tlc_arms_and_pair_counts() -> Result<(), String> {
        let opts = Opts {
            dive_bin: PathBuf::new(),
            dive_revision: None,
            baseline_bin: None,
            baseline_revision: None,
            summary: None,
            reps: 1,
            profiles: vec!["tlc", "default"],
            workers: vec![1, 8],
            filters: Vec::new(),
            tlc: true,
            xmx: "3g".to_owned(),
            timeout: Duration::from_secs(1),
            runs: None,
            set: None,
            list: false,
        };
        let arms = implementations(&opts);
        assert_eq!(arms.len(), 6);
        assert!(arms.contains(&"tlc-w8".to_owned()));
        let runs = arms
            .iter()
            .map(|a| sample(a, 0, if a.starts_with("tlc-w") { 4.0 } else { 2.0 }))
            .collect::<Vec<_>>();
        let report = report(&[prepared()], &arms, &runs, &opts)?;
        let comparison = report["comparisons"].as_array().and_then(|rows| {
            rows.iter().find(|r| {
                r["profile"] == "default"
                    && r["workers"] == 8
                    && r["comparison"] == "TLC throughput"
            })
        });
        assert_eq!(comparison.map(|r| &r["pairs"]["median"]), Some(&json!(2.0)));
        assert_eq!(comparison.map(|r| &r["pairs"]["paired"]), Some(&json!(1)));
        assert_eq!(report["target_acceptance"], "not-assessed");
        Ok(())
    }

    #[test]
    fn tlc_worker_use_is_observed_in_the_startup_message() {
        let header = "Running breadth-first search Model-Checking with fp 0 and seed 0 with 8 workers on 32 cores with 3072MB heap";
        assert_eq!(tlc_workers(header), Some(8));
        assert_eq!(
            tlc_workers(&header.replace("8 workers", "1 worker")),
            Some(1)
        );
        assert_eq!(tlc_workers(""), None);
        assert_eq!(tlc_workers(&format!("{header}\n{header}")), None);
    }

    #[test]
    fn wrapper_arguments_quote_apostrophes_and_shell_syntax() {
        assert_eq!(shell_quote("it's $(a)"), "'it'\\''s $(a)'");
    }
}
