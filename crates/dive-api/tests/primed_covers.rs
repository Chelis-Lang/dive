//! A primed quantifier reads its domain in the successor, not the current state.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_eval::St;
use dive_model::{BindOptions, Model, Pred};
use dive_trace::exists::{self, Decision, ExistsError, Limits};
use dive_value::{Ctx, Value};
use std::sync::Arc;

fn model(next: &str) -> Model {
    let source = r"---- MODULE PrimedCovers ----
VARIABLES S, t
Init == S={} /\ t=FALSE
A == (\E b \in S : t=b)'
B == S'={TRUE} /\ (\E b \in S : t=b)'
C == (\E b \in BOOLEAN : (\E c \in S : t=c))'
Ordinary == \E b \in S : t'=b
Constant == (\E b \in BOOLEAN : t=b)'
Next == UNCHANGED <<S,t>>
====
";
    let analysis =
        dive_sema::analyze_source("PrimedCovers.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(&format!("INIT Init\nNEXT {next}\n"))
        .into_config()
        .expect("cfg");
    dive_model::bind(ir, &cfg, &BindOptions::tlc()).expect("bind")
}
fn state(model: &Model, populated: bool) -> dive_trace::State {
    let set = Value::set(
        if populated { vec![true.into()] } else { vec![] },
        Ctx::tlc(),
    )
    .expect("set");
    model
        .ir
        .vars()
        .iter()
        .map(|d| {
            Some(match model.ir.decl(*d).name.as_str() {
                "S" => set.clone(),
                "t" => populated.into(),
                name => panic!("unknown variable {name}"),
            })
        })
        .collect::<Vec<_>>()
        .into()
}
fn pred(model: &Model, name: &str) -> Pred {
    let d = model.ir.root_def(name).expect(name);
    Pred::new(
        model.ir.def(d).body,
        model.evaluator.scope(d).expect("valid scope"),
    )
}
#[test]
fn primed_domains_cannot_be_scanned_as_current_empty_sets() {
    let model = model("Next");
    let before = state(&model, false);
    let witness = state(&model, true);
    for name in ["A", "B", "C"] {
        let action = pred(&model, name);
        assert_eq!(
            model
                .eval(&action, &St::action(before.clone(), witness.clone()))
                .expect("concrete witness"),
            true.into(),
            "{name}"
        );
        let result = exists::exists_successor(&model, &action, &before, None, Limits::default());
        assert!(
            matches!(result, Err(ExistsError::Unsupported(_))),
            "{name}: current-domain enumeration lost a real witness: {result:?}"
        );
    }
}
#[test]
fn current_empty_domain_still_certifies_disabledness() {
    let model = model("Next");
    let result = exists::exists_successor(
        &model,
        &pred(&model, "Ordinary"),
        &state(&model, false),
        None,
        Limits::default(),
    )
    .expect("decision");
    assert!(matches!(result, Decision::Disabled));
}
#[test]
fn constant_primed_domain_still_requires_a_cover_for_its_body_reads() {
    let model = model("Next");
    let result = exists::exists_successor(
        &model,
        &pred(&model, "Constant"),
        &state(&model, false),
        None,
        Limits::default(),
    );
    assert!(
        matches!(result, Err(ExistsError::Unsupported(_))),
        "{result:?}"
    );
}

#[test]
fn a_deadlock_claim_with_a_real_successor_is_not_validated() {
    let model = model("A");
    let trace = dive_trace::Trace {
        states: vec![state(&model, false)],
        labels: vec![None],
        ending: dive_trace::Ending::Deadlock,
        length: Some(1),
        properties: vec![],
    };
    let report = dive_trace::validate(&model, &trace, None);
    assert!(
        matches!(report.validation, dive_trace::Validation::Unsupported(_)),
        "{report:?}"
    );
}
