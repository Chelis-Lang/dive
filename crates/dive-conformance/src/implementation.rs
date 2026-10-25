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
//! - [`DiveImplementation`] runs the `dive` binary: `dive analyze` and
//!   `dive parse` for modules, `dive check` for models (see [`crate::dive`]).
//! - [`ExternalAdapter`] is the shape for external tools (M2): a TOML file
//!   with a command template, an exit-code map and output rules. It is
//!   parsed and validated, but not executed yet; every request is refused.

pub use crate::dive::DiveImplementation;

use crate::corpus::{ModelEntry, ModelMode};
use crate::error::{Error, Result};
use crate::oracle::{CheckMode, Invocation, OracleEnv, TlcJob};
use crate::sany::{self, ParseVerdict, SanyResult};
use crate::tlc::messages;
use crate::tlc::result::{OracleResult, RunFacts, Verdict, classify, classify_owned};
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
    /// Whether `timeout` and `xmx` are the run's own budgets (`--tlc-budgets`,
    /// ADR 0107). Otherwise `dive check` runs without budgets of its own and
    /// only the harness timeout applies, as before.
    pub budgets: bool,
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

/// A parse-phase result (`run --sany-phase parse`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    /// The verdict, or `None` when the run does not determine one (see the
    /// note).
    pub verdict: Option<ParseVerdict>,
    /// The harness timeout fired.
    pub timed_out: bool,
    /// Why there is no verdict, or the first error.
    pub note: Option<String>,
}

