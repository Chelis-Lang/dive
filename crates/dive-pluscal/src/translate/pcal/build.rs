//! Builds the translator tree from the parsed source (the tree-building half
//! of `pcal.ParseAlgorithm`), including macro definitions.

use super::ast::{
    Algorithm, Body, Facts, Fairness, Kind, Loc, Macro, Procedure, Process, SingleAssign, Stmt,
    VarDecl,
};
use super::expr::{Expr, classify};
use super::{Fail, macros};
use crate::Parsed;
use crate::ast::{self, BindingKind, Block, StatementKind, StmtId, Syntax};

pub(crate) struct Builder<'a> {
    parsed: &'a Parsed,
    /// Collected per process/procedure while its body is built.
    plus: Vec<String>,
    minus: Vec<String>,
    called: Vec<String>,
    current_procedure: Option<String>,
    facts: Facts,
    depth: usize,
    max_depth: usize,
    budget: &'a mut macros::Budget,
    /// Byte offsets of line starts, for positions.
    line_starts: Vec<usize>,
    original: bool,
}

impl<'a> Builder<'a> {
    pub(crate) fn new(
        parsed: &'a Parsed,
        max_depth: usize,
        budget: &'a mut macros::Budget,
    ) -> Self {
        let mut line_starts = vec![0];
        // BufferedReader.readLine treats CR, LF and CRLF as physical line breaks.
        // Keep original byte offsets; the LF member of CRLF supplies one start.
        line_starts.extend(
            parsed
                .source
                .match_indices(['\r', '\n'])
                .filter_map(|(i, end)| {
                    (end == "\n" || parsed.source.as_bytes().get(i + 1) != Some(&b'\n'))
                        .then_some(i + 1)
                }),
        );
        Self {
            line_starts,
            original: false,
            depth: 0,
            max_depth,
            budget,
            parsed,
            plus: Vec::new(),
            minus: Vec::new(),
            called: Vec::new(),
            current_procedure: None,
            facts: Facts {
                omit_pc: true,
                omit_stuttering_when_done: true,
                next_label_num: 1,
                ..Facts::default()
            },
        }
    }

    /// One-based line and column, tabs expanded to multiples of eight.
    fn position(&self, offset: usize) -> (usize, usize) {
        let line = self.line_starts.partition_point(|s| *s <= offset);
        let start = self
            .line_starts
            .get(line.saturating_sub(1))
            .copied()
            .unwrap_or(0);
        let mut column = 1;
        for ch in self.parsed.source.get(start..offset).unwrap_or("").chars() {
            match ch {
                '\t' => column += 8 - (column - 1) % 8,
                '\r' => {}
                _ => column += if self.original { ch.len_utf16() } else { 1 },
            }
        }
        (line, column)
    }

    fn loc(&self, offset: usize) -> Loc {
        let (line, col) = self.position(offset);
        Loc {
            line,
            col,
            macro_call: None,
        }
    }

    /// The expression covering the significant tokens inside `span`.
    pub(crate) fn expr(&self, span: ast::Span) -> Expr {
        let tokens = &self.parsed.tokens;
        let first = tokens.partition_point(|t| t.span.start < span.start);
        let mut out = Vec::new();
        for token in &tokens[first..] {
            if token.span.end > span.end {
                break;
            }
            let (line, column) = self.position(token.span.start);
            let col = column - 1;
            out.push((token.text.clone(), classify(&token.text), line, col));
        }
        if self.original {
            Expr::from_original_tokens(&out)
        } else {
            Expr::from_tokens(&out)
        }
    }

    /// The first significant token at or after `offset`.
    fn token_at(&self, offset: usize) -> Option<&ast::Token> {
        let tokens = &self.parsed.tokens;
        tokens.get(tokens.partition_point(|t| t.span.start < offset))
    }

    /// The significant token immediately before `offset`.
    fn token_before(&self, offset: usize) -> Option<&ast::Token> {
        let tokens = &self.parsed.tokens;
        let index = tokens.partition_point(|t| t.span.start < offset);
        index.checked_sub(1).and_then(|i| tokens.get(i))
    }

