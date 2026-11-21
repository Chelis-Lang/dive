//! `RandomElement` in simulation (ADR 0162 §8, D-044), against the pinned
//! TLC receipts in `crates/dive-sim/tests/fixtures/random-ops/oracle.json`.
//!
//! Draws come from each trace's own stream, so a run is the same at any
//! worker count and `--trace-ids` regenerates its traces; each trace records
//! its draws, and validation checks them against its own evaluation of the
//! sets.
#![allow(clippy::expect_used, reason = "fixture setup and exact report checks")]

use dive_api::trace::{Check, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{SimulateRequest, simulate_file};
use serde_json::{Value, json};
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/random-ops")
}

fn oracle(id: &str) -> Value {
    let record: Value = serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/random-ops/oracle.json"
    ))
    .expect("oracle record");
    assert_eq!(record["oracle"], "tlc-2026.10.06.014338");
    record["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == id)
        .cloned()
        .expect("case")
}

/// A request for `module` under the configuration of oracle case `id`.
fn request(id: &str, module: &str, compat_tlc: bool) -> (SimulateRequest, tempdir::Dir) {
    let case = oracle(id);
    assert_eq!(case["module"], module);
    let dir = tempdir::Dir::new(id);
    let file = dir.0.join(format!("{module}.tla"));
    std::fs::copy(fixtures().join(format!("{module}.tla")), &file).expect("module");
    std::fs::write(
        file.with_extension("cfg"),
        format!("{}\n", case["cfg"].as_str().expect("cfg")),
    )
    .expect("configuration");
    let request = SimulateRequest {
        file,
        compat_tlc,
        seed: Some(0),
        traces: 8,
        depth: Some(6),
        ..Default::default()
    };
    (request, dir)
}

mod tempdir {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub struct Dir(pub PathBuf);