impl ParseResult {
    /// The parse-phase verdict of `module`'s own file, derived from a full
    /// SANY result ([`sany::parse_phase`]).
    #[must_use]
    pub fn from_sany(result: &SanyResult, module: &str) -> Self {
        match sany::parse_phase(result, module) {
            Ok(v) => Self {
                verdict: Some(v),
                timed_out: false,
                note: None,
            },
            Err(why) => Self {
                verdict: None,
                timed_out: result.verdict == sany::SanyVerdict::Timeout,
                note: Some(why),
            },
        }
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

    /// Whether the counterexample in `result` validates against the spec
    /// (charter rule 2: every counterexample is replayed by a separate trace
    /// validator). The harness computes this itself with `dive-trace`
    /// ([`crate::trace_check`]); it never takes the implementation's word.
    /// An implementation that is not an oracle cannot `agree` on a violation
    /// without it. `golden` is the oracle's result for the same model. The
    /// default is `false`.
    fn trace_validated(
        &self,
        _req: &ModelRequest<'_>,
        _result: &OracleResult,
        _golden: &OracleResult,
    ) -> crate::trace_check::TraceCheck {
        crate::trace_check::TraceCheck::no(format!(
            "{} has no counterexample the harness can validate",
            self.name()
        ))
    }

    /// Parses one module (corpus-relative path) the way full SANY does:
    /// parsing, semantic analysis and level checking.
    ///
    /// # Errors
    /// Only for harness failures.
    fn parse_module(
        &self,
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<ImplResult<SanyResult>>;

    /// Parses only the module's own file and gives SANY's parse-phase
    /// verdict for it (`run --sany-phase parse`). The default refuses.
    ///
    /// # Errors
    /// Only for harness failures.
    fn parse_only(
        &self,
        _corpus_root: &Path,
        _module: &str,
        _timeout: Duration,
    ) -> Result<ImplResult<ParseResult>> {
        Ok(ImplResult::Unsupported(format!(
            "{} has no parse-only mode",
            self.name()
        )))
    }

    /// `Some(reason)` when the implementation cannot check any model at all.
    /// Every selected model is then `unsupported` with that reason, whether
    /// or not it has a golden: the refusal is the implementation's answer.
    fn models_unsupported(&self) -> Option<String> {
        None
    }

    /// Provenance of the `dive` binary under test, for the report.
    fn dive_provenance(&self) -> Option<crate::dive::DiveProvenance> {
        None
    }
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
    Ok(run_oracle_model_traced(env, req)?.result)
}

/// One model run on an oracle: the result, and the command line of the
/// trace pass if one ran (recorded in golden provenance).
#[derive(Debug, Clone)]
pub struct OracleModelRun {
    /// The classified result. Its `command` and `elapsed` are the clean
    /// run's.
    pub result: ImplResult<OracleResult>,
    /// The `-dumpTrace json` trace pass's command line, if it ran.
    pub trace_command: Option<Vec<String>>,
}

/// [`run_oracle_model`], also returning the trace pass's command line.
///
/// The model is run *without* `-dumpTrace`, and that run alone decides the
/// verdict, counts, errors and the counterexample's states (issue #35).
/// Only when it ends with a counterexample is the model run again with
/// `-dumpTrace json`, for the JSON document; see
/// [`classify_with_trace_pass`].
///
/// # Errors
/// Only for harness failures.
pub fn run_oracle_model_traced(env: &OracleEnv, req: &ModelRequest<'_>) -> Result<OracleModelRun> {
    let mode = match req.model.mode {
        ModelMode::Exhaustive => CheckMode::Exhaustive,
        ModelMode::Simulate { traces } => CheckMode::Simulate { traces },
        ModelMode::Generate | ModelMode::Symbolic => {
            return Ok(OracleModelRun {
                result: ImplResult::Unsupported(format!(
                    "{:?} models are out of scope",
                    req.model.mode
                )),
                trace_command: None,
            });
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
        seed: 0,
        lncheck: "final",
        fp: 0,
        dump_trace: false,
    };
    let inv = env.run_tlc(&job)?;
    let (result, trace) = classify_with_trace_pass(env, &job, &inv)?;
    Ok(OracleModelRun {
        result: ImplResult::Completed {
            result,
            command: inv.command,
            elapsed: inv.outcome.elapsed,
        },
        trace_command: trace.map(|t| t.command),
    })
}

fn run_facts<'a>(
    inv: &'a Invocation,
    mode: CheckMode,
    trace_json: Option<&'a str>,
) -> RunFacts<'a> {
    RunFacts {
        exit_code: inv.outcome.exit_code,
        timed_out: inv.outcome.timed_out,
        trace_json,
        simulate: matches!(mode, CheckMode::Simulate { .. }),
        stderr: Some(&inv.outcome.stderr),
    }
}

/// Classifies a clean run (`clean`, made from `job` without `-dumpTrace`)
/// and, when it ends with a counterexample, attaches TLC's `-dumpTrace json`
/// document from a second, *trace* run. Returns the result and the trace
/// run, if one was made.
///
/// Why two runs (issue #35, ADR 0003 amendment): `-dumpTrace json` makes TLC
/// extend the root module with `_JsonTrace`, which loads `TLCExt`, `Json`
/// and `Integers`, so `Integers`' Java overrides replace `Naturals`' and the
/// run no longer shows TLC as users run it. Everything in the result
/// therefore comes from the clean run. The trace run contributes only the
/// JSON document, and only when its own message stream reproduces the clean
/// run's counterexample exactly: the same verdict, deciding code and
/// violated property, and the same trace (length, ending, and every state's
/// label and printed value, byte for byte). On any mismatch the clean result
/// stands as it is and only the document is marked unavailable, with the
/// difference ([`merge_trace_pass`]); JSON is never taken from a run that
/// disagrees. The JSON's length is otherwise checked against the clean
/// stream by [`classify`] as before.
///
/// The trace run is made only for a single-worker run of an oracle with
/// `-dumpTrace` whose clean result is an oracle fact with a counterexample.
/// When TLC writes no JSON for it (assertion, evaluation and overflow
/// errors), the clean run's states from the message stream are kept.
///
/// # Errors
/// Only for harness failures of the trace run.
pub fn classify_with_trace_pass(
    env: &OracleEnv,
    job: &TlcJob<'_>,
    clean: &Invocation,
) -> Result<(OracleResult, Option<Invocation>)> {
    let parsed = messages::parse_owned(&clean.outcome.stdout);
    let transcript = parsed.transcript();
    let result = classify_owned(&parsed, &run_facts(clean, job.mode, None));
    let wants_trace = env.capabilities.dump_trace_json
        && job.workers == 1
        && result.trace.is_some()
        && result.verdict.is_oracle_fact();
    if !wants_trace {
        return Ok((result, None));
    }
    let traced = env.run_tlc(&TlcJob {
        dump_trace: true,
        ..job.clone()
    })?;
    let Some(json) = traced.trace_json.as_deref() else {
        return Ok((result, Some(traced)));
    };
    let traced_result = classify(
        &messages::parse(&traced.outcome.stdout),
        &run_facts(&traced, job.mode, None),
    );
    let merged = merge_trace_pass(
        transcript,
        &run_facts(clean, job.mode, None),
        result,
        &traced_result,
        json,
    );
    Ok((merged, Some(traced)))
}

/// Combines a clean run's result with a trace pass's JSON document.
///
/// `clean` is the clean run classified without JSON, and `traced` the trace
/// pass's result, also classified without JSON. When the trace pass
/// reproduces the clean counterexample ([`trace_pass_mismatch`] finds
/// nothing), the clean run is reclassified with `json`, which also checks
/// the document's state count against the clean stream. Otherwise the clean
/// result is kept whole (verdict, counts, error, violated property, and the
/// printed states that give the trace length), and only the document is
/// marked unavailable in `Trace::json_unavailable`, with the reason. JSON is
/// never taken from, or rebuilt for, a run that diverged.
#[must_use]
pub fn merge_trace_pass(
    clean_transcript: &messages::Transcript,
    clean_facts: &RunFacts<'_>,
    clean: OracleResult,
    traced: &OracleResult,
    json: &str,
) -> OracleResult {
    if let Some(why) = trace_pass_mismatch(&clean, traced) {
        let mut clean = clean;
        if let Some(trace) = clean.trace.as_mut() {
            trace.json_unavailable = Some(format!(
                "-dumpTrace json trace pass diverged from the clean run: {why}"
            ));
        }
        return clean;
    }
    classify(
        clean_transcript,
        &RunFacts {
            trace_json: Some(json),
            ..clean_facts.clone()
        },
    )
}

/// Why a trace run's result cannot vouch for the clean run's
/// counterexample, if it cannot. Both results must be classified without
/// JSON, so that their traces carry the printed states.
#[must_use]
pub fn trace_pass_mismatch(clean: &OracleResult, traced: &OracleResult) -> Option<String> {
    if clean.verdict != traced.verdict {
        return Some(format!(
            "verdict {} vs {}",
            clean.verdict.as_str(),
            traced.verdict.as_str()
        ));
    }
    if clean.verdict_code != traced.verdict_code {
        return Some(format!(
            "deciding code {:?} vs {:?}",
            clean.verdict_code, traced.verdict_code
        ));
    }
    if clean.violated_property != traced.violated_property {
        return Some(format!(
            "violated property {:?} vs {:?}",
            clean.violated_property, traced.violated_property
        ));
    }
    match (&clean.trace, &traced.trace) {
        (Some(a), Some(b)) if a.length != b.length => {
            Some(format!("trace length {} vs {}", a.length, b.length))
        }
        (Some(a), Some(b)) if a.ending != b.ending => {
            Some(format!("trace ending {:?} vs {:?}", a.ending, b.ending))
        }
        (Some(a), Some(b)) if a != b => Some(
            a.steps
                .iter()
                .flatten()
                .zip(b.steps.iter().flatten())
                .position(|(x, y)| x != y)
                .map_or_else(
                    || "trace content differs".to_owned(),
                    |i| format!("trace state {} differs", i + 1),
                ),
        ),
        (Some(_), Some(_)) | (None, None) => None,
        (a, b) => Some(format!("trace present {} vs {}", a.is_some(), b.is_some())),
    }
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

    fn parse_only(
        &self,
        corpus_root: &Path,
        module: &str,
        timeout: Duration,
    ) -> Result<ImplResult<ParseResult>> {
        Ok(
            match run_oracle_sany(&self.env, corpus_root, module, timeout)? {
                ImplResult::Unsupported(why) => ImplResult::Unsupported(why),
                ImplResult::Completed {
                    result,
                    command,
                    elapsed,
                } => ImplResult::Completed {
                    result: ParseResult::from_sany(&result, module),
                    command,
                    elapsed,
                },
            },
        )
    }
}

/// An external tool described by a TOML file (`conformance/adapters/` holds
/// the competitors measured in `docs/survey/competitive.md`):
///
/// ```toml
/// name = "tla2"
/// source = "joweeba/tla2 @ 5e828d3 (Apache-2.0)"   # optional provenance
/// # Placeholders: {module} {config} {workers} {corpus} {library} {scratch}
/// check = ["tla2", "check", "{module}", "--config", "{config}", "--workers", "{workers}"]
/// parse = ["tla2", "parse", "{module}"]          # optional
/// library = "env"                                  # optional: flag | env | classpath | copy-beside-spec
/// max_workers = 1                                  # optional: single-threaded tools
/// refused_exit_codes = [2]                         # optional: exit codes that mean "refused or error"
/// [env]                                            # optional, values are templates
/// TLA_PATH = "{library}"
/// [exit_codes]                                     # exit code -> verdict
/// 0 = "success"
/// 12 = "safety-violation"
/// [[rules]]                                        # optional; first match wins, before exit_codes
/// pattern = "^Error: Invariant"                    # a regex over output lines, or:
/// # json = "/result/error_type"  equals = "runtime_error"
/// stream = "stdout"                                # stdout | stderr | any (default)
/// verdict = "safety-violation"                     # or: refused = true
/// [counts]                                         # optional
/// distinct_states = { pattern = '([0-9,]+) distinct states found' }
/// ```
///
/// The verdict is taken from the first matching rule, else from
/// `exit_codes`; an exit code in `refused_exit_codes` is a refusal, and
/// anything else is unclassified, never a guess.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalAdapter {
    /// Tool name.
    pub name: String,
    /// Where the tool comes from (repository, commit, licence).
    #[serde(default)]
    pub source: Option<String>,
    /// Command template for model checking.
    pub check: Vec<String>,
    /// Command template for parsing.
    #[serde(default)]
    pub parse: Option<Vec<String>>,
    /// How the tool finds module libraries (CommunityModules, TLAPS).
    #[serde(default)]
    pub library: Option<LibraryMode>,
    /// The most workers the tool can use; larger requests are refused.
    #[serde(default)]
    pub max_workers: Option<u32>,
    /// Environment variables to set, as templates.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Exit code to verdict map.
    pub exit_codes: BTreeMap<String, Verdict>,
    /// Exit codes that mean the tool refused or failed without a verdict.
    #[serde(default)]
    pub refused_exit_codes: Vec<i32>,
    /// Output rules, tried in order before `exit_codes`.
    #[serde(default)]
    pub rules: Vec<OutputRule>,
    /// How to read state counts from the output.
    #[serde(default)]
    pub counts: BTreeMap<String, Extract>,
    /// Options the tool runs under (`--impl-option`); not part of the TOML.
    #[serde(skip)]
    pub options: Vec<ImplOption>,
}

