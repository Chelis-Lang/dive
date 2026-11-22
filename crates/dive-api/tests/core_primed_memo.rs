//! A zero-arity definition an action reads primed is a memo site keyed on
//! the variables it reads (#787): `Orders'` below reads `q'` only, so one
//! value serves every `ord'` candidate. The site is used only once its
//! primed variables are assigned, never caches an error and is never
//! evaluated for an empty enumeration; results are the reference
//! engine's with the site on and off.
#![allow(clippy::expect_used, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_primed_memo.json");

use std::path::PathBuf;

use dive_api::{CheckRequest, check_file, check_file_core_opts};
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

/// Core's observables with default options, and with the sites on (first)
/// and off.
fn compare(name: &str) -> (Value, Value) {
    let req = request(name);
    let reference = check_file(&req).expect("reference runs").to_json();
    let mut on = Value::Null;
    for switches in ["", "-primed-memo"] {
        let core = check_file_core_opts(&req, switches)
            .expect("core runs")
            .to_json();
        assert_eq!(core["engine"], "core", "{name} {switches}: {core:#}");
        agreed::check_eq(
            AGREED,
            "compare/0",
            &(observed(&core)),
            &(observed(&reference)),
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

/// The Core's observables with the sites on and off, which must agree.
fn core_on_off(name: &str) -> Value {
    let req = request(name);
    let on = check_file_core_opts(&req, "").expect("core runs").to_json();
    let off = check_file_core_opts(&req, "-primed-memo")
        .expect("core runs")
        .to_json();
    assert_eq!(on["engine"], "core", "{name}: {on:#}");
    assert_eq!(observed(&on), observed(&off), "{name}");
    on
}

/// #869 review: membership in a primed filter over `SUBSET` is decided on
/// the element through the site, as without it, never by building the set
/// to cache it.
#[test]
fn membership_in_a_primed_filter_does_not_build_it() {
    let (reference, _) = compare("LazyMember");
    assert_eq!(reference["verdict"], "deadlock", "{reference:#}");
    assert_eq!(reference["statistics"]["distinct"], 4);
    assert_eq!(reference["statistics"]["generated"], 4);
}

/// #869 review: a primed function constructor over `Nat`, applied and its
/// `DOMAIN` taken, is not a site; a primed filter enumerated by `y' \in`
/// is streamed.
#[test]
fn primed_values_used_without_being_built_stay_unbuilt() {
    let (reference, core) = compare("LazyShapes");
    assert_eq!(reference["verdict"], "deadlock", "{reference:#}");
    assert_eq!(reference["statistics"]["distinct"], 7);
    assert!(site(&core, "F (Primed, action)").is_none(), "{core:#}");
    assert!(site(&core, "D (Primed, action)").is_none(), "{core:#}");
}

/// #869 review: a primed `SUBSET` of 41 elements under a `SUBSET`-forall
/// filter keeps the #853 rewrite with the site on.
#[test]
fn a_primed_powerset_keeps_the_subset_rewrite() {
    let core = core_on_off("LazySubset");
    assert_eq!(core["verdict"], "deadlock", "{core:#}");
    assert_eq!(core["statistics"]["distinct"], 4);
}

#[test]
fn a_computed_component_ignores_unrelated_successor_choices() {
    let req = request("Component");
    let on = check_file_core_opts(&req, "")
        .expect("projected update")
        .to_json();
    let off = check_file_core_opts(&req, "-primed-memo")
        .expect("ordinary update")
        .to_json();
    assert_eq!(on["verdict"], "success");
    assert_eq!(observed(&on), observed(&off));
    let stats = site(&on, "successor component 2").expect("the table assignment is projected");
    assert!(stats["hits"].as_u64().unwrap_or(0) > 0);
    assert!(site(&off, "successor component 2").is_none());
}
