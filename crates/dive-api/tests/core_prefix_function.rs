//! Finite unbounded CHOOSE equations through the ordinary checker and replay.

#![allow(clippy::expect_used, reason = "test fixture and assertions")]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, check_file};
use serde_json::{Value, json};

const PREFIX: &str = r"CHOOSE g : g = [n \in 1..3 |->
    LET value == IF n = 1 THEN 2 ELSE g[n - 1] IN value + n]";
const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT StillZero\nCHECK_DEADLOCK FALSE\n";

fn source(recurrence: &str, next: &str) -> String {
    format!(
        "---- MODULE PrefixControl ----\nEXTENDS Integers\nVARIABLE x\nBuild == {recurrence}\nInit == x = 0\nNext == {next}\nStillZero == x = 0\n====\n"
    )
}

fn check(directory: &std::path::Path, source: &str, compat_tlc: bool) -> Value {
    let module = directory.join("PrefixControl.tla");
    let config = module.with_extension("cfg");
    std::fs::write(&module, source).expect("source");
    std::fs::write(&config, CFG).expect("config");
    check_file(&CheckRequest {
        file: module,
        config: Some(config),
        compat_tlc,
        store: Store::Exact,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..CheckRequest::default()
    })
    .expect("check")
    .to_json()
}

#[test]
fn builds_and_replays_a_unique_prefix() {
    let directory = std::env::temp_dir().join(format!("dive-prefix-valid-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("scratch");
    let original = source(PREFIX, "x' = Build[3]");
    let result = check(&directory, &original, false);
    assert_eq!(result["verdict"], "invariant-violation", "{result}");
    let mut word = result["counterexample"]["trace"].clone();
    assert_eq!(word["states"][1]["values"]["x"], 8);
    let model = SpecModel::from_source(
        "PrefixControl.tla",
        &original,
        CFG,
        &TraceOptions::default(),
    )
    .expect("separately bound model");
    let trace = TraceDoc::from_json(&word).expect("trace");
    assert_eq!(model.validate(&trace).validation, Validation::Valid);
    word["states"][1]["values"]["x"] = json!(7);
    let damaged = TraceDoc::from_json(&word).expect("damaged trace");
    assert!(matches!(
        model.validate(&damaged).validation,
        Validation::Invalid(_)
    ));
    assert_eq!(check(&directory, &original, true)["verdict"], "unsupported");
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn empty_domain_and_demanded_failures() {
    let directory =
        std::env::temp_dir().join(format!("dive-prefix-invalid-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("scratch");
    let empty = source(
        r"CHOOSE g : [n \in 1..0 |-> g[n-1]] = g",
        r"x' = IF DOMAIN Build = {} THEN x ELSE 1",
    );
    assert_eq!(check(&directory, &empty, false)["verdict"], "success");

    for (expression, verdict) in [
        (
            r"CHOOSE g : g = [n \in 1..2 |-> IF n = 1 THEN g[2] ELSE g[1]]",
            "unsupported",
        ),
        (r"CHOOSE g : g = [n \in 1..1 |-> g[n] + 1]", "unsupported"),
        (
            r"CHOOSE g : g = [n \in 1..1 |-> g[n-1]]",
            "evaluation-error",
        ),
        (
            r"CHOOSE g : g = [n \in 1..1 |-> IF n = 1 THEN 1 \div 0 ELSE g[n-1]]",
            "evaluation-error",
        ),
        (r"CHOOSE g : g = [n \in 1..1 |-> DOMAIN g]", "unsupported"),
    ] {
        let row = check(&directory, &source(expression, "x' = Build[1]"), false);
        assert_eq!(row["verdict"], verdict, "{expression} {row}");
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}
