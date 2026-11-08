//! Every admitted logical coverage row agrees with the pinned TLC fixture.
use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::{Value, json};

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/tlc-coverage-primary/specifications/Coverage")
        .join(format!("{name}.tla"))
}
fn project() -> std::path::PathBuf {
    fixture("LogicalCoverage")
}
fn projected_actions(doc: &Value) -> Value {
    json!(doc["tlc_coverage"]["actions"].as_array().into_iter().flatten().map(|action| {
        json!({"label":action["label"],"initial":action["initial"],"distinct":action["distinct"],"generated":action["generated"],
            "rows":action["rows"].as_array().into_iter().flatten().map(|row| json!({"depth":row["depth"],"begin":[row["begin"]["line"],row["begin"]["column"]],"end":[row["end"]["line"],row["end"]["column"]],"module":row["module"],"invocations":row["invocations"],"cost":row["cost"]})).collect::<Vec<_>>()})
    }).collect::<Vec<_>>())
}
#[test]
fn logical_action_costs_match_pinned_rows_and_preserve_lazy_demand_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let expected: Value = serde_json::from_str(include_str!(
        "../../../conformance/corpus/tlc-coverage-primary/specifications/Coverage/LogicalCoverage.json"
    ))?;
    for compat in [false, true] {
        let mut request = CheckRequest {
            file: project(),
            compat_tlc: compat,
            workers: Some(4),
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let plain = check_file_engine(&request, Engine::Reference)?.to_json();
        request.tlc_coverage = true;
        assert!(matches!(
            dive_api::check_file(&request),
            Err(dive_api::ApiError::Unsupported { feature }) if feature.contains("not implemented on Core")
        ));
        let report = check_file_engine(&request, Engine::Reference)?;
        let typed = report.tlc_coverage().ok_or("missing typed coverage")?;
        assert!(!typed.active);
        assert!(!typed.overflowed);
        assert!(report.source_coverage().is_none());
        let observed = report.to_json();
        assert_eq!(observed["verdict"], "success", "{observed}");
        assert_eq!(observed["options"]["workers"]["used"], 1);
        assert_eq!(projected_actions(&observed), expected["actions"]);
        for key in [
            "verdict",
            "statistics",
            "tlc",
            "output",
            "counterexample",
            "warnings",
        ] {
            assert_eq!(observed[key], plain[key], "{key}");
        }
        assert!(observed.get("source_coverage").is_none());
        let human = report.render_human();
        assert!(human.contains("TLC logical coverage"));
        assert_eq!(report.to_json()["tlc_coverage"], observed["tlc_coverage"]);
    }
    Ok(())
}
#[test]
fn logical_coverage_requires_an_explicit_supported_backend_and_separate_metric() {
    let mut request = CheckRequest {
        file: project(),
        tlc_coverage: true,
        ..CheckRequest::default()
    };
    assert!(check_file_engine(&request, Engine::Core).is_err());
    request.source_coverage = true;
    assert!(check_file_engine(&request, Engine::Reference).is_err());
    request.source_coverage = false;
    request.temporal = dive_api::TemporalMode::FinalShort;
    assert!(check_file_engine(&request, Engine::Reference).is_err());
}

