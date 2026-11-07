//! The existing TLC composition option retains bounded final and replay limits.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{ActionComposition, CheckRequest, ExitConvention, TemporalMode};
use std::path::PathBuf;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Spec(PathBuf);
impl Spec {
    fn new(source: &str, config: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!(
            "dive-ordinary-cdot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&directory)?;
        let file = directory.join("CompositionFinal.tla");
        std::fs::write(&file, source)?;
        std::fs::write(file.with_extension("cfg"), config)?;
        Ok(Self(file))
    }
    fn request(&self, workers: usize) -> CheckRequest {
        CheckRequest {
            file: self.0.clone(),
            compat_tlc: true,
            action_composition: Some(ActionComposition::Tlc),
            seed: Some(0),
            fp: Some(0),
            workers: Some(workers),
            ..CheckRequest::default()
        }
    }
}
impl Drop for Spec {
    fn drop(&mut self) {
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
const SOURCE: &str = include_str!("fixtures/ordinary-cdot/CompositionFinal.tla");
const HOLD: &str = include_str!("fixtures/ordinary-cdot/hold.cfg");
const FAIL: &str = include_str!("fixtures/ordinary-cdot/fail.cfg");

#[test]
fn actual_primary_hold_retains_intermediate_counts_and_fail_retains_replay_refusal() -> TestResult {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ordinary-cdot/oracle.json"))?;
    let cases = oracle["cases"].as_array().ok_or("primary cases")?;
    assert_eq!(cases.len(), 2);
    for case in cases {
        let holds = case["case"] == "hold";
        let spec = Spec::new(SOURCE, if holds { HOLD } else { FAIL })?;
        for workers in [1, 4] {
            let request = spec.request(workers);
            let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(value["schema_version"], 3);
            assert_eq!(value["options"]["profile"], "tlc");
            assert_eq!(value["execution"]["capture_completed"], true);
            for field in ["generated", "distinct", "depth"] {
                assert_eq!(value["statistics"][field], case[field], "{value}");
            }
            assert_eq!(value["options"]["workers"]["requested"], workers);
            assert_eq!(value["options"]["workers"]["used"], 1);
            assert_eq!(value["options"]["workers"]["sequential"], true);
            if holds {
                assert_eq!(report.verdict(), "success");
                assert_eq!(value["exit_code"], case["physical_exit"]);
                assert_eq!(value["tlc"]["code"], 2193);
            } else {
                assert_eq!(report.verdict(), "unsupported");
                assert_eq!(value["execution"]["stage"], "validation");
                assert_eq!(value["exit_code"], 3);
                assert!(
                    value["message"]
                        .as_str()
                        .is_some_and(|m| m.contains("composition"))
                );
            }
            assert!(value["counterexample"].is_null());
            assert!(value["property"].is_null());
            let mut explicit = request;
            explicit.temporal = TemporalMode::FinalShort;
            let direct =
                dive_api::check_file_engine(&explicit, dive_api::Engine::Reference)?.to_json();
            assert_eq!(direct["verdict"], value["verdict"]);
            assert_eq!(direct["statistics"], value["statistics"]);
        }
    }
    Ok(())
}

#[test]
fn composition_option_alone_keeps_noncomposed_positive_and_forged_word_controls() -> TestResult {
    let source = "---- MODULE CompositionFinal ----\nEXTENDS Naturals\nVARIABLE x\nInit==x=0\nNext==x'=IF x<2 THEN x+1 ELSE x\nHold==[]<>(x<=2)\nFail==<>(x=3)\n====\n";
    for property in ["Hold", "Fail"] {
        let spec = Spec::new(
            source,
            &format!("INIT Init\nNEXT Next\nPROPERTY {property}\n"),
        )?;
        let request = spec.request(4);
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
        let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
        assert_eq!(value["schema_version"], 3);
        assert_eq!(value["statistics"]["generated"], 4);
        assert_eq!(value["statistics"]["distinct"], 3);
        assert_eq!(value["statistics"]["depth"], 3);
        assert_eq!(
            report.verdict(),
            if property == "Hold" {
                "success"
            } else {
                "liveness-violation"
            }
        );
        if property == "Fail" {
            let model = SpecModel::load(
                &spec.0,
                &spec.0.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc: true,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            let mut document = TraceDoc::from_json(&value["counterexample"]["trace"])?;
            assert_eq!(model.read(&document).0.validation, Validation::Valid);
            document.states[0].insert("x".into(), 99.into());
            assert!(matches!(
                model.read(&document).0.validation,
                Validation::Invalid(_)
            ));
        }
        for change in 0..2 {
            let mut excluded = request.clone();
            match change {
                0 => excluded.action_composition = Some(ActionComposition::Tla),
                _ => excluded.compat_tlc = false,
            }
            let refused = dive_api::check_file_engine(&excluded, dive_api::Engine::Reference)?;
            assert_eq!(refused.verdict(), "unsupported");
            assert_eq!(refused.to_json()["schema_version"], 3);
        }
        // Store, polynomial, sequential and deadlock settings keep the same
        // composed temporal result as the default-option request.
        for change in 0..4 {
            let mut admitted = request.clone();
            match change {
                0 => admitted.fp = Some(1),
                1 => admitted.sequential = true,
                2 => admitted.store = dive_api::Store::Exact,
                _ => admitted.no_deadlock = true,
            }
            let actual =
                dive_api::check_file_engine(&admitted, dive_api::Engine::Reference)?.to_json();
            assert_eq!(actual["verdict"], value["verdict"], "{admitted:?}");
            assert_eq!(actual["property"], value["property"]);
            assert_eq!(actual["statistics"], value["statistics"]);
        }
    }
    Ok(())
}

#[test]
fn safety_roles_keep_single_startup_and_original_binding_priority() -> TestResult {
    let source = "---- MODULE CompositionFinal ----\nEXTENDS Naturals,TLC\nVARIABLE x\nBoot==Print(\"one-startup\",RandomElement(1..32))\nInit==x=RandomElement(1..32)\nNext==x'=IF x<Boot THEN x+1 ELSE x\nHold==[](x<=32)\n====\n";
    let spec = Spec::new(source, "INIT Init\nNEXT Next\nPROPERTY Hold\n")?;
    let analysis = dive_sema::analyze_file(&spec.0, &dive_sema::Options::default())?;
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Hold\n")
        .into_config()
        .ok_or("cfg")?;
    for seed in 0..4 {
        let mut request = spec.request(4);
        request.seed = Some(seed);
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Reference)?;
        let actual = report.result().ok_or("ordinary safety result")?;
        let mut options = dive_check::CheckOptions::tlc(0, seed);
        options.eval.action_composition = ActionComposition::Tlc;
        options.workers = 4;
        let direct = dive_check::check(&ir, &cfg, &options);
        assert_eq!(actual.verdict, direct.verdict);
        assert_eq!(actual.stats, direct.stats);
        assert_eq!(actual.output, direct.output);
        assert_eq!(actual.output.len(), 1);
        assert_eq!(report.to_json()["schema_version"], 3);
    }
    let missing = Spec::new(
        "---- MODULE CompositionFinal ----\nEXTENDS TLC\nCONSTANT C\nBoot==Print(\"must-not-start\",0)\nVARIABLE x\nInit==x=C\nNext==x'=x\nHold==<>(x=1)\n====\n",
        "INIT Init\nNEXT Next\nPROPERTY Hold\n",
    )?;
    let failure = dive_api::check_file_engine(&missing.request(1), dive_api::Engine::Reference)?;
    assert_eq!(failure.to_json()["schema_version"], 3);
    assert_eq!(failure.to_json()["output"], serde_json::json!([]));
    assert_ne!(failure.verdict(), "success");
    Ok(())
}

#[test]
fn composed_fairness_and_custom_generated_bound_keep_their_original_limits() -> TestResult {
    let spec = Spec::new(&SOURCE.replace("WF_x(Fair)", "WF_x(Next)"), HOLD)?;
    let refused =
        dive_api::check_file_engine(&spec.request(1), dive_api::Engine::Reference)?.to_json();
    assert_eq!(refused["verdict"], "unsupported");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|m| m.contains("composition"))
    );
    assert!(refused["counterexample"].is_null());
    let analysis = dive_sema::analyze_source(
        "CompositionFinal.tla",
        SOURCE,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse(HOLD).into_config().ok_or("cfg")?;
    let mut options = dive_check::CheckOptions::tlc(0, 0);
    options.eval.action_composition = ActionComposition::Tlc;
    let mut limits = dive_liveness::analyze::Limits::default();
    limits.capture.generated = 1;
    let failure = dive_api::temporal::check_final_with_evidence(&ir, &cfg, &options, limits)?;
    assert!(
        matches!(failure.outcome(), dive_api::temporal::Outcome::Safety(result) if matches!(result.verdict, dive_check::Verdict::ResourceLimit { .. }))
    );
    Ok(())
}

#[test]
fn actual_primary_word_validates_tla_composition_and_refuses_tlc_accounting() -> TestResult {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ordinary-cdot/oracle.json"))?;
    let analysis = dive_sema::analyze_source(
        "CompositionFinal.tla",
        SOURCE,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse(FAIL).into_config().ok_or("cfg")?;
    for mode in [ActionComposition::Tlc, ActionComposition::Tla] {
        let mut options = dive_model::BindOptions::tlc();
        options.eval.action_composition = mode;
        let model = dive_model::bind(ir.clone(), &cfg, &options)?;
        let states = oracle["word"]["states"]
            .as_array()
            .ok_or("primary states")?
            .clone();
        assert_eq!(oracle["word"]["ending"], "stuttering");
        let mut document = TraceDoc::behavior(states)?.with_ending(dive_trace::Ending::Stuttering);
        document.properties = vec![
            oracle["word"]["property"]
                .as_str()
                .ok_or("property")?
                .into(),
        ];
        let validation = dive_trace::validate_doc(&model, &document, &[])
            .0
            .validation;
        if mode == ActionComposition::Tla {
            // FALSE -> TRUE follows First then Last; stuttering TRUE
            // satisfies WF_x(Fair) and violates the named []<>~x property.
            assert_eq!(validation, Validation::Valid);
        } else {
            assert_eq!(
                validation,
                Validation::Unsupported(format!(
                    "unsupported temporal validation: {}",
                    dive_trace::COMPOSITION_UNSUPPORTED
                ))
            );
        }
        document.states[1].insert("x".into(), false.into());
        let forged = dive_trace::validate_doc(&model, &document, &[])
            .0
            .validation;
        if mode == ActionComposition::Tla {
            assert!(matches!(forged, Validation::Invalid(_)));
        } else {
            assert_ne!(forged, Validation::Valid);
        }
        assert_eq!(model.evaluator.take_output(), [] as [String; 0]);
    }
    Ok(())
}
