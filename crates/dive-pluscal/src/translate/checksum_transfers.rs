//! Typed view of shared original-header vectors mutated by pinned Explode.
//! The public parser AST and separate translation IR remain unchanged.
use super::{
    TranslationError, TranslationOptions, TranslationStage, checksum::unsupported, error, macros,
};
use crate::{
    Code,
    ast::{Block, Body, Branch, Expr, Span, StatementKind, StmtId},
};
use std::collections::BTreeMap;

pub(super) enum Rhs {
    Source(Option<Expr>),
    Frame { owner: usize, next: String },
    ReplacedFrame { owner: usize },
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
    IfBranch { id: StmtId, index: usize },
    Assign(Vec<Assignment>),
}
#[derive(Default)]
pub(super) struct Plan {
    pub blocks: BTreeMap<(usize, usize), Vec<Node>>,
    /// Source-owned edges, without the LAST declaration's main-header alias.
    pub body_called: Vec<Vec<usize>>,
    pub main_called: Vec<usize>,
}
type Groups = Vec<(String, Vec<StmtId>)>;
struct Builder<'a> {
    parsed: &'a macros::Expanded,
    procedure_owner: Option<usize>,
    plan: Plan,
    work: usize,
    bytes: usize,
    work_limit: usize,
    byte_limit: usize,
    depth_limit: usize,
    procedure_entries: Vec<String>,
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
        name: &str,
        args: &[Expr],
        next: &str,
        generated: bool,
    ) -> Result<Vec<Node>, TranslationError> {
        self.charge(self.parsed.algorithm.procedures.len(), 0)?;
        let owner = self
            .parsed
            .algorithm
            .procedures
            .iter()
            .position(|p| p.name.text == name)
            .ok_or_else(|| unsupported(self.parsed.algorithm.span, "missing call frame owner"))?;
        let p = &self.parsed.algorithm.procedures[owner];
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
            rhs: Rhs::Frame {
                owner,
                next: next.into(),
            },
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
        let entry = self
            .procedure_entries
            .get(owner)
            .ok_or_else(|| unsupported(p.span, "missing procedure entry"))?;
        let bytes = entry.len();
        self.charge(1, bytes)?;
        let entry = self.procedure_entries[owner].clone();
        nodes.push(self.update(&entry)?);
        Ok(nodes)
    }
    fn call_return(
        &mut self,
        name: &str,
        args: &[Expr],
        generated: bool,
    ) -> Result<Vec<Node>, TranslationError> {
        self.charge(self.parsed.algorithm.procedures.len(), 0)?;
        let from = self.procedure_owner.ok_or_else(|| {
            unsupported(
                self.parsed.algorithm.span,
                "call-return without procedure owner",
            )
        })?;
        let to = self
            .parsed
            .algorithm
            .procedures
            .iter()
            .position(|p| p.name.text == name)
            .ok_or_else(|| {
                unsupported(self.parsed.algorithm.span, "missing tail call frame owner")
            })?;
        let caller = &self.parsed.algorithm.procedures[from];
        let target = &self.parsed.algorithm.procedures[to];
        if args.len() != target.parameters.len() {
            return Err(unsupported(target.span, "procedure argument count"));
        }
        // ExplodeCallReturn restores caller locals and replaces the frame only
        // for distinct owners. Reserve all slots/wrappers before owned expansion.
        let slots = caller
            .variables
            .len()
            .saturating_add(target.parameters.len())
            .saturating_add(target.variables.len());
        let names = caller
            .variables
            .iter()
            .chain(&target.parameters)
            .chain(&target.variables)
            .map(|d| d.name.text.len())
            .sum::<usize>();
        let assignments = caller
            .variables
            .len()
            .saturating_add(args.len())
            .saturating_add(1);
        let sorting = if generated {
            assignments
                .saturating_mul(usize::try_from(assignments.bit_width()).unwrap_or(usize::MAX))
        } else {
            0
        };
        self.charge(
            slots
                .saturating_mul(3)
                .saturating_add(8)
                .saturating_add(sorting),
            names.saturating_mul(4).saturating_add(128),
        )?;
        let mut assignments = Vec::with_capacity(assignments);
        if from != to {
            for local in &caller.variables {
                assignments.push(Assignment {
                    variable: local.name.text.clone(),
                    rhs: Rhs::Head(local.name.text.clone()),
                });
            }
            assignments.push(Assignment {
                variable: "stack".into(),
                rhs: Rhs::ReplacedFrame { owner: to },
            });
        }
        for (param, arg) in target.parameters.iter().zip(args) {
            assignments.push(Assignment {
                variable: param.name.text.clone(),
                rhs: Rhs::Source(Some(*arg)),
            });
        }
        if generated {
            // Names are disjoint by admission, so an allocation-free unstable
            // sort has the same total order as pinned GenAssign.
            assignments.sort_unstable_by(|a, b| a.variable.cmp(&b.variable));
        }
        let mut nodes = vec![Node::Assign(assignments)];
        for local in &target.variables {
            nodes.push(Node::Assign(vec![Assignment {
                variable: local.name.text.clone(),
                rhs: Rhs::Source(local.initial.as_ref().map(|b| b.expression)),
            }]));
        }
        let entry = self
            .procedure_entries
            .get(to)
            .ok_or_else(|| unsupported(target.span, "missing tail call entry"))?;
        self.charge(1, entry.len())?;
        let entry = self.procedure_entries[to].clone();
        nodes.push(self.update(&entry)?);
        Ok(nodes)
    }
    fn collect_calls(&mut self) -> Result<(), TranslationError> {
        let parsed = self.parsed;
        // GetProcedure resets a static vector but stores it by reference. Keep
        // body ownership apart from main's later LAST-header alias: the latter
        // can look cyclic even when the actual procedure call graph is a DAG.
        self.charge(
            parsed.algorithm.procedures.len(),
            parsed
                .algorithm
                .procedures
                .len()
                .saturating_mul(std::mem::size_of::<Vec<usize>>()),
        )?;
        self.plan.body_called = (0..parsed.algorithm.procedures.len())
            .map(|_| Vec::new())
            .collect();
        self.charge(parsed.statements.len().saturating_mul(2), 0)?;
        let calls = parsed
            .statements
            .iter()
            .filter(|s| matches!(s.kind, StatementKind::Call { .. }))
            .count();
        let sorting = usize::try_from(calls.bit_width()).unwrap_or(usize::MAX);
        self.charge(
            calls.saturating_mul(sorting.saturating_add(3)),
            calls.saturating_mul(std::mem::size_of::<(Span, &str)>()),
        )?;
        let mut targets: Vec<_> = parsed
            .statements
            .iter()
            .filter_map(|s| {
                if let StatementKind::Call { name, .. } = &s.kind {
                    Some((s.span, name.text.as_str()))
                } else {
                    None
                }
            })
            .collect();
        targets.sort_by_key(|(span, _)| span.start);
        for (span, name) in targets {
            self.record_call(span, name)?;
        }
        Ok(())
    }
    fn record_call(&mut self, span: Span, name: &str) -> Result<(), TranslationError> {
        let procedures = &self.parsed.algorithm.procedures;
        let count = procedures.len();
        let search = usize::try_from(count.bit_width()).unwrap_or(usize::MAX);
        self.charge(count.saturating_add(search).saturating_add(1), 0)?;
        let procedures = &self.parsed.algorithm.procedures;
        let target = procedures
            .iter()
            .position(|p| p.name.text == name)
            .ok_or_else(|| unsupported(span, "missing called-vector target"))?;
        let index = procedures.partition_point(|p| p.body.span.start <= span.start);
        let owner = index
            .checked_sub(1)
            .filter(|&i| span.end <= procedures[i].body.span.end);
        let count = owner.map_or(self.plan.main_called.len(), |i| {
            self.plan.body_called[i].len()
        });
        self.charge(count, 0)?;
        let called = owner.map_or(&self.plan.main_called, |i| &self.plan.body_called[i]);
        let duplicate = called.contains(&target);
        if !duplicate {
            self.charge(1, std::mem::size_of::<usize>())?;
            match owner {
                Some(i) => self.plan.body_called[i].push(target),
                None => self.plan.main_called.push(target),
            }
        }
        Ok(())
    }
    fn check_call_graph(&mut self) -> Result<(), TranslationError> {
        let owners = self.plan.body_called.len();
        self.charge(owners, 0)?;
        let edges = self.plan.body_called.iter().map(Vec::len).sum::<usize>();
        // Reserve adjacency visits, indegrees and the entire bounded work queue
        // before allocation. The finite source graph is not an execution tree.
        // The bounded walk visits only its acyclic prefix; cyclic edges retain
        // their source-owned header entries without following calls or unrolling.
        self.charge(
            owners
                .saturating_mul(3)
                .saturating_add(edges.saturating_mul(2)),
            owners
                .saturating_mul(2)
                .saturating_mul(std::mem::size_of::<usize>()),
        )?;
        let mut incoming = vec![0usize; owners];
        for called in &self.plan.body_called {
            for &target in called {
                incoming[target] = incoming[target].saturating_add(1);
            }
        }
        let mut pending = Vec::with_capacity(owners);
        pending.extend(
            incoming
                .iter()
                .enumerate()
                .filter_map(|(i, &degree)| (degree == 0).then_some(i)),
        );
        let mut visited = 0;
        while let Some(&owner) = pending.get(visited) {
            visited += 1;
            for &target in &self.plan.body_called[owner] {
                incoming[target] = incoming[target].checked_sub(1).ok_or_else(|| {
                    unsupported(
                        self.parsed.algorithm.span,
                        "invalid procedure graph indegree",
                    )
                })?;
                if incoming[target] == 0 {
                    pending.push(target);
                }
            }
        }
        // Writer emits source-owned vectors, then distinct main targets only
        // on LAST. Reserve those comparisons without confusing them with edges.
        let last = self.plan.body_called.last().map_or(0, Vec::len);
        self.charge(
            owners
                .saturating_mul(owners)
                .saturating_add(last.saturating_mul(self.plan.main_called.len())),
            0,
        )?;
        Ok(())
    }
    fn returning(&mut self) -> Result<Vec<Node>, TranslationError> {
        let p = self
            .procedure_owner
            .and_then(|owner| self.parsed.algorithm.procedures.get(owner))
            .ok_or_else(|| unsupported(self.parsed.algorithm.span, "missing return frame owner"))?;
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
    fn conditional(
        &mut self,
        id: StmtId,
        branches: &[Branch],
        otherwise: Option<&Block>,
        next: &str,
        generated: bool,
        depth: usize,
    ) -> Result<bool, TranslationError> {
        // P elsif is a chain of actual reference If objects. Each Then and
        // Else owns a distinct shared vector, including the vector containing
        // the complete remaining chain. Reserve wrappers/flags/entries before
        // allocation; no repeated arena scan or owned branch-list clone.
        if depth.saturating_add(branches.len()) > self.depth_limit {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                self.parsed.algorithm.span,
                "PlusCal procedure header nesting limit reached",
            ));
        }
        self.charge(branches.len().saturating_mul(8), 0)?;
        let other = otherwise
            .map(|b| self.flatten(&b.statements))
            .transpose()?
            .unwrap_or_default();
        let (mut output_else, mut flag_else) =
            self.sequence(&other, next, true, generated, depth + branches.len())?;
        for (index, branch) in branches.iter().enumerate().rev() {
            let then = self.flatten(&branch.body.statements)?;
            let (mut output_then, flag_then) =
                self.sequence(&then, next, true, generated, depth + index + 1)?;
            let needs = flag_then && flag_else;
            if !needs && flag_then {
                output_then.push(self.update(next)?);
            }
            if !needs && flag_else {
                // Append after the entire virtual Else subtree. Appending
                // independently to its source branches changes the header.
                output_else.push(self.update(next)?);
            }
            self.plan.blocks.insert((id.0, index * 2), output_then);
            self.plan.blocks.insert((id.0, index * 2 + 1), output_else);
            output_else = vec![Node::IfBranch { id, index }];
            flag_else = needs;
        }
        Ok(flag_else)
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
            StatementKind::Call { name, arguments } => {
                needs = false;
                if shared {
                    nodes.pop();
                    nodes.extend(self.call(&name.text, arguments, next, generated)?);
                }
            }
            StatementKind::Return => {
                needs = false;
                if shared {
                    nodes.pop();
                    let previous = ids
                        .get(ids.len().saturating_sub(2))
                        .filter(|_| ids.len() > 1)
                        .and_then(|id| self.parsed.statement(*id));
                    if let Some(crate::ast::Statement {
                        kind: StatementKind::Call { name, arguments },
                        ..
                    }) = previous
                    {
                        nodes.pop();
                        nodes.extend(self.call_return(&name.text, arguments, generated)?);
                    } else {
                        nodes.extend(self.returning()?);
                    }
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
                            kind: StatementKind::Call { name, arguments },
                            ..
                        }) = self.parsed.statement(previous)
                        {
                            nodes.pop();
                            nodes.extend(self.call(
                                &name.text,
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
                        needs = self.conditional(
                            id,
                            branches,
                            otherwise.as_ref(),
                            next,
                            generated,
                            depth,
                        )?;
                        return Ok((nodes, needs));
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
    if parsed.algorithm.procedures.is_empty() {
        return Ok(Plan::default());
    }
    let Body::Sequential(main) = &parsed.algorithm.body else {
        return Err(unsupported(
            parsed.algorithm.span,
            "procedure/process checksum ownership",
        ));
    };
    let mut builder = Builder {
        parsed,
        procedure_owner: None,
        plan: Plan::default(),
        work: parsed.statements.len(),
        bytes: 0,
        work_limit: options.parser.max_nodes.saturating_mul(4).min(200_000),
        byte_limit: options.max_output_bytes.min(4_194_304),
        depth_limit: options.parser.max_depth.min(256),
        procedure_entries: Vec::new(),
        transfer_spans: Vec::new(),
    };
    builder.collect_calls()?;
    builder.check_call_graph()?;
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
    builder.charge(
        parsed.algorithm.procedures.len(),
        parsed
            .algorithm
            .procedures
            .len()
            .saturating_mul(std::mem::size_of::<String>()),
    )?;
    // Cache each entry exactly once, before any frame expansion. All owners
    // debit the same work/byte reservation rather than multiplying the quota.
    for procedure in &parsed.algorithm.procedures {
        let (_, proc_groups) = builder.groups(&procedure.body)?;
        let entry = &proc_groups
            .first()
            .ok_or_else(|| unsupported(procedure.span, "missing procedure entry"))?
            .0;
        builder.charge(1, entry.len())?;
        builder.procedure_entries.push(entry.clone());
    }
    for (owner, procedure) in parsed.algorithm.procedures.iter().enumerate() {
        let (_, proc_groups) = builder.groups(&procedure.body)?;
        builder.procedure_owner = Some(owner);
        builder.labelled(&proc_groups, "Error", true, 0)?;
    }
    builder.procedure_owner = None;
    let (_, main_groups) = builder.groups(main)?;
    builder.labelled(&main_groups, "Done", true, 0)?;
    Ok(builder.plan)
}
