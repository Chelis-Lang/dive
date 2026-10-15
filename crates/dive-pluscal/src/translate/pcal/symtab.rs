//! Name disambiguation (`pcal.PcalSymTab` and `pcal.PcalFixIDs`).
//!
//! Every global, label, procedure, process, process variable, procedure
//! variable and parameter is entered with its context. Globals keep their
//! names; any later symbol whose name is already in use gets a suffix: `_`,
//! then characters of its context name, then its type number. Labels,
//! variable references and call targets are then rewritten to the new names.

use super::Fail;
use super::ast::{Algorithm, Body, Clause, Kind, LabeledStmt, Procedure, Process, Stmt, VarDecl};
use super::expr::{Expr, Kind as TokKind};
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Type {
    Global = 0,
    Label = 1,
    Procedure = 2,
    Process = 3,
    ProcessVar = 4,
    ProcedureVar = 5,
    Parameter = 6,
}

const TYPE_NAMES: [&str; 7] = [
    "Global variable",
    "Label",
    "Procedure",
    "Process",
    "Process variable",
    "Procedure variable",
    "Parameter",
];

#[derive(Clone, Debug)]
struct Entry {
    ty: Type,
    id: String,
    context: String,
    c_type: &'static str,
    line: usize,
    col: usize,
    use_this: String,
}

/// A procedure's renamed name and entry label.
#[derive(Clone, Debug)]
pub(crate) struct ProcedureEntry {
    pub name: String,
    pub initial_pc: String,
}

/// A process's renamed name and entry label.
#[derive(Clone, Debug)]
pub(crate) struct ProcessEntry {
    pub name: String,
    pub initial_pc: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SymTab {
    entries: Vec<Entry>,
    pub procs: Vec<ProcedureEntry>,
    pub processes: Vec<ProcessEntry>,
    /// Comment lines reporting renamed symbols.
    pub report: Vec<String>,
    /// Initial label of a uniprocess algorithm.
    pub initial_pc: Option<String>,
    errors: String,
}

impl SymTab {
    /// Builds the table (`new PcalSymTab(ast)`); duplicate definitions fail.
    pub(crate) fn new(alg: &Algorithm) -> Result<Self, Fail> {
        let mut st = Self::default();
        st.insert(Type::Global, "pc", "", "", 0, 0);
        if !alg.prcds.is_empty() {
            st.insert(Type::Global, "stack", "", "", 0, 0);
        }
        for decl in &alg.decls {
            st.var_decl(decl, "");
        }
        for procedure in &alg.prcds {
            st.procedure(procedure);
        }
        match &alg.body {
            Body::Uni { body, .. } => {
                st.initial_pc = body.first().map(|l| l.label.clone());
                for labeled in body {
                    st.labeled(labeled, "", "");
                }
            }
            Body::Multi(processes) => {
                for process in processes {
                    st.process(process);
                }
            }
        }
        if st.errors.is_empty() {
            Ok(st)
        } else {
            Err(Fail::new(st.errors.trim_start_matches('\n').to_owned()))
        }
    }

    fn find(&self, ty: Type, id: &str, context: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.id == id && e.context == context && e.ty == ty)
    }

