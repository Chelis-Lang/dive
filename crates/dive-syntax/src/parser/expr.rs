//! Expressions: SANY's `Expression`, `ExtendableExpr`, `PrimitiveExp` and
//! the bracketed and "open" forms, plus the operator stack of
//! `OperatorStack.java`.
//!
//! Operators are pushed on a stack and reduced by comparing precedence
//! *ranges*. Where neither operator's range lies wholly above the other's,
//! SANY reports a precedence conflict, and so does this port. Prefix `-`
//! is pushed as the infix operator `-` and recognised as prefix during
//! reduction, as in SANY.

use super::{
    PErr, PResult, Parser, VPos, is_infix_tok, is_ne_prefix_tok, is_open_start, is_prefix_tok,
    starts_expr, starts_op_or_expr,
};
use crate::diagnostics::Code;
#[allow(
    clippy::enum_glob_use,
    reason = "the grammar names over a hundred token and node kinds"
)]
use crate::kind::SyntaxKind::{self, *};
use crate::ops::{self, Assoc, OpInfo, VOID};
use crate::tree::{Child, Raw};

/// What an operator-stack element holds.
#[derive(Debug)]
enum ENode {
    /// A complete expression.
    Expr(Raw),
    /// An operator token (prefix, infix or postfix).
    Op(u32),
    /// The `[args]` of a function application, waiting for its function.
    Fcn(Raw),
    /// SANY's `VoidOperator`, pushed to force the final reduction.
    Void,
}

#[derive(Debug)]
struct Elem {
    node: ENode,
    op: Option<&'static OpInfo>,
    /// Position for error messages.
    at: VPos,
}

type Stack = Vec<Elem>;

fn into_child(n: ENode) -> Option<Child> {
    match n {
        ENode::Expr(r) | ENode::Fcn(r) => Some(Child::Node(r)),
        ENode::Op(t) => Some(Child::Tok(t)),
        ENode::Void => None,
    }
}

fn is_void(o: &OpInfo) -> bool {
    o.canonical == VOID.canonical
}

/// `IDENT_TUPLE` as an expression: a `TUPLE` of names.
fn ident_tuple_to_tuple(t: Raw) -> Raw {
    let mut out = Raw::new(TUPLE);
    let mut t = t;
    let children = std::mem::take(&mut t.children);
    let last = children.len().saturating_sub(1);
    for (i, c) in children.into_iter().enumerate() {
        match c {
            Child::Tok(tok) if i != 0 && i != last && i % 2 == 1 => {
                let mut g = Raw::new(GENERAL_ID);
                g.tok(tok);
                out.node(g);
            }
            other => out.children.push(other),
        }
    }
    out
}

