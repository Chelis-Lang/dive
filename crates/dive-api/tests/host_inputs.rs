//! Actual file capture and fresh explicit input assumptions, never ambient replay.
#![allow(
    clippy::expect_used,
    reason = "finite fixture construction and assertions"
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dive_api::host_inputs::{
    InputRecord, InputRequest, InputResponse, RecordedInputs, inputs_from_report,
};
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, HostAccess, check_file_engine};
use serde_json::json;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "dive-host-inputs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("fixture directory");
        Self(path)
    }
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn source(&self, body: &str) -> PathBuf {
        let path = self.file("Host.tla");
        std::fs::write(
            &path,
            format!(
                "---- MODULE Host ----\nEXTENDS Naturals, TLC, IOUtils\nVARIABLE x\n{body}\n====\n"
            ),
        )
        .expect("source");
        std::fs::write(
            self.file("Host.cfg"),
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        )
        .expect("configuration");
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn modules() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-eval/tests/fixtures/community-modules")
}
fn options(tlc: bool, inputs: RecordedInputs) -> TraceOptions {
    TraceOptions {
        compat_tlc: tlc,
        inputs: Some(inputs),
        run: Some((7, 0)),
        library_paths: vec![modules()],
        ..Default::default()
    }
}

#[test]
fn actual_file_capture_replays_after_the_file_is_removed_and_rejects_forgery() {
    for tlc in [false, true] {
        for engine in [Engine::Reference, Engine::Auto] {
            let fixture = Fixture::new();
            let input = fixture.file("input.txt");
            std::fs::write(&input, "hello\n").expect("input");
            let file = fixture.source(&format!("Payload==Deserialize({},[format|->\"TXT\",charset|->\"UTF-8\"])\nInit==x=2 /\\ Payload.stdout=\"hello\\n\" /\\ Payload.exitValue=0 /\\ Payload.stderr=\"\"\nNext==x'=x+1\nInv==x<4", json!(input.display().to_string())));
            let request = CheckRequest {
                file: file.clone(),
                compat_tlc: tlc,
                host_access: HostAccess::parse("files").expect("grant"),
                seed: Some(7),
                fp: Some(0),
                workers: Some(1),
                library_paths: vec![modules()],
                ..Default::default()
            };
            let report = check_file_engine(&request, engine)
                .expect("check")
                .to_json();
            assert_eq!(
                report["verdict"], "invariant-violation",
                "{}",
                report["message"]
            );
            assert_eq!(report["statistics"]["generated"], 3);
            assert_eq!(report["property"], "Inv");
            let inputs = inputs_from_report(&report).expect("captured inputs");
            assert_eq!(inputs.records.len(), 1);
            assert_eq!(
                inputs.records[0].request,
                InputRequest::TextFile(input.display().to_string())
            );
            std::fs::remove_file(&input).expect("remove live source input");
            let model = SpecModel::load(
                &file,
                &fixture.file("Host.cfg"),
                &options(tlc, inputs.clone()),
            )
            .expect("offline source binding");
            let word = TraceDoc::from_json(&report).expect("reported word");
            assert_eq!(model.validate(&word).validation, Validation::Valid);
            assert!(matches!(
                dive_trace::validate_doc(model.model(), &word, &[])
                    .0
                    .validation,
                Validation::Unsupported(_)
            ));
            let mut forged = word.clone();
            forged.states[0]["x"] = json!(0);
            assert!(matches!(
                model.validate(&forged).validation,
                Validation::Invalid(_)
            ));
            for invalid in 0..3 {
                let mut changed = inputs.clone();
                match invalid {
                    0 => changed.records.clear(),
                    1 => changed.records[0].request = InputRequest::TextFile("another.txt".into()),
                    _ => changed.records.push(changed.records[0].clone()),
                }
                assert!(matches!(
                    SpecModel::load(&file, &fixture.file("Host.cfg"), &options(tlc, changed)),
                    Err(dive_api::ApiError::Unsupported { .. })
                ));
            }
            let mut changed = inputs;
            changed.records[0].response = InputResponse::Text("different\n".into());
            let changed = SpecModel::load(&file, &fixture.file("Host.cfg"), &options(tlc, changed))
                .expect("changed explicit assumption");
            assert!(matches!(
                changed.validate(&word).validation,
                Validation::Invalid(_)
            ));
        }
    }
}

