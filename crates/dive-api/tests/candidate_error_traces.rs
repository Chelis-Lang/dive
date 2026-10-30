//! Candidate-side wrappers need independent primary and diagnostic execution.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::json;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn actual_candidate_errors_require_exact_word_source_site_and_both_identities() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for site in ["Constr", "ActConstr", "Prop"] {
        for kind in ["Choose", "Domain", "NonBoolIf", "NonEnum", "TypeErr"] {
            let name = format!("Er{site}{kind}");
            let file = root.join(format!(
                "crates/dive-check/tests/fixtures/errors/specs/{name}/{name}.tla"
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
                for (engine, workers) in [Engine::Reference, Engine::Core]
                    .into_iter()
                    .flat_map(|engine| [1, 4].map(|workers| (engine, workers)))
                {
                    let result = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            workers: Some(workers),
                            seed: Some(0),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )?
                    .to_json();
                    let wire = &result["counterexample"]["trace"];
                    assert_eq!(result["tlc"]["code"], 2103, "{name}: {result}");
                    assert_eq!(wire["version"], 4, "{name}: {result}");
                    assert_eq!(
                        wire["ending"]["endpoint"]["stage"],
                        "candidate-ordered-terminal"
                    );
                    let doc = TraceDoc::from_json(wire)?;
                    assert_eq!(
                        model.try_validate(&doc)?.validation,
                        Validation::Valid,
                        "{name}/{compat_tlc}/{engine:?}: {result}"
                    );
                    assert!(matches!(
                        model.validate(&doc).validation,
                        Validation::Unsupported(_)
                    ));
                    assert_mutations(&model, wire, site)?;
                }
            }
        }
    }
    Ok(())
}

fn assert_mutations(model: &SpecModel, wire: &serde_json::Value, site: &str) -> TestResult {
    for (pointer, value) in [
        ("/ending/ec", json!("TLC_MODULE_DIVISION_BY_ZERO")),
        ("/ending/endpoint/reported", json!(2111)),
        ("/ending/endpoint/deciding", json!(2179)),
        ("/ending/endpoint/source/module", json!("Forged")),
    ] {
        let mut bad = wire.clone();
        *bad.pointer_mut(pointer).ok_or("missing field")? = value;
        assert!(
            matches!(
                model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
                Validation::Invalid(_)
            ),
            "{bad}"
        );
    }
    let mut bad = wire.clone();
    let last = bad["states"].as_array().ok_or("states")?.len() - 1;
    bad["states"][last]["values"]["x"] = json!(99);
    assert!(matches!(
        model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
        Validation::Invalid(_)
    ));
    let mut bad = wire.clone();
    bad["ending"]["endpoint"]["site"] = json!(if site == "Constr" {
        "action-constraint"
    } else {
        "constraint"
    });
    assert!(matches!(
        model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
        Validation::Invalid(_)
    ));
    let mut bad = wire.clone();
    bad["length"] = json!(1);
    assert!(matches!(
        model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
        Validation::Invalid(_)
    ));
    for version in [1, 2, 3, 5] {
        let mut bad = wire.clone();
        bad["version"] = json!(version);
        assert!(TraceDoc::from_json(&bad).is_err());
    }
    let mut bad = wire.clone();
    bad["ending"]["endpoint"]["site"] = json!("view");
    assert!(TraceDoc::from_json(&bad).is_err());
    Ok(())
}

#[test]
fn direct_module_errors_and_assertions_keep_their_existing_unproved_endings() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for site in ["Constr", "ActConstr", "Prop"] {
        for kind in ["Assert", "DivZero", "Enabled"] {
            let name = format!("Er{site}{kind}");
            let file = root.join(format!(
                "crates/dive-check/tests/fixtures/errors/specs/{name}/{name}.tla"
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
                for engine in [Engine::Reference, Engine::Core] {
                    let result = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            seed: Some(0),
                            fp: Some(0),
                            workers: Some(1),
                            ..CheckRequest::default()
                        },
                        engine,
                    )?
                    .to_json();
                    let wire = &result["counterexample"]["trace"];
                    assert_ne!(
                        wire["ending"]["endpoint"]["stage"],
                        "candidate-ordered-terminal"
                    );
                    assert_ne!(
                        model.try_validate(&TraceDoc::from_json(wire)?)?.validation,
                        Validation::Valid,
                        "{name}: {result}"
                    );
                }
            }
        }
    }
    Ok(())
}