impl Parser<'_> {
    /// Consumes an operator token and records it as SANY's `lastOp`.
    pub(super) fn op_token(&mut self) -> u32 {
        let i = self.bump();
        self.last_op = ops::lookup(self.text(i));
        i
    }

    // ---- Operator stack (OperatorStack.java) --------------------------------

    fn push_op(&mut self, st: &mut Stack, tok: u32, at: VPos) -> PResult<()> {
        let op = ops::lookup(self.text(tok));
        st.push(Elem {
            node: ENode::Op(tok),
            op,
            at,
        });
        if st.len() != 1 {
            self.reduce_stack(st)?;
        }
        Ok(())
    }

    fn push_expr(st: &mut Stack, r: Raw, at: VPos) {
        st.push(Elem {
            node: ENode::Expr(r),
            op: None,
            at,
        });
    }

    /// `reduceInfix`: `[.., l, op, r, top]` becomes `[.., (l op r), top]`.
    fn reduce_infix(st: &mut Stack, op: &OpInfo) {
        let n = st.len() - 1;
        if n < 3 {
            return;
        }
        let top = st.pop();
        let right = st.pop();
        let opn = st.pop();
        let left = st.pop();
        let (Some(top), Some(right), Some(opn), Some(left)) = (top, right, opn, left) else {
            return;
        };
        let at = left.at;
        let node = if op.fixity == ops::Fixity::Nfix {
            match left.node {
                ENode::Expr(mut l) if l.kind == CARTESIAN_PRODUCT => {
                    l.children.extend(into_child(opn.node));
                    l.children.extend(into_child(right.node));
                    l
                }
                other => {
                    let mut c = Raw::new(CARTESIAN_PRODUCT);
                    c.children.extend(into_child(other));
                    c.children.extend(into_child(opn.node));
                    c.children.extend(into_child(right.node));
                    c
                }
            }
        } else {
            let mut c = Raw::new(INFIX_EXPR);
            c.children.extend(into_child(left.node));
            c.children.extend(into_child(opn.node));
            c.children.extend(into_child(right.node));
            c
        };
        st.push(Elem {
            node: ENode::Expr(node),
            op: None,
            at,
        });
        st.push(top);
    }

    /// `reducePrefix`: `[.., op, e, top]` becomes `[.., (op e), top]`.
    fn reduce_prefix(st: &mut Stack) {
        let n = st.len() - 1;
        if n < 2 {
            return;
        }
        let top = st.pop();
        let e = st.pop();
        let opn = st.pop();
        let (Some(top), Some(e), Some(opn)) = (top, e, opn) else {
            return;
        };
        let at = opn.at;
        let mut c = Raw::new(PREFIX_EXPR);
        c.children.extend(into_child(opn.node));
        c.children.extend(into_child(e.node));
        st.push(Elem {
            node: ENode::Expr(c),
            op: None,
            at,
        });
        st.push(top);
    }

    /// `reducePostfix`: `[.., e, op, top]` becomes `[.., (e op), top]`.
    /// Function application is a postfix operator whose node is `[args]`.
    fn reduce_postfix(st: &mut Stack) {
        let n = st.len() - 1;
        if n < 2 {
            return;
        }
        let top = st.pop();
        let opn = st.pop();
        let e = st.pop();
        let (Some(top), Some(opn), Some(e)) = (top, opn, e) else {
            return;
        };
        let at = e.at;
        let node = match opn.node {
            ENode::Fcn(mut args) => {
                let mut c = Raw::new(FCN_APPL);
                c.children.extend(into_child(e.node));
                c.children.append(&mut args.children);
                c
            }
            other => {
                let mut c = Raw::new(POSTFIX_EXPR);
                c.children.extend(into_child(e.node));
                c.children.extend(into_child(other));
                c
            }
        };
        st.push(Elem {
            node: ENode::Expr(node),
            op: None,
            at,
        });
        st.push(top);
    }

    fn op_name(o: &OpInfo) -> &'static str {
        o.canonical
    }

    fn conflict(&self, at: VPos, l: &OpInfo, r: &OpInfo) -> PErr {
        self.err_at(
            at,
            Code::PrecedenceConflict,
            format!(
                "precedence conflict between operators `{}` and `{}`",
                Self::op_name(l),
                Self::op_name(r)
            ),
        )
        .with_help("add parentheses to say which operator applies first")
    }

    /// SANY's `reduceStack`, case by case.
    #[allow(
        clippy::too_many_lines,
        reason = "one function mirrors SANY's reduceStack case analysis, so the two can be compared"
    )]
    fn reduce_stack(&mut self, st: &mut Stack) -> PResult<()> {
        loop {
            let Some(n) = st.len().checked_sub(1) else {
                return Ok(());
            };
            let Some(o_r) = st[n].op else {
                return Ok(());
            };
            let at_r = st[n].at;
            if o_r.is_postfix() {
                if n == 0 {
                    return Err(self.err_at(
                        at_r,
                        Code::MisplacedOperator,
                        format!("postfix operator `{}` has no operand", o_r.canonical),
                    ));
                }
                if let Some(o_l) = st[n - 1].op {
                    if o_l.is_infix() || o_l.is_prefix() {
                        return Err(self.err_at(
                            at_r,
                            Code::MisplacedOperator,
                            format!(
                                "postfix operator `{}` follows operator `{}`",
                                o_r.canonical, o_l.canonical
                            ),
                        ));
                    }
                    Self::reduce_postfix(st);
                } else if n > 1 {
                    if let Some(o_l) = st[n - 2].op {
                        if o_l.succ(o_r) {
                            if o_l.is_infix() {
                                Self::reduce_infix(st, o_l);
                            } else {
                                Self::reduce_prefix(st);
                            }
                        } else if !o_l.prec(o_r) {
                            return Err(self.conflict(at_r, o_l, o_r));
                        } else {
                            return Ok(());
                        }
                    } else {
                        return Err(self.err_at(
                            st[n - 1].at,
                            Code::MissingOperator,
                            "two expressions follow each other without an operator between them",
                        ));
                    }
                } else {
                    return Ok(());
                }
            } else if o_r.is_prefix() {
                if n == 0 {
                    return Ok(());
                }
                match st[n - 1].op {
                    Some(o_l) if o_l.is_postfix() => {
                        return Err(self.err_at(
                            at_r,
                            Code::MisplacedOperator,
                            format!(
                                "prefix operator `{}` follows postfix operator `{}`",
                                o_r.canonical, o_l.canonical
                            ),
                        ));
                    }
                    Some(_) => return Ok(()),
                    None => {
                        return Err(self.err_at(
                            at_r,
                            Code::MisplacedOperator,
                            format!("prefix operator `{}` follows an expression", o_r.canonical),
                        ));
                    }
                }
            } else if n == 0 {
                // Infix operator on an empty stack: fine only if it has a
                // prefix form (`-`).
                if ops::mixfix(o_r).is_none() {
                    return Err(if is_void(o_r) {
                        self.err_at(at_r, Code::MissingExpression, "expected an expression")
                    } else {
                        self.err_at(
                            at_r,
                            Code::MisplacedOperator,
                            format!("infix operator `{}` has no left operand", o_r.canonical),
                        )
                    });
                }
                return Ok(());
            } else if let Some(o_l) = st[n - 1].op {
                if o_l.is_infix() || o_l.is_prefix() {
                    match ops::mixfix(o_r) {
                        None => {
                            return Err(if is_void(o_r) {
                                self.err_at(
                                    at_r,
                                    Code::MissingExpression,
                                    format!(
                                        "expected an expression after operator `{}`",
                                        o_l.canonical
                                    ),
                                )
                            } else {
                                self.err_at(
                                    at_r,
                                    Code::MisplacedOperator,
                                    format!(
                                        "infix operator `{}` follows operator `{}`",
                                        o_r.canonical, o_l.canonical
                                    ),
                                )
                            });
                        }
                        Some(mix_r) => {
                            if o_l.succ(mix_r) || (o_l.same(mix_r) && o_l.assoc == Assoc::Left) {
                                return Err(self.conflict(at_r, o_l, mix_r));
                            }
                        }
                    }
                } else {
                    Self::reduce_postfix(st);
                }
            } else if n > 1 {
                let Some(mut o_l) = st[n - 2].op else {
                    return Err(self.err_at(
                        st[n - 1].at,
                        Code::MissingOperator,
                        "two expressions follow each other without an operator between them",
                    ));
                };
                if let Some(mix_l) = ops::mixfix(o_l)
                    && (n == 2 || st[n - 3].op.is_some())
                {
                    o_l = mix_l;
                }
                if o_l.succ(o_r) || (o_l.same(o_r) && o_l.assoc == Assoc::Left) {
                    if o_l.is_infix() {
                        Self::reduce_infix(st, o_l);
                    } else if o_l.is_prefix() {
                        Self::reduce_prefix(st);
                    } else if is_void(o_r) {
                        return Err(self.err_at(
                            st[n - 2].at,
                            Code::MissingOperator,
                            "missing operator or separator after this expression",
                        ));
                    } else {
                        return Err(self.err_at(
                            at_r,
                            Code::MisplacedOperator,
                            format!(
                                "illegal combination of operators `{}` and `{}`",
                                o_l.canonical, o_r.canonical
                            ),
                        ));
                    }
                } else if !o_l.prec(o_r) {
                    return Err(self.conflict(at_r, o_l, o_r));
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
            if n == st.len() - 1 {
                return Ok(());
            }
        }
    }

    /// `finalReduce`: push the void operator and reduce to one expression.
    fn final_reduce(&mut self, mut st: Stack) -> PResult<Raw> {
        let at = self.cur;
        st.push(Elem {
            node: ENode::Void,
            op: Some(&VOID),
            at,
        });
        self.reduce_stack(&mut st)?;
        if st.len() == 2 {
            let _void = st.pop();
            if let Some(Elem {
                node: ENode::Expr(r),
                ..
            }) = st.pop()
            {
                return Ok(r);
            }
        }
        let at = st.first().map_or(at, |e| e.at);
        Err(self.err_at(at, Code::MissingOperator, "could not parse this expression"))
    }

    /// `reduceRecord`: `e.field`.
    fn reduce_record(&mut self, st: &mut Stack, dot: u32, field: u32, dot_at: VPos) -> PResult<()> {
        let Some(index) = st.len().checked_sub(1) else {
            return Err(self.err_at(dot_at, Code::UnexpectedToken, "`.` has no left-hand side"));
        };
        if let Some(op) = st[index].op {
            let below_is_op = index.checked_sub(1).map(|i| st[i].op.is_some());
            if op.is_postfix() && below_is_op == Some(false) {
                st.push(Elem {
                    node: ENode::Void,
                    op: None,
                    at: dot_at,
                });
                Self::reduce_postfix(st);
                st.pop();
            } else {
                return Err(self.err_at(
                    dot_at,
                    Code::UnexpectedToken,
                    format!("`.` follows operator `{}`", op.canonical),
                ));
            }
        }
        let Some(left) = st.pop() else {
            return Err(self.err_at(dot_at, Code::UnexpectedToken, "`.` has no left-hand side"));
        };
        let mut r = Raw::new(RECORD_FIELD);
        r.children.extend(into_child(left.node));
        r.tok(dot);
        r.tok(field);
        st.push(Elem {
            node: ENode::Expr(r),
            op: None,
            at: left.at,
        });
        Ok(())
    }

    /// `preInEmptyTop`: the stack is empty or ends in a prefix/infix operator.
    fn pre_in_empty_top(st: &Stack) -> bool {
        st.last()
            .is_none_or(|e| e.op.is_some_and(|o| o.is_prefix() || o.is_infix()))
    }

    // ---- Expression ---------------------------------------------------------

    /// `Expression()`.
    pub(super) fn expression(&mut self) -> PResult<Raw> {
        self.enter()?;
        let r = self.expression_inner();
        self.leave();
        r
    }

    fn expression_inner(&mut self) -> PResult<Raw> {
        let mut st: Stack = Vec::new();
        self.prefix_ops(&mut st)?;
        let v = self.cur;
        if is_open_start(self.kind(v)) && self.above(v) {
            let e = self.open_expr()?;
            Self::push_expr(&mut st, e, v);
        } else if self.above(v) {
            self.extendable_expr(&mut st)?;
        } else {
            return Err(self.expected("an expression").with_help(
                "in a `/\\` or `\\/` list, an item must be indented to the right of its bullet",
            ));
        }
        self.final_reduce(st)
    }

    /// The `(PrefixOp | "-")*` loop of `Expression` and `ExtendableExpr`.
    fn prefix_ops(&mut self, st: &mut Stack) -> PResult<()> {
        loop {
            let v = self.cur;
            let k = self.kind(v);
            if (is_prefix_tok(k) || k == MINUS_OP) && self.above(v) && !self.is_minus_op_start() {
                let t = self.op_token();
                self.push_op(st, t, v)?;
            } else {
                return Ok(());
            }
        }
    }

    /// `isMinusOpStart`: `-(a, b)` is the nonfix binary minus.
    fn is_minus_op_start(&self) -> bool {
        if self.la_kind(1) != MINUS_OP || self.la_kind(2) != L_PAREN {
            return false;
        }
        let mut depth = 0u32;
        let mut v = self.la(2);
        loop {
            v = self.next_v(v);
            match self.kind(v) {
                EOF => return false,
                L_PAREN | L_BRACK | L_BRACE | L_ANGLE => depth += 1,
                R_PAREN | R_BRACK | R_BRACE | R_ANGLE => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                COMMA if depth == 0 => return true,
                _ => {}
            }
        }
    }

    /// `ExtendableExpr()`. SANY recurses after each infix operator; this
    /// loops instead, which is equivalent because the call is in tail position.
    fn extendable_expr(&mut self, st: &mut Stack) -> PResult<()> {
        loop {
            self.operand(st)?;
            self.postfixes(st)?;
            // An infix operator and the rest, or a label.
            let at = self.cur;
            let kind = self.kind(at);
            if is_infix_tok(kind) && self.above(at) {
                let op = self.op_token();
                self.push_op(st, op, at)?;
                self.prefix_ops(st)?;
                let next = self.cur;
                if is_open_start(self.kind(next)) && self.above(next) {
                    let open = self.open_expr()?;
                    Self::push_expr(st, open, next);
                    return Ok(());
                }
                if !self.above(next) {
                    return Err(self.expected("an expression after the infix operator"));
                }
                // Loop: parse the next operand on the same stack.
            } else if kind == COLON_COLON && self.above(at) {
                return self.label(st);
            } else {
                return Ok(());
            }
        }
    }

    /// The operand of `ExtendableExpr`: a junction list, a bracketed form,
    /// or a primitive expression.
    fn operand(&mut self, st: &mut Stack) -> PResult<()> {
        let at = self.cur;
        let kind = self.kind(at);
        let operand = if matches!(kind, AND_OP | OR_OP) && Self::pre_in_empty_top(st) {
            self.junctions()?
        } else if matches!(kind, L_PAREN | L_BRACE | L_BRACK | L_ANGLE | WF_KW | SF_KW) {
            self.parentheses_expr()?
        } else {
            self.primitive_exp()?
        };
        Self::push_expr(st, operand, at);
        Ok(())
    }

    /// Postfix operators, record fields and function application.
    fn postfixes(&mut self, st: &mut Stack) -> PResult<()> {
        loop {
            let at = self.cur;
            let kind = self.kind(at);
            if kind == POSTFIX_OP && self.above(at) {
                let op = self.op_token();
                self.push_op(st, op, at)?;
            } else if kind == DOT && self.above(at) {
                let dot = self.bump();
                self.convert_field_name();
                let field = self.expect(IDENT, "a field name after `.`")?;
                self.reduce_record(st, dot, field, at)?;
            } else if kind == L_BRACK && self.above(at) {
                let (node, is_fcn) = self.sbracket_cases()?;
                if is_fcn {
                    st.push(Elem {
                        node: ENode::Fcn(node),
                        op: Some(ops::fcn_op()),
                        at,
                    });
                    if st.len() != 1 {
                        self.reduce_stack(st)?;
                    }
                } else {
                    Self::push_expr(st, node, at);
                }
            } else {
                return Ok(());
            }
        }
    }

    /// `lab :: e` (the `<COLONCOLON>` branch of `ExtendableExpr`).
    fn label(&mut self, st: &mut Stack) -> PResult<()> {
        let at = self.cur;
        let cc = self.bump();
        let top = st.pop();
        let name = match top {
            Some(Elem {
                node: ENode::Expr(r),
                at: name_at,
                ..
            }) if Self::is_label(&r) => (r, name_at),
            other => {
                if let Some(o) = other {
                    st.push(o);
                }
                return Err(self
                    .err_at(at, Code::BadLabel, "`::` does not follow a label")
                    .with_help("a label is an identifier, optionally with identifier arguments: `lab(x) :: e`"));
            }
        };
        let body = self.expression()?;
        // labelDoesNotChangeParse
        if matches!(body.kind, INFIX_EXPR | POSTFIX_EXPR)
            && let Some(stack_op) = st.last().and_then(|e| e.op)
            && let Some(label_op) = self.last_op
            && !stack_op.prec(label_op)
        {
            return Err(self
                .err_at(
                    name.1,
                    Code::LabelChangesParse,
                    "removing this label would change how the expression parses",
                )
                .with_help("parenthesise the labelled expression"));
        }
        let mut l = Raw::new(LABEL);
        l.node(name.0);
        l.tok(cc);
        l.node(body);
        Self::push_expr(st, l, name.1);
        Ok(())
    }

    /// SANY's `isLabel`: an unqualified name, or an application whose
    /// arguments are all unqualified names.
    fn is_label(r: &Raw) -> bool {
        fn plain_name(r: &Raw) -> bool {
            r.kind == GENERAL_ID
                && !r
                    .children
                    .iter()
                    .any(|c| matches!(c, Child::Node(n) if n.kind == ID_PREFIX))
        }
        match r.kind {
            GENERAL_ID => plain_name(r),
            OP_APPLICATION => r.children.iter().all(|c| match c {
                Child::Node(args) if args.kind == OP_ARGS => {
                    args.children.iter().all(|a| match a {
                        Child::Node(n) => plain_name(n),
                        Child::Tok(_) => true,
                    })
                }
                _ => true,
            }),
            _ => false,
        }
    }

    // ---- Primitive expressions ---------------------------------------------

    /// `PrimitiveExp()`.
    fn primitive_exp(&mut self) -> PResult<Raw> {
        let v = self.cur;
        let k = self.kind(v);
        if k == STRING && self.above(v) {
            let mut n = Raw::new(STRING_LIT);
            n.tok(self.bump());
            return Ok(n);
        }
        if k == NUMBER && self.above(v) {
            return self.number();
        }
        let name = match k {
            IDENT => self.bump(),
            STEP_NUMBER | STEP_IMPLICIT => {
                if self.proof_level() < 0 && self.proof_depth <= 0 {
                    return Err(self.err_at(
                        v,
                        Code::StepNumberMisuse,
                        "step number used outside a proof",
                    ));
                }
                if self.text(v.idx).encode_utf16().nth(1) == Some(u16::from(b'+')) {
                    return Err(self.err_at(
                        v,
                        Code::StepNumberMisuse,
                        "`<+>` step number used in an expression",
                    ));
                }
                self.bump()
            }
            _ if is_infix_tok(k) || k == POSTFIX_OP || k == NEG_DOT_OP => self.op_token(),
            _ => return Err(self.expected("an expression")),
        };
        let args = if self.at(L_PAREN) && starts_op_or_expr(self.la_kind(2)) {
            Some(self.op_args()?)
        } else {
            None
        };
        let mut bangs = Vec::new();
        while self.at(BANG) {
            bangs.push(self.bang_ext()?);
        }
        Ok(Self::general_id(Child::Tok(name), args, bangs))
    }

    /// Builds a `GENERAL_ID` (or `OP_APPLICATION`) from a name, its
    /// arguments and `!`-extensions, arranged as in SANY's `PrimitiveExp`:
    /// each `part [args] !` becomes an `ID_PREFIX_ELEMENT`, and the last
    /// part is the name (applied to its arguments, if any).
    fn general_id(name: Child, args: Option<Raw>, bangs: Vec<BangPart>) -> Raw {
        if bangs.is_empty() {
            let mut g = Raw::new(GENERAL_ID);
            g.children.push(name);
            return Self::apply(g, args);
        }
        let mut prefix = Raw::new(ID_PREFIX);
        let mut part = name;
        let mut part_args = args;
        for b in bangs {
            let mut elem = Raw::new(ID_PREFIX_ELEMENT);
            elem.children.push(part);
            if let Some(a) = part_args {
                elem.node(a);
            }
            elem.tok(b.bang);
            prefix.node(elem);
            part = b.part;
            part_args = b.args;
        }
        let mut g = Raw::new(GENERAL_ID);
        g.node(prefix);
        g.children.push(part);
        Self::apply(g, part_args)
    }

    fn apply(g: Raw, args: Option<Raw>) -> Raw {
        match args {
            None => g,
            Some(a) => {
                let mut app = Raw::new(OP_APPLICATION);
                app.node(g);
                app.node(a);
                app
            }
        }
    }

    /// `BangExt()`.
    fn bang_ext(&mut self) -> PResult<BangPart> {
        let bang = self.bump();
        let v = self.cur;
        let k = self.kind(v);
        let with_args = |p: &mut Self, part: u32| -> PResult<BangPart> {
            let args = if p.at(L_PAREN) && starts_op_or_expr(p.la_kind(2)) {
                Some(p.op_args()?)
            } else {
                None
            };
            Ok(BangPart {
                bang,
                part: Child::Tok(part),
                args,
            })
        };
        if k == IDENT && self.text(v.idx) != "@" {
            let t = self.bump();
            return with_args(self, t);
        }
        if is_ne_prefix_tok(k) || is_infix_tok(k) || k == POSTFIX_OP {
            let t = self.op_token();
            return with_args(self, t);
        }
        if k == L_PAREN {
            let a = self.op_args()?;
            return Ok(BangPart {
                bang,
                part: Child::Node(a),
                args: None,
            });
        }
        // StructOp
        let mut s = Raw::new(STRUCT_OP);
        match k {
            L_ANGLE | R_ANGLE | COLON | IDENT => s.tok(self.bump()),
            NUMBER => {
                let num = self.number()?;
                if num.kind == DECIMAL_LIT {
                    return Err(self.err_at(
                        v,
                        Code::BadStructuralSelector,
                        "a decimal number cannot select a subexpression",
                    ));
                }
                s.node(num);
            }
            _ => {
                return Err(self
                    .expected("a name, operator, `(`, `<<`, `>>`, `:`, `@` or number after `!`"));
            }
        }
        Ok(BangPart {
            bang,
            part: Child::Node(s),
            args: None,
        })
    }

    /// `Number()`: a natural number, or `n.m`.
    fn number(&mut self) -> PResult<Raw> {
        let t = self.expect(NUMBER, "a number")?;
        if self.at(DOT) && self.la_kind(2) == NUMBER {
            let mut n = Raw::new(DECIMAL_LIT);
            n.tok(t);
            n.tok(self.bump());
            n.tok(self.bump());
            return Ok(n);
        }
        let mut n = Raw::new(NUMBER_LIT);
        n.tok(t);
        Ok(n)
    }

    /// `OpArgs()`.
    pub(super) fn op_args(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(OP_ARGS);
        n.tok(self.expect(L_PAREN, "`(`")?);
        n.node(self.op_or_expr()?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.node(self.op_or_expr()?);
        }
        n.tok(self.expect(R_PAREN, "`,` or `)`")?);
        Ok(n)
    }

    /// `OpOrExpr()`: an operator argument, which may be a bare operator, a
    /// `LAMBDA`, or an expression.
    pub(super) fn op_or_expr(&mut self) -> PResult<Raw> {
        let v = self.cur;
        let k1 = self.kind(v);
        let k2 = self.la_kind(2);
        let op_alone = (is_ne_prefix_tok(k1) || is_infix_tok(k1) || k1 == POSTFIX_OP)
            && matches!(
                k2,
                COMMA
                    | R_PAREN
                    | DEFBREAK_LIT
                    | LOCAL_KW
                    | INSTANCE_KW
                    | THEOREM_KW
                    | ASSUME_KW
                    | END_MODULE
                    | SEPARATOR
                    | BEGIN_MODULE
                    | ASSUMPTION_KW
                    | CONSTANT_KW
                    | VARIABLE_KW
                    | RECURSIVE_KW
            )
            && self.above(v);
        if op_alone {
            let t = self.op_token();
            let text = self.text(t);
            if is_infix_tok(k1) && (text == "\\X" || text == "\\times") {
                return Err(self.err_at(
                    v,
                    Code::TimesAsArgument,
                    "`\\X` may not be used as an infix operator argument",
                ));
            }
            let mut n = Raw::new(OP_REF);
            n.tok(t);
            return Ok(n);
        }
        if k1 == LAMBDA_KW && self.above(v) {
            return self.lambda();
        }
        self.expression()
    }

    /// `Lambda()`.
    fn lambda(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(LAMBDA);
        n.tok(self.bump());
        n.tok(self.expect(IDENT, "a parameter name")?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.tok(self.expect(IDENT, "a parameter name")?);
        }
        n.tok(self.expect(COLON, "`,` or `:`")?);
        n.node(self.expression()?);
        Ok(n)
    }

    // ---- Junction lists -------------------------------------------------------

    /// `Junctions()`: a bulleted `/\` or `\/` list.
    fn junctions(&mut self) -> PResult<Raw> {
        let v = self.cur;
        let k = self.kind(v);
        let col = self.col(v);
        let (list_kind, item_kind) = if k == OR_OP {
            (DISJ_LIST, DISJ_ITEM)
        } else {
            (CONJ_LIST, CONJ_ITEM)
        };
        self.junctions.push((col, k));
        let mut list = Raw::new(list_kind);
        let r = (|| -> PResult<()> {
            loop {
                let item = self.junction_item(item_kind, col)?;
                list.node(item);
                let w = self.cur;
                let is_new_bullet = !w.virt && self.kind(w) == k && self.col(w) == col;
                if !is_new_bullet {
                    return Ok(());
                }
            }
        })();
        self.junctions.pop();
        r?;
        Ok(list)
    }

    /// `JuncItem()`, including SANY's `checkIndentation`: every token of the
    /// item must lie strictly right of the bullet.
    fn junction_item(&mut self, kind: SyntaxKind, col: u32) -> PResult<Raw> {
        let mut n = Raw::new(kind);
        let bullet = self.bump();
        n.tok(bullet);
        let e = self.expression()?;
        let first = bullet + 1;
        let last = e.last_token().unwrap_or(bullet);
        for i in first..=last {
            if self.kinds.get(i as usize) == Some(&DEFBREAK_LIT) {
                continue;
            }
            let c = self.toks.get(i as usize).map_or(u32::MAX, |t| t.col);
            if c <= col {
                return Err(self
                    .err_at(
                        VPos {
                            idx: i,
                            virt: false,
                        },
                        Code::JunctionIndentation,
                        format!(
                            "{} is not indented to the right of the `{}` bullet of its list item",
                            self.describe(VPos {
                                idx: i,
                                virt: false
                            }),
                            self.text(bullet)
                        ),
                    )
                    .with_help(
                        "indent the continuation further right than the bullet, or parenthesise",
                    ));
            }
        }
        n.node(e);
        Ok(n)
    }

    // ---- Bracketed expressions ----------------------------------------------

    /// `ParenthesesExpression()`.
    fn parentheses_expr(&mut self) -> PResult<Raw> {
        match self.peek() {
            L_PAREN => self.paren_expr(),
            L_BRACE => self.brace_cases(),
            L_BRACK => Ok(self.sbracket_cases()?.0),
            L_ANGLE => self.tuple_or_action(),
            _ => self.fairness(),
        }
    }

    /// `ParenExpr()`.
    fn paren_expr(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(PAREN_EXPR);
        n.tok(self.expect(L_PAREN, "`(`")?);
        n.node(self.expression()?);
        n.tok(self.expect(R_PAREN, "`)`")?);
        Ok(n)
    }

    /// `matchFcnConst`: `<<...>> \in` or `x, y \in` follows.
    fn match_fcn_const(&self) -> bool {
        let mut v = self.la(1);
        match self.kind(v) {
            L_ANGLE => {
                let mut count = 1i64;
                loop {
                    v = self.next_v(v);
                    match self.kind(v) {
                        L_ANGLE => count += 1,
                        R_ANGLE => count -= 1,
                        EOF => return false,
                        _ => {}
                    }
                    if count == 0 {
                        break;
                    }
                }
                self.kind(self.next_v(v)) == IN_OP
            }
            IDENT => {
                v = self.next_v(v);
                while self.kind(v) == COMMA {
                    v = self.next_v(v);
                    if self.kind(v) != IDENT {
                        return false;
                    }
                    v = self.next_v(v);
                }
                self.kind(v) == IN_OP
            }
            _ => false,
        }
    }

    /// Scans `IdentifierTuple()` from `v`; returns the position after it.
    fn scan_ident_tuple(&self, v: VPos) -> Option<VPos> {
        if self.kind(v) != L_ANGLE {
            return None;
        }
        let mut w = self.next_v(v);
        if self.kind(w) == IDENT {
            w = self.next_v(w);
            while self.kind(w) == COMMA {
                w = self.next_v(w);
                if self.kind(w) != IDENT {
                    return None;
                }
                w = self.next_v(w);
            }
        }
        if self.kind(w) == R_ANGLE {
            Some(self.next_v(w))
        } else {
            None
        }
    }

    /// `LOOKAHEAD((IdentifierTuple() | <IDENTIFIER>) follow)`.
    fn tuple_or_ident_then(&self, follow: SyntaxKind) -> bool {
        let v = self.la(1);
        let after = if self.kind(v) == IDENT {
            Some(self.next_v(v))
        } else {
            self.scan_ident_tuple(v)
        };
        after.is_some_and(|a| self.kind(a) == follow)
    }

    /// `IdentifierTuple()`.
    pub(super) fn ident_tuple(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(IDENT_TUPLE);
        n.tok(self.expect(L_ANGLE, "`<<`")?);
        if self.at(IDENT) {
            n.tok(self.bump());
            while self.at(COMMA) {
                n.tok(self.bump());
                n.tok(self.expect(IDENT, "an identifier")?);
            }
        }
        n.tok(self.expect(R_ANGLE, "`,` or `>>`")?);
        Ok(n)
    }

    /// `QuantBound()`: `x, y \in S` or `<<x, y>> \in S`.
    pub(super) fn quant_bound(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(QUANT_BOUND);
        if self.at(L_ANGLE) {
            n.node(self.ident_tuple()?);
        } else {
            n.tok(self.expect(IDENT, "a bound identifier or `<<`")?);
            while self.at(COMMA) {
                n.tok(self.bump());
                n.tok(self.expect(IDENT, "an identifier")?);
            }
        }
        n.tok(self.expect(IN_OP, "`\\in`")?);
        n.node(self.expression()?);
        Ok(n)
    }

    /// `QuantBound (, QuantBound)*` into `n`.
    pub(super) fn quant_bounds(&mut self, n: &mut Raw) -> PResult<()> {
        n.node(self.quant_bound()?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.node(self.quant_bound()?);
        }
        Ok(())
    }

    /// `BraceCases()`: `{a, b}`, `{x \in S : P}` and `{e : x \in S}`.
    fn brace_cases(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(SET_ENUM);
        n.tok(self.bump());
        if self.match_fcn_const() {
            let tuple = self.at(L_ANGLE);
            let var = if tuple {
                self.ident_tuple()?
            } else {
                let mut g = Raw::new(GENERAL_ID);
                g.tok(self.expect(IDENT, "an identifier")?);
                g
            };
            let in_tok = self.expect(IN_OP, "`\\in`")?;
            let set = self.expression()?;
            if self.at(COLON) {
                n.kind = SET_FILTER;
                let mut qb = Raw::new(QUANT_BOUND);
                if tuple {
                    qb.node(var);
                } else {
                    let mut var = var;
                    qb.children.append(&mut var.children);
                }
                qb.tok(in_tok);
                qb.node(set);
                n.node(qb);
                n.tok(self.bump());
                n.node(self.expression()?);
            } else {
                let mut infix = Raw::new(INFIX_EXPR);
                infix.node(if tuple {
                    ident_tuple_to_tuple(var)
                } else {
                    var
                });
                infix.tok(in_tok);
                infix.node(set);
                n.node(infix);
                while self.at(COMMA) {
                    n.tok(self.bump());
                    n.node(self.expression()?);
                }
            }
        } else if self.tuple_or_ident_then(COMMA) {
            n.node(self.expression()?);
            while self.at(COMMA) {
                n.tok(self.bump());
                n.node(self.expression()?);
            }
        } else if self.tuple_or_ident_then(COLON) {
            n.kind = SET_MAP;
            n.node(self.expression()?);
            n.tok(self.expect(COLON, "`:`")?);
            self.quant_bounds(&mut n)?;
        } else if starts_expr(self.peek()) {
            let first_at = self.cur;
            let e = self.expression()?;
            if self.at(COLON) {
                // SANY rejects `{a \in b : ...}` here (only the ASCII `\in`).
                if e.kind == INFIX_EXPR
                    && let Some(Child::Tok(op)) = e.children.get(1)
                    && self.text(*op) == "\\in"
                {
                    return Err(self
                        .err_at(
                            first_at,
                            Code::SetMapWithIn,
                            "the form `{a \\in b : c \\in d}` is not allowed",
                        )
                        .with_help(
                            "parenthesise the element expression: `{(a \\in b) : c \\in d}`",
                        ));
                }
                n.kind = SET_MAP;
                n.node(e);
                n.tok(self.bump());
                self.quant_bounds(&mut n)?;
            } else {
                n.node(e);
                while self.at(COMMA) {
                    n.tok(self.bump());
                    n.node(self.expression()?);
                }
            }
        }
        n.tok(self.expect(R_BRACE, "`}`")?);
        Ok(n)
    }

    /// `SBracketCases()`. Returns the node and whether it is the `[args]`
    /// of a function application (SANY's `isGenOp`).
    pub(super) fn sbracket_cases(&mut self) -> PResult<(Raw, bool)> {
        let lb = self.bump();
        let mut n = Raw::new(FCN_APPL);
        n.tok(lb);
        if self.match_fcn_const() {
            n.kind = FCN_CONSTRUCTOR;
            self.quant_bounds(&mut n)?;
            n.tok(self.expect(MAPS_TO, "`|->`")?);
            n.node(self.expression()?);
            n.tok(self.expect(R_BRACK, "`]`")?);
            return Ok((n, false));
        }
        let v1 = self.cur;
        let k1 = self.kind(v1);
        let k2 = self.la_kind(2);
        let field_kw = !v1.virt && super::is_field_name_kw(k1, self.text(v1.idx));
        if k2 == MAPS_TO && (k1 == IDENT || field_kw) {
            self.convert_field_name();
            n.kind = RECORD;
            n.node(self.field(FIELD_VAL, MAPS_TO, "`|->`")?);
            while self.at(COMMA) {
                n.tok(self.bump());
                n.node(self.field(FIELD_VAL, MAPS_TO, "`|->`")?);
            }
            n.tok(self.expect(R_BRACK, "`,` or `]`")?);
            return Ok((n, false));
        }
        if k1 == IDENT && k2 == COLON {
            n.kind = SET_OF_RECORDS;
            n.node(self.field(FIELD_SET, COLON, "`:`")?);
            while self.at(COMMA) {
                n.tok(self.bump());
                n.node(self.field(FIELD_SET, COLON, "`:`")?);
            }
            n.tok(self.expect(R_BRACK, "`,` or `]`")?);
            return Ok((n, false));
        }
        n.node(self.expression()?);
        match self.peek() {
            COMMA | R_BRACK => {
                while self.at(COMMA) {
                    n.tok(self.bump());
                    n.node(self.expression()?);
                }
                n.tok(self.expect(R_BRACK, "`,` or `]`")?);
                self.last_op = Some(ops::fcn_op());
                Ok((n, true))
            }
            ARROW => {
                n.kind = SET_OF_FCNS;
                n.tok(self.bump());
                n.node(self.expression()?);
                n.tok(self.expect(R_BRACK, "`]`")?);
                Ok((n, false))
            }
            EXCEPT_KW => {
                n.kind = EXCEPT;
                n.tok(self.bump());
                n.node(self.except_spec()?);
                while self.at(COMMA) {
                    n.tok(self.bump());
                    n.node(self.except_spec()?);
                }
                n.tok(self.expect(R_BRACK, "`,` or `]`")?);
                Ok((n, false))
            }
            R_BRACK_UNDERSCORE => {
                n.kind = ACTION_EXPR;
                n.tok(self.bump());
                n.node(self.reduced_expression()?);
                Ok((n, false))
            }
            _ => Err(self.expected("`]`, `,`, `->`, `EXCEPT` or `]_`")),
        }
    }

    /// `FieldVal()` / `FieldSet()`.
    fn field(&mut self, kind: SyntaxKind, sep: SyntaxKind, what: &str) -> PResult<Raw> {
        let mut n = Raw::new(kind);
        n.tok(self.expect(IDENT, "a field name")?);
        n.tok(self.expect(sep, what)?);
        n.node(self.expression()?);
        Ok(n)
    }

    /// `ExceptSpec()`: `![e].f = e`.
    fn except_spec(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(EXCEPT_SPEC);
        n.tok(self.expect(BANG, "`!`")?);
        loop {
            let v = self.cur;
            let mut c = Raw::new(EXCEPT_COMPONENT);
            match self.kind(v) {
                DOT => {
                    c.tok(self.bump());
                    self.convert_field_name();
                    let at = self.cur;
                    let f = self.expect(IDENT, "a field name after `.`")?;
                    if self.text(f) == "@" {
                        self.soft_error(at, Code::AtInExcept, "`@` used as a field name in `!.@`");
                    }
                    c.tok(f);
                }
                L_BRACK => {
                    c.tok(self.bump());
                    c.node(self.expression()?);
                    while self.at(COMMA) {
                        c.tok(self.bump());
                        c.node(self.expression()?);
                    }
                    c.tok(self.expect(R_BRACK, "`,` or `]`")?);
                }
                _ => {
                    if n.children.len() == 1 {
                        return Err(self.expected("`.` or `[` after `!`"));
                    }
                    break;
                }
            }
            n.node(c);
        }
        n.tok(self.expect(EQ_OP, "`=`")?);
        n.node(self.expression()?);
        Ok(n)
    }

    /// `TupleOrAction()`: `<<a, b>>` or `<<A>>_v`.
    fn tuple_or_action(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(TUPLE);
        n.tok(self.bump());
        if starts_expr(self.peek()) {
            n.node(self.expression()?);
            while self.at(COMMA) {
                n.tok(self.bump());
                n.node(self.expression()?);
            }
        }
        match self.peek() {
            R_ANGLE => n.tok(self.bump()),
            R_ANGLE_UNDERSCORE => {
                n.kind = ACTION_EXPR;
                n.tok(self.bump());
                n.node(self.reduced_expression()?);
            }
            _ => return Err(self.expected("`,`, `>>` or `>>_`")),
        }
        Ok(n)
    }

    /// `ReducedExpression()`: what may follow `]_` and `>>_`, and `WF_`/`SF_`.
    fn reduced_expression(&mut self) -> PResult<Raw> {
        match self.peek() {
            IDENT => self.no_op_extension(),
            L_PAREN => self.paren_expr(),
            L_BRACE => self.brace_cases(),
            L_BRACK => Ok(self.sbracket_cases()?.0),
            L_ANGLE => self.tuple_or_action(),
            _ => Err(self.expected("an identifier or a bracketed expression")),
        }
    }

    /// `NoOpExtension()`: `a!b(x)!c(y)`, setting SANY's `FairnessHook` to the
    /// argument list of the last part.
    fn no_op_extension(&mut self) -> PResult<Raw> {
        let mut prefix = Raw::new(ID_PREFIX);
        loop {
            let id = self.expect(IDENT, "an identifier")?;
            let args = if self.at(L_PAREN) && starts_op_or_expr(self.la_kind(2)) {
                Some(self.op_args()?)
            } else {
                None
            };
            if self.at(BANG) {
                let mut elem = Raw::new(ID_PREFIX_ELEMENT);
                elem.tok(id);
                if let Some(a) = args {
                    elem.node(a);
                }
                elem.tok(self.bump());
                prefix.node(elem);
                continue;
            }
            self.fairness_hook = args.as_ref().map(|a| a.children.len());
            let mut g = Raw::new(GENERAL_ID);
            if !prefix.children.is_empty() {
                g.node(prefix);
            }
            g.tok(id);
            return Ok(Self::apply(g, args));
        }
    }

    /// `FairnessExpr()`: `WF_v(A)` and `SF_v(A)`.
    fn fairness(&mut self) -> PResult<Raw> {
        let kw_at = self.cur;
        let mut n = Raw::new(FAIRNESS_EXPR);
        n.tok(self.bump());
        self.fairness_hook = None;
        let from_name = self.at(IDENT);
        let mut sub = self.reduced_expression()?;
        let hook = self.fairness_hook;
        if self.at(L_PAREN) && starts_expr(self.la_kind(2)) {
            let linked = from_name && hook.is_some();
            let ok_kinds = matches!(
                sub.kind,
                TUPLE
                    | PAREN_EXPR
                    | SET_ENUM
                    | SET_FILTER
                    | SET_MAP
                    | SET_OF_FCNS
                    | RECORD
                    | SET_OF_RECORDS
                    | EXCEPT
                    | FCN_CONSTRUCTOR
                    | ACTION_EXPR
            );
            if !linked && (from_name || !ok_kinds) {
                self.soft_error(
                    kw_at,
                    Code::BadFairness,
                    "ill-formed fairness expression: could not link the subscript and its arguments",
                );
            }
            n.node(sub);
            n.tok(self.bump());
            n.node(self.expression()?);
            n.tok(self.expect(R_PAREN, "`)`")?);
            return Ok(n);
        }
        match hook {
            None => Err(self
                .err_at(
                    kw_at,
                    Code::BadFairness,
                    "ill-structured fairness expression",
                )
                .with_help("write `WF_vars(Action)`")),
            Some(heirs) => {
                if heirs != 3 {
                    self.soft_error(
                        kw_at,
                        Code::BadFairness,
                        "a fairness expression takes exactly one action argument",
                    );
                }
                if from_name && sub.kind == OP_APPLICATION {
                    // Split `v(A)` into the subscript `v` and `(A)`.
                    let mut children = std::mem::take(&mut sub.children).into_iter();
                    if let Some(g) = children.next() {
                        n.children.push(g);
                    }
                    if let Some(Child::Node(mut args)) = children.next() {
                        n.children.append(&mut args.children);
                    }
                } else {
                    n.node(sub);
                }
                Ok(n)
            }
        }
    }

    // ---- Open expressions -----------------------------------------------------

    /// `OpenExpression()`.
    fn open_expr(&mut self) -> PResult<Raw> {
        match self.peek() {
            EXISTS_KW | FORALL_KW => self.some_quant(),
            TEMPORAL_EXISTS_KW | TEMPORAL_FORALL_KW => self.temporal_quant(),
            IF_KW => self.if_expr(),
            CASE_KW => self.case_expr(),
            LET_KW => self.let_expr(),
            _ => self.choose(),
        }
    }

    /// `LOOKAHEAD(<IDENTIFIER> (<COMMA> <IDENTIFIER>)* follow)`.
    pub(super) fn idents_then(&self, follow: SyntaxKind) -> bool {
        let mut v = self.la(1);
        if self.kind(v) != IDENT {
            return false;
        }
        v = self.next_v(v);
        while self.kind(v) == COMMA {
            v = self.next_v(v);
            if self.kind(v) != IDENT {
                return false;
            }
            v = self.next_v(v);
        }
        self.kind(v) == follow
    }

    /// `SomeQuant()`: `\A`/`\E`, bounded or unbounded.
    fn some_quant(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(BOUNDED_QUANT);
        n.tok(self.bump());
        if self.idents_then(COLON) {
            n.kind = UNBOUNDED_QUANT;
            n.tok(self.bump());
            while self.at(COMMA) {
                n.tok(self.bump());
                n.tok(self.bump());
            }
        } else {
            self.quant_bounds(&mut n)?;
        }
        n.tok(self.expect(COLON, "`:`")?);
        n.node(self.expression()?);
        Ok(n)
    }

    /// `SomeTQuant()`: `\AA`/`\EE`.
    fn temporal_quant(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(TEMPORAL_QUANT);
        n.tok(self.bump());
        n.tok(self.expect(IDENT, "an identifier")?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.tok(self.expect(IDENT, "an identifier")?);
        }
        n.tok(self.expect(COLON, "`,` or `:`")?);
        n.node(self.expression()?);
        Ok(n)
    }

    /// `IfThenElse()`.
    fn if_expr(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(IF_EXPR);
        n.tok(self.bump());
        n.node(self.expression()?);
        n.tok(self.expect(THEN_KW, "`THEN`")?);
        n.node(self.expression()?);
        n.tok(self.expect(ELSE_KW, "`ELSE`")?);
        n.node(self.expression()?);
        Ok(n)
    }

    /// SANY's `caseSep`: a `[]` right of the current junction column.
    fn case_sep(&self) -> bool {
        self.at(BOX_OP) && self.above(self.cur)
    }

    /// `Case()`.
    fn case_expr(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(CASE_EXPR);
        n.tok(self.bump());
        n.node(self.case_arm()?);
        while self.case_sep() && self.la_kind(2) != OTHER_KW {
            n.tok(self.bump());
            n.node(self.case_arm()?);
        }
        if self.case_sep() {
            n.tok(self.bump());
            let mut o = Raw::new(OTHER_ARM);
            o.tok(self.expect(OTHER_KW, "`OTHER`")?);
            o.tok(self.expect(ARROW, "`->`")?);
            o.node(self.expression()?);
            n.node(o);
        }
        Ok(n)
    }

    fn case_arm(&mut self) -> PResult<Raw> {
        let mut a = Raw::new(CASE_ARM);
        a.node(self.expression()?);
        a.tok(self.expect(ARROW, "`->`")?);
        a.node(self.expression()?);
        Ok(a)
    }

    /// `LetIn()`.
    fn let_expr(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(LET_EXPR);
        n.tok(self.bump());
        let mut defs = Raw::new(LET_DEFS);
        loop {
            match self.peek() {
                LOCAL_KW | DEFBREAK_LIT => defs.node(self.op_or_fcn_def()?),
                RECURSIVE_KW => defs.node(self.recursive_decl()?),
                _ => break,
            }
        }
        if defs.children.is_empty() {
            return Err(self.expected("a definition after `LET`"));
        }
        n.node(defs);
        n.tok(self.expect(IN_KW, "`IN` or another definition")?);
        n.node(self.expression()?);
        Ok(n)
    }

    /// `UnboundOrBoundChoose()`.
    fn choose(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(CHOOSE);
        n.tok(self.bump());
        if self.at(L_ANGLE) {
            n.node(self.ident_tuple()?);
        } else {
            n.tok(self.expect(IDENT, "an identifier or `<<`")?);
        }
        if self.at(IN_OP) {
            n.tok(self.bump());
            n.node(self.expression()?);
        }
        n.tok(self.expect(COLON, "`:`")?);
        n.node(self.expression()?);
        Ok(n)
    }
}

/// A `!` extension: the `!`, the selected part, and optional arguments.
#[derive(Debug)]
pub(super) struct BangPart {
    bang: u32,
    part: Child,
    args: Option<Raw>,
}
