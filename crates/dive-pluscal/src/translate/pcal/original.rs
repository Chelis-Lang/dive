//! Pre-generation `ParseAlgorithm.getAlgorithm` / `AST.toString` semantics.
//! Original artifacts stop before fixing/generation; debug headers separately
//! retain observable nested-vector mutations without exposing exploded steps.

use super::ast::{
    Algorithm, Body, Clause, Kind, LabeledStmt, Loc, Procedure, Process, Stmt, VarDecl,
};
use super::expr::{Expr, Kind as TokenKind};
use super::{Fail, build, labels, macros};
use crate::{Code, Parsed, TranslationOptions};

pub(crate) fn serialize(parsed: &Parsed, options: &TranslationOptions) -> Result<String, Fail> {
    serialize_stage(parsed, options, false)
}

/// Debug headers retain observable shared-vector mutations. Check the
/// real symbol table; renamed original/header ownership remains unfinished.
pub(crate) fn debug_header(parsed: &Parsed, options: &TranslationOptions) -> Result<String, Fail> {
    serialize_stage(parsed, options, true)
}
fn serialize_stage(
    parsed: &Parsed,
    options: &TranslationOptions,
    header: bool,
) -> Result<String, Fail> {
    if header && !parsed.algorithm.procedures.is_empty() {
        if !parsed.algorithm.macros.is_empty() {
            return Err(Fail::at(
                "unsupported debug AST checksum: procedural macro ownership".into(),
                parsed.algorithm.span.start,
            )
            .code(Code::Unsupported));
        }
        if parsed.algorithm.definitions.is_some() {
            return Err(Fail::at(
                "unsupported debug AST checksum: procedure DEFINE ownership".into(),
                parsed.algorithm.span.start,
            )
            .code(Code::Unsupported));
        }
    }
    let mut budget = macros::Budget::new(
        options.parser.max_nodes.min(20_000),
        options.parser.max_tokens.saturating_mul(4).min(400_000),
        options
            .parser
            .max_bytes
            .saturating_mul(4)
            .min(options.max_output_bytes)
            .min(4_194_304),
        options.parser.max_depth.min(256),
    );
    let mut algorithm = build::Builder::new(parsed, options.parser.max_depth.min(256), &mut budget)
        .build_original()?;
    if let Body::Multi(processes) = &mut algorithm.body {
        for process in processes {
            if process.fairness == super::ast::Fairness::Unfair {
                process.fairness = match options.fairness {
                    crate::FairnessOption::Weak => super::ast::Fairness::Weak,
                    crate::FairnessOption::Strong => super::ast::Fairness::Strong,
                    _ => super::ast::Fairness::Unfair,
                };
            }
        }
    }
    labels::normalize(
        &mut algorithm,
        &labels::LabelOptions {
            label: options.insert_missing_labels,
            root: &options.label_root,
        },
        &mut budget,
    )?;
    super::check_normalized_depth(&algorithm, options.parser.max_depth.min(256))?;
    let action_count = algorithm.prcds.iter().map(|p| p.body.len()).sum::<usize>()
        + match &algorithm.body {
            Body::Uni { body, .. } => body.len(),
            Body::Multi(processes) => processes.iter().map(|p| p.body.len()).sum(),
        };
    if action_count > options.max_actions.min(10_000) {
        return Err(Fail::at(
            "PlusCal labelled-action limit reached".into(),
            parsed.algorithm.span.start,
        )
        .code(Code::Resource));
    }
    super::header::classified_locations(&mut algorithm);
    if header {
        let mut table = super::symtab::SymTab::new(&algorithm)?;
        table.disambiguate();
        if !table.report.is_empty() {
            return Err(Fail::at(
                "unsupported debug AST checksum: identifier renaming ownership".into(),
                parsed.algorithm.span.start,
            )
            .code(Code::Unsupported));
        }
        super::symtab::fix(&mut algorithm, &mut table)?;
        super::check_generated_names(&algorithm)?;
        super::header::mutate(&mut algorithm, &mut budget)?;
    }
    let writer = Writer {
        text: String::new(),
        indents: vec![0],
        debug: options.debug,
        header,
        historical: options.parser.input_version < 1500,
        limit: options.max_output_bytes.min(4_194_304),
        at: parsed.algorithm.span.start,
    };
    writer.algorithm(&algorithm, &parsed.algorithm.name.text)
}

