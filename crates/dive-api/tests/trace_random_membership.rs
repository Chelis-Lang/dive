//! Direct choice support is independent of producer samples.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_api::trace::{Check, Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_eval::St;
use dive_value::Value;
use serde_json::{Value as Json, json};
use std::sync::Arc;

fn model(
    init: &str,
    next: &str,
    inv: &str,
    extra: &str,
    tlc: bool,
    run: Option<(i64, usize)>,
) -> SpecModel {
    let source = format!(
        "---- MODULE Audit ----\nEXTENDS Integers, TLC\nVARIABLE x\n{extra}\nInit == {init}\nNext == {next}\nInv == {inv}\n====\n"
    );
    SpecModel::from_source(
        "Audit.tla",
        &source,
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        &TraceOptions {
            compat_tlc: tlc,
            run,
            ..TraceOptions::default()
        },
    )
    .unwrap_or_else(|e| panic!("{init}, {next}, {inv}, {extra}: {e:?}"))
}
fn doc(values: &[Json], lossy: bool) -> TraceDoc {
    let doc = if lossy {
        TraceDoc::from_json(&json!({"vars":["x"],"counterexample":{"action":[],"state":values.iter().enumerate().map(|(i,x)|json!([i+1,{"x":x}])).collect::<Vec<_>>()}})).expect("TLC document")
    } else {
        TraceDoc::behavior(values.iter().map(|x| json!({"x":x})).collect()).expect("document")
    };
    doc.with_ending(Ending::Invariant("Inv".into()))
}
fn invalid(validation: Validation, check: Check) {
    let Validation::Invalid(failure) = validation else {
        panic!("expected invalid, got {validation:?}");
    };
    assert_eq!(failure.check, check);
}
#[test]
fn interval_membership_does_not_match_a_seed_sample() {
    for tlc in [false, true] {
        for run in [None, Some((0, 0)), Some((719, 3))] {
            let m = model(
                "x=RandomElement(1..1000000)",
                "UNCHANGED x",
                "x<0",
                "",
                tlc,
                run,
            );
            for lossy in [false, true] {
                for _ in 0..2 {
                    for x in [730_968, 777, 1, 1_000_000] {
                        let d = doc(&[json!(x)], lossy);
                        let (report, trace) = m.read(&d);
                        assert_eq!(report.validation, Validation::Valid);
                        assert_eq!(
                            m.validate_trace(&trace.expect("typed trace")).validation,
                            Validation::Valid
                        );
                    }
                    for x in [0, 1_000_001] {
                        invalid(m.validate(&doc(&[json!(x)], lossy)).validation, Check::Init);
                    }
                }
            }
        }
    }
}
#[test]
fn exact_stored_domain_and_ending_are_both_required() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            let singleton = model("x=RandomElement({7})", "UNCHANGED x", "x<0", "", tlc, None);
            assert_eq!(
                singleton.validate(&doc(&[json!(7)], lossy)).validation,
                Validation::Valid
            );
            invalid(
                singleton.validate(&doc(&[json!(8)], lossy)).validation,
                Check::Init,
            );
            let sparse = model(
                "x=RandomElement({2,7,7})",
                "UNCHANGED x",
                "x<0",
                "",
                tlc,
                None,
            );
            assert_eq!(
                sparse.validate(&doc(&[json!(7)], lossy)).validation,
                Validation::Valid
            );
            invalid(
                sparse.validate(&doc(&[json!(3)], lossy)).validation,
                Check::Init,
            );
            let claimed = model(
                "x=RandomElement(1..1000000)",
                "UNCHANGED x",
                "x>0",
                "",
                tlc,
                None,
            );
            invalid(
                claimed.validate(&doc(&[json!(777)], lossy)).validation,
                Check::Ending,
            );
            let step = model("x=RandomElement({7})", "UNCHANGED x", "x<8", "", tlc, None);
            invalid(
                step.validate(&doc(&[json!(7), json!(8)], lossy)).validation,
                Check::Next,
            );
        }
    }
}
#[test]
fn native_domain_errors_are_not_out_of_domain_verdicts() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            for (domain, code) in [
                ("{}", "2154"),
                ("1..0", "2154"),
                ("7", "2176"),
                ("Nat", "2176"),
                ("Int", "2176"),
                ("1\\div0", "TLC"),
            ] {
                let m = model(
                    &format!("x=RandomElement({domain})"),
                    "UNCHANGED x",
                    "x<0",
                    "",
                    tlc,
                    None,
                );
                let Validation::Unsupported(feature) =
                    m.validate(&doc(&[json!(777)], lossy)).validation
                else {
                    panic!("domain {domain} should retain error/refusal");
                };
                assert!(feature.contains(code), "{domain}: {feature}");
                assert!(
                    !feature.contains("internal evaluator error"),
                    "{domain}: {feature}"
                );
            }
            for domain in ["SUBSET {1,2}", "{<<1,2>>}"] {
                let m = model(
                    &format!("x=RandomElement({domain})"),
                    "UNCHANGED x",
                    "x<0",
                    "",
                    tlc,
                    None,
                );
                assert!(
                    matches!(
                        m.validate(&doc(&[json!(777)], lossy)).validation,
                        Validation::Unsupported(_)
                    ),
                    "{domain}"
                );
            }
        }
    }
}
#[test]
fn native_wrapped_repeated_and_later_choices_keep_source_demands() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            for init in [
                "x=RandomElement({7}) /\\ x=RandomElement({7})",
                "x=RandomElement({7,8}) /\\ x=RandomElement({7})",
                "x=RandomElement({7})+0",
                "x=IF TRUE THEN RandomElement({7}) ELSE 8",
                "(RandomElement({7})=7) /\\ x=7",
                "x=Pick({7})",
                "x=Cached",
            ] {
                let m = model(
                    init,
                    "UNCHANGED x",
                    "x<0",
                    if init == "x=Cached" {
                        "Cached==RandomElement({7})"
                    } else if init == "x=Pick({7})" {
                        "Pick(S)==RandomElement(S)"
                    } else {
                        ""
                    },
                    tlc,
                    Some((0, 0)),
                );
                let validation = m.validate(&doc(&[json!(7)], lossy)).validation;
                if lossy {
                    assert!(
                        matches!(validation, Validation::Unsupported(_)),
                        "{init}: {validation:?}"
                    );
                } else {
                    assert_eq!(validation, Validation::Valid, "{init}");
                    invalid(m.validate(&doc(&[json!(8)], false)).validation, Check::Init);
                }
            }
            for (next, inv, extra, values) in [
                ("x'=RandomElement({8})", "x<8", "", vec![json!(7), json!(8)]),
                ("UNCHANGED x", "RandomElement({7})=x+1", "", vec![json!(7)]),
                (
                    "UNCHANGED x",
                    "x<0",
                    "ASSUME RandomElement({7})=7",
                    vec![json!(7)],
                ),
                (
                    "UNCHANGED x",
                    "x<0",
                    "Unused==(RandomElement({7})=7) /\\ (1=FALSE)",
                    vec![json!(7)],
                ),
            ] {
                let m = model("x=RandomElement({7})", next, inv, extra, tlc, Some((0, 0)));
                let validation = m.validate(&doc(&values, lossy)).validation;
                if lossy {
                    assert!(
                        matches!(validation, Validation::Unsupported(_)),
                        "{next} {inv} {extra}: {validation:?}"
                    );
                } else {
                    assert_eq!(validation, Validation::Valid, "{next} {inv} {extra}");
                    let mut forged = values.clone();
                    forged[0] = json!(8);
                    invalid(m.validate(&doc(&forged, false)).validation, Check::Init);
                    if values.len() > 1 {
                        let mut forged = values;
                        forged[1] = json!(9);
                        invalid(m.validate(&doc(&forged, false)).validation, Check::Next);
                    }
                }
            }
            // Registers require actual demanded/discarded-prefix replay; the
            // chosen state alone cannot supply that authority.
            for (inv, extra) in [("x<0", "ASSUME TLCSet(1,42)"), ("TLCGet(1)=0", "")] {
                let m = model(
                    "x=RandomElement({7})",
                    "UNCHANGED x",
                    inv,
                    extra,
                    tlc,
                    Some((0, 0)),
                );
                assert!(
                    matches!(
                        m.validate(&doc(&[json!(7)], lossy)).validation,
                        Validation::Unsupported(_)
                    ),
                    "{inv} {extra}"
                );
            }
        }
    }
}
#[test]
fn direct_operation_cannot_consume_the_callers_stream_or_cache() {
    for tlc in [false, true] {
        let extra = "Draw==IF x=0 THEN RandomElement(1..1000000) ELSE 0";
        let a = model(
            "x=RandomElement(1..1000000)",
            "UNCHANGED x",
            "x<0",
            extra,
            tlc,
            Some((0, 0)),
        );
        let b = model(
            "x=RandomElement(1..1000000)",
            "UNCHANGED x",
            "x<0",
            extra,
            tlc,
            Some((0, 0)),
        );
        let draw = |m: &SpecModel| {
            m.model()
                .evaluator
                .eval_def(
                    m.model().ir.root_def("Draw").expect("draw"),
                    &St::state(Arc::from([Some(Value::Int(0))])),
                )
                .expect("caller sample")
        };
        assert_eq!(draw(&a), draw(&b));
        for lossy in [false, true] {
            for _ in 0..2 {
                assert_eq!(
                    a.validate(&doc(&[json!(777)], lossy)).validation,
                    Validation::Valid
                );
            }
        }
        assert_eq!(draw(&a), draw(&b));
        assert!(matches!(
            dive_trace::validate_doc(a.model(), &doc(&[json!(777)], false), &[])
                .0
                .validation,
            Validation::Unsupported(_)
        ));
    }
}
#[test]
fn actual_domain_evaluation_cannot_use_the_witness_or_host() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            for domain in ["{x}", "IF TLCSet(1,42) THEN {7} ELSE {8}"] {
                let m = model(
                    &format!("x=RandomElement({domain})"),
                    "UNCHANGED x",
                    "x<0",
                    "",
                    tlc,
                    Some((0, 0)),
                );
                assert!(
                    matches!(
                        m.validate(&doc(&[json!(7)], lossy)).validation,
                        Validation::Unsupported(_)
                    ),
                    "{domain}"
                );
            }
            let nested = model(
                "x=RandomElement({RandomElement({7})})",
                "UNCHANGED x",
                "x<0",
                "",
                tlc,
                Some((0, 0)),
            );
            let validation = nested.validate(&doc(&[json!(7)], lossy)).validation;
            if lossy {
                assert!(
                    matches!(validation, Validation::Unsupported(_)),
                    "{validation:?}"
                );
            } else {
                assert_eq!(validation, Validation::Valid);
                invalid(
                    nested.validate(&doc(&[json!(8)], false)).validation,
                    Check::Init,
                );
            }
            let m = model(
                "x=RandomElement(1..10)",
                "UNCHANGED x",
                "x<0",
                "",
                tlc,
                None,
            );
            let malformed = TraceDoc::behavior(vec![json!({"wrong":7})]).expect("document");
            assert!(matches!(
                m.validate(&malformed).validation,
                Validation::Invalid(_)
            ));
            assert!(matches!(
                m.validate(&doc(&[json!(true)], lossy)).validation,
                Validation::Unsupported(_)
            ));
            assert_eq!(
                m.validate(&doc(&[json!(7)], lossy)).validation,
                Validation::Valid
            );
        }
    }
}

