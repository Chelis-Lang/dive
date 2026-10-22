//! Schema-three configured owner settings. These never certify execution.

use serde_json::Value;

pub(super) fn read(value: &Value) -> Result<(), String> {
    if !value.is_object() || value.get("atom_cells").is_some() {
        return Err("schema 3 requires explicit temporal cell owners, not atom_cells".into());
    }
    let cells = &value["cell_limits"];
    let graph = &cells["graph_labels"];
    match graph["kind"].as_str() {
        Some("cells") if graph["cells"].as_u64().is_some() => {}
        Some("run-budgets") if graph.get("cells").is_none() => {}
        _ => return Err("unknown or contradictory graph-label policy".into()),
    }
    for owner in ["attribution_rows", "word_truth"] {
        if cells[owner]["kind"] != "cells" || cells[owner]["cells"].as_u64().is_none() {
            return Err(format!("missing or invalid temporal {owner} cell ceiling"));
        }
    }
    let acceptance = &value["acceptance_policy"];
    match acceptance["kind"].as_str() {
        Some("cells")
            if acceptance["cells"].as_u64().is_some()
                && acceptance.get("bytes").is_none()
                && acceptance.get("materialization_work").is_none()
                && value.get("acceptance_buffer_bytes") == Some(&Value::Null)
                && value.get("acceptance_materialization_work") == Some(&Value::Null) => {}
        Some("buffers")
            if acceptance.get("cells").is_none()
                && acceptance["bytes"].as_u64().is_some()
                && acceptance["materialization_work"].as_u64().is_some()
                && acceptance["bytes"] == value["acceptance_buffer_bytes"]
                && acceptance["materialization_work"]
                    == value["acceptance_materialization_work"] => {}
        _ => return Err("unknown or contradictory acceptance policy".into()),
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "finite wire-field assertions")]
mod tests {
    use super::*;
    use serde_json::json;

    fn configured() -> Value {
        json!({"cell_limits": {
            "graph_labels":{"kind":"run-budgets"},
            "attribution_rows":{"kind":"cells","cells":31},
            "word_truth":{"kind":"cells","cells":31}},
            "acceptance_policy":{"kind":"buffers","bytes":73,"materialization_work":79},
            "acceptance_buffer_bytes":73,"acceptance_materialization_work":79})
    }

    #[test]
    fn owner_settings_are_explicit_and_allow_real_custom_zero_values() {
        let mut value = configured();
        assert!(read(&value).is_ok());
        value["cell_limits"]["graph_labels"] = json!({"kind":"cells","cells":0});
        assert!(read(&value).is_ok());
        value["acceptance_policy"] = json!({"kind":"cells","cells":0});
        value["acceptance_buffer_bytes"] = Value::Null;
        value["acceptance_materialization_work"] = Value::Null;
        assert!(read(&value).is_ok());
    }

    #[test]
    fn missing_unknown_and_contradictory_settings_fail_closed() {
        for (pointer, replacement) in [
            ("/cell_limits", Value::Null),
            ("/cell_limits/graph_labels", json!({"kind":"guessed"})),
            ("/cell_limits/graph_labels", json!({"kind":"cells"})),
            (
                "/cell_limits/graph_labels",
                json!({"kind":"cells","cells":-1}),
            ),
            (
                "/cell_limits/graph_labels",
                json!({"kind":"run-budgets","cells":8_000_000}),
            ),
            (
                "/cell_limits/attribution_rows",
                json!({"kind":"run-budgets"}),
            ),
            ("/cell_limits/word_truth/cells", json!("8000000")),
            ("/acceptance_policy/kind", json!("guess")),
            ("/acceptance_policy/bytes", json!(74)),
            ("/acceptance_materialization_work", json!(80)),
        ] {
            let mut value = configured();
            *value.pointer_mut(pointer).expect("existing field") = replacement;
            assert!(read(&value).is_err(), "{value}");
        }
        let mut value = configured();
        value["atom_cells"] = json!(8_000_000);
        assert!(read(&value).is_err());
    }

    #[test]
    fn legacy_acceptance_requires_present_null_buffer_fields() {
        let mut value = configured();
        value["acceptance_policy"] = json!({"kind":"cells","cells":0});
        value["acceptance_buffer_bytes"] = Value::Null;
        value["acceptance_materialization_work"] = Value::Null;
        assert!(read(&value).is_ok());
        for field in ["acceptance_buffer_bytes", "acceptance_materialization_work"] {
            let mut missing = value.clone();
            missing.as_object_mut().expect("object").remove(field);
            assert!(read(&missing).is_err(), "missing {field}");
            let mut numeric = value.clone();
            numeric[field] = json!(0);
            assert!(read(&numeric).is_err(), "numeric {field}");
        }
    }
}
