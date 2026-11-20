//! Actual file capture and fresh explicit input assumptions, never ambient replay.
#![allow(
    clippy::expect_used,
    reason = "finite fixture construction and assertions"
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dive_api::host_inputs::{InputRecord, InputRequest, InputResponse, RecordedInputs};
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, HostAccess, check_file};
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
/// Core's named refusal of granted external inputs; only the retired
/// reference engine captured them (#791, #910).
fn assert_io_refused(request: &CheckRequest) {
    let result = check_file(request);
    assert!(
        matches!(&result, Err(dive_api::ApiError::Unsupported { feature })
            if feature.contains("external inputs (--allow-io, IOUtils/IOEnv) are not implemented on Core")),
        "expected the external-input refusal, got {result:?}"
    );
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
        {
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
            assert_io_refused(&request);
            // The independent validator still binds a recorded capture of
            // this file offline and rejects forged words and records.
            let inputs = RecordedInputs {
                grants: HostAccess::parse("files").expect("grant"),
                records: vec![InputRecord {
                    request: InputRequest::TextFile(input.display().to_string()),
                    response: InputResponse::Text("hello\n".into()),
                }],
            };
            std::fs::remove_file(&input).expect("remove live source input");
            let model = SpecModel::load(
                &file,
                &fixture.file("Host.cfg"),
                &options(tlc, inputs.clone()),
            )
            .expect("offline source binding");
            let word = TraceDoc::behavior(vec![json!({"x":2}), json!({"x":3}), json!({"x":4})])
                .expect("word")
                .with_ending(Ending::Invariant("Inv".into()));
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
fn each_finite_choice_attempt_replays_its_own_inputs_and_preserves_correlation() {
    let source = "---- MODULE Host ----\nEXTENDS Naturals, TLC, IOUtils\nVARIABLE x\nPayload==Deserialize(\"no-ambient-file.txt\",[format|->\"TXT\",charset|->\"UTF-8\"]).stdout\nInit == x = LET r == RandomElement({0,1}) IN <<Payload,r,r>>\nNext==UNCHANGED x\nInv==x[2]<0\n====\n";
    for tlc in [false, true] {
        let inputs = RecordedInputs {
            grants: HostAccess::parse("files").expect("grant"),
            records: vec![InputRecord {
                request: InputRequest::TextFile("no-ambient-file.txt".into()),
                response: InputResponse::Text("external-name-not-in-source".into()),
            }],
        };
        let model = SpecModel::from_source(
            "Host.tla",
            source,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &options(tlc, inputs),
        )
        .expect("explicit inputs and seeded startup context");
        for _ in 0..2 {
            for choice in [0, 1] {
                let word = TraceDoc::behavior(vec![
                    json!({"x":{"seq":["external-name-not-in-source",choice,choice]}}),
                ])
                .expect("word")
                .with_ending(Ending::Invariant("Inv".into()));
                assert_eq!(model.validate(&word).validation, Validation::Valid);
            }
            for forged in [
                json!(["external-name-not-in-source", 0, 1]),
                json!(["external-name-not-in-source", 2, 2]),
            ] {
                let word = TraceDoc::behavior(vec![json!({"x":{"seq":forged}})])
                    .expect("word")
                    .with_ending(Ending::Invariant("Inv".into()));
                let result = model.validate(&word).validation;
                assert!(matches!(result, Validation::Invalid(_)), "{result:?}");
            }
        }
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
        assert_io_refused(&request);
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
                        .expect("bounded oversize control");
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
            assert_io_refused(&request);
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
        matches!(dive_api::check_file_checkpoint(&request,&checkpoint),Err(dive_api::ApiError::Unsupported { feature }) if feature.contains("checkpoint/recovery"))
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
        let model = SpecModel::from_source(
            "Host.tla",
            source,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &options,
        )
        .expect("low-level soft precomputation remains available");
        let word = TraceDoc::behavior(vec![json!({"x":0})]).expect("word");
        let (report, transport) = model.read(&word);
        if tlc {
            // TLC's evaluation would have interned the environment's strings.
            assert!(
                matches!(report.validation,Validation::Unsupported(feature) if feature.contains("--allow-io=env"))
            );
            assert!(transport.is_none());
        } else {
            // Nothing checked reaches `Unused`, and reading the environment
            // leaves nothing a later evaluation observes (issue #852).
            assert!(
                matches!(report.validation, Validation::Valid),
                "{:?}",
                report.validation
            );
        }
    }
}

#[test]
fn recorded_inputs_do_not_authorize_unproved_next_register_prefixes() {
    for tlc in [false, true] {
        let fixture = Fixture::new();
        let input = fixture.file("input.txt");
        std::fs::write(&input, "hello\n").expect("input");
        let file=fixture.source(&format!("Payload==Deserialize({},[format|->\"TXT\",charset|->\"UTF-8\"])\nInit==x=0 /\\ Payload.stdout=\"hello\\n\"\nNext==x'=x+1 /\\ TLCSet(1,x')\nInv==x<1",json!(input.display().to_string())));
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
        assert_io_refused(&request);
        let inputs = RecordedInputs {
            grants: HostAccess::parse("files").expect("grant"),
            records: vec![InputRecord {
                request: InputRequest::TextFile(input.display().to_string()),
                response: InputResponse::Text("hello\n".into()),
            }],
        };
        let model = SpecModel::load(&file, &fixture.file("Host.cfg"), &options(tlc, inputs))
            .expect("source replay");
        let word = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])
            .expect("word")
            .with_ending(Ending::Invariant("Inv".into()));
        assert!(
            matches!(model.validate(&word).validation,Validation::Unsupported(feature) if feature.contains("register"))
        );
    }
}

