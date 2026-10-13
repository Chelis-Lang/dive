//! Environments: frames of slots, thunks, operator values, the dynamic
//! substitution context, and the states an expression is evaluated in.
//!
//! TLC binds names in an immutable linked `Context` (`tlc2.util.Context`) and
//! looks them up by semantic node. dive resolves names to frame slots when
//! lowering (ADR 0013 §2), so an [`Env`] is a frame of slots plus TLC's
//! dynamically scoped part: instance substitutions and `@`.
//!
//! **Persistence.** A thunk, a lazy set or a lazy function captures the
//! environment it was created in, as TLC's `LazyValue` captures its
//! `Context`. Frames are therefore copy-on-write: binding a slot clones the
//! frame only if something captured it ([`Arc::make_mut`]), so a quantifier
//! that rebinds its variable never changes what an earlier capture sees.

use std::sync::{Arc, Mutex, PoisonError};

use dive_ir::{BuiltinOp, ConstId, DeclId, DefId, ExprId, StdOp};
use dive_value::Value;

use crate::lazy::LazyFcn;

/// TLC's `EvalControl` flags.
pub(crate) mod control {
    /// `KeepLazy`: function constructors stay lambdas.
    pub(crate) const KEEP_LAZY: u8 = 1;
    /// `Primed`.
    pub(crate) const PRIMED: u8 = 1 << 1;
    /// `Enabled`.
    pub(crate) const ENABLED: u8 = 1 << 2;
    /// `Init`.
    pub(crate) const INIT: u8 = 1 << 3;
    /// `Const`.
    pub(crate) const CONST: u8 = 1 << 4;
    /// `EvalControl.Clear`.
    pub(crate) const CLEAR: u8 = 0;

    /// `EvalControl.semanticallyEquivalent`: equal up to `KeepLazy`, `Init`
    /// and `Const`.
    pub(crate) fn equivalent(a: u8, b: u8) -> bool {
        let mask = !(KEEP_LAZY | INIT | CONST);
        a & mask == b & mask
    }
}

/// A state as the evaluator sees it: TLC's `TLCState.Null` (no state),
/// `TLCState.Empty` (a state with no variable assigned), or the values of the
/// root module's variables by [`dive_ir::VarId`], `None` where unassigned.
#[derive(Clone, Debug, Default)]
pub enum StateRef {
    /// `TLCState.Null`.
    Null,
    /// `TLCState.Empty`.
    #[default]
    Empty,
    /// A (possibly partial) state.
    Vals(Arc<[Option<Value>]>),
}

impl StateRef {
    /// The value of a variable, if assigned.
    pub(crate) fn get(&self, var: usize) -> Option<&Value> {
        match self {
            Self::Vals(v) => v.get(var).and_then(Option::as_ref),
            _ => None,
        }
    }

    /// `TLCState.isSubset(self, cached)`: every variable assigned in `cached`
    /// has the same value in `self`.
    pub(crate) fn includes(&self, cached: &Self) -> bool {
        match cached {
            Self::Null | Self::Empty => true,
            Self::Vals(c) => {
                if let Self::Vals(s) = self
                    && Arc::ptr_eq(s, c)
                {
                    return true;
                }
                c.iter()
                    .enumerate()
                    .all(|(i, cv)| cv.as_ref().is_none_or(|cv| self.get(i) == Some(cv)))
            }
        }
    }
}

/// The states and control flags an expression is evaluated under: TLC's
/// `(s0, s1, control)`.
#[derive(Clone, Debug, Default)]
pub struct St {
    /// The current state (`s0`).
    pub s0: StateRef,
    /// The successor (`s1`).
    pub s1: StateRef,
    pub(crate) control: u8,
}

impl St {
    /// Constant-level evaluation, as TLC evaluates `ASSUME`s
    /// (`Tool.isValid(expr)`: `Empty`, `Empty`, `EvalControl.Const`).
    #[must_use]
    pub fn constant() -> Self {
        Self {
            s0: StateRef::Empty,
            s1: StateRef::Empty,
            control: control::CONST,
        }
    }

