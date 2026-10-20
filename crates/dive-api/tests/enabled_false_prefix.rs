//! Issue #320: a false prefix inside `ENABLED` leaves a later conjunct, here an
//! equality of an erroring startup set filter, unevaluated. The specs are the
//! `enabled-false-prefix` corpus controls; their goldens come from the pinned
//! TLC through the harness.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, TemporalMode};
use serde_json::Value;
use std::path::{Path, PathBuf};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn spec(name: &str) -> (PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/enabled-false-prefix/specifications")
        .join(name);
    (
        dir.join(format!("{name}.tla")),
        dir.join(format!("{name}.cfg")),
    )
}

fn check(
    name: &str,
    compat_tlc: bool,
    temporal: TemporalMode,
) -> Result<Value, Box<dyn std::error::Error>> {
    let (file, config) = spec(name);
    Ok(dive_api::check_file(&CheckRequest {
        file,
        config: Some(config),
        compat_tlc,
        temporal,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..Default::default()
    })?
    .to_json_with_exit_convention(ExitConvention::Tlc))
}

fn load(name: &str, compat_tlc: bool) -> Result<SpecModel, Box<dyn std::error::Error>> {
    let (file, config) = spec(name);
    Ok(SpecModel::load(
        &file,
        &config,
        &TraceOptions {
            compat_tlc,
            run: Some((0, 0)),
            ..Default::default()
        },
    )?)
}

fn validation(model: &SpecModel, word: &TraceDoc) -> (Validation, Validation) {
    let (document, typed) = model.read(word);
    let typed = typed.map_or_else(
        || document.validation.clone(),
        |typed| model.validate_trace(&typed).validation,
    );
    (document.validation, typed)
}

#[test]
fn false_enabled_prefix_reports_the_violation_with_a_validated_word() -> TestResult {
    for compat_tlc in [true, false] {
        for temporal in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
            // Pinned TLC: temporal property `Prop` violated, 2116, exit 13,
            // 2 generated, 1 distinct, depth 1, no division error.
            let report = check("StartupFilterUnreachedError", compat_tlc, temporal)?;
            assert_eq!(report["verdict"], "liveness-violation", "{report}");
            assert_eq!(report["exit_code"], 13);
            assert_eq!(report["property"], "Prop");
            assert_eq!(report["statistics"]["generated"], 2);
            assert_eq!(report["statistics"]["distinct"], 1);
            assert_eq!(report["statistics"]["depth"], 1);
            let genuine = TraceDoc::from_json(&report["counterexample"]["trace"])?;
            let model = load("StartupFilterUnreachedError", compat_tlc)?;
            let (document, typed) = validation(&model, &genuine);
            assert_eq!(document, Validation::Valid, "{compat_tlc} {temporal:?}");
            assert_eq!(typed, Validation::Valid, "{compat_tlc} {temporal:?}");

            // A word through a state the spec never reaches is forged.
            let mut unreachable = genuine.clone();
            unreachable.states[0].insert("x".into(), 1.into());
            let (document, typed) = validation(&model, &unreachable);
            assert!(matches!(document, Validation::Invalid(_)), "{document:?}");
            assert!(matches!(typed, Validation::Invalid(_)), "{typed:?}");

            // Where the action is enabled the same lasso satisfies `Prop`, so
            // claiming a violation of it is forged.
            for holds in ["StartupFilterReachedTrue", "StartupFilterAliasTrue"] {
                let (document, typed) = validation(&load(holds, compat_tlc)?, &genuine);
                assert!(
                    matches!(document, Validation::Invalid(_)),
                    "{holds}: {document:?}"
                );
                assert!(
                    matches!(typed, Validation::Invalid(_)),
                    "{holds}: {typed:?}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn true_controls_hold_in_both_profiles() -> TestResult {
    for name in ["StartupFilterReachedTrue", "StartupFilterAliasTrue"] {
        for compat_tlc in [true, false] {
            // Pinned TLC: success, 2193, exit 0, 2 generated, 1 distinct, depth 1.
            let report = check(name, compat_tlc, TemporalMode::Ordinary)?;
            assert_eq!(report["verdict"], "success", "{name}: {report}");
            assert_eq!(report["exit_code"], 0);
            assert_eq!(report["statistics"]["generated"], 2);
            assert_eq!(report["statistics"]["distinct"], 1);
            assert_eq!(report["statistics"]["depth"], 1);
        }
    }
    Ok(())
}

#[test]
fn reached_filter_equality_reaches_the_division_error_in_the_tlc_profile() -> TestResult {
    // The paired control without the false prefix: pinned TLC reaches the
    // division by zero (2179) and exits 255. dive reaches the same error; how
    // it reports a liveness-atom error is tracked by issue #147. A success
    // would mean the equality was skipped.
    let dir =
        std::env::temp_dir().join(format!("dive-enabled-false-prefix-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let (source, config) = spec("StartupFilterUnreachedError");
    let reached = std::fs::read_to_string(source)?
        .replace(
            "MODULE StartupFilterUnreachedError",
            "MODULE StartupFilterReachedError",
        )
        .replace("A==FALSE /\\ x'=0 /\\ D=D", "A==x'=0 /\\ D=D");
    assert!(reached.contains("A==x'=0 /\\ D=D"));
    let file = dir.join("StartupFilterReachedError.tla");
    std::fs::write(&file, reached)?;
    let result = (|| -> TestResult {
        let report = dive_api::check_file(&CheckRequest {
            file: file.clone(),
            config: Some(config.clone()),
            compat_tlc: true,
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            ..Default::default()
        })?
        .to_json_with_exit_convention(ExitConvention::Tlc);
        assert_ne!(report["verdict"], "success", "{report}");
        assert!(report.to_string().contains("2179"), "{report}");
        Ok(())
    })();
    std::fs::remove_dir_all(&dir)?;
    result
}
