//! TLC logical coverage is a named refusal on Core.
//!
//! Only the retired reference engine reported TLC's logical coverage rows
//! (#791). Each request the earlier tests made with `--coverage` now meets
//! Core's named refusal, and the same request without coverage still runs.
//! The pinned TLC rows stay in the corpus; #910 points to this file's earlier
//! form, to restore when Core implements logical coverage.
use dive_api::{ApiError, CheckRequest, check_file};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const FEATURE: &str = "source coverage and TLC logical coverage";

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/tlc-coverage-primary/specifications/Coverage")
        .join(format!("{name}.tla"))
}
fn assert_refused(request: &CheckRequest) {
    let result = check_file(request);
    assert!(
        matches!(&result, Err(ApiError::Unsupported { feature }) if feature.contains(FEATURE)),
        "expected the coverage refusal, got {result:?}"
    );
}
fn assert_plain_runs_and_coverage_refuses(
    file: &std::path::Path,
    limit: Option<u64>,
) -> TestResult {
    for compat in [false, true] {
        let mut request = CheckRequest {
            file: file.to_path_buf(),
            compat_tlc: compat,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            max_states: limit,
            ..CheckRequest::default()
        };
        let plain = check_file(&request)?.to_json();
        assert!(plain.get("tlc_coverage").is_none(), "{plain}");
        request.tlc_coverage = true;
        assert_refused(&request);
    }
    Ok(())
}

#[test]
fn pinned_coverage_fixtures_refuse_coverage_and_keep_their_plain_results() -> TestResult {
    for name in [
        "LogicalCoverage",
        "Multi",
        "MultiDomains",
        "Guards",
        "IfAction",
        "InitialFilter",
        "LazyEffects",
        "SharedAction",
        "QuantSet",
        "MultiQuant",
        "QuantShort",
        "Headers",
        "Unreached",
        "PrimeArg",
        "PrimeAlias",
        "AliasPrimeTarget",
        "HiddenPrimeRhs",
        "SharedMarker",
        "MarkerLazyActual",
        "EmptyActionMarker",
        "FailDemand",
    ] {
        assert_plain_runs_and_coverage_refuses(&fixture(name), None)?;
    }
    assert_plain_runs_and_coverage_refuses(&fixture("Multi"), Some(1))
}

#[test]
fn logical_coverage_requires_an_explicit_supported_backend_and_separate_metric() {
    let mut request = CheckRequest {
        file: fixture("LogicalCoverage"),
        tlc_coverage: true,
        ..CheckRequest::default()
    };
    assert_refused(&request);
    request.source_coverage = true;
    assert!(matches!(
        check_file(&request),
        Err(ApiError::Unsupported { .. })
    ));
}

#[test]
fn compound_values_and_captured_let_contexts_refuse_coverage() -> TestResult {
    let directory = std::env::temp_dir().join(format!("dive-logical-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    for (name, source) in [
        (
            "Compound",
            "---- MODULE Compound ----\nVARIABLE x\nInit == x={0}\nNext == x'=x\n====\n",
        ),
        (
            "Captured",
            "---- MODULE Captured ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == LET Unused == PrintT(\"unused\") IN x'=x\n====\n",
        ),
    ] {
        let file = directory.join(format!("{name}.tla"));
        std::fs::write(&file, source)?;
        std::fs::write(file.with_extension("cfg"), "INIT Init\nNEXT Next\n")?;
        assert_plain_runs_and_coverage_refuses(&file, None)?;
    }
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
