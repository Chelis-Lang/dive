//! Constructor-grounded result facts, separate from expression effects.

use std::sync::{Arc, Weak};

use dive_ir::{Arg, DefId, Expr, ExprId, OpRef, StdKind, StdOp, StdSupport};
use dive_value::{Fcn, Set, SetKind, Value};

use crate::core::Core;
use crate::env::{Env, Ev, Frame, StateRef, SubstLink, Sv, Thunk};
use crate::{ConstBinding, EvalError, Evaluator, HostControl, Result, Scope};

#[cfg(test)]
mod tests;

/// A checked own-call fact for the supplied source roots of one evaluator.
///
/// Every reachable concatenation in these roots has a tuple operand. This
/// says nothing about other native name creators, effects, model truth or
/// roots the caller omitted. There is no public constructor; a model driver
/// must supply its complete checking path before removing a sequential reason.
#[derive(Clone)]
pub struct TupleConcatSources {
    owner: Weak<Core>,
}

impl std::fmt::Debug for TupleConcatSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TupleConcatSources").finish_non_exhaustive()
    }
}

impl TupleConcatSources {
    /// Whether the inspected roots belong to this actual evaluator.
    #[must_use]
    pub fn belongs_to(&self, evaluator: &Evaluator) -> bool {
        self.owner.ptr_eq(&Arc::downgrade(&evaluator.core))
    }
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<()> {
    values
        .try_reserve(count)
        .map_err(|_| EvalError::resource_limit("tuple name analysis allocation"))
}

fn push<T>(values: &mut Vec<T>, value: T) -> Result<()> {
    reserve(values, 1)?;
    values.push(value);
    Ok(())
}

impl Evaluator {
    /// Inspects complete source roots for constructor-grounded tuple concatenation.
    ///
    /// Literal tuples and the unchanged native `Append` ground a finite monotone
    /// result analysis. All actuals remain unknown; recursion is never assumed
    /// tuple-valued. Every child is still inspected for its own calls. Opaque
    /// captures, native roots and higher-order calls decline without forcing a
    /// value, calling a provider, or changing permissions or evaluation.
    ///
    /// `definitions` are bare configuration/assumption roots, not merely native
    /// definitions seen as fully applied callees. This fact covers only the
    /// supplied roots; it does not grant model-wide name or parallel authority.
    /// The walk has no count or depth ceiling and polls the supplied control,
    /// or the installed evaluator control when it is absent.
    ///
    /// ```no_run
    /// # fn inspect(ev: &dive_eval::Evaluator, root: (dive_ir::ExprId, dive_eval::Scope))
    /// # -> dive_eval::Result<()> {
    /// let fact = ev.tuple_concat_sources(&[root], &[], None)?;
    /// # let _ = fact; Ok(()) }
    /// ```
    ///
    /// # Errors
    /// Actual control interruption or a failed analysis allocation.
    pub fn tuple_concat_sources(
        &self,
        roots: &[(ExprId, Scope)],
        definitions: &[DefId],
        control: Option<&dyn HostControl>,
    ) -> Result<Option<TupleConcatSources>> {
        let mut analysis = Analysis {
            ev: self,
            control,
            blocked: Vec::new(),
        };
        analysis.poll()?;
        // A direct standard reference has lost any configured-constant origin.
        // Keep these origins unknown, including a same-target override.
        for binding in self.core.consts.iter().flatten() {
            analysis.poll()?;
            if let ConstBinding::Def(def) = binding
                && let Some(op) = self.core.ir.def(self.redirected(*def)).std
            {
                push(&mut analysis.blocked, op)?;
            }
        }
        for (index, def) in self.core.ir.defs().iter().enumerate() {
            analysis.poll()?;
            let id = DefId(
                u32::try_from(index)
                    .map_err(|_| EvalError::resource_limit("tuple name definition index"))?,
            );
            if let Some(op) = def.std
                && (self.redirected(id) != id || self.is_value_override(id))
            {
                push(&mut analysis.blocked, op)?;
            }
        }
        let Some(mut sources) = analysis.captures(roots)? else {
            return Ok(None);
        };
        for &def in definitions {
            analysis.poll()?;
            let Some(body) = analysis.user_body(def, 0) else {
                return Ok(None);
            };
            push(&mut sources, body)?;
        }
        let tuples = analysis.results()?;
        if !analysis.calls(&sources, &tuples)? {
            return Ok(None);
        }
        analysis.poll()?;
        Ok(Some(TupleConcatSources {
            owner: Arc::downgrade(&self.core),
        }))
    }
}

struct Analysis<'a> {
    ev: &'a Evaluator,
    control: Option<&'a dyn HostControl>,
    blocked: Vec<StdOp>,
}