#[test]
fn quiet_print_domains_check_the_real_branch_without_caller_effects() {
    use dive_eval::{Host, HostError};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct CallerHost(AtomicUsize);
    impl Host for CallerHost {
        fn print_output(&self, _text: &str) -> Result<bool, HostError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(true)
        }
        fn random(
            &self,
            _op: dive_ir::StdOp,
            _args: &[Value],
            _ctx: dive_value::Ctx,
        ) -> Option<Result<Value, HostError>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Err(HostError {
                code: 0,
                message: "caller random stream must not be reached".into(),
            }))
        }
    }
    for tlc in [false, true] {
        for lossy in [false, true] {
            for (predicate, actual, forged) in [
                ("Print(7,TRUE)", 7, 8),
                ("Print(7,FALSE)", 8, 7),
                ("PrintT(7)", 7, 8),
            ] {
                let domain = format!("IF {predicate} THEN {{7}} ELSE {{8}}");
                let m = model(
                    &format!("x=RandomElement({domain})"),
                    "UNCHANGED x",
                    "x<0",
                    "",
                    tlc,
                    Some((0, 0)),
                );
                let host = Arc::new(CallerHost(AtomicUsize::new(0)));
                let caller: Arc<dyn Host> = host.clone();
                let _previous = m.model().evaluator.replace_host(Some(caller.clone()));
                let output = dive_eval::capture::PrintCapture::start();
                for _ in 0..2 {
                    assert_eq!(
                        m.validate(&doc(&[json!(actual)], lossy)).validation,
                        Validation::Valid
                    );
                    invalid(
                        m.validate(&doc(&[json!(forged)], lossy)).validation,
                        Check::Init,
                    );
                }
                assert_eq!(output.take(), [] as [dive_eval::capture::Captured; 0]);
                assert_eq!(host.0.load(Ordering::Relaxed), 0);
                let restored = m.model().evaluator.replace_host(None).expect("caller host");
                assert!(Arc::ptr_eq(&restored, &caller));
            }
        }
    }
}

