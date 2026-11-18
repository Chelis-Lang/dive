//! Real original action targets retain typed errors without alias-state
//! checking. The typed stage and action/state causes of these failures were
//! exposed only by the retired reference final route (#791); Core refuses
//! the failure report by name.
#[path = "support/refused.rs"]
mod refused;
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
}
/// Core's report for `request` in ordinary temporal mode, after asserting
/// that Core refuses the requested bounded final mode, which only the
/// retired reference engine ran (#791, #910).
fn core_ordinary(
    mut request: CheckRequest,
) -> Result<dive_api::CheckReport, Box<dyn std::error::Error>> {
    if request.temporal == TemporalMode::FinalShort {
        let final_short = dive_api::check_file(&request)?;
        refused::assert_refused(
            &final_short.to_json(),
            final_short.exit_code(),
            refused::FINAL_SHORT,
        );
        request.temporal = TemporalMode::Ordinary;
    }
    Ok(dive_api::check_file(&request)?)
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
            let report = core_ordinary(spec.request(true, mode))?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            if status == 255 {
                // TLC reports the selected action's error (1000, exit 255).
                // Only the retired reference engine built that temporal
                // failure report (#791, #910).
                refused::assert_refused(
                    &report.to_json(),
                    report.exit_code(),
                    refused::TEMPORAL_FAILURE_REPORT,
                );
                continue;
            }
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), status, "{value}");
            assert_eq!(value["statistics"]["generated"], generated);
            assert_eq!(value["statistics"]["distinct"], 1);
            assert_eq!(value["statistics"]["depth"], 1);
            assert_eq!(value["execution"]["capture_completed"], true);
            assert!(value["counterexample"].is_null());
            assert_eq!(value["verdict"], "success");
        }
        let default = core_ordinary(spec.request(false, TemporalMode::FinalShort))?;
        assert_eq!(default.exit_code_for(ExitConvention::Tlc), 0);
    }
    Ok(())
}
#[test]
fn residual_state_error_remains_validation_refusal_without_a_false_word() -> TestResult {
    let spec = Spec::new("Next", "EventuallyEqual", "")?;
    let report = core_ordinary(spec.request(true, TemporalMode::FinalShort))?;
    let value = report.to_json();
    refused::assert_refused(
        &value,
        report.exit_code(),
        refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
    );
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
        let report = core_ordinary(spec.request(true, TemporalMode::FinalShort))?;
        let value = report.to_json();
        refused::assert_refused(&value, report.exit_code(), refused::TEMPORAL_FAILURE_REPORT);
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        assert!(value["tlc"].is_null());
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
        let report = core_ordinary(request)?;
        let value = report.to_json();
        assert_eq!(report.verdict(), "unsupported", "{value}");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
        assert!(value["tlc"].is_null());
        assert!(value["counterexample"].is_null());
        if store == dive_api::Store::Exact {
            refused::assert_refused(&value, report.exit_code(), refused::TEMPORAL_EXACT_STORE);
        } else {
            refused::assert_refused(&value, report.exit_code(), refused::TEMPORAL_FAILURE_REPORT);
        }
    }
    Ok(())
}
