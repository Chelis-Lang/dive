//! `cargo xtask pluscal-corpus`: measures `dive translate` against the pinned
//! `pcal.trans` over every PlusCal algorithm in the pinned corpora.
//!
//! Each selected module is translated twice in its own scratch directory: once
//! by `pcal.trans` from the pinned `tla2tools.jar` (on the pinned JRE) and once
//! by `dive translate`. Optionally, when both translations succeed and the
//! source directory has a same-named `.cfg`, both translated modules are model
//! checked with `dive check` and the verdicts and counts are compared.
//! Everything is written under `--out`; nothing is written to the repository.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::{Result, workspace_root};

mod check_protocol;
mod pins;

const USAGE: &str =
    "usage: cargo xtask pluscal-corpus [--help] [--out DIR] [--dive BIN] [--cache DIR]
                       [--filter TEXT] [--timeout SECS] [--check] [--check-timeout SECS]

    --help               show this usage without loading pins or running children
    --out DIR            output directory (default <target-dir>/pluscal-corpus);
                         writes results.json, summary.md and work/<n>/
    --dive BIN           dive binary (default <target-dir>/release/dive)
    --cache DIR          conformance cache (default $DIVE_CACHE_DIR or ~/.cache/dive)
    --filter TEXT        only modules whose corpus path contains TEXT
    --timeout SECS       per-translation time limit (default 60)
    --check              model-check both translations of modules with a .cfg
    --check-timeout SECS per-check time limit (default 120)";

struct Config {
    help: bool,
    /// Module libraries in the cache (TLAPS, CommunityModules), passed to dive.
    libraries: Vec<PathBuf>,
    out: PathBuf,
    dive: PathBuf,
    cache: PathBuf,
    filter: Option<String>,
    timeout: Duration,
    check: bool,
    check_timeout: Duration,
}

/// A finished (or killed) child process.
struct Run {
    code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

/// Entry point of `cargo xtask pluscal-corpus`.
pub fn pluscal_corpus(args: &[String]) -> Result<()> {
    let mut config = parse_args(args)?;
    if config.help {
        println!("{USAGE}");
        return Ok(());
    }
    let root = workspace_root();
    let inputs = pins::load(&root, &config.cache)?;
    let java = inputs.java;
    let jar = inputs.jar;
    config.libraries = inputs.libraries;
    if !config.dive.is_file() {
        return Err(format!(
            "dive binary {} not found; build it with `cargo build --release -p dive-cli`",
            config.dive.display()
        ));
    }
    let specs = discover(&root, &config)?;
    if specs.is_empty() {
        return Err("no PlusCal modules found in the pinned corpora".to_owned());
    }
    let work = config.out.join("work");
    if work.exists() {
        std::fs::remove_dir_all(&work)
            .map_err(|e| format!("cannot clear {}: {e}", work.display()))?;
    }
    let mut records = Vec::new();
    for (index, (label, path)) in specs.iter().enumerate() {
        let dir = work.join(index.to_string());
        eprintln!("[{}/{}] {label}", index + 1, specs.len());
        records.push(measure(&config, &java, &jar, label, path, &dir)?);
    }
    let summary = summarize(&records);
    write(&config.out.join("results.json"), &{
        let mut text = serde_json::to_string_pretty(&json!({
            "jar": jar.display().to_string(),
            "java": java.display().to_string(),
            "libraries": config.libraries,
            "dive": config.dive.display().to_string(),
            "summary": summary.0,
            "specs": records,
        }))
        .map_err(|e| format!("cannot serialize results: {e}"))?;
        text.push('\n');
        text
    })?;
    write(&config.out.join("summary.md"), &summary.1)?;
    print!("{}", summary.1);
    println!("wrote {}", config.out.join("results.json").display());
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Config> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot resolve invocation directory: {e}"))?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace_root().join("target"), PathBuf::from);
    let cache = std::env::var_os("DIVE_CACHE_DIR").map_or_else(
        || {
            std::env::var_os("HOME")
                .map_or_else(PathBuf::new, PathBuf::from)
                .join(".cache/dive")
        },
        PathBuf::from,
    );
    parse_args_at(args, &cwd, &target, cache)
}

fn parse_args_at(args: &[String], cwd: &Path, target: &Path, cache: PathBuf) -> Result<Config> {
    let mut config = Config {
        help: false,
        libraries: Vec::new(),
        out: target.join("pluscal-corpus"),
        dive: target.join("release/dive"),
        cache,
        filter: None,
        timeout: Duration::from_secs(60),
        check: false,
        check_timeout: Duration::from_secs(120),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| USAGE.to_owned());
        match arg.as_str() {
            "--help" => config.help = true,
            "--out" => config.out = PathBuf::from(value()?),
            "--dive" => config.dive = PathBuf::from(value()?),
            "--cache" => config.cache = PathBuf::from(value()?),
            "--filter" => config.filter = Some(value()?),
            "--timeout" => config.timeout = seconds(&value()?)?,
            "--check-timeout" => config.check_timeout = seconds(&value()?)?,
            "--check" => config.check = true,
            _ => return Err(USAGE.to_owned()),
        }
    }
    // A child enters its module's scratch directory. Resolve the executable
    // while the invocation directory still owns relative CLI/default paths.
    if config.dive.is_relative() {
        config.dive = cwd.join(config.dive);
    }
    Ok(config)
}

/// Adds only the verified pinned module libraries.
fn with_libraries<'a>(command: &'a mut Command, config: &Config) -> &'a mut Command {
    for library in &config.libraries {
        command.arg("--library-path").arg(library);
    }
    command
}

fn seconds(text: &str) -> Result<Duration> {
    text.parse()
        .map(Duration::from_secs)
        .map_err(|_| format!("expected whole seconds, got {text}"))
}

