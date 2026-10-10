//! Label/action normalization matching pinned `PcalTranslate`'s continuations.
use super::{
    FairnessOption, TranslationError, TranslationOptions, TranslationStage, TranslationWarning,
    error,
};
use crate::{
    Code, Parsed,
    ast::{
        Assignment, BindingKind, Block, Body, Decl, Expr, Fairness, Label, Span, StatementKind,
        StmtId, WithBinding,
    },
};
use std::collections::BTreeSet;

#[derive(Clone)]
pub(super) struct Node {
    pub span: Span,
    pub kind: Kind,
}
#[derive(Clone)]
pub(super) enum Kind {
    Assign(Vec<Assignment>),
    Await(Expr),
    Assert(Expr),
    Print(Expr),
    Skip,
    Pc(String),
    If {
        test: Expr,
        yes: Vec<Node>,
        no: Vec<Node>,
    },
    Either(Vec<Vec<Node>>),
    With {
        bindings: Vec<WithBinding>,
        body: Vec<Node>,
    },
}
pub(super) struct Action {
    pub name: String,
    pub span: Span,
    pub body: Vec<Node>,
}
pub(super) struct Model {
    pub actions: Vec<Action>,
    pub variables: Vec<Decl>,
    pub use_pc: bool,
    pub stutter: bool,
    pub fair: bool,
    pub warnings: Vec<TranslationWarning>,
    pub process: Option<crate::ast::Process>,
}
struct Group {
    label: Label,
    ids: Vec<StmtId>,
    span: Span,
}
struct Lower<'a> {
    parsed: &'a Parsed,
    options: &'a TranslationOptions,
    actions: Vec<Action>,
    warnings: Vec<TranslationWarning>,
    use_pc: bool,
    nodes: usize,
}

fn failure(code: Code, span: Span, message: impl Into<String>) -> TranslationError {
    error(TranslationStage::Normalization, code, span, message)
}
fn unsupported(span: Span, feature: &str) -> TranslationError {
    failure(
        Code::Unsupported,
        span,
        format!("unsupported PlusCal translation: {feature}"),
    )
}
pub(super) fn tokens(parsed: &Parsed, expr: Expr) -> &[crate::ast::Token] {
    let start = parsed
        .tokens
        .partition_point(|token| token.span.start < expr.span.start);
    let end = parsed
        .tokens
        .partition_point(|token| token.span.end <= expr.span.end);
    parsed.tokens.get(start..end).unwrap_or_default()
}
fn truth(parsed: &Parsed, expr: Expr, strict: bool) -> bool {
    let text: Vec<_> = tokens(parsed, expr)
        .iter()
        .map(|token| token.text.as_str())
        .collect();
    text == ["TRUE"] || (!strict && text == ["(", "TRUE", ")"])
}

