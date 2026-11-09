//! `TLCExt!Trace`, `TLCExt!CounterExample` and the `Json` and `IOUtils`
//! operators (issue #883) on the new core: a call the
//! core does not implement compiles to a node that refuses only when it is
//! evaluated, so a run that never reaches it (TLC evaluates a disjunct only
//! when the ones before it are false) gives the agreed result,
//! and one that reaches it names the operator.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_deferred_introspection.json");

use std::path::Path;

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

fn run(spec: &Path, engine: Engine, compat_tlc: bool) -> Value {
    let report = check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            compat_tlc,
            ..CheckRequest::default()
        },
        engine,
    )
    .expect("check runs");
    let mut j = report.to_json();
    j["exit"] = Value::from(report.exit_code());
    j
}

fn observed(v: &Value) -> Value {
    serde_json::json!({
        "verdict": v["verdict"],
        "generated": v["statistics"]["generated"],
        "distinct": v["statistics"]["distinct"],
        "depth": v["statistics"]["depth"],
        "exit": v["exit"],
    })
}

fn write(dir: &Path, name: &str, inv: &str) -> std::path::PathBuf {
    let spec = dir.join(format!("{name}.tla"));
    std::fs::write(
        &spec,
        format!(
            "---- MODULE {name} ----\nEXTENDS Integers, Sequences, TLC, TLCExt, Json, IOUtils\nVARIABLE x\n\
             Init == x = 0\nNext == x < 3 /\\ x' = x + 1\nInv == {inv}\n====\n"
        ),
    )
    .expect("spec");
    std::fs::write(
        spec.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
    )
    .expect("cfg");
    spec
}

#[test]
fn unreached_trace_introspection_is_not_refused_and_reached_names_it() {
    let dir = std::env::temp_dir().join(format!("dive-core-introspection-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cases = [
        ("Unreached", "x < 9 \\/ Len(Trace) > 0", None),
        (
            "UnreachedCx",
            "x < 9 \\/ CounterExample = [state |-> {}, action |-> {}]",
            None,
        ),
        ("Reached", "x < 2 \\/ Len(Trace) > 0", Some("TLCExt!Trace")),
        (
            "ReachedCx",
            "x < 2 \\/ CounterExample = {}",
            Some("TLCExt!CounterExample"),
        ),
        (
            "UnreachedJson",
            "x < 9 \\/ JsonSerialize(\"never-written.json\", Trace)",
            None,
        ),
        (
            "UnreachedExec",
            "x < 9 \\/ IOExecTemplate(<<\"true\", \"%s\">>, <<ToJsonObject(Trace)>>).exitValue = 0",
            None,
        ),
        (
            "ReachedJson",
            "x < 2 \\/ JsonSerialize(\"never-written.json\", <<x>>)",
            Some("Json!JsonSerialize"),
        ),
        (
            "ReachedToJson",
            "x < 2 \\/ ToJsonObject([a |-> x]) = \"{}\"",
            Some("Json!ToJsonObject"),
        ),
        (
            "ReachedExec",
            "x < 2 \\/ IOExec(<<\"true\">>).exitValue = 0",
            Some("IOUtils!IOExec"),
        ),
    ];
    for (name, inv, refused) in cases {
        let spec = write(&dir, name, inv);
        for compat in [false, true] {
            let at = format!("{name} compat={compat}");
            let core = run(&spec, Engine::Core, compat);
            assert_eq!(core["engine"], "core", "{at}: {core:#}");
            if let Some(op) = refused {
                assert_eq!(core["verdict"], "unsupported", "{at}: {core:#}");
                let message = core["message"].as_str().unwrap_or_default();
                assert!(
                    message.contains(op) && message.contains("evaluated"),
                    "{at}: {core:#}"
                );
            } else {
                assert_eq!(core["verdict"], "success", "{at}: {core:#}");
                let reference = run(&spec, Engine::Core, compat);
                agreed::check_eq(
                    AGREED,
                    "unreached_trace_introspection_is_not_refused_and_reached_names_it/0",
                    &(observed(&core)),
                    &(observed(&reference)),
                );
            }
        }
    }
    // No I/O is performed without a grant (D-017).
    assert!(!dir.join("never-written.json").exists());
    assert!(!Path::new("never-written.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A failing run whose `Print` (here, a trace export's) is never reached
/// gives the agreed result; one that prints still refuses.
#[test]
fn failing_run_that_never_prints_is_not_refused() {
    let dir = std::env::temp_dir().join(format!("dive-core-unprinted-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cases = [
        (
            "UnprintedFail",
            "x < 2 \\/ (x > 9 /\\ Print(ToJsonObject(Trace), FALSE))",
            false,
        ),
        ("PrintedFail", "x < 2 \\/ Print(x, FALSE)", true),
    ];
    for (name, inv, refused) in cases {
        let spec = write(&dir, name, inv);
        for compat in [false, true] {
            let at = format!("{name} compat={compat}");
            let core = run(&spec, Engine::Core, compat);
            assert_eq!(core["engine"], "core", "{at}: {core:#}");
            if refused {
                assert_eq!(core["verdict"], "unsupported", "{at}: {core:#}");
                let message = core["message"].as_str().unwrap_or_default();
                assert!(message.contains("Print or PrintT"), "{at}: {core:#}");
            } else {
                assert_eq!(core["verdict"], "invariant-violation", "{at}: {core:#}");
                let reference = run(&spec, Engine::Core, compat);
                agreed::check_eq(
                    AGREED,
                    "failing_run_that_never_prints_is_not_refused/0",
                    &(observed(&core)),
                    &(observed(&reference)),
                );
                agreed::check_eq(
                    AGREED,
                    "failing_run_that_never_prints_is_not_refused/1",
                    &(core["output"]),
                    &(reference["output"]),
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
