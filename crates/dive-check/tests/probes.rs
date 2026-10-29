//! Milestone CM0 (`docs/design/m5-plan.md`): the TLC probes of
//! `tests/fixtures/tlc-probes/` and the conformance harness's TLC fixtures,
//! checked under `--compat=tlc` against the pinned TLC's transcripts.
//!
//! For each: the deciding message code (verdict and error class), the
//! violated property, generated / distinct / left on queue / depth (and
//! their absence where TLC prints none), the counterexample's length, every
//! state's label and printed value, in both stores. An error whose TLC class
//! dive does not claim yet (`TlcClass::verified`) must say so.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_check::{CheckOptions, CheckResult, Verdict, check, format_trace_state};
use dive_store::StoreMode;

const ORACLE: &str = "tlc-2026.10.06.014338";

/// One `-tool` message.
struct Msg {
    code: u32,
    class: u32,
    body: String,
}

fn messages(stdout: &str) -> Vec<Msg> {
    let mut out = Vec::new();
    let mut cur: Option<Msg> = None;
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("@!@!@STARTMSG ") {
            let head = rest.trim_end_matches(" @!@!@");
            let (code, class) = head.split_once(':').expect("code:class");
            cur = Some(Msg {
                code: code.parse().expect("code"),
                class: class.parse().expect("class"),
                body: String::new(),
            });
        } else if line.starts_with("@!@!@ENDMSG ") {
            if let Some(m) = cur.take() {
                out.push(m);
            }
        } else if let Some(m) = cur.as_mut() {
            if !m.body.is_empty() {
                m.body.push('\n');
            }
            m.body.push_str(line);
        }
    }
    out
}

/// What TLC reported.
#[derive(Debug, PartialEq, Eq)]
struct Tlc {
    code: u32,
    property: Option<String>,
    counts: Option<(u64, u64, u64)>,
    depth: Option<u64>,
    /// `(label, state)` per counterexample state.
    trace: Vec<(String, String)>,
}

fn between(s: &str, a: &str, b: &str) -> Option<String> {
    Some(s.strip_prefix(a)?.split_once(b)?.0.to_owned())
}

fn tlc(stdout: &str) -> Tlc {
    let ms = messages(stdout);
    let decide = ms
        .iter()
        .find(|m| m.class == 1 && ![2103, 2121].contains(&m.code));
    let code = decide.map_or(2193, |m| m.code);
    let first = decide.map(|m| m.body.lines().next().unwrap_or("").to_owned());
    let property = first.and_then(|l| {
        between(&l, "Invariant ", " is violated")
            .or_else(|| between(&l, "Action property ", " is violated."))
            .or_else(|| between(&l, "Property ", " is violated"))
            .or_else(|| between(&l, "Assumption ", " is false."))
    });
    let counts = ms.iter().rfind(|m| m.code == 2199).map(|m| {
        let b = m
            .body
            .replace(" states generated, ", "|")
            .replace(" distinct states found, ", "|")
            .replace(" states left on queue.", "");
        let v: Vec<u64> = b.split('|').map(|x| x.trim().parse().expect("n")).collect();
        (v[0], v[1], v[2])
    });
    let depth = ms.iter().rfind(|m| m.code == 2194).map(|m| {
        m.body
            .trim_start_matches("The depth of the complete state graph search is ")
            .trim_end_matches('.')
            .parse()
            .expect("depth")
    });
    let mut trace = Vec::new();
    for m in &ms {
        match m.code {
            2217 => {
                let (head, state) = m.body.split_once('\n').unwrap_or((&m.body, ""));
                let label = head.split_once(": ").map_or(head, |x| x.1);
                trace.push((label.to_owned(), state.trim_end().to_owned()));
            }
            2107 | 2108 => {
                if let Some((_, state)) = m.body.split_once('\n') {
                    trace.push((
                        "<Initial predicate>".to_owned(),
                        state.trim_end().to_owned(),
                    ));
                }
            }
            _ => {}
        }
    }
    Tlc {
        code,
        property,
        counts,
        depth,
        trace,
    }
}

fn dive(r: &CheckResult) -> Tlc {
    let ctx = dive_value::Ctx::tlc();
    let class = r.verdict.tlc().expect("a verdict");
    let (counts, depth) = match (&r.stats, r.tlc_prints_stats) {
        (Some(s), true) => (
            Some((s.generated, s.distinct, s.left_on_queue)),
            Some(s.depth),
        ),
        _ => (None, None),
    };
    Tlc {
        code: class.code,
        property: r.verdict.property().map(str::to_owned),
        counts,
        depth,
        trace: r
            .counterexample
            .iter()
            .flat_map(|c| &c.states)
            .map(|s| {
                (
                    s.label.clone(),
                    format_trace_state(r, s, ctx).trim_end().to_owned(),
                )
            })
            .collect(),
    }
}