    fn find_any(&self, id: &str, context: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.id == id && e.context == context)
    }

    fn insert(
        &mut self,
        ty: Type,
        id: &str,
        context: &str,
        c_type: &'static str,
        line: usize,
        col: usize,
    ) -> bool {
        if matches!(ty, Type::ProcedureVar | Type::ProcessVar | Type::Parameter) {
            if self.find(Type::Global, id, "").is_some() || self.find_any(id, context).is_some() {
                return false;
            }
        } else if self.find(ty, id, context).is_some() {
            return false;
        }
        self.entries.push(Entry {
            ty,
            id: id.to_owned(),
            context: context.to_owned(),
            c_type,
            line,
            col,
            use_this: id.to_owned(),
        });
        true
    }

    fn var_decl(&mut self, decl: &VarDecl, context: &str) {
        let ty = if context.is_empty() {
            Type::Global
        } else {
            Type::ProcessVar
        };
        if !self.insert(
            ty,
            &decl.var,
            context,
            "process",
            decl.loc.line,
            decl.loc.col,
        ) {
            let _ = write!(
                self.errors,
                "\n{} {} redefined at line {}, column {}",
                TYPE_NAMES[ty as usize], decl.var, decl.loc.line, decl.loc.col
            );
        }
    }

    fn procedure(&mut self, p: &Procedure) {
        if self.procs.iter().any(|e| e.name == p.name) {
            let _ = write!(
                self.errors,
                "\nProcedure {} redefined at line {}, column {}",
                p.name, p.loc.line, p.loc.col
            );
        } else {
            self.procs.push(ProcedureEntry {
                name: p.name.clone(),
                initial_pc: p.body.first().map(|l| l.label.clone()).unwrap_or_default(),
            });
        }
        self.insert(
            Type::Procedure,
            &p.name,
            "",
            "procedure",
            p.loc.line,
            p.loc.col,
        );
        for decl in &p.decls {
            if !self.insert(
                Type::ProcedureVar,
                &decl.var,
                &p.name,
                "procedure",
                decl.loc.line,
                decl.loc.col,
            ) {
                let _ = write!(
                    self.errors,
                    "\nProcedure variable {} redefined at line {}, column {}",
                    decl.var, decl.loc.line, decl.loc.col
                );
            }
        }
        for decl in &p.params {
            if !self.insert(
                Type::Parameter,
                &decl.var,
                &p.name,
                "procedure",
                decl.loc.line,
                decl.loc.col,
            ) {
                let _ = write!(
                    self.errors,
                    "\nParameter {} redefined at line {}, column {}",
                    decl.var, decl.loc.line, decl.loc.col
                );
            }
        }
        for labeled in &p.body {
            self.labeled(labeled, &p.name, "procedure");
        }
    }

    fn process(&mut self, p: &Process) {
        if self.processes.iter().any(|e| e.name == p.name) {
            let _ = write!(
                self.errors,
                "\nProcess {} redefined at line {}, column {}",
                p.name, p.loc.line, p.loc.col
            );
        } else {
            self.processes.push(ProcessEntry {
                name: p.name.clone(),
                initial_pc: p.body.first().map(|l| l.label.clone()).unwrap_or_default(),
            });
        }
        self.insert(Type::Process, &p.name, "", "process", p.loc.line, p.loc.col);
        for decl in &p.decls {
            self.var_decl(decl, &p.name);
        }
        for labeled in &p.body {
            self.labeled(labeled, &p.name, "process");
        }
    }

    fn labeled(&mut self, labeled: &LabeledStmt, context: &str, c_type: &'static str) {
        if !self.insert(
            Type::Label,
            &labeled.label,
            context,
            c_type,
            labeled.loc.line,
            labeled.loc.col,
        ) {
            let _ = write!(
                self.errors,
                "\nLabel {} redefined at line {}, column {}",
                labeled.label, labeled.loc.line, labeled.loc.col
            );
        }
        for stmt in &labeled.stmts {
            self.stmt(stmt, context, c_type);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, context: &str, c_type: &'static str) {
        match &stmt.kind {
            Kind::While { unlab, lab, .. } => {
                for s in unlab {
                    self.stmt(s, context, c_type);
                }
                for l in lab {
                    self.labeled(l, context, c_type);
                }
            }
            Kind::If { then, els, .. } => {
                for s in then.iter().chain(els) {
                    self.stmt(s, context, c_type);
                }
            }
            Kind::With { body, .. } => {
                for s in body {
                    self.stmt(s, context, c_type);
                }
            }
            Kind::LabelIf {
                unlab_then,
                lab_then,
                unlab_else,
                lab_else,
                ..
            } => {
                for s in unlab_then {
                    self.stmt(s, context, c_type);
                }
                for l in lab_then {
                    self.labeled(l, context, c_type);
                }
                for s in unlab_else {
                    self.stmt(s, context, c_type);
                }
                for l in lab_else {
                    self.labeled(l, context, c_type);
                }
            }
            Kind::Either(ors) => {
                for s in ors.iter().flatten() {
                    self.stmt(s, context, c_type);
                }
            }
            Kind::LabelEither(clauses) => {
                for Clause { unlab, lab } in clauses {
                    for s in unlab {
                        self.stmt(s, context, c_type);
                    }
                    for l in lab {
                        self.labeled(l, context, c_type);
                    }
                }
            }
            _ => {}
        }
    }

    fn ambiguous(&self, name: &str) -> bool {
        self.entries.iter().filter(|e| e.use_this == name).count() > 1
    }

    /// `Disambiguate`.
    pub(crate) fn disambiguate(&mut self) {
        for vtype in 0..=7 {
            for i in 0..self.entries.len() {
                if self.entries[i].ty as usize != vtype {
                    continue;
                }
                self.entries[i].use_this = self.entries[i].id.clone();
                let mut suffix = 0usize;
                while vtype > 0 && self.ambiguous(&self.entries[i].use_this) {
                    suffix += 1;
                    let entry = &mut self.entries[i];
                    let context_len = entry.context.chars().count();
                    if suffix == 1 {
                        entry.use_this.push('_');
                    } else if suffix > context_len + 1 {
                        entry.use_this.push_str(&vtype.to_string());
                    } else if let Some(c) = entry.context.chars().nth(suffix - 2) {
                        entry.use_this.push(c);
                    }
                }
                let entry = &self.entries[i];
                if entry.id != entry.use_this {
                    let context = if entry.context.is_empty() {
                        String::new()
                    } else {
                        format!(" of {} {}", entry.c_type, entry.context)
                    };
                    self.report.push(format!(
                        "\\* {} {}{} at line {} col {} changed to {}",
                        TYPE_NAMES[entry.ty as usize],
                        entry.id,
                        context,
                        entry.line,
                        entry.col,
                        entry.use_this
                    ));
                }
            }
        }
    }

    /// The disambiguated name of a symbol.
    pub(crate) fn use_this(&self, ty: Type, id: &str, context: &str) -> String {
        self.find(ty, id, context)
            .map_or_else(|| id.to_owned(), |i| self.entries[i].use_this.clone())
    }

    /// `UseThisVar`: the renamed variable visible as `id` in `context`.
    pub(crate) fn use_var(&self, id: &str, context: &str) -> String {
        let Some(i) = self.find_any(id, context) else {
            return id.to_owned();
        };
        let e = &self.entries[i];
        if matches!(
            e.ty,
            Type::Global | Type::ProcessVar | Type::ProcedureVar | Type::Parameter
        ) {
            e.use_this.clone()
        } else {
            id.to_owned()
        }
    }

    pub(crate) fn find_proc(&self, name: &str) -> Option<&ProcedureEntry> {
        self.procs.iter().find(|p| p.name == name)
    }

    pub(crate) fn has_label(&self, id: &str, context: &str) -> bool {
        self.find(Type::Label, id, context).is_some()
    }

    /// `CheckForDefaultInitValue`.
    pub(crate) fn check_default_init_value(&self) -> Result<(), Fail> {
        let mut errors = String::new();
        for e in &self.entries {
            if e.id == "defaultInitValue" {
                if errors.is_empty() {
                    errors.push_str("Cannot use `defaultInitValue' as ");
                } else {
                    errors.push_str(" or ");
                }
                errors.push_str(TYPE_NAMES[e.ty as usize]);
                errors.push_str(" name");
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Fail::new(errors))
        }
    }
}

