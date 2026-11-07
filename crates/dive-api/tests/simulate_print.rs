//! Ordinary simulation effects use actual evaluation and quiet fresh validation.
#![allow(
    clippy::expect_used,
    reason = "fixture/setup failures are test failures"
)]

use std::path::PathBuf;

use dive_api::SimulateRequest;
use dive_api::trace::{ControlMode, SpecModel, Trace, TraceDoc, TraceOptions, Validation};
use serde_json::{Value, json};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/runtime-print")
}

fn request(name: &str, depth: usize, compat_tlc: bool) -> SimulateRequest {
    SimulateRequest {
        file: fixtures().join(format!("{name}.tla")),
        traces: 1,
        depth: Some(depth),
        seed: Some(0),
        compat_tlc,
        ..Default::default()
    }
}

fn texts(events: &Value) -> Vec<&str> {
    events
        .as_array()
        .expect("events")
        .iter()
        .map(|event| event["text"].as_str().expect("text"))
        .collect()
}

#[test]
fn direct_captured_and_configured_prints_match_genuine_primary_words() {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/runtime-print/oracle.json"
    ))
    .expect("oracle");
    for row in oracle["primary_probes"].as_array().expect("rows") {
        let name = row["model"].as_str().expect("model");
        let depth = usize::try_from(row["depth"].as_u64().expect("depth")).expect("bounded depth");
        for compat_tlc in [false, true] {
            let req = request(name, depth, compat_tlc);
            let report = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
                .expect("simulation");
            let result = report.to_json();
            check_print_statistics(&req, row, &result);
            if row["native_scope"] == "unsupported" && name != "RuntimePrintConditional" {
                assert_eq!(result["result"], "unsupported");
                assert_eq!(result["violation_validated"], false);
                assert_eq!(result["options"]["workers_used"], 1);
                assert_eq!(texts(&result["runtime_output"]), ["0"]);
                assert_eq!(result["statistics"]["scope"], "completed-prefix");
                let refused = if name == "RuntimePrintRegister" {
                    "TLCSet"
                } else {
                    "RandomElement"
                };
                assert!(
                    result["message"]
                        .as_str()
                        .expect("refusal")
                        .contains(refused)
                );
                continue;
            }
            let printed: Vec<_> = texts(&result["startup_output"])
                .into_iter()
                .chain(texts(&result["runtime_output"]))
                .collect();
            assert_eq!(json!(printed), row["printed"]);
            if name == "RuntimePrintDirect" {
                assert_eq!(texts(&result["startup_output"]), ["\"startup\""]);
                assert_eq!(texts(&result["runtime_output"])[0], "\"init\"");
            } else {
                assert_eq!(texts(&result["startup_output"]), Vec::<&str>::new());
            }
            assert_eq!(result["statistics"]["generated"], row["generated"]);
            let expected_code = row["codes"]
                .as_array()
                .expect("codes")
                .first()
                .cloned()
                .unwrap_or(Value::Null);
            assert_eq!(result["observed_tlc_code"], expected_code);
            if row["exit"] == 12 {
                assert_eq!(result["observed_violation"]["name"], "Safe");
                assert_eq!(result["observed_violation"]["kind"], "invariant");
                assert_eq!(result["violation_validated"], true);
            } else {
                assert_eq!(result["result"], "sampled");
                assert_eq!(result["observed_violation"], Value::Null);
            }
            assert_eq!(result["exhaustive"], false);
            assert_eq!(result["model_proved"], false);
            let trace = &result["traces"][0];
            assert_eq!(trace["validation"]["result"], "valid");
            let xs: Vec<_> = trace["document"]["states"]
                .as_array()
                .expect("states")
                .iter()
                .map(|state| state["values"]["x"].clone())
                .collect();
            assert_eq!(json!(xs), row["states"]);
            if compat_tlc {
                assert_eq!(json!(report.exit_code()), row["exit"]);
            }
            check_independent_word(&req, &result);
        }
    }
}

