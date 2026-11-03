//! Finite random support is proved from fresh source, not a producer RNG tape.
#![allow(
    clippy::expect_used,
    reason = "source fixtures and exact result assertions"
)]

use std::sync::Arc;

use dive_trace::{Session, TraceDoc, Validation};
use serde_json::json;

fn session(source: &str, cfg: &str, tlc: bool) -> Session {
    let analysis = dive_sema::analyze_source("Choices.tla", source, &dive_sema::Options::default());
    assert!(
        analysis.accepted(),
        "{:?} {:?}",
        analysis.diagnostics(),
        analysis.syntax_diagnostics()
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(cfg)
        .into_config()
        .expect("configuration");
    Session::new(
        dive_model::Recipe::new(
            ir,
            cfg,
            if tlc {
                dive_eval::Options::tlc()
            } else {
                dive_eval::Options::dive()
            },
            false,
        ),
        None,
    )
}

fn checked(session: &Session, rows: Vec<serde_json::Value>) -> Validation {
    let doc = TraceDoc::behavior(rows).expect("native word");
    session
        .read_sampled(&doc, None)
        .expect("completed validation")
        .0
        .validation
}

fn large_stack(test: impl FnOnce() + Send) {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, test)
            .expect("stack")
            .join()
            .expect("test");
    });
}

#[test]
fn multivariable_parameterized_and_compound_choices_are_source_relations() {
    large_stack(|| {
        for tlc in [false, true] {
            let source = "---- MODULE Choices ----\nEXTENDS Naturals,TLC\nVARIABLES a,b\nPick(S)==RandomElement(S)\nInit==a=Pick({0,1}) /\\ b=Pick({<<a,2>>,<<a,3>>})\nNext==a'=Pick({0,1}) /\\ b' = <<a',Pick({2,3})>>\n====\n";
            let s = session(source, "INIT Init\nNEXT Next\n", tlc);
            assert_eq!(
                checked(
                    &s,
                    vec![
                        json!({"a":1,"b":{"seq":[1,2]}}),
                        json!({"a":0,"b":{"seq":[0,3]}})
                    ]
                ),
                Validation::Valid
            );
            assert!(matches!(
                checked(&s, vec![json!({"a":1,"b":{"seq":[0,2]}})]),
                Validation::Invalid(_)
            ));
            assert!(matches!(
                checked(
                    &s,
                    vec![
                        json!({"a":1,"b":{"seq":[1,2]}}),
                        json!({"a":0,"b":{"seq":[1,3]}})
                    ]
                ),
                Validation::Invalid(_)
            ));
        }
    });
}

#[test]
fn startup_choices_and_state_correlations_are_replayed_from_fresh_source() {
    large_stack(|| {
        for tlc in [false, true] {
            let source = "---- MODULE Choices ----\nEXTENDS TLC\nCONSTANT C\nVARIABLES a,b\nPick==RandomElement({1,2})\nInit==a=C /\\ b=a\nNext==UNCHANGED <<a,b>>\n====\n";
            let s = session(source, "CONSTANT C <- Pick\nINIT Init\nNEXT Next\n", tlc);
            assert_eq!(
                checked(&s, vec![json!({"a":2,"b":2}), json!({"a":2,"b":2})]),
                Validation::Valid
            );
            let forged = checked(&s, vec![json!({"a":1,"b":2})]);
            assert!(matches!(forged, Validation::Invalid(_)), "{forged:?}");
            assert!(matches!(
                checked(&s, vec![json!({"a":1,"b":1}), json!({"a":2,"b":2})]),
                Validation::Invalid(_)
            ));
        }
    });
}

#[test]
fn incomplete_assignment_demand_cannot_borrow_the_supplied_state() {
    large_stack(|| {
        for tlc in [false, true] {
            let source = "---- MODULE Choices ----\nEXTENDS TLC\nVARIABLES a,b\nInit==a=RandomElement({b}) /\\ b=1\nNext==UNCHANGED <<a,b>>\n====\n";
            let s = session(source, "INIT Init\nNEXT Next\n", tlc);
            assert!(!checked(&s, vec![json!({"a":1,"b":1})]).is_valid());
        }
    });
}

#[test]
fn random_and_register_prefixes_remain_explicitly_unproved() {
    large_stack(|| {
        for tlc in [false, true] {
            for init in [
                "a=RandomElement({0,1}) /\\ TLCSet(0,a)",
                "TLCSet(0,1) /\\ a=RandomElement({0,1})",
            ] {
                let source = format!(
                    "---- MODULE Choices ----\nEXTENDS TLC\nVARIABLE a\nInit=={init}\nNext==UNCHANGED a\n====\n"
                );
                let s = session(&source, "INIT Init\nNEXT Next\n", tlc);
                assert!(matches!(
                    checked(&s, vec![json!({"a":1})]),
                    Validation::Unsupported(_)
                ));
            }
        }
    });
}

#[derive(Debug)]
struct StopAfter {
    calls: std::sync::atomic::AtomicU64,
    limit: u64,
}
impl dive_eval::HostControl for StopAfter {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        use std::sync::atomic::Ordering;
        if self.calls.fetch_add(1, Ordering::Relaxed) >= self.limit {
            Err(dive_eval::Interrupted::new(145))
        } else {
            Ok(())
        }
    }
}