#[test]
fn scalar_override_closes_instance_input_but_leaves_live_environment_reads_refused() {
    let fixture = Fixture::new();
    let source = fixture.file("Host.tla");
    let cfg = fixture.file("Host.cfg");
    std::fs::write(
        fixture.file("Machine.tla"),
        r"---- MODULE Machine ----
CONSTANT Mode
VARIABLE x
Init == x = Mode
Next == x' = x
Hold == []<>(x = Mode)
Absent == <>(x # Mode)
====",
    )
    .expect("instance");
    std::fs::write(
        &source,
        r#"---- MODULE Host ----
EXTENDS IOUtils
VARIABLE x
Input == IF "GRAPH" \in DOMAIN IOEnv THEN IOEnv.GRAPH ELSE "missing"
INSTANCE Machine WITH Mode <- Input
Other == IF "ANOTHER" \in DOMAIN IOEnv THEN IOEnv.ANOTHER ELSE "empty"
MatchesOther == x = Other
===="#,
    )
    .expect("source");
    for (property, binding, expected) in [
        ("Hold", "CONSTANT Input = \"fixed\"", "success"),
        ("Absent", "CONSTANT Input = \"fixed\"", "liveness-violation"),
        ("Hold", "", "unsupported"),
        (
            "Hold",
            "CONSTANT Input = \"fixed\"\nINVARIANT MatchesOther",
            "unsupported",
        ),
    ] {
        std::fs::write(
            &cfg,
            format!("INIT Init\nNEXT Next\nPROPERTY {property}\n{binding}\n"),
        )
        .expect("cfg");
        let request = CheckRequest {
            file: source.clone(),
            config: Some(cfg.clone()),
            library_paths: vec![modules()],
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        };
        let report = check_file(&request).expect("check").to_json();
        assert_eq!(report["verdict"], expected, "{report}");
        if expected == "unsupported" {
            assert!(
                report["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("IOEnv"))
            );
        } else if expected == "liveness-violation" {
            let model = SpecModel::load(
                &source,
                &cfg,
                &TraceOptions {
                    library_paths: vec![modules()],
                    run: Some((0, 0)),
                    ..Default::default()
                },
            )
            .expect("independent model");
            let trace =
                TraceDoc::parse(&report["counterexample"]["trace"].to_string()).expect("trace");
            assert_eq!(model.validate(&trace).validation, Validation::Valid);
        }
    }
}
