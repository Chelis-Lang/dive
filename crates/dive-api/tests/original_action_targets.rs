//! Real original action targets retain typed errors without alias-state checking.
use dive_api::{CheckRequest, ExitConvention, TemporalMode};
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;
const SOURCE: &str = include_str!("../../dive-liveness/tests/fixtures/constructor-alias.tla");
struct Spec(PathBuf);
impl Spec {
    fn new(next: &str, property: &str, extra: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!(
            "dive-selected-action-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&directory)?;
        let file = directory.join("ConstructorAlias.tla");
        std::fs::write(&file, SOURCE.replace("====", &format!("{extra}\n====")))?;
        std::fs::write(
            file.with_extension("cfg"),
            format!("INIT Init\nNEXT {next}\nPROPERTY {property}\n"),
        )?;
        Ok(Self(file))
    }
    fn request(&self, compat: bool, temporal: TemporalMode) -> CheckRequest {
        CheckRequest {
            file: self.0.clone(),
            compat_tlc: compat,
            temporal,
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        }
    }
    fn failure(&self) -> Result<dive_api::temporal::Failure, Box<dyn std::error::Error>> {
        let source = std::fs::read_to_string(&self.0)?;
        let analysis = dive_sema::analyze_source(
            "ConstructorAlias.tla",
            &source,
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{analysis:?}");
        let ir = std::sync::Arc::new(dive_ir::lower(&analysis)?);
        let config = dive_config::parse(&std::fs::read_to_string(self.0.with_extension("cfg"))?)
            .into_config()
            .ok_or("configuration")?;
        dive_api::temporal::check_final_with_evidence(
            &ir,
            &config,
            &dive_check::CheckOptions::tlc(0, 0),
            dive_liveness::analyze::Limits::default(),
        )
        .err()
        .ok_or_else(|| "expected typed failure".into())
    }
}
impl Drop for Spec {
    fn drop(&mut self) {
        if let Some(directory) = self.0.parent() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}
#[test]
fn required_action_error_is_general255_while_state_and_selected_function_succeed() -> TestResult {
    for (next, property, status, generated) in [
        ("Next", "ActionCheck", 255, 2),
        ("NextRecordFirst", "ActionCheck", 255, 3),
        ("NextFunctionFirst", "ActionCheck", 0, 3),
        ("Next", "StateControl", 0, 2),
        ("Next", "EliminatedAction", 0, 2),
    ] {
        let spec = Spec::new(next, property, "")?;
        for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
            let report = dive_api::check_file_engine(
                &spec.request(true, mode),
                dive_api::Engine::Reference,
            )?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(
                report.exit_code_for(ExitConvention::Tlc),
                status,
                "{next}/{property}: {value}"
            );
            assert_eq!(value["statistics"]["generated"], generated);
            assert_eq!(value["statistics"]["distinct"], 1);
            assert_eq!(value["statistics"]["depth"], 1);
            assert_eq!(value["execution"]["capture_completed"], true);
            assert!(value["counterexample"].is_null());
            if status == 255 {
                assert_eq!(value["verdict"], "evaluation-error");
                assert_eq!(value["tlc"]["code"], 1000);
                assert_eq!(value["tlc"]["exit_status"], 255);
                assert_eq!(value["tlc"]["verified"], true);
                assert_eq!(value["error_site"], "temporal-action");
                assert_eq!(value["execution"]["stage"], "atoms");
                assert_eq!(
                    report.exit_code_for(ExitConvention::Native),
                    dive_api::exit::EVALUATION
                );
            } else {
                assert_eq!(value["verdict"], "success");
            }
        }
        let default = dive_api::check_file_engine(
            &spec.request(false, TemporalMode::FinalShort),
            dive_api::Engine::Reference,
        )?;
        assert_eq!(default.exit_code_for(ExitConvention::Tlc), 0);
    }
    Ok(())
}
#[test]
fn residual_state_error_remains_validation_refusal_without_a_false_word() -> TestResult {
    let spec = Spec::new("Next", "EventuallyEqual", "")?;
    let report = dive_api::check_file_engine(
        &spec.request(true, TemporalMode::FinalShort),
        dive_api::Engine::Reference,
    )?;
    let value = report.to_json();
    assert_eq!(report.verdict(), "unsupported");
    assert_eq!(value["execution"]["stage"], "validation");
    assert_eq!(value["diagnostics"][0]["code"], "DIVE-A0032");
    assert!(value["tlc"].is_null());
    assert!(value["counterexample"].is_null());
    Ok(())
}
#[test]
fn state_nonboolean_and_uncertain_multi_property_errors_are_not_verified_tlc_errors() -> TestResult
{
    for (property, extra) in [
        ("NonBoolean", "NonBoolean == <>[]x"),
        ("ActionCheck Other", "Other == <>[](FALSE)"),
    ] {
        let spec = Spec::new("Next", property, extra)?;
        let report = dive_api::check_file_engine(
            &spec.request(true, TemporalMode::FinalShort),
            dive_api::Engine::Reference,
        )?;
        let value = report.to_json();
        assert_eq!(report.verdict(), "unsupported", "{value}");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        assert!(value["tlc"].is_null());
        assert!(value["counterexample"].is_null());
        assert_eq!(value["execution"]["stage"], "atoms");
        let failure = spec.failure()?;
        assert_eq!(failure.stage().name(), "atoms");
        if property == "NonBoolean" {
            assert!(failure.action_evaluation().is_none());
            assert!(matches!(
                failure.error(),
                dive_api::temporal::Error::Analysis(
                    dive_liveness::analyze::AnalysisError::Atom { .. }
                )
            ));
        } else {
            let action = failure.action_evaluation().ok_or("actual action error")?;
            assert_eq!(action.cause().class(), dive_eval::ErrorClass::Eval);
            assert_eq!(action.cause().ec(), dive_eval::Ec::General);
            assert!(action.core_cause().is_some());
            assert_eq!(action.properties(), 2);
            assert!(action.direct_operand());
        }
    }
    Ok(())
}

#[test]
fn eliminated_false_action_candidate_still_refuses_attribution_without_a_word() -> TestResult {
    // Existing native-only review probe, not a new primary parity claim.
    let spec = Spec::new(
        "Next",
        "Pruned",
        "Pruned == <>(FALSE /\\ <>[][~(x' = <<0, 0>>)]_x)",
    )?;
    let source = std::fs::read_to_string(&spec.0)?.replace(
        "Init == x = [k \\in {\"a\"} |-> 0]",
        "Init == x = [a |-> 0]",
    );
    std::fs::write(&spec.0, source)?;
    for store in [dive_api::Store::Fingerprint, dive_api::Store::Exact] {
        let mut request = spec.request(true, TemporalMode::FinalShort);
        request.store = store;
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
        let value = report.to_json();
        assert_eq!(report.verdict(), "unsupported", "{value}");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        assert_eq!(value["execution"]["stage"], "attribution");
        assert_eq!(value["execution"]["capture_completed"], true);
        assert_eq!(value["statistics"]["generated"], 2);
        assert_eq!(value["statistics"]["distinct"], 1);
        assert_eq!(value["statistics"]["depth"], 1);
        assert_eq!(value["diagnostics"][0]["code"], "DIVE-L0033");
        assert!(value["tlc"].is_null());
        assert!(value["counterexample"].is_null());
    }
    // Failure evidence must not misattribute candidate attribution to labeling.
    let failure = spec.failure()?;
    assert_eq!(failure.stage().name(), "attribution");
    assert!(failure.action_evaluation().is_none());
    Ok(())
}
