//! End to end: `pipeline::run` gives every selected model and module exactly
//! one outcome, so the summary totals equal the enumerated selection. Also:
//! an empty selection is a usage error, and a run under a result-changing
//! option is never `agree` (ADR 0010).

use dive_conformance::cache::Cache;
use dive_conformance::compare::Outcome;
use dive_conformance::config::Config;
use dive_conformance::divergence::Profile;
use dive_conformance::error::{Error, Result as DResult};
use dive_conformance::golden::{
    GOLDEN_SCHEMA, GoldenModel, ModelGolden, Provenance, model_golden_path, to_pretty,
};
use dive_conformance::implementation::{ImplOption, ImplResult, Implementation, ModelRequest};
use dive_conformance::oracle::{Capabilities, Java, JavaSource, OracleProvenance};
use dive_conformance::pipeline::{self, Limits, RunOptions, Selection};
use dive_conformance::sany::SanyResult;
use dive_conformance::tlc::result::{Counts, OracleResult, Verdict};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MANIFEST: &str = r#"{
  "sources": [], "authors": ["A"], "tags": [],
  "modules": [
    {"path": "specifications/x/X.tla", "features": [], "models": [
      {"path": "specifications/x/X.cfg", "runtime": "00:00:01", "mode": "exhaustive search", "result": "success"},
      {"path": "specifications/x/Apa.cfg", "runtime": "00:00:01", "mode": "symbolic", "result": "success"}
    ]},
    {"path": "specifications/x/Helper.tla", "features": [], "models": []}
  ]
}"#;

fn git(dir: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// An implementation that refuses everything.
struct Refuser;

impl Implementation for Refuser {
    fn name(&self) -> String {
        "refuser".to_owned()
    }
    fn is_oracle(&self) -> bool {
        false
    }
    fn check_model(&self, _req: &ModelRequest<'_>) -> DResult<ImplResult<OracleResult>> {
        Ok(ImplResult::Unsupported("test".to_owned()))
    }
    fn parse_module(&self, _r: &Path, _m: &str, _t: Duration) -> DResult<ImplResult<SanyResult>> {
        Ok(ImplResult::Unsupported("test".to_owned()))
    }
}

fn setup(tmp: &Path) -> Result<(PathBuf, Cache), Box<dyn std::error::Error>> {
    setup_spec(tmp, "---- MODULE X ----\n====\n", "\n")
}

fn setup_spec(
    tmp: &Path,
    source: &str,
    cfg: &str,
) -> Result<(PathBuf, Cache), Box<dyn std::error::Error>> {
    setup_named_spec(tmp, source, cfg, "x", "X", "X")
}

fn setup_named_spec(
    tmp: &Path,
    source: &str,
    cfg: &str,
    directory: &str,
    module: &str,
    model: &str,
) -> Result<(PathBuf, Cache), Box<dyn std::error::Error>> {
    let src = tmp.join("src");
    let spec = src.join(format!("specifications/{directory}"));
    std::fs::create_dir_all(&spec)?;
    let manifest = MANIFEST
        .replace("specifications/x/", &format!("specifications/{directory}/"))
        .replace("X.tla", &format!("{module}.tla"))
        .replace("X.cfg", &format!("{model}.cfg"));
    std::fs::write(spec.join("manifest.json"), manifest)?;
    std::fs::write(spec.join(format!("{module}.tla")), source)?;
    std::fs::write(spec.join(format!("{model}.cfg")), cfg)?;
    std::fs::write(spec.join("Apa.cfg"), "\n")?;
    std::fs::write(spec.join("Helper.tla"), "---- MODULE Helper ----\n====\n")?;
    git(&src, &["init", "-q"])?;
    git(&src, &["add", "-A"])?;
    git(&src, &["commit", "-q", "-m", "c"])?;
    let commit = git(&src, &["rev-parse", "HEAD"])?;
    let cache = Cache::new(tmp.join("cache"));
    let root = cache.checkout_dir("examples", &commit);
    std::fs::create_dir_all(root.parent().ok_or("parent")?)?;
    std::fs::rename(&src, &root)?;
    std::fs::write(root.join(".git/dive-checkout-complete"), &commit)?;
    let repo = tmp.join("repo");
    std::fs::create_dir_all(repo.join("conformance"))?;
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/oracles.toml"),
        repo.join("conformance/oracles.toml"),
    )?;
    std::fs::write(
        repo.join("conformance/corpora.toml"),
        format!(
            "[[corpus]]\nid = \"examples\"\nrepo = \"x\"\ncommit = \"{commit}\"\nlicense = \"MIT\"\nlayout = \"examples-manifest\"\n"
        ),
    )?;
    Ok((repo, cache))
}

