//! `dive-api`: the embeddable library API and `cargo test` integration.
//!
//! It exposes everything the `dive` command line does as plain Rust calls,
//! each returning a typed report with a versioned JSON form
//! ([`REPORT_SCHEMA_VERSION`]):
//!
//! - **Parsing** TLA+ source ([`parse_source`], [`parse_file`]) into a lossless
//!   syntax tree, with SANY's syntax errors.
//! - **Model configurations** in TLC's `.cfg` format ([`parse_config_file`],
//!   [`parse_config_source`]).
//! - **Semantic analysis** equivalent to SANY ([`analyze_file`],
//!   [`analyze_source`]).
//! - **PlusCal** translation and checksum rewriting ([`translate_file`],
//!   [`translate_source`], [`rewrite_file`]).
//! - **Model checking** of safety and temporal properties ([`check_file`] with a
//!   [`CheckRequest`], returning a [`CheckReport`]), including run budgets,
//!   checkpoints ([`checkpoint`]) and several workers.
//! - **Simulation** with a seed ([`simulate_file`]) and depth-first iterative
//!   deepening ([`dfid_file`]).
//! - **Trace validation**: checking a counterexample against the
//!   specification with the independent validator ([`trace`]), and temporal
//!   checking on its own ([`temporal`]).
//!
//! Anything dive does not implement exactly is reported as
//! [`ApiError::Unsupported`], naming the feature, rather than approximated.
//!
//! # Example
//!
//! ```
//! let report = dive_api::parse_source("Spec.tla", "---- MODULE Spec ----\nx == 1\n====\n");
//! assert!(report.accepted());
//! assert_eq!(report.modules(), ["Spec"]);
//! let json = report.to_json(false);
//! assert_eq!(json["schema_version"], dive_api::PARSE_SCHEMA_VERSION);
//! ```
#![forbid(unsafe_code)]

mod config;
pub mod host_inputs;
/// Shared whole-run policy for symmetry combined with temporal properties.
pub use dive_model::SymmetryLiveness;
pub use host_inputs::HostAccess;
mod publication;
pub use publication::{
    OriginalAstWriteReport, TranslationPublication, prepare_translation_file,
    write_original_ast_file, write_translation_file,
};
mod rewrite;
pub use rewrite::{
    ChecksumPolicy, RewriteOptions, RewriteReport, RewriteRequest, rewrite_file, rewrite_source,
};
mod original_ast;
mod translate;
/// Native PlusCal fairness selection, including pinned translator option spellings.
pub use dive_pluscal::FairnessOption as TranslationFairness;
pub use original_ast::{OriginalAstReport, original_ast_file, original_ast_source};
pub use translate::{
    TRANSLATE_SCHEMA_VERSION, TranslationReport, TranslationRequest, translate_file,
    translate_source,
};

pub mod safety_projection;

pub mod debug;
pub mod diagnostics;
pub mod safety_reuse;
pub mod temporal;
pub mod temporal_projection;
pub mod temporal_reuse;
pub mod trace;

pub use config::{ConfigReport, parse_config_file, parse_config_source};

mod analyze;
mod check;
mod core_checkpoint;
mod core_worker;
pub use core_worker::{CoreWorker, prepare_core_worker};
/// Authenticated native distributed pure-safety options and actual work facts.
pub use dive_explore::distributed::{
    Options as DistributedOptions, Usage as DistributedUsage, WorkerInfo,
};
mod core_ending;
mod core_engine;
#[cfg(test)]
mod core_graph;
mod core_modes;
mod core_native_atoms;
mod core_native_graph;
mod core_native_values;
mod core_random;
mod core_record_types;
mod core_sample;
mod core_temporal;
mod mode_budget;
/// Native safety checkpoint request and format API.
pub mod checkpoint {
    pub use dive_runtime::checkpoint::{
        Action, Counters, MAX_COUNTER, MAX_LEVEL, MAX_PAYLOAD_BYTES, MAX_STATE_BYTES, MAX_STATES,
        Receipt, Request, Result, config_source_hash, owned_identity, owned_identity_seed_fp,
        read_payload, write_payload,
    };
}
mod dfid;
mod dfid_admission;
pub use dfid::{DFID_SCHEMA_VERSION, DfidReport, DfidRequest, dfid_file};
mod simulate;
mod simulate_alias;
mod simulate_export;
mod simulate_temporal;
mod simulate_traces;
pub use dive_core::unbounded::MAX_MEAN as MAX_SAMPLE_MEAN;
pub use dive_sim::{DEFAULT_SAMPLE_MEAN, KEEP_ALL_TRACES, SampleUnbounded, UNLIMITED_TRACES};
pub use simulate::{SIMULATE_SCHEMA_VERSION, SimulateReport, SimulateRequest, simulate_file};
pub use simulate_export::{TraceFormat, trace_file_name};