fn check_print_statistics(req: &SimulateRequest, row: &Value, plain: &Value) {
    assert_eq!(plain["action_flow"], Value::Null);
    let counted_req = SimulateRequest {
        action_stats: true,
        ..req.clone()
    };
    let counted_report = dive_api::simulate_file_engine(&counted_req, dive_api::Engine::Reference)
        .expect("Print with basic statistics");
    let counted = counted_report.to_json();
    check_statistics_toggle_fields(&counted, plain);
    assert_eq!(counted["options"]["action_stats"], true);
    assert_eq!(counted["action_flow"]["mode"], "basic");
    assert_eq!(counted["action_flow"]["scope"], "chosen-transitions");
    let chosen: u64 = counted["action_flow"]["transitions"]
        .as_array()
        .expect("transitions")
        .iter()
        .map(|edge| edge["count"].as_u64().expect("count"))
        .sum();
    // These fixtures choose only x=1. The x=2 invariant candidate is not chosen.
    // A refused register access has no chosen edge; it earns no valid-word credit.
    let expected = u64::from(row["model"] != "RuntimePrintRegister");
    assert_eq!(chosen, expected);
    assert!(
        counted_report
            .render_human()
            .contains("action flow (chosen transitions)")
    );
    if row["model"] != "RuntimePrintRegister" {
        check_independent_word(&counted_req, &counted);
    }
}

fn check_statistics_toggle_fields(counted: &Value, plain: &Value) {
    for field in [
        "result",
        "observed_result",
        "observed_tlc_code",
        "tlc_code",
        "exit_code",
        "statistics",
        "traces",
        "startup_output",
        "runtime_output",
        "observed_violation",
        "pending_violation",
        "violation_validated",
        "exhaustive",
        "model_proved",
        "message",
    ] {
        assert_eq!(counted[field], plain[field], "Print stats toggle: {field}");
    }
    for option in [
        "seed",
        "aril",
        "depth",
        "traces",
        "workers_used",
        "execution_stage",
    ] {
        assert_eq!(counted["options"][option], plain["options"][option]);
    }
}

fn check_independent_word(req: &SimulateRequest, result: &Value) {
    let trace = &result["traces"][0];
    let model = SpecModel::load(
        &req.file,
        &req.file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            ..Default::default()
        },
    )
    .expect("validator model");
    let before = model.model().evaluator.has_print_event();
    let mut doc = TraceDoc::from_json(&trace["document"]).expect("document");
    for _ in 0..2 {
        assert_eq!(model.validate(&doc).validation, Validation::Valid);
        assert_eq!(model.model().evaluator.has_print_event(), before);
        assert_eq!(model.model().evaluator.take_output(), Vec::<String>::new());
    }
    let typed = Trace::decode(model.model(), &doc).expect("typed word");
    assert_eq!(model.validate_trace(&typed).validation, Validation::Valid);
    if result["result"] == "violation" {
        // These are actual finite producer counterexamples, not arbitrary
        // valid behaviors. No shortest-path premise is introduced.
        let mutants = model.mutants_with_mode(&typed, ControlMode::Counterexample);
        assert_eq!(mutants.len(), 3);
        for mutant in mutants {
            assert!(
                matches!(
                    model.validate_trace(&mutant.trace).validation,
                    Validation::Invalid(_)
                ),
                "direct {}",
                mutant.name
            );
            assert!(
                matches!(
                    model
                        .validate(&model.to_doc(&mutant.trace).expect("mutant doc"))
                        .validation,
                    Validation::Invalid(_)
                ),
                "document {}",
                mutant.name
            );
        }
        assert_eq!(
            model
                .controls_with_mode(&doc, ControlMode::Counterexample)
                .failures(),
            Vec::<String>::new()
        );
    }
    doc.states[1].insert("x".to_owned(), json!(42));
    let bad = Trace::decode(model.model(), &doc).expect("bad typed word");
    assert!(matches!(
        model.validate_trace(&bad).validation,
        Validation::Invalid(_)
    ));
    assert!(matches!(
        model.validate(&doc).validation,
        Validation::Invalid(_)
    ));
    assert_eq!(model.model().evaluator.take_output(), Vec::<String>::new());
}

