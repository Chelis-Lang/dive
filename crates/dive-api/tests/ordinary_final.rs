//! Ordinary compat requests reuse final checking without changing safety startup.
#[path = "support/refused.rs"]
mod refused;
use std::path::PathBuf;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Spec(PathBuf);
impl Spec {
    fn new(body: &str, config: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!(
            "dive-ordinary-final-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&directory)?;
        let file = directory.join("Ordinary.tla");
        std::fs::write(
            &file,
            format!("---- MODULE Ordinary ----\nEXTENDS Naturals,TLC\n{body}\n====\n"),
        )?;
        std::fs::write(file.with_extension("cfg"), config)?;
        Ok(Self(file))
    }
    fn request(&self, workers: usize) -> CheckRequest {
        CheckRequest {
            file: self.0.clone(),
            compat_tlc: true,
            seed: Some(7),
            fp: Some(0),
            workers: Some(workers),
            ..CheckRequest::default()
        }
    }
}
/// Checks every grown completed level, so small graphs exercise prefixes.
const EVERY_LEVEL: dive_api::PeriodicSchedule = dive_api::PeriodicSchedule {
    min_states: 2,
    growth_factor: 1,
};

impl Drop for Spec {
    fn drop(&mut self) {
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";
const COUNTER: &str = "VARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\n";

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one matrix checks both properties against the pinned counts"
)]
fn ordinary_compat_final_truth_has_actual_counts_and_freshly_validated_words() -> TestResult {
    for (property, violation) in [("[]<>(x<=2)", false), ("<>(x=3)", true)] {
        let spec = Spec::new(&format!("{COUNTER}P == {property}"), CONFIG)?;
        for workers in [1, 4] {
            let request = spec.request(workers);
            let report = dive_api::check_file(&request)?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert!(report.result().is_none());
            assert!(report.temporal_result().is_some());
            assert_eq!(value["schema_version"], 3);
            assert_eq!(value["check_kind"], "temporal");
            assert_eq!(value["options"]["profile"], "tlc");
            assert_eq!(value["options"]["lncheck"], "final");
            assert_eq!(value["options"]["lasso"], "short");
            assert_eq!(value["execution"]["exhausted"], true);
            assert_eq!(value["execution"]["capture_completed"], true);
            assert_eq!(value["statistics"]["generated"], 4);
            assert_eq!(value["statistics"]["distinct"], 3);
            assert_eq!(value["statistics"]["depth"], 3);
            assert_eq!(value["statistics"]["left_on_queue"], 0);
            assert_eq!(
                report.exit_code_for(ExitConvention::Tlc),
                if violation { 13 } else { 0 }
            );
            if violation {
                let document = TraceDoc::from_json(&value["counterexample"]["trace"])?;
                let model = SpecModel::load(
                    &spec.0,
                    &spec.0.with_extension("cfg"),
                    &TraceOptions {
                        compat_tlc: true,
                        run: Some((7, 0)),
                        ..TraceOptions::default()
                    },
                )?;
                assert_eq!(model.read(&document).0.validation, Validation::Valid);
                let mut forged = document;
                forged.states[0].insert("x".into(), 99.into());
                assert!(matches!(
                    model.read(&forged).0.validation,
                    Validation::Invalid(_)
                ));
                assert_eq!(value["property"], "P");
                assert_eq!(value["tlc"]["code"], 2116);
            } else {
                assert_eq!(value["verdict"], "success");
                assert_eq!(value["tlc"]["code"], 2193);
                assert!(value["counterexample"].is_null());
            }
            let mut explicit = request.clone();
            explicit.temporal_refusal = Some(refused::FINAL_SHORT.into());
            // Only the retired reference engine ran bounded final checking
            // (#791, #910).
            let original = dive_api::check_file(&explicit)?;
            refused::assert_refused(
                &original.to_json(),
                original.exit_code(),
                refused::FINAL_SHORT,
            );
            let mut native = request;
            native.compat_tlc = false;
            // Below the default schedule's minimum, the default profile checks
            // once at exhaustion and reports the final truth and counts.
            let default = dive_api::check_file(&native)?.to_json();
            assert_eq!(default["options"]["profile"], "dive");
            assert_eq!(default["options"]["lncheck"], "deterministic");
            assert_eq!(default["periodic"]["min_states"], 65_536);
            assert_eq!(default["periodic"]["growth_factor"], 2);
            assert_eq!(default["periodic"]["prefix_checks"], 0);
            assert_eq!(default["periodic"]["final_check"], true);
            assert_eq!(default["execution"]["exhausted"], true);
            assert_eq!(value["verdict"], default["verdict"]);
            assert_eq!(value["statistics"], default["statistics"]);
            native.periodic = EVERY_LEVEL;
            let native = dive_api::check_file(&native)?;
            let value = native.to_json();
            assert_eq!(value["schema_version"], 3);
            assert_eq!(value["options"]["lncheck"], "deterministic");
            assert_eq!(value["periodic"]["exhausted"], !violation);
            assert_eq!(value["execution"]["capture_completed"], !violation);
            if violation {
                assert_eq!(value["verdict"], "liveness-violation");
                assert_eq!(value["property"], "P");
                assert_eq!(value["counterexample"]["validation"], "valid");
                assert_eq!(value["statistics"]["left_on_queue"], 1);
                assert_eq!(value["execution"]["level"], 1);
            } else {
                assert_eq!(value["verdict"], "success");
                assert_eq!(value["periodic"]["final_check"], true);
                assert_eq!(value["statistics"]["left_on_queue"], 0);
            }
        }
    }
    Ok(())
}

/// What pinned TLC reports for `COUNTER` with `P` as its only property:
/// the verdict and the generated, distinct and depth counts.
fn expected_safety(property: &str) -> (dive_runtime::Verdict, u64, u64, u64) {
    if property == "[](x<2)" {
        (
            dive_runtime::Verdict::InvariantViolated {
                property: "P".into(),
                initial: false,
            },
            3,
            3,
            3,
        )
    } else {
        (dive_runtime::Verdict::Success, 4, 3, 3)
    }
}

fn assert_safety(actual: &dive_runtime::CheckResult, property: &str, workers: usize) {
    let (verdict, generated, distinct, depth) = expected_safety(property);
    let stats = actual
        .stats
        .as_ref()
        .map(|s| (s.generated, s.distinct, s.depth));
    assert_eq!(actual.verdict, verdict, "{property}");
    assert_eq!(stats, Some((generated, distinct, depth)), "{property}");
    assert_eq!(actual.workers.requested, workers, "{property}");
    assert!(actual.output.is_empty(), "{property}");
    assert_eq!(
        actual.counterexample.is_some(),
        verdict != dive_runtime::Verdict::Success,
        "{property}"
    );
}

#[test]
fn configured_initial_invariant_action_and_mixed_safety_roles_keep_the_full_tail() -> TestResult {
    for property in [
        "x=0",
        "[](x<2)",
        "[][x'>=x]_x",
        "x=0 /\\ [](x<=2) /\\ [][x'>=x]_x",
    ] {
        let spec = Spec::new(&format!("{COUNTER}P == {property}"), CONFIG)?;
        for workers in [1, 4] {
            let report = dive_api::check_file(&spec.request(workers))?;
            // TLC also warns (2255) that a state-level `P` is used as a
            // PROPERTY; Core does not report that warning (#910).
            let actual = report.result().ok_or("ordinary safety result")?;
            assert_safety(actual, property, workers);
            assert!(!actual.binding_model_error);
            assert_eq!(report.to_json()["schema_version"], 3);
            assert_eq!(report.to_json()["check_kind"], "safety");
        }
    }
    Ok(())
}

#[test]
fn configured_safety_roles_keep_safety_family_and_the_original_print_rng_stream() -> TestResult {
    let spec = Spec::new(
        "VARIABLE x\nBoot == Print(\"one-startup\",RandomElement(1..32))\nInit == x=RandomElement(1..32)\nNext == x'=IF x<Boot THEN x+1 ELSE x\nP == [](x<=32)",
        CONFIG,
    )?;
    for seed in 0..16 {
        let mut request = spec.request(4);
        request.seed = Some(seed);
        // Only the retired reference engine continued TLC's start-up random
        // stream into Init (#791, #910); Core refuses it by name.
        let report = dive_api::check_file(&request)?;
        let value = report.to_json();
        refused::assert_refused(
            &value,
            report.exit_code(),
            "RandomElement in Init after constant definitions or ASSUMEs drew",
        );
        assert_eq!(value["schema_version"], 3);
        assert!(value.get("execution").is_none());
    }
    Ok(())
}

#[test]
fn ordinary_binding_failures_and_excluded_options_keep_their_existing_routes() -> TestResult {
    let failed = Spec::new(
        "CONSTANT C\nBoot == Print(\"must-not-start\",0)\nVARIABLE x\nInit == x=C\nNext == x'=x\nP == x=0",
        CONFIG,
    )?;
    let report = dive_api::check_file(&failed.request(1))?;
    let actual = report.result().ok_or("ordinary binding result")?;
    assert!(
        matches!(
            &actual.verdict,
            dive_runtime::Verdict::ModelError { code: 2222, .. }
        ),
        "{:?}",
        actual.verdict
    );
    assert_eq!(actual.output.len(), 0, "{:?}", actual.output);
    assert!(actual.binding_model_error);
    assert_eq!(report.to_json()["schema_version"], 3);
    assert_eq!(report.to_json()["check_kind"], "safety");
    let temporal = Spec::new(&format!("{COUNTER}P == <>(x=3)"), CONFIG)?;
    let mut requests = Vec::new();
    let mut request = temporal.request(1);
    request.chaos = Some(0);
    requests.push(request);
    let mut request = temporal.request(1);
    request.frontier_memory_bytes = Some(0);
    requests.push(request);
    let mut request = temporal.request(1);
    request.predecessor_memory_bytes = Some(0);
    requests.push(request);
    for request in requests {
        let report = dive_api::check_file(&request)?;
        assert_eq!(report.verdict(), "unsupported");
        assert!(report.result().is_some());
        assert_eq!(report.to_json()["schema_version"], 3);
        assert_eq!(report.to_json()["check_kind"], "safety");
        let message = report.to_json()["message"].clone();
        assert!(
            message
                .as_str()
                .is_some_and(|s| s.contains(if request.chaos.is_some() {
                    refused::CHAOS
                } else {
                    "temporal"
                })),
            "{message}"
        );
    }
    Ok(())
}

#[test]
fn ordinary_temporal_honors_store_polynomial_deadlock_and_sequential_requests() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == <>(x=3)"), CONFIG)?;
    let baseline =
        dive_api::check_file(&spec.request(1))?.to_json_with_exit_convention(ExitConvention::Tlc);
    assert_eq!(baseline["verdict"], "liveness-violation");
    let mut requests = Vec::new();
    let mut request = spec.request(1);
    request.fp = Some(5);
    requests.push(request);
    let mut request = spec.request(1);
    request.store = dive_api::Store::Exact;
    requests.push(request);
    let mut request = spec.request(1);
    request.no_deadlock = true;
    requests.push(request);
    let mut request = spec.request(4);
    request.sequential = true;
    requests.push(request);
    for request in requests {
        let report = dive_api::check_file(&request)?;
        let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
        assert!(report.temporal_result().is_some(), "{request:?}");
        assert_eq!(value["check_kind"], "temporal");
        assert_eq!(value["verdict"], baseline["verdict"]);
        assert_eq!(value["property"], baseline["property"]);
        assert_eq!(value["statistics"], baseline["statistics"]);
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 13);
    }
    // Pinned TLC (`-workers 1 -lncheck final -fp 0 -seed 0`): deadlock at
    // x = 2 with deadlock checking, and P violated with `-deadlock`; both
    // with 3 distinct and 3 generated states.
    let dead = Spec::new(
        "VARIABLE x\nInit == x=0\nNext == x<2 /\\ x'=x+1\nP == <>(x=2)",
        CONFIG,
    )?;
    let checked = dive_api::check_file(&dead.request(1))?;
    assert_eq!(checked.verdict(), "deadlock");
    assert_eq!(checked.exit_code_for(ExitConvention::Tlc), 11);
    let mut request = dead.request(1);
    request.no_deadlock = true;
    let report = dive_api::check_file(&request)?;
    let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
    assert_eq!(value["verdict"], "liveness-violation");
    assert_eq!(value["property"], "P");
    assert_eq!(value["statistics"]["distinct"], 3);
    assert_eq!(value["statistics"]["generated"], 3);
    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 13);
    let mut request = spec.request(1);
    request.chaos = Some(0);
    let chaotic = dive_api::check_file(&request)?;
    refused::assert_refused(&chaotic.to_json(), chaotic.exit_code(), refused::CHAOS);
    Ok(())
}

