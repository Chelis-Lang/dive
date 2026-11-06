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
  --xmx <size>        TLC heap and dive process-memory ceiling (default: 3g)
  --timeout <secs>    external per-run backstop (default: 900)
  --runs <file>       append every run as a JSON line to <file>
  --raw-dir <dir>     retain original process output outside the repository
  --fail-fast         stop after the first incorrect or incomplete sample
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
    raw_dir: Option<PathBuf>,
    fail_fast: bool,
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
        timeout: Duration::from_mins(15),
        runs: None,
        raw_dir: None,
        fail_fast: false,
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
            "--raw-dir" => o.raw_dir = Some(val()?.into()),
            "--fail-fast" => o.fail_fast = true,
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
    if memory_bytes(&o.xmx).is_none() {
        return Err(
            "--xmx expects positive bytes with an optional binary k, m, g or t suffix".to_owned(),
        );
    }
    if o.baseline_bin.is_some() != o.baseline_revision.is_some() {
        return Err("--baseline-bin and --baseline-revision must be supplied together".to_owned());
    }
    if o.reps == 0 {
        return Err("--reps must be positive".to_owned());
    }
    Ok(o)
}

#[derive(Clone, Copy)]
enum Producer {
    Dive,
    Tlc,
}

/// What one timed run measured.
#[derive(Clone)]
struct Run {
    producer: Producer,
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
    golden_elapsed_ms: u64,
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
            golden_elapsed_ms: g.provenance.elapsed_ms,
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
    provenance["native_budget_policy"] = json!({
        "time": "max(60s, 10 x pinned TLC elapsed), capped below the external backstop",
        "cleanup_reserve": "10% of the external backstop, at most 30s",
        "memory": "--xmx is also the explicit whole-process memory ceiling for both dive profiles",
    });
    provenance["xmx"] = json!(opts.xmx);
    provenance["native_engine"] = json!("core");
    provenance["fail_fast"] = json!(opts.fail_fast);
    provenance["schedule"] = json!(
        "model order fixed; worker/arm blocks reversed on odd repetitions; one process at a time"
    );
    provenance["seed"] = json!(0);
    provenance["fp"] = json!(0);
    println!("provenance: {provenance}");
    let runs = collect_runs(&prepared, &impls, opts, &provenance, |p, imp, rep| {
        if imp.starts_with("tlc-w") {
            let env = tlc_env.as_ref().ok_or("no TLC environment")?;
            run_tlc(env, p, imp, rep, &time_out, opts)
        } else {
            let bin = if imp.starts_with("baseline-") {
                baseline_bin.as_ref().ok_or("missing baseline binary")?
            } else {
                &dive_bin
            };
            run_dive(bin, &module_path, p, imp, rep, &time_out, opts)
        }
    })?;
    let all_correct = runs.iter().all(|r| r.correct);
    if opts.fail_fast && !all_correct {
        if let Some(path) = &opts.summary {
            std::fs::write(
                path,
                serde_json::to_vec_pretty(&json!({
                    "schema_version":2,"provenance":provenance,"summary":null,
                    "aborted":true,"samples_attempted":runs.len(),
                    "reason":"incorrect or incomplete sample; later arms were not started",
                }))
                .map_err(err)?,
            )
            .map_err(err)?;
        }
        eprintln!("Stopped after an incorrect or incomplete sample; no timing summary.");
        return Ok(false);
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

fn collect_runs(
    prepared: &[Prepared],
    impls: &[String],
    opts: &Opts,
    provenance: &Value,
    mut execute: impl FnMut(&Prepared, &str, usize) -> Result<Run, String>,
) -> Result<Vec<Run>, String> {
    let mut runs = Vec::new();
    'samples: for rep in 0..opts.reps {
        for p in prepared {
            for (ordinal, imp) in schedule(impls, rep).into_iter().enumerate() {
                let mut r = execute(p, imp, rep)?;
                r.metadata["schedule_ordinal_within_model"] = json!(ordinal);
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
                    append_json(path, &r, provenance)?;
                }
                let stop = opts.fail_fast && !r.correct;
                runs.push(r);
                if stop {
                    break 'samples;
                }
            }
        }
    }
    Ok(runs)
}

