//! Candidate-side wrappers need independent primary and diagnostic execution.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::json;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn actual_candidate_errors_require_exact_word_source_site_and_both_identities() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = ["Constr", "ActConstr", "Prop"]
        .into_iter()
        .flat_map(|site| {
            ["Choose", "Domain", "NonBoolIf", "NonEnum", "TypeErr"].map(|kind| (site, kind))
        })
        .chain([
            ("Prop", "NonBool"),
            ("Constr", "NonBool"),
            ("ActConstr", "NonBool"),
        ]);
    for (site, kind) in cases {
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
                if site == "Prop" && kind == "NonBool" {
                    assert_inner_nonboolean_source_priority(&file, &model, wire, compat_tlc)?;
                }
            }
        }
    }
    Ok(())
}

fn assert_mutations(model: &SpecModel, wire: &serde_json::Value, site: &str) -> TestResult {
    let wrong_cause = if wire["ending"]["ec"] == "TLC_MODULE_DIVISION_BY_ZERO" {
        "GENERAL"
    } else {
        "TLC_MODULE_DIVISION_BY_ZERO"
    };
    let wrong_deciding = if wire["ending"]["endpoint"]["deciding"] == 2179 {
        2103
    } else {
        2179
    };
    for (pointer, value) in [
        ("/ending/ec", json!(wrong_cause)),
        ("/ending/endpoint/reported", json!(2111)),
        ("/ending/endpoint/deciding", json!(wrong_deciding)),
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
fn direct_module_errors_and_assertions_replay_actual_candidate_and_diagnostic() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for site in ["Constr", "ActConstr", "Prop"] {
        for kind in ["Assert", "DivZero", "Enabled", "Overflow", "Seen"] {
            if kind == "Seen" && site != "Prop" {
                continue;
            }
            let name = format!("Er{site}{kind}");
            let file = root.join(format!(
                "crates/dive-check/tests/fixtures/errors/specs/{name}/{name}.tla"
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
                for (engine, workers) in [Engine::Reference, Engine::Core]
                    .into_iter()
                    .flat_map(|engine| [1, 4].map(|workers| (engine, workers)))
                {
                    let result = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            seed: Some(0),
                            fp: Some(0),
                            workers: Some(workers),
                            ..CheckRequest::default()
                        },
                        engine,
                    )?
                    .to_json();
                    let wire = &result["counterexample"]["trace"];
                    assert_eq!(wire["version"], 4, "{name}: {result}");
                    assert_eq!(
                        wire["ending"]["endpoint"]["stage"],
                        "candidate-ordered-terminal"
                    );
                    let expected = match kind {
                        "Assert" => 2132,
                        "Overflow" => 2178,
                        _ => 2179,
                    };
                    assert_eq!(result["tlc"]["code"], expected, "{name}: {result}");
                    assert_eq!(
                        model.try_validate(&TraceDoc::from_json(wire)?)?.validation,
                        Validation::Valid,
                        "{name}: {result}"
                    );
                    assert_mutations(&model, wire, site)?;
                }
            }
        }
    }
    Ok(())
}

fn assert_inner_nonboolean_source_priority(
    file: &std::path::Path,
    model: &SpecModel,
    wire: &serde_json::Value,
    compat_tlc: bool,
) -> TestResult {
    let source = std::fs::read_to_string(file)?;
    let endpoint = &wire["ending"]["endpoint"];
    let start = usize::try_from(endpoint["source"]["start"].as_u64().ok_or("source start")?)?;
    let end = usize::try_from(endpoint["source"]["end"].as_u64().ok_or("source end")?)?;
    assert_eq!(source.get(start..end), Some("[Err(x)]_<<x, y>>"));
    assert_eq!(endpoint["reported"], 2113);
    assert_eq!(endpoint["deciding"], 2103);
    assert_eq!(endpoint["site"], "implied-action");
    assert_eq!(wire["ending"]["ec"], "GENERAL");
    let mut bad = wire.clone();
    bad["ending"]["endpoint"]["source"]["start"] = json!(start + 1);
    assert!(matches!(
        model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
        Validation::Invalid(_)
    ));
    let terminal_x = wire["states"]
        .as_array()
        .ok_or("states")?
        .last()
        .ok_or("terminal")?["values"]["x"]
        .as_i64()
        .ok_or("terminal x")?;
    let earlier_source = source.replace(
        "====",
        &format!("Earlier == x # {terminal_x} \\/ 1 \\div 0 = 0\n===="),
    );
    let earlier_config = format!(
        "{}\nCONSTRAINT Earlier\n",
        std::fs::read_to_string(file.with_extension("cfg"))?
    );
    let earlier = SpecModel::from_source(
        file.file_name()
            .and_then(|name| name.to_str())
            .ok_or("file name")?,
        &earlier_source,
        &earlier_config,
        &TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )?;
    assert!(
        matches!(earlier.try_validate(&TraceDoc::from_json(wire)?)?.validation, Validation::Invalid(ref failure) if failure.check == dive_api::trace::Check::Ending),
        "an earlier actual constraint error must decide before the requested inner action error"
    );
    Ok(())
}

#[test]
fn top_level_nonboolean_role_proofs_do_not_admit_the_old_anonymous_word() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["ErConstrNonBool", "ErActConstrNonBool"] {
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
                assert_eq!(wire["version"], 4);
                assert_eq!(wire["ending"]["ec"], "TLC_EXPECTED_VALUE");
                assert_eq!(
                    model.try_validate(&TraceDoc::from_json(wire)?)?.validation,
                    Validation::Valid
                );
                // The retained old anonymous ending still cannot certify a
                // candidate-side role failure or its diagnostic wrapper.
                let old = TraceDoc::from_json(wire)?.with_ending(dive_api::trace::Ending::Error {
                    ec: Some("TLC_EXPECTED_VALUE".into()),
                });
                assert!(
                    !model.try_validate(&old)?.validation.is_valid(),
                    "{name}: {result}"
                );
            }
        }
    }
    Ok(())
}
