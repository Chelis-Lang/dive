//! Frame narrowing through the ordinary compiler, checker and trace API.
#![allow(clippy::expect_used, reason = "test fixtures and results")]

use std::fs;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, Store, check_file_core_opts};
use serde_json::Value;

const SPEC: &str = r"---- MODULE FramedChoices ----
EXTENDS Integers, SequencesExt, TLC
VARIABLE f

D == {1, 2, 3}
R == {<<>>, <<1>>, <<1, 1>>, <<1, 2>>}
BadRange == R \cup {7}
Init == f = [k \in D |-> <<>>]
InitOutside == f = [k \in D |-> IF k = 1 THEN <<99>> ELSE <<>>]
SameOutside == \A j \in D \ {2} : f'[j] = f[j]
Extend == /\ f' \in [D -> R]
          /\ IsPrefix(f[2], f'[2])
          /\ SameOutside
Next == Extend \/ Extend
PrintBeforeFrame == /\ f' \in [D -> R]
                    /\ PrintT(f'[2])
                    /\ SameOutside
ErrorBeforeFrame == /\ f' \in [D -> BadRange]
                    /\ IsPrefix(f[2], f'[2])
                    /\ SameOutside
MissingPrefixKey == /\ f' \in [D -> R]
                    /\ IsPrefix(f[99], f'[2])
                    /\ SameOutside
AlwaysEmpty == f[2] = <<>>
====
";

fn observables(report: &dive_api::CheckReport) -> Vec<Value> {
    let json = report.to_json();
    [
        &json["verdict"],
        &json["property"],
        &json["statistics"]["distinct"],
        &json["statistics"]["generated"],
        &json["statistics"]["depth"],
        &json["error_site"],
        &json["message"],
        &json["tlc"],
        &json["output"],
        &json["counterexample"]["trace"],
    ]
    .into_iter()
    .cloned()
    .chain([report.exit_code_for(ExitConvention::Native).into()])
    .collect()
}

#[test]
fn framed_choices_preserve_checks_output_and_independent_replay() {
    let dir = std::env::temp_dir().join(format!("dive-framed-choices-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("test directory");
    let file = dir.join("FramedChoices.tla");
    let config = file.with_extension("cfg");
    fs::write(&file, SPEC).expect("test model");
    for (init, next, property, expected) in [
        ("Init", "Next", "", "success"),
        (
            "Init",
            "Next",
            "INVARIANT AlwaysEmpty",
            "invariant-violation",
        ),
        ("Init", "PrintBeforeFrame", "", "success"),
        ("InitOutside", "ErrorBeforeFrame", "", "unsupported"),
        ("InitOutside", "MissingPrefixKey", "", "evaluation-error"),
    ] {
        fs::write(
            &config,
            format!("INIT {init}\nNEXT {next}\nCHECK_DEADLOCK FALSE\n{property}\n"),
        )
        .expect("test configuration");
        for workers in [1, 8] {
            let req = CheckRequest {
                file: file.clone(),
                config: Some(config.clone()),
                seed: Some(0),
                fp: Some(0),
                workers: Some(workers),
                store: Store::Exact,
                ..CheckRequest::default()
            };
            let off = check_file_core_opts(&req, "-framed-function-choices").expect("baseline");
            let on = check_file_core_opts(&req, "").expect("framed Core");
            assert_eq!(observables(&on), observables(&off), "{next}, {workers}");
            let json = on.to_json();
            assert_eq!(json["verdict"], expected, "{next}, {workers}: {json:#}");
            if next == "Next" && property.is_empty() {
                assert_eq!(json["statistics"]["distinct"], 4);
                assert_eq!(json["statistics"]["generated"], 19);
                assert_eq!(json["statistics"]["depth"], 2);
            }
            if next == "PrintBeforeFrame" {
                assert!(
                    json["output"]
                        .as_array()
                        .is_some_and(|items| !items.is_empty())
                );
            }
            if !property.is_empty() {
                let options = TraceOptions {
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                };
                let validator = SpecModel::load(&file, &config, &options).expect("bind validator");
                let doc = TraceDoc::from_json(&json["counterexample"]["trace"]).expect("trace");
                assert!(matches!(
                    validator.validate(&doc).validation,
                    Validation::Valid
                ));
            }
        }
    }
    fs::remove_dir_all(&dir).expect("remove generated fixtures");
}
