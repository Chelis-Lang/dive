//! Pure VIEW normalization keeps concrete recovery and independent witness semantics.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, check_file, check_file_checkpoint, checkpoint};
use std::path::Path;

fn case(
    value: &str,
    name: &str,
    compat_tlc: bool,
    store: Store,
    workers: usize,
    chaos: Option<u64>,
) {
    let directory = std::env::temp_dir().join(format!(
        "dive-view-normalization-{}-{name}-{compat_tlc}-{store:?}-{workers}-{chaos:?}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("private directory");
    let file = directory.join("ViewNormalize.tla");
    std::fs::write(&file, format!("---- MODULE ViewNormalize ----\nEXTENDS Naturals,TLC\nVARIABLES n,x,s\nInit == n = 0 /\\ x = 0 /\\ s = {value}\nNext == n < 3 /\\ n' = n+1 /\\ x' = n /\\ UNCHANGED s\nInv == n < 3\nProjection == n\n====\n")).expect("source");
    let cfg = file.with_extension("cfg");
    std::fs::write(
        &cfg,
        "INIT Init\nNEXT Next\nINVARIANT Inv\nVIEW Projection\nCHECK_DEADLOCK FALSE\n",
    )
    .expect("cfg");
    verify(
        &file,
        &cfg,
        &directory,
        &[compat_tlc],
        &[store],
        &[workers],
        &[chaos],
    );
    std::fs::remove_dir_all(directory).expect("cleanup");
}
fn observables(mut report: serde_json::Value) -> serde_json::Value {
    let object = report.as_object_mut().expect("report object");
    object.remove("elapsed_ms");
    object.remove("checkpoint");
    let options = object
        .get_mut("options")
        .expect("options")
        .as_object_mut()
        .expect("options object");
    options.remove("checkpoint");
    options.remove("workers");
    // Retain is an explicitly reported result-changing counter option.
    // Both exact labels are checked before this named metadata allowance.
    options.remove("profile");
    report
}
#[allow(
    clippy::too_many_lines,
    reason = "complete fixed recovery and independent witness controls"
)]
fn verify(
    file: &Path,
    cfg: &Path,
    directory: &Path,
    profiles: &[bool],
    stores: &[Store],
    widths: &[usize],
    schedules: &[Option<u64>],
) {
    let mut ordinal = 0;
    for &compat_tlc in profiles {
        let spec = SpecModel::load(
            file,
            cfg,
            &TraceOptions {
                compat_tlc,
                run: Some((0, 7)),
                ..TraceOptions::default()
            },
        )
        .expect("independent model");
        for &store in stores {
            for &workers in widths {
                for &chaos in schedules {
                    let request = CheckRequest {
                        file: file.to_path_buf(),
                        config: Some(cfg.to_path_buf()),
                        compat_tlc,
                        store,
                        workers: Some(workers),
                        chaos,
                        seed: Some(7),
                        fp: Some(0),
                        ..CheckRequest::default()
                    };
                    let full = check_file(&request).expect("full run").to_json();
                    assert_eq!(full["verdict"], "invariant-violation", "{full}");
                    assert_eq!(
                        full["options"]["profile"],
                        if compat_tlc { "tlc" } else { "dive" }
                    );
                    assert_eq!(full["statistics"]["generated"], 4);
                    assert_eq!(full["statistics"]["distinct"], 4);
                    assert_eq!(full["statistics"]["depth"], 4);
                    let doc = TraceDoc::from_json(&full).expect("concrete witness");
                    assert_eq!(spec.validate(&doc).validation, Validation::Valid);
                    // x does not occur in VIEW. Equal projected keys must never
                    // turn a changed Init/Next/final concrete state into proof.
                    for at in [0, 1, doc.states.len() - 1] {
                        let mut mutant = doc.clone();
                        mutant.states[at].insert("x".into(), serde_json::json!(99));
                        assert!(
                            matches!(spec.validate(&mutant).validation, Validation::Invalid(_)),
                            "state {at}"
                        );
                    }
                    for level in [0, 1] {
                        let cp = checkpoint::Request {
                            path: directory.join(format!("{ordinal}.cp")),
                            action: checkpoint::Action::PauseAfterLevels(level),
                            counters: checkpoint::Counters::Retain,
                            config_source_sha256: None,
                        };
                        ordinal += 1;
                        let pause = check_file_checkpoint(&request, &cp)
                            .expect("pause")
                            .to_json();
                        assert_eq!(pause["verdict"], "resource-limit");
                        assert_eq!(pause["checkpoint"]["status"], "paused-incomplete");
                        let recovered = check_file_checkpoint(
                            &request,
                            &checkpoint::Request {
                                action: checkpoint::Action::Recover,
                                ..cp.clone()
                            },
                        )
                        .expect("recover")
                        .to_json();
                        assert_eq!(
                            recovered["options"]["profile"],
                            if compat_tlc { "tlc+overrides" } else { "dive" }
                        );
                        assert_eq!(observables(recovered.clone()), observables(full.clone()));
                        assert_eq!(recovered["options"]["workers"]["used"], workers);
                        assert_eq!(
                            recovered["options"]["checkpoint"]["independent_certificate"],
                            false
                        );
                        assert_eq!(
                            spec.validate(
                                &TraceDoc::from_json(&recovered)
                                    .expect("concrete recovered witness")
                            )
                            .validation,
                            Validation::Valid
                        );
                        let reset = check_file_checkpoint(
                            &request,
                            &checkpoint::Request {
                                action: checkpoint::Action::Recover,
                                counters: checkpoint::Counters::Tlc,
                                ..cp
                            },
                        )
                        .expect("reset recovery")
                        .to_json();
                        assert_eq!(reset["verdict"], full["verdict"]);
                        assert_eq!(reset["counterexample"], full["counterexample"]);
                        assert_eq!(
                            reset["options"]["profile"],
                            if compat_tlc { "tlc" } else { "dive" }
                        );
                        assert_eq!(
                            reset["statistics"]["initial_generated"],
                            pause["statistics"]["distinct"]
                        );
                        assert_eq!(
                            reset["statistics"]["generated"]
                                .as_u64()
                                .expect("reset count"),
                            full["statistics"]["generated"]
                                .as_u64()
                                .expect("full count")
                                - pause["statistics"]["generated"]
                                    .as_u64()
                                    .expect("pause count")
                                + pause["statistics"]["distinct"]
                                    .as_u64()
                                    .expect("seen count")
                        );
                        assert_eq!(
                            reset["statistics"]["distinct"],
                            full["statistics"]["distinct"]
                        );
                    }
                }
            }
        }
    }
}
// Each width/schedule slice retains both checkpoints and all independent mutants.
macro_rules! slice {
    ($test:ident, $value:expr, $parent:expr, $compat:expr, $store:expr, $width:expr, $chaos:expr) => {
        #[test]
        fn $test() {
            case($value, $parent, $compat, $store, $width, $chaos);
        }
    };
}
macro_rules! controls {
    ($parent:ident, $value:expr, $compat:expr, $store:expr) => {
        mod $parent {
            use super::*;
            slice!(
                w1_plain,
                $value,
                stringify!($parent),
                $compat,
                $store,
                1,
                None
            );
            slice!(
                w1_chaos,
                $value,
                stringify!($parent),
                $compat,
                $store,
                1,
                Some(19)
            );
            slice!(
                w4_plain,
                $value,
                stringify!($parent),
                $compat,
                $store,
                4,
                None
            );
            slice!(
                w4_chaos,
                $value,
                stringify!($parent),
                $compat,
                $store,
                4,
                Some(19)
            );
            slice!(
                w8_plain,
                $value,
                stringify!($parent),
                $compat,
                $store,
                8,
                None
            );
            slice!(
                w8_chaos,
                $value,
                stringify!($parent),
                $compat,
                $store,
                8,
                Some(19)
            );
        }
    };
}
controls!(set_tlc_fingerprint, "{2,1}", true, Store::Fingerprint);
controls!(set_tlc_exact, "{2,1}", true, Store::Exact);
controls!(set_dive_fingerprint, "{2,1}", false, Store::Fingerprint);
controls!(set_dive_exact, "{2,1}", false, Store::Exact);
controls!(
    tuple_tlc_fingerprint,
    "<<{2,1},3>>",
    true,
    Store::Fingerprint
);
controls!(tuple_tlc_exact, "<<{2,1},3>>", true, Store::Exact);
controls!(
    tuple_dive_fingerprint,
    "<<{2,1},3>>",
    false,
    Store::Fingerprint
);
controls!(tuple_dive_exact, "<<{2,1},3>>", false, Store::Exact);
controls!(
    record_tlc_fingerprint,
    "[a |-> {2,1}]",
    true,
    Store::Fingerprint
);
controls!(record_tlc_exact, "[a |-> {2,1}]", true, Store::Exact);
controls!(
    record_dive_fingerprint,
    "[a |-> {2,1}]",
    false,
    Store::Fingerprint
);
controls!(record_dive_exact, "[a |-> {2,1}]", false, Store::Exact);

