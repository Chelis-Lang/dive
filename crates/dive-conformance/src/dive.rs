//! `dive` itself as an implementation under test.
//!
//! The harness runs the `dive` binary as a black box, exactly as a user
//! would, and maps its JSON output into the oracle vocabulary:
//! - full SANY goldens are compared with `dive analyze --output json`;
//! - parse-phase verdicts (`run --sany-phase parse`) with
//!   `dive parse --output json`;
//! - models are checked with `dive check --output json`, and its report is
//!   mapped to the oracle vocabulary ([`check_result`]): the deciding TLC
//!   message code dive reports gives the verdict and error class, the
//!   complete temporal capture statistics remain observable even where TLC's
//!   print marker is false; ordinary safety statistics follow that marker. The
//!   counterexample's length and printed states are kept. A refusal, a
//!   resource limit, or an error whose TLC class dive does not claim yet is
//!   `unsupported`, naming it. dive's counterexample document
//!   (`counterexample.trace`) is validated by the harness itself with
//!   `dive-trace`, in-process ([`crate::trace_check`]); a violation whose
//!   counterexample the validator cannot confirm is at best
//!   `agree-pending-trace-validation` (ADR 0016).
//!
//! **Module search path.** SANY goldens are generated with the oracle's
//! classpath: `tla2tools.jar`, the classpath jars, then the module
//! libraries. SANY finds a module in the root file's directory, then in
//! `tla2sany/StandardModules` inside `tla2tools.jar`, then in the rest of
//! the classpath in order. So dive gets every later classpath entry as a
//! `--module-path` (searched after its standard modules), in classpath
//! order:
//! - each classpath jar of the golden oracle contributes the `.tla` files at
//!   its root, extracted once into the cache (`jar-modules/<sha256>/`). The
//!   `CommunityModules` jar provides `SVG`, `IOUtils`, `Json` and the rest;
//!   jars without `.tla` files contribute nothing;
//! - each `[[module_library]]` contributes its verified directory (Apalache
//!   `src/tla`, then the TLAPS `library/`).
//!
//! Standard modules thereby win over the TLAPS library's own `Bags`,
//! `FiniteSets` and `RealTime`, as on SANY's classpath (ADR 0011).
//!
//! **Profiles (ADR 0010).** The profile is recorded in every report.
//! `dive parse` and `dive analyze` have no profile-dependent behaviour yet;
//! the harness still runs `dive` with `DIVE_COMPAT=tlc` under the pure `tlc`
//! profile, and with `DIVE_COMPAT` removed otherwise, so the profile reaches
//! `dive` once it matters.

use crate::cache::Cache;
use crate::config::Config;
use crate::corpus::ModelMode;
use crate::divergence::Profile;
use crate::error::{Error, Result};
use crate::hash::sha256_file;
use crate::implementation::{ImplOption, ImplResult, Implementation, ModelRequest, ParseResult};
use crate::oracle::{ClasspathEntry, library_entry};
use crate::process::{ProcessOutcome, run_with_timeout};
use crate::sany::{ParseVerdict, SanyResult, SanyVerdict};
use crate::tlc::codes::{ec_name, exit_status_name, verdict_for_code};
use crate::tlc::result::{
    CollisionProbability, Counts, OracleResult, Trace, TraceEnding, TraceStep, Verdict,
};
use crate::util::{TempDir, capture, redact};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

mod ordinary_final;

/// The `dive analyze` / `dive parse` JSON schema version the harness reads.
pub const DIVE_JSON_SCHEMA: u64 = 2;

/// Exit code `dive` uses for a recognised but unimplemented request.
const DIVE_EXIT_UNSUPPORTED: i32 = 3;
/// `dive check`'s exit status for a run its budget stopped (ADR 0107).
const DIVE_EXIT_STOPPED: i32 = 5;

/// What a report records about the `dive` binary under test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiveProvenance {
    /// The binary's path, repository-relative when inside the repository.
    pub binary: String,
    /// SHA-256 of the binary.
    pub sha256: String,
    /// `dive --version`.
    pub version: String,
    /// The `--module-path` entries, in order (ids and pins of the classpath
    /// jars and module libraries they come from).
    pub module_path: Vec<ClasspathEntry>,
}

/// `dive` itself.
#[derive(Debug, Clone)]
pub struct DiveImplementation {
    /// Options `dive` runs under (`--impl-option`).
    pub options: Vec<ImplOption>,
    /// The profile (`--impl-profile`, or `DIVE_COMPAT=tlc`).
    pub profile: Profile,
    bin: PathBuf,
    module_path: Vec<ClasspathEntry>,
    provenance: Option<DiveProvenance>,
    tmp_root: PathBuf,
    cache_root: PathBuf,
}

/// The module search path equivalent to the classpath `golden_oracle`'s
/// goldens were generated with (see the module documentation), in order.
///
/// # Errors
/// Fails if a classpath jar or module library is not in the cache (run
/// `oracle fetch`).
pub fn module_search_path(
    config: &Config,
    cache: &Cache,
    golden_oracle: &str,
) -> Result<Vec<ClasspathEntry>> {
    let oracle = config.oracles.oracle(golden_oracle)?;
    let mut module_path = Vec::new();
    for id in &oracle.classpath {
        let a = config.oracles.classpath_artifact(id)?;
        let jar = cache.verified_artifact(&format!("classpath jar `{id}`"), &a.url, &a.sha256)?;
        if let Some(dir) = jar_modules(cache, &jar, &a.sha256)? {
            module_path.push(ClasspathEntry {
                id: a.id.clone(),
                pin: a.sha256.clone(),
                path: dir,
            });
        }
    }
    for lib in &config.oracles.module_library {
        module_path.push(library_entry(cache, lib)?);
    }
    Ok(module_path)
}

// Resolve once, before model/parse subprocesses change their working directory.
// The caller's path is retained separately for repository-relative provenance.
fn resolve_binary(bin: &Path) -> Result<PathBuf> {
    if !bin.is_file() {
        return Err(Error::Config(format!(
            "the dive binary {} does not exist; build it with `cargo build --release -p dive-cli` or pass --dive-bin",
            bin.display()
        )));
    }
    bin.canonicalize().map_err(|e| Error::io(bin, &e))
}

