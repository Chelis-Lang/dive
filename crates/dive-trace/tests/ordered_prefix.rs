//! Bounded admission proofs keep ordinary named words separate from diagnostics.
use dive_eval::{Options, St};
use dive_model::{BindOptions, Recipe};
use dive_trace::{Ending, Session, TraceDoc, Validation, ViewSourceEndpoint};
use dive_value::Value;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(
    name: &str,
    options: Options,
    run_metadata: bool,
) -> Result<(Session, dive_model::Model), Box<dyn std::error::Error>> {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../dive-check/tests/fixtures/errors/specs/{name}/{name}.tla"
    ));
    let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default())?;
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg"))?)
        .into_config()
        .ok_or("config")?;
    let model = dive_model::bind(
        ir.clone(),
        &config,
        &BindOptions {
            eval: options,
            ..BindOptions::tlc()
        },
    )?;
    Ok((
        Session::new(
            Recipe::new(ir, config, options, false),
            run_metadata.then(run_config),
        ),
        model,
    ))
}
fn run_config() -> dive_eval::RunConfig {
    dive_eval::RunConfig {
        deadlock: false,
        workers: 1,
        seed: 0,
        fingerprint: i64::from_ne_bytes(dive_value::fp64::POLYS[0].to_ne_bytes()),
        install: String::new(),
    }
}

#[test]
fn six_view_source_words_reconstruct_the_absent_candidate_and_diagnostic() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        for kind in [
            "Choose",
            "Domain",
            "NonBool",
            "NonBoolIf",
            "NonEnum",
            "TypeErr",
        ] {
            let (session, model) = fixture(&format!("ErView{kind}"), options, true)?;
            let error = model
                .eval(
                    &model.view.as_ref().ok_or("VIEW")?.pred,
                    &St::state(Arc::from([Some(Value::Int(2)), Some(Value::Int(0))])),
                )
                .err()
                .ok_or("actual VIEW failure")?;
            let endpoint =
                ViewSourceEndpoint::from_error(&model.ir, &error, 1000, 2103).ok_or("source")?;
            let mut doc = TraceDoc::behavior(vec![json!({"x":0,"y":0}), json!({"x":1,"y":0})])?
                .with_ending(Ending::ViewSourceError {
                    ec: error.ec().name().into(),
                    endpoint,
                });
            doc.length = Some(2);
            assert_eq!(doc.to_json()["version"], 6);
            assert_eq!(TraceDoc::from_json(&doc.to_json())?, doc);
            assert_eq!(
                session.try_read(&doc)?.0.validation,
                Validation::Valid,
                "{kind}"
            );
            assert!(matches!(
                session.read(&doc).0.validation,
                Validation::Unsupported(_)
            ));
            for (pointer, value) in [
                ("/ending/ec", json!("Absent")),
                ("/ending/endpoint/reported", json!(2111)),
                ("/ending/endpoint/deciding", json!(2179)),
                ("/ending/endpoint/source/module", json!("Wrong")),
                ("/states/1/values/x", json!(2)),
                ("/length", json!(1)),
            ] {
                let mut wrong = doc.to_json();
                *wrong.pointer_mut(pointer).ok_or("field")? = value;
                assert!(
                    !session
                        .try_read(&TraceDoc::from_json(&wrong)?)?
                        .0
                        .validation
                        .is_valid(),
                    "{wrong}"
                );
            }
            for version in 1..=5 {
                let mut wrong = doc.to_json();
                wrong["version"] = json!(version);
                assert!(TraceDoc::from_json(&wrong).is_err());
            }
        }
    }
    Ok(())
}
#[test]
fn invariant_word_validity_and_executed_diagnostic_identity_are_separate() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        for (name, inv, deciding) in [
            ("ErInvConstrained", "Inv", 2179),
            ("ErInvReplayError", "InvA", 2103),
        ] {
            let (session, _) = fixture(name, options, true)?;
            let word = TraceDoc::behavior(vec![
                json!({"x":0,"y":0}),
                json!({"x":1,"y":0}),
                json!({"x":2,"y":0}),
            ])?
            .with_ending(Ending::InvariantError {
                name: inv.into(),
                ec: "TLC_MODULE_DIVISION_BY_ZERO".into(),
            });
            assert_eq!(session.try_read(&word)?.0.validation, Validation::Valid);
            let (report, class) = session.invariant_error_class(&word, None)?;
            assert_eq!(report.validation, Validation::Valid, "{name}");
            let class = class.ok_or("independent class")?;
            assert_eq!(class.reported_code(), 2111);
            assert_eq!(class.deciding_code(), deciding);
        }
    }
    Ok(())
}

