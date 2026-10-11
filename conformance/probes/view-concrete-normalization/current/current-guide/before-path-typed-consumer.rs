fn main() -> Result<(), dive_api::ApiError> {
use dive_api::{CheckRequest, check_file_checkpoint, checkpoint};
let model = CheckRequest {
    file: "conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla".into(),
    config: Some("conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg".into()),
    compat_tlc: true, seed: Some(7), fp: Some(0), workers: Some(4),
    frontier_memory_bytes: Some(0), predecessor_memory_bytes: Some(0),
    ..CheckRequest::default()
};
let native = checkpoint::Request {
    path: "/Users/jeff/code/dive-view-normalization-artifacts/current-guide/cp-spill.cp".into(), action: checkpoint::Action::PauseAfterLevels(1),
    counters: checkpoint::Counters::Tlc, config_source_sha256: None,
};
let paused = check_file_checkpoint(&model, &native)?;
assert_eq!(paused.to_json()["checkpoint"]["status"], "paused-incomplete");
let resumed = check_file_checkpoint(
    &CheckRequest { workers: Some(8), ..model },
    &checkpoint::Request { action: checkpoint::Action::Recover, counters: checkpoint::Counters::Retain, ..native },
)?;
assert_eq!(resumed.to_json()["verdict"], "invariant-violation");

let report = resumed.to_json();
assert_eq!(report["statistics"]["generated"], 5);
assert_eq!(report["statistics"]["distinct"], 3);
assert_eq!(report["statistics"]["depth"], 3);
assert_eq!(report["options"]["workers"]["used"], 8);
assert_eq!(report["options"]["checkpoint"]["independent_certificate"], false);
assert_eq!(report["checkpoint"]["transient_storage_cleanup_completed"], true);
let spec = dive_api::trace::SpecModel::load(
    "conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla",
    "conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg",
    &dive_api::trace::TraceOptions { compat_tlc: true, run: Some((0, 7)), ..Default::default() },
).expect("independent model");
let doc = dive_api::trace::TraceDoc::from_json(&report).expect("concrete trace");
assert_eq!(spec.validate(&doc).validation, dive_api::trace::Validation::Valid);
println!("{}", report);
std::fs::remove_file("/Users/jeff/code/dive-view-normalization-artifacts/current-guide/cp-spill.cp").expect("guide cleanup");

Ok(())
}
