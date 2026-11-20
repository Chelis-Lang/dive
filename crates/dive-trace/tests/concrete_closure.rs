//! Periodic words whose producer closes loops on concrete state equality
//! (`"closure": "concrete"`, simulation) validate under `VIEW` and
//! `SYMMETRY` exactly as without them; other periodic words keep refusing.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_model::Model;
use dive_trace::{Check, Ending, TraceDoc, Validation, validate_doc};
use serde_json::json;

fn bound(source: &str, cfg: &str) -> Model {
    let name = source.split_whitespace().nth(2).expect("name");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("lower"));
    let cfg = dive_config::parse(cfg).into_config().expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}

fn result(model: &Model, doc: &TraceDoc) -> Validation {
    validate_doc(model, doc, &[]).0.validation
}

const REFUSAL: &str =
    "periodic validation under VIEW or SYMMETRY needs concrete closing-state evidence";

/// `x` toggles forever and `y` never changes; under `VIEW x` the refusal
/// stands unless the closure is concrete, and then the closing step is checked.
#[test]
fn view_admits_only_concrete_closures_and_checks_their_closing_step() {
    let toggle = "---- MODULE Toggle ----\nEXTENDS Naturals\nVARIABLES x, y\nInit == x = 0 /\\ y = 0\n\
        Next == x' = 1 - x /\\ y' = y\nSpec == Init /\\ [][Next]_<<x, y>>\n\
        P == <>(y = 1)\nView == x\n====\n";
    let m = bound(toggle, "SPECIFICATION Spec\nPROPERTY P\nVIEW View\n");
    let lasso = TraceDoc::behavior(vec![json!({"x": 0, "y": 0}), json!({"x": 1, "y": 0})])
        .expect("word")
        .with_ending(Ending::Lasso { back_to: 0 })
        .with_properties(vec!["P".into()]);
    assert!(
        matches!(result(&m, &lasso), Validation::Unsupported(why) if why == REFUSAL),
        "{:?}",
        result(&m, &lasso)
    );
    let concrete = lasso.with_concrete_closure();
    assert_eq!(result(&m, &concrete), Validation::Valid);

    // TLC's V1: the loop closes on the view of a state the behavior never
    // returns to. With a concrete claim the closing step is not a Next step.
    let counter = "---- MODULE ViewCount ----\nEXTENDS Naturals\nVARIABLES x, y\n\
        Init == x = 0 /\\ y = 0\nNext == x' = 1 - x /\\ y' = y + 1\n\
        Spec == Init /\\ [][Next]_<<x, y>>\nP == <>(y = 5)\nView == x\n====\n";
    let m = bound(counter, "SPECIFICATION Spec\nPROPERTY P\nVIEW View\n");
    let spurious = TraceDoc::behavior(vec![json!({"x": 0, "y": 0}), json!({"x": 1, "y": 1})])
        .expect("word")
        .with_ending(Ending::Lasso { back_to: 0 })
        .with_properties(vec!["P".into()]);
    assert!(matches!(result(&m, &spurious), Validation::Unsupported(why) if why == REFUSAL));
    let claimed = spurious.with_concrete_closure();
    assert!(
        matches!(result(&m, &claimed), Validation::Invalid(f) if f.check == Check::Next && f.state == 1),
        "{:?}",
        result(&m, &claimed)
    );
}

/// TLC's Y2: under `SYMMETRY`, `x = b` is equivalent to `x = a`, which made
/// TLC miss the violation. The concrete a-b loop validates.
#[test]
fn symmetry_admits_concrete_closures() {
    let sym = "---- MODULE SymLoop ----\nEXTENDS TLC\nCONSTANTS a, b\nVARIABLE x\n\
        Init == x = a\nNext == x' = IF x = a THEN b ELSE a\n\
        Spec == Init /\\ [][Next]_x\nFairSpec == Spec /\\ WF_x(Next)\n\
        NeverOther == <>(x \\notin {a, b})\nPerms == Permutations({a, b})\n====\n";
    let m = bound(
        sym,
        "CONSTANTS a = a b = b\nSPECIFICATION FairSpec\nPROPERTY NeverOther\nSYMMETRY Perms\n",
    );
    let lasso = TraceDoc::behavior(vec![
        json!({"x": {"model": "a"}}),
        json!({"x": {"model": "b"}}),
    ])
    .expect("word")
    .with_ending(Ending::Lasso { back_to: 0 })
    .with_properties(vec!["NeverOther".into()]);
    assert!(matches!(result(&m, &lasso), Validation::Unsupported(why) if why == REFUSAL));
    assert_eq!(
        result(&m, &lasso.with_concrete_closure()),
        Validation::Valid
    );
}

#[test]
fn the_closure_claim_round_trips_and_rejects_other_values() {
    let doc = TraceDoc::behavior(vec![json!({"x": 0})])
        .expect("word")
        .with_ending(Ending::Stuttering)
        .with_properties(vec!["P".into()]);
    let plain = doc.to_json();
    assert!(plain.get("closure").is_none());
    assert!(!TraceDoc::from_json(&plain).expect("plain").concrete_closure);
    let concrete = doc.with_concrete_closure().to_json();
    assert_eq!(concrete["closure"], "concrete");
    assert!(
        TraceDoc::from_json(&concrete)
            .expect("concrete")
            .concrete_closure
    );
    let mut other = concrete;
    other["closure"] = json!("view");
    assert!(TraceDoc::from_json(&other).is_err());
}