impl DiveImplementation {
    /// Resolves the `dive` binary and the module search path equivalent to
    /// the classpath that `golden_oracle`'s SANY goldens were generated
    /// with.
    ///
    /// # Errors
    /// Fails if the binary is missing or does not run, or if a classpath
    /// jar or module library is not in the cache (run `oracle fetch`).
    pub fn prepare(
        config: &Config,
        cache: &Cache,
        golden_oracle: &str,
        bin: &Path,
    ) -> Result<Self> {
        let module_path = module_search_path(config, cache, golden_oracle)?;
        let executable = resolve_binary(bin)?;
        let version = capture(Command::new(&executable).arg("--version"))?
            .trim()
            .to_owned();
        let shown = bin
            .canonicalize()
            .ok()
            .and_then(|b| {
                config
                    .repo_root
                    .canonicalize()
                    .ok()
                    .and_then(|r| b.strip_prefix(r).ok().map(Path::to_path_buf))
            })
            .map_or_else(|| bin.display().to_string(), |p| p.display().to_string());
        let provenance = DiveProvenance {
            binary: shown,
            sha256: sha256_file(&executable)?,
            version,
            module_path: module_path.clone(),
        };
        Ok(Self {
            options: Vec::new(),
            profile: Profile::default(),
            bin: executable,
            module_path,
            provenance: Some(provenance),
            tmp_root: cache.tmp_dir(),
            cache_root: cache.root().to_path_buf(),
        })
    }

    /// Raw black-box model run. Final/short observation is assessed separately
    /// from the strict schema-1 conformance mapping.
    pub(crate) fn run_model(
        &self,
        req: &ModelRequest<'_>,
        final_short: bool,
    ) -> Result<(Vec<String>, ProcessOutcome)> {
        let cfg = req.corpus_root.join(&req.model.id);
        let workers = req.workers.to_string();
        // With `--tlc-budgets` the run gets its own budget (ADR 0107) and
        // stops itself with a structured result; the harness kills it only
        // `KILL_GRACE` later. Both profiles use the explicit host budget;
        // compat reports it as `tlc+overrides`. Without it, dive runs with
        // no budget of its own, exactly as before.
        let (time_limit, max_memory) = if req.budgets {
            (
                format!("{}ms", req.timeout.as_millis().max(1)),
                req.xmx.to_owned(),
            )
        } else {
            ("none".to_owned(), "none".to_owned())
        };
        // TLC's goldens run with `-fp 0 -seed 0` (`oracle::run_tlc`).
        let mut extra: Vec<&std::ffi::OsStr> = vec![
            "--config".as_ref(),
            cfg.as_os_str(),
            "--workers".as_ref(),
            workers.as_ref(),
            "--seed".as_ref(),
            "0".as_ref(),
            "--fp".as_ref(),
            "0".as_ref(),
            "--time-limit".as_ref(),
            time_limit.as_ref(),
            "--max-memory".as_ref(),
            max_memory.as_ref(),
        ];
        for p in &self.module_path {
            extra.push("--module-path".as_ref());
            extra.push(p.path.as_os_str());
        }
        // TLC runs these with `-Dtlc2.tool.impl.Tool.cdot=true`
        // (`oracle::run_tlc`); the default profile composes actions anyway
        // (D-022).
        if self.profile != Profile::Dive
            && req.model.features.iter().any(|f| f == "action composition")
        {
            extra
                .extend::<[&std::ffi::OsStr; 2]>(["--action-composition".as_ref(), "tlc".as_ref()]);
        }
        if final_short {
            extra.extend::<[&std::ffi::OsStr; 4]>([
                "--lncheck".as_ref(),
                "final".as_ref(),
                "--lasso".as_ref(),
                "short".as_ref(),
            ]);
        }
        self.run(
            "check",
            &extra,
            req.corpus_root,
            &req.model.module,
            if req.budgets {
                req.timeout.saturating_add(crate::pipeline::KILL_GRACE)
            } else {
                req.timeout
            },
        )
    }

    /// Runs `dive <command> --output json [extra] <file>` from the module's
    /// directory in the corpus checkout. `dive parse` and `dive analyze` only
    /// read files, and the harness checks afterwards that the checkout is
    /// unmodified.
    fn run(
        &self,
        command: &str,
        extra: &[&std::ffi::OsStr],
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<(Vec<String>, ProcessOutcome)> {
        let path = corpus_root.join(module);
        let dir = path
            .parent()
            .ok_or_else(|| Error::Config(format!("module `{module}` has no directory")))?;
        let file = path
            .file_name()
            .ok_or_else(|| Error::Config(format!("module `{module}` has no file name")))?;
        let scratch = TempDir::new_in(&self.tmp_root, "dive")?;
        let mut cmd = Command::new(&self.bin);
        cmd.current_dir(dir)
            .env_remove("DIVE_COMPAT")
            .args([command, "--output", "json"])
            .args(extra)
            .arg(file);
        if self.profile == Profile::Tlc {
            cmd.env("DIVE_COMPAT", "tlc");
        }
        let reps = [
            (scratch.path().to_path_buf(), "$TMP"),
            (self.cache_root.clone(), "$DIVE_CACHE_DIR"),
            (dir.to_path_buf(), "$CWD"),
        ];
        let shown: Vec<String> = std::iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(|a| redact(&a.to_string_lossy(), &reps))
            .collect();
        let mut outcome = run_with_timeout(&mut cmd, timeout, scratch.path())?;
        outcome.stdout = redact(&outcome.stdout, &reps);
        outcome.stderr = redact(&outcome.stderr, &reps);
        Ok((shown, outcome))
    }
}

/// Extracts the `.tla` files at the root of a verified jar into
/// `jar-modules/<sha256>/` in the cache, once, and returns that directory,
/// or `None` if the jar holds no root-level `.tla` file. Java resolves a
/// module as the resource `<Name>.tla` at the classpath root, so only those
/// files are visible to SANY.
///
/// # Errors
/// Fails if `unzip` is missing or fails.
pub fn jar_modules(cache: &Cache, jar: &Path, sha256: &str) -> Result<Option<PathBuf>> {
    let names: Vec<String> = capture(Command::new("unzip").arg("-Z1").arg(jar))?
        .lines()
        .filter(|n| {
            Path::new(n)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("tla"))
                && !n.contains('/')
        })
        .map(str::to_owned)
        .collect();
    if names.is_empty() {
        return Ok(None);
    }
    let dir = cache.root().join("jar-modules").join(sha256);
    let marker = dir.join(".complete");
    if marker.is_file() {
        return Ok(Some(dir));
    }
    let _lock = cache.lock(&format!("jar-modules-{sha256}"))?;
    if marker.is_file() {
        // Another process extracted it while this one waited.
        return Ok(Some(dir));
    }
    let parent = cache.root().join("jar-modules");
    std::fs::create_dir_all(&parent).map_err(|e| Error::io(&parent, &e))?;
    let tmp = TempDir::new_in(&parent, "extract")?;
    let mut cmd = Command::new("unzip");
    cmd.args(["-q", "-o"])
        .arg(jar)
        .args(&names)
        .arg("-d")
        .arg(tmp.path());
    capture(&mut cmd)?;
    let done = tmp.path().join(".complete");
    std::fs::write(&done, "").map_err(|e| Error::io(&done, &e))?;
    if dir.exists() {
        // An interrupted earlier extraction: replace it.
        std::fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, &e))?;
    }
    std::fs::rename(tmp.path(), &dir).map_err(|e| Error::io(&dir, &e))?;
    Ok(Some(dir))
}

