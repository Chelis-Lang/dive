//! `dive-bench`: times `dive check` (and optionally TLC) on the benchmark set.
//! See the library documentation and `--help`.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use dive_bench::{
    Bench, Usage, median, parse_time_output, range, survey_w1, time_args, tlc_counts,
};
use dive_conformance::cache::Cache;
use dive_conformance::config::Config;
use dive_conformance::oracle::{CheckMode, Java, OracleEnv, TlcJob};
use dive_conformance::process::run_with_timeout;
use dive_conformance::util::TempDir;
use serde_json::{Value, json};

const USAGE: &str = "\
dive-bench: time `dive check` on the G1 benchmark set

usage: dive-bench [options]

  --dive-bin <path>   the dive binary (default: target/release/dive)
  --reps <n>          interleaved repetitions (default: 3)
  --profile <p>       tlc, default or both (default: both)
  --filter <name>     run only this benchmark (repeatable)
  --workers <list>    dive worker counts, e.g. 1,2,4,8 (default: 1); TLC runs at 1
  --tlc               also time the pinned TLC, -workers 1, on this machine
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
                    .map(|w| w.trim().parse::<usize>().ok().filter(|&n| n > 0))
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
}

/// The golden facts a run must reproduce.
struct Expected {
    verdict: String,
    distinct: Option<u64>,
    generated: Option<u64>,
    elapsed_ms: Option<u64>,
}

