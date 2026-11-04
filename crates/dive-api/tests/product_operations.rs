//! Pinned receiver-preserving Product scalar operations and demanded errors.
#![allow(
    clippy::expect_used,
    reason = "pinned primary fixtures and model checks"
)]

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn request(name: &str, compat_tlc: bool) -> CheckRequest {
    let file = root().join(format!(
        "conformance/corpus/product-operations/specifications/{name}/{name}.tla"
    ));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
fn facts(report: &dive_api::CheckReport) -> Value {
    let d = report.to_json();
    json!({"verdict":d["verdict"],"code":d["tlc"]["code"],"distinct":d["statistics"]["distinct"],
        "generated":d["statistics"]["generated"],"depth":d["statistics"]["depth"],
        "exit":report.exit_code_for(ExitConvention::Tlc)})
}

#[test]
fn concrete_product_operations_match_pinned_successes_and_preserve_alias_history() {
    for name in [
        "OrdinaryProductCardinality",
        "OrdinaryProductFinite",
        "OrdinaryProductEquality",
        "OrdinaryTupleProductCardinality",
        "OrdinaryProductTupleMember",
        "OrdinaryProductFunctionMember",
        "ConcreteFactorTree",
        "FiniteRawAxisHistory",
        "EmptyFunctionRangeHistory",
        "FiniteFunctionDomainHistory",
        "SingletonFunctionDomainHistory",
    ] {
        let path=root().join(format!("conformance/golden/tlc-2026.10.06.014338/product-operations/specifications/{name}/{name}.cfg.json"));
        let d: Value = serde_json::from_slice(&std::fs::read(path).expect("generated primary"))
            .expect("golden JSON");
        let r = &d["result"];
        assert_eq!(r["verdict"], "success");
        let expected = json!({"verdict":"success","code":r["verdict_code"],"exit":r["exit_code"],
            "distinct":r["counts"]["distinct_states"],"generated":r["counts"]["states_generated"],"depth":r["counts"]["depth"]});
        for compat in [false, true] {
            let req = request(name, compat);
            let reference = check_file_engine(&req, Engine::Reference).expect("Reference check");
            assert_eq!(facts(&reference), expected, "{name}/{compat}/Reference");
            for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
                let core = check_file_core_opts(&req, switches).expect("Core check");
                assert_eq!(core.to_json()["engine"], "core");
                assert_eq!(facts(&core), expected, "{name}/{compat}/{switches}");
            }
        }
    }
}

#[test]
fn sizing_keeps_product_membership_receiver_and_demands_nonempty_axis_sizes() {
    for compat in [false, true] {
        let req = request("SizedProductReceiver", compat);
        let reference = check_file_engine(&req, Engine::Reference).expect("Reference check");
        let expected = reference.to_json();
        assert_eq!(expected["verdict"], "evaluation-error");
        assert!(
            expected["message"]
                .as_str()
                .is_some_and(|s| s.contains("non-tuple"))
        );
        assert_eq!(expected["tlc"]["verified"], false);
        for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
            let core = check_file_core_opts(&req, switches).expect("Core check");
            let d = core.to_json();
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
            assert_eq!(d["verdict"], expected["verdict"]);
            assert_eq!(d["tlc"], expected["tlc"]);
            assert_eq!(d["message"], expected["message"]);
        }
        let req = request("DemandedRawProductSize", compat);
        assert!(!root().join("conformance/golden/tlc-2026.10.06.014338/product-operations/specifications/DemandedRawProductSize/DemandedRawProductSize.cfg.json").exists());
        let reference = check_file_engine(&req, Engine::Reference).expect("Reference check");
        let d = reference.to_json();
        assert_eq!(d["verdict"], "evaluation-error");
        assert_eq!(reference.exit_code_for(ExitConvention::Tlc), 75);
        assert_eq!(d["tlc"]["verified"], false);
        assert!(
            d["message"]
                .as_str()
                .is_some_and(|s| s.contains("STARTMSG 2154:"))
        );
        for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
            let core = check_file_core_opts(&req, switches).expect("Core check");
            let d = core.to_json();
            assert_eq!(d["verdict"], "unsupported");
            assert_eq!(core.exit_code_for(ExitConvention::Tlc), 3);
            assert!(
                d["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("local scalar/tuple comparison")),
                "{d}"
            );
        }
    }
}