struct Writer {
    text: String,
    indents: Vec<usize>,
    debug: bool,
    header: bool,
    historical: bool,
    limit: usize,
    at: usize,
}
impl Writer {
    fn algorithm(mut self, algorithm: &Algorithm, name: &str) -> Result<String, Fail> {
        let sequential = matches!(algorithm.body, Body::Uni { .. });
        self.located(Loc::default())?;
        self.push(if sequential {
            "[type     |-> \"uniprocess\", "
        } else {
            "[type    |-> \"multiprocess\", "
        })?;
        self.newline()?;
        self.push(&format!(" name  |-> \"{name}\", "))?;
        self.newline()?;
        self.field(
            if sequential {
                " decls  |-> "
            } else {
                " decls |-> "
            },
            |w| w.seq(&algorithm.decls, Writer::decl),
        )?;
        self.push(",")?;
        self.newline()?;
        self.push(if sequential {
            " defs   |-> "
        } else {
            " defs  |-> "
        })?;
        self.expr(&algorithm.defs)?;
        self.push(",")?;
        self.newline()?;
        self.field(
            if sequential {
                " prcds  |-> "
            } else {
                " prcds |-> "
            },
            |w| w.seq(&algorithm.prcds, Writer::procedure),
        )?;
        self.push(",")?;
        self.newline()?;
        match &algorithm.body {
            Body::Uni { body, .. } => {
                self.field(" body  |-> ", |w| w.seq(body, Writer::labeled))?;
            }
            Body::Multi(processes) => {
                self.field(" procs |-> ", |w| w.seq(processes, Writer::process))?;
            }
        }
        self.push("]")?;
        self.end();
        Ok(self.text)
    }

