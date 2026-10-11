//! Supplied pure hook admission, actual pause composition and failure precedence.
#![allow(
    clippy::expect_used,
    reason = "bounded hook controls assert explicit outcomes"
)]
use dive_check::CheckOptions;
use dive_liveness::{periodic, periodic_hook, periodic_lifecycle, phase_graph};
use dive_model::Pred;
use std::sync::Arc;

fn model(hook: &str, live: &str) -> (dive_model::Model, Pred) {
    let source = format!(
        "---- MODULE Hook ----\nEXTENDS Integers\nCONSTANT Token\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == {live}\nHook == {hook}\nReplacement == TRUE\n====\n"
    );
    let a = dive_sema::analyze_source("Hook.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let def = ir
        .defs()
        .iter()
        .position(|d| d.name == "Hook")
        .expect("Hook");
    let def = dive_ir::DefId(u32::try_from(def).expect("id"));
    let cfg = dive_config::parse(
        "CONSTANT Token=Atom\nINIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("config");
    let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    let p = Pred::new(m.ir.def(def).body, m.evaluator.scope(def).expect("scope"));
    (m, p)
}
fn policy() -> periodic_lifecycle::Options {
    periodic_lifecycle::Options {
        progress_interval: 0,
        ratio_limit: 0.0,
        ..periodic_lifecycle::Options::default()
    }
}
#[test]
fn actual_hooks_resume_stop_or_retain_original_errors_under_real_pauses() {
    for width in [1, 3, 4, 8] {
        for (index, (expr, false_value, code)) in [
            ("TRUE", false, 0),
            ("FALSE", true, 0),
            ("1", false, 1000),
            ("\"text\"", false, 1000),
            ("{1}", false, 1000),
            ("<<1>>", false, 1000),
            ("[a |-> 1]", false, 1000),
            ("1 \\div 0", false, 2179),
            ("Token", false, 0),
        ]
        .into_iter()
        .enumerate()
        {
            let (m, p) = model(expr, "[]<>(x>=0)");
            let opts = CheckOptions {
                workers: width,
                chaos: Some(7),
                ..CheckOptions::tlc(0, 0)
            };
            let run =
                periodic_hook::execute(&m, &p, &opts, phase_graph::Limits::default(), policy())
                    .expect("admitted");
            record(&format!("value-{index}-w{width}"), &run);
            let e = run.hook_evaluations().first().expect("actual hook");
            assert_eq!(e.expression(), p.expr);
            assert_eq!(run.invocations()[0].workers(), width);
            if code != 0 {
                let error = e
                    .result()
                    .as_ref()
                    .expect_err("comparison/evaluation error");
                assert_eq!(error.ec().code(), code);
                assert!(error.origin().is_some());
                assert!(matches!(
                    run.backend().stop().expect("stop").error,
                    phase_graph::Error::PeriodicEvaluation(_)
                ));
                assert!(!run.invocations()[0].resume_requested());
                assert!(run.safety_eof().is_none());
            } else {
                assert_eq!(e.result().as_ref().expect("result"), &false_value);
                if false_value {
                    assert!(matches!(
                        run.invocations()[0].outcome(),
                        Ok(periodic::Outcome::PeriodicAssumptionFalse)
                    ));
                    assert!(!run.invocations()[0].resume_requested());
                    assert!(run.safety_eof().is_none());
                } else {
                    assert!(
                        run.invocations()
                            .iter()
                            .all(periodic_lifecycle::Invocation::resume_requested)
                    );
                    assert!(run.safety_eof().is_some());
                }
            }
            assert!(!run.model_proof());
        }
    }
}
#[test]
fn earlier_temporal_candidate_skips_false_hook_and_leaves_force_pending() {
    let (m, p) = model("FALSE", "<>(x=3)");
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let run = periodic_hook::execute(
            &m,
            &p,
            &opts,
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                force: true,
                progress_interval: 0,
                ..periodic_lifecycle::Options::default()
            },
        )
        .expect("admitted");
        record(&format!("live-first-w{width}"), &run);
        assert!(run.hook_evaluations().is_empty());
        assert_eq!(run.invocations()[0].force(), (true, true));
        assert!(matches!(
            run.invocations()[0].outcome(),
            Ok(periodic::Outcome::LivenessStopped(2116))
        ));
        assert!(run.safety_eof().is_none());
        let (fresh, _) = model("FALSE", "<>(x=3)");
        assert_fresh_candidate(&run, &fresh, width);
    }
}
#[test]
fn zero_state_guard_rejects_state_temporal_unknown_or_unbounded_before_execution() {
    for expr in [
        "x=0",
        "x'=0",
        "FALSE /\\ x=0",
        "<>(x=0)",
        "\\E i \\in 1..100 : i=0",
        "SUBSET {1}",
    ] {
        let (m, p) = model(expr, "[]<>(x>=0)");
        let err = periodic_hook::execute(
            &m,
            &p,
            &CheckOptions::tlc(0, 0),
            phase_graph::Limits::default(),
            policy(),
        )
        .expect_err("named refusal");
        assert_eq!(err.class(), dive_eval::ErrorClass::Unsupported);
        assert!(err.to_string().contains("DIVE-L0057"));
    }
    let (m, mut p) = model("TRUE", "[]<>(x>=0)");
    p.expr = dive_ir::ExprId(u32::MAX);
    assert!(
        periodic_hook::execute(
            &m,
            &p,
            &CheckOptions::tlc(0, 0),
            phase_graph::Limits::default(),
            policy()
        )
        .is_err()
    );
    let (_, foreign) = model("TRUE", "[]<>(x>=0)");
    assert!(
        periodic_hook::execute(
            &m,
            &foreign,
            &CheckOptions::tlc(0, 0),
            phase_graph::Limits::default(),
            policy()
        )
        .is_err()
    );
    let mut foreign_model = m.clone();
    foreign_model.names = Arc::new(dive_value::Interner::new());
    let (_, p) = model("TRUE", "[]<>(x>=0)");
    assert!(
        periodic_hook::execute(
            &foreign_model,
            &p,
            &CheckOptions::tlc(0, 0),
            phase_graph::Limits::default(),
            policy()
        )
        .is_err()
    );
}
#[test]
fn graph_quota_is_charged_before_hook_and_configured_binder_still_refuses() {
    let (m, p) = model("TRUE", "[]<>(x>=0)");
    let mut limits = phase_graph::Limits::default();
    limits.graph.steps = 1;
    let run = periodic_hook::execute(&m, &p, &CheckOptions::tlc(0, 0), limits, policy())
        .expect("admitted shape");
    assert!(run.hook_evaluations().is_empty());
    assert!(run.backend().stop().is_some());
    assert!(run.safety_eof().is_none());
    let cfg = dive_config::parse(
        "CONSTANT Token=Atom\nINIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\n",
    )
    .into_config()
    .expect("config");
    let error = dive_model::bind(m.ir.clone(), &cfg, &dive_model::BindOptions::tlc())
        .expect_err("still unsupported");
    assert!(error.to_string().contains("_PERIODIC"));
}

