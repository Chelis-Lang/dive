//! A simulation deadlock is a state none of whose successors lies inside the
//! model's constraints (issue #896); an exhaustive deadlock has no successor.
#![allow(clippy::expect_used, clippy::panic, reason = "fixed test sources")]

use dive_eval::{HostControl, Interrupted};
use dive_trace::{Check, Ending, Session, TraceDoc, Validation};
use std::sync::Arc;

#[derive(Debug)]
struct Continue;
impl HostControl for Continue {
    fn check(&self) -> Result<(), Interrupted> {
        Ok(())
    }
}

const SOURCE: &str = "---- MODULE D ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nC == x < 3\nA == x' < 3\nL == TLCGet(\"level\") < 4\n====\n";

fn session(constraint: &str, options: dive_eval::Options) -> Session {
    let analysis = dive_sema::analyze_source("D.tla", SOURCE, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(&format!("INIT Init\nNEXT Next\n{constraint}\n"))
        .into_config()
        .expect("config");
    Session::new(dive_model::Recipe::new(ir, config, options, false), None)
}

fn deadlock(last: i64) -> TraceDoc {
    TraceDoc::behavior((0..=last).map(|x| serde_json::json!({ "x": x })).collect())
        .expect("word")
        .with_ending(Ending::Deadlock)
}

fn sampled(session: &Session, doc: &TraceDoc) -> Validation {
    session
        .read_sampled_deadlock(doc, None)
        .expect("completed replay")
        .0
        .validation
}

#[test]
fn sampled_deadlock_requires_every_successor_outside_the_constraints() {
    for options in [dive_eval::Options::dive(), dive_eval::Options::tlc()] {
        for constraint in ["CONSTRAINT C", "ACTION_CONSTRAINT A", "CONSTRAINT L"] {
            let session = session(constraint, options);
            assert_eq!(
                sampled(&session, &deadlock(2)),
                Validation::Valid,
                "{constraint}"
            );
            // x = 2 is inside the model, so x = 1 is not a simulation deadlock.
            match sampled(&session, &deadlock(1)) {
                Validation::Invalid(failure) => {
                    assert_eq!((failure.state, failure.check), (1, Check::Ending));
                }
                other => panic!("{constraint}: {other:?}"),
            }
            // Exhaustive checking keeps the successor-free deadlock.
            assert!(matches!(
                session
                    .read_controlled(&deadlock(2), Arc::new(Continue))
                    .expect("completed replay")
                    .0
                    .validation,
                Validation::Invalid(_)
            ));
        }
        // Without constraints a deadlock still needs no successor at all.
        let session = session("", options);
        assert!(matches!(
            sampled(&session, &deadlock(2)),
            Validation::Invalid(_)
        ));
        // The operation never accepts another ending.
        let behavior = TraceDoc::behavior(vec![serde_json::json!({ "x": 0 })])
            .expect("word")
            .with_ending(Ending::Behavior);
        assert!(matches!(
            sampled(&session, &behavior),
            Validation::Invalid(_)
        ));
    }
}

fn terminal(last: i64) -> TraceDoc {
    TraceDoc::behavior((0..=last).map(|x| serde_json::json!({ "x": x })).collect())
        .expect("word")
        .with_ending(Ending::Terminal)
}

/// With deadlock checking off, TLC's simulator ends a trace at a state with
/// no admitted successor (issue #1424); the `terminal` word is certified as
/// a sampled deadlock is, and is invalid where deadlock checking is on.
#[test]
fn terminal_word_requires_deadlock_checking_off_and_no_admitted_successor() {
    for options in [dive_eval::Options::dive(), dive_eval::Options::tlc()] {
        for constraint in ["CONSTRAINT C", "ACTION_CONSTRAINT A", "CONSTRAINT L"] {
            let off = session(&format!("{constraint}\nCHECK_DEADLOCK FALSE"), options);
            assert_eq!(
                sampled(&off, &terminal(2)),
                Validation::Valid,
                "{constraint}"
            );
            match sampled(&off, &terminal(1)) {
                Validation::Invalid(failure) => {
                    assert_eq!((failure.state, failure.check), (1, Check::Ending));
                    assert!(failure.detail.contains("terminal state"), "{failure:?}");
                }
                other => panic!("{constraint}: {other:?}"),
            }
            // With deadlock checking on, that state is a deadlock instead.
            let on = session(constraint, options);
            assert!(matches!(sampled(&on, &terminal(2)), Validation::Invalid(_)));
            assert_eq!(sampled(&on, &deadlock(2)), Validation::Valid);
        }
        // Without constraints a terminal state has no successor at all.
        let off = session("CHECK_DEADLOCK FALSE", options);
        assert!(matches!(
            sampled(&off, &terminal(2)),
            Validation::Invalid(_)
        ));
        // The plain sample operation keeps its `behavior` ending only.
        assert!(matches!(
            off.read_sampled(&terminal(2), None)
                .expect("completed replay")
                .0
                .validation,
            Validation::Invalid(_)
        ));
    }
}
