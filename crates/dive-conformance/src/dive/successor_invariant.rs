//! Fresh source proof and canonical report admission for owned error families.
use crate::tlc::result::OracleResult;
use dive_api::ExitConvention;
use dive_api::trace::{Ending, SpecModel, TraceDoc};
use serde_json::Value;

pub(super) fn requested(value: &Value) -> bool {
    value["tlc"]["code"] == 2111
        || value["tlc"]["reported"] == 2111
        || value["counterexample"]["trace"]["ending"]["kind"] == "invariant-error"
        || value["counterexample"]["trace"]["ending"]["endpoint"]["stage"] == "view-source-terminal"
}

pub(super) fn read(
    value: &Value,
    code: i32,
    physical_exit: Option<i32>,
    convention: dive_api::ExitConvention,
    source: Option<&SpecModel>,
) -> Result<(), String> {
    if !requested(value) {
        return Ok(());
    }
    if value["counterexample"]["trace"]["ending"]["endpoint"]["stage"] == "view-source-terminal" {
        return read_view(value, code, physical_exit, convention, source);
    }
    if !crate::dive::supported_report_version(value)
        || value["check_kind"] != "safety"
        || value["verdict"].as_str() != Some(native_kind(code))
        || value["error_site"] != "invariant"
        || value["tlc"]["reported"] != 2111
        || value["tlc"]["name"].as_str() != crate::tlc::codes::ec_name(code)
        || !value["tlc"]["exit_status"]
            .as_i64()
            .and_then(|exit| i32::try_from(exit).ok())
            .is_some_and(|exit| crate::tlc::codes::exit_statuses_for(code).contains(&exit))
        || !value["error_witness"].is_null()
        || !value["diagnostic_witness"].is_null()
        || (!value["options"]["exit_codes"].is_null()
            && value["options"]["exit_codes"].as_str() != Some(convention.as_str()))
        || (!value["exit_code"].is_null()
            && value["exit_code"].as_i64() != physical_exit.map(i64::from))
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
        && class.as_ref().is_some_and(|class| {
            class.reported_code() == 2111 && i64::from(class.deciding_code()) == i64::from(code)
        })
    {
        canonical_report(value, 2111, code, physical_exit, convention)
    } else {
        Err(format!(
            "successor invariant diagnostic wrapper is not independently proved: {:?}",
            report.validation
        ))
    }
}

fn read_view(
    value: &Value,
    code: i32,
    physical_exit: Option<i32>,
    convention: ExitConvention,
    source: Option<&SpecModel>,
) -> Result<(), String> {
    if !crate::dive::supported_report_version(value)
        || value["check_kind"] != "safety"
        || value["verdict"].as_str() != Some(native_kind(code))
        || value["error_site"] != "view"
        || !value["diagnostic_witness"].is_null()
        || !value["error_witness"].is_null()
    {
        return Err("VIEW report contradicts its source-final error family".into());
    }
    let source =
        source.ok_or("VIEW classification requires fresh source-bound diagnostic replay")?;
    let word = TraceDoc::from_json(&value["counterexample"]["trace"])?;
    let Some(Ending::ViewSourceError { endpoint, .. }) = &word.ending else {
        return Err("missing VIEW source word".into());
    };
    if i64::from(endpoint.deciding_code()) != i64::from(code)
        || !source
            .try_validate(&word)
            .map_err(|error| error.to_string())?
            .validation
            .is_valid()
    {
        return Err("VIEW diagnostic wrapper is not independently proved".into());
    }
    canonical_report(
        value,
        endpoint.reported_code(),
        code,
        physical_exit,
        convention,
    )
}

fn native_kind(code: i32) -> &'static str {
    match code {
        2132 => "assertion-failure",
        2178 => "overflow",
        _ => "evaluation-error",
    }
}

fn physical_status(code: i32, convention: ExitConvention) -> Option<i32> {
    match convention {
        ExitConvention::Native => Some(i32::from(dive_api::exit::EVALUATION)),
        ExitConvention::Tlc => crate::tlc::codes::exit_statuses_for(code).first().copied(),
        _ => None,
    }
}

fn canonical_report(
    value: &Value,
    reported: u32,
    deciding: i32,
    physical_exit: Option<i32>,
    convention: ExitConvention,
) -> Result<(), String> {
    let canonical = crate::tlc::codes::exit_statuses_for(deciding)
        .first()
        .copied();
    if value["verdict"].as_str() != Some(native_kind(deciding))
        || value["tlc"]["code"].as_i64() != Some(i64::from(deciding))
        || value["tlc"]["reported"].as_u64() != Some(u64::from(reported))
        || value["tlc"]["name"].as_str() != crate::tlc::codes::ec_name(deciding)
        || value["tlc"]["exit_status"].as_i64() != canonical.map(i64::from)
        || physical_exit != physical_status(deciding, convention)
        || (!value["options"]["exit_codes"].is_null()
            && value["options"]["exit_codes"].as_str() != Some(convention.as_str()))
        || (!value["exit_code"].is_null()
            && value["exit_code"].as_i64() != physical_exit.map(i64::from))
    {
        return Err("error report class/name/status contradicts independently proved identity or caller-selected exit convention".into());
    }
    Ok(())
}

pub(crate) fn canonical_result(
    result: &OracleResult,
    deciding: u32,
    convention: ExitConvention,
) -> bool {
    let Ok(code) = i32::try_from(deciding) else {
        return false;
    };
    let canonical = crate::tlc::codes::exit_statuses_for(code).first().copied();
    result.verdict_code == Some(code)
        && crate::tlc::codes::verdict_for_code(code).as_ref() == Some(&result.verdict)
        && result.error_class.as_deref() == crate::tlc::codes::ec_name(code)
        && result.exit_status.as_deref() == canonical.and_then(crate::tlc::codes::exit_status_name)
        && result.exit_code == physical_status(code, convention)
}