#[test]
fn quiet_print_endings_check_the_actual_boolean() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            let violated = model(
                "x=RandomElement({7})",
                "UNCHANGED x",
                "Print(x,FALSE)",
                "",
                tlc,
                None,
            );
            assert_eq!(
                violated.validate(&doc(&[json!(7)], lossy)).validation,
                Validation::Valid
            );
            let satisfied = model(
                "x=RandomElement({7})",
                "UNCHANGED x",
                "Print(x,TRUE)",
                "",
                tlc,
                None,
            );
            invalid(
                satisfied.validate(&doc(&[json!(7)], lossy)).validation,
                Check::Ending,
            );
        }
    }
}

#[test]
fn quiet_print_domain_errors_are_not_membership_results() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            for domain in [
                "IF Print(1\\div0,TRUE) THEN {7} ELSE {8}",
                "IF Print(7,7) THEN {7} ELSE {8}",
            ] {
                let m = model(
                    &format!("x=RandomElement({domain})"),
                    "UNCHANGED x",
                    "x<0",
                    "",
                    tlc,
                    None,
                );
                assert!(matches!(
                    m.validate(&doc(&[json!(7)], lossy)).validation,
                    Validation::Unsupported(_)
                ));
            }
        }
    }
}

#[test]
fn shared_relation_keeps_error_classes_and_entry_budget() {
    for tlc in [false, true] {
        for (domain, limit, class, code) in [
            ("{7}", 0, dive_eval::ErrorClass::ResourceLimit, None),
            (
                "{}",
                1,
                dive_eval::ErrorClass::Eval,
                Some(dive_eval::Ec::JavaMethodOverride),
            ),
            (
                "1..0",
                1,
                dive_eval::ErrorClass::Eval,
                Some(dive_eval::Ec::JavaMethodOverride),
            ),
            (
                "Nat",
                1,
                dive_eval::ErrorClass::Eval,
                Some(dive_eval::Ec::ApplyingToWrongValue),
            ),
        ] {
            let m = model(
                &format!("x=RandomElement({domain})"),
                "UNCHANGED x",
                "x<0",
                "",
                tlc,
                None,
            );
            let pred = &m.model().init[0].pred;
            let dive_ir::Expr::Builtin {
                op: dive_ir::BuiltinOp::Eq,
                args,
            } = m.model().ir.expr(pred.expr)
            else {
                panic!("equality predicate")
            };
            let error = m
                .model()
                .evaluator
                .random_element_member(
                    &pred.scope,
                    args[1],
                    &St::initial(Arc::from([None])),
                    &Value::Int(7),
                    limit,
                )
                .expect_err("domain error");
            assert_eq!(error.class(), class);
            if let Some(code) = code {
                assert_eq!(error.ec(), code);
            }
        }
    }
}
#[test]
fn native_multivariable_and_configured_choices_need_complete_relations() {
    for tlc in [false, true] {
        for (source, cfg, values) in [
            (
                "---- MODULE Audit ----\nEXTENDS TLC\nVARIABLES x,y\nInit == x=RandomElement({7}) /\\ y=RandomElement({7})\nNext == UNCHANGED <<x,y>>\nInv == x=0\n====\n",
                "INIT Init\nNEXT Next\nINVARIANT Inv\n",
                json!({"x":7,"y":7}),
            ),
            (
                "---- MODULE Audit ----\nEXTENDS TLC\nCONSTANT Pick(_)\nVARIABLE x\nInit == x=Pick({7})\nNext == UNCHANGED x\nInv == x=0\n====\n",
                "CONSTANT Pick <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\n",
                json!({"x":7}),
            ),
        ] {
            let m = SpecModel::from_source(
                "Audit.tla",
                source,
                cfg,
                &TraceOptions {
                    compat_tlc: tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )
            .expect("model");
            let d = TraceDoc::behavior(vec![values.clone()])
                .expect("document")
                .with_ending(Ending::Invariant("Inv".into()));
            assert_eq!(m.validate(&d).validation, Validation::Valid);
            for name in values.as_object().expect("state").keys() {
                let mut forged = values.clone();
                forged[name] = json!(8);
                let d = TraceDoc::behavior(vec![forged])
                    .expect("forged document")
                    .with_ending(Ending::Invariant("Inv".into()));
                invalid(m.validate(&d).validation, Check::Init);
            }
        }
    }
}
#[test]
fn evaluated_finite_domain_and_zero_argument_relation_definition_work() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            let m = model(
                "Relation",
                "UNCHANGED x",
                "x<0",
                "Relation == x=RandomElement({i \\in 1..3: i>1})",
                tlc,
                None,
            );
            assert_eq!(
                m.validate(&doc(&[json!(2)], lossy)).validation,
                Validation::Valid
            );
            invalid(m.validate(&doc(&[json!(1)], lossy)).validation, Check::Init);
        }
    }
}