fn run(tla: &Path, cfg: &Path, store: StoreMode) -> CheckResult {
    run_with(tla, cfg, store, 1)
}

fn run_with(tla: &Path, cfg: &Path, store: StoreMode, workers: usize) -> CheckResult {
    let analysis = dive_sema::analyze_file(tla, &dive_sema::Options::default()).expect("readable");
    assert!(analysis.accepted(), "{} rejected", tla.display());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse(&std::fs::read_to_string(cfg).expect("cfg"))
        .into_config()
        .expect("cfg parses");
    let mut opts = CheckOptions::tlc(0, 0);
    opts.store = store;
    opts.workers = workers;
    check(&ir, &cfg, &opts)
}

fn compare(name: &str, dir: &Path, transcripts: &Path) -> Result<(), String> {
    let stdout = std::fs::read_to_string(transcripts.join(format!("{name}.stdout")))
        .map_err(|e| format!("{name}: {e}"))?;
    let want = tlc(&stdout);
    let mut first: Option<Tlc> = None;
    // Every worker count must reproduce TLC's single-worker run (ADR 0015).
    let settings = [StoreMode::Fingerprint, StoreMode::Exact]
        .into_iter()
        .flat_map(|store| [1, 3, 8].map(|w| (store, w)));
    for (store, workers) in settings {
        let r = run_with(
            &dir.join(format!("{name}.tla")),
            &dir.join(format!("{name}.cfg")),
            store,
            workers,
        );
        if !r.verdict.is_verdict() {
            return Err(format!("{name}: no verdict: {:?}", r.verdict));
        }
        let class = r.verdict.tlc().expect("a verdict");
        let got = dive(&r);
        if !class.verified {
            // dive does not claim TLC's class here (fixture `EvalError`: TLC
            // reports 1000, then decides on 2147); everything else must
            // still agree.
            let mut w = want_without_code(&want);
            w.code = got.code;
            if w != got {
                return Err(format!("{name}: TLC {w:?}\n dive {got:?}"));
            }
        } else if got != want {
            return Err(format!(
                "{name} ({store:?}, {workers} workers): TLC {want:?}\n dive {got:?}"
            ));
        }
        if let Some(f) = &first {
            if *f != got {
                return Err(format!(
                    "{name}: {workers} workers in {store:?} mode differ from 1 in fingerprint mode"
                ));
            }
        } else {
            first = Some(got);
        }
    }
    Ok(())
}

fn want_without_code(t: &Tlc) -> Tlc {
    Tlc {
        code: 0,
        property: t.property.clone(),
        counts: t.counts,
        depth: t.depth,
        trace: t.trace.clone(),
    }
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(f)
        .expect("thread")
        .join()
        .expect("no panic")
}

#[test]
fn tlc_probes_agree() {
    let root = crate_dir().join("tests/fixtures/tlc-probes");
    let failures = on_big_stack(move || {
        let mut names: Vec<String> = std::fs::read_dir(root.join("specs"))
            .expect("specs")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names.len(), 19);
        names
            .iter()
            .filter_map(|n| compare(n, &root.join("specs").join(n), &root.join(ORACLE)).err())
            .collect::<Vec<_>>()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn harness_fixtures_agree() {
    let root = crate_dir().join("../dive-conformance/tests/fixtures");
    let failures = on_big_stack(move || {
        [
            "Success",
            "Deadlock",
            "InvariantViolation",
            "InitInvariant",
            "InitProperty",
            "ActionProperty",
            "AssertFailure",
            "EvalError",
            "Overflow",
            "Assumption",
            "PrintOutput",
        ]
        .iter()
        .filter_map(|n| {
            compare(
                n,
                &root.join("specs").join(n),
                &root.join("tlc").join(ORACLE),
            )
            .err()
        })
        .collect::<Vec<_>>()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn print_output_is_kept_in_order() {
    let root = crate_dir().join("../dive-conformance/tests/fixtures/specs/PrintOutput");
    let r = on_big_stack(move || {
        run(
            &root.join("PrintOutput.tla"),
            &root.join("PrintOutput.cfg"),
            StoreMode::Fingerprint,
        )
    });
    assert_eq!(r.verdict, Verdict::Success);
    assert_eq!(
        r.output,
        ["<<\"step\", 0>>", "<<\"step\", 1>>", "<<\"step\", 2>>"]
    );
}