/// How an external tool finds module libraries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LibraryMode {
    /// A command-line flag naming `{library}` (in `check`).
    Flag,
    /// An environment variable naming `{library}` (in `env`).
    Env,
    /// A Java classpath, as TLC.
    Classpath,
    /// None: library modules are copied beside the spec.
    CopyBesideSpec,
}

/// Which output stream a rule reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
    /// Either.
    #[default]
    Any,
}

/// Where a value is read from: a regex over output lines (its first capture
/// group, for counts), or a JSON pointer into the tool's JSON output.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extract {
    /// A regular expression over output lines.
    #[serde(default)]
    pub pattern: Option<String>,
    /// A JSON pointer (RFC 6901) into the tool's JSON output.
    #[serde(default)]
    pub json: Option<String>,
}

/// One output rule of an [`ExternalAdapter`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRule {
    /// The stream `pattern` reads.
    #[serde(default)]
    pub stream: Stream,
    /// A regular expression over output lines.
    #[serde(default)]
    pub pattern: Option<String>,
    /// A JSON pointer into the tool's JSON output.
    #[serde(default)]
    pub json: Option<String>,
    /// With `json`: the value the pointer must hold.
    #[serde(default)]
    pub equals: Option<String>,
    /// The verdict the rule gives.
    #[serde(default)]
    pub verdict: Option<Verdict>,
    /// The rule means the tool refused or failed without a verdict.
    #[serde(default)]
    pub refused: bool,
}

