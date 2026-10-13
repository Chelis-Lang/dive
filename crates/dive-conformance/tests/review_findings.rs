//! Adversarial-review regression tests. Each test demonstrates one finding;
//! every test here FAILS on 0eb93d5 and should pass once the finding is fixed.

use dive_conformance::cache::Cache;
use dive_conformance::compare::{CompareContext, FieldDiff, Outcome, diff_results};
use dive_conformance::config::Config;
use dive_conformance::corpus::ModelMode;
use dive_conformance::divergence::Profile;
use dive_conformance::error::Result as DResult;
use dive_conformance::golden::{
    GOLDEN_SCHEMA, GoldenModel, ModelGolden, Provenance, SanyGolden, model_golden_path,
    sany_golden_path, to_pretty, worker_golden_path,
};
use dive_conformance::implementation::{ImplResult, Implementation, ModelRequest};
use dive_conformance::oracle::{Capabilities, Java, JavaSource, OracleProvenance};
use dive_conformance::pipeline::{self, Limits, RunOptions, Selection};
use dive_conformance::sany::{SanyResult, SanyVerdict};
use dive_conformance::tlc::messages;
use dive_conformance::tlc::result::{
    Counts, OracleResult, RunFacts, Trace, TraceEnding, Verdict, classify,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ORACLE: &str = "tlc-2026.10.06.014338";

// ---------------------------------------------------------------- helpers

fn msg(code: i32, class: i32, body: &str) -> String {
    format!("@!@!@STARTMSG {code}:{class} @!@!@\n{body}\n@!@!@ENDMSG {code} @!@!@\n")
}

fn classify_stdout(stdout: &str, exit: i32) -> OracleResult {
    classify(
        &messages::parse(stdout),
        &RunFacts {
            exit_code: Some(exit),
            ..RunFacts::default()
        },
    )
}

fn safety(distinct: u64, generated: u64, depth: u64, trace: u64) -> OracleResult {
    OracleResult {
        verdict: Verdict::SafetyViolation,
        verdict_code: Some(2110),
        error_class: Some("TLC_INVARIANT_VIOLATED_BEHAVIOR".to_owned()),
        exit_code: Some(12),
        exit_status: Some("VIOLATION_SAFETY".to_owned()),
        violated_property: Some("Inv".to_owned()),
        counts: Counts {
            distinct_states: Some(distinct),
            states_generated: Some(generated),
            states_left_on_queue: Some(0),
            depth: Some(depth),
        },
        fingerprint_collision: None,
        trace: Some(Trace {
            length: trace,
            ending: TraceEnding::Finite,
            json: None,
            steps: None,
            omitted: None,
            json_unavailable: None,
        }),
        error_codes: vec![2110],
        warning_codes: vec![],
        unresolved_modules: vec![],
        note: None,
    }
}

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

fn provenance(jar_sha: &str, corpus_commit: &str) -> Provenance {
    Provenance {
        oracle: OracleProvenance {
            id: ORACLE.to_owned(),
            sha256: jar_sha.to_owned(),
            upstream_commit: "94d0c50".to_owned(),
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
                version_line: "openjdk".to_owned(),
            },
        },
        corpus_commit: corpus_commit.to_owned(),
        command: vec!["java".to_owned()],
        trace_command: None,
        timeout_seconds: 60,
        reason: None,
        generator: "test".to_owned(),
        generated_at: "2026-01-01T00:00:00Z".to_owned(),
        elapsed_ms: 1,
        uncompared_counts: None,
    }
}

/// A throw-away repository root, cache and one-spec Examples-style corpus
/// with two modules (`X.tla`, which has a model, and `Helper.tla`, which has
/// none) whose checkout is "verified" exactly as `corpus fetch` leaves it.
struct World {
    _tmp: dive_conformance::util::TempDir,
    repo: PathBuf,
    cache: Cache,
    commit: String,
    jar_sha: String,
    corpus_root: PathBuf,
    counts_comparable: bool,
}

const MANIFEST: &str = r#"{
  "sources": [], "authors": ["A"], "tags": [],
  "modules": [
    {"path": "specifications/x/X.tla", "features": [], "models": [
      {"path": "specifications/x/X.cfg", "runtime": "00:00:01", "mode": "exhaustive search",
       "result": "safety failure"}
    ]},
    {"path": "specifications/x/Helper.tla", "features": [], "models": []}
  ]
}"#;

fn world() -> Result<World, Box<dyn std::error::Error>> {
    world_with("---- MODULE X ----\n====\n")
}

/// [`world`] with `x_tla` as the text of `X.tla`.
fn world_with(x_tla: &str) -> Result<World, Box<dyn std::error::Error>> {
    world_with_config(x_tla, "\n", true)
}