#[test]
fn ordinary_temporal_planning_errors_never_turn_into_holds() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == <>1"), CONFIG)?;
    let report = dive_api::check_file(&spec.request(1))?;
    // TLC reports its planning error (2215, exit 255). Only the retired
    // reference engine built that report (#791, #910); Core refuses it and
    // never reports a hold.
    let value = report.to_json();
    refused::assert_refused(
        &value,
        report.exit_code(),
        refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
    );
    assert_eq!(value["schema_version"], 3);
    assert!(value["statistics"].is_null());
    Ok(())
}

#[test]
fn file_final_requests_report_compact_acceptance_defaults() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == []<>FALSE"), CONFIG)?;
    for workers in [1, 4] {
        for compat_tlc in [false, true] {
            for mode in [refused::Mode::Ordinary, refused::Mode::FinalShort] {
                let mut request = spec.request(workers);
                request.compat_tlc = compat_tlc;
                request.temporal_refusal = mode.refusal();
                let report = dive_api::check_file(&request)?;
                let json = report.to_json();
                if mode == refused::Mode::FinalShort {
                    // Only the retired reference engine ran the file route
                    // (#791, #910); the refusal reports no bounds.
                    refused::assert_refused(&json, report.exit_code(), refused::FINAL_SHORT);
                    assert!(json["options"].get("temporal_bounds").is_none());
                    continue;
                }
                let bounds = &json["options"]["temporal_bounds"];
                assert_eq!(bounds["total_memory_budget"], false);
                assert!(bounds.get("acceptance_cells").is_none());
                // Core's ordinary temporal checking has no acceptance caps.
                assert_eq!(
                    bounds["acceptance_buffer_bytes"],
                    serde_json::json!(usize::MAX)
                );
                assert_eq!(
                    bounds["acceptance_materialization_work"],
                    serde_json::json!(usize::MAX)
                );
                assert_eq!(report.verdict(), "liveness-violation");
                let document = TraceDoc::from_json(&json["counterexample"]["trace"])?;
                let model = SpecModel::load(
                    &spec.0,
                    &spec.0.with_extension("cfg"),
                    &TraceOptions {
                        compat_tlc,
                        run: Some((7, 0)),
                        ..TraceOptions::default()
                    },
                )?;
                assert_eq!(model.read(&document).0.validation, Validation::Valid);
            }
        }
    }
    Ok(())
}

