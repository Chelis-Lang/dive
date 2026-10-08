//! Runs the successor generator over the Examples models that have TLC
//! goldens, through the test-only binding and single-worker search of
//! `tests/harness/model.rs`, and compares verdict, counts, depth and
//! counterexample length with each golden.
//!
//! `cargo run --release -p dive-eval --example next_corpus -- --examples ROOT
//! --golden DIR [--lib DIR]... [--budget N] [--filter TEXT] [--skip TEXT]... [--timeout SECS] [--jobs N] [--profile tlc|dive]`
//!
//! `ROOT` is the Examples checkout, `DIR` the goldens
//! (`conformance/golden/<oracle>/examples`). Prints one line per model and a
//! summary: `agree`, `disagree` (a bug, or a recorded difference),
//! `unsupported` (a refusal, naming the feature) and `skipped` (over the
//! budget, or not an exhaustive safety model).

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_used,
    clippy::panic,
    reason = "a command-line tool"
)]

#[path = "../tests/harness/mod.rs"]
mod harness;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use harness::model::{Checked, Verdict, bfs, bind_model, diff, on_big_stack};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|e| e == "json") {
            out.push(p);
        }
    }
}

/// The command line.
struct Args {
    examples: PathBuf,
    golden: PathBuf,
    libs: Vec<PathBuf>,
    budget: u64,
    filter: Option<String>,
    skip: Vec<String>,
    profile: String,
    timeout: u64,
    jobs: usize,
    child: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        examples: PathBuf::new(),
        golden: PathBuf::new(),
        libs: Vec::new(),
        budget: 2_000_000,
        filter: None,
        skip: Vec::new(),
        profile: "tlc".to_owned(),
        timeout: 300,
        jobs: 3,
        child: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(x) = args.next() {
        match x.as_str() {
            "--examples" => a.examples = args.next().expect("--examples ROOT").into(),
            "--golden" => a.golden = args.next().expect("--golden DIR").into(),
            "--lib" => a.libs.push(PathBuf::from(args.next().expect("--lib DIR"))),
            "--budget" => {
                a.budget = args
                    .next()
                    .and_then(|b| b.parse().ok())
                    .expect("--budget N");
            }
            "--filter" => a.filter = args.next(),
            "--skip" => a.skip.push(args.next().expect("--skip TEXT")),
            "--profile" => a.profile = args.next().expect("--profile tlc|dive"),
            "--timeout" => {
                a.timeout = args
                    .next()
                    .and_then(|b| b.parse().ok())
                    .expect("--timeout SECS");
            }
            "--jobs" => {
                a.jobs = args.next().and_then(|b| b.parse().ok()).expect("--jobs N");
            }
            "--child" => a.child = args.next(),
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

fn main() {
    let a = parse_args();
    let mut files = Vec::new();
    collect(&a.golden, &mut files);
    if let Some(id) = &a.child {
        for f in &files {
            let g: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(f).expect("golden")).expect("json");
            if g["model"]["id"].as_str() == Some(id.as_str()) {
                println!("{}", run_one(&a, &g, id));
                return;
            }
        }
        println!("error       {id}: no golden");
        return;
    }
    let mut summary: BTreeMap<String, usize> = BTreeMap::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let exe = std::env::current_exe().expect("the runner's path");
    let mut ids = Vec::new();
    for f in &files {
        let g: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(f).expect("golden")).expect("json");
        if g["kind"].as_str() != Some("tlc-model") {
            continue;
        }
        let id = g["model"]["id"].as_str().unwrap_or("").to_owned();
        if a.filter.as_ref().is_some_and(|t| !id.contains(t.as_str())) {
            continue;
        }
        ids.push(id);
    }
    // Children run `jobs` at a time; lines are printed in the corpus order.
    let next = std::sync::Mutex::new(0usize);
    let lines: Vec<std::sync::Mutex<Option<String>>> =
        ids.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..a.jobs.max(1) {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().expect("queue");
                        let i = *n;
                        *n += 1;
                        i
                    };
                    let Some(id) = ids.get(i) else { return };
                    let line = if a.skip.iter().any(|s| id.contains(s.as_str())) {
                        format!("skipped     {id}: --skip")
                    } else {
                        child(&exe, id, a.timeout)
                    };
                    eprintln!("{line}");
                    *lines[i].lock().expect("result") = Some(line);
                }
            });
        }
    });
    for line in lines {
        let line = line.into_inner().expect("result").unwrap_or_default();
        println!("{line}");
        let class = line.split_whitespace().next().unwrap_or("error").to_owned();
        if class == "unsupported" {
            let why = line.split_once(": ").map_or("", |(_, w)| w);
            *reasons.entry(feature(why)).or_default() += 1;
        }
        *summary.entry(class).or_default() += 1;
    }
    eprintln!("{summary:#?}");
    eprintln!("refusals by feature: {reasons:#?}");
}