/// Preserve original bytes on request, including a partially completed child.
/// Retained directories are exclusively created and never remove on drop.
struct RunOutput {
    path: PathBuf,
    temporary: Option<TempDir>,
}

impl RunOutput {
    fn new(raw_root: Option<&Path>) -> Result<Self, String> {
        let Some(root) = raw_root else {
            let temporary = TempDir::new("dive-bench-run").map_err(err)?;
            return Ok(Self {
                path: temporary.path().to_owned(),
                temporary: Some(temporary),
            });
        };
        std::fs::create_dir_all(root).map_err(err)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(err)?
            .as_nanos();
        let path = root.join(format!("sample-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).map_err(err)?;
        Ok(Self {
            path: std::fs::canonicalize(path).map_err(err)?,
            temporary: None,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn record(&self, name: &str, value: &Value) -> Result<(), String> {
        if self.temporary.is_none() {
            std::fs::write(
                self.path.join(name),
                serde_json::to_vec_pretty(value).map_err(err)?,
            )
            .map_err(err)?;
        }
        Ok(())
    }

    fn retain_usage(&self, time_out: &Path) -> Result<(), String> {
        if self.temporary.is_none() && time_out.exists() {
            std::fs::copy(time_out, self.path.join("time.txt")).map_err(err)?;
        }
        Ok(())
    }
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

// ADR 0004: primary parallel BFS depth varies; native BFS remains deterministic.
fn depth_parity_required(run: &Run) -> bool {
    matches!(run.producer, Producer::Dive) || run.workers == 1
}

fn check(p: &Prepared, r: &Run) -> (bool, String) {
    let e = &p.expected;
    let mut bad = Vec::new();
    if r.verdict != e.verdict {
        bad.push(format!("verdict {} (golden {})", r.verdict, e.verdict));
    }
    if r.distinct != e.distinct {
        bad.push(format!(
            "distinct {:?} (golden {:?})",
            r.distinct, e.distinct
        ));
    }
    if (depth_parity_required(r) && r.depth != e.depth) || r.depth.is_none_or(|depth| depth == 0) {
        bad.push(format!("depth {:?} (golden {:?})", r.depth, e.depth));
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
    let max_memory = memory_bytes(&opts.xmx).ok_or("invalid finite --xmx memory ceiling")?;
    let (profile, workers) = parse_impl(imp);
    for a in [
        "check", "--engine", "core", "--output", "json", "--seed", "0", "--fp", "0",
    ] {
        args.push(a.into());
    }
    args.push("--workers".into());
    args.push(workers.to_string().into());
    let time_limit = native_time_limit(p.golden_elapsed_ms, opts.timeout);
    let time_limit_ms = u64::try_from(time_limit.as_millis())
        .map_err(|_| "native time budget exceeds the reportable millisecond range")?;
    args.extend([
        "--time-limit".into(),
        format!("{time_limit_ms}ms").into(),
        "--max-memory".into(),
        opts.xmx.clone().into(),
    ]);
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
        .env_remove("DIVE_ENGINE")
        .env_remove("DIVE_CHAOS")
        .args(&args);
    let out_dir = RunOutput::new(opts.raw_dir.as_deref())?;
    let _ = std::fs::remove_file(time_out);
    let load_before = load_average();
    let invocation_args: Vec<String> = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    out_dir.record(
        "started.json",
        &json!({
            "bench":p.bench.name,"implementation":imp,"rep":rep + 1,
            "argv":invocation_args,"cwd":dir,"timeout_seconds":opts.timeout.as_secs_f64(),
            "expected":p.expected,"stdout":"stdout.txt","stderr":"stderr.txt",
        }),
    )?;
    let outcome = run_with_timeout(&mut cmd, opts.timeout, out_dir.path()).map_err(err)?;
    let physical = json!({"exit_code":outcome.exit_code,"signal":outcome.signal,
        "timed_out":outcome.timed_out,"elapsed_seconds":outcome.elapsed.as_secs_f64()});
    out_dir.record("finished.json", &physical)?;
    out_dir.retain_usage(time_out)?;
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
        producer: Producer::Dive,
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
        metadata: json!({"argv":invocation_args,"cwd":dir,"environment_removed":["DIVE_COMPAT","DIVE_ENGINE","DIVE_CHAOS"],"requested_native_budget":{"reference_tlc_elapsed_ms":p.golden_elapsed_ms,"time_limit_ms":time_limit_ms,"max_memory":opts.xmx,"max_memory_bytes":max_memory},"reported_options":v["options"],"reported_stopped":v["stopped"],"reported_resources":v["resources"],"reported_execution":v["execution"],"reported_engine":v["engine"],"reported_engine_requested":v["engine_requested"],"reported_engine_fallback":v["engine_fallback"],"load_before":load_before,"load_after":load_after,"inputs":p.provenance}),
    };
    r.metadata["physical_process"] = physical;
    r.metadata["raw_directory"] = json!(opts.raw_dir.as_ref().map(|_| out_dir.path()));
    check_native(p, &mut r, &v, time_limit_ms, max_memory);
    Ok(r)
}

fn check_native(p: &Prepared, r: &mut Run, v: &Value, time_limit_ms: u64, max_memory: u64) {
    (r.correct, r.note) = check(p, r);
    if v["engine"] != "core" || v["engine_requested"] != "core" || !v["engine_fallback"].is_null() {
        r.correct = false;
        r.note
            .push_str("; native report does not confirm Core without fallback");
    }
    let budget = &v["options"]["budget"];
    if budget["time_limit_ms"].as_u64() != Some(time_limit_ms)
        || budget["max_memory_bytes"].as_u64() != Some(max_memory)
        || budget["time_limit_source"] != "explicit"
        || budget["max_memory_source"] != "explicit"
    {
        r.correct = false;
        r.note
            .push_str("; native report does not match the requested explicit time/memory budgets");
    }
    if v["tlc"]["verified"] != true
        || v["tlc"]["code"].as_i64() != Some(2193)
        || v["tlc"]["exit_status"].as_i64() != Some(0)
    {
        r.correct = false;
        r.note
            .push_str("; dive did not report verified TLC success");
    }
}

/// Reserve cleanup time so a native stop can publish its structured report
/// before the independent external backstop kills an unresponsive process.
fn native_time_limit(golden_elapsed_ms: u64, external_timeout: Duration) -> Duration {
    let reserve = (external_timeout / 10).min(Duration::from_secs(30));
    Duration::from_millis(golden_elapsed_ms)
        .saturating_mul(10)
        .max(Duration::from_secs(60))
        .min(external_timeout.saturating_sub(reserve))
}

/// Binary memory units accepted by both native budget flags and JVM-style
/// heap settings; automatic, unlimited and overflowing ceilings are excluded.
fn memory_bytes(text: &str) -> Option<u64> {
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (digits, unit) = text.split_at(split);
    let number: u64 = digits.parse().ok().filter(|n| *n > 0)?;
    let shift = match unit.to_ascii_lowercase().as_str() {
        "" => 0,
        "k" => 10,
        "m" => 20,
        "g" => 30,
        "t" => 40,
        _ => return None,
    };
    number.checked_mul(1 << shift)
}

fn run_tlc(
    env: &OracleEnv,
    p: &Prepared,
    imp: &str,
    rep: usize,
    time_out: &Path,
    opts: &Opts,
) -> Result<Run, String> {
    let output = RunOutput::new(opts.raw_dir.as_deref())?;
    let mut env = env.clone();
    if opts.raw_dir.is_some() {
        env.raw_invocations = Some(output.path().join("oracle"));
    }
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
    output.retain_usage(time_out)?;
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
        producer: Producer::Tlc,
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
    r.metadata["physical_process"] = json!({
        "exit_code":inv.outcome.exit_code,"signal":inv.outcome.signal,
        "timed_out":inv.outcome.timed_out,"elapsed_seconds":inv.outcome.elapsed.as_secs_f64(),
    });
    r.metadata["raw_directory"] = json!(opts.raw_dir.as_ref().map(|_| output.path()));
    (r.correct, r.note) = check(p, &r);
    if inv.outcome.signal.is_some() || inv.outcome.timed_out || inv.outcome.exit_code != Some(0) {
        r.correct = false;
        r.note
            .push_str("; oracle process did not exit successfully");
    }
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
        "depth_parity_required": depth_parity_required(r),
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
    let censored = runs.iter().filter(|r| stats::incomplete(r)).count();
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
        "| Model | arm | wall s | CPU s | distinct/wall s | peak RSS MiB | completed/intended | incomplete/refused/failed/missing |"
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
        "| Model | profile | workers | comparison | ratio | pairs/intended | incomplete/refused/failed/missing/unused-workers |"
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
            producer: if implementation.starts_with("tlc-w") {
                Producer::Tlc
            } else {
                Producer::Dive
            },
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
            golden_elapsed_ms: 0,
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
    fn parallel_primary_depth_is_reported_but_other_observables_remain_strict() {
        let p = prepared();
        let mut r = sample("tlc-w8", 0, 1.0);
        r.depth = Some(41);
        assert!(check(&p, &r).0);
        assert!(!depth_parity_required(&r));
        for change in 0..7 {
            let mut bad = r.clone();
            match change {
                0 => bad.depth = None,
                1 => bad.distinct = Some(2),
                2 => bad.verdict = "unsupported".to_owned(),
                3 => bad.error_class = None,
                4 => bad.exit_code = Some(3),
                5 => bad.depth = Some(0),
                _ => bad.property = Some("unexpected".to_owned()),
            }
            assert!(!check(&p, &bad).0, "changed observable {change}");
        }
        r.workers = 1;
        assert!(depth_parity_required(&r));
        assert!(!check(&p, &r).0);
    }

    #[test]
    fn every_native_arm_keeps_exact_depth_even_with_an_oracle_label() {
        let p = prepared();
        for label in [
            "dive-tlc-w1",
            "dive-tlc-w8",
            "dive-default-w1",
            "dive-default-w8",
            "baseline-tlc-w1",
            "baseline-tlc-w8",
            "baseline-default-w1",
            "baseline-default-w8",
        ] {
            let mut r = sample(label, 0, 1.0);
            assert!(matches!(r.producer, Producer::Dive));
            assert!(depth_parity_required(&r));
            r.depth = Some(41);
            assert!(!check(&p, &r).0, "{label}");
            r.implementation = "tlc-w8".to_owned();
            assert!(
                depth_parity_required(&r),
                "a display label cannot change producer authority"
            );
            assert!(!check(&p, &r).0);
        }
    }

    #[test]
    fn json_rows_keep_actual_depth_and_explicit_comparison_policy() -> Result<(), String> {
        let dir = TempDir::new_in(&std::env::temp_dir(), "bench-depth-policy").map_err(err)?;
        let path = dir.path().join("runs.jsonl");
        for label in ["tlc-w8", "tlc-w1", "dive-tlc-w8", "baseline-default-w8"] {
            let mut run = sample(label, 0, 1.0);
            run.depth = Some(41);
            append_json(&path, &run, &json!({}))?;
        }
        let text = std::fs::read_to_string(&path).map_err(err)?;
        let rows: Vec<Value> = text
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .map_err(err)?;
        assert_eq!(rows.len(), 4);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row["depth"], 41);
            assert_eq!(row["depth_parity_required"], i != 0);
        }
        Ok(())
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
            raw_dir: None,
            fail_fast: false,
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

    #[test]
    fn native_budgets_use_tlc_time_and_leave_external_cleanup_room() {
        let backstop = Duration::from_mins(15);
        for (reference_ms, expected_s) in [(0, 60), (500, 60), (20_000, 200), (u64::MAX, 870)] {
            assert_eq!(
                native_time_limit(reference_ms, backstop),
                Duration::from_secs(expected_s)
            );
        }
        assert_eq!(
            native_time_limit(20_000, Duration::from_secs(1)),
            Duration::from_millis(900)
        );
    }

    #[test]
    fn memory_settings_require_a_finite_nonoverflowing_binary_size() {
        for (text, expected) in [("3g", 3 << 30), ("512M", 512 << 20), ("1048576", 1 << 20)] {
            assert_eq!(memory_bytes(text), Some(expected));
        }
        for text in [
            "",
            "auto",
            "none",
            "0",
            "0g",
            "1.5g",
            "3gb",
            "18446744073709551615g",
        ] {
            assert_eq!(memory_bytes(text), None, "{text}");
        }
    }

    fn native_test_options(bin: &Path) -> Opts {
        Opts {
            dive_bin: bin.to_path_buf(),
            dive_revision: None,
            baseline_bin: Some(bin.to_path_buf()),
            baseline_revision: Some("test".to_owned()),
            summary: None,
            reps: 1,
            profiles: vec!["tlc", "default"],
            workers: vec![1, 8],
            filters: Vec::new(),
            tlc: false,
            xmx: "3g".to_owned(),
            // These finite report fixtures test the real time/shell wrapper,
            // not startup latency. Leave headroom before its outer backstop.
            timeout: Duration::from_secs(5),
            runs: None,
            raw_dir: None,
            fail_fast: false,
            set: None,
            list: false,
        }
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "exercise the real child-process boundary for all native arms"
    )]
    fn native_arms_receive_budgets_and_stops_never_become_completed_timings() -> Result<(), String>
    {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new_in(&std::env::temp_dir(), "bench-native-budgets").map_err(err)?;
        let bin = dir.path().join("native");
        // The child refuses an invocation missing either guard. Its report
        // deliberately retains the expected full counts: stopped must still
        // be incomplete, even if counters happen to match the successful run.
        std::fs::write(&bin, r#"#!/bin/sh
limit=missing
memory=missing
workers=missing
while [ "$#" -gt 0 ]; do
    case "$1" in
        --time-limit) shift; limit="$1" ;;
        --max-memory) shift; memory="$1" ;;
        --workers) shift; workers="$1" ;;
    esac
    shift
done
[ "$limit" = 4500ms ] && [ "$memory" = 3g ] || exit 64
printf '{"verdict":"stopped","statistics":{"distinct":1,"generated":2,"depth":1},"stopped":{"resource":"time","phase":"safety-search","limit":4500,"observed":4501},"options":{"workers":{"used":%s},"budget":{"time_limit_ms":4500,"max_memory_bytes":3221225472,"time_limit_source":"explicit","max_memory_source":"explicit"}},"resources":{"checking_started":true},"engine":"reference","engine_requested":"auto","engine_fallback":{"stage":"compile","reason":"fixture fallback"},"execution":null}\n' "$workers"
exit 5
"#).map_err(err)?;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).map_err(err)?;
        let opts = native_test_options(&bin);
        let mut p = prepared();
        p.corpus_root = dir.path().to_path_buf();
        let output = dir.path().join("time.txt");
        let rows = dir.path().join("rows.jsonl");
        for arm in implementations(&opts) {
            let stopped = run_dive(&bin, &[], &p, &arm, 0, &output, &opts)?;
            assert_eq!(stopped.verdict, "stopped");
            assert_eq!(stopped.exit_code, Some(5));
            assert!(!stopped.correct);
            assert!(
                !stopped
                    .note
                    .contains("requested explicit time/memory budgets")
            );
            assert_eq!(stopped.used_workers, Some(stopped.workers));
            assert_eq!(
                stopped.metadata["requested_native_budget"]["time_limit_ms"],
                4500
            );
            assert_eq!(
                stopped.metadata["reported_stopped"]["phase"],
                "safety-search"
            );
            assert_eq!(
                stopped.metadata["reported_resources"]["checking_started"],
                true
            );
            // A producer label is not evidence that this child used the core.
            // Preserve the actual engine even for baseline/core campaign arms.
            assert_eq!(stopped.metadata["reported_engine"], "reference");
            assert_eq!(stopped.metadata["reported_engine_requested"], "auto");
            assert_eq!(
                stopped.metadata["reported_engine_fallback"]["stage"],
                "compile"
            );
            append_json(&rows, &stopped, &json!({}))?;
            let aggregate = summarise(&[&stopped], 1);
            assert_eq!(
                (aggregate.completed, aggregate.censored, aggregate.failed),
                (0, 1, 0)
            );
            assert!(aggregate.wall_s.is_none());
            let mut reference = sample("tlc-w1", 0, 1.0);
            reference.workers = stopped.workers;
            let pair = stats::pair(&[reference, stopped], "model", "tlc-w1", &arm, 1, false)?;
            assert_eq!((pair.paired, pair.censored), (0, 1));
            assert!(pair.median.is_none());
            assert!(stats::geometric_mean(&[pair]).is_none());
        }
        let raw = std::fs::read_to_string(rows).map_err(err)?;
        assert_eq!(raw.lines().count(), 8);
        for row in raw.lines() {
            let row: Value = serde_json::from_str(row).map_err(err)?;
            assert_eq!(row["verdict"], "stopped");
            assert_eq!(row["run"]["reported_stopped"]["observed"], 4501);
            assert_eq!(row["run"]["reported_engine"], "reference");
            assert_eq!(row["run"]["reported_engine_requested"], "auto");
            assert_eq!(
                row["run"]["reported_engine_fallback"]["reason"],
                "fixture fallback"
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn native_success_requires_explicit_budgets_and_core_without_fallback() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new_in(&std::env::temp_dir(), "bench-budget-contract").map_err(err)?;
        let bin = dir.path().join("native");
        let opts = native_test_options(&bin);
        let mut p = prepared();
        p.corpus_root = dir.path().to_path_buf();
        let output = dir.path().join("time.txt");
        for case in [
            "matching",
            "missing",
            "unlimited",
            "wrong-time",
            "wrong-memory",
            "implicit",
            "reference",
            "auto",
            "fallback",
            "missing-engine",
        ] {
            let mut report = json!({
                "engine":"core","engine_requested":"core","engine_fallback":null,
                "verdict":"success", "statistics":{"distinct":1,"generated":2,"depth":1},
                "tlc":{"name":"TLC_SUCCESS","verified":true,"code":2193,"exit_status":0},
                "options":{"workers":{"used":1},"budget":{
                    "time_limit_ms":4500,"time_limit_source":"explicit",
                    "max_memory_bytes":3_221_225_472_u64,"max_memory_source":"explicit"
                }},
            });
            match case {
                "missing" => {
                    report["options"]
                        .as_object_mut()
                        .expect("options")
                        .remove("budget");
                }
                "unlimited" => {
                    report["options"]["budget"]["time_limit_ms"] = Value::Null;
                    report["options"]["budget"]["max_memory_bytes"] = Value::Null;
                }
                "wrong-time" => report["options"]["budget"]["time_limit_ms"] = json!(4501),
                "wrong-memory" => report["options"]["budget"]["max_memory_bytes"] = json!(1 << 30),
                "implicit" => report["options"]["budget"]["max_memory_source"] = json!("default"),
                "reference" => report["engine"] = json!("reference"),
                "auto" => report["engine_requested"] = json!("auto"),
                "fallback" => report["engine_fallback"] = json!({"reason":"unproved"}),
                "missing-engine" => {
                    report.as_object_mut().expect("report").remove("engine");
                }
                _ => {}
            }
            let script = format!("#!/bin/sh\ncat <<'BENCH_REPORT'\n{report}\nBENCH_REPORT\n");
            std::fs::write(&bin, script).map_err(err)?;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).map_err(err)?;
            for arm in [
                "dive-tlc-w1",
                "dive-default-w1",
                "baseline-tlc-w1",
                "baseline-default-w1",
            ] {
                let run = run_dive(&bin, &[], &p, arm, 0, &output, &opts)?;
                assert_eq!(run.correct, case == "matching", "{case}: {}", run.note);
                assert_eq!(run.metadata["reported_engine"], report["engine"]);
                assert_eq!(
                    run.metadata["reported_engine_requested"],
                    report["engine_requested"]
                );
                assert_eq!(
                    run.metadata["reported_engine_fallback"],
                    report["engine_fallback"]
                );
                let pairs = stats::pair(
                    &[sample("tlc-w1", 0, 1.0), run],
                    "model",
                    "tlc-w1",
                    arm,
                    1,
                    false,
                )?;
                assert_eq!(pairs.paired, usize::from(case == "matching"));
                if case != "matching" {
                    assert_eq!(pairs.failed, 1);
                    assert!(pairs.median.is_none());
                    assert!(stats::geometric_mean(&[pairs]).is_none());
                }
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn fail_fast_preserves_original_bytes_and_starts_no_later_child() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new("bench-fail-fast").map_err(err)?;
        let bin = dir.path().join("native");
        let marker = dir.path().join("attempts");
        let report = json!({
            "engine":"core","engine_requested":"core","engine_fallback":null,
            "verdict":"success","statistics":{"distinct":1,"generated":2,"depth":1},
            "tlc":{"name":"TLC_SUCCESS","verified":true,"code":2193,"exit_status":0},
            "options":{"workers":{"used":1},"budget":{
                "time_limit_ms":4500,"time_limit_source":"explicit",
                "max_memory_bytes":3_221_225_472_u64,"max_memory_source":"explicit"
            }},
        });
        let script = format!(
            r#"#!/bin/sh
engine=missing
while [ "$#" -gt 0 ]; do
    case "$1" in --engine) shift; engine="$1" ;; esac
    shift
done
[ "$engine" = core ] || exit 64
if [ ! -f {marker} ]; then
    printf x > {marker}
    cat <<'BENCH_REPORT'
{report}
BENCH_REPORT
    exit 0
fi
printf x >> {marker}
printf '\377broken output\n'
printf '\376original error\n' >&2
exit 7
"#,
            marker = shell_quote(&marker.to_string_lossy())
        );
        std::fs::write(&bin, script).map_err(err)?;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).map_err(err)?;
        let mut opts = native_test_options(&bin);
        opts.reps = 3;
        opts.fail_fast = true;
        opts.raw_dir = Some(dir.path().join("raw"));
        opts.runs = Some(dir.path().join("runs.jsonl"));
        let mut p = prepared();
        p.corpus_root = dir.path().to_path_buf();
        let arms = vec!["dive-default-w1".to_owned(), "dive-tlc-w1".to_owned()];
        let rows = collect_runs(&[p], &arms, &opts, &json!({}), |p, imp, rep| {
            run_dive(&bin, &[], p, imp, rep, &dir.path().join("time.txt"), &opts)
        })?;
        assert_eq!(std::fs::read(marker).map_err(err)?, b"xx");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].correct);
        assert!(!rows[1].correct);
        assert_eq!(rows[1].metadata["physical_process"]["exit_code"], 7);
        assert_eq!(rows[1].metadata["physical_process"]["timed_out"], false);
        let raw = Path::new(
            rows[1].metadata["raw_directory"]
                .as_str()
                .expect("retained"),
        );
        assert_eq!(
            std::fs::read(raw.join("stdout.txt")).map_err(err)?,
            b"\xffbroken output\n"
        );
        let stderr = std::fs::read(raw.join("stderr.txt")).map_err(err)?;
        assert!(stderr.starts_with(b"\xfeoriginal error\n"));
        assert!(raw.join("started.json").is_file());
        assert!(raw.join("finished.json").is_file());
        assert!(raw.join("time.txt").is_file());
        assert_eq!(
            std::fs::read_to_string(opts.runs.expect("rows"))
                .map_err(err)?
                .lines()
                .count(),
            2
        );
        let paired = stats::pair(&rows, "model", &arms[0], &arms[1], 3, false)?;
        assert_eq!(paired.paired, 0);
        assert!(paired.median.is_none());
        assert!(stats::geometric_mean(&[paired]).is_none());
        opts.fail_fast = false;
        opts.raw_dir = None;
        opts.runs = None;
        let mut p = prepared();
        p.corpus_root = dir.path().to_path_buf();
        let continued = collect_runs(&[p], &arms, &opts, &json!({}), |p, imp, rep| {
            run_dive(&bin, &[], p, imp, rep, &dir.path().join("time.txt"), &opts)
        })?;
        assert_eq!(continued.len(), 6);
        assert_eq!(
            std::fs::read(dir.path().join("attempts"))
                .map_err(err)?
                .len(),
            8
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn fail_fast_retains_timeout_output_and_censoring_before_next_arm() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new("bench-censored").map_err(err)?;
        let bin = dir.path().join("native");
        std::fs::write(&bin, "#!/bin/sh\nprintf 'entered\\n'\nexec sleep 30\n").map_err(err)?;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).map_err(err)?;
        let mut opts = native_test_options(&bin);
        opts.fail_fast = true;
        opts.timeout = Duration::from_millis(100);
        opts.raw_dir = Some(dir.path().join("raw"));
        let mut p = prepared();
        p.corpus_root = dir.path().to_path_buf();
        let arms = vec!["dive-default-w1".to_owned(), "dive-tlc-w1".to_owned()];
        let runs = collect_runs(&[p], &arms, &opts, &json!({}), |p, imp, rep| {
            run_dive(&bin, &[], p, imp, rep, &dir.path().join("time.txt"), &opts)
        })?;
        assert_eq!(runs.len(), 1);
        assert!(!runs[0].correct);
        assert_eq!(runs[0].verdict, "timeout");
        assert_eq!(runs[0].metadata["physical_process"]["timed_out"], true);
        assert!(!runs[0].metadata["physical_process"]["signal"].is_null());
        let raw = Path::new(
            runs[0].metadata["raw_directory"]
                .as_str()
                .expect("retained"),
        );
        assert_eq!(
            std::fs::read(raw.join("stdout.txt")).map_err(err)?,
            b"entered\n"
        );
        assert!(raw.join("finished.json").is_file());
        assert!(summarise(&[&runs[0]], 1).wall_s.is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn retained_oracle_capture_keeps_original_bytes_and_physical_exit() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new("bench-oracle-output").map_err(err)?;
        let bin = dir.path().join("java");
        std::fs::write(&bin, "#!/bin/sh\nprintf '\\377original oracle output\\n'\nprintf '\\376original oracle error\\n' >&2\nexit 17\n").map_err(err)?;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).map_err(err)?;
        let (config, provenance) = oracle_fixture()?;
        let mut java = provenance.java;
        java.path = bin.clone();
        let env = OracleEnv {
            oracle: config.oracles.default_oracle().map_err(err)?.clone(),
            jar: dir.path().join("unused.jar"),
            classpath: Vec::new(),
            libraries: Vec::new(),
            java,
            capabilities: dive_conformance::oracle::Capabilities::default(),
            home: dir.path().join("home"),
            tmp_root: dir.path().join("tmp"),
            cache_root: dir.path().join("cache"),
            raw_invocations: None,
        };
        let mut opts = native_test_options(&bin);
        opts.raw_dir = Some(dir.path().join("raw"));
        let mut p = prepared();
        p.corpus_root = dir.path().join("source");
        std::fs::create_dir_all(&p.corpus_root).map_err(err)?;
        std::fs::write(
            p.corpus_root.join("model.tla"),
            "---- MODULE model ----\n====\n",
        )
        .map_err(err)?;
        std::fs::write(p.corpus_root.join("model.cfg"), "").map_err(err)?;
        let run = run_tlc(&env, &p, "tlc-w1", 0, &dir.path().join("time.txt"), &opts)?;
        assert!(!run.correct);
        assert_eq!(run.metadata["physical_process"]["exit_code"], 17);
        let root = Path::new(run.metadata["raw_directory"].as_str().expect("retained"));
        let capture = std::fs::read_dir(root.join("oracle"))
            .map_err(err)?
            .next()
            .expect("oracle capture")
            .map_err(err)?
            .path();
        assert_eq!(
            std::fs::read(capture.join("stdout.txt")).map_err(err)?,
            b"\xfforiginal oracle output\n"
        );
        assert_eq!(
            std::fs::read(capture.join("stderr.txt")).map_err(err)?,
            b"\xfeoriginal oracle error\n"
        );
        assert!(capture.join("started.json").is_file());
        assert!(capture.join("completed.json").is_file());
        Ok(())
    }
}
