//! Every retained genuine invariant family keeps its independently proved class.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn prior_agreements_keep_multi_init_records_instances_and_print_values() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (file, config) in [
        ("dive-api/core-instance-scopes/GuardError", "GuardError"),
        (
            "dive-api/core-m0/EnabConstInstanceError",
            "EnabConstInstanceError",
        ),
        (
            "dive-api/instance-naturals/LeftErrorFirst",
            "LeftErrorFirst",
        ),
        (
            "dive-api/instance-naturals/RightErrorFirst",
            "RightErrorFirst",
        ),
        ("dive-api/core-liveness/GenError/GenError", "InvariantError"),
        (
            "dive-check/disk-seen/ReplayLaterViolation/ReplayLaterViolation",
            "ReplayLaterViolation",
        ),
        (
            "dive-check/disk-seen/ReplayMembership/ReplayMembership",
            "ReplayMembership",
        ),
        ("dive-check/disk-seen/ReplayNew/ReplayNew", "ReplayNew"),
        (
            "dive-check/disk-seen/ReplayPrintWrapper/ReplayPrintWrapper",
            "ReplayPrintWrapper",
        ),
        ("dive-sim/SimInvariantError", "SimInvariantError"),
    ] {
        let (krate, relative) = file.split_once('/').ok_or("fixture")?;
        let file = root.join(format!("crates/{krate}/tests/fixtures/{relative}.tla"));
        let config = file.with_file_name(format!("{config}.cfg"));
        for compat_tlc in [false, true] {
            let model = SpecModel::load(
                &file,
                &config,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            {
                let engine = Engine::Core;
                for workers in [1, 4] {
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(config.clone()),
                            compat_tlc,
                            seed: Some(0),
                            fp: Some(0),
                            workers: Some(workers),
                            ..CheckRequest::default()
                        },
                        engine,
                    )?
                    .to_json();
                    assert_eq!(report["tlc"]["code"], 2111, "{file:?}/{engine:?}: {report}");
                    assert_eq!(report["tlc"]["verified"], true);
                    let word = TraceDoc::from_json(&report["counterexample"]["trace"])?;
                    let (proof, class) = model.invariant_error_class(&word, None)?;
                    assert_eq!(proof.validation, Validation::Valid, "{file:?}/{engine:?}");
                    assert_eq!(class.ok_or("certificate")?.deciding_code(), 2111);
                }
            }
        }
    }
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete retained family selection stays next to its common proof assertions"
)]
fn genuine_primary_wrappers_include_later_sources_and_value_method_causes() -> TestResult {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let modules = workspace.join("../dive-eval/tests/fixtures/community-modules");
    for (directory, names) in [
        (
            "tests/fixtures/core-lib",
            &[
                "ErAssertInv",
                "ErBagCardNonBag",
                "ErBagCupNonBag",
                "ErBagDiffNonFcn",
                "ErBagSqNonFcn",
                "ErBagUnionNonBag",
                "ErCmAllSubSeqsNonSeq",
                "ErCmContainsNonSeq",
                "ErCmInjectiveNonFcn",
                "ErCmLcpEmpty",
                "ErCmLcpNonSet",
                "ErCmRemoveFirstNonSeq",
                "ErCombineNonFcn",
                "ErCombineSecond",
                "ErFoldOnSetDomain",
                "ErFoldOnSetNonFcn",
                "ErFoldOnSetNonSet",
                "ErHoFoldLeftNonSeq",
                "ErHoInBody",
                "ErHoSelectSeqNonSeq",
                "ErPermNonSet",
            ][..],
        ),
        (
            "../dive-check/tests/fixtures/errors",
            &[
                "ErInvAssert",
                "ErInvChoose",
                "ErInvDivZero",
                "ErInvDomain",
                "ErInvEnabled",
                "ErInvNonBool",
                "ErInvNonBoolIf",
                "ErInvNonEnum",
                "ErInvOverflow",
                "ErInvReplayProp",
                "ErInvSecond",
                "ErInvTypeErr",
                "ErWrapInvTuple",
            ][..],
        ),
    ] {
        for name in names {
            let file = workspace
                .join(directory)
                .join(format!("specs/{name}/{name}.tla"));
            for compat_tlc in [false, true] {
                if !compat_tlc && *name == "ErInvOverflow" {
                    continue;
                }
                let model = SpecModel::load(
                    &file,
                    &file.with_extension("cfg"),
                    &TraceOptions {
                        compat_tlc,
                        run: Some((0, 0)),
                        module_paths: vec![modules.clone()],
                        ..TraceOptions::default()
                    },
                )?;
                for engine in std::iter::once(Engine::Core) {
                    for workers in [1, 4] {
                        let report = check_file_engine(
                            &CheckRequest {
                                file: file.clone(),
                                config: Some(file.with_extension("cfg")),
                                compat_tlc,
                                seed: Some(0),
                                fp: Some(0),
                                workers: Some(workers),
                                module_paths: vec![modules.clone()],
                                ..CheckRequest::default()
                            },
                            engine,
                        )?
                        .to_json();
                        if engine == Engine::Core && report["verdict"] == "unsupported" {
                            // Existing native evaluator refusals are outside this proof
                            // change. A new certification refusal must never hide here.
                            assert!(
                                !report["message"]
                                    .as_str()
                                    .ok_or("refusal reason")?
                                    .starts_with(
                                        "candidate primary/diagnostic lifecycle is unproved"
                                    ),
                                "{name}: {report}"
                            );
                            assert!(report["counterexample"].is_null());
                            continue;
                        }
                        assert_eq!(report["tlc"]["code"], 2111, "{name}/{engine:?}: {report}");
                        assert_eq!(report["tlc"]["verified"], true, "{name}/{engine:?}");
                        assert_eq!(report["tlc"]["exit_status"], 76, "{name}/{engine:?}");
                        let word = TraceDoc::from_json(&report["counterexample"]["trace"])?;
                        let (proof, class) = model.invariant_error_class(&word, None)?;
                        assert_eq!(proof.validation, Validation::Valid, "{name}/{engine:?}");
                        assert_eq!(class.ok_or("missing class")?.deciding_code(), 2111);
                    }
                }
            }
        }
    }
    Ok(())
}