impl Analysis<'_> {
    fn poll(&self) -> Result<()> {
        if let Some(control) = self.control {
            control.check().map_err(EvalError::interrupted)
        } else {
            self.ev.core.host.check_control()
        }
    }

    fn native(&self, op: &OpRef, arity: usize) -> Option<StdOp> {
        let op = match op {
            OpRef::Std(op) => *op,
            OpRef::Def(def) => {
                let target = self.ev.ir().defs().get(def.index())?;
                if self.ev.redirected(*def) != *def
                    || self.ev.is_value_override(*def)
                    || target.params.len() != arity
                    || target.params.iter().any(|p| p.arity != 0)
                {
                    return None;
                }
                target.std?
            }
            _ => return None,
        };
        (op.kind() == StdKind::Value
            && op.support() == StdSupport::Core
            && usize::try_from(op.java_arity()).ok() == Some(arity)
            && !self.blocked.contains(&op)
            && !self.ev.core.std_redirects.contains_key(&op))
        .then_some(op)
    }

    fn user_body(&self, def: DefId, arity: usize) -> Option<ExprId> {
        let target = self.ev.ir().defs().get(def.index())?;
        (target.std.is_none()
            && self.ev.redirected(def) == def
            && !self.ev.is_value_override(def)
            && target.params.len() == arity
            && target.params.iter().all(|p| p.arity == 0))
        .then_some(target.body)
    }

    // Result dependencies exclude children whose values do not determine the
    // outer result kind. The independent call walk below still visits them.
    fn result_edges(&self, expr: &Expr, edges: &mut Vec<ExprId>) -> Result<()> {
        match expr {
            Expr::If { then, other, .. } => {
                push(edges, *then)?;
                push(edges, *other)?;
            }
            Expr::Case { arms, other } => {
                for arm in arms {
                    push(edges, arm.value)?;
                }
                if let Some(other) = other {
                    push(edges, *other)?;
                }
            }
            Expr::Label { body, .. }
            | Expr::Selected(body)
            | Expr::Let { body, .. }
            | Expr::Subst { body, .. } => push(edges, *body)?,
            Expr::Def(def) => {
                if let Some(body) = self.user_body(*def, 0) {
                    push(edges, body)?;
                }
            }
            Expr::Apply { callee, args } if args.iter().all(|a| matches!(a, Arg::Expr { .. })) => {
                match self.native(callee, args.len()) {
                    Some(StdOp::SequencesConcat) => {
                        for arg in args {
                            if let Arg::Expr { expr, .. } = arg {
                                push(edges, *expr)?;
                            }
                        }
                    }
                    Some(_) => {}
                    None => {
                        if let OpRef::Def(def) = callee
                            && let Some(body) = self.user_body(*def, args.len())
                        {
                            push(edges, body)?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn results(&self) -> Result<Vec<bool>> {
        let count = self.ev.ir().nodes().len();
        let mut users = Vec::new();
        reserve(&mut users, count)?;
        users.resize_with(count, Vec::new);
        let mut tuples = Vec::new();
        reserve(&mut tuples, count)?;
        tuples.resize(count, false);
        let mut work = Vec::new();
        for (index, node) in self.ev.ir().nodes().iter().enumerate() {
            self.poll()?;
            let id = ExprId(
                u32::try_from(index)
                    .map_err(|_| EvalError::resource_limit("tuple name expression index"))?,
            );
            let mut edges = Vec::new();
            self.result_edges(&node.expr, &mut edges)?;
            for edge in edges {
                let Some(dependents) = users.get_mut(edge.index()) else {
                    return Err(EvalError::bug("tuple name result dependency"));
                };
                push(dependents, id)?;
            }
            push(&mut work, id)?;
        }
        while let Some(id) = work.pop() {
            self.poll()?;
            if tuples[id.index()] {
                continue;
            }
            let expr = self.ev.ir().expr(id);
            let mut edges = Vec::new();
            self.result_edges(expr, &mut edges)?;
            let grounded = matches!(expr, Expr::Tuple(_))
                || matches!(expr, Expr::Apply { callee, args }
                    if args.iter().all(|a| matches!(a, Arg::Expr { .. }))
                    && self.native(callee, args.len()) == Some(StdOp::SequencesAppend));
            let proved = if matches!(expr, Expr::Apply { callee, args }
                if self.native(callee, args.len()) == Some(StdOp::SequencesConcat))
            {
                edges.iter().any(|e| tuples[e.index()])
            } else {
                !edges.is_empty() && edges.iter().all(|e| tuples[e.index()])
            };
            if grounded || proved {
                tuples[id.index()] = true;
                for &user in &users[id.index()] {
                    push(&mut work, user)?;
                }
            }
        }
        Ok(tuples)
    }

    fn calls(&self, sources: &[ExprId], tuples: &[bool]) -> Result<bool> {
        let mut seen = Vec::new();
        reserve(&mut seen, self.ev.ir().nodes().len())?;
        seen.resize(self.ev.ir().nodes().len(), false);
        let mut work = Vec::new();
        reserve(&mut work, sources.len())?;
        work.extend_from_slice(sources);
        while let Some(id) = work.pop() {
            self.poll()?;
            let Some(visited) = seen.get_mut(id.index()) else {
                return Ok(false);
            };
            if *visited {
                continue;
            }
            *visited = true;
            let expr = self.ev.ir().expr(id);
            match expr {
                Expr::Apply { callee, args } => {
                    if args.iter().any(|a| matches!(a, Arg::Op { .. })) {
                        return Ok(false);
                    }
                    if let Some(op) = self.native(callee, args.len()) {
                        if op.creates_runtime_names() && op != StdOp::SequencesConcat {
                            return Ok(false);
                        }
                        if op == StdOp::SequencesConcat
                            && !args.iter().any(|a| {
                                matches!(a,
                                Arg::Expr { expr, .. } if tuples[expr.index()])
                            })
                        {
                            return Ok(false);
                        }
                    } else if let OpRef::Def(def) = callee
                        && let Some(body) = self.user_body(*def, args.len())
                    {
                        push(&mut work, body)?;
                    } else {
                        return Ok(false);
                    }
                }
                Expr::Def(def) => {
                    if let Some(op) = self.native(&OpRef::Def(*def), 0) {
                        if op.creates_runtime_names() {
                            return Ok(false);
                        }
                    } else if let Some(body) = self.user_body(*def, 0) {
                        push(&mut work, body)?;
                    } else {
                        return Ok(false);
                    }
                }
                Expr::Let { defs, .. } => {
                    for def in defs {
                        let Some(target) = self.ev.ir().defs().get(def.index()) else {
                            return Ok(false);
                        };
                        push(&mut work, target.body)?;
                    }
                }
                _ => {}
            }
            expr.try_for_each_child(|child| push(&mut work, child))?;
        }
        Ok(true)
    }
}

enum Capture {
    Environment(Env),
    Slot(Sv),
    Value(Value),
    State(StateRef),
}

#[derive(Default)]
struct Retained {
    frames: Vec<Arc<Frame>>,
    links: Vec<Arc<SubstLink>>,
    functions: Vec<Fcn>,
    sets: Vec<Set>,
    thunks: Vec<Arc<Thunk>>,
    states: Vec<Arc<[Option<Value>]>>,
}

impl Analysis<'_> {
    fn captures(&self, roots: &[(ExprId, Scope)]) -> Result<Option<Vec<ExprId>>> {
        let mut work = Vec::new();
        let mut sources = Vec::new();
        let mut retained = Retained::default();
        for (expr, scope) in roots {
            self.poll()?;
            if !Arc::ptr_eq(&scope.owner, &self.ev.core.ir)
                || !Arc::ptr_eq(&scope.names, &self.ev.core.names)
                || !scope.context.ptr_eq(&Arc::downgrade(&self.ev.core))
            {
                return Ok(None);
            }
            push(&mut sources, *expr)?;
            if let Some(wrapper) = scope.wrapper {
                push(&mut sources, wrapper)?;
            }
            push(&mut work, Capture::Environment(scope.env.clone()))?;
        }
        for value in self.ev.core.def_values.values() {
            push(&mut work, Capture::Value(value.clone()))?;
        }
        for value in self.ev.core.startup.iter().chain(&self.ev.core.def_cache) {
            if let Some(value) = value.get() {
                push(&mut work, Capture::Value(value.clone()))?;
            }
        }
        for binding in self.ev.core.consts.iter().flatten() {
            if let ConstBinding::Value(value) = binding {
                push(&mut work, Capture::Value(value.clone()))?;
            }
        }
        while let Some(task) = work.pop() {
            self.poll()?;
            let known = match task {
                Capture::Environment(env) => {
                    self.capture_environment(env, &mut work, &mut retained)?
                }
                Capture::Slot(slot) => {
                    self.capture_slot(slot, &mut work, &mut sources, &mut retained)?
                }
                Capture::Value(value) => self.capture_value(value, &mut work, &mut retained)?,
                Capture::State(StateRef::Null | StateRef::Empty) => true,
                Capture::State(StateRef::Vals(values)) => {
                    if !retained
                        .states
                        .iter()
                        .any(|prior| Arc::ptr_eq(prior, &values))
                    {
                        push(&mut retained.states, values.clone())?;
                        for value in values.iter().flatten() {
                            push(&mut work, Capture::Value(value.clone()))?;
                        }
                    }
                    true
                }
                Capture::State(StateRef::Named(_)) => false,
            };
            if !known {
                return Ok(None);
            }
        }
        Ok(Some(sources))
    }

    fn capture_environment(
        &self,
        env: Env,
        work: &mut Vec<Capture>,
        retained: &mut Retained,
    ) -> Result<bool> {
        if env.dynamic.at.is_some() || env.dynamic.cut.is_some() {
            return Ok(false);
        }
        let mut link = env.dynamic.subst.clone();
        while let Some(current) = link {
            self.poll()?;
            if retained
                .links
                .iter()
                .any(|prior| Arc::ptr_eq(prior, &current))
            {
                break;
            }
            push(&mut retained.links, current.clone())?;
            push(work, Capture::Slot(current.val.clone()))?;
            link = current.next.clone();
        }
        let mut frame = Some(env.frame);
        while let Some(current) = frame {
            self.poll()?;
            if retained
                .frames
                .iter()
                .any(|prior| Arc::ptr_eq(prior, &current))
            {
                break;
            }
            push(&mut retained.frames, current.clone())?;
            for slot in &current.slots {
                push(work, Capture::Slot(slot.clone()))?;
            }
            frame = current.parent.clone();
        }
        Ok(true)
    }

    fn capture_slot(
        &self,
        slot: Sv,
        work: &mut Vec<Capture>,
        sources: &mut Vec<ExprId>,
        retained: &mut Retained,
    ) -> Result<bool> {
        self.poll()?;
        match slot {
            Sv::Thunk(thunk) => {
                if retained
                    .thunks
                    .iter()
                    .any(|prior| Arc::ptr_eq(prior, &thunk))
                {
                    return Ok(true);
                }
                push(&mut retained.thunks, thunk.clone())?;
                push(sources, thunk.expr)?;
                push(work, Capture::Environment(thunk.env.clone()))?;
                for cache in thunk.stored_caches().into_iter().flatten() {
                    let (Ev::V(value), before, after) = cache else {
                        return Ok(false);
                    };
                    push(work, Capture::Value(value))?;
                    push(work, Capture::State(before))?;
                    push(work, Capture::State(after))?;
                }
            }
            Sv::Val(value) => push(work, Capture::Value(value))?,
            Sv::Unset | Sv::AnalysisBound | Sv::AnalysisConstant => {}
            Sv::Fcn(_) | Sv::Op(_) => return Ok(false),
        }
        Ok(true)
    }

    fn capture_value(
        &self,
        value: Value,
        work: &mut Vec<Capture>,
        retained: &mut Retained,
    ) -> Result<bool> {
        match value {
            Value::Fcn(function) => {
                if retained
                    .functions
                    .iter()
                    .any(|prior| prior.ptr_eq(&function))
                {
                    return Ok(true);
                }
                push(&mut retained.functions, function.clone())?;
                for (key, value) in function.iter() {
                    self.poll()?;
                    push(work, Capture::Value(key))?;
                    push(work, Capture::Value(value.clone()))?;
                }
            }
            Value::Set(set) => {
                // A warm provider is still opaque. Inspect representation before
                // any cached payload, without asking the provider about itself.
                if !matches!(
                    set.kind(),
                    SetKind::Raw
                        | SetKind::Enum
                        | SetKind::Interval
                        | SetKind::Nat
                        | SetKind::Int
                        | SetKind::Strings
                ) {
                    return Ok(false);
                }
                if retained.sets.iter().any(|prior| prior.ptr_eq(&set)) {
                    return Ok(true);
                }
                push(&mut retained.sets, set.clone())?;
                if matches!(set.kind(), SetKind::Raw | SetKind::Enum) {
                    let Some(values) = set.stored_elements() else {
                        return Ok(false);
                    };
                    for value in values {
                        self.poll()?;
                        push(work, Capture::Value(value.clone()))?;
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
