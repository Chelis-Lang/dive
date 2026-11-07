//! The core route's binding leaves exactly the pure constant definitions its
//! roles use only as values to the core (issue #846).
#![allow(clippy::expect_used, reason = "bounded deterministic binding fixtures")]

use std::collections::BTreeSet;
use std::sync::Arc;

use dive_model::{BindOptions, bind, bind_deferring_constants};

const SOURCE: &str = r"---- MODULE Deferred ----
EXTENDS Naturals, FiniteSets, TLC
CONSTANT Node
VARIABLE x
Ring == [n \in Node |-> CHOOSE m \in Node : m # n]
Start == CHOOSE n \in Node : TRUE
Bound == 3
Stop == FALSE
Drawn == RandomElement(Node)
Unused == {n \in Node : n # Start}
Typed == x \in Node
Viewed == Cardinality(Node)
ASSUME Bound > 0
Init == x = Start
Next == \/ x' = Ring[x]
        \/ x' = Drawn
        \/ Stop
Inv == Typed
Alias == [y |-> x, n |-> Viewed]
====
";

const CONFIG: &str = "CONSTANT Node = {a, b}\nINIT Init\nNEXT Next\nINVARIANT Inv\nALIAS Alias\n";

fn inputs(source: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let sema = dive_sema::analyze_source("Deferred.tla", source, &dive_sema::Options::default());
    assert!(sema.accepted(), "{sema:?}");
    let ir = Arc::new(dive_ir::lower(&sema).expect("IR"));
    let cfg = dive_config::parse(CONFIG).into_config().expect("config");
    (ir, cfg)
}

fn names(model: &dive_model::Model) -> BTreeSet<String> {
    model
        .deferred_constants
        .iter()
        .map(|d| model.ir.def(*d).name.clone())
        .collect()
}

#[test]
fn only_pure_value_uses_by_core_roles_are_deferred() {
    let (ir, cfg) = inputs(SOURCE);
    let deferred = bind_deferring_constants(ir.clone(), &cfg, &BindOptions::dive())
        .expect("deferring binding");
    // `Ring` and `Start` are pure and used only as values by `Init` and
    // `Next`. `Bound` is read by an `ASSUME`, `Stop` stands in an action
    // position, `Drawn` draws, `Unused` no role reaches (it is left to no
    // one), `Typed` is level 1 and `Viewed` is read by the `ALIAS`.
    assert_eq!(
        names(&deferred),
        BTreeSet::from(["Ring".to_owned(), "Start".to_owned()])
    );
    for d in &deferred.deferred_constants {
        assert!(deferred.evaluator.precomputed(*d).is_none());
    }
    let viewed = ir.root_def("Viewed").expect("Viewed");
    assert!(deferred.evaluator.precomputed(viewed).is_some());
    // The ordinary binding defers nothing and computes them at start-up.
    let plain = bind(ir.clone(), &cfg, &BindOptions::dive()).expect("binding");
    assert!(plain.deferred_constants.is_empty());
    for name in ["Ring", "Start", "Unused"] {
        let d = ir.root_def(name).expect("definition");
        assert!(plain.evaluator.precomputed(d).is_some(), "{name}");
    }
    let unused = ir.root_def("Unused").expect("Unused");
    assert!(deferred.evaluator.precomputed(unused).is_none());
    // Everything else the two bindings produce is the same.
    assert_eq!(deferred.actions.len(), plain.actions.len());
    assert_eq!(deferred.init_actions.len(), plain.init_actions.len());
    assert_eq!(deferred.invariants.len(), plain.invariants.len());
}

#[test]
fn a_definition_a_precomputed_one_reaches_is_not_deferred() {
    let source = SOURCE.replace("Bound == 3", "Bound == Cardinality({Start}) + 2");
    let (ir, cfg) = inputs(&source);
    let deferred =
        bind_deferring_constants(ir, &cfg, &BindOptions::dive()).expect("deferring binding");
    // `Bound` is evaluated at start-up for the `ASSUME`, and reads `Start`.
    assert_eq!(names(&deferred), BTreeSet::from(["Ring".to_owned()]));
}