#[test]
fn captured_effects_unused_effect_arguments_and_redirects_do_not_print() {
    for body in [
        "PrintT(v)",
        "LET Ignore(ignored) == TRUE IN Ignore(PrintT(v))",
    ] {
        let source = format!(
            "---- MODULE EffectHook ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x>=0)\nHook(v) == {body}\n====\n"
        );
        let a =
            dive_sema::analyze_source("EffectHook.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let def = dive_ir::DefId(
            u32::try_from(
                ir.defs()
                    .iter()
                    .position(|d| d.name == "Hook")
                    .expect("Hook"),
            )
            .expect("id"),
        );
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
            .into_config()
            .expect("cfg");
        let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
        let root = m.evaluator.scope(def).expect("scope");
        let called = m
            .evaluator
            .scope_call_values(&root, def, vec![dive_value::Value::Bool(true)])
            .expect("captured scope");
        let p = Pred::new(m.ir.def(def).body, called);
        assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
        assert!(
            periodic_hook::execute(
                &m,
                &p,
                &CheckOptions::tlc(0, 0),
                phase_graph::Limits::default(),
                policy()
            )
            .is_err()
        );
        assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
    }
    let (m, p) = model("Token", "[]<>(x>=0)");
    let cfg = dive_config::parse(
        "CONSTANT Token <- Replacement\nINIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    let redirected = dive_model::bind(m.ir.clone(), &cfg, &dive_model::BindOptions::tlc())
        .expect("redirected model");
    let def = dive_ir::DefId(
        u32::try_from(
            m.ir.defs()
                .iter()
                .position(|d| d.name == "Hook")
                .expect("Hook"),
        )
        .expect("id"),
    );
    let redirected_pred = Pred::new(p.expr, redirected.evaluator.scope(def).expect("own scope"));
    assert!(
        periodic_hook::execute(
            &redirected,
            &redirected_pred,
            &CheckOptions::tlc(0, 0),
            phase_graph::Limits::default(),
            policy()
        )
        .is_err()
    );
}

fn assert_fresh_candidate(run: &periodic_lifecycle::Run, fresh: &dive_model::Model, width: usize) {
    let check = run
        .backend()
        .checks()
        .iter()
        .find(|c| c.selection().candidate().is_some())
        .expect("actual candidate");
    let (states, back_to) = concrete_word(run.backend(), check);
    assert!(
        !dive_trace::temporal::evaluate_lasso(
            fresh,
            &fresh.implied_temporals[0].pred,
            &states,
            back_to
        )
        .expect("independent word")[0]
    );
    let trace = dive_trace::Trace {
        labels: vec![None; states.len()],
        length: Some(states.len()),
        states,
        ending: dive_trace::Ending::Lasso { back_to },
        properties: vec!["Live".into()],
    };
    assert_eq!(
        dive_trace::validate(fresh, &trace, None).validation,
        dive_trace::Validation::Valid
    );
    let document = trace.to_doc(fresh).expect("document");
    let decoded = dive_trace::TraceDoc::from_json(
        &serde_json::from_slice(&serde_json::to_vec(&document.to_json()).expect("serialize"))
            .expect("json"),
    )
    .expect("decode");
    assert_eq!(
        dive_trace::validate_doc(fresh, &decoded, &[], None)
            .0
            .validation,
        dive_trace::Validation::Valid
    );
    let mut outcomes = Vec::new();
    for mutation in 0..4 {
        let mut bad = decoded.clone();
        match mutation {
            0 => bad.properties = vec!["Forged".into()],
            1 => {
                bad.states[0] =
                    serde_json::from_value(serde_json::json!({"x":99})).expect("wrong Init");
            }
            2 => {
                // The actual first-prefix candidate stutters at x=0.
                // Merely closing at index zero is therefore still valid.
                bad.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
                assert_eq!(
                    dive_trace::validate_doc(fresh, &bad, &[], None)
                        .0
                        .validation,
                    dive_trace::Validation::Valid
                );
                // 0 -> 1 satisfies Next, while closing 1 -> 0 does not.
                bad.states.push(
                    serde_json::from_value(serde_json::json!({"x":1}))
                        .expect("valid forward successor"),
                );
                bad.labels.push(None);
                bad.length = Some(bad.states.len());
            }
            _ => {
                bad.ending = Some(dive_trace::Ending::Lasso {
                    back_to: usize::MAX,
                });
            }
        }
        let outcome = dive_trace::validate_doc(fresh, &bad, &[], None)
            .0
            .validation;
        assert_ne!(outcome, dive_trace::Validation::Valid);
        if mutation == 2 {
            assert!(format!("{outcome:?}").contains("Next"));
        }
        outcomes.push(format!("{outcome:?}"));
    }
    if let Some(dir) = std::env::var_os("DIVE_PERIODIC_HOOK_CAPTURE") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("capture dir");
        let row = serde_json::json!({"width":width,"producer_profile":"tlc","fresh_validator_profile":"tlc","memory":"Valid","json":"Valid","wrong_property":outcomes[0],"wrong_init":outcomes[1],"wrong_closing_next":outcomes[2],"malformed_ending":outcomes[3],"model_proof":false,"ordinary_schedule_certified":false});
        std::fs::write(
            dir.join(format!("replay-w{width}.json")),
            serde_json::to_vec_pretty(&row).expect("json"),
        )
        .expect("capture replay");
    }
}

