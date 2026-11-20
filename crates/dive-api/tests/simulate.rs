//! Simulation must reproduce the pinned selection stream and remain a sample.
#![allow(
    clippy::unwrap_used,
    reason = "fixture/setup corruption must fail these differential tests"
)]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/simulate.json");
const CORE: &str = include_str!("fixtures/core/simulate.json");
use dive_api::SimulateRequest;
use serde_json::Value;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/SimChoice.tla")
}

/// Documented compat gap (docs/tlc-differences.md, #910): under
/// `--compat=tlc`, Core's simulation does not reproduce TLC's seeded choice
/// stream. Each oracle row's seed and aril still give Core's own recorded,
/// independently validated sample, and the TLC rows stay the reference that a
/// port of the stream must meet.
#[test]
fn seeded_tlc_choices_and_counts_match_ten_oracle_runs() {
    let expected: Value = serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/choice-oracle.json"
    ))
    .unwrap();
    let mut matching_rows = 0;
    for row in expected.as_array().unwrap() {
        let report = dive_api::simulate_file(&SimulateRequest {
            file: fixture(),
            compat_tlc: true,
            seed: row["seed"].as_i64(),
            aril: row["aril"].as_u64().unwrap(),
            depth: Some(5),
            traces: 3,
            ..Default::default()
        })
        .unwrap();
        let json = report.to_json();
        assert_eq!(report.exit_code(), 0, "{json}");
        assert_eq!(json["result"], "sampled");
        assert_eq!(json["exhaustive"], false);
        assert_eq!(json["model_proved"], false);
        let actual: Vec<Vec<Value>> = json["traces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|trace| {
                assert_eq!(trace["validation"]["result"], "valid", "{trace}");
                trace["tlc_states"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["x"].clone())
                    .collect()
            })
            .collect();
        let sample = serde_json::json!({
            "generated": json["statistics"]["generated"],
            "traces": actual,
        });
        agreed::check_core(
            CORE,
            "seeded_tlc_choices_and_counts_match_ten_oracle_runs/0",
            &sample,
        );
        matching_rows += usize::from(
            sample["generated"] == row["generated"] && sample["traces"] == row["traces"],
        );
    }
    // The documented gap: when Core reproduces TLC's choice stream, every
    // row matches and this assertion (and the gap entry) must go.
    assert!(
        matching_rows < expected.as_array().unwrap().len(),
        "every oracle row now matches TLC: remove the documented gap"
    );
}

#[test]
fn default_trace_identity_replays_and_is_prefix_stable() {
    let mut request = SimulateRequest {
        file: fixture(),
        seed: Some(7),
        depth: Some(5),
        traces: 2,
        ..Default::default()
    };
    let a = dive_api::simulate_file(&request).unwrap().to_json();
    // D-003's per-trace streams are dive-defined: Core's own sample is the
    // contract for this seed.
    assert_eq!(dive_api::simulate_file(&request).unwrap().to_json(), a);
    agreed::check_core(
        CORE,
        "default_trace_identity_replays_and_is_prefix_stable/0",
        &serde_json::json!({"traces": unlabelled(&a["traces"]), "statistics": recorded(&a["statistics"])}),
    );
    request.traces = 5;
    let b = dive_api::simulate_file(&request).unwrap().to_json();
    assert_eq!(
        &a["traces"].as_array().unwrap()[..],
        &b["traces"].as_array().unwrap()[..2]
    );
    // Trace identity does not depend on the worker that samples it.
    request.workers = Some(8);
    let parallel = dive_api::simulate_file(&request).unwrap().to_json();
    assert_eq!(parallel["traces"], b["traces"]);
    assert_eq!(parallel["statistics"], b["statistics"]);
    assert_eq!(parallel["options"]["workers_used"], 8);
}

