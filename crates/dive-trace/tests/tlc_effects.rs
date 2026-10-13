//! Interpretation heuristics must not change the predicates they certify.
#![allow(clippy::expect_used, reason = "test construction and assertions")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dive_eval::{Host, HostError};
use dive_model::Model;
use dive_trace::{Check, Ending, TraceDoc, Validation, validate_doc};
use dive_value::{Ctx, Value};
use serde_json::json;

fn model(init: &str) -> Model {
    let source = format!(
        "---- MODULE Effects ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == {init}\nNext == x'=x\n====\n"
    );
    bind(&source, true)
}

fn bind(source: &str, tlc: bool) -> Model {
    let analysis = dive_sema::analyze_source("Effects.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        ir,
        &cfg,
        &if tlc {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::dive()
        },
    )
    .expect("bind")
}

fn word(x: i64) -> TraceDoc {
    document(&[x], true)
}

fn document(xs: &[i64], lossy: bool) -> TraceDoc {
    if lossy {
        TraceDoc::from_json(&json!({
            "vars": ["x"], "counterexample": {
                "action": [],
                "state": xs.iter().enumerate().map(|(i, x)| json!([i+1, {"x": x}])).collect::<Vec<_>>()
            }
        }))
        .expect("TLC document")
        .with_ending(Ending::Behavior)
    } else {
        TraceDoc::behavior(xs.iter().map(|x| json!({"x": x})).collect()).expect("dive document")
    }
}

#[test]
fn scoring_cannot_supply_a_register_value_that_makes_init_true() {
    let m = model("x=0 /\\ TLCGet(0)=1 /\\ TLCSet(0,1)");
    let report = validate_doc(&m, &word(0), &[]).0;
    assert_eq!(report.validation, registers_refused());
    // Scoring and replay must leave the previously absent register absent.
    let init = m.ir.root_def("Init").expect("Init");
    let state = dive_eval::St::state(Arc::from([Some(Value::Int(0))]));
    assert!(m.evaluator.eval_def(init, &state).is_err());
}

#[test]
fn scoring_does_not_print_an_untaken_conjunct() {
    let m = model("x=0 /\\ PrintT(TRUE)");
    let report = validate_doc(&m, &word(1), &[]).0;
    assert!(matches!(
        report.validation,
        Validation::Invalid(failure) if failure.check == Check::Init
    ));
    assert_eq!(m.take_output(), Vec::<String>::new());
}

#[derive(Debug, Default)]
struct DrawHost(AtomicUsize);

impl Host for DrawHost {
    fn random(&self, _: dive_ir::StdOp, _: &[Value], _: Ctx) -> Option<Result<Value, HostError>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Some(Ok(Value::Int(0)))
    }
}

#[test]
fn scoring_never_draws_from_the_inherited_host() {
    for (init, x, draws) in [
        ("x=0 /\\ RandomElement({0})=0", 1, false),
        ("x=RandomElement({0})", 0, true),
    ] {
        let m = model(init);
        let host = Arc::new(DrawHost::default());
        m.evaluator.set_host(host.clone());
        let report = validate_doc(&m, &word(x), &[]).0;
        if draws {
            assert!(matches!(
                report.validation,
                Validation::Unsupported(reason) if reason.contains(dive_trace::RANDOM_UNSUPPORTED)
            ));
        } else {
            assert!(matches!(report.validation, Validation::Invalid(_)));
        }
        assert_eq!(host.0.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn register_reads_writes_and_aggregates_refuse_without_changing_caller_registers() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            for inherited in [false, true] {
                for (key, access) in [
                    ("0", "TLCGet(0)=1"),
                    ("0", "TLCSet(0,2)"),
                    ("\"s:slot\"", "TLCGet(\"s:slot\")=1"),
                    ("\"s:slot\"", "TLCSet(\"s:slot\",2)"),
                    ("0", "TLCGet(\"all\")=TLCGet(\"all\")"),
                    ("\"s:slot\"", "TLCGet(\"all:named\")=TLCGet(\"all:named\")"),
                ] {
                    let source = format!(
                        "---- MODULE Effects ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0 /\\ {access}\nNext == x'=x\nSeed == x=0 /\\ TLCSet({key},1)\nRead == IF x=0 THEN TLCGet({key}) ELSE 0\n====\n"
                    );
                    let m = bind(&source, tlc);
                    // Existing hosts inherit the default that permits
                    // ordinary evaluator access before and after replay.
                    m.evaluator.set_host(Arc::new(DrawHost::default()));
                    let state = dive_eval::St::state(Arc::from([Some(Value::Int(0))]));
                    if inherited {
                        let seed = m.ir.root_def("Seed").expect("Seed");
                        assert_eq!(
                            m.evaluator.eval_def(seed, &state).expect("seed register"),
                            Value::TRUE
                        );
                    }
                    assert!(!m.has_runtime_names());
                    let report = validate_doc(&m, &document(&[0], lossy), &[]).0;
                    assert_eq!(
                        report.validation,
                        registers_refused(),
                        "{access}; inherited={inherited}; tlc={tlc}; lossy={lossy}"
                    );
                    let read = m.ir.root_def("Read").expect("Read");
                    let value = m.evaluator.eval_def(read, &state);
                    if inherited {
                        assert_eq!(value.expect("retained register"), Value::Int(1));
                    } else {
                        assert!(
                            value.is_err(),
                            "validation must not write a register: {access}; tlc={tlc}; lossy={lossy}; value={value:?}"
                        );
                    }
                }
            }
        }
    }
}

fn registers_refused() -> Validation {
    Validation::Unsupported(dive_trace::REGISTERS_UNSUPPORTED.to_owned())
}

#[test]
fn untaken_register_access_remains_a_valid_replay() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            for access in ["TLCGet(0)=1", "TLCSet(0,1)", "TLCGet(\"all\")=TRUE"] {
                let source = format!(
                    "---- MODULE Effects ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0 /\\ (IF x=0 THEN TRUE ELSE {access})\nNext == x'=x\n====\n"
                );
                let m = bind(&source, tlc);
                assert_eq!(
                    validate_doc(&m, &document(&[0], lossy), &[]).0.validation,
                    Validation::Valid
                );
            }
        }
    }
}

#[test]
fn swallowed_optional_label_errors_still_refuse_the_whole_verdict() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            let source = "---- MODULE Effects ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == x'=x\nRegisterLabel == TLCGet(0)=1\n====\n";
            let m = bind(source, tlc);
            let mut doc = document(&[0, 0], lossy);
            doc.labels[1] = Some(dive_trace::Label {
                name: "RegisterLabel".to_owned(),
                location: None,
            });
            assert_eq!(
                validate_doc(&m, &doc, &[]).0.validation,
                registers_refused()
            );
        }
    }
}