    fn push(&mut self, text: &str) -> Result<(), Fail> {
        if self.text.len().saturating_add(text.len()) > self.limit {
            return Err(Fail::at(
                "PlusCal AST serialization byte limit reached".into(),
                self.at,
            )
            .code(Code::Resource)
            .generation());
        }
        self.text.push_str(text);
        Ok(())
    }
    fn indent(&mut self, prefix: &str) -> Result<(), Fail> {
        let width = self
            .indents
            .last()
            .copied()
            .unwrap_or_default()
            .saturating_add(prefix.len());
        self.push(prefix)?;
        self.indents.push(width);
        Ok(())
    }
    fn end(&mut self) {
        self.indents.pop();
    }
    fn newline(&mut self) -> Result<(), Fail> {
        self.push("\n")?;
        for _ in 0..self.indents.last().copied().unwrap_or_default() {
            self.push(" ")?;
        }
        Ok(())
    }
    fn location(&mut self, loc: Loc) -> Result<(), Fail> {
        if self.debug {
            self.push(&format!("(*{}, {}*)", loc.line, loc.col))?;
        }
        Ok(())
    }
    fn located(&mut self, loc: Loc) -> Result<(), Fail> {
        self.indent(&if self.debug {
            format!("(*{}, {}*)", loc.line, loc.col)
        } else {
            String::new()
        })
    }
    fn field(
        &mut self,
        prefix: &str,
        item: impl FnOnce(&mut Self) -> Result<(), Fail>,
    ) -> Result<(), Fail> {
        self.indent(prefix)?;
        item(self)?;
        self.end();
        Ok(())
    }
    fn seq<T>(
        &mut self,
        items: &[T],
        mut item: impl FnMut(&mut Self, &T) -> Result<(), Fail>,
    ) -> Result<(), Fail> {
        self.indent("<<")?;
        for (index, value) in items.iter().enumerate() {
            if index > 0 {
                self.push(", ")?;
                self.newline()?;
            }
            item(self, value)?;
        }
        self.push(">>")?;
        self.end();
        Ok(())
    }
    fn quoted(&mut self, items: &[String]) -> Result<(), Fail> {
        self.indent("<<")?;
        for (index, value) in items.iter().enumerate() {
            if index > 0 {
                self.push(", ")?;
            }
            self.push(&format!("\"{value}\""))?;
        }
        self.push(">>")?;
        self.end();
        Ok(())
    }
    fn expr(&mut self, expr: &Expr) -> Result<(), Fail> {
        self.push("<< ")?;
        let mut nonempty = false;
        for (index, line) in expr.lines.iter().enumerate() {
            if index > 0 {
                self.push("\n")?;
            }
            for token in line {
                if nonempty {
                    self.push(", ")?;
                }
                nonempty = true;
                if token.kind == TokenKind::Str {
                    let inner = token
                        .text
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .unwrap_or(&token.text);
                    self.push(&format!("\"\\\"\", \"{inner}\", \"\\\"\""))?;
                } else if token.text == "\\/" || token.text == "/\\" {
                    self.push(&format!(
                        "\"(col{}) {}\"",
                        token.col,
                        token.text.replace('\\', "\\\\")
                    ))?;
                } else {
                    self.push(&format!("\"{}\"", token.text.replace('\\', "\\\\")))?;
                }
            }
        }
        self.push(" >>")
    }
    fn decl(&mut self, decl: &VarDecl) -> Result<(), Fail> {
        self.located(decl.loc)?;
        self.push(&format!(
            "[var |-> \"{}\", eqOrIn |-> {}, val |-> ",
            decl.var,
            eq(decl.is_eq)
        ))?;
        self.expr(&decl.val)?;
        self.push("]")?;
        self.end();
        Ok(())
    }
    fn labeled(&mut self, label: &LabeledStmt) -> Result<(), Fail> {
        self.located(label.loc)?;
        self.push(&format!("[label |-> \"{}\",", label.label))?;
        self.newline()?;
        self.field(" stmts |-> ", |w| w.seq(&label.stmts, Self::stmt))?;
        self.push("]")?;
        self.end();
        Ok(())
    }
    fn owner_fields(
        &mut self,
        minus: &[String],
        plus: &[String],
        called: &[String],
    ) -> Result<(), Fail> {
        self.push("minusLabels |-> ")?;
        self.quoted(minus)?;
        self.push(", plusLabels |->")?;
        self.quoted(plus)?;
        self.push(", proceduresCalled |->")?;
        self.quoted(called)?;
        self.push(",")
    }
    fn procedure(&mut self, p: &Procedure) -> Result<(), Fail> {
        self.located(p.loc)?;
        self.push(&format!("[name   |-> \"{}\", ", p.name))?;
        self.newline()?;
        if !self.historical {
            self.owner_fields(&p.minus_labels, &p.plus_labels, &p.procedures_called)?;
            self.newline()?;
        }
        self.field(" params |-> ", |w| w.seq(&p.params, Self::decl))?;
        self.push(",")?;
        self.newline()?;
        self.field(" decls  |-> ", |w| w.seq(&p.decls, Self::decl))?;
        self.push(",")?;
        self.newline()?;
        self.field(" body   |-> ", |w| w.seq(&p.body, Self::labeled))?;
        self.push("]")?;
        self.end();
        Ok(())
    }
    fn process(&mut self, p: &Process) -> Result<(), Fail> {
        self.located(p.loc)?;
        self.push(&format!(
            "[name   |-> \"{}\",{}",
            p.name,
            if self.historical { " " } else { "" }
        ))?;
        self.newline()?;
        if !self.historical {
            let fair = match p.fairness {
                super::ast::Fairness::Unfair => "unfair",
                super::ast::Fairness::Weak => "wf",
                super::ast::Fairness::Strong => "sf",
            };
            self.push(&format!(" fairness |-> \"{fair}\", "))?;
            self.owner_fields(&p.minus_labels, &p.plus_labels, &p.procedures_called)?;
            self.newline()?;
        }
        self.push(&format!(" eqOrIn |-> {},", eq(p.is_eq)))?;
        self.newline()?;
        self.push(" id     |-> ")?;
        self.expr(&p.id)?;
        self.push(",")?;
        self.newline()?;
        self.field(" decls  |-> ", |w| w.seq(&p.decls, Self::decl))?;
        self.push(",")?;
        self.newline()?;
        self.field(" body   |-> ", |w| w.seq(&p.body, Self::labeled))?;
        self.push("]")?;
        self.end();
        Ok(())
    }
    fn stmts(&mut self, prefix: &str, stmts: &[Stmt]) -> Result<(), Fail> {
        self.field(prefix, |w| w.seq(stmts, Self::stmt))
    }
    fn labs(&mut self, prefix: &str, stmts: &[LabeledStmt]) -> Result<(), Fail> {
        self.field(prefix, |w| w.seq(stmts, Self::labeled))
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one exact AST.toString template per semantic node"
    )]
    fn stmt(&mut self, stmt: &Stmt) -> Result<(), Fail> {
        let loc = stmt.loc;
        match &stmt.kind {
            Kind::Skip => {
                self.location(loc)?;
                self.push("[type |-> \"skip\"]")?;
            }
            Kind::Return { from } => {
                self.location(loc)?;
                self.push(&format!(
                    "[type |-> \"return\", from |-> \"{}\"]",
                    from.as_deref().unwrap_or("")
                ))?;
            }
            Kind::Goto(to) => {
                self.location(loc)?;
                self.push(&format!("[type |-> \"goto\",  to |-> \"{to}\"]"))?;
            }
            Kind::Assign(assignments) => {
                if self.header && assignments.len() > 1 {
                    return Err(Fail::at(
                        "unsupported debug AST checksum: simultaneous-assignment ordering".into(),
                        stmt.at,
                    )
                    .code(Code::Unsupported));
                }

                self.located(loc)?;
                self.push("[type |-> \"assignment\",")?;
                self.newline()?;
                self.field(" ass  |-> ", |w| {
                    w.seq(assignments, |w, a| {
                        w.located(a.loc)?;
                        w.push("[lhs |-> ")?;
                        // SubstituteInSingleAssign allocates a fresh Lhs, retaining
                        // the SingleAssign location but leaving Lhs at (0,0).
                        w.location(if a.loc.macro_call.is_some() {
                            Loc::default()
                        } else {
                            a.loc
                        })?;
                        w.push(&format!("[var |-> \"{}\", sub |-> ", a.var))?;
                        w.expr(&a.sub)?;
                        w.push("],")?;
                        w.newline()?;
                        w.push(" rhs |-> ")?;
                        w.expr(&a.rhs)?;
                        w.push("]")?;
                        w.end();
                        Ok(())
                    })
                })?;
                self.push("]")?;
                self.end();
            }
            Kind::If { test, then, els } => {
                self.located(loc)?;
                self.push("[type    |-> \"if\", ")?;
                self.newline()?;
                self.push(" test    |-> ")?;
                self.expr(test)?;
                self.push(",")?;
                self.newline()?;
                self.stmts(" then |-> ", then)?;
                self.push(",")?;
                self.newline()?;
                self.stmts(" else |-> ", els)?;
                self.push("]")?;
                self.end();
            }
            Kind::LabelIf {
                test,
                unlab_then,
                lab_then,
                unlab_else,
                lab_else,
            } => {
                self.located(loc)?;
                self.push("[type      |-> \"labelIf\",")?;
                self.newline()?;
                self.push(" test      |-> ")?;
                self.expr(test)?;
                self.push(",")?;
                self.newline()?;
                self.stmts(" unlabThen |-> ", unlab_then)?;
                self.push(",")?;
                self.newline()?;
                self.labs(" labThen   |-> ", lab_then)?;
                self.push(",")?;
                self.newline()?;
                self.stmts(" unlabElse |-> ", unlab_else)?;
                self.push(",")?;
                self.newline()?;
                self.labs(" labElse   |-> ", lab_else)?;
                self.push("]")?;
                self.newline()?;
                self.end();
            }
            Kind::Either(ors) => {
                self.located(loc)?;
                self.push("[type |-> \"either\", ")?;
                self.newline()?;
                self.field(" ors  |-> ", |w| {
                    w.indent("<< ")?;
                    for (index, seq) in ors.iter().enumerate() {
                        if index > 0 {
                            w.push(", ")?;
                            w.newline()?;
                        }
                        w.seq(seq, Self::stmt)?;
                    }
                    w.push(" >>")?;
                    w.end();
                    Ok(())
                })?;
                self.push("]")?;
                self.end();
            }
            Kind::LabelEither(clauses) => {
                self.located(loc)?;
                self.push("[type    |-> \"labelEither\",")?;
                self.newline()?;
                self.field(" clauses |-> ", |w| w.seq(clauses, Self::clause))?;
                self.push("]")?;
                self.newline()?;
                self.end();
            }
            Kind::While { test, unlab, lab } => {
                self.located(loc)?;
                self.push("[type    |-> \"while\", ")?;
                self.newline()?;
                self.push(" test    |-> ")?;
                self.expr(test)?;
                self.push(",")?;
                self.newline()?;
                self.labs(" labDo   |-> ", lab)?;
                self.push(",")?;
                self.newline()?;
                self.stmts(" unlabDo |-> ", unlab)?;
                self.push("]")?;
                self.end();
            }
            Kind::With {
                var,
                is_eq,
                exp,
                body,
            } => {
                self.located(loc)?;
                self.push("[type   |-> \"with\", ")?;
                self.newline()?;
                self.push(&format!(" var    |-> \"{var}\","))?;
                self.newline()?;
                self.push(&format!(" eqOrIn |-> {},", eq(*is_eq)))?;
                self.newline()?;
                self.push(" exp    |-> ")?;
                self.expr(exp)?;
                self.push(",")?;
                self.newline()?;
                self.stmts(" do     |-> ", body)?;
                self.push("]")?;
                self.end();
            }
            Kind::When(exp) | Kind::Print(exp) | Kind::Assert(exp) => {
                let typ = match stmt.kind {
                    Kind::When(_) => "when",
                    Kind::Print(_) => "print",
                    _ => "assert",
                };
                self.located(loc)?;
                self.push(&format!("[type |-> \"{typ}\", "))?;
                self.newline()?;
                self.push(" exp |-> ")?;
                self.expr(exp)?;
                self.push("]")?;
                self.end();
            }
            Kind::Call { to, args } => {
                self.located(loc)?;
                self.push("[type     |-> \"call\",")?;
                self.newline()?;
                self.push(" returnTo |-> \"\",")?;
                self.newline()?;
                self.push(&format!(" to       |-> \"{to}\","))?;
                self.newline()?;
                self.field(" args     |-> ", |w| w.seq(args, Self::expr))?;
                self.push("]")?;
                self.end();
            }
            Kind::CallReturn { to, args, .. } | Kind::CallGoto { to, args, .. } => {
                let (typ, field, from) = match &stmt.kind {
                    Kind::CallReturn { from, .. } => (
                        "callReturn",
                        " from     |-> ",
                        from.as_deref().unwrap_or(""),
                    ),
                    Kind::CallGoto { after, .. } => ("callGoto", " after    |-> ", after.as_str()),
                    _ => return Err(Fail::internal("invalid call variant")),
                };
                self.located(loc)?;
                self.push(&format!("[type     |-> \"{typ}\","))?;
                self.newline()?;
                self.push(&format!("{field}\"{from}\","))?;
                self.newline()?;
                self.push(&format!(" to       |-> \"{to}\","))?;
                self.newline()?;
                self.indent("args     |-> ")?;
                self.seq(args, Self::expr)?;
                self.push("]")?;
                self.newline()?;
                self.end();
                self.end();
            }
            Kind::MacroCall { .. } => return Err(Fail::internal("unexpanded original macro call")),
        }
        Ok(())
    }
    fn clause(&mut self, clause: &Clause) -> Result<(), Fail> {
        self.located(Loc::default())?;
        self.stmts("[unlabOr |-> ", &clause.unlab)?;
        self.push(",")?;
        self.newline()?;
        self.labs(" labOr   |-> ", &clause.lab)?;
        self.push("]")?;
        self.newline()?;
        self.end();
        Ok(())
    }
}
fn eq(is_eq: bool) -> &'static str {
    if is_eq { "\"=\"" } else { "\"\\\\in\"" }
}
