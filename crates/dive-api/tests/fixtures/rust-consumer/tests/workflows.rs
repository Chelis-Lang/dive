//! Ordinary Rust consumers use public APIs and do not invoke Java or the CLI.
#![forbid(unsafe_code)]
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{
    CheckRequest, DfidRequest, Engine, RewriteRequest, SimulateRequest, TranslationRequest,
    check_file_checkpoint_engine, check_file_engine, checkpoint, dfid_file_engine,
    simulate_file_engine, translate_source, write_translation_file,
};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

type Result = std::result::Result<(), Box<dyn Error>>;
const FINITE: &str = "---- MODULE Finite ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE 0\nInv == x<=2\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Project {
    directory: PathBuf,
    file: PathBuf,
}
impl Project {
    fn new(name: &str, source: &str, config: &str) -> std::io::Result<Self> {
        let directory = std::env::temp_dir().join(format!(
            "dive-rust-consumer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory)?;
        let file = directory.join(format!("{name}.tla"));
        fs::write(&file, source)?;
        fs::write(file.with_extension("cfg"), config)?;
        Ok(Self { directory, file })
    }
    fn request(&self, compat_tlc: bool) -> CheckRequest {
        CheckRequest {
            file: self.file.clone(),
            compat_tlc,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        }
    }
    fn model(&self, compat_tlc: bool) -> std::result::Result<SpecModel, dive_api::ApiError> {
        SpecModel::load(
            &self.file,
            &self.file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((7, 0)),
                ..TraceOptions::default()
            },
        )
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn checking_reports_real_success_and_independently_validated_violations() -> Result {
    let safe = Project::new("Finite", FINITE, CONFIG)?;
    let bad = Project::new("Finite", FINITE, &CONFIG.replace("Inv\n", "Bad\n"))?;
    fs::write(
        &bad.file,
        FINITE.replace("Inv == x<=2", "Inv == x<=2\nBad == x<2"),
    )?;
    for compat in [false, true] {
        for engine in [Engine::Reference, Engine::Core] {
            let report = check_file_engine(&safe.request(compat), engine)?;
            assert_eq!(report.verdict(), "success", "{}", report.to_json());
            assert_eq!(report.exit_code(), 0);
            assert!(report.result().is_some());
            assert_eq!(report.to_json()["statistics"]["distinct"], 3);
            let report = check_file_engine(&bad.request(compat), engine)?;
            assert_eq!(
                report.verdict(),
                "invariant-violation",
                "{}",
                report.to_json()
            );
            assert_ne!(report.exit_code(), 0);
            let mut word = TraceDoc::from_json(&report.to_json())?;
            let model = bad.model(compat)?;
            assert_eq!(model.validate(&word).validation, Validation::Valid);
            word.states[1].insert("x".into(), json!(99));
            assert!(matches!(
                model.validate(&word).validation,
                Validation::Invalid(_)
            ));
        }
    }
    Ok(())
}

fn words(report: &Value) -> Vec<Value> {
    report["traces"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| t["document"]["states"].clone())
        .collect()
}
#[test]
fn seeded_generation_is_reproducible_and_samples_remain_non_exhaustive() -> Result {
    let project = Project::new(
        "Choices",
        "---- MODULE Choices ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x' \\in 0..9\nInv == x \\in 0..9\n====\n",
        CONFIG,
    )?;
    for compat in [false, true] {
        for engine in [Engine::Reference, Engine::Core] {
            let request = SimulateRequest {
                file: project.file.clone(),
                compat_tlc: compat,
                generation: true,
                traces: 2,
                depth: Some(4),
                seed: Some(7),
                ..SimulateRequest::default()
            };
            let report = simulate_file_engine(&request, engine)?.to_json();
            assert_eq!(report["result"], "sampled", "{report}");
            assert_eq!(report["model_proved"], false);
            assert_eq!(report["exhaustive"], false);
            let traces = report["traces"].as_array().ok_or("missing samples")?;
            assert_eq!(traces.len(), 2);
            let model = project.model(compat)?;
            for trace in traces {
                let word = TraceDoc::from_json(&trace["document"])?;
                assert_eq!(
                    model.try_validate_sample(&word)?.validation,
                    Validation::Valid
                );
            }
            assert_eq!(
                words(&report),
                words(&simulate_file_engine(&request, engine)?.to_json())
            );
        }
    }
    Ok(())
}

#[test]
fn dfid_completion_requires_evidence_even_when_the_compatibility_exit_is_zero() -> Result {
    let finite = Project::new("Finite", FINITE, CONFIG)?;
    let unbounded = Project::new(
        "Unbounded",
        "---- MODULE Unbounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1\n====\n",
        "INIT Init\nNEXT Next\n",
    )?;
    for compat in [false, true] {
        for engine in [Engine::Reference, Engine::Core] {
            let request = DfidRequest {
                file: finite.file.clone(),
                compat_tlc: compat,
                max_level: 8,
                seed: Some(7),
                ..DfidRequest::default()
            };
            let report = dfid_file_engine(&request, engine)?.to_json();
            assert_eq!(report["result"], "complete", "{report}");
            assert_eq!(report["model_proved"], true);
            let bounded = dfid_file_engine(
                &DfidRequest {
                    file: unbounded.file.clone(),
                    max_level: 3,
                    ..request
                },
                engine,
            )?;
            let report = bounded.to_json();
            assert_eq!(report["result"], "incomplete", "{report}");
            assert_eq!(report["model_proved"], false);
            assert_eq!(report["termination"], "depth-bound");
            if compat {
                assert_eq!(bounded.exit_status(), 0);
            }
        }
    }
    Ok(())
}

#[test]
fn implementation_traces_accept_real_steps_and_reject_skipped_states() -> Result {
    let model = SpecModel::from_source("Finite.tla", FINITE, CONFIG, &TraceOptions::default())?;
    let actual = TraceDoc::behavior(vec![
        json!({"x":0}),
        json!({"x":1}),
        json!({"x":2}),
        json!({"x":0}),
    ])?;
    assert_eq!(model.validate(&actual).validation, Validation::Valid);
    let skipped = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})])?;
    assert!(matches!(
        model.validate(&skipped).validation,
        Validation::Invalid(_)
    ));
    Ok(())
}