#[test]
fn multi_variable_generators_reused_helpers_and_domain_costs_match_the_complete_pinned_rows()
-> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "Multi",
        "MultiDomains",
        "Guards",
        "IfAction",
        "InitialFilter",
        "LazyEffects",
        "SharedAction",
        "QuantSet",
        "MultiQuant",
        "QuantShort",
        "Headers",
        "Unreached",
        "PrimeArg",
        "PrimeAlias",
        "AliasPrimeTarget",
        "HiddenPrimeRhs",
        "SharedMarker",
        "MarkerLazyActual",
        "EmptyActionMarker",
    ] {
        let file = fixture(name);
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(file.with_extension("json"))?)?;
        for compat in [false, true] {
            let mut request = CheckRequest {
                file: file.clone(),
                compat_tlc: compat,
                workers: Some(1),
                seed: Some(7),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let plain = check_file_engine(&request, Engine::Reference)?.to_json();
            request.tlc_coverage = true;
            let report = check_file_engine(&request, Engine::Reference)?;
            let observed = report.to_json();
            assert_eq!(
                observed["tlc_coverage"]["status"], "available",
                "{name}: {observed}"
            );
            assert_eq!(
                projected_actions(&observed),
                expected["actions"],
                "{name} / compat={compat}"
            );
            for key in ["distinct", "generated", "depth"] {
                assert_eq!(observed["statistics"][key], expected[key], "{name}: {key}");
            }
            for key in [
                "verdict",
                "statistics",
                "tlc",
                "output",
                "counterexample",
                "warnings",
            ] {
                assert_eq!(observed[key], plain[key], "{name}: {key}");
            }
            if name == "MarkerLazyActual" {
                assert_eq!(observed["output"], json!([]));
            }
            if name == "LazyEffects" {
                assert_eq!(observed["output"], json!(["\"once\""]));
            }
            assert_eq!(observed["tlc_coverage"], report.to_json()["tlc_coverage"]);
            let _ = report.render_human();
            assert_eq!(observed["tlc_coverage"], report.to_json()["tlc_coverage"]);
        }
    }
    Ok(())
}
#[test]
fn failed_producer_refuses_unproved_tlc_reconstruction_costs_without_repeating_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let file = fixture("FailDemand");
    let oracle: Value =
        serde_json::from_str(&std::fs::read_to_string(file.with_extension("json"))?)?;
    assert_eq!(oracle["actions"][0]["generated"], 2); // TLC replays Init for its diagnostic word.
    for compat in [false, true] {
        let mut request = CheckRequest {
            file: file.clone(),
            compat_tlc: compat,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let plain = check_file_engine(&request, Engine::Reference)?.to_json();
        request.tlc_coverage = true;
        let report = check_file_engine(&request, Engine::Reference)?;
        let observed = report.to_json();
        assert_eq!(observed["verdict"], "assertion-failure");
        assert_eq!(observed["tlc_coverage"]["status"], "unsupported");
        assert!(observed["tlc_coverage"].get("actions").is_none());
        assert!(
            observed["tlc_coverage"]["feature"]
                .as_str()
                .is_some_and(|feature| feature.contains("diagnostic-reconstruction"))
        );
        assert_eq!(observed["tlc"]["exit_status"], oracle["oracle_exit"]);
        for key in [
            "verdict",
            "statistics",
            "tlc",
            "output",
            "counterexample",
            "warnings",
        ] {
            assert_eq!(observed[key], plain[key], "{key}");
        }
        let model = dive_api::trace::SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &dive_api::trace::TraceOptions {
                compat_tlc: compat,
                run: Some((7, 0)),
                ..Default::default()
            },
        )?;
        let word = dive_api::trace::TraceDoc::from_json(&observed)?;
        assert_eq!(
            model.try_validate(&word)?.validation,
            dive_api::trace::Validation::Valid
        );
        assert_eq!(
            report
                .tlc_coverage()
                .ok_or("missing coverage refusal")?
                .actions
                .len(),
            0
        );
    }
    Ok(())
}

#[test]
fn stopped_run_keeps_its_original_limits_and_refuses_coverage_costs()
-> Result<(), Box<dyn std::error::Error>> {
    let mut request = CheckRequest {
        file: fixture("Multi"),
        max_states: Some(1),
        workers: Some(1),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    };
    let plain = check_file_engine(&request, Engine::Reference)?.to_json();
    request.tlc_coverage = true;
    let observed = check_file_engine(&request, Engine::Reference)?.to_json();
    assert_eq!(observed["verdict"], "stopped");
    assert_eq!(observed["tlc_coverage"]["status"], "unsupported");
    assert!(observed["tlc_coverage"].get("actions").is_none());
    for key in [
        "verdict",
        "statistics",
        "tlc",
        "counterexample",
        "completion",
    ] {
        assert_eq!(observed[key], plain[key], "{key}");
    }
    Ok(())
}

#[test]
fn unimplemented_compound_value_cost_map_refuses_only_coverage()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::env::temp_dir().join(format!("dive-logical-compound-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("Compound.tla");
    std::fs::write(
        &file,
        "---- MODULE Compound ----\nVARIABLE x\nInit == x={0}\nNext == x'=x\n====\n",
    )?;
    std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
    let mut request = CheckRequest {
        file: file.clone(),
        workers: Some(1),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    };
    let plain = check_file_engine(&request, Engine::Reference)?.to_json();
    request.tlc_coverage = true;
    let observed = check_file_engine(&request, Engine::Reference)?.to_json();
    assert_eq!(observed["verdict"], "success");
    assert_eq!(observed["tlc_coverage"]["status"], "unsupported");
    assert!(observed["tlc_coverage"].get("actions").is_none());
    assert!(
        observed["tlc_coverage"]["feature"]
            .as_str()
            .is_some_and(|feature| feature.contains("compound"))
    );
    for key in ["verdict", "statistics", "tlc", "output", "counterexample"] {
        assert_eq!(observed[key], plain[key], "{key}");
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn unproved_captured_let_context_refuses_without_demanding_unused_actuals()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("dive-logical-let-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("Captured.tla");
    std::fs::write(
        &file,
        "---- MODULE Captured ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == LET Unused == PrintT(\"unused\") IN x'=x\n====\n",
    )?;
    std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
    for compat in [false, true] {
        let mut request = CheckRequest {
            file: file.clone(),
            workers: Some(1),
            compat_tlc: compat,
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let plain = check_file_engine(&request, Engine::Reference)?.to_json();
        assert_eq!(plain["verdict"], "success");
        assert_eq!(plain["output"], json!([]));
        request.tlc_coverage = true;
        let observed = check_file_engine(&request, Engine::Reference)?.to_json();
        assert_eq!(observed["verdict"], "unsupported");
        assert_eq!(observed["output"], json!([]));
        assert!(observed.to_string().contains("captured LET contexts"));
        assert!(observed["tlc_coverage"].get("actions").is_none());
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
