//! Direct temporal semantics on an ultimately periodic word (ADR 0018 §8).
//!
//! This module has no tableau, graph search, or successor enumerator. Each
//! subformula yields a truth value at every position of the given word.

use dive_eval::{Scope, St};
use dive_ir::{Arg, Effects, Expr, ExprId, JunctionKind, Level, OpRef, StdOp};
use dive_model::{Model, Pred};
use dive_value::Value;

use crate::State;

/// Why temporal evaluation cannot establish a truth value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemporalError {
    /// The word or a predicate is malformed.
    Invalid(String),
    /// A named feature cannot be decided independently yet.
    Unsupported(String),
}

impl std::fmt::Display for TemporalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(s) => write!(f, "invalid temporal word: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported temporal validation: {s}"),
        }
    }
}

impl std::error::Error for TemporalError {}

/// Evaluates `formula` at every position of an ultimately periodic word.
/// The last position is followed by `back_to`; setting it to the last
/// position describes infinite stuttering. This evaluates the formula only:
/// callers must separately check initial states, transitions and constraints.
///
/// ```
/// use std::sync::Arc;
/// use dive_trace::temporal::evaluate_lasso;
/// use dive_model::Pred;
/// use dive_value::Value;
/// let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\n\
///            Next == x' = ~x\nP == []<>x\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", src, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis)?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc())?;
/// let p = model.ir.root_def("P").unwrap();
/// let pred = Pred::new(model.ir.def(p).body, model.evaluator.scope(p));
/// let states = vec![Arc::from([Some(Value::Bool(true))]),
///                   Arc::from([Some(Value::Bool(false))])];
/// assert_eq!(evaluate_lasso(&model, &pred, &states, 0)?, vec![true, true]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Invalid words or nonboolean atoms; unsupported fairness, side effects,
/// temporal quantification, opaque scoped atoms, checker introspection or
/// resource bounds. `ENABLED` is deliberately
/// refused until independent successor-existence checking is available.
pub fn evaluate_lasso(
    model: &Model,
    formula: &Pred,
    states: &[State],
    back_to: usize,
) -> Result<Vec<bool>, TemporalError> {
    if states.is_empty() || back_to >= states.len() {
        return Err(TemporalError::Invalid("invalid lasso back edge".into()));
    }
    if states
        .iter()
        .any(|s| s.len() != model.ir.vars().len() || s.iter().any(Option::is_none))
    {
        return Err(TemporalError::Invalid(
            "a lasso state does not assign every variable".into(),
        ));
    }
    let mut cx = Word {
        model,
        states,
        back_to,
        cells: 0,
    };
    cx.eval(formula.expr, &formula.scope, 0)
}

struct Word<'a> {
    model: &'a Model,
    states: &'a [State],
    back_to: usize,
    cells: usize,
}

