//! Artifact admission must pass enclosing module analysis without a proof claim.
use dive_api::{TranslationRequest, translate_source};
fn module(body: &str) -> String {
    format!(
        "---- MODULE TranslationTest ----\nEXTENDS Integers,TLC\n(*\n--algorithm A\n{body}\n*)\n====\n"
    )
}
#[test]
fn process_artifact_is_validated_nonproof_in_both_profiles() {
    let source = module(
        "variable g=0; process P \\in{1,2} variables x=0; begin L: x:=1; g:=x; E: skip; end process; end algorithm;",
    );
    for compat_tlc in [false, true] {
        let report = translate_source(
            "TranslationTest.tla",
            &source,
            &TranslationRequest {
                compat_tlc,
                ..TranslationRequest::default()
            },
        );
        let doc = report.to_json();
        assert!(report.accepted(), "{doc}");
        assert!(doc.get("model_proved").is_none());
        assert!(doc.get("exhaustive").is_none());
        assert_eq!(doc["analysis"]["accepted"], true);
        assert_eq!(
            doc["artifact"]["processes"][0]["local_variables"],
            serde_json::json!(["x"])
        );
        assert!(
            doc["module_source"]
                .as_str()
                .is_some_and(|s| s.contains("g' = x'[self]"))
        );
    }
}
#[test]
fn artifact_validation_and_source_map() {
    let source = module("variable x=0; begin L: x:=1; E: skip; end algorithm;");
    let report = translate_source(
        "TranslationTest.tla",
        &source,
        &TranslationRequest::default(),
    );
    assert!(report.accepted(), "{}", report.to_json());
    let json = report.to_json();
    assert!(json.get("model_proved").is_none());
    assert!(json.get("exhaustive").is_none());
    assert_eq!(json["analysis"]["accepted"], true);
    assert_eq!(json["artifact"]["actions"], serde_json::json!(["L", "E"]));
    let request = TranslationRequest {
        compat_tlc: true,
        ..TranslationRequest::default()
    };
    let compat = translate_source("TranslationTest.tla", &source, &request).to_json();
    assert_eq!(compat["artifact"], json["artifact"]);
    assert_eq!(compat["profile"], "tlc");
}

