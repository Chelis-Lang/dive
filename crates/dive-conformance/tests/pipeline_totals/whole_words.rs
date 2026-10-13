//! Whole-counterexample obligations, independently replayed through the public pipeline.

use super::*;
use dive_api::trace::{Ending, TraceDoc};
use dive_conformance::tlc::result::{Trace, TraceEnding, TraceStep};
use dive_conformance::trace_check::TraceCheck;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Replay {
    result: OracleResult,
    profile: Profile,
    lie: bool,
    calls: AtomicUsize,
    primary_count: Option<u64>,
}

impl Implementation for Replay {
    fn name(&self) -> String {
        "whole-word-control".into()
    }
    fn is_oracle(&self) -> bool {
        false
    }
    fn profile(&self) -> Option<Profile> {
        Some(self.profile.clone())
    }
    fn check_model(&self, _req: &ModelRequest<'_>) -> DResult<ImplResult<OracleResult>> {
        Ok(ImplResult::Completed {
            result: self.result.clone(),
            command: vec![],
            elapsed: Duration::ZERO,
        })
    }
    fn parse_module(&self, _r: &Path, _m: &str, _t: Duration) -> DResult<ImplResult<SanyResult>> {
        Ok(ImplResult::Unsupported("not selected".into()))
    }
    fn trace_validated(
        &self,
        req: &ModelRequest<'_>,
        result: &OracleResult,
        golden: &OracleResult,
    ) -> TraceCheck {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.lie {
            return TraceCheck::yes("untrustworthy claim with no word");
        }
        let expected_count = if self.profile == Profile::Tlc && req.workers > 1 {
            Some(700 + u64::from(req.workers))
        } else {
            self.primary_count
        };
        if golden.counts.distinct_states != expected_count {
            return TraceCheck::refuted("validator received a different golden than comparison");
        }
        let spec = match dive_conformance::trace_check::load_model(
            req.corpus_root,
            req.model,
            &[],
            self.profile == Profile::Tlc,
        ) {
            Ok(spec) => spec,
            Err(e) => return TraceCheck::no(e),
        };
        // A custom implementation may carry JSON state literals as text
        // steps. Decode those into a word, then use the same fresh validator.
        let mut decoded = result.clone();
        if let Some(t) = decoded.trace.as_mut()
            && t.json.is_none()
            && let Some(steps) = &t.steps
        {
            let values: Result<Vec<_>, _> = steps
                .iter()
                .map(|s| serde_json::from_str(&s.state))
                .collect();
            let values = match values {
                Ok(v) => v,
                Err(e) => return TraceCheck::no(e.to_string()),
            };
            let mut doc = match TraceDoc::behavior(values) {
                Ok(d) => d,
                Err(e) => return TraceCheck::no(e),
            };
            doc.ending = dive_conformance::trace_check::ending_of(result);
            if result.verdict == Verdict::LivenessViolation {
                doc.properties = result.violated_property.iter().cloned().collect();
            }
            t.json = Some(doc.to_json());
        }
        dive_conformance::trace_check::validate_counterexample(&spec, &decoded, golden)
    }
}