fn options(repo: &Path, filters: Vec<String>) -> RunOptions {
    RunOptions {
        corpora: vec!["examples".to_owned()],
        sany_phase: dive_conformance::sany::SanyPhase::Full,
        golden_oracle: "tlc-2026.10.06.014338".to_owned(),
        selection: Selection {
            tier: "all".to_owned(),
            filters,
        },
        workers: vec![1, 4],
        limits: Limits {
            jobs: 2,
            xmx: "1g".to_owned(),
            timeout: Some(Duration::from_secs(5)),
            timeout_cap: Duration::from_secs(5),
        },
        crosscheck_manifest: true,
        models: true,
        sany: true,
        report_dir: repo.join("reports"),
        summary_path: Some(repo.join("conformance.md")),
    }
}

#[test]
fn summary_totals_equal_the_selection() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("totals")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let report = pipeline::run(&config, &cache, &Refuser, None, &options(&repo, vec![]))?;
    let s = &report.summary;
    assert_eq!((s.models_selected, s.modules_selected), (2, 2));
    assert_eq!(s.models.values().sum::<usize>(), 2);
    assert_eq!(s.modules.values().sum::<usize>(), 2);
    assert_eq!(report.models.len() + report.modules.len(), 4);
    // No goldens exist: the in-scope model is an oracle error, the symbolic
    // one out of scope, and both modules oracle errors (none vanish).
    assert_eq!(s.models.get(&Outcome::OracleError), Some(&1));
    assert_eq!(s.models.get(&Outcome::OutOfScope), Some(&1));
    assert_eq!(s.modules.get(&Outcome::OracleError), Some(&2));
    // Two runs in the same millisecond still get distinct report ids.
    let again = pipeline::run(&config, &cache, &Refuser, None, &options(&repo, vec![]))?;
    assert_ne!(again.run_id, report.run_id);
    Ok(())
}

#[test]
fn module_only_runs_do_not_select_models() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("modules-only")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    for phase in [
        dive_conformance::sany::SanyPhase::Full,
        dive_conformance::sany::SanyPhase::Parse,
    ] {
        let mut opts = options(&repo, vec![]);
        opts.models = false;
        opts.sany_phase = phase;
        let report = pipeline::run(&config, &cache, &Refuser, None, &opts)?;
        assert_eq!(report.summary.models_selected, 0);
        assert_eq!(report.summary.modules_selected, 2);
        assert!(report.models.is_empty());
        assert_eq!(report.modules.len(), 2);
        assert_eq!(report.summary.models.values().sum::<usize>(), 0);
    }
    Ok(())
}

#[test]
fn empty_selection_is_a_usage_error() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("empty")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let r = pipeline::run(
        &config,
        &cache,
        &Refuser,
        None,
        &options(&repo, vec!["no/such/*".to_owned()]),
    );
    assert!(matches!(r, Err(Error::Usage(_))));
    Ok(())
}

fn success() -> OracleResult {
    OracleResult {
        verdict: Verdict::Success,
        verdict_code: Some(2193),
        error_class: Some("TLC_SUCCESS".to_owned()),
        exit_code: Some(0),
        exit_status: Some("SUCCESS".to_owned()),
        violated_property: None,
        counts: Counts {
            distinct_states: Some(4),
            states_generated: Some(5),
            states_left_on_queue: Some(0),
            depth: Some(4),
        },
        fingerprint_collision: None,
        trace: None,
        error_codes: vec![],
        warning_codes: vec![],
        unresolved_modules: vec![],
        note: None,
    }
}

