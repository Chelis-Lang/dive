//! Implementations under test.
//!
//! Anything that can check a model or parse a module plugs in through
//! [`Implementation`]. Results use the oracle vocabulary ([`OracleResult`],
//! [`SanyResult`]), so the same comparison applies to TLC, `dive` and
//! external tools. An implementation that does not support something must
//! say so with [`ImplResult::Unsupported`]; it must never return a guess.
//!
//! - [`TlcImplementation`] re-runs a pinned TLC oracle (self-consistency,
//!   determinism and cross-oracle diffs).
//! - [`DiveImplementation`] is a stub that refuses everything until `dive`
//!   produces verdicts.
//! - [`ExternalAdapter`] is the shape for external tools (M2): a TOML file
//!   with a command template and an exit-code map. It is parsed and
//!   validated, but not executed yet; every request is refused.

use crate::corpus::{ModelEntry, ModelMode};
use crate::error::{Error, Result};
use crate::oracle::{CheckMode, OracleEnv, TlcJob};
use crate::sany::{self, SanyResult};
use crate::tlc::messages;
use crate::tlc::result::{OracleResult, RunFacts, Verdict, classify};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// A model-checking request.
#[derive(Debug, Clone, Copy)]
pub struct ModelRequest<'a> {
    /// Corpus checkout root.
    pub corpus_root: &'a Path,
    /// The model.
    pub model: &'a ModelEntry,
    /// Worker count.
    pub workers: u32,
    /// Timeout.
    pub timeout: Duration,
    /// JVM heap limit, for JVM-based implementations.
    pub xmx: &'a str,
}

/// One implementation option, as given with `--impl-option NAME[=VALUE]`
/// (ADR 0010: `dive` is TLC-compatible by default and better by option).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImplOption {
    /// Option name, e.g. `seed` or `int`.
    pub name: String,
    /// Option value, if any.
    pub value: Option<String>,
    /// Whether the option may change results (e.g. `int=big`). Results under
    /// such an option are never compared as `agree` with TLC goldens; they
    /// are reported as `divergence-mode`.
    pub result_changing: bool,
}

impl ImplOption {
    /// Parses `NAME[=VALUE]`; `result_changing` is decided by the
    /// implementation (see [`Implementation::result_preserving_options`]).
    ///
    /// # Errors
    /// Fails on an empty name.
    pub fn parse(text: &str, imp: &dyn Implementation) -> Result<Self> {
        let (name, value) = match text.split_once('=') {
            Some((n, v)) => (n.trim(), Some(v.trim().to_owned())),
            None => (text.trim(), None),
        };
        if name.is_empty() {
            return Err(Error::Usage(format!("--impl-option `{text}` has no name")));
        }
        Ok(Self {
            name: name.to_owned(),
            value,
            result_changing: !imp.result_preserving_options().contains(&name),
        })
    }
}

/// The answer to a request.
#[derive(Debug, Clone)]
pub enum ImplResult<T> {
    /// The implementation produced a result.
    Completed {
        /// The result.
        result: T,
        /// The command line, if the implementation ran a process.
        command: Vec<String>,
        /// Wall-clock time.
        elapsed: Duration,
    },
    /// The implementation explicitly refused, naming the feature.
    Unsupported(String),
}

/// An implementation under test.
pub trait Implementation: Sync {
    /// A short name for reports, e.g. `tlc:tlc-1.7.4`.
    fn name(&self) -> String;

    /// Whether this implementation is itself an oracle. Unclassifiable or
    /// contradictory results of an oracle are `oracle-error`; those of any
    /// other implementation are disagreements.
    fn is_oracle(&self) -> bool;