fn word_case(
    verdict: Verdict,
) -> Result<(String, String, OracleResult), Box<dyn std::error::Error>> {
    let (next, claim, cfg, code, class, exit, ending, states, shape) = match verdict {
        Verdict::SafetyViolation => (
            "x'=1",
            "Inv == x<1",
            "INVARIANT Inv",
            2110,
            "TLC_INVARIANT_VIOLATED_BEHAVIOR",
            12,
            Ending::Invariant("Inv".into()),
            vec![json!({"x":0}), json!({"x":1})],
            TraceEnding::Finite,
        ),
        Verdict::ActionPropertyViolation => (
            "x'=1",
            "A == [][x'=x]_x",
            "PROPERTY A",
            2112,
            "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR",
            13,
            Ending::Property("A".into()),
            vec![json!({"x":0}), json!({"x":1})],
            TraceEnding::Finite,
        ),
        Verdict::LivenessViolation => (
            "UNCHANGED x",
            "P == []<>(x=1)",
            "PROPERTY P",
            2116,
            "TLC_TEMPORAL_PROPERTY_VIOLATED",
            13,
            Ending::Stuttering,
            vec![json!({"x":0})],
            TraceEnding::Stuttering,
        ),
        Verdict::Deadlock => (
            "/\\ x=0 /\\ x'=1",
            "",
            "",
            2114,
            "TLC_DEADLOCK_REACHED",
            11,
            Ending::Deadlock,
            vec![json!({"x":0}), json!({"x":1})],
            TraceEnding::Finite,
        ),
        _ => return Err("not a mandatory-word verdict".into()),
    };
    let source = format!(
        "---- MODULE X ----\nEXTENDS Naturals, TLC\nVARIABLE x\nWorkers == TLCGet(\"config\").worker\nInit == x=0\nNext == {next}\n{claim}\n====\n"
    );
    let cfg = format!("INIT Init\nNEXT Next\n{cfg}\n");
    let mut doc = TraceDoc::behavior(states)?.with_ending(ending);
    let mut result = success();
    result.verdict = verdict;
    result.verdict_code = Some(code);
    result.error_class = Some(class.into());
    result.exit_code = Some(exit);
    result.violated_property = match verdict {
        Verdict::SafetyViolation => Some("Inv".into()),
        Verdict::ActionPropertyViolation => Some("A".into()),
        Verdict::LivenessViolation => Some("P".into()),
        _ => None,
    };
    if verdict == Verdict::LivenessViolation {
        doc.properties = vec!["P".into()];
    }
    let length = u64::try_from(doc.states.len())?;
    result.counts = Counts {
        distinct_states: Some(length),
        states_generated: Some(length),
        states_left_on_queue: Some(0),
        depth: Some(length),
    };
    result.trace = Some(Trace {
        length,
        ending: shape,
        json: Some(doc.to_json()),
        steps: None,
        omitted: None,
        json_unavailable: None,
    });
    Ok((source, cfg, result))
}

struct WordWorld {
    repo: PathBuf,
    cache: Cache,
    config: Config,
    golden: OracleResult,
    opts: RunOptions,
}

