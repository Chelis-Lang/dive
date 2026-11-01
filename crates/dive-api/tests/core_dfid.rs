//! Core DFID preserves finite visitation certificates and actual concrete words.
use dive_api::{DfidRequest, Engine, dfid_file, dfid_file_engine};
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
                    let reference = dfid_file(&request)?.to_json();
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
                        assert_eq!(
                            core[field], reference[field],
                            "{name}/{field}/{compat}/{exact}/{seed}: {core}"
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
fn native_depth_exhaustion_remains_incomplete_and_auto_reports_core() -> Result {
    for compat in [false, true] {
        for depth in [0, 1, 2, 4] {
            let request = DfidRequest {
                max_level: depth,
                ..request("DfidBound", compat, true, 0)
            };
            let report = dfid_file_engine(&request, Engine::Auto)?.to_json();
            assert_eq!(report["engine"], "core", "{report}");
            assert_eq!(report["engine_requested"], "auto");
            assert_eq!(report["result"], "incomplete");
            assert_eq!(report["model_proved"], false);
            assert_eq!(report["termination"], "depth-bound");
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
            let reference = dfid_file(&request)?.to_json();
            let native = dfid_file_engine(&request, Engine::Core)?.to_json();
            for field in [
                "result",
                "statistics",
                "tlc_code",
                "exit_code",
                "violation_validated",
            ] {
                assert_eq!(native[field], reference[field], "{field}: {native}");
            }
            assert_eq!(native["result"], "violation", "{native}");
            assert_eq!(native["violation_validated"], true);
        }
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
