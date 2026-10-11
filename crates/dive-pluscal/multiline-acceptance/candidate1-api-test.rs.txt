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
        assert_eq!(doc["model_proved"], false);
        assert_eq!(doc["exhaustive"], false);
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
    assert_eq!(json["model_proved"], false);
    assert_eq!(json["exhaustive"], false);
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
        assert!(!report.accepted()); assert_eq!(report.to_json()["model_proved"],false);
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
    assert_eq!(report.exit_code(), 3);
    assert!(
        report.to_json()["diagnostic"]["message"]
            .as_str()
            .is_some_and(|text| text.contains("multiline"))
    );
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
        assert_eq!(doc["model_proved"], false);
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
            assert_eq!(doc["model_proved"], false);
            assert_eq!(doc["exhaustive"], false);
        }
    }
}

#[test]
fn multiple_process_storage_and_refusals_remain_nonproof() {
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
        assert_eq!(doc["model_proved"], false);
        assert_eq!(doc["exhaustive"], false);
        assert_eq!(
            doc["artifact"]["processes"].as_array().map(Vec::len),
            Some(2)
        );
        for changed in [
            source.replace("\\in{2,3}", "\\in{1,2}"),
            source.replace("y:=2", "y:=x"),
        ] {
            let refusal = translate_source("TranslationTest.tla", &changed, &request).to_json();
            assert_eq!(refusal["result"], "unsupported", "{refusal}");
            assert_eq!(refusal["model_proved"], false);
            assert!(refusal.get("artifact").is_none());
        }
    }
}

#[test]
fn invalid_primary_macro_generation_is_not_public_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-pluscal/macro-acceptance");
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
            assert_eq!(doc["model_proved"], false);
            assert_eq!(doc["exhaustive"], false);
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
        .join("../dive-pluscal/procedure-acceptance");
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
            assert_eq!(doc["model_proved"], false);
            assert_eq!(doc["exhaustive"], false);
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
        .join("../dive-pluscal/procedure-graph-acceptance");
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
            assert_eq!(doc["model_proved"], false);
            assert_eq!(doc["exhaustive"], false);
            assert!(doc.get("artifact").is_none());
            assert!(doc.get("module_source").is_none());
            controls += 1;
        }
    }
    assert_eq!(controls, 16);
    Ok(())
}