/// `PcalFixIDs.Fix`: rewrites names to their disambiguated forms.
pub(crate) fn fix(alg: &mut Algorithm, st: &mut SymTab) -> Result<(), Fail> {
    for decl in &mut alg.decls {
        fix_decl(decl, st, "");
    }
    for procedure in &mut alg.prcds {
        fix_procedure(procedure, st)?;
    }
    match &mut alg.body {
        Body::Uni { body, .. } => {
            for labeled in body {
                fix_labeled(labeled, st, "")?;
            }
        }
        Body::Multi(processes) => {
            // Transitive closure of the procedures each procedure calls.
            let names: Vec<String> = alg.prcds.iter().map(|p| p.name.clone()).collect();
            let n = names.len();
            let mut path = vec![vec![false; n]; n];
            for (i, p) in alg.prcds.iter().enumerate() {
                for (j, name) in names.iter().enumerate() {
                    path[i][j] = p.procedures_called.contains(name);
                }
            }
            for k in 0..n {
                for i in 0..n {
                    for j in 0..n {
                        path[i][j] = path[i][j] || (path[i][k] && path[k][j]);
                    }
                }
            }
            for (i, p) in alg.prcds.iter_mut().enumerate() {
                p.procedures_called = (0..n)
                    .filter(|&j| path[i][j])
                    .map(|j| names[j].clone())
                    .collect();
            }
            for process in processes {
                fix_process(process, st)?;
                let direct = process.procedures_called.clone();
                for name in direct {
                    let Some(index) = names.iter().position(|n| *n == name) else {
                        return Err(Fail::new(format!("Could not find procedure name `{name}'")));
                    };
                    for k in 0..n {
                        if path[index][k] && !process.procedures_called.contains(&names[k]) {
                            process.procedures_called.push(names[k].clone());
                        }
                    }
                }
            }
        }
    }
    if let Some(ipc) = st.initial_pc.take() {
        st.initial_pc = Some(st.use_this(Type::Label, &ipc, ""));
    }
    Ok(())
}

