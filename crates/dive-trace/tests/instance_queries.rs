//! Primary-backed independent ENABLED decisions in INSTANCE namespaces.

#![allow(
    clippy::expect_used,
    reason = "pinned source fixtures must remain valid"
)]

use std::{path::Path, sync::Arc};

use dive_model::{BindOptions, Pred};
use dive_trace::{State, enabled::enabled};
use dive_value::Value;
use serde_json::json;

#[test]
fn independent_queries_preserve_exported_outside_nested_and_fresh_contexts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = [
        ("guard_decoy_exported", json!({"x":false,"y":true}), true),
        ("guard_decoy_nested", json!({"x":false,"y":true}), true),
        ("conflict_same_direct", json!({"y":false}), false),
        ("conflict_same_outside", json!({"y":false}), false),
        ("conflict_same_exported", json!({"y":false}), true),
        (
            "conflict_decoy_exported",
            json!({"x":false,"y":true,"z":false}),
            true,
        ),
        ("conflict_no_root_exported", json!({"x":false}), true),
        ("conflict_no_root_nested", json!({"x":false}), false),
        ("rebind_no_root_old", json!({"x":false}), true),
        ("rebind_no_root_fresh", json!({"x":false}), false),
    ];
    let mut failures = Vec::new();
    for (case, initial, expected) in cases {
        let fixtures = root
            .join("conformance/corpus/instance-temporal/specifications")
            .join(case);
        let primary: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join(
                    "conformance/golden/tlc-2026.10.06.014338/instance-temporal/specifications",
                )
                .join(case)
                .join("Probe.cfg.json"),
            )
            .expect("pinned primary result"),
        )
        .expect("primary JSON");
        // Guards claim Q <=> ~x; the others claim ~Q. These truth values
        // distinguish exported query cutoffs from an outside ENABLED.
        let violates = expected && !case.starts_with("guard_");
        assert_eq!(
            primary["result"]["verdict_code"],
            if violates { 2107 } else { 2193 }
        );
        for (profile, options) in [("tlc", BindOptions::tlc()), ("dive", BindOptions::dive())] {
            let analysis = dive_sema::analyze_file(
                &fixtures.join("Probe.tla"),
                &dive_sema::Options::default(),
            )
            .expect("original source and dependencies");
            assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
            let config = dive_config::parse(
                &std::fs::read_to_string(fixtures.join("Probe.cfg")).expect("config"),
            )
            .into_config()
            .expect("config parses");
            let model = dive_model::bind(
                Arc::new(dive_ir::lower(&analysis).expect("IR")),
                &config,
                &options,
            )
            .expect("model");
            let state: State = dive_trace::variables(&model)
                .iter()
                .map(|name| {
                    Some(Value::Bool(
                        initial[name].as_bool().expect("known initial value"),
                    ))
                })
                .collect::<Vec<_>>()
                .into();
            assert_eq!(
                model.eval(&model.init[0].pred, &dive_eval::St::initial(state.clone())),
                Ok(Value::TRUE),
                "literal state satisfies original Init"
            );
            let def = model.ir.root_def("Q").expect("actual original query");
            let pred = Pred::new(
                model.ir.def(def).body,
                model.evaluator.scope(def).expect("scope"),
            );
            for repetition in 0..2 {
                let decision = enabled(&model, &pred, None, &state);
                if decision != Ok(expected) {
                    failures.push(format!("{case}/{profile}/{repetition}: {decision:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