#[test]
fn recorded_environment_names_are_created_by_fresh_source_binding() {
    let source = "---- MODULE Host ----\nEXTENDS IOUtils\nVARIABLE x\nEnvironment==IOEnv\nInit==x=Environment[\"DIVE_LABEL\"]\nNext==UNCHANGED x\nInv==x=\"forbidden\"\n====\n";
    for tlc in [false, true] {
        let inputs = RecordedInputs {
            grants: HostAccess::parse("env").expect("grant"),
            records: vec![InputRecord {
                request: InputRequest::Environment,
                response: InputResponse::Environment(
                    [("DIVE_LABEL".into(), "created-outside-source".into())].into(),
                ),
            }],
        };
        let model = SpecModel::from_source(
            "Host.tla",
            source,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &options(tlc, inputs),
        )
        .expect("fresh environment binding");
        let word = TraceDoc::behavior(vec![json!({"x":"created-outside-source"})])
            .expect("word")
            .with_ending(Ending::Invariant("Inv".into()));
        assert_eq!(model.validate(&word).validation, Validation::Valid);
        let forged = TraceDoc::behavior(vec![json!({"x":"forbidden"})])
            .expect("word")
            .with_ending(Ending::Invariant("Inv".into()));
        assert!(matches!(
            model.validate(&forged).validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
fn reached_runtime_reads_and_unsupported_backends_do_not_gain_input_authority() {
    for tlc in [false, true] {
        let fixture = Fixture::new();
        let file = fixture.source("Init==x=0\nNext==x'=Deserialize(ToString(x),[format|->\"TXT\",charset|->\"UTF-8\"]).exitValue\nInv==TRUE");
        let request = CheckRequest {
            file,
            compat_tlc: tlc,
            host_access: HostAccess::parse("files").expect("grant"),
            seed: Some(7),
            workers: Some(1),
            library_paths: vec![modules()],
            ..Default::default()
        };
        let report = check_file_engine(&request, Engine::Reference)
            .expect("check")
            .to_json();
        assert_eq!(report["verdict"], "unsupported");
        assert!(
            report["message"]
                .as_str()
                .expect("refusal")
                .contains("runtime host reads")
        );
        assert!(
            inputs_from_report(&report)
                .expect("empty captured context")
                .records
                .is_empty()
        );
        assert!(matches!(
            check_file_engine(&request, Engine::Core),
            Err(dive_api::ApiError::Unsupported { .. })
        ));
    }
}

#[test]
fn file_limits_missing_inputs_and_exec_grants_keep_named_refusals() {
    for tlc in [false, true] {
        for case in ["large", "utf8", "missing", "exec"] {
            let fixture = Fixture::new();
            let input = fixture.file("input.txt");
            match case {
                "large" => {
                    std::fs::write(&input, vec![b'x'; dive_eval::inputs::MAX_INPUT_BYTES + 1])
                        .expect("bounded oversize control")
                }
                "utf8" => std::fs::write(&input, [0xff]).expect("invalid UTF-8"),
                _ => {}
            }
            let body = if case == "exec" {
                format!(
                    "Init==x=IOExec(<<\"touch\",{}>>).exitValue\nNext==UNCHANGED x\nInv==TRUE",
                    json!(fixture.file("must-not-exist").display().to_string())
                )
            } else {
                format!(
                    "Payload==Deserialize({},[format|->\"TXT\",charset|->\"UTF-8\"])\nInit==x=Payload.exitValue\nNext==UNCHANGED x\nInv==TRUE",
                    json!(input.display().to_string())
                )
            };
            let file = fixture.source(&body);
            let request = CheckRequest {
                file,
                compat_tlc: tlc,
                host_access: HostAccess::parse(if case == "exec" { "exec" } else { "files" })
                    .expect("grant"),
                seed: Some(7),
                workers: Some(1),
                library_paths: vec![modules()],
                ..Default::default()
            };
            let report = check_file_engine(&request, Engine::Reference)
                .expect("refused check")
                .to_json();
            assert_eq!(
                report["verdict"], "unsupported",
                "{case}: {}",
                report["message"]
            );
            let expected = match case {
                "large" => "byte limit",
                "utf8" => "non-UTF-8",
                "missing" => "file-open",
                _ => "IOExec",
            };
            assert!(
                report["message"]
                    .as_str()
                    .expect("refusal")
                    .contains(expected),
                "{case}: {}",
                report["message"]
            );
            assert!(
                inputs_from_report(&report)
                    .expect("captured prefix")
                    .records
                    .is_empty()
            );
            assert!(!fixture.file("must-not-exist").exists());
        }
    }
}

#[test]
fn external_inputs_refuse_recovery_before_loading_an_archive_or_specification() {
    let fixture = Fixture::new();
    let request = CheckRequest {
        file: fixture.file("absent.tla"),
        host_access: HostAccess::parse("files").expect("grant"),
        ..Default::default()
    };
    let checkpoint = dive_api::checkpoint::Request {
        path: fixture.file("absent.archive"),
        action: dive_api::checkpoint::Action::Recover,
        counters: dive_api::checkpoint::Counters::Tlc,
        config_source_sha256: None,
    };
    assert!(
        matches!(dive_api::check_file_checkpoint_engine(&request,&checkpoint,Engine::Reference),Err(dive_api::ApiError::Unsupported { feature }) if feature.contains("checkpoint/recovery"))
    );
    assert!(!checkpoint.path.exists());
}

#[test]
fn an_unused_actual_startup_denial_cannot_be_swallowed_by_binding() {
    for tlc in [false, true] {
        let source = "---- MODULE Host ----\nEXTENDS IOUtils\nVARIABLE x\nUnused==IOEnv\nInit==x=0\nNext==UNCHANGED x\nInv==TRUE\n====\n";
        let options = TraceOptions {
            compat_tlc: tlc,
            library_paths: vec![modules()],
            ..Default::default()
        };
        assert!(
            matches!(SpecModel::from_source("Host.tla",source,"INIT Init\nNEXT Next\nINVARIANT Inv\n",&options),Err(dive_api::ApiError::Unsupported {feature}) if feature.contains("--allow-io=env"))
        );
    }
}
