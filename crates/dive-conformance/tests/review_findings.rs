//! Adversarial-review regression tests. Each test demonstrates one finding;
//! every test here FAILS on 0eb93d5 and should pass once the finding is fixed.

use dive_conformance::cache::Cache;
use dive_conformance::compare::{CompareContext, Outcome, diff_results};
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
        native_unreduced_symmetry: false,
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
    let tmp = dive_conformance::util::TempDir::new("review")?;
    // Corpus repository.
    let src = tmp.path().join("src");
    let spec = src.join("specifications/x");
    std::fs::create_dir_all(&spec)?;
    std::fs::write(
        src.join(".gitignore"),
        "*\n!*/\n!*.tla\n!*.cfg\n!manifest.json\n!.gitignore\n*_TTrace_*.tla\n",
    )?;
    std::fs::write(spec.join("manifest.json"), MANIFEST)?;
    std::fs::write(spec.join("X.tla"), x_tla)?;
    std::fs::write(spec.join("X.cfg"), "\n")?;
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
                mode: ModelMode::Exhaustive,
                features: vec![],
                counts_comparable: true,
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
                budget_cap: None,
            },
            crosscheck_manifest: false,
            models: true,
            sany: true,
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

/// M6. The way a counterexample ends is in the golden but never compared: a
/// liveness golden with a lasso agrees with a finite (non-)counterexample of
/// the same length, and the harness has no trace-validation hook to catch it.
#[test]
fn m6_trace_ending_is_compared() {
    let mut golden = safety(3, 4, 3, 3);
    golden.verdict = Verdict::LivenessViolation;
    golden.verdict_code = Some(2116);
    golden.error_class = Some("TLC_TEMPORAL_PROPERTY_VIOLATED".to_owned());
    if let Some(t) = golden.trace.as_mut() {
        t.ending = TraceEnding::Lasso { back_to: Some(1) };
    }
    let mut observed = golden.clone();
    if let Some(t) = observed.trace.as_mut() {
        t.ending = TraceEnding::Finite;
    }
    let ctx = CompareContext {
        workers: 1,
        counts_comparable: true,
    };
    assert_ne!(diff_results(&golden, &observed, ctx), Vec::new());
}

// ------------------------------------------ worker-introspecting models

/// A spec that reads `TLCGet("config")`, like Examples `MCDieHardest`.
const INTROSPECTING: &str =
    "---- MODULE X ----\nEXTENDS TLC\nASSUME TLCGet(\"config\").worker = 1\n====\n";

fn success(distinct: u64) -> OracleResult {
    OracleResult {
        native_unreduced_symmetry: false,
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
        native_unreduced_symmetry: false,
        verdict: Verdict::AssumptionFailure,
        verdict_code: Some(2104),
        error_class: Some("TLC_ASSUMPTION_FALSE".to_owned()),
        exit_code: Some(10),
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
    worker_exit: Option<i32>,
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
                    exit_code: self.worker_exit,
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
/// workers, on verdict, error class and exact physical numeric exit.
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
            worker_exit: Some(10),
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
                    .is_some_and(|d| d.contains("verdict, error class and numeric exit")),
                "{r:?}"
            );
        }
    }
    // The single-worker golden is untouched and still applies at 1.
    assert_eq!(m.runs[0].verdict, Some(Verdict::Success));
    Ok(())
}

/// Numeric process exits are exact even where counts are intentionally
/// omitted for a worker-introspecting model. The deciding assumption fixture
/// is read from the pinned primary's actual worker-specific goldens.
#[test]
fn worker_specific_numeric_exits_are_exact_while_counts_are_ignored() -> TestResult {
    let w = introspecting_world()?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = repo.join("conformance/golden");
    for workers in [3, 4, 8] {
        let path = worker_golden_path(
            &root,
            ORACLE,
            "examples",
            "specifications/DieHard/MCDieHardest.cfg",
            workers,
        );
        let pinned: ModelGolden = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(pinned.workers, workers);
        assert_eq!(pinned.result.verdict, Verdict::AssumptionFailure);
        assert_eq!(pinned.result.verdict_code, Some(2104));
        assert_eq!(
            pinned.result.error_class.as_deref(),
            Some("TLC_ASSUMPTION_FALSE")
        );
        assert_eq!(pinned.result.exit_code, Some(10));
        assert_eq!(dive_conformance::tlc::codes::exit_statuses_for(2104), &[10]);
        assert_ne!(pinned.result.counts.distinct_states, Some(99));
        w.write_golden_at(
            pinned.result,
            workers,
            Some("pinned assumption exit fixture"),
        )?;
    }
    for exit in [
        Some(10),
        Some(11),
        None,
        Some(0),
        Some(12),
        Some(77),
        Some(255),
    ] {
        let report = w.run(
            &WorkerFake {
                profile: Some(Profile::Tlc),
                worker_exit: exit,
            },
            vec![3, 4, 8],
        )?;
        let expected = if exit == Some(10) {
            Outcome::Agree
        } else {
            Outcome::Disagree
        };
        let model = &report.models[0];
        assert!(model.worker_introspecting);
        assert_eq!(model.outcome, expected);
        assert_eq!(model.runs.len(), 3);
        for row in &model.runs {
            assert_eq!(row.outcome, expected, "{row:?}");
            assert_eq!(row.exit_code, exit);
            assert_eq!(row.verdict, Some(Verdict::AssumptionFailure));
            assert_eq!(row.verdict_code, Some(2104));
            assert_eq!(row.distinct_states, Some(99));
            if exit == Some(10) {
                assert_eq!(
                    row.diffs,
                    Vec::<dive_conformance::compare::FieldDiff>::new()
                );
            } else {
                assert_eq!(row.diffs.len(), 1, "{row:?}");
                assert_eq!(row.diffs[0].field, "exit_code");
                assert_eq!(row.diffs[0].expected, serde_json::json!(10));
                assert_eq!(row.diffs[0].actual, serde_json::json!(exit));
            }
        }
    }
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
            worker_exit: Some(10),
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
    let report = w.run(
        &WorkerFake {
            profile: None,
            worker_exit: Some(10),
        },
        vec![1, 4],
    )?;
    assert_eq!(report.models[0].outcome, Outcome::Agree);
    // A run that failed the assumption at 4 workers would disagree there.
    let report = w.run(
        &WorkerFake {
            profile: Some(Profile::Dive),
            worker_exit: Some(10),
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
            worker_exit: Some(10),
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
            worker_exit: Some(10),
        },
        vec![4],
    )?;
    assert_eq!(report.models[0].outcome, Outcome::Disagree);
    Ok(())
}