#[test]
fn default_periodic_reports_actual_boundary_and_fresh_word_at_all_widths() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == <>(x=3)"), CONFIG)?;
    for workers in [1, 4, 8] {
        let mut request = spec.request(workers);
        request.compat_tlc = false;
        request.periodic = EVERY_LEVEL;
        let report = dive_api::check_file(&request)?;
        let value = report.to_json();
        assert_eq!(value["verdict"], "liveness-violation");
        assert_eq!(value["schema_version"], 3);
        assert_eq!(value["options"]["lncheck"], "deterministic");
        assert_eq!(value["options"]["seed"], 7);
        assert_eq!(value["options"]["workers"]["used"], workers);
        assert_eq!(value["statistics"]["generated"], 2);
        assert_eq!(value["statistics"]["distinct"], 2);
        assert_eq!(value["statistics"]["depth"], 2);
        assert_eq!(value["statistics"]["left_on_queue"], 1);
        assert_eq!(value["execution"]["capture_completed"], false);
        assert_eq!(value["execution"]["exhausted"], false);
        assert_eq!(value["execution"]["level"], 1);
        assert_eq!(value["periodic"]["prefix_checks"], 1);
        assert_eq!(value["periodic"]["min_states"], 2);
        assert_eq!(value["periodic"]["growth_factor"], 1);
        assert_eq!(value["periodic"]["last_level"], 1);
        assert_eq!(value["periodic"]["last_size"], 2);
        assert_eq!(value["periodic"]["final_check"], false);
        let document = TraceDoc::from_json(&value["counterexample"]["trace"])?;
        let model = SpecModel::load(
            &spec.0,
            &spec.0.with_extension("cfg"),
            &TraceOptions {
                run: Some((7, 0)),
                ..TraceOptions::default()
            },
        )?;
        assert_eq!(model.read(&document).0.validation, Validation::Valid);
        let mut forged = document;
        forged.states[0].insert("x".into(), 99.into());
        assert!(matches!(
            model.read(&forged).0.validation,
            Validation::Invalid(_)
        ));
        request.temporal_refusal = Some(refused::FINAL_SHORT.into());
        // Only the retired reference engine ran bounded final checking
        // (#791, #910).
        let final_report = dive_api::check_file(&request)?;
        refused::assert_refused(
            &final_report.to_json(),
            final_report.exit_code(),
            refused::FINAL_SHORT,
        );
    }
    Ok(())
}

