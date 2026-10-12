//! Init's control context differs from generated-state checks and actions.
#![allow(clippy::expect_used, reason = "fixture construction and assertions")]

use dive_eval::Host;
use dive_model::{BindOptions, Model};
use dive_trace::{Check, Ending, Trace, TraceDoc, Validation, validate, validate_doc};
use serde_json::json;
use std::sync::Arc;

const SPEC: &str = include_str!("fixtures/startup-level/Audit.tla");
const CFG: &str = include_str!("fixtures/startup-level/Audit.cfg");

#[derive(Debug)]
struct CallerHost;
impl Host for CallerHost {
    fn level(&self) -> Option<i64> {
        Some(97)
    }
}

fn model(source: &str, cfg: &str, tlc: bool) -> (Model, Arc<dyn Host>) {
    let a = dive_sema::analyze_source("Audit.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{:?}", a.diagnostics());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(cfg).into_config().expect("cfg");
    let host: Arc<dyn Host> = Arc::new(CallerHost);
    let mut opts = if tlc {
        BindOptions::tlc()
    } else {
        BindOptions::dive()
    };
    opts.host = Some(host.clone());
    (dive_model::bind(ir, &cfg, &opts).expect("model"), host)
}

fn doc(values: &[i64], ending: Ending, lossy: bool) -> TraceDoc {
    let states = values.iter().map(|x| json!({"x":x})).collect();
    if lossy {
        TraceDoc::from_json(&json!({"vars":["x"],"counterexample":{"action":[],"state":values.iter().enumerate().map(|(k,x)| json!([k+1,{"x":x}])).collect::<Vec<_>>()}})).expect("lossy")
    } else {
        TraceDoc::behavior(states).expect("plain")
    }.with_ending(ending)
}

fn restored(m: &Model, host: &Arc<dyn Host>) {
    let after = m.evaluator.host().expect("caller host restored");
    assert!(Arc::ptr_eq(&after, host));
    assert_eq!(after.level(), Some(97));
}

#[test]
fn primary_startup_word_validates_and_forged_words_do_not() {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/startup-level/primary.json"))
            .expect("primary evidence");
    let values: Vec<i64> = evidence["states"]
        .as_array()
        .expect("states")
        .iter()
        .map(|s| s["x"].as_i64().expect("x"))
        .collect();
    assert_eq!(values, [0, 1, 2]);
    for tlc in [false, true] {
        for lossy in [false, true] {
            let (m, host) = model(SPEC, CFG, tlc);
            let actual = doc(&values, Ending::Invariant("Inv".into()), lossy);
            assert_eq!(
                validate_doc(&m, &actual, &[]).0.validation,
                Validation::Valid
            );
            restored(&m, &host);
            if !lossy {
                let decoded = Trace::decode(&m, &actual).expect("decode");
                assert_eq!(validate(&m, &decoded).validation, Validation::Valid);
                restored(&m, &host);
            }
            for (word, check) in [
                (&[1, 2, 3][..], Check::Init),
                (&[0, 2][..], Check::Next),
                (&[0, 1][..], Check::Ending),
            ] {
                let forged = doc(word, Ending::Invariant("Inv".into()), lossy);
                assert!(
                    matches!(validate_doc(&m,&forged,&[]).0.validation, Validation::Invalid(f) if f.check==check)
                );
                restored(&m, &host);
            }
        }
    }
}

#[test]
fn generated_initial_checks_and_action_checks_keep_their_own_levels() {
    let source = SPEC.replace("Next == x'=x+1", "Next == x'=x+1 /\\ TLCGet(\"level\")=x+1").replace("====", "InitialLevel == x=0 /\\ TLCGet(\"level\")=1\nActionLevel == [][x'=x+1 /\\ TLCGet(\"level\")=x+1]_x\nStateLevel == TLCGet(\"level\")=x+1\n====");
    let cfg = CFG.replace("INVARIANT Inv", "INVARIANT StateLevel Inv")
        + "PROPERTY InitialLevel ActionLevel\n";
    for tlc in [false, true] {
        for lossy in [false, true] {
            let (m, host) = model(&source, &cfg, tlc);
            assert_eq!(
                validate_doc(
                    &m,
                    &doc(&[0, 1, 2], Ending::Invariant("Inv".into()), lossy),
                    &[]
                )
                .0
                .validation,
                Validation::Valid
            );
            restored(&m, &host);
            // An Init context must not leak into generated-state predicates.
            for (needle, replacement, check) in [
                (
                    "TLCGet(\"level\")=1",
                    "TLCGet(\"level\")=0",
                    Check::ImpliedInit,
                ),
                (
                    "StateLevel == TLCGet(\"level\")=x+1",
                    "StateLevel == TLCGet(\"level\")=x",
                    Check::Invariant,
                ),
                (
                    "Next == x'=x+1 /\\ TLCGet(\"level\")=x+1",
                    "Next == x'=x+1 /\\ TLCGet(\"level\")=x",
                    Check::Next,
                ),
                (
                    "ActionLevel == [][x'=x+1 /\\ TLCGet(\"level\")=x+1]_x",
                    "ActionLevel == [][x'=x+1 /\\ TLCGet(\"level\")=x]_x",
                    Check::ImpliedAction,
                ),
            ] {
                let (bad, caller) = model(&source.replace(needle, replacement), &cfg, tlc);
                let result = validate_doc(
                    &bad,
                    &doc(&[0, 1, 2], Ending::Invariant("Inv".into()), lossy),
                    &[],
                )
                .0
                .validation;
                assert!(
                    matches!(result,Validation::Invalid(f) if f.check==check),
                    "{needle}"
                );
                restored(&bad, &caller);
            }
        }
    }
}

#[test]
fn caller_host_restored_after_refusal_and_missing_host_stays_missing() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            let (m, host) = model(
                &SPEC.replace("Next == x'=x+1", "Next == x'=RandomElement(1..10)"),
                CFG,
                tlc,
            );
            assert!(matches!(
                validate_doc(
                    &m,
                    &doc(&[0, 1, 2], Ending::Invariant("Inv".into()), lossy),
                    &[]
                )
                .0
                .validation,
                Validation::Unsupported(_)
            ));
            restored(&m, &host);
            let (m, _) = model(SPEC, CFG, tlc);
            let _ = m.evaluator.replace_host(None);
            assert_eq!(
                validate_doc(
                    &m,
                    &doc(&[0, 1, 2], Ending::Invariant("Inv".into()), lossy),
                    &[]
                )
                .0
                .validation,
                Validation::Valid
            );
            assert!(m.evaluator.host().is_none());
        }
    }
}
