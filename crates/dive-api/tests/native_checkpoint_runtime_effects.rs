//! Name admission does not authorize startup output, random draws or checker reads.
#![allow(clippy::expect_used, reason = "pre-effect refusal controls")]
use dive_api::{CheckRequest, checkpoint};
use std::path::PathBuf;

#[test]
fn mixed_effects_refuse_before_work_output_or_archive() {
    let dir = std::env::temp_dir().join(format!(
        "dive-checkpoint-runtime-effects-{}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).expect("private directory");
    for (at, body) in [
        "PrintT(ToString(1))",
        "RandomElement({ToString(1),ToString(2)})",
        "TLCGet(ToString(0))",
        "TLCSet(0,ToString(1))",
    ]
    .iter()
    .enumerate()
    {
        let file: PathBuf = dir.join(format!("Effect{at}.tla"));
        std::fs::write(&file,format!("---- MODULE Effect{at} ----\nEXTENDS Naturals,TLC\nVARIABLE n\nUnused == {body}\nInit == n=0\nNext == n<4 /\\ n'=n+1\n====\n")).expect("source");
        let cfg = file.with_extension("cfg");
        std::fs::write(&cfg, "INIT Init\nNEXT Next\n").expect("cfg");
        for compat_tlc in [false, true] {
            for workers in [1, 4, 8] {
                let path = dir.join(format!("{at}-{compat_tlc}-{workers}.cp"));
                let report = dive_api::check_file_checkpoint_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(cfg.clone()),
                        compat_tlc,
                        workers: Some(workers),
                        chaos: Some(19),
                        seed: Some(7),
                        fp: Some(0),
                        ..CheckRequest::default()
                    },
                    &checkpoint::Request {
                        path: path.clone(),
                        action: checkpoint::Action::PauseAfterLevels(0),
                        counters: checkpoint::Counters::Retain,
                        config_source_sha256: None,
                    },
                    dive_api::Engine::Reference,
                )
                .expect("admission")
                .to_json();
                assert_eq!(report["verdict"], "unsupported", "{report}");
                assert_eq!(report["options"]["workers"]["used"], 0);
                assert!(report["checkpoint"].is_null());
                assert!(report["statistics"].is_null());
                assert_eq!(report["output"], serde_json::json!([]));
                assert!(!path.exists());
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
