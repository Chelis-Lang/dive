//! Producer words retain independently reconstructed source/candidate meanings.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};
use serde_json::json;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn ten_retained_cases_validate_in_both_engines_profiles_and_worker_counts() -> TestResult {
    for name in [
        "ErViewChoose",
        "ErViewDomain",
        "ErViewNonBool",
        "ErViewNonBoolIf",
        "ErViewNonEnum",
        "ErViewTypeErr",
        "ErInvConstrained",
        "ErInvReplayError",
        "ErConstrNonBool",
        "ErActConstrNonBool",
    ] {
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "tests/fixtures/checker/errors/specs/{name}/{name}.tla"
        ));
        for compat_tlc in [false, true] {
            let model = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            {
                for workers in [1, 4] {
                    let report = check_file(&CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc,
                        workers: Some(workers),
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    })?
                    .to_json();
                    let wire = &report["counterexample"]["trace"];
                    let doc = TraceDoc::from_json(wire)
                        .map_err(|e| format!("{name}/{workers}: {e}: {report}"))?;
                    assert_eq!(
                        model.try_validate(&doc)?.validation,
                        Validation::Valid,
                        "{name}/{workers}: {report}"
                    );
                    if name.starts_with("ErView") {
                        assert_eq!(wire["version"], 6);
                        assert_eq!(doc.states.len(), 2);
                        assert_eq!(
                            doc.states[1],
                            json!({"x":1,"y":0}).as_object().ok_or("state")?.clone()
                        );
                    } else if name.starts_with("ErInv") {
                        assert_eq!(wire["version"], 2);
                        let (proof, class) = model.invariant_error_class(&doc, None)?;
                        assert_eq!(proof.validation, Validation::Valid, "{name}");
                        let class = class.ok_or("actual diagnostic certificate")?;
                        assert_eq!(report["tlc"]["reported"], class.reported_code());
                        assert_eq!(report["tlc"]["code"], class.deciding_code());
                    } else {
                        assert_eq!(wire["version"], 4);
                        assert_eq!(wire["ending"]["ec"], "TLC_EXPECTED_VALUE");
                    }
                    assert_eq!(report["tlc"]["verified"], true);
                    if workers == 1 {
                        // All existing tiny sources stop at their original
                        // generated counts; invariant diagnostic replay differs.
                        let (generated, distinct, depth) = if name == "ErInvReplayError" {
                            (4, 5, 3)
                        } else if name == "ErActConstrNonBool" {
                            (8, 5, 3)
                        } else {
                            (4, 3, 2)
                        };
                        assert_eq!(report["statistics"]["generated"], generated, "{name}");
                        assert_eq!(report["statistics"]["distinct"], distinct, "{name}");
                        assert_eq!(report["statistics"]["depth"], depth, "{name}");
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn view_module_errors_require_actual_primary_and_diagnostic_execution() -> TestResult {
    for kind in ["Assert", "DivZero", "Enabled", "Overflow"] {
        let name = format!("ErView{kind}");
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "tests/fixtures/checker/errors/specs/{name}/{name}.tla"
        ));
        for compat_tlc in [false, true] {
            if kind == "Overflow" && !compat_tlc {
                continue;
            }
            let model = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            {
                let report = check_file(&CheckRequest {
                    file: file.clone(),
                    config: Some(file.with_extension("cfg")),
                    compat_tlc,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                })?
                .to_json();
                let word = TraceDoc::from_json(&report["counterexample"]["trace"])?;
                assert_eq!(word.to_json()["version"], 6, "{name}");
                assert_eq!(
                    model.try_validate(&word)?.validation,
                    Validation::Valid,
                    "{name}"
                );
                assert_eq!(report["tlc"]["verified"], true);
                let mut legacy = word.to_json();
                legacy["version"] = json!(1);
                legacy["ending"] = json!({"kind":"error","ec":report["tlc"]["name"]});
                let legacy = TraceDoc::from_json(&legacy)?;
                assert!(matches!(
                    model.try_validate(&legacy)?.validation,
                    Validation::Unsupported(_)
                ));
                for pointer in ["/ending/endpoint/reported", "/ending/endpoint/deciding"] {
                    let mut forged = word.to_json();
                    *forged.pointer_mut(pointer).ok_or("endpoint")? = json!(2111);
                    assert!(
                        !model
                            .try_validate(&TraceDoc::from_json(&forged)?)?
                            .validation
                            .is_valid()
                    );
                }
            }
        }
    }
    Ok(())
}
