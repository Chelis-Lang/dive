//! Fresh serialized proof of primitive Next errors before temporal row completion.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};
use serde_json::json;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-liveness/GenError/GenError.tla")
}
#[test]
fn primitive_temporal_next_words_require_fresh_execution_of_both_sources() -> TestResult {
    let file = fixture();
    for case in ["General", "Nested", "Record", "Tuple"] {
        let config = file.with_file_name(format!("{case}.cfg"));
        for compat_tlc in [false, true] {
            let model = SpecModel::load(
                &file,
                &config,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            {
                let report = check_file(&CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    compat_tlc,
                    seed: Some(0),
                    fp: Some(0),
                    workers: Some(1),
                    ..CheckRequest::default()
                })?
                .to_json();
                assert_eq!(report["tlc"]["code"], 2179);
                assert_eq!(report["tlc"]["exit_status"], 255);
                assert_eq!(report["statistics"]["distinct"], 2);
                assert_eq!(report["statistics"]["generated"], 2);
                let wire = &report["counterexample"]["trace"];
                assert_eq!(wire["version"], 1);
                let doc = TraceDoc::from_json(wire)?;
                let proof = model.try_validate(&doc)?;
                assert_eq!(
                    proof.validation,
                    Validation::Valid,
                    "{case}/{compat_tlc}: {proof:?}"
                );
                for (pointer, value) in [
                    ("/states/0/values/n", json!(3)),
                    ("/states/1/values/n", json!(3)),
                    ("/ending/ec", json!("GENERAL")),
                    ("/ending/endpoint/reported", json!(1000)),
                    ("/ending/endpoint/deciding", json!(2103)),
                    ("/ending/endpoint/source/module", json!("Wrong")),
                ] {
                    let mut bad = wire.clone();
                    *bad.pointer_mut(pointer).ok_or("word field")? = value;
                    assert_ne!(
                        model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
                        Validation::Valid,
                        "{bad}"
                    );
                }
                if case == "Record" && compat_tlc {
                    controlled_reads(&model, &config, &doc)?;
                }
                let mut bad = wire.clone();
                bad["properties"] = json!(["P"]);
                assert_ne!(
                    model.try_validate(&TraceDoc::from_json(&bad)?)?.validation,
                    Validation::Valid
                );
            }
        }
    }
    Ok(())
}
#[test]
fn temporal_row_shape_and_exception_lifetime_are_not_assumed() -> TestResult {
    let file = fixture();
    let config = file.with_file_name("Record.cfg");
    let original = std::fs::read_to_string(&file)?;
    let config_text = std::fs::read_to_string(&config)?;
    for compat_tlc in [false, true] {
        let report = check_file(&CheckRequest {
            file: file.clone(),
            config: Some(config.clone()),
            compat_tlc,
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            ..CheckRequest::default()
        })?
        .to_json();
        let doc = TraceDoc::from_json(&report["counterexample"]["trace"])?;
        for source in [
            original.replace("P == []<>(n = 0)", "P == []<>(n = 5)"),
            original.replace("P == []<>(n = 0)", "P == []<>(1 \\div n = 0)"),
            original.replace("Init == n = 0", "Init == n \\in 0..1"),
            original.replace("n' = n + 1", "(n' = n + 1 \\/ n' = n + 2)"),
            original.replace("ELSE 1 \\div 0", "ELSE CHOOSE v \\in {} : TRUE"),
        ] {
            assert_ne!(source, original);
            let model = SpecModel::from_source(
                "GenError.tla",
                &source,
                &config_text,
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            let validation = model.try_validate(&doc)?.validation;
            assert!(
                matches!(validation, Validation::Unsupported(_)),
                "{validation:?}\n{source}"
            );
        }
    }
    Ok(())
}

#[derive(Debug)]
struct StopAfter {
    polls: std::sync::atomic::AtomicUsize,
    limit: usize,
}
impl dive_eval::HostControl for StopAfter {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        if self.polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= self.limit {
            Err(dive_eval::Interrupted::new(737))
        } else {
            Ok(())
        }
    }
}
fn controlled_reads(model: &SpecModel, config: &std::path::Path, doc: &TraceDoc) -> TestResult {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let config = dive_config::parse(&std::fs::read_to_string(config)?)
        .into_config()
        .ok_or("config")?;
    let session = dive_trace::Session::new(
        dive_model::Recipe::new(
            model.model().ir.clone(),
            config,
            dive_eval::Options::tlc(),
            false,
        ),
        None,
    );
    let complete = Arc::new(StopAfter {
        polls: AtomicUsize::new(0),
        limit: usize::MAX,
    });
    assert_eq!(
        session.read_controlled(doc, complete.clone())?.0.validation,
        Validation::Valid
    );
    let polls = complete.polls.load(Ordering::SeqCst);
    assert!(polls > 2);
    for limit in [0, polls / 2, polls - 1] {
        assert!(matches!(
            session.read_controlled(
                doc,
                Arc::new(StopAfter {
                    polls: AtomicUsize::new(0),
                    limit,
                })
            ),
            Err(dive_trace::ValidationRunError::Interrupted { .. })
        ));
    }
    Ok(())
}
