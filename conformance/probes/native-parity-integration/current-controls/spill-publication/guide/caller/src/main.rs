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
    path: "cp-spill.cp".into(), action: checkpoint::Action::PauseAfterLevels(1),
    counters: checkpoint::Counters::Tlc, config_source_sha256: None,
};
let paused = check_file_checkpoint(&model, &native)?;
assert_eq!(paused.to_json()["checkpoint"]["status"], "paused-incomplete");
let resumed = check_file_checkpoint(
    &CheckRequest { workers: Some(8), ..model },
    &checkpoint::Request { action: checkpoint::Action::Recover, counters: checkpoint::Counters::Retain, ..native },
)?;
assert_eq!(resumed.to_json()["verdict"], "invariant-violation");
Ok::<(), dive_api::ApiError>(())
}