/// Where a benchmark runs: a corpus root, the module and the cfg relative to
/// it (a scratch copy when the configuration is replaced).
struct Prepared {
    bench: Bench,
    corpus_root: PathBuf,
    module: String,
    expected: Expected,
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
        if g.oracle != oracle
            || g.corpus != "examples"
            || g.model.id != b.model
            || g.workers != 1
            || g.provenance.corpus_commit != corpus.commit
            || g.provenance.oracle.sha256 != config.oracles.oracle(&oracle).map_err(err)?.sha256
            || g.result.verdict != dive_conformance::tlc::result::Verdict::Success
            || g.result.counts.distinct_states.is_none()
            || g.result.counts.states_generated.is_none()
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
            elapsed_ms: Some(g.provenance.elapsed_ms),
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
        prepared.push(Prepared {
            bench: b.clone(),
            corpus_root: root,
            module,
            expected,
            _scratch: own,
        });
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
        Some(OracleEnv::prepare(&config, &cache, &oracle, &java).map_err(err)?)
    } else {
        None
    };

    let survey_csv = std::fs::read_to_string(repo.join("docs/survey/data/bench-summary.csv"))
        .unwrap_or_default();
    let mut impls: Vec<String> = Vec::new();
    for p in &opts.profiles {
        for &w in &opts.workers {
            impls.push(dive_impl(p, w));
        }
    }
    if tlc_env.is_some() {
        impls.push("tlc".to_owned());
    }
    let mut provenance = print_machine(&dive_bin);
    provenance["binary_sha256"] =
        json!(dive_conformance::hash::sha256_file(&dive_bin).map_err(err)?);
    provenance["oracle"] =
        json!({"id":oracle, "sha256":config.oracles.oracle(&oracle).map_err(err)?.sha256});
    provenance["corpus_commit"] = json!(corpus.commit);
    provenance["seed"] = json!(0);
    provenance["fp"] = json!(0);
    println!("provenance: {provenance}");
    let mut runs: Vec<Run> = Vec::new();
    let mut all_correct = true;
    for rep in 0..opts.reps {
        for p in &prepared {
            for imp in &impls {
                let r = if imp == "tlc" {
                    let env = tlc_env.as_ref().ok_or("no TLC environment")?;
                    run_tlc(env, p, rep, &time_out, opts)?
                } else {
                    run_dive(&dive_bin, &module_path, p, imp, rep, &time_out, opts)?
                };
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
    report(&prepared, &impls, &runs, &survey_csv);
    if opts.workers.len() > 1 {
        report_scaling(&prepared, &opts.profiles, &opts.workers, &runs);
    }
    Ok(all_correct)
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
        "runner_dirty": !capture("git", &["status", "--porcelain"]).is_empty()})
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

fn check(
    p: &Prepared,
    verdict: &str,
    distinct: Option<u64>,
    generated: Option<u64>,
) -> (bool, String) {
    let e = &p.expected;
    let mut bad = Vec::new();
    if verdict != e.verdict {
        bad.push(format!("verdict {verdict} (golden {})", e.verdict));
    }
    if distinct != e.distinct {
        bad.push(format!("distinct {distinct:?} (golden {:?})", e.distinct));
    }
    if generated != e.generated {
        bad.push(format!(
            "generated {generated:?} (golden {:?})",
            e.generated
        ));
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
    cmd.current_dir(dir).env_remove("DIVE_COMPAT").args(&args);
    let out_dir = TempDir::new_in(&std::env::temp_dir(), "dive-bench-run").map_err(err)?;
    let _ = std::fs::remove_file(time_out);
    let outcome = run_with_timeout(&mut cmd, opts.timeout, out_dir.path()).map_err(err)?;
    let usage = read_usage(time_out);
    let v: Value = serde_json::from_str(outcome.stdout.trim()).unwrap_or(Value::Null);
    let verdict = if outcome.timed_out {
        "timeout".to_owned()
    } else {
        v["verdict"].as_str().unwrap_or("no-report").to_owned()
    };
    let distinct = v["statistics"]["distinct"].as_u64();
    let generated = v["statistics"]["generated"].as_u64();
    let (mut correct, mut note) = check(p, &verdict, distinct, generated);
    if outcome.exit_code != Some(0) || v["tlc"]["verified"] != true {
        correct = false;
        note.push_str("; process did not exit successfully with a verified verdict");
    }
    if verdict == "no-report" {
        outcome
            .stderr
            .lines()
            .last()
            .unwrap_or_default()
            .clone_into(&mut note);
    }
    Ok(Run {
        bench: p.bench.name.clone(),
        implementation: imp.to_owned(),
        rep,
        usage,
        wall_s: outcome.elapsed.as_secs_f64(),
        verdict,
        distinct,
        generated,
        correct,
        note,
    })
}

fn run_tlc(
    env: &OracleEnv,
    p: &Prepared,
    rep: usize,
    time_out: &Path,
    opts: &Opts,
) -> Result<Run, String> {
    let job = TlcJob {
        corpus_root: &p.corpus_root,
        module: &p.module,
        config: &p.bench.model,
        mode: CheckMode::Exhaustive,
        features: &[],
        workers: 1,
        timeout: opts.timeout,
        xmx: &opts.xmx,
        seed: 0,
        fp: 0,
        lncheck: "final",
        dump_trace: false,
    };
    let _ = std::fs::remove_file(time_out);
    let inv = env.run_tlc(&job).map_err(err)?;
    let usage = read_usage(time_out);
    let counts = tlc_counts(&inv.outcome.stdout);
    let verdict = match inv.outcome.exit_code {
        Some(0) => "success",
        _ if inv.outcome.timed_out => "timeout",
        _ => "other",
    }
    .to_owned();
    let (generated, distinct) = counts.map_or((None, None), |(g, d)| (Some(g), Some(d)));
    let (correct, note) = check(p, &verdict, distinct, generated);
    Ok(Run {
        bench: p.bench.name.clone(),
        implementation: "tlc".to_owned(),
        rep,
        usage,
        wall_s: inv.outcome.elapsed.as_secs_f64(),
        verdict,
        distinct,
        generated,
        correct,
        note,
    })
}

fn append_json(path: &Path, r: &Run, provenance: &Value) -> Result<(), String> {
    use std::io::Write as _;
    let line = json!({
        "schema_version": 1,
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
        "correct": r.correct,
        "note": r.note,
    });
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(f, "{line}").map_err(err)
}

struct Summary {
    cpu: Option<f64>,
    cpu_range: Option<(f64, f64)>,
    wall: Option<f64>,
    rss_mb: Option<f64>,
    ok: usize,
    n: usize,
}

fn summarise(runs: &[&Run]) -> Summary {
    let cpu: Vec<f64> = runs
        .iter()
        .filter(|r| r.correct)
        .filter_map(|r| r.usage.map(|u| u.cpu_s()))
        .collect();
    let wall: Vec<f64> = runs
        .iter()
        .filter(|r| r.correct)
        .map(|r| r.wall_s)
        .collect();
    #[allow(clippy::cast_precision_loss, reason = "megabytes for display")]
    let rss: Vec<f64> = runs
        .iter()
        .filter(|r| r.correct)
        .filter_map(|r| r.usage.map(|u| u.max_rss_bytes as f64 / f64::from(1 << 20)))
        .collect();
    Summary {
        cpu: median(&cpu),
        cpu_range: range(&cpu),
        wall: median(&wall),
        rss_mb: median(&rss),
        ok: runs.iter().filter(|r| r.correct).count(),
        n: runs.len(),
    }
}

/// The implementation name of a dive profile at a worker count: `dive-tlc`,
/// `dive-default`, with `-w<n>` beyond one worker.
fn dive_impl(profile: &str, workers: usize) -> String {
    if workers == 1 {
        format!("dive-{profile}")
    } else {
        format!("dive-{profile}-w{workers}")
    }
}

/// The profile and worker count of a dive implementation name.
fn parse_impl(imp: &str) -> (&str, usize) {
    let rest = imp.strip_prefix("dive-").unwrap_or(imp);
    match rest.rsplit_once("-w") {
        Some((p, w)) if w.parse::<usize>().is_ok() => (p, w.parse().unwrap_or(1)),
        _ => (rest, 1),
    }
}

/// Wall time and CPU time against one worker, per profile and worker count.
fn report_scaling(prepared: &[Prepared], profiles: &[&str], workers: &[usize], runs: &[Run]) {
    println!("\n## Scaling (median wall and CPU, against the same profile at 1 worker)\n");
    println!("| Benchmark | profile | workers | wall s | speed-up | CPU s | CPU / 1-worker CPU |");
    println!("|---|---|---|---|---|---|---|");
    for p in prepared {
        for prof in profiles {
            let at = |w: usize| {
                let imp = dive_impl(prof, w);
                let rs: Vec<&Run> = runs
                    .iter()
                    .filter(|r| r.bench == p.bench.name && r.implementation == imp)
                    .collect();
                summarise(&rs)
            };
            let one = at(1);
            for &w in workers {
                let s = at(w);
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    p.bench.name,
                    prof,
                    w,
                    fmt_opt(s.wall, 2),
                    fmt_opt(one.wall.zip(s.wall).map(|(a, b)| a / b.max(1e-3)), 2),
                    fmt_opt(s.cpu, 2),
                    fmt_opt(one.cpu.zip(s.cpu).map(|(a, b)| b / a.max(1e-3)), 2),
                );
            }
        }
    }
}

fn fmt_opt(x: Option<f64>, digits: usize) -> String {
    x.map_or("—".to_owned(), |v| format!("{v:.digits$}"))
}

/// Pair successful runs from the same repetition before computing a median.
/// Historical wall times and current CPU times are never comparable ratios.
fn paired_wall_ratio(
    runs: &[Run],
    bench: &str,
    baseline: &str,
    implementation: &str,
) -> Option<f64> {
    let ratios: Vec<f64> = runs
        .iter()
        .filter(|r| {
            r.correct && r.bench == bench && r.implementation == implementation && r.wall_s > 0.0
        })
        .filter_map(|r| {
            runs.iter()
                .find(|b| {
                    b.correct && b.bench == bench && b.implementation == baseline && b.rep == r.rep
                })
                .map(|b| b.wall_s / r.wall_s)
        })
        .collect();
    median(&ratios)
}

fn report(prepared: &[Prepared], impls: &[String], runs: &[Run], survey: &str) {
    println!("## Per run medians (CPU = user + sys)\n");
    println!(
        "| Benchmark | Implementation | CPU s (min–max) | wall s | distinct/CPU s | peak RSS MB | correct |"
    );
    println!("|---|---|---|---|---|---|---|");
    let mut by: BTreeMap<(String, String), Summary> = BTreeMap::new();
    for p in prepared {
        for imp in impls {
            let rs: Vec<&Run> = runs
                .iter()
                .filter(|r| r.bench == p.bench.name && &r.implementation == imp)
                .collect();
            let s = summarise(&rs);
            #[allow(clippy::cast_precision_loss, reason = "a rate for display")]
            let rate = p
                .expected
                .distinct
                .zip(s.cpu)
                .map(|(d, c)| d as f64 / c.max(1e-3));
            println!(
                "| {} | {} | {} ({}) | {} | {} | {} | {}/{} |",
                p.bench.name,
                imp,
                fmt_opt(s.cpu, 2),
                s.cpu_range
                    .map_or("—".to_owned(), |(a, b)| format!("{a:.2}–{b:.2}")),
                fmt_opt(s.wall, 2),
                fmt_opt(rate, 0),
                fmt_opt(s.rss_mb, 0),
                s.ok,
                s.n
            );
            by.insert((p.bench.name.clone(), imp.clone()), s);
        }
    }
    println!("\n## Against TLC and tla-mc at one worker\n");
    println!(
        "TLC here: this machine, `-workers 1`, wall (CPU). Golden and survey times are historical \
         context only. Speed-up is the median paired TLC/dive wall-time ratio from successful \
         runs of the same model configuration and repetition on this machine. Without --tlc, \
         no speed-up is claimed.\n"
    );
    println!(
        "| Benchmark | distinct | dive tlc CPU s | dive default CPU s | TLC here s | TLC golden s | TLC survey s | tla-mc survey s | speed-up (tlc profile) |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for p in prepared {
        let get = |imp: &str| by.get(&(p.bench.name.clone(), imp.to_owned()));
        let dt = get("dive-tlc").and_then(|s| s.cpu);
        let dd = get("dive-default").and_then(|s| s.cpu);
        let here = get("tlc");
        let sv = |tool: &str| {
            p.bench
                .survey
                .as_deref()
                .and_then(|s| survey_w1(survey, s, tool))
        };
        let tlc_survey = sv("tlc").map(|s| s.wall_s);
        #[allow(clippy::cast_precision_loss, reason = "seconds for display")]
        let golden = p.expected.elapsed_ms.map(|ms| ms as f64 / 1000.0);
        let speed = paired_wall_ratio(runs, &p.bench.name, "tlc", "dive-tlc");
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            p.bench.name,
            p.expected
                .distinct
                .map_or("?".to_owned(), |d| d.to_string()),
            fmt_opt(dt, 2),
            fmt_opt(dd, 2),
            here.map_or("—".to_owned(), |s| format!(
                "{} ({})",
                fmt_opt(s.wall, 2),
                fmt_opt(s.cpu, 2)
            )),
            fmt_opt(golden, 1),
            fmt_opt(tlc_survey, 2),
            fmt_opt(sv("tla-mc").map(|s| s.wall_s), 2),
            speed.map_or("—".to_owned(), |x| format!("{x:.2}×")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(implementation: &str, rep: usize, wall_s: f64, correct: bool) -> Run {
        Run {
            bench: "model".to_owned(),
            implementation: implementation.to_owned(),
            rep,
            usage: None,
            wall_s,
            verdict: "success".to_owned(),
            distinct: Some(1),
            generated: Some(1),
            correct,
            note: String::new(),
        }
    }

    #[test]
    fn ratios_require_successful_current_pairs() {
        let runs = [
            sample("tlc", 0, 10.0, true),
            sample("dive-tlc", 0, 5.0, true),
            sample("tlc", 1, 100.0, false),
            sample("dive-tlc", 1, 1.0, true),
            sample("dive-tlc", 2, 1.0, true),
        ];
        assert_eq!(
            paired_wall_ratio(&runs, "model", "tlc", "dive-tlc"),
            Some(2.0)
        );
        assert_eq!(
            paired_wall_ratio(&runs, "model", "historical", "dive-tlc"),
            None
        );
        let summary = summarise(&runs.iter().collect::<Vec<_>>());
        assert_eq!(summary.n, 5);
        assert_eq!(summary.ok, 4);
        assert_eq!(summary.wall, Some(3.0));
    }

    #[test]
    fn wrapper_arguments_quote_apostrophes_and_shell_syntax() {
        assert_eq!(shell_quote("it's $(a)"), "'it'\\''s $(a)'");
    }
}
