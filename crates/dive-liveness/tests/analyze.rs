//! Pure final analysis versus pinned TLC counts and independent word validation.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{CheckOptions, Verdict};
use dive_liveness::analyze::{Analysis, AnalysisError, Limits, analyze_final};
use dive_liveness::plan;
use dive_trace::{Ending, TraceDoc, Validation};
use dive_value::Value;
use serde_json::json;
use std::sync::Arc;

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

fn expect_fairness_refusal(
    analysis: Result<Analysis, AnalysisError>,
    model: &dive_model::Model,
    name: &str,
) {
    let Err(AnalysisError::Planning(plan::PlanningError::Unsupported(r))) = analysis else {
        panic!("{name}: the behavior's WF must refuse before exploration");
    };
    assert!(r.detail.contains("fairness"));
    assert!(matches!(
        model.ir.expr(r.cause.expr),
        dive_ir::Expr::Wf { .. }
    ));
    assert!(
        r.cause.origin.file.is_some(),
        "the refusal must retain its source"
    );
}

#[test]
fn pinned_models_either_have_valid_candidates_and_tlc_counts_or_refuse_fairness() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-probes");
    for name in [
        "LvNoFair",
        "LvAlwaysEventually",
        "LvEventuallyAlways",
        "LvLeadsTo",
        "LvActionProp",
        "LvLassoMid",
        "LvEAPrefix",
        "LvStutterActionConstraint",
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
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            let opts = CheckOptions {
                store,
                workers: 8,
                ..CheckOptions::tlc(0, 0)
            };
            let model = bound(&source, &config, &opts);
            let analysis = analyze_final(&model, &opts, Limits::default());
            if matches!(
                name,
                "LvAlwaysEventually" | "LvEventuallyAlways" | "LvLeadsTo" | "LvLassoMid"
            ) {
                expect_fairness_refusal(analysis, &model, name);
                continue;
            }
            let Analysis::Candidate(c) = analysis.expect(name) else {
                panic!("{name}: must find a counterexample");
            };
            assert_eq!(
                (
                    c.exploration.stats.generated,
                    c.exploration.stats.distinct,
                    c.exploration.stats.depth
                ),
                counters(&stdout),
                "{name}"
            );
            let mut states: Vec<serde_json::Value> = c
                .states
                .iter()
                .map(|vals| {
                    let x = match vals[0].as_ref().expect("x") {
                        Value::Int(n) => json!(*n),
                        Value::Bool(b) => json!(*b),
                        v => panic!("unexpected {}", v.kind_name()),
                    };
                    json!({"x":x})
                })
                .collect();
            let doc = TraceDoc::behavior(states.clone())
                .expect("word")
                .with_ending(Ending::Lasso { back_to: c.back_to })
                .with_properties(vec![c.target_property]);
            assert_eq!(
                dive_trace::validate_doc(&model, &doc, &[], None)
                    .0
                    .validation,
                Validation::Valid,
                "{name}"
            );
            states[0]["x"] = json!(99999);
            let broken = TraceDoc::behavior(states)
                .expect("mutant")
                .with_ending(Ending::Lasso { back_to: c.back_to })
                .with_properties(doc.properties.clone());
            assert_ne!(
                dive_trace::validate_doc(&model, &broken, &[], None)
                    .0
                    .validation,
                Validation::Valid,
                "{name}: mutated Init"
            );
        }
    }
}

#[test]
fn pure_behavior_conjuncts_cannot_be_dropped_and_holds_is_exhaustive() {
    let opts = CheckOptions::tlc(0, 0);
    let source = "---- MODULE Analyze ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n\
        Spec == Init /\\ [][Next]_x /\\ []<>x\nProp == []<>x\n====\n";
    let model = bound(source, "SPECIFICATION Spec\nPROPERTY Prop\n", &opts);
    assert!(matches!(
        analyze_final(&model, &opts, Limits::default()).expect("analysis"),
        Analysis::Holds(_)
    ));
    let no_behavior = source.replace(" /\\ []<>x", "");
    let model = bound(&no_behavior, "SPECIFICATION Spec\nPROPERTY Prop\n", &opts);
    assert!(matches!(
        analyze_final(&model, &opts, Limits::default()).expect("analysis"),
        Analysis::Candidate(_)
    ));
}

#[test]
fn unknown_fairness_limits_and_safety_stops_never_become_holds() {
    let opts = CheckOptions::tlc(0, 0);
    let source = "---- MODULE Analyze ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n\
        Spec == Init /\\ [][Next]_x /\\ WF_x(Next)\nProp == []<>x\nInv == ~x\n====\n";
    let model = bound(source, "SPECIFICATION Spec\nPROPERTY Prop\n", &opts);
    assert!(
        matches!(analyze_final(&model,&opts,Limits::default()),Err(AnalysisError::Planning(e)) if e.to_string().contains("fairness"))
    );
    let source = source.replace(" /\\ WF_x(Next)", "");
    let model = bound(&source, "SPECIFICATION Spec\nPROPERTY Prop\n", &opts);
    for limits in [
        Limits {
            properties: 0,
            ..Limits::default()
        },
        Limits {
            atom_cells: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            analyze_final(&model, &opts, limits),
            Err(AnalysisError::Limit(_))
        ));
    }
    let model = bound(
        &source,
        "SPECIFICATION Spec\nPROPERTY Prop\nINVARIANT Inv\n",
        &opts,
    );
    assert!(
        matches!(analyze_final(&model,&opts,Limits::default()),Err(AnalysisError::Exploration(v)) if matches!(*v,Verdict::InvariantViolated { .. }))
    );
}

#[test]
fn atom_errors_retain_a_diagnostic_and_source_span() {
    let opts = CheckOptions::tlc(0, 0);
    let model = bound(
        "---- MODULE Analyze ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
        Next == x' = x\nProp == []<>(1 \\div x = 0)\n====\n",
        "INIT Init\nNEXT Next\nPROPERTY Prop\n",
        &opts,
    );
    let error =
        analyze_final(&model, &opts, Limits::default()).expect_err("division by zero cannot hold");
    assert_eq!(error.code(), "DIVE-L0033");
    let AnalysisError::Atom {
        property,
        source,
        detail,
    } = error
    else {
        panic!("atom error");
    };
    assert_eq!(property, "Prop");
    assert!(source.origin.file.is_some());
    assert!(source.origin.range.0 < source.origin.range.1);
    assert!(
        detail.contains("The second argument of \\div is 0."),
        "{detail}"
    );
}
