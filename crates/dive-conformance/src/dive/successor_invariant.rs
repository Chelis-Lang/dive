//! A named word alone cannot admit an invariant diagnostic wrapper.
use dive_api::trace::{Ending, SpecModel, TraceDoc};
use serde_json::Value;

pub(super) fn requested(value: &Value) -> bool {
    value["tlc"]["code"] != 2111
        && value["counterexample"]["trace"]["ending"]["kind"] == "invariant-error"
        && value["diagnostic_witness"].is_null()
}

pub(super) fn read(value: &Value, code: i32, source: Option<&SpecModel>) -> Result<(), String> {
    if !requested(value) {
        return Ok(());
    }
    if value["schema_version"] != 3
        || value["check_kind"] != "safety"
        || value["verdict"] != "evaluation-error"
        || value["error_site"] != "invariant"
        || value["tlc"]["reported"] != 2111
        || value["tlc"]["name"].as_str() != crate::tlc::codes::ec_name(code)
        || !value["tlc"]["exit_status"]
            .as_i64()
            .and_then(|exit| i32::try_from(exit).ok())
            .is_some_and(|exit| crate::tlc::codes::exit_statuses_for(code).contains(&exit))
        || !value["error_witness"].is_null()
    {
        return Err(
            "successor invariant diagnostic report contradicts its exact primary family".into(),
        );
    }
    let source = source.ok_or(
        "successor invariant classification requires fresh source-bound diagnostic replay",
    )?;
    let word = TraceDoc::from_json(&value["counterexample"]["trace"])?;
    if !matches!(word.ending, Some(Ending::InvariantError { .. })) {
        return Err("missing original named invariant word".into());
    }
    let (report, class) = source
        .invariant_error_class(&word, None)
        .map_err(|error| error.to_string())?;
    if report.validation.is_valid()
        && class.is_some_and(|class| {
            class.reported_code() == 2111 && i64::from(class.deciding_code()) == i64::from(code)
        })
    {
        Ok(())
    } else {
        Err(format!(
            "successor invariant diagnostic wrapper is not independently proved: {:?}",
            report.validation
        ))
    }
}