#[test]
fn scalar_membership_keeps_boolean_string_and_runtime_name_semantics() {
    for tlc in [false, true] {
        for lossy in [false, true] {
            let booleans = model(
                "x=RandomElement({TRUE,FALSE})",
                "UNCHANGED x",
                "x=FALSE",
                "",
                tlc,
                None,
            );
            assert_eq!(
                booleans.validate(&doc(&[json!(true)], lossy)).validation,
                Validation::Valid
            );
            invalid(
                booleans.validate(&doc(&[json!(false)], lossy)).validation,
                Check::Ending,
            );
            let strings = model(
                "x=RandomElement({\"a\",\"b\"})",
                "UNCHANGED x",
                "x=\"b\"",
                "",
                tlc,
                None,
            );
            assert_eq!(
                strings.validate(&doc(&[json!("a")], lossy)).validation,
                Validation::Valid
            );
            invalid(
                strings.validate(&doc(&[json!("b")], lossy)).validation,
                Check::Ending,
            );
            let result = strings
                .validate(&doc(&[json!("unknown")], lossy))
                .validation;
            if tlc {
                assert!(matches!(result, Validation::Unsupported(_)));
            } else {
                invalid(result, Check::Init);
            }
            assert_eq!(
                strings.validate(&doc(&[json!("a")], lossy)).validation,
                Validation::Valid
            );
        }
    }
}

