//! Fixed-arity Append requires operand proof and independent witness replay.
#![allow(clippy::expect_used, clippy::panic, reason = "adversarial controls")]
use dive_model::BindOptions;
use std::sync::Arc;
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete paired current selection and independent mutation controls"
)]
fn append_scope_controls_replay_fresh_memory_and_json() {
    use dive_api::tlc_temporal::Outcome;
    use dive_trace::{Ending, Validation};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/native-sequence-operators");
    let rows: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("selection.json")).expect("selection"))
            .expect("JSON");
    let mut cases = 0;
    for row in rows.as_array().expect("rows") {
        if row["kind"] != "pure-sequence-native"
            || !row["name"].as_str().expect("name").contains("Append")
        {
            continue;
        }
        let name = row["name"].as_str().expect("name");
        let dir = base.join("models").join(name);
        let a = dive_sema::analyze_file(
            &dir.join(format!("{name}.tla")),
            &dive_sema::Options::default(),
        )
        .expect("analysis");
        assert!(a.accepted(), "{name}");
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let cfg = dive_config::parse(
            &std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("cfg"),
        )
        .into_config()
        .expect("parse");
        for profile in [
            dive_check::CheckOptions::tlc(0, 0),
            dive_check::CheckOptions::dive(0),
        ] {
            for width in [1, 3, 4, 8] {
                cases += 1;
                let options = dive_check::CheckOptions {
                    workers: width,
                    ..profile.clone()
                };
                let result = dive_api::tlc_temporal::check_final(
                    &ir,
                    &cfg,
                    &options,
                    dive_liveness::phase_graph::Limits::default(),
                );
                if options.profile == dive_eval::Profile::Dive {
                    let refusal =
                        result.expect_err("explicit final TLC selection retains profile admission");
                    assert!(refusal.evidence.is_none());
                    continue;
                }
                let report = result.expect(name);
                if name.contains("Fails") {
                    let Outcome::Violation(v) = report.outcome() else {
                        panic!("{name} expected validated violation")
                    };
                    assert_eq!(v.validation().validation, Validation::Valid);
                    assert_eq!(v.document().properties, vec!["Live"]);
                    let fresh = dive_model::bind(
                        ir.clone(),
                        &cfg,
                        &if options.profile == dive_eval::Profile::Tlc {
                            BindOptions::tlc()
                        } else {
                            BindOptions::dive()
                        },
                    )
                    .expect("fresh authority");
                    let doc = v.document();
                    assert_eq!(
                        dive_trace::validate_doc(&fresh, doc, &[], None)
                            .0
                            .validation,
                        Validation::Valid
                    );
                    let bytes = serde_json::to_vec(&doc.to_json()).expect("serialized");
                    let decoded = dive_trace::TraceDoc::from_json(
                        &serde_json::from_slice(&bytes).expect("JSON"),
                    )
                    .expect("decoded");
                    assert_eq!(
                        dive_trace::validate_doc(&fresh, &decoded, &[], None)
                            .0
                            .validation,
                        Validation::Valid
                    );
                    let mut bad = doc.clone();
                    bad.properties = vec!["Foreign".into()];
                    assert_ne!(
                        dive_trace::validate_doc(&fresh, &bad, &[], None)
                            .0
                            .validation,
                        Validation::Valid
                    );
                    let mut bad = doc.clone();
                    bad.states[0] =
                        serde_json::from_value(serde_json::json!({"x":99})).expect("mutant");
                    assert_ne!(
                        dive_trace::validate_doc(&fresh, &bad, &[], None)
                            .0
                            .validation,
                        Validation::Valid
                    );
                    let mut bad = doc.clone();
                    bad.ending = Some(Ending::Lasso {
                        back_to: usize::MAX,
                    });
                    assert_ne!(
                        dive_trace::validate_doc(&fresh, &bad, &[], None)
                            .0
                            .validation,
                        Validation::Valid
                    );
                } else {
                    assert!(
                        matches!(report.outcome(), Outcome::Holds(_) | Outcome::Safety(_)),
                        "{name}"
                    );
                }
            }
        }
    }
    assert_eq!(cases, 64);
}

#[test]
fn append_two_state_operands_have_independent_periodic_truth() {
    use dive_model::Pred;
    use dive_value::Value;
    let source = "---- MODULE AppendWords ----\nEXTENDS Sequences\nVARIABLES x,y\nInit==x = <<TRUE>> /\\ y=TRUE\nNext==UNCHANGED <<x,y>>\nUse(F(_,_))==[]<>(F(x,y) = <<TRUE,TRUE>>)\nRelay(G(_,_))==Use(G)\nLive==Relay(Append)\n====\n";
    let a = dive_sema::analyze_source("AppendWords.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{a:?}");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    let mut cases = 0;
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
        let d = ir.root_def("Live").expect("property");
        let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
        let plan =
            dive_liveness::plan::lower(&model, &pred, dive_liveness::formula::Limits::default())
                .expect("plan");
        for n in 1..=4 {
            for mask in 0_usize..(1 << (2 * n)) {
                let truth: Vec<bool> = (0..n)
                    .map(|i| mask & (3 << (2 * i)) == 3 << (2 * i))
                    .collect();
                let states: Vec<dive_trace::State> = (0..n)
                    .map(|i| {
                        Arc::from([
                            Some(Value::tuple(vec![(mask & (1 << (2 * i)) != 0).into()])),
                            Some((mask & (1 << (2 * i + 1)) != 0).into()),
                        ])
                    })
                    .collect();
                for back in 0..n {
                    cases += 1;
                    let actual = dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                        .expect("independent word");
                    assert_eq!(actual, vec![truth[back..].iter().any(|b| *b); n]);
                    let bins: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let edge = dive_eval::St::action(
                                states[i].clone(),
                                states[if i + 1 == n { back } else { i + 1 }].clone(),
                            );
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &edge).expect("atom"))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&bins, &bins, back, 1_000_000)
                            .expect("kernel"),
                        actual
                    );
                }
            }
        }
    }
    assert_eq!(cases, 2504);
}

#[test]
fn string_capable_tail_and_subseq_remain_prebind_refused() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/native-sequence-operators/models");
    for name in [
        "SequenceTailConstantHolds",
        "SequenceTailStateHolds",
        "SequenceSubSeqConstantHolds",
        "SequenceSubSeqStateHolds",
    ] {
        let dir = base.join(name);
        let analysis = dive_sema::analyze_file(
            &dir.join(format!("{name}.tla")),
            &dive_sema::Options::default(),
        )
        .expect("analysis");
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse(
            &std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("cfg"),
        )
        .into_config()
        .expect("config");
        let failure = dive_api::tlc_temporal::check_final(
            &ir,
            &cfg,
            &dive_check::CheckOptions::tlc(0, 0),
            dive_liveness::phase_graph::Limits::default(),
        )
        .expect_err("runtime-name authority stays refused");
        assert!(failure.message.contains("runtime-name"), "{failure:?}");
        assert!(failure.evidence.is_none());
    }
}