#[test]
fn optimized_emitted_mapping_is_usable_before_checking_the_translated_module()
-> Result<(), Box<dyn std::error::Error>> {
    let source =
        module("variable x=0; begin Loop: while TRUE do x:=1-x; end while; end algorithm;");
    let directory = std::env::temp_dir().join(format!("dive-emitted-map-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("TranslationTest.tla");
    std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
    let mut previous = None;
    for compat_tlc in [false, true] {
        let report = translate_source(
            "TranslationTest.tla",
            &source,
            &TranslationRequest {
                compat_tlc,
                ..TranslationRequest::default()
            },
        );
        assert!(report.accepted(), "{}", report.to_json());
        let document = report.to_json();
        let artifact = &document["artifact"];
        let tla = artifact["tla"].as_str().ok_or("TLA body")?;
        let mappings = artifact["mappings"].as_array().ok_or("mappings")?;
        assert_eq!(mappings.len(), 1);
        let generated: dive_pluscal::ast::Span =
            serde_json::from_value(mappings[0]["generated"].clone())?;
        let original: dive_pluscal::ast::Span =
            serde_json::from_value(mappings[0]["original"].clone())?;
        let definition = tla
            .get(generated.start..generated.end)
            .ok_or("generated definition")?;
        assert!(definition.starts_with("Next =="), "{definition}");
        assert_eq!(
            source.get(original.start..original.end),
            Some("Loop: while TRUE do x:=1-x; end while;")
        );
        if let Some(previous) = previous.replace(artifact.clone()) {
            assert_eq!(*artifact, previous);
        }
        let emitted = document["module_source"].as_str().ok_or("emitted module")?;
        std::fs::write(&file, emitted)?;
        let checked = dive_api::check_file_engine(
            &dive_api::CheckRequest {
                file: file.clone(),
                compat_tlc,
                workers: Some(1),
                seed: Some(7),
                fp: Some(0),
                ..dive_api::CheckRequest::default()
            },
            dive_api::Engine::Core,
        )?
        .to_json();
        assert_eq!(checked["verdict"], "success", "{checked}");
        assert_eq!(checked["statistics"]["distinct"], 2);
        assert_eq!(checked["statistics"]["generated"], 3);
        assert_eq!(checked["statistics"]["depth"], 2);
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
#[test]
fn invalid_generated_name_is_not_an_artifact() {
    let source = module("variable x=0; begin L: x:=Unknown; E: skip; end algorithm;");
    let report = translate_source(
        "TranslationTest.tla",
        &source,
        &TranslationRequest::default(),
    );
    assert!(!report.accepted());
    assert_eq!(report.to_json()["stage"], "artifact-analysis");
    assert!(report.to_json().get("artifact").is_none());
    assert_eq!(report.exit_code(), 1);
}
#[test]
fn selection_refusals_remain_explicit() {
    for source in [module("variable Next=0; begin L: Next:=1; end algorithm;"),
        "---- MODULE TranslationTest ----\n(* --algorithm A begin L: skip; end algorithm *)\n(* --algorithm B begin E: skip; end algorithm *)\n====".into(),
        "---- MODULE TranslationTest ----\nS == \"--algorithm A\"\n====".into(),
        "---- MODULE TranslationTest ----\n\\* BEGIN TRANSLATION\n====".into()] {
        let report=translate_source("TranslationTest.tla",&source,&TranslationRequest::default());
        assert_eq!(report.exit_code(),3,"{}",report.to_json());
        assert!(!report.accepted()); assert!(report.to_json().get("model_proved").is_none());
    }
}

#[test]
fn long_module_terminator_and_multiline_expression_boundaries() {
    let source = module("variable x=0; begin L: x:=1; E: skip; end algorithm;")
        .replace("====", "====================");
    assert!(
        translate_source(
            "TranslationTest.tla",
            &source,
            &TranslationRequest::default()
        )
        .accepted()
    );
    let source = module("variable x=0; begin L: x:=1+\n 2; E: skip; end algorithm;");
    assert!(
        translate_source(
            "TranslationTest.tla",
            &source,
            &TranslationRequest::default()
        )
        .accepted()
    );
    let source = module("variable x=0; begin L: await /\\ TRUE\n /\\ x=0; E: skip; end algorithm;");
    let report = translate_source(
        "TranslationTest.tla",
        &source,
        &TranslationRequest::default(),
    );
    assert!(report.accepted(), "{}", report.to_json());
    assert!(report.to_json().get("model_proved").is_none());
}

#[test]
fn default_model_value_binding_and_label_options_are_data() {
    let source = module("variable x; begin x:=1; x:=2; end algorithm;");
    for compat in [false, true] {
        let request = TranslationRequest {
            compat_tlc: compat,
            ..TranslationRequest::default()
        };
        let doc = translate_source("TranslationTest.tla", &source, &request).to_json();
        assert_eq!(doc["accepted"], true, "{doc}");
        assert_eq!(
            doc["artifact"]["configuration"],
            "CONSTANT defaultInitValue = defaultInitValue\n"
        );
        assert_eq!(
            doc["artifact"]["actions"],
            serde_json::json!(["Lbl_1", "Lbl_2"])
        );
        assert!(doc.get("model_proved").is_none());
    }
    let source = module("variable x=0; begin x:=1; E: skip; end algorithm;");
    let mut request = TranslationRequest::default();
    request.limits.label_root = "Step".into();
    assert_eq!(
        translate_source("TranslationTest.tla", &source, &request).exit_code(),
        1
    );
    request.limits.insert_missing_labels = true;
    let doc = translate_source("TranslationTest.tla", &source, &request).to_json();
    assert_eq!(doc["accepted"], true, "{doc}");
    assert_eq!(
        doc["artifact"]["actions"],
        serde_json::json!(["Step1", "E"])
    );
}

#[test]
fn process_fairness_options_remain_validated_nonproof_artifacts() {
    let source = module(
        "variable g=0; process P \\in{1,2} variable x=0; begin L: + x:=1; E: - g:=x; end process; end algorithm;",
    );
    for (option, present, absent) in [
        (
            dive_api::TranslationFairness::Weak,
            "WF_vars((pc[self] #",
            "WF_vars(Next)",
        ),
        (
            dive_api::TranslationFairness::Strong,
            "SF_vars((pc[self] #",
            "SF_vars(L(self))",
        ),
        (
            dive_api::TranslationFairness::Next,
            "WF_vars(Next)",
            "SF_vars(L(self))",
        ),
        (
            dive_api::TranslationFairness::None,
            "Spec == Init",
            "WF_vars(",
        ),
    ] {
        for compat in [false, true] {
            let mut request = TranslationRequest {
                compat_tlc: compat,
                ..TranslationRequest::default()
            };
            request.limits.fairness = option;
            let report = translate_source("TranslationTest.tla", &source, &request);
            let doc = report.to_json();
            assert!(report.accepted(), "{doc}");
            let artifact = doc["artifact"]["tla"].as_str().unwrap_or_default();
            assert!(artifact.contains(present), "{artifact}");
            assert!(!artifact.contains(absent), "{artifact}");
            assert_eq!(doc["analysis"]["accepted"], true);
            assert!(doc.get("model_proved").is_none());
            assert!(doc.get("exhaustive").is_none());
        }
    }
}

#[test]
fn multiple_process_storage_and_domains_remain_nonproof() {
    let source = module(
        "variable g=0; process P=1 variable x=0; begin L: x:=1; E: g:=x; end process; fair process Q \\in{2,3} variable y=0; begin M: + y:=2; F: - g:=y; end process; end algorithm;",
    );
    for compat_tlc in [false, true] {
        let request = TranslationRequest {
            compat_tlc,
            ..TranslationRequest::default()
        };
        let doc = translate_source("TranslationTest.tla", &source, &request).to_json();
        assert_eq!(doc["accepted"], true, "{doc}");
        assert_eq!(doc["analysis"]["accepted"], true);
        assert!(doc.get("model_proved").is_none());
        assert!(doc.get("exhaustive").is_none());
        assert_eq!(
            doc["artifact"]["processes"].as_array().map(Vec::len),
            Some(2)
        );
        for changed in [
            source.replace("\\in{2,3}", "\\in{1,2}"),
            source.replace("y:=2", "y:=x"),
        ] {
            let translated = translate_source("TranslationTest.tla", &changed, &request).to_json();
            assert_eq!(translated["accepted"], true, "{translated}");
            assert_eq!(translated["analysis"]["accepted"], true);
            assert!(translated.get("model_proved").is_none());
            assert!(translated.get("exhaustive").is_none());
        }
    }
}

#[test]
fn invalid_primary_macro_generation_is_not_public_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/macro");
    let mut controls = 0;
    let mut selected = Vec::new();
    for directory in ["", "extra", "process", "scope"] {
        let directory = base.join(directory);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("fixtures/manifest.json"))?)?;
        for row in manifest["rows"].as_array().ok_or("rows")? {
            selected.push((directory.clone(), row.clone()));
        }
    }
    for (base, row) in selected {
        if row["sany"]["status"] != "rejected" {
            continue;
        }
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("input")?),
        )?;
        for compat_tlc in [false, true] {
            let request = TranslationRequest {
                compat_tlc,
                ..TranslationRequest::default()
            };
            let report = translate_source(row["input"].as_str().ok_or("file")?, &source, &request);
            let doc = report.to_json();
            assert!(!report.accepted(), "{}", doc);
            assert!(matches!(report.exit_code(), 1 | 3), "{}", doc);
            assert!(doc.get("model_proved").is_none());
            assert!(doc.get("exhaustive").is_none());
            assert!(doc.get("artifact").is_none());
            assert!(doc.get("module_source").is_none());
            controls += 1;
        }
    }
    assert_eq!(controls, 54);
    Ok(())
}