#[test]
fn actual_p9_original_and_all_qa_mutants_keep_their_full_controls_contract() {
    let source =
        include_str!("../../../conformance/corpus/probes/specifications/P9Random/P9Random.tla");
    let config =
        include_str!("../../../conformance/corpus/probes/specifications/P9Random/P9Random.cfg");
    for compat_tlc in [false, true] {
        let model = SpecModel::from_source(
            "P9Random.tla",
            source,
            config,
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("actual retained P9 model");
        for lossy in [false, true] {
            // The saved primary sample is retained as a positive control.
            let original = doc(&[json!(730_968)], lossy);
            let controls = model.controls(&original);
            assert_eq!(controls.positive.validation, Validation::Valid);
            assert_eq!(controls.dive_format, Some(Validation::Valid));
            assert!(
                !controls.mutants.is_empty(),
                "empty mutation suite has no credit"
            );
            assert_eq!(controls.failures(), [] as [String; 0]);
            let (_, trace) = model.read(&original);
            let trace = trace.expect("original typed trace");
            for mutant in model.mutants(&trace) {
                assert!(
                    matches!(
                        model.validate_trace(&mutant.trace).validation,
                        Validation::Invalid(_)
                    ),
                    "{}",
                    mutant.name
                );
                let document = model.to_doc(&mutant.trace).expect("mutant transport");
                assert!(
                    matches!(model.validate(&document).validation, Validation::Invalid(_)),
                    "{}",
                    mutant.name
                );
            }
            assert_eq!(
                model.validate(&doc(&[json!(777)], lossy)).validation,
                Validation::Valid
            );
            invalid(
                model.validate(&doc(&[json!(1_000_001)], lossy)).validation,
                Check::Init,
            );
        }
    }
}

#[test]
fn direct_random_dispatch_preserves_existing_pure_compound_init() {
    for tlc in [false, true] {
        for (init, value) in [
            ("x = <<1,2>>", json!({"seq":[1,2]})),
            ("x = {7}", json!({"set":[7]})),
            (r"x = [i \in {1,2} |-> i]", json!({"fcn":[[1,1],[2,2]]})),
        ] {
            let model = model(init, "UNCHANGED x", "x # x", "", tlc, None);
            assert_eq!(
                model.validate(&doc(&[value], false)).validation,
                Validation::Valid
            );
        }
    }
}
