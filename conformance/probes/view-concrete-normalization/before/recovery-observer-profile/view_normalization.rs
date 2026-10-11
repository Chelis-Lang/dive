//! Pure VIEW normalization keeps concrete recovery and independent witness semantics.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use std::path::{Path, PathBuf};
use dive_api::{CheckRequest, Store, check_file, check_file_checkpoint, checkpoint};
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};

fn case(value: &str, name: &str) {
    let directory = std::env::temp_dir().join(format!("dive-view-normalization-{}-{name}", std::process::id()));
    std::fs::create_dir(&directory).expect("private directory");
    let file = directory.join("ViewNormalize.tla");
    std::fs::write(&file, format!("---- MODULE ViewNormalize ----\nEXTENDS Naturals,TLC\nVARIABLES n,x,s\nInit == n = 0 /\\ x = 0 /\\ s = {value}\nNext == n < 3 /\\ n' = n+1 /\\ x' = n /\\ UNCHANGED s\nInv == n < 3\nProjection == n\n====\n")).expect("source");
    let cfg = file.with_extension("cfg");
    std::fs::write(&cfg, "INIT Init\nNEXT Next\nINVARIANT Inv\nVIEW Projection\nCHECK_DEADLOCK FALSE\n").expect("cfg");
    verify(&file, &cfg, &directory);
    std::fs::remove_dir_all(directory).expect("cleanup");
}
fn observables(mut report: serde_json::Value) -> serde_json::Value {
    let object = report.as_object_mut().expect("report object");
    object.remove("elapsed_ms");
    object.remove("checkpoint");
    let options = object.get_mut("options").expect("options").as_object_mut().expect("options object");
    options.remove("checkpoint");
    options.remove("workers");
    report
}
#[allow(clippy::too_many_lines, reason = "complete fixed recovery and independent witness controls")]
fn verify(file: &Path, cfg: &Path, directory: &Path) {
    let mut ordinal = 0;
    for compat_tlc in [true, false] {
        let spec = SpecModel::load(file, cfg, &TraceOptions { compat_tlc, run: Some((0,7)), ..TraceOptions::default() }).expect("independent model");
        for store in [Store::Fingerprint, Store::Exact] {
            for workers in [1,4,8] {
                for chaos in [None, Some(19)] {
                    let request = CheckRequest { file: file.to_path_buf(), config: Some(cfg.to_path_buf()), compat_tlc, store, workers: Some(workers), chaos, seed: Some(7), fp: Some(0), ..CheckRequest::default() };
                    let full = check_file(&request).expect("full run").to_json();
                    assert_eq!(full["verdict"], "invariant-violation", "{full}");
                    assert_eq!(full["statistics"]["generated"], 4);
                    assert_eq!(full["statistics"]["distinct"], 4);
                    assert_eq!(full["statistics"]["depth"], 4);
                    let doc = TraceDoc::from_json(&full).expect("concrete witness");
                    assert_eq!(spec.validate(&doc).validation, Validation::Valid);
                    // x does not occur in VIEW. Equal projected keys must never
                    // turn a changed Init/Next/final concrete state into proof.
                    for at in [0,1,doc.states.len()-1] {
                        let mut mutant = doc.clone();
                        mutant.states[at].insert("x".into(), serde_json::json!(99));
                        assert!(matches!(spec.validate(&mutant).validation, Validation::Invalid(_)), "state {at}");
                    }
                    for level in [0,1] {
                        let cp = checkpoint::Request { path: directory.join(format!("{ordinal}.cp")), action: checkpoint::Action::PauseAfterLevels(level), counters: checkpoint::Counters::Retain, config_source_sha256: None };
                        ordinal += 1;
                        let pause = check_file_checkpoint(&request, &cp).expect("pause").to_json();
                        assert_eq!(pause["verdict"], "resource-limit");
                        assert_eq!(pause["checkpoint"]["status"], "paused-incomplete");
                        let recovered = check_file_checkpoint(&request, &checkpoint::Request { action: checkpoint::Action::Recover, ..cp.clone() }).expect("recover").to_json();
                        assert_eq!(observables(recovered.clone()), observables(full.clone()));
                        assert_eq!(recovered["options"]["workers"]["used"], workers);
                        assert_eq!(recovered["options"]["checkpoint"]["independent_certificate"], false);
                        assert_eq!(spec.validate(&TraceDoc::from_json(&recovered).expect("concrete recovered witness")).validation, Validation::Valid);
                        let reset = check_file_checkpoint(&request, &checkpoint::Request { action: checkpoint::Action::Recover, counters: checkpoint::Counters::Tlc, ..cp }).expect("reset recovery").to_json();
                        assert_eq!(reset["verdict"], full["verdict"]);
                        assert_eq!(reset["counterexample"], full["counterexample"]);
                        assert_eq!(reset["statistics"]["distinct"], full["statistics"]["distinct"]);
                    }
                }
            }
        }
    }
}
#[test] fn ignored_set_recovers_concretely() { case("{2,1}", "set"); }
#[test] fn nested_tuple_recovers_concretely() { case("<<{2,1},3>>", "tuple"); }
#[test] fn nested_record_recovers_concretely() { case("[a |-> {2,1}]", "record"); }