fn world_with_config(
    x_tla: &str,
    cfg: &str,
    counts_comparable: bool,
) -> Result<World, Box<dyn std::error::Error>> {
    let tmp = dive_conformance::util::TempDir::new("review")?;
    // Corpus repository.
    let src = tmp.path().join("src");
    let spec = src.join("specifications/x");
    std::fs::create_dir_all(&spec)?;
    std::fs::write(
        src.join(".gitignore"),
        "*\n!*/\n!*.tla\n!*.cfg\n!manifest.json\n!.gitignore\n*_TTrace_*.tla\n",
    )?;
    let manifest = if counts_comparable {
        MANIFEST.to_owned()
    } else {
        MANIFEST.replace(
            "\"mode\": \"exhaustive search\"",
            "\"mode\": {\"simulate\": {\"traceCount\": 1}}",
        )
    };
    std::fs::write(spec.join("manifest.json"), manifest)?;
    std::fs::write(spec.join("X.tla"), x_tla)?;
    std::fs::write(spec.join("X.cfg"), cfg)?;
    std::fs::write(spec.join("Helper.tla"), "---- MODULE Helper ----\n====\n")?;
    git(&src, &["init", "-q"])?;
    git(&src, &["add", "-A"])?;
    git(&src, &["commit", "-q", "-m", "c"])?;
    let commit = git(&src, &["rev-parse", "HEAD"])?;
    // Cache with the checkout where `corpus fetch` would put it.
    let cache_root = tmp.path().join("cache");
    let cache = Cache::new(cache_root.clone());
    let corpus_root = cache.checkout_dir("examples", &commit);
    std::fs::create_dir_all(corpus_root.parent().ok_or("parent")?)?;
    std::fs::rename(&src, &corpus_root)?;
    std::fs::write(corpus_root.join(".git/dive-checkout-complete"), &commit)?;
    // Repository root: the real oracles.toml, a corpora.toml pinning our repo.
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join("conformance"))?;
    let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/oracles.toml");
    std::fs::copy(&real, repo.join("conformance/oracles.toml"))?;
    std::fs::write(
        repo.join("conformance/corpora.toml"),
        format!(
            "[[corpus]]\nid = \"examples\"\nrepo = \"x\"\ncommit = \"{commit}\"\nlicense = \"MIT\"\nlayout = \"examples-manifest\"\n"
        ),
    )?;
    let config = Config::load(&repo)?;
    let jar_sha = config.oracles.oracle(ORACLE)?.sha256.clone();
    Ok(World {
        _tmp: tmp,
        repo,
        cache,
        commit,
        jar_sha,
        corpus_root,
        counts_comparable,
    })
}

impl World {
    fn config(&self) -> DResult<Config> {
        Config::load(&self.repo)
    }

    fn golden_root(&self) -> PathBuf {
        self.repo.join("conformance/golden")
    }

    fn write_model_golden(&self, result: OracleResult) -> TestResult {
        self.write_golden_at(result, 1, None)
    }

    /// Writes the golden at `workers` (a worker-specific one above 1),
    /// recording `reason`.
    fn write_golden_at(
        &self,
        result: OracleResult,
        workers: u32,
        reason: Option<&str>,
    ) -> TestResult {
        let mut provenance = provenance(&self.jar_sha, &self.commit);
        provenance.reason = reason.map(str::to_owned);
        let g = ModelGolden {
            schema: GOLDEN_SCHEMA,
            kind: "tlc-model".to_owned(),
            oracle: ORACLE.to_owned(),
            corpus: "examples".to_owned(),
            model: GoldenModel {
                id: "specifications/x/X.cfg".to_owned(),
                module: "specifications/x/X.tla".to_owned(),
                mode: if self.counts_comparable {
                    ModelMode::Exhaustive
                } else {
                    ModelMode::Simulate { traces: 1 }
                },
                features: vec![],
                counts_comparable: self.counts_comparable,
            },
            workers,
            result,
            provenance,
        };
        let p = if workers == 1 {
            model_golden_path(
                &self.golden_root(),
                ORACLE,
                "examples",
                "specifications/x/X.cfg",
            )
        } else {
            worker_golden_path(
                &self.golden_root(),
                ORACLE,
                "examples",
                "specifications/x/X.cfg",
                workers,
            )
        };
        std::fs::create_dir_all(p.parent().ok_or("parent")?)?;
        std::fs::write(p, to_pretty(&g)?)?;
        Ok(())
    }

    fn write_sany_golden(&self, module: &str, jar_sha: &str) -> TestResult {
        let g = SanyGolden {
            schema: GOLDEN_SCHEMA,
            kind: "sany-module".to_owned(),
            oracle: ORACLE.to_owned(),
            corpus: "examples".to_owned(),
            module: module.to_owned(),
            result: accept(),
            provenance: provenance(jar_sha, &self.commit),
        };
        let p = sany_golden_path(&self.golden_root(), ORACLE, "examples", module);
        std::fs::create_dir_all(p.parent().ok_or("parent")?)?;
        std::fs::write(p, to_pretty(&g)?)?;
        Ok(())
    }

    fn run(
        &self,
        imp: &dyn Implementation,
        workers: Vec<u32>,
    ) -> DResult<dive_conformance::report::Report> {
        self.run_kind(imp, workers, true)
    }

    fn run_kind(
        &self,
        imp: &dyn Implementation,
        workers: Vec<u32>,
        sany: bool,
    ) -> DResult<dive_conformance::report::Report> {
        let opts = RunOptions {
            corpora: vec!["examples".to_owned()],
            sany_phase: dive_conformance::sany::SanyPhase::Full,
            golden_oracle: ORACLE.to_owned(),
            selection: Selection {
                tier: "all".to_owned(),
                filters: vec![],
            },
            workers,
            limits: Limits {
                jobs: 1,
                xmx: "1g".to_owned(),
                timeout: Some(Duration::from_secs(5)),
                timeout_cap: Duration::from_secs(5),
            },
            crosscheck_manifest: false,
            models: true,
            sany,
            report_dir: self.repo.join("reports"),
            summary_path: None,
        };
        pipeline::run(&self.config()?, &self.cache, imp, None, &opts)
    }
}