pub use check::{
    BudgetSetting, CHECK_SCHEMA_VERSION, CheckReport, CheckRequest, ExitConvention,
    PeriodicSchedule, Store, TEMPORAL_CHECK_SCHEMA_VERSION, TemporalMode, check_file,
    check_file_checkpoint, check_file_core_opts, check_file_tlc_inputs, exit,
};
/// Parses a seed as TLC's `-seed` does (`dive check --seed`).
pub use dive_stdlib::random::parse_seed;

pub use analyze::{
    ANALYZE_SCHEMA_VERSION, AnalyzeOptions, AnalyzeReport, analyze_file, analyze_source,
};

use dive_syntax::ast::Unit;
use dive_syntax::diagnostics::line_col;
use dive_syntax::{Diagnostic, Parse, Severity, SyntaxElement, SyntaxNode};
use serde_json::{Value, json};
use std::path::Path;

/// Version of every public operation's JSON report.
///
/// Reports use `command` to identify the operation; check reports also carry
/// `check_kind` to distinguish safety and temporal report families. Neither
/// discriminator establishes execution or proof. The native `dive-trace`
/// payload has its own independent format version.
///
/// Incompatible field, type or outcome changes require a new version, a
/// CHANGELOG entry and advance notice to known consumers. Optional fields and
/// diagnostic codes may be added; readers must reject unknown outcomes and
/// versions. The compatibility policy is in the
/// [reports and errors guide](https://github.com/Chelis-Lang/dive/blob/main/docs/user/reports-and-errors.md).
pub const REPORT_SCHEMA_VERSION: u32 = 4;

/// Version of the JSON document produced by [`ParseReport::to_json`].
///
/// Bump it on any incompatible change to the document's shape.
pub const PARSE_SCHEMA_VERSION: u32 = REPORT_SCHEMA_VERSION;

/// The outcome of parsing one file.
#[derive(Debug, Clone)]
pub struct ParseReport {
    file: String,
    source: String,
    parse: Parse,
    diagnostics: Vec<Diagnostic>,
}

/// Errors from the API that are not diagnostics about the source.
#[derive(Debug)]
#[non_exhaustive]
pub enum ApiError {
    /// A requested run budget could not be enforced; no check ran.
    BudgetStartup(dive_runtime::budget::StartError),
    /// Required memory observation failed; no verdict is produced.
    BudgetMeasurement(dive_runtime::budget::MeasurementError),
    /// The file could not be read.
    Io {
        /// The file.
        path: String,
        /// The underlying error.
        error: std::io::Error,
    },
    /// The request needs a feature dive does not implement yet. No result
    /// is produced (soundness contract rule 3).
    Unsupported {
        /// The feature, named for the user.
        feature: String,
    },
    /// The input is rejected: the specification or configuration has
    /// errors, or the model cannot be bound (TLC's error).
    Rejected {
        /// What is wrong.
        message: String,
    },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BudgetStartup(error) => error.fmt(f),
            Self::BudgetMeasurement(error) => error.fmt(f),
            Self::Io { path, error } => write!(f, "cannot read {path}: {error}"),
            Self::Unsupported { feature } => write!(f, "unsupported: {feature}"),
            Self::Rejected { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for ApiError {}

/// Parses TLA+ source text. `file` names the source in reports and is used
/// for SANY's check that the module name matches the file name.
#[must_use]
pub fn parse_source(file: &str, source: &str) -> ParseReport {
    let parse = dive_syntax::parse_module(source);
    let mut diagnostics = parse.diagnostics().to_vec();
    let base = Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file);
    diagnostics.extend(parse.check_file_name(base));
    for diagnostic in &mut diagnostics {
        diagnostic.help.get_or_insert_with(|| {
            diagnostics::syntax_explanation(diagnostic.code)
                .remediation
                .to_owned()
        });
    }
    ParseReport {
        file: file.to_owned(),
        source: source.to_owned(),
        parse,
        diagnostics,
    }
}

/// Reads and parses a TLA+ file.
///
/// Like SANY, which reads files as UTF-8 and replaces malformed bytes, a file
/// that is not valid UTF-8 is decoded with U+FFFD replacement characters and
/// parsed anyway, with a `DIVE-S0007` warning. The CST is then lossless with
/// respect to the decoded text.
///
/// # Errors
///
/// [`ApiError`] if the file cannot be read.
pub fn parse_file(path: &Path) -> Result<ParseReport, ApiError> {
    let shown = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|error| ApiError::Io {
        path: shown.clone(),
        error,
    })?;
    match String::from_utf8(bytes) {
        Ok(source) => Ok(parse_source(&shown, &source)),
        Err(e) => {
            let valid_up_to = e.utf8_error().valid_up_to();
            let source = String::from_utf8_lossy(e.as_bytes()).into_owned();
            let mut report = parse_source(&shown, &source);
            let at = dive_syntax::TextSize::try_from(valid_up_to).unwrap_or_default();
            report.diagnostics.insert(
                0,
                Diagnostic {
                    code: dive_syntax::Code::InvalidUtf8,
                    severity: Severity::Warning,
                    message: "the file is not valid UTF-8; invalid bytes were read as U+FFFD"
                        .to_owned(),
                    range: dive_syntax::TextRange::empty(at),
                    help: Some("save the file as UTF-8".to_owned()),
                },
            );
            Ok(report)
        }
    }
}

