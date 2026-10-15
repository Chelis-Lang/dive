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
            let report = dive_api::check_file(&request)?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert!(report.result().is_none());
            assert!(report.temporal_result().is_some());
            assert_eq!(value["schema_version"], 2);
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
            let original = dive_api::check_file(&explicit)?.to_json();
            assert_eq!(original["options"]["profile"], "tlc+overrides");
            assert_eq!(original["verdict"], value["verdict"]);
            assert_eq!(original["statistics"], value["statistics"]);
            let mut native = request;
            native.compat_tlc = false;
            let refused = dive_api::check_file(&native)?;
            assert_eq!(refused.verdict(), "unsupported");
            assert_eq!(refused.to_json()["schema_version"], 1);
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
            let report = dive_api::check_file(&spec.request(workers))?;
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
            assert_eq!(report.to_json()["schema_version"], 1);
        }
    }
    Ok(())
}

#[test]
fn configured_safety_roles_keep_schema_one_and_the_original_print_rng_stream() -> TestResult {
    let spec = Spec::new(
        "VARIABLE x\nBoot == Print(\"one-startup\",RandomElement(1..32))\nInit == x=RandomElement(1..32)\nNext == x'=IF x<Boot THEN x+1 ELSE x\nP == [](x<=32)",
        CONFIG,
    )?;
    let mut later_draw = false;
    for seed in 0..16 {
        let mut request = spec.request(4);
        request.seed = Some(seed);
        let report = dive_api::check_file(&request)?;
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
        assert_eq!(value["schema_version"], 1);
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
    let report = dive_api::check_file(&failed.request(1))?;
    let actual = report.result().ok_or("ordinary binding result")?;
    let expected = direct(&failed.0, 1, 7)?;
    assert_eq!(actual.verdict, expected.verdict);
    assert_eq!(actual.output, expected.output);
    assert_eq!(actual.binding_model_error, expected.binding_model_error);
    assert_eq!(report.to_json()["schema_version"], 1);
    let temporal = Spec::new(&format!("{COUNTER}P == <>(x=3)"), CONFIG)?;
    let mut requests = Vec::new();
    let mut request = temporal.request(1);
    request.fp = Some(1);
    requests.push(request);
    let mut request = temporal.request(1);
    request.sequential = true;
    requests.push(request);
    let mut request = temporal.request(1);
    request.no_deadlock = true;
    requests.push(request);
    let mut request = temporal.request(1);
    request.chaos = Some(0);
    requests.push(request);
    let mut request = temporal.request(1);
    request.store = dive_api::Store::Exact;
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
        assert_eq!(report.to_json()["schema_version"], 1);
        assert!(
            report.to_json()["message"]
                .as_str()
                .is_some_and(|s| s.contains("temporal"))
        );
    }
    Ok(())
}

#[test]
fn ordinary_temporal_planning_errors_never_turn_into_holds() -> TestResult {
    let spec = Spec::new(&format!("{COUNTER}P == <>1"), CONFIG)?;
    let report = dive_api::check_file(&spec.request(1))?;
    let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
    assert_eq!(report.verdict(), "evaluation-error");
    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 255);
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["execution"]["capture_completed"], false);
    assert_eq!(value["tlc"]["code"], 2215);
    assert!(value["statistics"].is_null());
    assert!(value["counterexample"].is_null());
    Ok(())
}
