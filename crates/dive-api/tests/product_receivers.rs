//! Product receiver classes survive compound construction and graph conversion.
#![allow(
    clippy::expect_used,
    reason = "pinned primary fixtures and result controls"
)]

use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn empty_products_keep_their_non_tuple_membership_demand() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "AppendMember",
        "ExceptMember",
        "FunctionMember",
        "MakeFcnMember",
        "BaseValidIntervals",
        "RecordMemberBefore",
        "RecordMemberAfter",
        "TupleMemberBefore",
        "TupleMemberAfter",
    ] {
        let relative = format!("specifications/{name}/{name}");
        let golden: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!(
                "conformance/golden/tlc-2026.10.06.014338/product-receivers/{relative}.cfg.json"
            )))
            .expect("generated primary"),
        )
        .expect("golden JSON");
        assert_eq!(golden["result"]["verdict"], "evaluation-error");
        assert_eq!(golden["result"]["verdict_code"], 2147);
        assert_eq!(golden["result"]["exit_code"], 75);
        for compat_tlc in [false, true] {
            let file = root.join(format!(
                "conformance/corpus/product-receivers/{relative}.tla"
            ));
            let req = CheckRequest {
                config: Some(file.with_extension("cfg")),
                file,
                library_paths: vec![root.join("crates/dive-eval/tests/fixtures/community-modules")],
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            };
            let reference = check_file_engine(&req, Engine::Reference).expect("Reference run");
            let doc = reference.to_json();
            assert_eq!(doc["verdict"], "evaluation-error");
            assert_eq!(reference.exit_code_for(ExitConvention::Tlc), 75);
            assert!(
                doc["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("non-tuple"))
            );
            // Actual Reference source-receiver classification is unproved;
            // the pinned root 2147 is not inferred from this context report.
            assert_eq!(doc["tlc"]["verified"], false);
            for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
                let core = check_file_core_opts(&req, switches).expect("Core run");
                let doc = core.to_json();
                let expected = reference.to_json();
                assert_eq!(
                    doc["verdict"], expected["verdict"],
                    "{name}/{compat_tlc}/{switches}: {doc}"
                );
                assert_eq!(core.exit_code_for(ExitConvention::Tlc), 75);
                assert_eq!(doc["tlc"], expected["tlc"]);
                assert_eq!(doc["message"], expected["message"]);
            }
        }
    }
}