fn accept() -> SanyResult {
    SanyResult {
        verdict: SanyVerdict::Accept,
        accepted: true,
        exit_code: Some(0),
        exit_code_reflects_errors: true,
        error_text: None,
        unresolved_modules: vec![],
        note: None,
    }
}

/// A fake implementation under test that is NOT an oracle (like `dive`).
struct Fake {
    model: OracleResult,
    oracle: bool,
}

impl Implementation for Fake {
    fn name(&self) -> String {
        "fake".to_owned()
    }
    fn is_oracle(&self) -> bool {
        self.oracle
    }
    fn check_model(&self, _req: &ModelRequest<'_>) -> DResult<ImplResult<OracleResult>> {
        Ok(ImplResult::Completed {
            result: self.model.clone(),
            command: vec![],
            elapsed: Duration::ZERO,
        })
    }
    fn parse_module(&self, _r: &Path, _m: &str, _t: Duration) -> DResult<ImplResult<SanyResult>> {
        Ok(ImplResult::Completed {
            result: accept(),
            command: vec![],
            elapsed: Duration::ZERO,
        })
    }
}

// ------------------------------------------------------ critical findings

/// C1. ADR 0004 §3: `dive` must match the single-worker golden's depth,
/// counts and counterexample length at EVERY worker count. The TLC-only
/// multi-worker relaxation is applied to every implementation, so a non-oracle
/// run at 4 workers with a wrong depth, wrong counts and a 3-state
/// counterexample (golden: 7) is reported as `agree`.
#[test]
fn c1_non_oracle_multi_worker_safety_run_must_not_agree() -> TestResult {
    let w = world()?;
    w.write_model_golden(safety(14, 73, 7, 7))?;
    w.write_sany_golden("specifications/x/X.tla", &w.jar_sha)?;
    w.write_sany_golden("specifications/x/Helper.tla", &w.jar_sha)?;
    let imp = Fake {
        model: safety(1, 1, 3, 3),
        oracle: false,
    };
    let report = w.run(&imp, vec![4])?;
    assert_eq!(report.models.len(), 1);
    assert_eq!(
        report.models[0].outcome,
        Outcome::Disagree,
        "non-oracle at 4 workers: depth 3 vs 7, trace 3 vs 7, counts 1/1 vs 14/73 => {:?}",
        report.models[0].runs[0].diffs
    );
    Ok(())
}

/// C2. A module with no SANY golden is silently dropped from the report
/// (models without a golden become `oracle-error`; modules just vanish), so
/// the summary does not add up to the enumeration.
#[test]
fn c2_module_without_sany_golden_is_reported() -> TestResult {
    let w = world()?;
    w.write_model_golden(safety(14, 73, 7, 7))?;
    w.write_sany_golden("specifications/x/X.tla", &w.jar_sha)?; // Helper.tla: none
    let imp = Fake {
        model: safety(14, 73, 7, 7),
        oracle: false,
    };
    let report = w.run(&imp, vec![1])?;
    let modules: Vec<&str> = report.modules.iter().map(|m| m.module.as_str()).collect();
    assert!(
        modules.contains(&"specifications/x/Helper.tla"),
        "Helper.tla has no SANY golden and is missing from the report: {modules:?}"
    );
    Ok(())
}

/// C2b. With no `sany/` golden directory at all, every module is dropped and
/// the report says nothing.
#[test]
fn c2b_missing_sany_golden_dir_is_reported() -> TestResult {
    let w = world()?;
    w.write_model_golden(safety(14, 73, 7, 7))?;
    let imp = Fake {
        model: safety(14, 73, 7, 7),
        oracle: false,
    };
    let report = w.run(&imp, vec![1])?;
    assert_eq!(
        report.modules.len(),
        2,
        "both modules must appear (as oracle-error)"
    );
    Ok(())
}

/// C3. A golden that names the violated invariant agrees with a result that
/// names none, even at one worker.
#[test]
fn c3_named_property_vs_none_is_a_diff() {
    let golden = safety(14, 73, 7, 7);
    let mut observed = golden.clone();
    observed.violated_property = None;
    let ctx = CompareContext {
        workers: 1,
        counts_comparable: true,
    };
    assert!(
        !diff_results(&golden, &observed, ctx).is_empty(),
        "golden names `Inv`, observed names nothing: must not agree"
    );
}

// --------------------------------------------------------- major findings

/// M1. TLC's own out-of-memory report (captured from the pinned 94d0c50 with
/// -Xmx64m) is classified `other-error`, an oracle fact, so it is written as a
/// golden and later runs "agree" with it. A resource failure that depends on
/// -Xmx is not a fact about the spec.
#[test]
fn m1_out_of_memory_is_not_an_oracle_fact() {
    let stdout = msg(2185, 0, "Starting... (2026-10-08 13:47:41)")
        + &msg(2189, 0, "Computing initial states...")
        + &msg(
            1002,
            1,
            "Out Of Memory. There are probably too many initial states.",
        )
        + &msg(2186, 0, "Finished in 12159ms at (2026-10-08 13:47:53)");
    let r = classify_stdout(&stdout, 255);
    assert!(
        !r.verdict.is_oracle_fact(),
        "OOM classified as {:?} / {:?}",
        r.verdict,
        r.error_class
    );
    for code in [1001, 1003] {
        let r = classify_stdout(&msg(code, 1, "Out Of Memory."), 255);
        assert!(!r.verdict.is_oracle_fact(), "{code}: {:?}", r.verdict);
    }
}

