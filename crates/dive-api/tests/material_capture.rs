//! Liveness over actions whose bound values are stored captured material
//! (strings, model values, sets, tuples, records, functions and nested
//! values) agrees with TLC on Core in both profiles and at several widths.
//! The retired reference capture drivers were checked here until #791.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::{CheckRequest, Engine};
use std::path::PathBuf;

fn base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/material-capture")
}

/// TLC's distinct-state count from its recorded final statistics (2199).
fn tlc_distinct(name: &str) -> Option<u64> {
    let text = std::fs::read_to_string(base().join("tlc").join(format!("{name}.txt"))).ok()?;
    let mut lines = text.lines();
    lines.find(|line| line.starts_with("@!@!@STARTMSG 2199:"))?;
    let stats = lines.next()?;
    let (_, rest) = stats.split_once(" states generated, ")?;
    rest.split_once(' ')?.0.parse().ok()
}

#[test]
fn stored_material_values_agree_with_tlc_on_core() {
    let mut checked = 0;
    for selection in ["selection.json", "selection-general.json"] {
        let entries: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(base().join(selection)).expect("selection"),
        )
        .expect("selection JSON");
        for entry in entries.as_array().expect("entries") {
            let name = entry["name"].as_str().expect("name");
            let exit = entry["expected_exit"].as_i64().expect("exit");
            let file = base().join("models").join(name).join(format!("{name}.tla"));
            let distinct = tlc_distinct(name);
            for compat_tlc in [true, false] {
                for workers in [1, 4] {
                    let req = CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc,
                        workers: Some(workers),
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    };
                    let report = dive_api::check_file_engine(&req, Engine::Core).expect("Core");
                    let json = report.to_json();
                    let what = format!("{name} tlc={compat_tlc} w={workers}: {json}");
                    assert_eq!(
                        i64::from(report.exit_code_for(dive_api::ExitConvention::Tlc)),
                        exit,
                        "{what}"
                    );
                    if exit == 0
                        && let Some(distinct) = distinct
                    {
                        assert_eq!(json["statistics"]["distinct"], distinct, "{what}");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 18 * 4);
}