    fn decl(&mut self, decl: &ast::Decl) -> VarDecl {
        let (is_eq, val) = if let Some(binding) = &decl.initial {
            (
                binding.kind == BindingKind::Equal,
                self.expr(binding.expression.span),
            )
        } else {
            self.facts.has_default_initialization = true;
            (true, Expr::default_init())
        };
        VarDecl {
            var: decl.name.text.clone(),
            is_eq,
            val,
            loc: self.loc(decl.name.span.start),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "build source declarations and bodies in reference traversal order"
    )]
    pub(crate) fn build(self) -> Result<Algorithm, Fail> {
        self.build_mode(false)
    }

    /// Original-stage ownership: the last sequential procedure's parser
    /// vectors remain shared while the main body is read. Generation uses
    /// its existing independently owned vectors.
    pub(crate) fn build_original(mut self) -> Result<Algorithm, Fail> {
        self.original = true;
        self.build_mode(true)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "build declarations and bodies in reference traversal order"
    )]
    fn build_mode(mut self, original: bool) -> Result<Algorithm, Fail> {
        let parsed = self.parsed;
        let algorithm = &parsed.algorithm;
        let decls: Vec<VarDecl> = algorithm.variables.iter().map(|d| self.decl(d)).collect();
        let defs = algorithm
            .definitions
            .map_or_else(Expr::empty, |e| self.expr(e.span));
        let mut macro_defs: Vec<Macro> = Vec::new();
        for definition in &algorithm.macros {
            if definition
                .parameters
                .iter()
                .enumerate()
                .any(|(index, parameter)| {
                    definition.parameters[..index]
                        .iter()
                        .any(|previous| previous.text == parameter.text)
                })
            {
                return Err(Fail::at(
                    "unsupported PlusCal translation: duplicate macro formal parameters".into(),
                    definition.span.start,
                )
                .code(crate::Code::Unsupported));
            }
            let mut body = self.block(&definition.body)?;
            macros::expand_in(&mut body, &macro_defs, self.budget)?;
            macro_defs.push(Macro {
                name: definition.name.text.clone(),
                params: definition
                    .parameters
                    .iter()
                    .map(|p| p.text.clone())
                    .collect(),
                body,
            });
        }
        let mut prcds = Vec::new();
        for procedure in &algorithm.procedures {
            self.plus.clear();
            self.minus.clear();
            self.called.clear();
            self.current_procedure = Some(procedure.name.text.clone());
            let params = procedure.parameters.iter().map(|d| self.decl(d)).collect();
            let local = procedure.variables.iter().map(|d| self.decl(d)).collect();
            let stmts = self.block(&procedure.body)?;
            self.current_procedure = None;
            prcds.push(Procedure {
                name: procedure.name.text.clone(),
                params,
                decls: local,
                stmts,
                body: Vec::new(),
                plus_labels: if original {
                    self.plus.clone()
                } else {
                    std::mem::take(&mut self.plus)
                },
                minus_labels: if original {
                    self.minus.clone()
                } else {
                    std::mem::take(&mut self.minus)
                },
                procedures_called: if original {
                    self.called.clone()
                } else {
                    std::mem::take(&mut self.called)
                },
                loc: self.loc(procedure.span.start),
            });
            // A procedure is assumed to be called, so the pc is needed.
            self.facts.omit_pc = false;
        }
        let body = match &algorithm.body {
            ast::Body::Sequential(block) => {
                if !original {
                    self.plus.clear();
                    self.minus.clear();
                    self.called.clear();
                }
                let stmts = self.block(block)?;
                if original && let Some(last) = prcds.last_mut() {
                    last.plus_labels.clone_from(&self.plus);
                    last.minus_labels.clone_from(&self.minus);
                    last.procedures_called.clone_from(&self.called);
                }
                Body::Uni {
                    stmts,
                    body: Vec::new(),
                }
            }
            ast::Body::Processes(processes) => {
                let mut built = Vec::new();
                for process in processes {
                    self.plus.clear();
                    self.minus.clear();
                    self.called.clear();
                    let id = self.expr(process.id.expression.span);
                    let decls = process.variables.iter().map(|d| self.decl(d)).collect();
                    let stmts = self.block(&process.body)?;
                    let keyword =
                        self.token_at(process.span.start)
                            .filter(|t| t.text == "process")
                            .or_else(|| {
                                self.parsed.tokens.iter().find(|t| {
                                    t.span.start >= process.span.start && t.text == "process"
                                })
                            })
                            .map_or(process.span.start, |t| t.span.start);
                    built.push(Process {
                        name: process.name.text.clone(),
                        fairness: match process.fairness {
                            ast::Fairness::Unfair => Fairness::Unfair,
                            ast::Fairness::Weak => Fairness::Weak,
                            ast::Fairness::Strong => Fairness::Strong,
                        },
                        is_eq: process.id.kind == BindingKind::Equal,
                        id,
                        decls,
                        stmts,
                        body: Vec::new(),
                        plus_labels: std::mem::take(&mut self.plus),
                        minus_labels: std::mem::take(&mut self.minus),
                        procedures_called: std::mem::take(&mut self.called),
                        loc: self.loc(keyword),
                    });
                }
                Body::Multi(built)
            }
        };
        Ok(Algorithm {
            fair: algorithm.fairness == ast::Fairness::Weak,
            decls,
            defs,
            macros: macro_defs,
            prcds,
            body,
            facts: self.facts,
        })
    }

    /// Converts a block, flattening C-syntax braces and merging
    /// `call ...; return;` and `call ...; goto l;`.
    fn deeper(&mut self, by: usize, at: usize) -> Result<(), Fail> {
        self.depth += by;
        if self.depth > self.max_depth {
            return Err(Fail::at(
                "PlusCal normalized control-flow depth limit reached".into(),
                at,
            )
            .code(crate::Code::Resource));
        }
        Ok(())
    }

    fn block(&mut self, block: &Block) -> Result<Vec<Stmt>, Fail> {
        self.deeper(1, block.span.start)?;
        let result = self.block_inner(block);
        self.depth -= 1;
        result
    }

    fn block_inner(&mut self, block: &Block) -> Result<Vec<Stmt>, Fail> {
        let mut out = Vec::new();
        let ids = &block.statements;
        let mut i = 0;
        while i < ids.len() {
            let statement = self.statement_ref(ids[i])?;
            if let StatementKind::Call { name, arguments } = &statement.kind {
                let next = ids.get(i + 1).and_then(|id| self.parsed.statement(*id));
                let follow = next.filter(|n| n.label.is_none());
                if let Some(next) = follow
                    && matches!(next.kind, StatementKind::Return | StatementKind::Goto(_))
                {
                    let mut stmt = self.statement(statement)?;
                    let args = arguments.iter().map(|a| self.expr(a.span)).collect();
                    stmt.kind = match &next.kind {
                        StatementKind::Goto(after) => {
                            self.facts.goto_used = true;
                            // pcal.trans tests the procedure name here, not the target.
                            if name.text == "Done" || name.text == "\"Done\"" {
                                self.facts.goto_done_used = true;
                            }
                            Kind::CallGoto {
                                to: name.text.clone(),
                                after: unquote(&after.text),
                                args,
                            }
                        }
                        _ => Kind::CallReturn {
                            to: name.text.clone(),
                            from: self.current_procedure.clone(),
                            args,
                        },
                    };
                    out.push(stmt);
                    i += 2;
                    continue;
                }
            }
            if let StatementKind::Block(child) = &statement.kind {
                let mut inner = self.block(child)?;
                if let Some(label) = &statement.label {
                    self.label(label);
                    if let Some(first) = inner.first_mut() {
                        if !first.lbl.is_empty() {
                            return Err(Fail::at(
                                format!(
                                    "Duplicate labeling of statement\n    at {}",
                                    first.loc.describe()
                                ),
                                first.at,
                            ));
                        }
                        first.lbl.clone_from(&label.name.text);
                    }
                }
                out.extend(inner);
            } else {
                out.push(self.statement(statement)?);
            }
            i += 1;
        }
        Ok(out)
    }

    fn statement_ref(&self, id: StmtId) -> Result<&'a ast::Statement, Fail> {
        self.parsed
            .statement(id)
            .ok_or_else(|| Fail::internal("statement id out of range"))
    }

    fn label(&mut self, label: &ast::Label) {
        self.facts.has_label = true;
        if !self.facts.all_labels.contains(&label.name.text) {
            self.facts.all_labels.push(label.name.text.clone());
        }
        match label.modifier {
            Some('+') => {
                self.plus.push(label.name.text.clone());
                self.facts.omit_pc = false;
            }
            Some('-') => {
                self.minus.push(label.name.text.clone());
                self.facts.omit_pc = false;
            }
            _ => {}
        }
    }

    /// The start of a statement after its label.
    fn start(statement: &ast::Statement) -> usize {
        statement
            .label
            .as_ref()
            .map_or(statement.span.start, |l| l.span.end)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per PlusCal statement form, mirroring ParseAlgorithm.GetStmt"
    )]
    fn statement(&mut self, statement: &ast::Statement) -> Result<Stmt, Fail> {
        if let Some(label) = &statement.label {
            self.label(label);
        }
        let start = Self::start(statement);
        let first = self.token_at(start).map_or(start, |t| t.span.start);
        let loc = self.loc(first);
        let kind = match &statement.kind {
            StatementKind::Assign(assignments) => {
                let mut ass = Vec::new();
                for a in assignments {
                    ass.push(SingleAssign {
                        var: a.variable.text.clone(),
                        sub: a.selectors.map_or_else(Expr::empty, |s| self.expr(s.span)),
                        rhs: self.expr(a.value.span),
                        loc: self.loc(a.variable.span.start),
                    });
                }
                Kind::Assign(ass)
            }
            StatementKind::Block(_) => {
                return Err(Fail::internal("unflattened statement block"));
            }
            StatementKind::While { test, body } => {
                let unlab = self.block(body)?;
                if unlab.is_empty() {
                    return Err(Fail::at(
                        format!("Missing body of while statement at\n    {}", loc.describe()),
                        first,
                    ));
                }
                Kind::While {
                    test: self.expr(test.span),
                    unlab,
                    lab: Vec::new(),
                }
            }
            StatementKind::If {
                branches,
                otherwise,
            } => {
                self.deeper(branches.len(), first)?;
                self.depth -= branches.len();
                // Convert in source order (labels, calls and gotos are
                // collected as a side effect), then nest each `elsif` in the
                // else part of the branch before it.
                let mut converted = Vec::new();
                for (index, branch) in branches.iter().enumerate() {
                    let then = self.block(&branch.body)?;
                    let test = self.expr(branch.test.span);
                    let (loc_here, at) = if index == 0 {
                        (loc, first)
                    } else {
                        let keyword = self
                            .token_before(branch.test.span.start)
                            .filter(|t| t.text == "elsif")
                            .map_or(branch.test.span.start, |t| t.span.start);
                        (self.loc(keyword), keyword)
                    };
                    converted.push((test, then, loc_here, at));
                }
                let mut els = match otherwise {
                    Some(block) => self.block(block)?,
                    None => Vec::new(),
                };
                for (test, then, loc_here, at) in converted.into_iter().rev() {
                    let kind = Kind::LabelIf {
                        test,
                        unlab_then: then,
                        lab_then: Vec::new(),
                        unlab_else: els,
                        lab_else: Vec::new(),
                    };
                    els = vec![Stmt::new(kind, loc_here, at)];
                }
                return Ok(match els.pop() {
                    Some(mut stmt) => {
                        stmt.lbl = statement
                            .label
                            .as_ref()
                            .map(|l| l.name.text.clone())
                            .unwrap_or_default();
                        stmt
                    }
                    None => return Err(Fail::internal("if without branches")),
                });
            }
            StatementKind::Either(branches) => {
                let mut clauses = Vec::new();
                for branch in branches {
                    let unlab = self.block(branch)?;
                    if unlab.is_empty() {
                        return Err(Fail::at(
                            format!(
                                "`either' statement with empty `or' clause\n    at {}",
                                loc.describe()
                            ),
                            first,
                        ));
                    }
                    clauses.push(super::ast::Clause {
                        unlab,
                        lab: Vec::new(),
                    });
                }
                Kind::LabelEither(clauses)
            }
            StatementKind::With { bindings, body } => {
                let inner = self.block(body)?;
                let mut kind = None;
                let mut body = Some(inner);
                for (index, binding) in bindings.iter().enumerate().rev() {
                    let exp = self.expr(binding.binding.expression.span);
                    let do_ = match kind.take() {
                        None => body.take().unwrap_or_default(),
                        Some((k, l, a)) => vec![Stmt::new(k, l, a)],
                    };
                    let with = Kind::With {
                        var: binding.name.text.clone(),
                        is_eq: binding.binding.kind == BindingKind::Equal,
                        exp,
                        body: do_,
                    };
                    // The location is the token before the variable: `with`,
                    // C-syntax `(`, or the separator of a later binding.
                    let before = self
                        .token_before(binding.name.span.start)
                        .map_or(binding.name.span.start, |t| t.span.start);
                    let at = if index == 0 { first } else { before };
                    let loc_here = if index == 0 && self.parsed.algorithm.syntax == Syntax::C {
                        self.loc(before)
                    } else {
                        self.loc(at)
                    };
                    kind = Some((with, loc_here, at));
                }
                match kind {
                    Some((k, l, _)) => {
                        let mut stmt = Stmt::new(k, l, first);
                        stmt.lbl = statement
                            .label
                            .as_ref()
                            .map(|l| l.name.text.clone())
                            .unwrap_or_default();
                        return Ok(stmt);
                    }
                    None => return Err(Fail::internal("with without bindings")),
                }
            }
            StatementKind::Await { condition, .. } => Kind::When(self.expr(condition.span)),
            StatementKind::Assert(e) => Kind::Assert(self.expr(e.span)),
            StatementKind::Print(e) => Kind::Print(self.expr(e.span)),
            StatementKind::Skip => Kind::Skip,
            StatementKind::Call { name, arguments } => {
                if !self.called.contains(&name.text) {
                    self.called.push(name.text.clone());
                }
                Kind::Call {
                    to: name.text.clone(),
                    args: arguments.iter().map(|a| self.expr(a.span)).collect(),
                }
            }
            StatementKind::Return => Kind::Return {
                from: self.current_procedure.clone(),
            },
            StatementKind::Goto(target) => {
                self.facts.goto_used = true;
                if target.text == "Done" || target.text == "\"Done\"" {
                    self.facts.goto_done_used = true;
                }
                Kind::Goto(unquote(&target.text))
            }
            StatementKind::MacroCall { name, arguments } => Kind::MacroCall {
                name: name.text.clone(),
                args: arguments.iter().map(|a| self.expr(a.span)).collect(),
            },
        };
        let mut stmt = Stmt::new(kind, loc, first);
        if let Some(label) = &statement.label {
            stmt.lbl.clone_from(&label.name.text);
        }
        Ok(stmt)
    }
}