/// A non-oracle implementation that reproduces the golden exactly, under
/// the given options.
struct Optioned {
    options: Vec<ImplOption>,
    profile: Option<Profile>,
    result: OracleResult,
}

impl Implementation for Optioned {
    fn name(&self) -> String {
        "optioned".to_owned()
    }
    fn is_oracle(&self) -> bool {
        false
    }
    fn options(&self) -> &[ImplOption] {
        &self.options
    }
    fn profile(&self) -> Option<Profile> {
        self.profile.clone()
    }
    fn result_preserving_options(&self) -> &[&str] {
        &["seed"]
    }
    fn check_model(&self, _req: &ModelRequest<'_>) -> DResult<ImplResult<OracleResult>> {
        Ok(ImplResult::Completed {
            result: self.result.clone(),
            command: vec![],
            elapsed: Duration::ZERO,
        })
    }
    fn trace_validated(
        &self,
        req: &ModelRequest<'_>,
        result: &OracleResult,
        golden: &OracleResult,
    ) -> dive_conformance::trace_check::TraceCheck {
        match dive_conformance::trace_check::load_model(
            req.corpus_root,
            req.model,
            &[],
            self.profile == Some(Profile::Tlc),
        ) {
            Ok(spec) => {
                dive_conformance::trace_check::validate_counterexample(&spec, result, golden)
            }
            Err(e) => dive_conformance::trace_check::TraceCheck::no(e),
        }
    }
    fn parse_module(&self, _r: &Path, _m: &str, _t: Duration) -> DResult<ImplResult<SanyResult>> {
        Ok(ImplResult::Unsupported("test".to_owned()))
    }
}

fn write_success_golden(repo: &Path, config: &Config) -> TestResult {
    write_result_golden(repo, config, success())
}

fn write_result_golden(repo: &Path, config: &Config, result: OracleResult) -> TestResult {
    write_result_golden_at(repo, config, result, 1)
}

fn write_result_golden_at(
    repo: &Path,
    config: &Config,
    result: OracleResult,
    workers: u32,
) -> TestResult {
    let oracle = config.oracles.default_oracle()?.clone();
    let corpus = config.corpora.corpus("examples")?.clone();
    let golden = ModelGolden {
        schema: GOLDEN_SCHEMA,
        kind: "tlc-model".to_owned(),
        oracle: oracle.id.clone(),
        corpus: "examples".to_owned(),
        model: GoldenModel {
            id: "specifications/x/X.cfg".to_owned(),
            module: "specifications/x/X.tla".to_owned(),
            mode: dive_conformance::corpus::ModelMode::Exhaustive,
            features: vec![],
            counts_comparable: true,
        },
        workers,
        result,
        provenance: Provenance {
            oracle: OracleProvenance {
                id: oracle.id.clone(),
                sha256: oracle.sha256.clone(),
                upstream_commit: String::new(),
                classpath: vec![],
                module_libraries: vec![],
                capabilities: Capabilities::default(),
                java: Java {
                    path: PathBuf::new(),
                    source: JavaSource::Pinned {
                        version: "17".to_owned(),
                        sha256: "1".repeat(64),
                        platform: "test".to_owned(),
                    },
                    version_line: String::new(),
                },
            },
            corpus_commit: corpus.commit.clone(),
            command: vec![],
            trace_command: None,
            timeout_seconds: 1,
            reason: (workers > 1).then(|| "test worker-specific word obligation".into()),
            generator: "test".to_owned(),
            generated_at: String::new(),
            elapsed_ms: 1,
            uncompared_counts: None,
        },
    };
    let path = if workers == 1 {
        model_golden_path(
            &repo.join("conformance/golden"),
            &oracle.id,
            "examples",
            "specifications/x/X.cfg",
        )
    } else {
        dive_conformance::golden::worker_golden_path(
            &repo.join("conformance/golden"),
            &oracle.id,
            "examples",
            "specifications/x/X.cfg",
            workers,
        )
    };
    std::fs::create_dir_all(path.parent().ok_or("parent")?)?;
    std::fs::write(&path, to_pretty(&golden)?)?;
    Ok(())
}

