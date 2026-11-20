//! Recorded existential witnesses (`"witnesses"`, D-040): simulation draws a
//! member of an infinite set for `\E n \in Nat`, and the validator may try
//! the recorded members to prove that existential true. They are untrusted
//! hints: a non-member is never tried, a witness that does not satisfy the
//! body proves nothing, and without one that proves it the step stays
//! unsupported, never valid.
#![allow(clippy::expect_used, clippy::panic, reason = "fixed test sources")]

use dive_trace::{Ending, Session, TraceDoc, Validation};
use serde_json::json;
use std::sync::Arc;

const SOURCE: &str = "---- MODULE W ----\nEXTENDS Integers\nVARIABLE x\nInit == \\E n \\in Nat : x = 2 * n\nNext == \\E n \\in Nat, m \\in {0, 1} : x' = x + n + m\nInv == ~(\\E n \\in Nat : x = n /\\ n > 100)\n====\n";

fn session(options: dive_eval::Options) -> Session {
    let analysis = dive_sema::analyze_source("W.tla", SOURCE, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    Session::new(dive_model::Recipe::new(ir, config, options, false), None)
}

fn word(xs: &[i64]) -> TraceDoc {
    TraceDoc::behavior(xs.iter().map(|x| json!({ "x": x })).collect())
        .expect("word")
        .with_ending(Ending::Behavior)
}

fn sampled(session: &Session, doc: &TraceDoc) -> Validation {
    session
        .read_sampled(doc, None)
        .expect("completed replay")
        .0
        .validation
}

#[test]
fn witnesses_prove_existentials_over_infinite_sets() {
    for options in [dive_eval::Options::dive(), dive_eval::Options::tlc()] {
        let session = session(options);
        let proved = word(&[4, 9, 9])
            .with_witnesses(0, vec![json!(2)])
            .with_witnesses(1, vec![json!(5)])
            .with_witnesses(2, vec![json!(0)]);
        assert_eq!(sampled(&session, &proved), Validation::Valid);
        // The document form keeps them.
        let text = proved.to_json().to_string();
        let read = TraceDoc::parse(&text).expect("document");
        assert_eq!(read.witnesses, proved.witnesses);
        assert_eq!(sampled(&session, &read), Validation::Valid);
    }
}

#[test]
fn witnesses_never_prove_what_they_do_not_show() {
    let session = session(dive_eval::Options::dive());
    // No witnesses: the `\E` over Nat cannot be decided.
    assert!(!matches!(
        sampled(&session, &word(&[4, 9])),
        Validation::Valid
    ));
    // A witness that does not satisfy the body proves nothing.
    let wrong = word(&[4, 9])
        .with_witnesses(0, vec![json!(2)])
        .with_witnesses(1, vec![json!(3)]);
    assert!(!matches!(sampled(&session, &wrong), Validation::Valid));
    // A step down needs n = -5, which is not in Nat, so it is never tried.
    let negative = word(&[4, 0])
        .with_witnesses(0, vec![json!(2)])
        .with_witnesses(1, vec![json!(-5), json!(-4)]);
    assert!(!matches!(sampled(&session, &negative), Validation::Valid));
    // A witness of another step does not leak into this one.
    let elsewhere = word(&[4, 9, 14])
        .with_witnesses(0, vec![json!(2)])
        .with_witnesses(1, vec![json!(5)]);
    assert!(!matches!(sampled(&session, &elsewhere), Validation::Valid));
    // An odd initial state has no witness in Nat however many are offered.
    let odd = word(&[3]).with_witnesses(0, vec![json!(1), json!(2), json!(1.5)]);
    assert!(!matches!(sampled(&session, &odd), Validation::Valid));
}
