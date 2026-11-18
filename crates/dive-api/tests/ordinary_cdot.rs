//! The existing TLC composition option retains bounded final and replay limits.
#[path = "support/refused.rs"]
mod refused;
use dive_api::trace::{TraceDoc, Validation};
use dive_api::{ActionComposition, CheckRequest};
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
            // Only the retired reference engine ran TLC action composition
            // (#791, #910). Pinned TLC: `hold` succeeds (2193) with the
            // oracle's counts; `fail` was a named replay refusal.
            let request = spec.request(workers);
            let report = dive_api::check_file(&request)?;
            refused::assert_refused(
                &report.to_json(),
                report.exit_code(),
                refused::ACTION_COMPOSITION,
            );
            let mut explicit = request;
            explicit.temporal_refusal = Some(refused::FINAL_SHORT.into());
            let direct = dive_api::check_file(&explicit)?;
            assert_eq!(direct.verdict(), "unsupported");
            assert_eq!(direct.exit_code(), 3);
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
        // Only the retired reference engine ran TLC action composition
        // (#791, #910): it held `Hold` and violated `Fail` after 4
        // generated, 3 distinct states, depth 3, with a validated word.
        let request = spec.request(4);
        let report = dive_api::check_file(&request)?;
        refused::assert_refused(
            &report.to_json(),
            report.exit_code(),
            refused::ACTION_COMPOSITION,
        );
        for change in 0..2 {
            let mut excluded = request.clone();
            match change {
                0 => excluded.action_composition = Some(ActionComposition::Tla),
                _ => excluded.compat_tlc = false,
            }
            let refused = dive_api::check_file(&excluded)?;
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
            let actual = dive_api::check_file(&admitted)?;
            refused::assert_refused(
                &actual.to_json(),
                actual.exit_code(),
                refused::ACTION_COMPOSITION,
            );
        }
    }
    Ok(())
}

#[test]
fn safety_roles_keep_single_startup_and_original_binding_priority() -> TestResult {
    let source = "---- MODULE CompositionFinal ----\nEXTENDS Naturals,TLC\nVARIABLE x\nBoot==Print(\"one-startup\",RandomElement(1..32))\nInit==x=RandomElement(1..32)\nNext==x'=IF x<Boot THEN x+1 ELSE x\nHold==[](x<=32)\n====\n";
    let spec = Spec::new(source, "INIT Init\nNEXT Next\nPROPERTY Hold\n")?;
    for seed in 0..4 {
        let mut request = spec.request(4);
        request.seed = Some(seed);
        // Only the retired reference engine ran TLC action composition
        // (#791, #910); Core refuses it before start-up prints.
        let report = dive_api::check_file(&request)?;
        let json = report.to_json();
        refused::assert_refused(&json, report.exit_code(), refused::ACTION_COMPOSITION);
        assert_eq!(json["schema_version"], 3);
    }
    let missing = Spec::new(
        "---- MODULE CompositionFinal ----\nEXTENDS TLC\nCONSTANT C\nBoot==Print(\"must-not-start\",0)\nVARIABLE x\nInit==x=C\nNext==x'=x\nHold==<>(x=1)\n====\n",
        "INIT Init\nNEXT Next\nPROPERTY Hold\n",
    )?;
    let failure = dive_api::check_file(&missing.request(1))?;
    assert_eq!(failure.to_json()["schema_version"], 3);
    assert_eq!(failure.to_json()["output"], serde_json::json!([]));
    assert_ne!(failure.verdict(), "success");
    Ok(())
}

#[test]
fn composed_fairness_is_a_named_refusal() -> TestResult {
    let spec = Spec::new(&SOURCE.replace("WF_x(Fair)", "WF_x(Next)"), HOLD)?;
    let refused = dive_api::check_file(&spec.request(1))?.to_json();
    assert_eq!(refused["verdict"], "unsupported");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|m| m.contains("composition"))
    );
    assert!(refused["counterexample"].is_null());
    // The custom generated-state bound belonged to the retired reference
    // final route (#791); Core refuses TLC composition before any search.
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
