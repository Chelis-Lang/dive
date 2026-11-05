//! Semantic analysis reports (`dive analyze`).

use dive_sema::model::{DefKind, Model, NodeKind};
use dive_sema::{Analysis, FileId, Options, SearchPath, Severity};
use dive_syntax::diagnostics::line_col;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Version of the JSON document produced by [`AnalyzeReport::to_json`].
///
/// It follows the conventions of [`crate::PARSE_SCHEMA_VERSION`]: the same
/// diagnostic and span shapes, with a `file` on every diagnostic because an
/// analysis spans several files. Bump it on any incompatible change.
pub const ANALYZE_SCHEMA_VERSION: u32 = crate::REPORT_SCHEMA_VERSION;

/// Where `dive analyze` looks for modules besides the root file's directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnalyzeOptions {
    /// Searched before the standard modules (SANY's `TLA-Library`).
    pub library_paths: Vec<PathBuf>,
    /// Searched after the standard modules (where TLC finds modules on its
    /// Java classpath, such as the `CommunityModules` jar).
    pub after_standard: Vec<PathBuf>,
}

/// The outcome of analysing a specification.
#[derive(Debug, Clone)]
pub struct AnalyzeReport {
    file: String,
    analysis: Analysis,
}

/// Analyses the specification rooted at `path`: module resolution, name
/// resolution, arity and level checking, as SANY's `-error-codes` run.
///
/// # Errors
///
/// [`crate::ApiError`] if the root file cannot be read.
pub fn analyze_file(
    path: &Path,
    options: &AnalyzeOptions,
) -> Result<AnalyzeReport, crate::ApiError> {
    let opts = Options {
        search: SearchPath {
            library_paths: options.library_paths.clone(),
            after_standard: options.after_standard.clone(),
        },
        ..Options::default()
    };
    let shown = path.display().to_string();
    let analysis = dive_sema::analyze_file(path, &opts).map_err(|error| crate::ApiError::Io {
        path: shown.clone(),
        error,
    })?;
    Ok(AnalyzeReport {
        file: shown,
        analysis,
    })
}

/// Analyses source text as the root module; other modules come only from
/// `options` and the standard modules.
#[must_use]
pub fn analyze_source(file: &str, source: &str, options: &AnalyzeOptions) -> AnalyzeReport {
    let opts = Options {
        search: SearchPath {
            library_paths: options.library_paths.clone(),
            after_standard: options.after_standard.clone(),
        },
        ..Options::default()
    };
    AnalyzeReport {
        file: file.to_owned(),
        analysis: dive_sema::analyze_source(file, source, &opts),
    }
}

/// One diagnostic of either kind, flattened for output.
struct Flat {
    code: String,
    severity: Severity,
    message: String,
    help: Option<String>,
    sany: Option<u16>,
    file: Option<FileId>,
    range: Option<dive_syntax::TextRange>,
    related: Vec<(FileId, dive_syntax::TextRange, String)>,
}

impl AnalyzeReport {
    /// A report of an analysis made elsewhere (`dive check`).
    pub(crate) fn from_analysis(file: String, analysis: Analysis) -> Self {
        Self { file, analysis }
    }

