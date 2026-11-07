//! Ordinary compat requests reuse final checking without changing safety startup.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, TemporalMode};

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
fn ordinary_compat_final_truth_has_actual_counts_and_freshly_validated_words() -> TestResult {
    for (property, violation) in [("[]<>(x<=2)", false), ("<>(x=3)", true)] {
        let spec = Spec::new(&format!("{COUNTER}P == {property}"), CONFIG)?;
        for workers in [1, 4] {
            let request = spec.request(workers);
            let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
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
            explicit.temporal = TemporalMode::FinalShort;
            let original =
                dive_api::check_file_engine(&explicit, dive_api::Engine::Reference)?.to_json();
            assert_eq!(original["options"]["profile"], "tlc+overrides");
            assert_eq!(original["verdict"], value["verdict"]);
            assert_eq!(original["statistics"], value["statistics"]);
            let mut native = request;
            native.compat_tlc = false;
            // Below the default schedule's minimum, the default profile checks
            // once at exhaustion and reports the final truth and counts.
            let default =
                dive_api::check_file_engine(&native, dive_api::Engine::Reference)?.to_json();
            assert_eq!(default["options"]["profile"], "dive");
            assert_eq!(default["options"]["lncheck"], "deterministic");
            assert_eq!(default["periodic"]["min_states"], 65_536);
            assert_eq!(default["periodic"]["growth_factor"], 2);
            assert_eq!(default["periodic"]["prefix_checks"], 0);
            assert_eq!(default["periodic"]["final_check"], true);
            assert_eq!(default["execution"]["exhausted"], true);
            assert_eq!(default["verdict"], value["verdict"]);
            assert_eq!(default["statistics"], value["statistics"]);
            native.periodic = EVERY_LEVEL;
            let native = dive_api::check_file_engine(&native, dive_api::Engine::Reference)?;
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

fn direct(
    file: &Path,
    workers: usize,
    seed: i64,
) -> Result<dive_check::CheckResult, Box<dyn std::error::Error>> {
    let analysis = dive_sema::analyze_file(file, &dive_sema::Options::default())?;
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg"))?)
        .into_config()
        .ok_or("configuration")?;
    let mut options = dive_check::CheckOptions::tlc(0, seed);
    options.workers = workers;
    Ok(dive_check::check(&ir, &cfg, &options))
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
            let report =
                dive_api::check_file_engine(&spec.request(workers), dive_api::Engine::Reference)?;
            let actual = report.result().ok_or("ordinary safety result")?;
            let expected = direct(&spec.0, workers, 7)?;
            assert_eq!(actual.verdict, expected.verdict);
            assert_eq!(actual.stats, expected.stats);
            assert_eq!(actual.workers, expected.workers);
            assert_eq!(actual.output, expected.output);
            assert_eq!(actual.warnings, expected.warnings);
            assert_eq!(actual.notes, expected.notes);
            assert_eq!(actual.var_names, expected.var_names);
            assert_eq!(actual.tlc_var_order, expected.tlc_var_order);
            assert_eq!(actual.tlc_prints_stats, expected.tlc_prints_stats);
            assert_eq!(actual.binding_model_error, expected.binding_model_error);
            assert_eq!(
                format!("{:?}", actual.counterexample),
                format!("{:?}", expected.counterexample)
            );
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
    let mut later_draw = false;
    for seed in 0..16 {
        let mut request = spec.request(4);
        request.seed = Some(seed);
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
        let actual = report.result().ok_or("ordinary safety result")?;
        let reference = direct(&spec.0, 4, seed)?;
        assert_eq!(actual.verdict, reference.verdict);
        assert_eq!(actual.stats, reference.stats);
        assert_eq!(actual.workers, reference.workers);
        assert_eq!(actual.output, reference.output);
        assert_eq!(actual.warnings, reference.warnings);
        assert_eq!(actual.notes, reference.notes);
        assert_eq!(actual.output.len(), 1);
        later_draw |= actual.stats.ok_or("counts")?.distinct > 1;
        let value = report.to_json();
        assert_eq!(value["schema_version"], 3);
        assert!(value.get("execution").is_none());
        assert!(value["options"].get("lncheck").is_none());
    }
    assert!(
        later_draw,
        "a reset would redraw Boot at Init for every seed"
    );
    Ok(())
}

#[test]
fn ordinary_binding_failures_and_excluded_options_keep_their_existing_routes() -> TestResult {
    let failed = Spec::new(
        "CONSTANT C\nBoot == Print(\"must-not-start\",0)\nVARIABLE x\nInit == x=C\nNext == x'=x\nP == x=0",
        CONFIG,
    )?;
    let report = dive_api::check_file_engine(&failed.request(1), dive_api::Engine::Reference)?;
    let actual = report.result().ok_or("ordinary binding result")?;
    let expected = direct(&failed.0, 1, 7)?;
    assert_eq!(actual.verdict, expected.verdict);
    assert_eq!(actual.output, expected.output);
    assert_eq!(actual.binding_model_error, expected.binding_model_error);
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
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
        assert_eq!(report.verdict(), "unsupported");
        assert!(report.result().is_some());
        assert_eq!(report.to_json()["schema_version"], 3);
        assert_eq!(report.to_json()["check_kind"], "safety");
        assert!(
            report.to_json()["message"]
                .as_str()
                .is_some_and(|s| s.contains("temporal"))
        );
    }
    Ok(())
}

#[test]
fn ordinary_temporal_honors_store_polynomial_deadlock_and_sequential_requests() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == <>(x=3)"), CONFIG)?;
    let baseline = dive_api::check_file_engine(&spec.request(1), dive_api::Engine::Reference)?
        .to_json_with_exit_convention(ExitConvention::Tlc);
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
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
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
    let checked = dive_api::check_file_engine(&dead.request(1), dive_api::Engine::Reference)?;
    assert_eq!(checked.verdict(), "deadlock");
    assert_eq!(checked.exit_code_for(ExitConvention::Tlc), 11);
    let mut request = dead.request(1);
    request.no_deadlock = true;
    let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
    let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
    assert_eq!(value["verdict"], "liveness-violation");
    assert_eq!(value["property"], "P");
    assert_eq!(value["statistics"]["distinct"], 3);
    assert_eq!(value["statistics"]["generated"], 3);
    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 13);
    let mut request = spec.request(1);
    request.chaos = Some(0);
    let refused = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
    assert_eq!(refused.verdict(), "unsupported");
    assert!(
        refused.to_json()["message"]
            .as_str()
            .is_some_and(|s| s.contains("temporal checking does not yet support --chaos")),
        "{}",
        refused.to_json()["message"]
    );
    Ok(())
}