fn fix_procedure(p: &mut Procedure, st: &mut SymTab) -> Result<(), Fail> {
    let context = p.name.clone();
    for decl in p.decls.iter_mut().chain(p.params.iter_mut()) {
        fix_decl(decl, st, &context);
    }
    for labeled in &mut p.body {
        fix_labeled(labeled, st, &context)?;
    }
    for label in p.plus_labels.iter_mut().chain(p.minus_labels.iter_mut()) {
        *label = st.use_this(Type::Label, label, &context);
    }
    let new_name = st.use_this(Type::Procedure, &p.name, "");
    if let Some(entry) = st.procs.iter_mut().find(|e| e.name == p.name) {
        entry.initial_pc = st
            .entries
            .iter()
            .find(|e| e.ty == Type::Label && e.id == entry.initial_pc && e.context == context)
            .map_or_else(|| entry.initial_pc.clone(), |e| e.use_this.clone());
        entry.name.clone_from(&new_name);
    }
    p.name = new_name;
    Ok(())
}

fn fix_process(p: &mut Process, st: &mut SymTab) -> Result<(), Fail> {
    let context = p.name.clone();
    for decl in &mut p.decls {
        fix_decl(decl, st, &context);
    }
    for labeled in &mut p.body {
        fix_labeled(labeled, st, &context)?;
    }
    for label in p.plus_labels.iter_mut().chain(p.minus_labels.iter_mut()) {
        *label = st.use_this(Type::Label, label, &context);
    }
    let new_name = st.use_this(Type::Process, &p.name, "");
    if let Some(entry) = st.processes.iter_mut().find(|e| e.name == p.name) {
        entry.initial_pc = st
            .entries
            .iter()
            .find(|e| e.ty == Type::Label && e.id == entry.initial_pc && e.context == context)
            .map_or_else(|| entry.initial_pc.clone(), |e| e.use_this.clone());
        entry.name.clone_from(&new_name);
    }
    p.name = new_name;
    Ok(())
}

fn fix_decl(decl: &mut VarDecl, st: &SymTab, context: &str) {
    decl.var = st.use_var(&decl.var, context);
    decl.val = fix_expr(&decl.val, st, context);
}

fn fix_labeled(labeled: &mut LabeledStmt, st: &SymTab, context: &str) -> Result<(), Fail> {
    labeled.label = st.use_this(Type::Label, &labeled.label, context);
    for stmt in &mut labeled.stmts {
        fix_stmt(stmt, st, context)?;
    }
    Ok(())
}