#[test]
fn default_periodic_keeps_safety_startup_and_initial_error_priority() -> TestResult {
    for property in ["x=0", "[](x<2)", "[][x'>=x]_x"] {
        let spec = Spec::new(&format!("{COUNTER}P == {property}"), CONFIG)?;
        for workers in [1, 4, 8] {
            let mut request = spec.request(workers);
            request.compat_tlc = false;
            let report = dive_api::check_file(&request)?;
            let actual = report.result().ok_or("ordinary safety route")?;
            assert_safety(actual, property, workers);
            assert_eq!(actual.workers.used, workers);
            assert_eq!(report.to_json()["schema_version"], 3);
            assert_eq!(report.to_json()["check_kind"], "safety");
        }
    }
    let spec = Spec::new(
        "VARIABLE x\nBoot == Print(\"one-startup\",0)\nInit == x=Boot\nNext == x'=x+1\nSafe == x<0\nP == <>(x=3)",
        "INIT Init\nNEXT Next\nINVARIANT Safe\nPROPERTY P\n",
    )?;
    // TLC reports the initial `Safe` violation after printing once. A
    // temporal capture whose start-up has an effect runs with one worker at
    // any requested width, and says why.
    let mut documents = Vec::new();
    for workers in [1, 4] {
        let mut request = spec.request(workers);
        request.compat_tlc = false;
        let report = dive_api::check_file(&request)?;
        let json = report.to_json();
        assert_eq!(json["schema_version"], 3);
        assert_eq!(json["verdict"], "invariant-violation", "{json}");
        assert_eq!(json["property"], "Safe");
        assert_eq!(json["output"], serde_json::json!(["\"one-startup\"  0"]));
        assert_eq!(json["statistics"]["distinct"], 1);
        assert!(report.temporal_result().is_none());
        let used = &json["options"]["workers"];
        assert_eq!(used["requested"], workers);
        assert_eq!(used["used"], 1);
        if workers == 1 {
            assert!(used["reason"].is_null());
        } else {
            let reason = used["reason"].as_str().ok_or("one-worker reason")?;
            assert!(reason.contains("with one worker"), "{reason}");
            assert!(report.render_human().contains(reason));
        }
        documents.push((json["statistics"].clone(), json["counterexample"].clone()));
    }
    assert_eq!(documents[0], documents[1]);
    Ok(())
}