/// Placeholders a command or environment template may use.
const PLACEHOLDERS: &[&str] = &[
    "module", "config", "workers", "corpus", "library", "scratch",
];

/// Counts an adapter may extract, named as in [`crate::tlc::result::Counts`].
const COUNT_FIELDS: &[&str] = &["distinct_states", "states_generated", "depth"];

/// Fails unless every `{...}` in `template` is a known placeholder.
fn check_template(template: &str) -> std::result::Result<(), String> {
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        let after = &rest[i + 1..];
        let j = after
            .find('}')
            .ok_or_else(|| format!("unclosed `{{` in `{template}`"))?;
        let name = &after[..j];
        if !PLACEHOLDERS.contains(&name) {
            return Err(format!("unknown placeholder `{{{name}}}` in `{template}`"));
        }
        rest = &after[j + 1..];
    }
    Ok(())
}

fn check_source(
    what: &str,
    pattern: Option<&String>,
    json: Option<&String>,
) -> std::result::Result<(), String> {
    match (pattern, json) {
        (Some(p), None) if !p.is_empty() => Ok(()),
        (None, Some(j)) if j.starts_with('/') => Ok(()),
        (None, Some(j)) => Err(format!("{what}: JSON pointer `{j}` must start with `/`")),
        _ => Err(format!(
            "{what}: give exactly one of a non-empty `pattern` or `json`"
        )),
    }
}