#[test]
fn registry_excuses_only_under_the_dive_profile() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("registry")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    write_success_golden(&repo, &config)?;
    // dive (big integers) reports success where TLC overflows: here the
    // golden says success and dive says overflow, i.e. a disagreement.
    let mut overflow = success();
    overflow.verdict = Verdict::Overflow;
    overflow.verdict_code = Some(2178);
    overflow.error_class = Some("TLC_MODULE_OVERFLOW".to_owned());
    std::fs::write(
        repo.join("conformance/expected-divergences.toml"),
        "[[divergence]]\nid = \"test\"\npattern = \"specifications/x/*\"\nprofile = \"dive\"\nexpected_verdict = \"overflow\"\nfields = [\"verdict\", \"error_class\"]\njustification = \"test\"\nlink = \"docs/divergences.md#test\"\n",
    )?;
    let mut opts = options(&repo, vec!["specifications/x/X.cfg".to_owned()]);
    opts.sany = false;
    opts.workers = vec![1];
    let run = |profile: Option<Profile>| -> Result<_, Box<dyn std::error::Error>> {
        let imp = Optioned {
            options: vec![],
            profile,
            result: overflow.clone(),
        };
        Ok(pipeline::run(&config, &cache, &imp, None, &opts)?)
    };
    let dive = run(Some(Profile::Dive))?;
    assert_eq!(dive.models[0].outcome, Outcome::ExpectedDivergence);
    assert_eq!(dive.settings.strict_tier, Some(false));
    let tlc = run(Some(Profile::Tlc))?;
    assert_eq!(tlc.models[0].outcome, Outcome::Disagree);
    assert_eq!(tlc.settings.strict_tier, Some(true));
    let over = run(Some(Profile::TlcWithOverrides(vec!["int".to_owned()])))?;
    assert_eq!(over.models[0].outcome, Outcome::Disagree);
    assert_eq!(over.settings.strict_tier, Some(false));
    // An entry narrowed to other fields does not excuse the verdict.
    std::fs::write(
        repo.join("conformance/expected-divergences.toml"),
        "[[divergence]]\nid = \"counts\"\npattern = \"specifications/x/*\"\nprofile = \"dive\"\nexpected_verdict = \"overflow\"\nfields = [\"counts.*\"]\njustification = \"test\"\nlink = \"docs/divergences.md#test\"\n",
    )?;
    assert_eq!(
        run(Some(Profile::Dive))?.models[0].outcome,
        Outcome::Disagree
    );
    // A registry entry claiming to apply under the tlc profile is refused.
    std::fs::write(
        repo.join("conformance/expected-divergences.toml"),
        "[[divergence]]\nid = \"bad\"\npattern = \"*\"\nprofile = \"tlc\"\nexpected_verdict = \"overflow\"\njustification = \"x\"\nlink = \"docs/divergences.md\"\n",
    )?;
    assert!(run(Some(Profile::Dive)).is_err());
    Ok(())
}

