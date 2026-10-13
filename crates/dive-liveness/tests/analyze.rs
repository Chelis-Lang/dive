//! Pure final analysis versus pinned TLC counts and independent word validation.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{CheckOptions, Verdict};
use dive_liveness::analyze::{Analysis, AnalysisError, Limits, analyze_final};
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

#[test]
fn pinned_pure_and_fair_models_have_valid_candidates_and_tlc_counts() {
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
            assert!(c.properties.contains(&c.target_property));
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
fn fairness_limits_and_safety_stops_do_not_guess_holds() {
    let opts = CheckOptions::tlc(0, 0);
    let source = "---- MODULE Analyze ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n\
        Spec == Init /\\ [][Next]_x /\\ WF_x(Next)\nProp == []<>x\nInv == ~x\n====\n";
    let model = bound(source, "SPECIFICATION Spec\nPROPERTY Prop\n", &opts);
    assert!(matches!(
        analyze_final(&model, &opts, Limits::default()).expect("fair toggle"),
        Analysis::Holds(_)
    ));
    let opaque = source
        .replace("WF_x(Next)", "F(Next)")
        .replace("Spec ==", "F(a) == WF_x(a)\nSpec ==");
    let model = bound(&opaque, "SPECIFICATION Spec\nPROPERTY Prop\n", &opts);
    let actual =
        analyze_final(&model, &opts, Limits::default()).expect("resolved fair action binding");
    let Analysis::Holds(c) = actual else {
        panic!("fair alias changed truth");
    };
    assert_eq!(
        (c.stats.generated, c.stats.distinct, c.stats.depth),
        (3, 2, 2)
    );
    let opaque = source
        .replace("VARIABLE x", "EXTENDS TLC\nVARIABLE x")
        .replace("WF_x(Next)", "[]<>(TLCGet(\"level\")=0)");
    let model = bound(&opaque, "SPECIFICATION Spec\nPROPERTY Prop\n", &opts);
    assert!(matches!(
        analyze_final(&model, &opts, Limits::default()),
        Err(AnalysisError::Planning(_))
    ));
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
        matches!(analyze_final(&model,&opts,Limits::default()),Err(AnalysisError::Exploration(v)) if matches!(v.verdict,Verdict::InvariantViolated { .. }))
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

#[test]
fn every_false_temporal_property_is_named_on_the_selected_lasso() {
    let opts = CheckOptions::tlc(0, 0);
    let model = bound(
        "---- MODULE Claims ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\nQ == <>[]~x\nR == []<>x\n====\n",
        "SPECIFICATION Spec\nPROPERTY P Q R\n",
        &opts,
    );
    let Analysis::Candidate(c) = analyze_final(&model, &opts, Limits::default()).expect("analysis")
    else {
        panic!("violations")
    };
    assert_eq!(c.properties, vec!["P", "Q"]);
    let values = c
        .states
        .iter()
        .map(|s| json!({"x":s[0].as_ref().expect("x")==&Value::Bool(true)}))
        .collect();
    let doc = TraceDoc::behavior(values)
        .expect("doc")
        .with_ending(Ending::Lasso { back_to: c.back_to })
        .with_properties(c.properties);
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-probes");
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
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            let opts = CheckOptions {
                store,
                ..CheckOptions::tlc(0, 0)
            };
            let model = bound(&source, &cfg, &opts);
            let Analysis::Holds(c) = analyze_final(&model, &opts, Limits::default()).expect(name)
            else {
                panic!("{name}: TLC finds no counterexample")
            };
            assert_eq!(
                (c.stats.generated, c.stats.distinct, c.stats.depth),
                counters(&stdout),
                "{name}"
            );
        }
    }
}
