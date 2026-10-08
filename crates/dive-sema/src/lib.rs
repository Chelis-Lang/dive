//! `dive-sema`: SANY-equivalent semantic analysis.
//!
//! This stage implements module resolution: [`analyze_file`] loads every
//! module the root `EXTENDS` or `INSTANCE`s ([`loader`]) and parses it with
//! `dive-syntax`, as SANY's parse phase does. Semantic analysis and level
//! checking follow in later stages.
#![forbid(unsafe_code)]

pub mod diagnostics;
pub mod loader;
mod pcal_check;
pub mod stdlib;

pub use diagnostics::{Code, Diagnostic, FileId, Related, Severity, Span};
pub use loader::{Origin, SearchPath, SourceFile};

use std::path::Path;

/// Analysis options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Where modules are searched for.
    pub search: SearchPath,
}

/// The result of analysing a specification.
#[derive(Debug, Clone)]
pub struct Analysis {
    files: Vec<SourceFile>,
    diagnostics: Vec<Diagnostic>,
    syntax: Vec<(FileId, dive_syntax::Diagnostic)>,
    loaded: bool,
}

impl Analysis {
    /// Whether every module loads and parses.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.loaded
    }

    /// The module-resolution diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Syntax diagnostics of the loaded files that failed to parse.
    #[must_use]
    pub fn syntax_diagnostics(&self) -> &[(FileId, dive_syntax::Diagnostic)] {
        &self.syntax
    }

    /// The loaded files.
    #[must_use]
    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }
}

/// Reads and loads the specification whose root module is in `path`.
///
/// # Errors
///
/// The root file cannot be read.
pub fn analyze_file(path: &Path, options: &Options) -> std::io::Result<Analysis> {
    let bytes = std::fs::read(path)?;
    let root = SourceFile {
        display: path.display().to_string(),
        origin: Origin::File(path.to_path_buf()),
        text: loader::decode(&bytes),
        parse: dive_syntax::parse_module(""),
    };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let loaded = loader::load(root, Some(name), &options.search);
    let mut syntax = Vec::new();
    for &f in &loaded.parse_failed {
        if let Some(file) = loaded.files.get(f.index()) {
            for d in file.parse.diagnostics() {
                syntax.push((f, d.clone()));
            }
        }
    }
    Ok(Analysis {
        loaded: loaded.ok(),
        diagnostics: loaded.diagnostics.clone(),
        files: loaded.files,
        syntax,
    })
}