#[test]
fn result_changing_options_never_agree() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("options")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    write_success_golden(&repo, &config)?;
    let filters = vec!["specifications/x/X.cfg".to_owned()];
    let mut opts = options(&repo, filters);
    opts.sany = false;
    opts.workers = vec![1];
    // A declared result-preserving option (`seed`) still agrees.
    let mut imp = Optioned {
        options: vec![],
        profile: Some(Profile::Dive),
        result: success(),
    };
    imp.options = vec![ImplOption::parse("seed=7", &imp)?];
    assert!(!imp.options[0].result_changing);
    let report = pipeline::run(&config, &cache, &imp, None, &opts)?;
    assert_eq!(report.models[0].outcome, Outcome::Agree);
    assert_eq!(report.settings.impl_options, imp.options);
    // An undeclared option is result-changing: never `agree`.
    imp.options = vec![ImplOption::parse("int=big", &imp)?];
    assert!(imp.options[0].result_changing);
    let report = pipeline::run(&config, &cache, &imp, None, &opts)?;
    assert_eq!(report.models[0].outcome, Outcome::DivergenceMode);
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "source-bound mapper/pipeline positive and negative matrix shares one pinned model"
)]
fn default_exit_pairs_preserve_physical_codes_and_independent_words() -> TestResult {
    use dive_api::trace::{Ending, TraceDoc};
    use dive_conformance::dive::{CheckMapping, check_result};
    use dive_conformance::process::ProcessOutcome;
    use serde_json::json;

    let tmp = dive_conformance::util::TempDir::new("exact-default-exit")?;
    let (repo, cache) = setup_spec(
        tmp.path(),
        "---- MODULE X ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1\nInv == x<2\n====\n",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    let config = Config::load(&repo)?;
    let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1}), json!({"x":2})])?
        .with_ending(Ending::Invariant("Inv".into()));
    let native = json!({
        "schema_version":1, "command":"check", "verdict":"invariant-violation", "property":"Inv",
        "tlc":{"code":2110,"name":"TLC_INVARIANT_VIOLATED_BEHAVIOR","exit_status":12,"verified":true},
        "statistics":{"reported_by_tlc":true,"distinct":3,"generated":3,"left_on_queue":0,"depth":3},
        "counterexample":{"states":[{"text":"x = 0"},{"text":"x = 1"},{"text":"x = 2"}],"trace":doc.to_json()}
    });
    let mapped =
        |document: &serde_json::Value, code| -> Result<OracleResult, Box<dyn std::error::Error>> {
            let process = ProcessOutcome {
                exit_code: code,
                signal: if code.is_none() { Some(9) } else { None },
                timed_out: false,
                stdout: document.to_string(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
            };
            match check_result(&process, false) {
                CheckMapping::Result(r) => Ok(*r),
                CheckMapping::Refused(reason) => Err(reason.into()),
            }
        };
    let golden = mapped(&native, Some(12))?;
    write_result_golden(&repo, &config, golden)?;
    std::fs::write(
        repo.join("conformance/expected-divergences.toml"),
        r#"
[[divergence]]
id = "known-native-property-status"
pattern = "specifications/x/X.cfg"
profile = "dive"
expected_verdict = "safety-violation"
fields = []
exit_code_pair = { expected = 12, actual = 10 }
justification = "established native property status"
link = "docs/divergences.md#d-023-truthful-default-numeric-exit-accounting"
"#,
    )?;
    let mut opts = options(&repo, vec!["specifications/x/X.cfg".into()]);
    opts.sany = false;
    let run =
        |document: &serde_json::Value, code, profile| -> Result<_, Box<dyn std::error::Error>> {
            let imp = Optioned {
                options: vec![],
                profile: Some(profile),
                result: mapped(document, code)?,
            };
            Ok(pipeline::run(&config, &cache, &imp, None, &opts)?)
        };
    for code in [
        None,
        Some(-1),
        Some(0),
        Some(10),
        Some(12),
        Some(13),
        Some(77),
        Some(256),
    ] {
        for profile in [Profile::Dive, Profile::Tlc] {
            let report = run(&native, code, profile.clone())?;
            let expected = if code == Some(12) {
                Outcome::Agree
            } else if code == Some(10) && profile == Profile::Dive {
                Outcome::ExpectedDivergence
            } else {
                Outcome::Disagree
            };
            assert_eq!(report.models[0].outcome, expected, "{profile:?} / {code:?}");
            for row in &report.models[0].runs {
                assert_eq!(row.exit_code, code);
                assert_eq!(row.outcome, expected);
                if code != Some(12) {
                    assert!(row.diffs.iter().any(|d| d.field == "exit_code"));
                }
            }
        }
    }
    for mutation in [
        "invalid-word",
        "unconfirmed-word",
        "fresh-name-word",
        "class",
        "class-null",
        "class-bool",
        "class-float",
        "class-range",
        "property",
        "counts",
    ] {
        let mut bad = native.clone();
        match mutation {
            "invalid-word" => {
                let mut wrong = doc.clone();
                wrong.states[1].insert("x".into(), json!(99));
                bad["counterexample"]["trace"] = wrong.to_json();
            }
            "unconfirmed-word" => bad["counterexample"]["trace"] = serde_json::Value::Null,
            "fresh-name-word" => {
                let mut wrong = doc.clone();
                wrong.states[1].insert("x".into(), json!("previously unseen name"));
                bad["counterexample"]["trace"] = wrong.to_json();
            }
            "class" => bad["tlc"]["code"] = json!(99999),
            "class-null" => bad["tlc"]["code"] = serde_json::Value::Null,
            "class-bool" => bad["tlc"]["code"] = json!(true),
            "class-float" => bad["tlc"]["code"] = json!(2110.0),
            "class-range" => bad["tlc"]["code"] = json!(2_147_483_648_u64),
            "property" => bad["property"] = json!("Other"),
            "counts" => bad["statistics"]["generated"] = json!(99),
            _ => return Err(format!("unknown test mutation {mutation}").into()),
        }
        let report = run(&bad, Some(10), Profile::Dive)?;
        assert_eq!(report.models[0].outcome, Outcome::Disagree, "{mutation}");
        if mutation.ends_with("word") {
            assert!(
                report.models[0].runs.iter().all(|r| r
                    .diffs
                    .iter()
                    .any(|d| d.field == "trace.invalid" || d.field == "trace.validated")),
                "{mutation}"
            );
        }
    }
    Ok(())
}

