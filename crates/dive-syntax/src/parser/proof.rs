//! Proofs: SANY's `Proof`, `Step`, `QEDStep`, `AssumeProve` and `NewSymb`,
//! with the step-level bookkeeping (`proofLevelStack`, `beginsProof`,
//! `correctLevel`).

use super::{PResult, Parser, VPos, is_ne_prefix_tok, is_step_start, starts_expr};
use crate::diagnostics::Code;
#[allow(
    clippy::enum_glob_use,
    reason = "the grammar names over a hundred token and node kinds"
)]
use crate::kind::SyntaxKind::*;
use crate::tree::Raw;

/// SANY's `MaxProofDepth`.
const MAX_PROOF_DEPTH: i32 = 100;

/// The second UTF-16 unit of a token's text, as SANY's `image.substring(1, 2)`.
fn second_unit(text: &str) -> Option<u16> {
    text.encode_utf16().nth(1)
}

impl Parser<'_> {
    /// SANY's `getProofLevel`.
    pub(super) fn proof_level(&self) -> i32 {
        if self.proof_depth < 0 {
            return self.proof_depth;
        }
        self.proof_levels
            .get(usize::try_from(self.proof_depth).unwrap_or(0))
            .copied()
            .unwrap_or(-1)
    }

    fn set_proof_level(&mut self, level: i32) {
        if let Ok(d) = usize::try_from(self.proof_depth)
            && let Some(slot) = self.proof_levels.get_mut(d)
        {
            *slot = level;
        }
    }

    /// The level of the enclosing proof, or -1 at the top.
    fn outer_level(&self) -> i32 {
        if self.proof_depth > 0 {
            self.proof_levels
                .get(usize::try_from(self.proof_depth - 1).unwrap_or(0))
                .copied()
                .unwrap_or(-1)
        } else {
            -1
        }
    }

    /// SANY's `levelOfProofStepLexeme`: -1 for `<*>`, -2 for `<+>`, else n.
    /// `None` if the number does not fit in an `i32` (SANY crashes there).
    fn step_level(&self, v: VPos) -> Option<i32> {
        let text = self.text_v(v);
        match second_unit(text) {
            Some(c) if c == u16::from(b'*') => return Some(-1),
            Some(c) if c == u16::from(b'+') => return Some(-2),
            _ => {}
        }
        let end = text.find('>')?;
        text.get(1..end)?.parse::<i32>().ok()
    }

    /// SANY's `beginsProof`.
    pub(super) fn begins_proof(&self, v: VPos) -> bool {
        let text = self.text_v(v);
        if v.virt || text.encode_utf16().count() < 2 {
            return false;
        }
        match second_unit(text) {
            Some(c) if c == u16::from(b'*') => return self.proof_depth < 0,
            Some(c) if c == u16::from(b'+') => return true,
            _ => {}
        }
        match self.kind(v) {
            STEP_NUMBER | STEP_BARE | STEP_UNNUMBERED | STEP_DOT => {
                if self.proof_depth < 0 {
                    return true;
                }
                let current = self.proof_level();
                self.step_level(v)
                    .is_some_and(|l| current >= 0 && l > current)
            }
            BY_KW | PROOF_KW | OBVIOUS_KW | OMITTED_KW => true,
            _ => false,
        }
    }

    /// SANY's `correctLevel`; sets the current proof's level on its first step.
    fn correct_level(&mut self, v: VPos) -> bool {
        let Some(tok_level) = self.step_level(v) else {
            return false;
        };
        let last = self.outer_level();
        let current = self.proof_level();
        match tok_level {
            -1 => {
                if current < 0 {
                    self.set_proof_level(last + 1);
                }
                true
            }
            -2 => {
                if current < 0 {
                    self.set_proof_level(last + 1);
                    true
                } else {
                    false
                }
            }
            l => {
                if current < 0 {
                    self.set_proof_level(l);
                    l > last
                } else {
                    current == l
                }
            }
        }
    }

    fn push_proof_level(&mut self) -> PResult<()> {
        self.proof_depth += 1;
        if self.proof_depth >= MAX_PROOF_DEPTH {
            return Err(self.err_at(
                self.cur,
                Code::ProofTooDeep,
                format!("proofs nested more than {MAX_PROOF_DEPTH} deep"),
            ));
        }
        let d = usize::try_from(self.proof_depth).unwrap_or(0);
        self.proof_levels.truncate(d);
        self.proof_levels.push(-1);
        Ok(())
    }

    /// `Proof()`: a terminal proof (`BY`, `OBVIOUS`, `OMITTED`) or a
    /// sequence of steps ending in `QED`.
    pub(super) fn proof(&mut self) -> PResult<Raw> {
        self.enter()?;
        self.push_proof_level()?;
        let r = self.proof_inner();
        self.proof_depth -= 1;
        self.leave();
        r
    }

    fn proof_inner(&mut self) -> PResult<Raw> {
        let k1 = self.peek();
        let k2 = self.la_kind(2);
        if k1 == BY_KW || k2 == BY_KW {
            return self.use_or_hide_or_by();
        }
        if matches!(k1, OBVIOUS_KW | OMITTED_KW)
            || (k1 == PROOF_KW && matches!(k2, OBVIOUS_KW | OMITTED_KW))
        {
            let mut n = Raw::new(TERMINAL_PROOF);
            if self.at(PROOF_KW) {
                n.tok(self.bump());
            }
            n.tok(self.bump());
            return Ok(n);
        }
        if k1 != PROOF_KW && !is_step_start(k1) {
            return Err(self.expected("a proof (`BY`, `OBVIOUS`, `OMITTED`, `PROOF` or a step)"));
        }
        let mut n = Raw::new(PROOF);
        if self.at(PROOF_KW) {
            n.tok(self.bump());
        }
        while self.la_kind(2) != QED_KW {
            n.node(self.step()?);
        }
        n.node(self.qed_step()?);
        Ok(n)
    }

    /// `StepStartToken()` plus the level check.
    fn step_start(&mut self, step: &mut Raw) -> PResult<()> {
        let v = self.cur;
        if !is_step_start(self.kind(v)) {
            return Err(self.expected("a proof step number such as `<1>1.`"));
        }
        if !self.correct_level(v) {
            return Err(self
                .err_at(
                    v,
                    Code::StepLevel,
                    format!("step number {} has the wrong level", self.describe(v)),
                )
                .with_help(
                    "all steps of one proof share a level, deeper than the enclosing proof's",
                ));
        }
        step.tok(self.bump());
        Ok(())
    }

    /// `QEDStep()`.
    fn qed_step(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(PROOF_STEP);
        self.step_start(&mut n)?;
        let mut q = Raw::new(QED_STEP);
        q.tok(self.expect(QED_KW, "`QED`")?);
        n.node(q);
        if self.begins_proof(self.cur) {
            n.node(self.proof()?);
        }
        Ok(n)
    }

    /// `Step()`.
    fn step(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(PROOF_STEP);
        self.step_start(&mut n)?;
        let body_at = self.cur;
        let mut may_have_proof = false;
        let body = match self.peek() {
            USE_KW | HIDE_KW => self.use_or_hide_or_by()?,
            INSTANCE_KW => self.instantiation()?,
            DEFBREAK_LIT | DEFINE_KW => self.def_step()?,
            HAVE_KW => {
                let mut b = Raw::new(HAVE_STEP);
                b.tok(self.bump());
                b.node(self.expression()?);
                b
            }
            TAKE_KW => self.take_step()?,
            WITNESS_KW => {
                let mut b = Raw::new(WITNESS_STEP);
                b.tok(self.bump());
                b.node(self.expression()?);
                while self.at(COMMA) {
                    b.tok(self.bump());
                    b.node(self.expression()?);
                }
                b
            }
            PICK_KW => {
                may_have_proof = true;
                self.pick_step()?
            }
            CASE_KW => {
                may_have_proof = true;
                let mut b = Raw::new(CASE_STEP);
                b.tok(self.bump());
                b.node(self.expression()?);
                b
            }
            k if k == SUFFICES_KW || starts_expr(k) || matches!(k, ASSUME_KW | BOX_ASSUME_KW) => {
                may_have_proof = true;
                let mut b = Raw::new(ASSERT_STEP);
                if self.at(SUFFICES_KW) {
                    b.tok(self.bump());
                }
                if matches!(self.peek(), ASSUME_KW | BOX_ASSUME_KW) {
                    b.node(self.assume_prove()?);
                } else if starts_expr(self.peek()) {
                    b.node(self.expression()?);
                } else {
                    return Err(self.expected("an expression or `ASSUME`"));
                }
                b
            }
            _ => return Err(self.expected("a proof step")),
        };
        n.node(body);
        if self.begins_proof(self.cur) {
            if !may_have_proof {
                return Err(self
                    .err_at(
                        body_at,
                        Code::UnexpectedProof,
                        "a proof follows a step that does not take a proof",
                    )
                    .with_help("only assertion, `CASE`, `PICK` and `QED` steps take proofs"));
            }
            n.node(self.proof()?);
        }
        Ok(n)
    }

    /// `DefStep()`.
    fn def_step(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(DEF_STEP);
        if self.at(DEFINE_KW) {
            n.tok(self.bump());
        }
        let mut any = false;
        loop {
            let k1 = self.peek();
            let k2 = self.la_kind(2);
            let starts = (k1 == LOCAL_KW && k2 == DEFBREAK_LIT)
                || (k1 == DEFBREAK_LIT && (k2 == IDENT || is_ne_prefix_tok(k2)));
            if !starts {
                break;
            }
            n.node(self.op_or_fcn_def()?);
            any = true;
        }
        if !any {
            return Err(self.expected("a definition"));
        }
        Ok(n)
    }

    /// `TakeStep()`.
    fn take_step(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(TAKE_STEP);
        n.tok(self.bump());
        if self.idents_then(IN_OP) || self.at(L_ANGLE) {
            self.quant_bounds(&mut n)?;
        } else {
            n.tok(self.expect(IDENT, "an identifier or a bound")?);
            while self.at(COMMA) {
                n.tok(self.bump());
                n.tok(self.expect(IDENT, "an identifier")?);
            }
        }
        Ok(n)
    }

    /// `PickStep()`.
    fn pick_step(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(PICK_STEP);
        n.tok(self.bump());
        if self.idents_then(COLON) {
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

    /// `AssumeProve()`, possibly labelled.
    pub(super) fn assume_prove(&mut self) -> PResult<Raw> {
        self.enter()?;
        let r = self.assume_prove_inner();
        self.leave();
        r
    }

    fn assume_prove_inner(&mut self) -> PResult<Raw> {
        if self.at(IDENT) {
            let mut l = Raw::new(LABEL);
            let mut g = Raw::new(GENERAL_ID);
            g.tok(self.bump());
            l.node(g);
            l.tok(self.expect(COLON_COLON, "`::`")?);
            l.node(self.assume_prove()?);
            return Ok(l);
        }
        let mut n = Raw::new(ASSUME_PROVE);
        if !matches!(self.peek(), ASSUME_KW | BOX_ASSUME_KW) {
            return Err(self.expected("`ASSUME`"));
        }
        n.tok(self.bump());
        n.node(self.assume_item()?);
        while self.at(COMMA) {
            n.tok(self.bump());
            n.node(self.assume_item()?);
        }
        if !matches!(self.peek(), PROVE_KW | BOX_PROVE_KW) {
            return Err(self.expected("`PROVE` or `,`"));
        }
        n.tok(self.bump());
        n.node(self.expression()?);
        Ok(n)
    }

    fn assume_item(&mut self) -> PResult<Raw> {
        let k1 = self.peek();
        if matches!(k1, ASSUME_KW | BOX_ASSUME_KW)
            || (self.la_kind(2) == COLON_COLON
                && matches!(self.la_kind(3), ASSUME_KW | BOX_ASSUME_KW))
        {
            self.assume_prove()
        } else if matches!(
            k1,
            NEW_KW | CONSTANT_KW | VARIABLE_KW | STATE_KW | ACTION_KW | TEMPORAL_KW
        ) {
            self.new_symb()
        } else {
            self.expression()
        }
    }

    /// `NewSymb()`: `NEW`, `CONSTANT`, `VARIABLE`, `STATE`, `ACTION`, `TEMPORAL`.
    fn new_symb(&mut self) -> PResult<Raw> {
        let mut n = Raw::new(NEW_DECL);
        let k1 = self.peek();
        let k2 = self.la_kind(2);
        let decl_start =
            |k: crate::kind::SyntaxKind| k == IDENT || is_ne_prefix_tok(k) || k == UNDERSCORE;
        if (k1 == NEW_KW && (k2 == CONSTANT_KW || decl_start(k2)))
            || (k1 == CONSTANT_KW && decl_start(k2))
        {
            if k1 == NEW_KW && k2 == CONSTANT_KW {
                n.tok(self.bump());
            }
            n.tok(self.bump());
            if self.at(IDENT) {
                let at = self.cur;
                let d = self.ident_decl()?;
                let has_args = d.children.len() > 1;
                n.node(d);
                if self.at(IN_OP) {
                    if has_args {
                        return Err(self.err_at(
                            at,
                            Code::NewWithArguments,
                            "a symbol declared with arguments cannot have a `\\in` bound",
                        ));
                    }
                    n.tok(self.bump());
                    n.node(self.expression()?);
                }
            } else {
                n.node(self.some_fix_decl()?);
            }
            return Ok(n);
        }
        if (k1 == NEW_KW && k2 == VARIABLE_KW) || (k1 == VARIABLE_KW && k2 == IDENT) {
            if self.at(NEW_KW) {
                n.tok(self.bump());
            }
            n.tok(self.bump());
            let mut d = Raw::new(IDENT_DECL);
            d.tok(self.expect(IDENT, "a variable name")?);
            n.node(d);
            return Ok(n);
        }
        let level_kw = |k| matches!(k, STATE_KW | ACTION_KW | TEMPORAL_KW);
        if (k1 == NEW_KW && level_kw(k2)) || (level_kw(k1) && decl_start(k2)) {
            if self.at(NEW_KW) {
                n.tok(self.bump());
            }
            n.tok(self.bump());
            n.node(self.ident_or_some_fix_decl()?);
            return Ok(n);
        }
        Err(self.expected("a declaration after `NEW`"))
    }
}
