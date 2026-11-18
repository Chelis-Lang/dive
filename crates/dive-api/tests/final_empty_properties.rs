//! Literal-empty property selection uses the ordinary safety contract on
//! Core. These cases compared the retired reference final route with the
//! reference safety engine until #791; they now assert Core's results.
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;
use core_final::Outcome;
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_runtime::{CheckOptions, CheckResult, Verdict};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn source(body: &str) -> String {
    format!("---- MODULE EmptyFinal ----\nEXTENDS Naturals, TLC\n{body}\n====\n")
}

/// Core's safety result for `body` and `config`.
fn safety(
    body: &str,
    config: &str,
    options: &CheckOptions,
) -> Result<CheckResult, Box<dyn std::error::Error>> {
    let run = core_final::check(source(body), config, options).map_err(|e| e.json().to_string())?;
    match run.outcome() {
        Outcome::Safety(result) => Ok(result.as_ref().clone()),
        other => Err(format!("expected a safety result: {other:?}").into()),
    }
}

fn equal_safety(actual: &CheckResult, expected: &CheckResult) {
    assert_eq!(actual.verdict, expected.verdict);
    assert_eq!(actual.binding_model_error, expected.binding_model_error);
    assert_eq!(actual.stats, expected.stats);
    assert_eq!(actual.output, expected.output);
    assert_eq!(actual.warnings, expected.warnings);
    assert_eq!(actual.notes, expected.notes);
    assert_eq!(actual.var_names, expected.var_names);
    assert_eq!(actual.workers, expected.workers);
    assert_eq!(actual.tlc_prints_stats, expected.tlc_prints_stats);
    assert_eq!(
        actual.counterexample.as_ref().map(|c| c.states.len()),
        expected.counterexample.as_ref().map(|c| c.states.len())
    );
}

#[test]
fn empty_properties_preserve_safety_counts_profiles_and_workers() -> TestResult {
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for workers in [1, 4] {
            options.workers = workers;
            let actual = safety(
                "VARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x",
                "INIT Init\nNEXT Next\n",
                &options,
            )?;
            assert_eq!(actual.verdict, Verdict::Success);
            let stats = actual.stats.ok_or("stats")?;
            assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 3, 3));
        }
    }
    Ok(())
}

#[test]
fn no_properties_keeps_invariant_witness_and_independent_mutant_rejection() -> TestResult {
    let body = "VARIABLE x\nInit == x=0\nNext == x'=x+1\nInv == x=0";
    let config = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
    let options = CheckOptions::tlc(0, 0);
    let actual = safety(body, config, &options)?;
    assert!(
        matches!(&actual.verdict, Verdict::InvariantViolated { property, .. } if property=="Inv")
    );
    let states = &actual
        .counterexample
        .as_ref()
        .ok_or("counterexample")?
        .states;
    let ctx = options.eval.ctx;
    let values = states
        .iter()
        .map(|s| Ok(serde_json::json!({"x": dive_trace::value_json::to_json(&s.values[0], ctx)?})))
        .collect::<Result<Vec<_>, String>>()?;
    let doc = TraceDoc::behavior(values)?.with_ending(Ending::Invariant("Inv".into()));
    let validator = SpecModel::from_source(
        "EmptyFinal.tla",
        &source(body),
        config,
        &TraceOptions {
            compat_tlc: true,
            ..Default::default()
        },
    )?;
    assert_eq!(validator.validate(&doc).validation, Validation::Valid);
    let mut mutant = doc.clone();
    mutant.states[1].insert("x".into(), serde_json::json!(2));
    assert!(matches!(
        validator.validate(&mutant).validation,
        Validation::Invalid(_)
    ));
    Ok(())
}

#[test]
fn safety_bootstrap_output_is_not_evaluated_twice() -> TestResult {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let actual = safety(
            "VARIABLE x\nBoot == Print(7,0)\nInit == x=Boot\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
            &options,
        )?;
        assert_eq!(actual.output.len(), 1, "one actual startup Print");
    }
    Ok(())
}