/// Finds `bin/java` below the pinned JRE directory.
fn find_java(dir: &Path) -> Result<PathBuf> {
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((path, depth)) = stack.pop() {
        let candidate = path.join("bin/java");
        if candidate.is_file() {
            return Ok(candidate);
        }
        if depth < 6
            && let Ok(entries) = std::fs::read_dir(&path)
        {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    stack.push((entry.path(), depth + 1));
                }
            }
        }
    }
    Err(format!(
        "no pinned JRE under {}; run `cargo run -p dive-conformance --release -- oracle fetch`",
        dir.display()
    ))
}

/// Every `.tla` file with a PlusCal algorithm in Examples and tlaplus tests.
fn discover(root: &Path, config: &Config) -> Result<Vec<(String, PathBuf)>> {
    let mut specs = Vec::new();
    for id in ["examples", "tlaplus-tests"] {
        let base = pins::corpus_dir(root, &config.cache, id)?;
        specs.extend(discover_dir(&base, id, config.filter.as_deref())?);
    }
    Ok(specs)
}

fn discover_dir(base: &Path, id: &str, filter: Option<&str>) -> Result<Vec<(String, PathBuf)>> {
    let mut specs = Vec::new();
    let mut files = Vec::new();
    collect_tla(base, &mut files)?;
    files.sort();
    for file in files {
        let relative = file
            .strip_prefix(base)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        // Translator sources, not inputs.
        if relative.contains("/src/") {
            continue;
        }
        if let Some(filter) = filter
            && !relative.contains(filter)
        {
            continue;
        }
        let text = read(&file)?;
        if has_algorithm(&text) {
            specs.push((format!("{id}:{relative}"), file));
        }
    }
    Ok(specs)
}

fn collect_tla(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot enumerate pinned corpus {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot enumerate pinned corpus {}: {e}", dir.display()))?
            .path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if path.is_dir() {
            collect_tla(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "tla") {
            out.push(path);
        }
    }
    Ok(())
}

fn has_algorithm(text: &str) -> bool {
    text.contains("--algorithm")
        || text.match_indices("--fair").any(|(at, _)| {
            text[at + "--fair".len()..]
                .trim_start()
                .starts_with("algorithm")
        })
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))
}

/// Runs `command` in `dir`, killing it after `timeout`.
fn run(command: &mut Command, dir: &Path, name: &str, timeout: Duration) -> Result<Run> {
    let stdout_path = dir.join(format!("{name}.stdout"));
    let stderr_path = dir.join(format!("{name}.stderr"));
    let stdout =
        File::create(&stdout_path).map_err(|e| format!("cannot create stdout file: {e}"))?;
    let stderr =
        File::create(&stderr_path).map_err(|e| format!("cannot create stderr file: {e}"))?;
    let mut child = command
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| format!("failed to run {command:?}: {e}"))?;
    let start = Instant::now();
    let (code, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status.code(), false),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break (None, true);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("cannot wait for {command:?}: {e}")),
        }
    };
    Ok(Run {
        code,
        timed_out,
        stdout: std::fs::read_to_string(&stdout_path).unwrap_or_default(),
        stderr: std::fs::read_to_string(&stderr_path).unwrap_or_default(),
    })
}

/// The pcal.trans error text: lines after the version banner, minus progress noise.
fn pcal_message(run: &Run) -> String {
    let mut lines = Vec::new();
    for line in run.stdout.lines().chain(run.stderr.lines()) {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("pcal.trans Version")
            || trimmed.starts_with("Parsing completed")
            || trimmed.starts_with("Translation completed")
            || trimmed.starts_with("New file")
            || trimmed.starts_with("File ")
            || trimmed.starts_with("so new one not written")
            || trimmed.starts_with("Picked up JAVA_TOOL_OPTIONS")
        {
            continue;
        }
        lines.push(trimmed.to_owned());
    }
    lines.join("\n")
}

