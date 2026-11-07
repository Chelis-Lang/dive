//! Public safety roles retain independently checked concrete witnesses.
#![allow(clippy::expect_used, reason = "test assertions")]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, SeenOptions, Store};
use std::path::Path;
fn witness_case(name: &str, root: &Path) {
    let file = root.join(name).join(format!("{name}.tla"));
    let config = file.with_extension("cfg");
    for compat_tlc in [false, true] {
        let validator = SpecModel::load(
            &file,
            &config,
            &TraceOptions {
                compat_tlc,
                run: Some((0, 7)),
                ..TraceOptions::default()
            },
        )
        .expect("independent model");
        for store in [Store::Fingerprint, Store::Exact] {
            for width in [1, 4, 8] {
                let report = dive_api::check_file_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(config.clone()),
                        seen_disk: Some(SeenOptions::default()),
                        compat_tlc,
                        store,
                        workers: Some(width),
                        seed: Some(7),
                        fp: Some(0),
                        chaos: Some(7),
                        frontier_memory_bytes: Some(0),
                        predecessor_memory_bytes: Some(0),
                        ..CheckRequest::default()
                    },
                    dive_api::Engine::Reference,
                )
                .expect("check");
                let json = report.to_json();
                assert_eq!(json["options"]["workers"]["requested"], width);
                assert_eq!(json["options"]["workers"]["used"], 1);
                assert_eq!(
                    json["seen_storage"]["after_cleanup"]["cleanup_completed"],
                    true
                );
                assert_eq!(
                    json["seen_storage"]["after_cleanup"]["expected_disk_bytes"],
                    0
                );
                assert_eq!(
                    json["seen_storage"]["first_failure"],
                    serde_json::Value::Null
                );
                let doc = TraceDoc::from_json(&json).expect("trace transport");
                let controls = validator.controls(&doc);
                assert_eq!(
                    controls.positive.validation,
                    Validation::Valid,
                    "{name} profile{compat_tlc} store{store:?} width{width}: {:?}",
                    controls.positive
                );
                assert!(controls.failures().is_empty(), "{:?}", controls.failures());
            }
        }
    }
}

macro_rules! role {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../dive-check/tests/fixtures/disk-seen-roles");
            witness_case($name, &root);
        }
    };
}
role!(action_property, "ActionProperty");
role!(always_property, "AlwaysProperty");
role!(conjunction_property, "ConjunctionProperty");
role!(instance_property, "InstanceProperty");
role!(let_property, "LetProperty");
role!(property_alias, "PropertyAlias");
