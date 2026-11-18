//! TLC normalizes the temporal properties before it evaluates any ASSUME. A
//! normalization with no solutions is TLC's start-up tautology error (2253,
//! exit 77), which Core refuses by name rather than report a false ASSUME
//! that TLC never reaches. A false ASSUME with no such error still reports
//! TLC's assumption failure (2104).
use dive_api::{CheckRequest, ExitConvention};
use std::path::PathBuf;

const REFUSAL: &str = "a temporal validation failure (core): its report is not implemented on Core";

fn spec(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../conformance/corpus/temporal-demand-primary-probes/specifications/{name}/{name}.tla"
    ))
}

fn check(file: PathBuf, compat_tlc: bool, workers: usize) -> dive_api::CheckReport {
    let report = dive_api::check_file(&CheckRequest {
        file,
        compat_tlc,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    });
    match report {
        Ok(report) => report,
        Err(error) => unreachable!("well-formed request: {error}"),
    }
}

#[test]
fn a_startup_tautology_refuses_before_a_false_assume() {
    for name in [
        "TautologyBeforeFalseAssume",
        "StartupRoleSplitBeforeFalseAssume",
    ] {
        for compat_tlc in [false, true] {
            for workers in [1, 4] {
                let report = check(spec(name), compat_tlc, workers);
                let json = report.to_json();
                let at = format!("{name} compat={compat_tlc} workers={workers}: {json}");
                assert_eq!(json["verdict"], "unsupported", "{at}");
                assert_eq!(json["message"], REFUSAL, "{at}");
                assert!(json["tlc"].is_null(), "{at}");
                assert!(json["counterexample"].is_null(), "{at}");
                for convention in [ExitConvention::Native, ExitConvention::Tlc] {
                    assert_eq!(report.exit_code_for(convention), 3, "{at}");
                }
            }
        }
    }
}

#[test]
fn a_false_assume_without_a_startup_tautology_still_reports_2104() {
    let dir =
        std::env::temp_dir().join(format!("dive-false-assume-temporal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(std::fs::create_dir(&dir).is_ok());
    let file = dir.join("FalseAssumeLive.tla");
    assert!(
        std::fs::write(
            &file,
            "---- MODULE FalseAssumeLive ----\nEXTENDS Naturals\nVARIABLE x\nASSUME 1 = 2\n\
             Init == x = 0\nNext == x' = 1 - x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\n\
             Live == []<>(x = 1)\n====\n",
        )
        .is_ok()
    );
    assert!(
        std::fs::write(
            file.with_extension("cfg"),
            "SPECIFICATION Spec\nPROPERTY Live\n"
        )
        .is_ok()
    );
    for compat_tlc in [false, true] {
        for workers in [1, 4] {
            let report = check(file.clone(), compat_tlc, workers);
            let json = report.to_json_with_exit_convention(ExitConvention::Tlc);
            let at = format!("compat={compat_tlc} workers={workers}: {json}");
            assert_eq!(json["verdict"], "assumption-failure", "{at}");
            assert_eq!(json["tlc"]["code"], 2104, "{at}");
            assert_eq!(json["tlc"]["verified"], true, "{at}");
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 10, "{at}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
