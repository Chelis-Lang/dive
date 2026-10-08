//! The TLC module's run-dependent operators during a search, against the
//! pinned TLC: `TLCGet("level")` in every kind of check, registers written
//! and read while checking, `TLCGet("all")`, `TLCGet("config")`, the
//! counters while checking assumptions, `TLCSet("exit", TRUE)`, and
//! `RandomElement` and the `Randomization` operators drawing from TLC's
//! start-up stream and per-state streams (`-seed 0`, `-fp 0`).
//!
//! Each spec under `tests/fixtures/tlc-module/specs/` has a transcript
//! captured with `dive-conformance dev capture` under
//! `tests/fixtures/tlc-module/tlc-2026.10.06.014338/`. The test checks
//! `dive check --compat=tlc` against it: the deciding message, the final
//! counts, and every line the specification printed up to the decision, in
//! order.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_check::{CheckOptions, Verdict, check};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-module")
}

/// What TLC reported: the deciding code, the counts and the printed lines.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    code: u32,
    counts: Option<(u64, u64, u64)>,
    printed: Vec<String>,
}

fn counts_of(line: &str) -> Option<(u64, u64, u64)> {
    let nums: Vec<u64> = line
        .split(|c: char| !c.is_ascii_digit() && c != ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.replace(',', "").parse().ok())
        .collect();
    match nums.as_slice() {
        [g, d, q] => Some((*g, *d, *q)),
        _ => None,
    }
}

fn tlc(stdout: &str) -> Outcome {
    let mut code = 2193;
    let mut counts = None;
    let mut printed = Vec::new();
    let mut started = false;
    let mut decided = false;
    let mut in_msg: Option<(u32, u32)> = None;
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("@!@!@STARTMSG ") {
            let head = rest.trim_end_matches(" @!@!@");
            let (c, k) = head.split_once(':').expect("code:class");
            let (c, k): (u32, u32) = (c.parse().expect("code"), k.parse().expect("class"));
            if c == 2219 {
                started = true;
            }
            if !decided && (k == 1 && c != 2121) {
                code = c;
                decided = true;
            }
            if c == 2193 || c == 2199 {
                decided = true;
            }
            in_msg = Some((c, k));
            continue;
        }
        if line.starts_with("@!@!@ENDMSG ") {
            in_msg = None;
            continue;
        }
        match in_msg {
            Some((2199, _)) => counts = counts_of(line),
            None if started && !decided => printed.push(line.to_owned()),
            _ => {}
        }
    }
    Outcome {
        code,
        counts,
        printed,
    }
}

fn dive(dir: &Path, name: &str) -> Outcome {
    let tla = dir.join(format!("{name}.tla"));
    let cfg_text = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("cfg");
    let analysis = dive_sema::analyze_file(&tla, &dive_sema::Options::default()).expect("reads");
    assert!(analysis.accepted(), "{name}: rejected");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse(&cfg_text).into_config().expect("cfg");
    let r = check(&ir, &cfg, &CheckOptions::tlc(0, 0));
    let code = match &r.verdict {
        Verdict::Success => 2193,
        Verdict::Deadlock => 2114,
        Verdict::InvariantViolated { initial: false, .. } => 2110,
        Verdict::InvariantViolated { initial: true, .. } => 2107,
        other => panic!("{name}: unexpected verdict {other:?}"),
    };
    Outcome {
        code,
        counts: r.stats.map(|s| (s.generated, s.distinct, s.left_on_queue)),
        printed: r.output,
    }
}

#[test]
fn tlc_module_operators_agree() {
    let specs = root().join("specs");
    let mut names: Vec<String> = std::fs::read_dir(&specs)
        .expect("specs")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    assert!(names.len() >= 8, "fixtures missing");
    let mut failures = Vec::new();
    for name in &names {
        let stdout = std::fs::read_to_string(root().join(ORACLE).join(format!("{name}.stdout")))
            .expect("transcript");
        let want = tlc(&stdout);
        let dir = specs.join(name);
        let n = name.clone();
        let got = std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn(move || dive(&dir, &n))
            .expect("thread")
            .join()
            .expect("no panic");
        if got != want {
            failures.push(format!("{name}:\n  TLC  {want:?}\n  dive {got:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
