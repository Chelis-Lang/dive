//! Actual primary `RecFcn` fairness outcomes/counts and independently checked words.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "tiny generated oracle controls"
)]
use dive_liveness::analyze::{Analysis, Limits, analyze_final};
use dive_runtime::CheckOptions;
use dive_trace::{Ending, TraceDoc, Validation};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recursive-function")
}
fn bound(module: &str, cfg: &str, opts: &CheckOptions) -> dive_model::Model {
    let analysis = dive_sema::analyze_file(
        &fixture().join(format!("{module}.tla")),
        &dive_sema::Options::default(),
    )
    .expect("input");
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let config = dive_config::parse(cfg).into_config().expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &dive_model::BindOptions {
            eval: opts.eval,
            ..dive_model::BindOptions::tlc()
        },
    )
    .expect("bound model")
}
fn oracle() -> Value {
    serde_json::from_slice(
        &std::fs::read(fixture().join("oracle.json")).expect("generated primary"),
    )
    .expect("JSON")
}
fn word(
    model: &dive_model::Model,
    states: &[dive_trace::State],
    back: usize,
    properties: Vec<String>,
) -> TraceDoc {
    let names = dive_trace::variables(model);
    let states = states
        .iter()
        .map(|state| {
            Value::Object(
                names
                    .iter()
                    .zip(state.iter())
                    .map(|(name, value)| {
                        (
                            name.clone(),
                            dive_trace::value_json::to_json(
                                value.as_ref().expect("complete"),
                                model.evaluator.options().ctx,
                            )
                            .expect("finite JSON"),
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    TraceDoc::behavior(states)
        .expect("word")
        .with_ending(Ending::Lasso { back_to: back })
        .with_properties(properties)
}
#[test]
fn seven_actual_primary_outcomes_counts_and_native_words_match() {
    let oracle = oracle();
    assert_eq!(oracle["primary_before_production_implementation"], true);
    assert_eq!(oracle["observations"].as_array().expect("rows").len(), 7);
    for row in oracle["observations"].as_array().expect("rows") {
        let module = row["module"].as_str().expect("module");
        let config =
            std::fs::read_to_string(fixture().join(row["config"].as_str().expect("config")))
                .expect("configuration");
        for tlc in [true, false] {
            let opts = if tlc {
                CheckOptions::tlc(0, 0)
            } else {
                CheckOptions::dive(0)
            };
            let model = bound(module, &config, &opts);
            let result = analyze_final(&model, &opts, Limits::default())
                .expect("supported recursive fairness");
            let stats = match &result {
                Analysis::Holds(c) => c.stats,
                Analysis::Candidate(c) => c.exploration.stats,
            };
            assert_eq!(json!(stats.generated), row["generated"], "{row}");
            assert_eq!(json!(stats.distinct), row["distinct"], "{row}");
            assert_eq!(json!(stats.depth), row["depth"], "{row}");
            match result {
                Analysis::Holds(_) => assert_eq!(row["physical_exit"], 0),
                Analysis::Candidate(candidate) => {
                    assert_eq!(row["physical_exit"], 13);
                    assert_eq!(row["error_codes"][0], 2116);
                    assert_eq!(json!(candidate.target_property), row["violated_property"]);
                    let fresh = bound(module, &config, &opts);
                    let document = word(
                        &fresh,
                        &candidate.states,
                        candidate.back_to,
                        candidate.properties,
                    );
                    assert_eq!(
                        dive_trace::validate_doc(&fresh, &document, &[])
                            .0
                            .validation,
                        Validation::Valid,
                        "native {row}"
                    );
                }
            }
        }
    }
}
#[test]
fn genuine_primary_lassos_and_init_edge_loop_property_mutations_are_independent() {
    let opts = CheckOptions::tlc(0, 0);
    for row in oracle()["observations"].as_array().expect("rows") {
        if row["counterexample"].is_null() {
            continue;
        }
        let module = row["module"].as_str().expect("module");
        let config =
            std::fs::read_to_string(fixture().join(row["config"].as_str().expect("config")))
                .expect("configuration");
        let model = bound(module, &config, &opts);
        let raw = &row["counterexample"]["counterexample"];
        let states = raw["state"]
            .as_array()
            .expect("actual states")
            .iter()
            .map(|s| json!({"pending":{"set":s[1]["pending"]},"done":s[1]["done"]}))
            .collect();
        let closing = raw["action"]
            .as_array()
            .expect("actual edges")
            .last()
            .expect("closing edge")[2][0]
            .as_u64()
            .expect("target");
        let back = usize::try_from(closing - 1).expect("tiny index");
        let doc = TraceDoc::behavior(states)
            .expect("actual word")
            .with_ending(Ending::Lasso { back_to: back })
            .with_properties(vec!["Bad".into()]);
        assert_eq!(
            dive_trace::validate_doc(&model, &doc, &[]).0.validation,
            Validation::Valid,
            "primary {row}"
        );
        let mut bad = doc.clone();
        bad.states[0]["pending"] = json!({"set":[99]});
        assert_ne!(
            dive_trace::validate_doc(&model, &bad, &[]).0.validation,
            Validation::Valid
        );
        let mut bad = doc.clone();
        bad.states[1]["done"] = json!(false);
        assert_ne!(
            dive_trace::validate_doc(&model, &bad, &[]).0.validation,
            Validation::Valid
        );
        let bad = doc.clone().with_ending(Ending::Lasso { back_to: 0 });
        assert_ne!(
            dive_trace::validate_doc(&model, &bad, &[]).0.validation,
            Validation::Valid
        );
        let good = bound(
            module,
            &config.replace("PROPERTY Bad", "PROPERTY Good"),
            &opts,
        );
        let bad = doc.with_properties(vec!["Good".into()]);
        assert_ne!(
            dive_trace::validate_doc(&good, &bad, &[]).0.validation,
            Validation::Valid
        );
    }
}

#[test]
fn recursive_primed_effect_native_and_forwarded_configured_boundaries_stay_refused() {
    for (expression, extra, config) in [
        ("F(x')", "", ""),
        ("F(PrintT(0))", "", ""),
        ("F(RandomElement({TRUE}))", "", ""),
        ("F(TLCGet(\"level\")=0)", "", ""),
        ("F(ENABLED (x'=TRUE))", "", ""),
        ("Pass(F,x)", "Pass(H(_),v)==H(v)", ""),
        ("C(x)", "CONSTANT C(_)", "CONSTANT C <- F\n"),
    ] {
        let source = format!(
            "---- MODULE RecursiveBoundary ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit==x=FALSE\nNext==x'=~x\nF(S)==LET f[t \\in {{0}}]==IF t=0 THEN S ELSE f[t] IN f[0]\n{extra}\nP==<>[][{expression}]_x\n====\n"
        );
        let analysis = dive_sema::analyze_source(
            "RecursiveBoundary.tla",
            &source,
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let cfg = dive_config::parse(&format!("INIT Init\nNEXT Next\nPROPERTY P\n{config}"))
            .into_config()
            .expect("config");
        let model = dive_model::bind(
            Arc::new(dive_ir::lower(&analysis).expect("IR")),
            &cfg,
            &dive_model::BindOptions::tlc(),
        )
        .expect("model");
        let property = &model
            .implied_temporals
            .first()
            .expect("genuine temporal obligation")
            .pred;
        assert!(
            dive_liveness::plan::lower(&model, property, dive_liveness::formula::Limits::default())
                .is_err(),
            "{expression}"
        );
        let states: Vec<dive_trace::State> = vec![Arc::from([Some(false.into())])];
        assert!(
            dive_trace::temporal::evaluate_lasso(&model, property, &states, 0).is_err(),
            "independent {expression}"
        );
        assert_eq!(model.output, [] as [dive_model::Message; 0]);
    }
}
