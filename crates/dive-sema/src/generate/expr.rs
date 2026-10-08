//! Expressions: `Generator.generateExpression` and the methods it calls.

use super::cst::{elems, exprs, first_expr, has_tok, last_expr, nodes, toks};
use super::{GResult, Gen};
use crate::diagnostics::Code;
use crate::model::{DefKind, Kind, LabelData, NodeId, NodeKind, OpArgData};
use dive_syntax::ast::AstNode as _;
use dive_syntax::{SyntaxElement, SyntaxKind as K, SyntaxNode};
use std::collections::BTreeSet;

impl Gen<'_> {
    /// `Generator.generateExpression`.
    pub(crate) fn generate_expression(
        &mut self,
        node: &SyntaxNode,
        module: NodeId,
    ) -> GResult<NodeId> {
        self.generate_expression_or_lap(node, module, false)
    }

    /// `Generator.generateExpressionOrLAP`: may return a label naming an
    /// `ASSUME`/`PROVE` when `allow_lap`.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per expression form, as in SANY"
    )]
    pub(crate) fn generate_expression_or_lap(
        &mut self,
        node: &SyntaxNode,
        module: NodeId,
        allow_lap: bool,
    ) -> GResult<NodeId> {
        let span = self.sp(node);
        match node.kind() {
            K::DECIMAL_LIT => {
                let ts = toks(node);
                let a = ts.first().map(|t| t.text().to_owned()).unwrap_or_default();
                let b = ts.get(2).map(|t| t.text().to_owned()).unwrap_or_default();
                Ok(self.m.add(span, NodeKind::Decimal(a, b)))
            }
            K::NUMBER_LIT => {
                let t = toks(node)
                    .first()
                    .map(|t| t.text().to_owned())
                    .unwrap_or_default();
                Ok(self.m.add(span, NodeKind::Numeral(t)))
            }
            K::STRING_LIT => {
                let t = toks(node)
                    .first()
                    .map(|t| t.text().to_owned())
                    .unwrap_or_default();
                Ok(self.m.add(span, NodeKind::Str(unescape(&t))))
            }
            K::PAREN_EXPR => match first_expr(node) {
                Some(e) => self.generate_expression(&e, module),
                None => Ok(self.null()),
            },
            K::INFIX_EXPR => {
                let es = exprs(node);
                let op = toks(node).into_iter().find(|t| t.kind().is_operator());
                let text = op.as_ref().map(|t| t.text().to_owned()).unwrap_or_default();
                let Some(opn) = self.resolve_operator_symbol(&text, false, span) else {
                    return Ok(self.null());
                };
                let lhs = match es.first() {
                    Some(e) => self.generate_expression(e, module)?,
                    None => self.null(),
                };
                let rhs = match es.get(1) {
                    Some(e) => self.generate_expression(e, module)?,
                    None => self.null(),
                };
                self.appl_match(opn, vec![Some(lhs), Some(rhs)], span)
            }
            K::PREFIX_EXPR => {
                let op = toks(node).into_iter().find(|t| t.kind().is_operator());
                let text = op.as_ref().map(|t| t.text().to_owned()).unwrap_or_default();
                let Some(opn) = self.resolve_operator_symbol(&text, true, span) else {
                    return Ok(self.null());
                };
                let arg = match first_expr(node) {
                    Some(e) => self.generate_expression(&e, module)?,
                    None => self.null(),
                };
                self.appl_match(opn, vec![Some(arg)], span)
            }
            K::POSTFIX_EXPR => {
                let op = toks(node).into_iter().find(|t| t.kind().is_operator());
                let text = op.as_ref().map(|t| t.text().to_owned()).unwrap_or_default();
                let Some(opn) = self.resolve_operator_symbol(&text, false, span) else {
                    return Ok(self.null());
                };
                let arg = match first_expr(node) {
                    Some(e) => self.generate_expression(&e, module)?,
                    None => self.null(),
                };
                self.appl_match(opn, vec![Some(arg)], span)
            }
            K::CARTESIAN_PRODUCT => {
                let mut ops = Vec::new();
                for e in exprs(node) {
                    ops.push(Some(self.generate_expression(&e, module)?));
                }
                Ok(self.appl_builtin("$CartesianProd", ops, span))
            }
            K::SET_ENUM => {
                let mut ops = Vec::new();
                for e in exprs(node) {
                    ops.push(Some(self.generate_expression(&e, module)?));
                }
                Ok(self.appl_builtin("$SetEnumerate", ops, span))
            }
            K::TUPLE => {
                let mut ops = Vec::new();
                for e in exprs(node) {
                    ops.push(Some(self.generate_expression(&e, module)?));
                }
                Ok(self.appl_builtin("$Tuple", ops, span))
            }
            K::GENERAL_ID => {
                let is_at = node.children().all(|c| c.kind() != K::ID_PREFIX)
                    && toks(node)
                        .last()
                        .is_some_and(|t| t.kind() == K::IDENT && t.text() == "@");
                if is_at {
                    let (Some(&exc), Some(&spec)) =
                        (self.exc_stack.last(), self.exc_spec_stack.last())
                    else {
                        self.err_help(
                            Code::AtOutsideExcept,
                            span,
                            "`@` used where its meaning is not defined",
                            "`@` may only appear in the right-hand side of an `EXCEPT` clause",
                        );
                        return Ok(self.null());
                    };
                    let at_span = self.m.node(spec).span;
                    return Ok(self.m.add(
                        at_span,
                        NodeKind::At {
                            except: exc,
                            component: spec,
                        },
                    ));
                }
                let sel = self.gen_id_to_selector(node, node, None)?;
                let r = self.selector_to_node(&sel, 0, false, false, module)?;
                if let Some(d) = self.appl_data(r) {
                    let op = d.operator;
                    self.recursion_check(op);
                }
                Ok(r)
            }
            K::OP_APPLICATION => {
                let Some(gid) = node.children().find(|c| c.kind() == K::GENERAL_ID) else {
                    return Err(self.abort(
                        Code::SanyInternal,
                        span,
                        "operator application without an operator",
                    ));
                };
                let args = node.children().find(|c| c.kind() == K::OP_ARGS);
                let sel = self.gen_id_to_selector(&gid, node, args)?;
                self.selector_to_node(&sel, 0, false, false, module)
            }
            K::FCN_APPL => self.process_fcn_appl(node, module),
            K::CHOOSE => self.process_choose(node, module),
            K::BOUNDED_QUANT => self.process_bound_quant(node, module),
            K::UNBOUNDED_QUANT | K::TEMPORAL_QUANT => self.process_unbound_quant(node, module),
            K::IF_EXPR => {
                let mut ops = Vec::new();
                for e in exprs(node) {
                    ops.push(Some(self.generate_expression(&e, module)?));
                }
                Ok(self.appl_builtin("$IfThenElse", ops, span))
            }
            K::CASE_EXPR => self.process_case(node, module),
            K::CONJ_LIST | K::DISJ_LIST => {
                let mut ops = Vec::new();
                for item in nodes(node) {
                    let e = match first_expr(&item) {
                        Some(e) => self.generate_expression(&e, module)?,
                        None => self.null(),
                    };
                    ops.push(Some(e));
                }
                let name = if node.kind() == K::DISJ_LIST {
                    "$DisjList"
                } else {
                    "$ConjList"
                };
                Ok(self.appl_builtin(name, ops, span))
            }
            K::RECORD_FIELD => {
                let rec = match first_expr(node) {
                    Some(e) => self.generate_expression(&e, module)?,
                    None => self.null(),
                };
                let field = toks(node).into_iter().rfind(|t| t.kind() == K::IDENT);
                let fs = field.as_ref().and_then(|t| self.spt(t));
                let f = self.m.add(
                    fs,
                    NodeKind::Str(field.map(|t| t.text().to_owned()).unwrap_or_default()),
                );
                Ok(self.appl_builtin("$RcdSelect", vec![Some(rec), Some(f)], span))
            }
            K::SET_OF_FCNS => {
                let mut ops = Vec::new();
                for e in exprs(node) {
                    ops.push(Some(self.generate_expression(&e, module)?));
                }
                Ok(self.appl_builtin("$SetOfFcns", ops, span))
            }
            K::SET_FILTER => self.process_subset_of(node, module),
            K::SET_MAP => self.process_set_of_all(node, module),
            K::RECORD => self.process_rcd_forms("$RcdConstructor", node, module),
            K::SET_OF_RECORDS => self.process_rcd_forms("$SetOfRcds", node, module),
            K::FCN_CONSTRUCTOR => self.process_fcn_const(node, module),
            K::ACTION_EXPR | K::FAIRNESS_EXPR => self.process_action(node, module),
            K::EXCEPT => self.process_except(node, module),
            K::LET_EXPR => self.process_let_in(node, module),
            K::LAMBDA => {
                self.err_help(
                    Code::LambdaAsExpression,
                    span,
                    "a LAMBDA expression is used where an expression is required",
                    "LAMBDA can only be an argument of an operator that takes an operator argument",
                );
                Ok(self.null())
            }
            K::LABEL => {
                let ln = self.generate_label(node, module)?;
                let is_ap =
                    matches!(&self.m.node(ln).kind, NodeKind::Label(d) if d.is_assume_prove);
                if is_ap && !allow_lap {
                    self.err(
                        Code::LabeledAssumeProveAsExpression,
                        span,
                        "a labelled ASSUME/PROVE is used where an expression is required",
                    );
                }
                Ok(ln)
            }
            other => {
                self.err(
                    Code::NotAnExpression,
                    span,
                    format!("{} is not an expression", describe(other)),
                );
                Ok(self.null())
            }
        }
    }

    /// Resolves the operator of an infix, prefix or postfix expression
    /// (`generateGenID` then `symbolTable.resolveSymbol`). `prefix` applies
    /// SANY's `unaryNegKludge` (`-` is `-.`).
    fn resolve_operator_symbol(
        &mut self,
        text: &str,
        prefix: bool,
        span: Option<crate::diagnostics::Span>,
    ) -> Option<NodeId> {
        let name = if prefix && text == "-" {
            "-.".to_owned()
        } else {
            crate::builtins::resolve_synonym(text).to_owned()
        };
        let r = self.resolve_symbol_str(&name);
        if r.is_none() {
            let shown = if prefix && text == "-" { "-" } else { text };
            let help = match name.as_str() {
                "+" | "-" | "*" | "<" | ">" | "\\leq" | "\\geq" | ".." | "%" | "\\div" | "^" => {
                    "add `EXTENDS Naturals` (or `Integers`, `Reals`)"
                }
                "-." => "add `EXTENDS Integers` (or `Reals`) for unary minus",
                "\\o" | "\\circ" => "add `EXTENDS Sequences`",
                _ => "define the operator, or extend the module that defines it",
            };
            self.err_help(
                Code::UndefinedOperatorSymbol,
                span,
                format!("operator `{shown}` is not defined"),
                help,
            );
        }
        r
    }

    /// `Function.recursionCheck`: a use of `f` inside the body of `f[x] == ..`
    /// makes the definition recursive.
    pub(crate) fn recursion_check(&mut self, op: NodeId) {
        let Some(name) = self.sym_name(op) else {
            return;
        };
        let hit = self
            .functions
            .iter()
            .rev()
            .find(|(n, _)| *n == name)
            .map(|e| e.1);
        if let Some(oan) = hit {
            let rfs = self.builtin("$RecursiveFcnSpec");
            if let Some(d) = self.appl_data_mut(oan) {
                d.operator = rfs;
            }
        }
    }

    /// The `N_FcnAppl` case: `f[a, b]`.
    fn process_fcn_appl(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let first = elems(node).into_iter().next();
        let fcn_node = first.and_then(SyntaxElement::into_node);
        let Some(fcn_node) = fcn_node else {
            self.err(
                Code::NotAnExpression,
                span,
                "`[a, b]` with no function before it is not an expression",
            );
            return Ok(self.null());
        };
        let args = fcn_args(node);
        let num_args = args.len();
        let f = self.generate_expression(&fcn_node, module)?;
        if let Some(d) = self.appl_data(f) {
            let op = d.operator;
            self.recursion_check(op);
            if let Some(od) = self.opdef(op)
                && od.kind == DefKind::UserDefined
                && let Some(body) = od.body
                && let Some(bd) = self.appl_data(body)
            {
                let bname = self.sym_text(bd.operator);
                if bname == "$NonRecursiveFcnSpec" || bname == "$RecursiveFcnSpec" {
                    let num_parms = number_of_bounded_bound_symbols(bd);
                    if num_args >= 2 && num_parms != num_args {
                        let fname = self.sym_text(op);
                        self.err(
                            Code::FunctionArgumentCount,
                            span,
                            format!(
                                "function `{fname}` is defined with {num_parms} parameters but is applied to {num_args} arguments"
                            ),
                        );
                        return Ok(self.null());
                    }
                }
            }
        }
        let arg = if num_args == 1 {
            match args.first() {
                Some(a) => self.generate_expression(a, module)?,
                None => self.null(),
            }
        } else {
            let mut ops = Vec::new();
            for a in &args {
                ops.push(Some(self.generate_expression(a, module)?));
            }
            self.appl_builtin("$Tuple", ops, span)
        };
        Ok(self.appl_builtin("$FcnApply", vec![Some(f), Some(arg)], span))
    }

    /// `Generator.processChoose`.
    fn process_choose(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let ctx = self.new_context();
        self.push_context(ctx);
        let r = self.process_choose_inner(node, module, span);
        self.pop_context();
        r
    }

    fn process_choose_inner(
        &mut self,
        node: &SyntaxNode,
        module: NodeId,
        span: Option<crate::diagnostics::Span>,
    ) -> GResult<NodeId> {
        let tuple = node.children().find(|c| c.kind() == K::IDENT_TUPLE);
        let bounded = has_tok(node, K::IN_OP);
        let es = exprs(node);
        let body_node = es.last().cloned();
        let declare = |g: &mut Self| -> Vec<NodeId> {
            let mut out = Vec::new();
            match &tuple {
                Some(t) => {
                    for id in toks(t).into_iter().filter(|t| t.kind() == K::IDENT) {
                        let n = g.intern(id.text());
                        out.push(g.new_param(Some(n), 0, g.spt(&id), true, Some(module)));
                    }
                }
                None => {
                    if let Some(id) = toks(node).into_iter().find(|t| t.kind() == K::IDENT) {
                        let n = g.intern(id.text());
                        out.push(g.new_param(Some(n), 0, g.spt(&id), true, Some(module)));
                    }
                }
            }
            out
        };
        if bounded {
            let set = match es.first() {
                Some(e) => self.generate_expression(e, module)?,
                None => self.null(),
            };
            let params = declare(self);
            self.push_formal_params(params.clone());
            let body = match &body_node {
                Some(b) => self.generate_expression(b, module),
                None => Ok(self.null()),
            };
            self.pop_formal_params();
            let body = body?;
            Ok(self.appl_bounded(
                "$BoundedChoose",
                None,
                vec![Some(body)],
                vec![params],
                vec![tuple.is_some()],
                vec![set],
                span,
            ))
        } else {
            let params = declare(self);
            self.push_formal_params(params.clone());
            let body = match &body_node {
                Some(b) => self.generate_expression(b, module),
                None => Ok(self.null()),
            };
            self.pop_formal_params();
            let body = body?;
            Ok(self.appl_unbounded("$UnboundedChoose", vec![Some(body)], params, span))
        }
    }

    /// `Generator.processBoundQuant`.
    fn process_bound_quant(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let exists = toks(node).first().is_some_and(|t| t.kind() == K::EXISTS_KW);
        let bounds: Vec<SyntaxNode> = nodes(node)
            .into_iter()
            .filter(|n| n.kind() == K::QUANT_BOUND)
            .collect();
        let ctx = self.new_context();
        self.push_context(ctx);
        let r = (|| {
            let (params, tuples, ranges) = self.process_quant_bounds(&bounds, module)?;
            self.push_formal_params(params.iter().flatten().copied().collect());
            let body = match last_expr(node) {
                Some(b) => self.generate_expression(&b, module),
                None => Ok(self.null()),
            };
            self.pop_formal_params();
            let body = body?;
            let name = if exists {
                "$BoundedExists"
            } else {
                "$BoundedForall"
            };
            Ok(self.appl_bounded(name, None, vec![Some(body)], params, tuples, ranges, span))
        })();
        self.pop_context();
        r
    }

    /// `Generator.processUnboundQuant`.
    fn process_unbound_quant(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let ts = toks(node);
        let name = match ts.first().map(dive_syntax::SyntaxToken::kind) {
            Some(K::EXISTS_KW) => "$UnboundedExists",
            Some(K::FORALL_KW) => "$UnboundedForall",
            Some(K::TEMPORAL_EXISTS_KW) => "$TemporalExists",
            _ => "$TemporalForall",
        };
        let ctx = self.new_context();
        self.push_context(ctx);
        let mut params = Vec::new();
        for id in ts.iter().filter(|t| t.kind() == K::IDENT) {
            let n = self.intern(id.text());
            params.push(self.new_param(Some(n), 0, self.spt(id), true, Some(module)));
        }
        self.push_formal_params(params.clone());
        let body = match last_expr(node) {
            Some(b) => self.generate_expression(&b, module),
            None => Ok(self.null()),
        };
        self.pop_formal_params();
        self.pop_context();
        let body = body?;
        Ok(self.appl_unbounded(name, vec![Some(body)], params, span))
    }

    /// `Generator.processCase`.
    fn process_case(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let mut pairs = Vec::new();
        for arm in nodes(node) {
            let es = exprs(&arm);
            let guard = if arm.kind() == K::OTHER_ARM {
                None
            } else {
                match es.first() {
                    Some(e) => Some(self.generate_expression(e, module)?),
                    None => Some(self.null()),
                }
            };
            let value = match es.last() {
                Some(e) => self.generate_expression(e, module)?,
                None => self.null(),
            };
            let s = self.sp(&arm);
            pairs.push(Some(self.appl_builtin(
                "$Pair",
                vec![guard, Some(value)],
                s,
            )));
        }
        Ok(self.appl_builtin("$Case", pairs, span))
    }

    /// `Generator.processSubsetOf`: `{x \in S : P}`.
    fn process_subset_of(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let Some(qb) = node.children().find(|c| c.kind() == K::QUANT_BOUND) else {
            return Ok(self.null());
        };
        let set = match last_expr(&qb) {
            Some(e) => self.generate_expression(&e, module)?,
            None => self.null(),
        };
        let ctx = self.new_context();
        self.push_context(ctx);
        let tuple = qb.children().find(|c| c.kind() == K::IDENT_TUPLE);
        let src = tuple.clone().unwrap_or_else(|| qb.clone());
        let mut params = Vec::new();
        for id in toks(&src).into_iter().filter(|t| t.kind() == K::IDENT) {
            let n = self.intern(id.text());
            params.push(self.new_param(Some(n), 0, self.spt(&id), true, Some(module)));
        }
        self.push_formal_params(params.clone());
        let pred = match last_expr(node) {
            Some(e) => self.generate_expression(&e, module),
            None => Ok(self.null()),
        };
        self.pop_formal_params();
        self.pop_context();
        let pred = pred?;
        Ok(self.appl_bounded(
            "$SubsetOf",
            None,
            vec![Some(pred)],
            vec![params],
            vec![tuple.is_some()],
            vec![set],
            span,
        ))
    }

    /// `Generator.processSetOfAll`: `{e : x \in S}`.
    fn process_set_of_all(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let bounds: Vec<SyntaxNode> = nodes(node)
            .into_iter()
            .filter(|n| n.kind() == K::QUANT_BOUND)
            .collect();
        let ctx = self.new_context();
        self.push_context(ctx);
        let r = (|| {
            let (params, tuples, ranges) = self.process_quant_bounds(&bounds, module)?;
            self.push_formal_params(params.iter().flatten().copied().collect());
            let e = match first_expr(node) {
                Some(e) => self.generate_expression(&e, module),
                None => Ok(self.null()),
            };
            self.pop_formal_params();
            let e = e?;
            Ok(self.appl_bounded(
                "$SetOfAll",
                None,
                vec![Some(e)],
                params,
                tuples,
                ranges,
                span,
            ))
        })();
        self.pop_context();
        r
    }

    /// `Generator.processFcnConst`: `[x \in S |-> e]`.
    fn process_fcn_const(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let bounds: Vec<SyntaxNode> = nodes(node)
            .into_iter()
            .filter(|n| n.kind() == K::QUANT_BOUND)
            .collect();
        let ctx = self.new_context();
        self.push_context(ctx);
        let r = (|| {
            let (params, tuples, ranges) = self.process_quant_bounds(&bounds, module)?;
            self.push_formal_params(params.iter().flatten().copied().collect());
            let e = match last_expr(node) {
                Some(e) => self.generate_expression(&e, module),
                None => Ok(self.null()),
            };
            self.pop_formal_params();
            let e = e?;
            Ok(self.appl_bounded(
                "$FcnConstructor",
                None,
                vec![Some(e)],
                params,
                tuples,
                ranges,
                span,
            ))
        })();
        self.pop_context();
        r
    }

    /// `Generator.processRcdForms`.
    fn process_rcd_forms(
        &mut self,
        op: &str,
        node: &SyntaxNode,
        module: NodeId,
    ) -> GResult<NodeId> {
        let span = self.sp(node);
        let mut pairs = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for field in nodes(node) {
            let ft = toks(&field);
            let Some(name_tok) = ft.first() else { continue };
            let name = name_tok.text().to_owned();
            if seen.contains(&name) {
                self.err_help(
                    Code::DuplicateRecordField,
                    self.spt(name_tok),
                    format!("field `{name}` appears twice"),
                    "each field of a record may appear only once",
                );
            }
            seen.push(name.clone());
            let s = self.m.add(self.spt(name_tok), NodeKind::Str(name));
            let v = match last_expr(&field) {
                Some(e) => self.generate_expression(&e, module)?,
                None => self.null(),
            };
            let fs = self.sp(&field);
            pairs.push(Some(self.appl_builtin("$Pair", vec![Some(s), Some(v)], fs)));
        }
        Ok(self.appl_builtin(op, pairs, span))
    }

    /// `Generator.processAction`: `[A]_v`, `<<A>>_v`, `WF_v(A)`, `SF_v(A)`.
    fn process_action(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let name = match toks(node).first().map(dive_syntax::SyntaxToken::kind) {
            Some(K::L_ANGLE) => "$AngleAct",
            Some(K::L_BRACK) => "$SquareAct",
            Some(K::SF_KW) => "$SF",
            _ => "$WF",
        };
        let parts = nodes(node);
        let mut ops = Vec::new();
        for i in 0..2 {
            let e = match parts.get(i) {
                Some(p) => self.generate_expression(p, module)?,
                None => self.null(),
            };
            ops.push(Some(e));
        }
        Ok(self.appl_builtin(name, ops, span))
    }

    /// `Generator.processExcept`.
    fn process_except(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        let base = match first_expr(node) {
            Some(e) => self.generate_expression(&e, module)?,
            None => self.null(),
        };
        let exc = self.appl_builtin("$Except", vec![Some(base)], span);
        let specs: Vec<SyntaxNode> = nodes(node)
            .into_iter()
            .filter(|n| n.kind() == K::EXCEPT_SPEC)
            .collect();
        for spec in specs {
            let spec_span = self.sp(&spec);
            let mut comps = Vec::new();
            for comp in nodes(&spec)
                .into_iter()
                .filter(|n| n.kind() == K::EXCEPT_COMPONENT)
            {
                if has_tok(&comp, K::L_BRACK) {
                    let es = exprs(&comp);
                    if es.len() > 1 {
                        let mut ops = Vec::new();
                        for e in &es {
                            ops.push(Some(self.generate_expression(e, module)?));
                        }
                        let cs = self.sp(&comp);
                        comps.push(Some(self.appl_builtin("$Tuple", ops, cs)));
                    } else {
                        let e = match es.first() {
                            Some(e) => self.generate_expression(e, module)?,
                            None => self.null(),
                        };
                        comps.push(Some(e));
                    }
                } else {
                    let f = toks(&comp).into_iter().find(|t| t.kind() == K::IDENT);
                    let fs = f.as_ref().and_then(|t| self.spt(t));
                    let s = self.m.add(
                        fs,
                        NodeKind::Str(f.map(|t| t.text().to_owned()).unwrap_or_default()),
                    );
                    comps.push(Some(s));
                }
            }
            let seq = self.appl_builtin("$Seq", comps, spec_span);
            let pair = self.appl_builtin("$Pair", vec![Some(seq), None], spec_span);
            self.exc_spec_stack.push(pair);
            self.exc_stack.push(exc);
            let rhs = match last_expr(&spec) {
                Some(e) => self.generate_expression(&e, module),
                None => Ok(self.null()),
            };
            self.exc_spec_stack.pop();
            self.exc_stack.pop();
            let rhs = rhs?;
            if let Some(d) = self.appl_data_mut(pair) {
                d.operands[1] = Some(rhs);
            }
            if let Some(d) = self.appl_data_mut(exc) {
                d.operands.push(Some(pair));
            }
        }
        Ok(exc)
    }

    /// `Generator.generateExprOrOpArg`: argument `pos` (from 0) of `main_op`.
    pub(crate) fn generate_expr_or_op_arg(
        &mut self,
        main_op: NodeId,
        main_span: Option<crate::diagnostics::Span>,
        pos: usize,
        arg: &SyntaxNode,
        module: NodeId,
    ) -> GResult<NodeId> {
        let main_name = self.sym_text(main_op);
        if self.kind(main_op) == Kind::Error {
            self.err(
                Code::SanyInternal,
                main_span,
                "cannot generate an argument because of earlier errors",
            );
            return Ok(self.null());
        }
        if self.kind(main_op) == Kind::Module {
            self.err(
                Code::ModuleNameAsOperator,
                main_span,
                format!("module name `{main_name}` used as an operator"),
            );
            return Ok(self.null());
        }
        let arity = self.arity(main_op);
        if i64::try_from(pos).unwrap_or(i64::MAX) + 1 > i64::from(arity) {
            self.err(
                Code::WrongArgumentCount,
                main_span,
                format!("too many arguments for `{main_name}`, which takes {arity}"),
            );
            return Ok(self.null());
        }
        let expected = match self.kind(main_op) {
            Kind::UserDefinedOp | Kind::ModuleInstance => {
                let params = self
                    .opdef(main_op)
                    .map(|d| d.params.clone())
                    .unwrap_or_default();
                let Some(&p) = params.get(pos) else {
                    return Ok(self.null());
                };
                self.arity(p)
            }
            _ => 0,
        };
        if expected == 0 {
            return self.generate_expression(arg, module);
        }
        let arg_span = self.sp(arg);
        if !matches!(arg.kind(), K::GENERAL_ID | K::OP_REF | K::LAMBDA) {
            self.err_help(
                Code::OperatorArgumentExpected,
                arg_span,
                format!(
                    "argument {} of `{main_name}` must be an operator of arity {expected}, but an expression is given",
                    pos + 1
                ),
                "pass an operator name or a LAMBDA",
            );
            return Ok(self.null());
        }
        if arg.kind() == K::LAMBDA {
            let lam = self.generate_lambda(arg, module)?;
            let la = self.arity(lam);
            if la == expected {
                return Ok(self.new_op_arg(lam, arg_span));
            }
            self.err(
                Code::LambdaArgumentArity,
                main_span,
                format!(
                    "a LAMBDA with {la} parameters is argument {} of `{main_name}`, which requires an operator of arity {expected}",
                    pos + 1
                ),
            );
            return Ok(self.null());
        }
        if arg.kind() == K::GENERAL_ID {
            let sel = self.gen_id_to_selector(arg, arg, None)?;
            return self.selector_to_node(&sel, expected, false, false, module);
        }
        // OP_REF: a bare operator symbol.
        let text = toks(arg)
            .first()
            .map(|t| t.text().to_owned())
            .unwrap_or_default();
        let resolved = crate::builtins::resolve_synonym(&text).to_owned();
        let Some(op) = self.resolve_symbol_str(&resolved) else {
            self.err(
                Code::UndefinedOperatorSymbol,
                arg_span,
                format!("operator `{text}` is not defined"),
            );
            return Ok(self.null());
        };
        let a = self.arity(op);
        if a == expected {
            return Ok(self.new_op_arg(op, arg_span));
        }
        self.err(
            Code::OperatorArgumentArity,
            main_span,
            format!(
                "operator `{text}` of arity {a} is argument {} of `{main_name}`, which requires an operator of arity {expected}",
                pos + 1
            ),
        );
        Ok(self.null())
    }

    /// `new OpArgNode(op, stn, mn)`.
    pub(crate) fn new_op_arg(
        &mut self,
        op: NodeId,
        span: Option<crate::diagnostics::Span>,
    ) -> NodeId {
        let arity = self.arity(op);
        self.m.add(span, NodeKind::OpArg(OpArgData { op, arity }))
    }

    /// `Generator.generateLambda`.
    pub(crate) fn generate_lambda(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let ctx = self.new_context();
        self.push_context(ctx);
        let mut params = Vec::new();
        for id in toks(node).into_iter().filter(|t| t.kind() == K::IDENT) {
            let n = self.intern(id.text());
            params.push(self.new_param(Some(n), 0, self.spt(&id), true, Some(module)));
        }
        self.push_formal_params(params.clone());
        let body = match last_expr(node) {
            Some(b) => self.generate_expression(&b, module),
            None => Ok(self.null()),
        };
        self.pop_formal_params();
        self.pop_context();
        let body = body?;
        let name = self.intern("LAMBDA");
        Ok(self.new_opdef(
            name,
            DefKind::UserDefined,
            params,
            false,
            Some(body),
            Some(module),
            false,
            self.sp(node),
            true,
            None,
        ))
    }

    /// `Generator.generateLabel`.
    pub(crate) fn generate_label(&mut self, node: &SyntaxNode, module: NodeId) -> GResult<NodeId> {
        let span = self.sp(node);
        if !self.in_op_def_node() {
            self.err_help(
                Code::LabelOutsideDefinition,
                span,
                "a label must be inside a definition or a proof step",
                "remove the label",
            );
            return Ok(self.null());
        }
        if self.no_labels_allowed() {
            self.err(
                Code::LabelInNestedAssume,
                span,
                "labels are not allowed in the scope of a declaration in a nested ASSUME/PROVE",
            );
            return Ok(self.null());
        }
        if !(self.exc_stack.is_empty() || self.exc_spec_stack.is_empty()) {
            self.err(
                Code::LabelInExcept,
                span,
                "labels inside EXCEPT clauses are not supported",
            );
            return Ok(self.null());
        }
        let parts = nodes(node);
        let name_part = parts.first().cloned();
        let body_part = parts.get(1).cloned();
        self.push_ls();
        let body = match &body_part {
            Some(b) if b.kind() == K::ASSUME_PROVE => {
                self.generate_assume_prove(b, module).map(|n| (n, true))
            }
            Some(b) if b.kind() == K::LABEL && label_wraps_assume_prove(b) => {
                // A labelled ASSUME/PROVE inside a label: SANY's grammar only
                // produces this through AssumeProve(), which handles it.
                self.generate_expression_or_lap(b, module, true)
                    .map(|n| (n, true))
            }
            Some(b) => self.generate_expression(b, module).map(|n| (n, false)),
            None => Ok((self.null(), false)),
        };
        let ht = self.pop_label_node_set();
        let (body, is_ap) = body?;
        let (name_text, arg_nodes) = match &name_part {
            Some(p) if p.kind() == K::OP_APPLICATION => {
                let gid = p.children().find(|c| c.kind() == K::GENERAL_ID);
                let n = gid
                    .and_then(dive_syntax::ast::GeneralId::cast)
                    .and_then(|g| g.name())
                    .unwrap_or_default();
                let args = p
                    .children()
                    .find(|c| c.kind() == K::OP_ARGS)
                    .map(|a| nodes(&a))
                    .unwrap_or_default();
                (n, args)
            }
            Some(p) => (
                dive_syntax::ast::GeneralId::cast(p.clone())
                    .and_then(|g| g.name())
                    .unwrap_or_default(),
                Vec::new(),
            ),
            None => (String::new(), Vec::new()),
        };
        let name = self.intern(&name_text);
        let mut params = Vec::new();
        for a in &arg_nodes {
            let an = dive_syntax::ast::GeneralId::cast(a.clone())
                .and_then(|g| g.name())
                .unwrap_or_default();
            let ann = self.intern(&an);
            let found = self
                .resolve_symbol(ann)
                .filter(|&s| self.kind(s) == Kind::FormalParam);
            let p = if let Some(p) = found {
                p
            } else {
                self.err(
                    Code::LabelParameterExtra,
                    self.sp(a),
                    format!("`{an}` is not a valid parameter of label `{name_text}`"),
                );
                self.new_param(Some(ann), 0, self.sp(a), false, Some(module))
            };
            params.push(p);
        }
        let goal = self.current_goal;
        let label = self.m.add(
            span,
            NodeKind::Label(Box::new(LabelData {
                name,
                params,
                goal,
                goal_clause: self.current_goal_clause,
                body: Some(body),
                is_assume_prove: is_ap,
                labels: None,
            })),
        );
        self.set_labels(label, ht);
        self.formal_params_equal(label, &name_text);
        if !self.add_label_node_to_set(label, name) {
            self.err(
                Code::LabelDuplicate,
                span,
                format!("label `{name_text}` is used twice"),
            );
        }
        Ok(label)
    }

    /// `Generator.formalParamsEqual`.
    fn formal_params_equal(&mut self, label: NodeId, name: &str) {
        let span = self.m.node(label).span;
        let params = match &self.m.node(label).kind {
            NodeKind::Label(d) => d.params.clone(),
            _ => return,
        };
        let mut set: BTreeSet<NodeId> = BTreeSet::new();
        for p in &params {
            if !set.insert(*p) {
                let pn = self.sym_text(*p);
                self.err(
                    Code::LabelParameterRepeated,
                    span,
                    format!("parameter `{pn}` is repeated in label `{name}`"),
                );
            }
        }
        let scope: Vec<NodeId> = self
            .ls_params
            .last()
            .map(|v| v.iter().flatten().copied().collect())
            .unwrap_or_default();
        for p in scope {
            if !set.remove(&p) {
                let pn = self.sym_text(p);
                self.err_help(
                    Code::LabelParameterMissing,
                    span,
                    format!("label `{name}` must have the bound identifier `{pn}` as a parameter"),
                    format!("write `{name}({pn}, ..) :: ..`"),
                );
            }
        }
        if !set.is_empty() {
            let extra: Vec<String> = set.iter().map(|p| self.sym_text(*p)).collect();
            self.err(
                Code::LabelParameterExtra,
                span,
                format!(
                    "label `{name}` declares extra parameters: {}",
                    extra.join(", ")
                ),
            );
        }
    }

    /// `Generator.noLabelsAllowed`.
    fn no_labels_allowed(&self) -> bool {
        (2..=self.ap_depth).any(|i| self.in_scope_of_ap_decl.get(i).copied().unwrap_or(false))
    }
}