#[test]
fn print_domains_preserve_every_existing_seeded_choice_and_count() {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/choice-oracle.json"
    ))
    .expect("oracle");
    for row in oracle.as_array().expect("rows") {
        let mut req = request("SimChoicePrinted", 5, true);
        req.traces = 3;
        req.seed = row["seed"].as_i64();
        req.aril = row["aril"].as_u64().expect("aril");
        let printed = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
            .expect("printed run")
            .to_json();
        req.action_stats = true;
        let counted = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
            .expect("printed basic run")
            .to_json();
        check_statistics_toggle_fields(&counted, &printed);
        assert_eq!(counted["result"], "sampled");
        req.action_stats = false;
        req.file = fixtures()
            .parent()
            .expect("fixtures parent")
            .join("SimChoice.tla");
        let plain = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
            .expect("plain run")
            .to_json();
        assert_eq!(printed["statistics"], plain["statistics"]);
        assert_eq!(printed["statistics"]["generated"], row["generated"]);
        let words = |result: &Value| {
            result["traces"]
                .as_array()
                .expect("traces")
                .iter()
                .map(|trace| {
                    trace["tlc_states"]
                        .as_array()
                        .expect("states")
                        .iter()
                        .map(|s| s["x"].clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(json!(words(&printed)), row["traces"]);
        assert_eq!(printed["traces"], plain["traces"]);
        assert_ne!(texts(&printed["runtime_output"]), Vec::<&str>::new());
        assert_eq!(texts(&plain["runtime_output"]), Vec::<&str>::new());
    }
}

#[test]
fn print_argument_errors_do_not_become_valid_fresh_words() {
    for compat_tlc in [false, true] {
        let model = SpecModel::from_source("BadPrint.tla", "---- MODULE BadPrint ----\nEXTENDS Integers, TLC\nVARIABLE x\nInit == x=0\nNext == x'=Print(\"bad\",1 \\div 0)\n====\n", "INIT Init\nNEXT Next\n", &TraceOptions { compat_tlc, ..Default::default() }).expect("model");
        let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})]).expect("doc");
        assert!(matches!(
            model.validate(&doc).validation,
            Validation::Invalid(_)
        ));
        assert_eq!(model.model().evaluator.take_output(), Vec::<String>::new());
    }
}

#[test]
fn lazy_print_returns_are_checked_without_replay_output() {
    for compat_tlc in [false, true] {
        let model = SpecModel::from_source("LazyPrint.tla", "---- MODULE LazyPrint ----\nEXTENDS Naturals, TLC\nVARIABLE x\nTwice(v) == v+v\nInit == x=0\nNext == x'=Twice(Print(\"lazy\",x+1))\n====\n", "INIT Init\nNEXT Next\n", &TraceOptions { compat_tlc, ..Default::default() }).expect("model");
        let mut doc =
            TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2}), json!({"x":6})]).expect("doc");
        assert_eq!(model.validate(&doc).validation, Validation::Valid);
        let typed = Trace::decode(model.model(), &doc).expect("typed");
        assert_eq!(model.validate_trace(&typed).validation, Validation::Valid);
        doc.states[2].insert("x".to_owned(), json!(7));
        assert!(matches!(
            model.validate(&doc).validation,
            Validation::Invalid(_)
        ));
        let bad = Trace::decode(model.model(), &doc).expect("bad typed");
        assert!(matches!(
            model.validate_trace(&bad).validation,
            Validation::Invalid(_)
        ));
        assert_eq!(model.model().evaluator.take_output(), Vec::<String>::new());
        assert!(!model.model().evaluator.has_print_event());
    }
}

#[test]
fn print_domains_keep_the_default_stream_and_prefix() {
    let mut req = request("SimChoicePrinted", 5, false);
    req.seed = Some(7);
    req.traces = 3;
    let printed = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
        .expect("printed")
        .to_json();
    req.action_stats = true;
    let counted = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
        .expect("printed basic")
        .to_json();
    check_statistics_toggle_fields(&counted, &printed);
    assert_eq!(counted["result"], "sampled");
    req.action_stats = false;
    req.file = fixtures()
        .parent()
        .expect("fixtures parent")
        .join("SimChoice.tla");
    let plain = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
        .expect("plain")
        .to_json();
    assert_eq!(printed["statistics"], plain["statistics"]);
    assert_eq!(printed["traces"], plain["traces"]);
    req.file = fixtures().join("SimChoicePrinted.tla");
    req.traces = 5;
    let longer = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
        .expect("longer")
        .to_json();
    assert_eq!(
        printed["traces"].as_array().expect("three"),
        &longer["traces"].as_array().expect("five")[..3]
    );
    assert_ne!(texts(&printed["runtime_output"]), Vec::<&str>::new());
    assert_eq!(printed["exhaustive"], false);
    assert_eq!(printed["model_proved"], false);
}
