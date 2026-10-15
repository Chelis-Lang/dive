//! TLA+ generation (`pcal.PcalTLAGen`).
//!
//! Each labelled step becomes an action. Within a step, later statements read
//! the primed value of variables assigned earlier; variables of procedures and
//! process sets, and the translator's `pc` and `stack`, are indexed by
//! `self`; branches of `if` and `either` add `UNCHANGED` for what the other
//! branches assign; the step ends with `UNCHANGED` for every variable it does
//! not assign. Layout follows the reference closely because TLA+ conjunction
//! and disjunction lists are layout sensitive.

use super::Fail;
use super::ast::{
    Algorithm, Body, Fairness, Kind, LabeledStmt, Procedure, Process, SingleAssign, Stmt,
};
use super::expr::{Expr, Kind as TokKind};
use super::symtab::{SymTab, Type};
use std::fmt::Write as _;

/// Generation settings (the relevant `PcalParams`).
#[derive(Clone, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent generation options and translator facts"
)]
pub(crate) struct GenOptions {
    /// `""`, `"nof"`, `"wf"`, `"sf"` or `"wfNext"`.
    pub fairness: String,
    pub fair_algorithm: bool,
    pub no_done_disjunct: bool,
    pub wrap_column: usize,
    pub omit_pc: bool,
    pub omit_stuttering_when_done: bool,
    pub has_default_initialization: bool,
    /// Bound checked before appending any generated output fragment.
    pub max_output_bytes: usize,
    /// Original algorithm location for output-budget diagnostics.
    pub source_at: usize,
}

fn spaces(n: usize) -> String {
    " ".repeat(n)
}

fn width(s: &str) -> usize {
    s.chars().count()
}

/// Per-variable assignment counts (`pcal.Changed`).
#[derive(Clone, Debug)]
struct Changed {
    count: Vec<u32>,
}

impl Changed {
    fn new(n: usize) -> Self {
        Self { count: vec![0; n] }
    }
    fn merge(&mut self, other: &Changed) {
        for (a, b) in self.count.iter_mut().zip(&other.count) {
            *a = (*a).max(*b);
        }
    }
}

/// `NeedsParentheses`, including the reference's habit of only scanning
/// the first line of a multi-line expression.
fn needs_parentheses(lines: &[String]) -> bool {
    let Some(line) = lines.first() else {
        return false;
    };
    let chars: Vec<char> = line.chars().collect();
    let mut depth: i64 = 0;
    let mut in_string = false;
    let mut need = false;
    for _ in 0..lines.len() {
        let mut i = 0;
        while i < chars.len() && !need {
            let c = chars[i];
            if in_string {
                match c {
                    '"' => in_string = false,
                    '\\' => i += 1,
                    _ => {}
                }
            } else {
                let next = chars.get(i + 1).copied().unwrap_or(' ');
                let mut left = false;
                let mut right = false;
                let mut may = false;
                match c {
                    '"' => in_string = true,
                    '=' | '#' => may = true,
                    '<' => {
                        if next == '<' {
                            i += 1;
                            left = true;
                        } else {
                            may = true;
                        }
                    }
                    '>' => {
                        if next == '>' {
                            i += 1;
                            right = true;
                        } else {
                            may = true;
                        }
                    }
                    '|' => {
                        if next == '-' || (i > 0 && chars[i - 1] == '-') {
                            may = true;
                        }
                    }
                    '\\' => {
                        if !matches!(next, ' ' | 'o' | 'X') {
                            may = true;
                        }
                    }
                    '/' => {
                        if next == '\\' {
                            may = true;
                        }
                    }
                    '(' | '[' | '{' => left = true,
                    ')' | ']' | '}' => right = true,
                    _ => {}
                }
                if may && depth == 0 {
                    need = true;
                }
                if left {
                    depth += 1;
                }
                if right {
                    if depth == 0 {
                        need = true;
                    }
                    depth -= 1;
                }
            }
            i += 1;
        }
        if in_string {
            need = true;
        }
        if need {
            break;
        }
    }
    need
}

pub(crate) struct Gen<'a> {
    opts: &'a GenOptions,
    st: &'a SymTab,
    lines: Vec<String>,
    next: String,
    vars: Vec<String>,
    /// Procedure parameters and locals.
    pc_vars: Vec<String>,
    /// Variables local to process sets.
    ps_vars: Vec<String>,
    mp: bool,
    self_expr: Option<Expr>,
    self_is_self: bool,
    next_step: Vec<String>,
    current_proc_name: String,
    output_bytes: usize,
}

impl<'a> Gen<'a> {
    pub(crate) fn new(opts: &'a GenOptions, st: &'a SymTab) -> Self {
        Self {
            opts,
            st,
            lines: Vec::new(),
            next: String::new(),
            vars: Vec::new(),
            pc_vars: Vec::new(),
            ps_vars: Vec::new(),
            mp: false,
            self_expr: None,
            self_is_self: false,
            next_step: Vec::new(),
            current_proc_name: "Next".into(),
            output_bytes: 0,
        }
    }

    // ----- output primitives ------------------------------------------------

    fn check_output(&self, bytes: usize) -> Result<(), Fail> {
        let total = self.output_bytes.saturating_add(bytes);
        if total > self.opts.max_output_bytes {
            return Err(Fail::at(
                "PlusCal generated-output byte limit reached".into(),
                self.opts.source_at,
            )
            .code(crate::Code::Resource)
            .generation());
        }
        Ok(())
    }

    fn reserve_output(&mut self, bytes: usize) -> Result<(), Fail> {
        self.check_output(bytes)?;
        self.output_bytes += bytes;
        Ok(())
    }

    fn token(&mut self, s: &str) -> Result<(), Fail> {
        let separator =
            usize::from(!s.is_empty() && self.next.is_empty() && !self.lines.is_empty());
        self.reserve_output(s.len().saturating_add(separator))?;
        self.next.push_str(s);
        Ok(())
    }

    fn end(&mut self) {
        if !self.next.is_empty() {
            self.lines.push(std::mem::take(&mut self.next));
        }
    }

    fn line(&mut self, s: &str) -> Result<(), Fail> {
        self.end();
        self.reserve_output(s.len().saturating_add(usize::from(!self.lines.is_empty())))?;
        self.lines.push(s.to_owned());
        Ok(())
    }