#[test]
fn empty_properties_keep_binding_init_assumption_and_effect_refusal_priority() -> TestResult {
    for (body, cfg, kind) in [
        (
            "CONSTANT C\nVARIABLE x\nInit == x=C\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
            "model-error",
        ),
        (
            "VARIABLE x\nInit == 0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
            "evaluation-error",
        ),
        (
            "VARIABLE x\nInit == x=0\nNext == x'=1\\div0",
            "INIT Init\nNEXT Next\n",
            "evaluation-error",
        ),
        (
            "ASSUME FALSE\nVARIABLE x\nInit == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
            "assumption-failure",
        ),
    ] {
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let actual = safety(body, cfg, &options)?;
            assert_eq!(actual.verdict.kind(), kind, "{body}: {:?}", actual.verdict);
        }
    }
    // RandomElement under SYMMETRY is a named Core refusal.
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let report = core_final::run(
            &source("VARIABLE x\nInit == x=RandomElement({0,1})\nNext == x'=x\nV == x"),
            "INIT Init\nNEXT Next\nSYMMETRY V\n",
            &options,
        );
        let json = report.to_json();
        assert_ne!(report.verdict(), "success", "{json}");
    }
    Ok(())
}

#[test]
fn configured_temporal_properties_keep_real_verdict() -> TestResult {
    let run = core_final::check(
        source("VARIABLE x\nInit == x=0\nNext == x'=x\nP == []<>(x=1)"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
        &CheckOptions::dive(0),
    )
    .map_err(|e| e.json().to_string())?;
    assert!(
        matches!(run.outcome(), Outcome::Violation(v) if v.validation().validation == Validation::Valid && v.document().properties == ["P"])
    );
    Ok(())
}

#[test]
fn nonempty_safety_roles_take_the_safety_route_in_every_store() -> TestResult {
    for property in ["P == x=0", "P == [][x'=x]_x"] {
        let body = format!("VARIABLE x\nInit == x=0\nNext == x'=x\n{property}");
        let cfg = "INIT Init\nNEXT Next\nPROPERTY P\n";
        let options = CheckOptions::dive(0);
        assert_eq!(safety(&body, cfg, &options)?.verdict, Verdict::Success);
        let mut exact = options.clone();
        exact.store = dive_store::StoreMode::Exact;
        assert_eq!(safety(&body, cfg, &exact)?.verdict, Verdict::Success);
    }
    Ok(())
}

#[test]
fn final_safety_reports_actual_binding_origin_not_model_error_class() -> TestResult {
    let dir = std::env::temp_dir().join(format!(
        "dive-final-origin-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&dir)?;
    let scratch = Scratch(dir);
    for (body, cfg, code, binding) in [
        (
            "Init == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\n_PERIODIC Missing\n",
            2229,
            true,
        ),
        (
            "Init == x=0\nNext == x'=x\nArg(a) == a",
            "INIT Init\nNEXT Next\n_PERIODIC Arg\n",
            2228,
            true,
        ),
        (
            "Init == x=0\nNext == x'=x\nSym == TRUE",
            "INIT Init\nNEXT Next\nSYMMETRY Sym\n",
            1000,
            true,
        ),
        ("Init == x=0", "INIT Init\n", 2115, false),
        (
            "Init == x=0\nNext == x'=x\nPost == TRUE",
            "INIT Init\nNEXT Next\nPOSTCONDITION Post\n",
            1000,
            false,
        ),
        (
            "Init == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\nPOSTCONDITION Missing\n",
            1000,
            false,
        ),
    ] {
        let file = scratch.0.join("Origin.tla");
        std::fs::write(
            &file,
            format!("---- MODULE Origin ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n"),
        )?;
        std::fs::write(file.with_extension("cfg"), cfg)?;
        for compat_tlc in [false, true] {
            let request = dive_api::CheckRequest {
                file: file.clone(),
                compat_tlc,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..Default::default()
            };
            let ordinary = dive_api::check_file(&request)?;
            let selected = dive_api::check_file(&dive_api::CheckRequest {
                temporal_refusal: Some(refused::FINAL_SHORT.into()),
                ..request
            })?;
            if cfg.contains("POSTCONDITION") {
                // Only the retired reference engine bound POSTCONDITION
                // (#791, #910); Core refuses it before binding.
                for report in [&ordinary, &selected] {
                    refused::assert_refused(
                        &report.to_json(),
                        report.exit_code(),
                        refused::POSTCONDITION,
                    );
                }
                continue;
            }
            let result = selected.result().ok_or("safety result")?;
            equal_safety(result, ordinary.result().ok_or("ordinary result")?);
            assert!(
                matches!(result.verdict, Verdict::ModelError { code: actual, .. } if actual == code),
                "{cfg}: {:?}",
                result.verdict
            );
            assert_eq!(result.binding_model_error, binding, "{cfg}");
            // Without temporal properties the refused strategy changes
            // nothing: the report is the ordinary safety report.
            let json = selected.to_json();
            assert_eq!(json["check_kind"], "safety");
            assert!(json["execution"].is_null(), "{json}");
        }
    }
    Ok(())
}
