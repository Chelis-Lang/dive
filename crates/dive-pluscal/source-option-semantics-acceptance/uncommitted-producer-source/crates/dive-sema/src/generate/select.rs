//! Names, qualified names and subexpression selectors: SANY's `Selector`,
//! `genIdToSelector` and `selectorToNode` (the latter an implementation of
//! the PlusCal algorithm in SANY's `Subexpression.tla`).

use super::cst::{op_args, toks};
use super::{GResult, Gen};
use crate::diagnostics::{Code, Span};
use crate::model::{CtxId, Key, Kind, Name, NodeId, NodeKind, SubstInData};
use dive_syntax::{SyntaxElement, SyntaxKind as K, SyntaxNode};

/// One element of a selector (SANY's `ops[i]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sel {
    /// `!n`.
    Num(i64),
    /// A name like `Foo`, an operator symbol, or a step number.
    Name,
    /// `!(...)`.
    Null,
    /// `!>>`.
    Gg,
    /// `!<<`.
    Ll,
    /// `!:`.
    Colon,
    /// `!@`.
    At,
}

/// A parsed general identifier: SANY's `Selector`.
#[derive(Debug, Clone)]
pub(crate) struct Selector {
    pub(crate) span: Option<Span>,
    pub(crate) ops: Vec<Sel>,
    pub(crate) names: Vec<String>,
    pub(crate) spans: Vec<Option<Span>>,
    pub(crate) args: Vec<Option<SyntaxNode>>,
}