/// The error diagnostics of a `dive` JSON report, as `(code, message)`.
fn errors(v: &Value) -> Vec<(String, String)> {
    v.get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|d| d.get("severity").and_then(Value::as_str) == Some("error"))
        .map(|d| {
            let s = |k: &str| {
                d.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            (s("code"), s("message"))
        })
        .collect()
}

/// Whether a dive diagnostic code belongs to what SANY reports in its parse
/// phase: syntax (`DIVE-Snnnn`) and module resolution (`DIVE-E00nn`; see
/// `dive-sema`'s `diagnostics.rs`), except `DIVE-E0005`.
///
/// `DIVE-E0005` (two modules with the same name in one scope, SANY 4223) is
/// a semantic error in SANY: "Multiply-defined module" is reported under
/// `Semantic errors:` with exit 4. Evidence: the `tlaplus-tests` goldens of
/// `test/tla2sany/semantic/error_corpus/E4223_Test.tla` and
/// `test-model/sany/NestedModuleLetInstance.tla`.
#[must_use]
pub fn is_parse_phase_code(code: &str) -> bool {
    code.starts_with("DIVE-S") || (code.starts_with("DIVE-E00") && code != "DIVE-E0005")
}

/// Reads `dive`'s JSON report from a finished run, or explains why it is
/// unusable. The exit code must agree with `accepted` (0 if accepted, 1 if
/// rejected).
fn read_report(outcome: &ProcessOutcome, command: &str) -> std::result::Result<Value, String> {
    let v: Value = serde_json::from_str(outcome.stdout.trim()).map_err(|e| {
        format!(
            "dive {command} exited {:?} without a JSON report ({e}); stderr: {}",
            outcome.exit_code,
            outcome.stderr.trim().chars().take(400).collect::<String>()
        )
    })?;
    let schema = v.get("schema_version").and_then(Value::as_u64);
    if !matches!(schema, Some(1 | DIVE_JSON_SCHEMA)) {
        return Err(format!(
            "dive {command} reported schema_version {schema:?}; the harness reads 1 and {DIVE_JSON_SCHEMA}"
        ));
    }
    let Some(accepted) = v.get("accepted").and_then(Value::as_bool) else {
        return Err(format!("dive {command}'s report has no boolean `accepted`"));
    };
    let expected_exit = i32::from(!accepted);
    if outcome.exit_code != Some(expected_exit) {
        return Err(format!(
            "dive {command} reported accepted = {accepted} but exited {:?}",
            outcome.exit_code
        ));
    }
    Ok(v)
}

