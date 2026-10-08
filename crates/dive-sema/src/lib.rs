//! `dive-sema`: SANY-equivalent semantic analysis.
//!
//! [`analyze_file`] takes a root module file and does what SANY's front end
//! does with `-error-codes`: it loads every module the root `EXTENDS` or
//! `INSTANCE`s ([`loader`]), parses them with `dive-syntax`, builds the
//! semantic graph of each module in dependency order (name resolution,
//! arity, instances and substitutions, labels, subexpression selectors,
//! proofs), and level-checks it. A specification is accepted exactly when
//! the pinned SANY accepts it.
//!
//! The result is an [`Analysis`]: the loaded files, the diagnostics, and the
//! resolved semantic [`model::Model`] that `dive-ir` lowers from.
//!
//! # Example
//!
//! ```
//! use dive_sema::{analyze_source, Options};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\n\
//!            Init == x = 0\nNext == x' = x + 1\n====\n";
//! let a = analyze_source("M.tla", src, &Options::default());
//! assert!(a.accepted());
//! let root = a.model().root().unwrap();
//! assert_eq!(a.model().symbol_name(root), Some("M"));
//!
//! let bad = analyze_source("M.tla", "---- MODULE M ----\nInit == y = 0\n====\n", &Options::default());
//! assert!(!bad.accepted());
//! assert_eq!(bad.diagnostics()[0].code.as_str(), "DIVE-E0100"); // unknown operator `y`
//! assert_eq!(bad.diagnostics()[0].sany, Some(4200));
//! ```
#![forbid(unsafe_code)]

pub mod builtins;
pub mod diagnostics;
mod generate;
pub mod hashtable;
mod level;
pub mod loader;
pub mod model;
mod pcal_check;
pub mod stdlib;

pub use diagnostics::{Code, Diagnostic, FileId, Related, Severity, Span};
pub use loader::{Origin, SearchPath, SourceFile};
pub use model::{Level, Model, NodeId};

use model::{DefKind, Key, LevelData, NodeKind, OpDefData, OpLevels};
use std::path::Path;

/// Analysis options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Where modules are searched for.
    pub search: SearchPath,
    /// Run semantic analysis after loading (SANY without `-s`). When off,
    /// only module resolution and parsing are checked.
    pub semantic: bool,
    /// Level-check after semantic analysis (SANY without `-l`).
    pub level_check: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            search: SearchPath::default(),
            semantic: true,
            level_check: true,
        }
    }
}

/// The result of analysing a specification.
#[derive(Debug, Clone)]
pub struct Analysis {
    files: Vec<SourceFile>,
    diagnostics: Vec<Diagnostic>,
    syntax: Vec<(FileId, dive_syntax::Diagnostic)>,
    model: Model,
}

impl Analysis {
    /// Whether SANY would accept the specification: no syntax errors in any
    /// loaded module, and no semantic or level errors.
    #[must_use]
    pub fn accepted(&self) -> bool {
        !self.syntax.iter().any(|(_, d)| d.is_error())
            && !self.diagnostics.iter().any(Diagnostic::is_error)
    }

    /// The semantic diagnostics (module resolution, names, levels, ...).
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Syntax diagnostics of the loaded files that failed to parse.
    #[must_use]
    pub fn syntax_diagnostics(&self) -> &[(FileId, dive_syntax::Diagnostic)] {
        &self.syntax
    }

    /// The loaded files; [`FileId::index`] indexes this.
    #[must_use]
    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    /// One loaded file.
    #[must_use]
    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.index())
    }

    /// The semantic model. Complete only when [`Analysis::accepted`]; after
    /// errors it holds whatever was built before analysis stopped.
    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }
}

/// Reads and analyses the specification whose root module is in `path`.
///
/// Like SANY, a file that is not valid UTF-8 is decoded with U+FFFD
/// replacement characters.
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
    Ok(analyze(root, Some(name), options))
}

/// Analyses source text as the root module. `file_name` is used for SANY's
/// check that the module is named after its file; other modules are found
/// through `options` only (there is no root directory).
#[must_use]
pub fn analyze_source(file_name: &str, text: &str, options: &Options) -> Analysis {
    let base = Path::new(file_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_name);
    let root = SourceFile {
        display: file_name.to_owned(),
        origin: Origin::Text,
        text: text.to_owned(),
        parse: dive_syntax::parse_module(""),
    };
    analyze(root, Some(base), options)
}

