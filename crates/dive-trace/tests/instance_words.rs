//! Independent replay of the pinned initial INSTANCE query counterexamples.

#![allow(
    clippy::expect_used,
    reason = "primary fixtures must remain well formed"
)]

use std::{path::Path, sync::Arc};

use dive_model::BindOptions;
use dive_trace::{Ending, Trace, TraceDoc, Validation};
use dive_value::Value;

fn replay_primary(case: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root
        .join("conformance/corpus/instance-temporal/specifications")
        .join(case);
    let primary: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join("conformance/golden/tlc-2026.10.06.014338/instance-temporal/specifications")
                .join(case)
                .join("Probe.cfg.json"),
        )
        .expect("pinned primary result"),
    )
    .expect("primary JSON");
    assert_eq!(primary["result"]["verdict_code"], 2107);
    assert_eq!(primary["result"]["violated_property"], "P");
    assert_eq!(primary["result"]["trace"]["length"], 1);
    let values = primary["result"]["trace"]["json"]["counterexample"]["state"][0][1]
        .as_object()
        .expect("actual primary initial state");

    let mut failures = Vec::new();
    for (profile, options) in [("tlc", BindOptions::tlc()), ("dive", BindOptions::dive())] {
        let analysis =
            dive_sema::analyze_file(&fixtures.join("Probe.tla"), &dive_sema::Options::default())
                .expect("source and original dependencies");
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let config = dive_config::parse(
            &std::fs::read_to_string(fixtures.join("Probe.cfg")).expect("original config"),
        )
        .into_config()
        .expect("config parses");
        let model = dive_model::bind(
            Arc::new(dive_ir::lower(&analysis).expect("IR")),
            &config,
            &options,
        )
        .expect("bound model");
        let state = dive_trace::variables(&model)
            .iter()
            .map(|name| {
                Some(Value::Bool(
                    values[name].as_bool().expect("primary boolean"),
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            state.len(),
            values.len(),
            "all primary variable assignments"
        );
        let genuine = Trace {
            states: vec![Arc::from(state.clone())],
            labels: vec![None],
            ending: Ending::Invariant("P".to_owned()),
            length: Some(1),
            properties: Vec::new(),
            concrete_closure: false,
            witnesses: Vec::new(),
            draws: Vec::new(),
        };
        let mut forged = genuine.clone();
        let mut changed = state;
        let first = changed[0].as_ref().expect("assigned first variable");
        changed[0] = Some(Value::Bool(first != &Value::TRUE));
        forged.states[0] = Arc::from(changed);

        for (kind, trace, valid) in [("genuine", genuine, true), ("forged Init", forged, false)] {
            let typed = dive_trace::validate(&model, &trace).validation;
            let doc = trace.to_doc(&model).expect("typed trace serializes");
            let parsed = TraceDoc::parse(&doc.to_json().to_string()).expect("transport parses");
            let serialized = dive_trace::validate_doc(&model, &parsed, &config.model_values)
                .0
                .validation;
            for (route, validation) in [("typed", typed), ("serialized", serialized)] {
                let expected = if valid {
                    validation == Validation::Valid
                } else {
                    matches!(
                        &validation,
                        Validation::Invalid(failure)
                            if failure.state == 0 && failure.check == dive_trace::Check::Init
                    )
                };
                if !expected {
                    failures.push(format!("{case}/{profile}/{kind}/{route}: {validation:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn exported_conflicting_aliases_use_original_successor_names() {
    replay_primary("conflict_same_exported");
}

#[test]
fn exported_hidden_assignments_do_not_need_root_slots() {
    replay_primary("conflict_no_root_exported");
}

#[test]
fn root_decoys_do_not_replace_the_query_namespace() {
    replay_primary("conflict_decoy_exported");
}

#[test]
fn captured_old_substitutions_respect_the_query_cutoff() {
    replay_primary("rebind_no_root_old");
}