fn setup_words(tmp: &Path, verdict: Verdict) -> Result<WordWorld, Box<dyn std::error::Error>> {
    let (source, cfg, golden) = word_case(verdict)?;
    let (repo, cache) = setup_spec(tmp, &source, &cfg)?;
    let config = Config::load(&repo)?;
    write_result_golden(&repo, &config, golden.clone())?;
    for width in [4, 8] {
        let mut worker = golden.clone();
        worker.counts.distinct_states = Some(700 + u64::from(width));
        write_result_golden_at(&repo, &config, worker, width)?;
    }
    let mut opts = options(&repo, vec!["specifications/x/X.cfg".into()]);
    opts.sany = false;
    opts.workers = vec![1, 4, 8];
    Ok(WordWorld {
        repo,
        cache,
        config,
        golden,
        opts,
    })
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one source-bound four-verdict word mutation matrix shares selected goldens and fresh replay"
)]
fn required_words_cannot_be_missing_metadata_only_or_invalid() -> TestResult {
    for verdict in [
        Verdict::SafetyViolation,
        Verdict::ActionPropertyViolation,
        Verdict::LivenessViolation,
        Verdict::Deadlock,
    ] {
        let tmp = dive_conformance::util::TempDir::new("required-word")?;
        let WordWorld {
            cache,
            config,
            golden,
            opts,
            ..
        } = setup_words(tmp.path(), verdict)?;
        for mutation in [
            "valid-json",
            "valid-text",
            "missing",
            "metadata",
            "invalid",
            "unconfirmed",
            "null",
            "empty-json",
            "empty-text",
        ] {
            let mut result = golden.clone();
            match mutation {
                "valid-json" => {}
                "missing" => result.trace = None,
                "metadata" => {
                    if let Some(t) = result.trace.as_mut() {
                        t.json = None;
                        t.steps = None;
                    }
                }
                "valid-text" => {
                    if let Some(t) = result.trace.as_mut() {
                        let doc = TraceDoc::from_json(t.json.as_ref().ok_or("doc")?)?;
                        t.steps = Some(
                            doc.states
                                .iter()
                                .map(|state| TraceStep {
                                    label: String::new(),
                                    state: json!(state).to_string(),
                                })
                                .collect(),
                        );
                        t.json = None;
                    }
                }
                "invalid" => {
                    if let Some(t) = result.trace.as_mut() {
                        let mut doc = TraceDoc::from_json(t.json.as_ref().ok_or("doc")?)?;
                        doc.states
                            .last_mut()
                            .ok_or("state")?
                            .insert("x".into(), json!(99));
                        t.json = Some(doc.to_json());
                    }
                }
                "unconfirmed" => {
                    if let Some(t) = result.trace.as_mut() {
                        t.json = Some(json!({"format":"unknown"}));
                    }
                }
                "null" => {
                    if let Some(t) = result.trace.as_mut() {
                        t.json = Some(serde_json::Value::Null);
                    }
                }
                "empty-json" => {
                    if let Some(t) = result.trace.as_mut() {
                        let mut doc = TraceDoc::from_json(t.json.as_ref().ok_or("doc")?)?;
                        doc.states.clear();
                        t.json = Some(doc.to_json());
                    }
                }
                "empty-text" => {
                    if let Some(t) = result.trace.as_mut() {
                        t.json = None;
                        t.steps = Some(vec![]);
                    }
                }
                _ => return Err("unknown mutation".into()),
            }
            let lie = matches!(mutation, "missing" | "metadata" | "null" | "empty-text");
            let probe = Replay {
                result,
                profile: Profile::Tlc,
                lie,
                calls: AtomicUsize::new(0),
                primary_count: golden.counts.distinct_states,
            };
            let report = pipeline::run(&config, &cache, &probe, None, &opts)?;
            let positive = mutation.starts_with("valid");
            println!(
                "required word {verdict:?}/{mutation}: {:?}; validation calls {}",
                report.models[0].runs,
                probe.calls.load(Ordering::Relaxed)
            );
            for row in &report.models[0].runs {
                if positive {
                    assert_eq!(
                        row.outcome,
                        Outcome::Agree,
                        "{verdict:?}/{mutation}: {row:?}"
                    );
                } else {
                    assert_ne!(
                        row.outcome,
                        Outcome::Agree,
                        "{verdict:?}/{mutation}: {row:?}"
                    );
                    assert_ne!(row.outcome, Outcome::ExpectedDivergence);
                    assert!(
                        row.diffs
                            .iter()
                            .any(|d| d.field == "trace.invalid" || d.field == "trace.validated"),
                        "{mutation}: {row:?}"
                    );
                }
                if row.workers > 1 {
                    assert!(row.diffs.iter().all(|d| !d.field.starts_with("counts.")
                        && d.field != "trace.length"
                        && d.field != "trace.ending"));
                }
            }
            assert_eq!(probe.calls.load(Ordering::Relaxed), if lie { 0 } else { 3 });
        }
    }
    Ok(())
}

