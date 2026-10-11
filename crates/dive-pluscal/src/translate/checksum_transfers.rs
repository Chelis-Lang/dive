//! Typed view of shared original-header vectors mutated by pinned Explode.
//! The public parser AST and separate translation IR remain unchanged.
use super::{
    TranslationError, TranslationOptions, TranslationStage, checksum::unsupported, error, macros,
};
use crate::{
    Code,
    ast::{Body, Expr, Procedure, Span, StatementKind, StmtId},
};
use std::collections::BTreeMap;

pub(super) enum Rhs {
    Source(Option<Expr>),
    Frame(String),
    Quoted(String),
    Head(String),
    Tail,
}
pub(super) struct Assignment {
    pub variable: String,
    pub rhs: Rhs,
}
pub(super) enum Node {
    Source(StmtId),
    Assign(Vec<Assignment>),
}
#[derive(Default)]
pub(super) struct Plan {
    pub blocks: BTreeMap<(usize, usize), Vec<Node>>,
}
type Groups = Vec<(String, Vec<StmtId>)>;
struct Builder<'a> {
    parsed: &'a macros::Expanded,
    procedure: &'a Procedure,
    plan: Plan,
    work: usize,
    bytes: usize,
    work_limit: usize,
    byte_limit: usize,
    depth_limit: usize,
    procedure_entry: String,
    transfer_spans: Vec<Span>,
}
impl Builder<'_> {
    fn charge(&mut self, nodes: usize, bytes: usize) -> Result<(), TranslationError> {
        self.work = self.work.saturating_add(nodes);
        self.bytes = self.bytes.saturating_add(bytes);
        if self.work > self.work_limit || self.bytes > self.byte_limit {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                self.parsed.algorithm.span,
                "PlusCal procedure header expansion limit reached",
            ));
        }
        Ok(())
    }
    fn flatten(&mut self, ids: &[StmtId]) -> Result<Vec<StmtId>, TranslationError> {
        self.charge(ids.len(), 0)?;
        let mut pending: Vec<_> = ids.iter().rev().copied().collect();
        let mut flat = Vec::new();
        let mut visited = 0usize;
        while let Some(id) = pending.pop() {
            visited = visited.saturating_add(1);
            if visited > self.parsed.statements.len() {
                return Err(unsupported(
                    self.parsed.algorithm.span,
                    "non-tree statement references",
                ));
            }
            let statement = self.parsed.statement(id).ok_or_else(|| {
                unsupported(self.parsed.algorithm.span, "invalid statement reference")
            })?;
            if let StatementKind::Block(block) = &statement.kind {
                if statement.label.is_some() {
                    return Err(unsupported(statement.span, "labelled C block wrapper"));
                }
                self.charge(block.statements.len(), 0)?;
                pending.extend(block.statements.iter().rev().copied());
            } else {
                self.charge(1, 0)?;
                flat.push(id);
            }
        }
        Ok(flat)
    }
    fn groups(
        &mut self,
        block: &crate::ast::Block,
    ) -> Result<(Vec<StmtId>, Groups), TranslationError> {
        let flat = self.flatten(&block.statements)?;
        self.charge(flat.len(), 0)?;
        let mut unlabelled = Vec::new();
        let mut groups: Vec<(String, Vec<StmtId>)> = Vec::new();
        for id in flat {
            let statement = self
                .parsed
                .statement(id)
                .ok_or_else(|| unsupported(block.span, "invalid statement reference"))?;
            if let Some(label) = &statement.label {
                self.charge(1, label.name.text.len())?;
                groups.push((label.name.text.clone(), Vec::new()));
            }
            if let Some((_, ids)) = groups.last_mut() {
                ids.push(id);
            } else {
                unlabelled.push(id);
            }
        }
        Ok((unlabelled, groups))
    }
    fn update(&mut self, next: &str) -> Result<Node, TranslationError> {
        self.charge(2, next.len().saturating_add(2))?;
        Ok(Node::Assign(vec![Assignment {
            variable: "pc".into(),
            rhs: Rhs::Quoted(next.into()),
        }]))
    }
    fn call(
        &mut self,
        args: &[Expr],
        next: &str,
        generated: bool,
    ) -> Result<Vec<Node>, TranslationError> {
        let p = self.procedure;
        if args.len() != p.parameters.len() {
            return Err(unsupported(p.span, "procedure argument count"));
        }
        // Charge the complete saved frame and every assignment before cloning
        // names, allocating a simultaneous vector or synthesizing an expression.
        let names = p
            .variables
            .iter()
            .chain(&p.parameters)
            .map(|d| d.name.text.len())
            .sum::<usize>();
        self.charge(
            p.parameters
                .len()
                .saturating_add(p.variables.len())
                .saturating_add(4),
            names
                .saturating_mul(3)
                .saturating_add(next.len())
                .saturating_add(p.name.text.len())
                .saturating_add(64),
        )?;
        let mut assignments = Vec::with_capacity(args.len().saturating_add(1));
        assignments.push(Assignment {
            variable: "stack".into(),
            rhs: Rhs::Frame(next.into()),
        });
        for (param, argument) in p.parameters.iter().zip(args) {
            assignments.push(Assignment {
                variable: param.name.text.clone(),
                rhs: Rhs::Source(Some(*argument)),
            });
        }
        // GenAssign sorts only assignments it actually visits. ExplodeWhile
        // mutates shared tail vectors even when IsTRUE discards that tail.
        if generated {
            assignments.sort_by(|a, b| a.variable.cmp(&b.variable));
        }
        let mut nodes = vec![Node::Assign(assignments)];
        for local in &p.variables {
            nodes.push(Node::Assign(vec![Assignment {
                variable: local.name.text.clone(),
                rhs: Rhs::Source(local.initial.as_ref().map(|b| b.expression)),
            }]));
        }
        self.charge(1, self.procedure_entry.len())?;
        let entry = self.procedure_entry.clone();
        nodes.push(self.update(&entry)?);
        Ok(nodes)
    }
    fn returning(&mut self) -> Result<Vec<Node>, TranslationError> {
        let p = self.procedure;
        let names = p
            .variables
            .iter()
            .chain(&p.parameters)
            .map(|d| d.name.text.len())
            .sum::<usize>();
        self.charge(
            p.variables
                .len()
                .saturating_add(p.parameters.len())
                .saturating_add(4),
            names.saturating_mul(2).saturating_add(32),
        )?;
        let mut nodes = vec![Node::Assign(vec![Assignment {
            variable: "pc".into(),
            rhs: Rhs::Head("pc".into()),
        }])];
        for slot in p.variables.iter().chain(&p.parameters) {
            nodes.push(Node::Assign(vec![Assignment {
                variable: slot.name.text.clone(),
                rhs: Rhs::Head(slot.name.text.clone()),
            }]));
        }
        nodes.push(Node::Assign(vec![Assignment {
            variable: "stack".into(),
            rhs: Rhs::Tail,
        }]));
        Ok(nodes)
    }
    #[allow(
        clippy::too_many_lines,
        reason = "ordered pinned CopyAndExplodeLastStmt cases retain their shared-vector decisions together"
    )]
    fn sequence(
        &mut self,
        ids: &[StmtId],
        next: &str,
        shared: bool,
        generated: bool,
        depth: usize,
    ) -> Result<(Vec<Node>, bool), TranslationError> {
        if depth > self.depth_limit {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                self.parsed.algorithm.span,
                "PlusCal procedure header nesting limit reached",
            ));
        }
        self.charge(ids.len(), 0)?;
        let mut nodes: Vec<_> = ids.iter().copied().map(Node::Source).collect();
        let Some(&id) = ids.last() else {
            return Ok((nodes, true));
        };
        let s = self.parsed.statement(id).ok_or_else(|| {
            unsupported(self.parsed.algorithm.span, "invalid statement reference")
        })?;
        let mut needs = true;
        match &s.kind {
            StatementKind::Call { arguments, .. } => {
                needs = false;
                if shared {
                    nodes.pop();
                    nodes.extend(self.call(arguments, next, generated)?);
                }
            }
            StatementKind::Return => {
                needs = false;
                if shared {
                    nodes.pop();
                    nodes.extend(self.returning()?);
                }
            }
            StatementKind::Goto(label) => {
                needs = false;
                if shared {
                    nodes.pop();
                    if let Some(&previous) = ids
                        .get(ids.len().saturating_sub(2))
                        .filter(|_| ids.len() > 1)
                    {
                        if let Some(crate::ast::Statement {
                            kind: StatementKind::Call { arguments, .. },
                            ..
                        }) = self.parsed.statement(previous)
                        {
                            nodes.pop();
                            nodes.extend(self.call(
                                arguments,
                                label.text.trim_matches('"'),
                                generated,
                            )?);
                        } else {
                            nodes.push(self.update(label.text.trim_matches('"'))?);
                        }
                    } else {
                        nodes.push(self.update(label.text.trim_matches('"'))?);
                    }
                }
            }
            StatementKind::If {
                branches,
                otherwise,
            } => {
                if branches.len() != 1 {
                    // The transfer index is reserved and constructed once.
                    // Do not rescan the whole arena for each no-transfer elsif.
                    self.charge(1, 0)?;
                    let index = self
                        .transfer_spans
                        .partition_point(|span| span.start < s.span.start);
                    if self
                        .transfer_spans
                        .get(index)
                        .is_some_and(|span| span.end <= s.span.end)
                    {
                        return Err(unsupported(s.span, "nested transfer with elsif checksum"));
                    }
                    return Ok((nodes, true));
                }
                self.charge(6, 0)?; // two child vectors, flags and plan entries
                let mut outputs = Vec::new();
                let mut flags = Vec::new();
                let then = self.flatten(&branches[0].body.statements)?;
                let other = otherwise
                    .as_ref()
                    .map(|b| self.flatten(&b.statements))
                    .transpose()?
                    .unwrap_or_default();
                for child in [&then, &other] {
                    let (output, flag) = self.sequence(child, next, true, generated, depth + 1)?;
                    outputs.push(output);
                    flags.push(flag);
                }
                needs = flags.iter().all(|b| *b);
                for (index, (mut output, flag)) in outputs.into_iter().zip(flags).enumerate() {
                    if !needs && flag {
                        output.push(self.update(next)?);
                    }
                    self.plan.blocks.insert((id.0, index), output);
                }
            }
            StatementKind::Either(blocks) => {
                self.charge(blocks.len().saturating_mul(3), 0)?;
                let mut outputs = Vec::new();
                let mut flags = Vec::new();
                for block in blocks {
                    let child = self.flatten(&block.statements)?;
                    let (output, flag) = self.sequence(&child, next, true, generated, depth + 1)?;
                    outputs.push(output);
                    flags.push(flag);
                }
                needs = flags.iter().all(|b| *b);
                for (index, (mut output, flag)) in outputs.into_iter().zip(flags).enumerate() {
                    if !needs && flag {
                        output.push(self.update(next)?);
                    }
                    self.plan.blocks.insert((id.0, index), output);
                }
            }
            StatementKind::With { body, .. } => {
                self.charge(1, 0)?;
                let child = self.flatten(&body.statements)?;
                let (output, flag) = self.sequence(&child, next, true, generated, depth + 1)?;
                needs = flag;
                self.plan.blocks.insert((id.0, 0), output);
            }
            _ => {}
        }
        Ok((nodes, needs))
    }
    fn labelled(
        &mut self,
        labelled: &[(String, Vec<StmtId>)],
        continuation: &str,
        generated: bool,
        depth: usize,
    ) -> Result<(), TranslationError> {
        for (index, (label, ids)) in labelled.iter().enumerate() {
            let next = labelled
                .get(index + 1)
                .map_or(continuation, |g| g.0.as_str());
            let first = ids.first().and_then(|id| self.parsed.statement(*id));
            if let Some(crate::ast::Statement {
                kind: StatementKind::While { body, test },
                ..
            }) = first
            {
                let (unlabelled, inner) = self.groups(body)?;
                let inner_next = inner.first().map_or(label.as_str(), |g| g.0.as_str());
                // The outer unlabDo vector is copied. Child control objects
                // are shared, so only their nested vectors become overlays.
                self.sequence(&unlabelled, inner_next, false, generated, depth + 1)?;
                self.labelled(&inner, label, generated, depth + 1)?;
                self.sequence(
                    &ids[1..],
                    next,
                    false,
                    generated && !super::lower::truth(self.parsed, *test, false),
                    depth + 1,
                )?;
            } else {
                self.sequence(ids, next, false, generated, depth + 1)?;
            }
        }
        Ok(())
    }
}
pub(super) fn build(
    parsed: &macros::Expanded,
    options: &TranslationOptions,
) -> Result<Plan, TranslationError> {
    let Some(procedure) = parsed.algorithm.procedures.first() else {
        return Ok(Plan::default());
    };
    let Body::Sequential(main) = &parsed.algorithm.body else {
        return Err(unsupported(
            parsed.algorithm.span,
            "procedure/process checksum ownership",
        ));
    };
    let mut builder = Builder {
        parsed,
        procedure,
        plan: Plan::default(),
        work: parsed.statements.len(),
        bytes: 0,
        work_limit: options.parser.max_nodes.saturating_mul(4).min(200_000),
        byte_limit: options.max_output_bytes.min(4_194_304),
        depth_limit: options.parser.max_depth.min(256),
        procedure_entry: String::new(),
        transfer_spans: Vec::new(),
    };
    // Reserve the complete scan, owned spans and comparison-sort work before
    // allocating the index. Work units are a conservative translation policy,
    // not a count of machine instructions or exact Rust allocator calls.
    let arena = parsed.statements.len();
    builder.charge(arena, 0)?;
    let transfers = parsed
        .statements
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                StatementKind::Call { .. } | StatementKind::Return | StatementKind::Goto(_)
            )
        })
        .count();
    let sort_depth = usize::try_from(transfers.bit_width()).unwrap_or(usize::MAX);
    builder.charge(
        transfers.saturating_mul(sort_depth.saturating_add(1)),
        transfers.saturating_mul(std::mem::size_of::<Span>()),
    )?;
    builder.transfer_spans = parsed
        .statements
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                StatementKind::Call { .. } | StatementKind::Return | StatementKind::Goto(_)
            )
        })
        .map(|s| s.span)
        .collect();
    builder.transfer_spans.sort_by_key(|span| span.start);
    let (_, proc_groups) = builder.groups(&procedure.body)?;
    let entry = &proc_groups
        .first()
        .ok_or_else(|| unsupported(procedure.span, "missing procedure entry"))?
        .0;
    builder.charge(1, entry.len())?;
    builder.procedure_entry.clone_from(entry);
    builder.labelled(&proc_groups, "Error", true, 0)?;
    let (_, main_groups) = builder.groups(main)?;
    builder.labelled(&main_groups, "Done", true, 0)?;
    Ok(builder.plan)
}
