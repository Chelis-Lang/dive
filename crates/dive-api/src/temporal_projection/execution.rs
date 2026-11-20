//! Qualify the establishing ordinary result at the serialized public boundary.
use super::{Limits, Miss, miss};
use dive_liveness::analyze::periodic::Schedule;
use serde_json::{Value, json};

pub(super) fn qualify(report: &Value, limits: Limits) -> Result<usize, Miss> {
    let atom_count = report
        .pointer("/core/metrics/liveness/atoms/total_atoms")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|&n| n > 0)
        .ok_or_else(|| miss("missing positive integer previous atom count"))?;
    let schedule = Schedule::DEFAULT;
    for (field, expected) in [
        ("/schema_version", json!(3)),
        ("/command", json!("check")),
        ("/check_kind", json!("temporal")),
        ("/engine", json!("core")),
        ("/verdict", json!("success")),
        ("/execution/capture_completed", json!(true)),
        ("/execution/exhausted", json!(true)),
        ("/execution/stage", json!("complete")),
        ("/statistics/left_on_queue", json!(0)),
        ("/periodic/exhausted", json!(true)),
        ("/periodic/final_check", json!(true)),
        ("/periodic/prefix_checks", json!(0)),
        ("/periodic/last_level", Value::Null),
        ("/periodic/last_size", json!(0)),
        ("/periodic/min_states", json!(schedule.min_states)),
        ("/periodic/growth_factor", json!(schedule.growth_factor)),
        ("/diagnostics", json!([])),
        ("/warnings", json!([])),
        ("/output", json!([])),
        ("/options/budget/max_states", json!(limits.max_states)),
        ("/options/budget/max_depth", json!(limits.max_depth)),
        ("/options/budget/max_memory_bytes", Value::Null),
        ("/options/budget/time_limit_ms", Value::Null),
        ("/options/int", json!("big")),
        ("/options/comparisons", json!("tla")),
        ("/options/explore_order", json!("value")),
        ("/options/choose_order", json!("canonical")),
        ("/options/action_composition", json!("tla")),
        ("/options/lasso", json!("short")),
        ("/options/tlc_exact_temporal", json!(false)),
        ("/options/fingerprint", Value::Null),
        ("/options/symmetry_liveness", json!("unreduced")),
        ("/symmetry_liveness/declared", Value::Null),
        ("/symmetry_liveness/effective", json!("not-applicable")),
        ("/symmetry_liveness/genuine_temporal", json!(true)),
        ("/options/workers/sequential", json!(false)),
        ("/options/candidate_memory_bytes_per_worker", Value::Null),
        ("/options/candidate_record_bytes_limit", Value::Null),
        ("/options/frontier_codec_nodes_per_value_limit", Value::Null),
        ("/options/frontier_memory_bytes_per_queue", Value::Null),
        ("/options/frontier_record_bytes_limit", Value::Null),
        ("/options/predecessor_memory_bytes", Value::Null),
        ("/options/predecessor_path_steps_limit", Value::Null),
        ("/options/store_compact_trigger_bytes", Value::Null),
        ("/options/workers/requested", json!(1)),
        ("/options/workers/used", json!(1)),
        ("/options/workers/chaos", Value::Null),
        ("/options/seed", json!(0)),
        ("/options/store", json!("exact")),
        ("/options/profile", json!("dive")),
        ("/options/deadlock_flag", json!(false)),
        ("/options/allow_io", json!([])),
        ("/options/warnings", json!(true)),
        ("/options/lncheck", json!("deterministic")),
        (
            "/options/temporal_bounds",
            crate::check::temporal_limits_json(crate::temporal::core_file_configuration()),
        ),
        ("/core/disabled", json!("default")),
        ("/core/metrics/liveness/storage", json!("native-csr")),
        (
            "/core/metrics/liveness/atoms/total_atoms",
            json!(atom_count),
        ),
        (
            "/core/metrics/liveness/atoms/eligible_atoms",
            json!(atom_count),
        ),
        ("/core/metrics/liveness/atoms/audit", json!(false)),
        ("/core/metrics/liveness/atoms/fallback", json!(0)),
        (
            "/core/metrics/liveness/atoms/reference_evaluations/state",
            json!(0),
        ),
        (
            "/core/metrics/liveness/atoms/reference_evaluations/action",
            json!(0),
        ),
    ] {
        if report.pointer(field) != Some(&expected) {
            return Err(miss(format!(
                "unqualified previous native execution at {field}"
            )));
        }
    }
    counts(report, limits)?;
    Ok(atom_count)
}

fn counts(report: &Value, limits: Limits) -> Result<(), Miss> {
    let count = |name: &str| {
        report["statistics"]
            .get(name)
            .and_then(Value::as_u64)
            .ok_or_else(|| miss(format!("missing previous {name} count")))
    };
    let distinct = count("distinct")?;
    if distinct == 0
        || distinct > limits.max_states.unwrap_or(0)
        || count("generated")? < distinct
        || count("initial_generated")? == 0
        || limits.max_depth.is_some_and(|max| {
            report["statistics"]["depth"]
                .as_u64()
                .is_none_or(|depth| depth > max)
        })
    {
        return Err(miss(
            "previous search counters do not establish bounded completion",
        ));
    }
    count("depth")?;
    Ok(())
}