fn record(name: &str, run: &periodic_lifecycle::Run) {
    let Some(dir) = std::env::var_os("DIVE_PERIODIC_HOOK_CAPTURE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("capture dir");
    let hooks:Vec<_>=run.hook_evaluations().iter().map(|h| match h.result() {
        Ok(value)=>serde_json::json!({"source":h.source(),"expr":h.expression().0,"false_equality":value}),
        Err(error)=>serde_json::json!({"source":h.source(),"expr":h.expression().0,"error_code":error.ec().code(),"error_class":format!("{:?}",error.class()),"error_text":error.message(),"eval_exception":error.is_eval_exception(),"origin":error.origin().map(|id|id.0)}),
    }).collect();
    let invocations:Vec<_>=run.invocations().iter().map(|i|serde_json::json!({"source":i.source(),"actual_workers":i.workers(),"generation":i.generation(),"stats":format!("{:?}",i.stats()),"outcome":format!("{:?}",i.outcome()),"resume":i.resume_requested(),"force":i.force(),"clock_millis":i.time_millis()})).collect();
    let row = serde_json::json!({"name":name,"supplied_expression":true,"configured_hook":false,"ordinary_schedule_certified":false,"model_proof":run.model_proof(),"hooks":hooks,"invocations":invocations,"safety_eof":run.safety_eof().map(|s|format!("{s:?}")),"stop":run.backend().stop().map(|s|s.error.to_string())});
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&row).expect("json"),
    )
    .expect("capture row");
}