    impl Dir {
        pub fn new(label: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "dive-sim-random-ops-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).expect("scratch directory");
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// Runs `request` and checks every reported trace validates.
fn run(request: &SimulateRequest) -> Value {
    let report = simulate_file(request).expect("simulation").to_json();
    for trace in report["traces"].as_array().into_iter().flatten() {
        assert_eq!(trace["validation"]["result"], "valid", "{trace}");
    }
    report
}

/// The fields of a report that must not depend on the worker count.
fn sample(report: &Value) -> Value {
    json!({
        "result": report["result"],
        "traces": report["traces"],
        "statistics": report["statistics"],
    })
}

#[test]
fn draws_are_identical_at_one_four_and_eight_workers_and_replay_by_trace_ids() {
    for (id, module) in [
        ("N1", "RndNext"),
        ("E1", "RndExists"),
        ("D2", "RndDeadlock"),
    ] {
        for compat_tlc in [false, true] {
            let (base, _dir) = request(id, module, compat_tlc);
            let mut base = base;
            base.keep_traces = Some(u64::MAX);
            let one = run(&SimulateRequest {
                workers: Some(1),
                ..base.clone()
            });
            assert_eq!(one["result"], "sampled", "{id}: {one}");
            for workers in [4, 8] {
                let many = run(&SimulateRequest {
                    workers: Some(workers),
                    ..base.clone()
                });
                assert_eq!(sample(&one), sample(&many), "{id} at {workers} workers");
            }
            let traces = one["traces"].as_array().expect("traces");
            assert_eq!(traces.len(), 8);
            for trace in traces {
                let states = trace["document"]["states"].as_array().expect("states");
                assert!(states[0]["draws"].is_null(), "Init draws nothing");
                for state in &states[1..] {
                    let draws = state["draws"].as_array().expect("each step drew");
                    assert_eq!(draws.len(), 1, "one draw per chosen step: {state}");
                }
            }
            let part = run(&SimulateRequest {
                workers: Some(3),
                trace_ids: Some((3, 7)),
                ..base.clone()
            });
            let part = part["traces"].as_array().expect("regenerated traces");
            assert_eq!(part.len(), 5);
            for trace in part {
                let id = trace["id"].as_u64().expect("id");
                let whole = traces.iter().find(|t| t["id"] == id).expect("same id");
                assert_eq!(trace["document"], whole["document"], "trace {id}");
            }
        }
    }
}

#[test]
fn next_draws_fresh_values_and_records_them_as_tlc_draws_them() {
    // TLC (N1): each step of each trace draws anew.
    let tlc = oracle("N1");
    assert_eq!(tlc["exit"], 0);
    let (request, _dir) = request("N1", "RndNext", false);
    let report = run(&request);
    let mut seen = std::collections::BTreeSet::new();
    for trace in report["traces"].as_array().expect("traces") {
        for state in &trace["document"]["states"].as_array().expect("states")[1..] {
            let x = state["values"]["x"].as_i64().expect("x");
            assert!((1..=1_000_000).contains(&x));
            assert_eq!(state["draws"], json!([x]));
            seen.insert(x);
        }
    }
    // 8 traces of 6 steps: every value differs (a repeat in 48 draws from
    // a million values would be unusual, and the stream is fixed).
    assert_eq!(seen.len(), 48, "fresh draws at every step: {seen:?}");
}

#[test]
fn init_draws_per_trace_where_tlc_draws_once() {
    // TLC (I1) starts every trace in the same state; dive draws per trace.
    let tlc = oracle("I1");
    let starts: std::collections::BTreeSet<_> = tlc["traces"]
        .as_array()
        .expect("traces")
        .iter()
        .map(|trace| trace[0].to_string())
        .collect();
    assert_eq!(starts.len(), 1, "TLC draws Init once");
    for compat_tlc in [false, true] {
        let (request, _dir) = request("I1", "RndInit", compat_tlc);
        let report = run(&request);
        let starts: std::collections::BTreeSet<_> = report["traces"]
            .as_array()
            .expect("traces")
            .iter()
            .map(|trace| trace["document"]["states"][0]["values"]["x"].to_string())
            .collect();
        assert!(starts.len() > 1, "dive draws Init per trace: {starts:?}");
    }
}

#[test]
fn a_draw_that_disables_next_is_drawn_again_never_a_deadlock() {
    // TLC (D1) reports deadlock 2114 although Next is enabled for draw 2.
    let tlc = oracle("D1");
    assert_eq!(tlc["exit"], 11);
    assert_eq!(tlc["deciding"][0]["code"], 2114);
    for compat_tlc in [false, true] {
        let (request, _dir) = request("D1", "RndDeadlock", compat_tlc);
        let report = run(&request);
        assert_eq!(report["result"], "sampled", "{report}");
        assert_eq!(report["tlc_code"], Value::Null);
        for trace in report["traces"].as_array().expect("traces") {
            for state in &trace["document"]["states"].as_array().expect("states")[1..] {
                assert_eq!(state["values"]["x"], 2);
            }
        }
    }
}

#[test]
fn draws_that_one_value_cannot_decide_refuse_by_name() {
    for (id, module, what) in [
        ("V1", "RndInvariant", "RandomElement in an invariant"),
        ("B1", "RndEnabled", "RandomElement (inside ENABLED"),
    ] {
        // TLC decides V1 (2110) and B1 (2114) on one draw.
        assert!(oracle(id)["exit"] != 0);
        for compat_tlc in [false, true] {
            let (request, _dir) = request(id, module, compat_tlc);
            let report = simulate_file(&request).expect("simulation");
            let json = report.to_json();
            assert_eq!(json["result"], "unsupported", "{json}");
            assert_eq!(report.exit_code(), 3);
            assert!(
                json["message"].as_str().is_some_and(|m| m.contains(what)),
                "{json}"
            );
        }
    }
}

#[test]
fn constants_draw_once_at_start_up_and_configured_targets_at_every_use() {
    // C1/C2: a constant definition draws once; C3: `C <- Pick` names Pick's
    // body, which TLC draws at every use.
    for (id, module) in [
        ("C1", "RndConst"),
        ("C2", "RndConst"),
        ("C3", "RndConfigured"),
    ] {
        let (request, _dir) = request(id, module, false);
        let report = run(&request);
        assert_eq!(report["result"], "sampled", "{id}: {report}");
        let tlc = oracle(id);
        let tlc_constant = tlc["traces"]
            .as_array()
            .expect("traces")
            .iter()
            .flat_map(|t| t.as_array().expect("states"))
            .map(Value::to_string)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == 1;
        let xs: std::collections::BTreeSet<_> = report["traces"]
            .as_array()
            .expect("traces")
            .iter()
            .flat_map(|t| t["document"]["states"].as_array().expect("states"))
            .map(|s| s["values"]["x"].to_string())
            .collect();
        assert_eq!(xs.len() == 1, tlc_constant, "{id}: {xs:?}");
        if tlc_constant {
            let startup = &report["traces"][0]["document"]["startup_draws"];
            assert_eq!(startup.as_array().map(Vec::len), Some(1), "{id}");
            assert_eq!(xs.iter().next(), Some(&startup[0].to_string()));
        }
    }
}

#[test]
fn validation_rejects_a_step_no_draw_of_its_set_gives() {
    // Next == x' = RandomElement({1, 2}) /\ x' = 2
    let (request, _dir) = request("D2", "RndDeadlock", false);
    let report = run(&request);
    let document = TraceDoc::from_json(&report["traces"][0]["document"]).expect("document");
    assert_eq!(document.draws[1], vec![json!(2)]);
    let model = SpecModel::load(
        &request.file,
        &request.file.with_extension("cfg"),
        &TraceOptions::default(),
    )
    .expect("model");
    assert_eq!(model.validate(&document).validation, Validation::Valid);
    // 3 is no draw of {1, 2}: the step is rejected, whatever is recorded.
    for recorded in [json!(3), json!(2)] {
        let mut forged = document.clone();
        forged.states[1]["x"] = json!(3);
        forged.draws[1] = vec![recorded];
        let validation = model.validate(&forged).validation;
        assert!(
            matches!(validation, Validation::Invalid(ref f) if f.check == Check::Next),
            "{validation:?}"
        );
    }
    // A recorded draw is only a hint: a wrong one (1, a member that gives no
    // successor) or one outside the set cannot make a valid step invalid.
    for recorded in [json!(1), json!(7)] {
        let mut hint = document.clone();
        hint.draws[1] = vec![recorded];
        assert_eq!(model.validate(&hint).validation, Validation::Valid);
    }
    let mut start = document;
    start.startup_draws = vec![json!(-1)];
    assert_eq!(model.validate(&start).validation, Validation::Valid);
}