    /// Whether SANY would accept the specification.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.analysis.accepted()
    }

    /// The underlying analysis, including the semantic model.
    #[must_use]
    pub fn analysis(&self) -> &Analysis {
        &self.analysis
    }

    /// The loaded modules' names in analysis order (root last).
    #[must_use]
    pub fn modules(&self) -> Vec<String> {
        let m = self.analysis.model();
        m.modules()
            .iter()
            .filter_map(|&id| m.symbol_name(id).map(str::to_owned))
            .collect()
    }

    fn flat(&self) -> Vec<Flat> {
        let mut out = Vec::new();
        for (f, d) in self.analysis.syntax_diagnostics() {
            out.push(Flat {
                code: d.code.as_str().to_owned(),
                severity: if d.is_error() {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                message: d.message.clone(),
                help: d.help.clone(),
                sany: None,
                file: Some(*f),
                range: Some(d.range),
                related: Vec::new(),
            });
        }
        for d in self.analysis.diagnostics() {
            out.push(Flat {
                code: d.code.as_str().to_owned(),
                severity: d.severity,
                message: d.message.clone(),
                help: crate::diagnostics::semantic_help(d.code, d.help.as_deref()),
                sany: d.sany,
                file: d.span.map(|s| s.file),
                range: d.span.map(|s| s.range),
                related: d
                    .related
                    .iter()
                    .map(|r| (r.span.file, r.span.range, r.message.clone()))
                    .collect(),
            });
        }
        out
    }

    fn file_name(&self, f: FileId) -> String {
        self.analysis
            .file(f)
            .map_or_else(|| self.file.clone(), |s| s.display.clone())
    }

    fn line_col(&self, f: FileId, at: dive_syntax::TextSize) -> (u32, u32) {
        self.analysis
            .file(f)
            .map_or((0, 0), |s| line_col(&s.text, at))
    }

    /// The diagnostics in a human-readable form, one per line, like
    /// `file:3:7: error[DIVE-E0100] (SANY 4200): message`.
    #[must_use]
    pub fn render_human(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        for d in self.flat() {
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            let loc = match (d.file, d.range) {
                (Some(fid), Some(range)) => {
                    let (line, col) = self.line_col(fid, range.start());
                    format!("{}:{line}:{col}", self.file_name(fid))
                }
                _ => self.file.clone(),
            };
            let sany = d.sany.map(|c| format!(" (SANY {c})")).unwrap_or_default();
            let _ = writeln!(s, "{loc}: {sev}[{}]{sany}: {}", d.code, d.message);
            for (f, r, m) in &d.related {
                let (l, c) = self.line_col(*f, r.start());
                let _ = writeln!(s, "  note: {}:{l}:{c}: {m}", self.file_name(*f));
            }
            if let Some(h) = &d.help {
                let _ = writeln!(s, "  help: {h}");
            }
        }
        s
    }

    fn span_json(&self, f: FileId, r: dive_syntax::TextRange) -> Value {
        let (line, column) = self.line_col(f, r.start());
        let (end_line, end_column) = self.line_col(f, r.end());
        json!({
            "start": u32::from(r.start()),
            "end": u32::from(r.end()),
            "line": line,
            "column": column,
            "end_line": end_line,
            "end_column": end_column,
        })
    }

    /// The versioned JSON report.
    ///
    /// Shape (schema version 3):
    ///
    /// ```text
    /// { "schema_version": 3, "command": "analyze", "file": "...",
    ///   "accepted": bool, "module": "Root" | null, "modules": ["Dep", ..., "Root"],
    ///   "files": [ { "id", "path", "origin": "file"|"standard"|"monolith"|"text" } ],
    ///   "diagnostics": [ { "code", "severity", "message", "help", "sany_code",
    ///                      "file", "span": { "start", "end", "line", "column",
    ///                                        "end_line", "end_column" } | null,
    ///                      "related": [ { "file", "span", "message" } ] } ],
    ///   "definitions": [ { "name", "arity", "level" } ]   // root module, when accepted
    /// }
    /// ```
    ///
    /// `file` fields are indexes into `files`. Offsets are bytes; lines and
    /// columns are 1-based, columns count Unicode scalar values. Levels are
    /// `"constant"`, `"state"`, `"action"` or `"temporal"`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let files: Vec<Value> = self
            .analysis
            .files()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let origin = match &f.origin {
                    dive_sema::Origin::File(_) => "file",
                    dive_sema::Origin::Standard => "standard",
                    dive_sema::Origin::Bundled => "bundled",
                    dive_sema::Origin::Monolith(_) => "monolith",
                    dive_sema::Origin::Text => "text",
                };
                json!({ "id": i, "path": f.display, "origin": origin })
            })
            .collect();
        let diagnostics: Vec<Value> = self
            .flat()
            .into_iter()
            .map(|d| {
                let span = match (d.file, d.range) {
                    (Some(f), Some(r)) => self.span_json(f, r),
                    _ => Value::Null,
                };
                let related: Vec<Value> = d
                    .related
                    .iter()
                    .map(|(f, r, m)| json!({ "file": f.index(), "span": self.span_json(*f, *r), "message": m }))
                    .collect();
                json!({
                    "code": d.code,
                    "severity": match d.severity { Severity::Error => "error", Severity::Warning => "warning" },
                    "message": d.message,
                    "help": d.help,
                    "sany_code": d.sany,
                    "file": d.file.map(FileId::index),
                    "span": span,
                    "related": related,
                })
            })
            .collect();
        let modules = self.modules();
        json!({
            "schema_version": ANALYZE_SCHEMA_VERSION,
            "command": "analyze",
            "file": self.file,
            "accepted": self.accepted(),
            "module": modules.last(),
            "modules": modules,
            "files": files,
            "diagnostics": diagnostics,
            "definitions": if self.accepted() { definitions(self.analysis.model()) } else { Vec::new() },
        })
    }
}

/// The root module's own top-level definitions with their levels.
fn definitions(m: &Model) -> Vec<Value> {
    let Some(root) = m.root() else {
        return Vec::new();
    };
    let NodeKind::Module(md) = &m.node(root).kind else {
        return Vec::new();
    };
    m.context(md.ctx)
        .entries()
        .filter_map(|(_, d)| match &m.node(d).kind {
            NodeKind::OpDef(od) if od.kind == DefKind::UserDefined && od.module == Some(root) => {
                Some(json!({
                    "name": m.name(od.name),
                    "arity": od.arity,
                    "level": level_name(m.level(d)),
                }))
            }
            NodeKind::ThmOrAssumpDef(td) if td.module == Some(root) => Some(json!({
                "name": m.name(td.name),
                "arity": td.params.len(),
                "level": level_name(m.level(d)),
            })),
            _ => None,
        })
        .collect()
}

fn level_name(l: dive_sema::Level) -> &'static str {
    match l {
        dive_sema::Level::Constant => "constant",
        dive_sema::Level::State => "state",
        dive_sema::Level::Action => "action",
        dive_sema::Level::Temporal => "temporal",
    }
}
