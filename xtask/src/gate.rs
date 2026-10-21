//! `cargo xtask gate`: the local mirror of CI's `Workspace gate` (`--fast`)
//! and the pre-PR full gate (`--full`), which adds the whole test suite and
//! the conformance runs CI no longer performs on every PR.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::Result;

const GATE_USAGE: &str = "usage: cargo xtask gate --fast | --full [--skip-corpus]

    --fast         mirror CI's `Workspace gate` (nextest `ci` profile)
    --full         everything in --fast with the whole nextest suite, then the
                   cm1 and validation-smoke conformance gates at 1, 4 and 8
                   workers and the small corpus tier in both profiles, each
                   checked against its committed baseline
                   (conformance/baselines/); writes <target-dir>/gate-report.md
                   for the PR body
    --skip-corpus  with --full: skip the small corpus tier runs

Respects CARGO_BUILD_JOBS and NEXTEST_TEST_THREADS for shared machines.";

/// Harness arguments shared by the full gate's small-tier corpus steps: the
/// established 156-model selection.
const CORPUS_ARGS: &[&str] = &[
    "--impl",
    "dive",
    "--corpus",
    "examples",
    "--corpus",
    "probes",
    "--corpus",
    "validation-smoke",
    "--corpus",
    "configured-temporal-operators",
    "--tier",
    "small",
    "--workers",
    "1",
    "--jobs",
    "2",
    "--tlc-budgets",
    "--budget-cap",
    "900",
    "--no-summary",
];

/// A small-tier corpus step: the profile, its extra harness arguments and
/// its committed baseline (`run --baseline`), relative to the repository.
struct CorpusProfile {
    profile: &'static str,
    extra: &'static [&'static str],
    baseline: &'static str,
}

/// The full gate's small-tier corpus steps. The default profile's modules
/// are the same SANY comparison as the TLC profile's, so it runs models only.
const CORPUS_PROFILES: &[CorpusProfile] = &[
    CorpusProfile {
        profile: "tlc",
        extra: &[],
        baseline: "conformance/baselines/small-tlc-w1.json",
    },
    CorpusProfile {
        profile: "dive",
        extra: &["--no-sany"],
        baseline: "conformance/baselines/small-dive-w1.json",
    },
];

/// Corpus outcome counts keyed by `(scope, outcome)`, or why there are none.
type CorpusCounts = std::result::Result<BTreeMap<(String, String), u64>, String>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Fast,
    Full,
}

enum Status {
    Pass,
    Fail(String),
    Skipped(String),
}

struct StepResult {
    name: String,
    status: Status,
    duration: Duration,
}

struct Gate {
    root: PathBuf,
    cargo: OsString,
    target: PathBuf,
    results: Vec<StepResult>,
    keep_going: bool,
}

impl Gate {
    fn failed(&self) -> bool {
        self.results
            .iter()
            .any(|r| matches!(r.status, Status::Fail(_)))
    }

    /// Whether the next step should run; records it as skipped otherwise.
    fn should_run(&mut self, name: &str) -> bool {
        if self.failed() && !self.keep_going {
            self.results.push(StepResult {
                name: name.to_owned(),
                status: Status::Skipped("earlier step failed".to_owned()),
                duration: Duration::ZERO,
            });
            return false;
        }
        true
    }

    fn skip(&mut self, name: &str, reason: &str) {
        println!("==> {name}: skipped ({reason})");
        self.results.push(StepResult {
            name: name.to_owned(),
            status: Status::Skipped(reason.to_owned()),
            duration: Duration::ZERO,
        });
    }

    fn cargo(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.cargo);
        cmd.current_dir(&self.root).args(args);
        cmd
    }

    fn run(&mut self, name: &str, mut cmd: Command) -> bool {
        if !self.should_run(name) {
            return false;
        }
        println!("==> {name}");
        let start = Instant::now();
        let status = match cmd.status() {
            Ok(s) if s.success() => Status::Pass,
            Ok(s) => Status::Fail(format!("exit {}", s.code().unwrap_or(-1))),
            Err(e) => Status::Fail(format!("cannot run {}: {e}", cmd.get_program().display())),
        };
        let ok = matches!(status, Status::Pass);
        self.results.push(StepResult {
            name: name.to_owned(),
            status,
            duration: start.elapsed(),
        });
        ok
    }

    fn run_fn(&mut self, name: &str, f: impl FnOnce() -> Result<()>) {
        if !self.should_run(name) {
            return;
        }
        println!("==> {name}");
        let start = Instant::now();
        let status = match f() {
            Ok(()) => Status::Pass,
            Err(e) => {
                eprintln!("{e}");
                Status::Fail(first_line(&e))
            }
        };
        self.results.push(StepResult {
            name: name.to_owned(),
            status,
            duration: start.elapsed(),
        });
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().to_owned()
}