/// Significant text of a translation block, with whitespace collapsed.
fn translation_block(module: &str) -> Option<String> {
    let begin = module.find("\\* BEGIN TRANSLATION")?;
    let end = module[begin..].find("\\* END TRANSLATION")? + begin;
    let body = &module[begin..end];
    let body = body.split_once('\n').map_or(body, |(_, rest)| rest);
    Some(body.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn measure(
    config: &Config,
    java: &Path,
    jar: &Path,
    label: &str,
    source: &Path,
    dir: &Path,
) -> Result<Value> {
    let name = source
        .file_name()
        .ok_or_else(|| format!("{} has no file name", source.display()))?;
    let stem = source.file_stem().unwrap_or(name).to_owned();
    let source_dir = source.parent().unwrap_or(Path::new("."));
    let cfg = source.with_extension("cfg");
    let pcal_dir = dir.join("pcal");
    let dive_dir = dir.join("dive");
    for d in [&pcal_dir, &dive_dir] {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
        copy(source, &d.join(name))?;
        if cfg.is_file() {
            copy(&cfg, &d.join(Path::new(&stem).with_extension("cfg")))?;
        }
    }

    let pcal = run(
        Command::new(java)
            .arg("-XX:+UseSerialGC")
            .arg("-cp")
            .arg(jar)
            .arg("pcal.trans")
            .arg(name),
        &pcal_dir,
        "pcal",
        config.timeout,
    )?;
    let pcal_ok = pcal.code == Some(0) && !pcal.timed_out;
    let pcal_message = pcal_message(&pcal);

    let dive = run(
        with_libraries(Command::new(&config.dive).arg("translate"), config)
            .arg(name)
            // pcal.trans always replaces an existing translation.
            .arg("--rewrite")
            .arg("--checksums")
            .arg("tla")
            .arg("--replace-modified")
            .arg("--output")
            .arg("json")
            .arg("--module-path")
            .arg(source_dir),
        &dive_dir,
        "dive",
        config.timeout,
    )?;
    let report: Value = serde_json::from_str(&dive.stdout)
        .or_else(|_| serde_json::from_str(dive.stderr.trim()))
        .unwrap_or(Value::Null);
    let dive_status = match (dive.timed_out, dive.code) {
        (true, _) => "timeout",
        (false, Some(0)) => "artifact",
        (false, Some(1)) => "rejected",
        (false, Some(3)) => "unsupported",
        (false, Some(4)) => "internal",
        _ => "error",
    };
    let dive_message = diagnostic_message(&report, &dive);
    let dive_module = report["replacement"]["module_source"]
        .as_str()
        .or_else(|| report["module_source"].as_str())
        .map(str::to_owned);
    if let Some(module) = &dive_module {
        write(&dive_dir.join("translated.tla"), module)?;
    }

    let category = match (pcal_ok, dive_status) {
        _ if pcal.timed_out => "pcal-timeout",
        (_, "timeout" | "internal" | "error") => "dive-error",
        (true, "artifact") => "ok",
        (_, "unsupported") => "refused",
        (false, "rejected") => "both-reject",
        _ => "error-mismatch",
    };

    let mut record = json!({
        "spec": label,
        "category": category,
        "pcal": {"exit": pcal.code, "timed_out": pcal.timed_out, "message": pcal_message},
        "dive": {"exit": dive.code, "status": dive_status, "message": dive_message},
    });

    if category == "ok" {
        let pcal_module = std::fs::read_to_string(pcal_dir.join(name)).unwrap_or_default();
        let same = match (
            translation_block(&pcal_module),
            dive_module.as_deref().and_then(translation_block),
        ) {
            (Some(a), Some(b)) => Some(a == b),
            _ => None,
        };
        record["same_text"] = json!(same);
        if config.check && cfg.is_file() {
            record["check"] =
                compare_checks(config, &pcal_dir, &dive_dir, name, &stem, source_dir)?;
        }
    }
    Ok(record)
}

/// Model-checks both translations with the `.cfg` that pcal.trans rewrote.
fn compare_checks(
    config: &Config,
    pcal_dir: &Path,
    dive_dir: &Path,
    name: &std::ffi::OsStr,
    stem: &std::ffi::OsStr,
    source_dir: &Path,
) -> Result<Value> {
    let module = Path::new(name);
    let cfg_name = Path::new(stem).with_extension("cfg");
    copy(&pcal_dir.join(&cfg_name), &dive_dir.join(&cfg_name))?;
    copy(&dive_dir.join("translated.tla"), &dive_dir.join(module))?;
    let a = check(config, pcal_dir, module, &cfg_name, source_dir)?;
    let b = check(config, dive_dir, module, &cfg_name, source_dir)?;
    let outcome = check_outcome(&a, &b);
    Ok(json!({"pcal": a, "dive": b, "outcome": outcome}))
}

fn diagnostic_message(report: &Value, run: &Run) -> String {
    let mut found = Vec::new();
    find_messages(report, &mut found);
    if found.is_empty() {
        let text = run.stderr.trim();
        if text.is_empty() {
            String::new()
        } else {
            text.lines().take(4).collect::<Vec<_>>().join("\n")
        }
    } else {
        found.join("\n")
    }
}

/// Collects `diagnostic.code: message` pairs anywhere in a dive JSON report.
fn find_messages(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let (Some(code), Some(message)) = (
                map.get("code").and_then(Value::as_str),
                map.get("message").and_then(Value::as_str),
            ) {
                out.push(format!("{code}: {message}"));
                return;
            }
            for item in map.values() {
                find_messages(item, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                find_messages(item, out);
            }
        }
        _ => {}
    }
}

fn check(
    config: &Config,
    dir: &Path,
    module: &Path,
    cfg: &Path,
    module_path: &Path,
) -> Result<Value> {
    let run = run(
        with_libraries(Command::new(&config.dive).arg("check"), config)
            .arg(module)
            .arg("--config")
            .arg(cfg)
            .arg("--workers")
            .arg("1")
            .arg("--output")
            .arg("json")
            .arg("--module-path")
            .arg(module_path),
        dir,
        "check",
        config.check_timeout,
    )?;
    let report: Value = serde_json::from_str(&run.stdout).unwrap_or(Value::Null);
    Ok(check_record(&run, &report))
}

fn check_record(run: &Run, report: &Value) -> Value {
    json!({
        "exit": run.code,
        "timed_out": run.timed_out,
        "verdict": report.get("verdict").cloned(),
        "property": report.get("property").cloned(),
        "distinct": report["statistics"].get("distinct").cloned(),
        "generated": report["statistics"].get("generated").cloned(),
        "depth": report["statistics"].get("depth").cloned(),
        "tlc": report.get("tlc").cloned(),
        "valid_report": valid_check_report(run, report),
    })
}

// Validate the observed native protocol before comparing it. Equality of
// missing fields is not a model outcome. The default/TLC exit pair comes from
// dive-api's public check contract; no compiler crate enters the hook graph.
fn valid_check_report(run: &Run, report: &Value) -> bool {
    if report["command"] != "check" {
        return false;
    }
    let Some(verdict) = report["verdict"].as_str() else {
        return false;
    };
    if matches!(verdict, "unsupported" | "resource-limit" | "internal-error") {
        return true; // Classified as uncredited below, never as agreement.
    }
    let (native_exit, needs_stats, needs_property) = match verdict {
        "success" => (0, true, false),
        "invariant-violation"
        | "property-violation"
        | "action-property-violation"
        | "liveness-violation" => (10, true, true),
        "deadlock" => (11, true, false),
        "postcondition-failure" => (10, false, true),
        "assumption-failure" => (12, false, false),
        "model-error" | "rejected" => (1, false, false),
        "incomplete-state" | "assertion-failure" | "overflow" | "evaluation-error" => {
            (13, false, false)
        }
        _ => return false,
    };
    let Some(stats) = report.get("statistics") else {
        return false;
    };
    if stats.is_null() {
        if needs_stats {
            return false;
        }
    } else {
        let (Some(distinct), Some(generated), Some(_depth)) = (
            stats["distinct"].as_u64(),
            stats["generated"].as_u64(),
            stats["depth"].as_u64(),
        ) else {
            return false;
        };
        // Every distinct state was generated. Empty Init can legally leave
        // both counters zero; depth remains an observed unsigned value.
        if generated < distinct {
            return false;
        }
    }
    let property = report.get("property");
    if needs_property {
        if property.is_none_or(|p| p.as_str().is_none_or(str::is_empty)) {
            return false;
        }
    } else if property.is_some_and(|p| !p.is_null()) {
        return false;
    }
    let class = &report["tlc"];
    let Some(code) = class["code"]
        .as_u64()
        .filter(|&c| c >= 1000 && u32::try_from(c).is_ok())
    else {
        return false;
    };
    let Some(tlc_exit) = class["exit_status"].as_u64().filter(|&c| c <= 255) else {
        return false;
    };
    let Some(name) = class["name"].as_str() else {
        return false;
    };
    let Some((_, expected_name, expected_exit, verdicts)) = check_protocol::class(code) else {
        return false;
    };
    if class["verified"] != true
        || !class.get("reported").map_or_else(
            // `CheckReport::to_json` omits this field only for frontend SANY
            // rejection; all checked-result classes carry it, even success.
            || verdict == "rejected" && code == 3002,
            |reported| {
                reported.as_u64().is_some_and(|reported| {
                    check_protocol::reported_matches(verdict, code, reported)
                })
            },
        )
        || name != *expected_name
        || tlc_exit != *expected_exit
        || !verdicts.contains(&verdict)
    {
        return false;
    }
    // Assumption evaluation errors use the default assumption exit. A real
    // TLC-profile run instead uses the declared deciding class's exit.
    run.code.is_some_and(|exit| {
        exit == if code == 2105 { 12 } else { native_exit }
            || u64::try_from(exit).ok() == Some(tlc_exit)
    })
}

/// Refusals and missing reports establish no model-check agreement.
fn check_outcome(a: &Value, b: &Value) -> &'static str {
    if a["timed_out"] == true || b["timed_out"] == true {
        return "timeout";
    }
    if a["valid_report"] != true || b["valid_report"] != true {
        return "error";
    }
    for record in [a, b] {
        match record["verdict"].as_str() {
            Some("unsupported" | "resource-limit") => return "refused",
            Some("internal-error") => return "error",
            _ => {}
        }
    }
    if [
        "verdict",
        "property",
        "distinct",
        "generated",
        "depth",
        "tlc",
        "exit",
    ]
    .iter()
    .all(|key| a[key] == b[key])
    {
        "agree"
    } else {
        "disagree"
    }
}

