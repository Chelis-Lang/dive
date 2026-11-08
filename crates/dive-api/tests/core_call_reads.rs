//! Invariants that call recursive definitions are keyed and skipped by the
//! variables the called bodies read, never by their arguments alone.

use dive_api::{CheckRequest, check_file_core_opts};
use serde_json::{Value, json};
use std::path::PathBuf;

fn request(config: &str, workers: usize) -> CheckRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-call-reads");
    CheckRequest {
        file: root.join("CallReads.tla"),
        config: Some(root.join(format!("{config}.cfg"))),
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..Default::default()
    }
}

fn observed(doc: &Value) -> Value {
    json!({"verdict": doc["verdict"], "property": doc["property"],
        "distinct": doc["statistics"]["distinct"],
        "generated": doc["statistics"]["generated"],
        "depth": doc["statistics"]["depth"]})
}

fn sites(doc: &Value) -> Vec<String> {
    doc["core"]["metrics"]["memo"]
        .as_array()
        .map(|sites| {
            sites
                .iter()
                .filter_map(|s| s["site"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_called_body_variable_is_in_the_invariant_read_set() -> Result<(), Box<dyn std::error::Error>> {
    for workers in [1, 4] {
        let req = request("CallReads", workers);
        // Without memo tables or the parent-delta skip, every invariant is
        // evaluated in full on every new state.
        let plain = check_file_core_opts(&req, "-memo,-parent-skip")?.to_json();
        let expected = json!({"verdict": "invariant-violation", "property": "InvY",
            "distinct": 9, "generated": 12, "depth": 4});
        assert_eq!(observed(&plain), expected);
        let tuned = check_file_core_opts(&req, "")?.to_json();
        assert_eq!(observed(&tuned), expected, "{workers} workers");
        // `RecY(2)` and `f[2]` have constant arguments: their sites are keyed
        // by the variables their bodies read.
        let names = sites(&tuned);
        assert!(names.iter().any(|s| s == "InvX#0"), "{names:?}");
        assert!(names.iter().any(|s| s == "InvY#0"), "{names:?}");
    }
    Ok(())
}

#[test]
fn a_called_body_effect_is_never_cached_or_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let req = request("Shown", 1);
    let plain = check_file_core_opts(&req, "-memo,-parent-skip")?.to_json();
    let tuned = check_file_core_opts(&req, "")?.to_json();
    let expected = json!({"verdict": "success", "property": null,
        "distinct": 12, "generated": 18, "depth": 6});
    assert_eq!(observed(&plain), expected);
    assert_eq!(observed(&tuned), expected);
    let names = sites(&tuned);
    assert!(
        !names.iter().any(|s| s.starts_with("InvShown")),
        "{names:?}"
    );
    // Only `InvX` is skipped when `x` is unchanged; `InvShown` prints on
    // every new state.
    let conjuncts = &tuned["core"]["metrics"]["conjuncts"];
    assert_eq!(conjuncts["total"], 24);
    assert_eq!(conjuncts["parent_skipped"], 9);
    Ok(())
}
