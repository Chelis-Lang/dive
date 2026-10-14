//! Private startup and witness-independent initial membership controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_eval::St;
use dive_value::Value;
use serde_json::json;

fn model(assume: &str, init: &str, next: &str, inv: &str, extra: &str) -> SpecModel {
    let source = format!(
        "---- MODULE Audit ----\nEXTENDS Naturals, TLC\nVARIABLE x\n{assume}\nInit == {init}\nNext == {next}\nInv == {inv}\n{extra}\n====\n"
    );
    SpecModel::from_source(
        "Audit.tla",
        &source,
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        &TraceOptions {
            compat_tlc: true,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("model")
}

fn doc(xs: &[i64], lossy: bool) -> TraceDoc {
    let doc = if lossy {
        TraceDoc::from_json(&json!({"vars":["x"],"counterexample":{"action":[],"state":xs.iter().enumerate().map(|(i,x)|json!([i+1,{"x":x}])).collect::<Vec<_>>()}})).expect("TLC doc")
    } else {
        TraceDoc::behavior(xs.iter().map(|x| json!({"x":x})).collect()).expect("doc")
    };
    doc.with_ending(Ending::Invariant("Inv".into()))
}

fn eval(model: &SpecModel, name: &str) -> Result<Value, dive_eval::EvalError> {
    let m = model.model();
    m.evaluator.eval_def(
        m.ir.root_def(name).expect("definition"),
        &St::state(Arc::from([Some(Value::Int(0))])),
    )
}

#[test]
fn startup_registers_validate_both_formats_and_keep_caller_isolated() {
    let m = model(
        "ASSUME TLCSet(1,42)",
        "x=0 /\\ TLCGet(1)=42",
        "x'=(x+1)%10",
        "TLCGet(\"level\")<4",
        "Seed == x=0 /\\ TLCSet(1,99)\nRead == IF x=0 THEN TLCGet(1) ELSE 0",
    );
    assert!(eval(&m, "Read").is_err());
    assert_eq!(eval(&m, "Seed").expect("seed"), Value::TRUE);
    for lossy in [false, true] {
        for _ in 0..2 {
            assert_eq!(
                m.validate(&doc(&[0, 1, 2, 3], lossy)).validation,
                Validation::Valid
            );
            assert!(matches!(
                m.validate(&doc(&[0, 1, 3], lossy)).validation,
                Validation::Invalid(_)
            ));
        }
    }
    assert_eq!(eval(&m, "Read").expect("caller read"), Value::Int(99));
    let (_, trace) = m.read(&doc(&[0, 1, 2, 3], false));
    assert_eq!(
        m.validate_trace(&trace.expect("trace")).validation,
        Validation::Valid
    );
}

#[test]
fn caller_registers_and_cached_startup_values_are_not_authority() {
    let m = model(
        "",
        "x=0 /\\ TLCGet(1)=42",
        "x'=x+1",
        "x<2",
        "Seed == x=0 /\\ TLCSet(1,42)",
    );
    eval(&m, "Seed").expect("seed");
    assert!(matches!(
        m.validate(&doc(&[0, 1, 2], false)).validation,
        Validation::Unsupported(_)
    ));
    let m = model(
        "",
        "x=0 /\\ TLCGet(1)=42",
        "x'=x+1",
        "x<2",
        "Unused == TLCSet(1,42) /\\ (1=FALSE)",
    );
    // The real binder executes the write even though its constant body fails.
    assert_eq!(
        m.validate(&doc(&[0, 1, 2], false)).validation,
        Validation::Valid
    );
}

#[test]
fn effectful_init_determines_values_before_comparing_the_witness() {
    let m = model(
        "ASSUME TLCSet(1,42)",
        "x=TLCGet(1) /\\ TLCSet(1,x+1) /\\ TLCGet(1)=43",
        "x'=x+1",
        "x<43",
        "",
    );
    assert_eq!(
        m.validate(&doc(&[42, 43], false)).validation,
        Validation::Valid
    );
    assert!(matches!(
        m.validate(&doc(&[7, 43], false)).validation,
        Validation::Invalid(_)
    ));
    for init in [
        "(x=0 /\\ TLCGet(1)=42) \\/ (x=1 /\\ TLCSet(1,99))",
        "x=0 \\/ (x=1 /\\ TLCSet(1,99))",
        "x \\in {0,1} /\\ TLCSet(1,x)",
        "\\E y \\in {0,1}: x=y /\\ TLCSet(1,y)",
        "TLCGet(1)=42 /\\ x=x",
    ] {
        let m = model("ASSUME TLCSet(1,42)", init, "x'=x+1", "x<2", "");
        assert!(
            matches!(
                m.validate(&doc(&[0, 1, 2], false)).validation,
                Validation::Unsupported(_)
            ),
            "{init}"
        );
    }
}

#[test]
fn every_register_access_after_init_and_generic_startup_effects_stay_refused() {
    for (next, inv, extra) in [
        ("x'=x+1 /\\ TLCSet(1,x')", "x<2", ""),
        ("x'=IF TLCGet(1)=42 THEN x+1 ELSE x", "x<2", ""),
        ("x'=x+1", "TLCGet(1)=42", ""),
        ("x'=x+1", "x<2", "Unused == RandomElement({7})"),
        ("x'=x+1", "x<2", "Unused == TLCSet(\"pause\",FALSE)"),
    ] {
        let m = model("ASSUME TLCSet(1,42)", "x=0", next, inv, extra);
        for lossy in [false, true] {
            assert!(
                matches!(
                    m.validate(&doc(&[0, 1, 2], lossy)).validation,
                    Validation::Unsupported(_)
                ),
                "{next}; {inv}; {extra}"
            );
        }
    }
    let m = model("ASSUME TLCGet(\"generated\")=0", "x=0", "x'=x+1", "x<2", "");
    assert!(matches!(
        m.validate(&doc(&[0, 1, 2], false)).validation,
        Validation::Unsupported(_)
    ));
    let next = "(x=0 /\\ x'=1) \\/ (x=0 /\\ x'=2 /\\ TLCSet(1,43)) \\/ (x=1 /\\ x'=IF TLCGet(1)=42 THEN 3 ELSE 1) \\/ (x=2 /\\ x'=2)";
    let m = model("ASSUME TLCSet(1,42)", "x=0", next, "x<3", "");
    assert!(matches!(
        m.validate(&doc(&[0, 1, 3], false)).validation,
        Validation::Unsupported(_)
    ));
}

#[test]
fn source_instances_and_config_keep_the_real_startup_lifecycle() {
    let source = "---- MODULE Audit ----\nEXTENDS Naturals,TLC\nVARIABLE x\n---- MODULE Inner ----\nEXTENDS Naturals\nVARIABLE v\nInit == v=0\n====\nI == INSTANCE Inner WITH v <- x\nASSUME TLCSet(1,42) /\\ TLCGet(\"config\").worker=1\nInit == I!Init /\\ TLCGet(1)=42\nNext == x'=x+1\nInv == x<2\n====\n";
    let m = SpecModel::from_source(
        "Audit.tla",
        source,
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        &TraceOptions {
            compat_tlc: true,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("instance model");
    assert_eq!(
        m.validate(&doc(&[0, 1, 2], false)).validation,
        Validation::Valid
    );
}

#[test]
fn lossy_failed_interpretations_get_fresh_registers_and_names() {
    let m = model(
        "ASSUME TLCSet(1,42)",
        "x = <<0,1>> /\\ TLCSet(1,TLCGet(1)+1) /\\ TLCGet(1)=43",
        "x'=x",
        "x # x",
        "",
    );
    let doc = TraceDoc::from_json(
        &json!({"vars":["x"],"counterexample":{"action":[],"state":[[1,{"x":[0,1]}]]}}),
    )
    .expect("ambiguous TLC doc")
    .with_ending(Ending::Invariant("Inv".into()));
    for _ in 0..3 {
        assert_eq!(m.validate(&doc).validation, Validation::Valid);
    }
    let foreign = TraceDoc::behavior(vec![json!({"x":"new name"})])
        .expect("foreign name")
        .with_ending(Ending::Invariant("Inv".into()));
    assert!(matches!(
        m.validate(&foreign).validation,
        Validation::Unsupported(_)
    ));
    assert_eq!(m.validate(&doc).validation, Validation::Valid);
}

#[test]
fn post_init_constraints_properties_labels_and_liveness_are_refused() {
    let source = "---- MODULE Audit ----\nEXTENDS Naturals,TLC\nVARIABLE x\nASSUME TLCSet(1,42)\nInit==x=0\nNext==x'=x+1\nInv==x<2\nConstraint==TLCGet(1)=42\nActionConstraint==x'=x+1 /\\ TLCGet(1)=42\nInitialProperty==x=0 /\\ TLCGet(1)=42\nLabel==x'=x+1 /\\ TLCGet(1)=42\nLive==[] (TLCGet(1)=42)\n====\n";
    for config in [
        "CONSTRAINT Constraint\n",
        "ACTION_CONSTRAINT ActionConstraint\n",
        "PROPERTY InitialProperty\n",
        "PROPERTY Live\n",
        "",
    ] {
        let config = format!("INIT Init\nNEXT Next\nINVARIANT Inv\n{config}");
        let m = SpecModel::from_source(
            "Audit.tla",
            source,
            &config,
            &TraceOptions {
                compat_tlc: true,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("stage model");
        let mut word = doc(&[0, 1, 2], false);
        if config.ends_with("Inv\n") {
            word.labels[1] = Some(dive_trace::Label {
                name: "Label".into(),
                location: None,
            });
        }
        if config.contains("PROPERTY Live") {
            word = doc(&[0], false)
                .with_ending(Ending::Stuttering)
                .with_properties(vec!["Live".into()]);
        }
        assert!(
            matches!(m.validate(&word).validation, Validation::Unsupported(_)),
            "{config}"
        );
    }
}

#[test]
fn assumptions_keep_false_and_undefined_error_identity_and_raw_api_refuses() {
    for assume in ["ASSUME TLCSet(1,42) /\\ FALSE", "ASSUME TLCGet(1)=42"] {
        let m = model(assume, "x=0", "x'=x+1", "x<2", "");
        let Validation::Invalid(failure) = m.validate(&doc(&[0, 1, 2], false)).validation else {
            panic!("assumption must fail")
        };
        assert_eq!(failure.check, dive_trace::Check::Assumption);
    }
    let m = model(
        "ASSUME TLCSet(1,42)",
        "x=0 /\\ TLCGet(1)=42",
        "x'=x+1",
        "x<2",
        "",
    );
    assert!(matches!(
        dive_trace::validate_doc(m.model(), &doc(&[0, 1, 2], false), &[])
            .0
            .validation,
        Validation::Unsupported(_)
    ));
}

#[test]
fn conservative_fallback_claims_only_the_word_and_grants_no_register_authority() {
    let m = model(
        "",
        "(x=0 /\\ TRUE) \\/ (x=1 /\\ TLCSet(1,43))",
        "x'=x",
        "x<0",
        "Seed == x=0 /\\ TLCSet(1,99)\nRead == IF x=0 THEN TLCGet(1) ELSE 0",
    );
    eval(&m, "Seed").expect("caller seed");
    for lossy in [false, true] {
        for _ in 0..2 {
            // Conservative predicate membership makes no register-schedule claim.
            assert_eq!(m.validate(&doc(&[0], lossy)).validation, Validation::Valid);
            assert!(matches!(
                m.validate(&doc(&[1], lossy)).validation,
                Validation::Unsupported(_)
            ));
        }
    }
    assert_eq!(eval(&m, "Read").expect("caller unchanged"), Value::Int(99));
    let m = model(
        "",
        "(x=0 /\\ TRUE) \\/ (x=1 /\\ TLCSet(1,43))",
        "x'=IF TLCGet(1)=42 THEN 2 ELSE 0",
        "x<2",
        "",
    );
    for lossy in [false, true] {
        assert!(matches!(
            m.validate(&doc(&[0, 2], lossy)).validation,
            Validation::Unsupported(_)
        ));
    }
}

#[test]
fn pure_predicate_membership_keeps_nondeterministic_initial_relations() {
    for init in [
        "x \\in {0,1}",
        "\\E y \\in {0,1}: x=y",
        "x=0 \\/ x=1",
        "IF TRUE THEN x=0 ELSE x=1",
    ] {
        let m = model("", init, "x'=x+1", "x<2", "");
        for lossy in [false, true] {
            assert_eq!(
                m.validate(&doc(&[0, 1, 2], lossy)).validation,
                Validation::Valid,
                "{init}"
            );
        }
    }
}

#[test]
fn named_and_aggregate_registers_share_the_startup_only_boundary() {
    let m = model(
        "ASSUME TLCSet(1,42) /\\ TLCSet(\"s:r\",42)",
        "x=0 /\\ TLCGet(\"s:r\")=42 /\\ TLCGet(\"all\")[1][1]=42 /\\ TLCGet(\"all:named\")[\"s:r\"][1]=42",
        "x'=x+1",
        "x<2",
        "",
    );
    for lossy in [false, true] {
        assert_eq!(
            m.validate(&doc(&[0, 1, 2], lossy)).validation,
            Validation::Valid
        );
    }
    let m = model(
        "ASSUME TLCSet(1,42)",
        "x=0",
        "x'=x+1",
        "TLCGet(\"all\")[1][1]=42",
        "",
    );
    assert!(matches!(
        m.validate(&doc(&[0, 1, 2], false)).validation,
        Validation::Unsupported(_)
    ));
}

#[test]
fn replay_startup_and_runtime_prints_keep_returns_without_caller_events() {
    let m = model(
        "",
        r"x=0 /\ PrintT(0)",
        "x'=Print(x,x+1)",
        "x<2",
        "Unused == PrintT(\"hello\")",
    );
    let before = m.model().evaluator.has_print_event();
    for lossy in [false, true] {
        for _ in 0..2 {
            assert_eq!(
                m.validate(&doc(&[0, 1, 2], lossy)).validation,
                Validation::Valid
            );
            assert_eq!(m.model().evaluator.has_print_event(), before);
            assert_eq!(m.model().evaluator.take_output(), Vec::<String>::new());
            assert!(matches!(
                m.validate(&doc(&[0, 42, 2], lossy)).validation,
                Validation::Invalid(_)
            ));
        }
    }
}