fn summarize(records: &[Value]) -> (Value, String) {
    let mut categories: BTreeMap<String, usize> = BTreeMap::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut checks: BTreeMap<String, usize> = BTreeMap::new();
    let mut same = 0;
    for record in records {
        let category = record["category"].as_str().unwrap_or("?").to_owned();
        if category == "refused" {
            let message = record["dive"]["message"].as_str().unwrap_or("");
            *reasons.entry(reason_key(message)).or_default() += 1;
        }
        if record["same_text"] == json!(true) {
            same += 1;
        }
        if let Some(outcome) = record["check"]["outcome"].as_str() {
            *checks.entry(outcome.to_owned()).or_default() += 1;
        }
        *categories.entry(category).or_default() += 1;
    }
    let mut text = String::new();
    let _ = writeln!(text, "# PlusCal corpus: dive translate vs pcal.trans\n");
    let _ = writeln!(text, "Modules: {}\n", records.len());
    let _ = writeln!(text, "| Category | Count |\n|---|---:|");
    for (category, count) in &categories {
        let _ = writeln!(text, "| {category} | {count} |");
    }
    let _ = writeln!(
        text,
        "\nTranslated by both with whitespace-equal text: {same}\n"
    );
    if !checks.is_empty() {
        let _ = writeln!(text, "| Model check | Count |\n|---|---:|");
        for (outcome, count) in &checks {
            let _ = writeln!(text, "| {outcome} | {count} |");
        }
        let _ = writeln!(text);
    }
    let mut ranked: Vec<_> = reasons.iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    if !ranked.is_empty() {
        let _ = writeln!(text, "| Refusal | Count |\n|---|---:|");
        for (reason, count) in ranked {
            let _ = writeln!(text, "| {} | {count} |", reason.replace('|', "\\|"));
        }
        let _ = writeln!(text);
    }
    let mut mismatches = String::new();
    for record in records {
        let category = record["category"].as_str().unwrap_or("");
        if matches!(category, "error-mismatch" | "dive-error")
            || record["check"]["outcome"] == json!("disagree")
        {
            let _ = writeln!(
                mismatches,
                "- `{}` ({category}{}): pcal: {} / dive: {}",
                record["spec"].as_str().unwrap_or(""),
                if record["check"]["outcome"] == json!("disagree") {
                    ", check disagrees"
                } else {
                    ""
                },
                first_line(record["pcal"]["message"].as_str().unwrap_or("")),
                first_line(record["dive"]["message"].as_str().unwrap_or("")),
            );
        }
    }
    if !mismatches.is_empty() {
        let _ = writeln!(text, "Mismatches:\n\n{mismatches}");
    }
    let summary = json!({
        "modules": records.len(),
        "categories": categories,
        "same_text": same,
        "checks": checks,
        "refusals": reasons,
    });
    (summary, text)
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// A refusal message with digits and quoted names erased, for grouping.
fn reason_key(message: &str) -> String {
    let line = first_line(message);
    let line = line.split_once(": ").map_or(line, |(_, rest)| rest);
    let mut key = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '`' | '"' => {
                quoted = !quoted;
                key.push('…');
            }
            _ if quoted => {}
            c if c.is_ascii_digit() => {
                if !key.ends_with('#') {
                    key.push('#');
                }
            }
            c => key.push(c),
        }
    }
    key.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Result<Self> {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            loop {
                let path = std::env::temp_dir().join(format!(
                    "dive-pluscal-options-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Ok(Self(path)),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
        }

        fn put(&self, path: &str, bytes: &[u8]) -> Result<PathBuf> {
            let path = self.0.join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            Ok(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn options_keep_defaults_and_accept_only_the_documented_surface() -> Result<()> {
        let cwd = std::env::temp_dir().join("invocation");
        let parse = |args: &[&str]| {
            parse_args_at(
                &args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
                &cwd,
                Path::new("build"),
                "cache".into(),
            )
        };
        let defaults = parse(&[])?;
        assert_eq!(defaults.out, Path::new("build/pluscal-corpus"));
        assert_eq!(defaults.dive, cwd.join("build/release/dive"));
        assert_eq!(defaults.cache, Path::new("cache"));
        assert_eq!(defaults.filter, None);
        assert_eq!(defaults.timeout, Duration::from_secs(60));
        assert_eq!(defaults.check_timeout, Duration::from_secs(120));
        assert!(!defaults.check && !defaults.help);
        let explicit = parse(&[
            "--out",
            "results",
            "--dive",
            "bin/dive",
            "--cache",
            "pins",
            "--filter",
            "Lock.tla",
            "--timeout",
            "0",
            "--check-timeout",
            "7",
            "--check",
            "--help",
        ])?;
        assert_eq!(explicit.out, Path::new("results"));
        assert_eq!(explicit.dive, cwd.join("bin/dive"));
        assert_eq!(explicit.cache, Path::new("pins"));
        assert_eq!(explicit.filter.as_deref(), Some("Lock.tla"));
        assert_eq!(explicit.timeout, Duration::ZERO);
        assert_eq!(explicit.check_timeout, Duration::from_secs(7));
        assert!(explicit.check && explicit.help);
        for flag in [
            "--out",
            "--dive",
            "--cache",
            "--filter",
            "--timeout",
            "--check-timeout",
        ] {
            assert!(parse(&[flag]).is_err(), "missing value: {flag}");
        }
        for flag in ["--unknown", "--label", "--compat=tlc", "--jobs"] {
            assert!(parse(&[flag]).is_err(), "unimplemented option: {flag}");
        }
        for seconds in ["-1", "1.5", "NaN", "18446744073709551616"] {
            assert!(parse(&["--timeout", seconds]).is_err(), "{seconds}");
        }
        Ok(())
    }

    #[test]
    fn discovery_filters_sorted_paths_and_never_drops_unreadable_inputs() -> Result<()> {
        let scratch = Scratch::new()?;
        scratch.put("beta/B.tla", b"(* --fair \n algorithm B *)")?;
        scratch.put("alpha/A.tla", b"(* --algorithm A *)")?;
        scratch.put("alpha/A.cfg", b"--algorithm not a TLA module")?;
        scratch.put("notes/Normal.tla", b"ordinary TLA module")?;
        scratch.put(".git/Hidden.tla", b"--algorithm ignored metadata")?;
        scratch.put(
            "translator/src/Internal.tla",
            b"--algorithm translator source",
        )?;
        let labels = |rows: Vec<(String, PathBuf)>| {
            rows.into_iter().map(|(label, _)| label).collect::<Vec<_>>()
        };
        assert_eq!(
            labels(discover_dir(&scratch.0, "fixture", None)?),
            ["fixture:alpha/A.tla", "fixture:beta/B.tla"]
        );
        assert_eq!(
            labels(discover_dir(&scratch.0, "fixture", Some("alpha/"))?),
            ["fixture:alpha/A.tla"]
        );
        scratch.put("alpha/Unreadable.tla", &[0xff])?;
        assert!(discover_dir(&scratch.0, "fixture", None).is_err());
        assert!(discover_dir(&scratch.0, "fixture", Some("alpha/")).is_err());
        assert!(discover_dir(&scratch.0.join("missing"), "fixture", None).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn relative_binary_and_timeout_work_after_scratch_chdir() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = Scratch::new()?;
        let binary = scratch.put(
            "bin/native",
            b"#!/bin/sh\nprintf 'relative executable reached\\n'\n",
        )?;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let child_dir = scratch.0.join("work/0/dive");
        std::fs::create_dir_all(&child_dir).map_err(|e| e.to_string())?;
        let config = parse_args_at(
            &["--dive".to_owned(), "bin/native".to_owned()],
            &scratch.0,
            Path::new("build"),
            "cache".into(),
        )?;
        let observed = run(
            &mut Command::new(&config.dive),
            &child_dir,
            "relative",
            Duration::from_secs(5),
        )?;
        assert_eq!(observed.code, Some(0));
        assert!(!observed.timed_out);
        assert_eq!(observed.stdout, "relative executable reached\n");
        let stopped = run(
            Command::new("sh").args(["-c", "while :; do :; done"]),
            &child_dir,
            "deadline",
            Duration::from_millis(10),
        )?;
        assert!(stopped.timed_out);
        assert_eq!(stopped.code, None);
        let record = check_record(&stopped, &Value::Null);
        assert_eq!(check_outcome(&record, &record), "timeout");
        Ok(())
    }

    #[test]
    fn malformed_generated_counters_never_receive_agreement() -> Result<()> {
        let rows: Vec<Value> =
            serde_json::from_str(include_str!("pluscal_corpus/fixtures/native-reports.json"))
                .map_err(|e| e.to_string())?;
        let mut cases = 0;
        for row in rows {
            let report = &row["report"];
            if report["statistics"].is_null() {
                continue;
            }
            let id = row["id"].as_str().ok_or("native id")?;
            let distinct = report["statistics"]["distinct"]
                .as_u64()
                .ok_or("distinct")?;
            if distinct == 0 {
                continue;
            }
            let exit = i32::try_from(row["exit"].as_i64().ok_or("native exit")?)
                .map_err(|e| e.to_string())?;
            let good = observed(report, exit);
            assert_eq!(check_outcome(&good, &good), "agree", "{id}");
            let mut changed = report.clone();
            // For each success control this is exactly generated2 ->0,
            // preserving distinct1/depth1 and every other field.
            changed["statistics"]["generated"] = json!(distinct - 1);
            let bad = observed(&changed, exit);
            assert_eq!(check_outcome(&bad, &bad), "error", "{id}: {changed}");
            assert_eq!(check_outcome(&good, &bad), "error", "{id}: {changed}");
            assert_eq!(check_outcome(&bad, &good), "error", "{id}: {changed}");
            cases += 1;
        }
        assert_eq!(cases, 8);
        Ok(())
    }

    #[test]
    fn malformed_statistics_omission_never_substitutes_for_explicit_null() -> Result<()> {
        let rows: Vec<Value> =
            serde_json::from_str(include_str!("pluscal_corpus/fixtures/native-reports.json"))
                .map_err(|e| e.to_string())?;
        let mut cases = 0;
        for row in rows {
            if ![json!(2222), json!(3002), json!(2104)].contains(&row["report"]["tlc"]["code"]) {
                continue;
            }
            let id = row["id"].as_str().ok_or("native id")?;
            let report = &row["report"];
            assert_eq!(report["statistics"], Value::Null, "{id}");
            assert_eq!(report["property"], Value::Null, "{id}");
            let exit = i32::try_from(row["exit"].as_i64().ok_or("native exit")?)
                .map_err(|e| e.to_string())?;
            let good = observed(report, exit);
            assert_eq!(check_outcome(&good, &good), "agree", "{id}");
            let mut changed = report.clone();
            let _ = changed
                .as_object_mut()
                .ok_or("report object")?
                .remove("statistics");
            assert_eq!(changed["property"], Value::Null, "{id}");
            let bad = observed(&changed, exit);
            assert_eq!(check_outcome(&bad, &bad), "error", "{id}: {changed}");
            assert_eq!(check_outcome(&good, &bad), "error", "{id}: {changed}");
            assert_eq!(check_outcome(&bad, &good), "error", "{id}: {changed}");
            cases += 1;
        }
        assert_eq!(cases, 6);
        Ok(())
    }

    #[test]
    fn actual_native_reports_keep_reported_traps_and_legal_exits_distinct() -> Result<()> {
        let rows: Value =
            serde_json::from_str(include_str!("pluscal_corpus/fixtures/native-reports.json"))
                .map_err(|e| e.to_string())?;
        let rows = rows.as_array().ok_or("native report rows")?;
        let mut traps = 0;
        for row in rows {
            let id = row["id"].as_str().ok_or("native report id")?;
            let exit = row["exit"].as_i64().ok_or("native process exit")?;
            let exit = i32::try_from(exit).map_err(|e| e.to_string())?;
            let report = &row["report"];
            let good = observed(report, exit);
            assert_eq!(check_outcome(&good, &good), "agree", "{id}: {report}");
            if let Some(forged) = row["forged_reported"].as_u64() {
                let mut bad = report.clone();
                bad["tlc"]["reported"] = json!(forged);
                let bad = observed(&bad, exit);
                assert_eq!(check_outcome(&bad, &bad), "error", "{id}: {bad}");
                assert_eq!(check_outcome(&good, &bad), "error", "{id}: {bad}");
                traps += 1;
            }
            let timed_out = check_record(
                &Run {
                    code: None,
                    timed_out: true,
                    stdout: String::new(),
                    stderr: String::new(),
                },
                report,
            );
            assert_eq!(check_outcome(&good, &timed_out), "timeout", "{id}");
            if id.starts_with("empty-init-") {
                assert_eq!(
                    report["statistics"],
                    json!({"generated":0,"distinct":0,"depth":0})
                );
                // This is a synthetic legal depth boundary, not a claim that
                // either current native profile observed depth1 on empty Init.
                let mut depth_one = report.clone();
                depth_one["statistics"]["depth"] = json!(1);
                let depth_one = observed(&depth_one, exit);
                assert_eq!(check_outcome(&depth_one, &depth_one), "agree", "{id}");
                assert_eq!(check_outcome(&good, &depth_one), "disagree", "{id}");
            }
            if report["verdict"] == "invariant-violation" {
                for (key, value) in [
                    ("property", json!("Other")),
                    ("statistics", json!({"generated":4,"distinct":3,"depth":3})),
                ] {
                    let mut other = report.clone();
                    other[key] = value;
                    let other = observed(&other, exit);
                    assert_eq!(check_outcome(&good, &other), "disagree", "{id}: {other}");
                }
            }
        }
        assert_eq!(rows.len(), 16);
        assert_eq!(traps, 6);
        Ok(())
    }

    fn record(report: &Value) -> Value {
        check_record(
            &Run {
                code: Some(if report["verdict"] == "success" {
                    0
                } else {
                    10
                }),
                timed_out: false,
                stdout: String::new(),
                stderr: String::new(),
            },
            report,
        )
    }

    #[test]
    fn missing_reports_and_matching_refusals_are_uncredited() {
        let absent = record(&Value::Null);
        assert_eq!(check_outcome(&absent, &absent), "error");
        for verdict in ["unsupported", "resource-limit", "internal-error"] {
            let refusal = record(&json!({"command": "check", "verdict": verdict}));
            assert_ne!(check_outcome(&refusal, &refusal), "agree");
        }
    }

    #[test]
    fn observable_property_depth_and_error_class_decide_agreement() {
        let base = record(
            &json!({"command": "check", "verdict": "invariant-violation",
            "property": "Safe", "statistics": {"distinct": 3, "generated": 4, "depth": 2},
            "tlc": {"code": 2110, "name": "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                "exit_status": 12, "reported": 2110, "verified": true}}),
        );
        assert_eq!(check_outcome(&base, &base), "agree");
        for (key, value) in [
            ("property", json!("Other")),
            ("depth", json!(3)),
            ("tlc", json!({"code": 2111, "exit_status": 12})),
        ] {
            let mut changed = base.clone();
            changed[key] = value;
            assert_eq!(check_outcome(&base, &changed), "disagree");
        }
    }

    #[test]
    fn incomplete_unknown_and_inconsistent_streams_never_agree() {
        let success = json!({"command": "check", "verdict": "success", "property": null,
            "statistics": {"generated": 21, "distinct": 12, "depth": 5},
            "tlc": {"code": 2193, "name": "TLC_SUCCESS", "reported": 2193,
                "exit_status": 0, "verified": true}});
        let good = record(&success);
        assert_eq!(check_outcome(&good, &good), "agree");
        let mut invalid = vec![
            json!({"command": "check", "verdict": "success"}),
            json!({"command": "check", "verdict": "banana"}),
        ];
        for (key, value) in [
            ("statistics", Value::Null),
            (
                "statistics",
                json!({"generated": -4, "distinct": "twelve", "depth": true}),
            ),
            ("tlc", Value::Null),
            (
                "tlc",
                json!({"code": 0, "name": "banana", "reported": 0,
                "exit_status": 0, "verified": true}),
            ),
        ] {
            let mut bad = success.clone();
            bad[key] = value;
            invalid.push(bad);
        }
        let mut unverified = success.clone();
        unverified["tlc"]["verified"] = json!(false);
        invalid.push(unverified);
        for bad in invalid {
            let observed = record(&bad);
            assert_eq!(check_outcome(&observed, &observed), "error", "{bad}");
        }
        let failing_success = check_record(
            &Run {
                code: Some(1),
                timed_out: false,
                stdout: String::new(),
                stderr: String::new(),
            },
            &success,
        );
        assert_eq!(check_outcome(&failing_success, &failing_success), "error");
    }

    #[test]
    fn valid_before_search_failure_keeps_null_statistics() {
        let failure = json!({"command": "check", "verdict": "model-error",
            "property": null, "statistics": null,
            "tlc": {"code": 2222, "name": "TLC_CONFIG_VALUE_NOT_ASSIGNED_TO_CONSTANT_PARAM",
                "exit_status": 151, "reported": 2222, "verified": true}});
        let observed = check_record(
            &Run {
                code: Some(1),
                timed_out: false,
                stdout: String::new(),
                stderr: String::new(),
            },
            &failure,
        );
        assert_eq!(check_outcome(&observed, &observed), "agree");
        // The frontend rejection schema has no separate `reported` code;
        // its complete claimed class and native exit remain observable.
        let rejected = json!({"command": "check", "verdict": "rejected",
            "statistics": null, "property": null,
            "tlc": {"code": 3002, "name": "TLC_PARSING_FAILED",
                "exit_status": 150, "verified": true}});
        let observed = check_record(
            &Run {
                code: Some(1),
                timed_out: false,
                stdout: String::new(),
                stderr: String::new(),
            },
            &rejected,
        );
        assert_eq!(check_outcome(&observed, &observed), "agree");
        let mut bad = failure;
        bad["statistics"] = json!({"generated": 1});
        let observed = record(&bad);
        assert_eq!(check_outcome(&observed, &observed), "error");
    }

    fn observed(report: &Value, exit: i32) -> Value {
        check_record(
            &Run {
                code: Some(exit),
                timed_out: false,
                stdout: String::new(),
                stderr: String::new(),
            },
            report,
        )
    }

    #[test]
    fn identical_malformed_classes_and_verdicts_never_agree() {
        // The six streams reproduced by the protocol review, not merely one
        // changed side of an otherwise valid comparison.
        for (verdict, code, name, tlc_exit, exit) in [
            ("model-error", 9999, "TLC_BANANA", 151, 1),
            ("invariant-violation", 2110, "TLC_BANANA", 12, 10),
            (
                "invariant-violation",
                2110,
                "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                151,
                10,
            ),
            (
                "invariant-violation",
                2110,
                "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                151,
                151,
            ),
            (
                "model-error",
                2110,
                "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                12,
                1,
            ),
            ("rejected", 2110, "TLC_INVARIANT_VIOLATED_BEHAVIOR", 12, 1),
            (
                "evaluation-error",
                2110,
                "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                12,
                13,
            ),
            (
                "evaluation-error",
                2222,
                "TLC_CONFIG_VALUE_NOT_ASSIGNED_TO_CONSTANT_PARAM",
                151,
                13,
            ),
            ("model-error", 3002, "TLC_PARSING_FAILED", 150, 1),
            ("rejected", 2179, "TLC_MODULE_DIVISION_BY_ZERO", 255, 1),
            (
                "evaluation-error",
                2111,
                "TLC_INVARIANT_EVALUATION_FAILED",
                76,
                12,
            ),
            (
                "invariant-violation",
                2110,
                "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                12,
                151,
            ),
        ] {
            let report = json!({"command": "check", "verdict": verdict,
                "property": if verdict == "invariant-violation" { json!("Safe") } else { Value::Null },
                "statistics": {"generated": 4, "distinct": 3, "depth": 2},
                "tlc": {"code": code, "name": name, "exit_status": tlc_exit,
                    "reported": code, "verified": true}});
            let record = observed(&report, exit);
            assert_eq!(
                check_outcome(&record, &record),
                "error",
                "{report} exit {exit}"
            );
        }
    }

    #[test]
    fn legal_native_and_tlc_exits_and_distinct_reported_codes_remain_valid() {
        for (verdict, code, name, tlc_exit, native_exit, reported) in [
            (
                "model-error",
                2222,
                "TLC_CONFIG_VALUE_NOT_ASSIGNED_TO_CONSTANT_PARAM",
                151,
                1,
                Some(2222),
            ),
            ("rejected", 3002, "TLC_PARSING_FAILED", 150, 1, None),
            (
                "invariant-violation",
                2110,
                "TLC_INVARIANT_VIOLATED_BEHAVIOR",
                12,
                10,
                Some(2110),
            ),
            (
                "evaluation-error",
                2103,
                "TLC_NESTED_EXPRESSION",
                75,
                13,
                Some(1000),
            ),
            (
                "evaluation-error",
                2103,
                "TLC_NESTED_EXPRESSION",
                75,
                13,
                Some(2111),
            ),
            (
                "evaluation-error",
                2103,
                "TLC_NESTED_EXPRESSION",
                75,
                13,
                Some(2102),
            ),
            (
                "assertion-failure",
                2132,
                "TLC_VALUE_ASSERT_FAILED",
                14,
                13,
                Some(2113),
            ),
            ("overflow", 2178, "TLC_MODULE_OVERFLOW", 255, 13, Some(2111)),
            (
                "evaluation-error",
                2111,
                "TLC_INVARIANT_EVALUATION_FAILED",
                76,
                13,
                Some(2111),
            ),
            (
                "evaluation-error",
                2105,
                "TLC_ASSUMPTION_EVALUATION_ERROR",
                10,
                12,
                Some(2105),
            ),
        ] {
            let mut report = json!({"command": "check", "verdict": verdict,
                "property": if verdict == "invariant-violation" { json!("Safe") } else { Value::Null },
                "statistics": if verdict == "invariant-violation" { json!({"generated": 4, "distinct": 3, "depth": 2}) } else { Value::Null },
                "tlc": {"code": code, "name": name, "exit_status": tlc_exit, "verified": true}});
            if let Some(reported) = reported {
                report["tlc"]["reported"] = json!(reported);
            }
            for exit in [native_exit, tlc_exit] {
                let record = observed(&report, exit);
                assert_eq!(
                    check_outcome(&record, &record),
                    "agree",
                    "{report} exit {exit}"
                );
            }
            report["tlc"]["reported"] = json!(9999);
            let record = observed(&report, native_exit);
            assert_eq!(check_outcome(&record, &record), "error", "{report}");
        }
    }

    #[test]
    fn impossible_or_missing_reported_classes_never_agree() {
        for (verdict, code, property, stats, native_exit, wrong_reported) in [
            (
                "success",
                2193,
                Value::Null,
                json!({"distinct": 12, "generated": 21, "depth": 5}),
                0,
                3002,
            ),
            (
                "invariant-violation",
                2110,
                json!("Safe"),
                json!({"distinct": 3, "generated": 4, "depth": 2}),
                10,
                2193,
            ),
            ("model-error", 2222, Value::Null, Value::Null, 1, 2110),
            ("model-error", 2132, Value::Null, Value::Null, 1, 1000),
            ("evaluation-error", 2103, Value::Null, Value::Null, 13, 2193),
            ("evaluation-error", 2103, Value::Null, Value::Null, 13, 2110),
            ("evaluation-error", 2103, Value::Null, Value::Null, 13, 2222),
            ("evaluation-error", 2103, Value::Null, Value::Null, 13, 3002),
            ("evaluation-error", 2103, Value::Null, Value::Null, 13, 2105),
            ("evaluation-error", 2103, Value::Null, Value::Null, 13, 2405),
            ("evaluation-error", 2105, Value::Null, Value::Null, 12, 1000),
            ("evaluation-error", 2111, Value::Null, Value::Null, 13, 1000),
            (
                "assertion-failure",
                2132,
                Value::Null,
                Value::Null,
                13,
                2193,
            ),
            ("overflow", 2178, Value::Null, Value::Null, 13, 2110),
        ] {
            let (_, name, tlc_exit, _) = check_protocol::class(code).unwrap_or(&(0, "", 0, &[]));
            let mut report = json!({"command": "check", "verdict": verdict,
                "property": property, "statistics": stats,
                "tlc": {"code": code, "name": name, "exit_status": tlc_exit,
                    "reported": code, "verified": true}});
            for exit in [native_exit, i32::try_from(*tlc_exit).unwrap_or(-1)] {
                let good = observed(&report, exit);
                assert_eq!(check_outcome(&good, &good), "agree", "{report}");
                report["tlc"]["reported"] = json!(wrong_reported);
                let bad = observed(&report, exit);
                assert_eq!(check_outcome(&bad, &bad), "error", "{report}");
                assert_eq!(check_outcome(&good, &bad), "error", "{report}");
                if let Some(class) = report["tlc"].as_object_mut() {
                    let _ = class.remove("reported");
                }
                let missing = observed(&report, exit);
                assert_eq!(check_outcome(&missing, &missing), "error", "{report}");
                report["tlc"]["reported"] = json!(code);
            }
        }
    }
}
