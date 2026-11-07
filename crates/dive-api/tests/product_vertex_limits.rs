//! File capacity and caller-supplied product vertex ceilings have distinct owners.
#![allow(clippy::expect_used, clippy::panic, reason = "finite test assertions")]

use std::path::PathBuf;
use std::sync::Arc;

use dive_api::temporal::{self, Error, Outcome, Stage};
use dive_api::{CheckRequest, TemporalMode};
use dive_check::CheckOptions;
use dive_liveness::analyze::{AnalysisError, GraphLabelPolicy, Limits};
use dive_liveness::product::{AcceptancePolicy, ProductError};

const SOURCE: &str = "---- MODULE VertexLimit ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";

#[test]
fn every_final_entrypoint_preserves_custom_vertex_boundaries_and_default() {
    let analysis =
        dive_sema::analyze_source("VertexLimit.tla", SOURCE, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(CONFIG).into_config().expect("config");
    assert_eq!(Limits::default().product.vertices, 1_000_000);
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        // FG extraction leaves this two-state behavior graph in the base product.
        for vertices in [0, 1, 2, 1_000_000] {
            let mut limits = Limits::default();
            limits.product.vertices = vertices;
            let results = [
                temporal::check_final_with_evidence(&ir, &config, &options, limits),
                temporal::check_final_with_policy(
                    &ir,
                    &config,
                    &options,
                    limits,
                    AcceptancePolicy::compact_default(),
                ),
                temporal::check_final_with_policies(
                    &ir,
                    &config,
                    &options,
                    limits,
                    AcceptancePolicy::compact_default(),
                    GraphLabelPolicy::NoCellLimit,
                ),
            ];
            for result in results {
                if vertices < 2 {
                    let failure = result.expect_err("exact supplied ceiling must refuse");
                    assert!(matches!(
                        failure.error(),
                        Error::Analysis(AnalysisError::Product(ProductError::Limit("vertices")))
                    ));
                    assert_eq!(
                        failure.stage(),
                        Stage::Analysis(dive_liveness::analyze::Stage::Product)
                    );
                    assert_eq!(failure.capture_completed(), Some(true));
                    assert_eq!(failure.exploration().expect("capture").stats.distinct, 2);
                    assert_eq!(
                        failure
                            .configuration()
                            .expect("configured refusal")
                            .limits
                            .product
                            .vertices,
                        vertices
                    );
                } else {
                    let report = result.expect("exact two vertices fit");
                    assert_eq!(
                        report
                            .configuration()
                            .expect("actual configuration")
                            .limits
                            .product
                            .vertices,
                        vertices
                    );
                    let Outcome::Violation(word) = report.outcome() else {
                        panic!("implicit FALSE stuttering violates GF x");
                    };
                    assert_eq!(
                        word.validation().validation,
                        dive_api::trace::Validation::Valid
                    );
                }
            }
            let legacy = temporal::check_final(&ir, &config, &options, limits);
            if vertices < 2 {
                assert!(matches!(
                    legacy,
                    Err(Error::Analysis(AnalysisError::Product(
                        ProductError::Limit("vertices")
                    )))
                ));
            } else {
                assert!(matches!(
                    legacy.expect("legacy exact boundary").outcome(),
                    Outcome::Violation(_)
                ));
            }
        }
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
                    dive_api::Engine::Reference,
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
                let bounds = &json["options"]["temporal_bounds"];
                assert_eq!(
                    bounds["product_vertices"],
                    serde_json::json!(usize::MAX),
                    "config={config:?}, compat_tlc={compat_tlc}, temporal={temporal:?}"
                );
                assert_eq!(bounds["product_edges"], 8_000_000);
                assert_eq!(bounds["product_steps"], 32_000_000);
                assert_eq!(bounds["acceptance_buffer_bytes"], 8_000_000);
                assert_eq!(bounds["acceptance_materialization_work"], 32_000_000);
                assert_eq!(bounds["cell_limits"]["graph_labels"]["kind"], "run-budgets");
                for owner in ["attribution_rows", "word_truth"] {
                    assert_eq!(bounds["cell_limits"][owner]["cells"], 8_000_000);
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
