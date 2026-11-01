//! Word/report proof does not classify an ambiguous pinned oracle stream.
use dive_api::trace::{SpecModel, TraceOptions};
use dive_api::{CheckRequest, Engine, check_file_engine};
use dive_conformance::compare::{CompareContext, diff_results};
use dive_conformance::dive::{CheckMapping, check_result_with_model};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::{RunFacts, Verdict};
use dive_conformance::tlc::{messages, result};
use dive_conformance::trace_check::validate_counterexample;
use std::path::PathBuf;
use std::time::Duration;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn ten_family_shadows_validate_without_promoting_unclassified_tlc_streams() -> TestResult {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-check/tests/fixtures/errors");
    for name in [
        "ErViewChoose",
        "ErViewDomain",
        "ErViewNonBool",
        "ErViewNonBoolIf",
        "ErViewNonEnum",
        "ErViewTypeErr",
        "ErInvConstrained",
        "ErInvReplayError",
        "ErConstrNonBool",
        "ErActConstrNonBool",
    ] {
        let file = root.join(format!("specs/{name}/{name}.tla"));
        let clean =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))?;
        let exit =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))?
                .trim()
                .parse()?;
        let oracle = result::classify(
            &messages::parse(&clean),
            &RunFacts {
                exit_code: Some(exit),
                ..RunFacts::default()
            },
        );
        assert_eq!(oracle.verdict, Verdict::Unclassified, "{name}");
        for compat_tlc in [false, true] {
            let model = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            for workers in [1, 4] {
                let mut pair = Vec::new();
                for engine in [Engine::Reference, Engine::Core] {
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            workers: Some(workers),
                            seed: Some(0),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )?;
                    let wire = report.to_json();
                    let outcome = ProcessOutcome {
                        exit_code: Some(i32::from(report.exit_code())),
                        signal: None,
                        timed_out: false,
                        stdout: wire.to_string(),
                        stderr: String::new(),
                        elapsed: Duration::ZERO,
                    };
                    let CheckMapping::Result(actual) =
                        check_result_with_model(&outcome, compat_tlc, &model)
                    else {
                        return Err(format!(
                            "source-bound mapping refused {name}/{engine:?}: {wire}"
                        )
                        .into());
                    };
                    assert!(
                        validate_counterexample(&model, &actual, &oracle).validated,
                        "{name}/{engine:?}"
                    );
                    pair.push(actual);
                }
                assert!(
                    diff_results(
                        &pair[0],
                        &pair[1],
                        CompareContext {
                            workers: u32::try_from(workers)?,
                            counts_comparable: true
                        }
                    )
                    .is_empty(),
                    "{name}/{compat_tlc}/{workers}"
                );
            }
        }
    }
    Ok(())
}