fn source_session(
    next: &str,
    init: &str,
    view: &str,
    extra: &str,
    cfg_extra: &str,
    options: Options,
) -> Result<(Session, TraceDoc), Box<dyn std::error::Error>> {
    let source = format!(
        "---- MODULE Prefix ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit == {init}\nNext == {next}\nView == {view}\n{extra}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Prefix.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(&format!(
        "INIT Init\nNEXT Next\nVIEW View\nCHECK_DEADLOCK FALSE\n{cfg_extra}"
    ))
    .into_config()
    .ok_or("config")?;
    let model = dive_model::bind(
        ir.clone(),
        &config,
        &BindOptions {
            eval: options,
            ..BindOptions::tlc()
        },
    )?;
    let error = model
        .eval(
            &model.view.as_ref().ok_or("VIEW")?.pred,
            &St::state(Arc::from([Some(Value::Int(1000))])),
        )
        .err()
        .ok_or("actual terminal error")?;
    let endpoint =
        ViewSourceEndpoint::from_error(&model.ir, &error, 1000, 2103).ok_or("actual source")?;
    let word = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])?.with_ending(
        Ending::ViewSourceError {
            ec: error.ec().name().into(),
            endpoint,
        },
    );
    Ok((
        Session::new(Recipe::new(ir, config, options, false), Some(run_config())),
        word,
    ))
}
const VIEW: &str = "IF x=1000 THEN (CHOOSE i \\in {1}: FALSE) ELSE x";
const NEXT: &str = "IF x=0 THEN x'=1 ELSE x'=1000";

#[test]
fn actual_prefix_order_exclusion_and_diagnostic_generation_are_required() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let (good, word) = source_session(NEXT, "x=0", VIEW, "", "", options)?;
        assert_eq!(good.try_read(&word)?.0.validation, Validation::Valid);
        for (next, init, extra, cfg, detail) in [
            (
                "IF x=0 THEN x'=1 ELSE x' \\in {1000,1001} /\\ (x'#1001 \\/ 1 \\div 0=0)",
                "x=0",
                "",
                "",
                "diagnostic",
            ),
            (NEXT, "x \\in {0,1}", "", "", "Init"),
            ("IF x=0 THEN x'=2 ELSE x'=1000", "x=0", "", "", "source"),
            (NEXT, "x=0", "C == x<1000", "CONSTRAINT C\n", "excluded"),
            (
                NEXT,
                "x=0",
                "C == x#1000 \\/ 1 \\div 0=0",
                "CONSTRAINT C\n",
                "constraint",
            ),
            (
                NEXT,
                "x=0",
                "C == x'<1000",
                "ACTION_CONSTRAINT C\n",
                "action constraint",
            ),
            (
                "IF x=0 THEN x' \\in 1..70 ELSE x'=1000",
                "x=0",
                "",
                "",
                "64 admitted",
            ),
            (
                "IF x=0 THEN (\\E i \\in 1..300: x'=1) ELSE x'=1000",
                "x=0",
                "",
                "",
                "256 emissions",
            ),
        ] {
            let (session, word) = source_session(next, init, VIEW, extra, cfg, options)?;
            let actual = session.try_read(&word)?.0.validation;
            assert!(!actual.is_valid(), "{detail}: {actual:?}");
            if detail == "diagnostic" {
                assert!(
                    matches!(&actual, Validation::Invalid(failure) if failure.detail.contains("diagnostic")),
                    "{actual:?}"
                );
                let mut wire = word.to_json();
                wire["ending"]["endpoint"]["deciding"] = json!(2179);
                assert_eq!(
                    session.try_read(&TraceDoc::from_json(&wire)?)?.0.validation,
                    Validation::Valid
                );
            }
            if detail == "64 admitted" || detail == "256 emissions" || detail == "Init" {
                assert!(
                    matches!(actual, Validation::Unsupported(_)),
                    "{detail}: {actual:?}"
                );
            }
        }
        // A false first constraint suppresses a later failing constraint and
        // VIEW demand. Neither the suppressed VIEW nor its source span proves a word.
        let (session, word) = source_session(
            NEXT,
            "x=0",
            VIEW,
            "A == x<1000\nB == x#1000 \\/ 1 \\div 0=0",
            "CONSTRAINT A B\n",
            options,
        )?;
        assert!(!session.try_read(&word)?.0.validation.is_valid());
        // VIEW x merges the second successor into the initial key. Exhaustion
        // of the complete first source still establishes the next source.
        let (session, word) = source_session(
            "IF x=0 THEN x' \\in {0,1} ELSE x'=1000",
            "x=0",
            VIEW,
            "",
            "",
            options,
        )?;
        assert_eq!(session.try_read(&word)?.0.validation, Validation::Valid);
    }
    Ok(())
}

