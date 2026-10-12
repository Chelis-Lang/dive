//! Original temporal Action names stay separate from claims and derived labels.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::tlc_temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_liveness::phase_graph::Limits;
use std::{path::PathBuf, sync::Arc};
fn base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-final-property-origins")
}
fn names(text: &str) -> Vec<String> {
    text.split(';')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn inputs(name: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let path = base().join("models").join(name).join(format!("{name}.tla"));
    let a = dive_sema::analyze_file(&path, &dive_sema::Options::default()).expect("source");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg =
        dive_config::parse(&std::fs::read_to_string(path.with_extension("cfg")).expect("cfg"))
            .into_config()
            .expect("config");
    (ir, cfg)
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete original, derived and configured field selection"
)]
fn primary_action_names_match_binder_projection_execution_and_schema_two() {
    let capture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(base().join("final-projection/capture.json")).expect("primary"),
    )
    .expect("JSON");
    let mut complete = 0;
    let mut refused = 0;
    for row in capture["rows"].as_array().expect("selection") {
        let name = row["name"].as_str().expect("name");
        let path = base().join("models").join(name).join(format!("{name}.tla"));
        let request = dive_api::CheckRequest {
            file: path,
            compat_tlc: true,
            seed: Some(0),
            fp: Some(0),
            workers: Some(4),
            temporal: dive_api::TemporalMode::FinalTlc,
            ..Default::default()
        };
        if row["exit"] != 0 {
            refused += 1;
            let report = dive_api::check_file(&request)
                .expect("typed report")
                .to_json();
            assert_eq!(report["verdict"], "unsupported", "{name}");
            assert!(
                report["execution"]["expanded_property_names"].is_null(),
                "{name} no completed identity credit"
            );
            continue;
        }
        complete += 1;
        let fields = &row["fields"];
        let original = names(fields["action_names"].as_str().expect("Action vector"));
        let configured = names(
            fields["configured_names"]
                .as_str()
                .expect("configured vector"),
        );
        let derived = names(fields["role_names"].as_str().expect("derived vector"));
        let (ir, cfg) = inputs(name);
        let model =
            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("binder");
        assert_eq!(
            model.implied_temporal_action_names(),
            original,
            "{name} original Action names"
        );
        assert_eq!(
            model
                .configured_properties
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>(),
            configured
        );
        assert_eq!(
            model
                .implied_temporals
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>(),
            derived
        );
        let plan =
            dive_liveness::tlc_plan::lower_model(&model, dive_liveness::formula::Limits::default())
                .expect("projection");
        assert_eq!(
            plan.projection.properties, derived,
            "{name} planning labels unchanged"
        );
        for workers in [1, 3, 4, 8] {
            let options = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let report = tlc_temporal::check_final(&ir, &cfg, &options, Limits::default())
                .expect("actual final");
            let execution = match report.outcome() {
                Outcome::Holds(e) => Some(&e.execution),
                Outcome::Violation(v) => {
                    assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
                    assert_eq!(
                        v.document().properties,
                        configured,
                        "{name} original witness claims unchanged"
                    );
                    Some(&v.candidate().evidence.execution)
                }
                Outcome::Safety(_) => {
                    assert_eq!(original, Vec::<String>::new());
                    None
                }
            };
            if let Some(e) = execution {
                assert_eq!(
                    e.expanded_property_names, original,
                    "{name} committed origins"
                );
                assert_eq!(e.role_names, derived, "{name} derived labels unchanged");
            }
        }
        let json = dive_api::check_file(&request).expect("report").to_json();
        if original.is_empty() {
            assert_eq!(json["execution"]["route"], "bounded-safety-only");
            assert!(json["execution"]["expanded_property_names"].is_null());
        } else {
            assert_eq!(
                json["execution"]["configured_property_names"],
                serde_json::json!(configured)
            );
            assert_eq!(
                json["execution"]["expanded_property_names"],
                serde_json::json!(original)
            );
            assert_eq!(json["execution"]["role_names"], serde_json::json!(derived));
        }
    }
    assert_eq!((complete, refused), (16, 2));
}
#[test]
fn edited_role_quantity_refuses_before_executing_stale_origins() {
    let (ir, cfg) = inputs("TfBooleanFails");
    let mut model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    assert_eq!(model.implied_temporal_action_names(), ["P", "P"]);
    model.implied_temporals.pop();
    let error = model
        .validate_context()
        .expect_err("public role edit invalidates binder origins");
    assert!(error.message().contains("property-origin metadata"));
    assert_eq!(model.take_output(), Vec::<String>::new());
    // Legacy diagnostics project formulas/labels, not private origin authority.
    let diagnostic =
        dive_liveness::tlc_plan::lower_model(&model, dive_liveness::formula::Limits::default())
            .expect("legacy diagnostic construction remains compatible");
    let _public_literal = dive_liveness::tlc_plan::BoundProjection {
        projection: diagnostic.projection,
        inputs: diagnostic.inputs,
        configured_properties: diagnostic.configured_properties,
    };
    let refused = dive_liveness::phase_l::Session::new(
        &model,
        0,
        1,
        dive_liveness::phase_l::Limits::default(),
    )
    .err()
    .expect("origin-bearing execution refuses stale role quantity");
    assert!(refused.to_string().contains("property-origin metadata"));
    assert_eq!(model.take_output(), Vec::<String>::new());
}