fn fix_seq(stmts: &mut [Stmt], st: &SymTab, context: &str) -> Result<(), Fail> {
    for stmt in stmts {
        fix_stmt(stmt, st, context)?;
    }
    Ok(())
}

fn fix_stmt(stmt: &mut Stmt, st: &SymTab, context: &str) -> Result<(), Fail> {
    match &mut stmt.kind {
        Kind::While { test, unlab, lab } => {
            fix_seq(unlab, st, context)?;
            for l in lab {
                fix_labeled(l, st, context)?;
            }
            *test = fix_expr(test, st, context);
        }
        Kind::Assign(ass) => {
            for a in ass {
                a.var = st.use_var(&a.var, context);
                a.sub = fix_expr(&a.sub, st, context);
                a.rhs = fix_expr(&a.rhs, st, context);
            }
        }
        Kind::If { test, then, els } => {
            fix_seq(then, st, context)?;
            fix_seq(els, st, context)?;
            *test = fix_expr(test, st, context);
        }
        Kind::With { exp, body, .. } => {
            *exp = fix_expr(exp, st, context);
            fix_seq(body, st, context)?;
        }
        Kind::When(e) | Kind::Print(e) | Kind::Assert(e) => *e = fix_expr(e, st, context),
        Kind::Skip | Kind::MacroCall { .. } => {}
        Kind::LabelIf {
            test,
            unlab_then,
            lab_then,
            unlab_else,
            lab_else,
        } => {
            *test = fix_expr(test, st, context);
            fix_seq(unlab_then, st, context)?;
            for l in lab_then {
                fix_labeled(l, st, context)?;
            }
            fix_seq(unlab_else, st, context)?;
            for l in lab_else {
                fix_labeled(l, st, context)?;
            }
        }
        Kind::Call { to, args } => {
            *to = st.use_this(Type::Procedure, to, context);
            for a in args {
                *a = fix_expr(a, st, context);
            }
        }
        Kind::Return { from } => {
            if let Some(from) = from {
                *from = st.use_this(Type::Procedure, from, context);
            }
        }
        Kind::CallReturn { to, from, args } => {
            if let Some(from) = from {
                *from = st.use_this(Type::Procedure, from, context);
            }
            *to = st.use_this(Type::Procedure, to, context);
            for a in args {
                *a = fix_expr(a, st, context);
            }
        }
        Kind::CallGoto { to, after, args } => {
            // pcal.trans looks the goto target up as a procedure.
            *after = st.use_this(Type::Procedure, after, context);
            *to = st.use_this(Type::Procedure, to, context);
            for a in args {
                *a = fix_expr(a, st, context);
            }
        }
        Kind::Goto(to) => {
            if !st.has_label(to, context) && to != "Done" {
                return Err(Fail::at(
                    format!(
                        "goto to non-existent label `{to}' at line {}, column {}",
                        stmt.loc.line, stmt.loc.col
                    ),
                    stmt.at,
                ));
            }
            *to = st.use_this(Type::Label, to, context);
        }
        Kind::Either(ors) => {
            for o in ors {
                fix_seq(o, st, context)?;
            }
        }
        Kind::LabelEither(clauses) => {
            for c in clauses {
                fix_seq(&mut c.unlab, st, context)?;
                for l in &mut c.lab {
                    fix_labeled(l, st, context)?;
                }
            }
        }
    }
    Ok(())
}

/// `FixExpr`: renames identifier tokens.
fn fix_expr(expr: &Expr, st: &SymTab, context: &str) -> Expr {
    let mut subs: Vec<(String, Expr)> = Vec::new();
    for tok in expr.tokens() {
        if tok.kind != TokKind::Ident {
            continue;
        }
        let new = st.use_var(&tok.text, context);
        if new == tok.text || subs.iter().any(|(id, _)| *id == tok.text) {
            continue;
        }
        subs.push((tok.text.clone(), Expr::line(&[(&new, TokKind::Ident)])));
    }
    expr.substitute(&subs, false)
}
