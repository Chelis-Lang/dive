//! Auxiliary initial-state diagnostics never become formal counterexamples.

use dive_api::trace::{Ending, InitialErrorStage, SpecModel, TraceDoc, Validation};
use serde_json::Value;

pub(super) fn requested(value: &Value) -> bool {
    !value["diagnostic_witness"].is_null() || value["tlc"]["code"] == 2147
}

pub(super) fn read(
    value: &Value,
    code: i32,
    exit: Option<i32>,
    source: Option<&SpecModel>,
) -> Result<(), String> {
    if !requested(value) {
        return Ok(());
    }
    if code != 2147
        || value["tlc"]["name"] != "TLC_FINGERPRINT_EXCEPTION"
        || value["tlc"]["exit_status"] != 75
        || exit != Some(75)
        || value["verdict"] != "evaluation-error"
        || value["schema_version"] != 3
        || value["check_kind"] != "safety"
        || !value["counterexample"].is_null()
        || !value["error_witness"].is_null()
    {
        return Err("initial diagnostic witness contradicts its exact report family or formal counterexample absence".into());
    }
    let source =
        source.ok_or("initial receiver classification requires a fresh source-bound reader")?;
    let (stage_report, stage) = source
        .initial_error_stage(None)
        .map_err(|error| format!("initial error routing did not complete: {error}"))?;
    if stage_report.validation != Validation::Valid {
        return Err(format!(
            "initial error lifecycle is not independently proved: {:?}",
            stage_report.validation
        ));
    }
    match stage {
        Some(InitialErrorStage::Generation | InitialErrorStage::LegacyInvariant) => {
            let site = if stage == Some(InitialErrorStage::Generation) {
                "initial-states"
            } else {
                "invariant"
            };
            return if value["tlc"]["reported"] == 1000
                && value["error_site"] == site
                && value["diagnostic_witness"].is_null()
            {
                Ok(())
            } else {
                Err("no-word initial diagnostic report differs from actual source lifecycle".into())
            };
        }
        Some(InitialErrorStage::InvariantReceiver) => {}
        None => return Err("fresh initial error routing returned no lifecycle".into()),
    }
    if value["tlc"]["reported"] != 2102 || value["error_site"] != "invariant" {
        return Err(
            "an emitted initial receiver error cannot masquerade as a no-word generation error"
                .into(),
        );
    }
    let auxiliary = value["diagnostic_witness"]
        .as_object()
        .filter(|auxiliary| auxiliary.len() == 1 && auxiliary.contains_key("trace"))
        .ok_or("initial receiver classification requires its separate diagnostic witness")?;
    let word = TraceDoc::from_json(&auxiliary["trace"])
        .map_err(|error| format!("initial diagnostic word: {error}"))?;
    if auxiliary["trace"]["version"] != 2
        || !matches!(&word.ending,Some(Ending::InvariantError { ec,.. }) if ec=="GENERAL")
    {
        return Err(
            "initial diagnostic witness requires the original named version2 GENERAL cause".into(),
        );
    }
    let (report, class) = source
        .initial_invariant_class(&word, None)
        .map_err(|error| format!("initial diagnostic replay did not complete: {error}"))?;
    match (report.validation, class) {
        (Validation::Valid, Some(class))
            if class.deciding_code() == 2147 && class.reported_code() == 2102 =>
        {
            Ok(())
        }
        (validation, _) => Err(format!(
            "initial diagnostic wrapper is not independently proved: {validation:?}"
        )),
    }
}
