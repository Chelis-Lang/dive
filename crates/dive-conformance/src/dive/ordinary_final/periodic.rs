//! Read actual deterministic schedule facts without granting word validity.

use serde_json::Value;

use super::{Fields, counts, lasso};
use crate::tlc::{codes, result::Counts};

pub(super) fn read(value: &Value, physical_exit: Option<i32>) -> Result<Fields, String> {
    let options = &value["options"];
    if options["lasso"] != "short"
        || options["exit_codes"] != "tlc"
        || physical_exit.is_none()
        || value["exit_code"].as_i64() != physical_exit.map(i64::from)
    {
        return Err("deterministic report contradicts its physical status or options".into());
    }
    let verdict = value["verdict"]
        .as_str()
        .ok_or("missing temporal verdict")?;
    let truth = matches!(verdict, "success" | "liveness-violation");
    let complete = value["execution"]["capture_completed"].as_bool() == Some(true);
    if truth || !value["periodic"].is_null() {
        progress(value, complete)?;
    } else if !value["statistics"].is_null()
        || value["execution"]["capture_completed"] != false
        || value["execution"]["stage"] != "planning"
    {
        return Err("missing deterministic progress after planning".into());
    }
    let counts = if complete {
        Some(counts(value)?)
    } else if value["statistics"].is_null() {
        None
    } else {
        Some(prefix_counts(value)?)
    };
    if truth {
        let (code, name, exit) = if verdict == "success" {
            (2193, "TLC_SUCCESS", 0)
        } else {
            (2116, "TLC_TEMPORAL_PROPERTY_VIOLATED", 13)
        };
        if value["tlc"]["code"].as_i64() != Some(code)
            || value["tlc"]["name"] != name
            || value["tlc"]["verified"] != true
            || value["tlc"]["exit_status"].as_i64() != Some(i64::from(exit))
            || physical_exit != Some(exit)
        {
            return Err("deterministic verdict contradicts its exact class/status".into());
        }
        if complete {
            if value["execution"]["stage"] != "complete" || value["execution"]["exhausted"] != true
            {
                return Err("a final verdict requires actual exhaustive capture".into());
            }
        } else if verdict == "success" {
            return Err("an incomplete graph cannot establish temporal success".into());
        } else {
            prefix(value, counts.as_ref().ok_or("missing prefix counts")?)?;
        }
    } else {
        failure(value, physical_exit)?;
    }
    let trace = if verdict == "liveness-violation" {
        Some(lasso(value)?)
    } else {
        if !value["counterexample"].is_null() || !value["property"].is_null() {
            return Err("a result without a violation must not claim its word or property".into());
        }
        None
    };
    Ok(Fields { counts, trace })
}

fn progress(value: &Value, complete: bool) -> Result<(), String> {
    let checks = &value["periodic"];
    let prefixes = checks["prefix_checks"]
        .as_u64()
        .ok_or("missing prefix check count")?;
    let size = checks["last_size"]
        .as_u64()
        .ok_or("missing last checked size")?;
    let exhausted = checks["exhausted"]
        .as_bool()
        .ok_or("missing exhaustion fact")?;
    let final_check = checks["final_check"]
        .as_bool()
        .ok_or("missing final check fact")?;
    let last_level = checks
        .get("last_level")
        .ok_or("missing last checked level")?;
    let min_states = checks["min_states"]
        .as_u64()
        .ok_or("missing schedule minimum")?;
    checks["growth_factor"]
        .as_u64()
        .ok_or("missing schedule growth factor")?;
    if exhausted != complete
        || final_check != exhausted
        || (prefixes == 0 && (!last_level.is_null() || size != 0))
        || (prefixes != 0
            && (last_level
                .as_u64()
                .is_none_or(|level| level.saturating_add(1) < prefixes)
                || size < min_states.max(1)))
    {
        return Err("contradictory deterministic check progress".into());
    }
    Ok(())
}

fn prefix_counts(value: &Value) -> Result<Counts, String> {
    let stats = &value["statistics"];
    let number = |key: &str| {
        stats[key]
            .as_u64()
            .ok_or_else(|| format!("missing/noninteger temporal statistics.{key}"))
    };
    Ok(Counts {
        distinct_states: Some(number("distinct")?),
        states_generated: Some(number("generated")?),
        states_left_on_queue: Some(number("left_on_queue")?),
        depth: Some(number("depth")?),
    })
}

fn prefix(value: &Value, counts: &Counts) -> Result<(), String> {
    let execution = &value["execution"];
    let checks = &value["periodic"];
    let level = execution["level"]
        .as_u64()
        .ok_or("missing prefix boundary")?;
    if execution["capture_completed"] != false
        || execution["stage"] != "prefix"
        || execution["exhausted"] != false
        || checks["prefix_checks"].as_u64().is_none_or(|n| n == 0)
        || checks["last_level"].as_u64() != Some(level)
        || checks["last_size"].as_u64() != counts.distinct_states
        || counts.depth != level.checked_add(1)
        || counts.states_left_on_queue.is_none_or(|n| n == 0)
    {
        return Err("a prefix violation requires its actual nonexhausted boundary".into());
    }
    Ok(())
}

fn failure(value: &Value, physical_exit: Option<i32>) -> Result<(), String> {
    if !matches!(
        value["verdict"].as_str(),
        Some(
            "unsupported"
                | "resource-limit"
                | "evaluation-error"
                | "model-error"
                | "internal-error"
        )
    ) || value["execution"]["stage"]
        .as_str()
        .is_none_or(|stage| stage.is_empty() || matches!(stage, "prefix" | "complete"))
    {
        return Err("unknown deterministic failure or missing failure stage".into());
    }
    if value["tlc"]["verified"] == true {
        let code = value["tlc"]["code"]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or("missing failure code")?;
        let class = codes::verdict_for_code(code);
        if codes::ec_name(code) != value["tlc"]["name"].as_str()
            || value["tlc"]["exit_status"].as_i64() != physical_exit.map(i64::from)
            || physical_exit.is_none_or(|exit| !codes::exit_statuses_for(code).contains(&exit))
            || !class.is_some_and(|class| {
                class.is_oracle_fact()
                    && !matches!(
                        class,
                        crate::tlc::result::Verdict::Success
                            | crate::tlc::result::Verdict::LivenessViolation
                    )
            })
        {
            return Err("deterministic failure contradicts its exact class/status".into());
        }
    }
    Ok(())
}
