//! A simulation report retains each trace as its serialized entry, not as a
//! JSON tree, so its memory grows by about the output per trace (#1024).
#![allow(
    clippy::unwrap_used,
    reason = "fixture/setup corruption must fail these tests"
)]
use dive_api::{ExitConvention, SimulateRequest};
use std::path::PathBuf;

fn report(traces: u64) -> dive_api::SimulateReport {
    dive_api::simulate_file(&{
        let mut built = SimulateRequest::default();
        built.file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-sim/tests/fixtures/SimChoice.tla");
        built.seed = Some(7);
        built.depth = Some(5);
        built.traces = traces;
        built.keep_traces = Some(dive_api::KEEP_ALL_TRACES);
        built
    })
    .unwrap()
}

fn written(report: &dive_api::SimulateReport, convention: Option<ExitConvention>) -> String {
    let mut text = Vec::new();
    report.write_json(&mut text, convention).unwrap();
    String::from_utf8(text).unwrap()
}

#[test]
fn streamed_report_matches_the_json_tree() {
    let report = report(12);
    let json = report.to_json();
    assert_eq!(json["result"], "sampled", "{json}");
    assert_eq!(json["traces"].as_array().unwrap().len(), 12);
    assert_eq!(written(&report, None), json.to_string());
    for convention in [ExitConvention::Native, ExitConvention::Tlc] {
        assert_eq!(
            written(&report, Some(convention)),
            report.to_json_with_exit_convention(convention).to_string()
        );
    }
}

#[test]
fn retained_trace_bytes_are_the_serialized_entries_and_grow_linearly() {
    let small = report(10);
    let large = report(40);
    for report in [&small, &large] {
        let entries: usize = report.to_json()["traces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry.to_string().len())
            .sum();
        // The report keeps exactly the text it writes for its traces.
        assert_eq!(report.retained_trace_bytes(), entries);
        assert!(report.retained_trace_bytes() < written(report, None).len());
    }
    // Four times the traces retain about four times the bytes: per-trace
    // retention is a constant, not a function of the run's length.
    let (a, b) = (small.retained_trace_bytes(), large.retained_trace_bytes());
    assert!(b <= 5 * a && b >= 3 * a, "{a} -> {b}");
}