fn concrete_word(
    run: &dive_liveness::periodic_backend::Run,
    check: &dive_liveness::phase_execution::PartialCheck,
) -> (Vec<dive_eval::next::StateVals>, usize) {
    use std::collections::BTreeMap;
    let c = check.selection().candidate().expect("candidate");
    let ids = check.identities();
    let groups = run
        .stop()
        .map(|s| &s.groups)
        .or_else(|| run.complete().map(|s| &s.groups))
        .expect("actual groups");
    let representatives: BTreeMap<_, _> = groups[check.group()]
        .commits
        .iter()
        .map(|commit| {
            let original = commit
                .receipt
                .successors
                .iter()
                .find(|s| s.fingerprint == commit.fingerprint)
                .expect("actual source stutter");
            (commit.fingerprint, original.state.clone())
        })
        .collect();
    let mut prefix = Vec::new();
    for i in c.prefix_reverse.iter().rev() {
        let fp = ids[*i].fingerprint;
        if prefix.last() != Some(&fp) {
            prefix.push(fp);
        }
    }
    let anchor = ids[c.anchor].fingerprint;
    assert_eq!(prefix.last(), Some(&anchor));
    let back = prefix.len() - 1;
    let mut reverse = c.closure_reverse.clone();
    for i in c.cycle_stack.iter().rev() {
        let fp = ids[*i].fingerprint;
        if reverse.last() != Some(&fp) {
            reverse.push(fp);
        }
    }
    reverse.dedup();
    if reverse.last() == Some(&anchor) {
        reverse.pop();
    }
    prefix.extend(reverse.into_iter().rev());
    (
        prefix
            .into_iter()
            .map(|fp| representatives[&fp].clone())
            .collect(),
        back,
    )
}