#[test]
fn exact_exit_pair_cannot_excuse_a_named_word_refusal() -> TestResult {
    use dive_api::trace::{Ending, TraceDoc};
    use dive_conformance::tlc::result::{Trace, TraceEnding};
    use serde_json::json;

    let tmp = dive_conformance::util::TempDir::new("exit-word-refusal")?;
    let (repo, cache) = setup_spec(
        tmp.path(),
        "---- MODULE X ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=0\nNext == x'=RandomElement({1})\nInv == x<1\n====\n",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    let config = Config::load(&repo)?;
    let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])?
        .with_ending(Ending::Invariant("Inv".into()));
    let mut golden = success();
    golden.verdict = Verdict::SafetyViolation;
    golden.verdict_code = Some(2110);
    golden.error_class = Some("TLC_INVARIANT_VIOLATED_BEHAVIOR".into());
    golden.exit_code = Some(12);
    golden.violated_property = Some("Inv".into());
    golden.trace = Some(Trace {
        length: 2,
        ending: TraceEnding::Finite,
        json: Some(doc.to_json()),
        steps: None,
        omitted: None,
        json_unavailable: None,
    });
    write_result_golden(&repo, &config, golden.clone())?;
    std::fs::write(
        repo.join("conformance/expected-divergences.toml"),
        r#"
[[divergence]]
id = "known-native-property-status"
pattern = "specifications/x/X.cfg"
profile = "dive"
expected_verdict = "safety-violation"
fields = []
exit_code_pair = { expected = 12, actual = 10 }
justification = "established native property status"
link = "docs/divergences.md#d-023-truthful-default-numeric-exit-accounting"
"#,
    )?;
    let mut result = golden;
    result.exit_code = Some(10);
    let imp = Optioned {
        options: vec![],
        profile: Some(Profile::Dive),
        result,
    };
    let mut opts = options(&repo, vec!["specifications/x/X.cfg".into()]);
    opts.sany = false;
    opts.workers = vec![1, 4, 8];
    let report = pipeline::run(&config, &cache, &imp, None, &opts)?;
    assert_eq!(report.models[0].outcome, Outcome::Disagree);
    for row in &report.models[0].runs {
        assert_eq!(row.exit_code, Some(10));
        assert!(row.diffs.iter().any(|d| d.field == "exit_code"));
        assert!(row.diffs.iter().any(|d| d.field == "trace.validated"));
        assert!(!row.diffs.iter().any(|d| d.field == "trace.invalid"));
        assert!(
            row.detail
                .as_deref()
                .is_some_and(|s| s.contains("unsupported"))
        );
    }
    Ok(())
}

// These controls share the source-bound temporary corpus and golden helpers.
#[path = "pipeline_totals/whole_words.rs"]
mod whole_words;
