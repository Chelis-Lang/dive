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

const USAGE: &str = "usage: cargo xtask pluscal-corpus [--out DIR] [--dive BIN] [--cache DIR]
                       [--filter TEXT] [--timeout SECS] [--check] [--check-timeout SECS]

    --out DIR            output directory (default <target-dir>/pluscal-corpus);
                         writes results.json, summary.md and work/<n>/
    --dive BIN           dive binary (default <target-dir>/release/dive)
    --cache DIR          conformance cache (default $DIVE_CACHE_DIR or ~/.cache/dive)
    --filter TEXT        only modules whose corpus path contains TEXT
    --timeout SECS       per-translation time limit (default 60)
    --check              model-check both translations of modules with a .cfg
    --check-timeout SECS per-check time limit (default 120)";

struct Config {
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
    let config = parse_args(args)?;
    let root = workspace_root();
    let java = find_java(&config.cache.join("jre"))?;
    let jar = oracle_jar(&root, &config.cache)?;
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
    let mut config = Config {
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
    config.libraries = libraries(&config.cache.join("git"));
    Ok(config)
}

/// `<cache>/git/tlapm-library-*/<commit>/library` and
/// `<cache>/git/community-modules/<commit>/modules`, when present.
fn libraries(git: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(git) else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let leaf = if name.starts_with("tlapm-library") {
            "library"
        } else if name == "community-modules" {
            "modules"
        } else {
            continue;
        };
        if let Ok(commits) = std::fs::read_dir(entry.path()) {
            for commit in commits.flatten() {
                let dir = commit.path().join(leaf);
                if dir.is_dir() {
                    found.push(dir);
                }
            }
        }
    }
    found.sort();
    found
}

/// Adds `--library-path` for every cached module library.
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

/// Reads the value of `key = "..."` from a TOML line.
fn toml_string<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line
        .trim()
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?;
    let rest = rest.trim_start().strip_prefix('"')?;
    rest.split('"').next()
}

/// The default gating TLC oracle jar from `conformance/oracles.toml`.
fn oracle_jar(root: &Path, cache: &Path) -> Result<PathBuf> {
    let text = read(&root.join("conformance/oracles.toml"))?;
    let mut default = false;
    for line in text.lines() {
        if line.trim() == "[[tlc]]" {
            default = false;
        } else if line.trim() == "default = true" {
            default = true;
        } else if default && let Some(sha) = toml_string(line, "sha256") {
            let jar = cache.join("sha256").join(sha).join("tla2tools.jar");
            return if jar.is_file() {
                Ok(jar)
            } else {
                Err(format!(
                    "pinned oracle {} missing; run the harness `oracle fetch`",
                    jar.display()
                ))
            };
        }
    }
    Err("no default oracle in conformance/oracles.toml".to_owned())
}

/// The pinned commit of a corpus in `conformance/corpora.toml`.
fn corpus_commit(text: &str, id: &str) -> Option<String> {
    let mut current = None;
    for line in text.lines() {
        if let Some(value) = toml_string(line, "id") {
            current = Some(value.to_owned());
        } else if current.as_deref() == Some(id)
            && let Some(commit) = toml_string(line, "commit")
        {
            return Some(commit.to_owned());
        }
    }
    None
}

/// Every `.tla` file with a PlusCal algorithm in Examples and tlaplus tests.
fn discover(root: &Path, config: &Config) -> Result<Vec<(String, PathBuf)>> {
    let corpora = read(&root.join("conformance/corpora.toml"))?;
    let mut specs = Vec::new();
    for id in ["examples", "tlaplus-tests"] {
        let Some(commit) = corpus_commit(&corpora, id) else {
            return Err(format!("corpus {id} is not pinned"));
        };
        let base = config.cache.join("git").join(id).join(&commit);
        if !base.is_dir() {
            eprintln!(
                "warning: corpus {id} not in cache ({}); skipped",
                base.display()
            );
            continue;
        }
        let mut files = Vec::new();
        collect_tla(&base, &mut files);
        files.sort();
        for file in files {
            let relative = file
                .strip_prefix(&base)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            // Translator sources, not inputs.
            if relative.contains("/src/") {
                continue;
            }
            if let Some(filter) = &config.filter
                && !relative.contains(filter.as_str())
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            if has_algorithm(&text) {
                specs.push((format!("{id}:{relative}"), file));
            }
        }
    }
    Ok(specs)
}

fn collect_tla(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if path.is_dir() {
            collect_tla(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "tla") {
            out.push(path);
        }
    }
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
        (_, "timeout" | "internal" | "error") => "dive-error",
        (true, "artifact") => "ok",
        (_, "unsupported") => "refused",
        (false, "rejected") => "both-reject",
        _ if pcal.timed_out => "pcal-timeout",
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
    let timed_out =
        a["timed_out"].as_bool().unwrap_or(false) || b["timed_out"].as_bool().unwrap_or(false);
    let agree = a["verdict"] == b["verdict"]
        && a["distinct"] == b["distinct"]
        && a["generated"] == b["generated"]
        && a["exit"] == b["exit"];
    Ok(json!({
        "pcal": a,
        "dive": b,
        "outcome": if timed_out { "timeout" } else if agree { "agree" } else { "disagree" },
    }))
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
    Ok(json!({
        "exit": run.code,
        "timed_out": run.timed_out,
        "verdict": report.get("verdict").or_else(|| report.get("outcome")).cloned(),
        "distinct": find_number(&report, "distinct"),
        "generated": find_number(&report, "generated"),
    }))
}

/// First number keyed by a name containing `key`, searching depth-first.
fn find_number(value: &Value, key: &str) -> Option<u64> {
    match value {
        Value::Object(map) => {
            for (name, item) in map {
                if name.contains(key)
                    && let Some(number) = item.as_u64()
                {
                    return Some(number);
                }
            }
            map.values().find_map(|item| find_number(item, key))
        }
        Value::Array(items) => items.iter().find_map(|item| find_number(item, key)),
        _ => None,
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
