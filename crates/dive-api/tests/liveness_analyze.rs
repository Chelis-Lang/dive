//! Final temporal checking on Core versus pinned TLC counts, with independent
//! word validation. These ran on the retired reference final analysis
//! (`dive_liveness::analyze::analyze_final`) until #791. Exact seen storage
//! with temporal properties is a named Core refusal.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use core_final::Outcome;
use dive_runtime::CheckOptions;
use dive_trace::{Ending, TraceDoc, Validation};
use dive_value::Value;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn probes() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes")
}

/// The bound model independent validation replays words against.
fn bound(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    let name = source.split_whitespace().nth(2).expect("name");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            symmetry_liveness: opts.symmetry_liveness,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
    .expect("bind")
}

fn counters(stdout: &str) -> (u64, u64, u64) {
    let states = stdout
        .lines()
        .find(|s| s.contains("states generated,") && s.contains("distinct states found"))
        .expect("statistics");
    let numbers: Vec<u64> = states
        .split_whitespace()
        .filter_map(|s| s.replace(',', "").parse().ok())
        .collect();
    let depth = stdout
        .lines()
        .find(|s| s.contains("depth of the complete state graph search"))
        .expect("depth")
        .split_whitespace()
        .last()
        .expect("number")
        .trim_end_matches('.')
        .parse()
        .expect("depth number");
    (numbers[0], numbers[1], depth)
}

fn exact_store_is_refused(source: &str, config: &str, opts: &CheckOptions) {
    let exact = CheckOptions {
        store: dive_store::StoreMode::Exact,
        ..opts.clone()
    };
    let report = core_final::run(source, config, &exact);
    refused::assert_refused(
        &report.to_json(),
        report.exit_code(),
        refused::TEMPORAL_EXACT_STORE,
    );
}

#[test]
fn pinned_pure_and_fair_models_have_valid_candidates_and_tlc_counts() {
    let root = probes();
    for name in [
        "LvNoFair",
        "LvAlwaysEventually",
        "LvEventuallyAlways",
        "LvLeadsTo",
        "LvActionProp",
        "LvLassoMid",
        "LvEAPrefix",
        "LvStutterActionConstraint",
        "LvWFIntermittent",
        "LvStutterEnd",
    ] {
        let source =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.tla")))
                .expect("source");
        let config =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.cfg")))
                .expect("config");
        let stdout = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .expect("oracle");
        let opts = CheckOptions {
            workers: 8,
            ..CheckOptions::tlc(0, 0)
        };
        exact_store_is_refused(&source, &config, &opts);
        let run =
            core_final::check(&source, &config, &opts).unwrap_or_else(|s| panic!("{name}: {s}"));
        let Outcome::Violation(v) = run.outcome() else {
            panic!("{name}: must find a counterexample: {:?}", run.outcome());
        };
        let c = v.candidate();
        assert_eq!(
            (
                c.exploration.stats.generated,
                c.exploration.stats.distinct,
                c.exploration.stats.depth
            ),
            counters(&stdout),
            "{name}"
        );
        assert!(!c.properties.is_empty(), "{name}");
        assert_eq!(v.validation().validation, Validation::Valid, "{name}");
        let model = bound(&source, &config, &opts);
        let vars = dive_trace::variables(&model);
        let mut states: Vec<serde_json::Value> = c
            .states
            .iter()
            .map(|vals| {
                let values: serde_json::Map<String, serde_json::Value> = vars
                    .iter()
                    .zip(vals.iter())
                    .map(|(name, value)| {
                        (
                            name.clone(),
                            dive_trace::value_json::to_json(
                                value.as_ref().expect("value"),
                                opts.eval.ctx,
                            )
                            .expect("JSON value"),
                        )
                    })
                    .collect();
                serde_json::Value::Object(values)
            })
            .collect();
        let doc = TraceDoc::behavior(states.clone())
            .expect("word")
            .with_ending(Ending::Lasso { back_to: c.back_to })
            .with_properties(c.properties.clone());
        assert_eq!(
            dive_trace::validate_doc(&model, &doc, &[]).0.validation,
            Validation::Valid,
            "{name}"
        );
        states[0]["x"] = json!(99999);
        let broken = TraceDoc::behavior(states)
            .expect("mutant")
            .with_ending(Ending::Lasso { back_to: c.back_to })
            .with_properties(doc.properties.clone());
        assert_ne!(
            dive_trace::validate_doc(&model, &broken, &[]).0.validation,
            Validation::Valid,
            "{name}: mutated Init"
        );
    }
}

#[test]
fn pure_behavior_conjuncts_cannot_be_dropped_and_holds_is_exhaustive() {
    let opts = CheckOptions::tlc(0, 0);
    let source = "---- MODULE Analyze ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n\
        Spec == Init /\\ [][Next]_x /\\ []<>x\nProp == []<>x\n====\n";
    let cfg = "SPECIFICATION Spec\nPROPERTY Prop\n";
    let run = core_final::check(source, cfg, &opts).unwrap_or_else(|s| panic!("{s}"));
    assert!(
        matches!(run.outcome(), Outcome::Holds(_)),
        "{:?}",
        run.outcome()
    );
    let no_behavior = source.replace(" /\\ []<>x", "");
    let run = core_final::check(&no_behavior, cfg, &opts).unwrap_or_else(|s| panic!("{s}"));
    assert!(
        matches!(run.outcome(), Outcome::Violation(_)),
        "{:?}",
        run.outcome()
    );
}

