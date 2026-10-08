//! The harness computes `trace_validated` itself (ADR 0016 §3): dive's
//! counterexample is read from the result and validated in-process, and a
//! deadlock is accepted only when the oracle's own counterexample ends in
//! the same state.

#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

use std::path::PathBuf;

use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions};
use dive_conformance::tlc::result::OracleResult;
use dive_conformance::trace_check::{tlc_document, validate_counterexample};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tlc() -> TraceOptions {
    TraceOptions {
        compat_tlc: true,
        ..TraceOptions::default()
    }
}

fn golden(path: &str) -> OracleResult {
    let text = std::fs::read_to_string(repo().join(path)).expect("golden");
    let v: serde_json::Value = serde_json::from_str(&text).expect("json");
    serde_json::from_value(v["result"].clone()).expect("result")
}

/// `result` with its counterexample replaced by `doc` (as dive would emit it).
fn with_doc(result: &OracleResult, doc: &TraceDoc) -> OracleResult {
    let mut r = result.clone();
    if let Some(t) = r.trace.as_mut() {
        t.json = Some(doc.to_json());
    }
    r
}

#[test]
fn an_invariant_counterexample_is_validated_by_the_harness() {
    let g = golden(
        "conformance/golden/tlc-2026.10.06.014338/examples/specifications/DieHard/DieHard.cfg.json",
    );
    let dir = repo().join("crates/dive-eval/tests/fixtures/examples/specifications/DieHard");
    let spec =
        SpecModel::load(&dir.join("DieHard.tla"), &dir.join("DieHard.cfg"), &tlc()).expect("model");
    let (_, trace) = spec.read(&tlc_document(&g).expect("TLC document"));
    let trace = trace.expect("trace");
    let doc = spec.to_doc(&trace).expect("doc");

    let ok = validate_counterexample(&spec, &with_doc(&g, &doc), &g);
    assert!(ok.validated, "{}", ok.detail);

    // A tampered state.
    let mut bad = doc.clone();
    bad.states[3].insert("big".to_owned(), serde_json::json!(4));
    let no = validate_counterexample(&spec, &with_doc(&g, &bad), &g);
    assert!(!no.validated);
    assert!(no.detail.contains("invalid"), "{}", no.detail);

    // A document that claims something else than the result reports.
    let other = doc.clone().with_ending(Ending::Invariant("TypeOK".into()));
    assert!(!validate_counterexample(&spec, &with_doc(&g, &other), &g).validated);

    // TLC's own document is not dive's: it carries no claim.
    assert!(!validate_counterexample(&spec, &g, &g).validated);
}

#[test]
fn a_deadlock_is_confirmed_only_by_the_oracles_last_state() {
    let probes = repo().join("crates/dive-check/tests/fixtures/tlc-probes");
    let dir = probes.join("specs/DeadlockMidLevel");
    let spec = SpecModel::load(
        &dir.join("DeadlockMidLevel.tla"),
        &dir.join("DeadlockMidLevel.cfg"),
        &tlc(),
    )
    .expect("model");
    let tlc_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(probes.join("tlc-2026.10.06.014338/DeadlockMidLevel.trace.json"))
            .expect("trace"),
    )
    .expect("json");
    // The probe's result, in the golden vocabulary.
    let mut g = golden(
        "conformance/golden/tlc-2026.10.06.014338/examples/specifications/DieHard/DieHard.cfg.json",
    );
    g.error_class = Some("TLC_DEADLOCK_REACHED".to_owned());
    g.violated_property = None;
    let t = g.trace.as_mut().expect("trace");
    t.length = 2;
    t.json = Some(tlc_json);

    let (_, trace) = spec.read(&tlc_document(&g).expect("TLC document"));
    let doc = spec.to_doc(&trace.expect("trace")).expect("doc");
    let ok = validate_counterexample(&spec, &with_doc(&g, &doc), &g);
    assert!(ok.validated, "{}", ok.detail);

    // `0 -> 1` is a behavior, and the validator cannot refute the claim that
    // 1 is deadlocked, but the oracle's deadlocked state is 2.
    let mut wrong = doc.clone();
    wrong.states[1].insert("x".to_owned(), serde_json::json!(1));
    let no = validate_counterexample(&spec, &with_doc(&g, &wrong), &g);
    assert!(!no.validated);
    assert!(no.detail.contains("differs"), "{}", no.detail);

    // Without the oracle's document, the claim cannot be confirmed.
    let mut blind = g.clone();
    if let Some(t) = blind.trace.as_mut() {
        t.json = None;
        t.json_unavailable = Some("issue #35".to_owned());
    }
    let no = validate_counterexample(&spec, &with_doc(&g, &doc), &blind);
    assert!(!no.validated);
    assert!(no.detail.contains("cannot confirm"), "{}", no.detail);
}
