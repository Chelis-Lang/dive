//! Structural transport for ordinary final reports in either profile and dive
//! deterministic completed-level reports.
//! Independent model replay remains the pipeline's authority, never these fields.

mod periodic;

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
    budget_override: bool,
) -> Result<Fields, String> {
    let options = &value["options"];
    if !tlc_profile && options["profile"] == "dive" && options["lncheck"] == "deterministic" {
        return periodic::read(value, physical_exit);
    }
    let profile = if budget_override {
        "tlc+overrides"
    } else if tlc_profile {
        "tlc"
    } else {
        "dive"
    };
    if options["profile"] != profile
        || options["lncheck"] != "final"
        || options["lasso"] != "short"
        || options["exit_codes"] != "tlc"
        || value["exit_code"].as_i64() != physical_exit.map(i64::from)
        || physical_exit.is_none()
    {
        return Err(
            "report is not an ordinary final report of this profile with its physical status"
                .into(),
        );
    }
    if let Some(fields) = finite_error(value, physical_exit)? {
        return Ok(fields);
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
        "unsupported"
            | "resource-limit"
            | "stopped"
            | "evaluation-error"
            | "model-error"
            | "internal-error"
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

/// Strict finite endpoint transport. Validity remains fresh SpecModel replay.
fn finite_error(value: &Value, physical_exit: Option<i32>) -> Result<Option<Fields>, String> {
    if value["error_site"] != "temporal-state-row" {
        return Ok(None);
    }
    let code = value["tlc"]["code"].as_i64();
    let expected = match code {
        Some(2179) => "TLC_MODULE_DIVISION_BY_ZERO",
        Some(2182) => "TLC_MODULE_EVALUATING",
        _ => return Err("unknown finite temporal error class".into()),
    };
    let properties_empty = |v: &Value| v.is_null() || v.as_array().is_some_and(Vec::is_empty);
    if value["schema_version"] != 3
        || value["check_kind"] != "temporal"
        || value["verdict"] != "evaluation-error"
        || physical_exit != Some(255)
        || value["exit_code"] != 255
        || value["tlc"]["exit_status"] != 255
        || value["tlc"]["name"] != expected
        || value["tlc"]["verified"] != true
        || value["tlc"]["reported"].as_i64() != code
        || value["execution"]["capture_completed"] != false
        || value["execution"]["stage"] != "capture"
        || value["execution"]["exhausted"] == true
        || !value["property"].is_null()
        || !properties_empty(&value["properties"])
        || !properties_empty(&value["counterexample"]["properties"])
        || value["counterexample"]["validation"] != "valid"
        || value["counterexample"]["ending"] != "finite"
        || !value["counterexample"]["back_to"].is_null()
        || value["options"]["workers"]["requested"] != 1
    {
        return Err("contradictory finite temporal error endpoint report".into());
    }
    let document = dive_api::trace::TraceDoc::from_json(&value["counterexample"]["trace"])?;
    let Some(dive_api::trace::Ending::TemporalStateError { ec, .. }) = &document.ending else {
        return Err("finite temporal error needs its source-bound endpoint request".into());
    };
    if ec != expected
        || !document.properties.is_empty()
        || document.states.is_empty()
        || value["counterexample"]["length"].as_u64() != u64::try_from(document.states.len()).ok()
        || value["counterexample"]["states"].as_array().map(Vec::len) != Some(document.states.len())
        || document.length.is_some_and(|n| n != document.states.len())
        || value["statistics"]["reported_by_tlc"] != true
    {
        return Err("finite temporal error trace/ending mismatch".into());
    }
    let shown = value["counterexample"]["states"]
        .as_array()
        .ok_or("missing finite displayed states")?;
    if shown
        .iter()
        .zip(&document.states)
        .enumerate()
        .any(|(i, (display, actual))| {
            display["index"].as_u64() != u64::try_from(i).ok().and_then(|n| n.checked_add(1))
                || display["values"] != Value::Object(actual.clone())
        })
    {
        return Err("finite displayed states contradict endpoint document".into());
    }
    let number = |key: &str| {
        value["statistics"][key]
            .as_u64()
            .ok_or_else(|| format!("missing finite temporal statistic {key}"))
    };
    let counts = Counts {
        distinct_states: Some(number("distinct")?),
        states_generated: Some(number("generated")?),
        states_left_on_queue: Some(number("left_on_queue")?),
        depth: Some(number("depth")?),
    };
    let trace = Trace {
        length: u64::try_from(document.states.len()).map_err(|_| "finite trace length overflow")?,
        ending: TraceEnding::Finite,
        json: Some(value["counterexample"]["trace"].clone()),
        steps: None,
        omitted: None,
        json_unavailable: None,
    };
    Ok(Some(Fields {
        counts: Some(counts),
        trace: Some(trace),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn finite() -> Value {
        json!({
            "schema_version":3,"check_kind":"temporal","verdict":"evaluation-error","exit_code":255,
            "error_site":"temporal-state-row","tlc":{"code":2179,"name":"TLC_MODULE_DIVISION_BY_ZERO","exit_status":255,"verified":true,"reported":2179},
            "execution":{"stage":"capture","capture_completed":false},
            "options":{"profile":"tlc","lncheck":"final","lasso":"short","exit_codes":"tlc","workers":{"requested":1}},
            "statistics":{"distinct":2,"generated":2,"left_on_queue":1,"depth":2,"reported_by_tlc":true},
            "property":null,
            "counterexample":{"ending":"finite","validation":"valid","length":1,"states":[{"index":1,"values":{"x":0}}],
                "trace":{"format":"dive-trace","version":1,"variables":["x"],"states":[{"values":{"x":0}}],
                    "ending":{"kind":"error","ec":"TLC_MODULE_DIVISION_BY_ZERO","endpoint":{"stage":"temporal-state-row","source":{"module":"M","start":10,"end":20,"instance":null}}}}}
        })
    }
    #[test]
    fn finite_temporal_error_requires_exact_endpoint_transport_and_partial_counts()
    -> Result<(), String> {
        let original = finite();
        let fields = read(&original, true, Some(255), false)?;
        assert_eq!(fields.counts.ok_or("counts")?.states_left_on_queue, Some(1));
        assert_eq!(fields.trace.ok_or("trace")?.ending, TraceEnding::Finite);
        for pointer in [
            "/tlc/verified",
            "/counterexample/validation",
            "/statistics/reported_by_tlc",
            "/counterexample/trace/ending/endpoint",
            "/counterexample/trace/ending/ec",
        ] {
            let mut bad = original.clone();
            *bad.pointer_mut(pointer).ok_or("fixture field")? = Value::Null;
            assert!(read(&bad, true, Some(255), false).is_err(), "{pointer}");
        }
        for (pointer, value) in [
            ("/execution/capture_completed", json!(true)),
            ("/property", json!("P")),
            ("/counterexample/states/0/values/x", json!(1)),
            (
                "/counterexample/trace/ending/endpoint/stage",
                json!("final"),
            ),
        ] {
            let mut bad = original.clone();
            *bad.pointer_mut(pointer).ok_or("fixture field")? = value;
            assert!(read(&bad, true, Some(255), false).is_err(), "{pointer}");
        }
        assert!(read(&original, true, Some(0), false).is_err());
        let mut generic = original;
        generic["error_site"] = json!("temporal-action");
        assert!(read(&generic, true, Some(255), false).is_err());
        Ok(())
    }
}