/// Maps a `dive analyze` run into SANY's vocabulary.
#[must_use]
pub fn analyze_result(outcome: &ProcessOutcome) -> SanyResult {
    let base = SanyResult {
        verdict: SanyVerdict::Unclassified,
        accepted: false,
        exit_code: outcome.exit_code,
        exit_code_reflects_errors: true,
        error_text: None,
        unresolved_modules: Vec::new(),
        note: None,
    };
    if outcome.timed_out {
        return SanyResult {
            verdict: SanyVerdict::Timeout,
            exit_code: None,
            note: Some("dive analyze hit the harness timeout".to_owned()),
            ..base
        };
    }
    let v = match read_report(outcome, "analyze") {
        Ok(v) => v,
        Err(note) => {
            return SanyResult {
                note: Some(note),
                ..base
            };
        }
    };
    let errs = errors(&v);
    let accepted = v.get("accepted").and_then(Value::as_bool) == Some(true);
    if accepted != errs.is_empty() {
        return SanyResult {
            note: Some(format!(
                "dive analyze reported accepted = {accepted} with {} error diagnostic(s)",
                errs.len()
            )),
            ..base
        };
    }
    let verdict = if accepted {
        SanyVerdict::Accept
    } else if errs.iter().any(|(c, _)| is_parse_phase_code(c)) {
        SanyVerdict::ParseError
    } else {
        SanyVerdict::SemanticError
    };
    let mut unresolved: Vec<String> = errs
        .iter()
        .filter(|(c, _)| c == "DIVE-E0001")
        .filter_map(|(_, m)| {
            let rest = m.split_once("cannot find module `")?.1;
            Some(rest.split_once('`')?.0.to_owned())
        })
        .collect();
    unresolved.sort();
    unresolved.dedup();
    SanyResult {
        verdict,
        accepted,
        error_text: (!accepted).then(|| {
            errs.iter()
                .map(|(c, m)| format!("{c}: {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        }),
        unresolved_modules: unresolved,
        ..base
    }
}

/// Maps a `dive parse` run into a parse-phase verdict: `DIVE-S0006` (the
/// module name differs from the file name) alone is a name mismatch, any
/// other error a rejection.
#[must_use]
pub fn parse_result(outcome: &ProcessOutcome) -> ParseResult {
    if outcome.timed_out {
        return ParseResult {
            verdict: None,
            timed_out: true,
            note: Some("dive parse hit the harness timeout".to_owned()),
        };
    }
    let v = match read_report(outcome, "parse") {
        Ok(v) => v,
        Err(note) => {
            return ParseResult {
                verdict: None,
                timed_out: false,
                note: Some(note),
            };
        }
    };
    let mismatch = v
        .get("diagnostics")
        .and_then(Value::as_array)
        .is_some_and(|ds| {
            ds.iter()
                .any(|d| d.get("code").and_then(Value::as_str) == Some("DIVE-S0006"))
        });
    let errs = errors(&v);
    let others: Vec<&(String, String)> = errs.iter().filter(|(c, _)| c != "DIVE-S0006").collect();
    let verdict = if !others.is_empty() {
        ParseVerdict::Reject
    } else if mismatch {
        ParseVerdict::NameMismatch
    } else {
        ParseVerdict::Accept
    };
    ParseResult {
        verdict: Some(verdict),
        timed_out: false,
        note: others.first().map(|(c, m)| format!("{c}: {m}")),
    }
}

/// A `dive check` run in the oracle vocabulary.
#[derive(Debug)]
pub enum CheckMapping {
    /// dive refused, or does not claim TLC's class for its result; the text
    /// names why.
    Refused(String),
    /// A result to compare.
    Result(Box<OracleResult>),
}

/// A result with only a verdict, an exit code and a note.
fn bare(verdict: Verdict, exit_code: Option<i32>, note: String) -> OracleResult {
    OracleResult {
        verdict,
        verdict_code: None,
        error_class: None,
        exit_code,
        exit_status: None,
        violated_property: None,
        counts: Counts::default(),
        fingerprint_collision: None,
        trace: None,
        error_codes: Vec::new(),
        warning_codes: Vec::new(),
        unresolved_modules: Vec::new(),
        note: Some(note),
    }
}

fn unclassified(exit_code: Option<i32>, note: String) -> CheckMapping {
    CheckMapping::Result(Box::new(bare(Verdict::Unclassified, exit_code, note)))
}

fn as_i32(v: &Value) -> Option<i32> {
    v.as_i64().and_then(|c| i32::try_from(c).ok())
}

fn budget_profile(options: &Value, time_limit: Option<Duration>) -> bool {
    let Some(limit) = time_limit else {
        return false;
    };
    let millis = u64::try_from(limit.as_millis()).unwrap_or(u64::MAX).max(1);
    options["profile"] == "tlc+overrides"
        && options["semantic_profile"] == "tlc"
        && options["budget"]["time_limit_source"] == "explicit"
        && options["budget"]["time_limit_ms"].as_u64() == Some(millis)
        && options["int"] == "tlc"
        && options["choose_order"] == "tlc"
        && options["explore_order"] == "tlc"
        && options["comparisons"] == "tlc"
        && options["store"] == "fingerprint"
        && options["fingerprint"]["scheme"] == "tlc"
        && options["fingerprint"]["polynomial_index"] == 0
        && options["seed"] == 0
        && options["deadlock_flag"] == false
        && options["warnings"] == true
        && matches!(options["action_composition"].as_str(), Some("off" | "tlc"))
        && options["checkpoint"].is_null()
}

/// Maps a `dive check --output json` run into the oracle vocabulary.
/// `tlc_profile`: the run was under `--compat=tlc`, so its exit code must be
/// TLC's for the verdict.
#[must_use]
pub fn check_result(outcome: &ProcessOutcome, tlc_profile: bool) -> CheckMapping {
    check_result_with_budget(outcome, tlc_profile, None)
}

/// A harness-selected time budget is the sole admitted compat override.
/// Its reported value and the semantic settings must match the request.
#[allow(clippy::too_many_lines, reason = "one mapping, field by field")]
fn check_result_with_budget(
    outcome: &ProcessOutcome,
    tlc_profile: bool,
    time_limit: Option<Duration>,
) -> CheckMapping {
    if outcome.timed_out {
        return CheckMapping::Result(Box::new(bare(
            Verdict::Timeout,
            None,
            "dive check hit the harness timeout".to_owned(),
        )));
    }
    let v: Value = match serde_json::from_str(outcome.stdout.trim()) {
        Ok(v) => v,
        Err(e) => {
            return unclassified(
                outcome.exit_code,
                format!(
                    "dive check exited {:?} without a JSON report ({e}); stderr: {}",
                    outcome.exit_code,
                    outcome.stderr.trim().chars().take(400).collect::<String>()
                ),
            );
        }
    };
    if v.get("command").and_then(Value::as_str) != Some("check") {
        return unclassified(
            outcome.exit_code,
            "dive check's report has an unknown schema".to_owned(),
        );
    }
    let budget_override = tlc_profile && time_limit.is_some();
    if budget_override && !budget_profile(&v["options"], time_limit) {
        return unclassified(
            outcome.exit_code,
            "budgeted compat report contradicts the requested budget or TLC semantics".into(),
        );
    }
    let temporal = match v.get("schema_version").and_then(Value::as_u64) {
        Some(1) if v.get("check_kind").is_none() => None,
        Some(2) => match v.get("check_kind").and_then(Value::as_str) {
            Some("safety") => {
                if !v["properties"].is_null()
                    || !v["execution"].is_null()
                    || !v["options"]["lncheck"].is_null()
                    || !v["options"]["lasso"].is_null()
                    || v["verdict"] == "liveness-violation"
                {
                    return unclassified(
                        outcome.exit_code,
                        "safety report carries contradictory temporal fields".into(),
                    );
                }
                None
            }
            Some("temporal") => {
                match ordinary_final::read(&v, tlc_profile, outcome.exit_code, budget_override) {
                    Ok(fields) => Some(fields),
                    Err(error) => return unclassified(outcome.exit_code, error),
                }
            }
            // Before report unification, schema 2 always meant temporal.
            None if v.get("check_kind").is_none() => {
                match ordinary_final::read(&v, tlc_profile, outcome.exit_code, budget_override) {
                    Ok(fields) => Some(fields),
                    Err(error) => return unclassified(outcome.exit_code, error),
                }
            }
            _ => return unclassified(outcome.exit_code, "unknown check report kind".into()),
        },
        _ => {
            return unclassified(
                outcome.exit_code,
                "dive check's report has an unknown schema".into(),
            );
        }
    };
    map_check_document(outcome, tlc_profile, &v, temporal.as_ref())
}

/// Maps safety facts after the study has validated its final/short mode and
/// options. This supplies a study observation, not temporal completion evidence,
/// and does not alter the document's version or report family.
pub(crate) fn safety_result(
    outcome: &ProcessOutcome,
    tlc_profile: bool,
    value: &Value,
) -> CheckMapping {
    map_check_document(outcome, tlc_profile, value, None)
}

#[allow(clippy::too_many_lines, reason = "one mapping, field by field")]
fn map_check_document(
    outcome: &ProcessOutcome,
    tlc_profile: bool,
    v: &Value,
    temporal: Option<&ordinary_final::Fields>,
) -> CheckMapping {
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let verdict = s("verdict").unwrap_or_default();
    let message = s("message").unwrap_or_default();
    match verdict.as_str() {
        "unsupported" | "resource-limit" => {
            if outcome.exit_code != Some(DIVE_EXIT_UNSUPPORTED) {
                return unclassified(
                    outcome.exit_code,
                    format!(
                        "dive check reported {verdict} but exited {:?}",
                        outcome.exit_code
                    ),
                );
            }
            let prefix = if verdict == "unsupported" {
                "unsupported"
            } else {
                "resource limit"
            };
            return CheckMapping::Refused(format!("{prefix}: {message}"));
        }
        "internal-error" => {
            return unclassified(
                outcome.exit_code,
                format!("dive check internal error: {message}"),
            );
        }
        "stopped" => {
            if outcome.exit_code != Some(DIVE_EXIT_STOPPED) {
                return unclassified(
                    outcome.exit_code,
                    format!(
                        "dive check reported stopped but exited {:?}",
                        outcome.exit_code
                    ),
                );
            }
            // Out of time is the harness's timeout, reported by dive itself
            // rather than by a kill. Neither stop claims anything about the
            // model.
            return match v["stopped"]["resource"].as_str() {
                Some("time") => CheckMapping::Result(Box::new(bare(
                    Verdict::Timeout,
                    outcome.exit_code,
                    format!("dive check stopped at its --time-limit: {message}"),
                ))),
                // A finding, like a timeout: the run did not finish within its
                // budget, and a blowup must not hide among refusals.
                Some("memory") => CheckMapping::Result(Box::new(bare(
                    Verdict::Timeout,
                    outcome.exit_code,
                    format!("dive check stopped at its --max-memory: {message}"),
                ))),
                _ => unclassified(
                    outcome.exit_code,
                    "dive check reported stopped without a resource".into(),
                ),
            };
        }
        "rejected" => {
            let errs = errors(v);
            let verdict = if errs.iter().any(|(c, _)| c.starts_with("DIVE-C")) {
                Verdict::ConfigError
            } else if errs.iter().any(|(c, _)| is_parse_phase_code(c)) {
                Verdict::ParseError
            } else {
                Verdict::SemanticError
            };
            let note = errs
                .iter()
                .map(|(c, m)| format!("{c}: {m}"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut result = bare(verdict, outcome.exit_code, note);
            // TLC's class for the rejection (3002 for SANY's), only when dive
            // claims it; otherwise it stays absent and differs from the
            // golden's, so a rejection never agrees on an unclaimed class.
            let tlc = &v["tlc"];
            if tlc.get("verified").and_then(Value::as_bool) == Some(true) {
                let tlc_exit = tlc.get("exit_status").and_then(as_i32);
                if tlc_profile && outcome.exit_code != tlc_exit {
                    return unclassified(
                        outcome.exit_code,
                        format!(
                            "dive check rejected the spec and exited {:?} under --compat=tlc, but TLC exits {tlc_exit:?}",
                            outcome.exit_code
                        ),
                    );
                }
                result.error_class = tlc.get("name").and_then(Value::as_str).map(str::to_owned);
            }
            return CheckMapping::Result(Box::new(result));
        }
        _ => {}
    }
    let tlc = &v["tlc"];
    let Some(code) = tlc.get("code").and_then(as_i32) else {
        return unclassified(
            outcome.exit_code,
            format!("dive check reported {verdict} without TLC's classification"),
        );
    };
    if tlc.get("verified").and_then(Value::as_bool) != Some(true) {
        let site = s("error_site").map_or_else(String::new, |x| format!(" at {x}"));
        return CheckMapping::Refused(format!(
            "TLC's error class for this result{site} is not reproduced yet (WP-C4); dive found {verdict} ({code}): {message}"
        ));
    }
    let tlc_exit = tlc.get("exit_status").and_then(as_i32);
    if tlc_profile && outcome.exit_code != tlc_exit {
        return unclassified(
            outcome.exit_code,
            format!(
                "dive check exited {:?} under --compat=tlc, but TLC exits {tlc_exit:?} for {code}",
                outcome.exit_code
            ),
        );
    }
    let Some(oracle_verdict) = verdict_for_code(code) else {
        return unclassified(
            outcome.exit_code,
            format!("dive check reported TLC code {code}, which TLC does not declare"),
        );
    };
    let stats = &v["statistics"];
    let counts = if let Some(counts) = temporal.as_ref().and_then(|fields| fields.counts.as_ref()) {
        counts.clone()
    } else if stats.get("reported_by_tlc").and_then(Value::as_bool) == Some(true) {
        let n = |k: &str| stats.get(k).and_then(Value::as_u64);
        Counts {
            distinct_states: n("distinct"),
            states_generated: n("generated"),
            states_left_on_queue: n("left_on_queue"),
            depth: n("depth"),
        }
    } else {
        Counts::default()
    };
    let trace = if let Some(fields) = &temporal {
        fields.trace.clone()
    } else {
        v["counterexample"]
            .get("states")
            .and_then(Value::as_array)
            .map(|states| {
                let text = |st: &Value, k: &str| {
                    st.get(k)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim_end()
                        .to_owned()
                };
                let steps: Vec<TraceStep> = states
                    .iter()
                    .map(|st| TraceStep {
                        label: text(st, "label"),
                        state: text(st, "text"),
                    })
                    .collect();
                Trace {
                    length: u64::try_from(steps.len()).unwrap_or(u64::MAX),
                    ending: TraceEnding::Finite,
                    // dive's own document (`counterexample.trace`, dive-trace's
                    // format), which the harness validates itself (ADR 0016 §3).
                    json: v["counterexample"]
                        .get("trace")
                        .filter(|t| !t.is_null())
                        .cloned(),
                    steps: Some(steps),
                    omitted: None,
                    json_unavailable: None,
                }
            })
    };
    let collision = v["collision"].as_object().map(|c| CollisionProbability {
        optimistic: c
            .get("optimistic")
            .and_then(Value::as_str)
            .map(str::to_owned),
        observed: c.get("actual").and_then(Value::as_str).map(str::to_owned),
    });
    let notes: Vec<&str> = v["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    CheckMapping::Result(Box::new(OracleResult {
        verdict: oracle_verdict,
        verdict_code: Some(code),
        error_class: ec_name(code).map(str::to_owned),
        exit_code: outcome.exit_code,
        exit_status: tlc_exit.and_then(exit_status_name).map(str::to_owned),
        violated_property: s("property"),
        counts,
        fingerprint_collision: collision,
        trace,
        error_codes: if code == 2193 { Vec::new() } else { vec![code] },
        warning_codes: v["warnings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| w.get("code").and_then(as_i32))
            .collect(),
        unresolved_modules: Vec::new(),
        note: (!notes.is_empty()).then(|| notes.join(" ")),
    }))
}

impl Implementation for DiveImplementation {
    fn profile(&self) -> Option<Profile> {
        Some(self.profile.clone())
    }

    fn options(&self) -> &[ImplOption] {
        &self.options
    }

    fn result_preserving_options(&self) -> &[&str] {
        // A deterministic seed or a worker count never changes a TLC-
        // compatible result (ADR 0010).
        &["seed", "workers"]
    }

    fn name(&self) -> String {
        "dive".to_owned()
    }

    fn is_oracle(&self) -> bool {
        false
    }

    fn dive_provenance(&self) -> Option<DiveProvenance> {
        self.provenance.clone()
    }

    /// Validates dive's counterexample in-process with `dive-trace`, under
    /// the profile dive ran in, with the same module search path. dive's own
    /// `counterexample.validation` is ignored (ADR 0016 §3).
    fn trace_validated(
        &self,
        req: &ModelRequest<'_>,
        result: &OracleResult,
        golden: &OracleResult,
    ) -> crate::trace_check::TraceCheck {
        let compat_tlc = match self.profile {
            Profile::Tlc => true,
            Profile::Dive => false,
            Profile::TlcWithOverrides(_) => {
                return crate::trace_check::TraceCheck::no(
                    "the validator has no tlc+overrides profile",
                );
            }
        };
        let paths: Vec<PathBuf> = self.module_path.iter().map(|e| e.path.clone()).collect();
        match crate::trace_check::load_model(req.corpus_root, req.model, &paths, compat_tlc) {
            Ok(spec) => crate::trace_check::validate_counterexample(&spec, result, golden),
            Err(e) => crate::trace_check::TraceCheck::no(format!(
                "the validator cannot bind the model: {e}"
            )),
        }
    }

    fn check_model(&self, req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>> {
        if req.model.mode != ModelMode::Exhaustive {
            return Ok(ImplResult::Unsupported(format!(
                "{:?} mode: `dive check` does exhaustive search only",
                req.model.mode
            )));
        }
        let (command, outcome) = self.run_model(req, false)?;
        Ok(
            match check_result_with_budget(
                &outcome,
                self.profile == Profile::Tlc,
                req.budgets.then_some(req.timeout),
            ) {
                CheckMapping::Refused(why) => ImplResult::Unsupported(why),
                CheckMapping::Result(result) => ImplResult::Completed {
                    result: *result,
                    command,
                    elapsed: outcome.elapsed,
                },
            },
        )
    }

    fn parse_module(
        &self,
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<ImplResult<SanyResult>> {
        let mut extra: Vec<&std::ffi::OsStr> = Vec::new();
        for p in &self.module_path {
            extra.push("--module-path".as_ref());
            extra.push(p.path.as_os_str());
        }
        let (command, outcome) = self.run("analyze", &extra, corpus_root, module, timeout)?;
        if outcome.exit_code == Some(DIVE_EXIT_UNSUPPORTED) {
            return Ok(ImplResult::Unsupported(outcome.stderr.trim().to_owned()));
        }
        Ok(ImplResult::Completed {
            result: analyze_result(&outcome),
            command,
            elapsed: outcome.elapsed,
        })
    }

    fn parse_only(
        &self,
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<ImplResult<ParseResult>> {
        let (command, outcome) = self.run("parse", &[], corpus_root, module, timeout)?;
        if outcome.exit_code == Some(DIVE_EXIT_UNSUPPORTED) {
            return Ok(ImplResult::Unsupported(outcome.stderr.trim().to_owned()));
        }
        Ok(ImplResult::Completed {
            result: parse_result(&outcome),
            command,
            elapsed: outcome.elapsed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn relative_binary_survives_a_different_child_directory() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let cwd = std::env::current_dir().map_err(|e| Error::Io(e.to_string()))?;
        let source = TempDir::new_in(&cwd, "relative-binary")?;
        let child = TempDir::new("relative-binary-child")?;
        let file = source.path().join("run");
        std::fs::write(&file, "#!/bin/sh\nprintf 'resolved-executable\\n'\n")
            .map_err(|e| Error::io(&file, &e))?;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| Error::io(&file, &e))?;
        let relative = Path::new(".").join(
            file.strip_prefix(&cwd)
                .map_err(|e| Error::Config(e.to_string()))?,
        );
        // This is the original failure: the child resolves the executable
        // relative to its model directory rather than the harness directory.
        assert!(
            Command::new(&relative)
                .current_dir(child.path())
                .output()
                .is_err()
        );
        let executable = resolve_binary(&relative)?;
        let output = Command::new(executable)
            .current_dir(child.path())
            .output()
            .map_err(|e| Error::Io(e.to_string()))?;
        assert!(output.status.success());
        assert_eq!(output.stdout, b"resolved-executable\n");
        Ok(())
    }

    fn outcome(exit: i32, stdout: &str) -> ProcessOutcome {
        ProcessOutcome {
            exit_code: Some(exit),
            signal: None,
            timed_out: false,
            stdout: stdout.to_owned(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        }
    }

    #[test]
    fn only_the_requested_budget_override_preserves_tlc_facts()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use dive_api::{BudgetSetting, CheckRequest, ExitConvention};
        use serde_json::json;

        let scratch = TempDir::new_in(&std::env::temp_dir(), "budget-profile")?;
        let file = scratch.path().join("BudgetProfile.tla");
        std::fs::write(
            &file,
            "---- MODULE BudgetProfile ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x\nP == <>(x=1)\n====\n",
        )?;
        let limit = Duration::from_secs(60);
        for temporal in [false, true] {
            std::fs::write(
                file.with_extension("cfg"),
                if temporal {
                    "INIT Init\nNEXT Next\nPROPERTY P\n"
                } else {
                    "INIT Init\nNEXT Next\n"
                },
            )?;
            let report = dive_api::check_file(&CheckRequest {
                file: file.clone(),
                compat_tlc: true,
                fp: Some(0),
                seed: Some(0),
                time_limit: BudgetSetting::Explicit(limit),
                max_memory: BudgetSetting::Unlimited,
                ..CheckRequest::default()
            })?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            let status = i32::from(report.exit_code_for(ExitConvention::Tlc));
            let mapped = |value: &Value| {
                check_result_with_budget(&outcome(status, &value.to_string()), true, Some(limit))
            };
            let CheckMapping::Result(fact) = mapped(&value) else {
                return Err("budgeted report must produce a classified result".into());
            };
            assert_eq!(
                fact.verdict,
                if temporal {
                    Verdict::LivenessViolation
                } else {
                    Verdict::Success
                }
            );
            for (pointer, replacement) in [
                ("/options/profile", json!("tlc")),
                ("/options/semantic_profile", json!("dive")),
                ("/options/int", json!("big")),
                ("/options/choose_order", json!("canonical")),
                ("/options/explore_order", json!("value")),
                ("/options/comparisons", json!("tla")),
                ("/options/action_composition", json!("tla")),
                ("/options/store", json!("exact")),
                ("/options/fingerprint/scheme", json!("dive")),
                ("/options/fingerprint/polynomial_index", json!(1)),
                ("/options/seed", json!(1)),
                ("/options/deadlock_flag", json!(true)),
                ("/options/warnings", json!(false)),
                ("/options/budget/time_limit_ms", json!(1)),
                ("/options/budget/time_limit_source", json!("none")),
            ] {
                let mut bad = value.clone();
                *bad.pointer_mut(pointer).ok_or("control field exists")? = replacement;
                assert!(
                    matches!(mapped(&bad), CheckMapping::Result(r) if r.verdict == Verdict::Unclassified),
                    "{pointer}"
                );
            }
            let mut checkpoint = value.clone();
            checkpoint["options"]["checkpoint"] = json!({"counters": "retain"});
            assert!(
                matches!(mapped(&checkpoint), CheckMapping::Result(r) if r.verdict == Verdict::Unclassified)
            );
            if temporal {
                // The ordinary reader still rejects overrides without a
                // harness request authorizing this exact budget.
                assert!(
                    matches!(check_result(&outcome(status, &value.to_string()), true), CheckMapping::Result(r) if r.verdict == Verdict::Unclassified)
                );
            }
        }
        Ok(())
    }

    #[test]
    fn safety_v2_is_explicit_and_cannot_bypass_temporal_transport() {
        let legacy = serde_json::json!({
            "schema_version":1, "command":"check", "verdict":"success",
            "tlc":{"code":2193,"name":"TLC_SUCCESS","exit_status":0,"verified":true},
            "statistics":{"reported_by_tlc":true,"distinct":2,"generated":3,"depth":2,"left_on_queue":0}
        });
        let mut current = legacy.clone();
        current["schema_version"] = 2.into();
        current["check_kind"] = "safety".into();
        for document in [&legacy, &current] {
            assert!(
                matches!(check_result(&outcome(0, &document.to_string()), true),
                CheckMapping::Result(r) if r.verdict == Verdict::Success
                    && r.counts.distinct_states == Some(2)
                    && r.counts.states_generated == Some(3))
            );
        }
        let changes = [
            ("check_kind", Value::from("temporal")),
            ("check_kind", Value::from("unknown")),
            ("check_kind", Value::Null),
            ("execution", serde_json::json!({"capture_completed":true})),
            ("properties", serde_json::json!([])),
            (
                "options",
                serde_json::json!({"lncheck":"final","lasso":"short"}),
            ),
            ("verdict", Value::from("liveness-violation")),
            ("schema_version", Value::from(99)),
            ("schema_version", Value::from(1)),
        ];
        for (key, value) in changes {
            let mut forged = current.clone();
            forged[key] = value;
            assert!(
                matches!(check_result(&outcome(0, &forged.to_string()), true),
                CheckMapping::Result(r) if r.verdict == Verdict::Unclassified),
                "{forged}"
            );
        }
        // Missing metadata on version 2 retains the old strict temporal route.
        assert_eq!(
            current
                .as_object_mut()
                .and_then(|object| object.remove("check_kind")),
            Some(Value::from("safety"))
        );
        assert!(
            matches!(check_result(&outcome(0, &current.to_string()), true),
            CheckMapping::Result(r) if r.verdict == Verdict::Unclassified)
        );
    }

    #[test]
    fn budget_stops_map_to_timeout_or_refusal_never_to_a_result() {
        let stopped = |resource: &str, exit: i32| {
            check_result(
                &outcome(
                    exit,
                    &format!(
                        r#"{{"schema_version":1,"command":"check","verdict":"stopped","message":"stopped during safety-search","stopped":{{"resource":"{resource}"}}}}"#
                    ),
                ),
                true,
            )
        };
        assert!(matches!(
            stopped("time", 5),
            CheckMapping::Result(r) if r.verdict == Verdict::Timeout
        ));
        assert!(matches!(
            stopped("memory", 5),
            CheckMapping::Result(r) if r.verdict == Verdict::Timeout
                && r.note.as_deref().is_some_and(|n| n.contains("--max-memory"))
        ));
        // A stop must carry its own exit status and resource.
        assert!(matches!(
            stopped("time", 3),
            CheckMapping::Result(r) if r.verdict == Verdict::Unclassified
        ));
        assert!(matches!(
            stopped("disk", 5),
            CheckMapping::Result(r) if r.verdict == Verdict::Unclassified
        ));
    }

    #[test]
    fn analyze_mapping() {
        let ok = analyze_result(&outcome(
            0,
            r#"{"schema_version":1,"accepted":true,"diagnostics":[{"code":"DIVE-W0001","severity":"warning","message":"w"}]}"#,
        ));
        assert_eq!((ok.verdict, ok.accepted), (SanyVerdict::Accept, true));
        let missing = analyze_result(&outcome(
            1,
            r#"{"schema_version":1,"accepted":false,"diagnostics":[{"code":"DIVE-E0001","severity":"error","message":"cannot find module `Nope`, which module `A` extends or instantiates"}]}"#,
        ));
        assert_eq!(missing.verdict, SanyVerdict::ParseError);
        assert_eq!(missing.unresolved_modules, ["Nope"]);
        let sem = analyze_result(&outcome(
            1,
            r#"{"schema_version":1,"accepted":false,"diagnostics":[{"code":"DIVE-E0100","severity":"error","message":"unknown operator"}]}"#,
        ));
        assert_eq!(sem.verdict, SanyVerdict::SemanticError);
        let dup = analyze_result(&outcome(
            1,
            r#"{"schema_version":1,"accepted":false,"diagnostics":[{"code":"DIVE-E0005","severity":"error","message":"module `M` is defined twice"}]}"#,
        ));
        assert_eq!(dup.verdict, SanyVerdict::SemanticError);
        // Exit code contradicting `accepted`, a crash, a schema change: never
        // a verdict.
        for bad in [
            outcome(
                1,
                r#"{"schema_version":1,"accepted":true,"diagnostics":[]}"#,
            ),
            outcome(101, "thread 'main' panicked"),
            outcome(
                0,
                r#"{"schema_version":99,"accepted":true,"diagnostics":[]}"#,
            ),
        ] {
            assert_eq!(analyze_result(&bad).verdict, SanyVerdict::Unclassified);
        }
    }

    #[test]
    #[allow(clippy::panic, reason = "test code")]
    fn check_mapping() {
        let violation = r#"{"schema_version":1,"command":"check","verdict":"invariant-violation","property":"Inv","message":null,"error_site":null,
            "tlc":{"code":2110,"name":"TLC_INVARIANT_VIOLATED_BEHAVIOR","exit_status":12,"verified":true},
            "statistics":{"generated":3,"distinct":3,"left_on_queue":1,"depth":2,"initial_generated":1,"reported_by_tlc":true},
            "counterexample":{"length":2,"ending":"finite","states":[{"label":"<Initial predicate>","text":"x = 0\n"},{"label":"<Next>","text":"x = 2\n"}]},
            "collision":null,"warnings":[],"notes":[]}"#;
        let CheckMapping::Result(r) = check_result(&outcome(12, violation), true) else {
            panic!("a result");
        };
        assert_eq!(r.verdict, Verdict::SafetyViolation);
        assert_eq!(
            r.error_class.as_deref(),
            Some("TLC_INVARIANT_VIOLATED_BEHAVIOR")
        );
        assert_eq!(r.violated_property.as_deref(), Some("Inv"));
        assert_eq!(
            (r.counts.states_generated, r.counts.depth),
            (Some(3), Some(2))
        );
        assert_eq!(r.trace.as_ref().map(|t| t.length), Some(2));
        // Under --compat=tlc the exit code must be TLC's.
        let CheckMapping::Result(bad) = check_result(&outcome(10, violation), true) else {
            panic!("a result");
        };
        assert_eq!(bad.verdict, Verdict::Unclassified);
        // An unverified class or a refusal is never a verdict.
        let unverified = violation.replace("\"verified\":true", "\"verified\":false");
        assert!(matches!(
            check_result(&outcome(12, &unverified), true),
            CheckMapping::Refused(_)
        ));
        let refused = r#"{"schema_version":1,"command":"check","verdict":"unsupported","message":"SYMMETRY S"}"#;
        assert!(matches!(
            check_result(&outcome(3, refused), true),
            CheckMapping::Refused(w) if w == "unsupported: SYMMETRY S"
        ));
        // A rejection carries TLC's class only when dive claims it.
        let rejected = r#"{"schema_version":1,"command":"check","verdict":"rejected","message":null,
            "diagnostics":[{"code":"DIVE-S0010","severity":"error","message":"expected `)`"}],
            "tlc":{"code":3002,"name":"TLC_PARSING_FAILED","exit_status":150,"verified":true}}"#;
        let CheckMapping::Result(r) = check_result(&outcome(150, rejected), true) else {
            panic!("a result");
        };
        assert_eq!(
            (r.verdict, r.error_class.as_deref()),
            (Verdict::ParseError, Some("TLC_PARSING_FAILED"))
        );
        let CheckMapping::Result(r) = check_result(&outcome(1, rejected), true) else {
            panic!("a result");
        };
        assert_eq!(
            r.verdict,
            Verdict::Unclassified,
            "TLC's exit under --compat=tlc"
        );
        let CheckMapping::Result(r) = check_result(&outcome(1, rejected), false) else {
            panic!("a result");
        };
        assert_eq!(r.error_class.as_deref(), Some("TLC_PARSING_FAILED"));
        let unclaimed = rejected.replace("\"verified\":true", "\"verified\":false");
        let CheckMapping::Result(r) = check_result(&outcome(150, &unclaimed), true) else {
            panic!("a result");
        };
        assert_eq!((r.verdict, r.error_class), (Verdict::ParseError, None));
        // No statistics where TLC prints none.
        let init = violation.replace("\"reported_by_tlc\":true", "\"reported_by_tlc\":false");
        let CheckMapping::Result(r) = check_result(&outcome(12, &init), true) else {
            panic!("a result");
        };
        assert_eq!(r.counts, Counts::default());
    }

    #[test]
    fn parse_mapping() {
        let v = |exit, json| parse_result(&outcome(exit, json)).verdict;
        assert_eq!(
            v(
                0,
                r#"{"schema_version":1,"accepted":true,"diagnostics":[]}"#
            ),
            Some(ParseVerdict::Accept)
        );
        assert_eq!(
            v(
                1,
                r#"{"schema_version":1,"accepted":false,"diagnostics":[{"code":"DIVE-S0006","severity":"error","message":"m"}]}"#
            ),
            Some(ParseVerdict::NameMismatch)
        );
        assert_eq!(
            v(
                1,
                r#"{"schema_version":1,"accepted":false,"diagnostics":[{"code":"DIVE-S0006","severity":"error","message":"m"},{"code":"DIVE-S0001","severity":"error","message":"x"}]}"#
            ),
            Some(ParseVerdict::Reject)
        );
        assert_eq!(v(2, "error: cannot read"), None);
    }
}
