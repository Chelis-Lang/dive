//! Theorems, assumptions and proofs: `Generator.processTheorem`,
//! `processAssumption`, `generateAssumeProve`, `generateNewSymb`,
//! `generateProof`, `generateLeafProof`, `generateUseOrHide`, and
//! `UseOrHideNode.factCheck`.
//!
//! Step names follow SANY's parser, which canonicalises step numbers
//! (`correctedStepNum`): `<*>` and `<+>` become the current proof level and
//! leading zeros are dropped from step starts. The proof-level bookkeeping
//! (`proofLevelStack`) is replayed here because the names depend on it.

use super::cst::{has_tok, last_expr, nodes, toks};
use super::{GResult, Gen};
use crate::diagnostics::Code;
use crate::model::{
    AssumeData, AssumeProveData, DeclKind, Kind, Name, NodeId, NodeKind, TheoremData, UseOrHideData,
};
use dive_syntax::{SyntaxElement, SyntaxKind as K, SyntaxNode, SyntaxToken};

/// SANY's `levelOfProofStepLexeme`: -1 for `<*>`, -2 for `<+>`.
fn step_level(text: &str) -> Option<i32> {
    match text.chars().nth(1) {
        Some('*') => Some(-1),
        Some('+') => Some(-2),
        _ => {
            let end = text.find('>')?;
            text.get(1..end)?.parse::<i32>().ok()
        }
    }
}

