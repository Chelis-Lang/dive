//! Pinned filtered-WF truth and original textual word, independently validated.
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, ExitConvention, Store, TemporalMode};
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;
const SOURCE: &str = include_str!("fixtures/deferred-filter-scalar/FilterWF.tla");
struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        if let Some(p) = self.0.parent() {
            let _ = std::fs::remove_dir_all(p);
        }
    }
}
#[test]
fn filtered_wf_matches_two_primary_outcomes_and_validates_the_primary_word() -> TestResult {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/deferred-filter-scalar/oracle.json"))?;
    let directory = std::env::temp_dir().join(format!(
        "dive-filtered-wf-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let files = Files(directory.join("FilterWF.tla"));
    std::fs::write(&files.0, SOURCE)?;
    let cases = oracle["cases"].as_array().ok_or("cases")?;
    assert_eq!(cases.len(), 2);
    for case in cases {
        let property = case["property"].as_str().ok_or("property")?;
        let cfg = match property {
            "Hold" => include_str!("fixtures/deferred-filter-scalar/hold.cfg"),
            "Fail" => include_str!("fixtures/deferred-filter-scalar/fail.cfg"),
            _ => return Err("unexpected property".into()),
        };
        std::fs::write(files.0.with_extension("cfg"), cfg)?;
        for compat_tlc in [true, false] {
            for store in [Store::Fingerprint, Store::Exact] {
                let report = dive_api::check_file(&CheckRequest {
                    file: files.0.clone(),
                    compat_tlc,
                    store,
                    temporal: TemporalMode::FinalShort,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                })?;
                let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
                assert_eq!(value["exit_code"], case["physical_exit"], "{value}");
                for field in ["generated", "distinct", "depth"] {
                    assert_eq!(value["statistics"][field], case[field], "{value}");
                }
                assert_eq!(value["execution"]["capture_completed"], true);
                if property == "Hold" {
                    assert_eq!(report.verdict(), "success");
                    assert!(value["counterexample"].is_null());
                } else {
                    assert_eq!(report.verdict(), "liveness-violation");
                    assert_eq!(value["property"], "Fail");
                    let model = SpecModel::load(
                        &files.0,
                        &files.0.with_extension("cfg"),
                        &TraceOptions {
                            compat_tlc,
                            run: Some((0, 0)),
                            ..TraceOptions::default()
                        },
                    )?;
                    let native = TraceDoc::from_json(&value["counterexample"]["trace"])?;
                    assert_eq!(model.read(&native).0.validation, Validation::Valid);
                    let states = oracle["word"]["states"]
                        .as_array()
                        .ok_or("primary states")?
                        .clone();
                    assert_eq!(oracle["word"]["ending"], "stuttering");
                    let mut primary = TraceDoc::behavior(states)?.with_ending(Ending::Stuttering);
                    primary.properties = vec![
                        oracle["word"]["property"]
                            .as_str()
                            .ok_or("primary property")?
                            .into(),
                    ];
                    assert_eq!(model.read(&primary).0.validation, Validation::Valid);
                    primary.states[1].insert("x".into(), false.into());
                    assert!(matches!(
                        model.read(&primary).0.validation,
                        Validation::Invalid(_)
                    ));
                }
            }
        }
    }
    Ok(())
}
