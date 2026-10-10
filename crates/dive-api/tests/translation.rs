//! Artifact admission must pass enclosing module analysis without a proof claim.
use dive_api::{TranslationRequest, translate_source};
fn module(body: &str) -> String {
    format!(
        "---- MODULE TranslationTest ----\nEXTENDS Integers,TLC\n(*\n--algorithm A\n{body}\n*)\n====\n"
    )
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
fn long_module_terminator_and_multiline_expression_limits() {
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