    fn expr(&mut self, e: &Expr) -> Result<(), Fail> {
        self.check_output(e.rendered_bytes())?;
        let sv = e.to_lines();
        let indent = width(&self.next);
        let mut index = 0;
        if indent != 0 {
            self.token(&sv[0])?;
            index = 1;
            if sv.len() > 1 {
                self.end();
            }
        }
        if sv.len() > 1 {
            let pad = spaces(indent);
            while index < sv.len() - 1 {
                self.reserve_output(
                    pad.len()
                        .saturating_add(sv[index].len())
                        .saturating_add(usize::from(!self.lines.is_empty())),
                )?;
                self.lines.push(format!("{pad}{}", sv[index]));
                index += 1;
            }
            let bytes = pad.len().saturating_add(sv[index].len());
            self.reserve_output(
                bytes.saturating_add(usize::from(bytes != 0 && !self.lines.is_empty())),
            )?;
            self.next = format!("{pad}{}", sv[index]);
        } else if indent == 0 {
            self.token(&sv[0])?;
        }
        Ok(())
    }

    fn insert_line(&mut self, index: usize, text: &str) -> Result<(), Fail> {
        let separator = usize::from(!self.lines.is_empty() || !self.next.is_empty());
        self.reserve_output(text.len().saturating_add(separator))?;
        self.lines.insert(index, text.to_owned());
        Ok(())
    }

    fn remove_line(&mut self, index: usize) -> String {
        let separator = usize::from(self.lines.len() > 1 || !self.next.is_empty());
        let text = self.lines.remove(index);
        self.output_bytes = self
            .output_bytes
            .saturating_sub(text.len().saturating_add(separator));
        text
    }

    fn replace_line(&mut self, index: usize, text: String) -> Result<(), Fail> {
        let old = self.lines[index].len();
        if text.len() > old {
            self.reserve_output(text.len() - old)?;
        } else {
            self.output_bytes -= old - text.len();
        }
        self.lines[index] = text;
        Ok(())
    }

    // ----- subscripts and primes --------------------------------------------

    fn is_procedure_var(&self, v: &str) -> bool {
        self.pc_vars.iter().any(|x| x == v)
    }

    fn is_process_set_var(&self, v: &str) -> bool {
        self.ps_vars.iter().any(|x| x == v)
    }

    /// `Self(context)`.
    fn self_for(&self, context: &str) -> Option<Expr> {
        if !self.mp {
            return None;
        }
        if context == "procedure" {
            Some(Expr::line(&[("self", TokKind::Ident)]))
        } else {
            self.self_expr.clone()
        }
    }

    /// `SubExpr`: `[e]`.
    fn sub_expr(sub: Option<Expr>) -> Option<Expr> {
        let sub = sub?;
        let mut out = Expr::line(&[("[", TokKind::Builtin)]);
        let mut body = sub;
        body.prepend(&out, 0);
        out = body;
        out.push("]", TokKind::Builtin, 0);
        Some(out)
    }

    /// `AddSubscriptsToExpr`: primes changed variables and subscripts the
    /// per-process ones.
    fn subscripts(&self, e: &Expr, sub: Option<&Expr>, c: &Changed) -> Expr {
        let mut subs: Vec<(String, Expr)> = Vec::new();
        for tok in e.tokens() {
            let prime = tok.kind == TokKind::Ident && self.is_changed(c, &tok.text);
            let needs_self = sub.is_some()
                && (tok.kind == TokKind::Added
                    || (self.mp
                        && tok.kind == TokKind::Ident
                        && (self.is_procedure_var(&tok.text)
                            || self.is_process_set_var(&tok.text))));
            if (prime || needs_self) && !subs.iter().any(|(id, _)| *id == tok.text) {
                let mut exp = Expr::line(&[(&tok.text, tok.kind)]);
                if prime {
                    exp.push("'", TokKind::Builtin, 0);
                }
                if needs_self && let Some(sub) = sub {
                    let mut s = sub.clone();
                    s.prepend(&exp, 0);
                    exp = s;
                }
                subs.push((tok.text.clone(), exp));
            }
        }
        e.substitute(&subs, false)
    }

    fn is_changed(&self, c: &Changed, v: &str) -> bool {
        self.vars
            .iter()
            .position(|x| x == v)
            .and_then(|i| c.count.get(i))
            .is_some_and(|n| *n > 0)
    }

    fn set(&self, c: &mut Changed, v: &str) -> u32 {
        match self.vars.iter().position(|x| x == v) {
            Some(i) => match c.count.get_mut(i) {
                Some(n) => {
                    *n += 1;
                    *n
                }
                None => 0,
            },
            None => 0,
        }
    }