    /// TLC's `(TLCState.Empty, TLCState.Empty, EvalControl.Clear)`: what
    /// `Tool.getActions` evaluates the arguments of inlined operators and the
    /// domains of split `\E`s in.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            s0: StateRef::Empty,
            s1: StateRef::Empty,
            control: control::CLEAR,
        }
    }

    /// A state predicate in `state` (`Empty` successor, `Clear`).
    #[must_use]
    pub fn state(state: Arc<[Option<Value>]>) -> Self {
        Self {
            s0: StateRef::Vals(state),
            s1: StateRef::Empty,
            control: control::CLEAR,
        }
    }

    /// An initial predicate on a supplied state (`Empty` successor, `Init`).
    ///
    /// Unlike ordinary state checks, this evaluates `TLCGet("level")` as 0.
    /// Use [`Self::state`] for invariants and implied initial properties,
    /// which are checked after the initial state has been generated.
    ///
    /// ```
    /// use dive_eval::St;
    /// use std::sync::Arc;
    /// let initial = St::initial(Arc::from([Some(0.into())]));
    /// ```
    #[must_use]
    pub fn initial(state: Arc<[Option<Value>]>) -> Self {
        Self {
            control: control::INIT,
            ..Self::state(state)
        }
    }

    /// An action between `s0` and `s1` (`Clear`).
    #[must_use]
    pub fn action(s0: Arc<[Option<Value>]>, s1: Arc<[Option<Value>]>) -> Self {
        Self {
            s0: StateRef::Vals(s0),
            s1: StateRef::Vals(s1),
            control: control::CLEAR,
        }
    }

    pub(crate) fn with_control(&self, control: u8) -> Self {
        Self {
            control,
            ..self.clone()
        }
    }

    pub(crate) fn keep_lazy(&self) -> Self {
        self.with_control(self.control | control::KEEP_LAZY)
    }

    pub(crate) fn is(&self, flag: u8) -> bool {
        self.control & flag != 0
    }

    /// `EvalControl.setPrimedIfEnabled`.
    pub(crate) fn primed_if_enabled(control: u8) -> u8 {
        if control & control::ENABLED != 0 {
            control | control::PRIMED
        } else {
            control
        }
    }
}

/// The result of evaluating an expression: a value, or a function TLC keeps
/// as a lambda (`FcnLambdaValue`).
#[derive(Clone, Debug)]
pub enum Ev {
    /// A value.
    V(Value),
    /// A lazy function.
    F(Arc<LazyFcn>),
}

impl From<Value> for Ev {
    fn from(v: Value) -> Self {
        Self::V(v)
    }
}

/// An operator value: what an operator parameter, an operator argument or a
/// substituted constant operator is bound to.
#[derive(Clone, Debug)]
pub(crate) enum OpVal {
    /// A user definition. For a `LET` operator or `LAMBDA`, `parent` is the
    /// frame it closes over.
    Def {
        def: DefId,
        parent: Option<Arc<Frame>>,
        /// The dynamic context it was passed in (TLC's `OpLambdaValue` keeps
        /// its context).
        dynamic: Option<Dyn>,
    },
    /// A Java override.
    Std(StdOp),
    /// A built-in operator.
    Builtin(BuiltinOp),
    /// A constant operator bound by the configuration.
    Const(ConstId),
    /// A declaration with no binding (`TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR`).
    Unbound(Arc<str>),
}

/// What a slot or a substitution holds.
#[derive(Clone, Debug, Default)]
pub(crate) enum Sv {
    /// Not bound yet.
    #[default]
    Unset,
    /// A value.
    Val(Value),
    /// A pure unknown binder value used only by static level/purity analysis.
    AnalysisBound,
    /// A lazy function.
    Fcn(Arc<LazyFcn>),
    /// A `LazyValue`.
    Thunk(Arc<Thunk>),
    /// An operator.
    Op(OpVal),
}

impl From<Ev> for Sv {
    fn from(ev: Ev) -> Self {
        match ev {
            Ev::V(v) => Self::Val(v),
            Ev::F(f) => Self::Fcn(f),
        }
    }
}

/// A frame: the slots of one definition's activation.
#[derive(Clone, Debug)]
pub(crate) struct Frame {
    /// The definition whose frame this is.
    pub(crate) def: DefId,
    /// Its frame depth ([`dive_ir::Def::depth`]).
    pub(crate) depth: u32,
    pub(crate) slots: Vec<Sv>,
    /// The frame a `LET` operator or `LAMBDA` closes over.
    pub(crate) parent: Option<Arc<Frame>>,
}

/// One instance substitution in the dynamic context.
#[derive(Debug)]
pub(crate) struct SubstLink {
    pub(crate) decl: DeclId,
    pub(crate) val: Sv,
    /// The number of links up to and including this one.
    pub(crate) depth: usize,
    pub(crate) next: Option<Arc<SubstLink>>,
}

