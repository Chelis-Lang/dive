//! The optional report context for explicit external input assumptions.

pub use dive_eval::inputs::{HostAccess, InputRecord, InputRequest, InputResponse, RecordedInputs};
use serde_json::{Value, json};

/// Encodes external assumptions in the existing report envelope. No trace
/// version or past-host authenticity claim is introduced.
#[must_use]
pub(crate) fn inputs_json(inputs: &RecordedInputs) -> Value {
    json!({
        "scope": "source-binding",
        "assumption": "recorded external values; past host truth is not independently proven",
        "grants": inputs.grants.names(),
        "limits": { "records": dive_eval::inputs::MAX_INPUT_RECORDS, "utf8_bytes": dive_eval::inputs::MAX_INPUT_BYTES, "environment_entries": dive_eval::inputs::MAX_ENV_ENTRIES },
        "records": inputs.records.iter().map(|record| match (&record.request, &record.response) {
            (InputRequest::Environment, InputResponse::Environment(entries)) => json!({ "operation":"IOUtils!IOEnv", "environment":entries }),
            (InputRequest::TextFile(path), InputResponse::Text(text)) => json!({ "operation":"IOUtils!Deserialize", "path":path, "text":text }),
            _ => json!({ "operation":"invalid-input-kind" }),
        }).collect::<Vec<_>>()
    })
}

/// Reads explicit external assumptions from a schema-3 check report, without
/// host access. Supply the result to [`crate::trace::TraceOptions::inputs`].
/// The source binder must independently demand every record in order.
///
/// # Errors
/// An unsupported report envelope, malformed records or input bounds.
pub fn inputs_from_report(report: &Value) -> Result<RecordedInputs, crate::ApiError> {
    let invalid = || crate::ApiError::Rejected {
        message: "DIVE-A0110: malformed external input report context".into(),
    };
    if report["schema_version"].as_u64() != Some(u64::from(crate::REPORT_SCHEMA_VERSION))
        || report["command"].as_str() != Some("check")
    {
        return Err(invalid());
    }
    let context = &report["external_inputs"];
    if context["scope"].as_str() != Some("source-binding") {
        return Err(invalid());
    }
    let categories = context["grants"].as_array().ok_or_else(invalid)?;
    if categories.len() > 3 {
        return Err(invalid());
    }
    let names = categories
        .iter()
        .map(|name| name.as_str().ok_or_else(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    let grants = if names.is_empty() {
        HostAccess::NONE
    } else {
        HostAccess::parse(&names.join(",")).map_err(|_| invalid())?
    };
    let records = context["records"].as_array().ok_or_else(invalid)?;
    if records.len() > dive_eval::inputs::MAX_INPUT_RECORDS {
        return Err(invalid());
    }
    let mut bytes = 0usize;
    for record in records {
        let additional = match record["operation"].as_str() {
            Some("IOUtils!IOEnv") => {
                let entries = record["environment"].as_object().ok_or_else(invalid)?;
                if entries.len() > dive_eval::inputs::MAX_ENV_ENTRIES {
                    return Err(invalid());
                }
                entries.iter().try_fold(0usize, |n, (name, value)| {
                    Ok::<_, crate::ApiError>(
                        n.saturating_add(name.len())
                            .saturating_add(value.as_str().ok_or_else(invalid)?.len()),
                    )
                })?
            }
            Some("IOUtils!Deserialize") => record["path"]
                .as_str()
                .ok_or_else(invalid)?
                .len()
                .saturating_add(record["text"].as_str().ok_or_else(invalid)?.len()),
            _ => return Err(invalid()),
        };
        bytes = bytes.saturating_add(additional);
        if bytes > dive_eval::inputs::MAX_INPUT_BYTES {
            return Err(invalid());
        }
    }
    let mut inputs = RecordedInputs {
        grants,
        records: Vec::with_capacity(records.len()),
    };
    for record in records {
        let (request, response) = match record["operation"].as_str() {
            Some("IOUtils!IOEnv") => {
                let entries = record["environment"].as_object().ok_or_else(invalid)?;
                let entries = entries
                    .iter()
                    .map(|(key, value)| {
                        Ok((key.clone(), value.as_str().ok_or_else(invalid)?.to_owned()))
                    })
                    .collect::<Result<_, crate::ApiError>>()?;
                (
                    InputRequest::Environment,
                    InputResponse::Environment(entries),
                )
            }
            Some("IOUtils!Deserialize") => (
                InputRequest::TextFile(record["path"].as_str().ok_or_else(invalid)?.to_owned()),
                InputResponse::Text(record["text"].as_str().ok_or_else(invalid)?.to_owned()),
            ),
            _ => return Err(invalid()),
        };
        inputs.records.push(InputRecord { request, response });
    }
    inputs
        .validate()
        .map_err(|error| crate::ApiError::Rejected {
            message: format!("DIVE-A0110: {}", error.message),
        })?;
    Ok(inputs)
}
