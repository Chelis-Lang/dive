//! Independent endpoint words with direct diagnostic calls in Next.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn model(
    source: &str,
    config: &str,
    compat_tlc: bool,
) -> Result<SpecModel, Box<dyn std::error::Error>> {
    Ok(SpecModel::from_source(
        "Diag.tla",
        source,
        config,
        &TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )?)
}

fn source(vars: &str, init: &str, next: &str) -> String {
    format!(
        "---- MODULE Diag ----\nEXTENDS Integers, TLC\nVARIABLES {vars}\nInit == {init}\nNext == {next}\n====\n"
    )
}

fn wire(values: &Value, ending: &Value, version: usize, length: usize) -> Value {
    let vars: Vec<_> = values
        .as_object()
        .into_iter()
        .flat_map(|m| m.keys())
        .collect();
    json!({"format":"dive-trace","variables":vars,"states":[{"action":null,"values":values}],
           "version":version,"length":length,"ending":ending})
}

fn origin(source: &str, expr: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let start = source.find(expr).ok_or("source expression")?;
    Ok(json!({"module":"Diag","start":start,"end":start+expr.len(),"instance":null}))
}

const CFG: &str = "INIT Init\nNEXT Next\n";

#[test]
fn ordered_error_replays_before_output_and_checks_its_source() -> TestResult {
    for action in [
        r#"IF x=0 THEN x'=1 \div 0 ELSE PrintT("late") /\ x'=4"#,
        r"IF x=0 THEN Print(1 \div 0,TRUE) /\ x'=x ELSE x'=x",
    ] {
        let source = source("x", r"x \in 0..1", action);
        let location = origin(&source, r"1 \div 0")?;
        let word = wire(
            &json!({"x":0}),
            &json!({"kind":"error","ec":"TLC_MODULE_DIVISION_BY_ZERO",
            "endpoint":{"stage":"next-ordered-terminal","source":location,"deciding":2179,"reported":2179}}),
            1,
            1,
        );
        for compat_tlc in [false, true] {
            let model = model(&source, CFG, compat_tlc)?;
            let validate = |w: &Value| -> Result<Validation, Box<dyn std::error::Error>> {
                Ok(model.try_validate(&TraceDoc::from_json(w)?)?.validation)
            };
            assert_eq!(validate(&word)?, Validation::Valid, "{action}");
            let mut damaged = word.clone();
            damaged["ending"]["endpoint"]["source"]["start"] = json!(0);
            assert!(matches!(validate(&damaged)?, Validation::Invalid(_)));
            if action.contains("late") {
                let mut reached = word.clone();
                reached["states"][0]["values"]["x"] = json!(1);
                assert!(
                    matches!(validate(&reached)?,Validation::Unsupported(s) if s.contains("Print"))
                );
            }
        }
    }
    Ok(())
}

#[test]
fn ordered_error_refuses_configured_effects_in_a_sibling_action() -> TestResult {
    let next = r"(IF x=0 THEN x'=1 \div 0 ELSE PrintT(x) /\ x'=x) \/ (Op(x) /\ x'=x)";
    let source = source("x", "x=0", next).replace(
        "Init ==",
        "CONSTANT Op(_)\nHidden(n) == TLCSet(0,n)\nInit ==",
    );
    let cfg = format!("{CFG}CONSTANT Op <- Hidden\n");
    let word = wire(
        &json!({"x":0}),
        &json!({"kind":"error","ec":"TLC_MODULE_DIVISION_BY_ZERO",
                    "endpoint":{"stage":"next-ordered-terminal","source":origin(&source,r"1 \div 0")?,"deciding":2179,"reported":2179}}),
        1,
        1,
    );
    for compat_tlc in [false, true] {
        let report =
            model(&source, &cfg, compat_tlc)?.try_validate(&TraceDoc::from_json(&word)?)?;
        assert!(
            matches!(report.validation,Validation::Unsupported(ref why) if why.contains("dynamically shared scopes")),
            "{report:?}"
        );
    }
    Ok(())
}

#[test]
fn partial_replay_checks_the_actual_bindings_before_a_dormant_print() -> TestResult {
    let action = r"IF x=0 THEN x'=6 ELSE PrintT(x) /\ UNCHANGED <<x,y>>";
    let source = source("x,y", r"x \in 0..1 /\ y=0", action);
    let word = wire(
        &json!({"x":0,"y":0}),
        &json!({"kind":"partial-state","ec":"TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT","endpoint":{"stage":"next-partial-state","source":origin(&source,action)?,"assignments":{"x":6},"unassigned":["y"]}}),
        3,
        2,
    );
    for compat_tlc in [false, true] {
        let model = model(&source, CFG, compat_tlc)?;
        assert_eq!(
            model.try_validate(&TraceDoc::from_json(&word)?)?.validation,
            Validation::Valid
        );
        let mut damaged = word.clone();
        damaged["ending"]["endpoint"]["assignments"]["x"] = json!(5);
        assert!(matches!(
            model
                .try_validate(&TraceDoc::from_json(&damaged)?)?
                .validation,
            Validation::Invalid(_)
        ));
    }
    Ok(())
}

#[test]
fn deadlock_rejects_a_successor_and_refuses_reached_or_dynamic_effects() -> TestResult {
    let next = r"IF x=0 THEN FALSE ELSE IF x=1 THEN x'=x ELSE PrintT(x) /\ x'=x";
    let source = source("x", r"x \in 0..2", next);
    for compat_tlc in [false, true] {
        let validate =
            |source: &str, cfg: &str, x: usize| -> Result<Validation, Box<dyn std::error::Error>> {
                let model = model(source, cfg, compat_tlc)?;
                let doc =
                    TraceDoc::from_json(&wire(&json!({"x":x}), &json!({"kind":"deadlock"}), 1, 1))?;
                Ok(model.try_validate(&doc)?.validation)
            };
        assert_eq!(validate(&source, CFG, 0)?, Validation::Valid);
        assert!(matches!(validate(&source, CFG, 1)?, Validation::Invalid(_)));
        assert!(matches!(
            validate(&source, CFG, 2)?,
            Validation::Unsupported(_)
        ));
        let dynamic = source
            .replace(
                "Init ==",
                "CONSTANT Op(_)\nHidden(n) == TLCSet(0,n)\nInit ==",
            )
            .replace("x'=x ELSE PrintT", "Op(x) /\\ x'=x ELSE PrintT");
        let cfg = format!("{CFG}CONSTANT Op <- Hidden\n");
        for x in [0, 1] {
            assert!(matches!(
                validate(&dynamic, &cfg, x)?,
                Validation::Unsupported(_)
            ));
        }
    }
    Ok(())
}
