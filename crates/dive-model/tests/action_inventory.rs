//! Bound action predicate inventory from the pinned actual simulator.

#![allow(clippy::expect_used, reason = "test setup failures must fail the test")]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use dive_ir::DefId;
use dive_ir::actions::{ActionName, ActionTree, Split};
use dive_model::{BindOptions, Model};

fn bind_case(name: &str, options: &BindOptions) -> Result<Model, Box<dive_model::Failure>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/action-inventory");
    let source = std::fs::read_to_string(dir.join(format!("{name}.tla"))).expect("source fixture");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        &source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{name}: {:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowered fixture"));
    let config = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("config fixture");
    let config = dive_config::parse(&config)
        .into_config()
        .expect("valid config");
    dive_model::bind(ir, &config, options)
}

#[test]
fn bound_inventory_matches_pinned_predicate_locations_and_names() {
    let facts: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/action-inventory/pinned-inventory.json"
    ))
    .expect("generated pinned inventory");
    let cases = facts["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 20);
    for options in [BindOptions::tlc(), BindOptions::default()] {
        for case in cases {
            let name = case["name"].as_str().expect("case name");
            let bound = bind_case(name, &options);
            if case["primary_split_failed"]
                .as_bool()
                .expect("split status")
            {
                let error = bound.expect_err("bare configured value cannot be split");
                assert!(error.to_string().contains("TLC cannot split"), "{error}");
                continue;
            }
            let model = bound.expect("primary-supported inventory binds");
            let mut seen = BTreeSet::new();
            let mut actual = Vec::new();
            for action in model.init_actions.iter().chain(&model.actions) {
                let origin = model.ir.node(action.expr()).origin;
                if seen.insert((origin.file, origin.begin, origin.end)) {
                    let name = dive_ir::actions::name_text(&model.ir, action.name())
                        .unwrap_or_else(|| "Action".to_owned());
                    let module = model.ir.files()
                        [origin.file.expect("fixture has a source location").index()]
                    .module
                    .clone();
                    actual.push(serde_json::json!({
                        "name": name,
                        "module": module,
                        "begin": origin.begin,
                        "end": origin.end,
                    }));
                }
            }
            assert_eq!(
                actual,
                *case["groups"].as_array().expect("group array"),
                "{name}"
            );
        }
    }
}

#[test]
fn public_action_creation_refuses_unknown_definition_names() {
    let model = bind_case("SharedFalse", &BindOptions::tlc()).expect("valid model");
    let action = &model.init_actions[0];
    let invalid = ActionName::Def(DefId(u32::MAX));
    let error = model
        .evaluator
        .predicate_in(&action.scope(), action.expr(), invalid, 999)
        .expect_err("constructor must admit the definition name before rendering it");
    assert!(error.to_string().contains("DIVE-EVAL-C001"), "{error}");
    let split = Split {
        root: ActionTree::Leaf {
            expr: action.expr(),
            name: invalid,
        },
        defs: BTreeMap::new(),
    };
    let error = model
        .evaluator
        .actions_in(&action.scope(), &split, 1)
        .expect_err("public split must admit every definition name");
    assert!(error.to_string().contains("DIVE-EVAL-C001"), "{error}");
}
