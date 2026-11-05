//! Module resolution: SANY's parse phase for a whole specification
//! (`tla2sany/modanalyzer/SpecObj.java`, `ParseUnit.java`).
//!
//! Starting from the root file, every module named by `EXTENDS` or
//! `INSTANCE` that is not an inner module in scope is loaded as a file and
//! parsed, until nothing is unresolved. The result is the list of parse
//! units in SANY's semantic-analysis order: every module after the modules
//! it extends or instantiates.
//!
//! **Search order**, as `util/SimpleFilenameToStream.java` and
//! `util/FileUtil.createNamedInputStream`:
//! 1. the root file's directory (SANY: the working directory);
//! 2. the library paths, in order (SANY's `TLA-Library` property);
//! 3. the standard modules, embedded in this crate;
//! 4. the "after standard" module paths (SANY: modules found at the root
//!    of the Java classpath, such as the `CommunityModules` jar);
//! 5. a module of that name inside the root file itself, extracted line by
//!    line (`util/MonolithSpecExtractor.module`);
//! 6. the bundled CommunityModules and TLAPS sources.

use crate::diagnostics::{Code, Diagnostic, FileId, Span};
use dive_syntax::ast::AstNode as _;
use dive_syntax::{Parse, SyntaxKind, SyntaxNode, TextRange, TextSize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Where module files are searched for. See the module docs for the order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchPath {
    /// Directories searched after the root file's directory and before the
    /// standard modules (SANY's `-DTLA-Library`).
    pub library_paths: Vec<PathBuf>,
    /// Directories searched after the standard modules (modules TLC finds
    /// on its Java classpath, such as the `CommunityModules` jar).
    pub after_standard: Vec<PathBuf>,
}

/// Where a loaded file came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// A file on disk.
    File(PathBuf),
    /// An embedded standard module.
    Standard,
    /// A bundled CommunityModules or TLAPS source.
    Bundled,
    /// A module extracted from the text after the root module.
    Monolith(PathBuf),
    /// Source text given directly to the analyzer.
    Text,
}

/// One source file of the analysis.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The name shown in diagnostics.
    pub display: String,
    /// Where it came from.
    pub origin: Origin,
    /// The text, decoded as UTF-8 with replacement as SANY does.
    pub text: String,
    /// The parse.
    pub parse: Parse,
}

/// One loaded file (SANY's `ParseUnit`).
#[derive(Debug, Clone)]
pub(crate) struct Unit {
    /// The module name the unit was loaded for (SANY's file-derived name).
    pub(crate) name: String,
    pub(crate) file: FileId,
    pub(crate) root: PtrId,
    extendees: Vec<usize>,
    instancees: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PtrId(usize);

/// A module (top-level or inner) inside a unit (SANY's `ModulePointer`
/// with its `ModuleRelatives`).
#[derive(Debug, Clone)]
struct ModPtr {
    unit: usize,
    node: SyntaxNode,
    name: String,
    parent: Option<PtrId>,
    inner: Vec<PtrId>,
    extends: Vec<String>,
    instances: Vec<String>,
    context: HashMap<String, PtrId>,
}

/// The outcome of loading.
#[derive(Debug, Default)]
pub(crate) struct Loaded {
    pub(crate) files: Vec<SourceFile>,
    pub(crate) units: Vec<Unit>,
    /// Unit indices in semantic-analysis order (root last).
    pub(crate) order: Vec<usize>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Files whose parse failed; their syntax diagnostics are errors.
    pub(crate) parse_failed: Vec<FileId>,
}

impl Loaded {
    pub(crate) fn ok(&self) -> bool {
        self.parse_failed.is_empty() && !self.diagnostics.iter().any(Diagnostic::is_error)
    }
}

/// Decodes bytes as SANY's UTF-8 reader does: invalid sequences become
/// U+FFFD.
#[must_use]
pub fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct Loader<'a> {
    search: &'a SearchPath,
    root_dir: Option<PathBuf>,
    root_text: Option<(PathBuf, String)>,
    out: Loaded,
    ptrs: Vec<ModPtr>,
    by_name: HashMap<String, usize>,
    // SANY's stateful search results (`nextParseUnitName`,
    // `nextExtenderOrInstancerModule`, `extentionFound`, `instantiationFound`).
    next_name: String,
    next_from: Option<PtrId>,
    extension_found: bool,
    instantiation_found: bool,
}