    /// Checks one model.
    ///
    /// # Errors
    /// Only for harness failures (cannot start a process); every outcome of
    /// the implementation itself is a result.
    fn check_model(&self, req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>>;

    /// The profile this instance runs under (ADR 0010): `dive`, `tlc` or
    /// `tlc+overrides`. `None` for TLC oracles and external tools, which
    /// have no profile and are never excused by the divergence registry.
    fn profile(&self) -> Option<crate::divergence::Profile> {
        None
    }

    /// The implementation options this instance runs under
    /// (`--impl-option`), recorded in every report (ADR 0010).
    fn options(&self) -> &[ImplOption] {
        &[]
    }

    /// Names of options this implementation declares result-preserving
    /// (e.g. `seed`). Every other option is treated as result-changing, so
    /// an undeclared option can never produce an `agree`.
    fn result_preserving_options(&self) -> &[&str] {
        &[]
    }

    /// Whether this implementation may omit the name of a violated property
    /// that the golden names. Only an oracle lacking the capability (v1.7.4
    /// does not name violated temporal properties) may; for anything else an
    /// omitted name is a diff.
    fn omits_property_names(&self) -> bool {
        false
    }

    /// Whether this implementation independently validated the
    /// counterexample in `result` against the spec (charter rule 2: every
    /// counterexample is replayed by a separate trace validator). An
    /// implementation that is not an oracle cannot `agree` on a violation
    /// without it. The default is `false`.
    fn trace_validated(&self, _result: &OracleResult) -> bool {
        false
    }

    /// Parses one module (corpus-relative path).
    ///
    /// # Errors
    /// Only for harness failures.
    fn parse_module(
        &self,
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<ImplResult<SanyResult>>;
}

/// Re-runs a pinned TLC oracle.
#[derive(Debug)]
pub struct TlcImplementation {
    /// The oracle.
    pub env: OracleEnv,
}

/// Runs one model on an oracle and classifies the result.
///
/// # Errors
/// Only for harness failures.
pub fn run_oracle_model(
    env: &OracleEnv,
    req: &ModelRequest<'_>,
) -> Result<ImplResult<OracleResult>> {
    let mode = match req.model.mode {
        ModelMode::Exhaustive => CheckMode::Exhaustive,
        ModelMode::Simulate { traces } => CheckMode::Simulate { traces },
        ModelMode::Generate | ModelMode::Symbolic => {
            return Ok(ImplResult::Unsupported(format!(
                "{:?} models are out of scope",
                req.model.mode
            )));
        }
    };
    let job = TlcJob {
        corpus_root: req.corpus_root,
        module: &req.model.module,
        config: &req.model.id,
        mode,
        features: &req.model.features,
        workers: req.workers,
        timeout: req.timeout,
        xmx: req.xmx,
    };
    let inv = env.run_tlc(&job)?;
    let transcript = messages::parse(&inv.outcome.stdout);
    let result = classify(
        &transcript,
        &RunFacts {
            exit_code: inv.outcome.exit_code,
            timed_out: inv.outcome.timed_out,
            trace_json: inv.trace_json.as_deref(),
            simulate: matches!(mode, CheckMode::Simulate { .. }),
            stderr: Some(&inv.outcome.stderr),
        },
    );
    Ok(ImplResult::Completed {
        result,
        command: inv.command,
        elapsed: inv.outcome.elapsed,
    })
}

/// Runs SANY on one module of an oracle.
///
/// # Errors
/// Only for harness failures, or a module path without a file name.
pub fn run_oracle_sany(
    env: &OracleEnv,
    corpus_root: &Path,
    module: &str,
    timeout: Duration,
) -> Result<ImplResult<SanyResult>> {
    let inv = env.run_sany(corpus_root, module, timeout)?;
    Ok(ImplResult::Completed {
        result: sany::classify(&inv.outcome),
        command: inv.command,
        elapsed: inv.outcome.elapsed,
    })
}

impl Implementation for TlcImplementation {
    fn name(&self) -> String {
        format!("tlc:{}", self.env.oracle.id)
    }

    fn is_oracle(&self) -> bool {
        true
    }

    fn omits_property_names(&self) -> bool {
        !self.env.capabilities.names_temporal_properties
    }

    fn check_model(&self, req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>> {
        run_oracle_model(&self.env, req)
    }

    fn parse_module(
        &self,
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<ImplResult<SanyResult>> {
        run_oracle_sany(&self.env, corpus_root, module, timeout)
    }
}

/// `dive` itself. Refuses everything until `dive check` and `dive parse`
/// produce results (soundness contract rule 3).
#[derive(Debug, Default)]
pub struct DiveImplementation {
    /// Options `dive` would run under (`--impl-option`).
    pub options: Vec<ImplOption>,
    /// The profile (`--impl-profile`, or `DIVE_COMPAT=tlc`).
    pub profile: crate::divergence::Profile,
}

impl Implementation for DiveImplementation {
    fn profile(&self) -> Option<crate::divergence::Profile> {
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

    fn check_model(&self, _req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>> {
        Ok(ImplResult::Unsupported(
            "`dive check` is not implemented yet; no verdict produced".to_owned(),
        ))
    }

    fn parse_module(
        &self,
        _root: &Path,
        _module: &str,
        _timeout: Duration,
    ) -> Result<ImplResult<SanyResult>> {
        Ok(ImplResult::Unsupported(
            "`dive parse` is not implemented yet; no verdict produced".to_owned(),
        ))
    }
}

/// An external tool described by a TOML file:
///
/// ```toml
/// name = "tla2"
/// # Placeholders: {module} {config} {workers} {corpus}
/// check = ["tla2", "check", "{module}", "--config", "{config}", "--workers", "{workers}"]
/// parse = ["tla2", "parse", "{module}"]          # optional
/// [exit_codes]                                     # exit code -> verdict
/// 0 = "success"
/// 12 = "safety-violation"
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalAdapter {
    /// Tool name.
    pub name: String,
    /// Command template for model checking.
    pub check: Vec<String>,
    /// Command template for parsing.
    #[serde(default)]
    pub parse: Option<Vec<String>>,
    /// Exit code to verdict map.
    pub exit_codes: BTreeMap<String, Verdict>,
    /// Options the tool runs under (`--impl-option`); not part of the TOML.
    #[serde(skip)]
    pub options: Vec<ImplOption>,
}

impl ExternalAdapter {
    /// Loads and validates an adapter description.
    ///
    /// # Errors
    /// Fails on unreadable or invalid files.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
        let a: Self =
            toml::from_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display())))?;
        if a.check.is_empty() {
            return Err(Error::Config(format!(
                "{}: `check` is empty",
                path.display()
            )));
        }
        for k in a.exit_codes.keys() {
            k.parse::<i32>().map_err(|_| {
                Error::Config(format!(
                    "{}: exit code `{k}` is not an integer",
                    path.display()
                ))
            })?;
        }
        Ok(a)
    }
}

impl Implementation for ExternalAdapter {
    fn options(&self) -> &[ImplOption] {
        &self.options
    }

    fn name(&self) -> String {
        format!("adapter:{}", self.name)
    }

    fn is_oracle(&self) -> bool {
        false
    }

    fn check_model(&self, _req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>> {
        Ok(ImplResult::Unsupported(format!(
            "external adapter `{}` is described but adapters are not executed yet (planned for M2)",
            self.name
        )))
    }

    fn parse_module(
        &self,
        _root: &Path,
        _module: &str,
        _timeout: Duration,
    ) -> Result<ImplResult<SanyResult>> {
        Ok(ImplResult::Unsupported(format!(
            "external adapter `{}` is described but adapters are not executed yet (planned for M2)",
            self.name
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_description_parses_and_refuses()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("adapter")?;
        let p = dir.path().join("a.toml");
        std::fs::write(
            &p,
            "name = \"x\"\ncheck = [\"x\", \"{module}\"]\n[exit_codes]\n0 = \"success\"\n12 = \"safety-violation\"\n",
        )?;
        let a = ExternalAdapter::load(&p)?;
        assert_eq!(a.exit_codes.get("12"), Some(&Verdict::SafetyViolation));
        assert!(!a.is_oracle());
        let r = a.parse_module(dir.path(), "M.tla", Duration::from_secs(1))?;
        assert!(matches!(r, ImplResult::Unsupported(_)));
        std::fs::write(&p, "name = \"x\"\ncheck = []\n[exit_codes]\n")?;
        assert!(ExternalAdapter::load(&p).is_err());
        std::fs::write(
            &p,
            "name = \"x\"\ncheck = [\"x\"]\n[exit_codes]\nzero = \"success\"\n",
        )?;
        assert!(ExternalAdapter::load(&p).is_err());
        Ok(())
    }

    #[test]
    fn dive_refuses() -> Result<()> {
        let d = DiveImplementation::default();
        let r = d.parse_module(Path::new("."), "M.tla", Duration::from_secs(1))?;
        assert!(matches!(r, ImplResult::Unsupported(m) if m.contains("not implemented")));
        Ok(())
    }
}
