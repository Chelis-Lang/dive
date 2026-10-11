//! Bounded source-owned macro substitution, before label/control normalization.
use super::{TranslationError, TranslationOptions, TranslationStage, error};
use crate::{
    Code, Parsed,
    ast::{
        Assignment, Binding, Block, Body, Branch, Expr, Macro, Name, Span, Statement,
        StatementKind, StmtId, Token, WithBinding,
    },
};
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

#[derive(Clone)]
struct Expression {
    tokens: Vec<Token>,
    text: String,
    origin: Span,
}
/// Internal token resolution; the public source AST is never rewritten.
#[derive(Clone)]
pub(super) struct Expanded {
    parsed: Parsed,
    declared_macros: bool,
    expressions: BTreeMap<(usize, usize), Expression>,
    statement_origins: BTreeMap<usize, (Span, Span)>,
    calls: BTreeMap<(usize, usize), Span>,
}
impl Deref for Expanded {
    type Target = Parsed;
    fn deref(&self) -> &Parsed {
        &self.parsed
    }
}
impl DerefMut for Expanded {
    fn deref_mut(&mut self) -> &mut Parsed {
        &mut self.parsed
    }
}
impl Expanded {
    pub(super) const fn declared_macros(&self) -> bool {
        self.declared_macros
    }
    pub(super) fn macro_origin(
        &self,
        id: StmtId,
    ) -> Option<(Span, crate::ast::Position, crate::ast::Position)> {
        let (definition, call) = *self.statement_origins.get(&id.0)?;
        let mut start = definition.start;
        if self.algorithm.syntax == crate::ast::Syntax::C
            && matches!(self.statement(id)?.kind, StatementKind::With { .. })
        {
            start = self
                .parsed
                .tokens
                .iter()
                .find(|t| {
                    t.span.start >= definition.start
                        && t.span.end <= definition.end
                        && t.text == "("
                })
                .map_or(start, |t| t.span.start);
        }
        Some((
            definition,
            self.position(start)?,
            self.position(call.start)?,
        ))
    }
    pub(super) fn assertion_location(&self, span: Span, expression: Expr) -> Option<String> {
        self.statements
            .iter()
            .enumerate()
            .find_map(|(index, statement)| {
                if statement.span != span
                    || !matches!(statement.kind, StatementKind::Assert(e) if e==expression)
                {
                    return None;
                }
                let (_, definition, call) = self.macro_origin(StmtId(index))?;
                Some(format!(
                    "line {}, column {} of macro called at line {}, column {}",
                    definition.line, definition.column, call.line, call.column
                ))
            })
    }
    pub(super) fn tokens(&self, expression: Expr) -> &[Token] {
        if let Some(value) = self
            .expressions
            .get(&(expression.span.start, expression.span.end))
        {
            return &value.tokens;
        }
        let start = self
            .parsed
            .tokens
            .partition_point(|t| t.span.start < expression.span.start);
        let end = self
            .parsed
            .tokens
            .partition_point(|t| t.span.end <= expression.span.end);
        self.parsed.tokens.get(start..end).unwrap_or_default()
    }
    pub(super) fn text(&self, span: Span) -> Option<&str> {
        self.expressions
            .get(&(span.start, span.end))
            .map(|e| e.text.as_str())
            .or_else(|| self.parsed.text(span))
    }
    pub(super) fn origin(&self, span: Span) -> Span {
        self.expressions
            .get(&(span.start, span.end))
            .map_or(span, |e| e.origin)
    }
    pub(super) fn semantic_tokens(&self, span: Span) -> Vec<Token> {
        let mut words = BTreeMap::new();
        let mut add = |token: &Token| {
            words
                .entry(token.text.clone())
                .or_insert_with(|| token.clone());
        };
        for token in &self.parsed.tokens {
            if token.span.start >= span.start && token.span.end <= span.end {
                add(token);
            }
        }
        let mut expressions = std::collections::BTreeSet::new();
        for statement in &self.statements {
            if statement.span.start < span.start || statement.span.end > span.end {
                continue;
            }
            let mut exprs = Vec::new();
            match &statement.kind {
                StatementKind::Assign(assignments) => {
                    for a in assignments {
                        add(&Token {
                            text: a.variable.text.clone(),
                            span: a.variable.span,
                        });
                        exprs.push(a.value);
                        exprs.extend(a.selectors);
                    }
                }
                StatementKind::While { test, .. } => exprs.push(*test),
                StatementKind::If { branches, .. } => exprs.extend(branches.iter().map(|b| b.test)),
                StatementKind::With { bindings, .. } => {
                    exprs.extend(bindings.iter().map(|b| b.binding.expression));
                }
                StatementKind::Await { condition, .. } => exprs.push(*condition),
                StatementKind::Assert(expr) | StatementKind::Print(expr) => exprs.push(*expr),
                _ => {}
            }
            for expr in exprs {
                if expressions.insert((expr.span.start, expr.span.end)) {
                    for token in self.tokens(expr) {
                        add(token);
                    }
                }
            }
        }
        words.into_values().collect()
    }
}
fn failure(code: Code, span: Span, message: impl Into<String>) -> TranslationError {
    error(TranslationStage::Normalization, code, span, message)
}
fn unsupported(span: Span, feature: &str) -> TranslationError {
    failure(
        Code::Unsupported,
        span,
        format!("unsupported PlusCal macro expansion: {feature}"),
    )
}
#[derive(Default)]
struct TokenBuffer {
    tokens: Vec<Token>,
    bytes: usize,
}
struct Expander<'a> {
    expanded: Expanded,
    originals: Vec<Statement>,
    definitions: Vec<Macro>,
    options: &'a TranslationOptions,
    tokens: usize,
    bytes: usize,
}
impl Expander<'_> {
    fn charge_kind(&mut self, kind: &StatementKind, origin: Span) -> Result<(), TranslationError> {
        let mut expressions = Vec::new();
        let mut name_bytes = 0usize;
        match kind {
            StatementKind::Assign(assignments) => {
                for a in assignments {
                    expressions.push(a.value);
                    expressions.extend(a.selectors);
                    name_bytes = name_bytes.saturating_add(a.variable.text.len());
                }
            }
            StatementKind::While { test, .. } => expressions.push(*test),
            StatementKind::If { branches, .. } => {
                expressions.extend(branches.iter().map(|b| b.test));
            }
            StatementKind::With { bindings, .. } => {
                expressions.extend(bindings.iter().map(|b| b.binding.expression));
            }
            StatementKind::Await { condition, .. } => expressions.push(*condition),
            StatementKind::Assert(expr) | StatementKind::Print(expr) => expressions.push(*expr),
            _ => {}
        }
        let mut tokens = 0usize;
        let mut bytes = name_bytes;
        for expression in expressions {
            let words = self.expanded.tokens(expression);
            tokens = tokens.saturating_add(words.len());
            bytes = words.iter().fold(bytes, |n, t| {
                n.saturating_add(t.text.len().saturating_add(1))
            });
        }
        self.tokens = self.tokens.saturating_add(tokens);
        self.bytes = self.bytes.saturating_add(bytes);
        if self.tokens
            > self
                .options
                .parser
                .max_tokens
                .saturating_mul(4)
                .min(400_000)
            || self.bytes
                > self
                    .options
                    .parser
                    .max_bytes
                    .saturating_mul(4)
                    .min(4_194_304)
        {
            return Err(failure(
                Code::Resource,
                origin,
                "PlusCal resolved macro expansion budget reached",
            ));
        }
        Ok(())
    }
    fn push_statement(&mut self, statement: Statement) -> Result<StmtId, TranslationError> {
        if self.expanded.statements.len()
            >= self.options.parser.max_nodes.saturating_mul(4).min(200_000)
        {
            return Err(failure(
                Code::Resource,
                statement.span,
                "PlusCal expanded macro node limit reached",
            ));
        }
        let id = StmtId(self.expanded.statements.len());
        self.expanded.statements.push(statement);
        Ok(id)
    }
    fn append(
        &self,
        output: &mut TokenBuffer,
        tokens: &[Token],
        origin: Span,
    ) -> Result<(), TranslationError> {
        let limit = self
            .options
            .parser
            .max_tokens
            .saturating_mul(4)
            .min(400_000)
            .saturating_sub(self.tokens);
        if output.tokens.len().saturating_add(tokens.len()) > limit {
            return Err(failure(
                Code::Resource,
                origin,
                "PlusCal expanded macro token limit reached",
            ));
        }
        let bytes = tokens.iter().fold(output.bytes, |n, t| {
            n.saturating_add(t.text.len().saturating_add(1))
        });
        let byte_limit = self
            .options
            .parser
            .max_bytes
            .saturating_mul(4)
            .min(4_194_304)
            .saturating_sub(self.bytes);
        if bytes > byte_limit {
            return Err(failure(
                Code::Resource,
                origin,
                "PlusCal expanded macro byte limit reached before token allocation",
            ));
        }
        output.bytes = bytes;
        output.tokens.extend_from_slice(tokens);
        Ok(())
    }
    fn store(&mut self, tokens: Vec<Token>, origin: Span) -> Result<Expr, TranslationError> {
        let bytes = tokens
            .iter()
            .try_fold(0usize, |n, t| n.checked_add(t.text.len().saturating_add(1)))
            .ok_or_else(|| failure(Code::Resource, origin, "PlusCal macro byte count overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| failure(Code::Resource, origin, "PlusCal macro byte count overflow"))?;
        if self.bytes
            > self
                .options
                .parser
                .max_bytes
                .saturating_mul(4)
                .min(4_194_304)
        {
            return Err(failure(
                Code::Resource,
                origin,
                "PlusCal expanded macro byte limit reached",
            ));
        }
        self.tokens = self
            .tokens
            .checked_add(tokens.len())
            .ok_or_else(|| failure(Code::Resource, origin, "PlusCal macro token count overflow"))?;
        let start = self
            .expanded
            .source
            .len()
            .checked_add(self.expanded.expressions.len().saturating_mul(2))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| {
                failure(
                    Code::Resource,
                    origin,
                    "PlusCal macro expression index overflow",
                )
            })?;
        let end = start.checked_add(1).ok_or_else(|| {
            failure(
                Code::Resource,
                origin,
                "PlusCal macro expression index overflow",
            )
        })?;
        let text = tokens
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        self.expanded.expressions.insert(
            (start, end),
            Expression {
                tokens,
                text,
                origin,
            },
        );
        Ok(Expr {
            span: Span { start, end },
        })
    }
    fn substitute(
        &mut self,
        expression: Expr,
        environment: &BTreeMap<String, Vec<Token>>,
        origin: Span,
    ) -> Result<Expr, TranslationError> {
        if environment.is_empty() {
            return Ok(expression);
        }
        if self
            .expanded
            .text(expression.span)
            .is_some_and(|text| text.contains(['\n', '\r']))
        {
            return Err(unsupported(
                origin,
                "multiline argument/expression substitution",
            ));
        }
        let original = self.expanded.tokens(expression).to_vec();
        let mut output = TokenBuffer::default();
        let mut changed = false;
        let mut i = 0;
        while i < original.len() {
            let token = &original[i];
            let skip = if token.text == "." {
                2
            } else if ["[", ","].contains(&token.text.as_str())
                && original
                    .get(i + 2)
                    .is_some_and(|t| [":", "|->", "↦"].contains(&t.text.as_str()))
            {
                3
            } else {
                0
            };
            if skip > 0 {
                let end = (i + skip).min(original.len());
                self.append(&mut output, &original[i..end], origin)?;
                i = end;
                continue;
            }
            if let Some(argument) = environment.get(&token.text) {
                changed = true;
                let parentheses = original.len() > 1 && argument.len() > 1;
                if parentheses {
                    self.append(
                        &mut output,
                        &[Token {
                            text: "(".into(),
                            span: origin,
                        }],
                        origin,
                    )?;
                }
                self.append(&mut output, argument, origin)?;
                if parentheses {
                    self.append(
                        &mut output,
                        &[Token {
                            text: ")".into(),
                            span: origin,
                        }],
                        origin,
                    )?;
                }
            } else {
                self.append(&mut output, std::slice::from_ref(token), origin)?;
            }
            i += 1;
        }
        if changed {
            self.store(output.tokens, origin)
        } else {
            Ok(expression)
        }
    }
    fn assignment(
        &mut self,
        assignment: &Assignment,
        environment: &BTreeMap<String, Vec<Token>>,
        origin: Span,
    ) -> Result<Assignment, TranslationError> {
        let mut result = assignment.clone();
        result.span = origin;
        result.value = self.substitute(assignment.value, environment, origin)?;
        result.selectors = assignment
            .selectors
            .map(|e| self.substitute(e, environment, origin))
            .transpose()?;
        if let Some(argument) = environment.get(&assignment.variable.text) {
            let first = argument.first().ok_or_else(|| {
                failure(Code::Expression, origin, "empty macro assignment argument")
            })?;
            let mut chars = first.text.chars();
            if !chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(failure(
                    Code::Expression,
                    origin,
                    format!(
                        "macro expansion substitutes {:?} for assignment variable",
                        first.text
                    ),
                ));
            }
            result.variable = Name {
                text: first.text.clone(),
                span: origin,
            };
            if argument.len() > 1 {
                let mut selectors = TokenBuffer::default();
                self.append(&mut selectors, &argument[1..], origin)?;
                if let Some(selector) = result.selectors {
                    self.append(&mut selectors, self.expanded.tokens(selector), origin)?;
                }
                result.selectors = Some(self.store(selectors.tokens, origin)?);
            }
        }
        Ok(result)
    }
    fn block(
        &mut self,
        block: &Block,
        environment: &BTreeMap<String, Vec<Token>>,
        placement: Option<Span>,
        template: bool,
        depth: usize,
    ) -> Result<Block, TranslationError> {
        if depth > self.options.parser.max_depth.min(256) {
            return Err(failure(
                Code::Resource,
                placement.unwrap_or(block.span),
                "PlusCal macro nesting limit reached",
            ));
        }
        let mut statements = Vec::new();
        for id in &block.statements {
            let statement = if template {
                self.expanded.statement(*id)
            } else {
                self.originals.get(id.0)
            }
            .cloned()
            .ok_or_else(|| {
                failure(
                    Code::Internal,
                    block.span,
                    "invalid macro statement arena reference",
                )
            })?;
            let definition = if template {
                self.expanded
                    .statement_origins
                    .get(&id.0)
                    .map_or(statement.span, |(source, _)| *source)
            } else {
                statement.span
            };
            let expanded = self.statement(
                &statement,
                environment,
                placement,
                template,
                definition,
                depth + 1,
            )?;
            if statements.len().saturating_add(expanded.len())
                > self.options.parser.max_nodes.saturating_mul(4).min(200_000)
            {
                return Err(failure(
                    Code::Resource,
                    placement.unwrap_or(block.span),
                    "PlusCal expanded macro block limit reached",
                ));
            }
            statements.extend(expanded);
        }
        Ok(Block {
            span: placement.unwrap_or(block.span),
            statements,
        })
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one bounded typed-statement substitution pass keeps every syntax variant accounted"
    )]
    fn statement(
        &mut self,
        statement: &Statement,
        environment: &BTreeMap<String, Vec<Token>>,
        placement: Option<Span>,
        template: bool,
        definition: Span,
        depth: usize,
    ) -> Result<Vec<StmtId>, TranslationError> {
        let origin = placement.unwrap_or(statement.span);
        let kind = match &statement.kind {
            StatementKind::MacroCall { name, arguments } => {
                let definition = self
                    .definitions
                    .iter()
                    .find(|m| m.name.text == name.text)
                    .cloned()
                    .ok_or_else(|| {
                        failure(
                            Code::Syntax,
                            origin,
                            format!(
                                "macro {} undefined (only earlier definitions can be expanded)",
                                name.text
                            ),
                        )
                    })?;
                if definition.parameters.len() != arguments.len() {
                    return Err(failure(
                        Code::Syntax,
                        origin,
                        format!("macro {} called with wrong number of arguments", name.text),
                    ));
                }
                let mut bindings = BTreeMap::new();
                for (parameter, argument) in definition.parameters.iter().zip(arguments) {
                    let expression = self.substitute(*argument, environment, origin)?;
                    if bindings
                        .insert(
                            parameter.text.clone(),
                            self.expanded.tokens(expression).to_vec(),
                        )
                        .is_some()
                    {
                        return Err(unsupported(origin, "duplicate formal parameters"));
                    }
                }
                let mut result =
                    self.block(&definition.body, &bindings, Some(origin), true, depth + 1)?;
                if let (Some(first), Some(label)) = (result.statements.first(), &statement.label) {
                    self.expanded.statements[first.0].label = Some(label.clone());
                }
                return Ok(std::mem::take(&mut result.statements));
            }
            StatementKind::Assign(assignments) => StatementKind::Assign(
                assignments
                    .iter()
                    .map(|a| self.assignment(a, environment, origin))
                    .collect::<Result<_, _>>()?,
            ),
            StatementKind::Block(block) => StatementKind::Block(self.block(
                block,
                environment,
                placement,
                template,
                depth + 1,
            )?),
            StatementKind::While { test, body } => StatementKind::While {
                test: self.substitute(*test, environment, origin)?,
                body: self.block(body, environment, placement, template, depth + 1)?,
            },
            StatementKind::If {
                branches,
                otherwise,
            } => StatementKind::If {
                branches: branches
                    .iter()
                    .map(|b| {
                        Ok(Branch {
                            test: self.substitute(b.test, environment, origin)?,
                            body: self.block(
                                &b.body,
                                environment,
                                placement,
                                template,
                                depth + 1,
                            )?,
                        })
                    })
                    .collect::<Result<_, TranslationError>>()?,
                otherwise: otherwise
                    .as_ref()
                    .map(|b| self.block(b, environment, placement, template, depth + 1))
                    .transpose()?,
            },
            StatementKind::Either(choices) => StatementKind::Either(
                choices
                    .iter()
                    .map(|b| self.block(b, environment, placement, template, depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
            StatementKind::With { bindings, body } => StatementKind::With {
                bindings: bindings
                    .iter()
                    .map(|b| {
                        Ok(WithBinding {
                            name: b.name.clone(),
                            binding: Binding {
                                kind: b.binding.kind,
                                expression: self.substitute(
                                    b.binding.expression,
                                    environment,
                                    origin,
                                )?,
                            },
                        })
                    })
                    .collect::<Result<_, TranslationError>>()?,
                body: self.block(body, environment, placement, template, depth + 1)?,
            },
            StatementKind::Await { when, condition } => StatementKind::Await {
                when: *when,
                condition: self.substitute(*condition, environment, origin)?,
            },
            StatementKind::Assert(expression) => {
                StatementKind::Assert(self.substitute(*expression, environment, origin)?)
            }

            StatementKind::Print(expression) => {
                StatementKind::Print(self.substitute(*expression, environment, origin)?)
            }
            StatementKind::Skip => StatementKind::Skip,
            StatementKind::Goto(name) => StatementKind::Goto(name.clone()),
            StatementKind::Call { .. } | StatementKind::Return => {
                return Err(unsupported(origin, "procedure control flow"));
            }
        };
        if placement.is_some() {
            self.charge_kind(&kind, origin)?;
        }
        let id = self.push_statement(Statement {
            span: origin,
            label: statement.label.clone(),
            kind,
        })?;
        if placement.is_some() {
            let call = self
                .expanded
                .calls
                .get(&(origin.start, origin.end))
                .copied()
                .unwrap_or(origin);
            self.expanded
                .statement_origins
                .insert(id.0, (definition, call));
        }
        Ok(vec![id])
    }
}

/// Macro declarations are expanded in order; public source AST stays untouched.
pub(super) fn expand(
    parsed: Parsed,
    options: &TranslationOptions,
) -> Result<Expanded, TranslationError> {
    if parsed.algorithm.macros.is_empty() {
        return Ok(Expanded {
            parsed,
            declared_macros: false,
            expressions: BTreeMap::new(),
            statement_origins: BTreeMap::new(),
            calls: BTreeMap::new(),
        });
    }
    let calls = parsed
        .statements
        .iter()
        .filter_map(|s| match &s.kind {
            StatementKind::MacroCall { name, .. } => Some(((s.span.start, s.span.end), name.span)),
            _ => None,
        })
        .collect();
    let originals = parsed.statements.clone();
    let declarations = parsed.algorithm.macros.clone();
    let mut expander = Expander {
        expanded: Expanded {
            parsed,
            declared_macros: true,
            expressions: BTreeMap::new(),
            statement_origins: BTreeMap::new(),
            calls,
        },
        originals,
        definitions: Vec::new(),
        options,
        tokens: 0,
        bytes: 0,
    };
    if !expander.expanded.algorithm.procedures.is_empty() {
        return Err(unsupported(
            expander.expanded.algorithm.span,
            "macro/procedure interaction",
        ));
    }
    expander.expanded.statements.clear();
    for mut declaration in declarations {
        if expander
            .definitions
            .iter()
            .any(|m| m.name.text == declaration.name.text)
        {
            return Err(failure(
                Code::Syntax,
                declaration.name.span,
                format!(
                    "multiple definitions of macro name {}",
                    declaration.name.text
                ),
            ));
        }
        if declaration
            .parameters
            .iter()
            .enumerate()
            .any(|(i, p)| declaration.parameters[..i].iter().any(|q| p.text == q.text))
        {
            return Err(unsupported(declaration.span, "duplicate formal parameters"));
        }
        if expander.originals.iter().any(|s| {
            s.span.start >= declaration.body.span.start
                && s.span.end <= declaration.body.span.end
                && s.label.is_some()
        }) {
            return Err(failure(
                Code::Label,
                declaration.span,
                "a label may not appear in a macro",
            ));
        }
        declaration.body = expander.block(&declaration.body, &BTreeMap::new(), None, false, 0)?;
        expander.definitions.push(declaration);
    }
    let body = expander.expanded.algorithm.body.clone();
    expander.expanded.algorithm.body = match body {
        Body::Sequential(block) => {
            Body::Sequential(expander.block(&block, &BTreeMap::new(), None, false, 0)?)
        }
        Body::Processes(mut processes) => {
            for process in &mut processes {
                process.body = expander.block(&process.body, &BTreeMap::new(), None, false, 0)?;
            }
            Body::Processes(processes)
        }
    };
    expander.expanded.algorithm.macros.clear();
    compact(&mut expander.expanded)?;
    Ok(expander.expanded)
}

// Prepared definitions are development templates, never executable statements.
// Retain only the expanded algorithm tree before labels/omission decisions.
fn blocks(kind: &StatementKind) -> Vec<&Block> {
    match kind {
        StatementKind::Block(block)
        | StatementKind::While { body: block, .. }
        | StatementKind::With { body: block, .. } => vec![block],
        StatementKind::If {
            branches,
            otherwise,
        } => branches
            .iter()
            .map(|b| &b.body)
            .chain(otherwise.iter())
            .collect(),
        StatementKind::Either(choices) => choices.iter().collect(),
        _ => Vec::new(),
    }
}
fn blocks_mut(kind: &mut StatementKind) -> Vec<&mut Block> {
    match kind {
        StatementKind::Block(block)
        | StatementKind::While { body: block, .. }
        | StatementKind::With { body: block, .. } => vec![block],
        StatementKind::If {
            branches,
            otherwise,
        } => branches
            .iter_mut()
            .map(|b| &mut b.body)
            .chain(otherwise.iter_mut())
            .collect(),
        StatementKind::Either(choices) => choices.iter_mut().collect(),
        _ => Vec::new(),
    }
}
fn compact(expanded: &mut Expanded) -> Result<(), TranslationError> {
    let roots = match &expanded.algorithm.body {
        Body::Sequential(block) => vec![block],
        Body::Processes(processes) => processes.iter().map(|p| &p.body).collect(),
    };
    let mut pending: Vec<_> = roots.iter().flat_map(|b| &b.statements).copied().collect();
    let mut used = vec![false; expanded.statements.len()];
    while let Some(id) = pending.pop() {
        let entry = used.get_mut(id.0).ok_or_else(|| {
            failure(
                Code::Internal,
                expanded.algorithm.span,
                "invalid macro reachable statement",
            )
        })?;
        if *entry {
            continue;
        }
        *entry = true;
        let statement = &expanded.statements[id.0];
        pending.extend(
            blocks(&statement.kind)
                .iter()
                .flat_map(|b| &b.statements)
                .copied(),
        );
    }
    let mut remap = vec![None; used.len()];
    let mut next = 0;
    for (index, present) in used.iter().enumerate() {
        if *present {
            remap[index] = Some(StmtId(next));
            next += 1;
        }
    }
    expanded.statement_origins = expanded
        .statement_origins
        .iter()
        .filter_map(|(id, origin)| {
            remap
                .get(*id)
                .copied()
                .flatten()
                .map(|new| (new.0, *origin))
        })
        .collect();
    let mut index = 0;
    expanded.statements.retain(|_| {
        let keep = used[index];
        index += 1;
        keep
    });
    let rewrite = |block: &mut Block| -> Result<(), TranslationError> {
        for id in &mut block.statements {
            *id = remap.get(id.0).copied().flatten().ok_or_else(|| {
                failure(Code::Internal, block.span, "unreachable macro arena edge")
            })?;
        }
        Ok(())
    };
    for statement in &mut expanded.statements {
        for block in blocks_mut(&mut statement.kind) {
            rewrite(block)?;
        }
    }
    match &mut expanded.algorithm.body {
        Body::Sequential(block) => rewrite(block)?,
        Body::Processes(processes) => {
            for p in processes {
                rewrite(&mut p.body)?;
            }
        }
    }
    Ok(())
}