/// Loads the specification rooted at `root` (already read).
pub(crate) fn load(
    root: SourceFile,
    root_file_name: Option<&str>,
    search: &SearchPath,
    extra_root_extendees: &[String],
) -> Loaded {
    let root_dir = match &root.origin {
        Origin::File(p) => p.parent().map(Path::to_path_buf),
        _ => None,
    };
    let root_text = match &root.origin {
        Origin::File(p) => Some((p.clone(), root.text.clone())),
        _ => None,
    };
    let mut l = Loader {
        search,
        root_dir,
        root_text,
        out: Loaded::default(),
        ptrs: Vec::new(),
        by_name: HashMap::new(),
        next_name: String::new(),
        next_from: None,
        extension_found: false,
        instantiation_found: false,
    };
    l.run(root, root_file_name, extra_root_extendees);
    l.out
}

#[allow(clippy::unnecessary_wraps, reason = "diagnostic spans are optional")]
fn span(file: FileId, range: TextRange) -> Option<Span> {
    Some(Span { file, range })
}

/// The module name a file name stands for (everything before the last `.`).
fn stem(file_name: &str) -> &str {
    file_name.rfind('.').map_or(file_name, |i| &file_name[..i])
}

impl Loader<'_> {
    fn add_file(&mut self, f: SourceFile) -> FileId {
        let id = FileId(u32::try_from(self.out.files.len()).unwrap_or(u32::MAX));
        self.out.files.push(f);
        id
    }

    fn error(&mut self, d: Diagnostic) {
        self.out.diagnostics.push(d);
    }

    fn run(&mut self, root: SourceFile, root_file_name: Option<&str>, extra: &[String]) {
        let display = root.display.clone();
        let file = self.add_file(root);
        let Some(root_unit) = self.parse_unit(file, root_file_name.map(stem), true, &display, None)
        else {
            return;
        };
        let root_ptr = self.out.units[root_unit].root;
        // `ParameterizedSpecObj.findOrCreateParsedUnit`: extra extendees of the
        // root module, for loading only (see `Options::extra_root_extendees`).
        for e in extra {
            if !self.ptrs[root_ptr.0].extends.contains(e) {
                self.ptrs[root_ptr.0].extends.push(e.clone());
            }
        }
        self.validate_pcal(root_unit);
        loop {
            let more = self.find_next_unresolved_extension(root_ptr)
                || self.find_next_unresolved_instantiation(root_ptr);
            if !more {
                break;
            }
            let name = self.next_name.clone();
            let Some(from) = self.next_from else { break };
            let unit = if let Some(&u) = self.by_name.get(&name) {
                u
            } else {
                match self.load_unit(&name, from) {
                    Some(u) => u,
                    None => return,
                }
            };
            let from_unit = self.ptrs[from.0].unit;
            if self.extension_found {
                self.validate_pcal(unit);
                self.out.units[from_unit].extendees.push(unit);
            }
            if self.instantiation_found {
                self.out.units[from_unit].instancees.push(unit);
            }
            if !self.non_circularity_test(unit) {
                return;
            }
            let unit_root = self.out.units[unit].root;
            if self.extension_found {
                self.resolve_between_spec_and_extension(from, unit, &name, unit_root);
            }
            if self.instantiation_found {
                self.ptrs[from.0]
                    .context
                    .entry(name.clone())
                    .or_insert(unit_root);
            }
        }
        let mut order = Vec::new();
        self.calculate_dependencies(root_unit, &mut order, &mut HashSet::new());
        self.out.order = order;
    }

    /// `SpecObj.validateParseUnit`: the rejecting part of SANY's PlusCal
    /// translation check (see `pcal_check`).
    fn validate_pcal(&mut self, unit: usize) {
        let file = self.out.units[unit].file;
        let node = self.ptrs[self.out.units[unit].root.0].node.clone();
        let text = &self.out.files[file.index()].text;
        let range = node.text_range();
        let line_of = |at: TextSize| {
            let at = usize::from(at).min(text.len());
            text[..at].matches('\n').count() + 1
        };
        // The module's location runs from its first to its last token.
        let first = node
            .children_with_tokens()
            .find(|e| !e.kind().is_trivia())
            .map_or(range.start(), |e| e.text_range().start());
        let begin = line_of(first);
        let end = line_of(range.end());
        if let Some(why) = crate::pcal_check::validation_failure(text, begin, end) {
            let name = self.out.units[unit].name.clone();
            self.error(Diagnostic::new(
                Code::PlusCalCheck,
                span(file, range),
                format!("module `{name}`: {why}"),
            ));
        }
    }

    /// SANY's `findOrCreateParsedUnit` for a dependency.
    fn load_unit(&mut self, name: &str, from: PtrId) -> Option<usize> {
        let from_name = self.ptrs[from.0].name.clone();
        let from_file = self.out.units[self.ptrs[from.0].unit].file;
        let Some(src) = self.locate(name) else {
            let at = self.ptrs[from.0].node.clone();
            let range = at
                .children()
                .find(|n| n.kind() == SyntaxKind::MODULE_HEADER)
                .map_or_else(|| at.text_range(), |h| h.text_range());
            self.error(
                Diagnostic::new(
                    Code::ModuleNotFound,
                    span(from_file, range),
                    format!(
                        "cannot find module `{name}`, which module `{from_name}` extends or instantiates"
                    ),
                )
                .with_help(format!(
                    "put `{name}.tla` next to the root module or in a library path (`--library-path`)"
                )),
            );
            return None;
        };
        let display = src.display.clone();
        let file = self.add_file(src);
        self.parse_unit(file, Some(name), false, &display, Some(from))
    }

    /// Finds the file for module `name`.
    fn locate(&self, name: &str) -> Option<SourceFile> {
        let file_name = format!("{name}.tla");
        let read = |dir: &Path| -> Option<SourceFile> {
            let p = dir.join(&file_name);
            let bytes = std::fs::read(&p).ok()?;
            Some(SourceFile {
                display: p.display().to_string(),
                origin: Origin::File(p),
                text: decode(&bytes),
                parse: dive_syntax::parse_module(""),
            })
        };
        if let Some(d) = &self.root_dir
            && let Some(f) = read(d)
        {
            return Some(f);
        }
        for d in &self.search.library_paths {
            if let Some(f) = read(d) {
                return Some(f);
            }
        }
        if let Some(text) = crate::stdlib::standard_module(name) {
            return Some(SourceFile {
                display: format!("<standard>/{file_name}"),
                origin: Origin::Standard,
                text: text.to_owned(),
                parse: dive_syntax::parse_module(""),
            });
        }
        for d in &self.search.after_standard {
            if let Some(f) = read(d) {
                return Some(f);
            }
        }
        if let Some((root_path, root_text)) = &self.root_text
            && let Some(text) = monolith_module(root_text, name)
        {
            return Some(SourceFile {
                display: format!("{}#{name}", root_path.display()),
                origin: Origin::Monolith(root_path.clone()),
                text,
                parse: dive_syntax::parse_module(""),
            });
        }
        let (library, text) = crate::bundled::module(name)?;
        Some(SourceFile {
            display: format!("<bundled:{library}>/{file_name}"),
            origin: Origin::Bundled,
            text: text.to_owned(),
            parse: dive_syntax::parse_module(""),
        })
    }

    /// Parses a loaded file and builds its module pointers (SANY's
    /// `ParseUnit.parseFile`). `expect` is the module name the file stands
    /// for; a different module name is an error.
    fn parse_unit(
        &mut self,
        file: FileId,
        expect: Option<&str>,
        is_root: bool,
        display: &str,
        from: Option<PtrId>,
    ) -> Option<usize> {
        let text = std::mem::take(&mut self.out.files[file.index()].text);
        let parse = dive_syntax::parse_module(&text);
        let ok = parse.is_ok();
        let module = parse.module();
        self.out.files[file.index()].text = text;
        self.out.files[file.index()].parse = parse;
        if !ok {
            self.out.parse_failed.push(file);
            if !is_root {
                let mut d = Diagnostic::new(
                    Code::DependencyParseError,
                    None,
                    format!("module file {display} has syntax errors"),
                );
                if let Some(p) = from {
                    let f = self.out.units[self.ptrs[p.0].unit].file;
                    d = d.with_related(
                        Span {
                            file: f,
                            range: self.ptrs[p.0].node.text_range(),
                        },
                        format!("imported by module `{}`", self.ptrs[p.0].name),
                    );
                }
                self.error(d);
            }
            return None;
        }
        let module = module?;
        let name = module.name().unwrap_or_default();
        let unit_name = expect.map_or_else(|| name.clone(), str::to_owned);
        let unit = self.out.units.len();
        let root_ptr = self.new_ptr(unit, module.syntax().clone(), None);
        self.out.units.push(Unit {
            name: unit_name.clone(),
            file,
            root: root_ptr,
            extendees: Vec::new(),
            instancees: Vec::new(),
        });
        self.by_name.insert(unit_name.clone(), unit);
        self.determine_relationships(root_ptr);
        if let Some(stem) = expect
            && stem != name
        {
            let range = module
                .syntax()
                .children()
                .find(|n| n.kind() == SyntaxKind::MODULE_HEADER)
                .map_or_else(|| module.syntax().text_range(), |h| h.text_range());
            let code = if is_root {
                Code::RootNameMismatch
            } else {
                Code::DependencyNameMismatch
            };
            self.error(
                Diagnostic::new(
                    code,
                    span(file, range),
                    format!("file name `{stem}` does not match the name `{name}` of the module it contains"),
                )
                .with_help(format!("rename the file to `{name}.tla` or the module to `{stem}`")),
            );
            return None;
        }
        Some(unit)
    }

    fn new_ptr(&mut self, unit: usize, node: SyntaxNode, parent: Option<PtrId>) -> PtrId {
        let name = dive_syntax::ast::Module::cast(node.clone())
            .and_then(|m| m.name())
            .unwrap_or_default();
        let id = PtrId(self.ptrs.len());
        self.ptrs.push(ModPtr {
            unit,
            node,
            name,
            parent,
            inner: Vec::new(),
            extends: Vec::new(),
            instances: Vec::new(),
            context: HashMap::new(),
        });
        id
    }

    /// `ParseUnit.determineModuleRelationships`.
    fn determine_relationships(&mut self, cur: PtrId) {
        let node = self.ptrs[cur.0].node.clone();
        if let Some(m) = dive_syntax::ast::Module::cast(node.clone()) {
            self.ptrs[cur.0].extends = m.extends();
        }
        self.calculate_context_within_unit(cur);
        for child in node.children() {
            if child.kind() == SyntaxKind::MODULE {
                let unit = self.ptrs[cur.0].unit;
                let inner = self.new_ptr(unit, child, Some(cur));
                self.ptrs[cur.0].inner.push(inner);
                self.determine_relationships(inner);
            } else if child.kind() != SyntaxKind::MODULE_HEADER
                && child.kind() != SyntaxKind::EXTENDS
            {
                let mut names = Vec::new();
                collect_instances(&child, &mut names);
                self.ptrs[cur.0].instances.extend(names);
            }
        }
    }

    /// `ParseUnit.calculateContextWithinParseUnit`.
    fn calculate_context_within_unit(&mut self, cur: PtrId) {
        let mut ctx = std::mem::take(&mut self.ptrs[cur.0].context);
        if let Some(p) = self.ptrs[cur.0].parent {
            for (k, v) in &self.ptrs[p.0].context {
                ctx.entry(k.clone()).or_insert(*v);
            }
        }
        let mut ancestor = cur;
        while let Some(parent) = self.ptrs[ancestor.0].parent {
            for (k, v) in &self.ptrs[parent.0].context {
                ctx.entry(k.clone()).or_insert(*v);
            }
            let peers = self.ptrs[parent.0].inner.clone();
            for peer in peers.iter().take(peers.len().saturating_sub(1)) {
                let n = self.ptrs[peer.0].name.clone();
                ctx.entry(n).or_insert(*peer);
            }
            ancestor = parent;
        }
        self.ptrs[cur.0].context = ctx;
        self.handle_extensions(cur, cur, &mut HashSet::new());
    }

    /// `ParseUnit.handleExtensions`: inner modules of extended modules are
    /// in scope.
    fn handle_extensions(&mut self, cur: PtrId, other: PtrId, seen: &mut HashSet<PtrId>) {
        if !seen.insert(other) {
            return;
        }
        let names = self.ptrs[other.0].extends.clone();
        for n in names {
            let Some(&ext) = self.ptrs[cur.0].context.get(&n) else {
                continue;
            };
            let inner = self.ptrs[ext.0].inner.clone();
            for m in inner {
                let mn = self.ptrs[m.0].name.clone();
                self.ptrs[cur.0].context.entry(mn).or_insert(m);
                self.handle_extensions(cur, ext, seen);
            }
        }
    }

    fn find_next_unresolved_extension(&mut self, root: PtrId) -> bool {
        let mut seen = HashSet::new();
        self.next_ext_body(root, &mut seen)
    }

    fn next_ext_body(&mut self, cur: PtrId, seen: &mut HashSet<PtrId>) -> bool {
        if !seen.insert(cur) {
            self.extension_found = false;
            return false;
        }
        let exts = self.ptrs[cur.0].extends.clone();
        for e in &exts {
            if !self.ptrs[cur.0].context.contains_key(e) {
                self.next_name.clone_from(e);
                self.next_from = Some(cur);
                self.extension_found = true;
                return true;
            }
        }
        for e in &exts {
            if let Some(&p) = self.ptrs[cur.0].context.get(e)
                && self.next_ext_body(p, seen)
            {
                self.extension_found = true;
                return true;
            }
        }
        let insts = self.ptrs[cur.0].instances.clone();
        for i in &insts {
            if let Some(&p) = self.ptrs[cur.0].context.get(i)
                && self.next_ext_body(p, seen)
            {
                self.extension_found = true;
                return true;
            }
        }
        let inner = self.ptrs[cur.0].inner.clone();
        for m in inner {
            if self.next_ext_body(m, seen) {
                self.extension_found = true;
                return true;
            }
        }
        self.extension_found = false;
        false
    }

    fn find_next_unresolved_instantiation(&mut self, root: PtrId) -> bool {
        let mut seen = HashSet::new();
        self.next_inst_body(root, &mut seen)
    }

    /// `SpecObj.instanceResolvesToInternalModule`.
    fn instance_is_internal(&self, cur: PtrId, name: &str) -> bool {
        let mut seen = HashSet::new();
        for child in self.ptrs[cur.0].node.children() {
            match child.kind() {
                SyntaxKind::MODULE => {
                    if let Some(n) = dive_syntax::ast::Module::cast(child).and_then(|m| m.name()) {
                        seen.insert(n);
                    }
                }
                SyntaxKind::INSTANCE | SyntaxKind::MODULE_DEF => {
                    let inst = child
                        .children()
                        .find(|c| c.kind() == SyntaxKind::INSTANTIATION)
                        .and_then(dive_syntax::ast::Instantiation::cast)
                        .and_then(|i| i.module_name());
                    if inst.as_deref() == Some(name) {
                        return seen.contains(name);
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn next_inst_body(&mut self, cur: PtrId, seen: &mut HashSet<PtrId>) -> bool {
        if !seen.insert(cur) {
            self.instantiation_found = false;
            return false;
        }
        let insts = self.ptrs[cur.0].instances.clone();
        for i in &insts {
            if !self.ptrs[cur.0].context.contains_key(i) && !self.instance_is_internal(cur, i) {
                self.next_name.clone_from(i);
                self.next_from = Some(cur);
                self.instantiation_found = true;
                return true;
            }
        }
        let exts = self.ptrs[cur.0].extends.clone();
        for e in &exts {
            if let Some(&p) = self.ptrs[cur.0].context.get(e)
                && self.next_inst_body(p, seen)
            {
                self.instantiation_found = true;
                return true;
            }
        }
        for i in &insts {
            if let Some(&p) = self.ptrs[cur.0].context.get(i)
                && self.next_inst_body(p, seen)
            {
                self.instantiation_found = true;
                return true;
            }
        }
        let inner = self.ptrs[cur.0].inner.clone();
        for m in inner {
            if self.next_inst_body(m, seen) {
                self.instantiation_found = true;
                return true;
            }
        }
        self.instantiation_found = false;
        false
    }

    /// `SpecObj.nonCircularityTest`.
    fn non_circularity_test(&mut self, unit: usize) -> bool {
        let mut seen = HashSet::new();
        let mut path = vec![unit];
        if let Some(cycle) = self.circularity_body(unit, unit, &mut seen, &mut path) {
            let names: Vec<String> = cycle
                .iter()
                .map(|&u| self.out.units[u].name.clone())
                .chain(std::iter::once(self.out.units[unit].name.clone()))
                .collect();
            let file = self.out.units[unit].file;
            let range = self.ptrs[self.out.units[unit].root.0].node.text_range();
            self.error(
                Diagnostic::new(
                    Code::CircularDependency,
                    span(file, range),
                    format!(
                        "modules depend on each other in a cycle: {}",
                        names.join(" -> ")
                    ),
                )
                .with_help("a module may not extend or instantiate itself, directly or indirectly"),
            );
            return false;
        }
        true
    }

    fn circularity_body(
        &self,
        unit: usize,
        candidate: usize,
        seen: &mut HashSet<usize>,
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        if !seen.insert(candidate) {
            return None;
        }
        let mut refs = self.out.units[candidate].extendees.clone();
        for i in &self.out.units[candidate].instancees {
            if !refs.contains(i) {
                refs.push(*i);
            }
        }
        for r in refs {
            if r == unit {
                return Some(path.clone());
            }
            path.push(r);
            if let Some(c) = self.circularity_body(unit, r, seen, path) {
                return Some(c);
            }
            path.pop();
        }
        None
    }

    /// `SpecObj.resolveNamesBetweenSpecAndExtention`.
    fn resolve_between_spec_and_extension(
        &mut self,
        extender: PtrId,
        unit: usize,
        name: &str,
        unit_root: PtrId,
    ) {
        self.ptrs[extender.0]
            .context
            .entry(name.to_owned())
            .or_insert(unit_root);
        let extenders = self.modules_indirectly_extending(extender);
        for m in extenders {
            self.resolve_between_module_and_extension(m, unit);
        }
    }

    fn directly_extends(&self, a: PtrId, b: PtrId) -> bool {
        self.ptrs[a.0]
            .extends
            .iter()
            .any(|e| self.ptrs[a.0].context.get(e) == Some(&b))
    }

    /// `SpecObj.getModulesIndirectlyExtending`.
    fn modules_indirectly_extending(&self, module: PtrId) -> Vec<PtrId> {
        let mut out = vec![module];
        let mut i = 0;
        while i < out.len() {
            let target = out[i];
            for p in 0..self.ptrs.len() {
                let pid = PtrId(p);
                if self.directly_extends(pid, target) && !out.contains(&pid) {
                    out.push(pid);
                }
            }
            i += 1;
        }
        out
    }

    /// `SpecObj.resolveNamesBetweenModuleAndExtention`.
    fn resolve_between_module_and_extension(&mut self, m: PtrId, unit: usize) {
        let root = self.out.units[unit].root;
        let inner = self.ptrs[root.0].inner.clone();
        let names: Vec<String> = self.ptrs[m.0]
            .extends
            .iter()
            .chain(self.ptrs[m.0].instances.iter())
            .cloned()
            .collect();
        for n in names {
            if let Some(&i) = inner.iter().find(|i| self.ptrs[i.0].name == n) {
                self.ptrs[m.0].context.entry(n).or_insert(i);
            }
        }
        let m_inner = self.ptrs[m.0].inner.clone();
        for i in m_inner {
            self.resolve_between_module_and_extension(i, unit);
        }
    }

    /// `SpecObj.calculateDependencies`.
    fn calculate_dependencies(
        &self,
        unit: usize,
        order: &mut Vec<usize>,
        seen: &mut HashSet<usize>,
    ) {
        if !seen.insert(unit) {
            if !order.contains(&unit) {
                // On a cycle SANY would loop; cycles were rejected above.
            }
            return;
        }
        for &e in &self.out.units[unit].extendees {
            self.calculate_dependencies(e, order, seen);
        }
        for &i in &self.out.units[unit].instancees {
            self.calculate_dependencies(i, order, seen);
        }
        if !order.contains(&unit) {
            order.push(unit);
        }
    }
}

/// Every module named by an `INSTANCE` below `node`, outside nested modules
/// (SANY's `ParseUnit.getInstances`).
fn collect_instances(node: &SyntaxNode, out: &mut Vec<String>) {
    if node.kind() == SyntaxKind::MODULE {
        return;
    }
    if node.kind() == SyntaxKind::INSTANTIATION
        && let Some(n) =
            dive_syntax::ast::Instantiation::cast(node.clone()).and_then(|i| i.module_name())
    {
        out.push(n);
    }
    for c in node.children() {
        collect_instances(&c, out);
    }
}

/// SANY's `MonolithSpecExtractor.module`: the lines from one matching
/// `-----*\s*MODULE\s+NAME\s*----*` through the next line starting `====`.
fn monolith_module(text: &str, name: &str) -> Option<String> {
    let mut out = String::new();
    let mut active = false;
    for line in text.lines() {
        if active && line.starts_with("====") {
            out.push_str(line);
            out.push('\n');
            break;
        }
        if !active && is_module_header_line(line, name) {
            active = true;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if active {
            out.push_str(line);
            out.push('\n');
        }
    }
    active.then_some(out)
}

/// Whether `line` matches `-----*\s*MODULE\s+NAME\s*----*` (the whole line).
fn is_module_header_line(line: &str, name: &str) -> bool {
    let rest = line.trim_start_matches('-');
    if line.len() - rest.len() < 4 {
        return false;
    }
    let rest = rest.trim_start_matches(is_java_space);
    let Some(rest) = rest.strip_prefix("MODULE") else {
        return false;
    };
    let trimmed = rest.trim_start_matches(is_java_space);
    if trimmed.len() == rest.len() {
        return false;
    }
    let Some(rest) = trimmed.strip_prefix(name) else {
        return false;
    };
    let rest = rest.trim_start_matches(is_java_space);
    let dashes = rest.len() - rest.trim_start_matches('-').len();
    dashes >= 3 && rest.trim_start_matches('-').is_empty()
}

/// Java's regex `\s`: `[ \t\n\x0B\f\r]`.
fn is_java_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
}

/// The span of the whole text of a file, for diagnostics without a better
/// location.
#[allow(dead_code, reason = "kept for diagnostics that need a whole-file span")]
pub(crate) fn whole(file: FileId, text: &str) -> Span {
    Span {
        file,
        range: TextRange::new(
            TextSize::from(0),
            TextSize::try_from(text.len()).unwrap_or_default(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monolith_header_lines() {
        assert!(is_module_header_line("---- MODULE A ----", "A"));
        assert!(is_module_header_line("-----MODULE A---", "A"));
        assert!(!is_module_header_line("--- MODULE A ----", "A"));
        assert!(!is_module_header_line("---- MODULE AB ----", "A"));
        assert!(!is_module_header_line("---- MODULE A --", "A"));
        assert!(!is_module_header_line("---- MODULEA ----", "A"));
    }

    #[test]
    fn monolith_extracts_through_the_end_line() {
        let t = "---- MODULE R ----\n====\n---- MODULE A ----\nx == 1\n==== tail\nrest\n";
        assert_eq!(
            monolith_module(t, "A").as_deref(),
            Some("---- MODULE A ----\nx == 1\n==== tail\n")
        );
        assert_eq!(monolith_module(t, "B"), None);
    }
}