#[allow(
    clippy::too_many_lines,
    reason = "the single admission pass checks namespaces before any continuation normalization"
)]
pub(super) fn normalize(
    parsed: &Parsed,
    options: &TranslationOptions,
) -> Result<Model, TranslationError> {
    let algorithm = &parsed.algorithm;
    if let Body::Processes(processes) = &algorithm.body {
        return super::process::normalize(parsed, processes, options);
    }
    let Body::Sequential(block) = &algorithm.body else {
        return Err(unsupported(algorithm.span, "multiprocess normalization"));
    };
    if let Some(procedure) = algorithm.procedures.first() {
        return Err(unsupported(procedure.span, "procedure normalization"));
    }
    if let Some(mac) = algorithm.macros.first() {
        return Err(unsupported(mac.span, "macro expansion"));
    }
    if let Some(definition) = algorithm.definitions {
        return Err(unsupported(definition.span, "define-block name binding"));
    }
    let mut lower = Lower {
        parsed,
        options,
        actions: Vec::new(),
        warnings: Vec::new(),
        use_pc: true,
        nodes: 0,
    };
    if algorithm.fairness == Fairness::Weak
        && matches!(
            options.fairness,
            FairnessOption::None | FairnessOption::Strong
        )
    {
        let option = if options.fairness == FairnessOption::None {
            "nof"
        } else {
            "sf"
        };
        lower.warnings.push(TranslationWarning {
            code: "PCALW_FAIR_OPTION".into(),
            span: algorithm.span,
            message: format!("Option `{option}' specified for --fair algorithm."),
        });
    }
    let ids = lower.flatten(block)?;
    let groups = lower.groups(&ids)?;
    let root = groups
        .first()
        .ok_or_else(|| unsupported(block.span, "empty/automatically labelled algorithm"))?;
    let mut variables = BTreeSet::new();
    for decl in &algorithm.variables {
        if !variables.insert(decl.name.text.as_str()) {
            return Err(failure(
                Code::Syntax,
                decl.span,
                format!("global variable {} redefined", decl.name.text),
            ));
        }
        // PcalSymTab seeds this global before extracting user symbols, even
        // when checkBody later allows omission of generated pc storage.
        if decl.name.text == "pc" {
            return Err(failure(
                Code::Syntax,
                decl.span,
                "global variable pc redefined by the reserved program counter symbol",
            ));
        }
        if decl.name.text == "defaultInitValue"
            && algorithm.variables.iter().any(|var| var.initial.is_none())
        {
            return Err(unsupported(
                decl.span,
                "default initializer name disambiguation",
            ));
        }
        if ["vars", "Init", "Next", "Spec", "Termination", "Terminating"]
            .contains(&decl.name.text.as_str())
        {
            return Err(unsupported(decl.span, "generated-operator naming conflict"));
        }
    }
    let mut labels = BTreeSet::new();
    let mut goto = false;
    let mut binders = BTreeSet::new();
    for statement in &parsed.statements {
        if let Some(label) = &statement.label {
            if !labels.insert(label.name.text.as_str()) {
                return Err(failure(Code::Label, label.span, "duplicate source label"));
            }
            if variables.contains(label.name.text.as_str())
                || [
                    "Done",
                    "pc",
                    "vars",
                    "Init",
                    "Next",
                    "Spec",
                    "Termination",
                    "Terminating",
                ]
                .contains(&label.name.text.as_str())
            {
                return Err(unsupported(
                    label.span,
                    "label/variable/generated-operator name disambiguation",
                ));
            }
        }
        if let StatementKind::With { bindings, .. } = &statement.kind {
            for binding in bindings {
                let name = binding.name.text.as_str();
                if !binders.insert(name)
                    || variables.contains(name)
                    || [
                        "pc",
                        "vars",
                        "Init",
                        "Next",
                        "Spec",
                        "Terminating",
                        "Termination",
                    ]
                    .contains(&name)
                {
                    return Err(unsupported(
                        binding.name.span,
                        "with-binder name disambiguation",
                    ));
                }
            }
        }
        match &statement.kind {
            StatementKind::Goto(_) => goto = true,
            StatementKind::Call { .. } | StatementKind::Return => {
                return Err(unsupported(statement.span, "procedure control flow"));
            }
            StatementKind::MacroCall { .. } => {
                return Err(unsupported(statement.span, "macro invocation"));
            }
            _ => {}
        }
    }
    for statement in &parsed.statements {
        if let StatementKind::Goto(target) = &statement.kind
            && target.text != "Done"
            && target.text != "\"Done\""
            && !labels.contains(target.text.as_str())
        {
            return Err(failure(
                Code::Label,
                target.span,
                "goto target is not a declared label",
            ));
        }
    }
    let infinite = groups.len() == 1
        && root.ids.len() == 1
        && matches!(
            &parsed.statements[root.ids[0].0].kind,
            StatementKind::While { test, .. } if truth(parsed, *test, true)
        );
    let any_modifier = parsed.statements.iter().any(|statement| {
        statement
            .label
            .as_ref()
            .is_some_and(|label| label.modifier.is_some())
    });
    let inner_labels = labels.len() > 1;
    lower.use_pc = !(infinite && !goto && !any_modifier && !inner_labels);
    lower.compile_groups(&groups, "Done")?;
    Ok(Model { actions: lower.actions, variables: algorithm.variables.clone(), use_pc: lower.use_pc,
        stutter: !infinite || parsed.statements.iter().any(|statement| matches!(&statement.kind, StatementKind::Goto(target) if ["Done", "\"Done\""].contains(&target.text.as_str()))),
        fair: algorithm.fairness == Fairness::Weak || matches!(options.fairness, FairnessOption::Weak | FairnessOption::Strong | FairnessOption::Next), warnings: lower.warnings, process: None })
}