#[test]
fn ignored_invalid_value_cannot_produce_a_public_verdict_or_archive() {
    let directory = std::env::temp_dir().join(format!(
        "dive-view-normalization-refusal-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("private directory");
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/view-normalization/reverse-mixed/Compound.tla");
    for store in [Store::Fingerprint, Store::Exact] {
        for workers in [1, 4, 8] {
            for chaos in [None, Some(19)] {
                let request = CheckRequest {
                    file: file.clone(),
                    compat_tlc: true,
                    store,
                    workers: Some(workers),
                    chaos,
                    seed: Some(7),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let full = check_file(&request).expect("explicit refusal").to_json();
                assert_eq!(full["verdict"], "unsupported", "{full}");
                assert!(full["counterexample"].is_null());
                assert_eq!(full["output"], serde_json::json!([]));
                let cp = checkpoint::Request {
                    path: directory.join(format!("{store:?}-{workers}-{chaos:?}.cp")),
                    action: checkpoint::Action::PauseAfterLevels(0),
                    counters: checkpoint::Counters::Tlc,
                    config_source_sha256: None,
                };
                let pause = check_file_checkpoint(&request, &cp)
                    .expect("explicit checkpoint refusal")
                    .to_json();
                assert_eq!(pause["verdict"], "unsupported", "{pause}");
                assert!(pause["counterexample"].is_null());
                assert_eq!(pause["output"], serde_json::json!([]));
                assert!(!cp.path.exists(), "invalid initial value archived");
            }
        }
    }
    std::fs::remove_dir(directory).expect("cleanup");
}