#[test]
fn simulation_error_sites_match_pinned_messages_and_statuses() {
    let expected: Value = serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/error-oracle.json"
    ))
    .unwrap();
    for row in expected.as_array().unwrap() {
        let name = row["model"].as_str().unwrap();
        let report = dive_api::simulate_file(&SimulateRequest {
            file: fixture().with_file_name(format!("{name}.tla")),
            compat_tlc: true,
            seed: Some(0),
            depth: Some(4),
            traces: 1,
            ..Default::default()
        })
        .unwrap();
        let json = report.to_json();
        if row["native_scope"] == "unsupported" {
            // Formerly refused: TLC checks the action property on the
            // candidate before the constraint excludes it, and the validator
            // accepts that excluded last state (ADR 0162). No agreed record
            // exists for this row, so it is compared with the oracle alone.
            assert_eq!(
                row["codes"][0], 2112,
                "oracle checks action property before constraints"
            );
            assert_eq!(json["result"], "violation", "{json}");
            assert_eq!(json["violation_validated"], true, "{json}");
            assert_eq!(u64::from(report.exit_code()), row["exit"].as_u64().unwrap());
            assert_eq!(json["tlc_code"], row["codes"][0], "{name}: {json}");
            assert_eq!(json["statistics"]["generated"], row["generated"], "{json}");
            let states: Vec<_> = json["traces"][0]["tlc_states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["x"].clone())
                .collect();
            assert_eq!(serde_json::json!(states), row["states"], "{name}");
            continue;
        }
        agreed::check_eq(
            AGREED,
            "simulation_error_sites_match_pinned_messages_and_statuses/0",
            &(row["exit"].as_u64().unwrap()),
            &(u64::from(report.exit_code())),
        );
        assert_eq!(json["tlc_code"], row["codes"][0], "{name}: {json}");
        if let Some(generated) = row["generated"].as_u64() {
            assert_eq!(json["statistics"]["generated"], generated, "{name}: {json}");
        }
        if json["result"] == "violation" {
            assert_eq!(json["violation_validated"], true, "{json}");
            let trace = json["traces"].as_array().unwrap().last().unwrap();
            let states: Vec<_> = trace["tlc_states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["x"].clone())
                .collect();
            assert_eq!(serde_json::json!(states), row["states"], "{name}");
        }
    }
}

#[test]
fn corrupted_trace_rejected_and_guarded_deadlock_certified() {
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
    let report = dive_api::simulate_file(&SimulateRequest {
        file: fixture(),
        seed: Some(0),
        depth: Some(3),
        traces: 1,
        ..Default::default()
    })
    .unwrap()
    .to_json();
    let mut doc = TraceDoc::from_json(&report["traces"][0]["document"]).unwrap();
    doc.states[1]["x"] = serde_json::json!(42);
    let model = SpecModel::load(
        &fixture(),
        &fixture().with_extension("cfg"),
        &TraceOptions::default(),
    )
    .unwrap();
    assert!(matches!(
        model.validate(&doc).validation,
        Validation::Invalid(_)
    ));
    let dir = std::env::temp_dir().join(format!("dive-simulation-cover-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Uncovered.tla");
    std::fs::write(&path, "---- MODULE Uncovered ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x > 0 /\\ x' \\in Nat\n====\n").unwrap();
    std::fs::write(path.with_extension("cfg"), "INIT Init\nNEXT Next\n").unwrap();
    let json = dive_api::simulate_file(&SimulateRequest {
        file: path,
        seed: Some(0),
        traces: 1,
        ..Default::default()
    })
    .unwrap()
    .to_json();
    // The false guard `x > 0` leaves no successor: ENABLED Next is FALSE.
    assert_eq!(json["observed_result"], "violation");
    assert_eq!(json["observed_tlc_code"], 2114);
    assert_eq!(json["result"], "violation", "{json}");
    assert_eq!(json["violation_validated"], true);
    assert_eq!(json["exit_code"], 11);
    assert_eq!(json["options"]["workers_used"], 1);
}

#[test]
fn unrepresentable_sample_retains_execution_statistics() {
    let dir = std::env::temp_dir().join(format!("dive-simulation-json-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("SymbolicState.tla");
    std::fs::write(&path, "---- MODULE SymbolicState ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = Nat\n====\n").unwrap();
    std::fs::write(path.with_extension("cfg"), "INIT Init\nNEXT Next\n").unwrap();
    let report = dive_api::simulate_file(&SimulateRequest {
        file: path,
        seed: Some(0),
        traces: 1,
        depth: Some(1),
        ..Default::default()
    })
    .unwrap();
    let json = report.to_json();
    assert_eq!(json["result"], "unsupported", "{json}");
    assert_eq!(report.exit_code(), 3);
    assert_eq!(json["options"]["workers_used"], 1);
    assert_eq!(json["options"]["execution_stage"], "sampling");
    assert_eq!(json["statistics"]["generated"], 2);
    assert_eq!(json["statistics"]["traces_started"], 1);
    // Sampling completed; only evidence formatting failed. Do not erase actual work.
    assert_eq!(json["statistics"]["scope"], "complete-request");
    assert_eq!(json["observed_result"], "sampled");
    assert_eq!(json["model_proved"], false);
    assert!(json["message"].as_str().unwrap().contains("infinite set"));
}

#[test]
fn tlc_runtime_name_sampling_refuses_before_uncertifiable_traces() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-trace/tests/fixtures/runtime-names/ReplayNames.tla");
    let request = SimulateRequest {
        file,
        compat_tlc: true,
        seed: Some(0),
        depth: Some(3),
        traces: 1,
        ..Default::default()
    };
    let report = dive_api::simulate_file(&request).unwrap().to_json();
    assert_eq!(report["result"], "unsupported", "{report}");
    assert_eq!(report["exhaustive"], false);
    assert_eq!(report["model_proved"], false);
    assert_eq!(
        report["options"]["execution_stage"], "preflight",
        "{report}"
    );
    assert_eq!(report["traces"].as_array().unwrap().len(), 0);
    let sampled = dive_api::simulate_file(&SimulateRequest {
        compat_tlc: false,
        ..request
    })
    .unwrap()
    .to_json();
    assert_eq!(sampled["result"], "sampled", "{sampled}");
    assert_eq!(
        sampled["traces"][0]["validation"]["result"], "valid",
        "{sampled}"
    );
}

#[test]
fn basic_action_statistics_preserve_replay_and_independent_safety_validation() {
    for compat_tlc in [false, true] {
        for name in ["SimChoice", "SimInvariant"] {
            let mut request = SimulateRequest {
                file: fixture().with_file_name(format!("{name}.tla")),
                compat_tlc,
                seed: Some(7),
                depth: Some(5),
                traces: 3,
                ..Default::default()
            };
            let plain = dive_api::simulate_file(&request).unwrap().to_json();
            request.action_stats = true;
            let counted_report = dive_api::simulate_file(&request).unwrap();
            assert!(
                counted_report
                    .render_human()
                    .contains("action flow (chosen transitions)")
            );
            let counted = counted_report.to_json();
            assert_eq!(
                counted,
                dive_api::simulate_file(&request).unwrap().to_json()
            );
            for field in [
                "result",
                "observed_result",
                "tlc_code",
                "exit_code",
                "statistics",
                "traces",
            ] {
                assert_eq!(counted[field], plain[field], "{field}");
                let counted = match field {
                    "traces" => unlabelled(&counted[field]),
                    "statistics" => recorded(&counted[field]),
                    _ => counted[field].clone(),
                };
                if compat_tlc {
                    // The documented compat choice-stream gap: Core's own sample.
                    agreed::check_core(
                        CORE,
                        "basic_action_statistics_preserve_replay_and_independent_safety_validation/0",
                        &counted,
                    );
                } else {
                    agreed::check(
                        AGREED,
                        "basic_action_statistics_preserve_replay_and_independent_safety_validation/0",
                        &counted,
                    );
                }
            }
            assert_eq!(counted["model_proved"], false);
            assert!(plain["action_flow"].is_null());
            assert_eq!(counted["action_flow"]["mode"], "basic");
            assert_eq!(counted["action_flow"]["scope"], "chosen-transitions");
            if name == "SimInvariant" {
                assert_eq!(counted["result"], "violation", "{counted}");
                assert_eq!(counted["violation_validated"], true, "{counted}");
                let mut document =
                    dive_api::trace::TraceDoc::from_json(&counted["traces"][0]["document"])
                        .unwrap();
                document.states[1]["x"] = serde_json::json!(42);
                let independent = dive_api::trace::SpecModel::load(
                    &request.file,
                    &request.file.with_extension("cfg"),
                    &dive_api::trace::TraceOptions {
                        compat_tlc,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert!(matches!(
                    independent.validate(&document).validation,
                    dive_api::trace::Validation::Invalid(_)
                ));
                // A failing candidate is retained as evidence but was never chosen.
                assert_eq!(
                    counted["action_flow"]["transitions"]
                        .as_array()
                        .unwrap()
                        .as_slice(),
                    [] as [Value; 0]
                );
            }
            // Flows are summed over the traces in id order, so they do not
            // depend on the worker count either.
            let single = dive_api::simulate_file(&request).unwrap().to_json();
            request.workers = Some(4);
            let parallel = dive_api::simulate_file(&request).unwrap().to_json();
            assert_eq!(parallel["options"]["workers_requested"], 4);
            assert_eq!(parallel["options"]["workers_used"], 4);
            for field in ["result", "statistics", "traces", "action_flow"] {
                assert_eq!(parallel[field], single[field], "{field}");
            }
            request.workers = None;
        }
    }
}

/// `traces` with each step's action label cleared, as sampled documents were
/// before they recorded labels: the records and cross-file comparisons pin
/// the sampled states, while the labels' sources differ between files and the
/// validator checks each label on its step.
/// Statistics without the retention counts, which the agreed records
/// predate.
fn recorded(statistics: &Value) -> Value {
    let mut statistics = statistics.clone();
    if let Some(map) = statistics.as_object_mut() {
        map.remove("traces_kept");
        map.remove("traces_dropped");
    }
    statistics
}

fn unlabelled(traces: &Value) -> Value {
    let mut traces = traces.clone();
    for trace in traces.as_array_mut().into_iter().flatten() {
        for state in trace["document"]["states"]
            .as_array_mut()
            .into_iter()
            .flatten()
        {
            state["action"] = Value::Null;
        }
    }
    traces
}
