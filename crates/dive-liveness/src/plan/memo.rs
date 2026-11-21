//! Positive results of the scoped level and purity walks. The model, evaluator,
//! lexical binding shapes and mode must agree. A value-dependent operation
//! invalidates every enclosing in-progress proof, including unused call actuals.
use dive_eval::{AnalysisFrames, Evaluator, Scope, SlotView};
use dive_ir::{Arg, Expr, ExprId, Level, OpRef};
use std::collections::HashMap;

type Key = (ExprId, usize, bool);
const MAX_ENTRIES: usize = 16_384;

pub(super) struct Memo<'a> {
    frames: AnalysisFrames<'a>,
    pub(super) levels: HashMap<Key, (Level, usize)>,
    pure: HashMap<Key, usize>,
    pub(super) demands: usize,
}

pub(super) struct Pending {
    siblings: usize,
    demands: usize,
    key: Key,
    depth: usize,
}

impl<'a> Memo<'a> {
    pub(super) fn new(evaluator: &'a Evaluator) -> Self {
        Self {
            frames: AnalysisFrames::new(evaluator),
            levels: HashMap::new(),
            pure: HashMap::new(),
            demands: 0,
        }
    }

    pub(super) fn key(
        &mut self,
        ev: &Evaluator,
        scope: &Scope,
        expr: ExprId,
        mode: bool,
    ) -> Option<Key> {
        let node = ev.ir().nodes().get(expr.index())?;
        let structural = match &node.expr {
            Expr::Apply { callee, args } => {
                matches!(callee, OpRef::Def(_) | OpRef::Std(_) | OpRef::Builtin(_))
                    && args.iter().all(|a| matches!(a, Arg::Expr { .. }))
            }
            Expr::Subst { .. }
            | Expr::SubstRef { .. }
            | Expr::Enabled(_)
            | Expr::TemporalQuant { .. }
            | Expr::RecFcn { .. } => false,
            _ => true,
        };
        if structural && let Some(shape) = self.frames.identify(scope) {
            return Some((expr, shape, mode));
        }
        self.demands += 1;
        None
    }

    // Literal evaluation is fixed for this IR and evaluator. Reading an already
    // bound scalar merely copies it into the new scope. An unknown constant
    // consistently takes the analysis-only path. Every other eager actual must
    // run under its own concrete binding, even when the callee ignores it.
    pub(super) fn eager(&mut self, ev: &Evaluator, scope: &Scope, arg: ExprId) {
        match ev.ir().expr(arg) {
            Expr::Bool(_) | Expr::Num(_) | Expr::Str(_) => {}
            Expr::Local(r)
                if matches!(
                    ev.scope_slot(scope, *r),
                    SlotView::Value(_) | SlotView::AnalysisConstant
                ) => {}
            _ => self.demands += 1,
        }
    }

    pub(super) fn record_level(
        &mut self,
        key: Option<Key>,
        before: usize,
        level: Level,
        depth: usize,
    ) {
        if self.demands == before
            && self.levels.len() < MAX_ENTRIES
            && let Some(key) = key
        {
            self.levels.insert(key, (level, depth));
        }
    }

    pub(super) fn pure(
        &self,
        key: Key,
        depth: usize,
        siblings: usize,
        pending: &mut Vec<Pending>,
    ) -> bool {
        // A proof that fit at a deeper starting point satisfies all relative
        // depth guards at a shallower one. Starting deeper requires a fresh walk.
        if self.pure.get(&key).is_some_and(|proved| depth <= *proved) {
            return true;
        }
        pending.push(Pending {
            siblings,
            demands: self.demands,
            key,
            depth,
        });
        false
    }

    // The purity walk is a LIFO traversal. Once it returns to the number of
    // siblings present at entry, every descendant has passed its own checks.
    pub(super) fn finish(&mut self, siblings: usize, pending: &mut Vec<Pending>) {
        while pending.last().is_some_and(|p| p.siblings == siblings) {
            if let Some(p) = pending.pop()
                && self.demands == p.demands
                && self.pure.len() < MAX_ENTRIES
            {
                self.pure.insert(p.key, p.depth);
            }
        }
    }
}