/// Runs one model in a child process, killed after `timeout` seconds.
fn child(exe: &Path, id: &str, timeout: u64) -> String {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    args.push("--child".to_owned());
    args.push(id.to_owned());
    let Ok(mut c) = std::process::Command::new(exe)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return format!("error       {id}: cannot start the child");
    };
    let started = std::time::Instant::now();
    loop {
        match c.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed().as_secs() >= timeout => {
                let _ = c.kill();
                let _ = c.wait();
                return format!("timeout     {id}: over {timeout}s");
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => return format!("error       {id}: {e}"),
        }
    }
    let mut out = String::new();
    if let Some(mut s) = c.stdout.take() {
        let _ = std::io::Read::read_to_string(&mut s, &mut out);
    }
    match out.lines().next() {
        Some(l) => l.to_owned(),
        None => format!("error       {id}: the child crashed"),
    }
}

/// Runs one model and describes the outcome in one line.
fn run_one(a: &Args, g: &serde_json::Value, id: &str) -> String {
    let mode = g["model"]["mode"]["kind"].as_str().unwrap_or("");
    if mode != "exhaustive" {
        return format!("skipped     {id}: mode {mode}");
    }
    let generated = g["result"]["counts"]["states_generated"]
        .as_u64()
        .unwrap_or(u64::MAX);
    if generated > a.budget {
        return format!("skipped     {id}: {generated} states generated, over the budget");
    }
    let opts = if a.profile == "dive" {
        dive_eval::Options::dive()
    } else {
        dive_eval::Options::tlc()
    };
    let module = a.examples.join(g["model"]["module"].as_str().unwrap_or(""));
    let cfg_path = a.examples.join(id);
    let libs = a.libs.clone();
    let budget = a.budget;
    let started = std::time::Instant::now();
    let result = on_big_stack(move || -> Result<(Checked, bool), String> {
        let loaded = harness::load(&module, &libs)?;
        let text = std::fs::read_to_string(&cfg_path).map_err(|e| e.to_string())?;
        let cfg = dive_config::parse(&text)
            .into_config()
            .ok_or("cfg rejected")?;
        let b = bind_model(&loaded, &cfg, opts)?;
        if let dive_eval::AssumptionOutcome::Error { error, .. } =
            b.ev.check_assumptions(|d| harness::is_axiom(&loaded, d))
        {
            return Err(format!("assumption: {error}"));
        }
        let liveness = !b.liveness.is_empty();
        Ok((bfs(&b, budget.saturating_mul(2)), liveness))
    })
    .and_then(|r| r);
    let secs = started.elapsed().as_secs_f64();
    match result {
        Err(e) => {
            let class = if e.contains("unsupported") {
                "unsupported"
            } else {
                "error"
            };
            format!("{class:<11} {id}: {e}")
        }
        Ok(_) if g["result"]["verdict"].as_str() == Some("liveness-violation") => {
            format!("unsupported {id}: liveness (TLC reports a liveness violation)")
        }
        Ok((d, liveness)) => match &d.verdict {
            Verdict::Unsupported(e) => format!("unsupported {id}: {}", e.replace('\n', " ")),
            Verdict::TooBig => format!("skipped     {id}: over the budget"),
            _ => match diff(g, &d, liveness) {
                None if liveness => {
                    format!("counts      {id}: counts agree; liveness not checked ({secs:.1}s)")
                }
                None => format!("agree       {id} ({secs:.1}s)"),
                Some(why) => format!(
                    "disagree    {id}: {} [{:?}]",
                    why.replace('\n', " "),
                    d.verdict
                ),
            },
        },
    }
}

/// The feature a refusal names, for the summary.
fn feature(e: &str) -> String {
    let e = e
        .trim_start_matches("assumption: ")
        .trim_start_matches("unsupported: ");
    let cut = e
        .find(" (")
        .unwrap_or(e.len())
        .min(e.find(':').unwrap_or(e.len()));
    e[..cut].chars().take(90).collect()
}
