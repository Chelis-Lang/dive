//! Comparisons across kinds in the new core (`--engine=core`), against the
//! reference engine in the default profile (`--comparisons=tla`, D-009;
//! `crates/dive-eval/src/tla_cmp.rs`).
//!
//! A direct comparison of values of different classes (scalars of different
//! kinds, a scalar and a compound, a set and a function) is TLC's error, and
//! so is membership of a value of the wrong class in a set. The core must
//! never answer such a comparison: before this test it answered `FALSE`, so
//! that a run TLC stops with an error reported success or a deadlock. Each
//! case runs on both engines; the core must give the reference's verdict and
//! counts, or refuse the run (it does not classify evaluation errors yet).
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::Path;

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

/// Invariants over `x \in 0..2`, each either an error or a plain answer in
/// the reference engine.
const CASES: &[&str] = &[
    r#"x # "a""#,
    r#""a" \notin {x, 5}"#,
    r#"x \in {1, "b"} \/ TRUE"#,
    r#"x \in {0, "b"}"#,
    r#"{x} # {"a"}"#,
    r#"<<x>> # <<"a">>"#,
    "TRUE \\notin {x}",
    r#"[a |-> x] # [a |-> "s"]"#,
    "x # {1}",
    "x # <<1>>",
    "{1} # x",
    "x \\notin {{1}}",
    "{1} \\notin {x}",
    r#""a" \notin {"b", "c"}"#,
    "x \\notin {<<1>>, 7}",
    "x \\notin {TRUE, FALSE}",
    "x \\notin {}",
    "{x} # <<1>>",
    "<<x>> # {1}",
    "<<x>> # [a |-> 1]",
    "{x} \\notin {<<1>>}",
    "<<x>> \\notin {{1}}",
    "<<x>> \\notin {<<1>>, {2}}",
    "{<<x>>} # {{1}}",
    "{} # <<>>",
    "<<>> # {}",
    "(x = 9) # {}",
    "x \\notin SUBSET {1}",
    "x \\notin [a : {1}]",
    "x \\notin [{1} -> {1}]",
    "x \\notin Seq({1})",
    "x \\notin {1} \\X {2}",
    "{x} \\notin Nat",
    r#""a" \notin Nat"#,
    "<<x>> \\notin SUBSET {1}",
    "{x} \\notin Seq({1})",
    "{x} \\subseteq {1, 2} \\/ TRUE",
    r#"{x} \cap {"a"} = {}"#,
];

/// The sets whose elements mix classes, which the core refuses
/// (`crates/dive-core/src/compare.rs`).
const MIXED: &[&str] = &[
    r#"x \in {1, "b"} \/ TRUE"#,
    r#"x \in {0, "b"}"#,
    "x \\notin {<<1>>, 7}",
    "<<x>> \\notin {<<1>>, {2}}",
];

fn run(spec: &Path, engine: Engine) -> Value {
    let report = check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
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
        "exit": v["exit"],
    })
}

#[test]
fn comparisons_across_kinds_are_never_answered_wrongly() {
    let dir = std::env::temp_dir().join(format!("dive-core-kinds-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut refused = 0;
    let mut answerable_refused = Vec::new();
    for (i, inv) in CASES.iter().enumerate() {
        let name = format!("K{i}");
        let spec = dir.join(format!("{name}.tla"));
        std::fs::write(
            &spec,
            format!(
                "---- MODULE {name} ----\nEXTENDS Integers, Sequences\nVARIABLE x\n\
                 Init == x = 0\nNext == x < 2 /\\ x' = x + 1\nInv == {inv}\n====\n"
            ),
        )
        .expect("spec");
        std::fs::write(
            spec.with_extension("cfg"),
            "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
        )
        .expect("cfg");
        let core = run(&spec, Engine::Core);
        let reference = run(&spec, Engine::Reference);
        // Never a wrong answer: the reference result, or an explicit
        // refusal. The core refuses an error until it classifies errors,
        // and refuses a set whose elements mix classes
        // (`04-plan-compiler.md` §7).
        if core["verdict"] == "unsupported" {
            refused += 1;
            if reference["verdict"] != "evaluation-error" {
                answerable_refused.push(*inv);
            }
            continue;
        }
        assert_eq!(
            observed(&core),
            observed(&reference),
            "Inv == {inv}: {core:#}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(refused > 0);
    // Only the mixed-class sets may be refused where the reference engine
    // answers.
    // Where the reference engine answers, only a mixed set may be refused.
    for inv in &answerable_refused {
        assert!(MIXED.contains(inv), "refused an answerable case: {inv}");
    }
}
