//! Actual native recovery preserves configured keys and independently checked concrete witnesses.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "native checkpoint regression controls"
)]
use dive_api::trace::{Check, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Store, check_file, check_file_checkpoint, checkpoint};
use std::path::Path;

fn request(name: &str) -> CheckRequest {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../conformance/probes/native-checkpoint-keys/specs/{name}/{name}.tla"
    ));
    CheckRequest {
        file: file.clone(),
        config: Some(file.with_extension("cfg")),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
fn directory() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "dive-native-key-api-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).expect("private directory");
    p
}
fn observable(mut value: serde_json::Value) -> serde_json::Value {
    let object = value.as_object_mut().expect("object");
    object.remove("elapsed_ms");
    object.remove("checkpoint");
    let options = object
        .get_mut("options")
        .expect("options")
        .as_object_mut()
        .expect("options object");
    for key in ["checkpoint", "workers", "profile"] {
        options.remove(key);
    }
    value
}

#[allow(
    clippy::too_many_lines,
    reason = "one complete recovery/independent-witness matrix per source case"
)]
fn recovery_case(name: &str, profiles: &[bool]) {
    let directory = directory();
    let mut serial = 0;
    {
        for &compat_tlc in profiles {
            let base = request(name);
            let spec = SpecModel::load(
                &base.file,
                base.config.as_ref().expect("config"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 7)),
                    ..TraceOptions::default()
                },
            )
            .expect("independent model");
            for store in [Store::Fingerprint, Store::Exact] {
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(19)] {
                        let req = CheckRequest {
                            compat_tlc,
                            store,
                            workers: Some(workers),
                            chaos,
                            ..base.clone()
                        };
                        let full = check_file(&req).expect("full").to_json();
                        assert_eq!(full["verdict"], "invariant-violation");
                        for boundary in [0, 1] {
                            let cp = checkpoint::Request {
                                path: directory.join(format!("{serial}.cp")),
                                action: checkpoint::Action::PauseAfterLevels(boundary),
                                counters: checkpoint::Counters::Retain,
                                config_source_sha256: None,
                            };
                            serial += 1;
                            let paused = check_file_checkpoint(&req, &cp).expect("pause").to_json();
                            assert_eq!(paused["verdict"], "resource-limit", "{paused}");
                            assert_eq!(paused["checkpoint"]["status"], "paused-incomplete");
                            assert_eq!(paused["options"]["workers"]["used"], 1);
                            assert!(paused["counterexample"].is_null());
                            let resumed = check_file_checkpoint(
                                &req,
                                &checkpoint::Request {
                                    action: checkpoint::Action::Recover,
                                    ..cp.clone()
                                },
                            )
                            .expect("resume")
                            .to_json();
                            assert_eq!(
                                observable(resumed.clone()),
                                observable(full.clone()),
                                "{name}, profile{compat_tlc}, store{store:?}, width{workers}, chaos{chaos:?}, boundary{boundary}"
                            );
                            assert_eq!(resumed["options"]["workers"]["requested"], workers);
                            assert_eq!(resumed["options"]["workers"]["used"], 1);
                            assert_eq!(
                                resumed["options"]["checkpoint"]["independent_certificate"],
                                false
                            );
                            let doc = TraceDoc::from_json(&resumed).expect("concrete witness");
                            assert_eq!(spec.validate(&doc).validation, Validation::Valid, "{name}");
                            if name == "CpSymConcrete" {
                                for state in &doc.states {
                                    assert_eq!(state["owner"], serde_json::json!({"model":"p2"}));
                                }
                            }
                            if name != "CpSymView" {
                                for at in [0, 1, doc.states.len() - 1] {
                                    let mut bad = doc.clone();
                                    let (variable, value) = if name == "CpView" {
                                        ("n", serde_json::json!(99))
                                    } else {
                                        ("owner", serde_json::json!({"model":"p1"}))
                                    };
                                    bad.states[at].insert(variable.into(), value);
                                    let Validation::Invalid(failure) =
                                        spec.validate(&bad).validation
                                    else {
                                        panic!("equivalent-key witness accepted: {name} state{at}");
                                    };
                                    assert_eq!(
                                        failure.check,
                                        if at == 0 { Check::Init } else { Check::Next }
                                    );
                                }
                            }
                            let reset = check_file_checkpoint(
                                &req,
                                &checkpoint::Request {
                                    action: checkpoint::Action::Recover,
                                    counters: checkpoint::Counters::Tlc,
                                    ..cp
                                },
                            )
                            .expect("TLC reset")
                            .to_json();
                            assert_eq!(reset["verdict"], full["verdict"]);
                            assert_eq!(reset["counterexample"], full["counterexample"]);
                            for key in ["distinct", "depth", "left_on_queue"] {
                                assert_eq!(reset["statistics"][key], full["statistics"][key]);
                            }
                            assert_eq!(
                                reset["statistics"]["generated"]
                                    .as_u64()
                                    .expect("generated"),
                                full["statistics"]["generated"]
                                    .as_u64()
                                    .expect("full generated")
                                    - paused["statistics"]["generated"]
                                        .as_u64()
                                        .expect("paused generated")
                                    + paused["statistics"]["distinct"]
                                        .as_u64()
                                        .expect("paused distinct")
                            );
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn view_recovery_dive_matches_full_and_checks_concrete_witnesses() {
    recovery_case("CpView", &[false]);
}
#[test]
fn view_recovery_tlc_matches_full_and_checks_concrete_witnesses() {
    recovery_case("CpView", &[true]);
}
#[test]
fn symmetry_view_recovery_dive_matches_full_and_checks_concrete_witnesses() {
    recovery_case("CpSymView", &[false]);
}
#[test]
fn symmetry_view_recovery_tlc_matches_full_and_checks_concrete_witnesses() {
    recovery_case("CpSymView", &[true]);
}
#[test]
fn symmetry_recovery_dive_preserves_concrete_owner_and_rejects_equal_key_witnesses() {
    recovery_case("CpSymConcrete", &[false]);
}
#[test]
fn symmetry_recovery_tlc_preserves_concrete_owner_and_rejects_equal_key_witnesses() {
    recovery_case("CpSymConcrete", &[true]);
}