/// Whether a `LABEL` node labels an `ASSUME`/`PROVE` (possibly through more
/// labels).
fn label_wraps_assume_prove(n: &SyntaxNode) -> bool {
    match n.children().nth(1) {
        Some(b) if b.kind() == K::ASSUME_PROVE => true,
        Some(b) if b.kind() == K::LABEL => label_wraps_assume_prove(&b),
        _ => false,
    }
}

/// The argument expressions of an `FCN_APPL` (those inside the brackets).
fn fcn_args(node: &SyntaxNode) -> Vec<SyntaxNode> {
    let mut seen = false;
    let mut out = Vec::new();
    for e in elems(node) {
        match e {
            SyntaxElement::Token(t) if t.kind() == K::L_BRACK => seen = true,
            SyntaxElement::Node(n) if seen => out.push(n),
            _ => {}
        }
    }
    out
}

/// `OpApplNode.getNumberOfBoundedBoundSymbols`: a tuple counts once.
pub(crate) fn number_of_bounded_bound_symbols(d: &crate::model::OpApplData) -> usize {
    let (Some(b), Some(t)) = (&d.bounded, &d.tuples) else {
        return 0;
    };
    b.iter()
        .zip(t.iter())
        .map(|(g, &tuple)| if tuple { 1 } else { g.len() })
        .sum()
}

/// A string literal's value: quotes removed and escapes resolved.
fn unescape(lit: &str) -> String {
    let inner = lit
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(lit);
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('f') => out.push('\u{c}'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A human description of a syntax kind used where an expression is needed.
fn describe(k: K) -> &'static str {
    match k {
        K::OP_REF => "a bare operator symbol",
        K::ASSUME_PROVE => "ASSUME/PROVE",
        _ => "this construct",
    }
}
