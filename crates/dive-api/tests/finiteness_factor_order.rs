//! `IsFiniteSet` of a record set with an empty and a non-set factor.
//!
//! TLC stops at the first empty factor in its own field order, so
//! `[amberAxis : {}, zephyrAxis : 0]` answers TRUE or fails (2154) by intern
//! order. The default profile must refuse the ill-typed factor whatever the
//! order, never report success; `--compat=tlc` follows TLC's order where the
//! golden pins it and refuses otherwise.
#![allow(clippy::expect_used, reason = "pinned corpus models")]

use dive_api::{CheckRequest, ExitConvention, check_file};
use std::path::PathBuf;

const UNPROVED_FACTOR: &str =
    "Cardinality/IsFiniteSet of a Product with an unproved factor shape or size";

fn request(name: &str, compat_tlc: bool, workers: usize) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../conformance/corpus/reference-finiteness-token-order/specifications/{name}/{name}.tla"
    ));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

#[test]
fn an_empty_factor_never_hides_a_non_set_factor_in_the_default_profile() {
    for name in ["EncounterTokenScalarFirst", "EncounterTokenEmptyFirst"] {
        for compat in [false, true] {
            for workers in [1, 4] {
                let report = check_file(&request(name, compat, workers)).expect("Core check");
                let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
                if compat && name == "EncounterTokenEmptyFirst" {
                    // Pinned TLC: success, 3 distinct, 3 generated, depth 3.
                    assert_eq!(value["verdict"], "success", "{value}");
                    assert_eq!(value["tlc"]["code"], 2193);
                    assert_eq!(value["statistics"]["distinct"], 3);
                    assert_eq!(value["statistics"]["generated"], 3);
                    assert_eq!(value["statistics"]["depth"], 3);
                    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 0);
                    continue;
                }
                assert_eq!(value["verdict"], "unsupported", "{name}/{compat}: {value}");
                assert_eq!(value["message"], UNPROVED_FACTOR, "{name}/{compat}");
                assert!(value["counterexample"].is_null());
                assert_eq!(report.exit_code_for(ExitConvention::Tlc), 3);
                assert_eq!(report.exit_code_for(ExitConvention::Native), 3);
            }
        }
    }
}