/// Runs `cargo xtask gate` with the arguments after `gate`.
pub fn gate(args: &[String]) -> Result<()> {
    let mut mode = None;
    let mut skip_corpus = false;
    for arg in args {
        match arg.as_str() {
            "--fast" => mode = Some(Mode::Fast),
            "--full" => mode = Some(Mode::Full),
            "--skip-corpus" => skip_corpus = true,
            _ => return Err(GATE_USAGE.to_owned()),
        }
    }
    let Some(mode) = mode else {
        return Err(GATE_USAGE.to_owned());
    };
    if skip_corpus && mode == Mode::Fast {
        return Err(GATE_USAGE.to_owned());
    }
    let root = crate::workspace_root();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map_or_else(|| root.join("target"), |t| root.join(t));
    let mut g = Gate {
        cargo: std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        root,
        target,
        results: Vec::new(),
        keep_going: mode == Mode::Full,
    };
    let started = Instant::now();

    workspace_steps(&mut g, mode);
    if mode == Mode::Full {
        conformance_steps(&mut g, skip_corpus);
    }

    let corpus: Vec<(&str, CorpusCounts)> = if mode == Mode::Full && !skip_corpus {
        CORPUS_PROFILES
            .iter()
            .map(|p| {
                (
                    p.profile,
                    corpus_counts(&corpus_report_dir(&g.target, p.profile)),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let summary = render_summary(&g, mode, started.elapsed(), &corpus);
    println!("\n{summary}");
    if mode == Mode::Full {
        let path = g.target.join("gate-report.md");
        std::fs::create_dir_all(&g.target)
            .and_then(|()| std::fs::write(&path, &summary))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("report written to {}", path.display());
    }
    if g.failed() {
        Err("gate: FAILED (see the summary above)".to_owned())
    } else {
        println!("gate: passed");
        Ok(())
    }
}

/// The steps of CI's `Workspace gate`, in its order. `--full` swaps the
/// `ci` nextest profile for the whole suite.
fn workspace_steps(g: &mut Gate, mode: Mode) {
    let fmt = g.cargo(&["fmt", "--all", "--check"]);
    g.run("rustfmt", fmt);
    let clippy = g.cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ]);
    g.run("clippy", clippy);
    let (name, profile) = match mode {
        Mode::Fast => ("nextest (ci profile)", "ci"),
        Mode::Full => ("nextest (whole suite)", "default"),
    };
    let tests = g.cargo(&[
        "nextest",
        "run",
        "--workspace",
        "--locked",
        "--profile",
        profile,
    ]);
    g.run(name, tests);
    let doctests = g.cargo(&["test", "--workspace", "--doc", "--locked"]);
    g.run("doctests", doctests);
    let mut doc = g.cargo(&["doc", "--workspace", "--no-deps", "--locked"]);
    doc.env("RUSTDOCFLAGS", "-D warnings");
    g.run("rustdoc", doc);
    g.run_fn("crate layering", crate::check_layering);
    if deny_inputs_changed(&g.root) == Some(false) {
        g.skip("cargo-deny", "Cargo.lock and deny.toml unchanged");
    } else {
        let deny = g.cargo(&["deny", "--locked", "check"]);
        g.run("cargo-deny", deny);
    }
}

/// Whether `Cargo.lock` or `deny.toml` differ from the merge base with
/// `origin/main`, committed or not. `None` when git cannot tell.
fn deny_inputs_changed(root: &Path) -> Option<bool> {
    let base = crate::run_capture(Command::new("git").current_dir(root).args([
        "merge-base",
        "origin/main",
        "HEAD",
    ]))
    .ok()?;
    let changed = crate::run_capture(Command::new("git").current_dir(root).args([
        "diff",
        "--name-only",
        base.trim(),
        "--",
        "Cargo.lock",
        "deny.toml",
    ]))
    .ok()?;
    Some(!changed.trim().is_empty())
}

fn corpus_report_dir(target: &Path, profile: &str) -> PathBuf {
    target.join("gate-corpus").join(profile)
}

fn corpus_step_name(profile: &str) -> String {
    format!("small corpus tier (dive, {profile} profile, vs baseline)")
}

fn conformance_steps(g: &mut Gate, skip_corpus: bool) {
    let build = g.cargo(&[
        "build",
        "--release",
        "--locked",
        "-p",
        "dive-conformance",
        "-p",
        "dive-cli",
    ]);
    if !g.run("release build (harness, dive)", build) {
        for name in [
            "oracle and corpus fetch",
            "cm1 gate",
            "validation-smoke gate",
        ] {
            g.skip(name, "release build failed");
        }
        if !skip_corpus {
            for p in CORPUS_PROFILES {
                g.skip(&corpus_step_name(p.profile), "release build failed");
            }
        }
        return;
    }
    let harness = g.target.join("release").join("dive-conformance");
    let dive = g.target.join("release").join("dive");
    let root = g.root.clone();
    let fetch_harness = harness.clone();
    g.run_fn("oracle and corpus fetch", move || {
        for args in [
            &["oracle", "fetch"][..],
            &["corpus", "fetch", "--corpus", "examples"][..],
        ] {
            let status = Command::new(&fetch_harness)
                .current_dir(&root)
                .args(args)
                .status()
                .map_err(|e| format!("cannot run {}: {e}", fetch_harness.display()))?;
            if !status.success() {
                return Err(format!("dive-conformance {} failed", args.join(" ")));
            }
        }
        Ok(())
    });
    let gate_script = |list: &str, corpus: &str, extra: &[&str]| {
        let mut cmd = Command::new("bash");
        cmd.current_dir(&g.root)
            .arg("scripts/conformance-gate.sh")
            .arg(list)
            .args(["--workers", "1,4,8"])
            .args(extra)
            .arg("--dive-bin")
            .arg(&dive)
            .env("HARNESS", &harness)
            .env("DIVE_GATE_CORPUS", corpus);
        cmd
    };
    let cm1 = gate_script("conformance/gates/cm1.txt", "examples", &[]);
    let smoke = gate_script(
        "conformance/gates/validation-smoke.txt",
        "validation-smoke",
        &[],
    );
    g.run("cm1 gate (1, 4, 8 workers)", cm1);
    g.run("validation-smoke gate (1, 4, 8 workers)", smoke);
    for p in CORPUS_PROFILES {
        let name = corpus_step_name(p.profile);
        if skip_corpus {
            g.skip(&name, "--skip-corpus");
            continue;
        }
        let report_dir = corpus_report_dir(&g.target, p.profile);
        // Stale reports would be counted as this run's.
        let _ = std::fs::remove_dir_all(&report_dir);
        let mut corpus = Command::new(&harness);
        corpus
            .current_dir(&g.root)
            .arg("run")
            .args(CORPUS_ARGS)
            .args(["--impl-profile", p.profile])
            .args(p.extra)
            .arg("--baseline")
            .arg(g.root.join(p.baseline))
            .arg("--dive-bin")
            .arg(&dive)
            .arg("--report-dir")
            .arg(&report_dir);
        g.run(&name, corpus);
    }
}

/// Outcome counts summed over the harness report(s) in `dir`.
fn corpus_counts(dir: &Path) -> CorpusCounts {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("no corpus report in {}: {e}", dir.display()))?;
    let mut totals = BTreeMap::new();
    let mut reports = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let report: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
        for scope in ["models", "modules"] {
            let Some(counts) = report["summary"][scope].as_object() else {
                continue;
            };
            for (outcome, n) in counts {
                *totals
                    .entry((scope.to_owned(), outcome.clone()))
                    .or_default() += n.as_u64().unwrap_or(0);
            }
        }
        reports += 1;
    }
    if reports == 0 {
        return Err(format!("no corpus report in {}", dir.display()));
    }
    Ok(totals)
}