#[test]
fn fairness_and_safety_stops_do_not_guess_holds() {
    let opts = CheckOptions::tlc(0, 0);
    let cfg = "SPECIFICATION Spec\nPROPERTY Prop\n";
    let source = "---- MODULE Analyze ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n\
        Spec == Init /\\ [][Next]_x /\\ WF_x(Next)\nProp == []<>x\nInv == ~x\n====\n";
    let run = core_final::check(source, cfg, &opts).unwrap_or_else(|s| panic!("{s}"));
    assert!(matches!(run.outcome(), Outcome::Holds(_)), "fair toggle");
    let opaque = source
        .replace("WF_x(Next)", "F(Next)")
        .replace("Spec ==", "F(a) == WF_x(a)\nSpec ==");
    let run = core_final::check(&opaque, cfg, &opts).unwrap_or_else(|s| panic!("{s}"));
    let Outcome::Holds(c) = run.outcome() else {
        panic!("fair alias changed truth: {:?}", run.outcome());
    };
    assert_eq!(
        (c.stats.generated, c.stats.distinct, c.stats.depth),
        (3, 2, 2)
    );
    // A fairness conjunct over a live checker read is not planned: no verdict.
    let opaque = source
        .replace("VARIABLE x", "EXTENDS TLC\nVARIABLE x")
        .replace("WF_x(Next)", "[]<>(TLCGet(\"level\")=0)");
    let stop = core_final::check(&opaque, cfg, &opts).expect_err("unplanned fairness");
    assert_eq!(stop.verdict(), "unsupported", "{}", stop.json());
    assert_eq!(stop.exit_code(), 3, "{}", stop.json());
    assert!(stop.json()["counterexample"].is_null());
    let source = source.replace(" /\\ WF_x(Next)", "");
    let run = core_final::check(
        &source,
        "SPECIFICATION Spec\nPROPERTY Prop\nINVARIANT Inv\n",
        &opts,
    )
    .unwrap_or_else(|s| panic!("{s}"));
    let Outcome::Safety(result) = run.outcome() else {
        panic!("the invariant stops the run first: {:?}", run.outcome());
    };
    assert!(
        matches!(
            result.verdict,
            dive_runtime::Verdict::InvariantViolated { .. }
        ),
        "{:?}",
        result.verdict
    );
}

/// Core does not certify this property's state source row; the retired
/// reference analysis reported the atom's evaluation error (DIVE-L0033).
const UNCERTIFIED_SOURCE_ROW: &str =
    "singleton temporal source-row primary constructor is not certified";

#[test]
fn an_atom_error_is_a_named_refusal_not_a_verdict() {
    let report = core_final::run(
        "---- MODULE Analyze ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
        Next == x' = x\nProp == []<>(1 \\div x = 0)\n====\n",
        "INIT Init\nNEXT Next\nPROPERTY Prop\n",
        &CheckOptions::tlc(0, 0),
    );
    refused::assert_refused(
        &report.to_json(),
        report.exit_code(),
        UNCERTIFIED_SOURCE_ROW,
    );
}

#[test]
fn every_false_temporal_property_is_named_on_the_selected_lasso() {
    let opts = CheckOptions::tlc(0, 0);
    let source = "---- MODULE Claims ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\nQ == <>[]~x\nR == []<>x\n====\n";
    let cfg = "SPECIFICATION Spec\nPROPERTY P Q R\n";
    let run = core_final::check(source, cfg, &opts).unwrap_or_else(|s| panic!("{s}"));
    let Outcome::Violation(v) = run.outcome() else {
        panic!("violations: {:?}", run.outcome())
    };
    let c = v.candidate();
    assert_eq!(c.properties, vec!["P", "Q"]);
    let model = bound(source, cfg, &opts);
    let values = c
        .states
        .iter()
        .map(|s| json!({"x":s[0].as_ref().expect("x")==&Value::Bool(true)}))
        .collect();
    let doc = TraceDoc::behavior(values)
        .expect("doc")
        .with_ending(Ending::Lasso { back_to: c.back_to })
        .with_properties(c.properties.clone());
    assert_eq!(
        dive_trace::validate_doc(&model, &doc, &[]).0.validation,
        Validation::Valid
    );
    let omitted = doc.clone().with_properties(vec!["P".into()]);
    assert!(
        matches!(dive_trace::validate_doc(&model,&omitted,&[]).0.validation,Validation::Invalid(f) if f.check==dive_trace::Check::Temporal)
    );
    let false_claim = doc.with_properties(vec!["P".into(), "Q".into(), "R".into()]);
    assert!(
        matches!(dive_trace::validate_doc(&model,&false_claim,&[]).0.validation,Validation::Invalid(f) if f.check==dive_trace::Check::Temporal)
    );
}

#[test]
fn pinned_fair_successes_use_full_enabledness_beyond_the_retained_graph() {
    let root = probes();
    for name in ["LvWeakFair", "LvSFIntermittent", "LvConstraint"] {
        let source =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let cfg =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("cfg");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        assert_eq!(
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
                .expect("exit")
                .trim(),
            "0"
        );
        let opts = CheckOptions::tlc(0, 0);
        exact_store_is_refused(&source, &cfg, &opts);
        let run = core_final::check(&source, &cfg, &opts).unwrap_or_else(|s| panic!("{name}: {s}"));
        let Outcome::Holds(c) = run.outcome() else {
            panic!("{name}: TLC finds no counterexample: {:?}", run.outcome())
        };
        assert_eq!(
            (c.stats.generated, c.stats.distinct, c.stats.depth),
            counters(&stdout),
            "{name}"
        );
    }
}
