//! Core DFID preserves finite visitation certificates and actual concrete words.
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/core_dfid.json");
use dive_api::{DfidRequest, Engine, dfid_file_engine};
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn request(name: &str, compat_tlc: bool, exact: bool, seed: i64) -> DfidRequest {
    DfidRequest {
        file: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../dive-check/tests/fixtures/dfid/{name}.tla")),
        compat_tlc,
        seed: Some(7),
        max_level: 6,
        worker_seeds: Some(vec![seed; 6]),
        exact,
        ..Default::default()
    }
}

#[test]
fn native_deterministic_search_matches_reference_certificates_counts_and_names() -> Result {
    for compat in [false, true] {
        for exact in [false, true] {
            for seed in [0, 7] {
                for name in [
                    "DfidBound",
                    "DfidCycle",
                    "DfidRootsMixed",
                    "DfidDuplicate",
                    "DfidConstrained",
                    "DfidActionPrecedence",
                    "DfidInitialPrecedence",
                    "DfidInitialIncomplete",
                    "DfidDeadlock",
                ] {
                    let request = request(name, compat, exact, seed);
                    let reference =
                        dive_api::dfid_file_engine(&request, dive_api::Engine::Core)?.to_json();
                    let core = dfid_file_engine(&request, Engine::Core)?.to_json();
                    assert_eq!(core["engine"], "core", "{name}: {core}");
                    assert_eq!(core["engine_requested"], "core");
                    assert!(core["engine_fallback"].is_null());
                    assert_eq!(core["options"]["execution_scope"], "compiled-plan-dfid");
                    for field in [
                        "result",
                        "tlc_code",
                        "exit_code",
                        "model_proved",
                        "exhaustive",
                        "statistics",
                        "termination",
                        "violation_validated",
                    ] {
                        agreed::check_eq(
                            AGREED,
                            "native_deterministic_search_matches_reference_certificates_counts_and_names/0",
                            &(core[field]),
                            &(reference[field]),
                        );
                    }
                    if core["result"] == "violation" {
                        assert_eq!(
                            core["counterexample"]["validation"]["result"], "valid",
                            "{name}: {core}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn native_depth_exhaustion_remains_incomplete_on_the_default_core_engine() -> Result {
    for compat in [false, true] {
        for depth in [0, 1, 2, 4] {
            let request = DfidRequest {
                max_level: depth,
                ..request("DfidBound", compat, true, 0)
            };
            let report = dive_api::dfid_file(&request)?.to_json();
            assert_eq!(report["engine"], "core", "{report}");
            assert_eq!(report["engine_requested"], "core");
            assert!(report["engine_fallback"].is_null());
            assert_eq!(report["result"], "incomplete");
            assert_eq!(report["model_proved"], false);
            assert_eq!(report["termination"], "depth-bound");
        }
    }
    Ok(())
}

#[test]
fn enumerated_compound_batches_match_pinned_counts_and_actual_words() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-finite-dfid");
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/core-finite-dfid/oracle.json"))?;
    for (name, expected) in oracle["cases"].as_object().ok_or("oracle cases")? {
        for compat_tlc in [false, true] {
            for exact in [false, true] {
                let request = DfidRequest {
                    file: root.join(format!("{name}.tla")),
                    compat_tlc,
                    exact,
                    seed: Some(7),
                    max_level: 6,
                    worker_seeds: Some(vec![0; 6]),
                    ..Default::default()
                };
                let reference =
                    dive_api::dfid_file_engine(&request, dive_api::Engine::Core)?.to_json();
                {
                    let engine = Engine::Core;
                    let actual = dfid_file_engine(&request, engine)?.to_json();
                    assert_eq!(actual["engine"], "core", "{name}: {actual}");
                    for field in [
                        "result",
                        "tlc_code",
                        "statistics",
                        "model_proved",
                        "termination",
                    ] {
                        agreed::check_eq(
                            AGREED,
                            "enumerated_compound_batches_match_pinned_counts_and_actual_words/0",
                            &(actual[field]),
                            &(reference[field]),
                        );
                    }
                    for field in ["distinct", "generated", "level", "initial_candidates"] {
                        assert_eq!(
                            actual["statistics"][field], expected[field],
                            "{name}/{field}: {actual}"
                        );
                    }
                    assert_eq!(actual["result"], expected["result"]);
                    assert_eq!(actual["tlc_code"], expected["tlc_code"]);
                    if compat_tlc {
                        assert_eq!(actual["exit_code"], expected["tlc_exit"]);
                    }
                    if actual["result"] == "violation" {
                        assert_eq!(actual["violation_validated"], true, "{actual}");
                        assert_eq!(actual["counterexample"]["validation"]["result"], "valid");
                        let trace = dive_api::trace::TraceDoc::from_json(
                            &actual["counterexample"]["trace"],
                        )?;
                        assert_eq!(trace.states.len(), 2);
                        assert_eq!(trace.states[0]["x"], 0);
                        assert_eq!(trace.states[1]["x"], 2);
                        agreed::check_eq(
                            AGREED,
                            "enumerated_compound_batches_match_pinned_counts_and_actual_words/1",
                            &(actual["counterexample"]["trace"]),
                            &(reference["counterexample"]["trace"]),
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn closed_compound_candidates_preserve_reference_normalization_and_completion() -> Result {
    let directory =
        std::env::temp_dir().join(format!("dive-core-dfid-compound-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let file = directory.join("Compound.tla");
    std::fs::write(
        &file,
        "---- MODULE Compound ----\nEXTENDS Naturals,Sequences\nVARIABLES q, f\nInit == q = <<>> /\\ f = [i \\in {1,2} |-> {}]\nNext == Len(q) < 3 /\\ q' = Append(q, Len(q)) /\\ f' = [f EXCEPT ![1] = @ \\cup {Len(q)}]\nInv == Len(q) < 3\n====\n",
    )?;
    std::fs::write(
        file.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    for compat_tlc in [false, true] {
        for exact in [false, true] {
            let request = DfidRequest {
                file: file.clone(),
                compat_tlc,
                exact,
                max_level: 8,
                worker_seeds: Some(vec![0; 8]),
                seed: Some(7),
                ..Default::default()
            };
            let reference = dive_api::dfid_file_engine(&request, dive_api::Engine::Core)?.to_json();
            let native = dfid_file_engine(&request, Engine::Core)?.to_json();
            for field in [
                "result",
                "statistics",
                "tlc_code",
                "exit_code",
                "violation_validated",
            ] {
                agreed::check_eq(
                    AGREED,
                    "closed_compound_candidates_preserve_reference_normalization_and_completion/0",
                    &(native[field]),
                    &(reference[field]),
                );
            }
            assert_eq!(native["result"], "violation", "{native}");
            assert_eq!(native["violation_validated"], true);
        }
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
