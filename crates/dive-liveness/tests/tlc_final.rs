//! Self-constructed final checks, clean oracle words and independent validation.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_check::CheckOptions;
use dive_liveness::{phase_graph::Limits, tlc_final};
use dive_trace::{Ending, Trace, Validation};
use std::sync::Arc;
fn bind(source: &str, config: &str, options: &CheckOptions) -> dive_model::Model {
    let name = source.split_whitespace().nth(2).expect("module");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("cfg");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )
    .expect("bound")
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "retain complete selected words, claim attribution and each independent mutation"
)]
fn actual_final_words_match_and_independently_replay_with_all_claims() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes/tlc-final-check");
    let capture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("capture.json")).expect("capture"))
            .expect("JSON");
    let mut complete = 0;
    for row in capture["rows"].as_array().expect("selected") {
        if row["exit"] != 0 {
            continue;
        }
        complete += 1;
        let name = row["name"].as_str().expect("name");
        let src =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.tla")))
                .expect("source");
        let cfg =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.cfg")))
                .expect("config");
        let stderr = std::fs::read_to_string(base.join("raw").join(name).join("stderr"))
            .expect("raw original finalCheck");
        for workers in [1, 3, 4, 8] {
            let options = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let model = bind(&src, &cfg, &options);
            let result = tlc_final::check(&model, &options, Limits::default())
                .expect("supported clean final check");
            if row["fields"]["final_check_code"] == "0" {
                assert!(matches!(result, tlc_final::Analysis::Holds(_)), "{name}");
                continue;
            }
            let tlc_final::Analysis::Candidate(candidate) = result else {
                panic!("{name} reference violation")
            };
            assert_eq!(
                candidate
                    .evidence
                    .execution
                    .capture
                    .stats
                    .generated
                    .to_string(),
                row["fields"]["generated"].as_str().expect("generated"),
                "{name}"
            );
            assert_eq!(
                candidate
                    .evidence
                    .execution
                    .capture
                    .stats
                    .distinct
                    .to_string(),
                row["fields"]["distinct"].as_str().expect("distinct"),
                "{name}"
            );
            let validation = bind(&src, &cfg, &options);
            let trace = Trace {
                states: candidate.states.clone(),
                labels: vec![None; candidate.states.len()],
                ending: Ending::Lasso {
                    back_to: candidate.back_to,
                },
                length: Some(candidate.states.len()),
                properties: candidate.properties.clone(),
            };
            assert_eq!(
                dive_trace::validate(&validation, &trace, None).validation,
                Validation::Valid,
                "{name}"
            );
            let document = trace.to_doc(&validation).expect("serialized word");
            assert_eq!(
                dive_trace::validate_doc(&validation, &document, &[], None)
                    .0
                    .validation,
                Validation::Valid,
                "{name}"
            );
            let claims = candidate.properties.join(" and ");
            assert!(
                stderr.contains(&format!("Temporal property {claims} was violated."))
                    || stderr.contains(&format!("Temporal properties {claims} were violated.")),
                "{name} claims={claims}"
            );
            if name != "GraphConcrete" {
                let printed: Vec<i32> = stderr
                    .split("@!@!@STARTMSG 2217:4 @!@!@")
                    .skip(1)
                    .map(|chunk| {
                        chunk
                            .lines()
                            .find_map(|line| line.strip_prefix("x = "))
                            .expect("printed x")
                            .parse()
                            .expect("integer")
                    })
                    .collect();
                let actual: Vec<_> = candidate
                    .states
                    .iter()
                    .map(|s| {
                        s[0].as_ref()
                            .expect("x")
                            .as_int()
                            .expect("int")
                            .to_i32()
                            .expect("small")
                    })
                    .collect();
                assert_eq!(actual, printed, "{name} pinned trace");
            }
            let mut wrong = document.clone();
            wrong.states[0] = serde_json::from_value(serde_json::json!({"x":99})).expect("state");
            assert_ne!(
                dive_trace::validate_doc(&validation, &wrong, &[], None)
                    .0
                    .validation,
                Validation::Valid,
                "{name} corrupted Init"
            );
            let mut wrong = document.clone();
            wrong.properties.clear();
            assert_ne!(
                dive_trace::validate_doc(&validation, &wrong, &[], None)
                    .0
                    .validation,
                Validation::Valid,
                "{name} omitted attribution"
            );
            let mut wrong = document.clone();
            wrong
                .states
                .push(serde_json::from_value(serde_json::json!({"x":99})).expect("state"));
            assert_ne!(
                dive_trace::validate_doc(&validation, &wrong, &[], None)
                    .0
                    .validation,
                Validation::Valid,
                "{name} corrupt Next/closing"
            );
            let mut wrong = document;
            wrong.ending = Some(Ending::Lasso {
                back_to: usize::MAX,
            });
            assert_ne!(
                dive_trace::validate_doc(&validation, &wrong, &[], None)
                    .0
                    .validation,
                Validation::Valid,
                "{name} malformed closure"
            );
        }
    }
    assert_eq!(complete, 19);
}
#[test]
fn exact_profile_and_resource_refusals_never_become_proofs() {
    let src = "---- MODULE Bound ----\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0,1}\nLive == []<>(x=2)\n====\n";
    let cfg = "INIT Init\nNEXT Next\nPROPERTY Live\n";
    let mut options = CheckOptions::tlc(0, 0);
    let model = bind(src, cfg, &options);
    options.store = dive_store::StoreMode::Exact;
    let fail =
        tlc_final::check(&model, &options, Limits::default()).expect_err("exact identity refused");
    assert_eq!(fail.workers.expect("admission").used, 0);
    assert!(fail.evidence.is_none());
    assert_eq!(fail.error.code(), "DIVE-L0063");
    options = CheckOptions::dive(0);
    assert!(tlc_final::check(&model, &options, Limits::default()).is_err());
    options = CheckOptions::tlc(0, 0);
    options.workers = 8;
    let fail = tlc_final::check(
        &model,
        &options,
        Limits {
            graph: dive_liveness::product::Limits {
                steps: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect_err("bounded graph");
    assert!(!matches!(fail.error, tlc_final::Error::Attribution(_)));
    options.workers = 0;
    assert_eq!(
        tlc_final::check(&model, &options, Limits::default())
            .expect_err("parameter")
            .workers
            .expect("evidence")
            .used,
        0
    );
}
#[test]
fn foreign_and_effectful_models_stop_without_pool_or_predicate_output() {
    let opts = CheckOptions {
        workers: 8,
        ..CheckOptions::tlc(0, 0)
    };
    let cfg = "INIT Init\nNEXT Next\nPROPERTY Live\n";
    let effect = "---- MODULE Effect ----\nEXTENDS TLC\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nLive == []<>(PrintT(x))\n====\n";
    let model = bind(effect, cfg, &opts);
    let failure = tlc_final::check(&model, &opts, Limits::default()).expect_err("state effect");
    assert_eq!(failure.workers.expect("admission").used, 0);
    assert!(failure.evidence.is_none());
    assert_eq!(model.take_output(), Vec::<String>::new());
    let src = "---- MODULE Owned ----\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
    for names in [false, true] {
        let mut model = bind(src, cfg, &opts);
        let other = bind(src, cfg, &opts);
        if names {
            model.names = other.names;
        } else {
            model.ir = other.ir;
        }
        let failure =
            tlc_final::check(&model, &opts, Limits::default()).expect_err("foreign context");
        assert_eq!(failure.workers.expect("admission").used, 0);
        assert!(failure.evidence.is_none());
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}
#[test]
fn malformed_bound_role_ids_stop_before_planning() {
    let opts = CheckOptions::tlc(0, 0);
    let src = "---- MODULE BadRole ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
    for location in [false, true] {
        let mut model = bind(src, "INIT Init\nNEXT Next\nPROPERTY Live\n", &opts);
        let pred = &mut model.implied_temporals[0].pred;
        if location {
            pred.location = dive_ir::ExprId(u32::MAX);
        } else {
            pred.expr = dive_ir::ExprId(u32::MAX);
        }
        let failure =
            tlc_final::check(&model, &opts, Limits::default()).expect_err("bad role before walker");
        assert_eq!(failure.workers.expect("admission").used, 0);
        assert!(failure.evidence.is_none());
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}
