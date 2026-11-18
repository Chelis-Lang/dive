//! Source coverage is a named refusal on Core.
//!
//! Only the retired reference engine counted source work (#791). Every request
//! the earlier tests made with `--source-coverage` now meets Core's named
//! refusal before model loading, and the same request without coverage keeps
//! its result. #910 points to this file's earlier form, to restore when Core
//! implements coverage.
#![allow(
    clippy::unwrap_used,
    reason = "test fixture construction and assertions require admitted source rows"
)]
use dive_api::{ApiError, CheckRequest, TemporalMode, check_file};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const SOURCE: &str = "---- MODULE Coverage ----\nEXTENDS Naturals,TLC\nVARIABLE x\nC == 1+2\nASSUME C=3\nInit == x=0\nNext == \\/ /\\ x<2 /\\ x'=x+1\n        \\/ /\\ x=2 /\\ x'=x\nInv == x<=2\nUnused == x=99\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
const FEATURE: &str = "source coverage and TLC logical coverage";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project {
    dir: PathBuf,
    file: PathBuf,
}
impl Project {
    fn new(source: &str, config: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dive-source-coverage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Coverage.tla");
        fs::write(&file, source).unwrap();
        fs::write(file.with_extension("cfg"), config).unwrap();
        Self { dir, file }
    }
    fn request(&self, compat: bool, coverage: bool) -> CheckRequest {
        CheckRequest {
            file: self.file.clone(),
            compat_tlc: compat,
            source_coverage: coverage,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn assert_refused(request: &CheckRequest) {
    let result = check_file(request);
    assert!(
        matches!(&result, Err(ApiError::Unsupported { feature }) if feature.contains(FEATURE)),
        "expected the coverage refusal, got {result:?}"
    );
}

#[test]
fn every_coverage_request_is_refused_by_name_and_plain_runs_keep_their_result() {
    let failing = SOURCE.replace("Inv == x<=2", "Inv == x<=1");
    let enabled = SOURCE.replace("Inv == x<=2", "Inv == x<=2 /\\ ENABLED (x'=x)");
    let unicode = SOURCE.replace("C == 1+2", "C == 1+2\n\tText == \"😀\"");
    for (source, plain_verdict) in [
        (SOURCE, "success"),
        (failing.as_str(), "invariant-violation"),
        (enabled.as_str(), "success"),
        (unicode.as_str(), "success"),
    ] {
        let project = Project::new(source, CONFIG);
        for compat in [false, true] {
            let plain = check_file(&project.request(compat, false))
                .unwrap()
                .to_json();
            assert_eq!(plain["verdict"], plain_verdict, "{plain}");
            assert!(plain.get("source_coverage").is_none());
            assert_refused(&project.request(compat, true));
            let mut stopped = project.request(compat, true);
            stopped.max_states = Some(2);
            assert_refused(&stopped);
        }
    }
    let temporal = Project::new(
        &SOURCE.replace("Unused == x=99", "Live == <> (x=99)"),
        &format!("{CONFIG}PROPERTY Live\n"),
    );
    assert_refused(&temporal.request(false, true));
    // The refusal precedes model loading.
    assert_refused(&CheckRequest {
        file: "missing-coverage-input.tla".into(),
        source_coverage: true,
        temporal: TemporalMode::Ordinary,
        ..Default::default()
    });
    let nested = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../conformance/corpus/core-instance-scopes/specifications/ScopeHolding/ScopeHolding.tla",
    );
    for compat_tlc in [false, true] {
        assert_refused(&CheckRequest {
            file: nested.clone(),
            source_coverage: true,
            compat_tlc,
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        });
    }
}