impl ParseReport {
    /// Whether SANY would accept the file (no error diagnostics).
    #[must_use]
    pub fn accepted(&self) -> bool {
        !self.diagnostics.iter().any(Diagnostic::is_error)
    }

    /// The parse result.
    #[must_use]
    pub fn parse(&self) -> &Parse {
        &self.parse
    }

    /// All diagnostics, including the file-name check.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Names of the top-level module and every nested module, in order.
    #[must_use]
    pub fn modules(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack: Vec<dive_syntax::ast::Module> = self.parse.module().into_iter().collect();
        while let Some(m) = stack.pop() {
            out.extend(m.name());
            let mut nested: Vec<_> = m
                .units()
                .filter_map(|u| match u {
                    Unit::Module(n) => Some(n),
                    _ => None,
                })
                .collect();
            nested.reverse();
            stack.extend(nested);
        }
        out
    }

    /// The diagnostics in a human-readable form, one per line, like
    /// `file:3:7: error[DIVE-S0012]: message`.
    #[must_use]
    pub fn render_human(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        for d in &self.diagnostics {
            let (line, col) = line_col(&self.source, d.range.start());
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            // Writing to a String cannot fail.
            let _ = writeln!(
                s,
                "{}:{line}:{col}: {sev}[{}]: {}",
                self.file, d.code, d.message
            );
            if let Some(h) = &d.help {
                let _ = writeln!(s, "  help: {h}");
            }
        }
        s
    }

    /// The versioned JSON report. With `cst`, the whole concrete syntax tree
    /// is included under `"cst"`.
    ///
    /// Shape (schema version 3):
    ///
    /// ```text
    /// { "schema_version": 4, "command": "parse", "file": "...",
    ///   "accepted": bool, "module": "Name" | null, "modules": ["Name", ...],
    ///   "diagnostics": [ { "code", "severity", "message", "help",
    ///                      "span": { "start", "end", "line", "column",
    ///                                "end_line", "end_column" } } ],
    ///   "cst": { "kind", "start", "end", "children" | "text" }   // optional
    /// }
    /// ```
    ///
    /// Offsets are bytes; lines and columns are 1-based, columns count
    /// Unicode scalar values.
    #[must_use]
    pub fn to_json(&self, cst: bool) -> Value {
        let diagnostics: Vec<Value> = self
            .diagnostics
            .iter()
            .map(|d| {
                let (line, column) = line_col(&self.source, d.range.start());
                let (end_line, end_column) = line_col(&self.source, d.range.end());
                json!({
                    "code": d.code.as_str(),
                    "severity": match d.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                    },
                    "message": d.message,
                    "help": d.help,
                    "span": {
                        "start": u32::from(d.range.start()),
                        "end": u32::from(d.range.end()),
                        "line": line,
                        "column": column,
                        "end_line": end_line,
                        "end_column": end_column,
                    },
                })
            })
            .collect();
        let modules = self.modules();
        let mut v = json!({
            "schema_version": PARSE_SCHEMA_VERSION,
            "command": "parse",
            "file": self.file,
            "accepted": self.accepted(),
            "module": modules.first(),
            "modules": modules,
            "diagnostics": diagnostics,
        });
        if cst && let Some(obj) = v.as_object_mut() {
            obj.insert("cst".to_owned(), cst_json(&self.parse.syntax()));
        }
        v
    }
}