#[test]
fn free_macro_actual_name_is_validated_at_enclosing_module_boundary() {
    for compat_tlc in [false, true] {
        let source = module(
            "variable x=0; macro Put(v,a) begin with t=a do v:=t; end with; end macro; begin L: Put(x,t); E: skip; end algorithm;",
        );
        let report = translate_source(
            "TranslationTest.tla",
            &source,
            &TranslationRequest {
                compat_tlc,
                ..TranslationRequest::default()
            },
        );
        assert!(!report.accepted());
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.to_json()["stage"], "artifact-analysis");
        assert!(report.to_json().get("artifact").is_none());
    }
}

#[test]
fn invalid_primary_procedure_outputs_are_never_public_artifacts()
-> Result<(), Box<dyn std::error::Error>> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/procedure");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/manifest.json"))?)?;
    let mut controls = 0;
    for row in manifest["rows"].as_array().ok_or("rows")? {
        if row["sany"]["status"] != "rejected" {
            continue;
        }
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("input")?),
        )?;
        for compat_tlc in [false, true] {
            let report = translate_source(
                row["input"].as_str().ok_or("file")?,
                &source,
                &TranslationRequest {
                    compat_tlc,
                    ..TranslationRequest::default()
                },
            );
            assert!(!report.accepted());
            assert!(matches!(report.exit_code(), 1 | 3));
            let doc = report.to_json();
            assert!(doc.get("model_proved").is_none());
            assert!(doc.get("exhaustive").is_none());
            assert!(doc.get("artifact").is_none());
            assert!(doc.get("module_source").is_none());
            controls += 1;
        }
    }
    assert_eq!(controls, 8);
    Ok(())
}

