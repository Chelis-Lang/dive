//! Structural transport for ordinary completed final reports in either profile.
//! Independent model replay remains the pipeline's authority, never these fields.

use serde_json::Value;

use crate::tlc::result::{Counts, Trace, TraceEnding};

pub(super) struct Fields {
    pub(super) counts: Option<Counts>,
    pub(super) trace: Option<Trace>,
}

pub(super) fn read(
    value: &Value,
    tlc_profile: bool,
    physical_exit: Option<i32>,
) -> Result<Fields, String> {
    let options = &value["options"];
    let profile = if tlc_profile { "tlc" } else { "dive" };
    if options["profile"] != profile
        || options["lncheck"] != "final"
        || options["lasso"] != "short"
        || options["exit_codes"] != "tlc"
        || value["exit_code"].as_i64() != physical_exit.map(i64::from)
        || physical_exit.is_none()
    {
        return Err(
            "schema 2 is not an ordinary final report of this profile with its physical status"
                .into(),
        );
    }
    let verdict = value["verdict"]
        .as_str()
        .ok_or("missing temporal verdict")?;
    let execution = &value["execution"];
    let complete = execution["capture_completed"].as_bool() == Some(true);
    let truth = matches!(verdict, "success" | "liveness-violation");
    if truth {
        let (code, name, exit) = if verdict == "success" {
            (2193, "TLC_SUCCESS", 0)
        } else {
            (2116, "TLC_TEMPORAL_PROPERTY_VIOLATED", 13)
        };
        if !complete
            || execution["stage"] != "complete"
            || execution["exhausted"] != true
            || value["tlc"]["code"].as_i64() != Some(code)
            || value["tlc"]["name"] != name
            || value["tlc"]["verified"] != true
            || physical_exit != Some(exit)
            || value["tlc"]["exit_status"].as_i64() != Some(i64::from(exit))
        {
            return Err(
                "a temporal verdict requires exhaustive final facts and its exact class/status"
                    .into(),
            );
        }
    } else if !matches!(
        verdict,
        "unsupported" | "resource-limit" | "evaluation-error" | "model-error" | "internal-error"
    ) || execution["stage"].as_str().is_none_or(str::is_empty)
    {
        return Err("unknown temporal result or missing failure stage".into());
    }
    if !truth && value["tlc"]["verified"] == true {
        let code = value["tlc"]["code"]
            .as_i64()
            .and_then(|code| i32::try_from(code).ok());
        let class = code.and_then(crate::tlc::codes::verdict_for_code);
        if code.and_then(crate::tlc::codes::ec_name) != value["tlc"]["name"].as_str()
            || !class.is_some_and(|class| {
                class.is_oracle_fact()
                    && !matches!(
                        class,
                        crate::tlc::result::Verdict::Success
                            | crate::tlc::result::Verdict::LivenessViolation
                    )
            })
        {
            return Err(
                "a temporal failure must not carry a contradictory truth classification".into(),
            );
        }
    }
    let counts = complete.then(|| counts(value)).transpose()?;
    let trace = if verdict == "liveness-violation" {
        Some(lasso(value)?)
    } else {
        if !value["counterexample"].is_null() || !value["property"].is_null() {
            return Err(
                "a result without a temporal violation must not claim its word or property".into(),
            );
        }
        None
    };
    Ok(Fields { counts, trace })
}

fn counts(value: &Value) -> Result<Counts, String> {
    let stats = &value["statistics"];
    let number = |key: &str| {
        stats[key]
            .as_u64()
            .ok_or_else(|| format!("missing/noninteger temporal statistics.{key}"))
    };
    if number("left_on_queue")? != 0 {
        return Err("exhaustive temporal capture has a pending queue".into());
    }
    // These are actual capture counters, even where TLC's print marker is false.
    Ok(Counts {
        distinct_states: Some(number("distinct")?),
        states_generated: Some(number("generated")?),
        states_left_on_queue: Some(0),
        depth: Some(number("depth")?),
    })
}

fn lasso(value: &Value) -> Result<Trace, String> {
    use dive_api::trace::{Ending, TraceDoc};
    let shown = &value["counterexample"];
    let document = TraceDoc::from_json(&shown["trace"])?;
    let Some(Ending::Lasso { back_to }) = document.ending else {
        return Err("final temporal violation requires a periodic document".into());
    };
    let length = u64::try_from(document.states.len()).map_err(|error| error.to_string())?;
    let target = u64::try_from(back_to)
        .ok()
        .and_then(|index| index.checked_add(1))
        .ok_or("periodic closing target overflow")?;
    if length == 0
        || back_to >= document.states.len()
        || shown["ending"] != "lasso"
        || shown["length"].as_u64() != Some(length)
        || shown["back_to"].as_u64() != Some(target)
    {
        return Err("periodic display contradicts its document".into());
    }
    let states = shown["states"]
        .as_array()
        .ok_or("missing displayed states")?;
    if states.len() != document.states.len()
        || states
            .iter()
            .zip(&document.states)
            .enumerate()
            .any(|(index, (display, actual))| {
                display["index"].as_u64()
                    != u64::try_from(index).ok().and_then(|n| n.checked_add(1))
                    || display["values"] != Value::Object(actual.clone())
            })
    {
        return Err("displayed states contradict the lossless document".into());
    }
    let claims = value["properties"]
        .as_array()
        .ok_or("missing temporal claims")?;
    let names: std::collections::BTreeSet<_> = claims.iter().filter_map(Value::as_str).collect();
    let inner: std::collections::BTreeSet<_> =
        document.properties.iter().map(String::as_str).collect();
    if names.is_empty()
        || names.contains("")
        || names.len() != claims.len()
        || inner.len() != document.properties.len()
        || names != inner
        || !value["property"]
            .as_str()
            .is_some_and(|name| names.contains(name))
    {
        return Err("periodic property claims are empty, duplicate or contradictory".into());
    }
    Ok(Trace {
        length,
        ending: TraceEnding::Lasso {
            back_to: Some(target),
        },
        json: Some(shown["trace"].clone()),
        steps: None,
        omitted: None,
        json_unavailable: None,
    })
}