impl Gen<'_> {
    fn level_at(&self, depth: i32) -> i32 {
        usize::try_from(depth)
            .ok()
            .and_then(|d| self.proof_levels.get(d).copied())
            .unwrap_or(-1)
    }

    /// SANY's `getProofLevel`.
    fn proof_level(&self) -> i32 {
        if self.proof_depth < 0 {
            self.proof_depth
        } else {
            self.level_at(self.proof_depth)
        }
    }

    fn set_level(&mut self, depth: i32, level: i32) {
        if let Ok(d) = usize::try_from(depth) {
            if d >= self.proof_levels.len() {
                self.proof_levels.resize(d + 1, -1);
            }
            self.proof_levels[d] = level;
        }
    }

    fn push_proof_level(&mut self) {
        self.proof_depth += 1;
        self.set_level(self.proof_depth, -1);
    }

    /// SANY's `correctLevel` side effect: the first step sets the level.
    fn correct_level(&mut self, text: &str) {
        let Some(tok) = step_level(text) else { return };
        let last = if self.proof_depth > 0 {
            self.level_at(self.proof_depth - 1)
        } else {
            -1
        };
        if self.proof_level() < 0 {
            let l = if tok < 0 { last + 1 } else { tok };
            self.set_level(self.proof_depth, l);
        }
    }

    /// SANY's `correctedStepNum`.
    pub(crate) fn corrected_step_name(&self, text: &str) -> String {
        let second = text.chars().nth(1);
        if matches!(second, Some('*' | '+')) {
            let mut level = self.proof_level();
            if level < 0 && self.proof_depth > 0 {
                level = self.level_at(self.proof_depth - 1);
            }
            format!("<{level}{}", text.get(2..).unwrap_or(""))
        } else {
            let lvl = step_level(text).unwrap_or(0);
            let rest = text.find('>').and_then(|i| text.get(i..)).unwrap_or("");
            format!("<{lvl}{rest}")
        }
    }

    /// `Generator.processTheorem`.
    pub(crate) fn process_theorem(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<()> {
        let span = self.sp(node);
        let ts = toks(node);
        let named = ts.iter().any(|t| t.kind() == K::DEF_EQ);
        let name_tok = if named {
            ts.iter().find(|t| t.kind() == K::IDENT).cloned()
        } else {
            None
        };
        let children = nodes(node);
        let proof_node = children
            .iter()
            .find(|c| matches!(c.kind(), K::PROOF | K::TERMINAL_PROOF))
            .cloned();
        let stmt = children
            .iter()
            .find(|c| !matches!(c.kind(), K::PROOF | K::TERMINAL_PROOF))
            .cloned();
        let mut tadn = None;
        if let Some(t) = &name_tok {
            self.push_ls();
            let n = self.intern(t.text());
            tadn = Some(self.new_thm_shell(n, span));
        }
        let is_ap = stmt.as_ref().is_some_and(|s| s.kind() == K::ASSUME_PROVE);
        let body = if is_ap {
            self.current_goal = tadn;
            let c = self.new_context();
            self.push_context(c);
            let b = match &stmt {
                Some(s) => self.generate_assume_prove(s, module),
                None => Ok(self.null()),
            };
            self.current_goal = None;
            b?
        } else {
            match &stmt {
                Some(s) => self.generate_expression(s, module)?,
                None => self.null(),
            }
        };
        if let Some(t) = tadn {
            let mut assume_ctx = None;
            if is_ap {
                assume_ctx = Some(self.top());
                self.pop_context();
            }
            self.construct_thm(t, true, Some(body), Some(module));
            let ht = self.pop_label_node_set();
            self.set_labels(t, ht);
            if let Some(md) = self.module_mut(module) {
                md.definitions.push(t);
            }
            if let Some(c) = assume_ctx {
                self.push_context(c);
            }
        }
        let proof = proof_node.as_ref().map(|p| self.generate_proof(p, module));
        if is_ap {
            self.pop_context();
            self.set_in_proof(body, false);
        }
        let proof = proof.transpose()?;
        let thm = self.m.add(
            span,
            NodeKind::Theorem(Box::new(TheoremData {
                body: Some(body),
                proof,
                def: tadn,
                suffices: false,
                checked: 0,
            })),
        );
        if let Some(md) = self.module_mut(module) {
            md.theorems.push(thm);
            md.top_level.push(thm);
        }
        Ok(())
    }

    fn set_in_proof(&mut self, ap: NodeId, v: bool) {
        if let Some(NodeKind::AssumeProve(d)) = self.m.node_mut(ap).map(|n| &mut n.kind) {
            d.in_proof = v;
        }
    }

    /// `Generator.processAssumption`.
    pub(crate) fn process_assumption(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<()> {
        let span = self.sp(node);
        let ts = toks(node);
        let named = ts.iter().any(|t| t.kind() == K::DEF_EQ);
        let is_axiom = ts.first().is_some_and(|t| t.text() == "AXIOM");
        if named {
            self.push_ls();
        }
        let expr = match last_expr(node) {
            Some(e) => self.generate_expression(&e, module),
            None => Ok(self.null()),
        };
        let mut tadn = None;
        if named {
            let ht = self.pop_label_node_set();
            let expr = expr?;
            let name = ts
                .iter()
                .find(|t| t.kind() == K::IDENT)
                .map(|t| t.text().to_owned())
                .unwrap_or_default();
            let n = self.intern(&name);
            let t = self.new_thmdef(
                n,
                false,
                Some(expr),
                Some(module),
                true,
                span,
                Vec::new(),
                None,
                None,
            );
            self.set_labels(t, ht);
            if let Some(md) = self.module_mut(module) {
                md.definitions.push(t);
            }
            tadn = Some(t);
            self.add_assume(span, expr, tadn, is_axiom, module);
        } else {
            let expr = expr?;
            self.add_assume(span, expr, tadn, is_axiom, module);
        }
        Ok(())
    }

    fn add_assume(
        &mut self,
        span: Option<crate::diagnostics::Span>,
        expr: NodeId,
        def: Option<NodeId>,
        is_axiom: bool,
        module: NodeId,
    ) {
        let a = self.m.add(
            span,
            NodeKind::Assume(AssumeData {
                expr: Some(expr),
                def,
                is_axiom,
                checked: 0,
            }),
        );
        if let Some(md) = self.module_mut(module) {
            md.assumptions.push(a);
            md.top_level.push(a);
        }
    }

    /// `Generator.generateAssumeProve`.
    pub(crate) fn generate_assume_prove(
        &mut self,
        node: &SyntaxNode,
        module: NodeId,
    ) -> GResult<NodeId> {
        let span = self.sp(node);
        let goal = if self.ap_depth == 0 {
            self.current_goal
        } else {
            None
        };
        self.ap_depth += 1;
        let depth = self.ap_depth;
        let ts = toks(node);
        let is_box = ts.first().is_some_and(|t| t.kind() == K::BOX_ASSUME_KW);
        let prove_box = ts
            .iter()
            .find(|t| matches!(t.kind(), K::PROVE_KW | K::BOX_PROVE_KW))
            .is_some_and(|t| t.kind() == K::BOX_PROVE_KW);
        let first_span = ts.first().and_then(|t| self.spt(t));
        if is_box && !prove_box {
            self.err(
                Code::BoxAssumeMismatch,
                first_span,
                "[]ASSUME is matched by PROVE instead of []PROVE",
            );
        }
        if !is_box && prove_box {
            self.err(
                Code::BoxAssumeMismatch,
                first_span,
                "ASSUME is matched by []PROVE instead of PROVE",
            );
        }
        // Items before PROVE, and the goal after it.
        let mut items = Vec::new();
        let mut goal_node = None;
        let mut seen_prove = false;
        for e in node.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) if matches!(t.kind(), K::PROVE_KW | K::BOX_PROVE_KW) => {
                    seen_prove = true;
                }
                SyntaxElement::Node(n) if !seen_prove => items.push(n),
                SyntaxElement::Node(n) => goal_node = Some(n),
                SyntaxElement::Token(_) => {}
            }
        }
        if depth >= self.in_scope_of_ap_decl.len() {
            self.in_scope_of_ap_decl.resize(depth + 1, false);
        }
        self.in_scope_of_ap_decl[depth] = false;
        if depth != 1 {
            let c = self.new_context();
            self.push_context(c);
        }
        let marker_name = self.intern("$$InAssume");
        if is_box {
            if self.resolve_symbol(marker_name).is_some() {
                self.err(
                    Code::BoxAssumeInAssume,
                    first_span,
                    "[]ASSUME is used within the scope of an ordinary ASSUME's assumptions",
                );
            }
        } else if self.resolve_symbol(marker_name).is_none() {
            let marker = self.sh.in_assume;
            self.add_symbol(marker_name, marker);
        }
        if depth == 1 {
            self.current_goal_clause = 0;
        }
        let mut assumes = Vec::new();
        let mut in_scope = vec![false];
        let r: GResult<()> = (|| {
            for item in &items {
                let prev = in_scope.last().copied().unwrap_or(false);
                in_scope.push(prev);
                let a = match item.kind() {
                    K::ASSUME_PROVE => self.generate_assume_prove(item, module)?,
                    K::NEW_DECL => {
                        let n = self.generate_new_symb(item, module)?;
                        if let Some(last) = in_scope.last_mut() {
                            *last = true;
                        }
                        self.in_scope_of_ap_decl[depth] = true;
                        n
                    }
                    _ => self.generate_expression_or_lap(item, module, true)?,
                };
                assumes.push(Some(a));
                if depth == 1 {
                    self.current_goal_clause += 1;
                }
            }
            Ok(())
        })();
        let prove = match (&r, &goal_node) {
            (Ok(()), Some(g)) => Some(self.generate_expression(g, module)),
            _ => None,
        };
        if depth != 1 {
            self.pop_context();
        }
        self.ap_depth -= 1;
        r?;
        let prove = prove.transpose()?;
        Ok(self.m.add(
            span,
            NodeKind::AssumeProve(Box::new(AssumeProveData {
                assumes,
                prove,
                in_scope_of_decl: in_scope,
                is_box,
                suffices: false,
                goal,
                in_proof: true,
            })),
        ))
    }

    /// `Generator.generateNewSymb`.
    fn generate_new_symb(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let set = if has_tok(node, K::IN_OP) {
            match last_expr(node) {
                Some(e) => Some(self.generate_expression(&e, module)?),
                None => None,
            }
        } else {
            None
        };
        let kw = toks(node).into_iter().map(|t| t.kind()).find(|k| {
            matches!(
                k,
                K::CONSTANT_KW | K::VARIABLE_KW | K::STATE_KW | K::ACTION_KW | K::TEMPORAL_KW
            )
        });
        let (decl, level) = match kw {
            Some(K::VARIABLE_KW) => (DeclKind::NewVariable, 1),
            Some(K::STATE_KW) => (DeclKind::NewState, 1),
            Some(K::ACTION_KW) => (DeclKind::NewAction, 2),
            Some(K::TEMPORAL_KW) => (DeclKind::NewTemporal, 3),
            _ => (DeclKind::NewConstant, 0),
        };
        let item = nodes(node).into_iter().find(|n| {
            matches!(
                n.kind(),
                K::IDENT_DECL | K::PREFIX_DECL | K::INFIX_DECL | K::POSTFIX_DECL
            )
        });
        let d = match item {
            Some(i) => self.build_parameter(&i, decl, level, module, true),
            None => self.null(),
        };
        Ok(self.m.add(span, NodeKind::NewSymb { decl: d, set }))
    }

    /// `Generator.generateProof`.
    pub(crate) fn generate_proof(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        self.push_proof_level();
        let r = if node.kind() == K::TERMINAL_PROOF {
            self.generate_leaf_proof(node, module)
        } else {
            self.generate_non_leaf_proof(node, module)
        };
        self.proof_depth -= 1;
        r
    }

    #[allow(
        clippy::too_many_lines,
        reason = "a direct port of SANY's generateProof"
    )]
    fn generate_non_leaf_proof(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let pf_ctx = self.new_context();
        self.push_context(pf_ctx);
        let mut number_of_pops = 0usize;
        let mut steps = Vec::new();
        let mut insts = Vec::new();
        let mut prev_is_infix = false;
        let mut prev_rhs: Option<NodeId> = None;
        let result: GResult<()> = (|| {
            for step in nodes(node)
                .into_iter()
                .filter(|n| n.kind() == K::PROOF_STEP)
            {
                let step_span = self.sp(&step);
                let Some(num_tok) = toks(&step).into_iter().next() else {
                    continue;
                };
                let parts = nodes(&step);
                let body = parts
                    .iter()
                    .find(|n| !matches!(n.kind(), K::PROOF | K::TERMINAL_PROOF))
                    .cloned();
                let step_proof = parts
                    .iter()
                    .find(|n| matches!(n.kind(), K::PROOF | K::TERMINAL_PROOF))
                    .cloned();
                self.correct_level(num_tok.text());
                let text = num_tok.text().to_owned();
                if matches!(num_tok.kind(), K::STEP_IMPLICIT | K::STEP_DOT)
                    && matches!(text.chars().nth(1), Some('*' | '+'))
                {
                    self.err_help(
                        Code::NamedImplicitStep,
                        self.spt(&num_tok),
                        "`<*>` and `<+>` cannot be used for a named step",
                        "write the step's level as a number, e.g. `<2>3.`",
                    );
                }
                let step_name = self.step_name(&num_tok);
                let mut make_num_node = step_name.is_some();
                if step_name.is_some() {
                    self.push_ls();
                }
                let tadn = step_name.map(|n| self.new_thm_shell(n, self.spt(&num_tok)));
                let Some(body) = body else { continue };
                if body.kind() != K::ASSERT_STEP {
                    prev_is_infix = false;
                }
                let mut num_node = None;
                let mut is_pick = false;
                let mut pick_ctx = None;
                match body.kind() {
                    K::DEF_STEP => {
                        let mut defs = Vec::new();
                        for d in nodes(&body) {
                            let mut vec = Vec::new();
                            match d.kind() {
                                K::FUNCTION_DEF => {
                                    self.process_function(&d, Some(&mut vec), module)?;
                                }
                                K::MODULE_DEF => {
                                    let mut dv = Vec::new();
                                    let md = self.process_module_definition(
                                        &d,
                                        Some(&mut dv),
                                        Some(&mut insts),
                                        module,
                                    )?;
                                    vec.push(md);
                                }
                                K::OPERATOR_DEF => {
                                    let o = self.process_operator(&d, Some(&mut vec), module)?;
                                    if let Some(m) = self.module_mut(module) {
                                        m.definitions.push(o);
                                    }
                                }
                                _ => {}
                            }
                            if let Some(&f) = vec.first() {
                                defs.push(f);
                            }
                        }
                        let n = self.m.add(
                            self.sp(&body),
                            NodeKind::DefStep {
                                step: step_name,
                                defs,
                            },
                        );
                        num_node = Some(n);
                        steps.push(n);
                    }
                    K::USE_OR_HIDE => {
                        let u = self.generate_use_or_hide(&body, module)?;
                        if let Some(n) = self.m.node_mut(u) {
                            n.span = step_span;
                        }
                        if let Some(NodeKind::UseOrHide(d)) =
                            self.m.node_mut(u).map(|n| &mut n.kind)
                        {
                            d.step = step_name;
                        }
                        if self.use_is_empty(u) {
                            self.err(Code::EmptyUse, self.sp(&body), "empty USE or HIDE");
                        }
                        self.fact_check(u);
                        num_node = Some(u);
                        steps.push(u);
                    }
                    K::INSTANTIATION => {
                        let i = self.generate_instance(&body, module, false)?;
                        if let Some(NodeKind::Instance(d)) = self.m.node_mut(i).map(|n| &mut n.kind)
                        {
                            d.step = step_name;
                        }
                        num_node = Some(i);
                        steps.push(i);
                    }
                    _ => {
                        make_num_node = false;
                        let mut is_ap = false;
                        let mut is_suffices = false;
                        let mut assume_ctx = None;
                        let body_node: NodeId = match body.kind() {
                            K::ASSERT_STEP => {
                                is_suffices = has_tok(&body, K::SUFFICES_KW);
                                let inner = nodes(&body).into_iter().next();
                                match inner {
                                    Some(i) if i.kind() == K::ASSUME_PROVE => {
                                        is_ap = true;
                                        let c = self.new_context();
                                        self.push_context(c);
                                        self.current_goal = tadn;
                                        let r = self.generate_assume_prove(&i, module);
                                        self.current_goal = None;
                                        assume_ctx = Some(self.top());
                                        self.pop_context();
                                        let b = r?;
                                        if is_suffices
                                            && let Some(NodeKind::AssumeProve(d)) =
                                                self.m.node_mut(b).map(|n| &mut n.kind)
                                        {
                                            d.suffices = true;
                                        }
                                        prev_is_infix = false;
                                        b
                                    }
                                    Some(e) => {
                                        if is_suffices {
                                            let a = self.generate_expression(&e, module)?;
                                            self.appl_builtin(
                                                "$Suffices",
                                                vec![Some(a)],
                                                self.sp(&body),
                                            )
                                        } else {
                                            let lhs_is_at = e.kind() == K::INFIX_EXPR
                                                && super::cst::first_expr(&e).is_some_and(|l| {
                                                    l.kind() == K::GENERAL_ID
                                                        && l.children()
                                                            .all(|c| c.kind() != K::ID_PREFIX)
                                                        && toks(&l).last().is_some_and(|t| {
                                                            t.kind() == K::IDENT && t.text() == "@"
                                                        })
                                                });
                                            let b = if prev_is_infix && lhs_is_at {
                                                self.at_step(&e, prev_rhs, module)?
                                            } else {
                                                self.generate_expression(&e, module)?
                                            };
                                            prev_is_infix = false;
                                            if e.kind() == K::INFIX_EXPR
                                                && let Some(d) = self.appl_data(b)
                                                && d.operands.len() > 1
                                            {
                                                prev_is_infix = true;
                                                prev_rhs = d.operands[1];
                                            }
                                            b
                                        }
                                    }
                                    None => self.null(),
                                }
                            }
                            K::HAVE_STEP | K::CASE_STEP => {
                                let op = if body.kind() == K::HAVE_STEP {
                                    "$Have"
                                } else {
                                    "$Pfcase"
                                };
                                let a = match last_expr(&body) {
                                    Some(e) => self.generate_expression(&e, module)?,
                                    None => self.null(),
                                };
                                self.appl_builtin(op, vec![Some(a)], self.sp(&body))
                            }
                            K::TAKE_STEP | K::PICK_STEP => {
                                let pick = body.kind() == K::PICK_STEP;
                                let op = if pick { "$Pick" } else { "$Take" };
                                if pick {
                                    is_pick = true;
                                    let c = self.new_context();
                                    self.push_context(c);
                                }
                                let bounds: Vec<SyntaxNode> = nodes(&body)
                                    .into_iter()
                                    .filter(|n| n.kind() == K::QUANT_BOUND)
                                    .collect();
                                let bs = self.sp(&body);
                                if bounds.is_empty() {
                                    let mut params = Vec::new();
                                    for id in
                                        toks(&body).into_iter().filter(|t| t.kind() == K::IDENT)
                                    {
                                        let n = self.intern(id.text());
                                        params.push(self.new_param(
                                            Some(n),
                                            0,
                                            self.spt(&id),
                                            true,
                                            Some(module),
                                        ));
                                    }
                                    let args = if pick {
                                        pick_ctx = Some(self.top());
                                        self.push_formal_params(params.clone());
                                        let a = match last_expr(&body) {
                                            Some(e) => self.generate_expression(&e, module),
                                            None => Ok(self.null()),
                                        };
                                        self.pop_formal_params();
                                        self.pop_context();
                                        vec![Some(a?)]
                                    } else {
                                        Vec::new()
                                    };
                                    self.appl_unbounded(op, args, params, bs)
                                } else {
                                    let (params, tuples, ranges) =
                                        self.process_quant_bounds(&bounds, module)?;
                                    let args = if pick {
                                        pick_ctx = Some(self.top());
                                        self.push_formal_params(
                                            params.iter().flatten().copied().collect(),
                                        );
                                        let a = match last_expr(&body) {
                                            Some(e)
                                                if e.kind() != K::QUANT_BOUND
                                                    && !bounds.iter().any(|b| b == &e) =>
                                            {
                                                self.generate_expression(&e, module)
                                            }
                                            _ => Ok(self.null()),
                                        };
                                        self.pop_formal_params();
                                        self.pop_context();
                                        vec![Some(a?)]
                                    } else {
                                        Vec::new()
                                    };
                                    self.appl_bounded(op, None, args, params, tuples, ranges, bs)
                                }
                            }
                            K::WITNESS_STEP => {
                                let mut args = Vec::new();
                                for e in super::cst::exprs(&body) {
                                    args.push(Some(self.generate_expression(&e, module)?));
                                }
                                self.appl_builtin("$Witness", args, self.sp(&body))
                            }
                            K::QED_STEP => self.appl_builtin("$Qed", Vec::new(), self.sp(&body)),
                            _ => {
                                return Err(self.abort(
                                    Code::SanyInternal,
                                    self.sp(&body),
                                    format!("unexpected proof step {}", body.kind().name()),
                                ));
                            }
                        };
                        if let Some(t) = tadn {
                            self.construct_thm(t, true, Some(body_node), Some(module));
                            let ht = self.pop_label_node_set();
                            self.set_labels(t, ht);
                            if is_suffices && let Some(d) = self.thmdef_mut(t) {
                                d.suffices = true;
                            }
                        }
                        let mut proof = None;
                        if let Some(p) = &step_proof {
                            if is_ap
                                && !is_suffices
                                && let Some(c) = assume_ctx
                            {
                                self.push_context(c);
                            }
                            let r = self.generate_proof(p, module);
                            if is_ap && !is_suffices && assume_ctx.is_some() {
                                self.pop_context();
                            }
                            proof = Some(r?);
                        }
                        if is_ap
                            && is_suffices
                            && let Some(c) = assume_ctx
                        {
                            number_of_pops += 1;
                            self.push_context(c);
                        }
                        if is_ap {
                            self.set_in_proof(body_node, false);
                        }
                        let thm = self.m.add(
                            step_span,
                            NodeKind::Theorem(Box::new(TheoremData {
                                body: Some(body_node),
                                proof,
                                def: tadn,
                                suffices: is_suffices,
                                checked: 0,
                            })),
                        );
                        steps.push(thm);
                    }
                }
                if make_num_node && let (Some(name), Some(n)) = (step_name, num_node) {
                    let d = self.new_step_def(name, n, Some(module), step_span);
                    let ht = self.pop_label_node_set();
                    self.set_labels(d, ht);
                }
                if is_pick && let Some(c) = pick_ctx {
                    let entries: Vec<_> = self.m.context(c).entries().collect();
                    for (k, v) in entries {
                        if let crate::model::Key::Sym(n) = k {
                            self.add_symbol(n, v);
                        }
                    }
                }
            }
            Ok(())
        })();
        for _ in 0..number_of_pops {
            let top = self.top();
            let entries: Vec<_> = self.m.context(top).entries().collect();
            for (k, v) in entries {
                self.ctx_put(pf_ctx, k, v);
            }
            self.pop_context();
        }
        self.pop_context();
        result?;
        Ok(self.m.add(
            self.sp(node),
            NodeKind::NonLeafProof {
                steps,
                insts,
                ctx: pf_ctx,
            },
        ))
    }

    /// The `@`-step of an assertion: `@ op e` continues the previous step's
    /// right-hand side.
    fn at_step(
        &mut self,
        e: &SyntaxNode,
        prev_rhs: Option<NodeId>,
        module: NodeId,
    ) -> GResult<NodeId> {
        let span = self.sp(e);
        let op = toks(e).into_iter().find(|t| t.kind().is_operator());
        let text = op.map(|t| t.text().to_owned()).unwrap_or_default();
        let name = crate::builtins::resolve_synonym(&text).to_owned();
        let Some(opn) = self.resolve_symbol_str(&name) else {
            self.err(
                Code::UndefinedOperatorSymbol,
                span,
                format!("operator `{text}` is not defined"),
            );
            return Ok(self.null());
        };
        let rhs = match super::cst::exprs(e).get(1) {
            Some(r) => self.generate_expression(r, module)?,
            None => self.null(),
        };
        let lhs_span = super::cst::first_expr(e).and_then(|l| self.sp(&l));
        let nop = self.appl_builtin("$Nop", vec![prev_rhs], lhs_span);
        self.appl_match(opn, vec![Some(nop), Some(rhs)], span)
    }

    /// The canonical name of a numbered step, or `None` for an unnumbered one.
    fn step_name(&mut self, t: &SyntaxToken) -> Option<Name> {
        match t.kind() {
            K::STEP_NUMBER | K::STEP_IMPLICIT => {
                let c = self.corrected_step_name(t.text());
                Some(self.intern(&c))
            }
            K::STEP_DOT => {
                let c = self.corrected_step_name(t.text());
                let cut = c.find('.').map_or(c.as_str(), |i| &c[..i]).to_owned();
                Some(self.intern(&cut))
            }
            _ => None,
        }
    }

    /// `Generator.generateLeafProof`.
    fn generate_leaf_proof(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let ts = toks(node);
        if ts.iter().any(|t| t.kind() == K::BY_KW) {
            let u = self.generate_use_or_hide(node, module)?;
            let (facts, defs, only) = match &self.m.node(u).kind {
                NodeKind::UseOrHide(d) => (d.facts.clone(), d.defs.clone(), d.only),
                _ => (Vec::new(), Vec::new(), false),
            };
            if facts.is_empty() && defs.is_empty() {
                self.err(Code::EmptyUse, span, "empty BY");
            }
            return Ok(self.m.add(
                span,
                NodeKind::LeafProof {
                    facts,
                    defs,
                    omitted: false,
                    only,
                },
            ));
        }
        let omitted = ts.iter().any(|t| t.kind() == K::OMITTED_KW);
        Ok(self.m.add(
            span,
            NodeKind::LeafProof {
                facts: Vec::new(),
                defs: Vec::new(),
                omitted,
                only: false,
            },
        ))
    }

    /// `Generator.generateUseOrHide` (also used for `BY`).
    pub(crate) fn generate_use_or_hide(
        &mut self,
        node: &SyntaxNode,
        module: NodeId,
    ) -> GResult<NodeId> {
        let span = self.sp(node);
        let hide = has_tok(node, K::HIDE_KW);
        let only = has_tok(node, K::ONLY_KW);
        let mut facts = Vec::new();
        let mut defs = Vec::new();
        let mut in_defs = false;
        let module_name = self.module_data(module).map(|d| d.name);
        for e in node.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) if t.kind() == K::DEF_KW => in_defs = true,
                SyntaxElement::Node(n) => {
                    if n.kind() == K::MODULE_REF {
                        let id = toks(&n).into_iter().find(|t| t.kind() == K::IDENT);
                        let text = id.as_ref().map(|t| t.text().to_owned()).unwrap_or_default();
                        let nm = self.intern(&text);
                        let mut mn = self.resolve_module(nm);
                        if mn.is_none() && Some(nm) == module_name {
                            mn = Some(module);
                        }
                        match mn {
                            Some(m) => {
                                if in_defs {
                                    defs.push(m);
                                } else {
                                    facts.push(m);
                                }
                            }
                            None => self.err(
                                Code::ModuleNotImported,
                                id.as_ref().and_then(|t| self.spt(t)),
                                format!(
                                    "module `{text}` is used without being extended or instantiated"
                                ),
                            ),
                        }
                    } else if in_defs {
                        if n.kind() == K::GENERAL_ID {
                            let sel = self.gen_id_to_selector(&n, &n, None)?;
                            let d = self.selector_to_node(&sel, -1, false, true, module)?;
                            let ok = match &self.m.node(d).kind {
                                NodeKind::OpDef(od) => od.kind != crate::model::DefKind::Null,
                                NodeKind::ThmOrAssumpDef(td) => {
                                    !self.m.name(td.name).starts_with('<')
                                }
                                _ => false,
                            };
                            if ok {
                                defs.push(d);
                            } else {
                                self.err(
                                    Code::DefNotDefinition,
                                    self.sp(&n),
                                    "a DEF item must name a defined operator",
                                );
                            }
                        } else {
                            self.err(
                                Code::BadUseItem,
                                self.sp(&n),
                                "a DEF item must be the name of a definition",
                            );
                        }
                    } else if n.kind() == K::GENERAL_ID {
                        let sel = self.gen_id_to_selector(&n, &n, None)?;
                        facts.push(self.selector_to_node(&sel, 0, true, false, module)?);
                    } else if n.kind() == K::ASSUME_PROVE {
                        facts.push(self.generate_assume_prove(&n, module)?);
                    } else {
                        facts.push(self.generate_expression(&n, module)?);
                    }
                }
                SyntaxElement::Token(_) => {}
            }
        }
        Ok(self.m.add(
            span,
            NodeKind::UseOrHide(Box::new(UseOrHideData {
                hide,
                facts,
                defs,
                only,
                step: None,
            })),
        ))
    }

    pub(crate) fn use_is_empty(&self, u: NodeId) -> bool {
        matches!(&self.m.node(u).kind, NodeKind::UseOrHide(d) if d.facts.is_empty() && d.defs.is_empty())
    }

    /// `UseOrHideNode.factCheck`.
    pub(crate) fn fact_check(&mut self, u: NodeId) {
        let (hide, facts) = match &self.m.node(u).kind {
            NodeKind::UseOrHide(d) => (d.hide, d.facts.clone()),
            _ => return,
        };
        if !hide {
            return;
        }
        for f in facts {
            if let Some(d) = self.appl_data(f)
                && self.kind(d.operator) != Kind::ThmOrAssumpDef
            {
                let at = self.m.node(f).span;
                self.err_help(
                    Code::HideFactInvalid,
                    at,
                    "the only expression allowed as a fact in a HIDE is the name of a theorem, assumption or step",
                    "hide definitions with `HIDE DEF`",
                );
            }
        }
    }
}