impl Word<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "one explicit arm per supported temporal constructor"
    )]
    fn eval(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<Vec<bool>, TemporalError> {
        self.cells = self.cells.saturating_add(self.states.len());
        if depth > 256 || self.cells > 1_000_000 {
            return Err(TemporalError::Unsupported(
                "temporal validation resource bound".into(),
            ));
        }
        let ev = &self.model.evaluator;
        let node = self.model.ir.node(expr);
        if node.effects.intersects(Effects::IMPURE | Effects::OPEN)
            || ev.scope_shares_side_effects(scope)
        {
            return Err(TemporalError::Unsupported(
                "side effects or dynamically bound constants in temporal validation".into(),
            ));
        }
        if node.effects.contains(Effects::USES_ENABLED) {
            return Err(TemporalError::Unsupported(
                "ENABLED: independent successor-existence checking is not implemented".into(),
            ));
        }
        if node.level <= Level::Action && !node.effects.contains(Effects::TEMPORAL) {
            return self.atom(expr, scope);
        }
        let child = depth + 1;
        match &node.expr {
            Expr::Always(e) | Expr::Eventually(e) => {
                let always = matches!(node.expr, Expr::Always(_));
                let values = self.eval(*e, scope, child)?;
                Ok(self.future(values, always))
            }
            Expr::Not(e) => Ok(self
                .eval(*e, scope, child)?
                .into_iter()
                .map(|b| !b)
                .collect()),
            Expr::Implies(a, b) | Expr::Equiv(a, b) | Expr::LeadsTo(a, b) => {
                let a = self.eval(*a, scope, child)?;
                let b = self.eval(*b, scope, child)?;
                let equivalent = matches!(node.expr, Expr::Equiv(..));
                let leads = matches!(node.expr, Expr::LeadsTo(..));
                let b = if leads { self.future(b, false) } else { b };
                let values = a
                    .into_iter()
                    .zip(b)
                    .map(|(a, b)| if equivalent { a == b } else { !a || b })
                    .collect();
                Ok(if leads {
                    self.future(values, true)
                } else {
                    values
                })
            }
            Expr::Junction { kind, items, .. } => {
                let conjunction = matches!(kind, JunctionKind::And);
                let mut values = vec![conjunction; self.states.len()];
                for item in items {
                    let rhs = self.eval(*item, scope, child)?;
                    for (a, b) in values.iter_mut().zip(rhs) {
                        *a = if conjunction { *a && b } else { *a || b };
                    }
                }
                Ok(values)
            }
            Expr::Def(d) => {
                let d = ev.redirected(*d);
                if ev.is_value_override(d) || self.model.ir.def(d).recursive {
                    return Err(TemporalError::Unsupported(
                        "overridden or recursive temporal definition".into(),
                    ));
                }
                let scope = ev
                    .scope_enter(scope, d)
                    .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
                self.eval(self.model.ir.def(d).body, &scope, child)
            }
            Expr::Selected(e) | Expr::Label { body: e, .. } => self.eval(*e, scope, child),
            Expr::Wf { .. } | Expr::Sf { .. } => Err(TemporalError::Unsupported(
                "fairness: independent ENABLED checking is not implemented".into(),
            )),
            other => Err(TemporalError::Unsupported(format!(
                "temporal constructor {}",
                other.tag()
            ))),
        }
    }

    fn atom(&self, expr: ExprId, scope: &Scope) -> Result<Vec<bool>, TemporalError> {
        self.check_atom_context(expr)?;
        let mut out = Vec::with_capacity(self.states.len());
        for (i, state) in self.states.iter().enumerate() {
            let next = if i + 1 == self.states.len() {
                self.back_to
            } else {
                i + 1
            };
            let st = St::action(state.clone(), self.states[next].clone());
            match self.model.evaluator.eval_scoped(scope, expr, &st) {
                Ok(Value::Bool(b)) => out.push(b),
                Ok(v) => {
                    return Err(TemporalError::Invalid(format!(
                        "temporal atom at position {i} is {}, not boolean",
                        v.kind_name()
                    )));
                }
                Err(e) => {
                    return Err(TemporalError::Unsupported(format!(
                        "temporal atom at position {i}: {e}"
                    )));
                }
            }
        }
        Ok(out)
    }

    // `TLCGet("level")` is deliberately not marked impure in the IR, but
    // its value is a search level, not a function of this periodic word.
    // Also refuse opaque local thunks/operator parameters: they could hide
    // such a read even when the conservative effects flags are clear.
    fn check_atom_context(&self, expr: ExprId) -> Result<(), TemporalError> {
        let mut todo = vec![expr];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(e) = todo.pop() {
            if !seen.insert(e) {
                continue;
            }
            let node = self.model.ir.expr(e);
            if matches!(node, Expr::Local(_) | Expr::SubstRef { .. }) {
                return Err(TemporalError::Unsupported(
                    "opaque scoped atom in temporal validation".into(),
                ));
            }
            let mut operators = Vec::new();
            match node {
                Expr::Def(d) => operators.push(OpRef::Def(*d)),
                Expr::Apply { callee, args } => {
                    operators.push(callee.clone());
                    operators.extend(args.iter().filter_map(|a| {
                        if let Arg::Op { op, .. } = a {
                            Some(op.clone())
                        } else {
                            None
                        }
                    }));
                }
                _ => {}
            }
            for op in operators {
                let standard = match op {
                    OpRef::Std(s) => Some(s),
                    OpRef::Def(d) => {
                        let d = self.model.evaluator.redirected(d);
                        if self.model.evaluator.is_value_override(d) {
                            continue;
                        }
                        let def = self.model.ir.def(d);
                        if def.std.is_none() {
                            todo.push(def.body);
                        }
                        def.std
                    }
                    OpRef::Param(_) | OpRef::Subst(_) | OpRef::Const(_) => {
                        return Err(TemporalError::Unsupported(
                            "dynamically bound temporal atom operator".into(),
                        ));
                    }
                    _ => None,
                };
                if standard == Some(StdOp::TlcTLCGet) {
                    return Err(TemporalError::Unsupported(
                        "TLCGet: checker introspection is not a function of a lasso".into(),
                    ));
                }
            }
            todo.extend(node.children());
        }
        Ok(())
    }

    fn future(&self, mut values: Vec<bool>, always: bool) -> Vec<bool> {
        let cycle = if always {
            values[self.back_to..].iter().all(|b| *b)
        } else {
            values[self.back_to..].iter().any(|b| *b)
        };
        values[self.back_to..].fill(cycle);
        for i in (0..self.back_to).rev() {
            values[i] = if always {
                values[i] && values[i + 1]
            } else {
                values[i] || values[i + 1]
            };
        }
        values
    }
}