/// `Generator.ArgNum`.
fn arg_num(op: Sel, arity: usize) -> Option<usize> {
    match op {
        Sel::Num(n) => {
            let n = usize::try_from(n).ok()?;
            (n >= 1 && n <= arity).then_some(n)
        }
        Sel::Ll => (arity > 0).then_some(1),
        Sel::Gg => (arity == 2).then_some(2),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    FindingOpName,
    FollowingLabels,
    FindingSubExpr,
    Start,
}

impl Gen<'_> {
    /// `Generator.genIdToSelector`. `whole` is the node whose span the
    /// selector reports (the `GENERAL_ID`, or the `OP_APPLICATION` around
    /// it), and `last_args` the application's arguments, if any.
    pub(crate) fn gen_id_to_selector(
        &mut self,
        gid: &SyntaxNode,
        whole: &SyntaxNode,
        last_args: Option<SyntaxNode>,
    ) -> GResult<Selector> {
        let mut sel = Selector {
            span: self.sp(whole),
            ops: Vec::new(),
            names: Vec::new(),
            spans: Vec::new(),
            args: Vec::new(),
        };
        if let Some(prefix) = gid.children().find(|c| c.kind() == K::ID_PREFIX) {
            for elem in prefix.children() {
                let parts: Vec<SyntaxElement> = elem
                    .children_with_tokens()
                    .filter(|e| !e.kind().is_trivia())
                    .collect();
                let Some(first) = parts.first() else { continue };
                let args = elem.children().find(|c| c.kind() == K::OP_ARGS);
                match first {
                    SyntaxElement::Node(n) if n.kind() == K::OP_ARGS => {
                        self.add_selector(&mut sel, first, Some(n.clone()))?;
                    }
                    SyntaxElement::Node(_) => self.add_selector(&mut sel, first, None)?,
                    SyntaxElement::Token(_) => self.add_selector(&mut sel, first, args)?,
                }
            }
        }
        let last = gid
            .children_with_tokens()
            .filter(|e| !e.kind().is_trivia() && e.kind() != K::ID_PREFIX)
            .last();
        let Some(last) = last else {
            return Err(self.abort(Code::SanyInternal, self.sp(gid), "empty name"));
        };
        match &last {
            SyntaxElement::Node(n) if n.kind() == K::OP_ARGS => {
                let a = Some(n.clone());
                self.add_selector(&mut sel, &last, a)?;
            }
            _ => self.add_selector(&mut sel, &last, None)?,
        }
        if let Some(a) = last_args
            && let Some(slot) = sel.args.last_mut()
        {
            *slot = Some(a);
        }
        Ok(sel)
    }

    /// `Selector.addSelector` and the classification of `Selector.finish`.
    fn add_selector(
        &mut self,
        sel: &mut Selector,
        part: &SyntaxElement,
        args: Option<SyntaxNode>,
    ) -> GResult<()> {
        let span = Some(Span {
            file: self.file,
            range: part.text_range(),
        });
        let (op, name) = match part {
            SyntaxElement::Token(t) => match t.kind() {
                K::STEP_IMPLICIT => (Sel::Name, self.corrected_step_name(t.text())),
                _ => (Sel::Name, t.text().to_owned()),
            },
            SyntaxElement::Node(n) if n.kind() == K::OP_ARGS => (Sel::Null, "N_OpArgs".to_owned()),
            SyntaxElement::Node(n) if n.kind() == K::STRUCT_OP => {
                if let Some(num) = n.children().find(|c| c.kind() == K::NUMBER_LIT) {
                    let text = toks(&num)
                        .first()
                        .map(|t| t.text().to_owned())
                        .unwrap_or_default();
                    match text.parse::<i32>() {
                        Ok(v) => (Sel::Num(i64::from(v)), "N_StructOp".to_owned()),
                        Err(_) => {
                            return Err(self.abort(
                                Code::BadSubexpression,
                                span,
                                format!("`!{text}` is not a valid subexpression number"),
                            ));
                        }
                    }
                } else {
                    let t = toks(n)
                        .first()
                        .map(|t| t.text().to_owned())
                        .unwrap_or_default();
                    let op = match t.as_str() {
                        ">>" | "⟩" => Sel::Gg,
                        "<<" | "⟨" => Sel::Ll,
                        ":" => Sel::Colon,
                        "@" => Sel::At,
                        _ => {
                            return Err(self.abort(
                                Code::SanyInternal,
                                span,
                                format!("unexpected selector `{t}`"),
                            ));
                        }
                    };
                    (op, t)
                }
            }
            SyntaxElement::Node(n) => {
                self.err(Code::BadUseItem, self.sp(n), "unexpected token found");
                (Sel::Name, String::new())
            }
        };
        sel.ops.push(op);
        sel.names.push(name);
        sel.spans.push(span);
        sel.args.push(args);
        Ok(())
    }

    fn selector_item_text(sel: &Selector, idx: usize) -> String {
        let n = sel.names.get(idx).cloned().unwrap_or_default();
        match n.as_str() {
            "N_StructOp" => match sel.ops.get(idx) {
                Some(Sel::Num(v)) => format!("argument selector {v}"),
                _ => n,
            },
            "N_OpArgs" => "!(...)".to_owned(),
            _ => n,
        }
    }

    fn selector_error(&mut self, sel: &Selector, idx: usize) {
        let t = Self::selector_item_text(sel, idx);
        self.err(
            Code::BadSubexpression,
            sel.spans.get(idx).copied().flatten(),
            format!("nonexistent operand specified by `{t}`"),
        );
    }

    fn bad(&mut self, sel: &Selector, idx: usize, msg: impl Into<String>) -> NodeId {
        self.err(
            Code::BadSubexpression,
            sel.spans.get(idx).copied().flatten(),
            msg,
        );
        self.null()
    }

    fn is_null(&self, n: Option<NodeId>) -> bool {
        n.is_none_or(|n| self.kind(n) == Kind::Error)
    }

    fn null_selection(&mut self, cur: Option<NodeId>, sel: &Selector, idx: usize) -> bool {
        if self.is_null(cur) {
            let t = Self::selector_item_text(sel, idx);
            self.err(
                Code::SanyInternal,
                sel.spans.get(idx).copied().flatten(),
                format!("an unexpected null node was selected by `{t}`, probably because of an earlier error"),
            );
            return true;
        }
        false
    }

    fn label_of(&self, node: NodeId, name: Name) -> Option<NodeId> {
        let lid = self.labels_id(node)?;
        self.m.labels(lid).get(&name).copied()
    }

    /// `Generator.illegalLabelRef`.
    fn illegal_label_ref(&mut self, label: NodeId, at: Option<Span>) -> GResult<bool> {
        let NodeKind::Label(ld) = &self.m.node(label).kind else {
            return Ok(false);
        };
        let Some(goal) = ld.goal else {
            return Ok(false);
        };
        let clause = usize::try_from(ld.goal_clause).unwrap_or(0);
        let body = self.thmdef(goal).and_then(|d| d.body);
        let suffices = self.thmdef(goal).is_some_and(|d| d.suffices);
        let Some(NodeKind::AssumeProve(ap)) = body.map(|b| &self.m.node(b).kind) else {
            return Err(self.abort(
                Code::SanyInternal,
                at,
                "expecting the label to be in an ASSUME/PROVE",
            ));
        };
        Ok(ap.in_scope_of_decl.get(clause).copied().unwrap_or(false) && suffices == ap.in_proof)
    }

    /// `Generator.illegalAPPosRef`.
    fn illegal_ap_pos_ref(&self, ap: NodeId, pos: usize) -> bool {
        let NodeKind::AssumeProve(d) = &self.m.node(ap).kind else {
            return false;
        };
        let in_scope = d.in_scope_of_decl.get(pos - 1).copied().unwrap_or(false);
        let goal_ok = match d.goal {
            None => true,
            Some(g) => self.thmdef(g).is_some_and(|t| t.suffices) == d.in_proof,
        };
        in_scope && goal_ok
    }

    fn body_of(&self, id: NodeId) -> Option<NodeId> {
        match &self.m.node(id).kind {
            NodeKind::OpDef(d) => d.body,
            NodeKind::ThmOrAssumpDef(d) => d.body,
            NodeKind::Label(d) => d.body,
            NodeKind::LetIn(d) => d.body,
            NodeKind::SubstIn(d) | NodeKind::ApSubstIn(d) => d.body,
            _ => None,
        }
    }

    fn params_of(&self, id: NodeId) -> Vec<NodeId> {
        match &self.m.node(id).kind {
            NodeKind::OpDef(d) => d.params.clone(),
            NodeKind::ThmOrAssumpDef(d) => d.params.clone(),
            NodeKind::Label(d) => d.params.clone(),
            _ => Vec::new(),
        }
    }

    /// `Generator.selectorToNode`. `expected` is the arity expected
    /// (0 for an expression, > 0 for an operator argument, < 0 for a `DEF`
    /// item); `is_fact` and `is_def` select the `USE`/`BY` variants.
    #[allow(
        clippy::too_many_lines,
        reason = "a direct port of SANY's selectorToNode"
    )]
    pub(crate) fn selector_to_node(
        &mut self,
        sel: &Selector,
        expected: i32,
        is_fact: bool,
        is_def: bool,
        module: NodeId,
    ) -> GResult<NodeId> {
        let len = sel.ops.len();
        let mut subst_prefix: Vec<NodeId> = Vec::new();
        let mut params: Vec<NodeId> = Vec::new();
        let mut all_args: Vec<NodeId> = Vec::new();
        let mut cur_name: Option<String> = None;
        let mut cur_name_span: Option<Span> = None;
        let mut new_name: Option<String> = None;
        let mut cur: Option<NodeId> = None;
        let mut idx = 0usize;
        let mut mode = Mode::FindingOpName;
        let mut prev_mode = Mode::Start;
        let mut let_ctx: Option<CtxId> = None;
        let mut arity_found: i32 = 0;
        let mut op_def_args: Vec<NodeId> = Vec::new();
        let mut first_finding = true;
        let mut sub_expr_of: Option<NodeId> = None;
        let mut in_ap_suffices = false;
        let at = |i: usize| sel.spans.get(i).copied().flatten();

        while idx < len {
            if ((sel.ops[idx] != Sel::Name && sel.ops[idx] != Sel::Null) || expected != 0)
                && sel.args[idx].is_some()
            {
                let t = Self::selector_item_text(sel, idx);
                return Ok(self.bad(
                    sel,
                    idx,
                    format!("selector `{t}` should not have arguments"),
                ));
            }
            match mode {
                Mode::FindingOpName | Mode::Start => {
                    let mut found: Option<NodeId> = None;
                    let mut temp_args: Vec<SyntaxNode> = Vec::new();
                    while found.is_none() && idx < len {
                        new_name = if sel.ops[idx] == Sel::Name {
                            let part = crate::builtins::resolve_synonym(&sel.names[idx]).to_owned();
                            Some(match &cur_name {
                                None => part,
                                Some(c) => format!("{c}!{part}"),
                            })
                        } else {
                            None
                        };
                        if cur_name.is_none() && sel.ops[idx] != Sel::Name {
                            if idx == 0 {
                                let t = sel.names[idx].clone();
                                return Ok(self.bad(
                                    sel,
                                    idx,
                                    format!("a name or step number is needed here, not `{t}`"),
                                ));
                            }
                            return Err(self.abort(
                                Code::SanyInternal,
                                at(idx),
                                "a name is needed here",
                            ));
                        }
                        if let Some(nn) = &new_name {
                            let name = self.m.names.get(nn);
                            found = name.and_then(|n| match let_ctx {
                                None => self.resolve_symbol(n),
                                Some(c) => self.m.context(c).get(Key::Sym(n)),
                            });
                        }
                        if found.is_none() {
                            if let Some(nn) = new_name.clone() {
                                cur_name_span = extend(cur_name_span, at(idx));
                                cur_name = Some(nn);
                            }
                            if let Some(a) = &sel.args[idx] {
                                temp_args.extend(op_args(a));
                            }
                            idx += 1;
                        }
                    }
                    let Some(sym) = found else {
                        let n = cur_name.clone().unwrap_or_default();
                        let help = undefined_help(&n);
                        self.err_help(
                            Code::UndefinedSymbol,
                            cur_name_span.or(sel.span),
                            format!("unknown operator `{n}`"),
                            help,
                        );
                        return Ok(self.null());
                    };
                    if self.kind(sym) == Kind::Module {
                        let t = sel.names[idx].clone();
                        return Ok(self.bad_code(
                            Code::ModuleNameAsOperator,
                            at(idx),
                            format!("module name `{t}` is not allowed here"),
                        ));
                    }
                    cur = Some(sym);
                    cur_name_span = extend(cur_name_span, at(idx));
                    cur_name.clone_from(&new_name);
                    let k = self.kind(sym);
                    let cn = cur_name.clone().unwrap_or_default();
                    match k {
                        Kind::ConstantDecl
                        | Kind::VariableDecl
                        | Kind::FormalParam
                        | Kind::BuiltIn
                        | Kind::NewConstant
                        | Kind::NewVariable
                        | Kind::NewState
                        | Kind::NewAction
                        | Kind::NewTemporal
                        | Kind::NumberedProofStep
                        | Kind::UserDefinedOp
                        | Kind::ThmOrAssumpDef
                        | Kind::ModuleInstance => {
                            let is_decl = !matches!(
                                k,
                                Kind::UserDefinedOp | Kind::ThmOrAssumpDef | Kind::ModuleInstance
                            );
                            if is_decl {
                                if idx != 0 {
                                    return Err(self.abort(
                                        Code::SanyInternal,
                                        sel.span,
                                        "impossible naming of a declaration",
                                    ));
                                } else if len != 1 {
                                    return Ok(self.bad(
                                        sel,
                                        idx,
                                        format!("cannot take a subexpression of `{cn}`"),
                                    ));
                                }
                            }
                            let node_arity = self.arity(sym);
                            if expected == 0 {
                                if let Some(a) = &sel.args[idx] {
                                    temp_args.extend(op_args(a));
                                }
                                let given = arity_found
                                    + i32::try_from(temp_args.len()).unwrap_or(i32::MAX);
                                if given != node_arity {
                                    let span =
                                        sel.args[idx].as_ref().map_or(sel.span, |a| self.sp(a));
                                    let need = node_arity - arity_found;
                                    self.err_help(
                                        Code::WrongArgumentCount,
                                        span,
                                        format!(
                                            "`{cn}` takes {need} argument{} but is given {}",
                                            if need == 1 { "" } else { "s" },
                                            temp_args.len()
                                        ),
                                        "pass exactly the operator's number of arguments",
                                    );
                                    return Ok(self.null());
                                }
                                for (i, a) in temp_args.iter().enumerate() {
                                    let pos = i + usize::try_from(arity_found).unwrap_or(0);
                                    let g =
                                        self.generate_expr_or_op_arg(sym, at(idx), pos, a, module)?;
                                    op_def_args.push(g);
                                }
                            }
                            arity_found = node_arity;
                            if k == Kind::ModuleInstance {
                                if idx == len - 1 && !(is_def || is_fact) {
                                    return Ok(self.bad_code(
                                        Code::IncompleteOperatorName,
                                        at(idx),
                                        format!("operator name `{cn}` is incomplete"),
                                    ));
                                }
                            } else {
                                if k == Kind::UserDefinedOp
                                    && !self.opdef(sym).is_some_and(|d| d.defined)
                                    && len != 1
                                {
                                    return Ok(self.bad(
                                        sel,
                                        idx,
                                        format!("a subexpression of `{cn}` is used inside the operator's own definition"),
                                    ));
                                }
                                if first_finding
                                    && matches!(k, Kind::UserDefinedOp | Kind::ThmOrAssumpDef)
                                {
                                    sub_expr_of = Some(sym);
                                }
                                if idx != len - 1 {
                                    params.extend(self.params_of(sym));
                                    let mut new_node = self.body_of(sym);
                                    cur_name = None;
                                    cur_name_span = None;
                                    mode = if sel.ops[idx + 1] == Sel::Name {
                                        Mode::FollowingLabels
                                    } else {
                                        Mode::FindingSubExpr
                                    };
                                    all_args.append(&mut op_def_args);
                                    arity_found = 0;
                                    if let Some(mut n) = new_node {
                                        while self.kind(n) == Kind::SubstIn {
                                            subst_prefix.push(n);
                                            match self.body_of(n) {
                                                Some(b) => n = b,
                                                None => break,
                                            }
                                        }
                                        while self.kind(n) == Kind::APSubstIn {
                                            subst_prefix.push(n);
                                            match self.body_of(n) {
                                                Some(b) => n = b,
                                                None => break,
                                            }
                                        }
                                        new_node = Some(n);
                                    }
                                    if mode == Mode::FindingSubExpr {
                                        cur = new_node;
                                    }
                                }
                            }
                            prev_mode = Mode::FindingOpName;
                        }
                        _ => {
                            return Err(self.abort(
                                Code::SanyInternal,
                                at(idx),
                                "unexpected node kind",
                            ));
                        }
                    }
                }
                Mode::FollowingLabels => {
                    let Some(c) = cur else {
                        return Err(self.abort(
                            Code::SanyInternal,
                            sel.span,
                            "no node for a label",
                        ));
                    };
                    let ck = self.kind(c);
                    let consistent = if prev_mode == Mode::FindingOpName {
                        matches!(ck, Kind::UserDefinedOp | Kind::ThmOrAssumpDef)
                    } else {
                        ck == Kind::Label
                    };
                    if !consistent {
                        return Err(self.abort(
                            Code::SanyInternal,
                            sel.span,
                            "unexpected node kind when following labels",
                        ));
                    }
                    let lname = self.m.names.get(&sel.names[idx]);
                    let Some(label) = lname.and_then(|n| self.label_of(c, n)) else {
                        let t = sel.names[idx].clone();
                        self.err(
                            Code::LabelNotFound,
                            at(idx),
                            format!("cannot find label `{t}`"),
                        );
                        return Ok(self.null());
                    };
                    cur = Some(label);
                    if self.illegal_label_ref(label, at(idx))? {
                        let t = sel.names[idx].clone();
                        return Ok(self.bad(
                            sel,
                            idx,
                            format!("the subexpression labelled `{t}` of an ASSUME/PROVE is used outside the scope of a declaration it is in"),
                        ));
                    }
                    let label_params = self.params_of(label);
                    if expected == 0 {
                        let given = sel.args[idx].as_ref().map_or(0, |a| op_args(a).len());
                        if label_params.len() != given {
                            let t = sel.names[idx].clone();
                            self.err(
                                Code::LabelArgumentCount,
                                at(idx),
                                format!("label `{t}` is used with the wrong number of arguments"),
                            );
                            return Ok(self.null());
                        }
                        if let Some(a) = &sel.args[idx] {
                            for e in op_args(a).iter().take(label_params.len()) {
                                let g = self.generate_expression(e, module)?;
                                all_args.push(g);
                            }
                        }
                    }
                    params.extend(label_params);
                    if idx < len - 1 && sel.ops[idx + 1] != Sel::Name {
                        mode = Mode::FindingSubExpr;
                    }
                    if mode == Mode::FindingSubExpr {
                        cur = self.body_of(label);
                    }
                    prev_mode = Mode::FollowingLabels;
                }
                Mode::FindingSubExpr => {
                    let op = sel.ops[idx];
                    if op == Sel::Colon {
                        let ok = (idx == len - 1 && prev_mode == Mode::FindingOpName)
                            || (idx < len - 1 && sel.ops[idx + 1] == Sel::Name);
                        if prev_mode == Mode::FindingSubExpr || !ok {
                            return Ok(self.bad(
                                sel,
                                idx,
                                "`!:` can be used only after a name, either at the end after an operator name or before an operator name",
                            ));
                        }
                    } else if self.is_null(cur) {
                        return Ok(self.bad(
                            sel,
                            idx,
                            "subexpression selection failed, probably because of an error in the selected expression",
                        ));
                    } else {
                        let c = cur.unwrap_or_else(|| self.null());
                        match self.kind(c) {
                            Kind::LetIn => {
                                if arg_num(op, 1) == Some(1) {
                                    cur = self.body_of(c);
                                } else {
                                    return Ok(self.bad(
                                        sel,
                                        idx,
                                        "a LET/IN expression has only one operand",
                                    ));
                                }
                            }
                            Kind::OpAppl => match self.select_in_appl(
                                c,
                                sel,
                                &mut idx,
                                &mut params,
                                &mut all_args,
                                module,
                            )? {
                                Some(n) => cur = Some(n),
                                None => return Ok(self.null()),
                            },
                            Kind::AssumeProve => {
                                let (suffices, assumes, prove) = match &self.m.node(c).kind {
                                    NodeKind::AssumeProve(d) => {
                                        (d.suffices, d.assumes.clone(), d.prove)
                                    }
                                    _ => (false, Vec::new(), None),
                                };
                                if suffices && !in_ap_suffices {
                                    if arg_num(op, 1) != Some(1) {
                                        return Ok(self.bad(
                                            sel,
                                            idx,
                                            "accessing a nonexistent subexpression of a SUFFICES",
                                        ));
                                    }
                                    in_ap_suffices = true;
                                } else {
                                    in_ap_suffices = false;
                                    let Some(temp) = arg_num(op, 1 + assumes.len()) else {
                                        self.selector_error(sel, idx);
                                        return Ok(self.null());
                                    };
                                    if self.illegal_ap_pos_ref(c, temp) {
                                        return Ok(self.bad(
                                            sel,
                                            idx,
                                            "an ASSUME/PROVE clause is accessed outside the scope of a declaration it is in",
                                        ));
                                    }
                                    if temp <= assumes.len() {
                                        cur = assumes.get(temp - 1).copied().flatten();
                                        if self.null_selection(cur, sel, idx) {
                                            return Ok(self.null());
                                        }
                                        if cur.is_some_and(|n| self.kind(n) == Kind::NewSymb)
                                            && idx != len - 1
                                        {
                                            return Ok(self.bad(sel, idx, "a subexpression of a NEW clause of an ASSUME is selected"));
                                        }
                                    } else {
                                        cur = prove;
                                    }
                                }
                            }
                            Kind::OpArg => {
                                let op_node = match &self.m.node(c).kind {
                                    NodeKind::OpArg(d) => d.op,
                                    _ => self.null(),
                                };
                                let is_lambda = self.kind(op_node) == Kind::UserDefinedOp
                                    && self.sym_text(op_node) == "LAMBDA";
                                if !is_lambda {
                                    return Ok(self.bad(
                                        sel,
                                        idx,
                                        "trying to select a subexpression of an operator argument",
                                    ));
                                }
                                if op != Sel::Null && op != Sel::At {
                                    let t = sel.names[idx].clone();
                                    return Ok(self.bad(sel, idx, format!("cannot use `!{t}` to select a subexpression of a LAMBDA")));
                                }
                                let lparams = self.params_of(op_node);
                                if op == Sel::Null {
                                    let given =
                                        sel.args[idx].as_ref().map(op_args).unwrap_or_default();
                                    if lparams.len() != given.len() {
                                        return Ok(self.bad(
                                            sel,
                                            idx,
                                            format!(
                                                "a selector with {} arguments is used for a LAMBDA taking {}",
                                                given.len(),
                                                lparams.len()
                                            ),
                                        ));
                                    }
                                    for e in &given {
                                        let g = self.generate_expression(e, module)?;
                                        all_args.push(g);
                                    }
                                }
                                params.extend(lparams);
                                cur = self.body_of(op_node);
                            }
                            Kind::UserDefinedOp | Kind::BuiltIn | Kind::NumberedProofStep => {
                                return Err(self.abort(
                                    Code::SanyInternal,
                                    at(idx),
                                    "this node cannot be selected",
                                ));
                            }
                            Kind::AtNode
                            | Kind::Decimal
                            | Kind::Numeral
                            | Kind::String
                            | Kind::FormalParam
                            | Kind::ConstantDecl
                            | Kind::VariableDecl
                            | Kind::BoundSymbol => {
                                return Ok(self.bad(
                                    sel,
                                    idx,
                                    "selecting a subexpression of an expression that has none",
                                ));
                            }
                            Kind::Label => {
                                cur = self.body_of(c);
                                // Re-examine this selector against the label's body.
                                prev_mode = Mode::FindingSubExpr;
                                if self.null_selection(cur, sel, idx) {
                                    return Ok(self.null());
                                }
                                continue;
                            }
                            _ => {
                                return Err(self.abort(
                                    Code::SanyInternal,
                                    at(idx),
                                    "unknown node kind",
                                ));
                            }
                        }
                    }
                    if self.null_selection(cur, sel, idx) {
                        return Ok(self.null());
                    }
                    if idx != len - 1 {
                        if sel.ops[idx + 1] == Sel::Name {
                            while let Some(c) = cur
                                && self.kind(c) == Kind::Label
                            {
                                cur = self.body_of(c);
                                if self.null_selection(cur, sel, idx) {
                                    return Ok(self.null());
                                }
                            }
                            if let Some(c) = cur
                                && let NodeKind::LetIn(d) = &self.m.node(c).kind
                            {
                                let_ctx = Some(d.ctx);
                                mode = Mode::FindingOpName;
                                first_finding = false;
                            } else {
                                return Ok(self.bad(
                                    sel,
                                    idx,
                                    "a name selector here must name a definition of a LET",
                                ));
                            }
                        } else if sel.ops[idx + 1] == Sel::Colon {
                            return Ok(self.bad(
                                sel,
                                idx,
                                "`!:` should not follow an operand selector",
                            ));
                        }
                    }
                    prev_mode = Mode::FindingSubExpr;
                }
            }
            idx += 1;
        }

        let Some(c) = cur else {
            return Ok(self.null());
        };
        let ck = self.kind(c);
        if ck == Kind::AssumeProve {
            if is_fact {
                return Ok(c);
            }
            self.err(
                Code::BadSubexpression,
                sel.span,
                "an ASSUME/PROVE is selected instead of an expression",
            );
            return Ok(self.null());
        }
        if ck == Kind::NewSymb {
            if is_fact {
                return Ok(c);
            }
            self.err(
                Code::BadSubexpression,
                sel.span,
                "a NEW declaration is selected as an expression or operator",
            );
            return Ok(self.null());
        }
        if expected < 0 {
            let ok_kind = matches!(
                ck,
                Kind::UserDefinedOp | Kind::ThmOrAssumpDef | Kind::NumberedProofStep
            ) || (ck == Kind::ModuleInstance && is_def);
            if prev_mode != Mode::FindingOpName || !ok_kind {
                self.err_help(
                    Code::DefNotDefinition,
                    sel.span,
                    "a DEF item must name a defined operator",
                    "list operator definitions after DEF",
                );
                return Ok(self.null());
            }
            if ck == Kind::NumberedProofStep {
                let step_kind = self.opdef(c).and_then(|d| d.step).map(|s| self.kind(s));
                if step_kind != Some(Kind::DefStep) {
                    self.err(
                        Code::StepMisuse,
                        sel.span,
                        "a DEF item refers to a step that is not a definition",
                    );
                    return Ok(self.null());
                }
            }
            return Ok(c);
        }
        if ck == Kind::NumberedProofStep {
            let msg = if is_fact {
                "the number of a step that is not a fact is used as a fact"
            } else {
                "the number of a step that is not an expression is used as an expression"
            };
            self.err(Code::StepMisuse, sel.span, msg);
            return Ok(self.null());
        }
        if expected > 0 {
            let mut temp = i32::try_from(params.len()).unwrap_or(i32::MAX);
            if ck == Kind::OpArg || prev_mode == Mode::FindingOpName {
                temp += self.arity(c);
            }
            if expected != temp {
                self.err(
                    Code::OperatorArgumentArity,
                    sel.span,
                    format!(
                        "an operator of arity {expected} is required, but this one has arity {temp}"
                    ),
                );
                return Ok(self.null());
            }
        }
        let mut cur = c;
        if prev_mode == Mode::FindingOpName && params.len() + subst_prefix.len() > 0 {
            let node_params = match self.kind(cur) {
                Kind::UserDefinedOp | Kind::ThmOrAssumpDef => self.params_of(cur),
                _ => {
                    return Err(self.abort(
                        Code::SanyInternal,
                        sel.spans.last().copied().flatten(),
                        "unexpected node kind after finding an operator name",
                    ));
                }
            };
            all_args.extend(op_def_args.iter().copied());
            let mut temp = Vec::new();
            for pm in node_params {
                let (pname, parity) = match &self.m.node(pm).kind {
                    NodeKind::FormalParam(d) => (d.name, d.arity),
                    _ => (None, 0),
                };
                let pspan = self.m.node(pm).span;
                let newpm = self.new_param(pname, parity, pspan, false, Some(module));
                let eoag = if parity == 0 {
                    self.appl_match(newpm, Vec::new(), sel.span)?
                } else {
                    self.new_op_arg(newpm, sel.span)
                };
                temp.push(Some(eoag));
                params.push(newpm);
            }
            cur = self.appl_match(cur, temp, sel.span)?;
        }
        if self.kind(cur) == Kind::OpArg {
            if expected == 0 {
                self.err(
                    Code::BadSubexpression,
                    sel.span,
                    "an operator argument is selected where an expression is expected",
                );
                return Ok(self.null());
            }
            let oa = self.arity(cur);
            let temp = i32::try_from(params.len()).unwrap_or(i32::MAX) + oa;
            if expected != temp {
                self.err(
                    Code::BadSubexpression,
                    sel.span,
                    format!("an operator of arity {expected} is expected, but the selected operator has arity {temp}"),
                );
                return Ok(self.null());
            }
            if params.len() + subst_prefix.len() > 0 {
                for i in 0..oa {
                    let n = self.intern(&format!("NewParam{i}"));
                    let p = self.new_param(Some(n), 0, None, false, Some(module));
                    params.push(p);
                }
                let op = match &self.m.node(cur).kind {
                    NodeKind::OpArg(d) => d.op,
                    _ => self.null(),
                };
                let args = op_def_args.iter().map(|a| Some(*a)).collect();
                cur = self.appl_match(op, args, sel.span)?;
            }
        }
        let ck = self.kind(cur);
        if !is_fact {
            let thm_ap = ck == Kind::ThmOrAssumpDef
                && self
                    .body_of(cur)
                    .is_some_and(|b| self.kind(b) == Kind::AssumeProve);
            let label_ap =
                matches!(&self.m.node(cur).kind, NodeKind::Label(d) if d.is_assume_prove);
            if thm_ap || label_ap {
                self.err(
                    Code::AssumeProveAsExpression,
                    sel.span,
                    "an ASSUME/PROVE is used where an expression is required",
                );
                return Ok(self.null());
            }
            if ck == Kind::ThmOrAssumpDef
                && let Some(b) = self.body_of(cur)
                && let Some(name) = self.appl_operator_name(b)
            {
                let what = match name.as_str() {
                    "$Qed" => Some("QED"),
                    "$Pfcase" => Some("CASE"),
                    "$Have" => Some("HAVE"),
                    "$Take" => Some("TAKE"),
                    "$Pick" => Some("PICK"),
                    "$Witness" => Some("WITNESS"),
                    "$Suffices" => Some("SUFFICES"),
                    _ => None,
                };
                if let Some(w) = what {
                    self.err(
                        Code::ProofStepAsExpression,
                        sel.span,
                        format!("a {w} proof step is selected instead of an expression"),
                    );
                    return Ok(self.null());
                }
            }
        }
        if matches!(
            ck,
            Kind::UserDefinedOp
                | Kind::ConstantDecl
                | Kind::VariableDecl
                | Kind::FormalParam
                | Kind::BuiltIn
                | Kind::BoundSymbol
                | Kind::ThmOrAssumpDef
                | Kind::NewConstant
                | Kind::NewVariable
                | Kind::NewState
                | Kind::NewAction
                | Kind::NewTemporal
        ) {
            if expected > 0 {
                return Ok(self.new_op_arg(cur, sel.span));
            }
            let args = op_def_args.iter().map(|a| Some(*a)).collect();
            let oan = self.appl_match(cur, args, sel.span)?;
            if let Some(d) = self.appl_data_mut(oan) {
                d.subexpression_of = sub_expr_of;
            }
            return Ok(oan);
        }
        if ck == Kind::OpArg {
            return Ok(cur);
        }
        if ck == Kind::ModuleInstance {
            if is_fact || is_def {
                return Ok(cur);
            }
            self.err(
                Code::IncompleteOperatorName,
                sel.span,
                "a module instance is selected instead of an expression",
            );
            return Ok(self.null());
        }
        if !matches!(
            ck,
            Kind::OpAppl
                | Kind::LetIn
                | Kind::Numeral
                | Kind::Decimal
                | Kind::String
                | Kind::AtNode
                | Kind::Label
                | Kind::SubstIn
        ) {
            return Err(self.abort(Code::SanyInternal, sel.span, "expected an expression node"));
        }
        let mut expr = cur;
        for &s in subst_prefix.iter().rev() {
            let (substs, instantiating, instantiated) = match &self.m.node(s).kind {
                NodeKind::SubstIn(d) | NodeKind::ApSubstIn(d) => {
                    (d.substs.clone(), d.instantiating, d.instantiated)
                }
                _ => continue,
            };
            let span = self.m.node(s).span;
            expr = self.m.add(
                span,
                NodeKind::SubstIn(Box::new(SubstInData {
                    substs,
                    body: Some(expr),
                    instantiating,
                    instantiated,
                })),
            );
        }
        let lambda = if params.is_empty() {
            None
        } else {
            let n = self.intern("LAMBDA");
            Some(self.new_opdef(
                n,
                crate::model::DefKind::UserDefined,
                params.clone(),
                false,
                Some(expr),
                Some(module),
                false,
                sel.span,
                true,
                None,
            ))
        };
        if expected > 0 {
            if i32::try_from(params.len()).unwrap_or(i32::MAX) != expected {
                self.err(
                    Code::OperatorArgumentArity,
                    sel.span,
                    format!("an operator argument of arity {expected} is expected, but this one has arity {}", params.len()),
                );
                return Ok(self.null());
            }
            let l = lambda.unwrap_or_else(|| self.null());
            return Ok(self.new_op_arg(l, sel.span));
        }
        if params.len() != all_args.len() {
            self.err(
                Code::WrongArgumentCount,
                sel.span,
                format!(
                    "{} arguments are expected but {} are given",
                    params.len(),
                    all_args.len()
                ),
            );
            return Ok(self.null());
        }
        let Some(l) = lambda else {
            let ln = self.appl_builtin("$Nop", vec![Some(expr)], sel.span);
            if let Some(d) = self.appl_data_mut(ln) {
                d.subexpression_of = sub_expr_of;
            }
            return Ok(ln);
        };
        let args = all_args.iter().map(|a| Some(*a)).collect();
        let oan = self.appl_match(l, args, sel.span)?;
        if let Some(d) = self.appl_data_mut(oan) {
            d.subexpression_of = sub_expr_of;
        }
        Ok(oan)
    }

    fn bad_code(&mut self, code: Code, at: Option<Span>, msg: String) -> NodeId {
        self.err(code, at, msg);
        self.null()
    }

    /// The `OpApplKind` case of `selectorToNode`'s `FindingSubExpr` mode.
    /// Returns the selected node, or `None` after reporting an error.
    #[allow(clippy::too_many_lines, reason = "mirrors SANY's case analysis")]
    fn select_in_appl(
        &mut self,
        c: NodeId,
        sel: &Selector,
        idx: &mut usize,
        params: &mut Vec<NodeId>,
        all_args: &mut Vec<NodeId>,
        module: NodeId,
    ) -> GResult<Option<NodeId>> {
        let Some(d) = self.appl_data(c).cloned() else {
            return Ok(None);
        };
        let op = sel.ops[*idx];
        let at = |i: usize| sel.spans.get(i).copied().flatten();
        let opk = self.kind(d.operator);
        let opname = self.sym_text(d.operator);
        let args = d.operands.clone();
        if matches!(
            opk,
            Kind::FormalParam | Kind::ConstantDecl | Kind::UserDefinedOp
        ) {
            let ar = usize::try_from(self.arity(d.operator)).unwrap_or(0);
            let Some(t) = arg_num(op, ar) else {
                self.selector_error(sel, *idx);
                return Ok(None);
            };
            return Ok(args.get(t - 1).copied().flatten().or(Some(self.null())));
        }
        if opk != Kind::BuiltIn {
            let t = Self::selector_item_text(sel, *idx);
            self.err(
                Code::BadSubexpression,
                at(*idx),
                format!("choosing operand `{t}` of a subexpression with no operands"),
            );
            return Ok(None);
        }
        match opname.as_str() {
            "$RcdConstructor" | "$SetOfRcds" => {
                let Some(t) = arg_num(op, args.len()) else {
                    self.selector_error(sel, *idx);
                    return Ok(None);
                };
                let pair = args.get(t - 1).copied().flatten();
                let pd = pair.and_then(|p| self.appl_data(p)).cloned();
                match pd {
                    Some(pd) if self.sym_text(pd.operator) == "$Pair" => {
                        Ok(pd.operands.get(1).copied().flatten().or(Some(self.null())))
                    }
                    _ => Err(self.abort(Code::SanyInternal, at(*idx), "expected a $Pair")),
                }
            }
            "$Case" => {
                if *idx == sel.ops.len() - 1 {
                    self.err(
                        Code::BadSubexpression,
                        at(*idx),
                        "a subexpression of CASE must have the form `!i!j`",
                    );
                    return Ok(None);
                }
                let Some(t) = arg_num(op, args.len()) else {
                    self.selector_error(sel, *idx);
                    return Ok(None);
                };
                let pair = args.get(t - 1).copied().flatten();
                let Some(pd) = pair.and_then(|p| self.appl_data(p)).cloned() else {
                    return Err(self.abort(Code::SanyInternal, at(*idx), "expected a $Pair"));
                };
                if self.sym_text(pd.operator) != "$Pair" {
                    return Err(self.abort(Code::SanyInternal, at(*idx), "expected a $Pair"));
                }
                *idx += 1;
                let Some(t2) = arg_num(sel.ops[*idx], 2) else {
                    self.err(
                        Code::BadSubexpression,
                        at(*idx),
                        "the second selector of a CASE subexpression must choose one of two operands",
                    );
                    return Ok(None);
                };
                match pd.operands.get(t2 - 1).copied().flatten() {
                    Some(n) => Ok(Some(n)),
                    None => {
                        self.err(
                            Code::BadSubexpression,
                            at(*idx),
                            "selecting OTHER in a CASE expression",
                        );
                        Ok(None)
                    }
                }
            }
            "$Except" => {
                let Some(t) = arg_num(op, args.len()) else {
                    self.selector_error(sel, *idx);
                    return Ok(None);
                };
                if t > 1 {
                    self.err(
                        Code::BadSubexpression,
                        at(*idx),
                        "selecting a subexpression of an EXCEPT is not implemented",
                    );
                    return Ok(None);
                }
                let n = args.get(t - 1).copied().flatten();
                if self.null_selection(n, sel, *idx) {
                    return Ok(None);
                }
                Ok(n)
            }
            _ => {
                let nbounded = super::expr::number_of_bounded_bound_symbols(&d);
                let unbounded_empty = d.unbounded.as_ref().is_none_or(Vec::is_empty);
                if nbounded == 0 && unbounded_empty {
                    let Some(t) = arg_num(op, args.len()) else {
                        self.selector_error(sel, *idx);
                        return Ok(None);
                    };
                    return Ok(args.get(t - 1).copied().flatten().or(Some(self.null())));
                }
                if op == Sel::Null || op == Sel::At {
                    let temp: Vec<NodeId> = if nbounded > 0 {
                        d.bounded
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .flatten()
                            .collect()
                    } else {
                        d.unbounded.clone().unwrap_or_default()
                    };
                    params.extend(temp.iter().copied());
                    if op == Sel::Null {
                        let given = sel.args[*idx].as_ref().map(op_args).unwrap_or_default();
                        if temp.len() != given.len() {
                            self.err(
                                Code::BadSubexpression,
                                at(*idx),
                                format!(
                                    "a selector with {} arguments is used for a quantifier with {} bound identifiers",
                                    given.len(),
                                    temp.len()
                                ),
                            );
                            return Ok(None);
                        }
                        for e in &given {
                            let g = self.generate_expression(e, module)?;
                            all_args.push(g);
                        }
                    }
                    return Ok(args.first().copied().flatten().or(Some(self.null())));
                }
                let Some(t) = arg_num(op, d.ranges.len()) else {
                    self.selector_error(sel, *idx);
                    return Ok(None);
                };
                Ok(d.ranges.get(t - 1).copied())
            }
        }
    }
}

/// `Generator.extendedBy`: a span covering both.
fn extend(a: Option<Span>, b: Option<Span>) -> Option<Span> {
    match (a, b) {
        (None, b) => b,
        (Some(a), Some(b)) if a.file == b.file => Some(Span {
            file: a.file,
            range: a.range.cover(b.range),
        }),
        (a, _) => a,
    }
}

/// A suggestion for an unknown name.
fn undefined_help(name: &str) -> String {
    let std = match name {
        "Nat" | "Int" | "Real" => Some("Naturals, Integers or Reals"),
        "Seq" | "Len" | "Append" | "Head" | "Tail" | "SubSeq" | "SelectSeq" => Some("Sequences"),
        "Cardinality" | "IsFiniteSet" => Some("FiniteSets"),
        "Print" | "PrintT" | "Assert" | "ToString" | "TLCGet" | "TLCSet" | "Permutations"
        | "SortSeq" | "RandomElement" | "JavaTime" => Some("TLC"),
        _ => None,
    };
    match std {
        Some(m) => format!("`{name}` is defined in the standard module {m}; add it to EXTENDS"),
        None => format!("define `{name}`, declare it, or extend the module that defines it"),
    }
}