#[test]
fn an_exact_or_registered_exit_cannot_excuse_a_missing_whole_word() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("word-exit-pair")?;
    let WordWorld {
        repo,
        cache,
        config,
        golden,
        opts,
    } = setup_words(tmp.path(), Verdict::SafetyViolation)?;
    std::fs::write(
        repo.join("conformance/expected-divergences.toml"),
        r#"
[[divergence]]
id = "word-status"
pattern = "specifications/x/X.cfg"
profile = "dive"
expected_verdict = "safety-violation"
fields = []
exit_code_pair = { expected = 12, actual = 10 }
justification = "known default property status"
link = "docs/divergences.md#d-023-truthful-default-numeric-exit-accounting"
"#,
    )?;
    for exit in [12, 10] {
        for absence in ["valid", "missing", "metadata"] {
            let missing = absence != "valid";
            let mut result = golden.clone();
            result.exit_code = Some(exit);
            if absence == "missing" {
                result.trace = None;
            } else if absence == "metadata"
                && let Some(t) = result.trace.as_mut()
            {
                t.json = None;
                t.steps = None;
            }
            let probe = Replay {
                result,
                profile: Profile::Dive,
                lie: missing,
                calls: AtomicUsize::new(0),
                primary_count: golden.counts.distinct_states,
            };
            let report = pipeline::run(&config, &cache, &probe, None, &opts)?;
            println!(
                "default exit {exit}/{absence}: {:?}; validation calls {}",
                report.models[0].runs,
                probe.calls.load(Ordering::Relaxed)
            );
            for row in &report.models[0].runs {
                if missing {
                    assert_ne!(row.outcome, Outcome::Agree);
                    assert_ne!(row.outcome, Outcome::ExpectedDivergence);
                    assert!(row.diffs.iter().any(|d| d.field == "trace.validated"));
                } else {
                    assert_eq!(
                        row.outcome,
                        if exit == 12 {
                            Outcome::Agree
                        } else {
                            Outcome::ExpectedDivergence
                        }
                    );
                }
            }
            assert_eq!(
                probe.calls.load(Ordering::Relaxed),
                if missing { 0 } else { 3 }
            );
        }
    }
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one selected-contract matrix distinguishes no-word errors, worker overrides and unavailable oracle witness payloads"
)]
fn no_word_error_statuses_remain_legitimate_but_expected_error_words_are_required() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("no-word-status")?;
    let WordWorld {
        repo,
        cache,
        config,
        golden: original,
        opts,
    } = setup_words(tmp.path(), Verdict::SafetyViolation)?;
    for (verdict, code, exit) in [
        (Verdict::Success, 2193, 0),
        (Verdict::AssumptionFailure, 2104, 10),
        (Verdict::ParseError, 3002, 150),
        (Verdict::SemanticError, 3002, 150),
        (Verdict::EvaluationError, 2179, 255),
    ] {
        let mut result = original.clone();
        result.verdict = verdict;
        result.verdict_code = Some(code);
        result.error_class = dive_conformance::tlc::codes::ec_name(code).map(str::to_owned);
        result.exit_code = Some(exit);
        result.trace = None;
        result.violated_property = None;
        write_result_golden(&repo, &config, result.clone())?;
        for width in [4, 8] {
            let mut worker = result.clone();
            worker.counts.distinct_states = Some(700 + u64::from(width));
            write_result_golden_at(&repo, &config, worker, width)?;
        }
        let probe = Replay {
            result,
            profile: Profile::Tlc,
            lie: true,
            calls: AtomicUsize::new(0),
            primary_count: original.counts.distinct_states,
        };
        let report = pipeline::run(&config, &cache, &probe, None, &opts)?;
        assert_eq!(report.models[0].outcome, Outcome::Agree);
        assert_eq!(probe.calls.load(Ordering::Relaxed), 0);
    }
    // The selected worker golden carries an error witness: now absence is
    // an unconfirmed required word, even though evaluation errors may also
    // legitimately be reported without any witness in other contexts.
    let mut error = original.clone();
    error.verdict = Verdict::EvaluationError;
    error.verdict_code = Some(2179);
    error.error_class = dive_conformance::tlc::codes::ec_name(2179).map(str::to_owned);
    error.exit_code = Some(255);
    error.violated_property = None;
    write_result_golden(&repo, &config, error.clone())?;
    for width in [4, 8] {
        let mut worker = error.clone();
        worker.counts.distinct_states = Some(700 + u64::from(width));
        write_result_golden_at(&repo, &config, worker, width)?;
    }
    // A worker error without a counterexample remains legitimate even
    // when the primary golden carries a witness: use the selected contract.
    for width in [4, 8] {
        let mut worker = error.clone();
        worker.counts.distinct_states = Some(700 + u64::from(width));
        worker.trace = None;
        write_result_golden_at(&repo, &config, worker, width)?;
    }
    let mut no_word = error.clone();
    no_word.trace = None;
    let no_word_probe = Replay {
        result: no_word,
        profile: Profile::Tlc,
        lie: true,
        calls: AtomicUsize::new(0),
        primary_count: original.counts.distinct_states,
    };
    let mut worker_opts = opts.clone();
    worker_opts.workers = vec![4, 8];
    let report = pipeline::run(&config, &cache, &no_word_probe, None, &worker_opts)?;
    assert_eq!(report.models[0].outcome, Outcome::Agree);
    assert_eq!(no_word_probe.calls.load(Ordering::Relaxed), 0);
    // The oracle printed a counterexample but its payload is unavailable.
    // That still requires an observed independently validated whole word;
    // it does not certify any word or authorize omission of one.
    for width in [4, 8] {
        let mut metadata = error.clone();
        metadata.counts.distinct_states = Some(700 + u64::from(width));
        if let Some(t) = metadata.trace.as_mut() {
            t.json = None;
            t.steps = None;
        }
        write_result_golden_at(&repo, &config, metadata, width)?;
    }
    let report = pipeline::run(&config, &cache, &no_word_probe, None, &worker_opts)?;
    println!(
        "unavailable selected worker error witness: {:?}",
        report.models[0].runs
    );
    assert!(report.models[0].runs.iter().all(
        |r| r.outcome != Outcome::Agree && r.diffs.iter().any(|d| d.field == "trace.validated")
    ));
    assert_eq!(no_word_probe.calls.load(Ordering::Relaxed), 0);
    for width in [4, 8] {
        let mut worker = error.clone();
        worker.counts.distinct_states = Some(700 + u64::from(width));
        write_result_golden_at(&repo, &config, worker, width)?;
    }
    error.trace = None;
    let probe = Replay {
        result: error,
        profile: Profile::Tlc,
        lie: true,
        calls: AtomicUsize::new(0),
        primary_count: original.counts.distinct_states,
    };
    let report = pipeline::run(&config, &cache, &probe, None, &opts)?;
    assert!(report.models[0].runs.iter().all(
        |r| r.outcome != Outcome::Agree && r.diffs.iter().any(|d| d.field == "trace.validated")
    ));
    assert_eq!(probe.calls.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "actual native P2 results and pinned golden exercise both approved success and mandatory safety contracts"
)]
fn pinned_p2_success_divergence_needs_no_counterexample_but_safety_still_does() -> TestResult {
    use dive_conformance::dive::{CheckMapping, check_result};
    use dive_conformance::process::ProcessOutcome;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("conformance/corpus/probes/specifications/P2Choose");
    let model = "specifications/P2Choose/P2Choose_str.cfg";
    let mut pinned: ModelGolden =
        serde_json::from_str(&std::fs::read_to_string(model_golden_path(
            &root.join("conformance/golden"),
            "tlc-2026.10.06.014338",
            "probes",
            model,
        ))?)?;
    assert_eq!(pinned.result.verdict, Verdict::SafetyViolation);
    assert_eq!(pinned.result.exit_code, Some(12));
    assert!(pinned.result.trace.is_some());
    let tmp = dive_conformance::util::TempDir::new("p2-word-contract")?;
    let (repo, cache) = setup_named_spec(
        tmp.path(),
        &std::fs::read_to_string(fixture.join("P2Choose.tla"))?,
        &std::fs::read_to_string(fixture.join("P2Choose_str.cfg"))?,
        "P2Choose",
        "P2Choose",
        "P2Choose_str",
    )?;
    let config = Config::load(&repo)?;
    pinned.corpus = "examples".into();
    pinned.provenance.corpus_commit = config.corpora.corpus("examples")?.commit.clone();
    let path = model_golden_path(
        &repo.join("conformance/golden"),
        &pinned.oracle,
        "examples",
        model,
    );
    std::fs::create_dir_all(path.parent().ok_or("golden parent")?)?;
    std::fs::write(path, to_pretty(&pinned)?)?;
    // The production registry is copied without any pattern/field/pair edit.
    std::fs::copy(
        root.join("conformance/expected-divergences.toml"),
        repo.join("conformance/expected-divergences.toml"),
    )?;
    let mut opts = options(&repo, vec![model.into()]);
    opts.sany = false;
    opts.workers = vec![1, 4, 8];
    let request = dive_api::CheckRequest {
        file: fixture.join("P2Choose.tla"),
        config: Some(fixture.join("P2Choose_str.cfg")),
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..dive_api::CheckRequest::default()
    };
    let native = dive_api::check_file(&request)?;
    assert_eq!(native.exit_code(), 0);
    let process = ProcessOutcome {
        exit_code: Some(i32::from(native.exit_code())),
        signal: None,
        timed_out: false,
        stdout: native.to_json().to_string(),
        stderr: String::new(),
        elapsed: Duration::ZERO,
    };
    let CheckMapping::Result(mapped) = check_result(&process, false) else {
        return Err("P2 default refused".into());
    };
    assert_eq!(mapped.verdict, Verdict::Success);
    assert!(mapped.trace.is_none());
    for profile in [Profile::Dive, Profile::Tlc] {
        let probe = Replay {
            result: (*mapped).clone(),
            profile: profile.clone(),
            lie: true,
            calls: AtomicUsize::new(0),
            primary_count: pinned.result.counts.distinct_states,
        };
        let report = pipeline::run(&config, &cache, &probe, None, &opts)?;
        println!("actual P2 success {profile:?}: {:?}", report.models[0].runs);
        assert_eq!(
            report.models[0].outcome,
            if profile == Profile::Dive {
                Outcome::ExpectedDivergence
            } else {
                Outcome::Disagree
            }
        );
        assert_eq!(probe.calls.load(Ordering::Relaxed), 0);
    }
    let mut wrong_json = native.to_json();
    wrong_json["tlc"]["code"] = json!(99999);
    let wrong_process = ProcessOutcome {
        stdout: wrong_json.to_string(),
        ..process
    };
    let CheckMapping::Result(wrong) = check_result(&wrong_process, false) else {
        return Err("wrong class unexpectedly refused".into());
    };
    assert_eq!(wrong.verdict, Verdict::Unclassified);
    let probe = Replay {
        result: *wrong,
        profile: Profile::Dive,
        lie: true,
        calls: AtomicUsize::new(0),
        primary_count: pinned.result.counts.distinct_states,
    };
    assert_eq!(
        pipeline::run(&config, &cache, &probe, None, &opts)?.models[0].outcome,
        Outcome::Disagree
    );
    let compatible = dive_api::check_file(&dive_api::CheckRequest {
        compat_tlc: true,
        ..request
    })?;
    assert_eq!(compatible.exit_code(), 12);
    let process = ProcessOutcome {
        exit_code: Some(12),
        signal: None,
        timed_out: false,
        stdout: compatible.to_json().to_string(),
        stderr: String::new(),
        elapsed: Duration::ZERO,
    };
    let CheckMapping::Result(safety) = check_result(&process, true) else {
        return Err("P2 compatibility refused".into());
    };
    assert_eq!(safety.verdict, Verdict::SafetyViolation);
    let probe = Replay {
        result: (*safety).clone(),
        profile: Profile::Tlc,
        lie: false,
        calls: AtomicUsize::new(0),
        primary_count: pinned.result.counts.distinct_states,
    };
    // P2 does not introspect workers: the primary remains selected at all widths.
    let mut single_opts = opts.clone();
    single_opts.workers = vec![1];
    assert_eq!(
        pipeline::run(&config, &cache, &probe, None, &single_opts)?.models[0].outcome,
        Outcome::Agree
    );
    let mut missing = *safety;
    missing.trace = None;
    let probe = Replay {
        result: missing,
        profile: Profile::Tlc,
        lie: true,
        calls: AtomicUsize::new(0),
        primary_count: pinned.result.counts.distinct_states,
    };
    let report = pipeline::run(&config, &cache, &probe, None, &opts)?;
    assert!(report.models[0].runs.iter().all(
        |r| r.outcome != Outcome::Agree && r.diffs.iter().any(|d| d.field == "trace.validated")
    ));
    assert_eq!(probe.calls.load(Ordering::Relaxed), 0);
    Ok(())
}