/// TLC's dynamically scoped bindings: instance substitutions and `@`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Dyn {
    pub(crate) subst: Option<Arc<SubstLink>>,
    pub(crate) at: Option<Value>,
    /// `ENABLED` evaluates its body in `Context.branch(c)`: a lookup with
    /// TLC's `cutoff` flag (primed evaluation of a variable, and the
    /// enumerator's `getVar`) stops there, so substitutions made outside the
    /// `ENABLED`, those at depth `<=` this, are hidden from it.
    pub(crate) cut: Option<usize>,
}

impl Dyn {
    pub(crate) fn lookup(&self, decl: DeclId) -> Option<&Sv> {
        self.lookup_cut(decl, false)
    }

    /// `Context.lookup(var, cutoff)`.
    pub(crate) fn lookup_cut(&self, decl: DeclId, cutoff: bool) -> Option<&Sv> {
        let floor = if cutoff { self.cut.unwrap_or(0) } else { 0 };
        let mut cur = self.subst.as_deref();
        while let Some(link) = cur {
            if link.depth <= floor {
                return None;
            }
            if link.decl == decl {
                return Some(&link.val);
            }
            cur = link.next.as_deref();
        }
        None
    }

    /// Conses a substitution onto the context.
    pub(crate) fn push(&mut self, decl: DeclId, val: Sv) {
        let next = self.subst.take();
        let depth = next.as_ref().map_or(0, |l| l.depth) + 1;
        self.subst = Some(Arc::new(SubstLink {
            decl,
            val,
            depth,
            next,
        }));
    }

    /// `Context.branch`: marks the current depth.
    #[cfg(feature = "enumerate")]
    pub(crate) fn branch(&mut self) {
        self.cut = Some(self.subst.as_ref().map_or(0, |l| l.depth));
    }
}

/// An environment: the current frame and the dynamic context.
#[derive(Clone, Debug)]
pub(crate) struct Env {
    pub(crate) frame: Arc<Frame>,
    pub(crate) dynamic: Dyn,
}

impl Env {
    /// A fresh frame for definition `def` with `slots` slots.
    pub(crate) fn new(
        def: DefId,
        depth: u32,
        slots: usize,
        parent: Option<Arc<Frame>>,
        dynamic: Dyn,
    ) -> Self {
        Self {
            frame: Arc::new(Frame {
                def,
                depth,
                slots: vec![Sv::Unset; slots],
                parent,
            }),
            dynamic,
        }
    }

    /// Binds a slot of the current frame (copy-on-write).
    pub(crate) fn bind(&mut self, slot: usize, v: Sv) {
        let frame = Arc::make_mut(&mut self.frame);
        if slot >= frame.slots.len() {
            frame.slots.resize(slot + 1, Sv::Unset);
        }
        frame.slots[slot] = v;
    }

    /// The frame `up` levels out.
    pub(crate) fn frame_up(&self, up: u32) -> Option<&Arc<Frame>> {
        let mut f = &self.frame;
        for _ in 0..up {
            f = f.parent.as_ref()?;
        }
        Some(f)
    }

    /// The frame at nesting depth `depth`, walking outwards.
    pub(crate) fn frame_at_depth(&self, depth: u32) -> Option<&Arc<Frame>> {
        let mut f = &self.frame;
        while f.depth > depth {
            f = f.parent.as_ref()?;
        }
        (f.depth == depth).then_some(f)
    }
}

/// A forced thunk's value and the circumstances it was computed under.
#[derive(Debug)]
pub(crate) struct Cached {
    pub(crate) val: Ev,
    pub(crate) s0: StateRef,
    pub(crate) s1: StateRef,
    pub(crate) control: u8,
}

/// TLC's `LazyValue`: an expression with its environment, forced at most once
/// under TLC's caching rule (`LazyValue.getCachedValue`).
#[derive(Debug)]
pub(crate) struct Thunk {
    pub(crate) expr: ExprId,
    pub(crate) env: Env,
    pub(crate) cachable: bool,
    pub(crate) cache: Mutex<Option<Cached>>,
}

impl Thunk {
    pub(crate) fn new(expr: ExprId, env: Env, cachable: bool) -> Arc<Self> {
        Arc::new(Self {
            expr,
            env,
            cachable,
            cache: Mutex::new(None),
        })
    }

