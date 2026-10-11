fn main() -> Result<(), dive_api::ApiError> {
use dive_api::{CheckRequest, check_file_checkpoint, checkpoint};
let model = CheckRequest {
    file: "conformance/probes/native-checkpoint-runtime-names/specs/RuntimeStrings/RuntimeStrings.tla".into(),
    compat_tlc: true,
    seed: Some(7), fp: Some(0), workers: Some(4),
    frontier_memory_bytes: Some(0), predecessor_memory_bytes: Some(0),
    ..CheckRequest::default()
};
let native = checkpoint::Request {
    path: "strings-rust.cp".into(),
    action: checkpoint::Action::PauseAfterLevels(1),
    counters: checkpoint::Counters::Tlc,
    config_source_sha256: None,
};
let paused = check_file_checkpoint(&model, &native)?.to_json();
assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
assert_eq!(paused["options"]["workers"]["used"], 1);
let recovered = check_file_checkpoint(
    &CheckRequest { workers: Some(8), ..model },
    &checkpoint::Request {
        action: checkpoint::Action::Recover,
        counters: checkpoint::Counters::Retain,
        ..native
    },
)?.to_json();
assert_eq!(recovered["verdict"], "invariant-violation");
assert_eq!(recovered["statistics"]["generated"], 4);
assert_eq!(recovered["options"]["workers"]["requested"], 8);
assert_eq!(recovered["options"]["workers"]["used"], 1);
assert_eq!(recovered["checkpoint"]["transient_storage_cleanup_completed"], true);
assert_eq!(recovered["options"]["checkpoint"]["independent_certificate"], false);
Ok::<(), dive_api::ApiError>(())
}
