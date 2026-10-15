//! Literal-empty property selection uses the existing ordinary safety contract.
use dive_api::temporal::{self, Outcome};
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_liveness::analyze::Limits;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn input(
    body: &str,
    config: &str,
) -> Result<(Arc<dive_ir::Ir>, dive_config::ModelConfig), Box<dyn std::error::Error>> {
    let source = format!("---- MODULE EmptyFinal ----\nEXTENDS Naturals, TLC\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("EmptyFinal.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    Ok((
        Arc::new(dive_ir::lower(&analysis)?),
        dive_config::parse(config).into_config().ok_or("config")?,
    ))
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
    let (ir, cfg) = input(
        "VARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x",
        "INIT Init\nNEXT Next\n",
    )?;
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for workers in [1, 4] {
            options.workers = workers;
            let expected = dive_check::check(&ir, &cfg, &options);
            let report = temporal::check_final(&ir, &cfg, &options, Limits::default())?;
            let Outcome::Safety(actual) = report.outcome() else {
                return Err("no temporal outcome for empty property config".into());
            };
            equal_safety(actual, &expected);
            assert_eq!(actual.verdict, Verdict::Success);
            let stats = actual.stats.ok_or("stats")?;
            assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 3, 3));
        }
    }
    Ok(())
}

#[test]
fn empty_properties_do_not_apply_temporal_capture_bounds() -> TestResult {
    let (ir, cfg) = input(
        "VARIABLE x\nInit == x=0\nNext == x'=x",
        "INIT Init\nNEXT Next\n",
    )?;
    let limits = Limits {
        properties: 0,
        atom_cells: 0,
        capture: dive_check::capture::Limits {
            states: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let report = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), limits)?;
    assert!(matches!(report.outcome(), Outcome::Safety(r) if r.verdict == Verdict::Success));
    Ok(())
}

#[test]
fn no_properties_keeps_invariant_witness_and_independent_mutant_rejection() -> TestResult {
    let body = "VARIABLE x\nInit == x=0\nNext == x'=x+1\nInv == x=0";
    let config = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
    let (ir, cfg) = input(body, config)?;
    let options = CheckOptions::tlc(0, 0);
    let report = temporal::check_final(&ir, &cfg, &options, Limits::default())?;
    let Outcome::Safety(actual) = report.outcome() else {
        return Err("safety witness".into());
    };
    equal_safety(actual, &dive_check::check(&ir, &cfg, &options));
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
    let source = format!("---- MODULE EmptyFinal ----\nEXTENDS Naturals, TLC\n{body}\n====\n");
    let validator = SpecModel::from_source(
        "EmptyFinal.tla",
        &source,
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
    let (ir, cfg) = input(
        "VARIABLE x\nBoot == Print(7,0)\nInit == x=Boot\nNext == x'=x",
        "INIT Init\nNEXT Next\n",
    )?;
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let expected = dive_check::check(&ir, &cfg, &options);
        assert_eq!(expected.output.len(), 1, "actual startup Print baseline");
        let report = temporal::check_final(&ir, &cfg, &options, Limits::default())?;
        let Outcome::Safety(actual) = report.outcome() else {
            return Err("ordinary effects".into());
        };
        equal_safety(actual, &expected);
        assert_eq!(actual.output.len(), 1);
    }
    Ok(())
}

#[test]
fn empty_properties_keep_binding_init_assumption_and_effect_refusal_priority() -> TestResult {
    for (body, cfg) in [
        (
            "CONSTANT C\nVARIABLE x\nInit == x=C\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
        ),
        (
            "VARIABLE x\nInit == 0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
        ),
        (
            "VARIABLE x\nInit == x=0\nNext == x'=1\\div0",
            "INIT Init\nNEXT Next\n",
        ),
        (
            "ASSUME FALSE\nVARIABLE x\nInit == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
        ),
        (
            "VARIABLE x\nInit == x=RandomElement({0,1})\nNext == x'=x\nV == x",
            "INIT Init\nNEXT Next\nSYMMETRY V\n",
        ),
    ] {
        let (ir, cfg) = input(body, cfg)?;
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let expected = dive_check::check(&ir, &cfg, &options);
            assert_ne!(expected.verdict, Verdict::Success);
            let report = temporal::check_final(&ir, &cfg, &options, Limits::default())?;
            let Outcome::Safety(actual) = report.outcome() else {
                return Err("ordinary failure".into());
            };
            equal_safety(actual, &expected);
        }
    }
    Ok(())
}

#[test]
fn configured_temporal_properties_keep_real_verdict_and_resource_route() -> TestResult {
    let (ir, cfg) = input(
        "VARIABLE x\nInit == x=0\nNext == x'=x\nP == []<>(x=1)",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    let report = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())?;
    assert!(
        matches!(report.outcome(), Outcome::Violation(v) if v.validation().validation == Validation::Valid && v.document().properties == ["P"])
    );
    let limits = Limits {
        properties: 0,
        ..Default::default()
    };
    assert!(temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), limits).is_err());
    Ok(())
}

#[test]
fn nonempty_safety_roles_require_the_narrow_default_memory_route() -> TestResult {
    for property in ["P == x=0", "P == [][x'=x]_x"] {
        let (ir, cfg) = input(
            &format!("VARIABLE x\nInit == x=0\nNext == x'=x\n{property}"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        let options = CheckOptions::dive(0);
        let report = temporal::check_final(&ir, &cfg, &options, Limits::default())?;
        assert!(matches!(report.outcome(), Outcome::Safety(r) if r.verdict==Verdict::Success));
        let mut outside_scope = options;
        outside_scope.store = dive_store::StoreMode::Exact;
        let error = temporal::check_final(&ir, &cfg, &outside_scope, Limits::default())
            .err()
            .ok_or("existing nonempty property route")?;
        assert!(matches!(
            error,
            temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Unsupported(
                "no temporal properties"
            ))
        ));
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
                temporal: dive_api::TemporalMode::FinalShort,
                ..request
            })?;
            let result = selected.result().ok_or("safety result")?;
            equal_safety(result, ordinary.result().ok_or("ordinary result")?);
            assert!(
                matches!(result.verdict, Verdict::ModelError { code: actual, .. } if actual == code),
                "{cfg}: {:?}",
                result.verdict
            );
            assert_eq!(result.binding_model_error, binding, "{cfg}");
            let json = selected.to_json();
            assert_eq!(
                json["execution"]["stage"],
                if binding { "binding" } else { "capture" }
            );
            assert_eq!(json["execution"]["capture_completed"], false);
        }
    }
    Ok(())
}