fn analyze(root: SourceFile, file_name: Option<&str>, options: &Options) -> Analysis {
    let loaded = loader::load(root, file_name, &options.search);
    let mut syntax = Vec::new();
    for &f in &loaded.parse_failed {
        if let Some(file) = loaded.files.get(f.index()) {
            for d in file.parse.diagnostics() {
                syntax.push((f, d.clone()));
            }
        }
    }
    let mut diagnostics = loaded.diagnostics.clone();
    let mut model = Model::default();
    if loaded.ok() && options.semantic {
        semantic_analysis(&loaded, &mut model, &mut diagnostics, options.level_check);
    }
    Analysis {
        files: loaded.files,
        diagnostics,
        syntax,
        model,
    }
}

/// Creates the built-in operators (`Context.initialize`).
fn init_builtins(model: &mut Model) -> model::CtxId {
    let global = model.new_context();
    for b in builtins::BUILT_INS {
        let name = model.names.intern(b.name);
        let arity = b.arity;
        let nparams = usize::try_from(arity).unwrap_or(0);
        let mut params = Vec::new();
        for i in 0..nparams {
            let pn = model.names.intern(&format!("Formal_{i}"));
            params.push(model.add(
                None,
                NodeKind::FormalParam(model::FormalParamData {
                    name: Some(pn),
                    arity: 0,
                    module: None,
                }),
            ));
        }
        let mut lv = OpLevels::default();
        let mut level = LevelData {
            correct: true,
            ..LevelData::default()
        };
        if let Some((max, weights)) = b.levels {
            if arity == -1 {
                lv.max_levels = max.first().map(|&m| vec![m]).unwrap_or_default();
                lv.weights = weights.first().map(|&w| vec![w]).unwrap_or_default();
            } else {
                lv.max_levels = max.to_vec();
                lv.weights = weights.to_vec();
            }
            let la: Vec<bool> = weights.iter().map(|&w| w > 0).collect();
            lv.is_leibniz = la.iter().all(|&x| x);
            lv.is_leibniz_arg = Some(la);
            level.level = b.level;
            level.checked = 99;
        }
        let id = model.add(
            None,
            NodeKind::OpDef(Box::new(OpDefData {
                name,
                kind: DefKind::BuiltIn,
                params,
                arity,
                local: false,
                body: None,
                module: None,
                source: None,
                defined: true,
                step: None,
                labels: None,
                let_in_level: -1,
                in_recursive: false,
                in_recursive_section: false,
                recursive_section: -1,
                lv,
            })),
        );
        if let Some(n) = model.node_mut(id) {
            n.level = level;
        }
        if let Some(c) = model.ctx_mut(global) {
            c.put(Key::Sym(name), id);
        }
        model.builtins.push(id);
    }
    global
}

/// `SANY.frontEndSemanticAnalysis`.
fn semantic_analysis(
    loaded: &loader::Loaded,
    model: &mut Model,
    diagnostics: &mut Vec<Diagnostic>,
    level_check: bool,
) {
    let global = init_builtins(model);
    let marker_name = model.names.intern("$$InAssume");
    let in_assume = model.add(
        None,
        NodeKind::OpDecl(model::OpDeclData {
            name: marker_name,
            decl: model::DeclKind::Marker,
            arity: 0,
            module: None,
        }),
    );
    let null = model.add(None, NodeKind::Error);
    let mut shared = generate::Shared {
        ext: std::collections::HashMap::new(),
        global,
        in_assume,
        null,
        errors: diagnostics.iter().filter(|d| d.is_error()).count(),
    };
    for &u in &loaded.order {
        let unit = &loaded.units[u];
        let name = model.names.intern(&unit.name);
        if shared.ext.contains_key(&name) {
            continue;
        }
        let Some(file) = loaded.files.get(unit.file.index()) else {
            continue;
        };
        let Some(module) = file.parse.module() else {
            continue;
        };
        let mut g = generate::Gen::new(model, diagnostics, &mut shared, unit.file);
        let result = g.generate(dive_syntax::ast::AstNode::syntax(&module));
        let ctx = g.external_context();
        let Ok(id) = result else {
            // SANY aborts the whole analysis.
            return;
        };
        if let Some(NodeKind::Module(md)) = model.node_mut(id).map(|n| &mut n.kind) {
            md.is_standard = file.origin == Origin::Standard;
        }
        shared.ext.insert(name, (id, ctx));
        model.modules.push(id);
        if level_check && shared.errors == 0 {
            let mut errors = 0usize;
            let itr = model.node(id).level.checked + 1;
            let mut lv = level::Lv {
                m: model,
                diags: diagnostics,
                errors: &mut errors,
            };
            lv.check_module(id, itr);
            shared.errors += errors;
        }
    }
    model.root = model.modules.last().copied();
}