/// M2. A JVM `StackOverflowError` (captured from 94d0c50: GENERAL 1000 then the
/// empty nested-expression stack 2103, exit 75) is classified
/// `evaluation-error` with error class `TLC_NESTED_EXPRESSION`, a context code.
/// The depth at which it happens depends on the JVM thread stack size (-Xss,
/// platform defaults), so this is machine-dependent.
#[test]
fn m2_stack_overflow_is_not_an_evaluation_error_fact() {
    let stdout = msg(
        1000,
        1,
        "This was a Java StackOverflowError. It was probably the result\nof an incorrect recursive function definition that caused TLC to enter\nan infinite loop when trying to compute the function or its application\nto an element in its putative domain.",
    ) + &msg(
        2103,
        1,
        "The error occurred when TLC was evaluating the nested\nexpressions at the following positions:\n    The error call stack is empty.",
    ) + &msg(
        2199,
        0,
        "0 states generated, 0 distinct states found, 0 states left on queue.",
    );
    let r = classify_stdout(&stdout, 75);
    assert_ne!(r.error_class.as_deref(), Some("TLC_NESTED_EXPRESSION"));
    assert!(!r.verdict.is_oracle_fact(), "{:?}", r.verdict);
}

/// M3. The post-run integrity check uses `git status` without `--ignored`.
/// tlaplus/Examples' .gitignore ignores `*` except a few extensions, so files
/// TLC or specs write into the corpus (the real cache already holds 125
/// `*_anim_*.svg` files written by oracle runs) are invisible to it.
#[test]
fn m3_ignored_files_written_into_corpus_are_detected() -> TestResult {
    let w = world()?;
    std::fs::write(w.corpus_root.join("Written_by_spec.svg"), "<svg/>")?;
    std::fs::write(
        w.corpus_root.join("specifications/x/X_TTrace_1.tla"),
        "---- MODULE X_TTrace_1 ----\n====\n",
    )?;
    let dirty = dive_conformance::fetch::dirty_paths(&w.corpus_root)?;
    assert!(
        !dirty.is_empty(),
        "files written into the corpus went unnoticed"
    );
    Ok(())
}

/// M4. SANY goldens are compared without checking the jar SHA-256 they were
/// generated with (model goldens check it; ADR 0003 requires it for both).
#[test]
fn m4_sany_golden_from_another_jar_is_refused() -> TestResult {
    let w = world()?;
    w.write_model_golden(safety(14, 73, 7, 7))?;
    w.write_sany_golden("specifications/x/X.tla", &"f".repeat(64))?;
    w.write_sany_golden("specifications/x/Helper.tla", &"f".repeat(64))?;
    let imp = Fake {
        model: safety(14, 73, 7, 7),
        oracle: false,
    };
    let report = w.run(&imp, vec![1])?;
    for m in &report.modules {
        assert_eq!(
            m.outcome,
            Outcome::OracleError,
            "{}: {:?}",
            m.module,
            m.outcome
        );
    }
    Ok(())
}

// --------------------------------------------------------- minor findings

/// m1. Golden paths are not injective: distinct model ids collide.
#[test]
fn minor_golden_paths_are_injective() {
    let root = Path::new("/g");
    assert_ne!(
        model_golden_path(root, "o", "c", "specifications/a b/M.cfg"),
        model_golden_path(root, "o", "c", "specifications/a_b/M.cfg"),
    );
}

/// M5. TLC's RandomElement/RandomSubset/Randomization draw from the `-seed`
/// RNG in exhaustive mode too, but the harness pins `-seed` only in
/// simulation. Examples `SpanningTree/SpanTreeRandom.cfg` (small tier) gave
/// 240806, 65396 and 106664 distinct states in three runs with the harness's
/// exact flags (the lead's fresh golden says 40460). Every TLC command
/// recorded in a committed golden must pin the seed.
#[test]
fn m5_every_golden_command_pins_the_seed() -> TestResult {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != "sany") {
                    walk(&p, out)?;
                }
            } else if p.extension().is_some_and(|x| x == "json") {
                out.push(p);
            }
        }
        Ok(())
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/golden");
    let mut files = Vec::new();
    walk(&root, &mut files)?;
    let mut unpinned = Vec::new();
    for f in &files {
        let g: ModelGolden = serde_json::from_str(&std::fs::read_to_string(f)?)?;
        if !g.provenance.command.iter().any(|a| a == "-seed") {
            unpinned.push(g.model.id.clone());
        }
    }
    assert_ne!(files, Vec::<PathBuf>::new());
    assert!(
        unpinned.is_empty(),
        "{} golden(s) without -seed, e.g. {:?}",
        unpinned.len(),
        &unpinned[..unpinned.len().min(3)]
    );
    Ok(())
}