#[test]
fn backtracking_fresh_binding_preserves_typed_control_interruption() {
    large_stack(|| {
        let source = "---- MODULE Choices ----\nEXTENDS TLC\nVARIABLE x\nInit==x=RandomElement({<<0>>,<<1>>,<<2>>})\nNext==UNCHANGED x\n====\n";
        let s = session(source, "INIT Init\nNEXT Next\n", true);
        let count = Arc::new(StopAfter {
            calls: std::sync::atomic::AtomicU64::new(0),
            limit: u64::MAX,
        });
        let first = TraceDoc::behavior(vec![json!({"x":{"seq":[0]}})]).expect("first member");
        assert!(
            s.read_sampled(&first, Some(count.clone()))
                .expect("first attempt")
                .0
                .validation
                .is_valid()
        );
        let limit = count.calls.load(std::sync::atomic::Ordering::Relaxed) + 1;
        let later = TraceDoc::behavior(vec![json!({"x":{"seq":[2]}})]).expect("later member");
        let stop = Arc::new(StopAfter {
            calls: std::sync::atomic::AtomicU64::new(0),
            limit,
        });
        let result = s.read_sampled(&later, Some(stop));
        assert!(
            matches!(result,Err(dive_trace::ValidationRunError::Interrupted { error,.. }) if error.interruption().is_some_and(|interruption|interruption.token()==145))
        );
    });
}

#[test]
fn native_constructor_support_preserves_component_and_empty_domain_demand() {
    large_stack(|| {
        for tlc in [false, true] {
            for (domain, value) in [
                ("[{} -> {i \\in Nat : TRUE}]", json!({"seq":[]})),
                (
                    "[a:{1,2},b:{TRUE,FALSE}]",
                    json!({"record":{"a":2,"b":true}}),
                ),
                ("{1,2} \\X {3,4}", json!({"seq":[2,4]})),
                (
                    "[{0,1} -> {TRUE,FALSE}]",
                    json!({"fcn":[[0,true],[1,false]]}),
                ),
            ] {
                let source = format!(
                    "---- MODULE Choices ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit==x=RandomElement({domain})\nNext==UNCHANGED x\n====\n"
                );
                let s = session(&source, "INIT Init\nNEXT Next\n", tlc);
                assert_eq!(
                    checked(&s, vec![json!({"x":value})]),
                    Validation::Valid,
                    "{domain}"
                );
            }
        }
    });
}

#[test]
fn reached_randomness_inside_enabled_cannot_certify_a_finite_word() {
    large_stack(|| {
        for tlc in [false, true] {
            let source = "---- MODULE Choices ----\nEXTENDS TLC\nVARIABLE x\nInit==x=0\nNext==ENABLED (x'=RandomElement({0,1})) /\\ x'=1\n====\n";
            let s = session(source, "INIT Init\nNEXT Next\n", tlc);
            let result = checked(&s, vec![json!({"x":0}), json!({"x":1})]);
            assert!(
                matches!(result,Validation::Unsupported(ref why) if why.contains("ENABLED")),
                "{result:?}"
            );
        }
    });
}

#[test]
fn value_expression_let_keeps_its_actual_cached_choice_correlation() {
    large_stack(|| {
        for tlc in [false, true] {
            let source = "---- MODULE Choices ----\nEXTENDS TLC\nVARIABLE x\nInit==x = <<0,0>>\nNext==x' = LET r==RandomElement({0,1}) IN <<r,r>>\n====\n";
            let s = session(source, "INIT Init\nNEXT Next\n", tlc);
            assert_eq!(
                checked(
                    &s,
                    vec![json!({"x":{"seq":[0,0]}}), json!({"x":{"seq":[1,1]}})]
                ),
                Validation::Valid
            );
            assert!(matches!(
                checked(
                    &s,
                    vec![json!({"x":{"seq":[0,0]}}), json!({"x":{"seq":[0,1]}})]
                ),
                Validation::Invalid(_)
            ));
        }
    });
}

#[test]
fn bound_action_captures_cannot_change_within_or_between_steps() {
    large_stack(|| {
        for tlc in [false, true] {
            let source = include_str!(
                "../../dive-sim/tests/fixtures/finite-choices/bound-action/Choices.tla"
            );
            let config = include_str!(
                "../../dive-sim/tests/fixtures/finite-choices/bound-action/Choices.cfg"
            );
            let s = session(source, config, tlc);
            assert_eq!(
                checked(
                    &s,
                    vec![
                        json!({"a":0,"b":0}),
                        json!({"a":1,"b":1}),
                        json!({"a":1,"b":1})
                    ]
                ),
                Validation::Valid
            );
            assert!(matches!(
                checked(&s, vec![json!({"a":0,"b":0}), json!({"a":0,"b":1})]),
                Validation::Invalid(_)
            ));
            assert!(matches!(
                checked(
                    &s,
                    vec![
                        json!({"a":0,"b":0}),
                        json!({"a":0,"b":0}),
                        json!({"a":1,"b":1})
                    ]
                ),
                Validation::Invalid(_)
            ));
        }
    });
}

#[test]
fn ordinary_pure_validation_does_not_expand_a_million_bound_actions() {
    large_stack(|| {
        let source = "---- MODULE Choices ----\nEXTENDS Naturals\nVARIABLE x\nInit==x=0\nNext==\\E n \\in 1..1000000 : x'=n\n====\n";
        let s = session(source, "INIT Init\nNEXT Next\n", true);
        assert_eq!(
            checked(&s, vec![json!({"x":0}), json!({"x":1})]),
            Validation::Valid
        );
    });
}
