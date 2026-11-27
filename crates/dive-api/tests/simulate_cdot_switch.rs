//! `TLCGet("-Dtlc2.tool.impl.Tool.cdot")` reads the run's action composition
//! switch in simulation and in its independent validation, as in `dive check`.
#![allow(
    clippy::unwrap_used,
    reason = "fixture/setup corruption must fail these tests"
)]
use dive_api::SimulateRequest;
use std::path::PathBuf;

fn run(compat_tlc: bool) -> serde_json::Value {
    dive_api::simulate_file(&{
        let mut built = SimulateRequest::default();
        built.file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-sim/tests/fixtures/SimCdotSwitch.tla");
        built.seed = Some(0);
        built.depth = Some(4);
        built.traces = 3;
        built.compat_tlc = compat_tlc;
        built
    })
    .unwrap()
    .to_json()
}

#[test]
fn default_profile_composes_actions_and_validates_every_kept_trace() {
    let json = run(false);
    assert_eq!(json["result"], "sampled", "{json}");
    let traces = json["traces"].as_array().unwrap();
    assert_eq!(traces.len(), 3);
    for trace in traces {
        assert_eq!(trace["validation"]["result"], "valid", "{trace}");
    }
}

#[test]
fn tlc_profile_reads_the_unset_property_as_tlc_does() {
    // Without -Dtlc2.tool.impl.Tool.cdot=true, TLC's TLCGet returns the key
    // itself, so the ASSUME is false (2104).
    let json = run(true);
    assert_eq!(json["result"], "evaluation-error", "{json}");
    assert_eq!(json["tlc_code"], 2104, "{json}");
}