#[test]
fn native_checkpoint_recovery_resumes_a_saved_frontier_with_truthful_counts() -> Result {
    let project = Project::new("Finite", FINITE, CONFIG)?;
    for (index, engine) in [Engine::Reference, Engine::Core].into_iter().enumerate() {
        let request = project.request(false);
        let full = check_file_engine(&request, engine)?.to_json();
        let pause = checkpoint::Request {
            path: project.directory.join(format!("{index}.checkpoint")),
            action: checkpoint::Action::PauseAfterLevels(1),
            counters: checkpoint::Counters::Retain,
            config_source_sha256: None,
        };
        let stopped = check_file_checkpoint_engine(&request, &pause, engine)?.to_json();
        assert_eq!(
            stopped["checkpoint"]["status"], "paused-incomplete",
            "{stopped}"
        );
        assert_ne!(stopped["verdict"], "success");
        assert!(fs::metadata(&pause.path)?.len() > 0);
        let recovered = check_file_checkpoint_engine(
            &request,
            &checkpoint::Request {
                action: checkpoint::Action::Recover,
                ..pause
            },
            engine,
        )?
        .to_json();
        assert_eq!(recovered["verdict"], "success", "{recovered}");
        assert_eq!(recovered["checkpoint"]["status"], "recovered");
        for counter in ["distinct", "generated", "depth"] {
            assert_eq!(
                recovered["statistics"][counter],
                full["statistics"][counter]
            );
        }
    }
    Ok(())
}

#[test]
fn translated_algorithms_are_admitted_then_checked_as_a_separate_model() -> Result {
    let source = "---- MODULE Algorithm ----\nEXTENDS Naturals,TLC\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n====\n";
    let translation = translate_source("Algorithm.tla", source, &TranslationRequest::default());
    assert!(translation.accepted(), "{}", translation.to_json());
    let document = translation.to_json();
    assert!(document.get("model_proved").is_none());
    assert!(document["module_source"].is_string());
    let project = Project::new("Algorithm", source, "")?;
    let published = write_translation_file(&project.file, &RewriteRequest::default())?;
    assert!(published.accepted(), "{}", published.to_json());
    assert_eq!(
        published.to_json()["filesystem_plan"]["writes_performed"],
        true
    );
    assert_eq!(
        fs::read_to_string(project.file.with_extension("old"))?,
        source
    );
    let checked = check_file_engine(&project.request(false), Engine::Core)?;
    assert_eq!(checked.verdict(), "success", "{}", checked.to_json());
    assert_eq!(checked.to_json()["statistics"]["distinct"], 3);
    Ok(())
}

#[test]
fn tlc_logical_coverage_is_available_to_an_independent_rust_consumer() -> Result {
    let source = "---- MODULE Costs ----\nEXTENDS Naturals\nVARIABLE x,y\nH(n)==n+1\nInit==/\\ x=0 /\\ y=0\nA==/\\ x<2 /\\ x'=H(x) /\\ y'=H(y)\nB==/\\ x=2 /\\ x'=x /\\ y'=y\nNext==A \\/ B\n====\n";
    let project = Project::new("Costs", source, "INIT Init\nNEXT Next\n")?;
    for compat in [false, true] {
        let mut request = project.request(compat);
        request.tlc_coverage = true;
        let report = check_file_engine(&request, Engine::Reference)?;
        assert_eq!(report.verdict(), "success");
        let costs = report.tlc_coverage().ok_or("logical coverage missing")?;
        assert!(!costs.active);
        assert!(costs.unsupported.is_none());
        assert_eq!(costs.actions.len(), 3);
        assert_eq!(costs.actions[1].generated, 2);
        assert!(
            costs.actions[1]
                .rows
                .iter()
                .any(|row| row.location.module == "Costs" && row.invocations > 0)
        );
        assert!(report.source_coverage().is_none());
    }
    Ok(())
}