#[test]
fn invalid_primary_graph_outputs_are_never_public_artifacts()
-> Result<(), Box<dyn std::error::Error>> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/procedure-graph");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/manifest.json"))?)?;
    let mut controls = 0;
    for row in manifest["rows"].as_array().ok_or("rows")? {
        if row["sany"]["status"] != "rejected" {
            continue;
        }
        let source = std::fs::read_to_string(
            base.join("cases")
                .join(row["input"].as_str().ok_or("input")?),
        )?;
        for compat_tlc in [false, true] {
            let report = translate_source(
                row["input"].as_str().ok_or("file")?,
                &source,
                &TranslationRequest {
                    compat_tlc,
                    ..TranslationRequest::default()
                },
            );
            assert!(!report.accepted());
            assert!(matches!(report.exit_code(), 1 | 3));
            let doc = report.to_json();
            assert!(doc.get("model_proved").is_none());
            assert!(doc.get("exhaustive").is_none());
            assert!(doc.get("artifact").is_none());
            assert!(doc.get("module_source").is_none());
            controls += 1;
        }
    }
    assert_eq!(controls, 16);
    Ok(())
}

#[test]
fn existing_translation_uses_its_original_slot_before_analysis()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE TranslationTest ----\nEXTENDS Integers,TLC\n(* --algorithm A variable x=0; begin L:x:=Later; E:skip; end algorithm; *)\nLater == 1\n\\* BEGIN TRANSLATION\nold generated body\n\\* END TRANSLATION\nAfter == x\n====\n";
    for compat_tlc in [false, true] {
        let request = TranslationRequest {
            compat_tlc,
            ..TranslationRequest::default()
        };
        let translated = translate_source("TranslationTest.tla", source, &request);
        let doc = translated.to_json();
        assert!(translated.accepted(), "{doc}");
        let module = doc["module_source"].as_str().ok_or("module")?;
        assert!(
            module.find("Later ==").ok_or("Later")?
                < module.find("VARIABLES").ok_or("variables")?
        );
        assert!(
            module.find("VARIABLES").ok_or("variables")?
                < module.find("After ==").ok_or("After")?
        );
        assert!(!module.contains("old generated body"));
        assert_eq!(module.matches("BEGIN TRANSLATION").count(), 1);
        let rewritten = dive_api::rewrite_source(
            "TranslationTest.tla",
            source,
            &dive_api::RewriteRequest {
                translation: request,
                editing: dive_api::RewriteOptions {
                    checksums: dive_api::ChecksumPolicy::BestEffort,
                    replace_modified: true,
                    ..dive_api::RewriteOptions::default()
                },
            },
        );
        assert!(rewritten.accepted(), "{}", rewritten.to_json());
        assert_eq!(rewritten.to_json()["analysis"]["accepted"], true);
    }
    Ok(())
}

#[test]
fn pinned_controls_preserve_translation_and_analysis_stages()
-> Result<(), Box<dyn std::error::Error>> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/port-controls");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/manifest.json"))?)?;
    let mut selected = 0;
    for row in manifest["rows"].as_array().ok_or("rows")? {
        let id = row["id"].as_str().ok_or("id")?;
        if ![
            "ApiAwait",
            "ApiOverlap",
            "ApiForeignRead",
            "PrintWithoutTLC",
        ]
        .contains(&id)
        {
            continue;
        }
        let input = row["input"].as_str().ok_or("input")?;
        let source = std::fs::read_to_string(base.join("cases").join(input))?;
        assert_eq!(row["primary"]["status"], "translated");
        let accepted = row["sany"]["status"]
            .as_str()
            .ok_or("missing pinned SANY status")?
            == "accepted";
        for compat_tlc in [false, true] {
            let report = translate_source(
                input,
                &source,
                &TranslationRequest {
                    compat_tlc,
                    ..TranslationRequest::default()
                },
            );
            let doc = report.to_json();
            assert_eq!(report.accepted(), accepted, "{id}: {doc}");
            assert_eq!(doc["stage"], "artifact-analysis");
            assert!(doc.get("model_proved").is_none());
            assert!(doc.get("exhaustive").is_none());
            if !accepted {
                assert_eq!(report.exit_code(), 1);
                assert!(doc.get("artifact").is_none());
                assert!(doc.get("module_source").is_none());
                if id == "PrintWithoutTLC" {
                    assert!(doc.to_string().contains("PrintT"));
                }
            }
        }
        selected += 1;
    }
    assert_eq!(selected, 4);
    Ok(())
}