    /// `LazyValue.getCachedValue`.
    pub(crate) fn cached(&self, st: &St) -> Option<Ev> {
        if !self.cachable {
            return None;
        }
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let c = cache.as_ref()?;
        (st.s0.includes(&c.s0)
            && st.s1.includes(&c.s1)
            && control::equivalent(st.control, c.control))
        .then(|| c.val.clone())
    }

    /// Records a forced value.
    pub(crate) fn store(&self, st: &St, val: &Ev) {
        if !self.cachable {
            return;
        }
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        *cache = Some(Cached {
            val: val.clone(),
            s0: st.s0.clone(),
            s1: st.s1.clone(),
            control: st.control,
        });
    }
}

/// Whether `env` holds, directly or through the values it binds, a cached
/// thunk or a lazy function whose expression has side effects
/// (`Effects::IMPURE`). See `Evaluator::scope_shares_side_effects`.
pub(crate) fn shares_side_effects(ir: &dive_ir::Ir, env: &Env) -> bool {
    captures_with_effects(ir, env, dive_ir::Effects::IMPURE, false, None)
}

/// Captured operators are opaque here: callers can supply them through the
/// Rust scope API without a corresponding statically analysed IR call site.
/// All thunks/functions must also have closed effects before filter execution.
pub(crate) fn filter_has_unproved_captures(
    ir: &dive_ir::Ir,
    env: &Env,
    forbidden: dive_ir::Effects,
) -> bool {
    captures_with_effects(ir, env, forbidden, true, None)
}

/// Checks retained executable contexts, including actual captured callables,
/// without evaluating them or walking arbitrary stored values.
pub(crate) fn scope_has_effects(
    ir: &dive_ir::Ir,
    env: &Env,
    callable_effects: &dyn Fn(&OpVal) -> bool,
) -> bool {
    captures_with_effects(
        ir,
        env,
        dive_ir::Effects::IMPURE.union(dive_ir::Effects::LIVE_TLCGET),
        false,
        Some(callable_effects),
    )
}

fn captures_with_effects(
    ir: &dive_ir::Ir,
    env: &Env,
    forbidden: dive_ir::Effects,
    reject_operators: bool,
    callable_effects: Option<&dyn Fn(&OpVal) -> bool>,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut frames: Vec<&Arc<Frame>> = vec![&env.frame];
    let mut svs: Vec<&Sv> = Vec::new();
    let mut dyns: Vec<&Dyn> = vec![&env.dynamic];
    let impure = |e: ExprId| {
        ir.nodes()
            .get(e.index())
            .map_or(reject_operators, |n| n.effects.intersects(forbidden))
    };
    loop {
        if let Some(f) = frames.pop() {
            if seen.insert(Arc::as_ptr(f) as usize) {
                svs.extend(f.slots.iter());
                if let Some(p) = &f.parent {
                    frames.push(p);
                }
            }
        } else if let Some(d) = dyns.pop() {
            let mut cur = d.subst.as_deref();
            while let Some(link) = cur {
                if !seen.insert(std::ptr::from_ref(link) as usize) {
                    break;
                }
                svs.push(&link.val);
                cur = link.next.as_deref();
            }
        } else if let Some(sv) = svs.pop() {
            match sv {
                Sv::Thunk(t) => {
                    if seen.insert(Arc::as_ptr(t) as usize) {
                        if (reject_operators || callable_effects.is_some() || t.cachable)
                            && impure(t.expr)
                        {
                            return true;
                        }
                        frames.push(&t.env.frame);
                        dyns.push(&t.env.dynamic);
                    }
                }
                Sv::Fcn(f) => {
                    if seen.insert(Arc::as_ptr(f) as usize) {
                        if impure(f.body) {
                            return true;
                        }
                        frames.push(&f.env.frame);
                        dyns.push(&f.env.dynamic);
                    }
                }
                Sv::Op(op)
                    if reject_operators || callable_effects.is_some_and(|effects| effects(op)) =>
                {
                    return true;
                }
                Sv::Op(OpVal::Def {
                    parent, dynamic, ..
                }) => {
                    if let Some(p) = parent {
                        frames.push(p);
                    }
                    if let Some(d) = dynamic {
                        dyns.push(d);
                    }
                }
                Sv::Unset | Sv::Val(_) | Sv::AnalysisBound | Sv::Op(_) => {}
            }
        } else {
            return false;
        }
    }
}
