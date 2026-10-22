//! JSON and human projections preserve real binding output and capture priority.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "deterministic report fixtures"
)]
use super::*;

fn report(tlc: bool, capture: Option<bool>) -> CheckReport {
    let (opts, failure) = crate::temporal::binding_controls::writer_failure(tlc, capture);
    CheckReport {
        checkpoint_request: None,
        checkpoint_receipt: None,
        file: "BindingWriter.tla".into(),
        config: "BindingWriter.cfg".into(),
        profile: if tlc { Profile::Tlc } else { Profile::Dive },
        opts,
        symmetry_liveness_explicit: false,
        seed_given: true,
        fp_given: true,
        workers_requested: 1,
        temporal: TemporalMode::FinalShort,
        budget_sources: ("explicit", "none"),
        memory_requested_source: "none",
        memory_samples: None,
        temporal_configuration: crate::temporal::Configuration {
            limits: dive_liveness::analyze::Limits::default(),
            acceptance: dive_liveness::product::AcceptancePolicy::compact_default(),
            graph_labels: dive_liveness::analyze::GraphLabelPolicy::Cells(
                dive_liveness::analyze::Limits::default().atom_cells,
            ),
        },
        phase: Phase::TemporalError(Box::new(failure)),
    }
}

#[test]
fn initial_binding_report_preserves_actual_messages_and_no_search() {
    for tlc in [false, true] {
        let report = report(tlc, None);
        let Phase::TemporalError(failure) = &report.phase else {
            unreachable!("fixture creates a typed binding failure");
        };
        let prints: Vec<_> = failure
            .binding_output()
            .iter()
            .filter_map(|message| {
                if let dive_model::Message::Print(text) = message {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect();
        let warnings: Vec<_> = failure
            .binding_output()
            .iter()
            .filter_map(|message| {
                if let dive_model::Message::Warning { code, message } = message {
                    Some(json!({"code": code.code(), "message": message}))
                } else {
                    None
                }
            })
            .collect();
        assert_ne!(prints, [] as [String; 0]);
        assert_ne!(warnings, [] as [Value; 0]);
        let json = report.to_json();
        assert_eq!(json["verdict"], "stopped");
        assert_eq!(json["stopped"]["phase"], "model-binding");
        assert_eq!(json["execution"]["stage"], "binding");
        assert_eq!(json["execution"]["capture_completed"], false);
        assert_eq!(json["resources"]["checking_started"], false);
        assert_eq!(json["options"]["workers"]["used"], 0);
        assert!(json["statistics"].is_null());
        assert!(json["counterexample"].is_null());
        assert!(json["tlc"].is_null());
        assert_eq!(json["output"], json!(prints));
        assert_eq!(json["warnings"], json!(warnings));
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), exit::STOPPED);
        assert_eq!(report.exit_code_for(ExitConvention::Native), exit::STOPPED);
        let human = report.render_human();
        assert!(human.contains("Execution stage: binding"));
        assert!(human.contains("1 worker(s) requested, 0 used"));
        for line in &prints {
            assert_eq!(human.lines().filter(|actual| *actual == line).count(), 1);
        }
        for warning in &warnings {
            let line = format!("Warning: {}", warning["message"].as_str().expect("warning"));
            assert_eq!(human.lines().filter(|actual| *actual == line).count(), 1);
        }
    }
}

#[test]
fn complete_and_prefix_binding_reports_keep_only_actual_capture_messages() {
    for tlc in [false, true] {
        for complete in [false, true] {
            let report = report(tlc, Some(complete));
            let Phase::TemporalError(failure) = &report.phase else {
                unreachable!("fixture creates a typed validation failure");
            };
            let capture = failure.exploration().expect("actual capture");
            assert!(
                !failure.binding_output().is_empty(),
                "fresh binding also produced output"
            );
            assert_ne!(capture.output, [] as [String; 0]);
            let json = report.to_json();
            assert_eq!(json["verdict"], "stopped");
            assert_eq!(json["stopped"]["phase"], "trace-validation");
            assert_eq!(json["execution"]["stage"], "validation");
            assert_eq!(json["execution"]["capture_completed"], complete);
            assert_eq!(json["statistics"]["generated"], capture.stats.generated);
            assert_eq!(json["statistics"]["distinct"], capture.stats.distinct);
            assert_eq!(json["statistics"]["depth"], capture.stats.depth);
            assert_eq!(json["output"], json!(capture.output));
            let warnings: Vec<_> = capture
                .warnings
                .iter()
                .map(|(code, message)| json!({"code": code, "message": message}))
                .collect();
            assert_eq!(json["warnings"], json!(warnings));
            assert!(json["resources"]["checking_started"].is_null());
            assert_eq!(json["resources"]["validation"]["checks_passed"], 0);
            assert!(json["counterexample"].is_null());
            assert!(json["tlc"].is_null());
            assert_eq!(report.exit_code(), exit::STOPPED);
            let human = report.render_human();
            assert!(human.contains("Execution stage: validation"));
            for line in &capture.output {
                assert_eq!(
                    human.lines().filter(|actual| *actual == line).count(),
                    capture
                        .output
                        .iter()
                        .filter(|actual| *actual == line)
                        .count(),
                );
            }
            for (_, warning) in &capture.warnings {
                let line = format!("Warning: {warning}");
                assert_eq!(
                    human.lines().filter(|actual| *actual == line).count(),
                    capture
                        .warnings
                        .iter()
                        .filter(|(_, actual)| actual == warning)
                        .count(),
                );
            }
        }
    }
}