#[test]
fn ordinary_temporal_planning_errors_never_turn_into_holds() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == <>1"), CONFIG)?;
    let report = dive_api::check_file_engine(&spec.request(1), dive_api::Engine::Reference)?;
    let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
    assert_eq!(report.verdict(), "evaluation-error");
    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 255);
    assert_eq!(value["schema_version"], 3);
    assert_eq!(value["execution"]["capture_completed"], false);
    assert_eq!(value["tlc"]["code"], 2215);
    assert!(value["statistics"].is_null());
    assert!(value["counterexample"].is_null());
    Ok(())
}

#[test]
fn file_final_requests_report_compact_acceptance_defaults() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == []<>FALSE"), CONFIG)?;
    for workers in [1, 4] {
        for compat_tlc in [false, true] {
            for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
                let mut request = spec.request(workers);
                request.compat_tlc = compat_tlc;
                request.temporal = mode;
                let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
                let json = report.to_json();
                let bounds = &json["options"]["temporal_bounds"];
                assert_eq!(bounds["acceptance_buffer_bytes"], 8_000_000);
                assert_eq!(bounds["acceptance_materialization_work"], 32_000_000);
                assert_eq!(bounds["total_memory_budget"], false);
                assert!(bounds.get("acceptance_cells").is_none());
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
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
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
        request.temporal = TemporalMode::FinalShort;
        let final_report =
            dive_api::check_file_engine(&request, dive_api::Engine::Reference)?.to_json();
        assert_eq!(final_report["options"]["lncheck"], "final");
        assert_eq!(final_report["execution"]["capture_completed"], true);
        assert_eq!(final_report["statistics"]["generated"], 4);
        assert_eq!(final_report["statistics"]["distinct"], 3);
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
            let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
            let source = dive_sema::analyze_file(&spec.0, &dive_sema::Options::default())?;
            let ir = Arc::new(dive_ir::lower(&source)?);
            let cfg = dive_config::parse(CONFIG).into_config().ok_or("config")?;
            let options = dive_check::CheckOptions {
                workers,
                ..dive_check::CheckOptions::dive(7)
            };
            let expected = dive_check::check(&ir, &cfg, &options);
            let actual = report.result().ok_or("ordinary safety route")?;
            assert_eq!(actual.verdict, expected.verdict);
            assert_eq!(actual.stats, expected.stats);
            assert_eq!(actual.output, expected.output);
            assert_eq!(actual.workers, expected.workers);
            assert_eq!(report.to_json()["schema_version"], 3);
            assert_eq!(report.to_json()["check_kind"], "safety");
        }
    }
    let spec = Spec::new(
        "VARIABLE x\nBoot == Print(\"one-startup\",0)\nInit == x=Boot\nNext == x'=x+1\nSafe == x<0\nP == <>(x=3)",
        "INIT Init\nNEXT Next\nINVARIANT Safe\nPROPERTY P\n",
    )?;
    let mut request = spec.request(4);
    request.compat_tlc = false;
    let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
    let actual = report.result().ok_or("initial safety stop")?;
    assert!(matches!(
        actual.verdict,
        dive_check::Verdict::InvariantViolated { initial: true, .. }
    ));
    assert_eq!(actual.output.len(), 1);
    assert_eq!(report.to_json()["schema_version"], 3);
    assert_eq!(report.to_json()["check_kind"], "safety");
    assert!(report.temporal_result().is_none());
    Ok(())
}
