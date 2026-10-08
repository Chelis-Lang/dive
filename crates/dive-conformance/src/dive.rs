//! `dive` itself as an implementation under test.
//!
//! The harness runs the `dive` binary as a black box, exactly as a user
//! would, and maps its JSON output into the oracle vocabulary:
//! - full SANY goldens are compared with `dive analyze --output json`;
//! - parse-phase verdicts (`run --sany-phase parse`) with
//!   `dive parse --output json`;
//! - models are refused: `dive check` does not exist yet, so every model is
//!   `unsupported`, naming the missing command (soundness contract rule 3).
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
use crate::divergence::Profile;
use crate::error::{Error, Result};
use crate::hash::sha256_file;
use crate::implementation::{ImplOption, ImplResult, Implementation, ModelRequest, ParseResult};
use crate::oracle::{ClasspathEntry, library_entry};
use crate::process::{ProcessOutcome, run_with_timeout};
use crate::sany::{ParseVerdict, SanyResult, SanyVerdict};
use crate::tlc::result::OracleResult;
use crate::util::{TempDir, capture, redact};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The `dive analyze` / `dive parse` JSON schema version the harness reads.
pub const DIVE_JSON_SCHEMA: u64 = 1;

/// Exit code `dive` uses for a recognised but unimplemented request.
const DIVE_EXIT_UNSUPPORTED: i32 = 3;

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
        let oracle = config.oracles.oracle(golden_oracle)?;
        let mut module_path = Vec::new();
        for id in &oracle.classpath {
            let a = config.oracles.classpath_artifact(id)?;
            let jar =
                cache.verified_artifact(&format!("classpath jar `{id}`"), &a.url, &a.sha256)?;
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
        if !bin.is_file() {
            return Err(Error::Config(format!(
                "the dive binary {} does not exist; build it with `cargo build --release -p dive-cli` or pass --dive-bin",
                bin.display()
            )));
        }
        let version = capture(Command::new(bin).arg("--version"))?
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
            sha256: sha256_file(bin)?,
            version,
            module_path: module_path.clone(),
        };
        Ok(Self {
            options: Vec::new(),
            profile: Profile::default(),
            bin: bin.to_path_buf(),
            module_path,
            provenance: Some(provenance),
            tmp_root: cache.tmp_dir(),
            cache_root: cache.root().to_path_buf(),
        })
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
    if schema != Some(DIVE_JSON_SCHEMA) {
        return Err(format!(
            "dive {command} reported schema_version {schema:?}; the harness reads {DIVE_JSON_SCHEMA}"
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

    fn models_unsupported(&self) -> Option<String> {
        Some("`dive check` is not implemented yet; no verdict produced".to_owned())
    }

    fn check_model(&self, _req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>> {
        Ok(ImplResult::Unsupported(
            self.models_unsupported().unwrap_or_default(),
        ))
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
                r#"{"schema_version":2,"accepted":true,"diagnostics":[]}"#,
            ),
        ] {
            assert_eq!(analyze_result(&bad).verdict, SanyVerdict::Unclassified);
        }
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