/// The reference's token reader returns a string's contents (`goto "Done"`).
fn unquote(text: &str) -> String {
    text.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text)
        .to_owned()
}

#[cfg(test)]
mod physical_line_tests {
    use super::Builder;
    use super::macros::Budget;

    #[test]
    fn physical_newlines_preserve_original_spans_and_expression_line_groups()
    -> Result<(), Box<dyn std::error::Error>> {
        for newline in ["\n", "\r", "\r\n"] {
            let source = format!(
                "--algorithm A{newline}variable x=0;{newline}begin{newline}\tL:x:=1+{newline}\t2;{newline}E:skip;{newline}end algorithm;"
            );
            let parsed = crate::parse(&source, &crate::Options::default())?;
            assert_eq!(parsed.source, source);
            let mut budget = Budget::new(50_000, 100_000, 4_194_304, 128);
            let builder = Builder::new(&parsed, 128, &mut budget);
            let label = source.find("L:").ok_or("label")?;
            let second = source.find("2;").ok_or("second expression line")?;
            assert_eq!(builder.position(label), (4, 9));
            assert_eq!(builder.position(second), (5, 9));
            let token = parsed
                .tokens
                .iter()
                .find(|token| token.text == "2")
                .ok_or("token")?;
            assert_eq!(token.span.start, second);
            assert_eq!(&source[token.span.start..token.span.end], "2");
            let expression = builder.expr(crate::ast::Span {
                start: source.find("1+").ok_or("first expression line")?,
                end: second + 1,
            });
            assert_eq!(expression.lines.len(), 2);
        }
        Ok(())
    }

    #[test]
    fn mixed_newlines_and_tabs_keep_each_original_byte_location()
    -> Result<(), Box<dyn std::error::Error>> {
        let source =
            "--algorithm A\nvariable x=0;\rbegin\r\n\tL:x:=1+\n\t2;\rE:skip;\rend algorithm;";
        let parsed = crate::parse(source, &crate::Options::default())?;
        let mut budget = Budget::new(50_000, 100_000, 4_194_304, 128);
        let builder = Builder::new(&parsed, 128, &mut budget);
        for (needle, expected) in [
            ("L:", (4, 9)),
            ("1+", (4, 14)),
            ("2;", (5, 9)),
            ("E:", (6, 1)),
            ("end algorithm", (7, 1)),
        ] {
            let offset = source.find(needle).ok_or("source location")?;
            assert_eq!(builder.position(offset), expected, "{needle}");
            assert_eq!(&parsed.source[offset..offset + needle.len()], needle);
        }
        Ok(())
    }
}
