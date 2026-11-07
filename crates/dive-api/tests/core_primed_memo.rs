//! A zero-arity definition an action reads primed is a memo site keyed on
//! the variables it reads (#787): `Orders'` below reads `q'` only, so one
//! value serves every `ord'` candidate. The site is used only once its
//! primed variables are assigned, never caches an error and is never
//! evaluated for an empty enumeration; results are the reference
//! engine's with the site on and off.
#![allow(clippy::expect_used, reason = "test fixtures")]

use std::path::PathBuf;

use dive_api::{CheckRequest, Engine, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};

fn request(name: &str) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-primed-memo")
        .join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

fn observed(v: &Value) -> Value {
    json!({
        "verdict": v["verdict"],
        "generated": v["statistics"]["generated"],
        "distinct": v["statistics"]["distinct"],
        "depth": v["statistics"]["depth"],
        "message": v["message"],
        "tlc": v["tlc"],
        "trace": v["counterexample"]["trace"],
    })
}

/// The reference engine's observables, and the Core's with the sites on
/// (first) and off.
fn compare(name: &str) -> (Value, Value) {
    let req = request(name);
    let reference = check_file_engine(&req, Engine::Reference)
        .expect("reference runs")
        .to_json();
    let mut on = Value::Null;
    for switches in ["", "-primed-memo"] {
        let core = check_file_core_opts(&req, switches)
            .expect("core runs")
            .to_json();
        assert_eq!(core["engine"], "core", "{name} {switches}: {core:#}");
        assert_eq!(
            observed(&core),
            observed(&reference),
            "{name}, opts={switches}"
        );
        if switches.is_empty() {
            on = core;
        }
    }
    (reference, on)
}

fn site(core: &Value, name: &str) -> Option<Value> {
    core["core"]["metrics"]["memo"]
        .as_array()?
        .iter()
        .find(|s| s["site"] == name)
        .cloned()
}

#[test]
fn a_primed_definition_independent_of_the_enumerated_variable_is_reused() {
    let (reference, core) = compare("Orders");
    assert_eq!(reference["verdict"], "success", "{reference:#}");
    assert_eq!(reference["statistics"]["distinct"], 7);
    assert_eq!(reference["statistics"]["generated"], 22);
    let orders = site(&core, "Orders (Primed, action)").expect("Orders' is a site");
    assert!(
        orders["hits"].as_u64().unwrap_or(0) * 10 > orders["lookups"].as_u64().unwrap_or(0) * 9,
        "{orders:#}"
    );
    // `Top'` follows an empty enumeration: it is never evaluated, so the
    // failing CHOOSE of the empty `q'` of the initial state never runs.
    assert!(site(&core, "Top (Primed, action)").is_none(), "{core:#}");
}

#[test]
fn an_error_in_a_primed_site_is_raised_as_without_it() {
    let (reference, _) = compare("PrimedError");
    assert_eq!(reference["verdict"], "evaluation-error", "{reference:#}");
}

#[test]
fn a_primed_site_read_before_its_variable_is_assigned_raises_the_read_error() {
    let (reference, _) = compare("Unassigned");
    assert_eq!(reference["verdict"], "evaluation-error", "{reference:#}");
}