#[derive(Debug)]
struct Stop;
impl dive_eval::HostControl for Stop {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        Err(dive_eval::Interrupted::new(91))
    }
}
#[test]
fn controlled_replay_stops_without_a_completed_view_or_invariant_certificate() -> TestResult {
    let (session, word) = source_session(NEXT, "x=0", VIEW, "", "", Options::tlc())?;
    assert!(matches!(
        session.read_controlled(&word, Arc::new(Stop)),
        Err(dive_trace::ValidationRunError::Interrupted { .. })
    ));
    let (session, _) = fixture("ErInvReplayError", Options::tlc(), true)?;
    let word = TraceDoc::behavior(vec![
        json!({"x":0,"y":0}),
        json!({"x":1,"y":0}),
        json!({"x":2,"y":0}),
    ])?
    .with_ending(Ending::InvariantError {
        name: "InvA".into(),
        ec: "TLC_MODULE_DIVISION_BY_ZERO".into(),
    });
    assert!(matches!(
        session.invariant_error_class(&word, Some(Arc::new(Stop))),
        Err(dive_trace::ValidationRunError::Interrupted { .. })
    ));
    Ok(())
}

#[test]
fn ignored_compound_concrete_fields_cannot_bypass_normalization_scope() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        let (session, mut word) = source_session(
            "x' = <<1>>",
            "x = <<0>>",
            "IF x=1000 \\/ x = <<1>> THEN (CHOOSE i \\in {1}: FALSE) ELSE 0",
            "",
            "",
            options,
        )?;
        word.states[0].insert("x".into(), json!({"seq":[0]}));
        word.states[1].insert("x".into(), json!({"seq":[1]}));
        assert!(matches!(session.try_read(&word)?.0.validation,
            Validation::Unsupported(ref reason) if reason.contains("scalar")));
    }
    Ok(())
}

#[test]
fn missing_run_polynomial_cannot_grant_an_admission_certificate() -> TestResult {
    let (session, _) = fixture("ErInvReplayError", Options::tlc(), false)?;
    let word = TraceDoc::behavior(vec![
        json!({"x":0,"y":0}),
        json!({"x":1,"y":0}),
        json!({"x":2,"y":0}),
    ])?
    .with_ending(Ending::InvariantError {
        name: "InvA".into(),
        ec: "TLC_MODULE_DIVISION_BY_ZERO".into(),
    });
    let (report, class) = session.invariant_error_class(&word, None)?;
    assert!(
        matches!(report.validation, Validation::Unsupported(ref why) if why.contains("polynomial"))
    );
    assert!(class.is_none());
    Ok(())
}

#[test]
fn diagnostic_generation_cap_counts_the_already_executed_prefix() -> TestResult {
    for options in [Options::dive(), Options::tlc()] {
        // Init plus ten first-source emissions plus the terminal primary
        // leave fewer than 250 slots. Each diagnostic action alone is below
        // 256, so a per-action cap would incorrectly certify this request.
        let (session, word) = source_session(
            "IF x=0 THEN x' \\in 1..10 ELSE (\\E i \\in 1..250: x'=1000)",
            "x=0",
            VIEW,
            "",
            "",
            options,
        )?;
        let (boundary, boundary_word) = source_session(
            "IF x=0 THEN x' \\in 1..10 ELSE (\\E i \\in 1..244: x'=1000)",
            "x=0",
            VIEW,
            "",
            "",
            options,
        )?;
        assert_eq!(
            boundary.try_read(&boundary_word)?.0.validation,
            Validation::Valid
        );
        let actual = session.try_read(&word)?.0.validation;
        assert!(
            matches!(&actual, Validation::Unsupported(reason) if reason.contains("256 cumulative emissions")),
            "{actual:?}"
        );
        // The cap is a completed explicit refusal; a real host interruption
        // still travels outside the validation report.
        assert!(matches!(
            session.read_controlled(&word, Arc::new(Stop)),
            Err(dive_trace::ValidationRunError::Interrupted { .. })
        ));
    }
    Ok(())
}
