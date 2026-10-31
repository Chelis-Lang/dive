//! Actual receiver construction/replay lifetime, against official pinned primaries.
#![allow(clippy::expect_used, reason = "pinned regression assertions")]
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use serde_json::Value;
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
#[test]
fn actual_initial_receivers_match_pinned_source_wrappers() {
    for name in [
        "InitialLiteral",
        "Issue620",
        "InitialStateLeft",
        "InitialStateRight",
        "InitialTupleNested",
        "InitGeneration",
        "RecordLiteral",
        "LetTuple",
        "InstanceTuple",
    ] {
        let file = root().join(format!(
            "conformance/corpus/comparison-receiver-primary/specifications/{name}/{name}.tla"
        ));
        let golden:Value=serde_json::from_str(&std::fs::read_to_string(root().join(format!("conformance/golden/tlc-2026.10.06.014338/comparison-receiver-primary/specifications/{name}/{name}.cfg.json"))).expect("golden")).expect("JSON");
        for workers in [1, 4] {
            for engine in [Engine::Reference, Engine::Core, Engine::Auto] {
                for compat in [false, true] {
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc: compat,
                            workers: Some(workers),
                            seed: Some(19),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )
                    .expect("run");
                    let doc = report.to_json();
                    unchanged_counts(name, compat, &doc);
                    if !compat {
                        assert_eq!(
                            doc["verdict"], "success",
                            "{name}/{engine:?}/{workers}: {doc}"
                        );
                        continue;
                    }
                    if engine == Engine::Core {
                        assert_eq!(doc["verdict"], "unsupported", "{name}/{workers}: {doc}");
                        continue;
                    }
                    assert_eq!(
                        doc["tlc"]["code"], golden["result"]["verdict_code"],
                        "{name}/{engine:?}/{workers}: {doc}"
                    );
                    assert_eq!(doc["tlc"]["verified"], true, "{doc}");
                    if name != "InitGeneration" {
                        assert!(
                            report
                                .result()
                                .expect("safety result")
                                .counterexample
                                .is_none()
                        );
                        assert_eq!(
                            report
                                .diagnostic_witness()
                                .expect("typed diagnostic context")
                                .states()
                                .len(),
                            1
                        );
                        validate_initial_word(&file, name, &doc);
                    }

                    assert_eq!(
                        Value::from(report.exit_code_for(ExitConvention::Tlc)),
                        golden["result"]["exit_code"]
                    );
                    for (key, pin) in [
                        ("generated", "states_generated"),
                        ("distinct", "distinct_states"),
                        ("depth", "depth"),
                    ] {
                        assert_eq!(
                            doc["statistics"][key], golden["result"]["counts"][pin],
                            "{name}/{key}: {doc}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn cached_and_stored_receivers_do_not_acquire_a_diagnostic_source() {
    for name in [
        "InitialConstantBoth",
        "InitialConstantLeft",
        "NextConstant",
        "NextStateLeft",
    ] {
        let file = root().join(format!(
            "crates/dive-api/tests/fixtures/comparison-receiver-unclassified/{name}.tla"
        ));
        for workers in [1, 4] {
            for engine in [Engine::Reference, Engine::Core, Engine::Auto] {
                for compat in [false, true] {
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc: compat,
                            workers: Some(workers),
                            seed: Some(19),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )
                    .expect("run");
                    let doc = report.to_json();
                    unchanged_counts(name, compat, &doc);
                    if !compat {
                        assert_eq!(doc["verdict"], "success", "{name}: {doc}");
                    } else if engine == Engine::Core {
                        assert_eq!(doc["verdict"], "unsupported", "{name}: {doc}");
                    } else {
                        assert_eq!(doc["tlc"]["code"], 2103, "{name}: {doc}");
                        assert_eq!(doc["tlc"]["verified"], false, "{name}: {doc}");
                        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 75);
                    }
                }
            }
        }
    }
}

#[test]
fn unrelated_tool_errors_and_later_errors_keep_their_existing_primary_contract() {
    for name in [
        "InitialTool",
        "InitialOverride",
        "InitialDivZero",
        "NextDivZero",
        "SubsequentInvariant",
        "NextLiteral",
    ] {
        let classified = matches!(name, "NextDivZero" | "SubsequentInvariant" | "NextLiteral");
        let file = if classified {
            root().join(format!(
                "conformance/corpus/comparison-receiver-primary/specifications/{name}/{name}.tla"
            ))
        } else {
            root().join(format!(
                "crates/dive-api/tests/fixtures/comparison-receiver-unclassified/{name}.tla"
            ))
        };
        for workers in [1, 4] {
            for engine in [Engine::Reference, Engine::Core, Engine::Auto] {
                for compat in [false, true] {
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc: compat,
                            workers: Some(workers),
                            seed: Some(19),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )
                    .expect("run");
                    let doc = report.to_json();
                    unchanged_counts(name, compat, &doc);
                    if (name == "SubsequentInvariant" || name == "NextLiteral") && !compat {
                        assert_eq!(doc["verdict"], "success", "{name}: {doc}");
                        continue;
                    }
                    if engine == Engine::Core
                        && (name == "InitialOverride"
                            || (compat && (name == "SubsequentInvariant" || name == "NextLiteral")))
                    {
                        assert_eq!(doc["verdict"], "unsupported", "{name}: {doc}");
                        continue;
                    }
                    let code = match name {
                        "NextDivZero" => 2179,
                        "SubsequentInvariant" => 2111,
                        _ => 2103,
                    };
                    assert_eq!(doc["tlc"]["code"], code, "{name}/{engine:?}: {doc}");
                    assert_eq!(
                        doc["tlc"]["verified"],
                        name != "NextLiteral",
                        "{name}: {doc}"
                    );
                    assert_eq!(
                        report.exit_code_for(ExitConvention::Tlc),
                        match code {
                            2179 => 255,
                            2111 => 76,
                            _ => 75,
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn later_initial_rows_and_unproved_state_history_do_not_gain_a_certificate() {
    let dir = std::env::temp_dir().join(format!("dive-receiver-bound-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("directory");
    for (body, extra) in [
        (
            "VARIABLE x\nInit==x = <<0>> \\/ x = <<1>>\nNext==UNCHANGED x\nInv==x = <<0>> \\/ x#[a|->0]",
            "",
        ),
        (
            "VARIABLES x,bag\nInit==x = <<0>> /\\ bag={0}\nNext==UNCHANGED <<x,bag>>\nInv==x#[a|->0]",
            "",
        ),
        (
            "VARIABLE x\nInit==x = <<0>>\nNext==UNCHANGED x\nInv==x#[a|->0]\nV==0",
            "VIEW V\n",
        ),
        (
            "VARIABLE x\nInit==x = <<0>>\nNext==UNCHANGED x\nInv==x#[a|->0]\nC==x[1]>=0",
            "CONSTRAINT C\n",
        ),
        (
            "VARIABLE x\nInit==x = <<0>>\nNext==UNCHANGED x\nInv==x#[a|->0]\nEarlier==x[1]>=0",
            "INVARIANT Earlier\n",
        ),
    ] {
        let file = dir.join("Bound.tla");
        std::fs::write(
            &file,
            format!("---- MODULE Bound ----\nEXTENDS Naturals\n{body}\n====\n"),
        )
        .expect("source");
        std::fs::write(
            file.with_extension("cfg"),
            format!("INIT Init\nNEXT Next\n{extra}INVARIANT Inv\nCHECK_DEADLOCK FALSE\n"),
        )
        .expect("config");
        for workers in [1, 4] {
            for engine in [Engine::Reference, Engine::Auto] {
                let report = check_file_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc: true,
                        workers: Some(workers),
                        seed: Some(19),
                        fp: Some(0),
                        ..CheckRequest::default()
                    },
                    engine,
                )
                .expect("run");
                let doc = report.to_json();
                assert_eq!(doc["verdict"], "evaluation-error", "{body}: {doc}");
                assert_eq!(doc["tlc"]["verified"], false, "{body}: {doc}");
                assert_ne!(doc["tlc"]["code"], 2147, "{body}: {doc}");
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

// Preserved original16-case API counts, including queue-at-error semantics.
// New scoped initial constructors have the same three-state counter control.
fn unchanged_counts(name: &str, compat: bool, doc: &Value) {
    if doc["verdict"] == "unsupported" {
        return;
    }
    let (generated, distinct, depth, queue) = if compat {
        match name {
            "InitGeneration" => (0, 0, 0, 0),
            "SubsequentInvariant" => (2, 2, 2, 0),
            "NextConstant" | "NextDivZero" | "NextLiteral" | "NextStateLeft" => (1, 1, 1, 0),
            _ => (1, 1, 1, 1),
        }
    } else {
        match name {
            "InitGeneration" | "InitialStateLeft" | "InitialStateRight" | "NextStateLeft" => {
                (2, 1, 1, 0)
            }
            "InitialDivZero" | "InitialOverride" | "InitialTool" => (1, 1, 1, 1),
            "NextDivZero" => (1, 1, 1, 0),
            _ => (3, 3, 3, 0),
        }
    };
    for (field, expected) in [
        ("generated", generated),
        ("distinct", distinct),
        ("depth", depth),
        ("left_on_queue", queue),
    ] {
        assert_eq!(
            doc["statistics"][field], expected,
            "{name}/{compat}/{field}: {doc}"
        );
    }
}

fn validate_initial_word(file: &std::path::Path, name: &str, doc: &Value) {
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
    assert!(
        doc["counterexample"].is_null(),
        "TLC has no formal word: {doc}"
    );
    assert_eq!(doc["diagnostic_witness"]["trace"]["version"], 2, "{doc}");
    assert_eq!(
        doc["diagnostic_witness"]["trace"]["ending"],
        serde_json::json!({"kind":"invariant-error","name":"Inv","ec":"GENERAL"}),
        "{doc}"
    );
    let options = TraceOptions {
        compat_tlc: true,
        run: Some((19, 0)),
        ..TraceOptions::default()
    };
    let model = SpecModel::load(file, &file.with_extension("cfg"), &options)
        .expect("fresh independent model");
    let word = TraceDoc::from_json(&doc["diagnostic_witness"]["trace"]).expect("word");
    let (report, class) = model
        .initial_invariant_class(&word, None)
        .expect("fresh source replay");
    assert!(
        matches!(report.validation, Validation::Valid),
        "{name}: actual named word"
    );
    let class = class.expect("independent actual first Init/Inv wrapper proof");
    assert_eq!(class.deciding_code(), 2147);
    assert_eq!(class.reported_code(), 2102);
    for (key, bad) in [("ec", "TLC_FINGERPRINT_EXCEPTION"), ("name", "Forged")] {
        let mut changed = doc["diagnostic_witness"]["trace"].clone();
        changed["ending"][key] = Value::from(bad);
        let mutant = TraceDoc::from_json(&changed).expect("mutant");
        let (report, class) = model
            .initial_invariant_class(&mutant, None)
            .expect("fresh mutant replay");
        assert!(
            matches!(report.validation, Validation::Invalid(_)),
            "{name}/{key}: mutant must fail"
        );
        assert!(
            class.is_none(),
            "invalid word cannot supply a wrapper certificate"
        );
    }
}
