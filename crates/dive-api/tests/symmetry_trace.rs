//! Actual symmetry-checker counterexamples through the independent validator.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_api::trace::{SpecModel, TraceDoc, TraceEncoding, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, TemporalMode, check_file};

#[test]
fn concrete_symmetry_predecessors_validate_and_corruptions_are_rejected() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes/specs");
    for name in ["SymOrder", "SymViolation"] {
        let file = root.join(name).join(format!("{name}.tla"));
        let config = file.with_extension("cfg");
        for compat_tlc in [true, false] {
            let model = SpecModel::load(
                &file,
                &config,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )
            .expect("model");
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    let report = check_file(&CheckRequest {
                        trace_encoding: TraceEncoding::default(),
                        frontier_memory_bytes: None,
                        frontier_directory: None,
                        predecessor_memory_bytes: None,
                        predecessor_directory: None,
                        file: file.clone(),
                        config: Some(config.clone()),
                        library_paths: Vec::new(),
                        module_paths: Vec::new(),
                        compat_tlc,
                        store,
                        seed: Some(0),
                        fp: Some(0),
                        workers: Some(workers),
                        no_deadlock: false,
                        sequential: false,
                        chaos: Some(19),
                        temporal: TemporalMode::default(),
                    })
                    .expect("check");
                    let doc = TraceDoc::from_json(&report.to_json())
                        .expect("concrete counterexample document");
                    let controls = model.controls(&doc);
                    assert_eq!(
                        controls.positive.validation,
                        Validation::Valid,
                        "{name}: {:?}",
                        controls.failures()
                    );
                    assert!(
                        controls.failures().is_empty(),
                        "{name}: {:?}",
                        controls.failures()
                    );
                    assert_ne!(controls.mutants.len(), 0);
                }
            }
        }
    }
}
