//! Core's product vertex capacity and the file routes' reported bounds.
#![allow(clippy::expect_used, clippy::panic, reason = "finite test assertions")]
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use std::path::PathBuf;

use dive_api::{CheckRequest, TemporalMode};
use dive_runtime::CheckOptions;

const SOURCE: &str = "---- MODULE VertexLimit ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";

/// Core's product has no vertex ceiling of its own: the two-state base
/// product of this word is built and its violation validated. Caller-supplied
/// vertex ceilings belonged to the retired reference final route (#791).
#[test]
fn core_product_has_no_vertex_ceiling_and_validates_the_word() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let run = core_final::check(SOURCE, CONFIG, &options).expect("verdict");
        let core_final::Outcome::Violation(word) = run.outcome() else {
            panic!(
                "implicit FALSE stuttering violates GF x: {:?}",
                run.outcome()
            );
        };
        assert_eq!(
            word.validation().validation,
            dive_api::trace::Validation::Valid
        );
        assert_eq!(word.candidate().exploration.stats.distinct, 2);
        let json = run.report().to_json();
        assert_eq!(
            json["options"]["temporal_bounds"]["product_vertices"],
            serde_json::json!(usize::MAX)
        );
    }
}

struct Files(PathBuf);
impl Files {
    fn new(config: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "dive-product-vertices-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir(&directory).expect("private test directory");
        let file = directory.join("VertexLimit.tla");
        // A genuine temporal holding claim, not a safety-lowered []state claim.
        let source = SOURCE.replace("[]<>x", "[]<>(x \\in {FALSE, TRUE})");
        std::fs::write(&file, source).expect("module");
        std::fs::write(file.with_extension("cfg"), config).expect("config");
        Self(file)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        if let Some(directory) = self.0.parent() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

#[test]
fn file_routes_report_machine_capacity_without_changing_other_owners() {
    for config in [CONFIG, "INIT Missing\nNEXT Next\nPROPERTY P\n"] {
        let files = Files::new(config);
        for compat_tlc in [false, true] {
            for temporal in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
                let report = dive_api::check_file_engine(
                    &CheckRequest {
                        file: files.0.clone(),
                        compat_tlc,
                        temporal,
                        seed: Some(0),
                        fp: Some(0),
                        workers: Some(1),
                        ..CheckRequest::default()
                    },
                    dive_api::Engine::Core,
                )
                .expect("real file report");
                let json = report.to_json();
                assert_eq!(json["schema_version"], 3);
                if config != CONFIG && matches!(temporal, TemporalMode::Ordinary) {
                    // Ordinary genuine binding failures retain the existing
                    // safety report; no temporal role was established.
                    let result = report.result().expect("ordinary binding result");
                    assert!(result.binding_model_error);
                    assert_ne!(report.verdict(), "success");
                    assert!(report.temporal_result().is_none());
                    assert_eq!(json["check_kind"], "safety");
                    for name in ["temporal_bounds", "lncheck", "lasso", "tlc_exact_temporal"] {
                        assert!(json["options"].get(name).is_none(), "{name}");
                    }
                    assert!(json.get("execution").is_none());
                    assert!(json["statistics"].is_null());
                    assert!(json["counterexample"].is_null());
                    continue;
                }
                if config == CONFIG && matches!(temporal, TemporalMode::FinalShort) {
                    // Only the retired reference engine ran bounded final
                    // checking (#791, #910).
                    refused::assert_refused(&json, report.exit_code(), refused::FINAL_SHORT);
                }
                let bounds = &json["options"]["temporal_bounds"];
                // Core's ordinary temporal checking has no product or
                // acceptance caps of its own; the file-route caps remain on
                // the final-mode route.
                let (edges, steps) = if matches!(temporal, TemporalMode::Ordinary) {
                    (serde_json::json!(usize::MAX), serde_json::json!(usize::MAX))
                } else {
                    (serde_json::json!(8_000_000), serde_json::json!(32_000_000))
                };
                assert_eq!(
                    bounds["product_vertices"],
                    serde_json::json!(usize::MAX),
                    "config={config:?}, compat_tlc={compat_tlc}, temporal={temporal:?}"
                );
                assert_eq!(bounds["product_edges"], edges);
                assert_eq!(bounds["product_steps"], steps);
                assert_eq!(bounds["acceptance_buffer_bytes"], edges);
                assert_eq!(bounds["acceptance_materialization_work"], steps);
                assert_eq!(bounds["cell_limits"]["graph_labels"]["kind"], "run-budgets");
                for owner in ["attribution_rows", "word_truth"] {
                    assert_eq!(bounds["cell_limits"][owner]["cells"], edges);
                }
                if config == CONFIG && matches!(temporal, TemporalMode::FinalShort) {
                    continue;
                }
                if config == CONFIG {
                    assert_eq!(
                        json["options"]["lncheck"],
                        if !compat_tlc && matches!(temporal, TemporalMode::Ordinary) {
                            "deterministic"
                        } else {
                            "final"
                        }
                    );
                    assert_eq!(report.verdict(), "success");
                    assert_eq!(json["statistics"]["distinct"], 2);
                    assert_eq!(json["execution"]["capture_completed"], true);
                    assert_eq!(json["execution"]["exhausted"], true);
                } else {
                    // Binding failed before any periodic schedule was installed.
                    assert_eq!(json["options"]["lncheck"], "final");
                    assert_ne!(report.verdict(), "success");
                    assert_eq!(json["execution"]["stage"], "binding");
                    assert_eq!(json["execution"]["capture_completed"], false);
                    assert!(json["statistics"].is_null());
                    assert!(json["counterexample"].is_null());
                }
            }
        }
    }
}
