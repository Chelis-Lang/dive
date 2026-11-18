//! A model that fails to bind ends a Core run with the Core's own binding
//! failure, reported exactly as the agreed record of that failure
//! (`support/agreed.rs`): verdict, message, location, printed output and
//! warnings. Core no longer runs the retired Reference checker to confirm it
//! (issue #791).
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_binding_failures.json");

use std::path::Path;

use dive_api::{CheckRequest, check_file};
use serde_json::Value;

/// `(name, module body, configuration)`: each fails at binding, after the
/// binder printed or evaluated something first.
const CASES: &[(&str, &str, &str)] = &[
    // Output printed while evaluating a constant definition comes before
    // the configuration error.
    (
        "BindPrint",
        "EXTENDS TLC, Naturals\nVARIABLE x\nNoise == Print(\"startup\", 1)\nVals == {n \\in 1..10 : n > 3}\nInit == x \\in Vals\nNext == x' = x\n",
        "INIT Init\nNEXT Next\nINVARIANT Missing\n",
    ),
    // A pure definition the default profile leaves to the Core fails at
    // start-up; TLC swallows that, and the configuration error is the
    // run's in both bindings.
    (
        "BindDeferred",
        "EXTENDS Naturals\nVARIABLE x\nBad == 1 \\div 0\nVals == {n \\in 1..10 : n > 3}\nInit == x \\in Vals\nNext == x' = x + Bad\n",
        "INIT Init\nNEXT Next\nINVARIANT Missing\n",
    ),
    // The temporal route binds with the requested liveness policy.
    (
        "BindProperty",
        "EXTENDS Naturals\nVARIABLE x\nVals == {n \\in 1..10 : n > 3}\nInit == x \\in Vals\nNext == x' = x\nSpec == Init /\\ [][Next]_x\n",
        "SPECIFICATION Spec\nPROPERTY Missing\n",
    ),
    // A constant the configuration does not assign.
    (
        "BindConstant",
        "EXTENDS Naturals\nCONSTANT N\nVARIABLE x\nInit == x = N\nNext == x' = x\n",
        "INIT Init\nNEXT Next\n",
    ),
];

fn run(spec: &Path, compat_tlc: bool) -> Value {
    check_file(&CheckRequest {
        config: Some(spec.with_extension("cfg")),
        file: spec.to_path_buf(),
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        compat_tlc,
        ..CheckRequest::default()
    })
    .expect("check runs")
    .to_json()
}

/// The report without what names the engine or measures the run.
fn observable(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        for key in [
            "engine",
            "core",
            "elapsed",
            "elapsed_ms",
            "timing",
            "resources",
        ] {
            o.remove(key);
        }
    }
    v
}

#[test]
fn core_binding_failures_are_reported_as_the_reference_engine_reports_them() {
    let dir = std::env::temp_dir().join(format!("dive-core-binding-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    for &(name, body, cfg) in CASES {
        let spec = dir.join(format!("{name}.tla"));
        std::fs::write(&spec, format!("---- MODULE {name} ----\n{body}====\n")).expect("spec");
        std::fs::write(spec.with_extension("cfg"), cfg).expect("cfg");
        for compat_tlc in [false, true] {
            let reference = run(&spec, compat_tlc);
            let core = run(&spec, compat_tlc);
            assert_eq!(core["engine"], "core", "{name}: {core:#}");
            assert_ne!(core["verdict"], "unsupported", "{name}: {core:#}");
            assert_ne!(core["verdict"], "success", "{name}: {core:#}");
            agreed::check_eq(
                AGREED,
                "core_binding_failures_are_reported_as_the_reference_engine_reports_them/0",
                &(observable(core.clone())),
                &(observable(reference.clone())),
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Under an option only Core implements, Core reports the binding failure
/// itself.
#[test]
fn core_reports_a_binding_failure_under_a_core_only_option() {
    let dir = std::env::temp_dir().join(format!("dive-core-binding-only-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let (name, body, cfg) = CASES[0];
    let spec = dir.join(format!("{name}.tla"));
    std::fs::write(&spec, format!("---- MODULE {name} ----\n{body}====\n")).expect("spec");
    std::fs::write(spec.with_extension("cfg"), cfg).expect("cfg");
    let report = check_file(&CheckRequest {
        config: Some(spec.with_extension("cfg")),
        file: spec.clone(),
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        candidate_memory_bytes: Some(1 << 20),
        ..CheckRequest::default()
    })
    .expect("check runs")
    .to_json();
    assert_eq!(report["verdict"], "model-error", "{report:#}");
    assert_eq!(report["tlc"]["name"], "TLC_CONFIG_SPECIFIED_NOT_DEFINED");
    assert_eq!(report["output"][0], "\"startup\"  1");
    let _ = std::fs::remove_dir_all(&dir);
}