/// The CST as JSON: nodes have `children`, tokens have `text`.
fn cst_json(root: &SyntaxNode) -> Value {
    // Iterative, so deep trees cannot overflow the stack.
    enum Frame {
        Open(SyntaxNode, Vec<Value>, std::vec::IntoIter<SyntaxElement>),
    }
    let element = |kind: &str, r: dive_syntax::TextRange| -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        m.insert("kind".to_owned(), Value::from(kind));
        m.insert("start".to_owned(), Value::from(u32::from(r.start())));
        m.insert("end".to_owned(), Value::from(u32::from(r.end())));
        m
    };
    let children_of = |n: &SyntaxNode| n.children_with_tokens().collect::<Vec<_>>().into_iter();
    let mut stack = vec![Frame::Open(root.clone(), Vec::new(), children_of(root))];
    loop {
        let Some(Frame::Open(node, mut done, mut rest)) = stack.pop() else {
            return Value::Null;
        };
        match rest.next() {
            Some(SyntaxElement::Token(t)) => {
                let mut m = element(t.kind().name(), t.text_range());
                m.insert("text".to_owned(), Value::from(t.text()));
                done.push(Value::Object(m));
                stack.push(Frame::Open(node, done, rest));
            }
            Some(SyntaxElement::Node(n)) => {
                let it = children_of(&n);
                stack.push(Frame::Open(node, done, rest));
                stack.push(Frame::Open(n, Vec::new(), it));
            }
            None => {
                let mut m = element(node.kind().name(), node.text_range());
                m.insert("children".to_owned(), Value::Array(done));
                let v = Value::Object(m);
                match stack.last_mut() {
                    Some(Frame::Open(_, parent_done, _)) => parent_done.push(v),
                    None => return v,
                }
            }
        }
    }
}

/// Re-exported so API users can work with the tree directly.
pub use dive_syntax;

/// Re-exported so API users can work with the semantic model directly.
pub use dive_sema;

/// Action composition modes (`--action-composition`).
pub use dive_eval::ActionComposition;
/// Bounded transient disk seen-state safety options.
pub use dive_runtime::SeenOptions;
/// Limits for full disk keys, entries, probes and collision summaries.
pub use dive_store::seen::Limits as SeenLimits;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_modules_are_listed() {
        let r = parse_source(
            "A.tla",
            "---- MODULE A ----\n---- MODULE B ----\n====\n---- MODULE C ----\n====\n====\n",
        );
        assert!(r.accepted());
        assert_eq!(r.modules(), ["A", "B", "C"]);
    }

    #[test]
    fn file_name_mismatch_is_an_error() {
        let r = parse_source("dir/B.tla", "---- MODULE A ----\n====\n");
        assert!(!r.accepted());
        assert_eq!(r.diagnostics()[0].code.as_str(), "DIVE-S0006");
    }

    #[test]
    fn invalid_utf8_is_read_like_sany() {
        let dir = std::env::temp_dir().join(format!("dive-api-utf8-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("Latin.tla");
        // `\xe9` (Latin-1 e-acute) inside a comment: SANY reads it as U+FFFD.
        let bytes = b"---- MODULE Latin ----\n(* caf\xe9 *)\nx == 1\n====\n";
        if let Err(e) = std::fs::write(&path, bytes) {
            unreachable!("cannot write {}: {e}", path.display());
        }
        let r = match parse_file(&path) {
            Ok(r) => r,
            Err(e) => unreachable!("{e}"),
        };
        assert!(r.accepted());
        assert_eq!(r.diagnostics()[0].code.as_str(), "DIVE-S0007");
        assert!(!r.diagnostics()[0].is_error());
    }

    #[test]
    fn json_has_schema_version_and_cst() {
        let r = parse_source("M.tla", "---- MODULE M ----\nx == 1 + \n====\n");
        let v = r.to_json(true);
        assert_eq!(v["schema_version"], PARSE_SCHEMA_VERSION);
        assert_eq!(v["accepted"], false);
        assert_eq!(v["diagnostics"][0]["span"]["line"], 3);
        assert_eq!(v["cst"]["kind"], "SOURCE_FILE");
        assert!(r.render_human().contains("error[DIVE-S"));
    }
}