/// M6's original negative obligation: a finite non-counterexample cannot
/// agree with a temporal violation. The public pipeline independently rejects
/// it; arbitrary endings of genuinely valid temporal words are not parity.
#[test]
fn m6_finite_temporal_non_counterexample_is_independently_rejected() -> TestResult {
    let w = world_with_config(TEMPORAL_SOURCE, TEMPORAL_CONFIG, true)?;
    let golden = temporal_word(1, false);
    w.write_model_golden(golden.clone())?;
    let mut result = golden;
    let trace = result.trace.as_mut().ok_or("trace")?;
    trace.ending = TraceEnding::Finite;
    let doc = trace.json.as_mut().ok_or("document")?;
    doc["ending"] = serde_json::json!({"kind":"behavior"});
    let spec = dive_api::trace::SpecModel::load(
        &w.corpus_root.join("specifications/x/X.tla"),
        &w.corpus_root.join("specifications/x/X.cfg"),
        &dive_api::trace::TraceOptions {
            compat_tlc: true,
            ..Default::default()
        },
    )?;
    let finite = dive_api::trace::TraceDoc::from_json(doc)?;
    assert!(matches!(
        spec.read(&finite).0.validation,
        dive_api::trace::Validation::Invalid(_)
    ));
    let imp = WordFake::new(result, false);
    let report = w.run_kind(&imp, vec![1, 4], false)?;
    assert_eq!(imp.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    for run in &report.models[0].runs {
        assert_eq!(run.outcome, Outcome::Disagree, "{run:?}");
        assert!(run.diffs.iter().any(|d| d.field == "trace.invalid"));
    }
    Ok(())
}

const TEMPORAL_SOURCE: &str = "---- MODULE X ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x\nP == <>(x=1)\n====\n";
const TEMPORAL_CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";

fn temporal_word(length: usize, invalid: bool) -> OracleResult {
    let mut result = safety(1, 2, 1, length as u64);
    result.verdict = Verdict::LivenessViolation;
    result.verdict_code = Some(2116);
    result.error_class = Some("TLC_TEMPORAL_PROPERTY_VIOLATED".into());
    result.exit_code = Some(13);
    result.exit_status = Some("VIOLATION_LIVENESS".into());
    result.violated_property = Some("P".into());
    result.error_codes = vec![2116];
    result.trace = Some(Trace {
        length: length as u64,
        ending: TraceEnding::Lasso { back_to: Some(1) },
        json: Some(serde_json::json!({
        "format":"dive-trace", "version":1, "variables":["x"],
        "states":(0..length).map(|_|serde_json::json!({"values":{"x":u8::from(invalid)}})).collect::<Vec<_>>(),
        "ending":{"kind":"lasso","back_to":0}, "length":length, "properties":["P"]
        })),
        steps: None,
        omitted: None,
        json_unavailable: None,
    });
    result
}

struct WordFake {
    result: OracleResult,
    calls: std::sync::atomic::AtomicUsize,
    unchecked_yes: bool,
    worker_result: Option<OracleResult>,
}
impl WordFake {
    fn new(result: OracleResult, unchecked_yes: bool) -> Self {
        Self {
            result,
            calls: std::sync::atomic::AtomicUsize::new(0),
            unchecked_yes,
            worker_result: None,
        }
    }
}
impl Implementation for WordFake {
    fn name(&self) -> String {
        "word-control".into()
    }
    fn is_oracle(&self) -> bool {
        false
    }
    fn profile(&self) -> Option<Profile> {
        self.worker_result.as_ref().map(|_| Profile::Tlc)
    }
    fn check_model(&self, req: &ModelRequest<'_>) -> DResult<ImplResult<OracleResult>> {
        Ok(ImplResult::Completed {
            result: self
                .worker_result
                .as_ref()
                .filter(|_| req.workers > 1)
                .unwrap_or(&self.result)
                .clone(),
            command: vec![],
            elapsed: Duration::ZERO,
        })
    }
    fn parse_module(&self, _: &Path, _: &str, _: Duration) -> DResult<ImplResult<SanyResult>> {
        Ok(ImplResult::Unsupported("unused".into()))
    }
    fn trace_validated(
        &self,
        req: &ModelRequest<'_>,
        result: &OracleResult,
        golden: &OracleResult,
    ) -> dive_conformance::trace_check::TraceCheck {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.unchecked_yes {
            return dive_conformance::trace_check::TraceCheck::yes("deliberately unchecked hook");
        }
        match dive_conformance::trace_check::load_model(req.corpus_root, req.model, &[], true) {
            Ok(spec) => {
                dive_conformance::trace_check::validate_counterexample(&spec, result, golden)
            }
            Err(error) => dive_conformance::trace_check::TraceCheck::no(error),
        }
    }
}

#[test]
fn temporal_documents_and_independent_validation_are_required_by_public_pipeline() -> TestResult {
    for comparable in [true, false] {
        let w = world_with_config(TEMPORAL_SOURCE, TEMPORAL_CONFIG, comparable)?;
        let golden = temporal_word(1, false);
        w.write_model_golden(golden.clone())?;
        let mut cases = Vec::new();
        let mut missing = golden.clone();
        missing.trace = None;
        cases.push(missing);
        let mut unavailable = golden.clone();
        if let Some(trace) = &mut unavailable.trace {
            trace.json = None;
            trace.json_unavailable = Some("control unavailable".into());
        }
        cases.push(unavailable);
        let mut no_json = golden.clone();
        if let Some(trace) = &mut no_json.trace {
            trace.json = None;
        }
        cases.push(no_json);
        for result in cases {
            for unchecked_yes in [false, true] {
                let imp = WordFake::new(result.clone(), unchecked_yes);
                let report = w.run_kind(&imp, vec![1, 4], false)?;
                assert_eq!(report.models.len(), 1);
                assert_eq!(report.models[0].model, "specifications/x/X.cfg");
                assert_eq!(imp.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
                for run in &report.models[0].runs {
                    assert_eq!(run.outcome, Outcome::AgreePendingTraceValidation, "{run:?}");
                    assert!(run.diffs.iter().any(|d| d.field == "trace.validated"));
                }
                assert_eq!(report.summary.models.get(&Outcome::Agree).copied(), Some(0));
            }
        }
        let mut malformed = golden.clone();
        if let Some(trace) = &mut malformed.trace {
            trace.json = Some(serde_json::json!({"not":"a word"}));
        }
        let unvalidated = WordFake::new(malformed, false);
        let report = w.run_kind(&unvalidated, vec![1, 4], false)?;
        assert!(
            report.models[0]
                .runs
                .iter()
                .all(|r| r.outcome != Outcome::Agree)
        );
        let invalid = WordFake::new(temporal_word(2, true), false);
        let report = w.run_kind(&invalid, vec![1, 4], false)?;
        assert!(
            report.models[0]
                .runs
                .iter()
                .all(|r| r.outcome == Outcome::Disagree
                    && r.diffs.iter().any(|d| d.field == "trace.invalid"))
        );
        let valid = WordFake::new(temporal_word(2, false), false);
        let report = w.run_kind(&valid, vec![1, 4], false)?;
        assert_eq!(valid.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert!(
            report.models[0]
                .runs
                .iter()
                .all(|r| r.outcome == Outcome::Agree),
            "{report:?}"
        );
    }
    Ok(())
}

#[test]
fn temporal_exit_is_exact_after_independent_public_pipeline_replay() -> TestResult {
    for comparable in [true, false] {
        let w = world_with_config(TEMPORAL_SOURCE, TEMPORAL_CONFIG, comparable)?;
        let golden = temporal_word(1, false);
        w.write_model_golden(golden.clone())?;
        for stutter in [false, true] {
            for exit in [Some(13), Some(0), None, Some(12)] {
                let mut observed = temporal_word(3, false);
                if stutter {
                    let trace = observed.trace.as_mut().ok_or("trace")?;
                    trace.ending = TraceEnding::Stuttering;
                    trace.json.as_mut().ok_or("document")?["ending"] =
                        serde_json::json!({"kind":"stuttering"});
                }
                observed.exit_code = exit;
                observed.exit_status = exit.map(|code| {
                    match code {
                        0 => "SUCCESS",
                        12 => "VIOLATION_SAFETY",
                        _ => "VIOLATION_LIVENESS",
                    }
                    .into()
                });
                let imp = WordFake::new(observed, false);
                let report = w.run_kind(&imp, vec![1, 4], false)?;
                assert_eq!(imp.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
                assert_eq!(report.models.len(), 1);
                for run in &report.models[0].runs {
                    assert_eq!(
                        run.outcome,
                        if exit == Some(13) {
                            Outcome::Agree
                        } else {
                            Outcome::Disagree
                        },
                        "{run:?}"
                    );
                    if exit == Some(13) {
                        assert_eq!(run.diffs, Vec::<FieldDiff>::new());
                    } else {
                        assert_eq!(
                            run.diffs.len(),
                            1,
                            "valid word must not hide exact exit: {run:?}"
                        );
                        assert_eq!(run.diffs[0].field, "exit_code");
                        assert_eq!(run.diffs[0].expected, serde_json::json!(13));
                        assert_eq!(run.diffs[0].actual, serde_json::json!(exit));
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn worker_golden_temporal_exit_is_exact_after_independent_replay() -> TestResult {
    let source = TEMPORAL_SOURCE
        .replace("EXTENDS Naturals", "EXTENDS Naturals,TLC")
        .replace("====", "Unused == TLCGet(\"config\").worker\n====");
    for comparable in [true, false] {
        let w = world_with_config(&source, TEMPORAL_CONFIG, comparable)?;
        let single = temporal_word(1, false);
        let mut many = temporal_word(1, false);
        many.exit_code = Some(12);
        many.exit_status = Some("VIOLATION_SAFETY".into());
        w.write_model_golden(single.clone())?;
        w.write_golden_at(many.clone(), 4, Some("actual worker-path control"))?;
        for exit in [Some(12), Some(0), None, Some(13)] {
            let mut observed_many = temporal_word(3, false);
            observed_many.exit_code = exit;
            observed_many.exit_status = exit.map(|_| "worker control".into());
            // Preserve the existing approximate-count/depth worker policy.
            observed_many.counts = Counts {
                distinct_states: Some(17),
                states_generated: Some(19),
                depth: Some(99),
                ..Counts::default()
            };
            let mut imp = WordFake::new(single.clone(), false);
            imp.worker_result = Some(observed_many);
            let report = w.run_kind(&imp, vec![1, 4], false)?;
            assert!(report.models[0].worker_introspecting);
            assert_eq!(imp.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
            assert_eq!(report.models[0].runs[0].outcome, Outcome::Agree);
            let run = &report.models[0].runs[1];
            assert_eq!(
                run.outcome,
                if exit == Some(12) {
                    Outcome::Agree
                } else {
                    Outcome::Disagree
                },
                "{run:?}"
            );
            assert!(
                run.detail
                    .as_ref()
                    .is_some_and(|note| note.contains("TLC at 4 workers"))
            );
            if exit == Some(12) {
                assert_eq!(run.diffs, Vec::<FieldDiff>::new());
            } else {
                assert_eq!(run.diffs.len(), 1);
                assert_eq!(run.diffs[0].field, "exit_code");
            }
        }
    }
    Ok(())
}

#[test]
fn finite_temporal_exit_twelve_retains_pinned_classifier_and_pipeline_positive() -> TestResult {
    let capture = include_str!("fixtures/temporal-prefix/tlc-2026.10.06.014338/QtEmpty.stdout");
    let classified = classify_stdout(capture, 12);
    assert_eq!(classified.verdict, Verdict::LivenessViolation);
    assert_eq!(classified.exit_code, Some(12));
    assert_eq!(
        classified.trace.as_ref().ok_or("trace")?.ending,
        TraceEnding::Finite
    );
    assert_eq!(classify_stdout(capture, 0).verdict, Verdict::OracleError);
    let source = include_str!("fixtures/temporal-prefix/specs/QtEmpty/QtEmpty.tla")
        .replace("MODULE QtEmpty", "MODULE X");
    let config = include_str!("fixtures/temporal-prefix/specs/QtEmpty/QtEmpty.cfg");
    for comparable in [true, false] {
        let w = world_with_config(&source, config, comparable)?;
        let mut golden = classified.clone();
        // The existing temporal transport interprets the finite report's
        // absent back edge as stuttering; this actual Next self-edge is valid.
        golden.trace.as_mut().ok_or("trace")?.json = Some(serde_json::json!({
            "format":"dive-trace", "version":1, "variables":["x"],
            "states":[{"values":{"x":0}}], "length":1,
            "ending":{"kind":"stuttering"}, "properties":["P"]
        }));
        w.write_model_golden(golden.clone())?;
        for exit in [Some(12), Some(0), None, Some(13)] {
            let mut observed = golden.clone();
            observed.exit_code = exit;
            let imp = WordFake::new(observed, false);
            let report = w.run_kind(&imp, vec![1, 4], false)?;
            assert_eq!(imp.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
            for run in &report.models[0].runs {
                assert_eq!(
                    run.outcome,
                    if exit == Some(12) {
                        Outcome::Agree
                    } else {
                        Outcome::Disagree
                    },
                    "{run:?}"
                );
                if exit != Some(12) {
                    assert_eq!(run.diffs.len(), 1);
                    assert_eq!(run.diffs[0].field, "exit_code");
                }
            }
        }
    }
    Ok(())
}

// ------------------------------------------ worker-introspecting models

/// A spec that reads `TLCGet("config")`, like Examples `MCDieHardest`.
const INTROSPECTING: &str =
    "---- MODULE X ----\nEXTENDS TLC\nASSUME TLCGet(\"config\").worker = 1\n====\n";

fn success(distinct: u64) -> OracleResult {
    OracleResult {
        verdict: Verdict::Success,
        verdict_code: None,
        error_class: None,
        exit_code: Some(0),
        exit_status: Some("SUCCESS".to_owned()),
        violated_property: None,
        trace: None,
        error_codes: vec![],
        counts: Counts {
            distinct_states: Some(distinct),
            states_generated: Some(distinct),
            states_left_on_queue: Some(0),
            depth: Some(1),
        },
        ..safety(0, 0, 0, 0)
    }
}

fn assumption_false() -> OracleResult {
    OracleResult {
        verdict: Verdict::AssumptionFailure,
        verdict_code: Some(2104),
        error_class: Some("TLC_ASSUMPTION_FALSE".to_owned()),
        exit_code: Some(12),
        exit_status: Some("VIOLATION_ASSUMPTION".to_owned()),
        violated_property: None,
        trace: None,
        error_codes: vec![2104],
        counts: Counts::default(),
        ..safety(0, 0, 0, 0)
    }
}

/// Behaves as TLC (and `dive --compat=tlc`) does on [`INTROSPECTING`]: the
/// assumption fails above one worker. Under the default profile the logical
/// count is 1, so it never fails.
struct WorkerFake {
    profile: Option<Profile>,
}

impl Implementation for WorkerFake {
    fn name(&self) -> String {
        "fake".to_owned()
    }
    fn is_oracle(&self) -> bool {
        false
    }
    fn profile(&self) -> Option<Profile> {
        self.profile.clone()
    }
    fn check_model(&self, req: &ModelRequest<'_>) -> DResult<ImplResult<OracleResult>> {
        let physical = matches!(self.profile, Some(Profile::Tlc));
        Ok(ImplResult::Completed {
            result: if physical && req.workers > 1 {
                // Counts as only approximate as TLC's: never compared.
                OracleResult {
                    counts: Counts {
                        distinct_states: Some(99),
                        ..Counts::default()
                    },
                    ..assumption_false()
                }
            } else {
                success(5)
            },
            command: vec![],
            elapsed: Duration::ZERO,
        })
    }
    fn parse_module(&self, _r: &Path, _m: &str, _t: Duration) -> DResult<ImplResult<SanyResult>> {
        Ok(ImplResult::Completed {
            result: accept(),
            command: vec![],
            elapsed: Duration::ZERO,
        })
    }
}

fn introspecting_world() -> Result<World, Box<dyn std::error::Error>> {
    let w = world_with(INTROSPECTING)?;
    w.write_model_golden(success(5))?;
    w.write_sany_golden("specifications/x/X.tla", &w.jar_sha)?;
    w.write_sany_golden("specifications/x/Helper.tla", &w.jar_sha)?;
    Ok(w)
}

/// Issue #55 (ADR 0004, amendment of 2026-10-09): under `--compat=tlc` a
/// worker-introspecting model at N > 1 workers is compared with TLC at N
/// workers, on verdict, error class and numeric exit code.
#[test]
fn compat_worker_introspecting_runs_compare_with_tlc_at_the_same_count() -> TestResult {
    let w = introspecting_world()?;
    for n in [3, 4, 8] {
        w.write_golden_at(
            OracleResult {
                counts: Counts {
                    distinct_states: Some(7),
                    ..Counts::default()
                },
                ..assumption_false()
            },
            n,
            Some("test"),
        )?;
    }
    let report = w.run(
        &WorkerFake {
            profile: Some(Profile::Tlc),
        },
        vec![1, 3, 4, 8],
    )?;
    let m = &report.models[0];
    assert!(m.worker_introspecting);
    assert_eq!(m.outcome, Outcome::Agree, "{:?}", m.runs);
    for r in &m.runs {
        assert_eq!(r.outcome, Outcome::Agree, "{r:?}");
        if r.workers > 1 {
            assert!(
                r.detail
                    .as_deref()
                    .is_some_and(|d| d.contains("verdict, error class and exit code")),
                "{r:?}"
            );
        }
    }
    // The single-worker golden is untouched and still applies at 1.
    assert_eq!(m.runs[0].verdict, Some(Verdict::Success));
    Ok(())
}

/// Without a worker-specific golden the single-worker golden applies, on
/// every field.
#[test]
fn compat_without_a_worker_golden_compares_with_the_single_worker_one() -> TestResult {
    let w = introspecting_world()?;
    let report = w.run(
        &WorkerFake {
            profile: Some(Profile::Tlc),
        },
        vec![1, 4],
    )?;
    assert_eq!(report.models[0].outcome, Outcome::Disagree);
    Ok(())
}

/// The default profile reports the logical worker count, 1: the
/// single-worker golden applies at every count, even with worker-specific
/// goldens present.
#[test]
fn default_profile_ignores_worker_goldens() -> TestResult {
    let w = introspecting_world()?;
    w.write_golden_at(assumption_false(), 4, Some("test"))?;
    let report = w.run(&WorkerFake { profile: None }, vec![1, 4])?;
    assert_eq!(report.models[0].outcome, Outcome::Agree);
    // A run that failed the assumption at 4 workers would disagree there.
    let report = w.run(
        &WorkerFake {
            profile: Some(Profile::Dive),
        },
        vec![4],
    )?;
    assert_eq!(report.models[0].outcome, Outcome::Agree);
    Ok(())
}

/// A worker-specific golden without a recorded reason is refused.
#[test]
fn worker_golden_needs_a_reason() -> TestResult {
    let w = introspecting_world()?;
    w.write_golden_at(assumption_false(), 4, None)?;
    let report = w.run(
        &WorkerFake {
            profile: Some(Profile::Tlc),
        },
        vec![4],
    )?;
    let m = &report.models[0];
    assert_eq!(m.outcome, Outcome::OracleError, "{:?}", m.runs);
    assert!(
        m.runs[0]
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("--reason")),
        "{:?}",
        m.runs
    );
    Ok(())
}

/// A different verdict or error class than TLC's at that count disagrees.
#[test]
fn compat_worker_golden_still_compares_the_verdict() -> TestResult {
    let w = introspecting_world()?;
    w.write_golden_at(success(5), 4, Some("test"))?;
    let report = w.run(
        &WorkerFake {
            profile: Some(Profile::Tlc),
        },
        vec![4],
    )?;
    assert_eq!(report.models[0].outcome, Outcome::Disagree);
    Ok(())
}

#[test]
fn actual_emitted_prefix_metadata_cannot_gain_public_agreement() -> TestResult {
    use dive_conformance::dive::{CheckMapping, check_result};
    let source = TEMPORAL_SOURCE
        .replace("Next == x'=x", "Next == x'=(x+1)%4")
        .replace("P == <>(x=1)", "P == <>(x=3)");
    let w = world_with_config(&source, TEMPORAL_CONFIG, true)?;
    let mut golden = temporal_word(1, false);
    golden.exit_code = Some(10);
    golden.counts = Counts {
        distinct_states: Some(2),
        states_generated: Some(2),
        states_left_on_queue: Some(1),
        depth: Some(2),
    };
    w.write_model_golden(golden)?;
    for workers in [1, 4] {
        let actual = dive_api::check_file(&dive_api::CheckRequest {
            file: w.corpus_root.join("specifications/x/X.tla"),
            workers: Some(workers),
            seed: Some(0),
            fp: Some(0),
            ..Default::default()
        })?
        .to_json();
        assert_eq!(actual["execution"]["completed_level"], 1);
        assert_eq!(actual["statistics"]["left_on_queue"], 1);
        for mutation in ["original", "final-prefix", "false-level"] {
            let mut raw = actual.clone();
            match mutation {
                "final-prefix" => raw["options"]["lncheck"] = serde_json::json!("final"),
                "false-level" => raw["execution"]["completed_level"] = serde_json::json!(999_999),
                _ => {}
            }
            let process = dive_conformance::process::ProcessOutcome {
                exit_code: Some(10),
                signal: None,
                timed_out: false,
                stdout: raw.to_string(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
            };
            let CheckMapping::Result(mapped) = check_result(&process, false) else {
                return Err("expected mapped native report".into());
            };
            // The existing honest hook independently binds/replays this pure
            // fixture. It never trusts the producer's JSON validation flag.
            let imp = WordFake::new(*mapped, false);
            let report = w.run_kind(&imp, vec![u32::try_from(workers)?], false)?;
            let run = &report.models[0].runs[0];
            assert_eq!(
                run.outcome,
                if mutation == "original" {
                    Outcome::Agree
                } else {
                    Outcome::Disagree
                },
                "{mutation}: {run:?}"
            );
            if mutation == "original" {
                assert_eq!(imp.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            }
        }
    }
    Ok(())
}