impl ExternalAdapter {
    /// Loads and validates an adapter description.
    ///
    /// # Errors
    /// Fails on unreadable or invalid files: an empty `check`, an unknown
    /// placeholder, a non-integer exit code, a code both mapped and refused,
    /// a malformed rule (it needs one of `pattern` or `json`, `equals` only
    /// with `json`, and one of `verdict` or `refused = true`), an unknown
    /// count, or a library mode whose `{library}` placeholder is missing.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
        let a: Self =
            toml::from_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display())))?;
        a.validate()
            .map_err(|why| Error::Config(format!("{}: {why}", path.display())))?;
        Ok(a)
    }

    fn validate(&self) -> std::result::Result<(), String> {
        if self.check.is_empty() {
            return Err("`check` is empty".to_owned());
        }
        for t in self
            .check
            .iter()
            .chain(self.parse.iter().flatten())
            .chain(self.env.values())
        {
            check_template(t)?;
        }
        if self.parse.as_ref().is_some_and(Vec::is_empty) {
            return Err("`parse` is empty".to_owned());
        }
        for k in self.exit_codes.keys() {
            let code = k
                .parse::<i32>()
                .map_err(|_| format!("exit code `{k}` is not an integer"))?;
            if self.refused_exit_codes.contains(&code) {
                return Err(format!("exit code {code} is both mapped and refused"));
            }
        }
        for (i, r) in self.rules.iter().enumerate() {
            let what = format!("rule {}", i + 1);
            check_source(&what, r.pattern.as_ref(), r.json.as_ref())?;
            if r.equals.is_some() && r.json.is_none() {
                return Err(format!("{what}: `equals` applies to `json` rules only"));
            }
            if r.json.is_some() && r.equals.is_none() {
                return Err(format!("{what}: a `json` rule needs `equals`"));
            }
            if r.json.is_some() && r.stream != Stream::Any {
                return Err(format!("{what}: `stream` applies to `pattern` rules only"));
            }
            match (r.verdict, r.refused) {
                (Some(v), false) if v.is_oracle_fact() => {}
                (Some(_), false) => return Err(format!("{what}: not a real verdict")),
                (None, true) => {}
                _ => return Err(format!("{what}: give one of `verdict` or `refused = true`")),
            }
        }
        for (k, e) in &self.counts {
            if !COUNT_FIELDS.contains(&k.as_str()) {
                return Err(format!(
                    "unknown count `{k}` (known: {})",
                    COUNT_FIELDS.join(", ")
                ));
            }
            check_source(&format!("count `{k}`"), e.pattern.as_ref(), e.json.as_ref())?;
        }
        let uses = |needle: &str| {
            self.check
                .iter()
                .chain(self.env.values())
                .any(|t| t.contains(needle))
        };
        match self.library {
            Some(LibraryMode::Flag) if !self.check.iter().any(|t| t.contains("{library}")) => {
                return Err("library = \"flag\" needs `{library}` in `check`".to_owned());
            }
            Some(LibraryMode::Env) if !self.env.values().any(|t| t.contains("{library}")) => {
                return Err("library = \"env\" needs `{library}` in `env`".to_owned());
            }
            Some(LibraryMode::CopyBesideSpec) | None if uses("{library}") => {
                return Err("`{library}` is used but no library mode passes it".to_owned());
            }
            _ => {}
        }
        if self.max_workers == Some(0) {
            return Err("`max_workers` must be at least 1".to_owned());
        }
        Ok(())
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

    const FULL: &str = r#"
name = "t"
source = "o/t @ 0000000 (MIT)"
check = ["t", "-lib", "{library}", "-w", "{workers}", "{module}", "{config}"]
library = "flag"
refused_exit_codes = [2]
[exit_codes]
0 = "success"
[[rules]]
pattern = "^Error: Invariant"
verdict = "safety-violation"
[[rules]]
json = "/result/error_type"
equals = "runtime_error"
refused = true
[counts]
distinct_states = { pattern = '([0-9,]+) distinct states found' }
"#;

    #[test]
    fn adapter_schema_validates() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("adapter-schema")?;
        let p = dir.path().join("a.toml");
        std::fs::write(&p, FULL)?;
        let a = ExternalAdapter::load(&p)?;
        assert_eq!(a.rules.len(), 2);
        assert_eq!(a.library, Some(LibraryMode::Flag));
        for (from, to) in [
            ("{workers}", "{threads}"),
            ("{module}", "{module"),
            ("refused_exit_codes = [2]", "refused_exit_codes = [0]"),
            ("verdict = \"safety-violation\"", "verdict = \"timeout\""),
            ("verdict = \"safety-violation\"", ""),
            ("refused = true", "refused = true\nverdict = \"success\""),
            ("equals = \"runtime_error\"\n", ""),
            ("json = \"/result/error_type\"", "json = \"result\""),
            (
                "pattern = \"^Error: Invariant\"",
                "pattern = \"^E\"\njson = \"/x\"",
            ),
            ("distinct_states = {", "distinct = {"),
            ("\"-lib\", \"{library}\", ", ""),
            ("library = \"flag\"", "library = \"env\""),
            ("library = \"flag\"", "library = \"nfs\""),
            ("source =", "origin ="),
        ] {
            assert!(FULL.contains(from), "{from}");
            std::fs::write(&p, FULL.replace(from, to))?;
            assert!(ExternalAdapter::load(&p).is_err(), "{from} -> {to}");
        }
        Ok(())
    }

    #[test]
    fn repository_adapters_parse() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/adapters");
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let a = ExternalAdapter::load(&path)?;
                assert!(a.source.is_some(), "{} records its source", path.display());
                names.push(a.name);
            }
        }
        names.sort();
        assert_eq!(names, ["tla-mc", "tla-rs", "tla2", "tlago", "tlaplusplus"]);
        Ok(())
    }

    fn violation(states: &[&str]) -> OracleResult {
        classify(
            &messages::parse(&violation_stream(states)),
            &RunFacts {
                exit_code: Some(12),
                ..RunFacts::default()
            },
        )
    }

    #[test]
    fn divergent_trace_pass_keeps_the_clean_result_without_json() -> std::result::Result<(), String>
    {
        let stream = violation_stream(&["x = 0", "x = 1"]);
        let transcript = messages::parse(&stream);
        let facts = RunFacts {
            exit_code: Some(12),
            ..RunFacts::default()
        };
        let clean = classify(&transcript, &facts);
        let json = r#"{"counterexample":{"state":[[1,{"x":0}],[2,{"x":2}]]}}"#;

        // Agreeing trace pass: its document is attached.
        let merged = merge_trace_pass(&transcript, &facts, clean.clone(), &clean, json);
        let t = merged.trace.as_ref().ok_or("no trace")?;
        assert!(t.json.is_some() && t.steps.is_none() && t.json_unavailable.is_none());
        assert_eq!(merged.verdict, Verdict::SafetyViolation);

        // Diverging trace pass: the clean result stands, the document does not.
        let traced = violation(&["x = 0", "x = 2"]);
        let merged = merge_trace_pass(&transcript, &facts, clean.clone(), &traced, json);
        assert_eq!(merged.verdict, clean.verdict);
        assert_eq!(merged.verdict_code, clean.verdict_code);
        assert_eq!(merged.violated_property, clean.violated_property);
        assert_eq!(merged.counts, clean.counts);
        assert_eq!(merged.note, clean.note);
        let t = merged.trace.ok_or("no trace")?;
        assert_eq!(t.length, 2);
        assert!(t.json.is_none(), "no JSON from a diverging run");
        assert_eq!(t.steps, clean.trace.and_then(|c| c.steps));
        assert_eq!(
            t.json_unavailable.as_deref(),
            Some("-dumpTrace json trace pass diverged from the clean run: trace state 2 differs")
        );
        Ok(())
    }

    fn violation_stream(states: &[&str]) -> String {
        use std::fmt::Write as _;
        let mut out =
            "@!@!@STARTMSG 2110:1 @!@!@\nInvariant Inv is violated.\n@!@!@ENDMSG 2110 @!@!@\n"
                .to_owned();
        for (i, s) in states.iter().enumerate() {
            let label = if i == 0 {
                "<Initial predicate>"
            } else {
                "<Next>"
            };
            let _ = write!(
                out,
                "@!@!@STARTMSG 2217:4 @!@!@\n{}: {label}\n{s}\n@!@!@ENDMSG 2217 @!@!@\n",
                i + 1
            );
        }
        out
    }

    #[test]
    fn trace_pass_must_reproduce_the_clean_counterexample() {
        let clean = violation(&["x = 0", "x = 1"]);
        assert_eq!(
            trace_pass_mismatch(&clean, &violation(&["x = 0", "x = 1"])),
            None
        );
        assert_eq!(
            trace_pass_mismatch(&clean, &violation(&["x = 0"])).as_deref(),
            Some("trace length 2 vs 1")
        );
        assert_eq!(
            trace_pass_mismatch(&clean, &violation(&["x = 0", "x = 2"])).as_deref(),
            Some("trace state 2 differs")
        );
        let mut other = violation(&["x = 0", "x = 1"]);
        other.violated_property = Some("Other".to_owned());
        assert!(
            trace_pass_mismatch(&clean, &other).is_some_and(|w| w.starts_with("violated property"))
        );
        other.verdict = Verdict::Deadlock;
        assert!(trace_pass_mismatch(&clean, &other).is_some_and(|w| w.starts_with("verdict")));
    }
}
