//! Real source binding: the no-bind fallback, binding failures and the
//! dispatch on the actual bound roles.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual owned-driver controls"
)]

use super::*;
use dive_runtime::budget::Phase;

fn input(body: &str, properties: &str) -> (Arc<Ir>, ModelConfig) {
    let text =
        format!("---- MODULE OwnedRow ----\nEXTENDS Naturals,TLC\nVARIABLE x\n{body}\n====\n");
    let source = dive_sema::analyze_source("OwnedRow.tla", &text, &dive_sema::Options::default());
    assert!(source.accepted(), "{source:?}");
    (
        Arc::new(dive_ir::lower(&source).expect("IR")),
        dive_config::parse(&format!("INIT Init\nNEXT Next\nPROPERTY {properties}\n"))
            .into_config()
            .expect("config"),
    )
}

fn control(options: &CheckOptions) -> Arc<EvaluationControl> {
    EvaluationControl::new(&options.budget, Phase::Binding).expect("binding owner")
}

#[test]
fn no_bind_fallback_binding_failure_and_safety_roles_stay_distinct() {
    let (ir, config) = input("Init == x=0\nNext == UNCHANGED x\nP == []<>(x=0)", "P");
    let mut options = CheckOptions::tlc(0, 0);
    options.chaos = Some(0);
    assert!(dispatch_controlled(&ir, &config, &options, control(&options)).is_none());
    let options = CheckOptions::tlc(0, 0);
    assert!(
        matches!(
            dispatch_controlled(&ir, &config, &options, control(&options)),
            Some(Ok(Dispatch::Temporal(_)))
        ),
        "a genuine temporal property keeps the fresh runtime"
    );
    let invalid = dive_config::parse("INIT Missing\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config grammar");
    assert!(matches!(
        dispatch_controlled(&ir, &invalid, &options, control(&options)),
        Some(Err(ControlledBindError::Binding(_)))
    ));
    let (ir, config) = input("Init == x=0\nNext == UNCHANGED x\nP == [](x=0)", "P");
    assert!(
        matches!(
            dispatch_controlled(&ir, &config, &options, control(&options)),
            Some(Ok(Dispatch::Safety))
        ),
        "a safety-only property is dispatched without being checked"
    );
}