impl Lower<'_> {
    fn charge(&mut self, span: Span) -> Result<(), TranslationError> {
        self.nodes += 1;
        if self.nodes > self.options.parser.max_nodes.saturating_mul(4).min(200_000) {
            return Err(failure(
                Code::Resource,
                span,
                "PlusCal normalized-node limit reached",
            ));
        }
        Ok(())
    }
    fn flatten(&self, block: &Block) -> Result<Vec<StmtId>, TranslationError> {
        let mut ids = Vec::new();
        for id in &block.statements {
            let statement = &self.parsed.statements[id.0];
            if let StatementKind::Block(child) = &statement.kind {
                if statement.label.is_some() {
                    return Err(unsupported(
                        statement.span,
                        "labelled C grouping block normalization",
                    ));
                }
                ids.extend(self.flatten(child)?);
            } else {
                ids.push(*id);
            }
        }
        Ok(ids)
    }
    fn groups(&self, ids: &[StmtId]) -> Result<Vec<Group>, TranslationError> {
        let mut groups: Vec<Group> = Vec::new();
        for id in ids {
            let statement = &self.parsed.statements[id.0];
            if let Some(label) = &statement.label {
                groups.push(Group {
                    label: label.clone(),
                    ids: Vec::new(),
                    span: statement.span,
                });
            }
            let group = groups
                .last_mut()
                .ok_or_else(|| unsupported(statement.span, "automatic label insertion"))?;
            group.ids.push(*id);
            group.span.end = statement.span.end;
        }
        Ok(groups)
    }
    fn pc(&mut self, target: &str, span: Span) -> Result<Node, TranslationError> {
        self.charge(span)?;
        Ok(Node {
            span,
            kind: Kind::Pc(target.into()),
        })
    }
    fn compile_groups(&mut self, groups: &[Group], next: &str) -> Result<(), TranslationError> {
        for (index, group) in groups.iter().enumerate() {
            let target = groups
                .get(index + 1)
                .map_or(next, |group| group.label.name.text.as_str());
            self.compile_group(group, target)?;
        }
        Ok(())
    }
    fn compile_group(&mut self, group: &Group, next: &str) -> Result<(), TranslationError> {
        if self.actions.len() >= self.options.max_actions.min(10_000) {
            return Err(failure(
                Code::Resource,
                group.span,
                "PlusCal action limit reached",
            ));
        }
        let position = self.actions.len();
        self.actions.push(Action {
            name: group.label.name.text.clone(),
            span: group.span,
            body: Vec::new(),
        });
        let first = &self.parsed.statements[group.ids[0].0];
        let body = if let StatementKind::While { test, body } = &first.kind {
            let test = *test;
            let inner = self.flatten(body)?;
            let first_label = inner
                .iter()
                .position(|id| self.parsed.statements[id.0].label.is_some())
                .unwrap_or(inner.len());
            let (prefix, labelled) = inner.split_at(first_label);
            let groups = self.groups(labelled)?;
            let target = groups
                .first()
                .map_or(group.label.name.text.as_str(), |group| {
                    group.label.name.text.as_str()
                });
            let (mut yes, needed) = self.sequence(prefix, target, 0)?;
            if needed && self.use_pc {
                yes.push(self.pc(target, first.span)?);
            }
            let (mut no, needed) = self.sequence(&group.ids[1..], next, 0)?;
            if needed && self.use_pc {
                no.push(self.pc(next, group.span)?);
            }
            let normalized = if truth(self.parsed, test, false) {
                if group.ids.len() > 1 {
                    self.warnings.push(TranslationWarning { code: "DIVE-P2000".into(), span: group.span, message: "statements after while TRUE are unreachable and omitted, matching the pinned translator".into() });
                }
                yes
            } else {
                self.charge(first.span)?;
                vec![Node {
                    span: first.span,
                    kind: Kind::If { test, yes, no },
                }]
            };
            self.compile_groups(&groups, &group.label.name.text)?;
            normalized
        } else {
            let (mut body, needed) = self.sequence(&group.ids, next, 0)?;
            if needed && self.use_pc {
                body.push(self.pc(next, group.span)?);
            }
            body
        };
        self.actions[position].body = body;
        Ok(())
    }
    fn unlabelled(&self, block: &Block) -> Result<Vec<StmtId>, TranslationError> {
        let ids = self.flatten(block)?;
        if let Some(statement) = ids
            .iter()
            .map(|id| &self.parsed.statements[id.0])
            .find(|statement| statement.label.is_some())
        {
            return Err(unsupported(
                statement.span,
                "labels inside if/either/with normalization",
            ));
        }
        Ok(ids)
    }
    // The grammar arms must share the last-statement continuation decision;
    // splitting them would obscure the original CopyAndExplodeLastStmt contract.
    #[allow(
        clippy::too_many_lines,
        reason = "one bounded source-kind dispatcher preserves continuation and source order"
    )]
    fn sequence(
        &mut self,
        ids: &[StmtId],
        next: &str,
        depth: usize,
    ) -> Result<(Vec<Node>, bool), TranslationError> {
        if depth > self.options.parser.max_depth.min(256) {
            return Err(failure(
                Code::Resource,
                self.parsed.algorithm.span,
                "PlusCal normalized control-flow depth limit reached",
            ));
        }
        let mut result = Vec::new();
        let mut needs_goto = self.use_pc;
        for (index, id) in ids.iter().enumerate() {
            let statement = &self.parsed.statements[id.0];
            let last = index + 1 == ids.len();
            self.charge(statement.span)?;
            let kind = match &statement.kind {
                StatementKind::Assign(assignments) => Kind::Assign(assignments.clone()),
                StatementKind::Await { condition, .. } => Kind::Await(*condition),
                StatementKind::Assert(expr) => Kind::Assert(*expr),
                StatementKind::Print(expr) => Kind::Print(*expr),
                StatementKind::Skip => Kind::Skip,
                StatementKind::Goto(target) => {
                    if !last {
                        return Err(failure(
                            Code::Label,
                            statement.span,
                            "missing label after goto",
                        ));
                    }
                    needs_goto = false;
                    // GetGoto explicitly treats quoted Done as the terminal
                    // label; it is not a label containing quote characters.
                    Kind::Pc(if target.text == "\"Done\"" {
                        "Done".into()
                    } else {
                        target.text.clone()
                    })
                }
                StatementKind::While { .. } => {
                    return Err(failure(
                        Code::Label,
                        statement.span,
                        "missing label before while",
                    ));
                }
                StatementKind::If {
                    branches,
                    otherwise,
                } => {
                    let other_ids = if let Some(block) = otherwise {
                        self.unlabelled(block)?
                    } else {
                        Vec::new()
                    };
                    let (mut no, mut no_needs) =
                        self.sequence(&other_ids, next, depth.saturating_add(branches.len()))?;
                    for branch in branches.iter().rev() {
                        let branch_ids = self.unlabelled(&branch.body)?;
                        let (mut yes, yes_needs) =
                            self.sequence(&branch_ids, next, depth.saturating_add(branches.len()))?;
                        let both = yes_needs && no_needs;
                        if last && self.use_pc && !both {
                            if yes_needs {
                                yes.push(self.pc(next, branch.body.span)?);
                            }
                            if no_needs {
                                no.push(self.pc(next, statement.span)?);
                            }
                        }
                        no = vec![Node {
                            span: statement.span,
                            kind: Kind::If {
                                test: branch.test,
                                yes,
                                no,
                            },
                        }];
                        no_needs = both;
                    }
                    if last {
                        needs_goto = no_needs;
                    }
                    result.extend(no);
                    continue;
                }
                StatementKind::Either(choices) => {
                    let mut branches = Vec::new();
                    let mut needs = Vec::new();
                    for choice in choices {
                        let choice_ids = self.unlabelled(choice)?;
                        let (body, needed) = self.sequence(&choice_ids, next, depth + 1)?;
                        branches.push(body);
                        needs.push(needed);
                    }
                    let all = needs.iter().all(|needed| *needed);
                    if last && self.use_pc && !all {
                        for (body, needed) in branches.iter_mut().zip(needs) {
                            if needed {
                                body.push(self.pc(next, statement.span)?);
                            }
                        }
                    }
                    if last {
                        needs_goto = all;
                    }
                    Kind::Either(branches)
                }
                StatementKind::With { bindings, body } => {
                    for binding in bindings {
                        if self
                            .parsed
                            .algorithm
                            .variables
                            .iter()
                            .any(|variable| variable.name.text == binding.name.text)
                        {
                            return Err(unsupported(
                                binding.name.span,
                                "with-binder name disambiguation",
                            ));
                        }
                        if binding.binding.kind != BindingKind::Equal
                            && binding.binding.kind != BindingKind::Member
                        {
                            return Err(unsupported(binding.name.span, "unknown with binding"));
                        }
                    }
                    let child_ids = self.unlabelled(body)?;
                    let (body, needed) = self.sequence(&child_ids, next, depth + 1)?;
                    if last {
                        needs_goto = needed;
                    }
                    Kind::With {
                        bindings: bindings.clone(),
                        body,
                    }
                }
                StatementKind::Block(_) => {
                    return Err(unsupported(
                        statement.span,
                        "unexpected unflattened C block",
                    ));
                }
                StatementKind::Call { .. } | StatementKind::Return => {
                    return Err(unsupported(statement.span, "procedure control flow"));
                }
                StatementKind::MacroCall { .. } => {
                    return Err(unsupported(statement.span, "macro invocation"));
                }
            };
            result.push(Node {
                span: statement.span,
                kind,
            });
        }
        Ok((result, needs_goto))
    }
}