    fn unchanged_names(&self, c: &Changed, other: Option<&Changed>) -> Vec<String> {
        self.vars
            .iter()
            .enumerate()
            .filter(|(i, _)| c.count[*i] == 0 && other.is_none_or(|o| o.count[*i] > 0))
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// `Changed.Unchanged(ch)`: comma-separated names in chunks of at most
    /// `ch` characters.
    fn chunks(names: &[String], ch: usize) -> Vec<String> {
        let mut out = Vec::new();
        let mut s = String::new();
        let mut have_one = false;
        for one in names {
            if have_one {
                s.push_str(", ");
            } else {
                have_one = true;
            }
            if width(&s) + width(one) > ch {
                if !s.is_empty() {
                    out.push(s.clone());
                }
                s.clone_from(one);
            } else {
                s.push_str(one);
            }
        }
        if !s.is_empty() {
            out.push(s);
        }
        out
    }

    // ----- top level ----------------------------------------------------------

    /// `PcalTLAGen.generate`.
    pub(crate) fn generate(mut self, alg: &Algorithm) -> Result<Vec<String>, Fail> {
        for line in &self.st.report {
            self.line(line)?;
        }
        match &alg.body {
            Body::Uni { body, .. } => {
                self.mp = false;
                self.current_proc_name = "Next".into();
                self.vars_and_defs(alg, None)?;
                self.init(alg, None)?;
                for procedure in &alg.prcds {
                    self.procedure(procedure)?;
                }
                for labeled in body {
                    self.next_step.push(labeled.label.clone());
                    self.labeled(labeled, "")?;
                }
            }
            Body::Multi(processes) => {
                self.mp = true;
                self.vars_and_defs(alg, Some(processes))?;
                self.proc_set(processes)?;
                self.init(alg, Some(processes))?;
                for procedure in &alg.prcds {
                    self.procedure(procedure)?;
                }
                for process in processes {
                    self.process(process)?;
                }
            }
        }
        self.gen_next(alg)?;
        self.spec(alg)?;
        self.termination()?;
        self.end();
        Ok(self.lines)
    }

    fn vars_and_defs(
        &mut self,
        alg: &Algorithm,
        processes: Option<&Vec<Process>>,
    ) -> Result<(), Fail> {
        let mut globals = Vec::new();
        let mut locals = Vec::new();
        if !self.opts.omit_pc {
            globals.push("pc".to_owned());
        }
        for decl in &alg.decls {
            globals.push(decl.var.clone());
        }
        if !alg.prcds.is_empty() {
            globals.push("stack".to_owned());
        }
        for procedure in &alg.prcds {
            for decl in procedure.params.iter().chain(&procedure.decls) {
                locals.push(decl.var.clone());
                self.pc_vars.push(decl.var.clone());
            }
        }
        if let Some(processes) = processes {
            for process in processes {
                for decl in &process.decls {
                    locals.push(decl.var.clone());
                    if !process.is_eq {
                        self.ps_vars.push(decl.var.clone());
                    }
                }
            }
        }
        if self.opts.has_default_initialization {
            self.line("CONSTANT defaultInitValue")?;
        }
        let mut all = globals.clone();
        if alg.defs.is_empty() {
            all.extend(locals.iter().cloned());
            self.var_decl(&all)?;
        } else {
            self.var_decl(&globals)?;
            self.line("")?;
            self.line("(* define statement *)")?;
            self.expr(&alg.defs)?;
            self.line("")?;
            self.var_decl(&locals)?;
            all.extend(locals.iter().cloned());
        }
        self.line("")?;
        if all.is_empty() {
            return Err(Fail::new("The algorithm has no variables.".into()));
        }
        self.vars.clone_from(&all);
        self.token("vars == << ")?;
        let indent = width(&self.next);
        for (i, v) in all.iter().enumerate() {
            if i > 0 {
                self.token(", ")?;
            }
            if width(&self.next) + width(v) + 1 > self.opts.wrap_column {
                self.end();
                self.token(&spaces(indent))?;
            }
            self.token(v)?;
        }
        if width(&self.next) + 4 > self.opts.wrap_column {
            self.end();
            self.token(&spaces("vars ==".len()))?;
        }
        self.token(" >>")?;
        self.line("")?;
        Ok(())
    }

    fn var_decl(&mut self, vars: &[String]) -> Result<(), Fail> {
        if vars.is_empty() {
            return Ok(());
        }
        let head = if vars.len() > 1 {
            "VARIABLES "
        } else {
            "VARIABLE "
        };
        self.token(head)?;
        for (i, v) in vars.iter().enumerate() {
            if i > 0 {
                self.token(", ")?;
            }
            if width(&self.next) + width(v) + 1 > self.opts.wrap_column {
                self.end();
                self.token(&spaces(head.len()))?;
            }
            self.token(v)?;
        }
        self.end();
        Ok(())
    }

    fn proc_set(&mut self, processes: &[Process]) -> Result<(), Fail> {
        if processes.is_empty() {
            return Ok(());
        }
        self.token("ProcSet == ")?;
        for (i, process) in processes.iter().enumerate() {
            if i > 0 {
                self.token(" \\cup ")?;
            }
            self.token(if process.is_eq { "{" } else { "(" })?;
            self.expr(&process.id)?;
            self.token(if process.is_eq { "}" } else { ")" })?;
        }
        self.end();
        self.line("")?;
        Ok(())
    }

    fn init_decl(&mut self, var: &str, is_eq: bool, val: &Expr, prefix: &str) -> Result<(), Fail> {
        self.token(&format!("{prefix}/\\ "))?;
        self.token(&format!("{var}{}", if is_eq { " = " } else { " \\in " }))?;
        let needs = needs_parentheses(&val.to_lines());
        if needs {
            self.token("(")?;
        }
        self.expr(val)?;
        if needs {
            self.token(")")?;
        }
        self.end();
        Ok(())
    }

    #[allow(clippy::too_many_lines, reason = "mirrors GenInit section by section")]
    fn init(&mut self, alg: &Algorithm, processes: Option<&Vec<Process>>) -> Result<(), Fail> {
        let col = "Init == ".len();
        let mut is = "Init == ".to_owned();
        if !alg.decls.is_empty() {
            is.push_str("(* Global variables *)");
            self.line(&is)?;
            is = spaces(col);
            for decl in &alg.decls {
                self.init_decl(&decl.var, decl.is_eq, &decl.val, &is)?;
            }
        }
        let empty = Changed::new(0);
        for procedure in &alg.prcds {
            if procedure.params.is_empty() && procedure.decls.is_empty() {
                continue;
            }
            let _ = write!(is, "(* Procedure {} *)", procedure.name);
            self.line(&is)?;
            is = spaces(col);
            for decl in procedure.params.iter().chain(&procedure.decls) {
                if self.mp {
                    self.token(&format!("{is}/\\ "))?;
                    self.token(&format!("{} = [ self \\in ProcSet |-> ", decl.var))?;
                    let sub = Self::sub_expr(self.self_for("procedure"));
                    let val = self.subscripts(&decl.val, sub.as_ref(), &empty);
                    self.expr(&val)?;
                    self.token("]")?;
                    self.end();
                } else {
                    self.init_decl(&decl.var, decl.is_eq, &decl.val, &is)?;
                }
            }
        }
        if let Some(processes) = processes {
            for process in processes {
                if process.decls.is_empty() {
                    continue;
                }
                let _ = write!(is, "(* Process {} *)", process.name);
                self.line(&is)?;
                is = spaces(col);
                for decl in &process.decls {
                    self.token(&format!("{is}/\\ "))?;
                    if process.is_eq {
                        self.token(&format!(
                            "{}{}",
                            decl.var,
                            if decl.is_eq { " = " } else { " \\in " }
                        ))?;
                        self.expr(&decl.val)?;
                    } else if decl.is_eq {
                        self.token(&format!("{} = [self \\in ", decl.var))?;
                        self.expr(&process.id)?;
                        self.token(" |-> ")?;
                        let sub = Self::sub_expr(Some(Expr::line(&[("self", TokKind::Ident)])));
                        let val = self.subscripts(&decl.val, sub.as_ref(), &empty);
                        self.expr(&val)?;
                        self.token("]")?;
                    } else {
                        // w becomes w[CHOOSE self \in S : TRUE].
                        let mut choose = process.id.clone();
                        choose.prepend(
                            &Expr::line(&[
                                ("[", TokKind::Builtin),
                                ("CHOOSE", TokKind::Builtin),
                                ("self", TokKind::Ident),
                                ("\\in", TokKind::Builtin),
                            ]),
                            1,
                        );
                        let mut tail = Expr::line(&[
                            (":", TokKind::Builtin),
                            ("TRUE", TokKind::Builtin),
                            ("]", TokKind::Builtin),
                        ]);
                        tail.prepend(&choose, 1);
                        self.token(&format!("{} \\in [", decl.var))?;
                        self.expr(&process.id)?;
                        self.token(" -> ")?;
                        let val = self.subscripts(&decl.val, Some(&tail), &empty);
                        self.expr(&val)?;
                        self.token("]")?;
                    }
                    self.end();
                }
            }
        }
        if !alg.prcds.is_empty() {
            if self.mp {
                is.push_str("/\\ stack = [self \\in ProcSet |-> << >>]");
            } else {
                is.push_str("/\\ stack = << >>");
            }
            self.line(&is)?;
            is = spaces(col);
        }
        if !self.opts.omit_pc {
            if self.mp {
                let entries: Vec<String> = self
                    .st
                    .processes
                    .iter()
                    .map(|e| e.initial_pc.clone())
                    .collect();
                let ids: Vec<(bool, Expr)> = processes
                    .map(|list| list.iter().map(|p| (p.is_eq, p.id.clone())).collect())
                    .unwrap_or_default();
                let use_case = entries.len() != 1;
                if use_case {
                    is.push_str("/\\ pc = [self \\in ProcSet |-> CASE ");
                } else {
                    is.push_str("/\\ pc = [self \\in ProcSet |-> ");
                }
                let col_pc = width(&is) - 3;
                let count = entries.len();
                for (p, initial_pc) in entries.iter().enumerate() {
                    let (is_eq, id) = ids.get(p).cloned().unwrap_or((true, Expr::empty()));
                    if use_case {
                        is.push_str("self ");
                        is.push_str(if is_eq { "= " } else { "\\in " });
                        self.token(&is)?;
                        self.expr(&id)?;
                        is = format!(" -> \"{initial_pc}\"");
                        if p == count - 1 {
                            is.push(']');
                        }
                    } else {
                        let _ = write!(is, "\"{initial_pc}\"]");
                    }
                    self.token(&is)?;
                    self.end();
                    is = spaces(col_pc);
                    if p + 1 < count {
                        is.push_str("[] ");
                    }
                }
            } else {
                let ipc = self.st.initial_pc.as_deref().unwrap_or("null");
                let _ = write!(is, "/\\ pc = \"{ipc}\"");
                self.line(&is)?;
            }
        }
        self.line("")?;
        Ok(())
    }

    fn disjunction(
        &mut self,
        name: &str,
        argument: &str,
        body: &[LabeledStmt],
    ) -> Result<(), Fail> {
        let head = format!("{name}{argument} == ");
        self.token(&head)?;
        let indent = spaces(width(&head) + 2);
        for (i, labeled) in body.iter().enumerate() {
            let disjunct = format!("{}{argument}", labeled.label);
            if i != 0 && width(&self.next) + 7 + width(&disjunct) > self.opts.wrap_column {
                self.end();
            }
            if i != 0 {
                let pad = if self.next.is_empty() {
                    indent.clone()
                } else {
                    String::new()
                };
                self.token(&format!("{pad} \\/ "))?;
            }
            self.token(&disjunct)?;
        }
        self.line("")?;
        Ok(())
    }

    fn procedure(&mut self, procedure: &Procedure) -> Result<(), Fail> {
        if self.mp {
            self.self_expr = Some(Expr::line(&[("self", TokKind::Ident)]));
            self.self_is_self = true;
        } else {
            self.next_step.push(procedure.name.clone());
        }
        for labeled in &procedure.body {
            self.labeled(labeled, "procedure")?;
        }
        let argument = if self.mp { "(self)" } else { "" };
        self.disjunction(&procedure.name, argument, &procedure.body)?;
        Ok(())
    }

    fn process(&mut self, process: &Process) -> Result<(), Fail> {
        self.current_proc_name.clone_from(&process.name);
        if process.is_eq {
            self.self_expr = Some(process.id.clone());
            self.self_is_self = false;
        } else {
            self.self_expr = Some(Expr::line(&[("self", TokKind::Ident)]));
            self.self_is_self = true;
        }
        if process.is_eq {
            self.next_step.push(process.name.clone());
        }
        for labeled in &process.body {
            self.labeled(labeled, "process")?;
        }
        if !self.opts.omit_pc {
            let argument = if process.is_eq { "" } else { "(self)" };
            self.disjunction(&process.name, argument, &process.body)?;
        }
        Ok(())
    }

    // ----- steps and statements ------------------------------------------------

    fn labeled(&mut self, ast: &LabeledStmt, context: &str) -> Result<(), Fail> {
        let action = if self.opts.omit_pc {
            self.current_proc_name.clone()
        } else {
            ast.label.clone()
        };
        let mut sb = action;
        if self.mp && (context == "procedure" || self.self_is_self) {
            sb.push_str("(self)");
        }
        sb.push_str(" == ");
        let col = width(&sb);
        sb.push_str("/\\ ");
        self.end();
        let def_start = self.lines.len();
        let col_after_and = width(&sb);
        let mut c = Changed::new(self.vars.len());
        for stmt in &ast.stmts {
            let len = width(&sb);
            self.stmt(stmt, &mut c, context, &sb, len)?;
            sb = format!("{}/\\ ", spaces(col));
        }
        let names = self.unchanged_names(&c, None);
        if names.len() > 1 {
            let unc = Self::chunks(
                &names,
                self.opts
                    .wrap_column
                    .saturating_sub(col + "/\\ UNCHANGED << ".len()),
            );
            let mut sb = format!("{}/\\ UNCHANGED << ", spaces(col));
            let here = width(&sb);
            sb.push_str(&unc[0]);
            for chunk in &unc[1..] {
                self.line(&sb)?;
                sb = format!("{}{chunk}", spaces(here));
            }
            sb.push_str(" >>");
            self.token(&sb)?;
        } else if names.len() == 1 {
            let name = &names[0];
            if width(name) > 5 {
                self.token(&format!("{}/\\ UNCHANGED {name}", spaces(col)))?;
            } else {
                self.token(&format!("{}/\\ {name}' = {name}", spaces(col)))?;
            }
        } else if ast.stmts.len() == 1 {
            self.end();
            for i in def_start..self.lines.len() {
                let line = &self.lines[i];
                if i == def_start {
                    let chars: Vec<char> = line.chars().collect();
                    if chars.len() >= col_after_and {
                        let new: String = chars[..col_after_and - 3]
                            .iter()
                            .chain(&chars[col_after_and..])
                            .collect();
                        self.replace_line(i, new)?;
                    }
                } else if width(line) > 3 {
                    self.replace_line(i, line.chars().skip(3).collect())?;
                }
            }
        }
        self.line("")?;
        Ok(())
    }

    fn stmt(
        &mut self,
        stmt: &Stmt,
        c: &mut Changed,
        context: &str,
        prefix: &str,
        col: usize,
    ) -> Result<(), Fail> {
        match &stmt.kind {
            Kind::Assign(ass) => self.assign(stmt, ass, c, context, prefix),
            Kind::If { test, then, els } => self.gen_if(test, then, els, c, context, prefix),
            Kind::Either(ors) => self.either(ors, c, context, prefix),
            Kind::With {
                var,
                is_eq,
                exp,
                body,
            } => self.with(var, *is_eq, exp, body, c, context, prefix, col),
            Kind::When(e) => {
                self.token(prefix)?;
                let e = self.subscripts(e, Self::sub_expr(self.self_for(context)).as_ref(), c);
                self.expr(&e)?;
                self.end();
                Ok(())
            }
            Kind::Print(e) => {
                let e = self.subscripts(e, Self::sub_expr(self.self_for(context)).as_ref(), c);
                self.token(&format!("{prefix}PrintT("))?;
                self.expr(&e)?;
                self.token(")")?;
                self.end();
                Ok(())
            }
            Kind::Assert(e) => {
                let e = self.subscripts(e, Self::sub_expr(self.self_for(context)).as_ref(), c);
                let sb = format!("{prefix}Assert(");
                self.token(&sb)?;
                self.expr(&e)?;
                let here = width(&sb);
                let tail = format!("\"Failure of assertion at {}.\")", stmt.loc.describe());
                if width(&self.next) + 2 + width(&tail) < self.opts.wrap_column {
                    self.token(&format!(", {tail}"))?;
                } else {
                    self.token(", ")?;
                    self.end();
                    self.token(&format!("{}{tail}", spaces(here)))?;
                }
                self.end();
                Ok(())
            }
            Kind::Skip => {
                self.token(prefix)?;
                self.token("TRUE")?;
                self.end();
                Ok(())
            }
            _ => Err(Fail::internal("unexpected statement after explosion")),
        }
    }

    #[allow(clippy::too_many_lines, reason = "mirrors GenAssign")]
    fn assign(
        &mut self,
        stmt: &Stmt,
        ass: &[SingleAssign],
        c: &mut Changed,
        context: &str,
        prefix: &str,
    ) -> Result<(), Fail> {
        if ass.is_empty() {
            // pcal.trans emits an empty conjunct here (a recursive tail call
            // without parameters), which is not valid TLA+.
            return Err(Fail::at(
                format!(
                    "call followed by return of the same procedure without parameters translates to an empty conjunct\n    at {}",
                    stmt.loc.describe()
                ),
                stmt.at,
            )
            .code(crate::Code::Expression));
        }
        let mut c_this = c.clone();
        let mut sorted: Vec<&SingleAssign> = ass.iter().collect();
        sorted.sort_by(|a, b| a.var.cmp(&b.var));
        self.token(prefix)?;
        let self_sub = Self::sub_expr(self.self_for(context));
        let mut sb = String::new();
        let mut i = 0;
        let mut multiple_vars = false;
        while i < sorted.len() {
            let first = sorted[i];
            if !self.vars.contains(&first.var) {
                return Err(Fail::at(
                    format!(
                        "Assignment to undeclared variable {}\n    at {}",
                        first.var,
                        first.loc.describe()
                    ),
                    stmt.at,
                ));
            }
            let i_first = i;
            let mut i_last = i;
            let mut no_subscript = false;
            while i_last < sorted.len() && sorted[i_last].var == first.var {
                if sorted[i_last].sub.is_empty() {
                    no_subscript = true;
                }
                i_last += 1;
            }
            if i_last != sorted.len() {
                multiple_vars = true;
            }
            i_last -= 1;
            if self.set(&mut c_this, &first.var) > 1 || (i_last > i_first && no_subscript) {
                return Err(Fail::at(
                    format!(
                        "Multiple assignment to {}\n    at {}",
                        first.var,
                        stmt.loc.describe()
                    ),
                    stmt.at,
                )
                .code(crate::Code::Label));
            }
            if multiple_vars {
                sb.push_str("/\\ ");
            }
            let var = &first.var;
            if i_first == i_last {
                let sub = self.subscripts(&first.sub, self_sub.as_ref(), c);
                let rhs = self.subscripts(&first.rhs, self_sub.as_ref(), c);
                if self.mp
                    && (var == "pc"
                        || self.is_procedure_var(var)
                        || self.is_process_set_var(var)
                        || var == "stack")
                {
                    let _ = write!(sb, "{var}' = [{var} EXCEPT ![");
                    self.token(&sb)?;
                    let me = self.self_expr.clone().unwrap_or_else(Expr::empty);
                    self.expr(&me)?;
                    self.token("]")?;
                    if !sub.is_empty() {
                        self.expr(&sub)?;
                    }
                    self.token(" = ")?;
                    self.expr(&rhs)?;
                    self.token("]")?;
                } else if !first.sub.is_empty() {
                    let _ = write!(sb, "{var}' = [{var} EXCEPT !");
                    self.token(&sb)?;
                    self.expr(&sub)?;
                    self.token(" = ")?;
                    self.expr(&rhs)?;
                    self.token("]")?;
                } else {
                    let _ = write!(sb, "{var}' = ");
                    let needs = needs_parentheses(&rhs.to_lines());
                    if needs {
                        sb.push('(');
                    }
                    self.token(&sb)?;
                    self.expr(&rhs)?;
                    if needs {
                        self.token(")")?;
                    }
                }
                sb = String::new();
            } else {
                let _ = write!(sb, "{var}' = [{var} EXCEPT ");
                let mut cc = width(&sb);
                if i == 0 {
                    cc += width(prefix);
                }
                let subscript =
                    self.mp && (self.is_procedure_var(var) || self.is_process_set_var(var));
                for (k, one) in sorted.iter().enumerate().take(i_last + 1).skip(i_first) {
                    let sub = self.subscripts(&one.sub, self_sub.as_ref(), c);
                    let rhs = self.subscripts(&one.rhs, self_sub.as_ref(), c);
                    sb.push('!');
                    if subscript {
                        sb.push('[');
                        self.token(&sb)?;
                        let me = self.self_for(context).unwrap_or_else(Expr::empty);
                        self.expr(&me)?;
                        self.token("]")?;
                    } else {
                        self.token(&sb)?;
                    }
                    self.expr(&sub)?;
                    self.token(" = ")?;
                    self.expr(&rhs)?;
                    self.token(if k == i_last { "]" } else { "," })?;
                    sb = String::new();
                    if k < i_last {
                        self.end();
                        sb = spaces(cc);
                    }
                }
            }
            i = i_last + 1;
            if i < sorted.len() {
                self.end();
                sb = spaces(width(prefix));
            }
        }
        self.end();
        c.merge(&c_this);
        Ok(())
    }

    fn unchanged_text(names: &[String]) -> String {
        if names.len() == 1 {
            let name = &names[0];
            if width(name) > 5 {
                format!("UNCHANGED {name}")
            } else {
                format!("{name}' = {name}")
            }
        } else {
            format!("UNCHANGED << {} >>", names.join(", "))
        }
    }

    fn gen_if(
        &mut self,
        test: &Expr,
        then: &[Stmt],
        els: &[Stmt],
        c: &mut Changed,
        context: &str,
        prefix: &str,
    ) -> Result<(), Fail> {
        let mut c_then = c.clone();
        let mut c_else = c.clone();
        let test = self.subscripts(test, Self::sub_expr(self.self_for(context)).as_ref(), c);
        let mut sb = format!("{prefix}IF ");
        let mut here = width(&sb);
        self.token(&sb)?;
        self.expr(&test)?;
        self.end();
        sb = format!("{}THEN ", spaces(here));
        here = width(&sb);
        sb.push_str("/\\ ");
        for stmt in then {
            self.stmt(stmt, &mut c_then, context, &sb.clone(), here + 3)?;
            sb = format!("{}/\\ ", spaces(here));
        }
        let line_unc_then = {
            self.end();
            self.lines.len()
        };
        self.line(&sb)?;
        sb = format!("{}ELSE ", spaces(here - "THEN ".len()));
        here = width(&sb);
        if els.is_empty() {
            sb.push_str("/\\ TRUE");
            self.line(&sb)?;
            sb = format!("{}/\\ ", spaces(here));
        } else {
            sb.push_str("/\\ ");
            for stmt in els {
                self.stmt(stmt, &mut c_else, context, &sb.clone(), here + 3)?;
                sb = format!("{}/\\ ", spaces(here));
            }
        }
        let else_names = self.unchanged_names(&c_else, Some(&c_then));
        if !else_names.is_empty() {
            self.token(&format!("{sb}{}", Self::unchanged_text(&else_names)))?;
            self.end();
        }
        let then_line = self.remove_line(line_unc_then);
        let then_names = self.unchanged_names(&c_then, Some(&c_else));
        if !then_names.is_empty() {
            self.insert_line(
                line_unc_then,
                &format!("{then_line}{}", Self::unchanged_text(&then_names)),
            )?;
        }
        c.merge(&c_then);
        c.merge(&c_else);
        Ok(())
    }

    fn either(
        &mut self,
        ors: &[Vec<Stmt>],
        c: &mut Changed,
        context: &str,
        prefix: &str,
    ) -> Result<(), Fail> {
        let mut all = c.clone();
        let mut changes = Vec::new();
        let mut placeholders = Vec::new();
        let prefix_indent = width(prefix);
        let mut sb = format!("{prefix}\\/ ");
        let here = width(&sb);
        for (i, clause) in ors.iter().enumerate() {
            if i != 0 {
                sb = format!("{}\\/ ", spaces(prefix_indent));
            }
            sb.push_str("/\\ ");
            let mut c_or = c.clone();
            for stmt in clause {
                self.stmt(stmt, &mut c_or, context, &sb.clone(), here + 3)?;
                sb = format!("{}/\\ ", spaces(here));
            }
            all.merge(&c_or);
            changes.push(c_or);
            self.end();
            placeholders.push(self.lines.len());
            self.line("")?;
        }
        for i in (0..ors.len()).rev() {
            let at = placeholders[i];
            self.remove_line(at);
            let names = self.unchanged_names(&changes[i], Some(&all));
            if names.len() > 1 {
                self.insert_line(
                    at,
                    &format!("{}/\\ UNCHANGED <<{}>>", spaces(here), names.join(", ")),
                )?;
            } else if names.len() == 1 {
                self.insert_line(
                    at,
                    &format!("{}/\\ {}", spaces(here), Self::unchanged_text(&names)),
                )?;
            }
        }
        c.merge(&all);
        Ok(())
    }

    #[allow(clippy::too_many_arguments, reason = "mirrors GenWith's parameters")]
    fn with(
        &mut self,
        var: &str,
        is_eq: bool,
        exp: &Expr,
        body: &[Stmt],
        c: &mut Changed,
        context: &str,
        prefix: &str,
        col: usize,
    ) -> Result<(), Fail> {
        let exp = self.subscripts(exp, Self::sub_expr(self.self_for(context)).as_ref(), c);
        if is_eq {
            self.token(&format!("{prefix}LET {var} == "))?;
            self.expr(&exp)?;
            self.token(" IN")?;
        } else {
            self.token(&format!("{prefix}\\E {var} \\in "))?;
            self.expr(&exp)?;
            self.token(":")?;
        }
        self.end();
        let mut sb = spaces(col + 2);
        if body.len() > 1 {
            sb.push_str("/\\ ");
        }
        for stmt in body {
            let len = width(&sb);
            self.stmt(stmt, c, context, &sb.clone(), len)?;
            sb = format!("{}/\\ ", spaces(col + 2));
        }
        Ok(())
    }

    // ----- Next, Spec, Termination ----------------------------------------------

    #[allow(clippy::too_many_lines, reason = "mirrors GenNext")]
    fn gen_next(&mut self, alg: &Algorithm) -> Result<(), Fail> {
        if self.opts.omit_pc && !self.mp {
            return Ok(());
        }
        let stutter = !(self.opts.no_done_disjunct || self.opts.omit_stuttering_when_done);
        if stutter {
            self.line("(* Allow infinite stuttering to prevent deadlock on termination. *)")?;
            if self.mp {
                self.line("Terminating == /\\ \\A self \\in ProcSet: pc[self] = \"Done\"")?;
                self.line(&format!(
                    "{}/\\ UNCHANGED vars",
                    spaces("Terminating == ".len())
                ))?;
            } else {
                self.line("Terminating == pc = \"Done\" /\\ UNCHANGED vars")?;
            }
            self.line("")?;
        }
        let wrap = self.opts.wrap_column;
        let mut next_s = Vec::new();
        let max = wrap.saturating_sub("Next == \\/ ".len());
        let mut sb = String::new();
        for a in &self.next_step {
            if width(a) + 4 + width(&sb) > max {
                next_s.push(std::mem::take(&mut sb));
            }
            if !sb.is_empty() {
                sb.push_str(" \\/ ");
            }
            sb.push_str(a);
        }
        if !sb.is_empty() {
            next_s.push(sb);
        }
        let mut next_ss = Vec::new();
        let max2 = wrap.saturating_sub("Next == \\/ (\\E self \\in ProcSet: \\/ ".len());
        if self.mp && !self.st.procs.is_empty() {
            let mut sb = String::new();
            for p in &self.st.procs {
                if width(&p.name) + "(self) \\/ ".len() + width(&sb) > max2 {
                    next_ss.push(std::mem::take(&mut sb));
                }
                if !sb.is_empty() {
                    sb.push_str(" \\/ ");
                }
                sb.push_str(&p.name);
                sb.push_str("(self)");
            }
            if !sb.is_empty() {
                next_ss.push(format!("{sb})"));
            }
        }
        let mut next_ssp: Vec<Vec<String>> = Vec::new();
        if let Body::Multi(processes) = &alg.body {
            for p in processes {
                if p.is_eq {
                    continue;
                }
                let sv = p.id.to_lines();
                let mut v = Vec::new();
                let mut sb = "(\\E self \\in ".to_owned();
                let col = width(&sb);
                sb.push_str(&sv[0]);
                for line in &sv[1..] {
                    v.push(std::mem::take(&mut sb));
                    sb = format!("{}{line}", spaces(col));
                }
                let _ = write!(sb, ": {}(self))", p.name);
                v.push(sb);
                next_ssp.push(v);
            }
        }
        let mut sb = "Next == ".to_owned();
        let col = width(&sb) + 2;
        for s in &next_s {
            sb.push_str(s);
            self.line(&sb)?;
            sb = format!("{} \\/ ", spaces(col));
        }
        if !next_ss.is_empty() {
            sb.push_str("(\\E self \\in ProcSet: ");
            let col2 = width(&sb);
            if next_ss.len() > 1 {
                sb.push_str(" \\/ ");
            }
            for s in &next_ss {
                sb.push_str(s);
                self.line(&sb)?;
                sb = format!("{} \\/ ", spaces(col2));
            }
            sb = format!("{} \\/ ", spaces(col));
        }
        for (i, v) in next_ssp.iter().enumerate() {
            for line in v {
                sb.push_str(line);
                self.line(&sb)?;
                sb = if next_s.is_empty() && next_ss.is_empty() && i == 0 {
                    spaces(col - 2)
                } else {
                    spaces(col + 4)
                };
            }
            sb = format!("{} \\/ ", spaces(col));
        }
        if stutter {
            sb.push_str("Terminating");
            self.line(&sb)?;
        }
        self.line("")?;
        Ok(())
    }

    fn label_set(pc: &str, labels: &[String]) -> String {
        if labels.len() == 1 {
            format!("(pc[{pc}] # \"{}\") /\\ ", labels[0])
        } else {
            let list: Vec<String> = labels.iter().map(|l| format!("\"{l}\"")).collect();
            format!("(pc[{pc}] \\notin {{{}}}) /\\ ", list.join(", "))
        }
    }

    // ProcessFairness.format and FormulaPair from the pinned PcalTLAGen. Width
    // accounting deliberately follows the reference's omitted body separators.
    fn fairness_lines(&self, prefix: &[String], pairs: &[(String, Option<String>)]) -> Vec<String> {
        let pair_line = |(weak, strong): &(String, Option<String>)| match strong {
            Some(strong) => format!("{weak} /\\ {strong}"),
            None => weak.clone(),
        };
        let indent = "Spec == /\\ ".len();
        let prefix_width = prefix.last().map_or(0, |line| width(line));
        let mut single_width = if prefix.len() > 1 { prefix_width } else { 0 }
            + width(&pairs[0].0)
            + pairs[0].1.as_ref().map_or(0, |line| width(line));
        for pair in pairs.iter().skip(1) {
            single_width += width(&pair_line(pair));
        }
        single_width = single_width.max(
            prefix
                .iter()
                .take(prefix.len().saturating_sub(1))
                .map(|line| width(line))
                .max()
                .unwrap_or(0),
        );
        let mut text = prefix.join(&format!("\n{}", spaces(indent)));
        if indent + single_width <= self.opts.wrap_column
            || (pairs[0].1.is_none() && pairs.len() == 1)
        {
            text.push_str(
                &pairs
                    .iter()
                    .map(pair_line)
                    .collect::<Vec<_>>()
                    .join(" /\\ "),
            );
        } else {
            let col = indent + prefix_width;
            for (i, pair) in pairs.iter().enumerate() {
                if i != 0 {
                    if i == 1 {
                        text.push('\n');
                    }
                    text.push_str(&spaces(col));
                }
                let line = pair_line(pair);
                if col + width(&line) + 3 <= self.opts.wrap_column {
                    let _ = write!(text, "/\\ {line}");
                    if i != 0 {
                        text.push('\n');
                    }
                } else {
                    let _ = write!(text, "/\\ {}", pair.0);
                    if let Some(strong) = &pair.1 {
                        let _ = write!(text, "\n{}/\\ {strong}", spaces(col));
                    }
                }
            }
        }
        text.lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    line.to_owned()
                } else {
                    line.get(indent..).unwrap_or(line).to_owned()
                }
            })
            .collect()
    }

    #[allow(clippy::too_many_lines, reason = "mirrors GenSpec")]
    fn spec(&mut self, alg: &Algorithm) -> Result<(), Fail> {
        let safety = "Init /\\ [][Next]_vars";
        let fairness = self.opts.fairness.as_str();
        if fairness == "nof" || (!self.mp && fairness.is_empty()) {
            self.line(&format!("Spec == {safety}"))?;
            self.line("")?;
            return Ok(());
        }
        let wf_next = fairness == "wfNext"
            || self.opts.fair_algorithm
            || (!self.mp && (fairness == "wf" || fairness == "sf"));
        let mut formulas: Vec<Vec<String>> = Vec::new();
        let mut formula_bytes = 0usize;
        if let Body::Multi(processes) = &alg.body {
            for p in processes {
                if p.fairness == Fairness::Unfair {
                    continue;
                }
                let xf = if p.fairness == Fairness::Weak {
                    "WF"
                } else {
                    "SF"
                };
                let p_self = p.id.to_lines();
                let mut make_let = false;
                let mut q_self = "self".to_owned();
                if p.is_eq {
                    if p_self.len() > 1 {
                        make_let = true;
                    } else {
                        q_self.clone_from(&p_self[0]);
                    }
                }
                let mut prefix: Vec<String> = Vec::new();
                if make_let || !p.is_eq {
                    let (begin, end) = if p.is_eq {
                        ("LET self == ", "")
                    } else {
                        ("\\A self \\in ", " : ")
                    };
                    for (j, line) in p_self.iter().enumerate() {
                        let mut l = if j == 0 {
                            format!("{begin}{line}")
                        } else {
                            format!("{}{line}", spaces(begin.len()))
                        };
                        if j == p_self.len() - 1 {
                            l.push_str(end);
                        }
                        prefix.push(l);
                    }
                    if make_let {
                        prefix.push("IN ".to_owned());
                    }
                }
                let mut pairs = Vec::new();
                let mut strong = None;
                let mut wf = format!("{xf}_vars(");
                if !p.minus_labels.is_empty() {
                    wf.push_str(&Self::label_set(&q_self, &p.minus_labels));
                }
                let p_name = if p.is_eq {
                    p.name.clone()
                } else {
                    format!("{}(self)", p.name)
                };
                wf.push_str(&p_name);
                wf.push(')');
                if xf == "WF" && !p.plus_labels.is_empty() {
                    let sf: Vec<String> = p
                        .plus_labels
                        .iter()
                        .map(|l| {
                            if p.is_eq {
                                format!("SF_vars({l})")
                            } else {
                                format!("SF_vars({l}(self))")
                            }
                        })
                        .collect();
                    strong = Some(sf.join(" /\\ "));
                }
                pairs.push((wf, strong));
                for original in &p.procedures_called {
                    let name = self.st.use_this(Type::Procedure, original, "");
                    let Some(procedure) = alg.prcds.iter().find(|q| q.name == name) else {
                        continue;
                    };
                    let mut wf = format!("{xf}_vars(");
                    if !procedure.minus_labels.is_empty() {
                        wf.push_str(&Self::label_set(&q_self, &procedure.minus_labels));
                    }
                    let _ = write!(wf, "{}({q_self})", procedure.name);
                    wf.push(')');
                    let mut strong = None;
                    if xf == "WF" && !procedure.plus_labels.is_empty() {
                        let sf: Vec<String> = procedure
                            .plus_labels
                            .iter()
                            .map(|l| format!("SF_vars({l}({q_self}))"))
                            .collect();
                        strong = Some(sf.join(" /\\ "));
                    }
                    pairs.push((wf, strong));
                }
                let lines = self.fairness_lines(&prefix, &pairs);
                formula_bytes = lines.iter().fold(formula_bytes, |bytes, line| {
                    bytes.saturating_add(line.len()).saturating_add(1)
                });
                self.check_output(formula_bytes)?;
                formulas.push(lines);
            }
        }
        if !wf_next && formulas.is_empty() {
            self.line(&format!("Spec == {safety}"))?;
            self.line("")?;
            return Ok(());
        }
        self.line(&format!("Spec == /\\ {safety}"))?;
        let indent = "Spec == /\\ ".len();
        if wf_next {
            self.line("        /\\ WF_vars(Next)")?;
        }
        for lines in formulas {
            for (j, l) in lines.iter().enumerate() {
                if j == 0 {
                    self.line(&format!("        /\\ {l}"))?;
                } else {
                    self.line(&format!("{}{l}", spaces(indent)))?;
                }
            }
        }
        self.line("")?;
        Ok(())
    }

    fn termination(&mut self) -> Result<(), Fail> {
        if self.opts.omit_pc || self.opts.omit_stuttering_when_done {
            return Ok(());
        }
        if self.mp {
            self.line("Termination == <>(\\A self \\in ProcSet: pc[self] = \"Done\")")?;
        } else {
            self.line("Termination == <>(pc = \"Done\")")?;
        }
        self.line("")?;
        Ok(())
    }
}