fn fmt_duration(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

fn render_summary(
    g: &Gate,
    mode: Mode,
    total: Duration,
    corpus: &[(&str, CorpusCounts)],
) -> String {
    let head = crate::run_capture(Command::new("git").current_dir(&g.root).args([
        "rev-parse",
        "--short",
        "HEAD",
    ]))
    .map_or_else(|_| "unknown".to_owned(), |s| s.trim().to_owned());
    let mode_name = match mode {
        Mode::Fast => "--fast",
        Mode::Full => "--full",
    };
    let verdict = if g.failed() { "FAILED" } else { "passed" };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "### `cargo xtask gate {mode_name}`: {verdict}\n\nCommit `{head}`, {} total, {} {}.\n",
        fmt_duration(total),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    out.push_str("| Step | Result | Duration |\n|---|---|---|\n");
    for r in &g.results {
        let result = match &r.status {
            Status::Pass => "pass".to_owned(),
            Status::Fail(why) => format!("**FAIL** ({why})"),
            Status::Skipped(why) => format!("skipped ({why})"),
        };
        let _ = writeln!(
            out,
            "| {} | {result} | {} |",
            r.name,
            fmt_duration(r.duration)
        );
    }
    for (profile, counts) in corpus {
        let counts = match counts {
            Err(why) => {
                let _ = writeln!(
                    out,
                    "\nSmall corpus tier, {profile} profile: no counts ({why})."
                );
                continue;
            }
            Ok(counts) => counts,
        };
        let _ = writeln!(
            out,
            "\nSmall corpus tier (`dive`, {profile} profile, 1 worker; examples, probes, validation-smoke, configured-temporal-operators):\n"
        );
        out.push_str("| Scope | agree | agree-pending-trace-validation | unsupported | disagree | timeout | oracle-error | other |\n|---|---|---|---|---|---|---|---|\n");
        for scope in ["models", "modules"] {
            let get = |name: &str| {
                counts
                    .get(&(scope.to_owned(), name.to_owned()))
                    .copied()
                    .unwrap_or(0)
            };
            let named = [
                "agree",
                "agree-pending-trace-validation",
                "unsupported",
                "disagree",
                "timeout",
                "oracle-error",
            ];
            let other: u64 = counts
                .iter()
                .filter(|((s, o), _)| s == scope && !named.contains(&o.as_str()))
                .map(|(_, n)| n)
                .sum();
            let _ = writeln!(
                out,
                "| {scope} | {} | {} | {} | {} | {} | {} | {other} |",
                get("agree"),
                get("agree-pending-trace-validation"),
                get("unsupported"),
                get("disagree"),
                get("timeout"),
                get("oracle-error"),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_counts_sum_reports_and_render() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("xtask-gate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let report = r#"{"summary": {"models": {"agree": 3, "unsupported": 1, "divergence-mode": 2},
                                      "modules": {"agree": 8, "disagree": 1}}}"#;
        for name in ["a.json", "b.json"] {
            std::fs::write(dir.join(name), report).map_err(|e| e.to_string())?;
        }
        std::fs::write(dir.join("notes.txt"), "ignored").map_err(|e| e.to_string())?;
        let counts = corpus_counts(&dir);
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        let map = counts.clone()?;
        assert_eq!(
            map.get(&("models".to_owned(), "agree".to_owned())),
            Some(&6)
        );
        assert_eq!(
            map.get(&("modules".to_owned(), "disagree".to_owned())),
            Some(&2)
        );
        let gate = Gate {
            root: PathBuf::from("."),
            cargo: "cargo".into(),
            target: PathBuf::from("target"),
            results: Vec::new(),
            keep_going: true,
        };
        let summary = render_summary(
            &gate,
            Mode::Full,
            Duration::from_secs(61),
            &[("tlc", counts)],
        );
        assert!(
            summary.contains("| models | 6 | 0 | 2 | 0 | 0 | 0 | 4 |"),
            "{summary}"
        );
        assert!(
            summary.contains("| modules | 16 | 0 | 0 | 2 | 0 | 0 | 0 |"),
            "{summary}"
        );
        Ok(())
    }

    #[test]
    fn missing_corpus_report_is_explained() {
        let dir = std::env::temp_dir().join("xtask-gate-no-such-dir");
        assert!(corpus_counts(&dir).is_err());
    }
}
