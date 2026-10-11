//! Modules, declarations, definitions, instances, assumptions and theorems.
//! Productions `Module` to `Theorem` and `UseOrHideOrBy` of `tla+.jj`.

use super::{MAX_TREE_HEIGHT, PResult, Parser, VPos, is_infix_tok, is_ne_prefix_tok, starts_expr};
use crate::diagnostics::Code;
#[allow(
    clippy::enum_glob_use,
    reason = "the grammar names over a hundred token and node kinds"
)]
use crate::kind::SyntaxKind::*;
use crate::tree::{Child, Raw};

impl Parser<'_> {
    /// `Module()`. Never fails: errors are recorded and recovered from.
    /// Returns the node and whether the closing `====` was found.
    pub(super) fn module(&mut self) -> (Raw, bool) {
        let mut module = Raw::new(MODULE);
        if let Err(e) = self.enter() {
            let start = self.cur.idx;
            let col = self.col(self.cur);
            module.node(self.recover(e, start, col));
            return (module, false);
        }
        let start = self.cur.idx;
        match self.module_header() {
            Ok(h) => module.node(h),
            Err(e) => {
                let col = self.col(self.cur);
                module.node(self.recover(e, start, col));
            }
        }
        if self.at(EXTENDS_KW) {
            let start = self.cur.idx;
            match self.extends() {
                Ok(n) => module.node(n),
                Err(e) => module.node(self.recover(e, start, 1)),
            }
        }
        let ended = loop {
            let k = self.peek();
            if k == END_MODULE {
                module.tok(self.bump());
                break true;
            }
            if k == EOF {
                let at = self.cur;
                let e = self
                    .err_at(
                        at,
                        Code::MissingModuleEnd,
                        "end of input before the module's closing `====`",
                    )
                    .with_help("end the module with a line of four or more `=` characters");
                self.record(e);
                break false;
            }
            let start = self.cur.idx;
            let col = self.col(self.cur);
            match self.unit() {
                Ok(Some(Child::Node(n))) if n.kind != MODULE && n.height() > MAX_TREE_HEIGHT => {
                    // rowan frees green trees recursively; refuse trees deep
                    // enough to risk the stack, keeping the tokens.
                    let at = VPos {
                        idx: start,
                        virt: false,
                    };
                    self.soft_error(
                        at,
                        Code::NestingTooDeep,
                        format!(
                            "this unit's syntax tree is deeper than {MAX_TREE_HEIGHT} levels, which dive does not support"
                        ),
                    );
                    let mut err = Raw::new(ERROR);
                    for i in start..self.cur.idx {
                        err.tok(i);
                    }
                    module.node(err);
                }
                Ok(Some(Child::Node(n))) => module.node(n),
                Ok(Some(Child::Tok(t))) => module.tok(t),
                Ok(None) => {
                    let e = self.expected(
                        "`====` or a module unit (definition, declaration, ASSUME, THEOREM, INSTANCE, ...)",
                    );
                    module.node(self.recover(e, start, col));
                }
                Err(e) => module.node(self.recover(e, start, col)),
            }
        };
        self.leave();
        (module, ended)
    }

    /// `BeginModule()`.
    fn module_header(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(MODULE_HEADER);
        n.tok(self.expect(BEGIN_MODULE, "`---- MODULE`")?);
        self.convert_field_name();
        n.tok(self.expect(IDENT, "the module name")?);
        n.tok(self.expect(SEPARATOR, "`----` after the module name")?);
        Ok(n)
    }

    /// `Extends()`.
    fn extends(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(EXTENDS);
        n.tok(self.bump());
        self.convert_field_name();
        n.tok(self.expect(IDENT, "a module name")?);
        while self.at(COMMA) {
            n.tok(self.bump());
            self.convert_field_name();
            n.tok(self.expect(IDENT, "a module name")?);
        }
        Ok(n)
    }

    /// One iteration of `Body()`. `Ok(None)` if no unit starts here.
    fn unit(&mut self) -> PResult<Option<Child>> {
        let k1 = self.peek();
        let k2 = self.la_kind(2);
        let n = match k1 {
            // A separator is a token directly in the module.
            SEPARATOR => return Ok(Some(Child::Tok(self.bump()))),
            VARIABLE_KW => self.variable_decl()?,
            CONSTANT_KW => self.constant_decl()?,
            _ if (k1 == LOCAL_KW && k2 == DEFBREAK_LIT)
                || (k1 == DEFBREAK_LIT && (k2 == IDENT || is_ne_prefix_tok(k2))) =>
            {
                self.op_or_fcn_def()?
            }
            RECURSIVE_KW => self.recursive_decl()?,
            _ if (k1 == LOCAL_KW && k2 == INSTANCE_KW) || (k1 == INSTANCE_KW && k2 == IDENT) => {
                let mut n = Raw::new(INSTANCE);
                if self.at(LOCAL_KW) {
                    n.tok(self.bump());
                }
                n.node(self.instantiation()?);
                n
            }
            ASSUME_KW | ASSUMPTION_KW if k2 == DEFBREAK_LIT || starts_expr(k2) => {
                self.assumption()?
            }
            THEOREM_KW | PROPOSITION_KW
                if matches!(k2, IDENT | ASSUME_KW | BOX_ASSUME_KW) || starts_expr(k2) =>
            {
                self.theorem()?
            }
            BEGIN_MODULE => {
                let (m, _) = self.module();
                self.belch_def();
                m
            }
            USE_KW if k2 != ONLY_KW => self.use_or_hide_or_by()?,
            HIDE_KW => self.use_or_hide_or_by()?,
            _ => return Ok(None),
        };
        Ok(Some(Child::Node(n)))
    }

    /// `VariableDeclaration()`.
    fn variable_decl(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(VARIABLE_DECL);
        n.tok(self.bump());
        n.tok(self.expect(IDENT, "a variable name")?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.tok(self.expect(IDENT, "a variable name")?);
        }
        Ok(n)
    }

    /// `ParamDeclaration()`.
    fn constant_decl(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(CONSTANT_DECL);
        n.tok(self.bump());
        n.node(self.constant_item()?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.node(self.constant_item()?);
        }
        Ok(n)
    }

    /// `Recursive()`.
    pub(super) fn recursive_decl(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(RECURSIVE_DECL);
        n.tok(self.bump());
        n.node(self.constant_item()?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.node(self.constant_item()?);
        }
        Ok(n)
    }

    /// `ConstantDeclarationItems()`.
    fn constant_item(&mut self) -> PResult<Raw> {
        let k = self.peek();
        if k == IDENT {
            let mut n = Raw::new(IDENT_DECL);
            n.tok(self.bump());
            if self.at(L_PAREN) && self.la_kind(2) == UNDERSCORE {
                self.arity_list(&mut n)?;
            }
            Ok(n)
        } else if is_ne_prefix_tok(k) || k == UNDERSCORE {
            self.some_fix_decl()
        } else {
            Err(self.expected("an identifier, `_` or a prefix operator"))
        }
    }

    /// `( _ , _ )` after a declared identifier.
    fn arity_list(&mut self, n: &mut Raw) -> PResult<()> {
        n.tok(self.expect(L_PAREN, "`(`")?);
        n.tok(self.expect(UNDERSCORE, "`_`")?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.tok(self.expect(UNDERSCORE, "`_`")?);
        }
        n.tok(self.expect(R_PAREN, "`,` or `)`")?);
        Ok(())
    }

    /// `IdentDecl()`.
    pub(super) fn ident_decl(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(IDENT_DECL);
        n.tok(self.expect(IDENT, "an identifier")?);
        if self.at(L_PAREN) {
            self.arity_list(&mut n)?;
        }
        Ok(n)
    }

    /// `SomeFixDecl()`: `op _`, `_ op _` or `_ op`.
    pub(super) fn some_fix_decl(&mut self) -> PResult<Raw> {
        let k = self.peek();
        if is_ne_prefix_tok(k) {
            let mut n = Raw::new(PREFIX_DECL);
            n.tok(self.op_token());
            n.tok(self.expect(UNDERSCORE, "`_` after the prefix operator")?);
            return Ok(n);
        }
        let us = self.expect(UNDERSCORE, "`_` or a prefix operator")?;
        let k = self.peek();
        if is_infix_tok(k) {
            let mut n = Raw::new(INFIX_DECL);
            n.tok(us);
            n.tok(self.op_token());
            n.tok(self.expect(UNDERSCORE, "`_` after the infix operator")?);
            Ok(n)
        } else if k == POSTFIX_OP {
            let mut n = Raw::new(POSTFIX_DECL);
            n.tok(us);
            n.tok(self.op_token());
            Ok(n)
        } else {
            Err(self.expected("an infix or postfix operator after `_`"))
        }
    }

    /// `IdentDecl() | SomeFixDecl()`.
    pub(super) fn ident_or_some_fix_decl(&mut self) -> PResult<Raw> {
        if self.at(IDENT) {
            self.ident_decl()
        } else {
            self.some_fix_decl()
        }
    }

    /// `OperatorOrFunctionDefinition()`.
    pub(super) fn op_or_fcn_def(&mut self) -> PResult<Raw> {
        let local = if self.at(LOCAL_KW) {
            Some(self.bump())
        } else {
            None
        };
        let literal_defbreak = self.bump_defbreak()?;
        let k1 = self.peek();
        let k2 = self.la_kind(2);
        let mut n;
        if k1 == IDENT && k2 == L_BRACK {
            n = Raw::new(FUNCTION_DEF);
            Self::push_prefix(&mut n, local, literal_defbreak);
            n.tok(self.bump());
            n.tok(self.bump());
            n.node(self.quant_bound()?);
            while self.at(COMMA) {
                n.tok(self.bump());
                n.node(self.quant_bound()?);
            }
            n.tok(self.expect(R_BRACK, "`,` or `]`")?);
            n.tok(self.expect(DEF_EQ, "`==`")?);
            self.belch_def();
            n.node(self.expression()?);
            return Ok(n);
        }
        n = Raw::new(OPERATOR_DEF);
        Self::push_prefix(&mut n, local, literal_defbreak);
        if k1 == IDENT && k2 == POSTFIX_OP {
            let mut lhs = Raw::new(POSTFIX_LHS);
            lhs.tok(self.bump());
            lhs.tok(self.bump());
            n.node(lhs);
        } else if k1 == IDENT && is_infix_tok(k2) {
            let mut lhs = Raw::new(INFIX_LHS);
            lhs.tok(self.bump());
            lhs.tok(self.bump());
            lhs.tok(self.expect(IDENT, "an identifier after the infix operator")?);
            n.node(lhs);
        } else if k1 == IDENT && matches!(k2, L_PAREN | DEF_EQ) {
            let mut lhs = Raw::new(IDENT_LHS);
            lhs.tok(self.bump());
            if self.at(L_PAREN) {
                lhs.tok(self.bump());
                lhs.node(self.ident_or_some_fix_decl()?);
                while self.at(COMMA) {
                    lhs.tok(self.bump());
                    lhs.node(self.ident_or_some_fix_decl()?);
                }
                lhs.tok(self.expect(R_PAREN, "`,` or `)`")?);
            }
            n.node(lhs);
            n.tok(self.expect(DEF_EQ, "`==`")?);
            self.belch_def();
            if self.at(INSTANCE_KW) {
                n.kind = MODULE_DEF;
                n.node(self.instantiation()?);
            } else {
                n.node(self.expression()?);
            }
            return Ok(n);
        } else if is_ne_prefix_tok(k1) {
            let mut lhs = Raw::new(PREFIX_LHS);
            lhs.tok(self.bump());
            lhs.tok(self.expect(IDENT, "an identifier after the prefix operator")?);
            n.node(lhs);
        } else {
            return Err(self.expected("the left-hand side of a definition"));
        }
        n.tok(self.expect(DEF_EQ, "`==`")?);
        self.belch_def();
        n.node(self.expression()?);
        Ok(n)
    }

    fn push_prefix(n: &mut Raw, local: Option<u32>, defbreak: Option<u32>) {
        if let Some(l) = local {
            n.tok(l);
        }
        if let Some(d) = defbreak {
            n.tok(d);
        }
    }

    /// `Instantiation()`.
    pub(super) fn instantiation(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(INSTANTIATION);
        n.tok(self.expect(INSTANCE_KW, "`INSTANCE`")?);
        self.convert_field_name();
        n.tok(self.expect(IDENT, "a module name")?);
        if self.at(WITH_KW) {
            n.tok(self.bump());
            n.node(self.substitution()?);
            while self.at(COMMA) && self.substitution_follows(2) {
                n.tok(self.bump());
                n.node(self.substitution()?);
            }
        }
        Ok(n)
    }

    /// The `LOOKAHEAD(3)` before another `, Substitution`.
    fn substitution_follows(&self, from: usize) -> bool {
        let k = self.la_kind(from);
        (k == IDENT || is_ne_prefix_tok(k) || is_infix_tok(k) || k == POSTFIX_OP)
            && self.la_kind(from + 1) == LEFT_ARROW
    }

    /// `Substitution()`.
    fn substitution(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(SUBSTITUTION);
        let k = self.peek();
        if k == IDENT {
            n.tok(self.bump());
        } else if is_ne_prefix_tok(k) || is_infix_tok(k) || k == POSTFIX_OP {
            n.tok(self.op_token());
        } else {
            return Err(self.expected("an identifier or operator to substitute"));
        }
        n.tok(self.expect(LEFT_ARROW, "`<-`")?);
        n.node(self.op_or_expr()?);
        Ok(n)
    }

    /// `Assumption()`.
    fn assumption(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(ASSUMPTION);
        n.tok(self.bump());
        let k1 = self.peek();
        let k2 = self.la_kind(2);
        if (k1 == DEFBREAK_LIT && k2 == IDENT) || (k1 == IDENT && k2 == DEF_EQ) {
            if self.at(DEFBREAK_LIT)
                && let Some(i) = self.bump_defbreak()?
            {
                n.tok(i);
            }
            n.tok(self.expect(IDENT, "the assumption's name")?);
            n.tok(self.expect(DEF_EQ, "`==`")?);
        }
        self.belch_def();
        n.node(self.expression()?);
        Ok(n)
    }

    /// `Theorem()`.
    fn theorem(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(THEOREM);
        n.tok(self.bump());
        if self.at(IDENT) && self.la_kind(2) == DEF_EQ {
            n.tok(self.bump());
            n.tok(self.bump());
        }
        self.belch_def();
        if matches!(self.peek(), ASSUME_KW | BOX_ASSUME_KW) {
            n.node(self.assume_prove()?);
        } else {
            n.node(self.expression()?);
        }
        if self.begins_proof(self.cur) {
            n.node(self.proof()?);
        }
        Ok(n)
    }

    /// `UseOrHideOrBy()`: a `USE`/`HIDE` unit or a `BY` terminal proof.
    pub(super) fn use_or_hide_or_by(&mut self) -> PResult<Raw> {
        let mut n;
        match self.peek() {
            PROOF_KW | BY_KW => {
                n = Raw::new(TERMINAL_PROOF);
                if self.at(PROOF_KW) {
                    n.tok(self.bump());
                }
                n.tok(self.expect(BY_KW, "`BY`")?);
                if self.at(ONLY_KW) {
                    n.tok(self.bump());
                }
            }
            USE_KW => {
                n = Raw::new(USE_OR_HIDE);
                n.tok(self.bump());
                if self.at(ONLY_KW) {
                    n.tok(self.bump());
                }
            }
            HIDE_KW => {
                n = Raw::new(USE_OR_HIDE);
                n.tok(self.bump());
            }
            _ => return Err(self.expected("`BY`, `USE` or `HIDE`")),
        }
        if self.at(MODULE_KW) || starts_expr(self.peek()) {
            self.use_item(&mut n)?;
            while self.at(COMMA) {
                n.tok(self.bump());
                self.use_item(&mut n)?;
            }
        }
        if self.at(DEF_KW) {
            n.tok(self.bump());
            self.use_item(&mut n)?;
            while self.at(COMMA) {
                n.tok(self.bump());
                self.use_item(&mut n)?;
            }
        }
        Ok(n)
    }

    /// `MODULE Identifier | Expression` in a `USE`/`HIDE`/`BY`/`DEF` list.
    fn use_item(&mut self, n: &mut Raw) -> PResult<()> {
        if self.at(MODULE_KW) {
            let mut m = Raw::new(MODULE_REF);
            m.tok(self.bump());
            m.tok(self.expect(IDENT, "a module name after `MODULE`")?);
            n.node(m);
        } else {
            n.node(self.expression()?);
        }
        Ok(())
    }
}
