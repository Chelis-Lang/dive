//! Instances and substitutions: `Generator.processModuleDefinition`,
//! `generateInstance`, `processSubst`, `generateSubst`, `generateOpArg`, and
//! the substitution logic of `SubstInNode` / `APSubstInNode`.

use super::cst::{last_expr, nodes, toks};
use super::{GResult, Gen};
use crate::diagnostics::{Code, Span};
use crate::model::{CtxId, DefKind, InstanceData, Key, Kind, NodeId, NodeKind, Subst, SubstInData};
use dive_syntax::{SyntaxKind as K, SyntaxNode};

/// A substitution template (`SubstInNode` without a body).
pub(crate) struct Template {
    pub(crate) substs: Vec<Subst>,
    pub(crate) instantiating: NodeId,
    pub(crate) instantiated: NodeId,
}

impl Gen<'_> {
    /// The `OpDeclNode`s of a context, in order (`getByClass(OpDeclNode)`).
    fn ctx_decls(&self, ctx: CtxId) -> Vec<NodeId> {
        self.m
            .context(ctx)
            .entries()
            .filter(|(_, v)| matches!(self.m.node(*v).kind, NodeKind::OpDecl(_)))
            .map(|(_, v)| v)
            .collect()
    }

    fn ctx_by(&self, ctx: CtxId, thm: bool) -> Vec<NodeId> {
        self.m
            .context(ctx)
            .entries()
            .filter(|(k, v)| {
                matches!(k, Key::Sym(_))
                    && if thm {
                        matches!(self.m.node(*v).kind, NodeKind::ThmOrAssumpDef(_))
                    } else {
                        matches!(self.m.node(*v).kind, NodeKind::OpDef(_))
                    }
            })
            .map(|(_, v)| v)
            .collect()
    }

    /// `Generator.processSubst` (with `SubstInNode.constructSubst`,
    /// `addExplicitSubstitute` and `matchAll`).
    fn process_subst(
        &mut self,
        span: Option<Span>,
        substs_syntax: &[SyntaxNode],
        instancee_ctx: CtxId,
        instancee: NodeId,
        module: NodeId,
    ) -> GResult<Template> {
        let decls = self.ctx_decls(instancee_ctx);
        let mut substs = Vec::new();
        // constructSubst: implicit substitutions c <- c.
        for &decl in &decls {
            let Some(name) = self.sym_name(decl) else {
                continue;
            };
            let Some(symb) = self.resolve_symbol(name) else {
                continue;
            };
            let k = self.kind(decl);
            let ar = self.arity(decl);
            let expr = if k == Kind::VariableDecl || (k == Kind::ConstantDecl && ar == 0) {
                self.appl_match(symb, Vec::new(), span)?
            } else {
                self.new_op_arg(symb, span)
            };
            substs.push(Subst {
                op: decl,
                expr,
                implicit: true,
            });
        }
        // Explicit substitutions.
        for s in substs_syntax {
            let Some(target) = toks(s).into_iter().next() else {
                continue;
            };
            let rhs = s.children().last();
            let tname = crate::builtins::resolve_synonym(target.text()).to_owned();
            let tname_raw = target.text().to_owned();
            let value = self.generate_subst(
                instancee_ctx,
                &tname_raw,
                self.spt(&target),
                rhs.as_ref(),
                module,
            )?;
            let lhs = self.intern(&tname_raw);
            let _ = tname;
            let idx = substs.iter().position(|x| self.sym_name(x.op) == Some(lhs));
            match idx {
                Some(i) => {
                    if substs[i].implicit {
                        substs[i].expr = value;
                        substs[i].implicit = false;
                    } else {
                        self.err(
                            Code::SubstitutionDuplicate,
                            self.sp(s),
                            format!("`{tname_raw}` is substituted twice"),
                        );
                    }
                }
                None => {
                    if let Some(sym) = self.m.context(instancee_ctx).get(Key::Sym(lhs))
                        && matches!(self.m.node(sym).kind, NodeKind::OpDecl(_))
                    {
                        substs.push(Subst {
                            op: sym,
                            expr: value,
                            implicit: false,
                        });
                    }
                }
            }
        }
        for s in substs.clone() {
            if s.implicit {
                self.check_subst_arity(s.op, s.expr);
            }
        }
        // matchAll
        let inst_name = self
            .module_data(instancee)
            .map(|d| self.text(d.name))
            .unwrap_or_default();
        for decl in decls {
            let dn = self.sym_name(decl);
            if !substs.iter().any(|s| self.sym_name(s.op) == dn) {
                let n = self.sym_text(decl);
                let mut d = crate::diagnostics::Diagnostic::new(
                    Code::SubstitutionMissing,
                    span,
                    format!(
                        "no substitution for `{n}`, which module `{inst_name}` declares; the instantiating module has no `{n}` either"
                    ),
                )
                .with_help(format!("add `WITH {n} <- ...` or declare or define `{n}`"));
                if let Some(s) = self.m.node(decl).span {
                    d = d.with_related(s, "declared here");
                }
                self.report(d);
            }
        }
        Ok(Template {
            substs,
            instantiating: module,
            instantiated: instancee,
        })
    }

    /// `Generator.generateSubst`.
    fn generate_subst(
        &mut self,
        instancee_ctx: CtxId,
        target: &str,
        target_span: Option<Span>,
        rhs: Option<&SyntaxNode>,
        module: NodeId,
    ) -> GResult<NodeId> {
        let tn = self.intern(target);
        let sym = self
            .m
            .context(instancee_ctx)
            .get(Key::Sym(tn))
            .filter(|s| matches!(self.m.node(*s).kind, NodeKind::OpDecl(_)));
        let Some(sym) = sym else {
            self.err_help(
                Code::SubstitutionIllegalTarget,
                target_span,
                format!("`{target}` is not a CONSTANT or VARIABLE of the instantiated module"),
                "only declared constants and variables can be substituted",
            );
            return Ok(self.null());
        };
        let Some(rhs) = rhs else {
            return Ok(self.null());
        };
        let value = if self.arity(sym) == 0 {
            self.generate_expression(rhs, module)?
        } else {
            self.generate_op_arg(sym, rhs, module)?
        };
        self.check_subst_arity(sym, value);
        Ok(value)
    }

    /// `Generator.checkSubstitutionForCorrectArity`.
    fn check_subst_arity(&mut self, target: NodeId, value: NodeId) {
        let expected = self.arity(target);
        let actual = match &self.m.node(value).kind {
            NodeKind::OpArg(d) => d.arity,
            _ => 0,
        };
        if actual != expected {
            let n = self.sym_text(target);
            let at = self.m.node(value).span;
            self.err(
                Code::SubstitutionArity,
                at,
                format!("an operator of arity {expected} must be substituted for `{n}`"),
            );
        }
    }

    /// `Generator.generateOpArg`.
    fn generate_op_arg(
        &mut self,
        target: NodeId,
        node: &SyntaxNode,
        module: NodeId,
    ) -> GResult<NodeId> {
        let tname = self.sym_text(target);
        let tar = self.arity(target);
        if node.kind() == K::LAMBDA {
            let l = self.generate_lambda(node, module)?;
            return Ok(self.new_op_arg(l, self.sp(node)));
        }
        if !matches!(node.kind(), K::GENERAL_ID | K::OP_REF) {
            self.err(
                Code::SubstitutionOperatorExpected,
                self.sp(node),
                format!("an operator of arity {tar}, not an expression, must be substituted for `{tname}`"),
            );
            return Ok(self.null());
        }
        if node.kind() == K::GENERAL_ID {
            let sel = self.gen_id_to_selector(node, node, None)?;
            let ln = self.selector_to_node(&sel, tar, false, false, module)?;
            if self.kind(ln) != Kind::OpArg {
                if self.sh.errors > 0 {
                    return Ok(self.null());
                }
                return Err(self.abort(
                    Code::SanyInternal,
                    self.sp(node),
                    "expected an operator argument but found something else",
                ));
            }
            return Ok(ln);
        }
        let text = toks(node)
            .first()
            .map(|t| t.text().to_owned())
            .unwrap_or_default();
        let resolved = crate::builtins::resolve_synonym(&text).to_owned();
        match self.resolve_symbol_str(&resolved) {
            Some(op) => Ok(self.new_op_arg(op, self.sp(node))),
            None => {
                self.err(
                    Code::UndefinedOperatorSymbol,
                    self.sp(node),
                    format!("operator `{text}` is not defined"),
                );
                Ok(self.null())
            }
        }
    }

    /// `new SubstInNode(..)` / `new APSubstInNode(..)` from a template.
    fn wrap(&mut self, t: &Template, body: Option<NodeId>, ap: bool, span: Option<Span>) -> NodeId {
        if body.is_none_or(|b| self.kind(b) == Kind::Error) {
            self.err(
                Code::SanyInternal,
                span,
                "substitution error, probably because of an error in the instantiated module",
            );
        }
        let data = Box::new(SubstInData {
            substs: t.substs.clone(),
            body,
            instantiating: t.instantiating,
            instantiated: t.instantiated,
        });
        let kind = if ap {
            NodeKind::ApSubstIn(data)
        } else {
            NodeKind::SubstIn(data)
        };
        self.m.add(span, kind)
    }

    fn instantiation_parts(node: &SyntaxNode) -> Option<(String, Vec<SyntaxNode>, SyntaxNode)> {
        let inst = if node.kind() == K::INSTANTIATION {
            node.clone()
        } else {
            node.children().find(|c| c.kind() == K::INSTANTIATION)?
        };
        let name = toks(&inst)
            .into_iter()
            .find(|t| t.kind() == K::IDENT)?
            .text()
            .to_owned();
        let substs = nodes(&inst)
            .into_iter()
            .filter(|n| n.kind() == K::SUBSTITUTION)
            .collect();
        Some((name, substs, inst))
    }

    /// `Generator.processModuleDefinition`: `I(p) == INSTANCE M WITH ...`.
    pub(crate) fn process_module_definition(
        &mut self,
        node: &SyntaxNode,
        mut defs: Option<&mut Vec<NodeId>>,
        insts: Option<&mut Vec<NodeId>>,
        module: NodeId,
    ) -> GResult<NodeId> {
        let local = toks(node).iter().any(|t| t.kind() == K::LOCAL_KW);
        let span = self.sp(node);
        let lhs = node.children().find(|c| c.kind() == K::IDENT_LHS);
        let name_text = lhs
            .as_ref()
            .and_then(|l| toks(l).into_iter().find(|t| t.kind() == K::IDENT))
            .map(|t| t.text().to_owned())
            .unwrap_or_default();
        let name = self.intern(&name_text);
        let mut args = Vec::new();
        let mut parm_ctx = None;
        let lhs_params = lhs.as_ref().map(nodes).unwrap_or_default();
        if !lhs_params.is_empty() {
            let c = self.new_context();
            self.push_context(c);
            parm_ctx = Some(c);
            for p in &lhs_params {
                let Some((pn, ar)) = self.lhs_param(p) else {
                    return Err(self.abort(Code::SanyInternal, span, "bad formal parameters"));
                };
                let id = self.new_param(Some(pn), ar, self.sp(p), true, Some(module));
                args.push(id);
            }
        }
        let Some((mname, substs_syntax, inst_node)) = Self::instantiation_parts(node) else {
            if parm_ctx.is_some() {
                self.pop_context();
            }
            return Ok(self.null());
        };
        let mn = self.intern(&mname);
        let inst_span = toks(&inst_node)
            .into_iter()
            .find(|t| t.kind() == K::IDENT)
            .and_then(|t| self.spt(&t));
        let ictx = self.module_context(mn);
        let imod = self.resolve_module(mn);
        let (Some(ictx), Some(imod)) = (ictx, imod) else {
            if parm_ctx.is_some() {
                self.pop_context();
            }
            self.err(
                Code::ModuleUnavailable,
                inst_span,
                format!("module `{mname}` is not known here"),
            );
            return Ok(self.null());
        };
        if let Some(d) = self.module_mut(imod) {
            d.is_instantiated = true;
        }
        let t = self.process_subst(span, &substs_syntax, ictx, imod, module);
        if parm_ctx.is_some() {
            self.pop_context();
        }
        let t = t?;
        for odn in self.ctx_by(ictx, false) {
            let Some(d) = self.opdef(odn).cloned() else {
                continue;
            };
            if d.local || !matches!(d.kind, DefKind::UserDefined | DefKind::ModuleInstance) {
                continue;
            }
            let qn = format!("{name_text}!{}", self.text(d.name));
            let qname = self.intern(&qn);
            let mut params = args.clone();
            params.extend(d.params.iter().copied());
            let src = self.source_of(odn);
            let new_odn = if d.kind == DefKind::UserDefined {
                let body = if t.substs.is_empty() {
                    d.body
                } else {
                    Some(self.wrap(&t, d.body, false, span))
                };
                let o = self.new_opdef(
                    qname,
                    DefKind::UserDefined,
                    params,
                    local,
                    body,
                    Some(module),
                    true,
                    span,
                    true,
                    Some(src),
                );
                self.set_op_def_node_recursion_fields(o, module);
                self.set_labels_id(o, d.labels);
                o
            } else {
                self.new_instance_def(qname, params, local, d.module, true, span, Some(src))
            };
            match defs.as_deref_mut() {
                None => {
                    if let Some(md) = self.module_mut(module) {
                        md.definitions.push(new_odn);
                    }
                }
                Some(v) => v.push(new_odn),
            }
        }
        for ta in self.ctx_by(ictx, true) {
            let Some(d) = self.thmdef(ta).cloned() else {
                continue;
            };
            if d.local {
                continue;
            }
            let qn = format!("{name_text}!{}", self.text(d.name));
            let qname = self.intern(&qn);
            let mut params = args.clone();
            params.extend(d.params.iter().copied());
            let src = self.source_of(ta);
            let body = if t.substs.is_empty() {
                d.body
            } else {
                Some(self.wrap(&t, d.body, true, span))
            };
            let n = self.new_thmdef(
                qname,
                d.theorem,
                body,
                Some(module),
                true,
                span,
                params,
                Some(imod),
                Some(src),
            );
            if let Some(td) = self.thmdef_mut(n) {
                td.local = local;
            }
            self.set_labels_id(n, d.labels);
            match defs.as_deref_mut() {
                None => {
                    if let Some(md) = self.module_mut(module) {
                        md.definitions.push(n);
                    }
                }
                Some(v) => v.push(n),
            }
        }
        let inst = self.m.add(
            span,
            NodeKind::Instance(Box::new(InstanceData {
                name: Some(name),
                local,
                params: args.clone(),
                module: imod,
                substs: t.substs.clone(),
                step: None,
            })),
        );
        match insts {
            None => {
                if let Some(md) = self.module_mut(module) {
                    md.instances.push(inst);
                    md.top_level.push(inst);
                }
            }
            Some(v) => v.push(inst),
        }
        let module_def = self.new_instance_def(name, args, local, Some(module), true, span, None);
        if let Some(v) = defs {
            v.push(module_def);
        }
        Ok(module_def)
    }

    /// The name and arity of a formal parameter of a module definition.
    fn lhs_param(&mut self, p: &SyntaxNode) -> Option<(crate::model::Name, i32)> {
        let ts = toks(p);
        let (text, ar) = match p.kind() {
            K::IDENT_DECL => (
                ts.iter().find(|t| t.kind() == K::IDENT)?.text().to_owned(),
                i32::try_from(ts.iter().filter(|t| t.kind() == K::UNDERSCORE).count()).unwrap_or(0),
            ),
            K::INFIX_DECL => (ts.get(1)?.text().to_owned(), 2),
            K::PREFIX_DECL => (ts.first()?.text().to_owned(), 1),
            K::POSTFIX_DECL => (ts.get(1)?.text().to_owned(), 1),
            _ => return None,
        };
        Some((self.intern(&text), ar))
    }

    /// `Generator.generateInstance`: `[LOCAL] INSTANCE M WITH ...` at the top
    /// level (`top_level`) or as a proof step.
    pub(crate) fn generate_instance(
        &mut self,
        node: &SyntaxNode,
        module: NodeId,
        top_level: bool,
    ) -> GResult<NodeId> {
        let local = toks(node).iter().any(|t| t.kind() == K::LOCAL_KW);
        let span = self.sp(node);
        let Some((mname, substs_syntax, inst_node)) = Self::instantiation_parts(node) else {
            return Ok(self.null());
        };
        let name_span = toks(&inst_node)
            .into_iter()
            .find(|t| t.kind() == K::IDENT)
            .and_then(|t| self.spt(&t));
        let mn = self.intern(&mname);
        let Some(ictx) = self.module_context(mn) else {
            return Err(self.abort(
                Code::ModuleUnavailable,
                name_span,
                format!("no context is available for module `{mname}`"),
            ));
        };
        let Some(imod) = self.resolve_module(mn) else {
            return Err(self.abort(
                Code::ModuleUnavailable,
                name_span,
                format!("could not find module `{mname}`"),
            ));
        };
        if let Some(d) = self.module_mut(imod) {
            d.is_instantiated = true;
        }
        let t = self.process_subst(span, &substs_syntax, ictx, imod, module)?;
        let param_free = self.is_parameter_free(imod);
        for odn in self.ctx_by(ictx, false) {
            let Some(d) = self.opdef(odn).cloned() else {
                continue;
            };
            if matches!(d.kind, DefKind::BuiltIn | DefKind::ModuleInstance) || d.local {
                continue;
            }
            let src = self.source_of(odn);
            let new_odn = if param_free {
                if local {
                    let o = self.new_opdef(
                        d.name,
                        DefKind::UserDefined,
                        d.params.clone(),
                        local,
                        d.body,
                        d.module,
                        true,
                        span,
                        true,
                        Some(src),
                    );
                    self.set_labels_id(o, d.labels);
                    o
                } else {
                    self.add_symbol(d.name, odn);
                    odn
                }
            } else {
                let orig_free = match d.module {
                    Some(om) => self.is_parameter_free(om),
                    None => true,
                };
                if orig_free {
                    if local && top_level {
                        let o = self.new_opdef(
                            d.name,
                            DefKind::UserDefined,
                            d.params.clone(),
                            local,
                            d.body,
                            d.module,
                            true,
                            span,
                            true,
                            Some(src),
                        );
                        self.set_labels_id(o, d.labels);
                        o
                    } else {
                        self.add_symbol(d.name, odn);
                        odn
                    }
                } else {
                    let body = Some(self.wrap(&t, d.body, false, span));
                    let o = self.new_opdef(
                        d.name,
                        DefKind::UserDefined,
                        d.params.clone(),
                        local,
                        body,
                        Some(module),
                        true,
                        span,
                        true,
                        Some(src),
                    );
                    self.set_labels_id(o, d.labels);
                    o
                }
            };
            if let Some(md) = self.module_mut(module) {
                md.definitions.push(new_odn);
            }
            self.set_op_def_node_recursion_fields(new_odn, module);
        }
        for ta in self.ctx_by(ictx, true) {
            let Some(d) = self.thmdef(ta).cloned() else {
                continue;
            };
            if d.local {
                continue;
            }
            let src = self.source_of(ta);
            let new_ta = if param_free {
                if local && top_level {
                    let n = self.new_thmdef(
                        d.name,
                        d.theorem,
                        d.body,
                        d.module,
                        true,
                        span,
                        d.params.clone(),
                        Some(imod),
                        Some(src),
                    );
                    if let Some(td) = self.thmdef_mut(n) {
                        td.local = local;
                    }
                    self.set_labels_id(n, d.labels);
                    n
                } else {
                    self.add_symbol(d.name, ta);
                    ta
                }
            } else {
                let orig_free = match d.module {
                    Some(om) => self.is_parameter_free(om),
                    None => true,
                };
                if orig_free {
                    if local && top_level {
                        let n = self.new_thmdef(
                            d.name,
                            d.theorem,
                            d.body,
                            Some(module),
                            true,
                            span,
                            d.params.clone(),
                            Some(imod),
                            Some(src),
                        );
                        if let Some(td) = self.thmdef_mut(n) {
                            td.local = true;
                        }
                        n
                    } else {
                        ta
                    }
                } else {
                    let body = Some(self.wrap(&t, d.body, true, span));
                    let n = self.new_thmdef(
                        d.name,
                        d.theorem,
                        body,
                        Some(module),
                        true,
                        span,
                        d.params.clone(),
                        Some(imod),
                        Some(src),
                    );
                    if let Some(td) = self.thmdef_mut(n) {
                        td.local = local;
                    }
                    self.set_labels_id(n, d.labels);
                    n
                }
            };
            if top_level && let Some(md) = self.module_mut(module) {
                md.definitions.push(new_ta);
            }
        }
        let inst = self.m.add(
            span,
            NodeKind::Instance(Box::new(InstanceData {
                name: None,
                local,
                params: Vec::new(),
                module: imod,
                substs: t.substs.clone(),
                step: None,
            })),
        );
        if top_level && let Some(md) = self.module_mut(module) {
            md.instances.push(inst);
            md.top_level.push(inst);
        }
        let _ = last_expr;
        Ok(inst)
    }
}
